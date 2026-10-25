# scumm-machine: port notes

The Rust port of model.py's `Machine` (decisions.md D7, layer 2). It builds on `scumm-ir` (../README.md).
The Python is the oracle. A row is ported when `golden.py compare` finds 0 differences on its run.
Read this file first. Update it at the end of each stage.

Frozen Python sources (sha1 prefixes): model.py cc630446bdbe, ir.py c6ccf01059f1, facts.py c06fc3489018,
actions.py ddbd1aaa2207, check.py fc1c3a935c4e, reader.py 94d495853f07. `candidate.py` stops when one differs.

## State per row

| row | features | state | compare |
|---|---|---|---|
| 0 | own_entry | done | 239 of 239 moves equal, traces too; plus 11,941 extra cases equal (below) |
| 1 | + dispatch, calls | done | 239 of 239 moves equal, traces too; plus 21,770 extra cases equal (below) |
| 2 | + room_change | done | 239 of 239 moves equal, traces too; plus 56,255 extra cases equal (below) |
| 3 | + settle | next | |
| 4 | + dialog (and switches const_polls, input_after_turns) | | |
| 5 | + esc (and switch esc_key_at_pre) | | |

`Features::parse` refuses a feature that is not in `machine::PORTED`. Add a feature there when its row
passes. Code of later features is marked `unreachable!("... is not ported")`. Each mark is a place
where model.py branches on `p.settle`, `p.round`, `p.timed` or a later feature's step: 24 marks (20 of
settle, dialog's merges, and the three actions of rows 3-5 in `harness::run_model`), plus an `assert!` in
endCutscene for a settle turn.

## Run the comparison

From `scumm/`:

    cargo build --release
    for r in row0 row1 row2; do ./target/release/machine-run $r --out DIR; done   # 8 s, 9 s, 18 s
    cd .. && ../../.venv/bin/python golden.py compare DIR --runs row0,row1,row2 --traces

(`machine-run` takes one run; run it once per run into the same DIR, then compare them together. Most of the
time is Python's prep and check. Row 2's three heavy moves, S2#21, S2#29 and S1#83, take 2.6, 2.6 and 0.9 s
in Rust traced on four threads, which slow each other down; untraced on one thread 0.5, 0.5 and 0.2 s.)

`machine-run RUN --out DIR [--only S1#4,...] [--jobs 4] [--no-trace] [--no-prep] [--no-check] [--repeat N]`:

1. `candidate.py prep` (Python, nice 10) writes `DIR/machine/RUN/jobs.json`: for each counted move, its
   label, its row `i`, golden.py's move line, and the action as check.py's `run_model` reads it. Also the
   run's features, switches, key (golden.py `_key`) and fact families (`check.held_families`).
2. The Rust machine runs each move with an action. `harness::run_model` is check.py's `run_model`
   dispatch (row 1: a sentence goes to `run_sentence`, a walk_to with B None). It writes
   `DIR/machine/RUN/<label>.jsonl.gz` (move, run, path and predict lines) and `<label>.trace.jsonl.gz`.
   A move with no action gets the move line only and no trace, as in golden.py.
3. `candidate.py check` (Python, nice 10) adds the last line, check.py's record. It decodes the Rust run
   and path lines back into `model.Run` and `model.Path` objects. It calls `check.score_move` with
   `check.run_model` patched to return that Run, as golden.py's capture does. It also stops if the decoded
   run does not encode back to the same lines, if the move line is not golden.py's, or if the predict line
   is not `model.predict` on the decoded run.

`--no-check` writes the move files without check.py's line, so `golden.py compare` then reports every move:
use it for timing only, into a folder you then throw away. With `--no-prep` the runner reads the folder's
jobs.json as it is (`--only` filters in prep only). `--repeat N` runs each move's machine N times and keeps
the mean time (timing, `profile.sh`).

`DIR/machine/RUN/rust-index.json` keeps the Rust time per move (machine alone, and with encoding and
writing), the paths, the trace events, and peak RSS.

Why check.py's line stays Python: `classify` holds the route's own moves (run B's VIA labels and object
numbers). The Rust must not name those. check.py reads nothing from a run but the run and path lines
(golden.py's CHECK_READS). So a Rust run that differs in anything check.py reads also shows up as a
difference in the check line.

Tests (`cargo test --release -p scumm-machine`; add `-- --include-ignored --test-threads 2` for all):

- `row0_subset` (about 16 s): 7 moves of row 0, compared file by file with the goldens, traces too: forks,
  a WaitFor fork, a room change, an object's end, the three actions that run nothing at row 0 (Esc,
  dialog line, wait), and a move with no action.
- `row1_subset` (about 16 s): 9 moves of row 1: merges, an actorFollowCamera fork, an endCutscene and a
  reentry (S1#33), a chainScript (S2#10), the sentence slot's own turn first (S1#40), loadRoom and a
  cutscene's freeze (S1#13), findInventory and loadRoomWithEgo (S1#11), a parked caller (S1#43), a
  doSentence (S1#96), an Esc skip that runs nothing at row 1 (S1#2), a move with no action (S1#95).
- `row2_subset` (about 25 s): 10 moves of row 2: startScene with the ego and two nesting ends (S1#11), a
  room change in an open cutscene (S1#58), room 0 (S1#13), a live object slot's kind taken from its owner
  and stopTalk (S3#78), VAR_ROOM_FLAG as a value set (S1#8), the entry code of an unknown room (S3#44), five
  room changes with startScene left on the stack (S1#42), merges at scale with three room ends (S1#83), an
  Esc skip that runs nothing at row 2 (S1#2), a move with no action (S1#95).
- `row0_full`, `row1_full`, `row2_full` (ignored): all 239 moves, `golden.py compare --runs rowN --traces`.
- `row0_wide`, `row1_wide`, `row2_wide`, `row2_synth`, `row2_limits` (ignored): the extra cases below,
  Python against Rust. `row1_wide` takes 5 to 10 minutes, `row2_wide` about 7, `row2_synth` about 10,
  `row2_limits` about 2, most of it Python. The whole suite with two test threads: 14 minutes at load
  about 8 (17 tests: 6 unit tests, 11 golden tests).
- unit tests in `value`, `enc`, `oracle`, `plist`.

Run the ignored tests after any change to shared code. Later stages: add a `rowN_subset` and a
`rowN_full` test per row in `tests/golden.rs` (the helpers `subset`, `full`, `wide` take a run name), and
keep the earlier rows' tests passing. This stage changed `_merge`'s walk over the writes and the join
signature's equality (speed, below) and re-ran every earlier test: all pass.

### Beyond the goldens

The goldens reach little of the code. `wide.py` (Python) and `machine-cases` (Rust) run more cases and
dump them in golden.py's format; `golden.py compare-file` compares the dumps:

    ../../../.venv/bin/python wide.py dump --out W [options]    # from scumm/machine
    ../target/release/machine-cases W                           # writes W/rust.jsonl.gz, W/rust.trace.jsonl.gz
    cd ../.. && ../../.venv/bin/python golden.py compare-file W/python.jsonl.gz W/rust.jsonl.gz
                                                                # and python.trace.jsonl.gz / rust.trace.jsonl.gz

Row 0 (`--row 0`, the default):

- objects: every object's entry for each verb of its event table, plus verb 0 (the 0xFF default),
  plus three object numbers with no code; from 5 ground-truth states (rows 3, 60, 120, 180, 240).
  9,350 cases, 26 s.
- units (`--units --rows 120`): every global, local, entry and exit unit from instruction 0, from row
  120's state. 2,591 cases with the objects of row 120, 14 s.

Both are equal, traces too. They reach: 2,077 path-limit ends, 15 unmodelled `systemOps`, unknown
writes of 8 kinds, room ends, yields, and forks on locals, actors, positions and messages.

Row 1 (`--row 1`, the machine with dispatch and calls):

- units (`--only-units --rows 120 --budget 2 --max-mb 20`): the 721 units from instruction 0, every start
  run nested. `--budget 2` drops a case Python runs for more than 2 s (6 units: global 14 and five locals;
  global 14 had dumped 780 MB when stopped), `--max-mb 20` one whose lines pass 20 MB (global 9: about
  1 GB). cases.json lists the dropped ones.
- objects and sentences (`--sentences --budget 2 --max-mb 20`): the 9,350 object entries above with calls
  on, plus 11,705 sentences through `run_sentence` from the same 5 states: each verb of each object's event
  table with A the object and no B, and per object its first verb with B the next object and with B = A.
  About 100 s in Python.
- `--path-limit N`, `--step-limit N` pass smaller limits to both machines (model.py's constructor takes
  them).

Measured on this stage with `--budget 2` and no size cap: 715 units and 21,055 objects and sentences equal,
traces too (the test drops global 9 by size: 714 units). They reach: 9,854
plus 5,580 merges (6,797 merge events), 227 spins, 354 "no such script" errors, 195 "object not here"
warnings, 2,370 dropped sentences, 2,926 paths that end with calls on the stack, unmodelled ends, room
ends, chained, parked and stopped ends, the recursive-end note. Not reached by any check so far: the
nesting limit (row 2's goldens have 2), value caps, the cutscene errors, a division by a known 0. The
path limit with calls on is reached by the limits run below.

Limits run: `--only-units --rows 120 --path-limit 8 --step-limit 300` (721 units) and `--sentences --rows 120
--path-limit 4` (4,211 cases): equal, traces too. 355 paths end "limit": path-limit cuts (326 forks dropped,
counted while other paths waited at joins) and step-limit ends with calls on the stack. 934 paths end with
calls on the stack.

Row 2 (`--row 2`, room_change). Synthetic states (wide.py's docstring): `--drop-kinds F,...` takes fact
families out of the held ones (their keys read "not held"), `--set-prefix P=V` gives every key of the state
that starts with P the value V, `--set K=V` sets one key; a key set to 0 leaves the state. Both machines
get the same state (cases.json carries the changes; machine-cases applies them as wide.py's `perturb`).
wide.py also drops a case whose output rests on Python's set order (below, "Kept apart"), listed under
"hash_order" in cases.json. Measured on this stage, all equal, traces too (56,255 cases):

| dump | wide.py options (all `--row 2 --budget 2 --max-mb 20`) | cases kept | dropped |
|---|---|---|---|
| units, 3 states | `--only-units --rows 120` (and 60, 240) | 713, 716, 715 | 7, 5, 6 over 2 s; 1 over 20 MB |
| objects and sentences | `--sentences` (rows 3, 60, 120, 180, 240) | 21,021 | 28 slow, 6 big |
| live object slots | `--sentences --rows 37,124,215` | 12,601 | 28 slow, 4 big |
| owners 1 | `--sentences --rows 120 --set-prefix owner:=1` (and rows 37) | 4,206, 4,206 | 3 slow, 2 big |
| owners not held | `--sentences --rows 120 --drop-kinds owner` (and rows 37) | 3,571, 3,574 | 166 and 163 slow; 474 and 474 on set order |
| limits | `--only-units --rows 120 --path-limit 8 --step-limit 300`; `--sentences --rows 120 --path-limit 4` | 721; 4,211 | none |

The ground truth has 13 states with a live object slot (rows 37, 68, 72, 79, 95, 124, 144, 215-218, 232,
239), all room slots; none has a live inventory slot, hence the owners-1 states. The wide dumps reach (counted
on their path and trace lines): 2,500 paths that ran scene_begin, 6,388 global, 2,381 exit and 1,106 entry
code pushes by startScene, 1,188 room-killed turns, 402 room-slot and 383 inventory-slot assumptions of
`_kill_room`, 15 "slot kind not known" writes, 22 "taken as surviving" and 4 "slot kind unknown" notes, 33
room changes in an open cutscene, 341 "ego object not in room" errors, 46 room 0 loads, 886 room codes of an
unknown room, 221 stopTalk writes, VAR_ROOM_FLAG kept at 1. The goldens add VAR_ROOM_FLAG as a value set (5
paths) and 2 nesting ends (S1#11). Not reached by any check: the "killed with an open cutscene" warning, an
unknown VAR_TALK_ACTOR at stopTalk, a resource 0 for a room that is not 0, the note on an unknown ego object,
an unknown VAR_ROOM_FLAG. `survey.py DUMP...` (here) prints these counts for a wide.py dump or a run folder.

The tests (`row2_wide`, `row2_synth`, `row2_limits`) run these with `--budget 1` for the synthetic ones and
rows 37 only for the owners-1 and owners-not-held states.

## What is ported

Modules (`src/`):

- `value.rs`: `Val` is `Int(i32)` or `Unk(UnkId)`, `Copy`, 8 bytes. Unknowns are interned per run in
  `Vals`, so equal unknowns have equal ids. model.py compares its frozen `Unknown` by value, so this
  keeps its equality. `Vals` also holds `VALUE_CAPS` (sites in first-count order). It has `one_of`,
  `values_of`, `join_values`, `decide`, `arith`, `i16w`, `wrap32`. Row 1 makes value sets (merges,
  findInventory).
- `oracle.rs`: `FactOracle`. `get` gives an int, "not held" or "timer". `tracks` is model.py's family rule.
  The prefix listings (`keys`) are built once per oracle. Their order is free: every model.py caller
  sorts them, or reads them in a way the order does not show.
- `path.rs`: `Frame`, `Call`, `Step`, `Entry`, `Path`, `Run`, with every model.py field; `Sig` and
  `SigItem` (a join signature); `Store`, the writes map (it reads as an `FxHashMap` and keeps an order-free
  hash of its entries as they are written). Strings that paths share are `Rc<str>`. `Run.vals` holds the run's
  unknowns (the paths' values index it). `Call.cid` is the call's identity (model.py's `id(steps)`).
  `Step` has row 1's steps: write, note, resume, sentence, script, object, hook, room1, scroll_note,
  cut_index; and row 2's: scene, scene_begin, room_code, scene_kill, scene_load, scene_ego. `Sig` carries a
  hash of itself, made once (`Sig::new`), that its equality tests first.
- `plist.rs`: `PList`, the list type of a path's assumptions, unmodelled, notes, unknown writes, errors,
  turns and conflicts. Frozen chunks of 64 items are shared by forks; a fork copies the short tail only.
  `add_branch` is `_merge`'s list rule: the common prefix is found chunk by chunk by pointer, then item by
  item by value.
- `machine.rs`: `Machine`. `run_entry`, `run`, `_drive` (joins, waiting groups in insertion order,
  merges, spins, the path limit counting waiting paths), `_advance`, `_step`, `_jump`, `_wait`, `_start`
  (row 0 and calls), every `_op_*` rule on its row-0 and calls branches, `read`, `write`, `val`, `test`,
  `getter`, `_getter_on`, `_owned_by`, `where`, `target`, `put`, `assign`, `_arith_at`, `clear`,
  `stopped`, `may_stop`, `started`, `hook`, `_inventory_hook`, `_room_change`, `_end`, `_done`, `_halt`,
  `_error`, `_note`. Also `predict` and `feasible`.
- `machine/calls.rs` (row 1): `run_sentence`, `_join`, `_spins`, `_merge`, `_call_step`, the `_eng_*`
  steps (`script`, `object`, `hook`, `resume`, `sentence`, `room1`, `scroll_note`, `cut_index`, `write`,
  `note`), `_sentence_slots`, `_sentence_waits`, `_push_call`, `_push_frame`, `_finish`,
  `_stop_script`, `_kill_object`, `_live_slots`, `_count`, `_set_freeze`, `_freeze`, `_unfreeze`,
  `_room_slot`, `_gone`, `_find_inventory`, `_scene_steps`.
- `machine/rooms.rs` (row 2): `_eng_scene`, `_eng_scene_begin`, `_eng_room_code`, `_kill_room` (the
  scene_kill step), `_eng_scene_load`, `_eng_scene_ego`. loadRoomWithEgo's room_change branch is in
  `machine.rs` (`op`).
- `heads.rs`: model.py's HEADS effects for op and start names.
- `enc.rs`: golden.py's canonical encoding (`enc`, the order rule, `path_dict`, `run_dict`, `_where`),
  step tuples included.
- `trace.rs`: golden.py's TraceMachine events, with path ids given in first-event order. Lines stream to a
  sink (a gzip file in the runner).
- `harness.rs`: the ground truth's pre states (groundtruth.py `load`), the jobs file, check.py's
  `run_model` dispatch, and a move's lines.
- `bin/machine_run.rs`, `bin/machine_cases.rs`, `candidate.py`, `wide.py`, `tests/golden.rs`.
  machine-cases applies wide.py's synthetic state changes (`set_prefix`, `sets` in cases.json).
- Tools (Python, the experiment's venv): `survey.py` (what a dump reached), `setorder.py` (getters whose
  model.py result rests on set order, per run), `profile.sh` (where the Rust time goes).

Trace events emitted: start, fork, merge, push, finish, end, halt, error, sentence. The fork event is
written after `_advance` returns and before the path-limit cut, as in golden.py. Settle's (turn, skip,
round, settled, queue), dialog's (input) and esc's (abort, key) are not ported.

## Python behaviours kept on purpose

Each one is model.py's (or golden.py's) behaviour, reproduced as it is.

Row 0:

1. An unknown is never equal to an int, and is "not 0" (`v != 0` is true). So `clear`, `may_stop` and
   `saveVerbs` write a key whose value is unknown. An unknown is truthy in `if n:`.
2. An ended path can still be written in the same step. At row 0, `loadRoomWithEgo` runs the inventory
   hook after `_room_change` ended the path with "room": the hook still writes its script's slot fact
   (and `may_stop`'s unknowns). When the running script is the hook itself, it ends the path again with
   "end" (lifecycle off): the second end wins, and the trace has two end events.
3. Path limit: the fork event is logged with the new paths' ids before the cut. Then the parent's new
   assumption is undone and it ends "limit", even if the step had already ended it (a WaitFor's yield).
   The cut paths' ids never appear again. `end_at` prints the pc as `None` when there is none.
4. `_end` with "yield" and no resume offset (a yield at the last instruction) adds a note with an empty
   cite, and no `at:` key.
5. `test` on a condition that is not a Cmp, Not or Truth prints the condition's dataclass repr
   (`py_repr`). `val` on a Str, Concat or List is `?(cannot work out: text)`; on a Flag, Cmp, Not or Truth
   it prints the expression (`show`).
6. An Unknown with an empty `what` prints `?(reason)`; one with values prints ` in [1, 2]`.
7. A bit write stores 0 or 1. A local write does not go through `write` (no `rw` log). A write to var 1
   also writes `ego`.
8. `setState`, `setOwnerOf`, `saveVerbs` and `restoreVerbs` evaluate all their arguments before any check.
   `drawObject` evaluates its setImage value before it checks the object.
9. `endCutscene` at row 0 does not read the `esc:` facts: model.py's `armed = self.calls and ...` stops at
   `self.calls`.
10. Keys are sorted as strings where model.py sorts them and the order shows (`_owned_by`'s first unknown
    owner: `var:10` sorts before `var:9`). `sort_str` does this.
11. `predict`: an oracle value that is unknown (not held, timer) counts as an unknown path value. Its
    `pre` is the unknown's string.
12. check.py's "no effect at this row" path has `Frame(("none",), 0)`. Its unit is `NONE_UNIT` here and
    prints `["none"]`.
13. Python ints are unbounded; the port keeps values in i32, as model.py wraps every stored value. A key
    number (an index sum) past i32 would panic (`key_num`). None occurs.

Row 1 (dispatch, calls):

14. The engine's own call ending (the stack empties in `_call_step`) sets the path's end to its last
    turn's `how` (or "nothing ran") at the call's cite directly: no `_end` or `_halt`, so no trace event.
15. `_finish` with "yield" writes the new `at:` key and does not clear the slot's older ones, and with
    "end" it clears the slot's `esc:` keys but not its `at:` keys. Only a stop, a resume, `run`, a turn
    or a kill clears them, so after a recursive start (which stops nothing) the old instance's `at:` key
    stays 1 next to the new one's.
16. startScript of script 0 with calls on still pushes a call. Its step does nothing, the call ends, and
    the caller goes on: one more step in the trace's `where`, and a join between the call's steps.
17. `_eng_resume` takes the slot's wait points from the oracle's keys only, not from the path's writes.
    Its "deferral unknown" text prints Python reprs: the unit as `('global', 5)` or `None`, the wait
    points as `['at:script:5:12']`, string-sorted.
18. `_merge`: the merged path is the first path's fork. Each other path adds, list by list, what follows
    its common prefix with the first path (compared by value). In a loop the lists so grow at every
    merge: global 9 from its start ends with 5.8 million assumptions over 39 paths. `reentries` is
    summed; `steps`, `depth`, `next_sid` are the max; `merged` is the max plus 1. A freeze count that one
    path lacks becomes None (not known), even when the others agree.
19. A merge of one path makes no event, but golden.py gives the path a trace id if it has none (`_tid`
    runs on every member first). A merge of two or more makes a new path with a new id.
20. `_spins` keeps each state as a 64-bit hash of (signature, sid, locals, writes, freeze counts), as
    model.py keeps Python's `hash()`. Equal states give equal hashes; neither side expects a collision.
    `seen` is copied by forks and never cleared at row 1 (only settle's turns clear it).
21. A join signature names a call by `id(steps)`; `Call.cid`, given once per Call made and shared by forks
    and merges, is the same identity. A call's head and cite follow from it and are left out.
22. `setOwnerOf` with an unknown owner while the object has a frame on the stack forks; the "owner 0" side
    writes `owner:O = 0` straight into the store (no `rw` log), after `_kill_object`, then runs the hook.
23. `_freeze`: a frame with an unknown F flag is noted and not frozen. Off-stack slots come in
    `_live_slots` order: the `script:`, `local:` and `objscript:` keys, each string-sorted, then entry and
    exit; their notes and `frozen:` writes follow that order.
24. `run_sentence` with the sentence slot's liveness or freeze not known returns a Run with one path whose
    stack is empty and which was never driven: no trace events, `value_caps` {}.
25. `_finish`'s resume test: the call goes inactive when its caller is gone, killed or frozen
    (`freeze > 0`). `p.last` is set only when the frame below is nothing or the engine's own call.
26. findInventory with calls on: an unknown owner fact gives `?(inventory order: findInventory(O,N): owner:K
    <value>)` naming the first unknown owner key in string order. An index out of 1..count is 0.
27. A division by a known 0 with calls on ends the path ("error"), but the value is still worked out (an
    unknown) and the statement finishes on the ended path.
28. A room change without room_change (`_eng_room1`) writes `room` even when the call is no longer active;
    it kills the caller only while the call is active.
29. `_stop_script` and `_kill_object`: one fatal error per frame with an open cutscene, then `stopped`.
30. The trace's finish event gives the `resume` passed to `_finish` (None for parked, even though parked
    works out its own) and the frame's `pc`.

Row 2 (room_change):

31. startScene is a call of its own. The opcode's call (loadRoom, loadRoomWithEgo, actorFollowCamera) holds
    the step `("scene", room, ego)`; `_eng_scene` pushes a "startScene" call (cite `room.cpp:42-260`) with the
    opcode call's caller and activity. Both calls are on the stack, in join signatures and in the path dump.
    When startScene pops, the opcode's call takes `active and startScene.active` (`call_step`).
32. scene_begin makes the call inactive when the caller is a room slot (room code, local, or an object slot
    not in the inventory), but does not kill it. The kill comes at scene_kill, after the exit code ran. The
    caller's turn then ends "room" when the opcode's call pops, at that call's cite. An open cutscene in the
    caller is the fatal "room change in an open cutscene": the path ends before the exit code. An object
    caller of unknown kind gets the note "slot kind unknown (...); taken as surviving the room change".
33. stopTalk: VAR_TALK_ACTOR in 1..127 becomes 255, an unknown becomes `?(cannot work out: stopTalk with
    VAR_TALK_ACTOR <talk>)`, any other value is not written. VAR_NEW_ROOM is written last in scene_begin.
34. room_code reads `room` when it runs: the exit code is the old room's (scene_load comes later), the entry
    code the new room's. An unknown room makes an unknown write ("startScene", "room.cpp:42-260", "the entry
    code of room ?(...)") and runs nothing. A room with no EXCD or ENCD runs nothing, with no note. The code
    runs nested at pc 0 with no slot fact: owned False, R False, F False, args known, no locals.
35. `_kill_room`, frames on the stack, bottom first: an object frame of unknown kind is noted ("<unit>: slot
    kind <unknown>; taken as surviving") and lives; a room slot not yet killed is killed ("room") and `_gone`
    writes its facts; an open cutscene there is a warning (not fatal), and the frame is still killed. Frames
    already killed are left as they are.
36. `_kill_room`, slots off the stack in `_live_slots` order: an object slot's owner is read. Unknown: its
    `objscript:` key becomes `?(cannot work out: obj:O: slot kind not known (<owner>))` and nothing else
    changes (its `at:`, `esc:`, `frozen:` stay). Known: an assumption (startScene's cite, "obj:O runs in a
    room slot (its owner is 15)" or "... inventory slot (its owner is N)", True, "the slot kind is not a
    fact") with no fork, then only a room slot dies. The why is plain text, not an unknown's string, so
    check.py's `assumption_kind` gives the text itself. Global slots live on. Local, entry and exit slots
    die: slot fact 0 (entry and exit have none), `at:` and `esc:` cleared, freeze count (0, exact), kept
    slot dropped.
37. scene_load writes `room`, VAR_ROOM and VAR_ROOM_RESOURCE first, room 0 too, and then room 0 ends the call
    (`call.i = len(steps)`): no VAR_ROOM_FLAG, no camera bounds, no entry hooks or code. The exit hooks, the
    exit code and the kills have run by then. An unknown room is never 0, and its resource (itself) is never
    0: it writes everything and goes on. A resource 0 of a room that is not 0 writes the camera bounds, then
    ends the call.
38. VAR_ROOM_FLAG: 1 stays 1; another int f becomes `?(resource cache: 1 if the room resource was not in
    memory in [f, 1])`, built directly (`Vals::mk_set`, no value cap); an unknown becomes the same unknown
    with no values. VAR_CAMERA_MIN_X is 160, VAR_CAMERA_MAX_X the room width minus 160, or `?(cannot work
    out: width of room <res>)` when the resource is unknown or has no width.
39. scene_ego: an unknown object is not looked up; the note prints it twice ("whether object ?(...) is in the
    new room: ?(...)"). A known one goes through `where` after the new room is set, so object 0 is looked up
    too. "inventory" or "nowhere" is the fatal "ego object not in room".
40. loadRoomWithEgo with room_change: its call's steps are `("write", "var:38", obj)`, the scene with the ego
    object, `("write", "var:38", 0)`, scroll_note, and the inventory hook. `obj` is the operand's value, an
    unknown when it is one, and is dumped in the steps.
41. actorFollowCamera's room-change side runs `("scene", ?(actor state: room of actor A), None)`: the
    unknown goes into room, VAR_ROOM, VAR_ROOM_RESOURCE and VAR_NEW_ROOM, the exit code is the old room's,
    and the entry code is an unknown write.

Kept apart (decide when a row reaches it):

- `getter` on value sets: model.py takes the first unknown of a Python set (`next(v for v in out ...)`),
  which is hash order. With PYTHONHASHSEED random (the goldens' setting), that is not deterministic when
  two different unknowns are in the set. The port takes the first in combination order. Measured with
  `setorder.py` (a subclass of Machine) on the goldens' moves: row 1 runs 576 getters on value sets
  (getVerbEntryPoint), row 2 runs 10,862; none has two different unknowns among its results. Check rows
  3-5 the same way. The synthetic owners-not-held states reach it (474 of 4,211 cases at row 120): there,
  model.py gives either unknown from one process to the next (one case, run under PYTHONHASHSEED 0 to 5:
  owner:16, 17, 17, 16, 17, 16). No port can match that, so wide.py's `Machine` notes such a getter and drops
  the case ("hash_order"). If the goldens of a later row reach it, the golden itself is one draw: decide
  with the PI (for example, fix model.py to a sorted order in a new oracle version).
- `endCutscene`'s `armed` (calls on) reads the `esc:` keys of a Python set until the first 1. Without a
  read log (settle's `owner`) the order changes nothing, and the port asserts there is none. In a settle
  turn the keys read before the first 1 go into `rw`, so which ones depends on Python's set order: a
  nondeterminism of model.py that row 3 must decide (for example, read them all, sorted, and note it).

## Notes for the next stage (settle: row 3)

Settle is the largest feature: time passes in rounds after the action's immediate part. The goldens of rows
3-5 are in out/golden/machine (row3, row4, the two row-4 switch runs, row5 and its switch run); they are big:
row 3 took 1,695 s of Python CPU and dumps 998 MB raw (60 MB gzipped), rows 4 and 5 about 1.1 GB each.

- model.py's settle code: `run_wait` (1538), the settle parts of `run_sentence` (1492), `_drive` (`_close_settle`
  at a path's end), `_join` (the ROUND call's signature and the settle tuple), `_merge` (kept slots, `immediate`,
  `rw`, `ran_by`, `writer`, `ambient_vals`, the de-duplicated lists, `settle_merges`, `value_merges`), `read`
  (TIME_VARS after round 0), `_jump` (time loops and the camera exit), `_wait` (rows 3+ rule), `_finish`
  (kept slots, `multi`), `_eng_script` (second instances), `_eng_resume`, `_eng_sentence` (the queue),
  `_push_frame` (didexec), `_op_doSentence`, `_freeze`/`_unfreeze` (the queue's counts), and the settle
  methods 3080-3513: `_pre_slots`, `_slots_sig`, `_position_key`, `_state_key`, `_changed`, `_unsettle`,
  `_next_round`, `_round_order`, `_round_end`, `_round_conflicts`, `_loosen`, `_close_settle`, `_unit_named`,
  `_slot_frame`, `_changed_any`, `_pre_rule`, `_origin`, `_skip`, `_time_wait`, `_eng_turn`, `_eng_sentences`,
  `_sentence_busy`.
- The 24 `unreachable!` marks and the endCutscene `assert!` (`grep -rn "not ported" src`) are the places to
  fill. Trace events still to add: turn, skip, round, settled, queue.
- `_spins`' `seen` is cleared by settle's turns; `Path.round_slots` and `history` hold hashes (golden.py dumps
  `history_len` only).
- `endCutscene`'s `armed` in a settle turn reads `esc:` keys in Python set order (Kept apart, above).
- Run the getter set-order check, `setorder.py RUN` (here; a model.Machine subclass, untraced), on rows 3-5's
  moves. Row 2: 10,862 getters on value sets, none with two different unknowns among its results (2 min).
- Speed candidates (profile first: `profile.sh RUN DIR` for moves, a gdb loop like it over `machine-cases DIR
  --no-trace` for wide cases): the join signature is built and hashed at every join visit (about a third of
  the time on the slowest row-2 units); merges in loops copy whole paths and extend the lists (each
  assumption 56 bytes with three `Rc<str>`: interning them as u32 ids would make copies and prefix compares
  integer work); `prefixed()`/`clear()` scan every write and sort at each call; `seen` is copied by every
  fork. Settle's round-start state (`_state_key`, `history`) hashes and copies whole stores each round:
  `Store::hash_sum` is the writes' part of such a hash already.

## Speed

Measured on the laptop on 2026-10-24, other jobs running (load about 5 unless a line says 10), one thread,
machine time only (no trace, no scoring, no encoding). Python: model.Machine timed around each call (scratch
scripts). Python and Rust were timed back to back for each line.

| work | Python | Rust | ratio |
|---|---|---|---|
| row 0's 232 moves with an action | 0.53 s | 0.002 s | about 250x |
| row 0: 9,350 object entries | 7.06 s | 0.053 s | about 130x |
| row 0: 2,591 units from instruction 0 | 5.34 s | 0.077 s | about 70x |
| row 0: slowest case (global 14 from its start) | 354 ms | 4.7 ms | about 75x |
| row 1's 232 moves with an action (load about 5) | 1.28 to 1.39 s | 0.020 s | about 65x |
| the same at load about 10 | 1.79 s | 0.037 s | about 48x |
| row 1: slowest moves (S1#33, S3#44; load about 10) | 33.9, 31.5 ms | 0.79, 1.08 ms | about 30-45x |
| row 1: 21,055 object entries and sentences (load about 10) | 180 s | 2.2 s | about 80x |
| row 1: 714 units from instruction 0, global 9 aside (load about 10) | 13.0 s | 0.27 s | about 48x |
| row 1: global 9 from instruction 0 (5.8 million assumptions; load about 5) | 0.58 s | 0.33 s | about 1.7x |
| row 2's 232 moves with an action (2026-10-25, load about 3) | 35.6 s | 1.25 s | about 28x |
| the same, both at nice 10, load about 8 | 72.1 s | 2.70 s | about 27x |
| row 2: slowest moves S2#21, S2#29, S1#83 (load about 3) | 15.0, 14.3, 4.2 s | 0.51, 0.52, 0.18 s | about 23-29x |
| row 2: 713 units from instruction 0 (row 120; nice 10, load about 7) | 17.8 s | 0.76 s | about 23x |
| row 2: 21,021 object entries and sentences (nice 10, load about 8) | 163 s | 1.65 s | about 99x |

Global 9 is model.py's list growth at merges (behaviour 18): the merged lists really hold millions of items,
and building them is the work. Before `PList`, Rust took 0.87 s there, slower than Python: a fork copied each
56-byte assumption where Python copies a pointer. Interning assumptions and notes as u32 ids would bring the
copies to 4 bytes (notes for the next stage).

Row 2's time is in three moves: S2#21, S2#29 and S1#83 are 33.5 s of Python's 35.6 s, and 1.21 s of Rust's
1.25 s. They are merges at scale: S2#21 runs 337,000 steps, 10,267 forks and 3,273 merges of 4 paths on
average, each path holding about 160 writes and 640 assumptions. Forks and merges copy and fold whole paths,
which is where Rust gains least.

Peak memory: the row-1 runner's Rust part 64 MB; Python about 110 MB per worker. Row 2: the runner's Rust
part 130 MB on one thread, 196 MB on four threads traced; Python's machine 119 MB; machine-cases on 21,021
cases 110 MB. machine-cases on the units
reaches 2.6 GB only when it encodes global 9's 1 GB of path lines (the test drops it with `--max-mb 20`).

Where the time goes: `./profile.sh RUN DIR` (from scumm/machine) samples the machine thread under gdb while
`machine-run --repeat N` repeats every move, and tallies the stacks. On row 1 after this stage's changes: half
the samples in `step` (spread over val, getter, assign, op, jump), about 12% in join signatures and their
allocation, 12% in merges, 5% in `_spins`, the rest in moving paths between the queues. This stage's speed
changes, each checked on every comparison above: `PList` (forks share the lists' frozen part), `Store` (an
order-free hash of the writes kept as they are written, so `_spins` does not rehash the store), and `assign`
making its statement head only for an unknown write.

The whole row-1 runner: Rust part 0.3 s wall (game load about 0.2 s, 64 MB peak RSS). The two Python steps
take about 16 s, mostly loading the game and the ground truth.

Row 2 on S2#21 with `profile.sh`: before this stage's changes, 42% of the samples were in `_merge`'s sort of
every path's write keys and 13% in comparing join signatures while looking for a path's waiting group. Now:
`_merge` walks the first path's writes and then each other path's new keys, with no sort (the order shows
nothing: the store is a map, and the unknowns a join makes are interned by value); `Sig` carries its hash and
equality tests it first; `step` makes its last cite only for a killed frame; `decide` on ints allocates
nothing. Each was checked on every comparison above. The profile is now flat: `step` 45%, `_merge` 23%
(the fork it starts from, the list rule, the joins), join signatures 7%, forks 9%. On the slowest row-2 units
(two locals from their start, 2.2 s in Python, 0.13 s in Rust) a third of the samples build and hash join
signatures. Traced runs cost about twice the untraced ones (the event lines are JSON built per event and
gzipped).
