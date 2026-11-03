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
//! * [`ReaderAlloc`]: the process's allocator, chosen at its first allocation: with the switch on, mimalloc
//!   (reader-scope's own) for the blocks made while a reading runs and the system's for the others (`phase`, the
//!   default; `FMI_READER_ALLOC=mimalloc` or `system` for one of them throughout); with it off, the system's.

use scumm_reader::memo::MemStore;
use scumm_reader::run::{run, Ctl, Log, Resident};
use std::alloc::{GlobalAlloc, Layout, System};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
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
        // by phase (`ReaderAlloc`): the blocks made from here to the reading's end come from mimalloc
        let _reading = Reading::enter();
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

/// The process's allocator (module docstring; E012 plan.md, resident, "the allocator by phase"). Its mode is chosen
/// once, at the process's first allocation, from the environment:
/// * the switch off, or `FMI_READER_ALLOC=system`: the system allocator for every block;
/// * `FMI_READER_ALLOC=mimalloc`: mimalloc for every block;
/// * otherwise (`phase`, the switch's default): mimalloc for the blocks made while a reading runs in the process
///   ([`InProc::read`]), the system allocator for the others.
///
/// A block is always freed, grown or shrunk by the allocator that made it: in the first two modes there is one; by
/// phase, mimalloc's page map (`mi_is_in_heap_region`) tells its blocks, which lie in the regions it reserved, from
/// the system's. A binary takes it with `#[global_allocator] static A: fmi_reading::inproc::ReaderAlloc =
/// fmi_reading::inproc::ReaderAlloc;`.
pub struct ReaderAlloc;

const SYSTEM: u8 = 1;
const MIMALLOC: u8 = 2;
const PHASE: u8 = 3;

/// 0 not chosen yet, else one of the modes above
static CHOICE: AtomicU8 = AtomicU8::new(0);
/// the readings running in this process (by phase: new blocks from mimalloc while it is above 0)
static READING: AtomicUsize = AtomicUsize::new(0);
/// mimalloc has made a block (before that, no block is its)
static MI_USED: AtomicBool = AtomicBool::new(false);

extern "C" {
    /// mimalloc's own test: the address lies in a region mimalloc reserved (its page map); false before its first block
    fn mi_is_in_heap_region(p: *const std::ffi::c_void) -> bool;
}

#[inline]
fn mode() -> u8 {
    match CHOICE.load(Ordering::Relaxed) {
        0 => choose(),
        m => m,
    }
}

#[cold]
fn choose() -> u8 {
    // getenv allocates nothing; the process's first allocation comes before any other thread
    let env = |name: &std::ffi::CStr| -> Option<&'static [u8]> {
        // SAFETY: a NUL-terminated name; the value, if any, is a NUL-terminated string the C library owns and keeps
        unsafe {
            let p = libc::getenv(name.as_ptr());
            (!p.is_null()).then(|| std::ffi::CStr::from_ptr(p).to_bytes())
        }
    };
    let m = if env(c"FMI_READER_LIB") != Some(b"1") {
        SYSTEM
    } else {
        match env(c"FMI_READER_ALLOC") {
            Some(b"system") => SYSTEM,
            Some(b"mimalloc") => MIMALLOC,
            _ => PHASE,
        }
    };
    CHOICE.store(m, Ordering::Relaxed);
    m
}

/// Which allocator the process takes: "system", "mimalloc" or "phase" (for reports).
pub fn allocator() -> &'static str {
    match mode() {
        MIMALLOC => "mimalloc",
        PHASE => "phase",
        _ => "system",
    }
}

/// A reading runs in the process while this lives (by phase: its blocks come from mimalloc).
pub struct Reading(());

impl Reading {
    pub fn enter() -> Reading {
        READING.fetch_add(1, Ordering::SeqCst);
        Reading(())
    }
}

impl Drop for Reading {
    fn drop(&mut self) {
        READING.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Whether a new block comes from mimalloc, in mode `m`.
#[inline]
fn make_mi(m: u8) -> bool {
    m == MIMALLOC || (m == PHASE && READING.load(Ordering::Relaxed) > 0)
}

/// Whether block `p` is mimalloc's, in mode `m`.
#[inline]
unsafe fn mi_owns(m: u8, p: *mut u8) -> bool {
    m == MIMALLOC || (m == PHASE && MI_USED.load(Ordering::Relaxed) && mi_is_in_heap_region(p as *const std::ffi::c_void))
}

#[inline]
fn mark_mi() {
    if !MI_USED.load(Ordering::Relaxed) {
        MI_USED.store(true, Ordering::Relaxed);
    }
}

// SAFETY: a block is made by one allocator and always freed, grown or shrunk by the one that made it (the type's
// docstring): the mode never changes after the first block, and by phase the owner is mimalloc's own answer
unsafe impl GlobalAlloc for ReaderAlloc {
    #[inline]
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if make_mi(mode()) {
            mark_mi();
            mimalloc::MiMalloc.alloc(l)
        } else {
            System.alloc(l)
        }
    }
    #[inline]
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        if make_mi(mode()) {
            mark_mi();
            mimalloc::MiMalloc.alloc_zeroed(l)
        } else {
            System.alloc_zeroed(l)
        }
    }
    #[inline]
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        if mi_owns(mode(), p) {
            mimalloc::MiMalloc.dealloc(p, l)
        } else {
            System.dealloc(p, l)
        }
    }
    #[inline]
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        // a block stays with the allocator that made it
        if mi_owns(mode(), p) {
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

    /// By phase, a block made during a reading is mimalloc's and one made outside is the system's, and each is freed
    /// and grown by its maker whenever and wherever that happens.
    #[test]
    fn the_allocator_by_phase() {
        CHOICE.store(PHASE, Ordering::SeqCst);
        let a = ReaderAlloc;
        let l = Layout::from_size_align(4096, 16).unwrap();
        // SAFETY: blocks of layout `l`, each freed once by the same GlobalAlloc
        unsafe {
            let outside = a.alloc(l);
            let (during, during2) = {
                let _r = Reading::enter();
                (a.alloc(l), a.alloc_zeroed(l))
            };
            assert!(!mi_is_in_heap_region(outside as _), "made outside a reading: the system's");
            assert!(mi_is_in_heap_region(during as _) && mi_is_in_heap_region(during2 as _), "made during one: mimalloc's");
            // grown outside a reading, a mimalloc block stays mimalloc's; a system block grown during one stays the system's
            let grown = a.realloc(during, l, 1 << 20);
            assert!(mi_is_in_heap_region(grown as _));
            let sys_grown = {
                let _r = Reading::enter();
                a.realloc(outside, l, 1 << 20)
            };
            assert!(!mi_is_in_heap_region(sys_grown as _));
            let big = Layout::from_size_align(1 << 20, 16).unwrap();
            a.dealloc(grown, big);
            a.dealloc(sys_grown, big);
            // freed on another thread than the one that made it
            let p = during2 as usize;
            std::thread::spawn(move || a.dealloc(p as *mut u8, l)).join().unwrap();
        }
    }

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
