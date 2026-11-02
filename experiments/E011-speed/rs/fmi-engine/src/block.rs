//! F2, the state block (plan.md "Binary formats"): what `facts_of` reads, as flat arrays; its facts, built in `Key`
//! order with no string and no full sort; and the delta a StepReply carries.

use fmi_core::*;

pub const MAGIC: u32 = 0x3153_4D46; // "FMS1"
pub const VERSION: u32 = 1;

/// One live slot as the block holds it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SlotRow {
    pub slot: i32,
    pub number: i32,
    pub where_: i32,
    pub status: i32,
    pub code: i64,
    pub frozen: i32,
}

/// The top level's override as the block holds it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Top {
    pub where_: i32,
    pub number: i32,
    pub code: i64,
    pub stale: bool,
}

/// A parsed block: borrowed arrays and the small lists.
#[derive(Debug)]
pub struct Block<'a> {
    pub frame: u64,
    pub tick: u64,
    pub rng: u64,
    pub room: i32,
    pub ego: i32,
    pub depth: i32,
    /// n_vars little-endian i32s
    pub vars: &'a [u8],
    pub n_bits: u32,
    pub bits: &'a [u8],
    pub owner: &'a [u8],
    pub state: &'a [u8],
    /// n_objs little-endian u32s
    pub class: &'a [u8],
    pub slots: Vec<SlotRow>,
    pub top: Option<Top>,
    pub verbs: Vec<i32>,
}

fn rd32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

impl<'a> Block<'a> {
    pub fn n_vars(&self) -> usize {
        self.vars.len() / 4
    }
    pub fn var(&self, i: usize) -> i32 {
        rd32(self.vars, 4 * i) as i32
    }
    pub fn n_objs(&self) -> usize {
        self.owner.len()
    }
    pub fn class_of(&self, o: usize) -> u32 {
        rd32(self.class, 4 * o)
    }

    pub fn parse(buf: &'a [u8]) -> Result<Block<'a>, String> {
        let mut p = 0usize;
        let need = |p: usize, n: usize| -> Result<(), String> {
            if buf.len() < p + n {
                Err(format!("F2 block ends at {} (needs {})", buf.len(), p + n))
            } else {
                Ok(())
            }
        };
        need(p, 48)?;
        if rd32(buf, 0) != MAGIC || rd32(buf, 4) != VERSION {
            return Err("not an F2 block (magic or version)".into());
        }
        let u64at = |at: usize| u64::from_le_bytes(buf[at..at + 8].try_into().unwrap());
        let frame = u64at(8);
        let tick = u64at(16);
        let rng = u64at(24);
        let room = rd32(buf, 32) as i32;
        let ego = rd32(buf, 36) as i32;
        let depth = rd32(buf, 40) as i32;
        let n_vars = rd32(buf, 44) as usize;
        p = 48;
        need(p, n_vars * 4 + 4)?;
        let vars = &buf[p..p + n_vars * 4];
        p += n_vars * 4;
        let n_bits = rd32(buf, p);
        p += 4;
        let nb = (n_bits as usize).div_ceil(8);
        need(p, nb + 4)?;
        let bits = &buf[p..p + nb];
        p += nb;
        let n_objs = rd32(buf, p) as usize;
        p += 4;
        need(p, n_objs * 6 + 4)?;
        let owner = &buf[p..p + n_objs];
        let state = &buf[p + n_objs..p + 2 * n_objs];
        let class = &buf[p + 2 * n_objs..p + 6 * n_objs];
        p += 6 * n_objs;
        let n_slots = rd32(buf, p) as usize;
        p += 4;
        need(p, n_slots * 28 + 1)?;
        let mut slots = Vec::with_capacity(n_slots);
        for _ in 0..n_slots {
            slots.push(SlotRow {
                slot: rd32(buf, p) as i32,
                number: rd32(buf, p + 4) as i32,
                where_: rd32(buf, p + 8) as i32,
                status: rd32(buf, p + 12) as i32,
                code: u64at(p + 16) as i64,
                frozen: rd32(buf, p + 24) as i32,
            });
            p += 28;
        }
        let has_top = buf[p];
        p += 1;
        let top = match has_top {
            0 => None,
            1 => {
                need(p, 17)?;
                let t = Top {
                    where_: rd32(buf, p) as i32,
                    number: rd32(buf, p + 4) as i32,
                    code: u64at(p + 8) as i64,
                    stale: match buf[p + 16] {
                        0 => false,
                        1 => true,
                        v => return Err(format!("F2 stale {v}")),
                    },
                };
                p += 17;
                Some(t)
            }
            v => return Err(format!("F2 override tag {v}")),
        };
        need(p, 4)?;
        let n_verbs = rd32(buf, p) as usize;
        p += 4;
        need(p, n_verbs * 4)?;
        let verbs = (0..n_verbs).map(|i| rd32(buf, p + 4 * i) as i32).collect();
        p += n_verbs * 4;
        if p != buf.len() {
            return Err(format!("F2 block has {} bytes left over", buf.len() - p));
        }
        Ok(Block {
            frame,
            tick,
            rng,
            room,
            ego,
            depth,
            vars,
            n_bits,
            bits,
            owner,
            state,
            class,
            slots,
            top,
            verbs,
        })
    }

    /// `facts_of` of the dump this block was taken with (the same `Facts`, pair for pair).
    pub fn facts(&self) -> Facts {
        let mut f = self.prefix();
        self.push_tail(&mut f);
        debug_assert!(f.windows(2).all(|w| w[0].0 < w[1].0));
        Facts(f)
    }

    /// The facts before the tail: vars, bits, owners, states, classes (`Key` order).
    fn prefix(&self) -> Vec<(Key, Val)> {
        let n_objs = self.n_objs();
        let set_bits: usize = self.bits.iter().map(|b| b.count_ones() as usize).sum();
        let classes: usize = self
            .class
            .chunks_exact(4)
            .map(|c| rd32(c, 0).count_ones() as usize)
            .sum();
        let mut f: Vec<(Key, Val)> =
            Vec::with_capacity(self.n_vars() + set_bits + 2 * n_objs + classes + 4 * self.slots.len() + 64);
        // the clocks (facts.py CLOCKS) are all below 64: a mask
        let clocks: u64 = CLOCKS.iter().fold(0, |m, &c| m | (1u64 << c));
        for (n, c) in self.vars.chunks_exact(4).enumerate() {
            if n >= 64 || clocks & (1u64 << n) == 0 {
                f.push((Key::var(n as u64), i32::from_le_bytes(c.try_into().unwrap()) as Val));
            }
        }
        for (i, &byte) in self.bits.iter().enumerate() {
            let mut b = byte;
            while b != 0 {
                let n = i * 8 + b.trailing_zeros() as usize;
                if (n as u32) < self.n_bits {
                    f.push((Key::bit(n as u64), 1));
                }
                b &= b - 1;
            }
        }
        f.extend(
            self.owner
                .iter()
                .enumerate()
                .skip(1)
                .map(|(o, &v)| (Key::owner(o as u64), v as Val)),
        );
        f.extend(
            self.state
                .iter()
                .enumerate()
                .skip(1)
                .map(|(o, &v)| (Key::state(o as u64), v as Val)),
        );
        for (o, c) in self.class.chunks_exact(4).enumerate().skip(1) {
            let mut c = rd32(c, 0);
            while c != 0 {
                f.push((Key::class(o as u64, c.trailing_zeros() as u64 + 1), 1));
                c &= c - 1;
            }
        }
        f
    }

    /// The tail: room, ego, the slot keys, the cutscene depth, the override, the verbs (`Key` order).
    fn push_tail(&self, f: &mut Vec<(Key, Val)>) {
        f.push((Key::pack(K_ROOM, 0, 0, 0), self.room as Val));
        f.push((Key::pack(K_EGO, 0, 0, 0), self.ego as Val));
        // the slot keys: kinds 8 to 12 in order, each list sorted and without repeats (value 1)
        let mut tail: Vec<Key> = Vec::with_capacity(4 * self.slots.len() + self.verbs.len() + 1);
        for sl in &self.slots {
            let room_code = sl.number == ENCD || sl.number == EXCD;
            match sl.where_ {
                WIO_GLOBAL => tail.push(Key::pack(K_SCRIPT, 0, sl.number as u64, 0)),
                WIO_LOCAL => tail.push(Key::pack(K_LOCAL, 0, sl.number as u64, 0)),
                WIO_INVENTORY | WIO_ROOM | WIO_FLOBJECT if !room_code => {
                    tail.push(Key::pack(K_OBJSCRIPT, 0, sl.number as u64, 0))
                }
                _ => {}
            }
            if let Some((u, a)) = unit_of(sl.where_, sl.number) {
                if sl.code >= 0 {
                    tail.push(Key::pack(K_AT, u, a, sl.code as u64));
                }
                if sl.frozen > 0 {
                    tail.push(Key::pack(K_FROZEN, u, a, 0));
                }
            }
        }
        tail.sort_unstable();
        tail.dedup();
        f.extend(tail.iter().map(|&k| (k, 1)));
        f.push((Key::pack(K_CUTSCENE, 0, 0, 0), self.depth as Val));
        if let Some(t) = self.top {
            if !t.stale && t.code >= 0 {
                if let Some((u, a)) = unit_of(t.where_, t.number) {
                    f.push((Key::pack(K_ESC, u, a, t.code as u64), 1));
                }
            }
        }
        let mut verbs: Vec<Key> = self.verbs.iter().map(|&v| Key::verb(v as u64)).collect();
        verbs.sort_unstable();
        verbs.dedup();
        f.extend(verbs.iter().map(|&k| (k, 1)));
    }
}

/// The byte layout of a block's fixed regions (offsets into it).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Layout {
    n_vars: usize,
    n_bits: u32,
    n_objs: usize,
    bits: usize,
    owner: usize,
    state: usize,
    class: usize,
    end: usize,
}

/// The layout of a block (None if it is not one).
pub fn layout(b: &[u8]) -> Option<Layout> {
    if b.len() < 48 || rd32(b, 0) != MAGIC {
        return None;
    }
    let n_vars = rd32(b, 44) as usize;
    let nb_at = 48 + 4 * n_vars;
    if b.len() < nb_at + 4 {
        return None;
    }
    let n_bits = rd32(b, nb_at);
    let bits = nb_at + 4;
    let no_at = bits + (n_bits as usize).div_ceil(8);
    if b.len() < no_at + 4 {
        return None;
    }
    let n_objs = rd32(b, no_at) as usize;
    let owner = no_at + 4;
    let state = owner + n_objs;
    let class = state + n_objs;
    let end = class + 4 * n_objs;
    if b.len() < end {
        return None;
    }
    Some(Layout {
        n_vars,
        n_bits,
        n_objs,
        bits,
        owner,
        state,
        class,
        end,
    })
}

/// Brings `facts` (those of a block of layout `old`) to those of `new`, given the byte ranges (offset, length) where
/// `new` was written over that block (a delta's chunks): var, owner and state values in place, the tail (room, ego,
/// slots, cutscene, override, verbs) rebuilt. False when it cannot (the layout differs, or a bit, class or count
/// changed): the caller then builds the facts whole. Equal to `Block::parse(new).facts()` whenever it returns true.
pub fn update_facts(facts: &mut Facts, old: Option<Layout>, new: &[u8], ranges: &[(usize, usize)]) -> bool {
    let (Some(lo), Some(ln)) = (old, layout(new)) else {
        return false;
    };
    if lo != ln {
        return false;
    }
    let l = ln;
    let clocks: u64 = CLOCKS.iter().fold(0, |m, &c| m | (1u64 << c));
    let skipped_below = |n: usize| (clocks & ((1u64 << n.min(63)) - 1)).count_ones() as usize;
    let n_var_facts = l.n_vars - CLOCKS.iter().filter(|&&c| c < l.n_vars).count();
    let set_bits: usize = new[l.bits..l.owner - 4].iter().map(|b| b.count_ones() as usize).sum();
    let owners_at = n_var_facts + set_bits;
    let states_at = owners_at + l.n_objs.saturating_sub(1);
    let classes: usize = new[l.class..l.end]
        .chunks_exact(4)
        .skip(1)
        .map(|c| rd32(c, 0).count_ones() as usize)
        .sum();
    let prefix = states_at + l.n_objs.saturating_sub(1) + classes;
    if facts.0.len() < prefix {
        return false;
    }
    let mut tail = false;
    for &(off, len) in ranges {
        let end = off + len;
        if off < 48 && end > 32 {
            tail = true; // room, ego, depth (the frame, tick and rng before them are no facts)
        }
        if off < 48 + 4 * l.n_vars && end > 44 {
            // the var count or values
            if off < 48 {
                return false;
            }
            let first = (off - 48) / 4;
            let last = ((end - 48).div_ceil(4)).min(l.n_vars);
            for n in first..last {
                if n < 64 && clocks & (1u64 << n) != 0 {
                    continue;
                }
                let v = rd32(new, 48 + 4 * n) as i32 as Val;
                facts.0[n - skipped_below(n)].1 = v;
            }
        }
        if off < l.owner && end > l.bits - 4 {
            return false; // a bit or the counts
        }
        if off < l.class && end > l.owner {
            let first = off.max(l.owner);
            for (i, &v) in new[first..end.min(l.class)].iter().enumerate().map(|(k, v)| (first + k, v)) {
                let (at, o) = if i < l.state {
                    (owners_at, i - l.owner)
                } else {
                    (states_at, i - l.state)
                };
                if o >= 1 {
                    facts.0[at + o - 1].1 = v as Val;
                }
            }
        }
        if off < l.end && end > l.class {
            return false; // a class
        }
        if end > l.end {
            tail = true;
        }
    }
    if tail {
        let Ok(b) = Block::parse(new) else { return false };
        facts.0.truncate(prefix);
        b.push_tail(&mut facts.0);
    }
    true
}

/// A StepReply's block field: kind 0 is a whole block (u32 length, bytes); kind 1 a delta from `last` (u32 new length,
/// u32 chunk count, then u32 offset, u32 length and the bytes per chunk). `last` becomes the new block; `ranges` gets
/// the delta's chunks. True for a delta, false for a whole block.
pub fn apply(last: &mut Vec<u8>, r: &mut crate::wire::R, ranges: &mut Vec<(usize, usize)>) -> Result<bool, String> {
    ranges.clear();
    match r.u8()? {
        0 => {
            let b = r.bytes()?;
            last.clear();
            last.extend_from_slice(b);
        }
        1 => {
            let n = r.u32()? as usize;
            let chunks = r.count(8)?;
            last.resize(n, 0);
            for _ in 0..chunks {
                let off = r.u32()? as usize;
                let len = r.u32()? as usize;
                if off + len > n {
                    return Err(format!("delta chunk {off}+{len} past the block's {n} bytes"));
                }
                let bytes = r.rest().get(..len).ok_or("delta chunk past the payload")?;
                last[off..off + len].copy_from_slice(bytes);
                r.pos += len;
                ranges.push((off, len));
            }
            return Ok(true);
        }
        v => return Err(format!("block kind {v}")),
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(vars: &[i32], bits: &[u8], objs: usize, slots: &[SlotRow], top: Option<Top>, verbs: &[i32]) -> Vec<u8> {
        let mut b = Vec::new();
        let p32 = |b: &mut Vec<u8>, v: u32| b.extend_from_slice(&v.to_le_bytes());
        p32(&mut b, MAGIC);
        p32(&mut b, VERSION);
        b.extend_from_slice(&5u64.to_le_bytes());
        b.extend_from_slice(&9u64.to_le_bytes());
        b.extend_from_slice(&77u64.to_le_bytes());
        p32(&mut b, 33);
        p32(&mut b, 1);
        p32(&mut b, 0);
        p32(&mut b, vars.len() as u32);
        for &v in vars {
            p32(&mut b, v as u32);
        }
        p32(&mut b, (bits.len() * 8) as u32);
        b.extend_from_slice(bits);
        p32(&mut b, objs as u32);
        b.extend((0..objs).map(|i| (i % 3) as u8));
        b.extend((0..objs).map(|i| (i % 2) as u8));
        for i in 0..objs {
            p32(&mut b, if i == 2 { 0b101 } else { 0 });
        }
        p32(&mut b, slots.len() as u32);
        for s in slots {
            for v in [s.slot, s.number, s.where_, s.status] {
                p32(&mut b, v as u32);
            }
            b.extend_from_slice(&s.code.to_le_bytes());
            p32(&mut b, s.frozen as u32);
        }
        match top {
            None => b.push(0),
            Some(t) => {
                b.push(1);
                p32(&mut b, t.where_ as u32);
                p32(&mut b, t.number as u32);
                b.extend_from_slice(&t.code.to_le_bytes());
                b.push(t.stale as u8);
            }
        }
        p32(&mut b, verbs.len() as u32);
        for &v in verbs {
            p32(&mut b, v as u32);
        }
        b
    }

    #[test]
    fn facts_equal_facts_of() {
        let slots = [
            SlotRow {
                slot: 0,
                number: 202,
                where_: WIO_LOCAL,
                status: 2,
                code: 21,
                frozen: 1,
            },
            SlotRow {
                slot: 1,
                number: 7,
                where_: WIO_GLOBAL,
                status: 2,
                code: -1,
                frozen: 0,
            },
            SlotRow {
                slot: 3,
                number: 7,
                where_: WIO_GLOBAL,
                status: 2,
                code: 4,
                frozen: 0,
            },
            SlotRow {
                slot: 4,
                number: ENCD,
                where_: WIO_ROOM,
                status: 2,
                code: 3,
                frozen: 0,
            },
            SlotRow {
                slot: 5,
                number: 428,
                where_: WIO_ROOM,
                status: 2,
                code: 105,
                frozen: 2,
            },
        ];
        let top = Some(Top {
            where_: WIO_ROOM,
            number: ENCD,
            code: 4,
            stale: false,
        });
        let vars: Vec<i32> = (0..60).map(|i| i * 3 - 7).collect();
        let raw = block(&vars, &[0b1001_0000, 0, 1], 5, &slots, top, &[9, 2, 9]);
        let b = Block::parse(&raw).unwrap();
        let obs = Obs {
            room: 33,
            ego: Some(1),
            vars: vars.clone(),
            bits_set: vec![4, 7, 16],
            owners: (1..5i32).map(|o| (o as u32, o % 3)).collect(),
            states: (1..5i32).map(|o| (o as u32, o % 2)).collect(),
            classes: vec![(2, vec![1, 3])],
            slots: slots
                .iter()
                .map(|s| Slot {
                    slot: s.slot,
                    number: s.number,
                    where_: s.where_,
                    status: s.status,
                    code: s.code,
                    frozen: s.frozen,
                    ..Default::default()
                })
                .collect(),
            verbs: [9, 2, 9]
                .iter()
                .map(|&id| Verb {
                    id,
                    ..Default::default()
                })
                .collect(),
            cutscene: Cutscene {
                depth: 0,
                levels: vec![CutLevel {
                    level: 0,
                    data: None,
                    override_: top.map(|t| Override {
                        where_: t.where_,
                        number: t.number,
                        code: t.code,
                        stale: t.stale,
                        ..Default::default()
                    }),
                }],
            },
            ..Default::default()
        };
        assert_eq!(b.facts(), facts_of(&obs));
    }

    /// The byte ranges where two blocks differ, as the engine's delta writes them (length changes as one tail chunk).
    fn ranges_of(old: &[u8], new: &[u8]) -> Vec<(usize, usize)> {
        let m = old.len().min(new.len());
        let mut v: Vec<(usize, usize)> = (0..m).filter(|&i| old[i] != new[i]).map(|i| (i, 1)).collect();
        if new.len() > m {
            v.push((m, new.len() - m));
        }
        v
    }

    #[test]
    fn facts_follow_a_delta() {
        let s0 = [SlotRow {
            slot: 0,
            number: 202,
            where_: WIO_LOCAL,
            status: 2,
            code: 21,
            frozen: 0,
        }];
        let s1 = [
            s0[0],
            SlotRow {
                slot: 1,
                number: 7,
                where_: WIO_GLOBAL,
                status: 2,
                code: 4,
                frozen: 1,
            },
        ];
        let vars: Vec<i32> = (0..60).collect();
        let base = block(&vars, &[0b1001_0000, 0, 1], 5, &s0, None, &[9]);
        let mut v2 = vars.clone();
        v2[3] = -5;
        v2[12] = 99; // a clock: no fact
        v2[59] = 1 << 20;
        let cases: Vec<(Vec<u8>, bool)> = vec![
            (block(&v2, &[0b1001_0000, 0, 1], 5, &s0, None, &[9]), true),
            (block(&vars, &[0b1001_0000, 0, 1], 5, &s1, None, &[9, 3]), true),
            (
                block(
                    &vars,
                    &[0b1001_0000, 0, 1],
                    5,
                    &s0,
                    Some(Top {
                        where_: WIO_LOCAL,
                        number: 202,
                        code: 3,
                        stale: false,
                    }),
                    &[],
                ),
                true,
            ),
            (block(&vars, &[0b1001_0001, 0, 1], 5, &s0, None, &[9]), false),
        ];
        for (new, ok) in cases {
            let mut f = Block::parse(&base).unwrap().facts();
            let r = ranges_of(&base, &new);
            assert_eq!(update_facts(&mut f, layout(&base), &new, &r), ok);
            if ok {
                assert_eq!(f, Block::parse(&new).unwrap().facts());
            }
        }
        // an owner and a state, written into the base block's bytes
        let l = layout(&base).unwrap();
        let mut new = base.clone();
        new[l.owner + 3] = 7;
        new[l.state + 4] = 9;
        let mut f = Block::parse(&base).unwrap().facts();
        assert!(update_facts(&mut f, Some(l), &new, &ranges_of(&base, &new)));
        assert_eq!(f, Block::parse(&new).unwrap().facts());
        // a class
        let mut new = base.clone();
        new[l.class + 4 * 3] = 1;
        let mut f = Block::parse(&base).unwrap().facts();
        assert!(!update_facts(&mut f, Some(l), &new, &ranges_of(&base, &new)));
    }

    #[test]
    fn delta_round_trip() {
        let old = vec![1u8, 2, 3, 4, 5];
        let mut msg = vec![1u8];
        msg.extend_from_slice(&7u32.to_le_bytes());
        msg.extend_from_slice(&2u32.to_le_bytes());
        msg.extend_from_slice(&1u32.to_le_bytes());
        msg.extend_from_slice(&1u32.to_le_bytes());
        msg.push(9);
        msg.extend_from_slice(&5u32.to_le_bytes());
        msg.extend_from_slice(&2u32.to_le_bytes());
        msg.extend_from_slice(&[6, 7]);
        let mut last = old.clone();
        let mut r = crate::wire::R::new(&msg);
        let mut ranges = Vec::new();
        assert!(apply(&mut last, &mut r, &mut ranges).unwrap());
        assert_eq!(ranges, vec![(1, 1), (5, 2)]);
        assert_eq!(last, vec![1, 9, 3, 4, 5, 6, 7]);
        assert!(r.done().is_ok());
    }
}
