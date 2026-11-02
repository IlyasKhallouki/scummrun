//! fmi-play's differential tests against the fixtures experiments/E011-speed/play/capture.py records from E009's
//! step test (a) (plan.md, "play", tests 1-4), through the test doubles (svm-w3b over the JSON protocol, the reading's
//! four files).
//!
//! usage: play-fixture recs FIXTURES            the plan's records as the search tracks them, against the capture
//!        play-fixture children FIXTURES [W]    test 2: every child from its fork frame, the capture's shared reads
//!        play-fixture strict FIXTURES [RUNS..] test 3: the three plans' windows, strict, fork order; the records
//!                                              against the capture's and other recorded runs' windows.jsonl
//!        play-fixture ready FIXTURES           test 4: the same with play when ready
//!        play-fixture frames FIXTURES N        the observed child frame's cost, N frames from test (a)'s third window
//! FIXTURES is the capture's output folder (plans.jsonl, windows.jsonl, children.jsonl, step-a/).

use fmi_core::*;
use fmi_play::file_reading::FileReading;
use fmi_play::json_engine::{event_of, JsonHost};
use fmi_play::pyj::{self, Py, J};
use fmi_play::reading::{PlayReading, ReadingData};
use fmi_play::recs::*;
use fmi_play::strict::Track;
use fmi_play::val::*;
use fmi_play::window::{self, ChildOracle, Config, Search};
use rustc_hash::{FxHashMap, FxHashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

fn lines(p: &Path) -> Vec<J> {
    std::fs::read_to_string(p)
        .unwrap_or_else(|e| panic!("{}: {e}", p.display()))
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| pyj::parse(l).unwrap_or_else(|e| panic!("{}: {e}", p.display())))
        .collect()
}

fn vals_of(j: &J) -> Vec<Val> {
    let mut v: Vec<Val> = j.arr().iter().map(val_of).collect();
    v.sort();
    v.dedup();
    v
}

fn conds_of(j: &J) -> Vec<(Key, Vec<Val>)> {
    j.arr()
        .iter()
        .map(|p| {
            let a = p.arr();
            (Key::parse(a[0].str().unwrap_or("")), vals_of(&a[1]))
        })
        .collect()
}

fn action_of(j: &J) -> Action {
    Action {
        name: j.s("name").unwrap_or("").to_string(),
        pre: conds_of(j.get("pre").unwrap_or(&J::Null)),
        effects: j
            .a("effects")
            .iter()
            .map(|e| Effect {
                key: Key::parse(e.s("key").unwrap_or("")),
                v: if e.get("add").is_some_and(|x| !x.is_null()) {
                    EffVal::Add(e.i("add").unwrap())
                } else if e.get("values").is_some_and(|x| !x.is_null()) {
                    EffVal::Values(vals_of(e.get("values").unwrap()))
                } else {
                    EffVal::Value(val_of(e.get("value").unwrap_or(&J::Null)))
                },
                when: conds_of(e.get("when").unwrap_or(&J::Null)),
            })
            .collect(),
        what: j.i("what").unwrap_or(0) as u32,
        cost: j.i("cost").unwrap_or(1),
    }
}

pub fn plan_result(p: &J) -> PlanResult {
    let steps = p
        .a("steps")
        .iter()
        .map(|s| PlanStep {
            op: s.s("op").unwrap_or("").into(),
            action: s.i("action").unwrap_or(0) as u32,
            name: s.s("name").unwrap_or("").into(),
            chosen: s
                .a("chosen")
                .iter()
                .map(|c| (Key::parse(c.arr()[0].str().unwrap_or("")), val_of(&c.arr()[1])))
                .collect(),
            guessed: s
                .a("guessed")
                .iter()
                .map(|g| {
                    let a = g.arr();
                    (Key::parse(a[0].str().unwrap_or("")), val_of(&a[1]), pyj::dumps(&a[2]))
                })
                .collect(),
        })
        .collect();
    let actions: Vec<Action> = p.a("actions").iter().map(action_of).collect();
    let mut choices: Vec<(u32, Vec<Action>)> = p
        .get("choices")
        .map(|c| {
            c.obj()
                .iter()
                .map(|(k, v)| (k.parse().unwrap(), v.arr().iter().map(action_of).collect()))
                .collect()
        })
        .unwrap_or_default();
    choices.sort_by_key(|c| c.0);
    PlanResult {
        status: "plan".into(),
        steps,
        paths: actions.clone(),
        actions,
        choices,
        source: "search".into(),
        ..Default::default()
    }
}

fn view_of(j: &J) -> View {
    let mut v = View::new();
    for p in j.arr() {
        let a = p.arr();
        let k = Key::parse(a[0].str().unwrap_or(""));
        let x = match &a[1] {
            J::Obj(o) if o.first().is_some_and(|e| e.0 == "$set") => VV::Set(Arc::from(vals_of(&o[0].1))),
            x => VV::V(val_of(x)),
        };
        v.set(k, x);
    }
    v
}

struct Fx {
    dir: PathBuf,
    plans: Vec<J>,
    windows: Vec<J>,
    children: Vec<J>,
    reading: FileReading,
}

fn load(dir: &Path) -> Fx {
    let plans = lines(&dir.join("plans.jsonl"));
    let windows = lines(&dir.join("windows.jsonl"));
    let children = lines(&dir.join("children.jsonl"));
    let rpath = PathBuf::from(plans[0].s("reading").unwrap());
    let t = Instant::now();
    let reading = FileReading::open(&rpath).unwrap_or_else(|e| panic!("{}: {e}", rpath.display()));
    eprintln!(
        "reading {}: {} actions, {:.2} s",
        rpath.display(),
        reading.n_actions(),
        t.elapsed().as_secs_f64()
    );
    Fx {
        dir: dir.to_path_buf(),
        plans,
        windows,
        children,
        reading,
    }
}

fn unobserved() -> Prefixes {
    Prefixes::of(&["args:", "saved:", "actorroom:"])
}

fn key_set(j: &J) -> Vec<String> {
    let mut v: Vec<String> = j.arr().iter().filter_map(|x| x.str().map(String::from)).collect();
    v.sort();
    v
}

fn sorted_texts(s: &FxHashSet<Key>) -> Vec<String> {
    let mut v: Vec<String> = s.iter().map(|k| k.text()).collect();
    v.sort();
    v
}

/// The plan's records rebuilt for window `w` (from its plan's start belief), with the capture's done flags.
fn recs_for(rd: &PlayReading, plan: &J, win: &J) -> (Vec<MRec>, Vec<Vec<usize>>, Vec<String>) {
    let res = plan_result(plan);
    let un = unobserved();
    let crecs = win.a("recs");
    let belief = view_of(crecs[0].get("before").unwrap());
    let mut recs = build_recs(&RecsCtx {
        rd,
        result: &res,
        unobserved: &un,
        belief,
    });
    let macros = macro_steps(&mut recs);
    let goal = conds_of(plan.get("goal").unwrap());
    checked_keys(&mut recs, &goal);
    let mut diffs = Vec::new();
    if recs.len() != crecs.len() {
        diffs.push(format!("{} records, the capture {}", recs.len(), crecs.len()));
    }
    for (r, c) in recs.iter_mut().zip(crecs) {
        let mut d = |what: &str, a: String, b: String| {
            if a != b {
                diffs.push(format!("rec {} {}: {what}: rust {a} / python {b}", r.i, r.name));
            }
        };
        d("name", r.name.clone(), c.s("name").unwrap_or("").into());
        d(
            "kind",
            format!("{:?}", r.kind).to_lowercase(),
            c.s("kind").unwrap_or("").into(),
        );
        d(
            "unit",
            format!("{:?}", r.unit.map(|u| u.text())),
            format!("{:?}", c.s("unit")),
        );
        d("wait", format!("{:?}", r.wait), format!("{:?}", c.i("wait")));
        d("cost", r.cost.to_string(), c.i("cost").unwrap_or(-9).to_string());
        d(
            "reads",
            format!("{:?}", sorted_texts(&r.reads)),
            format!("{:?}", key_set(c.get("reads").unwrap())),
        );
        d(
            "writes",
            format!("{:?}", sorted_texts(&r.writes)),
            format!("{:?}", key_set(c.get("writes").unwrap())),
        );
        d(
            "preds",
            format!("{:?}", r.preds),
            format!("{:?}", c.a("preds").iter().filter_map(J::int).collect::<Vec<_>>()),
        );
        d("macro", r.macro_.to_string(), c.i("macro").unwrap_or(-1).to_string());
        d(
            "checked",
            format!("{:?}", r.checked.as_ref().map(sorted_texts)),
            format!("{:?}", c.get("checked").filter(|x| !x.is_null()).map(key_set)),
        );
        let ex: Vec<Vec<String>> = r
            .exps
            .iter()
            .map(|e| {
                e.iter()
                    .map(|(k, x)| {
                        format!(
                            "{}={:?}/{}/{:?}/{}",
                            k.text(),
                            txt(&x.values),
                            x.kind,
                            x.chosen.map(vstr),
                            x.changes
                        )
                    })
                    .collect()
            })
            .collect();
        let cx: Vec<Vec<String>> = c
            .a("exps")
            .iter()
            .map(|e| {
                e.arr()
                    .iter()
                    .map(|p| {
                        let a = p.arr();
                        let x = &a[1];
                        let mut vs: Vec<String> = x.a("values").iter().map(|v| Py::from_j(v).str()).collect();
                        vs.sort();
                        format!(
                            "{}={:?}/{}/{:?}/{}",
                            a[0].str().unwrap(),
                            vs,
                            x.s("kind").unwrap_or(""),
                            x.get("chosen").filter(|c| !c.is_null()).map(|c| vstr(val_of(c))),
                            x.get("changes").map(J::truthy).unwrap_or(false)
                        )
                    })
                    .collect()
            })
            .collect();
        d("exps", format!("{ex:?}"), format!("{cx:?}"));
        let pr: Vec<Vec<String>> = r
            .pres
            .iter()
            .map(|p| p.iter().map(|(k, v)| format!("{}={:?}", k.text(), txt(v))).collect())
            .collect();
        let cp: Vec<Vec<String>> = c
            .a("pres")
            .iter()
            .map(|p| {
                p.arr()
                    .iter()
                    .map(|q| {
                        let a = q.arr();
                        let mut vs: Vec<String> = a[1].arr().iter().map(|v| Py::from_j(v).str()).collect();
                        vs.sort();
                        format!("{}={vs:?}", a[0].str().unwrap())
                    })
                    .collect()
            })
            .collect();
        d("pres", format!("{pr:?}"), format!("{cp:?}"));
        let bv = view_of(c.get("before").unwrap());
        if bv != r.before {
            let mut ks: Vec<Key> = bv.m.keys().chain(r.before.m.keys()).copied().collect();
            ks.sort();
            ks.dedup();
            let bad: Vec<String> = ks
                .iter()
                .filter(|k| bv.get(**k) != r.before.get(**k))
                .take(5)
                .map(|k| format!("{}: {:?}/{:?}", k.text(), r.before.get(*k), bv.get(*k)))
                .collect();
            diffs.push(format!("rec {} before differs: {bad:?}", r.i));
        }
        r.done = c.get("done").map(J::truthy).unwrap_or(false);
        r.done_frame = c.i("done_frame").map(|x| x as u64);
        r.path = c.i("path").unwrap_or(0) as usize;
    }
    (recs, macros, diffs)
}

fn cmd_recs(fx: &Fx) -> bool {
    let data = Arc::new(ReadingData::new(&fx.reading, true));
    let rd = PlayReading::new(&fx.reading, data);
    let mut ok = true;
    for w in &fx.windows {
        let plan = &fx.plans[w.i("plan").unwrap() as usize];
        let t = Instant::now();
        let (recs, macros, diffs) = recs_for(&rd, plan, w);
        let goal = conds_of(plan.get("goal").unwrap());
        let gw = window::goal_writers_of(&recs, &goal);
        let mut gws: Vec<i64> = gw.iter().map(|&i| i as i64).collect();
        gws.sort();
        let cgw: Vec<i64> = w.a("goal_writers").iter().filter_map(J::int).collect();
        let sp = fmi_play::strict::Plan::new(&rd, &recs);
        let mut idx_bad = 0;
        for (i, v) in w.get("splan_idx").unwrap().obj() {
            let mine = &sp.idx[&i.parse::<usize>().unwrap()];
            let theirs: Vec<Option<u32>> = v.arr().iter().map(|x| x.int().map(|n| n as u32)).collect();
            if *mine != theirs {
                idx_bad += 1;
            }
        }
        let k = w.i("k").unwrap() as usize;
        let members: Vec<usize> = w
            .a("members")
            .iter()
            .filter_map(|x| x.int().map(|n| n as usize))
            .collect();
        let mm = macros.get(k).cloned().unwrap_or_default();
        println!(
            "window {} (plan {} macro {k}): {} records, {} macros, {} differences, goal writers {}, plan idx {}, members {}; {:.3} s",
            w.i("wi").unwrap(),
            w.i("plan").unwrap(),
            recs.len(),
            macros.len(),
            diffs.len(),
            if gws == cgw { "equal".to_string() } else { format!("rust {gws:?} python {cgw:?}") },
            if idx_bad == 0 { "equal".to_string() } else { format!("{idx_bad} differ") },
            if mm == members { "equal".to_string() } else { format!("rust {mm:?} python {members:?}") },
            t.elapsed().as_secs_f64()
        );
        for d in diffs.iter().take(12) {
            println!("   {d}");
        }
        ok &= diffs.is_empty() && gws == cgw && idx_bad == 0 && mm == members;
    }
    println!("RECS: {}", if ok { "equal" } else { "DIFFER" });
    ok
}

// --- the engine ------------------------------------------------------------------------------------------------------

fn boot(fx: &Fx, movie: &J, frame: u64, tag: &str) -> Box<dyn Engine> {
    let events: Vec<Event> = movie.arr().iter().filter_map(event_of).collect();
    let out = fx.dir.join("rust").join(tag);
    let _ = std::fs::remove_dir_all(&out);
    let cfg = BootCfg {
        seed: 1,
        events,
        frame: Some(frame),
        dump_locals: true,
        lean: false,
        out_dir: out,
    };
    let t = Instant::now();
    let e = JsonHost::default()
        .boot_json(&cfg)
        .unwrap_or_else(|e| panic!("boot {tag}: {e}"));
    eprintln!("boot {tag} to frame {frame}: {:.2} s", t.elapsed().as_secs_f64());
    Box::new(e)
}

/// The strict matches carried into a window. Their run seqs are the recorded run's engine's; an engine booted
/// for the test counts its runs from its own segment start, so every carried seq is moved below every new run's
/// (each new run began after every carried one), keeping their own order.
fn track_of(j: &J) -> Track {
    const BELOW_NEW: i64 = 1_000_000_000_000;
    let seq = |x: &J| -> i64 {
        match x {
            J::Int(-1) => -1000,
            J::Int(n) => n * 1000 - BELOW_NEW,
            J::Float(f) => (f * 1000.0).round() as i64 - BELOW_NEW,
            _ => -1000,
        }
    };
    let mut t = Track::default();
    for (i, v) in j.get("matched").map(J::obj).unwrap_or(&[]) {
        let a = v.arr();
        t.matched.push((
            i.parse().unwrap(),
            (
                a[0].int().unwrap_or(0) as u64,
                a[1].int().unwrap_or(0) as usize,
                seq(&a[2]),
            ),
        ));
    }
    for (i, v) in j.get("first_seq").map(J::obj).unwrap_or(&[]) {
        t.first_seq.insert(i.parse().unwrap(), seq(v));
    }
    for (i, v) in j.get("unit_seq").map(J::obj).unwrap_or(&[]) {
        t.unit_seq.insert(
            i.parse().unwrap(),
            v.obj().iter().map(|(u, s)| (Unit::parse(u).unwrap(), seq(s))).collect(),
        );
    }
    for (i, v) in j.get("choices").map(J::obj).unwrap_or(&[]) {
        t.choices.push((
            i.parse().unwrap(),
            v.arr()
                .iter()
                .map(|p| (Key::parse(p.arr()[0].str().unwrap_or("")), VV::V(val_of(&p.arr()[1]))))
                .collect(),
        ));
    }
    for (i, v) in j.get("outside").map(J::obj).unwrap_or(&[]) {
        t.outside.push((
            i.parse().unwrap(),
            v.arr()
                .iter()
                .map(|p| (Key::parse(p.arr()[0].str().unwrap_or("")), p.arr()[1].int()))
                .collect(),
        ));
    }
    assert!(
        j.get("refuted").map(J::obj).unwrap_or(&[]).is_empty(),
        "a refuted record carried into a window"
    );
    t
}

/// The capture's answers to one window's children's shared reads (test 2).
struct Oracle {
    reads: FxHashMap<u64, Vec<(u64, u64, Option<u64>)>>, // g -> [(frame, qf, best q_end)]
    dup: FxHashMap<u64, bool>,
    stop_at: Option<u64>,
}

impl ChildOracle for Oracle {
    fn pruned(&self, g: u64, f: u64, qf: u64) -> Option<bool> {
        let r = self.reads.get(&g)?;
        let x = r.iter().find(|x| x.0 == f)?;
        Some(x.2.is_some_and(|b| qf >= b))
    }
    fn duplicate(&self, g: u64, _first: u64) -> Option<bool> {
        self.dup.get(&g).copied()
    }
    fn parent_stop(&self, g: u64) -> Option<bool> {
        Some(self.stop_at == Some(g))
    }
}

fn cmd_children(fx: &Fx, only: Option<usize>) -> bool {
    let data = Arc::new(ReadingData::new(&fx.reading, true));
    let rd = PlayReading::new(&fx.reading, data.clone());
    let mut all_ok = true;
    let mut total = (0usize, 0usize);
    for w in &fx.windows {
        let wi = w.i("wi").unwrap() as usize;
        if only.is_some_and(|o| o != wi) {
            continue;
        }
        let plan = &fx.plans[w.i("plan").unwrap() as usize];
        let (recs, _macros, diffs) = recs_for(&rd, plan, w);
        if !diffs.is_empty() {
            println!("window {wi}: the records differ ({}); run `recs` first", diffs.len());
            all_ok = false;
            continue;
        }
        let rec = w.get("rec").unwrap();
        let kids: Vec<&J> = fx.children.iter().filter(|c| c.i("wi") == Some(wi as i64)).collect();
        let mut reads: FxHashMap<u64, Vec<(u64, u64, Option<u64>)>> = FxHashMap::default();
        let mut dup = FxHashMap::default();
        for c in &kids {
            let g = c.i("g").unwrap() as u64;
            dup.insert(g, c.s("outcome") == Some("duplicate"));
            let mut v = Vec::new();
            for r in c.a("reads") {
                let a = r.arr();
                if a[0].str() == Some("best") {
                    v.push((
                        a[1].int().unwrap_or(0) as u64,
                        a[2].int().unwrap_or(0) as u64,
                        a[4].int().map(|x| x as u64),
                    ));
                }
            }
            reads.insert(g, v);
        }
        let stop_at = if rec.s("end_reason").unwrap_or("").starts_with("stop:") {
            rec.i("end").map(|x| x as u64)
        } else {
            None
        };
        let oracle: &'static Oracle = Box::leak(Box::new(Oracle { reads, dup, stop_at }));
        let mut cfg = Config::e009(6);
        cfg.workers = std::env::var("FMI_WORKERS")
            .ok()
            .and_then(|x| x.parse().ok())
            .unwrap_or(3);
        let mut search = Search::new(cfg).unwrap();
        search.oracle = Some(oracle);
        search.strack = track_of(w.get("strack").unwrap());
        search.goal_writers = w
            .a("goal_writers")
            .iter()
            .filter_map(|x| x.int().map(|n| n as usize))
            .collect();
        let game = search.game().unwrap();
        let code = fmi_play::code::Code::new(game).with_facts(data.facts.clone());
        let sp = fmi_play::strict::Plan::new(&rd, &recs);
        let members: Vec<usize> = w
            .a("members")
            .iter()
            .filter_map(|x| x.int().map(|n| n as usize))
            .collect();
        let tracked: Vec<usize> = w
            .a("tracked")
            .iter()
            .filter_map(|x| x.int().map(|n| n as usize))
            .collect();
        let needed: FxHashSet<usize> = w
            .a("needed")
            .iter()
            .filter_map(|x| x.int().map(|n| n as usize))
            .collect();
        let start = w.i("start").unwrap() as u64;
        let mut parent = boot(fx, w.get("movie").unwrap(), start, &format!("w{wi}"));
        let goal = conds_of(plan.get("goal").unwrap());
        let kept = view_of(w.get("kept").unwrap());
        let un = unobserved();
        let t = Instant::now();
        let res = search
            .window_strict(
                &rd,
                &code,
                &sp,
                &recs,
                &members,
                &tracked,
                &needed,
                &mut parent,
                &goal,
                &kept,
                w.i("plan").unwrap() as u32,
                w.i("k").unwrap() as usize,
                &un,
            )
            .unwrap_or_else(|e| panic!("window {wi}: {e}"));
        let wall = t.elapsed().as_secs_f64();
        // every child's row as it ended (children.jsonl: before window.py moved the best's fork frame)
        let mine: Vec<&J> = res.rows.iter().map(|r| &r.1).collect();
        let theirs: Vec<&J> = kids.iter().map(|c| c.get("row").unwrap()).collect();
        let by_g: FxHashMap<i64, &J> = theirs.iter().map(|r| (r.arr()[0].int().unwrap(), *r)).collect();
        let mut same = 0;
        let mut bad = Vec::new();
        for r in &mine {
            let g = r.arr()[0].int().unwrap();
            match by_g.get(&g) {
                Some(t) if *r == *t => same += 1,
                Some(t) => bad.push(format!("rust {} / python {}", pyj::dumps(r), pyj::dumps(t))),
                None => bad.push(format!("rust {} / python: no child", pyj::dumps(r))),
            }
        }
        let missing = theirs.len().saturating_sub(mine.len());
        println!(
            "window {wi}: {} children (python {}), {} equal, {} differ, {} missing; best {} (python {}); {:.1} s",
            mine.len(),
            theirs.len(),
            same,
            bad.len(),
            missing,
            res.best.as_ref().map(|b| b.g.to_string()).unwrap_or("None".into()),
            rec.get("best")
                .and_then(|b| b.i("g"))
                .map(|x| x.to_string())
                .unwrap_or("None".into()),
            wall
        );
        for b in bad.iter().take(10) {
            println!("   {b}");
        }
        total.0 += same;
        total.1 += theirs.len();
        all_ok &= bad.is_empty() && missing == 0;
        if let Some(b) = res.best {
            if let Some(e) = b.snapshot {
                e.quit();
            }
            if let Some(e) = b.live {
                e.quit();
            }
        }
        parent.quit();
    }
    println!(
        "CHILDREN: {} of {} equal: {}",
        total.0,
        total.1,
        if all_ok { "PASS" } else { "FAIL" }
    );
    all_ok
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 {
        eprintln!("usage: play-fixture recs|children|strict|ready|frames FIXTURES [...]");
        std::process::exit(2);
    }
    let fx = load(Path::new(&a[2]));
    let ok = match a[1].as_str() {
        "recs" => cmd_recs(&fx),
        "children" => cmd_children(&fx, a.get(3).and_then(|x| x.parse().ok())),
        _ => {
            eprintln!("unknown command {}", a[1]);
            false
        }
    };
    std::process::exit(if ok { 0 } else { 1 });
}
