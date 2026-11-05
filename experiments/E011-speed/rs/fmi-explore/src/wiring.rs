//! replay_part1.py's wiring for the harness, line for line: `reader_switches`, `with_facts`, `reach_extender` and
//! `refiner` (the reach extension's and E007's refinement's readings, sharing the attempt's start files and fact
//! variables), `goal_text_of`, and the reading's start name rules. The reader itself is the `Readings` port.

use crate::files;
use crate::ports::{ReadReq, Readings};
use fmi_core::ReadingSource;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// read_rust's switches for a reading made again with a reading's own switches (reading.json).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Switches {
    pub fact_vars: Vec<i64>,
    pub switches: String,
    pub reach: bool,
    pub frame_costs: bool,
    pub reach_also: Vec<PathBuf>,
}

/// replay_part1.reader_switches.
pub fn reader_switches(doc: &Value, reach_also: &[PathBuf]) -> Switches {
    let mut out = Switches::default();
    if files::truthy(doc.get("fact_vars")) {
        out.fact_vars = doc["fact_vars"].as_array().map(|a| a.iter().filter_map(|x| x.as_i64()).collect()).unwrap_or_default();
    }
    if files::truthy(doc.get("switches")) {
        out.switches = files::py_str(doc.get("switches"));
    }
    if files::truthy(doc.get("reach")) {
        out.reach = true;
    }
    if doc.get("frame_costs").map(|f| !f.is_null()).unwrap_or(false) {
        out.frame_costs = true;
    }
    out.reach_also = reach_also.to_vec();
    out
}

/// replay_part1.with_facts: the switches with the run's fact variables so far, when it has any.
pub fn with_facts(mut sw: Switches, facts: &[i64]) -> Switches {
    if !facts.is_empty() {
        sw.fact_vars = facts.to_vec();
    }
    sw
}

/// The seed and the start-rows file a reading was read from (its start name: "results/start-part1.jsonl seed N" or
/// "/ABS/PATH.jsonl seed N").
pub fn start_of(doc: &Value) -> Result<(i64, Option<PathBuf>), String> {
    let name = doc.get("start").and_then(|s| s.get("name")).and_then(|n| n.as_str()).unwrap_or("");
    if let Some(seed) = name.strip_prefix("results/start-part1.jsonl seed ") {
        if let Ok(s) = seed.parse() {
            return Ok((s, None));
        }
    }
    if let Some((path, seed)) = name.rsplit_once(" seed ") {
        if path.starts_with('/') && path.ends_with(".jsonl") && !path.chars().any(|c| c.is_whitespace()) {
            if let Ok(s) = seed.parse::<i64>() {
                return Ok((s, Some(PathBuf::from(path))));
            }
        }
    }
    Err(format!("--reach-extend needs a reading of results/start-part1.jsonl's starts or of a start-rows file, not {name}"))
}

/// replay_part1.goal_text_of: the reading's goal ({fact: [value]}) as reader-scope's --goal.
pub fn goal_text_of(goal: &Value) -> Result<String, String> {
    let mut parts: Vec<(String, String)> = Vec::new();
    for (k, vs) in goal.as_object().cloned().unwrap_or_default() {
        let a = vs.as_array().cloned().unwrap_or_default();
        if a.len() != 1 {
            return Err(format!("reader-scope's --goal takes one value per fact; {k} has {}", files::py_repr(&vs)));
        }
        parts.push((k, files::py_str(a.first())));
    }
    parts.sort();
    Ok(parts.into_iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(","))
}

/// The attempt's reach extender and refiner (replay_part1.reach_extender and refiner over one reading): the start
/// files and fact variables they share, and the reader.
pub struct Wiring {
    pub readings: Arc<dyn Readings>,
    /// the attempt's first reading's reading.json (its switches), as warm.rs keeps it
    pub doc: Arc<Value>,
    pub run_dir: PathBuf,
    pub threads: u32,
    pub starts: Vec<PathBuf>,
    pub facts: Vec<i64>,
    pub elsewhere: bool,
    seed: i64,
    start_rows: Option<PathBuf>,
    rooms: Vec<i64>,
    goal: String,
    refines: usize,
    pub cpu_s: f64,
    pub cancel: Arc<AtomicBool>,
    pub log: Option<Box<dyn Fn(&str) + Send>>,
}

impl Wiring {
    /// step.py's attempt: `starts = []`, `facts = list(rd.doc.get("fact_vars") or [])`, then reach_extender and
    /// refiner on the reading (the extender adds the reading's own reach_also files to the starts).
    pub fn new(readings: Arc<dyn Readings>, rd: &dyn ReadingSource, run_dir: &Path, threads: u32, cancel: Arc<AtomicBool>)
               -> Result<Wiring, String> {
        let doc = crate::warm::head(rd);
        if !files::truthy(doc.get("reach")) {
            return Err("--reach-extend needs a reading made with --reach".into());
        }
        let (seed, start_rows) = start_of(&doc)?;
        let rooms: Vec<i64> = doc["scope"]["rooms"].as_array().map(|a| a.iter().filter_map(|x| x.as_i64()).collect()).unwrap_or_default();
        let goal = goal_text_of(&doc["goal"])?;
        let has_elsewhere = rd.domains().0.iter().any(|(k, d)| {
            (k.kind() == fmi_core::K_POS || k.kind() == fmi_core::K_ESCPOS) && d.values.contains(&crate::start::ELSEWHERE)
        });
        let elsewhere = files::truthy(doc.get("elsewhere")) || has_elsewhere;
        let facts: Vec<i64> = doc.get("fact_vars").and_then(|f| f.as_array()).map(|a| a.iter().filter_map(|x| x.as_i64()).collect()).unwrap_or_default();
        let mut starts: Vec<PathBuf> = Vec::new();
        for f in doc.get("reach_also").and_then(|r| r.as_array()).cloned().unwrap_or_default() {
            let p = PathBuf::from(files::py_str(Some(&f)));
            if !starts.contains(&p) {
                starts.push(p);
            }
        }
        Ok(Wiring { readings, doc, run_dir: run_dir.to_path_buf(), threads, starts, facts, elsewhere, seed, start_rows,
                    rooms, goal, refines: 0, cpu_s: 0.0, cancel, log: None })
    }

    fn req(&self, name: &str, out: &Path, stream: bool) -> ReadReq {
        let sw = with_facts(reader_switches(&self.doc, &self.starts), &self.facts);
        ReadReq {
            rooms: self.rooms.clone(),
            seed: self.seed,
            goal_text: self.goal.clone(),
            name: name.into(),
            out: out.to_path_buf(),
            threads: self.threads,
            slot_locals: vec![],
            domain_goals: self.doc.get("domain_goals").and_then(|d| d.as_str()).unwrap_or("none").to_string(),
            elsewhere: self.elsewhere,
            live_locals: self.doc.get("live_locals").map(|l| !l.is_null()).unwrap_or(false),
            split_limit: files::truthy(self.doc.get("split_limit")),
            switches: sw.switches,
            reach: sw.reach,
            frame_costs: sw.frame_costs,
            reach_also: sw.reach_also,
            start_rows: self.start_rows.clone(),
            fact_vars: sw.fact_vars,
            stream,
        }
    }

    fn read(&mut self, req: &ReadReq) -> Result<Arc<dyn ReadingSource>, String> {
        let c0 = crate::machine::process_cpu_s();
        let path = self.readings.read(req, &self.cancel)?;
        self.cpu_s += crate::machine::process_cpu_s() - c0;
        self.readings.open(&path)
    }

    /// replay_part1.reach_extender's `extend`: the start as a file (its text `start::reach_start_text`), then the scope
    /// read again from every start so far.
    pub fn extend(&mut self, start: &str) -> Result<Arc<dyn ReadingSource>, String> {
        let n = self.starts.len() + 1;
        let d = self.run_dir.join(format!("reach-{n:02}"));
        let f = d.join("start.txt");
        crate::start::write_text(&f, start)?;
        self.starts.push(f);
        let req = self.req(&format!("reach-{n:02}"), &d, false);
        self.read(&req)
    }

    /// replay_part1.refiner's `refine`: the scope read again with the new variables made facts, streamed.
    pub fn refine(&mut self, new: &[i64], start: &str) -> Result<Arc<dyn ReadingSource>, String> {
        self.refines += 1;
        let d = self.run_dir.join(format!("refine-{:02}", self.refines));
        let f = d.join("start.txt");
        crate::start::write_text(&f, start)?;
        self.starts.push(f);
        for n in new {
            if !self.facts.contains(n) {
                self.facts.push(*n);
            }
        }
        if let Some(l) = &self.log {
            l(&format!("refinement {}: reading the scope again with var {:?} made facts, from {} starts", self.refines, self.facts,
                       self.starts.len() + 1));
        }
        let req = self.req(&format!("refine-{:02}", self.refines), &d, true);
        self.read(&req)
    }
}

impl crate::harness::Rereads for Wiring {
    fn extend(&mut self, start: &str) -> Result<Arc<dyn ReadingSource>, String> {
        Wiring::extend(self, start)
    }
    fn add_start(&mut self, start: &str) -> Result<(), String> {
        let n = self.starts.len() + 1;
        let f = self.run_dir.join(format!("seed-{n:02}")).join("start.txt");
        crate::start::write_text(&f, start)?;
        self.starts.push(f);
        Ok(())
    }
    fn refine(&mut self, new: &[i64], start: &str) -> Result<Arc<dyn ReadingSource>, String> {
        Wiring::refine(self, new, start)
    }
    fn cpu_s(&self) -> f64 {
        self.cpu_s
    }
}
