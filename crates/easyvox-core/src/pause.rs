//! Speech and pause detection: whether a recording holds any speech and where it starts
//! ([`SpeechDetector`]), where to cut a long recording into chunks (see `chunks.rs`), and
//! whether anything was said since the last preview pass ([`SpeechGate`]).
//!
//! Energy-based for now, on the levels of 30 ms frames. The recording's own frame levels are
//! measured once, as the audio arrives ([`FrameLevels`]): speech detection and the input level
//! meter (`level.rs`) both read them. A pause is a run of quiet frames, and a
//! frame is quiet when its level is well under the loud frames around it (their 90th
//! percentile), or under a silence level whatever the level around. By default that's 20 dB
//! under, or under about -55 dBFS (see [`Tuning`]'s `pause_*` and `gate_*` fields). Speech is
//! the other way round: frames well above the quietest ones (`speech_*`). It needs no model,
//! but it can't tell a breath, a cough or a loud room from speech. A VAD (e.g. Silero) can
//! replace it behind these functions.

use crate::SAMPLE_RATE;
use crate::tuning::{Tuning, db_to_linear};
use std::ops::Range;

/// Frame length, in ms: the resolution levels are measured at.
pub(crate) const FRAME_MS: usize = 30;
pub(crate) const FRAME: usize = SAMPLE_RATE as usize * FRAME_MS / 1000;

fn frame_levels(pcm: &[f32]) -> Vec<f32> {
    pcm.chunks(FRAME).map(|c| (c.iter().map(|x| x * x).sum::<f32>() / c.len() as f32).sqrt()).collect()
}

/// A recording's loudness, frame by frame: the RMS level of each whole 30 ms frame, from its
/// first sample. The capture thread measures each sample once, as it arrives
/// (`audio::SharedAudio::push`), so the speech detector and the level meter share one
/// measurement, and neither goes over the audio again. A frame's level is exactly what
/// `frame_levels` gives for it.
#[derive(Debug, Default)]
pub(crate) struct FrameLevels {
    levels: Vec<f32>,
    /// The sum of squares and the number of samples of the frame in progress.
    sum: f32,
    partial: usize,
}

impl FrameLevels {
    /// Adds the next samples of the recording.
    pub(crate) fn push(&mut self, pcm: &[f32]) {
        for x in pcm {
            self.sum += x * x;
            self.partial += 1;
            if self.partial == FRAME {
                self.levels.push((self.sum / FRAME as f32).sqrt());
                (self.sum, self.partial) = (0.0, 0);
            }
        }
    }

    /// The level of every whole frame so far.
    pub(crate) fn levels(&self) -> &[f32] {
        &self.levels
    }

    /// Samples pushed so far, the frame in progress included.
    pub(crate) fn samples(&self) -> usize {
        self.levels.len() * FRAME + self.partial
    }
}

/// The level of the loud frames: their 90th percentile (0 if there are none).
fn loud_level(levels: &[f32]) -> f32 {
    let mut sorted = levels.to_vec();
    sorted.sort_by(f32::total_cmp);
    sorted.get(sorted.len().saturating_sub(1) * 9 / 10).copied().unwrap_or(0.0)
}

/// Where quiet ends, relative to the loud frames around (linear levels, from [`Tuning`]'s
/// decibels).
#[derive(Clone, Copy, Debug)]
struct Quiet {
    /// Quiet: at most this fraction of the loud frames' level...
    ratio: f32,
    /// ...or at most this level, so a stretch of pure silence (no loud frames at all) counts.
    silence: f32,
}

impl Quiet {
    fn from_db(quiet_db: f32, silence_dbfs: f32) -> Self {
        Self { ratio: db_to_linear(quiet_db), silence: db_to_linear(silence_dbfs) }
    }

    /// Frames at or under this level are quiet, next to loud frames at `loud`.
    fn level(self, loud: f32) -> f32 {
        (loud * self.ratio).max(self.silence)
    }
}

/// Decides whether a preview pass is worth running: only when something was said since the
/// last check, plus `gate_settle_passes` more after the speech stops, so the last words are
/// decoded with the silence after them and confirmed by two passes that agree (`partials.rs`).
/// While you pause to think, the preview stops re-decoding, which saves most of the GPU and CPU
/// it would spend.
///
/// "Something was said" means a frame louder than the quiet level of the loudest speech so far
/// (the 90th percentile of a preview window), the same measure the pause finder uses, with its
/// own `gate_*` levels. So it errs towards decoding: before any speech, or in a noisy room,
/// room noise counts. It only affects the preview; the final text decodes everything either way.
pub(crate) struct SpeechGate {
    quiet: Quiet,
    settle_passes: u32,
    /// The loudest level of a preview window so far.
    loud: f32,
    /// Samples already checked for speech.
    checked: usize,
    /// Passes since speech was last heard. Starts full: nothing to decode before any speech.
    since_speech: u32,
}

impl SpeechGate {
    pub(crate) fn new(t: &Tuning) -> Self {
        Self::with(Quiet::from_db(t.gate_quiet_db, t.gate_silence_dbfs), t.gate_settle_passes)
    }

    fn with(quiet: Quiet, settle_passes: u32) -> Self {
        Self { quiet, settle_passes, loud: 0.0, checked: 0, since_speech: settle_passes }
    }

    /// Whether to re-decode now. `pcm` is the recording so far, and the pass would decode
    /// `pcm[window_start..]`.
    pub(crate) fn should_decode(&mut self, pcm: &[f32], window_start: usize) -> bool {
        self.loud = self.loud.max(loud_level(&frame_levels(&pcm[window_start.min(pcm.len())..])));
        let new = &pcm[self.checked.min(pcm.len())..];
        self.checked = pcm.len();
        let quiet = self.quiet.level(self.loud);
        if frame_levels(new).iter().any(|level| *level > quiet) {
            self.since_speech = 0;
            return true;
        }
        if self.since_speech < self.settle_passes {
            self.since_speech += 1;
            return true;
        }
        false
    }
}

/// Frames at or under this level (-100 dBFS) are digital silence, not the room: a Bluetooth
/// microphone can start with a stretch of zeros.
pub(crate) const DIGITAL_SILENCE: f32 = 1e-5;
/// The room's level counts once this many frames (0.3 s) of it were heard.
pub(crate) const MIN_ROOM_FRAMES: usize = 10;

/// Whether a recording holds speech, and where it starts (the `speech_*` fields of [`Tuning`]).
/// Until a recording holds speech the session decodes nothing, so silence or room noise gives
/// empty text, with no final pass, instead of words a model made up. The first preview pass
/// counts from where speech starts.
///
/// Speech is a run of `speech_min_ms` of frames louder than the room by `speech_above_noise_db`,
/// or louder than `speech_loud_dbfs` whatever the room, and never at or under
/// `gate_silence_dbfs`. The room is the recording's quietest frames, their 10th percentile,
/// leaving out digital silence; until 0.3 s of it was heard, only `speech_loud_dbfs` counts.
/// Every check reads the whole recording's [`FrameLevels`] again against the room measured so
/// far, so a guess made from the first few frames can't stick. The level meter shows loudness
/// against the same line ([`speech_level`](Self::speech_level)), so it rests flat on the room.
pub(crate) struct SpeechDetector {
    above_room: f32,
    loud: f32,
    silence: f32,
    /// Frames of speech needed in a row; 0 takes every recording as speech.
    min_frames: usize,
}

impl SpeechDetector {
    pub(crate) fn new(t: &Tuning) -> Self {
        Self {
            above_room: db_to_linear(t.speech_above_noise_db),
            loud: db_to_linear(t.speech_loud_dbfs),
            silence: db_to_linear(t.gate_silence_dbfs),
            min_frames: (t.speech_min_ms as usize).div_ceil(FRAME_MS),
        }
    }

    /// Where speech starts in a recording whose frames have these `levels` (a sample offset),
    /// or `None` if it holds none yet. Call it again as the recording grows.
    pub(crate) fn onset(&self, levels: &[f32]) -> Option<usize> {
        if self.min_frames == 0 {
            return Some(0);
        }
        let speech = self.speech_level(levels);
        let mut run = 0;
        for (i, level) in levels.iter().enumerate() {
            run = if *level > speech { run + 1 } else { 0 };
            if run == self.min_frames {
                return Some((i + 1 - run) * FRAME);
            }
        }
        None
    }

    /// Frames louder than this (a linear RMS level) are speech, in a recording whose frames have
    /// these `levels`.
    pub(crate) fn speech_level(&self, levels: &[f32]) -> f32 {
        let mut room: Vec<f32> = levels.iter().copied().filter(|level| *level > DIGITAL_SILENCE).collect();
        let above_room = if room.len() >= MIN_ROOM_FRAMES {
            let k = room.len() / 10;
            *room.select_nth_unstable_by(k, f32::total_cmp).1 * self.above_room
        } else {
            f32::INFINITY
        };
        above_room.min(self.loud).max(self.silence)
    }
}

/// Finds the pauses a long recording can be cut at (`pause_*` in [`Tuning`]).
pub(crate) struct PauseFinder {
    quiet: Quiet,
    /// Shortest pause worth cutting at, in frames (at least 1).
    min_frames: usize,
}

impl PauseFinder {
    pub(crate) fn new(t: &Tuning) -> Self {
        Self {
            quiet: Quiet::from_db(t.pause_quiet_db, t.pause_silence_dbfs),
            min_frames: (t.pause_min_ms as usize).div_ceil(FRAME_MS).max(1),
        }
    }

    /// The longest pause in `pcm` (a sample range, whole 30 ms frames), if one lasts at least
    /// `pause_min_ms`. The later one wins a tie.
    pub(crate) fn longest(&self, pcm: &[f32]) -> Option<Range<usize>> {
        let levels = frame_levels(pcm);
        if levels.is_empty() {
            return None;
        }
        let quiet = self.quiet.level(loud_level(&levels));
        let (mut best, mut run) = (None::<Range<usize>>, 0);
        for (i, level) in levels.iter().enumerate() {
            run = if *level <= quiet { run + 1 } else { 0 };
            if run >= self.min_frames && best.as_ref().is_none_or(|b| run >= b.len()) {
                best = Some(i + 1 - run..i + 1);
            }
        }
        best.map(|f| f.start * FRAME..(f.end * FRAME).min(pcm.len()))
    }
}

/// The middle of the quietest 30 ms frame in `pcm`: where to cut when nobody pauses.
pub(crate) fn quietest_point(pcm: &[f32]) -> usize {
    let levels = frame_levels(pcm);
    let frame = (0..levels.len()).min_by(|a, b| levels[*a].total_cmp(&levels[*b])).unwrap_or(0);
    (frame * FRAME + FRAME / 2).min(pcm.len())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    const SR: usize = SAMPLE_RATE as usize;

    fn longest_pause(pcm: &[f32]) -> Option<Range<usize>> {
        PauseFinder::new(&Tuning::default()).longest(pcm)
    }

    /// Test audio: `(seconds, amplitude)` stretches of a 220 Hz tone (amplitude 0 = silence).
    pub(crate) fn audio(spec: &[(f32, f32)]) -> Vec<f32> {
        spec.iter()
            .flat_map(|&(secs, amp)| {
                (0..(secs * SR as f32) as usize)
                    .map(move |i| amp * (i as f32 * 220.0 * std::f32::consts::TAU / SR as f32).sin())
            })
            .collect()
    }

    fn secs(r: Range<usize>) -> (f32, f32) {
        (r.start as f32 / SR as f32, r.end as f32 / SR as f32)
    }

    #[test]
    fn finds_the_longest_pause() {
        let pcm = audio(&[(2.0, 0.3), (0.3, 0.0), (2.0, 0.3), (0.6, 0.0), (2.0, 0.3), (0.2, 0.0), (1.0, 0.3)]);
        let (a, b) = secs(longest_pause(&pcm).unwrap());
        assert!((4.29..=4.35).contains(&a) && (4.86..=4.92).contains(&b), "{a}..{b}");
    }

    #[test]
    fn a_pause_over_room_noise_counts() {
        // Speech at -14 dBFS, pauses at -46 dBFS (above the -55 dBFS floor): 32 dB quieter.
        let pcm = audio(&[(3.0, 0.3), (0.4, 0.007), (3.0, 0.3)]);
        let (a, b) = secs(longest_pause(&pcm).unwrap());
        assert!((2.99..=3.04).contains(&a) && (3.36..=3.41).contains(&b), "{a}..{b}");
    }

    #[test]
    fn short_gaps_and_level_dips_are_not_pauses() {
        // A 90 ms gap between words, and a soft stretch only 12 dB under the loud parts.
        let pcm = audio(&[(2.0, 0.3), (0.09, 0.0), (2.0, 0.3), (1.0, 0.075), (2.0, 0.3)]);
        assert_eq!(longest_pause(&pcm), None);
        assert_eq!(longest_pause(&[]), None);
    }

    #[test]
    fn long_silence_is_a_pause_even_without_loud_frames() {
        let pcm = audio(&[(5.0, 0.0005)]);
        assert_eq!(longest_pause(&pcm).map(|r| r.len()), Some(pcm.len()));
    }

    /// Feeds `pcm` to a gate the way the session does: a check every 400 ms, over a window of
    /// the last 20 s. Returns the times (s) of the checks that decode.
    fn decoding_ticks(pcm: &[f32]) -> Vec<f32> {
        gate_ticks(SpeechGate::new(&Tuning::default()), pcm)
    }

    fn gate_ticks(mut gate: SpeechGate, pcm: &[f32]) -> Vec<f32> {
        (1..=pcm.len() / (SR * 4 / 10))
            .map(|i| i * SR * 4 / 10)
            .filter(|&end| gate.should_decode(&pcm[..end], end.saturating_sub(20 * SR)))
            .map(|end| end as f32 / SR as f32)
            .collect()
    }

    /// The loudness settings moved from linear levels (0.1 and 0.0018) to decibels (-20 dB and
    /// -55 dBFS, which is 0.00178). On every test clip, the speech gate and the pause finder
    /// make exactly the decisions they made with the old levels: the same preview passes, and
    /// the same longest pause in every stretch a chunk cut would search. Set `EASYVOX_CLIPS` to
    /// a space-separated list of WAV files (relative to the workspace) to check others too, e.g.
    /// the long-dictation clips: `EASYVOX_CLIPS="$(ls testdata/longform/clips/*.wav)"`.
    #[test]
    fn db_defaults_decide_like_the_old_levels() {
        let t = Tuning::default();
        let old_gate = Quiet { ratio: 0.1, silence: 0.0018 };
        let old_pause = PauseFinder { quiet: old_gate, min_frames: PauseFinder::new(&t).min_frames };
        let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
        let testdata = root.join("testdata");
        let mut clips: Vec<std::path::PathBuf> = std::fs::read_dir(&testdata)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "wav"))
            .collect();
        if let Some(extra) = std::env::var_os("EASYVOX_CLIPS") {
            clips.extend(extra.to_string_lossy().split_whitespace().map(|p| root.join(p)));
        }
        clips.sort();
        assert!(clips.len() >= 10, "expected the test clips in {}", testdata.display());
        let mut stretches = 0;
        for clip in &clips {
            let pcm = crate::audio::load_wav_16k_mono(clip).unwrap();
            let name = clip.display();
            let new_ticks = gate_ticks(SpeechGate::new(&t), &pcm);
            assert_eq!(new_ticks, gate_ticks(SpeechGate::with(old_gate, t.gate_settle_passes), &pcm), "{name}");
            // Every stretch of 2-60 s ending on a 0.5 s step, as the chunker searches them.
            let new_pause = PauseFinder::new(&t);
            for end in (SR / 2..=pcm.len()).step_by(SR / 2) {
                for len in [2, 5, 10, 30, 60] {
                    let stretch = &pcm[end.saturating_sub(len * SR)..end];
                    assert_eq!(new_pause.longest(stretch), old_pause.longest(stretch), "{name} at {end}");
                    stretches += 1;
                }
            }
        }
        assert!(stretches > 1000, "{stretches}");
    }

    #[test]
    fn the_gate_decodes_speech_and_settles_in_pauses() {
        // 1 s of silence, 2 s of speech, a 3 s pause over room noise (-46 dBFS), 1 s of speech.
        let pcm = audio(&[(1.0, 0.0), (2.0, 0.3), (3.0, 0.007), (1.0, 0.3)]);
        let ticks = decoding_ticks(&pcm);
        // Nothing before the speech; every check during it; two more into the pause; then the
        // speech that follows.
        assert!(ticks.first().is_some_and(|t| (1.15..1.25).contains(t)), "{ticks:?}");
        assert!(ticks.iter().any(|t| (3.2..3.25).contains(t)), "no pass just after the speech: {ticks:?}");
        let in_pause = ticks.iter().filter(|t| (3.25..6.0).contains(*t)).count();
        assert_eq!(in_pause, Tuning::default().gate_settle_passes as usize, "{ticks:?}");
        assert!(ticks.iter().any(|t| *t > 6.0), "the speech after the pause wasn't decoded: {ticks:?}");
    }

    #[test]
    fn the_gate_decodes_noise_until_it_has_heard_speech() {
        // Room noise alone above the -55 dBFS floor: it can't be told from quiet speech yet.
        let noise = audio(&[(2.0, 0.007)]);
        assert_eq!(decoding_ticks(&noise).len(), 5);
        // Digital silence never decodes.
        assert!(decoding_ticks(&audio(&[(2.0, 0.0)])).is_empty());
    }

    /// Feeds `pcm` to a speech detector every 100 ms, as the session does before the first
    /// preview pass (its levels measured as they arrive, in 20 ms pieces like a WAV replay), and
    /// returns the onset (seconds) each check found.
    fn onsets(t: &Tuning, pcm: &[f32]) -> Vec<Option<f32>> {
        let speech = SpeechDetector::new(t);
        let mut levels = FrameLevels::default();
        pcm.chunks(SR / 50)
            .enumerate()
            .filter_map(|(i, piece)| {
                levels.push(piece);
                ((i + 1) % 5 == 0).then(|| speech.onset(levels.levels()).map(|at| at as f32 / SR as f32))
            })
            .collect()
    }

    /// Where the detector finds speech once it has heard all of `pcm`, checking as it goes.
    fn onset(pcm: &[f32]) -> Option<f32> {
        onsets(&Tuning::default(), pcm).last().copied().flatten()
    }

    #[test]
    fn speech_starts_where_the_voice_does() {
        // Digital silence (a Bluetooth mic starting), room noise at -46 dBFS, then a voice.
        let pcm = audio(&[(0.5, 0.0), (0.5, 0.007), (1.0, 0.3)]);
        let at = onset(&pcm).unwrap();
        assert!((0.97..=1.0).contains(&at), "{at}");
        // A voice from the first frame, with no quiet frames to measure the room by, counts at
        // once (it's louder than `speech_loud_dbfs`).
        let checks = onsets(&Tuning::default(), &audio(&[(1.0, 0.3)]));
        assert_eq!(checks[1], Some(0.0), "{checks:?}");
    }

    #[test]
    fn silence_room_noise_and_clicks_hold_no_speech() {
        assert_eq!(onset(&audio(&[(2.0, 0.0)])), None);
        // Room noise at -46 dBFS, even right after digital silence.
        assert_eq!(onset(&audio(&[(0.5, 0.0), (3.0, 0.007)])), None);
        // A 60 ms click in the room: too short to be speech.
        assert_eq!(onset(&audio(&[(1.0, 0.007), (0.06, 0.3), (1.0, 0.007)])), None);
        // A short word is long enough.
        let at = onset(&audio(&[(1.0, 0.007), (0.15, 0.3), (1.0, 0.007)])).unwrap();
        assert!((0.97..=1.0).contains(&at), "{at}");
    }

    #[test]
    fn a_soft_voice_counts_against_a_quiet_room() {
        // A voice at -43 dBFS, under `speech_loud_dbfs`, but 30 dB over a room at -73 dBFS.
        let at = onset(&audio(&[(1.0, 0.0003), (0.5, 0.01)])).unwrap();
        assert!((0.97..=1.0).contains(&at), "{at}");
        // ...and 12 dB over a room at -55 dBFS, where a soft voice on a laptop's own mic may
        // sit. Over a room at -46 dBFS it's only 3 dB louder: not speech.
        let at = onset(&audio(&[(1.0, 0.0025), (0.5, 0.01)])).unwrap();
        assert!((0.97..=1.0).contains(&at), "{at}");
        assert_eq!(onset(&audio(&[(1.0, 0.007), (0.5, 0.01)])), None);
    }

    #[test]
    fn a_guess_from_the_first_frames_does_not_stick() {
        // Digital silence, one frame fading in (-87 dBFS), then room noise at -50 dBFS. Against
        // the faded frame alone, the room would pass for speech; every check reads it again.
        let mut pcm = audio(&[(0.15, 0.0)]);
        pcm.extend(std::iter::repeat_n(0.00006, FRAME));
        pcm.extend(audio(&[(2.0, 0.0045)]));
        assert!(onsets(&Tuning::default(), &pcm).iter().all(Option::is_none));
    }

    #[test]
    fn speech_min_ms_zero_takes_every_recording_as_speech() {
        let t = Tuning { speech_min_ms: 0, ..Tuning::default() };
        assert_eq!(onsets(&t, &audio(&[(0.5, 0.0)])).last(), Some(&Some(0.0)));
    }

    #[test]
    fn frame_levels_measured_as_audio_arrives_match_the_whole_recording() {
        let pcm = audio(&[(0.5, 0.0), (0.4, 0.007), (0.7, 0.3)]);
        let mut levels = FrameLevels::default();
        // Pieces that don't line up with frames, as a microphone delivers them.
        for piece in pcm.chunks(441) {
            levels.push(piece);
        }
        let whole = pcm.len() / FRAME * FRAME;
        assert_eq!(levels.levels(), frame_levels(&pcm[..whole]));
        assert_eq!(levels.samples(), pcm.len());
    }

    #[test]
    fn quietest_point_falls_in_the_dip() {
        let pcm = audio(&[(2.0, 0.3), (0.06, 0.02), (2.0, 0.3)]);
        let t = quietest_point(&pcm) as f32 / SR as f32;
        assert!((2.0..=2.06).contains(&t), "{t}");
        assert_eq!(quietest_point(&[]), 0);
    }
}
