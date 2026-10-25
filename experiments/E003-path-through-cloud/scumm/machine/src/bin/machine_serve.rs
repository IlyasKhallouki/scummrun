//! machine-serve: the Rust machine as check.py's `run_model`, for `check_rust.py` (PORT-NOTES.md, "The model check
//! on Rust").
//!
//! usage: machine-serve --features own_entry,dispatch,... [--switches const_polls,...]
//!
//! It loads the game once and writes one hello line on stdout: {"features", "switches", "path_limit", "step_limit",
//! "round_limit"} (check_rust.py checks them against the Python Machine it stands in for). Then it reads requests on
//! stdin, one JSON line each:
//!
//!     {"action": {"kind", "verb", "a", "b", "walk"}, "pre": {fact: int, ...}, "kinds": [family, ...] or null,
//!      "canonical": bool (optional), "verify": bool (optional)}
//!
//! `pre` and `kinds` are the `model.FactOracle`'s map and families, `action` check.py's `actions.Action`. It runs
//! `harness::run_model` (check.py's `run_model` dispatch) and answers with a header line
//!
//!     {"bytes": N, "canonical_bytes": M, "machine_s", "encode_s", "paths", "cpu_s", "peak_rss_kb"}
//!
//! then N bytes: golden.py's `run` line and one `path` line per path (FORMAT.txt), each ending in "\n", with lists of
//! plain tuples sent by columns (`compact`) unless "canonical" is true; then, when "verify" is true, M bytes: the same
//! lines in golden.py's canonical form (M is 0 otherwise). `cpu_s` and `peak_rss_kb` are this process's totals so
//! far. It exits at the end of stdin.

use rustc_hash::FxHashMap;
use scumm_ir::json::J;
use scumm_ir::Slot;
use scumm_ir::{Fact, FactMap};
use scumm_machine::harness::{run_model, Action};
use scumm_machine::{enc, FactOracle, Features, Machine, Path, Switches};
use serde_json::Value;
use std::io::{BufRead, BufWriter, Write};
use std::time::Instant;

fn usage() -> ! {
    eprintln!("usage: machine-serve --features a,b,... [--switches x,...]");
    std::process::exit(2)
}

fn names(s: &str) -> Vec<String> {
    s.split(',').map(str::trim).filter(|x| !x.is_empty()).map(String::from).collect()
}

fn int_of(v: &Value) -> Option<i32> {
    v.as_i64().map(|n| i32::try_from(n).expect("an int32"))
}

/// CPU seconds of this process so far (/proc/self/schedstat: one thread, nanoseconds on the CPU).
fn cpu_s() -> f64 {
    std::fs::read_to_string("/proc/self/schedstat")
        .ok()
        .and_then(|s| s.split_whitespace().next().and_then(|n| n.parse::<u64>().ok()))
        .map_or(0.0, |ns| ns as f64 / 1e9)
}

fn action_of(a: &Value) -> Action {
    Action {
        kind: a["kind"].as_str().expect("action kind").to_string(),
        verb: int_of(&a["verb"]),
        a: int_of(&a["a"]),
        b: int_of(&a["b"]),
        walk: a["walk"].as_bool().expect("action walk"),
    }
}

fn oracle_of(req: &Value) -> FactOracle {
    let pre: FactMap = req["pre"]
        .as_object()
        .expect("pre is a map")
        .iter()
        .map(|(k, v)| {
            let key = k.parse::<Fact>().unwrap_or_else(|e| panic!("pre: {e}"));
            (key, int_of(v).unwrap_or_else(|| panic!("pre: {k} is not an int")))
        })
        .collect();
    let kinds = match &req["kinds"] {
        Value::Null => None,
        v => Some(v.as_array().expect("kinds").iter().map(|s| s.as_str().expect("a family").to_string()).collect()),
    };
    FactOracle::new(pre, kinds)
}

/// The wire's one change to golden.py's lines, which check_rust.py's decoding undoes: a Python list (a JSON array
/// that is not the payload of a $tuple, $set or $map, nor a $map pair) of two or more tuples that all have the same
/// length n >= 1 and hold only null, bool, int or str is sent by columns, {"$tuples": [column 0, ..., column n-1]};
/// a column of strings only is {"$strs": [table, width, indices]} (`strs_col`), each string once. A path's turns,
/// assumptions, notes and the like shrink to a fraction, and Python builds them with zip instead of one object per
/// tuple. A path's turns (millions in the biggest moves) are built in that form directly (`turns_cols`).
fn compact(j: &mut J) {
    match j {
        J::Arr(items) => {
            for x in items.iter_mut() {
                compact(x);
            }
            if let Some(c) = columns(items) {
                *j = c;
            }
        }
        J::Obj(m) => {
            if m.len() == 1 {
                let (k, v) = m.iter_mut().next().expect("one key");
                let payload = match k.as_str() {
                    "$tuple" | "$set" => Some(false),
                    "$map" => Some(true),
                    "$tuples" | "$strs" => return, // already by columns
                    _ => None,
                };
                if let (Some(pairs), J::Arr(xs)) = (payload, v) {
                    for x in xs.iter_mut() {
                        match x {
                            J::Arr(kv) if pairs => kv.iter_mut().for_each(compact),
                            x => compact(x),
                        }
                    }
                    return;
                }
            }
            for v in m.values_mut() {
                compact(v);
            }
        }
        _ => {}
    }
}

/// The items of a {"$tuple": [...]} whose items are all plain values.
fn plain_tuple(x: &J) -> Option<&Vec<J>> {
    let J::Obj(m) = x else { return None };
    if m.len() != 1 {
        return None;
    }
    let J::Arr(t) = m.get("$tuple")? else { return None };
    t.iter().all(|v| matches!(v, J::Null | J::Bool(_) | J::Int(_) | J::Str(_))).then_some(t)
}

fn one(k: &str, v: J) -> J {
    J::Obj(std::iter::once((k.to_string(), v)).collect())
}

fn columns(items: &[J]) -> Option<J> {
    if items.len() < 2 {
        return None;
    }
    let n = plain_tuple(&items[0])?.len();
    let mut rows = Vec::with_capacity(items.len());
    for x in items {
        let t = plain_tuple(x)?;
        if t.len() != n {
            return None;
        }
        rows.push(t);
    }
    if n == 0 {
        return None;
    }
    let mut cols = Vec::with_capacity(n);
    for c in 0..n {
        if rows.iter().all(|t| matches!(t[c], J::Str(_))) {
            let mut table = Vec::new();
            let mut ids: FxHashMap<&str, u32> = FxHashMap::default();
            let mut index = Vec::with_capacity(rows.len());
            for t in &rows {
                let J::Str(s) = &t[c] else { unreachable!() };
                let next = ids.len() as u32;
                let id = *ids.entry(s.as_str()).or_insert_with(|| {
                    table.push(J::Str(s.clone()));
                    next
                });
                index.push(id);
            }
            cols.push(strs_col(table, &index));
        } else {
            cols.push(J::Arr(rows.iter().map(|t| t[c].clone()).collect()));
        }
    }
    Some(one("$tuples", J::Arr(cols)))
}

/// A column of strings: {"$strs": [table, width, indices]}, the indices into the table as little-endian u16 (width 2)
/// or u32 (width 4) in base64, which Python reads with `array` instead of parsing one JSON int each.
fn strs_col(table: Vec<J>, ids: &[u32]) -> J {
    let wide = table.len() > 1 << 16;
    let mut bytes = Vec::with_capacity(ids.len() * if wide { 4 } else { 2 });
    for &i in ids {
        if wide {
            bytes.extend_from_slice(&i.to_le_bytes());
        } else {
            bytes.extend_from_slice(&(i as u16).to_le_bytes());
        }
    }
    one("$strs", J::Arr(vec![J::Arr(table), J::Int(if wide { 4 } else { 2 }), J::Str(b64(&bytes))]))
}

/// Standard base64, with padding.
fn b64(data: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(A[(n >> 18) as usize & 63] as char);
        out.push(A[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { A[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { A[n as usize & 63] as char } else { '=' });
    }
    out
}

/// A path's turns, (unit name, how) per turn, as `compact` would send them, built from the path without one J per
/// turn.
fn turns_cols(p: &Path) -> J {
    if p.turns.len() < 2 {
        return J::Arr(p.turns.iter().map(|(n, how)| enc::tuple(vec![enc::slot(*n), enc::s(how)])).collect());
    }
    let (mut names, mut hows): (FxHashMap<Slot, u32>, FxHashMap<&str, u32>) = Default::default();
    let (mut name_t, mut how_t) = (Vec::new(), Vec::new());
    let (mut name_i, mut how_i) = (Vec::with_capacity(p.turns.len()), Vec::with_capacity(p.turns.len()));
    for &(n, how) in p.turns.iter() {
        let next = names.len() as u32;
        name_i.push(*names.entry(n).or_insert_with(|| {
            name_t.push(enc::slot(n));
            next
        }));
        let next = hows.len() as u32;
        how_i.push(*hows.entry(how).or_insert_with(|| {
            how_t.push(enc::s(how));
            next
        }));
    }
    one("$tuples", J::Arr(vec![strs_col(name_t, &name_i), strs_col(how_t, &how_i)]))
}

/// One line's text, compacted for the wire unless `canonical`.
fn wire_line(pairs: Vec<(&str, J)>, canonical: bool) -> String {
    let mut j = J::Obj(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect());
    if !canonical {
        compact(&mut j);
    }
    let mut s = String::new();
    j.write(&mut s);
    s
}

fn main() {
    let (mut features, mut switches) = (String::new(), String::new());
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--features" => features = it.next().unwrap_or_else(|| usage()),
            "--switches" => switches = it.next().unwrap_or_else(|| usage()),
            _ => usage(),
        }
    }
    let (fnames, snames) = (names(&features), names(&switches));
    let f = Features::parse(&fnames).unwrap_or_else(|e| {
        eprintln!("machine-serve: {e}");
        std::process::exit(1)
    });
    let s = Switches::parse(&snames).unwrap_or_else(|e| {
        eprintln!("machine-serve: {e}");
        std::process::exit(1)
    });
    let root = scumm_ir::default_root();
    let game = scumm_ir::Game::load(&root).expect("the game");
    let mut m = Machine::new(&game, f, s);
    let stdout = std::io::stdout();
    let mut out = BufWriter::with_capacity(1 << 20, stdout.lock());
    let strs = |v: &[String]| J::Arr(v.iter().map(|x| J::Str(x.clone())).collect());
    let hello = enc::line(vec![
        ("features", strs(&fnames)),
        ("switches", strs(&snames)),
        ("path_limit", J::Int(m.path_limit as i64)),
        ("step_limit", J::Int(m.step_limit as i64)),
        ("round_limit", J::Int(m.round_limit as i64)),
    ]);
    writeln!(out, "{hello}").and_then(|_| out.flush()).expect("stdout");
    let stdin = std::io::stdin();
    let mut text = String::new();
    let mut lock = stdin.lock();
    loop {
        text.clear();
        if lock.read_line(&mut text).expect("stdin") == 0 {
            break;
        }
        if text.trim().is_empty() {
            continue;
        }
        let req: Value = serde_json::from_str(&text).expect("a request line");
        let act = action_of(&req["action"]);
        let o = oracle_of(&req);
        let t = Instant::now();
        let run = run_model(&mut m, &o, &act);
        let machine_s = t.elapsed().as_secs_f64();
        let t = Instant::now();
        let wire = !req["canonical"].as_bool().unwrap_or(false);
        let verify = req["verify"].as_bool().unwrap_or(false);
        let (mut body, mut canon) = (String::new(), String::new());
        let mut put = |line: &dyn Fn(J) -> Vec<(&'static str, J)>, wire_j: J, canon_j: Option<J>| {
            if let Some(cj) = canon_j {
                canon.push_str(&enc::line(line(cj)));
                canon.push('\n');
            }
            body.push_str(&wire_line(line(wire_j), !wire));
            body.push('\n');
        };
        let run_line = |j: J| vec![("kind", enc::s("run")), ("run", j)];
        put(&run_line, enc::run(&run), verify.then(|| enc::run(&run)));
        for (k, p) in run.paths.iter().enumerate() {
            let path_line = |j: J| vec![("kind", enc::s("path")), ("index", J::Int(k as i64)), ("path", j)];
            let pj = if wire {
                let mut pj = enc::path_with(&game, &run.vals, p, false);
                if let J::Obj(m) = &mut pj {
                    m.insert("turns".to_string(), turns_cols(p));
                }
                pj
            } else {
                enc::path(&game, &run.vals, p)
            };
            put(&path_line, pj, verify.then(|| enc::path(&game, &run.vals, p)));
        }
        let paths = run.paths.len();
        drop(run);
        let encode_s = t.elapsed().as_secs_f64();
        let head = format!(
            "{{\"bytes\":{},\"canonical_bytes\":{},\"machine_s\":{machine_s:.6},\"encode_s\":{encode_s:.6},\"paths\":{paths},\"cpu_s\":{:.6},\"peak_rss_kb\":{}}}",
            body.len(),
            canon.len(),
            cpu_s(),
            scumm_ir::peak_rss_kb().unwrap_or(0)
        );
        // a closed pipe (the Python side stopped) ends the server
        let sent = writeln!(out, "{head}")
            .and_then(|_| out.write_all(body.as_bytes()))
            .and_then(|_| out.write_all(canon.as_bytes()))
            .and_then(|_| out.flush());
        if sent.is_err() {
            break;
        }
    }
}
