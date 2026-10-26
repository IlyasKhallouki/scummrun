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

// --- F9, actor_rooms ---------------------------------------------------------------------------------------------

#[test]
fn room33_actor_rooms_unchanged() {
    // room 33's code writes actors' rooms but reads none it knows (no start holds one): the reading is golden.py's
    let (b, _) = oracle::read_with(game(), static_oracle(), "room33", switches(&["actor_rooms"]));
    let (n, d) = differing(&b, "room33");
    assert_eq!(n, 880);
    assert!(d.is_empty(), "{} lines differ", d.len());
}

fn domain_ints(b: &Builder, key: &str) -> Option<Vec<i64>> {
    b.domain(key).map(|d| {
        d.values
            .iter()
            .map(|l| match l {
                scumm_reader::Label::Int(v) => *v,
                other => panic!("{key}: {other:?}"),
            })
            .collect()
    })
}

/// The kitchen's actor rooms: the start holds none (NOT_KNOWN, -1). The cook (6) is put in rooms 0 and 28 by the bar
/// and in 72 by global 120; the ego (VAR_EGO = 1, not written by the scope) in 28, 79, 81 and 82 by the cook's
/// dialogs and by four loadRoomWithEgo. An actor no write of the scope reaches keeps the start: a constant.
#[test]
fn kitchen_actor_room_domains() {
    let b = oracle::builder_with(game(), static_oracle(), "kitchen", switches(&["actor_rooms"]));
    assert_eq!(b.table.start.get0("actorroom:6"), scumm_reader::table::NOT_KNOWN);
    assert_eq!(b.kind("actorroom:6").as_str(), "fact");
    assert_eq!(domain_ints(&b, "actorroom:6"), Some(vec![-1, 0, 28, 72]));
    assert_eq!(domain_ints(&b, "actorroom:1"), Some(vec![-1, 28, 33, 41, 79, 81, 82, 85]));
    assert_eq!(domain_ints(&b, "actorroom:3"), Some(vec![-1, 0, 28]));
    assert_eq!(b.kind("actorroom:0").as_str(), "const");
    let off = oracle::builder(game(), static_oracle(), "kitchen");
    assert!((0..13).all(|a| !off.scope.writes.has(&format!("actorroom:{a}"))), "off: no actor room is written");
}

fn local_203(b: &mut Builder) -> Vec<scumm_reader::Record> {
    let ev = scumm_reader::Event {
        name: "local:203".into(),
        unit: scumm_ir::UnitKey::Local(28, 203),
        w: 0,
        sig: 0,
        locals: vec![],
        args_known: false,
        recursive: scumm_reader::Py::None,
        resistant: scumm_reader::Py::Bool(false),
        owned: scumm_reader::Py::Bool(false),
        abort: false,
        override_: 0,
        cut_data: vec![],
    };
    let job = scumm_reader::Job {
        kind: "event".into(),
        label: "event(local:203@0)".into(),
        pins: Default::default(),
        pre: Default::default(),
        what: scumm_reader::What::Event(ev),
        checks: vec![],
    };
    let mut m = scumm_reader::machine::Reader::new(game());
    b.run_job(&mut m, &job)
}

/// The bar door's local 203 from its start (the reader reaches it only once F1 runs clicks through the room's
/// input script): every record that tests the cook's room holds `actorroom:6` in its precondition, one value each:
/// 28 leads to the x test, 0 and 72 to the wait for the ego, NOT_KNOWN to getActorRoom's non-fact as with the switch
/// off. Off, no record holds it and the room test is a check on the non-fact.
#[test]
fn kitchen_local_203_holds_the_cooks_room() {
    let mut b = oracle::builder_with(game(), static_oracle(), "kitchen", switches(&["actor_rooms"]));
    let recs = local_203(&mut b);
    let tests_room = |r: &scumm_reader::Record| r.checks.iter().any(|a| a.cond.contains("VAR_RESULT == 28"));
    let x_test = |r: &scumm_reader::Record| r.checks.iter().any(|a| a.cond.contains("VAR_RESULT > 310"));
    let held: Vec<(i64, bool, bool)> = recs
        .iter()
        .filter_map(|r| {
            let v = r.pre.get("actorroom:6")?;
            assert_eq!(v.len(), 1, "{}: one value", r.label);
            let scumm_reader::Label::Int(v) = v[0] else { panic!() };
            Some((v, tests_room(r), x_test(r)))
        })
        .collect();
    let values: std::collections::BTreeSet<i64> = held.iter().map(|h| h.0).collect();
    assert_eq!(values, [-1, 0, 28, 72].into_iter().collect(), "{held:?}");
    for &(v, room, x) in &held {
        assert_eq!(room, v == -1, "the room test is a check only when the room is not known ({v})");
        if v == 0 || v == 72 {
            assert!(!x, "cook in {v}: no x test");
        }
        if v == 28 {
            assert!(x, "cook in 28: the x test");
        }
    }
    let recs_room = recs.iter().filter(|r| tests_room(r) || r.pre.contains_key("actorroom:6")).count();
    assert_eq!(recs_room, held.len(), "every record past the room test holds the cook's room");
    let mut off = oracle::builder(game(), static_oracle(), "kitchen");
    let recs = local_203(&mut off);
    assert!(recs.iter().all(|r| !r.pre.contains_key("actorroom:6")));
    assert!(recs.iter().any(|r| tests_room(r)));
}

/// F9 on the kitchen: 72,328 records, 72 more than golden.py's. Per label, the records whose precondition does not
/// know an actor's room (NOT_KNOWN or no actorroom key), with every actorroom key taken out of their precondition
/// and effects, are exactly golden.py's (as multisets). The 72 new records each hold the cook's room 0, 28 or 72.
#[test]
#[ignore]
fn kitchen_actor_rooms() {
    use std::collections::BTreeMap;
    let (b, _, _) = oracle::read_par_with(game(), static_oracle(), "kitchen", 3, switches(&["actor_rooms"]));
    assert_eq!(b.records.len(), 72_328);
    let golden = oracle::golden_dir().join("kitchen.jsonl.gz");
    let info = oracle::golden_info(&golden);
    let by_label = |lines: &mut dyn Iterator<Item = String>, strip: bool| {
        let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut known: BTreeMap<String, usize> = BTreeMap::new();
        for l in lines {
            let mut v: Value = serde_json::from_str(&l).unwrap();
            let Some(r) = v.get_mut("record").and_then(|r| r.get_mut("$Record")).and_then(|r| r.as_object_mut()) else {
                continue;
            };
            if strip {
                let pre = r["pre"].as_object_mut().unwrap();
                let held: Vec<String> = pre
                    .iter()
                    .filter(|(k, v)| k.starts_with("actorroom:") && **v != serde_json::json!({"$set": [-1]}))
                    .map(|(_, v)| v.to_string())
                    .collect();
                if !held.is_empty() {
                    for h in held {
                        *known.entry(h).or_default() += 1;
                    }
                    continue;
                }
                pre.retain(|k, _| !k.starts_with("actorroom:"));
                r["effects"].as_object_mut().unwrap().retain(|k, _| !k.starts_with("actorroom:"));
            }
            let label = r["label"].as_str().unwrap().to_string();
            out.entry(label).or_default().push(serde_json::to_string(&r).unwrap());
        }
        for v in out.values_mut() {
            v.sort();
        }
        (out, known)
    };
    let (g, _) = by_label(&mut scumm_reader::gz_lines(&golden), false);
    let (c, known) = by_label(&mut b.reading_lines(&info), true);
    assert_eq!(g.len(), c.len(), "the same labels");
    for (label, recs) in &g {
        assert_eq!(Some(recs), c.get(label), "{label}");
    }
    let held: BTreeMap<&str, usize> = known.iter().map(|(k, n)| (k.as_str(), *n)).collect();
    assert_eq!(
        held,
        [(r#"{"$set":[0]}"#, 27), (r#"{"$set":[28]}"#, 18), (r#"{"$set":[72]}"#, 27)].into_iter().collect(),
        "the new records hold the cook's room"
    );
}
