//! Fix round 1 (fix-round1.md): readings with the Rust-only switches on, against the frozen goldens (golden.py's
//! files, made with every switch off). Each test pins what the switch changes and that nothing else moves.
//!
//! Fast: room 33 with each switch on. Slow (`-- --ignored`, about 2 minutes each on 3 threads): the kitchen.
//! MI1's object and room numbers appear here as test data only (D1, plan.md's C2).

use scumm_reader::oracle::{self, Static};
use scumm_reader::Builder;
use serde_json::Value;
use std::sync::OnceLock;

fn game() -> &'static scumm_ir::Game {
    static G: OnceLock<scumm_ir::Game> = OnceLock::new();
    G.get_or_init(oracle::load_game)
}

fn static_oracle() -> &'static Static {
    static S: OnceLock<Static> = OnceLock::new();
    S.get_or_init(Static::load)
}

fn switches(names: &[&str]) -> scumm_machine::Switches {
    scumm_machine::Switches::parse(&names.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap()
}

/// The records of a reading against the golden file's, line by line: (lines, the golden and candidate record of
/// each line that differs; header and footer lines as (None, None) with the line text).
fn differing(b: &Builder, name: &str) -> (usize, Vec<(usize, Value, Value)>) {
    let golden = oracle::golden_dir().join(format!("{name}.jsonl.gz"));
    let info = oracle::golden_info(&golden);
    let mut gold = scumm_reader::gz_lines(&golden);
    let mut out = Vec::new();
    let mut n = 0;
    for mine in b.reading_lines(&info) {
        n += 1;
        let g = gold.next().expect("the golden file ends early");
        if g != mine {
            out.push((n, serde_json::from_str(&g).unwrap(), serde_json::from_str(&mine).unwrap()));
        }
    }
    assert!(gold.next().is_none(), "the golden file has more lines");
    (n, out)
}

fn record(v: &Value) -> Option<&serde_json::Map<String, Value>> {
    v.get("record")?.get("$Record")?.as_object()
}

#[test]
fn room33_draw_clears_unchanged() {
    // room 33's code draws no object that shares its place: the reading is golden.py's
    let (b, _) = oracle::read_with(game(), static_oracle(), "room33", switches(&["draw_clears"]));
    let (n, d) = differing(&b, "room33");
    assert_eq!(n, 880);
    assert!(d.is_empty(), "{} lines differ", d.len());
}

/// F5 on the kitchen (rooms 28 and 41): the same 72,256 records, no new fork; 307 of them add `state:O := 0` for the
/// objects that share the drawn object's place and size (the bar's 324-329 and 357-361 groups, the kitchen's
/// 571-576) and drop model.py's "not modelled" note; the footer's first-pass record count is one higher (68,954: a
/// record of the first pass that the clearing tells apart). Nothing else differs.
#[test]
#[ignore]
fn kitchen_draw_clears() {
    let (b, _, _) = oracle::read_par_with(game(), static_oracle(), "kitchen", 3, switches(&["draw_clears"]));
    assert_eq!(b.records.len(), 72_256);
    let (n, d) = differing(&b, "kitchen");
    assert_eq!(n, 72_258);
    let mut recs = 0;
    for (line, g, c) in &d {
        let (Some(rg), Some(rc)) = (record(g), record(c)) else {
            assert_eq!(*line, n, "only the footer differs besides records");
            continue;
        };
        recs += 1;
        for (k, v) in rg {
            if k != "effects" && k != "notes" {
                assert_eq!(Some(v), rc.get(k), "line {line}: field {k}");
            }
        }
        let (eg, ec) = (rg["effects"].as_object().unwrap(), rc["effects"].as_object().unwrap());
        for (k, v) in eg {
            assert_eq!(Some(v), ec.get(k), "line {line}: effect {k} kept");
        }
        let added: Vec<&String> = ec.keys().filter(|k| !eg.contains_key(*k)).collect();
        assert!(!added.is_empty(), "line {line}: an effect added");
        for k in added {
            assert!(k.starts_with("state:"), "line {line}: {k}");
            assert_eq!(ec[k], serde_json::json!({"$tuple": ["set", 0]}), "line {line}: {k}");
        }
    }
    assert_eq!(recs, 307);
}
