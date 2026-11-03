# E011: Everything that runs in Rust, each part at its measured floor

Date planned: 2026-11-02
Research question it serves: RQ1 (completion from the game files alone, when the scope cannot be read ahead of time)

## Question

Can one try of the explorer (return to an entry, read the rooms around it, plan, play) take well under a second, and
a route replay take milliseconds, on this laptop, exactly, with everything that runs written in Rust? Exactly means:
every engine state the same as svm-w3b's (fork exactness and verify from boot), every reading byte for byte the
reference's, and every plan a valid plan of the same task. And how close does each part get to its measured floor?

This is D7 carried to the whole system. The PI, 2026-11-02: "EVERYTHING should be in rust" and "make this as fast as
it can possibly go; squeeze every millisecond". In the evening: "We can do everything over night ... Let's run asap."
So this plan covers the whole port, built in parallel against the interfaces and formats fixed below, with later
rounds going only to what still binds.

## Baseline

Sources:

- E009's kitchen log, `~/.cache/fast-mi-e009/archives/kitchen/log.jsonl`. It holds 15 tries, run on the VM (n2
  cores).
- E010's measurements on this laptop: `/var/tmp/fast-mi/wt-e010/experiments/E010-play-when-ready/results/`
  (`baseline-e009.txt`, `baseline-frame-costs.txt`, `baseline-tries.txt`).
- Three profiles made for this plan (measurement only, laptop):
  - `~/.cache/fast-mi-e011/profile-engine.md` (scripts and outputs in `profile-engine/`);
  - `~/.cache/fast-mi-e011/profile-reader.md` (`profile-reader/`);
  - `~/.cache/fast-mi-e011/profile-planner.md` (`profile-planner/`).

  The load average was 9 to 24 throughout, so the profiles quote CPU seconds and cycle counts where walls were
  inflated.

### A try today (E009, 15 kitchen tries, VM CPU seconds)

| part | sum | share | median, ring(33) (7 tries) | median, ring(38) (8 tries) |
|---|---:|---:|---:|---:|
| return (replay from boot) | 29.1 | 1.5% | 1.9 | 2.0 |
| idle measurement (1,600 observed frames) | 89.2 | 4.6% | 6.9 | 7.4 |
| read | 1,013.4 | 51.8% | 85.8 | 7.4 |
| plan | 752.7 | 38.5% | 90.7 | 4.3 |
| play (window search) | 72.5 | 3.7% | 6.1 | 0.2 |
| total | 1,956.9 | | 191.2 | 22.0 |

- **Wall per job.** Median 23.6 s: ring(33) 143.9 s, ring(38) 19.6 s. The longest was 388.5 s (j00000: a fresh
  ring(33) reading plus two refinements).
- **The laptop takes about 2.8 times the VM's time.** E010's baseline: the same job took 548.9 s on the laptop and
  195.0 s on the VM.
- **All 15 tries ended in the same stop:** the refinement's stop on var 194 (Lemma R's premise R0). From the fourth
  try on, the read bucket is that refinement reading: 79-92 CPU s on ring(33), 5-6 on ring(38).
- **Later E009 tries spent minutes in single windows.** Windows where the input could not be issued ran about 900
  "interface" children each, about 8 minutes per window (E010's baseline).
- **The ring(33) plan bucket.** In the median ring(33) try, 83 s lie between the planner's return and the first window
  (planner profile, section 5). They go to `strict.Reading`'s `by_name`, which decodes all 234,401 actions through
  PackedTask's LRU (75 CPU s measured). The planner itself takes 57.6 s per plan (median of E008's archive):
  - taskcore 26.7 s, of which the reductions take 11.3 s;
  - Fast Downward 26.5 s, of which the landmark graph takes 16.1 s and the search 0.74 s;
  - decoding in Python 5.5 s.

### Engine and bridge (profile-engine)

- **A frame costs about 2.4 M cycles:** 0.93 ms of engine CPU inside a command (0.9 ms in E010).
  - Game logic is 32 k cycles of that (1.2-1.3%).
  - The rest is screen presentation (35-38%), OPL synthesis (21%), FLAC decoding (16%), the mixer (6-7%) and SCUMM's
    own pixel drawing (9%).
- **A fresh start costs 0.8-1.1 s.** 0.6 s of it is the splash sleep in `engines/engine.cpp`. The rest is linking 123
  libraries and SDL's joystick scan.
- **A state dump costs 4.6-6.6 ms.** About 2.7 ms is building JSON in the engine and 2.8 ms is Python's
  `json.loads`. `facts_of` then adds 3.0 ms. The same facts as flat arrays are 9.4 KB, which the engine copies in
  0.3 µs.
- **A fork plus quit costs 2.3 ms** (E010). About 2.0 ms of it is kernel time, which grows with the process's memory
  regions at about 4 µs each (660 of them). A slim build has 90 regions and forks in 0.5-0.95 ms.
- **The window search is mostly Python.**
  - The parent's observed frame costs 10.4 ms, or 13.9 ms with fork_each 7 (E010).
  - A child observes every frame at about 10 ms of CPU.
  - Python is 68% of the window search's wall.
  - In 181 of 190 recorded windows, the chosen child was the first child that issued its input.
- **Four exact switches cut a frame to about a sixth:** no splash, no presentation, no event polling, and silent audio
  that keeps the exact sample counts and timer timing.
  - A frame then costs 0.41 M cycles (0.10 ms), and a return 0.8-0.9 s.
  - A forked child's own memory falls from 3.7 MB to 0.54 MB.
  - Exactness so far: 0 differences on the door chain (37 exact dumps), the kitchen chain (40) and a short A0 run (400
    frames). The full A0 has not been run.
- **Floors.**
  - Game logic alone is about 15 µs a frame.
  - With two exact cuts that are neither built nor proven (a mixer that only counts samples, and skipping pixel
    output), a frame would cost 30-40 µs.
  - A return by fork of a kept entry would take 1-2 ms on the slim build.

### Reader (profile-reader)

- **Full readings.** A ring(33) reading has 235,579 records.
  - The Rust reader took a median of 94.2 s of wall in E008's laptop logs. read-rust's Python side then took another
    79.8 s, which is 43-45% of read wall.
  - In the profile it took 365-422 CPU s under load (283 CPU s in E008's log), about 1.7 CPU ms per record.
- **Inside the reader.**
  - The golden-writer threads take 13.8% of the reader's CPU.
  - The worker threads take 74%, spread over:
    - interpreting the scripts, 16.5%;
    - building records, 14.2%;
    - merging paths, 12.1%;
    - turning paths into output, 11.0%;
    - forks as panics, 6.1%;
    - copying the path before every step, 5.8%.
  - Exact implementation fixes are expected to save 30-45% of the reader's CPU.
- **Repeats.** In E008:
  - 32 of 65 fresh ring(33) readings and 64 of 132 refinements were exact repeats;
  - boot and j00089 gave byte-identical records;
  - two refinements from different entries shared 96% of their records, and a job memo would have reused about 68%
    of the job-seconds.
- **Verb duplicates.** 58.9% of records duplicate another verb's record. Using that needs a proof first.

### Planner (profile-planner)

- **The plan above is mostly not search.** Of the 57.6 s per ring(33) plan, 0.74 s is the search itself (10
  expansions); ring(38) takes 1.34 s per plan.
- **The taskcore server reused nothing** in all 231 recorded requests.
- **Dropping the landmarks costs no plan.** Lazy GBFS with h_FF and preferred operators gave the same plans on the
  fixed tasks.
- **A Rust prototype** of that search ran in 3.5-4.1 CPU s, against 33 for Fast Downward. The h_FF floor is 60 ms per
  evaluation times 10-11 evaluations.
- **Repeats.**
  - 95% of E008's kitchen planner time recomputed a plan it already had.
  - In 82 of 97 replans, the old plan's tail was the new plan.
- **Decoding** reads the whole operator table: 5.2 s. Reading only the plan's rows takes 0.36 s.

## Architecture

### One process, one workspace

**The run is one Rust process, `fmi`.** It holds:

- the archive;
- the kept entry engines (stopped engine processes, so a return is a fork);
- the readings (mapped), their cache and the reader's job memo;
- one warm planner per reading;
- the plan cache.

Around it:

- **The reader** (`reader-scope`, already Rust) runs as a child process of `fmi` in round 1, writing its files for
  `fmi` to map. It moves into the process in a later round only if the measured process start and file writes bind.
- **Fast Downward** leaves the hot path once the Rust search passes G7. It stays only as the reference for
  `Search::FdLamaFirstCosts`.
- **Python** stays the frozen reference for differential tests and runs in no run (G12).

**The Rust workspace** is `experiments/E011-speed/rs/`. Its `Cargo.toml` (a `[workspace]` with every crate below) is
written by the integrator; in round 1 each crate builds alone with path dependencies.

| crate | built by | replaces (Python) | provides |
|---|---|---|---|
| `fmi-core` | Appendix A, verbatim | facts.py | `Key`, `Facts`, `facts_of`, `Obs`, `Domain` and the ports |
| `fmi-engine` | engine | speedrun/control.py and engine.py as used; replay.py `boot`, `compare_checkpoint`, `verify` | `Engine`, `EngineHost`, the kept-entry pool |
| `fmi-play` | play | window.py (strict path), strict.py, trace_explain.py, executor.py's input code, replay.py's per-step functions | `Player` |
| `fmi-reading` | readio | packed_reading.py's consumer side; step.py's reuse rules | `ReadingSource`, the reading cache |
| `taskcore` (gains a lib) and `fmi-plan` | plan | plan_fd.py | `Planner`, `PlannerHost` |
| `fmi-explore` and `fmi` (the binary) | explore | explore.py, step.py, guide.py, replay.Harness's planning loop, replay_part1.py's wiring | the run |

The existing crates `scumm-ir`, `scumm-machine` and `scumm-reader` (E003/scumm), and `taskcore` (E003/taskcore), are
used where they are.

**The engine** stays ScummVM's C++ with the bridge (patch 0001). A new patch, 0002, adds the lean switches and the
binary channel. It is built into its own folder.

**Only the code paths E009's and E010's settings run are ported.** Those settings are:

- in step.py's attempt wiring: taskcore served, cost caps, the reach extension, unobserved actor rooms, approach
  stones;
- the strict window search, with 3 workers, cache, fork-each 7, lite and split-wait;
- E007's refinement loop;
- E009's switches;
- play when ready.

The Rust code refuses any other combination by name.

### Why a binary channel and not FFI

The bridge runs one engine per process, and fork is the exact snapshot (A0). The binary channel keeps A0's premises
(fork-exact.md, G1-G3) as they stand. Linking ScummVM into the Rust process would gain at most the socket round trip:

- about 10-20 µs for a binary message on a Unix socket;
- against 80-150 µs today with JSON and Python.

And linking costs a lot:

- the engine's main loop would have to call back into Rust;
- each child fork would copy the whole explorer;
- A0's premises would have to be shown again.

The batched commands below (`Step`, `RunSpec`'s stops tested in the engine) take most round trips off the hot path
anyway. If, after round 1, round trips still take more than 10% of a try, FFI becomes a round-2 item.

### Memory layout

- **Facts.**
  - A `Key` is a u64 packing kind, unit and two numbers; texts with no packed form are interned.
  - Facts are a sorted `Vec<(Key, i64)>`; a diff merges two sorted runs.
  - Engine facts come from flat arrays (F2), with no string built.
  - Domain labels are sentinel values (`BELOW`, `ABOVE`, `NONE`).
- **Readings.**
  - Files are mapped, never copied.
  - An action is decoded on demand from `reading.tc` plus its line, found through the offset index F3.
  - Names are looked up by hash, so nothing decodes all 234,401 actions per reading (today's 75-114 CPU s strict
    setup).
- **Planner.**
  - Taskcore's flat integer arrays are kept across requests; a request redoes only what depends on its start, goal,
    bans and caps.
  - The relaxed task is flat arrays of unary operators.
  - Search states are packed value vectors in an arena, with a hash set of state ids.
- **Reader core.** Interned fact ids, domains cached per batch, forks returned as values instead of panics, and the
  path copied only where a read can fork (profile-reader, item 4).
- **Engine.**
  - One stopped process per kept entry: 0.54 MB private plus 0.14 MB of page tables on the slim build.
  - Entries are evicted least recently used when MemAvailable falls under the guard's floor plus 1 GB.
  - An evicted entry is rebuilt from its nearest kept ancestor by replaying the rest of its movie, which is exact by
    A0.
- **Streaming.** The reader writes its files while it reads (no golden stream, no second pass, no copy). The explorer
  appends to its archive files once per try.

### Binary formats

All formats are little-endian and versioned. The existing formats stay as they are:

- `reading.tc` (taskcore/README.md, version 1);
- the archive's JSON files (`entries.jsonl`, `cells.json`, `log.jsonl`, `stage.json`, entries' `K.json.gz`). They are
  the cold path, and the reference's tools and `explore.py verify` must keep reading them.

**F1. The engine channel**, chosen by `SPEEDRUN_WIRE=bin`. The JSON protocol stays the default, so the Python
reference runs unchanged on the new build.

- **Framing.** u32 payload length, u8 message type, then the payload. A forked child connects to the controller's
  socket on its own and sends `Hello` with its parent's pid, as today.
- **Payload encoding.**
  - Integers at their Rust width; bool as u8 0/1.
  - `Option<T>`: u8 0, or u8 1 then T.
  - `Vec<T>`: u32 count, then the items.
  - `String`: u32 byte length, then UTF-8.
  - A struct: its fields in `fmi-core`'s declaration order.
  - An enum: u8 index of the variant in declaration order, then its fields.
  - A tuple: its items in order.
- **Controller to engine.**

  | type | message | payload |
  |---|---|---|
  | 0x81 | Run | `RunSpec` |
  | 0x82 | Inputs | `Vec<Event>` |
  | 0x83 | Fork | none |
  | 0x84 | ForkEach | u32 n |
  | 0x85 | Obs | `ObsReq` |
  | 0x86 | State | none |
  | 0x87 | Holds | `Vec<Cond>` |
  | 0x88 | At | i32 x, i32 y |
  | 0x89 | SetRng | u32 |
  | 0x8A | Segment | bool |
  | 0x8B | Runs | none |
  | 0x8C | Explain | `Option<ExplainArm>` |
  | 0x8D | ExplainLog | none |
  | 0x8E | StateJson | `ObsReq` |
  | 0x8F | Quit | none |
  | 0x90 | Step | u32 frames, u8 with_runs, u8 with_explain |

- **Engine to controller.**

  | type | message | payload |
  |---|---|---|
  | 0x01 | Hello | u32 pid, u32 parent, u64 frame, u64 tick |
  | 0x02 | Stop | `Status` |
  | 0x03 | End | `Status` |
  | 0x04 | Forked | u32 pid |
  | 0x05 | Obs | `Obs` |
  | 0x06 | State | F2 block |
  | 0x07 | Holds | bool |
  | 0x08 | At | `At` |
  | 0x09 | Runs | `RunsReply` |
  | 0x0A | Explain | `ExplainReply` |
  | 0x0B | Ok | none |
  | 0x0C | Error | `String` |
  | 0x0D | Json | `String` |
  | 0x0E | StepReply | `Status`, F2 block or delta, optionally the frame's runs and explain entries |

- **Step** runs `frames` frames (1 when every frame is observed) and answers with everything the window search reads
  after a frame. That is one round trip per observed frame, where today there are four (run, runs, explain_log,
  state) and a JSON dump.
- **Left to the engine component:** the delta encoding inside StepReply (a fork child's first reply is a full block)
  and how the engine keeps the last block per connection. Both ends of the channel are that component's.

**F2. The state block**, version 1: what `facts_of` reads, as flat arrays. Order:

```text
u32 magic 0x31534D46 ("FMS1"), u32 version = 1
u64 frame, u64 tick, u64 rng
i32 room, i32 ego (VAR_EGO's value), i32 cutscene_depth
u32 n_vars;  i32 vars[n_vars]                       _scummVars, every index (facts_of drops the six clocks)
u32 n_bits;  u8  bits[(n_bits + 7) / 8]             _bitVars as the engine holds them: bit n at byte n >> 3, mask 1 << (n & 7)
u32 n_objs;  u8 owner[n_objs]; u8 state[n_objs]; u32 class[n_objs]
                                                    index = object number, 0 unused (the dump's objects are 1..n_objs-1);
                                                    class bit c-1 = class c
u32 n_slots; { i32 slot, i32 number, i32 where, i32 status, i64 code, i32 frozen }[n_slots]   live slots, slot order
u8  has_override; { i32 where, i32 number, i64 code, u8 stale }                              the top level's override
u32 n_verbs; i32 verb_id[n_verbs]                   visible verbs, slot order
```

`fmi-engine` turns a block into `Facts` exactly as `fmi_core::facts_of` turns the full dump. G4 checks it.

**F3. `reading.idx`**, version 1, written by `reader-scope` next to the reading's files:

```text
u32 magic "FMRX", u32 version = 1
u32 n_actions, u32 n_lists
u64 action_off[n_actions + 1]       byte offset of each line of reading.actions.jsonl; the last is the file's size
u64 list_off[n_lists + 1]           the same for reading.lists.jsonl
u64 tc_word[n_actions]              word offset of each action's length word in reading.tc (packed_reading.Packed.offsets)
u32 n_names; { u64 hash, u32 action }[n_names]
                                    every action, sorted by (hash, action); hash = FNV-1a 64 of the action's name
                                    ("label#rec", or the label when rec is NONE, as PackedTask._action names it);
                                    by_name = the greatest action whose name, checked, is equal
u8  sha1_tc[20], u8 sha1_actions[20], u8 sha1_lists[20]
```

**F4. The plan record**, the plan cache: `~/.cache/fast-mi-e011/plans/<key>.plan`.

- **The key** is sha256 over:
  - the reading's id (sha1 of `reading.tc`);
  - the goal;
  - the start facts, as sorted texts;
  - the bans;
  - the cost caps, restricted to the actions they lower;
  - the search;
  - the start check.
- **The file** is magic `"FMPL"`, u32 version 1, the 32-byte key, then the `PlanResult` in F1's encoding. Every `Key` is
  written as its text, because interned keys are only valid within one process.

**F5. The reader's job memo.** Its store is internal to the readcore component. It must hold its keys as text or carry
its own id table.

### Paths and the machine

- **Builders' worktrees.**
  - Each component gets `/var/tmp/fast-mi/wt-e011-<key>` on branch `e011-<key>`, made from `e011`, with symlinks
    mirroring wt-e011's: `out`, `build`, `data`, `.venv`, `game` and the E001 key files point into the main checkout.
  - Cargo builds into each worktree's own `target/`, which is on `/`, not `/home`.
  - `reader-scope` finds the game data from the repository root it was built under (`scumm-ir` game.rs), so build it in
    the worktree.
- **The engine build.**
  - It goes in `/var/tmp/fast-mi/svm-e011/` (on `/`): a copy of svm-w3b's source with patch 0002 applied, built with
    plain `make` after the profile's slim configure line.
  - It is never built over svm-w3b, and never with a reset.
- **Outputs** go under `~/.cache/fast-mi-e011/`: readings, the reading cache, the memo store, plans, archives and
  benchmark data.
  - /home stays above 8 GB free; it had 12 GB free when this was planned.
  - Caches are evicted by E009's level: the floor plus twice the largest reading.
- **Heavy work.** Anything above about 1 GB runs under `flock /var/tmp/fast-mi/heavy.lock` and the guard
  (`~/.cache/fast-mi-e007/guard.sh`) at nice 10. That covers ring(33) readings, an explorer holding a ring(33) planner,
  and B0's ring(33) tries.
- **Environment.** `~/.cache/fast-mi-e009/env.sh` gives the reference's tools:
  - `FAST_MI_SCUMMVM_BIN`: svm-w3b's build;
  - `DOWNWARD_DIR`;
  - `TASKCORE_BIN`;
  - `READER_SCOPE_BIN`: E009's binary, built from 2a3cf8a1.

  The new binaries are named in `~/.cache/fast-mi-e011/env.sh`, which the integrator writes.
- **Seed 1 only.** Never 836557856; seeds 31-35 are held out.
- **No exploration runs** before the kitchen check.

## Stages: the order and what each buys

The stages follow E009's measured shares (15 tries, 1,957 CPU s on the VM). All six components start together in
round 1. The order says:

- which gates the integrator merges first;
- which parts get round 2's builders, if the round-1 numbers leave them binding.

| stage | share now | components | what it buys (profile numbers) |
|---|---:|---|---|
| 1. reading | 52% | readio, readcore | no Python side (−43-45% of read wall); no golden stream (−13.8% of reader CPU); exact reading cache (half of E008's readings were repeats); implementation fixes (−30-45% of reader CPU); job memo across readings (−68% of a refinement's job time, −100% for a fresh reading from another entry that differs only on K and task facts) |
| 2. planning | 38% | plan, and readio's index | strict setup 83 s → under 0.05 s per ring(33) plan; warm task (−3.2 s load per plan); Rust search on the reduced task in memory (−17.6 s of files and FD, −16.1 s of landmarks per ring(33) plan); decoding only the plan's rows (−5.5 s); plan cache (95% of E008's planner time was repeats); old tail first (82 of 97 replans) |
| 3. engine and play | 10%, plus the 8-minute interface windows | engine, play | lean switches (5-9× cheaper frames); binary state and Step (observed frame 10.4 ms → about 0.15 ms); return by fork (about 2 s → milliseconds); play when ready (one try where 181 of 190 windows chose the first child); idle measurement on the fast engine (7 s → about 0.3 s) |
| 4. explorer and harness | the glue | explore | one resident process: no process start, Game.load (3.3 s) or reuse_keys_of (9.6 s per reading) per try; entries, readings and planners stay warm from try to try |

Merge order: `fmi-core` first, then stage 1, 2 and 3. Explore goes last, because its end-to-end gates (G10-G12) need
all the others.

## Components (round 1)

There are six builders, one per component, each on its own branch. No two components edit the same file. The one
exception is `experiments/E011-speed/rs/fmi-core/`: every builder that needs it creates it byte for byte from
Appendix A and never edits it. Git merges identical additions without a conflict.

A component that needs another's port builds and tests against a test double behind the port's trait. Every
component's tests run alone, with no other new component.

### `engine`: the engine's fast path and its Rust driver

**Files**
- `patches/0002-scumm-speedrun-lean.patch` (new; applies on top of 0001; every engine change of E011 goes here).
- `experiments/E011-speed/engine/`: the build script, `WIRE.md` (F1 and F2 as built), the check scripts.
- `experiments/E011-speed/rs/fmi-engine/`.
- `rs/fmi-core` (Appendix A).

**Method**

1. **The lean switches, as bridge settings.** `SPEEDRUN_LEAN=1` turns on all four; each can also be turned on alone,
   for the checks. They come from the profile's `prof-switches.diff`, which measured 0 differences:
   - no splash wait;
   - no backend present (`waitForTimer` skips `updateScreen`);
   - no event poll (the bridge's input injection is unchanged);
   - silent audio. The OPL chip writes zeros but keeps its callback timing. A FLAC stream returns zeros with the
     decoder's exact per-call counts and end-of-data rule.

   The bridge's ini also gets `joystick_num=-1`.
2. **The slim configure line** (profile-engine, section 5), building into `/var/tmp/fast-mi/svm-e011/`.
3. **The binary channel F1 and the state block F2**, including `Step`. The engine keeps the last block it sent per
   connection, so a StepReply carries a delta. JSON is unchanged and stays the default.
4. **`fmi-engine`.**
   - `EngineHost::boot` spawns the engine with replay_part1.config's settings, plus `SPEEDRUN_WIRE=bin` and, when
     `lean`, `SPEEDRUN_LEAN=1`.
   - `Engine` is implemented over F1. `facts()` comes from F2 and `obs()` from the binary `Obs`.
   - The kept-entry pool maps an entry id to a stopped process, so a return is `fork()`. It evicts least recently used
     under the memory level above and rebuilds an evicted entry from its nearest kept ancestor.
   - replay.py's `boot`, `compare_checkpoint` and `verify` are ported.
   - A frame-cost bench measures E010's categories a-d, the return by fork and the replay from boot.

**Exactness**
- The switches change only what no game state reads: presentation, the synthesized samples, the event poll and the
  splash wait. Sample counts and timer timing stay as they are. G1-G3 check this.
- The binary channel encodes what the JSON encodes (G4).
- A return by fork is A0's fork.

**Python reference**
- svm-w3b's JSON dumps, through facts.py's `facts_of`.
- `fork_exact.py`.
- The profile's `scripts/chain.py dumps|compare` (exact dumps).
- E009's step test (a).

**Test alone.** Needs nothing from the other components.

**Targets** (floors in brackets)

| measure | target | floor |
|---|---|---|
| frame inside a command | ≤ 0.10 ms of engine CPU | the four switches; 15 µs of game logic |
| start to the first stop | ≤ 0.1 s of CPU | 80-90 ms on the slim build |
| fork + quit | ≤ 1.0 ms of parent CPU | 0.67-1.33 ms |
| return by fork of a kept entry | ≤ 3 ms of wall | 1-2 ms |
| door chain replayed from boot | ≤ 0.5 s of wall | |
| observed frame (`Step` with state delta and runs) | ≤ 0.15 ms | |
| full binary `Obs` | ≤ 0.1 ms | |

**Gate.** G1, G2, G3, G4 and G5, with the bench numbers reported against the targets.

### `play`: the strict matcher, the window search with play when ready, the input code

**Files**
- `experiments/E011-speed/rs/fmi-play/`.
- `rs/fmi-core` (Appendix A).
- `experiments/E011-speed/play/`: fixture capture. These are Python test tools that wrap the frozen reference by
  monkeypatching; they edit none of its files.

**What it ports**
- window.py's strict path: `Search.play_plan`, `window_strict`, `_child_strict`, `Matcher`, `EndRule`,
  `build_recs`, `macro_steps`, `checked_keys`, `chain_frames`, `source_index`, `other_path`, the alternatives, and
  the window records.
- E010's play when ready (E010's plan.md, "(1) Play when ready"; `~/.cache/fast-mi-e011/e010-play-when-ready-wip.patch`
  as the draft of it).
- strict.py, and the parts of trace_explain.py that strict.py uses.
- executor.py's input code as E009 runs it: `_wait_walk_to`, `_wait_verb_visible`, `wait_gate`, `_click_verb`.
- replay.py's per-step functions: `Input`, `step_input`, `verb_point`, `to_move`, `reads_writes`, `view`,
  `read_view`, `elsewhere`, `holds`, `extra_of`, `History`, `eval_check`, `expectations`, `match`, `unplanned`,
  `verdict`, `classify`, `predict`, `after`, `ran_on`, `dumped_slots`, `live_match`, `list_matches`, `match_list`,
  `StepLog`, `Stop`.
- `Harness.observe`, `belief`, `checkpoint` and `_set_pred`, as `Player::observe` and `PlayState`.

**Ports**
- It provides `Player`.
- It uses `Engine`, `ReadingSource` and `PlanResult`, and `scumm-ir` for trace_explain's `Code`.
- Test doubles, behind the cargo feature `json-double` and replaced at integration:
  - `JsonEngine`: the existing JSON protocol against svm-w3b.
  - `FileReading`: a reading's four files, read as packed_reading reads them.

**Exactness**
- The windows play on the engine's own states.
- The chosen try is checked by B1's matcher, as now.
- Ready mode drops only what E010 states it drops: the least Q over every fork frame. Each window's record says so
  (`"ready": true`, `"earliest": false`).

**Python reference and differential tests**
1. **Capture.** E009's step test (a) is run under a wrapper that records each window's inputs and outputs:
   - inputs: the records, members, tracked and needed sets, the plan, the parent's movie and frame;
   - outputs: the `windows.jsonl` rows, the chosen child and the outcome.

   It goes under `~/.cache/fast-mi-e011/fixtures/play/`. The reference run is also kept:
   `~/.cache/fast-mi-e009/off/step-a/jobs/ta/run/a00/windows.jsonl`.
2. **Every child.** For each of test (a)'s 502 children (4 windows), the Rust child check from the same fork frame
   gives the recorded outcome, first-event frame, end frame and Q.
3. **Strict mode.** `play_plan` on the 4 windows chooses the same children (fork frames 2728, 2796, 3091, 3204). Every
   window record is equal field for field, except times and pids.
4. **Ready mode.** The same 4 children are chosen (E010's prediction), and the tries are counted.

**Targets**
- An observed child frame takes ≤ 0.2 ms on the lean engine (≤ 1.5 ms through the JSON double).
- Test (a)'s third window takes ≤ 5 s of wall in ready mode with 3 workers. Python's strict search took 261 s on it
  (293 failed children before the chosen one, 30,806 child frames).
- A window whose first try succeeds takes ≤ 50 ms.

**Gate.** Tests 2-4 pass on svm-w3b through the double, then again on the lean build through `fmi-engine` at
integration (part of G10).

### `readio`: the reading's own files, the Rust consumer, the reading cache

**Files**
- `experiments/E003-path-through-cloud/scumm/reader/src/bin/reader_scope.rs`, and new modules under
  `scumm/reader/src/bin/reader_scope/`.
- `scumm/reader/src/packed.rs`.
- `experiments/E011-speed/rs/fmi-reading/`.
- `rs/fmi-core` (Appendix A).

Not `lib.rs` and not the reader's core: those are readcore's. `Record::py()` is the stable interface between the two,
read here only.

**Method** (profile-reader, item 1 and section 6)

1. **`reader-scope --out-dir DIR [--name reading]`** writes the reading's files while it reads:
   - `reading.json`, with the bounds from the reader's own domains;
   - `reading.actions.jsonl` and `reading.lists.jsonl`, each byte for byte what `reading_from_golden` writes from the
     golden stream;
   - the packed file, written directly as `reading.tc`;
   - `reading.reached`;
   - `reading.idx` (F3).

   `--golden` is still written when asked, for tests only.
2. **`fmi-reading`** implements `ReadingSource` over the mapped files and the index. `action(i)` and `record(i)` are
   `PackedTask._action(i)`, decoded on demand.
3. **The exact reading cache.**
   - The key is sha256 over: the reader binary's sha1, the game data's index, the rooms, the switches, the goal text,
     the fact variables, the start facts (sorted texts), the contents of the reach-also start files, and the slot
     locals.
   - The store is `~/.cache/fast-mi-e011/readings/<key>/`, evicted by E009's level.
4. **The reuse rules,** ported: E008's (task facts and slot keys) and E009's K (`step.reuse_keys_of`).

**Exactness**
- The files are the same bytes (sha1).
- A cache hit returns the files made from identical inputs. The reader is deterministic: 2-4 repeats per input gave
  the same sha1 in the profile.
- The reuse rules are E008's and E009's, unchanged.

**Python reference.** `packed_reading.reading_from_golden` on E009's binary's golden, `PackedTask._action`,
`step.reuse_keys_of` and `step.reusable`.

**Test alone.** On three readings, with E009's switches:
- ring(38) from j00091's start row (`~/.cache/fast-mi-e011/profile-planner/readings/r38/start.jsonl`);
- ring(33) from j00089's start row (`.../r33/start.jsonl`), under the heavy lock;
- the refinement j00089 [103, 194].

**Targets**
- read-rust's Python side (79.8 s median on ring(33)) is gone.
- Writing the files adds ≤ 3 s of wall to a ring(33) reading.
- The strict setup (75-114 CPU s) becomes opening the reading: ≤ 50 ms.
- Opening a cached reading: ≤ 50 ms.
- `reuse_keys`: ≤ 0.5 s (Python 9.6 s).
- `action(i)`: ≤ 20 µs.

**Gate.** G6 (the files) and G8.

### `readcore`: the reader's core, byte for byte, and the job memo across readings

**Files**
- `experiments/E003-path-through-cloud/scumm/machine/**`.
- `experiments/E003-path-through-cloud/scumm/reader/src/**`, except readio's files (`src/bin/reader_scope.rs`,
  `src/bin/reader_scope/`, `src/packed.rs`).
- `scumm/ir/**` where needed.
- `experiments/E011-speed/memo.md`.

**Method** (profile-reader, section 7)

1. **Item 4: exact implementation fixes,** expected to save 30-45% of the reader's CPU:
   - interned fact ids instead of String keys (8.5%);
   - `args:`/`pos:` domains cached per batch (8.1%);
   - forks returned as values instead of panics (6.1%);
   - the path copied only where a read can fork (5.9%);
   - jobs hashed instead of formatted with `Debug` (0.7%, plus 2.1% of the main thread);
   - less allocator churn and fewer BTree structures, where their order is not observed (20.5% and 14.5% inclusive).
2. **Item 3: the job memo across readings.** E004's `pass2_reuse` (memo.rs), carried from one reading to many under
   the five conditions of profile-reader section 7, item 3.
   - The store is on disk under `~/.cache/fast-mi-e011/memo/<key of the whole reading's inputs>/`.
   - The switch is `--memo DIR`, off by default.
   - `memo.md` is written first: E004's induction (`experiments/E004-fast-exact/pass2-reuse.md`), with condition 3
     (every consulted key has the same kind and domain, and the same start value if it is a constant) in place of
     "no widened key was consulted".

**Exactness**
- The item 4 fixes must give byte-identical output, so they need no soundness argument.
- The memo's soundness is memo.md's induction. It is checked byte for byte against the same reading without the memo.

**Python reference.** The outputs of E009's binary (2a3cf8a1), and golden.py for the kitchen golden.

**Test alone.**
- `cargo test` in `scumm/`: oracle, golden, fix_round1, havoc_cut.
- `reader-port read kitchen`.
- The three readings above, against E009's binary run back to back on the same input.

**Targets**
- A fresh ring(33) reading: ≤ 60% of the old binary's CPU, measured back to back.
- With the memo:
  - a refinement from another entry: ≤ 35% of the old job-seconds;
  - a fresh reading from another entry that differs only on K and task facts: ≤ 15% of the old wall.
- Peak memory: ≤ the old binary's plus 1.5 GB (pass2_reuse alone measured +1.4 GB).

**Gate.**
- G6 (the reader).
- Memo byte-identity on two pairs: j00000's and j00089's refinements, and the fresh boot and j00119 readings.

### `plan`: the warm planner, the Rust search, the decode and the plan caches

**Files**
- `experiments/E003-path-through-cloud/taskcore/`. `src/lib.rs` is new; `main.rs` changes only to call the library.
  New modules: `warm.rs`, `search.rs`, `decode.rs`, `predict.rs`, `cache.rs`, `tail.rs`.
- `experiments/E011-speed/rs/fmi-plan/`: `PlannerHost` and `Planner` over `ReadingSource`.
- `rs/fmi-core` (Appendix A).

**Method** (profile-planner, section 4)

1. **taskcore as a library.** The `taskcore sas|decode|validate|serve` command line keeps its outputs byte for byte.
2. **A warm task per reading.**
   - `reading.tc` is loaded once, where today each plan pays 3.2 s.
   - The goal comes with each request (today every goal starts a server).
   - Caps are keyed by the actions they lower (none were lowered in 225 requests).
   - The fold and the goal-dependent first round are kept per (reading, goal), and the per-start work is the kept
     request.
3. **The Rust search.** Lazy greedy best-first, h_FF with preferred operators, cost + 1, as in the profile's prototype
   (`profile-planner/gbfs/`). It runs on the reduced task held in memory, with no SAS text in between. That saves:
   - FD's text read, 7.1 s;
   - the SAS write, 2.5 s;
   - the operator table, 1.9 s;
   - the driver, 0.6 s;
   - the landmark graph, 16.1 s.

   The relaxed-task build (2-3 s) replaces FD's simplification (2.7 s).
4. **Decoding** only the plan's rows (5.2 s → near 0), and `plan_fd.predict`, `either` and the cut to needed facts,
   returning `PlanResult`.
5. **The plan cache** (F4).
6. **The old tail first at a replan.** The previous plan's remaining steps are validated from the new start on the
   new task (taskcore `validate`) and used when they hold; otherwise the planner plans (82 of 97 replans).
7. **`Search::FdLamaFirstCosts`**, kept for the differential tests: FD's search on taskcore's SAS text, as today, with
   FD's search binary called directly, not through its Python driver.

**Exactness**
- The reductions are unchanged, and each is exact, as today.
- Every plan returned is checked from its start by `validate` before it is returned.
- The Rust search is held to give the same plans as FD's `lazy_greedy([hff], preferred=[hff])` with plusone costs:
  byte-identical `sas_plan` on the fixed tasks.
- lama-first never claimed optimal plans, so no optimality claim is lost.
- A cached plan is returned only for a byte-identical key. A tail is used only when it validates.

**Python reference**
- The old `taskcore` binary's outputs.
- Fast Downward (`third_party/downward`) with lama-first-costs and with lazy_greedy hff.
- `plan_fd.decode_ops` and `plan_fd.predict` on the fixed tasks.
- `profile-planner/out/suffix.jsonl`, the 97 replans.

**Test alone.** Fixed tasks:
- ring(38) j00091: the kept reading in `profile-planner/readings/r38/` and `out/cold38/`;
- ring(33) j00089: made again from `readings/r33/start.jsonl` with `profile-planner/scripts/make_reading.py`, under the
  heavy lock.

Each with the goal bit:108=1 and the plan-000 `start.txt` and `caps.txt`, plus the request sequences of
`out/serve33/`.

**Targets**
- A new ring(33) request: ≤ 13 s of CPU (57.6 s of wall today). Its floor is the per-start reductions plus about
  0.6 s of h_FF.
- The search within that: ≤ 1 s.
- A new ring(38) request: ≤ 0.3 s.
- A cache hit: ≤ 2 ms.
- A tail check: ≤ 10 ms.
- Decoding: ≤ 5 ms.

**Gate.** G7.

### `explore`: the Rust explorer, its tries, the harness's planning loop and the guide

**Files**
- `experiments/E011-speed/rs/fmi-explore/` and `experiments/E011-speed/rs/fmi/` (the binary).
- `rs/fmi-core` (Appendix A).
- `experiments/E011-speed/explore/`: fixtures and the reference runner scripts.

**What it ports**
- explore.py: the archive, cells, selection, rotation, the idle set, stop rules, commit as finished, verify.
- step.py: a try, which is the return, the idle measurement, the reading choice, the targets, the attempts,
  `entries_from` and the result.
- guide.py.
- replay.Harness's planning loop:
  - `plan`, `_plan_taskcore`, `_replan_locals`, `_swap`, `_same_input`;
  - `task_start`, `replan_start`, `replan_start_locals`, `prediction`;
  - `cost_caps_of`, `capped_cost`;
  - the bans and their pieces;
  - the reach extension, the refinement turns.
- replay_part1.py's wiring: `reader_switches`, `with_facts`, `refiner`, `reach_extender`, `rereader`.

**The binary.**
- `fmi explore init|run|verify|status`, with explore.py's options and E009's and E010's switches.
- `fmi try JOB_DIR [--targets K=V,...]`: one try from a job.json, as step.py runs it. `--targets` forces the targets,
  for the benchmark.
- `fmi guide`.

**Ports and doubles.** It uses every port. Its doubles:
- `FakeStep`, a port of fake_step.py, for the driver;
- recorded readings (the four files the reference made, e.g. `profile-planner/readings/r38/`), plan results and
  window outcomes, for the harness loop.

**Exactness**
- The archive's logic is deterministic given the tries' results.
- The planning loop's decisions (replan points, refinement turns, stop reasons) are checked against the reference's
  recorded runs.

**Python reference.**
- explore.py with fake_step.py.
- E008's and E009's unit tests: test_explore.py, test_step.py, test_guide.py, test_e009.py.
- guide.py.
- step.py's `targets`, `idle_rule` and `reusable`.
- The off check's run records (`~/.cache/fast-mi-e009/off/step-a/`).

**Test alone.**
- Fake steps.
- The guide for the door and kitchen goals.
- The targets on recorded readings.

No engine is needed.

**Targets**
- The driver's own time per try: ≤ 5 ms.
- The guide: ≤ 1 s.
- The time a try spends outside the ports: ≤ 10 ms.
- Process start, Game.load and reuse_keys per try: 0, because the process stays resident.

**Gate.** G9, and the ported unit tests.

## Exactness gates

Each gate's checks:

- **G1. A0 on the lean build.** E003's `fork_exact.py run` (3,000 frames from frame 235, 7 load children, exact dumps):
  0 differences.
- **G2. The door chain.**
  - E009's door archive, entry 5: boot, then 28 inputs to frame 3129.
  - 37 exact dumps against svm-w3b, with the lean switches on, each alone and all together, on the slim build: 0
    differences.
  - state:316 = 1 at 3129.
- **G3. The kitchen chain.**
  - E009's off check, `off/step-a/jobs/ta/new` entry 7: 36 inputs to frame 3216.
  - 40 exact dumps: 0 differences.
  - owner:566 = 1 at 3216.
- **G4. The binary dump equals the JSON dump.**
  - `Obs` from F1 equals `Obs` parsed from the C7 JSON at every dump of G2 and G3, and at every frame of 400 frames from
    2728.
  - `facts()` from F2 equals `facts_of(obs)` there.
- **G5. The reference on the new build.** E009's step test (a) on the lean build (`FAST_MI_SCUMMVM_BIN` set to it,
  `SPEEDRUN_LEAN=1` in the environment), through the JSON protocol, prints
  `CHECK (a): 0 goal True 2728 3216 488 11712 2 36` with 501 or 502 children, and its verify shows 0 differences.
- **G6. The reader, byte for byte.**
  - The kitchen golden: `reader-port read kitchen --packed` sha1 279b7a24, and 0 differences against golden.py.
  - ring(38) j00091, ring(33) j00089 and the refinement j00089 [103, 194]: the packed file, the golden body and the
    reached file have the sha1 of E009's binary's.
  - The four reading files have the sha1 of `reading_from_golden`'s.
- **G7. The planner.**
  - `taskcore sas` outputs (SAS, ops.jsonl, stats.json) are byte for byte the old binary's on the fixed tasks.
  - Every Rust plan validates. Each equals FD's lazy_greedy hff plusone plan (`sas_plan`, byte for byte).
  - Decoding and `predict` give `plan_fd`'s steps and states.
  - On `suffix.jsonl`, the tail check accepts the recorded 82, and every accepted tail validates.
- **G8. The consumer.**
  - `action(i)` and `record(i)` equal `PackedTask._action(i)` on every ring(38) action, and on a fixed sample of 5,000
    ring(33) actions (every 47th).
  - `by_name` equals `strict.Reading.by_name`.
  - The reuse keys equal `step.reuse_keys_of` on the three readings.
- **G9. The explorer.**
  - On fake steps, `fmi explore` writes the same archive as explore.py with fake_step.py (entries.jsonl, cells.json,
    log.jsonl and summary.json, without time and pid fields), for each switch set in test_explore.py and test_e009.py.
  - The guide's output is byte for byte guide.py's for the door and kitchen goals.
- **G10. End to end, test (a)** (one try from the root on rooms 28, 33, 41 to owner:566 = 1):
  - with strict windows and `FdLamaFirstCosts`, `fmi try` prints E007's line
    `CHECK (a): 0 goal True 2728 3216 488 11712 2 36`, and its verify from boot shows 0 differences;
  - with ready, the same route;
  - with `Gbfs`, the goal is reached and verifies, with the same route when the plans are equal.
- **G11. The 15 tries.**
  - With strict windows and `FdLamaFirstCosts`, each try's stop class and new entries (frames and facts) equal B0's.
  - With the fast settings (`Gbfs`, ready), every new entry verifies from boot with 0 differences.
- **G12. No Python in a run.** During B3, B4 and the kitchen check, no python process is in the run's process tree,
  checked every second through `/proc`.

## Benchmark

All inputs are fixed. Every step runs on the laptop at nice 10, under the lock and the guard where heavy, with seed 1.
Every measure is reported as wall and CPU seconds, by part: return, idle, read, plan, play, and the rest.

- **B0. The 15 tries' entries and the "before" outcomes** (the Python reference, during round 1). Every input comes
  from E009's kitchen log, one line per try:
  - the entry;
  - the scope;
  - the targets tried, in order;
  - the reading used: fresh where the log says fresh, else the reading the named job made;
  - `idle_measured`, false where the log line has an `idle` record.

  The root, entry 0, is E009's root (`~/.cache/fast-mi-e009/archives/door/entries/0.json.gz`). Every other entry is
  made by an earlier try in this sequence, with its id as the log's `new_entries` assigns it.

  `experiments/E011-speed/bench/before.py TRY_DIR` writes `job.json` and runs `step.run_job`, with `step.targets`
  replaced in the wrapper by the log's list. No reference file is edited. A check: each entry used later has the
  frame the log's `return.frame` gives (entry 1: 2767, 2: 2848, 3: 2850, 4: 2862, 6: 2852, 14: 2768, 15: 2818, 20: 2769,
  21: 2818, 30: 2770, 31: 2818).

  Outputs go under `~/.cache/fast-mi-e011/bench/tries/`. The benchmark's scripts live in
  `experiments/E011-speed/bench/`, on branch `e011-bench` (worktree `/var/tmp/fast-mi/wt-e011-bench`), and belong to
  whoever runs the benchmark; no component edits them.
- **B1. Micro-benchmarks, before and after,** back to back on the same input:
  - E010's frame costs, a-d;
  - start;
  - return by fork, and replay from boot of both chains;
  - the readings of G6, cold and as cache hits; the refinement with the memo from j00000's;
  - the planner's fixed tasks: cold, the same request, another start, another goal, a cache hit, a tail;
  - test (a)'s 4 windows.
- **B2. Test (a) end to end** (G10), before (the reference) and after (`fmi try`).
- **B3. The 15 tries after**, in two regimes:
  - (A) cold: one `fmi try` process per try, the reading choice as in B0;
  - (B) warm: one `fmi` process runs the 15 tries in log order and keeps its kept entries, reading cache, memo, warm
    planners and plan cache.

  Each is run with the reference settings (strict, `FdLamaFirstCosts`; outcomes checked against B0, G11) and with the
  fast settings (`Gbfs`, ready).
- **B4. The "before" times**, measured again interleaved with B3: before, after, before, after, try by try, so that
  both meet the same load.
- **B5. The report**, in `experiments/E011-speed/results/`:
  - per part, before, after and floor, with after/floor;
  - per try, both regimes, with medians over the 15 tries, over ring(33) and over ring(38), and the means;
  - the gates.

## Integration and later rounds

**Integration.** The integrator, on branch `e011`:

1. merges the component branches in the stage order;
2. writes `rs/Cargo.toml` (the workspace) and `~/.cache/fast-mi-e011/env.sh`;
3. replaces each double with the real port, through thin adapters only;
4. runs the gates after each merge, and B1-B5 after the last.

A component whose gate fails is not merged.

**Later rounds** go to what still binds: the largest remaining share of B3's warm total. Each item is named here with
its profile number, and none is built before its exactness argument is written:

- the per-start reductions on ring(33). The idea is to keep (a)-(c), (e) and (f) per (reading, goal) and run only (d)
  and a finish per start. The profile estimates 2-3 s against about 10 s, not measured as such.
- a resident reader in the `fmi` process, holding the memo in memory (rerunning 8-11% of jobs on the measured
  refinement);
- FFI or shared memory for the engine, if round trips exceed 10% of a try;
- the count-only mixer (−131 k cycles a frame) and skipping pixel output (−213 k). Each first needs a written argument
  that no pixel or sample value reaches the game state, and a pixel-free exact dump for G1-G3;
- verb symmetry, after its proof (D9), possibly as its own experiment;
- pass 1's discarded output (32% of job time), only with a proof that pass 2's domains are the same without it.

## Kitchen check

This runs once, when every gate G1-G12 passes and B3's warm median per try is under 10 s:

- **What.** `fmi explore` from the boot in a new archive, `~/.cache/fast-mi-e011/archives/kitchen`, to owner:566 = 1.
- **Scope.** The k = 2 scope's 10 rooms: 28, 29, 33, 34, 35, 38, 41, 83, 85, 96.
- **Settings.** b = 2, with E009's switches (`--idle-frames 1600 --rotate --evict-by-size --reuse-nonfacts`), play
  when ready and commit as finished, the fast settings (`Gbfs`), and E009's copy of E008's guides.
- **Stops.** E009's stop rules, and the falsifier `--stop-jobs 80`.
- **Verify.** The goal entry is verified from boot twice: in a fresh lean engine through `fmi explore verify`, and on
  svm-w3b through E008's `explore.py verify` (the reference, outside the run).

## Prediction (before any code)

| | predicted |
|---|---|
| gates | G1-G9 pass in round 1, each after at most one fix round; G10-G12 after the first integration |
| engine | a frame 0.09-0.11 ms inside a command; start 0.1-0.2 s; fork + quit 0.7-1.3 ms; a return by fork 1-4 ms; the door chain from boot 0.3-0.6 s; an observed frame 0.05-0.2 ms |
| reader | Python side 0; a fresh ring(33) reading 45-60% of the old CPU (35-60 s of wall at 3 threads); a refinement from another entry with the memo 6-15 s (73 s today); an exact repeat 0.05 s |
| planner | a new ring(33) request 6-13 s of CPU; ring(38) 0.1-0.3 s; Rust search plans equal FD's hff plans on every fixed task; a repeat or a held tail 1-10 ms |
| play | test (a)'s third window 1-5 s (261 s today); a window whose first try succeeds 10-50 ms; the idle measurement 0.2-0.5 s |
| 15 tries, warm, fast settings | ring(38) median 0.2-1.5 s; ring(33) median 8-35 s, set by the planner's per-request reductions (2-3 requests per try) and the refinement reading; median of all 15: 0.5-2 s; mean 4-15 s |
| 15 tries, cold | ring(38) median 0.5-3 s; ring(33) median 20-60 s |
| route replay | 1-5 ms by fork of a kept entry; 0.3-0.6 s from boot |
| answer to the question | partly: a route replay takes milliseconds; a ring(38) try is well under a second; a ring(33) try is not after round 1 (8-35 s), and comes down to 3-10 s only with round 2's per-start reductions and the resident reader |
| kitchen check | reached and verified from boot with 0 differences; 5-40 tries; 0.1-1.5 h of wall; the goal chain 488-1,500 frames from 2728 |

## Success threshold

- **Yes.** All of the following:
  - every gate G1-G12 passes;
  - a route replay takes ≤ 10 ms by fork and ≤ 1 s from boot on both chains;
  - B3's warm median over the 15 tries is ≤ 1 s, and the ring(33) median is ≤ 10 s;
  - the kitchen is reached and verified from boot (0 differences) within 80 tries.
- **Partly.** The gates pass, a route replay takes milliseconds by fork, and the warm median is under 10 s (so the
  kitchen check runs), but some other part of "yes" fails.
- **No.** An exactness gate fails and is not fixed, or the warm median stays at 10 s or more after the last round.

## Stop rule

- **Exactness first.**
  - A component whose gate fails is not merged.
  - A difference in G1-G3, or in any verify from boot, stops every use of the lean build until it is explained (A0's
    premises).
  - A difference in G11 under the reference settings stops B3 until it is explained.
- **Rounds.**
  - After each integration, B3 runs.
  - The next round builds only items aimed at the part with the largest share of B3's warm total, each with its
    profile number and its exactness argument written first.
  - The rounds stop when every target is met, or when a round gains less than 10% of B3's warm total.
- **Items that need a proof** (pixel skip, verb symmetry, pass 1, any new reduction) wait for the proof (D9). If it is
  not written in this run, the item moves to a later experiment.
- **The kitchen check** runs once, under its own stop rules and falsifier (80 tries).
- **When this experiment stops,** the write-up gives:
  - each part before and after, against its floor;
  - the gates;
  - the kitchen check.

  E011 adds no idea beyond its components and the later-round items named here.
- **Watchdogs.** The harness's, the window search's and the reader's limits are E006's. The guard's 3 GB and the
  disk's 8 GB floors are the machine's. Both are engineering watchdogs, reported where they fire.

## Results, round 1 (2026-11-02, written after the runs)

Branch e011: the six component branches merged (engine 2fa0789, play dc955bf, readio a4c925a, readcore f386535, plan
6a995cb, explore 7d57f27), then the integration commits (the rs/ workspace, reader-scope --memo, the real ports in
fmi with the reading cache and the memo, warm tries in one process, one fix for serde_json's map order in the
workspace). Benchmark scripts: `bench/` (branch e011-bench, merged). Numbers and tables: `results/README.md`,
`results/gates.md`, `results/tries.md`. Lab note: `notes/log/2026-11-02-e011.md`. Laptop, seed 1, nice 10.

**Gates.** G1-G9, G11 and G12 pass on the integrated build, with each component's switches off and on (gates.md). G10
passes its FdLamaFirstCosts clauses and fails its Gbfs clause:

- `fmi try` on test (a) with FdLamaFirstCosts, strict, prints `CHECK (a): 0 goal True 2728 3216 488 11712 2 36` and
  its goal verifies from boot with 0 differences; with ready, the same route.
- With Gbfs, test (a) stops without the goal: no plan at 3174 (strict); the planner's 600 s limit at 7086 (ready, tail).
  Gbfs's second plan names a record whose precondition the engine refutes at 3171 (class semantics); FD's does not.
- G11: under the reference settings all 15 tries equal B0's in everything, cold and warm. Under the fast settings the
  stop class is B0's in 15, and 3 tries play another route; all 110 new entries of the fast sequences verify from boot.
- G12: 3,326 samples of /proc during B3 and B4, no python process in an fmi tree.

**B3 and B4** (wall per try, before = B4 interleaved on the same load):

| | median, 15 | ring(33), 7 | ring(38), 8 | mean | sum |
|---|---:|---:|---:|---:|---:|
| before (Python) | 29.4 s | 186.2 s | 23.8 s | 118.7 s | 1,779.8 s |
| cold, reference | 5.2 s | 63.3 s | 4.2 s | 37.7 s | 565.6 s |
| cold, fast | 4.4 s | 38.3 s | 2.8 s | 25.2 s | 378.6 s |
| warm, reference | 3.8 s | 44.6 s | 2.3 s | 27.6 s | 414.3 s |
| warm, fast | 3.8 s | 17.5 s | 1.1 s | 16.0 s | 240.6 s |

**Route replay** (B1): by fork of a kept entry 0.55-0.84 ms on both chains; from boot 0.33-0.43 s (2.26-3.60 s
before).

**Against the prediction.**

- Gates: predicted G1-G9 in round 1 and G10-G12 after the first integration. G1-G9 and G11-G12 came; G10's Gbfs
  clause did not.
- Engine: predicted a frame of 0.09-0.11 ms inside a command (measured 0.036-0.052 ms), a start of 0.1-0.2 s (37-64
  ms), fork + quit 0.7-1.3 ms (0.66-0.77 ms), a return by fork 1-4 ms (0.55-0.84 ms), the door chain from boot
  0.3-0.6 s (0.30-0.47 s), an observed frame 0.05-0.2 ms (0.063-0.101 ms). All inside or under the predicted ranges.
- Reader: predicted a fresh ring(33) reading at 45-60% of the old CPU (measured 42%, 34.3 s of wall at 3 threads
  against the predicted 35-60 s), a refinement from another entry with the memo in 6-15 s (11.1 s), an exact repeat in
  0.05 s (5-8 ms).
- Planner: predicted a new ring(33) request in 6-13 s of CPU (8.5-11.7 s for another start or caps at load 5; 12.9-22.3
  s for a new goal in B3), ring(38) 0.1-0.3 s (0.11-0.21 s), the Rust search's plans equal FD's h_FF plans on every
  fixed task (yes), a repeat or a held tail 1-10 ms (0.2-13 ms; 135 ms to 1.7 s for a tail on its reading's first use).
- Play: predicted test (a)'s third window in 1-5 s (10.8 s ready, 14.9 s strict), a first-try window in 10-50 ms
  (10-80 ms), the idle measurement in 0.2-0.5 s (0.2-0.4 s).
- 15 tries, warm, fast: predicted ring(38) median 0.2-1.5 s (1.1 s), ring(33) 8-35 s (17.5 s), all 15 0.5-2 s (3.8 s),
  mean 4-15 s (16.0 s). The overall median falls on a fresh ring(38) reading (3.8 s), which the prediction did not
  count.
- 15 tries, cold: predicted ring(38) median 0.5-3 s (2.8 s fast, 4.2 s reference) and ring(33) 20-60 s (38.3 s and
  63.3 s).
- Route replay: predicted 1-5 ms by fork (0.55-0.84 ms) and 0.3-0.6 s from boot (0.33-0.43 s).
- The answer: predicted "partly", with ring(33) tries at 8-35 s after round 1. That is what came (17.5 s).

**Threshold.** "Yes" fails on the warm median (3.8 s against 1 s), the ring(33) median (17.5 s against 10 s) and G10.
"Partly" holds for the route replay and the warm median under 10 s, but its gates clause fails on G10. So the kitchen
check does not run in this round.

**Stop rule.** The next round goes to what has the largest share of B3's warm total: the reading (53%), then the
planner (45%); G10's Gbfs clause needs the search that FD's lama-first runs, in Rust. The items, with targets and gates,
are at the end of results/README.md. Round 1 gained 86% of the before total (1,780 s to 241 s), so the 10% rule does
not stop the rounds.

## Coordination

- **COORD.md.** Builders coordinate only through `/home/ilyask/.cache/fast-mi-e011/COORD.md`, never through messages.
  Each writes one dated line per milestone: started, a gate passed, pushed, disk freed.
- **Interface changes.** A change needed in Appendix A is an `IFACE` entry in COORD.md, giving the change and why.
  Meanwhile the builder adds its own extension in its crate. The integrator applies IFACE changes at integration.
- **Branches.** Builders push their own branch `e011-<key>`; only the integrator merges into `e011`. Nobody pushes to
  main or edits the main checkout.
- **Commits.**
  - Small, one topic each, `kind(E011): ...`, with no Co-Authored-By line and no assistant name.
  - No committed text states a run-length limit or a CPU cap as a decision.
  - No game text, video or other copyrighted material is committed.
- **Before a heavy step,** a builder checks /home's free space and MemAvailable and writes the step to COORD.md.

## Appendix A: `fmi-core`, verbatim

Every builder whose crate uses the ports creates these three files byte for byte, and checks the sha1 before
building:

- `experiments/E011-speed/rs/fmi-core/Cargo.toml`
- `experiments/E011-speed/rs/fmi-core/rustfmt.toml`
- `experiments/E011-speed/rs/fmi-core/src/lib.rs`

Never run `cargo fmt` on it from another crate's settings, and never edit it. Changes go through COORD.md (IFACE).
It builds with no dependency, and its three unit tests pass (`cargo test`, rustc 1.97.1).

Extraction, from the worktree root:

```sh
P=experiments/E011-speed/plan.md; D=experiments/E011-speed/rs/fmi-core; mkdir -p $D/src
for f in Cargo.toml rustfmt.toml src/lib.rs; do
  awk -v b="<!-- fmi-core/$f begin -->" -v e="<!-- fmi-core/$f end -->" \
    '$0==e{f=0} f{print} $0==b{f=1}' $P | sed '1d;$d' > $D/$f
done
sha1sum $D/Cargo.toml $D/rustfmt.toml $D/src/lib.rs
```

Expected sha1:
- `Cargo.toml`: `56db0f6d61a479ca300c365bab75043dadd37997`
- `rustfmt.toml`: `8b41c9aa62958eb3e5eee2d71e67c82dd67e045a`
- `src/lib.rs`: `db2016b608be5f5d069be79decfe8abcfb8e1623`

<!-- fmi-core/Cargo.toml begin -->
```toml
[package]
name = "fmi-core"
version = "0.1.0"
edition = "2021"
publish = false
description = "E011: the types and ports every E011 crate shares (experiments/E011-speed/plan.md, Appendix A)"

[dependencies]
```
<!-- fmi-core/Cargo.toml end -->

<!-- fmi-core/rustfmt.toml begin -->
```toml
max_width = 120
```
<!-- fmi-core/rustfmt.toml end -->

<!-- fmi-core/src/lib.rs begin -->
```rust
//! fmi-core: the types and ports every E011 crate shares (experiments/E011-speed/plan.md, Appendix A).
//!
//! Copied byte for byte from plan.md into `experiments/E011-speed/rs/fmi-core/`; only the integrator changes it
//! (a change is proposed in /home/ilyask/.cache/fast-mi-e011/COORD.md as an "IFACE" entry first). No dependency.
//!
//! * `Key`, `Val`, `Facts`: facts.py's `{key: int}` maps with integer keys (absent is 0).
//! * `facts_of`: facts.py `facts_of` on the typed state dump `Obs` (the C7 dump less its text).
//! * `Domain`: domains.py `Domain` (labels BELOW/ABOVE as sentinel values).
//! * the engine port (`Engine`, `EngineHost`), the reading port (`ReadingSource`), the planner port (`Planner`) and the
//!   play port (`Player`): what one component calls in another. Each port has one provider (plan.md, "Components").

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

// --- values ------------------------------------------------------------------------------------------------------

/// A fact's value: an engine int, or a domain label below.
pub type Val = i64;
/// domains.py BELOW ("below"): every value under the domain's `low`.
pub const BELOW: Val = i64::MIN;
/// domains.py ABOVE ("above"): every value over the domain's `high`.
pub const ABOVE: Val = i64::MAX;
/// Python's None where a value is not known or the domain does not hold it (`Domain.label` -> None).
pub const NONE: Val = i64::MIN + 1;

/// A value as Python prints it in the reference's files: "below", "above", "None" or the int.
pub fn val_text(v: Val) -> String {
    match v {
        BELOW => "below".into(),
        ABOVE => "above".into(),
        NONE => "None".into(),
        n => n.to_string(),
    }
}

// --- keys ----------------------------------------------------------------------------------------------------------

/// A fact key, packed: kind (bits 56-63), unit (48-55), a (24-47), b (0-23). `Key::parse(t).text() == t` for every
/// key text; a text no packed form holds exactly (another prefix, a number over 2^24 - 1, a non-canonical number) is
/// interned instead (kind `K_OTHER`, a = its index in this process's table), so keys of that kind are equal only
/// within one process. The order of `Key` is not Python's string order: sort by `text()` where a file must match.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Key(pub u64);

pub const K_VAR: u8 = 1; // var:N
pub const K_BIT: u8 = 2; // bit:N
pub const K_OWNER: u8 = 3; // owner:O
pub const K_STATE: u8 = 4; // state:O
pub const K_CLASS: u8 = 5; // class:O:C
pub const K_ROOM: u8 = 6; // room
pub const K_EGO: u8 = 7; // ego
pub const K_SCRIPT: u8 = 8; // script:N
pub const K_LOCAL: u8 = 9; // local:N
pub const K_OBJSCRIPT: u8 = 10; // objscript:O
pub const K_AT: u8 = 11; // at:<unit>:W
pub const K_FROZEN: u8 = 12; // frozen:<unit>
pub const K_CUTSCENE: u8 = 13; // cutscene
pub const K_ESC: u8 = 14; // esc:<unit>:W
pub const K_VERB: u8 = 15; // verb:V
pub const K_POS: u8 = 16; // pos:<unit>
pub const K_ESCPOS: u8 = 17; // escpos:<unit>
pub const K_ARGS: u8 = 18; // args:<unit>
pub const K_SAVED: u8 = 19; // saved:A:B
pub const K_ACTORROOM: u8 = 20; // actorroom:A
pub const K_OBJPOS: u8 = 21; // objpos:O
pub const K_GOAL: u8 = 22; // goal (task.py GOAL_KEY)
pub const K_OTHER: u8 = 255; // interned text

/// A slot's unit (facts.py `unit_of`): script:N, local:N, obj:O, entry, exit.
pub const U_NONE: u8 = 0;
pub const U_SCRIPT: u8 = 1;
pub const U_LOCAL: u8 = 2;
pub const U_OBJ: u8 = 3;
pub const U_ENTRY: u8 = 4;
pub const U_EXIT: u8 = 5;

const MAX24: u64 = (1 << 24) - 1;

type Interned = Mutex<(Vec<String>, HashMap<String, u32>)>;

fn intern() -> &'static Interned {
    static T: OnceLock<Interned> = OnceLock::new();
    T.get_or_init(|| Mutex::new((Vec::new(), HashMap::new())))
}

fn num(s: &str) -> Option<u64> {
    // canonical decimal only ("0", "17"; not "017", "+1", "-1"), so that text() gives the same string back
    if s.is_empty() || (s.len() > 1 && s.starts_with('0')) || !s.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    s.parse::<u64>().ok().filter(|&n| n <= MAX24)
}

fn unit(s: &str) -> Option<(u8, u64)> {
    match s {
        "entry" => Some((U_ENTRY, 0)),
        "exit" => Some((U_EXIT, 0)),
        _ => {
            let (k, n) = s.split_once(':')?;
            let u = match k {
                "script" => U_SCRIPT,
                "local" => U_LOCAL,
                "obj" => U_OBJ,
                _ => return None,
            };
            Some((u, num(n)?))
        }
    }
}

fn unit_text(u: u8, a: u64) -> String {
    match u {
        U_SCRIPT => format!("script:{a}"),
        U_LOCAL => format!("local:{a}"),
        U_OBJ => format!("obj:{a}"),
        U_ENTRY => "entry".into(),
        U_EXIT => "exit".into(),
        _ => "?".into(),
    }
}

impl Key {
    pub const fn pack(kind: u8, unit: u8, a: u64, b: u64) -> Key {
        Key(((kind as u64) << 56) | ((unit as u64) << 48) | ((a & MAX24) << 24) | (b & MAX24))
    }
    pub fn kind(self) -> u8 {
        (self.0 >> 56) as u8
    }
    pub fn unit(self) -> u8 {
        (self.0 >> 48) as u8
    }
    pub fn a(self) -> u64 {
        (self.0 >> 24) & MAX24
    }
    pub fn b(self) -> u64 {
        self.0 & MAX24
    }
    pub fn var(n: u64) -> Key {
        Key::pack(K_VAR, 0, n, 0)
    }
    pub fn bit(n: u64) -> Key {
        Key::pack(K_BIT, 0, n, 0)
    }
    pub fn owner(o: u64) -> Key {
        Key::pack(K_OWNER, 0, o, 0)
    }
    pub fn state(o: u64) -> Key {
        Key::pack(K_STATE, 0, o, 0)
    }
    pub fn class(o: u64, c: u64) -> Key {
        Key::pack(K_CLASS, 0, o, c)
    }
    pub fn verb(v: u64) -> Key {
        Key::pack(K_VERB, 0, v, 0)
    }

    /// The key of a text (facts.py, the reader's and the task's keys).
    pub fn parse(t: &str) -> Key {
        if let Some(k) = Key::packed(t) {
            if k.text() == t {
                return k;
            }
        }
        let mut g = intern().lock().unwrap();
        if let Some(&i) = g.1.get(t) {
            return Key::pack(K_OTHER, 0, i as u64, 0);
        }
        let i = g.0.len() as u32;
        g.0.push(t.to_string());
        g.1.insert(t.to_string(), i);
        Key::pack(K_OTHER, 0, i as u64, 0)
    }

    fn packed(t: &str) -> Option<Key> {
        match t {
            "room" => return Some(Key::pack(K_ROOM, 0, 0, 0)),
            "ego" => return Some(Key::pack(K_EGO, 0, 0, 0)),
            "cutscene" => return Some(Key::pack(K_CUTSCENE, 0, 0, 0)),
            "goal" => return Some(Key::pack(K_GOAL, 0, 0, 0)),
            _ => {}
        }
        let (p, rest) = t.split_once(':')?;
        let one = |k: u8| num(rest).map(|n| Key::pack(k, 0, n, 0));
        let two = |k: u8| {
            let (a, b) = rest.split_once(':')?;
            Some(Key::pack(k, 0, num(a)?, num(b)?))
        };
        let of_unit = |k: u8| unit(rest).map(|(u, a)| Key::pack(k, u, a, 0));
        let unit_w = |k: u8| {
            let (u, w) = rest.rsplit_once(':')?;
            let (u, a) = unit(u)?;
            Some(Key::pack(k, u, a, num(w)?))
        };
        match p {
            "var" => one(K_VAR),
            "bit" => one(K_BIT),
            "owner" => one(K_OWNER),
            "state" => one(K_STATE),
            "class" => two(K_CLASS),
            "script" => one(K_SCRIPT),
            "local" => one(K_LOCAL),
            "objscript" => one(K_OBJSCRIPT),
            "at" => unit_w(K_AT),
            "frozen" => of_unit(K_FROZEN),
            "esc" => unit_w(K_ESC),
            "verb" => one(K_VERB),
            "pos" => of_unit(K_POS),
            "escpos" => of_unit(K_ESCPOS),
            "args" => of_unit(K_ARGS),
            "saved" => two(K_SAVED),
            "actorroom" => one(K_ACTORROOM),
            "objpos" => one(K_OBJPOS),
            _ => None,
        }
    }

    /// The key's text, as facts.py and the reader write it.
    pub fn text(self) -> String {
        let (a, b) = (self.a(), self.b());
        match self.kind() {
            K_VAR => format!("var:{a}"),
            K_BIT => format!("bit:{a}"),
            K_OWNER => format!("owner:{a}"),
            K_STATE => format!("state:{a}"),
            K_CLASS => format!("class:{a}:{b}"),
            K_ROOM => "room".into(),
            K_EGO => "ego".into(),
            K_SCRIPT => format!("script:{a}"),
            K_LOCAL => format!("local:{a}"),
            K_OBJSCRIPT => format!("objscript:{a}"),
            K_AT => format!("at:{}:{b}", unit_text(self.unit(), a)),
            K_FROZEN => format!("frozen:{}", unit_text(self.unit(), a)),
            K_CUTSCENE => "cutscene".into(),
            K_ESC => format!("esc:{}:{b}", unit_text(self.unit(), a)),
            K_VERB => format!("verb:{a}"),
            K_POS => format!("pos:{}", unit_text(self.unit(), a)),
            K_ESCPOS => format!("escpos:{}", unit_text(self.unit(), a)),
            K_ARGS => format!("args:{}", unit_text(self.unit(), a)),
            K_SAVED => format!("saved:{a}:{b}"),
            K_ACTORROOM => format!("actorroom:{a}"),
            K_OBJPOS => format!("objpos:{a}"),
            K_GOAL => "goal".into(),
            K_OTHER => intern()
                .lock()
                .unwrap()
                .0
                .get(a as usize)
                .cloned()
                .unwrap_or_else(|| "?".into()),
            _ => "?".into(),
        }
    }

    /// `text().starts_with(prefix)`, as the reference's prefix tests (UNOBSERVED, PREDICTED, SLOT_PREFIXES).
    pub fn has_prefix(self, prefix: &str) -> bool {
        self.text().starts_with(prefix)
    }
}

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text())
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text())
    }
}

// --- facts ---------------------------------------------------------------------------------------------------------

/// facts.py's `{key: int}`: pairs sorted by `Key`, each key once; a key that is absent is 0 (facts.py).
#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub struct Facts(pub Vec<(Key, Val)>);

impl Facts {
    pub fn new() -> Facts {
        Facts(Vec::new())
    }
    /// From pairs in any order; a later pair for the same key wins (a Python dict built in that order).
    pub fn from_pairs(v: Vec<(Key, Val)>) -> Facts {
        let mut idx: Vec<usize> = (0..v.len()).collect();
        idx.sort_by_key(|&i| (v[i].0, std::cmp::Reverse(i)));
        idx.dedup_by_key(|i| v[*i].0);
        Facts(idx.into_iter().map(|i| v[i]).collect())
    }
    /// The value as `dict.get(k)`: None when absent.
    pub fn get(&self, k: Key) -> Option<Val> {
        self.0.binary_search_by_key(&k, |p| p.0).ok().map(|i| self.0[i].1)
    }
    /// The value as facts.py reads a map: absent is 0.
    pub fn val(&self, k: Key) -> Val {
        self.get(k).unwrap_or(0)
    }
    pub fn set(&mut self, k: Key, v: Val) {
        match self.0.binary_search_by_key(&k, |p| p.0) {
            Ok(i) => self.0[i].1 = v,
            Err(i) => self.0.insert(i, (k, v)),
        }
    }
    pub fn remove(&mut self, k: Key) {
        if let Ok(i) = self.0.binary_search_by_key(&k, |p| p.0) {
            self.0.remove(i);
        }
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn iter(&self) -> impl Iterator<Item = &(Key, Val)> {
        self.0.iter()
    }
    /// facts.py `diff`: (key, old, new) for every key whose value differs, absent being 0; in `Key` order.
    pub fn diff(&self, other: &Facts) -> Vec<(Key, Val, Val)> {
        let (a, b) = (&self.0, &other.0);
        let (mut i, mut j, mut out) = (0, 0, Vec::new());
        while i < a.len() || j < b.len() {
            let (k, x, y) = if j >= b.len() || (i < a.len() && a[i].0 < b[j].0) {
                i += 1;
                (a[i - 1].0, a[i - 1].1, 0)
            } else if i >= a.len() || b[j].0 < a[i].0 {
                j += 1;
                (b[j - 1].0, 0, b[j - 1].1)
            } else {
                i += 1;
                j += 1;
                (a[i - 1].0, a[i - 1].1, b[j - 1].1)
            };
            if x != y {
                out.push((k, x, y));
            }
        }
        out
    }
    /// facts.py `patch`: the diff applied; a new value of 0 removes the key.
    pub fn patch(&mut self, d: &[(Key, Val, Val)]) {
        for &(k, _, new) in d {
            if new != 0 {
                self.set(k, new)
            } else {
                self.remove(k)
            }
        }
    }
    /// The pairs in Python's string order of the keys (for files the reference writes sorted).
    pub fn text_sorted(&self) -> Vec<(String, Val)> {
        let mut v: Vec<(String, Val)> = self.0.iter().map(|&(k, x)| (k.text(), x)).collect();
        v.sort();
        v
    }
}

// --- the engine's state (C7 less its text) -------------------------------------------------------------------------

/// The bridge's C7 state dump (speedrun_state.cpp `stateJson`), field for field and in its order, less the names
/// (game text): what the JSON dump and the binary dump both decode to, so `Obs` equality is the binary dump's check.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Obs {
    pub tick: u64,
    pub frame: u64,
    pub room: i32,
    pub ego: Option<i32>,
    pub ego_pos: Option<(i32, i32)>,
    pub seed: Option<i64>,
    pub boot_param: i32,
    pub vars: Vec<i32>,
    pub bits_set: Vec<u32>,
    /// absent in a lite dump
    pub inventory: Option<Vec<i32>>,
    /// (object, owner) in the dump's order; a lite dump holds only the asked objects
    pub owners: Vec<(u32, i32)>,
    pub states: Vec<(u32, i32)>,
    /// (object, its classes 1..32 ascending), objects with a class only
    pub classes: Vec<(u32, Vec<u32>)>,
    /// absent in a lite dump
    pub room_objects: Option<Vec<RoomObject>>,
    pub verbs: Vec<Verb>,
    pub slots: Vec<Slot>,
    pub cutscene: Cutscene,
    pub actors: Vec<ActorPos>,
    /// SPEEDRUN_DUMP_EXACT's object (the profile's exactness checks), when asked
    pub exact: Option<Exact>,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct RoomObject {
    pub id: i32,
    pub state: i32,
    pub parent: i32,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub walk: (i32, i32),
    /// the name (game text): only with `ObsReq::names`
    pub name: Option<String>,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct Verb {
    pub slot: i32,
    pub id: i32,
    pub mode: i32,
    pub saveid: i32,
    pub kind: i32, // the dump's "type"
    pub key: i32,
    pub x: i32,
    pub y: i32,
    /// the name (game text): only with `ObsReq::names`
    pub name: Option<String>,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct Slot {
    pub slot: i32,
    pub number: i32,
    pub where_: i32, // the dump's "where"
    pub status: i32,
    pub offs: u64,
    pub code: i64,
    pub frozen: i32,
    pub delay: i32,
    pub cutscene_override: i32,
    /// with dump_locals (replay_part1.config): the slot's locals, its freeze resistance and recursion
    pub locals: Option<Vec<i32>>,
    pub resistant: Option<bool>,
    pub recursive: Option<bool>,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct Cutscene {
    pub depth: i32,
    pub levels: Vec<CutLevel>,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct CutLevel {
    pub level: i32,
    pub data: Option<i32>,
    pub override_: Option<Override>, // the dump's "override" (null when none)
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct Override {
    pub slot: i32,
    pub number: i32,
    pub where_: i32,
    pub status: i32,
    pub ptr: u64,
    pub code: i64,
    pub stale: bool,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct ActorPos {
    pub id: i32,
    pub x: i32,
    pub y: i32,
    pub elevation: i32,
    pub moving: i32,
}

/// The exact object (speedrun_state.cpp exactJson): every field as the dump has it, in its order; the text-valued
/// fields kept as their text ("audio_owed" is "%.17g", "screens" a hex digest).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Exact {
    pub fields: Vec<(String, ExactVal)>,
}

#[derive(Clone, PartialEq, Debug)]
pub enum ExactVal {
    Int(i64),
    Bool(bool),
    Text(String),
    List(Vec<ExactVal>),
    Map(Vec<(String, ExactVal)>),
    Null,
}

/// What a dump holds (the JSON "state" command's options, and the binary twin's flags).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ObsReq {
    /// Some: a lite dump (`{"cmd": "state", "objects": [...]}`): these objects' owners, states and classes only
    pub objects: Option<Vec<u32>>,
    pub names: bool,
    pub exact: bool,
}

// --- facts_of ------------------------------------------------------------------------------------------------------

pub const WIO_INVENTORY: i32 = 0;
pub const WIO_ROOM: i32 = 1;
pub const WIO_GLOBAL: i32 = 2;
pub const WIO_LOCAL: i32 = 3;
pub const WIO_FLOBJECT: i32 = 4;
pub const ENCD: i32 = 10002;
pub const EXCD: i32 = 10001;
pub const CLOCKS: [usize; 6] = [11, 12, 13, 14, 46, 47];

/// facts.py `unit_of`: (unit, number) of a slot, or None.
pub fn unit_of(where_: i32, number: i32) -> Option<(u8, u64)> {
    match where_ {
        WIO_GLOBAL => Some((U_SCRIPT, number as u64)),
        WIO_LOCAL => Some((U_LOCAL, number as u64)),
        WIO_ROOM if number == ENCD => Some((U_ENTRY, 0)),
        WIO_ROOM if number == EXCD => Some((U_EXIT, 0)),
        WIO_INVENTORY | WIO_ROOM | WIO_FLOBJECT => Some((U_OBJ, number as u64)),
        _ => None,
    }
}

/// facts.py `facts_of`, on the typed dump.
pub fn facts_of(s: &Obs) -> Facts {
    let mut f: Vec<(Key, Val)> = Vec::with_capacity(s.vars.len() + 4096);
    for (n, &v) in s.vars.iter().enumerate() {
        if !CLOCKS.contains(&n) {
            f.push((Key::var(n as u64), v as Val));
        }
    }
    f.extend(s.bits_set.iter().map(|&n| (Key::bit(n as u64), 1)));
    f.extend(s.owners.iter().map(|&(o, v)| (Key::owner(o as u64), v as Val)));
    f.extend(s.states.iter().map(|&(o, v)| (Key::state(o as u64), v as Val)));
    for (o, cs) in &s.classes {
        f.extend(cs.iter().map(|&c| (Key::class(*o as u64, c as u64), 1)));
    }
    f.push((Key::pack(K_ROOM, 0, 0, 0), s.room as Val));
    if let Some(e) = s.ego {
        f.push((Key::pack(K_EGO, 0, 0, 0), e as Val));
    }
    for sl in &s.slots {
        let room_code = sl.number == ENCD || sl.number == EXCD;
        match sl.where_ {
            WIO_GLOBAL => f.push((Key::pack(K_SCRIPT, 0, sl.number as u64, 0), 1)),
            WIO_LOCAL => f.push((Key::pack(K_LOCAL, 0, sl.number as u64, 0), 1)),
            WIO_INVENTORY | WIO_ROOM | WIO_FLOBJECT if !room_code => {
                f.push((Key::pack(K_OBJSCRIPT, 0, sl.number as u64, 0), 1))
            }
            _ => {}
        }
        if let Some((u, a)) = unit_of(sl.where_, sl.number) {
            if sl.code >= 0 {
                f.push((Key::pack(K_AT, u, a, sl.code as u64), 1));
            }
            if sl.frozen > 0 {
                f.push((Key::pack(K_FROZEN, u, a, 0), 1));
            }
        }
    }
    f.push((Key::pack(K_CUTSCENE, 0, 0, 0), s.cutscene.depth as Val));
    if let Some(top) = s.cutscene.levels.last().and_then(|l| l.override_.as_ref()) {
        if !top.stale && top.code >= 0 {
            if let Some((u, a)) = unit_of(top.where_, top.number) {
                f.push((Key::pack(K_ESC, u, a, top.code as u64), 1));
            }
        }
    }
    f.extend(s.verbs.iter().map(|v| (Key::verb(v.id as u64), 1)));
    Facts::from_pairs(f)
}

// --- domains -------------------------------------------------------------------------------------------------------

/// domains.py `Domain`: (BELOW?, ints ascending..., ABOVE?), with `low`/`high` set when BELOW/ABOVE are in it.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Domain {
    pub values: Vec<Val>,
    pub low: Option<i64>,
    pub high: Option<i64>,
}

impl Domain {
    /// `Domain.label`: the value an engine value has here; NONE when the domain does not hold it.
    pub fn label(&self, n: i64) -> Val {
        if let Some(h) = self.high {
            if n > h {
                return ABOVE;
            }
        }
        if let Some(l) = self.low {
            if n < l {
                return BELOW;
            }
        }
        if self.values.binary_search(&n).is_ok() {
            n
        } else {
            NONE
        }
    }
}

/// key -> domain (domains.Domains.doms with the binary prefixes, as the harness's `doms`); sorted by key.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Domains(pub Vec<(Key, Domain)>);

impl Domains {
    pub fn get(&self, k: Key) -> Option<&Domain> {
        self.0.binary_search_by_key(&k, |p| p.0).ok().map(|i| &self.0[i].1)
    }
}

// --- engine commands -----------------------------------------------------------------------------------------------

/// One C8 input event (speedrun_inputs.cpp); `frame` as the "inputs" command takes it (frames after now).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Event {
    pub frame: u64,
    pub what: Input,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Input {
    Click(i32, i32),
    RClick(i32, i32),
    Move(i32, i32),
    /// "esc", "space", "enter" or one printable character
    Key(String),
    Skips {
        text: bool,
        cutscenes: bool,
        spare: Option<Vec<i32>>,
    },
}

/// One C3 condition as the bridge holds it (SpeedrunCondition).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Cond {
    pub kind: CondKind,
    pub subject: i32,
    pub op: CondOp,
    pub value: i32,
    pub values: Vec<i32>,
    pub negate: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CondKind {
    Var,
    Bit,
    Room,
    Owner,
    State,
    Has,
    ActorRoom,
    ActorX,
    ActorY,
    Class,
    ActorMoving,
    Script,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CondOp {
    Eq,
    Le,
    Ge,
    In,
}

/// The "run" command's stops (speedrun_control.cpp); the first that holds ends the run.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct RunSpec {
    /// run to the decision point of this frame
    pub frame: Option<u64>,
    pub max_frame: Option<u64>,
    pub until: Vec<Cond>,
    pub until_any: Vec<Vec<Cond>>,
    pub control: bool,
    pub idle: bool,
    pub menu: bool,
}

/// The stop (or end) reply of a run (speedrun_control.cpp `status`), field for field.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Status {
    /// the stop reason ("frame", "max_frame", "control", "until", "idle", "menu", "until_any:<i>", "fork_each"),
    /// or the end reason when `ended`
    pub reason: String,
    pub ended: bool,
    pub frame: u64,
    pub tick: u64,
    pub qf: u64,
    pub qf0: u64,
    pub room: i32,
    pub up: i32,
    pub cur: i32,
    pub cs: i32,
    pub v19: i32,
    pub seg: bool,
    pub tick0: u64,
    pub rng: u64,
    /// "sentence" or "dialog" when the bridge's idle rule holds
    pub idle: Option<String>,
    pub blk: Option<String>,
    pub ego: Option<(i32, i32)>,
    pub mv: Option<i32>,
    pub walkbox: Option<i32>, // the dump's "box"
    pub cam: i32,
    pub xstart: i32,
    pub room_w: i32,
    /// fork_each: (frame, pid) of each child
    pub forks: Vec<(u64, u32)>,
}

/// The "at" reply: what a click at screen (x, y) would hit now.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct At {
    pub x: i32,
    pub y: i32,
    pub zone: i32,
    /// in the main virtual screen: (vx, vy, obj, actor)
    pub main: Option<(i32, i32, i32, i32)>,
    pub verb_slot: i32,
}

/// One slot run (speedrun_explain.cpp, the "runs" reply's rows), field for field.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Run {
    pub frame: u64,
    pub after: bool,
    pub seq: u64,
    pub depth: i32,
    pub slot: i32,
    pub where_: i32,
    pub number: i32,
    pub room: i32,
    pub code0: i64,
    pub code1: i64,
    pub status: i32,
    pub frozen: i32,
    pub delay: i32,
    pub ops: u64,
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct RunsReply {
    pub runs: Vec<Run>,
    pub dropped: u64,
    pub violations: u64,
    pub frame: u64,
}

/// One "explain" entry (speedrun_explain.cpp), field for field.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Explain {
    Begin {
        seq: u64,
        frame: u64,
        after: bool,
        slot: i32,
        where_: i32,
        number: i32,
        room: i32,
        code0: i64,
    },
    Op {
        seq: u64,
        code: i64,
        opcode: u8,
    },
    Read {
        seq: u64,
        var: u32,
        value: i32,
    },
    Write {
        seq: u64,
        var: u32,
        value: i32,
    },
    End {
        seq: u64,
        code1: i64,
        status: i32,
        frozen: i32,
        delay: i32,
    },
}

/// The "explain" command's arming: frames from..=to, the units [(where, number)] (empty: every unit).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ExplainArm {
    pub from: u64,
    pub to: u64,
    pub units: Vec<(i32, i32)>,
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ExplainReply {
    pub entries: Vec<Explain>,
    pub dropped: u64,
    pub violations: u64,
}

#[derive(Debug)]
pub enum EngineError {
    /// the engine replied with an error (the bridge's "error" text)
    Bridge(String),
    /// the engine ended (its "end" reply) where a stop was expected
    Ended(Box<Status>),
    Io(std::io::Error),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::Bridge(e) => write!(f, "bridge: {e}"),
            EngineError::Ended(s) => write!(f, "engine ended: {}", s.reason),
            EngineError::Io(e) => write!(f, "io: {e}"),
        }
    }
}

impl std::error::Error for EngineError {}

pub type EResult<T> = Result<T, EngineError>;

/// One engine process, stopped at a decision point (the bridge's control socket, one connection per process).
/// Provider: the `engine` component (fmi-engine, the binary channel). Every method is one command and its reply.
pub trait Engine: Send {
    fn pid(&self) -> u32;
    /// the status of the last stop (the hello's frame and tick before the first run)
    fn status(&self) -> &Status;
    fn run(&mut self, spec: &RunSpec) -> EResult<Status>;
    /// schedule input; frames are frames after now (the "inputs" command)
    fn inputs(&mut self, events: &[Event]) -> EResult<()>;
    fn obs(&mut self, req: &ObsReq) -> EResult<Obs>;
    /// `facts_of(obs(full))` without building the dump the facts do not read (the hot path)
    fn facts(&mut self) -> EResult<Facts>;
    fn holds(&mut self, conds: &[Cond]) -> EResult<bool>;
    fn at(&mut self, x: i32, y: i32) -> EResult<At>;
    fn set_rng(&mut self, state: u32) -> EResult<()>;
    fn segment(&mut self, on: bool) -> EResult<()>;
    fn runs(&mut self) -> EResult<RunsReply>;
    /// Some(arm) arms the explain log; None disarms it
    fn explain(&mut self, arm: Option<&ExplainArm>) -> EResult<()>;
    fn explain_log(&mut self) -> EResult<ExplainReply>;
    /// an exact copy of this process (fork), stopped where this one is
    fn fork(&mut self) -> EResult<Box<dyn Engine>>;
    /// the "run" command's fork_each: a child at each of the next `n` decision points, each stopped there; the
    /// parent stops after the last one ("fork_each")
    fn fork_each(&mut self, n: u32) -> EResult<(Status, Vec<Box<dyn Engine>>)>;
    /// the C7 dump's JSON text (the differential tests against the Python reference only)
    fn state_json(&mut self, req: &ObsReq) -> EResult<String>;
    /// CPU seconds of this process so far (utime + stime)
    fn cpu_s(&self) -> f64;
    fn quit(self: Box<Self>);
}

/// How an engine starts (replay_part1.config): seed 1, the input movie from boot, the bridge's settings.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct BootCfg {
    pub seed: u32,
    /// the movie: events from boot with absolute frames (an entry's "events")
    pub events: Vec<Event>,
    /// run to the decision point of this frame before returning (None: the first idle stop, as replay_part1.boot)
    pub frame: Option<u64>,
    pub dump_locals: bool,
    /// the engine's exact fast path (patch 0002's switches); false runs it as svm-w3b does
    pub lean: bool,
    pub out_dir: PathBuf,
}

/// Starts engines. Provider: the `engine` component.
pub trait EngineHost: Send + Sync {
    fn boot(&self, cfg: &BootCfg) -> EResult<Box<dyn Engine>>;
}

// --- the reading ---------------------------------------------------------------------------------------------------

/// A reading's files (packed_reading.py's reading: reading.json, reading.tc, reading.actions.jsonl,
/// reading.lists.jsonl, reading.reached) and E011's index. Provider: the `readio` component (fmi-reading).
pub trait ReadingSource: Send + Sync {
    /// sha1 of reading.tc: a reading's identity in every cache key
    fn id(&self) -> [u8; 20];
    fn dir(&self) -> &Path;
    /// reading.json's text
    fn head(&self) -> &str;
    /// reading.tc's bytes (mapped)
    fn packed(&self) -> &[u8];
    fn n_actions(&self) -> u32;
    /// reading.actions.jsonl's line i, without its newline (byte-identical to packed_reading's)
    fn action_line(&self, i: u32) -> &str;
    /// strict.Reading's `by_name`: the last action with this name
    fn by_name(&self, name: &str) -> Option<u32>;
    fn n_lists(&self) -> u32;
    /// reading.lists.jsonl's line i, without its newline
    fn list_line(&self, i: u32) -> &str;
    /// reading.reached's text
    fn reached(&self) -> &str;
    /// the reading's task facts and their domains (the packed file's facts, as PackedTask.facts)
    fn domains(&self) -> &Domains;
    /// the packed file's goal
    fn goal(&self) -> &[(Key, Vec<Val>)];
    /// the start facts it was read from (reading.json "start", as a facts.py map)
    fn start(&self) -> &Facts;
    /// PackedTask._action(i): name "label#rec" (or the label), pre, effects and cost; `what` = i
    fn action(&self, i: u32) -> Action;
    /// the reader.Record PackedTask._action(i) builds (the action's `what`)
    fn record(&self, i: u32) -> Record;
}

/// reader.Record's effect on one key (packed_reading.effects_of): ("set", v), ("guess", values), ("add", d),
/// ("copy", key), ("when", key, v).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RecEff {
    Set(Val),
    Guess(Vec<Val>),
    Add(i64),
    Copy(Key),
    When(Key, Val),
}

/// reader.Record as packed_reading.PackedTask._action builds it from the packed action and its line.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Record {
    pub kind: String,
    pub label: String,
    /// the precondition on the line's "pre_keys" that the packed precondition holds
    pub pre: Vec<(Key, Vec<Val>)>,
    pub effects: Vec<(Key, RecEff)>,
    /// the line's "checks", each as its JSON text
    pub checks: Vec<String>,
    pub end: String,
    /// the line's "source" as its JSON text
    pub source: String,
    pub engine_pre: Vec<Key>,
}

// --- the planner ---------------------------------------------------------------------------------------------------

/// task.py `Effect`: a known value, the key plus a constant, or one of several values (a guess).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EffVal {
    Value(Val),
    Add(i64),
    Values(Vec<Val>),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Effect {
    pub key: Key,
    pub v: EffVal,
    /// ((key, values), ...): all hold before the action
    pub when: Vec<(Key, Vec<Val>)>,
}

/// task.py `Action` as the planner's paths give it; `what` is the reading's action index (its record).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Action {
    pub name: String,
    pub pre: Vec<(Key, Vec<Val>)>,
    pub effects: Vec<Effect>,
    pub what: u32,
    pub cost: i64,
}

/// replay.Ban: a record's label and the (key, value) pairs it is banned under.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Ban {
    pub label: String,
    pub facts: Vec<(Key, Val)>,
}

/// plan_fd.Step: one step of a plan.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlanStep {
    pub op: String,
    pub action: u32,
    pub name: String,
    pub chosen: Vec<(Key, Val)>,
    /// (key, value, when): values the reader could not work out, guessed here
    pub guessed: Vec<(Key, Val, String)>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Search {
    /// Fast Downward's lama-first-costs on taskcore's SAS file (plan_fd.SEARCHES), as E009 runs it
    FdLamaFirstCosts,
    /// the Rust search: lazy greedy best-first, h_FF with preferred operators, cost + 1 (the profile's prototype)
    Gbfs,
}

/// One plan request (plan_fd.plan_taskcore's inputs).
#[derive(Clone, Debug)]
pub struct PlanRequest {
    /// the goal (taskcore --goal): key -> values
    pub goal: Vec<(Key, Vec<Val>)>,
    /// the start (a facts.py map with the task's pos:/escpos:/args: keys)
    pub start: Facts,
    pub bans: Vec<Ban>,
    /// the cost caps (replay.cost_caps_of): action name -> cap
    pub caps: Vec<(String, i64)>,
    pub search: Search,
    /// taskcore --start-check: "needed" or "input"
    pub start_check: String,
    /// the previous plan's remaining steps' names: used when they still form a plan from this start (plan.md)
    pub tail: Option<Vec<String>>,
    pub time_limit_s: u64,
}

/// plan_fd.TaskcoreResult, the fields the harness reads.
#[derive(Clone, Debug, Default)]
pub struct PlanResult {
    /// "plan", "unsolvable", "incomplete", "time", "memory", "error" or "out of domain"
    pub status: String,
    pub steps: Vec<PlanStep>,
    /// per step its path of the banned task (res.paths) and that path cut to the needed facts (res.actions)
    pub paths: Vec<Action>,
    pub actions: Vec<Action>,
    /// step index -> its paths left to the engine (res.choices), cut to the needed facts
    pub choices: Vec<(u32, Vec<Action>)>,
    /// per step its fold's name when it folds more than one member (res.folds)
    pub folds: Vec<Option<String>>,
    pub predicted_goal: bool,
    pub plan_cost: Option<i64>,
    pub file_facts: Vec<Key>,
    pub needed_facts: Vec<Key>,
    /// out of domain: (key, start value) of the facts out of their domains, and their count
    pub misses: Vec<(Key, Val)>,
    pub n_misses: u64,
    /// taskcore's stats.json text
    pub stats: String,
    /// where the plan came from: "search", "cache", "tail"
    pub source: String,
    pub seconds: f64,
    pub cpu_s: f64,
}

#[derive(Debug)]
pub enum PlanError {
    /// plan_fd.PathChoice: a step whose paths differ on facts the file holds
    PathChoice(String),
    /// plan_fd.TaskcoreError (with its message, which `reach_misses` reads)
    Taskcore(String),
}

/// The planner of one reading, kept warm across requests. Provider: the `plan` component (taskcore's library).
pub trait Planner: Send {
    fn plan(&mut self, req: &PlanRequest) -> Result<PlanResult, PlanError>;
}

/// Makes the warm planner of a reading (one per reading id, kept by the caller).
pub trait PlannerHost: Send + Sync {
    fn planner(&self, reading: &dyn ReadingSource) -> Result<Box<dyn Planner>, PlanError>;
}

// --- play ----------------------------------------------------------------------------------------------------------

/// The harness's state the window search reads and writes (replay.Harness's fields that window.py uses).
pub struct PlayState {
    /// h.branch: the engine the plan plays on
    pub engine: Box<dyn Engine>,
    /// h.state, h.obs, h.now (view of obs), h.pred, h.kept
    pub state: Obs,
    pub obs: Facts,
    pub now: Facts,
    pub pred: Facts,
    pub kept: Facts,
    /// h.checkpoints
    pub checkpoints: Vec<Checkpoint>,
    pub stop_reason: Option<String>,
    /// h.replan_cause (window.py's deviation cause), JSON text
    pub replan_cause: Option<String>,
    /// h.plans' entries and the window records, JSON lines the reference writes
    pub records: Vec<String>,
    pub workdir: PathBuf,
}

/// replay.Harness.checkpoint's tuple: (frame, facts, extra_of(state, status)), for `verify`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Checkpoint {
    pub frame: u64,
    pub facts: Facts,
    pub tick: u64,
    pub rng: u64,
    /// sorted [id, x, y] of the dump's actors
    pub actors: Vec<(i32, i32, i32)>,
}

/// What the plan plays against (the rest of the harness's fields window.py reads).
pub struct PlanCtx<'a> {
    pub reading: &'a dyn ReadingSource,
    pub doms: &'a Domains,
    /// h.unobserved: key prefixes the dump does not hold
    pub unobserved: &'a [&'a str],
    pub goal: &'a [(Key, Vec<Val>)],
    pub result: &'a PlanResult,
    pub unobserved_actor_rooms: bool,
    pub time_limit_s: f64,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PlayOutcome {
    Goal,
    Replan,
    Stop,
}

/// Plays one plan through its windows (window.Search.play_plan), and observes for the harness (Harness.observe).
/// Provider: the `play` component (fmi-play).
pub trait Player: Send {
    fn observe(&mut self, ctx: &PlanCtx<'_>, ps: &mut PlayState) -> EResult<()>;
    fn play_plan(&mut self, ctx: &PlanCtx<'_>, ps: &mut PlayState, n_plan: u32) -> PlayOutcome;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_round_trip() {
        for t in [
            "var:100",
            "bit:108",
            "owner:566",
            "state:316",
            "class:12:3",
            "room",
            "ego",
            "script:2",
            "local:202",
            "objscript:489",
            "at:local:202:21",
            "at:script:2:7",
            "at:entry:0",
            "at:obj:428:105",
            "frozen:script:2",
            "frozen:entry",
            "cutscene",
            "esc:local:200:33",
            "verb:2",
            "pos:script:21",
            "escpos:exit",
            "args:obj:12",
            "saved:1:2",
            "actorroom:3",
            "objpos:44",
            "goal",
            "pick:var:100",
            "due:x",
            "var:007",
            "var:-1",
        ] {
            assert_eq!(Key::parse(t).text(), t, "{t}");
        }
        assert_eq!(Key::parse("var:007").kind(), K_OTHER);
        assert_eq!(Key::parse("at:local:202:21"), Key::pack(K_AT, U_LOCAL, 202, 21));
    }

    #[test]
    fn diff_and_patch() {
        let a = Facts::from_pairs(vec![(Key::var(1), 3), (Key::bit(2), 1), (Key::var(1), 4)]);
        assert_eq!(a.get(Key::var(1)), Some(4));
        let b = Facts::from_pairs(vec![(Key::var(1), 4), (Key::var(5), 0), (Key::verb(9), 1)]);
        let d = a.diff(&b);
        assert_eq!(d, vec![(Key::bit(2), 1, 0), (Key::verb(9), 0, 1)]);
        let mut c = a.clone();
        c.patch(&d);
        assert!(c.diff(&b).is_empty());
    }

    #[test]
    fn facts_of_slots() {
        let s = Obs {
            vars: vec![7; 50],
            slots: vec![Slot {
                where_: WIO_LOCAL,
                number: 202,
                code: 21,
                frozen: 1,
                ..Default::default()
            }],
            cutscene: Cutscene {
                depth: 1,
                levels: vec![CutLevel {
                    level: 0,
                    data: None,
                    override_: Some(Override {
                        where_: WIO_ROOM,
                        number: ENCD,
                        code: 4,
                        ..Default::default()
                    }),
                }],
            },
            ..Default::default()
        };
        let f = facts_of(&s);
        assert_eq!(f.get(Key::var(11)), None);
        assert_eq!(f.get(Key::parse("local:202")), Some(1));
        assert_eq!(f.get(Key::parse("at:local:202:21")), Some(1));
        assert_eq!(f.get(Key::parse("frozen:local:202")), Some(1));
        assert_eq!(f.get(Key::parse("esc:entry:4")), Some(1));
        assert_eq!(f.get(Key::parse("cutscene")), Some(1));
    }
}
```
<!-- fmi-core/src/lib.rs end -->

## Amendment 2026-11-03, before the kitchen check: FD's search on the Rust path

- **Why.** Round 1's integration passes every gate but G10's Gbfs clause: with the Rust search (`--search gbfs`),
  test (a) does not reach the kitchen, while `FdLamaFirstCosts` (`--search fd`) gives the reference's route, inputs
  and entries. Round 2, which would have taken that up, did not run.
- **What changes.** The kitchen check runs with `--search fd` instead of the fast settings' Gbfs; that configuration
  passes G1-G12 (G10's first clause), and its warm median per try in B3 is 3.8 s (b-ref), under the 10 s condition.
  Everything else is as above: the 10 k = 2 rooms, b = 2, E009's switches, play when ready, commit as finished,
  E009's copy of E008's guides, E009's stop rules and `--stop-jobs 80`, verify from boot twice.
- **Where.** The archive is `/var/tmp/fast-mi/e011-archives/kitchen` with `E008_DISK_PATH=/var/tmp`, since /home is at
  its floor.

## Amendment 2026-11-03, during the kitchen check: the planner's limit per request from the recorded plans

- **Measured.** Over the 260 plans found in E008's, E009's and this check's tries, a found plan's planning took at
  most 161 s (median 18 s, p90 45 s, p99 74 s). The only two requests that reached the 600 s limit were in this check
  (tries 15 and 16, target bit:118 = 1, from frames 2770 and 2771) and returned no plan; with targets in rotation, each
  such target cost 10 minutes of a worker.
- **What.** The check was stopped after 15 committed tries (two running, uncommitted) and resumed from its files with
  `FMI_PLAN_TIME=180`: above every found plan's time, so no plan the tries have found would have been lost. Nothing
  else changes.

## Amendment 2026-11-03, round 2's integration: the fast settings' search, and what round 2 is measured against

Written before round 2's benchmark ran (branch e011-r2: the six round-2 branches merged, then the integration's own
commits).

- **Why.** Round 2's route component (e011-r2-route) ported Fast Downward's lama-first-costs to Rust, with plans,
  landmark graphs and counts byte for byte Fast Downward's on 30 requests. On its branch the fast settings with that
  search reached the kitchen on test (a) and gave B0's tries in everything, where Gbfs (round 1's fast search) fails G10.
- **What changes.**
  1. The fast settings become: play when ready, the old tail first, and `Search::FdLamaFirstCosts` served by the Rust
     port (`--search fd --tail`, `FMI_PLAN_LAMA=rust`). G10's third clause and G11's second are run with them. Gbfs
     stays in the code; its G10 clause stays as round 1 left it.
  2. Round 2's B3 is the warm regime (B3 (B)) of round 2's binaries against round 1's (e011 9bbd44a's `fmi`, round
     1's reader-scope and engine build), interleaved try by try as B4 interleaves the reference: round 2's "before" is
     round 1. The reference settings run three resident processes (round 1; round 2 with every round-2 switch off;
     round 2 with every round-2 switch on), the fast settings two (round 1 with Gbfs; round 2 with every switch on and
     the Rust lama-first-costs). The Python reference is not timed again: its times are round 1's B4.
  3. "Switches off" and "on" in round 2's gates mean every round-2 switch at once (`env-r2.sh`'s OFF2 and ON2), on top
     of each component's own checks of its switch alone where its script makes them.
- Nothing else changes: the gates, the 15 tries, B0, the parts and the floors are as above.

## Amendment 2026-11-03, during round 2's integration: B3 without the job memo, and G12's roots

Written after a first start of round 2's B3 was stopped (two tries in), before it ran again.

- **Measured.** During the integration / had 6-8.8 GB free (other experiments' folders on the same disk). The job
  memo's store writes nothing while its disk is under 8 GB plus twice its largest file (memo.md, "The store"; round 1's
  reader has that floor built in, round 2's reads it from READER_MEMO_FLOOR_GB). In the stopped start, round 1's
  j00000 stored 22,842 runs and round 2's, ten minutes later, none: the memo was on in one sequence and off in the
  other by the disk's level, not by the code. G6's memo clause for the fresh ring(33) pair stored and took nothing for
  the same reason (its refinement pair did store and take).
- **What changes.**
  1. Round 2's B3 runs with the job memo off in every sequence (FMI_MEMO unset), so that both rounds read the same way
     whatever the disk's level; the reading cache, kept entries, warm planners and plan store stay as B3 (B) has them,
     with the reading cache's floor (E008_FLOOR_GB) at 5 GB, the disk rule of this run, in every sequence. The memo's
     own gains are measured by G6's memo clauses, not by B3.
  2. G6 runs again with READER_MEMO_FLOOR_GB=5 for round 2's reader, so that its memo clauses store and take.
  3. G12's watcher counts as roots only the benchmark's resident fmi processes. The kitchen check's `fmi explore run`
     shares the laptop and runs each job under guard.sh (driver.rs, a process per job), whose last line is
     `python3 -c "print(round(...))"` for its wall time; the round-1 watcher, rooted at every process named fmi, logged
     seven such processes in the stopped start. None of them was under a process of the benchmark.

## Results, round 2 (2026-11-03, written after the runs)

Branch e011-r2: origin/e011 9bbd44a with the six round-2 branches merged (engine b80bf2a, play 71c370c, read 15414f1,
plan 661fcb2, route 621e519, own 735faec), then the integration commits: e011-r2-own's interface request
(`Engine::step_changes`) next to e011-r2-play's new methods in fmi-core and fmi-engine (the only conflicts, both sides
kept), FMI_PLAY_KEEP, FMI_READER_PREP (reader-scope's --prep), the Rust lama-first-costs with the watched h_FF under
FMI_PLAN_FAST, the engine built with patches 0001-0004, and the bench scripts. e011-r2-read's reader inside the fmi
process is not on its branch (uncommitted in its worktree), so fmi still runs reader-scope as a child. Numbers and
tables: `results/round2/` (README.md, gates.md, tries-*.md). Lab note: `notes/log/2026-11-03-e011-round2.md`. Laptop,
seed 1, nice 10; the kitchen check (round 1's binaries) and another experiment shared the laptop.

**Gates.** G1-G12 pass with every round-2 switch off and every one on (results/round2/gates.md). G10 now passes in
full: with the Rust lama-first-costs, `fmi try` on test (a) prints `CHECK (a): 0 goal True 2728 3216 488 11712 2 36` in
strict, ready and ready-with-the-old-tail modes, and its entries verify from boot with 0 differences. G11: the 15 tries
equal B0's in everything under the reference settings (round 2 with the switches off and on) and under the fast
settings (round 2); 129 new entries verify from boot. Two gate details: G6's reading.json at 6 threads differs from the
reference only in its recorded thread count; G12 is rooted at the benchmark's processes (amendment above).

**B3** (warm, the job memo off in every sequence, round 1's and round 2's binaries interleaved try by try at load
1.9-5.1; wall per try):

| settings | binaries | median, 15 | ring(33), 7 | ring(38), 8 | mean | max | sum |
|---|---|---:|---:|---:|---:|---:|---:|
| reference | round 1 | 6.3 s | 43.5 s | 2.75 s | 28.3 s | 158.0 s | 424.5 s |
| reference | round 2 | 4.8 s | 5.9 s | 1.25 s | 13.3 s | 128.2 s | 199.2 s |
| fast | round 1 (Gbfs) | 4.9 s | 17.6 s | 1.75 s | 17.7 s | 114.9 s | 264.9 s |
| fast | round 2 (the Rust lama-first-costs) | 4.9 s | 6.1 s | 1.3 s | 12.9 s | 121.0 s | 193.1 s |

By part, round 2's fast sequence (median / max per try): return 0.00 / 0.09 s, idle 0.10 / 0.10 s, read 0.05 / 105.8 s,
plan 0.29 / 14.8 s, play 0.02 / 0.35 s; round 1's in the same run: 0.01 / 0.33, 0.20 / 0.40, 0.19 / 94.5, 0.71 / 19.3,
0.02 / 0.52. The read is 74% of round 2's sum and the plan 25%. The planner's ring(33) request on the plan component's
fixed sequence: another start 3.0-3.2 s (round 1: 7.0-7.6 s with Gbfs, 19.9-22.7 s with Fast Downward), another goal
4.4 s (11.5 s, 26.6 s), of which the per-start reductions are 2.0-3.4 s. A fresh ring(33) reading: 35.0-36.3 s with
round 1's reader, 36.4-40.8 s with round 2's. Test (a)'s third window, ready: 4.5 s (17.1 s with the switches off).
The try's own time: 13-14 ms (80-113 ms). The door chain from boot on the engine with both cuts: 0.12 s.

**Against the prediction.** The prediction's answer said a ring(33) try would come down to 3-10 s only with round 2's
per-start reductions and the resident reader. It came to 6.1 s (fast) and 5.9 s (reference) with the per-start
reductions' fast forms and the Rust lama-first-costs, without the resident reader. The 15 tries' median, predicted at
0.5-2 s, stays at 4.9 s: it is a fresh ring(38) reading, which round 2 did not change.

**Threshold.** "Yes" fails on the warm median: 4.9 s against 1 s. The ring(33) median (6.1 s against 10 s), the route
replay and every gate now pass; the kitchen clause is the kitchen check's, which runs on round 1's binaries (the
amendments above). So the answer stays "partly", now with all its gates.

**Stop rule.** Round 2 gained 27% of the fast warm sum (264.9 s to 193.1 s) and 53% of the reference sum against round
1's binaries on the same load, more than 10%, so the rounds do not stop by that rule. The next round goes to the
reading (74%): the first try's fresh ring(33) reading and two refinements (106 s), a refinement from another entry
(21 s without the memo), and the fresh ring(38) readings (4.4-4.8 s each), which set the median. Then the planner's
per-start reductions (25%).

## Amendment 2026-11-03, during the kitchen check: targets with no plan made cheap

- **Why.** In the check, target bit:118 = 1 from frames 2770 and 2771 (tries 15 and 16) made Fast Downward's
  lama-first-costs search its whole limit and end with no plan: 607 s and 612 s, then 191 s and 187 s under the 180 s
  limit. With targets in rotation, each such target costs a worker the whole limit. A found plan took at most 161 s
  over the 260 recorded (median 18 s, p90 45 s, p99 74 s). The PI asked for targets with no plan to be cheap.
- **Prior work** (each paper opened):
  - h^m (Haslum and Geffner, AIPS 2000): reachability over sets of m facts; m = 2 gives the pairs no reachable state
    holds (mutexes), polynomial in facts² × operators.
  - Alcázar and Torralba (ICAPS 2015): h^2 mutexes on SAS+ as preprocessing, forward and in regression; the
    computation took up to minutes on the largest IPC tasks.
  - Hoffmann, Kissmann and Torralba (ECAI 2014): heuristics aimed at unsolvability only (∞ or 0); merge-and-shrink with
    shrinking that keeps solvability; they cite projections tested for unsolvability (Bäckström, Jonsson and
    Ståhlberg 2013).
  - The Unsolvability IPC 2016: Fast Downward Aidos (Seipp, Pommerening, Sievers, Wehrle, Fawcett, Alkhazraji), the
    winner, a portfolio of breadth-first search with dead-end pattern databases (Pommerening and Seipp), dead-end
    potentials (an LP over features of up to two facts), resource detection, h^m, h^max, and h^2-mutex operator pruning
    in every portfolio.
  - Traps (Lipovetzky, Muise and Geffner, ICAPS 2016): k-DNF conditional invariants, compared against h^1 and h^2.
  - Certificates of unsolvability (Eriksson, Röger and Helmert, ICAPS 2017).
- **What fits our tasks.** A request's task is 10 k (ring(38)) to 155 k (ring(33)) operators, 460 to 7,200 facts,
  with conditional effects (1.45 M of 3.8 M effects on ring(33)). taskcore's reductions already apply forward value
  reachability (d) and backward relevance (e), so h^1 seldom proves more. h^2 is polynomial and needs no pattern
  choice and no LP. Pattern databases, merge-and-shrink and potentials need a pattern or merge strategy and much more
  code. A blind complete search is not cheap here: Fast Downward's blind A* on the 10 k-operator ring(38) task for
  bit:118 = 1 ran 120 s without ending.
- **Measured before this amendment** (exploration, the code below in its first form):
  - h^2 proves bit:118 = 1 dead on both of the check's requests (j00015, j00016): the goal fact itself is outside the
    fixpoint. On the profile's ring(38) target survey, where 3 of 33 targets ran the 30 s limit with no plan, h^2
    proves bit:118 = 1 and var:53 = -1 dead and not var:52 = -1.
  - h^2 takes 0.05 s on ring(38) and 6.2 s on ring(33) at load 10-12, with the same rows as the plain fixpoint.
- **Method (1): sound no-plan proofs before the search.** Switch `FMI_PLAN_DEAD=1` (off: the old behaviour).
  - In fmi-plan, after the reduced task is built in memory and before the search (Gbfs or Fast Downward):
    1. h^1, relaxed reachability on the SAS task;
    2. h^2, the pair fixpoint (taskcore `dead.rs`).
  - A goal fact or pair outside the fixpoint is a proven no-plan: status "unsolvable" (the harness's "no plan (the
    planner proved none)"), source "proof". Its stats name the method and the goal fact, with the seconds.
  - Otherwise the search runs as before. The h^2 fixpoint has an engineering watchdog (`FMI_PLAN_DEAD_S`, 60 s): past
    it, the proof is open and the search runs.
- **Soundness.**
  - h^1: every fact of a reachable state is in the fixpoint (induction on the path). An effect's fact is added once
    its operator's preconditions and its own conditions are in.
  - h^2: every pair of facts of a reachable state is in the fixpoint (induction on the path). For a step o from s to
    s', a fact of s' was written by an effect that fired (its conditions and o's preconditions hold in s) or kept
    from s (no fired effect writes its variable, so no unconditional effect does). Each case of a pair (both written,
    one written and one kept, both kept) is a rule of the fixpoint whose premises are pairs of s. So a goal pair
    outside the fixpoint is in no reachable state.
  - Neither proof can claim no plan for a task with one. The fast fixpoint must equal the plain one row for row; that
    is a test, not an argument.
- **Method (2): iterative deepening on planning time.** Switch `FMI_PLAN_DEEPEN=L0` in seconds (unset: the old
  behaviour).
  - **The first limit.** L0 is the p90 of the found plans' search seconds on the warm Rust path (fmi-plan, warm
    planner), measured on the recorded requests of the gate below, rounded up to 10 s.
  - **Within a try.** A target deferred n times is planned with limit L0 · 2^n (n = 0: not deferred). This limit
    applies to its plans until the attempt finds one; replans after that keep the old limit. The try's targets come in
    their order, the deferred ones last, by n.
  - **Deferral.** An attempt whose first plan ends on the limit ("time") defers its target (n + 1): it is not "tried"
    and stays in the entry's targets for later tries. Each target is attempted at most once per try.
  - **Exhaustion.** An entry is exhausted only when a try played nothing and deferred nothing. So a deferred target is
    retried, with a doubled limit, only when its entry has no undeferred target left.
  - **The archive.** It keeps the deferred targets per (cell, entry) as it keeps the tried ones: job.json "deferred",
    the log line's "deferred". A resumed run reads them back.
  - **Completeness.** No target is dropped for good by a limit. A target with a plan whose search takes t seconds is
    found once L0 · 2^n ≥ t.
- **Prediction** (before the code that uses it):
  - The gate below passes: no recorded request with a plan is claimed dead, and every plan is the recorded one.
  - The check's two bit:118 requests are proven dead by h^2, each in under 30 s of request wall at the check's load
    (was 187-612 s).
  - h^2 costs ring(33) requests 3-10 s each (load 5-12) and ring(38) requests under 0.1 s.
  - Over every target at the check's entries 36 and 26 (89 each), h^2 proves 1-10 dead and h^1 proves none.
  - L0 lands in 30-60 s, and a dead target that no proof catches costs L0 in its first round.
- **Gate.**
  1. Every recorded request with a plan that can be rebuilt is replayed with both switches on: E008's fixed tasks
     (ring(38) j00091, ring(33) j00089, serve38, serve33), the benchmark's B0 (E009's) and B3 tries, and the check's
     tries. For each: no proof claims no plan, and the planner returns the recorded plan.
  2. Each found plan's states are inside the h^2 rows (`taskcore dead --plan`).
  3. The fast h^2 equals the plain h^2 row for row on the gate's tasks.
  4. With both switches off: fmi-plan's results are byte for byte the old ones on serve38 and serve33, and the
     explorer passes G9 (fake steps against explore.py).
  5. Deepening, on fake steps:
     - a target that times out at L0 is deferred and retried at 2 · L0 only after the entry's undeferred targets;
     - an entry with deferred targets is not exhausted;
     - a resumed archive keeps the counts.
  6. Report the check's bit:118 requests: what proves them dead and how long it takes.

## Results of the amendment "targets with no plan made cheap" (2026-11-03, written after the runs)

Branch e011-deadtarget. Tables and lists: `results/deadtarget.md`. Laptop, seed 1, nice 10, at load 3-15.

- **What it does now.** With `FMI_PLAN_DEAD=1`, the check's bit:118 = 1 request ends in 18-19 s as a proven no-plan
  ("no plan (the planner proved none)"), against 187-191 s with the 180 s limit and 607-612 s with 600 s. End to end,
  `fmi try` on try 15's job with the target forced ends its attempt in 15.6 s with that stop. With
  `FMI_PLAN_DEEPEN=120`, a dead target that no proof catches costs 120 s plus the reductions in its first round, and
  comes back with 240 s, 480 s, ... only once its entry has no other target left. Both switches are off by default.
- **What proves bit:118.** h^2: the goal fact is outside the pair fixpoint. Each of its 8,712 writers needs a pair of
  facts no reachable state holds (bit:386 = 0 with var:252 = 1, bit:387 = 0 with var:253 = 1, and nine more of that
  kind). h^1 reaches it. Tries 15 and 16 give the same task.
- **Gates.**
  - Gate 1: 41 distinct recorded requests were rebuilt (103 records: the check 25, B0 16, B3 60, E008 2), plus E008's
    serve38 and serve33; 319 recorded plans had their reading evicted or gone. No proof claims no plan where a plan
    is recorded, and each of the 40 requests with a plan returns the recorded plan of every record (101). serve38 and
    serve33 give the same bytes with the proofs on and off.
  - Gate 2: the states of all 53 found plans are inside the h^2 rows.
  - Gate 3: the fast h^2 equals the plain one on those 53 tasks and on 20,000 random tasks.
  - Gate 4: with both switches off, G9 passes (18 scenarios × 2 modes, 0 differing).
  - Gate 5: deepen_check passes.
  - Gate 6: above.
- **Fraction proven dead.** Over every target at entries 36 and 26 (89 each), h^1 proves none and h^2 proves one at
  each (bit:118 = 1). Over the check's 24 rebuildable distinct requests, h^2 proves the same one. On the profile's
  ring(38) survey, h^2 proves 2 of the 3 targets that ran out of time (var:52 = -1 stays open).
- **Cost.** h^2 stops once the goal is in its rows. On ring(33) requests with a plan the proofs take a median of
  0.38 s (max 1.4 s), against 13.8 s of reductions; on ring(38), 0.01 s. The dead one takes 4.5 s. The whole fixpoint
  takes 2.6-7.3 s on ring(33).
- **L0.** The 26 found plans with Fast Downward's search on the warm path took a median of 37.3 s of search, p90
  111.6 s and max 119.8 s, so L0 = 120 s.
- **Against the prediction.**
  - Gate: yes.
  - bit:118 proven dead by h^2 in under 30 s: yes (18.4-19.1 s).
  - h^2 cost on ring(33), predicted 3-10 s: the whole fixpoint took 2.6-7.3 s. With the early stop (added after the
    prediction), requests with a plan pay 0.4 s.
  - ring(38) under 0.1 s: yes.
  - 1-10 dead per entry and none by h^1: 1 per entry, none by h^1.
  - L0 in 30-60 s: no. It is 120 s, because the check's requests on the refinement reading take 89-120 s of Fast
    Downward's search at this load.
- **Interaction.** A try that only defers targets makes no new reached values, so it counts toward E008's stagnation
  stop. The stop rules are unchanged.

## Kitchen check, result (2026-11-03): stopped at 65 committed tries, the goal not reached

The check ran from 01:54 to 04:03 on round 1's binaries with `--search fd` (amendment of 2026-11-03), resumed once at
15 tries with `FMI_PLAN_TIME=180` (amendment above). It was stopped by hand at 04:03 with 65 tries committed and try
j00065 running, for the measured reason below; no stop rule had fired (the falsifier was at 80).

| | measured |
|---|---|
| tries | 65 committed, 0 errors; wall per try median 31.6 s, max 998 s; 1.69 h of try wall, 1,772 CPU s |
| entries | 226; rooms 33, 38, 35, 29, 34 and 28 (6 of the 10 allowed); the latest frame 10,674 |
| facts reached (owner, state, room) | owner:377 = 1, state:367 = 1, state:315 = 1, state:428 = 1, and the six rooms |
| targets | the last target of 29 tries was a value of var:103 (the sentence's first object, a fact since the refinement); 33 tries ended on a var target, 23 on a bit |
| the goal | a target once: j00061, tier 0, from a room-28 entry at frame 3443 on scope (28, 33, 41); the planner reached its 180 s limit with no plan |
| planner limits reached | 10 requests, each 180-191 s with no plan; j00065 spent 8 attempts on such targets (bit:87, class:568:6 = 0, owner:567, owner:568, owner:578 = 0, var:197 = 1, 2, 3), 24 minutes |

**Why it was stopped.** With targets in rotation, the goal, tried once, comes again only after the cell's other targets;
from the room-28 entries those were targets that reach the planner's limit, each costing 3 minutes, so a try took about
25 minutes and the 15 tries left to the falsifier about 6 hours, with no retry of the goal in sight.

**What it shows.**
- The explorer leaves the start rooms on its own: 6 of the 10 rooms, an object taken, two doors' states changed, the
  bar reached, all by tries a few seconds to minutes long.
- Two things bind, neither of them the per-try speed of round 1: targets with no plan cost the planner's whole limit
  (round 2's no-plan proofs and deferred retries), and targets that are values of a refined variable (var:103's
  values) take most of the tries.
- The goal's request from frame 3443 did not plan within 180 s, where the same goal planned in 8-15 s from frame 3204
  in E006-E008; whether it needs the cook's timer from there, or more time, is open.

## Amendment 2026-11-03, after round 2: the no-plan proofs merged into round 2, and what is measured

Written after the merge built and its cargo tests passed, before any gate or measurement ran on it.

- **What.** Branch e011-r2 (736f306) gets e011-deadtarget (aacf668) and e011 (0bde069, the kitchen check's result).
  The proofs run on the reduced task before every search arm, the Rust lama-first-costs included. "Everything on" is
  env-r2.sh's ON2 with `FMI_PLAN_DEAD=1` and `FMI_PLAN_DEEPEN=120`; "everything off" is OFF2 with both off.
- **Gates run again,** the ones the two branches touch:
  - G7 with everything off, and with the fast forms in every check mode and the proofs on;
  - G10 with everything on (the Rust lama-first-costs: strict, ready, ready with the old tail) and everything off
    (Fast Downward's binary: strict, ready);
  - G11 on B3's sequences below: the reference settings with everything off and with everything on, the fast
    settings with everything on; every new entry verified from boot;
  - the amendment's gates 1-5 on the merged build. Gates 1-3 replay the recorded requests with everything on (the fast
    forms; the Rust lama-first-costs for Fast Downward's groups). Gate 4: G9 with both switches off, serve38 and
    serve33 the same with the proofs on and off, and the cargo tests. Gate 5: deepen_check.py.
- **What is measured.**
  1. **B3,** warm, with the job memo off (as round 2's amendment). The fast settings run three resident processes,
     interleaved try by try:
     - round 1: r1/fmi with Gbfs;
     - round 2 alone: round 2's fmi as built from 736f306, copied to target-e011-r2/r2, with ON2;
     - the merged build with everything on.

     Reported for each: the per-try median, max, mean and sum over the 15 tries, ring(33) and ring(38). The reference
     settings run the merged build with everything on and with everything off, interleaved, for G11.
  2. **The check's two bit:118 = 1 requests,** end to end. `fmi try` runs on copies of the check's tries 15 and 16 with
     the target forced, as in the amendment's end-to-end check. Each try is cold, in its own process, with the check's
     limit of 180 s (FMI_PLAN_TIME), in three forms:
     - round 1: r1/fmi with `--search fd`, Fast Downward's binary, the check's settings;
     - round 2 alone: ON2 with `--search fd --tail`;
     - the merged build with everything on.

     The tries alternate one by one, each under the heavy lock. The refinement reading is B0's j00000 refine-01. It has
     the check's start file (same sha1) and the same records and actions, and the check's copy was evicted.
- **Prediction.**
  - Every gate passes with everything off and on.
  - **B3 fast.** The merged build's tries equal round 2's in everything: no request in the 15 tries is dead or takes
    120 s.
    - Its median is within 0.5 s of round 2's.
    - Each ring(33) try is 0.4-1.5 s slower than round 2's, from the proofs at about 0.4 s for each searched request.
    - Its sum is 1-5% above round 2's.
  - **bit:118.** The merged build ends each try in 15-25 s. Round 1 and round 2 alone take 185-200 s each: the 180 s
    limit plus the reductions.

## Amendment 2026-11-03, during the merged build's measurement: the heavy lock held across the fast sequence

- **Measured.**
  - The fast B3's first start lost its third sequence (the merged build) at its first try. b3r2.sh took the heavy lock
    on fd 9, the third sequence's fifo, so that process saw its input end. b3r2.sh now holds the lock on fd 200.
  - The second start ran each sequence's first try (j00000, 3.1-5.0 min each at load 8-15). Then it waited more than 25
    minutes for its next single try: another experiment's steps held the heavy lock, and several more were queued for
    it.
- **What changes.** The fast B3 runs again from an empty start. The heavy lock is taken once and held for the whole
  sequence, then for the two bit:118 tries still to run (round 2's and the merged build's on try 16). Before, it was
  taken once per try. The tries still alternate one by one between the three processes, so they meet the same load,
  and no other heavy step runs between them. Everything else is as in the amendment above.
