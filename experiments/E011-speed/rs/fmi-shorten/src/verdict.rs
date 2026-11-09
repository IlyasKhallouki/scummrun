//! E024 Part 2, item 4 (plan.md, "Verdicts"): whether a placed input and a re-found step did what the movie's did.
//!
//! * **Click** (`click`): the input log's record of the click's frame says Taken, with the identity the movie's input
//!   had (label.rs `Identity`: the hit at the frame before, the handler's arguments, the sentences and walks).
//! * **Step** (`Goal::Keep`): keep(k) as the bridge's conditions (`conds`), a run with RunSpec.until and max_frame
//!   (`step`; the scan's bound gives it: the best's frame, then on while the candidate's tick is below the best's,
//!   scan.rs); the completion is the stop where they hold. A draw key (the twins
//!   of item 3) is judged as changed from its root value, not by value (`changed`). With them, where the movie's step
//!   left ego (`ego_at`, pass.rs).
//! * **Last step** (`Goal::End`): the game's end with the same end reason, its script site with it
//!   (SPEEDRUN_END_SITE's "restart@SITE").
//! * **Times** are Status.tick; frames break ties (`End`). A reduction (stage B) is kept only for an earlier verified
//!   end (`earlier`).

use crate::label::{identity, HitId, Identity};
use fmi_core::*;

/// A click's verdict by its record.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Click {
    Taken,
    /// no record of the click's frame
    NoRecord,
    /// the record's verdict: dropped or ignored
    Refused(String),
    /// taken, with another identity
    Other(Box<Identity>),
}

/// The verdict on the click of `frame` among `recs` (the input log's records), its point's hit at the frame before
/// being `hit`, against the movie's identity `want`.
pub fn click(recs: &[InputRecord], frame: u64, hit: HitId, want: &Identity) -> Click {
    let Some(r) = recs.iter().find(|r| r.frame == frame) else { return Click::NoRecord };
    if r.verdict != Verdict::Taken {
        return Click::Refused(r.verdict.text());
    }
    let got = identity(r, hit);
    if got == *want {
        Click::Taken
    } else {
        Click::Other(Box::new(got))
    }
}

/// The bridge's condition that `k` has value `v` (None for a key no condition kind reads: the progress holds only
/// rooms, owners and the guide's atom keys).
pub fn cond(k: Key, v: Val) -> Option<Cond> {
    let mk = |kind, subject: u64, value: Val, negate| Cond { kind, subject: subject as i32, op: CondOp::Eq, value: value as i32, values: vec![], negate };
    match k.kind() {
        K_VAR => Some(mk(CondKind::Var, k.a(), v, false)),
        K_BIT => Some(mk(CondKind::Bit, k.a(), v, false)),
        K_OWNER => Some(mk(CondKind::Owner, k.a(), v, false)),
        K_STATE => Some(mk(CondKind::State, k.a(), v, false)),
        // the bridge's class condition: getClass(object, class) != negate
        K_CLASS => Some(mk(CondKind::Class, k.a(), k.b() as Val, v == 0)),
        // the room condition holds when the current room is the subject
        K_ROOM => Some(mk(CondKind::Room, v as u64, 0, false)),
        _ => None,
    }
}

/// The bridge's condition that `k` no longer has its root value `root` (a draw key's, plan.md item 3).
pub fn changed(k: Key, root: Val) -> Option<Cond> {
    match k.kind() {
        // a class is set or not: changed is the other
        K_CLASS => cond(k, (root == 0) as Val),
        _ => cond(k, root).map(|c| Cond { negate: true, ..c }),
    }
}

/// keep(k) as conditions. A key `drawn` gives its root value for is a draw's: judged as changed from its root value
/// instead of by value, and not judged when the movie's value is the root's (changed from it is then no outcome the
/// movie had). Err(key) for the first key no condition reads.
pub fn conds(keep: &[(Key, Val)], drawn: &dyn Fn(Key) -> Option<Val>) -> Result<Vec<Cond>, Key> {
    let mut out = Vec::new();
    for &(k, v) in keep {
        match drawn(k) {
            Some(root) if root == v => {}
            Some(root) => out.push(changed(k, root).ok_or(k)?),
            None => out.push(cond(k, v).ok_or(k)?),
        }
    }
    Ok(out)
}

/// Ego (actor `ego`) standing at (x, y), not moving: where the movie's step left it. A walk changes no key, so
/// keep(k) alone does not see a walk cut short by the next input (pass.rs).
pub fn ego_at(ego: Val, x: i32, y: i32) -> Vec<Cond> {
    let mk = |kind, value: i32| Cond { kind, subject: ego as i32, op: CondOp::Eq, value, values: vec![], negate: false };
    vec![mk(CondKind::ActorX, x), mk(CondKind::ActorY, y), mk(CondKind::ActorMoving, 0)]
}

/// What a step must reach.
#[derive(Clone, Debug)]
pub enum Goal {
    /// keep(k), as conditions
    Keep(Vec<Cond>),
    /// the game's end with this reason
    End(String),
}

/// A step's run past its last input.
#[derive(Clone, Debug)]
pub enum Run {
    /// the completion: the stop where keep(k) holds, or the game's end with the goal's reason
    Done(Status),
    /// max_frame came first
    Late(u64),
    /// the game ended with another reason
    Ended(String),
}

/// Item 4's step verdict: `e` run until the goal holds, at most to `max_frame`.
pub fn step(e: &mut Box<dyn Engine>, goal: &Goal, max_frame: u64) -> EResult<Run> {
    if e.status().frame >= max_frame {
        return Ok(Run::Late(e.status().frame));
    }
    let spec = match goal {
        Goal::Keep(c) => RunSpec { until: c.clone(), max_frame: Some(max_frame), ..Default::default() },
        Goal::End(_) => RunSpec { max_frame: Some(max_frame), ..Default::default() },
    };
    match e.run(&spec) {
        Ok(st) if st.reason == "until" => Ok(Run::Done(st)),
        Ok(st) => Ok(Run::Late(st.frame)),
        Err(EngineError::Ended(st)) => match goal {
            Goal::End(want) if st.reason == *want => Ok(Run::Done(*st)),
            _ => Ok(Run::Ended(st.reason)),
        },
        Err(x) => Err(x),
    }
}

/// A verified end: its tick, its frame, its reason.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct End {
    pub tick: u64,
    pub frame: u64,
    pub reason: String,
}

impl End {
    pub fn of(st: &Status) -> End {
        End { tick: st.tick, frame: st.frame, reason: st.reason.clone() }
    }
}

/// Whether `new` (a candidate's verified end, None when it did not verify) is kept over `best`: the same reason, and
/// earlier by (tick, frame) (plan.md item 7: a drop is kept only if the verified end comes earlier).
pub fn earlier(new: Option<&End>, best: &End) -> bool {
    new.is_some_and(|n| n.reason == best.reason && (n.tick, n.frame) < (best.tick, best.frame))
}

/// A condition as text (the step logs' misses): kind, subject, value.
pub fn cond_text(c: &Cond) -> String {
    let kind = match c.kind {
        CondKind::Var => "var",
        CondKind::Bit => "bit",
        CondKind::Room => "room",
        CondKind::Owner => "owner",
        CondKind::State => "state",
        CondKind::Has => "has",
        CondKind::ActorRoom => "actorroom",
        CondKind::ActorX => "actorx",
        CondKind::ActorY => "actory",
        CondKind::Class => "class",
        CondKind::ActorMoving => "moving",
        CondKind::Script => "script",
    };
    format!("{kind}:{}{}{}", c.subject, if c.negate { "!=" } else { "=" }, c.value)
}

/// The goal's conditions that do not hold at `e`'s stop (each asked alone).
pub fn misses(e: &mut Box<dyn Engine>, goal: &Goal) -> EResult<Vec<String>> {
    let Goal::Keep(cs) = goal else { return Ok(Vec::new()) };
    let mut out = Vec::new();
    for c in cs {
        if !e.holds(std::slice::from_ref(c))? {
            out.push(cond_text(c));
        }
    }
    Ok(out)
}
