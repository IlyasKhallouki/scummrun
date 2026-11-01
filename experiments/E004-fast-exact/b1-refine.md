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
