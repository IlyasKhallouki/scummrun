//! E013 gate C2 (experiments/E013-robust/plan.md, section C and the amendment "component `stall`"), on generated
//! archives: the table kept after each commit (the driver's way) equals the table from the archive's files at that
//! commit, and so do the shift's share, the cell rule and the kind sequence they give free's rule; and the mechanism of
//! C3: a driver run with FMI_STALLS=1 leaves the stalls.json that `fmi explore stalls` makes from the same files, also
//! across a resume, and with the switch unset writes none.

use fmi_explore::archive::Archive;
use fmi_explore::driver::{self, DriverStats, JobLog, Step, StepMode};
use fmi_explore::files;
use fmi_explore::progress::{self, TAtom};
use fmi_explore::stall::{self, Class, Stalls};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

static N: AtomicUsize = AtomicUsize::new(0);
/// The switches are the process's environment: one test at a time.
static ENV: Mutex<()> = Mutex::new(());

fn env(stalls: bool) -> MutexGuard<'static, ()> {
    let g = ENV.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var(progress::SELECT_ENV, progress::PROGRESS);
    if stalls {
        std::env::set_var(stall::STALLS_ENV, "1");
    } else {
        std::env::remove_var(stall::STALLS_ENV);
    }
    g
}

fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("stall-{}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst), name));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn guide() -> Value {
    json!({
        "format": "e008-guide-1", "goal": {"owner:566": [1]}, "map": "direct",
        "rooms": {"start": 33, "edges": {"33": [28, 35, 38], "28": [33, 41], "41": [28], "35": [33], "38": [33]},
                  "reached": [28, 33, 35, 38, 41]},
        "atoms": [["owner:566", 1, 0], ["room", 41, 1], ["state:316", 1, 2], ["var:103", null, 2], ["bit:7", 1, 3],
                  ["room", 28, 3]],
        "keys": {"owner:566": 0, "room": 1, "state:316": 2, "var:103": 2, "bit:7": 3},
        "writers": {"owner:566=1": ["obj:566"]}, "stats": {}})
}

fn cp(frame: i64, facts: Value, tick: i64) -> Value {
    json!([frame, facts, {"tick": tick, "rng": 1, "actors": []}])
}

fn make(t: &Path, e009: Option<Value>) -> Archive {
    let gp = t.join("GUIDE.json");
    std::fs::write(&gp, guide().to_string()).unwrap();
    let mut a = Archive::create(&t.join("arch"), "test", &json!({"owner:566": [1]}), &gp, &[28, 33, 35, 38, 41], 1,
                                Some(json!({"frame": 100, "tick": 0, "qf": 1000, "room": 33})), e009, None).unwrap();
    a.add(None, 100, Some(0), &[], &[cp(100, json!({"room": 33, "ego": 1, "owner:500": 1, "var:5": 7}), 0)], None, "boot").unwrap();
    a.write_cells().unwrap();
    a
}

/// One scripted try of job number n from its job.json: what it does cycles with n (the stops of the class table,
/// entries new or not, an affordance step, the rows' records), so that the table has every kind of row. With `gain`,
/// every try's wait also keeps a stop with a new atom (the driver's runs then never stagnate).
fn script(d: &Path, n: usize, gain: bool) -> Value {
    let job = files::read_json(&d.join("job.json")).unwrap();
    let entry = job["entry"]["id"].as_u64().unwrap() as usize;
    let c = job["entry"]["checkpoint"].clone();
    let f0 = c[0].as_i64().unwrap();
    let base = c[1].clone();
    let w = |input: &str, end: &str, oc: Value, s: f64| json!({"input": input, "end_reason": end, "outcomes": oc, "wall_s": s});
    // the new entries: (frames after the entry, the facts changed, how)
    let k = n % 6;
    let mut news: Vec<(i64, Vec<(&str, Value)>, &str)> = Vec::new();
    let tested: Vec<TAtom> = vec![TAtom::value("var:9", n as i64 + 1), TAtom::value("state:316", 1)];
    let (stop, windows, afford): (Value, Vec<Value>, Value) = match k {
        // a new atom of T from the attempt
        0 => {
            news.push((3, vec![("var:9", json!(n + 1))], "checkpoint"));
            (json!("goal"), vec![w("walk_to(1)#2", "ready: the first end point in the order of tries", json!({"goal": 1}), 0.4)], Value::Null)
        }
        // R0, then a dialog step that keeps something not new
        1 => {
            news.push((2, vec![("var:5", json!(7))], "checkpoint"));
            news.push((9, vec![("var:5", json!(7))], "afford"));
            // E014: a trial of the step ended the game through a script (an end entry, its dead move "afford")
            news.push((11, vec![("var:5", json!(7))], "end+"));
            (json!("refinement: Lemma R's premise R0: the last pass wrote values of var [194] outside the domain"),
             vec![w("walk_to(1)#2", "ready: the first end point in the order of tries", json!({"deviation": 1}), 0.2)],
             json!([{"why": "R0", "offer": {"idle": "dialog"}, "kept": 1, "step_seconds": 0.5}]))
        }
        // the same empty window, refused at the interface; the step keeps a new room
        2 => {
            news.push((4, vec![("room", json!(35))], "afford"));
            let s = "the same empty window 2 times on sentence(10,452)#2636: no cause";
            (json!(s), vec![w("sentence(10,452)#2636", "3600 frames", json!({"interface": 296}), 2.4),
                            w("sentence(10,452)#2636", "3600 frames", json!({"interface": 296}), 3.4)],
             json!([{"why": "empty window", "offer": {"idle": "sentence"}, "kept": 1, "step_seconds": 7.5}]))
        }
        // the same empty window, every try spent: effect mismatch
        3 => (json!("the same empty window 2 times on walk_to(428)#64: x"),
              vec![w("walk_to(428)#64", "3600 frames", json!({"spent": 294, "cancelled": 2}), 1.1),
                   w("walk_to(428)#64", "3600 frames", json!({"spent": 294, "cancelled": 2}), 1.2)],
              json!([{"why": "empty window", "offer": {"idle": "sentence"}, "kept": 0, "step_seconds": 3.0}])),
        // the target reached, nothing new
        4 => {
            news.push((5, vec![("var:5", json!(7))], "end"));
            (json!("goal"), vec![w("walk_to(1)#2", "ready: the first end point in the order of tries", json!({"goal": 1}), 0.3)], Value::Null)
        }
        // a free try (component free's record), from n = 11 on: its entry new every other time
        5 if n >= 11 => {
            news.push((6, vec![("var:5", json!(7))], "free"));
            if n % 12 == 11 {
                news.push((8, vec![("var:9", json!(n + 1))], "free"));
            }
            (Value::Null, vec![], Value::Null)
        }
        // no attempt: every request stops on the reach extension
        _ => (Value::Null, vec![], Value::Null),
    };
    let free = k == 5 && n >= 11;
    if gain && k != 0 {
        news.insert(0, (1, vec![("var:9", json!(n + 1))], "wait"));
    }
    std::fs::create_dir_all(d.join("new")).unwrap();
    let mut new = Vec::new();
    let mut dead: Vec<Value> = Vec::new();
    for (i, (df, fs, how)) in news.iter().enumerate() {
        let mut f = base.clone();
        for (key, v) in fs {
            f[*key] = v.clone();
        }
        let fr = f0 + df;
        let doc = json!({"id": null, "parent": entry, "frame": fr, "tick": fr, "events": [{"frame": fr, "click": [2, 2]}],
                         "checkpoints": [cp(fr, f, fr)]});
        files::write_gz_default(&d.join("new").join(format!("{i}.json.gz")), &doc).unwrap();
        if *how == "end+" {
            new.push(json!({"file": format!("new/{i}.json.gz"), "frame": fr, "tick": fr, "how": "end", "novel": ["end"]}));
            dead.push(json!({"reason": "restart@1:2", "frame": fr, "events": [{"frame": fr, "click": [2, 2]}], "how": "afford"}));
        } else {
            new.push(json!({"file": format!("new/{i}.json.gz"), "frame": fr, "tick": fr, "how": how}));
        }
    }
    let (probes, attempt) = if free {
        (json!([]), Value::Null)
    } else if stop.is_null() {
        let s = "reach extension: the start at frame 1 holds 1 values the extended reading did not reach either: x";
        (json!([{"target": ["bit:1", 1], "status": null, "stop": s, "seconds": 1.3, "attempt": "a00"},
                {"target": ["bit:2", 1], "status": null, "stop": s, "seconds": 0.2, "attempt": "a01"}]), Value::Null)
    } else {
        std::fs::create_dir_all(d.join("run").join("a01")).unwrap();
        let run = json!({"windows": windows, "refinements": [{"seconds": 0.7, "added": [194]}], "extensions": [{"seconds": 0.3, "replan": true}]});
        std::fs::write(d.join("run").join("a01").join("run.json"), run.to_string()).unwrap();
        (json!([{"target": ["bit:1", 1], "status": "unsolvable", "stop": "no plan (the planner proved none)", "seconds": 1.0, "attempt": "a00"},
                {"target": ["bit:2", 1], "status": "plan", "stop": stop, "seconds": 9.0, "attempt": "a01"}]),
         json!({"target": ["bit:2", 1], "stop": stop, "reached": stop == "goal"}))
    };
    let mut e012 = json!({"t": {"seconds": 0.8}, "wait": {"end": if n % 2 == 0 { "static" } else { "horizon" }, "seconds": 0.1}});
    if !afford.is_null() {
        e012["afford"] = afford;
    }
    let res = json!({"job": job["job"], "entry": entry, "ok": true, "error": null,
                     "return": {"same": true, "differ": {}, "boot_s": 0.01},
                     "reading": {"fresh": n == 0, "why": if n == 0 { "no cached reading" } else { "differs on 2 keys" }, "wall_s": 4.0, "load_s": 0.05},
                     "targets": {"by_tier": {"0": 0, "1": 2, "2": 0}, "first": []}, "probes": probes, "attempt": attempt,
                     "readings_made": [], "new": new, "tried": [], "exhausted": false,
                     "tested": progress::atoms_json(&tested), "dead": dead, "e012": e012,
                     "cpu_s": {"total": 1.0}, "peak_rss_mb": {"python": 1.0, "children": 0.0}, "wall_s": 20.0 + n as f64});
    let mut res = res;
    if free {
        res["kind"] = json!("free");
        res["free"] = json!({"offers": 2, "n_points": 2, "trials": 30, "delays": {"counted": 2, "seconds": 0.6}, "seconds": 5.0,
                             "points": [{"where": "entry", "frame": f0, "level": 1, "seconds": 3.0, "record": {"verdict_s": {"ignored": 0.5, "taken": 2.0}}},
                                        {"where": "kept", "frame": f0 + 6, "level": 2, "seconds": 1.0, "record": {"ready": true}}]});
    }
    files::write_json(&d.join("result.json"), &res, Some(1)).unwrap();
    res
}

fn texts(s: &Stalls) -> String {
    serde_json::to_string(&s.json()).unwrap()
}

/// free's kind rule (free's amendment): job n is free when floor((n + 1)ρ + 1e-9) > floor(nρ + 1e-9), or by the cell
/// rule on the model's candidate's cell.
fn kind(n: usize, rho: f64, cell_free: bool) -> &'static str {
    let f = |x: f64| (x + 1e-9).floor();
    if f((n + 1) as f64 * rho) > f(n as f64 * rho) || cell_free {
        "free"
    } else {
        "model"
    }
}

#[test]
fn the_live_table_equals_the_table_from_the_files_after_every_commit() {
    let _g = env(false);
    let t = tmp("live");
    let mut a = make(&t, None);
    let mut live = Stalls::of_open(&a).unwrap();
    let (mut kinds_live, mut kinds_files) = (Vec::new(), Vec::new());
    let (port, port3) = (stall::ArchivePort::default(), stall::ArchivePort::default());
    for n in 0..30 {
        let e = a.select(1)[0];
        let d = a.prepare_job(e).unwrap();
        script(&d, n, false);
        let line = a.ingest(&d, None).unwrap();
        live.push(&line, &a).unwrap();
        live.set_cells(stall::cells_of_archive(&a));
        let files = Stalls::of_archive(&a.root).unwrap();
        assert_eq!(texts(&live), texts(&files), "after commit {n}");
        // every try's rows sum to its wall, and the novelty check holds
        let tr = live.tries.last().unwrap();
        let rows: f64 = tr.rows.iter().map(|x| x.2).sum();
        assert!((rows - tr.wall).abs() < 1e-9, "{n}: {rows} against {}", tr.wall);
        assert_eq!(tr.novelty, Some(true), "{n}");
        // the share and the cell rule on the next candidate, from the live table and from the files
        let next = a.select(1)[0];
        let cell = a.rows[next].cell.clone();
        assert_eq!(live.share(0.25), files.share(0.25));
        assert_eq!(live.free_cell(&cell), files.free_cell(&cell));
        assert_eq!(live.free_cells(), files.free_cells());
        kinds_live.push(kind(n + 1, stall::share(&live, 0.25), stall::free_cell(&live, &cell)));
        // free's port over the open archive gives the same
        for (p, ask) in [(&port, true), (&port3, n % 3 == 2)] {
            if ask {
                assert_eq!(p.share(&a, 0.25).unwrap(), live.share(0.25), "port, after commit {n}");
                assert_eq!(p.share(&a, 1.0).unwrap(), live.share(1.0));
                for c in a.rows.iter().map(|r| r.cell.clone()) {
                    assert_eq!(p.free_cell(&a, &c).unwrap(), live.free_cell(&c), "port, cell {c} after commit {n}");
                }
            }
        }
        // the archive opened again at this commit (a resume)
        let b = Archive::open(&a.root).unwrap();
        let reopened = Stalls::of_open(&b).unwrap();
        assert_eq!(texts(&reopened), texts(&live));
        let cell_b = b.rows[b.select(1)[0]].cell.clone();
        assert_eq!(cell_b, cell);
        kinds_files.push(kind(n + 1, stall::share(&reopened, 0.25), stall::free_cell(&reopened, &cell_b)));
        let pb = stall::ArchivePort::default();
        assert_eq!((pb.share(&b, 0.25).unwrap(), pb.free_cell(&b, &cell_b).unwrap()), (live.share(0.25), live.free_cell(&cell)));
    }
    assert_eq!(kinds_live, kinds_files);
    // the cycle's classes are all in the table
    let classes: std::collections::BTreeSet<Class> = live.tries.iter().map(|t| t.class).collect();
    for c in [Class::Productive, Class::ModelBlind, Class::Mismatch, Class::ModelNoYield] {
        assert!(classes.contains(&c), "{c:?} in {classes:?}");
    }
    let stops: Vec<&str> = live.tries.iter().take(6).map(|t| t.stop.as_str()).collect();
    assert_eq!(stops, ["goal", "refinement: Lemma R's R0", "the same empty window", "the same empty window", "goal", "none played: reach extension"]);
    assert_eq!(live.tries[2].stop_class, Class::Refused);
    assert_eq!(live.tries[3].stop_class, Class::Mismatch);
    assert_eq!(live.tries[5].stop_class, Class::ModelBlind);
    // try 0's new atom makes its model seconds productive; try 2's new room only its affordance step's
    let secs = |i: usize, c: Class| -> f64 { live.tries[i].rows.iter().filter(|x| x.1 == c).map(|x| x.2).sum() };
    assert!(live.tries[0].gave && secs(0, Class::Productive) > 10.0);
    assert!(live.tries[2].gave && (secs(2, Class::Productive) - 7.5).abs() < 1e-9 && secs(2, Class::Refused) > 10.0);
    assert!(!live.tries[4].gave && secs(4, Class::ModelNoYield) > 10.0);
    // the free tries: their kind, their rows by offer point, productive when a free entry was new
    let frees: Vec<&stall::TryStall> = live.tries.iter().filter(|t| t.kind == "free").collect();
    assert_eq!(frees.len(), 4);
    for t in &frees {
        let n: usize = t.job[1..].parse().unwrap();
        assert_eq!(t.stop, "free");
        assert_eq!(t.gave, n % 12 == 11, "{}", t.job);
        let row = |o: &str| t.rows.iter().find(|x| x.0.phase == "free" && x.0.outcome == o).map(|x| (x.1, x.2));
        let taken = if t.gave { Class::Productive } else { Class::InterfaceNoYield };
        assert_eq!(row("entry, level 1: ignored"), Some((Class::Refused, 0.5)));
        assert_eq!(row("entry, level 1: taken"), Some((taken, 2.0)));
        assert_eq!(row("kept, level 2"), Some((taken, 1.0)));
        assert_eq!(row("delays"), Some((taken, 0.6)));
        assert!(row("other").map(|x| (x.1 - 0.4).abs() < 1e-9).unwrap_or(false));
    }
    // the dialog step's end entry is the step's: not new, the step stays "interface, no yield"
    assert!(live.tries[1].rows.iter().any(|x| x.0.phase == "afford" && x.1 == Class::InterfaceNoYield));
    let (w, r) = live.totals();
    assert!((w - r).abs() < 1e-6);
}

/// The driver's step for the test: the scripted try of its job number.
struct Scripted;

impl Step for Scripted {
    fn run(&self, job_dir: &Path, _log: &JobLog, _cancel: &AtomicBool) -> i32 {
        let name = job_dir.file_name().unwrap().to_string_lossy().into_owned();
        script(job_dir, name[1..].parse().unwrap(), true);
        0
    }
}

fn drive(root: &Path) -> Value {
    std::env::set_var("E008_FLOOR_GB", "0.01");
    std::env::set_var("E008_EVICT_GB", "0.01");
    std::env::set_var("E008_MEM_SECOND_GB", "0");
    std::env::set_var("E008_POLL_S", "0.01");
    std::env::set_var("E008_DISK_PATH", root);
    std::env::set_var("E008_HEAVY_LOCK", root.join("heavy.lock"));
    let mut st = DriverStats::default();
    driver::run(root, 1, &StepMode::InProcess(Arc::new(Scripted)), None, &mut st).unwrap()
}

fn same_as_the_files(root: &Path) {
    let live = std::fs::read_to_string(root.join(stall::STALLS_JSON)).unwrap();
    let s = Stalls::of_archive(root).unwrap();
    let off = root.join("offline.json");
    files::write_json(&off, &s.json(), Some(1)).unwrap();
    assert_eq!(live, std::fs::read_to_string(&off).unwrap());
}

#[test]
fn the_drivers_stalls_json_is_the_files_table_and_survives_a_resume() {
    let _g = env(true);
    let t = tmp("driver");
    let a = make(&t, Some(json!({"stop_jobs": 8})));
    let root = a.root.clone();
    drop(a);
    let s = drive(&root);
    assert_eq!(s["stop"], json!("prediction bound"));
    assert_eq!(s["jobs"], json!(8));
    same_as_the_files(&root);
    let v = files::read_json(&root.join(stall::STALLS_JSON)).unwrap();
    assert_eq!(v["run"]["tries"], json!(8));
    // a resume: the bound raised, the summary gone; the driver opens the table from the files and goes on
    let mut stage = files::read_json(&root.join("stage.json")).unwrap();
    stage["e009"]["stop_jobs"] = json!(14);
    files::write_json(&root.join("stage.json"), &stage, Some(1)).unwrap();
    std::fs::remove_file(root.join("summary.json")).unwrap();
    let s = drive(&root);
    assert_eq!(s["jobs"], json!(14));
    same_as_the_files(&root);
    let v = files::read_json(&root.join(stall::STALLS_JSON)).unwrap();
    assert_eq!(v["run"]["tries"], json!(14));
    assert_eq!(v["trace"].as_array().unwrap().len(), 14);
}

#[test]
fn with_the_switch_unset_the_driver_writes_no_stalls_json() {
    let _g = env(false);
    let t = tmp("off");
    let a = make(&t, Some(json!({"stop_jobs": 3})));
    let root = a.root.clone();
    drop(a);
    drive(&root);
    assert!(!root.join(stall::STALLS_JSON).exists());
    // the report reads the files all the same
    let s = Stalls::of_archive(&root).unwrap();
    assert_eq!(s.tries.len(), 3);
}

/// E014 amendment (8), FMI_GOAL_FACT (row 23): the stop of an attempt whose refined task lost the target's fact is a
/// refinement's (model-blind), and the fact it names is the first the domains lack.
#[test]
fn a_refined_task_without_the_targets_fact() {
    use fmi_explore::harness::{goal_fact_stop, missing_goal_fact};
    use fmi_explore::stall::{stop_class, stop_kind, Class};
    let k = fmi_core::Key::parse("bit:20");
    let stop = goal_fact_stop(k);
    assert_eq!(stop, "refinement: the refined task holds no fact of the target: bit:20");
    assert_eq!(stop_kind(&stop), "refinement");
    assert_eq!(stop_class(&stop, false), Class::ModelBlind);
    let d = fmi_core::Domain { values: vec![0, 1], low: None, high: None };
    let doms = fmi_core::Domains(vec![(fmi_core::Key::parse("var:165"), d)]);
    let goal = vec![(fmi_core::Key::parse("var:165"), vec![1]), (k, vec![0])];
    assert_eq!(missing_goal_fact(&goal, &doms), Some(k));
    assert_eq!(missing_goal_fact(&goal[..1], &doms), None);
}

/// E014 amendment (8): FMI_STOP_WALL. At a wall limit of 0 s no try starts and the stop is "wall limit"; a limit far
/// off leaves the run to its other rules (here the falsifier's bound).
#[test]
fn the_wall_limit_stops_the_driver() {
    let _g = env(true);
    let t = tmp("wall");
    let root = make(&t, Some(json!({"stop_jobs": 3}))).root.clone();
    std::env::set_var("FMI_STOP_WALL", "0");
    let s = drive(&root);
    assert_eq!(s["stop"], json!("wall limit"));
    assert_eq!(s["jobs"], json!(0));
    assert_eq!(s["detail"]["stop_wall_s"], json!(0.0));
    // resumed with a limit far off: the bound stops it after its 3 tries, every one committed
    std::fs::remove_file(root.join("summary.json")).unwrap();
    std::env::set_var("FMI_STOP_WALL", "100000");
    let s = drive(&root);
    std::env::remove_var("FMI_STOP_WALL");
    assert_eq!(s["stop"], json!("prediction bound"));
    assert_eq!(s["jobs"], json!(3));
}
