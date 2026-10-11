"""wakes.py <pid> [seconds] [label]: rates of one process over a window, from proc_pid_rusage
(RUSAGE_INFO_V4; no sudo for your own processes): wake-ups per second (all interrupt/timer
wake-ups, and package-idle exits as top's IDLEW counts them), CPU as % of one core, footprint."""
import ctypes, sys, time

class RU(ctypes.Structure):
    _fields_ = [("uuid", ctypes.c_uint8 * 16)] + [(n, ctypes.c_uint64) for n in (
        "user", "system", "pkg_idle_wkups", "interrupt_wkups", "pageins", "wired", "resident",
        "phys_footprint", "start", "exit", "child_user", "child_system", "child_pkg_idle",
        "child_interrupt", "child_pageins", "child_elapsed", "disk_read", "disk_write",
        "cpu_time_qos_default", "q1", "q2", "q3", "q4", "q5", "q6", "q7", "billed_system_time",
        "serviced_system_time", "logical_writes", "lifetime_max_phys_footprint", "instructions",
        "cycles", "billed_energy", "serviced_energy", "interval_max_phys_footprint", "runnable_time")]

libc = ctypes.CDLL("/usr/lib/libSystem.B.dylib")
class TB(ctypes.Structure):
    _fields_ = [("numer", ctypes.c_uint32), ("denom", ctypes.c_uint32)]
tb = TB(); libc.mach_timebase_info(ctypes.byref(tb))

def sample(pid):
    ru = RU()
    if libc.proc_pid_rusage(pid, 4, ctypes.byref(ru)) != 0:
        raise SystemExit(f"proc_pid_rusage({pid}) failed")
    return ru, time.monotonic()

pid = int(sys.argv[1]); secs = float(sys.argv[2]) if len(sys.argv) > 2 else 30; label = sys.argv[3] if len(sys.argv) > 3 else ""
a, t0 = sample(pid); time.sleep(secs); b, t1 = sample(pid)
dt = t1 - t0
cpu_ns = ((b.user + b.system) - (a.user + a.system)) * tb.numer / tb.denom
print(f"{label:40s} pid {pid} over {dt:5.1f}s: wakeups {(b.interrupt_wkups - a.interrupt_wkups) / dt:7.1f}/s, "
      f"idle-exit wakeups {(b.pkg_idle_wkups - a.pkg_idle_wkups) / dt:6.1f}/s, cpu {100 * cpu_ns / 1e9 / dt:6.3f}% of a core, "
      f"footprint {b.phys_footprint / 1e6:6.1f} MB, resident {b.resident / 1e6:6.1f} MB")
