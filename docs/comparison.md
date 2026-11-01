# Comparison: our Part I route against the human route

This report compares the bot's Part I segment with the human speedrun route. The segment runs from free control on the Dock until the treasure trial (`Bit[86]`) and the idol trial (`Bit[85]`) are both complete (`pddl/part1/segment.toml`).

Our route is the time-optimal plan `out/plans/part1.time.sas_plan` (67 actions), compiled to `out/plans/part1.time.jsonl` (68 steps). `speedrun optimize part1` chose it (candidate `sample-16`) on bridge v2 with the position-keyed surrogate and every room keyed (`docs/optimization.md`; run `out/optimize/20261005T111318Z`). Compared with the first time-optimal plan, `sample-02`, it takes forest path 686 instead of 685 at the Fork, twice (D12), and drugs the meat at the petal screen instead of in the jail (D6). Action numbers `#1` to `#67` are the plan's lines.

The human route is `docs/human-route.md`, and step numbers `H1` to `H74` are its §3.2 steps. Its timed sources are saruya's 2025 Part 1 individual-level run (IL, DOS VGA floppy) and the 2026 world record (WR, DOS CD). Both runs press `.` and Esc, and both use the logo speed glitch, which is banned for us (§2).

Every tick number below comes from runs with text skip and cutscene skip on, talk speed at maximum (`talkspeed` 255, var 37 = 0) and bridge v2. The bot defers input in frames where a human's input would be lost (§6.3). Seed-1 ticks come from `out/runs/20261005T122038Z-run/trace.jsonl`, read with `speedrun.trace.load_trace`. Trace steps 0 to 10 are `#1` to `#11`, steps 11 and 12 are the two pushes of `#12`, and from step 13 on the trace index equals the plan number.

The earlier unit-cost comparison (66 actions, no skips, talk speed 60) is condensed in the Appendix. Its classified differences are reused here wherever the two routes still agree.

Script citations use `data/scripts/<file> [XXXX]`, shortened to `<room>/<file> [XXXX]` inside tables. Doc citations name the file and section.

## 1. Summary

### 1.1 Counts

There are two counting conventions:

- **Naming rule** (`docs/part1/model.md` §1; `src/speedrun/stats.py`). An action is a room transition if and only if its name starts with `walk`.
- **Human-comparable.** Our four composite `walk-*` actions are recounted the way the human list counts them:
  - `walk-to-kitchen-door-provoking-cook` (#3) is an Open sentence. It counts as 1 action and 0 transitions.
  - `walk-out-of-tent-after-helmet-meat` (#13) counts as 1 action (the helmet) plus 1 transition (the scripted walk-out, which the human list marks [T]).
  - `walk-past-fester-to-underwater` (#52) counts as 1 action (Open door). Its scripted transitions count 0.
  - `walk-up-ladder-taking-idol` (#53) counts as 1 action (Pick up idol). Its scripted transition counts 0.

| | ours, naming rule | ours, human-comparable | human, as listed (human-route §4) | human, corrected for D1 and D9 |
|---|---:|---:|---:|---:|
| actions | 19 | 23 | 22 | 23 |
| room transitions | 48 | 45 | 48 (from the Lookout) | 47 |
| actions + transitions | 67 | 68 | 70 | 70 |
| compiled engine steps | 68 | | | |
| dialogue choices | 11, plus 2 verb-slot clicks | | about 12-14 as listed; about 11-13 with the store at 3 picks (D7) | |

**Reconciliation, human-comparable:**

- **Actions: 23 against 22.** The +1 is D9, the idol-room Walk to. Against the corrected 23 the delta is 0.
  - D2 is 0: our #3 is an Open on 316, as the human's H5 is.
  - D3 is 0: both routes use two pickups.
- **Transitions: 45 against 48.** The −3 comes from D1 (the scripted Lookout → Dock move), D2 (no curtain walk on the way in) and D4 (no curtain walk on the way out). Against the corrected 47 the delta is −2.
- Under the naming rule, D2 reads −1 action and 0 transitions instead. Either way it is one input fewer.
- D5 to D8 and D10 change no count. D11 covers the swordfight and crew steps, which both sides leave out. With them, the human run as played has +10 actions and +6 transitions: 32 actions and 54 transitions from the Lookout, or 33 and 53 after the D1 and D9 corrections.

### 1.2 Time

The human times are re-based to the same start as ours: the first frame of control on the Dock. Ours run from `segment_start` to the goal frame. "Adjusted" removes the time that the swordfight-only detours add over the direct route. "Crew-free" also removes the crew-only chicken (and Credit Early in the WR), so it matches our segment exactly. The arithmetic is in §4.

| run | time | ticks | ours (held-out mean) minus this run |
|---|---:|---:|---:|
| **ours, held-out mean, seeds 31-60** (stdev 48.3, range 22262-22400) | **6:11.53** | **22292.0** | |
| ours, seed 1, headless (`out/runs/20261005T122038Z-run`) | 6:13.03 | 22382 | |
| ours, seed 1, visible demo with fast boot (`out/runs/20261005T122053Z-demo`); end record and `state-end.json` byte-identical to headless | 6:13.03 | 22382 | |
| IL raw: dock control → T-shirt (logo glitch on; swordfight and crew detours included) | 6:29 | 389 s | −17.5 s |
| **IL adjusted** (swordfight removed, ±5 s) | **5:31** | 331 s | **+40.5 s (+12.2%)** |
| IL crew-free | 5:22 | 322 s | +49.5 s (+15.4%) |
| WR raw: dock control → dig (logo glitch on; swordfight, insult-fight and crew detours included) | 7:00 | 420 s | −48.5 s |
| **WR adjusted** (±5 s, plus 0-3 s at the end) | **5:29** | 329 s | **+42.5 s (+12.9%)** |
| WR crew-free | 5:14 | 314 s | +57.5 s (+18.3%) |

All deltas use the held-out mean, 371.53 s. Seed 1 (373.03 s) is 1.50 s slower, so its deltas are 1.50 s larger: +42.0 s against IL adjusted (+12.7%), +44.0 s against WR adjusted (+13.4%), +51.0 s against IL crew-free (+15.8%) and +59.0 s against WR crew-free (+18.8%). On seed 1 the storekeeper was away, which costs 120 ticks (2.0 s) against a run where he is present (D7).

### 1.3 Our progression

| stage | plan | settings | ticks | time | source |
|---|---|---|---:|---:|---|
| unskipped | unit-cost plan (66 actions) | talk speed 60, no skips, bridge v1 | 107,660 (seed 1); mean of seeds 1-5: 107,264 | 29:54.33 | Appendix |
| maximum talk speed | the same era's compiled plan | talk speed 255, no skips | 57,098 (seed 1) | 15:51.63 | `docs/plan.md` Task 8.2 "Results" |
| with skips | the same plan | `.` and Esc on, bridge v1 | 23,888 (seed 1) | 6:38.13 | `docs/plan.md` Task 8.2 "Results" |
| with skips, bridge v2 | `unit` candidate (66 actions) | with input deferral | 23,988.2 (mean, seeds 1-30) | 6:39.80 | `docs/optimization.md` Candidates |
| time-optimal, first run | `sample-02` (67 actions) | the same | 22,416.2 (held-out mean); 22,395.6 (select mean); 22,388 (seed 1) | 6:13.60 | `out/optimize/20261005T084713Z/report.json` |
| **time-optimal, final** | `sample-16` (67 actions): path 686 at the Fork (D12), the meat drugged at the petal screen (D6) | the same | **22,292.0** (held-out mean); 22,291.4 (select mean); 22,382 (seed 1) | **6:11.53** | `docs/optimization.md` |

Against the `unit` candidate, the final plan's select mean differs by −1696.8 ticks (22291.4 against 23988.2). The main parts are about −1532 for provoking the cook from bar-left (D2), −24 for two pickups instead of one sentence (D3), −30 for Talk to Otis instead of the refused give (#26) and −108 for the two hops through path 686 (D12).

Against the first time-optimal plan, paired on the same held-out seeds 31 to 60, the final plan is 124.2 ± 14.0 ticks per seed faster (standard error); it was faster on 27 seeds and slower on 3. The two hops through 686 give −108 on every seed. The pay step gives −16.0 on the mean, because the storekeeper was away on 6 of the seeds instead of 10, and every other step together gives −0.2. These figures come from the per-seed traces in `out/optimize/20261005T084713Z/candidates/sample-02/report` and `out/optimize/20261005T111318Z/candidates/sample-16/report`.

The segment start moved from boot tick 12865 in the unskipped runs to 9325, because maximum talk speed shortens the pre-segment intro. Ticks count from the segment start, so the move does not enter any total.

### 1.4 What explains the remaining gap

We are about 40.5 to 42.5 s (12 to 13%) slower than the adjusted human runs, and about 49.5 to 57.5 s slower than the crew-free figures. The route does not account for this. The comparison finds no modelling bug (§5), and our route uses two transitions fewer than the corrected human list. The gap is pace, and it grows along the route:

| stretch (from dock control) | ours, seed 1 | IL, re-based | ratio |
|---|---:|---:|---:|
| control → start of Otis #1 | 166.4 s | 150 s (165 s less fish 6 s and chicken 9 s) | 1.11 |
| Otis #1 → Lookout → Map | 161.3 s | 134 s (138 s less about 4 s of sword) | 1.20 |
| Map → T-shirt | 45.3 s | 38 s (86 s less the 42-s detour and the 6-s pirate dodge) | 1.19 |
| of which the 9 forest hops | 35.4 s | 29.3 s | 1.21 |
| **total** | **373.0 s** | **322 s** (crew-free) | **1.16** |

These ratios are derived here from the §4 timestamps, which have about 1 s resolution. Three things explain the gap:

- **The logo speed glitch (banned for us, Banned 2).** It leaves var 19 (`VAR_TIMER_NEXT`) at 5 for the whole run, so every frame is 5 jiffies instead of 6 (`docs/research/skips-engine.md` §6). Frame-paced motion (walk steps, animation, `breakHere` loops, a skipped line's one frame) runs 6/5 as fast. `delay()` and talk timers count jiffies and keep their length.
  - The forest is the cleanest test, because both sides make the same 9 transitions. At 5/6, our 35.4 s becomes 29.5 s, against 29.3 s (IL) and about 28 s (WR). The glitch accounts for about 5.9 of the 6.1 s gap to the IL there.
  - The middle stretch runs at 1.20, about 6/5. Seed 1 had the storekeeper away, which adds 2.0 s to this stretch (D7), while the IL runner found him in; without those 2 s the ratio is 1.19.
  - The first stretch runs at only 1.11. Part of the reason is our route: we provoke the cook from bar-left (D2) and leave the bar in one sentence (D4). Part is fixed time on both sides, such as the cook's 600-jiffy wait (10 s at any frame rate).
  - If every one of our ticks were frame-paced, 5/6 would give 18,577 ticks (5:09.6), a saving of 62 s. That is only an upper bound on the glitch's effect, because fixed jiffy waits do not shrink. Even so, it is larger than the whole gap.
- **Pixel and timing tricks (banned for us, Banned 4).** These are early door entry, "shmoovement" (repeated clicks during walks), floor clicks and clicks on the nearest pixel of an exit (`docs/human-route.md` §2.3). They plausibly explain the residual 0.2 to 1.5 s in the forest after the glitch. None was measured.
- **Version.** We run the Mac release; the IL is DOS VGA floppy and the WR is DOS CD. Walk speeds, animation lengths and interface timings were not compared (§2.1), so our per-step ticks say nothing about DOS. The WR is also a CD run with a different interface (9 verbs, icon inventory).

The only known route-level saving that a human has and we do not is D7: Talk to the storekeeper when he is present. It is estimated at about 100 ticks (1.7 s) on the mean and needs a plan that can branch.

## 2. Game version and rules differences

### 2.1 Versions

| | version | interface | evidence |
|---|---|---|---|
| ours | Mac release, SCUMM v5, ScummVM gameid `monkey`, variant `Mac` | the Mac interface | every run's `scummvm.ini` (`platform=macintosh`); boot records (`"variant":"Mac"`, `"bridge":"speedrun-bridge v2"`) |
| IL (saruya 2025) | DOS VGA floppy | 12 verbs, text inventory | `docs/human-route.md` §2.4 |
| WR (saruya 2026) | DOS CD, ScummVM 2026.3.0, seed 2756848129 | 9 verbs, icon inventory | `docs/human-route.md` §2.4 |
| G19 guide | DOS CD | | `docs/human-route.md` §2.4 |

Every script claim here is about the Mac scripts, because only those are decompiled in `data/scripts/`. The DOS scripts were not checked. Nothing found here needs a Mac-specific explanation, but two items carry a version caveat. For D2, the IL's kitchen line was not identified. D9 would become a version difference if DOS started the idol-room cutscene on Open alone. The IL's mansion items match the Mac scripts: the staple remover, the manual and the lips after the idol room (D9).

Ticks do not transfer across versions. On Mac, ScummVM keeps a 240 Hz timer, so one tick is 1/60 s (`docs/research/engine-bridge.md` §2). The DOS timer branches do not apply. No human Mac runs were found (`docs/human-route.md` §2.4).

### 2.2 Rules differences

| item | human runs | ours | rule | effect on the comparison |
|---|---|---|---|---|
| logo speed glitch | Esc at the first sparkle of the Lucasfilm logo; faster walking and animation for the whole run | never; skips start only after `segment_start`, which asserts var 19 = 6 | Banned 2 | the largest single cause of the time gap (§1.4) |
| text skip `.` and cutscene skip Esc | everywhere | after `segment_start`, on the first frame each has an effect | Allowed 5 | the same on both sides; the old report's main caveat is gone |
| talk speed | not stated (`.` makes it matter little) | maximum (255, var 37 = 0) | Allowed 6 | none |
| pixel input: floor clicks, actor clicks, early door entry, shmoovement | used | never; every input is a sentence, a verb-slot click or a visible dialogue choice | Banned 4 | D5 (costs nothing), D2 and D4 (the off-screen rule stands in for floor clicks), part of the walking gap |
| save/load: Credit Early (WR) | used, crew-only | never | Banned 3 | out of scope (D11) |
| random seed | fixed ScummVM seed (WR 2756848129); the safe combination is seeded | fixed seed per run, but the route is chosen by the mean over 30 seeds and reported on 30 held-out seeds | Route selection | our headline is a mean, not a best seed |
| reacting to chance | a human sees whether the storekeeper is in and clicks him | a fixed plan with no branches; only menu interrupts (`segment.toml` `interrupts`) | none: a plan-format limit, not a rule (D7) | D7 |
| input timing | human reaction time | acts on the first frame a player's click would count; defers in frames where a player's input would be lost | `docs/plan.md` "Plan-player semantics as implemented" (not yet in the rules file) | 10 deferrals on seed 1; +62 ticks on the mean of seeds 1-10 (§6.3) |
| timing | RTA, from the logo skip to the final fade of the whole game | engine ticks from the first frame of control on the Dock to the goal frame | Timing | the human figures in §1.2 and §4 are re-based to dock control |
| swordfight | interleaved (fish, sword, troll, Smirk, insult fights) | not modelled | Out of scope for v1 | D11 |

## 3. Alignment table

These are the aligner's rows, in our plan order, with light copy-editing. The ticks are seed 1. Each row runs from the previous row's end to its own end, with the end time from the segment start in brackets.

- **Kind** is `same`, `only-human`, `only-ours`, `different-method` or `different-order`.
- **Diff** links the row to its §5 entry. Every row whose kind is not `same` has one.
- All 67 plan actions and all 74 human steps appear. The swordfight and crew steps share one out-of-scope row.

| Ours | Human | Kind | Diff | Our ticks (end time) | Human time | Note |
|---|---|---|---|---|---|---|
| - | H1 [T] Lookout → Dock | only-human | D1 | none (before our segment; `segment_start` at boot tick 9325) | IL 0:02-0:03 (control already on the Dock after the intro Esc); WR 0:05 | Scripted on every boot: `room-038-lookout/local-203.txt [02CB]` startObject(486,11) → `room-096-part1/local-200.txt [003B]` loadRoomWithEgo(426,33). Our first input is #1. The human's intro Esc also triggers the banned logo speed glitch. Skips are allowed only after the segment start (Allowed 5, Banned 2). |
| #1 open-bar-door | H2 [A] Open door (SCUMM Bar) | same | - | 354 (0:05.90) | IL 0:12-0:15 | Open 428 (`room-033-dock/obj-0428-door.txt [0015]`). Its sentence walk leaves ego at the door. |
| #2 walk-into-bar | H3 [T] Dock → Bar main room | same | - | 6 (0:06.00) | IL 0:18 | Walk to 428 → room 28. One frame, because #1 already walked ego to the door. Entry from the Dock closes 316 and starts the cook's timer (`room-028-bar/local-205.txt [0040]`/`[0044]`). |
| #3 walk-to-kitchen-door-provoking-cook | H4 [T] Bar main room → Bar back room; H5 [A] Open door (kitchen) | different-method | D2 | 462 (0:13.70); 2 text skips | IL 0:18-0:21 (H4), 0:21-0:37 (H5, including the cook walking out) | Open 316 pushed from bar-left while local-211 runs, so it starts local-214 instead of opening (`room-028-bar/obj-0316-door.txt [0018]`/`[0021]`). local-214 plays the cook's line, then `startScript(212)` at `[004B]`; local-212 waits 600 jiffies, then starts 216. The walk crosses x 320 and local-201 `[0012]` pans right, so 316 comes on screen (§6.4). The human's curtain walk H4 is folded into the Open. Compiled `until`: the cook is not in room 28 (local-203 `[003A]` to `[004A]`). |
| #4 walk-into-kitchen-after-provoking-cook | H6 [T] Bar back room → Kitchen | same | - | 864 = 822 cook wait + 42 walk-in (0:28.10) | IL 0:38 | Walk to 316, pushed once the cook is in room 28 at x ≤ 310 and 316 is open (local-203 `[001E]`; local-216 `[001B]` opens 316/570). The `until` wait is folded into this step, as in `docs/optimization.md`. Bar entry → kitchen: ours 22.1 s, IL about 20 s (0:18 → 0:38). |
| - | OUT OF SCOPE, listed once. Swordfight only: H7 open pier door; H8 plank and seagull; H9 pick up fish; H44 pick up sword, plus the sword picks inside H45; the H71 detour, sub-steps 1-7 (Map → Bridge, talk to troll, give fish, Bridge → Map, Map → House, Smirk and pay 30, House → Map); the IL pirate dodge on the map; the WR insult fight. Crew only: H32 open Voodoo door; H33 Low street → Voodoo shop; H34 pick up chicken; H35 open door; H36 Voodoo shop → Low street; Credit Early safe pulls in H45 (WR only). | only-human | D11 | none (not in our segment) | IL: fish 0:39-0:45; chicken 2:27-2:36; sword 3:03-3:04 plus picks in 3:05-3:13; troll and Smirk 5:06-5:48; pirate dodge 5:48-5:56. WR: Credit Early 3:00; troll 5:15-5:25; insult fight 5:55-6:30. | None of these steps feeds `Bit[85]` or `Bit[86]`. Both routes leave them out of the counts (`docs/human-route.md` §4.3), and they are not in our model (`docs/part1/model.md` §2, §5; `docs/part1/money.md` B4). They add 10 actions and 6 transitions to the human run as played. |
| #5 pick-up-meat | H11 [A] Pick up hunk of meat | different-order | D3 | 60 (0:29.10) | IL 0:46 | Pick up 566 (`room-041-kitchen/obj-0566-hunk-of-meat.txt [0041]`). The meat comes first because it must be displayed before the pot for the circus helmet (D5). |
| #6 pick-up-pot-not-first-meat | H10 [A] Pick up pot | different-order | D3 | 60 (0:30.10) | IL 0:45 | Pick up 567 (`room-041-kitchen/obj-0567-pot.txt [002A]`); records `pot-guarded-by meat`. The human's pot-first order is our dead end `pick-up-pot-first`. |
| #7 walk kitchen bar-right | H12 [T] Kitchen → Bar back room | same | - | 48 (0:30.90) | IL 0:47-0:48 | Walk to 570, with no defensive Open (`docs/part1/model.md` §6). |
| #8 walk-out-of-bar-from-right-meanwhile | H13 [T] Bar back room → Bar main room; H14 [T] Bar main room → Dock (Esc on the LeChuck cutscene) | different-method | D4 | 456 (0:38.50); 1 cutscene skip | IL 0:48-0:51, 0:51-0:54 | One Walk to 315 from the right half (`room-028-bar/obj-0315-door.txt [0072]` to `[008A]`). The first exit plays global 120, the LeChuck "Meanwhile" cutscene (`global/script-120.txt [0538]`). We now skip it with Esc, as the human does. |
| #9 walk dock lookout | H15 [T] Dock → Lookout | same | - | 1008 (0:55.30) | IL 1:00-1:06 | Walk to cliffside 426. |
| #10 walk lookout melee-map | H16 [T] Lookout → Island map | same | - | 120 (0:57.30) | IL 1:06-1:09 | Walk to path 487; map entry 1. |
| #11 walk melee-map clearing | H17 [T] Map → Clearing | same | - | 432 (1:04.50) | IL 1:12-1:15 | Walk to 912. |
| #12 walk-into-tent-with-pot | H18 [T] Clearing → Circus tent; H19 auto-conversation (Dia 1, 1, 2) | same | - | 848 = 204 (first Walk to 621) + 644 (second push and the menus) (1:18.63); 16 text + 3 cutscene skips | IL 1:18-1:21; options visible at 1:24 | Choices "ahem", "I'll do it" and "Of course" are the human's Dia 1, 1, 2. Room 52's local-202 STOPs the first walk (`[0000]` to `[001C]`), so this one action compiles to two steps: 67 actions become 68 compiled steps. |
| #13 walk-out-of-tent-after-helmet-meat | H20 [A] Give pot to Fettucini brothers; H21 [T] Circus tent → Clearing | different-method | D5 | 1062 (1:36.33); 8 text + 3 cutscene skips; 2 verb-slot clicks | IL 1:27 (give), 1:39-1:42 (out), 478 in the inventory at 1:42; WR 1:25 (give) | Click Use, then the slot before the pot (`room-051-circus-te/local-200.txt [0107]` → `[0021]`, `[008B]`), then choose "nibboB". The human clicks Give pot on a brother, which resolves the brother with `actorFromPos`: pixel input. The walk-out is scripted (`local-207.txt [114D]`); both routes count it as 1 transition. |
| #14 walk clearing melee-map | H22 [T] Clearing → Map | same | - | 576 (1:45.93) | IL 1:45-1:48 | Walk to 622; map entry 2. |
| #15 walk melee-map f218 | H23 [T] Map → Fork | same | - | 210 (1:49.43) | IL 1:48-1:51 | The Fork is pseudo-room 218. |
| #16 walk-f218-f215-via-686 | H24 [T] Fork → Petal screen | different-method | D12 | 162 (1:52.13) | IL 1:51-1:54 | Walk to 686, the second "back" path at the Fork. Any verb on it runs 685's exit (`room-058-damnfores/obj-0686-path.txt [0010]`), so the transition and the arrival are 685's. Only the walk to the object differs: the same hop through 685 takes 216. |
| #17 pick-up-petal | H25 [A] Pick up plants (yellow petal) | same | - | 78 (1:53.43) | IL 1:54 | Pick up 678 at room 215; the plants give petal 689 (`room-058-damnfores/obj-0678-plants.txt [0086]` to `[0092]`). The meat is drugged right after it, at #18 (D6). |
| #18 drug-meat-with-petal | H29 [A] Use yellow petal with hunk of meat | different-order | D6 | 6 (1:53.53) | IL 2:06 (on the Dock, after H28) | Use 566 with 689 at the petal screen (room 215), not on the Dock. Inventory-only, one frame: obj-0566 `[007E]` setClass(566,[134]); the petal forwards doSentence(7,566,689) (`room-058-damnfores/obj-0689-yellow-petal.txt [0049]`). |
| #19 walk f215 f218 | H26 [T] Petal screen → Fork | same | - | 102 (1:55.23) | IL 1:57-1:58 | Walk to 687. |
| #20 walk f218 melee-map | H27 [T] Fork → Map | same | - | 234 (1:59.13) | IL 1:59-2:00 | Map entry 3. |
| #21 walk melee-map dock | H28 [T] Map → Village (lands on the Dock) | same | - | 714 (2:11.03) | IL 2:00-2:03 | Walk to 917 → loadRoomWithEgo(426,33). The human drugs the meat next (H29); we did it at #18. |
| #22 walk dock low-street | H30 [T] Dock → Low street | same | - | 930 (2:26.53) | IL 2:09-2:21 | Walk to archway 427. |
| #23 buy-map | H31 [A] Talk to Citizen of Mêlée (Dia 4, Esc, 2) | same | - | 198 (2:29.83); 6 text + 1 cutscene skip | IL 2:23 (talk), 2:27 (map bought, 378 left) | Talk to 441, choices "barber" and "swell gift". The human's chicken detour (H32-H36) follows here and is out of scope. |
| #24 walk low-street high-street-town | H37 [T] Low street → High street | same | - | 516 (2:38.43) | IL 2:36-2:42 | Walk to archway 451. |
| #25 walk high-street-town jail | H38 [T] High street → Jail | same | - | 480 (2:46.43); 2 text skips | IL 2:45-2:47 | Walk to doorway 434. |
| #26 talk-to-prisoner | H39 [A] Talk to prisoner (Otis #1) | same | N1 | 204 (2:49.83); 1 cutscene skip | IL 2:48; WR 2:45 | Talk to 405 sets `Bit[420]` (`room-031-jail/obj-0405-prisoner.txt [001A]`). This is the human's method. With skips it beats the refused give (`give-meat-to-prisoner`, 234) that the unit-era §14 plan used. Jail before store is also the human's order, and it is forced (N1). |
| #27 walk jail high-street-town | H40 [T] Jail → High street | same | - | 174 (2:52.73) | IL 2:50 | Walk to doorway 400. |
| #28 open-store-door | H41 [A] Open door (store) | same | - | 348 (2:58.53); 1 text skip | IL 2:53-2:56 | Open 437. The walk in follows at once (`docs/part1/model.md` §6). |
| #29 walk-into-store | H42 [T] High street → Store | same | - | 6 (2:58.63) | IL 2:57 | Walk to 437. In the WR, Credit Early happens here (out of scope). |
| #30 pick-up-shovel | H43 [A] Pick up shovel | same | - | 396 (3:05.23) | IL 3:00-3:02 | Pick up 396, unpaid. The human's sword pickup (H44, IL 3:03-3:04) is out of scope. |
| #31 pay-for-shovel-and-mints | H45 [A] Talk to storekeeper (the mint and shovel part) | different-method | D7 | 828 (3:19.03); 11 text + 3 cutscene skips | IL 3:05-3:13 (6 picks, including the sword); WR Credit Early at 3:00 | Walk to 387 with the shovel unpaid → `room-030-store/local-204.txt [042B]` startScript(211). Picks "shovel", "I want it", "breath mint" (3); the menu then closes itself (`local-211.txt [03EB]`). The storekeeper was away on seed 1; with him present the step takes 708 (D7). |
| #32 walk-out-of-store | H46 [T] Store → High street | same | - | 138 (3:21.33) | IL 3:15-3:16 | A second Walk to 387, now that the shovel is paid. |
| #33 walk high-street-town high-street-mansion | H47 [T] High street → Trail | same | - | 594 (3:31.23); 1 text skip | IL 3:17-3:21 | Walk to archway 436. |
| #34 walk high-street-mansion mansion | H48 [T] Trail → Mansion exterior | same | - | 390 (3:37.73) | IL 3:24-3:30 | Walk to 431. |
| #35 give-meat-to-poodles | H49 [A] Use meat with condiment with poodles | different-method | D8 | 636 (3:48.33) | IL 3:30-3:38 | Give 566 to 467 runs the same verb-80 handler (`room-036-mansion-e/local-201.txt [0087]` sets `Bit[15]`). |
| #36 open-mansion-door | H50 [A] Open door (mansion front door) | same | - | 204 (3:51.73) | IL 3:39 | Open 465. |
| #37 walk-into-foyer | H51 [T] Mansion exterior → Mansion interior | same | - | 6 (3:51.83) | IL 3:42 | Walk to 465 → room 53. |
| #38 open-idol-room-door | H52 [A] Open door (foyer, right) | same | D9 (with #39) | 108 (3:53.63) | IL 3:42-3:45 | Open 632 only opens the door (`room-053-foyer/obj-0632-door.txt [0028]` → global 25). |
| #39 enter-idol-room | - | only-ours | D9 | 60 (3:54.63); 2 text + 1 cutscene skip | within IL 3:42-3:45 (the human's click on the open door is inferred) | Walk to the open 632 starts local-210 (obj-0632 `[0024]`), which gives the repellent, the manual, the lips and the staple remover. The cutscene is skipped with Esc. |
| #40 walk-foyer-to-mansion | H53 [T] Mansion interior → Mansion exterior | same | - | 546 (4:03.73) | IL 3:48-3:52 | Walk to 633. |
| #41 walk mansion high-street-mansion | H54 [T] Mansion exterior → Trail | same | - | 294 (4:08.63) | IL 3:51-3:57 | Walk to 466. |
| #42 walk high-street-mansion high-street-town | H55 [T] Trail → High street | same | - | 642 (4:19.33) | IL 4:00-4:02 | Walk to 435. |
| #43 walk high-street-town jail | H56 [T] High street → Jail | same | - | 144 (4:21.73) | IL 4:05-4:07 | Walk to 434. |
| #44 give-mints-to-prisoner | H57 [A] Give breath mints to prisoner (Dia 2) | same | - | 306 (4:26.83); 2 text + 2 cutscene skips | IL 4:07 | Choice "stiff upper lip" is the human's Dia 2. |
| #45 give-repellent-to-prisoner | H58 [A] Give gopher repellent to prisoner | same | - | 156 (4:29.43); 4 text skips | IL 4:12-4:14 | Otis gives the cake (`room-031-jail/local-203.txt [01E0]`). |
| #46 walk jail high-street-town | H60 [T] Jail → High street | same | (D10) | 174 (4:32.33) | IL 4:16-4:17 | We leave the jail before opening the cake. |
| #47 open-cake | H59 [A] Open cake (in the jail) | different-order | D10 | 6 (4:32.43) | IL 4:15 | Open 420 on High Street, after the jail (`room-031-jail/obj-0420-cake.txt [0056]`/`[005F]`). Inventory-only, one frame. |
| #48 walk high-street-town high-street-mansion | H61 [T] High street → Trail | same | - | 354 (4:38.33) | IL 4:17-4:18 | Walk to 436. |
| #49 walk high-street-mansion mansion | H62 [T] Trail → Mansion exterior | same | - | 390 (4:44.83) | IL 4:24-4:27 | Walk to 431; the poodles are still asleep. |
| #50 walk-into-foyer | H63 [T] Mansion exterior → Mansion interior (no Open) | same | - | 276 (4:49.43) | IL 4:28-4:30 | 465 is still open from #36. |
| #51 steal-idol | H64 [A] Walk to gaping hole | same | - | 726 (5:01.53); 8 text + 4 cutscene skips | IL 4:32 (walk to), 4:35 (idol in the inventory), 4:39 (Fester) | Walk to 637, with 1 pick, "could have it" (the human picks 1, "any"). The Uh/Um/Blfft menus lie inside the Esc-skipped override region (`global/script-119.txt [0000]` → `[08A1]`), so they never show. |
| #52 walk-past-fester-to-underwater | H65 [A] Open door (leave; Fester); H66 [S] Mansion interior → Pier → Underwater | same | - | 264 (5:05.93); 3 cutscene skips | IL 4:42, 4:45-4:46 | Open 633 while owning 635. The "Buzz off" menu lies inside the skipped override region (`room-053-foyer/local-217.txt [000F]` → `[034B]`). Both sides count the scripted 53 → 83 → 42 chain as 0 transitions. |
| #53 walk-up-ladder-taking-idol | H67 [A] Pick up idol; H68 [S] Underwater → Pier | same | - | 378 (5:12.23); 1 cutscene skip | IL 4:46-4:47 (idol), 4:52-4:53 (climb out) | Pick up 578. `Bit[85]` is set at `room-042-underwate/local-200.txt [0041]` → `global/script-071.txt [008D]`. The Elaine scene is skipped with Esc. The idol comes before the treasure, as in both human runs; the idol-last variant was available and not chosen. |
| #54 walk cu-dock dock | H69 [T] Pier → Dock | same | - | 168 (5:15.03) | IL 4:53-4:55 | Walk to 904 in room 83. |
| #55 walk dock lookout | H70 [T] Dock → Lookout | same | - | 642 (5:25.73) | IL 4:56-5:02 | Walk to 426. |
| #56 walk lookout melee-map | H71 [T] Lookout → Map | same | - | 120 (5:27.73) | IL 5:03-5:06 | Map entry 4, so wandering pirates are possible (`room-085-melee/entry.txt [009B]`). None came on seed 1: the trace has no interrupt records. The human swordfight detour follows here (out-of-scope row). |
| #57 walk melee-map f218 | H72 [T] Map → Fork | same | - | 234 (5:31.63) | IL 5:48-5:57 (after the detour, including a pirate dodge); WR 6:32-6:33 (after the insult fight) | Walk to 911. |
| #58 walk-f218-f215-via-686 | H73 move 1: Back (Fork → Petal screen) | different-method | D12 | 162 (5:34.33) | IL cut at 5:59.2; WR dance 6:33-7:01 | 686, the second back path, as at #16. |
| #59 walk-forest-gate-215-220 | H73 move 2: Left | same | - | 276 (5:38.93) | IL cut at 6:02.9 | 688 Left: the map gate. It checks for the treasure map (`room-058-damnfores/obj-0688-path.txt [004F]`), sets `Bit[401]` (`[0066]`) and loads room 220 (`[006B]`). |
| #60 walk f220 f213 | H73 move 3: Right | same | - | 276 (5:43.53) | IL cut at 6:06.9 | 687 Right. |
| #61 walk f213 f212 | H73 move 4: Left | same | - | 276 (5:48.13) | IL cut at 6:10.5 | 688 Left. |
| #62 walk f212 f204 | H73 move 5: Right | same | - | 300 (5:53.13) | IL cut at 6:14.0 | 687 Right. |
| #63 walk f204 f211 | H73 move 6: Back | same | - | 240 (5:57.13) | IL cut at 6:18.1 (the black frame at 6:16.3 is not a room change) | 685 Back. |
| #64 walk f211 f216 | H73 move 7: Right | same | - | 228 (6:00.93) | IL cut at 6:21.2 | 687 Right. |
| #65 walk f216 f201 | H73 move 8: Left | same | - | 168 (6:03.73) | IL cut at 6:23.2 | 688 Left. |
| #66 walk f201 treasure-site | H73 move 9: Back (→ X clearing) | same | - | 198 (6:07.03) | IL cut at 6:26.3; WR about 7:01 | 685 Back → room 64. Forest total: ours 2124 ticks = 35.4 s (5:31.63 → 6:07.03); IL 29.3 s (5:57 → 6:26.3); WR about 28 s. |
| #67 dig-treasure | H74 [A] Walk right, use shovel with X (T-shirt) | same | - | 360, up to the goal frame (6:13.03 = 22382 ticks); 1 cutscene skip | IL 6:29 (use), 6:32 (T-shirt); WR 7:05 | Use 396 with 749. `Bit[86]` is set at `room-064-treasure/local-200.txt [0214]` → `global/script-071.txt [008D]`. |

## 4. Segment timing

### 4.1 Human segment times

The sources are the video timestamps in `docs/human-route.md` §3.2. Only differences between timestamps are used, so the IL's on-screen timer, which runs about 4 s behind the video, cancels out. Timestamps have 1 s resolution.

| | raw | adjusted (swordfight removed) | crew-free (our exact segment) |
|---|---|---|---|
| IL | **6:29 (389 s)**: dock control at about IL 0:03 → T-shirt in the inventory at about IL 6:32. Includes the swordfight and crew detours. | **about 5:31 (331 s, ±5 s)** = 389 − 6 (fish) − 4 (sword) − 42 (troll and Smirk) − 6 (pirate dodge) | **about 5:22 (322 s)**: also without the chicken (about 9 s) |
| WR | **7:00 (420 s)**: dock control at about WR 0:05 → dig at about WR 7:05. Add 0-3 s if 7:05 is the dig click rather than the T-shirt. Includes the swordfight, insult-fight and crew detours. | **about 5:29 (329 s, ±5 s, plus 0-3 s at the end)** = 420 − 81 (detour block) − 6 (fish, IL proxy) − 4 (sword, IL proxy) | **about 5:14 (314 s)**: also without the chicken (about 9 s, IL proxy) and Credit Early (about 6 s, inferred) |

Both human runs use the logo speed glitch throughout, so none of these times is glitchless.

### 4.2 IL arithmetic

Each swordfight-only block is counted as the time it adds over the direct route.

- **(a) Fish, H7 to H9.** From the pier door at 0:39 to the pot pickup at 0:45: about 6 s. It is ±1 s, because it includes walking back to the table.
- **(b) Sword.** The H44 pickup at 3:03 to 3:04 (about 1 s), plus 3 of the 6 store picks between 3:05 and 3:13 (about 1.3 s each). Our mint and shovel menu needs only 3 picks (D7). Total about 4 s; this is an estimate (±2 s).
- **(c) Troll and Smirk, H71 sub-steps 1 to 7.** 5:06 → 5:48 = 42 s.
- **(d) Pirate dodge.** Map → Fork took 5:48 → 5:57 = 9 s with the dodge. The IL's own direct Map → Fork (H23, 1:48 to 1:51) took 3 s, so the dodge adds 6 s. Subtracting the whole 5:48-5:56 window instead would give 8 s, so the sensitivity is ±2 s.
- **Swordfight total:** 58 s, so 389 − 58 = **331 s**.
- **Cross-check:** [0:03 → 5:06] 303 s + 3 s direct Map → Fork + [5:57 → 6:32] 35 s = 341 s; 341 − 6 − 4 = 331 s.
- **Crew only: the chicken, H32 to H36.** From 2:27 (map bought) to 2:36 (out of the Voodoo shop): about 9 s. This is an upper bound, because the walk to the archway then starts from a different spot. 331 − 9 = **322 s**.

### 4.3 WR arithmetic

The doc has fewer WR timestamps, so more of this is inferred.

- **Pre-detour pace matches the IL.** The WR reaches the troll between 5:15 and 5:25 (IL 5:12 to 5:24). Measured from control, both reach the troll at about 5:09 to 5:10.
- **Detour start.** The WR's map arrival is estimated at 5:09: the troll at 5:15 less the IL's 6-s Map → Bridge walk (5:06 → 5:12).
- **Detour block.** 5:09 → Fork at 6:33 = 84 s. It holds troll and Smirk (about 42 s), the pirate interception, the insult fight (5:55 to 6:30, 35 s) and the return to the map. Replaced by a 3-s direct Map → Fork walk, it removes 81 s.
- Fish (6 s) and sword (4 s) are IL proxies; neither is timestamped in the WR.
- 420 − 81 − 6 − 4 = **329 s**.
- **Crew.** The chicken is an IL proxy (9 s). Credit Early (WR 3:00) has no duration in the doc, so it is inferred at about 6 s. Measured from control, the WR is 5 s ahead of the IL at Otis #1 (2:40 against 2:45) and 1 s behind at the troll (5:10 against 5:09). That is a 6-s loss across the stretch that contains Credit Early. IL-CE's 2:58-3:08 window for the savestate and the safe gives an upper bound of about 10 s. 329 − 9 − 6 = **314 s**.
- **Proxies and inferences:** the detour start (5:09), fish, sword, chicken and Credit Early. The band is about ±5 s.

### 4.4 Against ours

- **Headline deltas** (held-out mean, 6:11.53): +40.5 s against IL adjusted (+12.2%) and +42.5 s against WR adjusted (+12.9%). Against the crew-free figures: +49.5 s (IL) and +57.5 s (WR). Seed 1 (6:13.03) is 1.50 s more in each case.
- **Pace anchors**, ours against the IL, both measured from control:

| anchor | ours (seed 1) | IL |
|---|---:|---:|
| kitchen entry | 0:28.1 | 0:35 |
| tent exit | 1:36.3 | 1:39 |
| start of Otis #1 | 2:46.4 | 2:45 (the IL still carries about 15 s of fish and chicken here) |
| gaping hole | 4:49.4 | 4:29 |
| Lookout → Map | 5:27.7 | 5:03 |

- We gain early from the route: the provoke from bar-left, the one-sentence bar exit, and no fish or chicken. From Otis #1 on we lose to slower walking. §1.4 gives the ratios by stretch.
- The cleanest pure-walking comparison is the 9 forest hops, the same 9 transitions on both sides (our first one goes through path 686, D12). Ours take 2124 ticks = 35.4 s (#58 to #66), the IL 29.3 s (5:57 → 6:26.3) and the WR about 28 s (6:33 → 7:01). The humans walk about 1.21 to 1.26 times faster. The dig is similar on both sides: ours 6.0 s (#67), the IL about 5.7 s (6:26.3 → 6:32).

### 4.5 Our numbers

Seed 1 is `out/runs/20261005T122038Z-run/trace.jsonl`, read with `speedrun.trace.load_trace`.

- **Row convention.** Each row runs from the previous step's end to this step's end, so the rows sum to the segment total. Until-waits are folded into the step that follows: #4 = 822 + 42, matching `docs/optimization.md`. `docs/part1/model.md` §14.1 describes the opposite convention.
- **#12 merges its two pushes** (204 + 644), so 67 actions compile to 68 steps. The 67 rows sum to 22382.
- **#13's 1062 includes a 6-tick deferral.** The trace step itself lasts 1056 ticks (14049 → 15105). It is preceded by a 6-tick `esc_frame` deferral after #12.
- The run has 63 text skips, 24 cutscene skips, 11 dialogue choices, 2 verb-slot clicks and 10 input deferrals (6 `esc_frame`, 4 `clicks_cleared`). There is no map-pirate interrupt.
- **Demo parity.** `out/runs/20261005T122053Z-demo` (boot record `"fast": false, "fast_boot": true`) has the same goal record (tick 31707, 22382 ticks) and the same end record (`vars_fnv1a` `190be7cb`, `audio_frames` 11652096). Its `state-end.json` is byte-identical to the headless run's.
- **Held-out spread.** Over seeds 31 to 60 the winner has stdev 48.3 and a range of 22262 to 22400 (`docs/optimization.md` "Held-out seeds"). Most of the spread is the storekeeper: the pay step takes 708 ticks when he is present and 828 when he is away (D7). He was away on 6 of the 30 seeds, and those 6 have the highest totals (22382 to 22400). The other 24 lie between 22262 and 22298.

## 5. Differences

**Classification.** Two binary classes are allowed (`docs/plan.md` Task 6.2):

- a **modelling bug**: the model is missing a precondition, or allows something the scripts forbid;
- a **genuine shortcut**: the scripts permit what we do.

"Genuine shortcut" means "legal, and not a bug". It does not by itself mean that we save anything; the finer category in parentheses and the delta say whether we do. One item, D7, is a legal method that is slower than the human's.

**Review.** Each item was classified, then checked by two independent skeptics.

- Where both skeptics refuted a classification, their better classification is adopted. That happened once: D7, which the classifier called a rules difference.
- One item is split 1-1: D4. Its label is kept, and the split is stated there. It raises an open rules question (§6.4).
- Where skeptics upheld a label but corrected supporting claims, the corrections are applied and noted.
- D12 came with the final route and has not been through this review.

| ID | rows | human-comparable delta (actions, transitions) | ticks against the human method | classification | skeptics |
|---|---|---|---|---|---|
| D1 | H1 | 0, 0 against 47 (−1 transition against the listed 48) | 0 | genuine shortcut (counting convention) | 2 of 2 upheld |
| D2 | #3 + #4 against H4 + H5 + H6 | 0, −1 | at least −18 against the bot's two-sentence form, plus one input; a human's pan wait (unmeasured) would add more; −1532 against the old unit plan | genuine shortcut (camera rule; different method) | 2 of 2 upheld |
| D3 | #5 + #6 against H10 + H11 | 0, 0 | 0 (order only); −24 against the old one-sentence pickup | genuine shortcut (different order, forced by the helmet quirk) | 2 of 2 upheld |
| D4 | #8 against H13 + H14 | 0, −1 | about −180 (estimate) plus one input | genuine shortcut (off-screen exit) | **split 1-1** |
| D5 | #13 against H20 + H21 | 0, 0 | 0 (one uncounted click fewer) | genuine shortcut (different method; the human's method is banned, and the ban costs nothing) | 2 of 2 upheld |
| D6 | #18 against H29 | 0, 0 | 0 (tie) | genuine shortcut (different order, tie) | 2 of 2 upheld |
| D7 | #31 against H45 | 0, 0 | about +100 on the mean (estimate, unmeasured) | genuine shortcut (fixed-plan limitation: replay feasibility) | **2 of 2 refuted "rules difference"; their label adopted** |
| D8 | #35 against H49 | 0, 0 | 0 (exact tie, inferred) | genuine shortcut (equivalent verb, exact tie) | 2 of 2 upheld |
| D9 | #38 + #39 against H52 | +1, 0 against the listed 22 (0, 0 against the corrected 23) | 0 | genuine shortcut (counting convention) | 2 of 2 upheld, with corrections |
| D10 | #46 + #47 against H59 + H60 | 0, 0 | 0 (tie) | genuine shortcut (different order, tie) | 2 of 2 upheld |
| D11 | the out-of-scope row | 0 (the human run as played has +10 actions, +6 transitions) | removed from the human times (§4) | genuine shortcut (out of scope) | 2 of 2 upheld, with corrections |
| D12 | #16 + #58 against H24 + H73 move 1 | 0, 0 | −54 per hop against 685, −108 per run (measured); which path the human clicks was not checked | genuine shortcut (equivalent exit with a closer walk point, found by blind extraction) | not reviewed |
| N1 | #25-#32 against H38-H46 | none | 0 | not a difference: genuine shortcut (agreement) | 2 of 2 upheld, with corrections |

**Modelling bugs: 0.**

### D1. Lookout → Dock (H1) is a scripted transition

- **Classification: genuine shortcut (counting convention).** Unchanged from the old D1.
- **Delta:** 0 ticks and 0 actions. Transitions: −1 against the listed 48, 0 against the corrected 47.
- **Ours:** no input. The segment starts at the first free control on the Dock (`pddl/part1/segment.toml`). Our first input is #1 `open-bar-door`.
- **Human:** H1 "[T] Lookout → Dock", counted in "48 from the Lookout". The intro Esc passes it (IL 0:02 to 0:03, WR 0:05).

**Explanation.** Both routes make the same inputs here: none. On a natural boot the player never has a frame of control at the Lookout:

1. On the first visit, room-038 local-200 sets `Bit[116]` and starts the intro cutscene local-203 (`[0042]` to `[004F]`).
2. local-203 walks ego to the stairs (`[02B6]`) and ends the cutscene at `[02CA]`. With no `breakHere` in between, `[02CB]` runs `startObject(486,11)`.
3. The stairs script sees `!Bit[395]` (`[004A]`), sets it (`[004F]`) and loads room 96, the "Part One" card (`[0054]`).
4. Room-096 local-200 turns input off (`[0000]`). Its `[0037]` UserputOn is followed in the same frame by `[003B]` `loadRoomWithEgo(426,33,346,133)`, so the first control is on the Dock.

Esc does not change the chain. The override targets (local-203 `[02BF]`, room-096 `[0033]`) both come before the move, so skipped and unskipped boots run the same 38 → 96 → 33 sequence (`docs/part1/rooms.md` T01/T02; `docs/part1/start.md` TL;DR).

The human list's own rule settles the count. `docs/human-route.md` §3.1 marks a transition the game makes during a cutscene as [S], "listed but not counted". So H1 should be [S], and the comparable human figure is 47 whether or not the intro Esc is pressed. The §3.2 "Start" premise is wrong for the Mac scripts. Both DOS videos are Esc'd boots and start with control on the Dock, so no version difference arises.

**Trace (seed 1).** A `userput` stall in room 38 at tick 7201 shows input was off at the Lookout. `segment_start` is at tick 9325 in room 33. The first skip record is at tick 10123, inside #3. So the bot pressed no key before the segment start (Allowed 5, Banned 2).

The human's intro Esc, and with it the logo speed glitch, is a separate, run-long rules difference (§2.2). It is not part of D1.

**Citations:**

- `data/scripts/room-038-lookout/local-200.txt [0042]` to `[004F]`
- `data/scripts/room-038-lookout/local-203.txt [0000]`, `[0005]`/`[0007]` (override → `[02BF]`), `[02B6]`, `[02CA]`, `[02CB]`
- `data/scripts/room-038-lookout/obj-0486-stairs.txt [004A]`, `[004F]`, `[0054]`
- `data/scripts/room-096-part1/local-200.txt [0000]`, `[0004]`/`[0006]` (override → `[0033]`), `[0037]`, `[003B]`
- `pddl/part1/segment.toml` (start condition); `docs/part1/rooms.md` T01, T02; `docs/part1/start.md` TL;DR
- `docs/human-route.md` §3.1 ([S] listed but not counted), §3.2 Start and step 1, §4.2, §4.3
- `out/runs/20261005T122038Z-run/trace.jsonl`: stall `userput` room 38 tick 7201; `segment_start` tick 9325; first skip tick 10123

**Fix and follow-up.** No model change. The documentation follow-ups carried over from the old report are listed in §8.

### D2. Provoking the cook from the bar's left half

- **Classification: genuine shortcut (camera rule; different method).** This replaces the old D2, "no kitchen Open". The time plan now makes the human's Open on 316, but from bar-left, so the Open's own walk replaces the human's curtain walk.
- **Delta:** 0 actions and −1 transition (human-comparable). Under the naming rule, the same input reads −1 action and 0 transitions. Either way it is one input fewer.
- **Ours:** #3 pushes Open 316 from bar-left while local-211 runs. #4 then waits until the cook is in room 28 at x ≤ 310 and 316 is open, and walks in.
- **Human:** H4, the curtain walk to the back room; H5, Open door (kitchen); then about 10 s of waiting for the cook (IL 0:18 to 0:38).

**Scripts.**

- Bar entry from the Dock closes 316 and starts the cook's timer, local-211 (local-205 `[0040]`/`[0044]`). The cook is in room 0, the kitchen.
- Open 316 while 211 is running starts local-214 instead of opening (obj-0316 `[0018]`/`[0021]`). local-214 prints the cook's warning with `print(255)` at a fixed position (`[000F]`), one message of two parts joined by `wait()`, then starts local-212 (`[004B]`).
- local-212 waits 600 jiffies (`[0000]` to `[0020]`), then starts local-216. local-216 stops 211 and 212 and opens the door pair 316/570 (`[0017]`/`[0019]`/`[001B]`).
- The walk-in is a Walk to 316, which checks only that the door is open (obj-0316 `[003F]` to `[004B]`; local-218 `[0017]`).
- No script checks where ego stood when the Open was given.

**Click equivalence.** A real click on 316 goes through the bar's input script: local-202 (`[001C]` to `[0023]`) chains to local-203. With the cook out of room 28, local-203 walks ego to 316 (`[0009]`), waits (`[003A]`) and runs the door's verb directly (`[003E]` to `[004A]`). The pushed sentence reaches the same verb through the sentence script (walk at `global/script-002.txt [02DB]`/`[02E0]`, verb at `[039D]`). The step's `until` (the cook not in room 28) copies that branch. The trace confirms local-214 ran: #3's two text skips have `wait_script` 214.

**Camera rule.** 316 (x 592 to 624) is off screen from the left half (`docs/part1/rooms.md` §2.4, §7). The Open's walk crosses x 320, and local-201 `[0012]` pans the camera to 480, which brings 316 on screen. That is the case `rules/glitchless.md` "Camera visibility" allows, and the rule names the bar halves as its example. A human must first spend another input to cross: the curtain (H4). Whether 316 is actually on screen before the Open fires was not checked; see §6.4.

**Time (seed 1).**

- #3 runs from tick 9685 to 10147 (462 ticks). Then the until-wait is 822 ticks, and the walk-in takes 42. Bar entry to the kitchen is 1326 ticks (22.1 s). `docs/optimization.md` shows no variance: 462 (n = 1042) and 864 (n = 1445).
- **Against the human's method.** Curtain, then Open from the right half, then walk-in, measured in our engine: 198 (`walk bar-left bar-right`) + 282 (`provoke-cook`) + 864 = 1344, against our 462 + 864 = 1326. So the gain over the human method is one sentence and about 18 ticks, in our engine only. A human may also have to wait for the camera pan before clicking 316; that wait was not measured.
- **Against the old unit-cost plan.** The unprovoked route costs the curtain walk (198) plus `walk-into-kitchen` with the cook's own timer (2659.7 mean, n = 70, range 2118 to 3198): about 2858. The provoke saves about 1532 ticks (25.5 s) per run, the largest gain from time optimisation. It reverses the old D2: the human's Open was the faster choice all along.
- **The IL.** Bar entry to the kitchen takes about 20 s (0:18 → 0:38). `docs/human-route.md` §5 Q4 says the IL's cook came out after a "generic failure line" (0:21 to 0:27). The timing fits a provoke: a line, then the cook about 10 s later (local-212's 600 jiffies). An unprovoked cook needs at least 30 s after bar entry (local-211 `[000D]`). Because local-214 prints with talker 255 at a fixed position, the cook's line can look like narration on screen. This is an inference; the DOS scripts were not checked.

**The cook-freeze gap** (`rules/glitchless.md` "Known fidelity gap") is unchanged, but it now matters much less. Ego is already at 316, the walk-in takes 42 ticks, and the cook is walking away. local-218's cutscene then freezes him (`global/script-018.txt [0070]`). See §6.5.

**Citations:**

- `data/scripts/room-028-bar/obj-0316-door.txt [0018]`, `[0021]`, `[0027]`, `[003F]` to `[004B]`
- `data/scripts/room-028-bar/local-214.txt [000F]`, `[004B]`; `local-212.txt [0000]` to `[0020]`; `local-216.txt [0017]`, `[0019]`, `[001B]`
- `data/scripts/room-028-bar/local-211.txt [0000]`, `[000D]`; `local-205.txt [0040]`, `[0044]`; `local-201.txt [0000]` to `[0016]`
- `data/scripts/room-028-bar/local-202.txt [001C]` to `[0023]`; `local-203.txt [0009]`, `[001E]`, `[0030]`, `[003A]` to `[004A]`; `local-218.txt [0017]`
- `data/scripts/global/script-002.txt [02DB]`, `[02E0]`, `[039D]`; `data/scripts/global/script-018.txt [0070]`
- `rules/glitchless.md` Click equivalence, Camera visibility, Known fidelity gap
- `docs/part1/rooms.md` §2.4, §7; `docs/part1/model.md` §2, §14.1, §14.3
- `docs/optimization.md` per-action costs (`provoke-cook` 282, `walk bar-left bar-right` 198, `walk-into-kitchen` 2659.7, `walk-into-kitchen-after-provoking-cook` 864, `walk-to-kitchen-door-provoking-cook` 462)
- `out/runs/20261005T122038Z-run/trace.jsonl` trace steps 2 to 3
- `docs/human-route.md` §3.2 steps 4 to 6, §5 Q4

**Fix and follow-up.** No model change.

- `docs/part1/model.md` §9: add `walk-to-kitchen-door-provoking-cook` as a fourth composite recount (1 action, 0 transitions), and replace "−1: no Open door (kitchen)" with "−1 transition: no curtain walk on the way in".
- Keep `docs/part1/model.md` §10's bar-left line. It covers the unprovoked Walk to 316, which is still unmodelled.
- `docs/human-route.md` §5 Q4 is answered: the Open is not required, but it is faster.
- Optional: rewatch IL 0:21 to 0:27 to confirm that the "generic failure line" is the cook's line.
- Open rules question: whether 316 comes on screen before the Open fires (§6.4).

### D3. Meat and pot picked up separately, meat first

- **Classification: genuine shortcut (different order, forced by the helmet quirk).** This replaces the old D3, the one-sentence pickup.
- **Delta:** 0 actions and 0 transitions against the human. The old route's −1 action is gone.
- **Ours:** #5 Pick up 566 (the meat), then #6 Pick up 567 (the pot).
- **Human:** H10 Pick up pot (IL 0:45), then H11 Pick up hunk of meat (IL 0:46). Pot first is our dead end `pick-up-pot-first`.

**Explanation.** Both routes use the same two Pick up sentences. Only the order differs.

- The meat lies in the room (owner 15), so `pickupObject(566)` runs (obj-0566 `[0035]`/`[0041]`). The same holds for the pot (obj-0567 `[001E]`/`[002A]`).
- Trace steps 4 and 5 (#5, #6) take 60 ticks each. They set `Var[133]` 0 → 566 and then `Var[134]` 0 → 567.

**Why the order is forced for us (the D5 coupling).** The helmet works only through the tent's off-by-one:

- Clicking slot verb 200+k makes local-200 read `Var[134+k]` at `[0107]`. It checks for 567 at `[010E]`, then jumps to the helmet code.
- The display fills slot k from `Var[133+k]` (`global/script-009.txt [0092]`). So some item must be displayed before the pot. Slot verbs below 200 fail local-200's range check at `[00EE]`.
- The trace confirms it at #13: clicks [7, 200] with `Var[133]` = 566 and `Var[134]` = 567 going in. `Bit[103]` goes 0 → 1, `Var[195]` goes 0 → 478, and 567 is removed.
- `rules/glitchless.md` (Click equivalence) names this quirk as allowed.
- Taking the pot first while holding nothing would put it in slot 0. That is our dead end `pick-up-pot-first` (`docs/part1/model.md` §4.3).

The human's order is not a dead end for the human, for two independent reasons:

- They pick up the fish first (H9), so their display order is fish, pot, meat.
- Their helmet is Give pot plus a click on a brother. That resolves the brother through `actorFromPos` (local-200 `[0013]`) and ignores slot order.

Neither is open to us. The fish needs the plank, which is coordinate input (and the fish is swordfight-only). Give plus a brother is pixel input (Banned 4).

One skeptic added a nuance. Pot-first is a dead end only in the model. In the game it can be recovered through the money object 488, but that needs the minutes deal, which the model excludes as never faster. Within the time-optimal model the quirk forces meat before pot.

**Time.**

- 60 + 60 = 120 ticks on every sample: `pick-up-meat` n = 1439; `pick-up-pot-not-first-meat` after `pick-up-meat`, n = 1259, all 60.
- The pooled row for `pick-up-pot-not-first-meat` (62.3 ± 6.0, max 78) mixes in a different route, where it follows `pick-up-stewed-meat-plain`.
- The old one-sentence `use-meat-with-pot` measures 144 ticks (n = 76), because it ends in the meat's refusal line (obj-0566 `[00BD]` → global 3).
- So the time plan spends one more action to save 24 ticks. Route selection by mean ticks intends exactly that.
- The IL takes about 2 s (0:45 → 0:47); ours takes 2.0 s.

**Citations:**

- `data/scripts/room-041-kitchen/obj-0566-hunk-of-meat.txt [0035]`, `[0041]`, `[00BD]`
- `data/scripts/room-041-kitchen/obj-0567-pot.txt [001E]`, `[002A]`
- `data/scripts/room-051-circus-te/local-200.txt [0013]`, `[00EE]`, `[0107]`, `[010E]`
- `data/scripts/global/script-009.txt [0092]`
- `data/scripts/global/script-002.txt [0229]`, `[0232]`, `[0239]`, `[0251]`, `[025A]`, `[0261]` (the old one-sentence form)
- `pddl/part1/domain.pddl`: `pick-up-meat` (line 474), `pick-up-pot-first` (485), `pick-up-pot-not-first-meat` (494), `use-meat-with-pot` (540)
- `docs/part1/model.md` §4.3, §14.4; `docs/part1/money.md` B1, B4
- `docs/optimization.md` per-action costs and per-context rows
- `out/runs/20261005T122038Z-run/trace.jsonl` trace steps 4, 5 and 13

**Fix and follow-up.** No model change. Keep `pot-guarded-by` and the dead end `pick-up-pot-first`, and never model Give pot to a brother. `docs/part1/model.md` §8 and §9 ("−1: pot and meat in one sentence") describe the unit-cost plan; label them as such.

### D4. The bar exit in one sentence from the right half

- **Classification: genuine shortcut (off-screen exit).** Unchanged from the old D4, but the skeptics split 1-1 (below).
- **Delta:** 0 actions and −1 transition. In time, about −180 ticks plus one input against a human (an estimate, below).
- **Ours:** #8, one Walk to 315 from bar-right. The first exit plays the LeChuck cutscene, now skipped with Esc.
- **Human:** H13 back room → main room, then H14 main room → Dock with Esc (IL 0:48 to 0:54).

**Explanation.** The human's "back room" and "main room" are one engine room, 28.

- local-201 points the camera at x 160 while ego is left of x 320, and at x 480 otherwise (`room-028-bar/local-201.txt [0000]` to `[0016]`). Door 315 (x 32 to 72) is on screen only in the left half (`docs/part1/rooms.md` §2.4, T25a).
- The door's Walk to checks only its state (obj-0315 `[0072]` to `[0077]`). It was opened at #1 and stays open.
- On the first exit it sets `Bit[446]` and starts global 120 (`[0080]` to `[008A]`), which ends in `loadRoomWithEgo(428,33)` (`global/script-120.txt [0538]`).
- Our single Walk to 315 walks ego left across x 320, and the camera pans. That is the case "Camera visibility" allows.
- A human in the right half must cross first. That is H13, and it must be the curtain (obj-0323 `[000C]` to `[0022]`, `walkActorTo` 310,137) or its alias, object 320: one skeptic found that a floor click cannot cross the bar from the right half. So the −1 holds against any human, and the old caveat about an uncounted floor click is dropped.

**Time (seed 1, trace step 7, ticks 11179 → 11635 = 456; n = 900, stdev 0).**

- 11179 to 11557: 378 ticks of walking. At 11557 the cutscene starts and clears the click queue.
- Then 72 ticks while script 120 runs `delay(60)` (`[0007]`) and reaches `beginOverride` (`[000E]`). Esc can do nothing before that. The skip at 11629 has `offs` 24, the instruction just after the override is installed, so it fired on the first frame it could take effect. A human pays the same wait.
- Then 6 ticks to load the Dock: the skip goes to `[052D]`, then `loadRoomWithEgo(428,33)` at `[0538]`.
- Unskipped, the same exit took 9696 ticks (Appendix).

**Against a human: corrected.** The classifier compared #8 with the bot's own two-sentence form in our engine: `walk bar-right bar-left` (210) then `walk-out-of-bar-from-left-meanwhile` (252) = 462, a saving of one frame. Both skeptics rejected that comparison.

- The second push of that form is itself an off-screen-rule sentence, pushed before the camera has finished panning. No human can play it.
- A human who clicks the curtain must wait for 315 to come on screen before clicking it. The camera pans 8 px per frame (var 26, `VAR_CAMERA_FAST_X`, is 0), so that is about 30 frames.
- So against a human, #8 saves about 180 ticks plus one input. This is an estimate; it was not measured.

The IL takes about 6 s for H13 and H14 (0:48 to 0:54, ±1 s); ours takes 7.6 s. That gap is walking speed from the logo speed glitch (§1.4), not D4.

**The split.**

- One skeptic upheld the label with the corrections above.
- The other refuted it. By that skeptic's reading, at 8 px per frame 315 never comes on screen before the exit fires. The rule's premise, that the walk the sentence starts "would bring the object on screen", then fails, and a strict reading would disallow this exit. It would also disallow the two-sentence form, and possibly #3 (D2).
- The label is kept, because the rule names the bar halves as an allowed example. The question is recorded as an open rules decision in §6.4, with the cost of the strict reading.

**Citations:**

- `data/scripts/room-028-bar/obj-0315-door.txt [0072]` to `[0077]`, `[0080]` to `[008A]`, `[0090]`
- `data/scripts/room-028-bar/local-201.txt [0000]` to `[0016]`; `obj-0323-curtain.txt [000C]` to `[0022]`
- `data/scripts/global/script-120.txt [0007]`, `[000E]`, `[052D]` to `[0538]`
- `rules/glitchless.md` Camera visibility; Allowed 5
- `docs/part1/rooms.md` §2.4, T25a; `docs/part1/model.md` §2, §13.5, §14.1
- `docs/optimization.md` per-context rows: `walk bar-right bar-left` @ `entry:kitchen:bar-right:570` 210; `walk-out-of-bar-from-left-meanwhile` @ `entry:bar-right:bar-left:323` 252; `walk-out-of-bar-from-right-meanwhile` @ `entry:kitchen:bar-right:570` 456 (n = 900)
- `out/runs/20261005T122038Z-run/trace.jsonl` trace step 7 (clicks cleared at 11557; skip at 11629, script 120, `offs` 24)
- `docs/human-route.md` §3.2 steps 13 to 14

**Fix and follow-up.** No model change. Both forms are modelled and measured, and the optimiser picked the faster one. The rules decision on the camera rule's premise is open (§6.4, §8).

### D5. The circus helmet: Use plus the slot before the pot, not Give pot to a brother

- **Classification: genuine shortcut (different method; the human's method is banned, and the ban costs nothing).** This settles the old D5's 1-1 split: both skeptics now uphold it.
- **Delta:** 0 actions and 0 transitions. One uncounted click is saved: Use plus slot is 2 clicks, while Give, pot and brother is 3.
- **Ours:** #13, during the helmet wait (`VAR_VERB_SCRIPT` = 200): click Use (verb 7), click verb 200 (the meat's slot), then choose "nibboB". The walk-out is scripted.
- **Human:** H20 Give pot to Fettucini brothers, then H21, the walk-out.

**How the two paths join.** Both enter the tent's input script local-200. local-207 installs it (`[0D4C]`) and waits for `Bit[103]` (`[0D5C]`).

- **Ours (2 clicks).**
  1. The Use click falls through to `chainScript(4)` at `[0129]`. Script 4 sets `Var[107]` = 7 (`[0347]`) and `Var[110]` = 0 (`[0356]`).
  2. The click on verb 200+k passes the range checks `[00EE]`/`[00F5]`. It reads `Var[134+k]` at `[0107]`, one past the display (which fills slot k from `Var[133+k]`; script 4 reads it back the same way at `[0333]`).
  3. `[010E]` (== 567), `[0115]` (`Var[107]` == 7) and `[011C]` (`!Var[110]`) pass. `[0121]` sets `Var[108]` = 567, and the script jumps to `[0021]`.
- **The human (3 clicks).** Give and the pot go through script 4's normal slot read. The click on a brother is a scene click, resolved by `actorFromPos(VAR_VIRT_MOUSE_X,VAR_VIRT_MOUSE_Y)` at `[0013]`. Then `[0032]` (`Var[108]` == 567) leads into the same `[0021]`.
- **Shared.** `[0021]` walks ego to (243,135), then plays the helmet cutscene. `[008B]` sets `Bit[103]`; it is the only reachable setter (the one in local-210 `[001E]` is dead code, `docs/part1/money.md` B1). Then come the cannon stunt, the reversed menu, the 478 payout (local-207 `[110E]`) and the scripted walk-out (`[114D]`).

**Trace (seed 1).** The Use click is at tick 14049 and the slot click at 14055. Before them, `Var[133]` = 566 and `Var[134]` = 567. During the step, `Bit[103]` goes 0 → 1, `Var[195]` goes 0 → 478, the pot leaves the inventory, 488 (the money) is added, and the room goes 51 → 52.

**Why genuine shortcut and not rules difference.**

- The human's path needs mouse coordinates, which Banned 4 forbids. Pushed as a sentence instead, it skips local-200 and can lose the pot (`docs/part1/money.md` B1). So the model never contains it (`docs/part1/model.md` §5).
- The ban costs us nothing. Our path is legal and named in `rules/glitchless.md` (Allowed 1; Click equivalence, for the off-by-one and for effects that live only in an input script), and it is never slower: 2 clicks against 3, with everything after `[0021]` shared. Even with pixel input allowed, the optimiser would still pick Use plus slot.
- It is not a version difference. The human's path exists in our Mac scripts, and our path was open to the IL runner too, who held the fish before the pot.

**Time.** 1062 ticks on every sample (n = 1515), with 8 text and 3 cutscene skips. That is the surrogate's convention: the trace step lasts 1056 ticks, and the 6-tick `esc_frame` deferral before it is included. On the same span as the IL (the helmet line at tick 14139 to room 52 at 15075), ours takes 936 ticks (15.6 s), against about 15 s for the IL (1:27 → 1:42). Those agree within the video's 1 s resolution.

**Citations:**

- `data/scripts/room-051-circus-te/local-200.txt [0013]`, `[0021]`, `[0032]`, `[008B]`, `[00EE]`, `[00F5]`, `[0107]`, `[010E]`, `[0115]`, `[011C]`, `[0121]`, `[0129]`
- `data/scripts/room-051-circus-te/local-207.txt [0D4C]`, `[0D5C]`, `[110E]`, `[114D]`; `local-210.txt [001E]` (dead code)
- `data/scripts/global/script-009.txt [0092]`; `data/scripts/global/script-004.txt [0333]`, `[0347]`, `[0356]`
- `rules/glitchless.md` Allowed 1, Banned 4, Click equivalence
- `docs/part1/money.md` B1; `docs/part1/model.md` §4.3, §5
- `pddl/part1/domain.pddl` `walk-out-of-tent-after-helmet-meat` (line 258; precondition `(pot-guarded-by meat)` at line 260)
- `out/plans/part1.time.jsonl` (click `[{verb:7},{inventory:567,offset:-1}]`, choose `nibboB`, until var 32 == 200)
- `out/runs/20261005T122038Z-run/trace.jsonl` trace step 13; `docs/optimization.md` (1062.0, sd 0, n = 1515)
- `docs/human-route.md` §3.2 steps 9, 20, 21

**Fix and follow-up.** None. Keep the `(pot-guarded-by g)` precondition and the `pick-up-pot-not-first-*` actions, and keep Give pot to a brother out of the model.

### D6. The meat is drugged at the petal screen, not on the Dock

- **Classification: genuine shortcut (different order, tie).** This was the old D9, which had no skeptic review. Two skeptics reviewed it for the first time plan, which drugged the meat in the jail. The script argument below is the same for both places.
- **Delta:** 0 actions, 0 transitions and 0 ticks.
- **Ours:** #18 `drug-meat-with-petal`, the sentence (verb 7, 566, 689), at the petal screen (room 215), right after #17 picks up the petal.
- **Human:** H29 Use yellow petal with hunk of meat, on the Dock after H28 Map → Village (IL 2:06).
- The first time plan, `sample-02`, drugged the meat in the jail (room 31), right after Otis #1. In the final run that placement is the candidate `mean`.

**Explanation.** It is the same sentence in a different place.

- The meat's Use entry has no room guard. obj-0566 `[0077]` tests `Local[0]==689`, then `[007E]` runs `setClass(566,[134])` (sets class 6, the drugged class) and `[0085]` starts script 182.
- Script 182 renames the meat to "meat with condiment" (`[0000]`) and consumes the petal (`[0017]` `setOwnerOf(689,0)`). The rename matches the IL inventory at 2:06, so the human ran the same script.
- Operand order does not matter: the petal's Use forwards to `doSentence(7,566,689)` (obj-0689 `[0049]`).
- With both items held, the sentence script skips every `walkActorToObject` (`global/script-002.txt [027A]` to `[02A6]`), so ego does not move.
- The model excludes only the map, where every click becomes Walk to (`room-085-melee/local-201.txt [0035]`), the tent, an open store door and a provoked cook (`pddl/part1/domain.pddl` `drug-meat-with-petal`, line 562).
- The human's Dock placement would be a tie too. `speedrun.positions` leaves the position token unchanged for an inventory-only sentence, so drugging on the Dock would cost the same surrogate 6 ticks.

**Time.** 6 ticks (one frame) on every pooled sample (n = 1515), and in all 8 contexts measured in the final run. On seed 1, trace step 18 runs from tick 16131 to 16137.

**Petal screen against jail.** Both optimiser runs measured the two placements as a pair of plans that differ only in where this one line sits; a `diff` of the two plan files shows nothing else.

In the final run, `mean` (jail) minus the winner `sample-16` (petal screen) on select seeds 1 to 30 is +11.6 ± 14.9 ticks per seed (standard error). The winner was faster on 11 seeds, `mean` on 10, and 9 tied. `mean` was not measured on the held-out seeds.

The first run (`out/optimize/20261005T084713Z`, with path 685 at the Fork) had them the other way round: the winner `sample-02` drugged in the jail and the runner-up `mean` at the petal screen. Paired per-seed differences there, `mean` minus `sample-02`:

| seeds | ticks per seed |
|---|---:|
| select, 1-30 | +6.2 ± 11.3 |
| held-out, 31-60 | −18.2 ± 13.4 |
| pooled, 60 seeds | −6.0 ± 8.8 |

- The pooled value is consistent with zero.
- Plan numbers in these bullets are `sample-02`'s. Steps #1 to #25 had identical durations on all 60 seeds. The steps that differed were #27, #28, #31 and #32.
- The pay step, #31, gave −600 of the held-out −546 sum (and +240 of the select +186). That is the storekeeper's random line choice (`room-030-store/local-211.txt [043D]`, `[060B]`, `[08FF]`). The two placements shift the store's random draws by one frame, so the gap is RNG, not route cost.
- The two runs lean in opposite directions, each by less than one standard error: the first run's select seeds favoured the jail by 6.2 ticks, and the final run's favoured the petal screen by 11.6. The placement is a tie, and the store's RNG decides it on any given set of seeds.
- Under `rules/glitchless.md` "Route selection", the choice made on seeds 1 to 30 stands. Choosing by the held-out seeds would be selecting on the report set, and on noise.

**Not verified.** G19 says to drug "while walking". Under our sequential step player the step takes one idle frame. ScummVM defers a queued sentence while the sentence script runs (`script.cpp` `checkAndRunSentenceScript`), so a sentence-walk would not overlap. Whether a push during a scripted (non-sentence) walk would overlap is not established. It could save at most 6 ticks per inventory-only step.

**Citations:**

- `data/scripts/room-041-kitchen/obj-0566-hunk-of-meat.txt [0077]`, `[007E]`, `[0085]`
- `data/scripts/global/script-182.txt [0000]`, `[0017]`
- `data/scripts/room-058-damnfores/obj-0689-yellow-petal.txt [0049]`
- `data/scripts/global/script-002.txt [027A]` to `[02A6]`
- `data/scripts/room-085-melee/local-201.txt [0035]`
- `data/scripts/room-030-store/local-211.txt [043D]`, `[060B]`, `[08FF]`
- `pddl/part1/domain.pddl` `drug-meat-with-petal`; `src/speedrun/positions.py`
- `docs/part1/model.md` §4.3, §8; `docs/optimization.md` Candidates, Held-out seeds
- `out/optimize/20261005T084713Z/candidates/{mean,sample-02}/plan.sas_plan`; `out/optimize/20261005T111318Z/candidates/{mean,sample-16}/plan.sas_plan` and their `select` traces
- `out/runs/20261005T122038Z-run/trace.jsonl` trace step 18
- `docs/human-route.md` §3.2 step 29; `rules/glitchless.md` Route selection

**Fix and follow-up.** No model change. `docs/part1/model.md` §8 ("moved … to the town half of High Street") describes the old unit plan; note that the time plan drugs at the petal screen. Optional: test a push during a scripted walk for the inventory-only steps (#18, #47).

### D7. The store menu, opened through the door

- **Classification: genuine shortcut (fixed-plan limitation: replay feasibility).** The classifier called this a rules difference. Both skeptics refuted that, and their label is adopted. Our door path is legal, and the model is right to exclude Talk to for a fixed plan. But under ticks the human's method is faster, so this is no longer the old D6's "equal-cost tie".
- **Delta:** 0 actions, 0 transitions and 3 picks on each side. In time, the human's Talk to is estimated at about 100 ticks faster on the mean. That figure was not measured.
- **Ours:** #31, Walk to 387 with the shovel unpaid. Picks "shovel", "I want it", "breath mint"; the menu then closes itself.
- **Human:** H45, Talk to storekeeper 394, with picks 3, 1, 1, 1, 1, 2 including the sword (IL 3:05 to 3:13).

**Both inputs open the same dialogue.**

- With the shovel unpaid, Walk to 387 runs local-204 (obj-0387 `[008C]` to `[0098]`). local-204 won't let ego leave (`[001B]` to `[002C]`, `[0031]` to `[004C]`) and ends in `startScript(211)` (`[042B]`).
- Talk to 394 starts 211 directly (obj-0394 `[0015]`).
- 211 builds its menu from game state and closes it by itself once only "browse" is left (`[03EB]`). So shovel plus mints takes 3 picks either way. The human's extra picks are the sword purchase, which only the swordfight needs (D11).

**Why the routes differ: a fixed plan cannot see the draw.**

- On each store entry, `getRandomNr(3)` leaves the storekeeper away 1 time in 4 (`room-030-store/entry.txt [002B]`). When he is away, actor 11 is moved out of the room and 394 is made untouchable (class 32, `[0054]` to `[0057]`), so Talk to cannot be clicked. When he is present, local-200 `[0000]` makes 394 touchable.
- A human sees whether he is there and clicks him. Our plan format has no conditional or fallback step. `until` only waits, and an absent storekeeper never comes back by himself, so the step would hang. `interrupts` fire only on menus the plan does not answer.
- So a Talk to plan would fail on about 1 seed in 4, and route selection needs a plan that is valid on every seed. The door path is the one sentence that works in both cases. When he is away, it makes him walk back in (`local-204.txt [0053]` to `[0359]`).
- **Not a rules difference.** Nothing in `rules/glitchless.md` forces the door path. Allowed 2 (values randomised before the segment) does not apply: this draw happens at store entry. The old D6's argument from Allowed 2 is withdrawn.
- **Not a modelling bug.** The exclusion is correct for the current plan format (`docs/part1/model.md` §4.5 and the replay-feasibility rows of §5).

**Time.**

- Across the 832 pooled samples in `docs/optimization.md`, the pay step is exactly 708 ticks when he is present (620 cases) and 828 when he is away (212 cases, about 25%). That gives the pooled mean of 738.6. On the winner's held-out seeds he was away 6 times in 30.
- On seed 1 of the final route he was away, and #31 took 828 ticks (trace step 31, 20439 → 21267). The walk to the door ends at 20817, where local-204's cutscene starts, and the first choice comes at 21081, 264 ticks later.
- In the first time plan's seed-1 run (`out/runs/20261005T094110Z-run`) he was present. Inside #31 there: the walk to the door runs 20511 → 20889, where local-204's cutscene starts. The "pay for that?" line, the walk to 394 and `endCutscene` take 138 ticks (20889 → 21027). The first choice is at 21033, 144 ticks after the cutscene starts.
- local-204 has no `beginOverride`, so Esc cannot shorten that window. All 3 cutscene skips in the step are inside 211.
- Talk to would avoid the door-to-394 leg, but add one skipped frame for the greeting (211 skips it only when `Bit[309]` is set, `[0020]` to `[002D]`). The walk to 394 is about as long as the walk to the door: roughly 9 against 10 frames, from walkbox geometry.
- **Estimated saving:** about 120 to 140 ticks (2 to 2.3 s) on present entries and nothing on away entries, so about 100 ticks on the mean. This comes from walkbox geometry plus the measured window; Talk to itself was not measured.

**Citations:**

- `data/scripts/room-030-store/obj-0387-door.txt [008C]` to `[0098]`
- `data/scripts/room-030-store/local-204.txt [001B]` to `[002C]`, `[0031]` to `[004C]`, `[004E]` (cutscene, no override), `[0053]` to `[0359]`, `[0415]` (`Bit[309]`), `[0421]` (walk to 394), `[042B]`
- `data/scripts/room-030-store/obj-0394-storekeeper.txt [0015]`
- `data/scripts/room-030-store/local-211.txt [0017]`, `[0020]` to `[002D]`, `[03EB]`, `[1DB6]` to `[1DD2]`
- `data/scripts/room-030-store/entry.txt [002B]`, `[0042]` to `[0057]`; `local-200.txt [0000]`
- `out/runs/20261005T122038Z-run/trace.jsonl` trace step 31 (storekeeper away: 20439 start, 20817 cutscene, 21081 first choice, 21267 end)
- `out/runs/20261005T094110Z-run/trace.jsonl` trace step 31 (first time plan, storekeeper present: 20511 start, 20889 cutscene, 21027 `endCutscene`, 21033 first choice, 21219 end)
- `out/optimize/20261005T111318Z/measured-costs.json`: 708 × 620, 828 × 212; the winner's held-out traces (`candidates/sample-16/report`): 708 × 24, 828 × 6. Earlier count, `out/optimize/20261005T084713Z` and `out/measure` traces: 708 × 390, 828 × 134
- `pddl/part1/segment.toml` (`interrupts`); `pddl/part1/steps.toml` `pay-for-shovel-and-mints`
- `rules/glitchless.md` Allowed 2, Route selection, Engine settings ("plan must remain valid")
- `docs/part1/model.md` §4.5, §5; `docs/human-route.md` §3.2 step 45; `docs/next.md` "Optimisation"

**Fix and follow-up.** No model change. The follow-up is a conditional step keyed on the storekeeper's presence (actor 11 in room 30, or 394 without class 32): Talk to 394 when he is present, Walk to 387 when he is away. Then measure it. This is the same reactive-plan mechanism `docs/next.md` wants for the swordfight and for "seed-dependent durations". The old D6 docs fix still applies: `docs/human-route.md` step 45 and the §4.1 picks table should say 3 picks for mint and shovel, not "about 4".

### D8. The poodles: Give instead of Use

- **Classification: genuine shortcut (equivalent verb, exact tie).** This was the old D7.
- **Delta:** 0 actions, 0 transitions and, by script inference, 0 ticks.
- **Ours:** #35, Give 566 to 467, the sentence [4,566,467].
- **Human:** H49, Use meat with condiment with poodles (IL 3:30 to 3:38).

**Both verbs reach the same handler** (obj-0467 verb 80 → local-201 with `Local[0]` = 566), with the same walk and the same cutscene. Neither adds a frame.

- **Give (ours).** `global/script-002.txt [003F]`: verb 4. `[0046]`/`[004D]`: 467 is above 12 and has class 5. `[0083]` walks ego to 467 and `[0088]` waits. `[008C]`/`[0093]`: the distance is at most 32. `[009A]`: `startObject(467,80,[566])`. Then obj-0467 `[00B2]` (`Bit[15]` still 0) and `[00D5]` `startScript(201,[566])`.
- **Use (the human's).**
  - `[016C]` → `[0203]` is the branch for an object with a Use entry; the verb-7 test is at `[020F]`.
  - `[021D]` to `[026D]`: the meat is in the inventory, and 467 has no class 7, so nothing is auto-picked-up.
  - `[027A]` to `[029C]`: the target becomes `Local[4]` = 467.
  - The walk at `[02DB]` happens because 467 has neither class 8 nor class 10 (checks at `[02B4]` and `[02C1]`; its classes are {5,13}). Then the distance at `[02E4]` must be at most 16 (`[02EB]`).
  - The reach-animation block (`[0385]` to `[0397]`) sits behind the owner == 15 check at `[0369]`, and the meat is in the inventory, so it cannot fire.
  - `[039D]` runs obj-0566's Use, and `[0098]`/`[00A2]` there call the same `startObject(467,80,[566])`. These nested calls run straight through, with no `breakHere`.
- **Reach.** The old worry about "≤ 16 against locked boxes" is unfounded. Before the dogs sleep, room 36's entry sets boxes 1-6 to flag 128 (`entry.txt [002B]` to `[0038]`). That flag is `kBoxInvisible`, not `kBoxLocked` (0x40) (`boxes.h:39-41`). `adjustXYToBeInBox` skips invisible boxes (`actor.cpp` about line 2040), and `getObjActToObjActDist` adjusts the object point the same way (`object.cpp:546-550`). So the walk target and the distance target are the same point, and the distance after the walk is 0. The IL shows Use working.
- **Click equivalence.** In trace step 35, `Var[32]` (`VAR_VERB_SCRIPT`) goes 4 → 203. So the default input script was active when the sentence was pushed, and room 36 had no guard; local-201 `[01B9]` installs input script 203 only afterwards.

**Time.** local-201 sets `Bit[15]` at `[0087]` (it needs the drugged class 6, `[0079]`). It contains five `delay(60)` calls, a fixed 300 jiffies, and no `beginOverride`, so Esc cannot shorten it in either variant. Give costs 636 ticks on every sample (n = 1515, stdev 0); on seed 1, trace step 35 runs from tick 22389 to 23025. Use has never been replayed; its tie is inferred from the scripts.

**Gap to the human** (about 8 s against our 10.6 s). The verb does not cause this gap, because the fixed delays are the same for both. It falls in the walk to the dogs and in frame-paced animation. The likely causes are the human's logo speed glitch, DOS against Mac timing, and the human clicking during the walk-in. The whole-second timestamps also blur it.

**Citations:**

- `data/scripts/global/script-002.txt [003F]`, `[0046]`, `[004D]`, `[0083]`, `[0088]`, `[008C]`, `[0093]`, `[009A]`, `[016C]`, `[0203]`, `[020F]`, `[021D]` to `[026D]`, `[027A]` to `[029C]`, `[02B4]`, `[02C1]`, `[02DB]`, `[02E4]`, `[02EB]`, `[0369]`, `[0385]` to `[0397]`, `[039D]`
- `data/scripts/room-041-kitchen/obj-0566-hunk-of-meat.txt [0098]`, `[00A2]`
- `data/scripts/room-036-mansion-e/obj-0467-deadly-piranha-poodles.txt [00B2]`, `[00D5]`
- `data/scripts/room-036-mansion-e/local-201.txt [0079]`, `[0087]`, `[01B9]`; `entry.txt [002B]` to `[0038]`
- `third_party/scummvm/engines/scumm/boxes.h:39-41`, `actor.cpp` (about 2040), `object.cpp:546-550`, `script.cpp:1689-1715` (`abortCutscene` needs the pointer that `beginOverride` sets)
- `out/runs/20261005T122038Z-run/trace.jsonl` trace step 35; `docs/optimization.md` (636.0, n = 1515)
- `docs/human-route.md` §3.2 step 49; `rules/glitchless.md` Allowed 1, Click equivalence

**Fix and follow-up.** No model change; the model keeps Give, the variant that has been replayed. Doc fixes carried over from the old D7 (§8): `docs/part1/model.md` §5 (the "Use meat with poodles" row), `docs/part1/idol.md` §2.5 and idol.md open question 3 still describe Use as riskier against "locked" boxes. Optional: one headless replay of [7,566,467] would turn the inferred tie into a measurement.

### D9. The idol room needs both Open and Walk to

- **Classification: genuine shortcut (counting convention).** This was the old D8, renumbered (#37/#38 there, #38/#39 here). The route did not change here. Both skeptics upheld the label, and one corrected the script reading. Those corrections are applied below.
- **Delta:** +1 action against the human list as written (22); 0 against the corrected 23. Transitions 0: both steps stay in room 53.
- **Ours:** #38 Open 632, then #39 Walk to the open 632.
- **Human:** H52 "Open door (foyer, right)", counted as one action. The human's click on the open door is inferred (IL 3:42 to 3:45).

**Explanation.** Both routes most likely use the same two sentences; the human list counts them as one.

- **#38, Open 632 (108 ticks).** obj-0632 `[0028]` runs global 25. Script 25 opens the door only when it is in state 0 and class 6 is clear (`classOfIs(Local[0],[6])` at `[0016]`, then `setState(…,1)` at `[0024]`); otherwise it prints that the door is locked (`[0040]`). So class 6 set means locked, as the old report said. Script 25 starts nothing else, and trace step 38 changes no bits and no inventory.
- **#39, Walk to the open 632 (60 ticks).** obj-0632 `[0018]`/`[001D]` checks for state 1, then `[0024]` starts local-210. local-210 is a cutscene. It sets `Bit[481]` at `[0002]`, before its override is installed at `[0007]`/`[0009]`.
- **What ran on seed 1: the override path.** The Esc on script 210 came at tick 23349, while 210 was still waiting for 205 (`[000C]` to `[0014]`). The abort resumes at the override target `[042F]` with `VAR_OVERRIDE` set (`script.cpp:1689-1726`). That path:
  - closes the door (`[0467]` `setState(632,0)`);
  - locks it for good (`[046B]` `setClass(632,[134])`, which sets class 6);
  - gives the four items directly, through the nested owner checks: 643, 641, 642 and 640 (`[04A6]`, `[04AA]`, `[04AE]`, `[04B2]`).

  The trace adds them in that order (vars 137 to 140), with `Bit[481]` going 0 → 1 and no room change. Then come two `.` skips (23379, 23385) and `clicks_cleared` by script 19 (23397).
- **The unskipped body** gives the same items another way (`[01FB]`, `[0236]` → `local-204.txt [0047]`, `[0267]`, `[02B8]`). The skip-safety harness found no route-relevant difference (§6.2), so no `no_skip` is needed.

**There is no one-sentence path, so this is not a modelling bug.**

- Walk to on the closed 632 does nothing (`[001D]`).
- Use 632 runs script 31, which only queues an Open or a Close (`doSentence(3)` at `[0016]`, `doSentence(2)` at `[001F]`).
- In room 53 and the globals, only obj-0632 `[0024]` and the Part III global `script-102.txt [0071]` start 210.
- The only `setState(632` is local-210 `[0467]`, which closes the door. Script 25 is also run on 632 by local-206 `[0016]` and local-216 `[0077]`, but both run only inside 210's unskipped body.
- On the open door, verb 90 sets `Var[182]` = 3 (Close, `[0048]`). That is the highlighted verb, which is a further sign that entering takes a separate Walk to click.

**The human list's own rules count the Walk to.** §3.1 defines [A] as one verb/object sentence. By that rule the list counts H64 "Walk to gaping hole", which has the same shape: a Walk to inside the room that starts a cutscene. G19 also folds this step into "Open door on right, SKIP cutscene, leave".

**Time.** 108 + 60 = 168 ticks (2.80 s) on every sample (n = 1515, stdev 0). The IL window 3:42 to 3:45 starts at the H51 entry; adding our 6-tick #37 gives 174 ticks (2.90 s) on the same basis, against about 3 s for the IL. Unskipped, #39 took 10140 ticks (Appendix).

**Caveat.** We only have Mac scripts. If the DOS version started 210 on Open alone, this would be a version difference instead. It would still not be a modelling bug, and nothing points to it.

**Citations:**

- `data/scripts/room-053-foyer/obj-0632-door.txt [0018]`, `[001D]`, `[0024]`, `[0028]`, `[0030]`, `[0034]`/`[0048]`
- `data/scripts/global/script-025.txt [0016]`, `[0024]`, `[0040]`; `data/scripts/global/script-031.txt [0016]`, `[001F]`
- `data/scripts/room-053-foyer/local-210.txt [0002]`, `[0007]`/`[0009]` (override → `[042F]`), `[000C]` to `[0014]`, `[0467]`, `[046B]`, `[04A6]` to `[04B2]`; unskipped body `[01FB]`, `[0236]`, `[0267]`, `[02B8]`
- `data/scripts/room-053-foyer/local-204.txt [0047]`; `local-206.txt [0016]`; `local-216.txt [0077]`; `data/scripts/global/script-102.txt [0071]`
- `third_party/scummvm/engines/scumm/script_v5.cpp` `o5_ifClassOfIs`, `o5_setClass`; `script.cpp:1689-1726`
- `out/runs/20261005T122038Z-run/trace.jsonl` trace steps 38, 39 (Esc on 210 at 23349; text skips at 23379, 23385; `clicks_cleared` at 23397)
- `pddl/part1/domain.pddl` `open-idol-room-door` (line 430), `enter-idol-room` (838); `pddl/part1/measured-costs.json`
- `docs/human-route.md` §3.1, §3.2 steps 51 to 53 and 63 to 64, §4.1 row 15

**Fix and follow-up.** No model change: two sentences is the minimum the Mac scripts allow. Doc follow-ups (§8): split `docs/human-route.md` step 52 and §4.1 row 15 into "Open door" and "Walk to door" (23 actions), and compare against 23 in `docs/part1/model.md` §9. Optional: check the IL frames at 3:42 to 3:45 for a "Walk to door" sentence line, which would turn the inferred click into an observed one and settle the DOS caveat.

### D10. The cake is opened on High Street, after the jail

- **Classification: genuine shortcut (different order, tie).** New.
- **Delta:** 0 actions, 0 transitions and 0 ticks.
- **Ours:** #45 Give repellent to Otis, then #46 walk jail → High Street, then #47 Open 420 in room 34.
- **Human:** H59 Open cake in the jail (IL 4:15), then H60 Jail → High street (IL 4:16 to 4:17). G19's written route has "Leave, Open cake", which is our order, so the divergence is only from the IL video.

**Explanation.** The same Open in a different room.

- The Open entry, `room-031-jail/obj-0420-cake.txt [0056]`, tests only that 420 still has class 6 (`classOfIs(VAR_ME,[134])`). It has no room or position check. `[005F]` `setClass(420,[6,131])` clears class 6, which is the model's `(cake-opened)`.
- For an inventory object the sentence script adds no reach animation and goes straight to the verb (`global/script-002.txt [0364]`/`[0369]`, `[0398]`/`[039D]`).
- A real Open on the cake on High Street would go through plain script 4. The stale mansion input script 203 is already gone: trace step 43 shows `Var[32]` going 203 → 4 (`room-034-high-stre/exit.txt [0021]`). High Street's own local 201 is installed only while global 67 (the storekeeper guide) runs (`entry.txt [0007]` to `[0010]`), which is not on this route.
- The open took effect: in trace step 47 (ticks 25665 to 25671), `Var[376]` goes 1037 → 1003, which is the class-6-cleared branch of the cake's verb-91 script (`[0027]`/`[0030]`). `steal-idol` later succeeds with the file.

**Time.** Per-context means, all with stdev 0 (`out/optimize/20261005T084713Z/report.json`):

| order | open-cake | walk jail → High Street | walk to the mansion half | total |
|---|---:|---:|---:|---:|
| in the jail (the human's) | 6 | 174 | 354 | 534 |
| on High Street (ours) | 6 | 174 | 354 | 534 |

`steal-idol` costs 726 in every context, so nothing downstream changes. In both optimiser runs, every candidate except `unit` puts the open on High Street; `unit` puts it in the jail. That is the search's tie-break. No measured candidate pair differs only by this swap, so the tie rests on the per-context means.

**Citations:**

- `data/scripts/room-031-jail/obj-0420-cake.txt [0027]`, `[0030]`, `[0056]`, `[005F]`
- `data/scripts/global/script-002.txt [0364]`, `[0369]`, `[0398]`, `[039D]`
- `data/scripts/room-034-high-stre/exit.txt [0021]`; `entry.txt [0007]`, `[0010]`
- `docs/part1/model.md` §4.3; `docs/part1/input-scripts.md` §1.2, §3.2
- `docs/optimization.md` (`open-cake` 6.0, n = 1515; 2 contexts in the final run); `out/optimize/20261005T084713Z/report.json` `context_flags`
- `out/runs/20261005T122038Z-run/trace.jsonl` trace steps 43, 46, 47, 48
- `docs/human-route.md` §3.2 steps 59 to 60

**Fix and follow-up.** No model change: the model allows `open-cake` in both places. Optional: a paired measurement of the winner against a copy with the open moved back into the jail would rule out any hidden context effect. It is low priority, because every context involved has stdev 0.

### D11. Swordfight-only and crew-only human steps

- **Classification: genuine shortcut (out of scope).** This was the old D10, which had no skeptic review; it now has two. Both upheld it and corrected two time notes, applied below.
- **Delta:** 0 in both counts. The human run as played has +10 actions and +6 transitions: 7 actions and 4 transitions for the swordfight, 3 and 2 for the crew.
- **Ours:** none of these steps.
- **Human:** swordfight: H7 to H9 (pier door, plank, fish), H44 and the sword picks inside H45, the H71 detour (bridge, troll, Smirk), the IL pirate dodge and the WR insult fight. Crew: H32 to H36 (the chicken) and the WR's Credit Early.

**Explanation.** None of these steps sets a goal flag or unlocks one.

- Both trial flags are set by global script 71 alone: `Bit[85]` (idol) by `startScript(71,[2])` at `room-042-underwate/local-200.txt [0041]`, and `Bit[86]` (treasure) by `startScript(71,[3])` at `room-064-treasure/local-200.txt [0214]`. The flag itself is set at `global/script-071.txt [008D]` (`Bit[83+Local[0]]` = 1).
- The dig tests nothing about the sword, fish, troll, Smirk or chicken. The human treasure path goes Lookout → Map → Fork with or without the detour (`docs/human-route.md` §4.3).
- Swordfight steps serve only the swordfighting trial, which `rules/glitchless.md` lists as "Out of scope for v1". Crew steps serve the Part I crew phase, which comes after the goal.
- **The fish.** This does not depend on `docs/part1/money.md` B4 (the fish looks unobtainable with sentences). Even if it were obtainable, it would not help: the poodles refuse it (`room-036-mansion-e/local-201.txt [01F9]`/`[0203]`), and the troll bridge is a dead end for these trials (`docs/part1/model.md` §2).
- **Our side has none of them.** The seed-1 trace adds only 566, 567, 488, 689, 442, 396, 395, 643, 641, 642, 640, 420, 635 and 578 to the inventory; the sword 388 and the fish never appear. The plan has no sword, fish, troll, Smirk, voodoo or safe action.
- **Map pirates.** The IL's dodge corresponds to our `map-pirate` interrupt on the 4th map entry, which answers "Sorry to bother you" (`room-049-road/local-200.txt [0325]`; `docs/part1/model.md` §7.2). It did not fire on seed 1. The WR's insult fight is swordfighting.
- **Credit Early** would on its own be a rules difference: it needs save/load (Banned 3) and knowledge of the seed. It is crew-only, so it stays out of scope.

**Time.** §4 removes this time from the human figures: fish about 6 s, sword about 4 s (partly inferred: the sword picks are mixed in with the mint and shovel picks), troll and Smirk 42 s, the pirate dodge about 6 s and the chicken about 9 s for the IL. For the WR, the detour block is about 81 s (partly inferred; no timestamp exists for its start) and Credit Early about 6 s (inferred).

- **Correction: owning the sword does not lengthen Fester under Esc.** The menu and the sword handover (`room-053-foyer/local-217.txt [030F]` to `[0336]`) lie inside the level-0 override `[000F]` → `[034B]`. The override branch repeats the confiscation at `[0364]` to `[0370]` at no cost (`docs/part1/skips.md`, row A51).
- **Only the underwater block costs the humans time.** `room-042-underwate/local-203.txt [0030]` to `[008B]` runs because the confiscated sword 388 is owned by 14. It has no override. It walks ego to 590, prints a skippable line and runs two `delay(30)`. It runs before local-200 `[0041]`, so it falls inside the humans' idol time (IL 4:42 to 4:47). That part is small and cannot be separated.

**Citations:**

- `data/scripts/room-042-underwate/local-200.txt [0041]`; `data/scripts/room-064-treasure/local-200.txt [0214]`; `data/scripts/global/script-071.txt [008D]`
- `data/scripts/room-053-foyer/local-217.txt [000F]`, `[030F]`, `[0336]`, `[034B]`, `[0364]`, `[0370]`
- `data/scripts/room-042-underwate/local-203.txt [0000]`, `[0030]`, `[008B]`
- `data/scripts/room-036-mansion-e/local-201.txt [01F9]`, `[0203]`; `data/scripts/room-049-road/local-200.txt [0325]`
- `rules/glitchless.md` Out of scope for v1, Banned 3
- `docs/human-route.md` §3.2 steps 7 to 9, 32 to 36, 44 to 45, 71; §4.3
- `docs/part1/model.md` §2, §7.2, §14.3; `docs/part1/skips.md` row A51; `docs/part1/money.md` B4
- `out/runs/20261005T122038Z-run/trace.jsonl` (inventory adds); `out/plans/part1.time.sas_plan`

**Fix and follow-up.** None for this segment. The swordfight segment is tracked in `docs/next.md` "Segment coverage", and it needs `docs/part1/money.md` B4 settled first. The only goal-relevant use of the sword is the `Bit[98]` guide unlock, which `docs/part1/model.md` §14.3 defers; the humans buy the map instead.

### D12. The Fork → Petal screen hop goes through path 686

- **Classification: genuine shortcut (equivalent exit with a closer walk point, found by blind extraction).** New with the final route. It has not been through the skeptic review.
- **Delta:** 0 actions and 0 transitions. In time, −54 ticks per hop against the same hop through 685, measured, so −108 per run.
- **Ours:** #16 and #58, `walk-f218-f215-via-686`: Walk to 686 at the Fork (pseudo-room 218).
- **Human:** H24 (IL 1:51 to 1:54) and H73 move 1, "Back" from the Fork to the petal screen. `docs/human-route.md` §3.2 step 73 calls "Back" the top exit. Which of the two back paths, 685 or 686, the human clicks was not checked. If it is 686, the two rows are `same`.

**Explanation.** 686 is a second back path at the Fork, and it runs 685's exit code.

- The Fork's entry draws 686 at strip 28, next to 685 at strip 15 (`room-058-damnfores/entry.txt [08E5]`, `[08ED]`).
- 686 has no Walk to of its own. Any verb on it runs `startObject(685,11)` (`obj-0686-path.txt [0010]`), and 685's case for 218 moves ego to 215 (`obj-0685-path.txt [0168]`). So the transition is the same as through 685, and so is the arrival: 215, at 687's walk point.
- The only difference is the sentence's walk before that code runs. It goes to 686's walk point instead of 685's. Entering the Fork from the map leaves ego at 687's walk point, and from there the walk to 686 is the shorter one.
- Every forest entry clears class 32 on 685 to 688 (`entry.txt [01BC]` to `[01D1]`), so 686 is touchable.
- The action carries exactly the guards of the generic walk: `¬store-door-open`, `¬cook-provoked` and `¬following-storekeeper` (`docs/part1/model.md` §14.8).

**How it was found.** The hand model generated its forest links from paths 685, 687 and 688 only. The Phase 7 blind extraction rebuilt Part I from the scripts alone and listed Walk to 686 at 218 as a legal alternative (`docs/extraction-diff.md` "Analysis" §3). `docs/part1/model.md` §14.8 checked it against the scripts and added it. Under unit costs it ties with its twin, so only the time objective tells them apart.

**Time.**

- In the §14.8 replay of the first time plan on seeds 1 to 3, with the new exits substituted, the hop from the map took 162 ticks through 686 and 216 through 685 on every seed: −54 per use (`out/measure/20261005T104047Z`, `…104054Z`, `…104100Z`).
- The final run's pooled costs agree: `walk-f218-f215-via-686` 162.0 (n = 1296, stdev 0) and `walk f218 f215` 216.0 (n = 1708, stdev 0) (`docs/optimization.md`).
- On seed 1, #16 takes 162 ticks (trace step 16, 15891 → 16053) and #58 takes 162 (29223 → 29385). The first time plan's seed-1 run took 216 for each (`out/runs/20261005T094110Z-run`).
- Against the first time plan on the held-out seeds, the two hops give exactly −108 on every seed, out of a mean difference of −124.2 per seed (§1.3).
- Seed 1 is only 6 ticks faster than the first time plan's seed 1 (22382 against 22388). The hops give −108 and #27 gives −18 (174 ticks against 192), but #31 gives +120, because the storekeeper was away in this seed-1 run and present in the earlier one (D7).

**Citations:**

- `data/scripts/room-058-damnfores/entry.txt [08E5]`, `[08ED]`, `[01BC]` to `[01D1]`
- `data/scripts/room-058-damnfores/obj-0686-path.txt [0010]`; `obj-0685-path.txt [0168]`
- `docs/part1/model.md` §14.8, points 1 and 2; `docs/extraction-diff.md` "Analysis" §3
- `out/measure/20261005T104047Z`, `…104054Z`, `…104100Z`
- `docs/optimization.md` per-action costs (`walk-f218-f215-via-686`, `walk f218 f215`)
- `out/runs/20261005T122038Z-run/trace.jsonl` and `out/runs/20261005T094110Z-run/trace.jsonl`, trace steps 16 and 58
- `out/optimize/20261005T084713Z/candidates/sample-02/report` and `out/optimize/20261005T111318Z/candidates/sample-16/report` (paired held-out seeds)
- `docs/human-route.md` §3.2 steps 24 and 73

**Fix and follow-up.** No model change; `docs/part1/model.md` §14.8 already added the action. Optional: check in the IL video (1:51 to 1:54) which back path the human clicks at the Fork.

### N1. Checked, not a difference: the jail and store order

- **Classification: not a difference.** In the binary scheme it is a genuine shortcut (agreement): legal, not a bug, and the same as the human's.
- **Delta:** 0.
- **Ours:** #25 to #27 the jail (Otis #1), #28 to #32 the store, #33 to #41 mansion #1, #43 to #46 Otis #2, #47 the cake, #48 to #53 mansion #2.
- **Human:** H38 to H40 Otis #1, H41 to H46 the store, H47 to H55 mansion #1, H56 to H60 Otis #2, H61 on, mansion #2. In room numbers, both sides run 34 → 31 → 34 → 30 → 34 → 36 → 53 → 36 → 34 → 31 → 34 → 36 → 53.

**Why the scripts force jail before store.**

- The store's breath-mint topic appears only if `Bit[420]` is set and `Bit[312]` is not (`room-030-store/local-211.txt [0251]`/`[0256]`).
- `Bit[420]` has exactly two writers, both in the jail: Talk to Otis (`room-031-jail/obj-0405-prisoner.txt [001A]`) and any give he refuses (`room-031-jail/local-203.txt [0351]`).
- The mints are required. Otis takes the repellent only once the mints have been given (`local-203.txt [011F]`; only the mints clear the blocking class, `[00BF]`), and the cake comes from the repellent (`[01E0]`).
- The mints are bought in the store menu (`local-211.txt [1BC0]`). The only other place that gives them is `global/script-001.txt [0AD0]`, the banned boot-param setup.
- So with one store visit, the jail must come first. The other order (store, jail, store again) is strictly dominated. This answers `docs/human-route.md` §5 Q2: Otis #1 is required.
- **Wording, corrected.** "The mints can be bought only together with the shovel, through the door path" is a restriction of our model (D7), not of the scripts. Talk to the storekeeper opens the same menu script 211. Every path to the mint topic still needs `Bit[420]` first.
- The scripts force only jail before store. Where mansion #1 goes is the optimiser's choice by measured mean.

**The optimiser's pool.** In the final run, 11 of the 23 candidates visit mansion #1 before the store, and all 11 then pay through the files topic, which appears while ego owns the repellent 640 (`local-211.txt [032D]`). Ten go mansion #1, jail, store; `perturb-02` goes jail, mansion #1, store. The best of the 11, `perturb-13` (mansion #1 first), had a select mean of 22316.8, against the winner's 22291.4 (`docs/optimization.md` Candidates). In the first run the one other order was `sample-06` (mansion #1, then Otis #1, then the store), at 22404.0 against that winner's 22395.6.

**Seed-1 ticks.**

- Jail block #25 to #27: 858 ticks (walk 480, talk 204 setting `Bit[420]`, walk out 174).
- Store block #28 to #32: 1716 ticks (open 348, enter 6, shovel 396, pay 828 setting `Bit[312]` with the storekeeper away, exit 138 including a 6-tick deferral).

Differences inside these blocks are classified elsewhere: paying through door 387 instead of Talk to 394 (D7) and the human's sword pickup H44 (D11). The drug is no longer in the jail block (D6).

**Citations:**

- `data/scripts/room-030-store/local-211.txt [0251]`, `[0256]`, `[032D]`, `[1BC0]`; `obj-0394-storekeeper.txt [0015]`
- `data/scripts/room-031-jail/obj-0405-prisoner.txt [001A]`; `local-203.txt [00BF]`, `[011F]`, `[01E0]`, `[0351]`
- `data/scripts/global/script-001.txt [0AD0]`
- `pddl/part1/domain.pddl` `talk-to-prisoner`, `pay-for-shovel-and-mints`, `give-repellent-to-prisoner`
- `docs/optimization.md` Candidates (`sample-16` 22291.4, `perturb-13` 22316.8, `perturb-02` 22425.6); for the first run, `out/optimize/20261005T084713Z/report.json` (`sample-02` 22395.6, `sample-06` 22404.0)
- `out/plans/part1.time.sas_plan` #25 to #53; `out/optimize/20261005T111318Z/candidates/perturb-13/plan.keyed.sas_plan`; `out/optimize/20261005T084713Z/candidates/sample-06/plan.keyed.sas_plan`
- `docs/human-route.md` §3.2 steps 38 to 60, §5 Q2

**Fix and follow-up.** None for the model. Record in `docs/human-route.md` §5 Q2 that Otis #1 is required because it sets `Bit[420]`, which gates the mint topic.

## 6. Fidelity gaps and rules decisions

`rules/glitchless.md` lists the pinned settings, the skips, click equivalence, the off-screen rule, the circus quirk, route selection by mean ticks and the known fidelity gap. The input deferral is not in the rules file yet; it is specified in `docs/plan.md` "Plan-player semantics as implemented". This section says how the measured runs meet each item, and which decisions are still open.

### 6.1 Pinned settings

- Every run, headless or demo, uses `enhancements=0`, `talkspeed` at maximum (255), `subtitles=true`, `original_gui=false`, `copy_protection=false` (which bypasses the Dial-a-Pirate wheel), `autosave_period=0`, a fixed `--random-seed` and `--disable-sdl-audio` (`rules/glitchless.md` "Pinned engine settings").
- **Checked at the segment start.** The seed-1 `segment_start` record has var 19 = 6 (`VAR_TIMER_NEXT`: no logo glitch), var 37 = 0 (`VAR_CHARINC` at maximum talk speed), var 24 = 27 (Esc) and var 57 = 46 (`.`).
- **Natural boot only.** `boot_param` is 0. A boot param never qualifies under Allowed 4 for MI1, because any non-zero value switches ScummVM into debug mode (var 39), and the segment start requires `Var[39]` = 0.
- **The segment start moved** from boot tick 12865 (talk speed 60) to 9325, because maximum talk speed shortens the pre-segment intro. Ticks count from the segment start, so totals are unaffected. The RNG state at the start differs, which is one reason the unskipped and skipped runs of a seed are not comparable step by step (`docs/part1/skips.md` §7.2).

### 6.2 Skips (Allowed 5)

- **When.** Esc is pressed only when `abortCutscene()` would find a live override, on the first frame it is live. `.` is pressed only while a real line is showing, never for the map's hover label (`docs/plan.md` Task 8.2 "As implemented"). Nothing is pressed before `segment_start`: on seed 1 the first skip is at tick 10123.
- **Safety.** The skip-safety harness (`tests/integration/test_skip_safety.py`, seeds 1 to 3) diffed bits, inventory and owners at every `step_end` with and without skips. The only differences were `Bit[561]` from the LeChuck exit on and the owner of vase 630 from `enter-idol-room` on (both `docs/part1/skips.md` §4.4 items), plus `Bit[324]`, which is RNG drift at the store. So no step needs `no_skip`.
- **Var 19.** It must be 6 at every `step_end` and after every cutscene skip (the `timer_next` check). This guards against the mid-game members of the logo-glitch family (`docs/research/skips-engine.md` §6).
- **Menus inside overrides.** With Esc on, #51's "Uh"/"Um"/"Blfft" menus and #52's "Buzz off" menu never show. Their answers are `override_choose` entries, so the same plan replays with and without skips (`docs/part1/skips.md` §4.3).
- **What skips cannot shorten** on this route: script 120's `delay(60)` before its override (72 ticks in #8), local-204's store cutscene, which has no override (138 ticks in #31 when the storekeeper is present), local-201's five `delay(60)` at the poodles (300 jiffies in #35) and the cook's 600-jiffy wait (in #4). A human pays all four too.

### 6.3 Input deferral (bridge v2)

- **The rule as implemented.** The plan player stands in for a click that `checkExecVerbs()` would have taken. It takes no input action in a frame whose input is used up. It writes a `defer` record and acts at the next decision point instead. A frame's input is used up in two cases:
  - **`esc_frame`:** the bridge pressed Esc, and `processKeyboard()` writes key 27 over the frame's mouse state, so a player's earliest action after an Esc is the next frame;
  - **`clicks_cleared`:** a script called `clearClickedStatus()` between `processInput()` and `checkExecVerbs()`.

  The text skip `.` uses up nothing.
- Seed 1 has 10 deferrals of one frame (6 ticks) each: in #12 (2), #13 (2), #23 (2), #32, #45, #51 and #54. Six are `esc_frame` and four are `clicks_cleared`.
- **Cost.** On seeds 1 to 10 the mean total went from 22,331 to 22,393 ticks (+62). Single seeds moved by −54 to +186 ticks, because the shifted frames change NPC and RNG timing (`docs/plan.md` "Plan-player semantics as implemented").
- **Rules gap.** `rules/glitchless.md` does not state this rule. It should (§8).

### 6.4 The off-screen rule: an open rules decision

`rules/glitchless.md` "Camera visibility": a sentence may target a touchable object that is off screen, but only if the walk the sentence starts would bring it on screen.

- **Used twice in the plan:** #3, Open 316 from bar-left (D2), and #8, Walk to 315 from bar-right (D4). The rule is also needed for the route at all: the Dock where control starts has no exit on screen after the arrival walk (`docs/part1/rooms.md` §7).
- It excludes Walk to 431 from High Street's town half, whose camera is pinned to c ∈ [528,640] and never shows 431 (`room-034-high-stre/entry.txt [0056]`).
- Every target was checked against the rule using CDHD rects (`docs/part1/model.md` §13.5). That check is about whether the walk *can* bring the target on screen, not about when.
- **The open question (from the D4 split).** In the bar, the camera pans 8 px per frame after ego crosses x 320. One skeptic argues that 315 never comes on screen before the exit fires. If so, the rule's premise fails for #8, and possibly for #3 as well. Neither was measured; the trace has no camera record.
- **Under the rule as written, no label changes.** The rule names the bar halves as its allowed example, so D2 and D4 stay genuine shortcuts.
- **Cost of a strict reading** ("the target must be on screen before the sentence takes effect"). Both actions would fall back to two-input forms:
  - #8 would become the curtain, then a wait for the pan, then Walk to 315: about 210 + 180 + 252 = 642 ticks against 456, so about +190 (estimate).
  - #3 would become the curtain, then Open 316 from the right half: 198 + 282 + 864 = 1344 against 1326 (+18), plus any wait for 316 to come on screen (not measured).
  - In total, at least about 200 ticks per run.
- **Decision needed:** whether the rule's "would bring the object on screen" means "eventually, along the walk" (the current reading) or "before the sentence takes effect". The first step is to record the camera position per frame in the bar on seed 1 (§8).

### 6.5 The cook-freeze gap

`rules/glitchless.md` "Known fidelity gap": a real click on 316 freezes the cook (`room-028-bar/local-203.txt [0030]`), but a pushed sentence does not.

- **Exposure now.** In the time plan, #3's Open already leaves ego at 316. #4's walk-in takes 42 ticks, while the cook walks away toward targets at x ≤ 285. local-218's cutscene then freezes him (`global/script-018.txt [0070]`), and the room change ends his script.
- **Status.** #4 took 864 ticks in all 1445 measured samples, so no race was lost.
- **Rules text is stale.** It names `walk-into-kitchen` as the race window; the time plan uses `walk-into-kitchen-after-provoking-cook`. The gap can still only make things harder for the bot.

### 6.6 The circus inventory quirk

- **The quirk.** The tent's input script reads `Var[134+k]` for slot k, while the display fills `Var[133+k]` (`room-051-circus-te/local-200.txt [0107]`; `global/script-009.txt [0092]`). So the helmet works only by clicking the slot just before the pot (D5).
- **Allowed by name.** `rules/glitchless.md` lists it under "Script quirks reached through clicks are allowed". The helmet effect exists only in the input script (`[008B]`), so it is reached with coordinate-free verb-slot clicks through `runInputScript(kVerbClickArea, verbid, 1)`.
- **What the model needs.** An item must be held and displayed before the pot, hence `pot-guarded-by` and the D3/D5 coupling.
- **No scrolling.** At most 7 items are displayed before the payout, so the slots never scroll. The money object is hidden (owner 14) while `Var[195]` = 0, so the −1 offset works (`docs/part1/model.md` §7.6).

### 6.7 A fixed plan against chance events

- The storekeeper is away on 1 store entry in 4. The plan cannot branch, so it uses the door path on every seed, which costs about 100 ticks on the mean (D7). This is also the main source of spread: 708 ticks present, 828 away.
- Map pirates can appear from the 4th map entry (#56). The `map-pirate` interrupt answers the road menu and restarts the interrupted walk (`docs/part1/model.md` §7.2; `tests/integration/test_interrupts.py`, seed 292). None appeared on seed 1.
- The cook is now deterministic. Provoking him makes #3 and #4 take 462 and 864 ticks on every sample. The unprovoked timer ranged over 1080 ticks in the old runs.

### 6.8 Route selection by mean ticks

- The winner was chosen on select seeds 1 to 30 (22291.4) and reported on held-out seeds 31 to 60 (22292.0) (`docs/optimization.md`; `rules/glitchless.md` "Route selection").
- On the held-out seeds the runner-up `perturb-04` was 30.8 ± 13.5 ticks per seed slower (standard error). The winner was faster on 23 seeds and the runner-up on 7. `perturb-04` has the same 67 actions in another order: it fetches the petal and drugs the meat before the circus, not after. Its select mean was 22299.4, 8.0 ticks above the winner's.
- The winner's twin with the drug in the jail, `mean`, was 11.6 ± 14.9 ticks per seed slower on the select seeds, a tie within noise (D6). The held-out seeds are run only for the winner and the runner-up, so `mean` has no held-out figure.
- The keyed surrogate predicted 22298.8 for the winner, 7.4 ticks above its measured select mean.

### 6.9 Headless against demo, and audio

- `rules/glitchless.md` requires a run to produce the same tick count headless and in the visible demo. Seed 1 does: the goal and end records match, and the `state-end.json` files are byte-identical (§4.5).
- Audio is silent and tick-locked (`--disable-sdl-audio`; the bridge advances audio in game ticks), so sound-dependent waits take the same ticks in every run. Both runs end with `audio_frames` 11652096. The cost is that the demo is silent.

## 7. Where the ticks go now

Seed 1, `out/runs/20261005T122038Z-run/trace.jsonl`, using the row convention of §4.5 (until-waits folded in, #12's two pushes merged). The rows sum to 22382.

| plan action | ticks | share | seconds | what takes the time |
|---|---:|---:|---:|---|
| #13 walk-out-of-tent-after-helmet-meat | 1062 | 4.7% | 17.7 | two verb-slot clicks, the helmet and cannon cutscenes, the reversed menu, the 478 payout and the scripted walk-out; 8 text and 3 cutscene skips |
| #9 walk dock lookout | 1008 | 4.5% | 16.8 | the full width of the Dock, from the bar door to the cliffside (from the pier, #55 takes 642) |
| #22 walk dock low-street | 930 | 4.2% | 15.5 | from the map's arrival point on the Dock to archway 427 |
| #4 walk-into-kitchen-after-provoking-cook | 864 | 3.9% | 14.4 | 822 waiting for the cook (the rest of local-212's 600 jiffies after his line, then his walk out to x ≤ 310), then 42 walking in |
| #12 walk-into-tent-with-pot | 848 | 3.8% | 14.1 | the walk in (stopped once by local-202), the Fettucini argument and 3 menus; 16 text and 3 cutscene skips |
| #31 pay-for-shovel-and-mints | 828 | 3.7% | 13.8 | the walk to door 387, local-204's cutscene (no override), 3 picks; the storekeeper was away on seed 1, and the step takes 708 when he is present |
| #51 steal-idol | 726 | 3.2% | 12.1 | the walk to the hole, the theft cutscene and 1 pick; 8 text and 4 cutscene skips |
| #21 walk melee-map dock | 714 | 3.2% | 11.9 | the map walk to the village |
| #42 walk high-street-mansion high-street-town | 642 | 2.9% | 10.7 | the scripted walk through 435 back to the town half |
| #55 walk dock lookout | 642 | 2.9% | 10.7 | from the pier end of the Dock to the cliffside |
| #35 give-meat-to-poodles | 636 | 2.8% | 10.6 | the walk to the dogs, then local-201's five `delay(60)` (300 jiffies, no override) |
| #33 walk high-street-town high-street-mansion | 594 | 2.7% | 9.9 | the walk from the store door to archway 436 and the scripted walk to the mansion half |
| the other 55 actions | 12888 | 57.6% | 214.8 | |
| **total** | **22382** | 100% | **373.0 (6:13.03)** | |

Shares are rounded.

- **Walking now dominates.** The 32 plain `walk a b` transitions take 11376 ticks (50.8%). With the 16 `walk-*` actions (the 14 composites and the two hops through 686), the 48 walk actions take 17288 ticks (77.2%). The 19 other actions take 5094 (22.8%).
- The three Dock walks (#9, #22, #55) alone take 2580 ticks (11.5%). The nine forest hops (#58 to #66) take 2124 (9.5%).
- This is the reverse of the unskipped run, where the ten largest actions were 78.5% of 107,660 ticks and every one was mostly cutscene or dialogue (Appendix). Skips cut those to a few hundred ticks each: the idol theft went from 18,744 ticks to 726, the idol pickup and the Elaine scene from 12,738 to 378, and the idol room from 10,140 to 60.
- **What is left to gain within the rules is small.** Glitchless walking speed is fixed. The known levers are Talk to the storekeeper when he is present (D7, about 100 ticks on the mean, estimated) and, at most 6 ticks each, inventory-only sentences during scripted walks (D6). The deferrals (+62 on the mean) are the price of input fidelity, and a strict camera rule would add about 200 (§6.4). The held-out stdev is 48.3.

## 8. Follow-ups

The project-level follow-ups are in `docs/next.md`. The ones this comparison bears on are "Segment coverage" (the swordfight, D11), "Optimisation" (seed-dependent durations as chance outcomes, D7) and "Fidelity and categories" (glitched categories, coordinate input and reaction time, §1.4). The specific items below come from this report.

**Rules decisions**

1. **The camera rule's premise** (§6.4; the D4 split, possibly D2). Decide whether "would bring the object on screen" means along the walk or before the sentence takes effect. First record the camera position per frame in the bar on seed 1. A strict reading costs at least about 200 ticks per run.
2. **State the input deferral in `rules/glitchless.md`** (§6.3). It is now only in `docs/plan.md`.
3. **Update the "Known fidelity gap" text** to name `walk-into-kitchen-after-provoking-cook` and the much smaller window (§6.5).

**Plans and measurement**

4. **A conditional store step** (D7): Talk to 394 when the storekeeper is present, Walk to 387 when he is away. Then measure the estimated saving of about 100 ticks on the mean. This is the reactive-plan mechanism `docs/next.md` also needs for the swordfight.
5. *Optional.* Push an inventory-only sentence during a scripted walk (D6; #18 and #47, at most 6 ticks each).
6. *Optional.* A headless replay of [7,566,467] at the poodles (D8), and a paired measurement with the cake opened in the jail (D10).

**Human comparison**

7. **Reaction time** (`docs/next.md`). The bot acts on the first frame a human could. A configurable reaction delay would give a "human-plausible" comparison.
8. **A glitched category** (`docs/next.md`). Modelling the logo speed glitch in a separately labelled category would turn §1.4's bound on the glitch's share of the gap into a measurement.
9. *Optional video checks.* IL 0:21 to 0:27, to confirm that the "generic failure line" is the cook's line (D2). IL 3:42 to 3:45, for a "Walk to door" sentence line (D9). The WR's map arrival before the detour (§4.3). IL 1:51 to 1:54, for which back path the human clicks at the Fork (D12).

**Documentation corrections.** These have not been applied to the files they concern.

- `docs/human-route.md`:
  - Summary and §4.2: 47 transitions, not "48 (47 with the intro skip)" (D1).
  - §3.2: fix the "Start" paragraph and relabel step 1 as [S], citing `room-038-lookout/local-203.txt [02CB]` and `room-096-part1/local-200.txt [003B]`. The scripted count goes from 3 to 4 (D1).
  - §4.3: 53 transitions as played, not "54 (53 with the intro skip)" (D1).
  - Step 45 and the §4.1 picks table: 3 picks for mint and shovel, not "about 4" (D7).
  - Step 52 and §4.1 row 15: split into "Open door" and "Walk to door", giving 23 actions (D9).
  - §2.3: the row calling `.` and Esc "out of scope for v1" is stale.
  - §5: Q2 is answered (Otis #1 is required, N1), Q4 is answered (the Open is not required but is faster, D2), and Q8 is answered (`Bit[85]` is set at the underwater pickup, #53).
- `docs/part1/model.md`:
  - §9: counts for the time plan (23 actions and 45 transitions, against 23 and 47), with `walk-to-kitchen-door-provoking-cook` as a fourth composite. Also "falls away with the intro skip" becomes "is scripted on every boot" (D1, D2).
  - §8 and §9: label the step lists and the "−1: pot and meat in one sentence" note as the unit-cost plan (D3), and note that the time plan drugs the meat at the petal screen (D6).
  - §5: reword the "Use meat with poodles" row. It is an exact tie kept to one variant, not a riskier one (D8).
  - §14.1: note that its until-wait convention differs from `docs/optimization.md` (§4.5).
- `docs/part1/idol.md` §2.5 and open question 3: box flag 128 is invisible, not locked (D8).

## Appendix: the unit-cost route (before Phase 8)

The first version of this report compared the unit-cost plan with the human route. That plan is the `v1-unit-cost` checkpoint in `docs/plan.md` Phase 8: the optimal Fast Downward plan at unit cost (`uv run speedrun plan part1`), cost 66 = 66 actions (48 `walk*` and 18 others), compiled to 67 steps. It was measured without skips, at `talkspeed=60`, on bridge v1, and chosen by action count rather than by ticks.

**Counts.**

| | ours, naming rule | ours, human-comparable | human, as listed | human, corrected |
|---|---:|---:|---:|---:|
| actions | 18 | 21 | 22 | 23 |
| room transitions | 48 | 46 | 48 | 47 |

The differences were −1 action (old D2, no kitchen Open), −1 action (old D3, one-sentence pickup), +1 action (old D8, the idol-room Walk to) and −1 transition (old D4, the bar exit).

**Ticks** (natural boot, segment start at boot tick 12865):

| seed | 1 | 2 | 3 | 4 | 5 | mean | stdev |
|---|---:|---:|---:|---:|---:|---:|---:|
| ticks | 107660 | 107054 | 106874 | 107366 | 107366 | 107264 (29:47.73) | 306 |

Seed 1 ran in `out/runs/20261004T213031Z-run` (with a byte-identical trace in `20261004T205705Z-run`). Seed 2 came from `tests/integration/test_determinism.py`, with no run directory. The previous plan (cost 67) had demo parity at 106892 ticks (`20261004T201241Z-run` against `20261004T201330Z-demo`). The unit plan itself never got a demo run; the time plan now has one (§4.5).

**Where the ticks went (seed 1, 107660).** The ten largest actions were 78.5% of the run, and every one was mostly cutscene or dialogue:

| old action | ticks | now (time plan, seed 1) |
|---|---:|---:|
| #50 steal-idol (4 menus with Fester and the Governor) | 18744 | #51: 726 |
| #52 walk-up-ladder-taking-idol (the Elaine scene) | 12738 | #53: 378 |
| #38 enter-idol-room (local-210) | 10140 | #39: 60 |
| #7 walk bar-right dock (the LeChuck cutscene) | 9696 | #8: 456 |
| #11 walk-into-tent-with-pot (the argument and 3 menus) | 9308 | #12: 848 |
| the other 61 actions, plus the cook wait before #4 | 47034 | |

**RNG.** That audit's model change saved exactly 54 ticks on every seed, but per-seed totals moved by −294 to +768. The seed spread was 786 ticks, mostly the cook's timer (a range of 1080) and the store (690 on the pay step). This is why `rules/glitchless.md` now chooses routes by mean ticks over many seeds.

**The old differences, and where they went:**

| old ID | old classification | now |
|---|---|---|
| D1 Lookout → Dock | genuine shortcut (counting convention) | **D1**, unchanged |
| D2 no kitchen Open: wait for the cook's own timer (1884-2964 ticks) | genuine shortcut (skips an intended step); a time saving of 600-1700 ticks was estimated for the Open | **reversed by D2**: the time plan makes the Open, from bar-left, saving about 1532 ticks against the unit plan |
| D3 one-sentence pickup, `use-meat-with-pot` (−1 action) | genuine shortcut (one-sentence pickup) | **replaced by D3**: two pickups, +1 action, −24 ticks |
| D4 bar exit in one sentence | genuine shortcut (off-screen exit); "−6 ticks" and a floor-click caveat | **D4**, the same label; the time comparison and the caveat are corrected, and the skeptics now split |
| D5 circus helmet | genuine shortcut; skeptics split 1-1 | **D5**, now upheld 2 of 2 |
| D6 store menu through the door | genuine shortcut (different method, equal cost) | **D7**, reclassified: a fixed-plan limitation, and slower than the human's Talk to by about 100 ticks on the mean |
| D7 poodles, Give for Use | genuine shortcut (equivalent verb, exact tie) | **D8**, unchanged |
| D8 idol room, Open plus Walk to | genuine shortcut (counting convention) | **D9**, unchanged; the script reading is corrected for the skipped path |
| D9 drugging the meat in the jail | genuine shortcut (different order; not reviewed) | **D6**, now reviewed 2 of 2 for the jail placement; the final plan drugs at the petal screen |
| D10 swordfight and crew steps | genuine shortcut (out of scope; not reviewed) | **D11**, now reviewed 2 of 2; the Fester time note is corrected |

**Claims of the old report that no longer hold:**

- "Our ticks cannot be set against the human times." Both sides now skip, and §1.2 and §4 re-base the human times to dock control and remove the out-of-scope detours. The remaining caveats are the logo speed glitch and the version.
- "The Open on 316 only speeds the cook up." It is the largest time gain of the time plan (D2).
- "Allowed 2 permits only values read at segment start", as the reason for the store door path. Withdrawn: the draw happens at store entry, and the real reason is the fixed plan (D7).
- "Owning the sword lengthens Fester for the humans." Under Esc it does not; only the underwater block does (D11).
- The old D8 citation "class 6 means locked" was right. Script 25 opens 632 only while class 6 is clear (D9).






