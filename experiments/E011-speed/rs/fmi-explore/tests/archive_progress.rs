//! E012 gate S1 (experiments/E012-game-mechanics/plan.md, the amendment "component `archive`"): the archive under
//! FMI_SELECT=progress. T and its cells, novelty and `new_t`, each key of the order alone, the candidate within a cell,
//! the dead and exhausted marks, the counts counted again when the archive is opened again or T grows, stagnation,
//! job.json's novelty, and the order against a brute force over the rows and the log.

use fmi_explore::archive::{fact_map, Archive, FactMap};
use fmi_explore::files;
use fmi_explore::progress::{self, Novel, TAtom};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};

static N: AtomicUsize = AtomicUsize::new(0);
/// The switch is the process's environment: one test at a time.
static ENV: Mutex<()> = Mutex::new(());

fn switch(on: bool) -> MutexGuard<'static, ()> {
    let g = ENV.lock().unwrap_or_else(|e| e.into_inner());
    if on {
        std::env::set_var(progress::SELECT_ENV, progress::PROGRESS);
    } else {
        std::env::remove_var(progress::SELECT_ENV);
    }
    g
}

fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("progress-{}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst), name));
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
    json!({"room": 33, "ego": 1, "owner:500": 1, "var:5": 7, "at:script:1:10": 1})
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

/// A new entry of a job: frames after the entry, facts over the entry's, and its "dead" mark.
struct New<'a> {
    df: i64,
    facts: Vec<(&'a str, Value)>,
    dead: Option<&'a str>,
}

fn nw<'a>(df: i64, facts: &[(&'a str, Value)]) -> New<'a> {
    New { df, facts: facts.to_vec(), dead: None }
}

/// A job dir for `entry` (prepare_job) with its result: the new entries, the try's T, exhausted, the dead moves.
fn job(a: &Archive, entry: usize, news: &[New], tested: &[TAtom], exhausted: bool, dead: Value) -> PathBuf {
    let d = a.prepare_job(entry).unwrap();
    let jb = files::read_json(&d.join("job.json")).unwrap();
    let f0 = a.rows[entry].frame;
    let base = a.info(entry).unwrap().last_cp()[1].clone();
    let mut new = Vec::new();
    for (k, n) in news.iter().enumerate() {
        let f = over(&base, &n.facts);
        let tick = f0 + n.df;
        let doc = json!({"id": null, "parent": entry, "frame": f0 + n.df, "tick": tick,
                         "events": [{"frame": f0 + n.df, "click": [2, 2]}], "checkpoints": [cp(f0 + n.df, f, tick)]});
        files::write_gz_default(&d.join("new").join(format!("{k}.json.gz")), &doc).unwrap();
        let mut item = json!({"file": format!("new/{k}.json.gz"), "frame": f0 + n.df, "tick": tick, "how": "checkpoint"});
        if let Some(r) = n.dead {
            item["dead"] = json!(r);
        }
        new.push(item);
    }
    let res = json!({"job": jb["job"], "entry": entry, "ok": true, "error": null,
                     "return": {"same": true, "differ": {}, "boot_s": 0.0}, "reading": null,
                     "targets": {"by_tier": {"0": 0, "1": 1, "2": 0}, "first": []}, "probes": [], "attempt": null,
                     "readings_made": [], "new": new, "tried": [], "exhausted": exhausted,
                     "tested": progress::atoms_json(tested), "dead": dead,
                     "cpu_s": {"total": 1.0}, "peak_rss_mb": {"python": 1.0, "children": 0.0}, "wall_s": 0.1});
    std::fs::write(d.join("result.json"), res.to_string()).unwrap();
    d
}

fn ingest(a: &mut Archive, entry: usize, news: &[New], tested: &[TAtom]) -> Value {
    let d = job(a, entry, news, tested, false, json!([]));
    a.ingest(&d, None).unwrap()
}

fn ids(line: &Value) -> Vec<usize> {
    line["new_entries"].as_array().unwrap().iter().map(|x| x.as_u64().unwrap() as usize).collect()
}

fn v(key: &str, x: i64) -> TAtom {
    TAtom::value(key, x)
}

fn facts_of(a: &Archive, i: usize) -> FactMap {
    fact_map(&a.info(i).unwrap().last_cp()[1])
}

#[test]
fn stage_json_keeps_the_switch_and_t_starts_with_the_goal() {
    let _g = switch(true);
    let t = tmp("stage");
    let a = make(&t);
    assert_eq!(files::read_json(&a.root.join("stage.json")).unwrap()["e012"], json!({"select": "progress"}));
    let p = a.progress.as_ref().unwrap();
    assert_eq!(p.t, vec![v("owner:566", 1)]);
    assert_eq!(a.cells_json()[&a.rows[0].cell]["chosen"], json!(0));
    drop(_g);
    // a resume without the variable keeps the mode the archive was made with
    let _g = switch(false);
    let b = Archive::open(&a.root).unwrap();
    assert!(b.progress.is_some());
}

#[test]
fn cells_are_room_owned_and_the_held_atoms_of_t_less_the_control_keys() {
    let _g = switch(true);
    let t = tmp("cells");
    let mut a = make(&t);
    let k0 = a.cell_key(&fact_map(&root_facts()));
    assert_eq!(k0, json!([33, [500], []]));
    let tested = [v("var:103", 1), v("local:211", 1), v("state:316", 1), v("at:script:1:10", 1), v("var:5", 7)];
    let line = ingest(&mut a, 0, &[nw(5, &[("var:103", json!(1)), ("local:211", json!(1))])], &tested);
    assert_eq!(line["t_new"], progress::atoms_json(&tested));
    assert_eq!(line["t_size"], json!(6));
    // the root holds var:5 = 7 and at:script:1:10 = 1 (a control key, out of the cell)
    assert_eq!(a.cell_key(&facts_of(&a, 0)), json!([33, [500], ["var:5=7"]]));
    let e1 = ids(&line)[0];
    assert_eq!(a.cell_key(&facts_of(&a, e1)), json!([33, [500], ["var:103=1", "var:5=7"]]));
    // the atoms a cell keys on do not see the control keys: local:211 alone makes no new cell
    let line = ingest(&mut a, 0, &[nw(6, &[("local:211", json!(1))])], &[]);
    assert_eq!(a.rows[ids(&line)[0]].cell, a.rows[0].cell);
    // owned objects: owner:O == ego
    assert_eq!(a.cell_key(&fact_map(&over(&root_facts(), &[("owner:566", json!(1))])))[1], json!([500, 566]));
    // E009's idle set is not in these cells
    a.idle.insert("var:103".into());
    assert_eq!(a.cell_key(&facts_of(&a, e1))[2], json!(["var:103=1", "var:5=7"]));
}

#[test]
fn novelty_new_t_and_new_rooms_and_the_counts() {
    let _g = switch(true);
    let t = tmp("novelty");
    let mut a = make(&t);
    let tested = [v("state:316", 1), v("bit:7", 1)];
    let c0 = a.rows[0].cell.clone();
    let l1 = ingest(&mut a, 0, &[nw(5, &[("state:316", json!(1))])], &tested);
    assert_eq!((l1["new_t"].clone(), l1["new_rooms"].clone()), (json!(["state:316=1"]), json!([])));
    assert_eq!((a.cell(&c0).n, a.cell(&c0).chosen, l1["n"].clone(), l1["chosen"].clone()), (0, Some(1), json!(0), json!(1)));
    // the same atom again: nothing new; n counts up
    let l2 = ingest(&mut a, 0, &[nw(6, &[("state:316", json!(1)), ("var:5", json!(1))])], &[]);
    assert_eq!((l2["new_t"].clone(), l2["new_rooms"].clone(), l2["t_new"].clone()), (json!([]), json!([]), json!([])));
    assert_eq!((a.cell(&c0).n, a.cell(&c0).chosen), (1, Some(2)));
    // a new room alone resets n
    let l3 = ingest(&mut a, 0, &[nw(7, &[("room", json!(35))])], &[]);
    assert_eq!((l3["new_t"].clone(), l3["new_rooms"].clone()), (json!([]), json!([35])));
    assert_eq!((a.cell(&c0).n, a.cell(&c0).chosen), (0, Some(3)));
    // a "return differs" counts in neither
    let d = job(&a, 0, &[], &[], false, json!([]));
    let mut r = files::read_json(&d.join("result.json")).unwrap();
    r["return"]["same"] = json!(false);
    std::fs::write(d.join("result.json"), r.to_string()).unwrap();
    a.ingest(&d, None).unwrap();
    assert_eq!((a.cell(&c0).n, a.cell(&c0).chosen), (0, Some(3)));
    // novelty against the live entries, as a try gets it from job.json
    let jd = a.prepare_job(0).unwrap();
    let jb = files::read_json(&jd.join("job.json")).unwrap();
    let nv = Novel::of_job(&jb, None).unwrap();
    assert_eq!(jb["novelty"]["rooms"], json!([33, 35]));
    for i in a.live_rows() {
        assert!(nv.novel(&facts_of(&a, i)).is_empty(), "entry {i} holds only what the archive holds");
    }
    let f = fact_map(&over(&root_facts(), &[("bit:7", json!(1)), ("room", json!(41))]));
    assert_eq!(nv.novel(&f), vec!["bit:7=1", "room=41"]);
    assert!(nv.is_novel(&f) && !nv.is_novel(&facts_of(&a, 0)));
    // the try's own T in place of the archive's
    let mine = Novel::of_job(&jb, Some(vec![v("var:5", 9)])).unwrap();
    assert_eq!(mine.novel(&fact_map(&over(&root_facts(), &[("var:5", json!(9))]))), vec!["var:5=9"]);
    std::fs::remove_dir_all(&jd).unwrap();
}

/// Four cells each with one entry, made by hand: a's cell chosen once with n 1, b's chosen twice with n 0, ...
#[test]
fn the_order_n_first() {
    let _g = switch(true);
    let t = tmp("n");
    let mut a = make(&t);
    // root's cell: chosen 3 times (n 0 after the third gave a room); x's cell: never chosen
    let l = ingest(&mut a, 0, &[nw(5, &[("room", json!(35))])], &[]);
    let x = ids(&l)[0];
    ingest(&mut a, 0, &[nw(6, &[("var:5", json!(3))])], &[]);
    let c0 = a.rows[0].cell.clone();
    assert_eq!((a.cell(&c0).n, a.cell(&c0).chosen), (1, Some(2)));
    let cx = a.rows[x].cell.clone();
    assert_eq!((a.cell(&cx).n, a.cell(&cx).chosen), (0, Some(0)));
    assert_eq!(a.select(10), vec![x, 0]);
    // n before chosen: x's cell chosen 5 times with n 0 still comes before the root's (n 1)
    a.cell_mut(&cx).chosen = Some(5);
    assert_eq!(a.select(10), vec![x, 0]);
    a.cell_mut(&cx).n = 2;
    assert_eq!(a.select(10), vec![0, x]);
}

#[test]
fn the_order_chosen_second() {
    let _g = switch(true);
    let t = tmp("chosen");
    let mut a = make(&t);
    let l = ingest(&mut a, 0, &[nw(5, &[("room", json!(35))]), nw(6, &[("room", json!(38))])], &[]);
    let (p, q) = (ids(&l)[0], ids(&l)[1]);
    let (cp_, cq) = (a.rows[p].cell.clone(), a.rows[q].cell.clone());
    a.cell_mut(&cp_).chosen = Some(2);
    a.cell_mut(&cq).chosen = Some(1);
    let c0 = a.rows[0].cell.clone();
    a.cell_mut(&c0).n = 9;
    assert_eq!(a.select(2), vec![q, p]);
    a.cell_mut(&cq).chosen = Some(3);
    assert_eq!(a.select(2), vec![p, q]);
}

#[test]
fn the_order_rooms_on_the_chain_third() {
    let _g = switch(true);
    let t = tmp("rooms");
    let mut a = make(&t);
    let l = ingest(&mut a, 0, &[nw(5, &[("room", json!(35))])], &[]);
    let h = ids(&l)[0];
    // p: root 33 -> 35 -> 38 (3 rooms); q: root 33 -> 28 (2 rooms), earlier frame
    let l = ingest(&mut a, h, &[nw(5, &[("room", json!(38))])], &[]);
    let p = ids(&l)[0];
    let l = ingest(&mut a, 0, &[nw(2, &[("room", json!(28))])], &[]);
    let q = ids(&l)[0];
    for c in [a.rows[0].cell.clone(), a.rows[h].cell.clone()] {
        a.cell_mut(&c).n = 9;
    }
    assert_eq!((a.chain_rooms(p).len(), a.chain_rooms(q).len()), (3, 2));
    assert!(a.rows[q].frame < a.rows[p].frame);
    assert_eq!(a.select(2), vec![p, q]);
}

#[test]
fn the_order_novel_atoms_fourth_then_frame_then_id() {
    let _g = switch(true);
    let t = tmp("novel");
    let mut a = make(&t);
    let tested = [v("state:316", 1), v("bit:7", 1)];
    // p holds two atoms first (frame 106), q none (frame 103), r one (frame 104): all of one room's chain length
    let l = ingest(&mut a, 0, &[nw(6, &[("room", json!(35)), ("state:316", json!(1)), ("bit:7", json!(1))]),
                               nw(3, &[("room", json!(38))]), nw(4, &[("room", json!(28)), ("bit:7", json!(1))])], &tested);
    let (p, q, r) = (ids(&l)[0], ids(&l)[1], ids(&l)[2]);
    let c0 = a.rows[0].cell.clone();
    a.cell_mut(&c0).n = 9;
    let nov = a.novel_counts();
    // bit:7=1's least holder by (frame, id) is r (frame 104), not p (106)
    assert_eq!((nov.get(&p).copied(), nov.get(&q).copied(), nov.get(&r).copied()), (Some(1), None, Some(1)));
    assert_eq!(a.select(3), vec![r, p, q]);
    assert_eq!(a.sel_key_json(r), json!([0, 0, -2, -1, 104, r]));
    // frame, then id: two entries alike but for the frame
    let t2 = tmp("frame");
    let mut b = make(&t2);
    let l = ingest(&mut b, 0, &[nw(6, &[("room", json!(35))]), nw(3, &[("room", json!(38))])], &[]);
    let c0 = b.rows[0].cell.clone();
    b.cell_mut(&c0).n = 9;
    assert_eq!(b.select(2), vec![ids(&l)[1], ids(&l)[0]]);
    let t3 = tmp("id");
    let mut c = make(&t3);
    let l = ingest(&mut c, 0, &[nw(3, &[("room", json!(35))]), nw(3, &[("room", json!(38))])], &[]);
    let c0 = c.rows[0].cell.clone();
    c.cell_mut(&c0).n = 9;
    assert_eq!(c.select(2), ids(&l));
}

#[test]
fn the_candidate_within_a_cell_and_the_dead_and_exhausted_marks() {
    let _g = switch(true);
    let t = tmp("cand");
    let mut a = make(&t);
    // three entries of one cell (room 35, nothing of T held): x at frame 107, y at 105 (dead), z at 106
    let l = ingest(&mut a, 0, &[nw(7, &[("room", json!(35))]),
                               New { df: 5, facts: vec![("room", json!(35)), ("var:5", json!(1))], dead: Some("quit") },
                               nw(6, &[("room", json!(35)), ("var:5", json!(2))])], &[]);
    let (x, y, z) = (ids(&l)[0], ids(&l)[1], ids(&l)[2]);
    let c = a.rows[x].cell.clone();
    assert!(a.rows[y].cell == c && a.rows[z].cell == c);
    assert_eq!((l["dead_entries"].clone(), a.rows[y].dead.clone()), (json!([y]), Some("quit".to_string())));
    assert_eq!(a.candidate(&c), Some(z));
    assert!(!a.select(10).contains(&y));
    // a dead entry holds nothing for novelty: its room alone would not be new
    let l = ingest(&mut a, 0, &[New { df: 8, facts: vec![("room", json!(41))], dead: Some("restart") }], &[]);
    assert_eq!(l["new_rooms"], json!([]));
    assert!(!a.progress.as_ref().unwrap().rooms.contains_key(&41));
    // z exhausted: x is the candidate; the mark keeps |T|
    let d = job(&a, z, &[], &[], true, json!([{"reason": "quit", "frame": 120, "events": [{"frame": 110, "key": "x"}]}]));
    let lz = a.ingest(&d, None).unwrap();
    assert_eq!((lz["exhausted"].clone(), lz["t_size"].clone()), (json!(true), json!(1)));
    assert_eq!(a.candidate(&c), Some(x));
    // the dead moves come back to the try from that entry
    let jb = files::read_json(&a.prepare_job(z).unwrap().join("job.json")).unwrap();
    assert_eq!(jb["dead"], json!([{"reason": "quit", "frame": 120, "events": [{"frame": 110, "key": "x"}]}]));
    for e in a.unfinished_jobs() {
        std::fs::remove_dir_all(a.root.join("jobs").join(e)).unwrap();
    }
    // x exhausted too: the cell has no candidate until T grows
    a.ingest(&job(&a, x, &[], &[], true, json!([])), None).unwrap();
    assert_eq!(a.candidate(&c), None);
    assert!(!a.selectable(&c));
    a.ingest(&job(&a, 0, &[], &[v("var:9", 4)], false, json!([])), None).unwrap();
    assert_eq!(a.candidate(&c), Some(z));
    // all of it again from the files
    let b = Archive::open(&a.root).unwrap();
    assert_eq!(b.rows, a.rows);
    assert_eq!(b.cells_json(), a.cells_json());
    assert_eq!(b.candidate(&c), Some(z));
    assert_eq!(b.progress.as_ref().unwrap().t, a.progress.as_ref().unwrap().t);
    assert_eq!(b.progress.as_ref().unwrap().exhausted, a.progress.as_ref().unwrap().exhausted);
}

#[test]
fn the_rows_keyed_again_when_t_grows_equal_a_fresh_open() {
    let _g = switch(true);
    let t = tmp("rekey");
    let mut a = make(&t);
    let l = ingest(&mut a, 0, &[nw(5, &[("var:103", json!(1))]), nw(6, &[("var:103", json!(2))])], &[]);
    let (p, q) = (ids(&l)[0], ids(&l)[1]);
    assert_eq!(a.rows[p].cell, a.rows[q].cell);
    ingest(&mut a, p, &[nw(3, &[("state:316", json!(1))])], &[]);
    let before = a.cells.len();
    // T grows with var:103 = 1: p leaves q's cell; the counts follow the log
    let l = ingest(&mut a, q, &[], &[v("var:103", 1)]);
    assert_eq!(l["t_new"], json!([["var:103", [1], null, null]]));
    assert_ne!(a.rows[p].cell, a.rows[q].cell);
    assert_eq!(a.cells.len(), before + 1);
    // p's one try (the state:316 entry) gave nothing of T then: n 1, chosen 1 in p's new cell
    let cp_ = a.rows[p].cell.clone();
    assert_eq!((a.cell(&cp_).n, a.cell(&cp_).chosen), (1, Some(1)));
    let b = Archive::open(&a.root).unwrap();
    assert_eq!(b.rows, a.rows);
    assert_eq!(b.cells_json(), a.cells_json());
    assert_eq!(b.select(10), a.select(10));
    assert_eq!(files::read_json(&a.root.join("cells.json")).unwrap(), a.cells_json());
}

#[test]
fn stagnation_over_t_and_rooms() {
    let _g = switch(true);
    let t = tmp("stag");
    let mut a = make(&t);
    assert!(!a.stagnant());
    a.logs = vec![json!({"job": "j0", "outcome": "ok", "new_t": [], "new_rooms": [], "new_r": [["room", 41]], "selectable": 4})];
    assert!(a.stagnant());
    a.logs = vec![json!({"job": "j0", "outcome": "ok", "new_t": ["state:316=1"], "new_rooms": [], "selectable": 3}),
                  json!({"job": "j1", "outcome": "ok", "new_t": [], "new_rooms": [], "selectable": 5}),
                  json!({"job": "j2", "outcome": "guard-killed"}),
                  json!({"job": "j2", "outcome": "error", "new_t": [], "new_rooms": [], "selectable": 5})];
    assert!(!a.stagnant());
    a.logs.push(json!({"job": "j3", "outcome": "ok", "new_t": [], "new_rooms": [], "selectable": 5}));
    assert!(a.stagnant());
    a.logs.push(json!({"job": "j4", "outcome": "ok", "new_t": [], "new_rooms": [96], "selectable": 2}));
    assert!(!a.stagnant());
}

/// E014 amendment (7)'s switch, set for one test and unset when it ends (the ENV lock is held by the caller).
struct StagCells;

impl StagCells {
    fn on() -> StagCells {
        std::env::set_var("FMI_STAG_CELLS", "1");
        StagCells
    }
}

impl Drop for StagCells {
    fn drop(&mut self) {
        std::env::remove_var("FMI_STAG_CELLS");
    }
}

/// A committed log line of a try from `entry`: its kind, its outcome, and whether it gave a new atom of T.
fn tried(job: &str, entry: usize, kind: &str, outcome: &str, gave: bool) -> Value {
    json!({"job": job, "entry": entry, "kind": kind, "outcome": outcome, "new_t": if gave { json!(["var:103=1"]) } else { json!([]) },
           "new_rooms": [], "new_r": if gave { json!([["var:103", 1]]) } else { json!([]) }, "selectable": 3})
}

/// Three cells: the root's, p's (var:103 = 1) and q's (state:316 = 1), each selectable.
fn three_cells(a: &mut Archive) -> (usize, usize, Vec<String>) {
    let l = ingest(a, 0, &[nw(5, &[("var:103", json!(1))]), nw(6, &[("state:316", json!(1))])],
                   &[v("var:103", 1), v("state:316", 1)]);
    let (p, q) = (ids(&l)[0], ids(&l)[1]);
    let cells: Vec<String> = [0, p, q].iter().map(|&i| a.rows[i].cell.clone()).collect();
    assert_eq!(cells.iter().collect::<BTreeSet<_>>().len(), 3);
    assert_eq!(a.n_selectable(), 3);
    (p, q, cells)
}

#[test]
fn stagnation_waits_for_a_try_from_every_selectable_cell() {
    let _g = switch(true);
    let t = tmp("stagcells");
    let mut a = make(&t);
    let (p, q, cells) = three_cells(&mut a);
    // the try that gave progress, then three that gave none, from the root's cell and p's: the count (3 >= 3) is
    // reached, q's cell has no try since
    a.logs = vec![tried("j0", 0, "model", "ok", true), tried("j1", 0, "model", "ok", false), tried("j2", p, "free", "ok", false),
                  tried("j3", p, "model", "ok", false)];
    assert!(a.stagnant(), "the count alone");
    let sw = StagCells::on();
    assert!(!a.stagnant());
    let since: Vec<&Value> = a.logs[1..].iter().collect();
    assert_eq!(a.untried_selectable(&since), vec![cells[2].clone()]);
    // a "return differs" from q's cell is no try of it; a free try is
    a.logs.push(tried("j4", q, "free", "return differs", false));
    assert!(!a.stagnant());
    a.logs.push(tried("j5", q, "free", "ok", false));
    assert!(a.stagnant());
    // a new atom starts a new sweep: the count and every cell again; an error counts as a try
    a.logs.push(tried("j6", q, "model", "ok", true));
    a.logs.extend([tried("j7", 0, "model", "ok", false), tried("j8", p, "model", "error", false), tried("j9", p, "free", "ok", false)]);
    assert!(!a.stagnant());
    a.logs.push(tried("j10", q, "model", "ok", false));
    assert!(a.stagnant());
    // before any try gave progress, the sweep is from the start
    a.logs = vec![tried("j0", 0, "model", "ok", false), tried("j1", p, "model", "ok", false), tried("j2", p, "free", "ok", false)];
    assert!(!a.stagnant());
    // an exhausted cell (no candidate under the current T) is not waited for
    let n = a.progress.as_ref().unwrap().t.len();
    a.progress.as_mut().unwrap().exhausted.insert(q, n);
    assert_eq!(a.n_selectable(), 2);
    assert!(a.stagnant());
    drop(sw);
    // unset: the count alone again
    a.progress.as_mut().unwrap().exhausted.clear();
    a.logs = vec![tried("j0", 0, "model", "ok", true), tried("j1", 0, "model", "ok", false), tried("j2", 0, "free", "ok", false),
                  tried("j3", 0, "model", "ok", false)];
    assert!(a.stagnant());
}

#[test]
fn stagnation_waits_for_every_selectable_cell_without_the_progress_order() {
    let _g = switch(false);
    let t = tmp("stagcells-off");
    let mut a = make(&t);
    assert!(a.progress.is_none());
    let d = job(&a, 0, &[nw(5, &[("var:103", json!(1))]), nw(6, &[("state:316", json!(1))])], &[], false, json!([]));
    let l = a.ingest(&d, None).unwrap();
    let (p, q) = (ids(&l)[0], ids(&l)[1]);
    let n = [0, p, q].iter().map(|&i| a.rows[i].cell.clone()).collect::<BTreeSet<_>>().len();
    assert_eq!(n, 3);
    assert_eq!(a.n_selectable(), 3);
    a.logs = vec![tried("j0", 0, "model", "ok", true), tried("j1", 0, "model", "ok", false), tried("j2", p, "model", "ok", false),
                  tried("j3", p, "model", "ok", false)];
    assert!(a.stagnant(), "the count alone");
    let _sw = StagCells::on();
    assert!(!a.stagnant());
    a.logs.push(tried("j4", q, "model", "ok", false));
    assert!(a.stagnant());
}

#[test]
fn off_nothing_of_e012_is_in_the_files() {
    let _g = switch(false);
    let t = tmp("off");
    let mut a = make(&t);
    assert!(a.progress.is_none());
    assert!(files::read_json(&a.root.join("stage.json")).unwrap().get("e012").is_none());
    let l = ingest(&mut a, 0, &[New { df: 5, facts: vec![("room", json!(35))], dead: Some("quit") }], &[v("var:9", 1)]);
    for k in ["t_new", "t_size", "new_t", "new_rooms", "chosen", "dead_entries", "dead"] {
        assert!(l.get(k).is_none(), "{k}");
    }
    let e = ids(&l)[0];
    assert_eq!(a.rows[e].dead, None);
    assert!(a.rows[e].to_json().get("dead").is_none());
    assert!(a.cells_json().as_object().unwrap().values().all(|c| c.get("chosen").is_none()));
    let jb = files::read_json(&a.prepare_job(e).unwrap().join("job.json")).unwrap();
    assert!(jb.get("novelty").is_none() && jb.get("dead").is_none());
    assert!(Novel::of_job(&jb, None).is_none());
    // E008's cell key and order
    assert_eq!(a.cell_key(&fact_map(&root_facts())), json!([33, [500], [["bit:7", 0], ["owner:566", 0], ["state:316", 0], ["var:103", 0]]]));
}

// --- the order against a brute force over the rows and the log ------------------------------------------------------

/// A small deterministic generator (xorshift32, as the engine's).
struct Rng(u32);

impl Rng {
    fn next(&mut self, n: u32) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x % n
    }
}

/// The plan's order recomputed from scratch: T from the log, each row's atoms from its file, the counts from the log
/// lines in order, the candidates, the keys.
fn brute(a: &Archive) -> Vec<usize> {
    let mut t: Vec<TAtom> = progress::goal_atoms(&a.goal);
    for x in &a.logs {
        for at in progress::atoms_of_json(x.get("t_new")) {
            if !t.contains(&at) {
                t.push(at);
            }
        }
    }
    let n = a.rows.len();
    let facts: Vec<FactMap> = (0..n).map(|i| facts_of(a, i)).collect();
    let get = |i: usize, k: &str| facts[i].get(k).copied().unwrap_or(0);
    let held: Vec<BTreeSet<usize>> = (0..n).map(|i| (0..t.len()).filter(|&j| t[j].holds(get(i, &t[j].key))).collect()).collect();
    let live: Vec<bool> = a.rows.iter().map(|r| r.dead.is_none()).collect();
    // cells: room, owned, held texts less the control keys
    let cell = |i: usize| -> String {
        let ego = get(i, "ego");
        let mut owned: Vec<i64> = facts[i].iter().filter(|(k, v)| k.starts_with("owner:") && **v == ego && ego != 0)
            .map(|(k, _)| k[6..].parse().unwrap()).collect();
        owned.sort();
        let mut ts: Vec<String> = held[i].iter().filter(|&&j| !progress::control_key(&t[j].key)).map(|&j| t[j].text()).collect();
        ts.sort();
        Archive::cell_id(&json!([facts[i].get("room"), owned, ts]))
    };
    let cells: Vec<String> = (0..n).map(cell).collect();
    for i in 0..n {
        assert_eq!(cells[i], a.rows[i].cell, "row {i}'s cell");
    }
    // the counts
    let mut nn: HashMap<String, i64> = HashMap::new();
    let mut chosen: HashMap<String, i64> = HashMap::new();
    let mut exhausted: HashMap<usize, usize> = HashMap::new();
    for x in a.logs.iter().filter(|x| fmi_explore::archive::committed(x)) {
        if x["outcome"] == "return differs" {
            continue;
        }
        let e = x["entry"].as_u64().unwrap() as usize;
        let c = cells[e].clone();
        let gave = !x["new_t"].as_array().unwrap().is_empty() || !x["new_rooms"].as_array().unwrap().is_empty();
        let m = nn.entry(c.clone()).or_insert(0);
        *m = if gave { 0 } else { *m + 1 };
        *chosen.entry(c).or_insert(0) += 1;
        if x["exhausted"] == json!(true) {
            exhausted.insert(e, x["t_size"].as_u64().unwrap() as usize);
        }
    }
    // novel atoms held: the least live holder by (frame, id)
    let mut first: HashMap<usize, (i64, usize)> = HashMap::new();
    for i in (0..n).filter(|&i| live[i]) {
        for &j in &held[i] {
            let k = (a.rows[i].frame, i);
            if first.get(&j).map(|&o| k < o).unwrap_or(true) {
                first.insert(j, k);
            }
        }
    }
    let novel = |i: usize| first.values().filter(|&&(_, h)| h == i).count() as i64;
    let rooms = |i: usize| -> i64 {
        let s: BTreeSet<i64> = a.chain(i).iter().filter_map(|&j| a.rows[j].room).collect();
        s.len() as i64
    };
    let ok = |i: usize| {
        live[i] && exhausted.get(&i).map(|&s| s < t.len()).unwrap_or(true) && a.rows[i].room.map(|r| a.allowed.contains(&r)).unwrap_or(false)
    };
    let ekey = |i: usize| (-rooms(i), -novel(i), a.rows[i].frame, i);
    let mut best: HashMap<String, usize> = HashMap::new();
    for i in (0..n).filter(|&i| ok(i)) {
        let c = cells[i].clone();
        match best.get(&c) {
            Some(&j) if ekey(j) <= ekey(i) => {}
            _ => {
                best.insert(c, i);
            }
        }
    }
    let mut out: Vec<((i64, i64, (i64, i64, i64, usize)), usize)> = best
        .into_iter()
        .map(|(c, i)| ((nn.get(&c).copied().unwrap_or(0), chosen.get(&c).copied().unwrap_or(0), ekey(i)), i))
        .collect();
    out.sort();
    out.into_iter().map(|(_, i)| i).collect()
}

#[test]
fn the_order_equals_a_brute_force_over_a_generated_archive() {
    let _g = switch(true);
    let t = tmp("brute");
    let mut a = make(&t);
    let mut r = Rng(3182679328);
    let keys = ["var:103", "state:316", "bit:7", "local:211", "var:5", "owner:566", "owner:567"];
    let rooms = [28, 33, 35, 38, 41, 96];
    let mut fired = 0;
    for step in 0..60 {
        let sel = a.select(1);
        let Some(&e) = sel.first() else { break };
        assert_eq!(sel, brute(&a)[..1].to_vec(), "step {step}");
        let mut news = Vec::new();
        let mut owned_facts: Vec<Vec<(&str, Value)>> = Vec::new();
        for _ in 0..r.next(3) {
            let mut f: Vec<(&str, Value)> = Vec::new();
            for _ in 0..r.next(3) {
                let k = keys[r.next(keys.len() as u32) as usize];
                f.push((k, if k.starts_with("owner:") { json!(r.next(2)) } else { json!(r.next(4)) }));
            }
            if r.next(4) == 0 {
                f.push(("room", json!(rooms[r.next(rooms.len() as u32) as usize])));
            }
            owned_facts.push(f);
        }
        for (k, f) in owned_facts.iter().enumerate() {
            let dead = if r.next(10) == 0 { Some("quit") } else { None };
            news.push(New { df: 1 + k as i64 + r.next(5) as i64, facts: f.clone(), dead });
        }
        let mut tested = Vec::new();
        if r.next(3) == 0 {
            let k = keys[r.next(keys.len() as u32) as usize];
            tested.push(TAtom { key: k.to_string(), vals: vec![r.next(4) as i64], lo: None, hi: if r.next(2) == 0 { Some(2) } else { None } });
        }
        let exhausted = r.next(6) == 0;
        let d = job(&a, e, &news, &tested, exhausted, json!([]));
        a.ingest(&d, None).unwrap();
        let all = a.select(1000);
        assert_eq!(all, brute(&a), "step {step}: the whole order");
        fired += a.stagnant() as usize;
    }
    assert!(a.rows.len() > 30, "{} rows", a.rows.len());
    // job.json's novelty against a brute force over the live rows' facts, on every row's state and on states moved off
    let jd = a.prepare_job(a.select(1)[0]).unwrap();
    let nv = Novel::of_job(&files::read_json(&jd.join("job.json")).unwrap(), None).unwrap();
    let t_all = a.progress.as_ref().unwrap().t.clone();
    let live: Vec<FactMap> = a.live_rows().into_iter().map(|i| facts_of(&a, i)).collect();
    let val = |f: &FactMap, k: &str| f.get(k).copied().unwrap_or(0);
    let mut checked = 0;
    for i in 0..a.rows.len() {
        for shift in 0..3i64 {
            let mut f = facts_of(&a, i);
            if shift > 0 {
                let k = keys[(i + shift as usize) % keys.len()];
                f.insert(k.to_string(), val(&f, k) + shift);
            }
            let mut want: Vec<String> = t_all
                .iter()
                .filter(|x| x.holds(val(&f, &x.key)) && !live.iter().any(|g| x.holds(val(g, &x.key))))
                .map(|x| x.text())
                .collect();
            let room = val(&f, "room");
            if !live.iter().any(|g| val(g, "room") == room) {
                want.push(format!("room={room}"));
            }
            assert_eq!(nv.novel(&f), want, "row {i} shift {shift}");
            checked += 1;
        }
    }
    assert!(checked > 90);
    let b = Archive::open(&a.root).unwrap();
    assert_eq!(b.select(1000), a.select(1000));
    assert_eq!(b.cells_json(), a.cells_json());
    eprintln!("brute: {} rows, {} cells, T {}, stagnant at {fired} steps", a.rows.len(), a.cells.len(), a.progress.as_ref().unwrap().t.len());
}

#[test]
fn backfill_gives_what_the_ingest_logged() {
    let _g = switch(true);
    let t = tmp("backfill");
    let mut a = make(&t);
    let tested = [v("state:316", 1), v("bit:7", 1), v("var:103", 2)];
    let l = ingest(&mut a, 0, &[nw(5, &[("state:316", json!(1))]), nw(6, &[("room", json!(35))])], &tested);
    let x = ids(&l)[1];
    ingest(&mut a, x, &[nw(5, &[("state:316", json!(1)), ("bit:7", json!(1))])], &[]);
    ingest(&mut a, 0, &[nw(9, &[("var:103", json!(2)), ("room", json!(28))])], &[]);
    let want: Vec<(Value, Value)> = a.logs.iter().map(|x| (x["new_t"].clone(), x["new_rooms"].clone())).collect();
    let n0 = a.cells_json();
    for x in a.logs.iter_mut() {
        x.as_object_mut().unwrap().remove("new_t");
        x.as_object_mut().unwrap().remove("new_rooms");
    }
    assert_eq!(a.backfill_novelty(), 3);
    let got: Vec<(Value, Value)> = a.logs.iter().map(|x| (x["new_t"].clone(), x["new_rooms"].clone())).collect();
    assert_eq!(got, want);
    assert_eq!(a.cells_json(), n0);
}

/// E016 F's switch, set for one test under the lock and unset when the guard goes (a failing test too).
struct Frontier;

impl Frontier {
    fn on() -> Frontier {
        std::env::set_var("FMI_FRONTIER", "1");
        Frontier
    }
}

impl Drop for Frontier {
    fn drop(&mut self) {
        std::env::remove_var("FMI_FRONTIER");
    }
}

#[test]
fn e016_f_the_frontier_entry_gets_the_next_worker_once() {
    // E016 section F (FMI_FRONTIER=1): entries 1 (room 28) and 2 (room 35) from the root; then entry 3 in room 35 again,
    // from entry 1 and holding an object, so in a cell of its own with three rooms on its chain. The order puts entry 3
    // first (rooms on the chain); the frontier puts entry 2 first, the first entry of room 35, which no job had yet
    let _g = switch(true);
    let t = tmp("frontier");
    let mut a = make(&t);
    let l = ingest(&mut a, 0, &[nw(5, &[("room", json!(28))]), nw(6, &[("room", json!(35))])], &[]);
    let (e1, e2) = (ids(&l)[0], ids(&l)[1]);
    let l = ingest(&mut a, e1, &[nw(4, &[("room", json!(35)), ("owner:600", json!(1))])], &[]);
    let e3 = ids(&l)[0];
    assert_ne!(a.rows[e2].cell, a.rows[e3].cell);
    assert_eq!(a.select(1), vec![e3]);
    assert_eq!(a.select_free(1), vec![e3]);
    let d = a.prepare_job(e2).unwrap();
    assert!(files::read_json(&d.join("select.json")).unwrap().get("frontier").is_none());
    std::fs::remove_dir_all(&d).unwrap();
    {
        let _f = Frontier::on();
        // the root and entry 1 had jobs; entry 2 is the first of room 35; entry 3 is not the first of its room
        assert_eq!(a.frontier(), [e2].into_iter().collect());
        assert_eq!(a.select(1), vec![e2]);
        assert_eq!(a.select_free(1), vec![e2]);
        assert_eq!(a.select(2), vec![e2, e3]);
        let d = a.prepare_job(e2).unwrap();
        assert_eq!(files::read_json(&d.join("select.json")).unwrap()["frontier"], json!(true));
        std::fs::remove_dir_all(&d).unwrap();
        // a running job on entry 2: not the frontier's any more, and its cell is busy
        a.busy.insert("j9".into(), (e2, a.rows[e2].cell.clone()));
        assert!(a.frontier().is_empty());
        assert_eq!(a.select(1), vec![e3]);
        a.busy.clear();
        // once its job is committed, the order is as before
        ingest(&mut a, e2, &[], &[]);
        assert!(a.frontier().is_empty());
        assert_eq!(a.select(1), vec![e3]);
    }
    assert!(std::env::var("FMI_FRONTIER").is_err());
}
