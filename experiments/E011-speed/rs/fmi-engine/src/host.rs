//! The engine processes: `Host` starts them (speedrun/engine.py's launcher and control.py's Controller, as
//! replay_part1.config sets them up) and `BinEngine` drives one over the binary channel (control.py's Branch).

use crate::block::{self, Block};
use crate::sys;
use crate::wire::{self, R, W};
use fmi_core::*;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The game's release and its pinned settings (speedrun.releases "dos-cd", start.py, engine.py).
pub const TARGET: &str = "monkey-cd";
pub const GAMEID: &str = "monkey";
pub const PLATFORM: &str = "pc";
pub const TALKSPEED: u32 = 255;
pub const BOOT_PARAM: i32 = 0;
pub const MAX_TICKS: u64 = 10_000_000;
pub const MAX_FRAME: u64 = 200_000;
/// replay_part1.ANY_SEED: never used
pub const ANY_SEED: u32 = 836_557_856;

/// Where the engine and the game are, and the per-command timeout.
#[derive(Clone, Debug)]
pub struct HostCfg {
    /// the ScummVM build (patch 0002's: the binary channel)
    pub bin: PathBuf,
    /// the repository root: out/scummvm/saves and the ini's third_party paths, as speedrun.paths has them
    pub root: PathBuf,
    /// the game's files (releases.game_dir of dos-cd)
    pub game_dir: PathBuf,
    pub timeout: Duration,
    /// more bridge settings for every engine (the checks: SPEEDRUN_DUMP_EXACT, one lean switch alone)
    pub extra_env: Vec<(String, String)>,
}

pub const DEFAULT_BIN: &str = "/var/tmp/fast-mi/svm-e011/build/scummvm/scummvm";

impl HostCfg {
    /// FMI_ENGINE_BIN (else the E011 build), FMI_ROOT (else this crate's repository), the root's game/dos-cd;
    /// FMI_ENGINE_CUTS=1: every engine also gets SPEEDRUN_CUTS=1, round 2's count-only mixer and no pixel output
    /// (patch 0003, engine/CUTS.md; FMI_ENGINE_BIN must then be a build with patch 0003).
    pub fn from_env() -> HostCfg {
        let bin = std::env::var_os("FMI_ENGINE_BIN")
            .map(PathBuf::from)
            .unwrap_or_else(|| DEFAULT_BIN.into());
        let root = std::env::var_os("FMI_ROOT").map(PathBuf::from).unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../..")
                .canonicalize()
                .unwrap_or_default()
        });
        let game = root.join("game").join("dos-cd");
        let game_dir = game.canonicalize().unwrap_or(game);
        let mut extra_env = Vec::new();
        if std::env::var("FMI_ENGINE_CUTS").is_ok_and(|v| v == "1") {
            extra_env.push(("SPEEDRUN_CUTS".to_string(), "1".to_string()));
        }
        HostCfg {
            bin,
            root,
            game_dir,
            timeout: Duration::from_secs(600),
            extra_env,
        }
    }

    /// engine.py `_ini_text` (the run's scummvm.ini), plus `joystick_num=-1` when lean (no joystick scan at start).
    pub fn ini_text(&self, lean: bool) -> String {
        let mut s = String::from("[scummvm]\n");
        let saves = self.root.join("out/scummvm/saves");
        for (k, v) in [
            ("savepath", saves.display().to_string()),
            ("autosave_period", "0".into()),
            ("confirm_exit", "false".into()),
            ("fullscreen", "false".into()),
            ("gui_return_to_launcher_at_exit", "false".into()),
            ("vsync", "false".into()),
            (
                "extrapath",
                self.root
                    .join("third_party/scummvm/dists/engine-data")
                    .display()
                    .to_string(),
            ),
            (
                "themepath",
                self.root.join("third_party/scummvm/gui/themes").display().to_string(),
            ),
        ] {
            s.push_str(&format!("{k}={v}\n"));
        }
        if lean {
            s.push_str("joystick_num=-1\n");
        }
        s.push_str(&format!("\n[{TARGET}]\n"));
        for (k, v) in [
            ("gameid", GAMEID.to_string()),
            ("engineid", "scumm".into()),
            ("path", self.game_dir.display().to_string()),
            ("platform", PLATFORM.into()),
            ("language", "en".into()),
            ("copy_protection", "false".into()),
            ("autosave_period", "0".into()),
            ("subtitles", "true".into()),
            ("original_gui", "false".into()),
            ("enhancements", "0".into()),
            ("talkspeed", TALKSPEED.to_string()),
        ] {
            s.push_str(&format!("{k}={v}\n"));
        }
        s
    }
}

/// One C8 event as a SPEEDRUN_INPUTS line (replay.write_events: json.dumps of the event's dict).
pub fn event_line(e: &Event) -> String {
    let what = match &e.what {
        Input::Click(x, y) => format!("\"click\": [{x}, {y}]"),
        Input::RClick(x, y) => format!("\"rclick\": [{x}, {y}]"),
        Input::Move(x, y) => format!("\"move\": [{x}, {y}]"),
        Input::Key(k) => format!("\"key\": {}", crate::json::quote(k)),
        Input::Skips { text, cutscenes, spare } => {
            let sp = match spare {
                Some(v) => format!(
                    ", \"spare\": [{}]",
                    v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ")
                ),
                None => String::new(),
            };
            format!(
                "\"skips\": {{\"text\": {}, \"cutscenes\": {}{sp}}}",
                *text as u8, *cutscenes as u8
            )
        }
    };
    format!("{{\"frame\": {}, {what}}}", e.frame)
}

/// Whether an event is an input (replay._is_input: a key, click, rclick or move).
pub fn is_input(e: &Event) -> bool {
    !matches!(e.what, Input::Skips { .. })
}

#[derive(Clone, Copy, Debug)]
pub struct Hello {
    pub pid: u32,
    pub parent: u32,
    pub frame: u64,
    pub tick: u64,
}

/// One root engine and every process forked from it: the socket they connect to (as control.py's Controller has one
/// per root engine). The root is the connection whose hello has no parent; a fork's is found by its pid.
struct Family {
    timeout: Duration,
    dir: PathBuf,
    sock: PathBuf,
    listener: Mutex<UnixListener>,
    /// connections accepted for another process than the one asked for
    pending: Mutex<HashMap<u32, (UnixStream, Hello)>>,
}

/// The pid that stands for the root in `Family::accept` (a hello's parent 0).
const ROOT: u32 = 0;

impl Drop for Family {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.sock);
        let _ = std::fs::remove_dir(&self.dir);
    }
}

fn io_err(e: std::io::Error) -> EngineError {
    EngineError::Io(e)
}

fn bad(what: String) -> EngineError {
    EngineError::Bridge(format!("bad reply: {what}"))
}

fn read_msg(s: &mut UnixStream) -> EResult<(u8, Vec<u8>)> {
    let mut h = [0u8; 5];
    s.read_exact(&mut h).map_err(io_err)?;
    let n = u32::from_le_bytes(h[..4].try_into().unwrap()) as usize;
    let mut p = vec![0u8; n];
    s.read_exact(&mut p).map_err(io_err)?;
    Ok((h[4], p))
}

impl Family {
    fn new(timeout: Duration) -> std::io::Result<Family> {
        let dir = std::env::temp_dir().join(format!(
            "fmi-ctl-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir)?;
        let sock = dir.join("control.sock");
        let _ = std::fs::remove_file(&sock);
        let listener = UnixListener::bind(&sock)?;
        listener.set_nonblocking(true)?;
        Ok(Family {
            timeout,
            dir,
            sock,
            listener: Mutex::new(listener),
            pending: Mutex::new(HashMap::new()),
        })
    }

    /// The connection of engine process `pid` (`ROOT`: the family's root) and its hello: accepted now, or earlier
    /// for another caller.
    fn accept_pid(&self, pid: u32) -> EResult<(UnixStream, Hello)> {
        let deadline = Instant::now() + self.timeout;
        loop {
            if let Some(c) = self.pending.lock().unwrap().remove(&pid) {
                return Ok(c);
            }
            if Instant::now() > deadline {
                return Err(EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    format!("engine {pid} did not connect"),
                )));
            }
            let l = self.listener.lock().unwrap();
            if let Some(c) = self.pending.lock().unwrap().remove(&pid) {
                return Ok(c);
            }
            if !sys::readable(l.as_raw_fd(), Duration::from_millis(50)) {
                continue;
            }
            let (mut s, _) = match l.accept() {
                Ok(c) => c,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(e) => return Err(io_err(e)),
            };
            drop(l);
            s.set_read_timeout(Some(self.timeout)).map_err(io_err)?;
            let (ty, p) = read_msg(&mut s)?;
            if ty != wire::R_HELLO {
                return Err(bad(format!("type {ty:#x} where a hello was expected")));
            }
            let mut r = R::new(&p);
            let h = (|| -> wire::DResult<Hello> {
                let h = Hello {
                    pid: r.u32()?,
                    parent: r.u32()?,
                    frame: r.u64()?,
                    tick: r.u64()?,
                };
                r.done()?;
                Ok(h)
            })()
            .map_err(bad)?;
            let key = if h.parent == 0 { ROOT } else { h.pid };
            if key == pid {
                return Ok((s, h));
            }
            self.pending.lock().unwrap().insert(key, (s, h));
        }
    }
}

/// Starts engine processes (EngineHost). Each root engine gets its own listening socket, which its forks connect
/// to; their connections are matched to their processes by pid.
#[derive(Clone)]
pub struct Host {
    cfg: Arc<HostCfg>,
}

static SEQ: AtomicU64 = AtomicU64::new(0);

impl Host {
    pub fn new(cfg: HostCfg) -> std::io::Result<Host> {
        Ok(Host { cfg: Arc::new(cfg) })
    }

    pub fn from_env() -> std::io::Result<Host> {
        Host::new(HostCfg::from_env())
    }

    pub fn cfg(&self) -> &HostCfg {
        &self.cfg
    }

    /// A fresh engine (replay_part1.config's settings, the binary channel, the lean switches when `cfg.lean`), its
    /// movie from boot as SPEEDRUN_INPUTS (events up to `cfg.frame` when set), stopped at its first decision point.
    pub fn spawn(&self, cfg: &BootCfg) -> EResult<BinEngine> {
        if cfg.seed == ANY_SEED {
            return Err(EngineError::Bridge(
                "the Any% seed is never used by the system's runs".into(),
            ));
        }
        let h = &*self.cfg;
        let family = Arc::new(Family::new(h.timeout).map_err(io_err)?);
        let out = &cfg.out_dir;
        for d in [
            out.clone(),
            h.root.join("out/scummvm/saves"),
            out.join("xdg/data"),
            out.join("xdg/cache"),
        ] {
            std::fs::create_dir_all(&d).map_err(io_err)?;
        }
        let ini = out.join("scummvm.ini");
        std::fs::write(&ini, h.ini_text(cfg.lean)).map_err(io_err)?;
        let events: Vec<&Event> = cfg
            .events
            .iter()
            .filter(|e| cfg.frame.is_none_or(|f| e.frame <= f))
            .collect();
        let mut sorted = events.clone();
        sorted.sort_by_key(|e| e.frame); // stable, as write_events
        let mut cmd = Command::new(&h.bin);
        cmd.arg(format!("--config={}", ini.display()))
            .arg(format!("--savepath={}", h.root.join("out/scummvm/saves").display()))
            .arg(format!("--boot-param={BOOT_PARAM}"))
            .arg("--disable-sdl-audio")
            .arg(format!("--random-seed={}", cfg.seed))
            .arg(TARGET);
        for (k, _) in std::env::vars_os() {
            if k.to_string_lossy().starts_with("SPEEDRUN_") {
                cmd.env_remove(k);
            }
        }
        cmd.env("SPEEDRUN_OUT", out)
            .env("SPEEDRUN_START", "[]")
            .env("SPEEDRUN_GOAL", "[]")
            .env("SPEEDRUN_FAST", "1")
            .env("SPEEDRUN_MAX_TICKS", MAX_TICKS.to_string())
            .env("SPEEDRUN_TIMER_NEXT", "any")
            .env("XDG_DATA_HOME", out.join("xdg/data"))
            .env("XDG_CACHE_HOME", out.join("xdg/cache"))
            .env("SDL_VIDEODRIVER", "dummy")
            .env("SDL_AUDIODRIVER", "dummy")
            .env("SPEEDRUN_CONTROL", &family.sock)
            .env("SPEEDRUN_WIRE", "bin");
        if !sorted.is_empty() {
            let movie = out.join("prefix.jsonl");
            let text: String = sorted.iter().map(|e| event_line(e) + "\n").collect();
            std::fs::write(&movie, text).map_err(io_err)?;
            cmd.env("SPEEDRUN_INPUTS", movie.canonicalize().unwrap_or(movie));
        }
        if cfg.dump_locals {
            cmd.env("SPEEDRUN_DUMP_LOCALS", "1");
        }
        if cfg.lean {
            cmd.env("SPEEDRUN_LEAN", "1");
        }
        for (k, v) in &h.extra_env {
            cmd.env(k, v);
        }
        let log = std::fs::File::create(out.join("stdout.log")).map_err(io_err)?;
        let log2 = log.try_clone().map_err(io_err)?;
        cmd.stdin(Stdio::null()).stdout(log).stderr(log2);
        let mut child = cmd.spawn().map_err(io_err)?;
        // the root by its hello (no parent), not by the pid spawned: a wrapper may run the engine as its child
        let conn = match family.accept_pid(ROOT) {
            Ok(c) => c,
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e);
            }
        };
        let mut e = BinEngine::new(family, conn.0, conn.1);
        e.proc_ = Some(child);
        e.first_stop()?;
        Ok(e)
    }

    /// replay.boot (frame set) or replay_part1.boot (frame None: the first idle stop), as a `BinEngine`.
    pub fn boot_bin(&self, cfg: &BootCfg) -> EResult<BinEngine> {
        let mut e = self.spawn(cfg)?;
        let spec = match cfg.frame {
            Some(f) => RunSpec {
                frame: Some(f),
                ..Default::default()
            },
            None => RunSpec {
                idle: true,
                max_frame: Some(MAX_FRAME),
                ..Default::default()
            },
        };
        let st = e.run(&spec)?;
        if let Some(f) = cfg.frame {
            if st.frame != f {
                return Err(EngineError::Bridge(format!(
                    "the boot replay stopped at {} instead of frame {f}",
                    st.frame
                )));
            }
        }
        Ok(e)
    }
}

impl EngineHost for Host {
    fn boot(&self, cfg: &BootCfg) -> EResult<Box<dyn Engine>> {
        Ok(Box::new(self.boot_bin(cfg)?))
    }
}

/// What a Step returns besides the new state (kept in the engine: `facts`, `block`).
#[derive(Clone, Debug, Default)]
pub struct StepOut {
    pub status: Status,
    pub runs: Option<RunsReply>,
    pub explain: Option<ExplainReply>,
}

/// One engine process over the binary channel (control.py's Branch).
pub struct BinEngine {
    shared: Arc<Family>,
    pid: u32,
    stream: UnixStream,
    rbuf: Vec<u8>,
    rlen: usize,
    status: Status,
    /// the last F2 block received on this connection: the next delta's base
    last: Vec<u8>,
    /// `last` is the state at the current stop
    last_now: bool,
    /// the facts of `last`, once computed (kept up to date through Step's deltas)
    last_facts: Option<Facts>,
    /// a Step's delta chunks (scratch)
    ranges: Vec<(usize, usize)>,
    /// how the facts were had: (followed a delta, built whole)
    pub facts_counts: (u64, u64),
    proc_: Option<Child>,
    done: bool,
}

impl BinEngine {
    fn new(shared: Arc<Family>, stream: UnixStream, h: Hello) -> BinEngine {
        let status = Status {
            frame: h.frame,
            tick: h.tick,
            ..Default::default()
        };
        BinEngine {
            shared,
            pid: h.pid,
            stream,
            rbuf: vec![0; 1 << 16],
            rlen: 0,
            status,
            last: Vec::new(),
            last_now: false,
            last_facts: None,
            ranges: Vec::new(),
            facts_counts: (0, 0),
            proc_: None,
            done: false,
        }
    }

    fn send(&mut self, msg: Vec<u8>) -> EResult<()> {
        self.stream.write_all(&msg).map_err(io_err)
    }

    /// One message: (type, payload), the payload copied out of the read buffer.
    fn recv(&mut self) -> EResult<(u8, Vec<u8>)> {
        let mut need = 5usize;
        loop {
            if self.rlen >= 5 {
                need = 5 + u32::from_le_bytes(self.rbuf[..4].try_into().unwrap()) as usize;
                if self.rlen >= need {
                    let ty = self.rbuf[4];
                    let p = self.rbuf[5..need].to_vec();
                    self.rbuf.copy_within(need..self.rlen, 0);
                    self.rlen -= need;
                    return Ok((ty, p));
                }
            }
            if self.rbuf.len() < need {
                self.rbuf.resize(need.next_power_of_two(), 0);
            }
            let n = self.stream.read(&mut self.rbuf[self.rlen..]).map_err(io_err)?;
            if n == 0 {
                return Err(EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    format!("engine {} closed the connection", self.pid),
                )));
            }
            self.rlen += n;
        }
    }

    /// The reply of type `want`, or the bridge's error or the engine's end as errors.
    fn expect(&mut self, want: u8) -> EResult<Vec<u8>> {
        let (ty, p) = self.recv()?;
        if ty == want {
            return Ok(p);
        }
        match ty {
            wire::R_ERROR => Err(EngineError::Bridge(R::new(&p).str().map_err(bad)?)),
            wire::R_END => {
                let st = R::new(&p).status().map_err(bad)?;
                self.status = st.clone();
                Err(EngineError::Ended(Box::new(st)))
            }
            _ => Err(bad(format!("type {ty:#x} where {want:#x} was expected"))),
        }
    }

    fn stop(&mut self) -> EResult<Status> {
        let p = self.expect(wire::R_STOP)?;
        let mut r = R::new(&p);
        let st = r.status().map_err(bad)?;
        r.done().map_err(bad)?;
        self.status = st.clone();
        Ok(st)
    }

    fn first_stop(&mut self) -> EResult<()> {
        self.stream
            .set_read_timeout(Some(self.shared.timeout))
            .map_err(io_err)?;
        self.stop()?;
        Ok(())
    }

    fn moved(&mut self) {
        self.last_now = false;
    }

    /// Runs `frames` frames and replies, in one round trip, with the stop status, the state (kept here: `facts`,
    /// `block`) and, when asked, the runs and explain entries recorded meanwhile.
    pub fn step(&mut self, frames: u32, with_runs: bool, with_explain: bool) -> EResult<StepOut> {
        let mut w = W::msg(wire::C_STEP);
        w.u32(frames);
        w.b(with_runs);
        w.b(with_explain);
        self.send(w.finish())?;
        self.moved();
        let p = self.expect(wire::R_STEP_REPLY)?;
        let mut r = R::new(&p);
        let status = r.status().map_err(bad)?;
        let old = block::layout(&self.last);
        let mut ranges = std::mem::take(&mut self.ranges);
        let delta = block::apply(&mut self.last, &mut r, &mut ranges).map_err(bad)?;
        // the facts follow the delta when they were known; otherwise they are built on the next facts()
        let kept = match (&mut self.last_facts, delta) {
            (Some(f), true) => block::update_facts(f, old, &self.last, &ranges),
            _ => false,
        };
        if kept {
            self.facts_counts.0 += 1;
        } else {
            self.last_facts = None;
        }
        self.ranges = ranges;
        let runs = if r.tag().map_err(bad)? {
            Some(r.runs_reply().map_err(bad)?)
        } else {
            None
        };
        let explain = if r.tag().map_err(bad)? {
            Some(r.explain_reply().map_err(bad)?)
        } else {
            None
        };
        r.done().map_err(bad)?;
        self.last_now = true;
        self.status = status.clone();
        Ok(StepOut { status, runs, explain })
    }

    /// The F2 block at the current stop (one State round trip unless a Step just brought it).
    pub fn block(&mut self) -> EResult<&[u8]> {
        if !self.last_now {
            self.send(W::msg(wire::C_STATE).finish())?;
            let p = self.expect(wire::R_STATE)?;
            Block::parse(&p).map_err(bad)?;
            self.last = p;
            self.last_facts = None;
            self.last_now = true;
        }
        Ok(&self.last)
    }

    /// The Obs reply's payload, not decoded (`wire::R::obs` decodes it).
    pub fn obs_raw(&mut self, req: &ObsReq) -> EResult<Vec<u8>> {
        let mut w = W::msg(wire::C_OBS);
        w.obs_req(req);
        self.send(w.finish())?;
        self.expect(wire::R_OBS)
    }

    /// The F2 block asked anew (one State round trip), whatever is cached.
    pub fn block_fresh(&mut self) -> EResult<&[u8]> {
        self.last_now = false;
        self.block()
    }

    pub fn hello_status(&self) -> &Status {
        &self.status
    }

    fn shutdown(&mut self) {
        if self.done {
            return;
        }
        self.done = true;
        let _ = self.stream.write_all(&W::msg(wire::C_QUIT).finish());
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
        if let Some(mut c) = self.proc_.take() {
            let t = Instant::now();
            loop {
                match c.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) if t.elapsed() < Duration::from_secs(10) => std::thread::sleep(Duration::from_millis(1)),
                    _ => {
                        let _ = c.kill();
                        let _ = c.wait();
                        break;
                    }
                }
            }
        }
    }

    fn child_of(&mut self, pid: u32) -> EResult<BinEngine> {
        let (s, h) = self.shared.accept_pid(pid)?;
        let mut c = BinEngine::new(self.shared.clone(), s, h);
        c.first_stop()?;
        Ok(c)
    }
}

impl Drop for BinEngine {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl Engine for BinEngine {
    fn pid(&self) -> u32 {
        self.pid
    }

    fn status(&self) -> &Status {
        &self.status
    }

    fn run(&mut self, spec: &RunSpec) -> EResult<Status> {
        let mut w = W::msg(wire::C_RUN);
        w.run_spec(spec);
        self.send(w.finish())?;
        self.moved();
        self.stop()
    }

    fn inputs(&mut self, events: &[Event]) -> EResult<()> {
        if events.is_empty() {
            return Ok(());
        }
        let mut w = W::msg(wire::C_INPUTS);
        w.u32(events.len() as u32);
        for e in events {
            w.event(e);
        }
        self.send(w.finish())?;
        self.expect(wire::R_OK)?;
        Ok(())
    }

    fn obs(&mut self, req: &ObsReq) -> EResult<Obs> {
        let mut w = W::msg(wire::C_OBS);
        w.obs_req(req);
        self.send(w.finish())?;
        let p = self.expect(wire::R_OBS)?;
        let mut r = R::new(&p);
        let o = r.obs().map_err(bad)?;
        r.done().map_err(bad)?;
        Ok(o)
    }

    fn facts(&mut self) -> EResult<Facts> {
        self.block()?;
        if self.last_facts.is_none() {
            self.last_facts = Some(Block::parse(&self.last).map_err(bad)?.facts());
            self.facts_counts.1 += 1;
        }
        Ok(self.last_facts.clone().unwrap())
    }

    fn holds(&mut self, conds: &[Cond]) -> EResult<bool> {
        let mut w = W::msg(wire::C_HOLDS);
        w.conds(conds);
        self.send(w.finish())?;
        let p = self.expect(wire::R_HOLDS)?;
        let mut r = R::new(&p);
        let v = r.b().map_err(bad)?;
        r.done().map_err(bad)?;
        Ok(v)
    }

    fn at(&mut self, x: i32, y: i32) -> EResult<At> {
        let mut w = W::msg(wire::C_AT);
        w.i32(x);
        w.i32(y);
        self.send(w.finish())?;
        let p = self.expect(wire::R_AT)?;
        let mut r = R::new(&p);
        let a = r.at().map_err(bad)?;
        r.done().map_err(bad)?;
        Ok(a)
    }

    fn set_rng(&mut self, state: u32) -> EResult<()> {
        let mut w = W::msg(wire::C_SET_RNG);
        w.u32(state);
        self.send(w.finish())?;
        self.expect(wire::R_OK)?;
        self.moved();
        Ok(())
    }

    fn segment(&mut self, on: bool) -> EResult<()> {
        let mut w = W::msg(wire::C_SEGMENT);
        w.b(on);
        self.send(w.finish())?;
        self.expect(wire::R_OK)?;
        Ok(())
    }

    fn runs(&mut self) -> EResult<RunsReply> {
        self.send(W::msg(wire::C_RUNS).finish())?;
        let p = self.expect(wire::R_RUNS)?;
        let mut r = R::new(&p);
        let v = r.runs_reply().map_err(bad)?;
        r.done().map_err(bad)?;
        Ok(v)
    }

    fn explain(&mut self, arm: Option<&ExplainArm>) -> EResult<()> {
        let mut w = W::msg(wire::C_EXPLAIN);
        w.explain_arm(arm);
        self.send(w.finish())?;
        self.expect(wire::R_OK)?;
        Ok(())
    }

    fn explain_log(&mut self) -> EResult<ExplainReply> {
        self.send(W::msg(wire::C_EXPLAIN_LOG).finish())?;
        let p = self.expect(wire::R_EXPLAIN)?;
        let mut r = R::new(&p);
        let v = r.explain_reply().map_err(bad)?;
        r.done().map_err(bad)?;
        Ok(v)
    }

    fn fork(&mut self) -> EResult<Box<dyn Engine>> {
        Ok(Box::new(self.fork_bin()?))
    }

    fn fork_each(&mut self, n: u32) -> EResult<(Status, Vec<Box<dyn Engine>>)> {
        let (st, kids) = self.fork_each_bin(n)?;
        Ok((st, kids.into_iter().map(|k| Box::new(k) as Box<dyn Engine>).collect()))
    }

    fn state_json(&mut self, req: &ObsReq) -> EResult<String> {
        let mut w = W::msg(wire::C_STATE_JSON);
        w.obs_req(req);
        self.send(w.finish())?;
        let p = self.expect(wire::R_JSON)?;
        let mut r = R::new(&p);
        let s = r.str().map_err(bad)?;
        r.done().map_err(bad)?;
        Ok(s)
    }

    fn cpu_s(&self) -> f64 {
        sys::cpu_s(self.pid)
    }

    fn quit(mut self: Box<Self>) {
        self.shutdown();
    }
}

impl BinEngine {
    /// `fork` as a `BinEngine`: the child stops where this one is, with this stop's facts already known.
    pub fn fork_bin(&mut self) -> EResult<BinEngine> {
        self.send(W::msg(wire::C_FORK).finish())?;
        let p = self.expect(wire::R_FORKED)?;
        let mut r = R::new(&p);
        let pid = r.u32().map_err(bad)?;
        r.done().map_err(bad)?;
        let mut c = self.child_of(pid)?;
        if self.last_now {
            // the child stands where this one stands: the same block and facts. Its connection has sent no block,
            // so its first block comes whole and `last` is only a cache there, never a delta's base.
            c.last = self.last.clone();
            c.last_facts = self.last_facts.clone();
            c.last_now = true;
        }
        Ok(c)
    }

    /// `fork_each` as `BinEngine`s, in frame order.
    pub fn fork_each_bin(&mut self, n: u32) -> EResult<(Status, Vec<BinEngine>)> {
        let mut w = W::msg(wire::C_FORK_EACH);
        w.u32(n);
        self.send(w.finish())?;
        self.moved();
        let st = match self.stop() {
            Ok(st) => st,
            Err(EngineError::Ended(end)) => {
                // the children made before the end: accepted and quit
                for &(_, pid) in &end.forks {
                    if pid == u32::MAX {
                        continue;
                    }
                    if let Ok(c) = self.child_of(pid) {
                        drop(c);
                    }
                }
                return Err(EngineError::Ended(end));
            }
            Err(e) => return Err(e),
        };
        let mut kids = Vec::with_capacity(st.forks.len());
        for &(_, pid) in &st.forks {
            if pid == u32::MAX {
                return Err(EngineError::Bridge("fork_each: a fork failed".into()));
            }
            kids.push(self.child_of(pid)?);
        }
        kids.sort_by_key(|k| k.status.frame);
        Ok((st, kids))
    }
}
