//! The switch havoc_cut (experiments/E004-fast-exact/havoc-cut.md): each path the step limit ended becomes one havoc
//! record, with its precondition, a guess or a constant-or-nothing case for each fact the code it can still run may
//! write, and an event with unknown locals at each wait point of that code; off, the reading stops. The ignored test
//! runs the Part I job whose paths ended at the step limit, with the Rust machine.

use scumm_ir::UnitKey;
use scumm_reader::builder::{Job, Machine, RunOut, Switches};
use scumm_reader::domain::Label;
use scumm_reader::oracle::{self, Static};
use scumm_reader::record::{Cut, PathOut, Pins, Pre, Record, Val, What};
use scumm_reader::{havoc, Builder, Ctx, Eff};
use std::collections::BTreeMap;
use std::sync::OnceLock;

fn game() -> &'static scumm_ir::Game {
    static G: OnceLock<scumm_ir::Game> = OnceLock::new();
    G.get_or_init(oracle::load_game)
}

fn static_oracle() -> &'static Static {
    static S: OnceLock<Static> = OnceLock::new();
    S.get_or_init(Static::load)
}

/// A machine whose run has one path that ends and `cuts` paths the step limit ended at `cut`.
struct Fake {
    cut: Cut,
    cuts: usize,
    write: (String, Val),
}

impl Machine for Fake {
    fn run(&mut self, _cx: &Ctx, _label: &str, _pins: &Pins, pre: &Pre, _what: &What) -> RunOut {
        let writes = BTreeMap::from([self.write.clone()]);
        let mut paths = vec![PathOut { end: Some("end".into()), sym: true, pre: pre.clone(), writes, ..Default::default() }];
        for _ in 0..self.cuts {
            paths.push(PathOut { end: Some("limit".into()), sym: true, pre: pre.clone(), cut: Some(self.cut.clone()), ..Default::default() });
        }
        RunOut { paths, ..Default::default() }
    }
}

fn job() -> Job {
    Job { kind: "dialog_line".into(), label: "fake".into(), pins: Pins::new(), pre: Pre::new(), what: What::Verb(1), checks: vec![] }
}

/// Room 33's builder, and a unit of its scope whose code writes a fact by a constant, cut at its first instruction.
fn setup(on: bool) -> (Builder<'static>, Fake, havoc::HavocWrites, UnitKey) {
    let sw = Switches { split_limit: true, havoc_cut: on, ..Default::default() };
    let mut b = oracle::builder_with(game(), static_oracle(), "room33", sw);
    b.table.split_limit = true;
    b.table.havoc_cut = on;
    let mut found = None;
    for &u in &b.scope.units {
        let k = game().unit(u).key;
        let cut = Cut { frames: vec![(k, Some(0))], steps: vec![] };
        let hw = havoc::writes(game(), &b.scope, &b.table, &cut, None);
        if hw.facts.values().any(|v| v.is_some()) && !hw.slots.is_empty() {
            found = Some((cut, hw, k));
            break;
        }
    }
    let (cut, hw, k) = found.expect("a unit of room 33's scope that writes a fact by a constant");
    let key = b.table.doms.doms.keys().find(|k| k.starts_with("var:") && b.kind(k) == scumm_reader::table::Kind::Fact).unwrap().clone();
    let v = b.domain(&key).unwrap().values.iter().find_map(|l| l.int()).unwrap();
    (b, Fake { cut, cuts: 2, write: (key, Val::Int(v)) }, hw, k)
}

fn havocs(recs: &[Record]) -> Vec<&Record> {
    recs.iter().filter(|r| r.end.as_deref() == Some("havoc")).collect()
}

#[test]
fn each_cut_path_is_one_havoc_record() {
    let (mut b, mut m, hw, unit) = setup(true);
    let recs = b.run_job(&mut m, &job());
    let h = havocs(&recs);
    assert_eq!(h.len(), 2, "one havoc record per path the step limit ended");
    assert_eq!(recs.len(), 3, "and the path that ended, as before");
    assert_eq!(b.counts.get("havoc_cut: paths the step limit ended, made havoc records: dialog_line"), 2);
    let r = h[0];
    // every fact the cut code may write is written: a guess, or cases on its current value
    for (k, vals) in &hw.facts {
        let Some(e) = r.effects.iter().find(|e| &e.0 == k) else {
            assert!(b.domain(k).is_none_or(|d| d.values.len() < 2), "{k} not written");
            continue;
        };
        match (vals, &e.1) {
            (None, Eff::Guess(g)) => assert_eq!(g, &b.domain(k).unwrap().values),
            (Some(vs), Eff::Cases(cs)) => {
                for (when, out) in cs {
                    assert_eq!(when.len(), 1);
                    assert!(out.contains(&when[0].1[0]), "{k}: no change is a choice");
                    for v in vs {
                        assert!(out.contains(&Label::Int(*v)), "{k}: {v} is a choice");
                    }
                }
            }
            (Some(_), Eff::Guess(_) | Eff::Set(_)) => {}
            (v, e) => panic!("{k}: {v:?} as {e:?}"),
        }
    }
    // the cut unit's slot: anywhere or nowhere, its list unknown, an event with unknown locals at each wait point
    let name = scumm_ir::Game::unit_name(unit).to_string();
    assert!(r.effects.iter().any(|e| e.0 == format!("args:{name}") && e.1 == Eff::Set(Label::Int(0))));
    let u = game().id(unit).unwrap();
    for w in scumm_reader::scope::stop_points(game(), u) {
        if b.table.is_point("at", &name, w as i64) {
            assert!(r.next_events.iter().any(|e| e.name == name && e.w == w as i64 && e.sig == 0 && !e.abort), "{name}@{w}");
        }
    }
    assert_eq!(r.pre, recs[0].pre, "the path's precondition");
}

#[test]
#[should_panic(expected = "paths the step limit ended; the reading stops")]
fn off_the_reading_stops() {
    let (mut b, mut m, _, _) = setup(false);
    b.run_job(&mut m, &job());
}

#[test]
fn off_the_machine_hands_over_no_cut() {
    let (b, _, _, _) = setup(false);
    assert!(!b.table.havoc_cut);
    let (b, _, _, _) = setup(true);
    assert!(b.table.havoc_cut);
}

/// The machine half, keeping where each cut path stood.
struct Keep<'g> {
    rd: scumm_reader::machine::Reader<'g>,
    cuts: Vec<Cut>,
}

impl Machine for Keep<'_> {
    fn run(&mut self, cx: &Ctx, label: &str, pins: &Pins, pre: &Pre, what: &What) -> RunOut {
        let out = self.rd.run(cx, label, pins, pre, what);
        self.cuts.extend(out.paths.iter().filter_map(|p| p.cut.clone()));
        out
    }
    fn set_path_limit(&mut self, n: usize) {
        self.rd.set_path_limit(n);
    }
}

/// Part I's scope as reader-scope --part1 reads it, with run D's switches (E004's lab note of 2026-10-31).
fn part1(havoc: bool) -> Builder<'static> {
    let g = game();
    let root = scumm_ir::default_root();
    let start = scumm_reader::scaling::start_facts(&root, 1);
    let goal = scumm_reader::scaling::part1_goal(&root);
    let rooms = scumm_reader::scaling::part1_rooms(g, &root);
    let sw = Switches::parse("value_sets,draw_clears,actor_rooms,split_limit,grow_limit").unwrap();
    let sw = Switches { havoc_cut: havoc, ..sw };
    let doms = scumm_reader::domains::build_with(g, &scumm_reader::domains::meta_rooms(&root), &start, &goal, sw.scope());
    let scope = scumm_reader::Scope::of_with(g, &rooms, &start, sw.scope());
    let mut b = Builder::new(g, scope, doms);
    b.switches = sw;
    b.table.elsewhere = true;
    b.table.live_locals = Some(std::sync::Arc::new(scumm_reader::locals::LiveLocals::of(g)));
    b.table.value_sets = true;
    b.table.split_limit = true;
    b.table.grow_limit = true;
    b.table.havoc_cut = havoc;
    b
}

const STOP: &str = "dialog_line(120)[204@28]";

#[test]
#[ignore]
fn part1_the_job_that_stopped_the_reading() {
    let mut b = part1(true);
    let jobs: Vec<Job> = b.inputs().into_iter().filter(|j| j.label == STOP).collect();
    assert_eq!(jobs.len(), 1, "{STOP} among Part I's inputs");
    let mut m = Keep { rd: scumm_reader::machine::Reader::new(game()), cuts: vec![] };
    let t = std::time::Instant::now();
    let recs = b.run_job(&mut m, &jobs[0]);
    let h = havocs(&recs);
    eprintln!("{STOP}: {} records, {} havoc records, {:.1} s", recs.len(), h.len(), t.elapsed().as_secs_f64());
    assert!(!m.cuts.is_empty(), "paths the step limit ended");
    assert_eq!(h.len(), m.cuts.len(), "one havoc record per path the step limit ended");
    for (c, r) in m.cuts.iter().zip(&h) {
        let hw = havoc::writes(game(), &b.scope, &b.table, c, Some(28));
        let facts: Vec<String> = hw.facts.iter().map(|(k, v)| format!("{k}{}", if v.is_some() { "=c" } else { "" })).collect();
        eprintln!(
            "  cut at {:?}, pending {:?}; closure {} units; {} facts ({} with constants), {} slots; record: {} effects, {} events",
            c.frames,
            c.steps,
            hw.closure_units,
            hw.facts.len(),
            hw.facts.values().filter(|v| v.is_some()).count(),
            hw.slots.len(),
            r.effects.len(),
            r.next_events.len()
        );
        eprintln!("  facts: {}", facts.iter().take(40).cloned().collect::<Vec<_>>().join(" "));
        let top = c.frames.last().unwrap();
        assert_eq!(top.0, UnitKey::Local(28, 204), "the cut path runs room 28's local 204");
    }
    // off, the same job stops the reading
    let mut b = part1(false);
    let job = jobs[0].clone();
    let mut m = scumm_reader::machine::Reader::new(game());
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| b.run_job(&mut m, &job)));
    let msg = r.err().and_then(|e| e.downcast_ref::<String>().cloned()).unwrap_or_default();
    assert!(msg.contains("paths the step limit ended; the reading stops"), "{msg}");
}

// --- the independent check's holes (1) and (4) to (6), on Part I's scope ------------------------------------------

use scumm_ir::{known, Expr, RefKind, Stmt, UnitId};
use scumm_reader::table::Kind;
use std::collections::BTreeSet;

/// A cut at a unit's start: its first instruction, or each verb entry of an object.
fn cut_at(u: UnitId) -> Cut {
    let unit = game().unit(u);
    let frames = match unit.key {
        UnitKey::Obj(_) => unit.entries.iter().map(|e| (unit.key, Some(e.1))).collect(),
        k => vec![(k, Some(0))],
    };
    Cut { frames, steps: vec![] }
}

/// The room resource room code runs in.
fn res_of(u: UnitId) -> Option<i32> {
    match game().unit(u).key {
        UnitKey::Local(r, _) | UnitKey::Entry(r) | UnitKey::Exit(r) => Some(r),
        _ => None,
    }
}

/// The ops of a unit's own code.
fn ops(u: UnitId) -> impl Iterator<Item = (scumm_ir::Sym, Vec<scumm_ir::ExprId>, scumm_ir::Span)> {
    let g = game();
    g.ins(u).iter().filter_map(move |x| match x.st {
        Some(Stmt::Op { name, args, subops, .. }) => Some((name, g.ast.kids(args).to_vec(), subops)),
        _ => None,
    })
}

fn is_fact(b: &Builder, k: &str) -> bool {
    (b.table.doms.doms.contains_key(k) || b.table.extra.contains_key(k)) && b.kind(k) == Kind::Fact
}

/// Hole (1): with F5, a drawObject with setXY moves its object, so the havoc record may change `objpos:O`, whether or
/// not the code can change the room.
#[test]
fn part1_a_draw_with_set_xy_moves_its_object() {
    let b = part1(true);
    let g = game();
    let (mut units, mut sites) = (0, 0);
    for &u in &b.scope.units {
        let mut moved = BTreeSet::new();
        for (name, args, subops) in ops(u) {
            let xy = g.ast.subs(subops).iter().any(|s| s.name == known::SET_XY && g.ast.constant(g.ast.kids(s.args)[0]) != Some(255));
            if name != known::DRAW_OBJECT || !xy {
                continue;
            }
            if let Some(o) = g.ast.constant(args[0]) {
                let k = format!("objpos:{o}");
                if g.movable.contains_key(&o) && is_fact(&b, &k) {
                    moved.insert(k);
                }
            }
        }
        if moved.is_empty() {
            continue;
        }
        units += 1;
        let hw = havoc::writes(g, &b.scope, &b.table, &cut_at(u), res_of(u));
        for k in &moved {
            sites += 1;
            assert!(hw.facts.contains_key(k), "{:?}: a drawObject with setXY moves {k}", g.unit(u).key);
        }
    }
    eprintln!("{units} units, {sites} objects moved by setXY");
    assert!(sites > 0);
}

/// Hole (4): setOwnerOf(O, 0) stops O's scripts (clearOwnerOf, object.cpp:131-133, 176-180), so O's slots are in the
/// closure's stops. Each cut is at the setOwnerOf itself. In Part I the inventory hook it runs reaches code that names
/// every unit, so the hook is taken away here (VAR_INVENTORY_SCRIPT's domain {0}): the stop must not hang on it.
#[test]
fn part1_set_owner_of_to_0_stops_the_objects_scripts() {
    let mut b = part1(true);
    b.table.extra.insert("var:34".into(), scumm_reader::Domain::of([0], None, None));
    let g = game();
    let (mut n, mut all) = (0, 0);
    for &u in &b.scope.units {
        let k = g.unit(u).key;
        for (i, x) in g.ins(u).iter().enumerate() {
            let Some(Stmt::Op { name: known::SET_OWNER_OF, args, .. }) = x.st else { continue };
            let args = g.ast.kids(args);
            if args.len() < 2 || g.ast.constant(args[1]) != Some(0) {
                continue;
            }
            let Some(o) = g.ast.constant(args[0]) else { continue };
            let Some(ou) = g.id(UnitKey::Obj(o)) else { continue };
            all += 1;
            let cut = Cut { frames: vec![(k, Some(i as u32))], steps: vec![] };
            let c = havoc::closure(g, &b.scope, &b.table, &cut, res_of(u));
            if c.stops_all || c.starts.contains_key(&ou) {
                continue;
            }
            n += 1;
            assert!(c.stops.contains(&UnitKey::Obj(o)), "{k:?}: setOwnerOf({o}, 0) stops object {o}'s scripts");
            let hw = havoc::writes(g, &b.scope, &b.table, &cut, res_of(u));
            assert!(hw.slots.contains(&format!("obj:{o}")));
        }
    }
    eprintln!("{n} of {all} setOwnerOf(O, 0) sites with a closure that stops not every slot and runs no code of O");
    assert!(n > 0);
}

/// Hole (5): an indexed write whose index does not fold lands on the base plus each value of the index variable's
/// domain, or on any variable; not on a window of 16 after the base.
#[test]
fn part1_an_index_that_does_not_fold_is_not_a_window() {
    let b = part1(true);
    let g = game();
    let all_vars: Vec<String> = b.table.doms.doms.keys().filter(|k| k.starts_with("var:") && is_fact(&b, k)).cloned().collect();
    let (mut sites, mut outside) = (0, 0);
    for &u in &b.scope.units {
        let mut caps = 0;
        for x in g.ins(u) {
            let Some(Stmt::Assign(a)) = x.st else { continue };
            let Expr::Index { kind: RefKind::Var, base, offset } = g.ast.expr(a.lhs) else { continue };
            if scumm_reader::domains::fold(&g.ast, offset, None, None, &mut caps).is_some() {
                continue;
            }
            let expect: Vec<String> = match g.ast.expr(offset) {
                Expr::Var(k) if is_fact(&b, &format!("var:{k}")) => match b.domain(&format!("var:{k}")).unwrap().values.iter().map(|l| l.int()).collect::<Option<Vec<i64>>>() {
                    Some(vs) => vs.into_iter().map(|v| format!("var:{}", base as i64 + v)).filter(|k| is_fact(&b, k)).collect(),
                    None => all_vars.clone(),
                },
                Expr::Var(_) => continue,
                _ => all_vars.clone(),
            };
            sites += 1;
            let hw = havoc::writes(g, &b.scope, &b.table, &cut_at(u), res_of(u));
            for k in &expect {
                let n: i32 = k[4..].parse().unwrap();
                if n >= base && n < base + 16 {
                    continue;
                }
                outside += 1;
                assert!(hw.facts.contains_key(k), "{:?}: Var[{base} + {:?}] can write {k}", g.unit(u).key, g.ast.expr(offset));
            }
        }
    }
    eprintln!("{sites} indexed writes whose index does not fold, {outside} targets outside the 16 after the base");
    assert!(outside > 0);
}
