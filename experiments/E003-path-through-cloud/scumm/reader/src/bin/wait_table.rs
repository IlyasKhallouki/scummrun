//! wait-table: the duration rules' table for taskcore's `--timed` (experiments/E004-fast-exact/durations.md), from the
//! game's IR. An instrument: it writes one file and changes no reading.
//!
//! usage: wait-table (--reading R/reading.json | --dmax D [--charinc C]) --out FILE
//!
//! Lines, tab-separated:
//!   wait  NAME ROOM W KINDS RULE   every wait point of every unit (costs.rs `FrameCosts::rows`): the yields that end
//!                                  there and costs.rs's rule with its frames, with the reading's `dmax` and
//!                                  VAR_CHARINC (its head's `frame_costs`), or D and C
//!   recursive NAME ROOM CITE       a unit a recursive start can reach (D7), with the first site that does
//!   wild NAME CITE EXPR            a recursive start (or a chainScript from a recursive unit) whose script number
//!                                  does not fold: every global and local script is then recursive
//!   walk  NAME ROOM W ACTOR OBJECT a WaitForActor after walkActorToObject on the straight code before it (D9)
//!   const KEY START WRITTEN        with --reading: var 19, 57, 37 and 1 at the reading's start, and whether the
//!                                  scope's write set holds them (scope.rs `Writes::has`)
//!   scope ROOMS UNITS              with --reading: the scope's rooms and its number of units
//! ROOM is the room of a local, entry or exit unit, else `-`. NAME is the unit as the task's labels write it.
//!
//! With --reading the recursive starts are those of the scope's units (a route that stays in the scope runs no other
//! unit), and a start whose number is a variable, or a variable at a constant base plus an index that does not fold,
//! has as its targets the start's values of those variables when the scope's write set holds none of them (a 0 starts
//! nothing, script.cpp:61-62); otherwise it stays wild. Without --reading every site of the game counts.

use scumm_ir::ast::{Expr, ExprId, RefKind, Stmt, UnitKey};
use scumm_ir::{known, Game};
use scumm_reader::costs::{self, FrameCosts};
use scumm_reader::facts::unit_name;
use scumm_reader::{oracle, Facts, Scope};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;

/// A recursive start's script number that does not fold: the start's values it can take, if the scope writes none
/// of the variables it reads (module docstring), as script numbers (0s left out); None if wild.
fn resolve(game: &Game, e: ExprId, scope: &Scope, start: &Facts) -> Option<Vec<i32>> {
    let ast = &game.ast;
    let vars: Vec<i32> = match ast.expr(e) {
        Expr::Var(n) => vec![n],
        // Var[base + i]: the writer's window of 16 (scope.rs `scope_writes`, `var_windows`)
        Expr::Index { kind: RefKind::Var, base, .. } => (base..base + 16).collect(),
        _ => return None,
    };
    if vars.iter().any(|v| scope.writes.has(&format!("var:{v}"))) {
        return None;
    }
    let mut out: Vec<i32> = vars.iter().map(|v| start.get0(&format!("var:{v}")) as i32).filter(|&n| n != 0).collect();
    out.sort_unstable();
    out.dedup();
    Some(out)
}

fn room_of(k: UnitKey) -> String {
    match k {
        UnitKey::Local(r, _) | UnitKey::Entry(r) | UnitKey::Exit(r) => r.to_string(),
        _ => "-".to_string(),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |k: &str| args.iter().position(|a| a == k).map(|i| args[i + 1].clone());
    let out = arg("--out").expect("--out");
    let game = oracle::load_game();
    let reading: Option<serde_json::Value> =
        arg("--reading").map(|p| serde_json::from_slice(&std::fs::read(&p).expect("--reading")).expect("reading.json"));
    let (dmax, charinc) = match &reading {
        Some(r) => (r["frame_costs"]["dmax"].as_i64().expect("a reading with --frame-costs"), r["frame_costs"]["charinc"].as_i64()),
        None => (arg("--dmax").expect("--dmax").parse().expect("dmax"), arg("--charinc").map(|c| c.parse().expect("charinc"))),
    };
    let scope = reading.as_ref().map(|r| {
        let start = Facts::from_items(
            r["start"]["facts"].as_object().expect("start facts").iter().map(|(k, v)| (k.clone(), v.as_i64().expect("an int"))),
        );
        let rooms: Vec<i32> = r["scope"]["rooms"].as_array().expect("rooms").iter().map(|x| x.as_i64().unwrap() as i32).collect();
        let sw = scumm_reader::builder::Switches::parse(r["switches"].as_str().unwrap_or("")).expect("switches");
        let sc = Scope::of_with(&game, &rooms, &start, sw.scope());
        (sc, start)
    });
    let fc = FrameCosts::with(&game, dmax, charinc);
    let mut f = std::io::BufWriter::new(std::fs::File::create(&out).expect("--out"));
    writeln!(f, "# wait-table: dmax {dmax}, charinc {charinc:?}").unwrap();
    if let Some((sc, start)) = &scope {
        let rooms: Vec<String> = sc.rooms.iter().map(|r| r.to_string()).collect();
        writeln!(f, "scope\t{}\t{}", rooms.join(","), sc.units.len()).unwrap();
        for k in ["var:19", "var:57", "var:37", "var:1"] {
            writeln!(f, "const\t{k}\t{}\t{}", start.get0(k), sc.writes.has(k) as u8).unwrap();
        }
    }
    let in_scope = |u| scope.as_ref().is_none_or(|(sc, _)| sc.units.contains(&u));
    for (u, w, kinds, rule) in fc.rows() {
        writeln!(f, "wait\t{}\t{}\t{}\t{}\t{}", unit_name(u), room_of(u), w, kinds.join(","), rule).unwrap();
    }
    // D7: the recursive units. Sites: startScript with the recursive flag; chainScript passes its slot's flag on
    let ast = &game.ast;
    let mut rec: BTreeMap<UnitKey, String> = BTreeMap::new();
    let mut chains: Vec<(UnitKey, Vec<UnitKey>, String)> = Vec::new();
    let mut wild: Vec<(String, String, String)> = Vec::new();
    let mut walks = 0usize;
    let expr_text = |e: ExprId| match game.ast.expr(e) {
        Expr::Var(n) => format!("var:{n}"),
        Expr::Index { kind: RefKind::Var, base, .. } => format!("var:{base}+index"),
        _ => "other".to_string(),
    };
    for u in game.ids() {
        let key = game.unit(u).key;
        let here = in_scope(u);
        let room = match key {
            UnitKey::Local(r, _) | UnitKey::Entry(r) | UnitKey::Exit(r) => Some(r),
            _ => None,
        };
        for (i, x) in game.ins(u).iter().enumerate() {
            match x.st {
                Some(Stmt::Start { name, script, recursive, .. }) if here && name == known::START_SCRIPT && recursive => {
                    let ns = match ast.constant(script) {
                        Some(n) => Some(vec![n]),
                        None => scope.as_ref().and_then(|(sc, st)| resolve(&game, script, sc, st)),
                    };
                    match ns {
                        None => wild.push((unit_name(key), game.cite(u, i as u32), expr_text(script))),
                        Some(ns) => {
                            for n in ns {
                                for dst in game.started(n, room) {
                                    rec.entry(dst).or_insert_with(|| game.cite(u, i as u32));
                                }
                            }
                        }
                    }
                }
                Some(Stmt::Start { name, script, .. }) if here && name == known::CHAIN_SCRIPT => {
                    let ns = match ast.constant(script) {
                        Some(n) => Some(vec![n]),
                        None => scope.as_ref().and_then(|(sc, st)| resolve(&game, script, sc, st)),
                    };
                    let cite = game.cite(u, i as u32);
                    match ns {
                        None => chains.push((key, Vec::new(), format!("{cite}\t{}", expr_text(script)))),
                        Some(ns) => chains.push((key, ns.iter().flat_map(|&n| game.started(n, room)).collect(), cite)),
                    }
                }
                Some(Stmt::Op { name, .. }) if name == known::WAIT_FOR_ACTOR => {
                    if let Some((a, o)) = costs::walk_before(&game, u, i as u32) {
                        writeln!(f, "walk\t{}\t{}\t{}\t{}\t{}", unit_name(key), room_of(key), x.offset, a, o).unwrap();
                        walks += 1;
                    }
                }
                _ => {}
            }
        }
    }
    let mut changed = true;
    while changed {
        changed = false;
        for (src, dsts, cite) in &chains {
            if !rec.contains_key(src) {
                continue;
            }
            if dsts.is_empty() && cite.contains('\t') && !wild.iter().any(|(_, c, _)| c == cite) {
                let (c, e) = cite.split_once('\t').unwrap();
                wild.push((unit_name(*src), c.to_string(), e.to_string()));
                changed = true;
            }
            for d in dsts {
                if !rec.contains_key(d) {
                    rec.insert(*d, cite.clone());
                    changed = true;
                }
            }
        }
    }
    for (k, cite) in &rec {
        writeln!(f, "recursive\t{}\t{}\t{}", unit_name(*k), room_of(*k), cite).unwrap();
    }
    for (n, cite, e) in &wild {
        writeln!(f, "wild\t{n}\t{cite}\t{e}").unwrap();
    }
    let names: BTreeSet<String> = rec.keys().map(|k| unit_name(*k)).collect();
    eprintln!(
        "wait-table: {} wait points, {} recursive units ({} names), {} wild recursive starts, {} walk waits: {out}",
        fc.rows().len(),
        rec.len(),
        names.len(),
        wild.len(),
        walks
    );
}
