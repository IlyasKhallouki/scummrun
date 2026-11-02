//! The few system calls std has no wrapper for (poll), and /proc readers.

use std::os::fd::RawFd;
use std::time::Duration;

#[repr(C)]
struct PollFd {
    fd: i32,
    events: i16,
    revents: i16,
}

extern "C" {
    fn poll(fds: *mut PollFd, nfds: u64, timeout: i32) -> i32;
}

const POLLIN: i16 = 1;

/// Whether `fd` becomes readable (or a listener has a connection) within `timeout`.
pub fn readable(fd: RawFd, timeout: Duration) -> bool {
    let mut p = PollFd {
        fd,
        events: POLLIN,
        revents: 0,
    };
    let ms = timeout.as_millis().min(i32::MAX as u128) as i32;
    loop {
        // SAFETY: one valid pollfd for the duration of the call.
        let r = unsafe { poll(&mut p, 1, ms) };
        if r < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
            continue;
        }
        return r > 0;
    }
}

/// CPU seconds (utime + stime) of a process from /proc/PID/stat; 0 when it is gone.
pub fn cpu_s(pid: u32) -> f64 {
    let Ok(s) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
        return 0.0;
    };
    // the command name can hold spaces: fields count from the last ')'
    let Some(rest) = s.rfind(')').map(|i| &s[i + 2..]) else {
        return 0.0;
    };
    let f: Vec<&str> = rest.split(' ').collect();
    // after ')' the fields start at 3 (state): utime is field 14, stime 15
    let t = |i: usize| f.get(i - 3).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
    (t(14) + t(15)) as f64 / clk_tck()
}

fn clk_tck() -> f64 {
    extern "C" {
        fn sysconf(name: i32) -> i64;
    }
    const SC_CLK_TCK: i32 = 2;
    // SAFETY: sysconf has no preconditions.
    let v = unsafe { sysconf(SC_CLK_TCK) };
    if v > 0 {
        v as f64
    } else {
        100.0
    }
}

/// /proc/meminfo's MemAvailable in kB (None if unreadable).
pub fn mem_available_kb() -> Option<u64> {
    let s = std::fs::read_to_string("/proc/meminfo").ok()?;
    s.lines()
        .find(|l| l.starts_with("MemAvailable:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}

/// This process's own CPU seconds (getrusage RUSAGE_SELF).
pub fn self_cpu_s() -> f64 {
    cpu_s(std::process::id())
}
