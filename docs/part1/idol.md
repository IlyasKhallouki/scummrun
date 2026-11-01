# Part I: the idol (thievery) trial chain

This file lists every player action needed to steal the idol, starting from
free control at the Lookout with no items and ending at the trial-complete
flag. It covers the piranha poodles (meat + yellow petal), the mansion and
foyer, the jail and Otis (mints, gopher repellent, cake with file), the theft,
Fester, the underwater escape and the completion flag.

## Conventions

- **Script citations** use `data/scripts/<file> [XXXX]`, where `XXXX` is the
  descumm byte offset (see `data/scripts/INDEX.md`, "Offsets"). Inside a
  section, a short form such as "`local-201 [0079]`" refers to the file named
  in that section's heading.
- **Engine citations** use `third_party/scummvm/engines/scumm/<file>:<line>`
  at the pinned ScummVM tag `v2026.3.0`.
- **[DOBJ]** marks initial object owner, state and class data. This data is
  **not** in the script dump. It comes from a read-only parse of the `DOBJ`
  block of `game/classic/MONKEY1.000`. The index file is XOR-encrypted with
  key 0x69 for v5 (`resource.cpp:204`). The layout follows the engine's
  reader: a uint16 count, then one byte per object (owner = low nibble,
  state = high nibble), then little-endian uint32 class bitmasks
  (`resource.cpp:1360-1378`). Class `c` is bit `c-1` (`object.cpp:254`).

  Values used here:

  | Object | Initial classes |
  |---|---|
  | 467 poodles | 5, 13 |
  | 566 hunk of meat | 2, 7, 15 |
  | 568 fish | 2, 6, 16 |
  | 689 yellow petal | 2 |
  | 405 prisoner | 5, 6, 12, 13 |
  | 420 cake | 6, 32 |
  | 394 storekeeper | 5, 13 |
  | 465, 632, 633, 316, 570 doors | 15 |
  | 634 closet door | 6, 15 |
  | 637 gaping hole | 32 |
  | 640-644 foyer items | all have 32 |

  All of these objects start with owner 15 and state 0.
- **Verified** means read directly in the scripts or the engine source.
  **Inferred** means the code implies it but does not state it.
- Choice substrings are case-insensitive and avoid descumm's `^`, which may
  render as an ellipsis in game.

### Notation (verified in engine)

- **Class tests and changes.** `classOfIs(o,[c])` with `c >= 128` means
  "class `c & 0x7F` is set"; with `c < 128` it means "class `c` is **not**
  set" (`script_v5.cpp:1512`). `setClass(o,[c])` sets class `c & 0x7F` when
  `c >= 128` and clears class `c` otherwise (`script_v5.cpp:687`). For
  example, `setClass(566,[134])` sets class 6.
- **Owners.**
  - `15` is `OF_OWNER_ROOM`, meaning "lying in its room" (`scumm.cpp:1739`).
  - `pickupObject` sets the owner to `VAR_EGO`, adds class 32 (untouchable)
    and sets state 1 (`script_v5.cpp:2021-2033`). `VAR_EGO = 1`.
  - Owners `13`, `14` and `0` mean "neither in a room nor in the inventory".
    **Inferred** from usage: 13 is "in the stew pot"
    (`room-041-kitchen/local-213 [00B8]`) and 14 is "used up / handed over".
- **Time.**
  - `delay(N)` and `delayVariable` count jiffies (1/60 s). The engine
    subtracts each frame's `delta` (`script_v5.cpp:972-986`,
    `script.cpp:1521`, `scumm.cpp:3123`).
  - `getRandomNr(N)` is uniform over **0..N inclusive**
    (`script_v5.cpp:1441`, `third_party/scummvm/common/random.h:69`).
- **Verb ids.**
  - Verb **80** is the "item given/used on me" entry (see §1).
  - Verbs 90/91 only set the default-verb hint `Var[182]` and the inventory
    icon `Var[376]`, so they play no part in the chain (**inferred** from
    usage).

---

## 0. Summary

- **Poodles.** Drug the meat with the yellow petal, then give it to the
  poodles. This sets `Bit[15]`.
  - Petal: forest screen 215.
  - Meat: kitchen, room 41. To reach it, the bar cook must be out of the
    kitchen.
  - Drugging is a `Use` (meat with petal, or the stew route). It never
    happens automatically.
- **Mansion visit 1.**
  1. Open the front door and walk in.
  2. Open the foyer door 632 and walk through. This plays a long cutscene
     that gives the gopher repellent, the Manual of Style, the wax lips and
     the staple remover.
  3. The cutscene also makes the gaping hole usable.
- **Jail.**
  1. Visit 1: talk to the prisoner. This sets `Bit[420]`, which unlocks the
     storekeeper's mint line.
  2. Buy the breath mints for 1 piece of eight (`Var[195]`).
  3. Visit 2: give the mints (this opens a dialogue; take the exit choice),
     then give the gopher repellent. You get the cake.
  4. Open the cake. It becomes the "file".
- **Mansion visit 2.**
  1. Walk to the gaping hole. This plays the theft cutscene, the Fester
     dialogue and the Elaine dialogue (three menus). You get idol 635.
  2. Open the front door. This plays the Fester dialogue and takes you to the
     dock cutscene, then underwater.
- **Underwater.** Pick up the idol (object 578). This sets the completion
  flags.
- **Completion.** `Bit[85] = 1`, `Var[200] = 2` and `Var[196] += 1`, set by
  `global/script-071 [0088]-[0094]`. The call is at
  `room-042-underwate/local-200 [0041]`, **before** the forced Elaine dock
  scene. Showing the idol to the pirate leaders is optional.

---

## 1. Sentence-script mechanics the chain relies on

The sentence script is global script 2. It runs every queued sentence
(`VAR_SENTENCE_SCRIPT`). All citations in this section are in
`data/scripts/global/script-002.txt`.

| Sentence | Behaviour | Cite |
|---|---|---|
| Give X to object Y (verb 4), Y > 12 | If Y has **class 5**: walk to Y; if distance ≤ 32, run **Y's verb 80** with `Local[0] = X`. Without class 5 this path is skipped. Poodles 467, prisoner 405 and storekeeper 394 have class 5 [DOBJ]. | `[003F]`, `[0046]`, `[004D]`, `[0083]`, `[008C]`, `[0093]`, `[009A]` |
| Use X with Y (verb 7), X or Y lying in the room with **class 7** | Pushes `(7,X,Y)` and then `(9,X)` (or `(9,Y)`). The sentence stack is LIFO (push at `script.cpp:1165`, pop at `script.cpp:1196`), so the **pick-up runs first**, then the use. Meat 566 has class 7 [DOBJ]. | `[020F]`, `[021D]`-`[023F]`, `[0245]`-`[0267]` |
| Any other verb on an object lying in the room | Walk to the object. If the distance is > 16: "I can't reach it.", and the sentence is aborted. | `[02A1]`-`[02E0]`, `[02E4]`-`[0317]` |
| Then | `startObject(A, verb, [B, verb])`, i.e. A's verb script with `Local[0] = B`. | `[039D]` |

An inventory object owned by ego is located at ego's position
(`object.cpp:428-436`). So sentences whose objects are all in the inventory
(Use meat with petal, Open cake) pass the reach check in any room.

---

## 2. (a) The piranha poodles

### 2.1 What neutralises them

Files: `room-036-mansion-e/obj-0467-deadly-piranha-poodles.txt`,
`local-201.txt`, `entry.txt`.

- **Verb 80 entry.** `obj-0467 [00B2]`: if `Bit[15]` is set, the dogs only
  say "Let sleeping dogs lie." Otherwise `[00D5]` runs `startScript(201,[item])`.
- **Script 201.**
  - `[0000]` walks ego to the dogs.
  - `[0009]`: **only item 566 (hunk of meat) is accepted**.
  - Other items:

    | Item | Result |
    |---|---|
    | 568 fish | "Piranha poodles don't eat fish." (`[01F9]`/`[0203]`) |
    | 689 petal | "...dogs are vegetarians." (`[0227]`/`[0231]`) |
    | anything else | "I doubt the dogs will want this." (`[0260]`) |

  - **Meat**, in a `cutscene`:
    - `[002F]`, `[0033]`: `setOwnerOf(566,0)`, then `setOwnerOf(566,15)`. The
      meat leaves the inventory and is back "in its room" (the kitchen).
      This happens **whether or not** it was drugged.
    - `[0037]`: `setState(566,0)`. `[003B]`: clear class 19 ("stewed").
    - `[0079]`: **`classOfIs(566,[134])`, i.e. the meat must have class 6
      ("drugged").**
      - **Success** (`[0082]`-`[01B9]`):
        - `Var[116] = 10` (`[0082]`; meaning not analysed).
        - **`Bit[15] = 1`** (`[0087]`).
        - The dogs are renamed "sleeping piranha poodles" (`[008C]`).
        - Walk-boxes 1-6 are unlocked (`setBoxFlags(...,0)`, `[00BA]`-`[00C7]`).
        - **Door 465 becomes touchable** (`setClass(465,[32])`, `[00CE]`).
        - The notice object 468 is drawn and printed (`[00D9]`-`[01B6]`).
        - `VAR_VERB_SCRIPT = 203` (`[01B9]`; see §12).
      - **Failure** (`[01C1]`-`[01E5]`): the barking scripts restart and the
        meat is lost to the kitchen.
    - `[01EE]`: `setClass(566,[32,6])` clears untouchable and class 6.
- **Room entry.** `entry [0007]`: if `Bit[15]` is clear, walk-boxes 1-6 are
  locked (`[002B]`-`[0038]`, flag 128) and door 465 is made untouchable
  (`[003F]` `setClass(465,[160])`). The door cannot be clicked or reached
  until the dogs sleep.

**Alternatives.** There is no other way past the dogs: `Bit[15]` is set only
at `local-201 [0087]` (grep). The only real choice is **how the meat gets
class 6** (§2.4).

### 2.2 The yellow petal (forest, pseudo-room 215)

**Pseudo-rooms.** The forest is room 58, loaded under the room numbers 201
to 220. `global/script-001 [0736]`-`[0762]` executes `PseudoRoom(58, …)`.
The engine maps resource ids `0x80|j` to room 58 (`script_v5.cpp:2089`) and
loads them as room 58 (`room.cpp:166-167`). `VAR_ROOM` holds 201 to 220.

| # | Sentence | Room | Preconditions | Effects | Cite |
|---|---|---|---|---|---|
| P1 | Walk to `fork` (11, 911) | 85 (Melee map) | none | → room **218** (forest) | `room-085-melee/obj-0911-fork.txt [000C]` |
| P2 | Walk to `path` (11, 685) | 218 | `VAR_ROOM == 218` | → **215** | `room-058-damnfores/obj-0685-path.txt [015E]`/`[0168]` |
| P3 | **Pick up `plants` (9, 678)** | 215 | `VAR_ROOM == 215` (`[007F]`); petal 689 owner == 15 (`[0086]`) | **gain 689 yellow petal** (`[0092]` `pickupObject(689,0)`, `[0096]`). Otherwise: "I've already got one." (`[009D]`) | `room-058-damnfores/obj-0678-plants.txt` |
| P4 | Walk to `path` (11, 687) | 215 | - | → 218 | `room-058-damnfores/obj-0687-path.txt [0045]`/`[004F]` |
| P5 | Walk to `path` (11, 687) | 218 | - | → map 85 | same file `[010B]`/`[0115]` |

Notes:

- **Plants exist only at 215.** In the 215 screen setup, plants 678 are drawn
  at `room-058-damnfores/entry.txt [07D1]`. In any other forest screen, Pick
  up refuses (`obj-0678 [00B8]`).
- **No map is needed for 215 ↔ 218.** Paths 685 and 688 *out of* 215 need the
  treasure map (442), the guide script 67, or `Bit[401]`
  (`obj-0685 [004F]`-`[00C6]`, `obj-0688 [004F]`-`[006B]`). That check is not
  on this route.
- In 218, object 686 forwards any verb to 685's Walk to
  (`obj-0686-path.txt [0010]`).
- **The petal is single-use** when it is combined directly with the meat.
  `global/script-182 [0017]` sets its owner to 0, and P3 then fails with
  "I've already got one." The stew and fish uses instead reset it to owner 15,
  so it can be picked again (`room-041-kitchen/local-213 [0042]`/`[0046]`,
  `obj-0568-fish [0084]`/`[0088]`).

### 2.3 The hunk of meat (kitchen, room 41)

The kitchen is reached only from the bar: door 316 →
`room-028-bar/local-218 [0017]` `loadRoomWithEgo(570,41)` (grep). Detailed
cook timing is in `money.md`. All `local-*` files in this section are in
`room-028-bar/` unless noted.

**Cook state at bar entry.**

- `entry [0012]`/`[0017]` starts `local-205`.
- If you arrive **from the kitchen** (`Var[101] == 41`, i.e. the previous
  room), the cook comes out at once (`local-205 [0033]`/`[003A]`).
  `Var[101] = VAR_ROOM` is set at `global/script-007 [0000]`, and script 7 is
  the room-exit hook (`global/script-001 [066E]` `VAR_EXIT_SCRIPT = 7`).
- **Otherwise** door 316 is set to state 0 and `local-211` starts
  (`local-205 [0040]`/`[0044]`):
  - The cook (actor 6) is out of the room (`[0000]`).
  - **Timer:** `delay((rand(0..20) + 30) * 60)`, i.e. **1800 to 3000 jiffies
    (30 to 50 s)** (`local-211 [000D]`).
  - The wait is extended while script 220 (leaders' talk) or 203 runs
    (`[0021]`-`[0036]`).
  - Then `local-216` starts (`[0039]`).

**Cook outside the kitchen (`local-216`).**

- `[0017]`/`[0019]` stop 211 and 212.
- `[001B]` runs `startObject(316,2)`: door 316's Open, with 211 no longer
  running, reaches `startScript(25,[316,570])`. This opens **316 and 570**
  (state 1).
- The cook walks to a random object 330 + rand(0..28) (`[003F]`/`[004C]`).
- Then back: `[0065]` walk to the door, `[0080]` close it (`26 [316,570]`),
  `[0085]` restart 211.

**Door 316** (`obj-0316-door.txt`).

- **Open** (`[0018]`): if 211 is running (cook in the kitchen), start
  `local-214`, the cook's warning (one message, two parts joined by `wait()`). Otherwise
  `startScript(25,[316,570])`.
- **Walk to** (`[003F]`): if state == 1, start `local-218`, which walks in and
  loads room 41.

**Get-caught shortcut** (verified script behaviour; timing gain inferred).

- Opening 316 while the cook is inside runs `local-214`. That cutscene opens
  and closes the door, then starts `local-212` (`[004B]`).
- 212 waits `delay(300)` twice, **600 jiffies**, then starts 216: the cook
  comes out (`local-212 [0000]`-`[0020]`).
- This can be faster than the random wait of 1800 to 3000 jiffies in 211.

**Click guard** (only in the room's input script; see §12).

- The bar sets `VAR_VERB_SCRIPT = 202` (`entry [007D]`). Clicks on door 316
  go to `local-203` (`local-202 [001C]`/`[0023]`).
- If the cook is in the bar with x > 310, the click is **refused**:
  `local-215`, "Don't go into the kitchen!" (`local-203 [001E]`/`[0025]`).
  - If the cook's x is ≥ 520 at that point, he goes **back into the kitchen**
    and 211 restarts (`local-215 [0048]`-`[0083]`).
- If x ≤ 310, the cook is frozen (`[0030]`-`[0035]`) and the sentence runs.
- Every other idle click restarts the cook's walk (`local-202 [0034]`-`[005B]`).

| # | Sentence | Room | Preconditions | Effects |
|---|---|---|---|---|
| M1 | Walk to `door` (11, 316) | 28 | door 316 state == 1, i.e. the cook is out (`obj-0316 [003F]`). Click-faithful: also cook x ≤ 310 (`local-203 [001E]`). | cutscene → **room 41** (`local-218 [0017]`). Under direct push the cook can close the doors mid-walk (§12, race). |
| M1' | Open `door` (2, 316) | 28 | cook in the kitchen (211 running) | "caught" cutscene; the cook comes out ~600 jiffies later (above) |
| M2 | Pick up `hunk of meat` (9, 566) | 41 | owner(566) == 15 (`room-041-kitchen/obj-0566-hunk-of-meat.txt [0035]`) | **gain 566** (`[0041]`). Usually merged into M3 (§2.4). |
| M3 | Walk to `door` (11, 570) | 41 | 570 state == 1 (opened together with 316) | → room 28 (`room-041-kitchen/obj-0570-door.txt [0030]`/`[003C]`). If the cook closed 570 during M1 (§12), first Open `door` (2, 570) → `startScript(25,[570,316])` (`obj-0570 [0018]`). It is a no-op when already open (`global/script-025 [000F]`). |

Nothing in the kitchen guards the meat. The kitchen actor 7 is the
seagull on the pier, not the cook (`room-041-kitchen/local-204`, `local-205`).

### 2.4 Drugging the meat (class 6): alternatives

**A. Direct: Use `hunk of meat` with `yellow petal` (7, 566, 689).**
Recommended.

- Meat's Use with `Local[0] == 689` (`obj-0566 [0077]`) runs
  `setClass(566,[134])`, i.e. class 6 (`[007E]`), and `startScript(182)`
  (`[0085]`).
- `global/script-182`: rename to "meat with condiment" (`[0000]`); **petal
  owner = 0, consumed** (`[0017]`).
- Reversed order (7, 689, 566) is equivalent: the petal's Use forwards to
  `doSentence(7,566,689)` (`room-058-damnfores/obj-0689-yellow-petal.txt [0042]`/`[0049]`).
- If the meat is still on the kitchen table, this **single** sentence also
  picks it up first, via the class-7 auto-pickup (§1). The engine runs it as
  two sentences: `(9,566)` and then `(7,566,689)`.
- If both items are already in the inventory, it works in any room.

**B. Stew route** (kitchen only; three sentences).

1. Use `yellow petal` with `pot o' stew` (7, 689, 574).
   - Forwards to the stew (`obj-0689 [004F]`/`[0059]`) and then
     `room-041-kitchen/local-213`.
   - "...every cook makes substitutions." The **petal returns to owner 15**
     (`[0042]`/`[0046]`).
   - The stew gets class 6 (`[004A]`) and may be renamed "spicy stew"
     (`[0069]`).
2. Use `hunk of meat` with `pot o' stew` (7, 566, 574).
   - Forwards via `obj-0566 [0088]`/`[0092]`, then `local-213 [0077]`.
   - `setOwnerOf(566,13)`: the meat is in the pot (`[00B8]`).
   - This is refused if the fish is already in the pot (`[0081]`/`[008D]`).
3. Pick up `pot o' stew` (9, 574).
   - `room-041-kitchen/obj-0574-pot-o-stew.txt [0036]`/`[0042]` →
     `local-214`.
   - Meat goes to ego (`[0007]`), named "stewed meat" (`[000C]`), class 19
     (`[001B]`).
   - If the stew has class 6, the meat gets class 6 (`[0043]`/`[005A]`).

Route A is strictly shorter.

**Not alternatives.**

- Fish + petal only renames the fish to "fish with condiment"; there is no
  class change (`room-041-kitchen/obj-0568-fish.txt [0066]`-`[0088]`). The
  dogs refuse fish anyway (§2.1).
- Giving undrugged meat loses it (§2.1).

### 2.5 Give the drugged meat to the poodles

| # | Sentence | Room | Preconditions | Effects |
|---|---|---|---|---|
| D1 | **Give `hunk of meat` to `deadly piranha poodles` (4, 566, 467)** | 36 | `!Bit[15]`; ego owns 566; 566 has class 6; dogs have class 5 [DOBJ]; ego within 32 (`script-002 [0093]`) | `Bit[15] = 1`; meat consumed (owner 15 in its room); door 465 touchable; walk-boxes open (§2.1) |

"Use meat with poodles" (7, 566, 467) reaches the same verb 80
(`obj-0566 [0098]`/`[00A2]`). It goes through the stricter ≤ 16 reach check
(`script-002 [02E4]`), and the dogs' walk-boxes are locked at that point, so
it may fail with "I can't reach it." Prefer Give.

The mansion is reached from the High Street: Walk to `Governor's mansion`
(11, 431) → room 36 (`room-034-high-stre/obj-0431-governor-s-mansion.txt [000C]`).

---

## 3. (b) Entering the mansion; the foyer

### 3.1 Front door (room 36)

| # | Sentence | Preconditions | Effects | Cite |
|---|---|---|---|---|
| E1 | Open `door` (2, 465) | 465 state 0; 465 not class 6 (locked) [DOBJ]. Click-faithful: `Bit[15]`, since 465 is untouchable and walk-boxes are locked before that. | `global/script-025 [000A]`-`[0036]` sets 465 state 1, **and the foyer side 633 state 1** (633 lacks class 11). | `room-036-mansion-e/obj-0465-door.txt [0018]` |
| E2 | Walk to `door` (11, 465) | 465 state == 1 | → **room 53**, entering at 633 | `obj-0465 [0030]`/`[003C]` |

Door 465 is the only way into room 53. A grep for `loadRoomWithEgo(…,53,…)`
finds only `obj-0465 [003C]`.

### 3.2 First foyer visit (the inner door 632 → cutscene 210)

Files: `room-053-foyer/`.

| # | Sentence | Preconditions | Effects |
|---|---|---|---|
| F1 | Open `door` (2, 632) | 632 state 0, not class 6 [DOBJ] | 632 state 1 (`obj-0632-door.txt [0028]` → `global/script-025`) |
| F2 | **Walk to `door` (11, 632)** | 632 state == 1 (`obj-0632 [0018]`/`[0024]`) | starts **`local-210`**, a long cutscene with no input (details below) |

Effects of `local-210`. The middle part is a joke: the game shows simulated
sentences on the sentence line (`local-218`), and nothing in it is player
input.

- **Flags and the vase.**
  - `Bit[481] = 1` (`[0002]`), the "first visit done" flag.
  - `local-206` (`[003E]`) briefly picks up the vase 630. `local-210 [0056]`-`[006E]`
    then takes it away again (owner 14).
- **Items gained:**

  | Item | Cite |
  |---|---|
  | 643 staple remover | `[01FB]` |
  | **641 Manual of Style** | `local-204 [0047]`, via `[0236]` |
  | **642 wax lips** | `[0267]` |
  | **640 gopher repellent** | `[02B8]` |

  The heavy chair 644 is picked up and removed again (`[03F2]`, `[0413]`).
- **Door 632 is locked for good:** `setClass(632,[134])` (`[004B]`).
- **The hole is created.**
  - `local-203` draws the hole 636 (`[0059]`), locks walk-box 16 (`[005D]`)
    and puts ego back in room 53 (`[0067]`).
  - **The gaping hole 637 becomes touchable:** `setClass(637,[32])`
    (`[022F]`). It starts with class 32 [DOBJ].
- **End.** `local-214` (`[04CC]`) shuts the sheriff in the closet: door 634
  is unlocked, opened, closed and re-locked (`[0000]`-`[0029]`). Ego says
  "If only I had a file" (`[0030]`).

Pressing Esc gives the same items: the override branch (`[0472]`-`[04B6]`)
grants 640/641/642/643. Skipping is out of scope for v1.

### 3.3 Leaving the foyer before the theft

| # | Sentence | Preconditions | Effects |
|---|---|---|---|
| F3 | Walk to `door` (11, 633) | 633 state == 1 (still open from E1) | → room 36 (`obj-0633-door.txt [005C]`/`[0068]`) |
| F4 | Walk to `trail` (11, 466) | none | → room 34 (`room-036-mansion-e/obj-0466-trail.txt [000C]`) |

On the way back in (§5), E2 alone is enough, because door 465 stays at
state 1 and nothing closes it.

### 3.4 The gaping hole: where the file is used

`obj-0637-gaping-hole.txt`, Walk to (`[000C]`):

| Condition | Result |
|---|---|
| ego owns cake 420 (`[000C]`/`[0011]`) **and** `classOfIs(420,[6])`, i.e. class 6 **cleared** = the cake has been opened (`[0018]`) | **`local-211`**, the theft (§5) |
| ego owns 420 but it is unopened | "I still don't have a file." (`[0027]` → `[0039]`) |
| ego does not own 420 but owns 641 | same line (`[002D]`/`[0039]`) |
| otherwise | "I'm not going back in there!" (`[0059]`) |

The opened cake is the only item the hole checks.

---

## 4. (c) The jail: Otis

Files: `room-031-jail/`.

- Enter from the High Street: Walk to `doorway` (11, 434), if `!Bit[453]` →
  room 31 (`room-034-high-stre/obj-0434-doorway.txt [000C]`/`[0011]`).
- Leave: any verb on `doorway` (400) → room 34 (`obj-0400-doorway.txt [0037]`).
- The prisoner is object **405**. Actor 4 is drawn at the same spot
  (`local-201`). Sentences target 405, which has the verbs and class 5.

### 4.1 What Otis wants, and the gates (verified)

- **Bad breath.** The prisoner starts with **class 6**, "bad breath" [DOBJ].
  - Talk to (`obj-0405-prisoner.txt [0015]`):
    - The first time (`!Bit[420]`), it sets **`Bit[420] = 1`** (`[001A]`)
      and runs the halitosis scene `local-202`. That scene has no menu
      (`local-202 [0050]` → `[18DE]`-`[19C0]`).
    - Later Talk-tos while class 6 is set are refused: "Talk to death-breath?
      No thanks." (`[0034]`/`[0038]`).
- **Give** (verb 80) → `local-203` with the item (`obj-0405 [006F]`). A give
  also sets `Bit[420]` at the end (`local-203 [034C]`/`[0351]`), **except**
  in three cases that never reach `[034C]`:
  - grog mugs 362 to 366 (`chainScript(70)` at `[0076]`);
  - the opened cake/file (`stopScript(0)` at `[0031]`);
  - the mints (`chainScript(202)` at `[0112]`).
- **Item handlers in `local-203`:**
  - **395 breath mints** (`[00B8]`-`[0112]`):
    - **clears class 6** (`[00BF]` `setClass(405,[6])`);
    - "Grog-O-Mint! How refreshing!";
    - **`chainScript(202)`**: the Otis dialogue opens at once.
    - The mints are **not** removed. There is no owner change in 203.
  - **640 gopher repellent** (`[0115]`-`[01E5]`):
    - **requires class 6 cleared on 405** (`[011F]`);
    - repellent → owner 0, then 14 (`[012A]`/`[012E]`);
    - Otis's line depends on `Bit[95]` (`[0171]`);
    - **`pickupObject(420,0)`: ego gains the cake** (`[01E0]`).
    - With class 6 still set, it falls through to the refusal at `[029D]`:
      "I don't want anything but my freedom!", plus "...and maybe a breath
      mint." (`[02D3]`). Nothing is lost.
  - **420 cake:**
    - opened (class 6 cleared): "But I need it to get the idol." Aborts
      (`[0000]`-`[0031]`);
    - unopened: "I told you, I HATE carrot cake!" (`[01EE]`-`[021F]`).
  - **488 money:** Otis takes 1 piece of eight (`[0221]`-`[0297]`). Not
    useful here.

So Otis wants the breath mints first, then something against the rats (the
gopher repellent). In return he gives the carrot cake with the file. Nothing
in 203 checks the dialogue flags (`Bit[95]`, `Bit[559]`).

### 4.2 Breath mints (store, room 30)

- **Getting in.** Open `door` (2, 437) → `global/script-025` opens 437 and
  387. Walk to `door` (11, 437) → room 30
  (`room-034-high-stre/obj-0437-door.txt [0018]`, `[004A]`/`[005B]`).
- **Storekeeper RNG at entry** (`room-030-store/entry.txt [002B]`-`[0061]`):
  - `Var[100] = rand(0..3)`, forced to 0 while script 67 runs.
  - On **0 (1 in 4)** the storekeeper is away: actor 11 is removed and 394 is
    made untouchable (`[0054]`/`[0057]`).
  - In that case, Push or Use `bell` (5/7, 399) runs `local-207`. He returns,
    **closes door 387** (`[00FD]`) and opens the dialogue `local-211`
    directly (`[01A1]`).
- **Talk to `storekeeper` (10, 394)** → `local-211` (`obj-0394-storekeeper.txt [0015]`).
  - Menu choice **"I could really use a breath mint."** (verb 124). It is
    shown only if `Bit[420] && !Bit[312]` (`local-211 [0251]`-`[02AE]`).
  - The handler (`[1AFA]`) runs `if (Var[195] > 0)` (`[1B1C]`). Then:
    - `Bit[312] = 1` (`[1BB1]`);
    - **`pickupObject(395,0)`** (`[1BC0]`);
    - `startObject(488,250,[0,1])`, i.e. **`Var[195] -= 1`**
      (`[1BC8]`; `room-038-lookout/obj-0488-pieces-of-eight.txt [0086]`/`[008B]`).
    - With no money: "I got some, but they ain't free." (`[1B88]`).
  - Both paths return to the menu ("What else do you want?", `[1DB6]`-`[1DD2]`).
  - Exit with **"I think I'd just like to browse."** (verb 125, always shown,
    `[0387]`-`[03D2]`; handler `[1BD6]` → `[1DD5]`).
  - Other relevant choices:
    - "Do you have files?" (127, needs ego to own 640) leads to a submenu
      that ends "Sorry, we're out of those." (`[1C2C]`-`[1DB0]`). The store
      sells no file.
    - "I'd like some rat repellent, please." (126) gets "...I haven't got
      any." (`[1BE3]`-`[1C29]`).
- **Leaving.** Walk to `door` (11, 387) → `local-204`. If ego carries no
  unpaid sword or shovel and 387 has state 1 → room 34
  (`obj-0387-door.txt [008C]`/`[0098]`, `local-204 [0031]`-`[0044]`). If the
  bell was used, Open 387 first.
- The mints cost 1 piece of eight, paid from the money var `Var[195]`.

### 4.3 Jail action table

| # | Sentence | Room | Preconditions | Effects | Dialogue |
|---|---|---|---|---|---|
| J1 | Talk to `prisoner` (10, 405) | 31 | `!Bit[420]` | **`Bit[420] = 1`**; halitosis scene | none |
| S1 | Talk to `storekeeper` (10, 394) | 30 | storekeeper present; `Bit[420]`; `!Bit[312]`; `Var[195] ≥ 1` | **gain 395**; `Var[195] -= 1`; `Bit[312] = 1` | choose `breath mint`, then `browse` |
| J2 | **Give `breath mints` to `prisoner` (4, 395, 405)** | 31 | ego owns 395. The handler checks nothing else (`local-203 [00B8]`). | 405 class 6 cleared; dialogue `local-202` opens | choose `stiff upper lip` (below) |
| J3 | **Give `gopher repellent` to `prisoner` (4, 640, 405)** | 31 | ego owns 640; 405 class 6 **cleared** (J2 done) | 640 → owner 14; **gain 420 cake** | none |
| J4 | **Open `cake` (2, 420)** | any | ego owns 420; 420 has class 6 | `setClass(420,[6,131])`: class 6 cleared, class 3 set; renamed "file"; "There's a file in it!" (`obj-0420-cake.txt [0056]`-`[0071]`). Use `cake` (7, 420) does the same (`[003E]`/`[0047]`). | none |

**Dialogue after J2** (`local-202`).

- Since class 6 is now clear and `Bit[476]` is unset, Otis says "So, have you
  come to release me?" (`[005C]`-`[0091]`). The menu is built at
  `[0148]`-`[05A8]`.
- With ego owning 640 and 643 after mansion visit 1, the visible choices are:

  | Verb | Choice | Shown when |
  |---|---|---|
  | 120 | "Who are you?" | `!Bit[91]` |
  | 125 | "Would you happen to have a file?" | ego owns 640, `!Bit[95]` |
  | **127** | **"…keep a stiff upper lip. I've gotta go."** | always |

  The sheriff line (122) is hidden because ego owns 643 (`[0302]`-`[030E]`).
- **127 → "Thanks a lot." → exit** (`[1381]`-`[13A9]` → `[19C4]`). Use the
  substring `stiff upper lip`.
- The other branches:
  - 120: Otis's backstory submenus, which loop back to the menu
    (`[05B7]`-`[0ADA]`).
  - 125: sets `Bit[95]` and `Bit[559]`, cake/rats lines, then exit
    (`[12A1]`-`[137E]`).
  - None of them affects J3.

### 4.4 Is the jail visit required? Yes (verified)

- Cake 420 has one source: it is picked up only at
  `room-031-jail/local-203 [01E0]`.
- Mints 395 also have one source: they are picked up only at
  `room-030-store/local-211 [1BC0]`.
- The other grep hits for both are in `global/script-001`, inside the
  boot-param branch (`[085F]` `if (Local[0] != 0)`; e.g. `[0AD0]`, `[1420]`).
  Boot params that change state are banned (`rules/glitchless.md`, Banned 1).
- **The hole needs the opened cake** (§3.4).
- Two jail visits are the minimum.
  - `Bit[420]` must be set before the storekeeper offers mints.
  - The mints must reach Otis before the repellent.
  - The repellent exists only after mansion visit 1.
  - J1 can be the first Talk-to, or any give except mugs, file or mints
    (§4.1), at any time before S1.
  - J2 and J3 must come after both S1 and F2.

---

## 5. (d) The theft sequence (room 53, second visit)

| # | Sentence | Preconditions | Effects | Dialogue choices |
|---|---|---|---|---|
| T1 | **Walk to `gaping hole` (11, 637)** | `Bit[481]` path done (637 touchable); ego owns 420 with class 6 cleared | `local-211` → `local-215` → `local-212` → `global/script-119` (below); **gain 635 fabulous idol** | 4 menus: `could have it`; `Uh`; `Um`; `Blfft` |
| T2 | **Open `door` (2, 633)** | ego owns 635 (`obj-0633 [0018]`/`[001D]`) | `local-217` → room 83 → `global/script-065` → **room 42** | 1 menu: `Buzz off` |

### T1 in detail (all `cutscene`; no timers that need input)

**`local-211`.**

- `[0002]` → `local-213`: ego climbs through and says "I've got the file"
  (`local-213 [0036]`-`[0054]`).
- Simulated sentences follow. Effects:
  - 641 renamed "stylish confetti", owner 14 (`[0050]`-`[008C]`);
  - 642 → owner 14 (`[00E3]`/`[00E7]`);
  - **`pickupObject(635,0)`** (`[016C]`);
  - **420 → owner 14** (`[0174]`/`[0178]`), i.e. the file is consumed;
  - **633 state 0**: the front door is now closed (`[018C]`).
- The Esc branch `[019E]`-`[01E4]` gives the same end state.

**`local-215`.** Ego is thrown out at (460,120): "Phew! … At least I got the
idol." (`[0063]`).

**`local-212`** (Fester, then the Governor).

- Dialogue at `[02AA]`-`[0453]`, verbs 120 to 124:

  | Verb | Choice | `Var[273]` |
  |---|---|---|
  | 120 | "She said I could have it!" | 0 |
  | 121 | "…just going to borrow it!" | 1 |
  | 122 | "It belongs in a museum!" | 2 |
  | 123 | "The pirate leaders told me…" (only if `Var[200] != 0`, `[0393]`) | 3 |
  | 124 | "…taking it out for a walk." (otherwise) | 4 |

- The choice sets `Var[273]` (`[0458]`-`[04A0]`). `Var[273]` only selects
  later lines (`[0545]`-`[0702]`, `global/script-119 [006B]`-`[019A]`,
  `room-083-cu-dock/local-201 [009E]`). All choices are equivalent.
  Pick `could have it` (verb 120), which has the shortest reply lines.
- The Governor enters ("What's going on here?") and Fester leaves. Then
  `startScript(119)` (`[0882]`).

**`global/script-119`** (Elaine close-up).

- It loads room 23 (`[000E]` `loadRoom(23)`). This is a script-forced room
  change.
- Three menus, each with four choices and **empty branches** (no state change):

  | Menu | Choices | Cite | Suggested substring |
  |---|---|---|---|
  | 1 | "Uh", "Gee", "Well", "Gosh" | `[01A9]`-`[02BC]` | `Uh` |
  | 2 | "Er", "Um", "Golly", "Jeepers" | `[037C]`-`[0492]` | `Um`. **Not `Er`**: it also matches "Jeepers". |
  | 3 | "Blfft", "Grlpyt", "Hrdrl", "Rldft" | `[05C0]`-`[06DB]` | `Blfft` |

- Ego is put back in room 53 at (365,131) (`[07C2]`). `actorFollowCamera`
  (`[07E4]`) switches the scene back to 53 (`camera.cpp:69-70`). Control is
  restored at `[092E]`/`[0930]`.

### T2 in detail

- **Why T2 is forced.** After T1 the foyer has only one exit:
  - Front door 633 is closed (`local-211 [018C]`), and its Walk to needs
    state 1.
  - Door 632 is locked (class 6, `local-210 [004B]`).
  - Closet 634 is locked (`local-214 [0029]`).
  - The hole refuses: "I'm not going back in there!" (§3.4).

  So Open `door` 633 is the only way out. With the idol owned, it runs
  `local-217` instead of opening the door.
- **`local-217`.**
  - Fester blocks the door. Menu at `[006E]`-`[01FF]`, verbs 120 to 123:
    "…safe-deposit box", "…make up and be friends", "…blocking the doorway",
    **"Buzz off, Fester."**
  - 122 and 123 share the shortest reply (`[02DC]`-`[02E9]`), so **choose
    `Buzz off`**. 120 and 121 have longer replies.
  - If ego owns sword 388, it is confiscated (owner 14, `[030F]`-`[0336]`).
  - **`setOwnerOf(635,0)`** (`[0388]`): the foyer idol is gone.
  - **`Var[277] = 1`** (`[038C]`).
  - `loadRoomWithEgo(904,83)` (`[0391]`).
- **Room 83 entry.** `Var[277] == 1` → `global/script-065`
  (`room-083-cu-dock/entry.txt [0016]`/`[001D]`). This is Fester's dock
  monologue, a cutscene with no input. It ends with `putActorInRoom(ego,42)`
  (`[023A]`) and `actorFollowCamera` (`[025C]`), i.e. **room 42**.

`Var[277] = 1` is set only at `local-217 [038C]`, and `global/script-065` is
called only from the cu-dock entry (grep). There is no other way into the
underwater room.

**Timers in §5: none.** Every dialogue waits for input forever (the
`Var[194]` polling loops). Override (Esc) points exist, but text and cutscene
skipping are out of scope for v1.

---

## 6. (e) The underwater sequence (room 42)

**Entry** (`room-042-underwate/entry.txt`):

- If the sword was not confiscated, the sunken sword 590 is hidden
  (`[0000]`-`[0010]`).
- Ego uses the slow costume (`[0017]`). `VAR_VERB_SCRIPT = 204` (`[0020]`).
- Actor 11 (the idol on the rope) is placed (`[002A]`-`[003C]`).
- Scripts 201 (rope), 205 (drowning timer) and 209 (cameo timer) start
  (`[007C]`-`[0082]`).
- Walk-box 1 is locked (`[0085]`).

| # | Sentence | Preconditions | Effects |
|---|---|---|---|
| U1 | **Pick up `fabulous idol` (9, 578)** | owner(578) == 15 (`obj-0578-fabulous-idol.txt [001B]`/`[0027]`) | `local-203`, then `local-200` (below). **Sets the trial-complete flags.** |

The ladder alone does not work. While 201 runs, Walk to `ladder` (577) gives
"…I'm tied to this stupid idol!" (`obj-0577-ladder.txt [000C]` →
`local-200 [0000]`/`[0009]`).

**`local-203`** (cutscene).

- Stops the drowning timer 205 and the cameo timer 209 (`[0002]`, `[0008]`).
- Unlocks box 1 (`[0004]`) and stops the rope script 201 (`[0016]`).
- **`pickupObject(578,0)`** (`[0024]`).
- If the sword was confiscated (owner 14), ego walks to sword 590 and takes it
  back with `setOwnerOf(388,VAR_EGO)` (`[0030]`-`[008B]`).
- Walks to the ladder (`[0094]`) and starts `local-200` (`[009E]`).

**`local-200`** (now that 201 is stopped).

- **`startScript(71,[2])`** at **`[0041]`**: the completion routine (§7).
- `Var[277] = 4` if `Var[196] < 3` (`[0076]`/`[007D]`), else 5 (`[0085]`).
- Ego is moved to room 83 (`[0091]`) and the camera follows (`[009D]`), i.e.
  **room 83**.

**Timers.**

- **Drowning** (`local-205`):
  - `delay(28800)`, then "…how much longer I can hold my breath." (`[0000]`/`[0008]`).
  - Then 3600 + 1800 + 900 + 900 more jiffies (`[0042]`-`[0066]`).
  - At **36000 jiffies (600 s) after entry**, ego dies: `UserputOff`,
    `doSentence(STOP)`, and `VAR_VERB_SCRIPT = 211`. Verbs then only
    highlight (`[006A]`-`[00CE]`, `local-211`), so the run is at a dead end.
  - U1 is the first possible action and stops 205 at once
    (`local-203 [0002]`), so the timer poses no risk.
- **Cameo** (`local-209`): after 5400 jiffies, two pirates chat (`local-208`).
  This is cosmetic. If it is still running at U1, `local-210` plays two more
  lines (`local-203 [000A]`/`[0013]`).

---

## 7. (f) Completion and what comes after

**Completion routine** (`global/script-071`, called with `Local[0] = 2` from
`room-042-underwate/local-200 [0041]`):

| Effect | Cite |
|---|---|
| `Var[196] += 1` (trials completed) | `[0088]` |
| **`Bit[85] = 1`** (`Bit[83 + 2]`) | `[008D]` |
| **`Var[200] = 2`** (`Var[198 + 2]`, idol trial done) | `[0094]` |

- **Goal condition:** `{"bit": 85, "eq": 1}`. This matches
  `docs/part1/goal-flags.md`.
- Other readers of `Bit[85]`:
  - the dock poster (`room-033-dock/obj-0429-poster.txt [003D]`);
  - the leaders' "what's left" line (`room-028-bar/local-220.txt [020A]`);
  - the store's rat-repellent gate (`room-030-store/local-211 [02CB]`).
- `Bit[85]` and `Var[200]` are set only here (grep).

**Nothing at the dock is required.**

- The flag is set at `local-200 [0041]`, inside the underwater cutscene,
  **before** the move to room 83.
- With `Var[277] == 4`, room 83's entry runs `room-083-cu-dock/local-201`
  (`entry [0084]`/`[008E]`). That is the Elaine rescue scene: cutscene only,
  no menu. It ends with control in room 83 (`[07A0]`).
  - It is **forced but not required**. The trial already counts.
  - The bridge's goal check fires on the first frame where `Bit[85]` holds,
    which is before this scene.
- **If the idol is the third trial**, `Var[196]` reaches 3 and
  `local-200 [0085]` sets `Var[277] = 5`. Room 83 then runs
  `room-083-cu-dock/local-203`, the kidnapping scene, which has a menu
  (`[0243]`-`[03C4]`). v1 (treasure + idol) never gets there.
- **Showing the idol to the pirate leaders is optional.** Give `fabulous idol`
  (578) to `important-looking pirates` (322) →
  `room-028-bar/obj-0322-important-looking-pirates.txt [0046]`/`[0050]` →
  `local-220 [1501]`-`[157D]`. This removes the idol and sets `Var[200] = 3`
  and `Var[197] += 1`. It does not touch `Bit[85]` or `Var[196]`.
- **Talking to the leaders first is not required.** No script in this chain
  checks `Var[197]` or `Var[200]`. The only effect is the extra choice 123 in
  §5.

---

## 8. Ordering constraints (partial order, verified)

1. **Petal before drugging.** P3 (petal) → drug the meat (A or B). The meat
   needs the kitchen (§2.3).
2. **Dogs before the mansion.** Drugged meat → D1 → E1/E2. E needs `Bit[15]`:
   door 465 is untouchable and the walk-boxes are locked until then.
3. **Repellent from visit 1.** E2 → F1 → F2. F2 gives 640 and makes 637
   touchable.
4. **Mints unlocked by the jail.** J1 (`Bit[420]`) → S1 (mints; needs
   `Var[195] ≥ 1`).
5. **Mints before repellent.** S1 → J2 → J3. J3 also needs F2.
6. **File before the hole.** J3 → J4 (open cake) → T1. T1 also needs F2.
7. **Then the end.** T1 → T2 → U1. Every step from T1 on is forced except
   the three sentences T1, T2 and U1.

Free choices for the planner:

- when J1 happens (any jail visit before S1);
- whether the meat is picked up before or after P3;
- where J4 happens (any room after J3).

---

## 9. Minimal action sequence from the Lookout (one valid ordering)

**Start:** free control at the Lookout (room 38), no items.
**Prerequisite:** ≥ 1 piece of eight before S1. This comes from `money.md`
and is not counted here.

The table uses only room-to-room links verified in this file. Routing
between rooms is left to `rooms.md`. It may find shorter paths, and the
Lookout stairs go straight to the dock.

**Legend:**

- **PT**: room change caused by this sentence.
- **FT**: room change forced by a script (`loadRoom`, `actorFollowCamera`).
- **C**: dialogue choices answered within the step.

| # | Sentence (verb, obj[, obj2]) | Room | PT | FT | C |
|---|---|---|---|---|---|
| 1 | Walk to `path` (11, 487) | 38 | 38→85 (`room-038-lookout/obj-0487-path.txt [0010]`) | | |
| 2 | Walk to `fork` (11, 911) | 85 | 85→218 | | |
| 3 | Walk to `path` (11, 685) | 218 | 218→215 | | |
| 4 | Pick up `plants` (9, 678) | 215 | | | |
| 5 | Walk to `path` (11, 687) | 215 | 215→218 | | |
| 6 | Walk to `path` (11, 687) | 218 | 218→85 | | |
| 7 | Walk to `village` (11, 917) | 85 | 85→33 (`room-085-melee/obj-0917-village.txt [0046]`) | | |
| 8 | Open `door` (2, 428) | 33 | | | |
| 9 | Walk to `door` (11, 428) | 33 | 33→28 (`room-033-dock/obj-0428-door.txt [0029]`/`[0035]`) | | |
| 10 | Walk to `door` (11, 316), **after waiting for the cook** (§2.3; race in §12) | 28 | 28→41 | | |
| 11 | Use `hunk of meat` with `yellow petal` (7, 566, 689) | 41 | | | |
| 12 | Walk to `door` (11, 570). Under direct push, possibly preceded by Open `door` (2, 570) (§12). | 41 | 41→28 | | |
| 13 | Walk to `door` (11, 315) | 28 | 28→33 (`room-028-bar/obj-0315-door.txt [0072]`-`[0090]`) | first exit only: `global/script-120` loads rooms 0, 70, 72 (`[0005]`, `[002F]`, `[00B3]`) before `[0538]` | |
| 14 | Walk to `archway` (11, 427) | 33 | 33→35 | | |
| 15 | Walk to `archway` (11, 451) | 35 | 35→34 | | |
| 16 | Walk to `doorway` (11, 434) | 34 | 34→31 | | |
| 17 | Talk to `prisoner` (10, 405) (J1) | 31 | | | |
| 18 | Walk to `doorway` (11, 400) | 31 | 31→34 | | |
| 19 | Open `door` (2, 437) | 34 | | | |
| 20 | Walk to `door` (11, 437) | 34 | 34→30 | | |
| 21 | Talk to `storekeeper` (10, 394) (S1) | 30 | | | `breath mint`, `browse` |
| 22 | Walk to `door` (11, 387) | 30 | 30→34 | | |
| 23 | Walk to `Governor's mansion` (11, 431) | 34 | 34→36 | | |
| 24 | Give `hunk of meat` to `poodles` (4, 566, 467) (D1) | 36 | | | |
| 25 | Open `door` (2, 465) | 36 | | | |
| 26 | Walk to `door` (11, 465) | 36 | 36→53 | | |
| 27 | Open `door` (2, 632) | 53 | | | |
| 28 | Walk to `door` (11, 632), which plays cutscene 210 | 53 | | | |
| 29 | Walk to `door` (11, 633) | 53 | 53→36 | | |
| 30 | Walk to `trail` (11, 466) | 36 | 36→34 | | |
| 31 | Walk to `doorway` (11, 434) | 34 | 34→31 | | |
| 32 | Give `breath mints` to `prisoner` (4, 395, 405) (J2) | 31 | | | `stiff upper lip` |
| 33 | Give `gopher repellent` to `prisoner` (4, 640, 405) (J3) | 31 | | | |
| 34 | Open `cake` (2, 420) (J4) | 31 | | | |
| 35 | Walk to `doorway` (11, 400) | 31 | 31→34 | | |
| 36 | Walk to `Governor's mansion` (11, 431) | 34 | 34→36 | | |
| 37 | Walk to `door` (11, 465) | 36 | 36→53 | | |
| 38 | Walk to `gaping hole` (11, 637) (T1) | 53 | | 53→23→53 (`global/script-119 [000E]`, `[07C2]`/`[07E4]`) | `could have it`, `Uh`, `Um`, `Blfft` |
| 39 | Open `door` (2, 633) (T2) | 53 | | 53→83 (`local-217 [0391]`), 83→42 (`global/script-065 [023A]`/`[025C]`) | `Buzz off` |
| 40 | Pick up `fabulous idol` (9, 578) (U1) → **goal** | 42 | | 42→83, after the flag (`local-200 [0091]`/`[009D]`) | |

**Totals:** 40 sentences (plus money), 24 PT, 5 to 8 FT (3 of them only on the
first bar exit), 8 dialogue choices.

Variant steps:

- **Storekeeper away** (§4.2): add Push `bell` (5, 399) instead of step 21,
  and Open `door` (2, 387) before step 22. That is +1 sentence, and the menu
  appears without the Talk.
- **Get-caught shortcut** (§2.3): insert Open `door` (2, 316) before step 10.
  That is +1 sentence in exchange for a shorter wait.
- **Separate pick-up.** If the bridge cannot handle the two-sentence auto
  pickup of step 11 (§12), split it into Pick up `hunk of meat` (9, 566) and
  then Use (7, 566, 689). That is +1 sentence.

---

## 10. Prices and money

- **Breath mints: 1 piece of eight**, debited via `startObject(488,250,[0,1])`
  (`room-030-store/local-211 [1BC8]`).
- **Money is `Var[195]`.** Object 488 verb 250 computes
  `Var[195] = Var[195] + Local[0] - Local[1]` (`obj-0488 [0086]`-`[008B]`).
  When the total drops below 1, 488 leaves the inventory (owner 14,
  `[00B7]`/`[00BE]`).
- Nothing else in the idol chain costs money.

---

## 11. Timers and RNG in this chain

| What | Value | Cite | Planning impact |
|---|---|---|---|
| Cook stays in the kitchen after each bar entry | 1800-3000 jiffies (`rand(0..20)`) | `room-028-bar/local-211 [000D]` | Must wait before step 10. Get-caught alternative: 600 jiffies + cutscene. |
| Cook's destination in the bar | object 330 + `rand(0..28)` | `room-028-bar/local-216 [003F]` | Decides whether x ≤ 310 occurs (click guard) |
| Storekeeper away on entry | 1/4 (`rand(0..3) == 0`) | `room-030-store/entry.txt [002B]`/`[0042]` | +1 or 2 sentences |
| Barking dogs | cosmetic `rand` delays | `room-036-mansion-e/local-202 [0028]` | none |
| Foyer sound/cloud effects | cosmetic | `room-053-foyer/local-207`, `local-219` | none |
| Drowning | 36000 jiffies | `room-042-underwate/local-205` | none (U1 stops it) |
| Underwater cameo | 5400 jiffies | `room-042-underwate/local-209 [0000]` | cosmetic |

The seed is fixed (`rules/glitchless.md`), so these outcomes are deterministic
for a given frame history. They still depend on the route, so they have to be
read from engine state or observed, not guessed.

---

## 12. Modelling hazards: direct sentence push vs. the room's input script

A real click goes through `VAR_VERB_SCRIPT` (`runInputScript`,
`script.cpp:1478-1511`), which then queues the sentence. The bridge pushes
sentences straight into the queue (`docs/plan.md` C4), so room input scripts
**do not run**. `rules/glitchless.md` (Banned 4) allows only sentences that
the input script would push after a click, so the model must reproduce any
guard that lives in an input script.

- **Bar, door 316 (blocks modelling of step 10).**
  - The guard (cook in the bar with x > 310, so the click is refused) exists only in
    `room-028-bar/local-203 [001E]`, reached from the input script
    `local-202 [001C]`.
  - Under direct push, Walk to 316 succeeds whenever 316 has state 1.
  - The plan should therefore push step 10 only while the cook is in room 28
    with x ≤ 310.
  - **Gap:** C3 conditions (`docs/plan.md`) cover vars, bits, room, owner and
    state. They cannot express actor room or position, or whether a script is
    running.
    - `{"state": 316, "eq": 1}` does express "cook is out of the kitchen"
      (§2.3).
    - Nothing expresses x ≤ 310.
- **Mansion notice.**
  - After D1, `VAR_VERB_SCRIPT = 203` (`room-036-mansion-e/local-201 [01B9]`).
    A real click would run mansion `local-203`, which hides notice 468 and
    restores 4.
  - Under direct push, 203 stays set. Nothing resets it until:
    - the High Street exit (`room-034-high-stre/exit.txt [0021]`);
    - `room-053-foyer/local-217 [0379]`;
    - the underwater entry.
  - The dialogue scripts save and restore it (e.g. `room-053-foyer/local-212 [0297]`/`[04AC]`).
  - This is harmless **only if** the bridge never calls the input script
    outside dialogue menus. A stale 203 in room 53 or 34 would run that
    room's unrelated local 203.
  - The notice text may also stay on screen. That is cosmetic.
- **Underwater 204.** It only updates `Var[164]`, the rope animation target
  (`room-042-underwate/local-204 [0007]`-`[0030]`). This is cosmetic.
- **Bar, kitchen-door race (direct push only).**
  - On the click path, `local-203 [0030]`-`[0035]` freezes the cook (stops
    216 and 217) as soon as the click lands.
  - Under direct push nothing freezes him. While ego walks to door 316 (and
    during the `local-218` walk-in cutscene), `local-216` keeps running. It
    can reach `[0080]`, `startObject(316,3)` → `26 [316,570]`, which closes
    **both 316 and 570**. Room-28 local scripts die only when the room
    changes.
  - Possible results:
    - Step 10 does nothing because 316 is at state 0. The next step then
      fails with `room_mismatch`.
    - Ego arrives in the kitchen with 570 closed, so step 12's Walk to does
      nothing.
  - `until {"state":316,"eq":1}` is necessary but not sufficient. Safe
    fallback for 570: push Open (2, 570) before step 12, for +1 sentence
    (no-op if already open).
  - Emulating the bar's input script would remove the race and the x ≤ 310
    gap together (open question 1).
- **Bar 202, other clicks.** On every idle click it restarts the cook's walk
  (`local-202 [0034]`-`[005B]`). A click-faithful replay of the waiting period
  would include this. Direct push does not.
- **Two engine sentences from one plan step.** Step 11 with the meat on the
  table queues (7,566,689) and (9,566) (`global/script-002 [0232]`/`[0239]`).
  The bridge's "consumed → sentence-idle" test must wait for the sentence
  stack to empty.
- **Untouchable objects.** Direct push could target objects a player cannot
  click: door 465 before `Bit[15]`, the hole 637 before F2. Such sentences
  are input fabrication and must not be modelled. They would also fail on the
  locked walk-boxes or the item checks.

---

## 13. Open questions

1. **Cook guard representation (blocks the meat step).** How should the plan
   wait for "cook in the bar with x ≤ 310"?
   - Options: add actor-position or script-running conditions to C3, or
     have the bridge emulate the room input script.
   - It is also unknown which of objects 330 to 358 lie at x ≤ 310. That needs
     `objects.json`. If none do, the cook must be caught mid-walk.
   - Emulating the input script would also remove the door-closing race
     (§12).
2. **Get-caught timing.** Is "Open 316 while the cook is inside" always
   faster than waiting? It depends on the random 211 delay versus the 214
   cutscene + 600 jiffies. Measure in engine.
3. **Reach checks.** Do these pass the distance checks from the positions
   that walking actually reaches?
   - Give (≤ 32) for the poodles (walk-boxes 1-6 locked).
   - Walk to `gaping hole` 637 (≤ 16, box 16 locked).
   - Pick up idol 578 (≤ 16, box 1 locked).

   These are the designed solutions, so this is **inferred** but not
   verified.
4. **Owner meanings.** Owners 13 and 14 are inferred (§Conventions).
5. **`Var[116] = 10`** at `local-201 [0082]`. Its purpose was not analysed. It
   is used as a script number by the sentence script's give-to-actor path
   (`global/script-002 [0127]`).
6. **Citation format.** `docs/plan.md` (Phase 3 / `test_every_action_is_cited`)
   expects `data/scripts/<file>:<line>`. This brief and this file use
   `[XXXX]` offsets. The PDDL `; src:` lines need one convention.
7. **Bridge dialogue picking.** Choice matching assumes the bridge compares
   against the verb's internal string. If it compares rendered text, `^` and
   the `\x0F` / `\x88` escape codes may differ. The substrings chosen here
   avoid both.
