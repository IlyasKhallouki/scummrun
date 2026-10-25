//! size_scaling.py's scopes, start and goal, for the reading at Part I's scale: the room graph from the IR alone
//! (`room_graph`, `scopes`), the start (results/start-part1.jsonl, seed 1, its facts in the file's order) and the
//! goal (results/end-part1.json `goal_all`).

use crate::facts::Facts;
use rustc_hash::FxHashMap;
use scumm_ir::{known, Expr, Game, Stmt, UnitKey};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// size_scaling.py `SEED`.
pub const SEED: i64 = 1;

/// size_scaling.py `room_graph`: room R links to R' when code that belongs to R (its entry and exit code, its
/// local scripts, the scripts of the objects whose room is R) has loadRoom(R') or loadRoomWithEgo(obj, R') with a
/// constant R'; a pseudo room is mapped to its room resource, a link to R itself is dropped.
pub fn room_graph(game: &Game) -> BTreeMap<i32, BTreeSet<i32>> {
    let ast = &game.ast;
    let mut links: BTreeMap<i32, BTreeSet<i32>> = BTreeMap::new();
    for u in game.ids() {
        let room = match game.unit(u).key {
            UnitKey::Entry(r) | UnitKey::Exit(r) | UnitKey::Local(r, _) => Some(r),
            UnitKey::Obj(o) => game.object_room.get(&o).copied(),
            UnitKey::Global(_) => continue,
        };
        let Some(room) = room else { continue };
        for x in game.ins(u) {
            let Some(Stmt::Op { name, args, .. }) = x.st else { continue };
            let i = match name {
                known::LOAD_ROOM => 0,
                known::LOAD_ROOM_WITH_EGO => 1,
                _ => continue,
            };
            let Some(&arg) = ast.kids(args).get(i) else { panic!("a room change with too few arguments") };
            let Expr::Const(mut t) = ast.expr(arg) else { continue };
            if t == 0 {
                continue;
            }
            if let Some(&r) = game.pseudo.get(&t) {
                t = r;
            }
            if t == room {
                continue;
            }
            links.entry(room).or_default().insert(t);
        }
    }
    links
}

/// size_scaling.py `scopes`: {k: the rooms within k links of the start room, sorted}, for every k to the last ring.
pub fn scopes(game: &Game, start_room: i32) -> Vec<Vec<i32>> {
    let links = room_graph(game);
    let mut dist: FxHashMap<i32, usize> = FxHashMap::default();
    dist.insert(start_room, 0);
    let mut ring = vec![start_room];
    while !ring.is_empty() {
        let mut nxt = Vec::new();
        for r in ring {
            for &t in links.get(&r).into_iter().flatten() {
                if !dist.contains_key(&t) {
                    dist.insert(t, dist[&r] + 1);
                    nxt.push(t);
                }
            }
        }
        ring = nxt;
    }
    let max = *dist.values().max().unwrap();
    (0..=max)
        .map(|k| {
            let mut v: Vec<i32> = dist.iter().filter(|(_, &d)| d <= k).map(|(&r, _)| r).collect();
            v.sort_unstable();
            v
        })
        .collect()
}

/// The start of size_scaling.py: results/start-part1.jsonl's line with `seed`, its facts in the file's order.
pub fn start_facts(root: &Path, seed: i64) -> Facts {
    let p = root.join("experiments/E003-path-through-cloud/results/start-part1.jsonl");
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    for line in text.lines() {
        let v = Json::parse(line);
        if v.get("seed").and_then(Json::int) != Some(seed) {
            continue;
        }
        let facts = v.get("facts").expect("facts");
        let Json::Obj(items) = facts else { panic!("facts is not an object") };
        return Facts::from_items(items.iter().map(|(k, x)| (k.clone(), x.int().expect("an int fact"))));
    }
    panic!("no seed {seed} in {}", p.display())
}

/// size_scaling.py `goal`: results/end-part1.json `goal_all`, {key: [value]} in the file's order.
pub fn part1_goal(root: &Path) -> Vec<(String, Vec<i64>)> {
    let p = root.join("experiments/E003-path-through-cloud/results/end-part1.json");
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    let v = Json::parse(&text);
    if let Some(Json::Obj(n)) = v.get("goal_not") {
        assert!(n.is_empty(), "goal_not is not used here");
    }
    let Some(Json::Obj(items)) = v.get("goal_all") else { panic!("no goal_all") };
    items.iter().map(|(k, x)| (k.clone(), vec![x.int().expect("an int goal value")])).collect()
}

/// A JSON value whose objects keep their keys in the text's order (serde_json's map sorts them, and Python's dict
/// order is the start's: reader.Scope.of walks it).
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(String),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn parse(text: &str) -> Json {
        let b = text.as_bytes();
        let mut i = 0;
        let v = value(b, &mut i);
        ws(b, &mut i);
        assert_eq!(i, b.len(), "trailing text after the JSON value");
        v
    }
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(items) => items.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn int(&self) -> Option<i64> {
        match self {
            Json::Num(s) => s.parse().ok(),
            _ => None,
        }
    }
}

fn ws(b: &[u8], i: &mut usize) {
    while *i < b.len() && matches!(b[*i], b' ' | b'\t' | b'\n' | b'\r') {
        *i += 1;
    }
}

fn value(b: &[u8], i: &mut usize) -> Json {
    ws(b, i);
    match b[*i] {
        b'{' => {
            *i += 1;
            let mut items = Vec::new();
            ws(b, i);
            if b[*i] == b'}' {
                *i += 1;
                return Json::Obj(items);
            }
            loop {
                ws(b, i);
                let Json::Str(k) = value(b, i) else { panic!("an object key that is not a string") };
                ws(b, i);
                assert_eq!(b[*i], b':');
                *i += 1;
                items.push((k, value(b, i)));
                ws(b, i);
                match b[*i] {
                    b',' => *i += 1,
                    b'}' => {
                        *i += 1;
                        return Json::Obj(items);
                    }
                    c => panic!("unexpected {:?} in an object", c as char),
                }
            }
        }
        b'[' => {
            *i += 1;
            let mut xs = Vec::new();
            ws(b, i);
            if b[*i] == b']' {
                *i += 1;
                return Json::Arr(xs);
            }
            loop {
                xs.push(value(b, i));
                ws(b, i);
                match b[*i] {
                    b',' => *i += 1,
                    b']' => {
                        *i += 1;
                        return Json::Arr(xs);
                    }
                    c => panic!("unexpected {:?} in an array", c as char),
                }
            }
        }
        b'"' => {
            *i += 1;
            let mut s: Vec<u8> = Vec::new();
            loop {
                let c = b[*i];
                *i += 1;
                match c {
                    b'"' => return Json::Str(String::from_utf8(s).expect("UTF-8")),
                    b'\\' => {
                        let e = b[*i];
                        *i += 1;
                        match e {
                            b'"' | b'\\' | b'/' => s.push(e),
                            b'b' => s.push(8),
                            b'f' => s.push(12),
                            b'n' => s.push(b'\n'),
                            b'r' => s.push(b'\r'),
                            b't' => s.push(b'\t'),
                            b'u' => {
                                let h = std::str::from_utf8(&b[*i..*i + 4]).unwrap();
                                *i += 4;
                                let mut cp = u32::from_str_radix(h, 16).unwrap();
                                if (0xD800..0xDC00).contains(&cp) && b.get(*i) == Some(&b'\\') && b.get(*i + 1) == Some(&b'u') {
                                    let lo = u32::from_str_radix(std::str::from_utf8(&b[*i + 2..*i + 6]).unwrap(), 16).unwrap();
                                    *i += 6;
                                    cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                                }
                                let ch = char::from_u32(cp).unwrap_or('\u{FFFD}');
                                let mut buf = [0u8; 4];
                                s.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                            }
                            c => panic!("bad escape \\{}", c as char),
                        }
                    }
                    c => s.push(c),
                }
            }
        }
        b't' => {
            *i += 4;
            Json::Bool(true)
        }
        b'f' => {
            *i += 5;
            Json::Bool(false)
        }
        b'n' => {
            *i += 4;
            Json::Null
        }
        _ => {
            let st = *i;
            while *i < b.len() && matches!(b[*i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                *i += 1;
            }
            assert!(*i > st, "not a JSON value at byte {st}");
            Json::Num(std::str::from_utf8(&b[st..*i]).unwrap().to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_keeps_key_order() {
        let v = Json::parse(r#"{"b": 1, "a": [true, null, -2.5e3, "x\"é"], "c": {}}"#);
        let Json::Obj(items) = &v else { panic!() };
        assert_eq!(items.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), ["b", "a", "c"]);
        assert_eq!(v.get("b").unwrap().int(), Some(1));
        assert_eq!(v.get("a").unwrap(), &Json::Arr(vec![Json::Bool(true), Json::Null, Json::Num("-2.5e3".into()), Json::Str("x\"é".into())]));
    }
}
