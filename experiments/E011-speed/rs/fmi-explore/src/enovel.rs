//! E015 G2 (experiments/E015-engine-gates/plan.md, "G2: novelty over engine-visible state"): what the engine shows at
//! a stop, as atoms of four kinds next to T, and afford's per-stop observer. A measurement only: switch
//! FMI_NOVEL_ENGINE=1, off by default; with it off nothing here runs, and with it on the observer reads each trial's
//! stop on a fork of the trial's engine and changes no trial and no keep.
//!
//! The kinds (fixed by the plan before any result):
//! * U, live units: `live:<room>:<kind>:<n>` for each slot alive (fmi-core's K_SCRIPT, K_LOCAL, K_OBJSCRIPT keys; a
//!   room's entry and exit code are left out, as `facts_of` leaves them out);
//! * W, wait offsets: `at:<room>:<unit>:<offset>` (K_AT keys; the entry and exit code's units left out); an ambient
//!   unit's offsets never count (`ambient_units`);
//! * R, reachable: `reach:<room>:obj:<id>` and `reach:<room>:actor:<id>` for each object and actor afford's
//!   `offer_cfg` finds a hit point for on the screen;
//! * C, camera: `cam:<room>:<xstart div 160>`.
//!
//! No game number is named here.

use crate::afford::{self, Hit, Trial};
use fmi_core::{EResult, Engine, Facts, Key, ObsReq, Val, K_AT, K_LOCAL, K_OBJSCRIPT, K_ROOM, K_SCRIPT, U_ENTRY, U_EXIT};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

/// The switch.
pub const SWITCH: &str = "FMI_NOVEL_ENGINE";
/// The camera bucket: half the 320-pixel screen.
pub const CAM_BUCKET: i32 = 160;
/// The no-input frames over which a unit's offsets are watched for the ambient rule (E012's idle measurement).
pub const AMBIENT_FRAMES: u64 = 1600;

pub fn on() -> bool {
    std::env::var(SWITCH).is_ok_and(|v| v == "1")
}

/// The per-stop observer: called with each trial's record, and its engine at the stop (None when the run ended or the
/// bridge refused the inputs). It may fork the engine; it must not move it.
pub type Observer = Box<dyn FnMut(&Trial, Option<&mut Box<dyn Engine>>)>;

thread_local! {
    static OBSERVER: RefCell<Option<Observer>> = const { RefCell::new(None) };
}

/// Sets this thread's observer (None: none); returns the one it replaces.
pub fn set_observer(o: Option<Observer>) -> Option<Observer> {
    OBSERVER.with(|c| std::mem::replace(&mut *c.borrow_mut(), o))
}

/// afford's hook: the observer, if one is set.
pub fn observe(t: &Trial, b: Option<&mut Box<dyn Engine>>) {
    let o = OBSERVER.with(|c| c.borrow_mut().take());
    if let Some(mut f) = o {
        f(t, b);
        OBSERVER.with(|c| {
            let mut s = c.borrow_mut();
            if s.is_none() {
                *s = Some(f);
            }
        });
    }
}

/// What the engine shows at a stop.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct View {
    pub frame: u64,
    pub room: i32,
    pub xstart: i32,
    /// live units as fmi-core names their keys ("script:N", "local:N", "objscript:N")
    pub live: Vec<String>,
    /// (unit, offset) of each K_AT key ("local:N", 12)
    pub at: Vec<(String, u64)>,
    /// "obj:N" or "actor:N" for each target with a hit point on the screen
    pub reach: Vec<String>,
}

/// The unit's text and the offset of a K_AT key at 1 (None: not one, or a room's entry or exit code).
pub fn at_of(k: Key, v: Val) -> Option<(String, u64)> {
    if k.kind() != K_AT || v != 1 || k.unit() == U_ENTRY || k.unit() == U_EXIT {
        return None;
    }
    let t = k.text();
    let rest = t.strip_prefix("at:")?;
    let (unit, off) = rest.rsplit_once(':')?;
    Some((unit.to_string(), off.parse().ok()?))
}

/// U's and W's parts of a state's facts: the live units and the (unit, offset) pairs.
pub fn control_of(f: &Facts) -> (Vec<String>, Vec<(String, u64)>) {
    let mut live = Vec::new();
    let mut at = Vec::new();
    for &(k, v) in f.iter() {
        match k.kind() {
            K_SCRIPT | K_LOCAL | K_OBJSCRIPT if v != 0 => live.push(k.text()),
            K_AT => {
                if let Some(p) = at_of(k, v) {
                    at.push(p);
                }
            }
            _ => {}
        }
    }
    live.sort();
    at.sort();
    (live, at)
}

/// The view of the engine's stop: its dump's facts and slots, the status's xstart, one `offer_cfg` (no walk boxes,
/// actors on the screen only), all on a fork that is quit after. Returns the view and the stop's facts.
pub fn view_at(e: &mut Box<dyn Engine>) -> EResult<(View, Facts)> {
    let mut f = e.fork()?;
    let r = (|| -> EResult<(View, Facts)> {
        let o = f.obs(&ObsReq::default())?;
        let st = f.status().clone();
        let facts = fmi_core::facts_of(&o);
        let of = afford::offer_cfg(&mut f, &o, false, true)?;
        let (live, at) = control_of(&facts);
        let mut reach: Vec<String> = of
            .targets
            .iter()
            .filter_map(|p| match p.hit {
                Hit::Object(id) => Some(format!("obj:{id}")),
                Hit::Actor(id) => Some(format!("actor:{id}")),
                _ => None,
            })
            .collect();
        reach.sort();
        reach.dedup();
        Ok((View { frame: st.frame, room: o.room, xstart: st.xstart, live, at, reach }, facts))
    })();
    f.quit();
    r
}

/// The view of a stored state (an archive entry): U and W from its checkpoint's facts, R and C from the engine booted
/// to it (`reach`, `xstart`).
pub fn view_of(facts: &Facts, frame: u64, xstart: i32, reach: Vec<String>) -> View {
    let (live, at) = control_of(facts);
    let room = facts.get(Key::pack(K_ROOM, 0, 0, 0)).unwrap_or(0) as i32;
    View { frame, room, xstart, live, at, reach }
}

/// The atoms of a view by kind ("U", "W", "R", "C"), W less the ambient units' offsets.
pub fn atoms(v: &View, ambient: &BTreeSet<String>) -> BTreeMap<&'static str, Vec<String>> {
    let r = v.room;
    let mut m: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
    m.insert("U", v.live.iter().map(|u| format!("live:{r}:{u}")).collect());
    m.insert("W", v.at.iter().filter(|(u, _)| !ambient.contains(u)).map(|(u, o)| format!("at:{r}:{u}:{o}")).collect());
    m.insert("R", v.reach.iter().map(|t| format!("reach:{r}:{t}")).collect());
    m.insert("C", vec![format!("cam:{r}:{}", v.xstart.div_euclid(CAM_BUCKET))]);
    m
}

/// The sensitivity rule's W (reported, not deciding): one atom per (unit, cell) for every unit with an offset, the
/// cell given by the caller (the stop's cell over T), with no ambient rule.
pub fn cell_atoms(v: &View, cell: &str) -> Vec<String> {
    let mut us: Vec<String> = v.at.iter().map(|(u, _)| format!("atc:{}:{u}:{cell}", v.room)).collect();
    us.dedup();
    us
}

/// The ambient units at the engine's state: on a no-input fork run `frames` frames, each unit whose offset takes two or
/// more values (the state's own included). Returns every unit's offsets seen and the ambient ones.
pub fn ambient_units(e: &mut Box<dyn Engine>, frames: u64) -> EResult<(BTreeMap<String, BTreeSet<u64>>, BTreeSet<String>)> {
    let mut f = e.fork()?;
    let r = (|| -> EResult<BTreeMap<String, BTreeSet<u64>>> {
        let f0 = f.facts()?;
        let mut seen: BTreeMap<String, BTreeSet<u64>> = BTreeMap::new();
        for &(k, v) in f0.iter() {
            if let Some((u, o)) = at_of(k, v) {
                seen.entry(u).or_default().insert(o);
            }
        }
        let mut each = |_st: &fmi_core::Status, ch: &[(Key, Val, Val)]| -> bool {
            for &(k, _, new) in ch {
                if let Some((u, o)) = at_of(k, new) {
                    seen.entry(u).or_default().insert(o);
                }
            }
            false
        };
        match f.wait(frames, &mut each) {
            Ok(_) | Err(fmi_core::EngineError::Ended(_)) => {}
            Err(x) => return Err(x),
        }
        Ok(seen)
    })();
    f.quit();
    let seen = r?;
    let amb = seen.iter().filter(|(_, o)| o.len() >= 2).map(|(u, _)| u.clone()).collect();
    Ok((seen, amb))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(pairs: &[(&str, i64)]) -> Facts {
        Facts::from_pairs(pairs.iter().map(|(k, v)| (Key::parse(k), *v)).collect())
    }

    #[test]
    fn kinds_of_a_state() {
        let f = facts(&[("room", 49), ("script:59", 1), ("local:209", 1), ("at:local:209:74", 1), ("at:script:59:783", 1),
                        ("var:3", 2)]);
        let v = view_of(&f, 10, 256, vec!["obj:7".into(), "actor:1".into()]);
        assert_eq!(v.live, vec!["local:209".to_string(), "script:59".to_string()]);
        assert_eq!(v.at, vec![("local:209".to_string(), 74), ("script:59".to_string(), 783)]);
        let none = BTreeSet::new();
        let a = atoms(&v, &none);
        assert_eq!(a["U"], vec!["live:49:local:209".to_string(), "live:49:script:59".to_string()]);
        assert_eq!(a["W"], vec!["at:49:local:209:74".to_string(), "at:49:script:59:783".to_string()]);
        assert_eq!(a["R"], vec!["reach:49:obj:7".to_string(), "reach:49:actor:1".to_string()]);
        assert_eq!(a["C"], vec!["cam:49:1".to_string()]);
        let amb: BTreeSet<String> = ["script:59".to_string()].into_iter().collect();
        assert_eq!(atoms(&v, &amb)["W"], vec!["at:49:local:209:74".to_string()]);
        // the ambient unit still counts through U
        assert_eq!(atoms(&v, &amb)["U"].len(), 2);
        assert_eq!(cell_atoms(&v, "c1"), vec!["atc:49:local:209:c1".to_string(), "atc:49:script:59:c1".to_string()]);
    }

    #[test]
    fn camera_buckets() {
        let v = |x| View { room: 49, xstart: x, ..Default::default() };
        let none = BTreeSet::new();
        assert_eq!(atoms(&v(0), &none)["C"], vec!["cam:49:0".to_string()]);
        assert_eq!(atoms(&v(159), &none)["C"], vec!["cam:49:0".to_string()]);
        assert_eq!(atoms(&v(160), &none)["C"], vec!["cam:49:1".to_string()]);
        assert_eq!(atoms(&v(320), &none)["C"], vec!["cam:49:2".to_string()]);
    }

    #[test]
    fn the_observer_is_off_by_default() {
        std::env::remove_var(SWITCH);
        assert!(!on());
    }
}
