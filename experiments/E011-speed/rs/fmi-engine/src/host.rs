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

/// E014's switch `FMI_GAME_ID` (experiments/E014-fate-demo): another game's ScummVM game id; its files are in
/// `FMI_GAME_DIR`. Unset: MI's DOS CD release (`TARGET`, `GAMEID`, the root's game/dos-cd), as before. Set, every
/// engine also gets a save folder of its own, made empty at its boot (`HostCfg::own_saves`): a game's scripts may
/// write and read files there (SCUMM v5's roomOps 9, 13, 14), and a shared folder would carry one run's files into
/// another's boot.
pub fn game_id_env() -> Option<String> {
    std::env::var("FMI_GAME_ID").ok().filter(|v| !v.is_empty())
}

/// Where the engine and the game are, and the per-command timeout.
#[derive(Clone, Debug)]
pub struct HostCfg {
    /// the ScummVM build (patch 0002's: the binary channel)
    pub bin: PathBuf,
    /// the repository root: out/scummvm/saves and the ini's third_party paths, as speedrun.paths has them
    pub root: PathBuf,
    /// the game's files (releases.game_dir of dos-cd)
    pub game_dir: PathBuf,
    /// the ScummVM target (the ini's section) and game id (E014: FMI_GAME_ID's; else MI's)
    pub target: String,
    pub gameid: String,
    /// E014: every engine's save folder is its own (OUT/saves, made empty at the boot), not the root's shared one
    pub own_saves: bool,
    pub timeout: Duration,
    /// more bridge settings for every engine (the checks: SPEEDRUN_DUMP_EXACT, one lean switch alone)
    pub extra_env: Vec<(String, String)>,
}

pub const DEFAULT_BIN: &str = "/var/tmp/fast-mi/svm-e011/build/scummvm/scummvm";

/// E012's switch `SPEEDRUN_REPORT_RESTART` (component end): "1" in this process's environment passes it to every
/// engine (`HostCfg::from_env`). Off by default.
pub fn report_restart() -> bool {
    std::env::var("SPEEDRUN_REPORT_RESTART").is_ok_and(|v| v == "1")
}

/// E012's switch `FMI_BOXES` (component end): "1" lets the walk-box affordances ask `Engine::boxes`. Off by default,
/// and then nothing sends the Boxes command.
pub fn boxes_on() -> bool {
    std::env::var("FMI_BOXES").is_ok_and(|v| v == "1")
}

/// E013's switches `SPEEDRUN_INPUT=1` (the input log, patch 0007) and `FMI_REFUSE=1` (component refuse, which reads
/// it): either passes SPEEDRUN_INPUT=1 to every engine. Off by default.
pub fn input_log_on() -> bool {
    std::env::var("SPEEDRUN_INPUT").is_ok_and(|v| v == "1") || std::env::var("FMI_REFUSE").is_ok_and(|v| v == "1")
}

impl HostCfg {
    /// FMI_ENGINE_BIN (else the E011 build), FMI_ROOT (else this crate's repository), the root's game/dos-cd;
    /// FMI_ENGINE_CUTS=1: every engine also gets SPEEDRUN_CUTS=1, round 2's count-only mixer and no pixel output
    /// (patch 0003, engine/CUTS.md; FMI_ENGINE_BIN must then be a build with patch 0003).
    /// E012: SPEEDRUN_REPORT_RESTART=1 in this process's environment: every engine gets it too (patch 0005: a
    /// restart or a pause ends the run as "restart" or "pause"; FMI_ENGINE_BIN must then be a build with patch 0005).
    /// `spawn` drops every inherited SPEEDRUN_ variable, so this is the only way it reaches an engine.
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
        let (target, gameid, game, own_saves) = match game_id_env() {
            Some(id) => {
                let dir = std::env::var_os("FMI_GAME_DIR").map(PathBuf::from).expect("FMI_GAME_DIR: the files of FMI_GAME_ID's game");
                (id.clone(), id, dir, true)
            }
            None => (TARGET.to_string(), GAMEID.to_string(), root.join("game").join("dos-cd"), false),
        };
        let game_dir = game.canonicalize().unwrap_or(game);
        let mut extra_env = Vec::new();
        if std::env::var("FMI_ENGINE_CUTS").is_ok_and(|v| v == "1") {
            extra_env.push(("SPEEDRUN_CUTS".to_string(), "1".to_string()));
        }
        if report_restart() {
            extra_env.push(("SPEEDRUN_REPORT_RESTART".to_string(), "1".to_string()));
        }
        // E014: the decision point without verb ids (SPEEDRUN_IDLE=handler) and the end's site (SPEEDRUN_END_SITE=1),
        // passed to every engine when set in this process's environment (an engine with the E014 patch).
        // E024 (plan.md Part 2, item 8): the segment's start frame (SPEEDRUN_START_FRAME, patch 0001) and var 19's
        // required value (SPEEDRUN_TIMER_NEXT), so skips run from boot; extra_env goes on after TIMER_NEXT=any (spawn)
        for k in ["SPEEDRUN_IDLE", "SPEEDRUN_END_SITE", "SPEEDRUN_START_FRAME", "SPEEDRUN_TIMER_NEXT"] {
            if let Some(v) = std::env::var(k).ok().filter(|v| !v.is_empty()) {
                extra_env.push((k.to_string(), v));
            }
        }
        // E013 (patch 0007): the input log, passed to every engine with SPEEDRUN_INPUT=1, and with FMI_REFUSE=1 (the
        // component refuse reads it); an engine with patch 0007
        if input_log_on() {
            extra_env.push(("SPEEDRUN_INPUT".to_string(), "1".to_string()));
        }
        HostCfg {
            bin,
            root,
            game_dir,
            target,
            gameid,
            own_saves,
            timeout: Duration::from_secs(600),
            extra_env,
        }
    }

    /// The save folder of an engine whose out folder is `out`: its own (E014), else the root's shared one.
    pub fn saves(&self, out: &Path) -> PathBuf {
        if self.own_saves {
            out.join("saves")
        } else {
            self.root.join("out/scummvm/saves")
        }
    }

    /// engine.py `_ini_text` (the run's scummvm.ini), plus `joystick_num=-1` when lean (no joystick scan at start).
    /// `saves`: the save folder (`HostCfg::saves`).
    pub fn ini_text(&self, lean: bool, saves: &Path) -> String {
        let mut s = String::from("[scummvm]\n");
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
        s.push_str(&format!("\n[{}]\n", self.target));
        for (k, v) in [
            ("gameid", self.gameid.clone()),
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

    /// The engine's command for a boot: its out folder, ini and save folder made, its settings, its movie from boot
    /// as SPEEDRUN_INPUTS (events up to `cfg.frame` when set); with no control channel (`spawn` adds it).
    fn command(&self, cfg: &BootCfg) -> EResult<Command> {
        if cfg.seed == ANY_SEED {
            return Err(EngineError::Bridge(
                "the Any% seed is never used by the system's runs".into(),
            ));
        }
        let h = &*self.cfg;
        let out = &cfg.out_dir;
        let saves = h.saves(out);
        if h.own_saves {
            // E014: an empty save folder at every boot
            let _ = std::fs::remove_dir_all(&saves);
        }
        for d in [
            out.clone(),
            saves.clone(),
            out.join("xdg/data"),
            out.join("xdg/cache"),
        ] {
            std::fs::create_dir_all(&d).map_err(io_err)?;
        }
        let ini = out.join("scummvm.ini");
        std::fs::write(&ini, h.ini_text(cfg.lean, &saves)).map_err(io_err)?;
        let events: Vec<&Event> = cfg
            .events
            .iter()
            .filter(|e| cfg.frame.is_none_or(|f| e.frame <= f))
            .collect();
        let mut sorted = events.clone();
        sorted.sort_by_key(|e| e.frame); // stable, as write_events
        let mut cmd = Command::new(&h.bin);
        cmd.arg(format!("--config={}", ini.display()))
            .arg(format!("--savepath={}", saves.display()))
            .arg(format!("--boot-param={BOOT_PARAM}"))
            .arg("--disable-sdl-audio")
            .arg(format!("--random-seed={}", cfg.seed))
            .arg(&h.target);
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
            .env("SDL_AUDIODRIVER", "dummy");
        // E020 (FMI_IMUSE_TICKS=1): iMUSE's sequencer advances by game time, not on the mixer's wall-time timer
        if std::env::var("FMI_IMUSE_TICKS").is_ok_and(|v| v == "1") {
            cmd.env("SPEEDRUN_IMUSE_TICKS", "1");
        }
        // E021 (FMI_TEMP_SAVES=1): the scripts' temporary saves and loads are the game's, kept in the engine's memory
        if std::env::var("FMI_TEMP_SAVES").is_ok_and(|v| v == "1") {
            cmd.env("SPEEDRUN_TEMP_SAVES", "1");
        }
        // E024 (FMI_MEM_FILES=1, patch 0010): the files the scripts write (roomOps 13, 14) kept in the engine's memory,
        // so the forks of one boot (a scan's candidates) never share the save folder's file
        if std::env::var("FMI_MEM_FILES").is_ok_and(|v| v == "1") {
            cmd.env("SPEEDRUN_MEM_FILES", "1");
        }
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
        Ok(cmd)
    }

    /// E024 (plan.md Part 2, item 9): the engine run standalone, its movie from boot as `spawn` gives it but no
    /// control channel: it runs the movie by itself until its end or `max_ticks` (SPEEDRUN_MAX_TICKS), and the end is
    /// in its trace (OUT/trace.jsonl). Waits for it at most `wall`.
    pub fn standalone(&self, cfg: &BootCfg, max_ticks: u64, wall: Duration) -> EResult<std::process::ExitStatus> {
        let mut cmd = self.command(cfg)?;
        cmd.env("SPEEDRUN_MAX_TICKS", max_ticks.to_string());
        let log = std::fs::File::create(cfg.out_dir.join("stdout.log")).map_err(io_err)?;
        let log2 = log.try_clone().map_err(io_err)?;
        cmd.stdin(Stdio::null()).stdout(log).stderr(log2);
        let mut child = cmd.spawn().map_err(io_err)?;
        let t = Instant::now();
        loop {
            match child.try_wait().map_err(io_err)? {
                Some(st) => return Ok(st),
                None if t.elapsed() < wall => std::thread::sleep(Duration::from_millis(20)),
                None => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(EngineError::Bridge(format!("standalone: no end within {} s", wall.as_secs())));
                }
            }
        }
    }

    /// A fresh engine (replay_part1.config's settings, the binary channel, the lean switches when `cfg.lean`), its
    /// movie from boot as SPEEDRUN_INPUTS (events up to `cfg.frame` when set), stopped at its first decision point.
    pub fn spawn(&self, cfg: &BootCfg) -> EResult<BinEngine> {
        let mut cmd = self.command(cfg)?;
        let family = Arc::new(Family::new(self.cfg.timeout).map_err(io_err)?);
        cmd.env("SPEEDRUN_CONTROL", &family.sock).env("SPEEDRUN_WIRE", "bin");
        let out = &cfg.out_dir;
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
    /// the last Step's block came as a delta (its chunks in `ranges`)
    last_delta: bool,
    /// the block before the last Step (step_changes's scratch)
    prev: Vec<u8>,
    /// E019: the keys the next waits stop on (`Engine::set_watch`), for StepTrace (FMI_STEP_TRACE=1)
    watch: Option<Vec<Key>>,
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
            last_delta: false,
            prev: Vec::new(),
            watch: None,
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
            let n = match self.stream.read(&mut self.rbuf[self.rlen..]) {
                Ok(n) => n,
                // a signal (a debugger's or a profiler's stop) interrupted the read: read again
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(io_err(e)),
            };
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
        self.last_delta = delta;
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

    /// The JSON "boxes" reply's text at the current stop (patch 0005's BoxesJson; the Boxes gate's other side).
    pub fn boxes_json(&mut self) -> EResult<String> {
        self.send(W::msg(wire::C_BOXES_JSON).finish())?;
        let p = self.expect(wire::R_JSON)?;
        let mut r = R::new(&p);
        let s = r.str().map_err(bad)?;
        r.done().map_err(bad)?;
        Ok(s)
    }

    /// The JSON "input_log" reply's text at the current stop (patch 0007's InputLogJson; takes the records, as InputLog).
    pub fn input_log_json(&mut self) -> EResult<String> {
        self.send(W::msg(wire::C_INPUT_LOG_JSON).finish())?;
        let p = self.expect(wire::R_JSON)?;
        let mut r = R::new(&p);
        let s = r.str().map_err(bad)?;
        r.done().map_err(bad)?;
        Ok(s)
    }

    /// The JSON "digest" reply's text at the current stop (patch 0007's DigestJson).
    pub fn digest_json(&mut self) -> EResult<String> {
        self.send(W::msg(wire::C_DIGEST_JSON).finish())?;
        let p = self.expect(wire::R_JSON)?;
        let mut r = R::new(&p);
        let s = r.str().map_err(bad)?;
        r.done().map_err(bad)?;
        Ok(s)
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

    /// One StepObs message (patch 0004): the run to `frame` and the stop's runs, explain entries and dump.
    fn step_observe(&mut self, frame: u64, req: &ObsReq) -> EResult<(Status, RunsReply, ExplainReply, Obs)> {
        let mut w = W::msg(wire::C_STEP_OBS);
        w.u64(frame);
        w.obs_req(req);
        self.send(w.finish())?;
        self.moved();
        let p = self.expect(wire::R_STEP_OBS_REPLY)?;
        let mut r = R::new(&p);
        let st = r.status().map_err(bad)?;
        let runs = r.runs_reply().map_err(bad)?;
        let ents = r.explain_reply().map_err(bad)?;
        let o = r.obs().map_err(bad)?;
        r.done().map_err(bad)?;
        self.status = st.clone();
        Ok((st, runs, ents, o))
    }

    /// One RunObs message (patch 0004): the run and the dump at its stop.
    fn run_obs(&mut self, spec: &RunSpec, req: &ObsReq) -> EResult<(Status, Obs)> {
        self.run_obs_msg(spec, req, false)
    }

    /// One RunObs message asking the room's dump (patch 0004).
    fn run_room(&mut self, spec: &RunSpec) -> EResult<(Status, Obs)> {
        self.run_obs_msg(spec, &ObsReq::default(), true)
    }

    /// One AtMany message (patch 0004).
    fn at_many(&mut self, pts: &[(i32, i32)]) -> EResult<Vec<At>> {
        if pts.is_empty() {
            return Ok(Vec::new());
        }
        let mut w = W::msg(wire::C_AT_MANY);
        w.u32(pts.len() as u32);
        for &(x, y) in pts {
            w.i32(x);
            w.i32(y);
        }
        self.send(w.finish())?;
        let p = self.expect(wire::R_AT_MANY)?;
        let mut r = R::new(&p);
        let n = r.count(17).map_err(bad)?;
        if n != pts.len() {
            return Err(bad(format!("at_many: {n} replies for {} points", pts.len())));
        }
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            v.push(r.at().map_err(bad)?);
        }
        r.done().map_err(bad)?;
        Ok(v)
    }

    /// One Boxes message (patch 0005, E012's component end).
    fn boxes(&mut self) -> EResult<Boxes> {
        self.send(W::msg(wire::C_BOXES).finish())?;
        let p = self.expect(wire::R_BOXES)?;
        let mut r = R::new(&p);
        let b = r.boxes().map_err(bad)?;
        r.done().map_err(bad)?;
        Ok(b)
    }

    /// One InputLog message (patch 0007, E013's component refuse): the records since the last one, taken.
    fn input_log(&mut self) -> EResult<InputLog> {
        self.send(W::msg(wire::C_INPUT_LOG).finish())?;
        let p = self.expect(wire::R_INPUT_LOG)?;
        let mut r = R::new(&p);
        let l = r.input_log().map_err(bad)?;
        r.done().map_err(bad)?;
        Ok(l)
    }

    /// One Digest message (patch 0007).
    fn digest(&mut self) -> EResult<Digest> {
        self.send(W::msg(wire::C_DIGEST).finish())?;
        let p = self.expect(wire::R_DIGEST)?;
        let mut r = R::new(&p);
        let d = r.digest().map_err(bad)?;
        r.done().map_err(bad)?;
        Ok(d)
    }

    fn set_watch(&mut self, keys: Option<&[Key]>) {
        self.watch = keys.map(|k| k.to_vec());
    }

    /// E019 (FMI_STEP_TRACE=1 with a watch): the frames run by StepTrace, a batch at a time: the engine runs frame by
    /// frame and replies after the first frame that changed a watched key's bytes (or at the batch's end), with every
    /// frame's status and delta; `each` sees them frame by frame as `step_changes(1)` would give them. Otherwise one
    /// `step_changes(1)` per frame (the trait's default).
    fn wait(&mut self, frames: u64, each: &mut dyn FnMut(&Status, &[(Key, Val, Val)]) -> bool) -> EResult<(Status, u64)> {
        let trace = std::env::var("FMI_STEP_TRACE").is_ok_and(|v| v == "1");
        let mut st = self.status().clone();
        let mut n = 0u64;
        let Some(keys) = self.watch.clone().filter(|_| trace) else {
            while n < frames {
                let (s, ch) = self.step_changes(1)?;
                n += 1;
                st = s;
                if each(&st, &ch) {
                    break;
                }
            }
            return Ok((st, n));
        };
        while n < frames {
            self.block()?;
            let ranges = block::watch_ranges(&self.last, &keys);
            let batch = (frames - n).min(4096) as u32;
            let mut w = W::msg(wire::C_STEP_TRACE);
            w.u32(batch);
            w.u32(ranges.len() as u32);
            for (o, l) in &ranges {
                w.u32(*o);
                w.u32(*l);
            }
            self.send(w.finish())?;
            self.moved();
            let p = self.expect(wire::R_STEP_TRACE_REPLY)?;
            let mut r = R::new(&p);
            let count = r.u32().map_err(bad)?;
            let mut prev = std::mem::take(&mut self.prev);
            for i in 0..count {
                let status = r.status().map_err(bad)?;
                prev.clear();
                prev.extend_from_slice(&self.last);
                let mut rg = std::mem::take(&mut self.ranges);
                let delta = block::apply(&mut self.last, &mut r, &mut rg).map_err(bad)?;
                let ch = block::changes(&prev, &self.last, &rg, delta).map_err(bad)?;
                self.ranges = rg;
                self.last_delta = delta;
                self.last_facts = None;
                self.status = status.clone();
                st = status;
                n += 1;
                if each(&st, &ch) {
                    if i + 1 != count {
                        self.prev = prev;
                        return Err(bad(format!("step_trace: the wait stopped at frame {} of a batch of {count}", i + 1)));
                    }
                    r.done().map_err(bad)?;
                    self.prev = prev;
                    self.last_now = true;
                    return Ok((st, n));
                }
            }
            r.done().map_err(bad)?;
            self.prev = prev;
            self.last_now = true;
            if count == 0 {
                break;
            }
        }
        Ok((st, n))
    }

    /// One Step, its delta read into the facts' changes (block::changes) with no whole facts built (IFACE e011-r2-own).
    fn step_changes(&mut self, frames: u32) -> EResult<(Status, Vec<(Key, Val, Val)>)> {
        // the base: this stop's block (a State round trip unless a Step brought it)
        self.block()?;
        let mut prev = std::mem::take(&mut self.prev);
        prev.clear();
        prev.extend_from_slice(&self.last);
        let r = self.step(frames, false, false);
        let out = r.and_then(|o| {
            block::changes(&prev, &self.last, &self.ranges, self.last_delta).map(|c| (o.status, c)).map_err(bad)
        });
        self.prev = prev;
        out
    }
}

impl BinEngine {
    /// RunObs: the run, then the dump at its stop (`room`: the room's dump, fmi_core::room_cut's).
    fn run_obs_msg(&mut self, spec: &RunSpec, req: &ObsReq, room: bool) -> EResult<(Status, Obs)> {
        let mut w = W::msg(wire::C_RUN_OBS);
        w.run_spec(spec);
        w.obs_req(req);
        w.b(room);
        self.send(w.finish())?;
        self.moved();
        let p = self.expect(wire::R_RUN_OBS_REPLY)?;
        let mut r = R::new(&p);
        let st = r.status().map_err(bad)?;
        let o = r.obs().map_err(bad)?;
        r.done().map_err(bad)?;
        self.status = st.clone();
        Ok((st, o))
    }

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
