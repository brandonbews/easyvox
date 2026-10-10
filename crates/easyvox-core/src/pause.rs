//! Pause detection: where to cut a long recording into chunks (see `chunks.rs`), and whether
//! anything was said since the last preview pass ([`SpeechGate`]).
//!
//! Energy-based for now: a pause is a run of quiet 30 ms frames, and a frame is quiet when its
//! level is well under the loud frames around it (their 90th percentile), or under a silence
//! level whatever the level around. By default that's 20 dB under, or under about -55 dBFS
//! (see [`Tuning`]'s `pause_*` and `gate_*` fields). It needs no model, but it can't tell a
//! breath or room noise from speech. A VAD (e.g. Silero) can replace it behind these functions.

use crate::SAMPLE_RATE;
use crate::tuning::Tuning;
use std::ops::Range;

/// Frame length, in ms: the resolution levels are measured at.
const FRAME_MS: usize = 30;
const FRAME: usize = SAMPLE_RATE as usize * FRAME_MS / 1000;

fn frame_levels(pcm: &[f32]) -> Vec<f32> {
    pcm.chunks(FRAME).map(|c| (c.iter().map(|x| x * x).sum::<f32>() / c.len() as f32).sqrt()).collect()
}

/// The level of the loud frames: their 90th percentile (0 if there are none).
fn loud_level(levels: &[f32]) -> f32 {
    let mut sorted = levels.to_vec();
    sorted.sort_by(f32::total_cmp);
    sorted.get(sorted.len().saturating_sub(1) * 9 / 10).copied().unwrap_or(0.0)
}

/// Where quiet ends, relative to the loud frames around.
#[derive(Clone, Copy, Debug)]
struct Quiet {
    /// Quiet: at most this fraction of the loud frames' level...
    ratio: f32,
    /// ...or at most this level, so a stretch of pure silence (no loud frames at all) counts.
    silence: f32,
}

impl Quiet {
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
        Self {
            quiet: Quiet { ratio: t.gate_quiet_ratio, silence: t.gate_silence_level },
            settle_passes: t.gate_settle_passes,
            loud: 0.0,
            checked: 0,
            since_speech: t.gate_settle_passes,
        }
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

/// Finds the pauses a long recording can be cut at (`pause_*` in [`Tuning`]).
pub(crate) struct PauseFinder {
    quiet: Quiet,
    /// Shortest pause worth cutting at, in frames (at least 1).
    min_frames: usize,
}

impl PauseFinder {
    pub(crate) fn new(t: &Tuning) -> Self {
        Self {
            quiet: Quiet { ratio: t.pause_quiet_ratio, silence: t.pause_silence_level },
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
        let mut gate = SpeechGate::new(&Tuning::default());
        (1..=pcm.len() / (SR * 4 / 10))
            .map(|i| i * SR * 4 / 10)
            .filter(|&end| gate.should_decode(&pcm[..end], end.saturating_sub(20 * SR)))
            .map(|end| end as f32 / SR as f32)
            .collect()
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

    #[test]
    fn quietest_point_falls_in_the_dip() {
        let pcm = audio(&[(2.0, 0.3), (0.06, 0.02), (2.0, 0.3)]);
        let t = quietest_point(&pcm) as f32 / SR as f32;
        assert!((2.0..=2.06).contains(&t), "{t}");
        assert_eq!(quietest_point(&[]), 0);
    }
}
