# Forward reachability in the reader (`--reach`)

Reduction 3 of E004. The proof comes first, for any SCUMM v5 game. MI1 numbers appear only in the results section,
at the end, and in the tests and logs.

## What the switch does

Taskcore's step (d) computes relaxed reachability from the start: delete effects ignored, every value an effect can
write counts as reached once its action applies and its conditions hold. It then drops every action whose
precondition has a set with no reached value. The reader today makes those actions first, and most of its time goes
there. With `--reach` the reader keeps the same set of reached values while it reads, and it never runs a branch that
needs a value not reached yet. A held-back branch runs when its value arrives.

Off by default. With it off, no line of the reading changes.

## Terms

- A **job** is one input or one event. Its **seed** is the precondition its job carries (engine conditions, the
  slot's place and list of locals).
- A **path** is one way through a job's run. `pre(p)` is the path's precondition: the job's seed plus every fact the
  path read, each with the set of values it took there.
- A **fork** on fact `k` splits a path into one path per value, or per value set (switch `value_sets`). Each such set
  is a **group**.
- A **record** is what `Builder::record` makes of a path. An **action** is what `record_action` makes of a record: a
  precondition and effects, some under conditions.
- `meets(k, S, R)`: some value of `S` is in `R(k)`. `at:<unit>:W` and `esc:<unit>:W` are read through `pos:<unit>` and
  `escpos:<unit>`, the facts the record writes for them: `{1}` needs `W` in `R(pos)`, `{0}` needs another value there.
- `(d)`: the least `R` with the start's values in it, closed under: if every precondition set of an action meets `R`
  and every condition of an effect meets `R`, then the effect's values are in `R`. It is taskcore's `red_d_core`,
  rule for rule, on the actions as `packed.rs` loads them (a set cut to the domain; a written value the domain lacks
  reaches nothing; `add` as one effect per current value).

## Why (d) keeps every plan from that start

Ignoring deletes only adds states. Take any plan from the start and any step of it. By induction on the steps, every
fact's value in the state before the step is in `R`: the start's values are, and a step writes only values its effects
list, under conditions that hold, so they are in `R` by the closure rule. So every precondition set of the step meets
`R`, and it is kept. This is taskcore's existing argument. It is the only property the reader relies on.

## The four facts about the machine the reader relies on

1. **Determinism.** A step of a path is a function of the path's state. It reads no other path.
2. **Branches on fact values never merge.** `Reader.join`'s signature contains the whole of `pre(p)`. Two paths merge
   only if their preconditions are equal. So a path with `k = v` never merges with a path with `k = u`, and every
   path in a merge group has the same fate under any rule that decides by the precondition.
3. **The order of the rest does not change.** `Machine::drive` keeps its paths on a stack and merges waiting groups
   when the stack is empty, deepest first. Removing a path removes only its descendants. The groups of the others hold
   the same members in the same relative order, because a group's members have equal preconditions.
4. **Only the path limit crosses paths.** `done + todo + waiting` is compared with `PATH_LIMIT`. With split_limit on, a
   run past the limit is run again as parts. `--reach` needs split_limit, because without it the paths the limit
   ends depend on how many others are running.

So: **Lemma 1 (a branch can be cut).** Cutting group `g` at a fork removes exactly the paths whose precondition holds
`g` at that fork. Every other path of the run, and its record, is unchanged. The only exception is a run whose path
count crosses the limit in one reading and not in the other. There split_limit's pins change what merges (see "What
is not exact").

## The rule

`R` is a monotone map from fact to reached values. Each value carries the round it arrived in.

- **Seed.** `R` starts as `task_start`'s values for the start file (the same function that writes the packed file's
  start), one for each start given. Dumped slots (`--slot-locals`) add their lists' numbers, as the start file the
  harness writes for them does.
- **A job** whose seed has a set that does not meet `R` does not run. It waits on that fact.
- **A fork** in `on_fork` runs only the groups that meet `R`. Each other group is written down as a *held branch*:
  the job, the path's precondition with the group added (its *guide*), the fact and the group. If no group is left,
  the path ends there, as a path with no value left does.
- **A path that ends** with a record whose precondition does not meet `R` is held the same way, on its first set that
  fails. This happens only when two sets on one unit combine: `at:X:5 = 0` and `at:X:7 = 0` each meet, and together
  they may not. The check is on the record's own precondition (`positions`, then `record_action`).
- **A record that meets** is written. Its effects go through `(d)`'s closure rule (an effect with unmet conditions
  waits on them). Only written records name the next events.
- **Round.** The jobs of a batch read `R` as it stood when the batch began, so the reading is the same for any number of
  threads. The batch's new values are committed together. What they free runs in the next batch.
- **Resume.** A job with held branches freed runs again with their guides. At a fork it follows a group only if some
  guide agrees with the path's precondition and the group (every fact both name has a common value). A later fork on
  a fact no guide names is free. Dropping a guide's entry only makes the run follow more, so a guide is an
  optimization and never a condition of being right.
- **Exactly once.** A rerun in round `t` writes a record only if its precondition meets `R_t` and fails `R_{t-1}`. A
  record that meets `R_{t-1}` was written earlier and is left out.

The reading ends when a batch has no jobs. Nothing is freed then, and no event is new.

## Theorem 1: the reading writes what the closure allows

Let `F` be the records of every job of the full reading with the same domains and lists, `R*` the closure over `F`
with event naming (below), and `K` the records of `F` whose precondition meets `R*`. Then `--reach` writes exactly `K`,
each once, and its final `R` is `R*`.

*The reading stays inside.* Every written record is a record of `F` (Lemma 1) whose precondition meets `R` when
written. By induction on the commits, `R ⊆ R*` and every written record is in `K`.

*Nothing is lost.* Suppose `r ∈ K` and take the first round `t` at which `pre(r)` meets `R_t`. Walk `r`'s path from
the job's start. Every group on it holds `pre(r)`'s set for its fact or a superset (a path only narrows), so it meets
`R_t`. If the job's first run is at round `t` or later, the walk is never cut. Otherwise let the first group that fails
`R_{t-1}` be at fork `f`. All forks before `f` meet `R_{t-1}`. By induction on the rounds, the prefix up to `f` was
explored by round `t-1`, and at that moment `f`'s group was held with the guide `P ∪ {k: g}`. It is freed at round `t`.
Its rerun follows the prefix (it agrees with the guide), passes `f`, and from there on every fork is free of the other
guides' cuts because `pre(r)` extends the guide. If no group fails `R_{t-1}`, the path's record was held on a set that
fails `R_{t-1}`, which is the same case with `f` at the path's end. So the path ends in round `t`, and by Lemma 1 it
ends in `r`. `r` meets `R_t` and fails `R_{t-1}`, so it is written then, and in no other round.

*The closure.* Effects go through the same rule as `(d)`, in any order. A monotone closure on a finite set has one
least fixed point, so the order of the rounds changes nothing, and `R` at the end is `R*`.

## Event naming, the one place `R*` can be smaller than `(d)`'s

The reader learns that an event exists only from a record that names it (`next_events`). `(d)` treats `pos:X` and
`args:X` as independent facts. Say record `a` writes `pos:X = 5, args:X = s` and record `b` writes `pos:X = 7,
args:X = u`, both kept, and only a third record `c`, which `(d)` drops, writes `pos:X = 7, args:X = s`. `c` names the
event `(X, 7, s)`. `(d)` keeps that event's records, because `7` and `s` were each reached on their own. `--reach`
never learns of that event.

**Lemma 2 (no plan loses these records).** In the model, any record that writes `pos:X = w` with `w` not `NOWHERE` also
sets `args:X` to the number of the list it names (`Builder::record`, `next_events`), or keeps the value that its
precondition already gives. So in every state reachable by a plan from the start, a live slot's `(pos:X, args:X)`
pair is one a record on the plan named, or the start's, or `(w, 0)` with `(w, ·)` named (the replan copy every wait point
gets). So an event `(X, w, s)` that no kept record names cannot apply in a state a plan reaches. Its records are in no
plan from that start.

This makes `--reach` a slightly stronger reduction than `(d)`, and it is still exact for plans. The check counts these
records in every scope. Any other record `(d)` keeps and `--reach` does not write is a failure of the reduction, except
for the case below.

**A job read otherwise.** Under split_limit a run past `PATH_LIMIT` is run again as parts, each with the split fact
pinned from the start. A path that does not read that fact then holds it in its precondition all the same, and merges
differ. With `--reach` the run may stay under the limit and not split. Both readings are exact (split_limit's proof), but
their records for that job differ: the unsplit job's records are the finer ones. The check lists them as a class of
their own, by label.

## Domains

A pass reads with fixed domains. A value written outside its domain widens the domain for the next pass, and the
reading starts over, as it does today. `R` is thrown away at the start of a pass: the labels of a fact can change
(`Domain.label`).

With `--reach`, only kept records widen a domain. So a domain may hold fewer values than the full reading's. That
affects three things.

- **A fork on the extra value.** The value is not reached, so `(d)` would drop those records anyway.
- **A set that equals the whole domain.** A record leaves such a set out of its precondition. A value set that is the
  whole domain with `--reach` but not in the full reading: `(d)` would cut the full reading's set to the same reached
  values.
- **A guess over the whole domain** (`Eff::Guess(dom.values)`, a write the machine cannot work out). This is the real
  difference. The full reading's guess also lists a value only an unkept record widened the domain with, and `(d)`
  counts it as reached. With `--reach` the value is not in the domain. Such a value can then free records in the full
  reading that `--reach` lacks. The domain was never a bound on what an unknown write can hold: a guess is the model's
  list of candidates, not a fact about the game. The check lists every value of this kind. A plan that needs one has a
  step whose guess picked a value that only unreachable code writes.

`ELSEWHERE` needs nothing new. It is a value of `pos:` that a written record reaches. Since `positions` takes it out of
every precondition set, it frees nothing. An event at a place outside the scope's points has `{1}` on a place `pos`'s
domain lacks. It never meets `R`, and its job never runs. `(d)` drops its records too, because their set is empty.

## Lists

A job's precondition on `args:X` is left out today when the lists known at that moment are none, because the set would
be the whole domain. Which lists are known then depends on what ran before. With `--reach` the set is always written, so
a record does not depend on the order lists arrive in. In the full reading such a record has no set on `args:X`, and it
also writes `args:X = 0`. With `--reach` it holds `args:X = {0}`, and that write goes, as for any write of the value a
precondition already gives. The check compares both without this. A record that loses its only effect this way is a
no-op.

The record with the set is the tighter one: without it, an event with unknown locals applies at a state where the slot
has a known list. Such a step has no plan of its own: what it can do with the real list is in the event of that list.

SIG_LIMIT: the number of lists a wait point has seen depends on what ran. With fewer lists, `--reach` may fold fewer of
them, and then its records name a list the full reading has folded into 0. The check counts these, and they are
allowed: the full reading's folded record stands for a set of lists, `--reach`'s for one of them. A list that `--reach`
folds sets `args:X = 0` in `R`, since `RecordStream::finish` rewrites the written number to 0.

## A replan start with a value the first start did not reach

`(d)` from a later start keeps every plan from that start if `R` already holds the start's values, by monotonicity
(reduction 1). Otherwise the reading does not cover it, and this must not pass in silence.

- **Detection.** The reader writes `NAME.reached` next to the packed file: one line per fact in the file, its key and
  its reached values. `taskcore sas` and `taskcore serve` take `--reached F`. If the start holds a value outside the
  file's reached values, taskcore stops with exit code 7, naming each fact and value. It writes no task. A start value
  a task's domain lacks is taskcore's existing start check; this check is the one for a value the domain has and
  the reading did not reach.
- **Extension.** The held branches stay in the Builder for the whole pass. A new start is one more seed: its values
  are committed as a round, they free what waits on them, and the rounds go on. By Theorem 1 the result is the closure
  from both starts. It is the same set of records as a reading seeded with both from the beginning, because a
  closure does not depend on its order. `Builder::reach_extend` does this in process. `reader-scope --reach-also FILE`
  makes the reading from both starts on the command line, with a start file in the format of
  `taskcore --start-file`.
- **Why it stays correct.** `R` only grows, so what was written stays right, and the task only gains actions.

## Checks

1. With the switch off, every golden is byte for byte today's.
2. On each scope, from the full reading with split_limit and the same switches:
   - The actions `(d)` alone leaves of each file (`taskcore dump --fold off --reduce d`, `reach_check.py`), as sets of
     canonical texts. Both directions are reported. An action `(d)` keeps that `--reach` does not write must be in
     Lemma 2's class, a job read otherwise, or the domain class above. Anything else is a bug.
   - Reached fact values by `(d)`, from both packed files.
   - The task taskcore makes from each with all its reductions, as canonical texts. Where they are equal up to names,
     every optimal cost is equal without a search. On the kitchen, the optimal cost with A* and landmark cost
     partitioning from both.
3. A property test on a fake machine: any order of arrival gives the same records, and a held branch resumes.
4. A start with a new value: taskcore stops, and the extended reading covers it.
