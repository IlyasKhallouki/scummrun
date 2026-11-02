//! E011's reading files, written by reader-scope as it reads (`--out-dir DIR`; experiments/E011-speed/plan.md,
//! component readio): the files replay_part1.py's `read_rust` (with `stream`) makes from the golden stream with
//! packed_reading.py's `reading_from_golden`, byte for byte, without the golden stream and without Python:
//!
//! * `reading.actions.jsonl`: per action of the packed file, `line_of_golden`'s line of the record it names, as the
//!   records are made (one line per record the packed file writes); the records the packed file folds at its end
//!   (`RecordStream::finish_keep`) drop their lines, and the lines are numbered again;
//! * `reading.lists.jsonl`: `lists_of` over the records in the order they are made, each (name, w, sig) once, less
//!   the lists the last pass folded (`read_rust`'s rule for a streamed reading), and the slots' own lists
//!   (`slot_events`) that are not among them;
//! * `reading.json`: `reading_from_golden`'s head with `read_rust`'s info and its additions (`n_lists`,
//!   `slot_lists`); the bounds are the reader's own domains' (`FactTable::domain`), checked as `reading_from_golden`
//!   checks Python's;
//! * `reading.idx`: E011's index (plan.md, F3): the line offsets of both line files, the word offset of each
//!   action in the packed file, the actions by the FNV-1a 64 hash of their names, and the three files' sha1.
//!
//! The packed file (`reading.tc`) and the reached values (`reading.reached`) are reader-scope's own, as before.

use super::pyjson::PJ;
use rustc_hash::FxHashSet;
use scumm_reader::domain::Label;
use scumm_reader::py::Py;
use scumm_reader::record::{Eff, Record, Val, What};
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

pub const FORMAT: &str = "E003 reading 1"; // packed_reading.FORMAT
pub const PACKED: &str = "reading.tc";
pub const ACTIONS: &str = "reading.actions.jsonl";
pub const LISTS: &str = "reading.lists.jsonl";
pub const HEAD: &str = "reading.json";
pub const IDX: &str = "reading.idx";
pub const IDX_MAGIC: &[u8; 4] = b"FMRX";
pub const IDX_VERSION: u32 = 1;
const TC_MAGIC: u32 = 0x524F4354;
const TC_END: u32 = 0x444E4554;
const TC_NONE: u32 = 0xFFFF_FFFF;

/// The line files as the records come (the sink's thread), and what `finish` needs of them.
pub struct Files {
    pub dir: PathBuf,
    /// `--live-locals`: the list lines carry the slot flags (`lists_of(..., flags)`)
    flags: bool,
    actions: BufWriter<File>,
    /// byte offset of each written action's line, and the end of the last
    act_off: Vec<u64>,
    act_end: u64,
    act_sha: sha1_smol::Sha1,
    /// the list lines, each (name, w, sig) once, in the order the records made them
    lists: Vec<((String, i64, i64), String)>,
    seen: FxHashSet<(String, i64, i64)>,
    /// records of the pass so far (the golden stream's records)
    pub n_records: usize,
    /// seconds in `batch` (the sink's thread): wall, and the thread's CPU
    pub busy: f64,
    pub busy_cpu: f64,
    line: String,
}

/// Python's `str()` of a decoded golden value that is not a bool or None (`_flag_json`'s text).
fn py_str(p: &Py) -> String {
    match p {
        Py::Int(n) => n.to_string(),
        Py::Str(s) => s.clone(),
        Py::Obj(_, f) => {
            let get = |k: &str| f.iter().find(|x| x.0 == k).and_then(|x| match &x.1 {
                Py::Str(s) if !s.is_empty() => Some(s.clone()),
                _ => None,
            });
            get("what").or_else(|| get("reason")).unwrap_or_else(|| panic!("a slot flag with no text: {}", p.text()))
        }
        other => panic!("a slot flag reader.py never makes: {}", other.text()),
    }
}

/// packed_reading.py `_flag_json`.
fn flag_json(p: &Py) -> PJ {
    match p {
        Py::None => PJ::Null,
        Py::Bool(b) => PJ::Bool(*b),
        other => PJ::obj(vec![("unknown", PJ::Str(py_str(other)))]),
    }
}

/// packed_reading.py `_local_json`.
fn local_json(v: &Val) -> PJ {
    match v {
        Val::Int(n) => PJ::Int(*n),
        Val::Unk(u) => {
            let text = if !u.what.is_empty() { &u.what } else { &u.reason };
            PJ::obj(vec![
                ("unknown", PJ::str(text.clone())),
                ("values", u.values.as_ref().map_or(PJ::Null, |v| PJ::ints(v.iter().copied()))),
            ])
        }
    }
}

/// `json.dumps` of an assumption's side (a bool in every record so far).
fn side_json(p: &Py, out: &mut String) {
    PJ::from_py(p).compact(out)
}

/// packed_reading.py `line_of_golden(i, rec, r)` as `json.dumps` writes it, with its newline.
pub fn action_line(out: &mut String, i: usize, rec: usize, r: &Record) {
    use scumm_ir::json::write_str;
    let _ = write!(out, "{{\"i\": {i}, \"rec\": {rec}, \"label\": ");
    write_str(&r.label, out);
    out.push_str(", \"source\": ");
    match &r.source {
        What::Sentence { verb, a, b } => {
            let _ = write!(out, "[\"sentence\", {verb}, {a}, ");
            match b {
                Some(b) => {
                    let _ = write!(out, "{b}]");
                }
                None => out.push_str("null]"),
            }
        }
        What::Verb(v) => {
            let _ = write!(out, "[\"verb\", {v}]");
        }
        What::Event(e) => {
            out.push_str("[\"event\", ");
            write_str(&e.name, out);
            let _ = write!(out, ", {}, {}, {}]", e.w, e.sig, if e.abort { "true" } else { "false" });
        }
    }
    out.push_str(", \"checks\": [");
    for (n, c) in r.checks.iter().enumerate() {
        if n > 0 {
            out.push_str(", ");
        }
        out.push('[');
        write_str(&c.cite, out);
        out.push_str(", ");
        write_str(&c.cond, out);
        out.push_str(", ");
        side_json(&c.side, out);
        out.push_str(", ");
        write_str(&c.why, out);
        out.push(']');
    }
    out.push_str("], \"pre_keys\": [");
    for (n, k) in r.pre.keys().enumerate() {
        if n > 0 {
            out.push_str(", ");
        }
        write_str(k, out);
    }
    out.push_str("], \"engine_pre\": [");
    let mut ep: Vec<&String> = r.engine_pre.iter().collect();
    ep.sort();
    for (n, k) in ep.into_iter().enumerate() {
        if n > 0 {
            out.push_str(", ");
        }
        write_str(k, out);
    }
    out.push_str("]}\n");
}

/// packed_reading.py `lists_of(next_events, effects, flags)` of one record, less the lists `seen` holds (each
/// (name, w, sig) once, as `_ListWriter` writes them): (its (name, w, sig), its line) for each new one, which goes
/// into `seen`.
pub fn list_lines(
    r: &Record,
    flags: bool,
    seen: &mut FxHashSet<(String, i64, i64)>,
    out: &mut Vec<((String, i64, i64), String)>,
) {
    let mut key = String::new();
    for e in &r.next_events {
        if e.sig == 0 || e.abort {
            continue;
        }
        key.clear();
        key.push_str("args:");
        key.push_str(&e.name);
        match r.effect(&key) {
            Some(Eff::Set(Label::Int(v))) if *v == e.sig => {}
            _ => continue,
        }
        let k = (e.name.clone(), e.w, e.sig);
        if seen.contains(&k) {
            continue;
        }
        seen.insert(k.clone());
        let mut locals: Vec<&(i64, Val)> = e.locals.iter().collect();
        locals.sort_by_key(|x| x.0);
        let mut ln = PJ::obj(vec![
            ("name", PJ::str(e.name.clone())),
            ("w", PJ::Int(e.w)),
            ("sig", PJ::Int(e.sig)),
            ("locals", PJ::List(locals.into_iter().map(|(n, v)| PJ::List(vec![PJ::Int(*n), local_json(v)])).collect())),
            ("override", PJ::Int(e.override_)),
            ("cut_data", PJ::ints(e.cut_data.iter().copied())),
        ]);
        if flags {
            ln.set(
                "flags",
                PJ::obj(vec![
                    ("recursive", flag_json(&e.recursive)),
                    ("resistant", flag_json(&e.resistant)),
                    ("owned", flag_json(&e.owned)),
                ]),
            );
        }
        out.push((k, ln.compact_text()));
    }
}

/// CPU seconds of the calling thread so far (/proc/thread-self/schedstat: its run time in ns); 0 where unknown.
pub fn thread_cpu() -> f64 {
    std::fs::read_to_string("/proc/thread-self/schedstat")
        .ok()
        .and_then(|t| t.split_whitespace().next().and_then(|x| x.parse::<u64>().ok()))
        .map_or(0.0, |ns| ns as f64 / 1e9)
}

fn sha1_hex(d: &[u8; 20]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

/// FNV-1a 64 of a name (F3's name index).
pub fn fnv1a64(s: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in s {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

impl Files {
    pub fn create(dir: &Path, flags: bool) -> io::Result<Files> {
        std::fs::create_dir_all(dir)?;
        Ok(Files {
            dir: dir.to_path_buf(),
            flags,
            actions: BufWriter::with_capacity(1 << 20, File::create(dir.join(ACTIONS))?),
            act_off: Vec::new(),
            act_end: 0,
            act_sha: sha1_smol::Sha1::new(),
            lists: Vec::new(),
            seen: FxHashSet::default(),
            n_records: 0,
            busy: 0.0,
            busy_cpu: 0.0,
            line: String::with_capacity(4096),
        })
    }

    /// A pass starts again: what was written goes.
    pub fn reset(&mut self) -> io::Result<()> {
        self.actions = BufWriter::with_capacity(1 << 20, File::create(self.dir.join(ACTIONS))?);
        self.act_off.clear();
        self.act_end = 0;
        self.act_sha = sha1_smol::Sha1::new();
        self.lists.clear();
        self.seen.clear();
        self.n_records = 0;
        Ok(())
    }

    /// A batch of records as made, `first` the index of the first; `written` the records of it the packed file
    /// wrote as actions (`RecordStream::records_with`), in order.
    pub fn batch(&mut self, first: usize, recs: &[Record], written: &[usize]) -> io::Result<()> {
        let t = Instant::now();
        let c = thread_cpu();
        for r in recs {
            if !r.next_events.is_empty() {
                list_lines(r, self.flags, &mut self.seen, &mut self.lists);
            }
        }
        for &j in written {
            self.line.clear();
            action_line(&mut self.line, self.act_off.len(), first + j, &recs[j]);
            self.act_off.push(self.act_end);
            self.act_end += self.line.len() as u64;
            self.act_sha.update(self.line.as_bytes());
            self.actions.write_all(self.line.as_bytes())?;
        }
        self.n_records += recs.len();
        self.busy += t.elapsed().as_secs_f64();
        self.busy_cpu += thread_cpu() - c;
        Ok(())
    }

    /// The line files made final (module docstring): the actions the packed file kept (`keep`, one per action
    /// written), numbered again when some went; the lists less the `folded` ones, then the `extra` lists (the
    /// report's `slot_events`) not among them.
    pub fn finish(mut self, keep: &[bool], folded: &BTreeSet<i64>, extra: &[PJ]) -> io::Result<Lines> {
        self.actions.flush()?;
        drop(self.actions);
        assert_eq!(keep.len(), self.act_off.len(), "one line per action the packed file wrote");
        let path = self.dir.join(ACTIONS);
        let (action_off, sha_actions) = if keep.iter().all(|k| *k) {
            let mut off = self.act_off;
            off.push(self.act_end);
            (off, self.act_sha.digest().bytes())
        } else {
            renumber(&path, keep)?
        };
        // the lists
        let mut have: FxHashSet<(String, i64, i64)> = FxHashSet::default();
        let mut text = String::new();
        let mut list_off = vec![0u64];
        for (k, line) in &self.lists {
            if folded.contains(&k.2) {
                continue;
            }
            have.insert(k.clone());
            text.push_str(line);
            text.push('\n');
            list_off.push(text.len() as u64);
        }
        for x in extra {
            let key = (
                x.get("name").and_then(|v| if let PJ::Str(s) = v { Some(s.clone()) } else { None }).expect("name"),
                x.get("w").and_then(|v| if let PJ::Int(n) = v { Some(*n) } else { None }).expect("w"),
                x.get("sig").and_then(|v| if let PJ::Int(n) = v { Some(*n) } else { None }).expect("sig"),
            );
            if have.contains(&key) {
                continue;
            }
            x.compact(&mut text);
            text.push('\n');
            list_off.push(text.len() as u64);
        }
        std::fs::write(self.dir.join(LISTS), text.as_bytes())?;
        let sha_lists = sha1_smol::Sha1::from(text.as_bytes()).digest().bytes();
        Ok(Lines { n_records: self.n_records, action_off, list_off, sha_actions, sha_lists, sink_s: self.busy, sink_cpu_s: self.busy_cpu })
    }
}

/// The actions file with only the kept lines, numbered again ("i"): their offsets and sha1.
fn renumber(path: &Path, keep: &[bool]) -> io::Result<(Vec<u64>, [u8; 20])> {
    let tmp = path.with_extension("jsonl.tmp");
    let mut r = BufReader::with_capacity(1 << 20, File::open(path)?);
    let mut w = BufWriter::with_capacity(1 << 20, File::create(&tmp)?);
    let mut sha = sha1_smol::Sha1::new();
    let mut off = vec![0u64];
    let mut buf = Vec::new();
    let mut n = 0usize;
    for &k in keep {
        buf.clear();
        r.read_until(b'\n', &mut buf)?;
        if !k {
            continue;
        }
        // `{"i": N, ` then the rest of the line
        let rest = buf.iter().position(|&c| c == b',').expect("an action line");
        let head = format!("{{\"i\": {n}");
        sha.update(head.as_bytes());
        sha.update(&buf[rest..]);
        w.write_all(head.as_bytes())?;
        w.write_all(&buf[rest..])?;
        off.push(off.last().unwrap() + (head.len() + buf.len() - rest) as u64);
        n += 1;
    }
    w.flush()?;
    drop(w);
    std::fs::rename(&tmp, path)?;
    Ok((off, sha.digest().bytes()))
}

/// The line files' offsets and sha1 (`Files::finish`).
pub struct Lines {
    pub n_records: usize,
    pub action_off: Vec<u64>,
    pub list_off: Vec<u64>,
    pub sha_actions: [u8; 20],
    pub sha_lists: [u8; 20],
    pub sink_s: f64,
    pub sink_cpu_s: f64,
}

/// What the packed file holds that the head and the index need (one streaming pass, which also gives its sha1).
pub struct Tc {
    pub sha1: [u8; 20],
    /// word offset of each action's length word
    pub words: Vec<u64>,
    /// each action's (rec, label string id)
    pub rec_label: Vec<(u32, u32)>,
    pub strings: Vec<String>,
    /// the facts in the file's order: (key, the values' texts)
    pub facts: Vec<(String, Vec<String>)>,
    /// the goal in the file's order: (key, the values' texts)
    pub goal: Vec<(String, Vec<String>)>,
}

impl Tc {
    pub fn sha1_hex(&self) -> String {
        sha1_hex(&self.sha1)
    }

    /// PackedTask's name of action i: "label#rec", or the label when rec is NONE.
    pub fn name(&self, i: usize) -> String {
        let (rec, label) = self.rec_label[i];
        let l = &self.strings[label as usize];
        if rec == TC_NONE {
            l.clone()
        } else {
            format!("{l}#{rec}")
        }
    }

    /// Reads the packed file (taskcore/README.md, version 1) once, front to back.
    pub fn read(path: &Path) -> io::Result<Tc> {
        let bad = |what: &str| io::Error::new(io::ErrorKind::InvalidData, format!("{}: {what}", path.display()));
        let mut f = File::open(path)?;
        let total = f.metadata()?.len();
        if total % 4 != 0 || total < 40 {
            return Err(bad("not a packed file"));
        }
        let nw = total / 4;
        f.seek(SeekFrom::Start(total - 32))?;
        let mut fb = [0u8; 32];
        f.read_exact(&mut fb)?;
        let footer: Vec<u32> = fb.chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        if footer[7] != TC_END {
            return Err(bad("bad footer"));
        }
        let (body, n, o_str) = (footer[0] as u64, footer[1] as usize, footer[2] as u64);
        let (o_facts, o_goal) = (footer[3] as u64, footer[5] as u64);
        f.seek(SeekFrom::Start(0))?;
        let mut r = BufReader::with_capacity(1 << 20, f);
        let mut sha = sha1_smol::Sha1::new();
        let mut bytes = vec![0u8; 0];
        let mut read_words = |r: &mut BufReader<File>, n: usize, bytes: &mut Vec<u8>| -> io::Result<()> {
            bytes.resize(n * 4, 0);
            r.read_exact(bytes)?;
            sha.update(bytes);
            Ok(())
        };
        let word = |b: &[u8], i: usize| u32::from_le_bytes([b[4 * i], b[4 * i + 1], b[4 * i + 2], b[4 * i + 3]]);
        read_words(&mut r, body as usize, &mut bytes)?;
        if word(&bytes, 0) != TC_MAGIC || word(&bytes, 1) != 1 {
            return Err(bad("not a taskcore file of version 1"));
        }
        let mut words = Vec::with_capacity(n);
        let mut rec_label = Vec::with_capacity(n);
        let mut at = body;
        for _ in 0..n {
            read_words(&mut r, 1, &mut bytes)?;
            let len = word(&bytes, 0) as usize;
            words.push(at);
            read_words(&mut r, len, &mut bytes)?;
            rec_label.push((word(&bytes, 0), word(&bytes, 1)));
            at += 1 + len as u64;
        }
        if at != o_str {
            return Err(bad("the actions do not end where the strings start"));
        }
        // the tables and the footer
        read_words(&mut r, (nw - o_str) as usize, &mut bytes)?;
        let t = &bytes;
        let ns = word(t, 0) as usize;
        let mut pos = (1 + ns) * 4;
        let mut strings = Vec::with_capacity(ns);
        for k in 0..ns {
            let len = word(t, 1 + k) as usize;
            strings.push(String::from_utf8(t[pos..pos + len].to_vec()).map_err(|_| bad("a string that is not UTF-8"))?);
            pos += len;
        }
        let at = |o: u64| (o - o_str) as usize;
        let mut p = at(o_facts);
        let nf = word(t, p) as usize;
        p += 1;
        let mut facts = Vec::with_capacity(nf);
        for _ in 0..nf {
            let (key, size) = (word(t, p) as usize, word(t, p + 1) as usize);
            let vals = (0..size).map(|k| strings[word(t, p + 2 + k) as usize].clone()).collect();
            facts.push((strings[key].clone(), vals));
            p += 2 + size;
        }
        let mut p = at(o_goal);
        let ng = word(t, p) as usize;
        let mut goal = Vec::with_capacity(ng);
        for _ in 0..ng {
            let (fid, m) = (word(t, p + 1) as usize, word(t, p + 2) as usize);
            let vals = (0..m).map(|k| strings[word(t, p + 3 + k) as usize].clone()).collect();
            goal.push((facts[fid].0.clone(), vals));
            p += 2 + m;
        }
        Ok(Tc { sha1: sha.digest().bytes(), words, rec_label, strings, facts, goal })
    }
}

/// `reading.idx` (plan.md, F3).
pub fn write_idx(path: &Path, tc: &Tc, lines: &Lines) -> io::Result<()> {
    let n = tc.words.len();
    assert_eq!(lines.action_off.len(), n + 1, "an action line per action");
    let mut b: Vec<u8> = Vec::with_capacity(16 + 8 * (3 * n + lines.list_off.len() + 1) + 12 * n + 64);
    b.extend_from_slice(IDX_MAGIC);
    b.extend_from_slice(&IDX_VERSION.to_le_bytes());
    b.extend_from_slice(&(n as u32).to_le_bytes());
    b.extend_from_slice(&((lines.list_off.len() - 1) as u32).to_le_bytes());
    for x in lines.action_off.iter().chain(&lines.list_off).chain(&tc.words) {
        b.extend_from_slice(&x.to_le_bytes());
    }
    let mut names: Vec<(u64, u32)> = (0..n).map(|i| (fnv1a64(tc.name(i).as_bytes()), i as u32)).collect();
    names.sort_unstable();
    b.extend_from_slice(&(names.len() as u32).to_le_bytes());
    for (h, i) in names {
        b.extend_from_slice(&h.to_le_bytes());
        b.extend_from_slice(&i.to_le_bytes());
    }
    b.extend_from_slice(&tc.sha1);
    b.extend_from_slice(&lines.sha_actions);
    b.extend_from_slice(&lines.sha_lists);
    let tmp = path.with_extension("idx.tmp");
    std::fs::write(&tmp, &b)?;
    std::fs::rename(&tmp, path)
}

/// A value's text as the domain holds it (packed_reading `_value`): an int, or "above"/"below".
fn value_pj(text: &str) -> PJ {
    text.parse::<i64>().map_or_else(|_| PJ::str(text), PJ::Int)
}

/// What `reading.json` is made of besides the reader's own state (`read_rust`'s arguments and info).
pub struct HeadIn<'a> {
    pub name: &'a str,
    pub n_lists: usize,
    pub n_actions: usize,
    pub n_records: usize,
    pub start_name: String,
    /// the start row's facts (json.loads of the row)
    pub start_facts: &'a PJ,
    pub rooms_scope: &'a [i32],
    pub walk_verb: Option<i64>,
    pub verbs: &'a [i64],
    pub dialog_verbs: &'a [i64],
    pub bounds: PJ,
    pub counts: PJ,
    pub golden: Option<&'a Path>,
    pub packed: &'a Path,
    pub live_locals: Option<PJ>,
    pub frame_costs: Option<PJ>,
    /// read_rust's info, in its order
    pub info: Vec<(String, PJ)>,
    pub slot_lists: usize,
}

/// The goal as reading.json gives it: {fact: values sorted by their text} in the packed file's order.
pub fn goal_pj(tc: &Tc) -> PJ {
    PJ::Obj(
        tc.goal
            .iter()
            .map(|(k, vals)| {
                let mut v: Vec<&String> = vals.iter().collect();
                v.sort();
                (k.clone(), PJ::List(v.into_iter().map(|t| value_pj(t)).collect()))
            })
            .collect(),
    )
}

/// `reading_from_golden`'s bounds: [low, high] of every fact whose values hold "below" or "above", from `dom`
/// (the reader's own domains), checked as there; Err with the problems.
pub fn bounds_pj(tc: &Tc, dom: &dyn Fn(&str) -> Option<scumm_reader::domain::Domain>) -> Result<PJ, String> {
    let mut out = Vec::new();
    let mut problems = Vec::new();
    for (k, vals) in &tc.facts {
        let (has_b, has_a) = (vals.iter().any(|v| v == "below"), vals.iter().any(|v| v == "above"));
        if !(has_b || has_a) {
            continue;
        }
        let d = dom(k);
        let ok = d.as_ref().is_some_and(|d| d.values.contains(&Label::Below) == has_b && d.values.contains(&Label::Above) == has_a);
        if !ok {
            problems.push(format!("{k}: the packed file's values {vals:?}, the domain's {:?}", d.map(|d| d.values)));
            continue;
        }
        let d = d.unwrap();
        let ints: Vec<i64> = vals.iter().filter_map(|v| v.parse().ok()).collect();
        let below = has_b && ints.iter().any(|&v| d.low.is_some_and(|l| v < l));
        let above = has_a && ints.iter().any(|&v| d.high.is_some_and(|h| v > h));
        if below || above {
            problems.push(format!("{k}: the packed file's values {vals:?} outside [{:?}, {:?}]", d.low, d.high));
            continue;
        }
        let o = |x: Option<i64>| x.map_or(PJ::Null, PJ::Int);
        out.push((k.clone(), PJ::List(vec![o(d.low), o(d.high)])));
    }
    if problems.is_empty() {
        Ok(PJ::Obj(out))
    } else {
        Err(format!("{} facts whose bounds the domains do not give: {:?}", problems.len(), &problems[..problems.len().min(5)]))
    }
}

/// reading.json (module docstring): `reading_from_golden`'s doc, then `read_rust`'s changes.
pub fn head(tc: &Tc, h: HeadIn) -> PJ {
    let mut facts = match h.start_facts {
        PJ::Obj(items) => items.clone(),
        _ => panic!("the start's facts are not an object"),
    };
    facts.sort_by(|a, b| a.0.cmp(&b.0));
    let mut doc = PJ::obj(vec![
        ("format", PJ::str(FORMAT)),
        ("packed", PJ::str(PACKED)),
        ("actions", PJ::str(ACTIONS)),
        ("lists", PJ::str(LISTS)),
        ("n_lists", PJ::Int(h.n_lists as i64)),
        ("n_actions", PJ::Int(h.n_actions as i64)),
        ("n_records", PJ::Int(h.n_records as i64)),
        ("folded", PJ::Int(h.n_records as i64 - h.n_actions as i64)),
        ("goal", goal_pj(tc)),
        ("start", PJ::obj(vec![("name", PJ::str(h.start_name)), ("facts", PJ::Obj(facts))])),
        (
            "scope",
            PJ::obj(vec![
                ("rooms", PJ::ints(h.rooms_scope.iter().map(|&r| r as i64))),
                ("walk_verb", h.walk_verb.map_or(PJ::Null, PJ::Int)),
                ("verbs", PJ::ints(h.verbs.iter().copied())),
                ("dialog_verbs", PJ::ints(h.dialog_verbs.iter().copied())),
            ]),
        ),
        ("bounds", h.bounds),
        ("counts", h.counts),
        ("sources", PJ::Obj(vec![])),
        ("control", PJ::Bool(false)),
        ("name", PJ::str(h.name)),
        (
            "from_golden",
            PJ::obj(vec![
                ("golden", h.golden.map_or(PJ::Null, |g| PJ::str(g.display().to_string()))),
                ("packed", PJ::str(h.packed.display().to_string())),
            ]),
        ),
    ]);
    if let Some(ll) = h.live_locals {
        doc.set("live_locals", ll);
    }
    if let Some(fc) = h.frame_costs {
        doc.set("frame_costs", fc);
    }
    for (k, v) in h.info {
        if doc.get(&k).is_none() {
            doc.set(&k, v);
        }
    }
    doc.set("slot_lists", PJ::Int(h.slot_lists as i64));
    doc
}

/// Writes a file through a temporary name in the same folder.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv_known() {
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
    }
}
