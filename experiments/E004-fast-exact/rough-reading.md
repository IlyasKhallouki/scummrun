# A rough reading: every job read to N steps (`--step-limit N`)

The rule and its proof come first, for any SCUMM v5 game. MI1 numbers appear only in the tests and the lab notes.
Off by default; it needs `havoc_cut` (havoc-cut.md), which needs `split_limit`.

## What the switch does

`reader-scope --step-limit N` reads every job (every input and every event) with the machine's step limit `N` in
place of `STEP_LIMIT` (5,000). A path that runs `N` instructions ends there, and `havoc_cut` makes it a havoc record:
its precondition and writes so far, then "one of the constants written, or no change", or any value of the domain,
for each fact the code it can still run may write, each slot that code can start, stop or reach at any of its wait
points with unknown locals, or none, and the job's own cost (at least one frame). Nothing else changes: the same
jobs, the same forks, the same records for every path that ends within `N` steps, the same `--reach` rounds.

With the switch off (`N` = 0, the default) the machine keeps `STEP_LIMIT`, and no line of the reading changes.

## Lemma R1: the limit only ends paths

The machine checks the limit at the top of `Machine::drive`'s loop, before it runs the path's next action (one
instruction of the top frame, or one engine step of the top call), and the check only sets the path's end. So a path
the limit ends stands between two actions with its whole state: its stack of frames, each with the instruction it runs
next, and its calls, each with the engine steps it has still to run. The limit reads no other path and changes no
other path, except that a path it ends no longer joins a merge group. A merge group's members have equal
preconditions (reach.md, fact 2), so a group with one member less covers fewer states, never other ones: every
remaining path is still a path the machine's rules make from the job's start.

## Lemma R2: a cut at any point is covered

Take an engine run of the job from a state in which the job applies, and the machine path `p` it follows. If `p` ends
within `N` steps, its record covers the run, as without the switch. If the limit ends `p`, havoc-cut.md's argument
applies at the point where `p` stands, whatever that point is: `C(p)` starts from each frame's next instruction and from
each call's pending steps, so it does not depend on where in the code the cut falls. What it needs is that every kind
of point a cut can stand at has its continuation in `C(p)`. The kinds are:

| where the path stands | what runs next | where `C(p)` has it |
|---|---|---|
| in a frame, between two instructions | the rest of that unit | the frame's control flow from its next instruction |
| in a frame at the end of its code (no next instruction) | nothing | the unit from its first instruction, which is more |
| in a nested frame (`startScript`, `startObject`, a hook, room code run at once) | the callee, then the caller | every frame of the stack |
| after an op pushed its call, before the call's first step | the call's steps | the pending steps |
| between two engine steps of a call: a script or object start, a hook, a write, a sentence | each step | the pending steps, each as the code it runs |
| between the steps of a room change (scene, room code, kill, load, ego) | the rest of the change | "a room change": every scope room's entry and exit, the room hooks |
| between a scroll note or the second inventory hook note and what follows | the hook the engine may run | the scroll hook, the inventory hook |
| inside the reader's own click steps (the input script, F1c's clicks, F1c's end) | the second click, the queued sentence | the click's variables, the input script, the sentence script |
| any other pending note or step | not modelled | every unit of the scope |

Each row's code is in `C(p)`, so every fact the continuation writes is in `T(p)` and every slot it touches is in
`S(p)`, and havoc-cut.md's items 3 to 5 give the values, the slots and the cost. So `H(p)` covers the run.

## Lemma R3: the rough reading over-approximates the reading

Every record of the full reading is a path of some job. By R1 and R2, that job's run under the limit `N` has either
the same path or a havoc record that covers it: every fact the path writes after the cut is written by code of `C(p)`,
with a value its sites write or a value of the domain, and every slot it leaves waiting is at one of its wait points,
whose event with unknown locals the havoc record names. Under `--reach`, a havoc record's effects grow the reached
values as any record's do, and its events are jobs like any other, read again under `N`. By induction on `--reach`'s
rounds, every value the full reading reaches is reached by the rough one, every job the full reading runs is run, and
every action of the full task is matched by an action of the rough task that applies in at least the same states and
can write at least the same values. Two things differ in form, not in what they cover:

- **Lists of locals.** An event a havoc record names has the list 0 (unknown), which every list's event covers
  (havoc-cut.md, item 4), so `args:` facts take 0 where the full reading has a list's number.
- **Domains.** A havoc effect takes its values from the fact's domain (havoc-cut.md, item 3, the convention every
  guess of the reader rests on: a domain lists the values the reader knows a fact can take). The full reading also
  widens a domain in pass 2 with a value a path wrote outside it (`Builder::extend_domains`); the rough reading widens
  it only with values its uncut paths write. So R3 holds for the values of the domains the rough reading ends with,
  and on a fact whose domain the full reading widened further, the value the rough reading lacks is one only code past
  a cut writes, which the havoc record gives as "any value of the domain".

So:

- **No plan in the rough task means no plan in the full task**, for the same goal and start: a goal fact the rough
  task cannot reach is unreachable in the reading's model.
- **A plan in the rough task is a candidate.** Its exact steps are the full reading's actions. Its havoc steps may pick
  outcomes the engine never reaches; each is checked by reading its job exactly or by playing it.
- The cost of a havoc step is its job's own cost, a lower bound of the engine's, so a rough plan's cost is a lower
  bound of the cost of the full plan it stands for.

## What `N` trades

The smaller `N`, the more paths are cut and the more of the reading is havoc. A havoc record whose code can change the
room names every scope room's entry and exit code; one whose code has a start that does not fold, a hook whose
variable has no domain, `systemOps`, or a pending step the closure does not follow names every unit, and then every
fact the scope writes. Past some `N` the plan stops being trivial. The lab notes measure where.

## Checks

1. Off: the kitchen's packed sha1 279b7a24; the reader's tests.
2. `--step-limit 5000` writes the same packed file, records and counters as no `--step-limit`, byte for byte (the
   limit is the machine's own).
3. On the check scopes with run D's switches: for every fact but the `args:` facts, the values the rough reading reaches
   (`NAME.reached`) hold the full reading's that are in the rough reading's domain (Lemma R3), and a goal the full
   reading reaches the rough one reaches.
4. A job whose path the limit ends in each kind of place of the table above gets a havoc record whose closure holds
   the code that runs next (`tests/havoc_cut.rs`, `machine.rs`'s tests of `cut_of`).
