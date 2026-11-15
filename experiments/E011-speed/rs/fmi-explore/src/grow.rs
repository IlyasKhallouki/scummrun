//! E014 amendment (4) (b) (experiments/E014-fate-demo/plan.md): scopes grown as needed (`FMI_SCOPE_GROW=1`). The scope
//! of an entry in room R is {R} ∪ G(R), cut to ring(R); G(R), the archive's rooms grown for R, grows by a room of the
//! ring outside the scope when a target needs one (no attempt of a model try played; the guide's atoms the scope's
//! reading does not reach whose writers lie in that room only), when a plan needs one (an attempt stopped outside the
//! task's domains with the engine in that room), or when the model-free step leaves the room for it (an entry kept by
//! the wait, an affordance step or a free try). The try names the first two (result.json "scope_need"); the archive
//! adds them with the third at the commit and writes the growth in the log line ("grow"), which a reopened archive
//! replays.
//!
//! A scope is still a set of rooms read exactly: the reader reads every unit of its rooms and the global scripts from
//! the entry's exact state; the readings are registered and reused by their exact scope.

use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::OnceLock;

pub const GROW_ENV: &str = "FMI_SCOPE_GROW";

/// `FMI_SCOPE_GROW=1`.
pub fn on() -> bool {
    std::env::var(GROW_ENV).is_ok_and(|v| v == "1")
}

/// `FMI_SCOPE_GROW=room` (E016 plan F, amendment (8)): a room's reading is that room, and it never grows; a room outside
/// the ring keeps E009's rule, as `scope` has it.
pub fn room_only() -> bool {
    std::env::var(GROW_ENV).is_ok_and(|v| v == "room")
}

/// The scope of room `room`: {room} ∪ `grown`, cut to `ring` (the guide's ring, cut to the allowed rooms); the ring
/// itself when the room is not in it (a room outside the allowed ones keeps E009's rule).
pub fn scope(room: i64, ring: &[i64], grown: Option<&BTreeSet<i64>>) -> Vec<i64> {
    if !ring.contains(&room) {
        return ring.to_vec();
    }
    let mut s: BTreeSet<i64> = BTreeSet::from([room]);
    if let Some(g) = grown {
        s.extend(g.iter().copied().filter(|r| ring.contains(r)));
    }
    s.into_iter().collect()
}

/// Where a writer runs: every scope reads the global scripts; a room's local, entry and exit scripts and its objects'
/// code are that room's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Global,
    Room(i64),
    Unknown,
}

/// A guide's unit name (`global:N`, `local:R:N`, `entry:R`, `exit:R`, `obj:N`) and its place; `objs`: object -> room.
pub fn place(unit: &str, objs: &HashMap<i64, i64>) -> Place {
    let mut it = unit.split(':');
    let kind = it.next().unwrap_or("");
    let n: Option<i64> = it.next().and_then(|x| x.parse().ok());
    match (kind, n) {
        ("global", _) => Place::Global,
        ("local" | "entry" | "exit", Some(r)) => Place::Room(r),
        ("obj", Some(o)) => objs.get(&o).map(|r| Place::Room(*r)).unwrap_or(Place::Unknown),
        _ => Place::Unknown,
    }
}

/// The objects' rooms in an extraction's index.json ("objects": {N: {"room": R}}).
pub fn obj_rooms_of(index: &Value) -> HashMap<i64, i64> {
    let mut m = HashMap::new();
    if let Some(o) = index["objects"].as_object() {
        for (k, v) in o {
            if let (Ok(n), Some(r)) = (k.parse::<i64>(), v["room"].as_i64()) {
                m.insert(n, r);
            }
        }
    }
    m
}

/// The game's objects' rooms, from its extraction's index (the game's own data: `FMI_GAME_SCRIPTS`'s, else Monkey
/// Island's), read once per process; empty when it cannot be read.
pub fn obj_rooms() -> &'static HashMap<i64, i64> {
    static M: OnceLock<HashMap<i64, i64>> = OnceLock::new();
    M.get_or_init(|| {
        let p = scumm_ir::game_index(&crate::guide::default_root());
        crate::files::read_json(&p).map(|v| obj_rooms_of(&v)).unwrap_or_default()
    })
}

/// The room a try names (result.json "scope_need").
#[derive(Clone, Debug, PartialEq)]
pub struct Need {
    pub room: i64,
    /// "target" or "plan"
    pub why: &'static str,
    /// the atom that needs it (target) and its depth, and how many atoms at that depth need it
    pub atom: Option<String>,
    pub depth: Option<i64>,
    pub atoms: usize,
}

impl Need {
    pub fn json(&self) -> Value {
        let mut v = json!({"room": self.room, "why": self.why});
        if let Some(a) = &self.atom {
            v["atom"] = json!(a);
            v["depth"] = json!(self.depth);
            v["atoms"] = json!(self.atoms);
        }
        v
    }
}

/// Case 1, a target needs a room: of the guide's atoms ([key, value or null, depth]) not `held` at the entry and not
/// `reached` by the scope's reading, those with no writer global or in the scope and one in a room of `ring` outside
/// `scope`; the room of the shallowest, ties by the most such atoms at that depth, then the lower room. None when no
/// atom needs one.
pub fn target_need(guide: &Value, scope: &[i64], ring: &[i64], held: &dyn Fn(&str, Option<i64>) -> bool,
                   reached: &dyn Fn(&str, Option<i64>) -> bool, objs: &HashMap<i64, i64>) -> Option<Need> {
    let outside: BTreeSet<i64> = ring.iter().copied().filter(|r| !scope.contains(r)).collect();
    if outside.is_empty() {
        return None;
    }
    // (depth, room) -> (atoms, the first atom's text)
    let mut by: BTreeMap<(i64, i64), (usize, String)> = BTreeMap::new();
    for a in guide["atoms"].as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
        let (Some(k), Some(d)) = (a.get(0).and_then(|k| k.as_str()), a.get(2).and_then(|d| d.as_i64())) else { continue };
        let v = a.get(1).and_then(|v| v.as_i64());
        if held(k, v) || reached(k, v) {
            continue;
        }
        let text = format!("{k}={}", v.map(|v| v.to_string()).unwrap_or_else(|| "null".into()));
        let Some(ws) = guide["writers"].get(&text).and_then(|w| w.as_array()) else { continue };
        let places: Vec<Place> = ws.iter().filter_map(|u| u.as_str()).map(|u| place(u, objs)).collect();
        if places.iter().any(|p| match p {
            Place::Global => true,
            Place::Room(r) => scope.contains(r),
            Place::Unknown => false,
        }) {
            continue;
        }
        let rooms: BTreeSet<i64> = places.iter().filter_map(|p| match p {
            Place::Room(r) if outside.contains(r) => Some(*r),
            _ => None,
        }).collect();
        for r in rooms {
            let e = by.entry((d, r)).or_insert((0, text.clone()));
            e.0 += 1;
        }
    }
    let d0 = by.keys().map(|(d, _)| *d).min()?;
    let (room, (n, atom)) = by
        .iter()
        .filter(|((d, _), _)| *d == d0)
        .map(|((_, r), x)| (*r, x.clone()))
        .fold(None, |best: Option<(i64, (usize, String))>, (r, x)| match best {
            Some(b) if b.1 .0 >= x.0 => Some(b),
            _ => Some((r, x)),
        })?;
    Some(Need { room, why: "target", atom: Some(atom), depth: Some(d0), atoms: n })
}

/// Case 2, a plan needs a room: an attempt's stop outside the task's domains, with the engine in `room`, a room of
/// `ring` outside `scope`.
pub fn plan_need(stop: Option<&str>, room: Option<i64>, scope: &[i64], ring: &[i64]) -> Option<Need> {
    let s = stop?;
    if !s.starts_with("the engine's state is outside the task's domains") {
        return None;
    }
    let r = room?;
    (ring.contains(&r) && !scope.contains(&r)).then_some(Need { room: r, why: "plan", atom: None, depth: None, atoms: 0 })
}

/// The kinds of entry made by the model-free step (case 3).
pub const MODEL_FREE_HOWS: [&str; 3] = ["wait", "afford", "free"];

/// The growth at a commit: the try's need (`need`: result.json "scope_need") and the rooms of its model-free entries
/// (`left`: (how, room) of each new entry), each kept when it is a room of `ring` outside `scope`; (room, why) in the
/// order found, each room once.
pub fn growth(need: Option<&Value>, left: &[(String, Option<i64>)], scope: &[i64], ring: &[i64]) -> Vec<(i64, String)> {
    let mut out: Vec<(i64, String)> = Vec::new();
    let mut add = |r: i64, why: &str| {
        if ring.contains(&r) && !scope.contains(&r) && !out.iter().any(|x| x.0 == r) {
            out.push((r, why.to_string()));
        }
    };
    if let Some(n) = need {
        if let (Some(r), Some(w)) = (n["room"].as_i64(), n["why"].as_str()) {
            add(r, w);
        }
    }
    for (how, room) in left {
        if let (true, Some(r)) = (MODEL_FREE_HOWS.contains(&how.as_str()), room) {
            add(*r, "left");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn objs() -> HashMap<i64, i64> {
        HashMap::from([(582, 48), (1034, 10), (900, 49)])
    }

    #[test]
    fn scope_of_a_room() {
        let ring = vec![24, 42, 48, 49, 63];
        assert_eq!(scope(49, &ring, None), vec![49]);
        assert_eq!(scope(49, &ring, Some(&BTreeSet::from([48, 99]))), vec![48, 49]);
        // a room outside the ring keeps the ring
        assert_eq!(scope(7, &ring, Some(&BTreeSet::from([48]))), ring);
    }

    #[test]
    fn places() {
        let o = objs();
        assert_eq!(place("global:57", &o), Place::Global);
        assert_eq!(place("local:48:210", &o), Place::Room(48));
        assert_eq!(place("entry:63", &o), Place::Room(63));
        assert_eq!(place("exit:82", &o), Place::Room(82));
        assert_eq!(place("obj:582", &o), Place::Room(48));
        assert_eq!(place("obj:1", &o), Place::Unknown);
        let idx = json!({"objects": {"20": {"room": 68}, "x": {"room": 1}, "21": {}}});
        assert_eq!(obj_rooms_of(&idx), HashMap::from([(20, 68)]));
    }

    fn guide() -> Value {
        json!({
            "atoms": [["bit:107", null, 1], ["state:582", 1, 1], ["var:367", 1, 1], ["var:9", 2, 2], ["var:8", 3, 2],
                      ["var:7", 4, 1], ["var:6", 5, 1]],
            "writers": {
                "bit:107=null": ["global:1"],
                "state:582=1": ["local:48:210", "obj:582"],
                "var:367=1": ["local:49:200", "local:63:201"],
                "var:9=2": ["local:63:201"],
                "var:8=3": ["obj:1034"],
                "var:7=4": ["entry:63"],
                "var:6=5": ["local:63:209", "exit:42"]
            }
        })
    }

    #[test]
    fn the_shallowest_atom_names_the_room() {
        let g = guide();
        let o = objs();
        let ring = vec![24, 42, 48, 49, 63];
        let no = |_: &str, _: Option<i64>| false;
        // depth 1: state:582=1 (48), var:7=4 (63), var:6=5 (63, 42): room 63 has two atoms
        let n = target_need(&g, &[49], &ring, &no, &no, &o).unwrap();
        assert_eq!((n.room, n.why, n.depth, n.atoms), (63, "target", Some(1), 2));
        // var:367=1 has a writer in the scope; bit:107 a global one: neither counts
        // with 63 in the scope, var:6=5 has a writer there: 48 remains at depth 1
        let n = target_need(&g, &[49, 63], &ring, &no, &no, &o).unwrap();
        assert_eq!((n.room, n.atom.as_deref()), (48, Some("state:582=1")));
        // held or reached atoms need nothing
        let held = |k: &str, _: Option<i64>| k == "state:582";
        let reached = |k: &str, v: Option<i64>| k == "var:7" && v == Some(4) || k == "var:6";
        let n = target_need(&g, &[49], &ring, &held, &reached, &o).unwrap();
        // depth 1 left: none; depth 2: var:9=2 (63); var:8=3's object is in room 10, outside the ring
        assert_eq!((n.room, n.depth, n.atoms), (63, Some(2), 1));
        // no room outside the scope: none
        assert_eq!(target_need(&g, &ring, &ring, &no, &no, &o), None);
    }

    #[test]
    fn ties_go_to_the_lower_room() {
        let g = json!({"atoms": [["a", 1, 1], ["b", 1, 1]], "writers": {"a=1": ["local:63:1"], "b=1": ["local:48:1"]}});
        let no = |_: &str, _: Option<i64>| false;
        let n = target_need(&g, &[49], &[48, 49, 63], &no, &no, &HashMap::new()).unwrap();
        assert_eq!(n.room, 48);
    }

    #[test]
    fn a_plan_outside_the_domains() {
        let ring = [48, 49, 63];
        let s = Some("the engine's state is outside the task's domains (3 facts)");
        assert_eq!(plan_need(s, Some(48), &[49], &ring).map(|n| n.room), Some(48));
        assert_eq!(plan_need(s, Some(49), &[49], &ring), None);
        assert_eq!(plan_need(s, Some(10), &[49], &ring), None);
        assert_eq!(plan_need(Some("goal"), Some(48), &[49], &ring), None);
    }

    #[test]
    fn the_growth_at_a_commit() {
        let ring = [42, 48, 49, 63];
        let need = json!({"room": 48, "why": "target"});
        let left = vec![("free".to_string(), Some(63)), ("checkpoint".to_string(), Some(42)), ("wait".to_string(), Some(49)),
                        ("afford".to_string(), Some(48)), ("afford".to_string(), Some(10))];
        assert_eq!(growth(Some(&need), &left, &[49], &ring), vec![(48, "target".to_string()), (63, "left".to_string())]);
        // a room the scope holds already: nothing
        assert_eq!(growth(Some(&need), &[], &[48, 49], &ring), vec![]);
    }
}
