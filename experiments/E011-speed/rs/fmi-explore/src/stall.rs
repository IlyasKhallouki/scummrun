//! E013 component `stall` (experiments/E013-robust/plan.md, section C, and the amendment "component `stall`"): every
//! second of a committed try's wall in one row (phase, outcome) of the harness's own records, each row in one class of
//! a fixed table that names no game fact; the table over the run and over the last 20 committed tries (stalls.json,
//! written by the driver after each commit with `FMI_STALLS=1`); and the two shift rules of the free share
//! (`FMI_FREE_SHIFT=1`), replayed over the committed tries in commit order so that a reopened archive answers the same.
//!
//! It reads files only: the log's committed lines, each try's result.json and its attempts' run.json, and the new
//! entries' rooms and last facts. Nothing here changes what a try or the archive does.

use crate::archive::{fact_map, Archive, FactMap, COMMITTED};
use crate::files::{self, read_json, Res};
use crate::progress::{self, TAtom};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

/// The switches (plan.md, the component table): the live table, and the shift rules for free's kind.
pub const STALLS_ENV: &str = "FMI_STALLS";
pub const SHIFT_ENV: &str = "FMI_FREE_SHIFT";
/// The live table's file in the archive.
pub const STALLS_JSON: &str = "stalls.json";
/// The shift rules' window (committed tries), its threshold and its way back (plan.md C, "The driver's use").
pub const LAST: usize = 20;
pub const SHIFT_UP: f64 = 0.5;
pub const SHIFT_DOWN: f64 = 0.3;
/// The most the shift raises the share to (it never lowers a share above it).
pub const SHARE_CAP: f64 = 0.5;

/// `FMI_STALLS=1`.
pub fn on() -> bool {
    env_is(STALLS_ENV)
}

/// `FMI_FREE_SHIFT=1`.
pub fn shift_on() -> bool {
    env_is(SHIFT_ENV)
}

fn env_is(k: &str) -> bool {
    std::env::var(k).map(|v| v == "1").unwrap_or(false)
}

// --- the classes and the rows ----------------------------------------------------------------------------------------

/// The fixed class table's classes (the amendment): section C's eight, "model, no yield", and E014 amendment (4)'s
/// "reading bound" (a try's fresh reading stopped at the bound, `FMI_READ_BOUND`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Class {
    Productive,
    ModelBlind,
    NoPlan,
    Refused,
    Mismatch,
    InterfaceNoYield,
    ModelNoYield,
    Wait,
    Overhead,
    ReadBound,
}

/// The number of classes.
pub const N_CLASSES: usize = 10;

impl Class {
    pub const ALL: [Class; N_CLASSES] = [
        Class::Productive,
        Class::ModelBlind,
        Class::NoPlan,
        Class::Refused,
        Class::Mismatch,
        Class::InterfaceNoYield,
        Class::ModelNoYield,
        Class::Wait,
        Class::Overhead,
        Class::ReadBound,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Class::Productive => "productive",
            Class::ModelBlind => "model-blind",
            Class::NoPlan => "no plan",
            Class::Refused => "refused input",
            Class::Mismatch => "effect mismatch",
            Class::InterfaceNoYield => "interface, no yield",
            Class::ModelNoYield => "model, no yield",
            Class::Wait => "wait",
            Class::Overhead => "overhead",
            Class::ReadBound => "reading bound",
        }
    }

    pub fn of_name(s: &str) -> Option<Class> {
        Class::ALL.iter().copied().find(|c| c.name() == s)
    }

    fn ix(self) -> usize {
        Class::ALL.iter().position(|c| *c == self).unwrap()
    }
}

/// A row: the phase of the try and its outcome as the harness wrote it (numbers written N).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Row {
    pub phase: String,
    pub outcome: String,
}

fn row(phase: &str, outcome: impl Into<String>) -> Row {
    Row { phase: phase.to_string(), outcome: outcome.into() }
}

/// A record's text with its numbers written N. A digit run right after a letter is part of a name and stays ("R0").
pub fn numbers_n(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    // 0: not in a digit run; 1: a run kept (after a letter); 2: a run written N
    let mut mode = 0u8;
    let mut after_letter = false;
    for c in s.chars() {
        if c.is_ascii_digit() {
            match mode {
                1 => out.push(c),
                2 => {}
                _ => {
                    if after_letter {
                        mode = 1;
                        out.push(c);
                    } else {
                        mode = 2;
                        out.push('N');
                    }
                }
            }
        } else {
            mode = 0;
            out.push(c);
        }
        after_letter = c.is_alphabetic() || c == '_';
    }
    out
}

fn f64_of(v: &Value) -> f64 {
    v.as_f64().unwrap_or(0.0)
}

// --- the stop table ------------------------------------------------------------------------------------------------

/// A harness stop's kind: its text up to its detail.
pub fn stop_kind(stop: &str) -> String {
    if stop == "goal" || stop == "time limit" || stop == "replan limit" {
        return stop.to_string();
    }
    for (p, k) in [
        ("goal, not from the plan", "goal, not from the plan"),
        ("refinement: Lemma R's premise R0", "refinement: Lemma R's R0"),
        ("refinement: ", "refinement"),
        ("reach extension: ", "reach extension"),
        ("error: ", "error"),
        ("the engine's state is outside the task's domains", "outside the task's domains"),
        ("the same empty window", "the same empty window"),
        ("planner timeout", "planner timeout"),
        ("planner memory limit", "planner memory limit"),
        ("planner: ", "planner error"),
    ] {
        if stop.starts_with(p) {
            return k.to_string();
        }
    }
    if stop.starts_with("replan at frame") && stop.ends_with("the same plan again") {
        return "the same plan again".into();
    }
    if stop.contains("Lemma 2's premise failed") {
        return "Lemma 2's premise".into();
    }
    if let Some(r) = stop.strip_prefix("engine: engine ended: ") {
        return format!("engine ended: {}", fmi_core::end_base(end_reason_of(r)));
    }
    if stop.starts_with("engine: ") {
        return "engine error".into();
    }
    numbers_n(stop.split(": ").next().unwrap_or(stop))
}

fn end_reason_of(r: &str) -> &str {
    r.split(['\n', ';']).next().unwrap_or("").trim()
}

/// The stop class of a harness stop (the amendment's second table). `empty_refused` says, for "the same empty
/// window", whether its windows were refusals (`empty_refused`).
pub fn stop_class(stop: &str, empty_refused: bool) -> Class {
    if stop == "goal" || stop.starts_with("goal, not from the plan") {
        Class::ModelNoYield
    } else if stop.starts_with("refinement: ")
        || stop.starts_with("reach extension: ")
        || stop.starts_with("the engine's state is outside the task's domains")
        || stop.contains("Lemma 2's premise failed")
    {
        Class::ModelBlind
    } else if stop.starts_with("no plan (") || stop.starts_with("planner") || stop == "time limit" {
        Class::NoPlan
    } else if stop.starts_with("the same empty window") {
        if empty_refused {
            Class::Refused
        } else {
            Class::Mismatch
        }
    } else if stop == "replan limit" || (stop.starts_with("replan at frame") && stop.ends_with("the same plan again")) {
        Class::Mismatch
    } else if let Some(r) = stop.strip_prefix("engine: engine ended: ") {
        if fmi_core::is_game_end(end_reason_of(r)) {
            Class::ModelNoYield
        } else {
            Class::Overhead
        }
    } else {
        Class::Overhead
    }
}

/// A window record that ended at an end point (a child chosen, or the plan matched or the goal held in the parent).
fn end_point_window(w: &Value) -> bool {
    let e = w["end_reason"].as_str().unwrap_or("");
    e.starts_with("ready: the first end point")
        || e.starts_with("stop: Q(g) + b reached")
        || e == "matched in the parent"
        || e.starts_with("goal in the parent")
}

/// The children's outcomes a window record counts, "cancelled" left out, sorted.
fn child_outcomes(w: &Value) -> Vec<String> {
    let mut out: Vec<String> = w["outcomes"]
        .as_object()
        .map(|m| m.iter().filter(|(k, n)| k.as_str() != "cancelled" && n.as_u64() != Some(0)).map(|(k, _)| k.clone()).collect())
        .unwrap_or_default();
    out.sort();
    out
}

/// A refusal: a window B ended ("refused input: ..."), or one that ran to its frame bound with every try failing at
/// the interface (the executor could not make the input take).
pub fn refused_window(w: &Value) -> bool {
    let e = w["end_reason"].as_str().unwrap_or("");
    if e.starts_with("refused input") {
        return true;
    }
    let bound = e.strip_suffix(" frames").map(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())).unwrap_or(false);
    let oc = child_outcomes(w);
    bound && !oc.is_empty() && oc.iter().all(|k| k == "interface")
}

/// For a stop "the same empty window N times on R: ...": whether every empty window on R in the attempt's records is
/// a refusal.
pub fn empty_refused(stop: &str, run: Option<&Value>) -> bool {
    let Some(ws) = run.and_then(|r| r["windows"].as_array()) else { return false };
    let empty: Vec<&Value> = ws
        .iter()
        .filter(|w| w["input"].as_str().map(|i| stop.contains(&format!(" on {i}: "))).unwrap_or(false))
        .filter(|w| !end_point_window(w))
        .collect();
    !empty.is_empty() && empty.iter().all(|w| refused_window(w))
}

/// A window's play row outcome: its end reason, then its children's outcomes.
fn play_outcome(w: &Value) -> String {
    let e = numbers_n(w["end_reason"].as_str().unwrap_or("?"));
    let oc = child_outcomes(w);
    if oc.is_empty() {
        e
    } else {
        format!("{e}; {}", oc.join("+"))
    }
}

/// A trial verdict that is a refusal (B's `dropped: ...` and `ignored`).
pub fn refused_verdict(v: &str) -> bool {
    v.starts_with("dropped") || v == "ignored"
}

// --- one try ----------------------------------------------------------------------------------------------------------

/// What one committed try's rows are made of.
pub struct TryIn<'a> {
    /// result.json
    pub result: &'a Value,
    /// each request's attempt's run.json, by its folder ("a00"), where there is one
    pub runs: &'a BTreeMap<String, Value>,
    /// per result.json "new" item: whether the entry is new at the commit (the amendment, "Which entries are new")
    pub new: &'a [bool],
    /// the log line's outcome: "ok", "error" or "return differs"
    pub outcome: &'a str,
    /// "model" or "free"
    pub kind: &'a str,
}

/// A try's rows, its stop's kind and class, and whether its attempt kept a new entry.
pub struct Account {
    pub rows: Vec<(Row, Class, f64)>,
    pub stop: String,
    pub stop_class: Class,
    pub attempt_new: bool,
}

const ATTEMPT_HOWS: [&str; 3] = ["checkpoint", "end", "delay"];
/// An affordance step's E014 end entry (the part `part_of_item` names; never an archive "how").
const AFFORD_END: &str = "afford: end";

/// A result.json "new" item's part (the stall amendment (2)): its "how"; for E014's end entries (an extra of how
/// "end", `mech::end_extra`: only the extras' items carry "novel") the part whose dead move the same end made (the
/// move at the same frame with a game end), the attempt's when there is none.
fn part_of_item(n: &Value, dead: &[Value]) -> String {
    let how = n["how"].as_str().filter(|h| !h.is_empty()).unwrap_or("checkpoint");
    if how != "end" || n.get("novel").is_none() {
        return how.to_string();
    }
    let f = n["frame"].as_u64();
    let by = dead
        .iter()
        .find(|d| f.is_some() && d["frame"].as_u64() == f && fmi_core::is_game_end(d["reason"].as_str().unwrap_or("")))
        .and_then(|d| d["how"].as_str());
    match by {
        Some("wait") => "wait",
        Some("afford") => AFFORD_END,
        Some("free") => "free",
        _ => "end",
    }
    .to_string()
}

/// The affordance step an item belongs to, given the afford items up to it (`c`) and each step's kept count: the step
/// whose kept items reach the c-th (an end entry: the step of the afford items before it); the last step past them.
fn step_of(kept: &[usize], c: usize) -> Option<usize> {
    let mut cum = 0usize;
    for (s, n) in kept.iter().enumerate() {
        cum += n;
        if cum >= c {
            return Some(s);
        }
    }
    kept.len().checked_sub(1)
}

/// Seconds by verdict, or the whole part as one row: (label: verdict) rows, the part's other seconds as (label:
/// other); refusals are refused input, the rest `taken`.
fn verdict_rows(rows: &mut Vec<(Row, Class, f64)>, phase: &str, label: &str, whole: &str, total: f64, vs: Option<&Map<String, Value>>, taken: Class) {
    match vs.filter(|m| !m.is_empty()) {
        Some(m) => {
            let mut ks: Vec<(&String, &Value)> = m.iter().collect();
            ks.sort_by(|a, b| a.0.cmp(b.0));
            let mut used = 0.0;
            for (v, s) in ks {
                let s = f64_of(s);
                used += s;
                let c = if refused_verdict(v) { Class::Refused } else { taken };
                rows.push((row(phase, format!("{label}: {}", numbers_n(v))), c, s));
            }
            rows.push((row(phase, format!("{label}: other")), taken, (total - used).max(0.0)));
        }
        None => rows.push((row(phase, whole), taken, total)),
    }
}

/// The amendment's attribution of one try: every part once, the rest the wall less the parts.
pub fn account(t: &TryIn) -> Account {
    let r = t.result;
    let dead: &[Value] = r["dead"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);
    let hows: Vec<String> = r["new"].as_array().map(|a| a.iter().map(|n| part_of_item(n, dead)).collect()).unwrap_or_default();
    let flag = |k: usize| t.new.get(k).copied().unwrap_or(false);
    let any_of = |hs: &[&str]| hows.iter().enumerate().any(|(k, h)| hs.contains(&h.as_str()) && flag(k));
    let attempt_new = any_of(&ATTEMPT_HOWS);
    let wait_new = any_of(&["wait"]);
    let free_new = any_of(&["free"]);
    let steps: &[Value] = r["e012"]["afford"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);
    let kept: Vec<usize> = steps.iter().map(|a| a["kept"].as_u64().unwrap_or(0) as usize).collect();
    // each step's entries: the next `kept` items of how "afford", and the end entries its trials made
    let mut step_new = vec![false; steps.len()];
    let mut c = 0usize;
    for (k, h) in hows.iter().enumerate() {
        let s = match h.as_str() {
            "afford" => {
                c += 1;
                step_of(&kept, c)
            }
            AFFORD_END => step_of(&kept, c),
            _ => None,
        };
        if let Some(s) = s {
            step_new[s] |= flag(k);
        }
    }
    let any_new = (0..hows.len()).any(flag);
    let free = t.kind == "free";
    // E014 amendment (4) (a): a model try whose fresh reading reached the bound went on model-free
    let bounded = r["reading"]["bound"].is_object();
    let probes: Vec<Value> = r["probes"].as_array().cloned().unwrap_or_default();
    let e12 = &r["e012"];

    // the stop
    let ok = files::truthy(r.get("ok"));
    let att = &r["attempt"];
    let (stop, stop_class) = if t.outcome == "return differs" {
        ("the return differs".to_string(), Class::Overhead)
    } else if t.outcome == "error" || !ok {
        ("error".to_string(), Class::Overhead)
    } else if free {
        ("free".to_string(), Class::InterfaceNoYield)
    } else if bounded {
        ("reading bound".to_string(), Class::ReadBound)
    } else if att.is_object() {
        match att["stop"].as_str() {
            Some(s) => {
                let played = probes.iter().rev().find(|p| p["status"].as_str() == Some("plan")).and_then(|p| p["attempt"].as_str());
                let run = played.and_then(|a| t.runs.get(a));
                (stop_kind(s), stop_class(s, empty_refused(s, run)))
            }
            None => ("no stop".to_string(), Class::Overhead),
        }
    } else if probes.is_empty() {
        ("none played: no target".to_string(), Class::NoPlan)
    } else if probes.iter().all(|p| p["stop"].as_str().map(|s| s.starts_with("reach extension: ")).unwrap_or(false)) {
        ("none played: reach extension".to_string(), Class::ModelBlind)
    } else {
        ("none played: no plan".to_string(), Class::NoPlan)
    };
    let model = if free || bounded {
        if any_new {
            Class::Productive
        } else {
            Class::InterfaceNoYield
        }
    } else if attempt_new {
        Class::Productive
    } else {
        stop_class
    };

    let mut rows: Vec<(Row, Class, f64)> = Vec::new();
    // return
    let ret = &r["return"];
    if ret.is_object() {
        let same = ret["same"].as_bool().unwrap_or(true);
        rows.push((row("return", if same { "same" } else { "differs" }), Class::Overhead, f64_of(&ret["boot_s"])));
    }
    // E009's idle measurement (the wait replaces it when on: its record is then the wait's)
    if !e12["wait"].is_object() && r["idle"].is_object() {
        rows.push((row("idle", ""), Class::Wait, f64_of(&r["idle"]["seconds"])));
    }
    // the reading
    let rd = &r["reading"];
    if rd.is_object() && bounded {
        // the seconds waited for the reading, then the stand-in's load (E014 amendment (4) (a))
        rows.push((row("read", "bound"), Class::ReadBound, f64_of(&rd["bound"]["waited_s"])));
        let stand = if rd["path"].is_string() { "stand-in" } else { "no stand-in" };
        rows.push((row("read", stand), model, f64_of(&rd["load_s"])));
    } else if rd.is_object() {
        let s = if files::truthy(rd.get("fresh")) { f64_of(&rd["wall_s"]) + f64_of(&rd["load_s"]) } else { f64_of(&rd["load_s"]) };
        rows.push((row("read", numbers_n(rd["why"].as_str().unwrap_or(""))), model, s));
    }
    if rd["retry_bound"].is_object() {
        rows.push((row("read", "bound: the reading again"), Class::ReadBound, f64_of(&rd["retry_bound"]["waited_s"])));
    }
    // T and the wait
    if e12["t"].is_object() {
        rows.push((row("T", ""), model, f64_of(&e12["t"]["seconds"])));
    }
    if e12["wait"].is_object() {
        let w = &e12["wait"];
        let c = if wait_new { Class::Productive } else { Class::Wait };
        rows.push((row("wait", numbers_n(w["end"].as_str().unwrap_or(""))), c, f64_of(&w["seconds"])));
    }
    // the requests: their windows (play), refinements and extensions (read), and the rest of each (plan)
    for p in &probes {
        let total = f64_of(&p["seconds"]);
        let run = p["attempt"].as_str().and_then(|a| t.runs.get(a));
        let mut inner = 0.0;
        if let Some(run) = run {
            for w in run["windows"].as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
                let s = f64_of(&w["wall_s"]);
                inner += s;
                rows.push((row("play", play_outcome(w)), model, s));
            }
            for x in run["refinements"].as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
                if x["seconds"].is_number() {
                    let s = f64_of(&x["seconds"]);
                    inner += s;
                    let added = x["added"].as_array().map(|a| !a.is_empty()).unwrap_or(false);
                    rows.push((row("read: refinement", if added { "added" } else { "none added" }), model, s));
                }
            }
            for x in run["extensions"].as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
                if x["seconds"].is_number() {
                    let s = f64_of(&x["seconds"]);
                    inner += s;
                    let replan = x["replan"].as_bool().unwrap_or(false);
                    rows.push((row("read: extension", if replan { "replan" } else { "no replan" }), model, s));
                }
            }
        }
        let outcome = match p["status"].as_str() {
            Some(s) => numbers_n(s),
            None => p["stop"].as_str().map(stop_kind).unwrap_or_else(|| "none".into()),
        };
        rows.push((row("plan", outcome), model, (total - inner).max(0.0)));
    }
    // the affordance steps
    for (s, a) in steps.iter().enumerate() {
        let why = numbers_n(a["why"].as_str().unwrap_or("?"));
        let whole = match a["offer"]["idle"].as_str() {
            Some(kind) => format!("{why}, {kind}"),
            None => why.clone(),
        };
        let taken = if step_new[s] { Class::Productive } else { Class::InterfaceNoYield };
        verdict_rows(&mut rows, "afford", &why, &whole, f64_of(&a["step_seconds"]), a["verdict_s"].as_object(), taken);
    }
    // a free try's step (free's record, the stall amendments (2) and (3)): each offer point, the delays, the rest of it
    let fr = &r["free"];
    if fr.is_object() {
        let taken = if free_new { Class::Productive } else { Class::InterfaceNoYield };
        let mut used = 0.0;
        for p in fr["points"].as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
            let s = f64_of(&p["seconds"]);
            used += s;
            let label = format!("{}, level {}", numbers_n(p["where"].as_str().unwrap_or("?")), p["level"].as_i64().unwrap_or(1));
            verdict_rows(&mut rows, "free", &label, &label, s, p["record"]["verdict_s"].as_object(), taken);
        }
        if let Some(s) = fr["delays"]["seconds"].as_f64() {
            used += s;
            rows.push((row("free", "delays"), taken, s));
        }
        rows.push((row("free", "other"), taken, (f64_of(&fr["seconds"]) - used).max(0.0)));
    }
    // the rest
    let wall = f64_of(&r["wall_s"]);
    let parts: f64 = rows.iter().map(|x| x.2).sum();
    rows.push((row("rest", ""), Class::Overhead, wall - parts));
    let stop_class = if free { dominant(&rows).unwrap_or(Class::InterfaceNoYield) } else { stop_class };
    Account { rows, stop, stop_class, attempt_new }
}

/// The class with most of the rows' seconds other than overhead and productive (a free try's stop class).
fn dominant(rows: &[(Row, Class, f64)]) -> Option<Class> {
    let mut s = [0.0f64; N_CLASSES];
    for (_, c, x) in rows {
        s[c.ix()] += x;
    }
    Class::ALL
        .iter()
        .copied()
        .filter(|c| *c != Class::Overhead && *c != Class::Productive)
        .filter(|c| s[c.ix()] > 0.0)
        .fold(None, |best: Option<Class>, c| match best {
            Some(b) if s[b.ix()] >= s[c.ix()] => Some(b),
            _ => Some(c),
        })
}

// --- the entries a commit made ------------------------------------------------------------------------------------------

/// What the novelty of a commit's entries needs: each entry's room and last facts.
pub trait Entries {
    fn room(&self, i: usize) -> Option<i64>;
    fn facts(&self, i: usize) -> Res<FactMap>;
}

impl Entries for Archive {
    fn room(&self, i: usize) -> Option<i64> {
        self.rows.get(i).and_then(|r| r.room)
    }
    fn facts(&self, i: usize) -> Res<FactMap> {
        Archive::facts(self, i)
    }
}

/// The entries from an archive's files (entries.jsonl and entries/ID.json.gz), read only.
pub struct FileEntries {
    root: PathBuf,
    rooms: Vec<Option<i64>>,
}

impl FileEntries {
    pub fn open(root: &Path) -> Res<FileEntries> {
        let (rows, _) = files::read_jsonl(&root.join("entries.jsonl"))?;
        let mut rooms = Vec::with_capacity(rows.len());
        for (k, r) in rows.iter().enumerate() {
            if r["id"].as_u64() != Some(k as u64) {
                return Err(format!("{}/entries.jsonl: entry {} at line {k}", root.display(), r["id"]));
            }
            rooms.push(r["room"].as_i64());
        }
        Ok(FileEntries { root: root.to_path_buf(), rooms })
    }
}

impl Entries for FileEntries {
    fn room(&self, i: usize) -> Option<i64> {
        self.rooms.get(i).copied().flatten()
    }
    fn facts(&self, i: usize) -> Res<FactMap> {
        let doc = files::read_gz(&self.root.join("entries").join(format!("{i}.json.gz")))?;
        let cp = doc["checkpoints"].as_array().and_then(|a| a.last()).ok_or(format!("entry {i}: no checkpoint"))?;
        Ok(fact_map(&cp[1]))
    }
}

// --- the table -------------------------------------------------------------------------------------------------------

/// One committed try in the table.
#[derive(Clone, Debug, PartialEq)]
pub struct TryStall {
    pub job: String,
    pub entry: usize,
    pub kind: String,
    pub outcome: String,
    pub wall: f64,
    pub stop: String,
    pub stop_class: Class,
    /// productive when the commit gave a new atom of T or a new room; else the stop class
    pub class: Class,
    pub gave: bool,
    pub rows: Vec<(Row, Class, f64)>,
    /// the new entries' atoms and rooms give back the line's `new_t` and `new_rooms` (None: the line has neither field)
    pub novelty: Option<bool>,
}

/// The shift's state after a commit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shift {
    pub on: bool,
    pub class: Option<Class>,
}

/// The table over an archive's committed tries, in commit order.
#[derive(Default)]
pub struct Stalls {
    root: PathBuf,
    pub tries: Vec<TryStall>,
    /// T's atoms by their text (the goal's, then each committed line's `t_new`)
    atoms: HashMap<String, TAtom>,
    /// each entry's current cell
    cells: HashMap<usize, String>,
}

fn goal_of(stage_goal: &Value) -> Vec<(String, Vec<i64>)> {
    stage_goal
        .as_object()
        .map(|m| m.iter().map(|(k, vs)| (k.clone(), vs.as_array().map(|a| a.iter().filter_map(|x| x.as_i64()).collect()).unwrap_or_default())).collect())
        .unwrap_or_default()
}

/// cells.json's cells as entry -> cell.
pub fn cells_of_json(cells: &Value) -> HashMap<usize, String> {
    let mut out = HashMap::new();
    if let Some(m) = cells.as_object() {
        for (c, v) in m {
            for e in v["entries"].as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
                if let Some(e) = e.as_u64() {
                    out.insert(e as usize, c.clone());
                }
            }
        }
    }
    out
}

/// The archive's entries' current cells as entry -> cell.
pub fn cells_of_archive(a: &Archive) -> HashMap<usize, String> {
    a.rows.iter().map(|r| (r.id, r.cell.clone())).collect()
}

impl Stalls {
    /// An empty table for the archive at `root` with the stage's goal.
    pub fn new(root: &Path, goal: &[(String, Vec<i64>)]) -> Stalls {
        let mut s = Stalls { root: root.to_path_buf(), ..Default::default() };
        for a in progress::goal_atoms(goal) {
            s.atoms.insert(a.text(), a);
        }
        s
    }

    /// One try's rows (the amendment's attribution).
    pub fn of_try(t: &TryIn) -> Vec<(Row, Class, f64)> {
        account(t).rows
    }

    /// The table over an archive's files (read only): stage.json's goal, the log's committed lines, each try's files,
    /// the entries and cells.json.
    pub fn of_archive(dir: &Path) -> Res<Stalls> {
        let stage = read_json(&dir.join("stage.json"))?;
        let (lines, _) = files::read_jsonl(&dir.join("log.jsonl"))?;
        let entries = FileEntries::open(dir)?;
        let mut s = Stalls::new(dir, &goal_of(&stage["goal"]));
        for l in &lines {
            s.push(l, &entries)?;
        }
        let cells = if dir.join("cells.json").exists() { read_json(&dir.join("cells.json"))? } else { json!({}) };
        s.set_cells(cells_of_json(&cells));
        Ok(s)
    }

    /// The table from an open archive (its lines in memory, its entries' rows and files).
    pub fn of_open(a: &Archive) -> Res<Stalls> {
        let mut s = Stalls::new(&a.root, &a.goal);
        for l in &a.logs {
            s.push(l, a)?;
        }
        s.set_cells(cells_of_archive(a));
        Ok(s)
    }

    pub fn set_cells(&mut self, cells: HashMap<usize, String>) {
        self.cells = cells;
    }

    /// Each entry's current cell (cells.json's, or the open archive's).
    pub fn cells_now(&self) -> &HashMap<usize, String> {
        &self.cells
    }

    /// The table as it stood after the first k commits (the cells as they are now): the reports' replay.
    pub fn upto(&self, k: usize) -> Stalls {
        Stalls { root: self.root.clone(), tries: self.tries[..k.min(self.tries.len())].to_vec(), atoms: self.atoms.clone(), cells: self.cells.clone() }
    }

    /// A log line: a committed one becomes the table's next try (its job's files read here); others are left out.
    pub fn push(&mut self, line: &Value, entries: &dyn Entries) -> Res<()> {
        let outcome = line["outcome"].as_str().unwrap_or("");
        if !COMMITTED.contains(&outcome) {
            return Ok(());
        }
        for a in progress::atoms_of_json(line.get("t_new")) {
            self.atoms.entry(a.text()).or_insert(a);
        }
        let job = line["job"].as_str().unwrap_or("").to_string();
        let jd = self.root.join("jobs").join(&job);
        let result = read_json(&jd.join("result.json")).unwrap_or(Value::Null);
        let mut runs = BTreeMap::new();
        for p in result["probes"].as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
            if let Some(a) = p["attempt"].as_str() {
                if let Ok(v) = read_json(&jd.join("run").join(a).join("run.json")) {
                    runs.insert(a.to_string(), v);
                }
            }
        }
        let kind = line["kind"].as_str().or_else(|| result["kind"].as_str()).unwrap_or("model").to_string();
        let ids: Vec<usize> = line["new_entries"].as_array().map(|a| a.iter().filter_map(|x| x.as_u64().map(|v| v as usize)).collect()).unwrap_or_default();
        let n_new = result["new"].as_array().map(|a| a.len()).unwrap_or(0);
        let progress = line.get("new_t").is_some() || line.get("new_rooms").is_some();
        let (gave, new, novelty) = if progress {
            let texts: Vec<String> = line["new_t"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
            let rooms: BTreeSet<i64> = line["new_rooms"].as_array().map(|a| a.iter().filter_map(|x| x.as_i64()).collect()).unwrap_or_default();
            let dead: BTreeSet<usize> = line["dead_entries"].as_array().map(|a| a.iter().filter_map(|x| x.as_u64().map(|v| v as usize)).collect()).unwrap_or_default();
            let atoms: Vec<Option<&TAtom>> = texts.iter().map(|t| self.atoms.get(t)).collect();
            let mut seen_t = vec![false; texts.len()];
            let mut seen_r: BTreeSet<i64> = BTreeSet::new();
            let mut flags = Vec::with_capacity(ids.len());
            let mut ok = atoms.iter().all(|a| a.is_some());
            for &i in &ids {
                if dead.contains(&i) {
                    flags.push(false);
                    continue;
                }
                let mut is_new = false;
                if let Some(r) = entries.room(i).filter(|r| rooms.contains(r)) {
                    seen_r.insert(r);
                    is_new = true;
                }
                if !texts.is_empty() {
                    match entries.facts(i) {
                        Ok(f) => {
                            for (k, a) in atoms.iter().enumerate() {
                                if a.map(|a| a.holds_in(&f)).unwrap_or(false) {
                                    seen_t[k] = true;
                                    is_new = true;
                                }
                            }
                        }
                        Err(_) => ok = false,
                    }
                }
                flags.push(is_new);
            }
            ok = ok && seen_t.iter().all(|x| *x) && seen_r == rooms;
            (!texts.is_empty() || !rooms.is_empty(), flags, Some(ok))
        } else {
            // an archive without E012's records: the try's gain is E008's (new cells or reached atoms), for each entry
            let gave = line["new_cells"].as_i64().unwrap_or(0) > 0 || line["new_r"].as_array().map(|a| !a.is_empty()).unwrap_or(false);
            (gave, vec![gave; ids.len()], None)
        };
        let new = if ids.len() == n_new { new } else { vec![false; n_new] };
        let acc = account(&TryIn { result: &result, runs: &runs, new: &new, outcome, kind: &kind });
        let class = if gave { Class::Productive } else { acc.stop_class };
        self.tries.push(TryStall {
            job,
            entry: line["entry"].as_u64().unwrap_or(0) as usize,
            kind,
            outcome: outcome.to_string(),
            wall: f64_of(&result["wall_s"]),
            stop: acc.stop,
            stop_class: acc.stop_class,
            class,
            gave,
            rows: acc.rows,
            novelty,
        });
        Ok(())
    }

    /// Seconds by class over tries[lo..hi].
    fn by_class(&self, lo: usize, hi: usize) -> [f64; N_CLASSES] {
        let mut s = [0.0f64; N_CLASSES];
        for t in &self.tries[lo..hi] {
            for (_, c, x) in &t.rows {
                s[c.ix()] += x;
            }
        }
        s
    }

    fn window(&self, i: usize) -> (usize, usize) {
        let hi = i + 1;
        (hi.saturating_sub(LAST), hi)
    }

    /// The shift rule replayed after each commit: each commit's last-20 shares by class and the state after it.
    pub fn shift_trace(&self) -> Vec<([f64; N_CLASSES], Shift)> {
        let mut st = Shift { on: false, class: None };
        let mut out = Vec::with_capacity(self.tries.len());
        for i in 0..self.tries.len() {
            let (lo, hi) = self.window(i);
            let s = self.by_class(lo, hi);
            let total: f64 = s.iter().sum();
            let mut fr = [0.0f64; N_CLASSES];
            if total > 0.0 {
                for k in 0..N_CLASSES {
                    fr[k] = s[k] / total;
                }
                if let (true, Some(c)) = (st.on, st.class) {
                    if fr[c.ix()] < SHIFT_DOWN {
                        st = Shift { on: false, class: None };
                    }
                }
                if !st.on {
                    let top = Class::ALL
                        .iter()
                        .copied()
                        .filter(|c| *c != Class::Productive)
                        .fold(None, |b: Option<Class>, c| match b {
                            Some(b) if fr[b.ix()] >= fr[c.ix()] => Some(b),
                            _ => Some(c),
                        })
                        .unwrap();
                    if fr[top.ix()] >= SHIFT_UP {
                        st = Shift { on: true, class: Some(top) };
                    }
                }
            }
            out.push((fr, st));
        }
        out
    }

    /// The shift's state after the last commit.
    pub fn shift(&self) -> Shift {
        self.shift_trace().last().map(|x| x.1).unwrap_or(Shift { on: false, class: None })
    }

    /// The free share: ρ0, or with the shift on max(ρ0, min(2ρ0, 0.5)).
    pub fn share(&self, rho0: f64) -> f64 {
        if self.shift().on {
            rho0.max((2.0 * rho0).min(SHARE_CAP))
        } else {
            rho0
        }
    }

    /// The cell rule: the cell's last two committed tries are model tries, neither productive, in the same stop class.
    pub fn free_cell(&self, cell: &str) -> bool {
        let ts: Vec<&TryStall> = self.tries.iter().filter(|t| self.cells.get(&t.entry).map(|c| c == cell).unwrap_or(false)).collect();
        let n = ts.len();
        if n < 2 {
            return false;
        }
        let (a, b) = (ts[n - 2], ts[n - 1]);
        a.kind == "model" && b.kind == "model" && !a.gave && !b.gave && a.stop_class == b.stop_class
    }

    /// Every cell the cell rule names now, sorted.
    pub fn free_cells(&self) -> Vec<String> {
        let cells: BTreeSet<&String> = self.tries.iter().filter_map(|t| self.cells.get(&t.entry)).collect();
        cells.into_iter().filter(|c| self.free_cell(c)).cloned().collect()
    }

    /// The tries' total wall and the rows' total (equal by construction).
    pub fn totals(&self) -> (f64, f64) {
        let w = self.tries.iter().map(|t| t.wall).sum();
        let r = self.tries.iter().flat_map(|t| t.rows.iter().map(|x| x.2)).sum();
        (w, r)
    }

    fn rows_table(&self, lo: usize, hi: usize) -> Vec<(Row, Class, f64, usize)> {
        let mut m: BTreeMap<(Row, Class), (f64, usize)> = BTreeMap::new();
        for t in &self.tries[lo..hi] {
            let mut seen: BTreeSet<(Row, Class)> = BTreeSet::new();
            for (r, c, x) in &t.rows {
                let e = m.entry((r.clone(), *c)).or_insert((0.0, 0));
                e.0 += x;
                if seen.insert((r.clone(), *c)) {
                    e.1 += 1;
                }
            }
        }
        let mut v: Vec<(Row, Class, f64, usize)> = m.into_iter().map(|((r, c), (x, n))| (r, c, x, n)).collect();
        v.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal).then_with(|| (&a.0, a.1).cmp(&(&b.0, b.1))));
        v
    }

    fn part_json(&self, lo: usize, hi: usize) -> Value {
        let s = self.by_class(lo, hi);
        let total: f64 = s.iter().sum();
        let share = |x: f64| if total > 0.0 { r6(x / total) } else { 0.0 };
        let rest: f64 = self.tries[lo..hi].iter().flat_map(|t| t.rows.iter().filter(|x| x.0.phase == "rest").map(|x| x.2)).sum();
        let rest_abs: f64 = self.tries[lo..hi].iter().flat_map(|t| t.rows.iter().filter(|x| x.0.phase == "rest").map(|x| x.2.abs())).sum();
        json!({
            "tries": hi - lo,
            "jobs": if hi > lo { json!([self.tries[lo].job, self.tries[hi - 1].job]) } else { json!([]) },
            "wall": r6(self.tries[lo..hi].iter().map(|t| t.wall).sum()),
            "seconds": r6(total),
            "classes": Class::ALL.iter().map(|c| json!([c.name(), r6(s[c.ix()]), share(s[c.ix()])])).collect::<Vec<_>>(),
            "rest": {"seconds": r6(rest), "share": share(rest), "abs": r6(rest_abs), "abs_share": share(rest_abs)},
            "rows": self.rows_table(lo, hi).into_iter().map(|(r, c, x, n)| json!([r.phase, r.outcome, c.name(), r6(x), n])).collect::<Vec<_>>(),
        })
    }

    /// stalls.json (the amendment): the run, the last 20, the shift's trace, the cells the cell rule names, each try.
    pub fn json(&self) -> Value {
        let n = self.tries.len();
        let trace = self.shift_trace();
        let mut changes = Vec::new();
        let mut prev = Shift { on: false, class: None };
        for (i, (fr, st)) in trace.iter().enumerate() {
            if *st != prev {
                let c = st.class.or(prev.class);
                changes.push(json!([i, self.tries[i].job, if st.on { "on" } else { "off" }, c.map(|c| c.name()), c.map(|c| r6(fr[c.ix()]))]));
            }
            prev = *st;
        }
        let last = self.shift();
        json!({
            "format": "e013-stalls-1",
            "run": self.part_json(0, n),
            "last20": self.part_json(n.saturating_sub(LAST), n),
            "shift": {"on": last.on, "class": last.class.map(|c| c.name()), "changes": changes},
            "trace": trace.iter().enumerate().map(|(i, (fr, st))| json!([i, self.tries[i].job, fr.iter().map(|x| r6(*x)).collect::<Vec<_>>(), st.on, st.class.map(|c| c.name())])).collect::<Vec<_>>(),
            "free_cells": self.free_cells(),
            "tries": self.tries.iter().map(|t| json!({
                "job": t.job, "entry": t.entry, "kind": t.kind, "outcome": t.outcome, "wall": t.wall, "stop": t.stop,
                "stop_class": t.stop_class.name(), "class": t.class.name(), "gave": t.gave, "novelty": t.novelty,
                "rows": t.rows.iter().map(|(r, c, x)| json!([r.phase, r.outcome, c.name(), r6(*x)])).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        })
    }

    /// The driver's line: the run's top three classes, the last 20's, and the shift.
    pub fn line(&self) -> String {
        let n = self.tries.len();
        let top = |lo: usize, hi: usize| -> String {
            let s = self.by_class(lo, hi);
            let total: f64 = s.iter().sum();
            let mut v: Vec<(Class, f64)> = Class::ALL.iter().map(|c| (*c, s[c.ix()])).collect();
            v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0)));
            v.iter().take(3).map(|(c, x)| format!("{} {:.1}%", c.name(), if total > 0.0 { 100.0 * x / total } else { 0.0 })).collect::<Vec<_>>().join(", ")
        };
        let (w, _) = self.totals();
        let sh = self.shift();
        format!(
            "stalls: {n} tries, {w:.1} s: {}; last {}: {}; shift {}",
            top(0, n),
            n.min(LAST),
            top(n.saturating_sub(LAST), n),
            match (sh.on, sh.class) {
                (true, Some(c)) => format!("on ({})", c.name()),
                _ => "off".into(),
            }
        )
    }

    /// Writes ROOT/stalls.json.
    pub fn write(&self) -> Res<()> {
        files::write_json(&self.root.join(STALLS_JSON), &self.json(), Some(1))
    }
}

fn r6(x: f64) -> f64 {
    (x * 1e6).round() / 1e6
}

/// The interface's free functions (plan.md, the component table).
pub fn share(s: &Stalls, rho0: f64) -> f64 {
    s.share(rho0)
}

pub fn free_cell(s: &Stalls, cell: &str) -> bool {
    s.free_cell(cell)
}

/// The driver's live table (`FMI_STALLS=1`): opened from the archive, then one try per commit, stalls.json written each
/// time.
pub struct Live {
    pub stalls: Stalls,
}

impl Live {
    pub fn open(a: &Archive) -> Res<Live> {
        let l = Live { stalls: Stalls::of_open(a)? };
        l.stalls.write()?;
        Ok(l)
    }

    /// After `a.ingest` gave `line`: the try's rows, the cells as the commit left them, and stalls.json.
    pub fn commit(&mut self, a: &Archive, line: &Value) -> Res<()> {
        self.stalls.push(line, a)?;
        self.stalls.set_cells(cells_of_archive(a));
        self.stalls.write()
    }
}

/// free's port (`free::Shift`, the stall amendment (2)): the two rules over the open archive, from a table kept over
/// its log lines and caught up at each call with the live table's own push, so that it equals the table from the files
/// at every commit, also after a reopen. `fmi explore run` wires it (`free::set_shift`) before the driver starts.
#[derive(Default)]
pub struct ArchivePort {
    /// the archive's root, the log lines pushed, the table
    kept: std::sync::Mutex<Option<(PathBuf, usize, Stalls)>>,
}

impl ArchivePort {
    fn with<T>(&self, a: &Archive, f: impl FnOnce(&Stalls) -> T) -> Res<T> {
        let mut g = self.kept.lock().unwrap_or_else(|e| e.into_inner());
        let fresh = !matches!(&*g, Some((root, n, _)) if *root == a.root && *n <= a.logs.len());
        if fresh {
            *g = Some((a.root.clone(), 0, Stalls::new(&a.root, &a.goal)));
        }
        let (_, n, s) = g.as_mut().unwrap();
        for l in &a.logs[*n..] {
            s.push(l, a)?;
        }
        *n = a.logs.len();
        s.set_cells(cells_of_archive(a));
        Ok(f(s))
    }

    /// The free share now: ρ0, or with the shift on max(ρ0, min(2ρ0, 0.5)).
    pub fn share(&self, a: &Archive, rho0: f64) -> Res<f64> {
        self.with(a, |s| s.share(rho0))
    }

    /// The cell rule on `cell` now.
    pub fn free_cell(&self, a: &Archive, cell: &str) -> Res<bool> {
        self.with(a, |s| s.free_cell(cell))
    }
}

/// free's port, wired by `fmi explore run` (`free::set_shift`): free asks it only with FMI_FREE_SHIFT=1.
impl crate::free::Shift for ArchivePort {
    fn share(&self, a: &Archive, rho0: f64) -> Result<f64, String> {
        ArchivePort::share(self, a, rho0)
    }
    fn free_cell(&self, a: &Archive, cell: &str) -> Result<bool, String> {
        ArchivePort::free_cell(self, a, cell)
    }
}

// --- the report --------------------------------------------------------------------------------------------------------

/// `fmi explore stalls ARCHIVE`'s text: the run's and the last 20's classes, the largest rows, the stops, the shift and
/// the cell rule.
pub fn report_text(s: &Stalls) -> String {
    let v = s.json();
    let mut o = String::new();
    let (w, r) = s.totals();
    o += &format!("# Stalls: {}\n\n{} tries, wall {:.1} s, rows {:.1} s\n\n", s.root.display(), s.tries.len(), w, r);
    for (name, key) in [("the run", "run"), ("the last 20", "last20")] {
        let p = &v[key];
        o += &format!("## {name} ({} tries, {:.1} s)\n\n| class | seconds | share |\n|---|---:|---:|\n", p["tries"], f64_of(&p["seconds"]));
        for c in p["classes"].as_array().unwrap() {
            o += &format!("| {} | {:.1} | {:.1}% |\n", c[0].as_str().unwrap(), f64_of(&c[1]), 100.0 * f64_of(&c[2]));
        }
        o += &format!("\nrest: {:.2} s ({:.2}%), absolute {:.2} s ({:.2}%)\n\n", f64_of(&p["rest"]["seconds"]), 100.0 * f64_of(&p["rest"]["share"]),
                      f64_of(&p["rest"]["abs"]), 100.0 * f64_of(&p["rest"]["abs_share"]));
    }
    o += "## Rows over the run (largest 40)\n\n| phase | outcome | class | seconds | tries |\n|---|---|---|---:|---:|\n";
    for x in v["run"]["rows"].as_array().unwrap().iter().take(40) {
        o += &format!("| {} | {} | {} | {:.1} | {} |\n", x[0].as_str().unwrap(), x[1].as_str().unwrap(), x[2].as_str().unwrap(), f64_of(&x[3]), x[4]);
    }
    let mut by_stop: BTreeMap<(String, &'static str), (usize, f64)> = BTreeMap::new();
    for t in &s.tries {
        let e = by_stop.entry((t.stop.clone(), t.class.name())).or_insert((0, 0.0));
        e.0 += 1;
        e.1 += t.wall;
    }
    o += "\n## Tries by stop and class\n\n| stop | try's class | tries | wall s |\n|---|---|---:|---:|\n";
    for ((st, c), (n, x)) in &by_stop {
        o += &format!("| {st} | {c} | {n} | {x:.1} |\n");
    }
    o += &format!("\n## Shift\n\nnow: {}\nchanges: {}\ncells the cell rule names: {}\n", v["shift"]["class"], v["shift"]["changes"], v["free_cells"]);
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runs() -> BTreeMap<String, Value> {
        BTreeMap::new()
    }

    #[test]
    fn numbers_are_written_n_but_names_keep_theirs() {
        assert_eq!(numbers_n("3600 frames"), "N frames");
        assert_eq!(numbers_n("differs on 12 keys"), "differs on N keys");
        assert_eq!(numbers_n("sentence(10,452)#2636"), "sentence(N,N)#N");
        assert_eq!(numbers_n("Lemma R's premise R0 a00 x12y"), "Lemma R's premise R0 a00 x12y");
        assert_eq!(numbers_n("-1 and 0.62"), "-N and N.N");
    }

    #[test]
    fn the_stop_table() {
        use Class::*;
        let cases = [
            ("goal", false, ModelNoYield, "goal"),
            ("goal, not from the plan: it held at a plan's start", false, ModelNoYield, "goal, not from the plan"),
            ("refinement: Lemma R's premise R0: the last pass wrote values of var [194] outside the domain", false, ModelBlind, "refinement: Lemma R's R0"),
            ("refinement: var 194 added at plan 1 is named again; x", false, ModelBlind, "refinement"),
            ("reach extension: the start at frame 3 holds 2 values the extended reading did not reach either: x", false, ModelBlind, "reach extension"),
            ("the engine's state is outside the task's domains (3 facts)", false, ModelBlind, "outside the task's domains"),
            ("var 19 below δ_lo at 5: Lemma 2's premise failed", false, ModelBlind, "Lemma 2's premise"),
            ("no plan (the planner proved none)", false, NoPlan, "no plan (the planner proved none)"),
            ("no plan (the search ended without one)", false, NoPlan, "no plan (the search ended without one)"),
            ("planner timeout (120 s)", false, NoPlan, "planner timeout"),
            ("planner memory limit (3G)", false, NoPlan, "planner memory limit"),
            ("planner: taskcore said no", false, NoPlan, "planner error"),
            ("time limit", false, NoPlan, "time limit"),
            ("the same empty window 2 times on sentence(10,452)#2636: no cause", true, Refused, "the same empty window"),
            ("the same empty window 2 times on walk_to(428)#64: x", false, Mismatch, "the same empty window"),
            ("replan limit", false, Mismatch, "replan limit"),
            ("replan at frame 9 with plan 2's start and bans and no frame played since: the same plan again", false, Mismatch, "the same plan again"),
            ("engine: engine ended: quit", false, ModelNoYield, "engine ended: quit"),
            ("engine: engine ended: crash", false, Overhead, "engine ended: crash"),
            ("engine: the bridge hung up", false, Overhead, "engine error"),
            ("error: refinement: oops", false, Overhead, "error"),
        ];
        for (s, refused, c, k) in cases {
            assert_eq!(stop_class(s, refused), c, "{s}");
            assert_eq!(stop_kind(s), k, "{s}");
        }
    }

    fn win(input: &str, end: &str, outcomes: Value) -> Value {
        json!({"input": input, "end_reason": end, "outcomes": outcomes, "wall_s": 1.0})
    }

    #[test]
    fn a_refusal_is_b_or_the_bound_with_every_try_at_the_interface() {
        assert!(refused_window(&win("x", "3600 frames", json!({"interface": 296}))));
        assert!(refused_window(&win("x", "3600 frames", json!({"interface": 3, "cancelled": 2}))));
        assert!(refused_window(&win("x", "refused input: dropped: gate", json!({}))));
        assert!(!refused_window(&win("x", "3600 frames", json!({"spent": 294, "cancelled": 2}))));
        assert!(!refused_window(&win("x", "a needed record refuted in the parent", json!({"interface": 1}))));
        assert!(!refused_window(&win("x", "3600 frames", json!({}))));
        let stop = "the same empty window 2 times on s(1)#2: no cause";
        let run = json!({"windows": [win("w#1", "ready: the first end point in the order of tries", json!({"deviation": 1})),
                                     win("s(1)#2", "ready: the first end point in the order of tries", json!({"deviation": 1})),
                                     win("s(1)#2", "3600 frames", json!({"interface": 296})),
                                     win("s(1)#2", "3600 frames", json!({"interface": 296}))]});
        assert!(empty_refused(stop, Some(&run)));
        let mixed = json!({"windows": [win("s(1)#2", "3600 frames", json!({"interface": 296})),
                                       win("s(1)#2", "a needed record refuted in the parent", json!({"refuted": 1}))]});
        assert!(!empty_refused(stop, Some(&mixed)));
        assert!(!empty_refused(stop, None));
        // a record whose name holds ": " is found by the stop's " on NAME: "
        let ev = "the same empty window 2 times on event(script:47@35)#205: refuted";
        let r2 = json!({"windows": [win("event(script:47@35)#205", "a needed record refuted in the parent", json!({}))]});
        assert!(!empty_refused(ev, Some(&r2)));
    }

    fn base(wall: f64) -> Value {
        json!({"ok": true, "return": {"same": true, "boot_s": 0.1}, "reading": {"fresh": true, "why": "no cached reading", "wall_s": 10.0, "load_s": 0.5},
               "probes": [], "attempt": null, "new": [], "wall_s": wall,
               "e012": {"t": {"seconds": 1.0}, "wait": {"end": "static", "seconds": 0.2}}})
    }

    fn sum(rows: &[(Row, Class, f64)]) -> f64 {
        rows.iter().map(|x| x.2).sum()
    }

    fn secs(rows: &[(Row, Class, f64)], c: Class) -> f64 {
        rows.iter().filter(|x| x.1 == c).map(|x| x.2).sum()
    }

    #[test]
    fn an_r0_try_with_an_affordance_step_that_kept_something_new() {
        let mut r = base(30.0);
        r["probes"] = json!([{"target": ["bit:1", 1], "status": "unsolvable", "stop": "no plan (the planner proved none)", "seconds": 2.0, "attempt": "a00"},
                             {"target": ["bit:2", 1], "status": "plan", "stop": "refinement: Lemma R's premise R0: x", "seconds": 8.0, "attempt": "a01"}]);
        r["attempt"] = json!({"stop": "refinement: Lemma R's premise R0: x", "reached": false});
        r["new"] = json!([{"how": "checkpoint"}, {"how": "end"}, {"how": "afford"}]);
        r["e012"]["afford"] = json!([{"why": "R0", "offer": {"idle": "dialog"}, "kept": 1, "step_seconds": 0.6}]);
        let mut rs = runs();
        rs.insert("a01".into(), json!({"windows": [win("walk_to(1)#2", "ready: the first end point in the order of tries", json!({"deviation": 1}))],
                                        "refinements": [{"seconds": 3.0, "added": [194]}, {"seconds": null, "added": []}], "extensions": []}));
        let new = [false, false, true];
        let a = account(&TryIn { result: &r, runs: &rs, new: &new, outcome: "ok", kind: "model" });
        assert_eq!(a.stop, "refinement: Lemma R's R0");
        assert_eq!(a.stop_class, Class::ModelBlind);
        assert!(!a.attempt_new);
        assert!((sum(&a.rows) - 30.0).abs() < 1e-9);
        // read 10.5 + T 1 + plan 2 + (8 - 1 - 3) + play 1 + refinement 3 to model-blind
        assert!((secs(&a.rows, Class::ModelBlind) - (10.5 + 1.0 + 2.0 + 4.0 + 1.0 + 3.0)).abs() < 1e-9);
        assert!((secs(&a.rows, Class::Productive) - 0.6).abs() < 1e-9);
        assert!((secs(&a.rows, Class::Wait) - 0.2).abs() < 1e-9);
        let rest = a.rows.iter().find(|x| x.0.phase == "rest").unwrap();
        assert!((rest.2 - (30.0 - 0.1 - 10.5 - 1.0 - 0.2 - 2.0 - 8.0 - 0.6)).abs() < 1e-9);
        assert!(a.rows.iter().any(|x| x.0 == row("afford", "R0, dialog")));
        assert!(a.rows.iter().any(|x| x.0 == row("plan", "unsolvable")));
        assert!(a.rows.iter().any(|x| x.0 == row("play", "ready: the first end point in the order of tries; deviation")));
    }

    #[test]
    fn the_attempts_entries_make_its_model_seconds_productive() {
        let mut r = base(20.0);
        r["probes"] = json!([{"status": "plan", "stop": "goal", "seconds": 5.0, "attempt": "a00"}]);
        r["attempt"] = json!({"stop": "goal", "reached": true});
        r["new"] = json!([{"how": "checkpoint"}, {"how": "wait"}]);
        let a = account(&TryIn { result: &r, runs: &runs(), new: &[true, false], outcome: "ok", kind: "model" });
        assert!(a.attempt_new);
        assert_eq!(a.stop_class, Class::ModelNoYield);
        assert!((secs(&a.rows, Class::Productive) - (10.5 + 1.0 + 5.0)).abs() < 1e-9);
        assert!((secs(&a.rows, Class::Wait) - 0.2).abs() < 1e-9);
        // nothing new: the reached target is "model, no yield"; a new wait stop makes the wait productive
        let b = account(&TryIn { result: &r, runs: &runs(), new: &[false, true], outcome: "ok", kind: "model" });
        assert!((secs(&b.rows, Class::ModelNoYield) - 16.5).abs() < 1e-9);
        assert!((secs(&b.rows, Class::Productive) - 0.2).abs() < 1e-9);
    }

    #[test]
    fn the_same_empty_window_refused_or_not() {
        let mut r = base(40.0);
        let stop = "the same empty window 2 times on sentence(10,452)#2636: no cause";
        r["probes"] = json!([{"status": "plan", "stop": stop, "seconds": 9.0, "attempt": "a00"}]);
        r["attempt"] = json!({"stop": stop});
        r["e012"]["afford"] = json!([{"why": "empty window", "offer": {"idle": "sentence"}, "kept": 0, "step_seconds": 20.0}]);
        let mut rs = runs();
        rs.insert("a00".into(), json!({"windows": [win("sentence(10,452)#2636", "3600 frames", json!({"interface": 296})),
                                                    win("sentence(10,452)#2636", "3600 frames", json!({"interface": 296}))]}));
        let a = account(&TryIn { result: &r, runs: &rs, new: &[], outcome: "ok", kind: "model" });
        assert_eq!(a.stop_class, Class::Refused);
        assert!((secs(&a.rows, Class::InterfaceNoYield) - 20.0).abs() < 1e-9);
        assert!(a.rows.iter().any(|x| x.0 == row("play", "N frames; interface") && x.1 == Class::Refused));
        let mut rs2 = runs();
        rs2.insert("a00".into(), json!({"windows": [win("sentence(10,452)#2636", "3600 frames", json!({"spent": 294, "cancelled": 2}))]}));
        assert_eq!(account(&TryIn { result: &r, runs: &rs2, new: &[], outcome: "ok", kind: "model" }).stop_class, Class::Mismatch);
    }

    #[test]
    fn none_played() {
        let mut r = base(10.0);
        let a = account(&TryIn { result: &r, runs: &runs(), new: &[], outcome: "ok", kind: "model" });
        assert_eq!((a.stop.as_str(), a.stop_class), ("none played: no target", Class::NoPlan));
        r["probes"] = json!([{"status": null, "stop": "reach extension: the start at frame 1 holds 1 values ...", "seconds": 1.0, "attempt": "a00"},
                             {"status": null, "stop": "reach extension: the start at frame 1 holds 1 values ...", "seconds": 1.0, "attempt": "a01"}]);
        let a = account(&TryIn { result: &r, runs: &runs(), new: &[], outcome: "ok", kind: "model" });
        assert_eq!((a.stop.as_str(), a.stop_class), ("none played: reach extension", Class::ModelBlind));
        assert!(a.rows.iter().any(|x| x.0 == row("plan", "reach extension")));
        r["probes"][1] = json!({"status": "time", "stop": "planner timeout (120 s)", "seconds": 120.0, "attempt": "a01"});
        let a = account(&TryIn { result: &r, runs: &runs(), new: &[], outcome: "ok", kind: "model" });
        assert_eq!((a.stop.as_str(), a.stop_class), ("none played: no plan", Class::NoPlan));
    }

    #[test]
    fn errors_and_a_return_that_differs_are_overhead() {
        let r = json!({"ok": false, "return": {"same": false, "boot_s": 0.2}, "wall_s": 0.5, "probes": [], "new": []});
        let a = account(&TryIn { result: &r, runs: &runs(), new: &[], outcome: "return differs", kind: "model" });
        assert_eq!(a.stop_class, Class::Overhead);
        assert!((secs(&a.rows, Class::Overhead) - 0.5).abs() < 1e-9);
        assert!(a.rows.iter().any(|x| x.0 == row("return", "differs")));
        let e = json!({"ok": false, "wall_s": 0.0});
        let a = account(&TryIn { result: &e, runs: &runs(), new: &[], outcome: "error", kind: "model" });
        assert_eq!((a.stop.as_str(), a.stop_class), ("error", Class::Overhead));
    }

    #[test]
    fn verdicts_split_a_step_and_refusals_are_refused_input() {
        let mut r = base(30.0);
        r["e012"]["afford"] = json!([{"why": "no plan", "kept": 1, "step_seconds": 12.0,
                                      "verdict_s": {"dropped: gate": 1.0, "ignored": 3.0, "taken": 7.0}},
                                     {"why": "R0", "kept": 0, "step_seconds": 1.0}]);
        r["new"] = json!([{"how": "afford"}]);
        let a = account(&TryIn { result: &r, runs: &runs(), new: &[true], outcome: "ok", kind: "model" });
        assert!((secs(&a.rows, Class::Refused) - 4.0).abs() < 1e-9);
        // the first step kept the new entry: its taken trials and other seconds are productive; the second kept none
        assert!((secs(&a.rows, Class::Productive) - 8.0).abs() < 1e-9);
        assert!(a.rows.iter().any(|x| x.0 == row("afford", "no plan: other") && (x.2 - 1.0).abs() < 1e-9));
        assert!(a.rows.iter().any(|x| x.0 == row("afford", "R0") && x.1 == Class::InterfaceNoYield));
        assert!((sum(&a.rows) - 30.0).abs() < 1e-9);
    }

    #[test]
    fn a_free_try() {
        let mut r = base(25.0);
        r["e012"]["afford"] = Value::Null;
        // free's record (e013-free ea0e920): the offer points, the delays' seconds, the whole step's seconds
        r["free"] = json!({"offers": 2, "n_points": 3, "trials": 40, "kept": {"1": 1}, "delays": {"counted": 3, "seconds": 2.0},
                           "seconds": 15.0,
                           "points": [{"where": "entry", "frame": 300, "level": 1, "seconds": 8.0, "record": {"verdict_s": {"ignored": 2.0, "taken": 5.0}},
                                       "delays": {"seconds": 2.0}},
                                      {"where": "stop", "frame": 4310, "level": 1, "seconds": 3.0, "record": {"ready": true}},
                                      {"where": "kept", "frame": 320, "level": 2, "seconds": 1.0, "record": {"ready": false}}]});
        r["new"] = json!([{"how": "wait"}, {"how": "free"}]);
        let a = account(&TryIn { result: &r, runs: &runs(), new: &[false, false], outcome: "ok", kind: "free" });
        assert!((sum(&a.rows) - 25.0).abs() < 1e-9);
        assert!((secs(&a.rows, Class::Refused) - 2.0).abs() < 1e-9);
        assert!(a.rows.iter().any(|x| x.0 == row("free", "entry, level 1: taken") && (x.2 - 5.0).abs() < 1e-9));
        assert!(a.rows.iter().any(|x| x.0 == row("free", "entry, level 1: other") && (x.2 - 1.0).abs() < 1e-9));
        assert!(a.rows.iter().any(|x| x.0 == row("free", "stop, level 1") && (x.2 - 3.0).abs() < 1e-9));
        assert!(a.rows.iter().any(|x| x.0 == row("free", "kept, level 2") && (x.2 - 1.0).abs() < 1e-9));
        assert!(a.rows.iter().any(|x| x.0 == row("free", "delays") && (x.2 - 2.0).abs() < 1e-9));
        assert!(a.rows.iter().any(|x| x.0 == row("free", "other") && (x.2 - 1.0).abs() < 1e-9));
        // nothing new: read and T are "interface, no yield", and so is the try's class (the most seconds)
        assert_eq!(a.stop_class, Class::InterfaceNoYield);
        assert!((secs(&a.rows, Class::InterfaceNoYield) - (10.5 + 1.0 + 5.0 + 1.0 + 3.0 + 1.0 + 2.0 + 1.0)).abs() < 1e-9);
        let b = account(&TryIn { result: &r, runs: &runs(), new: &[false, true], outcome: "ok", kind: "free" });
        assert!((secs(&b.rows, Class::Productive) - (10.5 + 1.0 + 5.0 + 1.0 + 3.0 + 1.0 + 2.0 + 1.0)).abs() < 1e-9);
        // a record with no points: the step's seconds in "other"
        r["free"] = json!({"seconds": 4.0, "n_points": 0, "points": []});
        let c = account(&TryIn { result: &r, runs: &runs(), new: &[false, false], outcome: "ok", kind: "free" });
        assert!(c.rows.iter().any(|x| x.0 == row("free", "other") && (x.2 - 4.0).abs() < 1e-9));
        assert!((sum(&c.rows) - 25.0).abs() < 1e-9);
    }

    #[test]
    fn a_bounded_try() {
        // E014 amendment (4) (a): the reading stopped at the bound after 300 s, a stand-in loaded in 0.5 s, then the
        // free step from the entry
        let mut r = base(320.0);
        r["reading"] = json!({"path": "/x/reading.json", "fresh": false, "why": "bound: no cached reading", "load_s": 0.5,
                              "bound": {"s": 300.0, "waited_s": 300.0, "background": true, "at_once": false, "joined": false}});
        r["e012"]["afford"] = Value::Null;
        r["free"] = json!({"seconds": 15.0, "n_points": 1,
                           "points": [{"where": "entry", "frame": 300, "level": 1, "seconds": 15.0, "record": {"ready": true}}]});
        r["new"] = json!([{"how": "free"}]);
        let a = account(&TryIn { result: &r, runs: &runs(), new: &[false], outcome: "ok", kind: "model" });
        assert!((sum(&a.rows) - 320.0).abs() < 1e-9);
        assert_eq!((a.stop.as_str(), a.stop_class), ("reading bound", Class::ReadBound));
        assert!((secs(&a.rows, Class::ReadBound) - 300.0).abs() < 1e-9);
        assert!(a.rows.iter().any(|x| x.0 == row("read", "stand-in") && x.1 == Class::InterfaceNoYield && (x.2 - 0.5).abs() < 1e-9));
        assert!(a.rows.iter().any(|x| x.0 == row("free", "entry, level 1") && x.1 == Class::InterfaceNoYield));
        // its free step kept something new: the step's rows are productive, the bound's stay "reading bound"
        let b = account(&TryIn { result: &r, runs: &runs(), new: &[true], outcome: "ok", kind: "model" });
        assert!((secs(&b.rows, Class::ReadBound) - 300.0).abs() < 1e-9);
        assert!(b.rows.iter().any(|x| x.0 == row("free", "entry, level 1") && x.1 == Class::Productive));
        // no stand-in: the bound and the rest
        let mut c = base(301.0);
        c["reading"] = json!({"path": null, "fresh": false, "why": "bound: x; no reading stands in", "load_s": 0.0,
                              "bound": {"s": 300.0, "waited_s": 300.0}});
        c["e012"] = json!({});
        let k = account(&TryIn { result: &c, runs: &runs(), new: &[], outcome: "ok", kind: "model" });
        assert!(k.rows.iter().any(|x| x.0 == row("read", "no stand-in")));
        assert!((sum(&k.rows) - 301.0).abs() < 1e-9);
        // the reading again after "not a plan" reached the bound: its own row
        let mut d = base(400.0);
        d["reading"]["retry_bound"] = json!({"s": 300.0, "waited_s": 300.0});
        let m = account(&TryIn { result: &d, runs: &runs(), new: &[], outcome: "ok", kind: "model" });
        assert!(m.rows.iter().any(|x| x.0 == row("read", "bound: the reading again") && x.1 == Class::ReadBound));
        // no bound record: no row of the class
        let n = account(&TryIn { result: &base(20.0), runs: &runs(), new: &[], outcome: "ok", kind: "model" });
        assert_eq!(secs(&n.rows, Class::ReadBound), 0.0);
        assert_eq!(Class::of_name("reading bound"), Some(Class::ReadBound));
    }

    #[test]
    fn e014_end_entries_go_to_the_part_whose_dead_move_the_end_made() {
        let item = |how: &str, frame: u64, novel: bool| {
            let mut v = json!({"file": "new/0.json.gz", "frame": frame, "tick": frame, "how": how});
            if novel {
                v["novel"] = json!(["end"]);
            }
            v
        };
        let dead = [json!({"reason": "restart@57:0D0E", "frame": 900, "events": [], "how": "wait"}),
                    json!({"reason": "restart@582:0067", "frame": 950, "events": [], "how": "afford"}),
                    json!({"reason": "quit", "frame": 990, "events": [], "how": "attempt"}),
                    json!({"reason": "crash", "frame": 999, "events": [], "how": "wait"}),
                    json!({"reason": "restart@57:0D0E", "frame": 960, "events": [], "how": "free"})];
        assert_eq!(part_of_item(&item("end", 900, true), &dead), "wait");
        assert_eq!(part_of_item(&item("end", 950, true), &dead), AFFORD_END);
        assert_eq!(part_of_item(&item("end", 990, true), &dead), "end");
        // a free trial's end (amendment (3)): the free step's
        assert_eq!(part_of_item(&item("end", 960, true), &dead), "free");
        // not a game end, or no dead move: the attempt's
        assert_eq!(part_of_item(&item("end", 999, true), &dead), "end");
        assert_eq!(part_of_item(&item("end", 700, true), &dead), "end");
        // the attempt's own last checkpoint (no "novel") and the other hows are their own
        assert_eq!(part_of_item(&item("end", 900, false), &dead), "end");
        assert_eq!(part_of_item(&item("wait", 900, true), &dead), "wait");
        assert_eq!(part_of_item(&json!({"frame": 1}), &dead), "checkpoint");
        // the steps' items: the c-th afford item and the end entries after c afford items
        assert_eq!(step_of(&[2, 1], 1), Some(0));
        assert_eq!(step_of(&[2, 1], 2), Some(0));
        assert_eq!(step_of(&[2, 1], 3), Some(1));
        assert_eq!(step_of(&[0], 0), Some(0));
        assert_eq!(step_of(&[2], 5), Some(0));
        assert_eq!(step_of(&[], 0), None);
        // in a try: a new end entry from the wait makes the wait productive, not the attempt's seconds
        let mut r = base(30.0);
        r["probes"] = json!([{"status": "plan", "stop": "goal", "seconds": 5.0, "attempt": "a00"}]);
        r["attempt"] = json!({"stop": "goal", "reached": true});
        r["e012"]["afford"] = json!([{"why": "R0", "kept": 1, "step_seconds": 3.0}]);
        r["new"] = json!([item("end", 800, false), item("wait", 850, true), item("end", 900, true), item("afford", 940, true),
                          item("end", 950, true)]);
        r["dead"] = json!(dead);
        let a = account(&TryIn { result: &r, runs: &runs(), new: &[false, false, true, false, false], outcome: "ok", kind: "model" });
        assert!(!a.attempt_new);
        assert!((secs(&a.rows, Class::Productive) - 0.2).abs() < 1e-9);
        assert!((secs(&a.rows, Class::ModelNoYield) - 16.5).abs() < 1e-9);
        assert!((secs(&a.rows, Class::InterfaceNoYield) - 3.0).abs() < 1e-9);
        // a new end entry from the affordance step makes the step productive
        let b = account(&TryIn { result: &r, runs: &runs(), new: &[false, false, false, false, true], outcome: "ok", kind: "model" });
        assert!((secs(&b.rows, Class::Productive) - 3.0).abs() < 1e-9);
        assert!((secs(&b.rows, Class::Wait) - 0.2).abs() < 1e-9);
        // the attempt's end entry is the attempt's
        r["dead"][2]["frame"] = json!(800);
        r["new"][0] = item("end", 800, true);
        let c = account(&TryIn { result: &r, runs: &runs(), new: &[true, false, false, false, false], outcome: "ok", kind: "model" });
        assert!(c.attempt_new);
        // a stop at a script's end names the end without its site
        assert_eq!(stop_kind("engine: engine ended: restart@57:0D0E"), "engine ended: restart");
        assert_eq!(stop_class("engine: engine ended: restart@57:0D0E", false), Class::ModelNoYield);
    }

    fn tstall(job: &str, entry: usize, kind: &str, gave: bool, stop_class: Class, by: &[(Class, f64)]) -> TryStall {
        TryStall {
            job: job.into(),
            entry,
            kind: kind.into(),
            outcome: "ok".into(),
            wall: by.iter().map(|x| x.1).sum(),
            stop: "x".into(),
            stop_class,
            class: if gave { Class::Productive } else { stop_class },
            gave,
            rows: by.iter().map(|(c, x)| (row("p", c.name()), *c, *x)).collect(),
            novelty: Some(true),
        }
    }

    #[test]
    fn the_shift_threshold_and_its_hysteresis() {
        use Class::*;
        let mut s = Stalls::default();
        // 1: half the seconds model-blind -> on
        s.tries.push(tstall("j0", 0, "model", false, ModelBlind, &[(ModelBlind, 5.0), (Productive, 5.0)]));
        assert_eq!(s.shift(), Shift { on: true, class: Some(ModelBlind) });
        assert!((s.share(0.25) - 0.5).abs() < 1e-12);
        assert!((s.share(0.4) - 0.5).abs() < 1e-12);
        assert!((s.share(1.0) - 1.0).abs() < 1e-12);
        // 2: model-blind at 5/20 = 25% < 30% -> off
        s.tries.push(tstall("j1", 0, "model", true, NoPlan, &[(Productive, 10.0)]));
        assert_eq!(s.shift(), Shift { on: false, class: None });
        assert!((s.share(0.25) - 0.25).abs() < 1e-12);
        // 3: model-blind 5 + 9 = 14 of 29 = 48% -> stays off (below 50%)
        s.tries.push(tstall("j2", 0, "model", false, ModelBlind, &[(ModelBlind, 9.0)]));
        assert!(!s.shift().on);
        // 4: 14 + 1 of 30 = 50% -> on again
        s.tries.push(tstall("j3", 0, "model", false, ModelBlind, &[(ModelBlind, 1.0)]));
        assert_eq!(s.shift(), Shift { on: true, class: Some(ModelBlind) });
        // 5: 15 of 45 = 33% -> stays on (not below 30%)
        s.tries.push(tstall("j4", 0, "model", true, NoPlan, &[(Productive, 15.0)]));
        assert!(s.shift().on);
        // 6: 15 of 51 = 29% -> off
        s.tries.push(tstall("j5", 0, "model", true, NoPlan, &[(Productive, 6.0)]));
        assert!(!s.shift().on);
        // the window is the last 20: 20 productive tries push the old ones out
        for k in 0..20 {
            s.tries.push(tstall(&format!("p{k}"), 0, "model", true, NoPlan, &[(Productive, 1.0)]));
        }
        let (fr, st) = *s.shift_trace().last().unwrap();
        assert!(!st.on);
        assert!((fr[Productive.ix()] - 1.0).abs() < 1e-12);
        // off, then another class at 50% in the same commit that turns the first one off
        let mut t = Stalls::default();
        t.tries.push(tstall("a", 0, "model", false, ModelBlind, &[(ModelBlind, 6.0)]));
        t.tries.push(tstall("b", 0, "model", false, Refused, &[(Refused, 20.0)]));
        // model-blind 6/26 = 23% -> off; refused 77% -> on for refused in the same commit
        assert_eq!(t.shift(), Shift { on: true, class: Some(Refused) });
        // productive is never the shift's class
        let mut u = Stalls::default();
        u.tries.push(tstall("a", 0, "model", true, NoPlan, &[(Productive, 6.0)]));
        assert!(!u.shift().on);
    }

    #[test]
    fn the_cell_rule() {
        use Class::*;
        let mut s = Stalls::default();
        s.cells = [(0usize, "c0".to_string()), (1, "c1".to_string()), (2, "c0".to_string())].into_iter().collect();
        s.tries.push(tstall("j0", 0, "model", false, ModelBlind, &[(ModelBlind, 1.0)]));
        assert!(!s.free_cell("c0"));
        s.tries.push(tstall("j1", 1, "model", false, ModelBlind, &[(ModelBlind, 1.0)]));
        assert!(!s.free_cell("c0") && !s.free_cell("c1"));
        // two in c0 (entries 0 and 2), same class -> named
        s.tries.push(tstall("j2", 2, "model", false, ModelBlind, &[(ModelBlind, 1.0)]));
        assert!(s.free_cell("c0"));
        assert_eq!(s.free_cells(), vec!["c0".to_string()]);
        // a free try in the cell clears it
        s.tries.push(tstall("j3", 0, "free", false, InterfaceNoYield, &[(InterfaceNoYield, 1.0)]));
        assert!(!s.free_cell("c0"));
        // two model tries again, different classes -> not named; same class -> named
        s.tries.push(tstall("j4", 0, "model", false, NoPlan, &[(NoPlan, 1.0)]));
        s.tries.push(tstall("j5", 2, "model", false, Refused, &[(Refused, 1.0)]));
        assert!(!s.free_cell("c0"));
        s.tries.push(tstall("j6", 0, "model", false, Refused, &[(Refused, 1.0)]));
        assert!(s.free_cell("c0"));
        // a productive try is never one of the two
        s.tries.push(tstall("j7", 2, "model", true, Refused, &[(Productive, 1.0)]));
        assert!(!s.free_cell("c0"));
        // the cells move with T: entry 2 now in c1 -> c0's last two are j4? no: j3 (free), j4, j6 -> j4 no plan, j6 refused
        s.cells.insert(2, "c1".into());
        assert!(!s.free_cell("c0"));
    }
}
