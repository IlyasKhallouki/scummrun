//! trace_check.py as strict.explain uses it: section 5 for one run the strict matcher left unexplained
//! (`explain_unexplained`), an `args:` precondition that fails (`args_cause`), and a slot fact's write sites
//! (`slot_write_sites`).

use crate::code::*;
use crate::pyj::{Py, J};
use crate::reading::{B1Eff, Loc, PlayReading};
use crate::strict::{holds_v, kv, logged, vals_j};
use crate::val::*;
use fmi_core::*;
use std::collections::BTreeMap;
use std::sync::Arc;

/// trace_check.slot_write_sites
pub fn slot_write_sites(code: &Code, key: UKey, fact: &str) -> Vec<u32> {
    let kind = fact.split(':').next().unwrap_or("");
    let mut out: Vec<u32> = if ["var", "bit", "state", "owner", "class", "verb", "room", "cutscene"].contains(&kind) {
        code.write_sites(key, fact, None, false)
    } else {
        vec![]
    };
    let ukey = code.game.script.unit(key).key;
    let own = unit_name(ukey) == fact.split_once(':').map(|x| x.1).unwrap_or("");
    if ["local", "script", "objscript"].contains(&kind) && own {
        out.extend(code.end_sites(key));
    }
    for (i, x) in code.ins(key).iter().enumerate() {
        let (is_start, opn) = match &x.st {
            Some(scumm_ir::ast::Stmt::Start { .. }) => (true, None),
            Some(scumm_ir::ast::Stmt::Op { name, .. }) => (false, Some(code.name(*name))),
            _ => (false, None),
        };
        if kind == "frozen" && (is_start || opn.is_some_and(|n| n.contains("reeze") || n.contains("utscene"))) {
            out.push(i as u32);
        } else if ["local", "script", "objscript"].contains(&kind)
            && (is_start || opn.is_some_and(|n| n.starts_with("stop")))
        {
            out.push(i as u32);
        }
    }
    out.sort();
    out.dedup();
    out
}

/// trace_check.args_cause
#[allow(clippy::too_many_arguments)]
pub fn args_cause(
    code: &Code,
    key: UKey,
    steps: &[StepX],
    run: &RunR,
    fk: Key,
    rd: &PlayReading,
    before: &View,
    dump: &Obs,
    k: u64,
) -> Vec<(String, J)> {
    let name = Unit::of_key(fk);
    let unit_j = || run.unit.map(|u| J::Str(u.text())).unwrap_or(J::Null);
    let (ev, _) = rd.events();
    let mut sigs: Vec<Val> = Vec::new();
    for &i in ev
        .get(&(run.unit.unwrap_or(Unit(0, 0)), run.code0))
        .map(|v| v.as_slice())
        .unwrap_or(&[])
    {
        if let Some(vs) = rd.b1(i).pre(fk) {
            sigs.extend(vs.iter().copied());
        }
    }
    let lists: Vec<&crate::reading::List> = rd
        .lists
        .get(&(name, run.code0))
        .map(|v| v.iter().filter(|l| sigs.contains(&l.sig)).collect())
        .unwrap_or_default();
    let slot = dump.slots.iter().find(|s| Unit::of(s.where_, s.number) == Some(name));
    let got = before.get(fk);
    let Some(slot) = slot.filter(|s| s.locals.is_some()) else {
        return vec![
            kv("cause", J::Null),
            kv("kind", "the slot's locals are not in the dump"),
            kv("unit", unit_j()),
            kv("frame", k),
        ];
    };
    let locals = slot.locals.as_ref().unwrap();
    // diffs: a Counter in insertion order (local numbers, then "override")
    let mut diffs: Vec<(Option<usize>, u64)> = Vec::new();
    let mut bump = |key: Option<usize>| match diffs.iter_mut().find(|d| d.0 == key) {
        Some(d) => d.1 += 1,
        None => diffs.push((key, 1)),
    };
    for l in &lists {
        for (n, &x) in locals.iter().enumerate() {
            match l.local(n as i64) {
                Loc::Unknown(Some(vs)) => {
                    if !vs.contains(&(x as i64)) {
                        bump(Some(n));
                    }
                }
                Loc::Unknown(None) => {}
                Loc::Num(v) => {
                    if v != x as i64 {
                        bump(Some(n));
                    }
                }
            }
        }
        if l.override_ != slot.cutscene_override as i64 {
            bump(None);
        }
    }
    let same: Vec<&Slot> = dump
        .slots
        .iter()
        .filter(|s| Unit::of(s.where_, s.number) == Some(name))
        .collect();
    if same.len() > 1 {
        let vals = vec![Value::new(
            format!("live slots of {}", name.text()),
            J::Int(same.len() as i64),
            "the dump: a recursive start's second instance; the reading keeps one slot per unit",
            "fact",
        )];
        return vec![
            kv("cause", true),
            kv("kind", "two instances of the unit"),
            kv("unit", unit_j()),
            kv("frame", k),
            kv("offset", run.code0),
            kv("opcode", J::Null),
            kv("values", vals_j(&vals)),
            kv("class", "semantics"),
        ];
    }
    let want = format!(
        "{{\"owned\": {}, \"recursive\": {}, \"resistant\": {}}}",
        if slot.where_ == WIO_INVENTORY { "true" } else { "false" },
        if slot.recursive == Some(true) { "true" } else { "null" },
        if slot.resistant == Some(true) { "true" } else { "false" }
    );
    let flags_txt = |l: &crate::reading::List| -> String {
        match &l.flags_j {
            None => "null".into(),
            Some(f) => {
                let mut o: Vec<(String, J)> = f.obj().to_vec();
                o.sort_by(|a, b| a.0.cmp(&b.0));
                crate::pyj::dumps(&J::Obj(o))
            }
        }
    };
    if diffs.is_empty() && !lists.is_empty() && slot.resistant.is_some() && lists.iter().all(|l| flags_txt(l) != want) {
        let mut lf: Vec<(String, u64)> = Vec::new();
        for l in &lists {
            let t = flags_txt(l);
            match lf.iter_mut().find(|x| x.0 == t) {
                Some(x) => x.1 += 1,
                None => lf.push((t, 1)),
            }
        }
        let mut order: Vec<usize> = (0..lf.len()).collect();
        order.sort_by_key(|&i| std::cmp::Reverse(lf[i].1));
        let top: Vec<Py> = order.iter().take(3).map(|&i| Py::Str(lf[i].0.clone())).collect();
        let vals = vec![Value::new(
            "slot flags",
            J::Str(want.clone()),
            format!(
                "the dump's (freezeResistant, recursive, owned); the records' lists hold {}",
                Py::List(top).repr()
            ),
            "fact",
        )];
        return vec![
            kv("cause", true),
            kv("kind", "the slot's flags are not the lists'"),
            kv("unit", unit_j()),
            kv("frame", k),
            kv("offset", run.code0),
            kv("opcode", J::Null),
            kv("values", vals_j(&vals)),
            kv("class", "semantics"),
        ];
    }
    if diffs.is_empty() {
        let why = if lists.is_empty() {
            "no list at that wait point has the records' numbers"
        } else {
            "the flags differ"
        };
        return vec![
            kv("cause", J::Null),
            kv("kind", format!("args: {why}")),
            kv("unit", unit_j()),
            kv("frame", k),
            kv("engine_args", got.j()),
        ];
    }
    // Counter.most_common(1): the greatest count, the first inserted among equals
    let loc = diffs
        .iter()
        .fold(None::<&(Option<usize>, u64)>, |b, d| {
            if b.is_none_or(|b| d.1 > b.1) {
                Some(d)
            } else {
                b
            }
        })
        .unwrap()
        .0;
    let Some(loc) = loc else {
        let vals = vec![Value::new(
            "cutscene override count",
            J::Int(slot.cutscene_override as i64),
            "the dump",
            "fact",
        )];
        return vec![
            kv("cause", true),
            kv("kind", "the slot's locals match no list the records ask for"),
            kv("unit", unit_j()),
            kv("frame", k),
            kv("offset", run.code0),
            kv("opcode", J::Null),
            kv("values", vals_j(&vals)),
            kv("class", "semantics"),
        ];
    };
    let vid = LOCAL_BIT | loc as u32;
    let n = steps.iter().position(|st| st.op.reads.iter().any(|r| r.0 == vid));
    let mut wanted: Vec<String> = lists
        .iter()
        .map(|l| match l.local(loc as i64) {
            Loc::Num(v) => v.to_string(),
            Loc::Unknown(_) => "unknown".into(),
        })
        .collect();
    wanted.sort();
    wanted.dedup();
    wanted.truncate(6);
    let vals = vec![Value::new(
        format!("local {loc}"),
        J::Int(locals[loc] as i64),
        format!(
            "the dumped slot's local; the records' lists hold {}",
            Py::List(wanted.into_iter().map(Py::Str).collect()).repr()
        ),
        "unknown",
    )];
    let x = n.and_then(|n| steps[n].i).map(|i| &code.ins(key)[i as usize]);
    vec![
        kv("cause", true),
        kv("kind", "the slot's locals match no list the records ask for"),
        kv("unit", unit_j()),
        kv("frame", k),
        kv("offset", n.map(|n| steps[n].op.code).unwrap_or(run.code0)),
        kv("opcode", x.map(|x| J::Str(code.head(x.st.as_ref()))).unwrap_or(J::Null)),
        kv("values", vals_j(&vals)),
        kv("class", "split"),
        kv(
            "note",
            if n.is_some() {
                J::Null
            } else {
                J::Str("no opcode of the run reads that local".into())
            },
        ),
    ]
}

/// trace_check.explain_unexplained
#[allow(clippy::too_many_arguments)]
pub fn explain_unexplained(
    code: &Code,
    ex: &Explained,
    k: u64,
    run: &RunR,
    why: &str,
    rd: &PlayReading,
    views: &BTreeMap<u64, Arc<View>>,
    dumps: &BTreeMap<u64, Arc<Obs>>,
) -> Vec<(String, J)> {
    let Some(key) = code.key(run.where_, run.number, run.room) else {
        return vec![kv("cause", J::Null), kv("kind", "unit not in the IR")];
    };
    let lg = ex.runs.values().find(|r| {
        r.frame == run.frame
            && r.after == run.after
            && r.code0 == run.code0
            && r.where_ == run.where_
            && r.number == run.number
    });
    let _ = logged;
    let Some(lg) = lg else {
        return vec![kv("cause", J::Null), kv("kind", "the run is not in the explain log")];
    };
    let steps = steps_of(code, key, &[lg]);
    let dump = dumps.get(&k.wrapping_sub(1)).map(|d| &**d);
    let (kind, fk) = why.split_once(' ').unwrap_or((why, ""));
    let empty = View::new();
    let before: &View = views.get(&k.wrapping_sub(1)).map(|v| &**v).unwrap_or(&empty);
    let unit_j = || run.unit.map(|u| J::Str(u.text())).unwrap_or(J::Null);
    let fkey = Key::parse(fk);
    let (ev, _) = rd.events();
    let evs: &[u32] = ev
        .get(&(run.unit.unwrap_or(Unit(0, 0)), run.code0))
        .map(|v| v.as_slice())
        .unwrap_or(&[]);
    if kind == "pre" && (fk.starts_with("args:") || fk.starts_with("pos:")) {
        if let Some(d) = dump {
            let u = Unit::parse(fk.split_once(':').map(|x| x.1).unwrap_or(""));
            let same = d
                .slots
                .iter()
                .filter(|s| Unit::of(s.where_, s.number) == u && u.is_some())
                .count();
            if same > 1 {
                let vals = vec![Value::new(
                    format!("live slots of {}", run.unit.map(|u| u.text()).unwrap_or_default()),
                    J::Int(same as i64),
                    "the dump: a recursive start's second instance; the reading keeps one slot per unit",
                    "fact",
                )];
                return vec![
                    kv("cause", true),
                    kv("kind", "two instances of the unit"),
                    kv("unit", unit_j()),
                    kv("frame", k),
                    kv("offset", run.code0),
                    kv("opcode", J::Null),
                    kv("values", vals_j(&vals)),
                    kv("class", "semantics"),
                ];
            }
        }
    }
    if kind == "pre" && fk.starts_with("args:") {
        let Some(d) = dump else {
            return vec![kv("cause", J::Null), kv("kind", "no dump")];
        };
        return args_cause(code, key, &steps, run, fkey, rd, before, d, k);
    }
    if kind == "pre" {
        let got = before.get(fkey);
        let mut want: Vec<String> = Vec::new();
        for &i in evs {
            if let Some(vs) = rd.b1(i).pre(fkey) {
                for &v in vs {
                    let t = vstr(v);
                    if !want.contains(&t) {
                        want.push(t);
                    }
                }
            }
        }
        want.sort();
        want.truncate(6);
        let vid = if let Some(n) = fk.strip_prefix("var:") {
            n.parse::<u32>().ok()
        } else if let Some(n) = fk.strip_prefix("bit:") {
            n.parse::<u32>().ok().map(|n| BIT_BIT | n)
        } else {
            None
        };
        let n = steps
            .iter()
            .position(|st| vid.is_some_and(|v| st.op.reads.iter().any(|r| r.0 == v)));
        let vals = vec![Value::new(
            fk,
            got.j(),
            format!(
                "the view at the decision point before; the records ask {}",
                Py::List(want.into_iter().map(Py::Str).collect()).repr()
            ),
            if fk.starts_with("args:") { "unknown" } else { "fact" },
        )];
        if let Some(n) = n {
            let x = &code.ins(key)[steps[n].i.unwrap() as usize];
            return vec![
                kv("cause", true),
                kv("kind", "the records' precondition fails"),
                kv("unit", unit_j()),
                kv("frame", k),
                kv("offset", steps[n].op.code),
                kv("opcode", code.head(x.st.as_ref())),
                kv("values", vals_j(&vals)),
                kv("class", cause_class(&vals)),
            ];
        }
        let x = steps.first().and_then(|s| s.i).map(|i| &code.ins(key)[i as usize]);
        return vec![
            kv("cause", true),
            kv("kind", "the records' precondition fails"),
            kv("unit", unit_j()),
            kv("frame", k),
            kv("offset", run.code0),
            kv("opcode", x.map(|x| J::Str(code.head(x.st.as_ref()))).unwrap_or(J::Null)),
            kv("values", vals_j(&vals)),
            kv("class", cause_class(&vals)),
            kv("note", "no opcode of the run reads it"),
        ];
    }
    if kind == "end" {
        let u = run.unit.unwrap_or(Unit(0, 0));
        let mut ends: Vec<i64> = Vec::new();
        for &i in evs {
            let rec = rd.b1(i);
            if rec.pre.iter().all(|(kk, vs)| holds_v(before.get(*kk), vs)) {
                let e = match rec.eff(u.pos()) {
                    Some(B1Eff::Set(v)) => *v,
                    _ => run.code0,
                };
                if !ends.contains(&e) {
                    ends.push(e);
                }
            }
        }
        let mut targets: Vec<u32> = Vec::new();
        for &e in &ends {
            targets.extend(if e == NOWHERE {
                code.end_sites(key)
            } else {
                code.yield_sites(key, e)
            });
        }
        targets.sort();
        targets.dedup();
        let n = first_off(code, key, &steps, &targets, false);
        let mut se = ends.clone();
        se.sort();
        let mut base = vec![
            kv("unit", unit_j()),
            kv("frame", k),
            kv("engine_end", run.code1),
            kv("record_ends", J::Arr(se.iter().map(|&e| J::Int(e)).collect())),
        ];
        if let Some(n) = n {
            let x = &code.ins(key)[steps[n].i.unwrap() as usize];
            if x.yields == Some(scumm_ir::ast::Yield::Wait) {
                let vals = wait_values(code, x, 0, dump);
                base.extend(vec![
                    kv("cause", true),
                    kv("kind", "the wait did not block"),
                    kv("offset", steps[n].op.code),
                    kv("opcode", code.head(x.st.as_ref())),
                    kv("values", vals_j(&vals)),
                    kv("class", "split"),
                ]);
                return base;
            }
            let vals = slice_test(code, key, &steps, n, dump);
            base.extend(vec![
                kv("cause", true),
                kv("kind", "left the records' path"),
                kv("offset", steps[n].op.code),
                kv("opcode", code.head(x.st.as_ref())),
                kv("values", vals_j(&vals)),
                kv("class", cause_class(&vals)),
            ]);
            return base;
        }
        if let Some(last) = steps.last() {
            if let Some(li) = last.i {
                let x = &code.ins(key)[li as usize];
                if x.yields == Some(scumm_ir::ast::Yield::Wait)
                    && run.code1 == last.op.code
                    && code.reachable(key, x.next, &targets, false)
                {
                    let vals = wait_values(code, x, 0, dumps.get(&k).map(|d| &**d));
                    base.extend(vec![
                        kv("cause", true),
                        kv("kind", "the wait blocked"),
                        kv("offset", last.op.code),
                        kv("opcode", code.head(x.st.as_ref())),
                        kv("values", vals_j(&vals)),
                        kv("class", "split"),
                    ]);
                    return base;
                }
            }
        }
        if ends.is_empty() {
            let mut bad: Vec<(Key, u64)> = Vec::new();
            for &i in evs {
                let rec = rd.b1(i);
                if let Some((kk, _)) = rec.pre.iter().find(|(kk, vs)| !holds_v(before.get(*kk), vs)) {
                    match bad.iter_mut().find(|b| b.0 == *kk) {
                        Some(b) => b.1 += 1,
                        None => bad.push((*kk, 1)),
                    }
                }
            }
            let top = bad.iter().fold(None::<&(Key, u64)>, |b, d| {
                if b.is_none_or(|b| d.1 > b.1) {
                    Some(d)
                } else {
                    b
                }
            });
            base.extend(vec![
                kv("cause", J::Null),
                kv("kind", "no record's precondition holds"),
                kv("most_failing", top.map(|t| J::Str(t.0.text())).unwrap_or(J::Null)),
                kv("engine", top.map(|t| before.get(t.0).j()).unwrap_or(J::Null)),
            ]);
            return base;
        }
        base.extend(vec![
            kv("cause", J::Null),
            kv("kind", "no branch off the records' path found"),
            kv("steps", steps.len()),
        ]);
        return base;
    }
    if kind == "effect" {
        let empty2 = View::new();
        let after: &View = views.get(&k).map(|v| &**v).unwrap_or(&empty2);
        let got = after.get(fkey);
        let vid = fk.strip_prefix("var:").and_then(|n| n.parse::<u32>().ok());
        let n = steps
            .iter()
            .position(|st| vid.is_some_and(|v| st.op.writes.iter().any(|w| w.0 == v)));
        if let Some(n) = n {
            let (vals, m) = write_values(code, key, &steps, n, fk, dump);
            let x = &code.ins(key)[steps[n].i.unwrap() as usize];
            return vec![
                kv("cause", true),
                kv("kind", "wrote another value"),
                kv("unit", unit_j()),
                kv("frame", k),
                kv("offset", steps[n].op.code),
                kv("opcode", code.head(x.st.as_ref())),
                kv("written", got.j()),
                kv("values", vals_j(&vals)),
                kv(
                    "class",
                    if vals.is_empty() {
                        "semantics"
                    } else {
                        cause_class(&vals)
                    },
                ),
                kv("test_offset", m.map(|m| J::Int(steps[m].op.code)).unwrap_or(J::Null)),
            ];
        }
        let targets = slot_write_sites(code, key, fk);
        let n = if targets.is_empty() {
            None
        } else {
            first_off(code, key, &steps, &targets, false)
        };
        if let Some(n) = n {
            let x = &code.ins(key)[steps[n].i.unwrap() as usize];
            let wait = x.yields == Some(scumm_ir::ast::Yield::Wait);
            let vals = if wait {
                wait_values(code, x, 0, dump)
            } else {
                slice_test(code, key, &steps, n, dump)
            };
            return vec![
                kv("cause", true),
                kv("kind", "left the path to the predicted write"),
                kv("unit", unit_j()),
                kv("frame", k),
                kv("offset", steps[n].op.code),
                kv("opcode", code.head(x.st.as_ref())),
                kv("values", vals_j(&vals)),
                kv("class", if wait { "split" } else { cause_class(&vals) }),
                kv("fact", fk),
            ];
        }
        return vec![
            kv("cause", J::Null),
            kv("kind", "the predicted write is not in the run"),
            kv("unit", unit_j()),
            kv("frame", k),
            kv("engine", got.j()),
            kv("sites", targets.len()),
        ];
    }
    vec![kv("cause", J::Null), kv("kind", format!("no rule for {why}"))]
}
