//! golden.py's canonical encoding (`enc`) of the machine's values, frames, paths and runs, as
//! [`J`] values that print as golden.py's lines (keys sorted, compact, ASCII only).
//!
//! Python types map as golden.py's FORMAT.txt says: a tuple is {"$tuple": [...]}, a set {"$set": [...]}
//! in the order rule's order, a dict with str keys an object, any other dict {"$map": [[k, v], ...]}
//! (an empty dict is {}), a dataclass {"$Name": {field: ...}}.

use crate::path::{Assumption, Call, Entry, Frame, Owned, Owner, Path, Run, Side, Step, Tri, NONE_UNIT};
use crate::value::{Val, Vals};
use scumm_ir::json::J;
use scumm_ir::{Fact, Game, Slot, UnitKey};
use std::collections::BTreeMap;

/// golden.py `_text`: one value's compact canonical JSON.
pub fn text(j: &J) -> String {
    let mut s = String::new();
    j.write(&mut s);
    s
}

/// golden.py `_order` on encoded values: all ints numeric, all strs by code point, else by their text.
pub fn order(mut items: Vec<J>) -> Vec<J> {
    if items.iter().all(|j| matches!(j, J::Int(_))) {
        items.sort_by_key(|j| match j {
            J::Int(v) => *v,
            _ => unreachable!(),
        });
    } else if items.iter().all(|j| matches!(j, J::Str(_))) {
        items.sort_by(|a, b| match (a, b) {
            (J::Str(x), J::Str(y)) => x.cmp(y),
            _ => unreachable!(),
        });
    } else {
        let mut keyed: Vec<(String, J)> = items.into_iter().map(|j| (text(&j), j)).collect();
        keyed.sort_by(|a, b| a.0.cmp(&b.0));
        items = keyed.into_iter().map(|p| p.1).collect();
    }
    items
}

pub fn tuple(items: Vec<J>) -> J {
    one("$tuple", J::Arr(items))
}

pub fn set(items: Vec<J>) -> J {
    one("$set", J::Arr(order(items)))
}

/// A dict whose keys are not all str: {"$map": [[k, v], ...]} by the order rule; {} when empty.
pub fn map(pairs: Vec<(J, J)>) -> J {
    if pairs.is_empty() {
        return J::Obj(BTreeMap::new());
    }
    let mut keyed: Vec<(J, J)> = pairs;
    let keys = order(keyed.iter().map(|p| p.0.clone()).collect());
    let mut out = Vec::with_capacity(keys.len());
    for k in keys {
        let t = text(&k);
        let pos = keyed.iter().position(|p| text(&p.0) == t).unwrap();
        let (k, v) = keyed.swap_remove(pos);
        out.push(J::Arr(vec![k, v]));
    }
    one("$map", J::Arr(out))
}

/// A dict with str keys: an object.
pub fn obj(pairs: impl IntoIterator<Item = (String, J)>) -> J {
    J::Obj(pairs.into_iter().collect())
}

/// A dataclass instance: {"$Name": {fields}}.
pub fn dataclass(name: &str, fields: Vec<(&str, J)>) -> J {
    one(&format!("${name}"), J::Obj(fields.into_iter().map(|(k, v)| (k.to_string(), v)).collect()))
}

fn one(k: &str, v: J) -> J {
    let mut m = BTreeMap::new();
    m.insert(k.to_string(), v);
    J::Obj(m)
}

pub fn s(x: &str) -> J {
    J::Str(x.to_string())
}

pub fn opt_s(x: Option<&str>) -> J {
    x.map_or(J::Null, s)
}

pub fn fact(k: Fact) -> J {
    J::Str(k.to_string())
}

pub fn slot(n: Slot) -> J {
    J::Str(n.to_string())
}

pub fn owner_name(o: Owner) -> String {
    match o {
        Owner::Slot(n) => n.to_string(),
        Owner::Sentence => "sentence".to_string(),
    }
}

/// A value: an int, or {"$Unknown": {"reason", "what", "values"}}.
pub fn val(v: &Vals, x: Val) -> J {
    match x {
        Val::Int(n) => J::Int(n as i64),
        Val::Unk(id) => {
            let u = v.get(id);
            dataclass(
                "Unknown",
                vec![
                    ("reason", J::Str(u.reason.as_str().into_owned())),
                    ("what", J::Str(u.what.to_string())),
                    (
                        "values",
                        match &u.values {
                            None => J::Null,
                            Some(vs) => set(vs.iter().map(|&n| J::Int(n as i64)).collect()),
                        },
                    ),
                ],
            )
        }
    }
}

/// A unit key as model.py's tuple: ("obj", 566), ("local", 1, 200), ...
pub fn unit_key(k: UnitKey) -> J {
    let (nums, n) = k.numbers();
    let mut items = vec![s(k.kind())];
    items.extend(nums[..n].iter().map(|&x| J::Int(x as i64)));
    tuple(items)
}

pub fn unit_of(g: &Game, u: scumm_ir::UnitId) -> J {
    if u == NONE_UNIT {
        tuple(vec![s("none")])
    } else {
        unit_key(g.unit(u).key)
    }
}

/// model.py `unit_name` of a frame's unit ("none" for check.py's empty frame).
pub fn unit_name_str(g: &Game, u: scumm_ir::UnitId) -> String {
    if u == NONE_UNIT {
        "none".to_string()
    } else {
        Game::unit_name(g.unit(u).key).to_string()
    }
}

fn owned(v: &Vals, o: Owned) -> J {
    match o {
        Owned::Bool(b) => J::Bool(b),
        Owned::Unk(x) => val(v, x),
    }
}

fn opt_bool(b: Option<bool>) -> J {
    b.map_or(J::Null, J::Bool)
}

fn opt_int(x: Option<u32>) -> J {
    x.map_or(J::Null, |n| J::Int(n as i64))
}

pub fn frame(g: &Game, v: &Vals, f: &Frame) -> J {
    dataclass(
        "Frame",
        vec![
            ("unit", unit_of(g, f.unit)),
            ("pc", opt_int(f.pc)),
            ("locals", map(f.locals.iter().map(|&(n, x)| (J::Int(n as i64), val(v, x))).collect())),
            ("args_known", J::Bool(f.args_known)),
            ("owned", owned(v, f.owned)),
            ("recursive", opt_bool(f.recursive)),
            ("resistant", opt_bool(f.resistant)),
            ("sid", J::Int(f.sid as i64)),
            ("freeze", J::Int(f.freeze as i64)),
            ("killed", opt_s(f.killed)),
            ("override", J::Int(f.override_ as i64)),
            ("nested", J::Bool(f.nested)),
            ("origin", s(f.origin)),
            ("time_loops", tuple(f.time_loops.iter().map(|&h| J::Int(h as i64)).collect())),
        ],
    )
}

/// A step's args dict ({i: value}): {"$map": ...}, or {} when empty.
fn args(v: &Vals, a: &[(i32, Val)]) -> J {
    map(a.iter().map(|&(n, x)| (J::Int(n as i64), val(v, x))).collect())
}

/// A step as model.py's tuple.
pub fn step(v: &Vals, st: &Step) -> J {
    let int = |n: i32| J::Int(n as i64);
    match st {
        Step::Write(k, x) => tuple(vec![s("write"), fact(*k), val(v, *x)]),
        Step::Note(w) => tuple(vec![s("note"), s(w)]),
        Step::Resume(k) => tuple(vec![s("resume"), fact(*k)]),
        Step::Sentence { verb, a, b } => tuple(vec![s("sentence"), int(*verb), int(*a), b.map_or(J::Null, int)]),
        Step::Script { n, resistant, recursive, args: a } => {
            tuple(vec![s("script"), int(*n), opt_bool(*resistant), opt_bool(*recursive), args(v, a)])
        }
        Step::Object { obj, verb, args: a } => tuple(vec![s("object"), int(*obj), int(*verb), args(v, a)]),
        Step::Hook { var, args: a } => tuple(vec![s("hook"), int(*var), args(v, a)]),
        Step::Room1(r) => tuple(vec![s("room1"), val(v, *r)]),
        Step::ScrollNote => tuple(vec![s("scroll_note")]),
        Step::CutIndex(sid) => tuple(vec![s("cut_index"), opt_int(*sid)]),
        Step::Scene { room, ego } => tuple(vec![s("scene"), val(v, *room), ego.map_or(J::Null, |e| val(v, e))]),
        Step::SceneBegin(r) => tuple(vec![s("scene_begin"), val(v, *r)]),
        Step::RoomCode { entry } => tuple(vec![s("room_code"), s(if *entry { "entry" } else { "exit" })]),
        Step::SceneKill => tuple(vec![s("scene_kill")]),
        Step::SceneLoad(r) => tuple(vec![s("scene_load"), val(v, *r)]),
        Step::SceneEgo(o) => tuple(vec![s("scene_ego"), val(v, *o)]),
        Step::Turn(n) => tuple(vec![s("turn"), slot(*n)]),
        Step::Sentences => tuple(vec![s("sentences")]),
    }
}

pub fn call(v: &Vals, c: &Call) -> J {
    dataclass(
        "Call",
        vec![
            ("head", s(&c.head)),
            ("cite", s(&c.cite)),
            ("steps", tuple(c.steps.iter().map(|st| step(v, st)).collect())),
            ("caller", opt_int(c.caller)),
            ("active", J::Bool(c.active)),
            ("i", J::Int(c.i as i64)),
        ],
    )
}

pub fn entry(g: &Game, v: &Vals, e: &Entry) -> J {
    match e {
        Entry::Frame(f) => frame(g, v, f),
        Entry::Call(c) => call(v, c),
    }
}

pub fn side(x: Side) -> J {
    match x {
        Side::Bool(b) => J::Bool(b),
        Side::Exit => s("exit"),
        Side::Stay => s("stay"),
    }
}

pub fn assumption(a: &Assumption) -> J {
    tuple(vec![s(&a.cite), s(&a.cond), side(a.side), s(&a.why)])
}

fn tri(t: &Tri) -> J {
    tuple(vec![s(&t.0), s(&t.1), s(&t.2)])
}

/// A fact -> value dict (writes, round_start, immediate).
pub fn store(v: &Vals, m: &crate::path::Store) -> J {
    obj(m.iter().map(|(k, x)| (k.to_string(), val(v, *x))))
}

/// golden.py `path_dict`: every field of model.Path but seen, round_slots, joined and history; plus
/// history_len and tid.
pub fn path(g: &Game, v: &Vals, p: &Path) -> J {
    let mut m: BTreeMap<String, J> = BTreeMap::new();
    let mut put = |k: &str, j: J| {
        m.insert(k.to_string(), j);
    };
    put("stack", J::Arr(p.stack.iter().map(|e| entry(g, v, e)).collect()));
    put("writes", store(v, &p.writes));
    put("assumptions", J::Arr(p.assumptions.iter().map(assumption).collect()));
    put("unmodelled", J::Arr(p.unmodelled.iter().map(tri).collect()));
    put("notes", J::Arr(p.notes.iter().map(tri).collect()));
    put("unknown_writes", J::Arr(p.unknown_writes.iter().map(tri).collect()));
    put("end", opt_s(p.end));
    put("end_at", opt_s(p.end_at.as_deref()));
    put("steps", J::Int(p.steps as i64));
    put(
        "freeze",
        obj(p.freeze.iter().map(|(n, c)| {
            (n.to_string(), c.map_or(J::Null, |(k, exact)| tuple(vec![J::Int(k as i64), J::Bool(exact)])))
        })),
    );
    put("cut_index", opt_int(p.cut_index));
    put("cut_data", J::Arr(p.cut_data.iter().map(|&x| val(v, x)).collect()));
    put("turns", J::Arr(p.turns.iter().map(|(n, how)| tuple(vec![slot(*n), s(how)])).collect()));
    put("errors", J::Arr(p.errors.iter().map(tri).collect()));
    put("depth", J::Int(p.depth as i64));
    put("reentries", J::Int(p.reentries as i64));
    put("next_sid", J::Int(p.next_sid as i64));
    put("last", p.last.as_ref().map_or(J::Null, |f| frame(g, v, f)));
    put("merged", J::Int(p.merged as i64));
    put("resumed", J::Bool(p.resumed));
    put("settle", J::Bool(p.settle));
    put("round", J::Int(p.round as i64));
    put("first_end", opt_s(p.first_end));
    put("immediate", p.immediate.as_ref().map_or(J::Null, |w| store(v, w)));
    put("after", set(p.after.iter().map(|&k| fact(k)).collect()));
    put(
        "slots",
        J::Arr(p.slots.iter().map(|(n, f)| J::Arr(vec![slot(*n), frame(g, v, f)])).collect()),
    );
    put(
        "queue",
        J::Arr(
            p.queue
                .iter()
                .map(|&(a, b, c, d)| tuple(vec![J::Int(a as i64), J::Int(b as i64), J::Int(c as i64), J::Int(d as i64)]))
                .collect(),
        ),
    );
    put("round_start", store(v, &p.round_start));
    put("round_ran", set(p.round_ran.iter().map(|&n| slot(n)).collect()));
    put(
        "ambient_done",
        set(p.ambient_done.iter().map(|&(n, h)| tuple(vec![slot(n), J::Int(h as i64)])).collect()),
    );
    put("owner", p.owner.map_or(J::Null, |o| J::Str(owner_name(o))));
    put("owner_rule", s(&p.owner_rule));
    put("writer", obj(p.writer.iter().map(|(k, r)| (k.to_string(), s(r)))));
    put(
        "rw",
        obj(p.rw.iter().map(|(o, (r, w))| {
            (
                owner_name(*o),
                tuple(vec![set(r.iter().map(|&k| fact(k)).collect()), set(w.iter().map(|&k| fact(k)).collect())]),
            )
        })),
    );
    put(
        "conflicts",
        J::Arr(
            p.conflicts
                .iter()
                .map(|(r, a, b, keys, n)| {
                    tuple(vec![
                        J::Int(*r as i64),
                        J::Str(owner_name(*a)),
                        J::Str(owner_name(*b)),
                        tuple(keys.iter().map(|&k| fact(k)).collect()),
                        J::Int(*n as i64),
                    ])
                })
                .collect(),
        ),
    );
    put("ran_by", obj(p.ran_by.iter().map(|(n, r)| (n.to_string(), s(r)))));
    put("not_run", obj(p.not_run.iter().map(|(n, r)| (n.to_string(), s(r)))));
    put("skips", set(p.skips.iter().map(|(n, o, w)| tuple(vec![slot(*n), s(o), s(w)])).collect()));
    put("sentences", J::Int(p.sentences as i64));
    put("steps0", J::Int(p.steps0 as i64));
    put("timed", J::Bool(p.timed));
    put(
        "camera_done",
        set(p.camera_done.iter().map(|&(n, h)| tuple(vec![slot(n), J::Int(h as i64)])).collect()),
    );
    put("ambient_pass", set(p.ambient_pass.iter().map(|&n| slot(n)).collect()));
    put(
        "ambient_vals",
        obj(p.ambient_vals.iter().map(|(k, vs)| (k.to_string(), J::Arr(vs.iter().map(|&x| val(v, x)).collect())))),
    );
    put("ambient_slots", set(p.ambient_slots.iter().map(|&n| slot(n)).collect()));
    put("multi", set(p.multi.iter().map(|&n| slot(n)).collect()));
    put("closed", J::Bool(p.closed));
    put(
        "input_drift",
        tuple(p.input_drift.iter().map(|&(k, a, b)| tuple(vec![fact(k), val(v, a), val(v, b)])).collect()),
    );
    put("cleared", opt_bool(p.cleared));
    put("history_len", J::Int(p.history.len() as i64));
    put("tid", opt_int(p.tid));
    J::Obj(m)
}

/// golden.py `run_dict`.
pub fn run(r: &Run) -> J {
    let mut m = BTreeMap::new();
    m.insert("start".to_string(), s(&r.start));
    m.insert("limit".to_string(), J::Int(r.limit as i64));
    m.insert("unit".to_string(), r.unit.map_or(J::Null, unit_key));
    m.insert("merges".to_string(), J::Int(r.merges as i64));
    m.insert("value_caps".to_string(), obj(r.value_caps.iter().map(|&(k, n)| (k.to_string(), J::Int(n as i64)))));
    m.insert("settle_merges".to_string(), J::Int(r.settle_merges as i64));
    m.insert("value_merges".to_string(), J::Int(r.value_merges as i64));
    m.insert("pre_slots".to_string(), tuple(r.pre_slots.iter().map(|&n| slot(n)).collect()));
    m.insert(
        "key_routes".to_string(),
        tuple(
            r.key_routes
                .iter()
                .map(|(fate, a)| tuple(vec![s(fate), tuple(a.iter().map(assumption).collect())]))
                .collect(),
        ),
    );
    m.insert("n_paths".to_string(), J::Int(r.paths.len() as i64));
    J::Obj(m)
}

/// golden.py `_where`: the top of a path's stack, [unit name, pc] for a frame, [call head, step] for a call.
pub fn where_(g: &Game, p: &Path) -> J {
    match p.stack.last() {
        Some(Entry::Frame(f)) => J::Arr(vec![J::Str(unit_name_str(g, f.unit)), opt_int(f.pc)]),
        Some(Entry::Call(c)) => J::Arr(vec![s(&c.head), J::Int(c.i as i64)]),
        None => J::Null,
    }
}

/// A line: {"kind": kind, ...}.
pub fn line(pairs: Vec<(&str, J)>) -> String {
    let mut s = String::new();
    J::Obj(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()).write(&mut s);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_rule() {
        let j = set(vec![J::Int(10), J::Int(9)]);
        assert_eq!(text(&j), r#"{"$set":[9,10]}"#);
        let j = set(vec![s("var:10"), s("var:9")]);
        assert_eq!(text(&j), r#"{"$set":["var:10","var:9"]}"#);
        let j = map(vec![(J::Int(2), J::Int(0)), (J::Int(1), J::Int(5))]);
        assert_eq!(text(&j), r#"{"$map":[[1,5],[2,0]]}"#);
        assert_eq!(text(&map(vec![])), "{}");
        let j = set(vec![tuple(vec![s("b"), J::Int(1)]), tuple(vec![s("a"), J::Int(2)])]);
        assert_eq!(text(&j), r#"{"$set":[{"$tuple":["a",2]},{"$tuple":["b",1]}]}"#);
    }
}
