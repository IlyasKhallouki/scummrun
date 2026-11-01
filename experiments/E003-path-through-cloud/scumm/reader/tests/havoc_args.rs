//! The switch havoc_args (experiments/E005-narrow-havoc/havoc-args.md): a start in a havoc closure whose number is a
//! local or a variable indexed by one names the scripts of its values, from a static analysis of the game's code; off,
//! every unit. On Part I's scope, with MI1's two starts that made E004's rough reading name every unit.

use scumm_ir::{known, Stmt, UnitId, UnitKey};
use scumm_reader::args::HavocArgs;
use scumm_reader::builder::Switches;
use scumm_reader::havoc;
use scumm_reader::oracle;
use scumm_reader::record::Cut;
use scumm_reader::Builder;
use std::collections::BTreeSet;
use std::sync::{Arc, OnceLock};

fn game() -> &'static scumm_ir::Game {
    static G: OnceLock<scumm_ir::Game> = OnceLock::new();
    G.get_or_init(oracle::load_game)
}

/// Part I's scope as reader-scope --part1 reads it, with run D's switches, havoc_cut and havoc_starts.
fn part1() -> Builder<'static> {
    let g = game();
    let root = scumm_ir::default_root();
    let start = scumm_reader::scaling::start_facts(&root, 1);
    let goal = scumm_reader::scaling::part1_goal(&root);
    let rooms = scumm_reader::scaling::part1_rooms(g, &root);
    let sw = Switches::parse("value_sets,draw_clears,actor_rooms,split_limit,grow_limit,havoc_cut,havoc_starts").unwrap();
    let doms = scumm_reader::domains::build_with(g, &scumm_reader::domains::meta_rooms(&root), &start, &goal, sw.scope());
    let scope = scumm_reader::Scope::of_with(g, &rooms, &start, sw.scope());
    let mut b = Builder::new(g, scope, doms);
    b.switches = sw;
    b.table.havoc_cut = true;
    b.table.havoc_starts = true;
    b
}

/// The index of the first `startScript` of global `n` whose number is not a constant.
fn computed_start(n: i32) -> (UnitId, u32) {
    let g = game();
    let u = g.id(UnitKey::Global(n)).unwrap();
    let i = g
        .ins(u)
        .iter()
        .position(|x| matches!(x.st, Some(Stmt::Start { name, script, .. }) if name != known::START_OBJECT && g.ast.constant(script).is_none()))
        .unwrap();
    (u, i as u32)
}

fn script_of(u: UnitId, i: u32) -> scumm_ir::ExprId {
    match game().ins(u)[i as usize].st {
        Some(Stmt::Start { script, .. }) => script,
        _ => unreachable!(),
    }
}

#[test]
fn part1_the_analysis_names_what_the_callers_pass() {
    let b = part1();
    let g = game();
    let a = HavocArgs::of(g, &b.scope, &b.table);
    // global 73 starts Local[1]: globals 116 and 114 pass 58 and 200 (and its guard drops 0)
    let (u, i) = computed_start(73);
    let v = a.values_at(g, &b.scope, &b.table, u, i, script_of(u, i), 256);
    assert_eq!(v, Some(BTreeSet::from([58, 200])));
    // global 23 starts Var[396 + Local[0]] with Local[0] 1 to 12 there (its guards; findObject and actorFromPos are
    // never negative; the inventory slots it reads hold 0 by the domain convention): those variables hold 0
    let (u, i) = computed_start(23);
    let l = a.locals_at(u, i).unwrap();
    assert_eq!(l[0].values(64), Some((1..=12).collect()), "Local[0] at global 23's start: {:?}", l[0]);
    let v = a.values_at(g, &b.scope, &b.table, u, i, script_of(u, i), 256);
    assert_eq!(v, Some(BTreeSet::from([0])));
    assert!(a.wild.is_empty(), "no computed start of the game is left unknown: {:?}", a.wild);
}

/// A cut at the computed start: off it names every unit; on, only what the analysis gives.
fn closure_at_start(b: &mut Builder, n: i32, on: bool) -> havoc::Closure {
    let g = game();
    b.table.havoc_args = on.then(|| Arc::new(HavocArgs::of(g, &b.scope, &b.table)));
    let (u, i) = computed_start(n);
    let cut = Cut { frames: vec![(g.unit(u).key, Some(i))], steps: vec![] };
    havoc::closure(g, &b.scope, &b.table, &cut, g.resource(85))
}

#[test]
fn part1_a_cut_at_global_73s_start_names_its_callers_scripts() {
    let mut b = part1();
    let off = closure_at_start(&mut b, 73, false);
    assert!(off.every_unit, "off, a start by a local names every unit");
    assert!(off.why.iter().any(|w| w.contains("global") && w.contains("73")), "{:?}", off.why);
    let on = closure_at_start(&mut b, 73, true);
    assert!(!on.why.iter().any(|w| w.contains("start not folded")), "{:?}", on.why);
    let g = game();
    let keys: BTreeSet<UnitKey> = on.starts.keys().map(|&u| g.unit(u).key).collect();
    assert!(keys.contains(&UnitKey::Global(58)), "global 58 is in the closure");
    assert!(keys.contains(&UnitKey::Local(85, 200)), "room 85's local 200 is in the closure");
    assert!(on.args_named >= 1, "{}", on.args_named);
    assert!(on.args_unknown.is_empty());
}

#[test]
fn part1_a_cut_at_global_23s_start_names_no_script_for_it() {
    let mut b = part1();
    let off = closure_at_start(&mut b, 23, false);
    assert!(off.why.iter().any(|w| w.contains("start not folded")), "{:?}", off.why);
    let on = closure_at_start(&mut b, 23, true);
    assert!(!on.why.iter().any(|w| w.contains("start not folded")), "{:?}", on.why);
    assert!(!on.every_unit, "{:?}", on.why);
    assert!(on.args_named >= 1, "{}", on.args_named);
    assert!(on.args_unknown.is_empty(), "{:?}", on.args_unknown);
}

#[test]
fn off_the_table_holds_no_analysis() {
    let b = part1();
    assert!(b.table.havoc_args.is_none());
    assert!(!b.switches.havoc_args);
}
