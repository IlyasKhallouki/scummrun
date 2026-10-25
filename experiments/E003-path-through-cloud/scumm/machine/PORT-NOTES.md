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
| 3 | + settle | done | 239 of 239 moves equal, traces too; plus 2,590 extra cases equal (below) |
| 4 | + dialog | done | 239 of 239 moves equal, traces too; plus 5,806 extra cases equal (below) |
| 4, switch const_polls (run `row4-const_polls`) | | done | 239 of 239 moves equal, traces too |
| 4, switch input_after_turns (run `row4-input_after_turns`) | | done | 239 of 239 moves equal, traces too |
| 5 | + esc | done | 239 of 239 moves equal, traces too; plus 2,028 extra cases equal (below) |
| 5, switch esc_key_at_pre (run `row5-esc_key_at_pre`) | | done | 239 of 239 moves equal, traces too |

Every feature of model.py is ported (`machine::PORTED` holds all seven), and every machine golden run compares with 0
differences. No "not ported" mark is left in the Rust. check.py's model check runs on the Rust machine
(`check_rust.py`, next section), and its files for rows 0-5 and the three switch runs are byte-identical to results/.
What is left of D7's port is outside the machine: the reader (reader.py, the `reader/` goldens) and the speed work
listed under Speed.

## The model check on Rust (check_rust.py)

`../../check_rust.py` (in the experiment folder) runs check.py's own `main`, unchanged, with the Rust machine in place
of model.py's. It replaces three of check.py's module globals in its own process (and so in the workers check.py forks
with `--jobs`):

- `check.run_model`, the machine's entry point for one move, sends the action and the oracle's fact map and families
  to `machine-serve` (`src/bin/machine_serve.rs`) and decodes the Run it gets back.
- `check.OUT` (results/) becomes `--out DIR`: every file check.py writes goes there, never into results/.
- `check.score_move` is wrapped only to time each move.

Everything else is check.py's code: the moves and actions (`classify`, run B's VIA moves), usability, the scoring,
`model.predict` on the decoded Run, the summary, growth.md. check.py's scoring stays in Python, as in `candidate.py`,
because it names the route's own moves.

`machine-serve --features ... [--switches ...]` loads the game once, says hello with its features, switches and
limits (check_rust.py checks them against the Python Machine), then answers one request per move: it runs
`harness::run_model` and sends golden.py's `run` line and one `path` line per path. There is one server per scoring
process, started on first use. A forked worker closes and waits for its server through multiprocessing's finalizers,
so the server's CPU time is counted in the parent's.

The decoded Run has every field check.py reads (FORMAT.txt's CHECK_READS): every `Run` field, and every `Path` field
but `seen`, `round_slots`, `joined` and `history`, which keep their defaults. Three things differ from the Run
model.py builds, and check.py's output depends on none of them (all nine runs are byte-identical):

- Dicts come back in key order, since golden.py sorts them. `Path.slots` keeps the machine's order.
- Every set comes back as a `set`, also one that was a frozenset inside a value. `Unknown.values` is rebuilt as a
  `frozenset`, as in model.py.
- Lists that model.py shares between forked paths come back as separate copies. That is why Python's peak memory
  is higher than with model.py (below).

A later check.py that iterates a Path dict in insertion order, or hashes a set inside a Path, would see the first two.

**The wire.** The lines are golden.py's, with one change for speed, made only in machine-serve (`compact`,
`turns_cols`). A Python list of two or more tuples of the same length, holding only null, bool, int or str, goes by
columns: `{"$tuples": [column, ...]}`. A column of strings goes as `{"$strs": [table, width, base64]}`: each string
once, and the indices as little-endian u16 or u32 in base64. Python rebuilds the list with `array`, `map` and `zip`.
A path's turns (3.8 million in S1#8) are built in that form straight from the path, without a JSON value per turn
(`enc::path_with(.., false)`); `enc::path` and the goldens are unchanged. The decoder turns the cyclic garbage
collector off while it builds (the decoded objects hold no cycles). On S1#8, the biggest move (one process, both
forms back to back, load about 12), this took the Rust's encoding from 5.4 s to 0.8 s, the bytes from 132 MB to
26 MB, and Python's decoding from 8.1 s to 1.8 s. Over row 3, the bytes went from 512 MB to 259 MB and Python's peak
memory from 1,055 MB to 445 MB.

Options: `--verify` makes the server also send the canonical lines, and check_rust.py encodes every decoded Run back
with golden.py and stops at the first line that differs. `--canonical` makes the server send the canonical lines
only (for comparison). `--resume` is refused: check.py keys its resume records by the Python sources only, so a
folder could mix records of both machines. `--bin PATH` uses another machine-serve binary.

**Run it** (from the experiment folder; it runs `cargo build --release -p scumm-machine --bin machine-serve` first
unless `--no-build`):

    mkdir -p DIR && cp results/growth.md DIR/        # check.py replaces its row of DIR/growth.md
    F=own_entry,dispatch,calls,room_change,settle,dialog,esc
    nice -n 10 ../../.venv/bin/python check_rust.py --out DIR --features $F                     # row 5
    nice -n 10 ../../.venv/bin/python check_rust.py --out DIR --features $F --switches esc_key_at_pre
    cmp results/check-part1-row5.jsonl DIR/check-part1-row5.jsonl
    cmp <(tail -n +2 results/check-part1-row5.log) <(tail -n +2 DIR/check-part1-row5.log)   # line 1 holds the time
    cmp results/growth.md DIR/growth.md               # after rows 0-5 have run

Run the rows in order 0 to 5 and then the switch runs, as check.py would: each row run also writes
check-part1.jsonl and .log (the latest run), and results/ holds row 5's. Besides check.py's files it writes
`DIR/<stem>.rust-times.tsv` (per move: the Rust machine's seconds, encoding, bytes, decoding, the whole move's wall
and CPU in Python, paths, peak RSS of both processes) and `DIR/<stem>.rust-summary.json` (the run's wall time, CPU of
all its processes and of the Rust servers, peak RSS, the slowest moves).

Tests: `cargo test --release -p scumm-machine --test check_rust` runs rows 0-2 and compares each jsonl, each log from
line 2 and growth.md with results/ (about 20 s idle, 40 s at load 15). `check_rust_all` (ignored) does all nine runs
with two jobs each (9 minutes at load about 12).

**Result (2026-10-25).** For rows 0-5 and the three switch runs, every file matches results/ byte for byte: each run's
jsonl, its log from line 2, check-part1.jsonl and .log (row 5's), and growth.md. This was checked four times, in four
fresh folders: with the canonical lines (eight runs; the ninth broke on an edit to check_rust.py made while it ran, and
was rerun in the later passes), with the
first column wire, with the final wire on one job, and with the final wire and `--verify` on four jobs. The verify run
encoded every decoded Run of every move of all nine runs back to the server's canonical lines with no difference.
Then `check_rust_all` passed on the final build (two jobs per run, 9 minutes at load about 12).

**Times.** The laptop was very busy (load 11 to 21 on 8 threads, other agents' jobs), so wall times are long for both.
The fair comparison is the slow moves, Python and Rust run in the same window (2026-10-25 10:37 to 10:59, load 11 to
21). Python is `check.score_move` with model.py, one process per move, in parallel. Rust is `check.score_move` through
check_rust.py, three passes back to back. Both gave the same check line (sha256 equal).

| move (row 5) | Python CPU | Python wall | Rust CPU (3 passes) | Rust wall | CPU ratio |
|---|---|---|---|---|---|
| S2#21 | 892 s | 1,287 s | 23.5 to 24.5 s | 44 to 58 s | about 37x |
| S2#29 | 885 s | 1,304 s | 22.2 to 23.9 s | 42 to 49 s | about 38x |
| S1#83 | 282 s | 569 s | 6.9 to 7.4 s | 12 to 18 s | about 40x |
| S1#8 (131 MB of lines) | | | Rust 0.3 s, then 1 to 2 s of encoding, 3 to 15 s of decoding at that load | 8 to 28 s | |

Per row: check_rust.py on one job, final wire, 2026-10-25 10:19 to 10:34 (load 9 to 16). Python: check.py's own runs
that wrote results/ (log line 1, wall; job count not recorded), the CPU of results/*.times.tsv (rows 0-3, untraced,
scoring only), and the goldens' run (`golden.py machine`, traced, 4 jobs; out/golden/machine.log).

| run | Python: results/ log line 1 | Python: times.tsv CPU | Python: goldens, CPU / wall | Rust: wall | Rust: CPU all (Python part, Rust part) |
|---|---|---|---|---|---|
| row 0 | 3.7 s | 1.1 s | 0.9 / 2.0 s | 6.2 s | 6.4 s (5.7, 0.7) |
| row 1 | 5.4 s | 3.6 s | 2.9 / 1.7 s | 7.0 s | 7.3 s (6.0, 1.2) |
| row 2 | 52.8 s | 90.1 s | 64.8 / 26.4 s | 17.2 s | 14.6 s (10.1, 4.5) |
| row 3 | 2,749 s | 1,652 s | 1,695 / 668 s | 112 s | 107 s (43, 64) |
| row 4 | 1,719 s | | 2,018 / 784 s | 136 s | 126 s (47, 79) |
| row 5 | 3,212 s | | 1,591 / 509 s | 163 s | 140 s (55, 85) |
| row 4, const_polls | 4,505 s | | 1,835 / 617 s | 176 s | 150 s (59, 91) |
| row 4, input_after_turns | 4,531 s | | 2,205 / 858 s | 125 s | 120 s (46, 73) |
| row 5, esc_key_at_pre | 4,504 s | | 2,217 / 884 s | 174 s | 146 s (55, 91) |

The Rust times above were taken at load 9 to 16. An earlier pass of the same runs at load 3 to 8, with the first
column wire, took 63 s (row 3), 71 s (row 4), 102 s (row 5) and 122 to 129 s (switch runs) on one job; its Rust
machine time was 26 and 30 s for rows 3 and 4. Rows 0 and 1 are slower than check.py on model.py: their moves take
milliseconds, and the fixed costs (Python loading the game and the ground truth, one JSON request of about 4,000 facts
per move) dominate.

Where a row's time goes now (row 5 at load about 12, 163 s wall): 79 s of Rust machine, 17 s of Rust encoding, 22 s
of Python decoding, and about 33 s of the rest of Python (imports, loading the game and the ground truth, `classify`
and `usable`, check.py's scoring and summary). S2#21 and S2#29 alone are 47 s of the 79 s of machine time.

**Peak memory.** Rust server: 84 to 86 MB per row run (28 MB on the slow moves alone; 293 to 309 MB with the canonical
lines). Python side: 445 MB, set by decoding S1#8, whose 165 paths hold 3.8 million turns as separate lists (model.py
shares them between forked paths). check.py on model.py: 109 to 171 MB per worker untraced (times.tsv), 161 to 202
MB traced (goldens).

## Run the comparison

From `scumm/`:

    cargo build --release
    for r in row0 row1 row2 row3 row4 row4-const_polls row4-input_after_turns row5 row5-esc_key_at_pre; do
        ./target/release/machine-run $r --out DIR --jobs 4; done   # 6 s, 8 s, 10 s, 2.5 min, then about 3 min each
    cd .. && ../../.venv/bin/python golden.py compare DIR \
        --runs row0,row1,row2,row3,row4,row4-const_polls,row4-input_after_turns,row5,row5-esc_key_at_pre --traces

(`machine-run` takes one run; run it once per run into the same DIR, then compare them together. The run names are
golden.py's (`golden.RUNS`, FORMAT.txt): a switch run is `row4-const_polls`, not the switch's name. Most of the
time is Python's prep and check: row 3's check step takes about 2 minutes and each row-4 or row-5 run's 2.5 to 3.5
minutes, because they decode 1 GB of path lines.
Row 3's heavy moves, S2#21 and S2#29, take about 11 s each in Rust traced on one thread, 8.5 to 10 s untraced. The
candidate folder of rows 0-3 takes about 120 MB, with row 4's three runs 409 MB, and each row-5 run adds about as much
as a row-4 run: trace files are written with the fastest gzip level, since golden.py compares their text, not their
bytes.)

`machine-run RUN --out DIR [--only S1#4,...] [--jobs 4] [--no-trace] [--no-prep] [--no-check] [--repeat N]`:

1. `candidate.py prep` (Python, nice 10) writes `DIR/machine/RUN/jobs.json`: for each counted move, its
   label, its row `i`, golden.py's move line, and the action as check.py's `run_model` reads it. Also the
   run's features, switches, key (golden.py `_key`) and fact families (`check.held_families`).
2. The Rust machine runs each move with an action. `harness::run_model` is check.py's `run_model`
   dispatch (row 1: a sentence goes to `run_sentence`, a walk_to with B None; row 3: a wait goes to
   `run_wait`; row 4: a dialog line goes to `run_verb` with a left click; row 5: a skip goes to `run_esc`). It writes
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
- `row3_subset` (about 15 s): 14 moves of row 3, picked by `survey3.py --cover` on the goldens: merges at a
  round's start into value sets and a second instance (S2#48, S1#59), a camera exit that forks once and stays
  (S3#60), a parked first end (S1#39), a wait (S2#30), doSentence's queue run at a round's end and an engine error
  (S1#96), a slot with no single wait point (S2#49), the round limit (S3#78), a poll whose facts did not change
  (S1#22), a chained first end (S2#10), the path limit (S1#33), endCutscene in a settle turn (S3#47), an Esc skip
  that runs nothing at row 3 (S1#2), a move with no action (S1#95).
- `row4_subset` (about 13 s): 8 moves of row 4, picked with `survey4.py --per-move` on the goldens: a local input
  script (S1#19), a turn that changed VAR_USERPUT before the click (input drift) on a path merged later (S1#34), drift
  on 32 merged paths (S2#8), merges after the click (S1#21), a move whose output const_polls changes (S3#14), and
  moves that run as at row 3: a wait (S2#30), an Esc skip that runs nothing at row 4 (S1#2), a move with no action
  (S1#95).
- `row4_switch_subset` (about 20 s): run `row4-input_after_turns` on four clicks it drops (VAR_USERPUT 0 after the
  turns: S1#34, S1#53, S2#6, S2#8) and run `row4-const_polls` on two moves the switch changes (S2#15, S3#14). These
  are the only moves where the switch runs differ from row 4: input_after_turns changes 8 moves (all dropped
  clicks), const_polls 4 (S2#15, S2#16, S2#17, S3#14; all dialog lines). Row 4's other 179 moves give row 3's files
  but for the move line.
- `row5_subset` (about 10 s): all 22 Esc moves of row 5 (0.1 s of Rust in all) and three moves that run as at row 4
  (a dialog line S1#19, a wait S2#30, a move with no action S1#95). The Esc moves reach: no override armed (S3#46), the
  rewound slot's turn (the other 21), the fork on merged paths that disagree on clearClickedStatus (S1#2, S3#86), the key
  dropped with input off (S3#13), wiped by clearClickedStatus (S1#3), taken by input scripts 4 (S3#59) and 14 (S3#16)
  and by an unknown one (S3#4).
- `row5_switch_subset` (about 10 s): the 22 Esc moves under run `row5-esc_key_at_pre`. The switch changes 13 of them
  (S1#2, S2#52, S3#2, S3#4, S3#13, S3#15, S3#16, S3#21, S3#42, S3#54, S3#59, S3#61, S3#86). Row 5's 217 other moves
  give row 4's files but for the move line, traces too (checked on the goldens).
- `row0_full` to `row5_full`, `row4_const_polls_full`, `row4_input_after_turns_full`, `row5_esc_key_at_pre_full`
  (ignored): all 239 moves, `golden.py compare --runs RUN --traces`. `row3_full` takes about 4 minutes, each row-4 and
  row-5 one about 5.
- `row0_wide`, `row1_wide`, `row2_wide`, `row2_synth`, `row2_limits`, `row3_wide`, `row4_wide`, `row4_synth`,
  `row5_wide`, `row5_synth`, `row5_arm` (ignored): the extra cases below, Python against Rust. `row1_wide` takes 5 to 10
  minutes, `row2_wide` about 7, `row2_synth` about 10, `row2_limits` about 2, `row3_wide` about 6, `row4_wide` about 15,
  `row4_synth` about 3, `row5_wide` about 4, `row5_synth` about 11, `row5_arm` about 2, most of it Python.
- unit tests in `value`, `enc`, `oracle`, `plist`.

The whole suite with two test threads (`-- --include-ignored --test-threads 2`, nice 10): 36.5 minutes at load
about 3 to 5 (36 tests: 8 unit tests, 28 golden tests), all pass (2026-10-25, after the row-5 stage's last change).

Run the ignored tests after any change to shared code. Later stages: add a `rowN_subset` and a
`rowN_full` test per row in `tests/golden.rs` (the helpers `subset`, `full`, `wide` take a run name;
`wide_costed` picks the cases by the Rust costs first), and keep the earlier rows' tests passing. The row-4 stage
changed shared code in two places: `merge` (the drift) and `FactOracle::get` (the `unheld` test, empty outside
wide.py's synthetic states). Every earlier test passes after them. The row-5 stage added three `Step` variants, their
encoding and their `eng` arms, and the skip arm of `harness::run_model`; nothing else shared changed.

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

Row 3 (`--row 3`, settle). `Machine.run` and `run_entry` start a path that does not settle, so their cases at row 3
are row 2's. The new cases are the two settle entry points:

| dump | wide.py options | cases kept | dropped | Python | Rust (traced) |
|---|---|---|---|---|---|
| waits | `--row 3 --rows all --waits --no-objects --budget 20 --max-mb 50` | 249 | 1 on set order | 110 s | 3 s |
| sentences | `--row 3 --rows 120 --sentences --no-objects --budget 2 --max-mb 20` | 2,341 | none | 198 s | 7.7 s |

Both are equal, traces too. `survey3.py DUMP...` (here) prints settle's counts for a dump or a run folder; with
`--cover` it picks a few cheap moves that reach all of them. On the 239 golden moves (8,890 settle paths): ends
settled 1,678, settled with a kept slot's locals still moving 252, cycle 436, round limit 10, spin 4,013, nesting
1,247, limit 1,139, error 115; 13,215 merges at a round's start, 12,403 of them into value sets; 44,317 waits over by
time, 8,058 loop exits by time, 21 camera assumptions (7 paths forked a camera exit); 64,140 facts made unknown by
settle; 180,281 order conflicts; 696 paths with a second instance, 8,879 with an ambient cap; 34 paths ran queued
sentences (37 doSentence notes). Every rule b is reached (a WaitFor op, a delay, a time-only loop, the camera's loop,
an ambient loop, a breakHere outside any loop, a parked slot), and these skips: frozen, freeze count not known, no
single wait point, a time-only wait held in the click's frame, polls facts the path has not changed, the ambient cap.
The waits add rule c (161 turns, "c: polls facts the path changed"); the sentences add the dropped first end (3,792)
and sentences still queued at the end (24).

Not reached by any check: the deferred and "no sentence script" first ends with settle (the click's sentence
queued behind a live sentence slot), rule c's WaitForSentence branch, the `_eng_sentences` notes (queue not run,
whether the sentence waits, a queued sentence whose B is its A), and the skips for a slot whose liveness is not
known, a kept slot whose position is not known, a wait point that is no instruction of its unit, and a pre wait
point of kind start or other. The `row3_wide` test runs both dumps with `--budget 1` for the sentences.

Row 4 (`--row 4`, dialog). The new entry point is `run_verb`; the others run as at row 3. `--clicks` gives, from each
state, a left click on every verb shown at pre (its `verb:` fact is 1), in number order, then on verb 0 (never shown).
`--switches` passes model.py's switches to both machines (cases.json keeps them).

Clicks from every state are 3,038 cases, and 38 of them dump over 50 MB of lines (one 152 MB): Python spends minutes
encoding such a case after its run, outside the budget's timer (the first try stalled there). So the Rust machine
picks what Python can afford: `wide.py dump ... --plan` writes cases.json with every case and runs nothing;
`machine-cases DIR --costs` runs them untraced and writes rust-costs.json (per case its time and its list entries,
no rust files; 47 s for the 3,038 clicks); `wide.py dump ... --from-costs DIR/rust-costs.json --max-rust-ms MS
--max-items N` keeps the cases under both (the rest under "costly" in cases.json). `--max-items N` alone drops a
case after its Python run and before encoding it. `--unheld K,...` makes the oracle read these keys as not held
(wide.py's `Unheld`, `FactOracle::with_unheld`; model.py has no such rule): an unknown VAR_VERB_SCRIPT or VAR_USERPUT
without making every variable unknown.

| dump | wide.py options (all `--row 4 --clicks --no-objects`) | cases kept | dropped | Python | Rust (traced) |
|---|---|---|---|---|---|
| clicks | `--rows all --budget 20 --max-mb 5 --from-costs PLAN --max-rust-ms 50 --max-items 200000` | 2,741 | 276 costly, 17 over 5 MB, 4 on set order | 595 s | 28.6 s |
| clicks, input_after_turns | the same with `--switches input_after_turns` | 2,741 | the same | 568 s | 26.6 s |
| VAR_VERB_SCRIPT 0 | `--rows 3,22,60,120,131,180,240 --budget 5 --max-mb 5 --set var:32=0` (and with the switch) | 54, 54 | 1 slow | 9 s each | 0.07 s |
| VAR_VERB_SCRIPT not held | the same rows, `--unheld var:32` (and with the switch) | 54, 54 | 1 slow | 9 s each | 0.07 s |
| VAR_USERPUT not held | the same rows, `--unheld var:53` (and with the switch) | 54, 54 | 1 slow | 11, 12 s | 0.13, 0.14 s |

PLAN is a `--plan` folder of the same options after `machine-cases PLAN --costs`. The two click dumps ran side by side
with the test suite (load about 9); an earlier pair at load 5 to 7 took 557 s in Python and 11.4 s in Rust. All are
equal, traces too (5,806 cases). The 4 set-order cases are endCutscene's `esc:` reads in a settle turn (rows
24, 26 and 202); kept with `--keep-order-cases`, they are equal too. `survey4.py DUMP...` (here) prints dialog's counts
for a dump or a run folder. The goldens reach: 68 input events (8 with drift, all ("var:53", 1, 0): a turn turned
input off before the click), 105 paths with drift (95 of them merged), a local input script (S1#19), 1,391 slot turns
by rule c, and with input_after_turns 8 dropped clicks. The wide dumps add: the switch's fork on an unknown VAR_USERPUT
(320 paths hold side False, 244 side True), the hidden-verb note (verb 0; 821 notes in the clicks), VAR_VERB_SCRIPT 0 (the note, start "input
script 0"), an unknown VAR_VERB_SCRIPT (the unknown write, start "input script ?(not held: var:32)"), and drift whose
pre value is an unknown (VAR_USERPUT not held, 43 paths). The clicks reach 25,400 input events (265 with drift, all on
VAR_USERPUT), 1,337 paths with drift (972 merged), 265 local input scripts, and with the switch 2,163 dropped clicks.
Not reached by any check: drift on VAR_VERB_SCRIPT or on a
`verb:` fact, a path holding two drift entries, a merge of paths whose drift for one key differs (the sort by
Python's `str`), and an unknown room resource for a local input script at pre (the Run's unit None).

Row 5 (`--row 5`, esc). The new entry point is `run_esc`; the others run as at row 4. `--escs` gives the Esc key from each
state. `--arm` gives, per state and per slot live at pre with exactly one wait point W, the Esc key with that slot's
override armed at W and at W + 1, each case on its own state (`"arm": "esc:<slot>:W"`; the state's other `esc:` keys
leave it unless `--keep-armed`). Every dump below picks its cases by the Rust costs first (`--plan`, `machine-cases
--costs`, `--from-costs ... --max-rust-ms 1000 --max-items 200000`; a few states make runs of millions of list entries)
and runs with `--budget 60 --max-mb 50 --no-objects`. TEN = rows 3,22,37,60,120,131,180,202,215,240.

| dump | wide.py options (all `--row 5`) | cases kept | dropped |
|---|---|---|---|
| Esc from every state | `--escs --rows all` | 247 | 3 costly |
| the same, switch esc_key_at_pre | `--escs --rows all --switches esc_key_at_pre` | 247 | 3 costly |
| code 0 (Esc not the exit key) | `--escs --rows TEN --set var:24=0` | 9 | 1 on set order |
| code not held | `--escs --rows TEN --unheld var:24` | 9 | 1 on set order |
| Enter, cursor as at pre | `--escs --rows TEN --set var:24=13` | 10 | none |
| Enter, cursor hidden | `--escs --rows TEN --set var:24=13,var:52=0` | 10 | none |
| Enter, cursor not held | `--escs --rows TEN --set var:24=13 --unheld var:52` | 10 | none |
| Tab | `--escs --rows TEN --set var:24=9` | 10 | none |
| code 512, code -1 (not a key) | `--escs --rows TEN --set var:24=512` (and `=-1`) | 10, 10 | none |
| VAR_USERPUT not held | `--escs --rows all --unheld var:53` | 246 | 4 costly |
| the same, switch esc_key_at_pre | `--escs --rows all --unheld var:53 --switches esc_key_at_pre` | 246 | 4 costly |
| VAR_VERB_SCRIPT 0 | `--escs --rows all --set var:32=0` | 245 | 5 costly |
| VAR_VERB_SCRIPT not held, switch esc_key_at_pre | `--escs --rows all --unheld var:32 --switches esc_key_at_pre` | 245 | 5 costly |
| path limit 3 | `--escs --rows all --unheld var:53 --path-limit 3 --step-limit 2000` | 250 | none |
| path limit 2, overrides | `--arm --rows 3,37,120,215,240 --unheld var:53 --path-limit 2` | 46 | none |
| an override on each live slot | `--arm --rows 3,22,37,60,120,124,131,180,215,240` | 78 | none |
| two armed keys | `--arm --keep-armed --rows 96,158,202,215,240` | 18 | 16 on set order |
| slots not live at pre | `--arm --rows 3,37,120,215 --drop-kinds script,local` | 36 | none |
| an override on each live slot, switch esc_key_at_pre | `--arm --rows 3,37,120,215,240 --switches esc_key_at_pre` | 46 | none |

All are equal, traces too (2,028 cases). The 16 two-armed cases dropped on set order are equal too when kept
(`--keep-order-cases`). `survey5.py DUMP...` (here) prints esc's counts for a dump or a run folder. The goldens reach
(22 Esc moves): no override armed (1 move), one armed with a known code (21), the rewound slot's turn (24 pushes by the
Esc call), the fork on `Path.cleared` None (9), and the key wiped (25 routes), dropped with input off (17), taken by input
scripts 4 and 14 (16) and by an unknown one (3, a local's `Local[0]`). The wide dumps add: two armed keys (18 abort events,
the note), overrides that are no instruction of their unit (96 skips) or whose slot was frozen by an earlier turn (8
skips, "liveness 1 or freeze 1") or is not held (15 skips), the fall-back to settle's own turn (7, the slot's `at:` key
moved or the slot ran already), overrides of global, local, object and entry slots and of slots not live at pre (the
step at the end), code 0, an unknown code, Enter and Tab with the cursor shown, hidden and not known, codes 512 and -1,
the fork on an unknown VAR_USERPUT, VAR_VERB_SCRIPT 0, an unknown input script with the switch, and input scripts 200 to
209 (rooms' local input scripts). Not reached by any check: an override slot's freeze count read as unknown after
abortCutscene (abort writes it 0, so `frozen:` is unknown only when an earlier turn makes it so). The two path-limit
dumps reach the cut-back of key routes at a cut fork (model.py's `del self._key_routes[routes:]`): in 64 cases the key
events outnumber the routes the Run keeps (438 events against 378 routes, and 82 against 10); 370 paths end "limit" with
the Esc call on their stack. The other dumps have no limit end in the Esc frame.

## What is ported

Modules (`src/`):

- `value.rs`: `Val` is `Int(i32)` or `Unk(UnkId)`, `Copy`, 8 bytes. Unknowns are interned per run in
  `Vals`, so equal unknowns have equal ids. model.py compares its frozen `Unknown` by value, so this
  keeps its equality. `Vals` also holds `VALUE_CAPS` (sites in first-count order). It has `one_of`,
  `values_of`, `join_values`, `decide`, `arith`, `i16w`, `wrap32`. Row 1 makes value sets (merges,
  findInventory). A value hashes as Python hashes it: -1 like -2 (behaviour 42). `Vals::show_rc` keeps each
  unknown's string once made.
- `oracle.rs`: `FactOracle`. `get` gives an int, "not held" or "timer". `tracks` is model.py's family rule.
  The prefix listings (`keys`) are built once per oracle. Their order is free: every model.py caller
  sorts them, or reads them in a way the order does not show.
- `path.rs`: `Frame`, `Call`, `Step`, `Entry`, `Path`, `Run`, with every model.py field. `Sig` is a join
  signature: one vector of words (each field in a fixed place, lists with their lengths, frozensets sorted) and a
  hash of them that equality tests first; a round's start is `Sig::round(round, position key)`. `Store`, the writes
  map, keeps an order-free hash of its entries as they are written. `Locals`, a frame's locals, are shared by the
  frame's copies until one writes. `Rw` (the turn's read and write log: one flat map of (owner, key) -> flags) and
  `Writer` are shared by forks until one adds to them. `History` (a round start's state hash and writes) is a
  persistent list that forks share; `round_start` and `immediate` are `Rc<Store>` snapshots. Strings that paths
  share are `Rc<str>`; settle's rule strings are `&'static str` (all of model.py's are literals). `Run.vals` holds
  the run's unknowns (the paths' values index it). `Call.cid` is the call's identity (model.py's `id(steps)`).
  `Step` has row 1's steps: write, note, resume, sentence, script, object, hook, room1, scroll_note,
  cut_index; row 2's: scene, scene_begin, room_code, scene_kill, scene_load, scene_ego; row 3's: turn, sentences;
  row 4's: input.
- `plist.rs`: `PList`, the list type of a path's assumptions, unmodelled, notes, unknown writes, errors,
  turns and conflicts. Frozen chunks of 64 items are shared by forks; a fork copies the short tail only.
  `add_branch` is `_merge`'s list rule: the common prefix is found chunk by chunk by pointer, then item by
  item by value (strings by pointer first: `FastEq`). `add_branch_new` is settle's rule: an item the list holds
  already is not added (a short list is scanned, a long one goes into a set).
- `machine.rs`: `Machine`. `run_entry`, `run`, `_drive` (joins, waiting groups in insertion order,
  merges, spins, the path limit counting waiting paths), `_advance`, `_step`, `_jump`, `_wait`, `_start`
  (row 0 and calls), every `_op_*` rule on its row-0 and calls branches, `read`, `write`, `val`, `test`,
  `getter`, `_getter_on`, `_owned_by`, `where`, `target`, `put`, `assign`, `_arith_at`, `clear`,
  `stopped`, `may_stop`, `started`, `hook`, `_inventory_hook`, `_room_change`, `_end`, `_done`, `_halt`,
  `_error`, `_note`; settle's branches of `_jump` (`time_jump`: time loops and the camera exit) and `_wait`. Also
  `predict` and `feasible`. Per run it keeps the oracle's unknowns, getter unknowns and unknown locals by key
  (`ocache`), and per machine each condition's text (`conds`): made once instead of at every read or fork.
- `machine/calls.rs` (row 1): `run_sentence`, `_join`, `_spins`, `_merge`, `_call_step`, the `_eng_*`
  steps (`script`, `object`, `hook`, `resume`, `sentence`, `room1`, `scroll_note`, `cut_index`, `write`,
  `note`), `_sentence_slots`, `_sentence_waits`, `_push_call`, `_push_frame`, `_finish`,
  `_stop_script`, `_kill_object`, `_live_slots`, `_count`, `_set_freeze`, `_freeze`, `_unfreeze`,
  `_room_slot`, `_gone`, `_find_inventory`, `_scene_steps`.
- `machine/rooms.rs` (row 2): `_eng_scene`, `_eng_scene_begin`, `_eng_room_code`, `_kill_room` (the
  scene_kill step), `_eng_scene_load`, `_eng_scene_ego`. loadRoomWithEgo's room_change branch is in
  `machine.rs` (`op`).
- `machine/settle.rs` (row 3): `run_wait`, `run_sentence`'s settle steps (`sentence_frame_steps`), `_pre_slots`,
  `_slots_sig` (`slots_sig_short`, `slots_hash`), `_position_key`, `_state_key`, `_changed`, `_unsettle`,
  `_next_round`, `_round_order`, `_round_end`, `_round_conflicts`, `_loosen`, `_close_settle`, `_unit_named`,
  `_slot_frame`, `_changed_any`, `_pre_rule`, `_origin`, `_skip`, `_time_wait`, `_eng_turn`, `_eng_sentences`,
  `_sentence_busy`. The settle branches of the other methods are in place: `_drive` (`_close_settle`), `_join`
  (the round's signature and the settle part), `_merge` (kept slots, the de-duplicated lists, `after`, `rw`,
  `ran_by`, `writer`, `ambient_vals`, the round start's counts and snapshots), `read` (TIME_VARS), `_finish` (kept
  slots, `multi`), `_eng_script` (second instances), `_eng_resume`, `_eng_sentence` (the queue), `_push_frame`
  (didexec), `_op_doSentence`, `_freeze` and `_unfreeze` (the queue's counts), `_call_step` (the next round),
  endCutscene's `armed` in a settle turn.
- `machine/dialog.rs` (row 4): `run_verb`, `_eng_input` (`eng_input`, the input step, with switch input_after_turns)
  and `_merge`'s `input_drift` (`merged_drift`, called from `calls.rs` `merge`). The input event is `ev_input`.
  `VERB_CLICK_AREA` and `LEFT_CLICK` are model.py's constants. `Vals::py_repr` is Python's `repr` of a value (the
  drift sort needs it).
- `machine/esc.rs` (row 5): `run_esc`, `_eng_abort` (`eng_abort`; golden.py's abort event after it), `_eng_override`
  (`eng_override`), `_eng_key` (`eng_key`) and `_key_route` (`key_route`; golden.py's key event). `KEY_CLICK_AREA`,
  `MBS_MAX_KEY`, `KEYCODE_RETURN`, `KEYCODE_TAB` and `KEYCODE_ESCAPE` are model.py's constants. `Step` has row 5's
  steps: `Abort { armed, code }`, `Override { name, w }`, `Key(code)`.
- `heads.rs`: model.py's HEADS effects for op and start names.
- `enc.rs`: golden.py's canonical encoding (`enc`, the order rule, `path_dict`, `run_dict`, `_where`),
  step tuples included. `path_with(.., turns: false)` leaves the turns out, for machine-serve's wire.
- `trace.rs`: golden.py's TraceMachine events, with path ids given in first-event order. Lines stream to a
  sink (a gzip file in the runner).
- `harness.rs`: the ground truth's pre states (groundtruth.py `load`), the jobs file, check.py's
  `run_model` dispatch, and a move's lines.
- `bin/machine_run.rs`, `bin/machine_cases.rs`, `candidate.py`, `wide.py`, `tests/golden.rs`.
  machine-cases applies wide.py's synthetic state changes (`set_prefix`, `sets`, `unheld` in cases.json) and its
  switches, runs `click` cases with `run_verb` and `esc` cases with `run_esc` (an `arm` case on its own state), and
  with `--costs` writes the Rust costs per case (above).
  `FactOracle::with_unheld` serves wide.py's `--unheld` only (an empty list costs one test per read).
- `bin/machine_serve.rs`, `../../check_rust.py`, `tests/check_rust.rs`: check.py's model check on the Rust machine
  (section "The model check on Rust").
- Tools (Python, the experiment's venv): `survey.py` (what a dump reached), `survey3.py` (settle's counts, and
  `--cover` to pick test moves), `setorder.py` (getters whose model.py result rests on set order, per run),
  `orderdeps.py` (the same, plus endCutscene's reads in a settle turn, on worker processes), `profile.sh` (where
  the Rust time goes), `survey4.py` (dialog's counts), `survey5.py` (esc's counts). wide.py has `--row 3`, `--waits`, `--no-objects` and `--rows
  all`; the row-4 stage added `--row 4`, `--clicks`, `--switches`, `--unheld`, `--plan`, `--from-costs`,
  `--max-rust-ms`, `--max-items`, and a progress line every 200 cases; the row-5 stage `--row 5`, `--escs`, `--arm` and
  `--keep-armed`.

Trace events emitted: start, fork, merge, push, finish, end, halt, error, sentence, settle's turn, skip, round,
settled, queue, dialog's input, and esc's abort and key: all of golden.py's. The fork event is written after `_advance`
returns and before the path-limit cut, as in golden.py.

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

Row 3 (settle):

42. model.py's state hashes are Python `hash()` values it compares as ints (`_spins`' states, `_position_key`,
    `_state_key`, `round_slots`). Python's `hash(-1)` is -2, so two states that differ only by a value -1
    against -2 (a local, a fact, a queue entry) hash alike and count as one: a spin, a merge at a round's start,
    a cycle, a fixed point with no moving locals. `Val`'s hash does the same (`value::py_int`). Equality (the
    waiting groups, merges' joins) still tells them apart, as Python's `==` does. Not reproduced: an unknown whose
    value set differs only by -1 against -2 (Python's frozenset hashes alike; the port's ids differ).
43. The camera exit's fork: the side that leaves is forked before the exit is recorded, so only the side that
    stays holds `camera_done` (that loop's later tests stay without a fork on it).
44. A time-only loop is taken to exit once per loop head and turn: `time_loops` comes from the slot's wait point
    at the turn's start (`time_loops_at`), and the exit removes its head. A known test value decides as usual.
45. In the click's frame (round 0) TIME_VARS keep pre's values; a time-only wait held at pre is skipped ("held in
    the click's frame"); the mouse variables are written first as R_INPUT unknowns. A slot of the sentence script's
    number whose fact is live but not 1 (a resume step) is dropped from the frame: the turns come from the slots
    live at pre (fact 1).
46. `_eng_resume` does not reset `owner` first: the previous turn's slot still logs the room and `at:` reads, and
    the delay fork's paused side keeps that owner. `_eng_sentence` resets owner and `timed`; `_eng_sentences`
    resets owner only, so a queued sentence runs with the last turn's `timed`.
47. `_skip` with an empty why (a dead slot, room code not running) records nothing, but golden.py's skip event
    is still written, with why "" and origin null. A dead slot's kept Frame is dropped there. A slot that ran in
    the frame already gets no skip: golden.py writes a turn event with ran false, why didexec.
48. `_pre_rule`'s why for a pre wait point of kind start or other is "its wait point is a start" / "... a other".
49. `_finish` keeps the frame as the kept slot with its freeze count, sid, override and origin (pc at the resume
    offset, killed None, not nested, no time loops); `_eng_turn` runs a copy with freeze 0. A WaitFor turn starts
    past the wait (its `next`, which may be None).
50. `_round_order`: the slots live at pre in their key order, then the other kept slots by sid (then name).
51. `_merge` with settle: the lists other than turns take a branch's new items only when the merged path does not
    hold them; the test is made for all of a branch's new items before any is added, so two equal new items of one
    branch both go in. `ambient_vals` is extended item by item (a repeat within one branch goes in once).
    `reentries` is the most, not the sum. `writer` becomes "mixed" where the rules differ. `same` (value merges)
    compares the stores and the kept slots as dicts. At a round's start the merged path's `round_start`,
    `round_slots` and last history entry are made again from the merged state.
52. `_unsettle` and `_loosen` write straight into the store (no read or write log) and set `writer` to "not
    settled". A set of one value stays an unknown with that one value. `_close_settle` loosens the ambient values
    first, then each capped or two-instance slot's `at:` keys to {0, 1}; a two-instance slot that is dead gets its
    slot fact {0, 1} too.
53. doSentence with settle: STOP empties the queue (and in round 0 sets `cleared`); a sentence with an unknown verb
    or object is an unknown write whose text is Python's list repr of the values' strings, and is not queued.
    `_eng_sentences` pops a sentence even when VAR_SENTENCE_SCRIPT is 0, and then runs nothing.
54. `run_wait` makes the first round before `_drive`: its round event comes before the start event in the trace.
55. The rule strings of `writer`, `ran_by` and `owner_rule` are model.py's literals; `rw` logs a read only when the
    owner has not written the key, and an owner added by `setdefault` with no key still dumps as two empty sets.

Row 4 (dialog):

56. `run_verb`'s call is ("click", "scumm.cpp:3177-3255"), caller None, inactive. Its steps: the four mouse writes, one
    plain turn per slot live at pre (the sentence script's slot too: no resume step, no delay fork, no "live slots run in
    an order" note, unlike `run_sentence`), the input step, the sentences step. The path starts at round 0 with no
    `first_end` and no `immediate`: `_next_round` sets them when round 0 ends (first_end: the last turn's `how`, or
    "nothing ran").
57. The Run's unit and start come from VAR_VERB_SCRIPT at pre (the oracle, not the path): a global script, or the local
    script in the resource of `room` at pre (unit None when that resource is unknown). start is the unit's first
    instruction's cite, else "input script N": N 0, N unknown (the unknown's string), or a unit the game lacks.
58. `_eng_input` resets `owner` and `timed` first. The drift reads and the switch's reads are not logged, and the input
    script runs with no owner: its reads and writes go into no turn's `rw` and set no `writer` rule (no order
    conflicts with the turns). Its frame does not count as having run this round (`round_ran`; round 0, not timed).
59. Drift compares each key's value on the path (its store, else the oracle) with the oracle's pre value, as values
    (an unknown equals an equal unknown). A key a turn wrote back to its pre value is no drift. The keys are read in the
    order VAR_VERB_SCRIPT, VAR_USERPUT, `verb:V`.
60. With every switch off the click is always taken: N is VAR_VERB_SCRIPT at pre even when a turn changed it, and the
    click goes on when the turns turned input off or hid the verb. Only `input_drift` records it.
61. The input script's call ("runInputScript", "script.cpp:1478-1511") is pushed straight onto the stack, caller None,
    inactive, with one step ("script", N, False, False, {0: area, 1: verb, 2: button}). `_eng_script` then stops N's
    running slots first (R is False). N unknown: an unknown write ("runInputScript", the click's cite, "input script
    <N>"), no call. N 0: a note, no call.
62. Switch input_after_turns: VAR_USERPUT is read from the path and decided by `_decide` (`<= 0`; a value set whose
    values agree decides without a fork). Unknown: the path forks before either side's assumption; the new path takes
    side True ("_userPut <= 0: the click is dropped") and gets the note, and then still goes on to the click's next
    step (sentences) with no input script and no verb or N read; the old path takes side False, reads `verb:V` (not 1,
    unknown included: the "may not be shown" note) and N from the path. Known `<= 0`: the note, and the step returns:
    no verb read, no N read, no call.
63. golden.py's input event is written after `_eng_input` returns, on the path it was called with (its drift), also
    for a dropped click. The switch's fork event follows it (the drive loop writes it).
64. `_merge`: `input_drift` becomes the union of the members' entries, sorted by Python's `str` of each tuple (the
    reprs of key, pre value and value). A merge of paths that all hold none leaves it empty. One path's own entries come
    back in the same order ('var:32' < 'var:53' < 'verb:'), so the sort shows only when a key has two values.

Row 5 (esc):

65. `run_esc`'s call is ("Esc", "scumm.cpp:3177-3255"), caller None, inactive. Its steps: ("abort", armed, code), one
    step per slot live at pre (the override step for the rewound slot, a plain turn for the others), an override step
    at the end when the rewound slot is not live at pre, ("key", code), ("sentences",). No mouse writes and no notes
    (unlike `run_verb` and `run_sentence`). The path starts at round 0 with no `first_end` and no `immediate`, as for a
    click.
66. `armed` is every `esc:` key of the fact map whose oracle value is 1 (a key of a family not held is not 1), sorted as
    strings. The step and the abort event keep all of them; abortCutscene takes the first and notes "N esc: facts are
    1; the first is taken". `code` is VAR_CUTSCENEEXIT_KEY from the oracle (an unknown when not held).
67. Only with a known code that is not 0 is there a rewound slot. Otherwise the Run's start is "Esc: no override armed
    at the top level" and its unit None even when an override is armed, and every slot gets a plain turn. With a
    rewound slot, the unit comes from `_unit_named` on a fresh path (the oracle's room for a local or the room's code)
    and the start is "Esc: abortCutscene rewinds <name> to <cite>", or "... to [W]" when W is no instruction of the
    unit or the unit is not known or missing.
68. The override step dumps W as a string (model.py splits it from the key); the `at:` key it writes holds the int.
69. `_eng_abort` resets owner and `timed`, so nothing it reads or writes is logged. With a rewound slot it writes, in
    order: the slot's `at:` keys to 0 (clear), `at:<name>:W` = 1, the freeze count (0, exact) with `frozen:<name>` = 0
    only when the path does not read 0 already, a note at "script.cpp:1703-1704" (head "assumption: cutsceneOverride"),
    VAR_OVERRIDE = 1, the `esc:` key = 0. The cutscene depth and the slot fact are not written.
70. `_eng_override` falls back to `_eng_turn` (settle's own rule and its trace turn event) when `at:<name>:W` is not 1
    on the path or the slot already ran this round. Its first reads (the `at:` key, the room for a local or room code,
    the slot fact, `frozen:`) come before the owner is set, so they are not logged; the `owner:O` and room reads after
    it are logged under the slot.
71. Its skip: "its override point is no instruction of its unit" (also for a unit not known) is tested first, then
    "its liveness <v> or freeze <v> changed before its turn" with Python's `str` of both values (an unknown liveness is
    not 1). Entry and exit code count as live.
72. The rewound slot runs with `timed` False (settle's turns are timed), rule "a: rewound by Esc (abortCutscene)"
    (`owner_rule`, and `ran_by` when the slot has none yet), `round_ran` set by the step itself (`_push_frame` does not
    set it, not timed in round 0). The frame: pc at W, no locals, `args_known` False, `owned` from `owner:O` (an unknown
    is kept), R None, F from the start sites ("<name>: F flag not known" at the Esc call's cite when None; entry and
    exit code False), not nested. Its `at:` keys are cleared under the slot's owner, then `_push_frame` writes the slot
    fact 1 (none for entry and exit code).
73. `_eng_key` resets owner and `timed`: its reads of VAR_USERPUT and VAR_VERB_SCRIPT from the path are not logged.
    The cursor state for Enter and Tab is read from the oracle (pre) with the switch on or off.
74. A code of 0 becomes 27 (Esc's ASCII code) in `_eng_key` only: the steps keep 0, abortCutscene does not run (a
    note), and the key still reaches checkExecVerbs, which runs the input script with [4, 27, 1].
75. `Path.cleared` True: the note and the route "wiped by clearClickedStatus", and nothing else runs (no code test).
    None (merged paths that disagree): the path forks before either side's assumption; the new path takes side True
    ("clearClickedStatus ran in the frame"), gets the note and its route; the old one takes side False and goes on. The
    why is the text of `Unknown(R_BRANCH, "whether a doSentence(STOP) ran in the frame")`, made as text (no unknown in
    the run's values).
76. An unknown VAR_USERPUT forks as dialog's switch does (`_decide` on `<= 0`; a value set whose values agree decides
    without a fork): the new path takes side True, the note and the route "dropped with input off"; the old one goes
    on. Forks of one step all go into the step's result, in order. An unknown input script is an unknown write and a
    note, and its route "input script not known".
77. A route is kept when the key is taken, with a copy of the path's assumptions then (`Run.key_routes`), so a path
    merged later still counts once per way it carries. golden.py's key event on a fork gives the fork its trace id
    before the fork event names it. A fork the path limit cuts drops the routes of that step, both sides
    (`del self._key_routes[routes:]`).
78. Switch esc_key_at_pre: VAR_USERPUT and VAR_VERB_SCRIPT come from the oracle (pre), not from the path. It changes 13
    of the 22 Esc moves.
79. The input script's call is dialog's: ("runInputScript", "script.cpp:1478-1511"), caller None, inactive, one step
    ("script", N, False, False, {0: 4, 1: code, 2: 1}), with the code after the 0-to-27 change.

Kept apart (decided where a row reached them):

- `getter` on value sets: model.py takes the first unknown of a Python set (`next(v for v in out ...)`),
  which is hash order. With PYTHONHASHSEED random (the goldens' setting), that is not deterministic when
  two different unknowns are in the set. The port takes the first in combination order. Measured with
  `setorder.py` (a subclass of Machine) on the goldens' moves: row 1 runs 576 getters on value sets
  (getVerbEntryPoint), row 2 runs 10,862, row 3 146,428 (`orderdeps.py row3`); none has two different
  unknowns among its results; row 4 below. Check row 5 the same way. The synthetic owners-not-held states reach it
  (474 of 4,211 cases at row 120): there, model.py gives either unknown from one process to the next (one case,
  run under PYTHONHASHSEED 0 to 5: owner:16, 17, 17, 16, 17, 16). No port can match that, so wide.py's `Machine`
  notes such a getter and drops the case ("hash_order"). If the goldens of a later row reach it, the golden
  itself is one draw: decide with the PI (for example, fix model.py to a sorted order in a new oracle version).
- `endCutscene`'s `armed` (calls on) reads the `esc:` keys of a Python set until the first 1. Without a read
  log the order changes nothing. In a settle turn the keys read go into the turn's `rw`, so which keys are
  logged rests on Python's set order when a key is 1 and another key not yet logged (or a second 1) could come
  before it. Decision taken here: the port reads the keys in key (string) order. `orderdeps.py row3` finds 5,464
  endCutscenes in settle turns in the goldens; 2 rest on the order, both in S3#47 (global/script-121.txt:188).
  The golden matches either order: a build that read them in reverse order gave the same files (the reads do not
  reach the output). wide.py drops a case that reaches it ("hash_order"; 1 of the 250 waits, the same site); kept
  with `--keep-order-cases`, that case is equal too, traces included.
- `_merge`'s `input_drift` sort (row 4) uses Python's `str` of each (key, pre, now) tuple. An unknown with a value set
  prints its frozenset in CPython's hash-table order. That order is ascending for small non-negative ints, and
  otherwise rests on how the set was built; `Vals::py_repr` prints ascending. It shows only when one key has two
  different unknown values with value sets among the merged paths, which no check reaches (the goldens' drift is
  ("var:53", 1, 0) only).
- `endCutscene`'s reads, row 4: `orderdeps.py row4 --only <the 60 dialog moves>` (also for both switch runs) finds 182
  endCutscenes in settle turns and 1 whose logged reads rest on the order (S1#23, room 51's local 207 at line 469).
  A build reading the keys in reverse order gave the same files for S1#23 and S3#47, so it does not show. Getters
  on value sets: 1,352 (getVerbEntryPoint), none ambiguous. Row 4's other 179 moves give row 3's files.
- Row 5: `orderdeps.py row5 --only <the 22 Esc moves>` finds 1,374 getters on value sets (getVerbEntryPoint), none
  ambiguous, and 56 endCutscenes in settle turns, none whose logged reads rest on the order; under
  `row5-esc_key_at_pre`, 1,278 and 48, none. Row 5's other 217 moves give row 4's files. The wide dumps drop cases on
  set order only where two `esc:` keys are 1 or an endCutscene meets the rule above: 1 of 10 cases with code 0, 1 of
  10 with the code not held, 16 of 34 with two armed keys (`--keep-armed`). Kept with `--keep-order-cases`, the 34
  armed cases are equal too, traces included (one Python process, one hash seed).

## Notes for the next stage

The machine is ported: all seven features, all nine golden runs at 0 differences. What a later stage can do here:

- Keep the gate. After any change to shared code, run the whole suite with the ignored tests (`-- --include-ignored
  --test-threads 2`): every `rowN_full` and every wide test.
- Speed (see Speed): the merge's join of every write key, `prefixed()`'s scan of every write, the drop of merged paths,
  and trace events built as JSON per event. Row 5 adds no time of its own: the Esc frame is one more click-like frame.
- The decisions still open with the PI ("Kept apart"): the getter's first unknown of a value set and endCutscene's
  `esc:` reads in a settle turn both rest on Python's set order. No golden of any row rests on either; synthetic
  states do (the owners-not-held states, two armed keys).
- check.py's scoring stays in Python (`candidate.py check`, `check_rust.py`), because it names the route's own moves.
- The model check on Rust (`check_rust.py`): a row's time is now the Rust machine (S2#21 and S2#29 are a third of
  it), Python's decoding of the lines, and Python's own loading and scoring. Next steps, by size: the machine's
  speed candidates above; building the Run's Python objects in Rust (pyo3) instead of decoding lines; scoring in
  Rust, which needs the route's data (run B's VIA moves) passed in as data, not named in the Rust.

## Notes from the row-5 stage (esc)

- The port of esc matched on its first build: the 22 Esc moves of both runs, then all 239 moves of all nine runs. As at
  row 4, most of the work was the checks beyond the goldens, since the goldens reach one key code (27, the ground
  truth's only VAR_CUTSCENEEXIT_KEY) and one armed key at a time.
- The code is in `machine/esc.rs`; the three new `Step` variants, their encoding (`enc::step`), the `eng` arms in
  `calls.rs`, `harness::run_model`'s skip arm, and "esc" in `PORTED` are the only changes to shared code. The pieces
  row 4 had put in place (`Machine.key_routes` and its cut-back, `Path.cleared` and its merge, `Switches.esc_key_at_pre`)
  needed no change.
- wide.py's `--arm` makes a state per case; machine-cases builds that case's oracle on its own (the other cases share
  one oracle per row).

## Notes from the row-4 stage (dialog)

- The port of dialog matched on its first build: all 60 dialog moves, then all 239 moves of the three row-4 runs.
  Most of the work was the checks beyond the goldens (above), because the goldens reach little of `_eng_input`.
- Switch const_polls had been wired at row 3 (every loop, wait point, time exit and time-loop lookup passes
  `switches.const_polls` to scumm-ir's analyses); its run now compares clean. It changes 4 of the 239 moves.
- Speed candidates are unchanged (row 4 adds little time of its own; see Speed): the merge's join of every write key
  (a store shared between forks would let a merge look at the keys written since the fork only; a two-level store
  was tried and made reads and full scans slower); `prefixed()` scans every write; the drop of merged paths; trace
  events build JSON values per event.

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
| row 2's 232 moves, after this stage's speed changes (load about 2) | | 0.93 s | |
| row 3's 232 moves with an action (2026-10-25; Python nice 10 at load 4-6, Rust load about 2) | 1,247 s | 26.1 to 26.5 s | about 47x |
| row 3: S2#29, S2#21, S1#83 | 534, 423, 113 s | 9.1, 9.3, 2.9 s | about 59x, 46x, 40x |
| row 3: the other 229 moves | 177 s | 5.3 s | about 33x |
| row 3: 249 waits, traced (wide.py dump against machine-cases) | 110 s | 3.0 s | about 37x |
| row 3: 2,341 sentences from row 120, traced | 198 s | 7.7 s | about 26x |
| row 4's 232 moves (2026-10-25, load 1 to 2; Python nice 10) | 887 s | 19.2 to 25.7 s | about 35-46x |
| row 4: S2#21, S2#29, S1#83 (Rust: the 25.7 s run, back to back with Python) | 307, 285, 94 s | 7.75, 8.3, 2.35 s | about 40x, 34x, 40x |
| row 4: the 60 dialog moves | 68.5 s | 2.2 to 2.9 s | about 24-31x |
| row 4: S2#25, the slowest dialog move | 14.9 s | 0.43 to 0.65 s | about 23-34x |
| row4-const_polls, row4-input_after_turns: the 60 dialog moves | 70.6, 70.9 s | 2.3, 2.4 s | about 30x |
| row4-const_polls, row4-input_after_turns: all 232 moves (Rust alone) | | 20.8, 21.6 s | |
| row 4: 2,740 clicks, traced (wide.py dump against machine-cases; load 5 to 7) | 557 s | 11.4 s | about 49x |
| the same, 2,741 clicks, at load about 9 (the test suite running) | 595 s | 28.6 s | about 21x |
| row 5: the 22 Esc moves (2026-10-25, load 3 to 5; Python second pass, Rust mean of 5) | 2.57 s | 0.076 s | about 34x |
| row5-esc_key_at_pre: the 22 Esc moves | 1.98 s | 0.065 s | about 30x |
| row 5: S3#46, the slowest Esc move | 0.97 s | 0.033 s | about 29x |
| row 5's 232 moves, row5-esc_key_at_pre's (Rust alone, load about 2.5) | | 27.0 s, 27.5 s | |
| row 5: S2#29, S2#21, S1#83 (Python one pass, load 2 to 3; Rust the 27.0 s run) | 332, 336, 105 s | 8.44, 8.25, 2.63 s | about 39x, 41x, 40x |
| row 5: 247 Esc keys from every state, traced (wide.py dump against machine-cases) | 91 s | 2.5 s | about 36x |

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

Row 3 (2026-10-25). The goldens' Python run (traced) took 586 and 591 s for S2#21 and S2#29; untraced, 423 and 534 s.
S2#21 is merges at scale inside the click's frame: 4.9 million steps, 220,221 forks, 85,191 merges folding 220,177
paths, 1.68 million waits at joins, 754,398 trace events. The first Rust build took 17.8 s on it, untraced, one
thread. Timers around the drive loop's parts then showed the time spread over the merge (4.5 s), the steps (3.8 s),
the join signatures (2.6 s), forks (0.8 s) and the waiting groups (0.5 s). The changes, each checked on rows 0-3's
goldens and the wide dumps:

- `_merge` takes the first path itself as the merged one (model.py forks it and drops the original), works out
  what it reads from it first, skips frames whose locals are equal in every path, and finds a later path's new
  keys by one lookup in the first path's store.
- A resumed path (just merged at a join) reuses the signature it merged at instead of building it again (model.py
  rebuilds it; a merge changes nothing it holds; a debug build asserts the two are equal).
- `Sig` is one vector of words: one allocation per join visit instead of six.
- `Rw` is one flat map shared by forks until a new key is logged, and `Writer` is shared the same way; `Locals`
  are shared by a frame's copies. Rule strings are `&'static str`.
- `PList` walks its chunk chains without building vectors, and compares strings by pointer first.
- The waiting groups have an index by signature hash.
- The oracle's unknowns, getter unknowns, unknown locals, condition texts and unknowns' strings are made once.
- Trace files use the fastest gzip level (the trace of S2#21 is 160 MB of text): S2#21 traced went from 17 s to
  11 s.

S2#21 went from 17.8 s to 8.5 to 9.3 s untraced (the spread is the laptop's other jobs). Tried and dropped: a
two-level store (a shared base map and a small top of recent writes, so that a fork copies the top and a merge
reads the tops' keys only). Reads then cost two lookups and full scans (`prefixed()`, which scans every write)
filtered each base entry against the top: the steps got 40% slower, and merges found few shared bases.

Peak memory: the runner on S2#21 and S2#29, one thread, 68 MB in all (game and ground truth included); Python's
time_run on row 3, 207 MB; the goldens' traced worker on S2#21, 173 MB. The runner on all of row 3 with four
threads, traced, peaks at 540 MB: that is the encoding of the biggest moves' path lines (S1#8 dumps 131 MB of
JSON, S1#12 68 MB), not the machine.

Row 5 (2026-10-25). Esc adds little time: the 22 Esc moves are 2.6 s of Python and 0.08 s of Rust; row 5's other 217
moves are row 4's (the same files), so row 5's total is row 4's plus that. Peak memory: the runner on row 5, one thread
untraced, 320 MB (the encoding of the biggest moves' path lines, as at rows 3 and 4); four threads traced, 539 to 553 MB;
machine-cases on the Esc dumps, 65 to 123 MB.

Row 4 (2026-10-25). Dialog adds little time: the 60 dialog moves are 68.5 s of Python's 887 s and 2.2 to 2.9 s of Rust's
19 to 26 s. The rest is row 3's settle work, which this stage did not change. The spread between Rust runs is the
laptop's other jobs (load 1 against 2). Peak memory: the runner on row 4, one thread untraced, 321 MB, and four threads
traced 581 MB (both mostly the encoding of the biggest moves' path lines, as at row 3); Python's time_run on all of row
4, 207 MB, on the dialog moves 130 MB; machine-cases on 2,740 clicks traced, 116 MB.
