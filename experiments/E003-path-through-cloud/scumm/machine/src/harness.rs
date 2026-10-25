//! What the runner needs around the machine: the ground truth's pre states (groundtruth.py `load`), the
//! jobs file `candidate.py prep` writes (check.py's header per move: its label, status and action), the
//! action's machine entry point (check.py `run_model`), and one move's candidate lines.
//!
//! check.py's scoring itself (the `check` line) stays in Python (`candidate.py check`): it holds the
//! route's own data (run B's VIA moves), and it reads nothing from a run but the `run` and `path` lines.

use crate::enc;
use crate::machine::{predict, Features, Machine};
use crate::oracle::FactOracle;
use crate::path::{Entry, Frame, Owned, Path, Run, NONE_UNIT};
use rustc_hash::FxHashMap;
use scumm_ir::json::J;
use scumm_ir::{Fact, FactMap};
use serde_json::Value;
use std::io;

/// One counted move of a jobs file.
#[derive(Clone, Debug)]
pub struct Job {
    pub label: String,
    pub i: i64,
    /// the move line's `move` object, as golden.py writes it
    pub head: Value,
    pub action: Option<Action>,
}

/// check.py's `actions.Action` of a move: its kind (sentence, dialog_line, skip_cutscene, wait), verb,
/// objects, and whether a sentence is a walk_to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Action {
    pub kind: String,
    pub verb: Option<i32>,
    pub a: Option<i32>,
    pub b: Option<i32>,
    pub walk: bool,
}

/// A jobs file (`candidate.py prep`).
#[derive(Clone, Debug)]
pub struct Jobs {
    pub run: String,
    pub features: Vec<String>,
    pub switches: Vec<String>,
    pub key: String,
    pub kinds: Vec<String>,
    pub groundtruth: String,
    pub moves: Vec<Job>,
}

fn int_of(v: &Value) -> Option<i32> {
    v.as_i64().map(|n| i32::try_from(n).expect("an int32"))
}

fn strs(v: &Value) -> Vec<String> {
    v.as_array().expect("a list").iter().map(|s| s.as_str().expect("a str").to_string()).collect()
}

impl Jobs {
    pub fn read(path: &std::path::Path) -> io::Result<Jobs> {
        let v: Value = serde_json::from_slice(&std::fs::read(path)?)?;
        let moves = v["moves"]
            .as_array()
            .expect("moves")
            .iter()
            .map(|m| Job {
                label: m["label"].as_str().expect("label").to_string(),
                i: m["i"].as_i64().expect("i"),
                head: m["move"].clone(),
                action: (!m["machine_action"].is_null()).then(|| {
                    let a = &m["machine_action"];
                    Action {
                        kind: a["kind"].as_str().expect("kind").to_string(),
                        verb: int_of(&a["verb"]),
                        a: int_of(&a["a"]),
                        b: int_of(&a["b"]),
                        walk: a["walk"].as_bool().expect("walk"),
                    }
                }),
            })
            .collect();
        Ok(Jobs {
            run: v["run"].as_str().expect("run").to_string(),
            features: strs(&v["features"]),
            switches: strs(&v["switches"]),
            key: v["key"].as_str().expect("key").to_string(),
            kinds: strs(&v["kinds"]),
            groundtruth: v["groundtruth"].as_str().expect("groundtruth").to_string(),
            moves,
        })
    }
}

/// groundtruth.py `load`'s pre state of every row, by its `i`: the first row's `start`, then each row's
/// post (`patch(pre, post_diff)`) is the next row's pre.
pub fn load_pres(path: &std::path::Path) -> io::Result<FxHashMap<i64, FactMap>> {
    let text = std::fs::read_to_string(path)?;
    let mut out = FxHashMap::default();
    let mut pre: Option<FactMap> = None;
    let key = |k: &str| k.parse::<Fact>().unwrap_or_else(|e| panic!("ground truth: {e}"));
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let r: Value = serde_json::from_str(line)?;
        if let Some(start) = r.get("start") {
            let m = start.as_object().expect("start is a map");
            pre = Some(m.iter().map(|(k, v)| (key(k), int_of(v).expect("an int fact"))).collect());
        }
        let cur = pre.take().expect("the first row has a start");
        let mut post = cur.clone();
        for (k, d) in r["post_diff"].as_object().expect("post_diff") {
            let new = int_of(&d[1]).expect("an int fact");
            if new != 0 {
                post.insert(key(k), new);
            } else {
                post.remove(&key(k));
            }
        }
        out.insert(r["i"].as_i64().expect("i"), cur);
        pre = Some(post);
    }
    Ok(out)
}

/// check.py `run_model`: the machine's entry point for the action under the machine's features; an
/// action whose feature is off gets check.py's one path that ends "no effect at this row".
pub fn run_model(m: &mut Machine, o: &FactOracle, act: &Action) -> Run {
    let f: Features = m.features;
    match act.kind.as_str() {
        "sentence" => {
            let (verb, a) = (act.verb.expect("a verb"), act.a.expect("object A"));
            if f.dispatch {
                // a walk_to is the sentence with the walk verb and no B
                return m.run_sentence(o, verb, a, act.b);
            }
            m.run_entry(o, a, verb, None)
        }
        "wait" if f.settle => m.run_wait(o),
        "dialog_line" if f.dialog => unreachable!("dialog is not ported"),
        "skip_cutscene" if f.esc => unreachable!("esc is not ported"),
        kind => {
            let frame = Frame::new(NONE_UNIT, Some(0), false, Owned::Bool(false), None);
            let mut p = Path::new(vec![Entry::Frame(frame)]);
            p.end = Some("no effect at this row");
            Run { paths: vec![p], start: format!("{kind}: nothing runs at this row"), ..Default::default() }
        }
    }
}

/// A serde value as a canonical J (golden.py's encoding of plain JSON data).
pub fn json_to_j(v: &Value) -> J {
    match v {
        Value::Null => J::Null,
        Value::Bool(b) => J::Bool(*b),
        Value::Number(n) => J::Int(n.as_i64().expect("an int (golden data has no floats)")),
        Value::String(s) => J::Str(s.clone()),
        Value::Array(a) => J::Arr(a.iter().map(json_to_j).collect()),
        Value::Object(m) => J::Obj(m.iter().map(|(k, x)| (k.clone(), json_to_j(x))).collect()),
    }
}

/// The candidate's lines of a move as the machine gives them: move, run, path per path, predict (the
/// check line is `candidate.py check`'s). Each with its "\n".
pub fn move_lines(game: &scumm_ir::Game, job: &Job, run: Option<(&Run, &FactOracle)>) -> Vec<String> {
    let mut out = vec![enc::line(vec![("kind", enc::s("move")), ("move", json_to_j(&job.head))])];
    if let Some((r, o)) = run {
        out.push(enc::line(vec![("kind", enc::s("run")), ("run", enc::run(r))]));
        for (k, p) in r.paths.iter().enumerate() {
            out.push(enc::line(vec![
                ("kind", enc::s("path")),
                ("index", J::Int(k as i64)),
                ("path", enc::path(game, &r.vals, p)),
            ]));
        }
        out.push(enc::line(vec![("kind", enc::s("predict")), ("predict", predict(r, o))]));
    }
    for l in &mut out {
        l.push('\n');
    }
    out
}
