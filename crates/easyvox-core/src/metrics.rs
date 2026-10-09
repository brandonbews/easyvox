//! Process memory/CPU counters (macOS `proc_pid_rusage` + `getrusage`).

#[derive(Clone, Copy, Debug, Default)]
pub struct ProcStats {
    /// Resident set size now.
    pub rss_bytes: u64,
    /// Physical footprint now (what Activity Monitor's "Memory" column shows; includes
    /// GPU/IOKit memory owned by the process).
    pub footprint_bytes: u64,
    /// Peak physical footprint over the process lifetime.
    pub peak_footprint_bytes: u64,
    /// Peak RSS (getrusage ru_maxrss).
    pub peak_rss_bytes: u64,
    /// CPU time consumed so far (user + system), seconds.
    pub cpu_secs: f64,
}

pub fn proc_stats() -> ProcStats {
    let mut s = ProcStats::default();
    unsafe {
        let mut ru: libc::rusage = std::mem::zeroed();
        if libc::getrusage(libc::RUSAGE_SELF, &mut ru) == 0 {
            let tv = |t: libc::timeval| t.tv_sec as f64 + t.tv_usec as f64 * 1e-6;
            s.cpu_secs = tv(ru.ru_utime) + tv(ru.ru_stime);
            // macOS reports ru_maxrss in bytes (Linux: KiB).
            s.peak_rss_bytes = if cfg!(target_os = "macos") { ru.ru_maxrss as u64 } else { ru.ru_maxrss as u64 * 1024 };
        }
        #[cfg(target_os = "macos")]
        {
            let mut info: libc::rusage_info_v4 = std::mem::zeroed();
            let p = &mut info as *mut libc::rusage_info_v4 as *mut libc::rusage_info_t;
            if libc::proc_pid_rusage(libc::getpid(), libc::RUSAGE_INFO_V4, p) == 0 {
                s.rss_bytes = info.ri_resident_size;
                s.footprint_bytes = info.ri_phys_footprint;
                s.peak_footprint_bytes = info.ri_lifetime_max_phys_footprint;
            }
        }
    }
    s
}
