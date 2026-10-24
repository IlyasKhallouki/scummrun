//! The canonical dump the oracle compares: the same lines dump_ir.py writes from the Python modules
//! (its docstring has the format). `lines` gives them for a loaded game, `noblocks_lines` for units
//! read without the raw blocks.

use crate::analysis::{getters, GetterKind, Loop};
use crate::arr;
use crate::ast::*;
use crate::facts::{self, Fact, FactMap};
use crate::game::Game;
use crate::json::{obj, J};
use crate::read::Script;
use crate::show::{head, show_stmt};
use crate::sym::Interner;
use crate::vars;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// An IR node as dump_ir.py's `enc`: [class name, fields...].
pub fn enc(ast: &Ast, syms: &Interner, e: ExprId) -> J {
    let list = |s: Span| J::Arr(ast.kids(s).iter().map(|&k| enc(ast, syms, k)).collect());
    match ast.expr(e) {
        Expr::Const(v) => arr!["Const", v],
        Expr::Var(n) => arr!["Var", n],
        Expr::Bit(n) => arr!["Bit", n],
        Expr::Local(n) => arr!["Local", n],
        Expr::Index { kind, base, offset } => arr!["Index", kind.as_str(), base, enc(ast, syms, offset)],
        Expr::BinOp { op, a, b } => arr!["BinOp", op.as_str(), enc(ast, syms, a), enc(ast, syms, b)],
        Expr::Neg(x) => arr!["Neg", enc(ast, syms, x)],
        Expr::Getter { name, args } => arr!["Getter", syms.str(name), list(args)],
        Expr::Embed(a) => arr!["Embed", enc_assign(ast, syms, a)],
        Expr::Str(y) => arr!["Str", syms.str(y)],
        Expr::Concat(s) => arr!["Concat", list(s)],
        Expr::List(s) => arr!["List", list(s)],
        Expr::Flag(y) => arr!["Flag", syms.str(y)],
        Expr::Cmp { op, a, b } => arr!["Cmp", op.as_str(), enc(ast, syms, a), enc(ast, syms, b)],
        Expr::Not(x) => arr!["Not", enc(ast, syms, x)],
        Expr::Truth(x) => arr!["Truth", enc(ast, syms, x)],
    }
}

fn enc_assign(ast: &Ast, syms: &Interner, a: AssignSt) -> J {
    arr!["Assign", enc(ast, syms, a.lhs), a.op.as_str(), enc(ast, syms, a.rhs)]
}

/// A statement as dump_ir.py's `enc`.
pub fn enc_stmt(ast: &Ast, syms: &Interner, st: &Stmt) -> J {
    let list = |s: Span| J::Arr(ast.kids(s).iter().map(|&k| enc(ast, syms, k)).collect());
    match *st {
        Stmt::Assign(a) => enc_assign(ast, syms, a),
        Stmt::Jump { cond, to, form } => arr!["Jump", cond.map(|c| enc(ast, syms, c)), to, form.as_str()],
        Stmt::Start { name, script, obj, args, freeze, recursive } => arr![
            "Start",
            syms.str(name),
            enc(ast, syms, script),
            obj.map(|o| enc(ast, syms, o)),
            list(args),
            freeze,
            recursive
        ],
        Stmt::Op { name, args, subops, flags } => arr![
            "Op",
            syms.str(name),
            list(args),
            J::Arr(ast.subs(subops).iter().map(|s| arr!["Sub", syms.str(s.name), list(s.args)]).collect()),
            J::Arr(ast.flags(flags).iter().map(|&f| J::from(syms.str(f))).collect())
        ],
    }
}

fn key_j(k: UnitKey) -> J {
    let (ns, len) = k.numbers();
    let mut v = vec![J::from(k.kind())];
    v.extend(ns[..len].iter().map(|&n| J::from(n)));
    J::Arr(v)
}

fn pairs<K: Into<J> + Copy + Ord, V: Into<J> + Copy>(m: impl IntoIterator<Item = (K, V)>) -> J {
    let mut v: Vec<(K, V)> = m.into_iter().collect();
    v.sort_by_key(|p| p.0);
    J::Arr(v.into_iter().map(|(k, x)| J::Arr(vec![k.into(), x.into()])).collect())
}

fn ins_obj(sc: &Script, u: UnitId, i: u32, x: &Ins) -> J {
    let (ast, syms) = (&sc.ast, &sc.syms);
    obj([
        ("offset", x.offset.into()),
        ("op", (x.op as i32).into()),
        ("text", syms.str(x.text).into()),
        ("file", syms.str(sc.unit(u).file).into()),
        ("line", x.line.into()),
        ("cite", sc.cite(u, i).into()),
        ("st", x.st.map(|s| enc_stmt(ast, syms, &s)).into()),
        ("show", x.st.map(|s| show_stmt(ast, syms, &s)).into()),
        ("head", x.st.map(|s| head(ast, syms, &s)).into()),
        ("next", x.next.into()),
        ("target", x.target.into()),
        ("esc", x.esc.into()),
        ("yields", x.yields.map(|y| y.as_str()).into()),
        ("ends", x.ends.map(|y| y.as_str()).into()),
        ("hidden", x.hidden.map(|y| y.as_str()).into()),
    ])
}

/// A unit's IR fields (dump_ir.py `unit_line` without a game).
pub fn unit_ir(sc: &Script, u: UnitId) -> J {
    let unit = sc.unit(u);
    let ins = sc.ins(u);
    let mut at: Vec<(i32, u32)> = Vec::new();
    for (i, x) in ins.iter().enumerate() {
        match at.last_mut() {
            Some(last) if last.0 == x.offset => last.1 = i as u32,
            _ => at.push((x.offset, i as u32)),
        }
    }
    obj([
        ("kind", "unit".into()),
        ("key", key_j(unit.key)),
        ("room", unit.room.into()),
        ("file", sc.syms.str(unit.file).into()),
        ("block", sc.syms.str(unit.block).into()),
        ("ins", J::Arr(ins.iter().enumerate().map(|(i, x)| ins_obj(sc, u, i as u32, x)).collect())),
        ("entries", J::Arr(unit.entries.iter().map(|&(v, i)| arr![v, i]).collect())),
        ("shadowed", J::Arr(unit.shadowed.iter().map(|&(v, i)| arr![v, i]).collect())),
        ("at", J::Arr(at.iter().map(|&(o, i)| arr![o, i]).collect())),
        ("notes", J::Arr(unit.notes.iter().map(|n| arr![n.class, n.line, n.detail.as_str()]).collect())),
    ])
}

fn loop_obj(lp: &Loop) -> J {
    obj([
        ("head", lp.head.into()),
        ("body", lp.body.clone().into()),
        ("exits", J::Arr(lp.exits.iter().map(|&(k, s)| arr![k, s]).collect())),
        ("kind", lp.kind.as_str().into()),
        ("reads", J::Arr(lp.reads.iter().map(|r| J::from(r.to_string())).collect())),
        ("non_facts", J::Arr(lp.non_facts.iter().map(|r| J::from(r.to_string())).collect())),
    ])
}

fn analysis(g: &Game, u: UnitId, cp: bool) -> J {
    let n = g.unit(u).ins_len;
    let te = |camera: bool| J::Arr(g.time_exits(u, camera, cp).into_iter().map(|(j, s, h)| arr![j, s, h]).collect());
    obj([
        ("loops", J::Arr(g.loops(u, cp).iter().map(loop_obj).collect())),
        (
            "wait_points",
            J::Arr(
                (0..=n)
                    .map(|idx| {
                        let wp = g.wait_point(u, idx, cp);
                        arr![wp.kind.as_str(), g.syms.str(wp.op), wp.loop_]
                    })
                    .collect(),
            ),
        ),
        ("time_exits", obj([("camera_off", te(false)), ("camera_on", te(true))])),
        ("time_loops_at", J::Arr((0..=n).map(|idx| g.time_loops_at(u, idx, cp).to_vec().into()).collect())),
    ])
}

/// A unit's line with its analyses (dump_ir.py `unit_line` with a game).
pub fn unit_line(g: &Game, u: UnitId, verbs: &[i32]) -> J {
    let mut d = unit_ir(g, u);
    if let J::Obj(m) = &mut d {
        if let Some(J::Arr(ins)) = m.get_mut("ins") {
            for (i, x) in ins.iter_mut().enumerate() {
                x.set("successors", g.successors(u, i as u32).to_vec());
            }
        }
    }
    let key = g.unit(u).key;
    d.set("unit_name", Game::unit_name(key).to_string());
    d.set("lifecycle_key", g.lifecycle_key(key).map(|f| f.to_string()));
    d.set("joins", g.join_points(u));
    if let UnitKey::Obj(o) = key {
        d.set("entry", J::Arr(verbs.iter().map(|&v| arr![v, g.entry(o, v)]).collect()));
    }
    d.set("const_polls", obj([("off", analysis(g, u, false)), ("on", analysis(g, u, true))]));
    d
}

/// dump_ir.py `tables`.
pub fn tables() -> J {
    let mut yields: Vec<(&str, &str)> = vec![
        ("breakHere", "frame"),
        ("delay", "delay"),
        ("delayVariable", "delay"),
        ("WaitForActor", "wait"),
        ("WaitForMessage", "wait"),
        ("WaitForCamera", "wait"),
        ("WaitForSentence", "wait"),
    ];
    yields.sort();
    let mut gs = getters();
    gs.sort_by_key(|g| g.1);
    obj([
        ("kind", "tables".into()),
        ("vars", J::Arr(vars::VARS.iter().map(|&(s, n)| arr![s, n]).collect())),
        ("var_name", J::Arr(vars::var_names().into_iter().map(|(n, s)| arr![n, s]).collect())),
        ("ambiguous", J::Arr(vars::AMBIGUOUS.iter().map(|&(s, ns)| arr![s, ns.to_vec()]).collect())),
        ("flags", vec!["F", "R", "STOP"].into()),
        ("yields", J::Arr(yields.iter().map(|&(a, b)| arr![a, b]).collect())),
        ("room_change", vec!["actorFollowCamera", "loadRoom", "loadRoomWithEgo"].into()),
        (
            "header",
            J::Arr(
                [("entry", 8), ("exit", 8), ("global", 8), ("local", 9), ("obj", 0)]
                    .iter()
                    .map(|&(k, v)| arr![k, v])
                    .collect(),
            ),
        ),
        ("jump", (crate::read::JUMP as i32).into()),
        ("default_verb", crate::read::DEFAULT_VERB.into()),
        (
            "getters",
            J::Arr(
                gs.into_iter()
                    .map(|(_, name, kind, cite)| {
                        let k = match kind {
                            GetterKind::Fact => "fact".to_string(),
                            GetterKind::NonFact(r) => r.to_string(),
                        };
                        arr![name, k, cite]
                    })
                    .collect(),
            ),
        ),
        ("hook_vars", crate::analysis::HOOK_VARS.collect::<Vec<i32>>().into()),
        ("time_vars", crate::analysis::TIME_VARS.to_vec().into()),
        ("clocks", facts::CLOCKS.to_vec().into()),
        (
            "wio",
            vec![facts::WIO_INVENTORY, facts::WIO_ROOM, facts::WIO_GLOBAL, facts::WIO_LOCAL, facts::WIO_FLOBJECT]
                .into(),
        ),
        ("room_code", vec![facts::ENCD, facts::EXCD].into()),
        ("facts_header", facts::HEADER.into()),
    ])
}

/// dump_ir.py `all_verbs`: every verb the game names, with 0 and 255.
pub fn all_verbs(meta: &Value) -> Vec<i32> {
    let mut vs: BTreeSet<i32> = [0, crate::read::DEFAULT_VERB].into_iter().collect();
    for k in meta["verbs"].as_object().expect("verbs").keys() {
        vs.insert(k.parse().expect("verb"));
    }
    for ob in meta["objects"].as_object().expect("objects").values() {
        if let Some(evs) = ob.get("verbs").and_then(|v| v.as_array()) {
            for e in evs {
                vs.insert(e["verb"].as_i64().expect("verb") as i32);
            }
        }
    }
    vs.into_iter().collect()
}

fn resistant_queries(g: &Game) -> J {
    let mut qs: BTreeSet<(&str, i32, i32)> = BTreeSet::new(); // (kind, number, room or -1)
    let keys = g.units.iter().map(|u| u.key).chain(g.resistance.keys().copied());
    for k in keys {
        match k {
            UnitKey::Global(n) => {
                qs.insert(("script", n, -1));
            }
            UnitKey::Local(r, n) => {
                qs.extend([("local", n, -1), ("local", n, r), ("local", n, 0)]);
            }
            _ => {}
        }
    }
    let mut out: Vec<J> = qs
        .into_iter()
        .map(|(kind, n, res)| {
            let key = if kind == "script" { Fact::Script(n) } else { Fact::Local(n) };
            let res = (res >= 0).then_some(res);
            arr![key.to_string(), res, g.resistant(key, res)]
        })
        .collect();
    out.push(arr!["objscript:1", J::Null, g.resistant(Fact::ObjScript(1), None)]);
    J::Arr(out)
}

fn enc_write(g: &Game, w: &crate::analysis::Write) -> J {
    match *w {
        crate::analysis::Write::Expr(e) => enc(&g.ast, &g.syms, e),
        crate::analysis::Write::Arith(op, a, b) => {
            arr!["BinOp", op.as_str(), enc(&g.ast, &g.syms, a), enc(&g.ast, &g.syms, b)]
        }
    }
}

/// dump_ir.py `game_line`.
pub fn game_line(g: &Game, meta: &Value) -> J {
    let mut writes: Vec<&(i32, Vec<crate::analysis::Write>)> = g.script_writes.iter().collect();
    writes.sort_by_key(|w| w.0);
    let mut resistance: Vec<(UnitKey, crate::game::Resist)> = g.resistance.iter().map(|(k, v)| (*k, *v)).collect();
    resistance.sort_by_key(|r| r.0);
    let mut nonfacts: Vec<(i32, String)> = g.nonfacts.iter().map(|(k, v)| (*k, v.to_string())).collect();
    nonfacts.sort();
    obj([
        ("kind", "game".into()),
        ("units", J::Arr(g.units.iter().map(|u| key_j(u.key)).collect())),
        ("first_local", g.first_local.into()),
        ("local_rooms", J::Arr(g.local_rooms.iter().map(|(n, rs)| arr![*n, rs.clone()]).collect())),
        ("object_room", pairs(g.object_room.iter().map(|(k, v)| (*k, *v)))),
        ("object_parent", pairs(g.object_parent.iter().map(|(k, v)| (*k, *v)))),
        ("verb_offset", pairs(g.verb_offset.iter().map(|(k, v)| (*k, *v)))),
        ("room_width", pairs(g.room_width.iter().map(|(k, v)| (*k, *v)))),
        ("pseudo", pairs(g.pseudo.iter().map(|(k, v)| (*k, *v)))),
        ("pseudo_conflicts", J::Arr(g.pseudo_conflicts.iter().map(|(c, d)| arr![c.as_str(), d.as_str()]).collect())),
        ("resistance", J::Arr(resistance.iter().map(|(k, r)| arr![key_j(*k), r.sorted()]).collect())),
        ("wild_starts", J::Arr(g.wild_starts.iter().map(|(c, f)| arr![c.as_str(), *f]).collect())),
        ("nonfacts", J::Arr(nonfacts.into_iter().map(|(n, r)| arr![n, r]).collect())),
        (
            "script_writes",
            J::Arr(writes.iter().map(|(n, ws)| arr![*n, J::Arr(ws.iter().map(|w| enc_write(g, w)).collect())]).collect()),
        ),
        (
            "const_poll_loops",
            J::Arr(g.const_poll_loops.iter().map(|&(u, h)| arr![key_j(g.unit(u).key), h]).collect()),
        ),
        ("resource", J::Arr((0..256).map(|r| arr![r, g.resource(r)]).collect())),
        ("resistant", resistant_queries(g)),
        ("all_verbs", all_verbs(meta).into()),
    ])
}

fn fact_obj(f: &FactMap) -> J {
    J::Obj(f.iter().map(|(k, v)| (k.to_string(), J::from(*v))).collect::<BTreeMap<String, J>>())
}

/// dump_ir.py `facts_line`, over the synthetic states (scumm/ir/tests/fact_states.json).
pub fn facts_line(cases: &Value) -> J {
    let cases = cases.as_array().expect("fact states");
    let got: Vec<FactMap> = cases.iter().map(|c| facts::facts_of(&c["state"])).collect();
    let mut diffs = Vec::new();
    for k in 1..cases.len() {
        let d = facts::diff(&got[k - 1], &got[k]);
        let dj = J::Obj(d.iter().map(|(f, &(a, b))| (f.to_string(), arr![a, b])).collect::<BTreeMap<String, J>>());
        diffs.push(obj([
            ("a", cases[k - 1]["name"].as_str().unwrap().into()),
            ("b", cases[k]["name"].as_str().unwrap().into()),
            ("diff", dj),
            ("patch", fact_obj(&facts::patch(&got[k - 1], &d))),
        ]));
    }
    let mut grid = Vec::new();
    for w in -1..7 {
        for n in [0, 1, 9, 200, facts::EXCD, facts::ENCD] {
            grid.push(arr![w, n, facts::unit_of(w, n).map(|s| s.to_string())]);
        }
    }
    obj([
        ("kind", "facts".into()),
        ("unit_of", J::Arr(grid)),
        (
            "cases",
            J::Arr(
                cases
                    .iter()
                    .zip(&got)
                    .map(|(c, f)| obj([("name", c["name"].as_str().unwrap().into()), ("facts", fact_obj(f))]))
                    .collect(),
            ),
        ),
        ("diffs", J::Arr(diffs)),
    ])
}

/// Every line of the full dump, in order.
pub fn lines(g: &Game, meta: &Value, fact_states: &Value) -> Vec<String> {
    let verbs = all_verbs(meta);
    let mut out = vec![tables().line(), game_line(g, meta).line()];
    out.extend(g.ids().map(|u| unit_line(g, u, &verbs).line()));
    out.push(facts_line(fact_states).line());
    out
}

/// Every line of the dump of units read without the raw blocks.
pub fn noblocks_lines(sc: &Script) -> Vec<String> {
    sc.ids().map(|u| unit_ir(sc, u).line()).collect()
}

/// A unit key from its JSON list (["global", 1], ["local", 1, 200], ...).
pub fn key_of(v: &Value) -> UnitKey {
    let a = v.as_array().expect("unit key");
    let n = |k: usize| a[k].as_i64().expect("unit key number") as i32;
    match a[0].as_str().expect("unit kind") {
        "global" => UnitKey::Global(n(1)),
        "local" => UnitKey::Local(n(1), n(2)),
        "entry" => UnitKey::Entry(n(1)),
        "exit" => UnitKey::Exit(n(1)),
        "obj" => UnitKey::Obj(n(1)),
        k => panic!("unit kind {k}"),
    }
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|k| u8::from_str_radix(&s[k..k + 2], 16).expect("hex")).collect()
}

/// dump_ir.py `synthetic_lines`, over scumm/ir/tests/synthetic.json.
pub fn synthetic_lines(cases: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let mut scratch = Script::new();
    for t in cases["statements"].as_array().expect("statements") {
        let text = t.as_str().expect("statement");
        let j = match crate::parse::parse_statement(text, &mut scratch.ast, &mut scratch.syms) {
            Ok(st) => obj([
                ("kind", "statement".into()),
                ("text", text.into()),
                ("st", enc_stmt(&scratch.ast, &scratch.syms, &st)),
                ("show", show_stmt(&scratch.ast, &scratch.syms, &st).into()),
                ("head", head(&scratch.ast, &scratch.syms, &st).into()),
            ]),
            Err(e) => obj([("kind", "statement".into()), ("text", text.into()), ("error", e.into())]),
        };
        out.push(j.line());
    }
    let mut sc = Script::new();
    let mut verbs: BTreeSet<i32> = [0, crate::read::DEFAULT_VERB].into_iter().collect();
    for c in cases["units"].as_array().expect("units") {
        let data = c.get("data").and_then(|d| d.as_str()).filter(|d| !d.is_empty()).map(unhex);
        let events: Option<Vec<(i32, i32)>> = c.get("verbs").and_then(|v| v.as_array()).map(|vs| {
            vs.iter()
                .map(|v| {
                    let verb = v["verb"].as_i64().unwrap() as i32;
                    (verb, i32::from_str_radix(v["offset"].as_str().unwrap(), 16).unwrap())
                })
                .collect()
        });
        verbs.extend(events.iter().flatten().map(|e| e.0));
        let room = c.get("room").and_then(|r| r.as_i64()).map(|r| r as i32);
        sc.add_unit(key_of(&c["key"]), room, "test.txt", "", c["text"].as_str().unwrap(), data.as_deref(), events.as_deref());
    }
    let gd = &cases["game"];
    let mut data = crate::game::GameData { first_local: gd["first_local"].as_i64().unwrap() as i32, ..Default::default() };
    for e in gd["local_rooms"].as_array().unwrap() {
        let mut rooms: Vec<i32> = e[1].as_array().unwrap().iter().map(|r| r.as_i64().unwrap() as i32).collect();
        rooms.sort_unstable();
        rooms.dedup();
        data.local_rooms.insert(e[0].as_i64().unwrap() as i32, rooms);
    }
    let g = Game::new(sc, data);
    let (resistance, wild) = g.compute_resistance();
    let mut resistance: Vec<(UnitKey, crate::game::Resist)> = resistance.into_iter().collect();
    resistance.sort_by_key(|r| r.0);
    let mut writes: Vec<&(i32, Vec<crate::analysis::Write>)> = g.script_writes.iter().collect();
    writes.sort_by_key(|w| w.0);
    let mut nonfacts: Vec<(i32, String)> = g.nonfacts.iter().map(|(k, v)| (*k, v.to_string())).collect();
    nonfacts.sort();
    out.push(
        obj([
            ("kind", "game".into()),
            ("units", J::Arr(g.units.iter().map(|u| key_j(u.key)).collect())),
            ("resistance", J::Arr(resistance.iter().map(|(k, r)| arr![key_j(*k), r.sorted()]).collect())),
            ("wild_starts", J::Arr(wild.iter().map(|(c, f)| arr![c.as_str(), *f]).collect())),
            ("nonfacts", J::Arr(nonfacts.into_iter().map(|(n, r)| arr![n, r]).collect())),
            (
                "script_writes",
                J::Arr(
                    writes.iter().map(|(n, ws)| arr![*n, J::Arr(ws.iter().map(|w| enc_write(&g, w)).collect())]).collect(),
                ),
            ),
            (
                "const_poll_loops",
                J::Arr(g.const_poll_loops.iter().map(|&(u, h)| arr![key_j(g.unit(u).key), h]).collect()),
            ),
        ])
        .line(),
    );
    let verbs: Vec<i32> = verbs.into_iter().collect();
    out.extend(g.ids().map(|u| unit_line(&g, u, &verbs).line()));
    out
}
