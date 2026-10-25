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
//! * `row3_subset` (fast), `row3_full` and `row3_wide` (ignored): settle; the wide one runs waits from every state and
//!   sentences from one.
//! * `row4_subset` and `row4_switch_subset` (fast), `row4_full`, `row4_const_polls_full`, `row4_input_after_turns_full`,
//!   `row4_wide` and `row4_synth` (ignored): dialog, and the two switch runs of row 4; the wide one clicks every verb
//!   shown at pre from every state (the cases the Rust machine runs in at most 20 ms), switches off and with
//!   input_after_turns; the synthetic one clicks with VAR_VERB_SCRIPT 0 or not held, and VAR_USERPUT not held.
//! * `row5_subset` and `row5_switch_subset` (fast), `row5_full`, `row5_esc_key_at_pre_full`, `row5_wide`, `row5_synth`
//!   and `row5_arm` (ignored): esc, and the switch run of row 5; the wide one presses Esc from every state, switch off and
//!   on; the synthetic ones change the key code, the cursor, VAR_USERPUT and VAR_VERB_SCRIPT, and arm an override on
//!   each slot live at pre.
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

/// Moves of row 3 (settle), picked by `survey3.py --cover` on the goldens: rounds, merges at a round's start into value
/// sets and the second-instance close (S2#48, S1#59), a camera exit that forks once and stays (S3#60), a parked first
/// end (S1#39), a wait (S2#30), doSentence's queue run at a round's end and an engine error (S1#96), a slot with no
/// single wait point (S2#49), the round limit (S3#78), a poll whose facts did not change (S1#22), a chained first end
/// (S2#10), the path limit (S1#33), endCutscene in a settle turn with esc: reads resting on set order (S3#47), an Esc
/// skip that runs nothing at this row (S1#2), and a move with no action (S1#95).
const SUBSET3: &str = "S2#48,S3#60,S1#39,S2#30,S1#96,S2#49,S3#78,S1#22,S1#59,S2#10,S1#33,S3#47,S1#2,S1#95";

/// Moves of row 4 (dialog), picked with `survey4.py --per-move` on the goldens: a local input script (S1#19), a turn
/// that changed VAR_USERPUT before the click (input drift) on a path merged later (S1#34), drift on 32 merged paths
/// (S2#8), merges after the click (S1#21), a move whose output const_polls changes (S3#14), and moves that run as at
/// row 3: a wait (S2#30), an Esc skip that runs nothing at this row (S1#2), a move with no action (S1#95).
const SUBSET4: &str = "S1#19,S1#34,S2#8,S1#21,S3#14,S2#30,S1#2,S1#95";

/// Moves of row 4's switch runs: clicks input_after_turns drops (VAR_USERPUT 0 after the turns: S1#34, S1#53, S2#6,
/// S2#8), and moves const_polls changes (S2#15, S3#14).
const SUBSET4_IAT: &str = "S1#34,S1#53,S2#6,S2#8";
const SUBSET4_CP: &str = "S2#15,S3#14";

/// Moves of row 5 (esc): all 22 Esc moves of the goldens (0.1 s of Rust in all): no override armed (S3#46), the rewound
/// slot's turn (the other 21), the fork on merged paths that disagree on clearClickedStatus (S1#2, S3#86), the key
/// dropped with input off (S3#13), wiped (S1#3), taken by input scripts 4 (S3#59) and 14 (S3#16) and by an unknown one
/// (S3#4); and moves that run as at row 4: a dialog line (S1#19), a wait (S2#30), a move with no action (S1#95).
const ESC_MOVES: &str = "S1#2,S1#3,S2#14,S2#20,S2#52,S3#2,S3#4,S3#10,S3#13,S3#15,S3#16,S3#21,S3#42,S3#46,S3#48,S3#54,\
                         S3#56,S3#58,S3#59,S3#61,S3#64,S3#86";

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
    wide_with(dumps, None);
}

/// `wide`, keeping only the cases the Rust machine runs in at most `max_ms` milliseconds with at most `max_items`
/// list entries (wide.py `--plan`, `machine-cases --costs`, then `--from-costs`).
fn wide_costed(dumps: &[(&str, Vec<&str>)], max_ms: &str, max_items: &str) {
    wide_with(dumps, Some((max_ms, max_items)));
}

fn wide_with(dumps: &[(&str, Vec<&str>)], costed: Option<(&str, &str)>) {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("wide.py");
    for (name, args) in dumps {
        let out = tmp(name);
        let mut extra: Vec<String> = Vec::new();
        let plan = tmp(&format!("{name}-plan"));
        if let Some((max_ms, max_items)) = costed {
            let mut c = py(&[script.to_str().unwrap(), "dump", "--out", plan.to_str().unwrap(), "--plan"]);
            c.args(args);
            if let Err(e) = run(&mut c) {
                panic!("wide.py --plan: {e}");
            }
            if let Err(e) = run(Command::new(env!("CARGO_BIN_EXE_machine-cases")).arg(&plan).arg("--costs")) {
                panic!("machine-cases --costs: {e}");
            }
            let costs = plan.join("rust-costs.json");
            extra = vec!["--from-costs".into(), costs.to_str().unwrap().into(), "--max-rust-ms".into(), max_ms.into()];
            extra.extend(["--max-items".to_string(), max_items.to_string()]);
        }
        let mut c = py(&[script.to_str().unwrap(), "dump", "--out", out.to_str().unwrap()]);
        c.args(args).args(&extra);
        if let Err(e) = run(&mut c) {
            panic!("wide.py: {e}");
        }
        let _ = std::fs::remove_dir_all(&plan);
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
fn row3_subset() {
    subset("row3", SUBSET3);
}

#[test]
fn row4_subset() {
    subset("row4", SUBSET4);
}

#[test]
fn row4_switch_subset() {
    subset("row4-input_after_turns", SUBSET4_IAT);
    subset("row4-const_polls", SUBSET4_CP);
}

#[test]
fn row5_subset() {
    subset("row5", &format!("{},S1#19,S2#30,S1#95", ESC_MOVES.replace(' ', "")));
}

/// The 22 Esc moves under switch esc_key_at_pre (VAR_USERPUT and VAR_VERB_SCRIPT at pre).
#[test]
fn row5_switch_subset() {
    subset("row5-esc_key_at_pre", &ESC_MOVES.replace(' ', ""));
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

/// Every row-3 move, S2#21 and S2#29 among them (about 10 s each in Rust, traced; 3 minutes in all, most of it
/// candidate.py check).
#[test]
#[ignore]
fn row3_full() {
    full("row3");
}

/// Every row-4 move (the 60 dialog lines, the rest as at row 3), and the same for the two switch runs of row 4.
#[test]
#[ignore]
fn row4_full() {
    full("row4");
}

#[test]
#[ignore]
fn row4_const_polls_full() {
    full("row4-const_polls");
}

#[test]
#[ignore]
fn row4_input_after_turns_full() {
    full("row4-input_after_turns");
}

/// Every row-5 move (the 22 Esc moves, the rest as at row 4), and the same for the switch run esc_key_at_pre.
#[test]
#[ignore]
fn row5_full() {
    full("row5");
}

#[test]
#[ignore]
fn row5_esc_key_at_pre_full() {
    full("row5-esc_key_at_pre");
}

/// Row 5 beyond the goldens: the Esc key from every ground-truth state, switch off and with esc_key_at_pre (the cases
/// the Rust machine runs in at most 1 s with at most 200,000 list entries: 247 of 250; about 2 minutes of Python each).
#[test]
#[ignore]
fn row5_wide() {
    let escs = |sw: &'static str| {
        vec!["--row", "5", "--rows", "all", "--escs", "--no-objects", "--budget", "60", "--max-mb", "50", "--switches", sw]
    };
    wide_costed(&[("wide5-escs", escs("")), ("wide5-escs-pre", escs("esc_key_at_pre"))], "1000", "200000");
}

/// Row 5 on synthetic states: the key code VAR_CUTSCENEEXIT_KEY 0 (Esc is not the exit key: the key is Esc's ASCII
/// code), not held, Enter with the cursor as at pre, hidden and not held, Tab, 512 and -1 (not a key) from ten states;
/// VAR_USERPUT not held (the fork on whether the key is dropped; switch off and on), VAR_VERB_SCRIPT 0 and not held from
/// every state; and with a path limit of 3 (forks in the Esc frame cut, key routes cut back).
#[test]
#[ignore]
fn row5_synth() {
    const ROWS: &str = "3,22,37,60,120,131,180,202,215,240";
    let s = |rows: &'static str, change: Vec<&'static str>, sw: &'static str| {
        let mut v = vec!["--row", "5", "--rows", rows, "--escs", "--no-objects", "--budget", "60", "--max-mb", "50"];
        v.extend_from_slice(&["--switches", sw]);
        v.extend(change);
        v
    };
    wide_costed(
        &[
            ("wide5-code0", s(ROWS, vec!["--set", "var:24=0"], "")),
            ("wide5-codeunk", s(ROWS, vec!["--unheld", "var:24"], "")),
            ("wide5-enter", s(ROWS, vec!["--set", "var:24=13"], "")),
            ("wide5-enter-nocursor", s(ROWS, vec!["--set", "var:24=13,var:52=0"], "")),
            ("wide5-enter-cursorunk", s(ROWS, vec!["--set", "var:24=13", "--unheld", "var:52"], "")),
            ("wide5-tab", s(ROWS, vec!["--set", "var:24=9"], "")),
            ("wide5-code512", s(ROWS, vec!["--set", "var:24=512"], "")),
            ("wide5-codeneg", s(ROWS, vec!["--set", "var:24=-1"], "")),
            ("wide5-upunk", s("all", vec!["--unheld", "var:53"], "")),
            ("wide5-upunk-pre", s("all", vec!["--unheld", "var:53"], "esc_key_at_pre")),
            ("wide5-vs0", s("all", vec!["--set", "var:32=0"], "")),
            ("wide5-vsunk-pre", s("all", vec!["--unheld", "var:32"], "esc_key_at_pre")),
            // small path limits: forks in the Esc frame cut, key routes cut back with them
            ("wide5-limits", s("all", vec!["--unheld", "var:53", "--path-limit", "3", "--step-limit", "2000"], "")),
        ],
        "1000",
        "200000",
    );
}

/// Row 5 with an override armed on each slot live at pre at its wait point W and at W + 1 (wide.py `--arm`): global,
/// local, object and entry slots, override points that are no instruction, overrides of slots not run at pre (script
/// facts not held), two armed keys (`--keep-armed`, on states that arm one), and a path limit of 2 with VAR_USERPUT
/// not held (forks in the Esc frame cut, key routes cut back).
#[test]
#[ignore]
fn row5_arm() {
    let s = |rows: &'static str, extra: Vec<&'static str>| {
        let mut v = vec!["--row", "5", "--rows", rows, "--arm", "--no-objects", "--budget", "60", "--max-mb", "50"];
        v.extend(extra);
        v
    };
    wide_costed(
        &[
            ("wide5-arm", s("3,22,37,60,120,124,131,180,215,240", vec![])),
            ("wide5-arm-keep", s("96,158,202,215,240", vec!["--keep-armed"])),
            ("wide5-arm-unheld", s("3,37,120,215", vec!["--drop-kinds", "script,local"])),
            ("wide5-arm-limits", s("3,37,120,215,240", vec!["--unheld", "var:53", "--path-limit", "2"])),
        ],
        "1000",
        "200000",
    );
}

/// Row 4 beyond the goldens: a click on every verb shown at pre, and on verb 0, from every ground-truth state, with
/// the switches off and with input_after_turns; the cases the Rust machine runs in at most 20 ms with at most 200,000
/// list entries (about 2,500 of the 3,038 clicks; some clicks dump over 100 MB, which Python takes minutes to encode).
#[test]
#[ignore]
fn row4_wide() {
    let clicks = |sw: &'static str| {
        vec!["--row", "4", "--rows", "all", "--clicks", "--no-objects", "--budget", "20", "--max-mb", "5", "--switches", sw]
    };
    wide_costed(&[("wide4-clicks", clicks("")), ("wide4-clicks-iat", clicks("input_after_turns"))], "20", "200000");
}

/// Row 4 on synthetic states, clicks from seven states: VAR_VERB_SCRIPT 0, VAR_VERB_SCRIPT not held (an unknown
/// input script), VAR_USERPUT not held (drift from an unknown pre value; with input_after_turns, the fork on whether
/// the click is dropped); each with the switches off and with input_after_turns.
#[test]
#[ignore]
fn row4_synth() {
    let s = |change: [&'static str; 2], sw: &'static str| {
        let mut v = vec!["--row", "4", "--rows", "3,22,60,120,131,180,240", "--clicks", "--no-objects", "--budget", "5"];
        v.extend_from_slice(&["--max-mb", "5", "--max-items", "2000000", "--switches", sw]);
        v.extend_from_slice(&change);
        v
    };
    wide(&[
        ("wide4-vs0", s(["--set", "var:32=0"], "")),
        ("wide4-vs0-iat", s(["--set", "var:32=0"], "input_after_turns")),
        ("wide4-vsunk", s(["--unheld", "var:32"], "")),
        ("wide4-vsunk-iat", s(["--unheld", "var:32"], "input_after_turns")),
        ("wide4-upunk", s(["--unheld", "var:53"], "")),
        ("wide4-upunk-iat", s(["--unheld", "var:53"], "input_after_turns")),
    ]);
}

/// Row 3 beyond the goldens: the wait action from every ground-truth state (249 cases, about 2 minutes of Python), and
/// sentences with settle from state 120 (a case over 1 s in Python is dropped).
#[test]
#[ignore]
fn row3_wide() {
    wide(&[
        ("wide3-waits", vec!["--row", "3", "--rows", "all", "--waits", "--no-objects", "--budget", "20", "--max-mb", "50"]),
        ("wide3-sentences", vec!["--row", "3", "--rows", "120", "--sentences", "--no-objects", "--budget", "1", "--max-mb", "20"]),
    ]);
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
