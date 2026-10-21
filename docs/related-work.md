# Related work, leaderboard context and feasibility

This document places scummrun in context. It covers what speedrun.com allows for *The Secret of Monkey Island* (MI1), earlier automated and tool-assisted runs, related research, what is and is not new here, and how far the method could reach across ScummVM games.

All sources were accessed on 2026-10-05. Bracketed IDs such as `[SRC-GAME]` point to the reference table at the end, which lists every URL with its access date.

Each claim carries one status tag:

- **D (direct):** the writer of this document fetched the primary source, for this revision or for the previous revision written the same day.
- **C (checked):** a separate verification pass re-read the primary sources. Where that pass found an error, the corrected claim is given here, not the original.
- **U (unverified):** reported by a research pass and not independently checked. Treat these as leads.

Sentences that begin "Inference:" are a reading of the sources, not something a source states. Section 6 lists the statements from the previous revision that this revision corrects.

## 1. speedrun.com rules for MI1

### 1.1 Pages and categories

speedrun.com blocks page fetches, so everything in this section comes from its public JSON APIs (v1 and the v2 endpoints the site itself uses). They serve the same rules, runs and forum posts as the pages do.

speedrun.com has two pages dedicated to the game `[SRC-MODS, C]`:

- **The Secret Of Monkey Island** (id `w6j4m46j`, URL `tsomi`), created 2015-11-06. The ruleset allows real time only, requires verification and video, allows emulators and shows milliseconds. Platforms are PC, Sega CD, Amiga, Atari ST and FM Towns. Moderators are frozenspade and thewoofs (super-moderators) and aWay0fLife and LeoLitz.
- **The Secret of Monkey Island: Special Edition** (id `9d3rvxgd`, URL `tsomise`), created 2019-08-29 (UTC). Real time only, verification and video required, emulators not allowed, no milliseconds. Platforms are PlayStation 3, Xbox 360 and PC. Moderators are frozenspade, thewoofs and LeoLitz.

A third page, "Multiple Monkey Island Games", holds marathon categories that include MI1, such as Mega-Monkey Marathon (all six games) `[SRC-MULTI, C]`.

The `tsomi` categories:

| Category | Type | Timing and key rules (rules text) | Top run |
|---|---|---|---|
| Any% | full game | "Timing starts when the Lucasfilm Games logo is skipped and ends when the game fades to black after the final cut scene" | saruya 21:52 (2026-08-23, ScummVM, Saves, Credit Early) |
| Swordmaster% | full game | "Timing starts when the Lucasfilm Games logo is skipped and ends when you recieve the shirt after beating Carla." Also "Saving and Loading to reroll RNG is ALLOWED." | hyperformance 6:07 (2024-09-21) |
| Any% (Talkie) | full game | Opens "Play through the game as fast as possible using the Ultimate Talkie Edition patch", then repeats the Any% paragraphs with small wording changes | no runs in any status |
| Demo | full game | "Timer starts on first movement and ends when you select the final dialogue. Only runs on Dosbox are allowed. Max Cycles allowed." | mikeSpeedyAdventures 2:37.300 (2023-07-06) |
| Beat the Sword Master | per level | rules field empty (null) | no levels are defined, and the category has no runs in any status |

Sources: categories and rules `[SRC-GAME, D]` `[SRC-GAMEDATA, C]`; top runs `[SRC-LB, D]`; levels `[SRC-LEVELS, D]`; run counts per status `[SRC-RUNS, C]`. The "recieve" spelling is in the source.

The Any% category has three variables, all mandatory `[SRC-GAME, D]` `[SRC-VAR, C]`:

- **Category:** Saveless or Saves. A filter, not a separate board.
- **Credit Early:** No or Yes. Also a filter.
- **Subcategory:** ScummVM (default) or NON-ScummVM. This is the only variable that splits the leaderboard. The ScummVM value's rules read: "Any speed benefits only possible in ScummVM are allowed. Any Music Device may be used EXCEPT No Music. Going forward we are requiring all runs submitted in this subcategory to be performed on ScummVM versions 2.8 and above." The NON-ScummVM value's rules read: "Going forward we are requiring runs performed in this subcategory to list the emulation used in the comments."

All three variables belong to Any% only. Swordmaster%, Any% (Talkie), Demo and Beat the Sword Master have no variables `[SRC-VAR, C]`.

The game-level rules read: "Runs are timed in RTA", "Runs require video evidence" and "Runs must be a single segment video" `[SRC-SUM, D]` `[SRC-GAMEDATA, C]`.

The Special Edition's game-level rules give its timing: "Timer starts on selecting "Play New Game" (or pressing "OK" when playing in a non-english version). Timer stops when the screen fades to black after choosing the last dialogue after defeating LeChuck." Its Any% category has an empty rules field. Special% adds "Game must be completed without ever switching to "Classic Graphics Mode"." `[SRC-SE, D]` `[SRC-GAMEDATA, C]`.

Run totals: `tsomi` has 230 verified runs (Any% 153, Swordmaster% 62, Demo 15) and `tsomise` has 65 (Any% 44, Special% 21). These per-category counts add up exactly to each game's total, so no hidden or archived category holds verified runs `[SRC-RUNS, C]`. Neither game has a glitchless, TAS, segmented or miscellaneous category, or a variable for any of these `[SRC-GAMEDATA, C]`.

### 1.2 What Any% allows

No rules field on either game mentions glitches in general, the logo speed glitch, Ctrl+W, cheats or ScummVM random seeds. No glitch is banned in writing `[SRC-GAMEDATA, C]`.

The written constraints, with the scope each one has in the source `[SRC-GAMEDATA, C]` `[SRC-VAR, C]`:

- **Timing** start and end, per category (table above).
- **Keybinds** (Any%, Swordmaster%, Any% (Talkie)): remapping in ScummVM or with tools such as AutoHotkey is allowed, and "Keybindings should stick to the rule of one key press resulting in one game input only." The wording is "should", not "must".
- **Disclosure** (Any%, Any% (Talkie)): "Please state which category the game was played in and if you got Credit Early when submitting runs."
- **ScummVM subcategory of Any% only:** ScummVM 2.8 or later, and any music device except No Music.
- **NON-ScummVM subcategory of Any% only:** list the emulation used.
- **Any% (Talkie):** play with the Ultimate Talkie Edition patch.
- **Demo:** DOSBox only.
- **Game level (`tsomi`):** RTA timing, video evidence, a single-segment video.
- **Special Edition:** Special% bans Classic Graphics Mode, and the game's ruleset does not allow emulators.

The Any% rules explicitly allow several things `[SRC-GAME, D]`:

- "Credit Early*, Saveless and Saves (Using save states) are all allowed." The asterisk points to the forum. There, in October 2020, aWay0fLife defined Credit Early as having "Opened the shopkeepers safe early in the run without knowing the code". The same post records that Saves and Saveless had been split into two categories and were merged back because "Only 2 runners ever submitted to the saves category" `[SRC-F-CREDIT, C]`.
- "Any speed benefits only possible in ScummVM are allowed" (ScummVM subcategory).

The moderator-credited Any% guide adds that in the Saves category "only save files created during the run may be loaded". This is guide text, not ruleset text `[SRC-GUIDE-ANY, C]`.

Glitches in active use:

- **The logo speed glitch.** On 2017-08-18, aWay0fLife posted "Speed glitch is viable and easily activated" `[SRC-F-SPEED, C]`. He is a moderator now. The checker inferred from run-examination records that he probably became one about two weeks after this post.
  - The post opens with a caveat: "I have only confirmed this glitch works on ScummVM version of the game (version 1.9.0.2)".
  - It says the glitch "allows you to move much faster, possibly saving 4 minutes in all", and describes skipping the Lucasfilm Games logo after the "sparkle" above the L.
  - It ends: "So basically anyone running the game should be using this glitch because it saves SO much time... Also because it is easy to activate is why I am not advocating for a "glitchless" category."
  - The thread has no replies. It is the only use of the word "glitchless" in the game's 25 forum threads. It declines to propose a glitchless category; it is not a debate about one.
  - scummrun's own engine analysis gives the mechanism: the early skip leaves var 19 (`VAR_TIMER_NEXT`) at 5 instead of 6 for the whole run, so frame-paced motion and animation run 6/5 as fast, while `delay()` and talk timers keep their length `[L-CMP]`.
- **Its discovery.** Two days earlier, aWay0fLife had pointed out a speed difference in Cryptones' 36:50 record (run dated 2013-04-11). That thread did not name the logo skip; the link was made in the 2017-08-18 post and in his run comment of 2017-08-17 `[SRC-F-ODDITY, C]`.
- **The guide's view.** The Any% guide (credited to aWay0fLife and LeoLitz, last updated 2019-11-14) says: "This game has a "glitch" where if you skip the opening Lucasfilm Games logo at the correct time, it allows guybrush to walk slightly faster", that "It also speeds up animations throughout the game", and that "Activating this glitch is essential to getting a good time, thankfully it's very easy." Its notes add: "We call the faster walking thing a glitch, but to be honest I am not sure if it is, it's probably just a quirk of the game." It also names an "invisible stairs glitch" at the shopkeeper's safe `[SRC-GUIDE-ANY, C]`. Both credited authors are moderators today; whether they were in 2019 cannot be confirmed from the API `[SRC-GUIDE-ANY, C]`.
- **The Special Edition.** In 2025 moderators welcomed a report that the speed glitch also works on the Special Edition `[SRC-F-SE, C]`. The reporter's method ("Pressing Backspace to skip the scene at that point will get you the speed glitch") and thewoofs' measurement of 7.1 s versus 9.3 s for one walk are reported but not checked `[SRC-F-SE, U]`.
- **Item slides.** The current record's run comment says "We've completed the item slide trilogy!" and "Finishing a run without crashing the game is getting harder" `[SRC-WR, D]`. A guide on the page covers the slides `[SRC-GUIDES, D]`. A floppy-version guide by saruya (2026-08-09) says: "The main disadvantage of the item slide state is that the game will crash if you do certain actions." `[SRC-GUIDES, U]`.

Random seeds:

- All three top Any% runs state a fixed ScummVM seed `[SRC-RUNREC, C]`:
  - saruya's 21:52 record: "Played on ScummVM 2026.3.0 using 2756848129 as random seed" `[SRC-WR, D]`.
  - hyperformance's 24:23: ScummVM 2.9.1, seed 102545962.
  - frozenspade's 25:31: "Played on ScummVM 2.9.0 ... Set Seed: 102545962".
- The page hosts a seed finder by saruya (updated 2026-07-23). It prints seeds that give "the fastest possible catacombs" and seeds that work for Credit Early `[SRC-SEED, D]`.
- A guide covers catacombs RNG manipulation `[SRC-G-CATA, D]`. It gives save timings such as "You need to save the game between 26 and 29 frames before entering the store." `[SRC-G-CATA, U]`.
- Inference: no rule mentions seeds, but moderators verify runs that disclose them. Seed choice is accepted in practice, probably under "Any speed benefits only possible in ScummVM". That is a reading of moderator behaviour, not written policy `[SRC-RUNREC, C]`.

Ctrl+W is the original game's instant-win key. In the 2017 "Rules?" thread, aWay0fLife described it ("Are you sure you want to win? Y/N"), and in December 2017 a runner asked the moderators to ban it explicitly. No moderator replied `[SRC-F-RULES, C]`. The current rules do not mention it `[SRC-GAMEDATA, C]`. The site-wide rules (updated 2026-02-05) say "Cheat codes are disallowed" and "Just because something isn't in writing doesn't mean it is allowed" `[SRC-SITE, D]`. Inference: a Ctrl+W run would be rejected under the site rules and would also miss the Any% end condition, even though the game rules are silent.

Two further points are reported but not checked:

- In May 2025 saruya read the keybind rule as banning two-key combinations such as Ctrl+R and asked for an exception for ScummVM quicksave and quickload. The thread has no replies `[SRC-F-KEYS, U]`.
- The board's ScummVM build has changed over time. In 2019 aWay0fLife recommended a modified 2.1.0 build that is "almost 2 minutes faster over the course of the run", and in 2020 LeoLitz recommended another modified build. The rules now require 2.8 or later `[SRC-F-BUILDS, U]`. Inference: human times depend on the ScummVM build, which matters when they are set beside scummrun's ScummVM v2026.3.0 figures.

To answer the question directly: Any% bans no glitch in writing. Within its timing, keybind and subcategory rules it allows glitches, save states, Credit Early and, in practice, seed choice.

### 1.3 Glitchless, TAS and bot runs

- **Glitchless.** There is no glitchless category on either page `[SRC-GAMEDATA, C]`. The community has no definition of "glitchless" for this game: the rules are silent, the guide doubts the speed-up is a glitch at all, and in 2017 the runner who documented it declined to propose such a category `[SRC-GUIDE-ANY, C]` `[SRC-F-SPEED, C]`. Inference: `rules/glitchless.md` is scummrun's own standard, not a community one, and any glitchless claim has to carry its own definition `[L-RULES]`.
- **TAS.** There is no TAS category. The site rules list "Tool-assisted emulator features such as save-states, re-recording, or frame advance" as not allowed, with the note that "some games may allow things on this list - if it is explicitly stated in the game or category rules that it's allowed, then it's fine for that game or category." `[SRC-SITE, D]`. The MI1 board uses that override for save states only: "Saves (Using save states)" in Any% and the RNG-reroll clause in Swordmaster% `[SRC-GAMEDATA, C]`. A research pass found no TAS category on any classic SCUMM game's board it checked `[SRC-SCUMM-BOARDS, U]`.
- **Bots.** The site rules say "The game itself must not be altered or controlled with outside tools. Exceptions can be made on a community basis (e.g., character creation scripts, logo/cutscene removal dlls, etc.)" `[SRC-SITE, D]`. scummrun runs a patched ScummVM that pushes sentences into the engine `[L-README]`. Inference: a scummrun run is a game controlled by an outside tool, and its injected sentences do not fit "one key press resulting in one game input only". It cannot go on any current MI1 board unless the moderators create a category for it.
- **Segments.** In June 2025 runners proposed individual levels, one per part of the game. The thread ends without a moderator decision `[SRC-F-IL, D]`. The page still has no levels and no IL runs `[SRC-LEVELS, D]` `[SRC-SUM, D]`. saruya keeps a playlist of personal individual-level videos outside speedrun.com `[SRC-F-IL, D]`.
- **Talk of an MI1 TAS.** In May 2025 LeoLitz wrote that "maybe TAS could find some weird dances that are faster for certain walks" `[SRC-F-SAFE, U]`. In July 2025 mikeSpeedyAdventures mentioned "working on the TAS" (section 2.1) `[SRC-F-SAFE, D]`.
- **TASVideos** is the usual home for tool-assisted runs. Its ScummVM page requires libTAS with "An official release of ScummVM" and a movie that starts with no save data `[TV-SCUMMVM, D]`. It also asks that the game be rated Good or Excellent on ScummVM's compatibility list `[TV-SCUMMVM, U]`. Section 2.2 discusses what TASVideos has accepted and rejected.

### 1.4 What our run is comparable to

Our result is a held-out mean of 6:11.53 (22,292.0 ticks, stdev 48.3, seeds 31 to 60) `[L-CMP]` `[L-README]`. The segment runs from first control on the Mêlée dock until bits 85 (idol trial) and 86 (treasure trial) are both set `[L-SEG, D]`. The route ends by digging up the treasure, a T-shirt.

It is not comparable to any speedrun.com leaderboard:

- **Any%** times the whole game, from the logo skip to the final fade `[SRC-GAME, D]`. Our segment is part of Part I.
- **Swordmaster%** (record 6:07 by hyperformance, 2024-09-21; the video is titled "The Secret of Monkey Island Swordmaster% Speedrun in 6:07 [WR]") `[SRC-RUNREC, C]` has a different goal. It ends when Carla gives Guybrush her shirt after the sword-master fight. In the game's scripts that is object 596, "100% Cotton T-shirt", picked up in global script 116 right after bit 20 (sword master beaten) is set. Our segment ends at a different object, 752 "T-shirt", dug up in the treasure room, and it also requires the idol trial `[L-SCRIPTS, D]` `[SRC-GAMEDATA, C]`. So the two categories cover different trials. Three further differences are smaller:
  - Swordmaster% starts at the logo skip. In the two human videos `docs/comparison.md` analyses, dock control comes 2 to 5 s after it `[L-CMP]`.
  - Swordmaster% runs use the logo speed glitch, which the guide calls "essential" and our rules ban `[SRC-GUIDE-ANY, C]` `[L-RULES]`. Inference: the 6:07 run uses it; the video was not inspected.
  - Swordmaster% allows save-and-load RNG rerolls, while we report a mean over seeds.

  The two times happen to be close, but they should not be set side by side.
- **No IL board exists** for any part of the game `[SRC-LEVELS, D]`.

The closest human reference is the same segment cut out of human videos. `docs/comparison.md` re-bases saruya's 2025 Part 1 IL video and the 2026 record to dock control and removes the swordfight and crew-only detours `[L-CMP]`:

| run | time | ours (6:11.53) minus this run |
|---|---:|---:|
| IL adjusted (swordfight removed, ±5 s) | 5:31 | +40.5 s (+12.2%) |
| WR adjusted | 5:29 | +42.5 s (+12.9%) |
| IL crew-free (also removes the chicken) | 5:22 | +49.5 s |
| WR crew-free (also removes Credit Early) | 5:14 | +57.5 s |

So the human runners are about 40 to 58 s faster on this segment. The same analysis attributes most of the gap to the logo speed glitch, which our rules ban, and the rest to pixel and timing tricks such as early door entry and "shmoovement" `[L-CMP]` `[L-RULES]`. These human figures are estimates taken from video, not measurements. The versions also differ: the human runs are DOS releases (VGA floppy and CD), while scummrun runs the Mac release, and ticks do not transfer between versions `[L-CMP]` `[L-README]`. The record was played on ScummVM 2026.3.0 `[SRC-WR, D]`, and scummrun pins ScummVM v2026.3.0 `[L-README]`.

The rules on the two sides differ in both directions:

- **Humans** use the logo glitch, item slides, save states and chosen seeds `[SRC-F-SPEED, C]` `[SRC-WR, D]` `[SRC-SEED, D]`.
- **The bot** may use none of these, and it may not choose a route because a particular seed favours it `[L-RULES]`.
- **The bot's advantage** is that it acts on the first frame a click would count, with no reaction time `[L-CMP]`.

No human glitchless time exists for comparison, because there is no glitchless category `[SRC-GAMEDATA, C]`.

On "world record" wording. Inference from the points above: the term does not apply, for three independent reasons. No board matches the segment. The site rules exclude runs controlled by outside tools. Human runs on the same segment are faster, under looser rules. A defensible description is: "an automated route for the Part I idol and treasure segment, planned under self-defined glitchless rules and measured in engine ticks; we found no earlier automated or planned route for this segment." Section 2 explains why that last part is "we found no", not "none exists" `[L-RULES]` `[NEG-SEARCH, U]`.

## 2. Earlier automated and TAS runs

### 2.1 Monkey Island 1

**Tool-assisted runs.**

- **TASVideos has nothing for MI1.** A verification pass downloaded the full TASVideos API listings: 5,275 games, 7,377 publications including obsoleted ones, and 10,685 submissions. The 86 missing submission ids each return 404. No game entry, publication or submission exists for *The Secret of Monkey Island* on any platform `[TV-API, C]`. The same is true of Day of the Tentacle, Full Throttle, Sam & Max Hit the Road, Fate of Atlantis, the Last Crusade graphic adventure, Zak McKracken, The Curse of Monkey Island, The Dig and Grim Fandango. The Indiana Jones entries that do exist are non-SCUMM games on Genesis, NES, Game Boy Color, N64, SNES and DS `[TV-API, C]`. Apart from the 2012 thread below, MI1 appears only as a request: on the DOS and Amiga wishlist threads and on the legacy ListOfIdeas page `[TV-API, C]` `[TV-MI, C]`.
- **The 2012 TASVideos thread.** "Monkey Island, is it TASable?" (topic 12828) has nine posts, all from 3 to 5 May 2012, and produced no TAS on the site `[TV-MI, C]`:
  - Only one reply, from Warepire, said point-and-click games "usually do not gain much from TASing since most of the time it will be frame-perfect clicking". He qualified it: glitches or sequence breaks that cannot be done in real time "could be interesting".
  - Tub called three things "difficult" to optimise: the swordfights ("just the right amount of questions/answers"), the "random maze-sections" and rowing.
  - DrJones noted that the Swordmaster can be reached without the forest guide if the path is known, and that the path is generated randomly each game.
  - MUGG linked a 39:12 multi-segment real-time run by Mike [AS], which the opening poster took as "something to aim for".
- **A referenced MI1 TAS that could not be located.** In a July 2025 speedrun.com thread about the safe seed, mikeSpeedyAdventures writes "when I was working on the TAS", and saruya replies "after reading your message and watching your TAS" `[SRC-F-SAFE, D]`. Neither post names the game or links the video. mikeSpeedyAdventures' speedrun.com profile links no TAS `[SRC-USER-MIKE, U]`. His TASes that could be found are of other games (section 2.2). `docs/human-route.md` also lists this TAS as "no public page found" `[L-HR]`.
- **A BizHawk attempt.** On 2025-10-11 TASVideos user Emil_Borg said they were trying to TAS the MI1 CD version on BizHawk's new DOSBox-X core, but its CD audio did not play `[TV-BORG, U]`. The relevant CD fixes shipped in BizHawk 2.11.1 on 2026-05-01 `[BIZHAWK, C]`. Emil_Borg's user files contain no Monkey Island movie `[TV-BORG, U]`.
- **Video sites.** A verification pass re-ran YouTube and Twitch searches and found no MI1 TAS. The only TAS hits were for other SCUMM games (section 2.2) `[YT-SEARCH, C]`.

**Automated play.** Several projects let LLM agents play MI1 through engine-level hooks. A verification pass found that this refutes an earlier claim that no MI1 bot run exists `[AI-YT, C]`. None of them is a completion, a speedrun or a timed run.

- **ScummBench** (rabengraph/scummbench, created 2026-04-11). It pairs with a ScummVM fork that adds SCUMM telemetry and a JavaScript bridge to a WebAssembly build. Its stated question is whether "exposing symbolic SCUMM state in-browser materially improve[s] an agent's ability to play the game compared to pure vision" `[SCUMMBENCH, D]`. A video published 2026-04-13 shows Claude Opus 4.6 playing MI1 Part I through it and getting stuck at the troll bridge `[AI-YT, C]`. Its two documents describe the sentence call differently `[SCUMMBENCH, D]`:
  - the agent runbook says `__scummDoSentence({verb, objectA, objectB})` "Queues directly into the engine's sentence stack";
  - the engine's `AGENT_HARNESS.md` says "`doSentence` is a convenience wrapper that clicks the verb, then the object(s) with appropriate timing", and that clicking a verb "triggers `runInputScript(kVerbClickArea, verbId, 1)`".
- **scummvm-mcp** (xavierhardy, GPL-3.0; created 2026-04-20 and last pushed 2026-09-17 `[MCP-README, U]`). This ScummVM fork adds an MCP server so that LLM agents can play SCUMM games.
  - Its README calls MI1 "the primary supported game", tested end to end, and lists "Work reliably outside MI1" under "What It Cannot Do" `[MCP-README, C]`.
  - For MI1, a targeted `act` goes to `ScummEngine::doSentence(verb, A, B)`, and a verb-only `act` calls `runInputScript(kVerbClickArea, verb, 1)`. The `walk` tool calls `startWalkActor` directly, without a sentence `[MCP-SRC, C]`.
  - scummrun's bridge uses the same two entry points, `doSentence()` and `runInputScript(kVerbClickArea, verb, 1)` `[L-PATCH, D]`.
  - Its README mentions no timing, planning, route optimisation or save/load, and says actions may time out at a 20 s hard limit `[MCP-README, U]`.
  - The "AI Adventure XP" YouTube channel uses it. Videos include Claude Haiku 4.5 on the MI1 demo (2026-04-20), Qwen 3.8 27B on the MI1 demo (2026-08-21), "AI Agents Race Through Monkey Island: Ornith 1.5 vs Ornith 1.0" on the MI1 demo (2026-08-24), whose description says the agents "had a hard time playing it", and a Qwen run of the German release `[AI-YT, C]`. A GPT-5.6 versus Claude Opus 5 video on MI2 (August 2026) is reported but not checked `[AI-YT, U]`.
- **philnash/monkey-island-webmcp** (created 2026-09-01). It patches WebMCP into a WebAssembly build of ScummVM and gives browser agents tools such as `observe_game`, `perform_action`, `walk_to` and `choose_dialog` `[WEBMCP, C]`.
- **An LLM playing through screenshots (May 2025).** Luis HG built a vision-LLM loop that drives ScummVM through screenshots and clicks on a numbered grid, on Monkey Island and Maniac Mansion. It explores; it does not plan a route `[LUISHG, C]` `[LUISHG, U]`.
- **A routing proposal without code.** mnuhn/ScummGameGraph (created 2026-06-28) is a README-only RFC. It proposes decompiling SCUMM scripts with scummvm-tools, building a dependency graph of global-variable reads and writes, and running Dijkstra or A* to get "a mathematically optimized speedrun route" for MI1 and MI2. It has no implementation and covers neither engine replay nor tick measurement `[SGG, U]`.
- **Not prior work.** ScummVM's 1 April 2023 news post about AI playing adventure games is an April Fools' joke `[SVM-APRIL, C]`.

Inference: engine-level sentence injection for MI1 and agent play of MI1 were public by April 2026 (ScummBench 2026-04-11, scummvm-mcp 2026-04-20). This document does not establish whether either predates scummrun's bridge.

**Negative result.** No earlier work was found that plans a time-optimal or provably shortest route for MI1, or for any commercial graphic adventure, and replays it in the engine `[NEG-SEARCH, U]`. This is a search result, not proof of absence. The searches had gaps: web-search budgets ran out in several passes, GitHub repository search skips forks, and Reddit, Mixnmojo, Discord and the ScummVM forums were barely searched `[NEG-SEARCH, U]`.

### 2.2 Other SCUMM and ScummVM games

**TASVideos publications made with ScummVM.** There are seven, all libTAS input movies running ScummVM `[TV-SVM, C]`:

| Game | ScummVM engine | Time | Authors | Tools | Movie |
|---|---|---:|---|---|---|
| Backyard Baseball ("Pick-Up Game") | SCUMM HE | 4:45.050 | TiKevin83 | libTAS 1.4.2, ScummVM 2.2.0 | 4496M |
| Pajama Sam: No Need To Hide When It's Dark Outside | SCUMM HE | 0:43.717 | EZGames69 | libTAS 1.4.4, ScummVM 2.6.0 | 4985M |
| Putt-Putt Saves The Zoo | SCUMM HE | 0:29.133 | Lobsterzelda | libTAS 1.4.3, ScummVM 2.7.1 | 5571M |
| Myst: Masterpiece Edition | Mohawk | 0:27.000 | Spikestuff, Gelly | libTAS 1.4.3, ScummVM 2.0.0 | 4669M |
| Phantasmagoria | SCI | | darkshoxx | libTAS, ScummVM | 5827M |
| Riven ("best ending") | Mohawk | | | libTAS, ScummVM | 5840M |
| Door | | | | libTAS, ScummVM | 6408M |

Blank cells were not recorded by the research passes. Sources: the publication list `[TV-SVM, C]`; Myst, Pajama Sam and Putt-Putt details `[TV-MYST, D]` `[TV-PJS, D]` `[TV-PUTT, D]`; the Backyard Baseball row (status Published, libTAS with ScummVM) `[TV-SVM, C]`, with its time and versions `[TV-SVM, U]`. The Pajama Sam judge wrote that "the movie syncs about 50% of the time" `[TV-PJS, D]`. A further Phantasmagoria submission (#10699, libTAS 1.4.8 with ScummVM 2026.3.0, 2026-08-29) is pending `[TV-SVM, C]`. These are hand-made input movies, not planned routes. Inference: the very short times suggest skips or glitches, but that was not checked.

**The one LucasArts SCUMM submission was rejected.** darkshoxx submitted Loom (DOS EGA) as #9181 on 2024-07-13: 0:43.76, ScummVM 2.8.1 with libTAS 1.4.5, launched with `--random-seed=0`, at a framerate of 1000 using ScummVM's Ctrl+G fast mode `[TV-LOOM, C]`. Judge feos rejected it ("Reason: Emulation"):

- he wrote that Ctrl+G "is just unthrottled emulation speed, similar to turbo speed in traditional emulators", and that Ctrl+G and Ctrl+F are "emulator-only hotkeys";
- "The logic behind running emulators in libTAS is in sticking to reasonably accurate emulation, not in using emulator-only features to speed things up to absolute maximum";
- the movie "doesn't sync on framerate lower than 1000". He had tried 60, 100 and 500 fps.

The author later posted it on YouTube as "Loom EGA TAS really REALLY fast (Abandoned Project)" (2026-03-22) `[TV-LOOM, C]`.

Inference from these two items: TASVideos accepts libTAS with an official ScummVM build at normal timing, including SCUMM HE games. What it rejected was a ScummVM-only speed feature. An MI1 route expressed as libTAS input on an official build at normal timing would follow the accepted precedent. scummrun's patched build and injected sentences would not fit as they stand.

**TASVideos context.**

- **Monkey Island 2.** TASVideos has a game page for the DOS version (4437G) with no publications, submissions or user files. Its Game Resources page quotes Ron Gilbert's code for when LeChuck randomly appears, linking Gilbert's post on speedrunning `[TV-MI2, C]`.
- **Maniac Mansion (NES).** The current publication is 5:19.47 by Arc and ShesChardcore, movie 6029M, made on FCEUX `[TV-MM, D]` `[TV-MM, C]`. It runs on an NES emulator, not ScummVM.
- **The Curse of Monkey Island thread** (topic 18435). April 2026 posts discuss a new glitch that sends Guybrush to Blood Island early. It works in ScummVM but crashed the original interpreter under DOSBox-X. Staff warned that ScummVM "might also introduce bugs that aren't possible in the original games", and Spikestuff wrote that "libTAS x ScummVM is the way to go". eien86's homepage lists The Curse of Monkey Island and the DOS Last Crusade as possible future projects `[TV-COMI, U]`.

**TASes outside TASVideos.**

- **mikeSpeedyAdventures** published libTAS and ScummVM TASes as Twitch highlights `[TW-MI2, D]` `[RENTRY, U]`:
  - Monkey Island 2 (ScummVM, DOS) in 29:47. His notes give libTAS 1.4.4, ScummVM 2.8.0git and `--random-seed=48 monkey2-de` (the German release), dated 2023-05-31.
  - Day of the Tentacle in 14:04.91: libTAS 1.4.5, ScummVM 2.9.0git, `--random-seed=1 tentacle`, notes dated 2024-03-09.
  - "Full Throttle TAS in 8:14 with broken audio" (2023-04-07), with no tooling notes.

  None of these is on TASVideos. A later verification pass could not reproduce the Day of the Tentacle and Full Throttle hits through YouTube or Twitch search, though it found the MI2 one `[YT-SEARCH, C]`.
- **Other videos labelled "TAS"** `[YT-TAS, C]`:
  - ATK42 Games' Curse of Monkey Island series (January and February 2023) are stream recordings. AutoHotkey scripts send timed mouse and key input into ScummVM, with waits in wall-clock milliseconds; libTAS was not used. In a follow-up stream, titled "We did not Escape the Hold.", the author says "it's just an ahk and scum interaction" and that the scripted playback is saveless. These quotes come from YouTube's automatic captions.
  - LeoLitz's "Monkey Island 2 Lite Tas Explained" (2020-12-30) names no tool, but the narration describes frame-by-frame input with RNG manipulation, on "the emulator that I had to use [which] is not the emulator that we use in runs", recorded in two parts and edited together.
  - NESs7or9's "Maniac Mansion any% proof of concept TAS" (2023-02-05) does not state a platform. Inference from his 62 speedrun.com runs, all of the NES version: it is an NES TAS.

  Inference: none of these is tick-locked or works at engine level. The AutoHotkey approach is wall-clock input into ScummVM, not a precedent for sentence injection.

**Tools.**

- **BizHawk's DOSBox-X core.** BizHawk 2.11 (published 2025-09-20) added "DOSBox-X for DOS and early Windows", with savestates, rewind and TAStudio. By 2026-10-05, 37 DOS TASes made with it had been published on TASVideos, including mouse-driven games `[BIZHAWK, C]`. It is not the first DOS rerecording route: JPC-RR has 61 DOS publications since 2010, and libTAS with PCem has 11 since 2021 `[BIZHAWK, C]`. Fixes that a Monkey Island CD run would need, including CD music during gameplay, data-plus-audio discs and absolute-mouse calibration, came in 2.11.1 (2026-05-01) `[BIZHAWK, C]`. No LucasArts SCUMM game has been published on any DOS emulator `[BIZHAWK, C]`. Inference: this route runs the original interpreter, so it avoids ScummVM-only behaviour, but its timing will not map onto scummrun's ScummVM tick counts.
- **ScummVM's Event Recorder** `[EVREC, C]`:
  - *Purpose.* The ScummVM wiki calls it "a tool for regression testing of game engines". It records and plays back gameplay. The `--record-mode` option dates from a 2007 patch, Danil Ishkov rewrote the recorder for GSoC 2012, and the rewrite was merged in July 2013.
  - *Build.* It is off by default: `configure` sets `_eventrec=no`, and `--enable-eventrecorder` turns it on only when the backend is `sdl` or `emscripten`. SDL-derived ports with their own backend name, such as Switch, cannot enable it.
  - *What it records.* The values returned by `getMillis()`, date and time queries, input events and (from format version 2) empty input polls, screen-update markers with periodic screenshot MD5s, the game files' MD5s, save files, and one seed per named `Common::RandomSource`. During recording and playback it swaps the audio mixer for a null mixer.
  - *Determinism.* Replay is not guaranteed to be deterministic. On divergence the code skips events up to the next screen update. The wiki's known issues include "Events don't match 100%" and an MI1 CD record/playback desync caused by audio-callback timing. PR #7955 (merged 2026-09-23) fixed "a few causes of Event Recorder playback divergence", and no source found says the MI1 issue is fixed. Engines can also read the real clock without recording it (`getMillis(true)`), as Freescape does for key repeat.
  - *2026 activity.* PR #7119 (January) added a command-line fast-playback mode and a regression-test runner. PR #7555 (June) added an ImGui toolbar. PR #7870 (August) fixed builds without ImGui. PR #7955 (September) is the determinism work above.
  - *Use.* No TASVideos movie uses it. The closest outside reuse is katiahayati/lucasartsifier (August 2026), which patched it into a scripted-input driver with a controllable clock for automated playtesting of SCI games `[LUCASARTSIFIER, C]`.

  Inference: deterministic replay in ScummVM is not new, but it is not guaranteed either. scummrun does not depend on it: it keeps its own tick clock and injects sentences at engine level, and it varies seeds on purpose `[L-README]`.
- **A Gymnasium environment.** ashnasbot/t7g-ml (2025) wrapped the Microscope minigame of *The 7th Guest*, running in ScummVM, as a `gymnasium.Env`. It read the board from screenshots, clicked with win32 mouse events and trained MaskablePPO. In March 2026 the gym and PPO path was removed in favour of a C simulator `[T7G, C]`. No general ScummVM reinforcement-learning environment was found `[T7G, C]` `[NEG-SEARCH, U]`.
- **ICARUS** (Pfau, Smeddinck and Malaka, CHI PLAY 2017) completes Visionaire-engine adventure games autonomously with reinforcement learning and heuristics, "in roughly the same amount of time that a professional game tester requires for a speedrun" `[ICARUS, C]`. It aims at test automation, not time-optimal routes, and makes no optimality claim.
- **Other automation of ScummVM or SCUMM games** `[OTHER-AUTO, U]`:
  - Rubyboat1207/pajama-neuro (2026, work in progress): a replacement `SDL2.dll` that lets the AI streamer Neuro-sama play Pajama Sam in ScummVM.
  - fabianostermann/scummvm-WoodtickRL (2023): a patched ScummVM that walks Guybrush around Woodtick (MI2) at random to generate iMUSE music data for a paper.
  - roccozanni/grogvm (June 2026): a TypeScript SCUMM v5 reimplementation for MI1 CD, tested by a full-game walkthrough.
- **JaffarPlus** (SergioMartin86) is a search-based TAS bot that runs reward-guided breadth-first search over emulator or engine states. Its cores include NES, SNES, Genesis, Doom, SDLPoP and RAWGL (Another World). It has no ScummVM or DOSBox core `[JAFFAR, U]`.
- **dosbox-automation**, a DOSBox fork announced in 2026, offers a REST API, Lua scripting, input record and replay, and an MCP integration. It mentions no SCUMM games, and a TASVideos thread was dismissive of it `[DOSBOX-AUTO, U]`.

## 3. Related research

### 3.1 Formal solving and analysis of adventure games

- **Pickett, Verbrugge and Martineau, "(P)NFG: A Language and Runtime System for Structured Computer Narratives"** (GameOn'NA 2005, pp. 23 to 32) `[P-PICKETT, C]`.
  - It presents the PNFG language, a compiler and an interpreter for the Narrative Flow Graph, "a structured form of Petri Net" (a class of 1-safe Petri nets from Verbrugge's 2002 work). The interpreter builds a model for the NuSMV model checker.
  - Its verification covers only `query win` and `query lose`. They check whether a winning or losing state is reachable and, if so, present "a minimal winning solution", minimal in steps. Pointlessness (the gap between becoming unwinnable and losing) is left to future work.
  - The case studies are Cloak of Darkness, the first two chapters of Return to Zork (a graphical adventure, hand-translated: "we have translated narratives by hand") and Scott Adams' The Count. Model checking succeeded only on Cloak of Darkness and on cut-down "tiny" variants. The full Count is "much too large to analyse formally".
- **Verbrugge and Zhang, "Dataflow Analysis of Computer Game Narratives"** (CDP08 workshop talk, 30 October 2008), with a paper version, "Analyzing Computer Game Narratives" (ICEC 2010, LNCS 6243, pp. 224 to 231), and Zhang's 2009 MSc thesis `[P-VERBRUGGE, C]`.
  - Narratives written in PNFG are checked for winnability by backtracking depth-first search, with bounded depth, cycle and error-state detection and a search cache. A game interpreter decides reachability.
  - Ahead-of-time dataflow analysis computes conservative pre- and post-conditions for each action, a "winning set" of actions that can lead to a win, and "useless objects". These prune the branching factor; they are not a planning model.
  - The benchmarks are PNFG ports of the first two Return to Zork chapters ("we have modeled the first two chapters in PNFG"), plus student narratives. The search reports the "first winning path", which with larger depth bounds can contain redundant moves. The talk claims "orders of magnitude improvement" over the earlier NuSMV analysis, which gave shortest paths but did not scale past about six steps.
  - The workshop program credits the talk correctly. The per-talk abstract page has no byline, only the page maintainer's footer, so an earlier note that it "misattributes" the talk was wrong.
- **Lester, "Solving Interactive Fiction Games via Partial Evaluation and Bounded Model Checking"** (arXiv 2012.15365, December 2020) `[P-LESTER, C]`.
  - Background: a work-in-progress version, under a different title, was presented at WPTE 2020.
  - What it does: it runs CBMC on ScottFree, an open-source C reimplementation of the Scott Adams interpreter, together with each game's original database. LLVM optimisation and semi-manual loop unrolling stand in for partial evaluation, because the dedicated partial evaluator crashed.
  - What it solved: three tutorial games from 2006 and a cleaned-up Adventureland Sampler (placeholder rooms, items and messages deleted), in 23 moves, using 5h32m and about 15 GB.
  - What it did not solve: "Neither ScAmPER nor ScAmPI could solve the original Adventureland Sampler." It judges full SAGA games unlikely to be solvable without further advances.
  - Its claim is hedged: "To the best of our knowledge, this is the first example of a commercially released game being solved by application of a program model-checker to the game's code."
  - Its goal is reachability within a fixed move bound. It claims no shortest or minimal solution. It contrasts its use of the engine's own code with the hand translation into PNFG used by Pickett et al.
- **Érdi, "Solving text adventure games via symbolic execution"** (blog, 1 August 2020; Berlin FP talk, 13 October 2020; IFL 2020 abstract) `[P-ERDI, C]`. He wrote his own Haskell interpreter (ScottCheck) and ran it symbolically with SBV and Z3, inspired by Lester's WPTE talk. He solved only ScottKit tutorial 4 (14 commands, found in 3:35). The search deepens one step at a time. Inference from the code: 14 is the minimum number of commands, not the minimum play time.
- **Moreno-Ger, Fuentes-Fernández, Sierra-Rodríguez and Fernández-Manjón, "Model-checking for adventure videogames"** (Information and Software Technology 51(3), 2009). It translates `<e-Adventure>` game scripts into state models and checks temporal properties such as dead ends `[P-MORENO, U]`.
- **Ludocore** (Smith, Nelson and Mateas, IEEE CIG 2010). A logical game engine built on the event calculus and answer-set programming, which generates gameplay traces `[P-LUDO, U]`.
- **Natkin and Vega** (GAME-ON 2003, London) `[P-NATKIN, C]`. The paper appears under two titles: the authors' own "A Petri Net Model for the Analysis of The Ordering of Actions in Computer Games" and DBLP's "Petri Net Modelling for the Analysis of the Ordering of Actions in Computer Games". Its abstract describes "a semi formal approach based on a Petri Net specification", illustrated on Myst. A related paper by the same authors, "A Petri Net Model for Computer Games Analysis", appeared in IJIGS 3(1):37 to 44, 2004. Calling it the journal version is an inference; no source says so. **Natkin, Vega and Grünvogel** (CGAIDE 2004, pp. 109 to 113) continue the work `[P-NATKIN, C]`.
- **Yernaux, Verjans and Vanhoof, "Formalizing Escape Game Mechanics"** (Springer LNCS, 2026). A three-layer graph framework (puzzles and rooms, session state, and a forest of all execution traces) for "automated analysis of game solvability, balance, and puzzle dependencies", with a tool called GraphEG `[P-YERNAUX, U]`.

None of these optimises for time. The closest to a "shortest" result is the NuSMV minimal winning trace of Pickett et al., which counts steps and did not scale `[P-PICKETT, C]`.

### 3.2 Planning and PDDL in games

- **Bartheye and Jacopin,** "A Real-Time PDDL-based Planning Component for Video Games" (AIIDE 2009) `[P-BARTHEYE, U]`.
- **Balyo, Youngblood, Dvořák, Chrpa and Barták** (arXiv 2402.12393, 2024). They learn a PDDL model from game logs and use a planner to generate regression tests that are replayed in the game. Their test game is a Unity RPG `[P-BALYO, C]`. They write that classical-planning testing suits "games with many discreet causal interactions such as role playing games (RPG), point-and-click adventures, strategy games, visual novels or puzzle games" (spelling as in the source), and propose that "one can prove that undesired shortcuts are impossible through regular gameplay using optimal planners" if the model is detailed enough. This is proposed, not demonstrated on an adventure game `[P-BALYO, U]`.
- **Balyo et al., "Using Planning for Automated Testing of Video Games"** (IJCAI 2025 demo track). Test engineers "define the game's rules using the Planning Domain Definition Language (PDDL)", and the system integrates with the Unity and Unreal editors `[P-BALYO25, U]`.
- **Lipovetzky, Ramirez and Geffner** (IJCAI 2015). They run width-based classical planning online against an Atari emulator, because "there is no compact PDDL-model of the games" `[P-LIPO, U]`. Inference: this is the opposite design choice to scummrun, which uses an explicit model with costs measured offline.
- **Fast Downward inside text-game environments.** The ALFWorld paper (ICLR 2021) says its TextWorld engine "uses Fast Downward ... to maintain and update the current state of the game", with rules defined in PDDL. The original TextWorld paper (2018) generates quests with linear-logic rules instead `[P-ALFWORLD, U]`. Inference: Fast Downward has been used in game environments for state tracking, not for minimising route time.
- **Planners on single commercial puzzle games:**
  - Snowman (arXiv 2310.01471): SAT beats PDDL planners `[P-PUZZLE, U]`.
  - Puzznic (arXiv 2310.01503) `[P-PUZZLE, U]`.
  - PDDL generated from VGDL descriptions (arXiv 2109.00449) `[P-PUZZLE, U]`.
- **Numeric PDDL for a text RPG** (Gobbi's Goblins, SBGames 2025) `[P-GOBBI, U]`.
- **Planners as playability testers** for generated mechanics (Zook and Riedl, AAAI 2014) `[P-ZOOK, U]`.
- **A hobby project,** VictorLaugt/pddl-planning-for-adventure-game, uses a PDDL planner to check that a scripted outdoor treasure hunt with item pickup and item-gated movement can be finished `[P-VICTOR, U]`. Inference: using PDDL to check an item-and-location puzzle for solvability is a known idea, not a contribution.

### 3.3 LLMs that write PDDL or game models

- Guan, Valmeekam, Sreedharan and Kambhampati (NeurIPS 2023): GPT-4 drafts PDDL, which is then corrected with feedback `[P-GUAN, U]`.
- Oswald et al., "Large Language Models as Planning Domain Generators" (ICAPS 2024): they report "a moderate level of proficiency" `[P-OSWALD, U]`.
- PDDLEGO (*SEM 2024): LLMs build PDDL problem files step by step in partially observed text games and solve them with a symbolic planner `[P-PDDLEGO, U]`.
- STORY2GAME (arXiv 2505.03547, 2025) generates interactive-fiction games, using LLM-generated action preconditions and effects to decide what state the engine must track `[P-STORY2GAME, U]`.

Inference: these are prior art for scummrun's blind extractor agents (README, Phase 7) `[L-README]`. They work from natural language, not from decompiled game bytecode.

### 3.4 LLM and GUI agents, and game environments

- **FlashAdventure** (EMNLP 2025) covers 34 Flash adventure games played from screenshots. The best agent reaches 5.88% success, against 97.06% for humans. It includes no SCUMM games `[P-FLASH, U]`.
- **VideoGameBench** (arXiv 2505.18134) covers ten 1990s games. Frontier models "complete only 0.48% of VideoGameBench" `[P-VGB, U]`.
- **Orak** (arXiv 2506.03610) has 12 games, and its two adventure games are Ace Attorney and Her Story `[P-ORAK, U]`.
- **BALROG** (ICLR 2025) reports that several models do worse when given visual observations. **EscapeBench** (ACL 2025) reports about 15% average progress on room-escape games without hints. **AI GameStore** (arXiv 2602.17594, 2026) reports top vision-language models below 10% of the human average on 100 commercial games `[P-AGENTS, U]`.
- **TextQuests** (Phan, Mazeika, Zou and Hendrycks; arXiv 2507.23701) uses 25 Infocom games with a 500-step cap and no external tools `[P-TQ, C]`. In the August 2025 paper no model finished any game without clues, and with clues GPT-5 finished 5 of 25. That is now out of date: the official leaderboard data lists No-Clues completions for GPT-6-astra (5 of 25), Claude Fable 5 (2 of 25) and GPT-5.6-sol (1 of 25) `[P-TQ, C]`.
- **Jericho** (AAAI 2020) is the standard reinforcement-learning environment for Z-machine games. It measures solutions in steps: "Solution Length is the number of steps needed for the walkthrough" `[P-JERICHO, U]`.
- **TextWorld** (2018) generates text games `[P-TEXTWORLD, U]`.
- **Tsai et al.** (arXiv 2304.02868) find that ChatGPT cannot build a world model through play `[P-TSAI, U]`.
- **Go-Explore** (arXiv 1901.10995) remembers visited states, returns to promising ones and explores from them, exploiting simulators that can restore state `[P-GOEXPLORE, U]`. Inference: it is the main model-free alternative for a ScummVM system with save states, and it carries no optimality guarantee.

Inference: end-to-end agents are far from finishing graphic adventures, let alone finishing them quickly, and text benchmarks score steps or progress, never play time. Agents are better used to help build a symbolic model than to replace one.

### 3.5 Puzzle dependency charts and puzzle generation

- **Gilbert (2014).** In "Puzzle Dependency Charts", Ron Gilbert writes that he and Gary Winnick "didn't have Puzzle Dependency Charts for Maniac Mansion, and in a lot of ways it really shows". He adds that the concept came "probably right before or during the development of The Last Crusade adventure game", that "both David Fox and Noah Falstein contributed heavy", and that "They reached their full potential during Monkey Island where I relied on them for every aspect of the puzzle design." He contrasts the two tools: "Flowcharts are great if you're trying to solve a game, dependency charts are great if you're trying to design a game." `[P-GILBERT, D]`.
- **Falstein's GDC 2013 talk** is reported to say the charts were invented for Maniac Mansion, which conflicts with Gilbert's account `[P-FALSTEIN, U]`.
- **Weinberg, "Puzzle Dependency Graph Primer"** (a community blog post on Game Developer, 2 May 2016, excerpted from a GDC 2016 talk) `[P-WEINBERG, C]`. It calls the graphs "an analytical technique which can be used to gain insights into the size, structure and complexity of a game" and says "Ron Gilbert is credited with the first use". It notes that "In the game The Day of the Tentacle there is a layer with 13 puzzles which results in over 6.2 billion possible orders" (13! = 6,227,020,800), and uses this to argue that branching complicates dialogue writing. It builds GANTT charts and dependency structure matrices, and a time histogram from 1,000 random play-throughs.
  - Inference, replacing an earlier one: 13! is the number of orders a player could take, not the space a planner searches. A* with duplicate detection merges every ordering that reaches the same state, so 13 independent puzzles give about 2^13 subsets times the location and inventory state. For scale, the unit-cost Part I search logged in `out/plans/part1.log` expanded 16,544 states to find a 66-step plan `[L-PLANLOG, D]`.
- **Manual analyses.** Bäumer (2015, MI2) analyses puzzle dependencies by hand `[P-BAUMER, U]`.
- **Tools.** A GitHub search for "puzzle dependency chart" returns 8 repositories; none derives a chart from game scripts. Most are manual editors, one renders hand-written text with Graphviz, and one exports a chart to Adventure Creator code `[P-PDTOOLS, C]`. Two projects do derive dependency structure statically from scripts in their own engine languages `[P-PDTOOLS, C]`:
  - rocketsurgery-games/gnusto's `frotz requires --victory` prints a "static dependency tree for a goal" from Grue game scripts, including Grue ports of Infocom games that were translated from ZIL with help from a partial converter;
  - ChicagoDave/sharpee's `deriveReach` runs a static pass over compiled story IR and records what it calls "the dependency graph of progress". It covers room gating only.

  No tool was found that derives a dependency chart from a shipped commercial game's bytecode. The search was limited to GitHub `[P-PDTOOLS, C]`.
- **Puzzle generation** builds puzzles from known solution chains, so it builds solvability in. It is not the opposite of solving `[P-PCG, C]`:
  - Dart and Nelson, "Smart terrain causality chains for adventure-game puzzle generation" (IEEE CIG 2012, pp. 328 to 334): "a puzzle known to be solvable can be generated by simply inserting the items contained in a causality chain into the environment", demonstrated in the game Space Dust.
  - Fernández-Vara and Thomson, the Puzzle-Dice system (PCG workshop 2012, pp. 1 to 6).
  - De Kegel and Haahr (ICIDS 2019, pp. 241 to 249) use one grammar for both generation and solving: "Given a valid grammar, the system guarantees that its puzzles are solvable."
  - De Kegel and Haahr's survey, "Procedural Puzzle Generation: A Survey", IEEE Transactions on Games 12(1):21 to 40 (print March 2020, online May 2019).
  - CONAN (arXiv 1808.06217) `[P-PCG, U]`.

### 3.6 Speedrun routing

- **Groß, Zühlke and Naujoks, "Automating Speedrun Routing: Overview and Vision"** (EvoApplications 2022; arXiv 2106.01182) `[P-GROSS, U]`. Using Ocarina of Time, they model routing as a weighted graph of game events and list challenges including dynamic weights and repeatable events. They write: "When modeling an Event Graph with traversal times as edge weights, conventional pathfinding algorithms like Dijkstra's or A* are unsuitable given the edge dynamics", and "For edge weights to become static, a State Graph with extensively detailed states is needed, vastly increasing the amount of nodes." They favour evolutionary algorithms, and note that glitchless routing "can easier be defined formally". Groß's 2022 thesis builds a partial Ocarina of Time graph of 6,764 nodes and concludes that routing "is not a trivial shortest path problem" `[P-GROSS, U]`.
  - Inference: scummrun answers this objection only in part. A factored PDDL state, with Guybrush's position in the state and costs keyed by position, makes each action's cost static without enumerating a state graph, and LM-cut keeps A* tractable. But real durations still depend on more than position (`docs/optimization.md` calls the cost table a "surrogate"), which is why the route is chosen empirically on seeds `[L-OPT]`.
- **Community routing optimisers** `[P-GROSS, U]` `[P-ROUTERS, U]`. This corrects the previous revision, which said no TSP-solver use was found in speedrun communities:
  - Groß et al. report that the speedrunner JaV modelled TrackMania Nations Forever checkpoints as "a variation of the travelling salesperson problem", solved with a genetic algorithm. They call it "one of the rare examples that led to an improvement on the leaderboard times".
  - They also report Iškovs' 2018 Morrowind all-factions route, planned by simulated annealing over a quest dependency graph. The original blog URL now redirects to an unrelated site, so it is cited only through Groß et al.
  - wxarmstrong/YakuzaRouter routes the Yakuza Kiwami 2 All Substories run "as a Sequential Ordering Problem solved via Ant Colony Optimization".
  - Yoyoshix/Road_to_Ballhalla optimises an any% route from a runner's own times.

  What was not found is community use of an exact solver such as Concorde.
- **Lafond, "The complexity of speedrunning video games"** (FUN 2018). Damage boosting and routing "lead to novel generalizations of the well-known NP-hard knapsack and feedback arc set problems". Routing in Mega Man-style stage graphs is W[2]-hard in time lost, even at bounded treewidth `[P-LAFOND, U]`.
- **Gajduk (2016).** Greedy nearest-item collection is on average 7% worse than optimal `[P-GAJDUK, U]`.
- **Randomizer logic solvers** check whether a seed can be beaten, not how fast. The OoT Randomizer README says "Proper logic is used to ensure every seed is possible to complete without the use of glitches" `[P-RANDO, U]`. The Pokémon RouteOne/Two/Three tools evaluate routes that players write `[P-RANDO, U]`.
- **Murphy's learnfun/playfun** (SIGBOVIK 2013) plays NES games automatically by searching emulator states `[P-MURPHY, U]`.
- **Cook, Charity, Awiszus, Carnovalini and Dockhorn, "We Call This Controller Skip: AI for Speedrunning"** (EXAG-INT workshop at AIIDE 2025). They call speedrunning "an area largely untouched by game AI research", write that "TAS runs are not guided by AI search" and that "they are made manually", and say routing "maps naturally onto problems in computer science related to scheduling, planning, and search", with "a lot of unexplored space" `[P-COOK, U]`.

## 4. What seems new here, and what does not

### 4.1 Not new

- **Engine-level sentence injection for MI1, and verb clicks through the input script.** scummvm-mcp (public from April 2026) uses the same two engine calls as scummrun `[MCP-SRC, C]` `[L-PATCH, D]`. ScummBench (also April 2026) offers a sentence call too, but its two documents conflict on whether it queues the sentence or clicks the verb and then the object `[SCUMMBENCH, D]`.
- **Automated play of MI1 by LLM agents** `[AI-YT, C]` `[WEBMCP, C]` `[LUISHG, C]`, and autonomous completion of commercial graphic adventures `[ICARUS, C]`.
- **Tool-assisted ScummVM runs**, including SCUMM HE games on TASVideos `[TV-SVM, C]` and off-site libTAS TASes of MI2 and Day of the Tentacle `[RENTRY, U]`.
- **Record and replay in ScummVM** `[EVREC, C]`.
- **PDDL in games,** including planner-generated sequences replayed in a game for testing, and Fast Downward as a game-state engine `[P-BARTHEYE, U]` `[P-BALYO, C]` `[P-ALFWORLD, U]`.
- **Formal solving of adventure games for reachability** `[P-PICKETT, C]` `[P-VERBRUGGE, C]` `[P-LESTER, C]` `[P-ERDI, C]` `[P-MORENO, U]` `[P-NATKIN, C]`.
- **Deriving action conditions from game code.** Verbrugge and Zhang derive pre- and post-conditions from PNFG action code, and Lester model-checks a commercial game's interpreter and data directly `[P-VERBRUGGE, C]` `[P-LESTER, C]`.
- **LLMs drafting PDDL** or game-state models `[P-GUAN, U]` `[P-OSWALD, U]` `[P-PDDLEGO, U]` `[P-STORY2GAME, U]`.
- **Speedrun routing as graph optimisation,** including TSP-family formulations solved with metaheuristics `[P-GROSS, U]` `[P-ROUTERS, U]` `[P-LAFOND, U]`, and a written proposal for shortest-path routing over decompiled SCUMM script dependencies for MI1, without code `[SGG, U]`.

### 4.2 Plausibly new (inference, based on the negative searches)

The searches found no prior instance of the following combination. That is evidence, not proof `[NEG-SEARCH, U]`. Each part has precedents; the combination is what was not found.

1. Cost-optimal classical planning (A* with LM-cut) over action costs measured in engine ticks and keyed by position, used to produce a speedrun route for a commercial graphic adventure, with the plan replayed in the real engine `[L-README]`. Planners have been used in games (section 3.2) and routing has been studied (section 3.6), but no work was found that plans for measured play time.
2. A PDDL model of a commercial game written from its decompiled SCUMM bytecode, with a script citation for each action `[L-README]`. Two limits apply:
   - The shipped route uses the hand-written model. LLM extraction was demonstrated once, on this one segment: six blind extractor agents produced a cited model that reached the hand-written model's optimum after three defects were fixed, and found a forest exit (path 686) the hand-written model lacked `[L-README]`.
   - Deriving a model from game code is not new in itself (Verbrugge and Zhang, Lester). What may be new is doing it from SCUMM bytecode into a cited PDDL model that feeds a cost-optimal planner.
3. A written "glitchless" rule set for an automated player, defined through click equivalence with the game's own input scripts `[L-RULES]`. The MI1 community has no glitchless definition to match (section 1.3), so this is scummrun's own standard.

Choosing the route by racing candidates on seeds 1 to 30 and reporting on held-out seeds 31 to 60 is sound practice `[L-README]`. Inference: it is standard held-out evaluation applied to route selection, so it should not be claimed as a contribution. Cook et al.'s observation that routing as planning is largely unexplored supports a modest framing, not a claim of priority `[P-COOK, U]`.

### 4.3 The limits of "provably shortest"

Inference: A* with an admissible heuristic proves optimality only relative to two things:

- **The PDDL model.** Any action, skip or sequence break it omits limits the proof. The extractor already found one omission, path 686 `[L-README]`.
- **The mean measured cost table,** which varies by seed and by context beyond position `[L-OPT]`.

This matches Balyo et al.'s condition that optimal planners prove properties only if the model is detailed enough `[P-BALYO, U]`, and Groß et al.'s warning that times depend on traversal history `[P-GROSS, U]`. The final route is chosen by an empirical race `[L-README]`. An accurate description is "optimal under the PDDL model and the mean measured costs, validated on held-out seeds". It is not a proof about the game itself. Lafond (2018) gives general hardness background `[P-LAFOND, U]`. Human runners are faster on this segment under looser rules (section 1.4) `[L-CMP]`.

## 5. Feasibility of "all or most ScummVM games"

### 5.1 Scale

- **Releases.** The latest release is v2026.3.0, published 2026-06-20 `[SVM-REL, D]`. It was the third release of 2026, after v2026.1.0 (2026-01-31) and v2026.2.0 (2026-03-28). The 2026.1.0 news post announced "12 new engines" with "at least 194 titles", and 2026.2.0 added the American Laser Games light-gun FMV titles `[SVM-REL, U]`.
- **Engines.** At tag v2026.3.0 the source tree has 122 engine directories, and 105 engines are enabled in default builds. Two research passes counted this independently and agree `[SVM-ENGINES, U]`. On master at commit `8e5d689` (2026-10-05) there are 126 directories, which is 124 game engines plus two test engines, and 111 are enabled by default. The difference comes from four new engines (eem, fool, harvester, macs2) and from chamber and macventure being switched on `[SVM-ENGINES, U]`.
- **Compatibility list.** The page lists 679 entries across 111 engine ids `[SVM-COMPAT, C]`. By support level: 210 Excellent, 419 Good, 10 Bugged, 20 Broken, 20 Untested. The largest engines by entries are Wintermute (163), SCUMM (74: 18 LucasArts entries and 56 Humongous), SCI (61), AGS (39) and Director (27) `[SVM-COMPAT, U]`.
- **Detection tables.** These hold about 12,592 game ids at v2026.3.0 `[SVM-DET, C]`. This is a slight undercount, because three engines parsed as zero. Three engines account for about 93% of the ids:

| Engine | Ids | Breakdown |
|---|---:|---|
| Glk (interactive fiction) | about 6,000 | zcode 3,146, glulx 1,057, adrift 996, tads 475 |
| AGS | 3,811 at the tag; 3,825 on master | at the tag: 3,320 in a section labelled "Free post-2.5 games that are likely supported by the AGS engine", 184 commercial, 7 remakes, 292 in sections labelled not supported (234 pre-2.5, 41 AGS 3.6.2/3, 17 AGS 4.0), 6 RuCOMM and 2 placeholders. Master adds 14 ids and removes none. |
| Director | 1,856 | |

Sources: `[SVM-DET, C]` `[SVM-AGS-DET, C]`. The AGS section labels describe the id table only. The "using unsupported ... plugin" blocks sit in the separate detection-entry table and cover about 40 entries, some for ids that the id table files under "likely supported" `[SVM-AGS-DET, C]`.

Inference: "every ScummVM game" means mostly text interactive fiction, freeware AGS games and Director multimedia titles. Few of them have leaderboards, and Glk text games fit a parser agent (as in Jericho) better than a click bridge `[SVM-DET, C]` `[P-JERICHO, U]`.

### 5.2 Leaderboards

- **Genre data.** speedrun.com's `/genres` endpoint returns an empty list, and game objects carry empty genre arrays, so the site cannot count boards by genre `[SRC-GENRES, C]`. All counts below come from matching titles against the site's bulk catalogue of 52,035 games `[SRC-BULK, C]`.
- **First pass, compatibility-list scope, all genres.** 305 compatibility entries map to 300 distinct boards. By verified runs, 4 boards have none, 146 have 1 to 9, 144 have 10 to 199 and 6 have 200 or more `[SRC-BULK, C]`.
  - Three more matches are probable, which would give 303 boards.
  - Several matched boards cover ports ScummVM does not run. One example is the NES-only Déjà Vu board.
  - The mapping missed boards that ScummVM does support, such as Maniac Mansion (NES) with 219 runs `[SRC-ADV, C]`.
- **First pass, adventure games only.** About 243 boards, 239 of them with runs `[SRC-ADV, C]`. By engine:

| Engine | Boards |
|---|---:|
| SCI | 46 |
| SCUMM | 34 (13 LucasArts pages and 21 Humongous) |
| AGS | 25 |
| Nancy | 16 |
| AGI | 14 |
| Gob | 9 |
| Wintermute | 7 |
| AGOS | 5 |
| Groovie | 4 |

  About 56 further engines have 1 to 3 titles each. About 25 of the boards are Sierra parser or keyboard-walking games `[SRC-ADV, C]`.
- **Second pass, point-and-click only.** A separate pass matched the 679 titles by exact normalised name, removed false positives and added hand-curated SCUMM, SCI and AGI boards. It found at least 283 boards for compatibility-list titles, and estimated about 200 point-and-click boards (range 195 to 215) `[SRC-BULK2, U]`. It excluded parser games (12 SCI0, 15 AGI), RPGs, action games, light-gun games, Maniac Mansion NES and spin-off categories. Its split is 12 SCUMM LucasArts boards, 20 HE story adventures, 32 SCI icon-bar games and about 140 others (AGS 25, Nancy 16, Gob 8, Wintermute 7, AGOS 5, Groovie 4, and many single-game engines). The two passes use different scopes, so their figures should not be added together or averaged.
- **Detection-table scope.** About 400 boards with at least one run map to ScummVM titles, and up to about 455 if uncertain matches count `[SRC-ADV, C]`. Of these, about 217 have a run dated in the last 24 months and 133 in the last 12. Some freeware AGS remakes have active boards, for example QfG2 AGDI with 69 runs and KQ1 AGDI with 37 `[SVM-DET, C]`.
- **Most-run boards (all-time verified runs):**

| Board | Verified runs |
|---|---:|
| Putt-Putt Saves the Zoo | 496 |
| Pajama Sam 1 | 390 |
| Monkey Island 2 | 379 |
| King's Quest VI | 326 |
| Myst | 314 |
| The Secret of Monkey Island | 230 |
| Maniac Mansion (NES) | 219 |
| Quest for Glory I EGA | 195 |

  Sources: `[SRC-ACTIVE, C]` `[SRC-SUM, D]`.
- **Activity.** The summary endpoint's "recent runs" counter, whose window is not documented, is zero for most boards. Its highest values are Nancy Drew: Stay Tuned for Danger with 11 and King's Quest IV SCI with 10 `[SRC-ACTIVE, C]`. The second pass counted verified runs per board, capped at 200 per board, and dates `[SRC-BULK2, U]`:
  - all 13 SCUMM LucasArts boards (including Maniac Mansion NES) have a run dated in the last 12 months, with at least 1,155 runs between them;
  - 15 of the 20 HE story boards were active in the last 12 months, with at least 1,742 runs;
  - 10 of the 32 SCI icon-bar boards, 7 of the 12 SCI0 boards and 5 of the 15 AGI boards were active in the last 12 months.

### 5.3 What carries over inside SCUMM

Corrected SCUMM versions at v2026.3.0 `[SCUMM-DET, C]`:

| Version | Games |
|---|---|
| v0 to v2 | Maniac Mansion (including NES), Zak McKracken |
| v3 | Indy3, Loom (EGA, Mac, PC-Engine, FM-Towns), Zak FM-Towns, FM-Towns compilations |
| v4 | MI1 VGA floppy and EGA, Loom VGA/CD talkie, Passport to Adventure |
| v5 | MI1 CD, Mac and SE; Monkey Island 2; Fate of Atlantis |
| v6 | Day of the Tentacle, Sam & Max, and all 56 Humongous (HE) game ids |
| v7 | Full Throttle, The Dig, Rebel Assault I and II |
| v8 | The Curse of Monkey Island |

What this means for the bridge:

- **v4 and v3 build on v5.** `ScummEngine_v4` is a subclass of `ScummEngine_v5`, and v3 derives from v4 `[SCUMM-V4, C]`. Inference: a v5 bridge probably covers the v4 titles with little change. `docs/next.md` says "other SCUMM v5 games first" `[L-NEXT]`. Only MI2 and Fate of Atlantis are other v5 game ids, but the v4 and v3 titles are close by.
- **The sentence queue does not always work.** scummvm-mcp's source comments say that "Indy4 single-target actor sentences (e.g. talk_to sophia) dispatched via doSentence() do not reliably fire the talk action". The Dig needs simulated clicks for walk-to handlers, and Sam & Max needs cursor cycling for some verbs `[MCP-SRC, C]`. Indy4 is Fate of Atlantis, a v5 game. Inference: scummrun's sentence-only rule will need per-game exceptions there, or a coordinate-free way to reach the input script.
- **HE games.** There are 56 HE ids. About 21 are story adventures (Putt-Putt, Freddi Fish, Pajama Sam, Spy Fox and others); the rest are arcade, activity, sports or educational titles `[DESCUMM, C]`. In three HE60 to HE62 demos the input script calls the sentence script directly with `start-script`, not `do-sentence` `[DESCUMM, C]`. Inference: a pushed sentence reaches the same handler by another code path, so tick counts may differ from a real click.
- **INSANE.** Full Throttle's INSANE scenes cover the mine-road driving segments as well as the bike fights. Rebel Assault I and II run on INSANE subclasses and are flagged as testing in the detection tables `[RT-CODE, C]`. Rebel Assault has no SCUMM bytecode `[DESCUMM, C]`.
- **Script dumps.** descumm decodes SCUMM v0 to v8 and HE, with version flags from `-0` (C64) to `-8` and `-g<NNN>` for HE `[DESCUMM, C]` `[DESCUMM, U]`. The extractors leave gaps: ScummTR excludes Maniac Mansion NES and Loom TG16, and NUTCracker covers only v5 to v8 and HE `[DESCUMM, C]`.
- **Settings change game behaviour.** SCUMM's `copy_protection` option defaults to off, and with it off the engine sets variables or redirects scripts so that the copy-protection checks in Loom DOS, MI1 VGA, Day of the Tentacle, Indy4 and Mac MI2 are skipped `[SVM-CP2, U]`. The `enhancements` option defaults to game-breaking bug fixes plus enhancement group 1, and `script_v5.cpp` alone has 60 `enhancementEnabled()` checks `[SCUMM-ENH, U]`. Inference: a route is tied to a ScummVM version and its settings, so a per-game profile has to record the ScummVM version, the enhancement flags and the game release. scummrun already pins its engine settings `[L-RULES]`.
- **Per-game work.** scummvm-mcp now has dedicated bridge classes for 12 SCUMM game ids, and per-game commits continued through August 2026 `[MCP-SRC, C]`. Inference: per-game bridge work is real even inside one engine.

### 5.4 Other engines

**Script access.**

- **SCI.** ScummVM's SCI console registers 158 command names, including `send`, `said`, `parse`, `disasm`, `gameflags_set` and several breakpoint types; many are aliases, for about 113 distinct handlers `[SCI-CON, C]`. sluicebox/sci-scripts holds decompiled scripts, described as "100% decompiled", for every SCI game from King's Quest IV (1988) to Leisure Suit Larry 7 (1996), in more than 300 versions across platforms `[SCI-SCRIPTS, U]`. SCI0 games use a text parser, and SCI1 introduced point-and-click `[SCI-WIKI, U]`.
- **AGI.** The main AGI console has 23 commands, including `flags`, `vars`, `setvar` and `setflag` `[SCI-CON, C]`. WinAGI decompiles LOGIC resources `[AGI-TOOLS, U]`.
- **AGS** `[AGS-TOOLS, C]`:
  - ScummVM's AGS console registers five AGS-specific commands on top of the generic debugger commands. `ags_set_script_dump` only enables a per-instruction runtime trace, and that code is compiled in only when `DEBUG_CC_EXEC` is set, which it is not by default.
  - rofl0r/agsutils extracts and disassembles every compiled script, and can reassemble, reinject, simulate and optimise them. Its README says "all versions >= 2.5.0 and < 4.0.0 are 100% supported". It has no decompiler.
  - Decompilation exists outside agsutils, in immature form. adm244/Ghidra-ReAGS is a Ghidra extension with an AGS VM processor specification; its README says it "allows you to decompile and reverse-engineer AGS scripts", but also that it is "in halfway finished state", "no longer being developed", and "Sometimes decompilation is incorrect". It was ported to Ghidra 12.1.2 in June 2026. bequidox74/ags-decomp (created September 2026) is an early Python decompiler that has not been checked.
  - Inference: AGS scripts can be read in structured form, but the output needs checking against the disassembly.
- **Other decompilers and debuggers.** Director's Lingo decompiler (ProjectorRays) was merged into ScummVM in May 2024, and the Director debugger has breakpoints and a REPL. Wintermute has a decompiler in scummvm-tools and a ScummVM debugger with step, watch and break. scummvm-tools also has dekyra, degob, decine and desword2 `[TOOLS, U]` `[LINGODEC, U]`.
- **AGOS and Tinsel.** scummvm-tools has only compression and extraction tools for them `[AGOS-TOOLS, C]`. Disassemblers exist elsewhere `[AGOS-TOOLS, C]`:
  - ScummVM's own AGOS engine contains a disassembler, with opcode name tables for Elvira 1 and 2, Waxworks, Simon 1 and 2, The Feeble Files and the Puzzle Pack. Alt+U dumps all game-logic subroutines, and the `opcode` and `subroutine` debug channels trace execution.
  - adventurebrew/magos (GPL-3.0, v0.6.0 released 2025-05-19) can "Decompile and Recompile Game Scripts" for the same games. Inference from its tests: the output is an opcode-level listing that can be reassembled, not structured source.
  - peterkohaut/tinsel3viewer has a PCODE disassembler for Discworld Noir only. A generated Discworld 1 scanner in phoenix1of1/GhidraMCP has not been checked. Nothing was found for Discworld 2, and ScummVM's Tinsel engine only traces raw opcode numbers.

**Game logic written in C++.** Many engines hand-port each game's logic into C++ rather than interpreting the original bytecode. The file counts below are from master at `8e5d689` `[CPP-LOGIC, C]`. The code style varies, and in several engines it is not one class per room:

| Engine | Files | How the logic is organised |
|---|---:|---|
| MADS | 293 room files | free functions per room on file-global state (moved from per-scene classes in June and July 2026) |
| M4 | 216 room files | one class per room |
| Titanic | 198 in `game/` | one class per game object, not per room |
| Blade Runner | 112 scene and 74 AI scripts | one class per scene script, with a script-like API (`Game_Flag_Query`, `Game_Flag_Set`) |
| Chewy | 91 room files | one class per room, static methods |
| Last Express | 58 character files (19 for the demo) | methods of a single `LogicManager` class |
| Star Trek | 55 room files | methods of a single `Room` class |
| Pegasus | 48 (9 infrastructure) | one class per neighbourhood |
| NGI (Full Pipe) | 41 scene files | free functions per scene |
| Neverhood | 39 files | 21 module classes, with scene classes inside |
| Hadesch | 28 files | 24 handler classes, some for menus |
| TsAGE (Ringworld, Blue Force, Ringworld 2) | 22 scene files | scene classes |
| Gnap | 10 files | 48 scene classes |
| Mohawk | Myst 13, Riven 9 | Myst counts 7 non-gameplay pseudo-stacks; Riven has 8 stacks plus a base class |
| Buried | 10 (3 infrastructure) | scene classes |

Drascula, Teenagent and Dark Seed also hard-code their logic in C++ `[CPP-LOGIC, U]`.

What this means for extraction `[CPP-HYBRID, C]`:

- The C++ is mostly readable. Placeholder names are rare in Chewy, Hadesch, Supernova and Drascula (none found), Gnap (4 in about 32,000 lines), Dragons (224 in about 21,000) and Star Trek (305 in about 48,000).
- Some engines are hybrids. Full Pipe loads its interaction table from game data, and Queen loads its command and game-state tables from `QUEEN.JAS`. For these, the C++ alone does not give the action space, and a per-engine data dumper would be needed. None exists, unlike descumm for SCUMM.
- For C++ engines the extractor models ScummVM's reimplementation, not the original game code. Blade Runner's scene scripts, for example, have 321 references to optional restored cut content, and many comments on bugs in the original.
- Inference: C++ logic does not block extraction. The cost is a bridge per engine, a mapping from C++ state to planner atoms, and per-engine prompts and validation. Most of these engines run one or two games, so the fixed cost per board is high.

**Real-time and minigame code** `[RT-CODE, C]`:

- Within adventure engines: SCUMM's INSANE (Full Throttle, Rebel Assault), God of Thunder's boss fights, Griffon and Blade Runner combat, Lure's fights, Harvester's room combat, BBVS minigames, Dragons' five minigames, the Hodj 'n' Podj minigames in Bagel, Gnap's arcade scenes, and Nancy Drew's arcade puzzles (for example Barnacle Blast, an Arkanoid clone in Nancy 8). The Nancy engine has 70 `.cpp` files in `action/puzzle/` on master, a file count rather than a count of puzzle types `[RT-CODE, C]`. An earlier pass counted 43 puzzle types at the tag `[NANCY, U]`.
- Whole engines that are action, RPG, 3D or shooter games: Another World (awe), Freescape, Hyperspace Delivery Boy (hdb), Ultima (4, 6 and 8 are enabled; Crusader: No Remorse is rated Good), Might and Magic, Kyra's Eye of the Beholder and Lands of Lore, the Hypnotix rail shooters, the American Laser Games titles, Grim Fandango and Escape from Monkey Island, and Penumbra (hpl1). Hypno also runs Spider-Man: The Sinister Six, which has puzzle sections. Little Big Adventure 1 is real-time action-adventure; LBA2's buggy code is marked unstable `[RT-CODE, C]`.

**Action injection outside SCUMM.** ScummVM's AGS port has engine-level calls that work like SCUMM's `doSentence`: `RunObjectInteraction(obj, mood)` and `RunCharacterInteraction(cc, mood)`, with `RoomProcessClick(x, y, mood)` as a coordinate fallback `[AGS-API, U]`. Inference: many AGS games implement their own verb GUIs and `on_mouse_click` handlers, so a pushed interaction may skip code that a real click runs. This is the same kind of problem scummvm-mcp reported for Indy4, and each AGS game would need its injected actions checked against real clicks `[AGS-API, U]` `[MCP-SRC, C]`.

### 5.5 Randomness, clocks and determinism

- **The seed mechanism** `[SVM-RNG, C]`. `Common::RandomSource` takes its seed from the `random_seed` config key, which `--random-seed=SEED` sets, and otherwise from the time of day plus milliseconds. When the Event Recorder is compiled in, it still uses `random_seed` and substitutes recorded seeds only during playback. `common/forbidden.h` forbids `rand()` and `srand()` and tells engines to use `Common::RandomSource`.
- **Engines that bypass it** `[SVM-RNG, C]`. `--random-seed` does not control every engine:
  - AGS sets its seed from `getMillis()`, and so do EFH, Watchmaker and Glk's Level 9 interpreter. AGOS's Personal Nightmare uses its own generator seeded from the clock.
  - Only Grim's Lua library and ScummVM's shared Lua math library declare the `rand` exception. The shared one backs Lua `math.random` in Sword25, Tetraedge, HDB and Nuvie. Nuvie reseeds it from the engine's generator; in the others, Lua randomness follows the C library's `rand()` state.
  - Director honours `random_seed` through its own random state. SCUMM showed no reseeding or clock seeding.
  - Inference: a fixed seed does not guarantee identical runs even where it applies, because the number and order of random draws can depend on timing. Reproducibility has to be tested for each engine.
- **Clocks.** SCI throttles through `kGameIsRestarting` with a 30 ms default, and SCI2 Mac scripts "currently run faster than their PC versions" `[SCI-TIME, U]`. Inference: scummrun's SCUMM tick clock does not carry over to other engines, and SCI, AGS and Wintermute each throttle frames differently `[SCI-TIME, U]`.
- **Copy protection.** SCUMM, AGI (for example Gold Rush's quiz), Gob, AGOS, SAGA and Lure bypass copy protection by default through a `copy_protection` option `[SVM-CP2, U]`. SCI has no such option, and its source refers to live copy-protection screens in The Island of Dr. Brain and the PC-98 Police Quest 2 `[SCI-CP, U]`. Inference: the previous revision's statement that copy protection is "mostly a non-issue" holds for those engines, but SCI routes may have to answer the protection screens. The answers can be read from the scripts.
- **Script patches.** ScummVM rewrites SCI game scripts at load time: `script_patches.cpp` has 911 patch entries in 54 per-game tables, and about 89 entry descriptions mention speed, timing or "fast" `[SCI-PATCH, U]`. Inference: as with SCUMM's enhancements, SCI routes measured in ScummVM will not carry over directly to the DOS interpreter or to other ScummVM versions.
- **Random content.** Blade Runner chooses which characters are replicants at random when a game starts `[BR-INIT, U]`. MI1 has its insult swordfights `[L-NEXT]`. Full Throttle has bike fights, and Fate of Atlantis and Indy3 have fistfights `[GAME-WIKI, U]`. Fixed plans need reactive or contingent policies for all of these.

### 5.6 Main blockers

Ranked by how hard each is for a plan-then-replay system. Inference, drawing on the sections above.

1. **Real-time sequences.** Fights, shooters, minigames and arcade puzzles (section 5.4). A route plan cannot cover them; they need reactive controllers or frame-level search.
2. **Building the model.** Each game needs one. LLM help so far reaches "a moderate level of proficiency" `[P-OSWALD, U]`, Lester needed program transformations before off-the-shelf tools could handle 1980 games `[P-LESTER, C]`, and scummrun's extractor result is one game segment `[L-README]`.
3. **A bridge per engine,** and sometimes per game: state dumps, action injection at the engine's own level, idle detection, a clock, skip keys and dialogue selection `[L-NEXT]` `[MCP-SRC, C]`. ScummVM's shared layer (seeding, the Event Recorder, save streams, a millisecond play-time counter) injects only raw mouse and key input, which scummrun's rules ban `[CPP-HYBRID, C]` `[L-RULES]`.
4. **Randomness inside segments,** which seeding makes reproducible but does not remove (section 5.5).
5. **Version and settings dependence,** from script patches, enhancement options and release differences. speedrun.com also gives remakes and editions separate boards `[SRC-ADV, C]`. scummrun uses the Mac release, while MI1 runners use DOS releases `[L-CMP]`.
6. **Parser input** in AGI and SCI0 games, which needs a text-command bridge and keyboard walking.
7. **SCI copy protection** (section 5.5).
8. **Categories.** No board checked has a glitchless or bot category. Each community would have to agree to one before a bot run could appear anywhere `[SRC-SITE, D]`.

Dialogue trees and multiple solutions help a planner rather than block it, provided the bridge can choose dialogue lines without coordinates.

### 5.7 Tiered roadmap (inference)

The tiers are ordered by how well each group fits the existing bridge, how hard the model is to get, the amount of real-time or random content, and whether active boards exist.

1. **Finish MI1.** Write the per-game profile `[L-NEXT]`, add the swordfight as a reactive policy `[L-NEXT]`, and cover Parts II to IV. Measure a DOS version as well, so that results can be compared with human runs `[L-CMP]`.
2. **SCUMM v4/v5, same engine class.** Start with MI2 (379 runs) `[SRC-ACTIVE, C]`, then Loom VGA/CD and Passport (v4). Take on Fate of Atlantis after that: its actor-talk sentences, fistfights and three paths make it the hardest of the group `[MCP-SRC, C]` `[GAME-WIKI, U]`.
3. **SCUMM v3 and v6.** Indy3, Loom EGA, Day of the Tentacle and Sam & Max. The verb interfaces differ, and Sam & Max needed simulated clicks in scummvm-mcp `[MCP-SRC, C]`.
4. **HE story adventures** (about 21 ids, with some of the most-run boards). First check how a click reaches the sentence script, and whether a pushed sentence costs the same ticks `[DESCUMM, C]` `[SRC-ACTIVE, C]`.
5. **A second engine: SCI1.x point-and-click** (for example King's Quest V and VI, Space Quest IV to VI, Larry 5 and 6, Laura Bow 2, Gabriel Knight 1). This needs a new bridge, clock and copy-protection handling. The script corpus and the console help `[SCI-CON, C]` `[SCI-SCRIPTS, U]` `[SCI-CP, U]`.
6. **AGS commercial point-and-click games** (about 25 boards), through the interaction calls, with injected actions validated per game and models read through the disassembler or Ghidra-ReAGS `[SRC-ADV, C]` `[AGS-API, U]` `[AGS-TOOLS, C]`.
7. **Other SCUMM versions and parser games.** Full Throttle (INSANE bike fights and driving), The Dig (needs clicks), The Curse of Monkey Island (v8), Maniac Mansion and Zak (their own interface, including the NES port), and the AGI and SCI0 parser games `[SCUMM-DET, C]` `[MCP-SRC, C]` `[RT-CODE, C]`.
8. **Other script engines with tools:** Gob, Wintermute, Nancy Drew (without its arcade puzzles), Groovie, Kyra and AGOS `[TOOLS, U]` `[AGOS-TOOLS, C]`.
9. **Single-game C++ engines,** one project each `[CPP-LOGIC, C]`.

Out of reach with this method: the AGS, Director and Glk long tail, minigame-heavy titles, and action, RPG, 3D, light-gun and rail-shooter games. These need frame-level input search or dedicated solvers `[SVM-DET, C]` `[RT-CODE, C]`.

Rough reach, from the two board counts in section 5.2:

- First pass, adventure scope: SCUMM (34) plus SCI (46) gives 80 boards, and adding AGI (14) gives 94 `[SRC-ADV, C]`.
- Second pass, point-and-click scope: SCUMM plus SCI point-and-click covers about 65 of about 200 boards, and adding AGS and the other script engines brings it to roughly 100 to 150 `[SRC-BULK2, U]`.

Each of these boards still needs its own model and profile.

Verdict (inference): "every ScummVM game" is not a reasonable goal. Most detection ids are text interactive fiction or freeware with no leaderboard, and many engines run action or RPG games `[SVM-DET, C]`. "Most point-and-click adventures that have leaderboards" (about 200 to 240 boards, depending on scope) is a bounded goal, but it means engine-by-engine work over a long period, with model extraction and bridges as the main costs. A realistic medium-term target is the SCUMM adventures plus the SCI1.x point-and-click games.

## 6. Corrections in this revision

The previous revision of this document, or the first research passes behind this one, stated the following wrongly or incompletely. The corrected versions are in the sections named.

- **Any% restrictions (section 1.2).** The list omitted the Talkie patch requirement, the NON-ScummVM emulator disclosure, the duty to state Saves and Credit Early, and video and RTA. The ScummVM 2.8 and music-device rules apply to the Any% ScummVM subcategory only, and the keybind rule says "should".
- **Swordmaster% (section 1.4).** The main reason it is not comparable is that it ends at a different object (Carla's shirt, 596) and skips the idol and treasure trials. The start point is a small difference.
- **MI1 bot runs (section 2.1).** LLM-agent playthroughs of MI1 exist (ScummBench, scummvm-mcp videos). None is a completion or a timed run. The previous claim that scummvm-mcp did sentence injection "before scummrun" is withdrawn, because no scummrun start date was established.
- **TASVideos and ScummVM (section 2.2).** The previous list of "at least three" ScummVM publications omitted Backyard Baseball, Phantasmagoria, Riven and Door. There are seven, three of them SCUMM HE games. The one LucasArts SCUMM submission (Loom) was rejected.
- **YouTube "TAS" videos (section 2.2).** ATK42's Curse of Monkey Island videos document their tooling (AutoHotkey into ScummVM), and LeoLitz's narration describes his method.
- **Verbrugge and Zhang (section 3.1).** The workshop page does not misattribute the talk.
- **Weinberg (section 3.5).** The 6.2 billion orderings are not the space a planner searches.
- **Puzzle dependency tools (section 3.5).** Two projects derive dependency structure statically from engine-native scripts. None from shipped commercial bytecode was found.
- **TSP in speedrunning (section 3.6).** TSP-family routing with metaheuristics exists in speedrun communities. Community use of an exact solver was not found.
- **AGS (sections 5.1 and 5.4).** Decompilation exists, in unfinished form. Master has 3,825 detection ids.
- **AGOS and Tinsel (section 5.4).** Disassemblers exist outside scummvm-tools, including one inside ScummVM's AGOS engine.
- **C++ logic (section 5.4).** A research pass described these engines as using one C++ class per room. MADS, Full Pipe, Last Express and Star Trek do not.
- **Seeds and replay (sections 2.2 and 5.5).** `--random-seed` does not control every engine, and Event Recorder replay is not guaranteed to be deterministic.
- **Copy protection (section 5.5).** SCI probably does not bypass it.

## References

All entries were accessed on 2026-10-05. "Local" entries are files in this repository, read on that date. Within a row, a path that starts with `.../` uses the same base URL as the previous full URL in that row.

| ID | Source(s) | Accessed |
|---|---|---|
| SRC-GAME | https://www.speedrun.com/api/v1/games/w6j4m46j?embed=categories,variables,levels (page: https://www.speedrun.com/tsomi) | 2026-10-05 |
| SRC-GAMEDATA | https://www.speedrun.com/api/v2/GetGameData?gameUrl=tsomi; https://www.speedrun.com/api/v2/GetGameData?gameUrl=tsomise; https://www.speedrun.com/api/v2/GetGameData?gameId=w6j4m46j; https://www.speedrun.com/api/v2/GetGameData?gameId=9d3rvxgd; https://www.speedrun.com/api/v1/games/w6j4m46j/categories?embed=variables; https://www.speedrun.com/api/v1/games/9d3rvxgd/categories?embed=variables; https://www.speedrun.com/api/v1/categories/mkeq5vnk | 2026-10-05 |
| SRC-MODS | https://www.speedrun.com/api/v1/games?name=secret%20of%20monkey%20island; https://www.speedrun.com/api/v1/games/w6j4m46j?embed=platforms,moderators; https://www.speedrun.com/api/v1/games/9d3rvxgd?embed=platforms,moderators; https://www.speedrun.com/api/v1/games/w6j4m46j; https://www.speedrun.com/api/v1/games/9d3rvxgd (pages: https://www.speedrun.com/tsomi, https://www.speedrun.com/tsomise) | 2026-10-05 |
| SRC-MULTI | https://www.speedrun.com/api/v1/games/o1yj4341/categories; https://www.speedrun.com/api/v1/series/m72rkmn2/games?max=200 | 2026-10-05 |
| SRC-VAR | https://www.speedrun.com/api/v1/games/w6j4m46j/variables; https://www.speedrun.com/api/v1/variables/9l7yd9ql | 2026-10-05 |
| SRC-SUM | https://www.speedrun.com/api/v2/GetGameSummary?gameId=w6j4m46j | 2026-10-05 |
| SRC-LEVELS | https://www.speedrun.com/api/v1/games/w6j4m46j/levels | 2026-10-05 |
| SRC-LB | https://www.speedrun.com/api/v1/leaderboards/w6j4m46j/category/jdrqwqlk?top=3 (also with `var-9l7yd9ql=q8kd6p6q` and `=qyzjp9d1`); .../category/wkpmnlwk?top=3; .../category/9kv70zjk?top=3; .../category/w207w65k | 2026-10-05 |
| SRC-RUNS | https://www.speedrun.com/api/v1/runs?category=jdrqwqlk&status=verified&max=200; https://www.speedrun.com/api/v1/runs?category=wkpmnlwk&status=verified&max=200; https://www.speedrun.com/api/v1/runs?category=9kv70zjk&status=verified&max=200; https://www.speedrun.com/api/v1/runs?category=w207w65k&max=200 and https://www.speedrun.com/api/v1/runs?category=mkeq5vnk&max=200 (each also with &status=new, &status=verified, &status=rejected); https://www.speedrun.com/api/v1/runs?game=9d3rvxgd&status=verified&max=200 | 2026-10-05 |
| SRC-WR | https://www.speedrun.com/api/v1/runs/yjvd1ndy (page: https://www.speedrun.com/tsomi/runs/yjvd1ndy) | 2026-10-05 |
| SRC-RUNREC | https://www.speedrun.com/api/v1/runs/y8g6x1wz (page: https://www.speedrun.com/tsomi/runs/y8g6x1wz); https://www.speedrun.com/api/v1/runs/yoer2w5z (page: https://www.speedrun.com/tsomi/runs/yoer2w5z); https://www.speedrun.com/api/v1/runs/z1nk53ry (page: https://www.speedrun.com/tsomi/runs/z1nk53ry); https://www.youtube.com/oembed?url=https://www.youtube.com/watch?v=GE6QLGgdOqg&format=json; https://www.speedrun.com/api/v1/leaderboards/w6j4m46j/category/wkpmnlwk?embed=players; https://www.speedrun.com/api/v1/leaderboards/w6j4m46j/category/jdrqwqlk?embed=players | 2026-10-05 |
| SRC-SE | https://www.speedrun.com/api/v1/games/9d3rvxgd?embed=categories,variables; https://www.speedrun.com/api/v2/GetGameSummary?gameUrl=tsomise | 2026-10-05 |
| SRC-F-SPEED | https://www.speedrun.com/api/v2/GetThread?id=2bi5y (page: https://www.speedrun.com/tsomi/forums/2bi5y), aWay0fLife, 2017-08-18; https://www.speedrun.com/api/v2/GetThreadList?forumId=4zl1p12d&page=1 (25 threads) | 2026-10-05 |
| SRC-F-ODDITY | https://www.speedrun.com/api/v2/GetThread?id=by7vn&page=1 (page: https://www.speedrun.com/tsomi/forums/by7vn), 2017-08-16; https://www.speedrun.com/api/v1/runs/nz1r4ojm; https://www.speedrun.com/api/v1/runs/y23xde7m | 2026-10-05 |
| SRC-GUIDE-ANY | https://www.speedrun.com/api/v2/GetGuide?id=pnydx (page: https://www.speedrun.com/tsomi/guides/pnydx), updated 2019-11-14; https://www.speedrun.com/api/v1/users/18vk0z5j; https://www.speedrun.com/api/v1/users/5j5oykgx | 2026-10-05 |
| SRC-F-CREDIT | https://www.speedrun.com/api/v2/GetThread?id=lvlhg&page=1 (page: https://www.speedrun.com/tsomi/forums/lvlhg), aWay0fLife, 2020-10-03 | 2026-10-05 |
| SRC-F-SE | https://www.speedrun.com/api/v2/GetThread?id=2vb92&page=1 (page: https://www.speedrun.com/tsomise/forums/2vb92); https://www.speedrun.com/api/v2/GetGuide?id=01ykf (page: https://www.speedrun.com/tsomise/guides/01ykf) | 2026-10-05 |
| SRC-GUIDES | https://www.speedrun.com/api/v2/GetGuideList?gameId=w6j4m46j; https://www.speedrun.com/api/v2/GetGuide?id=693q7; https://www.speedrun.com/api/v2/GetGuide?id=p8zx7 (page: https://www.speedrun.com/tsomi/guides/p8zx7) | 2026-10-05 |
| SRC-SEED | https://www.speedrun.com/api/v2/GetResourceList?gameId=w6j4m46j (resource 2z7qm, updated 2026-07-23) | 2026-10-05 |
| SRC-G-CATA | https://www.speedrun.com/api/v2/GetGuide?id=hw8us (page: https://www.speedrun.com/tsomi/guides/hw8us) | 2026-10-05 |
| SRC-F-RULES | https://www.speedrun.com/api/v2/GetThread?id=4vc4y (page: https://www.speedrun.com/tsomi/forums/4vc4y), posts of 2017-08-24 and 2017-12-26 | 2026-10-05 |
| SRC-F-KEYS | https://www.speedrun.com/api/v2/GetThread?id=5mrk1&page=1 (page: https://www.speedrun.com/tsomi/forums/5mrk1), 2025-05-25 | 2026-10-05 |
| SRC-F-BUILDS | https://www.speedrun.com/api/v2/GetThread?id=2s9x6&page=1 (page: https://www.speedrun.com/tsomi/forums/2s9x6); https://www.speedrun.com/api/v2/GetThread?id=oa64e&page=1 (page: https://www.speedrun.com/tsomi/forums/oa64e) | 2026-10-05 |
| SRC-SITE | https://www.speedrun.com/api/v2/GetArticle?slug=site-rules (page: https://www.speedrun.com/support/learn/site-rules, updated 2026-02-05) Re-fetched for this revision: updatedAt 2026-02-05T22:15:15Z. | 2026-10-05 |
| SRC-F-IL | https://www.speedrun.com/api/v2/GetThread?id=1esx1 (page: https://www.speedrun.com/tsomi/forums/1esx1), 2025-06-24 to 2025-07-14 | 2026-10-05 |
| SRC-F-SAFE | https://www.speedrun.com/api/v2/GetThread?id=6etn4 (page: https://www.speedrun.com/tsomi/forums/6etn4), posts of 2025-07-03 and 2025-07-04; https://www.speedrun.com/api/v2/GetThread?id=m4npa&page=1 | 2026-10-05 |
| SRC-USER-MIKE | https://www.speedrun.com/api/v2/GetUserSummary?url=mikeSpeedyAdventures | 2026-10-05 |
| SRC-SCUMM-BOARDS | https://www.speedrun.com/api/v1/games?name=monkey%20island; https://www.speedrun.com/api/v1/games/w6j4m46j?embed=categories,platforms; https://www.speedrun.com/api/v1/games/268vo5dp?embed=categories | 2026-10-05 |
| SRC-GENRES | https://www.speedrun.com/api/v1/genres?max=200; https://www.speedrun.com/api/v1/genres/qdnqyk28; https://www.wikidata.org/wiki/Property:P6783; https://query.wikidata.org/sparql | 2026-10-05 |
| SRC-BULK | https://www.speedrun.com/api/v1/games?_bulk=yes&max=1000&offset=0 to 52000; https://www.speedrun.com/api/v1/runs?game=<id>&status=verified&max=200; https://www.scummvm.org/compatibility/ | 2026-10-05 |
| SRC-ADV | as SRC-BULK, plus https://www.speedrun.com/api/v1/games/<id>?embed=platforms,categories for each changed board (e.g. kings_quest_quest_for_the_crown_sci_remake, maniac_mansion_nes, quest_for_glory_so_you_want_to_be_a_hero_ega, dejavu, The_Uninvited, Uninvited_MacVenture_Series, amerzone) and https://www.scummvm.org/compatibility/DEV/scumm:maniac/, .../macventure:deja_vu/ | 2026-10-05 |
| SRC-BULK2 | https://www.speedrun.com/api/v1/games?_bulk=yes&max=1000&offset=0 to offset=52000; https://www.speedrun.com/api/v1/runs?game=<id>&status=verified&orderby=date&direction=desc&max=200 (one call per board, 119 curated SCUMM, SCI and AGI boards); https://www.scummvm.org/compatibility/ | 2026-10-05 |
| SRC-ACTIVE | https://www.speedrun.com/api/v2/GetGameSummary?gameUrl=puttzoo, ...=pjsam1, ...=mi2, ...=kings_quest_vi_heir_today_gone_tomorrow, ...=myst, ...=tsomi, ...=maniac_mansion_nes, ...=quest_for_glory_so_you_want_to_be_a_hero_ega, ...=nd_stfd | 2026-10-05 |
| TV-API | https://tasvideos.org/api/v1/Games?PageSize=100 (all pages); https://tasvideos.org/api/v1/Publications?ShowObsoleted=true&PageSize=100 (all pages); https://tasvideos.org/api/v1/Submissions?PageSize=100 (all pages, plus /api/v1/Submissions/{id} for the 86 missing ids); https://tasvideos.org/swagger/v1/swagger.json; https://tasvideos.org/Search?SearchTerms=monkey%20island; https://tasvideos.org/Forum/Posts/233379; https://tasvideos.org/Forum/Posts/532068; https://tasvideos.org/3211G; https://tasvideos.org/4333G; https://tasvideos.org/5243G | 2026-10-05 |
| TV-MI | https://tasvideos.org/Forum/Topics/12828; https://tasvideos.org/Forum/Posts/315229; https://tasvideos.org/Forum/Posts/315237; https://tasvideos.org/Forum/Posts/315241; https://tasvideos.org/Forum/Posts/315517; https://www.youtube.com/oembed?url=https://www.youtube.com/watch?v=oyEpNie-7pY&format=json; https://tasvideos.org/Users/Profile/TheNewTeddy; https://tasvideos.org/LegacyPages/ListOfIdeas | 2026-10-05 |
| TV-MI2 | https://tasvideos.org/4437G; https://tasvideos.org/GameResources/DOS/MonkeyIsland2 (links https://www.grumpygamer.com/speed_running_mi) | 2026-10-05 |
| TV-SCUMMVM | https://tasvideos.org/EmulatorResources/ScummVM; https://tasvideos.org/Forum/Topics/13228 | 2026-10-05 |
| TV-SVM | https://tasvideos.org/4496M (submission https://tasvideos.org/7068S); https://tasvideos.org/5827M; https://tasvideos.org/5840M; https://tasvideos.org/6408M; https://tasvideos.org/8249S; https://tasvideos.org/10699S; https://tasvideos.org/api/v1/publications and https://tasvideos.org/api/v1/submissions (full listings) | 2026-10-05 |
| TV-MYST | https://tasvideos.org/7441S (published as 4669M) | 2026-10-05 |
| TV-PJS | https://tasvideos.org/7846S; https://tasvideos.org/4985M | 2026-10-05 |
| TV-PUTT | https://tasvideos.org/8528S (published as 5571M) | 2026-10-05 |
| TV-LOOM | https://tasvideos.org/9181S; https://tasvideos.org/Forum/Topics/25549; https://www.youtube.com/watch?v=doMYYgBoHzk; https://www.youtube.com/oembed?url=https://www.youtube.com/watch?v=doMYYgBoHzk&format=json | 2026-10-05 |
| TV-MM | https://tasvideos.org/84G (movie 6029M); https://tasvideos.org/9092S; https://tasvideos.org/8312S | 2026-10-05 |
| TV-COMI | https://tasvideos.org/Forum/Topics/18435; https://tasvideos.org/HomePages/eien86 | 2026-10-05 |
| TV-BORG | https://tasvideos.org/Forum/Topics/26629; https://github.com/TASEmulators/BizHawk/pull/4595; https://tasvideos.org/UserFiles/ForUser/Emil_Borg | 2026-10-05 |
| BIZHAWK | https://github.com/TASEmulators/BizHawk/releases/tag/2.11; https://api.github.com/repos/TASEmulators/BizHawk/releases/tags/2.11; https://api.github.com/repos/TASEmulators/BizHawk/releases?per_page=8; https://tasvideos.org/Bizhawk/DOSBox; https://tasvideos.org/Bizhawk/ReleaseHistory; https://tasvideos.org/api/v1/Publications?Systems=DOS&PageSize=100 (pages 1 to 5) | 2026-10-05 |
| TW-MI2 | https://www.twitch.tv/videos/2084827018 | 2026-10-05 |
| RENTRY | https://rentry.co/fp3eb; https://rentry.co/dott-tas; https://www.twitch.tv/videos/2084827017; https://www.twitch.tv/videos/1787419604 | 2026-10-05 |
| YT-SEARCH | https://www.youtube.com/results?search_query=monkey+island+TAS; https://www.youtube.com/results?search_query=secret+of+monkey+island+tool+assisted+speedrun; https://www.youtube.com/results?search_query=day+of+the+tentacle+TAS; https://gql.twitch.tv/gql (searchFor: "monkey island tas", "secret of monkey island tas", "scummvm tas", "monkey island bot", "scumm tas", "monkey island libtas") | 2026-10-05 |
| YT-TAS | https://www.youtube.com/watch?v=16vwmMZk8T8; https://www.youtube.com/watch?v=FEmdfQLpBR4; https://www.youtube.com/watch?v=xPJl1nbQqlE; https://www.youtube.com/watch?v=how0HtkS8J4; https://www.youtube.com/watch?v=MTI7h3LGK5k; https://www.youtube.com/watch?v=RYjcp5-apUM; https://www.youtube.com/watch?v=RCYPUNy8Eyw; https://www.speedrun.com/api/v1/runs?user=86n0dowx&max=200&embed=game,category,platform; https://www.speedrun.com/api/v1/runs?user=5j5oykgx&max=200&embed=game,category,platform | 2026-10-05 |
| AI-YT | https://www.youtube.com/watch?v=57h1SwlC-2k; https://www.youtube.com/watch?v=NJbDvZJOuqs; https://www.youtube.com/watch?v=d5S3gSvQ80I; https://www.youtube.com/watch?v=iM3PCpzhzcU; https://www.youtube.com/watch?v=l1rNh8Zy-Yk; https://www.youtube.com/watch?v=dust0_XYxN8; https://www.youtube.com/watch?v=h4eGj12wRAE; https://www.youtube.com/results?search_query=AI+plays+monkey+island; https://www.youtube.com/results?search_query=scummvm+mcp+monkey+island | 2026-10-05 |
| SCUMMBENCH | https://github.com/rabengraph/scummbench; https://raw.githubusercontent.com/rabengraph/scummbench/main/README.md; https://raw.githubusercontent.com/rabengraph/scummbench/main/claude/runbook.md (line 47); https://raw.githubusercontent.com/rabengraph/scummvm/develop/engines/scumm/AGENT_HARNESS.md (section 9, "Usage notes") | 2026-10-05 |
| MCP-README | https://github.com/xavierhardy/scummvm-mcp; https://raw.githubusercontent.com/xavierhardy/scummvm-mcp/master/README.md; https://api.github.com/repos/xavierhardy/scummvm-mcp; https://api.github.com/repos/xavierhardy/scummvm-mcp/commits?path=README.md | 2026-10-05 |
| MCP-SRC | https://raw.githubusercontent.com/xavierhardy/scummvm-mcp/master/engines/scumm/mcp.cpp (toolAct lines 1219-1821; comments at about 1361-1388 and 1505-1543); .../mcp_subclasses.h; .../mcp.h; .../mcp_classic.cpp; .../mcp_v7.cpp; .../mcp_v8.cpp; .../script.cpp; https://raw.githubusercontent.com/xavierhardy/scummvm-mcp/master/test/mcp/README.md; https://api.github.com/repos/xavierhardy/scummvm-mcp/commits?path=engines/scumm/mcp.cpp (HEAD 2750e5a6, 2026-09-17) | 2026-10-05 |
| WEBMCP | https://github.com/philnash/monkey-island-webmcp | 2026-10-05 |
| LUISHG | https://luishg.com/2025/05/26/teaching-llms-to-play-classic-graphic-adventures/; https://github.com/luishg/llmplaysadventuregames | 2026-10-05 |
| SGG | https://github.com/mnuhn/ScummGameGraph | 2026-10-05 |
| SVM-APRIL | https://www.scummvm.org/en/news/20230401 | 2026-10-05 |
| EVREC | https://wiki.scummvm.org/index.php?title=Event_Recorder&action=raw; https://wiki.scummvm.org/index.php?title=Summer_of_Code/GSoC2012&action=raw; https://github.com/scummvm/scummvm/commit/77eea722af; https://github.com/scummvm/scummvm/commit/f59512c47e; https://lists.scummvm.org/pipermail/scummvm-git-logs/2013-July/064741.html; https://github.com/scummvm/scummvm/blob/master/configure; https://github.com/scummvm/scummvm/commits/master/gui/EventRecorder.cpp; https://web.archive.org/web/20251126210159/https://wiki.scummvm.org/index.php/Event_Recorder; https://raw.githubusercontent.com/scummvm/scummvm/8e5d6896e7ba8bc46c2c6c5ac5a0a526ab27371d/configure; .../gui/EventRecorder.cpp; .../gui/EventRecorder.h; .../common/recorderfile.h; .../common/system.h; .../engines/freescape/events.cpp; https://github.com/scummvm/scummvm/pull/7119; https://github.com/scummvm/scummvm/pull/7555; https://github.com/scummvm/scummvm/pull/7870; https://github.com/scummvm/scummvm/pull/7955 | 2026-10-05 |
| LUCASARTSIFIER | https://github.com/katiahayati/lucasartsifier/blob/HEAD/tools/build_recorder_scummvm.sh; https://github.com/katiahayati/lucasartsifier/blob/HEAD/tools/scummvm-patches/0004-scripted-input-and-a-controllable-clock.patch; https://github.com/katiahayati/lucasartsifier/blob/HEAD/docs/PLAYTEST-AUTOMATION.md | 2026-10-05 |
| T7G | https://github.com/ashnasbot/t7g-ml; https://github.com/ashnasbot/t7g-ml/blob/79e48ed9f80ea42cc05c77997c5b5db2f9ce9a3c/t7g_env.py; https://github.com/ashnasbot/t7g-ml/commit/8fa6071369a98399ec973e6022a1d8c7dc52dfe7 | 2026-10-05 |
| ICARUS | https://research-portal.uu.nl/en/publications/automated-game-testing-with-icarus-intelligent-completion-of-adve/ | 2026-10-05 |
| OTHER-AUTO | https://github.com/Rubyboat1207/pajama-neuro; https://github.com/fabianostermann/scummvm-WoodtickRL; https://github.com/fabianostermann/WoodtickWalkingSimulator; https://github.com/roccozanni/grogvm | 2026-10-05 |
| JAFFAR | https://github.com/ToolAssisted-run/jaffarPlus; https://github.com/SergioMartin86/jaffar | 2026-10-05 |
| DOSBOX-AUTO | https://vogons.org/viewtopic.php?t=111800; https://tasvideos.org/Forum/Topics/27626; https://www.dosbox-automation.org/ | 2026-10-05 |
| NEG-SEARCH | WebSearch queries listed in the first research pass, e.g. "PDDL planning point-and-click adventure game puzzles", "SCUMM OR LucasArts game planning domain PDDL automated solver Monkey Island" (extended), "automated speedrun route optimization adventure game planner ScummVM bot" (extended), "speedrun route computed with TSP solver Concorde LKH 100% collectibles" (extended); plus, for this document, "Monkey Island 1 TAS tool-assisted speedrun ScummVM youtube" (extended) and "tasvideos.org \"Monkey Island\" submission libTAS" (extended); for this revision, GitHub repository and code searches (https://api.github.com/search/repositories and https://api.github.com/search/code: "scummvm tas", "scummvm automation", "scummvm llm", "scumm pddl", "scumm planner", "scummvm reinforcement learning", "scummvm gym", "descumm pddl", "monkey island fast downward"), the ScummVM fork list (https://api.github.com/repos/scummvm/scummvm/forks, 1,282 of 1,402 direct forks scanned), https://export.arxiv.org/api/query?search_query=all:ScummVM and https://api.openalex.org/works?search=puzzle%20dependency%20graph | 2026-10-05 |
| P-PICKETT | https://www.sable.mcgill.ca/~clump/papers/pickett-05-pnfg.pdf; https://www.sable.mcgill.ca/~clump/bib/pickett-05-pnfg.bib; https://www.informatik.uni-hamburg.de/TGI/pnbib/p/pickett_c_j_f1.html | 2026-10-05 |
| P-VERBRUGGE | https://www.eecg.utoronto.ca/~steffan/workshops/08/cdp/verbrugge.pdf; https://www.eecg.utoronto.ca/~steffan/workshops/08/cdp/; https://www.eecg.utoronto.ca/~steffan/workshops/08/cdp/verbrugge.html; https://www.sable.mcgill.ca/~clump/papers/verbrugge-10-analyzing.pdf; https://mcgill.scholaris.ca/items/363c15a5-1ba9-44ac-acfb-3e63cd9e72e9 | 2026-10-05 |
| P-LESTER | https://arxiv.org/abs/2012.15365v1; https://arxiv.org/pdf/2012.15365; https://centaur.reading.ac.uk/99135/; https://export.arxiv.org/abs/2012.15365 | 2026-10-05 |
| P-ERDI | https://erdi.dev/blog/2020-08-01-solving_text_adventure_games_via_symbolic_execution/; https://erdi.dev/talks/2020-10-symbolic-adventure/2020-10-BerlinFP-SymbolicAdventure.pdf; https://erdi.dev/papers/2020-ifl-symbolic-adventure/2020-ifl-symbolic-adventure.pdf; https://github.com/gergoerdi/scottcheck/blob/master/src/main.hs | 2026-10-05 |
| P-MORENO | https://api.crossref.org/works?query.title=Model-checking+for+adventure+videogames (DOI 10.1016/j.infsof.2008.08.003) | 2026-10-05 |
| P-LUDO | https://www.kmjn.org/publications/Ludocore_CIG10-abstract.html | 2026-10-05 |
| P-NATKIN | https://sgruenvo.web.th-koeln.de/download/a-new-methodology-for-spatiotemporal-game-design/; https://sgruenvo.web.th-koeln.de/?p=255; https://api.archives-ouvertes.fr/search/?q=halId_s:hal-01124895; https://cedric.cnam.fr/fichiers/art_2993.pdf; https://vldb.org/dblp/db/indices/a-tree/n/Natkin:St=eacute=phane.html; https://api.openalex.org/works/W109560118; https://api.openalex.org/works/W181867969 | 2026-10-05 |
| P-YERNAUX | https://doi.org/10.1007/978-3-032-11043-5_26; https://api.openalex.org/works?search=Formalizing%20Escape%20Game%20Mechanics | 2026-10-05 |
| P-BARTHEYE | https://ojs.aaai.org/index.php/AIIDE/article/view/12370 | 2026-10-05 |
| P-BALYO | https://arxiv.org/abs/2402.12393; https://export.arxiv.org/pdf/2402.12393 | 2026-10-05 |
| P-BALYO25 | https://www.ijcai.org/proceedings/2025/1250 | 2026-10-05 |
| P-LIPO | https://ijcai.org/Abstract/15/230; https://mlanthology.org/ijcai/2015/lipovetzky2015ijcai-classical | 2026-10-05 |
| P-ALFWORLD | https://export.arxiv.org/pdf/2010.03768 (appendix C); https://github.com/microsoft/TextWorld; https://export.arxiv.org/abs/1806.11532 | 2026-10-05 |
| P-PUZZLE | https://arxiv.org/abs/2310.01471; https://arxiv.org/abs/2310.01503; https://arxiv.org/abs/2109.00449 | 2026-10-05 |
| P-GOBBI | https://sol.sbc.org.br/index.php/sbgames/article/view/45395 | 2026-10-05 |
| P-ZOOK | https://faculty.cc.gatech.edu/~riedl/pubs/zook-aaai14.pdf | 2026-10-05 |
| P-VICTOR | https://github.com/VictorLaugt/pddl-planning-for-adventure-game | 2026-10-05 |
| P-GUAN | https://neurips.cc/virtual/2023/poster/69907 | 2026-10-05 |
| P-OSWALD | https://ojs.aaai.org/index.php/ICAPS/article/view/31502 | 2026-10-05 |
| P-PDDLEGO | https://arxiv.org/abs/2405.19793 | 2026-10-05 |
| P-STORY2GAME | https://export.arxiv.org/abs/2505.03547 | 2026-10-05 |
| P-FLASH | https://arxiv.org/abs/2509.01052; https://export.arxiv.org/pdf/2509.01052v2 | 2026-10-05 |
| P-VGB | https://arxiv.org/abs/2505.18134 | 2026-10-05 |
| P-ORAK | https://arxiv.org/abs/2506.03610 | 2026-10-05 |
| P-AGENTS | https://export.arxiv.org/abs/2411.13543 (BALROG); https://export.arxiv.org/abs/2412.13549 (EscapeBench); https://export.arxiv.org/abs/2602.17594 (AI GameStore) | 2026-10-05 |
| P-TQ | https://arxiv.org/abs/2507.23701; https://arxiv.org/html/2507.23701v3 (Appendix C, Table 4); https://www.textquests.ai/ (leaderboard data in https://www.textquests.ai/_next/static/chunks/app/page-fb73b71b40005eb3.js?dpl=dpl_DLvsFBbWvoZY25EN8RiDYoWvHMfu); https://leaderboard.safe.ai/ | 2026-10-05 |
| P-JERICHO | https://arxiv.org/abs/1909.05398; https://export.arxiv.org/pdf/1909.05398; https://ojs.aaai.org/index.php/AAAI/article/view/6297 | 2026-10-05 |
| P-TEXTWORLD | https://arxiv.org/abs/1806.11532 | 2026-10-05 |
| P-TSAI | https://arxiv.org/abs/2304.02868 | 2026-10-05 |
| P-GOEXPLORE | https://export.arxiv.org/abs/1901.10995 | 2026-10-05 |
| P-GILBERT | https://grumpygamer.com/puzzle_dependency_charts/ (re-fetched for this revision) | 2026-10-05 |
| P-FALSTEIN | https://gdcvault.com/play/1017978/The-Arcane-Art-of-Puzzle | 2026-10-05 |
| P-WEINBERG | https://www.gamedeveloper.com/design/puzzle-dependency-graph-primer | 2026-10-05 |
| P-BAUMER | https://www.gamedeveloper.com/design/designing-puzzles-for-adventure-games-analyzing-monkey-island-2-puzzle-dependencies-and-balancing | 2026-10-05 |
| P-PDTOOLS | https://api.github.com/search/repositories?q=puzzle%20dependency%20chart; https://github.com/nathanhoad/godot_puzzle_dependencies; https://github.com/rocketsurgery-games/gnusto/blob/main/docs/frotz.md; https://github.com/rocketsurgery-games/gnusto/blob/main/src/frotz/cli.py; https://github.com/rocketsurgery-games/gnusto/blob/main/games/notes.md; https://github.com/ChicagoDave/sharpee/blob/main/packages/world-index/src/reach.ts; https://github.com/ChicagoDave/sharpee/blob/main/docs/proposals/state-space-analysis.md | 2026-10-05 |
| P-PCG | https://www.kmjn.org/publications/Puzzle_CIG12-abstract.html; https://api.crossref.org/works/10.1145/2538528.2538538; https://dspace.mit.edu/handle/1721.1/100267; https://pcgworkshop.com/archive/fernández-vara2012procedural.pdf; https://api.semanticscholar.org/graph/v1/paper/DOI:10.1007/978-3-030-33894-7_25?fields=title,authors,abstract,venue,year,publicationDate; https://www.tara.tcd.ie/items/58270d70-f98a-462e-bdf3-5a21672dd3ba; https://api.crossref.org/works/10.1109/tg.2019.2917792; https://arxiv.org/abs/1808.06217 | 2026-10-05 |
| P-GROSS | https://arxiv.org/abs/2106.01182; https://export.arxiv.org/pdf/2106.01182v3; https://epb.bibl.th-koeln.de/frontdoor/index/index/year/2022/docId/2049 | 2026-10-05 |
| P-ROUTERS | https://github.com/wxarmstrong/YakuzaRouter; https://github.com/Yoyoshix/Road_to_Ballhalla---Speedrun_Ressources; JaV and Iškovs are reported in https://export.arxiv.org/pdf/2106.01182v3 | 2026-10-05 |
| P-LAFOND | https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.FUN.2018.27; https://drops.dagstuhl.de/opus/volltexte/2018/8818 | 2026-10-05 |
| P-GAJDUK | https://arxiv.org/abs/1608.06175 | 2026-10-05 |
| P-RANDO | https://www.ece.lsu.edu/lpeng/papers/IEEE-Acc-21.pdf; https://randommetroidsolver.pythonanywhere.com/infos; https://github.com/grazz/OoT-Randomizer; https://github.com/Dabomstew/pokemon-routetwo; https://github.com/OoTRandomizer/OoT-Randomizer | 2026-10-05 |
| P-MURPHY | https://sigbovik.org/2013/proceedings.pdf | 2026-10-05 |
| P-COOK | https://ceur-ws.org/Vol-4090/paper1.pdf; https://ceur-ws.org/Vol-4090/ | 2026-10-05 |
| SVM-REL | https://api.github.com/repos/scummvm/scummvm/releases; https://www.scummvm.org/news/archive/; https://www.scummvm.org/news/20260131/ | 2026-10-05 |
| SVM-ENGINES | https://api.github.com/repos/scummvm/scummvm/contents/engines?ref=v2026.3.0; https://api.github.com/repos/scummvm/scummvm/contents/engines; https://raw.githubusercontent.com/scummvm/scummvm/v2026.3.0/engines/<engine>/configure.engine; https://api.github.com/repos/scummvm/scummvm/contents/engines?ref=8e5d6896e7ba8bc46c2c6c5ac5a0a526ab27371d; https://raw.githubusercontent.com/scummvm/scummvm/8e5d6896e7ba8bc46c2c6c5ac5a0a526ab27371d/engines/<engine>/configure.engine (all 126 files) | 2026-10-05 |
| SVM-COMPAT | https://www.scummvm.org/compatibility/ | 2026-10-05 |
| SVM-DET | https://raw.githubusercontent.com/scummvm/scummvm/v2026.3.0/engines/ags/detection_tables.h; .../engines/director/detection_tables.h; .../engines/glk/zcode/detection_tables.h; .../engines/glk/glulx/detection_tables.h; .../engines/glk/adrift/detection_tables.h; .../engines/glk/tads/detection_tables.h; .../engines/glk/configure.engine; https://www.speedrun.com/api/v1/runs?game=w6jxnxdj&max=200; https://www.speedrun.com/api/v1/runs?game=j1nxpxdp&max=200 | 2026-10-05 |
| SVM-AGS-DET | https://raw.githubusercontent.com/scummvm/scummvm/8e5d6896e7ba8bc46c2c6c5ac5a0a526ab27371d/engines/ags/detection_tables.h (GAME_NAMES lines 24 to 3875; detection entries from line 4034); https://raw.githubusercontent.com/scummvm/scummvm/v2026.3.0/engines/ags/detection_tables.h | 2026-10-05 |
| SCUMM-DET | https://github.com/scummvm/scummvm/blob/v2026.3.0/engines/scumm/detection_tables.h (lines 60-136, 171-243); https://wiki.scummvm.org/index.php?title=SCUMM/Versions&action=raw | 2026-10-05 |
| SCUMM-V4 | https://raw.githubusercontent.com/scummvm/scummvm/v2026.3.0/engines/scumm/scumm_v4.h; https://raw.githubusercontent.com/scummvm/scummvm/v2026.3.0/engines/scumm/metaengine.cpp; https://raw.githubusercontent.com/scummvm/scummvm/v2026.3.0/engines/scumm/script.cpp | 2026-10-05 |
| DESCUMM | https://raw.githubusercontent.com/scummvm/scummvm-tools/master/engines/scumm/descumm-tool.cpp; https://raw.githubusercontent.com/dwatteau/scummtr/master/README.md; https://raw.githubusercontent.com/BLooperZ/nutcracker/develop/README.md; https://raw.githubusercontent.com/EricOakford/SCUMM-Decompilation-Archive/main/puttdemo_dos/rm1.scu; .../moondemo_dos/rm7.scu; .../fbdemo_dos/rm1.scu; https://raw.githubusercontent.com/scummvm/scummvm/master/engines/scumm/insane/rebel1/rebel.cpp; https://raw.githubusercontent.com/scummvm/scummvm/master/engines/scumm/detection_tables.h; https://manpages.debian.org/trixie/scummvm-tools/descumm.6.en.html | 2026-10-05 |
| SCUMM-ENH | https://raw.githubusercontent.com/scummvm/scummvm/8e5d6896e7ba8bc46c2c6c5ac5a0a526ab27371d/engines/scumm/metaengine.cpp; .../engines/scumm/script_v5.cpp | 2026-10-05 |
| SVM-CP2 | https://docs.scummvm.org/en/latest/use_scummvm/add_play_games.html; https://raw.githubusercontent.com/scummvm/scummvm/8e5d6896e7ba8bc46c2c6c5ac5a0a526ab27371d/engines/scumm/metaengine.cpp; .../engines/scumm/scumm.cpp; .../engines/scumm/script_v5.cpp; .../engines/agi/metaengine.cpp; .../engines/agi/cycle.cpp; .../engines/gob/gob.cpp; .../engines/agos/agos.cpp; .../engines/saga/saga.cpp; .../engines/lure/lure.cpp | 2026-10-05 |
| SCI-CON | https://github.com/scummvm/scummvm/blob/v2026.3.0/engines/sci/console.cpp#L90-L260; https://github.com/scummvm/scummvm/blob/v2026.3.0/engines/agi/console.cpp#L38-L60; https://github.com/scummvm/scummvm/blob/v2026.3.0/engines/ags/console.cpp#L34-L38; https://github.com/scummvm/scummvm/blob/v2026.3.0/gui/debugger.cpp#L66-L83 | 2026-10-05 |
| SCI-SCRIPTS | https://github.com/sluicebox/sci-scripts; https://sciwiki.sierrahelp.com/index.php/SCI_Companion | 2026-10-05 |
| SCI-WIKI | https://en.wikipedia.org/wiki/Sierra%27s_Creative_Interpreter | 2026-10-05 |
| SCI-TIME | https://raw.githubusercontent.com/scummvm/scummvm/v2026.3.0/engines/sci/engine/kmisc.cpp | 2026-10-05 |
| SCI-PATCH | https://raw.githubusercontent.com/scummvm/scummvm/8e5d6896e7ba8bc46c2c6c5ac5a0a526ab27371d/engines/sci/engine/script_patches.cpp | 2026-10-05 |
| SCI-CP | https://raw.githubusercontent.com/scummvm/scummvm/8e5d6896e7ba8bc46c2c6c5ac5a0a526ab27371d/engines/sci/metaengine.cpp; .../engines/sci/engine/script_patches.cpp; .../engines/sci/graphics/cursor.cpp (line 262); .../engines/sci/graphics/text16.cpp (line 607) | 2026-10-05 |
| AGI-TOOLS | https://wiki.sierrahelp.com/index.php/WinAGI; https://www.ifarchive.org/if-archive/programming/agi/ | 2026-10-05 |
| AGS-TOOLS | https://github.com/rofl0r/agsutils; https://raw.githubusercontent.com/rofl0r/agsutils/master/README; https://raw.githubusercontent.com/scummvm/scummvm/8e5d6896e7ba8bc46c2c6c5ac5a0a526ab27371d/engines/ags/console.cpp; .../gui/debugger.cpp; .../engines/ags/engine/script/cc_instance.h; .../engines/ags/engine/script/cc_instance.cpp; https://github.com/adm244/Ghidra-ReAGS; https://github.com/adm244/Ghidra-ReAGS/releases; https://github.com/adm244/AGSUnpacker; https://github.com/bequidox74/ags-decomp | 2026-10-05 |
| AGS-API | https://raw.githubusercontent.com/scummvm/scummvm/8e5d6896e7ba8bc46c2c6c5ac5a0a526ab27371d/engines/ags/engine/ac/global_object.h (line 74); .../engines/ags/engine/ac/global_game.h (line 98); .../engines/ags/engine/ac/global_character.h (line 69) | 2026-10-05 |
| TOOLS | https://api.github.com/repos/scummvm/scummvm-tools/contents/engines; https://api.github.com/repos/scummvm/scummvm-tools/contents/decompiler; https://raw.githubusercontent.com/scummvm/scummvm-tools/master/engines/wintermute/decompile_script.py; https://github.com/scummvm/scummvm-tools; https://raw.githubusercontent.com/scummvm/scummvm/8e5d6896e7ba8bc46c2c6c5ac5a0a526ab27371d/engines/wintermute/debugger.cpp; .../engines/director/debugger.cpp; https://github.com/ProjectorRays/ProjectorRays | 2026-10-05 |
| LINGODEC | https://lists.scummvm.org/pipermail/scummvm-git-logs/2024-May/105553.html | 2026-10-05 |
| AGOS-TOOLS | https://api.github.com/repos/scummvm/scummvm-tools/contents/engines/agos; https://api.github.com/repos/scummvm/scummvm-tools/contents/engines/tinsel; https://github.com/scummvm/scummvm/blob/8e5d6896e7ba8bc46c2c6c5ac5a0a526ab27371d/engines/agos/debug.cpp; .../engines/agos/event.cpp; .../engines/agos/detection.cpp; .../engines/tinsel/pcode.cpp; .../engines/tinsel/debugger.cpp; https://github.com/adventurebrew/magos; https://github.com/adventurebrew/magos/releases/tag/v0.6.0; https://github.com/peterkohaut/tinsel3viewer; https://github.com/phoenix1of1/GhidraMCP | 2026-10-05 |
| CPP-LOGIC | https://api.github.com/repos/scummvm/scummvm/contents/engines/bladerunner/script/scene; https://raw.githubusercontent.com/scummvm/scummvm/v2026.3.0/engines/drascula/rooms.cpp; https://raw.githubusercontent.com/scummvm/scummvm/v2026.3.0/engines/teenagent/callbacks.cpp; https://raw.githubusercontent.com/scummvm/scummvm/v2026.3.0/engines/darkseed/usecode.cpp; https://api.github.com/repos/scummvm/scummvm/contents/engines/neverhood/modules; https://api.github.com/repos/scummvm/scummvm/git/trees/8e5d6896e7ba8bc46c2c6c5ac5a0a526ab27371d?recursive=1; https://raw.githubusercontent.com/scummvm/scummvm/8e5d6896e7ba8bc46c2c6c5ac5a0a526ab27371d/engines/mads/dragonsphere/rooms/room101.cpp; .../engines/ngi/fullpipe/scene01.cpp; .../engines/lastexpress/characters/anna.cpp; .../engines/titanic/game/arboretum_gate.cpp; .../engines/startrek/rooms/demon0.cpp; .../engines/neverhood/modules/module1000.cpp; .../engines/hadesch/rooms/argo.cpp; .../engines/mohawk/riven_stacks/domespit.h; https://api.github.com/repos/scummvm/scummvm/commits?path=engines/mads/dragonsphere/rooms/room101.cpp&per_page=100 | 2026-10-05 |
| CPP-HYBRID | https://github.com/scummvm/scummvm/blob/v2026.3.0/engines/ngi/interaction.cpp; .../engines/ngi/fullpipe/scene01.cpp; .../engines/queen/logic.cpp; .../engines/queen/command.cpp; .../engines/gnap/scenes/group0.cpp; .../engines/dragons/specialopcodes.cpp; .../engines/bladerunner/script/scene/ar01.cpp; .../engines/bladerunner/bladerunner.cpp; .../engines/engine.h; .../gui/EventRecorder.h; https://docs.scummvm.org/en/latest/advanced_topics/command_line.html | 2026-10-05 |
| RT-CODE | https://api.github.com/repos/scummvm/scummvm/git/trees/8e5d6896e7ba8bc46c2c6c5ac5a0a526ab27371d?recursive=1; https://raw.githubusercontent.com/scummvm/scummvm/8e5d6896e7ba8bc46c2c6c5ac5a0a526ab27371d/engines/twine/twine.cpp; .../engines/twine/detection.cpp; .../engines/hypno/detection.cpp; .../engines/hypno/spider/hard.cpp; .../engines/ultima/detection_tables.h; .../engines/scumm/detection_tables.h; .../engines/scumm/insane/insane.h; .../engines/scumm/insane/insane_scenes.cpp; .../engines/nancy/action/puzzle/arcadepuzzle.h; .../engines/lure/fights.h; .../engines/harvester/room_combat.h; .../engines/gnap/scenes/arcade.h; .../engines/got/game/boss1.h; https://www.scummvm.org/compatibility/; https://en.wikipedia.org/w/index.php?title=SCUMM&action=raw | 2026-10-05 |
| NANCY | https://api.github.com/repos/scummvm/scummvm/contents/engines/nancy/action/puzzle?ref=v2026.3.0 | 2026-10-05 |
| SVM-RNG | https://raw.githubusercontent.com/scummvm/scummvm/8e5d6896e7ba8bc46c2c6c5ac5a0a526ab27371d/common/random.cpp; .../common/forbidden.h; .../base/commandLine.cpp (lines 202 and 980); .../gui/EventRecorder.cpp (lines 412 to 421); .../engines/ags/engine/main/engine.cpp (lines 518 to 519); .../engines/efh/efh.cpp (line 281); .../engines/watchmaker/ll/ll_diary.cpp (line 117); .../engines/glk/level9/level9_main.cpp (lines 2990 to 2993); .../engines/agos/script_pn.cpp (line 572); .../common/lua/lmathlib.cpp; .../engines/grim/lua/lmathlib.cpp; .../engines/ultima/nuvie/script/script.cpp; .../engines/director/util.cpp; https://raw.githubusercontent.com/scummvm/scummvm/v2026.3.0/base/commandLine.cpp (line 202) | 2026-10-05 |
| BR-INIT | https://raw.githubusercontent.com/scummvm/scummvm/v2026.3.0/engines/bladerunner/script/init_script.cpp | 2026-10-05 |
| GAME-WIKI | https://en.wikipedia.org/wiki/Full_Throttle_(1995_video_game); https://en.wikipedia.org/wiki/Indiana_Jones_and_the_Fate_of_Atlantis; https://en.wikipedia.org/wiki/Indiana_Jones_and_the_Last_Crusade:_The_Graphic_Adventure | 2026-10-05 |
| L-README | Local: README.md | 2026-10-05 |
| L-RULES | Local: rules/glitchless.md | 2026-10-05 |
| L-SEG | Local: pddl/part1/segment.toml (goal, line 24) | 2026-10-05 |
| L-SCRIPTS | Local (generated, gitignored): data/scripts/global/script-116.txt ([010A], [010F]); data/scripts/room-044-masters-h/obj-0596-100-cotton-t-shirt.txt; data/scripts/room-064-treasure/local-200.txt ([020C], [0214]) | 2026-10-05 |
| L-CMP | Local: docs/comparison.md §1.2, §1.4, §2 | 2026-10-05 |
| L-OPT | Local: docs/optimization.md | 2026-10-05 |
| L-PLANLOG | Local: out/plans/part1.log (lines 254 to 260) | 2026-10-05 |
| L-HR | Local: docs/human-route.md §1.1 | 2026-10-05 |
| L-NEXT | Local: docs/next.md | 2026-10-05 |
| L-PATCH | Local: patches/0001-scumm-add-speedrun-bridge.patch (lines 1932-1933, 2224, 2304) | 2026-10-05 |
