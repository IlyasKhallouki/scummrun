//! fmi-engine: the engine component's checks and bench (experiments/E011-speed/engine/; plan.md G2-G4, B1).
//!
//!   fmi-engine dumps OUT ARCHIVE IDX [--lean] [--env K=V]...
//!       the chain of ARCHIVE's entry IDX from boot (seed 1, its movie), as profile-engine's chain.py dumps: an exact
//!       state dump every 100 frames, at the archive's entry frames 0..=IDX and at the entry's frame, written as
//!       OUT/dumps.jsonl (chain.py compare reads it). At every dump also G4: the binary Obs equals the JSON's, and the
//!       F2 block's facts equal facts_of. Prints a JSON summary.
//!   fmi-engine walk OUT ARCHIVE IDX FROM N [--lean]
//!       G4 frame by frame: the entry's movie from boot to FROM, then N frames one Step each (runs and explain on):
//!       at every frame the Step's block (delta) equals a State block, its facts equal facts_of of the binary Obs,
//!       and the binary Obs equals the JSON dump's. A second engine plays the same frames by Step alone (no JSON, no
//!       Obs) and its final dump must equal the first's.
//!   fmi-engine verify OUT ARCHIVE IDX [--lean]
//!       replay.verify of the entry's movie against its checkpoints (and the root's), as the Rust port runs it.
//!   fmi-engine pool OUT ARCHIVE [--lean]
//!       the kept-entry pool on the archive's entries: every entry returned by fork and compared with its last
//!       checkpoint, then made again after evicting everything but the root (from the nearest kept ancestor).
//!   fmi-engine bench OUT ARCHIVE IDX [N] [--lean]
//!       E010's frame costs a-d over the binary channel, the start, a return by fork of a kept entry, and the entry's
//!       chain replayed from boot.

use fmi_core::*;
use fmi_engine::json::{self, J};
use fmi_engine::replay;
use fmi_engine::{BinEngine, EntrySpec, Host, HostCfg, Pool};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

type Res<T> = Result<T, String>;

fn e2s<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// An archive entry (explore.py's K.json.gz): its frame, parent, movie and checkpoints.
struct Entry {
    id: u64,
    parent: Option<u64>,
    frame: u64,
    events: Vec<Event>,
    checkpoints: Vec<Checkpoint>,
}

fn gunzip(p: &Path) -> Res<String> {
    let out = std::process::Command::new("gzip")
        .arg("-dc")
        .arg(p)
        .output()
        .map_err(e2s)?;
    if !out.status.success() {
        return Err(format!(
            "gzip -dc {}: {}",
            p.display(),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    String::from_utf8(out.stdout).map_err(e2s)
}

fn event_of(j: &J) -> Res<Event> {
    let frame = j.at("frame")?.int()? as u64;
    let pt = |k: &str| -> Res<Option<(i32, i32)>> {
        match j.get(k) {
            None => Ok(None),
            Some(v) => {
                let a = v.arr()?;
                Ok(Some((a[0].i32()?, a[1].i32()?)))
            }
        }
    };
    let what = if let Some((x, y)) = pt("click")? {
        Input::Click(x, y)
    } else if let Some((x, y)) = pt("rclick")? {
        Input::RClick(x, y)
    } else if let Some((x, y)) = pt("move")? {
        Input::Move(x, y)
    } else if let Some(k) = j.get("key") {
        Input::Key(k.text()?.to_string())
    } else if let Some(s) = j.get("skips") {
        Input::Skips {
            text: s.at("text")?.int()? != 0,
            cutscenes: s.at("cutscenes")?.int()? != 0,
            spare: match s.get("spare") {
                Some(v) => Some(v.arr()?.iter().map(J::i32).collect::<Res<_>>()?),
                None => None,
            },
        }
    } else {
        return Err(format!("unknown event {j:?}"));
    };
    Ok(Event { frame, what })
}

fn checkpoint_of_json(j: &J) -> Res<Checkpoint> {
    let a = j.arr()?;
    let facts = Facts::from_pairs(
        a[1].obj()?
            .iter()
            .map(|(k, v)| Ok((Key::parse(k), v.int()?)))
            .collect::<Res<Vec<_>>>()?,
    );
    let x = &a[2];
    let mut actors = Vec::new();
    for t in x.at("actors")?.arr()? {
        let t = t.arr()?;
        actors.push((t[0].i32()?, t[1].i32()?, t[2].i32()?));
    }
    Ok(Checkpoint {
        frame: a[0].int()? as u64,
        facts,
        tick: x.at("tick")?.int()? as u64,
        rng: x.at("rng")?.int()? as u64,
        actors,
    })
}

fn entry(arch: &Path, idx: u64) -> Res<Entry> {
    let j = json::parse(&gunzip(&arch.join("entries").join(format!("{idx}.json.gz")))?)?;
    Ok(Entry {
        id: idx,
        parent: j
            .get("parent")
            .filter(|p| !p.is_null())
            .map(J::int)
            .transpose()?
            .map(|p| p as u64),
        frame: j.at("frame")?.int()? as u64,
        events: j.at("events")?.arr()?.iter().map(event_of).collect::<Res<_>>()?,
        checkpoints: j
            .at("checkpoints")?
            .arr()?
            .iter()
            .map(checkpoint_of_json)
            .collect::<Res<_>>()?,
    })
}

struct Opts {
    lean: bool,
    env: Vec<(String, String)>,
    pos: Vec<String>,
}

fn opts(args: &[String]) -> Opts {
    let mut o = Opts {
        lean: false,
        env: Vec::new(),
        pos: Vec::new(),
    };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--lean" => o.lean = true,
            "--env" => {
                i += 1;
                let (k, v) = args[i].split_once('=').expect("--env K=V");
                o.env.push((k.into(), v.into()));
            }
            a => o.pos.push(a.into()),
        }
        i += 1;
    }
    o
}

fn host(o: &Opts) -> Res<Host> {
    let mut cfg = HostCfg::from_env();
    cfg.extra_env = o.env.clone();
    Host::new(cfg).map_err(e2s)
}

fn boot_cfg(out: &Path, events: &[Event], frame: Option<u64>, lean: bool) -> BootCfg {
    BootCfg {
        seed: 1,
        events: events.to_vec(),
        frame,
        dump_locals: true,
        lean,
        out_dir: out.to_path_buf(),
    }
}

const FULL_EXACT: ObsReq = ObsReq {
    objects: None,
    names: true,
    exact: true,
};

/// G4 at one stop: the binary Obs (names, exact) equals the JSON dump's, and the F2 facts equal facts_of.
/// Returns the JSON text and a list of what differs.
fn g4(e: &mut BinEngine) -> Res<(String, Vec<String>)> {
    let text = e.state_json(&FULL_EXACT).map_err(e2s)?;
    let from_json = json::obs_of(&json::parse(&text)?, true)?;
    let bin = e.obs(&FULL_EXACT).map_err(e2s)?;
    let mut bad = Vec::new();
    if bin != from_json {
        bad.push(first_obs_difference(&bin, &from_json));
    }
    let f = e.facts().map_err(e2s)?;
    let want = facts_of(&from_json);
    if f != want {
        let d = want.diff(&f);
        bad.push(format!(
            "facts differ on {} keys, first {:?}",
            d.len(),
            &d[..d.len().min(4)]
        ));
    }
    Ok((text, bad))
}

fn first_obs_difference(a: &Obs, b: &Obs) -> String {
    macro_rules! cmp {
        ($($f:ident),*) => {
            $(if a.$f != b.$f { return format!("obs.{} differs: {:?} vs {:?}", stringify!($f),
                                               trunc(format!("{:?}", a.$f)), trunc(format!("{:?}", b.$f))); })*
        };
    }
    cmp!(tick, frame, room, ego, ego_pos, seed, boot_param, vars, bits_set, inventory, owners, states, classes);
    cmp!(room_objects, verbs, slots, cutscene, actors, exact);
    "obs differ".into()
}

fn trunc(s: String) -> String {
    if s.len() > 300 {
        format!("{}...", &s[..300])
    } else {
        s
    }
}

fn dumps(o: &Opts) -> Res<()> {
    let (out, arch, idx) = (
        PathBuf::from(&o.pos[0]),
        PathBuf::from(&o.pos[1]),
        o.pos[2].parse::<u64>().map_err(e2s)?,
    );
    let ent = entry(&arch, idx)?;
    let mut frames: Vec<u64> = (100..ent.frame).step_by(100).collect();
    for i in 0..=idx {
        frames.push(entry(&arch, i)?.frame);
    }
    frames.retain(|&f| f <= ent.frame);
    frames.sort();
    frames.dedup();
    std::fs::create_dir_all(&out).map_err(e2s)?;
    let events: Vec<Event> = ent.events.iter().filter(|e| e.frame <= ent.frame).cloned().collect();
    let h = host(o)?;
    let t0 = Instant::now();
    let mut e = h
        .spawn(&boot_cfg(&out.join("run"), &events, None, o.lean))
        .map_err(e2s)?;
    let start_s = t0.elapsed().as_secs_f64();
    let mut fh = std::fs::File::create(out.join("dumps.jsonl")).map_err(e2s)?;
    let (mut run_s, mut g4_bad) = (0.0, Vec::new());
    let mut last_text = String::new();
    for &f in &frames {
        let t = Instant::now();
        let st = e
            .run(&RunSpec {
                frame: Some(f),
                ..Default::default()
            })
            .map_err(e2s)?;
        run_s += t.elapsed().as_secs_f64();
        if st.frame != f {
            return Err(format!("stopped at {} instead of {f}", st.frame));
        }
        let (text, bad) = g4(&mut e)?;
        for b in bad {
            g4_bad.push(format!("frame {f}: {b}"));
        }
        writeln!(fh, "{{\"frame\": {f}, \"status\": {{}}, \"state\": {text}}}").map_err(e2s)?;
        last_text = text;
    }
    let last = json::obs_of(&json::parse(&last_text)?, false)?;
    let lf = facts_of(&last);
    let cpu = e.cpu_s();
    Box::new(e).quit();
    println!(
        "{{\"dumps\": {}, \"frames\": {}, \"start_s\": {:.3}, \"run_s\": {:.3}, \"engine_cpu_s\": {:.2}, \
         \"g4_differences\": {}, \"state:316\": {}, \"owner:566\": {}}}",
        frames.len(),
        frames.last().unwrap(),
        start_s,
        run_s,
        cpu,
        g4_bad.len(),
        lf.val(Key::state(316)),
        lf.val(Key::owner(566))
    );
    for b in g4_bad.iter().take(10) {
        eprintln!("G4: {b}");
    }
    if g4_bad.is_empty() {
        Ok(())
    } else {
        Err("G4 differences".into())
    }
}

fn walk(o: &Opts) -> Res<()> {
    let (out, arch, idx) = (
        PathBuf::from(&o.pos[0]),
        PathBuf::from(&o.pos[1]),
        o.pos[2].parse::<u64>().map_err(e2s)?,
    );
    let (from, n) = (
        o.pos[3].parse::<u64>().map_err(e2s)?,
        o.pos[4].parse::<u64>().map_err(e2s)?,
    );
    let ent = entry(&arch, idx)?;
    let events: Vec<Event> = ent.events.iter().filter(|e| e.frame <= from + n).cloned().collect();
    let h = host(o)?;
    let mut a = h
        .boot_bin(&boot_cfg(&out.join("a"), &events, Some(from), o.lean))
        .map_err(e2s)?;
    let mut b = h
        .boot_bin(&boot_cfg(&out.join("b"), &events, Some(from), o.lean))
        .map_err(e2s)?;
    for x in [&mut a, &mut b] {
        x.segment(true).map_err(e2s)?;
        x.runs().map_err(e2s)?;
        x.explain(Some(&ExplainArm {
            from: from + 1,
            to: from + 100_000,
            units: vec![],
        }))
        .map_err(e2s)?;
        x.explain_log().map_err(e2s)?;
    }
    let (mut bad, mut runs, mut entries, mut deltas) = (Vec::new(), 0usize, 0usize, 0usize);
    for i in 0..n {
        let s = a.step(1, true, true).map_err(e2s)?;
        runs += s.runs.as_ref().map_or(0, |r| r.runs.len());
        entries += s.explain.as_ref().map_or(0, |r| r.entries.len());
        let stepped = a.block().map_err(e2s)?.to_vec();
        let sf = a.facts().map_err(e2s)?;
        // the State block of an exact copy (a fork: its connection has no delta base) must be the Step's
        let mut c = a.fork_bin().map_err(e2s)?;
        let fresh = c.block().map_err(e2s)?.to_vec();
        Box::new(c).quit();
        if fresh != stepped {
            bad.push(format!(
                "frame {}: the Step's block differs from a State block",
                from + i + 1
            ));
        }
        deltas += 1;
        let (_, g) = g4(&mut a)?;
        for x in g {
            bad.push(format!("frame {}: {x}", from + i + 1));
        }
        if sf != a.facts().map_err(e2s)? {
            bad.push(format!("frame {}: facts changed by the dumps", from + i + 1));
        }
        b.step(1, true, true).map_err(e2s)?;
    }
    let ja = a.state_json(&FULL_EXACT).map_err(e2s)?;
    let jb = b.state_json(&FULL_EXACT).map_err(e2s)?;
    if json::parse(&ja)? != json::parse(&jb)? {
        bad.push("the Step-only engine's final dump differs from the observed one's".into());
    }
    println!(
        "{{\"frames\": {n}, \"from\": {from}, \"steps_checked\": {deltas}, \"runs\": {runs}, \"explain_entries\": {entries}, \
         \"differences\": {}}}",
        bad.len()
    );
    for x in bad.iter().take(10) {
        eprintln!("G4 walk: {x}");
    }
    if bad.is_empty() {
        Ok(())
    } else {
        Err("walk differences".into())
    }
}

fn verify_cmd(o: &Opts) -> Res<()> {
    let (out, arch, idx) = (
        PathBuf::from(&o.pos[0]),
        PathBuf::from(&o.pos[1]),
        o.pos[2].parse::<u64>().map_err(e2s)?,
    );
    let ent = entry(&arch, idx)?;
    let root = entry(&arch, 0)?;
    let mut cps = root.checkpoints.clone();
    cps.extend(ent.checkpoints.iter().cloned());
    let h = host(o)?;
    let v = replay::verify(
        &h,
        &boot_cfg(&out, &[], None, o.lean),
        &ent.events,
        &cps,
        &[Key::state(316), Key::owner(566)],
    )
    .map_err(e2s)?;
    println!(
        "{{\"checkpoints\": {}, \"differ\": {}, \"inputs\": {}, \"final\": \"{:?}\", \"same\": {}, \"seconds\": {:.3}}}",
        v.checkpoints,
        v.differ.len(),
        v.inputs,
        v.final_,
        v.same,
        v.seconds
    );
    for d in v.differ.iter().take(5) {
        eprintln!("differ: {d:?}");
    }
    if v.same {
        Ok(())
    } else {
        Err("verify differs".into())
    }
}

fn pool_cmd(o: &Opts) -> Res<()> {
    let (out, arch) = (PathBuf::from(&o.pos[0]), PathBuf::from(&o.pos[1]));
    let mut ents = Vec::new();
    for i in 0.. {
        if !arch.join("entries").join(format!("{i}.json.gz")).exists() {
            break;
        }
        ents.push(entry(&arch, i)?);
    }
    let h = host(o)?;
    let mut pool = Pool::new(h, 1, true, o.lean, out.clone());
    for e in &ents {
        pool.add(EntrySpec {
            id: e.id,
            parent: e.parent,
            frame: e.frame,
            events: e.events.clone(),
        });
    }
    let mut bad = Vec::new();
    let check = |pool: &mut Pool, e: &Entry, what: &str, bad: &mut Vec<String>| -> Res<f64> {
        let t = Instant::now();
        let mut c = pool.ret(e.id).map_err(e2s)?;
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let obs = c.obs(&ObsReq::default()).map_err(e2s)?;
        let st = c.status().clone();
        let want = e.checkpoints.last().unwrap();
        let d = replay::compare_checkpoint(want, &obs, &st);
        if st.frame != e.frame || !d.is_empty() {
            bad.push(format!(
                "{what} entry {}: frame {} vs {}, {d:?}",
                e.id, st.frame, e.frame
            ));
        }
        Box::new(c).quit();
        Ok(ms)
    };
    let mut first = Vec::new();
    for e in &ents {
        first.push(check(&mut pool, e, "made", &mut bad)?);
    }
    let mut again = Vec::new();
    for _ in 0..3 {
        for e in &ents {
            again.push(check(&mut pool, e, "kept", &mut bad)?);
        }
    }
    // evict all but the root (used last), so that every other entry is made again from it
    Box::new(pool.ret(ents[0].id).map_err(e2s)?).quit();
    while pool.n_kept() > 1 {
        pool.evict_one();
    }
    let mut rebuilt = Vec::new();
    for e in ents.iter().rev() {
        rebuilt.push(check(&mut pool, e, "rebuilt", &mut bad)?);
    }
    again.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let med = again[again.len() / 2];
    println!(
        "{{\"entries\": {}, \"first_ms\": {:?}, \"return_by_fork_ms_median\": {:.3}, \"return_by_fork_ms_max\": {:.3}, \
         \"rebuilt_ms\": {:?}, \"stats\": \"{:?}\", \"differences\": {}}}",
        ents.len(),
        first.iter().map(|x| (x * 10.0).round() / 10.0).collect::<Vec<_>>(),
        med,
        again.last().unwrap(),
        rebuilt.iter().map(|x| (x * 10.0).round() / 10.0).collect::<Vec<_>>(),
        pool.stats,
        bad.len()
    );
    for b in bad.iter().take(5) {
        eprintln!("pool: {b}");
    }
    if bad.is_empty() {
        Ok(())
    } else {
        Err("pool differences".into())
    }
}

struct Clock {
    t: Instant,
    cpu: f64,
    pid: u32,
}

impl Clock {
    fn new(pid: u32) -> Clock {
        Clock {
            t: Instant::now(),
            cpu: fmi_engine::sys::cpu_s(pid),
            pid,
        }
    }
    /// (wall ms, engine CPU ms) per item since start
    fn per(&self, n: u64) -> (f64, f64) {
        let w = self.t.elapsed().as_secs_f64() * 1e3 / n as f64;
        let c = (fmi_engine::sys::cpu_s(self.pid) - self.cpu) * 1e3 / n as f64;
        (w, c)
    }
}

fn bench(o: &Opts) -> Res<()> {
    let (out, arch, idx) = (
        PathBuf::from(&o.pos[0]),
        PathBuf::from(&o.pos[1]),
        o.pos[2].parse::<u64>().map_err(e2s)?,
    );
    let n: u64 = o.pos.get(3).map(|s| s.parse().unwrap()).unwrap_or(300);
    let ent = entry(&arch, idx)?;
    let h = host(o)?;
    let mut res: Vec<(String, String)> = Vec::new();
    let mut put = |k: &str, v: String| {
        eprintln!("{k}: {v}");
        res.push((k.into(), v))
    };
    let me = std::process::id();

    // the start, and the root (replay_part1.boot: the first idle stop)
    let t = Instant::now();
    let mut root = h.spawn(&boot_cfg(&out.join("boot"), &[], None, o.lean)).map_err(e2s)?;
    let start_wall = t.elapsed().as_secs_f64();
    let start_cpu = root.cpu_s();
    put(
        "start to the first stop",
        format!("{:.1} ms wall, {:.1} ms engine CPU", start_wall * 1e3, start_cpu * 1e3),
    );
    root.run(&RunSpec {
        idle: true,
        max_frame: Some(200_000),
        ..Default::default()
    })
    .map_err(e2s)?;

    // (a) the parent's observed frame: one Step with runs, explain entries and the state, facts from the block
    let mut p = root.fork_bin().map_err(e2s)?;
    p.segment(true).map_err(e2s)?;
    p.runs().map_err(e2s)?;
    let f0 = p.status().frame;
    p.explain(Some(&ExplainArm {
        from: f0 + 1,
        to: f0 + 100_000,
        units: vec![],
    }))
    .map_err(e2s)?;
    p.explain_log().map_err(e2s)?;
    let c = Clock::new(p.pid());
    let s0 = fmi_engine::sys::self_cpu_s();
    for _ in 0..n {
        p.step(1, true, true).map_err(e2s)?;
        p.facts().map_err(e2s)?;
    }
    let (w, cpu) = c.per(n);
    let own = (fmi_engine::sys::self_cpu_s() - s0) * 1e3 / n as f64;
    put(
        "a parent frame, observed (Step + facts)",
        format!("{w:.3} ms wall, {cpu:.3} ms engine CPU, {own:.3} ms driver CPU"),
    );

    // (b) the same with fork_each 7: each child's runs, explain log and facts read, then quit
    let c = Clock::new(p.pid());
    let mut k = 0u64;
    let mut kid_cpu = 0.0;
    while k < n {
        p.fork_each_bin(7).map_err(e2s)?.1.into_iter().for_each(|mut kid| {
            kid.runs().unwrap();
            kid.explain_log().unwrap();
            kid.facts().unwrap();
            kid_cpu += kid.cpu_s();
            Box::new(kid).quit();
            k += 1;
        });
        p.runs().map_err(e2s)?;
        p.explain_log().map_err(e2s)?;
    }
    let (w, cpu) = c.per(k);
    put(
        "b parent frame, fork_each 7",
        format!("{w:.3} ms wall, {cpu:.3} ms parent CPU"),
    );
    Box::new(p).quit();

    // (c) bare frames: one command per frame, 50-frame commands, and a frame plus the facts (State) or a full Obs
    let mut q = root.fork_bin().map_err(e2s)?;
    let c = Clock::new(q.pid());
    for _ in 0..n {
        let f = q.status().frame + 1;
        q.run(&RunSpec {
            frame: Some(f),
            ..Default::default()
        })
        .map_err(e2s)?;
    }
    let (w, cpu) = c.per(n);
    put("c bare frame", format!("{w:.3} ms wall, {cpu:.3} ms engine CPU"));
    let c = Clock::new(q.pid());
    for _ in 0..n {
        q.step(1, false, false).map_err(e2s)?;
        q.facts().map_err(e2s)?;
    }
    let (w, cpu) = c.per(n);
    put(
        "c frame + facts (Step with the block)",
        format!("{w:.3} ms wall, {cpu:.3} ms engine CPU"),
    );
    let c = Clock::new(q.pid());
    for _ in 0..n {
        let f = q.status().frame + 1;
        q.run(&RunSpec {
            frame: Some(f),
            ..Default::default()
        })
        .map_err(e2s)?;
        q.facts().map_err(e2s)?;
    }
    let (w, cpu) = c.per(n);
    put(
        "c frame + facts (run, then State)",
        format!("{w:.3} ms wall, {cpu:.3} ms engine CPU"),
    );
    let c = Clock::new(q.pid());
    for _ in 0..n {
        let f = q.status().frame + 1;
        q.run(&RunSpec {
            frame: Some(f),
            ..Default::default()
        })
        .map_err(e2s)?;
        q.obs(&ObsReq::default()).map_err(e2s)?;
    }
    let (w, cpu) = c.per(n);
    put(
        "c frame + full binary Obs",
        format!("{w:.3} ms wall, {cpu:.3} ms engine CPU"),
    );
    let c = Clock::new(q.pid());
    let reps = 50u64;
    let s0 = fmi_engine::sys::self_cpu_s();
    for _ in 0..20 * reps {
        q.obs(&ObsReq::default()).map_err(e2s)?;
    }
    let (w, cpu) = c.per(20 * reps);
    let own = (fmi_engine::sys::self_cpu_s() - s0) * 1e3 / (20 * reps) as f64;
    put(
        "full binary Obs alone",
        format!("{w:.3} ms wall, {cpu:.3} ms engine CPU, {own:.3} ms driver CPU"),
    );
    let c = Clock::new(q.pid());
    for _ in 0..20 * reps {
        q.block_fresh().map_err(e2s)?;
    }
    let (w, cpu) = c.per(20 * reps);
    put(
        "State (the F2 block) alone",
        format!("{w:.3} ms wall, {cpu:.3} ms engine CPU"),
    );
    let c = Clock::new(q.pid());
    for _ in 0..reps {
        q.state_json(&ObsReq::default()).map_err(e2s)?;
    }
    let (w, cpu) = c.per(reps);
    put(
        "JSON dump alone (StateJson)",
        format!("{w:.3} ms wall, {cpu:.3} ms engine CPU"),
    );
    let c = Clock::new(q.pid());
    let m = (10 * n / 50).max(1);
    for _ in 0..m {
        let f = q.status().frame + 50;
        q.run(&RunSpec {
            frame: Some(f),
            ..Default::default()
        })
        .map_err(e2s)?;
    }
    let (w, cpu) = c.per(m * 50);
    put(
        "c frame in 50-frame commands",
        format!("{w:.3} ms wall, {cpu:.3} ms engine CPU"),
    );
    let c = Clock::new(q.pid());
    for _ in 0..20 * reps {
        q.holds(&[]).map_err(e2s)?;
    }
    let (w, cpu) = c.per(20 * reps);
    put(
        "round trip (Holds, no condition)",
        format!("{:.1} us wall, {:.1} us engine CPU", w * 1e3, cpu * 1e3),
    );

    // (d) fork + quit; fork + facts + quit
    let c = Clock::new(q.pid());
    let s0 = fmi_engine::sys::self_cpu_s();
    for _ in 0..200 {
        let k = q.fork_bin().map_err(e2s)?;
        Box::new(k).quit();
    }
    let (w, cpu) = c.per(200);
    let own = (fmi_engine::sys::self_cpu_s() - s0) * 1e3 / 200.0;
    put(
        "d fork + quit",
        format!("{w:.3} ms wall, {cpu:.3} ms parent CPU, {own:.3} ms driver CPU"),
    );
    let c = Clock::new(q.pid());
    for _ in 0..20 {
        let mut k = q.fork_bin().map_err(e2s)?;
        k.run(&RunSpec {
            frame: Some(k.status().frame + 1),
            ..Default::default()
        })
        .map_err(e2s)?;
        k.facts().map_err(e2s)?;
        Box::new(k).quit();
    }
    let (w, cpu) = c.per(20);
    put(
        "d fork + frame + facts + quit",
        format!("{w:.3} ms wall, {cpu:.3} ms parent CPU"),
    );
    Box::new(q).quit();
    Box::new(root).quit();
    let _ = me;

    // the entry: replay from boot (start, then its movie to its frame), and a return by fork of it kept
    let mut walls = Vec::new();
    for r in 0..3 {
        let t = Instant::now();
        let e = h
            .boot_bin(&boot_cfg(
                &out.join(format!("chain{r}")),
                &ent.events,
                Some(ent.frame),
                o.lean,
            ))
            .map_err(e2s)?;
        walls.push(t.elapsed().as_secs_f64());
        if r == 2 {
            put("chain from boot, engine CPU", format!("{:.0} ms", e.cpu_s() * 1e3));
        }
        Box::new(e).quit();
    }
    walls.sort_by(|a, b| a.partial_cmp(b).unwrap());
    put(
        &format!("entry {idx} (frame {}) replayed from boot", ent.frame),
        format!(
            "{:.0} ms wall (median of 3; {:?})",
            walls[1] * 1e3,
            walls.iter().map(|x| (x * 1e3).round()).collect::<Vec<_>>()
        ),
    );
    let mut pool = Pool::new(h.clone(), 1, true, o.lean, out.join("pool"));
    pool.add(EntrySpec {
        id: idx,
        parent: None,
        frame: ent.frame,
        events: ent.events.clone(),
    });
    Box::new(pool.ret(idx).map_err(e2s)?).quit();
    let mut rets = Vec::new();
    for _ in 0..50 {
        let t = Instant::now();
        let mut k = pool.ret(idx).map_err(e2s)?;
        let _ = k.status().frame;
        rets.push(t.elapsed().as_secs_f64() * 1e3);
        k.facts().map_err(e2s)?;
        Box::new(k).quit();
    }
    rets.sort_by(|a, b| a.partial_cmp(b).unwrap());
    put(
        "return by fork of a kept entry",
        format!(
            "{:.3} ms wall median, {:.3} min, {:.3} p90",
            rets[25], rets[0], rets[45]
        ),
    );
    let mut fr = Vec::new();
    for _ in 0..50 {
        let t = Instant::now();
        let mut k = pool.ret(idx).map_err(e2s)?;
        k.facts().map_err(e2s)?;
        fr.push(t.elapsed().as_secs_f64() * 1e3);
        Box::new(k).quit();
    }
    fr.sort_by(|a, b| a.partial_cmp(b).unwrap());
    put("return by fork + facts", format!("{:.3} ms wall median", fr[25]));
    let _ = kid_cpu;
    println!("{{");
    for (i, (k, v)) in res.iter().enumerate() {
        println!(
            "  {}: {}{}",
            json::quote(k),
            json::quote(v),
            if i + 1 < res.len() { "," } else { "" }
        );
    }
    println!("}}");
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: fmi-engine dumps|walk|verify|pool|bench ... (see the source's header)");
        std::process::exit(2);
    }
    let o = opts(&args[2..]);
    let r = match args[1].as_str() {
        "dumps" => dumps(&o),
        "walk" => walk(&o),
        "verify" => verify_cmd(&o),
        "pool" => pool_cmd(&o),
        "bench" => bench(&o),
        c => Err(format!("unknown command {c}")),
    };
    if let Err(e) = r {
        eprintln!("fmi-engine: {e}");
        std::process::exit(1);
    }
}
