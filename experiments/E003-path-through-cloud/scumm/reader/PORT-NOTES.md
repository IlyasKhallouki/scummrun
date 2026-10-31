# scumm-reader: port notes

The Rust port of reader.py, both halves, and of domains.py (decisions.md D7, layer 3). The machine-free half
(domains, Scope, FactTable, Records, Builder, build_task, prune, the starts, the packed writer) builds on `scumm-ir`
alone; the machine-dependent half (`machine.rs`: reader.py's `Reader`, `SymOracle`, `SymPath`) is a set of hooks on
`scumm-machine`, whose rows 0-2 the reader uses (no settle, decision D5). With it the Rust Builder reads a scope with
no Python and no playback. The Python is the oracle: every part below matches it with 0 differences. Read this file
first.

Frozen Python sources (sha1 prefixes): reader.py 94d495853f07, domains.py 19932b58530a, model.py cc630446bdbe,
task.py c356a6df633a. `tests/oracle.rs` regenerates the static oracle when one of them changes.

## What is ported

| module | ports |
|---|---|
| `domain.rs` | domains.py `Domain`: `of`, `label`, `test`, `add`, `exact` |
| `domains.rs` | domains.py `build` (all of `Domains`: domains, why, lossy, incomplete, counts, notes), `fold`, `decide`, `live`, `_Reader` (call sites, hook units, the locals fixpoint), `_closure` |
| `scope.rs` | reader.py `Writes`, `scope_writes`, `entry_writes`, `made_verbs`, `walk_verbs`, `stop_points`, `Scope.of`, `Scope.other_b`, `_live_bound`, and `Reader._hook_writes` (the inventory hook summary: it needs the IR only) |
| `table.rs` | reader.py `FactTable`: the reader's domains (hook and cursor variables, room, args, saved, pos, escpos), `kind`, `value`, `lazy`, `label`, `points` |
| `record.rs` | reader.py `Record`, `Event`, the effects, `Cond`, `Copy`, model.py `Unknown`; golden.py's canonical encoding both ways |
| `builder.rs` | reader.py `Builder`: `_pin`, `_sentence_base` (the ~frozen variant), `placements`, `_b_verbs`, `b_values` (B = "other"), `inputs`, `start_events`, `_sig`, `event_pins`, `run` (splits past PATH_LIMIT), `record` (args, the slot positions, the next events), `positions`, `effect` (conditional writes, copies of saved verbs, step counters, guesses), `Reader._stepped`, `unknown_targets`, `read_all` (passes, `extend_domains`, `_reset`), `_read_once` (event rounds, the replans' args-0 events, SIG_LIMIT, the folded lists' rewrite), `_limit`, `_run_jobs`; golden.py's reader file |
| `task.rs` | reader.py `build_task`, `prune`, `task_start`, `replan_start`; relevance.py `relevant`; task.py's `Task`, `Action`, `Effect` |
| `packed.rs` | export_records.py `Writer`, `export_records`, `export_task`, `_cont`; `RecordStream` (records written as made) |
| `machine.rs` | reader.py `Reader` (the subclass of model.Machine: every override, wrapper and method of its own), `SymOracle`, `SymPath`, `Fork`, `_to_sym`, `_consistent`, `MEMO_GETTERS`, `STEP_OPS`, `LAZY_METHODS`, `ESC_KEY`; model.py `_eng_input` (its switch off), which the machine port lacks (row 4's); the `builder::Machine` the Builder reads with |
| `replay.rs` | a `Machine` that plays back the Python reading's machine runs; `Compare`, which runs the Rust machine half on each captured run too and compares the two |
| `oracle.rs` | the comparisons; `bin/reader_port.rs` runs them and the timings |
| `locals.rs` | the switch `--live-locals` (below, "Live locals"): the locals a unit's code reads, from the IR; not in reader.py |

`write_sas` is taskcore's: with `--fold off --reduce none --encoding task` its file is task.py's, byte for byte.
taskcore's default encoding is now `model` (metric 1 when the file has pick or goal operators), so the
`--encoding task` flag is needed for task.py's bytes.

## The interface the machine-dependent half fills

`builder::Machine`, one method:

    fn run(&mut self, cx: &Ctx, label: &str, pins: &Pins, pre: &Pre, what: &What) -> RunOut;

It is reader.py's `Reader.run_sentence_sym` (`What::Sentence`), `run_click_verb` (`What::Verb`) and `run_event`
(`What::Event`). `cx` gives the game, the scope and the `FactTable` (domains, kinds, `value`, `lazy`, the start,
the slot points, the args lists). `pins` are the oracle's fixed values (ints, or a collapsed value's
representative as an `Unknown`); `pre` is the run's precondition (fact -> labels).

`RunOut` is what `Builder.run` and `Builder.record` read from the run (reader.py names in brackets):

- `paths`: every path of `Run.paths`, in order, as `PathOut`: `end` (None for a path with no end; the real machine
  always gives one), `sym` (a `SymPath`), and for a path that can be a record: `pre` (`SymPath.pre`), `writes`
  (`Path.writes`: ints and `Unknown`s, `Cond`, `Copy`, step values with reason "step"; kept sorted by key, item 5), `kept` (`SymPath.kept`, the slots the path left waiting), `unknown_writes`, `assumptions`
  (model.py's 4-tuples) and the last five `notes`;
- `limit` (`Run.limit`), `value_caps` (`sum(Run.value_caps.values())`);
- `first_fork` (`Reader.first_fork`): the first fact the run forked on, for the split;
- `rd`: the run's increments of `Reader.counts`, `forks` and `fork_keys`, keys added with 0 included (the split
  reads "forks on read" and "forks on non-fact tests"; golden.py dumps all three).

`machine::Reader` fills it (next section). `replay::Replay` fills it from the captured runs, and `replay::Compare`
runs both and compares them. reader.py's `check_machine` checks that model.Machine still has what the Reader
wraps; in Rust the `Hooks` trait and the compiler do that, and `tests/oracle.rs::check_machine_lists` pins
reader.py's lists (LAZY_METHODS, CALLED, PATH_FIELDS, FRAME_FIELDS, DROP_ENDS), so a change there shows.

The Builder keeps no machine state. `Builder::last_pins` stands for the Reader's last oracle, whose pins
`event_pins` lists slot keys from (below).

## The machine-dependent half (`machine.rs`)

`machine::Reader` is reader.py's `Reader`: a `scumm_machine::Machine` with rows 0-2's features (own_entry, dispatch,
calls, room_change), PATH_LIMIT 1024, and reader.py's overrides as `scumm_machine::machine::Hooks`
(`ReaderHooks`). Each hook is the Reader method of the same name, in its order of reads, writes, forks and counts:

| hook | reader.py |
|---|---|
| `advance` | `_advance`: a snapshot per step; a `Fork` reruns the step from it once per value left; `_forget` after an op of MEMO_GETTERS or an unknown write; `_drive`'s `_to_sym` for a run_sentence's first path |
| `read` | `read`: steps, the clear mark, `pre`, pins, VAR_ROOM and VAR_ROOM_RESOURCE as the room, constants, a fork or a lazy read for a fact, a non-fact unknown |
| `oget`, `okeys` | `SymOracle.get` (pins, the merge's view, constants, lazy facts) and `SymOracle.keys` (the universe, then the pins it lacks) |
| `val`, `arith_at`, `getter` | the names a negated, computed or MEMO_GETTERS non-fact gets |
| `test`, `jump` | one frame's memory of non-fact tests (`_memo_ident`, `_consistent`, `_pending`, `SymPath.nf`), "forks on non-fact tests" |
| `join`, `merge` | `_join`'s extension (pre, steps, kept slots, freezes) and `_merge`'s (the view, `nf`, the kept slots' locals) |
| `clear` | `clear`: the mark, or the machine's clear lazily |
| `op` | STEP_OPS (`_make_step`: cutscene, endCutscene, the soft cursor and userput ops), `_op_saveVerbs`, `_op_restoreVerbs`, `_op_VerbOps` lazily |
| `eng` | `_eng_scene` (left the scope), LAZY_METHODS' `_kill_room`, `_eng_scene_begin`, `_eng_scene_load`, `_eng_scene_ego`, `_eng_scroll_note`, and the two steps the runs start with: `_eng_input`, `_eng_resume_at` |
| `eng_script` | `_eng_script`: the inventory hook's writes guessed (`scope::hook_writes`) |
| `finish` | `_finish`: the kept slots |
| `set_freeze`, `count`, `freeze`, `unfreeze` | `_set_freeze` lazily, `_count`, `_freeze`, `_unfreeze` |
| `owned_by`, `find_inventory` | `_owned_by` (the inventory is not enumerated) and model.py's `_find_inventory` over it |

`Reader::run` is `run_sentence_sym`, `run_click_verb` and `run_event`: it sets the run's pins and seed, starts the
machine (`run_sentence`, or `_go`'s path with the "click" or "runAllScripts" call) and turns its Run into `RunOut`.
`may_stop` and `_live_slots` need no hook: with calls on, `may_stop` is not reached, and `_live_slots` runs only
inside lazy methods already.

How Python's mechanisms map:

- `Fork` (an exception) is a panic whose payload is `ForkOn(fact)`, thrown with `resume_unwind` (no panic hook, no
  message) and caught in `advance` by `catch_unwind`. The step stops at the read, as Python's does; `advance` then
  resets what the methods' `finally` blocks reset (`_lazy`, `_step_key`, the merge's view, `_pending`). The kitchen
  forks on read 38,454 times.
- `SymPath`'s fields are `SymX`, the path's `ext` (`Path::ext`), cloned with the path; its maps are `Rc`s that a fork
  shares until one changes them (`Rc::make_mut`). The Reader's state (`St`: the pass's FactTable and the scope, the
  run's pins and seed, the counters, `_lazy`, `_step_key`, `_pending`, the view) is the machine's `ext`.
- A key's kind, domain and start value come from a copy of the Builder's FactTable, taken at the first run and again
  when a pass widened a domain (`extra` changed); each key's are worked out once per pass (`Info`).
- The reader's own keys in the machine's fact store: `saved:V:I` is `Fact::Class(i32::MIN + V, I)`, the clear mark
  `at:<unit>:*` (`esc:<unit>:*`) is `Fact::At(unit, i32::MIN)` (`Fact::Esc`); both print as reader.py's strings in
  `RunOut`. A path's assumptions are keyed by `PKey`: a fact, or `args:<unit>` (from the run's precondition).
- `Cond` and `Copy` are unknowns of class 1 and 2 (`Unknown::class`): they never equal an `Unknown` with the same
  fields, as Python's dataclasses compare, and what the machine makes from them (a join, arithmetic) is class 0,
  as model.py makes a plain `Unknown`. The reader's reasons ("fact", "not a fact", "collapsed value", "step",
  "conditional write", "copy") are `Reason::Other`. `("sym", n)` in `Path.freeze` is `(i32::MIN + n, false)`.
- `run_click_verb` and `run_event` start with a call of one step (`("input", ...)`, `("resume_at", ev)`); the
  machine has no such steps, so the step is a `Step::Note` with a marked text that the `eng` hook runs.

## Machine changes

The hooks need these changes outside the reader crate. Each is additive; a machine with no hooks (every caller in
scumm-machine) runs as before: the machine goldens of rows 0-2 still give 0 differences (717 files, `golden.py
compare --runs row0,row1,row2 --traces`), and after the merge with main all nine runs (rows 0-5 and the three switch
runs) give 0 differences in 2,151 files (`golden.py compare --runs row0,...,row5-esc_key_at_pre --traces`).

| file | change |
|---|---|
| `machine/src/machine/hooks.rs` (new) | the `Hooks` trait (one default method per hooked method: its `_base`), the dispatchers (`Machine::advance`, `read`, `oget`, `okeys`, `val`, `test`, `getter`, `owned_by`, `find_inventory`, `clear`, `arith_at`, `jump`, `wait`, `op`, `join`, `merge`, `eng`, `eng_script`, `finish`, `count`, `set_freeze`, `freeze`, `unfreeze`), and public wrappers for what a hook calls (`vals`, `vals_ref`, `h_write`, `h_halt`, `h_note`, `h_done`, `h_resource`, `h_push_frame`, `h_stop_script`, `h_live_slots`, `h_cid`, `h_drive`, `h_waits`) |
| `machine/src/machine.rs` | `mod hooks` and `pub use hooks::Hooks`; `Machine.hooks` and `Machine.ext` (two fields, set to None in `Machine::new`); renamed to `_base` and made public, bodies unchanged: `oget`, `advance`, `read`, `val`, `test`, `getter`, `owned_by`, `clear`, `arith_at`, `jump`, `wait`, `op`; `prefixed` lists the oracle's keys through `self.okeys` (one line) |
| `machine/src/machine/calls.rs` | renamed to `_base` and made public, bodies unchanged: `join`, `merge`, `eng`, `eng_script`, `finish`, `count`, `set_freeze`, `freeze`, `unfreeze`, `find_inventory`; `eng_resume` lists the oracle's keys through `self.okeys` (one line) |
| `machine/src/path.rs` | `Path.ext` (a hook's path fields, the `PathExt` trait, `Clone` for its box), `Sig.ext` (a hook's join signature extension; empty in `Sig::from_words`, which `Sig::round` uses), `Store::remove` |
| `machine/src/value.rs` | `Unknown.class` (0 in `Vals::mk`, `mk_set`, `one_of`) |
| `ir/src/analysis.rs` | `Reason::Other(&'static str)` and its arm in `Reason::as_str` |

**The merge with main (rows 3-5).** Main's rows 3-5 kept every change above. Its `Sig` is now a vector of words and
a round flag; `ext` stays a field next to them, set in `Sig::from_words`. Its `oget` caches its unknowns per run; the
cache is `oget_base`'s. It adds no `Unknown { .. }` literal. `Frame.locals` is now `Locals` (shared, sorted); the
reader reads it with `as_slice()` (three lines of `machine.rs`, the same values). The methods rows 3-5 add (`settle.rs`,
`dialog.rs`, `esc.rs`) are none that reader.py's `Reader` overrides, and the Reader runs without settle, dialog and
esc, so none needed a hook. Row 4 now has model.py's `("input", ...)` step (`Step::Input`); `run_click_verb` still
uses its marked note (`machine.rs` `eng_input`, model.py's `_eng_input` with its switch off).

## The oracles and the results

Made by `reader_port_oracle.py` (new; from `experiments/E003-path-through-cloud`, `../../.venv/bin/python`, nice
10), into `out/reader-port/`. Every command runs under PYTHONHASHSEED=0 (the script starts itself again with it;
item 3 says why), and a capture numbers each job by its place in its batch, so two jobs of one batch with one label
keep their own runs.

    reader_port_oracle.py static                                  # 15 s
    reader_port_oracle.py capture room33                          # 6 s
    reader_port_oracle.py capture kitchen --pickle FILE           # 14 min, 4 workers, peak ~2 GB per process
    reader_port_oracle.py sas --pickle FILE --work DIR            # write_sas on the kitchen, ~30 min, 6.8 GB peak
    reader_port_oracle.py bench                                   # Python's times

A capture is the Python reading with every machine run the Builder asked for (inputs and outputs), in the order
the parent process handled the jobs. It writes the reading in golden.py's format too: for room 33 and the kitchen
it is equal to `out/golden/reader/*.jsonl.gz` (sha256), so the captured runs are the goldens' runs.

| oracle | compared | differences |
|---|---|---|
| domains.build: room 33's start (seed 1), the kitchen's (S1#7 pre), Part I's with its goal, the engine's init | 4 builds, every field of `Domains` (2,708 domains each, 989 for init) | 0 |
| Scope.of: room 33, the kitchen, size_scaling's k = 0..3, all 83 rooms | 7 scopes: units (28 to 1,172), objects, verbs, dialog verbs, walk verb, universe (428 to 18,625 keys), counts, write sets | 0 |
| the inventory hook summary (`Reader._hook_writes`) for every global script; `entry_writes` for every verb | 178 + 16 | 0 |
| the Builder's machine-free parts, room 33 and the kitchen: FactTable (domains, counts, points, start), kind and domain of 4,158 / 7,082 keys, `sentence_scripts`, `ego_values`, `inputs()` (80 / 1,850 jobs: pins, pre, what, checks) and its counts, `start_events()`, `unknown_targets` | 4,246 / 8,964 items | 0 |
| golden records parsed into `Record` and written back | 878 + 72,256 lines | 0 |
| the whole reading by the Rust Builder over the captured runs, against golden.py's file (records and every builder and Reader counter) | room 33: 880 lines, 166 runs; kitchen: 72,258 lines, 18,127 runs, two passes (21 domain values added), 4 lists past SIG_LIMIT, 2 runs past the path limit (not split) | 0 |
| the kitchen task: Rust build_task (and relevance + prune) -> packed file -> taskcore SAS, against task.py write_sas's file (sha1, PYTHONHASHSEED=0) | full: all s17, all replan, no-211 s17; pruned: all s17, all replan, no-211 s17; and `export_records` (no Task) and `RecordStream` (written as made) for all s17 | 0 (all 8 sha1s equal) |
| the Rust machine half run by run: every captured run made again from its inputs (pins, precondition, what) and compared field by field (`reader-port runs`; the Builder goes on with the captured run) | room 33: 166 runs; kitchen: 18,127 runs (every path's end, pre, writes, kept slots, unknown writes, assumptions and last five notes; limit, value caps, first fork, the Reader's counters) | 0 |
| the whole reading by the Rust Builder and the Rust machine half, no Python and no playback (`reader-port read`), against golden.py's file | room 33: 880 lines; kitchen: 72,258 lines | 0 |
| whole runs: `reader_run_dump.py` against `reader-port dump-runs` (every path field: all assumptions, notes, errors, unmodelled statements and turns, end_at, steps) | 6 room 33 runs, 6 kitchen runs (2 of the second pass, made with the first pass's domains on both sides) | 0 |
| model.Machine after the machine changes (`machine-run`, `golden.py compare --traces`, machine/PORT-NOTES.md) | rows 0-2: 717 files; after the merge with main, all nine runs: 2,151 files | 0 |
| synthetic readings: the frozen Python Builder over a fake machine that makes random paths, captured by reader_port_oracle.py's capture, replayed by the Rust Builder; then build_task, relevance, prune, both starts, export_records and RecordStream through taskcore against write_sas | 40 readings (190 to 16,495 records; room 33 and the kitchen; SIG_LIMIT 1 to 4, lists folded in 22; the caller's skip in 3; dropped domains in 3) and one with 119 labels that two jobs of one batch share (20,740 runs) | 0 in every reading; 120 SAS files equal (where write_sas fails, it fails in both); fold counts equal |

build_task folds 527 records (71,729 actions; no-211: 71,721); relevance keeps 71,324 (no-211: 71,316), as in
Python. On the kitchen, `RecordStream` sets 4 `args:` values of folded lists to 0 in its second pass and folds
nothing more.

The kitchen and room 33 do not reach some paths: a split of a run past PATH_LIMIT on its first fork (the kitchen's
two limited runs forked more on non-facts), a step on a collapsed value, the caller's `skip`, and records that only
the rewrite of a folded list makes identical. The synthetic readings reach all four: every one of the 40 splits
runs and steps on collapsed values, and 22 have records that fold only after the rewrite (2 to 1,062 each; 15 of
them on room 33, where write_sas succeeds and the SAS files are equal). `_pin` on a fact with no domain is
reached by none (ported line for line). The synthetic harness is not part of the crate (it needs Python and a
fake machine); `task::tests` pins the fold parts against build_task's key after the rewrite.

The run comparison leaves out the Reader's counter keys at 0: a Python worker's Counter keeps a key `c[k] += 0`
added only when the key was new to that worker, and the reading's totals are the same either way (the readings
compare every counter).

To find where the Rust machine half first differs (from `experiments/E003-path-through-cloud`):

    scumm/target/release/reader-port runs NAME [--max N]                 # every captured run; prints "run N" per difference
    ../../.venv/bin/python reader_run_dump.py NAME --runs N,... --out P  # the runs whole, made again in Python
    scumm/target/release/reader-port dump-runs NAME --runs N,... --out R # the same from Rust
    ../../.venv/bin/python golden.py compare-file P R

`cargo test --release -p scumm-reader` runs the fast ones (1.5 s after the build), room 33's run comparison and its
reading with the Rust machine half among them. `-- --ignored` runs the kitchen: records written back (25 s), the
reading against the golden over the captured runs (100 s), every captured run against the Rust machine half
(about 2.5 minutes), the reading with the Rust machine half (about a minute), the task's six SAS files and the
streaming writer against write_sas (about 5 minutes; needs `python-sas.json` from `reader_port_oracle.py sas`).

## Time and memory, Python against Rust

The laptop (8 cores, 23 GB) on 2026-10-24, with other heavy jobs running (load 6 to 9): single runs vary by up to
2x. Python: `reader_port_oracle.py bench` (best of 3) and taskcore_kitchen.py `pyref` (through
`reader_port_oracle.py sas`); Rust: `reader-port bench` (best of 3), `reader-port bench-task`, and taskcore's own
stats. Peak memory: getrusage / VmHWM of the whole process, with the reading it holds (Python: the unpickled
reading, 1.05 GB; Rust: the Builder and its records after the replay, 0.75 GB); taskcore runs in its own process.

| step | Python | Rust |
|---|---|---|
| domains.build, one start (four starts) | 1.06 to 2.20 s | 0.09 to 0.21 s |
| Scope.of (room 33 ... all 83 rooms) | 0.05 to 1.57 s | 0.005 to 0.155 s |
| peak memory of those (game loaded) | 80 MB | 103 MB (with the oracle file parsed) |
| kitchen build_task | 54.9 s, 1.56 GB | 3.0 s, 1.26 GB; or streamed into the packed file with no Task: 2.3 s, 0.76 GB |
| kitchen write_sas, start s17 (328.6 MB file) | 212.2 s + 2.1 s for the text; 6.75 GB | taskcore from the packed file: 15 to 24 s, 64 MB (packing: 1.5 s) |
| build_task + write_sas, s17 | 269 s, 6.75 GB | 26.6 s (streamed) to 28.8 s; 0.78 GB (streamed) to 1.27 GB, plus taskcore's 64 MB |
| test_reader's pruned task: build_task, relevance, prune, write_sas (s17) | 70.4 + 98.5 + 23.7 + 293.0 + 1.7 = 487 s, 4.1 GB | 3.0 + 5.9 + 3.5 + 7.4 = 19.8 s, 1.39 GB |

A second Python run (2026-10-25, PYTHONHASHSEED=0, other jobs running) took 100 s for build_task and 353 s for
write_sas, at the same 6.75 GB.

The Rust Builder's replay of the captured kitchen runs takes 95 s, almost all of it parsing the 1 GB of captures.

**The kitchen's reading** (`read_all`: 1,850 inputs, then events, two passes, 18,127 machine runs, 72,256 records),
2026-10-25, with another workflow's jobs running. Rust: `reader-port read kitchen` (nice 10), the reading alone
(the game, domains.build and Scope.of take 0.5 s more), one thread. Python: `golden.read_scope` with 1 worker
(nice 10, PYTHONHASHSEED=0), the times the task gives and one run here. Peak memory: VmHWM / getrusage of the
process, which holds the reading.

| | Python | Rust, one thread |
|---|---|---|
| time | 768 s in one process, 863 s with 4 workers (the task's figures); 1,172 s in one process here at load 4 to 8, of which 474 s inside the machine runs (`Builder.seconds`) | 43.5, 49.9 and 53.9 s in three runs at load 3 to 5 (46 s at load 2) |
| peak memory | 660 MB | 755 MB |

So the Rust reading is about 15 times faster than Python's 768 s, and needs about 15% more memory: each Rust record
holds its own strings (fact keys, cites, notes), where Python's records share the machine's string and tuple
objects. Interning them in the Builder is the candidate if memory matters.

Where the Rust time goes (gdb stack samples of the reading, 127 of them): the machine's runs 43% (forks, the
per-step snapshot, joins and merges), turning each run's paths into the Builder's `PathOut` (fact keys as strings,
values as the Builder's `Val`) 18%, `Builder.record` 27%, the rest the Builder's own work. `Builder.effect` keeps
each key's kind and domain for the pass (`fact_info`); before that the reading took 52 s at load 2.

## ELSEWHERE: the fix for the scope-1 finding, as a switch

**The finding** (item 1 below). `Builder.positions` writes `pos:<unit>` := W for the place W a path left a slot
at. W is a place in the code the machine ran. The domain of `pos:<unit>` holds only NOWHERE and the places of the
scope's own units with that name (`FactTable.points`, from the scope's universe). The name `local:N` is shared by
every room's local script N, and the room fact's domain is wider than the scope (every constant room change in
it). So a path can run the local script of a room outside the scope and leave it at a place the domain lacks.
taskcore then stops on the task: "an effect writes a value its domain lacks".

**The cases.** `reader-scope` lists them in its report (`outside_points`: fact -> place -> paths), with the switch
on or off.

- k = 1 (rooms 28, 33, 35, 38): 126 records, all of them global 47's event from offset 35 (`startScript(Var[193])`
  with var 193 = 200 or 208), each with a room precondition outside the scope (29, 34, 41, 70, 79, 81, 82, 83, 85
  or 96). `pos:local:200` at 1, 5, 10, 17, 21, 34, 100, 151 and 212; `escpos:local:200` at 6 and 205;
  `pos:local:208` at 10, 13 and 28.
- k = 2 (rooms 28, 29, 33, 34, 35, 38, 41, 83, 85, 96): 176 records, again all of them global 47's event from
  offset 35, each with a room precondition outside the scope (27, 31, 32, 36, 43, 57, 70, 81, 82 or 218).
  `pos:local:200` at 12, 16, 21, 34 and 45; `escpos:local:200` at 4 and 35; `escpos:local:208` at 47;
  `pos:local:202` at 56; `pos:local:203` at 20; `pos:local:208` at 13 and 98.

No record at k = 1 or k = 2 writes a room outside its scope: a path that changes to such a room ends there
(`Reader._eng_scene`, "left the scope"). So from a start inside the scope none of these records can apply. They
can apply only from a start, or a replan start, in a room outside the scope.

**The switch**: `FactTable.elsewhere` (`reader-scope --elsewhere`), off by default. When it is on:

1. every `pos:` and `escpos:` domain also holds ELSEWHERE (-2, `table::ELSEWHERE`);
2. a place written to a slot that is none of the scope's points for its name is written as ELSEWHERE (in a guess
   of several places, each such place; a guess left with one value is a set);
3. no precondition set on `pos:` or `escpos:` holds ELSEWHERE (`positions` takes it out of every set it builds);
4. `task_start`: a unit whose slot the start shows at such a place, and at none of the points, starts at ELSEWHERE,
   not NOWHERE. The harness does the same for a reading whose domains hold ELSEWHERE (`replay.task_start`,
   `replay.replan_start`, `packed_reading.Points.elsewhere`); a reading made with the switch off is read as before.

**Why a plan never relies on a value the engine might not have.**

- A plan reads a `pos:` or `escpos:` value only through an action's precondition set on it. No condition of an
  effect is on these facts (none of the k = 1 records has a conditional write or a copy on a place). Every such
  set is built by `positions`, from the path's reads of `at:<unit>:P`. With
  the switch, no set holds ELSEWHERE (rule 3). So while a unit is at ELSEWHERE, no action whose path read that
  unit's place at all can apply. An action that can apply read nothing about the place, so neither its run nor its
  effects depend on it.
- ELSEWHERE is written only where the path left the slot at a place W that is none of the points. If the model is
  right, the engine's slot is at W. "ELSEWHERE" claims only that it is at a place that is none of the points, which
  is true. The slot fact (`local:N` = 1, running) is written as before.
- The two other values would claim more. W itself is outside the domain, and the writer raises on it. NOWHERE
  claims "no live slot", which is false, and a path can read a slot's liveness from its places: a room's entry code
  is live when one of its `at:entry:` keys is 1 (model.py `_pre_slots`).
- A unit leaves ELSEWHERE only by a write that does not depend on the old place. A stop or a kill writes NOWHERE. A
  start or a resumed slot writes its point. A room change kills the room's local slots: it writes the place of
  every local slot name the scope has points for, NOWHERE or where the new room's code left it (all 6,361 room
  change records at k = 1 write local places). The engine's stopScript and its room change act on whatever slot
  runs that number. A name the scope has no point for is not written by a room change; it stays at ELSEWHERE,
  which blocks, never misleads.
- The engine's later turns of that slot are not modelled: an event at W would need `pos:<unit>` = {W}, which
  `positions` cuts to the domain, the empty set, and taskcore drops an action with an empty set. At k = 1 no record
  has such a set, and none has ELSEWHERE in a set. The replay harness sees the slot's writes as changes with no
  prediction and replans from the engine's state, as for anything else the model lacks.
- A start in the engine can have a slot at a place outside the points. reader.py's `task_start` gives NOWHERE
  there, which rule 4 replaces with ELSEWHERE, by the same argument.
- A place that happens to equal one of another room's points is in the domain and is written as itself. That needs
  no rule. Every event of a room's local script has its room in its precondition (all 86,792 such records at
  k = 1), and a room change writes every local slot's place again. So a place of room A's script never satisfies
  an event of room B's.

What it costs: an action that only tests "not at P" cannot apply while the unit is at ELSEWHERE. That is
conservative, never unsound.

**With the switch off**, the reader is reader.py's: room 33 and the kitchen give golden.py's files (the tests
above), and reader-scope's k = 1 reading is the same as before the switch, byte for byte (records sha1 e1174103...,
counters 173137f8..., packed file 29bc44eb...). taskcore stops on the full k = 1 task as task.py's writer did:
"event(script:47@35#a2bf3d907238)#150299: pos:local:200 := 10 is not in its domain".

**With the switch on**, room 33 and the kitchen give golden.py's files too (no place outside their points:
`room33_reading_elsewhere_on`, and `kitchen_reading_elsewhere_on` under `--ignored`). At k = 1: 237,262 records
and 620 folded, as before; the 126 records write ELSEWHERE. taskcore writes the whole k = 1 task (fold strict, no
reductions, `--start-check input`, start "start"): 168,899 actions, 11,349,709 effects, a 1.36 GB SAS file, in
173 s. With reductions on, both start checks (`input` and `needed`) pass and the task reduces to 0 actions: k = 1
holds no write of the Part I end's facts (results/task-size-scaling.log, finding 3).

## Live locals: events keyed on the locals their unit reads, as a switch

**The finding** (the first Part I attempt, results/vm-part1-seed1.json, and a diagnosis of it on the laptop).
An event's key is (unit name, unit, wait point, list of locals, Esc), and the list's number (`Builder.sig`) is a hash
of every local the slot holds. A unit that is started with arguments it never reads gets one list per value of those
arguments. In MI1 the sentence script starts a room's local 202 with `startScript(Var[116],[Local[1],Local[3],
Local[2]])`; room 31's local 202 (a long dialog, 105 wait points) reads only its Local[0], so its events were keyed
by up to 1,440 lists per wait point (SIG_LIMIT keeps 1,024), and each list walked the whole dialog. Folding such lists
into args 0 or lowering SIG_LIMIT is not sound: the args-0 event names none of a loop's next wait points, and a lower
limit folds the sentence script's real object arguments.

**The rule** (`locals.rs`). For each unit, S(u) is the set of locals any instruction of its code can read, from the
IR, over all its code (every verb entry of an object, every path; no flow): every `Local[n]` node in any
instruction's operands, conditions, start arguments, sub-ops and texts (`getInt(Local[0])` inside a text), but the
target of a plain `Local[n] = ...`. `+=`, `-=`, `*=`, `/=`, `++` and `--` read their target; an opcode inside an
expression (`Embed`) reads its target back. S(u) is "all locals" when the IR does not show the reads exactly:
a local read through an index (`Local[base + x]`, an `Index` of kind local, read or updated), an instruction the IR
did not parse, a code descumm did not decode (`unknownN(..)`, `...Unknown...`) with an operand that is not a
constant, a code inside a text whose operand bytes the engine may read as text (below), or any IR note but six that
leave every byte the engine runs in a shown instruction (`locals::EXACT_NOTES`: else-if targets read from the bytes,
else chains, a global's name read from the bytes or left as printed, a unit with no instruction). So a gap ("closer
gap", "else gap not a multiple of 3", "jump line not 3 bytes", "while without a closer at its end", "two instructions
at one offset"), a place the engine can start decoding that is no shown instruction's start ("jump to the end",
"target not an instruction offset", "verb entry not an instruction offset", "else-if target from the text rule,
unverified", "beginOverride not followed by a jump", whose 3 next bytes the engine skips, script.cpp:1711-1722), and
any note added later keep all locals.

*A code inside a text* (`locals::text_code_exact`). The engine converts a text in convertMessageToString
(string.cpp:1573-1706): 0xFF, then a code; codes 1, 2, 3 and 8 take no operand, 4 to 7, 9, 10 and 12 to 14 a 2-byte
word, any other code none (string.cpp:1690-1693); 0xFE is plain text there. descumm (descumm-common.cpp get_string)
takes a word after 8, 11, 0 and 15 and up (`unknownN(k)`) and reads 0xFE as 0xFF. Bytes descumm took as an operand can
so be plain text to the engine, and a plain 0xFF there starts an escape the IR does not show (`unknown8(1279)` is FF
04: with 0x4003 next, the engine reads Local[3]). A text code is exact only when no operand byte is 0xFF (a constant,
or a plain variable: Var[n] is the word n, Local[n] 0x4000 + n, Bit[n] 0x8000 + n), the operand has no index (descumm
reads the index word from the text, the engine's readVar from the script), the code is not 0xFF, and it is not
`sound(..)` (descumm skips bytes it does not show).

**The switch** `FactTable.live_locals` (`reader-scope --live-locals`, `reader-port read NAME --live-locals`), off
(None) by default. When it is on, `Builder.record` (where rule (a), "Local[i] from a fact", lives) drops from each
kept slot's list every local outside S(u) before the list is numbered, and numbers it together with the slot's
flags (`Builder.sig`'s text gets them, only with the switch on): resistant (the R flag) and owned as they are, and
recursive only as "true or not" (`locals::recursive`: true stays, false and unknown are written unknown, in the
event too). That is all anything reads of it: the engine reads a slot's recursive flag only in chainScript, which
passes it to runScript, which tests it (script_v5.cpp:834-850, script.cpp:65-66), and the model only compares it with
true (a start that is not recursive stops the running instance, the note at the slot's end). So the event's key, its
number (the `args:` write and the `args:` precondition of the event's own records) and the locals its run starts with
(`Event.locals`, which `machine.rs` `eng_resume_at` puts in the frame) hold only S(u), and two slots with the same
read locals but other flags keep two numbers. A dropped local is absent from the list: with
known args it would read 0, without them an unknown; no instruction reads it. Events with unknown locals (args 0:
the start's slots, the replans' copies, folded lists) are as before. The counter "lists of locals with locals their
unit never reads (left out: live locals)" counts the lists it changed. The reading's head (golden.py's file) gets a
field `live_locals`: `{"switch": "--live-locals", "slots": {slot: S(u) or null}}` for every unit of the game (a scope's
records can name events of room code outside the scope, the ELSEWHERE cases), a slot named as the engine's state names
it (`script:N`, `obj:N`; `local:N@R`, `entry@R`, `exit@R` for the unit's room R and each pseudo room on its resource);
packed_reading.py copies it into reading.json. With the switch off the field is absent and nothing else changes.

In MI1 (tests only): of 1,195 units, 1,193 have S(u) known exactly (750 read no local at all) and 2 keep all locals,
both for an indexed local: global 153 (`Local[0 + Var[300]]`) and room 75's local 200 (`Local[6 + Local[5]]`). The six
decompiles with descumm warnings all carry `unknown8(8224)` inside a verb's text: bytes 20 20, which the engine reads
as plain text after code 8 (no 0xFF), so their reads are exact (room 31's local 202 is not one of them). No text code
in MI1 has an operand byte 0xFF, an index, code 0xFF or `sound`, and every IR note in MI1 is one of the six (1,111
else-if targets that the text rule agrees with, 2 it does not, 9 else chains, 2 names from the bytes, 2 empty units).
Room 31's local 202 reads {0}, the sentence script {0, 1, 2, 3, 4, 6, 7}.

**Why it is sound.**

1. *The engine reads a slot's locals through its own instructions, and in two cases the model does not follow.* A
   slot's locals are `vm.localvar[slot]`. They are written when a script starts (`initializeLocals`, script.cpp:156-165,
   from the starter's argument list) and by `writeVar` (script.cpp:834-846). They are read only by `readVar` for a
   variable number with bit 0x4000, and only the running slot's (`vm.localvar[_currentScript]`, script.cpp:669-706).
   The number comes from the running instruction's own bytes: an operand (`getVarOrDirectByte/Word`,
   script_v5.cpp:371-381), a result's index word (`getResultPos`, script_v5.cpp:383-396), an indexed variable's index
   (script.cpp:573-580), or the codes 4 to 6 of a text it prints (`convertIntMessage`, `convertVerbMessage`,
   `convertNameMessage`, string.cpp:1647-1657, 1795-1890; code 7 names a string, not a variable, in v5). descumm prints
   each of these as `Local[n]` or `Local[n + x]` (and `getInt(Local[n])` in a text), which the IR parses into
   `Local(n)` and `Index { kind: Local, .. }`; a constant offset is folded at parse time (`Local[1 + 1]` is
   `Local(2)`); a text's codes are the IR's when their bytes read alike (above). Saving and loading copy the locals;
   that is no script read. Three workarounds read the running slot's locals inside other ops for particular games
   (script_v5.cpp:1578 and 1599-1610 for MI2's Ultimate Talkie edition, 3807 for Indy3's FM-TOWNS release); none
   applies to MI1, and a game one applies to needs those locals in S(u) of the units those ops run in.

   Two engine reads of a local are not in the running instruction's bytes. In both the model already reads something
   else, with the switch or without it, so the switch keeps exactly what the model reads (point 3) and adds no
   difference from the engine.
   - *A stored text.* An object's or actor's name, a verb's text or a string resource is converted when it is printed
     or drawn (`convertVerbMessage`, `convertNameMessage`, `convertStringMessage`: string.cpp:1875, 1904, 1971;
     `drawVerb`: verbs.cpp:1075), and its codes 4 to 6 read the locals of the slot running then, which may be another
     unit's slot and a local outside its S(u). That changes only the text shown (its digits, names and length): a
     conversion writes no variable, and the model gives every text an unknown value (machine.rs `val_base`).
     `locals::stored_text_locals` lists the locals that texts stored by an instruction (setObjectName, VerbOps,
     actorOps, PutCodeInString, anything but print and printEgo) name. MI1 has none: its 10 local codes are in print
     and printEgo lines, which the engine converts at once with the printing slot's own locals. A text built at run
     time (SetStringChar; MI1 sets characters of string 37 from Local[0]) is not listed, and needs the same argument.
   - *An index that carries.* readVar adds the index k to the whole base word, flag 0x2000 included, then clears 0x2000
     (script.cpp:573-580). A carry or a borrow can so give a local's word: for Var[b + k] when b + k is 0x2000 to 0x5FFF
     (or -0xE000 to -0xA001), for Bit[b + k] when it is 0xA000 to 0xDFFF (or -0x6000 to -0x2001); the engine then reads
     Local[(b + k) & 0xFFF], or stops with an error past Local[20]; getResultPos does the same for a write. The model
     reads Var[b + k] or Bit[b + k] there (scumm-machine's `val_base`), a variable no instruction writes, so a
     constant, and writes it. In MI1 the bases are at most 396 (Var, 71 reads) and 521 (Bit, 617 reads), so only an index of
     7,796 or more in size could (`locals::tests::indexed_variables`).
2. *The model reads them the same way.* scumm-machine reads a local only in `Machine::local`, from `val_base`'s
   `Expr::Local` and `Expr::Index { kind: Local }`; no hook of the reader reads a local's value to decide anything.
   A unit's slot runs only that unit's code (a start or a chainScript pushes a frame of another unit, with its own
   locals, from argument values the starter's instruction reads). So a run of a slot of u, from any wait point, on any
   path, reads no local outside S(u).
3. *So two lists that agree on S(u) give the same paths.* Take the same wait point and the same facts. Step by step,
   each instruction reads the same facts and the same locals in S(u), so it makes the same forks, the same writes to
   facts and the same writes to locals in S(u) (a write's value is computed from what the instruction reads). A local
   outside S(u) may hold different values in the two runs, but nothing reads it. So the paths take the same branches,
   write the same facts, start the same slots with the same arguments, and leave the same slots at the same places.
   Their records are equal except where the list itself appears: the `args:` number, and the kept slot's own list,
   which the rule restricts to S(u) again, so the next events agree too. Lists that differ only outside S(u) therefore
   give the same records apart from the `args:` number. The switch keeps one list per class (the restriction to S(u))
   and numbers it; the records of every list of the class are that list's records. No plan is lost (a plan that used
   an event of one list of the class can use the class's event) and none is added (every record of the class's event is
   a record of each of its lists).
4. *What the model's bookkeeping compares.* The machine compares whole frames in two places: `merge` joins the locals
   of paths that meet at a join (calls.rs `merge_base`, `join_locals`), and the spin test hashes the frame's locals
   (calls.rs `spins`). There an unread local whose value happens to equal another path's can make two paths merge, or
   a loop count as a spin, one turn earlier than with the local absent. That changes how the model groups paths, not
   what the engine does, and a merge only joins values. Measured: on room 33, the kitchen and rooms 31 and 33 the two
   readings give exactly the same set of records once renumbered, and at k = 1 none is lost (below).
5. *The slot's flags.* An event's key never held the slot's flags, yet its run starts with them: the R flag decides
   whether a freeze stops the slot (calls.rs, the freeze), owned whether an object's slot survives a room change
   (machine.rs `room_change`), and recursive is read by chainScript and by the note at the slot's end. Without the
   switch two lists of one unit and wait point with equal locals and other flags share a key, and the first one met
   runs (reader.py's rule, kept). Lists that differ in unread locals can differ in their flags too (two call sites start
   the unit with other arguments and other flags), so keying on S(u) alone merges them and runs only the first flag
   set: at k = 1 that lost 2 records, which differ from the kept ones only in their next event's R flag. So the switch
   numbers each list with the flags its unit's run can read (above). That keeps every flag set the reading without the
   switch ran where it can matter, and also runs the R and owned variants reader.py's rule dropped where locals were
   equal (the readings below count them). Keying the recursive flag as it is would double room 31's local 202 in rooms
   31 and 33 (78,518 records against 47,052): it is started both with recursive false and with it unknown, which
   nothing tells apart.
6. *Folding.* With the switch, SIG_LIMIT counts a wait point's lists apart from the slot's flags (`builder::flag_free`:
   unit name, read locals, override count, cutscene data). That is a function of the list of all locals, so a wait
   point has no more of these than lists of all locals, and the switch folds lists only where their lists of all
   locals pass SIG_LIMIT too: a list's flag variants share one place. Counting the numbers instead (the first
   version) could fold more than the reading without the switch: at k = 1, 19 wait points have more numbered lists with
   the switch (room 38's local 202 at 1,740: 110 against 105), and one just under 1,024 would have folded lists the
   reading without the switch kept. The rule for which lists fold (the first met are kept) and the rewrite are
   unchanged, and without the switch the limit counts numbers as reader.py does.
7. *What the switch leaves as it was.* Three behaviours of reader.py's rule are the same with the switch and without
   it; the switch neither causes nor fixes them, and fixing them would change the reading without the switch.
   - *Folding still loses paths.* At k = 1, 6 wait points have more than 1,024 lists either way (room 35's local 204
     at 86 and local 205 at 66: 1,032; room 28's local 208 at 21 and 41: 1,026; local 209 at 56 and 33: 1,025): those
     units read the locals that vary. Both readings fold 22 lists. Part I may have more such wait points.
   - *An args-0 event with a known local.* At a wait point only the first args-0 event runs (`events_done` is keyed
     (name, unit, w, 0, Esc)), and it can carry a local known as a number (a slot whose args are unknown set it before
     waiting). Then a slot met there with another value of that local is not run, and no args-0 copy with unknown
     locals is added for replans. Kitchen: room 28's local 220 at 16 wait points (2,292-2,300, 4,077-4,085,
     5,173-5,181, 6,491-6,499) runs only Local[2] = 1, and at 4 of them an args-0 event with Local[2] = 2 is met and
     not run. k = 1: 34 such wait points, with the switch and without.
   - *The args-0 copy's flags.* The replan copy of a wait point's event takes the R and owned flags of the first known
     list met there. At k = 1, 41 wait points have known lists with both R flags and an args-0 event with one, with
     the switch and without (room 33's local 202 at 1 and 21; room 35's local 202 at 51, 52 and 98).
   A replan slot at args 0 is where these matter; fix round 1's F10 replaces args 0 with the dumped slot's list
   (below).

**Measured** (the laptop, 2026-10-26, nice 10, other jobs running: wall times vary with the load).

- *With the switch off* the reader is unchanged: room 33's and the kitchen's readings are golden.py's (the fast tests,
  `kitchen_reading_rust_machine`), and reader-scope's k = 1 reading with `--elsewhere` is the VM's byte for byte
  (packed file sha1 3ceabe50..., golden lines sha1 213e68f3..., 237,264 lines).
- *Room 33* (`room33_live_locals_on`): 878 records either way; 38 lists onto 38 (one loses a local its unit never
  reads, none merge); the golden records renumbered are exactly the switch's.
- *The kitchen* (`kitchen_live_locals_on`): 72,256 records against 72,126; 5,298 lists onto 5,221; event runs in the
  last pass 7,098 against 6,990; the golden records renumbered are 72,124 distinct records, exactly the switch's 72,126
  records (72,124 distinct). Its task (start S1#7, taskcore's default reductions, lama-first): 17,823 operators
  against 17,648, 12,029 reduced actions against 11,987, and the same 10-step plan, step for step.
- *Rooms 31 and 33* (`reader-scope --rooms 31,33 --elsewhere`): the same either way: 57,866 records, 906 event runs
  (861 events, 45 Esc) in 32 rounds; room 31's local 202 315 runs, 47,052 records, 105 wait points, at most 3 lists
  each (it reads Local[0], the saved VAR_VERB_SCRIPT, whose 3 values give the 3 lists). The switch changed 828 lists,
  merged none; the records renumbered are equal.
- *k = 1 with `--elsewhere`*: 237,262 records against 237,465; event runs of the last pass 18,225 against 18,149; 1,027
  rounds either way; 9,710 lists onto 9,580; no record lost: the 237,143 distinct records renumbered are all in the
  switch's reading. Its 320 more come from flag sets reader.py's rule dropped: 215 from 10 lists that carried two flag
  sets without the switch (room 38's local 202, room 28's local 200, object 442), 105 from those variants' later wait
  points (`live_locals_check.py`). Read on 3 threads: 291.0 s and 853 CPU s at load 6.7 without, 160.7 s and 570 CPU s
  at load 3.3 with; peak RSS 1,026 and 1,072 MB. Seed 1 through replay_part1.py on each: the start checks pass (0 facts
  differ, taskcore's `input` and `needed` checks ok), taskcore reduces the task to 0 operators, lama-first proves no
  plan, verify from boot 0 differences: the VM's result, either way.
- *k = 2 with `--elsewhere`* (3 threads each, one after the other): 532,166 records (the committed reading's count)
  against 519,755; runs in both passes 62,526 against 58,788, event runs of the last pass 26,261 against 24,392, its
  event records 356,557 against 344,146; 1,027 rounds either way; the packed file 1.048 GB against 0.964 GB. Wall
  1,450 s and 3,465 CPU s at load 12 without, 1,404 s and 3,107 CPU s at load 17 with; peak RSS 2,935 and 2,836 MB.
  The diagnosis's estimate with its text rule was 26,434 to 23,693 event runs and 356,557 to 342,516 event records.
- *Part I* (`reader-scope --part1 --elsewhere --live-locals`, pass 1, 3 threads): the inputs are the VM's, as they
  must be (the switch changes no input): 34,124 runs, 1,544,657 records (2,581 s and 1,962 s here in two attempts).
  Event round 1 has 10,096 runs from 17,151 events, against the VM's 19,847 runs from 30,545 events (the diagnosis's
  count with its text rule was 10,015). Round 1's records were not measured: both attempts stopped at the 4.8 GB RSS
  guard at the same point, 193,710 records into round 1 (1,738,367 in the pass; peak VmHWM 4,952 and 5,022 MB), the
  second with `--max-waiting 2000` instead of 20,000, so one job's own run, not the records waiting, takes the memory.
- *After the review's fixes* (the IR notes, the text codes' bytes, `list_number`, the flag-free SIG_LIMIT, `reader-port
  read --packed`; 2026-10-26, load 1 to 5). No MI1 unit's S(u) changes. Without the switch: the fast tests, room 33
  and the kitchen (`kitchen_reading_rust_machine`) are golden.py's; `reader-port read room33 --packed` and `read
  kitchen --packed` give 0 differences (880 and 72,258 lines; the kitchen's packed file 71,729 actions, 527 folded); k =
  1 with `--elsewhere` is the VM's (packed 3ceabe50..., golden lines 213e68f3..., 237,264 lines; 137 s on 3 threads).
  With the switch, the readings are the ones before the fixes: room 33 and the kitchen give the same records and lists
  (`room33_live_locals_on`, `kitchen_live_locals_on`), k = 1 and k = 2 the same packed files byte for byte (k = 1:
  packed 590c6acf..., golden lines 8a1e143e..., 237,467 lines, 145 s, 1,079 MB; k = 2: packed 39200a67..., 519,755
  records, 582 s, 2,894 MB). The harness tests: 88 passed, 2 slow ones deselected.
- *Projection for Part I* (the diagnosis's per-run record counts on the measured runs): pass 1 about 3.0 M records
  (2.9 to 3.3 M): the inputs' 1.54 M, round 1 about 0.84 M (the sentence script's 7,732 runs at 103.5 records each, the
  2,364 others about 0.04 M), room 31's local 202 about 0.19 M, the other rounds 0.4 to 0.6 M. Both passes about 20 to
  28 minutes on the VM's 32 vCPUs, a packed file of about 10 to 11 GB. Keying the R and owned flags adds little: 0.1 %
  of k = 1's records, and k = 2 with it is 2.3 % smaller than without the switch. One round-1 job needs more than
  4.8 GB on its own here; on the VM (n2-highmem-32) that is no limit, but its unexplained 61 GB peak in round 3 may come
  back.

**What the harness must do** (replay.py, packed_reading.py). A replan starts each live slot from the engine's state.
Without F10's dumped locals nothing compares a slot's locals: `task_start` gives every live slot args 0, and
`replan_start` copies the plan's `args:` number when the slot is where the plan put it, else 0. Under the switch the
plan's number names the list restricted to S(u), with the slot's flags, which is all a run of the slot can depend on, so
this rule needs no change. Anything that matches a live slot's locals to a list compares S(u) only: `Points.read_locals`
gives the read set of a slot (from reading.json's `live_locals`): S(u) under the switch; None (every local) for a reading
made without it, for a unit whose reads are not known exactly, or for a slot the reading does not list. The read set is
per unit, so a local script is looked up with the room the engine is in (`local:202@31`). The run's record says whether
its reading was made with the switch (`task.live_locals`).

**F10 with this switch** (fix round 1's slots' dumped locals, merged with it on fr1-main; "Fix round 1, F10" below).
Under the switch a list's number covers the read locals and the slot's flags, so a dumped slot is numbered and matched
on both, in Rust and in Python alike:

- The bridge dumps the flags: with `SPEEDRUN_DUMP_LOCALS` each live slot also has `resistant` (ScriptSlot's
  freezeResistant) and `recursive` (script.h:91). The owned flag is the slot's `where`, in the dump already: an object's
  slot started from the inventory (WIO_INVENTORY) survives a room change (killScriptsAndResources,
  script.cpp:1083-1106), as the model's owned flag says (scumm-machine `room_slot`). replay.py `dumped_slots` gives
  them as the slot's `flags` (`SlotFlags` in Rust).
- `dumped_events` numbers a dumped slot's list with `Builder::list_number`, as `record` does: the locals restricted to
  S(u), the number over them and the dump's flags (recursive as "true or not"). A dumped slot without flags stops the
  reading (a panic that names the slot): its number cannot be known. Without the switch the flags stay the start's
  (`start_events`' rule: the R flag from the unit's starts, recursive unknown, owned from the object's owner), as F10
  had them, so F10's readings without the switch are unchanged.
- `list_matches` (task.rs, replay.py) under the switch skips every dumped local outside S(u), keeps "0 where the list
  lacks it" for a local in S(u), keeps the override count and cutscene data, and needs the list's flags to be the
  dump's as the number takes them: resistant and owned equal, recursive true for a recursive slot and unknown (None)
  for the others. A list flag the model holds as an unknown value matches no dump. `live_match` makes what a match
  compares, and stops on a dumped slot without flags (Rust: a panic; Python: a ValueError). A slot no list matches
  takes args 0 and is read again (`--reread`), as without the switch.
- The lists carry the flags under the switch: `BuiltList` has the event's three flags, packed_reading.py's list lines
  get `"flags"` when the reading's head has `live_locals`, and reader-scope's `slot_events` too. A reading made
  without the switch writes the lists as before.
- `match_list`'s choice (the list with the most locals known as numbers) is unchanged.

Measured (fr1-main, 2026-10-26): the bridge with the flags changes nothing but the dump (`bridge_locals.py check`,
results/bridge-locals-check-flags.log: 0 frames differ on room 33, the bar door and the kitchen's inputs, 20,198
frames). Rooms 33 and 28 from seed 1 read with `--live-locals` (66,030 records, 65,516 actions, 5,100 lists, against
66,106, 65,592 and 5,166 without), then run by replay_part1.py with `--slot-locals --reread` on the build with the
flags: both runs are the committed F10 runs (results/replay-part1-bar-door-rust-seed1-*locals.json) step for step,
frame for frame and input for input, up to the lists' numbers. The full run reaches the goal at frame 3129 with no
replan. The control's one replan (frame 6376) starts with 1 live slot at args 0 (global 159); it is read again
from its dumped locals and flags, and then 0 slots are at args 0 (local 202 and global 23 match a built list). Both
verify from boot with 0 differences (results/replay-part1-bar-door-ll.log).

## The path limit: split until no part passes it, as a switch

**The finding** (Part I attempt 2 on the VM, results/vm-part1-2-*.json, and a diagnosis on the laptop). The machine
ends a path where a fork would pass PATH_LIMIT (1,024 paths): neither side runs, the path ends "limit"
(scumm-machine `machine.rs`, `drive`). reader.py's `Builder.run` then splits such a run on its first fork on a read,
but not when the run forked more on non-facts than on facts, and not past SPLIT_DEPTH (2) levels or SPLIT_BUDGET (4)
parts; DROP_ENDS drops the paths that end "limit". So every state of a run that only a cut path covered has no
record. In the bar with room 35 (rooms 33, 28 and 35, seed 1, `--elsewhere --live-locals`), room 35 makes the classes
of the kitchen door and var:196 facts, and the event that walks through the door (`event(script:2@736...)`, the
sentence script's wait after the walk) makes 1,024 paths in pass 2 and cuts 532 (506 forks on read, 1,353 on
non-facts: not split). The start's case (classes 15 and 16 of the door at 0) is among the cut ones: no record writes
room 28 from the start, and the delete relaxation reaches rooms 33 and 35 only, against 33, 28 and 38 with room 38
in place of 35. At k = 1, 30,065 paths are cut and 101 runs are refused by the non-fact rule; in the Part I scope
every walk through a door of room 33 is cut, and from the start the relaxation reaches rooms 33 and 38 only.

**The rule** (`Builder::run_split`, switch `split_limit`; reader-scope `--split-limit`, reader-port
`--switches split_limit`). A run past the path limit is run again once per value of its first fork on a read (the
values its precondition leaves), that fact pinned; a part that passes the limit is split again, with no cap on depth
or parts and whatever its forks on non-facts. A part that passes the limit with no fact left to split on stops the
reading (a panic naming the job), and so does a path the step limit ended (STEP_LIMIT, 5,000 steps; with no path
cut by the path limit, every "limit" end is the step limit's, so the machine needs no change). Why it is exact: in
any state the engine runs one path of the job; the parts' preconditions divide the job's states by the pinned fact's
values, so they do not overlap and together cover them, and each part runs the same code with that fact known. So
the parts' records cover every state the job can start in. Each record of a part carries the pinned value in its
precondition, also when its path never read that fact.

What it changes besides: with the switch the machine half hands over no path of a run the limit cut
(`Reader::run`), since the Builder splits it and reads none of them; the counters gain "split_limit: runs split at
depth N" and "split_limit: parts run"; "paths ended by the path limit" stays 0.

**With the switch off** the reading is reader.py's: the Builder and the machine half take the old branches, and the
goldens are unchanged (below).

Measured (stuck, 2026-10-26; the laptop at load 10 to 20 from other jobs; results/split-limit-checks.log):

- **Off, unchanged.** The reader's fast tests (20 unit, 21 oracle with room 33's goldens, 6 fix_round1) and the
  kitchen's nine oracle tests (`--ignored`: the reading against golden.py's, over the captured runs and with the Rust
  machine half, on threads, the SAS files against write_sas) pass. k = 2 with `--elsewhere --live-locals` gives the
  committed packed file (sha1 39200a67). The machine crate is unchanged.
- **Room 33 with the limit at 32** (tests/split_limit.rs): the cut reading has 806 records and reaches 363 (fact,
  value) pairs ignoring deletes; the split one 900 records, 18 splits, and 364 pairs, the uncut reading's (878
  records). At 24 and below a part passes the limit on non-fact forks alone and the reading stops.
- **The kitchen** (s17): 2 runs passed the limit (318 paths cut), both `sentence(8,574)`; with the switch each is
  split once into 5 parts (on var:102), 0 cut. 72,220 records against 72,256, and only those two runs' records
  change, 108 of each into 90: with var:102 pinned, the paths the full run kept apart on a non-fact test
  (`Var[103] == Local[2]`) merge, and the part writes var:102 as a guess of both values. The delete relaxation
  applies 27,910 actions against 27,912 and reaches the same rooms; taskcore keeps 12,027 actions (12,029), and
  lama-first gives the same 10 steps. With due_first, value_sets, draw_clears, actor_rooms, `--elsewhere` and `--live-locals` too: 71,242
  records against 71,278, 11,975 actions against 11,977, the same 10 steps.
- **Rooms 33, 28 and 35 from seed 1** (`--elsewhere --live-locals`): 269,868 actions (208,950 without the switch),
  packed sha1 28abab45; 921 splits in the last pass, 8 levels deep; 0 paths cut. Room 28 and state:316 = 1
  are reached ignoring deletes. 1,840 CPU s and 2.2 GB on 6 threads, against 744 CPU s and 0.9 GB without.
- **k = 1** (`--elsewhere --live-locals`, goal state:316 = 1): 299,617 records (237,465 without), 973 splits, 0 cut.
  taskcore's (d) no longer proves the goal unreachable: reductions a-d keep 31,939 actions (16,052 without, and
  unsolvable); all reductions keep 24,059 (67,234 operators), and lama-first finds 6 steps: open the bar door, the
  sentence script's two waits, walk through the kitchen door, the cook's timer at 4 and 33. 2,164 CPU s and 1.8 GB
  on 5 threads, against 570 CPU s and 1.1 GB without.
- **k = 2** (`--elsewhere --live-locals`, 6 threads, the laptop alone): not measured. Pass 1's inputs took 2,143 s for
  253,933 records (off: 227 s, 163,981 records). The reading was stopped at 3,517 s wall and 17,733 CPU s, inside pass
  1's event round 1 (off: 363 s). Pass 2 takes about what pass 1 takes, so the two passes project past 8,000 s.

## Python behaviours kept on purpose, and the differences

1. **The scope-1 finding, kept with the switch off.** `Builder.positions` turns a write of
   `at:<unit>:W` = 1 into `pos:<unit>` := W without checking W against the domain of `pos:<unit>`
   (results/task-size-scaling.log, finding 4: at k = 1, `pos:local:200 := 10` is outside its domain, and
   write_sas is the first place that sees it). `Builder::positions` does the same. The fix is the ELSEWHERE
   switch (the section above).
2. `event_pins` lists a unit's other wait points from the Reader's oracle: the scope's universe, then the pins of
   the last run that the universe lacks (`SymOracle.keys`). In Python that last run is the parent process's
   last one (workers run most jobs), here the Builder's last. An event's own wait point is always in the universe,
   so both give the universe's keys: equal on both readings.
3. **Python's fold depends on the hash seed: the second fix after the port.** build_task's fold key is `str()` of
   the effects, and a guess's frozenset with `above` or `below` in it prints in an order that follows the string
   hash. So the same kitchen reading folds 527 records under PYTHONHASHSEED 0 to 6 and 491 under seed 7 (the
   verifier's count; export_records.py's fold has the same key). Here the key sorts every set, so records with
   equal sets always fold: 527 on the kitchen, Python's count under seed 0. The oracle runs under seed 0. Fix after
   the port: key the fold on sorted sets in reader.py too.
4. A record's precondition is kept sorted by key. Python's dict order (the machine's read order) is not in the
   goldens and no output rests on it (the SAS files are equal). A record's effects keep Python's order (the SAS
   operators follow it); for a record read from a golden file it is rebuilt by `Builder.record`'s rule
   (`record::effect_order`: the path's writes by key, then `args:`, `pos:`, `escpos:`).
5. A path's Esc events (`esc:<unit>:W` = 1) are taken in key order (`PathOut.writes` is sorted, as the capture
   is); Python takes them in write order. The machine's beginOverride clears every `esc:` key before it arms one
   (model.py `_op_beginOverride`: `self.clear(p, "esc:", ...)`), so a path ends with at most one armed and the two
   orders are equal. The Rust machine's beginOverride does the same (it clears every `esc:` key first).
6. reader.py's `ego_values` raises `Fork` when VAR_EGO is a fact with several values; `Builder::ego_values`
   panics there. It keeps VAR_EGO's values for one pass (reader.py reads them on every call): a new pass and
   `extend_domains` clear them, so a domain a pass widens is read again. `effect` of a collapsed value outside its
   domain writes `("set", None)` in Python; here it panics. Neither happens in either reading.
7. `RecordStream` writes records as the Builder makes them and drops a pass that starts over. A list of locals
   SIG_LIMIT folds into 0 after its records were written changes those records (`_read_once`'s rewrite) and can
   make some identical, which build_task folds. So each written record keeps its fold key in two parts (a 16-byte
   hash with the `args:` values marked, and those values: `task::FoldParts`). `finish` works out every key after
   the rewrite; when a record folds or a value changes, it writes the body again in one streaming pass (later
   identical records dropped, the folded lists' values 0, the tables copied, the footer moved). It counts the
   steps that need room only for the records it keeps, as build_task does.
8. The packed files' bytes differ from export_records.py's (fact and string ids are given on first sight, and the
   precondition order differs, item 4); taskcore's SAS file is the same.
9. `start_events` maps a slot named `entry` or `exit` to the room's unit; any other name is counted as outside the
   scope, as Python's `(name, room)` is.
10. The reading's header (golden.py's file) takes five info fields from the golden file it is compared with:
    `name`, `start`, `workers`, `sources`, `domains_goals`. They describe the run (the caller's inputs and the
    Python sources) and are not compared; every counter in the header is.
11. **Found with the machine half: the clear mark covers entry and exit code only.** reader.py's `clear` writes
    one mark (`at:<unit>:*` = 0) for a prefix with two colons. `at:entry:` has two, but `at:script:5:`,
    `at:local:200:` and `at:obj:12:` have three, so for every global, local and object slot it runs model.py's
    clear (lazily): a 0 for each wait point the oracle lists. Its docstring says one mark per unit. Kept.
    `Builder.positions` reads a mark and a 0 alike (NOWHERE), so the records are the same; the cost is one write per
    wait point. Fix after the port: test the prefix's form (`at:<unit>:`, `esc:<unit>:`), not its colons.
12. **Found with the machine half: every dialog line forks on VAR_USERPUT.** model.py's `_eng_input` reads
    VAR_VERB_SCRIPT, VAR_USERPUT and the clicked verb for `Path.input_drift` through `Reader.read`. The dialog
    inputs pin VAR_USERPUT to its values above 0 (three in the kitchen: 1, 2 and ABOVE), so that read forks: the
    kitchen's 66 dialog runs give 549 records, three copies of 183 that differ only in their VAR_USERPUT
    precondition (66 of the 38,454 forks on read). Kept. Fix after the port: read the drift lazily, or not at all
    (no output reads it).
13. The first path of `run_event` waits at a join before its first step: its call's head is model.py's ROUND
    ("runAllScripts"), so `_join` gives it a round's signature, and the path waits alone and goes on (a merge of
    one). Nothing changes; kept. It is why `_go` makes the first path a SymPath before `_drive` (`first_path`).
14. `Path.input_drift` is not kept: no output reads it, and the machine port's merge refuses dialog's drift (row 4).
15. `Reader._join` walks every write for a step value; the port looks at the three counters STEP_OPS step
    (`cutscene`, VAR_USERPUT, VAR_CURSORSTATE): a read of a step anywhere else resolves it or forks, and a merge
    joins only paths with the same steps, so no other key holds one. The readings are equal.

## Fix round 1

Fix round 1 (fix-round1.md, D8) is made in Rust only (D7). Each fix is a reader switch (`builder::Switches`, set
on the Builder before `read_all`), off by default; with every switch off the reading is reader.py's, and every
comparison above still finds 0 differences. A switch's reading has its own golden in `out/golden/reader-fr1/`, made by
`reader-port read NAME --switches S --out FILE` and checked by `reader-port read NAME --switches S --golden FILE` and
`tests/fix_round1.rs` (room 33 fast; the kitchen, one thread and three, ignored).

**With main's two switches** (fr1-main). ELSEWHERE and `--live-locals` are the FactTable's (`elsewhere`,
`live_locals`), not `Switches`, and every switch combines with every other. `reader-port read` takes `--elsewhere` and
`--live-locals` beside `--switches`; reader-scope takes `--elsewhere`, `--live-locals` and F10's `--slot-locals`, not
`--switches`. `task_start` applies the ELSEWHERE start rule first, then gives the `due:` facts from that start (a
slot at ELSEWHERE is at no due wait). A slot at ELSEWHERE at a replan takes args 0 and is not read again from its
locals (no list is built at a place none of the points). `reader-port read --packed` streams the records as made,
before due_first writes `due:` on the kept records, so it refuses due_first: its task goes through `--task-out`.
With due_first and `--elsewhere`, ELSEWHERE is a value of every `pos:` domain, so the `due:` writes' cases list it
among a slot's places where it is not due, as NOWHERE; no precondition holds it.

Measured on the kitchen (3 threads, fr1-main, 2026-10-26), with the four reader switches (A), with them and
`--elsewhere` (C), and with them, `--elsewhere` and `--live-locals` (B). A gives 71,408 records, the fr1 branch's.
C gives the same 71,408 records once ELSEWHERE is taken out of the 16,012 value sets of `due:` cases that hold it,
and the same task (start s17: 12,019 reduced actions, 17,877 operators) and 10-step plan. B against C, renumbered
(`live_locals_check.py`): 5,299 lists onto 5,222, 71,408 records onto 71,278 (71,274 distinct either way), 0 records
on one side only. B's task has 11,977 reduced actions and 17,702 operators, and lama-first gives the same 10 steps
up to the lists' numbers. The due writes match a direct evaluation on 20.5 to 20.8 million states in each.

**`draw_clears` (F5) and `actor_rooms` (F9)** are scumm-machine's switches of the same names, and they change the
reading's facts too, so the Builder's scope and domains are made with them (`oracle::builder_with`:
`Switches::scope()` into `Scope::of_with` and `domains::build_with`; the Reader's machine takes them from the scope in
`sync`). Their rules, readings and tests are in `../fixes/F5-draw-clears.md` and `../fixes/F9-actor-rooms.md`; their
tests pin the reading against golden.py's files, not a golden of their own.

**`due_first` (F2): a due event goes first.** `due.rs` (module docs) and scumm-ir's `due.rs` (the wait classes).
After the reading, `due:<unit>` (binary) is 1 while the unit's slot is live, unfrozen and at a pure poll whose test
holds or a plain breakHere, at a wait an input meets; every record that writes the unit's position, slot, freeze count,
room (two units of one name) or a test fact writes it by `Eff::Cases` (conditions on the facts before the action);
every input (sentence, walk, dialog line) that meets such a wait's next part needs `due:<unit>` = 0; an input a time
wait's work could change gets one replay check. Kitchen: 13 due facts over 22 waits, 30,553 inputs with a `due:` 0,
30,073 with a time check; `due::verify` checks the writes on 20.4 million states. `reader-port due-engine` checks the
rule's premise against run B's frames (`due_engine.py`).

**`value_sets` (F12): forks by value sets, the mouse as ranges.** The machine half turns on scumm-machine's switch of
the same name (its PORT-NOTES.md: ranges, findObject and actorFromPos as value sets, tests decided on them, the fork
that narrows), with findObject over the scope's objects. On the reader's side (`machine.rs`):

- A read of a fact with several values left, made by a jump whose condition is a comparison of that fact with a
  constant (or a zero test of it), forks into the value sets the comparison separates (`compare_sets`, `ForkOn`'s
  sets, `on_fork`), not into one path per value; the path's precondition holds the set. A read where only one set is
  left is that set's value (the test decides; no fork). Every other read forks per value, as reader.py does, and is
  counted by where it is ("F12: fact reads forked per value: ...").
- A mouse variable (a non-fact) is its range. A range is no memo identity (`memo_ident`) and keeps its bounds through
  the naming hooks (`val`, `arith_at`).
- Not done: a fact copied into a variable or local and then only compared (`VAR_RESULT = getObjectOwner(o)`, then a
  comparison) still forks per value: in the kitchen 3,077 such reads at an assignment (most of them `Var++` or a
  number, some owner and state reads). The mouse is not narrowed in the reader (it is read, not written, there): a
  second test of it forks again.
- Kitchen (one thread; reader.py's reading in brackets): 71,336 records (72,256); forks on non-fact tests 154,944
  (174,727); forks on read 41,683 (38,454): 25,530 into value sets, 4,154 reads decided on their set with no fork,
  and per value 12,737 in a jump's other test (indexed bits and classOfIs: binary facts, where sets gain nothing),
  3,077 at an assignment, 339 in an engine step; var forks 8,390 (4,354), because a comparison chain now forks once
  per comparison; unknown-target startScript writes 264 (1,134), startObject 266 (1,136). Reading 102.7 CPU s (105.8).
  Most of the gain is the fork into value sets: without it (the machine's ranges and sets alone, a measurement with
  that part turned off) the kitchen gives 72,223 records and 170,452 forks on non-fact tests. Room 33: 819 records
  (878), forks on non-fact tests 2,329 (2,930).

## Fix round 1, F10: the slots' dumped locals

The first fix made on the port (fix-round1.md F10, decision D8). Off by default: with `Builder.slot_locals` empty the
reading is reader.py's: the twelve fast tests and the five kitchen tests the base ran (records written back, the
reading over the captured runs and with the Rust machine half, every captured run, the six SAS files) pass, in 586 s
for the kitchen's. `room33_slot_locals` is the thirteenth fast test.

- **The bridge** (patches/0001, `SPEEDRUN_DUMP_LOCALS`, EngineConfig.dump_locals): the state dump lists each live
  slot's 25 locals and its two flags (`resistant`: freezeResistant, `recursive`), and each cutscene level's
  cutSceneData. `bridge_locals.py check` shows it changes nothing else (results/bridge-locals-check.log for the locals,
  results/bridge-locals-check-flags.log with the flags).
- **`Builder.slot_locals`** (`SlotLocals`: unit name, room, wait point, armed Esc point, locals, cutscene count and
  data, owned, and the dump's flags when it has them): `read_once` adds `dumped_events()` to the start events. Each is
  the slot's event with its locals known, numbered as `record` numbers a list (`Builder::list_number`; the zero locals
  left out: a missing local reads 0 with known args), with its Esc event where the override is armed; the number goes
  into the `args:` domain before the event is pinned. `slot_events` keeps them (`task::slot_lists`). Under
  `--live-locals` the event's flags are the dump's ("F10 with this switch", in "Live locals" above).
- **`task::replan_start_locals`**: `task_start`, then each live slot's `args:<unit>` is the number of the built list
  its locals match at its wait point (`match_list`: every dumped local equals the list's number, 0 where the list lacks
  it, or the list's local is unknown and its value set, if any, holds it; override count and cutscene data equal; the
  list with the most numbers wins). A slot no list matches gets 0 and is returned for a reading again; a slot the dump
  gives no locals keeps `replan_start`'s rule. `built_lists` takes the lists from the records' next events whose
  `args:` write the record kept (a folded list is none).
- **reader-scope**: `--slot-locals FILE`, `--start-seed N`, `--domain-goals part1|none` (none: golden.read_scope's
  domains) and `--goal`; the report lists the dumped slots' lists (`slot_events`).
- **What the reading changes.** The new number widens the `args:<unit>` domain, so the unit's args-0 records now
  hold `args:<unit> = 0` in their precondition and their `args:<unit> := 0` write is no change and goes; a record
  whose only write that was is dropped (room 33 with its three start slots dumped: 24 such records; `room33_slot_locals`
  checks every other record is kept).
- The harness side is Python (replay.py `replan_start_locals`, the same rule; replay_part1.py `--slot-locals
  --reread`, which runs reader-scope and `reading_from_golden`). On rooms 33 and 28 from seed 1, reader-scope's reading
  equals the Python reader's action for action (66,106 records, 65,592 actions) in 30 s on 3 threads (Python: 2,434 s).

## Files

- `src/`: the modules above; `src/bin/reader_port.rs` (`reader-port static | records NAME | replay NAME |
  runs NAME | dump-runs NAME --runs N,... | read NAME [--switches S] [--live-locals] [--elsewhere] | task --out DIR |
  stream --out FILE | bench-task --out DIR | bench | due-engine FRAMES`).
- `tests/oracle.rs`: the comparisons as tests; `tests/fix_round1.rs`: fix round 1's switches against their goldens;
  `tests/split_limit.rs`: the switch split_limit (a fake machine for the rule; room 33 with the limit lowered; rooms 33,
  28 and 35 from seed 1, ignored).
- `../../reader_port_oracle.py`: the Python oracles (new file; no existing .py file changed).
- `../../reader_run_dump.py`: chosen runs made again in Python and written whole (new file).
- `../machine/src/machine/hooks.rs`: the hook points (Machine changes, above).
