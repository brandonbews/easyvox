//! Every number that shapes how dictation feels or what it costs, in one place: how often the
//! preview re-decodes and how much, how long dictations are cut into chunks and joined again,
//! and what counts as a pause.
//!
//! The defaults here are the single source of truth: a settings file or a benchmark overrides
//! single fields and leaves the rest alone, so a default can change in a release. Every model
//! gets the same values (one pipeline, no per-model cases).
//!
//! Units, the same for every field:
//! - durations are whole milliseconds (`_ms`);
//! - loudness is a linear RMS level: a `_ratio` is a fraction of another level (0.1 is 20 dB
//!   under it), a `_level` a fraction of full scale (1.0 is the loudest a sample can be);
//! - counts are whole numbers.

use crate::SAMPLE_RATE;
use std::fmt;

const SR: usize = SAMPLE_RATE as usize;

/// Declares [`Tuning`] with each field's default and allowed range, so the struct, its
/// defaults, its key list and its checks can't drift apart.
macro_rules! tuning {
    (
        $(#[$meta:meta])*
        pub struct Tuning {
            $(
                $(#[doc = $doc:literal])*
                $field:ident: $ty:ty = $default:expr, $min:literal..=$max:literal;
            )*
        }
    ) => {
        $(#[$meta])*
        pub struct Tuning {
            $( $(#[doc = $doc])* pub $field: $ty, )*
        }

        impl Default for Tuning {
            fn default() -> Self {
                Self { $( $field: $default, )* }
            }
        }

        impl Tuning {
            /// Every field's name, as a settings file or `easyvox-bench --set` spells it.
            pub const KEYS: &'static [&'static str] = &[$( stringify!($field) ),*];

            /// Every field with its value, default, range and documentation, in declaration order.
            pub fn fields(&self) -> Vec<Field> {
                let default = Self::default();
                vec![$(
                    Field {
                        key: stringify!($field),
                        value: self.$field.to_string(),
                        default: default.$field.to_string(),
                        min: { let min: $ty = $min; min.to_string() },
                        max: { let max: $ty = $max; max.to_string() },
                        doc: concat!($( $doc, "\n" ),*),
                    },
                )*]
            }

            /// Sets the field named `key` from its text form (`"800"`, `"0.25"`). It doesn't check
            /// the range: call [`validate`](Self::validate) once every field is set.
            pub fn set(&mut self, key: &str, value: &str) -> Result<(), String> {
                match key {
                    $(
                        stringify!($field) => {
                            self.$field = value.trim().parse::<$ty>().map_err(|_| {
                                format!("{key}: can't read {value:?} as {}", stringify!($ty))
                            })?;
                            Ok(())
                        }
                    )*
                    _ => Err(format!("unknown tuning key {key:?}; the keys are: {}", Self::KEYS.join(", "))),
                }
            }

            /// Puts every field outside its range (or not a number) back to its default, leaving the
            /// others alone, and returns what it changed.
            pub fn validate(&mut self) -> Vec<OutOfRange> {
                let default = Self::default();
                let mut out = Vec::new();
                $(
                    let (min, max): ($ty, $ty) = ($min, $max);
                    if !(min <= self.$field && self.$field <= max) {
                        out.push(OutOfRange {
                            key: stringify!($field),
                            value: self.$field.to_string(),
                            min: min.to_string(),
                            max: max.to_string(),
                            default: default.$field.to_string(),
                        });
                        self.$field = default.$field;
                    }
                )*
                out
            }
        }
    };
}

tuning! {
    /// The pipeline's tuning: the live preview, the chunks polished while you talk, how they're
    /// joined, and how pauses are found. See the module docs for the units.
    ///
    /// Build it with `Tuning::default()` and change fields, or by name with [`Tuning::set`];
    /// [`Tuning::validate`] puts out-of-range values back to their defaults.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Tuning {
        /// How often the preview re-decodes the newest audio while the key is held. 0 turns the
        /// preview off (long dictations are still polished in chunks). Lower shows words sooner
        /// but wakes the GPU more often; higher is lighter, and the preview lags further behind
        /// and updates in bigger steps. At 1600, 95 % of words showed up within about 2 s of
        /// being said; past it the saving flattens while the lag keeps growing
        /// (`docs/model-bench.md`, "Lightness").
        preview_interval_ms: u32 = 1_600, 0..=10_000;
        /// The most recent audio one preview pass decodes, at most. A pass normally starts just
        /// before the confirmed words (`preview_context_ms`), so it's usually much shorter; this
        /// only caps it while passes keep disagreeing and nothing gets confirmed. Lower makes
        /// those passes cheaper, but words that fall out of the window before two passes
        /// agreed on them leave the preview until their chunk is polished. At least 2000.
        preview_window_ms: u32 = 20_000, 2_000..=120_000;
        /// Audio before the confirmed words (the ones two passes agreed on) that each preview
        /// pass decodes again, as context for the words after them. A pass starts there, or at
        /// the last chunk cut if that's later, so confirmed words aren't decoded over and over:
        /// the preview's GPU energy follows the audio it decodes. Higher gives the newest words
        /// more of their sentence but makes every pass longer (2000 took about 14 % more GPU
        /// energy than 1000, for much the same preview); lower is lighter. The final text
        /// doesn't depend on it. 120000 decodes everything since the last cut, much as the
        /// preview did before this setting existed.
        preview_context_ms: u32 = 1_000, 0..=120_000;
        /// Audio needed before the first preview pass. Lower can show the first word a moment
        /// sooner, but spends a pass on too little audio to decode.
        preview_min_audio_ms: u32 = 300, 0..=5_000;
        /// Rest between two preview passes when they can't keep up with `preview_interval_ms`
        /// (a pass, or a chunk polished in the same tick, took longer than the interval). Higher
        /// lets a busy GPU breathe; the preview then falls further behind.
        preview_min_gap_ms: u32 = 50, 0..=5_000;
        /// Local agreement: a pass's first new word that starts within this much of the last
        /// confirmed word's end is checked for being a re-decoded copy of the confirmed words
        /// (word timings move by a frame or two between passes). Higher catches more doubled
        /// words in the preview, but can drop a word that really was said twice ("very, very").
        agreement_repeat_ms: u32 = 300, 0..=2_000;
        /// Local agreement: the longest run of words checked for such a copy.
        agreement_max_repeat_words: u32 = 5, 0..=20;
        /// Speech gate: preview passes that still run after speech stops, so the last words are
        /// decoded with the silence after them and confirmed by two passes that agree. Then the
        /// preview rests until the next word. 0 rests at once, and the last words may stay
        /// provisional until release; higher spends more passes on every pause.
        gate_settle_passes: u32 = 2, 0..=1_000;
        /// Speech gate: new audio counts as speech (worth a preview pass) when a 30 ms frame of
        /// it is louder than this fraction of the loudest speech so far (0.1 is 20 dB under).
        /// Higher lets the preview rest in more pauses over room noise, but may skip soft words
        /// until a louder one comes; lower errs towards decoding. The final text decodes
        /// everything either way.
        gate_quiet_ratio: f32 = 0.1, 0.0..=1.0;
        /// Speech gate: ...and louder than this level (0.0018 is about -55 dBFS), so that before
        /// any speech only near-silence lets the preview rest. Higher ignores more background
        /// noise; lower counts fainter sounds as speech.
        gate_silence_level: f32 = 0.0018, 0.0..=1.0;
        /// Once this much audio waits unpolished, the oldest stretch is cut at a pause and
        /// decoded in final mode while you keep talking; release then only waits for the rest.
        /// Chunks come out 0.5 to 2 times this long: the cut goes in the longest pause in the
        /// newer half, or at the quietest moment once twice this much waits without a pause.
        /// The preview window also starts at the last cut. Lower makes release faster and
        /// preview passes cheaper, but cuts more often (each cut is a join that can go wrong,
        /// and its context is decoded twice). 0 never cuts: one pass over the whole recording
        /// on release, which grows fast (21.6 s after 9 minutes).
        chunk_ms: u32 = 30_000, 0..=600_000;
        /// Audio decoded on each side of a chunk for context, when the model times each word
        /// (Parakeet; Whisper gets none). The context's words belong to the neighbours. Without
        /// it, punctuation across a cut suffers (a sentence lost its full stop at about one cut
        /// in ten); 1 s and 2 s gave the same accuracy. Higher costs more decoding per chunk.
        chunk_context_ms: u32 = 1_000, 0..=5_000;
        /// The newest audio is never cut, since the speaker may be mid-word.
        chunk_guard_ms: u32 = 500, 0..=5_000;
        /// Joining chunks: a word whose middle lies up to this much before a chunk is still kept
        /// when the previous chunk's decode didn't hear it (two decodes' timings differ by a
        /// frame or two). Higher loses fewer words at joins, but may keep a word made up in the
        /// pause; lower the opposite.
        join_slack_ms: u32 = 300, 0..=2_000;
        /// Joining chunks: two decodes' copies of the same word around a cut count as one word
        /// when they lie at most this far apart. On real speech they differed by up to 0.46 s.
        /// Higher catches more doubled words; lower keeps more words said twice in a row.
        join_match_ms: u32 = 1_000, 0..=5_000;
        /// Preview only: the longest run of words at the start of the preview that is checked
        /// against the end of the polished text, so words aren't shown twice after a chunk.
        join_max_repeat_words: u32 = 4, 0..=20;
        /// Shortest quiet stretch a chunk can be cut at (rounded up to whole 30 ms frames). Gaps
        /// between words are mostly shorter. Higher cuts only in clear pauses, but more chunks
        /// end up cut at the quietest moment instead; lower may cut between words.
        pause_min_ms: u32 = 150, 30..=10_000;
        /// Pause finder: a 30 ms frame is quiet when its level is at most this fraction of the
        /// loud frames around it (their 90th percentile; 0.1 is 20 dB under). Higher still finds
        /// pauses over loud room noise, but may cut in a soft syllable; lower counts only clear
        /// silence.
        pause_quiet_ratio: f32 = 0.1, 0.0..=1.0;
        /// Pause finder: ...or at most this level (0.0018 is about -55 dBFS), so a stretch of
        /// pure silence counts even with no loud frames around it.
        pause_silence_level: f32 = 0.0018, 0.0..=1.0;
    }
}

/// One field of [`Tuning`], as text (see [`Tuning::fields`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub key: &'static str,
    pub value: String,
    pub default: String,
    /// The allowed range, inclusive.
    pub min: String,
    pub max: String,
    /// The field's doc comment, one line per source line.
    pub doc: &'static str,
}

/// A field [`Tuning::validate`] put back to its default.
#[derive(Clone, Debug, PartialEq)]
pub struct OutOfRange {
    pub key: &'static str,
    /// The rejected value.
    pub value: String,
    pub min: String,
    pub max: String,
    /// The value used instead.
    pub default: String,
}

impl fmt::Display for OutOfRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { key, value, min, max, default } = self;
        write!(f, "{key} = {value} is outside {min}..={max}, so it stays at its default, {default}")
    }
}

/// Milliseconds as 16 kHz samples (exact: 16 samples a millisecond).
pub(crate) fn ms_samples(ms: u32) -> usize {
    ms as usize * SR / 1000
}

/// Milliseconds as seconds.
pub(crate) fn ms_secs(ms: u32) -> f32 {
    ms as f32 / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The values the pipeline used before they moved here, in the units it used them in,
    /// except the preview's interval and context, which the lightness sweep set
    /// (`docs/model-bench.md`, "Lightness"; the interval was 400 ms).
    #[test]
    fn defaults_equal_the_old_constants() {
        let t = Tuning::default();
        // session.rs and EngineConfig
        assert_eq!((t.preview_interval_ms, t.preview_context_ms), (1_600, 1_000));
        assert_eq!(ms_samples(t.preview_window_ms), (20.0 * SR as f32) as usize);
        assert_eq!(ms_samples(t.preview_min_audio_ms), SR * 3 / 10);
        assert_eq!(t.preview_min_gap_ms, 50);
        assert_eq!(ms_samples(t.chunk_ms), (30.0 * SR as f32) as usize);
        // partials.rs
        assert_eq!(ms_secs(t.agreement_repeat_ms), 0.3);
        assert_eq!(t.agreement_max_repeat_words, 5);
        // pause.rs (SETTLE_PASSES, QUIET_RATIO, SILENCE_RMS, MIN_PAUSE_FRAMES of 30 ms)
        assert_eq!(t.gate_settle_passes, 2);
        assert_eq!((t.gate_quiet_ratio, t.pause_quiet_ratio), (0.1, 0.1));
        assert_eq!((t.gate_silence_level, t.pause_silence_level), (0.0018, 0.0018));
        assert_eq!((t.pause_min_ms as usize).div_ceil(30), 5);
        // chunks.rs (CONTEXT_SECS, GUARD, SLACK_SECS, MATCH_SECS, MAX_REPEAT)
        assert_eq!(ms_samples(t.chunk_context_ms), (1.0 * SR as f32) as usize);
        assert_eq!(ms_samples(t.chunk_guard_ms), SR / 2);
        assert_eq!(ms_secs(t.join_slack_ms), 0.3);
        assert_eq!(ms_secs(t.join_match_ms), 1.0);
        assert_eq!(t.join_max_repeat_words, 4);
    }

    #[test]
    fn validate_puts_only_the_out_of_range_fields_back() {
        assert!(Tuning::default().validate().is_empty(), "the defaults must be in range");
        let mut t = Tuning {
            preview_interval_ms: 60_000,
            preview_window_ms: 500,
            gate_quiet_ratio: f32::NAN,
            pause_silence_level: -0.5,
            chunk_ms: 15_000,
            ..Tuning::default()
        };
        let problems = t.validate();
        let keys: Vec<_> = problems.iter().map(|p| p.key).collect();
        assert_eq!(keys, ["preview_interval_ms", "preview_window_ms", "gate_quiet_ratio", "pause_silence_level"]);
        assert_eq!(t, Tuning { chunk_ms: 15_000, ..Tuning::default() });
        assert_eq!(
            problems[0].to_string(),
            "preview_interval_ms = 60000 is outside 0..=10000, so it stays at its default, 1600"
        );
        // Range ends are allowed.
        let mut t = Tuning { preview_interval_ms: 0, chunk_ms: 0, pause_quiet_ratio: 1.0, ..Tuning::default() };
        assert!(t.validate().is_empty());
    }

    #[test]
    fn set_reads_any_field_by_name_and_rejects_unknown_keys() {
        let mut t = Tuning::default();
        t.set("preview_interval_ms", "800").unwrap();
        t.set("pause_quiet_ratio", " 0.25 ").unwrap();
        assert_eq!(t, Tuning { preview_interval_ms: 800, pause_quiet_ratio: 0.25, ..Tuning::default() });
        let err = t.set("preview_interval", "800").unwrap_err();
        assert!(err.contains("unknown tuning key") && err.contains("preview_interval_ms"), "{err}");
        assert!(t.set("preview_interval_ms", "-1").is_err());
        assert!(t.set("preview_interval_ms", "1.5").is_err());
        assert!(t.set("gate_quiet_ratio", "loud").is_err());
        assert_eq!(t.preview_interval_ms, 800, "a failed set leaves the field alone");
    }

    #[test]
    fn fields_list_every_key_once_with_its_docs() {
        let t = Tuning { chunk_ms: 15_000, ..Tuning::default() };
        let fields = t.fields();
        let keys: Vec<_> = fields.iter().map(|f| f.key).collect();
        assert_eq!(keys, Tuning::KEYS);
        let mut unique = keys.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), keys.len());
        for f in &fields {
            assert!(!f.doc.trim().is_empty(), "{} has no docs", f.key);
            let mut copy = Tuning::default();
            copy.set(f.key, &f.default).unwrap();
            assert_eq!(copy, Tuning::default(), "{}'s default doesn't read back", f.key);
        }
        let chunk = fields.iter().find(|f| f.key == "chunk_ms").unwrap();
        assert_eq!((chunk.value.as_str(), chunk.default.as_str()), ("15000", "30000"));
        let level = fields.iter().find(|f| f.key == "gate_silence_level").unwrap();
        assert_eq!((level.value.as_str(), level.min.as_str(), level.max.as_str()), ("0.0018", "0", "1"));
    }
}
