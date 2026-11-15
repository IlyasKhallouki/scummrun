//! E014 amendment (4) (a) (experiments/E014-fate-demo/plan.md): the bound on a try's fresh reading. The reading runs
//! on a thread of its own and the try waits for it at most `FMI_READ_BOUND` seconds of wall; a watchdog with a
//! fallback (the try goes on model-free, tryjob.rs). At the bound the reading is stopped through its own stop flag
//! (the in-process reader's), or, while fewer than `FMI_READ_BOUND_BG` readings run in the background in this
//! process, it goes on there and its files reach the reading cache when it ends, where a later try that asks for the
//! same reading finds it. A try that asks for a reading still running here waits for that one instead of starting
//! another; a reading stopped at the bound is bounded at once when asked for again in this process.
//!
//! Both switches unset: nothing here runs, and the try reads on its own thread as before.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

pub const BOUND_ENV: &str = "FMI_READ_BOUND";
pub const BG_ENV: &str = "FMI_READ_BOUND_BG";

/// The bound in seconds and the most readings left running in the background at once.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cfg {
    pub s: f64,
    pub bg: usize,
}

/// The switches: `FMI_READ_BOUND=S` (S > 0) and `FMI_READ_BOUND_BG=N` (default 0). None when the bound is off.
pub fn cfg() -> Option<Cfg> {
    let s: f64 = std::env::var(BOUND_ENV).ok().and_then(|v| v.trim().parse().ok()).filter(|s: &f64| *s > 0.0)?;
    let bg: usize = std::env::var(BG_ENV).ok().and_then(|v| v.trim().parse().ok()).unwrap_or(0);
    Some(Cfg { s, bg })
}

/// Whether the bound is on.
pub fn on() -> bool {
    cfg().is_some()
}

/// A reading's outcome: its reading.json, or the reader's error.
pub type Out = Result<PathBuf, String>;

/// One reading running on its thread.
struct Flight {
    res: Mutex<Option<Out>>,
    cv: Condvar,
    stop: Arc<AtomicBool>,
    background: AtomicBool,
}

#[derive(Default)]
struct Reg {
    inflight: HashMap<String, Arc<Flight>>,
    stopped: HashSet<String>,
    /// the readings left running in the background now
    bg: usize,
}

/// What a bounded reading gave the try.
#[derive(Clone, Debug, PartialEq)]
pub enum Got {
    /// the reading ended within the bound (`joined`: it was one already running, started by an earlier try)
    Done { out: Out, waited: f64, joined: bool },
    /// the bound: the reading went on in the background, was stopped, or (`at_once`) was stopped before in this
    /// process and is not started again
    Bound { waited: f64, background: bool, at_once: bool, joined: bool },
    /// the try's own stop (the guard's kill) while it waited; the reading was stopped
    Cancelled { waited: f64 },
}

impl Got {
    /// result.json's record of a bounded reading ("bound").
    pub fn record(&self, s: f64) -> serde_json::Value {
        match self {
            Got::Bound { waited, background, at_once, joined } => serde_json::json!({
                "s": s, "waited_s": (waited * 100.0).round() / 100.0, "background": background, "at_once": at_once,
                "joined": joined}),
            Got::Done { waited, joined, .. } => serde_json::json!({"s": s, "waited_s": (waited * 100.0).round() / 100.0,
                "joined": joined, "ended": true}),
            Got::Cancelled { waited } => serde_json::json!({"s": s, "waited_s": (waited * 100.0).round() / 100.0, "cancelled": true}),
        }
    }
}

/// The readings of one process (the driver's: [`bounder`]; the tests make their own).
#[derive(Default)]
pub struct Bounder {
    reg: Mutex<Reg>,
}

/// This process's bounder.
pub fn bounder() -> &'static Bounder {
    static B: OnceLock<Bounder> = OnceLock::new();
    B.get_or_init(Bounder::default)
}

/// A reading's identity in the process: its scope, goal and start row (the same as the reading cache's key for one
/// reader and one game).
pub fn ident(rooms: &[i64], goal: &str, start_row: &str) -> String {
    let mut h = sha1_smol::Sha1::new();
    h.update(start_row.as_bytes());
    format!("{rooms:?}|{goal}|{}", h.digest())
}

/// How often a waiting try looks at its own stop flag.
const LOOK: Duration = Duration::from_millis(200);

impl Bounder {
    /// The readings left running in the background now.
    pub fn background(&self) -> usize {
        self.reg.lock().unwrap().bg
    }

    /// The reading `ident`, made by `start` on a thread of its own (given its stop flag) unless the same is running
    /// already, waited for at most `cfg.s` seconds; `cancel`: the try's own stop.
    /// The bounder is 'static (the process's, or a test's leaked one): a reading's thread may outlive the try.
    pub fn read(&'static self, ident: &str, cfg: Cfg, cancel: &AtomicBool, start: impl FnOnce(&AtomicBool) -> Out + Send + 'static) -> Got {
        let t0 = Instant::now();
        let (fl, joined) = {
            let mut g = self.reg.lock().unwrap();
            if g.stopped.contains(ident) {
                return Got::Bound { waited: 0.0, background: false, at_once: true, joined: false };
            }
            match g.inflight.get(ident) {
                Some(f) => (f.clone(), true),
                None => {
                    let f = Arc::new(Flight { res: Mutex::new(None), cv: Condvar::new(), stop: Arc::new(AtomicBool::new(false)),
                                              background: AtomicBool::new(false) });
                    g.inflight.insert(ident.to_string(), f.clone());
                    (f, false)
                }
            }
        };
        if !joined {
            let f = fl.clone();
            let id = ident.to_string();
            // the thread outlives the try when the reading goes on in the background
            let me: &'static Bounder = self;
            let spawned = std::thread::Builder::new()
                .name("bounded reading".into())
                .stack_size(64 << 20)
                .spawn(move || {
                    let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| start(&f.stop)))
                        .unwrap_or_else(|_| Err("the reading failed: a panic".into()));
                    {
                        let mut r = f.res.lock().unwrap();
                        *r = Some(out);
                        f.cv.notify_all();
                    }
                    let mut g = me.reg.lock().unwrap();
                    if g.inflight.get(&id).map(|x| Arc::ptr_eq(x, &f)).unwrap_or(false) {
                        g.inflight.remove(&id);
                    }
                    if f.background.load(Ordering::SeqCst) {
                        g.bg = g.bg.saturating_sub(1);
                    }
                });
            if let Err(e) = spawned {
                self.reg.lock().unwrap().inflight.remove(ident);
                return Got::Done { out: Err(format!("a reading thread: {e}")), waited: 0.0, joined: false };
            }
        }
        // wait for it, at most the bound, looking at the try's own stop
        let deadline = t0 + Duration::from_secs_f64(cfg.s);
        {
            let mut r = fl.res.lock().unwrap();
            loop {
                if let Some(out) = r.as_ref() {
                    return Got::Done { out: out.clone(), waited: t0.elapsed().as_secs_f64(), joined };
                }
                let now = Instant::now();
                if now >= deadline {
                    break;
                }
                if cancel.load(Ordering::SeqCst) {
                    drop(r);
                    if !fl.background.load(Ordering::SeqCst) {
                        fl.stop.store(true, Ordering::SeqCst);
                        self.reg.lock().unwrap().inflight.remove(ident);
                    }
                    return Got::Cancelled { waited: t0.elapsed().as_secs_f64() };
                }
                r = fl.cv.wait_timeout(r, (deadline - now).min(LOOK)).unwrap().0;
            }
        }
        // the bound: the background, or the stop
        let mut g = self.reg.lock().unwrap();
        if let Some(out) = fl.res.lock().unwrap().as_ref() {
            return Got::Done { out: out.clone(), waited: t0.elapsed().as_secs_f64(), joined };
        }
        let waited = t0.elapsed().as_secs_f64();
        if fl.background.load(Ordering::SeqCst) {
            return Got::Bound { waited, background: true, at_once: false, joined };
        }
        if g.bg < cfg.bg {
            g.bg += 1;
            fl.background.store(true, Ordering::SeqCst);
            return Got::Bound { waited, background: true, at_once: false, joined };
        }
        // stopped: the reader stops at its next look at the flag (after the jobs its workers run); its files are
        // never used
        fl.stop.store(true, Ordering::SeqCst);
        g.inflight.remove(ident);
        g.stopped.insert(ident.to_string());
        Got::Bound { waited, background: false, at_once: false, joined }
    }
}

impl Bounder {
    /// A bounder of its own that lives as long as the process (the tests').
    pub fn leaked() -> &'static Bounder {
        Box::leak(Box::new(Bounder::default()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    fn sleeper(secs: f64, ran: Arc<AtomicUsize>) -> impl FnOnce(&AtomicBool) -> Out + Send + 'static {
        move |stop: &AtomicBool| {
            ran.fetch_add(1, Ordering::SeqCst);
            let t = Instant::now();
            while t.elapsed().as_secs_f64() < secs {
                if stop.load(Ordering::SeqCst) {
                    return Err("the reading stopped: stop".into());
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Ok(PathBuf::from(format!("/r/{secs}")))
        }
    }

    fn wait_until(f: impl Fn() -> bool) {
        let t = Instant::now();
        while !f() {
            assert!(t.elapsed().as_secs() < 10, "timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn ends_within_the_bound() {
        let b = Bounder::leaked();
        let ran = Arc::new(AtomicUsize::new(0));
        let no = AtomicBool::new(false);
        let got = b.read("a", Cfg { s: 2.0, bg: 0 }, &no, sleeper(0.05, ran.clone()));
        match got {
            Got::Done { out, joined, .. } => {
                assert_eq!(out, Ok(PathBuf::from("/r/0.05")));
                assert!(!joined);
            }
            g => panic!("{g:?}"),
        }
        assert_eq!(ran.load(Ordering::SeqCst), 1);
        // not in flight any more: the same reading again starts again (the cache answers it in the try)
        wait_until(|| b.reg.lock().unwrap().inflight.is_empty());
        assert!(matches!(b.read("a", Cfg { s: 2.0, bg: 0 }, &no, sleeper(0.01, ran.clone())), Got::Done { .. }));
        assert_eq!(ran.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn stopped_at_the_bound_then_bounded_at_once() {
        let b = Bounder::leaked();
        let ran = Arc::new(AtomicUsize::new(0));
        let no = AtomicBool::new(false);
        let got = b.read("b", Cfg { s: 0.1, bg: 0 }, &no, sleeper(30.0, ran.clone()));
        assert!(matches!(got, Got::Bound { background: false, at_once: false, joined: false, .. }), "{got:?}");
        if let Got::Bound { waited, .. } = got {
            assert!(waited >= 0.1);
        }
        assert_eq!(b.background(), 0);
        // the same reading asked again: at once, nothing started
        let got = b.read("b", Cfg { s: 0.1, bg: 0 }, &no, sleeper(30.0, ran.clone()));
        assert_eq!(got, Got::Bound { waited: 0.0, background: false, at_once: true, joined: false });
        assert_eq!(ran.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn background_reading_is_joined_and_ends() {
        let b = Bounder::leaked();
        let ran = Arc::new(AtomicUsize::new(0));
        let no = AtomicBool::new(false);
        let got = b.read("c", Cfg { s: 0.1, bg: 1 }, &no, sleeper(0.6, ran.clone()));
        assert!(matches!(got, Got::Bound { background: true, joined: false, .. }), "{got:?}");
        assert_eq!(b.background(), 1);
        // a later try for the same reading waits for that one and starts none
        let got = b.read("c", Cfg { s: 5.0, bg: 1 }, &no, sleeper(30.0, ran.clone()));
        match got {
            Got::Done { out, joined, .. } => {
                assert_eq!(out, Ok(PathBuf::from("/r/0.6")));
                assert!(joined);
            }
            g => panic!("{g:?}"),
        }
        assert_eq!(ran.load(Ordering::SeqCst), 1);
        wait_until(|| b.background() == 0);
    }

    #[test]
    fn background_cap_stops_the_next() {
        let b = Bounder::leaked();
        let ran = Arc::new(AtomicUsize::new(0));
        let no = AtomicBool::new(false);
        let cfg = Cfg { s: 0.05, bg: 1 };
        let g1 = b.read("d1", cfg, &no, sleeper(30.0, ran.clone()));
        assert!(matches!(g1, Got::Bound { background: true, .. }), "{g1:?}");
        let g2 = b.read("d2", cfg, &no, sleeper(30.0, ran.clone()));
        assert!(matches!(g2, Got::Bound { background: false, at_once: false, .. }), "{g2:?}");
        assert_eq!(b.background(), 1);
        // a joined try past its own bound on a background reading: still in the background, counted once
        let g3 = b.read("d1", cfg, &no, sleeper(30.0, ran.clone()));
        assert!(matches!(g3, Got::Bound { background: true, joined: true, .. }), "{g3:?}");
        assert_eq!(b.background(), 1);
        assert_eq!(ran.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn the_try_stop_stops_the_reading() {
        let b = Bounder::leaked();
        let ran = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let s2 = stop.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            s2.store(true, Ordering::SeqCst);
        });
        let got = b.read("e", Cfg { s: 20.0, bg: 0 }, &stop, sleeper(30.0, ran.clone()));
        assert!(matches!(got, Got::Cancelled { .. }), "{got:?}");
        wait_until(|| b.reg.lock().unwrap().inflight.is_empty());
    }

    #[test]
    fn the_switches() {
        // read from the environment only when set; the tests above give their own
        assert_eq!(ident(&[49], "end=1", "x"), ident(&[49], "end=1", "x"));
        assert_ne!(ident(&[49], "end=1", "x"), ident(&[48, 49], "end=1", "x"));
        assert_ne!(ident(&[49], "end=1", "x"), ident(&[49], "end=1", "y"));
    }
}
