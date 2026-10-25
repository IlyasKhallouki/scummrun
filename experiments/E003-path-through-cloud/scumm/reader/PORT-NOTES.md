# scumm-reader: port notes

The Rust port of the half of reader.py that does not depend on the Machine, and of domains.py (decisions.md D7,
layer 3). It builds on `scumm-ir` and does not depend on `scumm-machine`. The Python is the oracle: every part
below matches it with 0 differences. Read this file first.

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
| `replay.rs` | a `Machine` that plays back the Python reading's machine runs (the oracle until the Rust machine lands) |
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

What the machine half must still port (reader.py's `Reader`, `SymOracle`, `SymPath`): the fork on read (`read`,
`_advance`, `Fork`), the oracle (`SymOracle.get`, `keys`), the lazy reads (`LAZY_METHODS`, `_lazily`), the step
counters (`STEP_OPS`, `_make_step`; `_stepped` is `Builder::stepped`), one frame's non-fact memory (`test`,
`_jump`, `_forget`, `_memo_ident`, `getter`, `_arith_at`, `val`), the join and merge on assumptions (`_join`,
`_merge`), `clear` with its mark, saved verbs (`_op_saveVerbs`, `_op_restoreVerbs`), freezes (`_freeze`,
`_unfreeze`, `_count`), `_owned_by`, `_finish` (the kept slots), `_eng_scene` (leaving the scope), `_eng_script`
(the inventory hook: `scope::hook_writes` gives its writes), `_eng_resume_at`, `_wait`, `_go`, `_drive`, and the
three entry points. reader.py's `check_machine` checks that model.Machine still has what these wrap; in Rust the
`Machine` trait and the compiler do that once the half exists. Until then `tests/oracle.rs::check_machine_lists`
pins reader.py's lists (LAZY_METHODS, CALLED, PATH_FIELDS, FRAME_FIELDS, DROP_ENDS), so a change there shows.

The Builder keeps no machine state. `Builder::last_pins` stands for the Reader's last oracle, whose pins
`event_pins` lists slot keys from (below).

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

`cargo test --release -p scumm-reader` runs the fast ones (1.5 s after the build). `-- --ignored` runs the kitchen:
records written back (25 s), the reading against the golden (100 s), the task's six SAS files and the streaming
writer against write_sas (about 5 minutes; needs `python-sas.json` from `reader_port_oracle.py sas`).

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

The reading itself is the machine's work and still Python's: 817 s with 4 workers for the kitchen. The Rust
Builder's replay of it takes 95 s, almost all of it parsing the 1 GB of captured runs.

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
   orders are equal. If the Rust machine can ever end a path with two, `PathOut.writes` must keep the write order.
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

## Files

- `src/`: the modules above; `src/bin/reader_port.rs` (`reader-port static | records NAME | replay NAME |
  task --out DIR | stream --out FILE | bench-task --out DIR | bench`).
- `tests/oracle.rs`: the comparisons as tests.
- `../../reader_port_oracle.py`: the Python oracles (new file; no existing .py file changed).
