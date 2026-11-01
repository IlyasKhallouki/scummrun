# Pairs of the verb script and the room for job keys (`reach_pairs`)

Plan step A6(a). The rule and its proof come first, for any SCUMM v5 game. MI1 numbers appear only in the tests and
the logs. Off by default; it needs `--reach`.

## What the switch does

`--reach` (reach.md) keeps one set of reached values per fact. Two facts can each reach a value while no reachable
state holds both at once. A dialog line job clicks a verb while the verb script variable holds `v` in room `R`
(runInputScript runs `VAR(VAR_VERB_SCRIPT)`, script.cpp:1478-1511; VAR_VERB_SCRIPT is variable 32 in v5, vars.cpp),
and its seed pins both. So do F1c's sentence jobs that start at a local input script (inputs.rs).

With the switch the reader also keeps the reached pairs of two facts, A = `var:32` and B = `room`. A job whose seed
has a set on both runs only when a reached pair meets both sets. Such a job waits on the pair and runs when a pair
that meets it arrives. Every other pair of facts is taken as reachable. On those the rule is `--reach`'s.

## Terms

- `R`: `--reach`'s reached values, as it stands when the rule is applied (reach.md).
- An **action** is `record_action` of a written record: a precondition (a value set per fact) and effects. An effect
  has a fact, what it writes (a value, a guess's list of values, or an add's step), and conditions (value sets).
- `pre_A(α)` is the set of action α on A. It is absent when α has none, and then every value meets it. Likewise
  `pre_B(α)`.
- An effect on A or B **may fire** at the pair `(x, y)` when its conditions on A hold `x`, its conditions on B hold
  `y`, and each of its other conditions meets `R`.
- It **must fire** at `(x, y)` when its conditions are on A and B only, they hold `x` and `y`, and every value it
  writes is in its fact's domain.
- What an effect writes at the current value `x` of its fact: its value, its guess's values, or the domain's step
  `dom.add(x, d)` for an add. A value its fact's domain lacks writes nothing: taskcore gives it no index, and (d)
  reaches nothing from it.
- `C_A(α, x, y)` is the set of the values each effect of α on A that may fire at `(x, y)` writes at `x`, plus `x`
  itself unless an effect of α on A must fire at `(x, y)`. `C_B(α, x, y)` is the same with B and `y`.

## The rule

`P` is the least set of pairs such that:

- each start's pair (its values of A and B, as `task_start` gives them) is in `P`;
- for each written action α and each `(x, y)` in `P` with `x` in `pre_A(α)` and `y` in `pre_B(α)`, every pair of
  `C_A(α, x, y) × C_B(α, x, y)` is in `P`.

This is h² (Haslum & Geffner 2000) restricted to the pairs of A and B. The pairs of any other two facts are taken as
reachable, which is h¹ on them: the rule of `--reach`.

A job whose seed has a set `S_A` on A and `S_B` on B runs only when `P` holds a pair in `S_A × S_B`. A job whose seed
lacks either set is not held by the rule. When a start lacks a value of A or of B (no domain holds it), the rule holds
no job back.

## Lemma 1: every state a plan reaches holds a pair of `P`

Take the actions of the written records and a plan π over them from a start. Every state `s` on π has
`(s[A], s[B])` in `P` and each `s[k]` in `R(k)`.

By induction on the steps. At the start this is the rule's first line, and reach.md's seed. Take a step α from `s` to
`s'`.

- Each value of `s'` is in `R`: this is (d)'s argument (reach.md, "Why (d) keeps every plan").
- α applies at `s`, so `s[A]` is in `pre_A(α)` and `s[B]` in `pre_B(α)`. By the induction `(s[A], s[B])` is in `P`.
- In `s'`, A holds what an effect of α on A that fired at `s` wrote, or `s[A]` when none fired. A step reads its
  effects' conditions in `s`, as taskcore and Fast Downward do.
- An effect that fired at `s` has its conditions hold there: on A at `s[A]`, on B at `s[B]`, and on any other fact `k`
  at `s[k]`, which is in `R(k)`. So each of them meets `R`, and the effect may fire at `(s[A], s[B])`. The value it
  wrote is one it writes at `s[A]`.
- If no effect on A fired, then none must fire at `(s[A], s[B])`, since an effect that must fire there fires at every
  state with that pair.
- So `s'[A]` is in `C_A(α, s[A], s[B])`, and likewise `s'[B]` is in `C_B(α, s[A], s[B])`. By the rule's second line
  `(s'[A], s'[B])` is in `P`.

So h² never calls a reachable pair unreachable (Haslum & Geffner's result, restricted here to one pair of facts).
Taking the other pairs as reachable only adds pairs to `P`.

## Theorem: the reading with the switch keeps every plan

Let `K` be what `--reach` writes and `K'` what it writes with the switch, from the same starts, domains and lists, and
`P'` the pairs over `K'`. Then `K'` is a part of `K`, and every plan from a start over `K` is a plan over `K'`.

*`K'` is a part of `K`.* The switch adds no record and no value. It only holds some jobs back, so `R'` is a part of
`R`. Each written record is a record of its job's run and meets `R'`, so it meets `R`. By Theorem 1 of reach.md that
record is in `K`. The proof of Theorem 1 does not depend on the round at which a job first runs, so it holds for a job
held on a pair and first run when the pair arrives.

*Every plan over `K` is a plan over `K'`.* Take a plan π over `K` and its first step α whose record `r` is not in `K'`.
Every step before it is in `K'`, so by Lemma 1 over `K'` the state `s` before α holds a pair of `P'` and values of
`R'`.

- `r`'s precondition holds at `s`.
- A path's precondition starts as its job's seed and only narrows (machine.rs: a path's precondition begins as the
  seed). `record` leaves out only a set that is a fact's whole domain, which every state holds. So at `s`, A and B hold
  values of the seed's sets of `r`'s job, and `(s[A], s[B])`, a pair of `P'`, meets them.
- So that job runs in the reading with the switch. By Theorem 1 of reach.md its run writes every record of it whose
  precondition meets `R'`. `r`'s precondition holds at `s`, whose values are in `R'`, so `r` is written: `r` is in
  `K'`, which contradicts the choice of α.

So the task from each start has the same plans, and the same optimal cost, with the switch and without it.

## What is not exact

The three classes of reach.md, for the same reasons.

1. **Event naming.** An event that only the records of a held job name is never learned. No plan reaches it: by the
   Theorem, no plan uses those records.
2. **Domains.** Only written records widen a domain for the next pass. A value only a held job writes is not in the
   domain with the switch. A guess over the whole domain lists it without the switch, and (d) counts it as reached. A
   plan that needs it picks, at a guess, a value only code no plan runs writes.
3. **A job read otherwise.** A run split in one reading only.

The check compares the reached values (`NAME.reached`) and the records of both readings, and lists each difference
by class.

## Computed while reading

`P` only grows as the reading goes on. More written actions give more pairs. A larger `R` lets more effects fire, and
"must fire" does not depend on `R`, so a pair once in `P` stays there. So `P` can be computed while reading.

- At each round's commit, after `--reach`'s own closure, each action written in the round is applied to every pair of
  `P`.
- Each pair new to `P` is applied to every action.
- An action is applied again to every pair when one of its effects' other conditions meets `R` for the first time.
- A held job is freed when a pair meets its sets, and it runs in the next batch.

A monotone closure has one least fixed point whatever the order, so this computes the least `P` above. A batch reads
`P` as it stood when the batch began, so the jobs freed are the same for any number of threads. `P` is thrown away
with `R` at the start of a pass. A later start (`reach_extend`) adds its pair as one more seed.

## With the switch off

Nothing here runs. Every golden is byte for byte today's, and the kitchen's packed file keeps sha1 279b7a24.

## Checks

1. Off: the kitchen's packed sha1 279b7a24; the reader's tests.
2. A unit test of the rule: an effect that may not fire leaves its fact's value in the pair; an effect that must fire
   replaces it; a condition on another fact frees its effect only once `R` meets it.
3. On the five check scopes, against `--reach` alone: the same reached values, `K'` a part of `K`, and each record of
   `K` that `K'` lacks either in a job held on a pair or in one of the three classes. The records and reading CPU each
   saves (E004's keep rule).
4. k = 2: the `dialog_line(·)[202@85]` and `dialog_line(·)[204@28]` families.
