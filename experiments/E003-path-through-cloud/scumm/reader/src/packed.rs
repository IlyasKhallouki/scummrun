//! taskcore's packed task format (taskcore/README.md, version 1), written streaming: export_records.py's `Writer`,
//! `export_records` and `export_task` in Rust, and [`RecordStream`], which takes records as the Builder makes them.
//!
//! The body (one action per record) goes first, the tables (strings, facts, add tables, goal, starts) and the footer
//! last, so the writer keeps only the string and fact tables, the add tables' keys and, for build_task's fold of
//! identical records, a 16-byte hash per distinct record ([`RecordStream`]: plus the record's `args:` values, for
//! the fold after SIG_LIMIT's rewrite).

use crate::builder::Builder;
use crate::domain::{Domain, Label, Labels};
use crate::facts::Facts;
use crate::py::Counter;
use crate::record::Record;
use crate::table::FactTable;
use crate::costs::FrameCosts;
use crate::task::{fold_hash_cost, fold_parts_cost, record_action, FoldParts, TEffect, Task};
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::BTreeSet;
use std::fs::File;
use std::io::{self, BufWriter, Seek, SeekFrom, Write};
use std::path::Path;

pub const MAGIC: u32 = 0x524F4354;
pub const END_MAGIC: u32 = 0x444E4554;
pub const VERSION: u32 = 1;
pub const NONE: u32 = 0xFFFF_FFFF;
pub const OP_SET: u32 = 0;
pub const OP_GUESS: u32 = 1;
pub const OP_ADD: u32 = 2;

/// export_records.py `Writer`.
pub struct Writer {
    f: BufWriter<File>,
    strings: FxHashMap<String, u32>,
    slist: Vec<String>,
    facts: FxHashMap<String, u32>,
    flist: Vec<String>,
    /// a value's string id, by value
    labels: FxHashMap<Label, u32>,
    /// (fact id, d) -> table id, in first-sight order
    adds: Vec<(u32, i64)>,
    add_index: FxHashMap<(u32, i64), u32>,
    pub n_actions: u32,
    pub words: u64,
    body_start: u64,
}

/// A precondition or condition entry: (fact key, values).
pub type SetEntry<'a> = (&'a str, &'a [Label]);

impl Writer {
    pub fn create(path: &Path) -> io::Result<Writer> {
        let f = BufWriter::with_capacity(1 << 20, File::create(path)?);
        let mut w = Writer {
            f,
            strings: FxHashMap::default(),
            slist: Vec::new(),
            facts: FxHashMap::default(),
            flist: Vec::new(),
            labels: FxHashMap::default(),
            adds: Vec::new(),
            add_index: FxHashMap::default(),
            n_actions: 0,
            words: 0,
            body_start: 0,
        };
        w.put(&[MAGIC, VERSION])?;
        w.body_start = w.words;
        Ok(w)
    }

    fn put(&mut self, words: &[u32]) -> io::Result<()> {
        for &x in words {
            self.f.write_all(&x.to_le_bytes())?;
        }
        self.words += words.len() as u64;
        Ok(())
    }

    pub fn s(&mut self, text: &str) -> u32 {
        if let Some(&i) = self.strings.get(text) {
            return i;
        }
        let i = self.slist.len() as u32;
        self.strings.insert(text.to_string(), i);
        self.slist.push(text.to_string());
        i
    }

    /// The string id of a value's text (`str(v)`).
    pub fn sl(&mut self, l: Label) -> u32 {
        if let Some(&i) = self.labels.get(&l) {
            return i;
        }
        let i = self.s(&l.text());
        self.labels.insert(l, i);
        i
    }

    pub fn fact(&mut self, key: &str) -> u32 {
        if let Some(&i) = self.facts.get(key) {
            return i;
        }
        let i = self.flist.len() as u32;
        self.facts.insert(key.to_string(), i);
        self.flist.push(key.to_string());
        i
    }

    fn set_entry(&mut self, out: &mut Vec<u32>, key: &str, vals: &[Label]) {
        out.push(self.fact(key));
        out.push(vals.len() as u32);
        for &v in vals {
            let s = self.sl(v);
            out.push(s);
        }
    }

    /// One action (export_records.py `Writer.action`): `effects` as task.py Effects, each value labelled against its
    /// fact's domain by `lab` (`dom.label(v) if isinstance(v, int) else v`, a value outside the domain kept).
    #[allow(clippy::too_many_arguments)]
    pub fn action(
        &mut self,
        rec: u32,
        label: &str,
        kind: &str,
        end: &str,
        cont: &str,
        cost: u32,
        pre: &[(String, Labels)],
        effects: &[TEffect],
        lab: &dyn Fn(&str, Label) -> Label,
    ) -> io::Result<()> {
        let mut out: Vec<u32> = vec![rec];
        out.push(self.s(label));
        out.push(self.s(kind));
        out.push(self.s(end));
        out.push(self.s(cont));
        out.push(cost);
        out.push(pre.len() as u32);
        for (k, vals) in pre {
            self.set_entry(&mut out, k, vals);
        }
        out.push(effects.len() as u32);
        for e in effects {
            if let Some(v) = e.value {
                out.push(OP_SET);
                out.push(self.fact(&e.key));
                let l = lab(&e.key, v);
                out.push(self.sl(l));
            } else if let Some(vs) = &e.values {
                out.push(OP_GUESS);
                out.push(self.fact(&e.key));
                let mut ids: Vec<u32> = Vec::new();
                for &v in vs {
                    let l = lab(&e.key, v);
                    let s = self.sl(l);
                    if !ids.contains(&s) {
                        ids.push(s);
                    }
                }
                out.push(ids.len() as u32);
                out.extend(ids);
            } else {
                let step = e.add.unwrap();
                out.push(OP_ADD);
                let fid = self.fact(&e.key);
                out.push(fid);
                out.push(step as u32);
                if !self.add_index.contains_key(&(fid, step)) {
                    self.add_index.insert((fid, step), self.adds.len() as u32);
                    self.adds.push((fid, step));
                }
            }
            out.push(e.when.len() as u32);
            for (wk, wv) in &e.when {
                self.set_entry(&mut out, wk, wv);
            }
        }
        self.put(&[out.len() as u32])?;
        self.put(&out)?;
        self.n_actions += 1;
        Ok(())
    }

    /// The tables and the footer (export_records.py `Writer.close`): `goal` {key: values}, `starts` (name, facts).
    pub fn close(mut self, goal: &[(String, Labels)], starts: &[(String, Facts)], dom: &dyn Fn(&str) -> Domain) -> io::Result<WriterInfo> {
        for (k, _) in goal {
            self.fact(k);
        }
        let doms: Vec<Domain> = self.flist.iter().map(|k| dom(k)).collect();
        let mut facts_arr = vec![self.flist.len() as u32];
        for (k, d) in self.flist.clone().iter().zip(&doms) {
            facts_arr.push(self.s(k));
            facts_arr.push(d.values.len() as u32);
            for v in &d.values {
                facts_arr.push(self.s(&v.text()));
            }
        }
        let mut adds_arr = vec![self.adds.len() as u32];
        for (fid, d) in self.adds.clone() {
            let dm = &doms[fid as usize];
            adds_arr.extend([fid, d as u32, dm.values.len() as u32]);
            for &u in &dm.values {
                let nxt = dm.add(u, d);
                adds_arr.push(nxt.len() as u32);
                for v in nxt {
                    adds_arr.push(self.s(&v.text()));
                }
            }
        }
        let mut goal_arr = vec![goal.len() as u32];
        for (k, vals) in goal {
            let fid = self.facts[k.as_str()];
            let dm = &doms[fid as usize];
            let mut labs: Vec<Label> = Vec::new();
            for &v in vals {
                if let Some(l) = dm.label_of(v) {
                    if !labs.contains(&l) {
                        labs.push(l);
                    }
                }
            }
            goal_arr.extend([fid, labs.len() as u32]);
            for l in labs {
                goal_arr.push(self.s(&l.text()));
            }
        }
        let mut starts_arr = vec![starts.len() as u32];
        for (name, st) in starts {
            starts_arr.push(self.s(name));
            for (k, d) in self.flist.clone().iter().zip(&doms) {
                let v = st.get0(k);
                starts_arr.push(match d.label(v) {
                    Some(l) if d.contains(l) => self.s(&l.text()),
                    _ => NONE,
                });
            }
        }
        let blob: Vec<u8> = self.slist.iter().flat_map(|t| t.as_bytes().iter().copied()).collect();
        let mut strings_arr = vec![self.slist.len() as u32];
        strings_arr.extend(self.slist.iter().map(|t| t.len() as u32));
        let off_strings = self.words;
        self.put(&strings_arr)?;
        let pad = (4 - blob.len() % 4) % 4;
        self.f.write_all(&blob)?;
        self.f.write_all(&vec![0u8; pad])?;
        self.words += ((blob.len() + pad) / 4) as u64;
        let mut offs = Vec::new();
        for arr in [&facts_arr, &adds_arr, &goal_arr, &starts_arr] {
            offs.push(self.words);
            self.put(arr)?;
        }
        let footer = [
            self.body_start as u32,
            self.n_actions,
            off_strings as u32,
            offs[0] as u32,
            offs[1] as u32,
            offs[2] as u32,
            offs[3] as u32,
            END_MAGIC,
        ];
        self.put(&footer)?;
        self.f.flush()?;
        Ok(WriterInfo { actions: self.n_actions, facts: self.flist.len(), strings: self.slist.len(), bytes: self.words * 4, zeroed: 0 })
    }
}

#[derive(Clone, Debug)]
pub struct WriterInfo {
    pub actions: u32,
    pub facts: usize,
    pub strings: usize,
    pub bytes: u64,
    /// `RecordStream`: the `args:` values (sets and preconditions) of folded lists set to 0 after they were written
    pub zeroed: u64,
}

/// export_records.py `_cont`: the events a record's path leaves, by unit, wait point and Esc, sorted.
pub fn cont(r: &Record) -> String {
    let mut evs: Vec<(&str, i64, bool)> = r.next_events.iter().map(|e| (e.name.as_str(), e.w, e.abort)).collect();
    evs.sort();
    evs.iter().map(|(n, w, ab)| format!("{n}@{w}{}", if *ab { "!" } else { "" })).collect::<Vec<_>>().join(";")
}

/// The table's domains, each worked out once (an `args:` domain is built from its list numbers on every call).
fn table_dom(table: &FactTable) -> impl Fn(&str) -> Domain + '_ {
    let memo: std::cell::RefCell<FxHashMap<String, Domain>> = Default::default();
    move |k: &str| {
        if let Some(d) = memo.borrow().get(k) {
            return d.clone();
        }
        let d = table.domain(k).unwrap_or_else(|| panic!("no domain for {k}")).into_owned();
        memo.borrow_mut().insert(k.to_string(), d.clone());
        d
    }
}

/// export_records.py `export_records`: build_task's task from `records`, one record at a time (no Task, no action
/// list), records of one label with the same precondition and effects folded into the first by a 16-byte hash.
pub fn export_records(
    path: &Path,
    b: &mut Builder,
    records: &[Record],
    goal: &[(String, Labels)],
    starts: &[(String, Facts)],
) -> io::Result<(WriterInfo, usize)> {
    let mut w = Writer::create(path)?;
    for (k, _) in goal {
        w.fact(k);
    }
    let mut seen: FxHashSet<[u8; 16]> = FxHashSet::default();
    let mut folded = 0;
    let mut counts = Counter::default();
    let costs = b.frame_costs.clone();
    {
        let dom = table_dom(&b.table);
        for (i, r) in records.iter().enumerate() {
            let cost = costs.as_ref().map(|fc| fc.cost(r));
            if !seen.insert(fold_hash_cost(r, cost)) {
                folded += 1;
                continue;
            }
            let (pre, effects) = record_action(&b.table, r, &mut counts);
            w.action(i as u32, &r.label, &r.kind, r.end_text(), &cont(r), cost.unwrap_or(1), &pre, &effects, &|k, v| dom(k).label_of(v).unwrap_or(v))?;
        }
    }
    let info = w.close(goal, starts, &table_dom(&b.table))?;
    b.counts.update(&counts);
    Ok((info, folded))
}

/// export_records.py `export_task`: a task.Task already built; `meta(action)` gives (kind, end, cont).
pub fn export_task(
    path: &Path,
    task: &Task,
    starts: &[(String, Facts)],
    meta: &dyn Fn(usize) -> (String, String, String),
) -> io::Result<WriterInfo> {
    let mut w = Writer::create(path)?;
    for (k, _) in &task.goal {
        w.fact(k);
    }
    let dom = |k: &str| task.fact(k).clone();
    for a in &task.actions {
        let (kind, end, c) = meta(a.what);
        w.action(NONE, &a.name, &kind, &end, &c, a.cost as u32, &a.pre, &a.effects, &|k, v| dom(k).label_of(v).unwrap_or(v))?;
    }
    w.close(&task.goal, starts, &dom)
}

impl crate::builder::RecordSink for RecordStream {
    fn batch(&mut self, first: usize, recs: &[Record], table: &FactTable) {
        self.records(first, recs, table).expect("the packed file")
    }
    fn reset(&mut self) {
        RecordStream::reset(self).expect("the packed file")
    }
}

/// build_task's actions written as the Builder makes the records ("written as made", taskcore/README.md's stage
/// 2): `records` after each batch of runs, `reset` when a pass starts again, `finish` at the end.
///
/// A list of locals SIG_LIMIT folds into 0 after its records were written (`_read_once`'s rewrite: an `args:` set
/// to it sets 0, an `args:` precondition with it is {0}) can make records identical that were not when written, and
/// build_task folds those. So each written record keeps its fold key in two parts ([`FoldParts`]: a 16-byte hash
/// with the `args:` values marked, and those values); `finish` works out each key after the rewrite, and when a
/// record folds or a value changes, writes the body again in one streaming pass: the later identical records
/// dropped, the folded lists' values 0, the tables copied.
pub struct RecordStream {
    path: std::path::PathBuf,
    w: Option<Writer>,
    seen: FxHashSet<[u8; 16]>,
    /// records folded when written (identical before the rewrite)
    folded_written: usize,
    /// per written action: its fold key's parts and its count of steps that need room
    written: Vec<(FoldParts, u32)>,
    /// the bounds of each fact's domain (`low`, `high`), which alone decide how a value is labelled: kept for the
    /// pass (an `args:`, `pos:` or `escpos:` domain grows within a pass, but never has bounds)
    bounds: FxHashMap<String, (Option<i64>, Option<i64>)>,
    /// the switch `--frame-costs` (costs.rs): each record's cost, in its fold key too; None: every record costs 1
    pub costs: Option<std::sync::Arc<FrameCosts>>,
}

const STEP_ROOM: &str = "steps that need room in their domain (a precondition)";

impl RecordStream {
    pub fn create(path: &Path) -> io::Result<RecordStream> {
        Ok(RecordStream {
            path: path.to_path_buf(),
            w: Some(Writer::create(path)?),
            seen: FxHashSet::default(),
            folded_written: 0,
            written: Vec::new(),
            bounds: FxHashMap::default(),
            costs: None,
        })
    }

    /// Records folded so far because they were identical when written (`finish` adds those the rewrite makes
    /// identical).
    pub fn folded_when_written(&self) -> usize {
        self.folded_written
    }

    /// The facts the written records name so far (with the goal's once `finish` adds them): the keys `task_start`
    /// needs (`task::record_keys` of the records).
    pub fn fact_keys(&self) -> Vec<String> {
        self.w.as_ref().map_or_else(Vec::new, |w| w.flist.clone())
    }

    /// Actions written so far.
    pub fn actions(&self) -> u32 {
        self.w.as_ref().map_or(0, |w| w.n_actions)
    }

    /// A new pass: what was written is dropped.
    pub fn reset(&mut self) -> io::Result<()> {
        self.w = Some(Writer::create(&self.path)?);
        self.seen.clear();
        self.folded_written = 0;
        self.written.clear();
        self.bounds.clear();
        Ok(())
    }

    /// Records as made, `first` the index of the first.
    pub fn records(&mut self, first: usize, recs: &[Record], table: &FactTable) -> io::Result<()> {
        let w = self.w.as_mut().unwrap();
        let none = BTreeSet::new();
        for r in recs {
            for (k, _) in &r.effects {
                if !self.bounds.contains_key(k) {
                    let b = if k.starts_with("args:") || crate::table::pos_key(k).is_some() {
                        (None, None)
                    } else {
                        let d = table.domain(k).unwrap_or_else(|| panic!("no domain for {k}"));
                        (d.low, d.high)
                    };
                    self.bounds.insert(k.clone(), b);
                }
            }
        }
        let bounds = &self.bounds;
        // `Domain.label` with the domain's bounds: a value past them is collapsed, any other stays as it is
        let lab = |k: &str, v: Label| match (v, bounds[k]) {
            (Label::Int(n), (_, Some(h))) if n > h => Label::Above,
            (Label::Int(n), (Some(l), _)) if n < l => Label::Below,
            _ => v,
        };
        let mut counts = Counter::default();
        for (j, r) in recs.iter().enumerate() {
            let cost = self.costs.as_ref().map(|fc| fc.cost(r));
            let parts = fold_parts_cost(r, cost);
            if !self.seen.insert(parts.hash_after(&none)) {
                self.folded_written += 1;
                continue;
            }
            counts.clear();
            let (pre, effects) = record_action(table, r, &mut counts);
            w.action((first + j) as u32, &r.label, &r.kind, r.end_text(), &cont(r), cost.unwrap_or(1), &pre, &effects, &lab)?;
            self.written.push((parts, counts.get(STEP_ROOM) as u32));
        }
        Ok(())
    }

    /// The tables and the footer; then the fold and the values of lists folded since their records were written.
    /// Returns the file's info and the records folded (as build_task's `folded`).
    pub fn finish(mut self, b: &mut Builder, goal: &[(String, Labels)], starts: &[(String, Facts)]) -> io::Result<(WriterInfo, usize)> {
        let mut w = self.w.take().unwrap();
        let zero = w.s("0");
        let mut seen: FxHashSet<[u8; 16]> = FxHashSet::default();
        let keep: Vec<bool> = self.written.iter().map(|(p, _)| seen.insert(p.hash_after(&b.folded))).collect();
        let steps: u32 = self.written.iter().zip(&keep).filter(|(_, k)| **k).map(|((_, n), _)| *n).sum();
        let dropped = keep.iter().filter(|k| !**k).count();
        // string ids of the folded lists' values, fact ids of the args: keys (texts as Writer.sl gives them)
        let folded_ids: FxHashSet<u32> =
            b.folded.iter().filter_map(|&x| w.strings.get(&Label::Int(x).text()).copied()).collect();
        let args_facts: FxHashSet<u32> =
            w.flist.iter().enumerate().filter(|(_, k)| k.starts_with("args:")).map(|(i, _)| i as u32).collect();
        let mut info = w.close(goal, starts, &table_dom(&b.table))?;
        if dropped > 0 || !folded_ids.is_empty() {
            info = rewrite_body(&self.path, info, &keep, &args_facts, &folded_ids, zero)?;
        }
        if steps > 0 {
            b.counts.add(STEP_ROOM, steps as i64);
        }
        Ok((info, self.folded_written + dropped))
    }
}

fn read_words(r: &mut impl io::Read, n: usize, buf: &mut Vec<u32>) -> io::Result<()> {
    let mut bytes = vec![0u8; n * 4];
    r.read_exact(&mut bytes)?;
    buf.clear();
    buf.extend(bytes.chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])));
    Ok(())
}

/// One action's words with the folded lists' values 0: an `args:` precondition with one in it is {0}, an `args:` set
/// to one sets 0 (the layout of [`Writer::action`]).
fn rewrite_action(a: &[u32], args_facts: &FxHashSet<u32>, folded: &FxHashSet<u32>, zero: u32, zeroed: &mut u64) -> Vec<u32> {
    let mut out: Vec<u32> = a[..6].to_vec();
    let mut i = 6;
    let mut entry = |i: &mut usize, out: &mut Vec<u32>| {
        let (fid, n) = (a[*i], a[*i + 1] as usize);
        let vals = &a[*i + 2..*i + 2 + n];
        if args_facts.contains(&fid) && vals.iter().any(|v| folded.contains(v)) {
            out.extend([fid, 1, zero]);
            *zeroed += 1;
        } else {
            out.extend_from_slice(&a[*i..*i + 2 + n]);
        }
        *i += 2 + n;
    };
    let npre = a[i] as usize;
    out.push(a[i]);
    i += 1;
    for _ in 0..npre {
        entry(&mut i, &mut out);
    }
    let neff = a[i] as usize;
    out.push(a[i]);
    i += 1;
    for _ in 0..neff {
        let (op, fid) = (a[i], a[i + 1]);
        out.extend([op, fid]);
        i += 2;
        match op {
            OP_SET => {
                let v = a[i];
                if args_facts.contains(&fid) && folded.contains(&v) {
                    out.push(zero);
                    *zeroed += 1;
                } else {
                    out.push(v);
                }
                i += 1;
            }
            OP_GUESS => {
                let n = a[i] as usize;
                out.extend_from_slice(&a[i..i + 1 + n]);
                i += 1 + n;
            }
            _ => {
                out.push(a[i]);
                i += 1;
            }
        }
        let nwhen = a[i] as usize;
        out.push(a[i]);
        i += 1;
        for _ in 0..nwhen {
            // conditions are copied: the rewrite does not reach them
            let n = a[i + 1] as usize;
            out.extend_from_slice(&a[i..i + 2 + n]);
            i += 2 + n;
        }
    }
    assert_eq!(i, a.len(), "an action's words");
    out
}

/// The packed file at `path` again, streaming: the actions `keep` marks, rewritten by [`rewrite_action`]; the
/// tables as they were; the footer's count and offsets moved.
fn rewrite_body(path: &Path, info: WriterInfo, keep: &[bool], args_facts: &FxHashSet<u32>, folded: &FxHashSet<u32>, zero: u32) -> io::Result<WriterInfo> {
    use std::io::{BufReader, Read};
    let mut f = File::open(path)?;
    let total = f.metadata()?.len() / 4;
    f.seek(SeekFrom::Start((total - 8) * 4))?;
    let mut footer = Vec::new();
    read_words(&mut f, 8, &mut footer)?;
    assert_eq!(footer[7], END_MAGIC);
    let (body_start, n_actions, off_strings) = (footer[0] as u64, footer[1] as usize, footer[2] as u64);
    assert_eq!(n_actions, keep.len(), "one written action per kept fold key");
    f.seek(SeekFrom::Start(0))?;
    let mut r = BufReader::with_capacity(1 << 20, f);
    let tmp = path.with_extension("rewrite");
    let mut out = BufWriter::with_capacity(1 << 20, File::create(&tmp)?);
    let mut buf = Vec::new();
    read_words(&mut r, body_start as usize, &mut buf)?;
    let mut words = body_start;
    let put = |out: &mut BufWriter<File>, ws: &[u32]| -> io::Result<()> {
        for x in ws {
            out.write_all(&x.to_le_bytes())?;
        }
        Ok(())
    };
    put(&mut out, &buf)?;
    let mut kept = 0u32;
    let mut zeroed = 0u64;
    let mut at = body_start;
    for &k in keep {
        read_words(&mut r, 1, &mut buf)?;
        let len = buf[0] as usize;
        read_words(&mut r, len, &mut buf)?;
        at += 1 + len as u64;
        if !k {
            continue;
        }
        let a = rewrite_action(&buf, args_facts, folded, zero, &mut zeroed);
        put(&mut out, &[a.len() as u32])?;
        put(&mut out, &a)?;
        words += 1 + a.len() as u64;
        kept += 1;
    }
    assert_eq!(at, off_strings, "the body ends where the tables start");
    let shift = off_strings as i64 - words as i64;
    // the tables, as they were
    let mut rest = r.take((total - 8 - off_strings) * 4);
    let copied = io::copy(&mut rest, &mut out)?;
    assert_eq!(copied, (total - 8 - off_strings) * 4);
    let mut nf = footer.clone();
    nf[1] = kept;
    for x in &mut nf[2..7] {
        *x = (*x as i64 - shift) as u32;
    }
    put(&mut out, &nf)?;
    out.flush()?;
    drop(out);
    std::fs::rename(&tmp, path)?;
    Ok(WriterInfo { actions: kept, bytes: std::fs::metadata(path)?.len(), zeroed, ..info })
}
