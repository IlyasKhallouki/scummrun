# E015 item gate: results

Run 2026-11-04 on the laptop with one engine host under `nice -n 10`. The tool is `fmi-checks e015 item` at ba98b46.
The plan is amended by (1) and (2) at c0e3c61 and 844e1ae, both committed before the first run. The environment is in
`env.sh` and the commands in `commands.sh`. Rooms, objects, scripts and verbs are given by number only.

## Verdict: PASS

| criterion | result |
|---|---|
| owner:946 = ego on a chain of exit A, then room 63, then room 69, verified from boot with 0 differences | yes: 3,850 facts, room and digest all equal at frame 1548 |
| room 42's local 212 has one start site, in object 946's code | yes (see step 5, static, for the two indirect arguments that could not be traced) |
| 0 sweep trials start local 212 or clear class 12 on object 491 | 0 of 30,000 |

## Numbers

**Step 1: exit A.** Room 63 loads at frame 1392 and the first idle is at 1405 (sentence idle, rng 1523492811,
xstart 88).

**Step 2: room 63 to room 69.** The search stopped at depth 1 after 95 trials (one node), in 0.80 s, at 8.4 ms per
trial.

- **Openings to room 69:** 6 of 95. All are clicks on object 854: alone, or after verb slot 1, 100, 101, 103 or 104.
  The first, `v1t854`, is the chain: clicks (2,154) at 1406 and (91,55) at 1408, with room 69 idle at 1533.
- **The other openings:** 6 lead to room 49 (object 851, the same five verb forms and the click alone), and 78 stay
  in room 63. 5 end with reason `error`; all of these are verb 102 then a target.
- **The draw.** Every opening forks the same state, which has rng 1523492811.
- **Check from boot:** the room-69 stop alone verifies with 0 differences (3,848 facts, room, digest).

**Step 3: object 946 in room 69.** Room 69's first idle is the stop of step 2 (frame 1533, rng 323603000). The search
stopped at depth 1 after 135 trials, in 0.83 s.

- **The opening that takes 946:** 1 of 135, `v11t946`: verb slot 11, then object 946, with clicks (56,161) at 1534
  and (91,59) at 1536. At the stop, frame 1548, owner:946 = 1 = ego.
- **The other openings:** owner:946 = 15 at 126 stops, and 8 runs end with `error` (verb 102 then a target). 6 stops
  are in room 63 (object 945, the exit back).

**Step 4: the whole chain from boot.** The movie is ev5's 5 clicks plus 2 and 2 more:

`[993,4,184] [1040,5,135] [1101,5,110] [1201,30,25] [1320,4,154] [1406,2,154] [1408,91,55] [1534,56,161] [1536,91,59]`

A fresh engine run to frame 1548 gives 0 fact differences out of 3,850 (control keys included), the same room (69)
and the same digest (`0babbc2cb2c1a1de`). A second run of steps 1-4 gave the same summaries.

**Step 5, static** (`summary-5s.json`)

- **Literal start sites of a script 212.** There are 3:
  - room 48, object 587 at 0x0053, `chainScript`;
  - room 49, local 200 at 0x00EC, `startScript`;
  - room 69, object 946 at 0x0091, `startScript`, in its verb-8 code, guarded by class 12 set on object 491.

  The room-48 and room-49 sites are room code and start their own rooms' local 212. Object 587 is a room-48 object
  with no pick-up verb. So the only literal site that can start room 42's local 212 is object 946's code, run when
  946 is held in room 42.
- **Indirect starts.** There are 6:
  - `Var[163]`: assigned 11 and 217;
  - `Var[164]`: assigned 0, 203 and 204;
  - `Var[180]`: assigned 161;
  - `Var[125]`: no assignment in the extraction;
  - script 94's `Local[2]`: its only caller passes 208;
  - script 72's `Local[4]`: script 72 has no literal start.

  None is traced to 212. The last two (`Var[125]` and script 72's argument) are not traced to any value.
- **Clears of class 12 on object 491.** There is 1: room 42, local 212 at 0x0000, `setClass(491,[12])`.

**Step 5, engine** (`summary-56.json`): 10 entries, each searched to depth 2 with up to 3,000 trials.

| detector | trials that show it |
|---|---|
| a `Started` touch of local 212 | 0 |
| local:212 live at the stop | 0 |
| class:491:12 cleared at the stop | 0 |
| a run of room 42's local 212 anywhere in the trial (not gating) | 0 |

- **Entries:** 302, 324, 341, 358, 456, 556, 573, 595, 356 and 296. Every one holds class:491:12 = 1, and none has
  local 212 live.
- **Trials:** 30,000 in all. The first 9 entries give 285 depth-1 trials each (1,455 of them with a third click); entry
  296 gives 344. All 30,000 stops are in room 42. 297 runs end with `error`, all verb 102.
- **Coverage at depth 2.** The 3,000-trial limit expands 9 of 275 expandable depth-1 stops (7 of 335 for entry 296).
  Depth 1 is complete; depth 2 covers about 3% of its stops.
- **Detector control** (not gating; entry 302, depth 1). With local 213, a room-42 local started by object 492's code,
  the counts are live 2, ran 2 and started 0. With local 202 all three are 0.
  - So the `Started` touch never sees a start made from object code. Patch 0007's log holds the input handler's run
    only, so the "live" and "ran" detectors carry the count.
  - The cleared detector did not fire on any of the 30,000 stops, so it reads class 12 as set at every stop.

**Step 6: room 42 back to room 49.** No stop of the 30,000 is in room 75 or in any room other than 42, so no
room-75 sweep ran. The answer is "not within the sweep".

**Cost.** Steps 1-4 took 2.4 s of wall. Steps 5-6 took 136 s of wall, with 15.5 s user and 10.7 s system CPU
(children included). The trials ran at 3.0-5.4 ms each. The code is 811 lines with its comments, against the 200-300 planned.

## Predictions scored

| prediction | p | outcome |
|---|---|---|
| exit A reproduces (room 63 at frame 1392) | 97% | hit: 1392 (idle 1405) |
| room 69 within limits | 85% | hit |
| at depth 1-2 | | hit: depth 1 |
| exactly 1 opening leads there from one state | | **miss**: 6 of 95 openings, all on one object (854) |
| owner:946 = ego given room 69 | 80% | hit |
| at depth 1-3 | | hit: depth 1, `v11t946`, 1 of 135 openings |
| the chain verifies given 946 | 95% | hit: 0 differences |
| local 212 has one start site in 946's code, and 0 sweep starts | 85% | hit (two indirect arguments not traced, see above) |
| gate passes | 65% | hit |
| room 42 back to 49 via room 75: yes 55%, no 20%, not within the sweep 25% | | "not within the sweep" |

## Findings

1. **The step from room 63 to room 69 is one click on object 854.** The verb is the default, or any of the verb slots
   1, 100, 101, 103 or 104 clicked before it. Object 946 is one sentence away: verb 11, then 946. The room-49 to
   room-69 to 946 chain is 9 clicks from boot, and it verifies exactly.
2. **Room 63 leads back to room 49 by object 851, and room 69 leads back to room 63 by object 945.** Both take one
   click.
3. **No input from 10 room-42 entries, at depth 2, starts local 212 or clears class 12 on object 491.** None leaves
   room 42 either. Statically, the only clear of that class is local 212 itself, and 212's only reachable start is
   object 946's code. This supports the claim that play8's room-42 branch could not progress without 946. The engine
   half is bounded: depth 2 covers about 3% of the depth-1 stops.
4. **Verb 102 then any target ends the engine run with reason `error`.** This happened in 5, 8 and 297 trials of
   steps 2, 3 and 5. It was not investigated.
5. **Patch 0007's `Started` touch cannot see a script started by object code.** The input log covers only the input
   handler's run. A detector of a step's script must read the slots or the segment's runs.
