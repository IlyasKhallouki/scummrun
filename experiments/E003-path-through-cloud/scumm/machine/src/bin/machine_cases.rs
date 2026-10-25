//! machine-cases: the machine beyond the goldens (wide.py). Reads DIR/cases.json, runs every case with the Rust
//! machine of its features (row 0 to 5) and switches on the case's state (wide.py's synthetic changes applied; an
//! `arm` case gets its own state, wide.py `armed`), and writes DIR/rust.jsonl.gz and DIR/rust.trace.jsonl.gz in
//! wide.py's layout.
//!
//! usage: machine-cases DIR [--no-trace] [--costs]
//!   --no-trace: time the machine alone; rust.trace.jsonl.gz holds case lines only.
//!   --costs: run untraced and write DIR/rust-costs.json alone (no rust files): per case its machine time (ms) and
//!   its paths' list entries ("items": assumptions, notes, unknown writes, errors, turns, conflicts and writes, wide.py
//!   --max-items' count), so that wide.py --from-costs can keep the cases Python can afford without encoding them.
//! then:  golden.py compare-file DIR/python.jsonl.gz DIR/rust.jsonl.gz (and the two trace files)

use flate2::{Compression, GzBuilder};
use scumm_ir::json::J;
use scumm_machine::enc;
use scumm_ir::UnitKey;
use scumm_machine::harness::{json_to_j, load_pres};
use scumm_machine::path::Owned;
use scumm_machine::trace::{Sink, Tracer};
use scumm_machine::{predict, FactOracle, Features, Machine, Switches};
use serde_json::Value;
use std::cell::RefCell;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;

/// A trace sink that appends to a shared buffer.
struct Shared(Rc<RefCell<Vec<u8>>>);

impl Write for Shared {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Sink for Shared {
    fn close(self: Box<Self>) -> std::io::Result<()> {
        Ok(())
    }
}

fn main() {
    let dir = PathBuf::from(std::env::args().nth(1).expect("usage: machine-cases DIR [--no-trace]"));
    let flags: Vec<String> = std::env::args().skip(2).collect();
    let costs = flags.iter().any(|a| a == "--costs");
    let tracing = !costs && !flags.iter().any(|a| a == "--no-trace");
    let mut cost_rows: Vec<String> = Vec::new();
    let mut slow: Vec<(f64, usize)> = Vec::new();
    let root = scumm_ir::default_root();
    let v: Value = serde_json::from_slice(&std::fs::read(dir.join("cases.json")).expect("cases.json")).unwrap();
    let kinds: Vec<String> = v["kinds"].as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect();
    let mut pres = load_pres(std::path::Path::new(v["groundtruth"].as_str().unwrap())).expect("ground truth");
    // wide.py's synthetic states (`perturb`): --set-prefix, then --set; a key set to 0 leaves the state
    let pairs = |name: &str| -> Vec<(String, i32)> {
        v.get(name).and_then(|x| x.as_array()).map_or(Vec::new(), |a| {
            a.iter().map(|p| (p[0].as_str().unwrap().to_string(), p[1].as_i64().unwrap() as i32)).collect()
        })
    };
    let (set_prefix, sets) = (pairs("set_prefix"), pairs("sets"));
    // wide.py's --unheld: keys the oracle reads as not held
    let unheld: Vec<scumm_ir::Fact> = v.get("unheld").and_then(|x| x.as_array()).map_or(Vec::new(), |a| {
        a.iter().map(|k| k.as_str().unwrap().parse().unwrap_or_else(|e| panic!("--unheld {k}: {e}"))).collect()
    });
    for pre in pres.values_mut() {
        for (p, x) in &set_prefix {
            for (k, val) in pre.iter_mut() {
                if k.to_string().starts_with(p.as_str()) {
                    *val = *x;
                }
            }
        }
        for (k, x) in &sets {
            pre.insert(k.parse().unwrap_or_else(|e| panic!("--set {k}: {e}")), *x);
        }
        pre.retain(|_, val| *val != 0);
    }
    let game = scumm_ir::Game::load(&root).expect("game");
    let names: Vec<String> = match v.get("features") {
        Some(f) => f.as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect(),
        None => vec!["own_entry".to_string()],
    };
    let f = Features::parse(&names).unwrap_or_else(|e| panic!("{e}"));
    let switches: Vec<String> = match v.get("switches") {
        Some(s) => s.as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect(),
        None => Vec::new(),
    };
    let sw = Switches::parse(&switches).unwrap_or_else(|e| panic!("{e}"));
    let mut m = Machine::new(&game, f, sw);
    if let Some(n) = v.get("path_limit").and_then(|n| n.as_u64()) {
        m.path_limit = n as usize;
    }
    if let Some(n) = v.get("step_limit").and_then(|n| n.as_u64()) {
        m.step_limit = n as u32;
    }
    let gz = |name: &str| {
        let f = std::fs::File::create(dir.join(name)).unwrap();
        GzBuilder::new().mtime(0).write(BufWriter::new(f), Compression::new(6))
    };
    let (mut w, mut t) = (gz("rust.jsonl.gz"), gz("rust.trace.jsonl.gz"));
    let mut oracles: rustc_hash::FxHashMap<i64, FactOracle> = Default::default();
    let mut machine_s = 0.0;
    let cases = v["cases"].as_array().unwrap();
    let t0 = Instant::now();
    for (k, c) in cases.iter().enumerate() {
        let row = c["row"].as_i64().unwrap();
        // wide.py --arm: the state with one override armed (the other esc: keys leave it unless "keep")
        let mut own;
        let o = match c.get("arm").and_then(|k| k.as_str()) {
            Some(arm) => {
                let key: scumm_ir::Fact = arm.parse().unwrap_or_else(|e| panic!("arm {arm}: {e}"));
                let keep = c["keep"].as_bool().unwrap();
                let mut state = pres[&row].clone();
                state.retain(|k, _| keep || !matches!(k, scumm_ir::Fact::Esc(..)));
                state.insert(key, 1);
                own = FactOracle::new(state, Some(kinds.clone())).with_unheld(unheld.clone());
                &mut own
            }
            None => oracles
                .entry(row)
                .or_insert_with(|| FactOracle::new(pres[&row].clone(), Some(kinds.clone())).with_unheld(unheld.clone())),
        };
        let mut head = vec![("kind", enc::s("case")), ("index", J::Int(k as i64))];
        for (key, x) in c.as_object().unwrap() {
            head.push((key.as_str(), json_to_j(x)));
        }
        let head = enc::line(head) + "\n";
        w.write_all(head.as_bytes()).unwrap();
        t.write_all(head.as_bytes()).unwrap();
        let buf = Rc::new(RefCell::new(Vec::new()));
        if tracing {
            m.trace = Some(Tracer::new(Box::new(Shared(buf.clone()))));
        }
        let s = Instant::now();
        let run = match c.get("unit") {
            Some(u) => {
                let u: Vec<&Value> = u.as_array().unwrap().iter().collect();
                let n = |i: usize| u[i].as_i64().unwrap() as i32;
                let key = match u[0].as_str().unwrap() {
                    "global" => UnitKey::Global(n(1)),
                    "local" => UnitKey::Local(n(1), n(2)),
                    "entry" => UnitKey::Entry(n(1)),
                    "exit" => UnitKey::Exit(n(1)),
                    k => panic!("unit kind {k}"),
                };
                let pc = c["pc"].as_i64().unwrap() as u32;
                m.run(o, key, pc, None, Owned::Bool(false), None)
            }
            None => match c.get("sentence") {
                Some(st) => {
                    let st = st.as_array().unwrap();
                    let n = |i: usize| st[i].as_i64().map(|x| x as i32);
                    m.run_sentence(o, n(0).unwrap(), n(1).unwrap(), n(2))
                }
                None if c.get("wait").is_some() => m.run_wait(o),
                None if c.get("click").is_some() => {
                    m.run_verb(o, c["click"].as_i64().unwrap() as i32, scumm_machine::machine::LEFT_CLICK)
                }
                None if c.get("esc").is_some() => m.run_esc(o),
                None => m.run_entry(o, c["obj"].as_i64().unwrap() as i32, c["verb"].as_i64().unwrap() as i32, None),
            },
        };
        let e = s.elapsed().as_secs_f64();
        machine_s += e;
        slow.push((e, k));
        if let Some(tr) = m.trace.take() {
            tr.finish().unwrap();
        }
        if costs {
            let items: usize = run
                .paths
                .iter()
                .map(|p| {
                    p.assumptions.len()
                        + p.notes.len()
                        + p.unknown_writes.len()
                        + p.errors.len()
                        + p.turns.len()
                        + p.conflicts.len()
                        + p.writes.len()
                })
                .sum();
            cost_rows.push(format!("{{\"index\":{k},\"ms\":{:.3},\"items\":{items}}}", e * 1e3));
            continue;
        }
        t.write_all(&buf.borrow()).unwrap();
        let mut lines = vec![enc::line(vec![("kind", enc::s("run")), ("run", enc::run(&run))])];
        for (i, p) in run.paths.iter().enumerate() {
            lines.push(enc::line(vec![
                ("kind", enc::s("path")),
                ("index", J::Int(i as i64)),
                ("path", enc::path(&game, &run.vals, p)),
            ]));
        }
        lines.push(enc::line(vec![("kind", enc::s("predict")), ("predict", predict(&run, o))]));
        for l in lines {
            w.write_all(l.as_bytes()).unwrap();
            w.write_all(b"\n").unwrap();
        }
    }
    w.finish().unwrap().flush().unwrap();
    t.finish().unwrap().flush().unwrap();
    if costs {
        let _ = std::fs::remove_file(dir.join("rust.jsonl.gz"));
        let _ = std::fs::remove_file(dir.join("rust.trace.jsonl.gz"));
        let text = format!("{{\"cases\":[\n{}\n]}}\n", cost_rows.join(",\n"));
        std::fs::write(dir.join("rust-costs.json"), text).expect("rust-costs.json");
    }
    slow.sort_by(|a, b| b.0.total_cmp(&a.0));
    let top: Vec<String> = slow.iter().take(3).map(|(e, k)| format!("case {k} {:.3} ms", e * 1e3)).collect();
    eprintln!(
        "{} cases: machine {machine_s:.3} s (slowest {}), all {:.2} s; peak RSS {} MB",
        cases.len(),
        top.join(", "),
        t0.elapsed().as_secs_f64(),
        scumm_ir::peak_rss_kb().unwrap_or(0) / 1024
    );
}
