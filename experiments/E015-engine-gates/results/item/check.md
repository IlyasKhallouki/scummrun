# E015 item gate: independent check

Checked 2026-11-04 from e015-item at 5721df9, in a separate worktree, a separate build
(`CARGO_TARGET_DIR=/var/tmp/fast-mi/target-e015-item-chk`) and a separate output folder
(`/var/tmp/fast-mi/e015-out/item-chk`). The environment is `env.sh` with only `E015`, `FMI_ROOT` and `OUT` changed. At
most 2 engine processes ran, under `nice -n 10`.

## Verdict: reproduced; the PASS holds

The engine now shows the room-42 half directly, not only through the scripts: carried into room 42, object 946 runs
local 212 and is consumed.

## What was rerun

| | gate | check |
|---|---|---|
| step 1 | room 63 at 1392, idle 1405, rng 1523492811 | same |
| step 2 | 95 trials; 6 to room 69 (all object 854), 6 to room 49, 78 stay, 5 `error` | same; trial log identical line for line apart from timings |
| step 3 | 135 trials; 1 `v11t946`, owner 15 at 126, 8 `error`, 6 to room 63 | same; trial log identical |
| step 4 | 0 of 3,850 facts differ, room 69, digest 0babbc2cb2c1a1de | same |
| step 5, static | 3 literal starts, 6 indirect starts, 1 clear | `summary-5s.json` identical |
| step 5, engine | 0 started, live, cleared or ran in 30,000 trials | `summary-56.json` identical apart from timings; all 10 trial logs identical |
| step 6 | no stop in room 75 | same |
| detector control | local 213: live 2, ran 2, started 0; local 202: 0 | same |

- **Wall time.** The sweep took 298 s, against the gate's 136 s, because a second engine process was running beside it.
- **The step-5 entries.** The rule of amendment (1) gives 302, 324, 341, 358, 456, 556, 573 and 595 from play8's
  `entries.jsonl`. 136 room-42 entries have `class:491:6 = 1`, and all 136 have `class:491:12 = 1`. At all 10 chosen
  entries, owner:946 is 15.
- **Order of commits and runs.** Amendment (1) was committed at 13:42:39, and the tool and amendment (2) at 13:46:42.
  The tool's binary was built at 13:46:30, and the first output file was written at 13:46:58. The criteria in
  `results.md` are the plan's.
  - One limit: an earlier run whose outputs were overwritten cannot be ruled out from the files.
- **Hard-coded numbers.** `item.rs` has no game numbers. Its only numeric literals are the limits (4, 5,000, 2, 3,000
  and 10). G1's and G2's items do not apply to this gate.

## The claims about object 946 and room 42 in the engine

The gate showed only the negative half in the engine: no input starts local 212 without object 946. To test the
positive half, the check took 946 into room 42 and searched there. The tool is `fmi-checks e015chk`, added on this
branch. It chains the gate's own `item::search` hop by hop, and every game number is an argument.

1. **On the ev5 branch (opening choice (4,184)), room 42 was not found.**
   - 946 goes back to room 63 (`v1t945`) and then to room 49 (`w10`, `v1t851`), verified from boot.
   - From room 49, three searches did not reach room 42:
     - depth 2: 5,000 trials, room 42 not reached;
     - depth 2 with the verb slots alone: 5,148 trials, no dialog;
     - from local 211's dialog (exit A's dialog, reached by `v12t-2t596`), depth 3: 4,000 trials, which only reach
       room 63.
   - play8 reached room 42 only from entry 2's branch (opening choice (4,154)). Both of its room-42 entries made from
     a room-49 entry (296 and 307) descend from entries 2 and then 22.
2. **On play8's branch (from entry 184, frame 1154), 946 reaches room 42 and runs local 212.** Each hop is the gate's
   search with a different goal:

   | hop | inputs found | stop |
   |---|---|---|
   | exit A | `v1t596`, `v120` | room 63 at 1344 |
   | to room 69 | `v1t853` | 1444 |
   | take 946 | `v11t946` | owner:946 = 1 = ego at 1459 |
   | to room 63 | `v1t945` | 1475 |
   | to room 49 | `w10`, `v1t851` | 1910 |
   | the talk that leads to room 42 (verb 124 on) | `v1t592`, `v12t-11` | 2169 |
   | dialog choices, breadth-first over verb slots ≥ 120 | `v124 v120 v120 v120 v122 v120` | room 42 at 2980, owner:946 = 1, class:491:12 = 1 |
   | open object 490 (state:490 becomes 1) | `v3t490` | 2998 |
   | local 212 runs | `v8`, then `v105t491` | 3044 |

   - At the stop at 3044, room 42's local 212 ran in the trial, `class:491:12` is cleared and owner:946 = 0. That
     matches local 212's `setClass(491,[12])` at 0x0000 and `setOwnerOf(946,0)`.
   - The 35-click movie replays from boot with 0 differences at frames 2169, 2980, 2998 and 3044: 3,855, 3,862,
     3,861 and 3,859 facts, with room and digest equal. The final digest is 120c5358e4355923.
   - Searches and movies: `check/carry-*.json`.
3. **The same sentence without 946.** At the 8 sweep entries other than 296 and 356, the gate's step 5 expanded the
   `v8` node: `v8` then each held inventory slot (100-104) on object 491, with and without actor 11 between them, 339
   trials per entry. None ran local 212 or cleared class 12. Entries 324 and 341 are the same state, so this covers
   7 distinct states. 946 sits in slot 105, which is empty at those entries. Entry 296 did not expand `v8`.

So, in the engine:

- 946 is reached on exit A → room 63 → room 69.
- It can be carried to room 42.
- There, "use 946 with 491" runs local 212 and clears class 12.
- The same sentence with every item play8 held does not.

Two caveats remain:

- "Room 42 cannot progress without 946" is still shown only within the sweep's bounds and by the grep.
- That room 42 is reachable after exit A was shown on play8's opening branch, not on ev5's.

## Issues

1. **The 30,000 sweep trials include a duplicate.** Entries 324 and 341 are one state: same frame (9084) and
   identical trial digests. That leaves 9 distinct states and 27,000 distinct trials.
2. **The `Started` detector is blind on this path.** 946's verb-8 code starts local 212 from object code, which runs
   outside the input handler's log. The gate reports this from its control. The count therefore rests on `live`,
   `cleared` and `ran`.
   - `cleared` is the strongest of the three: class 12 on 491 is never set by any script, and local 212 clears it at
     0x0000.
3. **The static half misses a route to 946 and a load of room 69.** Neither is player-reachable as far as the
   scripts show, but the gate does not list them.
   - Global script 1 at 0x271C has `pickupObject(946,69)` under its argument 787, a start-point branch of the boot
     script.
   - Room 49's local 213 at 0x02B1 loads room 69, chained from room 49's entry when Bit[418] = 1. The only literal
     write to Bit[418] in the extraction sets it to 0.
4. **The two "untraced" indirect starts can be traced.**
   - Var[125] is only read (script 4 at 0x03B1 and 0x03B6, behind `if (Var[125])`), so it stays 0 and that start
     never runs.
   - Script 72 has no start site, literal or through Var[163], Var[164], Var[180] or script 94's argument.
   - So neither can start local 212.
5. **The ladder's C4 → C5 depends on the opening choice.** On ev5's branch, room 42 was not reached from room 49
   within the searches above. On play8's branch it was. The ladder in plan.md §"Decision rules" does not mention
   this.
6. **The "error" runs (verb 102 then a target) are still not investigated.** They do not hide the sentence that
   matters: `v102t491` under the `v8` node stops at an idle.

## Commands

```
cd /var/tmp/fast-mi/wt-e015-item-chk/experiments/E011-speed/rs
CARGO_TARGET_DIR=/var/tmp/fast-mi/target-e015-item-chk nice -n 10 cargo build --release -p fmi-checks --bin e015 --bin e015chk
source /var/tmp/fast-mi/e015-out/item-chk/env.sh    # results/item/env.sh with E015, FMI_ROOT and OUT changed
# the gate's commands.sh steps 3-6 with $OUT=/var/tmp/fast-mi/e015-out/item-chk, then compare with e015-out/item
# the carry: play8 entry 184's movie (its chain's events up to frame 1154) as p184.jsonl
C=/var/tmp/fast-mi/target-e015-item-chk/release/e015chk
nice -n 10 $C c4 --movie p184.jsonl --frame 1154 --object 946 --class-obj 491 --class 12 \
  --hops room:63:2:3000:1,room:69:2:3000:0,own:0:2:3000:0,room:63:2:3000:0,room:49:2:6000:0,verb:124:2:12000:1
nice -n 10 $C c5 --movie <c4's movie> --frame 2169 ... --hops vroom:42:8:8000:120,ran:212:1:3000:1
nice -n 10 $C c7 --movie <c5's movie> --frame 2980 ... --hops state:490:1:2000:1,ran:212:1:3000:1
nice -n 10 $C c8 --movie <c7's movie> --frame 2998 ... --hops ran:212:2:6000:1
```

In the first run, the dialog-choice hop (`vroom:42:7:6000:0`) clicked every verb slot and ran out at 6,000 trials. With
the slots restricted to ids of 120 and above, room 42 was reached in 206 trials.
