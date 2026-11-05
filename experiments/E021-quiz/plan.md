# E021: the dive needs a quiz answered from a book read two rooms earlier

## The block (E020's result)

- **The check:** room 42's dive (local 207) loads room 82 only if `Bit[443]` is set.
- **The only writer in play:** room 49's quiz (local 208, at 0x116D), when its counter `Local[0]` reaches 2. A turn's
  answer counts only if it equals the drawn coordinates: `Var[239]` in turn 1 and `Var[240]` in turn 2.
- **Where the coordinates come from:** only global script 98 draws them (`getRandomNr`). Only "Look at" on object 937
  (Plato's Lost Dialogue, held by Indy, defined in room 68) starts script 98, and only when the book has class 12 and
  `Var[239]` is 0.
- **The chain to regress:**

  > bit:443 ← local 208 @0x116D [Local[0] ≥ 2] ← two guarded increments [var:239, var:240 drawn] ← global 98 ← verb 9
  > on obj 937

## Prior work and what it says (research workflow wf_f8ae1775-2c0, 2026-11-05)

- **The reader's locals:**
  - It carries a script's locals exactly across waits (machine.rs `finish`, builder.rs `record`, `eng_resume_at`).
  - A slot already running at a reading's start resumes with unknown locals, because tryjob passes no
    `--slot-locals` (tryjob.rs:566), though tries boot with `dump_locals`.
  - Paths that differ only in locals are merged into value sets at joins.
- **Planning:**
  - Goal regression / means-ends analysis carries a fact needed later back to its achiever's preconditions (GPS;
    Pednault, UCPOP causal links and threats).
  - LAMA treats effect conditions as preconditions and gives landmarks that are "required again".
  - Novelty partitioned by the count of reached goals (BFWS) credits progress toward a regressed target.
- **Rejected:** abstracting the counter (it shows the write reachable with no witness), backward RL (no state has
  the bit) and a hand-written quiz route (rule: automate, don't hand-route).

## What the laptop shows (play9's C4 entry 3412, room 49, holding 946, E020's switches)

- **G0, measured:** none of play9's 4,176 states has `var:239 ≠ 0`, so the book was never read. 44 states have
  `var:176 = 11` (the captain's quiz topic). The quiz was reached without the coordinates drawn, so the engine itself
  would never set the bit.
- **Gap A, verbs on held objects never run in a reading.**
  - Every recorded `sentence(9,937)[inv]` takes `getDist > 16` ("I can't reach it", global 2 at 0x0395) and never
    reaches `startObject(937, 9)`. No action involves script 98 or `var:239`.
  - The engine places a held object at its owner's position (`getObjectOrActorXY`, `WIO_INVENTORY`), so its distance
    from ego is 0.
  - Giving the book class 13 (skip the distance check) removes the "can't reach" branch but still reaches no
    `startObject`. The cut is not only the distance.
- **Gap B, actors are never sentence objects.** The reading has no "talk to" sentence on actor 11 (the captain;
  `class:11:13 = 1`, `var:164 = 204`). `Builder::inputs` takes only room objects (and held ones) as A, so a reading
  cannot start the quiz. Only model-free clicks can.
- **Gap C, drawn values must be read exactly.** With held code, script 98 is in scope and writes `var:239`/`var:240`
  from `getRandomNr`. Unless they are facts (`--fact-vars`, empty in `read_fresh`), a reading reads them as fresh
  unknowns, and the quiz's guards become checks.

## Steps, gates and predictions

1. **Gap A:** find the cut and fix it generally (a held object's verb runs in a reading).
   - Gate GA: the room-49 reading from 3412 has an action on `sentence(9,937)` that starts global 98.
   - Prediction: it passes after a fix in the reader's sentence or position model, without naming objects.
2. **Gap B:** actors present in the room become sentence objects (A) for the verbs their classes allow.
   - Gate GB: the reading from 3412 has `sentence(12,11)`, and it reaches local 204 and then local 208.
3. **Gap C:** the guard variables are read as facts.
   - Gate GC: a start row from 3412 with `var:239`/`var:240` set to a drawn pair has an action writing `bit:443 = 1`,
     with `Local[0]` 0 → 1 → 2 exact and no check on it.
   - With 0/0, no action writes it.
4. **Regression across places:** a check that a stuck place fails (`bit:443` in local 207) is regressed statically to
   its write sites and guards, which become targets at the places whose code writes them.
   - Gate GR: from a room-42 entry stuck at 207, the targets include `bit:443 = 1` (room 49) and a nonzero `var:239`
     (the place where 937's verb 9 runs), within 3 back-chain steps.
5. **Run:** the play10 archive resumed on the VM from C6 with every gate passed.
   - Prediction: C7 within 90 minutes of run wall.
   - Stop rule: 90 minutes of wall with no new rung (the watcher). Then diagnose from the archive before any change.

Rule (2) still holds: nothing merges to main before Monkey Island's checks pass with every switch on.

## Amendment (1), 2026-11-05 20:00 UTC: the book's cut is the bridge, not the reader

- **Reading the book uses an in-game save and load.**
  - 937's Look at runs script 98 (the draw), then script 119 with room 83. Script 119 makes the game's temporary save
    (`saveLoad(1,26)`: the engine's `SO_ROOM_SAVEGAME`, slot 99), loads room 83 to show the page, and on exit loads
    the save back.
  - The bridge bans every save and load in runs (`observePreviousFrame`: `fail("saveload")`). That rule was meant for
    the player's keys and autosave, but it also ended every try that read the book. So no state ever held the drawn
    coordinates.
- **Change, patch 0009 (`patches/0009-scumm-speedrun-temp-saves.patch`; fmi `FMI_TEMP_SAVES=1` →
  `SPEEDRUN_TEMP_SAVES=1`):**
  - The game's temporary saves and loads are performed, kept in the engine's memory (`openSaveFileForWriting`/
    `Reading` with `compat`), so a forked branch holds its own copy.
  - The bridge records each one (`temp_save`). Other saves and loads still fail.
- **Gate GA, checked with the patched engine and E016's G3 tool, realizing `s9:937` from play9's entry 3412:**
  - Switch off: no sentence after 60 candidates.
  - Switch on: realized in 3 candidates. The temporary save is recorded (slot 99, room 49, frame 1649), and the run
    reaches room 83 by frame 1949.
  - The chain replays from boot with 0 differences over 3,845 facts, with equal state digests.
- **Correction:** `reading.actions.jsonl` holds an action's label, checks and preconditions, not its writes. The
  plan's "no action writes bit:443" counts from it showed nothing. A try's targets are the measure.
- **Gate GC, partly answered:** from play9's entry 1296, which waits in the quiz's turn 2 (0x10E6), a model try targets
  `bit:443 = 1` at tier 1. It plans and plays two plans (1 and 3 steps), and then at frame 1565 the planner proves none.
  The engine's counter stayed 0 there, because the coordinates were never drawn.
- **Gap B (actors as sentence objects) is not needed for this path:** free tries start the quiz (44 play9 states hold
  `var:176 = 11`), and a reading from a state in the quiz plans its answers. It stays open as a general gap.
- **Run:** play10's archive on the VM with `env.sh` (E020's set plus `FMI_TEMP_SAVES=1`) and the engine built with
  0001-0009.
  - Prediction: within 90 minutes, entries holding `var:239 ≠ 0` appear (the book read by a free try), then
    `bit:443 = 1` in room 49, then C7.
  - Stop rule: the watcher's 90 minutes with no new rung.
