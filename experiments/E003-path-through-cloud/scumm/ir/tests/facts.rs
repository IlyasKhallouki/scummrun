//! facts.py's key scheme: every fact key the experiment's result files hold parses into a [`Fact`] and
//! prints back to the same string; keys facts.py never writes are refused.

use scumm_ir::{default_root, Fact};
use std::collections::BTreeSet;

#[test]
fn every_recorded_key_round_trips() {
    let results = default_root().join("experiments/E003-path-through-cloud/results");
    let mut keys = BTreeSet::new();
    for f in ["groundtruth-part1.jsonl", "start-part1.jsonl"] {
        let text = std::fs::read_to_string(results.join(f)).unwrap_or_else(|e| panic!("{f}: {e}"));
        for line in text.lines() {
            let row: serde_json::Value = serde_json::from_str(line).unwrap();
            for field in ["start", "done_diff", "post_diff", "facts"] {
                if let Some(m) = row.get(field).and_then(|v| v.as_object()) {
                    keys.extend(m.keys().cloned());
                }
            }
        }
    }
    assert!(keys.len() > 4000, "only {} keys", keys.len());
    let kinds: BTreeSet<&str> = keys.iter().map(|k| k.split(':').next().unwrap()).collect();
    assert_eq!(kinds.len(), 15, "{kinds:?}");
    for k in &keys {
        let f: Fact = k.parse().unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(&f.to_string(), k);
    }
}

#[test]
fn keys_facts_py_never_writes_are_refused() {
    for bad in [
        "", "var", "var:", "var:x", "var:01", "var:+1", "room:1", "class:17", "class:17:", "at:script:5", "at:obj:5:",
        "at:entry", "at:room:5:1", "frozen:entry:1", "frozen:", "esc:local:200", "objscript:-", "verb:1:2", "Var:1",
    ] {
        assert!(bad.parse::<Fact>().is_err(), "{bad:?} parsed");
    }
    for good in ["var:-3", "at:entry:92", "esc:obj:17:5", "frozen:local:204", "class:17:32", "room", "cutscene", "ego"] {
        assert_eq!(good.parse::<Fact>().unwrap().to_string(), good);
    }
}

#[test]
fn families_split_slot_keys_by_unit_kind() {
    let f = |s: &str| s.parse::<Fact>().unwrap().family();
    assert_eq!(f("at:script:152:96"), "at:script");
    assert_eq!(f("esc:entry:3"), "esc:entry");
    assert_eq!(f("frozen:local:204"), "frozen");
    assert_eq!(f("class:17:32"), "class");
}
