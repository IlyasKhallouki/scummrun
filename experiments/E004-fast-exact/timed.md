# L: the timed search on one clock (plan step C3)

A lower bound L on the least game time OPT from a start to the goal, over the input sequences V, under K. Our own A*
in Rust over taskcore's operators (`taskcore timed`, off unless called), with the duration rules of `durations.md`
(C2). The proofs come first and hold for any SCUMM v5 game. Numbers of one game appear only in tests and logs.

## The timed model

- **Operators.** Taskcore's operators of the task from a start: the packed file loaded, the start check, the fold and
  reductions (a) to (e), expanded into variants by the writer's rules (`sas.rs` `Expander::expand`). Each operator
  has a precondition (one value per fact), effects (a value under conditions on facts and pick variables) and its
  action's label and frame cost. Reduction (f) is left out (see "What the reductions keep").
- **Classes and bounds.** By its label and frame cost, each operator is an *event* of unit U resuming at W, with
  lb(o) from its entry, per room for a `local:N` unit; or an *input* with lb(o) from the previous input (the bounds
  file, `durations.py`). A slot waiting at the start has its D8 cap.
- **State.** The facts s; per slot U the time e_U since the last firing that wrote `pos:U` (or since the start, with a
  flag, for a slot that has not moved since the start); e_in, the time since the last input (since the start before
  the first one).
- **Firing.** An operator o of slot U is enabled at (s, e) when its precondition holds in s and e_U ≥ b(o), where b(o)
  is lb(o) for the room `room` holds in s, or D8's bound while U's flag is set. An input needs e_in ≥ lb(o). Firing
  takes no time: s becomes o(s); every slot X whose `pos:X` an effect wrote that fired gets e_X = 0 and loses its flag;
  an input sets e_in = 0.
- **Advancing.** Time may pass by any δ > 0: every e grows by δ.
- **L** is the least time at which the goal holds over the model's runs from the start.

**Lemma T0 (L ≤ OPT).** Every engine run in V from the start to the goal, with game time Q, maps to a run of the
timed model reaching the goal at time at most Q. *Proof.* A1 maps the engine run to a sequence of records, each at its
frame k_i, a plan of the untimed task from the start (A1 is a premise, tested by B1). Time the model run at
τ_i = Q(k_i) − Q(s). Each record's precondition holds by A1. An event's slot clock at its firing is
Q(k_i) − Q(j) where j is the frame of the last record that wrote `pos:U` (the entry, by the same mapping), and C2's
lemmas give Q(k_i) − Q(j) ≥ lb; the start's caps by D8; an input's clock by D5. A record whose effect on `pos:X` the
reductions dropped resets no clock in the model, so its clock there is at least the engine's. The goal holds after the
last record, at τ ≤ Q(goal) − Q(s). ∎

## The left shift and the epochs

**Lemma T1 (the left shift).** For a fixed sequence of operators o_1 … o_n that the model can fire, the earliest
schedule τ*_i = max(τ*_{i−1}, d_i(τ*)) is feasible and τ*_i ≤ τ_i for every feasible schedule τ, where d_i is o_i's due
time: its slot's entry (the last earlier firing that wrote `pos:U`, or the start) plus b(o_i), or the previous input
plus lb(o_i). *Proof.* The facts before o_i are the same in every schedule of the sequence, so preconditions, the
room, the flags and which earlier firing is the entry do not depend on the times. d_i is a maximum of earlier times
plus constants, so it is monotone in them. By induction, τ*_j ≤ τ_j for j < i gives d_i(τ*) ≤ d_i(τ) ≤ τ_i, and
τ*_{i−1} ≤ τ_{i−1} ≤ τ_i. Feasibility holds by construction: each τ*_i is at least its due time and the order is kept. ∎

Guards here are only of the form "at least this long since an earlier firing", and facts change only at firings. No
guard ever closes with time: once a clock passes its bound it stays past it. This is the case Cushing et al. (2007)
show decision-epoch planning complete for; their incompleteness comes from guards that close (deadlines, windows) and
from actions that must start between events, neither of which the model has. The engine's urgency (a due event runs
in its frame) is relaxed: an event may run at any time at or after its bound. Relaxing it only adds runs, so L stays a
lower bound.

**Lemma T2 (epochs suffice).** Let the search's successors at (s, e) be: fire any enabled operator (time does not
move), or advance to the next epoch, the least d > 0 at which some operator whose precondition holds in s becomes due.
Then the search reaches the goal at the least time L. *Proof.* Take a run of least time and its sequence; by T1 its
earliest schedule is no later. Follow that schedule. If τ*_i = τ*_{i−1}, o_i is enabled now. Otherwise
τ*_i = d_i > now and o_i's precondition holds in the current s (s changes only at firings), so d_i is among the
candidates; each advance moves to the least candidate, which is at most d_i, and o_i stays a candidate until the clock
reaches d_i. So every step of the earliest schedule is a path of the search graph, and its goal time is L. Every path
of the search graph is a run of the model, so the search finds nothing below L. ∎

This is minimum-time reachability with the time as the cost (Behrmann et al. 2001), on points rather than zones (the
due times are exact sums of bounds, so one point per discrete state and clock vector is enough).

**Lemma T3 (capped clocks).** A clock enters only through tests e_U ≥ b(o) for operators at U's current wait point W.
With E(U, W) the largest such b (the D8 cap included while U's flag is set), all clocks e_U ≥ E(U, W) behave the same
from then on, so e_U can be stored as min(e_U, E(U, W)); a slot whose `pos:U` is at a point with no operator (or not
running) stores 0. Two nodes with the same facts, capped clocks and flags have the same futures, shifted by their
times: the later one is dominated. ∎

## Pruning

**Lemma T4 (partial-order reduction within an epoch).** Call o and o′ independent when neither writes a fact the other
reads (precondition, effect condition, `room` for a bound by room) or writes, and they are not both inputs (both read
and write e_in). If both are enabled at (s, e), firing o then o′ and o′ then o, at the same time, reach the same state:
the writes are disjoint, neither changes what the other tests (a clock reset is a write of `pos:X`, which the other's
events read). Fix an order of the operators. The search fires o′ right after o, with no advance between, only if o′
depends on o or comes after o in the order; the node key holds the last operator fired at the current epoch (none
after an advance). *Proof of completeness.* In a sequence of firings between two advances, two adjacent independent
operators in the wrong order can be swapped (both stay enabled, the state after them is the same), and the
lexicographically least sequence of the class has none (else the swap gives a smaller one). That sequence is a path
the rule keeps; with the last operator in the key, no node of it is merged with a node the rule treats otherwise. ∎

## The heuristic

**Lemma T5 (h^max with bounds as durations).** At (s, e), let T(p) = 0 for each fact value p of s, and let T be the
least fixpoint of: an operator o is relaxed-reachable at
T_o = max( max over its precondition p of T(p) + w(o, p), D(o) ), with w(o, `pos:U`=W) = b_min(o) for an event of U
that does not stand at W now (b_min: the least of its room bounds) and 0 otherwise, and D(o) = max(0, b_min(o) − e_U)
for an event of a slot standing at W now (D8's bound in place of b_min while the slot's flag is set),
D(o) = max(0, lb(o) − e_in) for an input; each effect with its fact conditions (pick conditions dropped: a
window can choose any value) gives its value at max(T_o, its conditions' T). Then h(s, e) = the largest over goal
facts of the least T of their goal values is admissible and consistent.

*Proof.* Admissible: in any continuation, a fact value is first achieved, and an operator fires, no earlier than its T
(induction over the continuation's firings in time order): an operator fires after its precondition values hold, so
after their first achievements; an event of U fires at least b after its slot's entry, which is the current entry
(e_U ago, so at least b − e_U from now) or a later firing that wrote `pos:U` = W, itself no earlier than T(`pos:U`=W);
an input at least lb after the last input, which is now or later. The goal holds no earlier than the latest of its
facts' first achievements. Consistent: an advance by δ lowers every D by at most δ, so h falls by at most δ; a firing
of an enabled o reaches s′ whose facts all have T = 0 in the relaxation from s through o at time 0 (o's T_o is 0), and
whose reset clocks give due times that the relaxation from s gives through o too, so T from s is pointwise at most T
from s′. ∎

This is Haslum & Geffner's (2001) admissible makespan heuristic in its h^1 form, where parallel waits count once.
Delete relaxation drops the facts an operator overwrites and the order of the input chain; both only lower h.

With a consistent h, the f-values A* pops do not fall, so the last f popped before a stop on states, memory or CPU is
a lower bound on L, and so on OPT.

## Pick windows

**Lemma T6.** An operator whose effects read pick variables (R(o) of the C1 proof, taskcore README "Pick windows")
fires as its window: one firing per assignment of R(o)'s values, the effects read with that assignment, every pick
variable 0 before and after. *Proof.* Theorems 1 and 2 of the C1 proof map the plans of the file with free picks and
of the file with windows both ways with the same operators outside the picks, in the same order; a window changes no
fact and its begin and picks take no time, so the timed runs map the same way at the same times. A window is atomic
(C1's Lemma 2), so it is one firing here. ∎

## What the reductions keep

The fold (strict) groups paths with the same label, kind, end, continuation and frame cost, so a folded operator has
its paths' bound. (a) drops effects that cannot change the state; one of them may be a `pos:U` write to the value it
has, whose clock reset the model then misses: the slot's clock is then at least the engine's, which keeps T0. (b), (c)
drop facts nothing reads and actions with no effect: an operator with no effect only resets clocks and moves e_in, and
never makes anything earlier. (d) and (e) keep every plan from the start, and every optimal plan whole, as sequences;
the timing of a sequence does not depend on what they drop beyond clock resets, so the least time is kept. (f) keeps
the least frame cost among duplicates and dominated actions, but two actions of different labels can have different
bounds for one frame cost, so `timed` leaves (f) out (its default reductions are a to e).

## The tests

On the bar door and the kitchen (from s17, from run B's S1#7 start on seed 1, and from the harness's boot), L is at
most the game time of every engine route found from the same start (window2's runs, run B's recorded inputs), and
L/U is reported. Kill: more than 10^7 states or 8 GB on the kitchen; or any engine route with less game time than L.
