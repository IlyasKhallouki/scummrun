# The job memo across readings (`--memo DIR`)

E011's readcore item 3 (plan.md, "readcore"; profile-reader section 7, change 3). Proof before the code, for the
reader as E009 runs it. Off by default.

## The claim

A reading B may take a job's output from a run made in another reading A (of another entry, or a refinement with
other starts), or from its own earlier pass, without running it, when five conditions hold. The reading's files and
counters are then byte for byte those of B without the switch.

This is `experiments/E004-fast-exact/pass2-reuse.md` carried from one reading's two passes to many readings. Its
induction is kept. Its third condition, "no widened key was consulted", becomes "every consulted key reads the same
in both readings", because between readings more than the widened domains can differ: the start, and with it some
domains, kinds and constants.

## The settings it covers

The switch is accepted only with `--reach`, `--split-limit` and N >= 1 worker threads, and without `--pass2-reuse`,
`shared_runs`, `havoc_cut` (and so `step_limit`), `grow_limit` and `due_first`. E009's readings
(`--reach --split-limit --live-locals --elsewhere --frame-costs` with `value_sets, draw_clears, actor_rooms,
input_settled, wait_pass_cost`, refinements adding `--fact-vars` and `--reach-also`) are in it. Any other combination
is refused by name: the audit below was made for these code paths only.

## What one run is

A job is run by a worker's Builder (`Builder::run_job`, par.rs `worker`). Its output is:

- the records it returns, in order;
- what it added to the worker Builder's counters and sets (`Deltas`: counts, ends, drops, non-fact checks, guesses,
  unknown writes and sites, values out of domain, places outside the points, the Reader's counters, the lists'
  texts by number, the `args:` values its records set, the branches it held back);
- the pins of its last machine run (`last_pins`).

The worker's counters are empty when a job starts (`take_deltas` after each), and nothing else of the worker's
state reaches a later job: the machine resets its values and oracle cache after every run (`finish_run`), its fact
information is a function of the table (`St.info`, cleared when the table changes), and the Builder's caches are
functions of the game, the scope or the table. The `args:` values a job's records add to the worker's table are
read by no run under `--reach` (below). That is why the reading is the same for every thread count and every
schedule (par.rs); the same fact is what lets a run's output be carried to another reading.

## What one run reads

Each item names where the code reads it.

1. **The job**: kind, label, pins, precondition, what it runs, its checks. Compared whole.
2. **Its guides** under `--reach` (`Reached::guides_of`, the machine's `St.guides`, `cur_guides`). Compared whole;
   none and none are equal.
3. **The whole reading's fixed inputs**, equal by the store's key (below): the reader's code; the game; the scope
   (rooms, units, objects, verbs, dialog verbs, walk verb, universe, write sets, the machine switches F5 and F9);
   the Builder's switches and `sig_limit`; the dumped slots (`--slot-locals`); and the table's parts that are not
   per key: the wait points (`points`, from the universe and the start's `at:`/`esc:` keys: `pos:`/`escpos:`
   domains, `is_point`, `place`), the engine variables, `elsewhere`, live locals on or off, `value_sets`,
   `split_limit`, `wait_both`, `--reach` on, the refined variables (`refined`, from `--fact-vars`), and the F1c rule
   (`settled`, which reads the start's room and var 32).
4. **Per key it consults: its kind, its domain, and its start value if it is a constant.**
   - The machine half reads a key's kind, domain and start only through `info()` (machine.rs), which logs the key.
     It reads the start only for a constant (`read_fact`, `oget`: `Kind::Const => info.start`); a fact's value comes
     from the domain, the precondition or the pins, a non-fact's is an unknown named by its key.
   - The Builder reads them in `record` (the precondition's keys: `table.domain`), `effect_or_same` (each written
     key, and the source of a `Cond` or `Copy` value: `fact_info`, and the start of a constant source), `stepped`,
     `run_split` (the first fork's key) and `positions` (`pos:`/`escpos:` domains: item 3). Each is logged.
   - `args:` keys are left out of the check. Under `--reach` no run's output depends on an `args:` domain: the
     machine reads no `args:` key; `record` and `reach_path` keep a path's `args:` set whatever the domain;
     `record_action` reads a domain only for an `add` effect, whose key is a written fact; `Reached` takes an
     `args:` key's default as 0 without its domain. So the values a job adds to its worker's `args:` domains reach
     no later run either.
   - A key's kind is a function of the scope and its domain (`FactTable::kind`), so comparing kind and domain is
     comparing what the table answers.
5. **Whether each list number it asked about is folded** (`record` asks `self.folded`; the reading's folded lists
   at the batch's start). Each number is logged with the answer.
6. **The reached values it asked about** under `--reach`: `meets` in the machine's forks (`reach_groups`),
   `first_unmet` and `was_met` in `reach_path`, `reach_record`, `reach_parts`. Each question is logged with its
   answer; `Reached::answers_hold` asks them again.

Nothing else reaches a run: the reach rounds, the event lists, the job order and the sink are the reading's thread's,
outside the run.

## The conditions

Reading B takes the output stored for a run of reading A when:

1. **Whole-reading inputs equal**: A and B have the same store key (item 3).
2. **Same job and guides**: the stored job and guides are byte for byte B's (items 1, 2).
3. **Same consulted keys**: every key A's run consulted, but `args:` keys, has in B the kind, the domain and, for a
   constant, the start value it had in A (item 4).
4. **Same list answers**: every list number A's run asked about is folded in B as it was in A (item 5).
5. **Same reach answers**: every question A's run asked gets the same answer in B, at B's round (item 6).

Under these, the run's inputs in B are its inputs in A, item for item, so the deterministic machine and Builder give
the same records, deltas and last pins.

## The induction

As pass2-reuse.md's. B's jobs run in B's order, which is a function of B's earlier outputs. Suppose every job of B
before job j gave the output B gives without the switch, whether it ran or was taken. Then B's state when j starts
(the table, the folded lists, the reached values, the guides, the job itself) is that state without the switch. If
j is taken, conditions 1-5 hold against that state, so the stored output is the output j's run would give; if j
runs, it is. So j's output is B's without the switch, and by induction every output is. The merge into the reading's
Builder (`merge_job`), the stream to the sink and the reach rounds then see the same outputs in the same order: the
files and counters are byte for byte the same.

A's runs stored in B's own earlier pass count the same way: pass 2 of B takes a pass-1 run when its consulted keys
did not widen (condition 3), which is pass2-reuse.md's case.

## The store

- `DIR/<key>/`, one folder per whole-reading key: the sha1 of a canonical text of item 3, which starts with the sha1
  of the reader binary's bytes and of the game index (`data/scripts-dos-cd/index.json`). The text is kept as
  `inputs.txt` in the folder. Readings of the same scope and switches from any entry share a folder.
- Each reading appends the runs it made (not those it took) to its own file in the folder, written while it
  reads, and renamed into place (`*.runs`) when the reading ends; an unfinished reading leaves no such file. A run
  stores the job and guides as bytes, the numbers of its consulted keys, the list answers, the reach questions, and
  its output, deflated. Each file holds a table of its consulted keys (text, kind, domain, constant start), each
  once. Keys are text, so a store is valid across processes.
- A reading indexes the runs of every finished file of its folder by a hash of job and guides, and reads a run's
  bytes when its job comes; candidates are compared byte for byte. Whether a consulted key reads the same is asked
  once per file, key and pass. Its own runs join the index between passes.
- Space, by E009's level: when the store's disk has less than 8 GB free plus twice the largest store file, a reading
  first deletes the files of the other folders, oldest first (a reading that opens a file renews its time), and
  writes no file of its own if that is not enough.

## The check

Byte for byte, with and without the switch, on the same inputs:

- refinements [103, 194]: j00089's with the store j00000's left, and j00000's with j00089's;
- fresh ring(33) readings: j00119's with boot's store, and boot's with j00119's;
- each also against itself (a store from the same inputs).

The packed file, the golden body and counters, and the reached file have the same sha1 with the switch on and off.

## Results

Ring(33) readings on E009's inputs (profile-reader's: rows j00089, j00119 and boot; refinements [103, 194] with
j00000's and j00089's starts), E009's command with the golden stream, 3 worker threads, on the laptop under the
heavy lock, one reading at a time (load 4-10). E009's binary (built from 2a3cf8a1) against e011-readcore's
(c797edc), the old and new reading back to back. Instructions and cycles are perf's (user space, all threads); the
CPU time follows the load, the instruction counts do not.

| reading | E009's binary | readcore, no store | readcore, `--memo` |
|---|---|---|---|
| fresh j00089, instructions | 708.7 G | 382.4 G (54%) | 152.9 G from boot's store (22%) |
| fresh j00089, CPU / wall | 263.0 s / 70.8 s | 174.4 s / 45.4 s | 63.0 s / 15.9 s (22%) |
| fresh j00119, CPU / wall | 246.8 s / 66.5 s | | 60.6 s / 15.5 s from boot's store (23%) |
| fresh j00089 without the golden stream, CPU / wall | 192.2 s / 60.3 s | 113.3 s / 34.3 s | 30.3 s / 12.4 s (20%) |
| refinement j00089, instructions | 289.7 G | 141.4 G (49%) | 95.9 G from j00000's store |
| refinement j00089, job-seconds / wall | 106.5 s / 44.3 s | 35.6 s / 15.6 s | 16.6 s (16%) / 10.5 s |

The packed file, the golden body, the Builder's line and the reached file have the same sha1 in every column. With
the branch's last commit (466a035) they are again E009's on every reading with the golden stream, with and without
the store, and on the j00000 refinement with the store against E009's binary's reading of it.

- **Taken and run.** The refinement j00089 from j00000's store took 31,136 runs and ran 3,190, each because a key
  it consulted reads otherwise from j00089's start. j00000's own reading took 11,249
  runs from its first pass (pass2-reuse.md's case) and ran 24,197. The fresh j00089 and j00119 readings took all
  39,515 runs from boot's store; boot's own reading took 16,673 from its first pass. j00000's reading again, from the
  store then holding its own, j00089's, boot's and j00119's files, took all 35,446 runs (7.4 s wall, 34.1 s with
  E009's binary). No run was refused for a list or a reach answer.
- **What a taken reading spends.** With every run taken, the jobs' part is 13 job-seconds (finding, reading,
  inflating and decoding the stored runs); the reading's 14.3 s of wall are the stream: the sink thread is busy
  12.1 s of them (golden lines and the packed file), and the reading's thread merges and streams the outputs for
  10.4 s. Without the golden stream the sink is busy 7.5 s of 10.9 s.
- **Store and memory.** A fresh ring(33) reading stores 22,842 runs in 445 MB, the j00000 refinement 24,197 in
  201 MB. A reading taking from boot's store peaks at 973-1,014 MB, against 526-554 MB for E009's binary; without
  the store readcore's reading peaks at 616-642 MB.

## Amendment 2026-11-03 (E012, component `resident`): the store in memory, the reading in fmi's process, `--prep`, the units as a set

Written before the code it covers (E012's plan.md, "Amendment 2026-11-03, component `resident`"). Four additions to
the claim above; conditions 1-5 and the induction are unchanged.

1. **The store in memory** (`MemStore`: `--memo DIR` with a resident store, or `--memo-mem`, the resident store
   alone). A file in memory holds the bytes a finished file on disk holds, in chunks, and a run's bytes are read from
   its chunk as from the file (`Blob`); its keys' and questions' tables are the file's. `take` checks conditions 2-5
   on those bytes as on a file's. A reading's file joins the store when the reading ends (`finish`), as a file on disk
   is renamed into place; until then only its own later passes index its runs (`index_own`), as on disk. So the store
   decides only where a stored run's bytes are, and the induction holds as it stands.
2. **The reading in another process's run** (fmi, `fmi_reading::inproc`). The reading is `run::run` with
   reader-scope's arguments: the code of reader-scope's main. What lives on from one reading to the next and could
   reach a run: the game (`Resident::game`, loaded from the game files and only read: `&Game`), the store (1), and the
   key's two sha1s (item 3: the running binary's, now fmi's, and the game index's, made once per process). The pool's
   workers with their machines and Builders, the reading's Builder and its table are made by each reading and dropped
   at its end. The allocator is reader-scope's (mimalloc).
3. **`--prep`.** A worker works out the sink's and the reading thread's part of a job's records after its run returns
   them, run or taken (par.rs `worker`), from the records and the worker's table for the batch, the same table the
   conditions are checked against. The store holds and gives only records, counters and last pins, as before. So a
   taken run's part is the part of the records the run would give.
4. **The units as a set** (`--memo-units-set`: the key's `units` line is the scope's unit keys sorted, in place of
   their order in `Scope::units`). Where the order of `Scope::units` is read:
   - **In a run.** The machine half reads the scope's `rooms`, `objects`, `dialog_verbs`, `universe` and `switches`
     (machine.rs, the copy made at a run's start), and the table's `kind` reads `writes`. The Builder
     reads `rooms`, `objects`, `verbs`, `dialog_verbs`, `walk_verb`, `universe`, `counts` and the unit set
     (`has_unit`, a hash set). Each of these is a line of item 3 by its value, or the set. The one read of `units`
     (`Builder::settled_rule`) makes the F1c rule once per table; its value is item 3's `settled` line.
   - **At the reading's setup.** `Scope::of_with` makes `writes` (item 3 by value), `universe` (sorted) and `counts`
     (by value) from the units. `FactTable::new` reads them for the hook variables', the room's and (switch
     actor_rooms) the actors' rooms' domains: sets filled by insertion, read by a run per key (condition 3).
   - **Outside runs.** The job list and its order follow the units' order; they are the reading's thread's ("Nothing
     else reaches a run"). LiveLocals' counts and the report read the units for counting only.
   - The switches that read `units` otherwise (havoc_cut and the havoc switches, due_first) are refused with the memo.

   So two readings whose scopes hold the same units in another order, and agree on every other line of item 3, give
   each run the same inputs that item 3 covers. With the key's `units` line as the sorted set, conditions 1-5 still
   give each run's inputs item for item, and the induction holds. Without the flag the key is as before.

The check (E012's plan.md, RS1-RS3): the readings with the store in memory, in fmi's process, with `--prep` and the
units as a set, against reader-scope's readings of the same arguments without the memo, byte for byte.
