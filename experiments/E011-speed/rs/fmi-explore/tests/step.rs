//! test_step.py's and test_e009.py's tests of step.py's parts that run alone, ported: targets, the reuse rule,
//! entries, the start row, the result's records, E009's idle rule and rotation.

use fmi_explore::archive::fact_map;
use fmi_explore::step::{self, Target, TV};
use serde_json::{json, Value};
use std::collections::HashSet;

const REACHED: &str = "room\t28\t33\t41\nowner:566\t1\t15\nstate:316\t0\t1\nvar:103\t0\t1\t2\nvar:53\tbelow\t-1\t0\t1\t2\tabove\npos:local:215\t0\t1\nat:local:203:30\t0\t1\nbit:6\t0\nclass:426:32\t0\t1\nowner:566\t1\n";

fn guide() -> Value {
    json!({"format": "e008-guide-1", "goal": {"owner:566": [1]}, "map": "direct",
           "rooms": {"start": 33, "edges": {"33": [28, 35, 38], "28": [33, 41], "41": [28]}, "reached": [28, 33, 41]},
           "atoms": [["owner:566", 1, 0], ["bit:6", 1, 1], ["room", 41, 1], ["state:316", 1, 1],
                     ["class:426:32", 1, 2], ["var:103", null, 2], ["var:53", 2, 3], ["var:53", null, 4]],
           "keys": {}, "writers": {}, "stats": {}})
}

fn entry() -> Value {
    json!({"room": 33, "owner:566": 15, "state:316": 0, "var:103": 0, "var:53": 0})
}

fn goal() -> Vec<(String, Vec<i64>)> {
    vec![("owner:566".into(), vec![1])]
}

fn t(k: &str, v: i64, tier: i64, depth: i64) -> Target {
    Target { k: k.into(), v: TV::Int(v), tier, depth }
}

fn reached() -> Vec<(String, Vec<i64>)> {
    step::read_reached_text(REACHED)
}

fn targets(e: &Value, root: Option<&Value>, held: Value, tried: Value, g: &[(String, Vec<i64>)], rot: Option<&Value>) -> Vec<Target> {
    let rf = root.map(fact_map);
    step::targets(&reached(), &guide(), &fact_map(e), rf.as_ref(), held.as_array().unwrap(), tried.as_array().unwrap(), g, rot).unwrap()
}

#[test]
fn read_reached_keeps_ints_and_joins_repeated_keys() {
    let r: std::collections::HashMap<String, Vec<i64>> = reached().into_iter().collect();
    assert_eq!(r["var:53"], vec![-1, 0, 1, 2]);
    assert_eq!(r["owner:566"], vec![1, 15]);
    assert_eq!(r["room"], vec![28, 33, 41]);
}

#[test]
fn targets_tiers_and_order() {
    let ts = targets(&entry(), None, json!([["state:316", 1]]), json!([["var:103", 2]]), &goal(), None);
    assert_eq!(ts, vec![t("owner:566", 1, 0, 0), t("room", 41, 1, 1), t("class:426:32", 1, 1, 2), t("var:103", 1, 1, 2),
                        t("var:53", 2, 1, 3), t("var:53", -1, 1, 4), t("var:53", 1, 1, 4), t("state:316", 1, 2, 1)]);
}

#[test]
fn targets_leave_out_the_entrys_values_other_kinds_and_atoms_not_in_r() {
    let ts = targets(&entry(), None, json!([]), json!([]), &goal(), None);
    let keys: Vec<(String, TV)> = ts.iter().map(|x| (x.k.clone(), x.v.clone())).collect();
    let has = |k: &str, v: i64| keys.contains(&(k.to_string(), TV::Int(v)));
    assert!(!has("owner:566", 15));
    assert!(!has("room", 28) && !has("room", 33));
    assert!(!keys.iter().any(|(k, _)| k.starts_with("pos:") || k.starts_with("at:")));
    assert!(!has("bit:6", 0));
    assert_eq!(keys.iter().filter(|x| **x == ("owner:566".to_string(), TV::Int(1))).count(), 1);
}

#[test]
fn tier_0_needs_every_goal_value_reached_and_the_entry_not_holding_it() {
    let mut held_goal = entry();
    held_goal["owner:566"] = json!(1);
    assert!(targets(&held_goal, None, json!([]), json!([]), &goal(), None).iter().all(|x| x.tier > 0));
    let other = step::read_reached_text("owner:566\t15\nroom\t33\t41\n");
    let ts = step::targets(&other, &guide(), &fact_map(&entry()), None, &[], &[], &goal(), None).unwrap();
    assert_eq!(ts, vec![t("room", 41, 1, 1)]);
    assert_eq!(targets(&entry(), None, json!([]), json!([["owner:566", 1]]), &goal(), None)[0], t("room", 41, 1, 1));
}

#[test]
fn tier_0_of_a_goal_of_several_keys() {
    let g = vec![("owner:566".to_string(), vec![1]), ("room".to_string(), vec![41])];
    let ts = targets(&entry(), None, json!([]), json!([]), &g, None);
    assert_eq!(ts[0], Target { k: "goal".into(), v: TV::Text("owner:566=1,room=41".into()), tier: 0, depth: 0 });
    assert!(ts.contains(&t("owner:566", 1, 1, 0)) && ts.contains(&t("room", 41, 1, 1)));
}

#[test]
fn held_any_atom_and_root_values() {
    let ts = targets(&entry(), None, json!([["var:103", null]]), json!([]), &goal(), None);
    assert!(ts.contains(&t("var:103", 1, 2, 2)) && ts.contains(&t("var:103", 2, 2, 2)));
    let mut e = entry();
    e["var:103"] = json!(2);
    let ts = targets(&e, Some(&json!({"var:103": 1, "var:53": 2})), json!([]), json!([]), &goal(), None);
    let keys: HashSet<(String, TV)> = ts.iter().map(|x| (x.k.clone(), x.v.clone())).collect();
    assert!(!keys.contains(&("var:103".into(), TV::Int(1))) && keys.contains(&("var:103".into(), TV::Int(0))));
    assert!(keys.contains(&("var:53".into(), TV::Int(2))));
}

#[test]
fn reusable_only_task_facts_and_slot_keys_may_differ() {
    let start = json!({"var:1": 0, "var:2": 5, "room": 33, "bit:5": 1, "at:local:200:10": 1, "script:3": 1});
    let task: HashSet<String> = ["var:1", "room"].iter().map(|s| s.to_string()).collect();
    let mut same = start.clone();
    for (k, v) in [("var:1", 4), ("room", 28), ("local:201", 1), ("objscript:9", 1), ("esc:script:3:20", 1)] {
        same[k] = json!(v);
    }
    same.as_object_mut().unwrap().remove("at:local:200:10");
    same.as_object_mut().unwrap().remove("script:3");
    let sf = fact_map(&start);
    assert_eq!(step::reusable(&sf, &task, &fact_map(&same)), (true, 0));
    let mut s2 = same.clone();
    s2["var:2"] = json!(6);
    assert_eq!(step::reusable(&sf, &task, &fact_map(&s2)), (false, 1));
    let mut other = same.clone();
    other.as_object_mut().unwrap().remove("bit:5");
    other["verb:7"] = json!(1);
    assert_eq!(step::reusable(&sf, &task, &fact_map(&other)), (false, 2));
    let d = step::reusable_detail(&sf, &task, &fact_map(&s2));
    assert_eq!(d["other"], json!(1));
    assert_eq!(d["first"], json!([["var:2", 5, 6]]));
    assert_eq!(d["task"], json!(2));
}

fn cpj(frame: i64, tick: i64, facts: Value) -> Value {
    json!([frame, facts, {"tick": tick, "rng": 7, "actors": [[1, 2, 3]]}])
}

#[test]
fn entries_from_a_movie_and_checkpoints() {
    let inputs = vec![json!({"frame": 2736, "click": [1, 2]}), json!({"frame": 2751, "click": [1, 2]}),
                      json!({"frame": 2800, "key": 13}), json!({"frame": 3000, "click": [5, 6]})];
    let cps = vec![cpj(2728, 100, json!({"room": 33})), cpj(2736, 150, json!({})), cpj(2751, 240, json!({})),
                   cpj(2751, 240, json!({})), cpj(2798, 500, json!({})), cpj(3216, 2900, json!({"owner:566": 1}))];
    let rev: Vec<Value> = cps.iter().rev().cloned().collect();
    let es = step::entries_from(2728, &inputs, &rev).unwrap();
    let f = |k: &str| es.iter().map(|e| e[k].clone()).collect::<Vec<_>>();
    assert_eq!(f("frame"), vec![json!(2736), json!(2751), json!(2798), json!(3216)]);
    assert_eq!(f("tick"), vec![json!(150), json!(240), json!(500), json!(2900)]);
    assert_eq!(es.iter().map(|e| e["events"].as_array().unwrap().len()).collect::<Vec<_>>(), vec![1, 2, 2, 4]);
    assert_eq!(es.iter().map(|e| e["checkpoints"].as_array().unwrap().iter().map(|c| c[0].as_i64().unwrap()).collect::<Vec<_>>()).collect::<Vec<_>>(),
               vec![vec![2736], vec![2736, 2751], vec![2736, 2751, 2798], vec![2736, 2751, 2798, 3216]]);
    assert_eq!(f("how"), vec![json!("checkpoint"), json!("checkpoint"), json!("checkpoint"), json!("end")]);
    assert!(es.iter().all(|e| e["id"].is_null()));
    let mut keys: Vec<&String> = es[0].as_object().unwrap().keys().filter(|k| *k != "how").collect();
    keys.sort();
    assert_eq!(keys, vec!["checkpoints", "events", "frame", "id", "parent", "tick"]);
    assert_eq!(es[3]["checkpoints"][3][1], json!({"owner:566": 1}));
    // the reference's order of keys in an entry
    assert_eq!(es[0].as_object().unwrap().keys().collect::<Vec<_>>(), vec!["id", "parent", "frame", "tick", "events", "checkpoints", "how"]);
    assert!(step::entries_from(2736, &inputs, &cps).is_err());
    assert!(step::entries_from(3216, &[], &cps).unwrap().is_empty());
}

#[test]
fn first_plan_status_skips_the_reach_extensions_try() {
    assert_eq!(step::first_plan_status(&[json!({"status": "reach extension"}), json!({"status": "plan"})]).as_deref(), Some("plan"));
    assert_eq!(step::first_plan_status(&[json!({"status": "unsolvable"})]).as_deref(), Some("unsolvable"));
    assert_eq!(step::first_plan_status(&[]), None);
}

#[test]
fn result_keys_are_plan_mds() {
    let r = step::empty_result(&json!({"job": "j1", "entry": {"id": 4}}));
    let mut keys: Vec<&String> = r.as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(keys, vec!["attempt", "cpu_s", "entry", "error", "exhausted", "job", "new", "ok", "peak_rss_mb", "probes",
                          "reading", "readings_made", "return", "targets", "tried", "wall_s"]);
    let att = step::attempt_record(&t("room", 41, 1, 1), &json!({"stop": "goal", "frames": 3}), 1.0);
    for k in ["target", "stop", "reached", "goal_from_the_plan", "frames", "qf0", "qf_end", "replans", "refinements",
              "extensions", "window_summary", "seconds"] {
        assert!(att.get(k).is_some(), "{k}");
    }
    assert_eq!(att["target"], json!(["room", 41]));
}

#[test]
fn attempt_costs_split_an_attempts_cpu() {
    let res = json!({"extensions": [], "refinements": [{"plan": 0, "added": []}, {"plan": 1, "cpu_s": 4.0}],
                     "window_summary": {"engine_cpu_s": {"child": 100.0, "stone trial": 20.0}, "python_cpu_s": 30.0}});
    assert_eq!(step::attempt_costs(&res, 50.0), (4.0, 150.0, 16.0));
    assert_eq!(step::attempt_costs(&json!({"extensions": [{"cpu_s": 2.0}]}), 1.0), (2.0, 0.0, 0.0));
    assert_eq!(step::attempt_costs(&json!({}), 3.0), (0.0, 0.0, 3.0));
}

#[test]
fn exit_codes() {
    assert_eq!(step::exit_code(&json!({"ok": true})), 0);
    assert_eq!(step::exit_code(&json!({"ok": false, "return": {"same": false}})), 3);
    assert_eq!(step::exit_code(&json!({"ok": false, "return": null})), 1);
}

#[test]
fn cached_readings_newest_first_and_same_scope() {
    let tmp = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("cached-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let mut paths = Vec::new();
    for (i, rooms) in [vec![28, 33, 41], vec![28, 33], vec![41, 33, 28]].iter().enumerate() {
        let d = tmp.join(format!("r{i}"));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("reading.json"), json!({"scope": {"rooms": rooms}, "packed": "reading.tc"}).to_string()).unwrap();
        std::fs::write(d.join("reading.tc"), "").unwrap();
        std::fs::write(d.join("reading.reached"), "").unwrap();
        set_mtime(&d.join("reading.json"), 1000 + i as i64);
        paths.push(d.join("reading.json"));
    }
    std::fs::create_dir_all(tmp.join("r3")).unwrap();
    std::fs::write(tmp.join("r3").join("reading.json"), json!({"scope": {"rooms": [28, 33, 41]}, "packed": "x.tc"}).to_string()).unwrap();
    let mut rs: Vec<Value> = paths.iter().map(|p| json!({"path": p.to_string_lossy()})).collect();
    rs.push(json!({"path": tmp.join("r3").join("reading.json").to_string_lossy()}));
    let job = json!({"scope": [33, 28, 41], "readings": rs});
    assert_eq!(step::cached_readings(&job), vec![paths[2].clone(), paths[0].clone()]);
    let rows = tmp.join("first").join("start.jsonl");
    let mut doc: Value = serde_json::from_str(&std::fs::read_to_string(&paths[2]).unwrap()).unwrap();
    doc["start"] = json!({"name": format!("{} seed 1", rows.display())});
    std::fs::write(&paths[2], doc.to_string()).unwrap();
    set_mtime(&paths[2], 1002);
    assert_eq!(step::cached_readings(&job), vec![paths[0].clone()]);
    std::fs::create_dir_all(rows.parent().unwrap()).unwrap();
    std::fs::write(&rows, "{}\n").unwrap();
    assert_eq!(step::cached_readings(&job), vec![paths[2].clone(), paths[0].clone()]);
    assert_eq!(step::start_rows_of(&json!({"start": {"name": "results/start-part1.jsonl seed 1"}})), None);
}

fn set_mtime(p: &std::path::Path, t: i64) {
    let c = std::ffi::CString::new(p.to_string_lossy().as_bytes()).unwrap();
    let ts = [libc_timespec(t), libc_timespec(t)];
    unsafe {
        utimensat(-100, c.as_ptr(), ts.as_ptr(), 0);
    }
}

#[repr(C)]
struct Timespec {
    tv_sec: i64,
    tv_nsec: i64,
}

fn libc_timespec(t: i64) -> Timespec {
    Timespec { tv_sec: t, tv_nsec: 0 }
}

extern "C" {
    fn utimensat(dirfd: i32, path: *const std::os::raw::c_char, times: *const Timespec, flags: i32) -> i32;
}

#[test]
fn drop_unused_keeps_the_registered_reading_and_the_start_files() {
    let tmp = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("drop-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let adir = tmp.join("a00");
    for name in ["refine-01", "refine-02", "reach-01", "plans"] {
        let d = adir.join(name);
        std::fs::create_dir_all(&d).unwrap();
        for f in ["reading.json", "reading.tc", "reading.actions.jsonl", "start.txt", "start.jsonl"] {
            std::fs::write(d.join(f), vec![b'x'; 10]).unwrap();
        }
    }
    let made = json!({"path": adir.join("refine-01").join("reading.json").to_string_lossy(), "scope": [28, 33], "kind": "refine"});
    assert_eq!(step::drop_unused(&adir, Some(&made)), vec![json!(["reach-01", 20]), json!(["refine-02", 20])]);
    for name in ["refine-02", "reach-01"] {
        let mut ns: Vec<String> = std::fs::read_dir(adir.join(name)).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        ns.sort();
        assert_eq!(ns, vec!["reading.json", "start.jsonl", "start.txt"]);
    }
    assert_eq!(std::fs::read_dir(adir.join("refine-01")).unwrap().count(), 5);
    assert_eq!(std::fs::read_dir(adir.join("plans")).unwrap().count(), 5);
    assert_eq!(step::drop_unused(&adir, None), vec![json!(["reach-01", 0]), json!(["refine-01", 20]), json!(["refine-02", 0])]);
    assert!(step::drop_unused(&tmp.join("none"), None).is_empty());
}

// --- test_e009.py: (a) the idle rule, (b) rotation ------------------------------------------------------------------

#[test]
fn idle_rule_a_value_coming_back_is_idle_a_one_shot_change_is_not() {
    let start = fact_map(&json!({"var:100": 5, "room": 33, "var:2": 0, "state:316": 0}));
    let frames = vec![
        (1, fact_map(&json!({"var:100": 6, "room": 33, "var:2": 1, "state:316": 0}))),
        (2, fact_map(&json!({"var:100": 7, "room": 28, "var:2": 2, "state:316": 1}))),
        (3, fact_map(&json!({"var:100": 5, "room": 28, "var:2": 3, "state:316": 1}))),
        (4, fact_map(&json!({"var:100": 6, "room": 28, "var:2": 3, "state:316": 0}))),
    ];
    let r = step::idle_rule(&start, frames);
    assert_eq!(r["returns"], json!(["state:316", "var:100"]));
    assert_eq!(r["first_return"], json!({"var:100": 3, "state:316": 4}));
    let mut ch: Vec<(String, i64)> = r["changes"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_i64().unwrap())).collect();
    ch.sort();
    assert_eq!(ch, vec![("room".into(), 1), ("state:316".into(), 2), ("var:100".into(), 4), ("var:2".into(), 3)]);
}

#[test]
fn idle_rule_absent_is_zero_and_a_key_that_appears_and_goes_returns() {
    let r = step::idle_rule(&fact_map(&json!({"var:1": 0})), vec![(1, fact_map(&json!({"var:1": 0, "bit:9": 1}))), (2, fact_map(&json!({"var:1": 0})))]);
    assert_eq!(r["returns"], json!(["bit:9"]));
    assert_eq!(r["first_return"], json!({"bit:9": 2}));
}

#[test]
fn rotate_least_tried_in_the_cell_then_in_all_then_depth_then_e008s_order() {
    let ts = vec![t("state:316", 1, 0, 0), t("bit:9", 1, 1, 1), t("bit:1", 1, 1, 2), t("room", 28, 1, 2), t("bit:5", 1, 2, 2)];
    let rot = json!({"cell": [["state:316", 1, 1], ["bit:1", 1, 1]], "all": [["bit:9", 1, 3], ["state:316", 1, 1], ["bit:1", 1, 4]]});
    let kv = |v: Vec<Target>| v.into_iter().map(|x| format!("{}={:?}", x.k, x.v)).collect::<Vec<_>>();
    assert_eq!(kv(step::rotate(ts.clone(), &rot)), kv(vec![t("room", 28, 0, 0), t("bit:5", 1, 0, 0), t("bit:9", 1, 0, 0), t("state:316", 1, 0, 0), t("bit:1", 1, 0, 0)]));
    assert_eq!(kv(step::rotate(ts, &json!({}))), kv(vec![t("state:316", 1, 0, 0), t("bit:9", 1, 0, 0), t("bit:1", 1, 0, 0), t("room", 28, 0, 0), t("bit:5", 1, 0, 0)]));
}

#[test]
fn targets_with_rotation_are_e008s_targets_reordered() {
    let plain = targets(&entry(), None, json!([["state:316", 1]]), json!([["room", 41]]), &goal(), None);
    let rot = json!({"cell": [["owner:566", 1, 2]], "all": [["owner:566", 1, 2], ["bit:6", 1, 1]]});
    let turned = targets(&entry(), None, json!([["state:316", 1]]), json!([["room", 41]]), &goal(), Some(&rot));
    let key = |x: &Target| format!("{}={:?}", x.k, x.v);
    let mut a: Vec<String> = plain.iter().map(key).collect();
    let mut b: Vec<String> = turned.iter().map(key).collect();
    a.sort();
    b.sort();
    assert_eq!(a, b);
    assert_ne!(plain, turned);
    assert_eq!((turned.last().unwrap().k.as_str(), &turned.last().unwrap().v), ("owner:566", &TV::Int(1)));
    assert!(!turned.iter().any(|x| x.k == "room" && x.v == TV::Int(41)));
}

#[test]
fn start_row_text_is_start_rows_json() {
    // a dump made from a recorded checkpoint (the doubles' inverse of facts_of), its row written both ways
    let home = std::env::var("HOME").unwrap_or_default();
    let Ok(j) = fmi_explore::files::read_json(&std::path::PathBuf::from(format!("{home}/.cache/fast-mi-e009/off/step-a/jobs/ta/job.json"))) else { return };
    let facts = fmi_explore::start::facts_of_json(&j["entry"]["checkpoint"][1]);
    let mut obs = fmi_explore::doubles::obs_from_facts(&facts);
    obs.ego_pos = Some((346, 133));
    let st = fmi_core::Status { reason: "idle".into(), frame: 2728, tick: 16321, qf: 65284, room: 33, idle: Some("sentence".into()), ego: Some((346, 133)), ..Default::default() };
    let a = step::start_row_text(1, &obs, &st).unwrap();
    let b = fmi_explore::pyjson::dumps_default(&step::start_row(1, &obs, &st).unwrap());
    assert_eq!(a, b);
    assert_eq!(fmi_core::facts_of(&obs), facts);
}

// --- E011 round 2: the idle rule on the facts' changes ------------------------------------------------------------

/// A deterministic sequence of facts maps (an LCG): keys appear, go, and change among a few values, most frames
/// changing a few keys of many, as an idle entry's frames do.
fn lcg_frames(seed: u64, n_keys: u64, frames: usize) -> (fmi_core::Facts, Vec<(i64, fmi_core::Facts)>) {
    let mut x = seed;
    let mut next = move || {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        x >> 33
    };
    let key = |n: u64| match n % 5 {
        0 => fmi_core::Key::var(n),
        1 => fmi_core::Key::bit(n),
        2 => fmi_core::Key::owner(n),
        3 => fmi_core::Key::state(n),
        _ => fmi_core::Key::class(n, 1 + n % 7),
    };
    let mut cur: Vec<(fmi_core::Key, i64)> = (0..n_keys).filter(|n| n % 3 != 0).map(|n| (key(n), (n % 4) as i64)).collect();
    let start = fmi_core::Facts::from_pairs(cur.clone());
    let mut out = Vec::new();
    for i in 1..=frames {
        for _ in 0..(next() % 6) {
            let n = next() % n_keys;
            let v = (next() % 4) as i64 - 1;
            match cur.iter().position(|p| p.0 == key(n)) {
                Some(p) if next() % 5 == 0 => {
                    cur.remove(p);
                }
                Some(p) => cur[p].1 = v,
                None => cur.push((key(n), v)),
            }
        }
        out.push((i as i64, fmi_core::Facts::from_pairs(cur.clone())));
    }
    (start, out)
}

fn idle_by_changes(start: &fmi_core::Facts, frames: &[(i64, fmi_core::Facts)]) -> Value {
    let mut rule = step::IdleRule::new();
    let mut prev = start.clone();
    let mut ch = Vec::new();
    for (off, f) in frames {
        ch.clear();
        step::diff_into(&prev, f, &mut ch);
        rule.frame(*off, &ch);
        prev = f.clone();
    }
    rule.result()
}

#[test]
fn diff_into_is_facts_diff() {
    for seed in 1..40 {
        let (start, frames) = lcg_frames(seed, 60, 30);
        let mut prev = start;
        for (_, f) in frames {
            let mut ch = Vec::new();
            step::diff_into(&prev, &f, &mut ch);
            assert_eq!(ch, prev.diff(&f));
            let mut back = Vec::new();
            step::diff_into(&f, &prev, &mut back);
            assert_eq!(back, f.diff(&prev));
            prev = f;
        }
    }
    let e = fmi_core::Facts::new();
    let (start, _) = lcg_frames(3, 20, 0);
    let mut ch = Vec::new();
    step::diff_into(&e, &start, &mut ch);
    assert_eq!(ch, e.diff(&start));
    ch.clear();
    step::diff_into(&start, &e, &mut ch);
    assert_eq!(ch, start.diff(&e));
}

#[test]
fn idle_rule_on_changes_is_idle_rule_facts_maps_in_the_same_order() {
    for seed in 1..60 {
        let (start, frames) = lcg_frames(seed, 40 + seed % 30, 200);
        let a = step::idle_rule_facts(&start, frames.clone());
        let b = idle_by_changes(&start, &frames);
        assert_eq!(fmi_explore::pyjson::dumps_default(&a), fmi_explore::pyjson::dumps_default(&b), "seed {seed}");
    }
    // the two cases of test_e009.py
    let f = |v: Value| fmi_explore::start::facts_of_json(&v);
    let start = f(json!({"var:100": 5, "room": 33, "var:2": 0, "state:316": 0}));
    let frames = vec![
        (1, f(json!({"var:100": 6, "room": 33, "var:2": 1, "state:316": 0}))),
        (2, f(json!({"var:100": 7, "room": 28, "var:2": 2, "state:316": 1}))),
        (3, f(json!({"var:100": 5, "room": 28, "var:2": 3, "state:316": 1}))),
        (4, f(json!({"var:100": 6, "room": 28, "var:2": 3, "state:316": 0}))),
    ];
    let r = idle_by_changes(&start, &frames);
    assert_eq!(r["returns"], json!(["state:316", "var:100"]));
    assert_eq!(r, step::idle_rule_facts(&start, frames));
    let start = f(json!({"var:1": 0}));
    let frames = vec![(1, f(json!({"var:1": 0, "bit:9": 1}))), (2, f(json!({"var:1": 0})))];
    let r = idle_by_changes(&start, &frames);
    assert_eq!(r["returns"], json!(["bit:9"]));
    assert_eq!(r["first_return"], json!({"bit:9": 2}));
    assert_eq!(r, step::idle_rule_facts(&start, frames));
}
