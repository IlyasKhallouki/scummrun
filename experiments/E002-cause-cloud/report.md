# E002: The cause cloud, report

Date run: 2026-10-22
Commit: 77317e8 plus the uncommitted E001 and E002 files; recorded again when they are committed.
Commands, in order: `census.py`, `prefill.py`, the engine-table workflow (below), `cloud.py`.
Raw output: `results/` (`census.json`, `prefill.json`, `engine-opcodes.json`, `engine-triggers.json`,
`cloud.json`, `cloud.log`).

## Result

**Opcode census.** All 39,663 lines in MI1's decompiled scripts map to ScummVM's v5 opcode table.
MI1 uses 86 of the table's 101 o5 functions.

**Engine table.** Model agents classified every v5 o5 function from the ScummVM source: 13 groups,
each checked by a skeptic. They produced 211 rows (one per sub-operation for the multi-purpose
functions), and the skeptics made 266 corrections. Five more agents, each checked by a skeptic, listed
the engine's own trigger sites: 563 sites (302 generic v5, 77 game-specific, 57 false shortcuts,
127 other versions only) and 148 cost knobs. A final critic added 24 sites and 6 knobs. Every row cites
source lines. `check_cites.py` resolves 6,716 of the 6,719 citations to lines that exist; the other 3
name `engine.cpp`, which matches more than one file. The check confirms the cited lines exist, not
that they say what the row claims. `cloud.py` reads these fixed files; no model runs in it.

**The cloud.** One row per added cause kind, cumulative. "Key kinds" counts the facts of the kinds E001's
key holds that are left after the key's travel and engine-bookkeeping rules. That is the number a size
cap applies to.

| Step | Facts | Key kinds | Writers per fact, mean (median) | Triggers per fact, mean (median) | Part I goals with a cause (exact value) | II-IV (exact) |
|---|---:|---:|---:|---:|---:|---:|
| Verb handlers, calls 4 deep, ScriptIndex's kinds | 2,813 | 2,353 | 17.2 (1) | 14.2 (5) | 97/99 (87) | 70/71 (67) |
| + the engine table's other kinds | 3,234 | 2,353 | 15.6 (1) | 15.3 (5) | 97/99 (87) | 70/71 (67) |
| + calls followed to the end | 3,285 | 2,400 | 15.4 (1) | 16.4 (9) | 99/99 (87) | 70/71 (67) |
| + room entry and exit | 3,536 | 2,615 | 14.6 (1) | 18.7 (7) | 99/99 (87) | 70/71 (67) |
| + input scripts (clicks, keys, dialog) | 3,587 | 2,648 | 14.5 (1) | 20.9 (6) | 99/99 (87) | 71/71 (68) |
| + cutscene and scroll hooks | 3,588 | 2,648 | 14.5 (1) | 20.9 (6) | 99/99 (87) | 71/71 (68) |
| + the boot script | 3,780 | 2,760 | 14.0 (1) | 20.4 (7) | 99/99 (87) | 71/71 (68) |
| + engine-written variables | 3,822 | 2,760 | 13.9 (1) | 20.2 (6) | 99/99 (87) | 71/71 (68) |

- A **writer** is a script line that writes the fact. A **trigger** is a root that reaches a writer: a
  verb handler, room code, an installed hook or the boot script.
- A goal "has a cause" if some writer writes its value, or a value known only at run time. "Exact"
  counts the first kind only.
- Call depth: 1,469 facts with no call followed, 3,770 at depth 4, and all 3,822 by depth 8. The longest
  chain from a root is 9 calls.
- Part I, by root kind: verb handlers reach 99 goals, the boot script 72, room code 53 and input scripts
  48. 19 goals are reached only from verb handlers, and no goal needs a non-verb root.
- The boot count is inflated. The cloud ignores conditions, and the boot script has a branch that runs
  only with a boot parameter in debug mode. That branch alone writes 31 of the 99 Part I goals and 20 of
  the 71 in Parts II-IV.
- 14 global or local scripts are reached from no root.
- For the cap: the key has 170 goals and the cloud 2,760 facts of the same kinds, so a cap of 340 is
  12% of the cloud.
- The index rule (below) is the loosest of the reading rules. With it set to offset 0 only (a one-off
  check), the cloud has 3,629 facts, 2,568 of them in key kinds.

**Line coverage.** Of the 11,883 script lines whose explicit target is a kind the cloud tracks, the
extractor reads 10,539, reads 218 with the subject unknown (an object or actor given by a variable) and
misses 1,126. The misses:
- 619 verbOps lines on a verb held in a variable. 573 of them create dialog lines (a text and On), so the
  cloud has no fact for a dialog line being shown. (An earlier version of this report said these lines
  only change colors or positions; that was wrong.) The writes a dialog line leads to are still in the
  cloud, through the conversation script;
- 160 ActorOps and 46 putActorInRoom lines on an actor held in a variable;
- 154 cursor lines;
- 100 doSentence lines with variable arguments (the input script turning a click into a sentence, which
  the verb roots already cover).

Not in those numbers (5,815 lines):
- 4,000 lines whose only tracked write is an engine variable (print setting the message variables,
  override, cutscene, camera and room effects). The engine root covers these variables.
- 1,694 animateCostume lines, which can turn an actor. That is a facing change the extractor does not read.
- 98 stopSound and 23 saveVerbs/restoreVerbs lines, which the extractor does not read.

The engine table also lists side effects the extractor does not model, for example pickupObject also
setting an object's class and state, and loadRoom resetting actors.

## Reading rules added

All of them are general, and none names an MI1 number. They are listed in `cloud.py`'s docstring:
- writes through a variable index fan out over the values the index can hold;
- a local-script call where the caller's room is unknown, or has no such script, goes to every room that
  has it;
- script numbers held in variables resolve to their bound or assigned constants;
- a script keeps the set of rooms it may be in after `loadRoom` (grow-only);
- `Resource.loadRoom` is not a room change;
- `startScript` with two flags is a call;
- expression values are unknown values.

## Against the prediction

There was no prediction (descriptive step). Against the plan's usability checks:
- every opcode byte maps and has a classification;
- every Part I goal has a cause;
- no rule names an MI1 number.

Deviations from the plan:
- The reading rules above were added during the run. Each was added to close a gap found on a Part I
  goal (the insult bits, the pirate-fight selector, the safe digits, the beast demonstration) or a
  parsing error. Held-out results were read as totals only.
- The table's first row already uses the new reading rules; only its kinds are ScriptIndex's. An
  earlier run of the same code without the rules gave 2,458 facts in that row.
- Line coverage is reported per o5 function, not per kind. A function's table row lists every kind it
  can write, including side effects, so a per-kind count overstated the misses.
- The engine table was written by model agents and checked against the source by other agents. The plan
  said "reviewers"; these were they.

## Surprises

- **The cloud has a cause for every goal.** With all roots, all 170 goals have one, and every Part I goal
  is reachable from a verb handler once calls are followed to the end. This does not say whether a
  backward search reaches them, which is E001 step 2's question, and the raw cloud contains most of the
  key just as the dump did.
- **Five bugs in `ScriptIndex`,** the reader the route tools and E001's baseline use:
  - `startScript(n, [...], F, R)` with two flags is never followed (59 lines);
  - `Resource.loadRoom(n)` is read as a room change (5 lines);
  - writes through a variable index, or with an expression value, are dropped (660 lines have a
    variable index);
  - a local-script call is looked up only in the caller's room, even after the script loads another room;
  - script numbers held in variables are not followed.

  E001's key does not use this reader. It is built from the route and its own script reading.
- **A heavy tail.** The median fact has one writer, but the mean is 14 and the largest is 1,515. Scratch
  variables and the index fan-out account for most of it. The median fact is reachable from 6 triggers.
- **Randomness.** The trigger critic found that ScummVM seeds its random generator from the clock unless
  `random_seed` is set, and that the seed is not saved. A route that depends on a draw depends on the seed.

## Decision

D3 left the size cap until the cloud was counted (2,760 facts of the key's kinds against 170 goals).
D4 drops the cap: the next step searches the full cloud for a path from the start of the game to its end.
