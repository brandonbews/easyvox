//! easyvox-bench: developer CLI for easyvox-core. Load time, memory, speed and word error rate
//! (WER) over the test clips, plus one-off probes for the microphone, paste and history.
//!
//! Run `easyvox-bench help` for the commands. Models come from `easyvox_core::paths::models_dir()`
//! (override with `--models DIR` or `EASYVOX_MODELS_DIR`); clips come from the workspace's
//! `testdata/` (see `testdata/generate.sh`). Only release builds give meaningful numbers:
//!
//!   cargo run --release -p easyvox-bench -- bench --model parakeet-tdt-0.6b-v2-q5_k_m-gguf
//!
//! Timings drift when something else is busy on the machine, so the output records the
//! 1-minute load average next to them.

mod wer;

use anyhow::{Context, bail};
use easyvox_core::metrics::proc_stats;
use easyvox_core::*;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{Duration, Instant};

const USAGE: &str = "\
easyvox-bench <command> [options]

  info                                         catalog, model presence, permissions
  transcribe --model ID --wav F [--words]      one-shot decode of a WAV file
  bench --model ID [--cold] [--fast|--reps N]  load times, memory, speed and WER over testdata (3 reps)
  session --model ID --wav F [--ref T] [--quiet] [--idle-before SECS] [--unload]
                                               WAV replay session with live partials (--ref: WER
                                               against the transcript in file T; --idle-before:
                                               sit idle with the model loaded first; --unload:
                                               free the model first, so the session reloads it)
  sessions --model ID                          sessions on 3/10/30 s clips, partials on and off
  tuning                                       every tuning key: effective value, default, range, docs
  mic [--request] [--secs N] [--model ID]      open the microphone once (--request asks for permission first)
  paste [--text T]                             write the clipboard, then simulate Cmd+V
  history [--db F]                             SQLite history demo

Engine options: --models DIR, --threads N, --vad, and --set KEY=VALUE (repeatable) for any
field of easyvox_core::Tuning (see `tuning`), e.g. --set preview_interval_ms=800. Older
aliases: --interval MS (preview_interval_ms), --window SECS (preview_window_ms), --chunk SECS
(chunk_ms); --set wins over them. Out-of-range values are an error.
Output: JSON on stdout (or --out F), progress on stderr. Results include the effective tuning.
EASYVOX_METAL_RESIDENCY=1 keeps ggml-metal's residency sets (and their idle wake-ups), for
comparisons; by default they're off, as in the app (see easyvox_core::process_env).";

/// The workspace's test clips.
fn testdata() -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata");
    dir.canonicalize().unwrap_or(dir)
}

/// Clips `sessions` runs: short, medium and long.
const SESSION_CLIPS: [&str; 3] = ["cmd_samantha", "punct_samantha", "long_daniel"];

struct Clip {
    id: String,
    category: String,
    text: String,
    path: PathBuf,
}

/// `testdata/manifest.tsv`: id, voice, category, reference text (tab-separated).
fn clips() -> anyhow::Result<Vec<Clip>> {
    let manifest = std::fs::read_to_string(testdata().join("manifest.tsv")).context("read testdata/manifest.tsv")?;
    manifest
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, l)| {
            let f: Vec<&str> = l.splitn(4, '\t').collect();
            if f.len() < 3 {
                bail!("manifest.tsv line {}: expected id, voice, category, text", i + 1);
            }
            Ok(Clip {
                id: f[0].into(),
                category: f[2].into(),
                text: f.get(3).copied().unwrap_or("").into(),
                path: testdata().join(format!("{}.wav", f[0])),
            })
        })
        .collect()
}

fn mb(b: u64) -> f64 {
    (b as f64 / 1e6 * 10.0).round() / 10.0
}

/// 1-minute load average, for context next to timings.
fn loadavg() -> f64 {
    unsafe extern "C" {
        fn getloadavg(loadavg: *mut f64, nelem: i32) -> i32;
    }
    let mut l = [0f64; 3];
    // SAFETY: the buffer holds the 3 values asked for.
    if unsafe { getloadavg(l.as_mut_ptr(), 3) } < 1 {
        return f64::NAN;
    }
    (l[0] * 100.0).round() / 100.0
}

fn mem_json() -> Value {
    let s = proc_stats();
    json!({
        "rss_mb": mb(s.rss_bytes), "footprint_mb": mb(s.footprint_bytes),
        "peak_footprint_mb": mb(s.peak_footprint_bytes), "peak_rss_mb": mb(s.peak_rss_bytes),
        "cpu_secs": s.cpu_secs,
    })
}

struct Args(Vec<String>);

impl Args {
    fn get(&self, k: &str) -> Option<String> {
        self.0.iter().position(|a| a == k).and_then(|i| self.0.get(i + 1).cloned())
    }
    /// The value after every occurrence of `k`, for repeatable options.
    fn all(&self, k: &str) -> Vec<String> {
        self.0.windows(2).filter(|w| w[0] == k).map(|w| w[1].clone()).collect()
    }
    fn has(&self, k: &str) -> bool {
        self.0.iter().any(|a| a == k)
    }
    fn num<T: FromStr>(&self, k: &str) -> anyhow::Result<Option<T>> {
        self.get(k).map(|v| v.parse().map_err(|_| anyhow::anyhow!("{k}: can't parse {v:?}"))).transpose()
    }
    fn model(&self) -> anyhow::Result<String> {
        self.get("--model").context("--model <id> required (see `easyvox-bench info`)")
    }
    fn wav(&self) -> anyhow::Result<PathBuf> {
        self.get("--wav").map(PathBuf::from).context("--wav <file> required")
    }
    fn models_dir(&self) -> PathBuf {
        self.get("--models").map(PathBuf::from).unwrap_or_else(paths::models_dir)
    }
}

fn config(args: &Args, model: &str) -> anyhow::Result<EngineConfig> {
    let mut c = EngineConfig::new(model, args.models_dir());
    if let Some(t) = args.num("--threads")? {
        c.threads = t;
    }
    c.vad_trim = args.has("--vad");
    c.tuning = tuning(args)?;
    Ok(c)
}

/// The defaults, changed by the older aliases and then by every `--set key=value`.
fn tuning(args: &Args) -> anyhow::Result<Tuning> {
    let mut t = Tuning::default();
    if let Some(ms) = args.num("--interval")? {
        t.preview_interval_ms = ms;
    }
    if let Some(secs) = args.num("--window")? {
        t.preview_window_ms = secs_to_ms("--window", secs)?;
    }
    if let Some(secs) = args.num("--chunk")? {
        t.chunk_ms = secs_to_ms("--chunk", secs)?;
    }
    for kv in args.all("--set") {
        let (key, value) = kv.split_once('=').with_context(|| format!("--set {kv:?}: expected key=value"))?;
        t.set(key.trim(), value).map_err(|e| anyhow::anyhow!("--set {kv:?}: {e}"))?;
    }
    let problems = t.validate();
    if !problems.is_empty() {
        let list: Vec<String> =
            problems.iter().map(|p| format!("{} = {} (allowed {}..={})", p.key, p.value, p.min, p.max)).collect();
        bail!("tuning out of range: {}", list.join("; "));
    }
    Ok(t)
}

fn secs_to_ms(flag: &str, secs: f64) -> anyhow::Result<u32> {
    let ms = (secs * 1e3).round();
    if !(0.0..=u32::MAX as f64).contains(&ms) {
        bail!("{flag}: {secs} isn't a usable number of seconds");
    }
    Ok(ms as u32)
}

/// The tuning as a JSON object of numbers.
fn tuning_json(t: &Tuning) -> Value {
    let number = |v: String| serde_json::from_str(&v).unwrap_or(Value::String(v));
    Value::Object(t.fields().into_iter().map(|f| (f.key.to_string(), number(f.value))).collect())
}

fn write_out(args: &Args, v: &Value) -> anyhow::Result<()> {
    let s = serde_json::to_string_pretty(v)?;
    if let Some(p) = args.get("--out") {
        std::fs::write(&p, &s)?;
        eprintln!("wrote {p}");
    } else {
        println!("{s}");
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    for (key, value) in process_env() {
        // SAFETY: the first thing `main` does, before any thread exists, so nothing can read
        // the environment while it changes.
        unsafe { std::env::set_var(key, value) };
    }
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let cmd = argv.first().cloned().unwrap_or_else(|| "help".into());
    let args = Args(argv);
    match cmd.as_str() {
        "info" => info(&args),
        "transcribe" => transcribe(&args),
        "bench" => bench(&args),
        "session" => session_cmd(&args),
        "sessions" => sessions(&args),
        "tuning" => tuning_cmd(&args),
        "mic" => mic(&args),
        "paste" => paste(&args),
        "history" => history_demo(&args),
        "help" | "-h" | "--help" => {
            println!("{USAGE}");
            Ok(())
        }
        other => {
            eprintln!("{USAGE}");
            bail!("unknown command {other:?}")
        }
    }
}

fn info(args: &Args) -> anyhow::Result<()> {
    let md = args.models_dir();
    let cat: Vec<Value> = catalog()
        .iter()
        .map(|m| {
            json!({
                "id": m.id, "name": m.name, "runtime": format!("{:?}", m.runtime),
                "offering": format!("{:?}", m.offering), "license": m.license.name,
                "compiled_in": m.runtime.is_compiled(), "present": m.resolve(&md).exists(),
                "download_mb": mb(m.download_bytes), "disk_mb": mb(m.disk_bytes),
            })
        })
        .collect();
    let v = json!({
        "models_dir": md, "testdata": testdata(), "recommended_model": RECOMMENDED_MODEL, "catalog": cat,
        "mic_permission": format!("{:?}", mic_permission_status()),
        "accessibility_trusted": accessibility_trusted(),
        "loadavg": loadavg(),
    });
    write_out(args, &v)
}

fn transcribe(args: &Args) -> anyhow::Result<()> {
    let model = args.model()?;
    let wav = args.wav()?;
    let t = Instant::now();
    let engine = Engine::load(config(args, &model)?)?;
    let load_ms = t.elapsed().as_millis();
    let pcm = audio::load_wav_16k_mono(&wav)?;
    let t = Instant::now();
    let h = engine.transcribe_pcm(&pcm)?;
    let ms = t.elapsed().as_secs_f64() * 1e3;
    let secs = pcm.len() as f64 / SAMPLE_RATE as f64;
    if args.has("--words") {
        for w in &h.words {
            eprintln!("{:7.2} {:7.2}  {}", w.start, w.end, w.text);
        }
    }
    let v = json!({
        "model": model, "text": h.text, "words": h.words.len(), "load_ms": load_ms,
        "decode_ms": ms, "audio_secs": secs, "rtf": ms / 1e3 / secs, "loadavg": loadavg(),
    });
    write_out(args, &v)
}

/// APFS clone of the model into a temp dir: same bytes, fresh vnode, so not in the page cache.
fn cold_clone(models_dir: &Path, model: &str) -> anyhow::Result<PathBuf> {
    let info = models::find(model)?;
    let src = info.resolve(models_dir);
    let root = std::env::temp_dir().join(format!("easyvox-cold-{}", std::process::id()));
    let dst = root.join(info.path);
    std::fs::create_dir_all(dst.parent().context("model path has no parent")?)?;
    let st = std::process::Command::new("cp").arg("-cR").arg(&src).arg(&dst).status()?;
    if !st.success() {
        bail!("cp -c {} failed", src.display());
    }
    // Silero VAD too (tiny), for --vad.
    let vad = models_dir.join(models::SILERO_VAD_PATH);
    if vad.exists() {
        std::fs::create_dir_all(root.join("vad"))?;
        std::fs::copy(vad, root.join(models::SILERO_VAD_PATH))?;
    }
    Ok(root)
}

fn bench(args: &Args) -> anyhow::Result<()> {
    let model = args.model()?;
    let loadavg_start = loadavg();
    let mem_start = mem_json();
    let mut cfg = config(args, &model)?;
    let mut cold_root = None;
    if args.has("--cold") {
        let root = cold_clone(&cfg.models_dir, &model)?;
        cfg.models_dir = root.clone();
        cold_root = Some(root);
    }
    let t = Instant::now();
    let engine = Engine::load(cfg.clone())?;
    let load1 = t.elapsed().as_secs_f64() * 1e3;
    let mem_loaded = mem_json();

    let reps = if args.has("--fast") { 1 } else { args.num("--reps")?.unwrap_or(3).max(1) };
    let mut first_decode_ms = None;
    let mut rows = Vec::new();
    let (mut err_sum, mut ref_sum) = (0, 0);
    let (mut speech_secs, mut speech_ms) = (0.0, 0.0);
    // Dictation's worst failure is "I spoke and nothing came out", so every rep counts:
    // an empty or failed decode on a speech clip, or reps that disagree, is reported.
    let (mut empty_decodes, mut failed_decodes, mut unstable_clips) = (0, 0, Vec::new());
    for c in &clips()? {
        let pcm = audio::load_wav_16k_mono(&c.path)?;
        let secs = pcm.len() as f64 / SAMPLE_RATE as f64;
        let speech = c.category != "silence";
        let load_before = loadavg();
        let mut times = Vec::new();
        let mut texts: Vec<String> = Vec::new();
        let mut failures = Vec::new();
        let (mut e, mut n) = (0, 0);
        for _ in 0..reps {
            let t = Instant::now();
            let text = match engine.transcribe_pcm(&pcm) {
                Ok(h) => h.text,
                Err(err) => {
                    failures.push(format!("{err:#}"));
                    String::new()
                }
            };
            let ms = t.elapsed().as_secs_f64() * 1e3;
            first_decode_ms.get_or_insert(ms);
            times.push(ms);
            let (re, rn) = wer::wer_counts(&c.text, &text);
            e += re;
            n += rn;
            if speech && text.trim().is_empty() {
                empty_decodes += 1;
            }
            texts.push(text);
        }
        times.sort_by(f64::total_cmp);
        let med = times[times.len() / 2];
        if speech {
            failed_decodes += failures.len();
            err_sum += e;
            ref_sum += n;
            speech_secs += secs;
            speech_ms += med;
        }
        let mut distinct = texts.clone();
        distinct.dedup();
        if distinct.len() > 1 {
            unstable_clips.push(c.id.clone());
        }
        let text = texts.last().cloned().unwrap_or_default();
        let flag = if !failures.is_empty() {
            "  FAILED"
        } else if speech && texts.iter().any(|t| t.trim().is_empty()) {
            "  EMPTY"
        } else if distinct.len() > 1 {
            "  UNSTABLE"
        } else {
            ""
        };
        eprintln!("{:16} {:5.1}s {:7.1}ms  wer {:>2}/{:<2}{flag}  {}", c.id, secs, med, e / reps, n / reps, text);
        rows.push(json!({
            "id": c.id, "category": c.category, "audio_secs": secs,
            "decode_ms_median": med, "decode_ms_all": times, "rtf": med / 1e3 / secs,
            "loadavg_before": load_before, "loadavg_after": loadavg(),
            "errors": e, "ref_words": n, "wer": if n > 0 { e as f64 / n as f64 } else { 0.0 },
            "reference": c.text, "hypothesis": text, "hypotheses_distinct": distinct, "failures": failures,
        }));
    }
    let mem_after_decodes = mem_json();
    std::thread::sleep(Duration::from_secs(2));
    let mem_idle = mem_json();

    // Warm reload in the same process (files in the page cache, runtime initialized).
    drop(engine);
    std::thread::sleep(Duration::from_millis(500));
    let mem_unloaded = mem_json();
    let t = Instant::now();
    let engine2 = Engine::load(cfg.clone())?;
    let load2 = t.elapsed().as_secs_f64() * 1e3;
    drop(engine2);
    if let Some(r) = cold_root {
        let _ = std::fs::remove_dir_all(r);
    }

    let v = json!({
        "model": model, "threads": cfg.threads, "vad_trim": cfg.vad_trim, "cold_clone": args.has("--cold"),
        "tuning": tuning_json(&cfg.tuning),
        "metal_residency": metal_residency(),
        "reps": reps, "loadavg_start": loadavg_start,
        "load_ms_first_in_process": load1, "load_ms_warm_reload": load2,
        "first_decode_ms": first_decode_ms,
        "wer_total": err_sum as f64 / ref_sum.max(1) as f64, "errors": err_sum, "ref_words": ref_sum,
        "empty_speech_decodes": empty_decodes, "failed_speech_decodes": failed_decodes,
        "unstable_clips": unstable_clips,
        "rtf_speech_clips": speech_ms / 1e3 / speech_secs,
        "mem": {
            "start": mem_start, "after_load": mem_loaded, "after_decodes": mem_after_decodes,
            "idle_2s": mem_idle, "after_unload": mem_unloaded,
        },
        "clips": rows,
    });
    write_out(args, &v)
}

/// Runs one WAV replay session to its `Final` and reports latency, partial behavior and CPU.
fn run_session(engine: &Engine, wav: &Path, quiet: bool) -> anyhow::Result<Value> {
    let cpu0 = proc_stats().cpu_secs;
    let t0 = Instant::now();
    let (session, rx) = engine.start(AudioSource::WavReplay(wav.to_path_buf()))?;
    let mut events = Vec::new();
    let mut final_ev = None;
    let mut errors = Vec::new();
    for ev in rx.iter() {
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        match ev {
            TranscriptEvent::Partial { confirmed, provisional } => {
                if !quiet {
                    eprintln!("{ms:8.0}ms  {confirmed} \x1b[2m{provisional}\x1b[0m");
                }
                events.push((ms, confirmed, provisional));
            }
            TranscriptEvent::Final { text, audio_secs, finalize_ms } => {
                if !quiet {
                    eprintln!("{ms:8.0}ms  FINAL ({finalize_ms} ms after end of audio): {text}");
                }
                final_ev = Some((ms, text, audio_secs, finalize_ms));
                break;
            }
            TranscriptEvent::Error(e) => {
                eprintln!("{ms:8.0}ms  ERROR {e}");
                errors.push(e);
            }
        }
    }
    session.join();
    let wall = t0.elapsed().as_secs_f64();
    let cpu = proc_stats().cpu_secs - cpu0;
    let stats = session.stats();
    let (fms, final_text, audio_secs, finalize_ms) = final_ev.context("no Final event")?;

    // Partial metrics.
    let n = events.len();
    let gaps: Vec<f64> = events.windows(2).map(|w| w[1].0 - w[0].0).collect();
    let mean_gap = if gaps.is_empty() { 0.0 } else { gaps.iter().sum::<f64>() / gaps.len() as f64 };
    let mut sorted = gaps.clone();
    sorted.sort_by(f64::total_cmp);
    let med_gap = sorted.get(sorted.len() / 2).copied().unwrap_or(0.0);
    let mut revisions = 0;
    let mut words_revised = 0;
    // Confirmed text only grows, except when a polished chunk replaces its preview text, so
    // this stays at or under `chunks`.
    let mut confirmed_rewrites = 0;
    for w in events.windows(2) {
        let d1 = wer::normalize(&format!("{} {}", w[0].1, w[0].2));
        let d2 = wer::normalize(&format!("{} {}", w[1].1, w[1].2));
        let common = d1.iter().zip(&d2).take_while(|(a, b)| a == b).count();
        if common < d1.len() {
            revisions += 1;
            words_revised += d1.len() - common;
        }
        if !w[1].1.starts_with(w[0].1.as_str()) {
            confirmed_rewrites += 1;
        }
    }
    let final_words = wer::normalize(&final_text);
    let conf_words = wer::normalize(&stats.confirmed_at_stop);
    let k = conf_words.len().min(final_words.len());
    let conf_vs_final_err = wer::edit_distance(&conf_words, &final_words[..k]);
    let first_partial_ms = events.iter().find(|e| !(e.1.is_empty() && e.2.is_empty())).map(|e| e.0);
    let speech_secs = audio_secs as f64;
    let confirmed_coverage =
        if final_words.is_empty() { 0.0 } else { conf_words.len() as f64 / final_words.len() as f64 };
    let pass_ms_mean =
        if stats.partial_passes > 0 { stats.partial_decode_ms_total / stats.partial_passes as f64 } else { 0.0 };
    Ok(json!({
        "wav": wav.file_name().map(|f| f.to_string_lossy()),
        "audio_secs": audio_secs, "finalize_ms": finalize_ms, "final_at_ms": fms,
        "model_wait_ms": stats.model_wait_ms, "speech_onset_secs": stats.speech_onset_secs,
        "final_wait_ms": stats.final_wait_ms, "final_decode_ms": stats.final_decode_ms,
        "final_text": final_text,
        "partials": {
            "events": n, "first_partial_ms": first_partial_ms,
            "mean_gap_ms": mean_gap, "median_gap_ms": med_gap,
            "updates_per_sec": n as f64 / speech_secs,
            "revisions": revisions, "revision_rate": if n > 1 { revisions as f64 / (n - 1) as f64 } else { 0.0 },
            "words_revised": words_revised, "confirmed_rewrites": confirmed_rewrites,
            "passes": stats.partial_passes, "skipped": stats.partial_skipped,
            "pass_ms_mean": pass_ms_mean, "pass_ms_max": stats.partial_decode_ms_max,
            "window_secs_mean": stats.partial_audio_secs / stats.partial_passes.max(1) as f64,
            "confirmed_words_at_stop": conf_words.len(), "final_words": final_words.len(),
            "confirmed_coverage": confirmed_coverage,
            "confirmed_vs_final_errors": conf_vs_final_err,
        },
        "chunks": {
            "polished": stats.chunks, "decode_ms_total": stats.chunk_decode_ms_total,
            "decode_ms_max": stats.chunk_decode_ms_max,
        },
        "cpu": {"cpu_secs": cpu, "wall_secs": wall, "cpu_pct_of_one_core": 100.0 * cpu / wall},
        "errors": errors,
    }))
}

/// Whether ggml-metal keeps its residency sets (see [`process_env`]).
fn metal_residency() -> bool {
    std::env::var_os("GGML_METAL_NO_RESIDENCY").is_none()
}

fn session_cmd(args: &Args) -> anyhow::Result<()> {
    let model = args.model()?;
    let wav = args.wav()?;
    let engine = Engine::load(config(args, &model)?)?;
    // Warm up (the first decode allocates and compiles kernels).
    engine.transcribe_pcm(&vec![0.0; SAMPLE_RATE as usize])?;
    // Like the app after a stretch of no use: the model freed, and/or the GPU left idle.
    let unloaded = args.has("--unload") && engine.unload();
    let mem_before = mem_json();
    let idle_secs: f64 = args.num("--idle-before")?.unwrap_or(0.0);
    if idle_secs > 0.0 {
        eprintln!("idling {idle_secs} s (model {})", if unloaded { "unloaded" } else { "loaded" });
        std::thread::sleep(Duration::from_secs_f64(idle_secs));
    }
    let loadavg_before = loadavg();
    let mut v = run_session(&engine, &wav, args.has("--quiet"))?;
    if let Some(path) = args.get("--ref") {
        let reference = std::fs::read_to_string(&path).with_context(|| format!("read {path}"))?;
        let (errors, words) = wer::wer_counts(&reference, v["final_text"].as_str().unwrap_or_default());
        v["wer"] = json!({"errors": errors, "ref_words": words, "wer": errors as f64 / words.max(1) as f64});
    }
    v["model"] = json!(model);
    v["tuning"] = tuning_json(&engine.config().tuning);
    v["metal_residency"] = json!(metal_residency());
    v["idle_before_secs"] = json!(idle_secs);
    v["unloaded_before"] = json!(unloaded);
    v["mem_before_session"] = mem_before;
    v["loadavg_before"] = json!(loadavg_before);
    v["loadavg_after"] = json!(loadavg());
    v["mem_end"] = mem_json();
    write_out(args, &v)
}

fn sessions(args: &Args) -> anyhow::Result<()> {
    let model = args.model()?;
    let base = config(args, &model)?;
    let engine = Engine::load(base.clone())?;
    engine.transcribe_pcm(&audio::load_wav_16k_mono(&testdata().join("cmd_samantha.wav"))?)?; // warm-up
    let interval = base.tuning.preview_interval_ms;
    let mut out = Vec::new();
    for (label, iv) in [("partials_on", interval), ("partials_off", 0)] {
        // The interval is part of the engine config, so "off" needs its own engine.
        let e = if iv == interval {
            engine.clone()
        } else {
            let mut c = base.clone();
            c.tuning.preview_interval_ms = iv;
            Engine::load(c)?
        };
        for w in SESSION_CLIPS {
            let loadavg_before = loadavg();
            let r = run_session(&e, &testdata().join(format!("{w}.wav")), true)?;
            eprintln!(
                "{label:12} {w:16} audio {:5.1}s  first words {:>5.0} ms  finalize {:>5} ms  partials {:>3}  cpu {:5.1}%",
                r["audio_secs"].as_f64().unwrap_or(0.0),
                r["partials"]["first_partial_ms"].as_f64().unwrap_or(f64::NAN),
                r["finalize_ms"],
                r["partials"]["events"],
                r["cpu"]["cpu_pct_of_one_core"].as_f64().unwrap_or(0.0)
            );
            out.push(json!({
                "mode": label, "interval_ms": iv, "clip": w,
                "loadavg_before": loadavg_before, "loadavg_after": loadavg(), "result": r,
            }));
        }
    }
    let v = json!({
        "model": model, "threads": base.threads, "vad_trim": base.vad_trim, "tuning": tuning_json(&base.tuning),
        "metal_residency": metal_residency(),
        "runs": out, "mem_end": mem_json(),
    });
    write_out(args, &v)
}

/// Every tuning key with its effective value (after the aliases and `--set`), default, range
/// and docs.
fn tuning_cmd(args: &Args) -> anyhow::Result<()> {
    let t = tuning(args)?;
    let keys: Vec<Value> = t
        .fields()
        .into_iter()
        .map(|f| {
            let doc = f.doc.lines().map(str::trim).collect::<Vec<_>>().join(" ");
            json!({"key": f.key, "value": f.value, "default": f.default, "min": f.min, "max": f.max, "doc": doc})
        })
        .collect();
    write_out(args, &json!({"tuning": tuning_json(&t), "keys": keys}))
}

fn mic(args: &Args) -> anyhow::Result<()> {
    let secs: f64 = args.num("--secs")?.unwrap_or(3.0);
    let before = mic_permission_status();
    eprintln!("mic permission before: {before:?}");
    let requested = if args.has("--request") {
        // Shows the system prompt (attributed to the terminal app); waits up to 60 s for an answer.
        let st = request_mic_permission(Duration::from_secs(60));
        eprintln!("mic permission after request: {st:?}");
        Some(format!("{st:?}"))
    } else {
        None
    };
    // The session API needs an engine, so this loads one even though only the mic matters.
    let model = args.get("--model").unwrap_or_else(|| RECOMMENDED_MODEL.into());
    let engine = Engine::load(config(args, &model)?)?;
    let t0 = Instant::now();
    let v = match engine.start(AudioSource::Microphone) {
        Err(e) => json!({"opened": false, "error": format!("{e:#}"), "open_ms": t0.elapsed().as_millis() as u64}),
        Ok((session, rx)) => {
            let open_ms = t0.elapsed().as_millis() as u64;
            let mut events = Vec::new();
            let deadline = Instant::now() + Duration::from_secs_f64(secs);
            while Instant::now() < deadline {
                if let Ok(ev) = rx.recv_timeout(Duration::from_millis(50)) {
                    events.push(format!("{ev:?}"));
                }
            }
            session.stop();
            for ev in rx.iter() {
                let fin = matches!(ev, TranscriptEvent::Final { .. });
                events.push(format!("{ev:?}"));
                if fin {
                    break;
                }
            }
            session.join();
            let s = session.stats();
            let m = s.mic.as_ref();
            json!({
                "opened": true, "open_ms": open_ms, "device": m.map(|m| &m.device),
                "device_rate": m.map(|m| m.sample_rate), "device_channels": m.map(|m| m.channels),
                "sample_format": m.map(|m| &m.sample_format),
                "audio_secs": s.audio_secs, "input_peak": s.input_peak, "events": events,
            })
        }
    };
    let after = mic_permission_status();
    let v = json!({
        "permission_before": format!("{before:?}"), "requested": requested,
        "permission_after": format!("{after:?}"), "result": v,
    });
    write_out(args, &v)
}

/// Writes the clipboard, then simulates Cmd+V into whatever app has focus. It never reads the
/// clipboard back: on macOS 15.4+ that can trigger a paste-privacy prompt.
fn paste(args: &Args) -> anyhow::Result<()> {
    let text = args.get("--text").unwrap_or_else(|| format!("easyvox clipboard probe {}", std::process::id()));
    let ax = accessibility_trusted();
    let write = match copy_to_clipboard(&text) {
        Ok(w) => json!({
            "ok": true, "change_count_before": w.change_count_before, "change_count_after": w.change_count_after,
            "has_string_type": w.has_string_type, "access_behavior": w.access_behavior,
        }),
        Err(e) => json!({"ok": false, "error": format!("{e:#}")}),
    };
    let pasted = match paste_text(&text) {
        Ok(()) => json!({"ok": true}),
        Err(e) => json!({"ok": false, "error": format!("{e:#}")}),
    };
    let v = json!({"text": text, "accessibility_trusted": ax, "clipboard_write": write, "paste_text": pasted});
    write_out(args, &v)
}

fn history_demo(args: &Args) -> anyhow::Result<()> {
    let p = args
        .get("--db")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("easyvox-history-demo.sqlite3"));
    let h = History::open(&p)?;
    h.add("Hello from the history demo.", 2.5, RECOMMENDED_MODEL)?;
    for e in &h.recent(5)? {
        println!("{} {} {:.1}s {} {}", e.id, e.created_at_ms, e.duration_secs, e.model_id, e.text);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_lists_clips_that_exist() {
        let clips = clips().unwrap();
        assert!(clips.len() >= SESSION_CLIPS.len());
        for c in &clips {
            assert!(c.path.exists(), "{} is missing", c.path.display());
            assert!(c.category == "silence" || !c.text.is_empty(), "{} has no reference text", c.id);
        }
        for id in SESSION_CLIPS {
            assert!(clips.iter().any(|c| c.id == id), "{id} not in manifest.tsv");
        }
    }

    fn args(a: &[&str]) -> Args {
        Args(a.iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn set_changes_any_tuning_field_and_rejects_unknown_keys() {
        let t = tuning(&args(&["session", "--set", "preview_interval_ms=800", "--set", "pause_quiet_db=-14"])).unwrap();
        assert_eq!(t, Tuning { preview_interval_ms: 800, pause_quiet_db: -14.0, ..Tuning::default() });
        for bad in ["no_such_key=1", "preview_interval_ms", "preview_interval_ms=fast", "preview_interval_ms=99999"] {
            assert!(tuning(&args(&["session", "--set", bad])).is_err(), "--set {bad} was accepted");
        }
        let err = tuning(&args(&["--set", "chunk=15"])).unwrap_err().to_string();
        assert!(err.contains("unknown key") && err.contains("chunk_ms"), "{err}");
    }

    #[test]
    fn the_older_flags_are_aliases_that_set_loses_to() {
        let t = tuning(&args(&["--interval", "600", "--window", "12.5", "--chunk", "0"])).unwrap();
        assert_eq!(t, Tuning { preview_interval_ms: 600, preview_window_ms: 12_500, chunk_ms: 0, ..Tuning::default() });
        let t = tuning(&args(&["--chunk", "15", "--set", "chunk_ms=20000"])).unwrap();
        assert_eq!(t.chunk_ms, 20_000);
        assert!(tuning(&args(&["--chunk", "-1"])).is_err());
        assert_eq!(tuning(&args(&["session"])).unwrap(), Tuning::default());
    }

    #[test]
    fn results_record_the_tuning_as_numbers() {
        let v = tuning_json(&Tuning { chunk_ms: 15_000, ..Tuning::default() });
        assert_eq!(v["chunk_ms"], json!(15_000));
        assert_eq!(v["preview_interval_ms"], json!(1_600));
        assert_eq!(v["gate_silence_dbfs"].as_f64(), Some(-55.0));
        assert_eq!(v.as_object().unwrap().len(), Tuning::KEYS.len());
    }

    #[test]
    fn numeric_options_parse_or_fail_clearly() {
        let args = Args(["bench", "--threads", "8", "--window", "x"].map(String::from).to_vec());
        assert_eq!(args.num::<usize>("--threads").unwrap(), Some(8));
        assert_eq!(args.num::<usize>("--interval").unwrap(), None);
        assert!(args.num::<f32>("--window").is_err());
    }
}
