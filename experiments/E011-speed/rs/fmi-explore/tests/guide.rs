//! test_guide.py, ported (the tests that do not compare against Python's trace_explain.Code, whose agreement G9's
//! byte comparison with guide.py covers): from the game files alone, no engine, no reading.

use fmi_explore::guide::{self, Code, WMap, AK};
use scumm_ir::{Game, Stmt, UnitKey};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::OnceLock;

const K2: [i64; 10] = [28, 29, 33, 34, 35, 38, 41, 83, 85, 96];

fn root() -> PathBuf {
    guide::default_root()
}

fn e003() -> PathBuf {
    root().join("experiments").join("E003-path-through-cloud")
}

fn have_game() -> bool {
    root().join("data").join("scripts-dos-cd").join("index.json").exists()
}

fn game() -> &'static Game {
    static G: OnceLock<Game> = OnceLock::new();
    G.get_or_init(|| guide::load_game(&root()).unwrap())
}

fn start() -> HashMap<String, i64> {
    guide::start_row(&e003(), 1).unwrap()
}

fn map_of(goal: &str, wide: bool) -> Value {
    let g = guide::parse_goal(goal, &e003()).unwrap();
    guide::build(game(), &g, wide, &start(), &mut || 0.0).unwrap()
}

fn door() -> &'static Value {
    static D: OnceLock<Value> = OnceLock::new();
    D.get_or_init(|| map_of("state:316=1", false))
}

macro_rules! need_game {
    () => {
        if !have_game() {
            eprintln!("skipped: no game files");
            return;
        }
    };
}

#[test]
fn door_writers_and_depths() {
    need_game!();
    let g = door();
    assert_eq!((g["format"].as_str(), g["map"].as_str()), (Some("e008-guide-1"), Some("direct")));
    assert_eq!(g["goal"], json!({"state:316": [1]}));
    let w: Vec<&str> = g["writers"]["state:316=1"].as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
    assert!(w.contains(&"local:28:205") && w.contains(&"local:28:209"));
    let atoms = g["atoms"].as_array().unwrap();
    assert!(atoms.contains(&json!(["state:316", 1, 0])) && atoms.contains(&json!(["room", 28, 1])));
    assert_eq!(guide::depth(g, "room", Some(28)), Some(1));
    assert_eq!(guide::depth(g, "state:316", Some(1)), Some(0));
    let s = &g["stats"];
    assert_eq!(s["atoms"].as_u64().unwrap() as usize, atoms.len());
    assert_eq!(s["keys"].as_u64().unwrap() as usize, g["keys"].as_object().unwrap().len());
    assert_eq!(s["depth_hist"].as_object().unwrap().values().map(|v| v.as_u64().unwrap()).sum::<u64>(), s["atoms"].as_u64().unwrap());
    assert!(s["wide"]["atoms"].as_u64() >= s["atoms"].as_u64());
    let order: Vec<(i64, String, String)> = atoms.iter().map(|a| (a[2].as_i64().unwrap(), a[0].as_str().unwrap().to_string(),
        if a[1].is_null() { "None".into() } else { a[1].to_string() })).collect();
    let mut sorted = order.clone();
    sorted.sort();
    assert_eq!(order, sorted);
    let mut least = serde_json::Map::new();
    for a in atoms {
        let k = a[0].as_str().unwrap().to_string();
        if !least.contains_key(&k) {
            least.insert(k, a[2].clone());
        }
    }
    assert_eq!(g["keys"], Value::Object(least));
}

#[test]
fn door_depth_is_one_more_than_its_least_source() {
    need_game!();
    let g = door();
    let mut code = Code::new(game());
    let mut m = WMap::new(&mut code, start(), false);
    let depth: HashMap<(AK, Option<i32>), i64> = g["atoms"].as_array().unwrap().iter()
        .map(|a| ((AK::parse(a[0].as_str().unwrap()).unwrap(), a[1].as_i64().map(|v| v as i32)), a[2].as_i64().unwrap())).collect();
    let mut best: HashMap<(AK, Option<i32>), i64> = HashMap::new();
    for (&a, &d) in &depth {
        for (k, sites) in m.writers(a) {
            let u = m.unit_of(k).unwrap();
            for i in sites {
                for b in m.needs(u, i) {
                    let e = best.entry(b).or_insert(d + 1);
                    *e = (*e).min(d + 1);
                }
            }
        }
    }
    assert!(best.keys().all(|b| depth.contains_key(b)));
    for (a, d) in &depth {
        if *d > 0 {
            assert_eq!(best.get(a), Some(d), "{a:?}");
        }
    }
}

#[test]
fn door_same_bytes() {
    need_game!();
    let a = guide::dumps(door());
    let b = guide::dumps(&map_of("state:316=1", false));
    assert_eq!(a, b);
}

#[test]
fn door_writers_name_units() {
    need_game!();
    let g = door();
    let names: HashSet<String> = game().script.units.iter().map(|u| guide::unit_name(u.key)).collect();
    for (atom, units) in g["writers"].as_object().unwrap() {
        let us: Vec<String> = units.as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect();
        let mut s = us.clone();
        s.sort();
        assert_eq!(us, s, "{atom}");
        assert!(us.iter().all(|u| names.contains(u)), "{atom}");
    }
    let atom_str = |k: &Value, v: &Value| format!("{}={}", k.as_str().unwrap(), if v.is_null() { "null".into() } else { v.to_string() });
    let no: HashSet<String> = g["stats"]["no_writer"].as_array().unwrap().iter().map(|x| atom_str(&x[0], &x[1])).collect();
    let ws: HashSet<String> = g["writers"].as_object().unwrap().keys().cloned().collect();
    assert!(no.is_disjoint(&ws));
    let all: HashSet<String> = g["atoms"].as_array().unwrap().iter().map(|x| atom_str(&x[0], &x[1])).collect();
    assert_eq!(no.union(&ws).cloned().collect::<HashSet<_>>(), all);
}

#[test]
fn kitchen_room_41() {
    need_game!();
    let g = map_of("owner:566=1", false);
    let d = guide::depth(&g, "room", Some(41)).unwrap();
    assert!(g["atoms"].as_array().unwrap().contains(&json!(["room", 41, d])));
    assert!(g["atoms"].as_array().unwrap().contains(&json!(["owner:566", 1, 0])));
}

#[test]
fn rings_and_rooms() {
    need_game!();
    let g = door();
    assert_eq!(guide::ring(g, 33, Some(&K2)), vec![28, 33, 35, 38]);
    assert_eq!(guide::ring(g, 28, Some(&K2)), vec![28, 33, 41]);
    assert_eq!(g["rooms"]["start"], json!(33));
    assert_eq!(g["rooms"]["reached"].as_array().unwrap().len(), 80);
}

#[test]
fn part1() {
    need_game!();
    let g = map_of("part1", false);
    assert!(g["stats"]["atoms"].as_u64().unwrap() > 0);
    for o in [901, 902, 903] {
        assert!(g["atoms"].as_array().unwrap().contains(&json!([format!("owner:{o}"), 1, 0])));
    }
}

#[test]
fn load_depth_ring_rules() {
    let hand = json!({"format": "e008-guide-1", "goal": {"state:1": [1]}, "map": "direct",
        "rooms": {"start": 5, "edges": {"5": [6, 7], "6": [5], "7": []}, "reached": [5, 6, 7]},
        "atoms": [["state:1", 1, 0], ["room", 5, 1], ["var:2", null, 2], ["var:2", 3, 1], ["bit:4", 0, 3]],
        "keys": {"state:1": 0, "room": 1, "var:2": 1, "bit:4": 3}, "writers": {"state:1=1": ["local:5:200"]},
        "stats": {"atoms": 5, "keys": 4, "depth_hist": {"0": 1, "1": 2, "2": 1, "3": 1}, "units": 1, "seconds": 0.0,
                  "no_writer": [["room", 5]], "wide": {"atoms": 5, "keys": 4}}});
    let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("guide-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::write(tmp.join("g.json"), hand.to_string()).unwrap();
    let g = guide::load(&tmp.join("g.json")).unwrap();
    assert_eq!(guide::depth(&g, "state:1", Some(1)), Some(0));
    assert_eq!(guide::depth(&g, "state:1", Some(0)), None);
    assert_eq!(guide::depth(&g, "var:2", Some(3)), Some(1));
    assert_eq!(guide::depth(&g, "var:2", Some(9)), Some(2));
    assert_eq!(guide::depth(&g, "bit:4", Some(0)), Some(3));
    assert_eq!(guide::depth(&g, "bit:4", Some(1)), None);
    assert_eq!(guide::depth(&g, "var:9", Some(0)), None);
    assert_eq!(guide::ring(&g, 5, None), vec![5, 6, 7]);
    assert_eq!(guide::ring(&g, 5, Some(&[5, 7, 9])), vec![5, 7]);
    assert!(guide::ring(&g, 7, Some(&[5, 6])).is_empty());
    assert_eq!(guide::ring(&g, 6, Some(&[5, 6])), vec![5, 6]);
    assert_eq!(guide::ring(&g, 8, None), vec![8]);
    let mut bad = hand.clone();
    bad["format"] = json!("other");
    std::fs::write(tmp.join("bad.json"), bad.to_string()).unwrap();
    assert!(guide::load(&tmp.join("bad.json")).is_err());
}

#[test]
fn parse_goal() {
    let e = e003();
    assert_eq!(guide::parse_goal("state:316=1", &e).unwrap(), vec![("state:316".to_string(), vec![1])]);
    assert_eq!(guide::parse_goal("owner:566=1,state:316=1,state:316=0", &e).unwrap(),
               vec![("owner:566".to_string(), vec![1]), ("state:316".to_string(), vec![0, 1])]);
    if e.join("results").join("end-part1.json").exists() {
        assert_eq!(guide::parse_goal("part1", &e).unwrap(),
                   vec![("owner:901".to_string(), vec![1]), ("owner:902".to_string(), vec![1]), ("owner:903".to_string(), vec![1])]);
    }
}

#[test]
fn registers_followed_to_their_writes() {
    need_game!();
    let mut code = Code::new(game());
    let mut m = WMap::new(&mut code, start(), false);
    let u = m.unit_of(UnitKey::Local(28, 209)).unwrap();
    let unit = &game().script.units[u];
    let js: Vec<u32> = game().script.ins[unit.range()].iter().enumerate()
        .filter(|(_, x)| matches!(x.st, Some(Stmt::Jump { cond: Some(_), .. }))).map(|(j, _)| j as u32).collect();
    assert!(!js.is_empty());
    for j in js {
        assert!(!m.cond_atoms_of(u, j).contains(&(AK::Var(0), Some(1))));
    }
    assert_eq!(m.start_needs[u], HashSet::from([(AK::Room, Some(28))]));
}

#[test]
fn stats_cli_with_the_key() {
    need_game!();
    let key = root().join("experiments").join("E001-goals-from-scripts").join("key.json");
    if !key.exists() {
        eprintln!("skipped: no key.json");
        return;
    }
    let out = guide::stats_text(door(), Some(&key)).unwrap();
    assert!(out.contains("|R|") && out.contains("depth histogram") && out.contains("wide map"));
    let line = out.lines().find(|l| l.starts_with("key (evaluation only)")).unwrap();
    let nums: Vec<i64> = line.split(|c: char| !c.is_ascii_digit()).filter(|s| !s.is_empty()).map(|s| s.parse().unwrap()).collect();
    assert_eq!(nums[0], 99);
    assert!(nums[2] <= nums[1]);
}
