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

- `DIR/<key>/`, one folder per whole-reading key: sha256 over the reader binary's bytes, the game index
  (`data/scripts-dos-cd/index.json`) and a canonical text of item 3. Readings of the same scope and switches from
  any entry share a folder.
- Each reading appends the runs it made (not those it took) to its own file in the folder, written while it
  reads, and renamed into place when the reading ends; an unfinished reading leaves no file. A run stores the job
  and guides as bytes, the consulted keys as text with kind, domain and constant start, the list answers, the reach
  questions, and its output. Keys are text, so a store is valid across processes.
- A reading opens every finished file of its folder, mapped, and indexes the runs by a hash of job and guides;
  candidates are compared byte for byte. Its own runs join the index between passes.
- Space: a reading writes no store when `/home` is under 8 GB free plus twice its folder's largest file.

## The check

Byte for byte, with and without the switch, on the same inputs:

- refinements [103, 194]: j00089's with the store j00000's left, and j00000's with j00089's;
- fresh ring(33) readings: j00119's with boot's store, and boot's with j00119's;
- each also against itself (a store from the same inputs).

The packed file, the golden body and counters, and the reached file have the same sha1 with the switch on and off.
