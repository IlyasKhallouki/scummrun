# Any% runner sources: full-game route, tricks, versions and settings

This document collects what the human *Secret of Monkey Island* (MI1) speedrunners know about the full-game Any% route, as of 2026-10-19. It covers the current world-record route part by part, a catalogue of tricks and glitches, the choice of game version, ScummVM settings, tool-assisted work on related games, and the leaderboards. It builds on `docs/related-work.md` §1-2, `docs/human-route.md` and `docs/comparison.md` §1.4 and §2, and does not repeat what they establish unless a point is needed here.

All sources were accessed on 2026-10-19. Bracketed IDs point to the reference table at the end.

Status tags:

- **D (direct):** I fetched the primary source: the API record, the guide text, the forum post, the video frame, the auto-caption track, or the source file.
- **U (unverified):** secondhand, or a claim inside a primary source that I could not check myself.
- **Local:** read from this repository (decompiled game scripts, the pinned ScummVM checkout). These are primary sources too.
- **Inference:** my reading of the sources, not something a source states.

Two caveats on the evidence:

- Auto-captions (YouTube ASR) are D for content but not for exact wording. I paraphrase them and never quote them.
- Timestamps written as "timer m:ss" are the in-video LiveSplit timer, read from frames I extracted. Part boundaries come from the first frame of the next part and are good to about ±1 s.

## 0. Findings that matter for route planning

1. **The record route is a DOS CD route built on "item slides", and the floppy versions cannot currently do most of it.** saruya's 21:52 record (2026-08-23) is a CD run whose split file is titled "Saves (CD)" (D, [V-WR]). In August 2026 LeoLitz published three item-slide sequence breaks for CD: the oars skip, the monkey skip and the "feather skip" on the ghost ship [YT-SLIDE, D]. The floppy guide documents only the monkey skip for EGA and VGA floppy, using a different way to obtain the slide [G-MONKEY-FL, D]. saruya ran both: "The sequence break war has begun. Which version will win? (Spoiler: CD will win.)" [SRC-RUN-SARUYA, D].
2. **Measured from video, the CD record beats saruya's best EGA run (22:59, a week earlier) by 68 s.** Almost all of it comes from Part III before the catacombs (86 s, CD faster). CD loses about 22 s in the Part I crew phase setting up the slide (§4.2) (D, my frame measurements).
3. **In ScummVM 2026.3.0 the DOS floppy releases (SCUMM v4) run 1.39% slower than DOS CD (v5).** v4 uses 59.17 jiffies/s and DOS CD v5 uses 60.00 (Local, `engines/scumm/scumm.cpp`). On a 23-minute run this is at most about 19 s. Mac, FM-Towns, Sega CD, Amiga and Atari ST all run at exactly 60.00 (Local).
4. **The best floppy times are EGA, not VGA, but EGA against VGA is unresolved.** saruya's 2026 floppy runs use an "(EGA)" split file, and hyperformance's 24:23 is EGA ("EGA is still king"). Both floppy releases are v4, run at 59.17 jiffies/s and have snap scroll (§4.3). Each has one documented trick of its own: EGA has the Hook Isle map teleport, which needs ScummVM's minor bug fixes turned off; VGA has a crack position that improves the beach teleport. No source measures one against the other (§4.4). The VGA-floppy runs at places 3, 5 and 6 predate the item-slide route.
5. **Text speed conflicts with scummrun's pin.** LeoLitz says slow text is close to required for the full CD item-slide chain, because the slides ride on long "reading" animations [YT-SLIDE, D]. mike's MI2 TAS used `talkspeed=0` [RENTRY-MI2, D]. scummrun pins the maximum talk speed (`docs/comparison.md` §2.2). The record's input overlay shows no `-`/`+` (text speed) presses during the run, so its text speed was fixed beforehand, at an unknown value (D, my scan of [V-WR]).
6. **The RNG is reproducible for a bot.** A fixed `random_seed` plus a fixed number of RNG calls gives a fixed safe combination, fixed catacombs and fixed insult fights. Humans save and reload through the ScummVM launcher to reseed (Local; [G-CATA, D]). The RNG stream also depends on the music device and the game version (§7). Human seeds such as 2756848129 will not transfer to our setup unless every condition matches.
7. **No MI1 TAS exists that I could find.** That includes the one mike mentioned in July 2025 (§6).

## 1. Leaderboards

### 1.1 Any% (ScummVM subcategory), top 10

The v1 leaderboard API shows each player's best run. All ten are in the ScummVM subcategory, with Saves and Credit Early: Yes [SRC-LB, D]. The version column is my reading of one frame of each video:

- CD: 9 verbs and an icon inventory.
- Floppy: 12 verbs (with Walk to, Turn on and Turn off) and a text inventory.
- EGA versus VGA floppy: decided by palette (16-colour dithering versus 256 colours).

These calls are D for the frame but are my judgement. The EGA/VGA call is the weakest.

| # | Time | Runner | Date | Version (how known) | ScummVM | Seed | Run comment (key points) |
|---|---|---|---|---|---|---|---|
| 1 | 21:52 | saruya | 2026-08-23 | DOS CD (split title "Saves (CD)", frames) | 2026.3.0 | 2756848129 | "We've completed the item slide trilogy!"; missed one Carla insult (-10 s) |
| 2 | 24:23 | hyperformance | 2025-07-14 | EGA (comment "EGA is still king"; frame) | 2.9.1 | 102545962 | "Using MT-32 for music with enabled Mixed AdLib/MIDI mode"; AutoHotkey rebinds |
| 3 | 25:31 | frozenspade | 2025-06-18 | floppy, VGA palette (frame) | 2.9.0 | 102545962 | AutoHotkey rebinds |
| 4 | 25:42 | zergu12 | 2023-10-07 | DOS CD (Twitch frame) | not stated | not stated | "the safe combination was bad" |
| 5 | 26:28 | PeteSwanson | 2025-08-07 | floppy, VGA palette (frame) | not stated | not stated | retimed 26:28 |
| 6 | 26:32 | Wildzman | 2026-05-18 | floppy, VGA palette (frame) | not stated | not stated | none relevant |
| 7 | 26:41 | thewoofs | 2023-08-25 | DOS CD (frame) | not stated | not stated | "quad 1's for the safe, no shopkeeper, and a GOLD catacombs" (YouTube description) |
| 8 | 26:50 | LeoLitz | 2022-07-11 | DOS CD (frame) | not stated | not stated | none relevant |
| 9 | 26:59 | awesomeWaves | 2023-09-03 | DOS CD (frame) | not stated | not stated | none relevant |
| 10 | 27:20 | Ribrack | 2026-05-22 | not determined (the frame shows only the map) | not stated | 102545962 | "102545962" |

Sources: [SRC-LB, D], [SRC-RUNS, D], [YT-RUNFRAMES, D], [TW-ZERGU, D]. The NON-ScummVM subcategory has two runs: MilkToast 38:30 (2026-04-25, Amiga, "WinUAE WHDLoad Version") and Novah 40:13 [SRC-LB, D].

saruya's verified Any% history shows the version moves [SRC-RUN-SARUYA, D] [V-SARUYA-RUNS, D]:

| Date | Time | Version (split-file title) | Seed | Note |
|---|---|---|---|---|
| 2025-05-18 to 06-15 | 29:53 to 27:06 | floppy (2025 runs; one note calls a trick "exclusive to the VGA floppy disk version") | none | saveless |
| 2025-06-19 | 24:55 | floppy ("Any%" split file) | 102545962 | first seeded run: "using a seed so you get credit early every time" |
| 2025-06-30 | 24:45 | floppy; the video notes say VGA | 102545962 | 32 timestamped notes in the video description |
| 2026-08-08 | 23:35 | "Saves (EGA)" | 361522673 | "The sequence break war has begun" |
| 2026-08-14 | 23:15 | "Saves (EGA)" | 361522673 | |
| 2026-08-15 | 22:59 | "Saves (EGA)" | 361522673 | "might be useful for version comparisons. Time for CD and item slide madness!" |
| 2026-08-18 | 24:17 | "On-time" (EGA, credit on time) | none stated | "Looks like CD is going to be faster in credit early runs. But it's uncertain whether it'll be faster in credit on-time runs." |
| 2026-08-20 | 22:43 | "Saves (CD)" | 2257841435 | "the seeds for the floppy versions aren't working for me" |
| 2026-08-22 | 22:24 | "Saves (CD)" | 2756848129 | |
| 2026-08-23 | 21:52 | "Saves (CD)" | 2756848129 | WR |

### 1.2 Swordmaster%, top 3

Swordmaster% times from the logo skip to receiving Carla's shirt, and its rules say "Saving and Loading to reroll RNG is ALLOWED" [SRC-RULES, D].

| # | Time | Runner | Date | Version | Notes |
|---|---|---|---|---|---|
| 1 | 6:07 | hyperformance | 2024-09-21 | DOS CD (frame) | comment "Cozy." |
| 2 | 6:08.500 | zergu12 | 2023-10-02 | DOS CD (frame) | "bad circus and troll, very good pirates and carla - this is only 5 seconds off my sum of best" |
| 3 | 6:26 | mikeSpeedyAdventures | 2024-09-12 | FM Towns (platform id `jm95k79o` = "FM Towns"), ScummVM 2.8.0 | an earlier FM Towns run says "Towns for the music" |

Sources: [SRC-LB, D], [SRC-RUNS, D], [SRC-PLAT, D], [YT-RUNFRAMES, D].

### 1.3 Rules as of 2026-10-19

These are unchanged from `docs/related-work.md` §1.2 [SRC-RULES, D]:

- Timing runs from the logo skip to "when the game fades to black after the final cut scene".
- Keybinds: "Keybindings should stick to the rule of one key press resulting in one game input only".
- "Credit Early*, Saveless and Saves (Using save states) are all allowed".
- ScummVM subcategory: "Any speed benefits only possible in ScummVM are allowed. Any Music Device may be used EXCEPT No Music. ... ScummVM versions 2.8 and above."

No written rule mentions seeds. frozenspade's 2025-06-18 video says "There was a rules update on SRC allowing the use of Set Seed manipulation" [YT-RUNDESC, D for the quote]. The update itself is not in any rules field I fetched, so treat it as U. The page lists PC, Sega CD, Amiga, Atari ST and FM Towns as platforms, not Mac. Moderators added Sega CD, Atari ST and FM Towns on request in 2024 [F-PORT, D].

## 2. The record route (DOS CD, saruya, 21:52, 2026-08-23)

### 2.1 Split times

These are read from the LiveSplit panel at the end of the record video (D, [V-WR]). The final LiveSplit time is 21:52.41 and the sum of best is 21:25.21. The meaning of each split comes from the frame at that time.

| Split | Time | What is on screen at the split |
|---|---:|---|
| Street | 2:31 | Low Street, "Walk to archway" (after the map and the chicken) |
| Bridge | 5:17 | "Use fish with troll" |
| Sign | 8:09 | "Use sign" into Carla's clearing, after the pirate fights |
| T-shirts | 9:05 | leaving Carla's house after beating her, with both T-shirts in inventory |
| Chicken | 11:36 | Hook Isle, "Walk to cable" on the way back |
| Slide | 13:18 | High Street at night, Otis freed while holding the item slide |
| Circle | 16:03 | Monkey Island map, rowing, after the wimpy idol |
| Hell | 17:39 | at the giant monkey head, about to go underground |
| Necklace | 18:29 | ghost ship screen, after the catacombs |
| Showdown | 19:47 | the "Last Part / Guybrush kicks butt" card |
| Ending | 21:52 | fade to black |

Part times, measured from the first frame of each new part (D, frames of [V-WR]):

| Part | Start | End | Duration |
|---|---|---|---|
| I (trials, fights, crew) | 0:00 | 13:30.5 (first frame on the ship) | 13:30.5 |
| II (the journey) | 13:30.5 | 14:48 (Monkey Island beach) | 1:17.5 |
| III (Monkey Island, catacombs, ghost ship) | 14:48 | 19:45.5 ("Last Part" card) | 4:57.5 |
| IV (Mêlée, church, ending) | 19:45.5 | 21:52.4 | 2:07 |

Neither the Part Two nor the Part Three title card is visible, so both were skipped. The "Last Part" card stays on screen for at least 6 s in both the CD and EGA videos (D, frames). Whether it can be shortened was not checked.

### 2.2 Part I, the three trials and the fights (timer 0:00 to 9:05)

The trial route is the one in `docs/human-route.md` §3 (kitchen, circus, petal, map, chicken, Otis, store, mansion twice, underwater), with the swordfight interleave. Differences and additions found for this report:

- **Credit Early at the first store visit.** The record loads a savestate and pulls and pushes the safe at about timer 3:00 (`docs/human-route.md`; mechanics in §3.2).
- **Treasure.** The record reads the map inside the forest at about timer 6:44, and the next frame is a different forest screen (D, frames). This matches LeoLitz's 2022 "forest map teleport" glitch (§3.11). I did not check it frame by frame.
- **Fights.** The record uses the "zergu" three-pirate method with save/reload (§3.9). The troll insult is learned first ("Dia: 1, 2") [G-ANY, D] [YT-WOOFS, D].

### 2.3 Part I, the crew phase with the item-slide setup (timer 9:05 to 13:30.5)

Observed order (D, frames of [V-WR]; scene identifications are my reading):

1. 9:05 leave Carla's, go to the map and the village, then up to the Lookout.
2. 9:43 to 10:09 **Stan's**: the negotiation, including "about extras, already." No later Stan visit appears before Part II, so the ship is bought here (inference).
3. 10:15 map "Sword Master's", then **Carla's house** 10:21 to 10:45 (recruit Carla).
4. 10:49 map "shore", then **Hook Isle** 10:57 to 11:41 (Meathook, "The Governor's been KIDNAPPED!").
5. 11:45 map "village", then the SCUMM Bar and kitchen 12:05 to 12:17 (mugs and grog).
6. Jail 12:45 to 12:49 (talk to Otis first; see below).
7. 12:53 the compass inscription on screen ("When you set sail for good value, all winds blow towards Stan's"), then the **store** 12:57.
8. Jail 13:05 (free Otis).
9. Streets and dock 13:13 to 13:30, then Part II.

How the CD item slide is obtained and carried into Part II (D, [YT-SLIDE] auto-captions, paraphrased):

- **Collect three mugs.** LeoLitz never managed the sequence with two.
- **Talk to Otis first.** Go through the "recruit him" dialogue without freeing him. Without that dialogue the slide cannot be kept when leaving the jail.
- **Get the slide in the store.** Walk toward the pillows beside the counter while reading the compass. The shopkeeper stops Guybrush from going there, and because Guybrush was reading, this produces the item slide. Reading the compass is a pseudo-cutscene: the cursor is taken away for a moment but control remains.
- **Free Otis with the mug while holding the slide.** Guybrush can walk while the freeing animation runs. To leave the jail, right-click (look at) the map and skip that text.
- **Stretch the Otis animation to the end of Part I.** Keep reading the compass, and use `.` only at room edges, because Guybrush cannot cross into a new room while reading. In the end-of-chapter dialogue, skip only up to Carla's lines.
- **Why the slide survives.** The animation ends after the end-of-Part-I dialogue, and these animations restore the state they started in.

LeoLitz describes the slide as a background counter that leaves the cursor active while it is non-zero. He is not sure of the details, and this was not checked in the engine.

The CD cooking guide says the same: "It's possible to get the item slide state at the end of part 1 and carry it over to part 2" [G-COOK-CD, D].

### 2.4 Part II (timer 13:30.5 to 14:48)

From the frames: the captain's room with the cereal ("Crunch"), then "There is an old, dusty book inside" at 13:58 (the dusty book is taken). Next come the hull and gunpowder, then the galley cooking at 14:13 to 14:25, more gunpowder, and the cannon at 14:40 to 14:47 (D, [V-WR]).

The CD cooking guide (saruya, 2026-09-06) gives two routes, "flag after cereal" and "gunpowder after cereal", each with and without the dusty book [G-COOK-CD, D]:

- **Cereal while walking.** With the slide, open the cereal immediately after entering and click toward the next target, so Guybrush walks while opening it.
- **Inventory clicks during use.** During cooking you can press U and click inventory items while Guybrush uses items with the pot. Two restrictions apply: "clicking on the magnetic compass while Guybrush is using an item will crash the game", and "you cannot click on the cooking pot while Guybrush is using an item".
- **The dusty book.** It keeps an item slide alive later, at the cannibals, which allows getting "past the ghost rat without the grog, saving about 10-15 seconds". LeoLitz estimates the cost of taking the book at about 1.4 s [YT-SLIDE, D].
- **Crash windows.** After the cooking cutscene Guybrush stands up. Any click or key other than `.` or Esc before he has stood up and spoken crashes the game, and the same applies when he stands up at the start of Part III. Clicking between the cannon wheels during the "Monkey Island ahead" scroll stops him closer to the cannon [YT-SLIDE, D].

The older non-slide recipe, which also applies to floppy, is in the 2019 guide: "Use Ink, Cinnamon, Cereal, Gunpowder, wine, Jolly roger, chicken and Mints in Pot" plus "Use Feather with Fire" [G-ANY, D]. The floppy cooking guide uses number keys 1 to 6 for inventory items and Q/A to scroll [G-COOK-FL, D].

### 2.5 Part III (timer 14:48 to 19:45.5)

From the frames (D, [V-WR]; identifications mine):

| Timer | Event |
|---|---|
| 14:48 | beach |
| 14:58 | fort ("There's a nifty lens in it": the spyglass) |
| 15:07 | river fork |
| 15:25 | beach with the rowboat (the oars skip result) |
| 15:31 to 15:52 | clearing and monkey-head fence. The note is read ("Yours truly, Captain LeChuck"), the fence stays open, and the wimpy idol is picked up ("I doubt anyone will miss this piece of junk") |
| 15:58 to 16:13 | boat and rowing ("Circle" split at 16:03) |
| 16:16 to 16:58 | cannibal village. Guybrush reads the captain's log ("The directions we purchased on Mêlée proved to be a recipe...") while the cannibals talk, then the hut, then back out |
| 17:04 to 17:19 | rowing back, beach, clearing |
| 17:22 to 17:39 | giant monkey head (Hell split) |
| 17:40 to 18:04 | the underground, then the catacombs (only about two rooms visible) |
| 18:07 to 18:22 | talking to the head without the long animations |
| 18:29 | Necklace |
| 18:49 | "Yikes! I hate rats!", then "Yech, it's all over my hands" (grease taken) |
| 19:25 | leaving with the root |
| 19:34 | cannibals |
| 19:37 to 19:45 | the coast scene ("Yeah, we're out of sunscreen") |

The three CD skips that produce this, from [YT-SLIDE] (D, auto-captions, paraphrased):

- **Oars skip.** The slide arrives from Part II. Blow up the dam with the spyglass lens and the sun; the long blast animation gives more leeway. Walk away during the blast using the slide, then click the beach (an instant teleport) and then the few pixels at the border between island, sea and bottom screen edge. Guybrush must still be walking fast when the map changes. The game then places him on the sea and gives him the boat. This skips going to the crack for the oars. If the click is late, he returns to the fork.
- **Monkey skip (CD).** With the slide still active, open the monkey-head fence and keep it open while reading something. Then walk in and pick up the idol.
  - Herman normally appears in this room if not met earlier, and his dialogue removes the slide.
  - If the screen has scrolled a certain way, Guybrush cannot walk out.
  - A frame-perfect variant (click the notes on the first frame the room loads) avoids Herman entirely, but LeoLitz does not advise it.
  - Losing the slide to Herman on the way out is acceptable once the idol is taken.
- **Getting the slide back for the ghost ship (needs the dusty book).**
  - Pick up the bananas; this is still needed to talk to the cannibals.
  - Read the book while the camera scrolls so the cannibal dialogue starts with the book open, and keep reading the book through both cannibal dialogues. This needs slow text speed and stalling with the compass.
  - Give the leaflet, then walk away early, and walk away early from Herman after the banana picker.
  - In the catacombs and at the ghost ship, read the book while talking to the navigator head. Inventory clicks still work during the slide; hotspots do not.
- **Feather skip (the ghost rat).** Holding the slide, open the hatch and pick up the grease before Guybrush notices the rat. This skips the feather (tickling the ghost's foot), the grog and the rat feeding. Two cautions:
  - Picking up the key: do nothing during the animation.
  - After opening the crate: the old habit of spamming right-click to grab the root now crashes. Wait for the fanfare.

The catacombs use seed RNG manipulation (§3.8). Two head skips and the necklace skip apply on the ghost-ship screen (§3.12).

The pre-slide Part III route, for reference, from [G-ANY, D] and [YT-LEO-GUIDE, D]:

1. Fort: push the cannon, take the rope and gunpowder.
2. River fork: take the "noteworthy rock" (flint).
3. Pull the primitive art twice, then push the rock (Herman appears).
4. Gunpowder on the dam, then the rock with the compass.
5. Pond rope, then beach warp.
6. Crack: rope on the branch and the stump, take the oars.
7. Bananas, then the boat.
8. Village: bananas, dialogue 3, captured.
9. Hut: skull and loose board.
10. Monkey: bananas, then pull the nose, then the wimpy idol.
11. Cannibals: give the idol. Dialogue "1, 1, 1, 4, 2, 3, 1" with an Esc right after the 4, then the leaflet.
12. Banana picker to Herman.
13. The "key" (Q-tip) in the monkey head's ear, then the catacombs.

The floppy monkey skip instead gets the slide at the cannibals by reading the note (§3.6) [G-MONKEY-FL, D].

### 2.6 Part IV (timer 19:45.5 to 21:52.4)

From the frames: dock "I LOVE root beer!" at 19:53, streets, church at 20:12, the grog machine fight at 20:18 to 20:51, map and cliff, ending (D, [V-WR]).

The 2019 guide's text, unchanged in substance by later sources [G-ANY, D]:

- "Go right, SKIP, Dia: 2, go right"
- "Use shortcut, (No need to fight that Ghost) walk to town, enter church"
- "Dia: SKIP, 3, 3, SKIP, 3, SKIP, 4, SKIP"
- "Use Root Beer with Lechuck"
- "Dia: 1, 2 (this can't be selected with keyboard)"
- "Stop Timer when screen fades to black"

LeoLitz adds two points [YT-LEO-GUIDE, D]:

- Aim at LeChuck's left side, because his hitbox sits to the left.
- In the last dialogue, check which "sure" line Guybrush spoke before moving the cursor.

saruya adds that "picking up the root beer is as fast or even faster than using it without picking it up" [V-2445, D]. Whether the church door is open in Part IV depends on whether a random pirate opened it in Part I [F-DOORS, D].

### 2.7 Dialogue numbers for the full route

These are from [G-ANY, D], corrected by [F-GUIDE-ERR, D] and [YT-LEO-GUIDE, D]:

| Conversation | Choices |
|---|---|
| Circus | SKIP, 1, SKIP, 1, 2 SKIP; give pot; 1, SKIP |
| Citizen of Mêlée (map) | 4, SKIP, 2 |
| Storekeeper, first visit | 3, 1, 1, 1, 1, 2. With Credit Early and the storekeeper away: use the bell, then 3, 1×4, 2 [YT-LEO-GUIDE] |
| Otis #2 | 2 (or click the scene to pick the last option; see §3.13) |
| Mansion | any |
| Troll (insult) | 1, 2 |
| Smirk | 1, SKIP, 1, 1, 1, 1, 1, SKIP |
| Pirate fights | 2 to start the first two fights, 3 from the third fight on [G-SAVELESS] |
| Carla | ESC, 2 |
| Stan, credit on time (first visit) | 1, 2, 4, 5 |
| Storekeeper, credit on time | 2, 1 (invisible stairs; see §3.3) |
| Hook Isle | 1, SKIP, 3, ... |
| Stan, second visit | "SKIP, Dia: 3, ... 4, SKIP, ..., 4, 1, 1, 2, 2, 2, 2, 2, 2, 3, 2, 4, SKIP" [G-ANY]; LeoLitz: option 4 if credit on time, 5 with Credit Early [YT-LEO-GUIDE] |
| End of Part I | 1, 1, SKIP |
| Cannibals | 3; later 1, 1, 1, 4, 2, 3, 1 (Esc after the 4 is mandatory, "otherwise the dialog is going to change") |
| Navigator head | 2, 2, 2, 2, 2 |
| Bob (end of Part III) | 3, 2, 1, 1, SKIP |
| Church | SKIP, 3, 3, SKIP, 3, SKIP, 4, SKIP |
| LeChuck | 1, 2 (mouse) |

## 3. Trick catalogue

Version applicability is "both" only when a source says so or the mechanism is in shared script code that I checked. Time savings are the sources' own figures. "Not stated" means no source gives it.

Overview (details in the subsections named in the first column):

| Trick (§) | Time saved | Versions | Inputs | Risk |
|---|---|---|---|---|
| Logo speed glitch (3.1) | "possibly 4 minutes" (2017); frame-paced motion 6/5 faster | CD, floppy, SE | Esc while the logo sparkles run | none; easy to verify by timing |
| Credit Early (3.2) | about 58 s (same runner, EGA, CE vs on time) | both; seeds version-specific | seeded: save N frames before the store, quit to launcher, load; saveless: guess pulls/pushes | resets; safe glitches if inputs are too fast |
| Invisible stairs (3.3) | about 4 s | both | Dia 2, 1, skip the first walk at once, Dia 2 | none (only lose the 4 s); credit on time only |
| Item slides, mechanics (3.4) | enabler | CD (full chain); floppy (cannibals only) | read compass/note/book while moving | ScummVM crashes on actions during pickups or use |
| Oars skip (3.5) | not stated alone; Part III total 86 s vs EGA | CD only documented | slide from Part I; dam blast; click beach, then border pixels | return to fork if late |
| Monkey skip (3.5, 3.6) | about 1:10 (floppy) | CD and floppy (different setups) | slide; fence opened and kept open while reading | Herman removes the slide; crash if clicking during idol pickup |
| Feather / ghost-rat skip (3.5) | "about 10-15 seconds"; 5.5 s measured vs EGA ghost ship | CD only documented | book slide through cannibal dialogues; grease before the rat notices | crash if acting during key or root pickup; needs slow text |
| Shmoovement (3.7) | not stated | both | spam click or hold Enter walking left or up | slower if done walking right or down |
| Catacombs RNG (3.8) | catacombs in 50 s (record) vs many minutes unseeded | both; seeds version-specific | seed, take a wrong exit first, save at the entrance, launcher reload, hold Enter | CD has inconsistent RNG calls that can end the run |
| Insult fights with saves (3.9) | not stated; in the record, Bridge to T-shirts (Smirk, treasure, fights, Carla) takes 3:48 | both | troll insult; save before each pirate line, reload; Carla: allow 4 losses then reroll | reload count is random |
| Early door entry, pixel clicks, Monkey Island movement (3.10) | fractions of a second each; cable click 0.4 s | both; crack spot VGA only | click timing and exact pixels | small losses if missed |
| Beach warp (3.10) | not stated | both | click beach/sea border while Guybrush stands still | retry costs time |
| Herman skip at the rock (3.10) | not stated | both | read the compass walking to the rock, push, leave | Herman may still appear |
| Rowing (3.10) | not stated | both | click placement only | none |
| Forest map teleport (3.11) | not stated | CD shown; floppy not stated | look at the map after the fifth forest move, hold `.` | must be done quickly to save time |
| Head skips, necklace skip (3.12) | about 4 s together | both; floppy variant differs | T then head in a two-frame window; U + head; necklace at the button frame | none if missed (except U on floppy) |
| Crate fanfare skip (3.12) | not stated | v4 floppy with Roland/MT-32 only | music device setting | none |
| Conversation techniques (3.13) | saruya: "several minutes" for a beginner | both | `.`, Esc, numbers, cursor pre-positioning | wrong option costs seconds |
| EGA Hook Isle map teleport (3.14) | not stated | EGA only; ScummVM minor bug fixes off | use the map on the Hook Isle exterior | not shown in a top run |
| Church ending (2.6) | small | both | aim at LeChuck's left side; mouse for the last answer | "Last Part" card shown for at least 6 s |

### 3.1 Logo speed glitch

- **Input.** "At the Lucafilm Games logo, Press ESC when you see the very first 'sparkle' on the L, as long as you skipped once sparkles start happening then you will have the glitch" [G-ANY, D]. aWay0fLife's 2017 chart shows the walk speed unaffected until about 5 s into the logo and unaffected again after the sparkles end [F-SPEED, D]. LeoLitz starts his timer at -4.8 s when he launches the game and presses Esc at 0 [YT-LEO-GUIDE, D]. If you skip and end up stuck in a scene with no inventory, restart [YT-LEO-GUIDE, D].
- **Mechanism.** The DOS VGA script is identical to the Mac one analysed in `docs/research/skips-engine.md` §6. Room 10 local script 204 sets `VAR_TIMER_NEXT = 5` at `[0000]`, runs the 11 sparkle scripts, and restores 6 at `[0097]`. Esc while 204 runs leaves var 19 at 5 for the whole game (Local, `data/scripts-dos-vga/room-010-logo/local-204.txt`). For a bot, the input window is every frame while local 204 runs, which is several seconds wide.
- **Saving.** "possibly saving 4 minutes in all" (2017 estimate) [F-SPEED, D]. Frame-paced motion runs at 6/5 speed (`docs/comparison.md` §1.4).
- **Versions.** Shown working on CD, floppy and Special Edition. MilkToast could not see the sparkle on Amiga [F-CD, D].
- **Risk.** None once it is learned. On the timer it is visible as the walk to the bar door taking about 13 s instead of 15 [F-SPEED, D].

### 3.2 Credit Early (open the store safe at the first visit)

- **Definition.** "Opened the shopkeepers safe early in the run without knowing the code" [F-CREDIT, D].
- **Combination.** It is four numbers, each 1 to 4, made by store room local script 205 (`Var[220+i] = 1 + getRandomNr(3)` four times), which runs once on the first entry to the store, guarded by `Bit[319]`. That gives 256 combinations (Local, `data/scripts-dos-vga/room-030-store/local-205.txt` and `entry.txt [0094]-[009E]`).
- **Input.** "pull the handle of the safe to input the first number, push to input the second and so on". Going too fast at the end glitches the safe, and the only recovery is to leave and re-enter the store [G-ANY, D] [YT-LEO-CE, D].
- **Saveless method.** Guess combinations at the safe. LeoLitz's video shows the patterns he tries and a save-tree method: save after the second and third numbers, then reload to test the remaining digits [YT-LEO-CE, D]. zergu12 once "got 4 (four) credit early dings in a row with 16 combos attempted" [SRC-RUNS, D].
- **Seeded method, which needs save/load.**
  - Run with a fixed ScummVM seed.
  - Save a set number of frames before entering the store, quit to the ScummVM launcher (which restarts the engine and reseeds the RNG), load, and walk in. The combination is then known.
  - Seed 102545962 gives 1-1-1-1, or 3-1-1-1 if the save is on the last frame [F-SAFE, D].
  - The record's seed: "seed 2756848129 ... gives you 4 possible frames for credit early. You need to save the game between 26 and 29 frames before entering the store" [G-CATA, D].
  - "Simply quickloading the game won't work; you need to either go back to ScummVM's launcher (which is fast) or close ScummVM and open it again (which is slow)" [G-CATA, D]. That sentence is about the catacombs, but saruya uses the same quit-to-launcher method for the safe [F-SAFE, D].
- **Saving.** Measured on saruya's two EGA runs from the same week (the 22:59 with Credit Early and the 24:17 on time), T-shirts to Cannon took 5:10 against 6:08, so about 58 s (D, split panels of [V-EGA2259] and [V-EGA2417]). Inference: most of this is the skipped second store visit and the first Stan visit, though the runs differ in RNG too.
- **Versions.** Both. Seeds are version-specific (saruya: "the seeds for the floppy versions aren't working for me" on CD) [SRC-RUN-SARUYA, D].
- **Risk.** For humans, frame-tight saves and resets. For a deterministic bot, the combination follows from the seed and the RNG calls made before the first store entry (inference; §7).

### 3.3 Invisible stairs glitch (credit on time only)

When you get the code on the second store visit, "Dia: 2, 1". "SKIP the shopkeeper walking to the safe the first time as soon as possible", then "Dia: 2", "then he will do the glitch" and walks to the safe on stairs that are not there [G-ANY, D]. LeoLitz: if you miss it, "you're just losing like four seconds" (paraphrase) [YT-LEO-GUIDE, D]. You only need to watch the first three numbers; then push until the safe opens [F-GUIDE-ERR, D]. It is documented on CD and floppy. It is irrelevant with Credit Early.

### 3.4 Item slides: what they are

- **The state.** Guybrush can move while a "reading" pseudo-cutscene or a background animation is running. Reading the compass, a note or the book takes away the cursor but leaves control. "This state will allow you to move Guybrush during cutscenes" [G-MONKEY-FL, D] [YT-SLIDE, D].
- **Hotspots.** While sliding, hotspots in the room are disabled. A click on an object counts as a click on the floor behind it, but inventory clicks work [YT-SLIDE, D].
- **What removes it.** Most dialogues. Cutscenes that save and restore the state can give it back, which is how Otis's animation carries it through the end of Part I [YT-SLIDE, D].
- **Crashes.** These are ScummVM crashes:
  - "clicking on an item while Guybrush is picking it up, or moving while Guybrush is picking up an item" [G-MONKEY-FL, D];
  - "clicking on the magnetic compass while Guybrush is using an item" [G-COOK-CD, D];
  - acting while Guybrush stands up after cooking, at the start of Part III, or while he picks up the ghost-ship key or the root [YT-SLIDE, D].

  saruya: "Finishing a run without crashing the game is getting harder" [SRC-RUN-SARUYA, D]. Inference: whether the original interpreter crashes in the same places was not checked. For a bot these are hard constraints to model.

### 3.5 CD item slide trilogy (oars, monkey, feather)

The sequences are in §2.3 to §2.5. Sources give these savings:

- Monkey skip: about 1:10 on floppy [G-MONKEY-FL, D]. No CD figure is given.
- Ghost-rat skip with the book: "about 10-15 seconds" [G-COOK-CD, D].
- Dusty book: costs about 1.4 s [YT-SLIDE, D].
- Oars skip: not quantified.

My measurement of the whole chain is in §4.2: CD is 86 s faster than EGA in Part III before the catacombs and 5.5 s faster on the ghost ship.

Version: CD. LeoLitz's video is CD. The floppy guide points CD runners to it and documents only the monkey skip for floppy [G-MONKEY-FL, D]. **Whether the store/compass slide and the oars or feather skips work on EGA or VGA floppy is not stated anywhere I found.** saruya's view after trying both: CD wins with Credit Early, and with credit on time it is "uncertain" [SRC-RUN-SARUYA, D].

### 3.6 Floppy monkey skip (EGA and VGA floppy)

The guide was written 2026-08-09 [G-MONKEY-FL, D]:

- **Cannibals slide.** "walk to the right and start reading the note right before the conversation with the cannibals begins". It is easy with screen scroll. With screen snap it is "about a second and a half or two seconds faster, but it becomes a frame-perfect trick".
- **Inputs during the cannibal dialogue.** Esc once (twice ends the reading), "3", period, then Esc twice.
- **Leaving the hut without crashing.** Click the skull, P, click the skull, P, then hold Enter or spam-click the loose board.
- **The skip itself.** At the clearing, switch to screen snap (Ctrl+R). Pull the nose, read the note, walk right below the monkey's ear while holding Enter, then press Esc and click without moving the cursor. Wait until the idol pickup has finished before clicking anything, "or you'll crash the game".

### 3.7 Shmoovement (repeated walk clicks)

- **Input.** Spam-click or hold Enter (Enter acts as a left click) while walking left or up, moving the mouse. Do not do it walking right or down [YT-LEO-GUIDE, D].
- **Theory (LeoLitz, 2025).** Guybrush's speed is fractional, so steps alternate long and short. When walking left or up the first step is long, so restarting the walk repeatedly helps. "maybe TAS could find some weird dances that are faster for certain walks" [F-CONV, D].
- **Other descriptions.** thewoofs: "rapidly entering in LMB or Enter inputs while traveling left or up on certain screens ... to force Guybrush to take 'bigger steps'". It "greatly varies in success screen to screen". mike's original write-up is in Discord, which I could not access [F-CONV, D].
- **Saving.** Not quantified. Versions: both.
- **Inference.** This is an engine-level effect of how a fresh walk starts its step sequence. A bot can exploit it with walk-target re-issues, but it needs measurement in `actor.cpp`'s walk code. It would need a rules decision under scummrun's own rules.

### 3.8 Catacombs RNG manipulation

saruya's guide (2026-07-22) [G-CATA, D] gives the full generator:

- **Screens.** 18 screens in three sets. The set is chosen with `rand(0..2)`, rerolled until it differs from the previous room's set. Then come the left and right screens. A room with fewer than two exits is rerolled.
- **Entrance and exit.** The entrance is `rand(0..4)` until the cave is in the room. The exit is `rand(1..5)` with reachability checks, and the two cave orderings differ.
- **Counter.** Six net correct exits are needed. A wrong exit decrements the count, and a wrong exit through the entrance throws you out.
- **Bug-like extra exit roll.** An extra exit calculation happens when the previous correct exit was on the right-side screen.
- **Timing caveat.** "the game will give you control of Guybrush as soon as it's chosen the entrance", so leaving too early counts as wrong.

Manipulation:

- Take the wrong exit first. "It's faster, and increases the chances of finding good seeds by a factor of 60."
- Save next to the entrance after reading the compass while walking, which makes the frame count deterministic. Quit to the launcher, load, and hold Enter.
- Seeds that also give Credit Early: 361522673 (fastest catacombs; Credit Early window 2 to 5 frames), 1095696648 (16 to 19 frames, but affected by Fester's random number of RNG calls, 0 to 2), and **2756848129** (works with or without the extra initial exit calculation; 26 to 29 frames).
- "Limitations": "When playing the CD version, there are two RNG calls that aren't always consistent". Sometimes 6 RNG calls happen instead of 5, and then "chances are the run is over".

saruya's seed finder (C++, resource 2z7qm, 2026-07-23) prints such seeds. Its description says `fastest_catacombs_seeds.txt` holds "seeds that give you at least 4 frames of credit early and the fastest possible catacombs" [RES-SEED, D]. Its zip was blocked by Cloudflare (HTTP 403), so the source was not obtained.

Unseeded rules of thumb: wait for the head to spin, wait for the screen to scroll before backtracking, and test vertical corridors by standing beside them [G-ANY, D] [YT-CATA101, D]. In the record the catacombs take 50 s from the monkey head to the ghost-ship screen, the same as in the EGA 22:59 (D, §4.2).

### 3.9 Insult swordfights (Any% Saves) and Carla

- **Basics.** You start with two insults ("cow" and "feather duster"), and the troll gives the "dog" insult. Carla has 17 insults. The multi-responses are "family reunion" (three Carla insults), "smell your breath", "heard of YOU" and "hemorrhoids". Win chances against Carla by number of responses: 5 = 2.88%, 6 = 12.05%, 7 = 28.58%, 8 = 50%, 9 = 71.41%, 10 = 87.95%, 11 = 97.12%, 12 = 100% [G-ANY, D].
- **Current Saves method ("zergu three-pirate").** Get the troll insult. Then, on each pirate, save before the pirate answers or insults, and reload until he gives what you want:
  1. Pirate 1 learns the answer to "dog".
  2. Pirate 2 provides a multi-response insult such as "apes".
  3. Each fight is won by reloading until the pirate loses.

  After three wins, go to Carla [YT-WOOFS, D]. thewoofs: "we used to grab two multi responses I'm pretty sure now we only do one".
- **Carla, newer option (LeoLitz 2024).** Carla never repeats an insult in a fight. So do not save at first: accept up to four losses, then save and reroll. The remaining pool then holds a higher share of insults you can answer. Whether this is faster on average "really depends" on how fast you reload [YT-LEO-SAVES, D].
- **Saveless details.**
  - Pirates answer correctly 72.72% (blond), 63.63% (dark-haired, bald), 54.54% (hat) and 36.36% (scarf) of the time.
  - Giving up ("hold A, 6") is faster than the losing animation.
  - Notes on keeping track of insults [G-SAVELESS, D] [G-ANY, D].
- **Pirate placement.** Entering the fork resets where the pirates stand. Do the treasure on the first fork entry so the reset is free [YT-LEO-GUIDE, D].
- **Seeds and fights.** Inference: humans reroll with in-session quickloads, which do not reseed. A seed does not fix their fights. A frame-deterministic bot with a fixed seed could pick input timings that give a 5-0 Carla without any reloads. Nobody has documented this.

### 3.10 Early door entry, pixel clicks and Monkey Island movement

- **Doors.** "it's possible to enter most doors at the beginning of the door opening animation" [G-ANY, D]. Right-click opens or closes doors and talks to characters [YT-LEO-GUIDE, D].
- **Cook timing.** Enter "while the chef's elbow is still visible". The chef "will always leave the kitchen 10 seconds after you try to enter" [G-ANY, D].
- **Monkey Island map screens.** Clicking exactly on a destination makes Guybrush run there; clicking anywhere else makes him walk slowly [YT-FROZEN, D] [YT-LEO-GUIDE, D].
- **Beach warp.** Click the border between beach and sea while Guybrush is standing still [YT-LEO-GUIDE, D].
- **Crack.** "This is exclusive to the VGA floppy disk version: if you enter the crack at the right spot, Guybrush will be at the ideal position after teleporting to the beach" [V-2445, D].
- **Rowing.** No speed trick beyond click placement ("aim for the deep blue", "light blue") [YT-FROZEN, D]. After the first use you can just click the boat [G-ANY, D].
- **Herman skip at the rock.** Look at the compass while walking to the rock, Esc near the rock, push it, and leave before Herman arrives [YT-FROZEN, D] [YT-LEO-GUIDE, D].
- **Dam.** Either the gunpowder then the rock used with the compass while walking [G-ANY, D], or the spyglass lens with the sun, which the CD oars skip prefers [YT-SLIDE, D].
- **Hook Isle cable.** "Clicking on the cable during that specific frame makes Guybrush get there 0.4 seconds faster" [V-2445, D].
- **Stan.** Refusing extras lowers the price by 450 each (the first refusal excepted), and walking away lowers it by 1000/500/100 [F-SOLUTIONS, D].

### 3.11 Forest map teleport (treasure)

Do the forest moves up, left, right, left, then up. Then click the map in the inventory (holding `.`), and when the map closes Guybrush is teleported. Finish with right, left, up. It "only saves time if you do it decently quick" [YT-LEO-3A, D]. Versions: shown on CD (LeoLitz, 2022). Not stated for floppy. The record appears to use it (§2.2).

### 3.12 Navigator head and ghost ship skips

- **Head skip at room load.** "There are two frames before the cutscene starts where you can actually control Guybrush." T in the first, then click the head (or its number on floppy) in the second [G-HEAD, D]. LeoLitz: two glitches here, worth about four seconds together, with no loss if missed (paraphrase) [YT-CATA101, D].
- **Faster variant.** U plus the head, then walk to where the path down starts, then talk. Guybrush neither pulls out nor puts away the head [G-HEAD, D] [F-HEAD, D].
- **Floppy difference.** On floppy, clicking near the ghost ship sends you back to the entrance after the talk, and a missed U leaves you stuck in a Use action [G-HEAD, D].
- **Necklace skip.** Use the necklace while walking at the moment Guybrush's trouser buttons disappear behind the rock ("you have like five frames") [YT-CATA101, D] [G-ANY, D].
- **Ghost ship.** "Use feather twice on ghost pirates foot, pick up grog" (non-slide route) [G-ANY, D]. The feather can be picked up instead of taken from the chickens, and the grog only once it has turned blue [YT-LEO-GUIDE, D]. On CD with the book slide the feather and grog are skipped (§2.5).
- **Crate fanfare.** In the DOS VGA (v4) crate script the fanfare wait is skipped when `VAR_SOUNDCARD == 4` (Roland/MT-32) (Local; §5.1). hyperformance: "EGA patch with MT-32 set up skips the crate fanfare in the ghost ship" [SRC-RUNS, D].

### 3.13 Conversation techniques (all versions)

From [G-CONV, D] and [F-CONV, D]:

- `.` skips one line; holding it skips several.
- Esc or both mouse buttons skip some conversation fragments. Both buttons work while holding `.`.
- Number keys 1 to 6 choose answers.
- Clicking the game area picks the "leave" option.
- Clicking the scene in some dialogues picks the last option [YT-LEO-GUIDE, D].
- Hold Esc through runs of cutscenes, but Esc blocks all other input while held [G-ANY, D].
- Lower the OS key-repeat delay.

These matter for human input speed. Inference: a bot injecting key events at the right frame gets the same effect.

### 3.14 Other notes

- **Ctrl+W.** The original instant-win key. It is unmentioned in the rules and would fail the timing rule (`docs/related-work.md` §1.2).
- **EGA Hook Isle map teleport.** Using the map on the Hook Isle exterior teleports you to the start of the room, and could replace the chicken on the way back [F-EGA, D]. ScummVM blocks it by default (§5.3).
- **Kitchen door.** It is open or closed in the crew phase depending on whether the cook closed it in Part I [F-DOORS, D].
- **Ghost key bug.** Repeating it adds inventory entries until the game crashes; no speed use [F-GHOSTKEY, D].
- **Seven-exit catacombs, and getting stuck in the catacombs.** Fixed by using the head [F-CATA, D].

## 4. Version choice

### 4.1 History

- **2019.** "We all just use the cd-rom version by default" (aWay0fLife) [F-CD, D].
- **2023.** Top runs were CD (zergu12, thewoofs, LeoLitz, awesomeWaves frames) [YT-RUNFRAMES, D].
- **2025.** Seeds made Credit Early routine. hyperformance's EGA run took the record (24:23, "EGA is still king") [SRC-RUNS, D]. saruya ran floppy, VGA per his own notes [V-2445, D].
- **August 2026.** LeoLitz's item-slide video (2026-08-07) [YT-SLIDE, D]. saruya ran EGA floppy runs down to 22:59, then CD runs (22:43, 22:24, 21:52) [SRC-RUN-SARUYA, D].

The brief says saruya's 2025 IL was floppy. His dalek.zone IL playlist dates from 2025-06-24 to 07-01 [PT-SARUYA-LIST, D]. `docs/human-route.md` identifies it as VGA floppy from frames, which agrees with his VGA note above. I did not re-check the IL frames for EGA versus VGA.

### 4.2 Measured comparison: CD record (21:52) against saruya's best EGA (22:59)

Both runs are by the same runner, a week apart, on ScummVM 2026.3.0 with Credit Early. Boundaries are the first frame of the next part or the named split. Both "Hell" frames show the giant monkey head, and both "Necklace" frames show the ghost-ship screen (D, frames of [V-WR] and [V-EGA2259]).

| Segment | CD 21:52 | EGA 22:59 | CD minus EGA |
|---|---:|---:|---:|
| Part I trials and fights (to leaving Carla, "T-shirts") | 9:05 | 9:06 | -1 s |
| Part I crew phase | 4:25.5 | 4:03 | +22.5 s |
| Part II | 1:17.5 | 1:17 | +0.5 s |
| Part III, beach to giant monkey head | 2:51 | 4:17 | -86 s |
| Catacombs (monkey head to ghost-ship screen) | 0:50 | 0:50 | 0 |
| Ghost ship to "Last Part" card | 1:16.5 | 1:22 | -5.5 s |
| Part IV | 2:07 | 2:05 | +2 s |
| **Total (LiveSplit)** | **21:52.4** | **23:00.1** | **-67.7 s** |

Notes:

- The EGA crew order differs from the CD one. EGA goes to Carla at 11:59 to 12:14, then Stan at 12:26 to 12:53, then the dock (D, frames).
- Fight RNG and execution differ between the runs. The trials and fights segment is a near tie by chance. saruya calls the EGA run's pirates "Great" and says he lost about 10 s to a missed Carla insult in the CD run [SRC-RUN-SARUYA, D].
- Timer handicap. At 60.00 jiffies/s instead of 59.17, engine-paced EGA time would shrink by up to 1.37%, about 19 s at most here (Local; §4.3).
- Inference: the item-slide gain (about 86 + 5.5 s, less about 22.5 s of setup) is worth roughly 69 s by itself. If the EGA run were rescaled to the 60 Hz timer, it would still be about 49 s behind (23:00 to about 22:41, against 21:52). The CD advantage does not depend on the timer.

A second CD run (22:24) has crew phase "Slide" at 13:28, "Circle" 16:19, "Hell" 17:56 and "Necklace" 18:49 [V-CD2224, D]. These are consistent with the record.

### 4.3 Engine-level differences between versions (ScummVM 2026.3.0)

From the pinned source (Local, `third_party/scummvm`, `RELEASE: This is 2026.3.0`) and the decompiled scripts (Local, `data/scripts-dos-vga`, `data/scripts`):

**Timer rate.** `setTimerAndShakeFrequency()` (`engines/scumm/scumm.cpp`):

- DOS and Windows v2 to v4: 1193182/5041 = 236.70 quarter-frames/s (59.17 jiffies/s).
- DOS v5: 1193182/4096 × 3433/4167 = 239.99 (59.998).
- Every other platform: 240 (60.000). This includes Mac, FM-Towns and Sega CD. It also includes Amiga MI1 (excluded from the Amiga VBLANK branch by `_game.id != GID_MONKEY_VGA`) and Atari ST EGA.

These PIT rates were added in "SCUMM: Implement accurate PIT timings for DOS versions" (95894ae9fe, authored 2022-04-23, first in v2.6.0) [GH-PIT, D]. So the board's 2.8+ rule puts every DOS floppy run at 59.17.

**Snap scroll (Ctrl+R).** `snapScrollKeyEnabled = (_game.version >= 2 && _game.version <= 4)`, with Ctrl+R toggling `VAR_CAMERA_FAST_X` (`engines/scumm/input.cpp`). So EGA and VGA floppy (v4) have instant camera snap and CD (v5) does not. LeoLitz, on CD: no snap scroll "for this version" [YT-LEO-GUIDE, D]. saruya uses it on floppy and notes a possible bug: after loading a savestate, snap stays on until Ctrl+R is pressed twice [V-2445, D].

**Sound-card branches in the v4 scripts.** These are absent from the Mac v5 crate script and, by inference, from CD:

- The crate fanfare wait runs only `if (VAR_SOUNDCARD != 4)` (`room-075-gh-hull/obj-0781-glowing-crate.txt [019E]`). The Mac v5 script always waits on `isSoundRunning(53)` (`data/scripts/room-075-gh-hull/obj-0821-glowing-crate.txt [015C]`).
- The kitchen's local 201 runs `getRandomNr(4)` in a loop only on AdLib (`VAR_SOUNDCARD == 3`) (`room-041-kitchen/local-201.txt`).
- Global script 149 and the bar, ship hull and kitchen scripts branch on AdLib (3) versus Roland (4).
- ScummVM sets `VAR_SOUNDCARD = 4` for DOS EGA and VGA MI1 with any MIDI device, 3 for AdLib, and 0 for PC speaker or No Music (`engines/scumm/vars.cpp` `setSoundCardVarToCurrentConfig`).

**Enhancements.** The default is `kEnhGameBreakingBugFixes | kEnhGrp1` (`engines/scumm/metaengine.cpp`). Group 1 includes `kEnhMinorBugFixes`, which re-adds the CD's "I don't need to look at a map right now" check on the EGA Hook Isle shore (room 48). That blocks the EGA map teleport unless minor bug fixes are turned off (`engines/scumm/script_v5.cpp` `o5_isEqual`). Group 3 (restored content), which is off by default, changes the VGA floppy room 39 (Hellmaze) script 211 (`o5_startScript`). No MI1 timing enhancements exist.

**Interface.** Floppy has 12 verbs, a text inventory with 6 visible items selectable with keys 1 to 6, and Q/A to scroll [F-CD, D] [G-COOK-FL, D]. CD has 9 verbs and an icon inventory. The head skip's floppy option of pressing the item's number implies CD has no number keys for inventory items [G-HEAD, D]. Inference: for an injected-sentence bot, interface speed matters little except where a script's timing depends on the UI.

### 4.4 Other versions

- **EGA versus VGA floppy.** Both are v4 at 59.17 Hz with snap scroll. EGA-only: the Hook Isle map teleport (needs enhancements off) [F-EGA, D]. VGA-only: the crack position [V-2445, D]. Top runners chose EGA in 2025 and 2026. No source quantifies EGA against VGA. Inference: VGA floppy has no known advantage and at least one known disadvantage (no Hook Isle teleport).
- **FM-Towns.** v5 with CD audio, 60.000 Hz, the CD-style interface. Used for Swordmaster% #3 ("Towns for the music") [SRC-RUNS, D]. No item-slide or Any% data.
- **Sega CD.** v5, 60.000 Hz, `MDT_NONE` music in the detection table (CD audio). Listed as a platform since 2024 [F-PORT, D]. No runs.
- **Amiga.** v4 at 60.000 Hz in ScummVM (Local). The original Amiga version has unskippable dialogue (MilkToast, WinUAE) [F-CD, D]. In ScummVM, Amiga MI1 has, by inference, the floppy interface, snap scroll and the 60 Hz timer. Untested; no ScummVM Amiga runs exist.
- **Atari ST.** EGA variant ("No AdLib", PC speaker or PCjr music only), v4 at 60.000 Hz (Local). No MT-32 branch is possible. No runs.
- **Mac (scummrun's current version).** v5, 60.000 Hz, Mac sound (`VAR_SOUNDCARD = 0xFFFF`). Not a listed platform. Whether the CD item slides reproduce on Mac was not checked.
- **Special Edition.** A separate board, and slower even in classic mode [F-SE-PAGE, D].
- **Load times.** MilkToast wrote in 2018, in an Amiga context, that the "CD-Version runs and loads a bit faster" [F-CD, D]. No source documents a load-time difference between versions under ScummVM (not found). Inference: ScummVM loads rooms from disk images without the original's disk access, so load time is unlikely to separate versions there.

### 4.5 Verdict for scummrun

- **Facts.** The record route depends on CD-documented item slides. The CD record is 68 s ahead of the same runner's best EGA run. The DOS VGA floppy we hold is a v4 release at 59.17 Hz, like EGA.
- **Inference.** For a human-comparable route, DOS CD beats both floppy releases. EGA against VGA is unresolved: top runners preferred EGA, but no measurement exists, and the Hook Isle teleport was not seen in the EGA 22:59 (the frame at its Chicken split shows Guybrush on the cable post). The CD gain is engine time (slide states, 1.39% timer), so it applies to a bot too. The floppy-only engine advantages (snap scroll, the MT-32 fanfare branch, EGA's Hook Isle teleport, VGA's crack spot) are small next to the 86 s Part III gain.
- **The one open question that could change this.** Whether the store/compass slide, and so the oars and feather skips, can be reproduced on v4 floppy. If they can, floppy pays 1.4% on the timer and gains snap scroll. Test it in our engine before committing to floppy.
- **Practical point.** DOS CD is SCUMM v5, the same engine class as the Mac pipeline. The DOS VGA floppy is v4, a subclass with different scripts (`docs/related-work.md` §5.3).

## 5. ScummVM settings

### 5.1 Music device ("No Music" is banned)

- **Rule.** "Any Music Device may be used EXCEPT No Music" [SRC-RULES, D]. No source gives the reason (not found).
- **What runners use.** hyperformance: MT-32 with "Mixed AdLib/MIDI mode to enable sound effects", and "EGA patch with MT-32 set up skips the crate fanfare in the ghost ship" [SRC-RUNS, D]. The page hosts "Roland and AdLib Patch for VGA Floppy" (interpreter 5.0.18), "Roland and Adlib for EGA Floppy" and "MT32 required disk09 patch" [RES-PATCHES, D]. mike chose FM-Towns "for the music" [SRC-RUNS, D].
- **Why device choice changes time and RNG.** The v4 scripts branch on `VAR_SOUNDCARD` (§4.3):
  - Roland skips the crate fanfare wait.
  - AdLib runs an extra `getRandomNr(4)` loop in the kitchen, which changes the RNG stream.
  - Several scripts wait in `isSoundRunning` loops: the crate fanfare, the Part I card in `room-096-part1/local-200`, the mansion foyer `local-208` (Local).
- **Inference on the ban.** "No Music" sets `VAR_SOUNDCARD = 0` and takes the PC-speaker branches. It may also end sound waits early depending on what `isSoundRunning` returns without a real music driver. I did not determine which. Either way it would change timings and fairness, which plausibly explains the ban.
- **For scummrun.** The music device is a route parameter: it changes both wait lengths and the RNG.

### 5.2 Text speed, subtitles and fast mode

- **Text speed.** Text speed "doesn't matter. You should be using Period (.) Key" [F-RULES, D].
  - For the CD item-slide chain, LeoLitz recommends slow text, because reading the book through the cannibal dialogues only works with long text. He suggests switching text speed during the run if needed [YT-SLIDE, D].
  - The record's overlay shows 123 `.` press onsets and zero `-` or `+` presses (D, my scan of [V-WR]). Text speed was therefore not changed with the in-game keys during the run.
  - mike's MI2 TAS used `talkspeed=0` and `original_gui_text_speed=0` [RENTRY-MI2, D].
- **Subtitles.** Not relevant (no speech in DOS floppy or CD). The Talkie patch is a separate category [SRC-RULES, D].
- **Ctrl+F or Ctrl+G fast mode.** Not mentioned in any guide, thread or run comment I fetched (D, negative across the fetched corpus). TASVideos rejected a Loom TAS for Ctrl+G (`docs/related-work.md` §2.2). Inference: it is not usable on an RTA board, and the site rules say "Just because something isn't in writing doesn't mean it is allowed" [SITE-RULES, D].

### 5.3 Enhancements

No MI1 run comment states its enhancement setting (not found). mike's MI2 TAS used `enable_enhancements=false`; his DOTT TAS used `enhancements=1` [RENTRY-MI2, D] [RENTRY-DOTT, D]. Effects in MI1 are in §4.3. The EGA Hook Isle teleport needs `kEnhMinorBugFixes` off.

### 5.4 Modified builds ("almost 2 minutes faster")

- **2019.** aWay0fLife recommended "UrQuans version of ScummVM 2.1.0 ... optimized to run as frame perfect as possible and is almost 2 minutes faster over the course of the run" [F-GUIDE-ERR, D].
- **2020.** A runner on stock 2.2.0 lost 19 frames on the walk to the bar door. LeoLitz's fix was "a modified scummVM made specifically to fix some inaccuracies"; "we only have the executable and the guy in question is nowhere to be found" [F-WALKSPEED, D].
- **What they changed: not found.** Neither build was obtained.
- **Inference.** ScummVM 2.1.0's `waitForTimer` sleeps in 10 ms polls and does not compensate for overshoot or screen-update time [GH-210, D]. That loses a few milliseconds per 83 ms frame, which fits about 2 minutes over a 28-minute run. Upstream fixed it in 2022: compensated waits (`_lastWaitTime`), quarter-frame precision (9fc868c7ed) and PIT rates (95894ae9fe), all in 2.6.0 [GH-PIT, D]. The 2.8+ requirement postdates these.
- **Update in 2024.** LeoLitz: with "a different version of ScummVM" the quicksave prompt no longer needs clearing [YT-LEO-SAVES, D].
- **Version note.** A v2026.4.0 tag exists [GH-TAGS, D]. The record and scummrun use 2026.3.0.

### 5.5 Keybinds and ScummVM features in use

saruya's record keybinds, from the video description [V-WR, D]:

- Single keys mapped to verbs (Push, Pull, Give, Use, Pick Up, Look At, Open, Talk To).
- Answers 1 to 6, Q/A scroll.
- "f: Same as left click (Enter)", "4: Same as right click (Tab)".
- "Space: Skip dialog (.)", "d: Skip cutscene (Esc)".
- Caps Lock and Alt as the modifiers for quicksave (Alt+n) and quickload (Ctrl+n).
- "left arrow key: Combined with right Ctrl, enable/disable screen snap (R)".
- "n: Toggle mouse capture", "5: Restart the game (F8)".

Other ScummVM features runners use:

- Ctrl+M to trap or untrap the mouse, to reposition the cursor outside the window during cutscenes [G-CONV, D].
- "Return to launcher when leaving a game" with a quit hotkey such as Alt+Q, to reseed the RNG quickly [F-SAFE, D].
- In 2025 saruya argued that two-key ScummVM shortcuts such as quicksave are technically against "one key press = one input" and asked for an exception. No reply [F-KEYS, D].

## 6. Tool-assisted runs and what they reveal

- **MI1 TAS: not found.** mike wrote "when I was working on the TAS" in the July 2025 safe thread, and saruya replied "after ... watching your TAS" [F-SAFE, D]. mike's Twitch highlights list (to 2026-10-12) has no MI1 TAS. It has "Highlight: testing monkey1 shuffler" (2025-12-07), apparently a randomizer test [TW-MIKE, D]. YouTube searches found none [YT-SEARCH, D].
- **MI2 TAS (mike).** "Monkey Island 2: LeChuck's Revenge (DOS/German) in 29:47", libTAS 1.4.4, ScummVM 2.8.0git, `--random-seed=48 monkey2-de` [RENTRY-MI2, D] [TW-MIKE, D]. Its `scummvm.ini` shows:
  - `music_driver=pcspk` (game section);
  - `enable_enhancements=false`;
  - `talkspeed=0`;
  - `original_gui=true`.
- **DOTT TAS (mike).** "14:04.91 via RTA-timing", libTAS 1.4.5, ScummVM 2.9.0git, `--random-seed=1 tentacle`, `talkspeed=170`, `enhancements=1` [RENTRY-DOTT, D] [TW-MIKE, D]. saruya's 2026 RTA runs of DOTT (9:21) use newer skips [PT-SARUYA-LIST, D].
- **Full Throttle TAS (mike).** "Full Throttle TAS in 8:14 with broken audio" (2023-04-07), no notes [TW-MIKE, D].
- **LeoLitz, "Monkey Island 2 Lite Tas Explained" (2020).** D, auto-captions, paraphrased:
  - He worked frame by frame on an emulator other than the one used in runs. It lost an 18 s "music glitch", a 2 s scroll skip, and some general speed.
  - The main technique: when several cutscenes overlap and one is skipped, the cursor comes back while a cutscene still runs in the background. Reading the book's text stalls that cutscene long enough to walk to the map piece. This is the same family as MI1's item slide.
  - Buffering: select the verb before an item appears, or prepare a second action. Selecting a verb, clicking the first object and clicking the second object are each processed in a different six-frame window, so buffering saved 12 frames in one place.
  - LeChuck's spawns were manipulated by standing in doorways, where he cannot spawn.

  [YT-MI2LITE, D]
- **"Weird dances".** LeoLitz in 2025: "maybe TAS could find some weird dances that are faster for certain walks" [F-CONV, D]. Nobody has followed this up publicly.
- **saruya's MI2 "jiffyspeed".** Videos titled "Getting jiffyspeed in Monkey Island 2" and "Keeping jiffyspeed in Monkey Island 2: the invitation skip" (2026-07-25) [PT-SARUYA-LIST, D, titles only; not watched]. Inference: probably an MI2 analogue of the logo glitch (a changed `VAR_TIMER_NEXT`).

Inference for "at least as fast as any possible TAS": what a TAS can do and a human cannot is mostly three things. Frame-exact buffering in the six-frame input windows. Walk-step optimisation (shmoovement and the "dances"). RNG chosen by timing instead of by reloads. All three need the bot to act at engine-frame granularity, which scummrun's bridge already does.

## 7. RNG and determinism notes for a bot

- **Seeding.** `Common::RandomSource` takes its seed from the `random_seed` config key when set (`common/random.cpp`). Returning to the launcher re-creates the engine and reseeds. That is why runners quit to the launcher before loading [G-CATA, D] (Local).
- **No per-frame RNG call.** The 2022 main loop had one (`_rnd.getRandomNumber(2)` in `ScummEngine::go()`, visible in the 9fc868c7ed diff) [GH-PIT, D]. It is gone in 2026.3.0, which has no `_rnd` call in `scumm.cpp` (Local). The RNG advances only through script `getRandomNr` calls and engine calls such as actor talk animation (`actor.cpp:2631`), object state and some graphics effects (Local). The frame of a save matters only because background scripts (pirates, Fester, sounds) draw numbers while time passes.
- **Device and version dependence.** The kitchen's AdLib-only RNG loop is one example (Local). Seeds are version-specific [SRC-RUN-SARUYA, D].
- **CD nondeterminism.** saruya reports two RNG calls that "aren't always consistent" on CD, and a variable 0 to 2 Fester RNG calls [G-CATA, D]. He also wonders whether the item slide makes the catacombs RNG inconsistent [SRC-RUN-SARUYA, D]. Inference: CD audio timing (`VAR_MUSIC_TIMER` comes from CD playback in `sound.cpp` `updateMusicTimer`) is a plausible source. scummrun would need to control or stub the audio timing to make CD runs repeatable.
- **Original record/playback.** ScummVM 2026.3.0 has a Shift+F10 "Enter playback file name" prompt for DOS MI1, MI2 and Indy4 (`engines/scumm/input.cpp`, `canOpenPlaybackPrompt`) (Local). Not investigated. Inference: it may support the original interpreter's demo-recording format, which the MI1 demo's F9/F10 record/play feature hints at [F-DEMO, D].

## 8. Corrections to earlier project documents

- **Item slides are not Part III only.** `docs/human-route.md` §1.1 (the G-SLIDE row) and §2.3 say the item-slide family "is Part 3 only". On CD the slide is obtained in the Part I crew phase, in the store with the compass, and carried through Parts II and III (§2.3).
- **The forest is probably not nine walks in the top runs.** The record reads the map in the forest at timer 6:44, and the next frame is a different screen (§2.2). That matches LeoLitz's forest map teleport (§3.11).
  - I checked the 2025 IL video (dalek `7V26t1riuonhASTgSji2av`) at 6:08 to 6:22. Guybrush is off screen from about 6:14.0, the line "I think I've been had!" appears at 6:15.5, and he reappears at the left edge at 6:17.0 (D, frames).
  - `docs/human-route.md` §3.2 step 73 records a black frame at 6:16.3 with the same background before and after, and treats it as not a room change.
  - Inference: the IL probably uses the map teleport too. If so, it works on v4 floppy. It would also mean the forest is not the "cleanest test" that `docs/comparison.md` §1.4 takes it to be: part of the forest gap there would be the teleport, not the logo glitch. This needs a frame-by-frame check before those documents are changed.

## 9. Not found

- mike's MI1 TAS, or any MI1 TAS.
- Why the CD store/compass item slide is documented only for CD, and whether it works on v4 floppy or Mac.
- The reason "No Music" is banned.
- What the 2019 and 2020 modified ScummVM builds changed.
- mike's Discord shmoovement write-up, and the Speedy Adventures Discord in general.
- The seed finder's source code (Cloudflare 403 on the zip).
- The written rules change that frozenspade says allowed set seeds.
- Enhancement settings used by any MI1 runner.
- ScummVM version and seed for places 4 to 9 of the top 10.
- EGA versus VGA for frozenspade, PeteSwanson, Wildzman and Ribrack beyond a single-frame palette judgement (Ribrack undetermined).
- A quantified EGA versus VGA floppy comparison.

## References

All entries were accessed on 2026-10-19. "Local" entries are files in this repository read on that date.

| ID | Source(s) | Accessed |
|---|---|---|
| SRC-RULES | https://www.speedrun.com/api/v1/games/w6j4m46j?embed=categories,variables,moderators; https://www.speedrun.com/api/v1/games/w6j4m46j/variables; https://www.speedrun.com/api/v2/GetGameData?gameId=w6j4m46j (page https://www.speedrun.com/tsomi) | 2026-10-19 |
| SRC-LB | https://www.speedrun.com/api/v1/leaderboards/w6j4m46j/category/jdrqwqlk?var-9l7yd9ql=q8kd6p6q&embed=players; .../category/jdrqwqlk?var-9l7yd9ql=qyzjp9d1&embed=players; .../category/wkpmnlwk?embed=players | 2026-10-19 |
| SRC-RUNS | https://www.speedrun.com/api/v1/runs/yjvd1ndy, .../y8g6x1wz, .../yoer2w5z, .../y9k1772z, .../yd4482qy, .../zxpj6k8y, .../y2g4le6z, .../yddojnxy, .../y62kg61z, .../z020n49m, .../z1nk53ry, .../y4kw2xkm, .../zpxgxprm, .../yvepjpom; https://www.speedrun.com/api/v1/runs?category=jdrqwqlk&status=verified&max=200&embed=players (also status=rejected and status=new, and categories wkpmnlwk and w207w65k) | 2026-10-19 |
| SRC-RUN-SARUYA | https://www.speedrun.com/api/v1/runs?user=xz57l94x&max=200&embed=game,category | 2026-10-19 |
| SRC-PLAT | https://www.speedrun.com/api/v1/platforms/jm95k79o; .../8gej2n93; .../gde32g6k | 2026-10-19 |
| SITE-RULES | https://www.speedrun.com/api/v2/GetArticle?slug=site-rules (page https://www.speedrun.com/support/learn/site-rules, updatedAt 2026-02-05) | 2026-10-19 |
| G-LIST | https://www.speedrun.com/api/v2/GetGuideList?gameId=w6j4m46j (full text of all 13 guides) | 2026-10-19 |
| G-ANY | guide pnydx, "Any% Speedrun Guide (Saveless and Saves)", aWay0fLife and LeoLitz, updated 2019-11-14, via G-LIST (page https://www.speedrun.com/tsomi/guides/pnydx) | 2026-10-19 |
| G-CATA | guide hw8us, "Catacombs RNG manipulation", saruya, 2026-07-22 (https://www.speedrun.com/tsomi/guides/hw8us) | 2026-10-19 |
| G-CONV | guide verpr, "Advanced tips to go fast during conversations", saruya, 2025-05-25 (https://www.speedrun.com/tsomi/guides/verpr) | 2026-10-19 |
| G-COOK-CD | guide fi9t9, "Fast cooking in Part Two (CD)", saruya, 2026-09-06 (https://www.speedrun.com/tsomi/guides/fi9t9) | 2026-10-19 |
| G-COOK-FL | guide 75snn, "Fast cooking in Part Two (floppy)", saruya, 2025-06-08 (https://www.speedrun.com/tsomi/guides/75snn) | 2026-10-19 |
| G-SAVELESS | guide jdq50, "Getting the most of saveless insult swordfights", saruya, 2025-06-22 (https://www.speedrun.com/tsomi/guides/jdq50) | 2026-10-19 |
| G-HEAD | guide 83ev1, "Head skip revisited", saruya, 2025-08-05 (https://www.speedrun.com/tsomi/guides/83ev1) | 2026-10-19 |
| G-MONKEY-FL | guide p8zx7, "The monkey skip (floppy disk versions)", saruya, 2026-08-09 (https://www.speedrun.com/tsomi/guides/p8zx7) | 2026-10-19 |
| G-SLIDE | guide 693q7, "The item slide trylogy: monkey skip, oars skip and feather skip", LeoLitz, 2026-08-07 (links YT-SLIDE) | 2026-10-19 |
| G-OTHER | guides xtrn5 (frozenspade, links YT-FROZEN), pybi6 (thewoofs, links YT-WOOFS), cnst3 (LeoLitz, links YT-CATA101), 9bsh7 (demo), via G-LIST | 2026-10-19 |
| RES-SEED | https://www.speedrun.com/api/v2/GetResourceList?gameId=w6j4m46j (resource 2z7qm description); zip https://www.speedrun.com/static/resource/2z7qm.zip returned HTTP 403 | 2026-10-19 |
| RES-PATCHES | same resource list: 1yttk "MT32 required disk09 patch", khoyg "Roland and Adlib for EGA Floppy", 9lyqn "Roland and AdLib Patch for VGA Floppy" | 2026-10-19 |
| F-LIST | https://www.speedrun.com/api/v2/GetThreadList?forumId=4zl1p12d&page=1 (25 threads) | 2026-10-19 |
| F-SAFE | https://www.speedrun.com/api/v2/GetThread?id=6etn4 (https://www.speedrun.com/tsomi/forums/6etn4), 2025-06-27 to 07-04 | 2026-10-19 |
| F-CONV | https://www.speedrun.com/api/v2/GetThread?id=m4npa (https://www.speedrun.com/tsomi/forums/m4npa), 2025-05-17 to 06-22 | 2026-10-19 |
| F-CD | https://www.speedrun.com/api/v2/GetThread?id=ew0ht (https://www.speedrun.com/tsomi/forums/ew0ht), 2018-12 to 2019-01 | 2026-10-19 |
| F-SPEED | https://www.speedrun.com/api/v2/GetThread?id=2bi5y (https://www.speedrun.com/tsomi/forums/2bi5y), 2017-08-18 | 2026-10-19 |
| F-CREDIT | https://www.speedrun.com/api/v2/GetThread?id=lvlhg (https://www.speedrun.com/tsomi/forums/lvlhg), 2020-10-03 | 2026-10-19 |
| F-GUIDE-ERR | https://www.speedrun.com/api/v2/GetThread?id=2s9x6 (https://www.speedrun.com/tsomi/forums/2s9x6), 2019-01 | 2026-10-19 |
| F-WALKSPEED | https://www.speedrun.com/api/v2/GetThread?id=oa64e (https://www.speedrun.com/tsomi/forums/oa64e), 2020-12 to 2021-07 | 2026-10-19 |
| F-HEAD | https://www.speedrun.com/api/v2/GetThread?id=vqinl (https://www.speedrun.com/tsomi/forums/vqinl), 2025-06 to 08 | 2026-10-19 |
| F-DOORS | https://www.speedrun.com/api/v2/GetThread?id=2gya8 (https://www.speedrun.com/tsomi/forums/2gya8), 2025-06-25 | 2026-10-19 |
| F-CATA | https://www.speedrun.com/api/v2/GetThread?id=nm6nr (https://www.speedrun.com/tsomi/forums/nm6nr), 2025-06-05 | 2026-10-19 |
| F-KEYS | https://www.speedrun.com/api/v2/GetThread?id=5mrk1 (https://www.speedrun.com/tsomi/forums/5mrk1), 2025-05-25 | 2026-10-19 |
| F-EGA | https://www.speedrun.com/api/v2/GetThread?id=kughh (https://www.speedrun.com/tsomi/forums/kughh), 2023-11 | 2026-10-19 |
| F-GHOSTKEY | https://www.speedrun.com/api/v2/GetThread?id=7nhdb (https://www.speedrun.com/tsomi/forums/7nhdb), 2023-09 | 2026-10-19 |
| F-PORT | https://www.speedrun.com/api/v2/GetThread?id=gfxby (https://www.speedrun.com/tsomi/forums/gfxby), 2024-04 | 2026-10-19 |
| F-SOLUTIONS | https://www.speedrun.com/api/v2/GetThread?id=d2exu (https://www.speedrun.com/tsomi/forums/d2exu), 2019 | 2026-10-19 |
| F-RULES | https://www.speedrun.com/api/v2/GetThread?id=4vc4y (https://www.speedrun.com/tsomi/forums/4vc4y), 2017-2018 | 2026-10-19 |
| F-DEMO | https://www.speedrun.com/api/v2/GetThread?id=uojq6 (https://www.speedrun.com/tsomi/forums/uojq6), 2021-2023 | 2026-10-19 |
| F-SE-PAGE | https://www.speedrun.com/api/v2/GetThread?id=0ptkg (https://www.speedrun.com/tsomi/forums/0ptkg); see also `docs/human-route.md` SE row | 2026-10-19 |
| F-OTHER | threads aq9ji, 1esx1, i8uk1, 02zwk, t5uy4, 8sn9b, by7vn via https://www.speedrun.com/api/v2/GetThread?id=... | 2026-10-19 |
| V-WR | https://dalek.zone/w/grcsxWpv5XeLGarGLS2y81; https://dalek.zone/api/v1/videos/grcsxWpv5XeLGarGLS2y81 (description, keybinds); 480p file https://dalek.zone/static/streaming-playlists/hls/7cfdbc39-7020-409c-90b4-f2cd9d53e65e/2e692bf3-b38a-4bb7-aa74-ffa1f9452a82-480-fragmented.mp4 (frames at split times, part boundaries, forest 6:25-7:05, crew 9:05-13:30, Part III, overlay key scan at 10 fps) | 2026-10-19 |
| V-EGA2259 | https://dalek.zone/w/rRm2wpiuMobXNYoUufcmVf; https://dalek.zone/api/v1/videos/rRm2wpiuMobXNYoUufcmVf; 480p file .../hls/d15840bb-9905-4512-b296-f81c7be182f8/cf5c8856-6b7d-4a90-8ef7-038816882362-480-fragmented.mp4 (split panel; frames 11:59-14:30, 19:36-21:34, split times) | 2026-10-19 |
| V-EGA2417 | https://dalek.zone/w/cR1wTdYQzszbtttikuaPez (+ API; end-of-run split panel) | 2026-10-19 |
| V-CD2224 | https://dalek.zone/w/pFpGiTcCYAM3qb6JeUMMyh (+ API; end-of-run split panel) | 2026-10-19 |
| V-SARUYA-RUNS | https://dalek.zone/api/v1/videos/ for pma4bi88Ti2gpPBjdrS3j4 (23:35), ivvX5dpLNxE8z5qc1Vd7LE (23:15), 2i5PamYDoPdcfZX7Li5Qod (22:43), xhCTuC2x5i2ptxBdbp1uAk (24:55), with end-of-run split panels | 2026-10-19 |
| V-IL | https://dalek.zone/videos/watch/37fded15-f69d-44ca-9532-d7628b195393 (short id 7V26t1riuonhASTgSji2av); https://dalek.zone/api/v1/videos/7V26t1riuonhASTgSji2av (480p file; frames 6:08-6:22) | 2026-10-19 |
| V-2445 | https://dalek.zone/w/xbPfJqKjwPLKiPiR87dfPr; https://dalek.zone/api/v1/videos/xbPfJqKjwPLKiPiR87dfPr (32 timestamped notes) | 2026-10-19 |
| PT-SARUYA-LIST | https://dalek.zone/api/v1/accounts/saruya/videos?start=0&count=100&sort=-publishedAt (and start=100, 200, 300; 349 videos) | 2026-10-19 |
| PT-CLIPS | https://dalek.zone/api/v1/videos/ for 9VDsm3ruEQBBLRFsFT6kNS, dNZqXytV32mhSTvfxaJNNS, 7eWvk4X42NahyerzCXjNTR, 8BPF5vG7o6MqYrv8mXLU55, 2DTvUs9oHgv1ro6dWyuP4S, k9BxLbqxvWUxbTq1pa87b3, ngEqV26UCMwMVopvG7B1cN, adhuRRaPCFqhonXWFJz7bP, 54rgi1NkKAh72PrMr5h1pz, 5yDxjhtRpjtHASyH7JWFzS, v4eMC8cxVumPwSgXxaFk7c | 2026-10-19 |
| YT-SLIDE | https://www.youtube.com/watch?v=uQKAuh7rud0 "we broke mi1 a little bit", LeoLitz, 2026-08-07 (English auto-captions via yt-dlp) | 2026-10-19 |
| YT-LEO-GUIDE | playlist https://www.youtube.com/playlist?list=PLVmJ-fxqrJQ67P8L1EF8cOgUh_qXwJXzL: UWvgmm70jHo (2024-01-24), a0Kfuj5fo34, 08vSdJcXsKw, iyeve-HL6OI, m5PwPwh3R4E, ASBnUso1Yvw, 9CjSlCCZP1E, BpkRmfA2Ja0 (2022-07-05) (auto-captions and descriptions) | 2026-10-19 |
| YT-LEO-CE | https://www.youtube.com/watch?v=GR4J30NwJ7Y "Chapter 1 part 2a (shop with credit early)" (auto-captions) | 2026-10-19 |
| YT-LEO-3A | https://www.youtube.com/watch?v=1BTk3KzdW08 "Chapter 1 part 3a" (2022-07-05; auto-captions) | 2026-10-19 |
| YT-LEO-SAVES | https://www.youtube.com/watch?v=cMlSGB_buf8 "Chapter 1 part 3b (... using saves)" (2024-01-25; auto-captions) | 2026-10-19 |
| YT-CATA101 | https://www.youtube.com/watch?v=WYpD3FxTa1I "Catacombs 101", LeoLitz, 2021-05-21 (auto-captions) | 2026-10-19 |
| YT-FROZEN | https://www.youtube.com/watch?v=EJSPOAiUYYA "20 Movement Optimizations on Monkey Island", frozenspade, 2024-02-26 (description and auto-captions) | 2026-10-19 |
| YT-WOOFS | https://www.youtube.com/watch?v=8UxmpYtP0YQ "Overview of Speedrunning Swordfighting in MI1", thewoofs, 2023-11-25 (auto-captions) | 2026-10-19 |
| YT-MI2LITE | https://www.youtube.com/watch?v=MTI7h3LGK5k "Monkey Island 2 Lite Tas Explained", LeoLitz (auto-captions); https://www.youtube.com/watch?v=RYjcp5-apUM | 2026-10-19 |
| YT-RUNDESC | YouTube descriptions of aVzYsZ5_-Ms, V7LMIdslVYY, gVpk6XoL0fQ, 8tt3d76Z550, 0wWWhBv508c, h0jVeLQY-bM, QUb7vwIfChc, SU7Fyl5AEGo, GE6QLGgdOqg, n9DTESBHLrQ, JB4AExxrAOU, HOQrl5Q_7Fg | 2026-10-19 |
| YT-RUNFRAMES | one frame each from https://www.youtube.com/watch?v=aVzYsZ5_-Ms, V7LMIdslVYY, gVpk6XoL0fQ, 8tt3d76Z550, 0wWWhBv508c, h0jVeLQY-bM, QUb7vwIfChc, SU7Fyl5AEGo (two frames), GE6QLGgdOqg, n9DTESBHLrQ (stream URLs via yt-dlp) | 2026-10-19 |
| TW-ZERGU | https://www.twitch.tv/videos/1945256872 (one frame) | 2026-10-19 |
| TW-MIKE | https://gql.twitch.tv/gql (user mikespeedyadventures, highlights list); https://www.twitch.tv/videos/2249139810 (one frame); https://www.twitch.tv/videos/2084827018; https://www.twitch.tv/videos/2084827017; https://www.twitch.tv/videos/1787419604 | 2026-10-19 |
| RENTRY-MI2 | https://rentry.co/fp3eb | 2026-10-19 |
| RENTRY-DOTT | https://rentry.co/dott-tas | 2026-10-19 |
| YT-SEARCH | yt-dlp `ytsearch12:` for "secret of monkey island TAS", "monkey island 1 TAS libtas", "monkey island tool assisted speedrun scummvm", "mikeSpeedyAdventures monkey island" | 2026-10-19 |
| GH-PIT | https://api.github.com/repos/scummvm/scummvm/commits/95894ae9fe06bb073d1f9f4c28c2a7f887072da8; https://api.github.com/repos/scummvm/scummvm/commits/9fc868c7ed15cefd2a241d7a9a99fcd74b5c377a; https://api.github.com/repos/scummvm/scummvm/compare/v2.6.0...95894ae9fe06bb073d1f9f4c28c2a7f887072da8 | 2026-10-19 |
| GH-210 | https://raw.githubusercontent.com/scummvm/scummvm/v2.1.0/engines/scumm/scumm.cpp (`waitForTimer`, main loop); same file at v2.2.0 and v2.5.0 | 2026-10-19 |
| GH-TAGS | https://api.github.com/repos/scummvm/scummvm/tags?per_page=100 | 2026-10-19 |
| L-SVM | Local: third_party/scummvm (v2026.3.0): engines/scumm/scumm.cpp (`setTimerAndShakeFrequency`, `waitForTimer`), engines/scumm/scumm.h (PIT constants), engines/scumm/input.cpp (snap scroll, playback prompt), engines/scumm/vars.cpp (`setSoundCardVarToCurrentConfig`), engines/scumm/sound.cpp (`isSoundRunning`, `updateMusicTimer`), engines/scumm/script_v5.cpp (`o5_isEqual`, `o5_startScript`), engines/scumm/metaengine.cpp, engines/enhancements.h, engines/scumm/detection_tables.h, common/random.cpp | 2026-10-19 |
| L-DOS | Local: data/scripts-dos-vga/room-010-logo/local-204.txt; room-030-store/local-205.txt and entry.txt; room-075-gh-hull/obj-0781-glowing-crate.txt; room-041-kitchen/local-201.txt, local-212.txt; room-096-part1/local-200.txt; room-053-foyer/local-208.txt; room-078-church/entry.txt; global/script-149.txt | 2026-10-19 |
| L-MAC | Local: data/scripts/room-075-gh-hull/obj-0821-glowing-crate.txt | 2026-10-19 |
| L-DOCS | Local: docs/related-work.md, docs/human-route.md, docs/comparison.md, docs/research/skips-engine.md | 2026-10-19 |
