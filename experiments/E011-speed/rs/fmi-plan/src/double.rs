//! `FileReading`: the reading port (fmi_core::ReadingSource) over a reading's files, the test double this component
//! is built and tested against until readio's fmi-reading replaces it at integration (plan.md, "Components").
//!
//! It reads what packed_reading.load_reading reads: reading.json (its goal, the bounds of its domains, its start),
//! reading.tc (held in memory here; fmi-reading maps it), reading.actions.jsonl (an index of its lines),
//! reading.reached. `action(i)` is PackedTask._action's action (raw.rs); `record(i)` is not in the double (the planner
//! does not read records). `domains()` is PackedTask.facts: each packed fact's values, with reading.json's bounds where
//! BELOW or ABOVE is a value.

use crate::json::{self, Json};
use crate::raw::{value_of, Raw};
use fmi_core::{Action, Domain, Domains, Facts, Key, ReadingSource, Record, Val, ABOVE, BELOW};
use std::path::{Path, PathBuf};

pub struct FileReading {
    dir: PathBuf,
    head: String,
    packed: Vec<u8>,
    id: [u8; 20],
    lines: Vec<u8>,
    line_off: Vec<usize>,
    reached: String,
    domains: Domains,
    goal: Vec<(Key, Vec<Val>)>,
    start: Facts,
    raw: Raw,
}

fn words(b: &[u8]) -> impl Fn(usize) -> u32 + '_ {
    move |w| u32::from_le_bytes([b[4 * w], b[4 * w + 1], b[4 * w + 2], b[4 * w + 3]])
}

impl FileReading {
    /// The reading whose reading.json is in `dir`.
    pub fn open(dir: &Path) -> Result<FileReading, String> {
        let rd = |n: &str| std::fs::read(dir.join(n)).map_err(|e| format!("{}: {}", dir.join(n).display(), e));
        let head = String::from_utf8(rd("reading.json")?).map_err(|e| e.to_string())?;
        let doc = json::parse(&head)?;
        let packed_name = doc.get("packed").and_then(Json::as_str).ok_or("reading.json: no packed")?.to_string();
        let lines_name = doc.get("actions").and_then(Json::as_str).ok_or("reading.json: no actions")?.to_string();
        let packed = rd(&packed_name)?;
        let id = taskcore::cache::sha1(&packed);
        let lines = rd(&lines_name)?;
        let mut line_off = vec![0usize];
        for (i, &c) in lines.iter().enumerate() {
            if c == b'\n' {
                line_off.push(i + 1);
            }
        }
        let reached = String::from_utf8(rd("reading.reached").unwrap_or_default()).map_err(|e| e.to_string())?;
        // the packed file's tables (taskcore/README.md): strings, facts, goal
        let w = words(&packed);
        let nw = packed.len() / 4;
        let foot = nw - 8;
        let (o_str, o_facts, o_goal) = (w(foot + 2) as usize, w(foot + 3) as usize, w(foot + 5) as usize);
        let ns = w(o_str) as usize;
        let mut strs = Vec::with_capacity(ns);
        let mut pos = (o_str + 1 + ns) * 4;
        for i in 0..ns {
            let l = w(o_str + 1 + i) as usize;
            strs.push(String::from_utf8(packed[pos..pos + l].to_vec()).map_err(|e| e.to_string())?);
            pos += l;
        }
        let bounds = doc.get("bounds");
        let bound = |k: &str, i: usize| -> Option<i64> { bounds?.get(k)?.as_arr()?.get(i)?.as_i64() };
        let mut q = o_facts;
        let nf = w(q) as usize;
        q += 1;
        let mut fact_keys = Vec::with_capacity(nf);
        let mut doms = Vec::with_capacity(nf);
        for _ in 0..nf {
            let (key, size) = (w(q) as usize, w(q + 1) as usize);
            fact_keys.push(key as u32);
            let mut values: Vec<Val> = (0..size)
                .map(|j| value_of(&strs[w(q + 2 + j) as usize]).ok_or("a domain value that is no int, below or above"))
                .collect::<Result<_, _>>()?;
            values.sort_unstable();
            let k = &strs[key];
            let low = if values.contains(&BELOW) { bound(k, 0) } else { None };
            let high = if values.contains(&ABOVE) { bound(k, 1) } else { None };
            doms.push((Key::parse(k), Domain { values, low, high }));
            q += 2 + size;
        }
        doms.sort_by_key(|d| d.0);
        let mut q = o_goal;
        let mut goal = Vec::new();
        for _ in 0..w(q) {
            let (f, m) = (w(q + 1) as usize, w(q + 2) as usize);
            let mut vs: Vec<Val> =
                (0..m).map(|j| value_of(&strs[w(q + 3 + j) as usize]).ok_or("a goal value")).collect::<Result<_, _>>()?;
            vs.sort_unstable();
            goal.push((Key::parse(&strs[fact_keys[f] as usize]), vs));
            q += 2 + m;
        }
        let mut start = Facts::new();
        if let Some(Json::Obj(fs)) = doc.get("start").and_then(|s| s.get("facts")) {
            for (k, v) in fs {
                if let Some(n) = v.as_i64() {
                    start.set(Key::parse(k), n);
                }
            }
        }
        let path = dir.join(&packed_name);
        let raw = Raw::new(&path, &packed, &strs, &fact_keys)?;
        #[allow(clippy::drop_non_drop)]
        drop(w);
        Ok(FileReading {
            dir: dir.to_path_buf(),
            head,
            packed,
            id,
            lines,
            line_off,
            reached,
            domains: Domains(doms),
            goal,
            start,
            raw,
        })
    }
}

impl ReadingSource for FileReading {
    fn id(&self) -> [u8; 20] {
        self.id
    }
    fn dir(&self) -> &Path {
        &self.dir
    }
    fn head(&self) -> &str {
        &self.head
    }
    fn packed(&self) -> &[u8] {
        &self.packed
    }
    fn n_actions(&self) -> u32 {
        self.raw.n() as u32
    }
    fn action_line(&self, i: u32) -> &str {
        let (a, b) = (self.line_off[i as usize], self.line_off[i as usize + 1] - 1);
        std::str::from_utf8(&self.lines[a..b]).unwrap_or("")
    }
    fn by_name(&self, name: &str) -> Option<u32> {
        (0..self.raw.n() as u32).rev().find(|&i| self.raw.action(i).map(|a| a.name == name).unwrap_or(false))
    }
    fn n_lists(&self) -> u32 {
        0
    }
    fn list_line(&self, _i: u32) -> &str {
        ""
    }
    fn reached(&self) -> &str {
        &self.reached
    }
    fn domains(&self) -> &Domains {
        &self.domains
    }
    fn goal(&self) -> &[(Key, Vec<Val>)] {
        &self.goal
    }
    fn start(&self) -> &Facts {
        &self.start
    }
    fn action(&self, i: u32) -> Action {
        self.raw.action(i).expect("the packed file's action")
    }
    fn record(&self, _i: u32) -> Record {
        unimplemented!("FileReading (the planner's double) holds no records")
    }
}
