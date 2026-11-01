# A cut path becomes a havoc record (`havoc_cut`)

Plan step A1. The rule and its proof come first, for any SCUMM v5 game. MI1 numbers appear only in the tests and the
logs. Off by default; it needs `split_limit`.

## What it replaces

Under `split_limit`, a run in which the path limit cut nothing, but some path ended because it ran `STEP_LIMIT` steps,
stops the reading (`Builder::run`). Splitting does not help such a path. Every fact it read is already narrowed on its
own precondition, so pinning a fact only removes forks it has already taken, and the path reaches the step limit in
every part that holds its precondition. The usual cause is a loop whose exit tests a value that is not a fact. Each pass
forks on the non-fact test, and one path stays in the loop.

## The rule

Each path `p` that the step limit ended becomes one record `H(p)`, its havoc record:

- **Precondition:** what `record` makes of `pre(p)`: the job's seed and every fact `p` read, with the set it took.
- **Writes so far:** the writes `p` made before the cut, as `record` makes them.
- **The write set.** `C(p)` is the code `p` can still run:
  - the instructions reachable in the control flow of each frame on `p`'s stack, from its current instruction;
  - every unit a reachable `startScript`, `chainScript` or `startObject` names, whole, transitively; a number that
    does not fold to constants names every unit of the scope;
  - the hook scripts a reachable instruction can start (the cutscene hooks for `cutscene` and `endCutscene`, the
    scroll and inventory hooks for `loadRoomWithEgo`, `actorFollowCamera` and `setCameraAt`, the inventory hook for
    `setOwnerOf` and `pickupObject`, the sentence script for `doSentence`), each named by its variable's domain: the
    start value and every constant the scope's code writes there; a hook with no domain, an engine step the closure
    does not follow, or `systemOps` names every unit of the scope;
  - when a reachable instruction can change the room (`loadRoom`, `loadRoomWithEgo`, `actorFollowCamera`), the entry
    and exit code of every scope room, and the room hooks.

  `T(p)` is every fact of the table that some instruction of `C(p)` can write: the targets `scope_writes` finds in
  that code, the fix round 1 writes of the switches that are on, and the engine's own writes for the instructions
  present (the room variables for a room change, `cutscene` and `VAR_OVERRIDE` for cutscenes and overrides,
  `VAR_TALK_ACTOR` and `VAR_KEYPRESS` for talk and print).
- **Values.** For each fact `f` of `T(p)`: when every write site of `C(p)` that can hit `f` writes a constant, the
  effect is "one of those constants, or no change", written as cases on `f`'s current value. Otherwise it is any
  value of `f`'s domain. A value is picked by the task's existing pick variables, as a random draw is.
- **Slots.** `S(p)` is the slots of the units of `C(p)` (they can start, wait or end), the slots a reachable
  `stopScript` or `stopObjectScript` names (every slot of the scope when the number does not fold), and with a room
  change every slot of room code in the scope (the change kills them). For each slot of `S(p)`:
  - its place may become any of its wait points or none, and its list of locals may become 0 (unknown) or stay;
  - it may be frozen or unfrozen, and its Esc point may change or go;
  - its event at each of its wait points, with unknown locals, is named, so the plan can go on from any of them.

  With a reachable `freezeScripts`, the frozen flag of every slot may change. With a reachable `cutscene`,
  `endCutscene`, `beginOverride` or `endOverride`, every Esc point may change.
- **End and cost.** The record ends "havoc". Its cost is its job's own (costs.rs prices a record by its input or by
  the wait before its event, never by its end): at least 1 frame.

## Why the record covers every engine run of the path

Take any state in which the engine starts the job and follows `p`'s prefix. `pre(p)` holds there, so `H(p)` applies.
From the cut on, the engine runs some continuation `e` until the job's run ends: every frame on the stack has returned
or yielded and the engine's pending steps are done.

1. Every instruction `e` runs is in `C(p)`. It is reachable from a frame's current instruction, or it belongs to a unit
   started on the way, or to a hook or room code that a reachable instruction runs. The closure follows each of these.
2. Every fact `e` writes is in `T(p)`. `scope_writes` reads the same instructions and gives each target: a constant
   target exactly, an index that does not fold as its window or prefix. The engine's own writes are added for the
   instructions that make them.
3. Every value is allowed. A fact outside `T(p)` keeps the value `p` left it, which is `H(p)`'s own effect, or no
   effect. A fact in `T(p)` ends with one of the constants its sites write, or keeps its value, when all its sites
   write constants. Otherwise it ends with some value of its domain. That is the convention every guess of the reader
   already rests on (reach.md, "Domains"): a domain lists the values the reader knows a fact can take.
4. Every slot ends as allowed. A slot `e` does not touch keeps its place and list. A slot it touches is in `S(p)`. It
   ends dead, which is the place "none", or waiting at one of its unit's wait points with some list of locals. The event
   with unknown locals at that point covers every list: its run reads each local as unknown.
5. The time is a lower bound. The record's cost counts only the frames before the job's run starts (its clicks, or
   its slot's wait), which every continuation also takes, and at least 1.

So the state `e` ends in is one of `H(p)`'s outcomes, at a cost no lower than the record's. Adding records only adds
choices, so the model's optimum can only fall, and L stays a lower bound. This is a may-summary at a unit boundary, as
in SMASH (Godefroid et al. 2010).

What it does not claim: that `H(p)` is tight. The plan can pick outcomes the engine never reaches. A plan that uses a
havoc step is checked by playing it, and a refuted havoc step goes to the trace validation list (B1).

## With the switch off

The Builder's branch is the old one: the reading stops at a step-limit path. No other line changes. Every golden is
byte for byte today's, and the kitchen's packed file keeps sha1 279b7a24.

## With the switch on, where nothing was cut

A reading in which no path reaches the step limit makes no havoc record, so its output is the same byte for byte. The
check scopes (the kitchen, the bar, the jail, k = 1, k = 2) read without a step-limit path today, so their records and
optima do not change.

## Checks

1. Off: the kitchen's packed sha1 279b7a24; the reader's tests.
2. On, on the five check scopes: the packed files equal the off ones.
3. A machine that ends one path at the step limit: the reading makes exactly one havoc record for it, with the effects
   and events above, and goes on.
4. Part I's stopping job, run alone: each path the step limit ended gets one havoc record, and its write set is the
   code its unit can still run.
5. Kill: a check scope's records grow by more than 10%, or Part I's round 5 by more than 2x, or plans on a check scope
   use havoc steps the engine refutes more than 3 times.
