//! E013 component `free` (experiments/E013-robust/plan.md, section D and free's amendment): the job kind by the share
//! and by stall's cell rule (through the `Shift` port), the free counts apart from the model's, the free order, the
//! counts counted again when the archive is opened again, and E012's files with FMI_FREE unset (D2's unit part).

use fmi_explore::archive::Archive;
use fmi_explore::files;
use fmi_explore::free::{self, Kind, Shift};
use fmi_explore::progress::{self, TAtom};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};

static N: AtomicUsize = AtomicUsize::new(0);
/// The switches are the process's environment: one test at a time.
static ENV: Mutex<()> = Mutex::new(());

fn env(select: bool, rho: Option<&str>) -> MutexGuard<'static, ()> {
    let g = ENV.lock().unwrap_or_else(|e| e.into_inner());
    if select {
        std::env::set_var(progress::SELECT_ENV, progress::PROGRESS);
    } else {
        std::env::remove_var(progress::SELECT_ENV);
    }
    match rho {
        Some(r) => std::env::set_var(free::SWITCH, r),
        None => std::env::remove_var(free::SWITCH),
    }
    std::env::remove_var(free::SHIFT_SWITCH);
    g
}

fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("free-{}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst), name));
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

fn root_facts() -> Value {
    json!({"room": 33, "ego": 1, "owner:500": 1, "var:5": 7})
}

fn cp(frame: i64, facts: Value, tick: i64) -> Value {
    json!([frame, facts, {"tick": tick, "rng": 1, "actors": []}])
}

fn make(t: &Path) -> Archive {
    let gp = t.join("GUIDE.json");
    std::fs::write(&gp, guide().to_string()).unwrap();
    let mut a = Archive::create(&t.join("arch"), "test", &json!({"owner:566": [1]}), &gp, &[28, 33, 35, 38, 41], 2,
                                Some(json!({"frame": 100, "tick": 0, "qf": 1000, "room": 33})), None, None).unwrap();
    a.add(None, 100, Some(0), &[], &[cp(100, root_facts(), 0)], None, "boot").unwrap();
    a.write_cells().unwrap();
    a
}

fn over(base: &Value, facts: &[(&str, Value)]) -> Value {
    let mut b = base.clone();
    for (k, v) in facts {
        b[*k] = v.clone();
    }
    b
}

/// A job dir for `entry` of the given kind, with its result: new entries `news` (frames after the entry, facts over
/// the entry's) and the try's T.
fn job(a: &Archive, entry: usize, kind: Option<&Kind>, news: &[(i64, Vec<(&str, Value)>)], tested: &[TAtom]) -> PathBuf {
    let d = a.prepare_job_as(entry, kind).unwrap();
    let jb = files::read_json(&d.join("job.json")).unwrap();
    let f0 = a.rows[entry].frame;
    let base = a.info(entry).unwrap().last_cp()[1].clone();
    let mut new = Vec::new();
    for (k, (df, facts)) in news.iter().enumerate() {
        let f = over(&base, facts);
        let tick = f0 + df;
        let doc = json!({"id": null, "parent": entry, "frame": f0 + df, "tick": tick,
                         "events": [{"frame": f0 + df, "click": [2, 2]}], "checkpoints": [cp(f0 + df, f, tick)]});
        files::write_gz_default(&d.join("new").join(format!("{k}.json.gz")), &doc).unwrap();
        new.push(json!({"file": format!("new/{k}.json.gz"), "frame": f0 + df, "tick": tick,
                        "how": if kind.map(|k| k.free).unwrap_or(false) { "free" } else { "checkpoint" }}));
    }
    let mut res = json!({"job": jb["job"], "entry": entry, "ok": true, "error": null,
                         "return": {"same": true, "differ": {}, "boot_s": 0.0}, "reading": null,
                         "targets": {"by_tier": {"0": 0, "1": 0, "2": 0}, "first": []}, "probes": [], "attempt": null,
                         "readings_made": [], "new": new, "tried": [], "exhausted": false,
                         "tested": progress::atoms_json(tested), "dead": [],
                         "cpu_s": {"total": 1.0}, "peak_rss_mb": {"python": 1.0, "children": 0.0}, "wall_s": 0.1});
    if let Some(k) = kind {
        res["kind"] = json!(k.text());
    }
    std::fs::write(d.join("result.json"), res.to_string()).unwrap();
    d
}

fn ingest(a: &mut Archive, entry: usize, kind: Option<&Kind>, news: &[(i64, Vec<(&str, Value)>)], tested: &[TAtom]) -> Value {
    let d = job(a, entry, kind, news, tested);
    a.ingest(&d, None).unwrap()
}

fn ids(line: &Value) -> Vec<usize> {
    line["new_entries"].as_array().unwrap().iter().map(|x| x.as_u64().unwrap() as usize).collect()
}

fn fk(rho: f64) -> Kind {
    Kind { free: true, why: "share", rho }
}

fn mk(rho: f64) -> Kind {
    Kind { free: false, why: "share", rho }
}

#[test]
fn the_driver_picks_the_kind_by_the_share() {
    let _g = env(true, Some("0.25"));
    let t = tmp("share");
    let mut a = make(&t);
    let mut kinds = Vec::new();
    for _ in 0..8 {
        let (i, k) = free::pick(&a).unwrap().unwrap();
        let k = k.unwrap();
        kinds.push(k.text());
        let n = a.rows.len() as i64;
        ingest(&mut a, i, Some(&k), &[(n + 1, vec![("var:5", json!(n))])], &[]);
    }
    assert_eq!(kinds, vec!["model", "model", "model", "free", "model", "model", "model", "free"]);
    // the job files say so
    let jb = files::read_json(&a.root.join("jobs/j00003/job.json")).unwrap();
    assert_eq!(jb["kind"], json!("free"));
    let sel = files::read_json(&a.root.join("jobs/j00003/select.json")).unwrap();
    assert_eq!((sel["kind"].clone(), sel["why"].clone(), sel["rho"].clone()), (json!("free"), json!("share"), json!(0.25)));
    let line = a.logs.iter().find(|x| x["job"] == "j00003").unwrap();
    assert_eq!(line["kind"], json!("free"));
    // every job free with ρ = 1
    std::env::set_var(free::SWITCH, "1");
    for _ in 0..3 {
        let (i, k) = free::pick(&a).unwrap().unwrap();
        assert!(k.as_ref().unwrap().free);
        ingest(&mut a, i, k.as_ref(), &[], &[]);
    }
}

#[test]
fn free_counts_are_apart_from_the_model_counts() {
    let _g = env(true, Some("0.25"));
    let t = tmp("counts");
    let mut a = make(&t);
    let c0 = a.rows[0].cell.clone();
    assert_eq!(a.cell(&c0).free, Some((0, 0)));
    // a model try with nothing new: the model's n and chosen count, the free counts do not
    ingest(&mut a, 0, Some(&mk(0.25)), &[], &[]);
    assert_eq!((a.cell(&c0).n, a.cell(&c0).chosen, a.cell(&c0).free), (1, Some(1), Some((0, 0))));
    // a free try with nothing new: the other way round
    let l = ingest(&mut a, 0, Some(&fk(0.25)), &[], &[]);
    assert_eq!((a.cell(&c0).n, a.cell(&c0).chosen, a.cell(&c0).free), (1, Some(1), Some((1, 1))));
    assert_eq!((l["kind"].clone(), l["free"].clone(), l["n"].clone(), l["chosen"].clone()), (json!("free"), json!([1, 1]), json!(1), json!(1)));
    // a free try that gives a new room: its n goes back to 0, the model's stays
    let l = ingest(&mut a, 0, Some(&fk(0.25)), &[(5, vec![("room", json!(35))])], &[]);
    assert_eq!(l["new_rooms"], json!([35]));
    assert_eq!((a.cell(&c0).n, a.cell(&c0).chosen, a.cell(&c0).free), (1, Some(1), Some((0, 2))));
    // a free try never marks its entry exhausted (its result says false; the cell keeps none)
    assert!(a.cell(&c0).exhausted.is_empty());
    // cells.json carries [free_n, free_chosen]
    a.write_cells().unwrap();
    let cj = files::read_json(&a.root.join("cells.json")).unwrap();
    assert_eq!(cj[&c0]["free"], json!([0, 2]));
    assert_eq!(cj[&c0]["chosen"], json!(1));
    // opened again: the same counts from the log
    let cells = a.cells_json();
    let b = Archive::open(&a.root).unwrap();
    assert_eq!(b.cells_json(), cells);
    assert!(b.free_on);
    // also with FMI_FREE unset (a resume without the variable): the log's kinds keep the free counts
    std::env::remove_var(free::SWITCH);
    let b = Archive::open(&a.root).unwrap();
    assert!(b.free_on);
    assert_eq!(b.cells_json(), cells);
}

#[test]
fn the_free_order_reads_the_free_counts() {
    let _g = env(true, Some("0.25"));
    let t = tmp("order");
    let mut a = make(&t);
    let l = ingest(&mut a, 0, Some(&mk(0.25)), &[(5, vec![("room", json!(35))]), (6, vec![("room", json!(38))])], &[]);
    let (p, q) = (ids(&l)[0], ids(&l)[1]);
    let (cp_, cq) = (a.rows[p].cell.clone(), a.rows[q].cell.clone());
    let c0 = a.rows[0].cell.clone();
    // p's cell worn out by the model, q's by free tries
    a.cell_mut(&cp_).n = 4;
    a.cell_mut(&cp_).chosen = Some(4);
    a.cell_mut(&cq).free = Some((3, 3));
    a.cell_mut(&c0).n = 9;
    a.cell_mut(&c0).free = Some((9, 9));
    assert_eq!(a.select(3), vec![q, p, 0]);
    assert_eq!(a.select_free(3), vec![p, q, 0]);
    // free_chosen second, as chosen is in the model's order
    a.cell_mut(&cq).free = Some((0, 3));
    a.cell_mut(&cp_).free = Some((0, 1));
    assert_eq!(a.select_free(2), vec![p, q]);
    a.cell_mut(&cp_).free = Some((0, 5));
    assert_eq!(a.select_free(2), vec![q, p]);
    // the free job's key in select.json is the free order's
    let d = a.prepare_job_as(q, Some(&fk(0.25))).unwrap();
    let sel = files::read_json(&d.join("select.json")).unwrap();
    assert_eq!(sel["key"][0], json!(0));
    assert_eq!(sel["key"][1], json!(3));
    std::fs::remove_dir_all(&d).unwrap();
    // busy cells are skipped as in the model's selection
    a.busy.insert("jX".into(), (q, cq.clone()));
    assert_eq!(a.select_free(2), vec![p, 0]);
}

/// E016 amendment (11): with FMI_SELECT_PLACE=1 a place's own tries order it before its cells: the root's place, tried
/// 4 times with nothing new in the last 3, comes after two places no try has started from, although its cell's own
/// counts come first.
#[test]
fn places_tried_without_yield_come_after_untried_places() {
    let _g = env(true, Some("0.25"));
    std::env::remove_var("FMI_SELECT_PLACE");
    let t = tmp("place");
    let mut a = make(&t);
    let l = ingest(&mut a, 0, Some(&mk(0.25)), &[(5, vec![("room", json!(35))]), (6, vec![("room", json!(38))])], &[]);
    let (p, q) = (ids(&l)[0], ids(&l)[1]);
    for _ in 0..3 {
        ingest(&mut a, 0, Some(&mk(0.25)), &[], &[]);
    }
    let c0 = a.rows[0].cell.clone();
    let (cp_, cq) = (a.rows[p].cell.clone(), a.rows[q].cell.clone());
    a.cell_mut(&c0).n = 0;
    a.cell_mut(&c0).chosen = Some(0);
    for c in [&cp_, &cq] {
        a.cell_mut(c).n = 5;
        a.cell_mut(c).chosen = Some(5);
    }
    assert_eq!(a.place_key(0, false), (0, 0));
    assert_eq!(a.select(1), vec![0]);
    std::env::set_var("FMI_SELECT_PLACE", "1");
    assert_eq!(a.place_key(0, false), (3, 4));
    assert_eq!(a.place_key(p, false), (0, 0));
    assert_eq!(a.place_key(0, true), (0, 0));
    let s = a.select(3);
    assert_eq!(s.len(), 3);
    assert_eq!(s[2], 0);
    // no free try yet: every place's free counts are (0, 0), so the free order is the cells' as before
    let free_on = a.select_free(3);
    std::env::remove_var("FMI_SELECT_PLACE");
    assert_eq!(a.select_free(3), free_on);
}

/// Stall's rules as a test gives them: a share, and the cells whose next try is free.
struct TestShift {
    share: f64,
    cells: Vec<String>,
}

impl Shift for TestShift {
    fn share(&self, _a: &Archive, _rho0: f64) -> Result<f64, String> {
        Ok(self.share)
    }
    fn free_cell(&self, _a: &Archive, cell: &str) -> Result<bool, String> {
        Ok(self.cells.iter().any(|c| c == cell))
    }
}

#[test]
fn the_shift_port_gives_the_share_and_the_cell_rule() {
    let _g = env(true, Some("0.25"));
    let t = tmp("shift");
    let mut a = make(&t);
    // the share doubled: jobs 1, 3, 5, ... free
    let sh = TestShift { share: 0.5, cells: vec![] };
    let mut kinds = Vec::new();
    for _ in 0..4 {
        let (i, k) = free::pick_with(&a, Some(0.25), Some(&sh)).unwrap().unwrap();
        let k = k.unwrap();
        assert_eq!(k.rho, 0.5);
        kinds.push(k.text());
        ingest(&mut a, i, Some(&k), &[], &[]);
    }
    assert_eq!(kinds, vec!["model", "free", "model", "free"]);
    // the cell rule: a model job of a named cell is free, from the model's candidate, with why "cell"
    let model = free::pick_with(&a, Some(0.25), None).unwrap().unwrap();
    assert!(!model.1.as_ref().unwrap().free);
    let sh = TestShift { share: 0.25, cells: vec![a.rows[model.0].cell.clone()] };
    let (i, k) = free::pick_with(&a, Some(0.25), Some(&sh)).unwrap().unwrap();
    assert_eq!((i, k.unwrap()), (model.0, Kind { free: true, why: "cell", rho: 0.25 }));
    // without the switch: E012's selection, no kind
    assert_eq!(free::pick_with(&a, None, Some(&sh)).unwrap(), Some((a.select(1)[0], None)));
}

#[test]
fn the_switch_is_checked_before_any_job() {
    let _g = env(true, Some("0.25"));
    let t = tmp("check");
    let a = make(&t);
    assert!(free::check_env(&a).is_ok());
    std::env::set_var(free::SWITCH, "abc");
    assert!(free::check_env(&a).unwrap_err().contains("not a share"));
    std::env::set_var(free::SWITCH, "0");
    assert!(free::check_env(&a).is_err());
    std::env::set_var(free::SWITCH, "1.5");
    assert!(free::check_env(&a).is_err());
    std::env::set_var(free::SWITCH, "1");
    std::env::set_var(free::SHIFT_SWITCH, "1");
    assert!(free::check_env(&a).unwrap_err().contains("stall's rules"));
    std::env::remove_var(free::SHIFT_SWITCH);
    std::env::remove_var(free::SWITCH);
    assert!(free::check_env(&a).is_ok());
    drop(_g);
    // FMI_FREE without the progress order
    let _g = env(false, Some("0.25"));
    let t = tmp("check-off");
    let a = make(&t);
    assert!(free::check_env(&a).unwrap_err().contains("progress order"));
}

#[test]
fn off_the_files_are_e012s() {
    let _g = env(true, None);
    let t = tmp("off");
    let mut a = make(&t);
    assert!(!a.free_on);
    let c0 = a.rows[0].cell.clone();
    assert_eq!(a.cell(&c0).free, None);
    assert_eq!(free::pick(&a).unwrap(), Some((0, None)));
    let l = ingest(&mut a, 0, None, &[(5, vec![("room", json!(35))])], &[]);
    assert!(l.get("kind").is_none() && l.get("free").is_none());
    let jb = files::read_json(&a.root.join("jobs/j00000/job.json")).unwrap();
    let sel = files::read_json(&a.root.join("jobs/j00000/select.json")).unwrap();
    assert!(jb.get("kind").is_none());
    for k in ["kind", "why", "rho"] {
        assert!(sel.get(k).is_none(), "{k}");
    }
    assert_eq!(sel.as_object().unwrap().keys().cloned().collect::<Vec<_>>(), vec!["key", "cell", "selectable", "time"]);
    a.write_cells().unwrap();
    let cj = files::read_json(&a.root.join("cells.json")).unwrap();
    assert!(cj.as_object().unwrap().values().all(|c| c.get("free").is_none()));
    // prepare_job_as with no kind is prepare_job: the same job.json (but its name)
    let d1 = a.prepare_job_as(0, None).unwrap();
    let j1 = files::read_json(&d1.join("job.json")).unwrap();
    std::fs::remove_dir_all(&d1).unwrap();
    let d2 = a.prepare_job(0).unwrap();
    let j2 = files::read_json(&d2.join("job.json")).unwrap();
    assert_eq!(j1, j2);
}

#[test]
fn e016_d_a_free_jobs_file_lists_the_registered_readings_that_may_stand_in() {
    // E016 D (FMI_FREE_T=archive): a free job's job.json lists the registered readings of other scopes holding the
    // entry's room ("readings_room") and the newest of any scope ("readings_any"); a model job's does not, nor does a
    // free job's without the switch
    let _g = env(true, Some("0.25"));
    let t = tmp("free-t");
    let mut a = make(&t);
    let mut paths = Vec::new();
    for (k, scope) in [vec![28, 33], vec![35], vec![41]].iter().enumerate() {
        let d = a.root.join("readings").join(format!("r{k}"));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("reading.json"), "{}").unwrap();
        let p = d.join("reading.json").to_string_lossy().into_owned();
        a.register_reading(scope, &p, 0, None, "fresh", Some(0)).unwrap();
        paths.push(p);
    }
    let path_of = |v: &Value| v.as_array().unwrap().iter().map(|x| x["path"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    std::env::set_var("FMI_FREE_T", "archive");
    let d = a.prepare_job_as(0, Some(&fk(0.25))).unwrap();
    let jb = files::read_json(&d.join("job.json")).unwrap();
    std::fs::remove_dir_all(&d).unwrap();
    // newest first; the root's room is 33, held by the first reading's scope only
    assert_eq!(path_of(&jb["readings_any"]), vec![paths[2].clone(), paths[1].clone(), paths[0].clone()]);
    assert_eq!(path_of(&jb["readings_room"]), vec![paths[0].clone()]);
    let d = a.prepare_job_as(0, Some(&mk(0.25))).unwrap();
    let jm = files::read_json(&d.join("job.json")).unwrap();
    std::fs::remove_dir_all(&d).unwrap();
    std::env::remove_var("FMI_FREE_T");
    assert!(jm.get("readings_any").is_none() && jm.get("readings_room").is_none());
    let d = a.prepare_job_as(0, Some(&fk(0.25))).unwrap();
    let jo = files::read_json(&d.join("job.json")).unwrap();
    assert!(jo.get("readings_any").is_none() && jo.get("readings_room").is_none());
}
