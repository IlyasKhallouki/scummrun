# Part I: money, shops, the kitchen and the circus

Scope: how money is stored, every way to get money in Part I, the SCUMM Bar kitchen and its cook, the Fettucini circus, and what the store and the map seller charge.

Evidence labels used below:

- **Verified (script):** read in the descumm dump. Cited as `data/scripts/<file> [offset]`.
- **Verified (engine):** read in the pinned ScummVM source under `third_party/scummvm/`.
- **Raw block:** decoded by hand from a resource block.
  - Object positions come from the CDHD header in `build/blocks/DISK_0001/LECF/LFLF_<room>/ROOM/OBCD_<obj>`, read as v5 `{id u16, x/8, y/8, w/8, h/8, flags, parent, walk_x s16, walk_y s16, dir}`.
  - Initial owners, states and classes come from the DOBJ block of `game/classic/MONKEY1.000`, XOR 0x69. Class *n* is bit *n−1*.
  - Every class value used here agrees with the script logic around it.
- **Inferred:** a deduction that has not been run in the engine.

Class arguments: `setClass(o,[134])` **sets** class 6 and `[6]` **clears** it. `classOfIs(o,[6])` is true when class 6 is **clear**, and `[134]` is true when it is **set**. `[160]`/`[32]` set/clear class 32 (untouchable).

---

## 0. Blockers for the bridge and the rules (read first)

These findings change the PDDL model and the plan format, so they come before the details.

### B1. The circus helmet cannot be done with a sentence

- `Bit[103]` (helmet accepted) has exactly one reachable setter, the circus room's **input script**: `data/scripts/room-051-circus-te/local-200.txt [008B]`. (Verified, by grepping every `Bit[103]` write.)
  - The other setter, `data/scripts/room-051-circus-te/local-210.txt [001E]`, is dead code. Nothing assigns `Var[116] = 210` and nothing starts script 210 in room 51 (grep).
- Script 207 makes local 200 the input script at `data/scripts/room-051-circus-te/local-207.txt [0D4C]` (`VAR_VERB_SCRIPT = 200`). It then waits for `Bit[103]` at `[0D5C]`.
- The bridge pushes sentences into the sentence queue. That runs the sentence script (global 2) and **skips `VAR_VERB_SCRIPT`**, so local 200 never runs.
- What the pushed sentences do:
  - `Give (4) pot 567 → Fettucini actor 3/4`.
    - Global 2 sends a give-to-actor through `startScript(Var[116],[obj,ego,actor])` when the object has no Give entry and the actor has class 5 (`data/scripts/global/script-002.txt [00F2]` to `[0127]`). The pot has no verb-4 entry (`data/scripts/room-041-kitchen/obj-0567-pot.txt` events), and the brothers get class 5 at `data/scripts/room-051-circus-te/local-204.txt [0052]`, `[0059]`.
    - If `Var[116]` is 0: `runScript(0)` returns immediately (`third_party/scummvm/engines/scumm/script.cpp`, `runScript`: `if (!script) return;`), so nothing happens.
    - If `Var[116]` is 10 (several room exits set it, e.g. `data/scripts/room-036-mansion-e/exit.txt [0014]`): `data/scripts/global/script-010.txt [002E]` runs `setOwnerOf(567, actor)`. **The pot is lost, the helmet can never be given, and the money chain softlocks.**
    - In neither case is `Bit[103]` set.
  - `Use (7) pot`: the pot's Use entry (`data/scripts/room-041-kitchen/obj-0567-pot.txt [005D]`) re-issues `doSentence(7, <obj2>, 567)`. That does not touch `Bit[103]`.
- The only input path that needs no mouse coordinates is two **verb clicks** (inventory slots are verbs 200 to 207):
  1. Click verb 7 "Use". Local 200 hands it to script 4 (`[0129]`), which sets `Var[107] = 7` (`data/scripts/global/script-004.txt [0347]`).
  2. Click inventory verb `200+k`. Local 200 checks `200 ≤ Local[1] ≤ 205` (`[00EE]`, `[00F5]`), reads `Var[134 + (Local[1]-200)]` (`[0107]`), and requires `== 567` (`[010E]`), `Var[107] == 7` (`[0115]`) and `!Var[110]` (`[011C]`). Then it sets `Var[108] = 567` (`[0121]`) and jumps to the helmet code (`[0021]` to `[008B]`).
- **Off-by-one (verified):** the inventory script fills slot `k` (verb `200+k`) from `Var[133+k]` (`data/scripts/global/script-009.txt [0092]`, `[00BC]` to `[00C7]`), and global 4 reads it back the same way (`data/scripts/global/script-004.txt [0333]`). Local 200 reads `Var[134+k]`.
  - So the click that works is **the slot immediately before the pot** (pot in slot k+1, click verb `200+k`).
  - If the pot is the first visible item, this path does not exist. The pot must not be in display slot 0: hold another item (or money) acquired **before** the pot.
- The other input path, `Give` + pot + a click **on a brother**, uses `actorFromPos(VAR_VIRT_MOUSE_X, VAR_VIRT_MOUSE_Y)` (`data/scripts/room-051-circus-te/local-200.txt [0013]`). That is pixel input, which rule 4 bans.
- **Decision needed.** The plan format (C4) has sentences and dialogue `choose` only. The circus needs a third step kind, for example `click_verbs: [7, 200+k]`. This works through `runInputScript(kVerbClickArea, id)`, the same mechanism dialogue choices already use.
  - Without it, the circus payout is impossible, the shovel is unaffordable (§5), and the treasure trial cannot be completed.
  - The planner must never emit `Give pot → actor 3/4`.

### B2. The cook gate in the bar is weaker for pushed sentences than for clicks

- The bar installs its own input script: `data/scripts/room-028-bar/entry.txt [007D]` (`VAR_VERB_SCRIPT = 202`).
- On a scene click whose object is door 316, local 202 chains to local 203 (`data/scripts/room-028-bar/local-202.txt [001C]` to `[0023]`). Local 203 then checks:
  - Cook (actor 6) in the bar with `X > 310`: script 215 runs ("Don't go into the kitchen!") and the click is cancelled (`data/scripts/room-028-bar/local-203.txt [0019]` to `[002B]`).
  - Cook in the bar with `X ≤ 310`: the cook **freezes** (`stopScript(216)`, `stopScript(217)` at `[0030]` to `[0035]`) and the click goes on to script 4.
  - Cook not in the bar: the door's verb runs directly (`[003A]` to `[0050]`).
- A pushed sentence skips local 202/203. Its only gate is the door's own verb:
  - `Open (2) 316` is refused while script 211 runs (`data/scripts/room-028-bar/obj-0316-door.txt [0018]` to `[0021]`).
  - `Walk to (11) 316` only needs state 1 (`[003F]` to `[004B]`).
  - So a pushed sentence can get in while the cook stands in the doorway, which a click cannot. That breaks "exactly as the input script would push it".
- Recommendation:
  - Model the click semantics.
  - Add a C3 condition that can test the cook's position, e.g. `{"actor_room": 6, "eq": 28}` and `{"actor_x": 6, "le": 310}`, and put it in `until` together with `{"state": 316, "eq": 1}`.
- **Every cook trip opens a legal window.** All 29 possible cook targets (objects 330 to 358, see §3.1) have walk X between 55 and 285, which is ≤ 310 (raw block: CDHD of `build/blocks/DISK_0001/LECF/LFLF_0028/ROOM/OBCD_0330`…`OBCD_0358`). So on every trip the cook walks into the X ≤ 310 zone.
- Engine detail (verified): `walkActorToObject` and `getDist` on an object use its CDHD walk point in v5, and `getDist` is `max(|dx|,|dy|)` (`third_party/scummvm/engines/scumm/object.cpp`, `getObjectXYPos` v3-v5 branch, `getDist`).

### B3. Whether the storekeeper is there is random, and the RNG is not predictable

- On each store entry, `Var[100] = getRandomNr(3)` (0..3 inclusive) at `data/scripts/room-030-store/entry.txt [002B]`. A 0 means the storekeeper is away. The engine range is inclusive (`third_party/scummvm/common/random.cpp`, `getRandomNumber`: `% (max + 1)`).
  - Away: storekeeper 394 becomes untouchable (`[0057]`) and the bell becomes usable (`data/scripts/room-030-store/local-208.txt [000F]`).
- Dialogue wait loops call `getRandomNr(1)` every frame. Examples: `data/scripts/room-030-store/local-211.txt [043D]`; `data/scripts/room-051-circus-te/local-207.txt [0969]`, `[0A84]`, `[0BBA]`, `[0FDE]`.
  - Even with a fixed seed, the draw at store entry therefore depends on frame-exact history. The planner cannot predict it.
- **Default purchase path:** `Pick up` the item, then `Walk to (11)` door 387. Local 204 brings the storekeeper back if he is away and always ends in the pay dialogue (§4.1). This works whether or not he is there, so model it as the default.
- `Talk to 394` works only when he is present. A pushed sentence aimed at the untouchable 394 when he is away would start his dialogue with no actor in the room. The bridge should refuse sentences on class-32 objects.

### B4. The fish (red herring) looks unobtainable with sentences

The fish is not a money item. It is flagged here because the kitchen is in scope.

- The seagull leaves only when Guybrush steps on the plank: `getDist(ego,575) < 3` in `data/scripts/room-041-kitchen/local-203.txt [0001]`.
  - Object 575 is unnamed, has no verbs, and is class 32 (untouchable) from the start (raw DOBJ).
  - Its walk point is (298,134) (raw CDHD `OBCD_0575`).
- No touchable object has a walk point there. The fish's own walk point is (237,129), to the left of the plank.
- Inferred: no legal sentence walks Guybrush onto the plank, so the fish needs a scene (floor) click. See §3.3 and Open question 6.

### B5. Dialogue text quirks that the `choose` matcher must handle

- The circus "Bobbin" menu is written **backwards** and one entry has leading spaces: `data/scripts/room-051-circus-te/local-207.txt [0F40]`, `[0F91]`. Use `"nibboB"` or `"temleh"`.
- `^` in the dump is the game's ellipsis glyph, so keep it out of substrings.
- `unknown8(8224)` is a descumm placeholder for an escape code inside the "Of course I have a helmet" text (`[0B49]`). Match `"Of course"`.

### B6. More room-specific input/sentence scripts on the route (for rooms.md)

- Map room 85: `VAR_VERB_SCRIPT = 201` (`data/scripts/room-085-melee/entry.txt [0055]`).
- Troll room 57: `VAR_SENTENCE_SCRIPT = 202` (`data/scripts/room-057-bridge/entry.txt [0026]`).

---

## 1. (a) How money is represented

| Fact | Value | Evidence |
|---|---|---|
| Money variable | `Var[195]` = number of pieces of eight | Verified: `data/scripts/room-038-lookout/obj-0488-pieces-of-eight.txt [003A]` to `[007E]` (Look at prints it) |
| Money object | **488**, defined in room 38 (lookout). Its name is rewritten to "N pieces of eight" | `[00CC]`, `[00E3]` |
| Changing money | `startObject(488, 250, [add, sub])` runs verb 250 | `[0086]` to `[0100]` |
| Verb 250 logic | `Var[195] = Var[195] + add − sub` (`[008B]`). If the amount went up, `Bit[29]`…`Bit[35]` are cleared (`[009A]` to `[00B0]`). If `Var[195] < 1`, owner becomes 14 so it is hidden (`[00B7]` to `[00BE]`). Otherwise it is renamed and owned by ego (`[00C5]` to `[00FB]`). No clamping | verified |
| Initial state | 488: owner 15 (in room 38), state 0, classes {1, 32}, so untouchable | raw DOBJ |
| First lookout entry | owner is 15, so `pickupObject(488)` → `setOwnerOf(488,14)` → `startObject(488,250,[0,0])` | `data/scripts/room-038-lookout/entry.txt [002C]` to `[0040]` |
| **Starting money** | **0**. No non-debug script assigns `Var[195]`. The only direct writes are debug boot-param jumps (`Var[195] = 300`) | `data/scripts/global/script-001.txt [0A0F]`, `[0A42]`, `[0A75]`, `[0AB5]` |
| Debug grants (banned) | Boot-param jumps run only with `VAR_DEBUGMODE` (`data/scripts/global/script-001.txt [085F]`, `[086D]`). They grant 300/174/50 (`[0A04]`, `[0BCF]`, `[12CE]`, `[12FB]`). Debug key 33 grants +100 (`data/scripts/global/script-014.txt [0118]` to `[011D]`) | rules: banned |
| Inventory slot of 488 | `pickupObject` puts objects into `_inventory[]` in pickup order. `findInventory` lists them in that order, and `setOwnerOf(...,≠0)` never moves them (`third_party/scummvm/engines/scumm/object.cpp`, `addObjectToInventory`/`getInventorySlot`/`findInventory`/`setOwnerOf`) | engine verified |
| | So 488, picked up at the first lookout entry, takes `_inventory[0]`. Whenever money > 0 it is the **first** visible item | **inferred**; confirm with a state dump |

All money changes in the game (grep of `startObject(488,250` plus `Var[195]`):

| add/sub | Where | Part I? |
|---|---|---|
| +478 | Circus payout, `data/scripts/room-051-circus-te/local-207.txt [110E]` | yes |
| +2 | Minutes deal, `data/scripts/room-035-low-stree/local-216.txt [093E]` | yes |
| +2 | Road-pirate loan, `data/scripts/room-049-road/local-200.txt [053E]` | yes, conditional |
| −75 | Shovel, `data/scripts/room-030-store/local-211.txt [09C3]` | yes |
| −100 | Sword, `data/scripts/room-030-store/local-211.txt [0656]` | yes (not a goal) |
| −1 | Breath mints, `data/scripts/room-030-store/local-211.txt [1BC8]` | yes |
| −100 | Treasure map, `data/scripts/room-035-low-stree/local-218.txt [0945]` | yes |
| −30 | Captain Smirk's training, `data/scripts/global/script-057.txt [0BD9]` | yes (sword trial) |
| −1 | Give money to Otis, `data/scripts/room-031-jail/local-203.txt [0287]` | yes (useless) |
| −1 | Grog machine at Stan's, `data/scripts/room-059-stans/obj-0690-grog-machine.txt [0092]` | reachable; useless |
| +2, +2 | Coins 735/736 at Stan's, `data/scripts/room-059-stans/obj-0735-pieces-of-eight.txt [0017]`, `obj-0736 [0017]` | **no**. They are made touchable only by end-game script 133 (`data/scripts/global/script-133.txt [01C4]`, `[01CB]`), which is started from `data/scripts/room-045-cu-church/local-200.txt [1594]` |

---

## 2. (b) Money sources in Part I

### 2.1 The circus (Fettucini brothers): +478, once

**Getting there.**
- Map 85: `Walk to (11)` 912 "clearing" → room 52 (`data/scripts/room-085-melee/obj-0912-clearing.txt [000C]`).
- Room 52: `Walk to (11)` 621 "circus tent". If `!Bit[103]` this runs `loadRoom(51)`, otherwise "brothers are still in there" (`data/scripts/room-052-circus-gr/obj-0621-circus-tent.txt [000F]` to `[0019]`). So the tent is shut after the payout.
- There are no other preconditions. No items, flags or money are checked.
- Room 51's entry walks ego in under a cutscene, then starts local 207 (`data/scripts/room-051-circus-te/entry.txt [0036]` to `[004A]`). The entry also preloads the helmet costume if ego owns the pot (`[0000]` to `[001B]`). That is cosmetic only.
- The map coordinates suggest the clearing (walk 133,87) is on the same side of the troll bridge (914 at x 160 to 176, y 128 to 144) as the lookout (77,119) and the village (70,126). **Inferred** from raw CDHD of `LFLF_0085`. The walkbox graph is not checked, and the clearing's X sits on the bridge script's `< 133` threshold (Open question 6).

**Dialogue, local 207.** During the first menu the input script is local 209 (`[0005]`). Every later menu uses global 14 (`[03B2]`). Both turn a clicked verb id 120+ into `Var[194]` (`data/scripts/room-051-circus-te/local-209.txt [0013]`, `data/scripts/global/script-014.txt [0105]`).

| # | Shown when | Choices in display order (verb id) | Result |
|---|---|---|---|
| M1 | always (`[0204]` to `[0396]`) | ". . .ahem" (120), "bathroom" (121), "fine jackets" (122), "trampoline" (123), "ridiculous outfits" (124) | **Any choice.** The script only waits for a non-zero `Var[194]` (`[03A3]`). Suggested substring: `"ahem"` |
| - | | First visit (`!Bit[71]`): a long sales pitch, skippable (`[0438]` to `[0882]`). Return visit: "Hello again." (`[03EC]`). If `Bit[72]` is also set, it jumps straight to M3 (`[042B]`) | `Bit[71] = 1` (`[088D]`) |
| M2 | first-visit path (`[08AB]` to `[095D]`) | "OK, I'll do it." (120), "How much will you pay me?" (121), "Forget it!" (122) | 120: "We'll pay you 478 pieces of eight" (`[097A]` to `[097F]`) → M3. 121: "How about 478…" (`[09D2]`) → sub-menu "OK, sounds good." (120) → M3, or "No way!" (121) → leave (`[0A13]`, `[0A55]`, `[0A8E]` to `[0AA2]`). 122: leave (`[09C7]`). Suggested: `"I'll do it"` |
| M3 | `[0AA8]` to `[0BAE]` | "Er^ no, I don't have a helmet…" (**121, shown first**), "Of course I have a helmet…" (**120, shown second**) | Sets `Bit[72] = 1` (`[0AA8]`). 120 → the brothers fuss (skippable) → helmet phase (`[0BC4]` to `[0C65]`). 121 → "Go get a helmet" (skippable) → leave (`[0C68]` to `[0D3F]`). Suggested: `"Of course"`. **Do not** use "have a helmet": it matches both |
| H | `[0D4C]` to `[0D5C]` | Helmet input. Not a menu (see B1) | `Bit[103] = 1` |
| - | `[0D61]` to `[0F07]` | Cannon cutscene. Script 203 removes the pot: `setOwnerOf(567,0)` (`data/scripts/room-051-circus-te/local-203.txt [00C3]`) | pot is gone |
| M4 | `[0F40]` to `[0FE3]` | Reversed text: "…nibboB m'I" (120), "?temleh ym s'erehW" (121) | Any choice. Suggested: `"nibboB"` |
| - | `[1041]` to `[1107]` | Thanks (skippable) | **`startObject(488,250,[478,0])`** (`[110E]`) |
| - | `[1125]` to `[114D]` | The input script is restored (`[113E]`) and ego is walked out with `startObject(617,11)` → room 52 (`[114D]`; `data/scripts/room-051-circus-te/obj-0617-outside.txt [000C]`) | room 51 → 52 |

"Leave" means `goto 111E`: the input script is restored and ego walks out to room 52 (`[111E]` to `[114D]`). If ego comes back with `Bit[71]` and `Bit[72]` set, the dialogue jumps straight to M3.

**What counts as a helmet.**
- **Only the pot (567).** Local 200 accepts only `Var[108] == 567` (Give path, `[0032]`) or `Local[5] == 567` (inventory path, `[010E]`).
- Anything else gets "That's no helmet." (`[0093]`).
- There is no alternative headgear. The pot is not referenced anywhere else in a way that matters: the barrel refuses it (`data/scripts/room-041-kitchen/obj-0569-barrel.txt [008B]`), and the troll lists it only as a dialogue option (`data/scripts/global/script-055.txt [0820]`).

**During the helmet wait (local 200 as input script).**
- `Walk to` the "outside" objects 617 to 620 still works (`[00BE]` to `[00D3]` chain to script 4).
- Every other scene click is swallowed (`[00DF]` to `[00E5]`).
- Leaving and coming back resumes at M3.

**Effects:** `Var[195] += 478`, `Bit[71]`, `Bit[72]`, `Bit[103]`. Pot 567 goes to owner 0. The tent is closed for good (obj-0621 `[000F]`), and on later visits the brothers are removed (`data/scripts/room-051-circus-te/local-204.txt [0000]`, `[007B]`).

### 2.2 Minutes from the "Men of Low Moral Fiber": +2, once

- Sentence: `Talk to (10)` 452 in room 35 → local 216 (`data/scripts/room-035-low-stree/obj-0452-men-of-low-moral-fiber-pirates.txt [0015]`).
- Route A: choice 122. Its text is "…treasure map around here?" when `!Bit[129] && !Bit[16]` (`data/scripts/room-035-low-stree/local-216.txt [026B]` to `[02CE]`), or "…sneaky-looking man…" when `Bit[16]` (`[01EB]` to `[025C]`).
  - It goes to `[0564]`: `Bit[129] = 1`, then a skippable pitch, then "Want one?" (`[07C1]`).
  - Next menu: "No, thanks." (120), "No, I must be on my way." (122), "…two pieces of eight." (121) (`[07FB]`, `[0838]`, `[0882]`).
  - Choice **121** → "OK, that's fair." → `pickupObject(464)` (minutes) and **+2** (`[0926]` to `[093E]`).
- Route B: once `Bit[129]` is set and ego does not own 464, choice 128 "I'll take those minutes…" goes straight to the same deal (`[02D8]` to `[0349]`, `[055E]`).
- Afterwards the main menu returns (`[095B]`). Exit with "running along" (124), which goes to `[1CF4]` (`[0484]`, `[052A]`).
- Suggested substrings: `"treasure map"` or `"sneaky"`, then `"two pieces"`, then `"running along"`.

### 2.3 Road-pirate loan: +2, conditional

- This happens only in the random map encounters (room 49).
  - Wandering pirates start on the map from the third map entry, while `Var[196] < 3` and script 67 is not running (`data/scripts/room-085-melee/entry.txt [0088]` to `[00BA]`).
  - The encounter fires when a pirate reaches a **standing** Guybrush (distance < 2), via `startScript(114)` → `loadRoom(49)` (`data/scripts/room-085-melee/local-202.txt [018A]` to `[01A2]`, `data/scripts/global/script-114.txt [0035]`).
- The choice "…couple of pieces of eight^" (126) is offered when `!Bit[19]`, `Var[195] < 2` and ego does not own the mints 395 (`data/scripts/room-049-road/local-200.txt [022F]`, `[0296]` to `[030F]`).
- It pays +2 only if `Bit[420]` is set (Otis's bad breath is known; `[04FA]` to `[053E]`). Otherwise one of 4 random refusals plays (`[054C]` to `[06D6]`).
- The loan is not worth planning for. It is listed only because the encounters can interrupt map walking.

### 2.4 Things that are *not* sources

- Nothing in Part I can be sold for money. No script adds money in exchange for an item, apart from the minutes deal (grep).
- The troll refuses money and takes none: "such a paltry amount" (`data/scripts/room-057-bridge/local-204.txt [009E]` to `[00EC]`). Its menu only *mentions* the amount (`data/scripts/global/script-055.txt [0935]` to `[0A1A]`).
- Debug grants are banned (§1).

---

## 3. (c) The SCUMM Bar kitchen and the cook

### 3.1 How the cook moves

Actor 6 is the cook. All of this is room-28 local code, so it stops when ego leaves the bar. Inferred: ScummVM kills room-local scripts on a room change, which is standard engine behaviour.

- **Bar entry** (`data/scripts/room-028-bar/entry.txt [0012]` to `[0017]`, while `!Bit[453]`) starts local 205:
  - Coming from the kitchen (`Var[101] == 41`): local 216 starts, so the cook comes out (`data/scripts/room-028-bar/local-205.txt [0033]` to `[003A]`).
    - `Var[101]` is the previous room. Global 7 is the exit hook (`data/scripts/global/script-001.txt [066E]`) and sets `Var[101] = VAR_ROOM` (`data/scripts/global/script-007.txt [0000]`).
  - Coming from anywhere else: `setState(316,0)` closes the kitchen door, and local 211 starts with the cook **in the kitchen** (`[0040]` to `[0044]`).
- **Local 211, cook in the kitchen** (`data/scripts/room-028-bar/local-211.txt`):
  - `putActorInRoom(6,0)` (`[0000]`).
  - Wait while local 220 (the pirate-leaders dialogue) runs (`[0004]`).
  - **`delayVariable((getRandomNr(20) + 30) * 60)`** (`[000D]` to `[001E]`). That is 1800 to 3000 jiffies, **30 to 50 s, random and uniform over 21 values**.
    - Delays count down in jiffies: `decreaseScriptDelay(delta)` in `third_party/scummvm/engines/scumm/scumm.cpp`, where delta is in jiffies.
  - If local 220 or local 203 is running when the delay ends, it goes back to `[0003]` and **draws a new delay** (`[0021]` to `[0036]`). Talking to the pirate leaders at that moment resets the timer.
  - Otherwise local 216 starts (`[0039]`).
- **Local 216, cook out in the bar** (`data/scripts/room-028-bar/local-216.txt`):
  - If the cook is not in room 28: put him at (660,100), stop 211 and 212, `startObject(316,2)` (**the door opens**), walk to (595,125) (`[000B]` to `[003A]`).
  - Walk to a random object `330 + getRandomNr(28)` (`[003F]` to `[004C]`). Local 217 starts (`[0050]`).
  - Wait to arrive (`[0056]`), then wait while 220 runs (`[005C]`).
  - Walk back to door 316, open it, walk to (660,123), **close it** with `startObject(316,3)`, and start 211 again (`[0065]` to `[0085]`).
  - There is no idle time at the target.
- **Local 217:** if ego comes within 5 of the cook's target, 216 restarts and he picks a new target (`data/scripts/room-028-bar/local-217.txt [0000]` to `[000E]`).
- **Local 212, the shortcut timer:** two `delay(300)` calls, so 600 jiffies (10 s). Then, if neither 220 nor 203 is running, 216 starts and the cook comes out (`data/scripts/room-028-bar/local-212.txt [0000]` to `[0020]`).
  - Local 212 is started by local 214, the refused door attempt (§3.2).
- **Click input in the bar (local 202):** any scene click not on door 316 restarts local 216 if the cook is standing still in the bar at X ≤ 310 (`data/scripts/room-028-bar/local-202.txt [0034]` to `[005B]`). This only applies to clicks, not to pushed sentences (B2).
- Cook targets, raw CDHD of `LFLF_0028` objects 330 to 358, as walk positions:
  - 330-332, 345-347 and 351-353: (114,127)
  - 333-335: (235,126)
  - 336-338 and 354-356: (201,129)
  - 339-341: (285,128)
  - 342-344 and 357-358: (55,131)
  - 348-350: (203,126)
  - Door 316's walk point is (595,125). **Every target has X ≤ 285.**

Summary for the model:

- The door state tracks the cook exactly. State 1 means the cook is out in the bar; state 0 with 211 running means he is in the kitchen.
- The cook-out phase lasts one walk from the door to a random pirate and back.
- The cook-in phase lasts 30 to 50 s, or about 10 s after a refused door attempt.
- Neither duration can be predicted. The bridge has to wait on state.

### 3.2 Getting in, and what happens if the cook is in the kitchen

| Sentence (room 28) | Precondition | Effect | Evidence |
|---|---|---|---|
| `Open (2)` 316 "door" while 211 runs (cook in the kitchen) | - | **Blocked.** Local 214 runs as a cutscene: door opens, the cook's warning (one message, two parts joined by `wait()`), door closes, then **local 212 starts**, which brings the cook out after 600 jiffies | `data/scripts/room-028-bar/obj-0316-door.txt [0018]` to `[0021]`; `data/scripts/room-028-bar/local-214.txt [0005]` to `[004B]` |
| `Open (2)` 316 while 211 is not running | - | Script 25 opens 316 and the paired 570. Neither door has class 6 (locked) or 11 (unpaired) (raw DOBJ: 316, 570 have classes {15}) | `[0027]`; `data/scripts/global/script-025.txt [0016]` to `[0036]` |
| **`Walk to (11)` 316** | **state(316) == 1**, i.e. the cook is out. With click semantics, also: cook X ≤ 310 or cook not in room 28 (B2) | Local 218 runs as a cutscene: walk to (660,120), then `loadRoomWithEgo(570,41)` | `[003F]` to `[004B]`; `data/scripts/room-028-bar/local-218.txt [0000]` to `[0017]` |
| Any click on 316 while the cook is in the bar at X > 310 (click semantics only) | - | "Don't go into the kitchen!". If cook X < 520, 216 restarts and he picks a new target. Otherwise he goes back into the kitchen and 211 restarts | `data/scripts/room-028-bar/local-215.txt [0010]` to `[0083]` |

- Because the cook opens 316 himself when he comes out (`local-216 [001B]`), the minimal entry is a single sentence, `Walk to 316`, with `until state(316)==1` (plus the actor-X check in B2). `Open 316` is not needed.
- **Accelerator:** if the cook is in the kitchen when ego arrives, `Open 316` costs the local-214 cutscene, but then the cook comes out after 600 jiffies instead of after the 1800 to 3000 jiffies left on the timer.
- Leaving the kitchen: `Walk to (11)` 570 needs state 1 (`data/scripts/room-041-kitchen/obj-0570-door.txt [0030]` to `[003C]`) and goes to room 28. 570 was opened as 316's pair. Bar entry then restarts 216 (§3.1).
- The kitchen itself has **no cook logic** (no actor 6 in any room-41 script; grep). Ego can stay there as long as he likes.

### 3.3 What can be picked up in the kitchen (room 41)

Initial owner, state and classes are raw DOBJ. Walk points are raw CDHD of `LFLF_0041`.

| Object | Sentence | Preconditions | Effect | Evidence |
|---|---|---|---|---|
| 567 "pot" (walk 104,122; classes {2,7,16}) | `Pick up (9)` 567 | owner 15 | `pickupObject(567)` | `data/scripts/room-041-kitchen/obj-0567-pot.txt [001E]` to `[002A]` |
| 566 "hunk of meat" (walk 75,124; classes {2,7,15}) | `Pick up (9)` 566 | owner 15 | `pickupObject(566)`. Used in the idol chain via the stew (`data/scripts/room-041-kitchen/local-213.txt [0077]` to `[00CC]`, `local-214.txt [0000]` to `[001B]`) | `data/scripts/room-041-kitchen/obj-0566-hunk-of-meat.txt [0035]` to `[0041]` |
| 568 "fish" (walk 237,129; classes {2,6,16}) | `Pick up (9)` 568 | owner 15 **and class 6 clear** (gull away). Otherwise "bird will peck my hand off" | `pickupObject(568)` | `data/scripts/room-041-kitchen/obj-0568-fish.txt [001E]` to `[003A]` |
| 574 "pot o' stew" | `Use` meat/fish with it, then `Pick up` | - | Stewed meat or stewed fish (idol chain, see idol.md) | `data/scripts/room-041-kitchen/obj-0574-pot-o-stew.txt [0036]` to `[0065]`, `[00B4]` |

**Fish and seagull mechanics.** The fish is only needed by the troll (`data/scripts/room-057-bridge/local-204.txt [0002]` to `[0012]`); it is not money-related.

- The fish sits on the pier behind door 564. While 564 is closed, walkboxes 4/5 are disabled and the fish is untouchable (`data/scripts/room-041-kitchen/entry.txt [0048]` to `[005C]`).
- `Open (2)` or `Use (7)` 564 enables the boxes, makes the fish touchable, and starts the plank watcher, local 203 (`data/scripts/room-041-kitchen/obj-0564-door.txt [0018]` to `[003D]`). Door 564 has no lock class (raw DOBJ {15}).
- First kitchen visit with the fish still there: the gull lands when ego X > 170 (`Bit[424]`, local 205 sets class 6) (`data/scripts/room-041-kitchen/local-204.txt [000C]` to `[0023]`, `local-205.txt [0021]`). On later visits it is already there (`local-204 [0029]`). The fish starts with class 6 set (raw DOBJ).
- Stepping on the plank (`getDist(ego,575) < 3`) runs local 202. Ego bounces to (294,131) (`data/scripts/room-041-kitchen/local-202.txt [0043]`).
  - If `!Bit[100]` and the gull is on the fish, local 206 runs: the gull flies up and **class 6 is cleared, so the fish can be picked up** (`local-206.txt [0002]`).
  - When the gull lands again, 207/208/209 set class 6 back (`local-207.txt [002D]`, `local-208.txt [0092]`, `local-209.txt [011C]`).
  - Which flight plays depends on the attempt counter `Var[272]`:
    - 0: short hop (207).
    - 1: medium (208).
    - ≥2: long (209). At >2 it adds local 210, and at >3 it adds 210 again and resets `Var[272]` to 1 (`local-206.txt [0036]` to `[0064]`, `local-209.txt [0098]`, `[00F4]` to `[0108]`).
  - If the fish is still there after the flight, `Var[272]++` (`local-206.txt [0071]` to `[0080]`). Otherwise local 211 sets `Bit[100] = 1` and the gull leaves for good. After that the plank says "The plank's stuck." (`local-211.txt [0007]`, `local-202.txt [0028]`).
- Feasibility with sentences only: see B4.

---

## 4. (d) Shops and purchases

### 4.1 The store (room 30)

See treasure.md §2 for the shovel step by step. The store facts this model needs:

**Buying works by picking the item up, then paying in dialogue.**
- `Pick up (9)` sword 388 or shovel 396 is a plain `pickupObject` with **no money check** (`data/scripts/room-030-store/obj-0388-sword.txt [0067]`, `data/scripts/room-030-store/obj-0396-shovel.txt [0079]`).
- Being paid for is a flag: `Bit[98]` for the sword, `Bit[99]` for the shovel.
- Leaving with an unpaid item is impossible (local 204, below).
- The breath mints 395 have **no** Pick-up verb (events 7, 8, 90, 91 only; `data/scripts/room-030-store/obj-0395-breath-mints.txt`). They are bought only through dialogue.

**Getting into the dialogue.**

| Sentence | When | Effect | Evidence |
|---|---|---|---|
| `Talk to (10)` 394 "storekeeper" | storekeeper present | Local 211 opens with "Waddya want?" | `data/scripts/room-030-store/obj-0394-storekeeper.txt [0015]`; `data/scripts/room-030-store/local-211.txt [002D]` |
| `Push (5)` / `Use (7)` 399 "bell" | storekeeper away (the bell is touchable only then) | Local 207 runs: he walks back in (skippable), then local 211 starts. It **closes door 387** | `data/scripts/room-030-store/obj-0399-bell.txt [0018]`; `data/scripts/room-030-store/local-207.txt [00FD]`, `[01A1]` |
| **`Walk to (11)` 387 "door"** holding an unpaid sword/shovel | door 387 state 1. It is opened together with high-street door 437, since 387 has no class 11 (raw DOBJ; `data/scripts/room-034-high-stre/obj-0437-door.txt [0018]`) | Local 204 runs as a cutscene. If the storekeeper is away he walks in and you get "Caught you…" (`Bit[324]`). Then "pay for that?" with `Bit[309] = 1`, then 211, skipping the greeting. **Works whether or not he is present (B3)** | `data/scripts/room-030-store/obj-0387-door.txt [008C]` to `[0098]`; `data/scripts/room-030-store/local-204.txt [0000]` to `[042B]`; `local-211.txt [0020]` to `[0025]` |
| `Walk to (11)` 387 with nothing unpaid | state 1 | `loadRoomWithEgo(437,34)`, out to high street | `local-204.txt [0031]` to `[0044]` |

Ego is kept at X ≤ 250 by local 214 (`data/scripts/room-030-store/local-214.txt [0000]` to `[0023]`).

**Main menu of local 211.** It is rebuilt at `[003F]` after every topic. Choices appear in this order:

| id | Substring | Shown when | Evidence |
|---|---|---|---|
| 120 | "About this sword" | ego owns 388 and `!Bit[98]` | `[0047]` to `[009B]` |
| 121 | **"shovel"** | ego owns 396 and `!Bit[99]` | `[00A0]` to `[00F5]` |
| 122 | "Sword Master" / "test it out" | `Var[199]` set (from the pirate leaders: `data/scripts/room-028-bar/local-220.txt [1036]`) / `Bit[98]` | `[00FA]` to `[01D9]` |
| 123 | "note of credit" | `Bit[28]`, `!Bit[101]`, `!Bit[320]` | `[01DE]` to `[024C]` |
| 124 | **"breath mint"** | `Bit[420]` (Otis's breath; `data/scripts/room-031-jail/obj-0405-prisoner.txt [001A]`, `data/scripts/room-031-jail/local-203.txt [0351]`) and `!Bit[312]` | `[0251]` to `[02AE]` |
| 126 | "rat repellent" | `!Bit[456]`, `Bit[95]`, ego does not own 420, `!Bit[85]` | `[02B3]` to `[0328]` |
| 127 | "files" | ego owns 640, `!Bit[457]` | `[032D]` to `[0382]` |
| 125 | **"browse"** | always | `[0387]` to `[03D2]` |

- If none of 120 to 127 is shown, Guybrush says "browse for now" and the dialogue **ends on its own** with no menu (`[03EB]` to `[0433]`).
- **The step's `choose` list must end with "browse" whenever any other topic is still visible after the purchase**, e.g. `Var[199]` or the mints topic. Otherwise the bridge stops with `unexpected_choice`.
- "browse" (125) ends with one random parting line (`[1BD6]`, `[1DD5]` to `[1ECD]`).

**Shovel: 75 pieces of eight.**
- The sub-menu comes from local 206 ("What about it?"): "I want it." (120: `Var[105] = 1`, `Bit[479] = 1`), "How much is it?" (121: `Var[105] = 1`), "I don't want it." (122: `Var[105] = 0`) (`data/scripts/room-030-store/local-206.txt [0030]` to `[0119]`).
- Money check: **`if (Var[195] >= 75)`** at `data/scripts/room-030-store/local-211.txt [0733]`, the direct path after "I want it". The same check at `[0802]` decides whether "I'll take it." is offered after the price quote ("75 pieces of eight", `[07A8]`).
- Purchase (`[0910]` to `[09CE]`): a skippable spiel, then **`startObject(488,250,[0,75])`** (`[09C3]`) and **`Bit[99] = 1`** (`[09CE]`). Then "What else do you want?" and back to the main menu (`[1DB6]` to `[1DD2]`).
- Not enough money: after the price quote only "I don't have that much" (121) and "I don't want it" (122) are offered. Either one gives "go put it back". The shovel goes back on the shelf (`setOwnerOf(396,15)`, class 32 cleared, state 0), then "something here maybe that you CAN afford?" and back to the menu (`[09E0]` to `[0A96]`).
- Minimal `choose`: `["shovel", "I want it"]`, plus `"browse"` if other topics remain.

**Breath mints: 1 piece of eight.**
- Choice 124 → "You're telling me!" → **`if (Var[195] > 0)`** (`[1B1C]`).
- Then a skippable offer, `Bit[312] = 1` (`[1BB1]`), `pickupObject(395)` (`[1BC0]`) and **`startObject(488,250,[0,1])`** (`[1BC8]`).
- With no money: "they ain't free." and back to the menu (`[1B88]` to `[1BAE]`).
- The topic needs `Bit[420]`, so **Otis must have been spoken to first**.
- The mints matter for the idol chain: they clear Otis's class 6 (`data/scripts/room-031-jail/local-203.txt [00B8]` to `[00BF]`), which is the condition for trading him the gopher repellent 640 for the cake (`[0115]` to `[011F]`). See idol.md.

**Sword: 100 pieces of eight (noted only; not a goal).**
- Choice 120 → local 206 → `if (Var[195] >= 100)` at `[0465]` (direct) or `[0520]` (offer "I'll take it.").
- Purchase: `startObject(488,250,[0,100])` (`[0656]`) and `Bit[98] = 1` (`[0661]`).
- Declining puts the sword back (`[0673]` to `[06D3]`).

**Giving things to the storekeeper** (verb 80 → local 210) never pays for anything.
- Money gives "What's that for?" (`data/scripts/room-030-store/local-210.txt [0087]` to `[0091]`).
- Other items give "We only take cash here." (`[00FE]`).

### 4.2 The map seller (room 35, object 441 "Citizen of Mêlée")

Full dialogue in treasure.md §1.

- Sentence: `Talk to (10)` 441 → local 218 (`data/scripts/room-035-low-stree/obj-0441-citizen-of-mle.txt [0015]`).
- **Price: 100.** "Only 100 pieces of eight" (`data/scripts/room-035-low-stree/local-218.txt [0773]`).
- Money check: **`if (Var[195] >= 100)`** at `[07F6]` adds choice 121 "I'll take it… swell gift".
- Purchase (`[08EF]` to `[0945]`): `pickupObject(442)` (`[0936]`), `Bit[65] = 1` (`[093E]`), **`startObject(488,250,[0,100])`** (`[0945]`).
- Not enough money: only "don't have enough money" (120) and "don't want it" (122) are offered. Both end the talk (`[08B3]`, `[0961]`).
  - `Bit[475]` is set at `[06AA]`, so the next talk goes straight to the price menu (`[008E]` to `[0116]`).
- Minimal `choose`:
  - First talk (`!Bit[16]`): `["barber", "swell gift"]`. Choice 123 → "Close enough." → pitch (`[04AA]`, `[05AE]` to `[05CB]`).
  - Later, if `Bit[475]` is not set: `["just want", "swell gift"]` (`[012B]`, `[0339]`).
  - If `Bit[475]` is set: `["swell gift"]`.
- The treasure forest can be entered without the map: the gate accepts the map, **or** script 67 running, **or** `Bit[401]` (`data/scripts/room-058-damnfores/obj-0685-path.txt [004F]` to `[00C6]`). So the map may not be needed. Deciding that belongs to treasure.md.

### 4.3 Captain Smirk (sword trial; noted only)

- Training costs 30. The offer "I've got 30 pieces of eight." appears only if **`Var[195] > 30`**, i.e. at least 31 (`data/scripts/global/script-057.txt [0A6B]` to `[0AB9]`).
- Paying: `startObject(488,250,[0,30])` and `Bit[107] = 1` (`[0BD9]` to `[0BE4]`).

---

## 5. (e) Money needed against money available

| Purchase | Price | Check | Needed for |
|---|---:|---|---|
| Shovel | 75 | `>= 75` | treasure |
| Map | 100 | `>= 100` | treasure (maybe avoidable, §4.2) |
| Breath mints | 1 | `> 0` | idol, if the Otis trade is used |
| **Treasure + idol, worst case** | **176** | | |
| Sword (not a goal) | 100 | `>= 100` | sword trial |
| Smirk (not a goal) | 30 | `> 30` | sword trial |
| **All Part I purchases** | **306** | | |

- Available: start 0. Circus +478. Minutes +2. Loan +2, conditional and only while `Var[195] < 2`. **At most 482.**
- **Without the circus: at most 4.** That is below the shovel's 75, so **the circus is mandatory**, assuming the shovel is required to dig: `Use shovel with X`, `data/scripts/room-064-treasure/obj-0749-x.txt [006B]` per treasure.md.
- **One circus payout is enough**: 478 ≥ 176, leaving 302. It even covers all 306 of Part I, leaving 172.
- Every combination of Part I purchases leaves at least 172, so Smirk's `> 30` check always passes after the circus.
- PDDL encoding (facts only, no numbers):
  - `(paid-circus)` means every remaining Part I purchase is affordable. No money facts need to be deleted after the circus.
  - `(money-ge-1)` covers the mints. It holds after the circus, or after the minutes/loan.
- Hidden cost to model: the pot is consumed by the circus (`local-203 [00C3]`). Nothing else needs it.

---

## 6. (f) Minimal action sequence: fresh start at the lookout → 478 in hand

Assumptions:
- Ego is in room 38 with control, no items, `Var[195] = 0` (see start.md for the boot/lookout opening).
- The circus helmet uses the verb-click step proposed in B1.
- Bar entry uses click-faithful gating (B2).

"T" counts **room transitions** separately; forced cutscene rooms are listed but not counted.

| # | Room | Input | Preconditions / waits | Effect | T | Evidence |
|---|---|---|---|---|---|---|
| 1 | 38 | `Walk to (11)` 486 "stairs" (verb 255 entry, any verb works) | - | First use (`!Bit[395]`): `Bit[395] = 1` and `loadRoom(96)`, the skippable "Part One" title. Then `loadRoomWithEgo(426,33)` | 38→33 (+ forced room 96) | `data/scripts/room-038-lookout/obj-0486-stairs.txt [004A]` to `[0054]`; `data/scripts/room-096-part1/local-200.txt [003B]` |
| 2 | 33 | `Open (2)` 428 "door" | - | 428 and its pair 315 open | | `data/scripts/room-033-dock/obj-0428-door.txt [0015]`; `data/scripts/global/script-025.txt [0024]` to `[0036]` |
| 3 | 33 | `Walk to (11)` 428 | state(428) == 1 | → bar. Local 205 closes 316 and starts 211, so the cook is in the kitchen | 33→28 | `[0029]` to `[0035]`; `data/scripts/room-028-bar/local-205.txt [0040]` to `[0044]` |
| 3a | 28 | *(optional accelerator)* `Open (2)` 316 | 211 running | Refused (local 214), then the cook comes out after 600 jiffies (local 212) | | §3.2 |
| 4 | 28 | `Walk to (11)` 316 | **until** state(316) == 1 **and** cook X ≤ 310 (B2) | Local 218 → kitchen | 28→41 | `data/scripts/room-028-bar/obj-0316-door.txt [003F]`; `data/scripts/room-028-bar/local-218.txt [0017]` |
| 5 | 41 | `Pick up (9)` 566 "hunk of meat" | owner 15 | Meat in inventory. **Must come before the pot**, so the pot is not in display slot 0 (B1). It is also needed for the idol | | `data/scripts/room-041-kitchen/obj-0566-hunk-of-meat.txt [0035]` |
| 6 | 41 | `Pick up (9)` 567 "pot" | owner 15 | Pot in inventory, display slot 1 | | `data/scripts/room-041-kitchen/obj-0567-pot.txt [001E]` |
| 7 | 41 | `Walk to (11)` 570 "door" | state(570) == 1 (opened with 316) | → bar. Local 216 restarts | 41→28 | `data/scripts/room-041-kitchen/obj-0570-door.txt [0030]` |
| 8 | 28 | `Walk to (11)` 315 "door" | state(315) == 1 (opened in step 2) | First exit (`!Bit[446]`): global 120, the skippable LeChuck cutscene (rooms 0/70/72), ending in `loadRoomWithEgo(428,33)` | 28→33 (+ forced cutscene rooms) | `data/scripts/room-028-bar/obj-0315-door.txt [0072]` to `[008A]`; `data/scripts/global/script-120.txt [0000]`, `[0538]` |
| 9 | 33 | `Walk to (11)` 426 "cliffside" | - | → lookout | 33→38 | `data/scripts/room-033-dock/obj-0426-cliffside.txt [000C]` |
| 10 | 38 | `Walk to (11)` 487 "path" | - | → map | 38→85 | `data/scripts/room-038-lookout/obj-0487-path.txt [0010]` |
| 11 | 85 | `Walk to (11)` 912 "clearing" | - | → circus grounds | 85→52 | `data/scripts/room-085-melee/obj-0912-clearing.txt [000C]` |
| 12 | 52 | `Walk to (11)` 621 "circus tent", choose `["ahem", "I'll do it", "Of course"]` | `!Bit[103]` | → tent, M1/M2/M3 (§2.1). `Bit[71]`, `Bit[72]` | 52→51 | `data/scripts/room-052-circus-gr/obj-0621-circus-tent.txt [000F]` to `[0014]` |
| 13 | 51 | **Verb clicks:** 7 (Use), then inventory verb **200** (slot 0 = meat, so `Var[134]` = pot). The same step carries choose `["nibboB"]` for menu M4 | **until** `{"var": 32, "eq": 200}`, i.e. `VAR_VERB_SCRIPT` = 200 (v5 index 32: `third_party/scummvm/engines/scumm/vars.cpp`, `setupScummVars`). Written at `local-207 [0D4C]`; input comes back on at `[0D59]` | "will work as a helmet!", `Bit[103] = 1`. Cannon cutscene; pot → owner 0. Then M4. **`Var[195] = 478`**, 488 owned by ego. Ego walks out on his own | 51→52 | `data/scripts/room-051-circus-te/local-200.txt [00E7]` to `[0126]`, `[0021]` to `[008B]`; `data/scripts/room-051-circus-te/local-207.txt [0F40]`, `[110E]`, `[114D]` |

Row 13 needs the `until` gate. Step 12 finishes on the first sentence-idle frame after "Of course", and that can come while the skippable fuss (`[0BCB]` to `[0C65]`) is still playing and input still goes to global 14. Clicks sent then would be wasted. `VAR_VERB_SCRIPT` equals 200 only during the helmet wait: it is 209 at `[0005]`, 14 at `[03B2]`/`[0F08]`, and restored at `[113E]`.

Row 13 also carries `"nibboB"` because the Bobbin menu M4 opens while the click step is still active. A separate step would fail with `unexpected_choice`.

Totals:
- **12 sentences**, 13 with step 3a.
- **2 verb clicks.**
- **4 dialogue choices** across 2 steps (rows 12 and 13).
- **10 room transitions**, plus the forced title room 96 and the forced LeChuck cutscene rooms.

Variants worth giving the planner:

- **Step 1 alternative:** `Walk to` 487 → map, then `Walk to` 917 "village" → dock (`data/scripts/room-085-melee/obj-0917-village.txt [0046]`). This skips title room 96 but costs one more transition and the map walk.
  - Steps 9 and 10 have no alternative. The dock connects to the map only through the lookout (cliffside 426 → path 487). The only other dock exits are archway 427 (low street) and the bar door (exit-list grep of rooms 32 to 36).
- **Circus without the pot first:** answer M3 with "Er… no", leave, fetch the pot, return. Then "Hello again" jumps to M3 (`local-207 [042B]`). This always costs an extra map round trip.
- **Using the money object as the "item before the pot":** if the minutes deal (+2) is done before the circus, 488 is the first visible item (§1, inferred). The meat is then not needed for the helmet click; click verb 200 + the pot's slot − 1.

After step 14, the purchases from §4:

| Step | Input | Transition |
|---|---|---|
| a | `Walk to` 622 "path" | 52→85 |
| b | `Walk to` 917 "village" | 85→33 |
| c | `Walk to` 427 "archway" | 33→35 |
| d | `Talk to` 441, choose `["barber", "swell gift"]` | −100 |
| e | `Walk to` 451 "archway" | 35→34 |
| f | `Open` 437, then `Walk to` 437 | 34→30 |
| g | `Pick up` 396, then `Walk to` 387, choose `["shovel", "I want it", (+"browse")]` | −75 |
| h | `Walk to` 387 | 30→34 |

Room links are in rooms.md and treasure.md.

---

## 7. Flags and variables in this area

| Var/Bit | Meaning | Set at |
|---|---|---|
| `Var[195]` | pieces of eight | obj 488 verb 250 `[008B]` |
| `Bit[71]` / `Bit[72]` | circus: met the brothers / helmet question asked | `data/scripts/room-051-circus-te/local-207.txt [088D]` / `[0AA8]` |
| `Bit[103]` | helmet accepted, so the payout follows and the tent is closed afterwards | `data/scripts/room-051-circus-te/local-200.txt [008B]` |
| `Bit[98]` / `Bit[99]` / `Bit[312]` | sword / shovel / mints paid | `data/scripts/room-030-store/local-211.txt [0661]` / `[09CE]` / `[1BB1]` |
| `Bit[65]`, `Bit[16]`, `Bit[475]` | map bought / met the map seller / heard the price | `data/scripts/room-035-low-stree/local-218.txt [093E]`, `[054F]`, `[06AA]` |
| `Bit[129]` | asked the low-street pirates about maps | `data/scripts/room-035-low-stree/local-216.txt [0564]` |
| `Bit[420]` | Otis's breath noticed, which unlocks the mints topic | `data/scripts/room-031-jail/obj-0405-prisoner.txt [001A]` |
| `Bit[395]` / `Bit[446]` | Part One title shown / LeChuck cutscene shown | `data/scripts/room-038-lookout/obj-0486-stairs.txt [004F]` / `data/scripts/room-028-bar/obj-0315-door.txt [0085]` |
| `Var[101]` | previous room, which drives the cook's state on bar entry | `data/scripts/global/script-007.txt [0000]` |
| `Var[272]` | seagull attempt counter | `data/scripts/room-041-kitchen/local-206.txt [0080]` |
| `Bit[100]` / `Bit[424]` | gull gone and plank stuck / gull has landed | `data/scripts/room-041-kitchen/local-211.txt [0007]` / `local-204.txt [001E]` |
| state(316) | 1 = kitchen door open = cook out in the bar | `data/scripts/room-028-bar/local-216.txt [001B]`, `[0080]`; `local-205.txt [0040]` |

---

## 8. Open questions

1. **Circus helmet input (B1).** Will the rules and the bridge allow the two verb clicks (Use, then inventory slot 200+k)? If not, Part I's treasure goal cannot be reached glitchlessly under the current rules. **Highest priority.**
2. **Cook gate (B2).** Should the model follow click semantics (cook X ≤ 310) or the weaker sentence semantics? Either way, C3 needs an actor-room/actor-x condition, or the bridge has to send door-316 clicks through `VAR_VERB_SCRIPT`.
3. **`Var[116]` at circus time.** It is irrelevant if the planner never emits `Give pot → actor`. It would decide whether a mistaken Give destroys the pot (B1). Not traced further.
4. **The money object holds `_inventory[0]`.** Inferred from engine code plus the lookout entry. Confirm with `state-start.json` before relying on it for the slot arithmetic.
5. **Timings are unknown.** Cook walk times, cutscene lengths with and without skipping, and the store's random parting line all need the tick trace. Script-level delays are exact: 211 is 1800 to 3000 jiffies and 212 is 600.
6. **Fish (B4).** Can any legal sentence trigger the plank? If not, the troll bridge needs a different solution, or the troll is not on the treasure, idol **or circus** routes.
   - From raw CDHD, the forest fork (walk 72,87) and the circus clearing (walk 133,87) look to be on the lookout side of the bridge. This is inferred only.
   - The clearing's walk X is 133, exactly the bridge script's `X < 133` threshold (`data/scripts/room-085-melee/obj-0914-bridge.txt [0011]`).
   - **This is the one inferred link that could invalidate §6.** If the clearing is across the bridge, the circus needs the fish, and with B4 the money chain is blocked.
   - Check with the map walkboxes (BOXD/BOXM) or the first trace. See rooms.md and treasure.md.
7. **Is the map required?** The forest gate also opens with script 67 or `Bit[401]` (§4.2). See treasure.md.
8. **Are the mints required?** Only if the idol route needs Otis's cake (idol.md).
9. **Whether the map seller and the low-street pirates are always present** was not analysed. Local scripts 47 and 209 in room 35 are restarted around their dialogues.
10. **Map encounters (§2.3)** begin on the third map entry and can interrupt a standing Guybrush. Their determinism and their effect on the route belong to rooms.md.
