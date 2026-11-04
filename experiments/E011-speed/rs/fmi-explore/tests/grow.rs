//! E014 amendment (4) (b) (experiments/E014-fate-demo/plan.md): the archive's grown scopes under FMI_SCOPE_GROW=1.
//! The first scope of a room is the room alone; a try's need and its model-free entries in another ring room grow it
//! at the commit (the log line's "grow"); a reopened archive replays the lines; with the switch unset the scope is the
//! ring and job.json is as before.

use fmi_explore::archive::Archive;
use fmi_explore::files;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};

static N: AtomicUsize = AtomicUsize::new(0);
static ENV: Mutex<()> = Mutex::new(());

fn switch(on: bool) -> MutexGuard<'static, ()> {
    let g = ENV.lock().unwrap_or_else(|e| e.into_inner());
    if on {
        std::env::set_var("FMI_SCOPE_GROW", "1");
    } else {
        std::env::remove_var("FMI_SCOPE_GROW");
    }
    std::env::remove_var("FMI_READ_BOUND");
    g
}

fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("grow-{}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst), name));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn guide() -> Value {
    json!({
        "format": "e008-guide-1", "goal": {"owner:566": [1]}, "map": "direct",
        "rooms": {"start": 33, "edges": {"33": [28, 35, 38], "28": [33, 41], "41": [28], "35": [33], "38": [33]},
                  "reached": [28, 33, 35, 38, 41]},
        "atoms": [["owner:566", 1, 0], ["room", 41, 1]],
        "keys": {"owner:566": 0, "room": 1},
        "writers": {"owner:566=1": ["obj:566"]}, "stats": {}})
}

fn cp(frame: i64, facts: Value, tick: i64) -> Value {
    json!([frame, facts, {"tick": tick, "rng": 1, "actors": []}])
}

fn make(t: &Path) -> Archive {
    let gp = t.join("GUIDE.json");
    std::fs::write(&gp, guide().to_string()).unwrap();
    let mut a = Archive::create(&t.join("arch"), "test", &json!({"owner:566": [1]}), &gp, &[28, 33, 35, 38, 41], 2,
                                Some(json!({"frame": 100, "tick": 0, "qf": 1000, "room": 33})), None, None).unwrap();
    a.add(None, 100, Some(0), &[], &[cp(100, json!({"room": 33, "ego": 1}), 0)], None, "boot").unwrap();
    a.write_cells().unwrap();
    a
}

/// A job from `entry` whose result has new entries (how, room) and the try's scope need.
fn job(a: &Archive, entry: usize, news: &[(&str, i64)], need: Option<Value>) -> PathBuf {
    let d = a.prepare_job(entry).unwrap();
    let jb = files::read_json(&d.join("job.json")).unwrap();
    let f0 = a.rows[entry].frame;
    let mut new = Vec::new();
    for (k, (how, room)) in news.iter().enumerate() {
        let f = f0 + 5 * (k as i64 + 1);
        let doc = json!({"id": null, "parent": entry, "frame": f, "tick": f, "events": [{"frame": f, "click": [2, 2]}],
                         "checkpoints": [cp(f, json!({"room": room, "ego": 1, "var:9": k}), f)]});
        files::write_gz_default(&d.join("new").join(format!("{k}.json.gz")), &doc).unwrap();
        new.push(json!({"file": format!("new/{k}.json.gz"), "frame": f, "tick": f, "how": how}));
    }
    let mut res = json!({"job": jb["job"], "entry": entry, "ok": true, "error": null,
                         "return": {"same": true, "differ": {}, "boot_s": 0.0}, "reading": null,
                         "targets": {"by_tier": {"0": 0, "1": 1, "2": 0}, "first": []}, "probes": [], "attempt": null,
                         "readings_made": [], "new": new, "tried": [], "exhausted": false,
                         "cpu_s": {"total": 1.0}, "peak_rss_mb": {"python": 1.0, "children": 0.0}, "wall_s": 0.1});
    if let Some(n) = need {
        res["scope_need"] = n;
    }
    std::fs::write(d.join("result.json"), res.to_string()).unwrap();
    d
}

fn job_json(a: &Archive, i: usize) -> Value {
    a.job_doc("jx", i).unwrap()
}

#[test]
fn the_first_scope_is_the_room_and_it_grows_at_commits() {
    let _g = switch(true);
    let t = tmp("grow");
    let mut a = make(&t);
    let j = job_json(&a, 0);
    assert_eq!(j["scope"], json!([33]));
    assert_eq!(j["ring"], json!([28, 33, 35, 38]));
    // a try that kept a checkpoint in room 38 (the model's own: no growth) and needed nothing
    let d = job(&a, 0, &[("checkpoint", 38)], None);
    let line = a.ingest(&d, None).unwrap();
    assert!(line.get("grow").is_none());
    assert_eq!(job_json(&a, 0)["scope"], json!([33]));
    // a target needs room 28, and a free step entered room 35; room 41 is outside the ring, 33 is the scope's
    let d = job(&a, 0, &[("free", 35), ("afford", 41), ("wait", 33)], Some(json!({"room": 28, "why": "target", "atom": "room=41"})));
    let line = a.ingest(&d, None).unwrap();
    assert_eq!(line["grow"], json!({"room": 33, "add": [28, 35], "why": ["target", "left"], "scope": [28, 33, 35]}));
    assert_eq!(job_json(&a, 0)["scope"], json!([28, 33, 35]));
    // the entry in room 35 has its own first scope; room 41's (outside the allowed? no: allowed, ring {28, 41})
    let e35 = line["new_entries"][0].as_u64().unwrap() as usize;
    assert_eq!(job_json(&a, e35)["scope"], json!([35]));
    let e41 = line["new_entries"][1].as_u64().unwrap() as usize;
    assert_eq!(job_json(&a, e41)["scope"], json!([41]));
    assert_eq!(job_json(&a, e41)["ring"], json!([28, 41]));
    // a need of a room the scope holds, or outside the ring: nothing
    let d = job(&a, 0, &[], Some(json!({"room": 35, "why": "plan"})));
    assert!(a.ingest(&d, None).unwrap().get("grow").is_none());
    let d = job(&a, 0, &[], Some(json!({"room": 41, "why": "plan"})));
    assert!(a.ingest(&d, None).unwrap().get("grow").is_none());
    // a reopened archive replays the lines
    let b = Archive::open(&a.root).unwrap();
    assert_eq!(job_json(&b, 0)["scope"], json!([28, 33, 35]));
    assert_eq!(b.grown, a.grown);
    drop(_g);
    // and without the switch too, from the lines' "grow"
    let _g = switch(false);
    let c = Archive::open(&a.root).unwrap();
    assert_eq!(job_json(&c, 0)["scope"], json!([28, 33, 35]));
}

#[test]
fn off_the_scope_is_the_ring() {
    let _g = switch(false);
    let t = tmp("off");
    let mut a = make(&t);
    let j = job_json(&a, 0);
    assert_eq!(j["scope"], json!([28, 33, 35, 38]));
    assert!(j.get("ring").is_none());
    assert!(j.get("readings_room").is_none());
    let d = job(&a, 0, &[("free", 35)], Some(json!({"room": 28, "why": "target"})));
    let line = a.ingest(&d, None).unwrap();
    assert!(line.get("grow").is_none());
    assert!(a.grown.is_none());
}

#[test]
fn the_readings_that_may_stand_in() {
    let _g = switch(true);
    std::env::set_var("FMI_READ_BOUND", "300");
    let t = tmp("standin");
    let mut a = make(&t);
    let p = |n: &str| {
        let d = t.join(n);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("reading.json"), "{}").unwrap();
        d.join("reading.json").to_string_lossy().into_owned()
    };
    let (r1, r2, r3) = (p("r1"), p("r2"), p("r3"));
    a.register_reading(&[28, 33], &r1, 0, Some("j1"), "fresh", Some(1)).unwrap();
    a.register_reading(&[33], &r2, 0, Some("j2"), "fresh", Some(2)).unwrap();
    a.register_reading(&[28, 41], &r3, 0, Some("j3"), "fresh", Some(3)).unwrap();
    let j = job_json(&a, 0);
    // the scope's own reading is in "readings"; the others holding room 33 in "readings_room", newest first
    assert_eq!(j["readings"].as_array().unwrap().len(), 1);
    assert_eq!(j["readings_room"].as_array().unwrap().iter().map(|x| x["path"].as_str().unwrap().to_string()).collect::<Vec<_>>(), vec![r1]);
    std::env::remove_var("FMI_READ_BOUND");
}
