//! E012's targets (plan.md section 3, FMI_TARGET_PER_KEY): one target per key for the values the guide does not name,
//! and the T tier. Checked against `targets_with` (E008's and E009's targets, ported line for line from step.py and
//! tested against it in tests/step.rs): the same (key, value) pairs, grouped.

use fmi_explore::archive::fact_map;
use fmi_explore::step::{self, Target, TV};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap};

const REACHED: &str = "room\t28\t33\t41\nowner:566\t1\t15\nstate:316\t0\t1\nvar:103\t0\t1\t2\nvar:53\tbelow\t-1\t0\t1\t2\tabove\npos:local:215\t0\t1\nat:local:203:30\t0\t1\nbit:6\t0\nclass:426:32\t0\t1\nowner:566\t1\n";

fn guide() -> Value {
    json!({"format": "e008-guide-1", "goal": {"owner:566": [1]}, "map": "direct",
           "rooms": {"start": 33, "edges": {"33": [28, 35, 38], "28": [33, 41], "41": [28]}, "reached": [28, 33, 41]},
           "atoms": [["owner:566", 1, 0], ["bit:6", 1, 1], ["room", 41, 1], ["state:316", 1, 1],
                     ["class:426:32", 1, 2], ["var:103", null, 2], ["var:53", 2, 3], ["var:53", null, 4]],
           "keys": {}, "writers": {}, "stats": {}})
}

fn entry() -> Value {
    json!({"room": 33, "owner:566": 15, "state:316": 0, "var:103": 0, "var:53": 0})
}

fn goal() -> Vec<(String, Vec<i64>)> {
    vec![("owner:566".into(), vec![1])]
}

fn t(k: &str, v: TV, tier: i64, depth: i64) -> Target {
    Target { k: k.into(), v, tier, depth }
}

fn per_key(e: &Value, held: Value, tried: Value, rot: Option<&Value>, tested: Option<step::TestedKey>) -> Vec<Target> {
    let atoms = step::guide_atoms(&guide());
    step::targets_per_key(&step::read_reached_text(REACHED), &atoms, &fact_map(e), None, held.as_array().unwrap(),
                          tried.as_array().unwrap(), &goal(), rot, tested).unwrap()
}

fn old(e: &Value, held: Value, tried: Value, rot: Option<&Value>) -> Vec<Target> {
    step::targets(&step::read_reached_text(REACHED), &guide(), &fact_map(e), None, held.as_array().unwrap(),
                  tried.as_array().unwrap(), &goal(), rot).unwrap()
}

/// The (key, value) pairs of targets, a value list giving each of its values.
fn pairs(ts: &[Target]) -> BTreeSet<(String, String)> {
    let mut out = BTreeSet::new();
    for x in ts {
        match &x.v {
            TV::Text(s) => {
                out.insert((x.k.clone(), s.clone()));
            }
            v => {
                for n in v.ints() {
                    out.insert((x.k.clone(), n.to_string()));
                }
            }
        }
    }
    out
}

#[test]
fn a_keys_unnamed_values_are_one_target_and_named_values_their_own() {
    let ts = per_key(&entry(), json!([["state:316", 1]]), json!([]), None, None);
    assert_eq!(ts, vec![t("owner:566", TV::Int(1), 0, 0), t("room", TV::Int(41), 1, 1), t("class:426:32", TV::Int(1), 1, 2),
                        t("var:103", TV::Values(vec![1, 2]), 1, 2), t("var:53", TV::Int(2), 1, 3),
                        t("var:53", TV::Values(vec![-1, 1]), 1, 4), t("state:316", TV::Int(1), 2, 1)]);
    // E008's targets: the same pairs, var:103 and var:53's unnamed values one by one
    let o = old(&entry(), json!([["state:316", 1]]), json!([]), None);
    assert_eq!(pairs(&ts), pairs(&o));
    assert_eq!(o.len(), ts.len() + 2);
}

#[test]
fn a_single_unnamed_value_is_the_same_int_target() {
    let mut e = entry();
    e["var:103"] = json!(1);
    let ts = per_key(&e, json!([]), json!([["var:103", 0]]), None, None);
    assert!(ts.contains(&t("var:103", TV::Int(2), 1, 2)));
    assert_eq!(ts.iter().filter(|x| x.k == "var:103").count(), 1);
}

#[test]
fn tried_values_and_value_lists_leave_out_each_value() {
    let ts = per_key(&entry(), json!([]), json!([["var:103", [1, 2]], ["var:53", -1]]), None, None);
    assert!(!ts.iter().any(|x| x.k == "var:103"));
    assert!(ts.contains(&t("var:53", TV::Int(1), 1, 4)));
    // a list tried from this entry leaves out its values only: a value reached later is a target again
    let ts = per_key(&entry(), json!([]), json!([["var:103", [2]]]), None, None);
    assert!(ts.contains(&t("var:103", TV::Int(1), 1, 2)));
    // tier 0 as before
    let ts = per_key(&entry(), json!([]), json!([["owner:566", 1]]), None, None);
    assert!(ts.iter().all(|x| x.tier > 0));
}

#[test]
fn a_groups_tier_is_1_while_one_value_is_not_held() {
    let ts = per_key(&entry(), json!([["var:103", 1]]), json!([]), None, None);
    assert!(ts.contains(&t("var:103", TV::Values(vec![1, 2]), 1, 2)));
    let ts = per_key(&entry(), json!([["var:103", 1], ["var:103", 2]]), json!([]), None, None);
    assert!(ts.contains(&t("var:103", TV::Values(vec![1, 2]), 2, 2)));
    let ts = per_key(&entry(), json!([["var:103", null]]), json!([]), None, None);
    assert!(ts.contains(&t("var:103", TV::Values(vec![1, 2]), 2, 2)));
}

#[test]
fn the_root_value_of_an_any_atom_is_left_out_of_the_group() {
    let atoms = step::guide_atoms(&guide());
    let mut e = entry();
    e["var:103"] = json!(2);
    let root = fact_map(&json!({"var:103": 1, "var:53": 2}));
    let ts = step::targets_per_key(&step::read_reached_text(REACHED), &atoms, &fact_map(&e), Some(&root), &[], &[], &goal(),
                                   None, None).unwrap();
    assert!(ts.contains(&t("var:103", TV::Int(0), 1, 2)));
    assert!(ts.contains(&t("var:53", TV::Int(2), 1, 3)) && ts.contains(&t("var:53", TV::Values(vec![-1, 1]), 1, 4)));
}

#[test]
fn the_t_tier_comes_after_tier_0_and_before_the_rest() {
    let tested = |k: &str| k == "var:53" || k == "state:316";
    let ts = per_key(&entry(), json!([["state:316", 1]]), json!([]), None, Some(&tested));
    let order: Vec<String> = ts.iter().map(|x| x.k.clone()).collect();
    assert_eq!(order, vec!["owner:566", "var:53", "var:53", "state:316", "room", "class:426:32", "var:103"]);
    // within the T tier E008's order: tier, then depth
    assert_eq!(ts[1], t("var:53", TV::Int(2), 1, 3));
    assert_eq!(ts[3], t("state:316", TV::Int(1), 2, 1));
    // E008's order when no key is tested, and with no tested keys given
    let none = |_: &str| false;
    assert_eq!(per_key(&entry(), json!([]), json!([]), None, Some(&none)), per_key(&entry(), json!([]), json!([]), None, None));
}

#[test]
fn rotation_counts_a_list_by_the_records_that_cover_its_values() {
    let rot = json!({"cell": [["var:103", [1, 2, 7], 1]], "all": [["var:103", [1, 2, 7], 1], ["var:103", 1, 2], ["room", 41, 1]]});
    let ts = per_key(&entry(), json!([]), json!([]), Some(&rot), None);
    // tier 0 first; then the untried in the cell, by all, then depth; var:103 (cell 1) last
    assert_eq!(ts.first().unwrap(), &t("owner:566", TV::Int(1), 0, 0));
    assert_eq!(ts.last().unwrap(), &t("var:103", TV::Values(vec![1, 2]), 1, 2));
    let pos = |k: &str| ts.iter().position(|x| x.k == k).unwrap();
    assert!(pos("room") > pos("class:426:32") && pos("room") > pos("var:53"));
    // a record of other values of the key does not count
    let rot = json!({"cell": [["var:103", [5, 6], 3]], "all": []});
    let ts = per_key(&entry(), json!([]), json!([]), Some(&rot), None);
    assert_eq!(ts[1], t("room", TV::Int(41), 1, 1));
    assert_eq!(ts[2], t("state:316", TV::Int(1), 1, 1));
    assert_eq!(ts[3], t("class:426:32", TV::Int(1), 1, 2));
    assert_eq!(ts[4], t("var:103", TV::Values(vec![1, 2]), 1, 2));
}

#[test]
fn rotate_per_key_on_int_targets_is_rotate() {
    let mk = |k: &str, v: i64, tier: i64, depth: i64| t(k, TV::Int(v), tier, depth);
    let ts = vec![mk("state:316", 1, 0, 0), mk("bit:9", 1, 1, 1), mk("bit:1", 1, 1, 2), mk("room", 28, 1, 2), mk("bit:5", 1, 2, 2)];
    for rot in [json!({"cell": [["state:316", 1, 1], ["bit:1", 1, 1]], "all": [["bit:9", 1, 3], ["state:316", 1, 1], ["bit:1", 1, 4]]}),
                json!({}), json!({"cell": [["room", 28, 2]], "all": [["room", 28, 2], ["bit:5", 1, 1]]})] {
        assert_eq!(step::rotate_per_key(ts.clone(), &rot, None), step::rotate(ts.clone(), &rot));
    }
}

#[test]
fn the_harness_goal_of_a_target() {
    let k = fmi_core::Key::parse("var:103");
    assert_eq!(step::target_goal(&t("var:103", TV::Values(vec![1, 2]), 1, 2), &goal()).unwrap(), vec![(k, vec![1, 2])]);
    assert_eq!(step::target_goal(&t("var:103", TV::Int(2), 1, 2), &goal()).unwrap(), vec![(k, vec![2])]);
    assert_eq!(step::target_goal(&t("owner:566", TV::Int(1), 0, 0), &goal()).unwrap(),
               vec![(fmi_core::Key::parse("owner:566"), vec![1])]);
    assert!(step::target_goal(&t("goal", TV::Text("a=1".into()), 1, 0), &goal()).is_err());
    assert!(step::target_goal(&t("var:103", TV::Values(vec![]), 1, 0), &goal()).is_err());
}

#[test]
fn a_value_list_in_json_text_and_hold() {
    let x = t("var:103", TV::Values(vec![1, 2]), 1, 2);
    assert_eq!(x.json(), json!(["var:103", [1, 2], 1, 2]));
    assert_eq!(x.text(), "var:103=1|2");
    assert_eq!(TV::of(&json!([1, 2])), Some(TV::Values(vec![1, 2])));
    assert_eq!(TV::of(&json!(3)), Some(TV::Int(3)));
    let f = fact_map(&json!({"var:103": 2}));
    assert!(step::target_holds_in(&x, &goal(), &f) && step::target_holds(&x, &goal(), &f));
    let f = fact_map(&json!({"var:103": 3}));
    assert!(!step::target_holds_in(&x, &goal(), &f) && !step::target_holds(&x, &goal(), &f));
}

/// A small LCG for generated cases.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self, n: u64) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) % n
    }
}

/// Generated readings, guides, entries, held and tried sets: the per-key targets have E008's pairs, at most one value
/// list per key and only for values the guide does not name, and E008's order when every key's group has one value.
#[test]
fn generated_cases_have_e008s_pairs_grouped() {
    let kinds = ["var:", "bit:", "owner:", "state:", "class:", "at:local:", "pos:local:"];
    let mut g = Lcg(12345);
    let mut lists = 0;
    for case in 0..3000 {
        let nkeys = 1 + g.next(8);
        let mut reached: Vec<(String, Vec<i64>)> = Vec::new();
        let mut atoms: HashMap<(String, Option<i64>), i64> = HashMap::new();
        let mut entry = serde_json::Map::new();
        let mut root = serde_json::Map::new();
        let mut held = Vec::new();
        let mut tried = Vec::new();
        for i in 0..nkeys {
            let k = if g.next(10) == 0 { "room".to_string() } else { format!("{}{}", kinds[g.next(kinds.len() as u64) as usize], i) };
            if reached.iter().any(|(x, _)| *x == k) {
                continue;
            }
            let vals: BTreeSet<i64> = (0..1 + g.next(6)).map(|_| g.next(9) as i64 - 2).collect();
            for &v in &vals {
                if g.next(3) == 0 {
                    atoms.insert((k.clone(), Some(v)), g.next(4) as i64);
                }
                if g.next(6) == 0 {
                    held.push(json!([k, v]));
                }
                if g.next(7) == 0 {
                    tried.push(json!([k, v]));
                }
            }
            if g.next(2) == 0 {
                atoms.insert((k.clone(), None), g.next(4) as i64);
            }
            if g.next(8) == 0 {
                held.push(json!([k, null]));
            }
            entry.insert(k.clone(), json!(g.next(9) as i64 - 2));
            root.insert(k.clone(), json!(g.next(9) as i64 - 2));
            reached.push((k, vals.into_iter().collect()));
        }
        let gl: Vec<(String, Vec<i64>)> = match reached.first() {
            Some((k, vs)) if g.next(2) == 0 => vec![(k.clone(), vec![vs[0]])],
            _ => vec![],
        };
        let ef = fact_map(&Value::Object(entry));
        let rf = fact_map(&Value::Object(root));
        let rfo = if g.next(2) == 0 { Some(&rf) } else { None };
        let a = step::targets_with(&reached, &atoms, &ef, rfo, &held, &tried, &gl, None).unwrap();
        let b = step::targets_per_key(&reached, &atoms, &ef, rfo, &held, &tried, &gl, None, None).unwrap();
        assert_eq!(pairs(&a), pairs(&b), "case {case}");
        let mut seen = BTreeSet::new();
        for x in &b {
            if let TV::Values(vs) = &x.v {
                lists += 1;
                assert!(vs.len() >= 2 && seen.insert(x.k.clone()), "case {case}: {x:?}");
                assert!(vs.iter().all(|v| !atoms.contains_key(&(x.k.clone(), Some(*v)))), "case {case}: {x:?}");
            }
        }
        if !b.iter().any(|x| matches!(x.v, TV::Values(_))) {
            assert_eq!(a, b, "case {case}");
        }
    }
    assert!(lists > 500, "{lists} value lists in the generated cases");
}
