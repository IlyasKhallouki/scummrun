//! The harness's start rules and plan bookkeeping, line for line: reader.task_start and reader.replan_start with
//! replay.py's ELSEWHERE rule (`task_start`, `replan_start`, `elsewhere`), packed_reading.Points, replay.view,
//! `prediction`, the actors' rooms at a plan's start, `cost_caps_of`, `capped_cost`, `refinable`, `reach_misses`,
//! `reach_start`, the `_same_input` key, and export_records.write_start's file.
//!
//! Facts are fmi-core's packed maps; a value is a facts.py int or a domain label (BELOW, ABOVE, NONE).

use crate::files::{self, Res};
use crate::pyjson;
use fmi_core::{
    unit_of, val_text, Domains, Facts, Key, Obs, Val, ABOVE, BELOW, K_ACTORROOM, K_ARGS, K_AT, K_ESC, K_ESCPOS, K_OTHER,
    K_POS, K_SAVED, NONE, U_ENTRY, U_EXIT, U_LOCAL, U_OBJ, U_SCRIPT,
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

/// pos: and escpos: of a unit with no live slot, or no override armed (reader.NOWHERE)
pub const NOWHERE: Val = -1;
/// a slot at a place none of the scope's points (replay.ELSEWHERE)
pub const ELSEWHERE: Val = -2;
/// an actor's room the start does not give (replay.ACTOR_ROOM_NOT_KNOWN)
pub const ACTOR_ROOM_NOT_KNOWN: Val = -1;

thread_local! {
    static TEXTS: std::cell::RefCell<HashMap<Key, std::rc::Rc<str>>> = std::cell::RefCell::new(HashMap::new());
}

/// A key's text, made once per thread (the harness sorts and writes the same keys at every plan).
pub fn key_text(k: Key) -> std::rc::Rc<str> {
    TEXTS.with(|t| {
        if let Some(x) = t.borrow().get(&k) {
            return x.clone();
        }
        let mut s = String::with_capacity(24);
        crate::archive::write_key(&mut s, k);
        let r: std::rc::Rc<str> = s.into();
        t.borrow_mut().insert(k, r.clone());
        r
    })
}

/// The unobserved prefixes (replay.UNOBSERVED), with actorroom: under unobserved_actor_rooms.
pub fn unobserved(actor_rooms: bool) -> Vec<&'static str> {
    let mut v = vec!["args:", "saved:"];
    if actor_rooms {
        v.push("actorroom:");
    }
    v
}

/// replay.PREDICTED: the plan's values kept for the replan.
pub fn predicted(actor_rooms: bool) -> Vec<&'static str> {
    let mut v = vec!["pos:", "escpos:", "args:", "saved:"];
    if actor_rooms {
        v.push("actorroom:");
    }
    v
}

/// A key's kind test by its prefix, without building its text for the packed kinds.
pub fn has_prefix(k: Key, prefixes: &[&str]) -> bool {
    let kind = k.kind();
    let packed = |p: &str| -> Option<u8> {
        Some(match p {
            "pos:" => K_POS,
            "escpos:" => K_ESCPOS,
            "args:" => K_ARGS,
            "saved:" => K_SAVED,
            "actorroom:" => K_ACTORROOM,
            "at:" => K_AT,
            "esc:" => K_ESC,
            _ => return None,
        })
    };
    if kind == K_OTHER {
        let t = k.text();
        return prefixes.iter().any(|p| t.starts_with(p));
    }
    prefixes.iter().any(|p| match packed(p) {
        Some(kk) => kk == kind,
        None => k.has_prefix(p),
    })
}

/// facts.py `unit_of`'s text: script:N, local:N, obj:O, entry, exit.
pub fn unit_name(where_: i32, number: i32) -> Option<String> {
    let (u, a) = unit_of(where_, number)?;
    Some(match u {
        U_SCRIPT => format!("script:{a}"),
        U_LOCAL => format!("local:{a}"),
        U_OBJ => format!("obj:{a}"),
        U_ENTRY => "entry".into(),
        U_EXIT => "exit".into(),
        _ => return None,
    })
}

/// packed_reading.Points: per unit its wait points (at) and Esc points (esc), from the domains of the task's pos: and
/// escpos: facts; the keys whose domain holds ELSEWHERE; reading.json's live_locals.
#[derive(Clone, Debug, Default)]
pub struct Points {
    /// (unit, a) -> points, for pos: (at) and escpos: (esc)
    pub at: HashMap<(u8, u64), Vec<Val>>,
    pub esc: HashMap<(u8, u64), Vec<Val>>,
    pub elsewhere: HashSet<Key>,
    pub live_locals: Option<Value>,
    /// b.counts: the start rule's counters
    pub counts: BTreeMap<String, i64>,
}

fn is_int(v: Val) -> bool {
    v != BELOW && v != ABOVE && v != NONE
}

impl Points {
    pub fn of(doms: &Domains, live_locals: Option<Value>) -> Points {
        let mut p = Points { live_locals, ..Default::default() };
        for (k, d) in &doms.0 {
            let kind = k.kind();
            if kind != K_POS && kind != K_ESCPOS {
                continue;
            }
            let vals: Vec<Val> = d.values.iter().copied().filter(|&v| is_int(v) && v != NOWHERE && v != ELSEWHERE).collect();
            let m = if kind == K_POS { &mut p.at } else { &mut p.esc };
            m.insert((k.unit(), k.a()), vals);
            if d.values.contains(&ELSEWHERE) {
                p.elsewhere.insert(*k);
            }
        }
        p
    }

    /// The locals the reading keyed slot `name`'s lists on in room `room` (None: every local).
    pub fn read_locals(&self, name: &str, room: i32) -> Option<Vec<i64>> {
        let ll = self.live_locals.as_ref()?;
        let slots = ll.get("slots")?;
        let key = if name.starts_with("script:") || name.starts_with("obj:") { name.to_string() } else { format!("{name}@{room}") };
        slots.get(&key)?.as_array().map(|a| a.iter().filter_map(|x| x.as_i64()).collect())
    }
}

/// reader.task_start with replay.py's ELSEWHERE rule: the start as the task reads it, pos:/escpos: from the engine's
/// at:/esc: facts (the least place; a unit the task names with no place NOWHERE, ELSEWHERE where its domain holds it
/// and the engine shows the slot at a place none of the points).
pub fn task_start(b: &mut Points, obs: &Facts, doms: &Domains) -> Facts {
    let mut out = reader_task_start(b, obs, doms);
    elsewhere(b, obs, &mut out);
    out
}

/// reader.task_start alone (no ELSEWHERE rule).
fn reader_task_start(b: &mut Points, obs: &Facts, doms: &Domains) -> Facts {
    let mut v: Vec<(Key, Val)> = obs.0.clone();
    for (k, _) in &doms.0 {
        if k.kind() == K_POS || k.kind() == K_ESCPOS {
            v.push((*k, NOWHERE));
        }
    }
    // at:/esc: facts holding 1, by unit: their places
    let mut places: HashMap<(u8, u8, u64), Vec<Val>> = HashMap::new();
    for &(k, x) in &obs.0 {
        if x == 1 && (k.kind() == K_AT || k.kind() == K_ESC) {
            places.entry((k.kind(), k.unit(), k.a())).or_default().push(k.b() as Val);
        }
    }
    for (kind, pos, pts) in [(K_AT, K_POS, &b.at), (K_ESC, K_ESCPOS, &b.esc)] {
        for (&(u, a), ws_all) in pts.iter() {
            let held = places.get(&(kind, u, a));
            let ws: Vec<Val> = ws_all
                .iter()
                .copied()
                .filter(|w| {
                    if *w >= 0 {
                        held.map(|h| h.contains(w)).unwrap_or(false)
                    } else {
                        // a negative place: a key no packed form holds, by its text
                        let unit = Key::pack(pos, u, a, 0).text();
                        let what = if kind == K_AT { "at" } else { "esc" };
                        let name = unit.split_once(':').map(|x| x.1).unwrap_or("");
                        obs.get(Key::parse(&format!("{what}:{name}:{w}"))) == Some(1)
                    }
                })
                .collect();
            if ws.len() > 1 {
                let what = if kind == K_AT { "at" } else { "esc" };
                *b.counts.entry(format!("{what}: more than one place at the start for one unit (the first taken)")).or_insert(0) += 1;
            }
            v.push((Key::pack(pos, u, a, 0), ws.iter().copied().min().unwrap_or(NOWHERE)));
        }
    }
    Facts::from_pairs(v)
}

/// replay.elsewhere.
pub fn elsewhere(b: &mut Points, obs: &Facts, out: &mut Facts) {
    if b.elsewhere.is_empty() {
        return;
    }
    for &(k, v) in &obs.0 {
        if v != 1 {
            continue;
        }
        let (kind, u, a, w): (u8, u8, u64, i64) = match k.kind() {
            K_AT | K_ESC => (k.kind(), k.unit(), k.a(), k.b() as i64),
            K_OTHER => {
                // an at:/esc: key with a place no packed key holds (a negative one)
                let t = k.text();
                let Some((kind, rest)) = t.split_once(':') else { continue };
                if kind != "at" && kind != "esc" {
                    continue;
                }
                let Some((name, w)) = rest.rsplit_once(':') else { continue };
                let digits = w.trim_start_matches('-');
                if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
                    continue;
                }
                let pk = Key::parse(&format!("pos:{name}"));
                if pk.kind() != K_POS {
                    continue;
                }
                (if kind == "at" { K_AT } else { K_ESC }, pk.unit(), pk.a(), w.parse().unwrap_or(i64::MIN))
            }
            _ => continue,
        };
        let pos = if kind == K_AT { K_POS } else { K_ESCPOS };
        let key = Key::pack(pos, u, a, 0);
        let pts = if kind == K_AT { &b.at } else { &b.esc };
        if b.elsewhere.contains(&key) && out.get(key) == Some(NOWHERE) && !pts.get(&(u, a)).map(|p| p.contains(&w)).unwrap_or(false) {
            out.set(key, ELSEWHERE);
            *b.counts.entry("slots at the start at a place none of the scope's points (ELSEWHERE)".into()).or_insert(0) += 1;
        }
    }
}

/// replay.replan_start: reader.replan_start (args: copied from the prediction where the slot is where the plan put
/// it) with the ELSEWHERE rule. Returns (start, counts).
pub fn replan_start(b: &mut Points, obs: &Facts, predicted: &Facts, doms: &Domains) -> (Facts, BTreeMap<String, i64>) {
    let mut out = reader_task_start(b, obs, doms);
    let mut counts = BTreeMap::new();
    for (k, _) in &doms.0 {
        if k.kind() != K_ARGS {
            continue;
        }
        let pk = Key::pack(K_POS, k.unit(), k.a(), 0);
        let pos = out.get(pk).unwrap_or(NOWHERE);
        let pv = predicted.get(*k).unwrap_or(0);
        if pos != NOWHERE && predicted.get(pk) == Some(pos) && pv != 0 && pv != NONE {
            out.set(*k, pv);
            *counts.entry("args copied from the plan (the slot is where the plan put it)".to_string()).or_insert(0) += 1;
        } else {
            out.set(*k, 0);
            if pos != NOWHERE {
                *counts.entry("live slots with unknown locals (args 0)".to_string()).or_insert(0) += 1;
            }
        }
    }
    elsewhere(b, obs, &mut out);
    (out, counts)
}

/// replay.view: the engine's facts as the task's values (every domain key but the unobserved ones, each value
/// labelled by its domain).
pub fn view(b: &mut Points, doms: &Domains, obs: &Facts, unobserved: &[&str]) -> Facts {
    let st = task_start(b, obs, doms);
    let mut v = Vec::with_capacity(doms.0.len());
    for (k, d) in &doms.0 {
        if has_prefix(*k, unobserved) {
            continue;
        }
        v.push((*k, d.label(st.get(*k).unwrap_or(0))));
    }
    Facts(v)
}

/// replay.prediction: the engine's values with the plan's slot positions and unobserved facts over them.
pub fn prediction(now: &Facts, pred: &Facts) -> Facts {
    let mut out = now.clone();
    for &(k, v) in &pred.0 {
        out.set(k, v);
    }
    out
}

/// replay.holds: the keys of `conds` that `state` does not meet (unobserved keys skipped; absent is None).
pub fn holds(conds: &[(Key, Vec<Val>)], state: &Facts, unobserved: &[&str]) -> Vec<Key> {
    conds.iter().filter(|(k, s)| !has_prefix(*k, unobserved) && !state.get(*k).map(|v| s.contains(&v)).unwrap_or(false)).map(|(k, _)| *k).collect()
}

/// Harness._actor_rooms_start: each actorroom: fact of the task at its value in the plan's prediction at a replan,
/// else not known.
pub fn actor_rooms_start(start: &mut Facts, doms: &Domains, pred: &Facts, first: bool) {
    for (k, _) in &doms.0 {
        if k.kind() == K_ACTORROOM {
            let v = if first { None } else { pred.get(*k) };
            start.set(*k, match v {
                Some(x) if is_int(x) => x,
                _ => ACTOR_ROOM_NOT_KNOWN,
            });
        }
    }
}

/// replay.cost_caps_of: {"event(NAME@W": frames} for every live slot of the dump at its wait point.
pub fn cost_caps_of(state: &Obs, dmax: i64) -> BTreeMap<String, i64> {
    let mut out = BTreeMap::new();
    for sl in &state.slots {
        let Some(name) = unit_name(sl.where_, sl.number) else { continue };
        if sl.code < 0 {
            continue;
        }
        let cap = if sl.status == 1 && sl.delay > 0 { sl.delay as i64 / dmax.max(1) + 1 } else { 1 };
        let key = format!("event({name}@{}", sl.code);
        let e = out.entry(key).or_insert(cap);
        *e = (*e).min(cap);
    }
    out
}

/// replay.capped_cost: an action's cost under the caps (a label `P)` or `P#...`).
pub fn capped_cost(name: &str, cost: i64, caps: &BTreeMap<String, i64>) -> i64 {
    let label = if name.contains('#') { name.rsplit_once('#').unwrap().0 } else { name };
    let head = label.split('#').next().unwrap_or("");
    let prefix = head.strip_suffix(')').unwrap_or(head);
    caps.get(prefix).map(|&c| cost.min(c)).unwrap_or(cost)
}

/// replay.NOT_A_FACT: trace_explain.reading_kind of a variable the reading holds no fact of
pub const NOT_A_FACT: &str = "not a fact of the reading";

/// replay.refinable: the variables a cause of class split names as read by its test while the reading holds no fact
/// of them, in the cause's order.
pub fn refinable(cause: Option<&Value>) -> Vec<i64> {
    let Some(c) = cause.filter(|c| files::truthy(Some(c))) else { return vec![] };
    if c.get("class").and_then(|x| x.as_str()) != Some("split") {
        return vec![];
    }
    let mut out = Vec::new();
    for v in c.get("values").and_then(|x| x.as_array()).into_iter().flatten() {
        let what = files::py_str(v.get("what"));
        let n = what.strip_prefix("var ").filter(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()));
        if let Some(n) = n.and_then(|d| d.parse::<i64>().ok()) {
            if v.get("kind").and_then(|k| k.as_str()) == Some(NOT_A_FACT) && !out.contains(&n) {
                out.push(n);
            }
        }
    }
    out
}

/// replay.reach_misses: (how many, the named values "key=value") from taskcore's message, or None.
pub fn reach_misses(message: &str) -> Option<(i64, Vec<String>)> {
    let pat = "the start holds ";
    let i = message.find(pat)?;
    let rest = &message[i + pat.len()..];
    let (n, rest) = rest.split_once(' ')?;
    let n: i64 = n.parse().ok()?;
    let rest = rest.strip_prefix("values the reading did not reach: [")?;
    let end = rest.find(']')?;
    let inner = &rest[..end];
    // the quoted strings, JSON escapes read back
    let mut out = Vec::new();
    let b = inner.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'"' {
            let mut j = i + 1;
            while j < b.len() && b[j] != b'"' {
                if b[j] == b'\\' {
                    j += 1;
                }
                j += 1;
            }
            if j < b.len() {
                if let Ok(Value::String(s)) = serde_json::from_str::<Value>(&inner[i..=j]) {
                    out.push(s);
                }
            }
            i = j + 1;
        } else {
            i += 1;
        }
    }
    Some((n, out))
}

/// replay.reach_start: every int value, sorted by key text (as reader-scope's --reach-also reads it).
pub fn reach_start(start: &Facts) -> Vec<(String, i64)> {
    let mut v: Vec<(std::rc::Rc<str>, i64)> = start.0.iter().filter(|(_, x)| is_int(*x)).map(|(k, x)| (key_text(*k), *x))
        .filter(|(k, _)| !k.contains('\t') && !k.contains('\n')).collect();
    v.sort_unstable_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    v.into_iter().map(|(k, x)| (k.to_string(), x)).collect()
}

/// `start_file_text(&reach_start(start))` written from the keys' texts in place: their order (by text) is kept from the
/// last call while the key set is the same (the engine's facts change little from start to start).
pub fn reach_start_text(start: &Facts) -> String {
    thread_local! {
        static LAST: std::cell::RefCell<(Vec<Key>, Vec<(usize, std::rc::Rc<str>)>)> = const { std::cell::RefCell::new((Vec::new(), Vec::new())) };
    }
    LAST.with(|l| {
        let mut l = l.borrow_mut();
        let same = l.0.len() == start.0.len() && l.0.iter().zip(start.0.iter()).all(|(a, b)| *a == b.0);
        if !same {
            let keys: Vec<Key> = start.0.iter().map(|(k, _)| *k).collect();
            let mut v: Vec<(usize, std::rc::Rc<str>)> = start.0.iter().enumerate().map(|(i, (k, _))| (i, key_text(*k)))
                .filter(|(_, t)| !t.contains('\t') && !t.contains('\n')).collect();
            v.sort_unstable_by(|a, b| a.1.as_bytes().cmp(b.1.as_bytes()));
            *l = (keys, v);
        }
        let mut s = String::with_capacity(l.1.len() * 24);
        let mut b = itoa::Buffer::new();
        for (i, t) in &l.1 {
            let x = start.0[*i].1;
            if is_int(x) {
                s.push_str(t);
                s.push('\t');
                s.push_str(b.format(x));
                s.push('\n');
            }
        }
        s
    })
}

/// The text of a start file of `reach_start` (replay_part1's reach_extender and refiner).
pub fn start_file_text(start: &[(String, i64)]) -> String {
    let mut s = String::new();
    for (k, v) in start {
        s.push_str(k);
        s.push('\t');
        s.push_str(&v.to_string());
        s.push('\n');
    }
    s
}

/// Harness._same_input's key: sha1 of json.dumps([frame, len(bans), sorted((k, str(v)))]) [:16].
pub fn input_key(frame: u64, n_bans: usize, start: &Facts) -> String {
    // json.dumps([frame, n, [[k, str(v)], ...]]) with the pairs sorted (keys are unique: by the key's text). The
    // keys' text order and each pair's `["key", "` are kept from the last call while the key set is the same (the
    // engine's facts and the task's keys change little from plan to plan)
    thread_local! {
        static LAST: std::cell::RefCell<(Vec<Key>, Vec<usize>, Vec<String>)> = const { std::cell::RefCell::new((Vec::new(), Vec::new(), Vec::new())) };
    }
    LAST.with(|l| {
        let mut l = l.borrow_mut();
        let same = l.0.len() == start.0.len() && l.0.iter().zip(start.0.iter()).all(|(a, b)| *a == b.0);
        if !same {
            let keys: Vec<Key> = start.0.iter().map(|(k, _)| *k).collect();
            let texts: Vec<std::rc::Rc<str>> = keys.iter().map(|k| key_text(*k)).collect();
            let mut order: Vec<usize> = (0..keys.len()).collect();
            order.sort_unstable_by(|&a, &b| texts[a].as_bytes().cmp(texts[b].as_bytes()));
            let prefixes: Vec<String> = order.iter().map(|&i| {
                let mut p = String::with_capacity(texts[i].len() + 6);
                p.push('[');
                pyjson::write_str(&mut p, &texts[i]);
                p.push_str(", \"");
                p
            }).collect();
            *l = (keys, order, prefixes);
        }
        let (_, order, prefixes) = &*l;
        let mut s = String::with_capacity(order.len() * 28 + 32);
        let mut nb = itoa::Buffer::new();
        s.push('[');
        s.push_str(nb.format(frame));
        s.push_str(", ");
        s.push_str(nb.format(n_bans));
        s.push_str(", [");
        for (j, &i) in order.iter().enumerate() {
            if j > 0 {
                s.push_str(", ");
            }
            s.push_str(&prefixes[j]);
            match start.0[i].1 {
                BELOW => s.push_str("below"),
                ABOVE => s.push_str("above"),
                NONE => s.push_str("None"),
                n => s.push_str(nb.format(n)),
            }
            s.push_str("\"]");
        }
        s.push_str("]]");
        let h = sha1_smol::Sha1::from(s.as_bytes()).digest().to_string();
        h[..16].to_string()
    })
}

/// export_records.write_start's text: one line per fact of the domains, sorted by key text, its start value's label
/// text, or nothing after the tab where the domain lacks it.
pub fn start_text(doms: &Domains, start: &Facts) -> String {
    start_text_in(&text_order(doms), doms, start)
}

/// A reading's domain keys in their text order (the start file's), made once per reading.
pub fn text_order(doms: &Domains) -> Vec<(std::rc::Rc<str>, Key)> {
    let mut keys: Vec<(std::rc::Rc<str>, Key)> = doms.0.iter().map(|(k, _)| (key_text(*k), *k)).collect();
    keys.sort();
    keys
}

/// `start_text` with the keys' order made already.
pub fn start_text_in(order: &[(std::rc::Rc<str>, Key)], doms: &Domains, start: &Facts) -> String {
    let mut s = String::with_capacity(order.len() * 28);
    for (t, k) in order {
        let (t, k) = (&**t, *k);
        let d = doms.get(k).unwrap();
        let v = start.get(k).unwrap_or(0);
        let lab = if v == BELOW || v == ABOVE || v == NONE { v } else { d.label(v) };
        let txt = if lab == NONE || !d.values.contains(&lab) { String::new() } else { val_text(lab) };
        s.push_str(t);
        s.push('\t');
        s.push_str(&txt);
        s.push('\n');
    }
    s
}

/// The caps file (Harness._plan_taskcore's caps.txt).
pub fn caps_text(caps: &BTreeMap<String, i64>) -> String {
    caps.iter().map(|(k, v)| format!("{k}\t{v}\n")).collect()
}

pub fn write_text(p: &Path, t: &str) -> Res<()> {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| files::io_err(d, e))?;
    }
    std::fs::write(p, t).map_err(|e| files::io_err(p, e))
}

/// A facts map as JSON (keys by text, sorted; values as Python prints them: ints, "below", "above", null).
pub fn facts_json(f: &Facts) -> Value {
    let mut kv: Vec<(String, Val)> = f.0.iter().map(|(k, v)| (k.text(), *v)).collect();
    kv.sort();
    let mut m = serde_json::Map::new();
    for (k, v) in kv {
        m.insert(k, val_json(v));
    }
    Value::Object(m)
}

pub fn val_json(v: Val) -> Value {
    match v {
        BELOW => json!("below"),
        ABOVE => json!("above"),
        NONE => Value::Null,
        n => json!(n),
    }
}

/// A facts.py map read from JSON (ints; "below"/"above" labels; null as NONE).
pub fn facts_of_json(v: &Value) -> Facts {
    let mut pairs = Vec::new();
    if let Some(m) = v.as_object() {
        for (k, x) in m {
            let val = match x {
                Value::String(s) if s == "below" => BELOW,
                Value::String(s) if s == "above" => ABOVE,
                Value::Null => NONE,
                x => files::as_i64(Some(x)).unwrap_or(NONE),
            };
            pairs.push((Key::parse(k), val));
        }
    }
    Facts::from_pairs(pairs)
}
