//! Fix round 1's reader switches (fix-round1.md), each against its own golden and its own checks. The goldens live
//! in out/golden/reader-fr1/ (made by `reader-port read NAME --switches S --out FILE`, PORT-NOTES.md "Fix round 1");
//! with every switch off the reading is checked against golden.py's files by tests/oracle.rs.
//!
//! Fast: room 33 with due_first. Slow (`-- --ignored`): the kitchen with due_first on one thread and on three, and
//! the due rule against the engine's frames (due_engine.py's file).

use scumm_reader::builder::Switches;
use scumm_reader::oracle::{self, Static};
use std::path::PathBuf;
use std::sync::OnceLock;

fn static_oracle() -> &'static Static {
    static S: OnceLock<Static> = OnceLock::new();
    S.get_or_init(Static::load)
}

fn game() -> &'static scumm_ir::Game {
    static G: OnceLock<scumm_ir::Game> = OnceLock::new();
    G.get_or_init(oracle::load_game)
}

fn fr1_dir() -> PathBuf {
    oracle::golden_dir().parent().unwrap().join("reader-fr1")
}

fn assert_none(what: &str, (diffs, n): (Vec<String>, usize)) {
    assert!(n > 0, "{what}: nothing compared");
    assert!(diffs.is_empty(), "{what}: {} of {n} differ; first:\n{}", diffs.len(), diffs[..diffs.len().min(5)].join("\n"));
    eprintln!("{what}: {n} compared, 0 differences");
}

fn due_first() -> Switches {
    Switches { due_first: true, ..Default::default() }
}

/// The due writes of a reading against a direct evaluation (due.rs `verify`).
fn verify(b: &scumm_reader::Builder) -> u64 {
    let (n, bad) = scumm_reader::due::verify(b, 4096);
    assert!(bad.is_empty(), "due writes: {} differences; first:\n{}", bad.len(), bad[..bad.len().min(5)].join("\n"));
    n
}

#[test]
fn room33_due_first() {
    let (b, _) = oracle::read_with(game(), static_oracle(), "room33", due_first());
    assert_none("room 33, due_first", oracle::compare_reading(&b, &fr1_dir().join("room33-due_first.jsonl.gz"), 5));
    verify(&b);
}

#[test]
#[ignore]
fn kitchen_due_first() {
    let gold = fr1_dir().join("kitchen-due_first.jsonl.gz");
    let (b, _) = oracle::read_with(game(), static_oracle(), "kitchen", due_first());
    assert_none("kitchen, due_first", oracle::compare_reading(&b, &gold, 5));
    let n = verify(&b);
    eprintln!("kitchen, due_first: due writes checked on {n} states");
    let d = b.due.as_ref().unwrap();
    assert!(!d.units.is_empty());
    let (b3, _, _) = oracle::read_par_with(game(), static_oracle(), "kitchen", 3, due_first());
    assert_none("kitchen, due_first, 3 threads", oracle::compare_reading(&b3, &gold, 5));
}

/// The rule's premise against the engine: every frame where a slot's turn went against what the rule says from the
/// frame before is explained by the frame itself (a slot earlier in the frame changed a fact the test reads or froze
/// the slot, or Esc's abortCutscene ran first), and the rule decides the slot in every other case.
#[test]
#[ignore]
fn due_against_the_engine() {
    let frames = fr1_dir().join("due-engine-frames.jsonl.gz");
    assert!(frames.exists(), "{} is missing: run due_engine.py --out DIR and copy DIR/frames.jsonl.gz", frames.display());
    let (tally, _) = scumm_reader::due::engine_check(game(), &frames);
    let unexplained: u64 = tally.iter().filter(|(k, _)| k.ends_with("unexplained")).map(|(_, n)| n).sum();
    assert_eq!(unexplained, 0, "{tally:?}");
    let count = |pre: &str| -> u64 { tally.iter().filter(|(k, _)| k.starts_with(pre)).map(|(_, n)| n).sum() };
    // the rule's due slots ran on (or ended) in the next frame, but where the frame itself changed the test
    let due = count("plain | due") + count("polled | due");
    let due_stayed = count("plain | due | stayed") + count("polled | due | stayed");
    let explained = count("disagreement:");
    eprintln!("due slots: {due}, of them stayed {due_stayed}; disagreements {explained}, all explained");
    assert!(due > 1000);
}
