//! reading.idx (experiments/E011-speed/plan.md, F3), version 1, little-endian:
//!
//! ```text
//! u8[4] "FMRX", u32 version = 1
//! u32 n_actions, u32 n_lists
//! u64 action_off[n_actions + 1]   byte offset of each line of reading.actions.jsonl; the last is the file's size
//! u64 list_off[n_lists + 1]       the same for reading.lists.jsonl
//! u64 tc_word[n_actions]          word offset of each action's length word in reading.tc
//! u32 n_names; { u64 hash, u32 action }[n_names]
//!                                 every action, sorted by (hash, action); hash = FNV-1a 64 of the action's name
//! u8  sha1_tc[20], u8 sha1_actions[20], u8 sha1_lists[20]
//! ```
//!
//! reader-scope `--out-dir` writes it as it writes the reading; [`build`] makes the same bytes from the files of a
//! reading made before (one pass over each file).

use crate::mmap::{u32_at, u64_at, Map};
use crate::tc::Packed;
use std::io;
use std::path::Path;

pub const MAGIC: &[u8; 4] = b"FMRX";
pub const VERSION: u32 = 1;

/// FNV-1a 64 (the name index's hash).
pub fn fnv1a64(s: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in s {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

enum Bytes {
    Map(Map),
    Vec(Vec<u8>),
}

/// A reading.idx, mapped (or just built).
pub struct Idx {
    bytes: Bytes,
    pub n_actions: u32,
    pub n_lists: u32,
    a_off: usize,
    l_off: usize,
    w_off: usize,
    names_off: usize,
    pub n_names: u32,
    pub sha1_tc: [u8; 20],
    pub sha1_actions: [u8; 20],
    pub sha1_lists: [u8; 20],
}

fn bad(what: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, what)
}

impl Idx {
    fn of(bytes: Bytes) -> io::Result<Idx> {
        let b = match &bytes {
            Bytes::Map(m) => m.bytes(),
            Bytes::Vec(v) => v.as_slice(),
        };
        if b.len() < 16 || &b[..4] != MAGIC || u32_at(b, 4) != VERSION {
            return Err(bad(format!("not a reading.idx of version {VERSION}")));
        }
        let (na, nl) = (u32_at(b, 8), u32_at(b, 12));
        let a_off = 16;
        let l_off = a_off + 8 * (na as usize + 1);
        let w_off = l_off + 8 * (nl as usize + 1);
        let n_at = w_off + 8 * na as usize;
        if b.len() < n_at + 4 {
            return Err(bad("a short reading.idx".into()));
        }
        let n_names = u32_at(b, n_at);
        let names_off = n_at + 4;
        let sha_at = names_off + 12 * n_names as usize;
        if b.len() != sha_at + 60 {
            return Err(bad(format!("reading.idx: {} bytes, its counts give {}", b.len(), sha_at + 60)));
        }
        let sha = |k: usize| -> [u8; 20] { b[sha_at + 20 * k..sha_at + 20 * (k + 1)].try_into().unwrap() };
        let (sha1_tc, sha1_actions, sha1_lists) = (sha(0), sha(1), sha(2));
        Ok(Idx { bytes, n_actions: na, n_lists: nl, a_off, l_off, w_off, names_off, n_names, sha1_tc, sha1_actions, sha1_lists })
    }

    pub fn open(path: &Path) -> io::Result<Idx> {
        Idx::of(Bytes::Map(Map::open(path)?))
    }

    pub fn from_vec(v: Vec<u8>) -> io::Result<Idx> {
        Idx::of(Bytes::Vec(v))
    }

    #[inline]
    pub fn bytes(&self) -> &[u8] {
        match &self.bytes {
            Bytes::Map(m) => m.bytes(),
            Bytes::Vec(v) => v.as_slice(),
        }
    }

    #[inline]
    pub fn action_off(&self, i: u32) -> u64 {
        u64_at(self.bytes(), self.a_off + 8 * i as usize)
    }

    #[inline]
    pub fn list_off(&self, i: u32) -> u64 {
        u64_at(self.bytes(), self.l_off + 8 * i as usize)
    }

    #[inline]
    pub fn tc_word(&self, i: u32) -> u64 {
        u64_at(self.bytes(), self.w_off + 8 * i as usize)
    }

    #[inline]
    fn name_at(&self, k: u32) -> (u64, u32) {
        let p = self.names_off + 12 * k as usize;
        let b = self.bytes();
        (u64_at(b, p), u32_at(b, p + 8))
    }

    /// The actions whose name hashes to `h`, greatest first.
    pub fn with_hash(&self, h: u64) -> impl Iterator<Item = u32> + '_ {
        // the first entry with hash >= h
        let (mut lo, mut hi) = (0u32, self.n_names);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if self.name_at(mid).0 < h {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let mut end = lo;
        while end < self.n_names && self.name_at(end).0 == h {
            end += 1;
        }
        (lo..end).rev().map(move |k| self.name_at(k).1)
    }
}

/// Line starts of a line file, and its size last (a last line with no newline counts as a line).
pub fn line_offsets(b: &[u8]) -> Vec<u64> {
    let mut out = vec![0u64];
    for (k, &c) in b.iter().enumerate() {
        if c == b'\n' {
            out.push(k as u64 + 1);
        }
    }
    if b.last().is_some_and(|&c| c != b'\n') {
        out.push(b.len() as u64);
    }
    out
}

/// The bytes of reading.idx for a reading's files (module docstring).
pub fn build(packed: &Packed, actions: &[u8], lists: &[u8]) -> io::Result<Vec<u8>> {
    let a = line_offsets(actions);
    let l = line_offsets(lists);
    let words = packed.walk()?;
    let n = words.len();
    if a.len() != n + 1 {
        return Err(bad(format!("{} action lines for {} actions", a.len() - 1, n)));
    }
    let mut b: Vec<u8> = Vec::with_capacity(16 + 8 * (a.len() + l.len() + n) + 4 + 12 * n + 60);
    b.extend_from_slice(MAGIC);
    b.extend_from_slice(&VERSION.to_le_bytes());
    b.extend_from_slice(&(n as u32).to_le_bytes());
    b.extend_from_slice(&((l.len() - 1) as u32).to_le_bytes());
    for x in a.iter().chain(&l).chain(&words) {
        b.extend_from_slice(&x.to_le_bytes());
    }
    let mut names: Vec<(u64, u32)> = words.iter().enumerate().map(|(i, &w)| (fnv1a64(packed.name(w).as_bytes()), i as u32)).collect();
    names.sort_unstable();
    b.extend_from_slice(&(names.len() as u32).to_le_bytes());
    for (h, i) in names {
        b.extend_from_slice(&h.to_le_bytes());
        b.extend_from_slice(&i.to_le_bytes());
    }
    for data in [packed.map.bytes(), actions, lists] {
        b.extend_from_slice(&sha1_smol::Sha1::from(data).digest().bytes());
    }
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv_and_lines() {
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(line_offsets(b"ab\ncd\n"), vec![0, 3, 6]);
        assert_eq!(line_offsets(b"ab\ncd"), vec![0, 3, 5]);
        assert_eq!(line_offsets(b""), vec![0]);
    }
}
