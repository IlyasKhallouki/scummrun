//! A reading's input actions as packed_reading.PackedTask._action gives them (the paths `predict` chooses among and
//! the harness plays), read on demand from the packed file on disk: the planner keeps no copy of it.
//!
//! The planner port is made from a `&dyn ReadingSource` it cannot keep, so the planner reads reading.tc itself (the
//! file the reading's id is the sha1 of), one action at a time at its word offset, and decodes it as `Packed.raw_action`
//! and `PackedTask._action` do: precondition sets as written (not cut to the domain), set, guess and add effects with
//! their conditions, the name `label#rec` (or the label), the cost; `what` is the action's index.

use fmi_core::{Action, EffVal, Effect, Key, Val, ABOVE, BELOW};
use std::fs::File;
use std::os::unix::fs::FileExt;
use std::path::Path;

const OP_SET: u32 = 0;
const OP_GUESS: u32 = 1;
const OP_ADD: u32 = 2;
const NO_REC: u32 = u32::MAX;

pub struct Raw {
    file: File,
    /// word offset of each action's length word
    offsets: Vec<u64>,
    /// string id -> its value (packed_reading `_value`: an int, "below", "above"), None for another text
    vals: Vec<Option<Val>>,
    strs: Vec<String>,
    /// fact id -> its key
    keys: Vec<Key>,
}

/// packed_reading `_value` as a `Val`.
pub fn value_of(text: &str) -> Option<Val> {
    match text {
        "below" => Some(BELOW),
        "above" => Some(ABOVE),
        t => t.parse::<i64>().ok(),
    }
}

fn word(b: &[u8], w: usize) -> u32 {
    u32::from_le_bytes([b[4 * w], b[4 * w + 1], b[4 * w + 2], b[4 * w + 3]])
}

impl Raw {
    /// From the file's bytes (to index the actions; not kept), its path (read again per action), and its tables as
    /// taskcore loaded them (strings, and each fact's key string id).
    pub fn new(path: &Path, bytes: &[u8], strs: &[String], fact_keys: &[u32]) -> Result<Raw, String> {
        let nw = bytes.len() / 4;
        if nw < 10 {
            return Err(format!("{}: too short", path.display()));
        }
        let foot = nw - 8;
        let (body, n) = (word(bytes, foot) as usize, word(bytes, foot + 1) as usize);
        let mut offsets = Vec::with_capacity(n);
        let mut p = body;
        for _ in 0..n {
            offsets.push(p as u64);
            p += 1 + word(bytes, p) as usize;
        }
        let file = File::open(path).map_err(|e| format!("{}: {}", path.display(), e))?;
        let len = file.metadata().map_err(|e| e.to_string())?.len();
        if len != bytes.len() as u64 {
            return Err(format!("{}: {} bytes on disk, {} in the reading", path.display(), len, bytes.len()));
        }
        Ok(Raw {
            file,
            offsets,
            vals: strs.iter().map(|s| value_of(s)).collect(),
            strs: strs.to_vec(),
            keys: fact_keys.iter().map(|&k| Key::parse(&strs[k as usize])).collect(),
        })
    }

    pub fn n(&self) -> usize {
        self.offsets.len()
    }

    fn words(&self, i: u32) -> Result<Vec<u32>, String> {
        let off = *self.offsets.get(i as usize).ok_or(format!("no action {}", i))?;
        let mut b4 = [0u8; 4];
        self.file.read_exact_at(&mut b4, off * 4).map_err(|e| e.to_string())?;
        let n = u32::from_le_bytes(b4) as usize;
        let mut b = vec![0u8; 4 * n];
        self.file.read_exact_at(&mut b, off * 4 + 4).map_err(|e| e.to_string())?;
        Ok(b.chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect())
    }

    fn val(&self, s: u32) -> Result<Val, String> {
        self.vals[s as usize].ok_or_else(|| format!("a value {:?} that is no int, below or above", self.strs[s as usize]))
    }

    fn sets(&self, w: &[u32], p: &mut usize, n: u32) -> Result<Vec<(Key, Vec<Val>)>, String> {
        let mut out = Vec::with_capacity(n as usize);
        for _ in 0..n {
            let (f, m) = (w[*p], w[*p + 1] as usize);
            let mut vs: Vec<Val> = w[*p + 2..*p + 2 + m].iter().map(|&v| self.val(v)).collect::<Result<_, _>>()?;
            vs.sort_unstable();
            vs.dedup();
            out.push((self.keys[f as usize], vs));
            *p += 2 + m;
        }
        Ok(out)
    }

    /// The action's label (its record's label).
    pub fn label(&self, i: u32) -> Result<String, String> {
        let w = self.words(i)?;
        Ok(self.strs[w[1] as usize].clone())
    }

    /// PackedTask._action(i) (module docstring).
    pub fn action(&self, i: u32) -> Result<Action, String> {
        let w = self.words(i)?;
        let (rec, label, cost, npre) = (w[0], w[1], w[5], w[6]);
        let mut p = 7;
        let pre = self.sets(&w, &mut p, npre)?;
        let ne = w[p];
        p += 1;
        let mut effects = Vec::with_capacity(ne as usize);
        for _ in 0..ne {
            let (op, f) = (w[p], w[p + 1]);
            let v = match op {
                OP_SET => {
                    p += 3;
                    EffVal::Value(self.val(w[p - 1])?)
                }
                OP_GUESS => {
                    let m = w[p + 2] as usize;
                    let mut vs: Vec<Val> = w[p + 3..p + 3 + m].iter().map(|&v| self.val(v)).collect::<Result<_, _>>()?;
                    vs.sort_unstable();
                    vs.dedup();
                    p += 3 + m;
                    EffVal::Values(vs)
                }
                OP_ADD => {
                    p += 3;
                    EffVal::Add(w[p - 1] as i32 as i64)
                }
                x => return Err(format!("action {}: effect op {}", i, x)),
            };
            let nw = w[p];
            p += 1;
            let when = self.sets(&w, &mut p, nw)?;
            effects.push(Effect { key: self.keys[f as usize], v, when });
        }
        let l = &self.strs[label as usize];
        let name = if rec == NO_REC { l.clone() } else { format!("{}#{}", l, rec) };
        Ok(Action { name, pre, effects, what: i, cost: cost as i64 })
    }
}
