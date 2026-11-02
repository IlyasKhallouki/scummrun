//! The packed file (taskcore/README.md, version 1) as packed_reading.py `Packed` reads it: its tables at open, an
//! action by its word offset on demand (the offsets are reading.idx's, so nothing walks the body at open).

use crate::mmap::{u32_at, Map};
use fmi_core::{Key, Val, ABOVE, BELOW, NONE};
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicI64, Ordering};

/// A string's value not worked out yet (`Packed::v`); no value is this (NONE is i64::MIN + 1, BELOW i64::MIN).
const UNSEEN: i64 = i64::MIN + 2;

pub const MAGIC: u32 = 0x524F4354;
pub const END_MAGIC: u32 = 0x444E4554;
pub const VERSION: u32 = 1;
/// an action's `rec` when it names no record (export_task)
pub const REC_NONE: u32 = 0xFFFF_FFFF;
pub const OP_SET: u32 = 0;
pub const OP_GUESS: u32 = 1;
pub const OP_ADD: u32 = 2;

/// packed_reading `_value`: a value's text as the domain holds it. "None" is NONE; any other text that is not an
/// int, "above" or "below" is refused (no packed file holds one).
pub fn value_of(t: &str) -> Val {
    match t {
        "below" => BELOW,
        "above" => ABOVE,
        "None" => NONE,
        _ => t.parse::<i64>().unwrap_or_else(|_| panic!("a packed value that is not a domain value: {t:?}")),
    }
}

/// A set's values in ascending `Val` order, each once (a frozenset, canonically).
pub fn sorted(mut v: Vec<Val>) -> Vec<Val> {
    v.sort_unstable();
    v.dedup();
    v
}

pub struct Packed {
    pub map: Map,
    pub n_actions: u32,
    /// word offsets of the body and the tables (the footer's)
    pub body: u64,
    pub o_str: u64,
    pub o_facts: u64,
    pub o_goal: u64,
    str_base: usize,
    /// prefix sums of the string lengths (n_strings + 1)
    str_off: Vec<u64>,
    /// per fact id: its key, the string id of its text, its values in the file's order
    pub fact_keys: Vec<Key>,
    pub fact_sid: Vec<u32>,
    pub fact_values: Vec<Vec<Val>>,
    /// the goal: (key, values ascending) in the file's order
    pub goal: Vec<(Key, Vec<Val>)>,
    /// each string's value, worked out the first time an action holds it (UNSEEN before)
    vals: Vec<AtomicI64>,
}

/// One effect as the file holds it.
#[derive(Clone, Debug, PartialEq)]
pub enum Payload {
    Set(Val),
    Guess(Vec<Val>),
    Add(i64),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RawEffect {
    pub op: u32,
    pub fact: u32,
    pub payload: Payload,
    pub when: Vec<(u32, Vec<Val>)>,
}

/// packed_reading `Packed.raw_action`, with fact ids and string ids where it has texts.
#[derive(Clone, Debug, PartialEq)]
pub struct RawAction {
    pub rec: u32,
    pub label: u32,
    pub kind: u32,
    pub end: u32,
    pub cont: u32,
    pub cost: u32,
    pub pre: Vec<(u32, Vec<Val>)>,
    pub effects: Vec<RawEffect>,
}

impl Packed {
    pub fn open(path: &Path) -> io::Result<Packed> {
        let bad = |what: &str| io::Error::new(io::ErrorKind::InvalidData, format!("{}: {what}", path.display()));
        let map = Map::open(path)?;
        let b = map.bytes();
        if b.len() < 40 || b.len() % 4 != 0 || u32_at(b, 0) != MAGIC || u32_at(b, 4) != VERSION {
            return Err(bad(&format!("not a taskcore file of version {VERSION}")));
        }
        let nw = b.len() / 4;
        let f: Vec<u32> = (0..8).map(|k| u32_at(b, 4 * (nw - 8 + k))).collect();
        if f[7] != END_MAGIC {
            return Err(bad("bad footer"));
        }
        let (body, n, o_str, o_facts, o_goal) = (f[0] as u64, f[1], f[2] as u64, f[3] as u64, f[5] as u64);
        let w = |i: u64| u32_at(b, 4 * i as usize);
        let ns = w(o_str) as usize;
        let mut str_off = Vec::with_capacity(ns + 1);
        str_off.push(0u64);
        let mut acc = 0u64;
        for k in 0..ns {
            acc += w(o_str + 1 + k as u64) as u64;
            str_off.push(acc);
        }
        let str_base = 4 * (o_str as usize + 1 + ns);
        if str_base + acc as usize > b.len() {
            return Err(bad("the strings pass the file's end"));
        }
        // the table is UTF-8 and every string starts on a character's start, so each string is UTF-8 (`s`)
        let blob = &b[str_base..str_base + acc as usize];
        std::str::from_utf8(blob).map_err(|_| bad("strings that are not UTF-8"))?;
        if str_off.iter().any(|&o| (o as usize) < blob.len() && (blob[o as usize] & 0xC0) == 0x80) {
            return Err(bad("a string that starts inside a character"));
        }
        let mut p = Packed {
            map,
            n_actions: n,
            body,
            o_str,
            o_facts,
            o_goal,
            str_base,
            str_off,
            fact_keys: Vec::new(),
            fact_sid: Vec::new(),
            fact_values: Vec::new(),
            goal: Vec::new(),
            vals: (0..ns).map(|_| AtomicI64::new(UNSEEN)).collect(),
        };
        let mut q = o_facts;
        let nf = p.word(q) as usize;
        q += 1;
        let (mut keys, mut sids, mut values) = (Vec::with_capacity(nf), Vec::with_capacity(nf), Vec::with_capacity(nf));
        for _ in 0..nf {
            let (key, size) = (p.word(q), p.word(q + 1) as u64);
            keys.push(Key::parse(p.s(key)));
            sids.push(key);
            values.push((0..size).map(|k| value_of(p.s(p.word(q + 2 + k)))).collect::<Vec<Val>>());
            q += 2 + size;
        }
        (p.fact_keys, p.fact_sid, p.fact_values) = (keys, sids, values);
        let mut q = o_goal;
        let ng = p.word(q);
        let mut goal = Vec::with_capacity(ng as usize);
        for _ in 0..ng {
            let (fid, m) = (p.word(q + 1) as usize, p.word(q + 2) as u64);
            goal.push((p.fact_keys[fid], sorted((0..m).map(|k| value_of(p.s(p.word(q + 3 + k)))).collect())));
            q += 2 + m;
        }
        p.goal = goal;
        Ok(p)
    }

    #[inline]
    pub fn word(&self, i: u64) -> u32 {
        u32_at(self.map.bytes(), 4 * i as usize)
    }

    pub fn n_strings(&self) -> usize {
        self.str_off.len() - 1
    }

    /// String `id` of the string table.
    #[inline]
    pub fn s(&self, id: u32) -> &str {
        let (a, z) = (self.str_off[id as usize] as usize, self.str_off[id as usize + 1] as usize);
        let b = &self.map.bytes()[self.str_base + a..self.str_base + z];
        // SAFETY: `open` checked that the table is UTF-8 and that every string starts on a character's start, so
        // every string (from one start to the next) is UTF-8
        unsafe { std::str::from_utf8_unchecked(b) }
    }

    /// Every action's word offset (its length word), walking the body (the index's `tc_word`, for a reading made
    /// before reading.idx).
    pub fn walk(&self) -> io::Result<Vec<u64>> {
        let mut out = Vec::with_capacity(self.n_actions as usize);
        let mut p = self.body;
        for _ in 0..self.n_actions {
            out.push(p);
            p += 1 + self.word(p) as u64;
        }
        if p != self.o_str {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("the actions end at word {p}, the strings start at {}", self.o_str),
            ));
        }
        Ok(out)
    }

    /// (rec, label string id) of the action at word offset `at`.
    #[inline]
    pub fn rec_label(&self, at: u64) -> (u32, u32) {
        (self.word(at + 1), self.word(at + 2))
    }

    /// PackedTask's name of the action at `at`: "label#rec", or the label when rec is NONE.
    pub fn name(&self, at: u64) -> String {
        let (rec, label) = self.rec_label(at);
        if rec == REC_NONE {
            self.s(label).to_string()
        } else {
            format!("{}#{rec}", self.s(label))
        }
    }

    /// Whether the action at `at` is named `name` (no allocation).
    pub fn has_name(&self, at: u64, name: &str) -> bool {
        let (rec, label) = self.rec_label(at);
        let l = self.s(label);
        if rec == REC_NONE {
            return l == name;
        }
        let Some(rest) = name.strip_prefix(l).and_then(|r| r.strip_prefix('#')) else { return false };
        let mut buf = [0u8; 10];
        let mut k = buf.len();
        let mut r = rec;
        loop {
            k -= 1;
            buf[k] = b'0' + (r % 10) as u8;
            r /= 10;
            if r == 0 {
                break;
            }
        }
        rest.as_bytes() == &buf[k..]
    }

    /// The value of string `id` (packed_reading `_value`), kept once worked out.
    #[inline]
    pub fn v(&self, id: u32) -> Val {
        let c = &self.vals[id as usize];
        let x = c.load(Ordering::Relaxed);
        if x != UNSEEN {
            return x;
        }
        let x = value_of(self.s(id));
        c.store(x, Ordering::Relaxed);
        x
    }

    fn sets(&self, mut p: u64, n: u32, out: &mut Vec<(u32, Vec<Val>)>) -> u64 {
        for _ in 0..n {
            let (f, m) = (self.word(p), self.word(p + 1) as u64);
            out.push((f, (0..m).map(|k| self.v(self.word(p + 2 + k))).collect()));
            p += 2 + m;
        }
        p
    }

    /// packed_reading `Packed.raw_action` of the action at word offset `at`.
    pub fn raw(&self, at: u64) -> RawAction {
        let p = at + 1;
        let (rec, label, kind, end, cont, cost, npre) =
            (self.word(p), self.word(p + 1), self.word(p + 2), self.word(p + 3), self.word(p + 4), self.word(p + 5), self.word(p + 6));
        let mut pre = Vec::with_capacity(npre as usize);
        let mut p = self.sets(p + 7, npre, &mut pre);
        let ne = self.word(p);
        p += 1;
        let mut effects = Vec::with_capacity(ne as usize);
        for _ in 0..ne {
            let (op, fact) = (self.word(p), self.word(p + 1));
            let payload = match op {
                OP_SET => {
                    let v = self.v(self.word(p + 2));
                    p += 3;
                    Payload::Set(v)
                }
                OP_GUESS => {
                    let m = self.word(p + 2) as u64;
                    let v = (0..m).map(|k| self.v(self.word(p + 3 + k))).collect();
                    p += 3 + m;
                    Payload::Guess(v)
                }
                OP_ADD => {
                    let d = self.word(p + 2) as i32 as i64;
                    p += 3;
                    Payload::Add(d)
                }
                _ => panic!("action at word {at}: effect op {op}"),
            };
            let nw = self.word(p);
            let mut when = Vec::with_capacity(nw as usize);
            p = self.sets(p + 1, nw, &mut when);
            effects.push(RawEffect { op, fact, payload, when });
        }
        RawAction { rec, label, kind, end, cont, cost, pre, effects }
    }
}
