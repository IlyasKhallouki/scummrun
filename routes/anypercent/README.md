# Any% route, DOS CD: the segments end to end

This is the whole-route check of the seven segment files in this directory: the
Any% route for the DOS CD release (SCUMM v5, ScummVM 2026.3.0), modelled on
saruya's 21:52 record. It covers the handoffs between segments, the S1/S2
interleave, the RNG points and glitches in game order, and the questions the
engine runs have to settle.

Move numbers follow each file's own convention: S1 and S2 are 1-based (S2 cites
S1 by 1-based position); S3 to S7 are 0-based, as their notes use them. No move
was added or removed in this pass, so every existing move reference still holds.

## Route at a glance

| Seg | File | Entry | Exit (`until`) | Moves | Record |
|---|---|---|---|---:|---|
| S1 | `S1-trials.json` | room 10, boot; logo-skip Esc starts the clock | room 64 after the dig: Bit[85], Bit[86], has 752 | 96 | 0:00-7:05 (with S2's detours) |
| S2 | `S2-sword.json` | runs inside S1 (see the interleave); block 2 starts in room 64 after S1 96 | room 85 after leaving room 61: Bit[20], has 596 (Carla's shirt) | 59 | to "T-shirts" 9:05 |
| S3 | `S3-crew-slide.json` | room 85, walk-out from the Sword Master's path | the frame global 122 starts (has 901, 902, 903); var 352 == 2 carries the slide | 117 | 9:05-13:30.5 |
| S4 | `S4-part2.json` | Part II card (room 0, inside global 122) | room 20 with var 32 == 201 (Part III intro running) | 77 | 13:30.5-14:48 |
| S5 | `S5-part3-island.json` | room 20, the beach | room 69 holding 269, 293, 294 | 81 | 14:48 to about 17:22 |
| S6 | `S6-catacombs-ghostship.json` | room 69 | room 95, the "Last Part" card | 67 | about 17:22-19:45.5 |
| S7 | `S7-part4.json` | room 95 | room 0 with var 194 == 122 (final fade) | 38 | 19:45.5-21:52.4 |

535 moves in total. The route carries one item slide from S3's store visit to
S6's navigator head. The chain is: S3 m92 forms it, global 70 carries it into
Part II, and it is armed at S4 m6. The oars and monkey skips in S5 use it. S5
loses it to Herman, gets it back with the dusty book at the cannibals, and S6
keeps it across the head dialogue with the book for the feather skip.

## S1 and S2: the merged order

S1 and S2 are one stretch of play. S1's file encodes the merged run, and S2's
moves slot into it in four blocks:

```
S1 1-8      logo, intro, bar, cook, into the kitchen
S2 1-4      pier door, plank twice, fish on flight 2
S1 9-51     pot, meat, circus, petal, map, chicken, Otis 1, store: shovel, safe (Credit Early)
S2 5        sword
S1 52-53    bell, mint
S2 6-7      "About this sword...", "I want it."
S1 54-55    "About this shovel", "I want it."
S2 8        "I think I'd just like to browse." (ends the menu; S1 56 waits for var 32 == 4)
S1 56-83    mansion, Otis 2, cake, Fester, underwater idol, Lookout -> map
S2 9-28     troll, Smirk, fight 1
S1 84-96    fork, forest with the map teleport, dig
S2 29-59    room 64 -> map, fights 2-3, forest to Carla, Carla, exit
S3 0 ...
```

S2 m5 (sword) can go anywhere between S1 47 and S1 52; S2's note leaves it
open. This file puts it after the safe, at S1 51 -> S2 5 -> S1 52. The shovel
(walk point 155,76) and the safe handle (173,78) are upstairs, while the sword
(232,127) and the bell are by the counter, so this order is the shortest walk.
Its place in the inventory list does not matter: 388 and 397 both leave before
Part II.

The exit-against-entry diff reports S1 exit var 196 == 2 and Bit[86] == 1
against S2 entry var 196 == 1 and Bit[86] == 0. That is expected. S2's entry
state is the state at S1 m83 (idol done, treasure not yet), not at S1's exit.
S1 m84 has to wait for S2 m28, and in the merged list that order is automatic.

## Cross-segment problems

### Fixed in the segment files

Every fix adds a missing handoff condition, completes an inventory list or
corrects stale text. None changes a move. Writers were checked with an index of
every `Bit[...]`/`Var[...]` write in `data/scripts-dos-cd`, including indexed
forms such as `Bit[90 + 4]` and `setVarRange` ranges. A carried flag has no
writer between the segment that sets it and the segment that reads it.

1. **S2 exit did not carry S1's state that S3's entry requires.** Added has
   397, 377 and 395; Bit[420], Bit[446], Bit[476] and Bit[481] set to 1. Also
   added the crew-phase bits S3 assumes are clear: 477, 94, 304, 449, 447, 51,
   88, 89, 76, 17, 18, 323 and 421 == 0. Their writers are crew-phase only:
   jail local 202 behind Bit[304] or option 123, the cu-dock scripts, the exits
   guarded by Bit[88]+Bit[89]+Bit[76], and globals 56, 60, 49, 68 and 70.
   Bit[89]'s other writer, global 64 [034A], runs only when Carla is talked to
   after being beaten (global/script-116.txt [013B]).
2. **S1 exit was the standalone state.** The file encodes the merged run, so
   its exit is the start of S2 block 2. Added the sword 388 to the inventory:
   Fester takes it and room-042-underwate/local-203.txt [008B] gives it back.
   Added the block-1 state that block 2 relies on: Bit[98] (sword paid),
   Bit[19] (Smirk), var 282 == 1 and Bit[156] (insult 16 learned, needed for S2
   m32). Also added Bit[476] and Bit[481], set at S1 m69 and m64.
3. **Stale S2 interleave text.** The entry notes, move 1 and move 4 still said
   S1 move 9 was the meat. S1 move 9 is now the pot, and the meat is move 10.
4. **S7's entry flags were promised by no earlier segment.** Carried them
   forward:
   - Bit[51] == 1 (ship bought; only global 56 writes it) from S3 through S6.
   - var 196 == 3 (only global 71 increments it).
   - Bit[436] == 0 (ship not sunk; only room 11, off the route).
   - state(438) == 0 (church door; see open problem 4).
   - From the S6 exit frame: Bit[453] == 1 and var 277 == 6 (global 131
     [002F]/[0034], before its loadRoom(95) at [0039]), and var 194 == 120 (S6
     m65; room-070-hellcliff/local-206.txt has no later reset).
   - Bit[468], Bit[537] and Bit[565] == 0 (Part IV writers only).

   These are added to the S3-S6 exits, and to the S4-S6 entries where the next
   segment reads them.
5. **S3 exit lacked S4's entry conditions.** Added {room 0} (global 122 runs
   loadRoom(0) at [0005] in the exit frame, which S3's notes already say), plus
   var 244 == 0, var 259 == 0 and Bit[366], Bit[531], Bit[399] == 0. All of
   these have writers only in the ship rooms 7, 14 and 19 and in globals 108
   and 185.
6. **S4 exit lacked var 143 == 20** (rowboat at the main beach; the value set
   at boot, written only by the Part III beach objects).
7. **S5 exit lacked S6's entry conditions.** Added has 269, 74 and 732, and the
   head and ghost-ship bits S6 assumes are clear: 536, 545, 357, 316, 470 and
   473 == 0, plus var 286 == 0 and var 287 == 0. Bit[473] is written as
   `Bit[471 + 2]` at global/script-140.txt [044C].
8. **Truncated inventories.** S6's entry listed 9 of S5's 17 exit items, and
   S6's exit and S7's entry inherited the gap. S6's entry now equals S5's exit.
   S6's exit adds 799, 815, 788 and 823 (21 items): the hatch, door and crate
   handlers (obj-0824, obj-0840, obj-0821) never call setOwnerOf on the key,
   grease or tools. S7's entry equals S6's exit, and room 83's entry still cuts
   it down to 823.
9. **S4 said S3 places global 70's tail in room 19.** S3's exit notes already
   say room 7. S4's entry notes and its first open question are updated.
10. **S5 m25's note said S3 and S4 do not list the note 907.** They do. Fixed.

After the fixes, the exit-against-entry diff finds one gap: var 53 at S5/S6
(open problem 3). The id check finds one id outside index.json: S4 m0's room
0 (open problem 7). Every inventory now matches across each boundary.

### Open problems (listed, not fixed)

1. **Engine configuration is fixed per run, but the segments need different
   settings. This blocks a single end-to-end movie.**
   - `skip_text` and `skip_cutscenes` are set once per engine start
     (src/speedrun/engine.py:108-109, 255-258).
   - S1 and S2 assume both are on.
   - S3 needs both off from m90 to its exit. With the cutscene skip on, the
     bridge Escs the store reading on its first frame, so no slide forms, and
     the '.' skips shorten the carry.
   - S4 wants the text skip on again for global 70's tail.
   - S2's explicit Esc moves 14, 20 and 52 may never see their `after` while
     the auto Esc is on (S2's entry notes say to drop them in that case).
   - Talk speed is also one run-wide value. S3's compass windows assume 255.
     S5's book page budget and S6's book survival want slow text.

   Fix: a control-socket command that switches the two skips between moves,
   plus one talk speed chosen by measuring S3, S5 and S6 together. Until then,
   every '.' and Esc has to be an explicit move in every segment.
2. **Predicted hidden inventory slots in the merged run.** The executor
   (`target_point`) raises RouteError when a clicked inventory object is
   scrolled out of view. Global 9 shows two rows of four. Every pickupObject
   jumps the view to the last two rows (`runInventoryScript(1)`,
   global/script-009.txt [0012]-[0017]); setOwnerOf only clamps.

   The view was simulated from the acquisition order: fish before pot and meat,
   sword after the safe, S1's recorded foyer order, removals closing up the
   list. The simulation agrees with S4's predicted Part II order and S6's
   predicted entry view (var 118 == 2), which use the same model. Nothing else
   moves the view: Var[118] is written only by global 9 and the scroll arrows
   in global 4 ([02ED], [02FC]), and global 17 [005D] calls script 9 without
   an argument, so it only clamps. The simulation finds these clicks out of
   view:

   | Move | Object | Row | View | Cheapest fix (one arrow click, verb 208, does not stop a walk) |
   |---|---|---:|---|---|
   | S1 m60 Give meat | 566 | 0 | rows 1-2 | scroll up once during S1 m58-59 |
   | S2 m10 Give fish | 568 | 0 | rows 1-2 | scroll up once during S2 m9 (map walk); the view then stays at 0 through S1 m96 |
   | S1 m90 map teleport | 442 | 0 | rows 1-2 | covered by the S2 m9 scroll |
   | S1 m96 dig | 396 | 0 | rows 1-2 | covered by the S2 m9 scroll |
   | S3 m49, m64 cable + chicken | 377 | 0 | rows 1-2 | scroll up once during S3 m48 |
   | S3 m98 map in Otis's cutscene | 442 | 0 | rows 2-3 | two scrolls up after m97 (the mugs need row 3 until then), inside the slide gate; the optional compass stalls then need one scroll down |
   | S5 m9 fort shield | 732 | 1 | rows 2-3 | read the book 74 instead (row 2, visible from the S5 entry to the leaflet), or scroll up after has(882) (+1 frame; no clicks in the reach) |
   | S5 m25 monkey skip | 907 | 1 | rows 2-3 | read the book 74 instead; S5's own note lists it as a fallback |

   The book's reading has the same shape as the compass and the note:
   cutscene([2]) then beginOverride (global/script-148.txt [001F]-[0027] against
   obj-0732 [0021]-[0026] and obj-0907 [0015]-[001A]). Two moves cause S1 and
   S2's hidden slots: S2 m4 (fish) and S2 m5 (sword) push S1's inventory to
   three rows. S1's standalone notes, such as m69's, do not see this.

   Fix: executor auto-scroll (read vars 133-140 and var 118, click 208/209
   during the preceding walk). Explicit moves would renumber the files.
3. **The slide contract between S5 and S6 does not match.** S5's exit promises
   only var 53 != 0, because the book regain may fail. S6's entry requires
   var 53 == 2, and its whole route (talk inside the book, necklace and deck
   walks, feather skip) needs the slide. If var 53 == 1, S6 must switch to its
   no-slide fallbacks (feather, grog and rat route) or reload a save made
   before the cannibals. Either S6 gets a no-slide variant file, or the run
   treats var 53 != 2 at S6 entry as a reload. The same check sits at S4 m6
   (var 53 == 2) and at S5's entry (var 53 == 1 inside the intro cutscene).
4. **Church door 438 and the "only if closed" doors are decided in S1.**
   - Wandering pirates open and close 438 while var 196 < 3
     (room-034-high-stre/local-200.txt). If ego leaves room 34 inside that
     4-frame window, the door stays open for the rest of the game.
   - S7 m7's `after` {state 438 eq 0} would then never hold.
   - All of S1's room-34 exits (m42, 46, 59, 68, 75) decide this. S1's exit
     now asserts state(438) == 0.
   - S3 m69, 74 and 89 ("open the door only if closed": 428, 316, 437) depend
     on S1's door states the same way.
   - The executor does not test `until` before acting, so an Open on a door
     that is already open costs a sentence.
   - Fix: conditional moves (skip_if) in the executor, or separate variant
     files.
5. **The "Last Part" card costs about 124 s against the record.** The user has
   confirmed the CD audio tracks are on the disc
   (~/Downloads/torrents/TheSecretOfMonkeyIslandUSAEnFrDeEsIt/).
   - Sound 107 plays all of track 6 (129.8 s), and global 131 has no override
     (S7 entry notes).
   - The record's card lasted a few seconds: at most about 4 s by the dock
     arithmetic, 6 s or more by runner-sources §2.1. So the record's setup did
     not play track 6 in full.
   - This is a rules decision (rules/anypercent.md "Music check"), not
     something an engine run will settle.
   - It is the single largest time item in the route.
6. **Executor features the segments assume.** Grouped:
   - `choose` is given as text, but `Move.choose` is a digit. S6 m18-22
     (room 86) and S7 m32, m35 (room 89) select by mouse y, and S7's clicks
     need y >= 144, which `_click` rejects.
   - The K-ESC, K-DOT and K-CHOOSE repeat-press rules (S3, S5 m47/m62, S6,
     S7): `play` presses a key once.
   - kind=glitch moves that must bypass `wait_gate`'s cs == 0 test: S3
     m92-115, S4 m3-5, 15, 25, 34, 43-44, 68, 72-73, S5 m9-10, 18-19, 25-28,
     42, S6 m1, 17-29, 38-41.
   - Scroll-only moves with no verb, object or key (S4 m48, 51, 54, 55; S6
     m24, 27, 28) and obj2-only completions (S4 m44, m73).
   - Dynamic targets (S6 m7-12, `obj_from_var` 164, a field outside the move
     format).
   - S2's intercept moves (m22, 30, 36) and reply-by-insult moves (m28, 33,
     54-58, from var 240).
   - Keys in a room-load frame (S4).
   - Right clicks (S1, S2 doors 564 and 591, S7 door 438).
   - Per-move frame limits (S3 m92 retry).
   - Conditional and optional moves (S3 stall blocks 100-102, 104-106,
     108-110; problem 4).
   - Inventory auto-scroll (problem 2).
7. **Format points.**
   - S4 m0 has room 0, the only id outside index.json's rooms. It is the null
     room that global 122's loadRoom(0) leaves.
   - S4's entry.room says 83 while its flags say {room 0}.
   - S3 keeps an `exit.notes` key, and S6 uses `obj_from_var`.
   - S3's exit inventory lists what survives global 122. At the exit frame,
     388, 396, 578 and the mugs are still owned until global 122 [015C]-[0180]
     runs after the card's Esc. Nothing reads them in between.
8. **The global 70 handoff (S3/S4).** S3 argues the handoff is always in
   cutscene 1 (case B; any other state kills the exit sentence or freezes
   local 200). S4 still models case A. The engine run decides it, together
   with S3's carry budget (about 66 unfrozen frames) and its stall subsets.

### Items: duplicates, order, unused

- **Picked up twice:** only the gunpowder 101 (S4 m30 for the pot, m65 for the
  cannon). The pot hands the first one to owner 15 (room-014-sh-galley/
  obj-0158-cooking-pot.txt [0145]-[0149]). Room 69 is entered twice in S5, but
  its idol 761 is taken once.
- **Used before obtained:** none in the merged run. S1 m22 uses the fish (S2
  m4), which is why S1 cannot run alone without its noted standalone edits.
  The note 907 (S3 m3) serves S3 m44 and S5 m25. The compass 732 (S3 m41)
  serves S3, S5 m9 and S6 m32. The book 74 (S4 m26) serves S5 m42 and S6 m16.
  The leaflet 902 (end of S3) serves S5 m65. The key 269 (S5 m72) serves S6 m0.
  The root 823 (S6 m53) serves S7.
- **Never used:** only forced pickups. The staple remover 643, Manual of Style
  641 and wax lips 642 come from the foyer scripts. Pamphlet 901 and brochure
  903 come from the end of Part I. The bananas 282/283 are needed to start the
  cannibal talk. The trial proofs are 578, 752 and 596. The ink 82 and the
  mints 395 go in the pot but stay in the inventory (obj-0158 has no
  setOwnerOf for them).
- **Flags assumed but never set:** none left after the fixes. Bits 84-86, 156
  and 238 have no literal writer. They come from indexed writes:
  `Bit[83 + Local[0]]` for the trials (global/script-071.txt [008D]), and
  `Bit[140 + n]` and `Bit[222 + n]` for learned insults and comebacks (for
  example global/script-082.txt [0287]). S1's and S2's untils already check
  them.

## RNG points and glitches, in game order

The RNG stream is one sequence from boot to the end. Every frame spent in a
menu wait (getRandomNr(1) each frame), and every background animation, shifts
all later draws. The hard draws are therefore fixed in order: the store first
(seed filter), then S2's steered fights (menu-delay steering), then S6's
catacombs, which are fixed by the stream at the first room-39 entry. S5's notes
mention an optional reseed: save, restart the engine with the same seed, load.
Load time counts; check that against the rules before relying on it.

| # | Where (merged order) | Type | What | Treatment |
|---:|---|---|---|---|
| 1 | S1 m1 | glitch | Logo speed glitch: Esc on boot frames 46-92 leaves var 19 at 5 for the run | Fixed frame window; every handoff asserts var 19 == 5 |
| 2 | S1 m7-8 | RNG | Cook's target x (330 + rand 28) and kitchen timer | Timing only |
| 3 | S2 m2-4 | race | Fish on gull flight 2: pickup about W+13/14 against the re-guard at W+15 | Engine check; fallback key on W+2 |
| 4 | S1 m19 onward | RNG | Every menu wait draws once per frame | Answer on the first accepted frame; the frame count is part of the stream |
| 5 | Part I rooms | RNG | Background draws: bar, tent, Voodoo shop, store, street walkers, forest fireflies | Stream only |
| 6 | S1 m42-75, every room-34 exit (m42, 46, 59, 68, 75) | RNG | Street pirates open church door 438 (only while var 196 < 3) | Must leave it closed for S7 (problem 4) |
| 7 | S1 m46 | RNG | Store first entry: safe combination var 221-224 and storekeeper presence (p 1/4 away; 1-1-1-1 p 1/256; joint 1/1024) | Hard seed filter; presence is rerolled on every entry |
| 8 | S1 m48-51 | glitch | Credit Early: open the safe with the storekeeper away | Needs #7 |
| 9 | S1 m77-79 | RNG | Foyer theft and Fester animations, 0-2 draws | Stream only |
| 10 | S1 m83 + S2 m9-28 | RNG | Map pirates (spawn, destination, 0-24 frame vanish), pirate type and skill at the encounter, fight 1 insults (E3 must draw insult 16) | Intercept procedure; steer by menu delay |
| 11 | S1 m90 | glitch | Forest map teleport: Look at the map in the trapped pseudo-room 207 | Snap point and 207 boxes not decoded; map must be visible (problem 2) |
| 12 | S2 m30-40 | RNG | Fights 2-3: fork wait; fight 2 E1 must be countered | Steer by menu delay |
| 13 | S2 m41-49 | RNG | Forest birds | Cosmetic; check they never cover a clicked path pixel |
| 14 | S2 m53-58 | RNG | Carla's insults (any answerable one not yet used) | Steer by menu delay; S2's menu-wait steering replaces save/reload |
| 15 | S3 m7-41 | RNG | Stan's price prompt variant, sign lights every frame | Stream only |
| 16 | S3 m90 | RNG | Storekeeper presence on the crew-phase store entry | Not required; his greeting can replace the compass text: measure the slide window both ways |
| 17 | S3 m91-93 | glitch | Store compass item slide (read while walking to the pillows) | Retry from a fork on failure (var 352 back to 0) |
| 18 | S3 m98 | glitch | Map page restores hotspots inside Otis's cutscene | Map must be visible (problem 2) |
| 19 | S3 m100-116 | glitch | Carry Otis's cutscene (the slide) to the end of Part I; optional compass stalls J/H/L | Exit check var 352 == 2 |
| 20 | S4 m3-6 | glitch | Act during the carrier's tail; slide armed at m6 (var 53 == 2) | Retry rule; first reach waits for var 352 == 0 |
| 21 | S4 m14-16 | glitch + RNG | Cereal while walking; 20 cosmetic getRandomNr(20) for 'Crunch' | Fixed count |
| 22 | S4 m25, m34 | glitch | Walk during the book and wine discoveries | |
| 23 | S4 m39-57, 72-73 | glitch | Verb keys, inventory clicks and scrolls inside reaches | Fork-test once (LeoLitz warns against it) |
| 24 | S4 deck and hull | RNG | Crew idle (1 per 36 frames), hull sounds | Stream only |
| 25 | S4 m68 | glitch | Early stop by the cannon during "Monkey Island ahead" | |
| 26 | S4/S5 frame | RNG | Global 45 birds: 8 draws in the exit frame | Stream only |
| 27 | S5 m9-11 | glitch | Reading freeze shield: a reading freezes fort local 202's timer | Book instead of compass if the compass is hidden (problem 2) |
| 28 | S5 map rooms 2-6 | glitch + RNG | Map instant exits and fast map walk; global 24 draws every map frame, plus the map monkey | The draws are stream only, but they move S6's catacombs |
| 29 | S5 m17-20 | glitch | Oars skip: off-room instant exit during the dam blast, interrupted exit walk, row in the slow tail | var 19 == 12 during global 44; the beach walk waits for var 19 == 5 |
| 30 | S5 m24-28 | glitch | Monkey skip: a reading holds the gate open; needs var 2 == 624 | Discard the branch if the camera did not pan |
| 31 | S5 m42-64 | glitch + RNG | Book slide regain through both cannibal talks; village idle draws | Page budget unmeasured |
| 32 | S6 m1 | glitch | Pre-walk during script 94 | |
| 33 | S6 m3, m13 | glitch | Head variant: head away on arrival, talk from walk box 8 (the record's method) | |
| 34 | S6 m7-12 | RNG | Catacombs: screen set var 264 and layouts, 6 correct exits | The key RNG point after the store; simulate from the stream at the first room-39 entry |
| 35 | S6 m16-23 | glitch + RNG | The book carries the slide across the head dialogue. RNG: the dissolve into room 86 draws once per block (global/script-140.txt [013E]), and the head's delays are 0-50 jiffies per choice | Hard check var 352 == 1 when var 32 turns 4 |
| 36 | S6 m25, m29 | glitch | Necklace skip twice (room 70 box 8, deck box 5) and the deck walk during script 78 | |
| 37 | S6 m38-41 | glitch | Feather skip: grease before the rat with the slide | Fallback: feather, grog and rat |
| 38 | S6 ghost ship, Bob | RNG | Background and menu draws | Stream only; nothing later depends on it |
| 39 | S7 | RNG | LeChuck punch destinations (failure path only); candles, sign lights | Discard punch branches |
| 40 | S7 m5-8 | glitch | Shmoovement on the up-left walks | Avoid the room-35 spectre trigger (x < 299, y > 120) |

## Open questions for the engine runs, most important first

Two decisions come before any run: problem 1 (a runtime skip toggle and one
talk speed) and problem 5 (the card rules). With those settled:

1. **The slide chain end to end.**
   - S3 m92 forms the slide at (300,141), with and without the storekeeper.
   - Var 352 == 2 holds at S3's exit, and the carry budget is enough with each
     stall subset.
   - Var 53 == 2 at S4 m6 and var 53 == 1 inside the crunch. S5's entry sees
     var 53 == 1.
   - S5's book regain ends with var 53 == 2.
   - S6's book outlives global 140 [0C5B].

   Every glitch from #17 to #37 rests on this chain.
2. **The inventory view at every cross-segment click.** Read var 118 and vars
   133-140 at S1 m60, S2 m10, S1 m90, S1 m96, S3 m44-98, at S4 galley 2, S5
   m9, S5 m25 and at S6's entry. Confirm problem 2's predictions before
   building auto-scroll on them.
3. **The seed and the stream.**
   - Is the CD RNG reproducible under the bridge?
   - Find a seed with the storekeeper away and 1-1-1-1 at S1 m46, and the
     reroll cost if not.
   - S2's steering delays: about 27 frames expected over Carla, with the two
     hard targets.
   - The catacombs simulated from the stream at S6 m7, and whether the S5 frame
     count can be shifted to get six short rooms. S6 also lists saruya's
     CD-only catacomb inconsistency.
4. **Global 70 at the S3/S4 boundary**, case A or B (problem 8), and S4's
   walk-through against waiting in the cabin (up to about 17 frames).
5. **S5 skips:**
   - The fort shield against the plain exit.
   - The oars skip timing inside global 44's slow tail.
   - var 2 == 624 at state(142) == 1 with a direct nose approach.
   - The (319,123) to (308,117) snap.
6. **Book page budgets:** S5's 10 releases of global 148 against 12-14 gaps,
   and S6's 10 message ends and 80 unfrozen frames. Both depend on talk speed.
7. **Door states after S1:** state(438) at S7's entry, and 428/315, 316/570
   and 437/387 at S3 m69, 74 and 89 (problem 4).
8. **Crash rules that contradict each other.**
   - S3 rejects R2's reach-cutscene crash rule, which also contradicts S4's
     crash rule 7. One fork test settles both.
   - Esc during the galley and Part III stand-ups: the code suggests a crash,
     the runner says it is safe. The route presses only '.'.
   - Verb keys inside reaches (S4).
9. **Single-frame margins in S1 and S2.**
   - The fish pickup on flight 2 (S2 m4).
   - The camera bound for the X at S1 m95 (actor_x >= 296).
   - var 25 == 1 visible after the cake Open (S1 m72).
   - The box-5 step at S1 m17 (216 against 218).
   - The forest-teleport snap point and path 685 (S1 m90).
10. **Measurements still owed by single segments.** These are listed in each
    file's `open_questions`:
    - Smirk exit 594 against 592; Carla option 1 + Esc against option 2.
    - Stan plan A against the record's plan; one mug transfer or two.
    - The Hook Isle one-click door.
    - S4's walk gains on real walkboxes.
    - S6's room-70 and room-77 box checks; the crate fanfare length.
    - S7's dock walks; which overrides the record skipped; the room-59 click
      that must resolve to 734.
