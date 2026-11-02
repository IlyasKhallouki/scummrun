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
        let n_objs = self.n_objs();
        let mut f: Vec<(Key, Val)> = Vec::with_capacity(self.n_vars() + 3 * n_objs + 256);
        for n in 0..self.n_vars() {
            if !CLOCKS.contains(&n) {
                f.push((Key::var(n as u64), self.var(n) as Val));
            }
        }
        for (i, &byte) in self.bits.iter().enumerate() {
            if byte == 0 {
                continue;
            }
            for k in 0..8 {
                let n = i * 8 + k;
                if byte & (1 << k) != 0 && (n as u32) < self.n_bits {
                    f.push((Key::bit(n as u64), 1));
                }
            }
        }
        for o in 1..n_objs {
            f.push((Key::owner(o as u64), self.owner[o] as Val));
        }
        for o in 1..n_objs {
            f.push((Key::state(o as u64), self.state[o] as Val));
        }
        for o in 1..n_objs {
            let c = self.class_of(o);
            if c != 0 {
                for bit in 0..32 {
                    if c & (1 << bit) != 0 {
                        f.push((Key::class(o as u64, bit as u64 + 1), 1));
                    }
                }
            }
        }
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
        debug_assert!(f.windows(2).all(|w| w[0].0 < w[1].0));
        Facts(f)
    }
}

/// A StepReply's block field: kind 0 is a whole block (u32 length, bytes); kind 1 a delta from `last` (u32 new length,
/// u32 chunk count, then u32 offset, u32 length and the bytes per chunk). `last` becomes the new block.
pub fn apply(last: &mut Vec<u8>, r: &mut crate::wire::R) -> Result<(), String> {
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
            }
        }
        v => return Err(format!("block kind {v}")),
    }
    Ok(())
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
        apply(&mut last, &mut r).unwrap();
        assert_eq!(last, vec![1, 9, 3, 4, 5, 6, 7]);
        assert!(r.done().is_ok());
    }
}
