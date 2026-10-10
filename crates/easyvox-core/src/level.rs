//! The input level meter: how loud the microphone is now, for the overlay's level indicator
//! ([`TranscriptEvent::Level`](crate::TranscriptEvent::Level)), and whether audio reaches
//! EasyVox at all, for its "can't hear you" state
//! ([`TranscriptEvent::NoInput`](crate::TranscriptEvent::NoInput)).
//!
//! Light by design (`DESIGN.md`, "Overlay animation"):
//! - It doesn't measure the audio itself. It reads the 30 ms frame levels the capture thread
//!   measures once as audio arrives, the ones speech detection reads (`pause::FrameLevels`).
//! - It shows them against the speech detector's own line (`pause::SpeechDetector::speech_level`):
//!   the meter rests flat on the room and rises for what the detector counts as speech.
//! - It speaks only when what it would show changes visibly (`meter_step_db`), at most every
//!   `meter_interval_ms`, so a silent room sends nothing and the overlay doesn't redraw.
//! - It runs only while a session captures (`session.rs` gives it a thread), never at idle.
//!
//! [`LevelMeter`] is the logic, with time passed in, so it can be tested without a clock.

use crate::pause::{DIGITAL_SILENCE, MIN_ROOM_FRAMES, SpeechDetector};
use crate::tuning::Tuning;
use std::time::{Duration, Instant};

/// What one tick of the meter has to say. Both `None`: nothing visible changed.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Reading {
    /// The level to show now, 0 (flat) to 1 (full).
    pub level: Option<f32>,
    /// Whether EasyVox can't hear the input, when that changed.
    pub no_input: Option<bool>,
}

pub(crate) struct LevelMeter {
    detector: SpeechDetector,
    /// `meter_range_db`, `meter_release_ms`.
    range_db: f32,
    release_ms: f32,
    /// `meter_step_db`, as a share of the meter (0..1).
    step: f32,
    no_input_after: Duration,
    /// The speech detector's line (a linear RMS level), and how many frames it was measured on.
    line: f32,
    line_frames: usize,
    /// The level the meter is at, and the one last sent.
    shown: f32,
    sent: f32,
    /// Frames and samples already seen.
    frames: usize,
    samples: usize,
    /// Frames with sound in them (louder than digital silence), so far.
    sound_frames: usize,
    last_tick: Instant,
    /// When audio (any samples), and sound, last arrived.
    last_audio: Instant,
    last_sound: Instant,
    no_input: bool,
}

impl LevelMeter {
    /// A meter for a session that starts capturing at `now`.
    pub(crate) fn new(t: &Tuning, now: Instant) -> Self {
        let range_db = t.meter_range_db.max(1.0);
        Self {
            detector: SpeechDetector::new(t),
            range_db,
            release_ms: t.meter_release_ms as f32,
            step: (t.meter_step_db / range_db).clamp(0.0, 1.0),
            no_input_after: Duration::from_millis(t.meter_no_input_ms as u64),
            line: f32::INFINITY,
            line_frames: 0,
            shown: 0.0,
            sent: 0.0,
            frames: 0,
            samples: 0,
            sound_frames: 0,
            last_tick: now,
            last_audio: now,
            last_sound: now,
            no_input: false,
        }
    }

    /// One tick at `now`. `levels` holds the level of every whole frame of the recording so far,
    /// and `samples` how many samples have arrived (`FrameLevels`).
    pub(crate) fn tick(&mut self, now: Instant, levels: &[f32], samples: usize) -> Reading {
        let elapsed = now.saturating_duration_since(self.last_tick);
        self.last_tick = now;
        let new = &levels[self.frames.min(levels.len())..];
        self.frames = levels.len();
        if samples > self.samples {
            self.samples = samples;
            self.last_audio = now;
        }
        let sound = new.iter().filter(|level| **level > DIGITAL_SILENCE).count();
        if sound > 0 {
            self.sound_frames += sound;
            self.last_sound = now;
        }

        // Can't hear you: no audio for a while, or, until the input has delivered some sound
        // (0.3 s, what the speech detector needs to measure the room), only digital silence. A
        // muted or blocked input sends zeros; once real sound came, zeros are a pause (some
        // microphones gate their noise to exact zeros).
        let live = self.sound_frames >= MIN_ROOM_FRAMES;
        let heard = if live { self.last_audio } else { self.last_sound };
        let no_input = now.saturating_duration_since(heard) >= self.no_input_after;
        let no_input = (no_input != self.no_input).then(|| {
            self.no_input = no_input;
            no_input
        });

        // The line moves as the room is measured; measuring it reads every frame, so it's done
        // again only every 0.3 s of audio.
        if self.line_frames == 0 || levels.len() >= self.line_frames + MIN_ROOM_FRAMES {
            self.line = self.detector.speech_level(levels);
            self.line_frames = levels.len().max(1);
        }
        let loudest = new.iter().copied().fold(0.0, f32::max);
        let target =
            if loudest > self.line { (20.0 * (loudest / self.line).log10() / self.range_db).min(1.0) } else { 0.0 };
        // Up at once, down smoothly; a meter that has nearly settled snaps to where it's going.
        self.shown = if target >= self.shown {
            target
        } else {
            let keep = (-(elapsed.as_secs_f32() * 1e3) / self.release_ms.max(f32::MIN_POSITIVE)).exp();
            target + (self.shown - target) * keep
        };
        if self.shown - target < self.step / 2.0 {
            self.shown = target;
        }
        let moved = (self.shown - self.sent).abs();
        let at_an_end = self.shown != self.sent && (self.shown == 0.0 || self.shown == 1.0);
        let level = (moved > 0.0 && (moved >= self.step || at_an_end)).then(|| {
            self.sent = self.shown;
            self.shown
        });
        Reading { level, no_input }
    }

    /// The level last sent: where the overlay's meter is.
    pub(crate) fn sent(&self) -> f32 {
        self.sent
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::SAMPLE_RATE;
    use crate::pause::tests::audio;
    use crate::pause::{FRAME, FrameLevels};

    const TICK: Duration = Duration::from_millis(50);

    /// Feeds `pcm` to a meter as a real-time capture would (20 ms pieces), ticking every 50 ms,
    /// and returns what each tick said, with its time in seconds.
    fn run(t: &Tuning, pcm: &[f32]) -> Vec<(f32, Reading)> {
        let start = Instant::now();
        let mut meter = LevelMeter::new(t, start);
        let mut levels = FrameLevels::default();
        let per_tick = SAMPLE_RATE as usize / 20;
        let mut out = vec![];
        for (i, piece) in pcm.chunks(per_tick).enumerate() {
            levels.push(piece);
            let at = start + TICK * (i as u32 + 1);
            out.push((at.duration_since(start).as_secs_f32(), meter.tick(at, levels.levels(), levels.samples())));
        }
        out
    }

    fn levels(readings: &[(f32, Reading)]) -> Vec<(f32, f32)> {
        readings.iter().filter_map(|(at, r)| r.level.map(|l| (*at, l))).collect()
    }

    /// `secs` of a voice whose loudness swings by 20 dB four times a second, as syllables do
    /// (a 220 Hz tone from -13 to -33 dBFS).
    pub(crate) fn syllables(secs: f32) -> Vec<f32> {
        let sr = SAMPLE_RATE as f32;
        (0..(secs * sr) as usize)
            .map(|i| {
                let t = i as f32 / sr;
                let db = -20.0 * (1.0 + (std::f32::consts::TAU * 4.0 * t).sin()) / 2.0;
                0.3 * crate::tuning::db_to_linear(db) * (std::f32::consts::TAU * 220.0 * t).sin()
            })
            .collect()
    }

    /// Like [`run`], with a tick every `tick` ms.
    fn run_every(t: &Tuning, pcm: &[f32], tick: u32) -> Vec<(f32, Reading)> {
        let start = Instant::now();
        let mut meter = LevelMeter::new(t, start);
        let mut levels = FrameLevels::default();
        let per_tick = SAMPLE_RATE as usize * tick as usize / 1000;
        let mut out = vec![];
        for (i, piece) in pcm.chunks(per_tick).enumerate() {
            levels.push(piece);
            let at = start + Duration::from_millis(tick as u64 * (i as u64 + 1));
            out.push((at.duration_since(start).as_secs_f32(), meter.tick(at, levels.levels(), levels.samples())));
        }
        out
    }

    #[test]
    fn syllables_move_it_most_ticks() {
        // After half a second of a quiet room, 3 s of syllables: at about 15 and 30 ticks a
        // second, half the ticks or more show a visible change, and the meter swings visibly.
        let mut pcm = audio(&[(0.5, 0.003)]);
        pcm.extend(syllables(3.0));
        for tick in [Tuning::default().meter_interval_ms, 33] {
            let sent = levels(&run_every(&Tuning::default(), &pcm, tick));
            let speaking: Vec<f32> = sent.iter().filter(|(at, _)| *at > 0.6).map(|(_, l)| *l).collect();
            let ticks = 2.9e3 / tick as f32;
            assert!(speaking.len() as f32 >= ticks * 0.5, "{tick} ms: {} of {ticks}: {sent:?}", speaking.len());
            let (low, high) = speaking.iter().fold((1f32, 0f32), |(lo, hi), l| (lo.min(*l), hi.max(*l)));
            assert!(high > 0.9 && low < 0.75, "{tick} ms: {low}..{high}");
        }
    }

    #[test]
    fn silence_and_room_noise_send_nothing() {
        let t = Tuning::default();
        // Room noise at -46 dBFS, after a Bluetooth mic's first zeros.
        for pcm in [audio(&[(2.0, 0.0)]), audio(&[(0.3, 0.0), (3.0, 0.007)])] {
            assert_eq!(levels(&run(&t, &pcm)), vec![]);
        }
    }

    #[test]
    fn a_voice_moves_the_meter_and_it_falls_back_after() {
        let t = Tuning::default();
        // Room noise, a voice at -14 dBFS, then the room again.
        let readings = run(&t, &audio(&[(1.0, 0.007), (1.0, 0.3), (1.0, 0.007)]));
        let sent = levels(&readings);
        let peak = sent.iter().map(|(_, l)| *l).fold(0.0, f32::max);
        assert!(peak > 0.5, "{sent:?}");
        // It rises within a tick of the voice starting...
        let rise = sent.iter().find(|(_, l)| *l > 0.5).unwrap().0;
        assert!((1.0..=1.1).contains(&rise), "{sent:?}");
        // ...and rests flat again about half a second after it stops, sending nothing more.
        let (rest_at, rest) = *sent.last().unwrap();
        assert_eq!(rest, 0.0);
        assert!((2.05..=2.6).contains(&rest_at), "{sent:?}");
        // Never faster than once a tick, and a steady tone sends nothing while it holds.
        let during = sent.iter().filter(|(at, _)| (1.2..2.0).contains(at)).count();
        assert_eq!(during, 0, "{sent:?}");
    }

    #[test]
    fn it_sends_only_visible_changes() {
        // A voice whose level moves by 1 dB every 50 ms, under the 2 dB step: about every other
        // tick; with a 0.5 dB step, every tick.
        let mut pcm = vec![];
        for i in 0..40 {
            let db = -30.0 + (i % 10) as f32;
            pcm.extend(audio(&[(0.05, crate::tuning::db_to_linear(db) * std::f32::consts::SQRT_2)]));
        }
        let coarse = levels(&run(&Tuning::default(), &pcm)).len();
        let fine = levels(&run(&Tuning { meter_step_db: 0.5, ..Tuning::default() }, &pcm)).len();
        assert!(coarse < fine * 2 / 3 && fine >= 30, "coarse {coarse}, fine {fine}");
    }

    #[test]
    fn digital_silence_until_any_sound_is_cant_hear_you() {
        let t = Tuning::default();
        let said = |readings: &[(f32, Reading)]| -> Vec<(f32, bool)> {
            readings.iter().filter_map(|(at, r)| r.no_input.map(|n| (*at, n))).collect()
        };
        // A muted input: zeros from the start. Can't hear you after 1.5 s.
        let muted = said(&run(&t, &audio(&[(3.0, 0.0)])));
        assert_eq!(muted.len(), 1, "{muted:?}");
        assert!(muted[0].1 && (1.5..1.6).contains(&muted[0].0), "{muted:?}");
        // A Bluetooth headset's second of zeros, then the room: never.
        assert_eq!(said(&run(&t, &audio(&[(1.0, 0.0), (2.0, 0.007)]))), vec![]);
        // Zeros for 2 s, then sound: can't hear you, then heard again.
        let late = said(&run(&t, &audio(&[(2.0, 0.0), (1.0, 0.007)])));
        assert_eq!(late.iter().map(|(_, n)| *n).collect::<Vec<_>>(), [true, false], "{late:?}");
        assert!((2.0..2.1).contains(&late[1].0), "{late:?}");
        // After real sound, zeros are a pause (a noise gate), not a dead microphone.
        assert_eq!(said(&run(&t, &audio(&[(1.0, 0.007), (3.0, 0.0)]))), vec![]);
    }

    #[test]
    fn no_frames_at_all_is_cant_hear_you_even_after_sound() {
        let t = Tuning::default();
        let start = Instant::now();
        let mut meter = LevelMeter::new(&t, start);
        let mut levels = FrameLevels::default();
        let pcm = audio(&[(1.0, 0.3)]);
        let mut said = vec![];
        // 1 s of voice, then the input stalls for 2 s (no samples at all), then comes back.
        for i in 0..80u32 {
            let at = start + TICK * (i + 1);
            if !(20..60).contains(&i) {
                let from = (i as usize % 20) * FRAME;
                levels.push(&pcm[from..from + SAMPLE_RATE as usize / 20]);
            }
            let r = meter.tick(at, levels.levels(), levels.samples());
            if let Some(n) = r.no_input {
                said.push((at.duration_since(start).as_secs_f32(), n));
            }
        }
        assert_eq!(said.iter().map(|(_, n)| *n).collect::<Vec<_>>(), [true, false], "{said:?}");
        assert!((2.5..2.6).contains(&said[0].0) && (3.0..3.1).contains(&said[1].0), "{said:?}");
        assert!(meter.sent() > 0.5, "it's back up with the voice");
    }

    #[test]
    fn meter_settings_change_what_it_sends() {
        let pcm = audio(&[(0.5, 0.007), (0.5, 0.3), (2.5, 0.007)]);
        // A slower release takes longer to rest.
        let rest = |t: &Tuning| levels(&run(t, &pcm)).last().unwrap().0;
        let slow = Tuning { meter_release_ms: 600, ..Tuning::default() };
        assert!(rest(&slow) > rest(&Tuning::default()) + 0.4);
        // A wider range leaves the same voice lower on the meter.
        let peak = |t: &Tuning| levels(&run(t, &pcm)).iter().map(|(_, l)| *l).fold(0.0, f32::max);
        assert!(peak(&Tuning { meter_range_db: 72.0, ..Tuning::default() }) < peak(&Tuning::default()) * 0.75);
    }
}
