//! The oracle (model.py `Oracle`, `FactOracle`): what a path reads when its own store has no write.
//!
//! A fact map, where an absent key is 0, the engine's clocks (facts.CLOCKS) are timers, and a key whose
//! family (model.py `family`) is outside `kinds` (when given) is not held. The prefix listings
//! (`keys(prefix)`) are built once: model.py sorts every listing it uses, so their order here is free.

use rustc_hash::FxHashMap;
use scumm_ir::facts::is_clock;
use scumm_ir::{Fact, FactMap, Slot};
use std::cell::OnceCell;

/// What the oracle answers for a key.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum OVal {
    Int(i32),
    /// `Unknown(R_HELD, key)`: the map does not hold the key's family
    Held,
    /// `Unknown(R_TIMER, key)`: an engine clock
    Timer,
}

/// A key prefix model.py lists the oracle's keys by (its strings: `at:<unit>:`, `esc:<unit>:`, `esc:`,
/// `owner:`, `script:`, `local:`, `objscript:`).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Prefix {
    At(Slot),
    Esc(Slot),
    EscAll,
    Owner,
    Script,
    Local,
    ObjScript,
}

impl Prefix {
    /// Whether key `k` starts with the prefix's string.
    #[inline]
    pub fn matches(self, k: Fact) -> bool {
        match (self, k) {
            (Prefix::At(s), Fact::At(t, _)) => s == t,
            (Prefix::Esc(s), Fact::Esc(t, _)) => s == t,
            (Prefix::EscAll, Fact::Esc(..)) => true,
            (Prefix::Owner, Fact::Owner(_)) => true,
            (Prefix::Script, Fact::Script(_)) => true,
            (Prefix::Local, Fact::Local(_)) => true,
            (Prefix::ObjScript, Fact::ObjScript(_)) => true,
            _ => false,
        }
    }
}

/// The families model.py's `family` gives, as indices.
const FAMILIES: [&str; 25] = [
    "var", "bit", "owner", "state", "class", "room", "ego", "script", "local", "objscript", "at:script", "at:local",
    "at:obj", "at:entry", "at:exit", "frozen", "cutscene", "esc:script", "esc:local", "esc:obj", "esc:entry",
    "esc:exit", "verb", "objpos", "actorroom",
];

fn slot_kind_index(s: Slot) -> usize {
    match s {
        Slot::Script(_) => 0,
        Slot::Local(_) => 1,
        Slot::Obj(_) => 2,
        Slot::Entry => 3,
        Slot::Exit => 4,
    }
}

/// model.py `family(key)` as an index into FAMILIES.
pub fn family_index(k: Fact) -> usize {
    match k {
        Fact::Var(_) => 0,
        Fact::Bit(_) => 1,
        Fact::Owner(_) => 2,
        Fact::State(_) => 3,
        Fact::Class(..) => 4,
        Fact::Room => 5,
        Fact::Ego => 6,
        Fact::Script(_) => 7,
        Fact::Local(_) => 8,
        Fact::ObjScript(_) => 9,
        Fact::At(s, _) => 10 + slot_kind_index(s),
        Fact::Frozen(_) => 15,
        Fact::Cutscene => 16,
        Fact::Esc(s, _) => 17 + slot_kind_index(s),
        Fact::Verb(_) => 22,
        Fact::ObjPos(_) => 23,
        Fact::ActorRoom(_) => 24,
    }
}

/// model.py `FactOracle`.
#[derive(Debug)]
pub struct FactOracle {
    pub facts: FactMap,
    kinds: Option<Vec<String>>,
    tracks: [bool; 25],
    lists: FxHashMap<Prefix, Vec<Fact>>,
    owned: OnceCell<FxHashMap<i32, Vec<i32>>>,
    /// keys read as not held whatever their family (wide.py's synthetic `--unheld` states; empty otherwise)
    unheld: Vec<Fact>,
}

static EMPTY: Vec<Fact> = Vec::new();

impl FactOracle {
    /// The oracle over `facts`; `kinds` the families the map holds (None: all).
    pub fn new(facts: FactMap, kinds: Option<Vec<String>>) -> FactOracle {
        let mut tracks = [true; 25];
        if let Some(ks) = &kinds {
            for (i, fam) in FAMILIES.iter().enumerate() {
                tracks[i] = tracks_family(ks, fam);
            }
        }
        let mut lists: FxHashMap<Prefix, Vec<Fact>> = FxHashMap::default();
        for &k in facts.keys() {
            let mut add = |p: Prefix| lists.entry(p).or_default().push(k);
            match k {
                Fact::At(s, _) => add(Prefix::At(s)),
                Fact::Esc(s, _) => {
                    add(Prefix::Esc(s));
                    add(Prefix::EscAll);
                }
                Fact::Owner(_) => add(Prefix::Owner),
                Fact::Script(_) => add(Prefix::Script),
                Fact::Local(_) => add(Prefix::Local),
                Fact::ObjScript(_) => add(Prefix::ObjScript),
                _ => {}
            }
        }
        FactOracle { facts, kinds, tracks, lists, owned: OnceCell::new(), unheld: Vec::new() }
    }

    /// The same oracle with `keys` read as not held (wide.py's `Unheld` oracle, for synthetic states: model.py has
    /// no such rule). Their listings by prefix are unchanged.
    pub fn with_unheld(mut self, keys: Vec<Fact>) -> FactOracle {
        self.unheld = keys;
        self
    }

    /// `FactOracle.tracks(kind)` for a family string.
    pub fn tracks(&self, family: &str) -> bool {
        match &self.kinds {
            None => true,
            Some(ks) => tracks_family(ks, family),
        }
    }

    /// `FactOracle.get(key)`.
    #[inline]
    pub fn get(&self, k: Fact) -> OVal {
        if !self.unheld.is_empty() && self.unheld.contains(&k) {
            return OVal::Held;
        }
        if !self.tracks[family_index(k)] {
            return OVal::Held;
        }
        if let Fact::Var(n) = k {
            if is_clock(n) {
                return OVal::Timer;
            }
        }
        OVal::Int(self.facts.get(&k).copied().unwrap_or(0))
    }

    /// `FactOracle.keys(prefix)`: the keys with the prefix (in no particular order: every caller sorts).
    pub fn keys(&self, p: Prefix) -> &[Fact] {
        self.lists.get(&p).unwrap_or(&EMPTY)
    }

    /// `FactOracle.owned()`: {owner: its objects} from the owner facts (lists sorted by number here;
    /// model.py's are in key string order, and every caller sorts them again).
    pub fn owned(&self) -> &FxHashMap<i32, Vec<i32>> {
        self.owned.get_or_init(|| {
            let mut out: FxHashMap<i32, Vec<i32>> = FxHashMap::default();
            for &k in self.keys(Prefix::Owner) {
                if let Fact::Owner(o) = k {
                    out.entry(self.facts[&k]).or_default().push(o);
                }
            }
            for v in out.values_mut() {
                v.sort_unstable();
            }
            out
        })
    }
}

/// model.py `FactOracle.tracks` over the family strings `kinds`.
fn tracks_family(kinds: &[String], kind: &str) -> bool {
    kinds.iter().any(|k| {
        k == kind
            || (k.len() > kind.len() && k.starts_with(kind) && k.as_bytes()[kind.len()] == b':')
            || (kind.len() > k.len() && kind.starts_with(k.as_str()) && kind.as_bytes()[k.len()] == b':')
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_rule() {
        let ks: Vec<String> = ["var", "esc:local", "at"].iter().map(|s| s.to_string()).collect();
        assert!(tracks_family(&ks, "var"));
        assert!(tracks_family(&ks, "esc")); // "esc:local" starts with "esc:"
        assert!(!tracks_family(&ks, "esc:obj"));
        assert!(tracks_family(&ks, "at:obj")); // "at:obj" starts with "at:"
        assert!(!tracks_family(&ks, "bit"));
        let mut f = FactMap::default();
        f.insert(Fact::Var(11), 5);
        f.insert(Fact::Var(3), 7);
        let o = FactOracle::new(f, Some(ks));
        assert_eq!(o.get(Fact::Var(11)), OVal::Timer);
        assert_eq!(o.get(Fact::Var(3)), OVal::Int(7));
        assert_eq!(o.get(Fact::Var(4)), OVal::Int(0));
        assert_eq!(o.get(Fact::Bit(4)), OVal::Held);
    }
}
