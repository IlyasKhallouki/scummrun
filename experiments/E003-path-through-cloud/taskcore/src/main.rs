//! taskcore: the planner's task (E003 step 4) as flat integer arrays, the fork fold, exact reductions, and Fast
//! Downward's SAS file. See README.md.
//!
//! usage:
//!   taskcore sas    --task F.tc [--start NAME | --start-file F] [--bans F] [--fold strict|every|off]
//!                   [--reduce all|none|a,b,..] [--product-limit N] [--encoding model|task] --out F.sas
//!                   [--stats F.json] [--check-fold N] [--ops F.jsonl]
//!   taskcore decode (the same options) --plan sas_plan --out steps.json
//!   taskcore validate (the same options) --names plan.txt --out result.json
//!   taskcore info   --task F.tc

mod bans;
mod fold;
mod model;
mod packed;
mod reduce;
mod sas;

use fold::FoldMode;
use model::*;
use reduce::Enabled;
use std::fmt::Write as _;
use std::io::BufWriter;
use std::path::PathBuf;
use std::time::Instant;

struct Args {
    names: Option<PathBuf>,
    cmd: String,
    task: PathBuf,
    start: Option<String>,
    fold: FoldMode,
    reduce: Enabled,
    limit: u128,
    out: Option<PathBuf>,
    stats: Option<PathBuf>,
    plan: Option<PathBuf>,
    check_fold: usize,
    kept: Option<PathBuf>,
    model: bool,
    start_file: Option<PathBuf>,
    bans: Option<PathBuf>,
    ops: Option<PathBuf>,
}

fn parse() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let cmd = it.next().ok_or("usage: taskcore sas|decode|info --task F.tc ...")?;
    let mut a = Args {
        names: None,
        cmd,
        task: PathBuf::new(),
        start: None,
        fold: FoldMode::Strict,
        reduce: Enabled::parse("all")?,
        limit: 64,
        out: None,
        stats: None,
        plan: None,
        check_fold: 0,
        kept: None,
        model: true,
        start_file: None,
        bans: None,
        ops: None,
    };
    while let Some(k) = it.next() {
        let v = it.next().ok_or(format!("{} needs a value", k))?;
        match k.as_str() {
            "--task" => a.task = v.into(),
            "--start" => a.start = Some(v),
            "--fold" => {
                a.fold = match v.as_str() {
                    "strict" => FoldMode::Strict,
                    "every" => FoldMode::Every,
                    "off" | "none" => FoldMode::Off,
                    _ => return Err(format!("--fold {}: strict, every or off", v)),
                }
            }
            "--reduce" => a.reduce = Enabled::parse(&v)?,
            "--product-limit" => a.limit = v.parse().map_err(|e| format!("--product-limit: {}", e))?,
            "--out" => a.out = Some(v.into()),
            "--stats" => a.stats = Some(v.into()),
            "--plan" => a.plan = Some(v.into()),
            "--check-fold" => a.check_fold = v.parse().map_err(|e| format!("--check-fold: {}", e))?,
            "--kept" => a.kept = Some(v.into()),
            "--names" => a.names = Some(v.into()),
            "--start-file" => a.start_file = Some(v.into()),
            "--bans" => a.bans = Some(v.into()),
            "--ops" => a.ops = Some(v.into()),
            "--encoding" => {
                a.model = match v.as_str() {
                    "model" => true,
                    "task" => false,
                    _ => return Err(format!("--encoding {}: model or task", v)),
                }
            }
            _ => return Err(format!("unknown option {}", k)),
        }
    }
    if a.start.is_some() && a.start_file.is_some() {
        return Err("--start and --start-file: give one".into());
    }
    Ok(a)
}

/// A tiny JSON writer for the stats.
struct J(String);

impl J {
    fn new() -> J {
        J("{".into())
    }
    fn sep(&mut self) {
        if !self.0.ends_with('{') {
            self.0.push_str(", ");
        }
    }
    fn n(&mut self, k: &str, v: impl std::fmt::Display) -> &mut Self {
        self.sep();
        let _ = write!(self.0, "{}: {}", esc(k), v);
        self
    }
    fn f(&mut self, k: &str, v: f64) -> &mut Self {
        self.sep();
        let _ = write!(self.0, "{}: {:.4}", esc(k), v);
        self
    }
    fn s(&mut self, k: &str, v: &str) -> &mut Self {
        self.sep();
        let _ = write!(self.0, "{}: {}", esc(k), esc(v));
        self
    }
    fn raw(&mut self, k: &str, v: &str) -> &mut Self {
        self.sep();
        let _ = write!(self.0, "{}: {}", esc(k), v);
        self
    }
    fn done(&mut self) -> String {
        self.0.push('}');
        std::mem::take(&mut self.0)
    }
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for ch in s.chars() {
        match ch {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            c if (c as u32) < 0x20 => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn counts_json(c: &reduce::Counts) -> String {
    let mut j = J::new();
    for (k, v) in c {
        j.n(k, v);
    }
    j.done()
}

fn sas_json(c: &sas::SasCounts) -> String {
    let mut j = J::new();
    j.n("actions", c.actions)
        .n("operators", c.operators)
        .n("action_operators", c.action_ops)
        .n("pick_operators", c.pick_ops)
        .n("goal_operators", c.goal_ops)
        .n("effects", c.effects)
        .n("conditional_effects", c.cond_effects)
        .n("variables", c.variables)
        .n("values", c.values)
        .n("pick_variables", c.pick_vars)
        .n("variants", c.variants)
        .n("max_variants", c.max_variants)
        .n("dead_actions", c.dead_actions)
        .n("empty_variants", c.empty_variants)
        .n("pick_actions", c.pick_actions)
        .n("noop_effects", c.noop_effects)
        .n("goal_resets", c.goal_resets)
        .n("bytes", c.bytes);
    j.done()
}

struct Prepared {
    names: Vec<String>, // the input actions' names (label#rec), by input index
    task: Task,
    start: usize,
    stats: J,
    replaced: Vec<(u32, u32)>,
}

fn prepare(a: &Args) -> Result<Prepared, String> {
    let t0 = Instant::now();
    let loaded = packed::load(&a.task)?;
    let mut t = loaded.task;
    let names: Vec<String> = if a.names.is_some() { (0..t.acts.len()).map(|i| t.name(i)).collect() } else { Vec::new() };
    let mut st = J::new();
    st.s("task", &a.task.display().to_string())
        .n("file_bytes", loaded.file_bytes)
        .n("input_actions", t.acts.len())
        .n("input_effects", t.acts.n_effects())
        .n("facts", t.facts.len())
        .n("add_effects_expanded", loaded.add_effects)
        .f("seconds_load", t0.elapsed().as_secs_f64())
        .n("rss_after_load", peak_rss());
    let mut replaced = Vec::new();
    // a start given as a file, and the bans (bans.rs): before the start check, so that it and everything after it
    // read the banned task as their input
    let mut named = None;
    if let Some(path) = &a.start_file {
        let (vals, keys) = bans::read_start(&t, path)?;
        let name = t.strs.intern(&format!("file:{}", path.display()));
        t.starts.push((name, vals));
        named = Some(keys);
    }
    if let Some(path) = &a.bans {
        let t1 = Instant::now();
        let list = bans::read_bans(path)?;
        let mut bc = bans::BanCounts::default();
        t = bans::apply(t, &list, named.as_ref(), &mut bc)?;
        let mut j = J::new();
        j.n("bans", bc.bans)
            .n("not_applied", bc.not_applied)
            .n("actions_hit", bc.actions_hit)
            .n("pieces", bc.pieces)
            .n("actions_after", bc.actions_after)
            .f("seconds", t1.elapsed().as_secs_f64());
        st.raw("bans", &j.done());
    }
    let start = match &a.start {
        Some(name) => t.start_index(name).ok_or(format!("no start named {}", name))?,
        None if a.start_file.is_some() => t.starts.len() - 1,
        None => 0,
    };
    st.s("start", t.strs.get(t.starts.get(start).ok_or("the file has no start")?.0));
    // a start value outside a domain the input's file uses: task.py raises OutOfDomain; so does taskcore, before the
    // fold and the reductions can hide the fact (sas::start_misses)
    let misses = sas::start_misses(&t, &t.starts[start].1, a.limit)?;
    if !misses.is_empty() {
        return Err(format!("{} facts out of their domains: {:?}", misses.len(), &misses[..misses.len().min(10)]));
    }
    st.s("fold", &format!("{:?}", a.fold)).s("reduce", &format!("{:?}", a.reduce)).n("product_limit", a.limit);
    st.s("encoding", if a.model { "model" } else { "task" });
    // the fold, then the reductions to their fixpoint; then the fold again (after (d) has cut unreachable values,
    // groups with a hole may cover their box, and overlapping paths may be disjoint), and the reductions again
    let passes = if a.fold != FoldMode::Off && a.reduce.any() { 2 } else { 1 };
    let mut c = reduce::Counts::new();
    for pass in 0..passes {
        let tag = if pass == 0 { "" } else { "_second" };
        if a.fold != FoldMode::Off {
            let t1 = Instant::now();
            let keep = if a.check_fold > 0 { Some(t.clone()) } else { None };
            let (nt, fc, map) = fold::fold(t, a.fold);
            t = nt;
            st.f(&format!("seconds_fold{}", tag), t1.elapsed().as_secs_f64());
            let mut j = J::new();
            j.n("groups", fc.groups)
                .n("paths_in_groups", fc.grouped)
                .n("subgroups", fc.subgroups)
                .n("paths_folded", fc.folded_paths)
                .n("folded_actions", fc.folded_actions)
                .n("subgroups_with_a_hole_left_unfolded", fc.holes)
                .n("subgroups_past_128_bits_left_unfolded", fc.overflow)
                .n("subgroups_past_pre_limit_left_unfolded", fc.pre_limit)
                .n("dead_conditional_effects", fc.dead_effects)
                .n("actions_after", t.acts.len())
                .n("effects_after", t.acts.n_effects());
            st.raw(&format!("fold_counts{}", tag), &j.done());
            if let Some(orig) = keep {
                let t2 = Instant::now();
                let cc = fold::check(&orig, &t, &map, a.check_fold, 0x5eed);
                let mut j = J::new();
                j.n("folds", cc.folds)
                    .n("paths", cc.paths)
                    .n("states", cc.states)
                    .n("folded_does_not_apply", cc.folded_does_not_apply)
                    .n("another_path_applies", cc.another_path_applies)
                    .n("writes_differ", cc.writes_differ)
                    .n("conflicting_writes", cc.conflicting_writes)
                    .n("union_box_states", cc.union_box_states)
                    .n("union_box_not_exactly_one_path", cc.union_box_not_one_path)
                    .n("mismatches", cc.mismatches())
                    .f("seconds", t2.elapsed().as_secs_f64());
                st.raw(&format!("fold_check{}", tag), &j.done());
            }
            if pass == 1 && fc.folded_actions == 0 {
                break;
            }
        }
        if a.reduce.any() {
            let t1 = Instant::now();
            t = reduce::run(t, a.reduce, start, &mut c, &mut replaced)?;
            st.f(&format!("seconds_reduce{}", tag), t1.elapsed().as_secs_f64());
            st.n(&format!("actions_after_reduce{}", tag), t.acts.len());
        }
    }
    if a.reduce.any() {
        st.raw("reduce_counts", &counts_json(&c));
        let sample: Vec<String> = replaced.iter().take(20).map(|(i, k)| format!("[{}, {}]", i, k)).collect();
        st.raw("replaced_sample", &format!("[{}]", sample.join(", ")));
        st.n("replaced", replaced.len());
    }
    st.n("actions", t.acts.len()).n("effects", t.acts.n_effects()).n("rss_after_reduce", peak_rss());
    Ok(Prepared { names, task: t, start, stats: st, replaced })
}

fn run() -> Result<(), String> {
    let a = parse()?;
    let t0 = Instant::now();
    match a.cmd.as_str() {
        "info" => {
            let l = packed::load(&a.task)?;
            let t = &l.task;
            println!(
                "{}: {} actions, {} effects, {} facts, {} values, goal {} facts, starts {:?}",
                a.task.display(),
                t.acts.len(),
                t.acts.n_effects(),
                t.facts.len(),
                t.facts.total_values(),
                t.goal.len(),
                t.starts.iter().map(|(n, _)| t.strs.get(*n)).collect::<Vec<_>>()
            );
            Ok(())
        }
        "sas" => {
            let mut p = prepare(&a)?;
            let out = a.out.clone().ok_or("--out is needed")?;
            let t1 = Instant::now();
            let mut x = sas::Expander::new(&p.task, a.limit);
            x.model = a.model;
            let plan = sas::pass1(&mut x)?;
            let s1 = t1.elapsed().as_secs_f64();
            let t2 = Instant::now();
            let f = std::fs::File::create(&out).map_err(|e| format!("{}: {}", out.display(), e))?;
            let bytes = sas::pass2(&mut x, &plan, &p.task.starts[p.start].1, BufWriter::with_capacity(4 << 20, f))?;
            let s2 = t2.elapsed().as_secs_f64();
            if let Some(path) = &a.ops {
                let t3 = Instant::now();
                let f = std::fs::File::create(path).map_err(|e| format!("{}: {}", path.display(), e))?;
                sas::write_ops(&mut x, &plan, BufWriter::with_capacity(1 << 20, f))?;
                p.stats.f("seconds_ops", t3.elapsed().as_secs_f64());
            }
            let mut c = plan.counts.clone();
            c.bytes = bytes;
            p.stats
                .f("seconds_pass1", s1)
                .f("seconds_pass2", s2)
                .f("seconds_total", t0.elapsed().as_secs_f64())
                .raw("sas", &sas_json(&c))
                .n("peak_rss", peak_rss());
            if let Some(path) = &a.kept {
                // the input actions each reduced action stands for, and the ones (f) replaced: (dropped, by)
                let mut o = String::from("{\"actions\": [");
                for i in 0..p.task.acts.len() {
                    if i > 0 {
                        o.push_str(", ");
                    }
                    let m: Vec<String> = p.task.acts.members(i).iter().map(|x| x.to_string()).collect();
                    let _ = write!(o, "[{}]", m.join(", "));
                }
                o.push_str("], \"replaced\": [");
                let r: Vec<String> = p.replaced.iter().map(|(x, y)| format!("[{}, {}]", x, y)).collect();
                o.push_str(&r.join(", "));
                o.push_str("]}\n");
                std::fs::write(path, o).map_err(|e| e.to_string())?;
            }
            let js = p.stats.done();
            match &a.stats {
                Some(path) => std::fs::write(path, js + "\n").map_err(|e| e.to_string())?,
                None => println!("{}", js),
            }
            Ok(())
        }
        "decode" => {
            let p = prepare(&a)?;
            let plan_text = std::fs::read_to_string(a.plan.as_ref().ok_or("--plan is needed")?).map_err(|e| e.to_string())?;
            let mut ops = Vec::new();
            for line in plan_text.lines() {
                let l = line.trim();
                if let Some(rest) = l.strip_prefix('(') {
                    let name = rest.trim_end_matches(')').split_whitespace().next().unwrap_or("");
                    let num: u64 = name.trim_start_matches('a').parse().map_err(|_| format!("plan line {:?}", l))?;
                    ops.push(num);
                }
            }
            let t = &p.task;
            let mut x = sas::Expander::new(t, a.limit);
            x.model = a.model;
            let plan = sas::pass1(&mut x)?;
            let mut x = sas::Expander::new(t, a.limit);
            x.model = a.model;
            let steps = sas::decode(&mut x, &ops)?;
            let key = |f: u32| esc(t.key(f));
            let val = |f: u32, v: u32| esc(t.strs.get(t.facts.dom(f)[v as usize]));
            let mut out = String::from("{\"steps\": [");
            for (i, s) in steps.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                let mem: Vec<String> = t.acts.members(s.action).iter().map(|m| m.to_string()).collect();
                let ch: Vec<String> = s.chosen.iter().map(|&(f, v)| format!("[{}, {}]", key(f), val(f, v))).collect();
                let gu: Vec<String> = s.guessed.iter().map(|&(f, v)| format!("[{}, {}]", key(f), val(f, v))).collect();
                let pk: Vec<String> = s
                    .picked
                    .iter()
                    .map(|(f, vals, u)| {
                        let vs: Vec<String> = vals.iter().map(|&v| val(*f, v)).collect();
                        format!("[{}, [{}], {}]", key(*f), vs.join(", "), val(*f, *u))
                    })
                    .collect();
                let _ = write!(
                    out,
                    "{{\"op\": \"a{}\", \"action\": {}, \"name\": {}, \"members\": [{}], \"chosen\": [{}], \"guessed\": [{}], \"picked\": [{}]}}",
                    s.op,
                    s.action,
                    esc(&t.name(s.action)),
                    mem.join(", "),
                    ch.join(", "),
                    gu.join(", "),
                    pk.join(", ")
                );
            }
            let facts: Vec<String> = plan.vars.iter().filter(|&&v| v < t.facts.len() as u32).map(|&v| key(v)).collect();
            let _ = write!(out, "], \"facts\": [{}], \"operators\": {}}}", facts.join(", "), plan.n_ops);
            let path = a.out.clone().ok_or("--out is needed")?;
            std::fs::write(&path, out + "\n").map_err(|e| e.to_string())?;
            Ok(())
        }
        "validate" => {
            // run a plan given as input action names (label#rec, one per line) on the reduced task: each step is the
            // reduced action that holds that input action (or, if (f) dropped it, the one that replaced it); its
            // precondition must hold; a guessed write that fires leaves its fact unknown, and reading an unknown
            // value fails; at the end the goal must hold
            let p = prepare(&a)?;
            let t = &p.task;
            let text = std::fs::read_to_string(a.names.as_ref().ok_or("--names is needed")?).map_err(|e| e.to_string())?;
            let by_name: std::collections::HashMap<&str, u32> = p.names.iter().enumerate().map(|(i, n)| (n.as_str(), i as u32)).collect();
            let mut holder: std::collections::HashMap<u32, usize> = Default::default();
            for i in 0..t.acts.len() {
                for &m in t.acts.members(i) {
                    holder.insert(m, i);
                }
            }
            let replaced: std::collections::HashMap<u32, u32> = p.replaced.iter().copied().collect();
            let mut state: Vec<u32> = t.starts[p.start].1.clone();
            let mut out = Vec::new();
            let mut ok = true;
            let mut guesses = Vec::new();
            for (n, name) in text.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).enumerate() {
                let Some(&id) = by_name.get(name) else {
                    out.push(format!("{}: not an input action", esc(name)));
                    ok = false;
                    break;
                };
                let mut id2 = id;
                while !holder.contains_key(&id2) {
                    match replaced.get(&id2) {
                        Some(&b) => id2 = b,
                        None => break,
                    }
                }
                let Some(&i) = holder.get(&id2) else {
                    out.push(format!("{}: dropped", esc(name)));
                    ok = false;
                    break;
                };
                let v = t.acts.view(i);
                if !v.pre().all(|(f, s)| state[f as usize] != NONE && contains(s, state[f as usize])) {
                    out.push(format!("{}: its precondition does not hold (step {})", esc(name), n + 1));
                    ok = false;
                    break;
                }
                let mut new = state.clone();
                for e in v.effs() {
                    if !e.when().all(|(f, s)| state[f as usize] != NONE && contains(s, state[f as usize])) {
                        if e.when().any(|(f, _)| state[f as usize] == NONE) {
                            new[e.fact as usize] = NONE;
                        }
                        continue;
                    }
                    if e.kind == K_SET {
                        new[e.fact as usize] = e.vals[0];
                    } else {
                        new[e.fact as usize] = NONE;
                        guesses.push(format!("[{}, {}]", n + 1, esc(t.key(e.fact))));
                    }
                }
                state = new;
                let how = if id2 != id { format!("replaced by {}", esc(&p.names[id2 as usize])) } else if t.acts.members(i).len() > 1 { format!("in {}", esc(&t.name(i))) } else { "kept".to_string() };
                out.push(format!("{}: {}", esc(name), how.trim_matches('"')));
            }
            let goal = ok && t.goal.iter().all(|(f, vals)| state[*f as usize] != NONE && contains(vals, state[*f as usize]));
            let js = format!(
                "{{\"steps\": [{}], \"ok\": {}, \"goal\": {}, \"guesses_fired\": [{}]}}\n",
                out.iter().map(|s| format!("\"{}\"", s.replace('"', ""))).collect::<Vec<_>>().join(", "),
                ok,
                goal,
                guesses.join(", ")
            );
            std::fs::write(a.out.as_ref().ok_or("--out is needed")?, js).map_err(|e| e.to_string())?;
            Ok(())
        }
        c => Err(format!("unknown command {}", c)),
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("taskcore: {}", e);
        std::process::exit(2);
    }
}
