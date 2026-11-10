//! direct: a search in the engine itself (a spike, not yet an experiment's component).
//!
//! From the boot's first idle stop, every input the interface offers at a stop is tried in a fork of the engine, and a
//! stop that holds something new is kept and expanded in turn. Nothing here reads the scripts: the engine is the model.
//!
//!   direct OUT [--minutes M] [--workers W] [--horizon H] [--no-skips] [--watch KEY=V,...]
//!
//! * **Inputs** are click sequences, found breadth first at a stop (`clicks`). The clickable points are afford's offer
//!   (the visible verb slots, the objects and actors the hit test answers) and a grid of floor points. A click that
//!   leaves the game composing a sentence (afford's `composing`: only the input script ran, and ego stands where it
//!   stood) gives a stop to click on from, unless an earlier sequence showed the same interface over the same world;
//!   any other click ends its sequence. At most MAX_CLICKS clicks (a verb and two objects).
//! * **A trial** is a fork, the clicks, then a run to the next stop where the bridge is idle and ego stands still
//!   (`calm`), or the horizon.
//! * **The game's own moves.** Each stop is also run with no input for WAITS frames. Where that changes the world, the
//!   inputs are tried again from there (a door a timer opens is open only then).
//! * **Kept.** A stop's atoms are its room, owners, states, classes and bits (the world); what the interface shows; the
//!   objects and actors on the screen; ego's walk box and the camera's bucket. A stop goes to the first queue it fits:
//!   (0) a world atom no kept stop holds; (1) an interface or a thing on the screen none holds in that room; (2) a
//!   walk box or camera bucket none holds in that room; (3) a world atom none holds in that room; (4) a world none
//!   holds (its atoms together, without those that change by themselves). A stop that fits none is dropped. Queue 0
//!   is expanded first, then 1, and so on.
//!
//! OUT/nodes.jsonl holds one line per kept stop (its parent, clicks and movie, so it replays from boot); offers.jsonl
//! one line per expanded stop.

use fmi_core::*;
use fmi_engine::{Host, HostCfg};
use fmi_explore::afford::{self, Hit, Point};
use serde_json::{json, Value};
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// The longest click sequence of one input (a verb and two objects).
const MAX_CLICKS: usize = 3;
/// Frames a stop is held before it is read (`calm`).
const CALM: u64 = 6;
/// The game's own moves: frames run with no input, each one trial of a stop.
const WAITS: [u64; 3] = [50, 200, 800];
/// Pixels of the camera's bucket: a fifth of the screen, so a walk across part of it shows.
const CAM_BUCKET: i32 = 64;
/// The floor clicks' grid over the main screen: the columns, and the rows in eighths of its height.
const FLOOR_X: [i32; 7] = [6, 58, 110, 160, 210, 262, 313];
const FLOOR_Y: [i32; 4] = [3, 5, 7, 8];
/// Engines kept alive for stops waiting in a queue; the others are booted again from their movies.
const LIVE_MAX: usize = 64;
/// The queues (the module's text, "Kept").
const QUEUES: usize = 5;

struct Node {
    events: Vec<Event>,
    frame: u64,
    depth: u32,
}

/// The verb slots (slot, id) whose text follows the pointer (the sentence line names what the pointer is over): no
/// part of a stop's state.
type Echo = HashSet<(i32, i32)>;

#[derive(Default)]
struct Shared {
    nodes: Vec<Node>,
    queues: [VecDeque<usize>; QUEUES],
    live: HashMap<usize, Box<dyn Engine>>,
    seen: HashSet<(Key, Val)>,
    seen_at: HashSet<(i32, Key, Val)>,
    sees: HashSet<(i32, i32)>,
    ifaces: HashSet<(i32, u64)>,
    boxes: HashSet<(i32, i32)>,
    cams: HashSet<(i32, i32)>,
    cells: HashSet<u64>,
    echo: Echo,
    /// the world keys a run with no input changed somewhere: left out of the cells
    ambient: HashSet<Key>,
    rooms: Vec<i32>,
    busy: usize,
    done: bool,
    trials: u64,
    refused: u64,
    not_idle: u64,
    expanded: u64,
    boots: u64,
    kept: [u64; QUEUES],
    ends: Vec<(usize, String)>,
    watch: Vec<(Key, Val, Option<(usize, f64)>)>,
    log: Option<std::fs::File>,
    offers: Option<std::fs::File>,
    t0: Option<Instant>,
}

struct Cfg {
    out: PathBuf,
    horizon: u64,
}

static DIRS: AtomicU64 = AtomicU64::new(0);

fn eng_dir(out: &Path) -> PathBuf {
    let d = out.join("eng").join(format!("b{}", DIRS.fetch_add(1, Ordering::Relaxed)));
    std::fs::create_dir_all(&d).expect("an engine folder");
    d
}

fn world_kind(k: Key) -> bool {
    matches!(k.kind(), K_ROOM | K_OWNER | K_STATE | K_CLASS | K_BIT)
}

/// A stop as the search reads it (the module's text, "Kept").
struct View {
    room: i32,
    world: Vec<(Key, Val)>,
    /// the objects whose box is partly on the screen, and the actors on it (as negative ids)
    vis: Vec<i32>,
    walkbox: i32,
    cam: i32,
    /// what the interface shows: every slot (a sentence half made), and without the slots that follow the pointer
    iface_full: u64,
    iface: u64,
}

const NAMES: ObsReq = ObsReq { objects: None, names: true, exact: false };

/// What the interface shows at a stop: the visible verb slots with their ids and texts (the verbs, the inventory's
/// names, the sentence being composed, a dialog's choices), without the slots of `echo`. Only the hash is kept: no
/// game text leaves the process.
fn iface(o: &Obs, echo: Option<&Echo>) -> u64 {
    let mut v: Vec<(i32, i32, &str)> = o
        .verbs
        .iter()
        .filter(|x| echo.map_or(true, |e| !e.contains(&(x.slot, x.id))))
        .map(|x| (x.slot, x.id, x.name.as_deref().unwrap_or("")))
        .collect();
    v.sort_unstable();
    let mut h = DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}

fn view(o: &Obs, f: &Facts, st: &Status, echo: &Echo) -> View {
    let room = f.val(Key::pack(K_ROOM, 0, 0, 0)) as i32;
    let (x0, x1) = (st.xstart, st.xstart + afford::SCREEN_W);
    let mut vis: Vec<i32> = o
        .room_objects
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .filter(|r| r.w > 0 && r.h > 0 && r.x < x1 && r.x + r.w > x0)
        .map(|r| r.id)
        .chain(o.actors.iter().filter(|a| a.x >= x0 && a.x < x1).map(|a| -a.id))
        .collect();
    vis.sort_unstable();
    View {
        room,
        world: f.iter().filter(|p| world_kind(p.0)).copied().collect(),
        vis,
        walkbox: st.walkbox.unwrap_or(-1),
        cam: st.xstart.div_euclid(CAM_BUCKET),
        iface_full: iface(o, None),
        iface: iface(o, Some(echo)),
    }
}

impl View {
    /// The world's cell: the room, the world's atoms but the ambient keys', and the interface, wherever ego stands.
    fn cell(&self, ambient: &HashSet<Key>) -> u64 {
        let mut h = DefaultHasher::new();
        (self.room, self.iface).hash(&mut h);
        for p in self.world.iter().filter(|p| !ambient.contains(&p.0)) {
            p.hash(&mut h);
        }
        h.finish()
    }
    /// A stop inside a sentence: what the interface shows over the world (two with this hash answer a click alike).
    fn mid(&self) -> u64 {
        let mut h = DefaultHasher::new();
        (&self.world, self.iface_full).hash(&mut h);
        h.finish()
    }
}

/// One trial's stop.
struct Res {
    name: String,
    events: Vec<Event>,
    st: Status,
    facts: Facts,
    view: View,
}

enum Played {
    Stop(Status),
    Ended(Status),
    NotIdle,
}

fn go(b: &mut Box<dyn Engine>, spec: RunSpec) -> EResult<Result<Status, Status>> {
    match b.run(&spec) {
        Ok(s) => Ok(Ok(s)),
        Err(EngineError::Ended(s)) => Ok(Err(*s)),
        Err(x) => Err(x),
    }
}

/// On to a stop where the bridge is idle and ego stands still, or `end`.
fn settle(b: &mut Box<dyn Engine>, end: u64) -> EResult<Played> {
    loop {
        let st = b.status().clone();
        if st.frame >= end {
            return Ok(Played::NotIdle);
        }
        if st.idle.is_some() && st.mv.unwrap_or(0) == 0 {
            return Ok(Played::Stop(st));
        }
        let spec = if st.idle.is_some() {
            // idle, ego still walking: a few frames on
            RunSpec { frame: Some((st.frame + 4).min(end)), ..Default::default() }
        } else {
            RunSpec { idle: true, max_frame: Some(end), ..Default::default() }
        };
        if let Err(s) = go(b, spec)? {
            return Ok(Played::Ended(s));
        }
    }
}

/// `settle`, then CALM frames on and `settle` again: the texts an input leaves on the interface for a moment are gone.
fn calm(b: &mut Box<dyn Engine>, end: u64) -> EResult<Played> {
    match settle(b, end)? {
        Played::Stop(st) => {
            if let Err(s) = go(b, RunSpec { frame: Some(st.frame + CALM), ..Default::default() })? {
                return Ok(Played::Ended(s));
            }
            settle(b, end.max(st.frame + CALM + 1))
        }
        x => Ok(x),
    }
}

fn name_of(pre: &str, clicks: &[Point]) -> String {
    format!("{pre}{}", clicks.iter().map(|p| p.hit.name()).collect::<String>())
}

/// A stop inside a sentence being composed: its engine (None: the round's own), and the clicks that led to it.
struct Mid {
    engine: Option<Box<dyn Engine>>,
    clicks: Vec<Point>,
    events: Vec<Event>,
}

/// What a stop's trials gave.
#[derive(Default)]
struct Out {
    results: Vec<Res>,
    ends: Vec<String>,
    trials: u64,
    refused: u64,
    not_idle: u64,
    mids: u64,
    offer: Value,
}

/// The click sequences from the engine's stop (the module's text, "Inputs"), each a trial; `pre` names the round.
fn clicks(e: &mut Box<dyn Engine>, pre: &str, echo: &Echo, cfg: &Cfg, out: &mut Out) -> EResult<()> {
    let o0 = e.obs(&NAMES)?;
    let base = facts_of(&o0);
    let st0 = e.status().clone();
    let sentence = base.val(Key::var(afford::VAR_SENTENCE_SCRIPT as u64)) as i32;
    let input0 = base.val(Key::var(afford::VAR_VERB_SCRIPT as u64)) as i32;
    let mut seen_mid: HashSet<u64> = HashSet::from([view(&o0, &base, &st0, echo).mid()]);
    let mut frontier: Vec<Mid> = vec![Mid { engine: None, clicks: Vec::new(), events: Vec::new() }];
    for depth in 1..=MAX_CLICKS {
        let mut next: Vec<Mid> = Vec::new();
        for mut m in frontier {
            let me: &mut Box<dyn Engine> = match m.engine.as_mut() {
                Some(x) => x,
                None => &mut *e,
            };
            let o = me.obs(&ObsReq::default())?;
            let of = afford::offer_cfg(me, &o, false, true)?;
            let mf = me.status().frame;
            let mego = o.ego_pos;
            let mut points: Vec<Point> = of.verbs.clone();
            points.extend(of.targets.iter().copied());
            if depth == 1 {
                if out.offer.is_null() {
                    out.offer = of.json();
                }
                // floor clicks: a grid over the main screen, where the hit test finds no object and no actor
                let (top, h) = me.status().screen.map(|s| (s.0, s.1)).unwrap_or((0, afford::SCREEN_H));
                let grid: Vec<(i32, i32)> =
                    FLOOR_Y.iter().flat_map(|&k| FLOOR_X.iter().map(move |&x| (x, (top + (h - top) * k / 8).min(h - 2)))).collect();
                let ats = me.at_many(&grid)?;
                for (i, (&(x, y), a)) in grid.iter().zip(ats.iter()).enumerate() {
                    if a.main.is_some_and(|m| m.2 == 0 && m.3 == 0) {
                        points.push(Point { x, y, hit: Hit::Walk(i as i32) });
                    }
                }
            }
            for p in &points {
                let mut b = me.fork()?;
                b.segment(true)?;
                b.runs()?;
                let ev = Event { frame: mf + 1, what: Input::Click(p.x, p.y) };
                let mut cl = m.clicks.clone();
                cl.push(*p);
                let mut events = m.events.clone();
                events.push(ev.clone());
                out.trials += 1;
                match b.inputs(std::slice::from_ref(&ev)) {
                    Ok(()) => {}
                    Err(EngineError::Bridge(_)) => {
                        out.refused += 1;
                        b.quit();
                        continue;
                    }
                    Err(x) => return Err(x),
                }
                if let Err(st) = go(&mut b, RunSpec { frame: Some(mf + 2), ..Default::default() })? {
                    out.ends.push(format!("{} {}", name_of(pre, &cl), st.reason));
                    b.quit();
                    continue;
                }
                match calm(&mut b, mf + cfg.horizon)? {
                    Played::Stop(st) => {
                        let ob = b.obs(&NAMES)?;
                        let facts = facts_of(&ob);
                        let v = view(&ob, &facts, &st, echo);
                        // only the input script ran since the click and ego stands where it stood: a sentence half made
                        let half = ob.ego_pos == mego && {
                            let runs = b.runs()?;
                            let input = [input0, facts.val(Key::var(afford::VAR_VERB_SCRIPT as u64)) as i32];
                            afford::composing(st.idle.as_deref(), &runs.runs, mf + 1, &input, sentence)
                        };
                        if half {
                            // a stop to click on from; at the last click's depth it is no stop at all
                            if depth < MAX_CLICKS && seen_mid.insert(v.mid()) {
                                out.mids += 1;
                                next.push(Mid { engine: Some(b), clicks: cl, events });
                                continue;
                            }
                        } else {
                            out.results.push(Res { name: name_of(pre, &cl), events, view: v, facts, st });
                        }
                    }
                    Played::Ended(st) => out.ends.push(format!("{} {}", name_of(pre, &cl), st.reason)),
                    Played::NotIdle => out.not_idle += 1,
                }
                b.quit();
            }
            if let Some(x) = m.engine.take() {
                x.quit();
            }
        }
        frontier = next;
    }
    for mut m in frontier {
        if let Some(x) = m.engine.take() {
            x.quit();
        }
    }
    Ok(())
}

type Sh = Arc<(Mutex<Shared>, Condvar)>;

/// Every input of the stop tried, at the stop and after the game's own moves; the kept stops committed.
fn expand(e: &mut Box<dyn Engine>, id: usize, sh: &Sh, cfg: &Cfg) -> EResult<()> {
    let o0 = e.obs(&NAMES)?;
    let base = facts_of(&o0);
    let st0 = e.status().clone();
    // the slots whose text follows the pointer: the pointer moved over two targets and off them, in forks
    let mut echo: Echo = sh.0.lock().unwrap().echo.clone();
    {
        let op = e.obs(&ObsReq::default())?;
        let of = afford::offer_cfg(e, &op, false, true)?;
        let mut names: HashMap<(i32, i32), HashSet<String>> = HashMap::new();
        let mut note = |o: &Obs| {
            for v in &o.verbs {
                names.entry((v.slot, v.id)).or_default().insert(v.name.clone().unwrap_or_default());
            }
        };
        note(&o0);
        let pts: Vec<(i32, i32)> = of.targets.iter().take(2).map(|p| (p.x, p.y)).chain([(afford::SCREEN_W / 2, 1)]).collect();
        for (x, y) in pts {
            let mut t = e.fork()?;
            let mv = Event { frame: st0.frame + 1, what: Input::Move(x, y) };
            if t.inputs(std::slice::from_ref(&mv)).is_ok() && t.run(&RunSpec { frame: Some(st0.frame + 3), ..Default::default() }).is_ok() {
                note(&t.obs(&NAMES)?);
            }
            t.quit();
        }
        echo.extend(names.into_iter().filter(|x| x.1.len() > 1).map(|x| x.0));
    }
    let world0 = view(&o0, &base, &st0, &echo).world;
    let mut out = Out::default();
    clicks(e, "", &echo, cfg, &mut out)?;
    // the game's own moves: no input for a while; where the world changed, the inputs again from there
    let mut ambient: HashSet<Key> = HashSet::new();
    for n in WAITS {
        let mut b = e.fork()?;
        out.trials += 1;
        match go(&mut b, RunSpec { frame: Some(st0.frame + n), ..Default::default() })? {
            Err(st) => out.ends.push(format!("wait{n} {}", st.reason)),
            Ok(_) => match calm(&mut b, st0.frame + n + cfg.horizon)? {
                Played::Stop(st) => {
                    let ob = b.obs(&NAMES)?;
                    let facts = facts_of(&ob);
                    let v = view(&ob, &facts, &st, &echo);
                    let moved: Vec<Key> = Facts(world0.clone()).diff(&Facts(v.world.clone())).into_iter().map(|d| d.0).collect();
                    out.results.push(Res { name: format!("wait{n}"), events: Vec::new(), view: v, facts, st });
                    if !moved.is_empty() {
                        ambient.extend(moved);
                        clicks(&mut b, &format!("wait{n}+"), &echo, cfg, &mut out)?;
                    }
                }
                Played::Ended(st) => out.ends.push(format!("wait{n} {}", st.reason)),
                Played::NotIdle => out.not_idle += 1,
            },
        }
        b.quit();
    }

    let results = out.results;
    let mut keep: Vec<(usize, usize, usize)> = Vec::new(); // (result, node, queue)
    {
        let mut g = sh.0.lock().unwrap();
        let s = &mut *g;
        s.trials += out.trials;
        s.refused += out.refused;
        s.not_idle += out.not_idle;
        for x in out.ends {
            s.ends.push((id, x));
        }
        s.echo.extend(echo.iter().copied());
        s.ambient.extend(ambient);
        let (movie, depth) = (s.nodes[id].events.clone(), s.nodes[id].depth);
        let mut kept_here = 0;
        for (i, r) in results.iter().enumerate() {
            let v = &r.view;
            let mut novel: Vec<String> = Vec::new();
            let mut class = QUEUES;
            for &(k, val) in &v.world {
                if s.seen.insert((k, val)) {
                    novel.push(format!("{k}={val}"));
                    class = 0;
                }
                if s.seen_at.insert((v.room, k, val)) {
                    class = class.min(3);
                }
            }
            for &t in &v.vis {
                if s.sees.insert((v.room, t)) {
                    novel.push(format!("sees={t}"));
                    class = class.min(1);
                }
            }
            if s.ifaces.insert((v.room, v.iface)) {
                novel.push("iface".into());
                class = class.min(1);
            }
            if s.boxes.insert((v.room, v.walkbox)) {
                novel.push(format!("box={}", v.walkbox));
                class = class.min(2);
            }
            if s.cams.insert((v.room, v.cam)) {
                novel.push(format!("cam={}", v.cam));
                class = class.min(2);
            }
            if s.cells.insert(v.cell(&s.ambient)) {
                class = class.min(4);
            }
            // nothing new, alone or together: dropped
            if class == QUEUES {
                continue;
            }
            kept_here += 1;
            if !s.rooms.contains(&v.room) {
                s.rooms.push(v.room);
            }
            let nid = s.nodes.len();
            let mut events = movie.clone();
            events.extend(r.events.iter().cloned());
            let secs = s.t0.map(|t| t.elapsed().as_secs_f64()).unwrap_or(0.0);
            for w in s.watch.iter_mut() {
                if w.2.is_none() && r.facts.val(w.0) == w.1 {
                    w.2 = Some((nid, secs));
                    eprintln!("[{secs:7.1}s] WATCH {}={} first held by node {nid}", w.0, w.1);
                }
            }
            let line = json!({"id": nid, "parent": id, "depth": depth + 1, "frame": r.st.frame, "room": v.room,
                "class": class, "how": r.name, "novel": novel, "t": (secs * 10.0).round() / 10.0,
                "events": events.iter().map(fmi_explore::events::event_json).collect::<Vec<Value>>()});
            if let Some(f) = s.log.as_mut() {
                let _ = writeln!(f, "{line}");
            }
            s.nodes.push(Node { events, frame: r.st.frame, depth: depth + 1 });
            s.kept[class] += 1;
            keep.push((i, nid, class));
        }
        if let Some(f) = s.offers.as_mut() {
            let _ = writeln!(f, "{}", json!({"node": id, "room": st0.room, "offer": out.offer, "trials": out.trials, "mids": out.mids,
                "stops": results.len(), "kept": kept_here, "refused": out.refused, "not_idle": out.not_idle}));
        }
    }
    // the kept stops' engines, while this one is here to fork: the trial again (the engine is deterministic); a stop
    // is queued once its engine is there, or once it is known to have none
    for (i, nid, class) in keep {
        let enqueue = |b: Option<Box<dyn Engine>>| {
            let mut g = sh.0.lock().unwrap();
            if let Some(b) = b {
                g.live.insert(nid, b);
            }
            g.queues[class].push_back(nid);
            sh.1.notify_all();
        };
        if sh.0.lock().unwrap().live.len() >= LIVE_MAX {
            enqueue(None);
            continue;
        }
        let r = &results[i];
        let made = (|| -> EResult<Box<dyn Engine>> {
            let mut b = e.fork()?;
            let run = (|| -> EResult<()> {
                for ev in &r.events {
                    // each click as the trial gave it: at the frame before its own
                    if ev.frame - 1 > b.status().frame {
                        b.run(&RunSpec { frame: Some(ev.frame - 1), ..Default::default() })?;
                    }
                    b.inputs(std::slice::from_ref(ev))?;
                }
                if r.st.frame > b.status().frame {
                    b.run(&RunSpec { frame: Some(r.st.frame), ..Default::default() })?;
                }
                Ok(())
            })();
            match run {
                Ok(()) => Ok(b),
                Err(x) => {
                    b.quit();
                    Err(x)
                }
            }
        })();
        enqueue(made.ok());
    }
    Ok(())
}

fn worker(host: Arc<Host>, sh: Sh, cfg: Arc<Cfg>) {
    loop {
        let (id, engine, events, frame) = {
            let mut g = sh.0.lock().unwrap();
            loop {
                if g.done {
                    return;
                }
                let next = (0..QUEUES).find_map(|c| g.queues[c].pop_front());
                if let Some(id) = next {
                    g.busy += 1;
                    let e = g.live.remove(&id);
                    break (id, e, g.nodes[id].events.clone(), g.nodes[id].frame);
                }
                if g.busy == 0 {
                    g.done = true;
                    sh.1.notify_all();
                    return;
                }
                g = sh.1.wait(g).unwrap();
            }
        };
        let booted = match engine {
            Some(e) => Ok(e),
            None => {
                sh.0.lock().unwrap().boots += 1;
                host.boot(&BootCfg { seed: 1, events, frame: Some(frame), dump_locals: false, lean: true, out_dir: eng_dir(&cfg.out) })
            }
        };
        match booted {
            Ok(mut e) => {
                if let Err(x) = expand(&mut e, id, &sh, &cfg) {
                    eprintln!("node {id}: {x}");
                }
                e.quit();
            }
            Err(x) => eprintln!("node {id}: boot: {x}"),
        }
        let mut g = sh.0.lock().unwrap();
        g.busy -= 1;
        g.expanded += 1;
        sh.1.notify_all();
    }
}

/// `direct verify NODES.jsonl ID OUT [KEY=V,..]`: the kept stop's movie played from boot in a fresh engine, to its
/// frame; prints the room and each key's value there, and whether each equals V.
fn verify(nodes: &str, id: u64, out: &Path, keys: &str) {
    let text = std::fs::read_to_string(nodes).expect("NODES.jsonl");
    let line = text
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find(|v| v["id"].as_u64() == Some(id))
        .expect("the node");
    let events: Vec<Event> = line["events"].as_array().unwrap().iter().filter_map(fmi_explore::events::event_of).collect();
    assert_eq!(events.len(), line["events"].as_array().unwrap().len(), "every event read");
    let frame = line["frame"].as_u64().unwrap();
    std::fs::create_dir_all(out).expect("OUT");
    let host = Host::new(HostCfg::from_env()).expect("the host");
    let t0 = Instant::now();
    let mut e = host
        .boot(&BootCfg { seed: 1, events: events.clone(), frame: Some(frame), dump_locals: false, lean: true, out_dir: eng_dir(out) })
        .expect("the boot");
    let f = e.facts().expect("facts");
    let room = f.val(Key::pack(K_ROOM, 0, 0, 0));
    let mut ok = room == line["room"].as_i64().unwrap();
    let mut held = Vec::new();
    for w in keys.split(',').filter(|x| !x.is_empty()) {
        let (k, v) = w.split_once('=').expect("KEY=V");
        let got = f.val(Key::parse(k));
        ok &= got == v.parse::<i64>().expect("a value");
        held.push(json!([k, got]));
    }
    let clicks = events.iter().filter(|x| matches!(x.what, Input::Click(..))).count();
    println!("{}", json!({"node": id, "from_boot": true, "frame": frame, "room": room, "keys": held, "clicks": clicks,
        "seconds": t0.elapsed().as_secs_f64(), "ok": ok}));
    e.quit();
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let opt = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    if a.first().map(String::as_str) == Some("verify") {
        return verify(&a[1], a[2].parse().expect("ID"), Path::new(&a[3]), a.get(4).map(String::as_str).unwrap_or(""));
    }
    let out = PathBuf::from(a.first().expect("direct OUT [--minutes M] [--workers W] [--horizon H] [--no-skips] [--watch K=V,..]"));
    std::fs::create_dir_all(&out).expect("OUT");
    let minutes: f64 = opt("--minutes").map(|v| v.parse().unwrap()).unwrap_or(10.0);
    let workers: usize = opt("--workers").map(|v| v.parse().unwrap()).unwrap_or(6);
    let cfg = Arc::new(Cfg { out: out.clone(), horizon: opt("--horizon").map(|v| v.parse().unwrap()).unwrap_or(1600) });
    let host = Arc::new(Host::new(HostCfg::from_env()).expect("the host"));
    let root_events = if a.iter().any(|x| x == "--no-skips") {
        Vec::new()
    } else {
        vec![Event { frame: 1, what: Input::Skips { text: true, cutscenes: true, spare: None } }]
    };
    let t0 = Instant::now();
    let mut e = host
        .boot(&BootCfg { seed: 1, events: root_events.clone(), frame: None, dump_locals: false, lean: true, out_dir: eng_dir(&out) })
        .expect("the boot");
    let st = e.status().clone();
    eprintln!("boot: frame {} room {} idle {:?} in {:.1} s", st.frame, st.room, st.idle, t0.elapsed().as_secs_f64());
    let o = e.obs(&NAMES).expect("the root's dump");
    let f = facts_of(&o);
    let mut s = Shared {
        log: Some(std::fs::File::create(out.join("nodes.jsonl")).expect("nodes.jsonl")),
        offers: Some(std::fs::File::create(out.join("offers.jsonl")).expect("offers.jsonl")),
        t0: Some(t0),
        ..Default::default()
    };
    for w in opt("--watch").unwrap_or_default().split(',').filter(|x| !x.is_empty()) {
        let (k, v) = w.split_once('=').expect("--watch KEY=V");
        s.watch.push((Key::parse(k), v.parse().expect("a value"), None));
    }
    let v = view(&o, &f, &st, &Echo::new());
    s.cells.insert(v.cell(&s.ambient));
    for &(k, val) in &v.world {
        s.seen.insert((k, val));
        s.seen_at.insert((v.room, k, val));
    }
    for &t in &v.vis {
        s.sees.insert((v.room, t));
    }
    s.ifaces.insert((v.room, v.iface));
    s.boxes.insert((v.room, v.walkbox));
    s.cams.insert((v.room, v.cam));
    s.rooms.push(v.room);
    s.nodes.push(Node { events: root_events, frame: st.frame, depth: 0 });
    s.queues[0].push_back(0);
    s.live.insert(0, e);
    let sh: Sh = Arc::new((Mutex::new(s), Condvar::new()));
    let hs: Vec<_> = (0..workers)
        .map(|_| {
            let (h, s, c) = (host.clone(), sh.clone(), cfg.clone());
            std::thread::spawn(move || worker(h, s, c))
        })
        .collect();
    let limit = Duration::from_secs_f64(minutes * 60.0);
    let mut last = 0u64;
    loop {
        std::thread::sleep(Duration::from_secs(10));
        let mut g = sh.0.lock().unwrap();
        let secs = t0.elapsed().as_secs_f64();
        eprintln!(
            "[{secs:7.1}s] kept {} {:?}, expanded {}, queued {:?}, trials {} ({:.0}/s), rooms {} {:?}, boots {}",
            g.nodes.len(), g.kept, g.expanded, g.queues.iter().map(|q| q.len()).collect::<Vec<_>>(),
            g.trials, (g.trials - last) as f64 / 10.0, g.rooms.len(), g.rooms, g.boots
        );
        last = g.trials;
        if g.done || t0.elapsed() >= limit {
            g.done = true;
            sh.1.notify_all();
            break;
        }
    }
    for h in hs {
        let _ = h.join();
    }
    let mut g = sh.0.lock().unwrap();
    let live: Vec<Box<dyn Engine>> = g.live.drain().map(|x| x.1).collect();
    for e in live {
        e.quit();
    }
    let depth = g.nodes.iter().map(|n| n.depth).max().unwrap_or(0);
    let sum = json!({"seconds": t0.elapsed().as_secs_f64(), "workers": workers, "kept": g.nodes.len(), "by_queue": g.kept,
        "expanded": g.expanded, "trials": g.trials, "refused": g.refused, "not_idle": g.not_idle, "rooms": g.rooms,
        "max_depth": depth, "boots": g.boots,
        "ends": g.ends.iter().map(|(n, x)| json!([n, x])).collect::<Vec<Value>>(),
        "watch": g.watch.iter().map(|(k, v, at)| json!({"key": format!("{k}"), "value": v, "node": at.map(|x| x.0), "seconds": at.map(|x| x.1)})).collect::<Vec<Value>>()});
    std::fs::write(out.join("summary.json"), format!("{sum}\n")).expect("summary.json");
    println!("{sum}");
}
