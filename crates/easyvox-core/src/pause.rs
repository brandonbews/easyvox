//! Pause detection, for cutting a long recording into chunks (see `chunks.rs`).
//!
//! Energy-based for now: a pause is a run of quiet 30 ms frames, and a frame is quiet when its
//! level is at least 20 dB under the loud frames around it (their 90th percentile), or under
//! -55 dBFS whatever the level around. It needs no model, but it can't tell a breath or room
//! noise from speech. A VAD (e.g. Silero) can replace it behind these two functions.

use crate::SAMPLE_RATE;
use std::ops::Range;

const FRAME: usize = SAMPLE_RATE as usize * 30 / 1000;
/// Shortest pause worth cutting at: 150 ms. Gaps between words are mostly shorter.
const MIN_PAUSE_FRAMES: usize = 5;
/// Quiet: under a tenth (-20 dB) of the loud frames' RMS...
const QUIET_RATIO: f32 = 0.1;
/// ...or under -55 dBFS, so a stretch of pure silence (no loud frames at all) still counts.
const SILENCE_RMS: f32 = 0.0018;

fn frame_levels(pcm: &[f32]) -> Vec<f32> {
    pcm.chunks(FRAME).map(|c| (c.iter().map(|x| x * x).sum::<f32>() / c.len() as f32).sqrt()).collect()
}

/// The longest pause in `pcm` (a sample range, whole 30 ms frames), if one lasts at least
/// 150 ms. The later one wins a tie.
pub(crate) fn longest_pause(pcm: &[f32]) -> Option<Range<usize>> {
    let levels = frame_levels(pcm);
    if levels.is_empty() {
        return None;
    }
    let mut sorted = levels.clone();
    sorted.sort_by(f32::total_cmp);
    let loud = sorted[(sorted.len() - 1) * 9 / 10];
    let quiet = (loud * QUIET_RATIO).max(SILENCE_RMS);
    let (mut best, mut run) = (None::<Range<usize>>, 0);
    for (i, level) in levels.iter().enumerate() {
        run = if *level <= quiet { run + 1 } else { 0 };
        if run >= MIN_PAUSE_FRAMES && best.as_ref().is_none_or(|b| run >= b.len()) {
            best = Some(i + 1 - run..i + 1);
        }
    }
    best.map(|f| f.start * FRAME..(f.end * FRAME).min(pcm.len()))
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

    #[test]
    fn quietest_point_falls_in_the_dip() {
        let pcm = audio(&[(2.0, 0.3), (0.06, 0.02), (2.0, 0.3)]);
        let t = quietest_point(&pcm) as f32 / SR as f32;
        assert!((2.0..=2.06).contains(&t), "{t}");
        assert_eq!(quietest_point(&[]), 0);
    }
}
