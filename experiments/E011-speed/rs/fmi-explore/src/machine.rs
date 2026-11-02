//! explore.py's machine: the thresholds and paths from the environment, MemAvailable, the disk's free space, and the
//! /proc helpers for the jobs' processes.

use std::fs;
use std::path::PathBuf;

fn env_float(name: &str, default: f64) -> f64 {
    match std::env::var(name) {
        Ok(v) if !v.is_empty() => v.parse().unwrap_or(default),
        _ => default,
    }
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/".into()))
}

/// explore.py `machine()`.
#[derive(Clone, Debug)]
pub struct Machine {
    pub mem_second_gb: f64,
    pub evict_gb: f64,
    pub floor_gb: f64,
    pub disk_path: String,
    pub guard: String,
    pub heavy_lock: String,
    pub poll_s: f64,
}

impl Machine {
    pub fn from_env() -> Machine {
        Machine {
            mem_second_gb: env_float("E008_MEM_SECOND_GB", 7.0),
            evict_gb: env_float("E008_EVICT_GB", 10.0),
            floor_gb: env_float("E008_FLOOR_GB", 8.0),
            disk_path: std::env::var("E008_DISK_PATH").ok().filter(|v| !v.is_empty()).unwrap_or_else(|| "/home".into()),
            guard: std::env::var("E008_GUARD")
                .ok()
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| home().join(".cache/fast-mi-e007/guard.sh").to_string_lossy().into_owned()),
            heavy_lock: std::env::var("E008_HEAVY_LOCK")
                .ok()
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| "/var/tmp/fast-mi/heavy.lock".into()),
            poll_s: env_float("E008_POLL_S", 1.0),
        }
    }
}

pub fn mem_available_kb() -> u64 {
    fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|t| {
            t.lines().find(|l| l.starts_with("MemAvailable:")).and_then(|l| l.split_whitespace().nth(1)?.parse().ok())
        })
        .unwrap_or(0)
}

pub fn mem_available_gb() -> f64 {
    mem_available_kb() as f64 / (1u64 << 20) as f64
}

/// `shutil.disk_usage(path).free`, in bytes.
pub fn disk_free(path: &str) -> f64 {
    let c = std::ffi::CString::new(path).unwrap();
    unsafe {
        let mut s: libc::statvfs = std::mem::zeroed();
        if libc::statvfs(c.as_ptr(), &mut s) != 0 {
            return 0.0;
        }
        s.f_bavail as f64 * s.f_frsize as f64
    }
}

/// /proc/PID/stat's fields from the third (state) on.
pub fn proc_stat(pid: i32) -> Option<Vec<String>> {
    let s = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let i = s.rfind(')')?;
    Some(s.get(i + 2..)?.split_whitespace().map(|x| x.to_string()).collect())
}

pub fn proc_start(pid: i32) -> Option<u64> {
    proc_stat(pid)?.get(19)?.parse().ok()
}

pub fn proc_alive(pid: i32, start: Option<u64>) -> bool {
    match proc_stat(pid) {
        Some(st) => st[0] != "Z" && start.map(|s| st.get(19).and_then(|x| x.parse::<u64>().ok()) == Some(s)).unwrap_or(true),
        None => false,
    }
}

/// The process and its descendants (explore.py `proc_tree`).
pub fn proc_tree(pid: i32) -> Vec<i32> {
    let mut kids: std::collections::HashMap<i32, Vec<i32>> = std::collections::HashMap::new();
    if let Ok(rd) = fs::read_dir("/proc") {
        for e in rd.flatten() {
            if let Ok(p) = e.file_name().to_string_lossy().parse::<i32>() {
                if let Some(st) = proc_stat(p) {
                    if let Some(pp) = st.get(1).and_then(|x| x.parse::<i32>().ok()) {
                        kids.entry(pp).or_default().push(p);
                    }
                }
            }
        }
    }
    let (mut out, mut todo) = (Vec::new(), vec![pid]);
    while let Some(p) = todo.pop() {
        out.push(p);
        if let Some(k) = kids.get(&p) {
            todo.extend(k.iter().copied());
        }
    }
    out
}

/// explore.py `kill_tree`: TERM then KILL, each with a grace.
pub fn kill_tree(pid: i32, grace_s: f64) {
    let tree = proc_tree(pid);
    for sig in [libc::SIGTERM, libc::SIGKILL] {
        for &p in tree.iter().rev() {
            unsafe {
                libc::kill(p, sig);
            }
        }
        let t = std::time::Instant::now();
        while t.elapsed().as_secs_f64() < grace_s && tree.iter().any(|&p| proc_alive(p, None)) {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
}

/// The descendants of this process (not itself), deepest last.
pub fn my_descendants() -> Vec<i32> {
    let me = std::process::id() as i32;
    proc_tree(me).into_iter().filter(|&p| p != me).collect()
}

/// CPU seconds of the calling thread.
pub fn thread_cpu_s() -> f64 {
    unsafe {
        let mut ts: libc::timespec = std::mem::zeroed();
        libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts);
        ts.tv_sec as f64 + ts.tv_nsec as f64 * 1e-9
    }
}

/// CPU seconds of this process (utime + stime) and of its ended children.
pub fn process_cpu_s() -> f64 {
    unsafe {
        let mut a: libc::rusage = std::mem::zeroed();
        let mut b: libc::rusage = std::mem::zeroed();
        libc::getrusage(libc::RUSAGE_SELF, &mut a);
        libc::getrusage(libc::RUSAGE_CHILDREN, &mut b);
        let t = |r: &libc::rusage| {
            r.ru_utime.tv_sec as f64 + r.ru_utime.tv_usec as f64 * 1e-6 + r.ru_stime.tv_sec as f64 + r.ru_stime.tv_usec as f64 * 1e-6
        };
        t(&a) + t(&b)
    }
}

/// Peak RSS of this process and of its ended children, in MB (getrusage's maxrss // 1024).
pub fn peaks_mb() -> (i64, i64) {
    unsafe {
        let mut a: libc::rusage = std::mem::zeroed();
        let mut b: libc::rusage = std::mem::zeroed();
        libc::getrusage(libc::RUSAGE_SELF, &mut a);
        libc::getrusage(libc::RUSAGE_CHILDREN, &mut b);
        (a.ru_maxrss / 1024, b.ru_maxrss / 1024)
    }
}

/// CPU seconds of a live process (/proc/PID/stat utime + stime).
pub fn cpu_of(pid: i32) -> f64 {
    let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) } as f64;
    proc_stat(pid)
        .and_then(|st| {
            let u: f64 = st.get(11)?.parse().ok()?;
            let s: f64 = st.get(12)?.parse().ok()?;
            Some((u + s) / hz)
        })
        .unwrap_or(0.0)
}

/// nice 10 for this process when it runs at a lower nice value (the guard's and the plan's rule for runs).
pub fn nice10() {
    unsafe {
        *libc::__errno_location() = 0;
        let cur = libc::getpriority(libc::PRIO_PROCESS, 0);
        if cur < 10 {
            libc::setpriority(libc::PRIO_PROCESS, 0, 10);
        }
    }
}

thread_local! {
    static PORT_S: std::cell::Cell<f64> = const { std::cell::Cell::new(0.0) };
}

/// Runs `f` (a call into another component's port) and adds its wall time to this thread's port clock: a try's own
/// time is its wall less this (the plan's "time a try spends outside the ports").
pub fn port<T>(f: impl FnOnce() -> T) -> T {
    let t = std::time::Instant::now();
    let r = f();
    PORT_S.with(|c| c.set(c.get() + t.elapsed().as_secs_f64()));
    r
}

/// This thread's port clock, and a reset.
pub fn port_s() -> f64 {
    PORT_S.with(|c| c.get())
}

pub fn port_reset() {
    PORT_S.with(|c| c.set(0.0));
}

thread_local! {
    static SECTIONS: std::cell::RefCell<Vec<(&'static str, f64, u32)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Runs `f` and adds its wall time to this thread's named section (FMI_TRY_PROFILE's finer account).
pub fn section<T>(name: &'static str, f: impl FnOnce() -> T) -> T {
    let t = std::time::Instant::now();
    let r = f();
    let dt = t.elapsed().as_secs_f64();
    SECTIONS.with(|s| {
        let mut s = s.borrow_mut();
        match s.iter_mut().find(|x| x.0 == name) {
            Some(x) => {
                x.1 += dt;
                x.2 += 1;
            }
            None => s.push((name, dt, 1)),
        }
    });
    r
}

/// This thread's sections so far, and a reset.
pub fn sections_take() -> Vec<(&'static str, f64, u32)> {
    SECTIONS.with(|s| std::mem::take(&mut *s.borrow_mut()))
}
