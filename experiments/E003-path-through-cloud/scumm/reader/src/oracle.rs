//! The comparisons against the Python oracles (PORT-NOTES.md): reader_port_oracle.py's static dump and captures,
//! and golden.py's reader files. Each returns its differences as text (empty: equal); the tests and the
//! `reader-port` binary call them.

use crate::builder::Builder;
use crate::domain::{Label, Labels};
use crate::domains;
use crate::facts::Facts;
use crate::py::Py;
use crate::record::Record;
use crate::replay::Replay;
use crate::scope::{self, Scope};
use scumm_ir::Game;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub fn root() -> PathBuf {
    scumm_ir::default_root()
}

/// Where reader_port_oracle.py writes (`FAST_MI_READER_ORACLE` overrides it).
pub fn oracle_dir() -> PathBuf {
    std::env::var_os("FAST_MI_READER_ORACLE").map_or_else(|| root().join("out").join("reader-port"), PathBuf::from)
}

pub fn golden_dir() -> PathBuf {
    root().join("out").join("golden").join("reader")
}

pub fn load_game() -> Game {
    Game::load(&root()).expect("the game")
}

/// The static oracle's lines.
pub struct Static {
    pub lines: Vec<Py>,
}

impl Static {
    pub fn load() -> Static {
        let p = oracle_dir().join("static.jsonl.gz");
        Static { lines: crate::gz_lines(&p).map(|l| Py::from_text(&l)).collect() }
    }
    pub fn of_kind<'a>(&'a self, kind: &'a str) -> impl Iterator<Item = &'a Py> + 'a {
        self.lines.iter().filter(move |l| l.get("kind").as_str() == kind)
    }
    pub fn named<'a>(&'a self, kind: &'a str, name: &str) -> &'a Py {
        self.of_kind(kind).find(|l| l.get("name").as_str() == name).unwrap_or_else(|| panic!("no {kind} {name}"))
    }
}

/// The first difference between two canonical texts, as golden.py's `diff` reports it (a path into the JSON).
pub fn first_diff(golden: &str, candidate: &str) -> Option<String> {
    if golden == candidate {
        return None;
    }
    let a: Value = serde_json::from_str(golden).unwrap();
    let b: Value = serde_json::from_str(candidate).unwrap();
    fn diff(a: &Value, b: &Value, at: &str) -> Option<String> {
        let show = |v: &Value| {
            let s = v.to_string();
            if s.len() > 300 {
                format!("{}...", &s[..300])
            } else {
                s
            }
        };
        match (a, b) {
            (Value::Object(x), Value::Object(y)) => {
                let keys: std::collections::BTreeSet<&String> = x.keys().chain(y.keys()).collect();
                for k in keys {
                    let p = if at.is_empty() { k.clone() } else { format!("{at}.{k}") };
                    match (x.get(k), y.get(k)) {
                        (Some(u), Some(v)) => {
                            if let Some(d) = diff(u, v, &p) {
                                return Some(d);
                            }
                        }
                        (u, v) => return Some(format!("{p}: golden {:?}, candidate {:?}", u.map(show), v.map(show))),
                    }
                }
                None
            }
            (Value::Array(x), Value::Array(y)) => {
                for (i, (u, v)) in x.iter().zip(y).enumerate() {
                    if let Some(d) = diff(u, v, &format!("{at}[{i}]")) {
                        return Some(d);
                    }
                }
                (x.len() != y.len()).then(|| format!("{at}.length: golden {}, candidate {}", x.len(), y.len()))
            }
            _ => (a != b).then(|| format!("{}: golden {}, candidate {}", if at.is_empty() { "$" } else { at }, show(a), show(b))),
        }
    }
    Some(diff(&a, &b, "").unwrap_or_else(|| "the texts differ (same JSON)".into()))
}

fn check(out: &mut Vec<String>, what: &str, golden: &Py, mine: &Py) {
    if let Some(d) = first_diff(&golden.text(), &mine.text()) {
        out.push(format!("{what}: {d}"));
    }
}

fn goals_of(p: &Py) -> Vec<(String, Vec<i64>)> {
    if p.is_none() {
        return vec![];
    }
    p.dict_items().iter().map(|(k, v)| (k.clone(), v.items().iter().map(|x| x.int()).collect())).collect()
}

/// domains.build for each start of the oracle.
pub fn compare_domains(game: &Game, st: &Static) -> (Vec<String>, usize) {
    let rooms = domains::meta_rooms(&root());
    let mut out = Vec::new();
    let mut n = 0;
    for l in st.of_kind("domains") {
        let start = Facts::from_py(l.get("start_items"));
        let d = domains::build(game, &rooms, &start, &goals_of(l.get("goals")));
        check(&mut out, &format!("domains {}", l.get("name").as_str()), l.get("domains"), &d.py());
        n += 1;
    }
    (out, n)
}

/// Scope.of for each scope of the oracle.
pub fn compare_scopes(game: &Game, st: &Static) -> (Vec<String>, usize) {
    let mut out = Vec::new();
    let mut n = 0;
    for l in st.of_kind("scope") {
        let rooms: Vec<i32> = l.get("rooms").items().iter().map(|r| r.int() as i32).collect();
        let sc = Scope::of(game, &rooms, &Facts::from_py(l.get("start_items")));
        check(&mut out, &format!("scope {}", l.get("name").as_str()), l.get("scope"), &sc.py(game));
        n += 1;
    }
    (out, n)
}

/// The inventory hook's summary for every global script, and entry_writes for every verb.
pub fn compare_hooks(game: &Game, st: &Static) -> (Vec<String>, usize) {
    let mut out = Vec::new();
    let mut n = 0;
    for (k, v) in st.of_kind("hook_writes").next().unwrap().get("writes").map_items() {
        let mine = Py::List(scope::hook_writes(game, k.int() as i32).iter().map(Py::str).collect());
        check(&mut out, &format!("hook_writes {}", k.int()), v, &mine);
        n += 1;
    }
    for (k, v) in st.of_kind("entry_writes").next().unwrap().get("writes").map_items() {
        let mine = Py::Tuple(scope::entry_writes(game, k.int() as i32).iter().map(Py::str).collect());
        check(&mut out, &format!("entry_writes {}", k.int()), v, &mine);
        n += 1;
    }
    (out, n)
}

/// A Builder for the oracle's scope `name` (room33, kitchen) as read_all's first pass starts it.
pub fn builder<'g>(game: &'g Game, st: &Static, name: &str) -> Builder<'g> {
    let l = st.named("builder", name);
    let start = Facts::from_py(l.get("start_items"));
    let rooms: Vec<i32> = l.get("rooms").items().iter().map(|r| r.int() as i32).collect();
    let doms = domains::build(game, &domains::meta_rooms(&root()), &start, &[]);
    let sc = Scope::of(game, &rooms, &start);
    Builder::new(game, sc, doms)
}

/// The Builder's machine-free parts for scope `name`: the fact table, every fact's kind and domain, the sentence
/// scripts, VAR_EGO's values, the inputs with their counts, the start events, the unknown writes' candidates.
pub fn compare_builder(game: &Game, st: &Static, name: &str) -> (Vec<String>, usize) {
    let l = st.named("builder", name);
    let mut b = builder(game, st, name);
    let mut out = Vec::new();
    check(&mut out, &format!("{name} table"), l.get("table"), &b.table.py());
    let mut nfacts = 0;
    for (k, v) in l.get("facts").dict_items() {
        let mine = Py::dict(vec![
            ("kind", Py::str(b.kind(k).as_str())),
            ("domain", b.domain(k).map_or(Py::None, |d| d.py())),
        ]);
        check(&mut out, &format!("{name} fact {k}"), v, &mine);
        nfacts += 1;
    }
    check(
        &mut out,
        &format!("{name} sentence_scripts"),
        l.get("sentence_scripts"),
        &Py::List(b.sentence_scripts().into_iter().map(Py::Int).collect()),
    );
    check(&mut out, &format!("{name} ego_values"), l.get("ego_values"), &Py::List(b.ego_values().into_iter().map(|v| v.py()).collect()));
    let jobs = b.inputs();
    check(&mut out, &format!("{name} inputs"), l.get("inputs"), &Py::List(jobs.iter().map(|j| j.py()).collect()));
    check(&mut out, &format!("{name} input counts"), l.get("input_counts"), &b.counts.py());
    b.counts.clear();
    let evs = b.start_events();
    check(&mut out, &format!("{name} start_events"), l.get("start_events"), &Py::List(evs.iter().map(|e| e.py()).collect()));
    check(&mut out, &format!("{name} start event counts"), l.get("start_event_counts"), &b.counts.py());
    let mut nt = 0;
    for x in l.get("unknown_targets").items() {
        let w = x.items()[0].items();
        let mine = b.unknown_targets(w[0].as_str(), w[1].as_str());
        let mine = mine.map_or(Py::None, |v| Py::List(v.into_iter().map(Py::str).collect()));
        check(&mut out, &format!("{name} unknown_targets {} {}", w[0].as_str(), w[1].as_str()), &x.items()[1], &mine);
        nt += 1;
    }
    (out, nfacts + jobs.len() + evs.len() + nt)
}

/// The golden reading's info fields (name, start, workers, sources, domains_goals), copied into the Rust file.
pub fn golden_info(golden: &Path) -> Vec<(String, Py)> {
    let first = crate::gz_lines(golden).next().unwrap();
    let r = Py::from_text(&first);
    let r = r.get("reading");
    ["domains_goals", "name", "sources", "start", "workers"].iter().map(|k| (k.to_string(), r.get(k).clone())).collect()
}

/// The reading `name` by the Rust Builder over the captured machine runs; returns the Builder (its records and
/// counters as the reading left them) and the number of runs played back.
pub fn replay<'g>(game: &'g Game, st: &Static, name: &str) -> (Builder<'g>, usize) {
    replay_into(game, st, name, None)
}

/// `replay`, handing the records to `sink` as they are made.
pub fn replay_into<'g>(
    game: &'g Game,
    st: &Static,
    name: &str,
    sink: Option<&mut dyn crate::builder::RecordSink>,
) -> (Builder<'g>, usize) {
    let mut b = builder(game, st, name);
    let mut m = Replay::open(&oracle_dir().join(format!("capture-{name}.jsonl.gz")));
    b.read_all_into(&mut m, true, None, 2, sink);
    let runs = m.runs;
    assert!(m.done(), "captured runs left after the reading");
    (b, runs)
}

/// The Builder's reading against golden.py's file, line by line: the first differences (at most `max`) and the
/// number of lines compared.
pub fn compare_reading(b: &Builder, golden: &Path, max: usize) -> (Vec<String>, usize) {
    let info = golden_info(golden);
    let mut out = Vec::new();
    let mut gold = crate::gz_lines(golden);
    let mut n = 0;
    for mine in b.reading_lines(&info) {
        n += 1;
        match gold.next() {
            None => {
                out.push(format!("line {n}: the golden file ends"));
                break;
            }
            Some(g) => {
                if let Some(d) = first_diff(&g, &mine) {
                    out.push(format!("line {n}: {d}"));
                    if out.len() >= max {
                        break;
                    }
                }
            }
        }
    }
    if out.is_empty() && gold.next().is_some() {
        out.push(format!("the golden file has more than {n} lines"));
    }
    (out, n)
}

/// The records of a golden reader file (each must write back to its own line: the record type holds it whole).
pub fn golden_records(path: &Path) -> (Vec<Record>, Vec<String>) {
    let mut recs = Vec::new();
    let mut bad = Vec::new();
    for l in crate::gz_lines(path) {
        let p = Py::from_text(&l);
        if p.get("kind").as_str() != "record" {
            continue;
        }
        let r = Record::from_py(p.get("record"));
        let back = Py::dict(vec![("kind", Py::str("record")), ("index", p.get("index").clone()), ("record", r.py())]).text();
        if back != l && bad.len() < 5 {
            bad.push(first_diff(&l, &back).unwrap_or_default());
        }
        recs.push(r);
    }
    (recs, bad)
}

/// test_reader.py's kitchen task: the goal, the starts (s17, replan) and the label "no 211" leaves out.
pub struct KitchenTask {
    pub goal: Vec<(String, Labels)>,
    pub s17: Facts,
    pub replan: Facts,
    pub no211: String,
}

pub fn kitchen_task(st: &Static) -> KitchenTask {
    let l = st.of_kind("kitchen_task").next().unwrap();
    KitchenTask {
        goal: l
            .get("goal")
            .items()
            .iter()
            .map(|g| {
                let g = g.items();
                (g[0].as_str().to_string(), g[1].items().iter().map(|v| Label::Int(v.int())).collect())
            })
            .collect(),
        s17: Facts::from_py(l.get("starts").get("s17")),
        replan: Facts::from_py(l.get("starts").get("replan")),
        no211: l.get("no211").as_str().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The comparisons find a difference where there is one (they are not vacuous).
    #[test]
    fn first_diff_finds_differences() {
        assert_eq!(first_diff(r#"{"a":1}"#, r#"{"a":1}"#), None);
        assert_eq!(first_diff(r#"{"a":[1,2]}"#, r#"{"a":[1,3]}"#).unwrap(), "a[1]: golden 2, candidate 3");
        assert!(first_diff(r#"{"a":true}"#, r#"{"a":1}"#).is_some());
        assert!(first_diff(r#"{"a":{"b":1}}"#, r#"{"a":{"c":1}}"#).unwrap().starts_with("a.b"));
        assert!(first_diff(r#"[1]"#, r#"[1,2]"#).unwrap().contains("length"));
    }
}
