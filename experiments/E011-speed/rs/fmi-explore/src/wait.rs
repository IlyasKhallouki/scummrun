//! E012's wait move (experiments/E012-game-mechanics/plan.md, section 1, and the amendment "the `wait` component"):
//! from an entry the engine runs with no input. A frame is a stop when an atom of T (the reading's tested atoms)
//! changes truth for the first time since the wait began, its truth at the entry counting as seen. The wait ends W
//! frames after the last stop or the last due wake (the paused slots' due frames, `fmi_core::due`, read at the entry
//! and again at each stop), and is static when nothing stopped by then. The run is a fork of the entry, so a stop is
//! the entry's movie run to a later frame (exact by A0); it carries its checkpoint, and the try decides what to keep.
//!
//! Switch FMI_WAIT (off by default): the try calls the wait in place of E009's idle measurement, whose rule it feeds
//! from the same frames.

use crate::machine;
use crate::pyjson;
use crate::start::{self, Points, ELSEWHERE, NOWHERE};
use crate::step::IdleRule;
use fmi_core::{
    delay_step, due, facts_of, Checkpoint, Domains, Engine, EngineError, Facts, Key, Obs, ObsReq, ReadingSource, Status,
    Val, K_AT, K_ESC, K_ESCPOS, K_OTHER, K_POS, SS_PAUSED, VAR_TIMER_NEXT,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Instant;

/// W: frames after the last due wake with no stop before the wait ends (E009's measured idle length, plan.md).
pub const W: u64 = 1600;
/// The engineering watchdog on one wait's frames, unless FMI_WAIT_MAX_FRAMES sets it (reported where it fires).
pub const MAX_FRAMES: u64 = 20_000;

/// FMI_WAIT=1: the try waits (plan.md section 1). Off by default.
pub fn on() -> bool {
    std::env::var("FMI_WAIT").map(|v| v == "1").unwrap_or(false)
}

#[derive(Clone, Debug)]
pub struct WaitCfg {
    /// frames after the last stop or due wake with no stop before the wait ends
    pub w: u64,
    /// E009's idle rule fed from the wait's first `idle_frames` frames (its record in `WaitOut::idle`); 0: none
    pub idle_frames: u64,
    /// the watchdog on the wait's frames
    pub max_frames: u64,
    /// the differential check of every frame: T's values from the changes against `start::view` of the whole facts
    pub view_check: bool,
}

impl Default for WaitCfg {
    fn default() -> WaitCfg {
        let max_frames = std::env::var("FMI_WAIT_MAX_FRAMES").ok().and_then(|v| v.parse().ok()).unwrap_or(MAX_FRAMES);
        WaitCfg { w: W, idle_frames: 0, max_frames, view_check: false }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum WaitEnd {
    /// stops, then W frames past the last stop and due wake with none
    Horizon,
    /// no stop by W frames past the entry's last due wake
    Static,
    /// the engine ended (its end reason)
    Ended(String),
    /// the watchdog on the frames fired
    Watchdog,
}

impl WaitEnd {
    pub fn text(&self) -> String {
        match self {
            WaitEnd::Horizon => "horizon".into(),
            WaitEnd::Static => "static".into(),
            WaitEnd::Ended(r) => format!("ended: {r}"),
            WaitEnd::Watchdog => "watchdog".into(),
        }
    }
}

/// One stop: its frame, the atoms whose truth changed there for the first time (indices into the atoms given; their
/// new truth is the opposite of `WaitOut::truth0`), its checkpoint (as the harness makes one), and the due frames
/// read there.
#[derive(Clone, Debug)]
pub struct WaitStop {
    pub frame: u64,
    pub atoms: Vec<usize>,
    pub checkpoint: Checkpoint,
    pub room: i32,
    /// (slot, due frame) of the paused slots at the stop
    pub dues: Vec<(i32, u64)>,
}

#[derive(Clone, Debug)]
pub struct WaitOut {
    pub stops: Vec<WaitStop>,
    pub end: WaitEnd,
    pub frame0: u64,
    /// frames run (the last frame is frame0 + frames)
    pub frames: u64,
    pub seconds: f64,
    /// the fork's CPU seconds
    pub cpu_s: f64,
    /// the ticks a frame took off the paused delays at the entry (`fmi_core::delay_step`)
    pub delta: Option<i64>,
    /// (slot, due frame) of the paused slots at the entry
    pub dues0: Vec<(i32, u64)>,
    /// the last due frame read (the entry's or the last stop's), or frame0 when none
    pub horizon: u64,
    /// each atom's truth at the entry (None: never observed: an unobserved key, or one the view does not hold)
    pub truth0: Vec<Option<bool>>,
    /// E009's idle record from the first `idle_frames` frames, when asked
    pub idle: Option<Value>,
    /// frames whose tick advance was not var 19 at the frame before (at least 1): the delta rule's misses
    pub delta_misses: u64,
    /// stops whose facts from the changes differ from the stop's own dump (0: step_changes and the patching agree)
    pub facts_misses: u64,
    /// the differential check's frames and the keys that differed there (view_check)
    pub view_checked: u64,
    pub view_misses: Vec<(u64, String, Val, Val)>,
    /// wall seconds by part: the fork, the entry's dump, the frames, the stops' dumps, the quit
    pub parts: Vec<(&'static str, f64)>,
    /// E014: the end's frame and tick when the run ended in the wait (the engine's end status)
    pub end_at: Option<(u64, u64)>,
}

impl WaitOut {
    pub fn is_static(&self) -> bool {
        self.end == WaitEnd::Static
    }

    /// The record the try writes (result.json "wait").
    pub fn json(&self, atoms: &[(Key, Vec<Val>)]) -> Value {
        let f0 = self.frame0;
        let atom_json = |i: usize| {
            let (k, vs) = &atoms[i];
            json!([k.text(), vs.iter().map(|v| start::val_json(*v)).collect::<Vec<_>>(), self.truth0[i].map(|t| !t)])
        };
        let stops: Vec<Value> = self
            .stops
            .iter()
            .map(|s| {
                json!({"frame": s.frame, "off": s.frame - f0, "room": s.room, "tick": s.checkpoint.tick,
                       "atoms": s.atoms.iter().map(|&i| atom_json(i)).collect::<Vec<_>>(),
                       "dues": s.dues.iter().map(|(sl, f)| json!([sl, f - s.frame])).collect::<Vec<_>>()})
            })
            .collect();
        let observed = self.truth0.iter().filter(|t| t.is_some()).count();
        let mut out = json!({"frame0": f0, "frames": self.frames, "end": self.end.text(), "seconds": pyjson::round(self.seconds, 3),
            "cpu_s": pyjson::round(self.cpu_s, 3), "delta": self.delta,
            "dues0": self.dues0.iter().map(|(sl, f)| json!([sl, f - f0])).collect::<Vec<_>>(),
            "horizon": self.horizon - f0, "atoms": atoms.len(), "observed": observed, "stops": stops,
            "delta_misses": self.delta_misses, "facts_misses": self.facts_misses,
            "parts": self.parts.iter().map(|(k, v)| json!([k, pyjson::round(*v, 4)])).collect::<Vec<_>>()});
        if self.view_checked > 0 {
            out["view_checked"] = json!(self.view_checked);
            out["view_misses"] = json!(self.view_misses.len());
        }
        out
    }
}

/// (slot, due frame) of every paused slot of the dump, in slot order.
pub fn dues_of(o: &Obs) -> Vec<(i32, u64)> {
    o.slots.iter().filter(|s| s.status == SS_PAUSED).filter_map(|s| due(o, s.slot).map(|f| (s.slot, f))).collect()
}

/// An at:/esc: key's (kind, unit, a, place): packed, or a negative place (an interned text), as start.rs reads them.
fn at_parts(k: Key) -> Option<(u8, u8, u64, i64)> {
    match k.kind() {
        K_AT | K_ESC => Some((k.kind(), k.unit(), k.a(), k.b() as i64)),
        K_OTHER => {
            let t = k.text();
            let (kind, rest) = t.split_once(':')?;
            let kind = match kind {
                "at" => K_AT,
                "esc" => K_ESC,
                _ => return None,
            };
            let (name, w) = rest.rsplit_once(':')?;
            let digits = w.trim_start_matches('-');
            if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
                return None;
            }
            let pk = Key::parse(&format!("pos:{name}"));
            if pk.kind() != K_POS {
                return None;
            }
            Some((kind, pk.unit(), pk.a(), w.parse().ok()?))
        }
        _ => None,
    }
}

/// The harness's view (`start::view`) of T's keys, kept from frame to frame through the facts' changes: a key's value
/// labelled by its domain; pos:/escpos: the least place of the unit held among the scope's points, else NOWHERE, or
/// ELSEWHERE where the domain holds it and the unit is at a place none of the points (start::task_start, elsewhere).
pub struct TView<'a> {
    doms: &'a Domains,
    points: Points,
    /// the engine's facts, patched by each frame's changes
    pub raw: Facts,
    /// T's observed keys and their values now
    val: HashMap<Key, Val>,
    /// the pos:/escpos: keys of T by (at-kind, unit, a)
    pos_of: HashMap<(u8, u8, u64), Key>,
    /// per (at-kind, unit, a) of those, the places held now (an at:/esc: fact at 1)
    held: HashMap<(u8, u8, u64), Vec<i64>>,
}

impl<'a> TView<'a> {
    /// The view of `keys` (observed ones) at the facts `raw`.
    pub fn new(doms: &'a Domains, points: Points, raw: Facts, keys: &[Key]) -> TView<'a> {
        let mut tv = TView { doms, points, raw, val: HashMap::new(), pos_of: HashMap::new(), held: HashMap::new() };
        for &k in keys {
            match k.kind() {
                K_POS => {
                    tv.pos_of.insert((K_AT, k.unit(), k.a()), k);
                }
                K_ESCPOS => {
                    tv.pos_of.insert((K_ESC, k.unit(), k.a()), k);
                }
                _ => {}
            }
        }
        for &(k, v) in tv.raw.0.iter() {
            if v != 1 {
                continue;
            }
            if let Some((ak, u, a, w)) = at_parts(k) {
                if tv.pos_of.contains_key(&(ak, u, a)) {
                    tv.held.entry((ak, u, a)).or_default().push(w);
                }
            }
        }
        for &k in keys {
            if let Some(v) = tv.value(k) {
                tv.val.insert(k, v);
            }
        }
        tv
    }

    /// The key's view value from the facts kept (None: the view does not hold the key).
    pub fn value(&self, k: Key) -> Option<Val> {
        let d = self.doms.get(k)?;
        let v = match k.kind() {
            K_POS | K_ESCPOS => {
                let (ak, pts) = if k.kind() == K_POS { (K_AT, &self.points.at) } else { (K_ESC, &self.points.esc) };
                let held = self.held.get(&(ak, k.unit(), k.a()));
                let ws_all = pts.get(&(k.unit(), k.a()));
                let least = ws_all.and_then(|ws| ws.iter().copied().filter(|w| held.is_some_and(|h| h.contains(w))).min());
                match least {
                    Some(w) => w,
                    None => {
                        let away = held.is_some_and(|h| h.iter().any(|w| !ws_all.is_some_and(|p| p.contains(w))));
                        if self.points.elsewhere.contains(&k) && away {
                            ELSEWHERE
                        } else {
                            NOWHERE
                        }
                    }
                }
            }
            _ => self.raw.get(k).unwrap_or(0),
        };
        Some(d.label(v))
    }

    pub fn get(&self, k: Key) -> Option<Val> {
        self.val.get(&k).copied()
    }

    /// One frame's changes: the facts patched, and the T keys whose value changed, with their new values.
    pub fn apply(&mut self, ch: &[(Key, Val, Val)], out: &mut Vec<Key>) {
        let mut dirty: Vec<Key> = Vec::new();
        for &(k, _, new) in ch {
            if new != 0 {
                self.raw.set(k, new);
            } else {
                self.raw.remove(k);
            }
            if self.val.contains_key(&k) && k.kind() != K_POS && k.kind() != K_ESCPOS {
                dirty.push(k);
            }
            if let Some((ak, u, a, w)) = at_parts(k) {
                if let Some(&pk) = self.pos_of.get(&(ak, u, a)) {
                    let h = self.held.entry((ak, u, a)).or_default();
                    h.retain(|x| *x != w);
                    if new == 1 {
                        h.push(w);
                    }
                    dirty.push(pk);
                }
            }
        }
        dirty.sort_unstable();
        dirty.dedup();
        for k in dirty {
            let Some(old) = self.val.get(&k).copied() else { continue };
            if let Some(v) = self.value(k) {
                if v != old {
                    self.val.insert(k, v);
                    out.push(k);
                }
            }
        }
    }
}

/// T as the wait reads it: the atoms, by key, and each one's truth at the entry and whether it has changed since.
struct Truths {
    by_key: HashMap<Key, Vec<usize>>,
    truth0: Vec<Option<bool>>,
    flipped: Vec<bool>,
}

impl Truths {
    fn new(atoms: &[(Key, Vec<Val>)], tv: &TView) -> Truths {
        let mut by_key: HashMap<Key, Vec<usize>> = HashMap::new();
        let mut truth0 = Vec::with_capacity(atoms.len());
        for (i, (k, vs)) in atoms.iter().enumerate() {
            let t = tv.get(*k).map(|v| vs.contains(&v));
            if t.is_some() {
                by_key.entry(*k).or_default().push(i);
            }
            truth0.push(t);
        }
        Truths { by_key, truth0, flipped: vec![false; atoms.len()] }
    }

    /// The atoms of the changed keys whose truth is new since the entry, appended to `out`.
    fn first_changes(&mut self, atoms: &[(Key, Vec<Val>)], tv: &TView, changed: &[Key], out: &mut Vec<usize>) {
        for k in changed {
            let Some(ix) = self.by_key.get(k) else { continue };
            let v = tv.get(*k);
            for &i in ix {
                if self.flipped[i] {
                    continue;
                }
                let t = v.map(|v| atoms[i].1.contains(&v));
                if t.is_some() && t != self.truth0[i] {
                    self.flipped[i] = true;
                    out.push(i);
                }
            }
        }
    }
}

thread_local! {
    /// E019: inside an affordance step (`afford::try_all`), the trials' waits' floor in frames
    static TRIAL_FLOOR: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) };
}

/// E019 (`FMI_TRIAL_WAIT_FLOOR=N`): the floor of a trial's wait (its idle frames), N frames where the entry's wait keeps
/// E009's; the dues and the stops extend a wait as before. Kept states came within 164 frames of their offer in play8
/// and play9 (play8's room-42 crossing ~390 frames after its click), against a floor of 1,600.
pub fn trial_floor() -> Option<u64> {
    std::env::var("FMI_TRIAL_WAIT_FLOOR").ok().and_then(|v| v.parse().ok()).filter(|&v: &u64| v > 0)
}

/// Sets (or clears) the floor of the waits run in this thread, for an affordance step's trials.
pub fn set_trial_floor(f: Option<u64>) {
    TRIAL_FLOOR.with(|c| c.set(f));
}

/// The observed keys of the atoms (unobserved prefixes left out: args:, saved:, actorroom:), sorted, once each.
pub fn observed_keys(atoms: &[(Key, Vec<Val>)]) -> Vec<Key> {
    let unob = start::unobserved(true);
    let mut ks: Vec<Key> = atoms.iter().map(|a| a.0).filter(|k| !start::has_prefix(*k, &unob)).collect();
    ks.sort_unstable();
    ks.dedup();
    ks
}

/// The reading's points (the harness's: its domains and reading.json's live_locals).
pub fn points_of(rd: &dyn ReadingSource) -> Points {
    let head = crate::warm::head(rd);
    Points::of(rd.domains(), head.get("live_locals").filter(|v| !v.is_null()).cloned())
}

fn eng(e: EngineError) -> String {
    format!("EngineError: {e}")
}

/// The wait from the engine `root` stands at (a fork of it, quit at the end), on T's atoms over the reading `rd`.
pub fn wait(root: &mut Box<dyn Engine>, atoms: &[(Key, Vec<Val>)], rd: &dyn ReadingSource, cfg: &WaitCfg) -> Result<WaitOut, String> {
    let t = Instant::now();
    let mut b = machine::port(|| root.fork()).map_err(eng)?;
    let t_fork = t.elapsed().as_secs_f64();
    let r = wait_on(&mut b, atoms, rd.domains(), points_of(rd), cfg, t);
    let cpu = b.cpu_s();
    let tq = Instant::now();
    machine::port(|| b.quit());
    let mut out = r?;
    out.cpu_s = cpu;
    out.parts.insert(0, ("fork", t_fork));
    out.parts.push(("quit", tq.elapsed().as_secs_f64()));
    out.seconds = t.elapsed().as_secs_f64();
    Ok(out)
}

/// The wait on `b` itself (the caller's fork): it runs on from where `b` stands.
pub fn wait_on(b: &mut Box<dyn Engine>, atoms: &[(Key, Vec<Val>)], doms: &Domains, points: Points, cfg: &WaitCfg, t: Instant)
               -> Result<WaitOut, String> {
    let t0 = Instant::now();
    let o0 = machine::port(|| b.obs(&ObsReq::default())).map_err(eng)?;
    let st0 = b.status().clone();
    let f0 = st0.frame;
    let keys = observed_keys(atoms);
    let unob = start::unobserved(true);
    let mut tv = TView::new(doms, points, facts_of(&o0), &keys);
    let mut truths = Truths::new(atoms, &tv);
    let dues0 = dues_of(&o0);
    let mut horizon = dues0.iter().map(|d| d.1).max().unwrap_or(f0);
    let mut last_stop = f0;
    let floor = TRIAL_FLOOR.with(|c| c.get()).map(|f| f.min(cfg.idle_frames)).unwrap_or(cfg.idle_frames);
    let end_of = |horizon: u64, last_stop: u64| (f0 + floor).max(horizon.max(last_stop) + cfg.w);
    let mut end_frame = end_of(horizon, last_stop);
    let cap = f0 + cfg.max_frames;
    let mut rule = IdleRule::new();
    let mut stops: Vec<WaitStop> = Vec::new();
    let (mut delta_misses, mut facts_misses) = (0u64, 0u64);
    let mut view_checked = 0u64;
    let mut view_misses: Vec<(u64, String, Val, Val)> = Vec::new();
    // (tick, var 19, frame) at the frame before: the next frame's tick advance is var 19 there, at least 1
    let mut prev = (o0.tick, o0.vars.get(VAR_TIMER_NEXT).copied().unwrap_or(0), f0);
    let mut end = None;
    let mut end_at: Option<(u64, u64)> = None;
    let mut bad_frame: Option<String> = None;
    let mut view_points = if cfg.view_check { Some(tv.points.clone()) } else { None };
    let s_entry = t0.elapsed().as_secs_f64();
    let (mut s_frames, mut s_stops) = (0.0, 0.0);
    loop {
        let cur = b.status().frame;
        let target = end_frame.min(cap);
        if cur >= target {
            break;
        }
        let mut newly: Vec<usize> = Vec::new();
        let mut changed: Vec<Key> = Vec::new();
        let r = {
            let (tv, truths, rule, newly, changed) = (&mut tv, &mut truths, &mut rule, &mut newly, &mut changed);
            let (prev, delta_misses, bad_frame) = (&mut prev, &mut delta_misses, &mut bad_frame);
            let (view_points, view_checked, view_misses) = (&mut view_points, &mut view_checked, &mut view_misses);
            let keys = &keys;
            let unob = &unob;
            let mut each = |st: &Status, ch: &[(Key, Val, Val)]| -> bool {
                if st.frame != prev.2 + 1 {
                    *bad_frame = Some(format!("the wait's frame {} ({}), not {}", st.frame, st.reason, prev.2 + 1));
                    return true;
                }
                if st.tick.wrapping_sub(prev.0) != prev.1.max(1) as u64 {
                    *delta_misses += 1;
                }
                *prev = (st.tick, st.v19, st.frame);
                let off = st.frame - f0;
                if off <= cfg.idle_frames {
                    rule.frame(off as i64, ch);
                }
                changed.clear();
                tv.apply(ch, changed);
                truths.first_changes(atoms, tv, changed, newly);
                if let Some(p) = view_points.as_mut() {
                    *view_checked += 1;
                    let now = start::view(p, doms, &tv.raw, unob);
                    for &k in keys.iter() {
                        let (want, got) = (now.get(k), tv.get(k));
                        if want != got && view_misses.len() < 64 {
                            view_misses.push((st.frame, k.text(), want.unwrap_or(fmi_core::NONE), got.unwrap_or(fmi_core::NONE)));
                        }
                    }
                }
                !newly.is_empty()
            };
            let tw = Instant::now();
            // E019: the wait stops only after a frame that changed an observed key (newly needs one)
            b.set_watch(Some(keys.as_slice()));
            let r = machine::port(|| b.wait(target - cur, &mut each));
            b.set_watch(None);
            s_frames += tw.elapsed().as_secs_f64();
            r
        };
        if let Some(e) = bad_frame {
            return Err(e);
        }
        match r {
            Ok((st, _)) => {
                if newly.is_empty() {
                    continue;
                }
                let ts = Instant::now();
                let o = machine::port(|| b.obs(&ObsReq::default())).map_err(eng)?;
                let facts = facts_of(&o);
                // the same facts.py map (absent is 0: the dump lists every var, a patch drops the 0s)
                if !facts.diff(&tv.raw).is_empty() {
                    facts_misses += 1;
                }
                let mut actors: Vec<(i32, i32, i32)> = o.actors.iter().map(|a| (a.id, a.x, a.y)).collect();
                actors.sort();
                let dues = dues_of(&o);
                horizon = dues.iter().map(|d| d.1).max().unwrap_or(st.frame);
                last_stop = st.frame;
                end_frame = end_of(horizon, last_stop);
                stops.push(WaitStop {
                    frame: st.frame,
                    atoms: newly,
                    checkpoint: Checkpoint { frame: st.frame, facts, tick: st.tick, rng: st.rng, actors },
                    room: o.room,
                    dues,
                });
                s_stops += ts.elapsed().as_secs_f64();
            }
            Err(EngineError::Ended(s)) => {
                end = Some(WaitEnd::Ended(s.reason.clone()));
                end_at = Some((s.frame, s.tick));
                break;
            }
            Err(e) => return Err(eng(e)),
        }
    }
    let last = b.status().frame;
    let end = end.unwrap_or(if last >= end_frame {
        if stops.is_empty() {
            WaitEnd::Static
        } else {
            WaitEnd::Horizon
        }
    } else {
        WaitEnd::Watchdog
    });
    let idle = (cfg.idle_frames > 0).then(|| {
        let mut v = rule.result();
        v["frame_end"] = json!(f0 + cfg.idle_frames.min(last - f0));
        v["frames"] = json!(cfg.idle_frames);
        v["frame0"] = json!(f0);
        v
    });
    Ok(WaitOut {
        stops,
        end,
        frame0: f0,
        frames: last - f0,
        seconds: t.elapsed().as_secs_f64(),
        cpu_s: 0.0,
        delta: delay_step(&o0),
        dues0,
        horizon,
        truth0: truths.truth0,
        idle,
        delta_misses,
        facts_misses,
        view_checked,
        view_misses,
        parts: vec![("entry", s_entry), ("frames", s_frames), ("stops", s_stops)],
        end_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmi_core::{Domain, Slot, ABOVE};

    fn dom(vals: &[Val]) -> Domain {
        Domain { values: vals.to_vec(), low: None, high: None }
    }

    #[test]
    fn due_is_the_engines_rule() {
        let mut o = Obs { frame: 3393, vars: vec![0; 40], ..Default::default() };
        o.vars[19] = 6;
        o.slots = vec![
            Slot { slot: 3, status: SS_PAUSED, delay: 2760, ..Default::default() },
            Slot { slot: 4, status: SS_PAUSED | 0x80, frozen: 1, delay: 100, ..Default::default() },
            Slot { slot: 5, status: 2, delay: 0, ..Default::default() },
            Slot { slot: 6, status: SS_PAUSED, delay: 5, ..Default::default() },
            Slot { slot: 7, status: SS_PAUSED, delay: 6, ..Default::default() },
        ];
        // floor(2760 / 6) + 1 = 461 (plan.md M2); a frozen or running slot has none
        assert_eq!(due(&o, 3), Some(3393 + 461));
        assert_eq!(due(&o, 4), None);
        assert_eq!(due(&o, 5), None);
        assert_eq!(due(&o, 9), None);
        // delay 5 with delta 6 goes below 0 in the next frame; delay 6 reaches 0 there and below it one frame later
        assert_eq!(due(&o, 6), Some(3394));
        assert_eq!(due(&o, 7), Some(3395));
        assert_eq!(dues_of(&o), vec![(3, 3854), (6, 3394), (7, 3395)]);
        // the delta is var 19, at least 1, and the delay goes down by at most 15 a frame
        o.vars[19] = 0;
        assert_eq!(delay_step(&o), Some(1));
        assert_eq!(due(&o, 6), Some(3393 + 6));
        o.vars[19] = 40;
        assert_eq!(delay_step(&o), Some(15));
        assert_eq!(due(&o, 3), Some(3393 + 2760 / 15 + 1));
        assert_eq!(delay_step(&Obs::default()), None);
    }

    fn view_doms() -> Domains {
        let mut v = vec![
            (Key::parse("state:316"), dom(&[0, 1])),
            (Key::parse("var:32"), Domain { values: vec![fmi_core::BELOW, 0, 1, ABOVE], low: Some(0), high: Some(1) }),
            (Key::parse("pos:local:211"), dom(&[ELSEWHERE, NOWHERE, 21, 40])),
            (Key::parse("escpos:local:211"), dom(&[NOWHERE, 7])),
        ];
        v.sort_by_key(|p| p.0);
        Domains(v)
    }

    /// TView's values from changes equal start::view of the whole facts, frame after frame.
    #[test]
    fn tview_follows_the_view() {
        let doms = view_doms();
        let keys: Vec<Key> = doms.0.iter().map(|p| p.0).collect();
        let raw = Facts::from_pairs(vec![(Key::parse("state:316"), 0), (Key::parse("var:32"), 5), (Key::parse("at:local:211:21"), 1)]);
        let points = Points::of(&doms, None);
        let mut tv = TView::new(&doms, points.clone(), raw.clone(), &keys);
        let unob = start::unobserved(true);
        let check = |tv: &TView, raw: &Facts| {
            let mut p = points.clone();
            let now = start::view(&mut p, &doms, raw, &unob);
            for &k in &keys {
                assert_eq!(tv.get(k), now.get(k), "{k}");
            }
        };
        check(&tv, &raw);
        assert_eq!(tv.get(Key::parse("var:32")), Some(ABOVE));
        assert_eq!(tv.get(Key::parse("pos:local:211")), Some(21));
        let frames: Vec<Vec<(&str, Val, Val)>> = vec![
            vec![("at:local:211:21", 1, 0), ("at:local:211:40", 0, 1)],
            vec![("state:316", 0, 1), ("var:32", 5, 1)],
            vec![("at:local:211:40", 1, 0), ("at:local:211:55", 0, 1)],
            vec![("at:local:211:55", 1, 0)],
            vec![("at:local:211:-3", 0, 1), ("esc:local:211:7", 0, 1)],
            vec![("at:local:211:-3", 1, 0), ("at:local:211:21", 0, 1), ("at:local:211:40", 0, 1)],
        ];
        let mut raw = raw;
        let mut seen = Vec::new();
        for f in frames {
            let ch: Vec<(Key, Val, Val)> = f.iter().map(|(k, a, b)| (Key::parse(k), *a, *b)).collect();
            let mut ch = ch;
            ch.sort_by_key(|c| c.0);
            raw.patch(&ch);
            let mut out = Vec::new();
            tv.apply(&ch, &mut out);
            check(&tv, &raw);
            assert_eq!(tv.raw, raw);
            seen.push(out.iter().map(|k| k.text()).collect::<Vec<_>>());
        }
        assert_eq!(tv.get(Key::parse("pos:local:211")), Some(21));
        assert!(seen[2].contains(&"pos:local:211".to_string()), "{seen:?}");
    }

    #[test]
    fn first_changes_only_once() {
        let doms = view_doms();
        let atoms = vec![(Key::parse("state:316"), vec![1]), (Key::parse("state:316"), vec![0]), (Key::parse("var:32"), vec![0, 1])];
        let keys = observed_keys(&atoms);
        let raw = Facts::from_pairs(vec![(Key::parse("var:32"), 1)]);
        let mut tv = TView::new(&doms, Points::of(&doms, None), raw, &keys);
        let mut tr = Truths::new(&atoms, &tv);
        assert_eq!(tr.truth0, vec![Some(false), Some(true), Some(true)]);
        let mut run = |ch: Vec<(&str, Val, Val)>| {
            let ch: Vec<(Key, Val, Val)> = ch.iter().map(|(k, a, b)| (Key::parse(k), *a, *b)).collect();
            let (mut changed, mut out) = (Vec::new(), Vec::new());
            tv.apply(&ch, &mut changed);
            tr.first_changes(&atoms, &tv, &changed, &mut out);
            out
        };
        assert_eq!(run(vec![("state:316", 0, 1)]), vec![0, 1]);
        // back to the entry's truth, then the new one again: seen already, no stop
        assert_eq!(run(vec![("state:316", 1, 0)]), Vec::<usize>::new());
        assert_eq!(run(vec![("state:316", 0, 1)]), Vec::<usize>::new());
        assert_eq!(run(vec![("var:32", 1, 7)]), vec![2]);
    }
}
