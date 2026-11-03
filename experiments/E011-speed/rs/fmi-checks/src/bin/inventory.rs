//! E014 (experiments/E014-fate-demo/plan.md, method (a)): the mechanic inventory of a game's scripts against what the
//! reader models, before any run on the game.
//!
//! usage: inventory --engine-src SCUMMVM_TREE --game NAME=SCRIPTS_DIR [--game NAME=SCRIPTS_DIR ...] --out DIR
//!
//! SCRIPTS_DIR is an extraction's `data/scripts-RELEASE` (index.json and the decompiled files); its raw blocks are
//! under the extraction's root, two folders up (`scumm_ir::data_root`'s rule). The scripts are loaded by scumm-ir as the
//! reader loads them. For every game it counts:
//!
//! - each opcode byte, named by ScummVM's v5 table (the `OPCODE(0x.., o5_...)` lines of the engine's script_v5.cpp);
//! - each statement head the IR parsed (ops and starts), with the machine's class for it (scumm-machine `heads::effect`:
//!   write, yield, wait, end, deferred, none, or unmodelled) and its sub-ops;
//! - each getter in an expression, and whether the machine evaluates it (the getters of `Machine::getter_on` and
//!   `getter_set`) or gives an unknown value;
//! - the statements the IR did not parse, and the IR's notes by class;
//! - the `systemOps` sites with their sub-op (1 restart, 2 pause, 3 quit).
//!
//! The engine side: every line of the engine's SCUMM sources that names GID_INDY4 (the branches ScummVM takes for this
//! game id), and the bridge's constants that name verb ids.
//!
//! Writes DIR/inventory.json and DIR/inventory.md. Nothing of a game's text is written: units are named by their keys,
//! statements by their engine names, notes by class.

use scumm_ir::{known, read, Expr, ExprId, Game, Stmt, Sym, UnitKey};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The getters `Machine::getter_on` and `getter_set` evaluate (scumm-machine machine.rs); any other gives an unknown
/// value (Reason::Work). getRandomNr is a draw (Reason::Random), counted apart.
const EVALUATED: [Sym; 9] = [
    known::GET_OBJECT_OWNER,
    known::GET_OBJECT_STATE,
    known::GET_ACTOR_ROOM,
    known::IS_SCRIPT_RUNNING,
    known::CLASS_OF_IS,
    known::GET_INVENTORY_COUNT,
    known::GET_VERB_ENTRY_POINT,
    known::FIND_OBJECT,
    known::ACTOR_FROM_POS,
];

#[derive(Default)]
struct Count {
    n: u64,
    units: BTreeSet<String>,
}

impl Count {
    fn add(&mut self, unit: &str) {
        self.n += 1;
        self.units.insert(unit.to_string());
    }
    fn json(&self) -> Value {
        json!({"n": self.n, "units": self.units.len()})
    }
}

#[derive(Default)]
struct Inv {
    units: BTreeMap<&'static str, u64>,
    instructions: u64,
    opcodes: BTreeMap<String, Count>,
    heads: BTreeMap<String, (String, Count)>,
    subops: BTreeMap<String, Count>,
    getters: BTreeMap<String, (bool, Count)>,
    unparsed: BTreeMap<String, Count>,
    notes: BTreeMap<String, Count>,
    system_ops: Vec<(String, i32, Option<i32>)>,
    rooms: Vec<i32>,
}

fn kind(k: UnitKey) -> &'static str {
    match k {
        UnitKey::Global(_) => "global",
        UnitKey::Entry(_) => "entry",
        UnitKey::Exit(_) => "exit",
        UnitKey::Local(..) => "local",
        UnitKey::Obj(_) => "object",
    }
}

fn effect_name(e: scumm_machine::heads::Effect) -> String {
    use scumm_machine::heads::Effect::*;
    match e {
        Write => "write".into(),
        Yield => "yield".into(),
        Wait => "wait".into(),
        End => "end".into(),
        None => "none".into(),
        Deferred => "deferred".into(),
        Unmodelled(why) => format!("unmodelled ({why})"),
    }
}

/// ScummVM's v5 opcode table: byte -> handler, from the engine's script_v5.cpp.
fn opcode_table(src: &Path) -> BTreeMap<u8, String> {
    let p = src.join("engines/scumm/script_v5.cpp");
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    let mut t = BTreeMap::new();
    for line in text.lines() {
        let l = line.trim();
        if let Some(rest) = l.strip_prefix("OPCODE(0x") {
            let (hex, tail) = rest.split_once(',').expect("OPCODE(0x.., name)");
            let name = tail.trim().trim_end_matches(");").trim().to_string();
            t.insert(u8::from_str_radix(hex, 16).expect("hex"), name);
        }
    }
    assert!(t.len() > 200, "{}: {} OPCODE lines", p.display(), t.len());
    t
}

fn walk(g: &Game, e: ExprId, unit: &str, inv: &mut Inv) {
    let ast = &g.ast;
    match ast.expr(e) {
        Expr::Getter { name, args } => {
            let ev = EVALUATED.contains(&name);
            let s = g.syms.str(name).to_string();
            let c = inv.getters.entry(s).or_insert_with(|| (ev, Count::default()));
            c.1.add(unit);
            for &k in ast.kids(args) {
                walk(g, k, unit, inv);
            }
        }
        Expr::Index { offset, .. } => walk(g, offset, unit, inv),
        Expr::BinOp { a, b, .. } | Expr::Cmp { a, b, .. } => {
            walk(g, a, unit, inv);
            walk(g, b, unit, inv);
        }
        Expr::Neg(a) | Expr::Not(a) | Expr::Truth(a) => walk(g, a, unit, inv),
        Expr::Embed(a) => {
            walk(g, a.lhs, unit, inv);
            walk(g, a.rhs, unit, inv);
        }
        Expr::Concat(s) | Expr::List(s) => {
            for &k in ast.kids(s) {
                walk(g, k, unit, inv);
            }
        }
        _ => {}
    }
}

fn inventory(scripts: &Path, table: &BTreeMap<u8, String>) -> Inv {
    let meta: Value = serde_json::from_slice(&std::fs::read(scripts.join("index.json")).expect("index.json")).expect("index.json");
    let data = scripts.parent().and_then(|p| p.parent()).expect("the extraction's root").to_path_buf();
    let script = read::load_with(scripts, Some(&data), &meta).expect("the units");
    let g = Game::from_script(script, &data, &meta).expect("the game");
    let mut inv = Inv::default();
    inv.rooms = meta["rooms"].as_object().map(|m| m.keys().filter_map(|k| k.parse().ok()).collect()).unwrap_or_default();
    inv.rooms.sort_unstable();
    for (i, u) in g.units.iter().enumerate() {
        let uid = scumm_ir::UnitId(i as u32);
        let name = u.key.to_string();
        *inv.units.entry(kind(u.key)).or_default() += 1;
        for n in &u.notes {
            inv.notes.entry(n.class.to_string()).or_default().add(&name);
        }
        for x in g.ins(uid) {
            inv.instructions += 1;
            let op = table.get(&x.op).cloned().unwrap_or_else(|| format!("0x{:02X} (not in the table)", x.op));
            inv.opcodes.entry(op.clone()).or_default().add(&name);
            let Some(st) = x.st else {
                inv.unparsed.entry(op).or_default().add(&name);
                continue;
            };
            match st {
                Stmt::Op { name: h, args, subops, .. } => {
                    let hs = g.syms.str(h).to_string();
                    let class = effect_name(scumm_machine::heads::effect(h));
                    inv.heads.entry(hs.clone()).or_insert_with(|| (class, Count::default())).1.add(&name);
                    for s in g.ast.subs(subops) {
                        inv.subops.entry(format!("{hs}.{}", g.syms.str(s.name))).or_default().add(&name);
                        for &k in g.ast.kids(s.args) {
                            walk(&g, k, &name, &mut inv);
                        }
                    }
                    let kids = g.ast.kids(args).to_vec();
                    if h == known::SYSTEM_OPS {
                        let sub = kids.first().and_then(|&k| match g.ast.expr(k) {
                            Expr::Const(c) => Some(c),
                            _ => None,
                        });
                        inv.system_ops.push((name.clone(), x.offset, sub));
                    }
                    for k in kids {
                        walk(&g, k, &name, &mut inv);
                    }
                }
                Stmt::Start { name: h, script, obj, args, .. } => {
                    let hs = g.syms.str(h).to_string();
                    let class = effect_name(scumm_machine::heads::effect(h));
                    inv.heads.entry(hs).or_insert_with(|| (class, Count::default())).1.add(&name);
                    walk(&g, script, &name, &mut inv);
                    if let Some(o) = obj {
                        walk(&g, o, &name, &mut inv);
                    }
                    for &k in g.ast.kids(args) {
                        walk(&g, k, &name, &mut inv);
                    }
                }
                Stmt::Assign(a) => {
                    inv.heads.entry("(assignment)".into()).or_insert_with(|| ("write".into(), Count::default())).1.add(&name);
                    walk(&g, a.lhs, &name, &mut inv);
                    walk(&g, a.rhs, &name, &mut inv);
                }
                Stmt::Jump { cond, .. } => {
                    inv.heads.entry("(jump)".into()).or_insert_with(|| ("control".into(), Count::default())).1.add(&name);
                    if let Some(c) = cond {
                        walk(&g, c, &name, &mut inv);
                    }
                }
            }
        }
    }
    inv
}

/// Lines of the engine's SCUMM sources (outside the bridge) that name GID_INDY4; the bridge's lines that name verb ids.
fn engine_side(src: &Path) -> (Vec<(String, usize, String)>, Vec<(String, usize, String)>) {
    let dir = src.join("engines/scumm");
    let mut files = Vec::new();
    let mut stack = vec![dir.clone()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "cpp" || x == "h") {
                files.push(p);
            }
        }
    }
    files.sort();
    let (mut game, mut bridge) = (Vec::new(), Vec::new());
    for f in files {
        let rel = f.strip_prefix(src).unwrap().display().to_string();
        let Ok(text) = std::fs::read_to_string(&f) else { continue };
        let in_bridge = rel.contains("/speedrun/");
        for (i, l) in text.lines().enumerate() {
            if !in_bridge && l.contains("GID_INDY4") {
                game.push((rel.clone(), i + 1, l.trim().chars().take(160).collect()));
            }
            if in_bridge && (l.contains("kActionVerb") || l.contains("kDialogVerb")) {
                bridge.push((rel.clone(), i + 1, l.trim().chars().take(160).collect()));
            }
        }
    }
    (game, bridge)
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let (mut src, mut out, mut games) = (None, None, Vec::new());
    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "--engine-src" => src = argv.get(i + 1).map(PathBuf::from),
            "--out" => out = argv.get(i + 1).map(PathBuf::from),
            "--game" => {
                let (n, d) = argv[i + 1].split_once('=').expect("--game NAME=SCRIPTS_DIR");
                games.push((n.to_string(), PathBuf::from(d)));
            }
            a => panic!("unknown argument {a}"),
        }
        i += 2;
    }
    let (src, out) = (src.expect("--engine-src"), out.expect("--out"));
    std::fs::create_dir_all(&out).expect("--out");
    let table = opcode_table(&src);
    let invs: Vec<(String, Inv)> = games.iter().map(|(n, d)| (n.clone(), inventory(d, &table))).collect();
    let (game_lines, bridge_lines) = engine_side(&src);

    // JSON
    let mut j = Map::new();
    for (n, inv) in &invs {
        let m = |c: &BTreeMap<String, Count>| -> Value { c.iter().map(|(k, v)| (k.clone(), v.json())).collect::<Map<_, _>>().into() };
        j.insert(
            n.clone(),
            json!({
                "rooms": inv.rooms,
                "units": inv.units,
                "instructions": inv.instructions,
                "opcodes": m(&inv.opcodes),
                "heads": inv.heads.iter().map(|(k, (c, v))| (k.clone(), json!({"class": c, "n": v.n, "units": v.units.len()}))).collect::<Map<_, _>>(),
                "subops": m(&inv.subops),
                "getters": inv.getters.iter().map(|(k, (e, v))| (k.clone(), json!({"evaluated": e, "n": v.n, "units": v.units.len()}))).collect::<Map<_, _>>(),
                "unparsed": m(&inv.unparsed),
                "notes": m(&inv.notes),
                "system_ops": inv.system_ops.iter().map(|(u, o, s)| json!({"unit": u, "offset": format!("{o:04X}"), "subop": s})).collect::<Vec<_>>(),
            }),
        );
    }
    j.insert("engine".into(), json!({
        "gid_indy4": game_lines.iter().map(|(f, l, t)| json!([f, l, t])).collect::<Vec<_>>(),
        "bridge_verb_ids": bridge_lines.iter().map(|(f, l, t)| json!([f, l, t])).collect::<Vec<_>>(),
    }));
    std::fs::write(out.join("inventory.json"), serde_json::to_string_pretty(&Value::Object(j)).unwrap() + "\n").expect("write");

    // Markdown: each game's column side by side, the last game against the others
    let names: Vec<&str> = invs.iter().map(|(n, _)| n.as_str()).collect();
    let mut md = String::new();
    md.push_str("# Mechanic inventory\n\nWritten by `fmi-checks`' `inventory` (E014, plan.md method (a)). Counts are statements (units in brackets).\n\n");
    md.push_str(&format!("| | {} |\n|---|{}\n", names.join(" | "), "---:|".repeat(names.len())));
    md.push_str(&format!("| rooms | {} |\n", invs.iter().map(|(_, v)| v.rooms.len().to_string()).collect::<Vec<_>>().join(" | ")));
    for k in ["global", "entry", "exit", "local", "object"] {
        md.push_str(&format!("| {k} units | {} |\n", invs.iter().map(|(_, v)| v.units.get(k).copied().unwrap_or(0).to_string()).collect::<Vec<_>>().join(" | ")));
    }
    md.push_str(&format!("| instructions | {} |\n\n", invs.iter().map(|(_, v)| v.instructions.to_string()).collect::<Vec<_>>().join(" | ")));
    let cell = |c: Option<&Count>| c.map(|c| format!("{} ({})", c.n, c.units.len())).unwrap_or_else(|| "-".into());
    let last = invs.len() - 1;
    let new_in_last = |keys: &mut dyn Iterator<Item = (String, bool)>| -> Vec<String> { keys.filter(|(_, b)| *b).map(|(k, _)| k).collect() };

    md.push_str("## Opcodes (ScummVM's v5 handler names)\n\n");
    md.push_str(&format!("| handler | {} | only in {} |\n|---|{}---|\n", names.join(" | "), names[last], "---:|".repeat(names.len())));
    let all_ops: BTreeSet<&String> = invs.iter().flat_map(|(_, v)| v.opcodes.keys()).collect();
    for k in &all_ops {
        let only = invs[..last].iter().all(|(_, v)| !v.opcodes.contains_key(*k)) && invs[last].1.opcodes.contains_key(*k);
        md.push_str(&format!("| {k} | {} | {} |\n", invs.iter().map(|(_, v)| cell(v.opcodes.get(*k))).collect::<Vec<_>>().join(" | "), if only { "yes" } else { "" }));
    }
    md.push_str("\n## Statement heads and the machine's class (scumm-machine heads.rs)\n\n");
    md.push_str(&format!("| head | class | {} | only in {} |\n|---|---|{}---|\n", names.join(" | "), names[last], "---:|".repeat(names.len())));
    let all_heads: BTreeMap<&String, &String> = invs.iter().flat_map(|(_, v)| v.heads.iter().map(|(k, (c, _))| (k, c))).collect();
    for (k, c) in &all_heads {
        let only = invs[..last].iter().all(|(_, v)| !v.heads.contains_key(*k)) && invs[last].1.heads.contains_key(*k);
        md.push_str(&format!("| {k} | {c} | {} | {} |\n", invs.iter().map(|(_, v)| cell(v.heads.get(*k).map(|x| &x.1))).collect::<Vec<_>>().join(" | "), if only { "yes" } else { "" }));
    }
    md.push_str("\n## Sub-ops\n\n");
    md.push_str(&format!("| op.sub-op | {} | only in {} |\n|---|{}---|\n", names.join(" | "), names[last], "---:|".repeat(names.len())));
    let all_subs: BTreeSet<&String> = invs.iter().flat_map(|(_, v)| v.subops.keys()).collect();
    for k in &all_subs {
        let only = invs[..last].iter().all(|(_, v)| !v.subops.contains_key(*k)) && invs[last].1.subops.contains_key(*k);
        md.push_str(&format!("| {k} | {} | {} |\n", invs.iter().map(|(_, v)| cell(v.subops.get(*k))).collect::<Vec<_>>().join(" | "), if only { "yes" } else { "" }));
    }
    md.push_str("\n## Getters (evaluated by the machine, or an unknown value)\n\n");
    md.push_str(&format!("| getter | evaluated | {} | only in {} |\n|---|---|{}---|\n", names.join(" | "), names[last], "---:|".repeat(names.len())));
    let all_get: BTreeMap<&String, bool> = invs.iter().flat_map(|(_, v)| v.getters.iter().map(|(k, (e, _))| (k, *e))).collect();
    for (k, e) in &all_get {
        let only = invs[..last].iter().all(|(_, v)| !v.getters.contains_key(*k)) && invs[last].1.getters.contains_key(*k);
        md.push_str(&format!("| {k} | {} | {} | {} |\n", if *e { "yes" } else { "no" }, invs.iter().map(|(_, v)| cell(v.getters.get(*k).map(|x| &x.1))).collect::<Vec<_>>().join(" | "), if only { "yes" } else { "" }));
    }
    md.push_str("\n## Statements the IR did not parse (by opcode)\n\n");
    md.push_str(&format!("| handler | {} |\n|---|{}\n", names.join(" | "), "---:|".repeat(names.len())));
    let all_unp: BTreeSet<&String> = invs.iter().flat_map(|(_, v)| v.unparsed.keys()).collect();
    for k in &all_unp {
        md.push_str(&format!("| {k} | {} |\n", invs.iter().map(|(_, v)| cell(v.unparsed.get(*k))).collect::<Vec<_>>().join(" | ")));
    }
    if all_unp.is_empty() {
        md.push_str("| none | |\n");
    }
    md.push_str("\n## The IR's notes (by class)\n\n");
    md.push_str(&format!("| class | {} |\n|---|{}\n", names.join(" | "), "---:|".repeat(names.len())));
    let all_notes: BTreeSet<&String> = invs.iter().flat_map(|(_, v)| v.notes.keys()).collect();
    for k in &all_notes {
        md.push_str(&format!("| {k} | {} |\n", invs.iter().map(|(_, v)| cell(v.notes.get(*k))).collect::<Vec<_>>().join(" | ")));
    }
    md.push_str("\n## systemOps sites (1 restart, 2 pause, 3 quit)\n\n| game | unit | offset | sub-op |\n|---|---|---|---:|\n");
    for (n, inv) in &invs {
        for (u, o, s) in &inv.system_ops {
            md.push_str(&format!("| {n} | {u} | {o:04X} | {} |\n", s.map(|s| s.to_string()).unwrap_or_else(|| "not a constant".into())));
        }
    }
    let _ = new_in_last;
    md.push_str(&format!("\n## The engine: lines of engines/scumm outside the bridge that name GID_INDY4 ({})\n\n| file | line | text |\n|---|---:|---|\n", game_lines.len()));
    for (f, l, t) in &game_lines {
        md.push_str(&format!("| {f} | {l} | `{}` |\n", t.replace('|', "\\|").replace('`', "'")));
    }
    md.push_str(&format!("\n## The bridge: lines that use its verb id constants ({})\n\n| file | line | text |\n|---|---:|---|\n", bridge_lines.len()));
    for (f, l, t) in &bridge_lines {
        md.push_str(&format!("| {f} | {l} | `{}` |\n", t.replace('|', "\\|").replace('`', "'")));
    }
    std::fs::write(out.join("inventory.md"), md).expect("write");
    println!("wrote {} and {}", out.join("inventory.json").display(), out.join("inventory.md").display());
}
