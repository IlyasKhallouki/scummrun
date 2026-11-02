//! fmi-plan's check and bench tool: the planner port on a reading's files (the `FileReading` double), one request or a
//! sequence, each result as canonical JSON (the differential tests against plan_fd.plan_taskcore) and its parts'
//! seconds (the bench).
//!
//! usage:
//!   fmi-plan seq --reading DIR --reqs FILE [--search gbfs|fd|lama] [--dump DIR] [--cache DIR|none|off] [--keep N]
//!   (lama: FdLamaFirstCosts by the Rust port, Host::rust_lama)
//!   FILE: one request per line, `name<TAB>start.txt<TAB>caps.txt<TAB>goal[<TAB>bans.txt[<TAB>tail.txt]]`, as the
//!   harness writes them (start: `key<TAB>value`, "" where the domain lacks it; caps: `prefix<TAB>frames`; bans:
//!   `label<TAB>key<TAB>value...`; tail: one step name per line; goal: `K=V[|V],..`); "-" for none
//!   The result of request NAME goes to DIR/NAME.json; one JSON line of seconds per request on stdout.
//!   --tail-only 1: only the old tail's check (no search): status "plan" (source "tail") or "no tail".

use fmi_core::{val_text, Action, Ban, EffVal, Facts, Key, PlanRequest, PlanResult, Planner, Search, Val};
use fmi_plan::double::FileReading;
use fmi_plan::raw::value_of;
use fmi_plan::{Host, WarmPlanner};
use std::path::PathBuf;

fn esc(s: &str) -> String {
    taskcore::cli::esc(s)
}

fn read(p: &str) -> Result<String, String> {
    std::fs::read_to_string(p).map_err(|e| format!("{}: {}", p, e))
}

/// A start file as a facts.py map: each value its int or label; "" a value the fact's domain lacks (the first int,
/// from 999999999 down, that labels to none of its values) so that `write_start` writes "" again.
fn start_of(text: &str, domains: &fmi_core::Domains) -> Result<Facts, String> {
    let mut pairs = Vec::new();
    for line in text.lines().filter(|l| !l.is_empty()) {
        let (k, v) = line.split_once('\t').ok_or(format!("start line {:?}", line))?;
        let key = Key::parse(k);
        let val = if v.is_empty() {
            let d = domains.get(key).ok_or(format!("start: {} is no fact of the reading", k))?;
            let mut x = 999_999_999i64;
            while d.label(x) != fmi_core::NONE {
                x -= 1;
            }
            x
        } else {
            value_of(v).ok_or(format!("start value {:?}", v))?
        };
        pairs.push((key, val));
    }
    Ok(Facts::from_pairs(pairs))
}

fn goal_of(text: &str) -> Result<Vec<(Key, Vec<Val>)>, String> {
    text.split(',')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, vs) = p.split_once('=').ok_or(format!("goal {:?}", p))?;
            let vals = vs.split('|').map(|v| value_of(v).ok_or(format!("goal value {:?}", v))).collect::<Result<_, _>>()?;
            Ok((Key::parse(k), vals))
        })
        .collect()
}

fn caps_of(text: &str) -> Result<Vec<(String, i64)>, String> {
    text.lines()
        .filter(|l| !l.is_empty())
        .map(|l| {
            let (p, c) = l.split_once('\t').ok_or(format!("caps line {:?}", l))?;
            Ok((p.to_string(), c.trim().parse().map_err(|e| format!("caps {:?}: {}", l, e))?))
        })
        .collect()
}

fn bans_of(text: &str) -> Result<Vec<Ban>, String> {
    text.lines()
        .filter(|l| !l.is_empty())
        .map(|l| {
            let parts: Vec<&str> = l.split('\t').collect();
            let facts = parts[1..]
                .chunks(2)
                .map(|c| Ok((Key::parse(c[0]), value_of(c.get(1).copied().unwrap_or("")).ok_or(format!("ban value in {:?}", l))?)))
                .collect::<Result<_, String>>()?;
            Ok(Ban { label: parts[0].to_string(), facts })
        })
        .collect()
}

fn vals_json(v: &[Val]) -> String {
    let mut t: Vec<String> = v.iter().map(|&x| val_text(x)).collect();
    t.sort();
    format!("[{}]", t.iter().map(|s| esc(s)).collect::<Vec<_>>().join(", "))
}

fn sets_json(s: &[(Key, Vec<Val>)], sort: bool) -> String {
    let mut v: Vec<(String, String)> = s.iter().map(|(k, vs)| (k.text(), vals_json(vs))).collect();
    if sort {
        v.sort();
    }
    format!("[{}]", v.iter().map(|(k, x)| format!("[{}, {}]", esc(k), x)).collect::<Vec<_>>().join(", "))
}

/// An action as the comparison reads it (plan_ref.py writes the same): name, precondition by key, effects in order.
fn action_json(a: &Action) -> String {
    let effs: Vec<String> = a
        .effects
        .iter()
        .map(|e| {
            let (kind, payload) = match &e.v {
                EffVal::Value(v) => ("set", esc(&val_text(*v))),
                EffVal::Values(vs) => ("guess", vals_json(vs)),
                EffVal::Add(d) => ("add", esc(&d.to_string())),
            };
            format!("[{}, \"{}\", {}, {}]", esc(&e.key.text()), kind, payload, sets_json(&e.when, false))
        })
        .collect();
    format!(
        "{{\"name\": {}, \"pre\": {}, \"effects\": [{}], \"cost\": {}}}",
        esc(&a.name),
        sets_json(&a.pre, true),
        effs.join(", "),
        a.cost
    )
}

fn result_json(r: &PlanResult) -> String {
    let keys = |v: &[Key]| format!("[{}]", v.iter().map(|k| esc(&k.text())).collect::<Vec<_>>().join(", "));
    let steps: Vec<String> = r
        .steps
        .iter()
        .map(|s| {
            let ch: Vec<String> = s.chosen.iter().map(|(k, v)| format!("[{}, {}]", esc(&k.text()), esc(&val_text(*v)))).collect();
            let gu: Vec<String> =
                s.guessed.iter().map(|(k, v, w)| format!("[{}, {}, {}]", esc(&k.text()), esc(&val_text(*v)), w)).collect();
            format!("[{}, {}, {}, [{}], [{}]]", esc(&s.op), s.action, esc(&s.name), ch.join(", "), gu.join(", "))
        })
        .collect();
    let acts = |v: &[Action]| format!("[{}]", v.iter().map(action_json).collect::<Vec<_>>().join(", "));
    let choices: Vec<String> = r.choices.iter().map(|(i, ps)| format!("\"{}\": {}", i, acts(ps))).collect();
    let folds: Vec<String> = r.folds.iter().map(|f| f.as_ref().map_or("null".into(), |s| esc(s))).collect();
    let misses: Vec<String> = r.misses.iter().map(|(k, v)| format!("[{}, {}]", esc(&k.text()), esc(&val_text(*v)))).collect();
    format!(
        "{{\"status\": {}, \"plan_cost\": {}, \"predicted_goal\": {}, \"file_facts\": {}, \"needed_facts\": {}, \"folds\": [{}], \"steps\": [{}], \"paths\": {}, \"actions\": {}, \"choices\": {{{}}}, \"misses\": [{}], \"n_misses\": {}}}\n",
        esc(&r.status),
        r.plan_cost.map_or("null".into(), |c| c.to_string()),
        r.predicted_goal,
        keys(&r.file_facts),
        keys(&r.needed_facts),
        folds.join(", "),
        steps.join(", "),
        acts(&r.paths),
        acts(&r.actions),
        choices.join(", "),
        misses.join(", "),
        r.n_misses
    )
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().cloned().unwrap_or_default();
    let mut opt = std::collections::HashMap::new();
    let mut it = args.iter().skip(1);
    while let Some(k) = it.next() {
        opt.insert(k.clone(), it.next().cloned().ok_or(format!("{} needs a value", k))?);
    }
    if cmd != "seq" {
        return Err("usage: fmi-plan seq --reading DIR --reqs FILE [--search gbfs|fd|lama] [--dump DIR] [--cache DIR|none]".into());
    }
    let mut host = Host::from_env();
    match opt.get("--cache").map(String::as_str) {
        Some("off") => host.cache = false,
        Some("none") => host.cache_dir = None,
        Some(d) => host.cache_dir = Some(PathBuf::from(d)),
        None => {}
    }
    if let Some(k) = opt.get("--keep") {
        host.keep = k.parse().map_err(|e| format!("--keep: {}", e))?;
    }
    let search = match opt.get("--search").map(String::as_str).unwrap_or("gbfs") {
        "gbfs" => Search::Gbfs,
        "fd" => Search::FdLamaFirstCosts,
        // FdLamaFirstCosts by the Rust port (Host::rust_lama, as FMI_PLAN_LAMA=rust)
        "lama" => {
            host.rust_lama = true;
            Search::FdLamaFirstCosts
        }
        s => return Err(format!("--search {}: gbfs, fd or lama", s)),
    };
    let dump = opt.get("--dump").map(PathBuf::from);
    if let Some(d) = &dump {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let dir = PathBuf::from(opt.get("--reading").ok_or("--reading is needed")?);
    let t0 = std::time::Instant::now();
    let c0 = fmi_plan::cpu_s();
    let reading = FileReading::open(&dir)?;
    let open_s = t0.elapsed().as_secs_f64();
    let t1 = std::time::Instant::now();
    let c1 = fmi_plan::cpu_s();
    let mut planner = WarmPlanner::new(&host, &reading).map_err(|e| format!("{:?}", e))?;
    println!(
        "{{\"reading\": {}, \"open_s\": {:.4}, \"open_cpu_s\": {:.2}, \"planner_s\": {:.4}, \"planner_cpu_s\": {:.2}}}",
        esc(&dir.display().to_string()),
        open_s,
        c1 - c0,
        t1.elapsed().as_secs_f64(),
        fmi_plan::cpu_s() - c1
    );
    let reqs = read(opt.get("--reqs").ok_or("--reqs is needed")?)?;
    for line in reqs.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')) {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 4 {
            return Err(format!("request line {:?}", line));
        }
        let opt_file = |i: usize| -> Option<&str> { f.get(i).copied().filter(|s| !s.is_empty() && *s != "-") };
        let req = PlanRequest {
            goal: goal_of(f[3])?,
            start: start_of(&read(f[1])?, reading_domains(&reading))?,
            bans: match opt_file(4) {
                Some(p) => bans_of(&read(p)?)?,
                None => vec![],
            },
            caps: match opt_file(2) {
                Some(p) => caps_of(&read(p)?)?,
                None => vec![],
            },
            search,
            start_check: "needed".into(),
            tail: match opt_file(5) {
                Some(p) => Some(read(p)?.lines().filter(|l| !l.is_empty()).map(String::from).collect()),
                None => None,
            },
            time_limit_s: 600,
        };
        let r = if opt.contains_key("--tail-only") {
            match planner.tail_only(&req) {
                Ok(Some(r)) => Ok(r),
                Ok(None) => Ok(PlanResult { status: "no tail".into(), source: "tail".into(), ..Default::default() }),
                Err(e) => Err(e),
            }
        } else {
            planner.plan(&req)
        };
        let tm = planner.last.clone();
        let (status, n, source, err) = match &r {
            Ok(r) => (r.status.clone(), r.steps.len(), r.source.clone(), String::new()),
            Err(e) => ("error".to_string(), 0, String::new(), format!("{:?}", e)),
        };
        let tail_info = match &r {
            Ok(r) if r.source == "tail" && r.status == "plan" => format!(
                ", \"tail\": {}, \"tail_names\": [{}]",
                r.stats,
                r.steps.iter().map(|s| esc(&s.name)).collect::<Vec<_>>().join(", ")
            ),
            _ => String::new(),
        };
        println!(
            "{{\"name\": {}, \"status\": {}, \"source\": {}, \"steps\": {}, \"kept\": {}, \"total_s\": {:.4}, \"cpu_s\": {:.2}, \"key_s\": {:.4}, \"tail_s\": {:.4}, \"reduce_s\": {:.3}, \"build_s\": {:.3}, \"relax_s\": {:.3}, \"search_s\": {:.3}, \"check_s\": {:.4}, \"decode_s\": {:.4}, \"predict_s\": {:.4}, \"store_s\": {:.4}, \"operators\": {}, \"unary\": {}, \"expanded\": {}, \"evaluated\": {}, \"error\": {}{}}}",
            esc(f[0]),
            esc(&status),
            esc(&source),
            n,
            esc(&tm.kept),
            tm.total,
            tm.cpu,
            tm.key,
            tm.tail,
            tm.reduce,
            tm.build,
            tm.relax,
            tm.search,
            tm.check,
            tm.decode,
            tm.predict,
            tm.store,
            tm.operators,
            tm.unary,
            tm.expanded,
            tm.evaluated,
            esc(&err),
            tail_info
        );
        if opt.contains_key("--rss") {
            println!("{{\"rss\": {}, \"peak_rss_mb\": {}}}", esc(f[0]), taskcore::model::peak_rss() / (1 << 20));
        }
        if let (Some(d), Ok(r)) = (&dump, &r) {
            std::fs::write(d.join(format!("{}.json", f[0])), result_json(r)).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn reading_domains(r: &FileReading) -> &fmi_core::Domains {
    use fmi_core::ReadingSource;
    r.domains()
}

fn main() {
    if let Err(e) = run() {
        eprintln!("fmi-plan: {}", e);
        std::process::exit(2);
    }
}
