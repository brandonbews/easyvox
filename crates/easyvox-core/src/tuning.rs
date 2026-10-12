//! Every number that shapes how dictation feels or what it costs, in one place: how often the
//! preview re-decodes and how much, how long dictations are cut into chunks and joined again,
//! what counts as speech or a pause, and how the input level meter moves.
//!
//! The defaults live here and nowhere else: the settings file (`[advanced]`) or a benchmark
//! overrides single fields and leaves the rest alone, so a default can change in a release.
//! Every model gets the same values (one pipeline, no per-model cases), and a session takes the
//! values in force when it starts ([`Engine::start_with`](crate::Engine::start_with)),
//! so a change applies from the next recording without reloading the model.
//!
//! Units, the same for every field, and named by the key's ending:
//! - durations are whole milliseconds (`_ms`);
//! - loudness is in decibels of RMS level: a `_db` is relative to another level (-20 is a
//!   tenth of it, 0 the same, 20 ten times it), a `_dbfs` relative to full scale (0 is the
//!   loudest a sample can be; -55 is about 0.0018). [`db_to_linear`] turns them into the
//!   levels the code compares;
//! - counts are whole numbers (`_words`, `_passes`).

use crate::SAMPLE_RATE;
use std::fmt;

const SR: usize = SAMPLE_RATE as usize;

/// Declares a struct of numeric settings with each field's default and allowed range, so the
/// struct, its defaults, its key list and its checks can't drift apart. [`Tuning`] is one; the
/// app declares its own (when the model is freed, how long the overlay stays up) the same way,
/// so the settings file treats them alike.
///
/// Generates `Default`, `KEYS`, `fields()`, `set(key, text)` and `validate()`.
#[macro_export]
macro_rules! tunables {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $(
                $(#[doc = $doc:literal])*
                $field:ident: $ty:ty = $default:expr, $min:literal..=$max:literal;
            )*
        }
    ) => {
        $(#[$meta])*
        $vis struct $name {
            $( $(#[doc = $doc])* pub $field: $ty, )*
        }

        impl Default for $name {
            fn default() -> Self {
                Self { $( $field: $default, )* }
            }
        }

        impl $name {
            /// Every field's name, as the settings file or `easyvox-bench --set` spells it.
            pub const KEYS: &'static [&'static str] = &[$( stringify!($field) ),*];

            /// Every field with its value, default, range and documentation, in declaration order.
            pub fn fields(&self) -> Vec<$crate::tuning::Field> {
                let default = Self::default();
                vec![$(
                    $crate::tuning::Field {
                        key: stringify!($field),
                        value: self.$field.to_string(),
                        default: default.$field.to_string(),
                        min: { let min: $ty = $min; min.to_string() },
                        max: { let max: $ty = $max; max.to_string() },
                        doc: concat!($( $doc, "\n" ),*),
                    },
                )*]
            }

            /// Sets the field named `key` from its text form (`"800"`, `"-20.5"`). It doesn't
            /// check the range: call [`validate`](Self::validate) once every field is set.
            pub fn set(&mut self, key: &str, value: &str) -> Result<(), String> {
                match key {
                    $(
                        stringify!($field) => {
                            self.$field = value.trim().parse::<$ty>().map_err(|_| {
                                format!("{key}: can't read {value:?} as {}", $crate::tuning::type_name(stringify!($ty)))
                            })?;
                            Ok(())
                        }
                    )*
                    _ => Err(format!("unknown key {key:?}; the keys are: {}", Self::KEYS.join(", "))),
                }
            }

            /// Puts every field outside its range (or not a number) back to its default, leaving the
            /// others alone, and returns what it changed.
            pub fn validate(&mut self) -> Vec<$crate::tuning::OutOfRange> {
                let default = Self::default();
                let mut out = Vec::new();
                $(
                    let (min, max): ($ty, $ty) = ($min, $max);
                    if !(min <= self.$field && self.$field <= max) {
                        out.push($crate::tuning::OutOfRange {
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

tunables! {
    /// The pipeline's tuning: the live preview, the chunks polished while you talk, how they're
    /// joined, how speech and pauses are found, and the input level meter. See the module docs
    /// for the units.
    ///
    /// Build it with `Tuning::default()` and change fields, or by name with [`Tuning::set`];
    /// [`Tuning::validate`] puts out-of-range values back to their defaults.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Tuning {
        /// How often the preview re-decodes the newest audio while the key is held. 0 turns the
        /// preview off (long dictations are still polished in chunks). Lower shows words sooner
        /// but wakes the GPU more often; higher is lighter, and the preview lags further behind
        /// and updates in bigger steps. At 1600, 95 % of words showed up within about 2 s of
        /// being said; past it the saving flattens while the lag keeps growing.
        preview_interval_ms: u32 = 1_600, 0..=10_000;
        /// When the first preview pass runs: once this much audio has followed the start of
        /// speech (see `speech_above_noise_db`). Until the preview shows words, the next passes
        /// come as often (at least 100 ms apart), then every `preview_interval_ms`. Lower shows
        /// the first words sooner, but a first pass with too little speech shows nothing and
        /// costs another; higher keeps you waiting for the first words. It's a pass or two per
        /// dictation, so it costs almost nothing.
        preview_first_ms: u32 = 500, 0..=10_000;
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
        /// more of their sentence but makes every pass longer (2000 costs about 14 % more GPU
        /// energy than 1000); lower is lighter. The final text doesn't depend on it. 120000
        /// decodes everything since the last cut.
        preview_context_ms: u32 = 1_000, 0..=120_000;
        /// Audio needed before the first preview pass, counted from the start of the recording
        /// (the pass also waits for `preview_first_ms` of speech). Lower can show the first word
        /// a moment sooner, but spends a pass on too little audio to decode.
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
        /// Local agreement: how far from where the word timings put the end of the confirmed
        /// words a pass's copy of them is looked for, in words per word of the copy. Timings
        /// move between passes, by a word or more for Whisper, whose words are spread over the
        /// pass. Higher finds copies further off, but a common word may match by chance; 0 looks
        /// only where the timings put it.
        agreement_reach_words: u32 = 2, 0..=10;
        /// Speech gate: preview passes that still run after speech stops, so the last words are
        /// decoded with the silence after them and confirmed by two passes that agree. Then the
        /// preview rests until the next word. 0 rests at once, and the last words may stay
        /// provisional until release; higher spends more passes on every pause.
        gate_settle_passes: u32 = 2, 0..=1_000;
        /// Speech gate: new audio counts as speech (worth a preview pass) when a 30 ms frame of
        /// it is louder than this, relative to the loudest speech so far (-20 is 20 dB under:
        /// a tenth of its level). Higher (nearer 0) lets the preview rest in more pauses over
        /// room noise, but may skip soft words until a louder one comes; lower errs toward
        /// decoding. The final text decodes everything either way.
        gate_quiet_db: f32 = -20.0, -60.0..=0.0;
        /// Speech gate: ...and louder than this. Higher ignores more background noise; lower
        /// counts fainter sounds as speech. Nothing at or under it counts as speech for
        /// `speech_above_noise_db` either.
        gate_silence_dbfs: f32 = -55.0, -120.0..=0.0;
        /// Speech detection: a 30 ms frame counts as speech when it's this much louder than the
        /// room, which is the recording's quietest frames (their 10th percentile, once there's
        /// 0.3 s of sound; digital silence doesn't count). Until a recording holds speech,
        /// nothing is decoded: no preview, no final pass. So a recording of silence or room
        /// noise gives empty text, where a model could make up words (Whisper wrote "you" for
        /// 2 s of silence). Missing speech would lose the whole dictation, so this errs low: 6
        /// still catches a soft voice whose loudest moments are only 10 dB over the room, and
        /// a steady room stays under it. Higher ignores louder noises (a breath, a chair), but
        /// may miss a soft voice; lower takes fainter sounds for speech.
        speech_above_noise_db: f32 = 6.0, 0.0..=60.0;
        /// Speech detection: ...or louder than this, however loud the room. It catches speech
        /// that starts at once, before any quiet frames show how loud the room is, and speech
        /// in a loud room. Higher can hold the first words back until the room is measured;
        /// lower can take a loud room's noise for speech.
        speech_loud_dbfs: f32 = -35.0, -120.0..=0.0;
        /// Speech detection: frames like that must follow each other for this long (rounded up
        /// to whole 30 ms frames), so a click or a bump isn't speech. Higher ignores longer
        /// noises, but may miss a very short word. 0 treats every recording as speech from its
        /// start, and decodes it all.
        speech_min_ms: u32 = 120, 0..=10_000;
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
        /// (Parakeet; Whisper gets none). The context's words belong to the neighbors. Without
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
        /// Pause finder: a 30 ms frame is quiet when its level is at most this, relative to the
        /// loud frames around it (their 90th percentile; -20 is a tenth of their level). Higher
        /// (nearer 0) still finds pauses over loud room noise, but may cut in a soft syllable;
        /// lower counts only clear silence.
        pause_quiet_db: f32 = -20.0, -60.0..=0.0;
        /// Pause finder: ...or at most this, so a stretch of pure silence counts even with no
        /// loud frames around it.
        pause_silence_dbfs: f32 = -55.0, -120.0..=0.0;
        /// Level meter: how often the overlay's level indicator can move while you hold the
        /// key, at most (66 is about 15 times a second). It only moves when the level changed
        /// by `meter_step_db`, so in silence it doesn't redraw at all. On real speech it moved
        /// about 7 times a second, for 0.3 % of a CPU core; at 33 (30 a second) it moved 11
        /// times, for 0.43 %. Lower looks smoother but wakes the app and redraws the overlay more
        /// often; higher is lighter and jerkier. 0 turns the meter off, and with it the "can't
        /// hear you" state (for comparisons).
        meter_interval_ms: u32 = 66, 0..=1_000;
        /// Level meter: how fast it falls back after a loud moment (the time it takes to fall
        /// about two-thirds of the way). It rises at once. Lower follows each syllable more
        /// closely and flickers more; higher looks calmer but lags behind your pauses.
        meter_release_ms: u32 = 150, 0..=5_000;
        /// Level meter: how far above the speech detector's line (`speech_above_noise_db` over
        /// the room, or `speech_loud_dbfs`) a sound fills the meter. The meter rests flat on
        /// anything under that line, so room noise doesn't move it. Lower fills it sooner (a
        /// normal voice pins it at the top); higher leaves soft voices barely moving it.
        meter_range_db: f32 = 36.0, 6.0..=96.0;
        /// Level meter: the smallest change worth showing. Smaller changes aren't sent to the
        /// overlay, so it doesn't redraw for them. 2 moves the tallest bar by just under one
        /// point. Lower moves more finely, for more redraws; higher moves in coarser steps.
        meter_step_db: f32 = 2.0, 0.0..=20.0;
        /// "Can't hear you": the overlay says so once no audio has reached EasyVox for this
        /// long, which is not the same as quiet audio. That's when the microphone delivers
        /// nothing, or only digital silence (exact zeros, as a muted or blocked input gives)
        /// before it has delivered any sound; once it has, zeros count as quiet (some
        /// microphones send them in pauses). Lower warns sooner, but a Bluetooth headset, which
        /// can start with about a second of zeros, may flash it; higher lets you talk longer
        /// into a dead microphone.
        meter_no_input_ms: u32 = 1_500, 100..=60_000;
    }
}

/// One field of [`Tuning`] (or another [`tunables!`](crate::tunables) struct), as text (see
/// [`Tuning::fields`]).
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

impl Field {
    /// The unit, from the key's ending (`_ms`, `_db`, ...); empty for an unknown ending.
    pub fn unit(&self) -> &'static str {
        unit_of(self.key)
    }
}

/// The unit a key's ending names: `preview_interval_ms` is in milliseconds.
pub fn unit_of(key: &str) -> &'static str {
    const UNITS: &[(&str, &str)] = &[
        ("_ms", "milliseconds"),
        ("_secs", "seconds"),
        ("_dbfs", "dBFS (decibels relative to full scale)"),
        ("_db", "dB (relative to another level)"),
        ("_words", "words"),
        ("_passes", "passes"),
        ("_chars", "characters"),
        ("_pt", "points"),
        ("_pct", "percent"),
    ];
    UNITS.iter().find(|(end, _)| key.ends_with(end)).map_or("", |(_, unit)| unit)
}

/// What a field's Rust type means to someone writing a settings file, for error messages.
#[doc(hidden)]
pub fn type_name(ty: &str) -> &'static str {
    match ty {
        "u32" | "u64" | "usize" => "a whole number, 0 or more",
        "f32" | "f64" => "a number",
        _ => "this setting's type",
    }
}

/// Decibels to a linear level: 0 is 1.0, -20 is 0.1, -55 about 0.00178. For comparing RMS
/// levels against a `_db` or `_dbfs` field.
pub fn db_to_linear(db: f32) -> f32 {
    10f64.powf(db as f64 / 20.0) as f32
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

    /// The defaults, pinned in the units the code works in, so changing one takes a deliberate
    /// edit here too.
    #[test]
    fn defaults_equal_the_old_constants() {
        let t = Tuning::default();
        // session.rs
        assert_eq!((t.preview_interval_ms, t.preview_context_ms), (1_600, 1_000));
        assert_eq!(ms_samples(t.preview_window_ms), (20.0 * SR as f32) as usize);
        assert_eq!(ms_samples(t.preview_min_audio_ms), SR * 3 / 10);
        assert_eq!(t.preview_min_gap_ms, 50);
        assert_eq!(ms_samples(t.chunk_ms), (30.0 * SR as f32) as usize);
        // partials.rs
        assert_eq!(ms_secs(t.agreement_repeat_ms), 0.3);
        assert_eq!(t.agreement_max_repeat_words, 5);
        // pause.rs (pauses in whole 30 ms frames)
        assert_eq!(t.gate_settle_passes, 2);
        // -20 dB is exactly 0.1, and -55 dBFS (0.00178) is within 1.2 % of 0.0018, the linear
        // levels the pipeline was tuned with. On the test clips, the gate and the pause finder
        // decide the same with either (`pause::tests::db_defaults_decide_like_the_old_levels`).
        assert_eq!((db_to_linear(t.gate_quiet_db), db_to_linear(t.pause_quiet_db)), (0.1, 0.1));
        for level in [t.gate_silence_dbfs, t.pause_silence_dbfs] {
            assert!((db_to_linear(level) / 0.0018 - 1.0).abs() < 0.013, "{level} dBFS");
        }
        assert_eq!((t.pause_min_ms as usize).div_ceil(30), 5);
        // chunks.rs
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
            gate_quiet_db: f32::NAN,
            pause_silence_dbfs: 0.5,
            chunk_ms: 15_000,
            ..Tuning::default()
        };
        let problems = t.validate();
        let keys: Vec<_> = problems.iter().map(|p| p.key).collect();
        assert_eq!(keys, ["preview_interval_ms", "preview_window_ms", "gate_quiet_db", "pause_silence_dbfs"]);
        assert_eq!(t, Tuning { chunk_ms: 15_000, ..Tuning::default() });
        assert_eq!(
            problems[0].to_string(),
            "preview_interval_ms = 60000 is outside 0..=10000, so it stays at its default, 1600"
        );
        // Range ends are allowed.
        let mut t = Tuning {
            preview_interval_ms: 0,
            chunk_ms: 0,
            pause_quiet_db: 0.0,
            gate_silence_dbfs: -120.0,
            ..Tuning::default()
        };
        assert!(t.validate().is_empty());
    }

    #[test]
    fn set_reads_any_field_by_name_and_rejects_unknown_keys() {
        let mut t = Tuning::default();
        t.set("preview_interval_ms", "800").unwrap();
        t.set("pause_quiet_db", " -25.5 ").unwrap();
        assert_eq!(t, Tuning { preview_interval_ms: 800, pause_quiet_db: -25.5, ..Tuning::default() });
        let err = t.set("preview_interval", "800").unwrap_err();
        assert!(err.contains("unknown key") && err.contains("preview_interval_ms"), "{err}");
        assert!(t.set("preview_interval_ms", "-1").is_err());
        assert!(t.set("preview_interval_ms", "1.5").is_err());
        let err = t.set("gate_quiet_db", "loud").unwrap_err();
        assert_eq!(err, r#"gate_quiet_db: can't read "loud" as a number"#);
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
        let level = fields.iter().find(|f| f.key == "gate_silence_dbfs").unwrap();
        assert_eq!((level.value.as_str(), level.min.as_str(), level.max.as_str()), ("-55", "-120", "0"));
    }

    #[test]
    fn every_key_names_its_unit() {
        for f in Tuning::default().fields() {
            assert!(!f.unit().is_empty(), "{}: end the key in a unit (_ms, _db, _dbfs, _words, ...)", f.key);
        }
        assert_eq!(unit_of("gate_silence_dbfs"), "dBFS (decibels relative to full scale)");
        assert_eq!(unit_of("gate_quiet_db"), "dB (relative to another level)");
    }

    #[test]
    fn decibels_convert_to_levels() {
        assert_eq!(db_to_linear(0.0), 1.0);
        assert_eq!(db_to_linear(-20.0), 0.1);
        assert_eq!(db_to_linear(-40.0), 0.01);
        assert!((db_to_linear(-55.0) - 0.001_778_28).abs() < 1e-8);
        assert!((db_to_linear(-6.0) - 0.501_19).abs() < 1e-5);
    }
}
