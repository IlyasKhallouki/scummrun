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
compare --runs row0,row1,row2 --traces`).

| file | change |
|---|---|
| `machine/src/machine/hooks.rs` (new) | the `Hooks` trait (one default method per hooked method: its `_base`), the dispatchers (`Machine::advance`, `read`, `oget`, `okeys`, `val`, `test`, `getter`, `owned_by`, `find_inventory`, `clear`, `arith_at`, `jump`, `wait`, `op`, `join`, `merge`, `eng`, `eng_script`, `finish`, `count`, `set_freeze`, `freeze`, `unfreeze`), and public wrappers for what a hook calls (`vals`, `vals_ref`, `h_write`, `h_halt`, `h_note`, `h_done`, `h_resource`, `h_push_frame`, `h_stop_script`, `h_live_slots`, `h_cid`, `h_drive`, `h_waits`) |
| `machine/src/machine.rs` | `mod hooks` and `pub use hooks::Hooks`; `Machine.hooks` and `Machine.ext` (two fields, set to None in `Machine::new`); renamed to `_base` and made public, bodies unchanged: `oget`, `advance`, `read`, `val`, `test`, `getter`, `owned_by`, `clear`, `arith_at`, `jump`, `wait`, `op`; `prefixed` lists the oracle's keys through `self.okeys` (one line) |
| `machine/src/machine/calls.rs` | renamed to `_base` and made public, bodies unchanged: `join`, `merge`, `eng`, `eng_script`, `finish`, `count`, `set_freeze`, `freeze`, `unfreeze`, `find_inventory`; `eng_resume` lists the oracle's keys through `self.okeys` (one line) |
| `machine/src/path.rs` | `Path.ext` (a hook's path fields, the `PathExt` trait, `Clone` for its box), `Sig.ext` (a hook's join signature extension; empty in `Sig::new` and `Sig::round`), `Store::remove` |
| `machine/src/value.rs` | `Unknown.class` (0 in `Vals::mk`, `mk_set`, `one_of`) |
| `ir/src/analysis.rs` | `Reason::Other(&'static str)` and its arm in `Reason::as_str` |

A merge with main (rows 3-5) touches the renamed lines and the struct literals of `Unknown` and `Sig`: a new
`Unknown { .. }` needs `class: 0`, a new `Sig { .. }` `ext: Box::default()`, and a new caller of a renamed method
calls the dispatcher (its old name) as before. Once row 4 ports model.py's `("input", ...)` step, `run_click_verb`
can use it in place of its marked note (`machine.rs` `eng_input` is model.py's `_eng_input` with its switch off).

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
| model.Machine rows 0-2 after the machine changes (`machine-run`, `golden.py compare --traces`, machine/PORT-NOTES.md) | 717 files | 0 |
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

## Python behaviours kept on purpose, and the differences

1. **The scope-1 finding, kept: the first fix after the port.** `Builder.positions` turns a write of
   `at:<unit>:W` = 1 into `pos:<unit>` := W without checking W against the domain of `pos:<unit>`
   (results/task-size-scaling.log, finding 4: at k = 1, `pos:local:200 := 10` is outside its domain, and
   write_sas is the first place that sees it). `Builder::positions` does the same. Fix after the port: check the
   value (or give `pos:` the stop points of every unit with that name) and let `extend_domains` widen it.
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

## Files

- `src/`: the modules above; `src/bin/reader_port.rs` (`reader-port static | records NAME | replay NAME |
  runs NAME | dump-runs NAME --runs N,... | read NAME | task --out DIR | stream --out FILE | bench-task --out DIR |
  bench`).
- `tests/oracle.rs`: the comparisons as tests.
- `../../reader_port_oracle.py`: the Python oracles (new file; no existing .py file changed).
- `../../reader_run_dump.py`: chosen runs made again in Python and written whole (new file).
- `../machine/src/machine/hooks.rs`: the hook points (Machine changes, above).
