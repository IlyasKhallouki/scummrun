# One run for jobs that differ only in an argument they never read (`shared_runs`)

Plan step A2. The claim and its proof come first, for any SCUMM v5 game. MI1 numbers appear only in the tests and the
logs. Off by default.

## The claim

A dialog line job clicks verb `v` with the verb script variable at `n` in room `R`. The engine runs script `n` with
the arguments (click area, `v`, button) (script.cpp:1478-1511), and the job's precondition pins `verb:v` to 1. Jobs
with the same `n`, `R` and precondition, apart from `verb:v`, often make the same run: script `n` never looks at its
second argument. With the switch, the first such run is made, and every other job of the batch takes its output with
`verb:v` renamed. The files and counters are byte for byte those of the reading without the switch.

## What one run depends on

The machine is deterministic (reach.md, fact 1). A run of the machine half (`Reader::run`) is a function of the table
and scope of the batch, the path limit, the job's pins, its precondition, the guides of its label (`--reach`), and
what it runs. For a click on a verb, what it runs is the verb number `v`. The machine uses `v` in two places only
(machine.rs `eng_input`):

1. it reads `verb:v`, which the job pins to 1;
2. it starts the input script with `v` as `Local[1]`.

Take two jobs `J` and `J'` that differ only in `v` and `v'`: their pins and preconditions are equal once `verb:v` and
`verb:v'` are taken out, both pin their verb to 1, and their guides are equal with the verb renamed. Let σ rename
`verb:v` to `verb:v'`. Then the run of `J'` is σ of the run of `J` if all four of these hold on the run of `J`:

1. **The argument is never read.** No instruction of the input script's unit names `Local[1]`, in any expression or
   as a target, and the IR shows every local the unit reads (`locals::unit_reads`, no index on a local). Only the
   running slot's own code reaches its locals (script.cpp:568-706), and a started script gets new locals.
2. **No other verb of the group is read.** After `eng_input`'s own read, the run reads no `verb:` fact of the scope's
   dialog verbs. Such a read gives 1 in one job, where it is pinned, and the fact's values in the other.
3. **No verb of the group is written.** No path writes such a `verb:` fact. A record drops a write that equals its
   precondition's single value, which differs between the two jobs.
4. **No list keeps the argument.** No slot a path leaves waiting keeps `Local[1]` in its list of locals. Under
   `--live-locals` an unread local is never kept; without it the list holds `v`.

*Proof.* By induction on the machine's steps, the two runs are in states equal up to σ and up to the value of the
input frame's `Local[1]`. A step reads facts, locals and the oracle. By (2) it never reads a fact whose value differs
between the two. By (1) it never reads `Local[1]`, and no step writes it. So both runs take the same step, fork the
same way and write the same values. Two paths of one run merge when their signatures are equal. A signature holds
the precondition and the frames' locals, which differ between the runs only by σ and by `Local[1]` on every path
alike, so the same paths merge. At the end, each path's precondition is σ of the other's, its writes are equal by
(3) and its kept slots equal by (4). The held branches' guides and keys and the facts the run consulted are σ of the
other's. So the hand-over of `J'` is σ of `J`'s. The Builder then makes `J'`'s records from it as from any run, with
`J'`'s own counters and checks, so they are byte for byte the records of `J'`'s own run.

## The rule

- `Reader::run` keys a click's run by the job's pins, precondition and guides with its verb taken out, the path limit
  and the batch.
- The first run with a key is made and checked for (1)-(4). If they hold, it is kept for the batch. A run of another
  job with the same key waits for it and takes it, renamed. If they do not hold, every job with that key runs itself.
- Kept runs are dropped when the batch ends, since a later batch may read other reached values or domains.

## Checks

1. Off: the kitchen's packed sha1 279b7a24; the reader's tests.
2. On: the packed file, the records' lines and the counters have the same sha1 as off on the kitchen, the bar, the
   jail, k = 1 and k = 2, with `--reach`.
3. Part I's five `dialog_line(120..124)[202@85]` jobs alone: their records are identical with the switch on and off.
4. Gain (E004's keep rule): at least 5% CPU on two check scopes, one of them k = 1 or k = 2.
