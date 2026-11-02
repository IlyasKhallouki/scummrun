//! E008's and E009's unit tests of the archive (test_explore.py "unit", test_e009.py "explore.py: the archive with the
//! switches"), ported: the same fixtures, the same assertions.

use fmi_explore::archive::{Archive, GIB};
use fmi_explore::files;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static N: AtomicUsize = AtomicUsize::new(0);

pub fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("archive-{}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst), name));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

pub fn guide() -> Value {
    json!({
        "format": "e008-guide-1", "goal": {"owner:566": [1]}, "map": "direct",
        "rooms": {"start": 33, "edges": {"33": [28, 35, 38], "28": [33, 41], "41": [28], "35": [33], "38": [33]},
                  "reached": [28, 33, 35, 38, 41]},
        "atoms": [["owner:566", 1, 0], ["room", 41, 1], ["state:316", 1, 2], ["var:103", null, 2], ["bit:7", 1, 3],
                  ["room", 28, 3]],
        "keys": {"owner:566": 0, "room": 1, "state:316": 2, "var:103": 2, "bit:7": 3},
        "writers": {"owner:566=1": ["obj:566"]}, "stats": {}})
}

pub fn root_facts() -> Value {
    json!({"room": 33, "ego": 1, "owner:500": 1, "var:5": 7, "at:script:1:10": 1})
}

pub fn cp(frame: i64, facts: Value, tick: i64) -> Value {
    json!([frame, facts, {"tick": tick, "rng": 1, "actors": []}])
}

pub fn make_with(tmp: &Path, guide: Value, e009: Option<Value>) -> Archive {
    std::fs::create_dir_all(tmp).unwrap();
    let gp = tmp.join("GUIDE.json");
    std::fs::write(&gp, guide.to_string()).unwrap();
    let mut a = Archive::create(&tmp.join("arch"), "test", &json!({"owner:566": [1]}), &gp, &[28, 33, 35, 38, 41], 2,
                                Some(json!({"frame": 100, "tick": 0, "qf": 1000, "room": 33})), e009, None).unwrap();
    a.add(None, 100, Some(0), &[], &[cp(100, root_facts(), 0)], None, "boot").unwrap();
    a.write_cells().unwrap();
    a
}

pub fn make_archive(tmp: &Path) -> Archive {
    make_with(tmp, guide(), None)
}

pub fn make_e009(tmp: &Path, e009: Value) -> Archive {
    make_with(tmp, guide(), Some(e009))
}

fn over(base: &Value, facts: &[(&str, Value)]) -> Value {
    let mut b = base.clone();
    for (k, v) in facts {
        b[*k] = v.clone();
    }
    b
}

/// An entry ``df`` frames after ``parent`` whose facts are the parent's with ``facts`` over them.
pub fn child(a: &mut Archive, parent: usize, df: i64, tick: i64, job: Option<&str>, facts: &[(&str, Value)]) -> usize {
    let f0 = a.rows[parent].frame;
    let base = a.info(parent).unwrap().last_cp[1].clone();
    let f = over(&base, facts);
    a.add(Some(parent), f0 + df, Some(tick), &[json!({"frame": f0 + df, "click": [1, 1]})], &[cp(f0 + df, f, tick)], job, "checkpoint").unwrap()
}

/// A job dir for ``entry`` (prepare_job) and its result: ``news`` [(df, tick, facts over the entry's)].
#[allow(clippy::too_many_arguments)]
pub fn job_with(a: &Archive, entry: usize, news: &[(i64, i64, Vec<(&str, Value)>)], tried: Value, exhausted: bool,
                ok: bool, error: Value, same: bool, reading: Value) -> PathBuf {
    let d = a.prepare_job(entry).unwrap();
    let job = files::read_json(&d.join("job.json")).unwrap();
    let f0 = a.rows[entry].frame;
    let base = a.info(entry).unwrap().last_cp[1].clone();
    let mut new = Vec::new();
    for (k, (df, tick, ov)) in news.iter().enumerate() {
        let f = over(&base, ov);
        let doc = json!({"id": null, "parent": entry, "frame": f0 + df, "tick": tick,
                         "events": [{"frame": f0 + df, "click": [2, 2]}], "checkpoints": [cp(f0 + df, f, *tick)]});
        files::write_gz_default(&d.join("new").join(format!("{k}.json.gz")), &doc).unwrap();
        new.push(json!({"file": format!("new/{k}.json.gz"), "frame": f0 + df, "tick": tick, "how": "checkpoint"}));
    }
    let res = json!({"job": job["job"], "entry": entry, "ok": ok, "error": error,
                     "return": {"same": same, "differ": {}, "boot_s": 0.0}, "reading": reading,
                     "targets": {"by_tier": {"0": 0, "1": 1, "2": 0}, "first": []}, "probes": [], "attempt": null,
                     "readings_made": [], "new": new, "tried": tried, "exhausted": exhausted,
                     "cpu_s": {"total": 1.0}, "peak_rss_mb": {"python": 1.0, "children": 0.0}, "wall_s": 0.1});
    std::fs::write(d.join("result.json"), res.to_string()).unwrap();
    d
}

pub fn job_plain(a: &Archive, entry: usize, news: &[(i64, i64, Vec<(&str, Value)>)]) -> PathBuf {
    job_with(a, entry, news, json!([]), false, true, Value::Null, true, Value::Null)
}

fn sel(a: &Archive) -> Vec<usize> {
    a.select(2)
}

fn busy(a: &mut Archive, job: &str, e: usize) {
    let c = a.rows[e].cell.clone();
    a.busy.insert(job.into(), (e, c));
}

#[test]
fn cell_key_is_room_owned_objects_and_the_guides_keys() {
    let t = tmp("cellkey");
    let a = make_archive(&t);
    let rf = fmi_explore::archive::fact_map(&root_facts());
    let k0 = a.cell_key(&rf);
    assert_eq!(k0, json!([33, [500], [["bit:7", 0], ["owner:566", 0], ["state:316", 0], ["var:103", 0]]]));
    let f = |ov: &[(&str, Value)]| fmi_explore::archive::fact_map(&over(&root_facts(), ov));
    assert_eq!(a.cell_key(&f(&[("var:5", json!(9)), ("at:script:1:12", json!(1)), ("local:200", json!(1))])), k0);
    assert_eq!(a.cell_key(&f(&[("owner:566", json!(1))]))[1], json!([500, 566]));
    assert_eq!(a.cell_key(&f(&[("owner:566", json!(3))]))[1], json!([500]));
    assert_eq!(a.cell_key(&f(&[("room", json!(41))]))[0], json!(41));
    assert_eq!(Archive::cell_id(&k0), a.rows[0].cell);
    assert_eq!(a.rows[0].cell.len(), 40);
}

#[test]
fn held_atoms_values_and_any() {
    let t = tmp("held");
    let a = make_archive(&t);
    assert!(a.rows[0].held.is_empty());
    let f = |ov: &[(&str, Value)]| fmi_explore::archive::fact_map(&over(&root_facts(), ov));
    assert_eq!(a.held(&f(&[("var:103", json!(5)), ("state:316", json!(1)), ("room", json!(41))])),
               vec![("room".to_string(), 41), ("state:316".to_string(), 1), ("var:103".to_string(), 5)]);
    assert!(a.held(&f(&[("var:103", json!(0)), ("state:316", json!(2))])).is_empty());
    assert_eq!(a.g(&f(&[])), 1);
    assert_eq!(a.g(&f(&[("owner:566", json!(1))])), 0);
}

#[test]
fn representative_is_the_least_tick_then_id() {
    let t = tmp("rep");
    let mut a = make_archive(&t);
    let x = child(&mut a, 0, 5, 50, None, &[("var:5", json!(1))]);
    assert_eq!(a.rows[x].cell, a.rows[0].cell);
    assert_eq!(a.representative(&a.rows[0].cell.clone()), 0);
    let b1 = child(&mut a, 0, 5, 50, None, &[("state:316", json!(2))]);
    let b2 = child(&mut a, 0, 6, 30, None, &[("state:316", json!(2))]);
    let b3 = child(&mut a, 0, 7, 30, None, &[("state:316", json!(2))]);
    let c = a.rows[b1].cell.clone();
    assert!(a.rows[b2].cell == c && a.rows[b3].cell == c);
    assert_eq!(a.representative(&c), b2);
    assert_eq!(a.cell(&c).entries, vec![b1, b2, b3]);
}

#[test]
fn favoured_is_the_least_holder_of_some_atom() {
    let t = tmp("fav");
    let mut a = make_archive(&t);
    let late = child(&mut a, 0, 5, 40, None, &[("state:316", json!(1))]);
    let early = child(&mut a, 0, 6, 20, None, &[("state:316", json!(1)), ("bit:7", json!(1))]);
    let other = child(&mut a, 0, 7, 10, None, &[("state:316", json!(2))]);
    let fav = a.favoured();
    assert!(fav.contains(&early) && !fav.contains(&late) && !fav.contains(&other) && !fav.contains(&0));
}

#[test]
fn order_g_first() {
    let t = tmp("g");
    let mut a = make_archive(&t);
    busy(&mut a, "j", 0);
    let gx = child(&mut a, 0, 5, 100, None, &[("owner:566", json!(1))]);
    let y = child(&mut a, 0, 6, 1, None, &[("room", json!(41))]);
    assert_eq!(sel(&a), vec![gx, y]);
}

#[test]
fn order_favoured_second() {
    let t = tmp("fav2");
    let mut a = make_archive(&t);
    busy(&mut a, "j", 0);
    let f = child(&mut a, 0, 5, 50, None, &[("bit:7", json!(1))]);
    let u = child(&mut a, 0, 6, 10, None, &[("room", json!(35))]);
    assert_eq!(a.rows[f].g, a.rows[u].g);
    assert_eq!(sel(&a), vec![f, u]);
}

#[test]
fn order_n_third() {
    let t = tmp("n3");
    let mut a = make_archive(&t);
    busy(&mut a, "j", 0);
    let h = child(&mut a, 0, 5, 5, None, &[("bit:7", json!(1))]);
    busy(&mut a, "k", h);
    let p = child(&mut a, h, 5, 10, None, &[("room", json!(35))]);
    let q = child(&mut a, 0, 6, 20, None, &[("room", json!(38))]);
    let pc = a.rows[p].cell.clone();
    a.cell_mut(&pc).n = 1;
    assert_eq!(sel(&a), vec![q, p]);
}

#[test]
fn order_r_fourth() {
    let t = tmp("r4");
    let mut a = make_archive(&t);
    busy(&mut a, "j", 0);
    let h = child(&mut a, 0, 5, 5, None, &[("bit:7", json!(1))]);
    busy(&mut a, "k", h);
    let p = child(&mut a, h, 5, 30, None, &[("room", json!(35))]);
    let q = child(&mut a, 0, 6, 20, None, &[("room", json!(38))]);
    assert_eq!(a.rows[p].r_path, 1);
    assert_eq!(a.rows[q].r_path, 0);
    assert_eq!(sel(&a), vec![p, q]);
}

#[test]
fn order_tick_then_id() {
    let t = tmp("tick");
    let mut a = make_archive(&t);
    busy(&mut a, "j", 0);
    let late = child(&mut a, 0, 5, 30, None, &[("room", json!(35))]);
    let early = child(&mut a, 0, 6, 20, None, &[("room", json!(38))]);
    assert_eq!(sel(&a), vec![early, late]);
    let t2 = tmp("tick-b");
    let mut b = make_archive(&t2);
    busy(&mut b, "j", 0);
    let first = child(&mut b, 0, 5, 20, None, &[("room", json!(35))]);
    let second = child(&mut b, 0, 6, 20, None, &[("room", json!(38))]);
    assert_eq!(sel(&b), vec![first, second]);
}

#[test]
fn busy_cells_and_allowed_rooms() {
    let t = tmp("busy");
    let mut a = make_archive(&t);
    let out = child(&mut a, 0, 5, 1, None, &[("room", json!(96))]);
    let s1 = child(&mut a, 0, 6, 10, None, &[("room", json!(35))]);
    let s2 = child(&mut a, 0, 7, 20, None, &[("room", json!(35)), ("var:5", json!(3))]);
    assert!(!a.select(10).contains(&out));
    busy(&mut a, "j", s2);
    assert_eq!(a.select(10), vec![0]);
    a.busy.clear();
    assert_eq!(a.select(10), vec![0, s1]);
}

#[test]
fn n_goes_to_zero_on_a_new_cell_or_representative_else_up() {
    let t = tmp("n0");
    let mut a = make_archive(&t);
    let c0 = a.rows[0].cell.clone();
    let line = a.ingest(&job_plain(&a, 0, &[(5, 10, vec![("var:5", json!(1))])]), None).unwrap();
    assert_eq!((line["new_cells"].as_i64(), line["new_reps"].as_i64(), a.cell(&c0).n), (Some(0), Some(0), 1));
    a.ingest(&job_plain(&a, 0, &[(5, 11, vec![("var:5", json!(2))])]), None).unwrap();
    assert_eq!(a.cell(&c0).n, 2);
    let line = a.ingest(&job_plain(&a, 0, &[(5, 12, vec![("state:316", json!(2))])]), None).unwrap();
    assert_eq!(line["new_cells"], json!(1));
    assert_eq!(a.cell(&c0).n, 0);
    let c1 = a.rows[line["new_entries"][0].as_u64().unwrap() as usize].cell.clone();
    a.ingest(&job_plain(&a, 0, &[(5, 13, vec![("var:5", json!(3))])]), None).unwrap();
    assert_eq!(a.cell(&c0).n, 1);
    let line = a.ingest(&job_plain(&a, 0, &[(5, 2, vec![("state:316", json!(2)), ("var:5", json!(9))])]), None).unwrap();
    assert_eq!(line["new_reps"], json!(1));
    assert_eq!(a.cell(&c0).n, 0);
    assert_eq!(a.representative(&c1) as u64, line["new_entries"][0].as_u64().unwrap());
    let b = Archive::open(&a.root).unwrap();
    assert_eq!(b.cells_json(), a.cells_json());
    assert_eq!(b.rows, a.rows);
}

#[test]
fn tried_and_exhausted_per_representative() {
    let t = tmp("tried");
    let mut a = make_archive(&t);
    busy(&mut a, "j", 0);
    let x = child(&mut a, 0, 5, 50, None, &[("room", json!(35))]);
    let c = a.rows[x].cell.clone();
    a.busy.clear();
    a.ingest(&job_with(&a, x, &[], json!([["room", 41]]), false, true, Value::Null, true, Value::Null), None).unwrap();
    assert_eq!(a.cells_json()[&c]["tried"], json!({x.to_string(): [["room", 41]]}));
    let d = a.prepare_job(x).unwrap();
    assert_eq!(files::read_json(&d.join("job.json")).unwrap()["tried"], json!([["room", 41]]));
    std::fs::remove_dir_all(&d).unwrap();
    a.ingest(&job_with(&a, x, &[], json!([["bit:7", 1]]), true, true, Value::Null, true, Value::Null), None).unwrap();
    assert_eq!(a.cells_json()[&c]["tried"][x.to_string()], json!([["bit:7", 1], ["room", 41]]));
    assert_eq!(a.cell(&c).exhausted, vec![x]);
    assert!(!a.select(10).contains(&x));
    let y = child(&mut a, 0, 6, 10, None, &[("room", json!(35)), ("var:5", json!(1))]);
    assert_eq!(a.representative(&c), y);
    assert!(a.select(10).contains(&y));
    let b = Archive::open(&a.root).unwrap();
    assert_eq!(b.cell(&c).exhausted, vec![x]);
    assert_eq!(b.cells_json()[&c]["tried"][x.to_string()], json!([["bit:7", 1], ["room", 41]]));
}

#[test]
fn movie_and_chain_checkpoints() {
    let t = tmp("movie");
    let mut a = make_archive(&t);
    let f1 = over(&root_facts(), &[("room", json!(28))]);
    let e1 = a.add(Some(0), 120, Some(5), &[json!({"frame": 105, "click": [1, 2]}), json!({"frame": 118, "key": "p"})],
                   &[cp(110, root_facts(), 3), cp(120, f1.clone(), 5)], Some("j"), "end").unwrap();
    let e2 = a.add(Some(e1), 140, Some(9), &[json!({"frame": 130, "click": [3, 4]})], &[cp(140, over(&f1, &[("room", json!(41))]), 9)], Some("j"), "end").unwrap();
    assert_eq!(a.movie(e2).unwrap(), vec![json!({"frame": 105, "click": [1, 2]}), json!({"frame": 118, "key": "p"}), json!({"frame": 130, "click": [3, 4]})]);
    assert_eq!(a.checkpoints(e2).unwrap().iter().map(|c| c[0].as_i64().unwrap()).collect::<Vec<_>>(), vec![100, 110, 120, 140]);
    assert_eq!(a.chain(e2), vec![0, e1, e2]);
    assert_eq!(a.rows[e2].r_path, 2);
    assert!(a.add(Some(e2), 150, Some(10), &[json!({"frame": 140, "click": [0, 0]})], &[cp(150, f1.clone(), 10)], None, "checkpoint").is_err());
    assert!(a.add(Some(e2), 150, Some(10), &[], &[cp(149, f1, 10)], None, "checkpoint").is_err());
}

#[test]
fn job_json() {
    let t = tmp("jobjson");
    let mut a = make_archive(&t);
    let e1 = child(&mut a, 0, 20, 5, None, &[("bit:7", json!(1))]);
    let rd = t.join("arch").join("readings").join("r1");
    std::fs::create_dir_all(&rd).unwrap();
    std::fs::write(rd.join("reading.json"), "{}").unwrap();
    let rd2 = t.join("arch").join("readings").join("r2");
    std::fs::create_dir_all(&rd2).unwrap();
    std::fs::write(rd2.join("reading.json"), "{}").unwrap();
    let p1 = rd.join("reading.json").to_string_lossy().into_owned();
    let p2 = rd2.join("reading.json").to_string_lossy().into_owned();
    a.register_reading(&[33, 28, 35, 38], &p1, 0, None, "fresh", Some(0)).unwrap();
    a.register_reading(&[28, 33, 35, 38], &p2, e1 as i64, None, "fresh", Some(1)).unwrap();
    a.register_reading(&[28, 33], &p2, e1 as i64, None, "fresh", Some(1)).unwrap();
    let d = a.prepare_job(e1).unwrap();
    let job = files::read_json(&d.join("job.json")).unwrap();
    assert_eq!(d.file_name().unwrap(), "j00000");
    let mut keys: Vec<&String> = job.as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(keys, vec!["entry", "goal", "guide", "held", "job", "reading_dir", "readings", "scope", "seed", "stage", "threads", "tried", "workers"]);
    assert_eq!((job["job"].as_str(), job["seed"].as_i64(), job["stage"].as_str()), (Some("j00000"), Some(1), Some("test")));
    assert_eq!(job["goal"], json!({"owner:566": [1]}));
    assert_eq!(job["guide"].as_str().unwrap(), std::fs::canonicalize(t.join("GUIDE.json")).unwrap().to_string_lossy());
    assert_eq!(job["entry"], json!({"id": e1, "root": false, "frame": 120, "events": a.movie(e1).unwrap(),
                                    "checkpoint": a.checkpoints(e1).unwrap().last().unwrap()}));
    assert_eq!((job["scope"].clone(), job["tried"].clone(), job["held"].clone()), (json!([28, 33, 35, 38]), json!([]), json!([["bit:7", 1]])));
    assert_eq!(job["readings"], json!([{"path": p2, "start_entry": e1}, {"path": p1, "start_entry": 0}]));
    assert_eq!(job["reading_dir"].as_str().unwrap(), a.root.join("readings").join("28-33-35-38-j00000").to_string_lossy());
    assert_eq!((job["threads"].as_i64(), job["workers"].as_i64()), (Some(3), Some(3)));
    assert_eq!(a.next_job(), "j00001");
    assert_eq!(a.unfinished_jobs(), vec!["j00000".to_string()]);
    let rj = files::read_json(&a.prepare_job(0).unwrap().join("job.json")).unwrap();
    assert_eq!(rj["entry"]["root"], json!(true));
    assert_eq!(rj["entry"]["events"], json!([]));
}

#[test]
fn readings_evict_least_recently_used_and_the_floor() {
    let t = tmp("evict");
    let mut a = make_archive(&t);
    let mut paths = Vec::new();
    for (k, used) in [3, 1, 2, 4].into_iter().enumerate() {
        let d = a.root.join("readings").join(format!("r{k}"));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("reading.json"), "{}").unwrap();
        std::fs::write(d.join("big.bin"), vec![b'x'; 1000]).unwrap();
        std::fs::write(d.join("start.txt"), "room\t33\n").unwrap();
        std::fs::write(d.join("start.jsonl"), "{}\n").unwrap();
        paths.push(d.join("reading.json"));
        a.register_reading(&[28, 33], &d.join("reading.json").to_string_lossy(), 0, None, "fresh", Some(used)).unwrap();
    }
    let ps: Vec<String> = paths.iter().map(|p| p.to_string_lossy().into_owned()).collect();
    let rf = |a: &Archive| a.readings_for(&[33, 28]).iter().map(|x| x["path"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!(rf(&a), ps.iter().rev().cloned().collect::<Vec<_>>());
    let paths2 = paths.clone();
    let mut free_now = move || 10.0 * GIB + 1000.0 * paths2.iter().filter(|p| !p.exists()).count() as f64;
    let in_use: HashSet<String> = [ps[1].clone()].into_iter().collect();
    let out = a.evict(&mut free_now, &in_use, 10.0 + 1500.0 / GIB).unwrap();
    assert_eq!(out, vec![ps[2].clone(), ps[0].clone()]);
    assert!(!paths[2].exists() && paths[2].parent().unwrap().join("start.txt").exists() && paths[1].exists());
    assert!(paths[2].parent().unwrap().join("start.jsonl").exists() && !paths[2].parent().unwrap().join("big.bin").exists());
    assert_eq!(rf(&a), vec![ps[3].clone(), ps[1].clone()]);
    let b = Archive::open(&a.root).unwrap();
    assert_eq!(rf(&b), vec![ps[3].clone(), ps[1].clone()]);
}

#[test]
fn stagnation_after_one_sweep_of_the_frontier() {
    let t = tmp("stag");
    let mut a = make_archive(&t);
    assert!(!a.stagnant());
    a.logs = vec![json!({"job": "j0", "outcome": "ok", "new_r": [], "selectable": 4})];
    assert!(a.stagnant());
    a.logs = vec![json!({"job": "j0", "outcome": "ok", "new_r": [["room", 41]], "selectable": 3}),
                  json!({"job": "j1", "outcome": "ok", "new_r": [], "selectable": 5}),
                  json!({"job": "j2", "outcome": "guard-killed"}),
                  json!({"job": "j2", "outcome": "error", "new_r": [], "selectable": 5})];
    assert!(!a.stagnant());
    a.logs.push(json!({"job": "j3", "outcome": "ok", "new_r": [], "selectable": 5}));
    assert!(a.stagnant());
}

#[test]
fn a_cut_ingest_is_rolled_back_and_the_job_is_unfinished() {
    let t = tmp("cut");
    let mut a = make_archive(&t);
    let d = job_plain(&a, 0, &[(5, 10, vec![("state:316", json!(1))])]);
    let name = d.file_name().unwrap().to_string_lossy().into_owned();
    child(&mut a, 0, 5, 10, Some(&name), &[("state:316", json!(1))]);
    assert_eq!(Archive::open(&a.root).unwrap().rows.len(), 1);
    let mut b = Archive::open(&a.root).unwrap();
    assert!(!a.root.join("entries").join("1.json.gz").exists());
    assert_eq!(b.unfinished_jobs(), vec![name]);
    let line = b.ingest(&d, None).unwrap();
    assert_eq!(line["new_entries"], json!([1]));
    assert_eq!(line["new_r"], json!([["state:316", 1]]));
    assert!(b.unfinished_jobs().is_empty());
}

#[test]
fn ingest_refuses_a_segment_that_is_not_after_the_entry() {
    let t = tmp("seg");
    let mut a = make_archive(&t);
    let d = job_plain(&a, 0, &[(0, 10, vec![("state:316", json!(1))])]);
    let line = a.ingest(&d, None).unwrap();
    assert_eq!(line["outcome"], json!("error"));
    assert!(line["error"].as_str().unwrap().starts_with("ingest:"));
    assert_eq!(line["new_entries"], json!([]));
}

#[test]
fn readings_registered_from_results() {
    let t = tmp("regres");
    let mut a = make_archive(&t);
    let rd = a.root.join("readings").join("28-33-35-38-j00000");
    std::fs::create_dir_all(&rd).unwrap();
    std::fs::write(rd.join("reading.json"), "{}").unwrap();
    let p = rd.join("reading.json").to_string_lossy().into_owned();
    a.ingest(&job_with(&a, 0, &[], json!([]), false, true, Value::Null, true, json!({"path": p, "fresh": true, "scope": [28, 33, 35, 38]})), None).unwrap();
    assert_eq!(a.readings_for(&[28, 33, 35, 38]).iter().map(|x| x["start_entry"].as_i64().unwrap()).collect::<Vec<_>>(), vec![0]);
    a.ingest(&job_with(&a, 0, &[], json!([]), false, true, Value::Null, true, json!({"path": p, "fresh": false, "scope": [28, 33, 35, 38]})), None).unwrap();
    assert_eq!(a.readings[0]["last_used"], json!(1));
}

// --- test_e009.py: the archive with the switches ----------------------------------------------------------------

fn with_idle(d: &Path, keys: &[&str]) -> PathBuf {
    let mut r = files::read_json(&d.join("result.json")).unwrap();
    r["idle"] = json!({"returns": keys, "first_return": {}, "changes": {}, "frames": 1600, "seconds": 0.0});
    std::fs::write(d.join("result.json"), r.to_string()).unwrap();
    d.to_path_buf()
}

#[test]
fn stage_json_keeps_the_switches_only_when_given() {
    let t = tmp("sw");
    let off = make_archive(&t.join("off"));
    assert!(files::read_json(&off.root.join("stage.json")).unwrap().get("e009").is_none());
    let on = make_e009(&t.join("on"), json!({"idle_frames": 1600, "rotate": true, "stop_jobs": 80}));
    assert_eq!(files::read_json(&on.root.join("stage.json")).unwrap()["e009"], json!({"idle_frames": 1600, "rotate": true, "stop_jobs": 80}));
    assert_eq!(Archive::open(&on.root).unwrap().idle_frames, 1600);
}

#[test]
fn idle_keys_leave_the_cell_key_and_the_atoms_and_the_rows_are_keyed_again() {
    let t = tmp("idle");
    let mut a = make_e009(&t, json!({"idle_frames": 1600}));
    let e1 = child(&mut a, 0, 5, 5, None, &[("room", json!(35)), ("var:103", json!(1))]);
    let e2 = child(&mut a, 0, 6, 6, None, &[("room", json!(35)), ("var:103", json!(2))]);
    let cells: HashSet<String> = [0, e1, e2].iter().map(|&i| a.rows[i].cell.clone()).collect();
    assert_eq!(cells.len(), 3);
    assert!(a.held_by.contains_key(&("var:103".to_string(), 2)));
    let new = a.merge_idle(0, &["var:103".into(), "owner:566".into(), "var:990".into()]).unwrap();
    assert_eq!(new, vec!["var:103".to_string(), "var:990".to_string()]);
    assert_eq!(a.idle.iter().cloned().collect::<Vec<_>>(), vec!["var:103".to_string(), "var:990".to_string()]);
    assert!(a.rows[e1].cell == a.rows[e2].cell && a.rows[e1].cell != a.rows[0].cell && a.cells.len() == 2);
    assert_eq!(a.representative(&a.rows[e1].cell.clone()), e1);
    assert!(!a.held_by.keys().any(|(k, _)| k == "var:103"));
    assert_eq!(a.rows[e2].held, a.rows[0].held);
    assert!(a.merge_idle(e1 as i64, &["var:103".into()]).unwrap().is_empty());
    assert_eq!(a.idle_measured.iter().copied().collect::<Vec<_>>(), vec![0, e1 as i64]);
    let d = with_idle(&job_plain(&a, e2, &[(3, 9, vec![("var:103", json!(7))])]), &["var:103", "owner:566"]);
    let line = a.ingest(&d, None).unwrap();
    assert_eq!(line["idle"], json!({"entry": e2, "keys": ["var:103"], "new": [], "size": 2, "frames": 1600, "seconds": 0.0}));
    assert_eq!(line["new_cells"], json!(0));
    let b = Archive::open(&a.root).unwrap();
    assert_eq!(b.idle.iter().cloned().collect::<Vec<_>>(), vec!["var:103".to_string()]);
    assert_eq!(b.idle_measured.iter().copied().collect::<Vec<_>>(), vec![e2 as i64]);
    assert_eq!(b.cells.len(), 2);
    assert_eq!(b.rows[e1].cell, b.rows[e2].cell);
}

#[test]
fn off_the_rows_are_e008s() {
    let t = tmp("off");
    let mut a = make_archive(&t);
    let e1 = child(&mut a, 0, 5, 5, None, &[("var:103", json!(1))]);
    let e2 = child(&mut a, 0, 6, 6, None, &[("var:103", json!(2))]);
    let cells: HashSet<String> = [0, e1, e2].iter().map(|&i| a.rows[i].cell.clone()).collect();
    assert_eq!(cells.len(), 3);
    assert_eq!(a.job_e009(e1), json!({}));
    assert_eq!(a.idle_frames, 0);
}

#[test]
fn a_cell_offers_its_next_entry_once_its_first_is_exhausted() {
    let t = tmp("next");
    let mut a = make_e009(&t, json!({"idle_frames": 1600}));
    a.merge_idle(0, &["var:103".into()]).unwrap();
    let e1 = child(&mut a, 0, 5, 5, None, &[("room", json!(35)), ("var:103", json!(1))]);
    let e2 = child(&mut a, 0, 6, 6, None, &[("room", json!(35)), ("var:103", json!(2))]);
    let c = a.rows[e1].cell.clone();
    assert_eq!(a.rows[e2].cell, c);
    assert_eq!(a.candidate(&c), Some(e1));
    a.ingest(&job_with(&a, e1, &[], json!([]), true, true, Value::Null, true, Value::Null), None).unwrap();
    assert_eq!(a.candidate(&c), Some(e2));
    assert!(a.select(10).contains(&e2));
    a.ingest(&job_with(&a, e2, &[], json!([]), true, true, Value::Null, true, Value::Null), None).unwrap();
    assert_eq!(a.candidate(&c), None);
    assert!(!a.selectable(&c));
    let mut off = make_archive(&t.join("off"));
    let x = child(&mut off, 0, 5, 5, None, &[("room", json!(35))]);
    let y = child(&mut off, 0, 6, 6, None, &[("room", json!(35)), ("var:5", json!(7))]);
    let cc = off.rows[x].cell.clone();
    assert_eq!(off.rows[y].cell, cc);
    off.ingest(&job_with(&off, x, &[], json!([]), true, true, Value::Null, true, Value::Null), None).unwrap();
    assert_eq!(off.candidate(&cc), None);
    assert!(!off.select(10).contains(&y));
}

#[test]
fn n_counted_again_from_the_log_when_cells_merge() {
    let t = tmp("merge");
    let mut a = make_e009(&t, json!({"idle_frames": 1600}));
    let e1 = child(&mut a, 0, 5, 5, None, &[("room", json!(35)), ("var:103", json!(1))]);
    let e2 = child(&mut a, 0, 6, 6, None, &[("room", json!(35)), ("var:103", json!(2))]);
    a.ingest(&job_plain(&a, e1, &[(2, 7, vec![("var:5", json!(1))])]), None).unwrap();
    a.ingest(&job_plain(&a, e2, &[(2, 8, vec![("var:5", json!(1))])]), None).unwrap();
    assert_eq!(a.cell(&a.rows[e1].cell.clone()).n, 1);
    assert_eq!(a.cell(&a.rows[e2].cell.clone()).n, 1);
    a.merge_idle(0, &["var:103".into()]).unwrap();
    assert_eq!(a.cell(&a.rows[e1].cell.clone()).n, 2);
}

#[test]
fn rotation_counts_per_cell_and_in_all() {
    let t = tmp("rot");
    let mut a = make_e009(&t, json!({"rotate": true}));
    let e1 = child(&mut a, 0, 5, 5, None, &[("room", json!(35))]);
    let e2 = child(&mut a, 0, 6, 6, None, &[("room", json!(38))]);
    a.ingest(&job_with(&a, e1, &[], json!([["bit:7", 1], ["room", 41]]), false, true, Value::Null, true, Value::Null), None).unwrap();
    a.ingest(&job_with(&a, e2, &[], json!([["bit:7", 1]]), false, true, Value::Null, true, Value::Null), None).unwrap();
    a.ingest(&job_with(&a, e1, &[], json!([["state:316", 1]]), false, true, Value::Null, true, Value::Null), None).unwrap();
    assert_eq!(a.rotation(e1), json!({"cell": [["bit:7", 1, 1], ["room", 41, 1], ["state:316", 1, 1]],
                                      "all": [["bit:7", 1, 2], ["room", 41, 1], ["state:316", 1, 1]]}));
    assert_eq!(a.rotation(e2)["cell"], json!([["bit:7", 1, 1]]));
    let d = a.prepare_job(e2).unwrap();
    let job = files::read_json(&d.join("job.json")).unwrap();
    assert_eq!(job["rotation"], a.rotation(e2));
    assert!(job.get("idle_frames").is_none() && job.get("reuse").is_none());
}

#[test]
fn job_json_carries_the_switches() {
    let t = tmp("sw2");
    let mut a = make_e009(&t, json!({"idle_frames": 1600, "reuse_nonfacts": true}));
    a.merge_idle(0, &[]).unwrap();
    let job = files::read_json(&a.prepare_job(0).unwrap().join("job.json")).unwrap();
    assert_eq!((job["idle_frames"].as_i64(), job["idle_measured"].as_bool(), job["reuse"].as_str()), (Some(1600), Some(true), Some("e009")));
    assert!(job.get("rotation").is_none());
}

#[test]
fn evict_level_from_the_largest_reading() {
    let t = tmp("level");
    let mut a = make_e009(&t, json!({"evict_by_size": true}));
    assert_eq!(a.evict_level_gb(8.0), 8.0);
    for (k, n) in [1000u64, 3 * (1u64 << 30) / 10, 2000].into_iter().enumerate() {
        let d = a.root.join("readings").join(format!("r{k}"));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("reading.json"), "{}").unwrap();
        std::fs::write(d.join("big.bin"), vec![b'x'; n.min(4096) as usize]).unwrap();
        a.register_reading(&[33], &d.join("reading.json").to_string_lossy(), 0, None, "fresh", Some(k as i64)).unwrap();
        a.readings.last_mut().unwrap().insert("bytes".into(), json!(n));
    }
    assert!((a.evict_level_gb(8.0) - 8.6).abs() < 1e-6);
}

#[test]
fn stagnation_counts_only_atoms_of_keys_outside_i() {
    let t = tmp("stagi");
    let mut a = make_e009(&t, json!({"idle_frames": 1600}));
    a.ingest(&job_plain(&a, 0, &[(5, 5, vec![("var:103", json!(1))])]), None).unwrap();
    assert!(!a.stagnant());
    a.idle.insert("var:103".into());
    assert!(a.stagnant());
}
