//! The reader in the calling process (E012's component `resident`, E012 plan.md "Amendment 2026-11-03, component
//! `resident`"; E011's round 3 item for the reading): reader-scope's reading as a library call
//! (`scumm_reader::run::run`) with reader-scope's arguments, the game loaded once per process, and the job memo's
//! store kept in memory from reading to reading (memo.rs `MemStore`, its oldest files spilling to the `--memo` folder
//! when one is given). The reading's files and report are reader-scope's, byte for byte (the same code); its progress
//! goes to the folder's reader-scope.err.
//!
//! The switch is `FMI_READER_LIB=1` (off by default: the caller runs reader-scope as a child process). With it:
//! * [`InProc::from_env`]: the store's cap `FMI_READER_MEMO_GB` (default 1.5; 0: no store in memory), its spill
//!   folder the caller's `--memo` folder (`FMI_MEMO`);
//! * [`InProc::extra_args`]: what the switch adds to reader-scope's arguments: `--memo-mem` (the store in memory,
//!   when no `--memo` folder is given), `--memo-units-set` (memo.md's amendment of 2026-11-03; `FMI_READER_UNITS_SET=0`
//!   leaves it out) and `--prep` (`FMI_READER_PREP=0` leaves it out);
//! * [`ReaderAlloc`]: the process's allocator, mimalloc (reader-scope's own) with the switch on, the system allocator
//!   with it off, chosen at the process's first allocation.

use scumm_reader::memo::MemStore;
use scumm_reader::run::{run, Ctl, Log, Resident};
use std::alloc::{GlobalAlloc, Layout, System};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

/// The switch (module docstring).
pub const SWITCH: &str = "FMI_READER_LIB";

/// Whether the switch is on.
pub fn on() -> bool {
    std::env::var(SWITCH).is_ok_and(|v| v == "1")
}

/// The reader in this process (module docstring).
pub struct InProc {
    root: PathBuf,
    res: OnceLock<Result<Resident, String>>,
    /// the store in memory: its cap in bytes (None: no store in memory) and its spill folder
    mem_cap: Option<u64>,
    spill: Option<PathBuf>,
    /// `--memo-units-set` and `--prep` added to the arguments
    units_set: bool,
    prep: bool,
}

impl InProc {
    /// The game under `root`, loaded at the first reading; `mem_cap` and `spill`: the memo's store in memory.
    pub fn new(root: &Path, mem_cap: Option<u64>, spill: Option<PathBuf>, units_set: bool, prep: bool) -> InProc {
        InProc { root: root.to_path_buf(), res: OnceLock::new(), mem_cap, spill, units_set, prep }
    }

    /// With the switch on, the reader in this process as the environment sets it (module docstring); `spill`: the
    /// caller's `--memo` folder (FMI_MEMO), if any. None with the switch off.
    pub fn from_env(root: &Path, spill: Option<PathBuf>) -> Option<InProc> {
        if !on() {
            return None;
        }
        let gb: f64 = std::env::var("FMI_READER_MEMO_GB").ok().and_then(|g| g.parse().ok()).unwrap_or(1.5);
        let cap = (gb > 0.0).then(|| (gb * (1u64 << 30) as f64) as u64);
        let off = |k: &str| std::env::var(k).is_ok_and(|v| v == "0");
        Some(InProc::new(root, cap, spill, !off("FMI_READER_UNITS_SET"), !off("FMI_READER_PREP")))
    }

    /// What the switch adds to reader-scope's arguments; `memo_dir`: whether the caller gives `--memo DIR`.
    pub fn extra_args(&self, memo_dir: bool) -> Vec<String> {
        let mut a = Vec::new();
        let memo = memo_dir || self.mem_cap.is_some();
        if !memo_dir && self.mem_cap.is_some() {
            a.push("--memo-mem".to_string());
        }
        if memo && self.units_set {
            a.push("--memo-units-set".to_string());
        }
        if self.prep {
            a.push("--prep".to_string());
        }
        a
    }

    /// Whether `--prep` is among the added arguments.
    pub fn prep(&self) -> bool {
        self.prep
    }

    fn resident(&self) -> Result<&Resident, String> {
        self.res
            .get_or_init(|| {
                let mut r = Resident::load(&self.root).map_err(|e| format!("the game: {e}"))?;
                r.memo = self.mem_cap.map(|cap| Arc::new(MemStore::new(cap, self.spill.clone())));
                Ok(r)
            })
            .as_ref()
            .map_err(|e| e.clone())
    }

    /// The bytes the memo's store holds in memory.
    pub fn memo_bytes(&self) -> u64 {
        self.res.get().and_then(|r| r.as_ref().ok()).and_then(|r| r.memo.as_ref()).map_or(0, |m| m.bytes())
    }

    /// One reading by reader-scope's arguments `args` (with [`InProc::extra_args`] already in them), its progress in
    /// `dir`/reader-scope.err; `cancel` stops it. Returns the report.
    pub fn read(&self, args: &[String], dir: &Path, cancel: &AtomicBool) -> Result<serde_json::Value, String> {
        let res = self.resident()?;
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let log = Log::file(&dir.join("reader-scope.err")).map_err(|e| format!("{}: {e}", dir.display()))?;
        let abort = Arc::new(AtomicBool::new(false));
        let ctl = Ctl::new(log, abort.clone());
        let done = AtomicBool::new(false);
        let out = std::thread::scope(|s| {
            // the caller's stop, passed on to the reading's own flag
            s.spawn(|| {
                while !done.load(Ordering::Relaxed) {
                    if cancel.load(Ordering::Relaxed) {
                        abort.store(true, Ordering::Relaxed);
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
            });
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(args, res, Some(&ctl), Instant::now())));
            done.store(true, Ordering::Relaxed);
            r
        });
        match out {
            Ok(report) => Ok(report),
            Err(e) => {
                let why = ctl.stopped.lock().unwrap().clone();
                let msg = e
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_else(|| "a panic".into());
                Err(match why {
                    Some(w) => format!("the reading stopped: {w}"),
                    None => format!("the reading failed: {msg}"),
                })
            }
        }
    }
}

/// The process's allocator (module docstring): mimalloc when the switch is on at the first allocation, else the
/// system allocator. The choice is made once, before anything is allocated, so every block is freed by the allocator
/// that made it. A binary takes it with `#[global_allocator] static A: fmi_reading::inproc::ReaderAlloc =
/// fmi_reading::inproc::ReaderAlloc;`.
pub struct ReaderAlloc;

/// 0 not chosen yet, 1 the system's, 2 mimalloc
static CHOICE: AtomicU8 = AtomicU8::new(0);

#[inline]
fn mimalloc_on() -> bool {
    match CHOICE.load(Ordering::Relaxed) {
        1 => false,
        2 => true,
        _ => choose(),
    }
}

#[cold]
fn choose() -> bool {
    // getenv allocates nothing; the process's first allocation comes before any other thread
    // SAFETY: a NUL-terminated name; the value, if any, is a NUL-terminated string the C library owns
    let on = unsafe {
        let p = libc::getenv(c"FMI_READER_LIB".as_ptr());
        !p.is_null() && std::ffi::CStr::from_ptr(p).to_bytes() == b"1"
    };
    CHOICE.store(if on { 2 } else { 1 }, Ordering::Relaxed);
    on
}

/// Which allocator the process took: "mimalloc" or "system" (for reports).
pub fn allocator() -> &'static str {
    if mimalloc_on() {
        "mimalloc"
    } else {
        "system"
    }
}

// SAFETY: each call goes to one allocator, the same for the process's whole life (`choose` runs once, before the
// first block), so a block is always freed, grown or shrunk by the allocator that made it
unsafe impl GlobalAlloc for ReaderAlloc {
    #[inline]
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if mimalloc_on() {
            mimalloc::MiMalloc.alloc(l)
        } else {
            System.alloc(l)
        }
    }
    #[inline]
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        if mimalloc_on() {
            mimalloc::MiMalloc.alloc_zeroed(l)
        } else {
            System.alloc_zeroed(l)
        }
    }
    #[inline]
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        if mimalloc_on() {
            mimalloc::MiMalloc.dealloc(p, l)
        } else {
            System.dealloc(p, l)
        }
    }
    #[inline]
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        if mimalloc_on() {
            mimalloc::MiMalloc.realloc(p, l, n)
        } else {
            System.realloc(p, l, n)
        }
    }
}

/// CPU seconds (user + system) of this process's threads, and of its children waited for (getrusage).
pub fn rusage() -> (f64, f64) {
    let secs = |who: i32| {
        // SAFETY: getrusage fills the struct it is given
        let mut u: libc::rusage = unsafe { std::mem::zeroed() };
        unsafe { libc::getrusage(who, &mut u) };
        let tv = |t: libc::timeval| t.tv_sec as f64 + t.tv_usec as f64 / 1e6;
        tv(u.ru_utime) + tv(u.ru_stime)
    };
    (secs(libc::RUSAGE_SELF), secs(libc::RUSAGE_CHILDREN))
}

/// The largest resident set of a child waited for, in MB (getrusage's ru_maxrss).
pub fn children_maxrss_mb() -> u64 {
    // SAFETY: getrusage fills the struct it is given
    let mut u: libc::rusage = unsafe { std::mem::zeroed() };
    unsafe { libc::getrusage(libc::RUSAGE_CHILDREN, &mut u) };
    u.ru_maxrss as u64 / 1024
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the switch adds to reader-scope's arguments: the store in memory alone unless a --memo folder is given,
    /// the key's units as a set whenever a memo is on, --prep unless left out.
    #[test]
    fn the_switchs_arguments() {
        let r = Path::new("/");
        let ip = |cap: Option<u64>, units: bool, prep: bool| InProc::new(r, cap, None, units, prep);
        assert_eq!(ip(Some(1), true, true).extra_args(false), ["--memo-mem", "--memo-units-set", "--prep"]);
        assert_eq!(ip(Some(1), true, true).extra_args(true), ["--memo-units-set", "--prep"]);
        assert_eq!(ip(None, true, true).extra_args(true), ["--memo-units-set", "--prep"]);
        assert_eq!(ip(None, true, true).extra_args(false), ["--prep"]);
        assert_eq!(ip(Some(1), false, false).extra_args(false), ["--memo-mem"]);
        assert!(ip(None, true, false).extra_args(false).is_empty());
    }
}
