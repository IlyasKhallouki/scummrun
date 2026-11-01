# Part I planning model (treasure + idol)

This note describes the hand-written model in `pddl/part1/`:

- `domain.pddl` and `problem.pddl`: Fast Downward, `astar(lmcut())`;
- `steps.toml` (contract C8): every ground action → plan-player steps;
- `segment.toml` (contract C9): start, goal and inventory layout.

`tests/unit/test_pddl_model.py` checks all four.

Citations use `data/scripts/<file> [XXXX]` (descumm offsets). The `; src:` lines in the PDDL are the primary citations. This note repeats only the main ones. Facts that come from the analysis notes are linked to the note.

Sources besides the analysis notes:

- the replays in `out/runs/` (§7);
- the adversarial audit of this model, workflow `wf_e6db9470-553`. Its confirmed findings are applied here (§13).

§14 covers the additions for the time objective (`docs/plan.md` Phase 8, Task 8.1b): the alternatives re-admitted, the one-shot splits, and what stays excluded and why.

## 1. Conventions

- **PDDL subset.** `:strips :typing :negative-preconditions :action-costs`. There are no conditional effects, quantifiers, disjunctions or derived predicates (`docs/research/fast-downward.md` §7).
  - Fast Downward turns the delete of a fact that the precondition does not fix into a conditional effect, which lmcut rejects (exit 34). So every delete in the domain is of a fact the precondition requires (§4.3).
  - One exception: `steal-idol` deletes `(has manual)` and `(has lips)` without requiring them (§14.8). That is safe only because both are binary facts. For a binary variable the translator drops the effect condition and deletes the fact unconditionally. The translated task has no effect condition on any operator: 231 operators, 0 with a condition. A multi-valued fact, such as `(at ?r)`, would still need the rule above.
- **Costs.** Every action has exactly `(increase (total-cost) 1)`. The problem has `(:metric minimize (total-cost))` and `(= (total-cost) 0)`.
- **Parameters.**
  - Only the generic `walk ?from ?to` has parameters. Its ground instances are the static `(link a b)` facts, keyed `"walk a b"` in `steps.toml`.
  - Every other action is 0-ary, so its name is its only ground instance and its `steps.toml` key.
  - Variants are therefore spelled out by name, for example `pick-up-pot-not-first-meat`.
- **Naming rule.** An action changes `(at …)` if and only if its name starts with `walk`.
  - Transitions are counted by this rule, so `src/speedrun/stats.py` can count them from the plan alone.
  - Three kinds of `walk-*` action are a player sentence whose script then moves ego: the helmet, Fester, and the idol pickup with its `-last` twin (§9).
  - `walk-to-kitchen-door-provoking-cook` is an Open whose sentence walk crosses the bar halves (§14.3).
- **Objects in `steps.toml`.**
  - References are `{room, name[, id]}` with the object-dump names.
  - An inventory object is referenced in the room where it starts: pot 41, meat 41, petal 58, mints 30, repellent 53, cake 31, shovel 30.
  - `id` is given wherever a name repeats in its room: doors in 28, 34, 41 and 53, archways in 34 and 35, the forest paths, and the docks 904 and 905 in 83.

## 2. Room nodes

There are 39 nodes, all domain constants: 18 outside the forest and 21 forest nodes (F207 is split in two).

| node | engine room | why it is a separate node |
|---|---|---|
| `dock` | 33 | start |
| `lookout` | 38 | |
| `melee-map` | 85 | |
| `low-street` | 35 | |
| `high-street-town`, `high-street-mansion` | 34 | The entry script pins the camera to one half (`room-034-high-stre/entry.txt [0046]`). The halves are joined only by the cutscene walks 436 and 435 (rooms.md §2.3). |
| `bar-left`, `bar-right` | 28 | The camera halves (rooms.md §2.4); the curtain 323 crosses between them, and door 315 is an exit from both (T25, T25a). Requiring `bar-right` before the kitchen also shortens the kitchen-door race (§7.1). |
| `kitchen` | 41 | |
| `store` | 30 | |
| `jail` | 31 | |
| `mansion` | 36 | |
| `foyer` | 53 | |
| `clearing` | 52 | |
| `tent` | 51 | |
| `treasure-site` | 64 | |
| `underwater` | 42 | |
| `cu-dock` | 83 | Ego is left here after the idol (the Elaine scene). |
| `f201` to `f220` | 58 | Forest pseudo-rooms; `VAR_ROOM` (var 4) holds 201 to 220. |
| `f207a`, `f207b` | 58 | 207 entered from 212 has only path 687 touchable; entered otherwise, only 685 and 688 (`room-058-damnfores/entry.txt [0473]`). |

**Not modelled**, because no treasure or idol route can use them:

- 29 voodoo shop;
- 32 alley;
- 78 church;
- 48 crossing, and 37 beyond it;
- 57 troll bridge, plus 59 and 43 east of it. Paying the troll needs the fish, which is believed unobtainable with sentences (money.md B4), and the bridge is a dead end while unpaid.
- 61 Sword Master. It is reachable from F209 after the sign, but it is a dead end for these trials.

**Links.** There are 67 static links: 23 outside the forest and 44 inside it.

- Every link is in rooms.md §5.2 and carries a trailing `; src:` citation to the object script that changes the room.
- The forest links were generated from the switch tables in `room-058-damnfores/obj-0685/0687/0688-path.txt`. Only paths the entry script draws in that pseudo-room are included (`entry.txt [01D8]` to `[0A2C]`). They match rooms.md §2.9 and treasure.md §4.3 edge for edge.
  - Path 686 has no switch table of its own. It forwards every verb to 685 (`obj-0686-path.txt [0010]`), so the generation missed it. It is drawn only at 218 and 220 (§14.8).
- Four kinds of exit are left out of the link set and are dedicated actions instead:
  - the gated exits of 215 (map, Bit[401], or the guide; §14.3);
  - the store exit, which is guarded by `shovel-unpaid` and by 387 being open;
  - the bar exit through 315, which is split on Bit[446] (the first exit plays the LeChuck cutscene; §14.1). This removed the links `bar-left → dock` and `bar-right → dock`.
  - a second exit object for an existing link: 686 at F218 and F220, and dock 905 at 83 (§14.8). A link key `walk a b` names only its two rooms, so a second object for the same pair needs an action of its own.

**Off-screen exits.** The off-screen rule (`rules/glitchless.md`, "Camera visibility") allows a sentence on an off-screen exit only when the walk it starts brings the exit on screen. Every link is still a cited rooms.md §5 edge.

- **Modelled:** `walk-out-of-bar-from-right` and its first-exit twin, which are Walk to 315 from the right half (rooms.md T25a).
  - As ego walks left past x 320, the bar camera switches to the left half and 315 comes on screen (`room-028-bar/local-201.txt [0000]`).
  - It replaces the curtain walk plus Walk to 315, so it costs one action fewer.
- **Modelled:** `walk-to-kitchen-door-provoking-cook`, which is Open 316 from the left half (§14.3). The walk to 316 crosses x 320, and the camera pans to the right half (`local-201.txt [0012]`). The cook race of §7.1 does not apply: the provoke is only possible while the cook is in the kitchen.
- **Allowed by the rule, not modelled:** Walk to 316 from `bar-left`. The camera would switch as ego crosses, but the longer walk widens the cook's door race (§7.1).
- **Excluded by the rule:** Walk to 431 from `high-street-town`. The town half's camera never shows 431 (rooms.md §7), so a player can reach the mansion only through 436.

## 3. Predicates

| predicate | meaning (engine state) |
|---|---|
| `(at ?r)` | ego's node |
| `(link ?a ?b)` | static unconditional exit |
| `(has ?i)` | ego owns item `?i` and it is displayed. Items: meat 566, pot 567, petal 689, treasure-map 442, shovel 396 (paid), mints 395, repellent 640, manual 641, lips 642, staple-remover 643, cake 420, foyer-idol 635. |
| `(pot-guarded-by ?i)` | `?i` was picked up before the pot, so its inventory cell precedes the pot's (§4.3) |
| `(meat-in-kitchen)`, `(pot-in-kitchen)` | 566 / 567 owner 15 in room 41 |
| `(petal-in-forest)` | 689 owner 15: the plants at 215 still give a petal |
| `(meat-drugged)` | 566 class 6 |
| `(stew-drugged)` | 574 class 6 |
| `(meat-in-stew)` | 566 owner 13 |
| `(bar-door-open)` | 428/315 state 1 (sticky, rooms.md §4) |
| `(store-door-open)` | 437/387 just opened by `open-store-door`. Only `walk-into-store` can follow, and it consumes the fact (§6). |
| `(mansion-door-open)` | 465/633 state 1 |
| `(idol-room-door-open)` | 632 state 1 |
| `(idol-room-visited)` | `Bit[481]`: 637 is touchable and 632 is locked |
| `(poodles-asleep)` | `Bit[15]` |
| `(otis-breath-known)` | `Bit[420]` |
| `(otis-breath-fresh)` | 405 class 6 cleared |
| `(cake-opened)` | 420 class 6 cleared |
| `(shovel-unpaid)` | ego owns 396 and `!Bit[99]` |
| `(circus-money)` | +478 received (`Bit[103]`) |
| `(idol-trial-done)` | `Bit[85]` |
| `(treasure-trial-done)` | `Bit[86]` |
| `(lechuck-cutscene-seen)` | `Bit[446]`: the first exit through 315 played the "Meanwhile" cutscene (§14.1) |
| `(cook-timer-fresh)` | This bar visit began from the dock, so `local-211` runs and the cook is in the kitchen. Set by `walk-into-bar`, consumed by every kitchen entry (§14.3). |
| `(cook-provoked)` | Open 316 ran `local-214`, so `local-212` brings the cook out. True only between the provoke and the kitchen walk (§14.3). |
| `(sword-master-asked)` | `Var[199]` = 1, from the pirate leaders: store topic 122 is shown (§14.3) |
| `(following-storekeeper)` | Global 67 runs: the storekeeper guides ego from the store to the forest. True until gate 685 at 215. |
| `(store-door-387-closed)` | The guiding storekeeper closed 387 behind him (`room-030-store/local-211.txt [1122]`) |
| `(forest-gate-open)` | `Bit[401]`: the gates at 215 no longer ask for the map |

**Money.**

- Var[195] starts at 0 (money.md §1), and the circus pays 478 once (`room-051-circus-te/local-207.txt [110E]`).
- The map costs 100, the shovel 75 and the mints 1, so 176 in all. One payout therefore covers every purchase, and each purchase only needs `(circus-money)`.
- Spent flags are unnecessary, because nothing can be bought twice: the map and the shovel are gated by `(has …)`, and the mints only appear while `!Bit[312]`.

## 4. Action families

### 4.1 Transitions (`walk*`, 38 schemas)

| action | sentence (verb, object) | guard | main cites |
|---|---|---|---|
| `walk ?a ?b` | (Walk to, exit object) per link | `(link ?a ?b)`, `¬store-door-open` (§6), `¬cook-provoked`, `¬following-storekeeper` (§14.3) | `global/script-002.txt [02DB]`, `[039D]`; per-link cites in `problem.pddl` |
| `walk-into-bar` | Walk to 428 | `bar-door-open`; sets `cook-timer-fresh` | `room-033-dock/obj-0428-door.txt [0029]`, `[0035]`, `room-028-bar/local-205.txt [0040]` |
| `walk-into-kitchen`, `walk-into-kitchen-after-provoking-cook` | Walk to 316 | `until` cook (§6); split on `cook-provoked`, both consume `cook-timer-fresh` (§14.1) | `room-028-bar/obj-0316-door.txt [003F]`, `local-218.txt [0017]`, `local-203.txt [001E]` |
| `walk-to-kitchen-door-provoking-cook` | Open 316 from the left half | `cook-timer-fresh` (§14.3) | `obj-0316-door.txt [0018]`/`[0021]`, `local-214.txt [004B]`, `local-201.txt [0012]` |
| `walk-out-of-bar-from-{left,right}[-meanwhile]` ×4 | Walk to 315 | split on `lechuck-cutscene-seen` (§14.1) | `room-028-bar/obj-0315-door.txt [0072]`, `[0085]`/`[008A]`, `[0090]`, `global/script-120.txt [0538]` |
| `walk-into-store` | Walk to 437 | `store-door-open` (deleted). Only `open-store-door` can come just before it (§6). | `room-034-high-stre/obj-0437-door.txt [004A]`, `[005B]` |
| `walk-out-of-store` | Walk to 387 | `¬shovel-unpaid`, `¬store-door-387-closed` | `room-030-store/local-204.txt [0031]`, `[0044]` |
| `walk-into-foyer` | Walk to 465 | `poodles-asleep`, `mansion-door-open` | `room-036-mansion-e/obj-0465-door.txt [003C]`, `entry.txt [003F]` |
| `walk-foyer-to-mansion` | Walk to 633 | `¬(has foyer-idol)`: the theft closes 633 | `room-053-foyer/obj-0633-door.txt [0068]`, `local-211.txt [018C]` |
| `walk-forest-gate-215-203`, `walk-forest-gate-215-220` | Walk to 685 or 688 at 215 | `(has treasure-map)`; set `forest-gate-open` | `room-058-damnfores/obj-0685-path.txt [004F]`/`[00C6]`, `obj-0688-path.txt [004F]`/`[006B]` |
| `walk-forest-gate-215-203-open`, `walk-forest-gate-215-220-open` | the same | `forest-gate-open` (Bit[401]) | `obj-0685-path.txt [0064]`, `obj-0688-path.txt [005B]` |
| `walk-follow-guide-to-*` ×6, `walk-forest-gate-215-203-with-guide` | the plain links' sentences, then 685 at 215 | `following-storekeeper` (§14.3) | `global/script-067.txt [000D]` to `[0128]`, `obj-0685-path.txt [005B]`/`[00C1]` |
| `walk-follow-guide-to-f215-via-686` | Walk to 686 at 218 | `following-storekeeper`; the hop of `walk-follow-guide-to-f215` (§14.8) | `global/script-067.txt [00D6]`/`[02F6]`, `obj-0686-path.txt [0010]`, `obj-0685-path.txt [0168]` |
| `walk-into-tent-with-pot` | Walk to 621, then three menus | `has pot`, `¬circus-money` | `room-052-circus-gr/obj-0621-circus-tent.txt [000F]`/`[0014]`, `local-207.txt [0213]`/`[08AB]`/`[0B49]` |
| `walk-out-of-tent-after-helmet-<g>` ×6 | `click` Use + the slot before the pot, then the Bobbin menu | `has pot`, `pot-guarded-by g`, `has g` | `room-051-circus-te/local-200.txt [0107]`/`[008B]`, `global/script-009.txt [0092]`, `local-207.txt [110E]`/`[114D]` |
| `walk-past-fester-to-underwater` | Open 633 while owning 635 | `has foyer-idol` | `room-053-foyer/obj-0633-door.txt [0018]`, `local-217.txt [0391]`, `global/script-065.txt [023A]` |
| `walk-up-ladder-taking-idol`, `walk-up-ladder-taking-idol-last` | Pick up 578 | at `underwater`; split on `treasure-trial-done` (§14.1) | `room-042-underwate/local-203.txt [0024]`, `local-200.txt [0041]`/`[0091]`, `global/script-071.txt [008D]` |
| `walk-f218-f215-via-686`, `walk-f220-f210-via-686` | Walk to 686 | the generic walk's guards; the transition is that of `walk f218 f215` / `walk f220 f210` (§14.8) | `room-058-damnfores/entry.txt [08ED]`/`[09C1]`, `obj-0686-path.txt [0010]`, `obj-0685-path.txt [0168]`/`[018C]` |
| `walk-cu-dock-dock-via-905` | Walk to 905 | the generic walk's guards; the transition is that of `walk cu-dock dock`, landing at x 566 (§14.8) | `room-083-cu-dock/obj-0905-dock.txt [0010]`/`[0019]`, `room-033-dock/local-201.txt [0042]`/`[0070]` |

### 4.2 Doors (6)

| action | sentence | cite |
|---|---|---|
| `open-bar-door` | Open 428 | `room-033-dock/obj-0428-door.txt [0015]`, `global/script-025.txt [0024]` |
| `open-store-door` | Open 437, immediately followed by `walk-into-store` (§6) | `room-034-high-stre/obj-0437-door.txt [0018]` |
| `open-store-door-from-inside` | Open 387, after the guiding storekeeper closed it (§14.3) | `room-030-store/obj-0387-door.txt [004E]`, `local-211.txt [1122]` |
| `open-mansion-door` | Open 465 (needs the poodles asleep: 465 is untouchable before) | `room-036-mansion-e/obj-0465-door.txt [0018]`, `local-201.txt [00CE]` |
| `open-idol-room-door` | Open 632 | `room-053-foyer/obj-0632-door.txt [0028]` |
| `provoke-cook` | Open 316 while the cook is in the kitchen: refused, but he comes out 600 jiffies later (§14.3) | `room-028-bar/obj-0316-door.txt [0018]`/`[0021]`, `local-214.txt [004B]`, `local-212.txt [0020]` |

### 4.3 Kitchen and the circus inventory quirk

**The quirk.** `room-051-circus-te/local-200.txt [0107]` reads `Var[134+k]` for slot verb `200+k`. The inventory display fills `Var[133+k]` (`global/script-009.txt [0092]`). So the helmet works only by clicking the slot just **before** the pot, which requires that the pot is not the first displayed item (input-scripts.md §6).

**The engine's inventory order** (ScummVM `object.cpp`):

- New items take the first free cell of `_inventory[]`.
- Giving an item to owner 0 removes its cell, and the cells after it close up.
- Giving an item to another owner keeps the cell but hides the item.
- The money object 488 holds a cell from the lookout opening, but stays hidden (owner 14) while Var[195] is 0, and the circus is the only money source modelled.

**The encoding.**

- `pick-up-pot-not-first-<g>` records `(pot-guarded-by g)` for an item `g` that is held when the pot is picked up. `use-meat-with-pot` records the meat as the guard.
  - "Use meat with pot" with both items on the table queues `(9, meat)` and then `(9, pot)` through the class-7 auto pick-up (`global/script-002.txt [0229]`, `[0251]`). The meat's Use then falls back to script 3, a refusal line (`room-041-kitchen/obj-0566-hunk-of-meat.txt [00BD]`).
- The helmet requires both `(pot-guarded-by g)` and `(has g)`. Every way an item leaves (owner 0) or is hidden (owner 13/14) also clears its `(has)`.
  - The meat in the stew keeps its cell (owner 13) and returns to it (`room-041-kitchen/local-214.txt [0007]`, `setOwnerOf`). So `(has meat)` is exact for it.
  - A petal that was put back in the forest and then picked up again would land after the pot. So `pick-up-petal` requires `¬(pot-guarded-by petal)`.
  - With this design, removal actions never need to delete a guard fact. Deleting one would be a conditional effect (§1).
- Possible guards are the meat, the petal and the four idol-room items. These are every item obtainable without money, and money needs the pot. The planner chooses the guard.
- `pick-up-pot-first` (no guard item held) exists, but it is a dead end: no slot precedes the pot, so the helmet is impossible.

**Scrolling** cannot interfere. Before the payout at most 7 items are displayed: meat, pot, petal and 4 idol-room items. The 8 slots never scroll, and the pot is at most in slot 6, inside local 200's range of 200 to 205 for the clicked slot.

**Kitchen actions:**

| action | sentence | cites |
|---|---|---|
| `pick-up-meat` | Pick up 566 | `obj-0566-hunk-of-meat.txt [0035]`/`[0041]` |
| `pick-up-pot-first`, `pick-up-pot-not-first-<g>` ×6 | Pick up 567 | `obj-0567-pot.txt [002A]` |
| `use-meat-with-pot` | Use 566 with 567 | above |
| `use-meat-on-table-with-petal` | Use 566 (on the table) with 689 | `script-002 [0229]`, `obj-0566 [007E]`, `global/script-182.txt [0017]` |
| `drug-meat-with-petal` | Use 566 with 689, both held | `obj-0566 [007E]`, `script-182 [0017]` |
| `put-petal-in-stew` | Use 689 with 574 | `obj-0689-yellow-petal.txt [0059]`, `local-213.txt [0042]`/`[004A]` |
| `put-meat-in-stew` | Use 566 with 574 | `obj-0566 [0092]`, `local-213.txt [00B8]` |
| `pick-up-stewed-meat-drugged`, `pick-up-stewed-meat-plain` | Pick up 574 | `obj-0574-pot-o-stew.txt [0042]`, `local-214.txt [0007]`/`[005A]` |

`drug-meat-with-petal` and `open-cake` act on inventory items only, so they work in any room except two. On the map every click becomes Walk to (`room-085-melee/local-201.txt [0035]`), and in the tent, during the helmet wait, Use plus a slot click is itself the helmet trigger. Their steps omit `room`. Both also require `¬store-door-open`, so they cannot fall between the store door's Open and the walk in (§6).

### 4.4 Forest

`pick-up-petal` is Pick up 678 at `VAR_ROOM == 215` (`room-058-damnfores/obj-0678-plants.txt [007F]`, `[0086]`, `[0092]`).

### 4.5 Purchases

| action | sentences | choices |
|---|---|---|
| `buy-map` | Talk to 441 | `barber`, `swell gift` |
| `pick-up-shovel` | Pick up 396 | |
| `pay-for-shovel` | Walk to 387 | `shovel`, `I want it` |
| `pay-for-shovel-files-topic` | Walk to 387 | `shovel`, `I want it`, `browse` |
| `pay-for-shovel-and-mints` | Walk to 387 | `shovel`, `I want it`, `breath mint` |
| `pay-for-shovel-and-mints-files-topic` | Walk to 387 | `shovel`, `I want it`, `breath mint`, `browse` |
| `pay-for-shovel-ask-guide` | Walk to 387 | `shovel`, `I want it`, `Sword Master` (§14.3) |
| `pay-for-shovel-and-mints-ask-guide` | Walk to 387 | `shovel`, `I want it`, `breath mint`, `Sword Master` (§14.3) |

The four plain pay variants require `¬sword-master-asked`: once the pirate leaders were asked, topic 122 stays in the menu after the purchases, and their lists would not empty it. Topic 122 ends the dialogue by itself (`local-211.txt [1138]`), so the guide variants need no `browse`.

**Paying through the door.** Walking to 387 with an unpaid shovel always ends in the store menu (`room-030-store/local-204.txt [042B]`). If the storekeeper is away, which happens on 1 in 4 store entries (`entry.txt [002B]`), he walks back in first (`local-204 [0053]` to `[0077]`). So the same steps work in both cases (treasure.md §2.2, money.md B3).

**Why four pay variants.** After each purchase the menu is rebuilt. It ends by itself only when no topic but "browse" is left (`local-211.txt [03EB]`). Leftover `choose` entries would time out, so each variant's list empties the menu exactly. The visible topics depend on two things:

- `Bit[420] ∧ ¬Bit[312]`: the breath-mint topic (`[0251]`);
- owning 640: the "files" topic (`[032D]`).

The other topics stay hidden on every modelled route:

- the sword topic, because the sword is never picked up;
- the note topic, because `Bit[28]` is never set, and the sword variant of topic 122, because `Bit[98]` is never set. The Sword Master variant needs `Var[199]`, which only `talk-to-pirate-leaders` sets; the guide variants handle it (above);
- the rat repellent topic, because `Bit[95]` is never set: Otis's "file" line is never chosen.

### 4.6 Idol chain

| action | sentence | choices | cites |
|---|---|---|---|
| `give-meat-to-poodles` | Give 566 to 467 | | `global/script-002.txt [009A]`, `obj-0467 [00D5]`, `local-201.txt [002F]`/`[0079]`/`[0087]` |
| `enter-idol-room` | Walk to 632 (open) | | `obj-0632-door.txt [0024]`, `local-210.txt [0002]`/`[01FB]`/`[0267]`/`[02B8]`/`[022F]` |
| `talk-to-prisoner` | Talk to 405 | (no menu) | `obj-0405-prisoner.txt [001A]`; `local-202.txt [0050]` (405 still has class 6) → `[18DE]` to `[19C0]` halitosis, ends at `[19C4]` |
| `give-meat-to-prisoner`, `give-repellent-to-prisoner-before-mints` | Give 566 or 640 to 405 | (no menu) | the refusal `local-203.txt [029D]`, then `[0351]` Bit[420]; nothing changes owner (§14.3) |
| `give-mints-to-prisoner` | Give 395 to 405 | `stiff upper lip` | `local-203.txt [00BF]`/`[0112]`; `local-202.txt [0546]` (choice 127), `[1381]` to `[13A9]` → goto `[19C4]`, the dialogue ends |
| `give-repellent-to-prisoner` | Give 640 to 405 | | `local-203.txt [011F]`/`[012A]`/`[01E0]` |
| `open-cake` | Open 420 | | `obj-0420-cake.txt [0056]`/`[005F]` |
| `steal-idol` | Walk to 637 | `could have it`, `Uh`, `Um`, `Blfft` | `obj-0637-gaping-hole.txt [0018]`/`[0021]`, `local-211.txt [016C]`/`[0174]`, `local-212.txt [02BE]`, `global/script-119.txt [01BB]` |

`steal-idol` requires only the foyer, the idol-room visit and the opened cake. Walk to 637 tests nothing but the file 420 (`obj-0637-gaping-hole.txt [000C]`, `[0018]`). The `(has manual) (has lips)` guard it used to carry was not backed by any script, and it was dropped (§14.8).

Then `walk-past-fester-to-underwater` (choice `Buzz off`) and `walk-up-ladder-taking-idol` follow (§4.1).

### 4.7 Treasure

`dig-treasure` is Use 396 with 749 in room 64. The chain is `room-030-store/obj-0396-shovel.txt [0072]` → `room-064-treasure/obj-0749-x.txt [006B]`/`[00A6]` → `local-200.txt [0214]` → `global/script-071.txt [008D]`.

### 4.8 Dialogue choices

Each substring is matched case-insensitively against the visible lines of one menu. Each is unique there and avoids the `^` glyph and escape placeholders.

| substring | why it is unique |
|---|---|
| `ahem` | The circus's first menu accepts any line. |
| `I'll do it` | Unique in its menu. |
| `Of course` | `have a helmet` would match both lines of that menu (money.md B5). |
| `nibboB` | The reversed menu. |
| `barber` | Unique. The "file" line may also be visible. |
| `swell gift` | Unique. |
| `shovel` | Unique. |
| `I want it` | Not a substring of "I don't want it." |
| `breath mint` | Unique. |
| `browse` | Unique. |
| `stiff upper lip` | Unique. The visible lines are "Who are you?", the file question and this one. |
| `could have it` | Unique in Fester's menu. All lines only set Var[273]. |
| `Uh` | Unique among "Uh", "Gee", "Well", "Gosh". |
| `Um` | Not `Er`, which also matches "Jeepers". |
| `Blfft` | Unique. |
| `Buzz off` | Unique. |
| `Sword Master` | Unique in the store menu: the topic 122 line "I'm looking for the Sword Master of Mêlée Island™." (`local-211.txt [0110]`). |
| `be a pirate` | The leaders' first menu: "I mean to kill you all!", "I want to be a pirate.", "I want to be a fireman." |
| `mastering the sword` | Not a substring of "Tell me more about mastering the art of thievery." |
| `running along` | Unique: "I'll just be running along now." |

**Skips and menus.** The `Uh`, `Um`, `Blfft` (steal-idol) and `Buzz off` (Fester) menus lie inside level-0 override regions: `global/script-119.txt [0000]` → `[08A1]` and `room-053-foyer/local-217.txt [000F]` → `[034B]`. With cutscene skips on, the default for every v1 run, the Esc lands first and those menus never show (`docs/part1/skips.md` §4.3). Their templates therefore list them as `override_choose`, which the bridge expects only when the cutscene plays: skips off, or a `no_skip` step (`docs/plan.md` C4). The same compiled plan replays with and without skips (`--no-skips`), and both are tested (`tests/integration/test_skip_safety.py`).

## 5. Alternatives

**Modelled; the planner chooses:**

- **How the meat is drugged:**
  - directly, held (`drug-meat-with-petal`);
  - directly from the table (`use-meat-on-table-with-petal`), which needs one sentence fewer than Pick up plus Use;
  - through the stew (`put-petal-in-stew`, `put-meat-in-stew`, `pick-up-stewed-meat-*`).
- **How the meat and pot are picked up:**
  - separately, in either order;
  - together with `use-meat-with-pot`.
- **Which held item guards the pot** (six variants).
- **When the mints are bought.** Together with the shovel, with or without the files topic, in any order relative to the first idol-room visit.
- **Which forest gate** is passed at 215 (to 203 or to 220), plus the whole forest graph including F207a/b.
- **Which exit object** makes a transition that has two: 685 or 686 at F218 (also while following the storekeeper) and at F220, and dock 904 or 905 at 83 (§14.8). Under unit costs they tie.
- **Added for the time objective** (§14.3). Under unit costs these cost more actions or tie, and the planner chooses once action costs are measured ticks:
  - **provoking the cook** (Open 316 while he is in the kitchen), from either bar half;
  - **how Bit[420] is learnt**: Talk to Otis, or a give he refuses (the meat, or the repellent before the mints);
  - **the storekeeper as guide** instead of the map, unlocked by the pirate leaders' first meeting;
  - **the order of the trials**, which was always free: the idol taken last skips the Elaine scene (§14.1).

**Excluded, with reasons.** "Costs more actions" is no longer a reason: the time objective weighs actions by their measured ticks. Every row below is excluded because it is infeasible, against the rules, or never faster in ticks, argued per component. §14.4 has the full argument for each.

| alternative | reason |
|---|---|
| Talk to the storekeeper to open his menu (mints alone, or the shovel) | Replay feasibility. When he is away, 394 is untouchable and Talk to cannot be clicked (`room-030-store/entry.txt [0054]` to `[0057]`). That happens at random on 1 in 4 entries, and the plan cannot branch. The door path (§4.5) works either way. Consequence: the mints are always bought together with the shovel, so `otis-breath-known` must come first. |
| Ring the bell 399 | Replay feasibility. It is only touchable while he is away, so it is random. It also closes 387 (`local-207.txt [00FD]`). |
| Use meat with poodles | Never faster, and riskier. It runs the same walk to 467 and the same verb 80 (`obj-0566 [0098]`/`[00A2]`), so it takes the same ticks as Give. Its reach check is ≤ 16 against locked boxes (`global/script-002.txt [02EB]`); Give's is ≤ 32 (`[0093]`). |
| Use pot with meat (both on the table) | Never faster. It is the same two auto pick-ups as `use-meat-with-pot` in the other order (`global/script-002.txt [0229]`/`[0251]`), so it takes the same ticks, but it puts the pot first. |
| The minutes deal (+2) | Never faster. It is a whole extra dialogue whose only goal effect is a pot guard, which the planner already has for free. It would also add two items before the payout and break the no-scrolling bound of §4.3. |
| Entering the circus without the pot, answering "Er… no", returning later | Never faster. It adds the refusal lines, a walk-out, a map round trip and "Hello again", and removes nothing (§14.4). |
| Re-reading the map (Look at or Use 442) | No goal effect (treasure.md §3); its chart only adds a click wait (input-scripts.md §1.3). |
| `pick-up-stewed-meat` with no stew involved, fish routes | No goal effect: the dogs refuse fish (idol.md §2.1). |
| Walk to 316 from `bar-left` (straight into the kitchen) | Replay risk: the longer walk widens the cook's door race (§7.1, §10). |
| Unlocking the guide with the paid sword (`Bit[98]`) instead of the pirate leaders | Deferred, not infeasible. Owning the sword changes two later actions, which would need further splits (§14.4). |
| Give pot to a Fettucini brother | A trap: it can lose the pot (money.md B1). It is never modelled. |
| Boot params; anything that needs coordinates (floor clicks, the fish plank) | Banned (`rules/glitchless.md`). |

## 6. One sentence per action, and waits

**Accounting.** One player sentence is one action. No template pushes a sentence beyond its own action's, with one documented exception: the circus tent's second walk (§12).

`test_no_consecutive_duplicate_sentences` checks two things:

- no template pushes the same sentence twice in a row (the circus action is allow-listed);
- no action's last sentence equals the first sentence of an action it enables.

`test_optimal_plan_has_no_consecutive_duplicate_sentences` checks the compiled optimal plan. It allows one legitimate repeat: pay-for-shovel then `walk-out-of-store` both push Walk to 387. The first walk ends in the store menu because the shovel is unpaid; the second walk leaves (`room-030-store/local-204.txt [0031]`).

**Removed defensive steps** (audit `wf_e6db9470-553`, §13):

| where | removed step | why it was never needed |
|---|---|---|
| `walk kitchen bar-right` | `Open 570` before `Walk to 570` | 570 is always open while ego is in the kitchen, for two reasons:<ul><li>**316 open implies 570 open.** Only scripts 25/26 on the pair 316/570 change 570 (`room-028-bar/obj-0316-door.txt [0027]`/`[0031]`, `room-041-kitchen/obj-0570-door.txt [0018]`/`[0022]`). `local-205.txt [0040]` resets 316 alone. Ego entered through an open 316. The provoke's `local-214` opens and closes 316 alone (`[0005]`, `[003B]`: script 25/26 with no partner), but inside its own cutscene; the walk-in waits for the cook, whose `local-216 [001B]` opens the pair.</li><li>**The cook cannot close it behind ego.** His script local-216 is frozen from the first frame of the walk-in cutscene. local-218 opens with `cutscene([2])`, whose start script ends in `freezeScripts(127)` (`global/script-018.txt [0070]`), and local-216 is started without the freeze-resistant flag (`local-211.txt [0039]`, `local-205.txt [003A]`). The room change then kills it (`room.cpp` `startScene` → `killScriptsAndResources`).</li></ul>So the Open could never have an effect. It cost 66 ticks per run, minus the 42 ticks of walking it did for the next step. |
| `walk-into-store` | `Open 437` before `Walk to 437` | It repeated `open-store-door`'s own sentence, so one action pushed two sentences. The domain now makes `open-store-door` immediately precede `walk-into-store`: the generic `walk`, `drug-meat-with-petal` and `open-cake` are the only other actions possible in `high-street-town`, and all three require `¬store-door-open` (`test_store_door_open_is_used_at_once`). In an adjacent plan the duplicate Open removed no risk: it covered the frames between the two Opens, but left an equally long window between itself and the Walk (§7.12). It cost 24 ticks. |

**Waits:**

| where | wait | why |
|---|---|---|
| `walk-into-kitchen`, `walk-into-kitchen-after-provoking-cook` | `until [{actor_room = 6, eq = 28}, {actor_x = 6, le = 310}, {state = 316, eq = 1}]` | A real click on 316 is refused while the cook stands in 28 at x > 310 (`local-203.txt [001E]`). The door must also be open, which the cook does himself when he comes out (`local-216.txt [001B]`). Every cook target is at x ≤ 285 (money.md B2), so every trip opens this window. |
| `provoke-cook`, `walk-to-kitchen-door-provoking-cook` | `until [{not = {actor_room = 6, eq = 28}}]` | Click equivalence: with the cook out of room 28, a click on 316 runs the door's verb directly (`local-203.txt [003A]` to `[004A]`), as the pushed sentence does. It holds at once: the cook stays in the kitchen at least 1800 jiffies after bar entry (`local-211.txt [000D]`), and the provoke comes a few hundred ticks after it. |
| helmet | `until [{var = 32, eq = 200}]` | The tent's input script is 200 only during the helmet wait (`local-207.txt [0D4C]`). The entry step may finish during the skippable fuss. |
| first step in 33, 52, 53 and 42, and in 83 | `until [{room = R}]` | These rooms can follow a script-driven room change that may outlast the previous step's last idle frame: the first bar exit's LeChuck cutscene (`global/script-120.txt [0538]`), the tent walk-out, the room-23 close-up (`global/script-119.txt [07C2]`), the Fester → 83 → 42 chain and the Elaine scene. No waiting `until` is put on map steps. |
| forest steps | no `room`; `until [{var = 4, eq = <pseudo-room>}]` | In the forest `_currentRoom` is the pseudo-room 201 to 220 (`third_party/scummvm/engines/scumm/room.cpp`, `startScene`: `_currentRoom = room`, and only `_roomResource` is mapped to 58). A `room = 58` check would always fail. |

## 7. Fidelity gaps and replay risks

These come from `rules/glitchless.md` and the analysis notes.

**Replays.**

- The pre-audit plan (cost 67, 70 compiled steps) replayed to the goal on seed 1 in `out/runs/20261004T201241Z-run` (106892 ticks). Seed 2 also passed in `tests/integration/test_determinism.py`.
- The current plan (§8) replays to the goal on seeds 1 to 5:
  - seed 1: `out/runs/20261004T205705Z-run`;
  - seed 2: the integration tests;
  - seeds 3, 4 and 5: `out/runs/20261004T205848Z-run`, `…205851Z-run` and `…205854Z-run`.

- The §14 plan replays to the goal on seed 1: `out/runs/20261004T220733Z-run` (§8).
- A hand-written 70-action plan that uses every §14 alternative replays to the goal on seed 1 (§14.5).
- The time plan with the three §14.8 exit objects substituted, plus a two-walk detour through 210, replays to the goal on seeds 1 to 3, with skips on (§14.8).

"Verified" below means these replays ran the risky step and reached the goal.

1. **The kitchen-door race** (rules "Known fidelity gap"). **Verified** on seeds 1 to 5: `walk-into-kitchen` reaches room 41 every time, 234 ticks after the push. It remains timing-dependent in principle.
   - A click on 316 freezes the cook (`local-203.txt [0030]`); a pushed sentence does not.
   - While ego walks from `bar-right` (x ≈ 330) to 316 (x 595), the cook keeps walking. He can close 316 first. The Walk to then does nothing, and the next step fails with `room_mismatch`. No template can recover from that.
   - Once the walk-in cutscene local-218 starts, the cook is frozen (§11, correction 2), and the room change kills his script. So the race window is only that walk, and 570 is always open in the kitchen (§6).
   - The cook must walk from x ≤ 310 back past the door to (660,123) before he closes it, so ego should normally win. It did on seeds 1 to 5.
2. **Map pirates: one idle frame on the 4th map entry.** The plan enters the map 4 times. The 4th entry (step 55, lookout → map, then step 56 → fork) can spawn a wandering pirate (`room-085-melee/entry.txt [009B]`, `Var[290] > 2`).
   - An encounter needs ego standing still near the pirate (`room-085-melee/local-202.txt [018A]` to `[01A2]`). On the map ego stands still for about one frame: it arrives, and the bridge pushes Walk to fork on the first idle frame.
   - Whether a pirate reaches ego in that frame is deterministic per seed. It did not happen on seeds 1 to 5 without skips.
   - **Handled since Task 8.2.** It does happen on some seeds: seed 2 without skips at talkspeed 255, and seeds 164, 166 and 196 among 1 to 267 with skips. The encounter comes as ego reaches the fork, with the pirate heading there too. `segment.toml`'s `map-pirate` interrupt answers the road menu with "Sorry to bother you. I'll be on my way." (`room-049-road/local-200.txt [0325]`). Global 114 then puts ego back on the map where he stood, and the bridge restarts the interrupted Walk to fork (`docs/plan.md`, "Plan-player semantics as implemented"; `tests/integration/test_interrupts.py`).
   - Four entries is the minimum for this goal: two for the circus round trip, then either a separate petal trip plus the treasure trip, or the treasure trip plus the exit from 64.
3. **Map idle detection.** **Verified.** Global 24 prints a blank label every frame on the map (input-scripts.md §4.6), but the bridge still sees idle frames on room 85: every map step in every replay ends in its destination room.
4. **The troll trigger on the clearing walk.** Settled; see "Settled" below.
5. **Reach checks.** Settled; see "Settled" below.
6. **The money object's cell.** **Verified.** It was inferred that 488 holds `_inventory[0]` but stays hidden while Var[195] = 0 (money.md §1). The helmet's click on the slot before the pot (offset −1) works in every replay, and the segment-start dump shows 488 with owner 14 and an empty visible inventory.
7. **Object name 441.** **Verified.** `index.json` names it `Citizen of M\x88l\x82e` (raw Mac bytes), and `steps.toml` uses `"Citizen of M\u0088l\u0082e"`. The compiler resolves it against the run's own `objects.json` dump, and `buy-map` runs in every replay.
8. **The stale input script after the poodles.** `VAR_VERB_SCRIPT` stays 203 until High Street's exit (input-scripts.md §3.2). Every plan leaves through High Street before its next dialogue, so no `choose` or `click` runs in that window. In any case, dialogue scripts install their own input script.
9. **Multi-sentence steps.** **Verified** for `use-meat-with-pot`: the step ends with both 566 and 567 added to the inventory in every replay.
   - It makes the sentence script queue three engine sentences: `(9,566)`, `(9,567)`, then `(7,566,567)`. The step must not complete between them, and it does not.
   - That relies on the bridge's idle test requiring `_sentenceNum == 0` (`docs/plan.md` "Idle"; idol.md §12).
   - `use-meat-on-table-with-petal` (two queued sentences) relies on the same test, but is not in the current plan.
10. **Click shapes, checked against DOBJ.**
    - Talk-to targets need class 13: citizen 441 has `{13}` and prisoner 405 has `{5, 6, 12, 13}`.
    - Give targets need class 5: poodles 467 have `{5, 13}`, and 405 has it too.
    - Every Use with a second object starts from an object with class 2, so the Use waits for a second object: meat 566, petal 689 and shovel 396.
    - No targeted scene object has class 32 when it is used: 465 and 637 become touchable first (`room-036-mansion-e/local-201.txt [00CE]`, `room-053-foyer/local-210.txt [022F]`).
11. **Other cosmetic or human-only gaps** (input-scripts.md §7):
   - the bar's restart of the cook walk on other clicks;
   - the mansion notice text;
   - the underwater rope tilt;
   - High Street sound 114.
12. **A citizen closing 437 between `open-store-door` and `walk-into-store`.** This is a documented, seed-dependent risk. It was not hit on seeds 1 to 5.
    - High Street citizens open and close 437 on their own (`room-034-high-stre/local-200.txt [001E]`/`[0053]` and `[00A2]`/`[00CC]`). A citizen who appears or disappears at the store door first runs script 25 and then, 4 frames later, script 26. Script 26 closes 437 and 387 regardless of who opened them.
    - Global 47 tries to start a citizen every 6 to 9 s for each free actor slot from 4 to 8 (`global/script-047.txt [000E]` to `[0035]`). It is started by `room-034-high-stre/entry.txt [006A]` when `VAR_MACHINE_SPEED` (var 6) > 0. Var 6 is 2 in the headless runs (`state-start.json`), so citizens do run.
    - `open-store-door` comes immediately before the walk (§6), so the exposure runs from Open's script 25 to the state check of Walk to 437 (`obj-0437-door.txt [004A]`). Ego is already at the door's walk point. In every replay `walk-into-store` takes 6 ticks (one frame) from the push to the room change, so the window is about one to two frames.
    - A citizen's close in that window leaves ego in room 34, and the next step fails with `room_mismatch`. That close can also come from a citizen who opened the door a few frames earlier, which turns our Open into a no-op.
    - The second Open in the old `walk-into-store` did not reduce this risk. It covered the frames between the two Opens, but left an equally long window between itself and the Walk.
13. **The provoke (§14.3).** **Verified** on seed 1 by the §14.5 plan, from the left half.
    - Its `until` (cook out of room 28) holds at once: the provoke follows the bar entry within a few hundred ticks, and the cook stays in the kitchen at least 1800 jiffies (`room-028-bar/local-211.txt [000D]`).
    - `cook-timer-fresh` stops a plan from provoking after a kitchen return, when the cook is already out and Open 316 would only open the door (`local-205.txt [0033]` to `[003A]`).
14. **Following the storekeeper (§14.3).** **Verified** on seed 1 by the §14.5 plan.
    - The margin is wide. Global 67 waits up to 1800 jiffies for ego to enter each of its rooms, 3600 in 33 and 85 (`global/script-067.txt [035A]`). The slowest hop measured was 33 → 38, at 1,308 ticks, and it falls inside room 85's 3600.
    - Map pirates do not start while 67 runs (`room-085-melee/entry.txt [0088]`).

**Settled** (audit `wf_e6db9470-553`; these were §7.4 and §7.5):

- **The troll trigger cannot fire on the clearing walk.**
  - The only position trigger on the map is local-200. It stops the sentence and loads the bridge room when ego comes within 2 px of 914's walk point (169,133) (`room-085-melee/local-200.txt [0001]` to `[0011]`).
  - That point lies on box 34, the only segment joining the western boxes to the eastern ones (rooms.md §2.5). The clearing (133,87), the lookout point, the fork and the village are all western, so no walk between them comes near it.
  - The `X < 133` test that money.md Q6 worried about sits in 914's own Walk to (`room-085-melee/obj-0914-bridge.txt [0011]`). It runs only for a sentence on 914, which no plan pushes.
  - `walk melee-map clearing` reaches room 52 on seeds 1 to 5.
- **Reach checks reduce to reachability.**
  - The sentence script's distance test (`global/script-002.txt [02EB]` to `[0317]`; Give's ≤ 32) compares where the walk ends with the object's walk point.
  - That depends only on walk-box geometry and on which boxes the plan has unlocked, not on timing or randomness. So a passing replay settles it for every seed.
  - All three inferred cases pass on seeds 1 to 5: Give meat to the poodles, Walk to the gaping hole, and Pick up idol 578 (idol.md Q3).
- **Room 52's STOP never hits the eastbound return walk, so `walk clearing melee-map` stays one sentence.**
  - Local 202 tests ego's x only on the first frame it sees ego in walkbox 7. It then waits for ego to leave the box (`room-052-circus-gr/local-202.txt [0000]` to `[0024]`).
  - Box 7 is the segment from (178,116) to (215,118) (BOXD of room 52).
  - The westbound arrival walk from (430,130) enters the box from the east at x ≈ 215 > 200, so it is stopped (§12).
  - The tent walk-out leaves ego at (78,87), far west (`room-051-circus-te/obj-0617-outside.txt [000C]` loadRoomWithEgo(621,52,78,87)).
  - So the return walk to path 622 (walk point (515,136)) enters the box from the west at x ≈ 178 ≤ 200 and passes. In every replay it takes one sentence (576 ticks).

## 8. Expected optimal plan

`run_planner(domain, problem)` gives cost 66: 66 actions, of which 48 are `walk*` and 18 are other. Fast Downward 26.6, `astar(lmcut())`, expands 16,544 states and searches for about 3 s (4,500 states and 0.8 s before the §14 additions). There were 227 ground operators after §14; §14.8 brings them to 231 and leaves the search and the plan unchanged. The plan compiles against the index-derived `ObjectIndex` to 67 plan steps: 66 actions plus the circus tent's second walk (§12). There are no defensive steps (§6).

The §14 additions left the unit-cost optimum at 66. The plan below changed only within ties:

- step 7 is the split bar exit `walk-out-of-bar-from-right-meanwhile` (the same Walk to 315 as the old `walk bar-right dock` link);
- step 24 is `give-meat-to-prisoner` instead of `talk-to-prisoner`: one sentence either way;
- `drug-meat-with-petal` moved from the jail to the town half of High Street.

```
 1 open-bar-door
 2 walk-into-bar
 3 walk bar-left bar-right
 4 walk-into-kitchen
 5 use-meat-with-pot
 6 walk kitchen bar-right
 7 walk-out-of-bar-from-right-meanwhile   ; Walk to 315 from the right half (T25a); first exit: LeChuck cutscene
 8 walk dock lookout
 9 walk lookout melee-map
10 walk melee-map clearing
11 walk-into-tent-with-pot
12 walk-out-of-tent-after-helmet-meat
13 walk clearing melee-map
14 walk melee-map f218
15 walk f218 f215
16 pick-up-petal
17 walk f215 f218
18 walk f218 melee-map
19 walk melee-map dock
20 walk dock low-street
21 buy-map
22 walk low-street high-street-town
23 walk high-street-town jail
24 give-meat-to-prisoner   ; Otis refuses the meat and Bit[420] is set (§14.3)
25 walk jail high-street-town
26 drug-meat-with-petal
27 open-store-door
28 walk-into-store   ; right after open-store-door (§6)
29 pick-up-shovel
30 pay-for-shovel-and-mints
31 walk-out-of-store
32 walk high-street-town high-street-mansion
33 walk high-street-mansion mansion
34 give-meat-to-poodles
35 open-mansion-door
36 walk-into-foyer
37 open-idol-room-door
38 enter-idol-room
39 walk-foyer-to-mansion
40 walk mansion high-street-mansion
41 walk high-street-mansion high-street-town
42 walk high-street-town jail
43 give-mints-to-prisoner
44 give-repellent-to-prisoner
45 open-cake
46 walk jail high-street-town
47 walk high-street-town high-street-mansion
48 walk high-street-mansion mansion
49 walk-into-foyer
50 steal-idol
51 walk-past-fester-to-underwater
52 walk-up-ladder-taking-idol   ; Bit[85] set here
53 walk cu-dock dock
54 walk dock lookout
55 walk lookout melee-map
56 walk melee-map f218
57 walk f218 f215
58 walk-forest-gate-215-220
59 walk f220 f213
60 walk f213 f212
61 walk f212 f204
62 walk f204 f211
63 walk f211 f216
64 walk f216 f201
65 walk f201 treasure-site
66 dig-treasure   ; Bit[86] set here
; cost = 66 (general cost)
```

Ties exist. For example, the meat may be drugged in any room after the petal, and the first jail visit may come before or after the store. The store door's Open is always immediately followed by the walk in (§6). Fast Downward may print a different plan of the same cost after an edit.

**Replay ticks** (headless, from segment start to goal; §7 lists the run dirs):

| seed | pre-audit plan (cost 67) | current plan (cost 66) | difference |
|---:|---:|---:|---:|
| 1 | 106892 | 107660 | +768 |
| 2 | 106862 | 107054 | +192 |
| 3 | 106928 | 106874 | −54 |
| 4 | 107600 | 107366 | −234 |
| 5 | 107660 | 107366 | −294 |

In the pre-audit column, seed 1 is the original run. Seeds 2 to 5 are the same compiled steps replayed for this comparison, and that rebuild reproduces seed 1's 106892 exactly.

The model's own effect is −54 ticks on every seed:

- −66 for the removed Open 570, +42 because the Walk to 570 now does the walk to the door itself;
- −24 for the removed second Open 437;
- −6 for Walk to 315 straight from the right half.

Everything else is the RNG, because the shorter timing shifts every later random draw. The idol-room and Fester scenes vary by tens of ticks, but most of the spread is in the store:

- The storekeeper is absent on 1 store entry in 4 (`room-030-store/entry.txt [002B]`). He then walks in and catches ego first (`local-204.txt [0061]` to `[0359]`, Bit[324]).
- The store dialogue picks random line variants (`local-211.txt [043D]`, `[060B]`, `[08FF]`), some of which walk ego elsewhere. So the pay step takes 4854 to 5544 ticks, and the walk out takes 132 or 420.
- On seed 1 the new timing made the storekeeper absent: +468 on the pay step and +288 on the walk out.

No claim holds for every seed: the plan is shorter, but a single seed's total can still go either way.

**The §14 plan** (the listing above) replays to the goal on seed 1 in 106,412 ticks (`out/runs/20261004T220733Z-run`), 1,248 fewer than the previous plan's 107,660:

- −738 is `give-meat-to-prisoner` (1,032) against `talk-to-prisoner` (1,770);
- −588 is the pay step (4,956 against 5,544), which is the store RNG;
- the rest is a few tens of ticks of shifted timing.

Seeds 2 to 5 were not rerun for this plan.

## 9. Counts against the human route

`docs/human-route.md` §4 has 22 actions and 48 transitions from the Lookout. Our segment starts on the dock, and the human list's step 1 (Lookout → Dock) falls away with the intro skip, so the comparable human count is 47 transitions.

| | ours, by the naming rule | ours, human-comparable | human |
|---|---:|---:|---:|
| actions | 18 | 21 | 22 |
| room transitions | 48 | 46 | 47 |

**The human-comparable column** counts our three composite `walk-*` actions the way the human list does:

- `walk-out-of-tent-after-helmet-meat` is the helmet action plus the scripted walk-out, which the human list counts as a transition.
- `walk-past-fester-to-underwater` is Open door plus scripted transitions, which the human list does not count.
- `walk-up-ladder-taking-idol` is Pick up idol plus a scripted transition, which the human list does not count.

**Action differences against the human route:**

- **−1:** no "Open door (kitchen)". The cook opens 316 himself, and we wait for him with `until`.
- **−1:** pot and meat in one sentence (`use-meat-with-pot`), which G19's "Use pot on meat" also notes.
- **+1:** the idol room needs Open 632 *and* Walk to 632 (`obj-0632-door.txt [0028]`, `[0018]`); the human list counts one action. Only Walk to on an open 632 starts room 53's local-210 (`obj-0632-door.txt [0024]`; the only other `startScript(210` is a Part III global). The cutscene locks 632 again (`local-210.txt [004B]`), so it cannot replay. The human list does not count the walk-through, which happens inside the room.

**Transition difference against the human route:**

- **−1:** the bar exit. The human list counts "Bar back room → Bar main room" (human-route.md §3.2 step 13) and then "Bar main room → Dock" (step 14). Our `walk-out-of-bar-from-right-meanwhile` (the `walk bar-right dock` link before §14) is one Walk to 315 from the right half (rooms.md T25a).
  - The camera rule allows it, because the walk left brings 315 on screen (`rules/glitchless.md`).
  - A player saves the same transition with one floor click toward the left half before clicking 315. We do not count that click.

Otherwise the transition sequence is identical to the human route: kitchen, circus, petal, map, Otis, store, mansion, Otis, mansion, underwater, forest.

The §14 additions leave these counts unchanged. On the first jail visit, `give-meat-to-prisoner` replaces the Talk to Otis one for one: both set Bit[420], and both are one sentence.

Our plan uses 67 compiled steps in total: 66 actions plus the circus tent's second walk (§12). It also makes 15 dialogue choices and 2 verb-slot clicks (the helmet).

The pre-audit text here said "69 compiled steps (67 actions plus 2 defensive opens)". That missed the circus double walk: the pre-audit plan compiled to 70 steps, and its replay ran 70.

## 10. Follow-ups

- **Off-screen shortcuts.**
  - Walk to 315 from `bar-right` is now modelled (§2).
  - Walk to 431 from the town half is excluded by the camera rule (`rules/glitchless.md`: the town camera never shows 431). It is not a follow-up.
  - Walk to 316 from `bar-left` would save 1 action. It is held back by the cook race (§7.1) and would need an engine measurement of the longer walk against the cook's return.
- **Map entries.** Capping them at 3 is impossible for this goal (§7.2). The pirate risk is deterministic per seed, and it passed on seeds 1 to 5.
- **Store variance.** The store's random branches (§8) dominate run-to-run tick differences. The plan cannot branch on them.
- **Time objective.** §14 lists what the costing step (Task 8.3) must measure: the split halves, the alternatives, and the context effects that are not split (§14.6).

## 11. Corrections to analysis notes

1. **money.md §2.3 and §8 Q10** say the wandering pirates start "from the third map entry". In fact `room-085-melee/entry.txt [009B]` tests `Var[290] > 2` *before* the increment at `[00C0]`, so they start on the **4th** entry. That agrees with start.md, rooms.md and treasure.md.
2. **idol.md §12 (the kitchen-door race)** says local-216 keeps running "during the local-218 walk-in cutscene". In fact local-218 opens with `cutscene([2])` (`room-028-bar/local-218.txt [0000]`), whose start script ends in `freezeScripts(127)` (`global/script-018.txt [0070]`). Local-216 is started without the freeze-resistant flag (`room-028-bar/local-211.txt [0039]`, `local-205.txt [003A]`), so the cook is frozen from the start of the walk-in. The race window is only ego's sentence walk to 316.
3. **idol.md §12 and §13 Q1** say C3 cannot express actor room or position. Contract C3 now has `actor_room`, `actor_x` and `actor_y`, which `walk-into-kitchen` uses.
4. **The `room` key in the forest.** `docs/plan.md` C4 suggests the forest is checked as room 58. `_currentRoom` is the pseudo-room number, so forest steps omit `room` (§6). start.md §7 Q7 asked exactly this question.
5. **Open 570 as a fallback.** idol.md §12 (and M3 / step 12) and input-scripts.md §3.1 suggest pushing Open (2, 570) in the kitchen in case the cook closed 570 during the walk-in.
   - That cannot happen. 570 changes only together with 316 (scripts 25/26), and ego got in through an open 316. The cook is frozen from the first frame of local-218 (correction 2) and killed by the room change.
   - The only race outcome is a closed 316 before ego arrives. Ego then stays in room 28, and a kitchen-side Open could not run at all.
   - The fallback was removed from `walk kitchen bar-right` (§6).


## 12. Corrections found by replay

- **Circus grounds, room 52.** Local 202 cancels a walk with `doSentence(STOP)` while ego is in walkbox 7 at x > 200 (`data/scripts/room-052-circus-gr/local-202.txt [0000]` to `[001C]`). Arriving from the map at (430,130), the first `Walk to circus tent` stops at (210,117). `walk-into-tent-with-pot` therefore pushes the walk twice, as a player would click twice. The PDDL action count is unchanged: one action, two sentences. Found by the first full replay (step 12 `step_timeout` with `awaiting_menu` stalls).

## 13. Audit `wf_e6db9470-553`

An adversarial audit of this model (workflow `wf_e6db9470-553`) produced these confirmed findings. All are applied.

1. **The defensive Open 570 can never have an effect** (§6, §11 correction 5). It was removed from `walk kitchen bar-right`, and the "fallback re-open" in `rules/glitchless.md` was corrected.
2. **The store door was opened twice in a row** (`open-store-door`, then the Open inside `walk-into-store`). Now one player sentence is one action:
   - `walk-into-store` is a single Walk to 437;
   - the domain makes `open-store-door` immediately precede it (§6);
   - the remaining citizen window is §7.12.
3. **Missing link `bar-right → dock`** (rooms.md T25a): cost 67 → 66 (§8).
4. **Citations:**
   - both branches of door 315 (`obj-0315-door.txt [0072]`, `[0080]` to `[008A]` → `global/script-120.txt [0538]`, `[0090]`);
   - `talk-to-prisoner` (`local-202.txt [0050]` → `[18DE]` to `[19C4]`);
   - `give-mints-to-prisoner` (`local-202.txt [0546]`, `[1381]` to `[13A9]`);
   - the initial owners of 566/567/689 (DOBJ, treasure.md (E8)).
5. **The camera rule** applies only where the walk toward the target brings it on screen (`rules/glitchless.md`). This excludes Walk to 431 from the town half (§2, §10).
   - The current plan's targets were checked against the narrowed rule using CDHD rects:
     - 435 (x 240 to 320) is on screen in the mansion half (c = 160);
     - 434 and 436 come on screen as the town camera follows ego left within c ∈ [528,640];
     - 315 comes on screen through the bar's camera switch, and the circus tent 621 through room 52's switch at x 228;
     - the other wide rooms in the plan (33, 35, 53, 64) have follow cameras, and room 51 shows 617 to 620 (rooms.md §7).
6. **Risk review** (§7):
   - verified by replay: 7.1, 7.3, 7.6, 7.7 and 7.9;
   - narrowed: 7.2;
   - settled: the troll trigger, the reach checks and room 52's STOP on the return walk.

Tests added in `tests/unit/test_pddl_model.py`:

- `test_no_consecutive_duplicate_sentences`: templates, and pairs where one action enables the next;
- `test_store_door_open_is_used_at_once`;
- `test_optimal_plan_has_no_consecutive_duplicate_sentences`: the compiled optimal plan.

The §14 tests are listed in §14.7.

## 14. Alternatives for the time objective

Task 8.1b of `docs/plan.md` (Phase 8). The objective becomes measured ticks: the costing step (Task 8.3) rewrites each action's cost as its mean ticks, and Fast Downward picks the fastest plan. So this section does two things:

- it re-admits every alternative that §5 excluded only because it costs more actions;
- it splits every action whose duration depends on a one-shot flag, so each half gets its own measured cost.

The model keeps unit costs, and the unit-cost optimum stays 66 (§8).

**Where the numbers come from.** Tick numbers are from seed 1 with text and cutscene skipping off:

- the §14 unit-cost plan, `out/runs/20261004T220733Z-run`;
- the validation plan of §14.5.

They show what the scripts add or remove. Skipping will shrink every dialogue and cutscene figure, often by most of it. The costing step measures the real numbers.

### 14.1 Splits: the same sentence, a different duration

Each pair pushes identical steps and has complementary guards on one fact (`test_split_pairs_have_complementary_guards`).

| pair | fact | why the duration differs | seed 1 |
|---|---|---|---|
| `walk-out-of-bar-from-{left,right}-meanwhile` / `walk-out-of-bar-from-{left,right}` | `lechuck-cutscene-seen` (Bit[446]) | The first Walk to 315 sets Bit[446] and plays global 120, the LeChuck "Meanwhile" cutscene in rooms 70 and 72, before the dock (`room-028-bar/obj-0315-door.txt [0080]` to `[008A]`, `global/script-120.txt [0538]`). Later exits load the dock at once (`[0090]`). Bit[446] is set nowhere else and is clear at segment start (`state-start.json`). | first exit 9,696 / 9,612; a later exit is one room load (no plan has measured one yet) |
| `walk-into-kitchen` / `walk-into-kitchen-after-provoking-cook` | `cook-provoked` | Unprovoked, the cook stays in the kitchen 1800 to 3000 jiffies after bar entry (`local-211.txt [000D]`). Provoked, `local-212` brings him out 600 jiffies after the provoke's cutscene (`[0000]` to `[0020]`). | the wait before the push is 2,424 unprovoked and 822 after the provoke; the walk-in is 234 and 42 |
| `walk-up-ladder-taking-idol` / `walk-up-ladder-taking-idol-last` | `treasure-trial-done` | Bit[85] is set at `room-042-underwate/local-200.txt [0041]`. After it come the walk to the ladder, room 83, and the Elaine rescue scene (`room-083-cu-dock/entry.txt [008E]` → `local-201.txt [0000]` to `[07A0]`). If the treasure is already dug up, the goal holds at `[0041]` and the run ends there: the bridge quits on the first frame the goal holds (`docs/plan.md` C1). | 12,738 to `step_end` against 312 to the goal |

**Not split: `dig-treasure`.** Bit[86] is set at `room-064-treasure/local-200.txt [0214]`, two opcodes before `endCutscene` (`[021D]`). Whether or not the dig is the last action changes its length by about a frame.

**How `src/speedrun/measure.py` (Task 8.3) charges these.** It gives each action the time from its first `step_start` to the next action's, and gives the last action the time to the `goal` tick. The durations sum to the run total.

- **The kitchen wait lands on the action before the walk-in.** The cook wait is an `until` before the push (C4 step flow, step 1), so it is charged to whatever precedes the walk-in.
  - Unprovoked, that is the curtain walk: `walk bar-left bar-right` measures 198 + 2,424 on seed 1.
  - Provoked, the guard forces the walk-in to come right after the provoke, which then carries the short wait: 762 + 822. The two routes compare correctly: 2,622 + 234 against 1,584 + 42.
  - **The trap:** the curtain link's measured cost includes the long wait. If it is reused where no unprovoked walk-in follows, the cost is wrong. That happens in curtain + `provoke-cook` from the right half, which would look about 2,400 ticks worse than it is. The left-half provoke does not use the curtain.
- **The goal-completing action is costed to the `goal` record.** It has no `step_end`, and `measure.py` charges it up to the goal tick. So `walk-up-ladder-taking-idol-last` gets its 312-tick kind of cost, and `dig-treasure` (last today) already gets the same treatment.

### 14.2 First-visit dialogues need no split

The scripts branch on a visit flag in four dialogues. In each, every modelled instance is the first one, by construction:

- **The Fettucini brothers** (`Bit[71]`: the first visit's sales pitch, `room-051-circus-te/local-207.txt [0438]` to `[088D]`; `Bit[72]`: a return visit jumps to M3, `[042B]`).
  - The tent is entered only by `walk-into-tent-with-pot` (`¬circus-money`) and left only by the helmet (which sets `circus-money`), so the one visit is the first.
  - The two-visit variant stays out (§14.4).
- **The citizen** (`Bit[16]` first-talk menu, `Bit[475]` pitch heard; `room-035-low-stree/local-218.txt [03D0]`, `[06AA]`). `buy-map` is the only talk with 441 and is one-shot (`¬has treasure-map`). Its list `barber`, `swell gift` fits only the first talk.
- **The storekeeper.**
  - The store menu is opened only by the pay actions, once per run. Every pay path skips the greeting (`Bit[309]`, `room-030-store/local-204.txt [0415]`, `local-211.txt [0020]`).
  - `Bit[319]` on the first entry only draws the safe combination (`entry.txt [001E]` → `local-205`, no wait).
  - His presence (1 in 4 absent) changes the pay step by seed, not by visit (§8).
- **Otis.**
  - `talk-to-prisoner` and the two breath gives are one-shot (`¬otis-breath-known`).
  - `give-mints-to-prisoner` is one-shot (`¬otis-breath-fresh`), so it always gets the first "So, have you come to release me?" (`room-031-jail/local-202.txt [005C]`, `Bit[476]`).
- **The pirate leaders.** Only the first meeting is modelled (§14.3).

**Other first-time branches.** The entry scripts of every route room were checked for other one-shot branches. Two are visit-dependent, and neither is a long first-time branch:

- **The kitchen gull.** On the first visit the gull lands once ego is past x 170 (`room-041-kitchen/local-204.txt [000C]` to `[0023]` → `local-205.txt [0000]` to `[0028]`; Bit[424]). It is an actor walk with no cutscene, print or `WaitForMessage`. On seed 1 the kitchen's two actions took 384 and 48 ticks.
- **High Street's "Psst"** (`room-034-high-stre/entry.txt [001A]` → `local-203`, while `!Bit[481]`). It is an ambient print, not a one-shot: a context effect (§14.6).

### 14.3 Re-admitted alternatives

**1. Provoking the cook.** §5 used to exclude it only for "+1 sentence".

- **Actions:**
  - `provoke-cook`: Open 316 from the right half;
  - `walk-to-kitchen-door-provoking-cook`: the same Open from the left half;
  - then `walk-into-kitchen-after-provoking-cook`.
- **Script.** Open 316 while `local-211` runs starts `local-214` instead of opening (`room-028-bar/obj-0316-door.txt [0018]` to `[0021]`).
  - `local-214` is a short cutscene: the door opens (`[0005]`, 316 alone), `delay(30)`, the cook's warning (one message, two parts joined by `wait()`), the door closes (`[003B]`).
  - It then starts `local-212` (`[004B]`), which waits 600 jiffies and starts `local-216` (`local-212.txt [0020]`): the cook comes out and opens 316 and 570 (`local-216.txt [001B]`).
  - `local-211` keeps running, so a provoke can only make the cook come out earlier.
- **Guards:**
  - `cook-timer-fresh`: the provoke is valid only while this bar visit's `local-211` runs. `walk-into-bar` sets the fact (`local-205.txt [0040]`/`[0044]`), and every kitchen entry consumes it: coming back from the kitchen brings the cook out at once (`[0033]` to `[003A]`). One kitchen entry per bar visit follows from this; no route needs two.
  - `cook-provoked`: only the kitchen walk may follow a provoke (`test_cook_provoked_is_used_at_once`).
- **Click equivalence.** The step's `until [{not = {actor_room = 6, eq = 28}}]` is the branch of the bar's input script in which a click on 316 runs the door's verb directly (`local-203.txt [003A]` to `[004A]`).
- **The left-half variant.** It uses the camera rule: the walk crosses x 320 and the camera pans right (`local-201.txt [0012]`). The cook race of §7.1 does not apply, because the cook is in the kitchen.
- **Trade-off.** The provoke removes `local-211`'s remaining wait (1800 to 3000 jiffies from bar entry) and adds `local-214`'s cutscene plus 600 jiffies. On seed 1, from bar entry to the kitchen:
  - plain: curtain 198, wait 2,424, walk-in 234, so 2,856;
  - provoked from the left half: 762, wait 822, walk-in 42, so 1,626.

  That is −1,230 ticks. Under unit costs the left-half variant ties with the plain route (3 actions each); Fast Downward kept the plain one.

**2. Learning Otis's bad breath by a refused give.** §5 excluded the repellent give as "same cost as Talk to".

- **Actions:**
  - `give-meat-to-prisoner`;
  - `give-repellent-to-prisoner-before-mints`, while 405 still has class 6.
- **Script.** Every give except the mugs, the opened cake and the mints ends at `room-031-jail/local-203.txt [034C]`, which sets Bit[420] (`[0351]`).
  - The meat has no handler of its own. The repellent is taken only once class 6 is clear (`[011F]`), so before the mints it jumps to the same refusal (`[01EB]` → `[029D]`).
  - The refusal changes no owner: the meat stays the pot's guard and is still needed for the poodles.
  - The give path runs three things: the walk-to-the-cell cutscene (`[0082]` to `[00B7]`), the refusal ("I don't want anything but my freedom!", "…and maybe a breath mint.", `[02A8]` to `[02F9]`, with an override) and "Man! Talk about bad breath!" (`[035A]`).
  - The Talk runs the same walk-in (`local-202.txt [001E]` to `[004F]`) and then the halitosis scene: two Otis lines, two of ego's, and a walk (`[18DE]` to `[19C0]`).
- **Trade-off.** On seed 1 the give takes 1,032 ticks and the talk 1,770, so −738. The unit-cost plan now uses `give-meat-to-prisoner`; it is a tie under unit costs.

**3. The storekeeper as guide, instead of buying the map** (treasure.md §7). §5 excluded it as inexpressible and dominated. Rechecked, both reasons fail:

- **Expressible.** The old note said STRIPS could enforce "no detour" only if every action deleted a "following" flag. A precondition does it instead: the generic `walk`, and every other action that can run on his path, requires `¬following-storekeeper`. A precondition is not a conditional effect. The follow is then a chain of dedicated walks (`test_following_the_storekeeper_is_isolated`).
- **Reliable to replay.** This is not an NPC follow that tracks his position. Global 67 is driven by room entry:
  - for each room it puts him in the room, waits until `VAR_ROOM` is that room (`global/script-067.txt [02E3]`), walks him to the exit and restarts for the next room (`[0353]`);
  - it gives up only if ego has not entered the room within 1800 jiffies of that room's start, or 3600 in 33 and 85 (`[027D]` resets `VAR_TMR_2`; `[035A]`);
  - the gate at 215 checks only that 67 runs (`room-058-damnfores/obj-0685-path.txt [005B]`);
  - ego may get ahead of him: a room change ends his walk, because ScummVM's `startScene` hides every actor, and 67 continues.

  On seed 1 the hops took 6 to 1,308 ticks. The lookout, which he skips (33 → 85), took 1,308 + 120, inside room 85's 3600.
- **The unlock: `talk-to-pirate-leaders`.** Topic 122 needs `Var[199]` (`room-030-store/local-211.txt [00FA]`).
  - The first meeting with the leaders sets it: "I want to be a pirate." (`room-028-bar/local-220.txt [033D]`), the trials speech (`[03DF]` to `[0910]`), then "Tell me more about mastering the sword." (`[0970]`) and `Var[199] = 1` (`[1036]`), then "I'll just be running along now." (`[0DCD]`).
  - The talk is guarded to the first meeting with no trial done: `local-220 [0261]` skips the first menu once `Var[196] > 0`.
  - It must also come after this visit's kitchen entry, because `local-211 [0021]` redraws the cook's delay while `local-220` runs.
- **The store.**
  - `pay-for-shovel-ask-guide` and `pay-for-shovel-and-mints-ask-guide` choose topic 122 after the purchases. It ends the dialogue (`[1138]`): the storekeeper leaves (`[0ED2]`), closes 387 alone (`[1122]`) and starts global 67 for room 34 (`[1131]`).
  - The four plain pay variants require `¬sword-master-asked` (`test_store_menu_with_the_guide_topic`).
- **Leaving.** `open-store-door-from-inside` (Open 387, `obj-0387-door.txt [004E]`), then `walk-out-of-store`. Its exit branch does not stop 67 (`local-204.txt [0031]` to `[0044]`); only the unpaid branch does (`[0095]`).
- **The chain:**
  - `walk-follow-guide-to-low-street` (433), `-to-dock` (450), `-to-lookout` (426), `-to-map` (487), `-to-f218` (911), `-to-f215` (685 at 218);
  - then `walk-forest-gate-215-203-with-guide` (685 at 215). Gate 688 has no script-67 exemption (`obj-0688-path.txt [004F]` to `[0063]`). The gate sets Bit[401] (`obj-0685-path.txt [00C1]`).
  - On the chain only `pick-up-petal`, `drug-meat-with-petal` and `open-cake` may run (6 to 78 ticks each).
- **After the gate.** `forest-gate-open` lets `walk-forest-gate-215-203-open` and `walk-forest-gate-215-220-open` pass without the map. The map gates set the fact too.
- **Trade-off.** It removes `buy-map` (3,750 on seed 1) and adds:
  - the leaders' talk, 11,670 unskipped. Both long speeches have override points (`[03DF]`, `[0E37]`), so this shrinks most with cutscene skipping;
  - the guide topic: 7,416 for the guide pay against 4,956 to 5,544 for the plain one;
  - Open 387, 162;
  - two forest hops, 215 → 203 → 215, 162 + 24.

  Unskipped it loses about 10,000 ticks; skipped it is for the measurement to decide. Under unit costs it is +3 against the same plan with the map.
- **Sword unlock: deferred, not infeasible.** Topic 122 also shows with the paid sword (`Bit[98]`, `local-211.txt [016A]`). Owning the sword changes two later actions, which would need their own splits:
  - Fester confiscates it (`room-053-foyer/local-217.txt [030F]` to `[0336]`);
  - the underwater script walks ego to it before Bit[85] (`room-042-underwate/local-203.txt [0030]` to `[008B]`).

**4. The treasure before the idol.** The model always allowed this order, at +1 action: 67 with the map, because the petal is picked up on the treasure trip.

- It saves the Elaine scene (12,738 against 312 on seed 1, §14.1).
- It adds the walk back from the treasure site to town: on seed 1, 64 → map → dock → 35 → 34 took 294 + 714 + 930 + 522.

The split in §14.1 is what makes the order visible to the costing step.

### 14.4 Still excluded

| alternative | why, per component |
|---|---|
| Talk to the storekeeper; ring the bell 399 | **Replay feasibility.** His presence is random on each entry (`room-030-store/entry.txt [002B]`): Talk to 394 cannot be clicked while he is away (`[0057]`), and the bell is touchable only then (`local-208.txt [000F]`). The plan cannot branch. |
| Use meat with poodles | **Same ticks, riskier.** The sentence script walks to 467 exactly as for Give (`global/script-002.txt [02DB]` against `[0083]`), and the meat's Use just starts 467's verb 80 (`room-041-kitchen/obj-0566-hunk-of-meat.txt [0098]`/`[00A2]`), the script Give runs. The only difference is the reach check: ≤ 16 (`[02EB]`) instead of ≤ 32 (`[0093]`), with the dogs' boxes locked (`room-036-mansion-e/entry.txt [002B]` to `[0038]`). No faithful cost can prefer it. |
| Use pot with meat (both on the table) | **Same ticks, worse state.** One sentence, the same two auto pick-ups as `use-meat-with-pot` in the other order (`global/script-002.txt [0229]`/`[0251]`). The pot then comes first, so another guard is needed for the helmet. |
| The minutes deal (+2) | **Never faster.**<ul><li>It adds a whole dialogue: Talk to 452, the pitch, "two pieces of eight", "running along" (`room-035-low-stree/local-216.txt [0564]` to `[093E]`, `[095B]`).</li><li>Its only goal effect is to make 488 a pot guard. The meat comes with the pot in one sentence and is held through the circus on every route where the circus comes before the poodles.</li><li>The one order it would enable, the poodles before the circus with no guard held, needs a second map round trip.</li><li>It also adds 464 and 488 to the inventory before the payout, which breaks §4.3's bound of at most 7 visible items in 8 slots.</li></ul> |
| The circus without the pot, "Er… no", return later | **Never faster.** It adds the refusal and "Go get a helmet" (`room-051-circus-te/local-207.txt [0C68]` to `[0D3F]`), a walk-out, a map round trip to the clearing, and "Hello again" on return (`[03EC]`). The return skips only the pitch and M2 (`[042B]`), which the first visit has already played. It removes nothing from a single visit with the pot. |
| Re-reading the map; stewed meat without the stew; fish routes | **No goal effect** (treasure.md §3; the dogs refuse fish, `room-036-mansion-e/local-201.txt [01F9]`). They only add ticks. |
| Walk to 316 from `bar-left` (straight in) | **Replay risk.** It would save the curtain walk (198 ticks on seed 1), but the longer walk widens the cook race (§7.1). It needs an engine measurement of that race (§10). |
| Walk to 431 from `high-street-town` | **Rules.** The town camera never shows 431 (§2). |
| Give pot to a Fettucini brother; boot params; coordinates | **Rules or trap** (`rules/glitchless.md`; money.md B1). |
| Dialogue-choice variants (for example Fester's four answers) | **Not modelled as actions.** The templates take the choice with the shortest reply by reading the script (§4.8). A measured comparison of choices is possible later, with the same templates. |

### 14.5 Validation

- **The unit-cost plan.** It is still cost 66 (§8 lists the ties). Replayed headless on seed 1, it reaches the goal in 106,412 ticks (`out/runs/20261004T220733Z-run`).
- **A validation plan.** It is a hand-written 70-action plan that uses every alternative in §14.3 and the idol-last split. It was checked against the domain by STRIPS simulation, then replayed once on seed 1 through `run_engine` with skipping off, in a scratch run directory (not kept). It reached the goal in 106,310 ticks, in room 42 before the Elaine scene. Seed-1 ticks are in brackets:

  ```
   1 open-bar-door                              [354]
   2 walk-into-bar                              [6]
   3 walk-to-kitchen-door-provoking-cook        [762]
   4 walk-into-kitchen-after-provoking-cook     [822 wait + 42]
   5 use-meat-with-pot                          [384]
   6 walk kitchen bar-right                     [48]
   7 talk-to-pirate-leaders                     [11670]
   8 walk-out-of-bar-from-right-meanwhile       [9612]
   9 walk dock lookout
  10 walk lookout melee-map
  11 walk melee-map clearing
  12 walk-into-tent-with-pot
  13 walk-out-of-tent-after-helmet-meat
  14 walk clearing melee-map
  15 walk melee-map dock
  16 walk dock low-street
  17 walk low-street high-street-town
  18 walk high-street-town jail
  19 give-meat-to-prisoner                      [1032]
  20 walk jail high-street-town
  21 open-store-door
  22 walk-into-store
  23 pick-up-shovel
  24 pay-for-shovel-and-mints-ask-guide         [7416]
  25 open-store-door-from-inside                [162]
  26 walk-out-of-store                          [6]
  27 walk-follow-guide-to-low-street            [480]
  28 walk-follow-guide-to-dock                  [456]
  29 walk-follow-guide-to-lookout               [1308]
  30 walk-follow-guide-to-map                   [120]
  31 walk-follow-guide-to-f218                  [234]
  32 walk-follow-guide-to-f215                  [216]
  33 pick-up-petal                              [78]
  34 walk-forest-gate-215-203-with-guide        [162]   ; Bit[401]
  35 walk f203 f215                             [24]
  36 walk-forest-gate-215-220-open              [336]
  37 walk f220 f213
  38 walk f213 f212
  39 walk f212 f204
  40 walk f204 f211
  41 walk f211 f216
  42 walk f216 f201
  43 walk f201 treasure-site
  44 dig-treasure                               [3696]  ; Bit[86]
  45 walk treasure-site melee-map               [294]
  46 walk melee-map dock                        [714]
  47 walk dock low-street                       [930]
  48 walk low-street high-street-town           [522]
  49 drug-meat-with-petal
  50 walk high-street-town high-street-mansion
  51 walk high-street-mansion mansion
  52 give-meat-to-poodles
  53 open-mansion-door
  54 walk-into-foyer
  55 open-idol-room-door
  56 enter-idol-room
  57 walk-foyer-to-mansion
  58 walk mansion high-street-mansion
  59 walk high-street-mansion high-street-town
  60 walk high-street-town jail
  61 give-mints-to-prisoner
  62 give-repellent-to-prisoner
  63 open-cake
  64 walk jail high-street-town
  65 walk high-street-town high-street-mansion
  66 walk high-street-mansion mansion
  67 walk-into-foyer
  68 steal-idol
  69 walk-past-fester-to-underwater
  70 walk-up-ladder-taking-idol-last            [312 to the goal]  ; Bit[85]
  ```

  Every new action did what the model says, and the trace confirms it:

  - Bit[420] from the give;
  - Bit[102] and Bit[326] from the guide topic;
  - Bit[401] at the gate with the guide;
  - the goal mid-step in room 42.

  It also beat the unit-cost plan unskipped, even though the leaders' talk is unskipped.
- **Compiled but never run in the engine.** These actions pass `test_steps_resolve_against_script_index`, and each pushes the same sentence as an action that did run, but no replay has exercised them:
  - `provoke-cook` (the right-half provoke);
  - `pay-for-shovel-ask-guide` (the guide without the mints);
  - `give-repellent-to-prisoner-before-mints`;
  - `walk-forest-gate-215-203-open`;
  - `walk-out-of-bar-from-left-meanwhile`;
  - the later bar exits `walk-out-of-bar-from-{left,right}`.

### 14.6 Context effects left to the costing step

These change an action's duration but depend on where ego is or on ambient scripts, not on a one-shot flag, so they are not split. Task 8.3 keys them by context (`docs/plan.md` Phase 8).

- **Where the walk starts.**
  - `walk-into-foyer` takes 6 ticks right after `open-mansion-door`, whose walk left ego at the door, and 276 from the trail.
  - `walk dock lookout` takes 1,008 from the bar door, 642 from room 83, and 1,308 from the low-street archway.
  - `walk-into-store` and `walk-into-bar` take 6 ticks because their Open walked ego to the door.
- **The "Psst" on High Street.** `room-034-high-stre/local-203.txt` runs while `!Bit[481]` (before the idol-room visit). When ego is within 150 of the alley (`[0002]` to `[0009]`), it prints and parks on `WaitForMessage` (`[0010]`, `[00F8]`), which can hold the idle test.
- **The store's random presence and lines** (§8): the pay step took 4,956 to 7,416 on seed 1, depending on the variant.
- **The map pirates on the 4th map entry** (§7.2). In the treasure-first order that entry is the return from 64.
- **Where a room change lands, when two exits make it.** `speedrun.positions` names the spot a room change leaves by its rooms and its exit object: `entry:<from>:<to>:<id>`. The exit object is the room object its last sentence walks to; a room change with no such sentence, the helmet's click-only step, leaves `entry:<from>:<to>`.
  - Until §14.8 the token was `entry:<from>:<to>`. Docks 904 and 905 shared `entry:cu-dock:dock` although they land at x 308 and x 566, and `walk dock lookout` measures 642 after one and 876 after the other.
  - The curtain walk and the left-half provoke also shared `entry:bar-left:bar-right`, although one leaves ego at (330,137) and the other at 316's walk point.
  - Actions on the same exit still share a token: the split bar exits, the three gates at 215, the plain and the guided 685 at 218, and the two ladder variants.
  - Path 686 lands where 685 does, since it runs 685's code. It still gets its own token, because a template cannot say where the destination room puts ego. The only cost is samples: on the pooled data, `pick-up-petal` takes 78 ticks after either.
  - Pooling re-derives each earlier summary's contexts from its plan (`optimize.pool_prior`), so the earlier measurements still count under the new tokens. On the current `out/` tree all 49 summaries with the current bridge pooled, 885 runs.

### 14.7 Tests

These tests in `tests/unit/test_pddl_model.py` guard the new structure. They were written first, and each failed before the model change it guards.

- `test_bar_exits_are_actions_not_links`: no `bar-* → dock` links; the first exit adds `lechuck-cutscene-seen`, which is clear initially.
- `test_split_pairs_have_complementary_guards`: for each pair in `SPLIT_PAIRS`, the guards are complementary, the steps are identical and the moves are the same.
- `test_cook_provoked_is_used_at_once`: every action that can run in the bar, except the provoked kitchen walk, requires `¬cook-provoked`.
- `test_provoke_needs_a_fresh_cook_timer`: only `walk-into-bar` sets `cook-timer-fresh`; both provokes require it; every kitchen entry requires and deletes it.
- `test_breath_gives_keep_the_item`: the three ways to Bit[420] are one-shot and take no item.
- `test_following_the_storekeeper_is_isolated`: on his path only `FOLLOW_ACTIONS` may run while following; only the two guide pays start following; only the gate with the guide ends it.
- `test_store_menu_with_the_guide_topic`: a pay variant has the guide topic if and only if it requires `sword-master-asked`, and the guide variants close 387.
- `test_pirate_leaders_talk_is_the_first_meeting`: the talk's guards, and the two map-less gates.
- `test_planner_finds_plan` now also asserts the unit-cost optimum, `UNIT_COST_OPTIMUM = 66`.
- `test_no_consecutive_duplicate_sentences` now counts a pair only if A also leaves B applicable. A must not end elsewhere or make true a fact B needs false. Without this, the split bar exits and the two provokes were false positives.

The §14.8 tests were written first as well, and each failed before its model change:

- `test_alternative_exits_mirror_their_twins`: for each entry of `ALT_EXITS`, the alternative's ground preconditions and effects are its twin's (minus the `link` fact for a link twin), and its template is the twin's with only the object `id` changed;
- `test_alternative_exits_compile_to_their_object`: each template compiles to its own object (686 or 905) against the script index;
- `test_steal_idol_needs_only_the_opened_cake`: no `(has manual)` or `(has lips)` precondition; the theft still deletes the manual, the lips and the cake;
- `test_following_the_storekeeper_is_isolated` now lists `walk-follow-guide-to-f215-via-686` in `FOLLOW_ACTIONS`.

For the exit-object tokens (§14.6), in `tests/unit/test_positions.py`. Most failed before the change; two only guard behaviour. `test_a_room_change_with_no_sentence_has_no_exit_object` already passed, and `test_a_room_change_without_a_template_fails` was written after the implementation:

- the toy tests expect `entry:<from>:<to>:<id>`;
- `test_two_exits_between_the_same_rooms_leave_different_tokens` and `test_a_room_change_with_no_sentence_has_no_exit_object` cover the two cases of the token;
- `test_a_room_change_without_a_template_fails`: a room change now needs its template;
- `test_real_model_anchors` covers the four new actions and the bar-half pair;
- `test_real_model_room_changes_are_keyed_by_their_exit_object`: which real twins share a token and which do not.

`tests/unit/test_keyed.py` expects the new tokens. `tests/unit/test_optimize_keyed.py::test_pool_prior_rekeys_old_summaries_from_their_plans` pools a summary whose stored contexts use the old tokens, and checks that every sample is re-keyed from its plan.

### 14.8 Findings from the blind extraction

Phase 7 rebuilt Part I from the scripts alone. Its review is the "Analysis" section of `docs/extraction-diff.md`. It found three legal alternatives that this model lacked (its §3) and one guard that no script backs (its §4, pair 7, and §6). All four were checked against the scripts and are applied here.

**1. Alternative exit objects.** Each makes the same transition as an existing link, but from another walk point, so its duration may differ. Under unit costs each ties with its twin, so the plan of §8 is unchanged. The time objective decides between them.

| action | twin | sentence | script evidence | arrival |
|---|---|---|---|---|
| `walk-f218-f215-via-686` | `walk f218 f215` | Walk to 686 at 218 | 218's entry draws 686 at strip 28, next to 685 at strip 15 (`room-058-damnfores/entry.txt [08E5]`, `[08ED]`). 686 has no Walk to of its own: any verb on it runs `startObject(685,11)` (`obj-0686-path.txt [0010]`), so 685's case for 218 moves ego (`obj-0685-path.txt [0168]`). | 215, at 687's walk point, as the twin |
| `walk-f220-f210-via-686` | `walk f220 f210` | Walk to 686 at 220 | 220's entry draws 686 at strip 15 and 685 at strip 28 (`entry.txt [09B9]`, `[09C1]`). 685's case for 220 is `[018C]`. | 210, at 687's walk point, as the twin |
| `walk-cu-dock-dock-via-905` | `walk cu-dock dock` | Walk to 905 | 905 is 904's twin at the east edge of 83. While `!Bit[453]`, which is never set in Part I, it puts ego in room 33 (`room-083-cu-dock/obj-0905-dock.txt [0010]` to `[0020]`). | the dock at (566,132), instead of 904's (308,132) |

- **Guards.** Each alternative carries exactly the generic walk's guards: `¬store-door-open`, `¬cook-provoked` and `¬following-storekeeper`. 685 and 904 carry no other guard at these nodes.
- **Touchable.** Every forest entry clears class 32 on 685 to 688 (`entry.txt [01BC]` to `[01D1]`). 905 has no class in the object dump, and no Part I script sets one. Its class 6 (`room-083-cu-dock/local-204.txt [037B]`) matters only in the `Bit[453]` branch.
- **The camera after 905.** Arriving from 83 starts the dock's local-201 (`room-033-dock/entry.txt [0032]`).
  - At x 566 local-201 pins the camera to `RoomScroll(712,848)` until ego walks below x 566 or past x 726 (`room-033-dock/local-201.txt [0042]`, `[004E]`, `[005F]`). It then sets `RoomScroll(0,848)`, which frees the camera (`[0070]`).
  - While the camera is pinned, the cliffside 426 is off screen. A Walk to 426 crosses x 566 at once, and the follow camera then brings 426 into view. That is the case the camera rule allows (`rules/glitchless.md`, "Camera visibility").
  - From 904's landing at x 308, the pin is `RoomScroll(0,160)` (`[0011]`), which already shows the cliffside.
- **Also added: following the storekeeper through 686** (`walk-follow-guide-to-f215-via-686`, the twin of `walk-follow-guide-to-f215`).
  - At 218, global 67 walks the storekeeper to a point rather than to 685. Local[5] is not set there, so `walkActorTo(11,154,74)` runs (`global/script-067.txt [00D6]`, `[02F6]`).
  - It then restarts itself for 215 and waits only for `VAR_ROOM` (`[0353]`, `[02E3]`), so ego's choice of path does not matter.
  - It carries the twin's guards: `(at f218)` and `following-storekeeper`. It is in `FOLLOW_ACTIONS`.
  - A replay confirms it (point 2 below): the gate with the guide still passes.
- **Not added: the other `|` alternatives in rooms.md §5.2 for modelled links.** By the scripts, each makes its twin's walk and lands the same way, so it cannot be faster and is not worth an action. None was measured:
  - **439 for 34M → 36.** Its Walk to runs 431's (`room-034-high-stre/obj-0439-deadly-piranha-poodles.txt [000C]`), and its walk point is 431's, (82,47) in the object dump. So the sentence walk and the arrival are identical. 439 is also class 32 at segment start, and only room 36's entry makes it touchable (`room-036-mansion-e/entry.txt [0000]`).
  - **421 for 31 → 34T.** Any verb on it runs `startObject(400,Local[1])` (`room-031-jail/obj-0421-unnamed.txt [0010]`), and its walk point is 400's, (294,116).
  - **320 for the curtain.** Its Walk to runs 323's (`room-028-bar/obj-0320-unnamed.txt [000F]`). Both objects have class 8, for which the sentence script skips the walk to the object and runs the verb at once (`global/script-002.txt [02B4]`). So both are the same `walkActorTo` of 323's code.

**2. Replay.** The current time plan (`out/plans/part1.time.sas_plan`, 67 actions) was replayed with `speedrun measure part1 --seeds 1-3` (skips on) in three versions:

- **P0:** unchanged.
- **P2:** P0 plus a detour right after `walk-forest-gate-215-220`: `walk f220 f210`, then `walk f210 f220`. No plan visits 210, so P2 gives 686 at 220 a twin to compare with.
- **P1:** P2 with four substitutions: both `walk f218 f215` steps, `walk cu-dock dock`, and the detour's `walk f220 f210` replaced by the new actions.

All three plans were checked against the domain by STRIPS simulation first. All nine runs reached the goal (`out/measure/20261005T104047Z`, `…104054Z` and `…104100Z`). The per-action ticks of the changed actions were identical on all three seeds:

| P1 action | ticks | P2 twin | ticks | difference |
|---|---:|---|---:|---:|
| `walk-f218-f215-via-686` (from the map, ego at 687), twice | 162 | `walk f218 f215` | 216 | −54 each |
| `walk-f220-f210-via-686` (from gate 688) | 222 | `walk f220 f210` | 306 | −84 |
| `walk-cu-dock-dock-via-905` | 132 | `walk cu-dock dock` | 168 | −36 |
| `walk dock lookout` after it (context `entry:cu-dock:dock:905`, against `…:904`) | 876 | the same | 642 | +234 |

| seed | P0 | P2 | P1 | P1 − P0 | P1 − P2 |
|---:|---:|---:|---:|---:|---:|
| 1 | 22,388 | 22,622 | 22,670 | +282 | +48 |
| 2 | 22,406 | 22,640 | 22,610 | +204 | −30 |
| 3 | 22,370 | 22,604 | 22,730 | +360 | +126 |

- **P1 − P0** adds up as follows:
  - −108 for 686 at 218, used twice;
  - +198 for 905 together with the longer walk after it;
  - +150 for the detour. Its two walks take 222 and 30, and the next `walk f220 f213` is 102 shorter because it starts at 685's walk point instead of the gate's;
  - the rest is the store and jail RNG, which the shifted timing redraws: +42, −36 and +120.
- **686 at 218 saves 54 ticks per use from the map.** That is 108 per run in this plan, with nothing else changed.
- **686 at 220 saves 84 ticks from the gate.**
- **905 saves 36 ticks in room 83, but costs 234 on the walk to the cliffside.** It lands 258 px east, so it pays off only when the next dock exit is east of x 566: the bar door 428 or the archway 427. No route needs either after the idol.
- **The keyed contexts now tell 904 and 905 apart** (§14.6). Under the old token both landings were `entry:cu-dock:dock`, so `walk dock lookout` (642 after one, 876 after the other) was pooled into one context.
- **The guided hop.** The §14.5 guide plan (70 actions) was replayed in the same way, as G0, and as G1 with `walk-follow-guide-to-f215` replaced by `walk-follow-guide-to-f215-via-686`. Both were checked by STRIPS simulation. All six runs reached the goal (`out/measure/20261005T105719Z`, `…105727Z`). The hop took 216 ticks through 685 and 162 through 686, so −54 on every seed. No other action changed, so the totals differ by −54 too: 26,228 / 26,234 / 26,348 against 26,174 / 26,180 / 26,294.

**3. The `steal-idol` guard.** The hand model required `(has manual) (has lips)`. The scripts do not.

- Walk to 637 reads the owner of 420 and its class 6, and nothing else (`room-053-foyer/obj-0637-gaping-hole.txt [000C]`, `[0018]`, `[0021]`). The owner test on 641 at `[002D]` lies in the branch where ego does not hold 420, and it only picks the refusal line.
- In local-211, the manual and the lips go only to the sentence-line helper local-218 (`local-211.txt [0037]`, `[00CA]`). local-218 sets the sentence-line variables and tests no owner (`local-218.txt [001B]`).
- The theft then hides both items whether or not ego holds them (`local-211.txt [0088]`/`[008C]`, `[00E3]`/`[00E7]`).
- No other script on the theft path reads their owner. The scripts checked were the foyer's local-207, 209, 212, 213, 215 and 220, and globals 11, 12 and 119. In the whole dump, the owner of 641 or 642 is tested in four other places only, none of them on the theft path:
  - the idol-room scene (`local-210.txt [0482]`, `[048E]`);
  - door 634;
  - the hole's refusal branch;
  - the foyer's entry, which only loads costumes.

So the guard is dropped. The deletes stay, because the theft hides both items in every case (§1). The guard was harmless, because `enter-idol-room` grants both items together with the repellent. The plans did not change.
