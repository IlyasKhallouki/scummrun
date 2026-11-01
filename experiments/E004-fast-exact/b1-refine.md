# B1's causes, found again in the engine and refined in the reader

Plan step B1's Refine. B1's trace validation (`experiments/E003-path-through-cloud/trace-validation.md`) gives each
miss a cause: the first opcode where the engine left the record's path, and the value it tested. Each cause of class
semantics is found again in the engine before anything changes: a child of the run's engine at the cause's frame, the
bridge's `segment` and `explain` controls, and the state dump. Then it gets one of three outcomes:

- an opcode or a rule of the reader whose abstract semantics are wrong: one reader switch, off by default, with its
  lemma here before its code;
- a value the job must split on, or the time a slot still waits (class split): listed for the window search, not
  changed in the reader;
- the reader is right and the matcher's grain is not: said so, with what the matcher needs.

Every rule below is stated for any SCUMM v5 game under the bridge. MI1 numbers appear only in the tests and the logs.
With every switch off the reader writes the same bytes as before.

## 1. A path that changes nothing (`noop_records`)

**What the engine shows.** A unit loops on a test of a variable that only a room load sets and that the loop itself
clears. Its event records at the loop's wait point all ask the variable's set value. In the engine the variable is
already clear at that wait point on every frame after the first: the first run clears it (it starts nested in the room
change, from the code's start, and is not a matched run). The run then takes the other side of the test, writes no fact
of the task and stops at the same wait point. The room load's semantics are right: the machine writes the variable as
{old, 1} (resource.cpp:640-644, `eng_scene_load`), and the engine has 1 before the change and 0 after the first run.

The reader read that path too. `Builder::record` makes its record and returns none, because the record's effects are
empty: every write the path made to a fact of the table either sets the value its precondition already pins (its own
place, its list of locals) or goes to a key that is not a fact of the table. So the reading has no record of the run
the engine makes most often, and the matcher, which needs one record per run (Definition M), calls the run unexplained.

**The rule.** With `noop_records` on, a path whose record has no effect after `record`'s own filter is written as a
record with that precondition, those checks, no effects, its end and its job's cost, in place of being dropped. The
reader's counter `no-op paths (no effect)` then counts the records kept this way. With the switch off nothing changes.

**Lemma N.** Let r be such a record. Then:

1. *The task's behaviour is unchanged.* For every state s of the task with pre(r) ⊆ s, applying r gives s. So r adds
   only self-loops to the task's transition system: the states reachable from the start are the same, and every plan
   that uses r is a plan without it once its r steps are removed, with a cost lower by at least one frame each
   (`costs.rs` prices every record at 1 frame or more). The optimum and every bound on it are the same. The reached
   values of `--reach` do not change, since r writes nothing, and r's precondition is one the reading already met
   (the path ran).
2. *Its engine run is a step of r.* An engine run that follows the path from a state whose facts meet pre(r) and whose
   non-facts take r's checks' sides writes, among the task's facts, only keys and values the path wrote, and those
   equal their values before (the filter); its slot stops at the path's end. So r's precondition holds before the
   run, r's effects (none) hold after it, and Definition M's M1 to M4 hold: the run is explained by r, or is ambiguous
   with another record of the same point.
3. *Off is byte-identical.* With the switch off `record` returns none as before, and no other code reads the switch.

*Proof.* (1) r has no effect, so s·r = s, and a self-loop of positive cost is never on a cheapest path; reachability
and reached values are closures over effects. (2) The filter removes a set effect only when the precondition pins the
same single value, and keys outside the table are not task facts; the end is the path's. (3) By construction. ∎

**What it costs.** One more record per no-op path, each a self-loop of the planning task (1).

**Test.** Off: the kitchen's packed sha1 279b7a24 and the five check scopes' sha1s. On: B1's window validation, with
the unexplained runs of the group before and after; the five check scopes' records, CPU and peak; the reached values
the same.

## 2. A second instance of a recursively started unit (`ghost_slots`)

**What the engine shows.** A unit's loop starts another unit U with the recursive flag, so `runScript` skips
`stopScript` (script.cpp:65-66) and U gets a second slot while its first instance still runs. The reading keeps one
place, one list of locals and one live flag per unit name. Its start record writes the new instance's start over U's
slot facts, and the older instance's later runs then fail the records' precondition on U's place or list, and its end
fails the record's effect "U not live", since the newer instance is.

The reading already follows the newest instance: a recursive start writes U's slot facts with the new start, and a
start without the flag stops every instance first (script.cpp `stopScript` kills every slot of the number). So the
reading's abstraction is α: U's slot facts are those of U's most recently started instance (live or ended). The older
instances are ghosts, which α does not see. That is sound only if nothing the task holds can tell a ghost is there.

**The rule.** With `ghost_slots` on, the reader decides for each unit U that some code of the scope starts with the
recursive flag (a constant number, or any script unit when the number does not fold) whether U's ghosts are invisible:

- (a) the code a ghost can run writes no fact of the table but U's own slot facts: the havoc closure (havoc-cut.md,
  "The write set") of U from its first instruction has an empty fact set, touches no slot but U's, changes no room,
  and has no `freezeScripts`, cutscene or override op;
- (b) no instruction of the scope reads U's liveness: no `isScriptRunning` whose argument folds to a set holding U's
  number, or does not fold. An argument folds when it is a constant expression, or a variable whose domain in the
  table is a finite set of numbers (a variable's value stays in its domain: the domain is its start value closed
  under the scope's writes, and the reader widens it when a write falls outside).

It writes the verdict per unit (invisible, or the condition that failed) into the reading's head and report. The
records do not change. A reading with the switch states which units it follows by α; the trace validation then
applies α to the engine (below).

**Lemma G.** Let U satisfy (a) and (b), and let α map an engine state to the task's facts with U's slot facts taken
from U's most recently started instance only. Then every engine run of a ghost of U leaves α unchanged, and every
other engine run changes α as it did before.

*Proof.* A ghost runs U's code from a point that U's code reaches from its first instruction, so every instruction it
runs, and every unit or hook it starts, is in U's closure from its first instruction (havoc-cut.md's closure is over
the control flow, the starts and the hooks, so it contains every continuation). By (a) none of them writes a fact of
the table other than U's slot facts, and α reads U's slot facts from the newest instance, which the ghost's run does
not touch: its own slot is another slot (`getScriptSlot` gives each start a free slot, script.cpp:73-76). So α is the
same before and after the run. A run of any other code reads U only through U's liveness, and by (b) no instruction of
the scope reads it, so its path, and with it its writes, do not depend on whether a ghost lives. `stopScript(U)` and a
room change end every instance, ghosts included, and α's U ends with them; `freezeScripts` freezes a ghost as it does
the newest instance, which delays only the ghost's runs, and those change nothing α holds. ∎

So under (a) and (b) the reading's records, with α, cover every engine run: the newest instance's runs are matched as
before, and a ghost's runs are stutters of the task. Where (a) or (b) fails, the verdict says so and the reading makes
no claim on that unit's ghosts (an open item for B2's recursive starts).

**The matcher's part** (`trace_check.py window-validate --ghosts`, off by default, with a reading that has the
verdicts): for each unit the verdicts name, the newest instance is the slot of the unit's latest run from its first
instruction in the segmentation; a dumped slot of the unit that is another slot is left out of the view (α), and a
run on such a slot is a *ghost* run, not matched against records. On a trace, α is checked run by run, whatever the
verdict: every ghost run is explained (its logged opcodes and writes), and one that writes a fact of the task, or
runs an op that writes an object, a verb, a script's life or the room, is a cause. So a trace whose ghost runs all
pass is a run of the task under α even where Lemma G's static conditions fail; where they hold, every trace is.

**Test.** Off: the five check scopes' sha1s and the kitchen's 279b7a24 (the records do not change with it on either).
On: the verdicts on the window runs' readings; B1's window validation before and after.

## 3. The input script on run B's moves (no switch: B4's work)

**What the engine shows.** Three refuted predictions of run B's check (row 5) have their first opcode off the path in
the plain input script, the global that ends in doSentence(verb, A, B) of three variables. Row 5 runs a sentence by
starting the sentence script with (verb, A, B); the engine's click runs the input script first, and that script
decides whether a sentence is queued and which. Found again in the engine:

- a click whose point is over another object than the scored action's, with the default verb: the input script
  queues a walk to that other object. On this seed the click is another input of V than the scored one; the model is
  not at fault there, the sample is (the move is out of sync);
- a click whose point is over no object: the script walks ego to the mouse point. That is a floor click, outside V;
- a verb that takes a preposition with one object (its first click on an inventory icon): the script starts the
  preposition script, which sets the preposition variable, and then queues no sentence while the second object is
  missing (its local "a sentence is complete" stays 0). Row 5 runs the sentence anyway. This one is a semantics
  error of the input rule.

**Does input_settled cover it?** No. F1c (`inputs.rs`) starts an input at the room's input script only when that
script is not plain; a plain input script keeps today's job, the sentence run directly ("today's sentence record is
what it does"), and an input with an inventory object stays as it is. The third case is an inventory input under a
plain input script, so the switch reads it as row 5 does. A reading from that move's start with input_settled on has
no record of the input at all (its object is not one of the room's).

**What a fix needs.** The plain rule is not a proof: the plain script has paths that queue no sentence, or another
sentence, depending on the verb's preposition, the hovered object's default verb and owner, and a settings bit. A
sound rule runs the plain script too, from the sentence variables and the click's area (scene or inventory) as
constants, as F1c does for the other input scripts, and keeps its records apart from the sentence's (F1b's records,
D10). That is plan step B4, which B1 now asks for; it is not built here.

## 4. Two run B causes that are the done frame (class split, for the window search)

Both were classed semantics by B1's first explanation. Found again in the engine, each is the done frame against the
model's settled end, which is class split:

- **Cursor and userput soft-on.** The model's last writer of the cursor and userput variables was taken to be the
  cutscene-end script's soft-on ops. In the engine those ran, and in the done frame a room change ran the new room's
  first local script, which turns the cursor and userput off, then waits. One frame later that script resumes and
  turns them on. The model's settled end has them on, which the engine reaches one frame after the done frame.
- **A room change into a pseudo-room of the same resource.** The model predicted the entry code no longer waiting at
  its first breakHere. In the engine, the done frame is the room change: the entry code starts again and waits at
  that breakHere, and it ends one frame later. The model's settled end is that later state.

So neither opcode's abstract semantics is wrong; the time the slot still waits at done is the value to split on, as for
B1's other 20 split causes. They go to the window search's list, with the others: WaitForCamera that does not block,
polling loops on positions, delays, and the done frame before the write.
