//! The task in memory: flat integer arrays.
//!
//! Facts are numbered 0..n; each has a domain, a list of value strings (its order is the domain's order, the
//! order task.py's `Domain.values` gives). A value is its index in its fact's domain. A set of values is a sorted
//! list of indices.
//!
//! Actions live in one `Vec<u32>` (`Actions::words`), one slice per action, with an offset array:
//!
//! ```text
//! npre, npre x [fact, n, v1..vn],
//! neff, neff x [fact, kind, n, v1..vn, nwhen, nwhen x [fact, n, v1..vn]]
//! ```
//!
//! `kind` is `K_SET` (one value), `K_GUESS` (a write of a value the reader cannot work out: one of the values) or
//! `K_BAD` (a value the domain lacks: the writer raises on it in a live action, as task.py does). Each action also
//! has a `Meta` (name, record kind, path end, continuation, cost) and its members: the actions of the input file it
//! stands for (one, or several after the fork fold).

use std::collections::HashMap;

pub const NONE: u32 = u32::MAX;
pub const K_SET: u32 = 0;
pub const K_GUESS: u32 = 1;
pub const K_BAD: u32 = 2;

#[derive(Default, Clone)]
#[allow(dead_code)] // `intern` is for stage 2 (strings made in Rust)
pub struct Strings {
    pub list: Vec<String>,
    map: HashMap<String, u32>,
}

impl Strings {
    pub fn from_list(list: Vec<String>) -> Self {
        let mut map = HashMap::with_capacity(list.len());
        for (i, s) in list.iter().enumerate() {
            map.entry(s.clone()).or_insert(i as u32);
        }
        Strings { list, map }
    }
    #[allow(dead_code)]
    pub fn intern(&mut self, s: &str) -> u32 {
        if let Some(&i) = self.map.get(s) {
            return i;
        }
        let i = self.list.len() as u32;
        self.list.push(s.to_string());
        self.map.insert(s.to_string(), i);
        i
    }
    pub fn get(&self, i: u32) -> &str {
        &self.list[i as usize]
    }
    /// The id of a string, if the table has it.
    pub fn find(&self, s: &str) -> Option<u32> {
        self.map.get(s).copied()
    }
}

#[derive(Clone, Default)]
pub struct Facts {
    pub key: Vec<u32>,     // string id of the fact key
    pub dom_off: Vec<u32>, // CSR offsets, len n + 1
    pub dom: Vec<u32>,     // string ids of the domain values
}

impl Facts {
    pub fn len(&self) -> usize {
        self.key.len()
    }
    pub fn dom(&self, f: u32) -> &[u32] {
        &self.dom[self.dom_off[f as usize] as usize..self.dom_off[f as usize + 1] as usize]
    }
    pub fn size(&self, f: u32) -> u32 {
        self.dom_off[f as usize + 1] - self.dom_off[f as usize]
    }
    pub fn base(&self, f: u32) -> usize {
        self.dom_off[f as usize] as usize
    }
    pub fn total_values(&self) -> usize {
        self.dom.len()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Meta {
    pub rec: u32,   // the record's index (the name is label#rec), NONE for a name that is the label alone
    pub label: u32, // string ids from here on
    pub kind: u32,
    pub end: u32,
    pub cont: u32,
    pub cost: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OSet {
    pub fact: u32,
    pub vals: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OEff {
    pub fact: u32,
    pub kind: u32,
    pub vals: Vec<u32>,
    pub when: Vec<OSet>,
}

#[derive(Clone, Debug, Default)]
pub struct OAct {
    pub pre: Vec<OSet>,
    pub eff: Vec<OEff>,
}

#[derive(Clone)]
pub struct Actions {
    pub words: Vec<u32>,
    pub off: Vec<usize>,
    pub meta: Vec<Meta>,
    pub mem_off: Vec<usize>,
    pub mem: Vec<u32>,
}

impl Default for Actions {
    fn default() -> Self {
        Actions { words: Vec::new(), off: vec![0], meta: Vec::new(), mem_off: vec![0], mem: Vec::new() }
    }
}

/// Sets of a list: (fact, values) one after another, from the count word at `pos`.
#[derive(Clone)]
pub struct SetIter<'a> {
    w: &'a [u32],
    pos: usize,
    left: u32,
}

impl<'a> Iterator for SetIter<'a> {
    type Item = (u32, &'a [u32]);
    fn next(&mut self) -> Option<Self::Item> {
        if self.left == 0 {
            return None;
        }
        self.left -= 1;
        let f = self.w[self.pos];
        let n = self.w[self.pos + 1] as usize;
        let s = &self.w[self.pos + 2..self.pos + 2 + n];
        self.pos += 2 + n;
        Some((f, s))
    }
}

fn sets_at(w: &[u32], pos: usize) -> SetIter<'_> {
    SetIter { w, pos: pos + 1, left: w[pos] }
}

fn skip_sets(w: &[u32], pos: usize) -> usize {
    let n = w[pos];
    let mut q = pos + 1;
    for _ in 0..n {
        q += 2 + w[q + 1] as usize;
    }
    q
}

pub struct EffView<'a> {
    pub fact: u32,
    pub kind: u32,
    pub vals: &'a [u32],
    w: &'a [u32],
    when_pos: usize,
}

impl<'a> EffView<'a> {
    pub fn when(&self) -> SetIter<'a> {
        sets_at(self.w, self.when_pos)
    }
    pub fn nwhen(&self) -> u32 {
        self.w[self.when_pos]
    }
}

pub struct EffIter<'a> {
    w: &'a [u32],
    pos: usize,
    left: u32,
}

impl<'a> Iterator for EffIter<'a> {
    type Item = EffView<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.left == 0 {
            return None;
        }
        self.left -= 1;
        let p = self.pos;
        let n = self.w[p + 2] as usize;
        let e = EffView { fact: self.w[p], kind: self.w[p + 1], vals: &self.w[p + 3..p + 3 + n], w: self.w, when_pos: p + 3 + n };
        self.pos = skip_sets(self.w, p + 3 + n);
        Some(e)
    }
}

#[derive(Clone, Copy)]
pub struct View<'a> {
    pub w: &'a [u32],
}

impl<'a> View<'a> {
    pub fn pre(&self) -> SetIter<'a> {
        sets_at(self.w, 0)
    }
    pub fn effs(&self) -> EffIter<'a> {
        let p = skip_sets(self.w, 0);
        EffIter { w: self.w, pos: p + 1, left: self.w[p] }
    }
    pub fn neff(&self) -> u32 {
        self.w[skip_sets(self.w, 0)]
    }
    pub fn to_owned(&self) -> OAct {
        OAct {
            pre: self.pre().map(|(f, s)| OSet { fact: f, vals: s.to_vec() }).collect(),
            eff: self
                .effs()
                .map(|e| OEff {
                    fact: e.fact,
                    kind: e.kind,
                    vals: e.vals.to_vec(),
                    when: e.when().map(|(f, s)| OSet { fact: f, vals: s.to_vec() }).collect(),
                })
                .collect(),
        }
    }
}

impl Actions {
    pub fn len(&self) -> usize {
        self.meta.len()
    }
    pub fn view(&self, i: usize) -> View<'_> {
        View { w: &self.words[self.off[i]..self.off[i + 1]] }
    }
    pub fn get(&self, i: usize) -> OAct {
        self.view(i).to_owned()
    }
    pub fn members(&self, i: usize) -> &[u32] {
        &self.mem[self.mem_off[i]..self.mem_off[i + 1]]
    }
    pub fn push(&mut self, m: Meta, members: &[u32], a: &OAct) {
        let w = &mut self.words;
        w.push(a.pre.len() as u32);
        for s in &a.pre {
            w.push(s.fact);
            w.push(s.vals.len() as u32);
            w.extend_from_slice(&s.vals);
        }
        w.push(a.eff.len() as u32);
        for e in &a.eff {
            w.push(e.fact);
            w.push(e.kind);
            w.push(e.vals.len() as u32);
            w.extend_from_slice(&e.vals);
            w.push(e.when.len() as u32);
            for c in &e.when {
                w.push(c.fact);
                w.push(c.vals.len() as u32);
                w.extend_from_slice(&c.vals);
            }
        }
        self.off.push(w.len());
        self.meta.push(m);
        self.mem.extend_from_slice(members);
        self.mem_off.push(self.mem.len());
    }
    /// The same action again (no change), into another Actions.
    pub fn copy_from(&mut self, src: &Actions, i: usize) {
        let w = &src.words[src.off[i]..src.off[i + 1]];
        self.words.extend_from_slice(w);
        self.off.push(self.words.len());
        self.meta.push(src.meta[i]);
        self.mem.extend_from_slice(src.members(i));
        self.mem_off.push(self.mem.len());
    }
    pub fn n_effects(&self) -> u64 {
        (0..self.len()).map(|i| self.view(i).neff() as u64).sum()
    }
    /// Action `i` of a source, as it is (no change), into this Actions: `copy_from` for any `Src`.
    pub fn copy_src<S: Src + ?Sized>(&mut self, src: &S, i: usize) {
        self.words.extend_from_slice(src.view(i).w);
        self.off.push(self.words.len());
        self.meta.push(src.meta(i));
        self.mem.extend_from_slice(src.members(i));
        self.mem_off.push(self.mem.len());
    }
}

/// A read-only list of actions: an `Actions`, or the banned task as a view over the input's (bans::Banned), so that
/// the fold, (e)'s needed values and the start check can read the banned task without a copy of it (serve.rs).
pub trait Src {
    fn n(&self) -> usize;
    fn view(&self, i: usize) -> View<'_>;
    fn meta(&self, i: usize) -> Meta;
    fn members(&self, i: usize) -> &[u32];
    fn get(&self, i: usize) -> OAct {
        self.view(i).to_owned()
    }
}

impl Src for Actions {
    fn n(&self) -> usize {
        self.len()
    }
    fn view(&self, i: usize) -> View<'_> {
        Actions::view(self, i)
    }
    fn meta(&self, i: usize) -> Meta {
        self.meta[i]
    }
    fn members(&self, i: usize) -> &[u32] {
        Actions::members(self, i)
    }
}

#[derive(Clone)]
pub struct Task {
    pub strs: Strings,
    pub facts: Facts,
    pub goal: Vec<(u32, Vec<u32>)>,   // (fact, values), in the file's order
    pub starts: Vec<(u32, Vec<u32>)>, // (name, per fact its value or NONE)
    pub acts: Actions,
    pub n_orig: u32, // actions in the input file
}

impl Task {
    pub fn name(&self, i: usize) -> String {
        let m = &self.acts.meta[i];
        let base = if m.rec == NONE { self.strs.get(m.label).to_string() } else { format!("{}#{}", self.strs.get(m.label), m.rec) };
        let n = self.acts.members(i).len();
        if n > 1 {
            format!("fold:{}+{}", base, n - 1)
        } else {
            base
        }
    }
    pub fn key(&self, f: u32) -> &str {
        self.strs.get(self.facts.key[f as usize])
    }
    pub fn start_index(&self, name: &str) -> Option<usize> {
        self.starts.iter().position(|(n, _)| self.strs.get(*n) == name)
    }
}

// --- sorted sets --------------------------------------------------------------------------------------------------

pub fn contains(s: &[u32], x: u32) -> bool {
    if s.len() < 16 {
        s.contains(&x)
    } else {
        s.binary_search(&x).is_ok()
    }
}

pub fn intersects(a: &[u32], b: &[u32]) -> bool {
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            return true;
        }
        if a[i] < b[j] {
            i += 1;
        } else {
            j += 1;
        }
    }
    false
}

pub fn intersect(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut out = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            out.push(a[i]);
            i += 1;
            j += 1;
        } else if a[i] < b[j] {
            i += 1;
        } else {
            j += 1;
        }
    }
    out
}

pub fn union(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut out = Vec::with_capacity(a.len() + b.len());
    let (mut i, mut j) = (0, 0);
    while i < a.len() || j < b.len() {
        if j == b.len() || (i < a.len() && a[i] < b[j]) {
            out.push(a[i]);
            i += 1;
        } else if i == a.len() || b[j] < a[i] {
            out.push(b[j]);
            j += 1;
        } else {
            out.push(a[i]);
            i += 1;
            j += 1;
        }
    }
    out
}

pub fn is_subset(a: &[u32], b: &[u32]) -> bool {
    let mut j = 0;
    for &x in a {
        while j < b.len() && b[j] < x {
            j += 1;
        }
        if j == b.len() || b[j] != x {
            return false;
        }
        j += 1;
    }
    true
}

/// A small fast hasher (FxHash) for the writer's dedup.
#[derive(Default, Clone, Copy)]
pub struct Fx(u64);

impl std::hash::Hasher for Fx {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0.rotate_left(5) ^ b as u64).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
        }
    }
    fn write_u32(&mut self, i: u32) {
        self.0 = (self.0.rotate_left(5) ^ i as u64).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
    fn write_u64(&mut self, i: u64) {
        self.0 = (self.0.rotate_left(5) ^ i).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
    fn write_usize(&mut self, i: usize) {
        self.write_u64(i as u64);
    }
}

pub type FxBuild = std::hash::BuildHasherDefault<Fx>;
pub type FxMap<K, V> = HashMap<K, V, FxBuild>;

pub fn hash_words(w: &[u32]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &x in w {
        h = (h.rotate_left(5) ^ x as u64).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
    h
}

/// A deterministic random number generator (splitmix64) for the fold check.
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    pub fn below(&mut self, n: u32) -> u32 {
        (self.next() % n as u64) as u32
    }
}

/// Peak resident memory of this process (VmHWM), in bytes.
pub fn peak_rss() -> u64 {
    if let Ok(s) = std::fs::read_to_string("/proc/self/status") {
        for line in s.lines() {
            if let Some(rest) = line.strip_prefix("VmHWM:") {
                let kb: u64 = rest.trim().trim_end_matches("kB").trim().parse().unwrap_or(0);
                return kb * 1024;
            }
        }
    }
    0
}
