//! The test double of the engine port (plan.md, "play"; cargo feature `json-double`): `fmi_core::Engine` over the
//! bridge's JSON control protocol (patches/0001, speedrun_control.cpp; the Python client is speedrun/control.py), on
//! svm-w3b. One `Hub` per engine tree: it owns the control socket and the root process; every process of the tree
//! (the root, each fork, each fork_each child) connects to it on its own and is matched to its forker by pid.
//! Replaced at integration by fmi-engine (the binary channel); it exists for the differential tests.

use crate::pyj::{self, J};
pub use crate::replay::{event_j, event_of};
use fmi_core::*;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const MAX_FRAME: u64 = 200_000; // start.py MAX_FRAME
const MAX_TICKS: u64 = 10_000_000; // start.py MAX_TICKS

/// The repository root the engine's ini names (speedrun/paths.py ROOT): FMI_REPO_ROOT, or this crate's worktree.
pub fn repo_root() -> PathBuf {
    if let Ok(r) = std::env::var("FMI_REPO_ROOT") {
        return PathBuf::from(r);
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../..")
        .canonicalize()
        .expect("repo root")
}

/// The ScummVM binary: FAST_MI_SCUMMVM_BIN (svm-w3b's build in E009's env.sh).
pub fn scummvm_bin() -> PathBuf {
    match std::env::var("FAST_MI_SCUMMVM_BIN") {
        Ok(p) if !p.is_empty() => PathBuf::from(p),
        _ => repo_root().join("build/scummvm/scummvm"),
    }
}

struct HubInner {
    listener: UnixListener,
    stash: HashMap<u32, (BufReader<UnixStream>, J)>,
}

/// One engine tree's control socket and root process.
pub struct Hub {
    inner: Mutex<HubInner>,
    dir: PathBuf,
    root: Mutex<Option<Child>>,
    timeout: Duration,
}

impl Drop for Hub {
    fn drop(&mut self) {
        if let Some(mut c) = self.root.lock().unwrap().take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Hub {
    /// The connection of process `pid` (its hello read): accepted now or stashed by another taker.
    fn take(&self, pid: Option<u32>) -> EResult<(BufReader<UnixStream>, J)> {
        let mut g = self.inner.lock().unwrap();
        loop {
            if let Some(p) = pid {
                if let Some(c) = g.stash.remove(&p) {
                    return Ok(c);
                }
            } else if let Some(&p) = g.stash.keys().next() {
                return Ok(g.stash.remove(&p).unwrap());
            }
            let (s, _) = g.listener.accept().map_err(EngineError::Io)?;
            s.set_read_timeout(Some(self.timeout)).map_err(EngineError::Io)?;
            let mut r = BufReader::with_capacity(1 << 16, s);
            let mut line = String::new();
            r.read_line(&mut line).map_err(EngineError::Io)?;
            let hello = pyj::parse(line.trim_end()).map_err(EngineError::Bridge)?;
            let hp = hello.i("hello").unwrap_or(0) as u32;
            if pid.is_none() || pid == Some(hp) {
                return Ok((r, hello));
            }
            g.stash.insert(hp, (r, hello));
        }
    }
}

/// The engine settings of replay_part1.config(seed, out, inputs, dump_locals) as speedrun/engine.py writes them.
pub fn write_config(
    cfg: &BootCfg,
    inputs: Option<&Path>,
    sock: &Path,
) -> std::io::Result<(Vec<String>, Vec<(String, String)>)> {
    let root = repo_root();
    let out = &cfg.out_dir;
    let saves = root.join("out/scummvm/saves");
    for d in [
        out.clone(),
        root.join("out/scummvm"),
        saves.clone(),
        out.join("xdg/data"),
        out.join("xdg/cache"),
    ] {
        std::fs::create_dir_all(&d)?;
    }
    let game = root.join("game/dos-cd");
    let game = game.canonicalize().unwrap_or(game);
    let ini = format!(
        "[scummvm]\nsavepath={}\nautosave_period=0\nconfirm_exit=false\nfullscreen=false\n\
         gui_return_to_launcher_at_exit=false\nvsync=false\nextrapath={}\nthemepath={}\n\n\
         [monkey-cd]\ngameid=monkey\nengineid=scumm\npath={}\nplatform=pc\nlanguage=en\ncopy_protection=false\n\
         autosave_period=0\nsubtitles=true\noriginal_gui=false\nenhancements=0\ntalkspeed=255\n",
        saves.display(),
        root.join("third_party/scummvm/dists/engine-data").display(),
        root.join("third_party/scummvm/gui/themes").display(),
        game.display()
    );
    let ini_path = out.join("scummvm.ini");
    std::fs::write(&ini_path, ini)?;
    let argv = vec![
        scummvm_bin().display().to_string(),
        format!("--config={}", ini_path.display()),
        format!("--savepath={}", saves.display()),
        "--boot-param=0".into(),
        "--disable-sdl-audio".into(),
        format!("--random-seed={}", cfg.seed),
        "monkey-cd".into(),
    ];
    let mut env = vec![
        ("SPEEDRUN_OUT".to_string(), out.display().to_string()),
        ("SPEEDRUN_START".into(), "[]".into()),
        ("SPEEDRUN_GOAL".into(), "[]".into()),
        ("SPEEDRUN_FAST".into(), "1".into()),
        ("SPEEDRUN_MAX_TICKS".into(), MAX_TICKS.to_string()),
    ];
    if let Some(i) = inputs {
        env.push(("SPEEDRUN_INPUTS".into(), i.display().to_string()));
    }
    env.push(("SPEEDRUN_TIMER_NEXT".into(), "any".into()));
    if cfg.dump_locals {
        env.push(("SPEEDRUN_DUMP_LOCALS".into(), "1".into()));
    }
    if cfg.lean {
        env.push(("SPEEDRUN_LEAN".into(), "1".into()));
    }
    env.push(("XDG_DATA_HOME".into(), out.join("xdg/data").display().to_string()));
    env.push(("XDG_CACHE_HOME".into(), out.join("xdg/cache").display().to_string()));
    env.push(("SDL_VIDEODRIVER".into(), "dummy".into()));
    env.push(("SDL_AUDIODRIVER".into(), "dummy".into()));
    env.push(("SPEEDRUN_CONTROL".into(), sock.display().to_string()));
    Ok((argv, env))
}

/// The status of a stop reply, or of an end.
pub fn status_of(j: &J) -> Status {
    if let Some(end) = j.get("end") {
        return Status {
            reason: end.s("reason").map(String::from).unwrap_or_else(|| pyj::dumps(end)),
            ended: true,
            frame: end.i("frame").unwrap_or(0) as u64,
            tick: end.i("tick").unwrap_or(0) as u64,
            qf: end.i("qf").unwrap_or(0) as u64,
            room: end.i("room").unwrap_or(0) as i32,
            ..Default::default()
        };
    }
    let g = |k: &str| j.i(k).unwrap_or(0);
    let ego = j.get("ego").and_then(|e| {
        let a = e.arr();
        Some((a.first()?.int()? as i32, a.get(1)?.int()? as i32))
    });
    Status {
        reason: j.s("stop").unwrap_or("").to_string(),
        ended: false,
        frame: g("frame") as u64,
        tick: g("tick") as u64,
        qf: g("qf") as u64,
        qf0: g("qf0") as u64,
        room: g("room") as i32,
        up: g("up") as i32,
        cur: g("cur") as i32,
        cs: j.i("cs").unwrap_or(1) as i32,
        v19: g("v19") as i32,
        seg: j.get("seg").map(J::truthy).unwrap_or(false),
        tick0: g("tick0") as u64,
        rng: g("rng") as u64,
        idle: j.s("idle").map(String::from),
        blk: j.s("blk").map(String::from),
        ego,
        mv: j.i("mv").map(|n| n as i32),
        walkbox: j.i("box").map(|n| n as i32),
        cam: g("cam") as i32,
        xstart: g("xstart") as i32,
        room_w: g("room_w") as i32,
        forks: j
            .a("forks")
            .iter()
            .filter_map(|p| Some((p.arr().first()?.int()? as u64, p.arr().get(1)?.int()? as u32)))
            .collect(),
        // the JSON protocol does not report the screen layout (patch 0004's binary status does)
        screen: None,
    }
}

fn ints(j: &J) -> Vec<i32> {
    j.arr().iter().filter_map(|x| x.int().map(|n| n as i32)).collect()
}

/// The C7 dump's JSON as `Obs` (names kept only when asked).
pub fn obs_of(s: &J, names: bool) -> Obs {
    let i = |k: &str| s.i(k).unwrap_or(0);
    let pairs = |k: &str| -> Vec<(u32, i32)> {
        s.get(k)
            .map(|o| {
                o.obj()
                    .iter()
                    .map(|(a, v)| (a.parse().unwrap_or(0), v.int().unwrap_or(0) as i32))
                    .collect()
            })
            .unwrap_or_default()
    };
    Obs {
        tick: i("tick") as u64,
        frame: i("frame") as u64,
        room: i("room") as i32,
        ego: s.get("ego").and_then(J::int).map(|n| n as i32),
        ego_pos: s.get("ego_pos").and_then(|e| {
            let a = e.arr();
            Some((a.first()?.int()? as i32, a.get(1)?.int()? as i32))
        }),
        seed: s.get("seed").and_then(J::int),
        boot_param: i("boot_param") as i32,
        vars: ints(s.get("vars").unwrap_or(&J::Null)),
        bits_set: s
            .a("bits_set")
            .iter()
            .filter_map(|x| x.int().map(|n| n as u32))
            .collect(),
        inventory: s.get("inventory").map(ints),
        owners: pairs("owners"),
        states: pairs("states"),
        classes: s
            .get("classes")
            .map(|o| {
                o.obj()
                    .iter()
                    .map(|(a, v)| {
                        (
                            a.parse().unwrap_or(0),
                            v.arr().iter().filter_map(|x| x.int().map(|n| n as u32)).collect(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default(),
        room_objects: s.get("room_objects").map(|a| {
            a.arr()
                .iter()
                .map(|o| RoomObject {
                    id: o.i("id").unwrap_or(0) as i32,
                    state: o.i("state").unwrap_or(0) as i32,
                    parent: o.i("parent").unwrap_or(0) as i32,
                    x: o.i("x").unwrap_or(0) as i32,
                    y: o.i("y").unwrap_or(0) as i32,
                    w: o.i("w").unwrap_or(0) as i32,
                    h: o.i("h").unwrap_or(0) as i32,
                    walk: {
                        let w = o.a("walk");
                        (
                            w.first().and_then(J::int).unwrap_or(0) as i32,
                            w.get(1).and_then(J::int).unwrap_or(0) as i32,
                        )
                    },
                    name: if names { o.s("name").map(String::from) } else { None },
                })
                .collect()
        }),
        verbs: s
            .a("verbs")
            .iter()
            .map(|v| Verb {
                slot: v.i("slot").unwrap_or(0) as i32,
                id: v.i("id").unwrap_or(0) as i32,
                mode: v.i("mode").unwrap_or(0) as i32,
                saveid: v.i("saveid").unwrap_or(0) as i32,
                kind: v.i("type").unwrap_or(0) as i32,
                key: v.i("key").unwrap_or(0) as i32,
                x: v.i("x").unwrap_or(0) as i32,
                y: v.i("y").unwrap_or(0) as i32,
                name: if names { v.s("name").map(String::from) } else { None },
            })
            .collect(),
        slots: s
            .a("slots")
            .iter()
            .map(|o| Slot {
                slot: o.i("slot").unwrap_or(0) as i32,
                number: o.i("number").unwrap_or(0) as i32,
                where_: o.i("where").unwrap_or(0) as i32,
                status: o.i("status").unwrap_or(0) as i32,
                offs: o.i("offs").unwrap_or(0) as u64,
                code: o.i("code").unwrap_or(-1),
                frozen: o.i("frozen").unwrap_or(0) as i32,
                delay: o.i("delay").unwrap_or(0) as i32,
                cutscene_override: o.i("cutscene_override").unwrap_or(0) as i32,
                locals: o.get("locals").map(ints),
                resistant: o.get("resistant").map(J::truthy),
                recursive: o.get("recursive").map(J::truthy),
            })
            .collect(),
        cutscene: {
            let c = s.get("cutscene").cloned().unwrap_or(J::Null);
            Cutscene {
                depth: c.i("depth").unwrap_or(0) as i32,
                levels: c
                    .a("levels")
                    .iter()
                    .map(|l| CutLevel {
                        level: l.i("level").unwrap_or(0) as i32,
                        data: l.i("data").map(|n| n as i32),
                        override_: l.get("override").filter(|o| !o.is_null()).map(|o| Override {
                            slot: o.i("slot").unwrap_or(0) as i32,
                            number: o.i("number").unwrap_or(0) as i32,
                            where_: o.i("where").unwrap_or(0) as i32,
                            status: o.i("status").unwrap_or(0) as i32,
                            ptr: o.i("ptr").unwrap_or(0) as u64,
                            code: o.i("code").unwrap_or(-1),
                            stale: o.get("stale").map(J::truthy).unwrap_or(false),
                        }),
                    })
                    .collect(),
            }
        },
        actors: s
            .a("actors")
            .iter()
            .map(|a| ActorPos {
                id: a.i("id").unwrap_or(0) as i32,
                x: a.i("x").unwrap_or(0) as i32,
                y: a.i("y").unwrap_or(0) as i32,
                elevation: a.i("elevation").unwrap_or(0) as i32,
                moving: a.i("moving").unwrap_or(0) as i32,
            })
            .collect(),
        exact: None,
    }
}

fn cond_j(c: &Cond) -> J {
    let kind = match c.kind {
        CondKind::Var => "var",
        CondKind::Bit => "bit",
        CondKind::Room => "room",
        CondKind::Owner => "owner",
        CondKind::State => "state",
        CondKind::Has => "has",
        CondKind::ActorRoom => "actor_room",
        CondKind::ActorX => "actor_x",
        CondKind::ActorY => "actor_y",
        CondKind::Class => "class",
        CondKind::ActorMoving => "actor_moving",
        CondKind::Script => "script",
    };
    let mut o = vec![(kind.to_string(), J::Int(c.subject as i64))];
    if !matches!(c.kind, CondKind::Room | CondKind::Has) {
        match c.op {
            CondOp::Eq => o.push(("eq".into(), J::Int(c.value as i64))),
            CondOp::Le => o.push(("le".into(), J::Int(c.value as i64))),
            CondOp::Ge => o.push(("ge".into(), J::Int(c.value as i64))),
            CondOp::In => o.push((
                "in".into(),
                J::Arr(c.values.iter().map(|&v| J::Int(v as i64)).collect()),
            )),
        }
    }
    if c.negate {
        return J::Obj(vec![("not".into(), J::Obj(o))]);
    }
    J::Obj(o)
}

/// One engine process over the JSON protocol.
pub struct JsonEngine {
    r: BufReader<UnixStream>,
    pid: u32,
    st: Status,
    hub: Arc<Hub>,
    line: String,
    pub out: String,
}

fn proc_cpu(pid: u32) -> f64 {
    let Ok(t) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
        return 0.0;
    };
    let Some(p) = t.rfind(')') else { return 0.0 };
    let f: Vec<&str> = t[p + 2..].split_whitespace().collect();
    let tck = unsafe { libc::sysconf(libc::_SC_CLK_TCK) } as f64;
    let u: f64 = f.get(11).and_then(|x| x.parse().ok()).unwrap_or(0.0);
    let s: f64 = f.get(12).and_then(|x| x.parse().ok()).unwrap_or(0.0);
    (u + s) / tck
}

impl JsonEngine {
    fn send(&mut self, j: &J) -> EResult<()> {
        let mut s = pyj::dumps(j);
        s.push('\n');
        self.r.get_mut().write_all(s.as_bytes()).map_err(EngineError::Io)
    }
    fn send_raw(&mut self, s: &str) -> EResult<()> {
        self.r.get_mut().write_all(s.as_bytes()).map_err(EngineError::Io)
    }
    /// The next reply line (an "error" reply is an error).
    fn recv_line(&mut self) -> EResult<&str> {
        self.line.clear();
        let n = self.r.read_line(&mut self.line).map_err(EngineError::Io)?;
        if n == 0 {
            return Err(EngineError::Bridge(format!(
                "engine {} closed the connection",
                self.pid
            )));
        }
        if self.line.starts_with("{\"error\"") {
            let j = pyj::parse(self.line.trim_end()).map_err(EngineError::Bridge)?;
            return Err(EngineError::Bridge(format!(
                "engine {}: {}",
                self.pid,
                j.s("error").unwrap_or("")
            )));
        }
        Ok(self.line.trim_end())
    }
    fn recv(&mut self) -> EResult<J> {
        let l = self.recv_line()?;
        pyj::parse(l).map_err(EngineError::Bridge)
    }
    fn await_stop(&mut self) -> EResult<Status> {
        let j = self.recv()?;
        let st = status_of(&j);
        if st.ended {
            self.st = Status::default();
            return Err(EngineError::Ended(Box::new(st)));
        }
        self.st = st.clone();
        Ok(st)
    }
    fn connect(hub: &Arc<Hub>, pid: Option<u32>) -> EResult<JsonEngine> {
        let (r, hello) = hub.take(pid)?;
        let mut e = JsonEngine {
            r,
            pid: hello.i("hello").unwrap_or(0) as u32,
            st: Status::default(),
            hub: hub.clone(),
            line: String::new(),
            out: String::new(),
        };
        e.st.frame = hello.i("frame").unwrap_or(0) as u64;
        e.st.tick = hello.i("tick").unwrap_or(0) as u64;
        Ok(e)
    }
    /// The C7 dump as JSON (the reference's dict), for the differential tests.
    pub fn state_j(&mut self, objects: Option<&[u32]>) -> EResult<J> {
        match objects {
            Some(o) => {
                let mut s = String::from("{\"cmd\": \"state\", \"objects\": [");
                for (i, x) in o.iter().enumerate() {
                    if i > 0 {
                        s.push_str(", ");
                    }
                    s.push_str(&x.to_string());
                }
                s.push_str("]}\n");
                self.send_raw(&s)?;
            }
            None => self.send_raw("{\"cmd\": \"state\"}\n")?,
        }
        let j = self.recv()?;
        Ok(j.get("state").cloned().unwrap_or(J::Null))
    }
}

impl Engine for JsonEngine {
    fn pid(&self) -> u32 {
        self.pid
    }
    fn status(&self) -> &Status {
        &self.st
    }
    fn run(&mut self, spec: &RunSpec) -> EResult<Status> {
        let mut o = vec![("cmd".to_string(), J::Str("run".into()))];
        if spec.idle {
            o.push(("idle".into(), J::Bool(true)));
        }
        if spec.menu {
            o.push(("menu".into(), J::Bool(true)));
        }
        if !spec.until_any.is_empty() {
            o.push((
                "until_any".into(),
                J::Arr(
                    spec.until_any
                        .iter()
                        .map(|g| J::Arr(g.iter().map(cond_j).collect()))
                        .collect(),
                ),
            ));
        }
        if let Some(f) = spec.frame {
            o.push(("frame".into(), J::Int(f as i64)));
        }
        if !spec.until.is_empty() {
            o.push(("until".into(), J::Arr(spec.until.iter().map(cond_j).collect())));
        }
        if spec.control {
            o.push(("control".into(), J::Bool(true)));
        }
        if let Some(m) = spec.max_frame {
            o.push(("max_frame".into(), J::Int(m as i64)));
        }
        self.send(&J::Obj(o))?;
        self.await_stop()
    }
    fn inputs(&mut self, events: &[Event]) -> EResult<()> {
        if events.is_empty() {
            return Ok(());
        }
        let j = J::Obj(vec![
            ("cmd".into(), J::Str("inputs".into())),
            ("events".into(), J::Arr(events.iter().map(event_j).collect())),
        ]);
        self.send(&j)?;
        self.recv_line()?;
        Ok(())
    }
    fn obs(&mut self, req: &ObsReq) -> EResult<Obs> {
        let s = self.state_j(req.objects.as_deref())?;
        Ok(obs_of(&s, req.names))
    }
    fn facts(&mut self) -> EResult<Facts> {
        let o = self.obs(&ObsReq::default())?;
        Ok(facts_of(&o))
    }
    fn holds(&mut self, conds: &[Cond]) -> EResult<bool> {
        let j = J::Obj(vec![
            ("cmd".into(), J::Str("holds".into())),
            ("until".into(), J::Arr(conds.iter().map(cond_j).collect())),
        ]);
        self.send(&j)?;
        let r = self.recv()?;
        Ok(r.get("holds").map(J::truthy).unwrap_or(false))
    }
    fn at(&mut self, x: i32, y: i32) -> EResult<At> {
        self.send_raw(&format!("{{\"cmd\": \"at\", \"x\": {x}, \"y\": {y}}}\n"))?;
        let r = self.recv()?;
        let a = r.get("at").cloned().unwrap_or(J::Null);
        Ok(At {
            x: a.i("x").unwrap_or(x as i64) as i32,
            y: a.i("y").unwrap_or(y as i64) as i32,
            zone: a.i("zone").unwrap_or(-1) as i32,
            main: a.i("vx").map(|vx| {
                (
                    vx as i32,
                    a.i("vy").unwrap_or(0) as i32,
                    a.i("obj").unwrap_or(0) as i32,
                    a.i("actor").unwrap_or(0) as i32,
                )
            }),
            verb_slot: a.i("verb_slot").unwrap_or(0) as i32,
        })
    }
    fn set_rng(&mut self, state: u32) -> EResult<()> {
        self.send_raw(&format!("{{\"cmd\": \"set_rng\", \"state\": {state}}}\n"))?;
        self.recv_line()?;
        Ok(())
    }
    fn segment(&mut self, on: bool) -> EResult<()> {
        self.send_raw(&format!("{{\"cmd\": \"segment\", \"on\": {on}}}\n"))?;
        self.recv_line()?;
        Ok(())
    }
    fn runs(&mut self) -> EResult<RunsReply> {
        self.send_raw("{\"cmd\": \"runs\"}\n")?;
        let r = self.recv()?;
        let runs = r
            .a("runs")
            .iter()
            .map(|row| {
                let a = row.arr();
                let g = |i: usize| a.get(i).and_then(J::int).unwrap_or(0);
                Run {
                    frame: g(0) as u64,
                    after: g(1) != 0,
                    seq: g(2) as u64,
                    depth: g(3) as i32,
                    slot: g(4) as i32,
                    where_: g(5) as i32,
                    number: g(6) as i32,
                    room: g(7) as i32,
                    code0: g(8),
                    code1: g(9),
                    status: g(10) as i32,
                    frozen: g(11) as i32,
                    delay: g(12) as i32,
                    ops: g(13) as u64,
                }
            })
            .collect();
        Ok(RunsReply {
            runs,
            dropped: r.i("dropped").unwrap_or(0) as u64,
            violations: r.i("violations").unwrap_or(0) as u64,
            frame: r.i("frame").unwrap_or(0) as u64,
        })
    }
    fn explain(&mut self, arm: Option<&ExplainArm>) -> EResult<()> {
        let j = match arm {
            None => J::Obj(vec![
                ("cmd".into(), J::Str("explain".into())),
                ("off".into(), J::Bool(true)),
            ]),
            Some(a) => {
                let mut o = vec![
                    ("cmd".to_string(), J::Str("explain".into())),
                    ("from".into(), J::Int(a.from as i64)),
                    ("to".into(), J::Int(a.to as i64)),
                ];
                if !a.units.is_empty() {
                    o.push((
                        "units".into(),
                        J::Arr(
                            a.units
                                .iter()
                                .map(|(w, n)| J::Arr(vec![J::Int(*w as i64), J::Int(*n as i64)]))
                                .collect(),
                        ),
                    ));
                }
                J::Obj(o)
            }
        };
        self.send(&j)?;
        self.recv_line()?;
        Ok(())
    }
    fn explain_log(&mut self) -> EResult<ExplainReply> {
        self.send_raw("{\"cmd\": \"explain_log\"}\n")?;
        let r = self.recv()?;
        let mut entries = Vec::new();
        for e in r.a("explain") {
            let a = e.arr();
            let g = |i: usize| a.get(i).and_then(J::int).unwrap_or(0);
            let seq = g(1) as u64;
            let x = match a.first().and_then(J::str) {
                Some("b") => Explain::Begin {
                    seq,
                    frame: g(2) as u64,
                    after: g(3) != 0,
                    slot: g(4) as i32,
                    where_: g(5) as i32,
                    number: g(6) as i32,
                    room: g(7) as i32,
                    code0: g(8),
                },
                Some("o") => Explain::Op {
                    seq,
                    code: g(2),
                    opcode: g(3) as u8,
                },
                Some("r") => Explain::Read {
                    seq,
                    var: g(2) as u32,
                    value: g(3) as i32,
                },
                Some("w") => Explain::Write {
                    seq,
                    var: g(2) as u32,
                    value: g(3) as i32,
                },
                Some("e") => Explain::End {
                    seq,
                    code1: g(2),
                    status: g(3) as i32,
                    frozen: g(4) as i32,
                    delay: g(5) as i32,
                },
                _ => continue,
            };
            entries.push(x);
        }
        Ok(ExplainReply {
            entries,
            dropped: r.i("dropped").unwrap_or(0) as u64,
            violations: r.i("violations").unwrap_or(0) as u64,
        })
    }
    fn fork(&mut self) -> EResult<Box<dyn Engine>> {
        self.send_raw("{\"cmd\": \"fork\"}\n")?;
        let r = self.recv()?;
        let pid = r
            .i("forked")
            .ok_or_else(|| EngineError::Bridge(format!("fork: unexpected reply {}", pyj::dumps(&r))))?;
        let mut c = JsonEngine::connect(&self.hub, Some(pid as u32))?;
        c.await_stop()?; // its "forked" status
        Ok(Box::new(c))
    }
    fn fork_each(&mut self, n: u32) -> EResult<(Status, Vec<Box<dyn Engine>>)> {
        self.send_raw(&format!("{{\"cmd\": \"run\", \"fork_each\": {n}}}\n"))?;
        let st = self.await_stop()?;
        let mut kids: Vec<(u64, Box<dyn Engine>)> = Vec::new();
        for &(frame, pid) in &st.forks {
            let mut c = JsonEngine::connect(&self.hub, Some(pid))?;
            c.await_stop()?;
            kids.push((frame, Box::new(c)));
        }
        kids.sort_by_key(|k| k.0);
        Ok((st, kids.into_iter().map(|k| k.1).collect()))
    }
    fn state_json(&mut self, req: &ObsReq) -> EResult<String> {
        let s = self.state_j(req.objects.as_deref())?;
        Ok(pyj::dumps(&s))
    }
    fn cpu_s(&self) -> f64 {
        proc_cpu(self.pid)
    }
    fn quit(mut self: Box<Self>) {
        let _ = self.send_raw("{\"cmd\": \"quit\"}\n");
    }
}

/// Starts engines on svm-w3b through the JSON protocol (replay_part1.config's settings).
#[derive(Default)]
pub struct JsonHost {
    pub timeout_s: u64,
}

impl JsonHost {
    pub fn boot_json(&self, cfg: &BootCfg) -> EResult<JsonEngine> {
        std::fs::create_dir_all(&cfg.out_dir).map_err(EngineError::Io)?;
        let mut evs = cfg.events.clone();
        evs.sort_by_key(|e| e.frame);
        let inputs = cfg.out_dir.join("prefix.jsonl");
        let text: String = evs
            .iter()
            .filter(|e| cfg.frame.is_none_or(|f| e.frame <= f))
            .map(|e| pyj::dumps(&event_j(e)) + "\n")
            .collect();
        std::fs::write(&inputs, text).map_err(EngineError::Io)?;
        let dir = std::env::temp_dir().join(format!("fmi-ctl-{}-{}", std::process::id(), unique()));
        std::fs::create_dir_all(&dir).map_err(EngineError::Io)?;
        let sock = dir.join("control.sock");
        let listener = UnixListener::bind(&sock).map_err(EngineError::Io)?;
        let (argv, env) = write_config(cfg, Some(&inputs), &sock).map_err(EngineError::Io)?;
        let log = std::fs::File::create(cfg.out_dir.join("stdout.log")).map_err(EngineError::Io)?;
        let mut cmd = Command::new(&argv[0]);
        cmd.args(&argv[1..])
            .stdin(Stdio::null())
            .stdout(log.try_clone().map_err(EngineError::Io)?)
            .stderr(log);
        cmd.env_clear();
        for (k, v) in std::env::vars() {
            if !k.starts_with("SPEEDRUN_") {
                cmd.env(k, v);
            }
        }
        for (k, v) in env {
            cmd.env(k, v);
        }
        let child = cmd.spawn().map_err(EngineError::Io)?;
        let hub = Arc::new(Hub {
            inner: Mutex::new(HubInner {
                listener,
                stash: HashMap::new(),
            }),
            dir,
            root: Mutex::new(Some(child)),
            timeout: Duration::from_secs(if self.timeout_s > 0 { self.timeout_s } else { 900 }),
        });
        let mut e = JsonEngine::connect(&hub, None)?;
        e.await_stop()?;
        match cfg.frame {
            Some(f) => {
                let st = e.run(&RunSpec {
                    frame: Some(f),
                    ..Default::default()
                })?;
                if st.frame != f {
                    return Err(EngineError::Bridge(format!(
                        "the boot replay stopped at {} instead of frame {f}",
                        st.frame
                    )));
                }
            }
            None => {
                e.run(&RunSpec {
                    idle: true,
                    max_frame: Some(MAX_FRAME),
                    ..Default::default()
                })?;
            }
        }
        Ok(e)
    }
}

fn unique() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    N.fetch_add(1, Ordering::Relaxed)
}

impl EngineHost for JsonHost {
    fn boot(&self, cfg: &BootCfg) -> EResult<Box<dyn Engine>> {
        Ok(Box::new(self.boot_json(cfg)?))
    }
}
