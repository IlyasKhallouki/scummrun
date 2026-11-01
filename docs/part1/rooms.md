# Part I (Mêlée Island): rooms and connectivity

Scope: every room Guybrush can be in during Part I, every room change he can cause with a sentence, every automatic room change, door handling, and an adjacency list for PDDL `(link a b)` facts.

Citation format: `data/scripts/<file> [XXXX]`, where `XXXX` is the descumm byte offset (see `data/scripts/INDEX.md`). The ScummVM source is cited by function name in `third_party/scummvm/engines/scumm/`.

Labels used below:

- **verified**: read directly in the cited script.
- **derived**: computed from index or raw room data, not from scripts. The method is given in §0.3.
- **inferred**: a reading of how several facts combine, not stated in any one place.

---

## 0. Conventions and engine facts this document relies on

### 0.1 Script semantics (verified)

- **Class tests are inverted for small numbers.**
  - `setClass(obj,[N])` with N ≥ 128 *sets* class N−128. With N < 128 it *clears* class N. `[0]` clears all classes (`script_v5.cpp`, `ScummEngine_v5::o5_setClass`).
  - `classOfIs(obj,[N])` is true when class N−128 is set (N ≥ 128), or when class N is **not** set (N < 128) (`ScummEngine_v5::o5_ifClassOfIs`; `object.cpp`, `ScummEngine::getClass`).
  - So `setClass(x,[160])` makes x untouchable (class 32), and `setClass(x,[32])` makes it touchable again. `classOfIs(door,[6])` means "door is not locked".
- **Door open/close.**
  - Global script 25 opens a door only if it does **not** have class 6 (`data/scripts/global/script-025.txt [0016]`, `[0024]`). Otherwise Guybrush says the door "appears to be locked" (`[0040]`).
  - It also opens the partner door (2nd argument) unless the partner has class 11 (`data/scripts/global/script-025.txt [002D]` to `[0036]`).
  - Global script 26 closes both the same way (`data/scripts/global/script-026.txt [001B]`, `[0024]` to `[002D]`).
  - "Use door" runs global script 31, which turns into Close if the door is open and Open otherwise (`data/scripts/global/script-031.txt [000F]` to `[001F]`).
- **Fallback verb.** An object's verb table entry `FF` (255) catches any verb with no entry of its own. So "Walk to" works on objects whose table is `[90, 255]` (the per-file headers say this; also `ScummEngine::getVerbEntrypoint`). Verb 90 is the mouse-hover "default verb" hook (`data/scripts/global/script-023.txt [0147]`). It never changes rooms.
- **Sentence script** (`VAR_SENTENCE_SCRIPT = 2`, `data/scripts/global/script-001.txt [07DA]`):
  - For a room object, it first walks the ego to the object's walk point (`data/scripts/global/script-002.txt [02DB]`).
  - If the ego ends more than 16 px away (Chebyshev distance, `object.cpp ScummEngine::getDist`), it prints "I can't reach it." and runs nothing (`data/scripts/global/script-002.txt [02EB]` to `[0317]`).
  - Exceptions:
    - Objects with class 8 are run without walking (`data/scripts/global/script-002.txt [02B4]` to `[02BE]`).
    - Objects with class 10 walk to the mouse position (`data/scripts/global/script-002.txt [02C1]` to `[02D5]`).
    - No Part I exit object starts with class 8 or 10 (derived, DOBJ). Only two things in room 48 skip the walk: the cables 603 to 606, which have class 8 from the start, and the poles, which get class 8 while you stand on top of them (`data/scripts/room-048-crossing/local-204.txt [00CF]`; cleared by `data/scripts/room-048-crossing/local-205.txt [00BB]`).
  - Then it calls `startObject(objA, verb, [objB, verb])` (`data/scripts/global/script-002.txt [039D]`). In the object script, `Local[0]` = objB and `Local[1]` = verb.
- **Room change opcodes.**
  - `loadRoomWithEgo(obj, room, x, y)` puts the ego at `obj`'s walk point in `room` and then, if x ≠ −1, starts walking it to (x, y) (`ScummEngine_v5::o5_loadRoomWithEgo`).
  - `putActorInRoom(VAR_EGO, r)` followed by `actorFollowCamera(VAR_EGO)` also changes room (`camera.cpp ScummEngine::setCameraFollows` calls `startScene` when the followed actor is not in the current room).
  - `loadRoom(r)` changes room without moving the ego. Part I uses it only in cutscenes and two places noted below.
  - The room change kills the calling object/room script, so code after a taken `loadRoomWithEgo` never runs (inferred from `startScene` killing room scripts). This is why phase-B redirects (§2) are written as "if … loadRoomWithEgo(83); … default loadRoomWithEgo(dock)".
- **Global entry/exit scripts.**
  - The second room-entry global script (`VAR_ENTRY_SCRIPT2`) is 6 (`data/scripts/global/script-001.txt [0669]`). The first one, script 5, is empty. Script 6 does `doSentence(STOP)` on **every** room entry, so the sentence queue is flushed whenever the room changes (`data/scripts/global/script-006.txt [0065]`). It also stores the entry object in `Var[113]` (`[0060]`).
  - The exit global script 7 sets `Var[101] = VAR_ROOM` (`data/scripts/global/script-007.txt [0000]`), so `Var[101]` in entry scripts is **the previous room**.
- **Pseudo-rooms.**
  - `PseudoRoom(58,73,…,92)` in the boot script maps room numbers 201 to 220 onto physical room 58 (the forest) (`data/scripts/global/script-001.txt [0736]` to `[0762]`). descumm prints the byte minus 128.
  - `VAR_ROOM` holds the pseudo number (201 to 220), and the forest entry script switches on it.
- **Trials counter.** `Var[196]` counts completed trials. Script 71 increments it and sets `Bit[83+n]` for trial n (`data/scripts/global/script-071.txt [0088]`, `[008D]`):
  - trial 1 (sword): `data/scripts/global/script-116.txt [0117]`
  - trial 2 (idol): `data/scripts/room-042-underwate/local-200.txt [0041]`
  - trial 3 (treasure): `data/scripts/room-064-treasure/local-200.txt [0214]`

### 0.2 Phases

| phase | condition | what changes for connectivity |
|---|---|---|
| A | `Var[196] < 3` | normal Part I |
| B | `Var[196] ≥ 3` (all three trials) | <ul><li>The redirects to the dock close-up 83 become live (§2.8).</li><li>The mansion half of High Street is blocked (T20).</li><li>The cook no longer guards the kitchen (T27).</li></ul> |
| - | `Bit[453]` | Part IV (wedding) flag. It is never set in Part I, and many exits test `!Bit[453]` (e.g. `data/scripts/room-034-high-stre/obj-0432-alley.txt [000C]`). |

The project's v1 goal (treasure trial and idol trial) is reached in phase A whatever the trial order. Both goal flags are set by `startScript(71,…)` (§0.1) before any phase-B effect can run. At the latest, phase B begins at the same instant the second goal flag is set.

### 0.3 Data taken from outside the scripts (derived)

These facts are not in any script. They were read from the raw game data with a small throwaway parser and should be re-checked by the bridge's object dump.

- **Initial object class/state**: the `DOBJ` block of `game/classic/MONKEY1.000`.
  - Format: XOR 0x69; `uint16 count`, then `count` bytes (owner in the low nibble, state in the high nibble), then `count` × `uint32 LE` class words. Bit k means class k+1 (`resource.cpp ScummEngine::readGlobalObjects`).
  - Results used here:
    - Every Part I exit object starts in state 0.
    - Doors with class 6 (locked) at start: only 478 (Meathook's inner door) and 634 (foyer closet). No door has class 11.
    - Untouchable (class 32) at start: 918 (map "Sword Master's"), 654 (bridge east path), 439 (High Street poodles), 656 (forest ravine), 635 (idol).
    - Class 6 at start on map objects 912 and 915. The map uses this only for naming.
- **Object rectangles and walk points**: `CDHD` inside `build/blocks/DISK_0001/LECF/LFLF_0RRR/ROOM/OBCD_0OOO`. Layout: `uint16 id, u8 x/8, u8 y/8, u8 w/8, u8 h/8, u8 flags, u8 parent, int16 walk_x, int16 walk_y, u8 dir`.
- **Room width**: `RMHD` (`uint16 width, height, numObjects`) in the same folder.
- **Walk boxes**: `BOXD` (v5: `uint16 n`, then 20-byte boxes of 4 corners + mask + flags + scale) and `BOXM` (per box: `(from,to,next)` triples ended by 0xFF), in the same folder.

---

## 1. Room table

"Control" means the player gets the cursor in that room. Rooms marked *cutscene only* are entered and left inside one script. They need no PDDL node.

| room | short name | description | close-up | control | notes |
|---:|---|---|---|---|---|
| 38 | lookout | Lookout point on the cliff. The game starts here. | no | yes (after intro) | start room |
| 96 | part1 | "Part One" title card | - | no | auto, T01 to T02 |
| 33 | dock | Mêlée town dock (1008 px wide, scrolling) | no | yes | |
| 35 | low-stree | Lower street: Voodoo shop door, four "random" doors, map seller, archways (480 px) | no | yes | |
| 34 | high-stre | Upper street: store, jail, church, alley, path to mansion (800 px). Camera pinned to one of two halves. | no | yes | split into **34T** (town) and **34M** (mansion side), §2.3 |
| 28 | bar | SCUMM Bar (640 px). Camera pinned to left or right half. | no | yes | split into **28L** and **28R**, §2.4 |
| 41 | kitchen | Bar kitchen with pier door | no | yes | |
| 29 | fortune | Voodoo Lady's shop (496 px) | no | yes | |
| 30 | store | Shop | no | yes | |
| 31 | jail | Jail (Otis) | no | yes | |
| 32 | alley | Dark alley. First visit: Fester cutscene. | no | yes | |
| 36 | mansion-e | Governor's mansion exterior, piranha poodles | no | yes | |
| 53 | foyer | Governor's mansion foyer (640 px) | no | yes | |
| 78 | church | Mêlée church. In Part I nothing runs unless object 823 (a Part IV item) is owned (`data/scripts/room-078-church/entry.txt [000E]` to `[0027]`). | no | yes | dead end, not needed |
| 85 | melee | Island overview map | - | yes | §2.5 |
| 48 | crossing | Cable crossing to Hook Isle | no | yes | split into 4 nodes, §2.7 |
| 37 | meats-hou | Meathook's house (480 px) | no | yes | forced dialogue on entry |
| 43 | trainers- | Captain Smirk's house, outside | no | yes | east of troll |
| 57 | bridge | Troll bridge | no | yes | |
| 58 | damnfores | Forest maze, pseudo-rooms 201 to 220 | no | yes | 21 nodes, §2.9 |
| 59 | stans | Stan's Previously Owned Vessels (640 px) | no | yes | east of troll |
| 61 | sword-mas | Sword Master's clearing (464 px) | no | yes | |
| 64 | treasure- | Treasure site "X" (496 px) | no | yes | |
| 52 | circus-gr | Circus clearing (496 px) | no | yes | |
| 51 | circus-te | Circus tent (432 px) | no | yes | |
| 42 | underwate | Under the dock, tied to the idol | no | yes | entered only by cutscene |
| 83 | cu-dock | Dock close-up for story cutscenes | yes | yes (after cutscenes) | |
| 10 | logo | Intro/credits (global script 152) | - | no | cutscene only |
| 79, 81, 82 | cu-bar-1/2/3 | Close-ups for bar pirate conversations, global scripts 91/92/93. Each ends back in 28 (`data/scripts/global/script-091.txt [12A6]`, `data/scripts/global/script-092.txt [0E78]`, `data/scripts/global/script-093.txt [058E]`). | yes | no | cutscene only |
| 60, 76 | gym, cu-traine | Smirk's training, inside global script 57 (`data/scripts/global/script-057.txt [05A0]`, `[1033]`). Ego is put back at door 591 in 43 (`[1E84]` to `[1E8E]`). | 76 yes | no | cutscene only |
| 44, 62 | masters-h, cu-sword | Sword Master fight/house, global scripts 116/139/58/64/124. Each ends with ego back in 61 (`data/scripts/global/script-116.txt [013E]`, `data/scripts/global/script-139.txt [0239]`, `data/scripts/global/script-058.txt [0A39]`, `data/scripts/global/script-064.txt [055B]`, `data/scripts/global/script-124.txt [0359]`). | 62 yes | no | cutscene only |
| 49 | road | Insult duel with a wandering map pirate. Global script 114 loads it and returns ego to 85 (`data/scripts/global/script-114.txt [0035]`, `[006E]` to `[0079]`). | no | no | cutscene only, random (§2.5, §8 Q2) |
| 50 | cu-brush | Close-up when Meathook's inner door is first opened (`data/scripts/global/script-049.txt [008D]`, `[00A2]`) | yes | no | cutscene only |
| 63 | map | Dance-step chart for the forest map 442. Global script 123 (`data/scripts/global/script-123.txt [0235]`, `[0264]`) is started by using/looking at object 442. | yes | no | cutscene only |
| 23 | cu-gov | Governor close-up during the foyer scene, global script 119 (`data/scripts/global/script-119.txt [000E]`, back to 53 at `[07C2]`). | yes | no | cutscene only |
| 70, 72 | hellcliff, gh-captai | Monkey Island rooms used by the "Meanwhile" cutscenes, global scripts 120/121 (`data/scripts/global/script-120.txt [002F]`, `[00B3]`; `data/scripts/global/script-121.txt [0034]`, `[00F4]`) | - | no | cutscene only |
| 19 | sh-deck | Ship deck. Part II starts here (T38). | - | - | end of Part I |

Not Part I (verified by their callers or entry objects): rooms 1 to 21 other than 10 and 19 (Monkey Island/ship), 25, 27, 39, 40 (pond: its exit goes to room 4, `data/scripts/room-040-pond/obj-0554-jungle.txt [000C]`), 45, 65, 69, 71 to 77, 80 (fort: exit to room 3, `data/scripts/room-080-fort/obj-0886-path.txt [000C]`), 84 (loaded from the ship, `data/scripts/room-007-sh-captai/obj-0085-piece-of-paper.txt [001E]`), 86 (started from room 25), 89 (Part IV ending, via global scripts 132→151→138), 95/97/98.

**Count:** 26 player-controllable Part I rooms: 24 ordinary rooms, the map 85, and forest room 58 with its 20 pseudo-rooms. After the splits in §5 that is 51 PDDL location nodes:

- 30 non-forest nodes (the 34, 28 and 48 splits included);
- 21 forest nodes (F207 is split in two).

There are also 15 cutscene-only rooms: 10, 96, 79, 81, 82, 60, 76, 44, 62, 49, 50, 63, 23, 70 and 72. Room 19 is the Part II terminal.

---
## 2. Transitions

Each row is one room change. Columns:

- **Sentence**: `(verb, objA[, objB])` with object names. "any" means the object only has an `FF` entry, so every verb works. Use `11` (Walk to).
- **Room change**: the call site that performs it.
- **Preconditions**: checks made before that call, with citations.

Conventions:

- "→83 redirect" means the phase-B redirect described in §2.8.
- `T` numbers are referenced from §5.
- An **(auto)** row has no sentence of its own: a cutscene or script performs the move.

### 2.1 Game start and the lookout (38)

| T | from → to | sentence | room change | preconditions / notes |
|---|---|---|---|---|
| T00 | boot → 38 | (auto) | `data/scripts/global/script-001.txt [1483]` putActorInRoom(ego,38), `[1487]` putActor(320,72), `[1495]` actorFollowCamera | Natural boot (`Local[0]==0`) first runs the intro, global script 152, and waits for it (`[07B8]` to `[07CD]`). |
| T01 | 38 → 96 | (auto) | The intro cutscene ends with `startObject(486,11)` (`data/scripts/room-038-lookout/local-203.txt [02CB]`). That runs `data/scripts/room-038-lookout/obj-0486-stairs.txt [0054]` loadRoom(96). | `!Bit[395]` (`[004A]`), then `Bit[395]=1` (`[004F]`). The intro is local-203, started on the first visit (`data/scripts/room-038-lookout/local-200.txt [0042]` to `[004F]`). The player never has control in 38 before this. |
| T02 | 96 → 33 | (auto) | `data/scripts/room-096-part1/local-200.txt [003B]` loadRoomWithEgo(426,33,346,133) | The player's first control is on the dock. |
| T03 | 38 → 33 | (11, 486 "stairs") any | `data/scripts/room-038-lookout/obj-0486-stairs.txt [0059]` loadRoomWithEgo(426,33,346,133) | `Bit[395]` is set (always true after T01). Phase B: →83 redirect first (`[0010]` to `[0042]`). |
| T04 | 38 → 85 | (11, 487 "path") any | `data/scripts/room-038-lookout/obj-0487-path.txt [0010]` loadRoomWithEgo(913,85) | none |

### 2.2 Town: dock (33), low street (35), voodoo shop (29)

| T | from → to | sentence | room change | preconditions / notes |
|---|---|---|---|---|
| T07 | 33 → 38 | (11, 426 "cliffside") | `data/scripts/room-033-dock/obj-0426-cliffside.txt [000C]` loadRoomWithEgo(486,38,244,106) | none |
| T08 | 33 → 35 | (11, 427 "archway") | `data/scripts/room-033-dock/obj-0427-archway.txt [000C]` loadRoomWithEgo(450,35) | none |
| T09 | 33 → 28L | (11, 428 "door") | `data/scripts/room-033-dock/obj-0428-door.txt [0035]` loadRoomWithEgo(315,28) | `getObjectState(428)==1` (`[0029]` to `[002E]`). First do (2, 428) Open, `[0015]` → script 25 [428,315]. |
| T10 | 35 → 29 | (11, 444 "door") | `data/scripts/room-035-low-stree/obj-0444-door.txt [0067]` loadRoomWithEgo(367,29) | `!Bit[453]` (`[0030]`); state(444)==1 (`[005B]` to `[0060]`). Open first: (2, 444), `[0018]` → script 25 [444,367]. |
| T11 | 35 → 33 | (11, 450 "archway") | `data/scripts/room-035-low-stree/obj-0450-archway.txt [0091]` loadRoomWithEgo(427,33,992,114) | `!Bit[453]` (`[000C]`). Runs the perspective walk local-204 first (`[0044]` to `[0056]`). Phase B: →83 redirect (`[0057]` to `[0089]`). |
| T12 | 35 → 34T | (11, 451 "archway") | `data/scripts/room-035-low-stree/obj-0451-archway.txt [001F]` loadRoomWithEgo(433,34,750,128) | none (perspective walk local-205 at `[0011]`) |
| T24 | 29 → 35 | (11, 367 "door") | `data/scripts/room-029-fortune/obj-0367-door.txt [007E]` loadRoomWithEgo(444,35) | state(367)==1 (`[0072]` to `[0077]`). The shop closes 367 behind you on entry (`data/scripts/room-029-fortune/local-200.txt [0017]`), so (2, 367) Open (`data/scripts/room-029-fortune/obj-0367-door.txt [0018]` to `[0034]`, script 25 [367,444]) is always needed. |

Low-street doors 445 to 448 are **not** room exits. Open/Use runs local-207, which moves the ego out of another of those doors in the same room (`data/scripts/room-035-low-stree/obj-0445-door.txt [0012]`; `data/scripts/room-035-low-stree/local-207.txt [0026]` to `[0082]`).

### 2.3 High Street (34) and what hangs off it

High Street is 800 px wide (derived, RMHD). The entry script pins the camera, so it is modelled as two nodes:

- `RoomScroll(160,160)` when you come from the mansion (room 36). This is **34M**, the mansion half.
- `RoomScroll(528,1121)` otherwise. This is **34T**, the town half.

Cite: `data/scripts/room-034-high-stre/entry.txt [0046]` to `[0056]`.

The walk boxes are all connected (derived, BOXM), so the ego *could* walk between the halves. A player cannot click across, though. Mansion-side objects lie at x ≤ 330, and the town camera never shows x < 368. The game provides two crossing objects instead.

| T | from → to | sentence | room change | preconditions / notes |
|---|---|---|---|---|
| T13 | 34M → 36 | (11, 431 "Governor's mansion") | `data/scripts/room-034-high-stre/obj-0431-governor-s-mansion.txt [000C]` loadRoomWithEgo(466,36) | none. Alias: 439 "deadly piranha poodles" → `startObject(431,11)` (`data/scripts/room-034-high-stre/obj-0439-deadly-piranha-poodles.txt [000C]`). 439 is untouchable until room 36 clears it (`data/scripts/room-036-mansion-e/entry.txt [0000]`). |
| T14 | 34T → 32 | (11, 432 "alley") | `data/scripts/room-034-high-stre/obj-0432-alley.txt [0011]` loadRoomWithEgo(422,32) | `!Bit[453]` (`[000C]`) |
| T15 | 34T → 35 | (11, 433 "archway") | `data/scripts/room-034-high-stre/obj-0433-archway.txt [0011]` loadRoomWithEgo(451,35) | `!Bit[453]` |
| T16 | 34T → 31 | (11, 434 "doorway") | `data/scripts/room-034-high-stre/obj-0434-doorway.txt [0011]` loadRoomWithEgo(400,31,262,120) | `!Bit[453]` |
| T17 | 34T → 30 | (11, 437 "door") | `data/scripts/room-034-high-stre/obj-0437-door.txt [005B]` loadRoomWithEgo(387,30) | state(437)==1 (`[004A]` to `[004F]`), `!Bit[453]` (`[0056]`). Open: (2, 437), `[0018]` → script 25 [437,387]. |
| T18 | 34T → 78 | (11, 438 "door") | `data/scripts/room-034-high-stre/obj-0438-door.txt [0050]` loadRoomWithEgo(857,78,145,142) | state(438)==1 (`[0044]` to `[0049]`). Open: (2, 438), `[0018]` → script 25 [438] (no partner). |
| T19 | 34M → 34T | (11, 435 "town") | intra-room cutscene walk to (397,108) with `RoomScroll(528,1121)`, `data/scripts/room-034-high-stre/obj-0435-town.txt [000C]` to `[003F]` | none |
| T20 | 34T → 34M | (11, 436 "archway") | intra-room cutscene walk to (229,98) with `RoomScroll(160,160)`, `data/scripts/room-034-high-stre/obj-0436-archway.txt [001E]` to `[003C]` | Phase A only. In phase B (`Var[196] ≥ 3`, `[0011]`) it runs local-204 instead: a "reservations" dialogue that always ends with the ego walked back to (479,111) on the town side (`data/scripts/room-034-high-stre/local-204.txt [09A6]`). **34M, 36 and 53 are unreachable in phase B** (inferred: 436 is the only way into 34M other than coming back from 36). |
| T21 | 32 → 34T | (11, 422 "street") | `data/scripts/room-032-alley/obj-0422-street.txt [000C]` loadRoomWithEgo(432,34) | none. The first visit to 32 plays a Fester cutscene and does not move you (`data/scripts/room-032-alley/local-200.txt`, started at `data/scripts/room-032-alley/entry.txt [000A]` if `!Bit[481]`). |
| T22 | 31 → 34T | (11, 400 "doorway") any; alias (any, 421) → `startObject(400,…)` (`data/scripts/room-031-jail/obj-0421-unnamed.txt [0010]`) | `data/scripts/room-031-jail/obj-0400-doorway.txt [0037]` loadRoomWithEgo(434,34) | none. Phase B with any crew flag (`Bit[88]+Bit[89]+Bit[76] > 0`) and `!Bit[447]`: plays the "Meanwhile" cutscene, global script 121 (`[0010]` to `[002E]`), which ends at the same destination (`data/scripts/global/script-121.txt [0613]` loadRoomWithEgo(Local[0]=434, Local[1]=34)). |
| T23 | 30 → 34T | (11, 387 "door") | `data/scripts/room-030-store/obj-0387-door.txt [008C]` to `[0098]` → `data/scripts/room-030-store/local-204.txt [0044]` loadRoomWithEgo(437,34) | state(387)==1 (`data/scripts/room-030-store/local-204.txt [0038]`). You must not carry an unpaid sword (388 owned and `!Bit[98]`) or unpaid shovel (396 owned and `!Bit[99]`) (`[0005]` to `[0031]`). Otherwise the storekeeper catches you and you stay in 30 (`[004E]` to `[042B]`). |
| T34 | 78 → 34T | (any, 857 "exit") or (any, 858 "exit") | `data/scripts/room-078-church/obj-0857-exit.txt [0010]` / `data/scripts/room-078-church/obj-0858-exit.txt [0010]` loadRoomWithEgo(438,34) | none |
| T30 | 36 → 34M | (11, 466 "trail") | `data/scripts/room-036-mansion-e/obj-0466-trail.txt [000C]` loadRoomWithEgo(431,34) | none |
| T31 | 36 → 53 | (11, 465 "door") | `data/scripts/room-036-mansion-e/obj-0465-door.txt [003C]` loadRoomWithEgo(633,53) | state(465)==1 (`[0030]` to `[0035]`). While `!Bit[15]` (poodles awake), the entry script locks boxes 1 to 6 and makes 465 untouchable (`data/scripts/room-036-mansion-e/entry.txt [0007]`, `[002B]` to `[003F]`). `Bit[15]` is set when the poodles fall asleep (`data/scripts/room-036-mansion-e/local-201.txt [0087]`; same script unlocks the boxes `[00BA]` to `[00C7]` and the door `[00CE]`). Open: (2, 465), `data/scripts/room-036-mansion-e/obj-0465-door.txt [0018]` → script 25 [465,633]. |
| T32 | 53 → 36 | (11, 633 "door") | `data/scripts/room-053-foyer/obj-0633-door.txt [0068]` loadRoomWithEgo(465,36) | state(633)==1 (`[005C]` to `[0061]`). Open: (2, 633) → script 25 [633,465] (`[002A]`), **unless the idol 635 is owned** (`[0018]` to `[0024]`), which runs T33 instead. |
| T33 | 53 → 83 → 42 | (2, 633) while owning 635 (auto chain) | `data/scripts/room-053-foyer/local-217.txt [038C]` `Var[277]=1`, `[0391]` loadRoomWithEgo(904,83). Then `data/scripts/room-083-cu-dock/entry.txt [0016]` to `[001D]` starts global script 65, which does `data/scripts/global/script-065.txt [023A]` putActorInRoom(ego,42) and `[025C]` actorFollowCamera. | own 635. Fester throws you in the sea. 23 (cu-gov) is a cutscene round trip inside 53 (global script 119). |

### 2.4 Bar (28) and kitchen (41)

The bar is 640 px wide. Local script 201 pins the camera to x=160 while the ego is left of x=320, and to x=480 otherwise (`data/scripts/room-028-bar/local-201.txt [0000]` to `[0016]`). The front door 315 (x 32-72) is only on screen in the left half, and the kitchen door 316 (x 592-624) only in the right half (derived, CDHD).

The curtain 323 (x 304-344) is visible from both halves. It walks you across: to (330,137) if you are left of 320, else to (310,137) (`data/scripts/room-028-bar/obj-0323-curtain.txt [000C]` to `[0022]`). Object 320 is an alias for it (`data/scripts/room-028-bar/obj-0320-unnamed.txt [000F]`).

| T | from → to | sentence | room change | preconditions / notes |
|---|---|---|---|---|
| T25 | 28L → 33 | (11, 315 "door") | `data/scripts/room-028-bar/obj-0315-door.txt [0090]` loadRoomWithEgo(428,33) | state(315)==1 (`[0072]` to `[0077]`) and `Bit[446]` already set. Open: (2, 315), `[0018]` to `[0034]` → script 25 [315,428]. |
| T26 | 28L → (70, 72) → 33 | same sentence, **first time only** | `data/scripts/room-028-bar/obj-0315-door.txt [0080]` to `[008A]`: `Bit[446]=1`, start global script 120 (the LeChuck "Meanwhile" cutscene in rooms 70/72), which ends with `data/scripts/global/script-120.txt [0538]` loadRoomWithEgo(428,33) | none (phase A). Same endpoint as T25, but with a long cutscene in between. |
| T25a | 28R → 33 | (11, 315 "door") from the right half | Same door script as T25/T26: state check `data/scripts/room-028-bar/obj-0315-door.txt [0072]` to `[0077]`; first exit `[0080]` to `[008A]` → `data/scripts/global/script-120.txt [0538]`; later exits `[0090]` loadRoomWithEgo(428,33) | state(315)==1. 315 is off screen from 28R, but the sentence walks ego left, and local-201 pans the camera to the left half once ego is left of x 320 (`data/scripts/room-028-bar/local-201.txt [0000]` to `[0016]`), so 315 comes on screen. That is the case the off-screen rule allows (`rules/glitchless.md`, "Camera visibility"). It is one sentence instead of the curtain (T28) plus T25. |
| T27 | 28R → 41 | (11, 316 "door") | `data/scripts/room-028-bar/obj-0316-door.txt [003F]` to `[004B]` → `data/scripts/room-028-bar/local-218.txt [0017]` loadRoomWithEgo(570,41) | state(316)==1. Open: (2, 316), `data/scripts/room-028-bar/obj-0316-door.txt [0018]` to `[0027]` → script 25 [316,570]. **Phase A guard (the cook):**<ul><li>While the cook is in the kitchen (local-211 running), Open gives the cook's warning (`local-214`'s message, two parts joined by `wait()`) and the door stays shut (`[0018]` to `[0021]` → `data/scripts/room-028-bar/local-214.txt`).</li><li>The room's input script also blocks a *click* on 316 while the cook stands at x > 310 (`data/scripts/room-028-bar/local-203.txt [0012]` to `[002B]` → local-215).</li><li>A usable window exists only while the cook is in the bar at x ≤ 310 (`data/scripts/room-028-bar/local-203.txt [002D]` to `[0035]`).</li><li>The cook comes out after 30 to 50 s (`data/scripts/room-028-bar/local-211.txt [000D]`) and goes back in (`data/scripts/room-028-bar/local-216.txt [0065]` to `[0085]`).</li></ul>Each bar entry other than from 41 resets 316 to closed (`data/scripts/room-028-bar/local-205.txt [0040]`). In phase B none of this applies (`[000A]`; `data/scripts/room-028-bar/local-203.txt [0002]`). |
| T28 | 28L ↔ 28R | (11, 323 "curtain") or (11, 320) | intra-room walk (`data/scripts/room-028-bar/obj-0323-curtain.txt [000C]` to `[0022]`) | none |
| T29 | 41 → 28R | (11, 570 "door") | `data/scripts/room-041-kitchen/obj-0570-door.txt [003C]` loadRoomWithEgo(316,28) | state(570)==1 (`[0030]` to `[0035]`). It is open on arrival, because opening 316 also opens 570 (script 25 partner). |

Kitchen door 564 opens the pier part of room 41 (`data/scripts/room-041-kitchen/obj-0564-door.txt [0018]` to `[003D]`). That is intra-room, not an exit.

Bar close-ups 79/81/82 are cutscene round trips (§1).

### 2.5 Island map (85)

**How you get onto the map** (all verified):

- From the lookout via path 487 (T04).
- From the crossing path 599 (T50), the forest entrance 687 in pseudo-room 218 (§2.9), Sword Master path 743 (T66), treasure-site path 750 (T67), circus path 622 (T62), Stan's path 698 (T60), Smirk's paths 592/594 (T59), and the bridge paths 653/654 (T48/T49).
- **Not** from the dock or town directly. You go dock → cliffside 426 → lookout 38 → path 487.

**How the map handles input** (verified):

- The map's input script is local-201 (`data/scripts/room-085-melee/entry.txt [0055]`).
- Every click except one on 914 goes to global script 33. That script ignores untouchable objects and turns any click on an object into `doSentence(11, obj, 0)`, whatever verb is selected (`data/scripts/global/script-033.txt [000D]` to `[004F]`).
- So the only sentences the game itself ever pushes on the map are "Walk to <map object>". Use verb 11 for every map row.
- The ego walks the tiny map sprite (costume 3, `data/scripts/room-085-melee/entry.txt [005A]`) to the object's walk point, then the object script changes room.

**East/west split (derived plus verified).**

- The map's walk boxes form one connected network of line segments (derived, BOXD/BOXM). Box 34 is the only segment joining the western part (boxes 1 to 33) to the eastern part (boxes 35 to 43). It runs from (159,123) to (172,136) and passes exactly through (169,133), the walk point of the "bridge" object 914 (derived, CDHD).
- While the troll is unpaid (654 has class 32), the map entry starts local-200 (`data/scripts/room-085-melee/entry.txt [0071]` to `[007A]`). Local-200 stops the sentence and loads the bridge room as soon as the ego comes within 2 px of 914 (`data/scripts/room-085-melee/local-200.txt [0001]` to `[0011]`).
- So before the troll is paid, any walk from west to east ends in room 57 (inferred from box geometry plus local-200).
- East holds 915 (Stan's, walk point (206,147)) and 916 (Smirk's house, (274,107)). Everything else is west.

| T | from → to | sentence | room change | preconditions / notes |
|---|---|---|---|---|
| T39 | 85 → 48N | (11, 910 "shore"); alias (11, 909 "island") → `startObject(910,11)` (`data/scripts/room-085-melee/obj-0909-island.txt [000C]`) | `data/scripts/room-085-melee/obj-0910-shore.txt [000C]` loadRoomWithEgo(599,48,22,134) | none (west) |
| T40 | 85 → F218 | (11, 911 "fork") | `data/scripts/room-085-melee/obj-0911-fork.txt [000C]` loadRoomWithEgo(687,218) | none (west). 218 is a forest pseudo-room (§2.9). |
| T41 | 85 → 52 | (11, 912 "clearing"; named "circus" once 52 has been visited, `data/scripts/room-085-melee/entry.txt [0023]`) | `data/scripts/room-085-melee/obj-0912-clearing.txt [000C]` loadRoomWithEgo(622,52,430,130) | none (west) |
| T42 | 85 → 38 | (11, 913 "lookout point") | `data/scripts/room-085-melee/obj-0913-lookout-point.txt [000C]` loadRoomWithEgo(487,38,228,141) | none (west) |
| T43 | 85 → 57 | (11, 914 "bridge") | `data/scripts/room-085-melee/obj-0914-bridge.txt [0018]` loadRoomWithEgo(653,57,78,136) if ego x < 133, else `[0023]` loadRoomWithEgo(654,57) | **Troll unpaid:** local-200 fires on approach and always enters at 653, the west end (`data/scripts/room-085-melee/local-200.txt [0011]`). **Troll paid:** the x test runs *after* the walk to (169,133), so x ≥ 133 and it enters at 654, the east end (inferred). The human click path walks to (170,134) first when x ≥ 133 (`data/scripts/room-085-melee/local-201.txt [0007]` to `[002D]`). Same result. |
| T44 | 85 → 59 | (11, 915 "lights"; named "Used Ship Emporium" after 59 has been visited, `data/scripts/room-085-melee/entry.txt [0036]`; `data/scripts/room-059-stans/entry.txt [0000]`) | `data/scripts/room-085-melee/obj-0915-lights.txt [000C]` loadRoomWithEgo(698,59,53,121) | **troll paid** (east, see above) |
| T45 | 85 → 43 | (11, 916 "house") | `data/scripts/room-085-melee/obj-0916-house.txt [000C]` loadRoomWithEgo(592,43) | **troll paid** (east) |
| T46 | 85 → 33 | (11, 917 "village") | `data/scripts/room-085-melee/obj-0917-village.txt [0046]` loadRoomWithEgo(426,33,307,133) | Phase B: →83 redirect (`[000C]` to `[003E]`). |
| T47 | 85 → 61 | (11, 918 "Sword Master's") | `data/scripts/room-085-melee/obj-0918-sword-master-s.txt [0041]` loadRoomWithEgo(743,61,13,125). First it unlocks trail boxes 44 to 46 and walks to (92,51) (`[0013]` to `[002D]`). | 918 starts untouchable (class 32, derived DOBJ). The game cannot click it (`data/scripts/global/script-033.txt [0014]`) until 61 has been visited once: `data/scripts/room-061-sword-mas/entry.txt [0002]` setClass(918,[32]). So the **first** visit to 61 must go through the forest (F209). |

Map rows 909 to 918 are all west except 915 and 916. Returning to the map from 61 or 64 replays a short walk down the Sword Master trail (`data/scripts/room-085-melee/entry.txt [00C3]` to `[00F4]`, local-203).

**Random encounters.**

- In phase A, from the 4th map entry on (`Var[290] > 2`, incremented at `data/scripts/room-085-melee/entry.txt [00C0]`), the map starts 1 wandering pirate. It starts 3 once Smirk has trained you (`Bit[19]`) (`[009B]` to `[00BA]`).
- If a pirate meets a *stopped* ego, global script 114 runs an insult duel in room 49 and returns to 85 (`data/scripts/room-085-melee/local-202.txt [018A]` to `[01A2]`).
- See §8 Q2.

### 2.6 Bridge (57)

| T | from → to | sentence | room change | preconditions / notes |
|---|---|---|---|---|
| T48 | 57 → 85 | (11, 653 "path") | `data/scripts/room-057-bridge/obj-0653-path.txt [000C]` to `[0017]` putActorInRoom(ego,85), putActor(166,131), actorFollowCamera | none. This lands on the map at the bridge junction, west of the trigger point (inferred). |
| T49 | 57 → 85 | (11, 654 "path") | `data/scripts/room-057-bridge/obj-0654-path.txt [000C]` to `[0017]` putActorInRoom(ego,85), putActor(171,135), actorFollowCamera | **troll paid**: 654 touchable and boxes 2 and 3 unlocked. While 654 is untouchable, the entry locks boxes 2 and 3 (`data/scripts/room-057-bridge/entry.txt [000D]` to `[001A]`). |
| T49a | 57 → 85 | (4, 568 "red herring" → 655 "troll") (auto exit) | Troll's Give hook `data/scripts/room-057-bridge/obj-0655-troll.txt [004C]` → `data/scripts/room-057-bridge/local-204.txt [0002]` (item 568). It unlocks boxes `[0052]` to `[0056]`, makes 654 touchable `[005A]`, walks the ego to 654 and runs `startObject(654,11)` `[0097]`, which is T49. | own 568. Paying is itself a room change to the map. |

While the troll is unpaid, the room also swaps in its own sentence script 202 (`data/scripts/room-057-bridge/entry.txt [0026]`) and its own input script 201 (`data/scripts/room-057-bridge/entry.txt [0021]`).

- 202 rewrites "Use X with troll" into "Give" (`data/scripts/room-057-bridge/local-202.txt [0000]` to `[002B]`). It runs for injected sentences too.
- Input script 201 blocks clicks past the troll (`data/scripts/room-057-bridge/local-201.txt [000B]` to `[004D]`).

### 2.7 Crossing (48) and Meathook's house (37)

**Room layout.**

- Room 48 has a near side (boxes 1 to 6, path 599, pole 601) and a far side (boxes 8 and 9, door 598, pole 600, house 607).
- The pole tops are boxes 7 (pole 601) and 10 (pole 600).
- On entry, the far-side boxes are locked (`data/scripts/room-048-crossing/entry.txt [0003]` to `[001B]`). Entry also sets the near-side or far-side touchability, depending on the previous room: local-201 for far after coming from 37, local-202 for near otherwise (`[0049]` to `[0061]`; `data/scripts/room-048-crossing/local-201.txt`, `local-202.txt`).

**Nodes:** 48N (near ground), 48NT (near pole top), 48FT (far pole top), 48F (far ground).

| T | from → to | sentence | room change | preconditions / notes |
|---|---|---|---|---|
| T50 | 48N → 85 | (11, 599 "path") any | `data/scripts/room-048-crossing/obj-0599-path.txt [0037]` loadRoomWithEgo(910,85) | none. Phase B, first exit after any crew flag: global script 121 cutscene first (`[0010]` to `[002E]`), same endpoint (`data/scripts/global/script-121.txt [0613]`). |
| T51 | 48N → 48NT | (11, 601 "pole") | intra-room climb, `data/scripts/room-048-crossing/obj-0601-pole.txt [004F]` to `[0064]` → local-204 | none |
| T52 | 48NT → 48N | (11, 601) | `data/scripts/room-048-crossing/obj-0601-pole.txt [004F]` to `[005B]` (box 7) → local-205, climb down | none |
| T53 | 48N or 48NT → 48FT | (7, 377 "rubber chicken", 603 "cable"). Aliases 604/605/606 forward to 603 (`data/scripts/room-048-crossing/obj-0604-cable.txt [0014]`; `data/scripts/room-048-crossing/obj-0605-cable.txt [0010]`). | intra-room zip line:<ol><li>377's Use re-queues as (7, 603, 377) (`data/scripts/room-029-fortune/obj-0377-chicken.txt [00CE]` to `[00DA]`).</li><li>`data/scripts/room-048-crossing/obj-0603-cable.txt [0010]` to `[001E]` runs local-207.</li><li>Local-207 climbs pole 601 by itself if you are on the ground (`data/scripts/room-048-crossing/local-207.txt [0000]` to `[0012]`).</li><li>Then local-203 zips you across from box 7 (`data/scripts/room-048-crossing/local-203.txt [000A]` to `[0044]`).</li></ol> | own 377. Ends on top of far pole 600 with input script 206 active (`data/scripts/room-048-crossing/local-203.txt [0108]`). |
| T54 | 48FT → 48F | (11, 600 "pole") | `data/scripts/room-048-crossing/obj-0600-pole.txt [003F]` to `[004B]` → local-205, climb down | none. **This step must be explicit.** On a pole top, input script 206 turns any click elsewhere into a climb-down first (`data/scripts/room-048-crossing/local-206.txt [0000]` to `[007A]`). An injected "Walk to 598" skips that. |
| T55 | 48F → 48FT | (11, 600) | `data/scripts/room-048-crossing/obj-0600-pole.txt [0054]` → local-204, climb up | none |
| T56 | 48F or 48FT → 48NT | (7, 377, 603) | local-207 climbs 600 if on far ground (`data/scripts/room-048-crossing/local-207.txt [0017]` to `[0035]`), then local-203 zips back from box 10 (`data/scripts/room-048-crossing/local-203.txt [004A]` to `[0083]`) | own 377 |
| T57 | 48F → 37 | (11, 598 "door") | `data/scripts/room-048-crossing/obj-0598-door.txt [004F]` loadRoomWithEgo(469,37) | state(598)==1 (`[0043]` to `[0048]`). Open: (2, 598), `[002F]` → script 25 [598,469]. Alias: house 607 forwards any verb to 598 (`data/scripts/room-048-crossing/obj-0607-house.txt [0036]`). |
| T58 | 37 → 48F | (11, 469 "door") | `data/scripts/room-037-meats-hou/obj-0469-door.txt [0035]` loadRoomWithEgo(598,48) | state(469)==1 (`[0029]` to `[002E]`). Open: (2, 469), `[0015]`. On arrival in 48, the crossing closes 598/469 behind you (`data/scripts/room-048-crossing/entry.txt [0055]`). |
| T58a | 37 → 48F | (auto) | Meathook's entry dialogue (global script 60, started by `data/scripts/room-037-meats-hou/local-203.txt [0028]`) can end by evicting you: `data/scripts/global/script-060.txt [17B5]`, `[1801]`. Also `data/scripts/room-037-meats-hou/local-205.txt [04C9]` (after Meathook joins, `Bit[88]` `[04B9]`) and `data/scripts/room-037-meats-hou/local-207.txt [017F]`. | dialogue-dependent. The dialogue analyst owns the details. |

The cutscene round trip 37 → 50 → 37 is global script 49 (§1).

### 2.8 Story cutscenes through the dock close-up (83), underwater (42), and the end of Part I

The entry of 83 dispatches on `Var[277]` (`data/scripts/room-083-cu-dock/entry.txt [0016]` to `[00A1]`):

| `Var[277]` | set by | runs | outcome |
|---|---|---|---|
| 1 | idol escape, `data/scripts/room-053-foyer/local-217.txt [038C]` | global script 65 | ego → 42 (T33) |
| 2 | phase-B redirect, first time (`!Bit[449]`) | local-203 | Kidnapping scene: ghost ship leaves (local-202 sets `Bit[449]`, `data/scripts/room-083-cu-dock/local-202.txt [0000]`), then a lookout dialogue. Sets `Bit[304]` "Governor kidnapped" (`data/scripts/room-083-cu-dock/local-203.txt [0736]`). Control stays in 83 (`[074E]`). |
| 3 | phase-B redirect when crew and ship are ready (`Bit[88] & Bit[89] & Bit[76] & Bit[51]`) | local-200 | **End of Part I**: ego → room 19 (T38) |
| 4 | underwater exit while `Var[196] < 3` | local-201 | Elaine rescue scene, control stays in 83 (`data/scripts/room-083-cu-dock/local-201.txt [07A0]`) |
| 5 | underwater exit when the idol was the 3rd trial | local-203 | as 2, with an extra line |

**Phase-B redirect (§2 shorthand "→83 redirect").** Stairs 486, map village 917 and low-street archway 450 each test, before their normal destination:

- If `Var[196] ≥ 3` and `Bit[88] & Bit[89] & Bit[76] & Bit[51]`: set `Var[277]=3` and enter 83.
- Else, if `Var[196] ≥ 3` and `!Bit[449]`: set `Var[277]=2` and enter 83.

Cites:

- `data/scripts/room-038-lookout/obj-0486-stairs.txt [0010]` to `[0042]`
- `data/scripts/room-085-melee/obj-0917-village.txt [000C]` to `[003E]`
- `data/scripts/room-035-low-stree/obj-0450-archway.txt [0057]` to `[0089]`

The bits (from where they are set or tested):

- `Bit[88]` is set when Meathook joins (`data/scripts/room-037-meats-hou/local-205.txt [04B9]`).
- `Bit[89]` makes the Sword Master absent (`data/scripts/room-061-sword-mas/entry.txt [0054]`).
- `Bit[76]` makes the prisoner absent (`data/scripts/room-031-jail/entry.txt [0012]`).
- `Bit[51]` means Stan has sold the ship (`data/scripts/room-059-stans/obj-0731-sign.txt [0025]`).

The crew analyst should confirm these meanings.

| T | from → to | sentence | room change | preconditions / notes |
|---|---|---|---|---|
| T05 | 38 / 85 / 35 → 83 (`Var[277]=2`) | same sentence as T03 / T46 / T11 | `data/scripts/room-038-lookout/obj-0486-stairs.txt [0042]`, `data/scripts/room-085-melee/obj-0917-village.txt [003E]`, `data/scripts/room-035-low-stree/obj-0450-archway.txt [0089]` loadRoomWithEgo(904 or 905, 83) | `Var[196] ≥ 3`, `!Bit[449]`, crew/ship incomplete |
| T06 | 38 / 85 / 35 → 83 (`Var[277]=3`) | same sentence | `data/scripts/room-038-lookout/obj-0486-stairs.txt [0030]`, `data/scripts/room-085-melee/obj-0917-village.txt [002C]`, `data/scripts/room-035-low-stree/obj-0450-archway.txt [0077]` | `Var[196] ≥ 3` and all four bits |
| T35 | 42 → 83 | (11, 577 "ladder") | `data/scripts/room-042-underwate/obj-0577-ladder.txt [000C]` → `data/scripts/room-042-underwate/local-200.txt [0091]` putActorInRoom(ego,83), `[009D]` actorFollowCamera | Script 201 ("tied to this stupid idol") must not be running (`[0000]` to `[0009]`). Do (9, 578 "fabulous idol") Pick up first: `data/scripts/room-042-underwate/obj-0578-fabulous-idol.txt [001B]` to `[0027]` → `data/scripts/room-042-underwate/local-203.txt [0016]` stopScript(201). This exit completes trial 2 (`data/scripts/room-042-underwate/local-200.txt [0041]`) and sets `Var[277]` to 4 or 5 (`[0076]` to `[0085]`). |
| T36 | 83 → 33 | (any, 904 "dock") | `data/scripts/room-083-cu-dock/obj-0904-dock.txt [0015]` to `[0020]` putActorInRoom(ego,33), putActor(308,132), actorFollowCamera | `!Bit[453]` (`[0010]`) |
| T37 | 83 → 33 | (any, 905 "dock") | `data/scripts/room-083-cu-dock/obj-0905-dock.txt [0015]` to `[0020]` putActorInRoom(ego,33), putActor(566,132), actorFollowCamera | `!Bit[453]` (`[0010]`) |
| T38 | 83 → 19 | (auto, `Var[277]=3`) | `data/scripts/room-083-cu-dock/local-200.txt [0C56]` putActorInRoom(ego,19). Then `[0C74]` starts global script 122 (Part II). | end of Part I |

The dock entry runs a short camera restriction when you come from 83 (`data/scripts/room-033-dock/entry.txt [0032]` to `[0039]`, local-201). It does not move you.

### 2.9 Forest maze (58, pseudo-rooms 201-220)

**How the maze works.**

- All 20 pseudo-rooms use the physical room 58. Its entry script (`data/scripts/room-058-damnfores/entry.txt`):
  1. Moves every path object off screen and resets its state (`data/scripts/room-058-damnfores/entry.txt [003C]` to `[01B8]`). An undrawn path keeps its walk point off screen, so walking to it fails with "I can't reach it" (inferred from `o5_drawObject`, which shifts the walk point with the image).
  2. Makes the four path objects 685/686/687/688 touchable (`data/scripts/room-058-damnfores/entry.txt [01BC]` to `[01D1]`).
  3. Redraws a different subset for each `VAR_ROOM` (`data/scripts/room-058-damnfores/entry.txt [01D8]` to `[0A2C]`).
- Every path has an `FF` entry that forwards to its verb 11 code. 686 always forwards to 685 (`data/scripts/room-058-damnfores/obj-0686-path.txt [0010]`).
- Destinations are per pseudo-room switch tables in:
  - `data/scripts/room-058-damnfores/obj-0685-path.txt [0012]` to `[0197]`
  - `data/scripts/room-058-damnfores/obj-0687-path.txt [0012]` to `[0144]`
  - `data/scripts/room-058-damnfores/obj-0688-path.txt [0012]` to `[013C]`

**How to read the table below.**

- Only paths that the entry script draws in that pseudo-room are listed.
- The sentence is always `(11, <path>)`.
- Each destination comes from the switch case `VAR_ROOM == <from>` in that path's file. The arrival object is in brackets.

| from | drawn paths (offsets in `data/scripts/room-058-damnfores/entry.txt`) | exits |
|---|---|---|
| F201 | 688, 687, 685 (`[01DF]` to `[01EF]`) | 685 → **64 treasure** (`data/scripts/room-058-damnfores/obj-0685-path.txt [0019]`); 687 → F216 [688] (`data/scripts/room-058-damnfores/obj-0687-path.txt [0019]`); 688 → F206 [687] (`data/scripts/room-058-damnfores/obj-0688-path.txt [0019]`) |
| F202 | 687, 688 (`[0245]` to `[024D]`) | 687 → F205 [685]; 688 → F203 [687] |
| F203 | 685, 687 (`[02A7]` to `[02AF]`) | 685 → F215 [685]; 687 → F202 [688] |
| F204 | 688, 685 (`[0305]` to `[030D]`) | 685 → F211 [688]; 688 → F212 [687] |
| F205 | 685, 687, 688 (`[036F]` to `[037F]`) | 685 → F202 [687]; 687 → F213 [685]; 688 → F217 [688] |
| F206 | 687 (`[03D1]`) | 687 → F201 [688] |
| F207 | 687, 688, 685 (`[0423]` to `[0433]`) | **Split by previous room:**<ul><li>If you came from F212 (`Var[101]==212`), 685 and 688 are made untouchable (`[0473]` to `[0481]`), so only 687 → F212 [685] works. Call this node **F207a**.</li><li>Otherwise 687 is untouchable (`[048F]`) and 685 → F211 [685], 688 → F219 [685]. Call this node **F207b**.</li></ul> |
| F208 | 688, 687 (`[04A8]` to `[04B0]`) | 687 → F214 [685]; 688 → F219 [687] |
| F209 | 688, 687, sign 681 (`[0517]` to `[0557]`) | 688 → F217 [685]. 687 → **61 Sword Master** (`data/scripts/room-058-damnfores/obj-0687-path.txt [0097]` loadRoomWithEgo(743,61,13,125)), **only if `Bit[546]`**. Without it, 687 is untouchable, the ravine object 656 is drawn and box 3 stays locked (`data/scripts/room-058-damnfores/entry.txt [055F]` to `[0581]`). Pushing, pulling or using sign 681 toggles `Bit[546]` and the ravine (`data/scripts/room-058-damnfores/obj-0681-sign.txt [001E]` → `data/scripts/room-058-damnfores/local-203.txt [0007]` to `[0024]` sets it; `data/scripts/room-058-damnfores/local-204.txt [0012]` clears it). Any of (5/6/7, 681) works. |
| F210 | 688, 687 (`[059B]` to `[05A3]`) | 687 → F220 [685]; 688 → F214 [687] |
| F211 | 688, 687, 685 (`[05F9]` to `[0609]`) | 685 → F207b [685]; 687 → F216 [685]; 688 → F204 [685] |
| F212 | 688, 687, 685 (`[0667]` to `[0677]`) | 685 → F207a [687]; 687 → F204 [688]; 688 → F213 [688] |
| F213 | 688, 687, 685 (`[06CD]` to `[06DD]`) | 685 → F205 [687]; 687 → F220 [687]; 688 → F212 [688] |
| F214 | 685, 687 (`[0737]` to `[073F]`) | 685 → F208 [687]; 687 → F210 [688] |
| F215 | 688, 687, 685 (`[0789]` to `[0799]`) | 687 → F218 [685]. 685 → F203 [685] **gate A**. 688 → F220 [688] **gate B**. |
| F216 | 685, 688 (`[0829]` to `[0831]`) | 685 → F211 [687]; 688 → F201 [687] |
| F217 | 685, 688 (`[0883]` to `[088B]`) | 685 → F209 [688]; 688 → F205 [688] |
| F218 | 685, 686, 687 (`[08E5]` to `[08F5]`) | 685 / 686 → F215 [687]; 687 → **85 map** (`data/scripts/room-058-damnfores/obj-0687-path.txt [0115]` loadRoomWithEgo(911,85)) |
| F219 | 685, 687 (`[0953]` to `[095B]`) | 685 → F207b [688]; 687 → F208 [688] |
| F220 | 685, 686, 687, 688 (`[09B9]` to `[09D1]`) | 685 / 686 → F210 [687]; 687 → F213 [687]; 688 → F215 [688] |

**The F215 gates.**

- **Gate A** (path 685) passes if any one of these holds:
  - you own the map 442 (bought in room 35);
  - global script 67 (following the storekeeper) is running;
  - `Bit[401]` is set.

  Otherwise Guybrush refuses ("not going into this mazelike forest") and nothing happens (`data/scripts/room-058-damnfores/obj-0685-path.txt [004F]` to `[00BE]`).
- **Gate B** (path 688) passes if you own 442 or `Bit[401]` is set. Otherwise local-200 prints the same refusal (`data/scripts/room-058-damnfores/obj-0688-path.txt [004F]` to `[0063]`).
- Passing either gate sets `Bit[401]` for good (`data/scripts/room-058-damnfores/obj-0685-path.txt [00C1]`, `data/scripts/room-058-damnfores/obj-0688-path.txt [0066]`). After one successful entry, the forest is always open.

**The storekeeper's route** to the Sword Master (global script 67, cases `data/scripts/global/script-067.txt [00A3]` to `[0245]`) is the shortest map→61 path:

85 →(911) F218 →(685) F215 →(685) F203 →(687) F202 →(687) F205 →(688) F217 →(685) F209 → push sign 681 →(687) 61.

**Shortest map→64 route** (inferred from the table, 9 path clicks):

85 →(911) F218 →(685) F215 →(688) F220 →(687) F213 →(688) F212 →(687) F204 →(685) F211 →(687) F216 →(688) F201 →(685) 64.

### 2.10 Remaining map destinations

| T | from → to | sentence | room change | preconditions / notes |
|---|---|---|---|---|
| T59 | 43 → 85 | (11, 592 "path") or (11, 594 "path") | `data/scripts/room-043-trainers/obj-0592-path.txt [000C]`, `data/scripts/room-043-trainers/obj-0594-path.txt [000C]` loadRoomWithEgo(916,85) | none. Door 591 is not an exit: Open runs global script 57 (knock, then the Smirk dialogue and training), with the cutscene round trip 43 → 60/76 → 43 (`data/scripts/room-043-trainers/obj-0591-door.txt [0012]`). |
| T60 | 59 → 85 | (11, 698 "path") any | `data/scripts/room-059-stans/obj-0698-path.txt [0010]` loadRoomWithEgo(915,85) | none. 699 and 700 move you between the levels of the lot inside room 59 (`data/scripts/room-059-stans/local-203.txt [000B]`, `data/scripts/room-059-stans/local-204.txt [000B]`). They are not exits. |
| T60a | 59 → 85 | (auto) | Stan's dialogue can end by sending you to the map: `data/scripts/global/script-056.txt [2522]` loadRoomWithEgo(915,85) | dialogue-dependent. Stan himself appears only in phase B, before the ship is sold (`data/scripts/room-059-stans/entry.txt [0023]` to `[003A]`). |
| T62 | 52 → 85 | (11, 622 "path") | `data/scripts/room-052-circus-gr/obj-0622-path.txt [000C]` loadRoomWithEgo(912,85) | none |
| T63 | 52 → 51 | (11, 621 "circus tent") | `data/scripts/room-052-circus-gr/obj-0621-circus-tent.txt [0014]` **loadRoom(51)**. The tent's entry script places the ego itself (`data/scripts/room-051-circus-te/entry.txt [002A]` to `[003F]`) and starts the Fettucini dialogue (`[004A]`). | `!Bit[103]` (`data/scripts/room-052-circus-gr/obj-0621-circus-tent.txt [000F]`). `Bit[103]` is set once the pot-helmet is accepted (`data/scripts/room-051-circus-te/local-200.txt [008B]`, `data/scripts/room-051-circus-te/local-210.txt [001E]`). After that the tent is closed for good. |
| T64 | 51 → 52 | (11, 617 "outside"); aliases 618/619/620 forward any verb (`data/scripts/room-051-circus-te/obj-0618-outside.txt [000C]`) | `data/scripts/room-051-circus-te/obj-0617-outside.txt [000C]` loadRoomWithEgo(621,52,78,87) | none |
| T65 | 51 → 52 | (auto) | After the cannon stunt: `data/scripts/room-051-circus-te/local-207.txt [114D]` startObject(617,11), which is T64 | follows `Bit[103]` (`[0D5C]`) |
| T66 | 61 → 85 | (11, 743 "path") any | `data/scripts/room-061-sword-mas/obj-0743-path.txt [0037]` loadRoomWithEgo(911,85) | none. Phase B, first exit after any crew flag: global script 121 first (`[0010]` to `[002E]`), same endpoint. |
| T67 | 64 → 85 | (11, 750 "forest path") | `data/scripts/room-064-treasure/obj-0750-forest-path.txt [000C]` loadRoomWithEgo(911,85) | none. One-way: 64 is entered only from F201, and its exit goes straight to the map. |

---

## 3. Exits with no object, and other automatic room changes

**Method (verified).** The search covered every `loadRoomWithEgo`, `loadRoom` and `putActorInRoom(VAR_EGO, …)` + `actorFollowCamera` call site in the Part I rooms and in the global scripts they start. Each one sits in one of these places:

- an object verb script (the sentences in §2);
- a cutscene or dialogue script (the "(auto)" rows);
- the single position-triggered case below.

The other local scripts that poll the ego's position do not change rooms. They only scroll the camera, rename objects or move NPCs:

- `data/scripts/room-033-dock/local-201.txt`
- `data/scripts/room-052-circus-gr/local-201.txt`
- `data/scripts/room-028-bar/local-201.txt`
- `data/scripts/room-028-bar/local-217.txt`
- `data/scripts/room-083-cu-dock/local-205.txt` (Part IV only)

**Exits with no object:**

| room | trigger | where | object alternative |
|---|---|---|---|
| 85 map | Ego within 2 px (Chebyshev) of 914's walk point (169,133) while 654 is untouchable (troll unpaid). It does `doSentence(STOP)` and then loadRoomWithEgo(653,57,78,136). | `data/scripts/room-085-melee/local-200.txt [0001]` to `[0011]`, started by `data/scripts/room-085-melee/entry.txt [0071]` | (11, 914 "bridge") gives the same result. You never need to trigger it on purpose. It matters because it **blocks** walking east on the map (§2.5). |

There are no edge-of-screen exits. Exits that sit on a screen edge are real objects with walk points just outside the room (derived, CDHD):

- 427 at x 984-1008 in the dock, walk point (1022,113)
- 653 and 743 at x 0-24/32
- 904/905 and 857/858 at the edges of 83 and 78

**Automatic room changes with no sentence of their own:**

T00 to T02, T05/T06 (their own sentence is the phase-B redirect), T26, T33, T38, T49a, T58a, T60a, T65, and the script-121 "Meanwhile" interludes inside T22/T50/T66.

**Cutscene round trips** that return to the room they started in: 28↔79/81/82, 43↔60/76, 61↔44/62, 53↔23, 85↔49, 37↔50, any room↔63.

---

## 4. Door states

**Common rules (verified).**

- Every door below starts in state 0 (closed). None has class 6 (locked) or class 11 (do not sync partner) at start, except as noted (derived, DOBJ).
- Opening is `(2, door)` → global script 25. It sets the door and its partner to state 1, so you can open from either side (`data/scripts/global/script-025.txt [0024]`, `[002D]` to `[0036]`).
- `(7, door)` Use also works: script 31 turns it into Open when the door is closed (`data/scripts/global/script-031.txt [001F]`).
- **Which side you open does not matter.** The partner is opened too, so after you walk through, the door you arrive at is already open.
- **Nothing closes a door when you walk through it.** The only closing events are the scripts listed in "Closed again by". A planner can treat "door open" as sticky, except for those events.

| door (room) ↔ partner (room) | open sentence | walk-through needs | closed again by (cite) |
|---|---|---|---|
| 428 (33) ↔ 315 (28) | (2, 428) `data/scripts/room-033-dock/obj-0428-door.txt [0015]`, or (2, 315) `data/scripts/room-028-bar/obj-0315-door.txt [0018]` to `[0034]` (bar side plays an animation first) | state 1 (T09, T25) | A dock citizen opens and re-closes it only if it was closed (`data/scripts/room-033-dock/local-200.txt [0009]` to `[002A]`, `[0061]` to `[007D]`). A door you opened stays open. |
| 316 (28) ↔ 570 (41) | (2, 316) `data/scripts/room-028-bar/obj-0316-door.txt [0018]` to `[0027]` (refused while the cook is in the kitchen, phase A); (2, 570) `data/scripts/room-041-kitchen/obj-0570-door.txt [0018]` | state 1 (T27, T29) | Every bar entry not from 41 in phase A sets 316 to 0 directly. This does not touch 570, which stays open (`data/scripts/room-028-bar/local-205.txt [0040]`). The cook opens and closes 316 (with 570) on his rounds (`data/scripts/room-028-bar/local-216.txt [001B]`, `[006C]`, `[0080]`; `data/scripts/room-028-bar/local-215.txt [006A]`, `[007E]`). |
| 444 (35) ↔ 367 (29) | (2, 444) `data/scripts/room-035-low-stree/obj-0444-door.txt [0018]`; (2, 367) `data/scripts/room-029-fortune/obj-0367-door.txt [0018]` to `[0034]` | state 1 (T10, T24) | <ul><li>Entering 29 from 35 closes 367 only (`data/scripts/room-029-fortune/local-200.txt [0017]`).</li><li>Entering 35 from 29 closes both after 1 s (`data/scripts/room-035-low-stree/entry.txt [0106]`).</li><li>Low-street citizens may open and close 444 alone (`data/scripts/room-035-low-stree/local-208.txt [0095]`, `[00A2]`).</li></ul> In practice: open 444 just before entering, and always open 367 to leave. |
| 437 (34) ↔ 387 (30) | (2, 437) `data/scripts/room-034-high-stre/obj-0437-door.txt [0018]`; (2, 387) `data/scripts/room-030-store/obj-0387-door.txt [0032]` to `[004E]` | state 1 (T17, T23) | <ul><li>High-street citizens open and then **always** close it (`data/scripts/room-034-high-stre/local-200.txt [001E]`/`[0053]`, `[00A2]`/`[00CC]`).</li><li>The storekeeper closes it when you ring the bell (`data/scripts/room-030-store/local-207.txt [00FD]`) and when he leaves for the Sword Master (`data/scripts/room-030-store/local-211.txt [1117]` to `[1122]`, 387 only).</li></ul> |
| 438 (34) → church | (2, 438) `data/scripts/room-034-high-stre/obj-0438-door.txt [0018]` (no partner; the church exits are open archways) | state 1 (T18) | High-street citizens (`data/scripts/room-034-high-stre/local-200.txt [0030]`/`[0065]`, `[00B4]`/`[00DE]`) |
| 465 (36) ↔ 633 (53) | (2, 465) `data/scripts/room-036-mansion-e/obj-0465-door.txt [0018]`, possible only after `Bit[15]` (465 untouchable before); (2, 633) `data/scripts/room-053-foyer/obj-0633-door.txt [002A]` (with the idol this is T33 instead) | state 1 (T31, T32) | none found |
| 598 (48) ↔ 469 (37) | (2, 598) `data/scripts/room-048-crossing/obj-0598-door.txt [002F]`; (2, 469) `data/scripts/room-037-meats-hou/obj-0469-door.txt [0015]` | state 1 (T57, T58) | Entering 48 from 37 closes both (`data/scripts/room-048-crossing/entry.txt [0055]`). Meathook's evictions open it for you (`data/scripts/room-037-meats-hou/local-207.txt [0173]`; `data/scripts/global/script-060.txt [179B]`, `[17E8]`). |

**Doors that are not room exits:**

| door (room) | what it does |
|---|---|
| 591 (43) | Smirk's door. Open runs global script 57 (knocks and the training sequence). |
| 737 (60) | Cutscene room only. |
| 564 (41) | Pier door. One-way open: it becomes untouchable and unlocks boxes 4 and 5 (`data/scripts/room-041-kitchen/obj-0564-door.txt [0018]` to `[0023]`). The kitchen entry re-applies this from its state (`data/scripts/room-041-kitchen/entry.txt [001B]` to `[004C]`). |
| 632, 634 (53) | Inner foyer doors used by the idol and sheriff scenes. 634 starts locked (class 6). |
| 478 (37) | Meathook's inner door. Starts with class 6. |
| 445-448 (35) | Intra-room teleport doors, §2.2. |

**Whether door clicks are on screen.**

- Doors 316 and 315 are in different camera halves of the bar (§2.4).
- 465 is on screen in the 320 px room 36.
- 437/438 are on screen from anywhere in 34T.

---

## 5. Graph summary (for `(link a b)` facts)

### 5.1 Notation

- **Nodes:** room numbers, plus these splits:
  - `34T` / `34M` (High Street halves)
  - `28L` / `28R` (bar halves)
  - `48N`, `48NT`, `48FT`, `48F` (crossing)
  - `F201` to `F220`, with `F207a` / `F207b` (forest)
  - `19` is the Part II terminal.
- **Edge form:** `to (verb, objA[, objB]) {precondition}`. `|` lists alternative sentences for the same edge.
- **Precondition names:**
  - `open(d)` means door d has state 1.
  - `troll_paid` means 654 is touchable (`setClass(654,[32])` at `data/scripts/room-057-bridge/local-204.txt [005A]`).
  - `phaseA` means `Var[196] < 3`.
  - `has(o)` means you own o.
  - Any other flags are given by their bit number.
- Every edge was checked against the T rows in §2 and the forest table in §2.9.

### 5.2 Player edges

```
38   : 33 (11,486) {no phase-B redirect pending, see T05/T06} ; 85 (11,487)
33   : 38 (11,426) ; 35 (11,427) ; 28L (11,428) {open(428)}
35   : 29 (11,444) {open(444)} ; 33 (11,450) {no phase-B redirect pending} ; 34T (11,451)
34T  : 32 (11,432) ; 35 (11,433) ; 31 (11,434) ; 30 (11,437) {open(437)} ; 78 (11,438) {open(438)} ; 34M (11,436) {phaseA}
34M  : 36 (11,431) | (11,439) ; 34T (11,435)
32   : 34T (11,422)
31   : 34T (11,400) | (11,421)
30   : 34T (11,387) {open(387), not (has(388) and !Bit98), not (has(396) and !Bit99)}
29   : 35 (11,367) {open(367)}
78   : 34T (11,857) | (11,858)
28L  : 33 (11,315) {open(315)} ; 28R (11,323) | (11,320)
28R  : 41 (11,316) {open(316)} ; 28L (11,323) | (11,320) ; 33 (11,315) {open(315)} (T25a)
41   : 28R (11,570) {open(570)}
36   : 34M (11,466) ; 53 (11,465) {Bit15, open(465)}
53   : 36 (11,633) {open(633)}
85   : 48N (11,910) | (11,909) ; F218 (11,911) ; 52 (11,912) ; 38 (11,913) ; 57 (11,914) ;
       59 (11,915) {troll_paid} ; 43 (11,916) {troll_paid} ; 33 (11,917) {no phase-B redirect pending} ;
       61 (11,918) {918 touchable = 61 visited once}
57   : 85 (11,653) ; 85 (11,654) {troll_paid} ; 85 (4,568,655) {has(568), !troll_paid -> sets troll_paid}
48N  : 85 (11,599) ; 48NT (11,601) ; 48FT (7,377,603) {has(377)}
48NT : 48N (11,601) ; 48FT (7,377,603) {has(377)}
48FT : 48F (11,600) ; 48NT (7,377,603) {has(377)}
48F  : 48FT (11,600) ; 48NT (7,377,603) {has(377)} ; 37 (11,598) {open(598)}
37   : 48F (11,469) {open(469)}
43   : 85 (11,592) | (11,594)
59   : 85 (11,698)
52   : 85 (11,622) ; 51 (11,621) {!Bit103}
51   : 52 (11,617) | (11,618) | (11,619) | (11,620)
61   : 85 (11,743)
64   : 85 (11,750)
42   : 83 (11,577) {idol picked up: (9,578) done first}
83   : 33 (11,904) | (11,905)
F201 : 64 (11,685) ; F216 (11,687) ; F206 (11,688)
F202 : F205 (11,687) ; F203 (11,688)
F203 : F215 (11,685) ; F202 (11,687)
F204 : F211 (11,685) ; F212 (11,688)
F205 : F202 (11,685) ; F213 (11,687) ; F217 (11,688)
F206 : F201 (11,687)
F207a: F212 (11,687)
F207b: F211 (11,685) ; F219 (11,688)
F208 : F214 (11,687) ; F219 (11,688)
F209 : F217 (11,688) ; 61 (11,687) {Bit546, toggled by (5,681)}
F210 : F220 (11,687) ; F214 (11,688)
F211 : F207b (11,685) ; F216 (11,687) ; F204 (11,688)
F212 : F207a (11,685) ; F204 (11,687) ; F213 (11,688)
F213 : F205 (11,685) ; F220 (11,687) ; F212 (11,688)
F214 : F208 (11,685) ; F210 (11,687)
F215 : F218 (11,687) ; F203 (11,685) {has(442) or script67 running or Bit401} ; F220 (11,688) {has(442) or Bit401}
F216 : F211 (11,685) ; F201 (11,688)
F217 : F209 (11,685) ; F205 (11,688)
F218 : F215 (11,685) | (11,686) ; 85 (11,687)
F219 : F207b (11,685) ; F208 (11,687)
F220 : F210 (11,685) | (11,686) ; F213 (11,687) ; F215 (11,688)
```

**Correction: the planning model includes 686 and 905** (Phase 7 blind extraction; `docs/extraction-diff.md` §3; `docs/part1/model.md` §14.8).

The edges above list three `|` alternatives that the model had collapsed into one link each: `(11,686)` at F218 and at F220, and `(11,905)` at 83. A link fact names only its two rooms, so one exit object per pair survived: 685 and 904. The forest links were generated from the switch tables of 685, 687 and 688. 686 has none of its own: it forwards every verb to 685 (`data/scripts/room-058-damnfores/obj-0686-path.txt [0010]`).

The three are modelled as the actions `walk-f218-f215-via-686`, `walk-f220-f210-via-686` and `walk-cu-dock-dock-via-905`. Each makes the same room change as its twin from a different walk point. The two 686 actions also arrive where their twin does, because 686 runs 685's code. 905 lands elsewhere (below). Measured on seeds 1 to 3:

- 686 at F218 takes 162 ticks from the map entrance, against 216 for 685;
- 686 at F220 takes 222 ticks from gate 688, against 306 for 685;
- 905 takes 132 ticks, against 168 for 904.

905 lands ego at x 566, not 308 (T37). The dock's local-201, started on every arrival from 83, then pins the camera to `RoomScroll(712,848)` until ego walks below x 566 or past x 726. After that it frees the camera with `RoomScroll(0,848)` (`data/scripts/room-033-dock/local-201.txt [0042]` to `[0070]`). For the cliffside 426 this means:

- It is off screen on that arrival, unlike after 904, where the pin is `RoomScroll(0,160)` (`[0011]`).
- A Walk to 426 crosses x 566 at once, so the camera rule still allows it (§7).
- The walk to the cliffside is 234 ticks longer from 905's landing.

The guided hop F218 → F215, made while following the storekeeper, can also use 686 (`walk-follow-guide-to-f215-via-686`). At 218 global script 67 walks him to a point and waits only for `VAR_ROOM` (`data/scripts/global/script-067.txt [00D6]`, `[02F6]`, `[0353]`). It measured 162 ticks against 216.

The other `|` alternatives on modelled edges are still collapsed into single links or actions. The scripts make each one identical to its twin:

- 439 (34M → 36) forwards to 431 (`data/scripts/room-034-high-stre/obj-0439-deadly-piranha-poodles.txt [000C]`) and has 431's walk point (82,47). It is untouchable (class 32) until room 36's entry clears it (`data/scripts/room-036-mansion-e/entry.txt [0000]`).
- Any verb on 421 (31 → 34T) runs `startObject(400,Local[1])` (`data/scripts/room-031-jail/obj-0421-unnamed.txt [0010]`), and 421 has 400's walk point (294,116).
- 320 (28L ↔ 28R) forwards to 323 (`data/scripts/room-028-bar/obj-0320-unnamed.txt [000F]`). Both have class 8, for which the sentence script does not walk to the object (`data/scripts/global/script-002.txt [02B4]`).

Each makes its twin's walk and lands the same way, so none can be faster (`docs/part1/model.md` §14.8).

### 5.3 Phase-B and story edges

These replace the plain destination when their condition holds, or have no sentence of their own:

```
38 (11,486) | 85 (11,917) | 35 (11,450)  -> 83 {Var196>=3, !Bit449; Var277=2}            (T05)
38 (11,486) | 85 (11,917) | 35 (11,450)  -> 83 -> 19 {Var196>=3, Bit88&Bit89&Bit76&Bit51} (T06, T38: end of Part I)
53 (2,633) {has(635)}                    -> 83 -> 42  (T33)
28L/28R first exit {!Bit446}             -> (cutscene 70/72) -> 33  (T26; also from 28R, T25a)
31/48N/61 first exit {Bit88+Bit89+Bit76>0, !Bit447} -> (cutscene 70/72) -> same destination
37  -> 48F   (dialogue eviction, T58a)
59  -> 85    (Stan dialogue, T60a)
51  -> 52    (after cannon stunt, T65)
boot -> 38 -> 96 -> 33  (T00-T02)
```

### 5.4 Door actions

Each makes `open(d)` true and needs no room change: (2,428) or (2,315); (2,316) {phaseA: cook out of kitchen, see T27} or (2,570); (2,444) or (2,367); (2,437) or (2,387); (2,438); (2,465) {Bit15} or (2,633) {!has(635)}; (2,598) or (2,469).

### 5.5 One-way links and dead ends

- **64 treasure:** entered only from F201 (§2.9) and left only to the map (T67).
- **42 underwater:** entered only by the idol cutscene chain (T33) and left only by the ladder to 83 (T35).
- **83 dock close-up:** entered only by the phase-B redirects (T05/T06), the idol chain (T33) or the underwater exit (T35). It is left only to the dock (T36/T37) or to Part II (T38).
- **F207a:** a dead end that leads only back to F212 (§2.9).
- **Map and dock:** the map's "village" goes straight to the dock (T46), but the dock reaches the map only through the lookout (T07, then T04).
- **51 circus tent:** closed for good once `Bit[103]` is set (T63).
- **34T→34M:** closed in phase B (T20), which cuts off 36 and 53.
- **Map east (915 Stan's, 916 Smirk's):** reachable only after the troll is paid (§2.5, T49a).
- **61 by map:** the map's Sword Master's object only works after the first visit through the forest (T47).

### 5.6 Counts

- 71 numbered transition rows in §2: T00 to T67 without T61, plus T25a, T49a, T58a and T60a.
- 51 location nodes, plus terminal 19.
- 109 directed player edges in §5.2, counting `|` alternatives once. That is 47 edges from forest nodes (including the exits to 61, 64 and 85) and 62 edges from the other nodes.

---

## 6. Where a pushed sentence differs from a real click

The player pushes sentences straight into the queue, as recommended in `docs/research/engine-bridge.md`, §4. That skips the room's input script (`VAR_VERB_SCRIPT`). These Part I rooms replace the default input script 4. What each replacement does, and what the PDDL or compiler must do about it:

| room | input script | effect on transitions | consequence |
|---|---|---|---|
| 85 | local-201 → global script 33 (`data/scripts/room-085-melee/local-201.txt`; `data/scripts/global/script-033.txt [000D]` to `[004F]`) | Every object click becomes `(11, obj)`. Untouchable objects cannot be clicked. 914 gets a special pre-walk. | Use only `(11, obj)` on the map, and never target 918 before 61 has been visited. |
| 28 (phase A) | local-202 / local-203 (`data/scripts/room-028-bar/local-202.txt [001C]` to `[005B]`, `data/scripts/room-028-bar/local-203.txt [0000]` to `[0055]`) | A click on 316 is blocked while the cook is at x > 310 ("Don't go into the kitchen!", local-215). Clicking freezes the cook when he is at x ≤ 310. Clicking elsewhere restarts him. | An injected (2,316) or (11,316) is *not* blocked by the cook at x > 310. To stay glitchless, gate T27 on "cook in room 28 and x ≤ 310" (a `until` condition on actor 6), not only on "local-211 not running". |
| 57 (troll unpaid) | local-201 (`data/scripts/room-057-bridge/local-201.txt [0000]` to `[0052]`) | Clicks past the troll walk you up and trigger the troll (script 55). | The boxes are locked anyway, so an injected walk east just fails. The sentence-script override local-202 applies to injected sentences as well. |
| 48 (on a pole top) | local-206 (`data/scripts/room-048-crossing/local-206.txt`) | Any click except on the pole or a cable climbs down first. | Use explicit `(11,600)` or `(11,601)` steps (T52/T54) before any other sentence. |
| 51 | local-209 during dialogue, local-200 afterwards (`data/scripts/room-051-circus-te/local-200.txt [0000]` to `[00E7]`) | The pot-as-helmet Give is done **inside the input script** with `actorFromPos`, not as a sentence. Only Walk to 617 to 620 is passed on. | The circus/puzzle analyst must check whether an injected `(4, 567, actor)` reaches the same code. Script 2's Give branch calls `Var[116]` instead (`data/scripts/global/script-002.txt [0127]`). The exit itself (T64) is unaffected. |
| 42 | local-204 (`data/scripts/room-042-underwate/local-204.txt [0000]` to `[0033]`) | Each click sets `Var[164]` (the x the idol-drag script 201 steers towards) and restarts 201. | An injected (9,578) may behave differently while tied to the idol. The underwater analyst should test it. T35 needs the idol picked up first. |
| 36 (after poodles) | local-203 (`data/scripts/room-036-mansion-e/local-203.txt`) | The first click removes the "IMPORTANT NOTICE" text object 468 and restores script 4. | Cosmetic. Injection leaves `VAR_VERB_SCRIPT=203` set until room exit. |
| 34 | local-201 while script 67 runs (`data/scripts/room-034-high-stre/local-201.txt`) | Plays a sound only. | none |

A related engine fact: room entry always flushes the sentence queue (`data/scripts/global/script-006.txt [0065]`), so never queue a sentence across a room change.

---

## 7. Camera visibility (data for the "clickable" check)

The rules allow only sentences a click could produce (`rules/glitchless.md`, Banned 4), so the target object must be on screen.

**Rooms wider than 320 px** (derived, RMHD): 28 (640), 29 (496), 32 (344), 33 (1008), 34 (800), 35 (480), 37 (480), 51 (432), 52 (496), 53 (640), 59 (640), 61 (464), 64 (496).

**Rule of thumb.** With a following camera, the visible window is [c−160, c+160], where c = clamp(ego x, 160, width−160). This is approximate. The follow camera scrolls only when the ego leaves the screen band between strips 10 and 30 (80 to 240 px from the left edge), and then re-centres on the ego (`scumm.cpp`, where `camera._leftTrigger = 10` and `_rightTrigger = 30`; `camera.cpp ScummEngine::moveCamera`). So what is visible depends on where the ego walked from. Some rooms pin the camera instead:

| room | camera pinning (cite) | exits off screen on arrival |
|---|---|---|
| 34 | town: `RoomScroll(528,1121)`, so c ∈ [528,640]; mansion: `RoomScroll(160,160)` (`data/scripts/room-034-high-stre/entry.txt [0046]` to `[0056]`) | <ul><li>From the town half, 431/439 (x 0-88) are never visible. Hence the 34T/34M split.</li><li>Arriving from 35 (ego to x 750, so c = 640), 434 (456-472) and 436 (368-416) are off screen. 438, 437 and 432 are visible and can serve as walk-left steps.</li></ul> |
| 28 | c = 160 if ego x < 320, else 480 (`data/scripts/room-028-bar/local-201.txt [0000]` to `[0016]`) | 316 is off screen from 28L, and 315 from 28R. The curtain 323 is visible from both. A walk toward either door crosses x 320, so the camera switches and the door comes on screen (T25a). |
| 29 | c = 160 near the door, 336 deeper in (box > 5), back to 160 when box < 5 (`data/scripts/room-029-fortune/local-200.txt [0001]`, `[003D]`, `[005B]`) | 367 (88-120) is off screen while c = 336. Couch/trunk/chickens 369 lie in the overlap and can serve as walk-left steps. |
| 52 | c = 160 if ego x < 228, else 366 (`data/scripts/room-052-circus-gr/local-201.txt [000D]`, `[001D]`) | Arrival from the map walks the ego to x 430, so c = 366: **the tent 621 (48-168) is off screen.** No other object in 52 has a verb (index.json), so no sentence can bring it into view. See §8 Q1. |
| 61 | `RoomScroll(160,160)` then `RoomScroll(160,304)` (`data/scripts/room-061-sword-mas/entry.txt [0047]` to `[004E]`) | Path 743 (x 0-24) is visible on arrival. |
| 51 | `RoomScroll(224,272)` (`data/scripts/room-051-circus-te/entry.txt [001E]`) | 617 to 620 are visible. |
| 33 | follows the ego (1008 px) | Once the arrival walk ends, **all three exits appear to be off screen**, and no object sentence seems able to bring the cliffside into view. This is inferred from CDHD and a simplified camera model, and has not been verified in-engine:<ul><li>T02 and T03 end at x 346 and T46 at x 307, so the camera settles near c ≈ 346 or 307.</li><li>The only object with a verb on screen is then the poster 429 (x 256-280, Look at). Cliffside 426 (0-48), door 428 (696-736) and archway 427 (984-1008) are all off screen.</li><li>Walking to the poster (walk point x 268) stays inside the trigger band, so the camera does not move and 426 stays hidden. A real player reaches it by clicking the floor.</li><li>During the arrival walk itself, the camera starts at c = 160, because `loadRoomWithEgo` puts the ego at 426's walk point (5,75) first. So 426 is briefly on screen until the ego passes x ≈ 240.</li><li>Not checked: sentences on actors. The dock citizens walk between 428 and 427 (`data/scripts/room-033-dock/local-200.txt`), and a Look at or Talk to on a moving actor also walks the ego and so moves the camera.</li></ul> |

---

## 8. Open questions

1. **Off-screen targets (needs a policy decision; may block Part I as a whole).**
   - A strict "on screen when pushed" rule appears impossible to meet with object sentences alone. Verify in-engine.
     - On the dock, where the player first gets control, only the poster seems to be on screen once the arrival walk has ended. No object sentence appears to bring cliffside 426, door 428 or archway 427 into view (§7). The game would then be stuck on its first screen.
     - Exceptions not yet ruled out: pushing (11,426) during the arrival walk while 426 is still visible, and sentences on the walking dock citizens.
     - The same holds for the circus tent: you arrive in 52 at x 430, and nothing clickable lies between you and the tent (§7).
   - Choose one of:
     - (a) allow sentences on off-screen objects. The ego walks the same box path a floor click would.
     - (b) add a "walk to point" step type.
   - The choice also decides whether 34T/34M and 28L/28R are separate nodes or just cost an extra walk. The split graph in §5 is safe under either choice.
2. **Random map pirates.**
   - From the 4th map entry in phase A, wandering pirates can start an insult duel (room 49, global script 114) when one reaches a *stopped* ego (`data/scripts/room-085-melee/local-202.txt [018A]` to `[01A2]`). Insult duels are out of scope for v1.
   - Map entries made while following the storekeeper (script 67 running, `data/scripts/room-085-melee/entry.txt [0088]`) skip the pirate start, but `Var[290]` is still incremented on them (`[00C0]`).
   - The first three map entries are pirate-free. Seeds are fixed, so whether an encounter happens is deterministic but depends on the plan.
   - Need: either cap map entries in the model, or verify each map leg in-engine.
3. **Troll gate geometry.**
   - The claim that every west-to-east map walk passes (169,133) and triggers local-200 is derived from BOXD (box 34 is the segment (159,123) to (172,136)), not observed. Please confirm in-engine.
   - Also confirm that the walk points of 909 (234,16) and 917 (70,126), which lie off the box network, end within the 16 px reach.
4. **Cook timing for the kitchen (T27).** Is the precondition "cook in room 28 at x ≤ 310" (what a click allows) the one to model, or only "local-211 not running" (what an injected sentence needs)? See §6. The bar/kitchen analyst should settle it.
5. **`VAR_MACHINE_SPEED`.**
   - The intro measures it from `VAR_TMR_1` (`data/scripts/global/script-152.txt [001E]` to `[0046]`).
   - It gates the town citizens who open and close doors 437/438 and 444 (`data/scripts/room-034-high-stre/entry.txt [0063]`; `data/scripts/room-035-low-stree/entry.txt [009B]`) and the bar crowd (`data/scripts/room-028-bar/local-204.txt [0043]` to `[0050]`).
   - Headless fast mode could measure a different value than the visible demo, which would change door states between runs. Read it from the segment-start dump.
6. **T43 after the troll is paid.** Inferred: the x < 133 test in 914 always fails after the walk to (169,133), so you always enter the bridge at 654. This does not matter for the route, since the bridge is a dead end once paid.
7. **High Street entry branch `Var[101]==38`** (`data/scripts/room-034-high-stre/entry.txt [0073]` to `[0085]`). It places the ego at (680,136), but no script moves the ego from 38 to 34. It looks like debug-boot support. This document treats it as unreachable.
8. **Stan absent in phase A.** Room 59 places Stan only when `Var[196] ≥ 3` and `!Bit[51]` (`data/scripts/room-059-stans/entry.txt [0023]` to `[003A]`). That differs from what players of other versions may expect. It does not affect connectivity, but the ship/crew analyst should confirm it.
9. **Script kill on room change.** "Code after a taken `loadRoomWithEgo` never runs" is inferred from how `startScene` stops room scripts, not traced line by line. It only matters for the redirect blocks, which are written to be safe either way.
