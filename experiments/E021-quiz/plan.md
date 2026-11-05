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
