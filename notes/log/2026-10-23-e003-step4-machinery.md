# 2026-10-23: E003 step 4, the planner's machinery, built before the gate

Question: can the parts of step 4 be built and checked on hand-made tasks while step 3's gate is still
open, so that the search can start as soon as the gate passes? No search ran on the Part I task.

Built (all new files in `experiments/E003-path-through-cloud/`):
- Fast Downward release 26.6.0, commit 7ea2755, built with `./build.py release -j8` in
  `third_party/downward` (14 min, cmake 4.4.2, g++ 16.2.1). lama-first solves a hand-made two-action
  task, and its landmark graph is read from the debug output (`results/task-build.log`).
- `domains.py`: the finite domain of each fact, from the IR, following plan step 1's rule.
- `task.py`: the planner's task, and the writer for Fast Downward's finite-domain input.
- `relevance.py`: works backward from the goal.
- `plan_fd.py`: runs lama-first under a time and a memory limit, then parses the plan and the landmarks.
- `task_fixture.py`: a coarse fixture task built from the IR.
- `test_task.py`: 21 tests on hand-made tasks, all passing.

Domains, from the engine's initial state (`results/task-domains.log`):

| | count |
|---|---:|
| variables written or compared in the scripts | 246 |
| with a finite domain | 159 |
| of those, collapsed into "above" or "below" | 47 |
| of those, collapsed and also read other than by a comparison (lossy) | 8 |
| with no domain | 87 |
| values per finite variable, median / max | 2 / 107 |
| indexed variable writes whose index does not fold | 43 |

The 87 variables with no domain, by reason: a copy of another variable 20, a value or step from another
variable 21, a local its own script writes 14, a getter 9, a counter never compared 5, an engine-bound or
unbound local 5, written by the engine (clocks, mouse, camera, message state, talk timer, text speed) 11,
fixed by the settings 1, other 1.

The coarse fixture has one action per object verb entry. Each action's effects are the constant writes its
code can reach, and its reads are every fact that code compares. It is flow-insensitive and is not the
task (`results/task-fixture.log`):

| | count |
|---|---:|
| actions | 1,393 |
| actions relevance keeps (goal: the Part I end) | 627 |
| facts named | 955 |
| facts needed | 469 |
| SAS file, kept actions | 1,024 variables, 1,810 operators, 2.8 MB |

Fast Downward reads that file in 0.5 s. The writer gives byte-identical output under two different
PYTHONHASHSEED values.

Found while building:
- The RHW landmark factory stops on any value name that is not `Atom pred(...)`
  (landmark_factory_rpg_sasp.cc:359-376).
- Fast Downward rejects an operator with no effects.
- Encoding every unknown write as a product of variants does not scale. On the fixture, one entry needs
  about 10^66 variants. Above 64 variants per action, the guesses now go through pick variables, which
  makes the size a sum instead of a product. 198 of the 627 kept actions use them.
- The plan's "up to the largest compared constant" leaves no lower bound for a counter that is stepped
  down. A "below" value, under the smallest start value, constant written or constant compared, covers
  this for 3 variables.
- With no debug flag, VAR_DEBUGMODE is 0 (scumm.cpp:3389-3392), so 759 instructions behind decided jumps
  are left out. Without this, VAR_EGO has no domain: the input script's debug actor switch writes it from a
  local.
- Plain backward relevance over values is not sound once there are conditional effects. A test shows a
  case where it drops a needed action. relevance.py treats facts read by effect conditions as needed in
  full.

Not done yet:
- There are no real actions yet. The reader's symbolic oracle is not built, so the fixture's counts are an
  upper bound on what relevance keeps from the real task.
- Domains are built from the engine's initial state, not from step 2's start. Owners and states start
  from the index, so they will change. A start value outside its domain raises OutOfDomain, and the task is
  then rebuilt.
- A ban from step 5 adds a precondition, which needs the operators written again. Only a replan with no
  new ban changes nothing but the initial state.
- 37 hook-variable writes do not fold. Most restore a saved value. They are assumed to start no unit
  that the constant hook writes do not already name.
- 43 indexed writes have targets that are not worked out. These writes are assumed not to hit the 40
  finite variables within 16 of their bases.
