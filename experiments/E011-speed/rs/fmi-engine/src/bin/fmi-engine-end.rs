//! fmi-engine-end: E012's component `end` checks (experiments/E012-game-mechanics/plan.md, "Components" and the
//! amendment "component end"; patch 0005). Every engine is FMI_ENGINE_BIN's (a build with patch 0005), seed 1.
//!
//!   fmi-engine-end hooks OUT ARCHIVE IDX [--lean] [--env K=V]...
//!       the end hooks at the entry's frame F, each case from boot in a fresh engine (and, for the ends, in a fork):
//!       the switch on with no trigger equals the switch off (frame, tick, rng, room, facts at F + 100); with
//!       SPEEDRUN_TEST_RESTART=F+5 the switch on ends the run as "restart" at F + 5, off the game restarts and runs
//!       on; the key "space" at F + 1 ends the run as "pause" at F + 1 with the switch on, and blocks it off (no
//!       reply within --timeout, default 10 s).
//!   fmi-engine-end boxes OUT ARCHIVE IDX... [--reps N] [--lean]
//!       at each entry's frame: the Boxes reply against the BoxesJson text (the JSON "boxes" reply), both against the
//!       room's BOXD block in ROOT/build/blocks-dos-cd (the count and the corners equal; flags that differ, which the
//!       scripts may have set since the room loaded, are listed), and the Boxes round trip's median over N
//!       calls (default 200). Prints one JSON line per entry and a summary.
//!   fmi-engine-end unreach OUT ARCHIVE... [--threads N] [--lean]
//!       every entry of every archive verified from boot (its checkpoints and the root's, as `fmi-engine verify`)
//!       with SPEEDRUN_REPORT_RESTART off and on: the hooks are unreachable on a recorded chain when every entry gives
//!       the same result both ways and no run ends as "restart" or "pause". OUT/unreach.jsonl gets one line per entry.
//!   fmi-engine-end probe-key OUT ARCHIVE IDX KEY FRAMES [--timeout S] [--lean] [--env K=V]...
//!       the entry's movie from boot to its frame, then KEY one frame later, then a run of FRAMES frames under a
//!       command timeout of S seconds (default 20): how the run ends.

use fmi_core::*;
use fmi_engine::archive::{self, Entry, Res};
use fmi_engine::json::{self, J};
use fmi_engine::replay;
use fmi_engine::{BinEngine, Host, HostCfg};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn e2s<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

struct Opts {
    lean: bool,
    timeout: Option<u64>,
    reps: usize,
    threads: usize,
    env: Vec<(String, String)>,
    pos: Vec<String>,
}

fn opts(args: &[String]) -> Opts {
    let mut o = Opts {
        lean: false,
        timeout: None,
        reps: 200,
        threads: 2,
        env: Vec::new(),
        pos: Vec::new(),
    };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--lean" => o.lean = true,
            "--timeout" => {
                i += 1;
                o.timeout = Some(args[i].parse().expect("--timeout S"));
            }
            "--reps" => {
                i += 1;
                o.reps = args[i].parse().expect("--reps N");
            }
            "--threads" => {
                i += 1;
                o.threads = args[i].parse().expect("--threads N");
            }
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

/// A host for the options' settings plus `env`. HostCfg::from_env reads SPEEDRUN_REPORT_RESTART from this process's
/// environment; the checks set the switch per case, so it is dropped here and given by `env` only.
fn host(o: &Opts, env: &[(&str, String)], timeout: Option<u64>) -> Res<Host> {
    let mut cfg = HostCfg::from_env();
    cfg.extra_env.retain(|(k, _)| k != "SPEEDRUN_REPORT_RESTART");
    cfg.extra_env.extend(o.env.iter().cloned());
    cfg.extra_env.extend(env.iter().map(|(k, v)| (k.to_string(), v.clone())));
    if let Some(s) = timeout.or(o.timeout) {
        cfg.timeout = Duration::from_secs(s);
    }
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

fn movie_to(ent: &Entry, frame: u64) -> Vec<Event> {
    ent.events.iter().filter(|e| e.frame <= frame).cloned().collect()
}

/// How a run command came back.
#[derive(Debug, Clone, PartialEq)]
enum Outcome {
    /// a stop: reason, frame, tick, rng, room, and the facts there
    Stop(String, u64, u64, u64, i32, Facts),
    /// an end: reason, frame, room
    Ended(String, u64, i32),
    Error(String),
}

impl Outcome {
    fn text(&self) -> String {
        match self {
            Outcome::Stop(r, f, t, g, room, facts) => format!(
                "{{\"stop\": {r:?}, \"frame\": {f}, \"tick\": {t}, \"rng\": {g}, \"room\": {room}, \"facts\": {}}}",
                facts.len()
            ),
            Outcome::Ended(r, f, room) => format!("{{\"ended\": {r:?}, \"frame\": {f}, \"room\": {room}}}"),
            Outcome::Error(e) => format!("{{\"error\": {e:?}}}"),
        }
    }
}

fn run_to(e: &mut BinEngine, frame: u64) -> Outcome {
    match e.run(&RunSpec {
        frame: Some(frame),
        ..Default::default()
    }) {
        Ok(st) => match e.facts() {
            Ok(f) => Outcome::Stop(st.reason.clone(), st.frame, st.tick, st.rng, st.room, f),
            Err(err) => Outcome::Error(err.to_string()),
        },
        Err(EngineError::Ended(st)) => Outcome::Ended(st.reason.clone(), st.frame, st.room),
        Err(err) => Outcome::Error(err.to_string()),
    }
}

/// One hook case: boot the entry's movie to F in a fresh engine with `env`, add `inputs`, then run to `to`, in the
/// engine itself and (`in_fork`) in a fork of it made at F.
fn hook_case(
    o: &Opts,
    out: &Path,
    ent: &Entry,
    env: &[(&str, String)],
    inputs: &[Event],
    to: u64,
    timeout: Option<u64>,
    in_fork: bool,
) -> Res<(Outcome, Option<Outcome>, f64)> {
    let h = host(o, env, timeout)?;
    let t = Instant::now();
    let mut e = h.boot_bin(&boot_cfg(out, &movie_to(ent, ent.frame), Some(ent.frame), o.lean)).map_err(e2s)?;
    if !inputs.is_empty() {
        e.inputs(inputs).map_err(e2s)?;
    }
    let forked = if in_fork {
        let mut c = e.fork_bin().map_err(e2s)?;
        let r = run_to(&mut c, to);
        if matches!(r, Outcome::Stop(..)) {
            Box::new(c).quit();
        }
        Some(r)
    } else {
        None
    };
    let r = run_to(&mut e, to);
    if matches!(r, Outcome::Stop(..)) {
        Box::new(e).quit();
    }
    Ok((r, forked, t.elapsed().as_secs_f64()))
}

fn hooks(o: &Opts) -> Res<()> {
    let (out, arch, idx) = (PathBuf::from(&o.pos[0]), PathBuf::from(&o.pos[1]), o.pos[2].parse::<u64>().map_err(e2s)?);
    let ent = archive::chain_of(&arch, idx)?;
    let f = ent.frame;
    let on = ("SPEEDRUN_REPORT_RESTART", "1".to_string());
    let trig = ("SPEEDRUN_TEST_RESTART", (f + 5).to_string());
    let space = [Event {
        frame: f + 1,
        what: Input::Key("space".into()),
    }];
    let mut bad = Vec::new();
    let mut say = |name: &str, r: &(Outcome, Option<Outcome>, f64), ok: bool| {
        let fk = r.1.as_ref().map(|x| x.text()).unwrap_or_else(|| "null".into());
        println!("{{\"case\": {name:?}, \"run\": {}, \"fork\": {fk}, \"seconds\": {:.3}, \"ok\": {ok}}}", r.0.text(), r.2);
        if !ok {
            bad.push(name.to_string());
        }
    };
    // 1. no trigger: the switch on equals the switch off at F + 100 (and in a fork)
    let off = hook_case(o, &out.join("off"), &ent, &[], &[], f + 100, None, true)?;
    let on0 = hook_case(o, &out.join("on"), &ent, &[on.clone()], &[], f + 100, None, true)?;
    let same = matches!(off.0, Outcome::Stop(..)) && off.0 == on0.0 && off.1 == on0.1 && off.0 == *off.1.as_ref().unwrap();
    say("off, no trigger", &off, matches!(off.0, Outcome::Stop(..)));
    say("on, no trigger (= off)", &on0, same);
    // 2. restart() at F + 5: on, the run ends there as "restart" (in the root and in a fork); off, it runs on
    let r_on = hook_case(o, &out.join("restart-on"), &ent, &[on.clone(), trig.clone()], &[], f + 100, None, true)?;
    let want = Outcome::Ended("restart".into(), f + 5, match &off.0 {
        Outcome::Stop(_, _, _, _, room, _) => *room,
        _ => -1,
    });
    let ok = r_on.0 == want && r_on.1.as_ref() == Some(&want);
    say("on, restart() at F+5", &r_on, ok);
    let r_off = hook_case(o, &out.join("restart-off"), &ent, &[trig.clone()], &[], f + 100, None, false)?;
    let ok = match (&r_off.0, &off.0) {
        (Outcome::Stop(_, fr, _, _, _, fa), Outcome::Stop(_, _, _, _, _, fb)) => *fr == f + 100 && fa != fb,
        _ => false,
    };
    say("off, restart() at F+5 (the game restarts and runs on)", &r_off, ok);
    // 3. the pause key at F + 1: on, the run ends there as "pause"; off, the pause dialog blocks the run
    let p_on = hook_case(o, &out.join("pause-on"), &ent, &[on.clone()], &space, f + 100, None, true)?;
    let room = match &off.0 {
        Outcome::Stop(_, _, _, _, room, _) => *room,
        _ => -1,
    };
    let want = Outcome::Ended("pause".into(), f + 1, room);
    let ok = p_on.0 == want && p_on.1.as_ref() == Some(&want);
    say("on, the pause key at F+1", &p_on, ok);
    let wait = o.timeout.unwrap_or(10);
    let p_off = hook_case(o, &out.join("pause-off"), &ent, &[], &space, f + 100, Some(wait), false)?;
    let ok = matches!(&p_off.0, Outcome::Error(e) if e.contains("io:")) && p_off.2 >= wait as f64;
    say("off, the pause key at F+1 (blocked)", &p_off, ok);
    println!("{{\"entry\": {idx}, \"frame\": {f}, \"cases\": 7, \"failed\": {:?}}}", bad);
    if bad.is_empty() {
        Ok(())
    } else {
        Err(format!("hook cases failed: {bad:?}"))
    }
}

/// The room's BOXD block as the game files hold it (ROOT/build/blocks-dos-cd, block LFLF_<room>/ROOM/BOXD): v5's
/// u16 count, then per box ulx, uly, urx, ury, lrx, lry, llx, lly (i16), mask, flags (u8), scale (u16).
fn boxd_of(root: &Path, room: i32) -> Res<Vec<WalkBox>> {
    let p = root
        .join("build/blocks-dos-cd/DISK_0001/LECF")
        .join(format!("LFLF_{room:04}"))
        .join("ROOM/BOXD");
    let d = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    if d.len() < 10 || &d[..4] != b"BOXD" {
        return Err(format!("{}: not a BOXD block", p.display()));
    }
    let size = u32::from_be_bytes(d[4..8].try_into().unwrap()) as usize;
    let n = u16::from_le_bytes(d[8..10].try_into().unwrap()) as usize;
    if size != d.len() || 10 + 20 * n > d.len() {
        return Err(format!("{}: size {size}, {n} boxes, {} bytes", p.display(), d.len()));
    }
    let i16at = |o: usize| i16::from_le_bytes(d[o..o + 2].try_into().unwrap()) as i32;
    Ok((0..n)
        .map(|i| {
            let b = 10 + 20 * i;
            WalkBox {
                ul: (i16at(b), i16at(b + 2)),
                ur: (i16at(b + 4), i16at(b + 6)),
                lr: (i16at(b + 8), i16at(b + 10)),
                ll: (i16at(b + 12), i16at(b + 14)),
                flags: d[b + 17],
            }
        })
        .collect())
}

fn boxes_of_json(text: &str) -> Res<Boxes> {
    let j = json::parse(text)?;
    let pt = |v: &J| -> Res<(i32, i32)> {
        let a = v.arr()?;
        if a.len() != 2 {
            return Err(format!("not a point: {v:?}"));
        }
        Ok((a[0].i32()?, a[1].i32()?))
    };
    let mut boxes = Vec::new();
    for b in j.at("boxes")?.arr()? {
        boxes.push(WalkBox {
            ul: pt(b.at("ul")?)?,
            ur: pt(b.at("ur")?)?,
            lr: pt(b.at("lr")?)?,
            ll: pt(b.at("ll")?)?,
            flags: u8::try_from(b.at("flags")?.int()?).map_err(e2s)?,
        });
    }
    Ok(Boxes {
        room: j.at("room")?.i32()?,
        boxes,
    })
}

fn boxes(o: &Opts) -> Res<()> {
    let (out, arch) = (PathBuf::from(&o.pos[0]), PathBuf::from(&o.pos[1]));
    let root = HostCfg::from_env().root;
    let h = host(o, &[], None)?;
    let (mut bad, mut rooms, mut lat) = (Vec::new(), Vec::new(), Vec::new());
    for s in &o.pos[2..] {
        let idx = s.parse::<u64>().map_err(e2s)?;
        let ent = archive::chain_of(&arch, idx)?;
        let mut e = h
            .boot_bin(&boot_cfg(&out.join(format!("e{idx}")), &movie_to(&ent, ent.frame), Some(ent.frame), o.lean))
            .map_err(e2s)?;
        let b = e.boxes().map_err(e2s)?;
        let text = e.boxes_json().map_err(e2s)?;
        let j = boxes_of_json(&text)?;
        let file = boxd_of(&root, b.room)?;
        let st_room = e.status().room;
        let mut ts = Vec::with_capacity(o.reps);
        for _ in 0..o.reps {
            let t = Instant::now();
            let again = e.boxes().map_err(e2s)?;
            ts.push(t.elapsed().as_secs_f64() * 1e6);
            if again != b {
                bad.push(format!("entry {idx}: a second Boxes reply differs"));
                break;
            }
        }
        ts.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let med = ts[ts.len() / 2];
        lat.extend(ts.iter().copied());
        let corners = |w: &WalkBox| (w.ul, w.ur, w.lr, w.ll);
        let corner_diff = b.boxes.len() != file.len()
            || b.boxes.iter().zip(&file).any(|(x, y)| corners(x) != corners(y));
        let flag_diff: Vec<usize> = b
            .boxes
            .iter()
            .zip(&file)
            .enumerate()
            .filter(|(_, (x, y))| x.flags != y.flags)
            .map(|(i, _)| i)
            .collect();
        let walkable = b.boxes.iter().filter(|w| w.flags & 0xC0 == 0).count();
        let ok_json = b == j;
        // the flags are the live ones: a script may have changed them since the room loaded (o5_matrixOps), so a flag
        // that differs from the file is reported, not failed
        let ok = ok_json && b.room == st_room && !corner_diff;
        println!(
            "{{\"entry\": {idx}, \"frame\": {}, \"room\": {}, \"boxes\": {}, \"walkable\": {walkable}, \
             \"binary_eq_json\": {ok_json}, \"corners_eq_file\": {}, \"flags_differ_from_file\": {flag_diff:?}, \
             \"boxes_us_median\": {med:.1}, \"json_bytes\": {}}}",
            ent.frame,
            b.room,
            b.boxes.len(),
            !corner_diff,
            text.len()
        );
        if !ok {
            bad.push(format!("entry {idx} (room {}): binary = JSON {ok_json}, corners = file {}, flags differ at {flag_diff:?}", b.room, !corner_diff));
        }
        rooms.push(b.room);
        Box::new(e).quit();
    }
    lat.sort_by(|a, b| a.partial_cmp(b).unwrap());
    rooms.sort_unstable();
    rooms.dedup();
    println!(
        "{{\"entries\": {}, \"rooms\": {rooms:?}, \"boxes_us_median_all\": {:.1}, \"failed\": {bad:?}}}",
        o.pos.len() - 2,
        lat.get(lat.len() / 2).copied().unwrap_or(0.0)
    );
    if bad.is_empty() && rooms.len() >= 3 {
        Ok(())
    } else {
        Err("boxes gate failed".into())
    }
}

/// One entry verified from boot (replay.verify): its whole chain's movie, compared at every checkpoint of the chain
/// (root first; a frame already checked is not run again). "same", "differ ..." or the run's error, and the end
/// reason when the run ended.
fn verify_one(h: &Host, o: &Opts, out: &Path, ent: &Entry) -> (String, Option<String>, usize, f64) {
    let mut cps = ent.checkpoints.clone();
    let mut seen = std::collections::BTreeSet::new();
    cps.retain(|c| seen.insert(c.frame));
    let n = cps.len();
    let t = Instant::now();
    let r = replay::verify(h, &boot_cfg(out, &[], None, o.lean), &ent.events, &cps, &[]);
    let _ = std::fs::remove_dir_all(out);
    let s = t.elapsed().as_secs_f64();
    match r {
        Ok(v) if v.same => ("same".into(), None, n, s),
        Ok(v) => {
            let end = v
                .differ
                .iter()
                .filter_map(|d| d.error.as_deref())
                .find_map(|e| e.strip_prefix("engine ended: ").map(str::to_string));
            (format!("differ at {:?}", v.differ.first().map(|d| d.frame)), end, n, s)
        }
        Err(EngineError::Ended(st)) => (format!("ended: {}", st.reason), Some(st.reason.clone()), n, s),
        Err(e) => (format!("error: {e}"), None, n, s),
    }
}

fn unreach(o: &Opts) -> Res<()> {
    let out = PathBuf::from(&o.pos[0]);
    std::fs::create_dir_all(&out).map_err(e2s)?;
    let h_off = host(o, &[], None)?;
    let h_on = host(o, &[("SPEEDRUN_REPORT_RESTART", "1".into())], None)?;
    let mut jobs = Vec::new();
    for (k, a) in o.pos[1..].iter().enumerate() {
        let arch = PathBuf::from(a);
        let all = Arc::new(archive::load_all(&arch)?);
        for &id in all.keys() {
            jobs.push((k, arch.clone(), all.clone(), id));
        }
    }
    let n_jobs = jobs.len();
    let jobs = Arc::new(Mutex::new(jobs.into_iter().rev().collect::<Vec<_>>()));
    let rows = Arc::new(Mutex::new(Vec::new()));
    let t0 = Instant::now();
    std::thread::scope(|s| {
        for w in 0..o.threads.max(1) {
            let (jobs, rows, h_off, h_on, out) = (jobs.clone(), rows.clone(), h_off.clone(), h_on.clone(), out.clone());
            s.spawn(move || loop {
                let Some((k, arch, all, id)) = jobs.lock().unwrap().pop() else { break };
                let row = match archive::chained(&all, id) {
                    Err(e) => (k, arch, id, 0, 0, format!("unreadable: {e}"), None, format!("unreadable: {e}"), None, 0.0),
                    Ok(ent) => {
                        let off = verify_one(&h_off, o, &out.join(format!("w{w}-off")), &ent);
                        let on = verify_one(&h_on, o, &out.join(format!("w{w}-on")), &ent);
                        (k, arch, id, ent.frame, off.2, off.0, off.1, on.0, on.1, off.3 + on.3)
                    }
                };
                rows.lock().unwrap().push(row);
            });
        }
    });
    let mut rows = Arc::try_unwrap(rows).unwrap().into_inner().unwrap();
    rows.sort_by(|a, b| (a.0, a.2).cmp(&(b.0, b.2)));
    let mut f = std::fs::File::create(out.join("unreach.jsonl")).map_err(e2s)?;
    let mut bad = 0;
    for a in 0..o.pos.len() - 1 {
        let rs: Vec<_> = rows.iter().filter(|r| r.0 == a).collect();
        let (mut same_off, mut same_on, mut equal, mut hook_ends) = (0, 0, 0, 0);
        for r in &rs {
            let hook = matches!(r.8.as_deref(), Some("restart") | Some("pause"));
            writeln!(
                f,
                "{{\"archive\": {:?}, \"id\": {}, \"frame\": {}, \"checkpoints\": {}, \"off\": {:?}, \"on\": {:?}, \
                 \"on_end\": {:?}, \"seconds\": {:.3}}}",
                r.1.display().to_string(),
                r.2,
                r.3,
                r.4,
                r.5,
                r.7,
                r.8,
                r.9
            )
            .map_err(e2s)?;
            same_off += (r.5 == "same") as usize;
            same_on += (r.7 == "same") as usize;
            equal += (r.5 == r.7) as usize;
            hook_ends += hook as usize;
        }
        let ok = equal == rs.len() && hook_ends == 0;
        bad += (!ok) as usize;
        println!(
            "{{\"archive\": {:?}, \"entries\": {}, \"same_off\": {same_off}, \"same_on\": {same_on}, \
             \"equal_off_on\": {equal}, \"restart_or_pause_ends\": {hook_ends}, \"ok\": {ok}}}",
            o.pos[a + 1],
            rs.len()
        );
    }
    println!(
        "{{\"entries\": {n_jobs}, \"archives\": {}, \"failed_archives\": {bad}, \"wall_s\": {:.1}}}",
        o.pos.len() - 1,
        t0.elapsed().as_secs_f64()
    );
    if bad == 0 {
        Ok(())
    } else {
        Err("unreach gate failed".into())
    }
}

fn probe_key(o: &Opts) -> Res<()> {
    let (out, arch, idx) = (PathBuf::from(&o.pos[0]), PathBuf::from(&o.pos[1]), o.pos[2].parse::<u64>().map_err(e2s)?);
    let (key, frames) = (o.pos[3].clone(), o.pos[4].parse::<u64>().map_err(e2s)?);
    let ent = archive::chain_of(&arch, idx)?;
    let ev = [Event {
        frame: ent.frame + 1,
        what: Input::Key(key.clone()),
    }];
    let r = hook_case(o, &out, &ent, &[], &ev, ent.frame + frames, Some(o.timeout.unwrap_or(20)), false)?;
    println!("{{\"from\": {}, \"key\": {key:?}, \"run\": {}, \"seconds\": {:.3}}}", ent.frame, r.0.text(), r.2);
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: fmi-engine-end hooks|boxes|unreach|probe-key ... (see the source's header)");
        std::process::exit(2);
    }
    let o = opts(&args[2..]);
    let r = match args[1].as_str() {
        "hooks" => hooks(&o),
        "boxes" => boxes(&o),
        "unreach" => unreach(&o),
        "probe-key" => probe_key(&o),
        c => Err(format!("unknown command {c}")),
    };
    if let Err(e) = r {
        eprintln!("fmi-engine-end: {e}");
        std::process::exit(1);
    }
}
