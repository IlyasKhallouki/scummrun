//! The port against the Python oracle (PORT-NOTES.md, "Run the comparison").
//!
//! * `row0_subset`, `row1_subset`, `row2_subset` (fast, about 20 s each, most of it Python's prep and check):
//!   machine-run on a few moves of the row, compared file by file with the goldens, traces too.
//! * `row0_full`, `row1_full`, `row2_full` (ignored: `cargo test --release -- --ignored`): every move of the row,
//!   `golden.py compare --traces`.
//! * `row0_wide`, `row1_wide`, `row2_wide` (ignored): object entries from five ground-truth states and every other
//!   unit from its first instruction (rows 1-2: also sentences, and a case over 2 s in Python or 20 MB of lines is
//!   dropped), Python (wide.py) against Rust (machine-cases), traces too.
//! * `row2_synth`, `row2_limits` (ignored): row 2 on states with live object slots, two of them changed (owners 1, owners
//!   not held), and with small path and step limits.
//!
//! Each needs the goldens in ROOT/out/golden (golden.py machine --runs row0,row1,row2) and the experiment's venv.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    scumm_ir::default_root()
}

fn exp() -> PathBuf {
    root().join("experiments/E003-path-through-cloud")
}

fn python() -> PathBuf {
    root().join(".venv/bin/python")
}

fn golden_run(run: &str) -> PathBuf {
    root().join("out/golden/machine").join(run)
}

fn tmp(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Runs a command under nice 10 in the experiment folder; its output when it fails.
fn run(cmd: &mut Command) -> Result<String, String> {
    let out = cmd.current_dir(exp()).output().expect("run a command");
    let text = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    if out.status.success() {
        Ok(text)
    } else {
        Err(text)
    }
}

fn py(args: &[&str]) -> Command {
    let mut c = Command::new("nice");
    c.args(["-n", "10"]).arg(python()).args(args);
    c
}

fn machine_run(run_name: &str, out: &Path, only: Option<&str>) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_machine-run"));
    c.arg(run_name).arg("--out").arg(out);
    if let Some(o) = only {
        c.args(["--only", o]);
    }
    if let Err(e) = run(&mut c) {
        panic!("machine-run failed:\n{e}");
    }
}

fn compare_file(golden: &Path, cand: &Path) {
    let r = run(py(&["golden.py", "compare-file"]).arg(golden).arg(cand));
    match r {
        Ok(t) if t.trim() == "equal" => {}
        Ok(t) | Err(t) => panic!("{} differs: {t}", cand.display()),
    }
}

/// Moves of row 0: forks at unknown locals and a WaitFor op (S3#65), a walk_to's room change (S1#58), an
/// object's end (S1#10), moves that run nothing at this row (an Esc skip S1#2, a dialog line S1#19, a wait
/// S2#30), and a move with no action (S1#95: no run line, no trace).
const SUBSET: &str = "S3#65,S1#58,S1#10,S1#2,S1#19,S2#30,S1#95";

/// Moves of row 1: merges, an actorFollowCamera fork, an endCutscene and a reentry (S1#33), a chainScript
/// (S2#10), the sentence slot's own turn first (S1#40), loadRoom and a cutscene's freeze (S1#13), findInventory,
/// getInventoryCount and loadRoomWithEgo (S1#11), a parked caller and a merge (S1#43), a doSentence (S1#96), an Esc
/// skip that runs nothing at this row (S1#2), and a move with no action (S1#95).
const SUBSET1: &str = "S1#33,S2#10,S1#40,S1#13,S1#11,S1#43,S1#96,S1#2,S1#95";

/// Moves of row 2 (room_change): startScene with the ego and two nesting ends (S1#11), a room change in an open
/// cutscene (S1#58), room 0 (S1#13), a live object slot's kind taken from its owner and stopTalk (S3#78), VAR_ROOM_FLAG
/// as a value set (S1#8), an entry code of an unknown room (S3#44), five room changes with startScene calls left on
/// the stack (S1#42), merges at scale with three room ends (S1#83, about 1 s), an Esc skip that runs nothing at this
/// row (S1#2), and a move with no action (S1#95).
const SUBSET2: &str = "S1#11,S1#58,S1#13,S3#78,S1#8,S3#44,S1#42,S1#83,S1#2,S1#95";

/// machine-run on some moves of a run, compared file by file with the goldens, traces too.
fn subset(run_name: &str, labels: &str) {
    if !golden_run(run_name).join("index.json").exists() {
        eprintln!("skipped: no {run_name} goldens");
        return;
    }
    let out = tmp(&format!("golden-{run_name}-subset"));
    machine_run(run_name, &out, Some(labels));
    for label in labels.split(',') {
        for suffix in [".jsonl.gz", ".trace.jsonl.gz"] {
            let g = golden_run(run_name).join(format!("{label}{suffix}"));
            let c = out.join("machine").join(run_name).join(format!("{label}{suffix}"));
            assert_eq!(g.exists(), c.exists(), "{}", c.display());
            if g.exists() {
                compare_file(&g, &c);
            }
        }
    }
    let _ = std::fs::remove_dir_all(&out);
}

/// machine-run on every move of a run, then `golden.py compare --traces` on it.
fn full(run_name: &str) {
    let out = tmp(&format!("golden-{run_name}-full"));
    machine_run(run_name, &out, None);
    let r = run(py(&["golden.py", "compare"]).arg(&out).args(["--runs", run_name, "--traces"]));
    match r {
        Ok(t) => assert!(t.contains(&format!("{run_name}: 0 of 239 moves differ")), "{t}"),
        Err(t) => panic!("golden.py compare: {t}"),
    }
    let _ = std::fs::remove_dir_all(&out);
}

/// wide.py against machine-cases, one dump per (name, wide.py arguments), traces too.
fn wide(dumps: &[(&str, Vec<&str>)]) {
    for (name, args) in dumps {
        let out = tmp(name);
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("wide.py");
        let mut c = py(&[script.to_str().unwrap(), "dump", "--out", out.to_str().unwrap()]);
        c.args(args);
        if let Err(e) = run(&mut c) {
            panic!("wide.py: {e}");
        }
        if let Err(e) = run(Command::new(env!("CARGO_BIN_EXE_machine-cases")).arg(&out)) {
            panic!("machine-cases: {e}");
        }
        compare_file(&out.join("python.jsonl.gz"), &out.join("rust.jsonl.gz"));
        compare_file(&out.join("python.trace.jsonl.gz"), &out.join("rust.trace.jsonl.gz"));
        let _ = std::fs::remove_dir_all(&out);
    }
}

#[test]
fn row0_subset() {
    subset("row0", SUBSET);
}

#[test]
fn row1_subset() {
    subset("row1", SUBSET1);
}

#[test]
fn row2_subset() {
    subset("row2", SUBSET2);
}

#[test]
#[ignore]
fn row0_full() {
    full("row0");
}

#[test]
#[ignore]
fn row1_full() {
    full("row1");
}

#[test]
#[ignore]
fn row2_full() {
    full("row2");
}

#[test]
#[ignore]
fn row1_wide() {
    wide(&[
        ("wide1-units", vec!["--row", "1", "--only-units", "--rows", "120", "--budget", "2", "--max-mb", "20"]),
        ("wide1-objects", vec!["--row", "1", "--sentences", "--budget", "2", "--max-mb", "20"]),
    ]);
}

#[test]
#[ignore]
fn row2_wide() {
    wide(&[
        ("wide2-units", vec!["--row", "2", "--only-units", "--rows", "120", "--budget", "2", "--max-mb", "20"]),
        ("wide2-objects", vec!["--row", "2", "--sentences", "--budget", "2", "--max-mb", "20"]),
    ]);
}

/// Row 2 on states that reach the room change's kills of live slots: three ground-truth states with a live object
/// slot (room slots), the first of them with every owner fact 1 (inventory slots, the ego object not in the room), and
/// with the owner facts not held (slot kinds and object places not known).
#[test]
#[ignore]
fn row2_synth() {
    let s = |rows: &'static str, extra: &[&'static str]| {
        let mut v = vec!["--row", "2", "--sentences", "--budget", "1", "--max-mb", "20", "--rows", rows];
        v.extend_from_slice(extra);
        v
    };
    wide(&[
        ("wide2-live", s("37,124,215", &[])),
        ("wide2-inventory", s("37", &["--set-prefix", "owner:=1"])),
        ("wide2-unheld", s("37", &["--drop-kinds", "owner"])),
    ]);
}

/// Row 2 with small path and step limits: forks cut while paths wait at joins, step-limit ends inside startScene.
#[test]
#[ignore]
fn row2_limits() {
    wide(&[
        ("wide2-limits-units", vec!["--row", "2", "--only-units", "--rows", "120", "--path-limit", "8", "--step-limit", "300",
                                    "--budget", "2", "--max-mb", "20"]),
        ("wide2-limits-sentences", vec!["--row", "2", "--sentences", "--rows", "120", "--path-limit", "4", "--budget", "2",
                                        "--max-mb", "20"]),
    ]);
}

#[test]
#[ignore]
fn row0_wide() {
    wide(&[("wide-objects", vec![]), ("wide-units", vec!["--units", "--rows", "120"])]);
}
