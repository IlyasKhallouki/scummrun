//! The test double of the reading port (plan.md, "play"; cargo feature `json-double`): `fmi_core::ReadingSource`
//! over a reading's four files, read as packed_reading.py reads them (`Packed`, `Lines`, `PackedTask._action`,
//! `effects_of`, `load_lists`). Replaced at integration by fmi-reading (mapped files and reading.idx).

use crate::pyj::{self, J};
use crate::val::val_of;
use fmi_core::*;
use rustc_hash::FxHashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const MAGIC: u32 = 0x524F4354;
const END_MAGIC: u32 = 0x444E4554;
const VERSION: u32 = 1;
const NONE_REC: u32 = 0xFFFF_FFFF;
const OP_SET: u32 = 0;
const OP_GUESS: u32 = 1;

pub struct FileReading {
    dir: PathBuf,
    head: String,
    tc: Vec<u8>,
    words: Vec<u32>,
    id: [u8; 20],
    strings: Vec<String>,
    svals: Vec<Val>,
    fact_keys: Vec<Key>,
    offsets: Vec<usize>,
    actions_text: String,
    action_offs: Vec<usize>,
    lists_text: String,
    list_offs: Vec<usize>,
    reached: String,
    domains: Domains,
    goal: Vec<(Key, Vec<Val>)>,
    start: Facts,
    names: OnceLock<FxHashMap<String, u32>>,
}

fn line_offsets(t: &str) -> Vec<usize> {
    let mut v = vec![0];
    for (i, b) in t.bytes().enumerate() {
        if b == b'\n' && i + 1 < t.len() {
            v.push(i + 1);
        }
    }
    if t.is_empty() {
        v.clear();
    }
    v
}

fn line_at<'a>(t: &'a str, offs: &[usize], i: u32) -> &'a str {
    let a = offs[i as usize];
    let b = offs.get(i as usize + 1).copied().unwrap_or(t.len());
    t[a..b].trim_end_matches('\n')
}

impl FileReading {
    /// reading.json's folder (or its path).
    pub fn open(path: &Path) -> std::io::Result<FileReading> {
        let (dir, json) = if path.is_dir() {
            (path.to_path_buf(), path.join("reading.json"))
        } else {
            (path.parent().unwrap().to_path_buf(), path.to_path_buf())
        };
        let head = std::fs::read_to_string(&json)?;
        let doc = pyj::parse(&head).map_err(std::io::Error::other)?;
        let packed = dir.join(doc.s("packed").unwrap_or("reading.tc"));
        let tc = std::fs::read(&packed)?;
        let id = {
            let mut h = sha1_smol::Sha1::new();
            h.update(&tc);
            h.digest().bytes()
        };
        let words: Vec<u32> = tc
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        let bad = |m: &str| std::io::Error::other(format!("{}: {m}", packed.display()));
        if words.len() < 10 || words[0] != MAGIC || words[1] != VERSION {
            return Err(bad("not a taskcore file of version 1"));
        }
        let n = words.len();
        let (body, nact, o_str, o_facts, _o_adds, o_goal, _o_starts, end) = (
            words[n - 8] as usize,
            words[n - 7] as usize,
            words[n - 6] as usize,
            words[n - 5] as usize,
            words[n - 4] as usize,
            words[n - 3] as usize,
            words[n - 2] as usize,
            words[n - 1],
        );
        if end != END_MAGIC {
            return Err(bad("bad footer"));
        }
        let ns = words[o_str] as usize;
        let mut pos = (o_str + 1 + ns) * 4;
        let mut strings = Vec::with_capacity(ns);
        for k in 0..ns {
            let ln = words[o_str + 1 + k] as usize;
            strings.push(String::from_utf8_lossy(&tc[pos..pos + ln]).into_owned());
            pos += ln;
        }
        let svals: Vec<Val> = strings.iter().map(|s| val_of(&J::Str(s.clone()))).collect();
        let bounds = doc.get("bounds").cloned().unwrap_or(J::Null);
        let mut p = o_facts;
        let nf = words[p] as usize;
        p += 1;
        let mut fact_keys = Vec::with_capacity(nf);
        let mut doms = Vec::with_capacity(nf);
        for _ in 0..nf {
            let (key, size) = (words[p] as usize, words[p + 1] as usize);
            let k = Key::parse(&strings[key]);
            let mut vals: Vec<Val> = words[p + 2..p + 2 + size].iter().map(|&v| svals[v as usize]).collect();
            vals.sort();
            let b = bounds.get(&strings[key]);
            let lo = b.and_then(|b| b.arr().first().and_then(J::int));
            let hi = b.and_then(|b| b.arr().get(1).and_then(J::int));
            let dom = Domain {
                low: if vals.contains(&BELOW) { lo } else { None },
                high: if vals.contains(&ABOVE) { hi } else { None },
                values: vals,
            };
            fact_keys.push(k);
            doms.push((k, dom));
            p += 2 + size;
        }
        doms.sort_by_key(|d| d.0);
        let mut goal = Vec::new();
        let mut p = o_goal;
        for _ in 0..words[o_goal] {
            let (f, m) = (words[p + 1] as usize, words[p + 2] as usize);
            let mut vs: Vec<Val> = words[p + 3..p + 3 + m].iter().map(|&v| svals[v as usize]).collect();
            vs.sort();
            goal.push((fact_keys[f], vs));
            p += 2 + m;
        }
        let mut offsets = Vec::with_capacity(nact);
        let mut p = body;
        for _ in 0..nact {
            offsets.push(p);
            p += 1 + words[p] as usize;
        }
        if p != o_str {
            return Err(bad("the actions do not end where the strings start"));
        }
        let actions_text = std::fs::read_to_string(dir.join(doc.s("actions").unwrap_or("reading.actions.jsonl")))?;
        let action_offs = line_offsets(&actions_text);
        if action_offs.len() != nact {
            return Err(bad(&format!("{} action lines for {nact} actions", action_offs.len())));
        }
        let lists_text = match doc.s("lists") {
            Some(l) => std::fs::read_to_string(dir.join(l))?,
            None => String::new(),
        };
        let list_offs = line_offsets(&lists_text);
        let reached = std::fs::read_to_string(packed.with_extension("reached")).unwrap_or_default();
        let start = Facts::from_pairs(
            doc.get("start")
                .and_then(|s| s.get("facts"))
                .map(|f| {
                    f.obj()
                        .iter()
                        .map(|(k, v)| (Key::parse(k), v.int().unwrap_or(0)))
                        .collect()
                })
                .unwrap_or_default(),
        );
        Ok(FileReading {
            dir,
            head,
            tc,
            words,
            id,
            strings,
            svals,
            fact_keys,
            offsets,
            actions_text,
            action_offs,
            lists_text,
            list_offs,
            reached,
            domains: Domains(doms),
            goal,
            start,
            names: OnceLock::new(),
        })
    }

    fn sets(&self, mut p: usize, n: usize) -> (Vec<(Key, Vec<Val>)>, usize) {
        let w = &self.words;
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            let (f, m) = (w[p] as usize, w[p + 1] as usize);
            let mut vs: Vec<Val> = w[p + 2..p + 2 + m].iter().map(|&v| self.svals[v as usize]).collect();
            vs.sort();
            vs.dedup();
            out.push((self.fact_keys[f], vs));
            p += 2 + m;
        }
        (out, p)
    }

    /// packed_reading.Packed.raw_action: (rec, label, kind, end, cost, pre, effects [(op, key, payload, when)])
    #[allow(clippy::type_complexity)]
    fn raw(
        &self,
        i: u32,
    ) -> (
        u32,
        &str,
        &str,
        &str,
        i64,
        Vec<(Key, Vec<Val>)>,
        Vec<(u32, Key, RawPay, Vec<(Key, Vec<Val>)>)>,
    ) {
        let w = &self.words;
        let p = self.offsets[i as usize] + 1;
        let (rec, label, kind, end, _cont, cost, npre) = (
            w[p],
            w[p + 1] as usize,
            w[p + 2] as usize,
            w[p + 3] as usize,
            w[p + 4],
            w[p + 5],
            w[p + 6] as usize,
        );
        let (pre, mut p) = self.sets(p + 7, npre);
        let ne = w[p] as usize;
        p += 1;
        let mut effects = Vec::with_capacity(ne);
        for _ in 0..ne {
            let (op, f) = (w[p], w[p + 1] as usize);
            let key = self.fact_keys[f];
            let pay = match op {
                OP_SET => {
                    let v = self.svals[w[p + 2] as usize];
                    p += 3;
                    RawPay::Set(v)
                }
                OP_GUESS => {
                    let m = w[p + 2] as usize;
                    let mut vs: Vec<Val> = w[p + 3..p + 3 + m].iter().map(|&v| self.svals[v as usize]).collect();
                    vs.sort();
                    vs.dedup();
                    p += 3 + m;
                    RawPay::Guess(vs)
                }
                _ => {
                    let d = w[p + 2] as i32 as i64;
                    p += 3;
                    RawPay::Add(d)
                }
            };
            let nw = w[p] as usize;
            let (when, q) = self.sets(p + 1, nw);
            p = q;
            effects.push((op, key, pay, when));
        }
        (
            rec,
            &self.strings[label],
            &self.strings[kind],
            &self.strings[end],
            cost as i64,
            pre,
            effects,
        )
    }

    fn name_of(&self, i: u32) -> String {
        let w = &self.words;
        let p = self.offsets[i as usize] + 1;
        let (rec, label) = (w[p], &self.strings[w[p + 1] as usize]);
        if rec != NONE_REC {
            format!("{label}#{rec}")
        } else {
            label.clone()
        }
    }
}

#[derive(Clone, Debug)]
enum RawPay {
    Set(Val),
    Guess(Vec<Val>),
    Add(i64),
}

/// packed_reading.effects_of: the reader's effects from the packed action's, in first-appearance order.
fn effects_of(effects: &[(u32, Key, RawPay, Vec<(Key, Vec<Val>)>)]) -> Vec<(Key, RecEff)> {
    let mut order: Vec<Key> = Vec::new();
    let mut by: FxHashMap<Key, Vec<usize>> = FxHashMap::default();
    for (n, e) in effects.iter().enumerate() {
        by.entry(e.1).or_insert_with(|| {
            order.push(e.1);
            Vec::new()
        });
        by.get_mut(&e.1).unwrap().push(n);
    }
    let mut out = Vec::new();
    for key in order {
        let es: Vec<&(u32, Key, RawPay, Vec<(Key, Vec<Val>)>)> = by[&key].iter().map(|&n| &effects[n]).collect();
        if es.len() == 2 && es.iter().all(|e| e.0 == OP_SET && e.3.len() == 1) {
            let mut two = es.clone();
            two.sort_by_key(|e| {
                let mut t: Vec<String> = e.3[0].1.iter().map(|&v| val_text(v)).collect();
                t.sort();
                t
            });
            let (a, b) = (two[0], two[1]);
            let (v0, v1) = match (&a.2, &b.2) {
                (RawPay::Set(x), RawPay::Set(y)) => (*x, *y),
                _ => (NONE, NONE),
            };
            if a.3[0].0 == b.3[0].0 && a.3[0].1 == [0] && b.3[0].1 == [1] && (v0, v1) == (0, 1) {
                out.push((key, RecEff::Copy(a.3[0].0)));
                continue;
            }
        }
        let e = es[0];
        let eff = match &e.2 {
            RawPay::Guess(vs) => RecEff::Guess(vs.clone()),
            RawPay::Add(d) => RecEff::Add(*d),
            RawPay::Set(v) if e.3.is_empty() => RecEff::Set(*v),
            RawPay::Set(v) => RecEff::When(e.3[0].0, *v),
        };
        out.push((key, eff));
    }
    out
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
        &self.tc
    }
    fn n_actions(&self) -> u32 {
        self.offsets.len() as u32
    }
    fn action_line(&self, i: u32) -> &str {
        line_at(&self.actions_text, &self.action_offs, i)
    }
    fn by_name(&self, name: &str) -> Option<u32> {
        let m = self.names.get_or_init(|| {
            let mut m = FxHashMap::default();
            for i in 0..self.offsets.len() as u32 {
                m.insert(self.name_of(i), i); // the last action of a name wins (a dict comprehension)
            }
            m
        });
        m.get(name).copied()
    }
    fn n_lists(&self) -> u32 {
        self.list_offs.len() as u32
    }
    fn list_line(&self, i: u32) -> &str {
        line_at(&self.lists_text, &self.list_offs, i)
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
        let (rec, label, _kind, _end, cost, pre, effs) = self.raw(i);
        let name = if rec != NONE_REC {
            format!("{label}#{rec}")
        } else {
            label.to_string()
        };
        let effects = effs
            .into_iter()
            .map(|(_, key, pay, when)| Effect {
                key,
                v: match pay {
                    RawPay::Set(v) => EffVal::Value(v),
                    RawPay::Guess(vs) => EffVal::Values(vs),
                    RawPay::Add(d) => EffVal::Add(d),
                },
                when,
            })
            .collect();
        Action {
            name,
            pre,
            effects,
            what: i,
            cost,
        }
    }
    fn record(&self, i: u32) -> Record {
        let (_rec, label, kind, end, _cost, pre, effs) = self.raw(i);
        let line = pyj::parse(self.action_line(i)).unwrap_or(J::Null);
        let pre_keys: Vec<Key> = line
            .a("pre_keys")
            .iter()
            .filter_map(|k| k.str().map(Key::parse))
            .collect();
        Record {
            kind: kind.to_string(),
            label: label.to_string(),
            pre: pre_keys
                .iter()
                .filter_map(|k| pre.iter().find(|p| p.0 == *k).cloned())
                .collect(),
            effects: effects_of(&effs),
            checks: line.a("checks").iter().map(pyj::dumps).collect(),
            end: end.to_string(),
            source: pyj::dumps(line.get("source").unwrap_or(&J::Arr(vec![]))),
            engine_pre: line
                .a("engine_pre")
                .iter()
                .filter_map(|k| k.str().map(Key::parse))
                .collect(),
        }
    }
}
