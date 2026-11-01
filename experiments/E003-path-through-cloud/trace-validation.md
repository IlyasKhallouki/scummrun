# Trace validation with the engine's explanation (plan step B1)

The model claims to over-approximate the engine on V (assumption A1). B1 tests that claim on every run the system makes:
each piece of script work the engine does is matched against the reading's records, and where the engine leaves a
record, the engine itself says where and on which value. This note states what each part guarantees, for any SCUMM v5
game under the bridge. It was written before the code (`trace_check.py`, the bridge's `segment` and `explain`
commands).

## Setting

- E, K, P, C and F are as in `fork-exact.md`: the patched engine under a pinned configuration, a parent branch stopped at
  a decision point, its forked child, and a fresh engine with the same inputs. A0 holds (measured there).
- A *decision point* is the bridge's stop in a frame (`SpeedrunBridge::onDecisionPoint`, scummLoop right before
  `checkAndRunSentenceScript`). D_k is the decision point of frame k. The engine's facts at D_k are `facts_of` of the
  state dump there, and the reading's view of them is `replay.view` (`window.FastView`).
- The reading's records (reader.py `Record`): an *event* record event(U@W) is one path of unit U's slot from wait point
  W; an *input* record is one path of an input (a sentence, a walk, a dialog line, Esc). A record has a precondition
  pre(r) (key → set of values), checks (conditions on non-facts with the side the path took: cite, condition, side), its
  effects (key → a set value, a guess among values, or a step), its end (how the path stopped) and a frame cost (the
  task's action cost, a lower bound in frames, `costs.rs`).

## 1. The engine's segmentation

**Slot run.** One call of `ScummEngine::executeScript` (script.cpp:494-519). Its slot s is `_currentScript` at the
call; its unit is the slot's (where, number) and, for a local script or the room's entry and exit code, the room
resource `_roomResource`; c0 is the code offset it starts from (`scriptCode(s, offs)`: the `[XXXX]` offset of the
decompiled scripts, the W of the `at:` facts); c1 the slot's code offset when the call returns, if the slot is still
live; its status at the return (dead, running, paused by a delay; frozen when the freeze count is above 0); the number of
opcodes it executed itself; its depth (`vm.numNestedScripts` at the call); its frame k and whether it ran after D_k.

**Lemma S (segmentation).** Every opcode the engine executes is executed by exactly one slot run, the innermost call of
`executeScript` on the stack, and every opcode of a run belongs to the run's slot.

*Proof.* Opcodes are fetched and dispatched only in `executeScript`'s loop (`executeOpcode` has no other caller). The
loop runs while `_currentScript != 0xFF`. `_currentScript` is given a slot index in three places only: in
`runScriptNested` just before its `executeScript` call (script.cpp:369), in `runAllScripts` just before its call
(:980), and in `runScriptNested` after an inner run returns (:384), where it gets back the slot that was running when
the inner call was made, that is the slot of the run whose opcode made it, and only if that slot still holds the same
script, is live and is not frozen. Every other assignment sets 0xFF (script.cpp, script_v5.cpp, room.cpp, actor.cpp,
scumm.cpp), which ends the loop at its next test. So within one call, every opcode fetched is of the call's slot, and
the opcodes of an inner call are the inner run's. ∎

The bridge checks the premise as it runs: each opcode it logs is of the run's own slot, and it counts violations.

**The runs between two decision points** are the runs of frame k after D_k and of frame k+1 before D_{k+1}. The facts at
D_k and D_{k+1} are their pre and post states.

## 2. The bridge's controls (off by default)

- `{"cmd": "segment", "on": true}`: from the next frame on, the bridge records every slot run (section 1).
  `{"cmd": "runs"}` returns them and clears the buffer. Off (the default, and `"on": false`), nothing is recorded.
- `{"cmd": "explain", "from": F0, "to": F1, "units": [[where, number], ...]}`: in frames F0 to F1, the runs whose slot
  holds one of these units (none given: every run) log what the engine's own debug channels would print at the same
  call sites: each opcode with its code offset (DEBUG_OPCODES, script.cpp:508), each `readVar` with the variable and the
  value it returns (DEBUG_VARS, :571, with the value added), each `writeVar` with the variable and the value (:714).
  `{"cmd": "explain_log"}` returns the log and clears it. Both buffers are bounded; what does not fit is counted.

The engine gains three hooks in script.cpp: the start and the end of `executeScript`, each opcode before
`executeOpcode`, and the value `readVar` returns (`readVar` calls its old body, now `readVarBody`, and passes the value
on). With the bridge off (`SPEEDRUN_OUT` unset) each hook is a null-pointer test. With the bridge on and both controls
off, each is a test of one bridge member.

**Lemma O (the controls observe only).** With `segment` or `explain` on, the engine's game state at every decision
point is the same as with both off.

*Proof.* The hooks read engine members and call `const` member functions of the bridge (`scriptCode`, `scriptStart`,
which read the engine's tables and raw resource pointers without touching the LRU counters, as the state dump does).
They write only the bridge's own buffers and counters, which game logic never reads. `readVar`'s value is returned
unchanged, and its body runs exactly once either way. The commands run only while the engine is stopped. ∎

*Test* (`trace_check.py check-off`): three engines from boot in lockstep, one frame at a time, the same inputs: the
shared build, the new build with the controls off, and the new build with `segment` on from boot and `explain` on for
every run of a span of frames. At every frame the stop status and the full state dump (with `SPEEDRUN_DUMP_LOCALS` and
`SPEEDRUN_DUMP_EXACT`) must be equal, and at the end the three `trace.jsonl` files.

## 3. The strict matcher

For the runs between D_{k-1} and D_k, with V_{k-1} and V_k the views there:

**Definition M.** A run ρ of unit U with c0 = W matches the event record r = event(U@W) when

- (M1) r's unit is U and its wait point is W;
- (M2) pre(r) holds on V_{k-1} after the effects of the records matched to the earlier runs between the same decision
  points, in run order;
- (M3) every check of r that the engine can decide holds: the side the record took is the side the engine took. The
  engine's side is read from the state at D_{k-1} (`replay.eval_check`, which already decides most checks), or, with
  `explain`, from the run's own opcodes: the instruction after the check's cite tells which way the branch went. A
  check neither can decide is counted as open, never as held;
- (M4) r's effects match: its slot's write `pos:U` is c1 (or NOWHERE when the slot is dead), and every other effect
  key holds its value in V_k, unless a later run between the same decision points writes that key.

ρ is *explained* when exactly one record of U at W matches. It is *ambiguous* when more than one does, and
*unexplained* when none does. A record whose writes already held and whose run did not happen matches nothing: a match
needs its run (M1). An input record matches the runs between the decision point before its input's frame and the one
after it on the units its `pos:` effects name, with M2 on the view before and M4 on the view after.

**Lemma M (what a match says).** If every run of a trace is explained, then on the facts the view holds, the trace is a
run of the task: each run is a step of the record it matched, applied in run order. An unexplained run is a
counterexample to A1 on those facts, or a run the matcher's grain cannot place (a run that the reading folds into
another record's path, such as a nested run inside a record). It says nothing on non-facts other than the checks it
decides, and nothing on runs that were not made (Cirstea et al. 2024: partial traces can be accepted wrongly).

*Proof.* M1 ties each record to one run; M2 and M4 are the record's precondition and effects evaluated in the order the
engine ran them, so the sequence of matched records is applicable from V_{k-1} and ends in V_k on every key it writes
that no later run writes. ∎

**Cost.** The matcher indexes the records by (U, W) once. Per run it looks up the candidates and evaluates their
preconditions and effects on the view's keys: no fork, no frame. Its cost per call is measured on the window runs and
reported with the results.

## 4. Locate

**Lemma L (bisection over forks).** Let P be a predicate on the engine's facts at a decision point, with P(D_lo) true
and P(D_hi) false. From a branch at lo, bisection forks a child, runs it to mid = ⌊(lo + hi) / 2⌋ and evaluates P
there; if P holds, the child becomes the branch at lo = mid, else the child is dropped and hi = mid. After
⌈log2(hi − lo)⌉ forks it returns k with P(D_{k−1}) true and P(D_k) false. If P is monotone on [lo, hi] (true up to
some frame, false after), k is the first frame where P fails.

*Proof.* The invariant P(D_lo) ∧ ¬P(D_hi) holds at every step, and hi − lo halves. A child is the run with the same
inputs (A0), so P evaluated in it is P of that run. ∎

The predicates used: (a) a fact f stays at its value before the move or at the prediction's value; (b) the unit U whose
record predicts f is live at a wait point from which a write of f is reachable in U's code (section 5), or has not yet
started. (b) is monotone while U is not started again: an unreachable write stays unreachable.

## 5. Explain

In a child at D_{k−1}, `explain` is armed for frame k and the unit U the located frame names, the child runs one frame,
and the log is read. Then:

**The record's path.** A record's path through U from W is an instruction sequence of U's IR (ir.py: `next` and
`target`) that takes each of the record's checks on its side and stops at the record's end: its `pos:U` write W1 (the
instruction after a breakHere or delay, or a WaitFor opcode) or the slot's end. A write site of a fact f is an
instruction whose IR statement writes f (an assignment to a variable or a bit, setState, setOwnerOf, setClass,
pickupObject, a start or a stop of a script, a yield at W for `at:U:W`).

**Lemma X (the first opcode off the path).** Walk the run's logged opcodes in order alongside U's IR. The first
conditional jump J the engine executed whose taken side cannot reach the record's end (or, for a predicted write, any
write site of f with that value) without passing another yield, while its other side can, is the first opcode where
the engine left every path of the record. If no such J exists and the slot is live at a wait point from which the end
is reachable, the engine has not left the path: it has not got there yet (a wait the prediction ran past).

*Proof.* Reachability is over-approximated by the IR's control flow, which contains every way the opcodes can go
(ir.py's `next` and `target`, checked against the bytes). Before J, the end was reachable from every executed
instruction; after J it is not, so no continuation of this run follows the record's path. ∎

**Lemma B (the backward slice).** J's condition is an IR expression over constants, variables, bits, locals and
getters. Its value in the engine is:

- for a variable or a bit: the value `readVar` returned for it while J executed (the explain log);
- for a local: the same value; it was last written by the slot itself, by an instruction of U that assigns that
  local, or by the arguments at the slot's start (in v5 a local is written only through `writeVar` on the running slot,
  script.cpp:846, or by `initializeLocals` at the start, script.cpp:155-164). The slice follows the log back to that
  write and, when its value came from a getter, names the getter;
- for a getter the opcode evaluates itself (an actor's x or y, a box, an object's state or class, a distance): the
  value from the state dump at D_{k−1} (actors' x, y, elevation and moving state; objects' states and classes),
  since DEBUG_VARS does not show it. When the getter's value was written into a local or a variable inside the run,
  the log's `writeVar` gives the engine's value at that instant, and the two are reported side by side.

**The two classes of cause.**

- *split* (CEGAR, Clarke et al. 2003): the value the engine tested is not a fact of the reading (a position, an actor's
  state, a random draw, a clock, a local the reading does not know), or is the time a slot still had to wait. The
  record lumps together engine states that this test tells apart, and the job must split on that value.
- *semantics*: the value the engine tested is a fact, and its value at D_{k−1} is one the record's precondition allows
  or the prediction gives. The model computed that opcode, or an earlier one whose result the test reads, differently
  from the engine: one fix in the reader.

A test whose fact the engine had at a value the model did not expect is neither: the cause lies in the run that wrote
it, and the explanation follows that write.

## 6. The wait check

**Lemma W (frame costs).** For an event record r = event(U@W) matched to a run in frame k, let j be the frame of the run
that left U's slot at W. The engine waited k − j frames. A2 (every cost a lower bound) requires k − j ≥ cost(r). The
check runs on every matched event of every validated run; a wait below its bound is a counterexample to the rule that
priced r.

## 7. What is checked

- run B's recorded inputs on seed 1, the moves of the model check (check.py, row 5 with fix round 1's switches): every
  move the model rejects and every certain prediction the engine refutes gets a located frame, an explained opcode and
  the engine's value, or is reported without one, with the reason;
- the window search's runs (kitchen from boot and from run B's S1#7 pre, the bar door, k = 2) and their replans,
  replayed from boot with `segment` on: every run between the decision points the plan's steps cover, strictly
  matched;
- B1's gate: at least 90% of run B's rejected moves and refuted predictions get an automatic cause. Fewer than half: the
  grain is wrong.

## 8. Off

Every new control is off by default; the bridge with both off writes the same trace, status and dumps as before
(section 2's test). `trace_check.py` is an instrument: nothing the system runs imports it. It changes no reader, task
or planner output.
