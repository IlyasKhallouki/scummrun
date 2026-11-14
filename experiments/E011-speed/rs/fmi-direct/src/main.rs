//! direct: a search in the engine itself (a spike, not yet an experiment's component).
//!
//! From the boot's first idle stop, every input the interface offers at a stop is tried in a fork of the engine, and a
//! stop that holds something new is kept and expanded in turn. The engine is the model: the scripts are read only for
//! the guide's keys (which variables the game's code tests), never for what an input does.
//!
//!   direct OUT [--minutes M] [--stops N] [--workers W] [--slices S] [--at-once A] [--horizon H] [--no-skips]
//!              [--guide FILE] [--rungs FILE] [--watch KEY=V,..] [--lines gains] [--flag-lines] [--shares] [--go]
//!              [--motion-once]
//!              [--cross-pages]
//!              [--from NODES ID | --from-run DIR] [--edges FILE] [--news KEY,..] [--ever OBJ,..]
//!   direct verify NODES.jsonl ID OUT [KEY=V,..]      (a kept stop replayed from boot in a fresh engine)
//!   direct show | seq | forkbench ...                (measurement tools)
//!
//! * **Inputs** are click sequences, found breadth first at a stop (`clicks`). The clickable points are afford's offer
//!   (the visible verb slots, the objects and actors the hit test answers) and a grid of floor points. A click that
//!   leaves the game composing a sentence (afford's `composing`: only the input script ran, and ego stands where it
//!   stood) gives a stop to click on from, unless an earlier sequence showed the same interface over the same world;
//!   any other click ends its sequence. At most MAX_CLICKS clicks.
//! * **A trial** is a fork, the click, the pointer parked on a point that is no verb, object or actor (the sentence
//!   line names what the pointer is over), then a run to the next stop (`calm`), within the horizon, or four times
//!   it. A stop is where ego stands still and the bridge is idle, or, BUSY frames after the inputs, where the game
//!   takes a click all the same (user input on, no cutscene): a scene whose actors talk on has no idle frame.
//! * **The game's own moves.** Each stop is also run with no input for WAITS frames. Where that changes a key of the
//!   world that fewer than MOTION_TRIES stops of that level, room and walk box saw change, the inputs are tried again
//!   from there (a door a timer opens is open only then, and ego must stand near it; a room's own motion is tried a
//!   few times from each part of the room, not at every stop).
//! * **A click that changes only what the interface shows** (the bridge idle again LEAD frames on and still so CALM
//!   frames later, ego where it stood, the world the same, and every running script where it is in a fork given no
//!   input: the same place, delay and locals) is no stop: it is clicked on from. A dialog's line is none: it clears
//!   the choices at once, and its script goes on. Where it changed one
//!   slot it is the start or the middle of a sentence, a stop to click on from like a half-made one. Where it
//!   changed several (another page of the interface) and came from the round's stop, it is clicked on from at the
//!   same depth: at the slots it changed, at the slots that start a sentence (and from that sentence at the slots
//!   it changed), and at the slots that change the page again. Inside a sentence the page is not changed (with
//!   `--cross-pages` it is, once the sentence has its first object: for a second object on another page). What a
//!   slot shows is the engine's to say: a slot that shows an image has the image's hash for its name (engine
//!   svm-e027, patch 0011), so a page of images is a page; and after a page a slot that shows an image some slot
//!   showed on a page before is the same thing in another place, and is not clicked. A slot's click after which
//!   the interface is another, ego stands where it stood and the world is the no-input twin's at the same frame
//!   is a sentence half made too, whatever scripts ran (an object's own script runs for its slot).
//!   A click that left another picture and no more, but took longer than LEAD frames to, is read where the game
//!   has settled and set against the twin at that frame (the room, ego's place, the scripts that take the clicks
//!   and the world the same). With several slots changed and every running script as the twin's, locals and all,
//!   it is a page like one that showed at once. With one slot changed and every script at the twin's place (a
//!   local may hold what was picked) it is a sentence half made; where that slot shows what the round's stop did
//!   not, the sentence waits for more and is clicked on from. Neither is a stop for its interface or its cell
//!   alone (`pic`): as a stop of its own it would be the round's clicks made again. A dialog's line is neither:
//!   its script is at another place.
//!   And the slots such clicks change (a sentence's line, a page's slots) are the interface's own state: a
//!   stop's identity for what is new is its other slots and the scripts that take the clicks, so a room entered
//!   with the inventory on another page, or a half-made stop's picture seen again, is nothing new.
//! * **A round** is all that from one stop. It is one list of half-made stops, each clicked on from once (at most
//!   MAX_MIDS of them, counted apart for those a page led to: a guard for a room that leaves one for every click).
//!   A half-made stop's clicks read the round's lists and write none; what the round decides (which stops are
//!   clicked on from, which windows are followed, what the slots teach) is applied in the round's order. So the
//!   stops that no stop before them can change, and a single stop's clicks, run on `--slices` threads, and a round
//!   is the same with any number of them. A trial's result keeps what differs from the expanded stop, not its facts.
//! * **After an input.** A trial's stop is set against a twin: a fork of the expanded stop run to the same frame
//!   with no input. Where they differ in a key of the world, the input changed it, and the trial is run PROBE frames
//!   on with no input. The stop reached there is one more stop to keep (an effect that lands after the first stop: a
//!   count the game raises when its actor has landed). And where a key the input changed is back at its old value
//!   there, or PROBE_LONG frames on, the first stop is a window that closes by itself: the inputs are tried from it,
//!   whether it holds anything new or not (the first MAX_WINDOWS windows of a stop). A key the stop's own waits
//!   moved is not counted as changed by an input: it moves by itself from that stop. Nor is a key of the game's own
//!   motion: one the waits moved at some stop where no input's change of it was pending (a stop carries the keys the
//!   inputs on its path changed, each until it is back at its old value: PENDING of them at most; the stops inside
//!   a window and after a wait teach nothing). Such motion runs another way after any input, and is no effect of
//!   it. And a window is followed only
//!   while fewer than WINDOW_TRIES windows on one of its keys were followed from the same cell (its room, its
//!   interface and its world without the keys that move by themselves): where a room's own motion differs after
//!   every input, the same window is not entered at every stop.
//! * **The world** of a stop is its room, owners, states, classes and bits, and the guide's variables but the
//!   interface's own (those the input script writes, and those a fifth of a stop's own inputs change where they change
//!   nothing else in the world), the clocks (those
//!   that take two values or more over one stop's waits), and those seen with more than VAR_CAP values.
//! * **Lines and levels.** A stop's line is what ego has gained and lost on the way to it: the objects it has owned
//!   (and did not own at the boot) and those of them it owns no more. Its level is their count. Something is new to
//!   a stop when no kept stop of its line holds it: with an object gained or used up, every room, input and position
//!   counts again. The highest level is expanded first. With `--lines gains` a line is the gains alone and the
//!   level their count: a loss starts no line (what it changed is new in its line, the rooms are not).
//! * **Kept.** Within its level a stop goes to the first queue it fits: (0) a world atom other than a variable's
//!   that is new; (1) an interface new in its line, or a thing on the screen new in that room; (2) a variable's value that is new, or
//!   a walk box or camera bucket new in that room, or a new value of a local variable of a script running at the
//!   stop (a script that waits for the player keeps there what the facts do not show: how often a line was said;
//!   those a wait moves, and those seen with more than LOCAL_CAP values, do not count); (3) an atom of the guide's keys new in that room; (4) any other
//!   world atom new in that room, or a new world (its atoms together, without those that change by themselves). A
//!   stop that fits none is dropped. Queue 0 is expanded first, then 1, and so on; queue 4 waits until no
//!   level holds a stop in queues 0 to 3 (queue 3 carries a flag from room to room, which nothing else does), and
//!   gives its stop with the most flags set first.
//! * **Flags.** A flag is a bit, or a class of an object. A stop whose set of flags (those not seen unset again
//!   somewhere) is held whole by no kept stop of
//!   its line is new, whatever else it holds, and goes before every queue of its level, the last such stop first.
//!   Flags are won one at a time and on different paths; the stop that has won the most is the one to go on from,
//!   and once one stop holds them all the others' sets are within its own. With `--flag-lines` such a stop also
//!   starts a line of its own, as a gain does (every room and input counts again from it), and every stop of
//!   queue 0 goes on the stack. The newest line's stops go before any older line's: its stack's newest stop
//!   first, then its queues 1 to 3 in their own order. The search works on its newest line, walks through it,
//!   then goes through what it saw there, and only then turns to the line before. With `--shares` the lines take
//!   turns instead: the highest level's newest line every second turn, the next every fourth, and so on (a line
//!   entered on the way, with much in it, had every turn otherwise); and a stop at the end of a walk the search
//!   made has its turn before any line's, and so has a stop made inside a window that is new by a guide
//!   variable's value (a count the game keeps of what was done while the window was open is followed at once).
//! * **Carried while it lasts.** An object ego owns at a stop that one of the stop's own waits saw gone is held
//!   only for a while (what the stop's waits show, not what led to the stop: a key the game was seen to move by
//!   itself is no input's doing for the rest of a run). A result that still holds it in another room, where no
//!   stop of its line of gains was seen to hold it, is new (as atoms it is not: the object owned in that room and
//!   its flag set there were each seen). So is, in the stop's room, an input that changed a key of that object and
//!   set a flag or the owner of another (it passed on what the first holds), once for a room and the keys set.
//!   Such stops are expanded before all others, the earliest first. And with `--go` the stop walks at once to
//!   the nearest GO_ROOMS rooms, one trial a walk, and keeps a walk's stop that still holds the object: a walk
//!   crosses a room wider than the screen. Flags that go off by themselves are not counted so: there are many.
//! * **Counting in place.** A stop of a round holds one value of a count (the turns of a lock, a price asked);
//!   as a stop of its own it is new by a variable's value and waits its turn, and the next value is another such
//!   wait. So in a stop's expansion, after its rounds: of the sentences of its own round, in its room, that moved a
//!   guide variable of the world (not the clicks that left another picture and no more; a variable those sentences
//!   left at more than COUNT_FEW values is the sentence's own), a count is a variable whose shortest movers (the
//!   sentences of the fewest clicks) are at most COUNT_FEW, or did at most COUNT_FEW different things. Each of
//!   the shortest movers is an input where they are few (two that do the same at this stop may differ at the
//!   next), and counts with the variables it is a shortest mover of. The inputs that move the same counts
//!   are made again from where they led, breadth first over the states of what they moved, each state once
//!   (COUNT_STATES states, COUNT_INPUTS inputs, COUNT_GROUPS such groups at most): one trial an input, read PROBE
//!   frames after its stop. A stop where something beside those keys changed is a result like any other.
//! * **Where to go (`--go`).** A stop that begins a line has won something: an object, a flag. The scripts name
//!   it somewhere (an owner asked for, the other object of a sentence, a flag tested): from that stop one walk is
//!   made to each of the nearest GO_ROOMS rooms whose scripts do, over the clicks that were seen to lead from room
//!   to room, one trial a walk. Those stops are expanded first. The scripts advise only: a walk that fails, or a
//!   room that holds nothing, costs a trial or a stop, and the walk through every room stays. A way is a click
//!   seen to lead from a room to another and not to where the stop comes with no input; where none was seen, an
//!   object whose own code leads ego to the other room. A way whose clicks did not lead on is forgotten until it
//!   is seen again, and the walk leaves the stop where it stands: what the way asks for is asked there.
//! * **Order.** Stops are taken from the queues a batch at a time (`--at-once`) and expanded side by side, each
//!   from the search's state at the batch's start; their results are committed in the batch's order. A run is the
//!   same every time. An expanded stop's engine is kept (the one used longest ago goes first); a stop's engine is
//!   a fork of the nearest kept one on its way with the events since, and a boot from its movie where none is.
//!
//! OUT/nodes.jsonl holds one line per kept stop: the stop it came from and its own trial's events (its movie from
//! boot is theirs one after the other, down from the first line); rung-stops.jsonl the whole line of each stop that
//! first held a rung or a watched value; top.jsonl the newest stop that began a line at the top level (`--from-run`
//! starts there); edges.jsonl the ways; offers.jsonl one line per expanded stop. The rungs file is the evaluation's:
//! the search never reads it for a choice.

use fmi_core::*;
use fmi_engine::{Host, HostCfg};
use fmi_explore::afford::{self, Hit, Point};
use serde_json::{json, Value};
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The longest click sequence of one input (the inventory scrolled, a verb and two objects).
const MAX_CLICKS: usize = 4;
/// Frames a stop is held before it is read (`calm`).
const CALM: u64 = 6;
/// Frames a trial's inputs take: the click, then the pointer's move.
const LEAD: u64 = 3;
/// Frames after a trial's inputs from which a stop need not be idle (the module's text, "A trial").
const BUSY: u64 = 300;
/// The game's own moves: frames run with no input, each one trial of a stop.
const WAITS: [u64; 3] = [50, 200, 800];
/// Frames a trial that changed the world is run on with no input (the module's text, "After an input"), and the
/// windows one round follows.
const PROBE: u64 = 60;
const PROBE_LONG: u64 = 240;
const MAX_WINDOWS: usize = 3;
const WINDOW_TRIES: u32 = 3;
/// The keys a stop carries as changed by the inputs on its path and not yet back (the module's text, "After an input").
const PENDING: usize = 16;
/// Stops of a level, room and walk box whose waits a key may change before its change stops calling for the inputs
/// again.
const MOTION_TRIES: u32 = 3;
/// Pixels of the camera's bucket: a fifth of the screen, so a walk across part of it shows.
const CAM_BUCKET: i32 = 64;
/// The floor clicks' grid over the main screen: the columns, and the rows in eighths of its height.
const FLOOR_X: [i32; 7] = [6, 58, 110, 160, 210, 262, 313];
const FLOOR_Y: [i32; 4] = [3, 5, 7, 8];
/// Engines kept alive, at least: those of the stops expanded last. A stop's engine is made when its turn comes, as
/// a fork of the nearest stop on its way whose engine is kept, with the events since; from the boot where none is.
const LIVE_MAX: usize = 64;
/// The queues (the module's text, "Kept").
const QUEUES: usize = 5;
/// The queues expanded at every level, the highest first, before any level's later ones.
const FIRST_QUEUES: usize = 4;
/// `--go`: the rooms walked to from a stop that has won something, and the rooms a walk may cross.
const GO_ROOMS: usize = 6;
/// Counting in place: a variable that at most COUNT_FEW of a round's inputs moved is a count; the inputs that moved
/// the same counts (COUNT_INPUTS of them at most, COUNT_GROUPS such groups) are made again from where they led,
/// until COUNT_STATES states of what they moved were seen.
const COUNT_FEW: usize = 4;
const COUNT_INPUTS: usize = 4;
const COUNT_GROUPS: usize = 3;
const COUNT_STATES: usize = 256;
/// `--go`: the values of one group of counts from which a walk is made to each of the rooms whose scripts read them
const COUNT_WALKS: usize = 8;
const GO_HOPS: usize = 14;
/// Floor clicks a walk may make in one room to bring a thing onto the screen.
const GO_SCROLLS: usize = 8;
/// Half-made stops clicked on from in one round, at most: where a room makes one of every verb and thing (it drops
/// the sentence and shows it), the round stays a round. The shallow ones come first. Those that come of a page of
/// the interface are counted apart, so the stops of the first page cannot use up the count before a later page's
/// are made (a late stop of Part I has about 120 with three pages).
const MAX_MIDS: usize = 300;
/// Stops of one batch for each worker.
const BATCH: usize = 3;
/// A variable seen with more values than this in kept stops counts no more.
const VAR_CAP: usize = 8;
/// A local variable of a running script seen with more values than this counts no more.
const LOCAL_CAP: usize = 5;

struct Node {
    /// the events of the trial that made it (the first stop's: the whole movie it starts with), and the stop it came
    /// from (the first stop: none). Its movie from boot is theirs one after the other (`Search::movie`)
    events: Vec<Event>,
    parent: usize,
    frame: u64,
    depth: u32,
    /// the objects ego has owned on the way here and did not own at the boot, ascending
    ever: Vec<u32>,
    /// the stop's level and line (the module's text)
    level: u32,
    line: u64,
    room: i32,
    /// the keys the inputs on the way here changed and that are not back yet, each with its old value; and whether
    /// the stop was made inside a window or after a wait
    pending: Vec<(Key, Val)>,
    inside: bool,
    /// the flags it holds set, but those seen unset again somewhere
    flags: u32,
    /// the last stop on the way here (or this one) whose set of flags was new in its line of gains
    fline: usize,
    /// its line of gains
    gline: u64,
    /// what it won over the stop it came from, where it began a line: the objects gained and the flags set
    news: Vec<Key>,
    /// the stop at the end of a walk the search made (`--go`)
    walk: bool,
    /// a stop made inside a window that is new by a guide variable's value (a count the game keeps of what was
    /// done while the window was open): it has its turn before any line's
    win: bool,
    /// a stop that carries into another room what the stop before it held only for a while, or where an input
    /// passed that on: these go before all others, the earliest first
    carry: bool,
    /// a stop the game led to another room: reached from the expanded stop by a wait alone
    led: bool,
    /// a result of the stop that began its line, in that stop's room (`--born-first`)
    born: bool,
}

/// The line of a stop: its gains (`ever`) and the objects it owns now (so its losses), hashed.
fn line_of(ever: &[u32], owned: &[u32]) -> u64 {
    let mut h = DefaultHasher::new();
    (ever, owned).hash(&mut h);
    h.finish()
}

/// Which variables are world (the module's text, "The world").
#[derive(Default)]
struct Policy {
    /// the guide's keys, every kind
    guide: HashSet<Key>,
    iface_vars: HashSet<Key>,
    clocks: HashSet<Key>,
    capped: HashSet<Key>,
}

impl Policy {
    fn world(&self, k: Key) -> bool {
        world_kind(k) || (k.kind() == K_VAR && self.guide.contains(&k) && !self.iface_vars.contains(&k) && !self.clocks.contains(&k) && !self.capped.contains(&k))
    }
    fn world_of(&self, f: &Facts) -> Vec<(Key, Val)> {
        f.iter().filter(|p| self.world(p.0)).copied().collect()
    }
}

/// One rung of the evaluation's ladder: `least` of its conditions (key, at least?, value; None: ego) hold.
struct Rung {
    name: String,
    conds: Vec<(Key, bool, Option<Val>)>,
    least: usize,
    at: Option<(usize, f64)>,
}

#[derive(Default)]
struct Search {
    nodes: Vec<Node>,
    /// per level, the queues; and the stops whose flags no kept stop of their line held all of, the last first
    queues: Vec<[VecDeque<usize>; QUEUES]>,
    stacks: Vec<Vec<usize>>,
    /// per line: the largest sets of flags kept stops hold (none within another), as bits by the flag's number
    tops: HashMap<u64, Vec<Vec<u64>>>,
    live: HashMap<usize, Box<dyn Engine>>,
    /// --shares: the lines take turns (the batch's loop), and the turns counted so far
    shares: bool,
    /// the stops expanded for a dead end (one click given there): none is given from them
    given: HashSet<usize>,
    /// where a count was walked to (`explore`'s counting): (the line of gains, the room, the count, its value)
    went: HashSet<(u64, i32, Key, Val)>,
    /// --motion-once: a key the game moves by itself is new once in a line of gains
    motion_once: bool,
    /// --born-first: in a line's turn the results of the stop that began the line go before its other stops;
    /// --walk-born: so do the results of a walk the search made for the line's news, in the room walked to
    born_first: bool,
    walk_born: bool,
    /// --said-flags: a flag seen unset again counts once more at a stop whose own sentence set it, in the room the
    /// sentence was made in (a door a sentence opens and the room's entry closes again)
    said_flags: bool,
    /// when the highest level was last raised (--stall), and the values whose first holding ends the run (--until)
    top_at: Option<Instant>,
    until: Vec<(Key, Val)>,
    done: bool,
    /// --count-lines: a walk made with a count whose stop is new by more than the count's value begins a line
    /// (the first for the count's value in a line of gains)
    count_lines: bool,
    count_line: HashSet<(u64, Key, Val)>,
    /// --stay-lines: the lines a set of flags began (the stop, its line of gains, the set it held then; true for
    /// a line a count's walk began: it has no set of its own), and those whose set, without the flags seen unset
    /// since, an older line of their line of gains holds all of: they have their turns after the others
    stay_lines: bool,
    flines: Vec<(usize, u64, Vec<u64>, bool)>,
    junk: HashSet<usize>,
    toggles_log: Option<std::fs::File>,
    tick: u64,
    /// the engines of expanded stops, each with the count at its last use: a stop that comes of one of them, or of
    /// a stop that does, is made from it
    parents: HashMap<usize, (Arc<Mutex<Box<dyn Engine>>>, u64)>,
    stamp: u64,
    /// what the kept stops of each line hold (the first field is the line)
    seen: HashSet<(u64, Key, Val)>,
    seen_at: HashSet<(u64, i32, Key, Val)>,
    sees: HashSet<(u64, i32, i32)>,
    ifaces: HashSet<(u64, u64)>,
    /// the objects ego owned at a stop that one of its own waits saw gone: what lasts only for a while (a count,
    /// for the summary)
    expires: HashSet<Key>,
    /// per line of gains, room and such a key: a stop was seen to hold it there, come from a stop in another room
    carried_at: HashSet<(u64, i32, Key)>,
    /// per line of gains and room: the changes an input made at a stop to an object the stop held only for a
    /// while (hashed)
    acted_at: HashSet<(u64, i32, u64)>,
    /// the slots that are the interface's own state: those a click changed that changed nothing else. A stop's
    /// identity for what is new leaves them out (the sentence being made and the inventory's page are no news)
    own_slots: HashSet<i32>,
    boxes: HashSet<(u64, i32, i32)>,
    cams: HashSet<(u64, i32, i32)>,
    cells: HashSet<(u64, u64)>,
    /// per line: the counts of flags set that a kept stop has held; and the flags seen unset again (they are no
    /// record of anything: a dialog's choices, a door's state)
    counts: HashSet<(u64, u32)>,
    toggles: HashSet<Key>,
    /// lines by gains alone (--lines gains); a new set of flags starts a line (--flag-lines)
    gains_only: bool,
    flag_lines: bool,
    /// --go: where the scripts name what a stop has won, and the clicks seen to lead from a room to the next
    go: bool,
    mentions: Mentions,
    edges: HashMap<(i32, i32), Vec<Hit>>,
    edges_log: Option<std::fs::File>,
    /// the ways a walk found false (from, to, the clicks), and how often
    dead: HashMap<(i32, i32, String), u32>,
    /// the scripts' local variables: per line the values kept stops held, the values seen, the clocks, the capped
    lseen: HashSet<(u64, u64, i32)>,
    lvals: HashMap<u64, Vec<i32>>,
    lclocks: HashSet<u64>,
    lcapped: HashSet<u64>,
    /// the objects ego owns at the boot
    root_owned: HashSet<u32>,
    var_vals: HashMap<Key, Vec<Val>>,
    policy: Policy,
    /// the world keys a run with no input changed somewhere: left out of the cells
    ambient: HashSet<Key>,
    /// per level, room, walk box and key: the stops expanded there whose waits changed it
    motion: HashMap<(u32, i32, i32, Key), u32>,
    /// per cell and key: the windows on that key followed from that cell
    wins: HashMap<(u64, Key), u32>,
    /// the game's own motion: the keys a wait moved at a stop where no input's change of them was pending
    noise: HashSet<Key>,
    rooms: Vec<i32>,
    /// the highest level of a kept stop
    top: u32,
    /// engines being made for kept stops (counted against LIVE_MAX)
    /// the batches' wall seconds: the stops explored (side by side), committed (one by one), the kept engines made
    wall: [f64; 3],
    /// the commit's seconds by part: the world and what is owned, the flags, the atoms, the rest of what is new, the
    /// cell, a kept stop's line and node
    ct: [f64; 6],
    /// the expansions' wall seconds by part (`Out::xt`), over all stops
    xt: [f64; 7],
    live_max: usize,
    phase: [f64; 5],
    frames: u64,
    skipped: u64,
    trials: u64,
    refused: u64,
    not_idle: u64,
    expanded: u64,
    boots: u64,
    kept: [u64; QUEUES],
    ends: Vec<(usize, String)>,
    watch: Vec<(Key, Val, Option<(usize, f64)>)>,
    rungs: Vec<Rung>,
    log: Option<std::fs::File>,
    offers: Option<std::fs::File>,
    /// OUT/rung-stops.jsonl: the line of each stop that is the first to hold a rung or a watched value, as it is
    /// found; OUT/top.jsonl: the line of the newest stop that began a line at the top level (a run goes on from it
    /// with --from-run OUT)
    rung_log: Option<std::fs::File>,
    top_path: Option<PathBuf>,
    t0: Option<Instant>,
}

struct Cfg {
    out: PathBuf,
    horizon: u64,
    workers: usize,
    /// the forks a round's first clicks are dealt to, side by side; and the stops of one batch for each stop
    /// expanded at once (workers / slices of them)
    slices: usize,
    batch: usize,
    /// the stops expanded at once (0: workers / slices)
    at_once: usize,
    /// the half-made stops clicked on from in one round, at most (MAX_MIDS; DIRECT_MAX_MIDS for a measurement)
    max_mids: usize,
    /// --cross-pages: inside a sentence that has its first object the page may be turned, for a second object on
    /// another page (two things of the inventory that no page shows together). 25 to 48% more trials a late stop
    cross_pages: bool,
    /// --runs-on: a trial's stop where a script the input started runs on is read again PROBE frames on
    runs_on: bool,
    /// --ends: a trial that comes to no stop within the horizon (the game takes no input again: an ending, a scene
    /// with no end in sight) is a result all the same, read where the horizon ends: a stop that is never expanded
    ends: bool,
}

static DIRS: AtomicU64 = AtomicU64::new(0);

fn eng_dir(out: &Path) -> PathBuf {
    let d = out.join("eng").join(format!("b{}", DIRS.fetch_add(1, Ordering::Relaxed)));
    std::fs::create_dir_all(&d).expect("an engine folder");
    d
}

fn world_kind(k: Key) -> bool {
    matches!(k.kind(), K_ROOM | K_OWNER | K_STATE | K_CLASS | K_BIT)
}

const NAMES: ObsReq = ObsReq { objects: None, names: true, exact: false };

/// What the interface is at a stop: the visible verb slots with their ids and texts (the verbs, the inventory's
/// names or images, the sentence being composed, a dialog's choices), and which scripts take a click and run a
/// sentence. Only the hash is kept: no game text leaves the process.
fn iface(o: &Obs) -> u64 {
    let mut v: Vec<(i32, i32, &str)> = o.verbs.iter().map(|x| (x.slot, x.id, x.name.as_deref().unwrap_or(""))).collect();
    v.sort_unstable();
    let mut h = DefaultHasher::new();
    v.hash(&mut h);
    // and the scripts that take the clicks and run the sentences (the engine's two variables for them): where a
    // room puts its own in their place, a click does another thing there, whatever the slots show
    (o.vars.get(afford::VAR_VERB_SCRIPT).copied(), o.vars.get(afford::VAR_SENTENCE_SCRIPT).copied()).hash(&mut h);
    h.finish()
}

/// What each visible slot shows, by slot: its id and text hashed (no game text leaves the process).
fn slots_of(o: &Obs) -> Vec<(i32, u64)> {
    let mut v: Vec<(i32, u64)> = o
        .verbs
        .iter()
        .map(|x| {
            let mut h = DefaultHasher::new();
            (x.id, x.name.as_deref().unwrap_or("")).hash(&mut h);
            (x.slot, h.finish())
        })
        .collect();
    v.sort_unstable();
    v
}

/// The local variables of the scripts running at a stop: a script that waits for the player (a dialog's) keeps its
/// count of what was said in them, and nothing else of the game's state shows it.
fn locals_of(o: &Obs) -> Vec<(u64, i32)> {
    let mut v = Vec::new();
    for sl in &o.slots {
        if let Some(ls) = &sl.locals {
            for (i, &x) in ls.iter().enumerate() {
                let mut h = DefaultHasher::new();
                (sl.where_, sl.number, i).hash(&mut h);
                v.push((h.finish(), x));
            }
        }
    }
    v
}

/// What the scripts running at a stop are at: each one's place, delay and local variables, hashed together.
fn scripts_of(o: &Obs) -> u64 {
    let mut v: Vec<(i32, i32, u64, i32, i32, &[i32])> =
        o.slots.iter().map(|sl| (sl.where_, sl.number, sl.offs, sl.frozen, sl.delay, sl.locals.as_deref().unwrap_or(&[]))).collect();
    v.sort_unstable();
    let mut h = DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}

/// The scripts running at a stop, each once: (where, number), sorted.
fn running(o: &Obs) -> Vec<(i32, i32)> {
    let mut v: Vec<(i32, i32)> = o.slots.iter().map(|sl| (sl.where_, sl.number)).collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Where the scripts running at a stop are: each one's place and delay (`scripts_of` without the local variables).
fn places_of(o: &Obs) -> u64 {
    let mut v: Vec<(i32, i32, u64, i32, i32)> = o.slots.iter().map(|sl| (sl.where_, sl.number, sl.offs, sl.frozen, sl.delay)).collect();
    v.sort_unstable();
    let mut h = DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}

/// One stop as read from its engine: the facts, and what is not in them.
struct Res {
    name: String,
    events: Vec<Event>,
    st: Status,
    facts: Facts,
    room: i32,
    /// the objects whose box is partly on the screen, and the actors on it (as negative ids)
    vis: Vec<i32>,
    walkbox: i32,
    cam: i32,
    /// what the interface shows (`iface`)
    iface: u64,
    /// the world's and the variables' atoms that differ from the expanded stop's (set by `explore`)
    changed: Vec<(Key, Val)>,
    /// the keys its input changed (against the twin), each with its old value; and whether it was made inside a
    /// window or after a wait
    own: Vec<(Key, Val)>,
    inside: bool,
    /// the click left another picture and no more: beside the round's twin at the same frame the room, ego's place,
    /// the scripts that take the clicks and the world are the same, and either one slot is another with every
    /// running script at the twin's place (a sentence half made; a local may hold what was picked), or several are
    /// with every script as the twin's, locals and all (a page of the interface). No stop for its interface alone
    pic: bool,
    /// what each visible slot shows (its id and text, hashed), by slot; and the two scripts that take the clicks
    /// and run the sentences, hashed: the interface's parts, for an identity without the slots that are the
    /// interface's own state (`Search::own_slots`)
    slots: Vec<(i32, u64)>,
    hands: u64,
    /// the local variables of the scripts that run at the stop: (script and index hashed, value); after `explore`
    /// only those that differ from the expanded stop's
    locals: Vec<(u64, i32)>,
    /// once the trial is done (`slim`): what the stop holds other than the expanded stop, each key with its value
    /// there (absent: 0) and here (None: absent). `facts` is empty from then on: a stop's facts are 400 kB, a round
    /// has thousands of results, and what is asked of a result afterwards is its difference
    delta: Vec<(Key, Val, Option<Val>)>,
}

/// What `r` holds other than `base`, in the keys' order: (key, the value in `base` or 0, the value in `r` or None).
fn delta_of(base: &Facts, r: &Facts) -> Vec<(Key, Val, Option<Val>)> {
    let (a, b) = (&base.0, &r.0);
    let (mut i, mut j, mut out) = (0, 0, Vec::new());
    while i < a.len() || j < b.len() {
        if j >= b.len() || (i < a.len() && a[i].0 < b[j].0) {
            out.push((a[i].0, a[i].1, None));
            i += 1;
        } else if i >= a.len() || b[j].0 < a[i].0 {
            out.push((b[j].0, 0, Some(b[j].1)));
            j += 1;
        } else {
            if a[i].1 != b[j].1 {
                out.push((a[i].0, a[i].1, Some(b[j].1)));
            }
            i += 1;
            j += 1;
        }
    }
    out
}

/// `base` with a `delta_of` applied: the facts the delta was made from.
fn patched(base: &Facts, d: &[(Key, Val, Option<Val>)]) -> Facts {
    let a = &base.0;
    let (mut i, mut j, mut out) = (0, 0, Vec::with_capacity(a.len() + d.len()));
    while i < a.len() || j < d.len() {
        if j >= d.len() || (i < a.len() && a[i].0 < d[j].0) {
            out.push(a[i]);
            i += 1;
        } else {
            if let Some(v) = d[j].2 {
                out.push((d[j].0, v));
            }
            if i < a.len() && a[i].0 == d[j].0 {
                i += 1;
            }
            j += 1;
        }
    }
    Facts(out)
}

/// A trial's result once the trial is done with it: its difference from the expanded stop, and no facts.
fn slim(mut r: Res, base: &Facts) -> Res {
    r.delta = delta_of(base, &r.facts);
    r.facts = Facts(Vec::new());
    r
}

fn read(name: String, events: Vec<Event>, o: &Obs, st: Status) -> Res {
    let facts = facts_of(o);
    let (x0, x1) = (st.xstart, st.xstart + afford::SCREEN_W);
    let mut vis: Vec<i32> = o
        .room_objects
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .filter(|r| r.w > 0 && r.h > 0 && r.x < x1 && r.x + r.w > x0)
        .map(|r| r.id)
        .chain(o.actors.iter().filter(|a| a.x >= x0 && a.x < x1).map(|a| -a.id))
        .collect();
    vis.sort_unstable();
    Res {
        name,
        events,
        room: facts.val(Key::pack(K_ROOM, 0, 0, 0)) as i32,
        vis,
        walkbox: st.walkbox.unwrap_or(-1),
        cam: st.xstart.div_euclid(CAM_BUCKET),
        iface: iface(o),
        changed: Vec::new(),
        own: Vec::new(),
        inside: false,
        pic: false,
        slots: slots_of(o),
        hands: {
            let mut h = DefaultHasher::new();
            (o.vars.get(afford::VAR_VERB_SCRIPT).copied(), o.vars.get(afford::VAR_SENTENCE_SCRIPT).copied()).hash(&mut h);
            h.finish()
        },
        locals: locals_of(o),
        delta: Vec::new(),
        facts,
        st,
    }
}


/// The world's cell: the room, the world's atoms but the ambient keys', and the interface, wherever ego stands.
fn cell(room: i32, iface: u64, world: &[(Key, Val)], ambient: &HashSet<Key>) -> u64 {
    let mut h = DefaultHasher::new();
    (room, iface).hash(&mut h);
    for p in world.iter().filter(|p| !ambient.contains(&p.0)) {
        p.hash(&mut h);
    }
    h.finish()
}

enum Played {
    Stop(Status),
    Ended(Status),
    NotIdle,
}

fn go(b: &mut Box<dyn Engine>, spec: RunSpec) -> EResult<Result<Status, Status>> {
    match b.run(&spec) {
        Ok(s) => Ok(Ok(s)),
        Err(EngineError::Ended(s)) => Ok(Err(*s)),
        Err(x) => Err(x),
    }
}

/// Whether the game takes a click at this stop (the engine's own gate, checkExecVerbs: user input on, no cutscene,
/// a room).
fn takes_click(st: &Status) -> bool {
    st.up > 0 && st.cs == 0 && st.room != 0
}

/// On to a stop where ego stands still and the bridge is idle, or, from frame `busy` on, the game takes a click; or
/// `end`.
fn settle(b: &mut Box<dyn Engine>, end: u64, busy: u64, ego: i32) -> EResult<Played> {
    // the frames at which the engine's own tests said ego stands, and a click would count (the status's fields may
    // say otherwise: the moving flags hold a pose's bits, the gate has more to it than the status shows)
    let (mut stood, mut gate): (Option<u64>, Option<u64>) = (None, None);
    let mut same = 0;
    loop {
        let st = b.status().clone();
        if st.frame >= end || same > 8 {
            return Ok(Played::NotIdle);
        }
        let ready = st.idle.is_some() || (st.frame >= busy && (takes_click(&st) || gate == Some(st.frame)));
        if ready && (st.mv.unwrap_or(0) == 0 || stood == Some(st.frame)) {
            return Ok(Played::Stop(st));
        }
        let spec = if ready {
            // ego still walking: on to where it stands
            let stands = Cond { kind: CondKind::ActorMoving, subject: ego, op: CondOp::Eq, value: 0, values: Vec::new(), negate: false };
            RunSpec { until: vec![stands], max_frame: Some(end), ..Default::default() }
        } else if st.frame < busy {
            RunSpec { idle: true, max_frame: Some(busy.min(end)), ..Default::default() }
        } else {
            // past `busy` and no click taken yet: on to the first frame that is idle or takes one
            RunSpec { idle: true, control: true, max_frame: Some(end), ..Default::default() }
        };
        match go(b, spec)? {
            Err(s) => return Ok(Played::Ended(s)),
            Ok(s) => {
                if ready && s.reason == "until" {
                    stood = Some(s.frame);
                } else if s.reason == "control" {
                    gate = Some(s.frame);
                }
                // a run that stopped where it started must have told something new, or the loop ends
                same = if s.frame == st.frame { same + 1 } else { 0 };
            }
        }
    }
}

/// `settle`, then CALM frames on and `settle` again: the texts an input leaves on the interface for a moment are gone.
/// A stop not reached within `horizon` frames of `from` gets four times as many when a cutscene runs then.
fn calm(b: &mut Box<dyn Engine>, from: u64, horizon: u64, ego: i32) -> EResult<Played> {
    for end in [from + horizon, from + 4 * horizon] {
        // the longer horizon is a cutscene's: outside one, a run that reached no stop in the first is left there
        if end > from + horizon && b.status().cs == 0 {
            break;
        }
        let at = b.status().frame;
        match settle(b, end, from + LEAD + BUSY, ego)? {
            // it stood already, CALM frames past the inputs' frames (the trial ran them before this): read at once
            Played::Stop(st) if st.frame == at && at >= from + LEAD + CALM => return Ok(Played::Stop(st)),
            Played::Stop(st) => {
                if let Err(s) = go(b, RunSpec { frame: Some(st.frame + CALM), ..Default::default() })? {
                    return Ok(Played::Ended(s));
                }
                match settle(b, end.max(st.frame + CALM + 1), from + LEAD + BUSY, ego)? {
                    Played::NotIdle => {}
                    x => return Ok(x),
                }
            }
            Played::NotIdle => {}
            x => return Ok(x),
        }
    }
    Ok(Played::NotIdle)
}

fn name_of(pre: &str, clicks: &[Point]) -> String {
    format!("{pre}{}", clicks.iter().map(|p| p.hit.name()).collect::<String>())
}

/// What one half-made stop's clicks leave for the round to decide, in the clicks' order (`clicks`): the round keeps
/// one list of the half-made stops it has seen, and that list is read and written in the round's order only.
enum Act {
    /// a verb's slot at the round's stop left this interface
    Gave(Hit, u64),
    /// a click that changed only what the interface shows: whether it is a page, the stop it left and the slots shown
    /// before it
    Shows { hit: Hit, page: bool, cand: MidCand, before: Arc<HashSet<(i32, i32, String)>> },
    /// a sentence half made (`always`: its key is noted even at the last depth)
    Half { cand: MidCand, only: Option<Arc<HashSet<(i32, i32, String)>>>, always: bool },
    /// a window
    Window(Cand),
}

/// A stop that may be clicked on from: its engine and clicks, and its identity in the round (what the interface
/// shows and the running scripts' locals).
struct MidCand {
    engine: Box<dyn Engine>,
    clicks: Vec<Point>,
    events: Vec<Event>,
    iface: u64,
    base: u64,
    paged: bool,
    /// the stop it comes of is inside a sentence (`Mid::inside`)
    inside: bool,
}

/// What the scripts are LEAD and LEAD + CALM frames on from a stop with no input, and the facts there.
type Quiet = (u64, u64, Option<Facts>, Option<Facts>);

/// One click's trial from a half-made stop: its results, what it leaves to the round, the slot it learned to skip,
/// the first click of its sequence (a trace).
struct PointOut {
    out: Out,
    acts: Vec<Act>,
    learned: Vec<Hit>,
    first: Option<String>,
}

/// One half-made stop's clicks: the trials' results, what is left to the round, the slots it learned to skip (the
/// first stop of the second depth), a trace's counts.
struct MidOut {
    out: Out,
    acts: Vec<Act>,
    learned: Option<HashSet<Hit>>,
    by_first: HashMap<String, u64>,
}

/// The twin of a round: a copy of the round's stop that takes no input, to fork from, and its world at the frames
/// asked so far. The stops of a round worked on side by side each run a fork of their own.
struct Twins {
    src: Mutex<Option<Box<dyn Engine>>>,
    at: Mutex<HashMap<u64, Option<(Facts, u64, u64, Vec<(i32, i32)>)>>>,
}

impl Twins {
    /// The world of the round's stop run to frame `f` with no input (None: no twin, or its run ended before).
    fn world(&self, twin: &mut Option<Box<dyn Engine>>, f: u64) -> EResult<Option<Facts>> {
        Ok(self.at(twin, f)?.map(|x| x.0))
    }

    /// The same, with where its running scripts are there (`places_of`), and with their locals (`scripts_of`).
    fn at(&self, twin: &mut Option<Box<dyn Engine>>, f: u64) -> EResult<Option<(Facts, u64, u64, Vec<(i32, i32)>)>> {
        if let Some(w) = self.at.lock().unwrap().get(&f) {
            return Ok(w.clone());
        }
        if twin.as_ref().map_or(true, |t| t.status().frame > f) {
            if let Some(t) = twin.take() {
                t.quit();
            }
            match self.src.lock().unwrap().as_mut() {
                Some(src) => *twin = Some(src.fork()?),
                None => return Ok(None),
            }
        }
        let t = twin.as_mut().unwrap();
        let alive = t.status().frame == f || go(t, RunSpec { frame: Some(f), ..Default::default() })?.is_ok();
        let w = if alive {
            let o = t.obs(&NAMES)?;
            Some((Facts(t.facts()?.iter().filter(|p| world_kind(p.0)).copied().collect()), places_of(&o), scripts_of(&o), running(&o)))
        } else {
            if let Some(t) = twin.take() {
                t.quit();
            }
            None
        };
        self.at.lock().unwrap().insert(f, w.clone());
        Ok(w)
    }
}

/// A half-made stop without its engine: what a thread other than the engine's may read of it.
struct MidAt {
    clicks: Vec<Point>,
    events: Vec<Event>,
    iface: u64,
    only: Option<Arc<HashSet<(i32, i32, String)>>>,
    paged: bool,
    inside: bool,
}

/// A stop inside a sentence being composed: its engine (None: the round's own), and the clicks that led to it.
struct Mid {
    engine: Option<Box<dyn Engine>>,
    clicks: Vec<Point>,
    events: Vec<Event>,
    /// what the interface shows there
    iface: u64,
    /// after a click that changed only what the interface shows (the module's text, "Inputs"): the slots shown
    /// before it. From such a stop only the slots that changed are clicked, and the slots that start a sentence;
    /// from the sentence only those that changed
    only: Option<Arc<HashSet<(i32, i32, String)>>>,
    /// a page of the interface is among its clicks
    paged: bool,
    /// a sentence is half made there with its first object (so the page may be turned for its other object: a
    /// verb alone is clicked after the page as well as before it), or it is a page turned from such a stop
    inside: bool,
}

/// A stop that is a window (the module's text, "After an input"): its engine, its round's name, its events, the keys
/// its input changed (with their old values), its cell and the keys that came back.
struct Cand {
    engine: Box<dyn Engine>,
    name: String,
    events: Vec<Event>,
    keys: Vec<(Key, Val)>,
    cell: u64,
    back: Vec<Key>,
}

/// What a stop's trials gave.
#[derive(Default)]
struct Out {
    /// the windows found in a round, for the stop's expansion to follow or not
    cands: Vec<Cand>,
    results: Vec<Res>,
    ends: Vec<String>,
    trials: u64,
    refused: u64,
    not_idle: u64,
    mids: u64,
    /// stops that were windows
    windows: u64,
    /// clicks not tried: slots known to pick another verb or to do nothing
    skipped: u64,
    /// the windows followed: each one's cell with each of its keys
    followed: Vec<(u64, Key)>,
    /// the ways (from, to) whose clicks a walk made and that did not lead on
    bad: Vec<(i32, i32)>,
    /// the keys the stop's waits moved while no input's change of them was pending
    noise: HashSet<Key>,
    /// the scripts' local variables the stop's waits moved: clocks
    lclocks: HashSet<u64>,
    offer: Value,
    /// the variables the input script wrote in a trial, or that a fifth of the stop's inputs changed
    iface_vars: HashSet<Key>,
    /// the slots a click changed that changed nothing else (a sentence's line, a page's slots): the interface's
    /// own state
    own_slots: HashSet<i32>,
    /// the variables, and the world's other keys, a run with no input changed
    clocks: HashSet<Key>,
    ambient: HashSet<Key>,
    room: i32,
    walkbox: i32,
    /// the trials' wall seconds by phase (the fork, the inputs, the run to the stop, the stop's reading, the quit), and
    /// the frames they ran
    phase: [f64; 5],
    frames: u64,
    /// the expansion's wall seconds: the waits, the stop's round, the windows' rounds, the rounds after a wait, the
    /// walks; and of all those rounds, the stops worked on one by one and those side by side
    xt: [f64; 7],
    /// the expanded stop's facts: a result's are these with its `delta`
    base: Facts,
    /// a dead end: the stop takes sentences, no wait moved a thing, and of its own inputs none was more than one
    /// click, none moved ego, changed the room or a key of the world
    dead: bool,
}

impl Search {
    /// A kept stop's movie from boot: the events of each trial on the way to it, from the first stop's down.
    fn movie(&self, id: usize) -> Vec<Event> {
        let mut chain = vec![id];
        while self.nodes[*chain.last().unwrap()].parent != usize::MAX {
            chain.push(self.nodes[*chain.last().unwrap()].parent);
        }
        chain.iter().rev().flat_map(|&i| self.nodes[i].events.iter().cloned()).collect()
    }
}

/// A kept stop of a run's file: its line and its movie from boot. A line of nodes.jsonl holds the events of its own
/// trial ("movie": "own") and names the stop it came from, down to the run's first stop, whose line holds the movie
/// the run began with. A line of rung-stops.jsonl or top.jsonl, and of a run before this form, holds the whole
/// movie. `id` None: the file's first line.
fn stop_of(path: &str, id: Option<u64>) -> (Value, Vec<Event>) {
    let text = std::fs::read_to_string(path).expect("the stops' file");
    let find = |id: Option<u64>| -> Value {
        let line = match id {
            None => text.lines().next(),
            Some(id) => {
                let head = format!("{{\"id\":{id},");
                text.lines().find(|l| l.starts_with(&head)).or_else(|| text.lines().find(|l| serde_json::from_str::<Value>(l).is_ok_and(|v| v["id"].as_u64() == Some(id))))
            }
        };
        serde_json::from_str(line.expect("the stop's line")).expect("the stop's line read")
    };
    let events = |v: &Value| -> Vec<Event> {
        let a = v["events"].as_array().expect("a stop's events");
        let e: Vec<Event> = a.iter().filter_map(fmi_explore::events::event_of).collect();
        assert_eq!(e.len(), a.len(), "every event read");
        e
    };
    let line = find(id);
    let mut parts = vec![events(&line)];
    let mut at = line.clone();
    while at["movie"].as_str() == Some("own") {
        at = find(Some(at["parent"].as_u64().expect("the stop it came from")));
        parts.push(events(&at));
    }
    parts.reverse();
    (line, parts.into_iter().flatten().collect())
}

impl Out {
    /// Another part of the same stop's trials, after this one.
    fn absorb(&mut self, o: Out) {
        self.cands.extend(o.cands);
        self.results.extend(o.results);
        self.ends.extend(o.ends);
        self.trials += o.trials;
        self.refused += o.refused;
        self.not_idle += o.not_idle;
        self.mids += o.mids;
        self.windows += o.windows;
        self.skipped += o.skipped;
        self.followed.extend(o.followed);
        self.bad.extend(o.bad);
        if self.offer.is_null() {
            self.offer = o.offer;
        }
        self.iface_vars.extend(o.iface_vars);
        self.own_slots.extend(o.own_slots);
        self.clocks.extend(o.clocks);
        self.ambient.extend(o.ambient);
        self.noise.extend(o.noise);
        self.lclocks.extend(o.lclocks);
        for (a, b) in self.phase.iter_mut().zip(o.phase.iter()) {
            *a += b;
        }
        for (a, b) in self.xt.iter_mut().zip(o.xt.iter()) {
            *a += b;
        }
        self.frames += o.frames;
    }
}

/// A line on stderr when DIRECT_TRACE is set (a measurement aid).
fn trace(text: &str) {
    if std::env::var_os("DIRECT_TRACE").is_some() {
        eprintln!("TRACE {text}");
    }
}

/// A point where the pointer is over no verb, object or actor, by the engine's hit test at this stop.
fn park_point(e: &mut Box<dyn Engine>) -> EResult<Option<(i32, i32)>> {
    let (w, h) = (afford::SCREEN_W, afford::SCREEN_H);
    let cands = [(w - 1, h - 1), (0, h - 1), (w / 2, h - 1), (w - 1, 0), (0, 0), (w / 2, 0), (w - 1, h / 2), (0, h / 2)];
    let ats = e.at_many(&cands)?;
    Ok(cands.iter().zip(ats.iter()).find(|(_, a)| a.verb_slot == 0 && a.main.map_or(true, |m| m.2 == 0 && m.3 == 0)).map(|(p, _)| *p))
}

/// The click sequences from the engine's stop (the module's text, "Inputs"), each a trial; `pre` names the round.
fn clicks(
    e: &mut Box<dyn Engine>,
    pre: &str,
    lead: &[Event],
    motion: Option<&HashSet<Key>>,
    carry: &[(Key, Val)],
    lown: &HashSet<u64>,
    base: &Facts,
    snap: &Snap<'_>,
    cfg: &Cfg,
    out: &mut Out,
) -> EResult<()> {
    let o0 = e.obs(&NAMES)?;
    let st0 = e.status().clone();
    let r0 = read(String::new(), Vec::new(), &o0, st0);
    let sentence = r0.facts.val(Key::var(afford::VAR_SENTENCE_SCRIPT as u64)) as i32;
    let input0 = r0.facts.val(Key::var(afford::VAR_VERB_SCRIPT as u64)) as i32;
    let ego = r0.facts.val(Key::pack(K_EGO, 0, 0, 0)) as i32;
    // what the slots show at the round's stop
    let shown0: HashSet<(i32, i32, String)> = o0.verbs.iter().map(|x| (x.slot, x.id, x.name.clone().unwrap_or_default())).collect();
    // a stop inside a sentence or a dialog, for the round: what the interface shows there, and the running scripts'
    // locals but the clocks (a dialog that shows the same choices again has counted the line said)
    let mkey = |r: &Res| -> u64 {
        let mut v: Vec<(u64, i32)> = r.locals.iter().filter(|l| !snap.lclocks.contains(&l.0) && !lown.contains(&l.0)).copied().collect();
        v.sort_unstable();
        let mut h = DefaultHasher::new();
        (r.iface, v).hash(&mut h);
        h.finish()
    };
    // (those no page led to, those a page led to)
    let mut mids_here = [0usize; 2];
    // (a trace: the round's trials by the first click they began with)
    let tracing = std::env::var_os("DIRECT_TRACE").is_some();
    let mut by_first: HashMap<String, u64> = HashMap::new();
    let mut seen_mid: HashSet<u64> = HashSet::from([mkey(&r0)]);
    let mut frontier: Vec<Mid> = vec![Mid { engine: None, clicks: Vec::new(), events: lead.to_vec(), iface: r0.iface, only: None, paged: false, inside: false }];
    // the stops that are windows (the module's text, "After an input"), MAX_WINDOWS of them at most
    let mut found = 0;
    // the twin: a copy of the round's stop to fork it from, and its world at the frames asked (every round has one:
    // what a click left is held against it; what an input changed in the world is asked only where `motion` is given)
    let twins = Twins { src: Mutex::new(Some(e.fork()?)), at: Mutex::new(HashMap::new()) };
    // the verb slots as the first clicks left them (a half-made stop's interface), and those a later click need not
    // try: a slot that from the first half-made stop gave what it gave from the round's stop (it only picks another
    // verb), or gave that stop again (it does nothing)
    let mut first_gave: HashMap<Hit, u64> = HashMap::new();
    let mut skip: Option<HashSet<Hit>> = None;
    // the slots whose click from the round's stop changed only what the interface shows
    let mut shifters: HashSet<Hit> = HashSet::new();
    // One half-made stop's clicks (the round's stop is the first of them): every trial from it. It reads the round's
    // lists and writes none: what the round decides (which stops are clicked on from, which windows are followed,
    // what the slots teach) is left in `acts`, in the clicks' order. So stops that no stop before them in the round's
    // order can change are worked on side by side, and the round is what it is with one thread.
    let task = |me: &mut Box<dyn Engine>, m: &MidAt, depth: usize, inner: usize, skip: &Option<HashSet<Hit>>, shifters: &HashSet<Hit>, first_gave: &HashMap<Hit, u64>| -> EResult<MidOut> {
        let mut out = Out::default();
        let mut acts: Vec<Act> = Vec::new();
        let mut by_first: HashMap<String, u64> = HashMap::new();
        let mut twin: Option<Box<dyn Engine>> = None;
        {
            let o = me.obs(&NAMES)?;
            let shown: Arc<HashSet<(i32, i32, String)>> = Arc::new(o.verbs.iter().map(|x| (x.slot, x.id, x.name.clone().unwrap_or_default())).collect());
            // how many slots show each image here, and the slots with that count beside them (as slot -1, the count
            // for the id): two things may share a picture, so an image is the same thing in another place only where
            // no more slots show it than did on a page before
            let mut images: HashMap<String, i32> = HashMap::new();
            for n in o.verbs.iter().filter_map(|x| x.name.as_ref()).filter(|n| n.starts_with('#')) {
                *images.entry(n.clone()).or_default() += 1;
            }
            let shown_all: Arc<HashSet<(i32, i32, String)>> = Arc::new(shown.iter().cloned().chain(images.iter().flat_map(|(n, &c)| (1..=c).map(move |k| (-1, k, n.clone())))).collect());
            let of = afford::offer_cfg(me, &o, false, true)?;
            let park = park_point(me)?;
            // the trials' forks record their slot runs and start with empty logs
            me.segment(true)?;
            me.runs()?;
            let _ = me.input_log();
            let mf = me.status().frame;
            // what the scripts are at LEAD and LEAD + CALM frames on with no input (made at the first click that may
            // have changed the interface alone)
            let quiet: Mutex<Option<Arc<Quiet>>> = Mutex::new(None);
            let learn = depth == 2 && skip.is_none() && m.only.is_none();
            let mut learned: HashSet<Hit> = HashSet::new();
            let mego = o.ego_pos;
            let mut points: Vec<Point> = match &m.only {
                None => {
                    let mut v = of.verbs.clone();
                    v.extend(of.targets.iter().copied());
                    v
                }
                // after an interface-only click: the slots that changed (a slot whose image as many slots showed
                // on a page before is the same thing in another place, and no change); at the round's depth also
                // the slots that start a sentence and those that change the interface again
                Some(before) => of
                    .verbs
                    .iter()
                    .filter(|p| match p.hit {
                        Hit::Verb { slot, id } => {
                            shown.iter().any(|x| x.0 == slot && x.1 == id && if x.2.starts_with('#') { !before.contains(&(-1, images[&x.2], x.2.clone())) } else { !before.contains(x) })
                                || (depth == 1 && first_gave.contains_key(&p.hit))
                                || shifters.contains(&p.hit)
                        }
                        _ => false,
                    })
                    .copied()
                    .collect(),
            };
            if depth == 1 && m.only.is_none() {
                if out.offer.is_null() {
                    out.offer = of.json();
                }
                // floor clicks: a grid over the main screen, where the hit test finds no object and no actor
                let (top, h) = me.status().screen.map(|s| (s.0, s.1)).unwrap_or((0, afford::SCREEN_H));
                let grid: Vec<(i32, i32)> =
                    FLOOR_Y.iter().flat_map(|&k| FLOOR_X.iter().map(move |&x| (x, (top + (h - top) * k / 8).min(h - 2)))).collect();
                let ats = me.at_many(&grid)?;
                for (i, (&(x, y), a)) in grid.iter().zip(ats.iter()).enumerate() {
                    if a.main.is_some_and(|m| m.2 == 0 && m.3 == 0) {
                        points.push(Point { x, y, hit: Hit::Walk(i as i32) });
                    }
                }
            }
            // One click from the stop: its trial, and what it leaves for the stop's list. It reads the stop and
            // writes nothing of it, so a stop's clicks run side by side where the stop stands alone in its group,
            // each thread forking its trials from a copy of the stop's engine of its own
            let point = |src: &mut Box<dyn Engine>, p: &Point, twin: &mut Option<Box<dyn Engine>>| -> EResult<PointOut> {
                let mut out = Out::default();
                let mut acts: Vec<Act> = Vec::new();
                let mut learned: Vec<Hit> = Vec::new();
                let mut first: Option<String> = None;
                let mut qv: Option<Arc<Quiet>> = None;
                // (inside a sentence the page is turned only from a stop where the sentence is half made, or from a
                // page turned there: its other object may be on another page)
                if depth >= 2 && (skip.as_ref().is_some_and(|s| s.contains(&p.hit)) || (shifters.contains(&p.hit) && !m.inside)) {
                    out.skipped += 1;
                    return Ok(PointOut { out, acts, learned, first });
                }
                let t = Instant::now();
                let mut b = src.fork()?;
                out.phase[0] += t.elapsed().as_secs_f64();
                let t = Instant::now();
                let mut cl = m.clicks.clone();
                cl.push(*p);
                let mut events = m.events.clone();
                events.push(Event { frame: mf + 1, what: Input::Click(p.x, p.y) });
                if let Some((x, y)) = park {
                    events.push(Event { frame: mf + 2, what: Input::Move(x, y) });
                }
                out.trials += 1;
                if tracing {
                    first = Some(cl[0].hit.name());
                }
                match b.inputs(&events[m.events.len()..]) {
                    Ok(()) => {}
                    Err(EngineError::Bridge(_)) => {
                        out.refused += 1;
                        b.quit();
                        return Ok(PointOut { out, acts, learned, first });
                    }
                    Err(x) => return Err(x),
                }
                out.phase[1] += t.elapsed().as_secs_f64();
                let t = Instant::now();
                let early = match go(&mut b, RunSpec { frame: Some(mf + LEAD), ..Default::default() })? {
                    Ok(st) => st,
                    Err(st) => {
                        out.ends.push(format!("{} {}", name_of(pre, &cl), st.reason));
                        b.quit();
                        return Ok(PointOut { out, acts, learned, first });
                    }
                };
                if depth == 1 {
                    // the variables the input script wrote for this click: the interface's own
                    if let Ok(l) = b.input_log() {
                        out.iface_vars.extend(l.records.iter().flat_map(|r| r.writes.iter()).filter(|t| t.kind == TouchKind::Var).map(|t| Key::var(t.a as u64)));
                    }
                }
                // a sentence half made shows at once (the game is idle again, ego stands where it stood, only the
                // input script ran): the stop to click on from is this one, LEAD frames after the click
                let mut settled_half: Option<bool> = None;
                let mut ran_on = false;
                if (early.idle.is_some() || takes_click(&early)) && early.mv.unwrap_or(0) == 0 {
                    let mut ob = b.obs(&NAMES)?;
                    let mut at = early.clone();
                    let half3 = ob.ego_pos == mego && {
                        let runs = b.runs()?;
                        let input = [input0, facts_of(&ob).val(Key::var(afford::VAR_VERB_SCRIPT as u64)) as i32];
                        afford::composing(early.idle.as_deref().or(Some("sentence")), &runs.runs, mf + 1, &input, sentence)
                    };
                    // or only what the interface shows changed, and CALM frames on it still is so (a sentence that is
                    // about to run shows too): clicked on from as well. The world is held against what it is with
                    // no input at the same frame: a room moves by itself (a juggler, a fire), and that is not the
                    // click's doing
                    let world_same = |x: &Obs, w: &Option<Facts>| w.as_ref().is_some_and(|w| !w.diff(&facts_of(x)).iter().any(|d| world_kind(d.0)));
                    let mut shows = !half3 && early.idle.is_some() && ob.ego_pos == mego && iface(&ob) != m.iface;
                    let mut half = half3;
                    if half3 || shows {
                        // and no script is anywhere else than with no input at all: a dialog's line clears the
                        // choices at once and its script goes on a few frames later; such a click is read where
                        // the game has settled, as any other
                        // (made once for the stop, by the first click that asks for it)
                        qv = Some({
                            let mut g = quiet.lock().unwrap();
                            if g.is_none() {
                                let mut tw = src.fork()?;
                                out.trials += 1;
                                let mut at = [0u64; 2];
                                let mut w: [Option<Facts>; 2] = [None, None];
                                for (k, f) in [mf + LEAD, mf + LEAD + CALM].into_iter().enumerate() {
                                    if go(&mut tw, RunSpec { frame: Some(f), ..Default::default() })?.is_ok() {
                                        let to = tw.obs(&NAMES)?;
                                        at[k] = scripts_of(&to);
                                        w[k] = Some(facts_of(&to));
                                    }
                                }
                                tw.quit();
                                let [w0, w1] = w;
                                *g = Some(Arc::new((at[0], at[1], w0, w1)));
                            }
                            g.clone().unwrap()
                        });
                        let still = qv.as_ref().is_some_and(|q| q.0 == scripts_of(&ob));
                        shows = shows && still && qv.as_ref().is_some_and(|q| world_same(&ob, &q.2));
                        half = half3 && still;
                    }
                    if !half3 || half {
                        settled_half = Some(half);
                    }
                    if shows {
                        let shown3 = iface(&ob);
                        match go(&mut b, RunSpec { frame: Some(mf + LEAD + CALM), ..Default::default() })? {
                            Err(st) => {
                                out.ends.push(format!("{} {}", name_of(pre, &cl), st.reason));
                                b.quit();
                                return Ok(PointOut { out, acts, learned, first });
                            }
                            Ok(st2) => {
                                ran_on = true;
                                let ob2 = b.obs(&NAMES)?;
                                shows = st2.idle.is_some() && st2.mv.unwrap_or(0) == 0 && ob2.ego_pos == mego && iface(&ob2) == shown3 && qv.as_ref().is_some_and(|q| q.1 == scripts_of(&ob2) && world_same(&ob2, &q.3));
                                if shows {
                                    ob = ob2;
                                    at = st2;
                                }
                            }
                        }
                    }
                    if half || shows {
                        let r = read(name_of(pre, &cl), events, &ob, at);
                        out.phase[2] += t.elapsed().as_secs_f64();
                        if shows {
                            // a page of the interface (several slots changed) from the round's stop: clicked on from
                            // at the same depth, at the slots that changed. Else the start or the middle of a
                            // sentence: a stop to click on from like any other
                            let now: HashSet<(i32, i32, String)> = ob.verbs.iter().map(|x| (x.slot, x.id, x.name.clone().unwrap_or_default())).collect();
                            let mut slots: Vec<i32> = now.symmetric_difference(&shown).map(|x| x.0).collect();
                            slots.sort_unstable();
                            slots.dedup();
                            let page = slots.len() >= 2;
                            out.own_slots.extend(slots.iter().copied());
                            trace(&format!("    shows {} depth {depth} ({} slots changed)", r.name, slots.len()));
                            // (the slots shown before it: on this stop and, where pages led to it, on those before)
                            let before = match &m.only {
                                Some(b) => Arc::new(b.union(&shown_all).cloned().collect()),
                                None => shown_all.clone(),
                            };
                            let cand = MidCand { base: mkey(&r), engine: b, clicks: cl, iface: r.iface, events: r.events, paged: m.paged, inside: m.inside };
                            acts.push(Act::Shows { hit: p.hit, page, cand, before });
                            return Ok(PointOut { out, acts, learned, first });
                        }
                        // (a sentence half made: the slots it changed are the interface's own)
                        {
                            let now: HashSet<(i32, i32, String)> = ob.verbs.iter().map(|x| (x.slot, x.id, x.name.clone().unwrap_or_default())).collect();
                            out.own_slots.extend(now.symmetric_difference(&shown).map(|x| x.0));
                        }
                        if matches!(p.hit, Hit::Verb { .. }) && m.only.is_none() {
                            if depth == 1 {
                                acts.push(Act::Gave(p.hit, r.iface));
                            } else if learn && (first_gave.get(&p.hit) == Some(&r.iface) || r.iface == m.iface) {
                                learned.push(p.hit);
                            }
                        }
                        // a sentence begun after a page of the interface goes on at the slots that changed
                        let only = if depth == 1 { m.only.clone() } else { None };
                        let cand = MidCand { base: mkey(&r), engine: b, clicks: cl, iface: r.iface, events: r.events, paged: m.paged, inside: m.inside };
                        acts.push(Act::Half { cand, only, always: false });
                        return Ok(PointOut { out, acts, learned, first });
                    }
                }
                if !ran_on {
                    if let Err(st) = go(&mut b, RunSpec { frame: Some(mf + LEAD + CALM), ..Default::default() })? {
                        out.ends.push(format!("{} {}", name_of(pre, &cl), st.reason));
                        b.quit();
                        return Ok(PointOut { out, acts, learned, first });
                    }
                }
                let played = calm(&mut b, mf, cfg.horizon, ego)?;
                out.phase[2] += t.elapsed().as_secs_f64();
                out.frames += b.status().frame.saturating_sub(mf);
                let t = Instant::now();
                match played {
                    Played::Stop(st) => {
                        let ob = b.obs(&NAMES)?;
                        let mut r = read(name_of(pre, &cl), events, &ob, st);
                        r.inside = !pre.is_empty();
                        // inside a window: its keys are still the window's input's doing, where not back yet
                        r.own = carry.iter().filter(|c| r.facts.val(c.0) != c.1).copied().collect();
                        // only the input script ran since the click and ego stands where it stood: a sentence half made
                        let half = settled_half.is_none() && ob.ego_pos == mego && {
                            let runs = b.runs()?;
                            let input = [input0, r.facts.val(Key::var(afford::VAR_VERB_SCRIPT as u64)) as i32];
                            afford::composing(r.st.idle.as_deref().or(Some("sentence")), &runs.runs, mf + 1, &input, sentence)
                        };
                        // or a slot's click left the interface another and the world what it is with no input at
                        // the same frame (an object's own script ran for the slot, and no sentence): half made too.
                        // (In the stop's own round. After a wait or inside a window the round is the shorter one
                        // it was: measured, such sentences there cost 41 to 80% more trials to the same stops.)
                        let half = half
                            || (motion.is_some() && matches!(p.hit, Hit::Verb { .. }) && r.room == r0.room && ob.ego_pos == mego && r.st.idle.as_deref() == Some("sentence") && r.iface != m.iface && {
                                let mine = Facts(r.facts.iter().filter(|p| world_kind(p.0)).copied().collect());
                                twins.world(twin, r.st.frame)?.is_some_and(|tw| tw.diff(&mine).is_empty())
                            });
                        // What the stop is beside the twin at its frame, where the click left another picture in the
                        // same room, with ego where it stood and the same scripts taking the clicks
                        let (mut page, mut plain, mut begun) = (false, false, false);
                        if r.iface != m.iface
                            && r.room == r0.room
                            && ob.ego_pos == mego
                            && r.facts.val(Key::var(afford::VAR_VERB_SCRIPT as u64)) as i32 == input0
                            && r.facts.val(Key::var(afford::VAR_SENTENCE_SCRIPT as u64)) as i32 == sentence
                        {
                            if let Some(tw) = twins.at(twin, r.st.frame)? {
                                let mine = Facts(r.facts.iter().filter(|p| world_kind(p.0)).copied().collect());
                                if tw.0.diff(&mine).is_empty() {
                                    let now: HashSet<(i32, i32, String)> = ob.verbs.iter().map(|x| (x.slot, x.id, x.name.clone().unwrap_or_default())).collect();
                                    let mut slots: Vec<i32> = now.symmetric_difference(&shown).map(|x| x.0).collect();
                                    slots.sort_unstable();
                                    slots.dedup();
                                    page = slots.len() >= 2 && tw.2 == scripts_of(&ob);
                                    plain = slots.len() == 1 && tw.1 == places_of(&ob);
                                    if page || plain {
                                        out.own_slots.extend(slots.iter().copied());
                                    }
                                    // (the one slot shows what it does not show at the round's stop: a sentence that
                                    // waits for more, not one that ran and left the line as it was)
                                    begun = plain && now.iter().any(|x| x.0 == slots[0] && !shown0.contains(x));
                                }
                            }
                        }
                        r.pic = page || plain;
                        out.phase[3] += t.elapsed().as_secs_f64();
                        if half {
                            // another page of the interface, as one that showed at once; it only took longer than
                            // LEAD frames
                            if page {
                                trace(&format!("    shows {} depth {depth} (a page, settled)", r.name));
                                let before = match &m.only {
                                    Some(b) => Arc::new(b.union(&shown_all).cloned().collect()),
                                    None => shown_all.clone(),
                                };
                                let cand = MidCand { base: mkey(&r), engine: b, clicks: cl, iface: r.iface, events: r.events, paged: m.paged, inside: m.inside };
                                acts.push(Act::Shows { hit: p.hit, page: true, cand, before });
                                return Ok(PointOut { out, acts, learned, first });
                            }
                            if matches!(p.hit, Hit::Verb { .. }) && m.only.is_none() {
                                if depth == 1 {
                                    acts.push(Act::Gave(p.hit, r.iface));
                                } else if learn && (first_gave.get(&p.hit) == Some(&r.iface) || r.iface == m.iface) {
                                    learned.push(p.hit);
                                }
                            }
                            // a stop to click on from. The game ran on before it took a click again (a dialog's
                            // line, not a verb's slot): a stop like any other, too (but for its interface alone
                            // where it is a sentence half made, `pic`: the round clicks on from it, and as a stop
                            // of its own it would be the same clicks made again)
                            let cand = MidCand { base: mkey(&r), engine: b, clicks: cl, iface: r.iface, events: r.events.clone(), paged: m.paged, inside: m.inside };
                            acts.push(Act::Half { cand, only: None, always: true });
                            out.results.push(slim(r, base));
                            return Ok(PointOut { out, acts, learned, first });
                        } else {
                            // a sentence half made that neither test saw (a room's object took the verb, and the
                            // sentence waits for another): in the stop's own round a stop to click on from, like
                            // one that showed at once
                            if begun && motion.is_some() {
                                let before = match &m.only {
                                    Some(b) => Arc::new(b.union(&shown_all).cloned().collect()),
                                    None => shown_all.clone(),
                                };
                                let cand = MidCand { base: mkey(&r), engine: b, clicks: cl, iface: r.iface, events: r.events.clone(), paged: m.paged, inside: m.inside };
                                acts.push(Act::Shows { hit: p.hit, page: false, cand, before });
                                out.results.push(slim(r, base));
                                return Ok(PointOut { out, acts, learned, first });
                            }
                            // what the input changed: the stop against the twin at its frame (each with its old value)
                            let mut touched: Vec<(Key, Val)> = Vec::new();
                            if r.room == r0.room && motion.is_some() {
                                if let Some(tw) = twins.world(twin, r.st.frame)? {
                                    let mine = Facts(r.facts.iter().filter(|p| world_kind(p.0)).copied().collect());
                                    touched = tw
                                        .diff(&mine)
                                        .into_iter()
                                        .filter(|d| !motion.is_some_and(|m| m.contains(&d.0)) && !snap.noise.contains(&d.0))
                                        .map(|d| (d.0, r0.facts.val(d.0)))
                                        .collect();
                                }
                            }
                            if !touched.is_empty() {
                                r.own = touched.clone();
                            }
                            // --runs-on: a script the input started runs on at the stop (one that is not running in
                            // the twin there, and not the scripts that take the clicks): what it does after the stop
                            // is the input's too, and it is read PROBE frames on as well. (A sentence's script that
                            // plays out over 20 frames while the game takes clicks: its count, written at the end,
                            // was no stop's.)
                            let runs_on = cfg.runs_on && touched.is_empty() && r.room == r0.room && motion.is_some() && {
                                let mine = running(&ob);
                                twins.at(twin, r.st.frame)?.is_some_and(|tw| mine.iter().any(|x| !tw.3.contains(x) && x.1 != input0 && x.1 != sentence && x.1 != r.facts.val(Key::var(afford::VAR_VERB_SCRIPT as u64)) as i32))
                            };
                            // the keys back at their old value PROBE frames on, or PROBE_LONG frames on
                            let mut back: Vec<Key> = Vec::new();
                            if !touched.is_empty() || runs_on {
                                let mut pb = b.fork()?;
                                out.trials += 1;
                                let at = pb.status().frame;
                                if go(&mut pb, RunSpec { frame: Some(at + PROBE), ..Default::default() })?.is_ok() {
                                    if let Played::Stop(st2) = calm(&mut pb, at + PROBE, cfg.horizon, ego)? {
                                        let ob2 = pb.obs(&NAMES)?;
                                        let mut r2 = read(format!("{}~", r.name), r.events.clone(), &ob2, st2);
                                        r2.own = touched.clone();
                                        back = touched.iter().filter(|&&(k, old)| r2.facts.val(k) == old).map(|t| t.0).collect();
                                        let room2 = r2.room;
                                        out.results.push(slim(r2, base));
                                        if !touched.is_empty() && back.is_empty() && room2 == r0.room && go(&mut pb, RunSpec { frame: Some(at + PROBE_LONG), ..Default::default() })?.is_ok() {
                                            let w3 = pb.facts()?;
                                            back = touched.iter().filter(|&&(k, old)| w3.val(k) == old).map(|t| t.0).collect();
                                        }
                                    }
                                }
                                pb.quit();
                            }
                            if !back.is_empty() {
                                // a window, while one of its keys was followed from this cell fewer than
                                // WINDOW_TRIES times: for the stop's expansion to follow (the round's first
                                // MAX_WINDOWS of them)
                                let c = cell(r.room, r.iface, &snap.policy.world_of(&r.facts), snap.ambient);
                                if back.iter().any(|&k| snap.wins.get(&(c, k)).copied().unwrap_or(0) < WINDOW_TRIES) {
                                    acts.push(Act::Window(Cand { engine: b, name: format!("{}>", r.name), events: r.events.clone(), keys: touched.clone(), cell: c, back }));
                                    out.results.push(slim(r, base));
                                    return Ok(PointOut { out, acts, learned, first });
                                }
                            }
                            out.results.push(slim(r, base));
                        }
                    }
                    Played::Ended(st) => out.ends.push(format!("{} {}", name_of(pre, &cl), st.reason)),
                    Played::NotIdle => {
                        out.not_idle += 1;
                        if cfg.ends {
                            // (named with a '!': no stop of the game's, and none to click on from. The sentence
                            // that ends a game is such a trial: after it the game plays on by itself for good)
                            let ob = b.obs(&NAMES)?;
                            let st = b.status().clone();
                            let mut r = read(format!("{}!", name_of(pre, &cl)), events, &ob, st);
                            r.inside = !pre.is_empty();
                            out.results.push(slim(r, base));
                        }
                    }
                }
                let t = Instant::now();
                b.quit();
                out.phase[4] += t.elapsed().as_secs_f64();
                Ok(PointOut { out, acts, learned, first })
            };
            let outs: Vec<EResult<PointOut>> = if inner <= 1 || points.len() < 2 * inner {
                let mut v = Vec::with_capacity(points.len());
                for p in points.iter() {
                    let r = point(me, p, &mut twin);
                    let bad = r.is_err();
                    v.push(r);
                    if bad {
                        break;
                    }
                }
                v
            } else {
                let mut copies: Vec<Mutex<Option<Box<dyn Engine>>>> = Vec::new();
                for _ in 0..inner {
                    copies.push(Mutex::new(Some(me.fork()?)));
                }
                let slots: Vec<Mutex<Option<EResult<PointOut>>>> = points.iter().map(|_| Mutex::new(None)).collect();
                let at = AtomicUsize::new(0);
                std::thread::scope(|sc| {
                    for c in &copies {
                        sc.spawn(|| {
                            let mut eng = c.lock().unwrap().take().expect("the thread's copy of the stop's engine");
                            let mut twin: Option<Box<dyn Engine>> = None;
                            loop {
                                let i = at.fetch_add(1, Ordering::Relaxed);
                                if i >= points.len() {
                                    break;
                                }
                                *slots[i].lock().unwrap() = Some(point(&mut eng, &points[i], &mut twin));
                            }
                            if let Some(t) = twin.take() {
                                t.quit();
                            }
                            eng.quit();
                        });
                    }
                });
                slots.into_iter().map(|x| x.into_inner().unwrap().expect("every click of the stop")).collect()
            };
            // in the clicks' order
            for po in outs {
                let po = po?;
                out.absorb(po.out);
                acts.extend(po.acts);
                learned.extend(po.learned);
                if let Some(f) = po.first {
                    *by_first.entry(f).or_default() += 1;
                }
            }
            if let Some(t) = twin.take() {
                t.quit();
            }
            Ok(MidOut { out, acts, learned: learn.then_some(learned), by_first })
        }
    };
    let threads = cfg.slices.max(1);
    let max_mids = cfg.max_mids;
    let named = |c: &[Point]| c.iter().map(|x| x.hit.name()).collect::<String>();
    for depth in 1..=MAX_CLICKS {
        let mut next: Vec<Mid> = Vec::new();
        let mut work: VecDeque<Mid> = frontier.into_iter().collect();
        while !work.is_empty() {
            // side by side: the stops whose clicks no stop before them can change. At the first depth each page comes
            // of the stop before it; at the second the first stop teaches which slots to skip
            let n = if threads == 1 || depth == 1 || (depth == 2 && skip.is_none()) { 1 } else { work.len() };
            let (group, mut engines): (Vec<MidAt>, Vec<Option<Box<dyn Engine>>>) =
                work.drain(..n).map(|m| (MidAt { clicks: m.clicks, events: m.events, iface: m.iface, only: m.only, paged: m.paged, inside: m.inside }, m.engine)).unzip();
            let tg = Instant::now();
            let outs: Vec<EResult<MidOut>> = if n == 1 {
                let r = match engines[0].as_mut() {
                    Some(x) => task(x, &group[0], depth, threads, &skip, &shifters, &first_gave),
                    None => task(&mut *e, &group[0], depth, threads, &skip, &shifters, &first_gave),
                };
                vec![r]
            } else {
                let slots: Vec<Mutex<Option<Box<dyn Engine>>>> = engines.drain(..).map(Mutex::new).collect();
                let res: Vec<Mutex<Option<EResult<MidOut>>>> = (0..n).map(|_| Mutex::new(None)).collect();
                let at = AtomicUsize::new(0);
                std::thread::scope(|sc| {
                    for _ in 0..threads.min(n) {
                        sc.spawn(|| loop {
                            let i = at.fetch_add(1, Ordering::Relaxed);
                            if i >= n {
                                break;
                            }
                            let mut eng = slots[i].lock().unwrap().take().expect("a half-made stop's engine");
                            let r = task(&mut eng, &group[i], depth, 1, &skip, &shifters, &first_gave);
                            eng.quit();
                            *res[i].lock().unwrap() = Some(r);
                        });
                    }
                });
                res.into_iter().map(|x| x.into_inner().unwrap().expect("every stop of the group")).collect()
            };
            for x in engines.drain(..).flatten() {
                x.quit();
            }
            out.xt[if n == 1 { 5 } else { 6 }] += tg.elapsed().as_secs_f64();
            // in the round's order: what each stop left for the round
            let mut bad: Option<EngineError> = None;
            for mo in outs {
                let mo = match mo {
                    Ok(mo) if bad.is_none() => mo,
                    Ok(mo) => {
                        for a in mo.acts {
                            match a {
                                Act::Shows { cand, .. } | Act::Half { cand, .. } => cand.engine.quit(),
                                Act::Window(c) => c.engine.quit(),
                                Act::Gave(..) => {}
                            }
                        }
                        continue;
                    }
                    Err(x) => {
                        bad = bad.or(Some(x));
                        continue;
                    }
                };
                for (k, v) in mo.by_first {
                    *by_first.entry(k).or_default() += v;
                }
                for a in mo.acts {
                    match a {
                        Act::Gave(hit, i) => {
                            first_gave.insert(hit, i);
                        }
                        Act::Shows { hit, page, cand, before } => {
                            // a page of the interface from the round's stop: clicked on from at the same depth, at
                            // the slots that changed. Else the start or the middle of a sentence
                            if page && (depth == 1 || cand.inside) {
                                if depth == 1 {
                                    shifters.insert(hit);
                                }
                                if seen_mid.insert(cand.base) && mids_here[1] < max_mids {
                                    mids_here[1] += 1;
                                    out.mids += 1;
                                    trace(&format!("    mid {} depth {depth} page", named(&cand.clicks)));
                                    work.push_back(Mid { engine: Some(cand.engine), clicks: cand.clicks, iface: cand.iface, events: cand.events, only: Some(before), paged: true, inside: cand.inside });
                                } else {
                                    cand.engine.quit();
                                }
                            } else if !page && depth < MAX_CLICKS && seen_mid.insert(cand.base) && mids_here[cand.paged as usize] < max_mids {
                                mids_here[cand.paged as usize] += 1;
                                out.mids += 1;
                                trace(&format!("    mid {} depth {depth} shows", named(&cand.clicks)));
                                next.push(Mid { engine: Some(cand.engine), clicks: cand.clicks, iface: cand.iface, events: cand.events, only: None, paged: cand.paged, inside: false });
                            } else {
                                cand.engine.quit();
                            }
                        }
                        Act::Half { cand, only, always } => {
                            let fresh = (always || depth < MAX_CLICKS) && seen_mid.insert(cand.base);
                            if depth < MAX_CLICKS && fresh && mids_here[cand.paged as usize] < max_mids {
                                mids_here[cand.paged as usize] += 1;
                                out.mids += 1;
                                trace(&format!("    mid {} depth {depth} {}", named(&cand.clicks), if always { "settled" } else { "half" }));
                                next.push(Mid { engine: Some(cand.engine), clicks: cand.clicks, iface: cand.iface, events: cand.events, only, paged: cand.paged, inside: depth >= 2 && cfg.cross_pages });
                            } else {
                                cand.engine.quit();
                            }
                        }
                        Act::Window(c) => {
                            if found < MAX_WINDOWS {
                                found += 1;
                                out.cands.push(c);
                            } else {
                                c.engine.quit();
                            }
                        }
                    }
                }
                if let Some(l) = mo.learned {
                    skip = Some(l);
                }
                out.absorb(mo.out);
            }
            if let Some(x) = bad {
                return Err(x);
            }
        }
        frontier = next;
    }
    for mut m in frontier {
        if let Some(x) = m.engine.take() {
            x.quit();
        }
    }
    if let Some(t) = twins.src.lock().unwrap().take() {
        t.quit();
    }
    if tracing && !by_first.is_empty() {
        let mut v: Vec<(String, u64)> = by_first.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let all: u64 = v.iter().map(|x| x.1).sum();
        trace(&format!("    {pre}: {all} trials by first click: {}", v.iter().take(12).map(|x| format!("{} {}", x.0, x.1)).collect::<Vec<_>>().join(", ")));
    }
    Ok(())
}

/// One round of inputs from the engine's stop (`clicks`). Its half-made stops are worked on by `cfg.slices` threads
/// side by side and read in the round's order, so a round is the same every time and with any number of threads.
fn round(e: &mut Box<dyn Engine>, pre: &str, lead: &[Event], motion: Option<&HashSet<Key>>, carry: &[(Key, Val)], lown: &HashSet<u64>, base: &Facts, snap: &Snap<'_>, cfg: &Cfg) -> EResult<Out> {
    let mut out = Out::default();
    match clicks(e, pre, lead, motion, carry, lown, base, snap, cfg, &mut out) {
        Ok(()) => Ok(out),
        Err(x) => {
            for c in out.cands {
                c.engine.quit();
            }
            Err(x)
        }
    }
}

/// Where the game's scripts name an object or test a flag: per object and per flag, the rooms whose scripts do (the
/// scripts' dump, FMI_GAME_SCRIPTS: one folder a room, one file a script). Advice on where to go with what was
/// won, never taken as true: a room that is named and holds nothing costs one stop.
#[derive(Default)]
struct Mentions {
    objs: HashMap<u32, Vec<i32>>,
    bits: HashMap<u32, Vec<i32>>,
    /// per variable the scripts of a few rooms count on (add to or take from; GLOBAL_ROOMS rooms at most: one
    /// counted on in every room is a script's scratch), the rooms whose own scripts read it
    vars: HashMap<u32, Vec<i32>>,
    /// (from room, to room, object): the object's own code leads ego from its room to the other
    exits: Vec<(i32, i32, i32)>,
}

/// The integers that follow each `pat` in `line`.
fn nums_after(line: &str, pat: &str) -> Vec<u32> {
    let mut v = Vec::new();
    let mut at = 0;
    while let Some(i) = line[at..].find(pat) {
        let from = at + i + pat.len();
        let digits: String = line[from..].chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(n) = digits.parse() {
            v.push(n);
        }
        at = from;
    }
    v
}

/// The objects a script's text names (an owner asked for, a second object compared, an object's code started, a
/// class asked for or set, a sentence's objects), the flags it tests (a flag written is no test), the variables
/// it reads and those it counts on (adds to or takes from; one set to a value is neither).
fn names_of(text: &str) -> (Vec<u32>, Vec<u32>, Vec<u32>, Vec<u32>) {
    let (mut objs, mut bits, mut vars, mut counted) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for line in text.lines().filter(|l| !l.starts_with('#')) {
        for pat in ["getObjectOwner(", "startObject(", "classOfIs(", "setClass(", "pickupObject("] {
            objs.extend(nums_after(line, pat));
        }
        // `Local[n] == N`: the sentence's other object, as an object's own code asks for it
        let mut at = 0;
        while let Some(i) = line[at..].find("Local[") {
            let from = at + i + 6;
            let rest = line[from..].trim_start_matches(|c: char| c.is_ascii_digit());
            if let Some(r) = rest.strip_prefix("] == ") {
                let digits: String = r.chars().take_while(|c| c.is_ascii_digit()).collect();
                if let Ok(n) = digits.parse() {
                    objs.push(n);
                }
            }
            at = from;
        }
        if let Some(i) = line.find("doSentence(") {
            let args = line[i + 11..].split(')').next().unwrap_or("");
            objs.extend(args.split(',').skip(1).filter_map(|a| a.trim().parse::<u32>().ok()));
        }
        let mut at = 0;
        while let Some(i) = line[at..].find("Bit[") {
            let from = at + i + 4;
            let digits: String = line[from..].chars().take_while(|c| c.is_ascii_digit()).collect();
            let after = &line[from + digits.len()..];
            if let (Ok(n), true) = (digits.parse(), after.starts_with(']') && !after.starts_with("] = ")) {
                bits.push(n);
            }
            at = from;
        }
        let mut at = 0;
        while let Some(i) = line[at..].find("Var[") {
            let from = at + i + 4;
            let digits: String = line[from..].chars().take_while(|c| c.is_ascii_digit()).collect();
            let after = &line[from + digits.len()..];
            if let (Ok(n), true) = (digits.parse::<u32>(), after.starts_with(']')) {
                if ["] += ", "] -= ", "]++", "]--"].iter().any(|w| after.starts_with(w)) {
                    counted.push(n);
                } else if !after.starts_with("] = ") {
                    vars.push(n);
                }
            }
            at = from;
        }
    }
    (objs, bits, vars, counted)
}

/// The mentions of a scripts' dump: a room's scripts' own, those of the global scripts few rooms start (GLOBAL_ROOMS
/// at most: a script every room starts says nothing of a room), and for an object the rooms of the objects its
/// own code names.
fn mentions_of(dir: &Path) -> Mentions {
    const GLOBAL_ROOMS: usize = 3;
    let mut m = Mentions::default();
    let num = |name: &str, pre: &str| -> Option<u32> { name.strip_prefix(pre)?.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok() };
    let mut home: HashMap<u32, i32> = HashMap::new();
    let mut own: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut globals: HashMap<u32, (Vec<u32>, Vec<u32>)> = HashMap::new();
    let (mut read, mut counts): (HashMap<u32, Vec<i32>>, HashMap<u32, Vec<i32>>) = (HashMap::new(), HashMap::new());
    let mut starts: HashMap<u32, Vec<i32>> = HashMap::new();
    let Ok(rd) = std::fs::read_dir(dir) else { return m };
    let mut folders: Vec<PathBuf> = rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()).collect();
    folders.sort();
    for folder in folders {
        let fname = folder.file_name().and_then(|x| x.to_str()).unwrap_or("").to_string();
        let room = num(&fname, "room-").map(|r| r as i32);
        let Ok(rd) = std::fs::read_dir(&folder) else { continue };
        let mut files: Vec<PathBuf> = rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
        files.sort();
        for f in files {
            let name = f.file_name().and_then(|x| x.to_str()).unwrap_or("").to_string();
            let Ok(text) = std::fs::read_to_string(&f) else { continue };
            let (objs, bits, vars, counted) = names_of(&text);
            match room {
                Some(r) => {
                    let me = num(&name, "obj-");
                    if let Some(o) = me {
                        // the rooms the object's own code leads ego to: a way the scripts advise
                        for l in text.lines().filter(|l| !l.starts_with('#')) {
                            if let Some(at) = l.find("loadRoomWithEgo(") {
                                let args: Vec<&str> = l[at + "loadRoomWithEgo(".len()..].split(',').collect();
                                if let Some(to) = args.get(1).and_then(|x| x.trim().parse::<i32>().ok()) {
                                    if to != r && !m.exits.contains(&(r, to, o as i32)) {
                                        m.exits.push((r, to, o as i32));
                                    }
                                }
                            }
                        }
                        home.insert(o, r);
                        own.entry(o).or_default().extend(objs.iter().copied().filter(|x| *x != o));
                    }
                    for o in objs.into_iter().filter(|x| Some(*x) != me) {
                        let v = m.objs.entry(o).or_default();
                        if !v.contains(&r) {
                            v.push(r);
                        }
                    }
                    for b in bits {
                        let v = m.bits.entry(b).or_default();
                        if !v.contains(&r) {
                            v.push(r);
                        }
                    }
                    for (list, into) in [(vars, &mut read), (counted, &mut counts)] {
                        for x in list {
                            let v = into.entry(x).or_default();
                            if !v.contains(&r) {
                                v.push(r);
                            }
                        }
                    }
                    for g in text.lines().filter(|l| !l.starts_with('#')).flat_map(|l| nums_after(l, "startScript(")) {
                        let v = starts.entry(g).or_default();
                        if !v.contains(&r) {
                            v.push(r);
                        }
                    }
                }
                None => {
                    if let Some(g) = num(&name, "script-") {
                        globals.insert(g, (objs, bits));
                    }
                }
            }
        }
    }
    for (g, (objs, bits)) in &globals {
        let Some(rooms) = starts.get(g).filter(|r| r.len() <= GLOBAL_ROOMS) else { continue };
        for &r in rooms {
            for o in objs {
                let v = m.objs.entry(*o).or_default();
                if !v.contains(&r) {
                    v.push(r);
                }
            }
            for b in bits {
                let v = m.bits.entry(*b).or_default();
                if !v.contains(&r) {
                    v.push(r);
                }
            }
        }
    }
    for (x, rooms) in counts {
        if rooms.len() <= GLOBAL_ROOMS {
            if let Some(r) = read.remove(&x) {
                m.vars.insert(x, r);
            }
        }
    }
    for (o, others) in &own {
        for r in others.iter().filter_map(|x| home.get(x)) {
            let v = m.objs.entry(*o).or_default();
            if !v.contains(r) {
                v.push(*r);
            }
        }
    }
    for v in m.objs.values_mut().chain(m.bits.values_mut()).chain(m.vars.values_mut()) {
        v.sort_unstable();
    }
    m
}

/// The clicks of a stop's name (`v<verb>`, `t<object>`, `t-<actor>`, `w<floor point>`); None where the name holds
/// more than clicks (a wait, a window, a walk of the search's own).
fn hits_of(name: &str) -> Option<Vec<Hit>> {
    let name = name.strip_suffix('~').unwrap_or(name);
    let mut v = Vec::new();
    let b = name.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let kind = b[i];
        i += 1;
        let neg = i < b.len() && b[i] == b'-';
        if neg {
            i += 1;
        }
        let from = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        let n: i32 = name[from..i].parse().ok()?;
        v.push(match (kind, neg) {
            (b'v', false) => Hit::Verb { slot: 0, id: n },
            (b't', false) => Hit::Object(n),
            (b't', true) => Hit::Actor(n),
            (b'w', false) => Hit::Walk(n),
            _ => return None,
        });
    }
    (!v.is_empty()).then_some(v)
}

/// The shortest way from room to room over the clicks that were seen to lead from one to the next.
fn way<'a>(edges: &'a HashMap<(i32, i32), Vec<Hit>>, from: i32, to: i32) -> Option<Vec<(i32, &'a Vec<Hit>)>> {
    let mut prev: HashMap<i32, i32> = HashMap::from([(from, from)]);
    let mut queue = VecDeque::from([from]);
    while let Some(a) = queue.pop_front() {
        if a == to {
            break;
        }
        let mut next: Vec<i32> = edges.keys().filter(|k| k.0 == a && !prev.contains_key(&k.1)).map(|k| k.1).collect();
        next.sort_unstable();
        for b in next {
            prev.insert(b, a);
            queue.push_back(b);
        }
    }
    let mut path = Vec::new();
    let mut at = to;
    while at != from {
        let a = *prev.get(&at)?;
        path.push((at, edges.get(&(a, at))?));
        at = a;
    }
    path.reverse();
    (!path.is_empty() && path.len() <= GO_HOPS).then_some(path)
}

/// A walk from the engine's stop along `path`, each room's clicks as they were seen to lead on: the stop at its
/// end. Where a click finds nothing to click or a room is not the one expected (the world is another now, or the
/// way asks for something first), the stop where the walk stands, if it left the room it began in: what the way
/// asks for is asked there. None where it did not leave, or stands at no stop.
fn walk(e: &mut Box<dyn Engine>, path: &[(i32, &Vec<Hit>)], ego: i32, cfg: &Cfg, bad: &mut Vec<(i32, i32)>) -> EResult<Option<Res>> {
    let mut cur = e.fork()?;
    let mut events: Vec<Event> = Vec::new();
    let mut last = cur.status().clone();
    let room0 = last.room;
    // the walk's stop short of its end
    let short = |mut cur: Box<dyn Engine>, events: Vec<Event>, st: Status| -> EResult<Option<Res>> {
        if st.room == room0 || st.idle.is_none() {
            cur.quit();
            return Ok(None);
        }
        let ob = cur.obs(&NAMES)?;
        let r = read(format!("go{}", st.room), events, &ob, st);
        cur.quit();
        Ok(Some(r))
    };
    'hops: for (room, hits) in path {
        let mut from = cur.status().frame;
        let here = cur.status().room;
        for (k, hit) in hits.iter().enumerate() {
            // a click at the screen's edge that left the room: the stop it came to
            let mut left: Option<Status> = None;
            // the click's point; a thing of the room that is off the screen (a room wider than it) is walked
            // towards first, floor click by floor click at the screen's edge on its side, until it shows
            let mut point = None;
            for _ in 0..GO_SCROLLS {
                let o = cur.obs(&NAMES)?;
                let of = afford::offer_cfg(&mut cur, &o, false, true)?;
                let (top, h) = cur.status().screen.map(|s| (s.0, s.1)).unwrap_or((0, afford::SCREEN_H));
                let grid: Vec<(i32, i32)> =
                    FLOOR_Y.iter().flat_map(|&k| FLOOR_X.iter().map(move |&x| (x, (top + (h - top) * k / 8).min(h - 2)))).collect();
                point = match *hit {
                    Hit::Walk(i) => grid.get(i as usize).copied(),
                    Hit::Verb { id, .. } => of.verbs.iter().find(|p| matches!(p.hit, Hit::Verb { id: i, .. } if i == id)).map(|p| (p.x, p.y)),
                    other => of.targets.iter().find(|p| p.hit == other).map(|p| (p.x, p.y)),
                };
                if point.is_some() {
                    break;
                }
                // where the thing is in the room, against what the screen shows
                let (x0, x1) = (cur.status().xstart, cur.status().xstart + afford::SCREEN_W);
                let at = match *hit {
                    Hit::Object(id) => o.room_objects.as_deref().unwrap_or(&[]).iter().find(|r| r.id == id).map(|r| r.x + r.w / 2),
                    Hit::Actor(id) => o.actors.iter().find(|a| a.id == id).map(|a| a.x),
                    _ => None,
                };
                let Some(at) = at.filter(|x| *x < x0 || *x >= x1) else {
                    trace(&format!("    walk: {} is not in the room's list, or on the screen and not offered (screen {x0}..{x1}, at {at:?})", hit.name()));
                    break;
                };
                // the points at the screen's edge on that side, the outermost column first and in it the lowest
                // first: a click there walks ego that way (to a thing, if one lies there), and the room scrolls
                // once ego is near the screen's edge
                let cols: Vec<usize> = if at < x0 { (0..FLOOR_X.len()).collect() } else { (0..FLOOR_X.len()).rev().collect() };
                let ats = cur.at_many(&grid)?;
                let is_floor = |i: usize| ats.get(i).is_some_and(|a| a.main.is_some_and(|m| m.2 == 0 && m.3 == 0));
                // (only points of the room's picture: below it lie the verbs)
                let mut cands: Vec<usize> = cols
                    .iter()
                    .take(3)
                    .flat_map(|&c| (0..FLOOR_Y.len()).rev().map(move |r| r * FLOOR_X.len() + c))
                    .filter(|&i| ats.get(i).is_some_and(|a| a.main.is_some()))
                    .collect();
                cands.truncate(FLOOR_Y.len());
                let before = cur.status().xstart;
                let mut moved = false;
                for &i in cands.iter().take(4) {
                    let f = cur.status().frame;
                    let ev = Event { frame: f + 1, what: Input::Click(grid[i].0, grid[i].1) };
                    if cur.inputs(std::slice::from_ref(&ev)).is_err() || go(&mut cur, RunSpec { frame: Some(f + LEAD + CALM), ..Default::default() })?.is_err() {
                        break;
                    }
                    events.push(ev);
                    match calm(&mut cur, f, cfg.horizon, ego)? {
                        Played::Stop(st) if st.room != here => {
                            left = Some(st);
                            break;
                        }
                        Played::Stop(_) => {}
                        _ => break,
                    }
                    // the room goes on scrolling after ego stands: on until it rests
                    for _ in 0..40 {
                        let x = cur.status().xstart;
                        let f = cur.status().frame;
                        if go(&mut cur, RunSpec { frame: Some(f + 8), ..Default::default() })?.is_err() || cur.status().xstart == x {
                            break;
                        }
                    }
                    trace(&format!("    walk: room {} click at {:?} (floor {}) -> xstart {} ego {:?} (of {} points in the picture)", cur.status().room, grid[i], is_floor(i), cur.status().xstart, cur.status().ego, cands.len()));
                    if cur.status().xstart != before {
                        moved = true;
                        break;
                    }
                }
                if left.is_some() {
                    break;
                }
                if !moved {
                    trace(&format!("    walk: no click at the screen's edge moved the room towards {} (screen {x0}..{x1}, at {at})", hit.name()));
                    break;
                }
            }
            if let Some(st) = left {
                // the way on was the screen's edge itself: the hop is made if the room is the one expected
                if st.room == *room {
                    trace(&format!("    walk: the screen's edge led from room {here} to {room}"));
                    last = st;
                    continue 'hops;
                }
                trace(&format!("    walk: the screen's edge led from room {here} to {} not {room}", st.room));
                return short(cur, events, st);
            }
            let Some((x, y)) = point else {
                trace(&format!("    walk: in room {} nothing to click for {} (on the way to {room})", cur.status().room, hit.name()));
                let st = cur.status().clone();
                return short(cur, events, st);
            };
            let park = park_point(&mut cur)?;
            from = cur.status().frame;
            let mut evs = vec![Event { frame: from + 1, what: Input::Click(x, y) }];
            if let Some((px, py)) = park {
                evs.push(Event { frame: from + 2, what: Input::Move(px, py) });
            }
            if cur.inputs(&evs).is_err() {
                cur.quit();
                return Ok(None);
            }
            events.extend(evs);
            if k + 1 < hits.len() && go(&mut cur, RunSpec { frame: Some(from + LEAD + CALM), ..Default::default() })?.is_err() {
                cur.quit();
                return Ok(None);
            }
        }
        if go(&mut cur, RunSpec { frame: Some(from + LEAD + CALM), ..Default::default() })?.is_err() {
            cur.quit();
            return Ok(None);
        }
        match calm(&mut cur, from, cfg.horizon, ego)? {
            Played::Stop(st) if st.room == *room => last = st,
            other => {
                // the clicks were made and did not lead on: the way is false (or the world another)
                bad.push((here, *room));
                trace(&format!("    walk: {} led to room {} not {room} ({})", hits.iter().map(|h| h.name()).collect::<String>(), cur.status().room, match other { Played::Stop(_) => "a stop", Played::Ended(_) => "the end", Played::NotIdle => "no stop" }));
                return match other {
                    Played::Stop(st) => short(cur, events, st),
                    _ => {
                        cur.quit();
                        Ok(None)
                    }
                };
            }
        }
    }
    let ob = cur.obs(&NAMES)?;
    let r = read(format!("go{}", last.room), events, &ob, last);
    cur.quit();
    Ok(Some(r))
}

/// The hits clicked from the engine's stop, one after the other (a hit's point as the stop offers it), and the stop
/// the game comes to (with `on`: the stop PROBE frames after that one); None where a hit has no point there or the
/// game comes to no stop.
fn again(cur: &mut Box<dyn Engine>, hits: &[Hit], ego: i32, cfg: &Cfg, on: bool) -> EResult<Option<(Vec<Event>, Status)>> {
    let mut events: Vec<Event> = Vec::new();
    let mut from = cur.status().frame;
    for (k, hit) in hits.iter().enumerate() {
        let o = cur.obs(&NAMES)?;
        let of = afford::offer_cfg(cur, &o, false, true)?;
        let point = match *hit {
            Hit::Verb { id, .. } => of.verbs.iter().find(|p| matches!(p.hit, Hit::Verb { id: i, .. } if i == id)).map(|p| (p.x, p.y)),
            Hit::Walk(_) => None,
            other => of.targets.iter().find(|p| p.hit == other).map(|p| (p.x, p.y)),
        };
        let Some((x, y)) = point else { return Ok(None) };
        let park = park_point(cur)?;
        from = cur.status().frame;
        let mut evs = vec![Event { frame: from + 1, what: Input::Click(x, y) }];
        if let Some((px, py)) = park {
            evs.push(Event { frame: from + 2, what: Input::Move(px, py) });
        }
        if cur.inputs(&evs).is_err() {
            return Ok(None);
        }
        events.extend(evs);
        if k + 1 < hits.len() && go(cur, RunSpec { frame: Some(from + LEAD + CALM), ..Default::default() })?.is_err() {
            return Ok(None);
        }
    }
    if go(cur, RunSpec { frame: Some(from + LEAD + CALM), ..Default::default() })?.is_err() {
        return Ok(None);
    }
    let st = match calm(cur, from, cfg.horizon, ego)? {
        Played::Stop(st) => st,
        _ => return Ok(None),
    };
    if !on {
        return Ok(Some((events, st)));
    }
    // and PROBE frames on with no input, as a round reads a sentence's second stop
    let at = st.frame;
    if go(cur, RunSpec { frame: Some(at + PROBE), ..Default::default() })?.is_err() {
        return Ok(None);
    }
    match calm(cur, at + PROBE, cfg.horizon, ego)? {
        Played::Stop(st) => Ok(Some((events, st))),
        _ => Ok(None),
    }
}

/// What `explore` reads of the search: the rooms' motion at the batch's start.
struct Snap<'a> {
    motion: &'a HashMap<(u32, i32, i32, Key), u32>,
    wins: &'a HashMap<(u64, Key), u32>,
    noise: &'a HashSet<Key>,
    lclocks: &'a HashSet<u64>,
    policy: &'a Policy,
    ambient: &'a HashSet<Key>,
    mentions: &'a Mentions,
    edges: &'a HashMap<(i32, i32), Vec<Hit>>,
    /// --go
    go: bool,
}

/// Every input of the stop tried, at the stop and after the game's own moves. Reads the engine and `snap` only.
fn explore(e: &mut Box<dyn Engine>, level: u32, pending: &[(Key, Val)], inside: bool, news: &[Key], snap: &Snap<'_>, cfg: &Cfg) -> EResult<Out> {
    let o0 = e.obs(&NAMES)?;
    let st0 = e.status().clone();
    let r0 = read(String::new(), Vec::new(), &o0, st0.clone());
    let mut out = Out { room: r0.room, walkbox: r0.walkbox, base: r0.facts.clone(), ..Default::default() };
    let ego = r0.facts.val(Key::pack(K_EGO, 0, 0, 0)) as i32;
    // the game's own moves: no input for a while. What they move is the stop's own motion; a variable that takes two
    // values or more over them is a clock
    let known = |k: Key| snap.motion.get(&(level, r0.room, r0.walkbox, k)).is_some_and(|&c| c >= MOTION_TRIES);
    let mut waited: Vec<(u64, Box<dyn Engine>)> = Vec::new();
    let mut values: HashMap<Key, Vec<Val>> = HashMap::new();
    // the worlds the stop's waits came to (what differs from the stop's): the inputs are tried again from each
    // once, not after every wait that comes to it (a room that turns between two states comes to them again)
    let mut came_to: HashSet<Vec<(Key, Val)>> = HashSet::new();
    let tx = Instant::now();
    for n in WAITS {
        let mut b = e.fork()?;
        out.trials += 1;
        match go(&mut b, RunSpec { frame: Some(st0.frame + n), ..Default::default() })? {
            Err(st) => out.ends.push(format!("wait{n} {}", st.reason)),
            Ok(_) => match calm(&mut b, st0.frame + n, cfg.horizon, ego)? {
                Played::Stop(st) => {
                    let ob = b.obs(&NAMES)?;
                    let r = read(format!("wait{n}"), Vec::new(), &ob, st);
                    let mut moved = false;
                    let mut to: Vec<(Key, Val)> = Vec::new();
                    for (k, _, now) in r0.facts.diff(&r.facts) {
                        if k.kind() == K_VAR {
                            let v = values.entry(k).or_default();
                            if !v.contains(&now) {
                                v.push(now);
                            }
                        } else if world_kind(k) {
                            out.ambient.insert(k);
                            moved |= !known(k);
                            to.push((k, now));
                        }
                    }
                    out.results.push(slim(r, &r0.facts));
                    if moved && came_to.insert(to) {
                        waited.push((n, b));
                        continue;
                    }
                }
                Played::Ended(st) => out.ends.push(format!("wait{n} {}", st.reason)),
                Played::NotIdle => {
                    out.not_idle += 1;
                    if cfg.ends {
                        let ob = b.obs(&NAMES)?;
                        let st = b.status().clone();
                        out.results.push(slim(read(format!("wait{n}!"), Vec::new(), &ob, st), &r0.facts));
                    }
                }
            },
        }
        b.quit();
    }
    let moving: HashSet<Key> = values.keys().copied().collect();
    out.clocks.extend(values.into_iter().filter(|x| x.1.len() >= 2).map(|x| x.0));
    // what the stop's own waits moved: no input's doing
    let motion = out.ambient.clone();
    // and the game's own motion, where no input's change of the key was pending
    if !inside {
        out.noise = out.ambient.iter().filter(|k| !pending.iter().any(|p| p.0 == **k)).copied().collect();
    }
    trace(&format!("  room {} waits done, {} moved", r0.room, waited.len()));
    let quiet = waited.is_empty() && out.ambient.is_empty();
    out.xt[0] += tx.elapsed().as_secs_f64();
    let tx = Instant::now();
    // the scripts' locals the stop's own waits moved: its clocks, whatever the search knows of them yet
    let lown: HashSet<u64> = {
        let at0: HashMap<u64, i32> = r0.locals.iter().copied().collect();
        out.results.iter().flat_map(|r| r.locals.iter()).filter(|l| at0.get(&l.0) != Some(&l.1)).map(|l| l.0).collect()
    };
    let mut round0 = round(e, "", &[], Some(&motion), &[], &lown, &r0.facts, snap, cfg)?;
    let cands = std::mem::take(&mut round0.cands);
    let first = out.results.len();
    out.absorb(round0);
    let own_end = out.results.len();
    // A dead end: nothing the player does counts there, and the game does nothing by itself. (Where the interface
    // takes sentences a click on the floor walks and a verb's click waits for its object: one click that moves
    // nothing, and no second, is no game.) The game may have taken the click for itself (a scene that waits for one,
    // a text to click away), so the search gives one: the first of the stops the inputs came to is expanded next
    out.dead = quiet
        && st0.idle.as_deref() == Some("sentence")
        && own_end > first
        && out.results[first..own_end].iter().all(|r| {
            hits_of(&r.name).is_some_and(|h| h.len() == 1) && r.room == r0.room && r.st.ego == st0.ego && !r.delta.iter().any(|d| world_kind(d.0) && d.1 != d.2.unwrap_or(0))
        });
    if out.dead {
        trace(&format!("  room {}: a dead end", r0.room));
    }
    out.xt[1] += tx.elapsed().as_secs_f64();
    trace(&format!("  room {} round done: {} trials, {} windows found", r0.room, out.trials, cands.len()));
    // a variable a fifth of the stop's own inputs changed echoes the input: counted on the expanded stop's round
    // only (in a window's round, or after a wait, an earlier input's effect lands in every trial), over its first
    // stops (not those read PROBE frames on) that changed nothing else in the world (where a dialog has three
    // lines and one of them wins, a fifth of the inputs raise the count of what was won: that is no echo), and
    // never a variable the stop's waits moved
    {
        let made: Vec<&Res> = out.results[first..]
            .iter()
            .filter(|r| !r.name.ends_with('~') && !r.delta.iter().any(|d| world_kind(d.0) && d.1 != d.2.unwrap_or(0)))
            .collect();
        if made.len() >= 10 {
            let mut changed: HashMap<Key, usize> = HashMap::new();
            for r in &made {
                for &(k, old, new) in &r.delta {
                    if k.kind() == K_VAR && old != new.unwrap_or(0) && !moving.contains(&k) {
                        *changed.entry(k).or_default() += 1;
                    }
                }
            }
            let n = made.len();
            out.iface_vars.extend(changed.into_iter().filter(|&(_, c)| c * 5 >= n).map(|(k, _)| k));
        }
    }
    // the windows: the first MAX_WINDOWS whose cell has tries left; the inputs from each, with no window found
    // inside one
    let mut mine: HashMap<(u64, Key), u32> = HashMap::new();
    let mut bad = None;
    let tx = Instant::now();
    for c in cands {
        let mut b = c.engine;
        let fresh = c.back.iter().any(|&k| snap.wins.get(&(c.cell, k)).copied().unwrap_or(0) + mine.get(&(c.cell, k)).copied().unwrap_or(0) < WINDOW_TRIES);
        if bad.is_none() && fresh && (out.windows as usize) < MAX_WINDOWS {
            for &k in &c.back {
                *mine.entry((c.cell, k)).or_default() += 1;
                out.followed.push((c.cell, k));
            }
            out.windows += 1;
            trace(&format!("    window {} ({} trials so far)", c.name, out.trials));
            match round(&mut b, &c.name, &c.events, None, &c.keys, &lown, &r0.facts, snap, cfg) {
                Ok(o) => out.absorb(o),
                Err(x) => bad = Some(x),
            }
        }
        b.quit();
    }
    out.xt[2] += tx.elapsed().as_secs_f64();
    let tx = Instant::now();
    // where the world moved by itself (and few stops of this room and level saw that): the inputs again from there
    for (n, mut b) in waited {
        if bad.is_none() {
            match round(&mut b, &format!("wait{n}+"), &[], None, &[], &lown, &r0.facts, snap, cfg) {
                Ok(o) => out.absorb(o),
                Err(x) => bad = Some(x),
            }
        }
        b.quit();
    }
    if let Some(x) = bad {
        return Err(x);
    }
    out.xt[3] += tx.elapsed().as_secs_f64();
    let tx = Instant::now();
    // --go: the stop has won something (an object, a flag). Where the scripts name it, there it is asked for: a
    // walk to each such room over the clicks seen to lead from room to room, one trial a walk; the nearest room's
    // stop last, so that it is the first to be expanded. The walk through every room stays, for what the scripts
    // do not say
    if !news.is_empty() {
        let mut rooms: Vec<i32> = Vec::new();
        for k in news {
            let named = if k.kind() == K_BIT { snap.mentions.bits.get(&(k.a() as u32)) } else { snap.mentions.objs.get(&(k.a() as u32)) };
            for &r in named.into_iter().flatten() {
                if r != r0.room && !rooms.contains(&r) {
                    rooms.push(r);
                }
            }
        }
        let mut ways: Vec<Vec<(i32, &Vec<Hit>)>> = rooms.iter().filter_map(|&t| way(snap.edges, r0.room, t)).collect();
        ways.sort_by_key(|w| (w.len(), w.last().map(|x| x.0)));
        ways.truncate(GO_ROOMS);
        trace(&format!("  room {} go: rooms named {:?}, ways {:?}", r0.room, rooms, ways.iter().map(|w| w.iter().map(|x| format!("{}:{}", x.0, x.1.iter().map(|h| h.name()).collect::<String>())).collect::<Vec<_>>().join(" ")).collect::<Vec<_>>()));
        for w in ways.iter().rev() {
            out.trials += 1;
            if let Some(r) = walk(e, w, ego, cfg, &mut out.bad)? {
                out.results.push(slim(r, &r0.facts));
            }
        }
    }
    // --go: what the stop holds only for a while (an object ego owns that one of its own waits saw gone) is
    // taken at once as far as the ways lead: a walk to each of the nearest rooms, one trial a walk, and
    // the walk's stop is kept where it still holds it. A walk crosses a room wider than the screen; the round's
    // clicks reach only what the screen shows, and a stop on the way would wait its turn while the thing ran out
    if snap.go {
        let mut going: Vec<Key> = out
            .results
            .iter()
            .filter(|r| r.name.starts_with("wait") && !r.name.contains('+'))
            .flat_map(|r| r.delta.iter().filter(|x| x.0.kind() == K_OWNER && x.1 == ego as Val && x.2 != Some(ego as Val)).map(|x| x.0))
            .collect();
        going.sort_unstable();
        going.dedup();
        if !going.is_empty() {
            let mut rooms: Vec<i32> = snap.edges.keys().map(|k| k.1).filter(|&r| r != r0.room).collect();
            rooms.sort_unstable();
            rooms.dedup();
            let mut ways: Vec<Vec<(i32, &Vec<Hit>)>> = rooms.iter().filter_map(|&t| way(snap.edges, r0.room, t)).collect();
            ways.sort_by_key(|w| (w.len(), w.last().map(|x| x.0)));
            ways.truncate(GO_ROOMS);
            trace(&format!("  room {} holds {} keys only for a while: walks to {:?}", r0.room, going.len(), ways.iter().map(|w| w.last().map(|x| x.0)).collect::<Vec<_>>()));
            for w in ways.iter().rev() {
                out.trials += 1;
                if let Some(r) = walk(e, w, ego, cfg, &mut out.bad)? {
                    if going.iter().any(|&k| r.facts.val(k) == r0.facts.val(k)) {
                        out.results.push(slim(r, &r0.facts));
                    }
                }
            }
        }
    }
    out.xt[4] += tx.elapsed().as_secs_f64();
    // Counting in place. An input of the stop's own round that moved a guide variable few of the round's inputs
    // move (a count: the turns of a lock, how often a thing was asked) is made again from where it led, and
    // again, with those inputs alone: one trial an input, breadth first over the states of what they moved, each
    // state once. A stop of the round holds one value of a count, and as a stop of its own it waits its turn among
    // all that is new by a variable; the fourth turn of a lock is four such waits away. The stops it comes to are
    // results like any other
    {
        // (the round's stops in the stop's room, by one sentence, with the world's variables they moved and the
        // values: a sentence's first stop and the one read PROBE frames on, where a count is often written only
        // then)
        let mut cands: Vec<(String, Vec<Hit>, Vec<(Key, Val)>)> = Vec::new();
        // (not a click that left another picture and no more: the half of a sentence counts nothing)
        for r in out.results[first..own_end].iter().filter(|r| r.room == r0.room && !r.pic) {
            let name = r.name.trim_end_matches('~');
            let Some(hits) = hits_of(name) else { continue };
            if hits.iter().any(|h| matches!(h, Hit::Walk(_))) {
                continue;
            }
            let vars: Vec<(Key, Val)> =
                r.delta.iter().filter(|d| d.0.kind() == K_VAR && snap.policy.world(d.0) && d.1 != d.2.unwrap_or(0)).map(|d| (d.0, d.2.unwrap_or(0))).collect();
            if vars.is_empty() {
                continue;
            }
            match cands.iter_mut().find(|x| x.0 == name) {
                Some(x) => {
                    for v in vars {
                        match x.2.iter_mut().find(|y| y.0 == v.0) {
                            Some(y) => y.1 = v.1,
                            None => x.2.push(v),
                        }
                    }
                }
                None => cands.push((name.to_string(), hits, vars)),
            }
        }
        // a variable those sentences left at more than COUNT_FEW values is the sentence's own (its verb, its
        // objects): no count, and no part of what a sentence did
        let mut values: HashMap<Key, HashSet<Val>> = HashMap::new();
        for x in &cands {
            for v in &x.2 {
                values.entry(v.0).or_default().insert(v.1);
            }
        }
        for x in cands.iter_mut() {
            x.2.retain(|v| values[&v.0].len() <= COUNT_FEW);
            x.2.sort_unstable();
        }
        cands.retain(|x| !x.2.is_empty());
        // A count is a variable whose shortest movers (the sentences of the fewest clicks that moved it: a verb
        // with an object, not the same with every thing of the inventory before it) are at most COUNT_FEW, or did
        // at most COUNT_FEW different things. Those are the inputs counted with: each of the shortest movers where
        // they are few (two that do the same at this stop may differ at the next: a verb that turns the way the
        // last turn went), else the first of each thing done. An input counts with the variables it is a
        // shortest mover of, and no others
        let mut order: Vec<Key> = Vec::new();
        for x in &cands {
            for v in &x.2 {
                if !order.contains(&v.0) {
                    order.push(v.0);
                }
            }
        }
        let mut inputs: Vec<(String, Vec<Hit>, Vec<Key>)> = Vec::new();
        for &k in &order {
            let movers: Vec<&(String, Vec<Hit>, Vec<(Key, Val)>)> = cands.iter().filter(|x| x.2.iter().any(|v| v.0 == k)).collect();
            let least = movers.iter().map(|x| x.1.len()).min().unwrap_or(0);
            let mut reps: Vec<&(String, Vec<Hit>, Vec<(Key, Val)>)> = movers.into_iter().filter(|x| x.1.len() == least).collect();
            if reps.len() > COUNT_FEW {
                let mut did: HashSet<&Vec<(Key, Val)>> = HashSet::new();
                reps.retain(|x| did.insert(&x.2));
            }
            if reps.len() > COUNT_FEW {
                continue;
            }
            for x in reps {
                match inputs.iter_mut().find(|y| y.0 == x.0) {
                    Some(y) => y.2.push(k),
                    None => inputs.push((x.0.clone(), x.1.clone(), vec![k])),
                }
            }
        }
        // (the shortest sentences first: where a group has more inputs than are counted with, those stay)
        inputs.sort_by_key(|x| x.1.len());
        // the inputs that move the same counts are counted together (COUNT_GROUPS such groups at most, in the
        // round's order)
        let mut groups: Vec<(Vec<Key>, Vec<(String, Vec<Hit>)>)> = Vec::new();
        for x in inputs {
            match groups.iter_mut().find(|g| x.2.iter().any(|k| g.0.contains(k))) {
                Some(g) => {
                    for k in x.2 {
                        if !g.0.contains(&k) {
                            g.0.push(k);
                        }
                    }
                    g.1.push((x.0, x.1));
                }
                None => groups.push((x.2, vec![(x.0, x.1)])),
            }
        }
        groups.truncate(COUNT_GROUPS);
        for (vars, mut inputs) in groups {
            inputs.truncate(COUNT_INPUTS);
            // a state: the counts, and the world's other keys (no variables: those every sentence writes are not
            // the count's) that these inputs moved at their own stops
            let names: HashSet<&str> = inputs.iter().map(|x| x.0.as_str()).collect();
            let mut keys: Vec<Key> = out.results[first..own_end]
                .iter()
                .filter(|r| names.contains(r.name.trim_end_matches('~')))
                .flat_map(|r| r.delta.iter().filter(|d| world_kind(d.0) && d.1 != d.2.unwrap_or(0)).map(|d| d.0))
                .chain(vars.iter().copied())
                .collect();
            keys.sort_unstable();
            keys.dedup();
            let state = |f: &Facts| -> u64 {
                let mut h = DefaultHasher::new();
                for &k in &keys {
                    f.val(k).hash(&mut h);
                }
                h.finish()
            };
            // what the world is beside those keys (and beside what moves by itself): where counting changed that,
            // it led to something, and the stop is a result; a state of the counts alone is none (the round's own
            // stops hold the first, and the counting has been through the rest)
            let beside = |f: &Facts| -> u64 {
                let mut h = DefaultHasher::new();
                for p in snap.policy.world_of(f).iter().filter(|p| keys.binary_search(&p.0).is_err() && !snap.ambient.contains(&p.0) && !motion.contains(&p.0)) {
                    p.hash(&mut h);
                }
                h.finish()
            };
            // --go: a count counts where the scripts read it, and that may be another room (what is set here is
            // asked for there). For the counts the scripts of a few rooms add to or take from (`Mentions::vars`):
            // at each value of those counts that the counting comes to, a walk to the other rooms whose scripts
            // read one, the nearest first, one trial a walk, COUNT_WALKS values at most. The room that reads a
            // count reads its value, and many states hold one value (five things given one by one: 26 states,
            // 5 values; a walk for each state had spent its walks at the first value). A walk's stop is a walk's
            // stop like any other: new there by the count's value if it is, and expanded before the lines' turns.
            // (A state of the counts alone is no stop here, and as a stop of its own it is new by a variable's
            // value alone and waits: 633 such stops stood before it in one run)
            let mut ways: Vec<Vec<(i32, &Vec<Hit>)>> = Vec::new();
            let mut read_vars: Vec<Key> = Vec::new();
            if snap.go {
                let mut rooms: Vec<i32> = Vec::new();
                for k in &vars {
                    let Some(rs) = snap.mentions.vars.get(&(k.a() as u32)) else { continue };
                    read_vars.push(*k);
                    for &r in rs {
                        if r != r0.room && !rooms.contains(&r) {
                            rooms.push(r);
                        }
                    }
                }
                ways = rooms.iter().filter_map(|&t| way(snap.edges, r0.room, t)).collect();
                ways.sort_by_key(|w| (w.len(), w.last().map(|x| x.0)));
                ways.truncate(GO_ROOMS);
            }
            let value = |f: &Facts| -> Vec<Val> { read_vars.iter().map(|&k| f.val(k)).collect() };
            let mut walked: HashSet<Vec<Val>> = HashSet::from([value(&r0.facts)]);
            let mut walks = 0usize;
            let mut led: HashSet<u64> = HashSet::from([beside(&r0.facts)]);
            let mut seen: HashSet<u64> = HashSet::from([state(&r0.facts)]);
            let mut queue: VecDeque<(Box<dyn Engine>, String, Vec<Event>, usize)> = VecDeque::from([(e.fork()?, String::new(), Vec::new(), 0)]);
            let mut made = 0usize;
            while let Some((mut at, name, events, depth)) = queue.pop_front() {
                if seen.len() < COUNT_STATES {
                    for x in &inputs {
                        let mut b = at.fork()?;
                        out.trials += 1;
                        let Some((evs, st)) = again(&mut b, &x.1, ego, cfg, true)? else {
                            b.quit();
                            continue;
                        };
                        let ob = b.obs(&NAMES)?;
                        let mut all = events.clone();
                        all.extend(evs);
                        let r = read(if name.is_empty() { x.0.clone() } else { format!("{name}.{}", x.0) }, all.clone(), &ob, st);
                        let fresh = seen.insert(state(&r.facts));
                        let here = r.room == r0.room;
                        let val = value(&r.facts);
                        let rname = r.name.clone();
                        trace(&format!("    count {rname}: {:?}{}", keys.iter().map(|k| r.facts.val(*k)).collect::<Vec<_>>(), if fresh { " new" } else { "" }));
                        // (the first inputs' stops are the round's own results already)
                        if depth > 0 && (!here || led.insert(beside(&r.facts))) {
                            made += 1;
                            out.results.push(slim(r, &r0.facts));
                        }
                        if here && !ways.is_empty() && walked.len() <= COUNT_WALKS && walked.insert(val) {
                            for w in &ways {
                                walks += 1;
                                out.trials += 1;
                                if let Some(mut r) = walk(&mut b, w, ego, cfg, &mut out.bad)? {
                                    let mut whole = all.clone();
                                    whole.append(&mut r.events);
                                    r.events = whole;
                                    // ("gone": a walk made with a count; first in its room with that value in
                                    // its line of gains, it goes before the lines' turns as a walk does)
                                    r.name = format!("gone{}", r.room);
                                    out.results.push(slim(r, &r0.facts));
                                }
                            }
                        }
                        if fresh && here && seen.len() < COUNT_STATES {
                            queue.push_back((b, rname, all, depth + 1));
                        } else {
                            b.quit();
                        }
                    }
                }
                at.quit();
            }
            if walks > 0 {
                trace(&format!("  room {} counting: {walks} walks to {:?}", r0.room, ways.iter().map(|w| w.last().map(|x| x.0)).collect::<Vec<_>>()));
            }
            trace(&format!("  room {} counting: inputs {:?}, counts {:?}, keys {:?}, {} states, {} stops", r0.room, inputs.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), vars.iter().map(|k| format!("{k}")).collect::<Vec<_>>(), keys.iter().map(|k| format!("{k}")).collect::<Vec<_>>(), seen.len(), made));
        }
    }
    let locals0: HashMap<u64, i32> = r0.locals.iter().copied().collect();
    for r in out.results.iter_mut() {
        r.changed = r.delta.iter().filter(|d| (world_kind(d.0) || d.0.kind() == K_VAR) && d.1 != d.2.unwrap_or(0)).map(|d| (d.0, d.2.unwrap_or(0))).collect();
        r.locals.retain(|l| locals0.get(&l.0) != Some(&l.1));
        if r.name.starts_with("wait") && !r.name.contains('+') {
            // what a run with no input moved
            out.lclocks.extend(r.locals.iter().map(|l| l.0));
        }
    }
    // stops that are one stop for all the search counts (the room, the interface, the picture, the floor, the
    // scripts' locals, and the world against the expanded stop's): the first stands for the rest. Most inputs of a
    // stop end where another did, and the commit, which is not made side by side, need not read them again
    let before = out.results.len();
    let mut seen: HashSet<u64> = HashSet::new();
    out.results.retain(|r| {
        if r.name.starts_with("wait") || r.name.starts_with("go") {
            return true;
        }
        let mut h = DefaultHasher::new();
        (r.room, r.iface, &r.vis, r.walkbox, r.cam, r.inside, r.pic, &r.locals, &r.own).hash(&mut h);
        for c in r.changed.iter().filter(|c| snap.policy.world(c.0)) {
            c.hash(&mut h);
        }
        seen.insert(h.finish())
    });
    trace(&format!("  room {}: {} stops, {} of them not one another's like", r0.room, before, out.results.len()));
    Ok(out)
}

/// A kept stop's engine from a fork of a stop on its way: the events since that stop again, and the run to the
/// stop's frame (the engine is deterministic).
fn replay(mut b: Box<dyn Engine>, events: &[Event], frame: u64) -> EResult<Box<dyn Engine>> {
    let run = (|| -> EResult<()> {
        let mut i = 0;
        while i < events.len() {
            // the events of one frame together, given at the frame before the first's own or later
            let f = events[i].frame;
            if f - 1 > b.status().frame {
                b.run(&RunSpec { frame: Some(f - 1), ..Default::default() })?;
            }
            let j = events[i..].iter().position(|x| x.frame > f + 1).map_or(events.len(), |k| i + k);
            b.inputs(&events[i..j])?;
            i = j;
        }
        if frame > b.status().frame {
            b.run(&RunSpec { frame: Some(frame), ..Default::default() })?;
        }
        Ok(())
    })();
    match run {
        Ok(()) => Ok(b),
        Err(x) => {
            b.quit();
            Err(x)
        }
    }
}

impl Search {
    /// One expanded stop's results committed: the kept stops made nodes and queued. Returns each kept stop to make an
    /// engine for: (the node, its events past the parent's, its frame).
    fn commit(&mut self, id: usize, out: Out) {
        let room0 = out.room;
        let level0 = self.nodes[id].level;
        for &k in &out.ambient {
            *self.motion.entry((level0, room0, out.walkbox, k)).or_default() += 1;
        }
        self.noise.extend(out.noise.iter().copied());
        for f in &out.followed {
            *self.wins.entry(*f).or_default() += 1;
        }
        self.policy.iface_vars.extend(out.iface_vars.iter().copied());
        self.own_slots.extend(out.own_slots.iter().copied());
        self.policy.clocks.extend(out.clocks.iter().copied());
        self.lclocks.extend(out.lclocks.iter().copied());
        self.ambient.extend(out.ambient.iter().copied());
        self.trials += out.trials;
        self.skipped += out.skipped;
        self.frames += out.frames;
        for (a, b) in self.phase.iter_mut().zip(out.phase.iter()) {
            *a += b;
        }
        for (a, b) in self.xt.iter_mut().zip(out.xt.iter()) {
            *a += b;
        }
        self.refused += out.refused;
        self.not_idle += out.not_idle;
        for x in out.ends {
            self.ends.push((id, x));
        }
        let results = out.results;
        let (depth, ever0) = (self.nodes[id].depth, self.nodes[id].ever.clone());
        let (line0, room00) = (self.nodes[id].line, self.nodes[id].room);
        // a way whose clicks a walk made and that did not lead on: forgotten (seen again, it is learned again, twice
        // at most)
        for k in &out.bad {
            if let Some(h) = self.edges.remove(k) {
                let how: String = h.iter().map(|x| x.name()).collect();
                if let Some(f) = self.edges_log.as_mut() {
                    let _ = writeln!(f, "{}", json!({"from": k.0, "to": k.1, "how": how, "failed": true}));
                }
                *self.dead.entry((k.0, k.1, how)).or_default() += 1;
            }
        }
        // the rooms the stop comes to with no input at all: a click that ends there did not lead there
        let drift: HashSet<i32> = results.iter().filter(|r| r.name.starts_with("wait") && !r.name.contains('+') && r.room != room00).map(|r| r.room).collect();
        let mut keep: Vec<(usize, usize, usize, usize, bool)> = Vec::new(); // (result, node, level, queue, to the stack)
        // The expanded stop's own, made once. A result holds what differs from that stop (`delta`), and what is
        // asked of it here is put together from the stop's and the difference: a pass over a result's 25,000 facts
        // for each question was the commit's time once the writing was gone
        let base = &out.base;
        let ego0 = base.val(Key::pack(K_EGO, 0, 0, 0));
        let flaglike = |k: Key| k.kind() == K_BIT || k.kind() == K_CLASS;
        let bit_of = |k: Key| if k.kind() == K_BIT { k.a() as usize } else { 4096 + (k.a() as usize) * 32 + (k.b() as usize & 31) };
        let h1 = |k: Key, v: Val| -> u64 {
            let mut h = DefaultHasher::new();
            (k, v).hash(&mut h);
            h.finish()
        };
        // its guide's variables; what it owns; its flags set (but those seen unset again); the sum of its world's
        // atoms' hashes but the ambient keys' (the variables' part is added for each result: a variable may leave
        // the world while the stop is committed)
        let gv0: Vec<(Key, Val)> = base.iter().filter(|p| p.0.kind() == K_VAR && self.policy.guide.contains(&p.0)).copied().collect();
        let own0: Vec<u32> = base.iter().filter(|p| p.0.kind() == K_OWNER && p.1 == ego0).map(|p| p.0.a() as u32).filter(|o| !self.root_owned.contains(o)).collect();
        let mut set0: Vec<u64> = Vec::new();
        // (a flag was seen unset for the first time in this commit: the lines are judged again at its end)
        let mut retoggle = false;
        let mut sum0 = 0u64;
        for p in base.iter().filter(|p| world_kind(p.0)) {
            if flaglike(p.0) && p.1 != 0 && !self.toggles.contains(&p.0) {
                let n = bit_of(p.0);
                if set0.len() <= n / 64 {
                    set0.resize(n / 64 + 1, 0);
                }
                set0[n / 64] |= 1 << (n % 64);
            }
            if !self.ambient.contains(&p.0) {
                sum0 = sum0.wrapping_add(h1(p.0, p.1));
            }
        }
        // what the stop holds only for a while: the objects ego owns at it that one of its own waits saw gone
        // (what the game takes back by itself, as the stop's waits show it; nothing of the way to the stop is
        // asked). Not the flags that go off by themselves: there are many (5 or 6 in each of the two checks from
        // the VM's stops), and with them `var:196 = 3` came at the 29th stop for the 15th and object 420 not in 30
        let mut going: Vec<Key> = results
            .iter()
            .filter(|r| r.name.starts_with("wait") && !r.name.contains('+'))
            .flat_map(|r| r.delta.iter().filter(|x| x.0.kind() == K_OWNER && x.1 == ego0 && x.2 != Some(ego0)).map(|x| x.0))
            .collect();
        going.sort_unstable();
        going.dedup();
        self.expires.extend(going.iter().copied());
        // (the objects)
        let going_objs: Vec<_> = going.iter().map(|k| k.a()).collect();
        for (i, r) in results.iter().enumerate() {
            let tc = Instant::now();
            let d = &r.delta;
            // a key's value at the result
            let val = |k: Key| -> Val {
                match d.binary_search_by_key(&k, |x| x.0) {
                    Ok(i) => d[i].2.unwrap_or(0),
                    Err(_) => base.val(k),
                }
            };
            let mut novel: Vec<String> = Vec::new();
            let mut class = QUEUES;
            let ego = val(Key::pack(K_EGO, 0, 0, 0));
            // the cell's part of the guide's variables that are world (not the interface's, a clock's or a capped
            // one's), as the world is before this result's own atoms are read (one of them may cap a variable)
            let mut sum_vars = 0u64;
            for &(k, v0) in &gv0 {
                let v = match d.binary_search_by_key(&k, |x| x.0) {
                    Ok(i) => d[i].2,
                    Err(_) => Some(v0),
                };
                if let Some(v) = v.filter(|_| self.policy.world(k) && !self.ambient.contains(&k)) {
                    sum_vars = sum_vars.wrapping_add(h1(k, v));
                }
            }
            for x in d.iter().filter(|x| x.0.kind() == K_VAR && base.get(x.0).is_none()) {
                if let Some(v) = x.2.filter(|_| self.policy.world(x.0) && !self.ambient.contains(&x.0)) {
                    sum_vars = sum_vars.wrapping_add(h1(x.0, v));
                }
            }
            // what it owns: the expanded stop's with the owners that differ (read whole where ego is another)
            let owned: Vec<u32> = if ego != ego0 {
                patched(base, d).iter().filter(|p| p.0.kind() == K_OWNER && p.1 == ego).map(|p| p.0.a() as u32).filter(|o| !self.root_owned.contains(o)).collect()
            } else {
                let mut v = own0.clone();
                for x in d.iter().filter(|x| x.0.kind() == K_OWNER) {
                    let o = x.0.a() as u32;
                    if self.root_owned.contains(&o) {
                        continue;
                    }
                    match (v.binary_search(&o), x.2 == Some(ego)) {
                        (Err(at), true) => v.insert(at, o),
                        (Ok(at), false) => {
                            v.remove(at);
                        }
                        _ => {}
                    }
                }
                v
            };
            let mut ever = ever0.clone();
            for &o in &owned {
                if let Err(at) = ever.binary_search(&o) {
                    ever.insert(at, o);
                }
            }
            // --lines gains: a line is what was gained, a loss stays in its line (what it changed is new there,
            // the rest is not)
            let (level, gline) = if self.gains_only { (ever.len() as u32, line_of(&ever, &[])) } else { ((2 * ever.len() - owned.len()) as u32, line_of(&ever, &owned)) };
            // the flags the stop holds set (but those seen unset again): a count no stop of the line has held is new
            // (a stop that holds two flags which two other stops hold one each is the first with both)
            // a flag here is a bit or a class of an object (an object made into something else keeps its number and
            // gets a class): both are set once and stay, or are seen unset again and count no more
            self.ct[0] += tc.elapsed().as_secs_f64();
            let tc = Instant::now();
            for c in r.changed.iter().filter(|c| flaglike(c.0) && c.1 == 0) {
                if self.toggles.insert(c.0) {
                    // (kept beside the stops, OUT/toggles.txt, for a run that starts from one of them)
                    if let Some(f) = self.toggles_log.as_mut() {
                        let _ = writeln!(f, "{}", c.0);
                    }
                    retoggle = true;
                    // seen unset: it counts no more, for this stop's later results too
                    let n = bit_of(c.0);
                    if let Some(w) = set0.get_mut(n / 64) {
                        *w &= !(1u64 << (n % 64));
                    }
                }
            }
            let mut set = set0.clone();
            // --said-flags: the stop's own sentence (a verb and an object, in the room it stays in) set it; full2,
            // 2026-11-14: a gate opened by a sentence and closed again by the room's entry was seen unset, so its
            // opening began no line and the place behind it was new to none (the stop two clicks on was never made)
            let said = self.said_flags && r.room == room00 && !r.name.starts_with("wait") && !r.name.starts_with("go") && {
                let b = r.name.as_bytes();
                b.windows(2).any(|w| w[0] == b't' && w[1].is_ascii_digit())
            };
            let mut said_set = false;
            for x in d.iter().filter(|x| flaglike(x.0)) {
                let n = bit_of(x.0);
                if set.len() <= n / 64 {
                    set.resize(n / 64 + 1, 0);
                }
                let again = said && self.toggles.contains(&x.0) && r.changed.iter().any(|c| c.0 == x.0 && c.1 != 0);
                said_set |= again && x.2.is_some_and(|v| v != 0);
                if x.2.is_some_and(|v| v != 0) && (!self.toggles.contains(&x.0) || again) {
                    set[n / 64] |= 1u64 << (n % 64);
                } else {
                    set[n / 64] &= !(1u64 << (n % 64));
                }
            }
            let flags: u32 = set.iter().map(|w| w.count_ones()).sum();
            // a set of flags that no kept stop of the line holds all of: new, and followed before anything else of
            // its level (the last such stop first: what is won one flag at a time is won on one path)
            let within = |a: &[u64], b: &[u64]| a.iter().enumerate().all(|(i, w)| w & !b.get(i).copied().unwrap_or(0) == 0);
            let tops = self.tops.entry(gline).or_default();
            let mut dive = false;
            // only a stop that changed a flag, or stands in another line of gains than the expanded stop, can hold
            // a set that is new
            let moved = gline != self.nodes[id].gline || r.changed.iter().any(|c| flaglike(c.0));
            if moved && !tops.iter().any(|m| within(&set, m)) {
                tops.retain(|m| !within(m, &set));
                // (a line a flag seen unset began is not judged again by its flags: they did not stay before it)
                self.flines.push((self.nodes.len(), gline, set.clone(), said_set));
                tops.push(set);
                dive = true;
            }
            self.ct[1] += tc.elapsed().as_secs_f64();
            // what the stop won, where it begins a line: the objects it gained, the flags it set
            let news: Vec<Key> = if dive {
                r.changed
                    .iter()
                    .filter(|c| (c.0.kind() == K_OWNER && c.1 == ego) || (flaglike(c.0) && c.1 != 0 && !self.toggles.contains(&c.0)))
                    .map(|c| c.0)
                    .collect()
            } else {
                Vec::new()
            };
            // the clicks that led from the expanded stop's room to another: the way between the two, for a walk
            if r.room != room00 && !drift.contains(&r.room) && !r.name.contains('+') && !r.name.contains('>') && !r.name.starts_with("go") {
                if let Some(h) = hits_of(&r.name) {
                    let how = r.name.trim_end_matches('~').to_string();
                    if !self.edges.contains_key(&(room00, r.room)) && self.dead.get(&(room00, r.room, how)).copied().unwrap_or(0) < 2 {
                        // kept beside the stops (OUT/edges.jsonl), for a run that starts from one of them
                        if let Some(f) = self.edges_log.as_mut() {
                            let _ = writeln!(f, "{}", json!({"from": room00, "to": r.room, "how": r.name.trim_end_matches('~')}));
                        }
                        self.edges.insert((room00, r.room), h);
                    }
                }
            }
            // --flag-lines: such a stop starts a line of its own, as a gain does: every room and input counts
            // again from it (a flag is carried to where it matters by the stops that are new on the way)
            let fline = if dive { self.nodes.len() } else { self.nodes[id].fline };
            let line = if self.flag_lines {
                let mut h = DefaultHasher::new();
                (gline, fline).hash(&mut h);
                h.finish()
            } else {
                gline
            };
            if dive {
                novel.push(format!("flags={flags}"));
                class = 0;
            }
            // in the expanded stop's line and room only what changed can be new; elsewhere every atom may be
            let tc = Instant::now();
            let atoms: Vec<(Key, Val)> = if line == line0 && r.room == room00 {
                r.changed.iter().filter(|p| self.policy.world(p.0)).copied().collect()
            } else {
                // another room or another line: every atom of its world (its facts put together again)
                self.policy.world_of(&patched(base, d))
            };
            let mut count_new = false;
            // a walk made with a count goes first once for the room and the count's value in a line of gains (in
            // every line a set of flags begins it would be first again: 16 of 40 stops in one check)
            let mut count_went = false;
            let mut went_with: Option<(Key, Val)> = None;
            for &(k, val) in &atoms {
                let guide = self.policy.guide.contains(&k);
                if k.kind() == K_VAR {
                    if r.name.starts_with("gone") && self.mentions.vars.contains_key(&(k.a() as u32)) && self.went.insert((gline, r.room, k, val)) {
                        count_went = true;
                        went_with = Some((k, val));
                    }
                    if self.seen.insert((line, k, val)) {
                        count_new = true;
                        novel.push(format!("{k}={val}"));
                        class = class.min(2);
                        let vals = self.var_vals.entry(k).or_default();
                        if !vals.contains(&val) {
                            vals.push(val);
                        }
                        if vals.len() > VAR_CAP {
                            self.policy.capped.insert(k);
                        }
                    }
                    if self.seen_at.insert((line, r.room, k, val)) {
                        class = class.min(3);
                    }
                    continue;
                }
                // --motion-once (off by default; not yet run from the boot): a key the game moves by itself (a wait
                // moved it at some stop: a room's own motion, a door on a timer) is new once in a line of gains,
                // not once more in every line a set of flags begins there. No input brought the value about, and
                // with each new set of flags a room's motion was news again (64% of a run's trials after its
                // sixth rung, most of them in room 28)
                let own = if self.motion_once && self.ambient.contains(&k) { gline } else { line };
                if self.seen.insert((own, k, val)) {
                    novel.push(format!("{k}={val}"));
                    class = 0;
                }
                if self.seen_at.insert((own, r.room, k, val)) {
                    class = class.min(if guide { 3 } else { 4 });
                }
            }
            // What the expanded stop holds only for a while (`going`), still held here, in another room, where
            // no stop of this line of gains was seen to hold it so: carried there while it lasts. Followed like
            // what is new. (As atoms it is none: an object owned in that room and its flag set there were each
            // seen before, and the flag stays set on an object that is gone.)
            let mut carried = false;
            if r.room != room00 && !r.name.starts_with("wait") {
                for &k in &going {
                    if val(k) == base.val(k) && self.carried_at.insert((gline, r.room, k)) {
                        novel.push(format!("{k}@{}", r.room));
                        class = 0;
                        carried = true;
                    }
                }
            }
            // Or, in the stop's room, an input that changed a key of an object the stop holds only for a while and
            // set a flag or the owner of another object: it passed what the first holds on to the other. Followed
            // at once as well, once for a room and the keys it set (as atoms they are nothing new the second time)
            if r.room == room00 && !r.name.starts_with("wait") && !going_objs.is_empty() {
                let touched = r.changed.iter().any(|c| matches!(c.0.kind(), K_OWNER | K_CLASS) && going_objs.contains(&c.0.a()));
                let mut set: Vec<Key> = r
                    .changed
                    .iter()
                    .filter(|c| matches!(c.0.kind(), K_OWNER | K_CLASS) && !going_objs.contains(&c.0.a()) && c.1 != 0 && base.val(c.0) == 0)
                    .map(|c| c.0)
                    .collect();
                if touched && !set.is_empty() {
                    set.sort_unstable();
                    let mut h = DefaultHasher::new();
                    set.hash(&mut h);
                    if self.acted_at.insert((gline, r.room, h.finish())) {
                        novel.push(format!("acted@{}", r.room));
                        class = 0;
                        carried = true;
                    }
                }
            }
            self.ct[2] += tc.elapsed().as_secs_f64();
            let tc = Instant::now();
            // a local variable of a running script with a value new in the line (a count a dialog keeps of what
            // was said): like a variable's
            let mut fresh = false;
            for &(k, v) in &r.locals {
                if self.lclocks.contains(&k) || self.lcapped.contains(&k) {
                    continue;
                }
                if self.lseen.insert((line, k, v)) {
                    fresh = true;
                    let vals = self.lvals.entry(k).or_default();
                    if !vals.contains(&v) {
                        vals.push(v);
                    }
                    if vals.len() > LOCAL_CAP {
                        self.lcapped.insert(k);
                    }
                }
            }
            if fresh {
                novel.push("local".into());
                class = class.min(2);
            }
            for &t in &r.vis {
                if self.sees.insert((line, r.room, t)) {
                    novel.push(format!("sees={t}"));
                    class = class.min(1);
                }
            }
            // the interface without its own state: the slots but those a click changes alone, and the scripts that
            // take the clicks
            let face = {
                let mut h = DefaultHasher::new();
                r.hands.hash(&mut h);
                for x in r.slots.iter().filter(|x| !self.own_slots.contains(&x.0)) {
                    x.hash(&mut h);
                }
                h.finish()
            };
            // (another picture of the expanded stop and no more is nothing new: the stop's round clicked on from it)
            if self.ifaces.insert((line, face)) && !r.pic {
                novel.push("iface".into());
                class = class.min(1);
            }
            if self.boxes.insert((line, r.room, r.walkbox)) {
                novel.push(format!("box={}", r.walkbox));
                class = class.min(2);
            }
            if self.cams.insert((line, r.room, r.cam)) {
                novel.push(format!("cam={}", r.cam));
                class = class.min(2);
            }
            self.ct[3] += tc.elapsed().as_secs_f64();
            let tc = Instant::now();
            // the cell: the room, the interface, and the world's atoms but the ambient keys', as the sum of the
            // atoms' hashes: the expanded stop's sum with what differs, and the guide's variables that are world
            let mut sum = sum0;
            for x in d.iter().filter(|x| world_kind(x.0) && !self.ambient.contains(&x.0)) {
                if let Some(v0) = base.get(x.0) {
                    sum = sum.wrapping_sub(h1(x.0, v0));
                }
                if let Some(v) = x.2 {
                    sum = sum.wrapping_add(h1(x.0, v));
                }
            }
            sum = sum.wrapping_add(sum_vars);
            let c = {
                let mut h = DefaultHasher::new();
                (r.room, face, sum).hash(&mut h);
                h.finish()
            };
            if self.cells.insert((line, c)) && !r.pic {
                class = class.min(4);
            }
            self.ct[4] += tc.elapsed().as_secs_f64();
            // nothing new, alone or together: dropped
            if class == QUEUES {
                continue;
            }
            if !self.rooms.contains(&r.room) {
                self.rooms.push(r.room);
            }
            let tc = Instant::now();
            let nid = self.nodes.len();
            // --count-lines: a walk made with a count whose stop is new by more than the count's value (a thing
            // seen there, the interface, a key of the world) in a room its line knows: the value did something in
            // the room that reads it.
            // Such a stop begins a line as a new set of flags does, the first for the count's value in its line
            // of gains: every room and input counts again from it. (A count is no flag and began no line: where
            // an actor followed ego for it, the rooms on the way were known, their stops new by nothing, and
            // the way through a room that scrolls was not made again.)
            let (fline, line, dive) = match went_with {
                // (in a room the line knows: a stop new by its room is new by everything)
                Some((k, v)) if self.count_lines && self.flag_lines && class <= 1 && !novel.iter().any(|x| x.starts_with("room=")) && self.count_line.insert((gline, k, v)) => {
                    novel.insert(0, format!("line:{k}={v}"));
                    self.flines.push((nid, gline, Vec::new(), true));
                    let mut h = DefaultHasher::new();
                    (gline, nid).hash(&mut h);
                    (nid, h.finish(), true)
                }
                _ => (fline, line, dive),
            };
            // (a result of the stop that began this line, in its room, and no line's first stop itself; or of a
            // walk the search made for the line's news, in the room it walked to: the scripts name that room for
            // what the line won, and what the gain does there is looked at first. In one run the crate opened by
            // the tools the line had won waited under 230 stops of newer walks through the maze)
            let born = r.room == room00 && ((self.nodes[id].fline == id && fline == id) || (self.walk_born && self.nodes[id].walk && fline == self.nodes[id].fline));
            let events = r.events.clone();
            let secs = self.t0.map(|t| t.elapsed().as_secs_f64()).unwrap_or(0.0);
            let mut first_held = false;
            for w in self.watch.iter_mut() {
                if w.2.is_none() && val(w.0) == w.1 {
                    first_held = true;
                    w.2 = Some((nid, secs));
                    eprintln!("[{secs:7.1}s] WATCH {}={} first held by node {nid}", w.0, w.1);
                    if self.until.contains(&(w.0, w.1)) {
                        eprintln!("[{secs:7.1}s] UNTIL {}={} held: the run ends", w.0, w.1);
                        self.done = true;
                    }
                }
            }
            for g in self.rungs.iter_mut().filter(|g| g.at.is_none()) {
                let held = g.conds.iter().filter(|&&(k, ge, v)| {
                    let (have, want) = (val(k), v.unwrap_or(ego));
                    if ge { have >= want } else { have == want }
                });
                if held.count() >= g.least {
                    first_held = true;
                    g.at = Some((nid, secs));
                    eprintln!("[{secs:7.1}s] RUNG {} first held by node {nid} (level {level}, {} trials so far)", g.name, self.trials);
                }
            }
            if novel.len() > 12 {
                // a stop new to its level holds everything anew: the count is enough
                let n = novel.len();
                novel.truncate(12);
                novel.push(format!("+{}", n - 12));
            }
            // nodes.jsonl: the stop's own events and the stop it came from (the movie is theirs one after the other:
            // a line with every stop's whole movie was 1.8 GB in two hours). The line is made once and written in
            // one piece (a value written as it is formatted goes to the file in thousands of small writes: that was
            // nearly all of the commit's time)
            // (what was ever owned on the way to it: the run's first stop's list and its own)
            let mut ever_all: Vec<u32> = self.root_owned.iter().copied().chain(ever.iter().copied()).collect();
            ever_all.sort_unstable();
            ever_all.dedup();
            let mut text = json!({"id": nid, "parent": id, "depth": depth + 1, "frame": r.st.frame, "room": r.room, "level": level,
                "owned": owned.len(), "class": class, "how": r.name, "novel": novel, "t": (secs * 10.0).round() / 10.0,
                "news": news.iter().map(|k| format!("{k}")).collect::<Vec<_>>(),
                "ever": ever_all,
                "movie": "own",
                "events": events.iter().map(fmi_explore::events::event_json).collect::<Vec<Value>>()});
            if let Some(f) = self.log.as_mut() {
                let _ = f.write_all(format!("{text}\n").as_bytes());
            }
            let top = dive && level >= self.top;
            if first_held || top {
                // rung-stops.jsonl and top.jsonl: the whole movie, so that the line alone replays from boot
                let mut whole = self.movie(id);
                whole.extend(events.iter().cloned());
                text["events"] = Value::Array(whole.iter().map(fmi_explore::events::event_json).collect());
                text.as_object_mut().unwrap().remove("movie");
                let text = format!("{text}\n");
                if first_held {
                    if let Some(f) = self.rung_log.as_mut() {
                        let _ = f.write_all(text.as_bytes());
                        let _ = f.flush();
                    }
                }
                if top {
                    if let Some(p) = &self.top_path {
                        // written whole beside and moved in: a reader never sees half a line
                        let tmp = p.with_extension("jsonl.new");
                        if std::fs::write(&tmp, &text).is_ok() {
                            let _ = std::fs::rename(&tmp, p);
                        }
                    }
                }
            }
            let mut pending = r.own.clone();
            for &(k, old) in &self.nodes[id].pending {
                if val(k) != old && !pending.iter().any(|p| p.0 == k) {
                    pending.push((k, old));
                }
            }
            pending.truncate(PENDING);
            let inside = r.inside || (self.nodes[id].inside && r.name.starts_with("wait"));
            self.nodes.push(Node { events, parent: id, frame: r.st.frame, depth: depth + 1, ever, level, line, room: r.room, pending, inside, flags, fline, gline, news, walk: r.name.starts_with("go") && (count_went || !r.name.starts_with("gone")), win: count_new && r.name.contains('>'), carry: carried, led: r.room != room00 && r.name.starts_with("wait") && !r.name.contains('+'), born });
            self.kept[class] += 1;
            if level > self.top {
                self.top_at = Some(Instant::now());
            }
            self.top = self.top.max(level);
            // with --flag-lines every stop of queue 0 goes on the stack: the newest line is the one worked on
            // (a stop where the game takes no input is kept and never expanded)
            if !r.name.ends_with('!') {
                keep.push((i, nid, level as usize, class, dive || (self.flag_lines && class == 0)));
            }
            self.ct[5] += tc.elapsed().as_secs_f64();
        }
        if let Some(f) = self.offers.as_mut() {
            let _ = writeln!(f, "{}", json!({"node": id, "room": room0, "offer": out.offer, "trials": out.trials, "skipped": out.skipped, "mids": out.mids, "windows": out.windows,
                "stops": results.len(), "kept": keep.len(), "refused": out.refused, "not_idle": out.not_idle}));
        }
        for (_, nid, level, class, dive) in keep {
            if self.queues.len() <= level {
                self.queues.resize_with(level + 1, Default::default);
            }
            if self.stacks.len() <= level {
                self.stacks.resize_with(level + 1, Default::default);
            }
            if dive {
                self.stacks[level].push(nid);
            } else {
                self.queues[level][class].push_back(nid);
            }
        }
        if retoggle && self.stay_lines {
            self.rejudge();
        }
    }

    /// --stay-lines: a line a set of flags began is a line while those flags stay. Each line's set without the
    /// flags seen unset since; a line whose set an older line of its line of gains then holds all of began with
    /// nothing that stayed (a door's state, the classes a room's entry sets by the way ego came): it has its
    /// turns after all the others. (The newest line has every second turn, and such lines kept being the
    /// newest: 7 of them in 20 minutes on one island, the line that had won something at a 128th of the turns.)
    fn rejudge(&mut self) {
        let mut mask: Vec<u64> = Vec::new();
        for k in &self.toggles {
            let n = if k.kind() == K_BIT { k.a() as usize } else { 4096 + (k.a() as usize) * 32 + (k.b() as usize & 31) };
            if mask.len() <= n / 64 {
                mask.resize(n / 64 + 1, 0);
            }
            mask[n / 64] |= 1 << (n % 64);
        }
        let within = |a: &[u64], b: &[u64]| a.iter().enumerate().all(|(i, w)| w & !b.get(i).copied().unwrap_or(0) == 0);
        let before = self.junk.len();
        self.junk.clear();
        let mut kept: HashMap<u64, Vec<Vec<u64>>> = HashMap::new();
        for (id, g, set, fixed) in &self.flines {
            if *fixed {
                continue;
            }
            let s: Vec<u64> = set.iter().enumerate().map(|(i, w)| w & !mask.get(i).copied().unwrap_or(0)).collect();
            let tops = kept.entry(*g).or_default();
            if tops.iter().any(|t| within(&s, t)) {
                self.junk.insert(*id);
            } else {
                tops.push(s);
            }
        }
        if self.junk.len() != before {
            trace(&format!("  lines: {} of {} began with flags that did not stay", self.junk.len(), self.flines.len()));
        }
    }

    fn progress(&self, last: &mut u64, dt: f64) {
        let secs = self.t0.map(|t| t.elapsed().as_secs_f64()).unwrap_or(0.0);
        let rungs: Vec<&str> = self.rungs.iter().filter(|g| g.at.is_some()).map(|g| g.name.as_str()).collect();
        let queued: Vec<usize> = (0..QUEUES).map(|c| self.queues.iter().map(|q| q[c].len()).sum()).collect();
        let at_top: usize = self.queues.last().map_or(0, |q| q.iter().map(|x| x.len()).sum()) + self.stacks.last().map_or(0, |x| x.len());
        eprintln!(
            "[{secs:7.1}s] kept {} {:?}, expanded {}, level {} ({} queued there), queued {:?}, trials {} ({:.0}/s), rooms {}, rungs {:?}, boots {}",
            self.nodes.len(), self.kept, self.expanded, self.top, at_top, queued,
            self.trials, (self.trials - *last) as f64 / dt.max(0.1), self.rooms.len(), rungs, self.boots
        );
        *last = self.trials;
    }
}

/// The evaluation's ladder (fmi-checks/rungs): each rung's `all` conditions, or `least` of its `of`.
fn rungs_of(path: &str) -> Vec<Rung> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).expect("--rungs")).expect("the rungs' JSON");
    let cond = |c: &Value| -> (Key, bool, Option<Val>) {
        (Key::parse(c[0].as_str().expect("a key")), c[1].as_str() == Some(">="), c[2].as_i64())
    };
    v["rungs"]
        .as_array()
        .expect("rungs")
        .iter()
        .map(|g| {
            let all: Vec<_> = g["all"].as_array().map(|a| a.iter().map(cond).collect()).unwrap_or_default();
            let of: Vec<_> = g["of"].as_array().map(|a| a.iter().map(cond).collect()).unwrap_or_default();
            let (conds, least) = if of.is_empty() { let n = all.len(); (all, n) } else { (of, g["least"].as_u64().unwrap_or(1) as usize) };
            Rung { name: g["rung"].as_str().unwrap_or("?").to_string(), conds, least, at: None }
        })
        .collect()
}

/// `direct show NODES.jsonl ID OUT [FRAMES]`: a measurement tool. The kept stop's movie played from boot, then each
/// single click of its offer (targets and floor points) in a fork, run FRAMES frames at most: what the stop was, or
/// where the run stood every 400 frames when it reached none (room, idle, cutscene depth, user input, ego moving).
fn show(nodes: &str, id: u64, out: &Path, frames: u64) {
    let (line, events) = stop_of(nodes, Some(id));
    let frame = line["frame"].as_u64().unwrap();
    std::fs::create_dir_all(out).expect("OUT");
    let host = Host::new(HostCfg::from_env()).expect("the host");
    let mut e = host
        .boot(&BootCfg { seed: 1, events, frame: Some(frame), dump_locals: true, lean: true, out_dir: eng_dir(out) })
        .expect("the boot");
    let o = e.obs(&ObsReq::default()).expect("obs");
    let base = facts_of(&o);
    let ego = base.val(Key::pack(K_EGO, 0, 0, 0)) as i32;
    let of = afford::offer_cfg(&mut e, &o, false, true).expect("offer");
    println!("stop: frame {frame} room {} offer {}", e.status().room, of.json());
    println!("  ego {:?} xstart {} room width {}", o.ego_pos, e.status().xstart, e.status().room_w);
    for r in o.room_objects.as_deref().unwrap_or(&[]) {
        println!("  object {} state {} at ({}, {}) size {}x{} walk {:?}", r.id, r.state, r.x, r.y, r.w, r.h, r.walk);
    }
    for a in &o.actors {
        println!("  actor {} at ({}, {}) moving {}", a.id, a.x, a.y, a.moving);
    }
    println!("  targets: {}", of.targets.iter().map(|p| format!("{}@({},{})", p.hit.name(), p.x, p.y)).collect::<Vec<_>>().join(" "));
    println!("  verbs: {}", of.verbs.iter().map(|p| format!("{}@({},{})", p.hit.name(), p.x, p.y)).collect::<Vec<_>>().join(" "));
    if let Ok(bx) = e.boxes() {
        for (i, w) in bx.boxes.iter().enumerate() {
            println!("  box {i} flags {:#x} {:?} {:?} {:?} {:?}", w.flags, w.ul, w.ur, w.lr, w.ll);
        }
    }
    let mut points: Vec<Point> = of.targets.clone();
    let (top, h) = e.status().screen.map(|s| (s.0, s.1)).unwrap_or((0, afford::SCREEN_H));
    let grid: Vec<(i32, i32)> = FLOOR_Y.iter().flat_map(|&k| FLOOR_X.iter().map(move |&x| (x, (top + (h - top) * k / 8).min(h - 2)))).collect();
    let ats = e.at_many(&grid).expect("at");
    for (i, (&(x, y), a)) in grid.iter().zip(ats.iter()).enumerate() {
        if a.main.is_some_and(|m| m.2 == 0 && m.3 == 0) {
            points.push(Point { x, y, hit: Hit::Walk(i as i32) });
        }
    }
    for p in &points {
        let mut b = e.fork().expect("fork");
        b.inputs(&[Event { frame: frame + 1, what: Input::Click(p.x, p.y) }]).expect("inputs");
        let _ = go(&mut b, RunSpec { frame: Some(frame + LEAD + CALM), ..Default::default() });
        let mut trace: Vec<String> = Vec::new();
        let mut end = frame + 400;
        let res = loop {
            match settle(&mut b, end.min(frame + frames), frame + LEAD + BUSY, ego) {
                Ok(Played::Stop(st)) => break format!("stop at +{} room {}", st.frame - frame, st.room),
                Ok(Played::Ended(st)) => break format!("ended: {} at +{}", st.reason, st.frame - frame),
                Ok(Played::NotIdle) => {
                    let st = b.status().clone();
                    trace.push(format!("+{} room {} idle {:?} cs {} up {} mv {:?} blk {:?}", st.frame - frame, st.room, st.idle, st.cs, st.up, st.mv, st.blk));
                    if end >= frame + frames {
                        break "no stop".to_string();
                    }
                    end += 400;
                }
                Err(x) => break format!("error: {x}"),
            }
        };
        let diff: Vec<String> = b.facts().map(|f| base.diff(&f).into_iter().filter(|d| world_kind(d.0)).map(|d| format!("{}:{}->{}", d.0, d.1, d.2)).take(8).collect()).unwrap_or_default();
        println!("{} ({},{}) -> {res} world {:?}", p.hit.name(), p.x, p.y, diff);
        for t in trace.iter().take(20) {
            println!("    {t}");
        }
        b.quit();
    }
    e.quit();
}

/// `direct verify NODES.jsonl ID OUT [KEY=V,..]`: the kept stop's movie played from boot in a fresh engine, to its
/// frame; prints the room and each key's value there, and whether each equals V.
fn verify(nodes: &str, id: u64, out: &Path, keys: &str) {
    let (line, events) = stop_of(nodes, Some(id));
    let frame = line["frame"].as_u64().unwrap();
    std::fs::create_dir_all(out).expect("OUT");
    let host = Host::new(HostCfg::from_env()).expect("the host");
    let t0 = Instant::now();
    let mut e = host
        .boot(&BootCfg { seed: 1, events: events.clone(), frame: Some(frame), dump_locals: true, lean: true, out_dir: eng_dir(out) })
        .expect("the boot");
    let f = e.facts().expect("facts");
    let room = f.val(Key::pack(K_ROOM, 0, 0, 0));
    let mut ok = room == line["room"].as_i64().unwrap();
    let mut held = Vec::new();
    for w in keys.split(',').filter(|x| !x.is_empty()) {
        let (k, v) = w.split_once('=').expect("KEY=V");
        let got = f.val(Key::parse(k));
        ok &= got == v.parse::<i64>().expect("a value");
        held.push(json!([k, got]));
    }
    let clicks = events.iter().filter(|x| matches!(x.what, Input::Click(..))).count();
    // (the game's own clock at the stop: sixtieths of a second since the boot, as the engine counted them)
    let tick = e.status().tick;
    println!("{}", json!({"node": id, "from_boot": true, "frame": frame, "tick": tick, "room": room, "keys": held, "clicks": clicks,
        "seconds": t0.elapsed().as_secs_f64(), "ok": ok}));
    e.quit();
}

fn main() {
    // an image verb's name is its image's hash (engine svm-e027; an engine without it shows no name for one, and a
    // page of images is then not seen)
    std::env::set_var("SPEEDRUN_VERB_IMAGES", "1");
    let a: Vec<String> = std::env::args().skip(1).collect();
    let opt = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    if a.first().map(String::as_str) == Some("forkbench") {
        // `direct forkbench OUT N`: the cost of a fork, a one-frame run, a dump and a quit, one engine, one thread
        let out = PathBuf::from(&a[1]);
        std::fs::create_dir_all(&out).expect("OUT");
        let n: usize = a[2].parse().expect("N");
        let host = Host::new(HostCfg::from_env()).expect("the host");
        // `--from NODES ID`: at a kept stop instead of the boot's first frame
        let (events, frame) = match opt("--from") {
            Some(nodes) => {
                let id: u64 = a[a.iter().position(|x| x == "--from").unwrap() + 2].parse().expect("--from NODES ID");
                let (line, events) = stop_of(&nodes, Some(id));
                (events, line["frame"].as_u64())
            }
            None => (Vec::new(), None),
        };
        let mut e = host.boot(&BootCfg { seed: 1, events, frame, dump_locals: true, lean: true, out_dir: eng_dir(&out) }).expect("the boot");
        eprintln!("engine {} at frame {}: {} kB resident", e.pid(), e.status().frame, std::fs::read_to_string(format!("/proc/{}/statm", e.pid())).ok().and_then(|x| x.split(' ').nth(1).and_then(|p| p.parse::<u64>().ok())).unwrap_or(0) * 4);
        if let Some(threads) = a.get(3).and_then(|x| x.parse::<usize>().ok()) {
            // `direct forkbench OUT N THREADS`: N forks (each run one frame and quit) on each of THREADS threads at
            // once, every thread from an engine of its own, all of one family: what a fork costs side by side
            let parents: Vec<Mutex<Option<Box<dyn Engine>>>> = (0..threads).map(|_| Mutex::new(Some(e.fork().expect("fork")))).collect();
            let t0 = Instant::now();
            let sums: Vec<(f64, f64)> = std::thread::scope(|sc| {
                let hs: Vec<_> = parents
                    .iter()
                    .map(|p| {
                        sc.spawn(move || {
                            let mut me = p.lock().unwrap().take().unwrap();
                            let f0 = me.status().frame;
                            let (mut fork, mut rest) = (0f64, 0f64);
                            for _ in 0..n {
                                let t = Instant::now();
                                let mut b = me.fork().expect("fork");
                                fork += t.elapsed().as_secs_f64();
                                let t = Instant::now();
                                let _ = b.run(&RunSpec { frame: Some(f0 + 1), ..Default::default() });
                                b.quit();
                                rest += t.elapsed().as_secs_f64();
                            }
                            me.quit();
                            (fork, rest)
                        })
                    })
                    .collect();
                hs.into_iter().map(|h| h.join().unwrap()).collect()
            });
            let wall = t0.elapsed().as_secs_f64();
            let total = (threads * n) as f64;
            println!("{}", json!({"threads": threads, "n": n, "forks_per_s": (total / wall).round(),
                "ms": {"fork": (sums.iter().map(|x| x.0).sum::<f64>() * 1e6 / total).round() / 1e3, "run_1_frame_and_quit": (sums.iter().map(|x| x.1).sum::<f64>() * 1e6 / total).round() / 1e3}}));
            e.quit();
            return;
        }
        let f0 = e.status().frame;
        let mut t = [0f64; 6];
        for _ in 0..n {
            let t0 = Instant::now();
            let mut b = e.fork().expect("fork");
            t[0] += t0.elapsed().as_secs_f64();
            let t0 = Instant::now();
            b.run(&RunSpec { frame: Some(f0 + 1), ..Default::default() }).expect("run 1");
            t[1] += t0.elapsed().as_secs_f64();
            let t0 = Instant::now();
            b.run(&RunSpec { frame: Some(f0 + 101), ..Default::default() }).expect("run 100");
            t[2] += t0.elapsed().as_secs_f64();
            let t0 = Instant::now();
            let _ = b.facts().expect("facts");
            t[3] += t0.elapsed().as_secs_f64();
            let t0 = Instant::now();
            let _ = b.obs(&NAMES).expect("obs");
            t[4] += t0.elapsed().as_secs_f64();
            let t0 = Instant::now();
            b.quit();
            t[5] += t0.elapsed().as_secs_f64();
        }
        let ms = |x: f64| (x * 1e6 / n as f64).round() / 1e3;
        let ns = |i: usize| fmi_engine::host::FORK_NS[i].load(Ordering::Relaxed) as f64 / 1e6 / n as f64;
        eprintln!("fork: {:.3} ms to the parent's reply, {:.3} ms to the child's connection", ns(0), ns(1));
        println!("{}", json!({"n": n, "ms": {"fork": ms(t[0]), "run_1_frame": ms(t[1]), "run_100_frames": ms(t[2]), "facts": ms(t[3]), "obs_names": ms(t[4]), "quit": ms(t[5])}}));
        e.quit();
        return;
    }
    if a.first().map(String::as_str) == Some("seq") {
        // `direct seq NODES ID OUT KEYS STEP...`: a measurement tool. The kept stop's movie from boot, then each STEP
        // "X,Y,N": a click at (X, Y) on the next frame and N frames run (N = "s": to where ego stands and the bridge
        // is idle); after each step, and every 5 frames for 120 more after the last, the KEYS' values.
        let id: u64 = a[2].parse().expect("ID");
        let (line, events) = stop_of(&a[1], Some(id));
        let frame = line["frame"].as_u64().unwrap();
        let out = PathBuf::from(&a[3]);
        std::fs::create_dir_all(&out).expect("OUT");
        let host = Host::new(HostCfg::from_env()).expect("the host");
        let mut e = host.boot(&BootCfg { seed: 1, events, frame: Some(frame), dump_locals: true, lean: true, out_dir: eng_dir(&out) }).expect("the boot");
        let keys: Vec<Key> = a[4].split(',').map(Key::parse).collect();
        let ego = e.facts().expect("facts").val(Key::pack(K_EGO, 0, 0, 0)) as i32;
        let say = |e: &mut Box<dyn Engine>, what: &str| {
            let f = e.facts().expect("facts");
            let st = e.status().clone();
            println!("+{:<4} {what:<14} room {} xstart {} ego {:?} idle {:?} mv {:?} box {:?} {}", st.frame - frame, st.room, st.xstart, st.ego, st.idle, st.mv, st.walkbox,
                keys.iter().map(|k| format!("{k}={}", f.val(*k))).collect::<Vec<_>>().join(" "));
            if std::env::var_os("DIRECT_SEQ_VERBS").is_some() {
                // the slots shown (id, point, the text's length; the text itself only with DIRECT_SEQ_TEXT, for a
                // look by hand) and the running scripts' locals that are not 0
                let o = e.obs(&NAMES).expect("obs");
                let text = std::env::var_os("DIRECT_SEQ_TEXT").is_some();
                println!("      slots: {}", o.verbs.iter().map(|v| format!("v{}@({},{}):{}", v.id, v.x, v.y, if text { v.name.clone().unwrap_or_default() } else { v.name.as_deref().unwrap_or("").len().to_string() })).collect::<Vec<_>>().join(" "));
                println!("      room {} cs {} up {} scripts: {}", st.room, st.cs, st.up, o.slots.iter().filter_map(|sl| sl.locals.as_ref().map(|ls| format!("{}:{}{:?}", sl.where_, sl.number, ls.iter().enumerate().filter(|x| *x.1 != 0).map(|x| (x.0, *x.1)).collect::<Vec<_>>()))).collect::<Vec<_>>().join(" "));
            }
        };
        say(&mut e, "start");
        for step in &a[5..] {
            let p: Vec<&str> = step.split(',').collect();
            let at = e.status().frame;
            if p[0] == "k" {
                // "k,ON,N": the skips (text and cutscenes) set on or off
                let on = p[1] == "1";
                e.inputs(&[Event { frame: at + 1, what: Input::Skips { text: on, cutscenes: on, spare: None } }]).expect("skips");
                let _ = go(&mut e, RunSpec { frame: Some(at + p[2].parse::<u64>().unwrap()), ..Default::default() });
                say(&mut e, step);
                continue;
            }
            let (x, y): (i32, i32) = (p[0].parse().unwrap(), p[1].parse().unwrap());
            if x >= 0 {
                e.inputs(&[Event { frame: at + 1, what: Input::Click(x, y) }]).expect("inputs");
            }
            if p[2] == "s" {
                let _ = go(&mut e, RunSpec { frame: Some(at + 2), ..Default::default() });
                let _ = settle(&mut e, at + 2000, at + 300, ego);
            } else {
                let _ = go(&mut e, RunSpec { frame: Some(at + p[2].parse::<u64>().unwrap()), ..Default::default() });
            }
            say(&mut e, step);
        }
        for _ in 0..24 {
            let at = e.status().frame;
            if go(&mut e, RunSpec { frame: Some(at + 5), ..Default::default() }).map_or(true, |r| r.is_err()) {
                break;
            }
            say(&mut e, "");
        }
        e.quit();
        return;
    }
    if a.first().map(String::as_str) == Some("show") {
        return show(&a[1], a[2].parse().expect("ID"), Path::new(&a[3]), a.get(4).and_then(|x| x.parse().ok()).unwrap_or(6400));
    }
    if a.first().map(String::as_str) == Some("verify") {
        return verify(&a[1], a[2].parse().expect("ID"), Path::new(&a[3]), a.get(4).map(String::as_str).unwrap_or(""));
    }
    let out = PathBuf::from(a.first().expect("direct OUT [--minutes M] [--workers W] [--horizon H] [--no-skips] [--guide F] [--rungs F] [--watch K=V,..]"));
    std::fs::create_dir_all(&out).expect("OUT");
    let minutes: f64 = opt("--minutes").map(|v| v.parse().unwrap()).unwrap_or(10.0);
    let cfg = Cfg {
        out: out.clone(),
        horizon: opt("--horizon").map(|v| v.parse().unwrap()).unwrap_or(1600),
        workers: opt("--workers").map(|v| v.parse().unwrap()).unwrap_or(6),
        slices: opt("--slices").map(|v| v.parse().unwrap()).unwrap_or(1),
        batch: opt("--batch").map(|v| v.parse().unwrap()).unwrap_or(BATCH),
        at_once: opt("--at-once").map(|v| v.parse().unwrap()).unwrap_or(0),
        max_mids: std::env::var("DIRECT_MAX_MIDS").ok().and_then(|v| v.parse().ok()).unwrap_or(MAX_MIDS),
        cross_pages: a.iter().any(|x| x == "--cross-pages"),
        ends: a.iter().any(|x| x == "--ends"),
        runs_on: a.iter().any(|x| x == "--runs-on"),
    };
    let host = Host::new(HostCfg::from_env()).expect("the host");
    let root_events = if a.iter().any(|x| x == "--no-skips") {
        Vec::new()
    } else {
        vec![Event { frame: 1, what: Input::Skips { text: true, cutscenes: true, spare: None } }]
    };
    // --from NODES ID (a measurement aid): the search starts at that kept stop of another run, not at the boot.
    // --from-run DIR: at the stop of DIR/top.jsonl (the newest stop that began a line at that run's top level), with
    // that run's ways (DIR/edges.jsonl) unless --edges names others
    let from_run: Option<PathBuf> = opt("--from-run").map(PathBuf::from);
    let from_at: Option<(String, Option<u64>)> = match (&from_run, opt("--from")) {
        (Some(d), _) => Some((d.join("top.jsonl").to_string_lossy().into_owned(), None)),
        (None, Some(nodes)) => Some((nodes, Some(a[a.iter().position(|x| x == "--from").unwrap() + 2].parse().expect("--from NODES ID")))),
        (None, None) => None,
    };
    let from: Option<(Vec<Event>, u64, Vec<Key>, Vec<u32>)> = from_at.map(|(nodes, id)| {
        let (line, events) = stop_of(&nodes, id);
        let news = line["news"].as_array().map(|v| v.iter().filter_map(|x| x.as_str()).map(Key::parse).collect()).unwrap_or_default();
        let ever = line["ever"].as_array().map(|v| v.iter().filter_map(|x| x.as_u64()).map(|x| x as u32).collect()).unwrap_or_default();
        (events, line["frame"].as_u64().unwrap(), news, ever)
    });
    // what was owned on the way to the stop the run starts at (its line's "ever"; --ever N,N for a stop of a run
    // before lines held it): owned again, it is no gain
    let mut from_ever: Vec<u32> = opt("--ever").map(|t| t.split(',').filter_map(|x| x.parse().ok()).collect()).unwrap_or_default();
    let (root_events, root_frame, from_news) = match from {
        Some((ev, f, n, e)) => {
            from_ever.extend(e);
            (ev, Some(f), n)
        }
        None => (root_events, None, Vec::new()),
    };
    let t0 = Instant::now();
    let mut e = host
        .boot(&BootCfg { seed: 1, events: root_events.clone(), frame: root_frame, dump_locals: true, lean: true, out_dir: eng_dir(&out) })
        .expect("the boot");
    let st = e.status().clone();
    eprintln!("boot: frame {} room {} idle {:?} in {:.1} s", st.frame, st.room, st.idle, t0.elapsed().as_secs_f64());
    let mut s = Search {
        log: Some(std::fs::File::create(out.join("nodes.jsonl")).expect("nodes.jsonl")),
        offers: Some(std::fs::File::create(out.join("offers.jsonl")).expect("offers.jsonl")),
        rung_log: Some(std::fs::File::create(out.join("rung-stops.jsonl")).expect("rung-stops.jsonl")),
        top_path: Some(out.join("top.jsonl")),
        t0: Some(t0),
        live_max: LIVE_MAX.max(8 * cfg.workers),
        ..Default::default()
    };
    s.gains_only = opt("--lines").as_deref() == Some("gains");
    s.flag_lines = a.iter().any(|x| x == "--flag-lines");
    s.shares = a.iter().any(|x| x == "--shares");
    s.motion_once = a.iter().any(|x| x == "--motion-once");
    s.born_first = a.iter().any(|x| x == "--born-first");
    s.walk_born = a.iter().any(|x| x == "--walk-born");
    s.said_flags = a.iter().any(|x| x == "--said-flags");
    s.count_lines = a.iter().any(|x| x == "--count-lines");
    s.stay_lines = a.iter().any(|x| x == "--stay-lines");
    s.go = a.iter().any(|x| x == "--go");
    let mut loaded: Vec<String> = Vec::new();
    if s.go {
        if let Ok(d) = std::env::var("FMI_GAME_SCRIPTS") {
            s.mentions = mentions_of(Path::new(&d));
        }
        // --edges FILE: the ways from room to room an earlier run saw (its OUT/edges.jsonl)
        let edges_file = opt("--edges").or_else(|| from_run.as_ref().map(|d| d.join("edges.jsonl").to_string_lossy().into_owned()).filter(|p| Path::new(p).exists()));
        if let Some(p) = edges_file {
            for v in std::fs::read_to_string(&p).expect("--edges").lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()) {
                if let (Some(a), Some(b), Some(h)) = (v["from"].as_i64(), v["to"].as_i64(), v["how"].as_str().and_then(hits_of)) {
                    if v["failed"].as_bool() == Some(true) {
                        // a way that run found false
                        s.edges.remove(&(a as i32, b as i32));
                        *s.dead.entry((a as i32, b as i32, v["how"].as_str().unwrap_or("").to_string())).or_default() += 1;
                    } else if s.dead.get(&(a as i32, b as i32, v["how"].as_str().unwrap_or("").to_string())).copied().unwrap_or(0) < 2 {
                        s.edges.entry((a as i32, b as i32)).or_insert(h);
                    }
                    loaded.push(v.to_string());
                }
            }
        }
        // the ways the scripts advise (an object whose code leads ego to another room), where none was seen: a click
        // on the object. Advice only: a walk that fails on one forgets it
        let seen = s.edges.len();
        for &(a, b, o) in &s.mentions.exits {
            if !s.dead.contains_key(&(a, b, format!("t{o}"))) {
                s.edges.entry((a, b)).or_insert_with(|| vec![Hit::Object(o)]);
            }
        }
        eprintln!("mentions: {} objects, {} flags named in rooms' scripts; {} ways from room to room seen, {} more the scripts advise", s.mentions.objs.len(), s.mentions.bits.len(), seen, s.edges.len() - seen);
        trace(&format!("mentions: {} variables a few rooms' scripts count on", s.mentions.vars.len()));
    }
    // --toggles FILE: the flags an earlier run saw unset again (its OUT/toggles.txt)
    let toggles_file = opt("--toggles").or_else(|| from_run.as_ref().map(|d| d.join("toggles.txt").to_string_lossy().into_owned()).filter(|p| Path::new(p).exists()));
    if let Some(p) = toggles_file {
        for l in std::fs::read_to_string(&p).expect("--toggles").lines().filter(|l| !l.is_empty()) {
            s.toggles.insert(Key::parse(l));
        }
        eprintln!("toggles: {} flags an earlier run saw unset again", s.toggles.len());
    }
    s.toggles_log = std::fs::File::create(out.join("toggles.txt")).ok();
    if let Some(f) = s.toggles_log.as_mut() {
        let mut ks: Vec<String> = s.toggles.iter().map(|k| format!("{k}")).collect();
        ks.sort();
        for k in ks {
            let _ = writeln!(f, "{k}");
        }
    }
    s.edges_log = std::fs::File::create(out.join("edges.jsonl")).ok();
    if let Some(f) = s.edges_log.as_mut() {
        for l in &loaded {
            let _ = writeln!(f, "{l}");
        }
    }
    if let Some(p) = opt("--guide") {
        // the guide's keys (fmi guide map): the variables among them are the ones the game's code tests
        let g: Value = serde_json::from_str(&std::fs::read_to_string(&p).expect("--guide")).expect("the guide's JSON");
        s.policy.guide = g["keys"].as_object().expect("the guide's keys").keys().map(|k| Key::parse(k)).collect();
        eprintln!("guide: {} keys, {} of them variables", s.policy.guide.len(), s.policy.guide.iter().filter(|k| k.kind() == K_VAR).count());
    }
    // --counts-world (with --go): the counts the scripts of a few rooms keep (`Mentions::vars`: added to or taken
    // from in at most GLOBAL_ROOMS rooms' scripts, and read by a room's own scripts) are of the world too, beside
    // the guide's variables. The guide is regressed from a goal through the scripts' writers and misses a count
    // whose effect is an object taken by an indirect write (a crewman tickled twice drops a jug: its count was
    // in no guide, so the first tickle was no stop)
    if a.iter().any(|x| x == "--counts-world") {
        let before = s.policy.guide.len();
        for &x in s.mentions.vars.keys() {
            s.policy.guide.insert(Key::var(x as u64));
        }
        eprintln!("counts: {} variables the scripts of a few rooms count, {} of them not in the guide", s.mentions.vars.len(), s.policy.guide.len() - before);
    }
    if let Some(p) = opt("--rungs") {
        s.rungs = rungs_of(&p);
    }
    for w in opt("--watch").unwrap_or_default().split(',').filter(|x| !x.is_empty()) {
        let (k, v) = w.split_once('=').expect("--watch KEY=V");
        s.watch.push((Key::parse(k), v.parse().expect("a value"), None));
    }
    // --until K=V,..: watched values whose first holding ends the run (an evaluation's stop rule, as --minutes)
    for w in opt("--until").unwrap_or_default().split(',').filter(|x| !x.is_empty()) {
        let (k, v) = w.split_once('=').expect("--until KEY=V");
        let (k, v): (Key, Val) = (Key::parse(k), v.parse().expect("a value"));
        if !s.watch.iter().any(|x| x.0 == k && x.1 == v) {
            s.watch.push((k, v, None));
        }
        s.until.push((k, v));
    }
    let o = e.obs(&NAMES).expect("the root's dump");
    let r = read(String::new(), Vec::new(), &o, st.clone());
    let ego = r.facts.val(Key::pack(K_EGO, 0, 0, 0));
    s.root_owned = r.facts.iter().filter(|p| p.0.kind() == K_OWNER && p.1 == ego).map(|p| p.0.a() as u32).collect();
    s.root_owned.extend(from_ever.iter().copied());
    let gline = line_of(&[], &[]);
    let line = if s.flag_lines {
        let mut h = DefaultHasher::new();
        (gline, 0usize).hash(&mut h);
        h.finish()
    } else {
        gline
    };
    for p in r.facts.iter().filter(|p| world_kind(p.0) || p.0.kind() == K_VAR) {
        s.seen.insert((line, p.0, p.1));
        s.seen_at.insert((line, r.room, p.0, p.1));
    }
    for &t in &r.vis {
        s.sees.insert((line, r.room, t));
    }
    s.ifaces.insert((line, r.iface));
    let flags0 = r.facts.iter().filter(|p| p.0.kind() == K_BIT && p.1 != 0).count() as u32;
    s.counts.insert((line, flags0));
    s.boxes.insert((line, r.room, r.walkbox));
    s.cams.insert((line, r.room, r.cam));
    s.rooms.push(r.room);
    for g in s.rungs.iter_mut().filter(|g| g.conds.is_empty()) {
        g.at = Some((0, 0.0));
    }
    // what the starting stop won (its line of nodes.jsonl, or --news KEY,KEY for a stop of an older run): it is
    // walked from like any stop that begins a line
    let root_news: Vec<Key> = match opt("--news") {
        Some(t) => t.split(',').filter(|x| !x.is_empty()).map(Key::parse).collect(),
        None => from_news.clone(),
    };
    if let Some(f) = s.log.as_mut() {
        let mut ever0: Vec<u32> = s.root_owned.iter().copied().collect();
        ever0.sort_unstable();
        let first = json!({"id": 0, "parent": Value::Null, "depth": 0, "frame": st.frame, "room": r.room, "level": 0, "owned": 0, "class": 0, "how": "", "novel": [], "t": 0.0,
            "news": root_news.iter().map(|k| format!("{k}")).collect::<Vec<_>>(),
            "ever": ever0,
            "events": root_events.iter().map(fmi_explore::events::event_json).collect::<Vec<Value>>()});
        let _ = f.write_all(format!("{first}\n").as_bytes());
    }
    s.nodes.push(Node { events: root_events, parent: usize::MAX, frame: st.frame, depth: 0, ever: Vec::new(), level: 0, line, room: r.room, pending: Vec::new(), inside: false, flags: flags0, fline: 0, gline, news: root_news, walk: false, win: false, carry: false, led: false, born: false });
    s.queues.push(Default::default());
    s.stacks.push(Vec::new());
    s.queues[0][0].push_back(0);
    s.live.insert(0, e);
    let limit = Duration::from_secs_f64(minutes * 60.0);
    // --stall M: the run ends when the highest level was not raised for M minutes (a driver goes on from its top
    // stop, as a run begun from the stop it ended at). Only once the level was raised over the first stop's: in a run
    // that has gained nothing the top stop is a line begun next to its first stop, and a run from there would do
    // again what this one did (full1, 2026-11-13: seven legs of an hour each began within 44 frames of one another,
    // and each won the same two or three fights and lost them at its end)
    let stall: Option<Duration> = opt("--stall").map(|v| Duration::from_secs_f64(v.parse::<f64>().expect("--stall M") * 60.0));
    // --stops N: the run ends after N stops are expanded (a measure that is the same on any machine)
    let max_stops: Option<u64> = opt("--stops").map(|v| v.parse().expect("--stops N"));
    let (mut last, mut printed) = (0u64, Instant::now());
    type Job = (usize, Option<Box<dyn Engine>>, Option<Arc<Mutex<Box<dyn Engine>>>>, Vec<Event>, Vec<Event>, u64, u32, Vec<(Key, Val)>, bool, Vec<Key>);
    type Done = (usize, Result<(Box<dyn Engine>, Out), String>, bool);
    while t0.elapsed() < limit && max_stops.map_or(true, |n| s.expanded < n) && !s.done {
        if let Some(m) = stall {
            if s.top > 0 && s.top_at.unwrap_or(t0).elapsed() > m {
                eprintln!("[{:7.1}s] STALL: the highest level ({}) not raised for {:.0} minutes: the run ends", t0.elapsed().as_secs_f64(), s.top, m.as_secs_f64() / 60.0);
                break;
            }
        }
        // a batch: the next stops in the queues' order, each with its kept engine if it has one
        let mut batch: Vec<Mutex<Option<Job>>> = Vec::new();
        let at_once = if cfg.at_once > 0 { cfg.at_once } else { (cfg.workers / cfg.slices.max(1)).max(1) };
        let room_left = max_stops.map_or(usize::MAX, |n| (n - s.expanded) as usize);
        while batch.len() < (at_once * cfg.batch).min(room_left) {
            // the highest level first and in it the first queue, over the first queues of every level; then the same
            // over the later ones
            // with --flag-lines the newest line's stops go before any older line's (a line is as new as the stop
            // with a new set of flags it began at): of that line the stack's first (its newest stop: the walk
            // through the rooms), then queue 1, 2 and 3, each its earliest stop first. An older line's stops, the
            // stack's too, wait until the newest line has none: when no new set of flags is left to follow, what
            // the newest line saw on its way is gone through, not the walks of the lines left behind
            let newest = s.flag_lines;
            // --shares (with --flag-lines): the lines take turns. A line here is the stops of one level that one
            // stop with a new set of flags began. The newest line of the highest level has every second turn, the
            // next every fourth, and so on: a turn's line is named by the trailing zeros of the turn's number (past
            // the last line, the first). The search still follows what is newest; a line that leads nowhere (a room
            // with much to say, entered on the way) cannot keep the others waiting for good
            let shared = if s.shares && newest {
                s.tick += 1;
                let mut lines: Vec<(usize, usize)> = Vec::new();
                for l in 0..s.queues.len() {
                    for &id in s.stacks.get(l).into_iter().flatten().chain(s.queues[l][..FIRST_QUEUES].iter().flatten()) {
                        lines.push((l, s.nodes[id].fline));
                    }
                }
                lines.sort_unstable_by(|a, b| b.cmp(a));
                lines.dedup();
                if s.stay_lines {
                    // (stable: the lines whose flags did not stay after the others, each part newest first)
                    lines.sort_by_key(|x| s.junk.contains(&x.1));
                }
                let r = s.tick.trailing_zeros() as usize;
                lines.get(if r < lines.len() { r } else { 0 }).copied()
            } else {
                None
            };
            // and before any line's turn, a stop at the end of a walk the search made (the newest first): the scripts
            // name that room for what the line's first stop won, so it is looked at once before the line goes on.
            // (Such a stop is of the line of the stop that won, which is an old line as soon as another begins.)
            // The same for a stop made inside a window that is new by a guide variable's value: a count the game
            // keeps of what was done while the window was open. Such a stop is of the line of the stop the window
            // was found at, which may have no turn for long, and in its third queue (an input, the same inside its
            // window, the same again inside that one's, and then another: each stop new by the count alone).
            // (A stop new there by a key of the world has the stack of its line; put first as well, such stops
            // took half the turns and the check from vm24's stop held nothing in 30 stops.)
            // And for a stop that carries into another room what the stop before it held only for a while: on
            // its line's stack it waited (3 such stops, none expanded in 32) while what it carried ran out.
            let walked = if s.shares && newest {
                // (the stop, its level, its queue or usize::MAX for the stack, its place there)
                let mut best: Option<(usize, usize, usize, usize)> = None;
                for l in 0..s.queues.len() {
                    for (i, &id) in s.stacks.get(l).into_iter().flatten().enumerate() {
                        if (s.nodes[id].walk || s.nodes[id].win) && best.map_or(true, |b| id > b.0) {
                            best = Some((id, l, usize::MAX, i));
                        }
                    }
                    for c in 0..FIRST_QUEUES {
                        for (i, &id) in s.queues[l][c].iter().enumerate() {
                            if (s.nodes[id].walk || s.nodes[id].win) && best.map_or(true, |b| id > b.0) {
                                best = Some((id, l, c, i));
                            }
                        }
                    }
                }
                best
            } else {
                None
            };
            // and before those, a stop that carries what lasts only for a while (or where it was passed on): the
            // earliest first. (The newest first left the first to reach a room unexpanded while later ones came.)
            let carrying = if s.shares && newest {
                let mut best: Option<(usize, usize, usize, usize)> = None;
                for l in 0..s.queues.len() {
                    for (i, &id) in s.stacks.get(l).into_iter().flatten().enumerate() {
                        if s.nodes[id].carry && best.map_or(true, |b| id < b.0) {
                            best = Some((id, l, usize::MAX, i));
                        }
                    }
                    for c in 0..FIRST_QUEUES {
                        for (i, &id) in s.queues[l][c].iter().enumerate() {
                            if s.nodes[id].carry && best.map_or(true, |b| id < b.0) {
                                best = Some((id, l, c, i));
                            }
                        }
                    }
                }
                best
            } else {
                None
            };
            let walked = carrying.or(walked);
            let first = if let Some((_, l, c, i)) = walked {
                if c == usize::MAX {
                    Some(s.stacks[l].remove(i))
                } else {
                    s.queues[l][c].remove(i)
                }
            } else if let Some((l, f)) = shared {
                // of that line: the stack's newest stop, one the game led to another room before the others (with
                // no input ego stood in another room: an encounter, a scene the game began; what the game starts
                // by itself is followed before the rooms the inputs lead to), then queues 1, 2 and 3, each its
                // earliest
                let nodes = &s.nodes;
                // --born-first: of that line the results of the stop that began it, in that stop's room, before
                // its other stops (the stack's, then the queues'). What a new set of flags changes is looked for
                // first where it was set: the line's stack is the walk through the rooms, every one of them new
                // again, and the stop two clicks before a room no line had entered (new in its line by a walk
                // box, the third queue) waited for that walk to end
                let news = if s.born_first {
                    match s.stacks.get(l).and_then(|st| (0..st.len()).filter(|&i| nodes[st[i]].fline == f && nodes[st[i]].born).max_by_key(|&i| (nodes[st[i]].led, st[i]))) {
                        Some(i) => Some(s.stacks[l].remove(i)),
                        None => (0..FIRST_QUEUES).find_map(|c| {
                            let i = (0..s.queues[l][c].len()).find(|&i| nodes[s.queues[l][c][i]].fline == f && nodes[s.queues[l][c][i]].born)?;
                            s.queues[l][c].remove(i)
                        }),
                    }
                } else {
                    None
                };
                let nodes = &s.nodes;
                if news.is_some() {
                    news
                } else {
                match s.stacks.get(l).and_then(|st| (0..st.len()).filter(|&i| nodes[st[i]].fline == f).max_by_key(|&i| (nodes[st[i]].led, st[i]))) {
                    Some(i) => Some(s.stacks[l].remove(i)),
                    None => (0..FIRST_QUEUES).find_map(|c| {
                        let i = (0..s.queues[l][c].len()).find(|&i| nodes[s.queues[l][c][i]].fline == f)?;
                        s.queues[l][c].remove(i)
                    }),
                }
                }
            } else {
                (0..s.queues.len()).rev().find_map(|l| {
                if !newest {
                    if let Some(id) = s.stacks.get_mut(l).and_then(|st| st.pop()) {
                        return Some(id);
                    }
                    return s.queues[l][..FIRST_QUEUES].iter_mut().find_map(|c| c.pop_front());
                }
                let nodes = &s.nodes;
                // (the line's age, the queue, the order in it) of the best stop in the stack and in each queue
                let top = s.stacks.get(l).and_then(|st| {
                    (0..st.len()).max_by_key(|&i| (nodes[st[i]].fline, st[i])).map(|i| ((std::cmp::Reverse(nodes[st[i]].fline), 0usize, std::cmp::Reverse(st[i]).0), usize::MAX, i))
                });
                let rest = (0..FIRST_QUEUES)
                    .flat_map(|c| (0..s.queues[l][c].len()).map(move |i| (c, i)))
                    .map(|(c, i)| {
                        let id = s.queues[l][c][i];
                        ((std::cmp::Reverse(nodes[id].fline), c + 1, id), c, i)
                    })
                    .min_by_key(|x| x.0);
                // the stack's best is its newest stop of the newest line; a queue's best its earliest of that line
                let pick = match (top, rest) {
                    (Some(t), Some(r)) => {
                        if (t.0 .0, t.0 .1) <= (r.0 .0, r.0 .1) {
                            t
                        } else {
                            r
                        }
                    }
                    (Some(t), None) => t,
                    (None, Some(r)) => r,
                    (None, None) => return None,
                };
                if pick.1 == usize::MAX {
                    Some(s.stacks[l].remove(pick.2))
                } else {
                    s.queues[l][pick.1].remove(pick.2)
                }
                })
            };
            // the last queue: of the highest level that has one, the stop that holds the most flags (the earliest of
            // them): what is won one flag at a time is carried on by the stop that has won most
            let nodes = &s.nodes;
            let last = || {
                s.queues.iter_mut().rev().find_map(|q| {
                    let c = &mut q[QUEUES - 1];
                    let i = (0..c.len()).max_by_key(|&i| (nodes[c[i]].flags, std::cmp::Reverse(c[i])))?;
                    c.remove(i)
                })
            };
            let Some(id) = first.or_else(last) else { break };
            let live = s.live.remove(&id);
            // the nearest stop on its way whose engine is kept, and the events since (for a boot: the whole movie)
            let (mut src, mut since, mut movie) = (None, Vec::new(), Vec::new());
            if live.is_none() {
                let mut chain = vec![id];
                loop {
                    let p = s.nodes[*chain.last().unwrap()].parent;
                    if p == usize::MAX {
                        break;
                    }
                    s.stamp += 1;
                    let now = s.stamp;
                    if let Some(x) = s.parents.get_mut(&p) {
                        x.1 = now;
                        src = Some(x.0.clone());
                        break;
                    }
                    chain.push(p);
                }
                since = chain.iter().rev().flat_map(|&i| s.nodes[i].events.iter().cloned()).collect();
                movie = s.movie(id);
            }
            let n = &s.nodes[id];
            batch.push(Mutex::new(Some((id, live, src, since, movie, n.frame, n.level, n.pending.clone(), n.inside, if s.go { n.news.clone() } else { Vec::new() }))));
        }
        if batch.is_empty() {
            break;
        }
        let snap = Snap { motion: &s.motion, wins: &s.wins, noise: &s.noise, lclocks: &s.lclocks, policy: &s.policy, ambient: &s.ambient, mentions: &s.mentions, edges: &s.edges, go: s.go };
        // side by side: each stop explored, from the search's state at the batch's start
        let n = batch.len();
        let t_batch = Instant::now();
        let done: Vec<Mutex<Option<Done>>> = (0..n).map(|_| Mutex::new(None)).collect();
        let next = AtomicUsize::new(0);
        std::thread::scope(|sc| {
            for _ in 0..at_once.min(n) {
                sc.spawn(|| loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= n {
                        break;
                    }
                    let (id, engine, src, since, movie, frame, level, pending, inside, news) = batch[i].lock().unwrap().take().unwrap();
                    let boot = || host.boot(&BootCfg { seed: 1, events: movie.clone(), frame: Some(frame), dump_locals: true, lean: true, out_dir: eng_dir(&cfg.out) });
                    let mut booted = false;
                    let e = match (engine, src) {
                        (Some(e), _) => Ok(e),
                        (None, Some(src)) => {
                            // the fork under the kept engine's lock (other stops of the batch come of it too), the
                            // events since on the fork
                            let forked = src.lock().unwrap().fork();
                            match forked.and_then(|b| replay(b, &since, frame)) {
                                Ok(b) => Ok(b),
                                Err(_) => {
                                    booted = true;
                                    boot()
                                }
                            }
                        }
                        (None, None) => {
                            booted = true;
                            boot()
                        }
                    };
                    let r = match e {
                        Ok(mut e) => match { trace(&format!("node {id}: explore")); let r = explore(&mut e, level, &pending, inside, &news, &snap, &cfg); trace(&format!("node {id}: explored")); r } {
                            Ok(out) => Ok((e, out)),
                            Err(x) => {
                                e.quit();
                                Err(x.to_string())
                            }
                        },
                        Err(x) => Err(format!("boot: {x}")),
                    };
                    *done[i].lock().unwrap() = Some((id, r, booted));
                });
            }
        });
        let t_explored = t_batch.elapsed().as_secs_f64();
        // in the batch's order: committed; the expanded stop's engine is kept for the stops that come of it
        for d in done {
            let (id, r, booted) = d.into_inner().unwrap().expect("every stop of the batch explored");
            s.boots += booted as u64;
            s.expanded += 1;
            match r {
                Ok((e, out)) => {
                    let (dead, before) = (out.dead, s.nodes.len());
                    s.commit(id, out);
                    s.stamp += 1;
                    s.parents.insert(id, (Arc::new(Mutex::new(e)), s.stamp));
                    // At a dead end one click is given: the first stop an input of it came to (kept in the last
                    // queue: nothing was new there) is expanded next, before any line's turn. Once: where that stop
                    // is a dead end too, no more is given
                    if dead && !s.given.contains(&id) {
                        if let Some(c) = (before..s.nodes.len()).find(|&c| !s.nodes[c].events.is_empty()) {
                            let l = s.nodes[c].level as usize;
                            for q in s.queues[l].iter_mut() {
                                q.retain(|&x| x != c);
                            }
                            s.stacks[l].retain(|&x| x != c);
                            s.stacks[l].push(c);
                            s.nodes[c].win = true;
                            s.given.insert(c);
                            eprintln!("[{:7.1}s] DEAD END at node {id} (room {}): one click given, node {c} next", t0.elapsed().as_secs_f64(), s.nodes[id].room);
                        }
                    }
                }
                Err(x) => eprintln!("node {id}: {x}"),
            }
        }
        let t_committed = t_batch.elapsed().as_secs_f64();
        // the engines used longest ago go when there are too many
        while s.parents.len() > s.live_max {
            let Some(old) = s.parents.iter().min_by_key(|x| x.1 .1).map(|x| *x.0) else { break };
            if let Some((a, _)) = s.parents.remove(&old) {
                if let Ok(m) = Arc::try_unwrap(a) {
                    m.into_inner().unwrap().quit();
                }
            }
        }
        trace(&format!("batch of {n}: explored in {t_explored:.1} s, committed in {:.1} s, engines in {:.1} s", t_committed - t_explored, t_batch.elapsed().as_secs_f64() - t_committed));
        s.wall[0] += t_explored;
        s.wall[1] += t_committed - t_explored;
        s.wall[2] += t_batch.elapsed().as_secs_f64() - t_committed;
        if printed.elapsed() >= Duration::from_secs(10) {
            s.progress(&mut last, printed.elapsed().as_secs_f64());
            printed = Instant::now();
        }
    }
    s.progress(&mut last, printed.elapsed().as_secs_f64());
    let live: Vec<Box<dyn Engine>> = s.live.drain().map(|x| x.1).collect();
    for e in live {
        e.quit();
    }
    for (_, (a, _)) in s.parents.drain() {
        if let Ok(m) = Arc::try_unwrap(a) {
            m.into_inner().unwrap().quit();
        }
    }
    let depth = s.nodes.iter().map(|n| n.depth).max().unwrap_or(0);
    let mut own_slots: Vec<i32> = s.own_slots.iter().copied().collect();
    own_slots.sort_unstable();
    let sum = json!({"seconds": t0.elapsed().as_secs_f64(), "workers": cfg.workers, "kept": s.nodes.len(), "by_queue": s.kept,
        "expanded": s.expanded, "trials": s.trials, "skipped": s.skipped, "refused": s.refused, "not_idle": s.not_idle, "rooms": s.rooms,
        "max_depth": depth, "boots": s.boots, "top_level": s.top,
        "trial_seconds": {"fork": s.phase[0], "inputs": s.phase[1], "run": s.phase[2], "read": s.phase[3], "quit": s.phase[4]}, "trial_frames": s.frames,
        "batch_seconds": {"explore": s.wall[0], "commit": s.wall[1], "engines": s.wall[2]},
        "explore_seconds": {"waits": s.xt[0], "round": s.xt[1], "windows": s.xt[2], "after_waits": s.xt[3], "walks": s.xt[4], "one_by_one": s.xt[5], "side_by_side": s.xt[6]},
        "commit_seconds": {"world": s.ct[0], "flags": s.ct[1], "atoms": s.ct[2], "new": s.ct[3], "cell": s.ct[4], "kept": s.ct[5]},
        "queued": s.queues.iter().map(|q| q.iter().map(|c| c.len()).collect::<Vec<_>>()).collect::<Vec<_>>(),
        "iface_vars": s.policy.iface_vars.len(), "clocks": s.policy.clocks.len(), "capped": s.policy.capped.len(), "own_slots": own_slots, "expires": s.expires.len(),
        "ends": s.ends.iter().map(|(n, x)| json!([n, x])).collect::<Vec<Value>>(),
        "rungs": s.rungs.iter().map(|g| json!({"rung": g.name, "node": g.at.map(|x| x.0), "seconds": g.at.map(|x| x.1)})).collect::<Vec<Value>>(),
        "watch": s.watch.iter().map(|(k, v, at)| json!({"key": format!("{k}"), "value": v, "node": at.map(|x| x.0), "seconds": at.map(|x| x.1)})).collect::<Vec<Value>>()});
    std::fs::write(out.join("summary.json"), format!("{sum}\n")).expect("summary.json");
    println!("{sum}");
}
