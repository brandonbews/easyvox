//! Live partials for offline models: re-decode the newest audio every second or two and
//! confirm words once two consecutive passes agree on them ("local agreement", LA-2, as in
//! Macháček et al. 2023, whisper_streaming). Each pass starts just before the confirmed words,
//! so the session only re-decodes what may still change.
//!
//! Words are aligned across passes by time and text: a pass's new words start after its copy of
//! the confirmed words' end, found by text near where the timings put it (the first word whose
//! midpoint lies after the last confirmed word). Confirmed text only grows, until the session
//! polishes a chunk and hands it over with [`LocalAgreement::skip_to`].

use crate::Word;
use crate::tuning::{Tuning, ms_secs};

/// Lowercase alphanumerics only, so "Hello," and "hello" agree.
pub fn norm_word(w: &str) -> String {
    w.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect()
}

#[derive(Debug)]
pub struct LocalAgreement {
    confirmed: Vec<Word>,
    /// End time (seconds, absolute) of the last confirmed word.
    confirmed_until: f32,
    prev_tail: Vec<Word>,
    provisional: String,
    /// A new word starting within this many seconds of `confirmed_until` may be a re-decoded
    /// copy of the confirmed words (`agreement_repeat_ms`).
    repeat_secs: f32,
    /// Longest run of words checked for such a copy (`agreement_max_repeat_words`).
    max_repeat: usize,
    /// How far from the time boundary a pass's copy of the confirmed words is looked for, in
    /// words per word of the copy (`agreement_reach_words`).
    reach: usize,
}

impl Default for LocalAgreement {
    /// With the default [`Tuning`].
    fn default() -> Self {
        Self::new(&Tuning::default())
    }
}

impl LocalAgreement {
    pub fn new(t: &Tuning) -> Self {
        Self {
            confirmed: Vec::new(),
            confirmed_until: 0.0,
            prev_tail: Vec::new(),
            provisional: String::new(),
            repeat_secs: ms_secs(t.agreement_repeat_ms),
            max_repeat: t.agreement_max_repeat_words as usize,
            reach: t.agreement_reach_words as usize,
        }
    }

    /// End (absolute seconds) of the last confirmed word; the session may move the decode
    /// window start up to here.
    pub fn confirmed_until(&self) -> f32 {
        self.confirmed_until
    }

    pub fn confirmed_text(&self) -> String {
        self.confirmed.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ")
    }

    /// Confirmed words, with absolute times.
    pub fn confirmed_words(&self) -> &[Word] {
        &self.confirmed
    }

    /// The audio before `t` (absolute seconds) has been polished: forget the words there and
    /// only take words after it from now on.
    pub fn skip_to(&mut self, t: f32) {
        let after = |w: &Word| (w.start + w.end) / 2.0 >= t;
        self.confirmed.retain(after);
        self.prev_tail.retain(after);
        self.confirmed_until = self.confirmed_until.max(t);
        self.provisional = self.prev_tail.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ");
    }

    pub fn provisional_text(&self) -> &str {
        &self.provisional
    }

    /// Feed one pass. `words` must carry absolute times (window offset already added).
    /// Returns `(confirmed, provisional)`.
    pub fn update(&mut self, words: &[Word]) -> (String, String) {
        let tail = &words[self.new_words_start(words)..];
        // Agreed prefix with the previous pass, never including the newest word of this
        // pass (it is often cut off mid-word at the window edge).
        let limit = tail.len().saturating_sub(1);
        let mut n = 0;
        while n < limit && n < self.prev_tail.len() && norm_word(&tail[n].text) == norm_word(&self.prev_tail[n].text) {
            n += 1;
        }
        for w in &tail[..n] {
            self.confirmed.push(w.clone());
            self.confirmed_until = self.confirmed_until.max(w.end);
        }
        self.prev_tail = tail[n..].to_vec();
        self.provisional = self.prev_tail.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ");
        (self.confirmed_text(), self.provisional.clone())
    }

    /// Where the words a pass adds to the confirmed ones start, as an index into `words`.
    ///
    /// A pass starts a little before the confirmed words (`preview_context_ms`), so it opens by
    /// decoding their end again. By timing, its new words start at the first word whose middle
    /// lies after the last confirmed word. But timings move between passes: by a frame or two
    /// for Parakeet (TDT frames are 80 ms), and by a word or more for a model that only times
    /// segments, whose words are spread evenly over the pass (Whisper). Two passes that put
    /// that boundary at different words never agree, and a word put on the wrong side is never
    /// confirmed. So the boundary goes after the pass's copy of the confirmed words' end, found
    /// by text: the longest copy of the last words (up to `agreement_max_repeat_words`) that
    /// ends within `agreement_reach_words` words per word of the copy of where the timing puts
    /// the boundary, the closest one if there are several, or one starting right at it. A copy
    /// starting after the last confirmed word counts only within `agreement_repeat_ms` of it:
    /// later, it's a word said again ("very, very"). With no copy, timing alone decides.
    fn new_words_start(&self, words: &[Word]) -> usize {
        let by_time = words.iter().position(|w| (w.start + w.end) / 2.0 > self.confirmed_until).unwrap_or(words.len());
        let is_copy = |n: usize, end: usize| {
            let (copy, conf) = (&words[end - n..end], &self.confirmed[self.confirmed.len() - n..]);
            conf.iter().zip(copy).all(|(c, w)| norm_word(&c.text) == norm_word(&w.text))
                && (end - n < by_time || copy[0].start < self.confirmed_until + self.repeat_secs)
        };
        for n in (1..=self.max_repeat.min(self.confirmed.len())).rev() {
            let reach = n * self.reach;
            let ends = by_time.saturating_sub(reach).max(n)..=(by_time + reach.max(n)).min(words.len());
            let near = |&end: &usize| end.abs_diff(by_time) <= reach || end == by_time + n;
            if let Some(end) = ends.filter(near).filter(|&end| is_copy(n, end)).min_by_key(|end| end.abs_diff(by_time))
            {
                return end;
            }
        }
        by_time
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ws(spec: &[(&str, f32, f32)]) -> Vec<Word> {
        spec.iter().map(|(t, s, e)| Word { text: t.to_string(), start: *s, end: *e }).collect()
    }

    #[test]
    fn confirms_after_two_agreeing_passes_and_only_grows() {
        let mut la = LocalAgreement::default();
        let (c, p) = la.update(&ws(&[("Hello", 0.0, 0.4), ("wor", 0.5, 0.7)]));
        assert_eq!((c.as_str(), p.as_str()), ("", "Hello wor"));
        let (c, p) = la.update(&ws(&[("Hello,", 0.0, 0.4), ("world", 0.5, 0.9), ("how", 1.0, 1.2)]));
        assert_eq!((c.as_str(), p.as_str()), ("Hello,", "world how"));
        // Model rewrites already-confirmed "Hello" as "Yellow": ignored, confirmed text is stable.
        let (c, p) =
            la.update(&ws(&[("Yellow", 0.0, 0.42), ("world", 0.52, 0.9), ("how", 1.0, 1.2), ("are", 1.3, 1.5)]));
        assert_eq!((c.as_str(), p.as_str()), ("Hello, world how", "are"));
        assert!((la.confirmed_until() - 1.2).abs() < 1e-6);
    }

    #[test]
    fn jittered_copy_of_confirmed_word_is_not_duplicated() {
        let mut la = LocalAgreement::default();
        la.update(&ws(&[("ship", 0.0, 0.3), ("on", 0.3, 0.4), ("Friday", 0.5, 0.9)]));
        let (c, p) = la.update(&ws(&[("ship", 0.0, 0.3), ("on", 0.3, 0.4), ("Friday,", 0.5, 0.9), ("and", 1.0, 1.1)]));
        assert_eq!((c.as_str(), p.as_str()), ("ship on Friday,", "and"));
        // "Friday" re-decoded slightly later (mid 0.925 > 0.9) must not be confirmed twice.
        let (c, p) = la.update(&ws(&[
            ("ship", 0.0, 0.3),
            ("on", 0.3, 0.4),
            ("Friday", 0.85, 1.0),
            ("and", 1.0, 1.1),
            ("no", 1.2, 1.3),
        ]));
        assert_eq!((c.as_str(), p.as_str()), ("ship on Friday, and", "no"));
    }

    #[test]
    fn skip_to_hands_the_audio_before_a_cut_over() {
        let mut la = LocalAgreement::default();
        la.update(&ws(&[("one", 0.0, 0.3), ("two", 0.5, 0.8), ("three", 1.2, 1.5), ("four", 1.6, 1.9)]));
        la.update(&ws(&[
            ("one", 0.0, 0.3),
            ("two", 0.5, 0.8),
            ("three", 1.2, 1.5),
            ("four", 1.6, 1.9),
            ("x", 2.0, 2.1),
        ]));
        assert_eq!(la.confirmed_text(), "one two three four");
        // Polished up to 1.0 s (a pause): "one two" leave, the later words stay confirmed.
        la.skip_to(1.0);
        assert_eq!(la.confirmed_text(), "three four");
        assert_eq!(la.provisional_text(), "x");
        // A later skip past everything confirmed: words before it are never confirmed again.
        la.skip_to(3.0);
        assert_eq!(la.confirmed_text(), "");
        let (c, p) = la.update(&ws(&[("x", 2.0, 2.1), ("five", 3.2, 3.5), ("six", 3.6, 3.9)]));
        assert_eq!((c.as_str(), p.as_str()), ("", "five six"));
        assert!((la.confirmed_until() - 3.0).abs() < 1e-6);
    }

    /// "the big dog" confirmed, "ran to" provisional, as after two passes.
    fn dog_ran(t: &Tuning) -> LocalAgreement {
        let mut la = LocalAgreement::new(t);
        la.update(&ws(&[("the", 0.0, 0.5), ("big", 0.5, 1.0), ("dog", 1.0, 1.5), ("sat", 1.5, 2.0)]));
        la.update(&ws(&[("the", 0.0, 0.5), ("big", 0.5, 1.0), ("dog", 1.0, 1.5), ("ran", 1.5, 2.0), ("to", 2.0, 2.5)]));
        assert_eq!((la.confirmed_text().as_str(), la.provisional_text()), ("the big dog", "ran to"));
        la
    }

    #[test]
    fn new_words_start_after_the_copy_of_the_confirmed_words_wherever_the_timings_put_it() {
        // Whisper only times segments, so a pass's words are spread evenly over its audio, and
        // where the confirmed words end moves by a word or more from pass to pass. Here the next
        // pass (from 0.5 s) squeezes "ran" before the end of "dog" (1.5 s).
        let pass = ws(&[
            ("big", 0.5, 0.88),
            ("dog", 0.88, 1.26),
            ("ran", 1.26, 1.64),
            ("to", 1.64, 2.02),
            ("the", 2.02, 2.4),
            ("park", 2.4, 2.78),
        ]);
        let mut la = dog_ran(&Tuning::default());
        let (c, p) = la.update(&pass);
        assert_eq!((c.as_str(), p.as_str()), ("the big dog ran to", "the park"));
        // By timing alone, the pass's new words would start at "to": no agreement, and "ran"
        // would never be confirmed.
        let mut la = dog_ran(&Tuning { agreement_reach_words: 0, ..Tuning::default() });
        let (c, p) = la.update(&pass);
        assert_eq!((c.as_str(), p.as_str()), ("the big dog", "to the park"));
        // The other way: a pass that stretches "dog" well past its end, beyond the
        // `agreement_repeat_ms` a copy starting after it may have.
        let mut la = dog_ran(&Tuning::default());
        let (c, p) = la.update(&ws(&[
            ("big", 0.5, 1.9),
            ("dog", 1.9, 3.3),
            ("ran", 3.3, 4.7),
            ("to", 4.7, 6.1),
            ("x", 6.1, 7.5),
        ]));
        assert_eq!((c.as_str(), p.as_str()), ("the big dog ran to", "x"));
    }

    #[test]
    fn a_word_said_again_right_after_the_confirmed_words_stays() {
        let mut la = LocalAgreement::default();
        la.update(&ws(&[("it", 0.3, 0.5), ("was", 0.5, 0.8), ("very", 0.8, 1.2), ("ve", 1.25, 1.4)]));
        la.update(&ws(&[("it", 0.3, 0.5), ("was", 0.5, 0.8), ("very", 0.8, 1.2), ("very", 1.25, 1.6)]));
        assert_eq!(la.confirmed_text(), "it was very");
        // The pass decodes the confirmed "very" again, then the second one, 50 ms later.
        let (c, p) = la.update(&ws(&[
            ("it", 0.3, 0.5),
            ("was", 0.5, 0.8),
            ("very", 0.8, 1.2),
            ("very", 1.25, 1.6),
            ("good", 1.7, 2.0),
        ]));
        assert_eq!((c.as_str(), p.as_str()), ("it was very very", "good"));
    }

    #[test]
    fn disagreement_keeps_text_provisional() {
        let mut la = LocalAgreement::default();
        la.update(&ws(&[("I", 0.0, 0.2), ("scream", 0.3, 0.6), ("x", 0.7, 0.8)]));
        let (c, p) = la.update(&ws(&[("Ice", 0.0, 0.3), ("cream", 0.3, 0.6), ("is", 0.7, 0.8)]));
        assert_eq!(c, "");
        assert_eq!(p, "Ice cream is");
    }
}
