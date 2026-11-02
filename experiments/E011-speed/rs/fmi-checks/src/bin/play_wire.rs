//! play-wire: round 2's play messages (patch 0004) against the commands they stand for, and their cost
//! (experiments/E011-speed/results/README.md "What binds", item 4).
//!
//!   play-wire check OUT ARCHIVE IDX FROM N [--lean]
//!       two engines booted from the entry's movie to FROM; then N frames, each played on one engine by StepObs
//!       (`step_observe`) and on the other by Run, Runs, ExplainLog and Obs (the four commands it stands for), the
//!       dump alternating full, full with names and the exact object, and lite. At every frame the Status, the runs,
//!       the explain entries and the dump must be equal. Two commands in four are instead a RunObs (`run_obs`, a frame
//!       stop or a control stop) against Run then Obs, and one a RunObs with the room's dump (`run_room`) against Run,
//!       Obs and `room_cut`. Every 10th command AtMany over a grid of screen points must
//!       equal one At per point. Prints the screen layouts (Status.screen) met on the way.
//!   play-wire split OUT ARCHIVE IDX FROM N [--lean]
//!       stops change nothing: at starts every 37 frames from FROM to FROM + N, a run of 1, 2, 7, 40 and 120 frames in
//!       one command against one-frame commands (each with the room's dump), on two forks; the exact dumps, runs and
//!       explain entries at the end must be equal.
//!   play-wire bench OUT ARCHIVE IDX FROM N [--lean]
//!       the observed frame by StepObs against the four commands (wall per frame, the engine's CPU, this process's
//!       CPU), and AtMany against At for batches of 5, 20 and 80 points.

use fmi_core::*;
use fmi_engine::{BinEngine, Host, HostCfg};
use std::path::{Path, PathBuf};
use std::time::Instant;

type Res<T> = Result<T, String>;

fn e2s<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// The entry's movie (explore.py's K.json.gz "events"), through fmi-engine's JSON reader.
fn movie(arch: &Path, idx: u64) -> Res<Vec<Event>> {
    let p = arch.join("entries").join(format!("{idx}.json.gz"));
    let out = std::process::Command::new("gzip").arg("-dc").arg(&p).output().map_err(e2s)?;
    if !out.status.success() {
        return Err(format!("gzip -dc {}", p.display()));
    }
    let text = String::from_utf8(out.stdout).map_err(e2s)?;
    let j = fmi_engine::json::parse(&text)?;
    let mut v = Vec::new();
    for e in j.at("events")?.arr()? {
        let frame = e.at("frame")?.int()? as u64;
        let pt = |k: &str| -> Res<Option<(i32, i32)>> {
            match e.get(k) {
                None => Ok(None),
                Some(x) => {
                    let a = x.arr()?;
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
        } else if let Some(k) = e.get("key") {
            Input::Key(k.text()?.to_string())
        } else if let Some(s) = e.get("skips") {
            Input::Skips {
                text: s.at("text")?.int()? != 0,
                cutscenes: s.at("cutscenes")?.int()? != 0,
                spare: match s.get("spare") {
                    Some(x) => Some(x.arr()?.iter().map(|y| y.i32()).collect::<Res<_>>()?),
                    None => None,
                },
            }
        } else {
            return Err(format!("unknown event {e:?}"));
        };
        v.push(Event { frame, what });
    }
    Ok(v)
}

struct Args {
    out: PathBuf,
    arch: PathBuf,
    idx: u64,
    from: u64,
    n: u64,
    lean: bool,
}

fn args(a: &[String]) -> Res<Args> {
    let lean = a.iter().any(|x| x == "--lean");
    let p: Vec<&String> = a.iter().filter(|x| !x.starts_with("--")).collect();
    if p.len() < 5 {
        return Err("usage: play-wire check|bench OUT ARCHIVE IDX FROM N [--lean]".into());
    }
    Ok(Args {
        out: PathBuf::from(p[0]),
        arch: PathBuf::from(p[1]),
        idx: p[2].parse().map_err(e2s)?,
        from: p[3].parse().map_err(e2s)?,
        n: p[4].parse().map_err(e2s)?,
        lean,
    })
}

/// Two engines at FROM on the entry's movie, segment on, explain armed and both buffers emptied (as walk).
fn two(a: &Args, h: &Host) -> Res<(BinEngine, BinEngine)> {
    let events: Vec<Event> = movie(&a.arch, a.idx)?
        .into_iter()
        .filter(|e| e.frame <= a.from + a.n)
        .collect();
    let cfg = |d: &str| BootCfg {
        seed: 1,
        events: events.clone(),
        frame: Some(a.from),
        dump_locals: true,
        lean: a.lean,
        out_dir: a.out.join(d),
    };
    let mut x = h.boot_bin(&cfg("a")).map_err(e2s)?;
    let mut y = h.boot_bin(&cfg("b")).map_err(e2s)?;
    for e in [&mut x, &mut y] {
        e.segment(true).map_err(e2s)?;
        e.runs().map_err(e2s)?;
        e.explain(Some(&ExplainArm {
            from: a.from + 1,
            to: a.from + 100_000,
            units: vec![],
        }))
        .map_err(e2s)?;
        e.explain_log().map_err(e2s)?;
    }
    Ok((x, y))
}

/// The four commands StepObs stands for, one by one.
fn four(e: &mut BinEngine, frame: u64, req: &ObsReq) -> EResult<(Status, RunsReply, ExplainReply, Obs)> {
    let st = e.run(&RunSpec {
        frame: Some(frame),
        ..Default::default()
    })?;
    let runs = e.runs()?;
    let ents = e.explain_log()?;
    let o = e.obs(req)?;
    Ok((st, runs, ents, o))
}

fn lite_objects() -> Vec<u32> {
    let mut v: Vec<u32> = (1..400).step_by(3).collect();
    v.extend([316, 428, 566, 1038, 0, 9999]);
    v
}

fn grid() -> Vec<(i32, i32)> {
    let mut v = Vec::new();
    for y in (0..200).step_by(4) {
        for x in (0..320).step_by(8) {
            v.push((x, y));
        }
    }
    v.extend([(-1, 0), (0, -1), (320, 100), (319, 199), (100, 200), (5000, 5000)]);
    v
}

fn check(a: &Args) -> Res<()> {
    let h = Host::new(HostCfg::from_env()).map_err(e2s)?;
    let (mut x, mut y) = two(a, &h)?;
    let reqs = [
        ObsReq::default(),
        ObsReq {
            objects: None,
            names: true,
            exact: true,
        },
        ObsReq {
            objects: Some(lite_objects()),
            names: false,
            exact: false,
        },
    ];
    let pts = grid();
    let (mut bad, mut runs, mut ents, mut ats, mut runobs) = (Vec::<String>::new(), 0usize, 0usize, 0usize, 0usize);
    let mut rooms = 0usize;
    let mut screens: Vec<(Option<(i32, i32, bool)>, u64, u64)> = Vec::new();
    let mut i = 0u64;
    while x.status().frame < a.from + a.n {
        let f = x.status().frame + 1;
        let req = &reqs[(i / 4) as usize % reqs.len()];
        let (p, q);
        if i % 4 == 0 {
            p = x.step_observe(f, req).map_err(e2s)?;
            q = four(&mut y, f, req).map_err(e2s)?;
        } else if i % 4 == 3 {
            // RunObs with the room's dump against Run, then Obs and room_cut
            let spec = RunSpec {
                frame: Some(f),
                ..Default::default()
            };
            let (s1, o1) = x.run_room(&spec).map_err(e2s)?;
            let s2 = y.run(&spec).map_err(e2s)?;
            let mut o2 = y.obs(&ObsReq::default()).map_err(e2s)?;
            fmi_core::room_cut(&mut o2);
            if o1.inventory.is_none() || o1.room_objects.is_none() || o1.owners.len() > 200 {
                bad.push(format!("frame {f}: the room's dump lacks the inventory or the room's objects, or holds every object"));
            }
            p = (s1, RunsReply::default(), ExplainReply::default(), o1);
            q = (s2, RunsReply::default(), ExplainReply::default(), o2);
            rooms += 1;
        } else {
            // RunObs against Run then Obs: a frame stop, or the control stop within three frames
            let spec = if i % 4 == 1 {
                RunSpec {
                    frame: Some(f),
                    ..Default::default()
                }
            } else {
                RunSpec {
                    control: true,
                    max_frame: Some(f + 2),
                    ..Default::default()
                }
            };
            let (s1, o1) = x.run_obs(&spec, req).map_err(e2s)?;
            let s2 = y.run(&spec).map_err(e2s)?;
            let o2 = y.obs(req).map_err(e2s)?;
            p = (s1, RunsReply::default(), ExplainReply::default(), o1);
            q = (s2, RunsReply::default(), ExplainReply::default(), o2);
            runobs += 1;
        }
        i += 1;
        let f = x.status().frame;
        runs += p.1.runs.len();
        ents += p.2.entries.len();
        if p.0 != q.0 {
            bad.push(format!("frame {f}: status {:?} / {:?}", p.0, q.0));
        }
        if p.0 != *x.status() || q.0 != *y.status() {
            bad.push(format!("frame {f}: the engine's status is not the reply's"));
        }
        if p.1 != q.1 {
            bad.push(format!("frame {f}: runs differ ({} / {} rows)", p.1.runs.len(), q.1.runs.len()));
        }
        if p.2 != q.2 {
            bad.push(format!("frame {f}: explain entries differ"));
        }
        if p.3 != q.3 {
            bad.push(format!("frame {f}: the dump differs ({req:?})"));
        }
        if p.0.screen.is_none() {
            bad.push(format!("frame {f}: no screen in the status"));
        }
        match screens.last_mut() {
            Some(s) if s.0 == p.0.screen => s.2 = f,
            _ => screens.push((p.0.screen, f, f)),
        }
        if i % 10 == 0 {
            let many = x.at_many(&pts).map_err(e2s)?;
            let one: Vec<At> = pts.iter().map(|&(u, v)| y.at(u, v)).collect::<EResult<_>>().map_err(e2s)?;
            ats += pts.len();
            if many != one {
                let k = many.iter().zip(&one).position(|(m, o)| m != o).unwrap_or(0);
                bad.push(format!("frame {f}: at_many differs at {:?}: {:?} / {:?}", pts[k], many.get(k), one.get(k)));
            }
            // the empty batch, and a batch of one
            if !x.at_many(&[]).map_err(e2s)?.is_empty() || x.at_many(&pts[..1]).map_err(e2s)? != one[..1] {
                bad.push(format!("frame {f}: at_many of 0 or 1 point"));
            }
        }
    }
    // a StepObs refused as Run refuses (a frame not after now), and the engine still answers after it
    let now = x.status().frame;
    let e1 = x.step_observe(now, &ObsReq::default()).err().map(|e| e.to_string());
    let e2 = y
        .run(&RunSpec {
            frame: Some(now),
            ..Default::default()
        })
        .err()
        .map(|e| e.to_string());
    if e1.is_none() || e1 != e2 {
        bad.push(format!("a frame not after now: step_observe {e1:?} / run {e2:?}"));
    }
    if x.step_observe(now + 1, &ObsReq::default()).map_err(e2s)?.0 != four(&mut y, now + 1, &ObsReq::default()).map_err(e2s)?.0 {
        bad.push("after the refusal: status differs".into());
    }
    let ja = x.state_json(&reqs[1]).map_err(e2s)?;
    let jb = y.state_json(&reqs[1]).map_err(e2s)?;
    if ja != jb {
        bad.push("the final exact dumps differ".into());
    }
    println!(
        "{{\"frames\": {}, \"from\": {}, \"runs\": {runs}, \"explain_entries\": {ents}, \"hit_tests\": {ats}, \"run_obs\": {runobs}, \"run_room\": {rooms}, \"commands\": {i}, \"differences\": {}}}",
        a.n,
        a.from,
        bad.len()
    );
    for s in &screens {
        println!("screen {:?} at frames {}..={}", s.0, s.1, s.2);
    }
    for b in bad.iter().take(10) {
        eprintln!("play-wire: {b}");
    }
    Box::new(x).quit();
    Box::new(y).quit();
    if bad.is_empty() {
        Ok(())
    } else {
        Err("differences".into())
    }
}

fn bench(a: &Args) -> Res<()> {
    let h = Host::new(HostCfg::from_env()).map_err(e2s)?;
    let (mut x, mut y) = two(a, &h)?;
    let lite = ObsReq {
        objects: Some(lite_objects()),
        names: false,
        exact: false,
    };
    // interleaved in blocks of 25 frames so both meet the same load
    let (mut tw, mut te, mut tc) = ([0.0f64; 2], [0.0f64; 2], [0.0f64; 2]);
    let mut done = 0u64;
    while done < a.n {
        for (k, e) in [&mut x, &mut y].into_iter().enumerate() {
            let (c0, s0, t0) = (e.cpu_s(), fmi_engine::sys::self_cpu_s(), Instant::now());
            for i in 0..25 {
                let f = a.from + done + i + 1;
                if k == 0 {
                    e.step_observe(f, &lite).map_err(e2s)?;
                } else {
                    four(e, f, &lite).map_err(e2s)?;
                }
            }
            tw[k] += t0.elapsed().as_secs_f64();
            te[k] += e.cpu_s() - c0;
            tc[k] += fmi_engine::sys::self_cpu_s() - s0;
        }
        done += 25;
    }
    for (k, name) in ["step_observe (StepObs)", "run + runs + explain_log + obs"].iter().enumerate() {
        println!(
            "observed frame, lite dump, {name}: {:.4} ms wall, {:.4} ms engine CPU, {:.4} ms driver CPU ({} frames)",
            tw[k] * 1e3 / done as f64,
            te[k] * 1e3 / done as f64,
            tc[k] * 1e3 / done as f64,
            done
        );
    }
    // the full dump's decode alone (fmi-engine's wire::R::obs), on this stop's payload
    for (name, req) in [("full", ObsReq::default()), ("lite", lite.clone())] {
        let raw = x.obs_raw(&req).map_err(e2s)?;
        let t0 = Instant::now();
        let reps = 2000;
        let mut keep = 0usize;
        for _ in 0..reps {
            let o = fmi_engine::wire::R::new(&raw).obs()?;
            keep += o.classes.len();
        }
        println!(
            "decode of a {name} dump ({} bytes): {:.4} ms ({keep} class lists)",
            raw.len(),
            t0.elapsed().as_secs_f64() * 1e3 / reps as f64
        );
        let t0 = Instant::now();
        for _ in 0..200 {
            x.obs(&req).map_err(e2s)?;
        }
        println!("obs({name}) round trip: {:.4} ms", t0.elapsed().as_secs_f64() * 1e3 / 200.0);
    }
    let pts = grid();
    for &n in &[5usize, 20, 80] {
        let reps = 400 / n.min(400) * 10;
        let mut w = [0.0f64; 2];
        let mut ce = [0.0f64; 2];
        for r in 0..reps {
            let s = &pts[(r * 37) % (pts.len() - n)..][..n];
            let (c0, t0) = (x.cpu_s(), Instant::now());
            x.at_many(s).map_err(e2s)?;
            w[0] += t0.elapsed().as_secs_f64();
            ce[0] += x.cpu_s() - c0;
            let (c0, t0) = (x.cpu_s(), Instant::now());
            for &(u, v) in s {
                x.at(u, v).map_err(e2s)?;
            }
            w[1] += t0.elapsed().as_secs_f64();
            ce[1] += x.cpu_s() - c0;
        }
        println!(
            "{n} hit tests: at_many {:.4} ms wall ({:.4} engine CPU), at x{n} {:.4} ms wall ({:.4} engine CPU); per point {:.2} / {:.2} us",
            w[0] * 1e3 / reps as f64,
            ce[0] * 1e3 / reps as f64,
            w[1] * 1e3 / reps as f64,
            ce[1] * 1e3 / reps as f64,
            w[0] * 1e6 / (reps * n) as f64,
            w[1] * 1e6 / (reps * n) as f64
        );
    }
    Box::new(x).quit();
    Box::new(y).quit();
    Ok(())
}

/// Frame costs by bridge setting: N single-frame runs from FROM on forks of one engine, each with segment and
/// explain set as named; the engine's CPU per frame.
fn frames(a: &Args) -> Res<()> {
    let h = Host::new(HostCfg::from_env()).map_err(e2s)?;
    let (mut x, y) = two(a, &h)?;
    Box::new(y).quit();
    x.segment(false).map_err(e2s)?;
    x.explain(None).map_err(e2s)?;
    for round in 0..2 {
        for (name, seg, expl, obs) in [
            ("plain", false, false, false),
            ("segment", true, false, false),
            ("segment + explain", true, true, false),
            ("segment + explain + lite dump (StepObs)", true, true, true),
            ("50-frame commands, plain", false, false, false),
        ] {
            let mut k = x.fork_bin().map_err(e2s)?;
            k.segment(seg).map_err(e2s)?;
            if expl {
                k.explain(Some(&ExplainArm {
                    from: a.from + 1,
                    to: a.from + 100_000,
                    units: vec![],
                }))
                .map_err(e2s)?;
            }
            let (c0, t0) = (k.cpu_s(), Instant::now());
            let f0 = k.status().frame;
            if name.starts_with("50") {
                for i in 0..a.n / 50 {
                    k.run(&RunSpec {
                        frame: Some(f0 + 50 * (i + 1)),
                        ..Default::default()
                    })
                    .map_err(e2s)?;
                }
            } else {
                for i in 0..a.n {
                    if obs {
                        k.step_observe(f0 + i + 1, &ObsReq { objects: Some(lite_objects()), names: false, exact: false })
                            .map_err(e2s)?;
                    } else {
                        k.run(&RunSpec {
                            frame: Some(f0 + i + 1),
                            ..Default::default()
                        })
                        .map_err(e2s)?;
                        if seg {
                            k.runs().map_err(e2s)?;
                        }
                        if expl {
                            k.explain_log().map_err(e2s)?;
                        }
                    }
                }
            }
            let fr = k.status().frame - f0;
            println!(
                "round {round} {name}: {fr} frames, {:.4} ms engine CPU a frame, {:.4} ms wall a frame",
                (k.cpu_s() - c0) * 1e3 / fr as f64,
                t0.elapsed().as_secs_f64() * 1e3 / fr as f64
            );
            Box::new(k).quit();
        }
    }
    // the booted engine itself (no fork: no copy-on-write faults), plain frames
    for _ in 0..2 {
        let (c0, t0, f0) = (x.cpu_s(), Instant::now(), x.status().frame);
        for i in 0..a.n {
            x.run(&RunSpec {
                frame: Some(f0 + i + 1),
                ..Default::default()
            })
            .map_err(e2s)?;
        }
        println!(
            "unforked engine, plain: {} frames, {:.4} ms engine CPU a frame, {:.4} ms wall a frame",
            a.n,
            (x.cpu_s() - c0) * 1e3 / a.n as f64,
            t0.elapsed().as_secs_f64() * 1e3 / a.n as f64
        );
    }
    Box::new(x).quit();
    Ok(())
}

/// Stops are transparent: from forks of one engine, N frames in one command against N one-frame commands (each with
/// the room's dump, as a stone trial runs), every exact dump at the end equal; at several starts and lengths.
fn split(a: &Args) -> Res<()> {
    let h = Host::new(HostCfg::from_env()).map_err(e2s)?;
    let (mut x, y) = two(a, &h)?;
    Box::new(y).quit();
    let exact = ObsReq {
        objects: None,
        names: true,
        exact: true,
    };
    let (mut bad, mut cases) = (0, 0);
    let mut start = a.from;
    while start < a.from + a.n {
        for len in [1u64, 2, 7, 40, 120] {
            let mut p = x.fork_bin().map_err(e2s)?;
            let mut q = x.fork_bin().map_err(e2s)?;
            let f0 = p.status().frame;
            p.run(&RunSpec {
                frame: Some(f0 + len),
                ..Default::default()
            })
            .map_err(e2s)?;
            for i in 0..len {
                q.run_room(&RunSpec {
                    frame: Some(f0 + i + 1),
                    ..Default::default()
                })
                .map_err(e2s)?;
            }
            let (jp, jq) = (p.state_json(&exact).map_err(e2s)?, q.state_json(&exact).map_err(e2s)?);
            let (rp, rq) = (p.runs().map_err(e2s)?, q.runs().map_err(e2s)?);
            let (ep, eq) = (p.explain_log().map_err(e2s)?, q.explain_log().map_err(e2s)?);
            cases += 1;
            if jp != jq || rp != rq || ep != eq || p.status().frame != q.status().frame {
                bad += 1;
                eprintln!(
                    "split: from {f0}, {len} frames: one command and {len} commands differ (dump {}, runs {}, explain {})",
                    jp == jq,
                    rp == rq,
                    ep == eq
                );
            }
            Box::new(p).quit();
            Box::new(q).quit();
        }
        // on to the next start
        let f = x.status().frame + 37;
        x.run(&RunSpec {
            frame: Some(f),
            ..Default::default()
        })
        .map_err(e2s)?;
        start = f;
    }
    println!("{{\"cases\": {cases}, \"differences\": {bad}}}");
    Box::new(x).quit();
    if bad == 0 {
        Ok(())
    } else {
        Err("differences".into())
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let r = match (a.get(1).map(String::as_str), args(&a[2.min(a.len())..])) {
        (Some("check"), Ok(x)) => check(&x),
        (Some("bench"), Ok(x)) => bench(&x),
        (Some("frames"), Ok(x)) => frames(&x),
        (Some("split"), Ok(x)) => split(&x),
        (_, Err(e)) => Err(e),
        _ => Err("usage: play-wire check|bench OUT ARCHIVE IDX FROM N [--lean]".into()),
    };
    if let Err(e) = r {
        eprintln!("play-wire: {e}");
        std::process::exit(1);
    }
}
