//! Live partials for offline models: re-decode the recent audio window every few hundred ms
//! and confirm words once two consecutive passes agree on them ("local agreement", LA-2,
//! as in Macháček et al. 2023, whisper_streaming).
//!
//! Words are aligned across passes by time: a word belongs to the not-yet-confirmed tail if
//! its midpoint lies after the end of the last confirmed word. Confirmed text only grows.

use crate::Word;

/// Lowercase alphanumerics only, so "Hello," and "hello" agree.
pub fn norm_word(w: &str) -> String {
    w.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect()
}

#[derive(Debug, Default)]
pub struct LocalAgreement {
    confirmed: Vec<String>,
    /// End time (seconds, absolute) of the last confirmed word.
    confirmed_until: f32,
    prev_tail: Vec<Word>,
    provisional: String,
}

impl LocalAgreement {
    pub fn new() -> Self {
        Self::default()
    }

    /// End (absolute seconds) of the last confirmed word; the session may move the decode
    /// window start up to here.
    pub fn confirmed_until(&self) -> f32 {
        self.confirmed_until
    }

    pub fn confirmed_text(&self) -> String {
        self.confirmed.join(" ")
    }

    pub fn provisional_text(&self) -> &str {
        &self.provisional
    }

    /// Feed one pass. `words` must carry absolute times (window offset already added).
    /// Returns `(confirmed, provisional)`.
    pub fn update(&mut self, words: &[Word]) -> (String, String) {
        let mut tail: Vec<Word> =
            words.iter().filter(|w| (w.start + w.end) / 2.0 > self.confirmed_until).cloned().collect();
        // Word boundaries jitter between passes (TDT frames are 80 ms), so a re-decoded copy
        // of the last confirmed word(s) can slip past the time filter. Drop an n-gram at the
        // start of the tail that repeats the end of the confirmed text (as whisper_streaming does).
        if tail.first().is_some_and(|w| w.start < self.confirmed_until + 0.3) {
            let max_n = 5.min(self.confirmed.len()).min(tail.len());
            for n in (1..=max_n).rev() {
                let conf = &self.confirmed[self.confirmed.len() - n..];
                if conf.iter().zip(&tail[..n]).all(|(c, t)| norm_word(c) == norm_word(&t.text)) {
                    tail.drain(..n);
                    break;
                }
            }
        }
        // Agreed prefix with the previous pass, never including the newest word of this
        // pass (it is often cut off mid-word at the window edge).
        let limit = tail.len().saturating_sub(1);
        let mut n = 0;
        while n < limit && n < self.prev_tail.len() && norm_word(&tail[n].text) == norm_word(&self.prev_tail[n].text) {
            n += 1;
        }
        for w in &tail[..n] {
            self.confirmed.push(w.text.clone());
            self.confirmed_until = self.confirmed_until.max(w.end);
        }
        self.prev_tail = tail[n..].to_vec();
        self.provisional = self.prev_tail.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ");
        (self.confirmed_text(), self.provisional.clone())
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
        let mut la = LocalAgreement::new();
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
        let mut la = LocalAgreement::new();
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
    fn disagreement_keeps_text_provisional() {
        let mut la = LocalAgreement::new();
        la.update(&ws(&[("I", 0.0, 0.2), ("scream", 0.3, 0.6), ("x", 0.7, 0.8)]));
        let (c, p) = la.update(&ws(&[("Ice", 0.0, 0.3), ("cream", 0.3, 0.6), ("is", 0.7, 0.8)]));
        assert_eq!(c, "");
        assert_eq!(p, "Ice cream is");
    }
}
