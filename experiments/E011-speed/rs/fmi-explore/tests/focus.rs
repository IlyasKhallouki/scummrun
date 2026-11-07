//! E022 amendment (7): the plan focus (focus.rs) on a small task.

use fmi_core::{Action, Domain, Domains, EffVal, Effect, Facts, Key, Val};
use fmi_explore::focus::{family, focus_in, label_of, Index};

fn act(name: &str, pre: &[(&str, &[Val])], eff: &[(&str, Val)]) -> Action {
    Action {
        name: name.into(),
        pre: pre.iter().map(|(k, vs)| (Key::parse(k), vs.to_vec())).collect(),
        effects: eff.iter().map(|(k, v)| Effect { key: Key::parse(k), v: EffVal::Value(*v), when: vec![] }).collect(),
        what: 0,
        cost: 1,
    }
}

fn doms(keys: &[&str]) -> Domains {
    let mut d: Vec<(Key, Domain)> =
        keys.iter().map(|k| (Key::parse(k), Domain { values: vec![0, 1, 2], low: None, high: None })).collect();
    d.sort_by_key(|p| p.0);
    Domains(d)
}

#[test]
fn families_are_slots_and_labels_without_arguments() {
    assert_eq!(family("event(local:208@1214#1241aa247208)"), "event:local:208");
    assert_eq!(family("dialog_line(121)[17@49]"), "dialog_line()[17@49]");
    assert_eq!(family("sentence(look,(obj 592))[inv]~frozen"), "sentence()[inv]~frozen");
    assert_eq!(label_of("event(local:208@1214#1241aa247208)#5"), "event(local:208@1214#1241aa247208)");
    assert_eq!(label_of("dialog_line(121)[17@49]"), "dialog_line(121)[17@49]");
}

#[test]
fn the_focus_takes_the_smallest_writer_of_what_the_goal_needs() {
    let d = doms(&["bit:1", "var:2", "var:3", "pos:local:9"]);
    let acts = vec![
        // the goal's writer needs var:2 = 1
        act("event(local:1@10)#0", &[("var:2", &[1])], &[("bit:1", 1)]),
        // a small writer of var:2 = 1 and a big one
        act("click(5)[17@1]#1", &[], &[("var:2", 1)]),
        act("event(local:9@1)#2", &[], &[("var:2", 1), ("pos:local:9", 2)]),
        act("event(local:9@2)#3", &[], &[("pos:local:9", 1)]),
        act("event(local:9@3)#4", &[], &[("var:3", 2)]),
        // a family nothing needs
        act("event(script:4@7)#5", &[], &[("var:3", 1)]),
    ];
    let ix = Index::of(acts, &d);
    let start = Facts::from_pairs(vec![(Key::parse("bit:1"), 0), (Key::parse("var:2"), 0), (Key::parse("var:3"), 0)]);
    let f = focus_in(&ix, &d, &start, &[(Key::parse("bit:1"), vec![1])]).expect("a focus");
    assert_eq!(f.families, vec!["event:local:1".to_string(), "click()[17@1]".to_string()]);
    assert_eq!(f.actions, 2);
    let banned: Vec<&str> = f.bans.iter().map(|b| b.label.as_str()).collect();
    assert_eq!(banned, vec!["event(local:9@1)", "event(local:9@2)", "event(local:9@3)", "event(script:4@7)"]);
    assert!(f.bans.iter().all(|b| b.facts.is_empty()));
}

#[test]
fn a_fact_the_start_lacks_starts_at_0_as_the_planners_start_has_it() {
    let d = doms(&["bit:1", "var:2", "var:3"]);
    let acts = vec![
        // the goal's writer needs var:2 = 0, which the start does not name
        act("event(local:1@10)#0", &[("var:2", &[0])], &[("bit:1", 1)]),
        act("event(script:4@7)#1", &[], &[("var:3", 1)]),
    ];
    let ix = Index::of(acts, &d);
    let start = Facts::from_pairs(vec![(Key::parse("bit:1"), 0)]);
    let f = focus_in(&ix, &d, &start, &[(Key::parse("bit:1"), vec![1])]).expect("a focus");
    assert_eq!(f.families, vec!["event:local:1".to_string()]);
}

#[test]
fn no_focus_when_the_goal_needs_every_family_or_has_no_writer() {
    let d = doms(&["bit:1", "var:2"]);
    let acts = vec![
        act("event(local:1@10)#0", &[("var:2", &[1])], &[("bit:1", 1)]),
        act("click(5)[17@1]#1", &[], &[("var:2", 1)]),
    ];
    let ix = Index::of(acts, &d);
    let start = Facts::from_pairs(vec![(Key::parse("bit:1"), 0), (Key::parse("var:2"), 0)]);
    assert!(focus_in(&ix, &d, &start, &[(Key::parse("bit:1"), vec![1])]).is_none());
    assert!(focus_in(&ix, &d, &start, &[(Key::parse("bit:1"), vec![2])]).is_none());
}
