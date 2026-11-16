# E031: the calculated estimate of the optimizer's final time

Date: 2026-11-16, audited and corrected the same day (section "Audit" below). Model: `estimate.py` in this folder
(plain Python 3, standard library only, fixed seed 31). Run it with `python3 estimate.py`. It takes about two minutes
on the loaded laptop. Its whole output is pasted at the end of this file. Every number below comes from that output.

## The result in short

| | CD track 6 present | CD track 6 removed |
|---|---|---|
| Median final time | 1:03:19.9 | 1:01:04.8 |
| 10th to 90th percentile | 46:39.1 to 1:37:04.4 | 44:24.3 to 1:34:47.4 |
| Odds of under 60:00 | 44.6% | 48.4% |
| Odds of under 30:00 | 7.4e-06% | 0.00117% |
| Odds of under 28:01.4 (hand route, no slide) | 2.4e-08% | 1.5e-05% |
| Odds of under 23:11.3 (hand route, slide, track 6) | 0 | 0 |
| Odds of under 21:01.483 (hand route, slide, no track 6) | 0 | 0 |
| All assumptions pessimistic: median | 2:22:22.0 | 2:19:41.4 |
| All assumptions optimistic: median | 25:52.4 | 23:42.4 |
| All assumptions optimistic: odds of under 21:01.483 | 0 | 2.18% |
| All optimistic, success draws strongly correlated (rho 0.9): odds of under 21:01.483 | 0 | 13.2% |

- "0" means no pair of draws among 150,000 x 150,000 in any of the four strata. The best draw of the route side is
  29:29.8 without track 6. The largest draw of the gains side is 292.3 s. Even that pair gives 24:37.5.
- The model says the optimizer most likely lands between 45 and 95 minutes, 2 to 4 times shorter than full5
  (3:01:51 on the same clock). The median is about 2.2 times the hand route without the slide and 2.9 times the
  21:01.483 run.
- Beating 21:01.483 is out of reach unless almost every assumption sits at its optimistic end at once. Only there
  does it appear: 2.18% with independent success draws, 7.8% to 13.2% if the stages succeed or fail together.
  With the assumptions drawn from their ranges it did not occur once, at any correlation tried.
- The target needs two things at once: the route's structure at hand quality (R within about 50 s of the no-slide
  hand run) and the whole item-slide chain (monkey skip above all). R's best possible value is the hand route's
  structure; G's largest value is the hand-known gains plus at most 57 s of walking.
- In 4.44% of draws the optimizer never answers the closing menus. Then there is no Any% time. Those draws count as
  failing every line.
- If every stage works and every parameter sits at its centre, the time is 44:08.4 with track 6 and 41:58.6
  without. The median is slower because some stage usually fails.
- Odds below about 1% depend on the shape of the assumed ranges and on correlation (output tables 9 and 9b). Read
  them as orders of magnitude.

## Audit (2026-11-16)

The first version gave a median of 53:57.1 (track 6) and 0 odds for 21:01.483. A sceptical re-check found these
errors, all fixed in `estimate.py`:

1. **`q_delay` was not measured.** It was 7/8 because "a working delay existed at 6 of 6 points". At every one of
   those points the working delay included d = 0, the reference's own timing, which works by construction.
   Without it, a working delay in 1..12 exists at 2 of 6 points (no fight choice; one catacomb exit at d = 5, the
   other at d = 7-10; `gf/out/fight3.json`, `late.json`). `q_delay` is now a measured range: 0.375 (2 of 6 points,
   rule of succession), 0.59 (the last fight draw alone: 2 of 30 delays, 13 tries), 0.83 (one catacomb exit). At the
   centre the chance that every draw point is solved again falls from 73.8% to 39%.
2. **The direct-walk pool counted idle frames twice.** Its busy share was taken as 1 - 6 x 403 / 34,181 = 0.929. The
   frame log joined with the end path gives 24,474 busy frames of 34,181 (0.716): 9,707 frames are idle, and
   re-timing already removes them. The pool falls from 1,974 s to 1,503 s (it would be 1,124 s if direct walks had no
   idle at all).
3. **The in-room drop pool's busy share was too low.** The 16,808 frames are counted with padding removed, but
   `b_inroom` (0.3-0.6) applied the idle share of whole steps. The same join gives 0.653 (all 377 in-room no-gain
   steps without their idle tails) to 0.702 (the 205 in visits with no gain). It is now a measured range; the drop
   pool rises from 683 s to 1,158 s. Fixes 2 and 3 nearly cancel in size, but the drop pool is harder to remove
   (x_drop 0.584 and the drop stage needed), so the net removal at the centre falls from 76.3% to 71.7% of the detour.
4. **Shared mechanisms were drawn as independent.** The delay scan serves R (draw points, the store entry) and G
   (luck); the walk stage serves R (`e_walk`) and G (walking beyond the hand route). The model claimed R and G share
   nothing. It now mixes four strata (scan built or not, walk stage built or not); within a stratum R and G are
   independent. Without the scan, draw points rely on the frozen search alone and the store entry is kept with
   1 - (41/61)(1 - q_search). `e_cut` (in-visit drops, dialogue paths) now needs the executor and drops.
5. **Double count of the store entry.** It was both Credit Early's own term and one of the `n_dp` draw points.
   `n_dp` is now 7-19 (was 8-20).
6. **One penalty for any number of failed draw points.** `pen_draw` now scales with the expected count of failed
   points given at least one.
7. **Gains made by a route that was never rebuilt.** Without a re-timer the route stays as full5 played it, yet G
   was still subtracted. The percentiles now take G = 0 there (the odds were not affected: those draws are above 3 h).
8. **The feather hold used a blind joint scan.** 423,522 tries assumed all four inputs scanned together. The hold is
   found step by step on the input counter, the mechanism of the carry's keep-alive readings, so it now uses
   `sig_keep`. Its centre probability rises from 16.5% to 24.5% (worth 5 s).
9. **Upper ends below hand quality.** `e_walk` (0.9), `e_cut` (0.8) and `e_tune` (0.8) could never reach the hand
   route even at their best, which alone made 21:01.483 unreachable in the optimistic corner. Their upper ends are
   now 1 (hand quality); the centres are unchanged. This is the main reason the all-optimistic odds rose from
   0.069% to 2.18%.
10. **Independence of the success draws.** Kept in the main run, but table 9b now shows a common-factor (Gaussian
    copula) version. Correlation moves the median down (63 to 54-57 min) and the optimistic-corner odds up, never the
    drawn odds of 21:01.483.
11. Text: the atom-changing no-news steps are 279 - 72 = 207, not 205.

Checked and found right: every segment value against `tstruct/segments.json` (all 17 x 3 rows, the splits against
the no-slide route); waiting inside extra visits (re-ran `rep4.py`: 3,714.6 s); the waiting pools (sum 5,526.3 s);
the 403 room-change steps and 34,181 frames (re-counted from `path.pkl`); the reproduction of both hand anchors; the
stratified Monte Carlo (the 'all found' stratum weighted by its probability plus the plain draws that missed
something is an unbiased estimate of R's distribution; the sampler matches the plain function to 1e-6 s). Not
re-derivable from the files at hand: the 279-step / 16,808-frame drop pool and the 13,280 direct-walk frames (the
graph report's own definitions); my own joins give pools of the same size.

## What is estimated

- **Clock.** The Any% clock of the hand runs: quarter-frames / 239.992156448.
- **Start.** The first key pressed in the logo room. If no key is pressed there, the frame the logo room ends.
- **End.** The first frame of the final fade. full5 stops 40.3 s earlier, at its end flag, and answers no closing menu.
- **True end.** full5's reported end (3:12:58) came 614.8 s after its end flag first held. The model starts from the
  true end flag (frame 108,830). That saving is certain and already in the baseline.
- **Two variants.** CD track 6 present (compare with 28:01.4 and 23:11.3) and removed (compare with 21:01.483).
  Removing the track is a files condition. No search finds it. It takes 129.84 s off the last card.
- **What "the optimizer" means here.** The stages of the plan: measure the true end, re-time, boot scan, re-route
  (direct walks, drops, order), in-visit execution, the delay scan (draws, luck), walking, and the glitch scans.
- **The hand routes are yardsticks only.** The model uses them to say what "hand quality" is in each segment.
  The optimizer never reads them.

## The model in plain language

The final time is `T = R - G`.

- **R** is the route's time when its structure is brought toward the hand route without the slide. At its best R is
  that run: 28:01.4 with track 6, 1,551.55 s (25:51.5) without.
- **G** is what is gained beyond that run: luck at random draws, tuning, walking, and the item slide with its skips.
  With every hand-known gain at its hand value, G is 290.18 s. Then R - G is 21:01.4. The model reproduces both
  anchors (output table 7).
- R and G share two stages, the delay scan and the walk stage. Results are a mixture over the four combinations of
  those two; within each, every draw of R is combined with every draw of G.

### 1. Segments

The game is cut at 17 points where full5 and both hand routes hold the same progress facts (output table 1). For
each segment the measurements give full5's time in four classes: waiting, walking, cutscene and text, acting. They
also give the two hand routes, and three parts of full5's excess over the no-slide route:

- **wait+**: full5's waiting above the reference's. 5,436.1 s in all. 67% of full5's waiting lies inside extra visits.
- **detour**: full5's busy time in room visits beyond the reference's count for that room and segment. 3,239.3 s,
  between 2,767 and 3,686 s by which visits count as extra.
- **rest**: what is left. It is full5's play inside the visits both routes make. It also holds the frame length:
  full5 runs 6 ticks a frame, the hand routes 5.

### 2. The route side R

- **Re-timing** removes waiting. The pool splits by what the waiting follows (measured): 3,378.9 s after probes and
  calm, 1,254.3 s between the clicks of one trial, 893.1 s in explicit waits. The first part goes entirely. The
  other two go by the assumed shares `r_inner` and `r_xwait`. At the centre 96.4% of the excess waiting goes.
- **Re-routing** removes the detour in three pools:
  - *Direct walks.* The route's 403 room-change steps with no gain hold 2,457.4 busy seconds in 34,181 frames.
    Direct walks measured in the tree take 13,280 frames. The busy time saved is 1,503 s, 46.4% of the detour.
    `v_walk` is the share realised.
  - *Drops of no-gain steps.* 279 in-room steps in rooms left without progress hold 16,808 frames, padding removed;
    65-70% of them are busy (measured). That is 1,158 s, 35.8% of the detour. 72 of the steps only walk or look and
    always drop. The other 207 changed some world atom. `q_atom` is the share of those that can go.
  - *Unneeded progress and order.* The remaining 17.9% (578 s). Drop tests on progress trials remove the share
    `y_need`. A re-ordering stage removes the share `y_order` of what is then left.
  - If no executor exists, only loop cuts and corridors remain: the share `phi_fb`.
- **In-visit execution** removes part of the rest: `e_walk` for walking (needs the walk stage), `e_cut` for
  cutscene, text and acting (needs the executor and drops).
- **Frame length.** With the logo glitch every walking frame costs 5 ticks, not 6. Other busy time scales by
  `1 - u_frame / 6`. Without the glitch the whole route, hand-quality part included, is scaled back to 6.
- **Draws.** Re-timing moves every later random draw. The early safe opening may be lost (plus 58 s). Each draw
  point not solved again adds 120 s at the centre. A point is solved by the delay scan (`q_delay`, measured) or by
  the frozen search (`q_search`); without the delay scan only the search is left.
- **Interactions.** Whatever excess over the hand route is left grows by the share `iota`.
- **Intro and ending.** Intro skipped: S00 is 0.42 s. Not skipped: 167.7 s (37.9 s without track 6). The ending
  adds 40.27 s.

### 3. The gains side G

- **Luck by delay** (needs the delay scan). The slide run gets through the catacomb exits in 6.58 s, the no-slide
  run in 46.50 s. A 13-delay scan sees 5 to 7 random states; each gives the wanted exit with chance 1/4, so the share
  steered is 1 - (3/4)^6.1 = 0.826 of 39.92 s. The fights add 2 to 11.25 s.
- **Tuning.** The two hand runs differ by 31.4 s in stretches with no skip, and by 78.7 s of "order and tuning" in
  S11. The optimizer closes the share `e_tune` of the part that needs no skip.
- **Walking beyond the hand route** (needs the walk stage). 884 s of hand walking x the measured re-click rate
  (2.8% to 7.2%) x the share the hand route does not already hold x the share that survives later draws.
- **Head variant and necklace skip.** 6.5 s. Timing only.
- **The item slide family.** The chain is: formed, carried into Part III (pays the setup, 23 s), then its uses.
  The slide is adopted only if the chain pays. The feather skip has its own path through a regain at the village.

### 4. How each glitch probability is calculated

A glitch is found when three things hold. The scan covers the trigger. A general signal shows the hit. The
valuation bridges the delay to the payoff. Windows and delays are measured on the reference replay (evaluation only).

| Glitch | Trigger window | What a scan tries | Signal | Payoff delay | Probability (centre) |
|---|---|---|---|---|---|
| Logo speed glitch | 47 frames (boot 46-92), 1 key of 8 | Esc at each of 960 logo frames | frame-length variable; ticks | 0 frames | cover 1 x signal 1 x `p_logo_surv` = 80% |
| Credit Early (already found) | no frame window; keeper away on 20 of 61 entry frames | 13 entry delays, then one sentence repeated | ordinary progress | 105 goals, 385 s | with the delay scan (1 - (41/61)^13) x `p_adapt` = 89.5%; without it 77.9% |
| Draw points solved again | fixed continuation works at d in 1..12 at 2 of 6 points | 13 delays, else the frozen search | the draw's outcome | within a step | `q_delay` 0.375-0.83; all 11 points: 39% with the scan, 8.6% without |
| Slide formed (store) | 5 of 25 start frames; 2 of 72 verb-slot pairs; 11 of 240 floor points | 9 x 9 pairs x 25 frames x n floor points | input counter above rest; 0 false alarms on full5 | no gain by itself | 1 - (229/240)^n: 28.0% (n = 7), 84.7% (n = 40), 100% (n = 240) |
| Slide carried into Part III | 6 deliberate readings; 13 of 85 events frame-exact | 72 pairs x about 30 frames at each point where it would die | counter stays raised | 73 goals, 127 s, setup cost first | `p_ts` x `sig_keep`^6 = 0.3 x 0.7^6 = 3.53%; with formed 2.99% |
| Slide uses in S08, S09 | input inside cutscenes | window scan | room or progress earlier | short | `p_part2` 50% given carried; 1.49% overall |
| Oars skip | 13 of 14 frames and 4 of 128 points; 14 frames and 6 of 104; 19 frames; 12 of 61 | 14 x 128, 14 x 104, third click, 61 | ego absent with input on; room differs | 3 goals, 8.3 s | 1 x `c3_oars` x 1 x `s_step` = 0.48 given carried; 1.43% overall |
| Monkey skip | reading 2 of 61 frames, 1 of 72 pairs; then 4, 3, 13 of 61 | 72 x 61, then 1,847 tries per hit | frozen script slots with input on | 1 goal, 3.9 s | `cov_in` x `cov_fr` x `s1` = 0.105 given carried; 0.314% overall |
| Route order around the skips | - | re-routing of S11 | - | - | `p_order` 50% given the monkey skip; 0.157% overall |
| Slide regained, feather skip | regain 13 of 51 frames; hold across a dialogue 1 of 33, 1 of 23, 4 of 31; click 2 of 61 | stepwise on the counter (about 2,400 tries) | counter; object owned | 57 goals from the regain | `p_regain` x `sig_keep` = 24.5% |
| Head variant, necklace | 25 of 77 standing points; 2 frames | arrival-frame comparison | none | 10 goals, 20.9 s | `p_head` 30% |

The whole slide chain at the centre has probability 0.0377%.

### 5. Tags

Every parameter carries a tag (output table 3).

- **M**, measured: 35 values, each with its source.
- **D**, derived: 22 values, each with its formula.
- **MR**, a measured range that is drawn: 10 ranges (now including `q_delay` and `b_inroom`).
- **A**, assumed: 42 parameters, each with a low, centre and high value, the reason, and the cheapest measurement
  that would replace it.

### 6. The Monte Carlo

- 150,000 draws of R and 150,000 of G in each of four strata (delay scan built or not, walk stage built or not),
  mixed by the strata's probabilities (both built: 49.3%). Assumed parameters and measured ranges are drawn from a
  triangle (low, centre, high). Success draws use the calculated probabilities.
- A parameter that is the probability of one success draw enters through the mean of its range. That gives the same
  distribution as drawing it first.
- R is computed twice per draw. Once with every success draw succeeding, weighted by the probability of that (the
  "all found" stratum, 4.67% of the weight with both shared stages built). Once with the success draws as they fall.
- The rare step of G (the slide carried into Part III, 2.82% on average) is not drawn. Both outcomes are kept with
  their probabilities.
- All pairs are combined within each stratum. This is how odds far below 1 in 150,000 can be shown.
- Percentiles come from 150,000 paired draws, each with its stratum drawn.

## Results

### The staircase at the centre (output table 6)

Each step if it and all before it succeed. All parameters at their centres.

| Step | Track 6 | No track 6 | p(step) | p(all so far) |
|---|---|---|---|---|
| full5 as found, end menus answered | 3:01:50.9 | 2:57:31.3 | 97% | 97% |
| + re-timing | 1:34:28.6 | 1:30:09.0 | 97% | 94.1% |
| + intro skip | 1:31:41.3 | 1:29:31.5 | 97% | 91.3% |
| + direct walks | 1:08:13.7 | 1:06:03.9 | 75% | 68.5% |
| + drops of no-gain steps | 56:29.7 | 54:20.0 | 85% | 58.2% |
| + drops of unneeded progress | 53:29.1 | 51:19.4 | 100% | 58.2% |
| + order | 51:22.7 | 49:13.0 | 45% | 26.2% |
| + in-visit execution (walk stage) | 50:06.2 | 47:56.4 | 60% | 15.7% |
| + logo speed glitch | 44:28.9 | 42:19.2 | 80% | 12.6% |
| + interactions | 45:47.9 | 43:38.2 | 100% | 12.6% |
| (delay scan: Credit Early kept, draws solved) | 45:47.9 | 43:38.2 | 29.7% | 3.73% |
| + luck by delay | 45:10.5 | 43:00.7 | 100% | 3.73% |
| + tuning | 44:42.2 | 42:32.4 | 100% | 3.73% |
| + walking beyond the hand route | 44:19.9 | 42:10.1 | 100% | 3.73% |
| + head and necklace | 44:13.4 | 42:03.6 | 30% | 1.12% |
| + feather path | 44:08.4 | 41:58.6 | 24.5% | 0.274% |
| + whole slide chain | 41:40.4 | 39:30.6 | 0.0377% | 0.000103% |

Where the time goes at the centre:

- Re-timing is the largest step: 87 minutes. It is also the surest.
- Direct walks and drops are next: 23 and 12 minutes. They need the executor.
- Everything after that is worth about 12 minutes together.
- The whole slide chain is worth 2.5 minutes at the centre, at odds of 1 in 2,650.
- The draw points are now the weakest common step: all solved again only 39% of the time with the delay scan.
- With every stage working the detour is 71.7% gone. The reference's visit count would be 100%.

### The ends of the ranges and correlation (output tables 9 and 9b)

- All assumed parameters pessimistic: median 2:22:22.0 with track 6, no draw under 60:00.
- All optimistic: median 25:52.4 with track 6 and 23:42.4 without. Odds of under 28:01.4 with track 6: 72.4%. Odds
  of under 23:11.3 with track 6: 2.2%. Odds of under 21:01.483 without track 6: 2.18%.
- Flat ranges: median 1:08:08.6 with track 6; odds of under 30:00 without track 6 0.049%.
- Correlated success draws, parameters drawn: median 57:31.1 (rho 0.5) and 54:15.0 (rho 0.9) with track 6; odds of
  under 60:00 54-63%; 21:01.483 still 0. All optimistic and correlated: 7.84% (rho 0.5) and 13.2% (rho 0.9) of
  under 21:01.483 without track 6.

### Sensitivity (output tables 10, 11, 12)

- **Table 10.** One parameter pinned at an end, the others drawn. The odds of under 21:01.483 stay at 0 for every
  one of the 42 parameters, at both ends. No single assumption makes the target reachable.
- **Table 11.** Every parameter optimistic, then one moved to its pessimistic end. The all-optimistic odds are 2.17%
  at this sample size. Each of `v_walk`, `q_atom`, `r_xwait`, `r_inner`, `e_tune` and `sv_draw` alone brings them to
  0; `h_new` and `sig_keep` to near 0; `y_order` to x 0.09.

What moves the median (table 10, track 6, pessimistic to optimistic end): `p_exec` 1:13:16.8 to 57:56.5, `v_walk`
1:09:34.6 to 56:14.9, `pen_draw` 1:10:12.8 to 58:19.5, `q_search` 1:09:13.7 to 58:37.1, `p_drop` 1:08:31.4 to
58:52.8. The draw-point terms are now among the five largest: the cheapest measurement worth doing first is one
corridor run of the frozen binary from a shifted stop at the catacombs (6-15 minutes), and a 13-delay scan with a
re-solved continuation at the four fight choices.

## What the model cannot know

- **Unknown glitches.** Set to zero in every table. Output table 13 shows what an unknown gain would have to be. To
  reach 21:01.483 without track 6 the median outcome needs 2,403 s more, the 10th percentile 1,403 s. An unknown gain
  of 600 s, found for certain, gives odds of 0.0048%; 1,200 s gives 3.76%.
- **One concrete candidate.** full5 has two 14-frame stretches where the frame length is 1. If a key could end the
  script that holds it, walking would speed up far more than with the logo glitch. The frame log shows input off
  there, so nothing is expected. A scan of 420 tries settles it. It is not in the odds.
- **A better structure than the hand route.** R cannot go below the no-slide hand run. If the optimizer found a
  route order people missed, the model would not show it. This cap, with G capped at the hand-known gains plus
  57 s of walking, is what makes 21:01.483 need nearly every optimistic end at once.
- **Interactions between optimisations.** The parameter `iota` adds 0 to 25% to whatever excess is left. Draw
  risks add time when a re-timed route breaks a random outcome. The slide changes the route in S11; the model
  keeps the detour measured against the no-slide run.
- **Correlated stages.** The main run keeps success draws independent; table 9b brackets it. Cross-side correlation
  beyond the two shared stages is not modelled.
- **The same shares in every segment.** The three re-routing pools are known for the whole route, not per segment.
- **Settings.** The seed stays 1 and the text speed stays as in full5, except inside the slide's `p_ts`. A searched
  seed or text speed is not modelled. The slide's formation needs automatic skips off; the scan must vary that.
- **The intro card without track 6.** If the intro is not skipped, the model assumes its card also shortens without
  track 6. Not measured; it matters only in the 3-4.5% of draws without the intro skip.
- **The rules question.** Whether a run with track 6 removed counts is the owner's decision. Both variants are
  reported.

## How to read the odds

- The median and the 60:00 odds rest mostly on: the executor exists (`p_exec`), direct walks realise (`v_walk`),
  draw points are solved again (`q_search`, `pen_draw`, `n_dp`), drops work (`p_drop`, `q_atom`).
- The odds near 30:00 and below rest on the upper corners of several ranges at once. They are small for that reason,
  and they depend on shape and correlation.
- A printed 0 is not rounding. It means no pair of draws.

## Files

- `experiments/E031-optimizer/estimate.py`
- `experiments/E031-optimizer/estimate.md`
- Measurements used: `experiments/E031-optimizer/research/segments/segments.json`
  (segments), the same folder's `rep3.py`, `rep4.py`, `path.pkl` and `cache-full5.pkl` (waiting, step pools),
  `scratchpad/gf/out/fight3.json` and `late.json` (draw points), and the glitch-window, graph, reference and
  execution reports of 2026-11-16.
- The version before the audit: `scratchpad/e031-audit/estimate.orig.py`, `estimate.orig.md`, `out.orig.txt`.

## The script's output

```text
E031 estimate.  Seed 31.  Seconds on the Any% clock (quarter-frames / 239.992156448).
Rule: start at the first key in the logo room (or the room's end if no key), end at the first frame of the final fade.
Model: T = R - G.  R: the route brought toward the hand no-slide run.  G: gains beyond that run.

====================================================================================================
1. SEGMENTS (M).  F = full5, N = hand no-slide (28:01.4), L = hand slide (21:01.483).  Seconds.
====================================================================================================
seg  to                             |  F total    wait    walk cut+txt    act |       N       L |    wait+   detour (lo..hi)    rest
S00  boot or logo key -> dock       |    263.1     0.0     0.1   262.6    0.3 |     0.4     0.4 |      0.0     0.0 (   0..   0)   262.7
S01  -> rungs P1+P2                 |    968.5   551.2   280.0   127.0   10.0 |    82.7    82.7 |    536.0   317.7 ( 285.. 352)    32.0
S02  -> rungs P3+P4+P6              |   2176.0  1257.1   556.4   335.6   26.5 |   214.4   214.4 |   1252.5   609.0 ( 477.. 730)   100.2
S03  -> rung P5                     |    275.8   170.0    82.3    20.2    3.3 |    49.3    49.3 |    169.3    60.4 (  56..  65)    -3.2
S04  -> rung P7                     |   1202.8   622.8   218.0   353.2    8.7 |    91.8    80.6 |    607.5   539.9 ( 514.. 560)   -36.4
S05  -> rung P8                     |    813.1   439.4   244.2   117.7   11.7 |   106.2   103.3 |    427.9   279.4 ( 268.. 291)    -0.5
S06  -> rung P9                     |     47.1    12.9    24.8     8.9    0.5 |    20.0    19.2 |     12.7    23.0 (  22..  24)    -8.5
S07  -> rungs P10-P13               |   1137.0   539.8   355.3   227.2   14.6 |   218.4   226.3 |    536.6   263.1 ( 212.. 307)   119.0
S08  -> Part II begins              |     31.5     5.1    18.5     7.3    0.3 |    35.0    23.2 |      4.8    21.1 (  21..  21)   -29.5
S09  -> Part III begins (Part II)   |    509.0   345.6   108.8    39.6   15.0 |    74.2    64.9 |    340.7    86.7 (  55.. 119)     7.4
S10  -> object 884 owned            |    376.3   178.7   129.2    59.7    8.8 |    16.8    14.6 |    178.3   177.3 ( 175.. 179)     4.0
S11  -> object 269 owned            |   2132.4   958.1   925.4   207.2   41.5 |   298.1   108.9 |    948.1   774.2 ( 616.. 923)   111.9
S12  -> room 39 entered             |    122.5    39.4    65.9    14.7    2.4 |    69.2    61.2 |     39.1     0.0 (   0..   0)    14.2
S13  -> room 70 entered             |    103.0    74.5    25.2     1.8    1.5 |    46.5     6.6 |     58.6    10.5 (   4..  22)   -12.6
S14  -> owner 823                   |    450.2   269.1   111.2    61.4    8.5 |    84.4    64.0 |    262.8    77.0 (  61..  94)    26.0
S15  -> last part's card            |     46.6     9.0    20.5    16.2    0.8 |    27.2    26.2 |      8.6     0.0 (   0..   0)    10.8
S16  -> end flag (bit 565), Part IV |    310.3    53.6    23.3   231.6    1.7 |   206.2    75.2 |     52.6     0.0 (   0..   0)    51.4
S17  end flag -> final fade         |        -       -       -       -      - |    40.3    40.4 |
sum                                 |  10965.2  5526.3  3189.1  2091.9  156.1 |  1681.3  1261.4 |   5436.1  3239.3 (2767..3686)   648.7
F runs from boot to the end flag (S00 from boot; from the logo room's own end it is 167.7 s).
0.11 s of N lies outside the main loop and is not classed.  The 23:11.320 route is L plus 129.84 s in S16.
wait+ = F's waiting above N's.  detour = F's busy time in visits beyond N's count for that room and segment.
rest = the remainder: F's play inside the visits both make, against N's (it holds the frame length, 6 against 5).
Waiting inside the extra visits: 3714.6 s of 5526.3 s (67%).

====================================================================================================
2. WHAT SEPARATES THE TWO HAND ROUTES (D: N - L by segment, split with the glitch report's goal times)
====================================================================================================
  S04                        luck at the fights (delay look-ahead)                   11.25 s
  S05,S06,S10,S12,S15,S16    tuning                                                  16.16 s
  S07                        tuning (N - L + setup 23.0)                             15.09 s
  S07                        slide setup cost                                       -23.00 s
  S08,S09                    slide uses at the end of Part I and in Part II          21.17 s
  S11                        monkey skip                                             92.50 s
  S11                        oars skip                                               18.00 s
  S11                        route order and tuning                                  78.67 s
  S13                        luck at the catacomb exits (delay look-ahead)           39.92 s
  S14                        feather skip                                            13.80 s
  S14                        head variant and necklace skip                           6.50 s
  S14                        tuning                                                   0.12 s
  S16                        CD track 6 removed (a files condition, not a find)     129.84 s
                             sum                                                    420.02 s   (28:01.4 - 21:01.483 = 419.93 s)

====================================================================================================
3. PARAMETERS.  M measured, D derived, MR measured range (drawn), A assumed (drawn; low, centre, high)
====================================================================================================
  M  clock_hz                 239.992                  quarter-frames per second; trace boot record, scripts/anypercent_verify.py
  M  hand_noslide_s           1681.41                  28:01.4, runs/anypercent/verify-b1/verify.json (CD track 6 present)
  M  hand_slide_track6_s      1391.32                  23:11.320, runs/anypercent/m2-verify/verify.json
  M  hand_slide_s             1261.48                  21:01.483, runs/anypercent/m2n-verify/verify.json (track 6 removed)
  M  human_record_s           1312.4                   21:52.4, docs/anypercent/overnight.md
  M  card_noslide_s           129.92                   the last part's card in the no-slide run (segments.json card_s); waits for CD track 6
  M  card_full5_s             130                      the same card in full5 (also the first part's card in its intro)
  M  card_no_track6_s         0.167                    the card with track 6 missing, m2n-verify card_s
  M  track6_s                 129.84                   23:11.320 - 21:01.483, the trace shift of 31,160 qf
  M  s00_intro_skipped_s      0.42                     logo key to dock in both hand routes (three keys, frames 46-51)
  M  s00_intro_played_s       167.7                    full5's S00 from the logo room's own end (frame 961): next room 37.5 s + first card 130.0 s
  M  s17_ending_s             40.27                    end flag to final fade in the no-slide run, all cutscene and text
  M  wait_after_probe_s       2828.2                   idle after a probed trial, 399 stops (the search's 60 + 6 frames)
  M  wait_calm_s              550.7                    calm after plain trials
  M  wait_between_clicks_s    1254.3                   idle between the clicks of one trial
  M  wait_explicit_s          893.1                    24 wait-then-act trials + 9 pure wait trials (50, 200, 800 frames)
  M  frames_roomchange_nogain 34181                    403 room-change steps with no gain on the end path (re-counted from path.pkl: 403 steps, 34,181 frames)
  M  frames_direct_walks      13280                    the same travel by entry-keyed direct walks measured in the tree (12,988-16,238 by admitted transitions)
  M  frames_inroom_left       16808                    279 in-room steps with no gain, in rooms left without progress (padding removed)
  M  frames_inroom_sight      2822                     of those, 72 steps that only walk or look
  M  ticks_per_busy_frame     6.098                    full5 after the dock (rep5.py)
  M  busy_s_roomchange_nogain 2457.4                   busy seconds in those 403 steps: 24,474 of their 34,181 frames are busy, 9,707 idle (frame log joined with path.pkl; audit)
  M  frame_length_full5       6                        ticks a frame in full5 (var 19 at its segment start)
  M  frame_length_refs        5                        ticks a frame in both hand routes (the logo glitch)
  M  logo_window_frames       47                       Esc on boot frames 46-92 leaves the frame length at 5 (t_logo.py)
  M  logo_scan_frames         960                      frames of the logo room; only Esc of 8 input kinds works
  M  keeper_away_share        0.327869                 store entry frames with the keeper away: 20 of 61 (t_credit.py, t_slide.py)
  M  exit_values              4                        values the pointed-at exit takes over 31 delays (t_late.py)
  M  feather_s                13.8                     feather skip between the two verified runs
  M  head_necklace_s          6.5                      head variant and necklace skip between the two verified runs
  M  slide_floor_hit          0.0458333                floor points that form the slide: 11 of 240 grid points (17 of 42 in the right part)
  M  carry_readings           6                        deliberate readings that carry the slide through the end of Part I (85 events, 27 needed, 13 frame-exact)
  M  e024_drops_kept          0.0272727                E024 drop round 1: 3 of 110 candidates kept, 546 ticks (3.9%)
  M  e024_retime_ratio        0.0776473                E024 forward pass on the Fate of Atlantis demo: 182,260 -> 14,152 ticks
  M  e025_survival            0.25                     E025: before a draw 1 local gain in 4 survived
  D  wait_in_extra_share      0.672168                 waiting inside extra visits / all waiting = 3714.6 / 5526.3 (rep4.py)
  D  r_artefact               1                        share of probe and calm idle removed: 1, these frames are the search's own, the game is idle (bridge)
  D  busy_share_walk_steps    0.716012                 24,474 busy / 34,181 frames (audit; the first version took only 6 calm frames a step as idle, 0.929, and so counted 4,456 idle frames that re-timing already removes)
  D  pool_direct_walks_s      1502.65                  2,457.4 busy s x (1 - 13,280/34,181): direct walks are tree steps with the same idle share. If they had no idle at all the pool would be about 1,124 s
  D  q_sight_drop             1                        share of walk-or-look steps a drop test passes: 1, they change no world atom (position and time are re-timing's job)
  D  scale_walk               0.833333                 walking at frame length 5 against 6: 785 ticks against 942 on a measured pair, exactly 1 tick a frame (t_logo.py)
  D  cov_logo                 1                        scan tries Esc at every logo frame: 47 of 960 tries hit, so at least one hit is certain
  D  sig_logo                 1                        signal: the frame-length variable in Status differs from the next frame on; payoff delay 0 frames
  D  delay_scan_frames        13                       delays 0..12 tried at a draw, as the reference's look-ahead
  D  cov_keeper               0.994286                 1 - (1 - 20/61)^13: some scanned entry frame has the keeper away
  D  catacomb_luck_s          39.92                    S13: no-slide 46.50 s - slide 6.58 s; the slide run used delay look-ahead there
  D  part2_slide_s            21.17                    S08 + S09: no-slide - slide
  D  s11_order_tuning_s       78.67                    S11: no-slide - slide - monkey - oars
  D  tuning_pool_fixed_s      31.37                    no-slide - slide in S05, S06, S10, S12, S15; S16 less track 6; S07 plus the setup; S14 less feather and head
  D  cov_slide_pair           1                        9 verbs x 9 slots tried: holds the 2 working pairs of 72
  D  cov_slide_frame          1                        25 start frames tried: hold the 5-frame window measured on full5's own stop (20 frames at slow text)
  D  sig_slide                1                        signal: input counter above rest; 0 such frames on full5 before its end flag, so no threshold
  D  cov_oars_clicks12        1                        14 frames x 128 points holds 52 hits; 14 x 104 holds 84 hits
  D  cov_oars_click4          1                        61 frames tried, 12 hit
  D  keep_space_tries         423522                   tries per hit to hold a reading across a dialogue if all four inputs were scanned jointly and blind: 72 pairs x 33 x 23 x 31 frames / 4. Not used: the hold is found step by step on the counter, the same mechanism as the carry's keep-alive readings (sig_keep); about 72 x 33 + 23 + 31 tries
  D  cov_feather              1                        9 verbs x 61 x 61 frames is under 34,000 tries; the click is 2 of 61 frames, any pixel; ordinary progress shows it
  D  monkey_rest_tries        1847.63                  tries per hit for the three inputs after the monkey reading: 1,847, within any budget
  MR detour_scale             0.8543, 1, 1.138         which visits count as extra: shortest (2767 s) to longest (3686 s), centre 3239 s (rep2.py)
  MR q_delay                  0.375, 0.59, 0.83        a 13-delay scan re-solves one draw point. The first version (7/8) counted d = 0, the reference's own timing, as a working delay. Without it: low, fixed continuation, a working delay in 1..12 at 2 of 6 points (fights 485-488: none; exits: 5 and 7-10), (2+1)/(6+2); centre, the last fight draw alone, 2 of 30 delays, 1-(28/30)^13; high, one catacomb exit, 1-(3/4)^6.1 (fight3.json, late.json)
  MR rng_values_in_scan       5.032, 6.081, 7.129      distinct random states in a 13-delay scan: 12 to 17 per 31 delays (t_late.py)
  MR fight_luck_s             2, 4.5, 11.25            fights: 4.5 s from look-ahead (overnight.md); S04 differs by 11.25 s between the hand runs
  MR setup_slide_s            20, 23, 26               slide setup cost: store detour 13.6 + extra talk about 7 + one more item 2.4
  MR oars_s                   17, 18, 19               oars skip, from the two runs' goal times
  MR monkey_s                 90, 92.5, 95             monkey skip, from the two runs' goal times
  MR feather_net_s            0, 5, 13.8               feather skip less the regain it needs (up to 19.9 s from the reading to rest)
  MR walk_gain_rate           0.028, 0.06, 0.072       re-click gain as a share of player-walk ticks: 2.8% from full5's own steps, 6.0% rule and 7.2% beam in E025's probes
  MR b_inroom                 0.653, 0.678, 0.702      busy share of the droppable in-room steps' frames once their idle tails (padding) are removed: 0.653 over all 377 in-room no-gain steps, 0.702 over the 205 in visits with no gain (frame log joined with path.pkl; audit). The first version assumed 0.3-0.6 (centre 0.4), applying the idle share of whole steps to frames already stripped of padding
  A  p_retime                 0.9, 0.97, 0.995         the re-timing pass ends with a whole verified route. E024 did it on another game (182,260 -> 14,152 ticks); draws are the risk
  A  r_inner                  0.8, 0.95, 0.99          share of idle between one trial's clicks removed. 264 s of the 1,254 s is certain (581 click pairs at gaps of 3 and 9 frames)
  A  r_xwait                  0.6, 0.85, 0.97          share of the 33 explicit waits removed. They come in 50, 200, 800 frames; one stop reached the same state 806 frames sooner without its wait
  A  p_exec                   0.5, 0.75, 0.9           a trial can be made again by its name from another place and state (the executor). E024's pass stopped at step 5 of 12 on Monkey Island walking; E025's fixes are designed, not built
  A  v_walk                   0.6, 0.9, 1.15           share of the direct-walk pool realised. Admitted transitions give 0.86-1.01 of it, best measured walk costs 1.31; some walks will not exist at the later facts
  A  p_drop                   0.6, 0.85, 0.95          drop tests with re-timing of the rest work at scale. E024's drops ran but cost the square of the step count
  A  q_atom                   0.2, 0.5, 0.85           share of the 207 no-news steps (279 - 72) that changed some world atom and can still be dropped. Unknown: doors and states may be needed
  A  p_plan                   0.2, 0.45, 0.7           the re-ordering stage exists and verifies. The tree shows independence for 2% of pairs; precedence must be learned by engine tests
  A  y_need                   0.05, 0.3, 0.65          share of the remaining detour pool removed by drop tests on progress trials the end does not need. Ceiling 0.65: the 101 of 257 progress trials that set only keys no script tests hold 394 s busy; floor: E024 kept 3 of 110 drops
  A  y_order                  0.05, 0.3, 0.9           share of what then remains removed by re-ordering, when that stage works. The tree bounds re-ordering at 3,000-7,000 frames (305-711 s); AE+PNGS improved 60 of 270 plans
  A  phi_fb                   0.05, 0.18, 0.45         detour share removed without the executor, by loop cuts of raw inputs and corridors of the frozen search. The first plan put these at 583 s (167-1,500) and 250 s at 35%; E024 kept 3 of 110
  A  e_walk                   0.3, 0.6, 1              share of full5's extra walking inside matched visits removed (exit pixels, re-clicks, approach clicks). Needs the walk stage (p_walkopt): 0 without it. High end 1 = the hand route's quality (the first version capped it at 0.9, so even the best case could not reach the hand route)
  A  e_cut                    0.1, 0.4, 1              share of full5's extra cutscene, text and acting time inside matched visits removed (shorter dialogue paths, in-visit drops). Needs the executor and drops: 0 without them. High end 1 = the hand route's quality (was 0.8)
  A  u_frame                  0.05, 0.35, 0.9          share of non-walking busy time that is paced by frames, not ticks; that time scales by 1 - u x (1 - 5/6). The whole-route saving on the reference is 157 s (walking frames only) to 240 s (all frames)
  A  p_logo_surv              0.5, 0.8, 0.95           the route can be re-timed to its end at frame length 5 (tick-paced waits inside a group take more frames; the first plan gave a raw-input re-timer 40%)
  A  p_intro                  0.9, 0.97, 0.995         the boot scan also finds the keys that skip the rest of the intro. One Esc at frame 46 was shown to work under the search's settings
  A  p_end                    0.9, 0.97, 0.995         the two closing menus are answered and the final fade is reached. They come with input on inside a cutscene, where the frozen search never clicks
  A  iota                     0, 0.08, 0.25            interactions: share added to whatever excess over the hand route is left, because gains do not add up (E025: 1 local gain in 4 survived before a draw)
  A  p_adapt                  0.7, 0.9, 0.98           after re-timing, the early safe opening is made again with the count of sentences the new draw needs (20 of 20 keeper-away entries opened)
  A  pen_credit               40, 58, 80               seconds lost if the early safe opening is lost. 58 s from one runner's two runs on another release, not measured here
  A  q_search                 0.5, 0.8, 0.95           the frozen search solves a draw-ruled stretch again from a new random state, started with --from. It solved each once on seed 1
  A  n_dp                     7, 11, 19                draw points on the route that a re-timed route must solve again (fight choices, catacomb exits; the store entry is Credit Early's own term, so it is not counted twice)
  A  pen_draw                 30, 120, 400             seconds lost for each draw point not solved again, its stretch left as full5 played it (S13's excess is 96 s, one fight room holds 248 s); multiplied by the expected count of failed points
  A  p_luck                   0.6, 0.85, 0.95          the delay scan at draws exists and verifies (twins to mark draws are built in fmi-shorten; the scan is not). Shared with R: without it draw points and the store entry are re-solved by the frozen search alone
  A  e_tune                   0.1, 0.4, 1              share of the tuning gap between the two hand runs that the optimizer closes outside the slide's skips (early inputs gave 4.66 s on 248 goals). High end 1 = the hand tuning (was 0.8)
  A  t_share                  0.3, 0.5, 0.7            share of S11's 78.7 s of 'route order and tuning' that needs no skip; the rest needs the monkey skip
  A  p_walkopt                0.4, 0.6, 0.85           the walk stage (re-clicks, waypoints) gets built and its gains verify from boot. E025 was planned and never built. Shared with R: e_walk needs the same stage
  A  h_new                    0.4, 0.7, 0.9            share of walk gains the hand route does not already hold (it re-clicks on 11 of 532 moves, sweeps 2, hand-steps 7)
  A  sv_draw                  0.25, 0.6, 1             share of walk gains that survive later draws: 1 in 4 before a draw (E025), all after the last draw
  A  p_head                   0.1, 0.3, 0.5            the head variant and necklace skip: timing only, no state signal; 25 of 77 standing points and a 2-frame window; found only by comparing arrival frames
  A  cov_floor                0.2799, 0.8469, 1        coverage of the floor click that forms the slide: 1-(1-11/240)^n for n = 7, 40 or all 240 grid points per visit
  A  p_ts                     0.1, 0.3, 0.6            the route between the slide's forming and Part III can be played with readings that last (slow text chosen, no dialogue menu that cannot be bridged)
  A  sig_keep                 0.4, 0.7, 0.9            each of the 6 keep-alive readings is found by a stepwise scan that sees the counter stay raised (1 event in 4 moved by a frame left the route in a wrong room)
  A  p_part2                  0.2, 0.5, 0.8            with the slide alive, a window scan finds the Part II uses (input inside cutscenes)
  A  c3_oars                  0.5, 0.8, 0.95           coverage of the third oars click (19-frame window; its click point was not scanned)
  A  s_step                   0.3, 0.6, 0.85           each oars click shows a signal the scan rewards (ego absent with input on, a position jump, a room out of order), so the four clicks are found one at a time
  A  cov_in_monkey            0.1, 0.28, 0.6           the monkey reading is in the scan's input set: 40 pruned inputs of 72 pairs, times 0.5 for the inventory page it needs
  A  cov_fr_monkey            0.5, 0.75, 1             the scan hits the 2-frame window: 1 if every frame is tried, 0.5 if every fourth
  A  s1_monkey                0.2, 0.5, 0.8            'a timed window held open longer than in its twin' is rewarded as a step, so the hold is kept and the next three inputs searched
  A  p_order                  0.2, 0.5, 0.8            the route order around the skips is re-made once the monkey skip exists
  A  p_regain                 0.1, 0.35, 0.6           the slide is formed again at the village by the same scan (not probed; the hand way needs 4 frame-exact text skips)
  Derived shares at the centre: f_wait = 0.9644; direct-walk pool 1503 s = 0.464 of the detour; drop pool 1158 s = 0.358;
  the remaining 0.179 (578 s) is the pool of unneeded progress and order.  x_drop = 0.584.
  A parameter that is the probability of one success draw (21 of them) enters through the mean of its range.

====================================================================================================
4. MECHANISMS.  Probability (calculated at the centre), seconds removed if it succeeds (low, centre, high), needs
====================================================================================================
  mechanism                             p             low  centre    high  probability from / needs
  true end (bit 565, frame 108,830)     100%        614.8   614.8   614.8  M: 694,701 - 657,813 ticks; already in F
  end menus to the final fade           97%         -40.3   -40.3   -40.3  A p_end; adds S17; without it there is no Any% time
  re-timing: probe and calm idle        97%        3378.9  3378.9  3378.9  A p_retime; D r_artefact
  re-timing: input spacing in a trial   97%        1003.4  1191.6  1241.8  A r_inner; needs re-timing
  re-timing: explicit waits             97%         535.9   759.1   866.3  A r_xwait; needs re-timing
  intro skip (S00)                      97%         167.3   167.3   167.3  A p_intro; 37.4 s if track 6 is removed
  re-routing: direct walks              72.8%       901.6  1352.4  1728.0  needs re-timing; A p_exec; pool 0.464 of the detour (D) x v_walk
  re-routing: drops of no-gain steps    61.8%       372.9   676.3  1049.5  needs the executor; A p_drop; pool 0.358 x x_drop
  re-routing: unneeded progress         61.8%        28.9   173.5   375.9  needs drops; pool 0.179 x y_need
  re-routing: order                     27.8%        20.2   121.5   364.4  needs drops; A p_plan; what is left x y_order
  re-routing without the executor       24.2%       162.0   583.1  1457.7  only if the executor fails: A phi_fb x the detour
  execution: text                       100%          0.0     0.0     0.0  M: 1,819 of 1,834 skips already on the first frame
  execution: walking in matched visits  58.2%                              A e_walk x rest (table 5, column in-visit); needs the walk stage
  execution: cutscene, text, acting     61.8%                              A e_cut x rest (table 5, column in-visit); needs the executor and drops
  logo speed glitch                     77.6%                              47 of 960 scanned frames hit: cov 1; signal 1; x A p_logo_surv (table 5)
  Credit Early kept after re-timing     89.5%         0.0     0.0     0.0  with the delay scan 1-(1-20/61)^13 = 0.9943 x A p_adapt (without it 0.779); if lost: +58 s
  draw points solved again              39%           0.0     0.0     0.0  (1-(1-q_search)(1-q_delay))^n_dp, q_delay 0.59 (without the delay scan 8.59%); +120 s per lost point
  luck: catacomb exits (S13)            85%          30.5    33.0    34.8  A p_luck (the delay scan); 1-(3/4)^n over 6.1 random states = 0.826 of 39.9 s
  luck: fights (S04)                    85%           2.0     4.5    11.2  the same pass
  tuning toward the slide run           100%          5.5    28.3    86.4  A e_tune x (31.4 + t_share x 78.7)
  walking beyond the hand route         60%           2.5    22.3    57.3  A p_walkopt; 884 s walking x rate x h_new x sv_draw
  head variant and necklace (S14)       30%           6.5     6.5     6.5  A p_head (timing only, no state signal)
  item slide formed (store)             84.7%         0.0     0.0     0.0  cov_floor x pair 1 x frame 1 x signal 1; no gain by itself
  slide carried into Part III           2.99%       -26.0   -23.0   -20.0  needs formed; p_ts x sig_keep^6 = 0.0353; pays the setup
  slide uses in S08, S09                1.49%        21.2    21.2    21.2  needs carried; A p_part2
  oars skip (S11)                       1.43%        17.0    18.0    19.0  needs carried; 1 x c3_oars x 1 x s_step = 0.480
  monkey skip (S11)                     0.314%       90.0    92.5    95.0  needs carried; cov_in x cov_fr x s1 = 0.105
  route order around the skips (S11)    0.157%       23.6    39.3    55.1  needs the monkey skip; A p_order
  slide regained, feather skip (S14)    24.5%         0.0     5.0    13.8  p_regain x sig_keep (the hold, found step by step) x 1 = 0.245; its own path
  CD track 6 removed                    100%        129.8   129.8   129.8  a files condition: the second reported variant
  Whole slide chain at the centre (formed, carried, S08-S09 uses, oars, monkey, order): 0.0377%.
  The Monte Carlo uses the mean of each probability's range, not its centre: re-timing 95.5%, executor 71.7%, drops 80%,
  order 45%, logo 75%, intro 95.5%, end 95.5%, Credit Early 85.5%.  'Draws solved' is computed in every draw from q_search, n_dp, q_delay.
  The delay scan (p_luck 80%) and the walk stage (p_walkopt 61.7%) serve both R and G: results mix four strata.
  Payoff delays the valuation must bridge (M): logo 0 goals; early safe 105 goals, 385 s (kept by ordinary novelty);
  slide: first large saving 73 goals, 127 s after forming, setup cost first (bridged by the 6 keep-alive readings);
  oars 3 goals, 8.3 s; monkey 1 goal, 3.9 s; regain 13 goals to rest, 57 goals to the feather skip; feather 1 goal;
  head 10 goals, 20.9 s; luck within the next step.

====================================================================================================
5. PER-SEGMENT EFFECT AT THE CENTRE: seconds each mechanism removes if it succeeds, and the segment after all of them
====================================================================================================
  shares of each segment's detour removed: walks 0.417, drops 0.209, needs 0.054, order 0.037 (the same in every segment)
seg  |        F | re-time   walks   drops   needs   order in-visit    logo |    after |       N       L
S01  |    968.5 |   517.0   132.7    66.3    17.0    11.9      6.3    24.9 |    188.2 |    82.7    82.7
S02  |   2176.0 |  1207.9   254.2   127.1    32.6    22.8     31.7    56.1 |    437.3 |   214.4   214.4
S03  |    275.8 |   163.3    25.2    12.6     3.2     2.3     -3.4     8.9 |     61.1 |    49.3    49.3
S04  |   1202.8 |   585.9   225.4   112.7    28.9    20.2    -25.1    23.2 |    226.3 |    91.8    80.6
S05  |    813.1 |   412.7   116.7    58.3    15.0    10.5     -1.5    21.6 |    175.1 |   106.2   103.3
S06  |     47.1 |    12.2     9.6     4.8     1.2     0.9     -7.0     3.7 |     21.9 |    20.0    19.2
S07  |   1137.0 |   517.5   109.8    54.9    14.1     9.9     34.3    46.3 |    355.6 |   218.4   226.3
S08  |     31.5 |     4.7     8.8     4.4     1.1     0.8    -16.8     3.2 |     24.2 |    35.0    23.2
S09  |    509.0 |   328.6    36.2    18.1     4.6     3.3      0.3    12.8 |    102.6 |    74.2    64.9
S10  |    376.3 |   171.9    74.0    37.0     9.5     6.6      0.7     9.3 |     66.9 |    16.8    14.6
S11  |   2132.4 |   914.4   323.2   161.7    41.5    29.0     37.7    81.4 |    546.8 |   298.1   108.9
S12  |    122.5 |    37.8     0.0     0.0     0.0     0.0      0.8    11.9 |     72.2 |    69.2    61.2
S13  |    103.0 |    56.5     4.4     2.2     0.6     0.4     -9.9     4.7 |     43.2 |    46.5     6.6
S14  |    450.2 |   253.4    32.2    16.1     4.1     2.9      6.4    14.3 |    120.3 |    84.4    64.0
S15  |     46.6 |     8.3     0.0     0.0     0.0     0.0      2.9     4.0 |     31.6 |    27.2    26.2
S16  |    310.3 |    50.7     0.0     0.0     0.0     0.0     19.1     8.3 |    234.0 |   206.2    75.2
sum  |  10702.1 |  5242.7  1352.4   676.3   173.5   121.5     76.6   334.6 |   2707.3 |  1640.7  1220.6
  Low and high over the route: re-time 4838 to 5397 s (f_wait 0.890 to 0.993); the others as in table 4.
  Removing a visit also removes the waiting left in it (95 s at the centre; counted in 'after', not in the columns).
  logo = the segment at frame length 6 less the same at 5 (walking x 5/6, other busy time x (1 - u/6), u = 0.35).
  'after' holds the interaction share (iota = 0.08).  S00 after: 0.42 s.  S17 after: 40.27 s.
  Negative in-visit values: in that segment full5's matched visits are already faster than N's in one class.

====================================================================================================
6. STAIRCASE AT THE CENTRE (every parameter at its centre; each step if it and all before it succeed)
====================================================================================================
  step                                        track 6   no track 6    p(step)  p(all so far)
  full5 as found, end menus answered        3:01:50.9    2:57:31.3        97%            97%
  + re-timing                               1:34:28.6    1:30:09.0        97%          94.1%
  + intro skip                              1:31:41.3    1:29:31.5        97%          91.3%
  + direct walks                            1:08:13.7    1:06:03.9        75%          68.5%
  + drops of no-gain steps                    56:29.7      54:20.0        85%          58.2%
  + drops of unneeded progress                53:29.1      51:19.4       100%          58.2%
  + order                                     51:22.7      49:13.0        45%          26.2%
  + in-visit execution (walk stage)           50:06.2      47:56.4        60%          15.7%
  + logo speed glitch                         44:28.9      42:19.2        80%          12.6%
  + interactions (iota)                       45:47.9      43:38.2       100%          12.6%
    (delay scan: Credit Early, draws)         45:47.9      43:38.2      29.7%          3.73%
  + luck by delay                             45:10.5      43:00.7       100%          3.73%
  + tuning                                    44:42.2      42:32.4       100%          3.73%
  + walking beyond the hand route             44:19.9      42:10.1       100%          3.73%
  + head and necklace                         44:13.4      42:03.6        30%          1.12%
  + feather path                              44:08.4      41:58.6      24.5%         0.274%
  + whole slide chain                         41:40.4      39:30.6    0.0377%      0.000103%
  Anchors: hand no-slide 28:01.4, hand slide 23:11.3 (track 6) and 21:01.5 (no track 6), human record 21:52.4.

====================================================================================================
7. CROSS-CHECKS OF THE MODEL
====================================================================================================
  All removal shares at 1, logo and intro found: R = 1681.303 s (hand no-slide 1681.413); without track 6 1551.550 s.
  Every hand-known gain at its hand value: G = 290.18 s; R - G without track 6 = 1261.37 s (hand slide 1261.483).
  Nothing removed: R = 10910.9 s; full5 on the same rule (logo room's end to the fade) = 10912.5 s.
  Fixed order, all idle gone, every droppable no-gain step dropped, frame length 6: 44.1 min
    (the graph report's bound for the same route with its idle left in: 57.5 min; progress trials alone 35.2 min).
  Waiting at N's level and the intro skipped, nothing else: 5308 s; straight from table 1: 5309 s
    (the measurement report's 5,223 s is against the slide run's waiting and stops at the end flag).
  Sampler against the plain function at the centre: 0.000000 s apart (track 6), 0.000000 s (no track 6).
  Table 5's 'after' column plus S00 and S17: 2748.1 s; the plain function: 2747.9 s.
  Detour removed at the centre with every stage working: 0.717 (1 would be the reference's visit count).

====================================================================================================
8. MONTE CARLO (150000 draws of R in each stratum and 150000 of G; every R draw is combined with every G draw; seed 31)
====================================================================================================
  CD track 6 present (compare with 28:01.4 and 23:11.3)
    median 1:03:19.9   10th percentile 46:39.1   90th percentile 1:37:04.4
    odds of under:  60:00 44.6%   30:00 7.41e-06%   28:01.4 2.43e-08%   23:11.3 0   21:01.483 0
  CD track 6 removed (compare with 21:01.483)
    median 1:01:04.8   10th percentile 44:24.3   90th percentile 1:34:47.4
    odds of under:  60:00 48.4%   30:00 0.00117%   28:01.4 1.54e-05%   23:11.3 0   21:01.483 0
  No Any% end (closing menus not answered): 4.44% of draws; they count as failing every line.  Mean of the rest: 1:08:56.7 (track 6).
  With the delay scan and the walk stage built (49.3% of the weight): every other success draw of R succeeds in 4.67% of plain draws (weight of the 'all found' stratum: 4.67%).
  Mean chance the slide is carried into Part III: 2.82%.  Mean G without it 97.0 s, with it 113.2 s (both stages built).
  Best single draws: R without track 6 29:29.8; G 292.3 s; that pair would give 24:37.5.  A printed 0 means no pair of draws.
  Share of plain draws of R by size (track 6 present, before G, strata mixed):
    under 40 min 0.427%   40-50 14.6%   50-60 27.2%   60-90 43.9%   over 90 min 9.5%   no end 4.44%

====================================================================================================
9. ALL ASSUMED PARAMETERS AT THEIR PESSIMISTIC AND OPTIMISTIC ENDS, AND FLAT RANGES (30000 draws; success draws and measured ranges still drawn)
====================================================================================================
  pessimistic track 6 present median 2:22:22.0  10th 1:47:11.2  90th no end     | under:  60:00 0   30:00 0   28:01.4 0   23:11.3 0   21:01.483 0
  pessimistic track 6 removed median 2:19:41.4  10th 1:44:48.2  90th no end     | under:  60:00 0   30:00 0   28:01.4 0   23:11.3 0   21:01.483 0
  optimistic  track 6 present median 25:52.4    10th 24:53.9    90th 50:33.7    | under:  60:00 97.8%   30:00 81.1%   28:01.4 72.4%   23:11.3 2.2%   21:01.483 0
  optimistic  track 6 removed median 23:42.4    10th 22:43.8    90th 48:23.7    | under:  60:00 98.2%   30:00 83.2%   28:01.4 81.7%   23:11.3 31.2%   21:01.483 2.18%
  flat ranges track 6 present median 1:08:08.6  10th 46:43.0    90th 3:01:49.3  | under:  60:00 35.8%   30:00 0.00553%   28:01.4 0.000293%   23:11.3 0   21:01.483 0
  flat ranges track 6 removed median 1:05:52.8  10th 44:26.9    90th 2:57:29.7  | under:  60:00 39.9%   30:00 0.049%   28:01.4 0.0077%   23:11.3 4.14e-07%   21:01.483 0
  'flat ranges': every assumed parameter drawn evenly over its range instead of peaked at its centre (a check on the shape).

====================================================================================================
9b. CORRELATED SUCCESS DRAWS: one common factor among R's success draws and among G's (Gaussian copula; 30000 draws)
====================================================================================================
  The main run treats every success draw as independent.  Stages built by one team on one codebase are not:
  a common factor raises the chance that all succeed and that several fail together.  rho 0 = the main run's assumption.
  rho 0.0 drawn      track 6 present median 1:03:07.8  | under:  60:00 44.9%   30:00 4.88e-06%   28:01.4 1.55e-08%   23:11.3 0   21:01.483 0
  rho 0.0 drawn      track 6 removed median 1:00:51.5  | under:  60:00 48.6%   30:00 0.00176%   28:01.4 9.76e-06%   23:11.3 0   21:01.483 0
  rho 0.0 optimistic track 6 present median 25:52.4    | under:  60:00 97.8%   30:00 81.1%   28:01.4 72.4%   23:11.3 2.2%   21:01.483 0
  rho 0.0 optimistic track 6 removed median 23:42.4    | under:  60:00 98.2%   30:00 83.2%   28:01.4 81.7%   23:11.3 31.2%   21:01.483 2.18%
  rho 0.5 drawn      track 6 present median 57:31.1    | under:  60:00 54.2%   30:00 6.23e-05%   28:01.4 1.32e-06%   23:11.3 0   21:01.483 0
  rho 0.5 drawn      track 6 removed median 55:19.1    | under:  60:00 57.2%   30:00 0.00433%   28:01.4 8.1e-05%   23:11.3 0   21:01.483 0
  rho 0.5 optimistic track 6 present median 25:33.2    | under:  60:00 97.4%   30:00 84.2%   28:01.4 77.8%   23:11.3 7.86%   21:01.483 0
  rho 0.5 optimistic track 6 removed median 23:23.4    | under:  60:00 98.5%   30:00 85.5%   28:01.4 84.5%   23:11.3 36.7%   21:01.483 7.84%
  rho 0.9 drawn      track 6 present median 54:15.0    | under:  60:00 62.7%   30:00 0.000181%   28:01.4 5.07e-06%   23:11.3 0   21:01.483 0
  rho 0.9 drawn      track 6 removed median 52:05.2    | under:  60:00 65.1%   30:00 0.00687%   28:01.4 0.000222%   23:11.3 0   21:01.483 0
  rho 0.9 optimistic track 6 present median 25:28.0    | under:  60:00 96.4%   30:00 88.6%   28:01.4 83.1%   23:11.3 13.3%   21:01.483 0
  rho 0.9 optimistic track 6 removed median 23:18.2    | under:  60:00 98.6%   30:00 89.1%   28:01.4 88.7%   23:11.3 40.5%   21:01.483 13.2%

====================================================================================================
10. SENSITIVITY A: one assumed parameter pinned at its pessimistic and its optimistic end, all others drawn (6000 draws each)
====================================================================================================
  Nothing pinned, at this sample size: under 21:01.483 (no track 6) 0; under 30:00 (no track 6) 0.00149%; under 60:00 (track 6) 44.3%; median (track 6) 1:03:30.6.
  parameter      pess. -> opt. end   | under 21:01.483, no t6 | under 30:00, no t6     | under 60:00, t6    | median, t6
  v_walk         0.6 -> 1.15         | 0         -> 0         | 0         -> 0.124%    | 26.7%   -> 54.6%   | 1:09:34.6 -> 56:14.9
  q_atom         0.2 -> 0.85         | 0         -> 0         | 0         -> 0.0532%   | 33.1%   -> 50.5%   | 1:06:33.0 -> 59:18.2
  y_order        0.05 -> 0.9         | 0         -> 0         | 2.03e-08% -> 0.025%    | 42.6%   -> 45.8%   | 1:04:06.5 -> 1:02:59.0
  e_cut          0.1 -> 1            | 0         -> 0         | 0.000226% -> 0.00589%  | 42.7%   -> 46%     | 1:03:59.9 -> 1:02:48.7
  y_need         0.05 -> 0.65        | 0         -> 0         | 0.000422% -> 0.00546%  | 39.7%   -> 47.9%   | 1:05:07.2 -> 1:01:40.9
  r_inner        0.8 -> 0.99         | 0         -> 0         | 3.42e-05% -> 0.0044%   | 41.6%   -> 46.1%   | 1:05:16.7 -> 1:02:31.3
  r_xwait        0.6 -> 0.97         | 0         -> 0         | 1.35e-05% -> 0.00396%  | 40.8%   -> 47%     | 1:05:41.1 -> 1:01:55.4
  iota           0.25 -> 0           | 0         -> 0         | 0.00021%  -> 0.00402%  | 38.6%   -> 48.9%   | 1:07:18.9 -> 1:00:46.0
  e_tune         0.1 -> 1            | 0         -> 0         | 0.000359% -> 0.00357%  | 43.5%   -> 45.4%   | 1:03:56.7 -> 1:02:53.2
  q_search       0.5 -> 0.95         | 0         -> 0         | 0.000467% -> 0.00355%  | 33.7%   -> 51.6%   | 1:09:13.7 -> 58:37.1
  u_frame        0.05 -> 0.9         | 0         -> 0         | 0.000608% -> 0.00338%  | 43.4%   -> 45.5%   | 1:04:25.1 -> 1:02:37.9
  p_plan         0.2 -> 0.7          | 0         -> 0         | 0.000662% -> 0.00232%  | 43.2%   -> 45.4%   | 1:03:59.9 -> 1:03:05.4
  n_dp           19 -> 7             | 0         -> 0         | 0.000678% -> 0.00214%  | 38.5%   -> 48.9%   | 1:06:45.7 -> 1:00:49.1
  t_share        0.3 -> 0.7          | 0         -> 0         | 0.00103%  -> 0.00194%  | 44.1%   -> 44.6%   | 1:03:39.5 -> 1:03:24.0
  p_logo_surv    0.5 -> 0.95         | 0         -> 0         | 0.000992% -> 0.00189%  | 41%     -> 47%     | 1:04:57.2 -> 1:02:06.0
  sv_draw        0.25 -> 1           | 0         -> 0         | 0.00107%  -> 0.00196%  | 44.1%   -> 44.5%   | 1:03:36.5 -> 1:03:23.4
  p_exec         0.5 -> 0.9          | 0         -> 0         | 0.00104%  -> 0.00187%  | 31.6%   -> 55.4%   | 1:13:16.8 -> 57:56.5
  p_drop         0.6 -> 0.95         | 0         -> 0         | 0.00112%  -> 0.00177%  | 34.1%   -> 52.5%   | 1:08:31.4 -> 58:52.8
  p_luck         0.6 -> 0.95         | 0         -> 0         | 0.00112%  -> 0.00176%  | 41.3%   -> 46.6%   | 1:05:10.0 -> 1:02:11.6
  p_walkopt      0.4 -> 0.85         | 0         -> 0         | 0.00119%  -> 0.00182%  | 43.9%   -> 44.8%   | 1:03:39.8 -> 1:03:17.6
  e_walk         0.3 -> 1            | 0         -> 0         | 0.0013%   -> 0.00193%  | 43.9%   -> 44.8%   | 1:03:39.7 -> 1:03:19.5
  h_new          0.4 -> 0.9          | 0         -> 0         | 0.0012%   -> 0.00176%  | 44.2%   -> 44.4%   | 1:03:34.7 -> 1:03:26.0
  p_adapt        0.7 -> 0.98         | 0         -> 0         | 0.00121%  -> 0.0017%   | 44.2%   -> 44.6%   | 1:03:37.8 -> 1:03:23.6
  sig_keep       0.4 -> 0.9          | 0         -> 0         | 0.00142%  -> 0.00165%  | 44.3%   -> 44.4%   | 1:03:31.4 -> 1:03:29.0
  p_retime       0.9 -> 0.995        | 0         -> 0         | 0.0014%   -> 0.00155%  | 41.9%   -> 46.3%   | 1:05:38.9 -> 1:02:12.6
  p_intro        0.9 -> 0.995        | 0         -> 0         | 0.0014%   -> 0.00155%  | 43.9%   -> 44.7%   | 1:03:36.4 -> 1:03:27.7
  p_end          0.9 -> 0.995        | 0         -> 0         | 0.0014%   -> 0.00155%  | 41.7%   -> 46%     | 1:05:44.8 -> 1:02:22.2
  p_head         0.1 -> 0.5          | 0         -> 0         | 0.00142%  -> 0.00156%  | 44.3%   -> 44.4%   | 1:03:30.8 -> 1:03:29.5
  p_regain       0.1 -> 0.6          | 0         -> 0         | 0.00142%  -> 0.00155%  | 44.3%   -> 44.4%   | 1:03:31.4 -> 1:03:29.7
  p_ts           0.1 -> 0.6          | 0         -> 0         | 0.00146%  -> 0.00152%  | 44.3%   -> 44.3%   | 1:03:30.6 -> 1:03:29.7
  cov_in_monkey  0.1 -> 0.6          | 0         -> 0         | 0.00147%  -> 0.00151%  | 44.3%   -> 44.3%   | 1:03:30.6 -> 1:03:29.7
  s1_monkey      0.2 -> 0.8          | 0         -> 0         | 0.00147%  -> 0.00151%  | 44.3%   -> 44.3%   | 1:03:30.6 -> 1:03:29.7
  cov_floor      0.2799 -> 1         | 0         -> 0         | 0.00147%  -> 0.0015%   | 44.3%   -> 44.3%   | 1:03:30.6 -> 1:03:29.7
  cov_fr_monkey  0.5 -> 1            | 0         -> 0         | 0.00148%  -> 0.0015%   | 44.3%   -> 44.3%   | 1:03:30.6 -> 1:03:29.7
  p_order        0.2 -> 0.8          | 0         -> 0         | 0.00148%  -> 0.0015%   | 44.3%   -> 44.3%   | 1:03:30.6 -> 1:03:30.6
  p_part2        0.2 -> 0.8          | 0         -> 0         | 0.00148%  -> 0.0015%   | 44.3%   -> 44.3%   | 1:03:30.6 -> 1:03:30.6
  s_step         0.3 -> 0.85         | 0         -> 0         | 0.00148%  -> 0.00149%  | 44.3%   -> 44.3%   | 1:03:30.6 -> 1:03:29.7
  c3_oars        0.5 -> 0.95         | 0         -> 0         | 0.00149%  -> 0.00149%  | 44.3%   -> 44.3%   | 1:03:30.6 -> 1:03:30.6
  pen_draw       400 -> 30           | 0         -> 0         | 0.00149%  -> 0.00149%  | 29.7%   -> 51.9%   | 1:10:12.8 -> 58:19.5
  phi_fb         0.05 -> 0.45        | 0         -> 0         | 0.00149%  -> 0.00149%  | 44.3%   -> 45.8%   | 1:03:36.5 -> 1:01:31.9
  pen_credit     80 -> 40            | 0         -> 0         | 0.00149%  -> 0.00149%  | 44.2%   -> 44.4%   | 1:03:35.5 -> 1:03:27.0
  Sorted by the move in the odds of under 21:01.483, then by the 30:00 column.  t6 = CD track 6 present.
  All runs use the same random numbers.  The 30:00 column still carries noise of about 0.02 percentage points
  (one plain draw is 1/6000 = 0.0167%), so only its first rows are real differences.

====================================================================================================
11. SENSITIVITY B: every assumed parameter at its optimistic end, then one moved to its pessimistic end (6000 draws each)
====================================================================================================
  All optimistic: under 21:01.483 (no track 6) 2.17%; under 23:11.3 (track 6) 2.19%; under 28:01.4 (track 6) 72%.
  parameter      moved to     | under 21:01.483, no t6   | under 23:11.3, t6      | under 28:01.4, t6
  v_walk         0.6          | 0          (x 0        ) | 0                      | 0
  q_atom         0.2          | 0          (x 0        ) | 0                      | 0
  r_xwait        0.6          | 0          (x 0        ) | 0                      | 56.3%
  r_inner        0.8          | 0          (x 0        ) | 0                      | 61.2%
  e_tune         0.1          | 0          (x 0        ) | 0                      | 61.2%
  sv_draw        0.25         | 0          (x 0        ) | 0                      | 69%
  h_new          0.4          | 0.00207%   (x 0.000955 ) | 0.00207%               | 69.8%
  sig_keep       0.4          | 0.0147%    (x 0.00677  ) | 0.0148%                | 70%
  y_order        0.05         | 0.192%     (x 0.0886   ) | 0.194%                 | 62.7%
  e_cut          0.1          | 0.327%     (x 0.151    ) | 0.33%                  | 60.5%
  p_ts           0.1          | 0.361%     (x 0.167    ) | 0.364%                 | 70.6%
  cov_in_monkey  0.1          | 0.367%     (x 0.169    ) | 0.373%                 | 71%
  q_search       0.5          | 0.555%     (x 0.256    ) | 0.56%                  | 67.1%
  s1_monkey      0.2          | 0.562%     (x 0.259    ) | 0.571%                 | 71.1%
  p_order        0.2          | 0.574%     (x 0.265    ) | 0.585%                 | 71.9%
  cov_floor      0.2799       | 0.607%     (x 0.28     ) | 0.612%                 | 70.8%
  p_part2        0.2          | 0.615%     (x 0.284    ) | 0.622%                 | 71.7%
  p_plan         0.2          | 0.743%     (x 0.343    ) | 0.749%                 | 64%
  e_walk         0.3          | 0.899%     (x 0.415    ) | 0.907%                 | 69.1%
  s_step         0.3          | 0.905%     (x 0.418    ) | 0.926%                 | 71.8%
  p_walkopt      0.4          | 1.02%      (x 0.471    ) | 1.03%                  | 60%
  cov_fr_monkey  0.5          | 1.06%      (x 0.49     ) | 1.07%                  | 71.4%
  p_logo_surv    0.5          | 1.14%      (x 0.527    ) | 1.15%                  | 39.7%
  p_exec         0.5          | 1.2%       (x 0.556    ) | 1.22%                  | 40.3%
  c3_oars        0.5          | 1.24%      (x 0.572    ) | 1.26%                  | 71.8%
  p_luck         0.6          | 1.37%      (x 0.632    ) | 1.38%                  | 70.1%
  p_drop         0.6          | 1.37%      (x 0.633    ) | 1.38%                  | 45%
  p_adapt        0.7          | 1.55%      (x 0.713    ) | 1.56%                  | 71.1%
  n_dp           19           | 1.7%       (x 0.785    ) | 1.72%                  | 71.6%
  p_regain       0.1          | 1.78%      (x 0.82     ) | 1.79%                  | 71.6%
  iota           0.25         | 1.79%      (x 0.826    ) | 1.8%                   | 68.1%
  p_head         0.1          | 1.85%      (x 0.852    ) | 1.87%                  | 71.7%
  p_end          0.9          | 1.96%      (x 0.903    ) | 1.98%                  | 64.9%
  p_intro        0.9          | 1.96%      (x 0.905    ) | 1.98%                  | 67.6%
  p_retime       0.9          | 1.96%      (x 0.905    ) | 1.98%                  | 65.2%
  y_need         0.05         | 2.08%      (x 0.958    ) | 2.09%                  | 56.3%
  t_share        0.3          | 2.15%      (x 0.992    ) | 2.17%                  | 68.3%
  u_frame        0.05         | 2.17%      (x 0.999    ) | 2.19%                  | 71.6%
  pen_draw       400          | 2.17%      (x 1        ) | 2.19%                  | 62.6%
  pen_credit     80           | 2.17%      (x 1        ) | 2.19%                  | 71.8%
  phi_fb         0.05         | 2.17%      (x 1        ) | 2.19%                  | 72%
  Sorted by what is left of the 21:01.483 odds.  x = the factor on the all-optimistic odds.
  t_share has no fixed good end: a low share helps when the slide chain is found (more of S11 falls to the skips), a high one when it is not.

====================================================================================================
12. TOP FIVE FOR THE 21:01.483 ODDS, AND THE CHEAPEST EARLY MEASUREMENT THAT REPLACES EACH ASSUMPTION
====================================================================================================
  With the others drawn (table 10) the 21:01.483 odds stay at 0 for every single parameter, so the order is taken
  from table 11: the parameters whose pessimistic end removes most of the all-optimistic odds.
  v_walk         all-optimistic odds 2.17% -> 0 when it is at 0.6.
                 assumes: share of the direct-walk pool realised. Admitted transitions give 0.86-1.01 of it, best measured walk costs 1.31; some walks will not exist at the later facts
                 measure: check each of the route's direct walks once in the engine (about 200 forks of under 1 s)
  q_atom         all-optimistic odds 2.17% -> 0 when it is at 0.2.
                 assumes: share of the 207 no-news steps (279 - 72) that changed some world atom and can still be dropped. Unknown: doors and states may be needed
                 measure: 50 single drop tests on such steps: 50 x about 3.6 s = 3 minutes
  r_xwait        all-optimistic odds 2.17% -> 0 when it is at 0.6.
                 assumes: share of the 33 explicit waits removed. They come in 50, 200, 800 frames; one stop reached the same state 806 frames sooner without its wait
                 measure: padding ladder on the 33 wait trials alone: 33 x 4 forks, seconds
  r_inner        all-optimistic odds 2.17% -> 0 when it is at 0.8.
                 assumes: share of idle between one trial's clicks removed. 264 s of the 1,254 s is certain (581 click pairs at gaps of 3 and 9 frames)
                 measure: the same first pass: idle frames left per group
  e_tune         all-optimistic odds 2.17% -> 0 when it is at 0.1.
                 assumes: share of the tuning gap between the two hand runs that the optimizer closes outside the slide's skips (early inputs gave 4.66 s on 248 goals). High end 1 = the hand tuning (was 0.8)
                 measure: after stage 1, compare segment times with the references in S05, S07, S12
  For the odds of under 30:00 without track 6 (table 10), the five that move them most:
  v_walk         0 -> 0.124%.  measure: check each of the route's direct walks once in the engine (about 200 forks of under 1 s)
  q_atom         0 -> 0.0532%.  measure: 50 single drop tests on such steps: 50 x about 3.6 s = 3 minutes
  y_order        2.03e-08% -> 0.025%.  measure: nearest-neighbour tour over the kept progress visits with measured walks, checked on one Part: minutes
  e_cut          0.000226% -> 0.00589%.  measure: drop tests on dialogue-line clicks in the 10 longest matched visits
  y_need         0.000422% -> 0.00546%.  measure: 257 single drop tests of the progress trials: about 15 CPU-minutes
  For the median with track 6 (table 10), the five that move it most:
  p_exec         1:13:16.8 -> 57:56.5.  measure: make 30 of the route's trials again after a direct walk from another room: 30 forks, under a minute once the executor exists
  v_walk         1:09:34.6 -> 56:14.9.  measure: check each of the route's direct walks once in the engine (about 200 forks of under 1 s)
  pen_draw       1:10:12.8 -> 58:19.5.  measure: none before the pass; the pass reports it
  q_search       1:09:13.7 -> 58:37.1.  measure: one corridor run of the frozen binary at the catacombs from a shifted stop: 6-15 minutes
  p_drop         1:08:31.4 -> 58:52.8.  measure: time one drop round on 50 steps; project to the route (G4)

====================================================================================================
13. WHAT THE MODEL CANNOT KNOW: an unknown gain of X seconds, found for certain, taken off every draw
====================================================================================================
     X (s) | under 21:01.483, no track 6  | under 23:11.3, track 6   | under 28:01.4, track 6   | under 30:00, track 6
         0 | 0                            | 0                        | 2.43e-08%                | 7.41e-06%
        60 | 0                            | 0                        | 4.16e-07%                | 0.000139%
       120 | 0                            | 0                        | 8.16e-06%                | 0.000907%
       180 | 0                            | 0                        | 0.000147%                | 0.00387%
       300 | 4.34e-08%                    | 4.35e-08%                | 0.00397%                 | 0.0329%
       450 | 6.9e-05%                     | 6.92e-05%                | 0.0527%                  | 0.225%
       600 | 0.00477%                     | 0.00478%                 | 0.318%                   | 0.959%
       900 | 0.363%                       | 0.353%                   | 3.47%                    | 6.53%
      1200 | 3.76%                        | 3.68%                    | 13.5%                    | 19.1%
      1800 | 28.9%                        | 28.6%                    | 40.9%                    | 44.6%
  Unknown gain needed to reach 21:01.483 without track 6: 1403 s at the 10th percentile, 2403 s at the median, 4426 s at the 90th.
  Unknown glitches are set to zero in every table above.  Interactions are the parameter iota; R and G are independent given the two shared stages.

====================================================================================================
14. SUMMARY
====================================================================================================
  CD track 6 present  median 1:03:19.9, 10th-90th percentile 46:39.1 to 1:37:04.4; under:  60:00 44.6%   30:00 7.41e-06%   28:01.4 2.43e-08%   23:11.3 0   21:01.483 0
  CD track 6 removed  median 1:01:04.8, 10th-90th percentile 44:24.3 to 1:34:47.4; under:  60:00 48.4%   30:00 0.00117%   28:01.4 1.54e-05%   23:11.3 0   21:01.483 0
  Everything found at the centre (table 6): 44:08.4 with track 6, 41:58.6 without.  Hand runs: 28:01.4, 23:11.3, 21:01.5.

run time 126.7 s
```
