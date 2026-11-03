//! The plan cache (E011, plan.md format F4 "FMPL" v1): a plan result stored under the sha256 of everything the plan
//! is a function of, so that an exact repeat is a lookup.
//!
//! **The key** is sha256 over `key_bytes`: a version tag (`PLANNER`, changed whenever what a request returns
//! changes), the reading's id (sha1 of reading.tc), the goal, the start as its sorted texts (the start file's lines),
//! the bans, the cost caps restricted to the actions they lower (warm.rs: only those change the task), the search and
//! the start check. The planner is deterministic on these (the reductions, the fold, the writer and both searches
//! read nothing else), so a hit is the plan the request would make. Each part is written in F1's encoding (below), so
//! no two different inputs give the same bytes.
//!
//! **The file** `<dir>/<key in hex>.plan`: `"FMPL"`, u32 version 1, the 32-byte key, then the `PlanResult` in F1's
//! encoding (plan.md "F1": little-endian integers at their Rust width, bool as u8, `Option` as u8 0 or u8 1 then the
//! value, `Vec` as u32 count then the items, `String` as u32 byte length then UTF-8, a struct's fields in fmi-core's
//! declaration order, an enum as u8 variant index then its fields; f64 as its 8 bytes). Every `Key` is written as its
//! text (interned keys are only valid within one process). Written to a temporary file and renamed, so a reader sees a
//! whole file or none.
//!
//! sha1 and sha256 are FIPS 180-4's, here because taskcore has no dependency.

use fmi_core::{Action, Ban, EffVal, Effect, Key, PlanResult, PlanStep, Search, Val};
use std::path::{Path, PathBuf};

/// The planner's version in every key: change it when a request's result changes for the same inputs.
pub const PLANNER: &str = "taskcore-e011-plan 1";
pub const MAGIC: &[u8; 4] = b"FMPL";
pub const VERSION: u32 = 1;

// --- sha1, sha256 ---------------------------------------------------------------------------------------------------

fn pad(msg: &[u8]) -> Vec<u8> {
    let mut m = msg.to_vec();
    let bits = (msg.len() as u64).wrapping_mul(8);
    m.push(0x80);
    while m.len() % 64 != 56 {
        m.push(0);
    }
    m.extend_from_slice(&bits.to_be_bytes());
    m
}

pub fn sha1(msg: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    for chunk in pad(msg).chunks(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([chunk[4 * i], chunk[4 * i + 1], chunk[4 * i + 2], chunk[4 * i + 3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for (i, &wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let t = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e]) {
            *x = x.wrapping_add(y);
        }
    }
    let mut out = [0u8; 20];
    for (i, x) in h.iter().enumerate() {
        out[4 * i..4 * i + 4].copy_from_slice(&x.to_be_bytes());
    }
    out
}

const K256: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01,
    0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
    0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147,
    0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08,
    0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
    0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

pub fn sha256(msg: &[u8]) -> [u8; 32] {
    let mut h: [u32; 8] =
        [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    for chunk in pad(msg).chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([chunk[4 * i], chunk[4 * i + 1], chunk[4 * i + 2], chunk[4 * i + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7].wrapping_add(s1).wrapping_add(ch).wrapping_add(K256[i]).wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [t1.wrapping_add(t2), v[0], v[1], v[2], v[3].wrapping_add(t1), v[4], v[5], v[6]];
        }
        for (x, y) in h.iter_mut().zip(v) {
            *x = x.wrapping_add(y);
        }
    }
    let mut out = [0u8; 32];
    for (i, x) in h.iter().enumerate() {
        out[4 * i..4 * i + 4].copy_from_slice(&x.to_be_bytes());
    }
    out
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{:02x}", x)).collect()
}

// --- F1's encoding ----------------------------------------------------------------------------------------------------

#[derive(Default)]
pub struct W(pub Vec<u8>);

impl W {
    pub fn u8(&mut self, x: u8) {
        self.0.push(x)
    }
    pub fn u32(&mut self, x: u32) {
        self.0.extend_from_slice(&x.to_le_bytes())
    }
    pub fn u64(&mut self, x: u64) {
        self.0.extend_from_slice(&x.to_le_bytes())
    }
    pub fn i64(&mut self, x: i64) {
        self.0.extend_from_slice(&x.to_le_bytes())
    }
    pub fn f64(&mut self, x: f64) {
        self.0.extend_from_slice(&x.to_le_bytes())
    }
    pub fn bool(&mut self, x: bool) {
        self.u8(x as u8)
    }
    pub fn str(&mut self, s: &str) {
        self.u32(s.len() as u32);
        self.0.extend_from_slice(s.as_bytes())
    }
    pub fn key(&mut self, k: Key) {
        self.str(&k.text())
    }
    pub fn vals(&mut self, v: &[Val]) {
        self.u32(v.len() as u32);
        v.iter().for_each(|&x| self.i64(x));
    }
    pub fn sets(&mut self, s: &[(Key, Vec<Val>)]) {
        self.u32(s.len() as u32);
        for (k, v) in s {
            self.key(*k);
            self.vals(v);
        }
    }
    pub fn action(&mut self, a: &Action) {
        self.str(&a.name);
        self.sets(&a.pre);
        self.u32(a.effects.len() as u32);
        for e in &a.effects {
            self.key(e.key);
            match &e.v {
                EffVal::Value(v) => {
                    self.u8(0);
                    self.i64(*v)
                }
                EffVal::Add(d) => {
                    self.u8(1);
                    self.i64(*d)
                }
                EffVal::Values(v) => {
                    self.u8(2);
                    self.vals(v)
                }
            }
            self.sets(&e.when);
        }
        self.u32(a.what);
        self.i64(a.cost);
    }
    pub fn actions(&mut self, v: &[Action]) {
        self.u32(v.len() as u32);
        v.iter().for_each(|a| self.action(a));
    }
    pub fn result(&mut self, r: &PlanResult) {
        self.str(&r.status);
        self.u32(r.steps.len() as u32);
        for s in &r.steps {
            self.str(&s.op);
            self.u32(s.action);
            self.str(&s.name);
            self.u32(s.chosen.len() as u32);
            for (k, v) in &s.chosen {
                self.key(*k);
                self.i64(*v);
            }
            self.u32(s.guessed.len() as u32);
            for (k, v, w) in &s.guessed {
                self.key(*k);
                self.i64(*v);
                self.str(w);
            }
        }
        self.actions(&r.paths);
        self.actions(&r.actions);
        self.u32(r.choices.len() as u32);
        for (i, ps) in &r.choices {
            self.u32(*i);
            self.actions(ps);
        }
        self.u32(r.folds.len() as u32);
        for f in &r.folds {
            match f {
                None => self.u8(0),
                Some(s) => {
                    self.u8(1);
                    self.str(s)
                }
            }
        }
        self.bool(r.predicted_goal);
        match r.plan_cost {
            None => self.u8(0),
            Some(c) => {
                self.u8(1);
                self.i64(c)
            }
        }
        for keys in [&r.file_facts, &r.needed_facts] {
            self.u32(keys.len() as u32);
            keys.iter().for_each(|&k| self.key(k));
        }
        self.u32(r.misses.len() as u32);
        for (k, v) in &r.misses {
            self.key(*k);
            self.i64(*v);
        }
        self.u64(r.n_misses);
        self.str(&r.stats);
        self.str(&r.source);
        self.f64(r.seconds);
        self.f64(r.cpu_s);
    }
}

pub struct R<'a> {
    b: &'a [u8],
    p: usize,
}

type Res<T> = Result<T, String>;

impl<'a> R<'a> {
    pub fn new(b: &'a [u8]) -> R<'a> {
        R { b, p: 0 }
    }
    fn take(&mut self, n: usize) -> Res<&'a [u8]> {
        if self.p + n > self.b.len() {
            return Err(format!("F1: {} bytes wanted at {}, {} there", n, self.p, self.b.len()));
        }
        let s = &self.b[self.p..self.p + n];
        self.p += n;
        Ok(s)
    }
    pub fn u8(&mut self) -> Res<u8> {
        Ok(self.take(1)?[0])
    }
    pub fn u32(&mut self) -> Res<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn u64(&mut self) -> Res<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub fn i64(&mut self) -> Res<i64> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub fn f64(&mut self) -> Res<f64> {
        Ok(f64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub fn bool(&mut self) -> Res<bool> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            x => Err(format!("F1: bool {}", x)),
        }
    }
    pub fn str(&mut self) -> Res<String> {
        let n = self.u32()? as usize;
        String::from_utf8(self.take(n)?.to_vec()).map_err(|e| format!("F1: {}", e))
    }
    pub fn key(&mut self) -> Res<Key> {
        Ok(Key::parse(&self.str()?))
    }
    fn count(&mut self) -> Res<usize> {
        let n = self.u32()? as usize;
        if n > self.b.len() {
            return Err(format!("F1: a count of {} in {} bytes", n, self.b.len()));
        }
        Ok(n)
    }
    pub fn vals(&mut self) -> Res<Vec<Val>> {
        let n = self.count()?;
        (0..n).map(|_| self.i64()).collect()
    }
    pub fn sets(&mut self) -> Res<Vec<(Key, Vec<Val>)>> {
        let n = self.count()?;
        (0..n).map(|_| Ok((self.key()?, self.vals()?))).collect()
    }
    pub fn action(&mut self) -> Res<Action> {
        let name = self.str()?;
        let pre = self.sets()?;
        let n = self.count()?;
        let mut effects = Vec::with_capacity(n);
        for _ in 0..n {
            let key = self.key()?;
            let v = match self.u8()? {
                0 => EffVal::Value(self.i64()?),
                1 => EffVal::Add(self.i64()?),
                2 => EffVal::Values(self.vals()?),
                x => return Err(format!("F1: EffVal {}", x)),
            };
            let when = self.sets()?;
            effects.push(Effect { key, v, when });
        }
        Ok(Action { name, pre, effects, what: self.u32()?, cost: self.i64()? })
    }
    pub fn actions(&mut self) -> Res<Vec<Action>> {
        let n = self.count()?;
        (0..n).map(|_| self.action()).collect()
    }
    pub fn result(&mut self) -> Res<PlanResult> {
        let status = self.str()?;
        let n = self.count()?;
        let mut steps = Vec::with_capacity(n);
        for _ in 0..n {
            let op = self.str()?;
            let action = self.u32()?;
            let name = self.str()?;
            let nc = self.count()?;
            let chosen = (0..nc).map(|_| Ok((self.key()?, self.i64()?))).collect::<Res<_>>()?;
            let ng = self.count()?;
            let guessed = (0..ng).map(|_| Ok((self.key()?, self.i64()?, self.str()?))).collect::<Res<_>>()?;
            steps.push(PlanStep { op, action, name, chosen, guessed });
        }
        let paths = self.actions()?;
        let actions = self.actions()?;
        let n = self.count()?;
        let choices = (0..n).map(|_| Ok((self.u32()?, self.actions()?))).collect::<Res<_>>()?;
        let n = self.count()?;
        let folds = (0..n)
            .map(|_| match self.u8()? {
                0 => Ok(None),
                1 => Ok(Some(self.str()?)),
                x => Err(format!("F1: Option {}", x)),
            })
            .collect::<Res<_>>()?;
        let predicted_goal = self.bool()?;
        let plan_cost = match self.u8()? {
            0 => None,
            1 => Some(self.i64()?),
            x => return Err(format!("F1: Option {}", x)),
        };
        let n = self.count()?;
        let file_facts = (0..n).map(|_| self.key()).collect::<Res<_>>()?;
        let n = self.count()?;
        let needed_facts = (0..n).map(|_| self.key()).collect::<Res<_>>()?;
        let n = self.count()?;
        let misses = (0..n).map(|_| Ok((self.key()?, self.i64()?))).collect::<Res<_>>()?;
        Ok(PlanResult {
            status,
            steps,
            paths,
            actions,
            choices,
            folds,
            predicted_goal,
            plan_cost,
            file_facts,
            needed_facts,
            misses,
            n_misses: self.u64()?,
            stats: self.str()?,
            source: self.str()?,
            seconds: self.f64()?,
            cpu_s: self.f64()?,
        })
    }
    pub fn done(&self) -> bool {
        self.p == self.b.len()
    }
}

// --- the key and the file -----------------------------------------------------------------------------------------------

/// What a plan is a function of (module docstring).
pub struct KeyIn<'a> {
    pub reading: [u8; 20],
    pub goal: &'a [(Key, Vec<Val>)],
    /// the start file's lines, `key<TAB>value`, sorted
    pub start: &'a [String],
    pub bans: &'a [Ban],
    /// (action, cost) for each action the caps lower, in action order
    pub caps: &'a [(u32, u32)],
    pub search: Search,
    pub start_check: &'a str,
}

pub fn key_bytes(k: &KeyIn) -> Vec<u8> {
    let mut w = W::default();
    w.str(PLANNER);
    w.0.extend_from_slice(&k.reading);
    w.u32(k.goal.len() as u32);
    for (key, vals) in k.goal {
        w.key(*key);
        w.vals(vals);
    }
    w.u32(k.start.len() as u32);
    k.start.iter().for_each(|s| w.str(s));
    w.u32(k.bans.len() as u32);
    for b in k.bans {
        w.str(&b.label);
        w.u32(b.facts.len() as u32);
        for (key, v) in &b.facts {
            w.key(*key);
            w.i64(*v);
        }
    }
    w.u32(k.caps.len() as u32);
    for &(i, c) in k.caps {
        w.u32(i);
        w.u32(c);
    }
    w.u8(match k.search {
        Search::FdLamaFirstCosts => 0,
        Search::Gbfs => 1,
    });
    w.str(k.start_check);
    w.0
}

pub fn key(k: &KeyIn) -> [u8; 32] {
    sha256(&key_bytes(k))
}

pub fn path_of(dir: &Path, key: &[u8; 32]) -> PathBuf {
    dir.join(format!("{}.plan", hex(key)))
}

pub fn encode(key: &[u8; 32], r: &PlanResult) -> Vec<u8> {
    let mut w = W::default();
    w.0.extend_from_slice(MAGIC);
    w.u32(VERSION);
    w.0.extend_from_slice(key);
    w.result(r);
    w.0
}

pub fn decode(bytes: &[u8], key: &[u8; 32]) -> Res<PlanResult> {
    let mut r = R::new(bytes);
    if r.take(4)? != MAGIC || r.u32()? != VERSION {
        return Err("not an FMPL v1 plan".into());
    }
    if r.take(32)? != key {
        return Err("the plan's key is not the asked key".into());
    }
    let res = r.result()?;
    if !r.done() {
        return Err("bytes after the plan".into());
    }
    Ok(res)
}

/// The stored plan for `key`, if any (a file that does not decode counts as none).
pub fn load(dir: &Path, key: &[u8; 32]) -> Option<PlanResult> {
    let b = std::fs::read(path_of(dir, key)).ok()?;
    decode(&b, key).ok()
}

/// Store a plan (a temporary file renamed into place).
pub fn store(dir: &Path, key: &[u8; 32], r: &PlanResult) -> Res<()> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {}", dir.display(), e))?;
    let path = path_of(dir, key);
    let tmp = dir.join(format!(".{}.{}.tmp", hex(key), std::process::id()));
    std::fs::write(&tmp, encode(key, r)).map_err(|e| format!("{}: {}", tmp.display(), e))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("{}: {}", path.display(), e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes() {
        assert_eq!(hex(&sha1(b"abc")), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(hex(&sha256(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(hex(&sha256(b"")), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        let long = vec![b'a'; 1000];
        assert_eq!(hex(&sha1(&long)), "291e9a6c66994949b57ba5e650361e98fc36b1ba");
    }

    #[test]
    fn round_trip() {
        let a = Action {
            name: "e#1~ban:room".into(),
            pre: vec![(Key::parse("room"), vec![28, 33])],
            effects: vec![
                Effect { key: Key::var(1), v: EffVal::Values(vec![fmi_core::BELOW, 3]), when: vec![(Key::bit(2), vec![1])] },
                Effect { key: Key::var(2), v: EffVal::Add(-1), when: vec![] },
            ],
            what: 7,
            cost: 3,
        };
        let r = PlanResult {
            status: "plan".into(),
            steps: vec![PlanStep { op: "a3".into(), action: 0, name: "e#1".into(), chosen: vec![(Key::var(5), 2)], guessed: vec![(Key::var(1), 3, "[]".into())] }],
            paths: vec![a.clone()],
            actions: vec![a.clone()],
            choices: vec![(0, vec![a])],
            folds: vec![None, Some("fold:x+1".into())],
            predicted_goal: true,
            plan_cost: Some(16),
            file_facts: vec![Key::parse("pick:var:1")],
            needed_facts: vec![Key::bit(108)],
            misses: vec![],
            n_misses: 0,
            stats: "{}".into(),
            source: "search".into(),
            seconds: 1.5,
            cpu_s: 1.25,
        };
        let k = sha256(b"k");
        let b = encode(&k, &r);
        let back = decode(&b, &k).unwrap();
        assert_eq!(encode(&k, &back), b);
        assert!(decode(&b, &sha256(b"other")).is_err());
    }
}
