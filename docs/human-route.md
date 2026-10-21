# Human speedrun route: Part I, treasure + idol

*The Secret of Monkey Island* (1990, classic SCUMM), Part I "The Three Trials".
The segment runs from gaining control at the start of Part I until both the
treasure trial (dig up the T-shirt) and the idol trial (pick up the idol
underwater) are complete. Swordfighting/Carla is out of scope, but this document
records where the human route interleaves it.

All sources were accessed on 2026-10-04.

## Summary

There is no glitchless category. speedrun.com has Any%, Any% (Talkie),
Swordmaster% and Demo for the classic game, and Any% and Special% for the
Special Edition. Any% allows savestates, ScummVM-only speed benefits and
"Credit Early". None of those tricks changes the action list for the treasure
and idol trials, so the human trial route is glitch-free at the action level.
The speed differences come from timing tricks (see §2.3).

The current route order, from saruya's 2025 IL and 2026 WR, is:

1. kitchen (pot, meat, fish)
2. circus (money)
3. petal
4. map
5. chicken
6. Otis
7. store (shovel, sword, mints)
8. mansion #1 (gopher repellent)
9. Otis (cake, file)
10. mansion #2 (idol)
11. underwater
12. troll, then Smirk (swordfight only)
13. forest dance, then dig
14. insult fights, then Carla

Both runs finish the idol trial before the treasure trial. The 2019 text guide
dug the treasure after Carla instead, while newer runs dig it before the insult
fights.

Counting treasure + idol only (§4), the route has 22 player actions and 48
player-initiated room transitions from the Lookout (47 if the opening Esc skip
is used), plus 3 scripted transitions.

## 1. Sources

### 1.1 Primary speedrun sources

speedrun.com returns HTTP 403 to page fetches. Its content was read through
speedrun.com's own public JSON API (v1/v2), which serves the same rules, guides,
forum threads and run comments as the HTML pages.

| ID | URL | What was taken |
|---|---|---|
| SRC-GAME | https://www.speedrun.com/tsomi (via https://www.speedrun.com/api/v1/games/w6j4m46j?embed=categories,variables and https://www.speedrun.com/api/v2/GetGameSummary?gameId=w6j4m46j) | Category list, rules text, subcategory rules, game-level rules (§2). |
| SRC-LB | https://www.speedrun.com/tsomi?h=Any-scummvm&x=jdrqwqlk-9l7yd9ql.q8kd6p6q (via `api/v1/leaderboards/w6j4m46j/category/jdrqwqlk`) | Leaderboard: WR saruya 21:52 (2026-08-23, Saves, Credit Early, ScummVM). #2 hyperformance 24:23 (2025-07-14). Run comments: versions, seeds, ScummVM versions. |
| WR | https://www.speedrun.com/tsomi/runs/yjvd1ndy, video https://dalek.zone/w/grcsxWpv5XeLGarGLS2y81 | Description (ScummVM 2026.3.0, seed 2756848129, key bindings). I inspected video frames for 0:00-8:00 at 1 frame per 5 s, and the forest at 2 fps. The frames confirm the route order, that the WR uses the **DOS CD** interface, the Credit Early safe pull at the first store visit, and that the treasure is dug after Smirk and the first pirate fight. |
| HYP | https://www.speedrun.com/tsomi/runs/y8g6x1wz | Comment only: "EGA is still king", ScummVM 2.9.1, seed 102545962. Video not inspected. |
| IL | saruya, "Part 1 - The three trials (until the start of the insult fights)", https://dalek.zone/videos/watch/37fded15-f69d-44ca-9532-d7628b195393 (published 2025-06-24), in playlist https://dalek.zone/video-playlists/1c9b8b60-bbdc-4fbc-b4b5-9bcdc9b656b1 | **Main step-order source.** Inspected frame by frame: every 3 s for the whole 6:47 video, and 1-4 fps around the kitchen, Citizen, Voodoo shop, jail, store, mansion, Otis, troll, Smirk and forest. The verb line and text inventory confirm each verb/object and each item acquisition. Version: **DOS VGA floppy** (12 verbs including Walk to / Turn on / Turn off, text inventory). Timestamps below ("IL m:ss") are video times. The on-screen run timer reads about 4 s less. |
| IL-CE | saruya, "...with credit early", https://dalek.zone/w/1KCZMRmWHzrvqdyqn4XC1c | Inspected every 2 s around the store. A savestate dialog appears before entering the store, then the safe is opened and the note taken during the first store visit. |
| G19 | https://www.speedrun.com/tsomi/guides/pnydx "Any% Speedrun Guide (Saveless and Saves)", aWay0fLife and LeoLitz, updated 2019-11-14 | Full text route with dialogue numbers ("Dia:"), Esc/`.` usage, the logo speed glitch, the cook timing, and the Sword Master forest path. Written for the **DOS CD** version. |
| G-CONV | https://www.speedrun.com/tsomi/guides/verpr "Advanced tips to go fast during conversations", saruya, 2025-05-25 | Confirms that `.` skips a line, that Esc or both mouse buttons skip a cutscene, that the map-purchase conversation is answer 4, Esc, answer 2, and that clicking the game area does "leave conversation" (Otis, storekeeper). |
| G-SW | https://www.speedrun.com/tsomi/guides/jdq50 "Getting the most of saveless insult swordfights", 2025-06-22 | Get the troll insult before fights (swordfight interleave). |
| G-SLIDE | https://www.speedrun.com/tsomi/guides/693q7 and https://www.speedrun.com/tsomi/guides/p8zx7 | The "item slide" glitch family (monkey/oars/feather skips) is Part 3 only, so it does not touch this segment. |
| F-SPEED | https://www.speedrun.com/tsomi/forums/2bi5y (via `api/v2/GetThread?id=2bi5y`), aWay0fLife 2017-08-18 | Speed glitch description. Explicit statement against a glitchless category. |
| F-CREDIT | https://www.speedrun.com/tsomi/forums/lvlhg, 2020-10-03 | Definition of Credit Early. Saves/Saveless merged into one board. |
| F-RULES | https://www.speedrun.com/tsomi/forums/4vc4y, 2017-2018 | Text skipping with `.`. Ctrl+W instant win and the request to ban it. |
| F-STORE | https://www.speedrun.com/tsomi/forums/2s9x6, 2019-01-18 | LeoLitz's correction of the first storekeeper dialogue to `3,1,1,1,1,2`. |
| F-CD | https://www.speedrun.com/tsomi/forums/ew0ht, 2018-2019 | "We all just use the cd-rom version by default" (2019). Floppy versus CD notes. |
| F-CONV | https://www.speedrun.com/tsomi/forums/m4npa, 2025 | Describes LeoLitz's YouTube video guide as the most up-to-date guide. "Shmoovement" (repeated clicks for longer walk steps). |
| F-SAFE | https://www.speedrun.com/tsomi/forums/6etn4, 2025 | Seeded safe combination, savestate before entering the store (Credit Early mechanics). |
| F-DOORS | https://www.speedrun.com/tsomi/forums/2gya8, 2025 | Kitchen door state depends on the cook closing it in Part I. |
| F-EGA | https://www.speedrun.com/tsomi/forums/kughh, 2023 | EGA-only map teleport on Hook Isle (crew phase only). |
| SE | https://www.speedrun.com/tsomise (via `api/v1/games/tsomise?embed=categories,variables`, `api/v2/GetGameSummary?gameUrl=tsomise`) | Special Edition categories (Any%, Special%) and rules. SE WR comment (saruya 2026-08-31) on classic-mode SE being slower than the original. |

Not consulted, or not usable: the Speedy Adventures Discord (not publicly
indexed), LeoLitz's YouTube video-guide playlist
(https://www.youtube.com/playlist?list=PLVmJ-fxqrJQ67P8L1EF8cOgUh_qXwJXzL,
named in F-CONV but not watched), frozenspade's "20 Movement Optimizations"
video (https://www.youtube.com/watch?v=EJSPOAiUYYA, not watched), the Universal
Hint System (not fetched), and the TAS mentioned in F-SAFE (no public page
found).

### 1.2 Generic walkthroughs and reference (fallback only)

| ID | URL | What was taken |
|---|---|---|
| AG | https://adventuregamers.com/walkthrough/full/the-secret-of-monkey-island (Tom Hayes, v1.1 2008, PC) | Room-by-room geography (cliff path, archways, trail to the mansion). Dialogue wording. Item sources (gopher repellent from the mansion back room, file in the cake). Mansion and Fester sequence. Forest directions (these disagree; see §5). |
| JA | https://www.justadventure.com/walkthrough/the-secret-of-monkey-island-cheats-2/ (redirect target of the-spoiler.com link) | Same Tom Hayes text as AG. Used only to cross-check the forest directions. |
| WIKI | https://monkeyisland.fandom.com/wiki/Pieces_o%27_Eight, `.../wiki/Shovel`, `.../wiki/Sword`, `.../wiki/Breathmaster_Mints`, `.../wiki/M%C3%AAl%C3%A9e_Island_Treasure_Map`, `.../wiki/Captain_Smirk%27s_Big_Body_Pirate_Gym`, `.../wiki/Gopher_Repellent`, `.../wiki/The_Three_Trials` (read through the MediaWiki API) | Prices: map 100, sword 100, shovel 75, mints 1, Smirk sword training 30. Trials can be done in any order, and reporting to the pirate leaders is optional. Gopher repellent source. |
| MAP | https://static.wikia.nocookie.net/monkeyisland/images/1/1d/Map_to_treasure_close.gif (image on the WIKI treasure-map page) | The in-game map text, read from the screenshot: three stanzas whose first words are **Back, Left, Right / Left, Right, Back / Right, Left, Back**. |
| BMUG | https://discmaster.textfiles.com/file/6285/BMUG%20Revelations.toast/Entertainment/Game%20Files%E2%81%84%20Help/Monkey%20Island%20Walkthu/Monkey%20Island%20Walkthu.pdf (period walkthrough) | Forest rule: use the first word of each map line as the exit to take. The X is "off to your right in the last scene". |
| GW | https://gamerwalkthroughs.com/?p=14683 (Special Edition walkthrough) | Gives the forest directions in compass form. These disagree with the others; see §5. |
| SEARCH | Web search results (no fetch) for the circus payout and map price | 478 pieces of eight from the circus and 100 for the map. Both are also visible in the IL inventory. |

## 2. Category rules relevant to "glitchless"

### 2.1 What the rules say

- No glitchless category exists. The classic page has Any%, Any% (Talkie),
  Swordmaster%, Demo, and the IL "Beat the Sword Master" (SRC-GAME). The
  Special Edition has Any% and Special% (SE). In 2017 aWay0fLife wrote that the
  speed glitch is easy to activate, which "is why I am not advocating for a
  'glitchless' category" (F-SPEED).
- The Any% timing rule reads: "Timing starts when the Lucasfilm Games logo is
  skipped and ends when the game fades to black after the final cut scene"
  (SRC-GAME).
- Remapping inputs is allowed, but "one key press resulting in one game
  input only" (SRC-GAME).
- The saves rule reads: "Credit Early*, Saveless and Saves (Using save states)
  are all allowed" (SRC-GAME). Credit Early means you "opened the shopkeepers safe early
  in the run without knowing the code" (F-CREDIT).
- In the ScummVM subcategory, "Any speed benefits only possible in ScummVM are
  allowed". Any music device is allowed except "No Music", and runs must use
  ScummVM 2.8 or later (SRC-GAME). Runners fix the ScummVM random seed:
  2756848129 in the WR, 102545962 for HYP and frozenspade (SRC-LB).
- The game-level rules say "Runs are timed in RTA", "require video evidence"
  and "must be a single segment video" (SRC-GAME).
- In the Special Edition, Special% is "completed without ever switching to
  'Classic Graphics Mode'". SE Any% has no rules text (SE).

### 2.2 Text skip and cutscene skip

The rules do not mention `.` (skip one line) or Esc (skip a cutscene), and
neither is restricted. Every guide and every top run uses both:

- G19: "Press Esc or double click ... to [skip]", "skip dialogue by Pressing .
  (period)".
- F-RULES (2017): text speed "doesn't matter. You should be using Period (.)
  Key".
- G-CONV: Esc only sometimes skips whole conversation fragments. Clicking both
  mouse buttons together equals Esc.

### 2.3 Tricks in Part I and whether they change the trial action list

| Trick | Where | Changes the treasure/idol action list? | Against scummrun `rules/glitchless.md` |
|---|---|---|---|
| Logo "speed glitch": Esc at the first sparkle of the Lucasfilm logo makes walking and animations faster for the whole run (G19, F-SPEED). G19 calls it "probably just a quirk". | Boot | No (timing only) | A timing exploit of engine behaviour, so it falls under banned item 2. It cannot be expressed as a sentence anyway. |
| Early door entry: entering "at the beginning of the door opening animation" (G19) | All doors | No | Pixel/timing input, so it is never modelled (banned item 4). |
| Cook timing: enter while the cook's "elbow is still visible" (G19). The cook always leaves about 10 s after the attempt (G19 notes). | Kitchen | No | Not modelled. The plan waits for the scripts. |
| Interrupting the plant pickup early (G19: risky, small gain) | Petal screen | No | Not modelled. |
| Shmoovement (repeated clicks while walking left/up, F-CONV) | Walking | No | Pixel input (banned item 4). |
| `.` / Esc skipping | Everywhere | No (removes waits). G19 dialogue numbers assume skipping. | Out of scope for v1 (not banned). |
| Credit Early: seeded safe combination plus a savestate before entering the store (F-SAFE, IL-CE). Safe pulls happen at the first store visit (WR 3:00). | Store | No. The note of credit is crew-only (Part I ending). | Save/load (banned item 3) and seed knowledge. |
| Invisible-stairs glitch at the safe (G19) | Store, after the trials | No (post-trials crew phase) | Engine glitch (banned item 2). |
| Item slide: monkey, oars and feather skips (G-SLIDE) | Part 3 only | No | n/a |
| Ctrl+W instant win (F-RULES) | Debug | n/a. A ban was requested in 2017. The current rules text does not name it. | Banned item 1. |

The human treasure and idol action sequence below therefore uses only ordinary
verb/object sentences and dialogue choices. Its speed advantage over a literal
replay comes from timing tricks and skips, not from different actions.

### 2.4 Version used by runners

| Run / guide | Version | Evidence |
|---|---|---|
| G19 (2019) | DOS CD (icon inventory) | G19 "it'll show as flowers in the inventory"; F-CD "cd-rom version by default". |
| IL (saruya 2025) | DOS VGA floppy (12-verb UI, text inventory) | Frames (IL 0:06 onward). |
| WR (saruya 2026-08) | DOS CD (9-verb UI, icon inventory), ScummVM 2026.3.0 | Frames (WR 0:05 onward); SRC-LB. |
| HYP (2025-07, former WR) | EGA floppy, ScummVM 2.9.1 | SRC-LB comment. |

The Part I trial route was identical in the IL (VGA floppy) and the WR (CD) as
far as I inspected. The only difference is Credit Early in the WR. Version
differences that matter elsewhere:

- The Special Edition is slower and has a separate board. In SE, Carla never
  uses one insult (G-SW).
- EGA allows a map-teleport trick on Hook Isle (F-EGA, crew only).
- scummrun uses the Mac v5 VGA release. No human runs on Mac were found.

## 3. Route

### 3.1 Conventions

- **[A]** marks a player action: one verb/object sentence, counted 1. "Talk to
  X" counts 1. Its dialogue picks are listed in-line and not counted as actions.
  A conversation that starts by itself on room entry counts 0 actions. Waiting
  is not an action.
- **[T]** marks a player-initiated room transition: walking through an exit,
  door or archway, or clicking a map destination. Entering the island map
  counts as one transition, and leaving it for a destination counts as another.
  Scrolling inside a wide room is not a transition.
- **[S]** marks a scripted transition, where the game moves Guybrush during a
  cutscene. These are listed but not counted.
- **Serves:** `T` treasure, `I` idol, `T+I` both, `$` money (needed for the map
  and shovel, so it counts toward `T+I`), `SW` swordfight only, `CREW` Part I
  crew phase only. Only `T`, `I`, `T+I` and `$` steps are counted in §4.
- **Dia:** dialogue option numbers from G19 (top option = 1). They assume `.`
  and Esc skipping as in G19.
- **Sources:** each step names its sources. **(inferred)** marks anything not
  directly stated or seen.
- **Rooms (my names):**
  - Lookout
  - Dock (SCUMM Bar exterior, wide)
  - Bar main room
  - Bar back room (fireplace, important-looking pirates)
  - Kitchen (includes the outside pier platform with the fish)
  - Island map
  - Clearing
  - Circus tent
  - Fork (forest entrance)
  - Petal screen
  - forest screens
  - X clearing
  - Low street (Citizen of Mêlée, Voodoo shop door)
  - Voodoo shop
  - High street (clock archway, jail, store)
  - Jail
  - Store
  - Trail (cliff path to the mansion)
  - Mansion exterior (poodles)
  - Mansion interior (foyer and stair hall, one wide room; inferred from
    camera snaps in IL)
  - Pier (the dock where Fester throws Guybrush)
  - Underwater

### 3.2 Steps

**Start.** Without skipping, control starts at the Lookout after the Lookout
conversation (AG: "walk down the path from the lookout point"). In every
speedrun, holding Esc through the intro skips it, and control starts with
Guybrush already on the Dock's cliff path (IL 0:02-0:03, WR 0:05). In this
route, Guybrush never talks to the important-looking pirates: G19 says "don't
click on any pirate", IL 0:18-0:21 shows no conversation, and WIKI says
reporting the trials is optional.

**SCUMM Bar kitchen**

1. [T] Lookout → Dock (walk down the cliff path). Serves: T+I. Sources: AG.
   The speedrun intro Esc skips this step (IL 0:03).
2. [A] Open door (SCUMM Bar). Serves: T+I. Sources: G19, AG; IL 0:12-0:15
   ("Open door").
3. [T] Dock → Bar main room. Serves: T+I. Sources: G19; IL 0:18.
4. [T] Bar main room → Bar back room (walk right past the curtain). Serves:
   T+I. Sources: AG; IL 0:18-0:21.
5. [A] Open door (kitchen). The cook comes out. Skip his line and wait about
   10 s for him to leave (he is visible walking left at IL 0:35-0:37). Serves:
   T+I. Sources: G19, AG; IL 0:21-0:37.
6. [T] Bar back room → Kitchen. Serves: T+I. Sources: G19; IL 0:38.
7. [A] Open door (kitchen → outside pier platform, same room). Serves: SW.
   Sources: AG; IL 0:39 ("Open door").
8. [A] Step on the plank to launch the seagull, repeated (G19: twice; AG:
   three times). Serves: SW. Sources: G19, AG; IL 0:40-0:43.
9. [A] Pick up fish. Serves: SW (troll bribe). Sources: G19, AG; IL 0:43-0:44
   (fish in inventory).
10. [A] Pick up pot. Serves: $ (circus helmet). Sources: G19, AG; IL 0:45
    ("Pick up", pot appears).
11. [A] Pick up hunk of meat. Serves: I (poodles). Sources: G19, AG; IL 0:46.
    G19's "Use Pot on Meat" picks up both in one action, but IL uses two
    pickups.
12. [T] Kitchen → Bar back room. Serves: T+I. Sources: IL 0:47-0:48.
13. [T] Bar back room → Bar main room. Serves: T+I. Sources: IL 0:48-0:51.
14. [T] Bar main room → Dock. Esc skips the LeChuck cutscene. Serves: T+I.
    Sources: G19; IL 0:51-0:54.

**Circus (money)**

15. [T] Dock → Lookout (walk up the cliff path). Serves: T+I. Sources: AG;
    IL 1:00-1:06.
16. [T] Lookout → Island map ("walk to path"). Serves: T+I. Sources: AG; IL
    1:06-1:09.
17. [T] Map → Clearing. Serves: T+I. Sources: G19; IL 1:12-1:15.
18. [T] Clearing → Circus tent. Serves: T+I. Sources: G19, AG; IL 1:18-1:21.
19. (auto-conversation, 0 actions) The Fettucini brothers argue. Dia: 1, then
    1, 2, with Esc skips (G19). AG: any option, then "OK, I'll do it", then
    claim to have a helmet. Options visible at IL 1:24. Serves: $.
20. [A] Give pot to Fettucini brothers. Cannon cutscene. Dia: 1 (G19). Receive
    **478 pieces of eight**. Serves: $. Sources: G19, AG; IL 1:27 ("that will
    work as a helmet"); WR 1:25 ("Give pot to Fettucini Brothers"). 478 is
    visible in the IL inventory at 1:42.
21. [T] Circus tent → Clearing. Serves: T+I. Sources: IL 1:39-1:42.
22. [T] Clearing → Map. Serves: T+I. Sources: IL 1:45-1:48.

**Yellow petal**

23. [T] Map → Fork. Serves: I. Sources: G19; IL 1:48-1:51.
24. [T] Fork → Petal screen (top exit; G19: "Go up (there are two exits, both
    lead to the same place)"). Serves: I. Sources: G19, AG; IL 1:51-1:54.
25. [A] Pick up plants (yellow petal). Serves: I. Sources: G19, AG; IL 1:54
    ("Pick up plants").
26. [T] Petal screen → Fork. Serves: I. Sources: IL 1:57-1:58 (inferred room
    identity: a forest screen without the petals, then the map).
27. [T] Fork → Map. Serves: I. Sources: IL 1:59-2:00.
28. [T] Map → Village. The map "village" destination puts Guybrush directly on
    the Dock, not the Lookout. Serves: T+I. Sources: IL 2:00-2:03 (observed).
29. [A] Use yellow petal with hunk of meat, giving meat with condiment. Do it
    while walking; it only has to happen before the poodles. Serves: I.
    Sources: G19; IL 2:06 (inventory shows "meat with condiment").

**Map, chicken, Otis #1**

30. [T] Dock → Low street (walk right along the Dock, through the archway).
    Serves: T+I. Sources: AG; IL 2:09-2:21.
31. [A] Talk to Citizen of Mêlée. Dia: 4, Esc, 2. Buy the **map for 100**
    (478 → 378). Serves: T. Sources: G19, G-CONV; AG gives the option wording;
    IL 2:23 ("Talk to Citizen of Mêlée"), 2:27 (378, map in inventory); WIKI
    price.
32. [A] Open door (Voodoo shop). Serves: CREW. Sources: G19, AG; IL 2:27-2:30.
33. [T] Low street → Voodoo shop. Serves: CREW. Sources: IL 2:31.
34. [A] Pick up chicken. Serves: CREW (rubber chicken for Hook Isle). Sources:
    G19, AG; IL 2:32-2:33.
35. [A] Open door (leave). Serves: CREW. Sources: AG; IL 2:35.
36. [T] Voodoo shop → Low street. Serves: CREW. Sources: IL 2:36.
37. [T] Low street → High street (archway under the clock). Serves: T+I.
    Sources: G19, AG; IL 2:36-2:42.
38. [T] High street → Jail (walk to doorway). Serves: I. Sources: G19; IL
    2:45-2:47.
39. [A] Talk to prisoner (Otis). Skip his line, then leave. Serves: I.
    Sources: G19; IL 2:48 ("Talk to prisoner"); WR 2:45. **The reason for
    this step is inferred**: it probably unlocks the store's breath-mint
    option. See §5.
40. [T] Jail → High street. Serves: I. Sources: IL 2:50.

**Store (shovel, mints; sword for SW)**

41. [A] Open door (store). Serves: T+I. Sources: G19, AG; IL 2:53-2:56.
42. [T] High street → Store. Serves: T+I. Sources: IL 2:57.
43. [A] Pick up shovel (upstairs near the safe). Serves: T. Sources: G19, AG;
    IL 3:00-3:02.
44. [A] Pick up sword. Serves: SW. Sources: G19, AG; IL 3:03-3:04 (sword in
    inventory by 3:14).
45. [A] Talk to storekeeper. Dia: 3, 1, 1, 1, 1, 2 (G19 as corrected in
    F-STORE). The options visible at IL 3:09 are: ask about sword, ask about
    shovel, ask for a breath mint, browse. So 3 = **breath mint (1 piece of
    eight)**. Then 1, 1 = sword (**100**), 1, 1 = shovel (**75**), 2 = browse
    (mapping of the later numbers is inferred). Serves: T+I (sword picks: SW).
    Sources: G19, F-STORE; IL 3:05-3:13 (mints, sword, shovel in inventory at
    3:14); WIKI prices. The treasure+idol-only version needs about 4 picks
    (mint, shovel, "I want it", browse); this is inferred.
    - Credit Early (WR only, CREW): pull/push the safe handle with the seeded
      combination before buying (WR 3:00 "Pull handle"; IL-CE 2:58-3:08, which
      shows the savestate dialog and then "nothing in here but this note").
    - Computed money after the store: 378 − 100 − 75 − 1 = **202**.
46. [T] Store → High street (door already open). Serves: T+I. Sources: IL
    3:15-3:16.

**Mansion #1 (gopher repellent)**

47. [T] High street → Trail (archway on the left). Serves: I. Sources: AG; IL
    3:17-3:21.
48. [T] Trail → Mansion exterior. Serves: I. Sources: AG; IL 3:24-3:30.
49. [A] Use meat with condiment with poodles. They sleep for the rest of
    Part I and are still asleep at IL 4:27. Serves: I. Sources: G19, AG; IL
    3:30-3:38.
50. [A] Open door (mansion front door). Serves: I. Sources: G19, AG; IL 3:39.
51. [T] Mansion exterior → Mansion interior. Serves: I. Sources: IL 3:42.
52. [A] Open door (the doors on the right of the foyer). Esc skips the
    idol-room cutscene. Guybrush receives **gopher repellent**; IL also shows
    staple remover, Manual of Style and wax lips added. Serves: I. Sources: G19
    ("Open door on right, SKIP cutscene, leave"), AG, WIKI; IL 3:42-3:45.
53. [T] Mansion interior → Mansion exterior (front door still open). Serves: I.
    Sources: IL 3:48-3:52.
54. [T] Mansion exterior → Trail. Serves: I. Sources: IL 3:51-3:57.
55. [T] Trail → High street. Serves: I. Sources: IL 4:00-4:02.

**Otis #2 (cake, file)**

56. [T] High street → Jail. Serves: I. Sources: IL 4:05-4:07.
57. [A] Give breath mints to prisoner. Dia: 2 (G19; AG says either of the
    bottom two options). Serves: I. Sources: G19, AG; IL 4:07 ("Give breath
    mints to"). The mints stay in the inventory and are reused in Part II
    (G19).
58. [A] Give gopher repellent to prisoner. Otis gives the **cake**. Serves: I.
    Sources: G19, AG, WIKI; IL 4:12-4:14.
59. [A] Open cake, which gives the **file**. It can be done in the jail or
    while walking. Serves: I. Sources: G19 ("Leave, Open cake"), AG; IL 4:15
    (Guybrush finds the file).
60. [T] Jail → High street. Serves: I. Sources: IL 4:16-4:17.

**Mansion #2 (idol)**

61. [T] High street → Trail. Serves: I. Sources: IL 4:17-4:18.
62. [T] Trail → Mansion exterior. Serves: I. Sources: IL 4:24-4:27.
63. [T] Mansion exterior → Mansion interior (no Open needed; "Walk to door" at
    IL 4:28). Serves: I. Sources: IL 4:28-4:30.
64. [A] Walk to gaping hole. A cutscene follows: the file is used
    automatically and Guybrush takes the idol. Fester and the Governor talk to
    him. Dia: any (G19 lists one pick; AG says "any" for Fester and again for
    the Governor). Esc skips the rest. Serves: I. Sources: G19, AG; IL 4:32
    ("Walk to gaping hole"), 4:35 (fabulous idol in inventory), 4:39 (Fester).
65. [A] Open door (leave by the foyer door). Fester intercepts in a cutscene.
    Dia: any (AG; G19 just says skip). Serves: I. Sources: G19 ("leave, SKIP
    Cutscenes"), AG; IL 4:42 ("Open door").
66. [S] Mansion interior → Pier (Fester cutscene) → Underwater. Serves: I.
    Sources: AG; IL 4:45-4:46. (2 scripted transitions.)
67. [A] Pick up idol (underwater). Serves: I. Sources: G19 ("When you are
    underwater, Pick up Idol, SKIP"), AG; IL 4:46-4:47 (fabulous idol listed).
    **Idol trial done.**
68. [S] Underwater → Pier (Guybrush climbs out). Serves: I. Sources: IL
    4:52-4:53. (1 scripted transition.)

**Swordfight interleave (both IL and WR)**

69. [T] Pier → Dock (walk left). Serves: T. Sources: IL 4:53-4:55.
70. [T] Dock → Lookout. Serves: T. Sources: IL 4:56-5:02.
71. [T] Lookout → Map. Serves: T. Sources: IL 5:03-5:06.
    - SW detour, in human order:
      1. [T] Map → Bridge. The troll stops Guybrush.
      2. [A] Talk to troll, Dia: 1, 2, to learn the "dog" insult (G19, G-SW).
      3. [A] Give fish to troll (G19, AG; IL 5:12-5:24, WR 5:15-5:25).
      4. [T] Bridge → Map (IL 5:27).
      5. [T] Map → House (Smirk) (IL 5:28-5:33).
      6. [A] Open door (Smirk), Dia: 1, Esc, 1, 1, 1, 1, 1, Esc. Pay **30**
         (computed money left: **172**) (G19, AG, WIKI; IL 5:33-5:45).
      7. [T] House → Map (IL 5:45-5:48).

      In the WR, a pirate intercepted Guybrush on the map at this point and
      the first insult fight happened before the dig (WR 5:55-6:30). In the IL
      the runner dodged a pirate on the map (IL 5:48-5:56).

**Treasure (forest dance)**

72. [T] Map → Fork. Serves: T. Sources: IL 5:48-5:57; WR 6:32-6:33.
73. [T] Forest dance. Take these exits in order, starting on the Fork screen:
    **Back, Left, Right, Left, Right, Back, Right, Left, Back**. "Back" is the
    top exit. This is 9 transitions, and the first "Back" is Fork → Petal
    screen. You arrive in the X clearing, a wide room. Serves: T.
    - MAP gives the first word of each map line. MAP and my reading of the
      video cuts are the main support for 9 moves.
    - G19 lists **8** moves after "Enter Fork again": "Go left, Go Right, Go
      left, Go right, Go up, Go right, Go left, Go up", then "Go right and use
      shovel". These match the map's last 8 words. That G19's missing first
      "Back" is the Fork → Petal move is **inferred**; G19 does not say so.
    - IL 5:57-6:26 has 9 hard screen cuts (5:59.2, 6:02.9, 6:06.9, 6:10.5,
      6:14.0, 6:18.1, 6:21.2, 6:23.2, 6:26.3). Guybrush is seen exiting
      up/left/right/left at the first four and right/left/up at the last three.
      There is one extra black frame at 6:16.3, during Guybrush's "dancing
      lessons" line, with the same background before and after. I treat it as
      not a room change (inferred).
    - WR 6:33-7:01 shows the same pattern and the same line.
    - AG/JA and GW disagree; see §5.
74. [A] Walk right, then use shovel with X, which gives the **T-shirt**.
    Serves: T. Sources: G19, AG, BMUG ("off to your right in the last scene");
    IL 6:29 ("Use shovel with X"), 6:32 (T-shirt in inventory); WR 7:05.
    **Treasure trial done.** Both trials are complete at about IL 6:32 video
    time (about 6:28 run time).

**After the segment (not counted).** Walk left out of the X clearing to the
map ("Walk to forest path", IL 6:33). The insult fights with roaming pirates
come next. After 3 wins Guybrush goes to Carla, whose house G19 reaches from the
Fork by: up, up, right, right, left, up, use sign, right over log. The Part I
crew phase follows (Stan, mugs and grog for Otis, the safe or note of credit,
Meathook, Carla).

## 4. Counts (treasure + idol only)

Counts follow the human route in its order, with the SW, CREW and Credit Early
steps removed. I did not reorder anything to find a shorter route.

### 4.1 Actions: 22

| # | Action | Step |
|---|---|---|
| 1 | Open door (SCUMM Bar) | 2 |
| 2 | Open door (kitchen) | 5 |
| 3 | Pick up pot | 10 |
| 4 | Pick up hunk of meat | 11 |
| 5 | Give pot to Fettucini brothers | 20 |
| 6 | Pick up plants (yellow petal) | 25 |
| 7 | Use yellow petal with hunk of meat | 29 |
| 8 | Talk to Citizen of Mêlée | 31 |
| 9 | Talk to prisoner (Otis #1) | 39 |
| 10 | Open door (store) | 41 |
| 11 | Pick up shovel | 43 |
| 12 | Talk to storekeeper (mint + shovel) | 45 |
| 13 | Use meat with condiment with poodles | 49 |
| 14 | Open door (mansion) | 50 |
| 15 | Open door (foyer, right) | 52 |
| 16 | Give breath mints to prisoner | 57 |
| 17 | Give gopher repellent to prisoner | 58 |
| 18 | Open cake | 59 |
| 19 | Walk to gaping hole | 64 |
| 20 | Open door (leave mansion; Fester) | 65 |
| 21 | Pick up idol | 67 |
| 22 | Use shovel with X | 74 |

Using G19's "Use pot on meat" saves one pickup, giving **21**.

Dialogue picks, not counted as actions:

| Conversation | Picks |
|---|---|
| Circus | 4 |
| Citizen | 2 |
| Otis #1 | 0 |
| Store, mint + shovel only | about 4 (inferred) |
| Otis #2 | 1 |
| Mansion (gaping hole) | 1-2 |
| Fester at the door | 0-1 |
| **Total** | **about 12-14** |

### 4.2 Room transitions

**48 player-initiated transitions from the Lookout:** steps 1, 3, 4, 6, 12-18,
21-24, 26-28, 30, 37, 38, 40, 42, 46-48, 51, 53-56, 60-63, 69-72, plus 9
inside step 73. **47** if the speedrun intro skip is used (step 1 is then not
needed).

Of these, **8 are island-map transitions** (steps 16, 17, 22, 23, 27, 28, 71,
72):

- Lookout → Map, twice
- Map → Clearing
- Clearing → Map
- Map → Fork, twice
- Fork → Map
- Map → Village

There are also **3 scripted transitions** (steps 66 and 68), not counted.

### 4.3 Excluded steps, listed separately

The swordfight-only steps come to 7 actions (8 if G19's two plank launches
count separately) and 4 transitions, plus 2 dialogue picks in the store
conversation.

- Actions:
  - open the kitchen pier door
  - step on the plank (1 action; 2 if both launches count)
  - pick up fish
  - pick up sword
  - talk to troll
  - give fish to troll
  - open door at Smirk's
- Transitions:
  - Map → Bridge
  - Bridge → Map
  - Map → House
  - House → Map

  These replace nothing, because the treasure path goes Lookout → Map → Fork
  either way.

The crew-only steps (chicken) are 3 actions (open, pick up chicken, open) and
2 transitions (Low street ↔ Voodoo shop).

Credit Early (WR only) adds about 4 or more safe Pull/Push actions plus taking
the note, in the first store visit.

The human route as run in the IL, without Credit Early, totals:

- 22 + 7 + 3 = **32 actions** (33 with two plank actions)
- 48 + 4 + 2 = **54 transitions** from the Lookout (53 with the intro skip)

## 5. Open questions and uncertainties

1. **Forest dance directions.** The sources disagree:
   - MAP gives 9 first words: Back, L, R, L, R, Back, R, L, Back.
   - G19 lists only 8 moves after entering the Fork. They match the map's last
     8 words. That its missing first Back is the Fork → Petal move is
     inferred.
   - AG/JA give "take either exit at the top" plus L, R, L, R, U, R, L, U,
     **L, U** (11 moves).
   - GW (Special Edition) gives N, W, N, W, N, N, E, W, N, E.
   - IL and WR footage fits the 9-move reading, if the black frame during the
     "dancing lessons" line is not a room change. That interpretation is mine.

   I count 9, but this should be confirmed against the forest room scripts.
   It is also unconfirmed whether the forest checks for owning the map at all.
   G19 implies the Sword Master path only works with the map. If the X is
   reachable without the map, a shortcut exists: skip the Citizen and save 100.
2. **Otis #1 (step 39).** G19 and both videos do it right before the store, but
   no source says why. My guess is that it unlocks the breath-mint option or
   Otis's mint dialogue. Verify in the scripts. If it is not needed, the count
   drops by 1 action and 2 transitions.
3. **Store dialogue numbers.** Only option 3 = mint was seen on screen (IL
   3:09). The meaning of the later 1, 1, 1, 1, 2 is inferred, and the
   shovel-only variant without the sword is not observed in any source.
4. **Is "Open door (kitchen)" (step 5) needed?** This is worth ±1 action.
   - AG enters without an action: it waits near the fireplace until the cook
     comes out on his own.
   - G19 says to open the door, and its notes say the cook leaves about 10 s
     after you "try to enter".
   - In the IL, the cook appears after a misclicked verb on the door and a
     generic failure line (0:21-0:27), not after a clean Open.

   So it is unclear whether the cook runs on a timer or is triggered by the
   attempt. I keep step 5 in the count (22). Check the bar and kitchen scripts.
5. **Plank launches (step 8).** G19 says 2, AG says 3. This is SW only, so it
   does not affect the counts.
6. **Mansion dialogue picks.** G19 lists one "any" pick plus skips. AG lists
   picks for Fester and the Governor, then Fester again at the door. The
   difference probably comes from Esc skipping, but this is not verified.
7. **Room boundaries are partly inferred.**
   - IL camera snaps are the evidence that the mansion foyer and stair hall
     form one wide room, that the Pier is a separate room from the Dock, and
     that the X clearing is wide (Fork-side undergrowth plus the sign).
   - The Petal → Fork return (step 26) is also inferred.

   CD (WR) scrolling was consistent with this. Script room numbers would
   settle it.
8. **Idol trial completion point.** I mark it at "Pick up idol" underwater
   (step 67). The trial flag may be set at pickup, at the climb out, or on
   returning to the Dock. This matters for a goal condition.
9. **Version.** Human evidence is DOS VGA floppy (IL), DOS CD (WR, G19) and EGA
   (HYP, comment only). scummrun uses Mac v5 VGA. No differences in the Part I
   trial steps were seen between floppy and CD. Mac was not checked. The IL
   floppy inventory shows extra mansion items (staple remover, Manual of
   Style, wax lips); I did not check whether CD gives the same.
10. **Money.** 478 and 378 are seen in IL. The later balances (202 after the
   store, 172 after Smirk) are computed from WIKI prices, not observed.
   Treasure + idol needs only 176 (map 100 + shovel 75 + mints 1), so the
   circus is mandatory: the only other money source found, the Men of Low
   Moral Fiber (WIKI, AG), gives a coin or two. WIKI says the mints cost 1; AG
   says the storekeeper "will give" them.
11. **Not watched:** LeoLitz's YouTube video guide (named as the most
    up-to-date in F-CONV) and HYP's video. The route here comes from the
    2025-2026 dalek.zone videos plus G19, not from those.
