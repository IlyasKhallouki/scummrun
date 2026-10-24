//! The oracle: this crate's dump against dump_ir.py's, byte for byte (scumm/README.md).
//!
//! The Python dumps live in ROOT/out/scumm-ir/. A test runs dump_ir.py (ROOT/.venv/bin/python) when they
//! are missing or when a Python source they came from changed since (ir-python.sources.json). It then
//! builds the same lines here and compares them with the Python text, line by line. A line that
//! differs is reported by its unit and the field paths that differ.

use flate2::read::GzDecoder;
use scumm_ir::{default_root, dump, read, Game};
use serde_json::Value;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

fn experiment() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

fn out_dir() -> PathBuf {
    default_root().join("out").join("scumm-ir")
}

/// Whether the Python dumps exist and came from the Python sources as they are now.
fn fresh() -> bool {
    let out = out_dir();
    let Ok(text) = std::fs::read_to_string(out.join("ir-python.sources.json")) else { return false };
    let Ok(recorded): Result<Value, _> = serde_json::from_str(&text) else { return false };
    let Some(recorded) = recorded.as_object() else { return false };
    let all_there = ["", "-noblocks", "-synthetic"].iter().all(|v| out.join(format!("ir-python{v}.jsonl.gz")).exists());
    all_there
        && recorded.iter().all(|(name, sha)| {
            std::fs::read(experiment().join(name))
                .map(|b| sha1_smol::Sha1::from(b).digest().to_string() == sha.as_str().unwrap_or(""))
                .unwrap_or(false)
        })
}

/// Runs dump_ir.py once per test process when its dumps are missing or stale.
fn python_dumps() -> PathBuf {
    static DONE: OnceLock<()> = OnceLock::new();
    DONE.get_or_init(|| {
        if fresh() {
            return;
        }
        let python = default_root().join(".venv").join("bin").join("python");
        eprintln!("oracle: running dump_ir.py (missing or stale Python dumps)");
        let status = std::process::Command::new(python)
            .arg("dump_ir.py")
            .current_dir(experiment())
            .status()
            .expect("run dump_ir.py with ROOT/.venv/bin/python");
        assert!(status.success(), "dump_ir.py failed");
        assert!(fresh(), "dump_ir.py ran but its dumps still do not match the sources");
    });
    out_dir()
}

fn gunzip(path: &Path) -> String {
    let mut s = String::new();
    GzDecoder::new(std::fs::File::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())))
        .read_to_string(&mut s)
        .unwrap();
    s
}

/// The field paths where `a` and `b` differ.
fn diff(a: &Value, b: &Value, path: String, out: &mut Vec<String>) {
    if out.len() > 12 {
        return;
    }
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let keys: std::collections::BTreeSet<&String> = x.keys().chain(y.keys()).collect();
            for k in keys {
                match (x.get(k), y.get(k)) {
                    (Some(p), Some(q)) if p == q => {}
                    (Some(p), Some(q)) => diff(p, q, format!("{path}/{k}"), out),
                    (p, q) => out.push(format!("{path}/{k}: python {p:?}, rust {q:?}")),
                }
            }
        }
        (Value::Array(x), Value::Array(y)) => {
            if x.len() != y.len() {
                out.push(format!("{path}: python {} items, rust {}", x.len(), y.len()));
            }
            for (i, (p, q)) in x.iter().zip(y).enumerate() {
                if p != q {
                    diff(p, q, format!("{path}[{i}]"), out);
                }
            }
        }
        _ => {
            let s = |v: &Value| v.to_string().chars().take(100).collect::<String>();
            out.push(format!("{path}: python {}, rust {}", s(a), s(b)));
        }
    }
}

/// Compares the Python dump's text with the Rust lines; panics with a report of every line that differs.
fn compare(name: &str, python: &str, rust: &[String]) {
    let py: Vec<&str> = python.split_inclusive('\n').collect();
    let mut report = Vec::new();
    let mut bad = 0;
    if py.len() != rust.len() {
        report.push(format!("{name}: python {} lines, rust {}", py.len(), rust.len()));
    }
    for (k, (p, r)) in py.iter().zip(rust).enumerate() {
        if *p == r.as_str() {
            continue;
        }
        bad += 1;
        if bad > 20 {
            continue;
        }
        let a: Value = serde_json::from_str(p).expect("python line");
        let b: Value = serde_json::from_str(r).expect("rust line");
        let what = format!("{} {}", a["kind"].as_str().unwrap_or("?"), a.get("key").map(|k| k.to_string()).unwrap_or_default());
        let mut paths = Vec::new();
        diff(&a, &b, String::new(), &mut paths);
        if paths.is_empty() {
            paths.push("same JSON values, different bytes".to_string());
        }
        report.push(format!("{name} line {}: {what}\n    {}", k + 1, paths.join("\n    ")));
    }
    let bytes: usize = rust.iter().map(|l| l.len()).sum();
    eprintln!("{name}: {} lines, {bytes} bytes, {bad} lines differ", rust.len());
    assert!(report.is_empty() && python.len() == bytes, "{} lines differ\n{}", bad, report.join("\n"));
}

fn meta(root: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(root.join("data/scripts-dos-cd/index.json")).unwrap()).unwrap()
}

fn fixture(name: &str) -> Value {
    serde_json::from_slice(&std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join(name)).unwrap())
        .unwrap()
}

/// Every unit's IR and analyses, the game's fields and facts.py over the fixture states.
#[test]
fn full_dump_matches_python() {
    let out = python_dumps();
    let root = default_root();
    let g = Game::load(&root).expect("load the game");
    let lines = dump::lines(&g, &meta(&root), &fixture("fact_states.json"));
    compare("full", &gunzip(&out.join("ir-python.jsonl.gz")), &lines);
}

/// The units read without the raw blocks: the else-if text rule and the unverified var names.
#[test]
fn noblocks_dump_matches_python() {
    let out = python_dumps();
    let root = default_root();
    let sc = read::load_with(&root.join("data/scripts-dos-cd"), None, &meta(&root)).expect("load the units");
    compare("noblocks", &gunzip(&out.join("ir-python-noblocks.jsonl.gz")), &dump::noblocks_lines(&sc));
}

/// Malformed statements, every note class and loop kind, a Game built by hand.
#[test]
fn synthetic_dump_matches_python() {
    let out = python_dumps();
    let lines = dump::synthetic_lines(&fixture("synthetic.json"));
    compare("synthetic", &gunzip(&out.join("ir-python-synthetic.jsonl.gz")), &lines);
}
