//! fmi-reading (E011, experiments/E011-speed/plan.md, component readio): a reading's files as
//! `fmi_core::ReadingSource`, the exact reading cache, and the reuse rules of E008 and E009.
//!
//! * [`Reading`]: reading.json, reading.tc, reading.actions.jsonl, reading.lists.jsonl, reading.reached and
//!   reading.idx (F3), the big files mapped. Opening reads the packed file's tables (strings, facts, goal), the head
//!   and the index; an action and its record are decoded when asked for, as packed_reading.py `PackedTask._action`
//!   decodes them, and `by_name` looks the name up by its hash, so nothing walks the actions at open (the Python
//!   reference's strict setup decoded all of them). A reading made before reading.idx gets one at its first open
//!   (`idx::build`, written next to it).
//! * [`cache`]: the exact reading cache (plan.md, readio item 3).
//! * [`reuse`]: step.py's `reusable` (E008) and `reuse_keys_of` / `reuse_keys` (E009's K).

pub mod cache;
pub mod idx;
pub mod line;
pub mod mmap;
pub mod pyjson;
pub mod reuse;
pub mod tc;

use fmi_core::{val_text, Action, Domain, Domains, EffVal, Effect, Facts, Key, ReadingSource, RecEff, Record, Val};
use fmi_core::{ABOVE, BELOW};
use idx::Idx;
use mmap::Map;
use serde_json::Value;
use std::io;
use std::path::{Path, PathBuf};
use tc::{Packed, Payload, RawAction, RawEffect, OP_ADD, OP_GUESS, OP_SET, REC_NONE};

/// packed_reading.FORMAT
pub const FORMAT: &str = "E003 reading 1";

fn bad(what: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, what)
}

/// A reading's files (module docstring).
pub struct Reading {
    dir: PathBuf,
    head_path: PathBuf,
    head: String,
    doc: Value,
    packed: Packed,
    actions: Map,
    lists: Option<Map>,
    idx: Idx,
    reached: String,
    domains: Domains,
    start: Facts,
}

/// Whether an index agrees with the files it indexes (counts and sizes; a stale one is made again).
fn idx_fits(idx: &Idx, packed: &Packed, actions: &Map, lists: Option<&Map>) -> bool {
    let n = idx.n_actions;
    n == packed.n_actions
        && idx.n_names == n
        && idx.action_off(n) == actions.len() as u64
        && idx.list_off(idx.n_lists) == lists.map_or(0, |m| m.len() as u64)
        && (n == 0 || {
            let last = idx.tc_word(n - 1);
            last >= packed.body && last + 1 + packed.word(last) as u64 == packed.o_str
        })
}

fn json_val(v: &Value) -> io::Result<Val> {
    match v {
        Value::Number(n) => n.as_i64().ok_or_else(|| bad(format!("not an int value: {n}"))),
        Value::String(s) if s == "below" => Ok(BELOW),
        Value::String(s) if s == "above" => Ok(ABOVE),
        other => Err(bad(format!("not a domain value: {other}"))),
    }
}

impl Reading {
    /// The reading in `dir` (its reading.json).
    pub fn open(dir: &Path) -> io::Result<Reading> {
        Reading::open_head(&dir.join("reading.json"))
    }

    /// The reading whose head is `head_path` (packed_reading.load_reading): its files are the ones the head names,
    /// next to it; its index is the head's name with `.idx`.
    pub fn open_head(head_path: &Path) -> io::Result<Reading> {
        let head = std::fs::read_to_string(head_path)?;
        let doc: Value = serde_json::from_str(&head).map_err(|e| bad(format!("{}: {e}", head_path.display())))?;
        if doc["format"].as_str() != Some(FORMAT) {
            return Err(bad(format!("{}: format {}, not {FORMAT:?}", head_path.display(), doc["format"])));
        }
        let dir = head_path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let name = |k: &str| doc[k].as_str().map(|s| dir.join(s)).ok_or_else(|| bad(format!("reading.json: no {k:?}")));
        let packed_path = name("packed")?;
        let packed = Packed::open(&packed_path)?;
        let actions = Map::open(&name("actions")?)?;
        let lists = match doc.get("lists").and_then(Value::as_str) {
            Some(l) => Some(Map::open(&dir.join(l))?),
            None => None,
        };
        let idx_path = head_path.with_extension("idx");
        let idx = match Idx::open(&idx_path) {
            Ok(i) if idx_fits(&i, &packed, &actions, lists.as_ref()) => i,
            _ => {
                let b = idx::build(&packed, actions.bytes(), lists.as_ref().map_or(&[][..], |m| m.bytes()))?;
                let tmp = idx_path.with_extension(format!("idx.{}.tmp", std::process::id()));
                std::fs::write(&tmp, &b)?;
                std::fs::rename(&tmp, &idx_path)?;
                Idx::from_vec(b)?
            }
        };
        let reached = std::fs::read_to_string(packed_path.with_extension("reached")).unwrap_or_default();
        // the task's facts: the packed file's values, low and high from the head's bounds (PackedTask)
        let bounds = doc.get("bounds").and_then(Value::as_object);
        let mut doms = Vec::with_capacity(packed.fact_keys.len());
        for (fid, &k) in packed.fact_keys.iter().enumerate() {
            let vals = tc::sorted(packed.fact_values[fid].clone());
            let (lo, hi) = bounds
                .and_then(|b| b.get(packed.s(packed.fact_sid[fid])))
                .and_then(Value::as_array)
                .map_or((None, None), |a| (a.first().and_then(Value::as_i64), a.get(1).and_then(Value::as_i64)));
            let low = if vals.contains(&BELOW) { lo } else { None };
            let high = if vals.contains(&ABOVE) { hi } else { None };
            doms.push((k, Domain { values: vals, low, high }));
        }
        doms.sort_by_key(|d| d.0);
        // the head's goal is the packed file's (load_reading)
        let mut goal_doc: Vec<(Key, Vec<Val>)> = Vec::new();
        for (k, vs) in doc["goal"].as_object().ok_or_else(|| bad("reading.json: no goal".into()))? {
            let vals: io::Result<Vec<Val>> = vs.as_array().map_or(Ok(vec![]), |a| a.iter().map(json_val).collect());
            goal_doc.push((Key::parse(k), tc::sorted(vals?)));
        }
        goal_doc.sort_by_key(|g| g.0);
        let mut goal_tc = packed.goal.clone();
        goal_tc.sort_by_key(|g| g.0);
        if goal_doc != goal_tc {
            return Err(bad(format!("{}: its goal is not the packed file's", head_path.display())));
        }
        let mut start = Vec::new();
        if let Some(f) = doc["start"]["facts"].as_object() {
            for (k, v) in f {
                start.push((Key::parse(k), v.as_i64().ok_or_else(|| bad(format!("start fact {k}: {v}")))?));
            }
        }
        Ok(Reading {
            dir,
            head_path: head_path.to_path_buf(),
            head,
            doc,
            packed,
            actions,
            lists,
            idx,
            reached,
            domains: Domains(doms),
            start: Facts::from_pairs(start),
        })
    }

    /// reading.json, parsed (object keys sorted).
    pub fn doc(&self) -> &Value {
        &self.doc
    }

    pub fn head_path(&self) -> &Path {
        &self.head_path
    }

    pub fn index(&self) -> &Idx {
        &self.idx
    }

    pub fn tc(&self) -> &Packed {
        &self.packed
    }

    /// The task's fact keys in the packed file's order (PackedTask.facts' keys).
    pub fn fact_keys(&self) -> &[Key] {
        &self.packed.fact_keys
    }

    /// PackedTask._action's check: the line's i, rec and label are the packed action's.
    fn check(&self, i: u32, raw: &RawAction, li: i64, lrec: i64, llabel: &str) {
        let label = self.packed.s(raw.label);
        if li != i as i64 || lrec != raw.rec as i64 || llabel != label {
            panic!("action {i}: the line {li}, rec {lrec}, {llabel:?} is not the packed file's rec {}, {label:?}", raw.rec);
        }
    }

    /// The parsed line of action i, checked against the packed action.
    fn checked_line(&self, i: u32, raw: &RawAction) -> line::Line<'_> {
        let ln = line::parse(self.action_line(i)).unwrap_or_else(|e| panic!("action {i}: its line: {e}"));
        self.check(i, raw, ln.i, ln.rec, &ln.label);
        ln
    }

    fn sets(&self, sets: Vec<(u32, Vec<Val>)>) -> Vec<(Key, Vec<Val>)> {
        sets.into_iter().map(|(f, v)| (self.packed.fact_keys[f as usize], tc::sorted(v))).collect()
    }

    /// `{k: set(v) for k, v in raw["pre"]}`: a later set of the same key replaces the earlier one in its place.
    fn pre_dict(&self, pre: Vec<(u32, Vec<Val>)>) -> Vec<(Key, Vec<Val>)> {
        let mut out: Vec<(Key, Vec<Val>)> = Vec::with_capacity(pre.len());
        for (k, v) in self.sets(pre) {
            match out.iter_mut().find(|x| x.0 == k) {
                Some(x) => x.1 = v,
                None => out.push((k, v)),
            }
        }
        out
    }

    fn name_of(&self, raw: &RawAction) -> String {
        let l = self.packed.s(raw.label);
        if raw.rec == REC_NONE {
            l.to_string()
        } else {
            format!("{l}#{}", raw.rec)
        }
    }

    fn effects_of(&self, raw: &RawAction) -> Vec<(Key, RecEff)> {
        let mut by: Vec<(u32, Vec<&RawEffect>)> = Vec::new();
        for e in &raw.effects {
            match by.iter_mut().find(|x| x.0 == e.fact) {
                Some(x) => x.1.push(e),
                None => by.push((e.fact, vec![e])),
            }
        }
        let key = |f: u32| self.packed.fact_keys[f as usize];
        let only = |v: &[Val], x: Val| !v.is_empty() && v.iter().all(|&y| y == x);
        let mut out = Vec::with_capacity(by.len());
        for (fid, es) in by {
            let k = key(fid);
            if es.len() == 2 && es.iter().all(|e| e.op == OP_SET && e.when.len() == 1) {
                let texts = |e: &RawEffect| {
                    let mut t: Vec<String> = tc::sorted(e.when[0].1.clone()).into_iter().map(val_text).collect();
                    t.sort();
                    t
                };
                let mut s = es.clone();
                s.sort_by_key(|e| texts(e));
                let (e0, e1) = (s[0], s[1]);
                if e0.when[0].0 == e1.when[0].0
                    && only(&e0.when[0].1, 0)
                    && only(&e1.when[0].1, 1)
                    && e0.payload == Payload::Set(0)
                    && e1.payload == Payload::Set(1)
                {
                    out.push((k, RecEff::Copy(key(e0.when[0].0))));
                    continue;
                }
            }
            if es.len() != 1 {
                panic!("{}: {} effects that are not a copy", k.text(), es.len());
            }
            let e = es[0];
            let eff = match (&e.payload, e.op) {
                (Payload::Guess(v), OP_GUESS) => RecEff::Guess(tc::sorted(v.clone())),
                (Payload::Add(d), OP_ADD) => RecEff::Add(*d),
                (Payload::Set(v), OP_SET) if e.when.is_empty() => RecEff::Set(*v),
                (Payload::Set(v), OP_SET) if e.when.len() == 1 && e.when[0].0 != fid && only(&e.when[0].1, 1) => {
                    RecEff::When(key(e.when[0].0), *v)
                }
                _ => panic!("{}: a set under {:?}, which build_task does not write", k.text(), e.when),
            };
            out.push((k, eff));
        }
        out
    }
}

impl ReadingSource for Reading {
    fn id(&self) -> [u8; 20] {
        self.idx.sha1_tc
    }

    fn dir(&self) -> &Path {
        &self.dir
    }

    fn head(&self) -> &str {
        &self.head
    }

    fn packed(&self) -> &[u8] {
        self.packed.map.bytes()
    }

    fn n_actions(&self) -> u32 {
        self.packed.n_actions
    }

    fn action_line(&self, i: u32) -> &str {
        let (a, z) = (self.idx.action_off(i) as usize, self.idx.action_off(i + 1) as usize);
        let b = &self.actions.bytes()[a..z];
        let b = b.strip_suffix(b"\n").unwrap_or(b);
        std::str::from_utf8(b).expect("an action line in UTF-8")
    }

    fn by_name(&self, name: &str) -> Option<u32> {
        let h = idx::fnv1a64(name.as_bytes());
        self.idx.with_hash(h).find(|&a| self.packed.has_name(self.idx.tc_word(a), name))
    }

    fn n_lists(&self) -> u32 {
        self.idx.n_lists
    }

    fn list_line(&self, i: u32) -> &str {
        let (a, z) = (self.idx.list_off(i) as usize, self.idx.list_off(i + 1) as usize);
        let b = &self.lists.as_ref().expect("a reading with lists").bytes()[a..z];
        let b = b.strip_suffix(b"\n").unwrap_or(b);
        std::str::from_utf8(b).expect("a list line in UTF-8")
    }

    fn reached(&self) -> &str {
        &self.reached
    }

    fn domains(&self) -> &Domains {
        &self.domains
    }

    fn goal(&self) -> &[(Key, Vec<Val>)] {
        &self.packed.goal
    }

    fn start(&self) -> &Facts {
        &self.start
    }

    fn action(&self, i: u32) -> Action {
        let raw = self.packed.raw(self.idx.tc_word(i));
        let (li, lrec, llabel) = line::head(self.action_line(i)).unwrap_or_else(|e| panic!("action {i}: its line: {e}"));
        self.check(i, &raw, li, lrec, &llabel);
        let name = self.name_of(&raw);
        let cost = raw.cost as i64;
        let effects = raw
            .effects
            .into_iter()
            .map(|e| Effect {
                key: self.packed.fact_keys[e.fact as usize],
                v: match e.payload {
                    Payload::Set(v) => EffVal::Value(v),
                    Payload::Guess(vs) => EffVal::Values(tc::sorted(vs)),
                    Payload::Add(d) => EffVal::Add(d),
                },
                when: self.sets(e.when),
            })
            .collect();
        Action { name, pre: self.pre_dict(raw.pre), effects, what: i, cost }
    }

    fn record(&self, i: u32) -> Record {
        let raw = self.packed.raw(self.idx.tc_word(i));
        let ln = self.checked_line(i, &raw);
        // a key of the line as the action's precondition holds it (the last set of that key, as the dict keeps it):
        // matched by text against the packed file's fact names, so no key text is made again
        let in_pre = |k: &str| raw.pre.iter().rev().find(|x| self.packed.s(self.packed.fact_sid[x.0 as usize]) == k);
        let rec_pre = ln
            .pre_keys
            .iter()
            .filter_map(|k| in_pre(k).map(|(f, v)| (self.packed.fact_keys[*f as usize], tc::sorted(v.clone()))))
            .collect();
        let engine_pre =
            ln.engine_pre.iter().map(|k| in_pre(k).map_or_else(|| Key::parse(k), |x| self.packed.fact_keys[x.0 as usize])).collect();
        Record {
            kind: self.packed.s(raw.kind).to_string(),
            label: self.packed.s(raw.label).to_string(),
            pre: rec_pre,
            effects: self.effects_of(&raw),
            checks: ln.checks.iter().map(|c| c.to_string()).collect(),
            end: self.packed.s(raw.end).to_string(),
            source: ln.source.to_string(),
            engine_pre,
        }
    }
}
