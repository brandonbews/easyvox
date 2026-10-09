//! A tiny stderr logger for the `log` crate: `[   1.234] INFO  easyvox::controller: message`,
//! with seconds since launch. EasyVox's own crates log at `EASYVOX_LOG` (default `info`; try
//! `debug`); other crates log warnings and errors only.

use log::{LevelFilter, Log, Metadata, Record};
use std::sync::OnceLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

struct StderrLogger {
    start: Instant,
    level: LevelFilter,
}

static LOGGER: OnceLock<StderrLogger> = OnceLock::new();

impl Log for StderrLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        let max = if metadata.target().starts_with("easyvox") { self.level } else { self.level.min(LevelFilter::Warn) };
        metadata.level() <= max
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let secs = self.start.elapsed().as_secs_f64();
        // GPUI's `log_err` leaves the target empty outside its own repo, but gives the caller.
        let origin = match (record.target(), record.file(), record.line()) {
            ("", Some(file), Some(line)) => {
                let file = file.rsplit_once("/src/").map_or(file.to_string(), |(dir, rest)| {
                    format!("{}/src/{rest}", dir.rsplit('/').next().unwrap_or(dir))
                });
                format!("{file}:{line}")
            }
            (target, ..) => target.to_string(),
        };
        eprintln!("[{secs:8.3}] {:<5} {origin}: {}", record.level(), record.args());
    }

    fn flush(&self) {}
}

pub fn init() {
    let level = std::env::var("EASYVOX_LOG").ok().and_then(|v| v.parse().ok()).unwrap_or(LevelFilter::Info);
    let logger = LOGGER.get_or_init(|| StderrLogger { start: Instant::now(), level });
    if log::set_logger(logger).is_ok() {
        log::set_max_level(level);
    }
    let unix = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    log::info!("EasyVox {} starting (pid {}, unix time {unix})", env!("CARGO_PKG_VERSION"), std::process::id());
}
