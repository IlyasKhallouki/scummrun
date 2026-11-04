//! E016 section C (`FMI_NOGOOD=1`, experiments/E016-realize-run/plan.md, "C. Failure memory at the exact entry"):
//! what a try remembers of the plans that failed, and what the archive keeps of it.
//!
//! * **The refutation memory** (`Refuted`). A plan is made at an exact state: an archive entry, the inputs played from
//!   it in the attempt and the frame (`prefix_key`). When one of its windows ends empty (no child of the window search
//!   gave the step: the step was refused from where the plan stood), the step's label is banned at the plan's predicted
//!   state before it (`ban_of`): the step's precondition keys and the room, at the values the plan predicted. The ban is
//!   keyed by (entry, prefix, target) and given to the planner (taskcore's bans) whenever a plan for that target is made
//!   at that exact state again, in this attempt or a later try from the same entry. The engine is deterministic from an
//!   exact entry, so the same plan from there would be refused the same way. A ban never leaves its entry.
//! * **The no-plan memory** (`NoPlan`). When the planner proves no plan (status "unsolvable" or "incomplete", asked with
//!   no bans), the result is keyed by (the reading, the start restricted to the reading's tested keys and the target's
//!   keys, the target). A later request with the same key is answered from the memory. It is sound relative to the
//!   reading only, so the archive drops it once a newer reading of that reading's scope is registered (or the reading
//!   is evicted).
//!
//! job.json "nogood" gives a try its entry's refutations and the no-plan records of the readings it may use; result.json
//! "nogood" holds what the try added; the archive's log line keeps it. Off (the default): none of it runs or is written.

use fmi_core::{Action, Ban, EffVal, Event, Facts, Key, PlanStep, Val};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

/// The switch: `FMI_NOGOOD=1`.
pub fn on() -> bool {
    std::env::var("FMI_NOGOOD").is_ok_and(|v| v == "1")
}

fn sha1_hex(s: &str) -> String {
    sha1_smol::Sha1::from(s.as_bytes()).digest().to_string()
}

/// The exact state a plan is made at, within its entry: the inputs played from the entry (absolute frames) and the
/// frame, as a digest.
pub fn prefix_key(events: &[Event], frame: u64) -> String {
    let evs: Vec<Value> = events.iter().map(crate::events::event_json).collect();
    sha1_hex(&json!([frame, evs]).to_string())
}

/// A target (the harness's goal) as text: its keys and value sets, sorted.
pub fn target_text(goal: &[(Key, Vec<Val>)]) -> String {
    let mut t: Vec<(String, Vec<Val>)> = goal
        .iter()
        .map(|(k, vs)| {
            let mut v = vs.clone();
            v.sort_unstable();
            v.dedup();
            (k.text(), v)
        })
        .collect();
    t.sort();
    json!(t).to_string()
}

/// A start restricted to `keys` (the reading's tested keys and the target's), as a digest; a key the start lacks is
/// absent from it too.
pub fn start_key(start: &Facts, keys: &HashSet<Key>) -> String {
    let mut t: Vec<(String, Val)> = start.iter().filter(|(k, _)| keys.contains(k)).map(|(k, v)| (k.text(), *v)).collect();
    t.sort();
    sha1_hex(&json!(t).to_string())
}

/// A step's label from its path's name ("label#rec", or the label): the name less a last "#digits".
pub fn label_of_name(name: &str) -> String {
    match name.rsplit_once('#') {
        Some((l, n)) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => l.to_string(),
        _ => name.to_string(),
    }
}

/// The plan's state before step `i` as the plan predicts it, for the keys it can tell: the start with steps 0..i's
/// effects (a set value, or the one value of a value set; an effect whose condition holds; an added value or a value
/// of several is not told, and the key is left out).
pub fn predicted_before(start: &Facts, paths: &[Action], i: usize) -> HashMap<Key, Val> {
    let mut known: HashMap<Key, Val> = start.iter().copied().collect();
    for a in paths.iter().take(i) {
        let before = known.clone();
        for e in &a.effects {
            // the condition on the state before the step: holds, fails, or not told (a key not told)
            let holds = e.when.iter().map(|(k, vs)| before.get(k).map(|v| vs.contains(v))).collect::<Option<Vec<bool>>>();
            let v = match &e.v {
                EffVal::Value(v) => Some(*v),
                EffVal::Values(vs) if vs.len() == 1 => Some(vs[0]),
                _ => None,
            };
            match (holds.map(|h| h.iter().all(|x| *x)), v) {
                (Some(false), _) => {}
                (Some(true), Some(v)) => {
                    known.insert(e.key, v);
                }
                _ => {
                    known.remove(&e.key);
                }
            }
        }
    }
    known
}

/// The ban of a refused step: its label, and its path's precondition keys and the room at the values the plan predicted
/// before it (`predicted_before`) that its precondition allows, among the reading's keys `has`. None when no such key
/// is told (a ban on no key would cut the label everywhere).
pub fn ban_of(label: &str, start: &Facts, paths: &[Action], i: usize, has: &dyn Fn(Key) -> bool) -> Option<Ban> {
    let path = paths.get(i)?;
    let pred = predicted_before(start, paths, i);
    let mut keys: Vec<(Key, Option<&Vec<Val>>)> = path.pre.iter().map(|(k, vs)| (*k, Some(vs))).collect();
    let room = Key::parse("room");
    if !keys.iter().any(|(k, _)| *k == room) {
        keys.push((room, None));
    }
    let mut point: Vec<(Key, Val)> = keys
        .into_iter()
        .filter(|(k, _)| has(*k))
        .filter_map(|(k, vs)| {
            let v = *pred.get(&k)?;
            vs.map_or(true, |s| s.contains(&v)).then_some((k, v))
        })
        .collect();
    point.sort();
    point.dedup();
    (!point.is_empty()).then(|| Ban { label: label.to_string(), facts: point })
}

/// A refutation (module docstring).
#[derive(Clone, Debug, PartialEq)]
pub struct Refuted {
    pub entry: i64,
    pub prefix: String,
    pub target: String,
    pub ban: Ban,
    /// the refused step's name, the plan's frame and number, and why (the window's end)
    pub step: String,
    pub frame: u64,
    pub plan: u32,
    pub why: String,
}

impl Refuted {
    fn same(&self, o: &Refuted) -> bool {
        self.entry == o.entry && self.prefix == o.prefix && self.target == o.target && self.ban == o.ban
    }
    pub fn json(&self) -> Value {
        json!({"entry": self.entry, "prefix": self.prefix, "target": self.target, "label": self.ban.label,
               "point": self.ban.facts.iter().map(|(k, v)| json!([k.text(), v])).collect::<Vec<_>>(),
               "step": self.step, "frame": self.frame, "plan": self.plan, "why": self.why})
    }
    pub fn of_json(v: &Value) -> Option<Refuted> {
        let point = v.get("point")?.as_array()?.iter()
            .map(|p| Some((Key::parse(p.get(0)?.as_str()?), p.get(1)?.as_i64()?)))
            .collect::<Option<Vec<_>>>()?;
        Some(Refuted {
            entry: v.get("entry")?.as_i64()?,
            prefix: v.get("prefix")?.as_str()?.to_string(),
            target: v.get("target")?.as_str()?.to_string(),
            ban: Ban { label: v.get("label")?.as_str()?.to_string(), facts: point },
            step: v.get("step").and_then(|s| s.as_str()).unwrap_or("").to_string(),
            frame: v.get("frame").and_then(|f| f.as_u64()).unwrap_or(0),
            plan: v.get("plan").and_then(|f| f.as_u64()).unwrap_or(0) as u32,
            why: v.get("why").and_then(|s| s.as_str()).unwrap_or("").to_string(),
        })
    }
}

/// A no-plan record (module docstring).
#[derive(Clone, Debug, PartialEq)]
pub struct NoPlan {
    /// the reading's reading.json path, and its scope
    pub reading: String,
    pub scope: Vec<i64>,
    pub start: String,
    pub target: String,
    /// the planner's status ("unsolvable" or "incomplete")
    pub status: String,
}

impl NoPlan {
    fn same(&self, o: &NoPlan) -> bool {
        self.reading == o.reading && self.start == o.start && self.target == o.target
    }
    pub fn json(&self) -> Value {
        json!({"reading": self.reading, "scope": self.scope, "start": self.start, "target": self.target, "status": self.status})
    }
    pub fn of_json(v: &Value) -> Option<NoPlan> {
        Some(NoPlan {
            reading: v.get("reading")?.as_str()?.to_string(),
            scope: v.get("scope")?.as_array()?.iter().filter_map(|x| x.as_i64()).collect(),
            start: v.get("start")?.as_str()?.to_string(),
            target: v.get("target")?.as_str()?.to_string(),
            status: v.get("status")?.as_str()?.to_string(),
        })
    }
}

/// The plan statuses a no-plan record is made of: the planner proved there is none (not a limit, not an error).
pub fn proves_none(status: &str) -> bool {
    matches!(status, "unsolvable" | "incomplete")
}

/// A try's memory: what job.json gave and what its attempts add, shared by the try's attempts.
#[derive(Clone, Debug, Default)]
pub struct Memory {
    pub entry: i64,
    refuted: Vec<Refuted>,
    no_plan: Vec<NoPlan>,
    new_refuted: Vec<Refuted>,
    new_no_plan: Vec<NoPlan>,
    /// how often the memory answered: bans given (plans asked with one or more), no-plans answered
    pub used_bans: usize,
    pub used_no_plan: usize,
}

pub type Shared = Arc<Mutex<Memory>>;

impl Memory {
    /// job.json "nogood" for the try's entry (none: an empty memory).
    pub fn of_job(job: &Value) -> Memory {
        let entry = job.get("entry").and_then(|e| e.get("id")).and_then(|i| i.as_i64()).unwrap_or(-1);
        let ng = job.get("nogood");
        let list = |k: &str| ng.and_then(|n| n.get(k)).and_then(|x| x.as_array()).cloned().unwrap_or_default();
        Memory {
            entry,
            refuted: list("refuted").iter().filter_map(Refuted::of_json).filter(|r| r.entry == entry).collect(),
            no_plan: list("no_plan").iter().filter_map(NoPlan::of_json).collect(),
            ..Default::default()
        }
    }
    /// The bans for a plan at `prefix` for `target`, in the order they were made.
    pub fn bans(&self, prefix: &str, target: &str) -> Vec<Ban> {
        let mut out: Vec<Ban> = Vec::new();
        for r in self.refuted.iter().chain(&self.new_refuted) {
            if r.entry == self.entry && r.prefix == prefix && r.target == target && !out.contains(&r.ban) {
                out.push(r.ban.clone());
            }
        }
        out
    }
    /// A refutation of this try (a repeat of a known one is not kept again); whether it was new.
    pub fn refute(&mut self, r: Refuted) -> bool {
        if r.entry != self.entry || self.refuted.iter().chain(&self.new_refuted).any(|x| x.same(&r)) {
            return false;
        }
        self.new_refuted.push(r);
        true
    }
    pub fn no_plan(&self, reading: &str, start: &str, target: &str) -> Option<&NoPlan> {
        self.no_plan.iter().chain(&self.new_no_plan).find(|n| n.reading == reading && n.start == start && n.target == target)
    }
    pub fn note_no_plan(&mut self, n: NoPlan) -> bool {
        if self.no_plan.iter().chain(&self.new_no_plan).any(|x| x.same(&n)) {
            return false;
        }
        self.new_no_plan.push(n);
        true
    }
    pub fn new_refuted(&self) -> &[Refuted] {
        &self.new_refuted
    }
    pub fn new_no_plan(&self) -> &[NoPlan] {
        &self.new_no_plan
    }
    /// result.json "nogood": what the try added, and the counts of what it was given and used.
    pub fn result_json(&self) -> Value {
        json!({"refuted": self.new_refuted.iter().map(|r| r.json()).collect::<Vec<_>>(),
               "no_plan": self.new_no_plan.iter().map(|n| n.json()).collect::<Vec<_>>(),
               "given": {"refuted": self.refuted.len(), "no_plan": self.no_plan.len()},
               "used": {"bans": self.used_bans, "no_plan": self.used_no_plan}})
    }
}

/// What the harness reads and writes of the memory in one attempt.
#[derive(Clone)]
pub struct Ctx {
    pub mem: Shared,
    /// the reading's tested keys (T's keys, component `tested`); None: every key of the start (the key is then exact)
    pub tested: Option<Arc<dyn Fn(&dyn fmi_core::ReadingSource) -> Option<Vec<Key>> + Send + Sync>>,
}

/// The step index of a window's input in a plan: the first step of that name.
pub fn step_index(steps: &[PlanStep], name: &str) -> Option<usize> {
    steps.iter().position(|s| s.name == name)
}

/// The archive's side: job.json "nogood" for entry `entry` from the committed log lines' "nogood", the no-plan records
/// kept only for the readings in `live` (the readings the try may plan on that are still their scope's newest).
pub fn job_json(lines: &[&Value], entry: i64, live: &HashSet<String>) -> Value {
    let mut refuted: Vec<Value> = Vec::new();
    let mut no_plan: Vec<Value> = Vec::new();
    let mut seen_r: HashSet<String> = HashSet::new();
    let mut seen_n: HashSet<String> = HashSet::new();
    for x in lines {
        let Some(ng) = x.get("nogood") else { continue };
        for r in ng.get("refuted").and_then(|a| a.as_array()).map(|a| a.as_slice()).unwrap_or(&[]) {
            if r.get("entry").and_then(|e| e.as_i64()) == Some(entry) && seen_r.insert(r.to_string()) {
                refuted.push(r.clone());
            }
        }
        for n in ng.get("no_plan").and_then(|a| a.as_array()).map(|a| a.as_slice()).unwrap_or(&[]) {
            let Some(np) = NoPlan::of_json(n) else { continue };
            if live.contains(&np.reading) && seen_n.insert(format!("{}\t{}\t{}", np.reading, np.start, np.target)) {
                no_plan.push(n.clone());
            }
        }
    }
    let mut m = Map::new();
    m.insert("refuted".into(), Value::Array(refuted));
    m.insert("no_plan".into(), Value::Array(no_plan));
    Value::Object(m)
}
