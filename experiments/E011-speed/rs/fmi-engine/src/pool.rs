//! The kept-entry pool (plan.md "Memory layout", engine): an entry id maps to a stopped engine process at the entry's
//! decision point, so a return is a fork of it (A0's snapshot) instead of a replay from boot. Entries are evicted
//! least recently used when MemAvailable falls under the guard's floor plus 1 GB; an evicted (or never kept) entry is
//! made again from its nearest kept ancestor by replaying the rest of its movie, which is exact by A0.

use crate::host::{BinEngine, Host};
use crate::sys;
use fmi_core::*;
use std::collections::HashMap;
use std::path::PathBuf;

/// An entry as the pool needs it: its parent, its frame and its movie from boot (absolute frames). The movie up to
/// the parent's frame must be the parent's movie.
#[derive(Clone, Debug)]
pub struct EntrySpec {
    pub id: u64,
    pub parent: Option<u64>,
    pub frame: u64,
    pub events: Vec<Event>,
}

struct Kept {
    engine: BinEngine,
    used: u64,
}

/// What the pool did, for the benchmark and the explorer's log.
#[derive(Clone, Debug, Default)]
pub struct PoolStats {
    pub returns: u64,
    pub made_from_boot: u64,
    pub made_from_ancestor: u64,
    pub replayed_frames: u64,
    pub evicted: u64,
}

pub struct Pool {
    host: Host,
    seed: u32,
    dump_locals: bool,
    lean: bool,
    out_dir: PathBuf,
    specs: HashMap<u64, EntrySpec>,
    kept: HashMap<u64, Kept>,
    clock: u64,
    /// evict while MemAvailable is under this (kB)
    pub level_kb: u64,
    pub stats: PoolStats,
}

/// The guard's floor (guard.sh: 3 GB of MemAvailable) plus 1 GB.
pub const LEVEL_KB: u64 = (3 + 1) * 1024 * 1024;

impl Pool {
    pub fn new(host: Host, seed: u32, dump_locals: bool, lean: bool, out_dir: PathBuf) -> Pool {
        Pool {
            host,
            seed,
            dump_locals,
            lean,
            out_dir,
            specs: HashMap::new(),
            kept: HashMap::new(),
            clock: 0,
            level_kb: LEVEL_KB,
            stats: PoolStats::default(),
        }
    }

    /// Registers an entry; it is kept on its first return.
    pub fn add(&mut self, spec: EntrySpec) {
        self.specs.insert(spec.id, spec);
    }

    /// Keeps `engine` as entry `id` (registered). The caller vouches that it stands at the entry's decision point
    /// with no input scheduled after it (a fork of the play engine at a checkpoint, before any later input).
    pub fn keep(&mut self, id: u64, engine: BinEngine) {
        self.clock += 1;
        self.kept.insert(
            id,
            Kept {
                engine,
                used: self.clock,
            },
        );
        self.evict_to_level();
    }

    pub fn is_kept(&self, id: u64) -> bool {
        self.kept.contains_key(&id)
    }

    pub fn n_kept(&self) -> usize {
        self.kept.len()
    }

    /// A return to entry `id`: a fork of its kept process, made first if it is not kept.
    pub fn ret(&mut self, id: u64) -> EResult<BinEngine> {
        if !self.kept.contains_key(&id) {
            self.make(id)?;
        }
        self.clock += 1;
        let k = self.kept.get_mut(&id).unwrap();
        k.used = self.clock;
        self.stats.returns += 1;
        let child = k.engine.fork_bin()?;
        self.evict_to_level();
        Ok(child)
    }

    /// The nearest kept ancestor whose movie is a prefix of the entry's.
    fn ancestor(&self, spec: &EntrySpec) -> Option<u64> {
        let mut at = spec.parent;
        let mut seen = std::collections::HashSet::from([spec.id]);
        while let Some(a) = at {
            if !seen.insert(a) {
                return None; // a cycle in the parents: no ancestor
            }
            let s = self.specs.get(&a)?;
            if self.kept.contains_key(&a) && s.frame <= spec.frame {
                let mine: Vec<&Event> = spec.events.iter().filter(|e| e.frame <= s.frame).collect();
                let theirs: Vec<&Event> = s.events.iter().filter(|e| e.frame <= s.frame).collect();
                if mine == theirs {
                    return Some(a);
                }
            }
            at = s.parent;
        }
        None
    }

    fn make(&mut self, id: u64) -> EResult<()> {
        let spec = self
            .specs
            .get(&id)
            .cloned()
            .ok_or_else(|| EngineError::Bridge(format!("no entry {id}")))?;
        let engine = match self.ancestor(&spec) {
            Some(a) => {
                let from = self.specs[&a].frame;
                let k = self.kept.get_mut(&a).unwrap();
                let mut e = k.engine.fork_bin()?;
                let mut rest: Vec<Event> = spec
                    .events
                    .iter()
                    .filter(|e| e.frame > from && e.frame <= spec.frame)
                    .cloned()
                    .collect();
                rest.sort_by_key(|e| e.frame); // stable, as replay.write_events
                e.inputs(&rest)?;
                if spec.frame > from {
                    let st = e.run(&RunSpec {
                        frame: Some(spec.frame),
                        ..Default::default()
                    })?;
                    if st.frame != spec.frame {
                        return Err(EngineError::Bridge(format!("entry {id}: stopped at {}", st.frame)));
                    }
                }
                self.stats.made_from_ancestor += 1;
                self.stats.replayed_frames += spec.frame - from;
                e
            }
            None => {
                let cfg = BootCfg {
                    seed: self.seed,
                    events: spec.events.clone(),
                    frame: Some(spec.frame),
                    dump_locals: self.dump_locals,
                    lean: self.lean,
                    out_dir: self.out_dir.join(format!("boot-{id}")),
                };
                self.stats.made_from_boot += 1;
                self.stats.replayed_frames += spec.frame;
                self.host.boot_bin(&cfg)?
            }
        };
        self.clock += 1;
        self.kept.insert(
            id,
            Kept {
                engine,
                used: self.clock,
            },
        );
        Ok(())
    }

    /// Quits kept entries, least recently used first, while MemAvailable is under the level (one is always left).
    pub fn evict_to_level(&mut self) {
        while self.kept.len() > 1 {
            match sys::mem_available_kb() {
                Some(kb) if kb < self.level_kb => {}
                _ => return,
            }
            self.evict_one();
        }
    }

    /// Quits the least recently used kept entry; false if none is kept.
    pub fn evict_one(&mut self) -> bool {
        let Some((&id, _)) = self.kept.iter().min_by_key(|(_, k)| k.used) else {
            return false;
        };
        self.kept.remove(&id);
        self.stats.evicted += 1;
        true
    }

    /// Drops entry `id` from the pool (its process quits).
    pub fn forget(&mut self, id: u64) {
        self.kept.remove(&id);
        self.specs.remove(&id);
    }
}
