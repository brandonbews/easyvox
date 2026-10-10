//! Audio input: WAV replay at real-time speed, microphone via cpal, 16 kHz mono conversion.
//!
//! Mic gotcha: while the permission is "not determined", opening CoreAudio blocks with no
//! error until someone answers the system prompt. `spawn_mic` refuses to open it until the
//! permission is granted, so ask with [`request_mic_permission`] first.

use crate::SAMPLE_RATE;
use crate::pause::FrameLevels;
use anyhow::{Context, bail};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Audio shared between the capture thread, the session worker and the level meter.
#[derive(Default)]
pub(crate) struct SharedAudio {
    pub samples: Mutex<Vec<f32>>,
    /// The loudness of `samples`, 30 ms frame by frame, measured once as they arrive. Speech
    /// detection and the level meter read it; it has a lock of its own, so the meter never
    /// waits on the worker copying audio out of `samples`.
    pub levels: Mutex<FrameLevels>,
    pub cv: Condvar,
    /// The source ran out (WAV ended) or capture stopped.
    pub done: AtomicBool,
    pub done_at: Mutex<Option<Instant>>,
    /// Peak |sample| seen so far (f32 bits) and number of capture callbacks.
    pub peak_bits: AtomicU32,
    pub callbacks: AtomicU64,
}

impl SharedAudio {
    pub fn push(&self, chunk: &[f32]) {
        let peak = chunk.iter().fold(0f32, |m, s| m.max(s.abs()));
        if peak > f32::from_bits(self.peak_bits.load(Ordering::Relaxed)) {
            self.peak_bits.store(peak.to_bits(), Ordering::Relaxed);
        }
        self.callbacks.fetch_add(1, Ordering::Relaxed);
        self.levels.lock().unwrap().push(chunk);
        self.samples.lock().unwrap().extend_from_slice(chunk);
        self.cv.notify_all();
    }
    pub fn len(&self) -> usize {
        self.samples.lock().unwrap().len()
    }
    pub fn peak(&self) -> f32 {
        f32::from_bits(self.peak_bits.load(Ordering::Relaxed))
    }
    pub fn finish(&self) {
        self.done_at.lock().unwrap().get_or_insert_with(Instant::now);
        self.done.store(true, Ordering::SeqCst);
        self.cv.notify_all();
    }
}

/// Streaming resampler: windowed-sinc low-pass (when downsampling) + linear interpolation.
/// Good enough for speech; a production build might use `rubato`.
pub struct Resampler {
    step: f64,
    pos: f64,
    prev: f32,
    taps: Vec<f32>,
    hist: Vec<f32>,
}

impl Resampler {
    pub fn new(from_hz: u32, to_hz: u32) -> Self {
        let taps = if to_hz < from_hz {
            let fc = 0.45 * to_hz as f64 / from_hz as f64; // cutoff, cycles/sample
            let n = 31;
            let m = (n - 1) as f64 / 2.0;
            let mut t: Vec<f32> = (0..n)
                .map(|i| {
                    let x = i as f64 - m;
                    let sinc = if x == 0.0 {
                        2.0 * fc
                    } else {
                        (2.0 * std::f64::consts::PI * fc * x).sin() / (std::f64::consts::PI * x)
                    };
                    let w = 0.54 - 0.46 * (2.0 * std::f64::consts::PI * i as f64 / (n - 1) as f64).cos();
                    (sinc * w) as f32
                })
                .collect();
            let s: f32 = t.iter().sum();
            t.iter_mut().for_each(|v| *v /= s);
            t
        } else {
            vec![1.0]
        };
        let hist = vec![0.0; taps.len() - 1];
        Self { step: from_hz as f64 / to_hz as f64, pos: 0.0, prev: 0.0, taps, hist }
    }

    pub fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        if input.is_empty() {
            return;
        }
        // FIR low-pass with carried-over history.
        let filtered: Vec<f32> = if self.taps.len() == 1 {
            input.to_vec()
        } else {
            let mut buf = std::mem::take(&mut self.hist);
            buf.extend_from_slice(input);
            let k = self.taps.len();
            let f = (0..input.len()).map(|i| buf[i..i + k].iter().zip(&self.taps).map(|(a, b)| a * b).sum()).collect();
            self.hist = buf[buf.len() - (k - 1)..].to_vec();
            f
        };
        // Linear interpolation; x[-1] = prev.
        let n = filtered.len() as f64;
        let at = |i: isize| if i < 0 { self.prev } else { filtered[i as usize] };
        while self.pos < n - 1.0 {
            let i = self.pos.floor() as isize;
            let frac = (self.pos - i as f64) as f32;
            let (a, b) = (at(i), at(i + 1));
            out.push(a + (b - a) * frac);
            self.pos += self.step;
        }
        self.pos -= n;
        self.prev = *filtered.last().unwrap();
    }
}

/// Read a WAV file as 16 kHz mono f32.
pub fn load_wav_16k_mono(path: &Path) -> anyhow::Result<Vec<f32>> {
    let mut r = hound::WavReader::open(path).with_context(|| format!("cannot open WAV {}", path.display()))?;
    let spec = r.spec();
    let ch = spec.channels.max(1) as usize;
    let interleaved: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().collect::<Result<_, _>>()?,
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1u64 << (spec.bits_per_sample - 1)) as f32;
            r.samples::<i32>().map(|s| s.map(|v| v as f32 * scale)).collect::<Result<_, _>>()?
        }
    };
    let mono: Vec<f32> = interleaved.chunks(ch).map(|f| f.iter().sum::<f32>() / ch as f32).collect();
    if spec.sample_rate == SAMPLE_RATE {
        return Ok(mono);
    }
    let mut rs = Resampler::new(spec.sample_rate, SAMPLE_RATE);
    let mut out = Vec::with_capacity(mono.len() * SAMPLE_RATE as usize / spec.sample_rate as usize + 1);
    rs.process(&mono, &mut out);
    Ok(out)
}

/// Feed a WAV into `sink` at real-time speed in 20 ms chunks.
pub(crate) fn spawn_wav_replay(
    path: &Path,
    sink: Arc<SharedAudio>,
    stop: Arc<AtomicBool>,
) -> anyhow::Result<JoinHandle<()>> {
    let pcm = load_wav_16k_mono(path)?;
    Ok(std::thread::Builder::new().name("easyvox-wav-replay".into()).spawn(move || {
        let chunk = (SAMPLE_RATE / 50) as usize; // 20 ms
        let t0 = Instant::now();
        for (i, c) in pcm.chunks(chunk).enumerate() {
            let due = t0 + Duration::from_millis(20 * (i as u64 + 1));
            while Instant::now() < due {
                if stop.load(Ordering::SeqCst) {
                    sink.finish();
                    return;
                }
                std::thread::sleep((due - Instant::now()).min(Duration::from_millis(5)));
            }
            sink.push(c);
        }
        sink.finish();
    })?)
}

/// Microphone authorization as reported by AVFoundation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MicPermission {
    NotDetermined,
    Restricted,
    Denied,
    Authorized,
    Unknown,
}

/// `[AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio]`. Never prompts.
#[cfg(target_os = "macos")]
pub fn mic_permission_status() -> MicPermission {
    use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};
    let Some(audio) = (unsafe { AVMediaTypeAudio }) else { return MicPermission::Unknown };
    match unsafe { AVCaptureDevice::authorizationStatusForMediaType(audio) } {
        AVAuthorizationStatus::NotDetermined => MicPermission::NotDetermined,
        AVAuthorizationStatus::Restricted => MicPermission::Restricted,
        AVAuthorizationStatus::Denied => MicPermission::Denied,
        AVAuthorizationStatus::Authorized => MicPermission::Authorized,
        _ => MicPermission::Unknown,
    }
}

/// Shows the system Microphone prompt if the status is `NotDetermined` (needs an app bundle
/// with `NSMicrophoneUsageDescription`), and blocks until the user answers or `timeout`.
/// Call it from onboarding or Settings, not on key-down.
#[cfg(target_os = "macos")]
pub fn request_mic_permission(timeout: Duration) -> MicPermission {
    use objc2_av_foundation::{AVCaptureDevice, AVMediaTypeAudio};
    if mic_permission_status() != MicPermission::NotDetermined {
        return mic_permission_status();
    }
    let Some(audio) = (unsafe { AVMediaTypeAudio }) else { return MicPermission::Unknown };
    let (tx, rx) = std::sync::mpsc::channel::<bool>();
    let handler = block2::RcBlock::new(move |granted: objc2::runtime::Bool| {
        let _ = tx.send(granted.as_bool());
    });
    unsafe { AVCaptureDevice::requestAccessForMediaType_completionHandler(audio, &handler) };
    let _ = rx.recv_timeout(timeout);
    mic_permission_status()
}

#[cfg(not(target_os = "macos"))]
pub fn request_mic_permission(_timeout: Duration) -> MicPermission {
    MicPermission::Unknown
}

#[cfg(not(target_os = "macos"))]
pub fn mic_permission_status() -> MicPermission {
    MicPermission::Unknown
}

/// What we learned opening the microphone (for diagnostics / the bench).
#[derive(Clone, Debug, Default)]
pub struct MicInfo {
    pub device: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub sample_format: String,
}

/// Open the default input device and stream 16 kHz mono into `sink` until `stop`.
/// Returns once the stream is running (or with the open error).
#[cfg(feature = "mic")]
pub(crate) fn spawn_mic(sink: Arc<SharedAudio>, stop: Arc<AtomicBool>) -> anyhow::Result<(JoinHandle<()>, MicInfo)> {
    use std::sync::mpsc;
    match mic_permission_status() {
        MicPermission::Denied | MicPermission::Restricted => {
            bail!("Microphone permission is denied for this app (System Settings > Privacy & Security > Microphone)")
        }
        // With the status undetermined, CoreAudio blocks AudioComponentInstanceNew for as
        // long as the permission is pending (observed: >25 s from a CLI). Ask first.
        MicPermission::NotDetermined if std::env::var_os("EASYVOX_MIC_FORCE_OPEN").is_none() => bail!(
            "Microphone permission has not been requested yet; call easyvox_core::request_mic_permission() first \
             (opening the stream now would block until someone answers the system prompt)"
        ),
        _ => {}
    }
    let (tx, rx) = mpsc::sync_channel::<anyhow::Result<MicInfo>>(1);
    let handle = std::thread::Builder::new().name("easyvox-mic".into()).spawn(move || {
        let stream = match open_mic(sink.clone()) {
            Ok((s, info)) => {
                let _ = tx.send(Ok(info));
                s
            }
            Err(e) => {
                let _ = tx.send(Err(e));
                sink.finish();
                return;
            }
        };
        while !stop.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(5));
        }
        drop(stream); // stops capture
        sink.finish();
    })?;
    let timeout: u64 = std::env::var("EASYVOX_MIC_OPEN_TIMEOUT_S").ok().and_then(|v| v.parse().ok()).unwrap_or(5);
    let info = rx.recv_timeout(Duration::from_secs(timeout)).with_context(|| {
        format!(
            "microphone did not open within {timeout} s (CoreAudio blocked; permission prompt pending or device hung?)"
        )
    })??;
    Ok((handle, info))
}

#[cfg(feature = "mic")]
fn open_mic(sink: Arc<SharedAudio>) -> anyhow::Result<(cpal::Stream, MicInfo)> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    let t0 = Instant::now();
    let step = |what: &str| log::debug!("mic: {:6} ms  {what}", t0.elapsed().as_millis());
    step("default_host");
    let host = cpal::default_host();
    let device = host.default_input_device().context("no default input device")?;
    step("default_input_device ok");
    let name = device.description().map(|d| d.name().to_string()).unwrap_or_else(|_| "<unknown>".into());
    step(&format!("device = {name}"));
    let supported =
        device.default_input_config().map_err(|e| anyhow::anyhow!("default_input_config failed on {name}: {e}"))?;
    step(&format!("default_input_config = {supported:?}"));
    let config = supported.config();
    let fmt = supported.sample_format();
    let ch = config.channels.max(1) as usize;
    let info = MicInfo {
        device: name.clone(),
        sample_rate: config.sample_rate,
        channels: config.channels,
        sample_format: format!("{fmt:?}"),
    };
    let err_sink = sink.clone();
    let err_fn = move |e: cpal::Error| {
        log::warn!("microphone stream error: {e}");
        err_sink.finish();
    };
    let mut rs = Resampler::new(config.sample_rate, SAMPLE_RATE);
    let mut out = Vec::new();
    let mut handle = move |mono: Vec<f32>| {
        out.clear();
        rs.process(&mono, &mut out);
        sink.push(&out);
    };
    let stream = match fmt {
        cpal::SampleFormat::F32 => device.build_input_stream::<f32, _, _>(
            config,
            move |d: &[f32], _| handle(d.chunks(ch).map(|f| f.iter().sum::<f32>() / ch as f32).collect()),
            err_fn,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream::<i16, _, _>(
            config,
            move |d: &[i16], _| {
                handle(d.chunks(ch).map(|f| f.iter().map(|&s| s as f32 / 32768.0).sum::<f32>() / ch as f32).collect())
            },
            err_fn,
            None,
        ),
        other => bail!("unsupported input sample format {other:?} on {name}"),
    }
    .map_err(|e| anyhow::anyhow!("build_input_stream failed on {name}: {e} (kind {:?})", e.kind()))?;
    step("build_input_stream ok");
    stream.play().map_err(|e| anyhow::anyhow!("starting microphone stream on {name} failed: {e}"))?;
    step("play ok");
    Ok((stream, info))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resampler_preserves_tone_and_length() {
        let sr = 48_000u32;
        let input: Vec<f32> =
            (0..sr).map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sr as f32).sin() * 0.5).collect();
        let mut rs = Resampler::new(sr, 16_000);
        let mut out = Vec::new();
        for c in input.chunks(480) {
            rs.process(c, &mut out);
        }
        assert!((out.len() as i64 - 16_000).abs() <= 2, "len {}", out.len());
        let peak = out[1000..].iter().fold(0f32, |m, s| m.max(s.abs()));
        assert!((peak - 0.5).abs() < 0.05, "peak {peak}");
    }
}
