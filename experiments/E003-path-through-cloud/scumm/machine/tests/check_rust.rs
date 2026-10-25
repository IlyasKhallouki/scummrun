//! check.py's model check on the Rust machine (`check_rust.py` with `machine-serve`), against the repository's
//! results/ (PORT-NOTES.md, "The model check on Rust").
//!
//! * `check_rust_rows012` (fast, about 20 s, most of it Python loading the game): rows 0, 1 and 2.
//! * `check_rust_all` (ignored: `cargo test --release -- --ignored`): rows 0-5 and the three switch runs, two jobs
//!   each: 9 minutes at load about 12.
//!
//! Each run writes check.py's files into a scratch folder that starts with results/growth.md alone. The test compares
//! each run's jsonl, its log from line 2 (line 1 holds the run's time) and, at the end, growth.md with results/,
//! byte for byte. check_rust.py stops by itself when a frozen Python source changed.

use std::path::{Path, PathBuf};
use std::process::Command;

const R2: &str = "own_entry,dispatch,calls,room_change";
const R4: &str = "own_entry,dispatch,calls,room_change,settle,dialog";
const R5: &str = "own_entry,dispatch,calls,room_change,settle,dialog,esc";

/// (check.py's file stem, features, switches), in the order the runs go: rows first, so that check-part1.* ends as
/// row 5's, as in results/.
const RUNS: [(&str, &str, &str); 9] = [
    ("check-part1-row0", "own_entry", ""),
    ("check-part1-row1", "own_entry,dispatch,calls", ""),
    ("check-part1-row2", R2, ""),
    ("check-part1-row3", "own_entry,dispatch,calls,room_change,settle", ""),
    ("check-part1-row4", R4, ""),
    ("check-part1-row5", R5, ""),
    ("check-part1-row4-const_polls", R4, "const_polls"),
    ("check-part1-row4-input_after_turns", R4, "input_after_turns"),
    ("check-part1-row5-esc_key_at_pre", R5, "esc_key_at_pre"),
];

fn exp() -> PathBuf {
    scumm_ir::default_root().join("experiments/E003-path-through-cloud")
}

fn read(p: &Path) -> Vec<u8> {
    std::fs::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// The text from its second line on.
fn from_line2(b: &[u8]) -> &[u8] {
    b.iter().position(|&c| c == b'\n').map_or(&[], |k| &b[k + 1..])
}

fn check_runs(name: &str, runs: &[(&str, &str, &str)], jobs: &str) {
    let results = exp().join("results");
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    std::fs::copy(results.join("growth.md"), out.join("growth.md")).unwrap();
    for &(stem, features, switches) in runs {
        let mut c = Command::new("nice");
        c.args(["-n", "10"]).arg(scumm_ir::default_root().join(".venv/bin/python")).arg("check_rust.py");
        c.arg("--out").arg(&out).arg("--bin").arg(env!("CARGO_BIN_EXE_machine-serve"));
        c.args(["--features", features, "--jobs", jobs]);
        if !switches.is_empty() {
            c.args(["--switches", switches]);
        }
        let r = c.current_dir(exp()).output().expect("run check_rust.py");
        assert!(r.status.success(), "{stem}: check_rust.py failed:\n{}", String::from_utf8_lossy(&r.stderr));
        let jsonl = format!("{stem}.jsonl");
        assert!(read(&results.join(&jsonl)) == read(&out.join(&jsonl)), "{stem}.jsonl differs from results/");
        let log = format!("{stem}.log");
        let (want, got) = (read(&results.join(&log)), read(&out.join(&log)));
        assert!(from_line2(&want) == from_line2(&got), "{stem}.log differs from results/ after line 1");
        eprintln!("{stem}: jsonl and log equal to results/");
    }
    assert!(read(&results.join("growth.md")) == read(&out.join("growth.md")), "growth.md differs from results/");
}

#[test]
fn check_rust_rows012() {
    check_runs("check-rust-rows012", &RUNS[..3], "1");
}

#[test]
#[ignore]
fn check_rust_all() {
    check_runs("check-rust-all", &RUNS, "2");
}
