//! The eager final pass: polish a long recording chunk by chunk while the user is still talking.
//!
//! One offline pass over the whole recording gets slow as it grows (attention is quadratic:
//! 21.6 s after a 9-minute dictation). So once [`Chunker`] sees enough unpolished audio, the
//! session cuts off the oldest stretch at a pause and decodes it in `Final` mode while the user
//! keeps talking. On release only the rest is decoded, so the wait stays under half a second
//! at any length, and every word of the result still comes from a `Final` decode.
//!
//! - **Where to cut:** see [`Chunker::cut`]. Pauses come from `pause.rs` (energy-based).
//! - **Context:** when the model times each word, a chunk is decoded with [`CONTEXT_SECS`] of
//!   the audio around it ([`Polished::span`]), so the model punctuates across the cut as it
//!   would in one pass (without it, Parakeet Unified left a sentence without its full stop at
//!   about one cut in ten). A model with coarser timings (Whisper: per segment) gets none:
//!   dropping the context's words again needs word timings, and spread-out ones lost words.
//! - **Joining:** [`Polished::add`] keeps the words whose middle lies in the chunk, by their
//!   timings. A cut sits in the middle of a pause, so words rarely come near it; when one does
//!   (a late timing, a forced cut), the rules in [`Polished::continuation`] keep it once.

use crate::partials::norm_word;
use crate::{SAMPLE_RATE, Word, pause};
use std::ops::Range;

const SR: usize = SAMPLE_RATE as usize;

/// Audio decoded on each side of a chunk, for context, when the model times each word. Its
/// words belong to the neighbours. 1 s and 2 s gave the same accuracy on the long clips.
pub(crate) const CONTEXT_SECS: f32 = 1.0;
/// The newest audio is never cut: the speaker may be mid-word.
const GUARD: usize = SR / 2;
/// How far before the chunk a word's middle may lie and still be kept, when the previous chunk
/// didn't have it (word timings in two decodes can disagree by a frame or two).
const SLACK_SECS: f32 = 0.3;
/// Longest run of words at the start of a chunk checked against the end of the polished text.
const MAX_REPEAT: usize = 4;

/// Decides when and where to cut the unpolished audio.
pub(crate) struct Chunker {
    target: usize,
}

impl Chunker {
    /// `target_secs` of unpolished audio before cutting; 0 never cuts (one pass on release).
    pub(crate) fn new(target_secs: f32) -> Self {
        Self { target: (target_secs.max(0.0) * SR as f32) as usize }
    }

    /// Where to cut `pcm`, the audio not polished yet (an offset into it), or `None` to wait.
    ///
    /// Nothing happens until `target` samples are waiting. Then the cut goes in the middle of
    /// the longest pause between `target / 2` and the last 0.5 s, so chunks are 0.5–2× the
    /// target. Without a pause, it waits for one until `2 × target` are waiting, then cuts at
    /// the quietest point.
    pub(crate) fn cut(&self, pcm: &[f32]) -> Option<usize> {
        if self.target == 0 || pcm.len() < self.target {
            return None;
        }
        let range = self.target / 2..pcm.len().saturating_sub(GUARD);
        if range.is_empty() {
            return None;
        }
        let search = &pcm[range.clone()];
        match pause::longest_pause(search) {
            Some(p) => Some(range.start + (p.start + p.end) / 2),
            None if pcm.len() >= 2 * self.target => Some(range.start + pause::quietest_point(search)),
            None => None,
        }
    }
}

/// The polished (final-quality) words so far, with absolute timings.
#[derive(Debug, Default)]
pub(crate) struct Polished {
    words: Vec<Word>,
    /// First sample not polished yet: the last cut.
    start: usize,
    /// Samples of context decoded on each side of a chunk.
    context: usize,
}

impl Polished {
    /// `word_timings`: the model times each word, so chunks can be decoded with context.
    pub(crate) fn new(word_timings: bool) -> Self {
        let context = if word_timings { (CONTEXT_SECS * SR as f32) as usize } else { 0 };
        Self { context, ..Self::default() }
    }

    /// First sample not polished yet.
    pub(crate) fn start(&self) -> usize {
        self.start
    }

    pub(crate) fn text(&self) -> String {
        join(&self.words)
    }

    /// End of the last polished word, in seconds (0 if none yet).
    pub(crate) fn until(&self) -> f32 {
        self.words.last().map_or(0.0, |w| w.end)
    }

    /// Samples of context decoded on each side of a chunk.
    pub(crate) fn context(&self) -> usize {
        self.context
    }

    /// What to decode to polish up to `end` (exclusive): the chunk plus any context on both
    /// sides, within the `available` samples.
    pub(crate) fn span(&self, end: usize, available: usize) -> Range<usize> {
        self.start.saturating_sub(self.context)..(end + self.context).min(available)
    }

    /// Adds the words (absolute timings) of a decode of `span` for the chunk ending at `end`
    /// (`None`: the last chunk, up to the end of the recording); the next chunk starts there.
    /// Where `span` reaches into the neighbours' audio, their words are dropped: the earlier
    /// side by [`continuation`](Self::continuation), the later side by middle (before `end`).
    /// A decode without context keeps every word, whatever its timing.
    pub(crate) fn add(&mut self, words: &[Word], span: Range<usize>, end: Option<usize>) {
        let words = if span.start < self.start { self.continuation(words) } else { words.to_vec() };
        let limit = match end {
            Some(end) if span.end > end => secs(end),
            _ => f32::INFINITY,
        };
        self.words.extend(words.into_iter().filter(|w| mid(w) < limit));
        if let Some(end) = end {
            self.start = end;
        }
    }

    /// The words of `words` (absolute timings, in order) that continue the polished text:
    ///
    /// - those whose middle lies in the unpolished audio, or just before it (by up to
    ///   [`SLACK_SECS`]) if they come after the last polished word, which catches a word on
    ///   the cut that the previous decode placed on the other side (Parakeet Unified stretches
    ///   a word with trailing punctuation over the pause after it, so its middle can land past
    ///   the cut);
    /// - [`without_repeat`](Self::without_repeat).
    pub(crate) fn continuation(&self, words: &[Word]) -> Vec<Word> {
        let after = self.words.last().map_or(f32::NEG_INFINITY, |w| w.end);
        let from = secs(self.start) - SLACK_SECS;
        let new = words.iter().filter(|w| mid(w) >= secs(self.start) || (mid(w) >= from && mid(w) > after));
        self.without_repeat(new.cloned().collect())
    }

    /// `new` minus a leading copy of the last polished words: same text, and either before the
    /// cut (audio the previous decode already covered) or overlapping them in time (the same
    /// speech, placed on both sides of the cut by two decodes). A word said again after the
    /// cut ("very, very") does neither, so it stays.
    pub(crate) fn without_repeat(&self, mut new: Vec<Word>) -> Vec<Word> {
        let max = MAX_REPEAT.min(self.words.len()).min(new.len());
        for n in (1..=max).rev() {
            let old = &self.words[self.words.len() - n..];
            let same = old.iter().zip(&new).all(|(a, b)| norm_word(&a.text) == norm_word(&b.text));
            if same && (mid(&new[0]) < secs(self.start) || new[0].start < old[0].end) {
                new.drain(..n);
                break;
            }
        }
        new
    }
}

fn secs(sample: usize) -> f32 {
    sample as f32 / SR as f32
}

fn mid(w: &Word) -> f32 {
    (w.start + w.end) / 2.0
}

/// Words as text, one space apart.
pub(crate) fn join(words: &[Word]) -> String {
    words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pause::tests::audio;

    fn ws(spec: &[(&str, f32, f32)]) -> Vec<Word> {
        spec.iter().map(|(t, s, e)| Word { text: t.to_string(), start: *s, end: *e }).collect()
    }

    fn at(secs: f32) -> usize {
        (secs * SR as f32) as usize
    }

    #[test]
    fn waits_for_the_target_then_cuts_in_the_longest_pause() {
        let c = Chunker::new(10.0);
        // 9.9 s with a pause: too early.
        assert_eq!(c.cut(&audio(&[(4.0, 0.3), (0.5, 0.0), (5.4, 0.3)])), None);
        // Pauses at 3 s (before target/2, ignored), 6 s (0.3 s) and 8 s (0.6 s, the longest).
        let pcm = audio(&[(3.0, 0.3), (0.8, 0.0), (2.2, 0.3), (0.3, 0.0), (1.7, 0.3), (0.6, 0.0), (2.0, 0.3)]);
        let cut = c.cut(&pcm).unwrap() as f32 / SR as f32;
        assert!((8.25..=8.35).contains(&cut), "{cut}");
    }

    #[test]
    fn without_a_pause_waits_until_twice_the_target_then_cuts_at_the_quietest_point() {
        let c = Chunker::new(10.0);
        let mut pcm = audio(&[(12.0, 0.3), (0.06, 0.05), (7.0, 0.3)]);
        assert_eq!(c.cut(&pcm), None);
        pcm.extend(audio(&[(1.0, 0.3)]));
        let cut = c.cut(&pcm).unwrap() as f32 / SR as f32;
        assert!((12.0..=12.06).contains(&cut), "{cut}");
    }

    #[test]
    fn never_cuts_the_newest_half_second_or_with_target_zero() {
        // The only pause is in the last 0.5 s.
        let pcm = audio(&[(10.6, 0.3), (0.4, 0.0)]);
        assert_eq!(Chunker::new(10.0).cut(&pcm), None);
        assert_eq!(Chunker::new(0.0).cut(&audio(&[(5.0, 0.3), (1.0, 0.0), (5.0, 0.3)])), None);
    }

    /// Polishes the chunk up to `end` (seconds; `None`: the rest of `secs` of audio) with the
    /// words a decode of its span returned.
    fn polish(p: &mut Polished, words: &[Word], end: Option<f32>, secs: f32) {
        let span = p.span(end.map_or(at(secs), at), at(secs));
        p.add(words, span, end.map(at));
    }

    #[test]
    fn span_adds_context_only_for_word_timings() {
        let mut p = Polished::new(true);
        assert_eq!(p.span(at(10.0), at(10.5)), 0..at(10.5));
        p.add(&[], 0..at(10.5), Some(at(10.0)));
        assert_eq!(p.span(at(40.0), at(60.0)), at(9.0)..at(41.0));
        let mut p = Polished::new(false);
        p.add(&[], 0..at(10.0), Some(at(10.0)));
        assert_eq!(p.span(at(40.0), at(60.0)), at(10.0)..at(40.0));
    }

    #[test]
    fn context_words_belong_to_the_neighbouring_chunks() {
        let mut p = Polished::new(true);
        // Chunk 0..5 s, decoded with 1 s of right context; the cut is in a pause at 5 s.
        let first = ws(&[("We", 0.5, 0.7), ("met", 0.8, 1.0), ("Tuesday.", 3.0, 4.6), ("Then", 5.4, 5.7)]);
        polish(&mut p, &first, Some(5.0), 20.0);
        assert_eq!(p.text(), "We met Tuesday.");
        // The rest, decoded from 4 s: its copy of "Tuesday." is context.
        polish(
            &mut p,
            &ws(&[("Tuesday.", 4.0, 4.6), ("Then", 5.4, 5.7), ("we", 5.8, 6.0), ("left.", 6.1, 6.5)]),
            None,
            7.0,
        );
        assert_eq!(p.text(), "We met Tuesday. Then we left.");
    }

    #[test]
    fn a_word_on_the_cut_is_kept_once_whichever_side_each_decode_puts_it() {
        // Left decode puts "kitchen" after the cut (5.0 s), right decode before it: kept on the right.
        let mut p = Polished::new(true);
        polish(&mut p, &ws(&[("the", 4.4, 4.6), ("kitchen", 4.7, 5.4)]), Some(5.0), 9.0);
        polish(&mut p, &ws(&[("the", 4.4, 4.6), ("kitchen", 4.6, 5.3), ("sink", 5.5, 5.8)]), None, 9.0);
        assert_eq!(p.text(), "the kitchen sink");
        // Left decode puts it before the cut, right decode after: the copy overlaps it, so it goes.
        let mut p = Polished::new(true);
        polish(&mut p, &ws(&[("the", 4.4, 4.6), ("kitchen", 4.6, 5.3)]), Some(5.0), 9.0);
        polish(&mut p, &ws(&[("kitchen", 4.75, 5.4), ("sink", 5.5, 5.8)]), None, 9.0);
        assert_eq!(p.text(), "the kitchen sink");
    }

    #[test]
    fn a_short_word_placed_a_little_later_by_the_next_decode_is_not_doubled() {
        // One-frame words: the two decodes' copies of "the" don't overlap, but the second lies
        // before the cut, in audio the first decode already covered.
        let mut p = Polished::new(true);
        polish(
            &mut p,
            &ws(&[("from", 13.99, 14.07), ("the", 14.15, 14.23), ("west,", 14.27, 14.95)]),
            Some(14.54),
            16.0,
        );
        assert_eq!(p.text(), "from the");
        polish(
            &mut p,
            &ws(&[("from", 14.03, 14.11), ("the", 14.27, 14.35), ("west,", 14.4, 14.9), ("he", 15.2, 15.3)]),
            None,
            16.0,
        );
        assert_eq!(p.text(), "from the west, he");
    }

    #[test]
    fn a_word_said_twice_across_the_cut_stays_twice() {
        let mut p = Polished::new(true);
        polish(&mut p, &ws(&[("very", 4.3, 4.6)]), Some(5.0), 9.0);
        polish(&mut p, &ws(&[("very", 4.3, 4.6), ("very", 5.2, 5.5), ("good", 5.6, 5.9)]), None, 9.0);
        assert_eq!(p.text(), "very very good");
    }

    #[test]
    fn noise_deep_in_the_pause_before_the_cut_is_dropped() {
        let mut p = Polished::new(true);
        polish(&mut p, &ws(&[("Done.", 3.0, 3.4)]), Some(5.0), 9.0);
        // A made-up "you" in the silence at 4.4 s: before the chunk, beyond the slack.
        polish(&mut p, &ws(&[("Done.", 4.0, 4.1), ("you", 4.3, 4.5), ("Next", 5.6, 5.9)]), None, 9.0);
        assert_eq!(p.text(), "Done. Next");
    }

    #[test]
    fn without_context_every_word_is_kept_whatever_its_timing() {
        // Segment-level timings spread over the words can stray past the chunk's ends.
        let mut p = Polished::new(false);
        polish(&mut p, &ws(&[("one", 0.0, 2.0), ("two", 2.0, 4.0), ("three", 4.0, 6.0)]), Some(5.0), 9.0);
        polish(&mut p, &ws(&[("three", 4.6, 5.4), ("four", 5.4, 7.0)]), None, 9.0);
        assert_eq!(p.text(), "one two three three four");
    }

    #[test]
    fn empty_chunks_join_cleanly() {
        let mut p = Polished::new(true);
        polish(&mut p, &[], Some(30.0), 31.0);
        polish(&mut p, &ws(&[("Hello", 31.0, 31.4)]), Some(60.0), 61.0);
        polish(&mut p, &[], None, 62.0);
        assert_eq!(p.text(), "Hello");
        assert_eq!(p.start(), at(60.0));
    }
}
