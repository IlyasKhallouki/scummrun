# Strategy comparison: how other work attacked this kind of problem

Written 2026-10-19 to answer the question: "What did other research on the same kind of game do,
what was the strategy, and is it better or worse than ours?"

Scope: adventure and text games, automated tool-assisted speedruns (TAS), speedrun routing, two-level
planning where the lower level is expensive to evaluate, RNG manipulation, earlier automation of MI1
and other SCUMM games, Monte Carlo and learned search, exact combinatorial optimisation, and LLMs that
write planning models. A critical revision on the same day checked the document against the
repository and primary sources, added §3.8 to §3.11, and corrected the claims listed in §7.
`docs/related-work.md` §3 already surveys formal solving, PDDL in games, LLM agents, puzzle
dependency charts and speedrun routing. This document does not repeat that survey. It goes back to those works only where they
matter for judging strategy, and adds what §3 lacks.

Each source has a status, also listed in §6:

- **verified**: a separate fact-check pass re-read the primary source. Where it found errors, this
  document uses the corrected facts, not the original research notes.
- **direct**: added or corrected in the 2026-10-19 critical revision. The writer of that revision
  fetched the primary source (paper, abstract or submission page) and took the facts from it, but no
  separate fact-check pass re-read it. This is weaker than verified, and matches `related-work.md`'s
  D, not its C.
- **unconfirmed**: a specific claim that the fact-check could not confirm. Marked inline.
- **unverified**: fetched by a research pass but not fact-checked. Treat as a lead.

"Inference:" marks my reading, not a statement from a source. Local numbers come from the repository
files named in place.

## 1. The short answer

- No published work was found that minimises play time in a graphic adventure (a search result, not a proof). Related work falls into eleven families: formal solvers for adventure and text games, planning and model learning over game models, exploration agents and test bots, automated TAS bots, two-level planning with an expensive lower level (task-and-motion planning), speedrun routing, RNG manipulation tools, earlier automation of the same game and engine, Monte Carlo and learned search (AlphaZero, MuZero and their single-player relatives), exact combinatorial optimisation (sequencing with precedence, SAT), and LLMs that write a planning model.
- On MI1 itself, the earlier work is LLM agents playing through engine hooks, a written proposal to route MI1 by shortest path over script dependencies, and hand-made libTAS TASes of other LucasArts games (§3.8). None plans or measures a route, and none of the agents reports a completion.
- Formal solvers reach a win but do not minimise time. Lester's model checker stops at 23 moves. Agents minimise time only in two recent benchmarks, and there they are far behind humans: the best PokéAgent entry is about 2.2x slower than the top human, and SpeedrunBench's models average 3.2x to 4.3x the world record.
- TAS bots search frames inside a route that humans fix, with no route planner. Beating the human RTA record is the low bar for any TAS; the stronger evidence is a bot movie that beats an earlier TAS. JaffarPlus drafted a SNES Prince of Persia movie that beat the previous TAS by about a minute, and LunarBot beat an earlier bot movie by 54 s. A* over a copy of the game's own physics, with time as the cost, won the 2009 Mario AI competition (§3.4).
- Our macro level holds up for glitchless Part I: a bytecode-cited PDDL model, A* with LM-cut over measured ticks keyed by position, and held-out seeds. No published equivalent was found. The Any% route is not planned this way yet: as of 2026-10-19 it is 465 moves written from runner sources and scripts, not produced by the planner, which is the TAS bots' situation. Our micro level and RNG handling are behind the TAS and community tools, and the seam between the two levels is where we are ad hoc.
- Learned search adds little here. MuZero learns a model because it has no simulator, and we have the exact engine. AlphaZero's learned value would need many runs of a route that has not been executed once. Single-player Monte Carlo search (nested Monte Carlo search, NRPA) is a credible alternative to the planned micro beam, but nobody has applied it to a speedrun. Exact sequencing solvers prove optimality for orderings of a few hundred tasks, but they cannot express the game's state beyond precedence, so their bounds do not carry over to our model (§3.9, §3.10).
- Theory for that seam comes from task-and-motion planning. LazySP's theorem guarantees a measure-and-replan loop is optimal when unmeasured costs are underestimated; PDDLStream does this in practice with cost 0. Our unmeasured costs are not guaranteed to be underestimates. MAXQ and SAHTN say a "fastest to the next milestone" search loses optimality unless it keeps branches whose end states differ.
- Change 1: log RNG draws in the bridge and carry RNG-distinct branches to each random event, with saruya's MI1 seed finder as the baseline (gb-tas-gen, saruya).
- Change 2: give the micro search a state hash and keep end-state-diverse branches (Jaffar, LunarBot, SAHTN, MAXQ).
- Change 3: search for sequence breaks with forks and a per-exit timing sweep. Path 686 differs from 685 only in walk time, so a state-diff search would miss it (Go-Explore, ScAmPER, Reveal-More).
- Change 4: cost unmeasured (action, position) pairs at an admissible floor, and claim only "optimal within the model" (LazySP, Churchill and Buro).
- Change 5: replay early on official ScummVM under libTAS (JaffarPlus's resync experience).

## 2. Comparison table

| Family | Representative works | What it optimises | Guarantees | Scale reached | Better than ours at | Worse than ours at |
|---|---|---|---|---|---|---|
| A. Formal solving of adventure and text games | Lester (ScAmPI), ScAmPER, Verbrugge and Zhang, Pickett et al. | Reachability of a win; script coverage | Witnesses valid for a modified interpreter (Lester, ScAmPER). Winnability under a hand model (Verbrugge). Step-minimal only on tiny NuSMV models (Pickett) | 23 moves on a hand-cleaned commercial sampler, in 5.5 h and 15 GB. Full Adventureland unsolved | The model is the interpreter, so it cannot leave out what the code allows; Lester's solver skipped the lamp. ScAmPER measures script coverage | No notion of time. Stops at tens of moves. Text engines with no walking or timing |
| B. Planning and model learning over game models | Balyo et al. 2024, SAM/N-SAM, TheoryCoder, Blackout | Test generation, model safety, level completion | N-SAM's learned model is safe under stated assumptions | Toy domains: a 4-action Unity RPG, IPC domains plus a Minecraft crafting task, 13 Baba Is You levels | Checks or repairs the model automatically from execution logs | No time costs. Small domains |
| C. Exploration agents and test bots | Go-Explore, Jericho agents, Reveal-More, ICARUS, PokéAgent | Score, coverage, completion. Only PokéAgent scores wall-clock time | None | Montezuma's Revenge solved with hand-made cells. ICARUS finishes a commercial Visionaire adventure in about 30 min with hints. PokéAgent winner: 40:13 to the first gym | Finds what no model contains (Go-Explore's treasure-room bug). Needs no model | No optimality. Billions of frames. PokéAgent's winner is 2.2x slower than the top human. ICARUS matches expert QA testers' fastest runs only with hand-written hints and memory of earlier playthroughs, and is not compared with speedrunners |
| D. Automated TAS and savestate search | Jaffar / JaffarPlus, LunarBot, gb-tas-gen, SpeedrunBench, the Mario AI A* agent | Frames, inside a route that humans fix; Mario AI: in-game time left | Shortest inside a segment only while nothing is evicted or wrongly merged. Brute-force proof claimed only for Prince of Persia level 1. Mario AI's A* is not optimal (inflated heuristic, 40 ms budget) | Published TASVideos movies. A SNES Prince of Persia draft that beat the previous TAS by about a minute. 2.7M states/s on 64 cores | Throughput, state deduplication, polishing finished movies, movies accepted on TASVideos | No route planner (our Any% has none yet either). Not found on any mouse-driven adventure. Porting the movie to the accepted emulator desyncs |
| E. Two-level planning with an expensive lower level (task-and-motion planning analogues) | LazySP, PDDLStream, Khandelwal et al., angelic hierarchical A* (AHA*), SAHTN, MAXQ | Path or plan cost, or feasibility | Exact conditions: LazySP is optimal when unevaluated costs are underestimated. AHA* and SAHTN are hierarchically optimal. MAXQ names the loss from optimising each subtask alone | Robot arms, PR2 pick-and-place with up to 6 objects, small grids, a 20-room robot simulation | A theory of when two-level optimisation stays optimal, which is our weak seam | Robots and toy domains. Sampled or simulated lower level. No held-out validation |
| F. Speedrun routing and game-time planning | Churchill and Buro (StarCraft), Groß et al., community routers | Makespan or total time | Churchill and Buro: optimal "within these abstractions". Others: none | 520 StarCraft build orders compared with professional replays. One game per community router | Puts time and concurrency in the state (Churchill and Buro) | Hand-written simulators or estimated times. Churchill and Buro never compared planned with in-game times. The one in-game success reported is JaV's TrackMania route, which improved leaderboard times (known only through Groß et al.) |
| G. RNG manipulation and seed search | saruya's MI1 catacombs guide and seed finder, PokéFinder, Project_Xs, gb-tas-gen's StateBuffer | A target random outcome at an attainable (seed, delay) | None | saruya's seeds are used in the MI1 record runs | Exists and works on MI1. Models seed-dependent draw counts. Scores the width of the working timing window. Searches back from the target | Black box: its author reports unexplained inconsistencies. One or two events per game |
| H. Same game and same engine | ScummBench, scummvm-mcp, ScummGameGraph, mikeSpeedyAdventures' libTAS TASes, the rejected Loom TAS | Agents: progress. TASes: time, by hand | None | Agents: ScummBench's stuck at the troll bridge in Part I; scummvm-mcp's "had a hard time" with the demo. Hand TASes of MI2 (29:47) and Day of the Tentacle, not on TASVideos | The TASes use libTAS on ScummVM, the toolchain TASVideos accepts. scummvm-mcp has bridge classes for 12 SCUMM game ids | No planner, no measured costs. The routing proposal has no code. No MI1 TAS is public |
| I. Monte Carlo and learned search | MuZero, AlphaZero, Agent57; NMCS, NRPA, SP-MCTS for single-player games | Game outcome or score. Single-player variants: a score in a deterministic puzzle | None. Nested search memorises the best sequence found so far and falls back to it | Superhuman board-game play; all 57 Atari games over the human benchmark; Morpion Solitaire records (NRPA 177; a 1976 human record of 170 stood until 2010) | Needs no hand-written evaluation. Nested search beat a 34-year-old human record in a deterministic single-player game | No speedrun application found. Learned models answer a problem we do not have (no simulator). Hours to a week per run for the records |
| J. Exact combinatorial optimisation | SOP and precedence-constrained ATSP solvers; SAT planning (Snowman) | Tour or plan cost, certified | Optimality proofs or gaps | SOPLIB instances of 200 to 700 nodes, half proved optimal by tree search. 43 of 51 Snowman levels proved length-optimal in 1 h | Lower bounds and optimality certificates at a scale our planner has not been tried on | Ordering with precedence only; inventory, money and scripted state do not fit. Step-optimal, not time-optimal (Snowman) |
| K. LLMs write the model, a planner solves it | Guan et al. 2023, LLM+P, LLM-Modulo; TheoryCoder | A correct (sometimes optimal) plan from a generated model | The planner's, relative to the generated model | GPT-4: 59 errors over three domain models, fixed with validator and expert feedback. LLM+P generates only the problem file | Guan et al. measure model quality with an expert error count, which we lack. TheoryCoder repairs the model automatically | Models written from natural language, not from the game's code. No time costs |
| scummrun (ours) | Glitchless Part I pipeline; Any% bridge, forks, executor and a route not produced by the planner | Glitchless: mean ticks over seeds. Any%: quarter-frames of one chosen seed's deterministic run, the same objective as a TAS | Glitchless: cost-optimal under the PDDL model and the mean measured cost table; the loop's stopping rule carries no guarantee (§4.3). Any%: none yet | Glitchless Part I: 67 actions, 22,292 ticks held-out mean. Any%: 465 moves in six segment files as of 2026-10-19, not yet executed | | |

## 3. The families in detail

### 3.1 Formal solving of adventure and text games

**Lester, "Solving Interactive Fiction Games via Partial Evaluation and Bounded Model Checking"**
(arXiv 2012.15365, 2020; tool ScAmPI) `[P-LESTER]` verified, https://arxiv.org/abs/2012.15365

- Strategy:
  - Runs the CBMC bounded model checker on a modified ScottFree C interpreter, with the game database
    compiled in.
  - The input loop becomes a bounded loop over nondeterministic verb and noun indices.
  - Every scripted line that ends the game, whether a win or a death, gets its own assertion. A human
    then reads each counterexample trace to tell wins from deaths.
  - `rand()` is replaced by a fixed-seed generator, and GET ALL/DROP ALL are removed.
  - Clang optimisation and generated unrolling macros stand in for the partial evaluator, which
    crashed.
- Results: tutorial games in 9, 14 and 14 moves (3 s, 25 s, 93 s). A hand-cleaned Adventureland
  Sampler (32 items, 100 actions, 18 rooms) in 23 moves, taking 5h32m34s and 15.4 GB. Adventureland
  (66 items, 170 actions, 34 rooms) and the original Sampler were not solved.
- The tutorial 5 solution crosses the dark rooms without taking the lamp, which a human would not do.
- Limits: about 12 bits of nondeterminism per command, 276 bits for 23 moves, "over 1,000 bits" for
  other SAGA games. There is no cost model. The author claims "a high degree of confidence" that the
  model matches the game, not exact validity, and reports one unsound modification that had to be
  backed out.

**Lester, "ScAmPER: Generating Test Suites to Maximise Code Coverage in Interactive Fiction Games"**
(TAP 2020) `[SCAMPER]` verified, https://centaur.reading.ac.uk/90238/

- Strategy:
  - An explicit-state model checker over a `next_state(state, verb, noun)` function in a modified
    ScottFree.
  - A static pass over the game database lists every possible command.
  - To expand a state, it walks to every reachable room by breadth-first search, then tries every
    GET, DROP and scripted trigger in each room. Commands that fail are discarded.
  - States are deduplicated by a hash that ignores the RNG and the lamp counter.
  - It prefers novel states: first entry to a room, first execution of a script line, first message,
    or a new combination of guard truth values.
- Results on the first five SAGA games:
  - coverage: rooms 54-92%, messages 65-89%, script actions 54-84%;
  - cost: 18-209 s and 32-431 MB per game.
- Which ingredients mattered:
  - Novelty mattered far more than search depth.
  - The AFL fuzzer failed, because every input runs the same interpreter-loop lines.
  - Bounded model checkers could not produce plays longer than one command.
- Limits: it never proves that something is unreachable. Completion of the five games is not
  reported. The later ScAmPI paper says ScAmPER explored most of the modified Sampler but did not
  complete it, "presumably" because it could not hold all 3 treasures at once.

**Verbrugge and Zhang, "Analyzing Computer Game Narratives"** (ICEC 2010) `[P-VERBRUGGE]` verified,
https://www.sable.mcgill.ca/~clump/papers/verbrugge-10-analyzing.pdf

- Strategy:
  - Backtracking depth-first search over all commands up to a depth bound, cutting cycles and caching
    dead ends.
  - Forward and backward dataflow analysis of each action's code gives conservative pre- and
    post-conditions.
  - Action B is tried after action A only if A's post-condition is compatible with B's precondition.
  - The goal-relevance pruning ("winning set", "useless objects") is in the authors' 2008 workshop
    slides, https://www.eecg.utoronto.ca/~steffan/workshops/08/cdp/verbrugge.pdf, not in this paper.
- Results:
  - Return to Zork chapters 1 and 2 were hand-modelled; chapter 2 has 22 rooms, 37 objects, 118 room
    commands and 4 global commands.
  - Branching factor fell from 122 to 10 on chapter 2.
  - Solutions were found within 5-6 min at most depth bounds. The earlier NuSMV solver could not
    finish chapter 1 in several days.
  - It reports the first winning path. The authors note that "finding a minimum length solution is a
    harder problem".
- The predecessor, Pickett et al. 2005, returned step-minimal traces with NuSMV, but only on tiny
  models (`related-work.md` §3.1).

Unverified leads in this family:

- **Volokh and Halfond (AIIDE 2023)**, https://ojs.aaai.org/index.php/AIIDE/article/view/27510. They
  symbolically execute Unity input-handling code and treat each path condition as one action. The
  derived action spaces matched or beat hand-made ones on state coverage in 12 of 16 games.
- **IJON (IEEE S&P 2020)**,
  https://www.casa.rub.de/forschung/publikationen/detail/ijon-exploring-deep-state-spaces-via-fuzzing.
  Annotation-guided fuzzing of Super Mario Bros. found wall-jumps and the 4-2 warp-zone exit, both
  speedrun glitches, without anyone pointing it at them.
- **AIJon (arXiv 2609.18457, 2026)**, https://arxiv.org/abs/2609.18457. An LLM writes those
  annotations. The fuzzer completed 18 of 28 levels.

What this family says about us: sound solvers run on the real interpreter, so they cannot omit
actions, and that let one find a shortcut a hand model would have forbidden (the lamp). They pay with
scale: nothing beyond about 23 moves, and no time. We sit on the other side of that trade. The useful
transfers are coverage measurement and static checks of each PDDL action against its script, not
whole-game solving.

### 3.2 Planning and model learning over game models

**Mordoch, Shperberg, Stern and Juba, "Learning Safe Numeric Planning Action Models"** (N-SAM;
arXiv 2312.10705 v2, 2025; builds on SAM, KR 2021) `[SAM]` verified, https://arxiv.org/abs/2312.10705

- Strategy: learn a PDDL action model from fully observed successful trajectories.
  - A literal becomes a precondition if it held every time the action ran.
  - Numeric preconditions are the convex hull of observed values.
  - Effects are the observed changes.
- Guarantee: plans from the learned model are executable. This holds only under the stated
  assumptions: conjunctive preconditions, linear numeric parts and no numeric error.
- Results: neither N-SAM nor N-SAM* produced an inapplicable plan, while the PMnr baseline did on up
  to 100% of problems (Sailing). On a Minecraft crafting domain, every trace had
  `(have pogo stick)` false, so the learners added a spurious `(not (have pogo stick))` precondition.
  This is the classic failure when executions do not vary enough.
- Coverage was measured with a Metric-FF plus ENHSP portfolio.
- What it means for us: SAM can serve as a refutation oracle for the extracted model. Log engine
  state before and after every accepted move. Any extractor precondition that was false in a
  successful execution is wrong.
  - Caveat: SCUMM scripts branch on state. A conditional effect can look like a missing precondition
    or effect, so a flag is a lead, not a proof.

Unverified leads in this family:

- **Balyo et al. 2024**, https://arxiv.org/abs/2402.12393. They learn PDDL from game logs and turn
  plans into regression tests, in a Unity RPG with 4 actions (`related-work.md` §3.2).
- **TheoryCoder (TMLR 2025)**, https://arxiv.org/abs/2503.20124.
  - Strategy: hand-given PDDL abstractions plus an LLM-written Python transition model. Fast Downward
    plans the abstract steps and breadth-first search refines each one. Any mismatch between the
    predicted and the observed transition triggers an automatic rewrite.
  - Results: it solved 13 Baba Is You levels with 7 LLM calls. On Sokoban level 5 the bilevel search
    took 24.58 s, while Fast Downward on a flat PDDL model timed out at 500 s.
  - It is the closest published shape to our extractor plus planner, with the repair step automated.
- **Krishnan, Williams and Martens (AIIDE 2020, Blackout)**, https://arxiv.org/abs/2103.05682. Failed
  actions are used as evidence to narrow preconditions. We discard failed attempts.
- **Silver et al. (AAAI 2023)**, https://arxiv.org/abs/2203.09634. During refinement, after each
  step, they check that the low-level state still abstracts to the expected abstract state. That finds
  the exact step where the model goes wrong. We check only by end-to-end replay.

### 3.3 Exploration agents and test bots

**Go-Explore** (Ecoffet et al.; arXiv 1901.10995, 2019; Nature 590, 2021, "First return, then
explore") `[P-GOEXPLORE]` verified, https://arxiv.org/abs/1901.10995 and
https://arxiv.org/abs/2004.12919

- Strategy:
  - Keep an archive of cells, either downscaled frames or hand-made features. For Montezuma's Revenge
    the features are room, position on a 16 px grid, level and keys; for Pitfall, room and position.
  - Pick a cell, restore the emulator state saved there, and explore for 100 frames with random
    actions that repeat the previous action with 95% probability.
  - Keep new cells. Replace a cell's entry when a trajectory reaches it with a higher score, or with
    the same score in fewer steps.
  - Restoring saved states instead of replaying cut simulated steps by at least 10x.
- Results:
  - Without domain knowledge, Phase 1 solved Montezuma level 1 in 57% of runs within 1.2 billion
    frames.
  - In 51 of the 57 runs that solved level 1, the best trajectory exploited the little-known
    "treasure room curse" bug. Those trajectories were filtered out before robustification, so the
    archive's mean maximum of 57,439 most likely includes the bug.
  - With hand-made cells (Nature version), robustified Montezuma averages 1,731,645, capped by the
    frame limit, and Pitfall averages 102,571 of a possible 112,000.
- Limits:
  - No optimality bound, and it needs billions of frames.
  - "Shorter" means fewer steps, not less time.
  - The authors note that because cells merge states, a better path to cell B does not guarantee
    that cell C is still reached.

**Jericho** (Hausknecht et al., AAAI 2020) `[P-JERICHO]` verified, https://arxiv.org/abs/1909.05398

- Valid-action detection: save the state, try every template filled with every pair of interactive
  objects, keep an action if the world object tree changed, and restore.
  - Changes that live only in global variables give false negatives.
  - The current code also counts score changes and per-game RAM addresses as changes.
  - It keeps one representative action per distinct diff.
- Templates cut Zork1's action space to about 115M. Only the DRRN agent chooses among the valid
  actions; TDQN generates over the full space.
- Agents score by game points: DRRN completes 10.7% of the games' maximum scores on average, TDQN
  6.1%.
- Lesson for us: a state-diff sweep cannot find a path-686-class improvement. Path 686 runs path
  685's exit, with the same transition and the same arrival, and differs only in the walk to the
  object (162 against 216 ticks; `docs/comparison.md`, rows #16 and #58). Jericho's per-diff grouping
  would merge the two. A sweep for speedrunning must record time per alternative, not only the state
  change.

**Reveal-More** (Chang, Aytemiz and Smith, IEEE CoG 2019) `[REVEALMORE]` verified,
https://adamsmith.as/papers/PID5953685.pdf

- Strategy: a tester plays for about 15 min with a savestate every 3 s. Rapidly-exploring random
  trees then explore from each savestate for an equal share of the budget, using button frequencies
  taken from player traces.
- Results: about 4.6x the human's tile coverage in Super Mario World and about 1.9x in Zelda: A Link
  to the Past, with machine time equal to the human's play time.
  - RRT alone stalls: in 17 minutes it never destroys the one shrub that opens the Zelda dungeon.
  - A moderate number of seed states works best. Tuning that number more than doubled coverage, but
    the authors say the optimum depends on the game.
  - It reports no bugs.
- Lesson for us: seed exploration from points along a good route. A human-quality route supplies
  depth, and exploration supplies breadth.

**ICARUS** (Pfau, Smeddinck and Malaka, CHI PLAY 2017) `[ICARUS]` verified,
https://www.smeddinck.com/files/papers/AutomatedGameTestingwithICARUS.pdf

- Strategy:
  - A Lua solver inside the Visionaire engine lists the actions available on the current screen.
  - It gives +1 for an unrepeatable state change (inventory, quest step, new area) and -1 otherwise.
  - Short-term and long-term memory key rewards by action (type, target, item), not by state.
  - "Educated guessing" uses engine metadata to drop combinations that only trigger the default
    response.
  - Hand-written hints cover puzzles it would rarely solve.
- Results on The Pillars of the Earth:
  - Educated guessing cut time by about 40%. Adding short-term memory cut it by more than half.
  - With every feature, including memory from earlier playthroughs and the hints, it finishes in
    about 30 min, "on the same level as the fastest speedruns of expert QA testers".
  - In one Anna's Quest scene, educated guessing cut item-item combinations from 121 to 35.
- Limits: real-time play, no frame control and no optimality claim. The game was still under
  development at the time. It is the only system here that completes commercial point-and-click
  games on its own, and it needs hints to do so.

**The PokéAgent Challenge, speedrunning track** (Karten et al., arXiv 2603.15563, 2026; NeurIPS 2025
competition) `[POKEAGENT]` verified, https://arxiv.org/abs/2603.15563

- Strategy: Pokémon Emerald up to the first gym, 15 milestones. Runs are scored by completion, then
  wall-clock time, then action count.
  - The environment runs in real time, so agent thinking time counts.
  - The winner, Heatz, used "Scripted Policy Distillation": an LLM splits the task into subgoals and
    writes a script for each, a DQN imitates the scripts, and RL refines the result.
- Results:
  - 6 of 22 teams finished. Heatz took 40:13, about 2.2x the top human's time (the paper rounds the
    human time to 18 min).
  - Second place used pure recurrent PPO (1:14:43). The best pure LLM harness took 1:29:17.
  - Heatz used 1,608 actions, against 649 for a training-free LLM harness that placed 5th. Heatz won
    on execution speed per step, not on fewer actions.
- Lesson for us: it weakly supports our split (scripted high level, optimised low level), but much of
  the winner's edge is removing inference latency. That cost does not exist in our paused,
  tick-counted engine.

Unverified leads in this family:

- **MC!Q*BERT** (https://arxiv.org/abs/2006.07409). It detects a bottleneck when neither score nor
  its knowledge graph grows, then backtracks along its best trajectory. It reached 41.6 of 350 on
  Zork1.
- **XTX** (https://arxiv.org/abs/2201.01251). It averages 103.4 of 350 on Zork1.
- **Intelligent Go-Explore** (https://arxiv.org/abs/2405.15143). An LLM picks cells and actions. It
  reached 88% on TextWorld Coin Collector, where ReAct and Reflexion scored 0%.
- **Madotto et al.** (https://arxiv.org/abs/2001.08868). Go-Explore with admissible actions always
  found the optimal 30-step Coin Collector trajectory.
- **Ron Gilbert's TesterTron** for Thimbleweed Park,
  https://blog.thimbleweedpark.com/yay_testertron3000.html. Random play found a bug from a click one
  frame before something became clickable. Gilbert says it "wasn't very good at finding game logic
  bugs".
- **Chrabaszcz et al.** (https://arxiv.org/abs/1802.08842). An evolution strategy found a Q*bert
  scoring bug that reproduced in only 8 of 30 evaluation runs.
- **Burda et al.** (https://arxiv.org/abs/1808.04355). Curiosity is attracted to anything that
  changes on its own, the "noisy TV" problem.
- **CA2** (https://arxiv.org/abs/2605.13918). Function-call traces serve as an exploration signal.
- **FlashAdventure** (https://arxiv.org/abs/2509.01052). The best GUI agent succeeds on 5.88% of 34
  Flash adventures (`related-work.md` §3.4).

What this family says about us: model-free exploration finds what a model leaves out (the
treasure-room curse, the elevator bug), but it gives no optimality, needs huge budgets, and where
timed is about twice as slow as humans. Its primitive, an exact restorable state, is one we already
have. The literature supports an exploration layer that feeds findings back to the planner, not one
that replaces it.

### 3.4 Automated TAS and savestate search

**Jaffar and JaffarPlus** (Sergio Martin, "eien86", 2021 to present) `[JAFFAR]` verified,
https://github.com/ToolAssisted-run/jaffarPlus (formerly SergioMartin86/jaffarPlus) and
https://github.com/SergioMartin86/jaffar

- Strategy (JaffarPlus documentation):
  - Search by depth, one frame (plus any frame-skip) per step.
  - Each frontier state is expanded with every input from condition-gated "Allowed Input Sets".
  - Successors are dropped if they hit a fail rule or if their hash, computed over user-chosen "Hash
    Properties", was already seen.
  - The next frontier is ordered by a reward built from JSON rules and "magnets".
  - The state database is memory-capped. When it is full, the lowest-ranked states are evicted, and
    "checkpoint tolerance" drops lineages that lag behind a milestone.
  - So it is a shortest-first breadth search only while nothing is evicted; otherwise it is a
    reward-ordered beam. The docs warn that too coarse a hash "may miss the solution", and advise
    leaving RNG out of it.
  - Routes come from humans. Submission #7134 constrained the search "to that of a pre-determined
    route".
- Results:
  - **Jaffar on Prince of Persia DOS** (https://tasvideos.org/7134S, published as
    https://tasvideos.org/4477M, recorded on JPC-RR):
    - 13:08.53 real time, 12:10.000 in-game time.
    - 27 s faster than the RTA record and 9 s faster than the human sum of best, at submission.
    - The level 1 route is "proven to be optimal by the bot on sheer brute force"; no other level is
      claimed optimal.
    - 817.6 billion rerecords by the author's strict count (the site field shows 172).
    - Direct (critical revision): the only earlier TAS the submission compares against is David
      Newton's level-skip TAS (12:20.58), a different goal, so the RTA record was the baseline.
      eien86 obsoleted 4477M himself with https://tasvideos.org/5206M, 10 s less in-game time on
      libTAS + PCem; that page does not say whether a bot was used.
  - **JaffarPlus on Out of This World DOS** (https://tasvideos.org/8237S, libTAS 1.4.4 + PCem):
    - 2.7M states/s on a 64-core Threadripper 3990X with 256 GB, and about 3 weeks in total.
    - The author fixed "many desyncs" by hand, caused by RNG tied to time and inputs, and re-synced
      twice.
  - **JaffarPlus on SNES Prince of Persia** (https://tasvideos.org/8784S, published, with
    Challenger): 32:33.90 against the previous 33:31.20. The first draft took a month on the Snes9x
    core, and resyncing to the BSNES core took a week.
    - Direct (critical revision): the previous 33:31.20 is a published TAS (3271M, Challenger and
      Odongdong), so this is a bot-drafted movie beating an earlier TAS. The bot made the first
      draft; Challenger then refined it by hand, and the submission suggests "a full bot-driven
      re-work" could still help.
  - **JaffarPlus on Game Gear Prince of Persia** (https://tasvideos.org/9533S): 13:40.24, published
    2025.
  - Unconfirmed: that a run made with JaffarPlus itself, rather than Jaffar, beat an RTA record.
- Limits:
  - It needs a fast savestate and frame-advance API.
  - Reward and hash rules are written by hand for each game.
  - Search is bounded by memory, and the route is fixed by humans.
  - Most cores are stripped real emulators; Prince of Persia and Another World use reimplementations
    of the game. Out of This World was searched on a reimplementation core and moved to libTAS+PCem
    with many desyncs fixed by hand. Even the SNES Prince of Persia run, searched on a real emulator
    core (Snes9x), took a week to resync to the BSNES core.

**LunarBot** (eien86, TASVideos #9499, 2025) `[LUNARBOT]` verified, https://tasvideos.org/9499S and
https://github.com/SergioMartin86/LunarBot

- Strategy: eien86 calls JaffarPlus's frame-level breadth-first search "entirely the wrong approach"
  for NES Lunar Pool, because a shot and its outcome are hundreds of frames apart. LunarBot instead:
  - branches on strokes (angle and power);
  - simulates each shot until the balls stop;
  - keeps a beam of up to 500 candidates, ranked by a hand-calibrated reward (-500 per pending ball,
    -20 times the bonus rate, minus the frame count);
  - drops duplicate outcomes by hashing the state in the first frames of each shot.
- Results:
  - 22:53.62, published as https://tasvideos.org/6368M.
  - 54 s faster than Bisqwit's earlier movie (https://tasvideos.org/1565M), which was also made with
    a bot.
  - About 15k shots/s on 256 threads, finished in a few weeks.
- Lesson for us: branch on moves rather than frames, then deduplicate outcomes. That is the design
  `docs/anypercent/plan.md` describes, plus the deduplication we lack.

**gb-tas-gen** (MrWint, 2014-2017) `[GBTASGEN]` verified, https://github.com/MrWint/gb-tas-gen

- Strategy:
  - A Java program drives a modified libgambatte, and a run is written as a program of Segments.
  - Savestates can be taken mid-frame. `runToNextInputFrame` runs until the game reads the joypad,
    the same idea as our executor's input gate.
  - Each Segment maps a StateBuffer (a set of variants of the current state) to the next one.
- The StateBuffer:
  - It keeps at most one state per (step count, `rngState`). For Pokémon, `rngState` packs both RNG
    bytes and a 14-bit timer phase.
  - It prunes from the highest step count, dropping the state whose RNG metric is least distinct from
    the others.
  - The default cap is 1, which makes it greedy. Segments raise it to 10-50, once to 256.
- Random events: `DelayMoveSegment` tries 0, 1, 2 and more delay frames for each buffered state,
  runs a check, and keeps every state that passes. By default it stops 4 frames after the first
  success.
- Results: TASVideos #5034, Pokémon Red and Blue "Co-op Diploma" in 3:48:04.10, published, was
  generated with it; the submission links the repository (https://tasvideos.org/5034S). "The routing
  alone took months."
- Lesson for us: carry RNG-distinct variants forward and choose late, and treat delay frames as a
  search dimension. This answers our problem that RNG call indices shift with input timing.

**The Incredible Machine TAS** (TASVideos #10017, cancelled; #10481, published) `[TIM]` verified,
https://tasvideos.org/10017S and https://tasvideos.org/10481S

- #10017 was eien86's manual run of this mouse-driven DOS puzzle game (26:23.238, 1,601 rerecords).
  Of the mouse he wrote: "I did not obsess with it".
- It was cancelled on 2025-11-24, mainly because the 1993 machine preset runs the game more smoothly
  than the 1991 preset used. The judge also expected mouse and solution improvements.
- The resubmission, #10481 by DrD2k9 and eien86, is published at 21:57.419 with 71,543 rerecords.
  Its listed gains: newer machine settings, mouse-movement optimisation and faster puzzle solutions.
  No bot is mentioned.
- Inference: no bot-driven search over mouse input in a point-and-click game was found, even from
  the leading search-bot author. The emulator's machine configuration alone was enough to trigger a
  cancellation.

**SpeedrunBench** (Fujinuma et al., arXiv 2610.08076, 6 Oct 2026) `[SRBENCH]` verified,
https://arxiv.org/abs/2610.08076

- Strategy:
  - LLM agents edit a complete frame-indexed action trace.
  - A deterministic scorer replays the trace and returns success and the frame count.
  - The agent sees its best trace and its attempt history, and proposes edits for 200 turns.
  - An optional "in-turn scorer" lets it test up to 200 candidates per turn.
  - It covers nine games and no adventure game.
- Results:
  - No agent reaches a world record. Per-model means run from 316% to 432% of the human record.
  - With the in-turn scorer, results reach 7% (SuperTux 1-1) and 3% (Super Mario Land 1-1, after 220K
    traces over about 114 h) of the record.
  - Starting from Bisqwit's obsoleted 2003 Super Mario Bros. TAS, one agent found a trace 3 frames
    shorter.
  - Merging two near-optimal traces saved 26 frames; the paper does not say who did the merge.
  - It assumes a deterministic, fixed-seed game and does not model RNG.
- Lesson for us: splicing the best stretches of different movies and re-verifying end to end can
  still save frames on near-optimal input.

**The Mario AI competition A* agent** (Togelius, Karakovskiy and Baumgarten, IEEE CEC 2010;
Togelius, Shaker, Karakovskiy and Yannakakis, AI Magazine 2013) `[MARIOAI]` direct,
http://julian.togelius.com/Togelius2010The.pdf and
https://www.um.edu.mt/library/oar/bitstream/123456789/29888/1/The_Mario_AI_championship_2009-2012_2013.pdf

This is the closest academic precedent for A* over game states with time as the cost.

- Strategy (Baumgarten's 2009 winner):
  - The authors frame the task as "a path optimisation problem: What is the quickest route through
    the environment that doesn't get Mario killed?"
  - The agent copies the game's whole physics engine into the controller and uses it as the
    forward model. A node is the full simulated world state. A successor is one 40 ms step under
    one input combination.
  - The cost so far is elapsed time. The authors note that an admissible heuristic "would have to
    assume Mario runs with maximum speed towards the goal, which is often not the case". They use
    instead the simulated time to reach the edge of the visible window when accelerating as fast as
    possible, which they call more accurate.
  - It is not run optimally: the heuristic is multiplied by w = 1.11. On timeout the agent takes
    the node with the best heuristic, and it re-plans every two game updates.
- Results:
  - The three A* agents took the top three places in the first phase (17,264 progress for the
    winner). "None of the agents that were not based on A* outperformed the heuristic controller"
    supplied with the software.
  - In the second phase both of the top two A* agents finished every level. The winner was decided
    on in-game time left.
- Limits, as the organisers state them:
  - The generated levels had no dead ends, and the agents slowed down at vertical walls,
    "suggesting that A* would break down when faced with a dead end".
  - In 2010 the levels gained dead ends, and "all controllers that were based on pure search in
    state space performed worse". The winner, REALM, evolves high-level rules offline and executes
    them with A*.
- What it means for us:
  - A copied exact engine plus A* on time is a design that wins when the level is known and has no
    traps. Our fork gives the same exact forward model, with no copy to drift from the game.
  - Recommendation 5's floor for unmeasured walks, distance over the maximum walk speed, is the
    estimate Baumgarten names as admissible but loose for Mario. Inference: it is tighter for
    Guybrush, whose walk has no acceleration phase: each frame's step is set by the walk speed and
    the room's scale (`docs/research/anypercent-engine-mechanics.md`).
  - REALM's split, rules above and A* below, is our two-level split. Dead ends hurt the bottom level
    when it searches alone, which is an argument for keeping the planner above it.

Unverified leads in this family:

- **TMInterface bruteforce** (https://donadigo.com/tminterface/what-is-bf) perturbs inputs of a
  finished TrackMania replay. It restores a savestate taken before the changed input, and keeps the
  change if a later checkpoint or trigger is reached sooner.
- **bxt-rs** (https://github.com/YaLTeR/bxt-rs/tree/master/src/modules/tas_optimizer) does random
  hill-climbing on Half-Life TAS scripts, then a minimise pass that deletes unused inputs.
- **learnfun/playfun** (Murphy, SIGBOVIK 2013, http://tom7.org/mario/mario.pdf) adds "chop" and
  "ablation" operators that delete or blank input spans. Its learned objectives were exploited: it
  pauses Tetris forever.
- **Dragster** (Omnigamer, https://tasvideos.org/Forum/Posts/453106; MrWint's DP solver): a
  disassembly-derived model plus dynamic programming. The author calls the result "optimal, given no
  further discoveries that violate the assumption of the game model".
- **BizHawk Basic Bot** (https://tasvideos.org/Bizhawk/BasicBot): random inputs over a frame window,
  scored on a RAM value.
- **libTAS** (https://github.com/clementgallet/libTAS): the accepted route for ScummVM TASes on
  TASVideos (`related-work.md` §2.2). Its `fork()` only writes a savestate in the background; it is
  not a search branch.
- **Machado et al., "The Brute"** (https://arxiv.org/abs/1709.06009): an open-loop tree search over
  action histories that collapses as soon as the game is not deterministic.
- **Cook et al. 2025** (https://ceur-ws.org/Vol-4090/paper1.pdf) write that "TAS runs are not guided
  by AI search – they are made manually". Jaffar's published Prince of Persia movie (2021) and the
  JaffarPlus movies above contradict that.

What this family says about us: our two-level split is standard TAS practice. The bots are far ahead
on throughput, deduplication and polishing, and their movies are accepted. They have no route
planner, and none was found on a mouse-driven adventure. Our Any% route has no planner yet either
(§4.1).

On throughput, our only figure is "three branches and a straight replay took under 2 s"
(`docs/anypercent/plan.md`), and that includes engine start-up. Against JaffarPlus's 2.7M states/s,
the gap is several orders of magnitude, but the ratio is unconfirmed, because we have not measured
branches per second.

### 3.5 Two-level planning with an expensive lower level (task-and-motion planning analogues)

Our design is a symbolic plan over costs that a slow, exact simulator measures, with a search over
inputs below it. That is the shape of task-and-motion planning (TAMP): a robot plans which objects to
move in what order, then plans the motions for each step. This literature states exactly when such a
loop stays optimal.

**Khandelwal et al., "Planning in Action Language BC while Learning Action Costs for Mobile
Robots"** (ICAPS 2014) `[KHANDELWAL]` verified, https://eprints.whiterose.ac.uk/97763/1/icaps14-bc.pdf

- Strategy:
  - An ASP planner (CLINGO) minimises total action cost.
  - Navigation cost depends on the robot's location: the door it came from and the room.
  - After each successful action, the estimate is updated by a moving average.
  - Unmeasured costs start well below their true values, and the robot re-plans every episode.
- Results:
  - Simulation (20 rooms, 25 doors): it converged to the optimal collection order by episode 17.
  - Real robot (5 rooms, 8 doors): by episode 12, with planning typically taking 10-15 s, including
    the proof of optimality.
  - In simulation the search was capped at 35 steps and 1 minute, and pruned by two hand heuristics.
- Stated flaw: exploration from optimistic starting costs is "short-lived". Once a plan's estimate
  rises above the best, it is never tried again. The authors name epsilon-greedy as the fix and leave
  it to future work.
- Inference: this is the published precedent for our optimistic loop, with location-keyed learned
  costs. Ours is richer (103 position tokens, LM-cut, an exact engine, held-out seeds), and it has the
  same flaw.

**Dellin and Srinivasa, LazySP** (ICAPS 2016) `[LAZYSP]` verified, https://arxiv.org/abs/1603.03490

- Strategy: shortest path when evaluating an edge is expensive.
  1. Find a shortest path, using true weights for evaluated edges and an estimate for the rest.
  2. If every edge on it is evaluated, return it.
  3. Otherwise evaluate the edges a selector picks, and repeat.
- Theorem 2: if the estimate is at most epsilon times the true weight, the result is within a factor
  epsilon of optimal. "The optimality of LazySP depends on the admissibility of west in the same way
  that the optimality of A* depends on the admissibility of its goal heuristic h."
- Results on robot-arm roadmaps:
  - the A*-like Expand selector evaluated 949 edges in 270 s online;
  - the Alternate selector evaluated 55 edges in 6 s;
  - the Partition selector evaluated 48 edges in 5.8 s online, but 497 s in total, because it needs
    an initialisation step.
  - The selectors that win evaluate one well-chosen edge per iteration.
- What it means for us:
  - Our stopping rule (the plan contains only measured actions, and repeats) is LazySP's termination
    rule, plus the requirement that the plan repeat.
  - Our estimate for an unmeasured (action, position) pair is the action's minimum measured duration,
    which `docs/optimization.md` itself calls "not a proven lower bound". So the theorem does not
    apply, and "converged" carries no guarantee.
  - The loop is not where our planning time goes: in the last optimisation it needed 2 iterations,
    and the other 43 of 45 Fast Downward runs generated candidates. The edge selector is not worth
    changing.

**Garrett, Lozano-Pérez and Kaelbling, PDDLStream** (ICAPS 2020) `[PDDLSTREAM]` verified,
https://arxiv.org/abs/1802.08705

- Strategy:
  - PDDL is extended with "streams": black-box generators that certify facts, and external cost
    functions.
  - The planners plan with optimistic placeholder objects. Unevaluated costs count as 0, "a lower
    bound on the nonnegative cost function value". Fast Downward does the search.
  - Only the streams that support the current optimistic plan are evaluated. One variant stops as
    soon as a cost exceeds the current bound.
  - Another variant splits time between searching and sampling.
  - The laziness principle is credited to LazySP's authors.
- Guarantees: semi-complete and anytime. It makes no optimality claim.
- Results: 100 random problems in 3 simulated domains with a 2-minute timeout, and four real PR2
  tasks.
- What it means for us:
  - A measured cost can be modelled as a stream: zero until measured, then exact. Our engine is
    deterministic, so a stream's value can be cached.
  - The planner would ask only for the measurements its current plan needs.

**Marthi, Russell and Wolfe, "Angelic Hierarchical Planning"** (AHA*; ICAPS 2008) `[AHA]` verified,
https://aima.eecs.berkeley.edu/~russell/papers/icaps08-hla.pdf

- Strategy:
  - Each high-level action has an optimistic description (a superset of the reachable states and a
    lower bound on cost) and a pessimistic one (a subset of the reachable states and an upper bound).
  - AHA* runs A* over refinements ordered by optimistic cost. It prunes a plan when another node's
    pessimistic value dominates its optimistic value.
  - It returns hierarchically optimal plans (Theorem 4): the best plan the hierarchy allows, which is
    truly optimal only if the hierarchy preserves optimality.
- Results: on nav-switch size 20, flat A* took 176 s against 3 s for AHA*. On warehouse instance 4,
  flat A* timed out at 10^4 s, AHA* took 526 s and the satisficing variant 27 s. AHA* times out on the
  largest instances because it "must ... prove that all other high-level plans have higher cost".
- The bounds are hand-written and must provably hold; costs are general, not unit.
- What it means for us: an optimality claim for a two-level plan is relative to the hierarchy and
  needs true bounds. A per-pair sample minimum is not a lower bound across contexts that were never
  measured, so it cannot certify anything.

**Wolfe, Marthi and Russell, "Combined Task and Motion Planning for Mobile Manipulation"** (SAHTN;
ICAPS 2010) `[SAHTN]` verified,
https://www2.eecs.berkeley.edu/Pubs/TechRpts/2010/EECS-2010-27.html

- Strategy:
  - A task hierarchy goes all the way down to primitives that call motion planners, and continuous
    choices are sampled.
  - SAHTN caches, for each (high-level action, relevant state variables), a map from every reachable
    end state to the best cost and plan. It then recombines the result with the irrelevant variables.
- Theorem 1: hierarchically optimal, given a correct relevant-variables function, an acyclic
  hierarchy and a finite state space.
- Motivation: top-down separation gives plans that "may be inefficient or even infeasible due to
  missed lower-level synergies and conflicts".
- Results are runtimes only. On larger taxi and pick-and-place problems, only SAHTN finished within
  512 MB or 10 min.
- What it means for us:
  - Our position token is a claim about which variables are relevant to an action's cost. The
    context-variance table in `docs/optimization.md` flags 11 actions where that claim fails. For
    example, `walk dock lookout` takes 1008 ticks after leaving the bar and 642 after `walk cu-dock
    dock`.
  - SAHTN's remedy is to keep a cost per end state, not one number per action.

**Dietterich, "Hierarchical Reinforcement Learning with the MAXQ Value Function Decomposition"**
(JAIR 13, 2000) `[MAXQ]` verified, https://arxiv.org/abs/cs/9905014

- It distinguishes three kinds of optimality:
  - optimal;
  - hierarchically optimal: the best policy the hierarchy allows;
  - recursively optimal: each subtask optimal given its children, regardless of context.
- In the two-room maze (Figure 6), the fastest exit from each room gives a policy that is "neither
  hierarchically optimal nor optimal".
- Setting each exit's terminal reward to its value for the rest of the task restores optimality,
  provided all primitive actions are available inside the subtask.
- Dean and Lin iterate those values to convergence. That requires the full state at the subtask
  boundaries, with no state abstraction.
- What it means for us:
  - `docs/anypercent/plan.md` ranks branches by "RTA quarter-frames to a common milestone". That is a
    subtask whose terminal reward is 0 in every end state, so it is recursively optimal at best, and
    the beam's pruning makes it approximate on top of that.
  - The fastest way to reach a milestone can leave Guybrush at a worse spot, a worse RNG index or a
    worse walk phase for the next move.
  - A one-step lookahead after the milestone helps, but it is not Dean-Lin. It moves the same
    blindness to the next boundary.

Unverified leads in this family:

- **Srivastava et al. (ICRA 2014)**, https://aima.eecs.berkeley.edu/~russell/papers/icra14-planrob.pdf.
  When motion planning fails, the cause becomes a symbolic fact (for example "b2 obstructs the
  trajectory") and Fast Downward re-plans from there. Our executor raises `RouteError` and stops.
- **Ortiz-Haro et al. (ICAPS 2022)**, https://ojs.aaai.org/index.php/ICAPS/article/view/19811. They
  forbid the shortest infeasible plan prefix and plan again. It solved 18 of 20 problems, against 12
  for the baseline.
- **Toussaint and Lopes (ICRA 2017)**, https://argmin.lis.tu-berlin.de/papers/17-toussaint-ICRA.pdf.
  A ladder of cheap lower bounds prunes candidates before the expensive full evaluation.
- **Garrett et al., "Integrated Task and Motion Planning"** (2021 survey),
  https://arxiv.org/abs/2010.01083. It sorts these systems by how the two levels interact, and states
  that the assumption that each abstract step can be refined independently "rarely holds".
- **Kaelbling and Lozano-Pérez, "Hierarchical Task and Motion Planning in the Now"** (ICRA 2011),
  https://people.csail.mit.edu/tlp/pdf/2011/hpnICRA11Final.pdf. When a low-level choice affects later
  steps, expose it at the higher level.
- **Bandres, Bonet and Geffner, Rollout IW** (AAAI 2018), https://arxiv.org/abs/1801.03354.
  Novelty-pruned search over Atari emulator states, with no model.

What this family says about us: our measure-and-replan loop is a recognised pattern, not an invention,
and the theory gives exact conditions for when it is optimal. We meet neither condition today: our
unmeasured costs are not admissible, and our micro search ignores end states.

### 3.6 Speedrun routing and game-time planning

**Churchill and Buro, "Build Order Optimization in StarCraft"** (AIIDE 2011) `[CHURCHILL]` verified,
https://ojs.aaai.org/index.php/AIIDE/article/view/12435

- Strategy:
  - Makespan minimisation over concurrent actions. Each action has a duration in frames and resources
    it requires, borrows, consumes and produces.
  - The state includes the clock, the resources, the actions in progress and the income.
  - Depth-first branch and bound with admissible lower bounds (a landmark bound and a resource bound).
  - Every action is issued at its earliest legal time. The authors assert this is optimal "since
    hoarding resources cannot reduce the total makespan"; they state it rather than prove it.
  - Doubling macros reduce search depth.
  - The simulator's constants are measured from professional games, and optimality is claimed only
    "within these abstractions".
  - Evaluation plans the whole goal first. On a timeout it plans increments cut from the
    professional's own build order, advancing in the simulator. It does not re-plan from the observed
    game state.
- Results:
  - 520 prefix build orders from 100 Protoss replays.
  - The ratio of planned to professional makespan, at the 75th percentile, is 0.97 to 1.04 depending
    on game phase.
  - In the early game, 90% of searches used at most 1.5% of the plan's makespan in CPU time, and
    0.02% with macros.
  - Inside a bot it beat amateur players, with "specific results omitted"; predicted and actual times
    were not compared.
- What it means for us:
  - Earliest-legal issue is right only when waiting never helps, and waiting can help before a random
    event.
  - Their scope wording is the honest one to copy.

**Groß, Zühlke and Naujoks, "Automating Speedrun Routing: Overview and Vision"** (EvoApplications
2022) `[P-GROSS]` verified, https://arxiv.org/abs/2106.01182

- A vision paper with no experiments. It models routing as a weighted graph of game events and lists
  the challenges: defining nodes and edges, dynamic weights, repeatable events, and several
  objectives.
- On solvers: it calls Dijkstra and A* "unsuitable given the edge dynamics" on an event graph. It
  concedes that a detailed state graph makes the weights static, but at the price of "an immense
  increase of nodes and edges". It favours evolutionary algorithms, ant colonies and reinforcement
  learning.
- It separates operational routing (inputs on frames) from strategic routing (event order).
- What it means for us: a factored PDDL state searched with LM-cut answers their blowup objection.
  Their "hidden gain" detours (steps that cost time now and save more later) and their second
  objective (risk) are worth adding.

**Lafond, "The complexity of speedrunning video games"** (FUN 2018, LIPIcs 100, 27:1-27:19)
`[P-LAFOND]` direct, https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.FUN.2018.27

- Model: the player chooses the order of stages, as in Mega Man. Each finished stage gives an
  ability that saves time in later events, and the saving depends on the best ability held. This
  generalises feedback arc set.
- Results for routing:
  - W[2]-hard in the time lost, and hard to approximate within O(log n). Theorem 8 holds even when
    the dependency graph's underlying undirected graph is a tree and only one stage has more than
    one event.
  - Fixed-parameter tractable in the maximum in-degree plus the treewidth of the dependency graph.
- The paper also treats damage boosting (a knapsack generalisation with an FPTAS).
- What it means for us: an item that makes later actions faster is Lafond's stage ability. Under
  standard complexity assumptions, no algorithm is fixed-parameter tractable in the time lost, even
  when the dependency graph is a tree. A search that is exponential in the worst case, like ours, is
  therefore not a design flaw. Inference: a PDDL
  model can represent such a saving exactly, as a faster variant of the later action that has the
  ability as a precondition. Our cost table keys costs by position only, so a saving that depends on
  something else is averaged away.

Unverified leads in this family:

- **Iškovs' Morrowind router** (simulated annealing over a quest graph) and **JaV's TrackMania
  checkpoint router** (genetic algorithm). Both are known here only through Groß et al.; their
  primary sources were not fetched.
- **Linesight** (https://github.com/Linesight-RL/linesight). Reinforcement learning for TrackMania;
  its world-record claims are self-reported.

### 3.7 RNG manipulation and seed search

**saruya, "Catacombs RNG manipulation" and "Seed finder for The Secret of Monkey Island"**
(speedrun.com, 2026) `[SARUYA]` verified, https://www.speedrun.com/tsomi/guides/hw8us and
https://www.speedrun.com/tsomi/resources/2z7qm

This is prior work on the same game, the same engine and the same generator.

- The catacombs generator, as the guide reverse-engineers it:
  - Each room draws a set with `rand(0..2)`, redrawn until it differs from the previous room's set.
    The first room treats the previous set as set 0.
  - A room with fewer than two exits is redrawn.
  - When the previous correct exit was on the right-hand screen, an extra exit draw is discarded. On
    CD this also happens unpredictably on first entry.
  - The entrance and exit are drawn with rejection loops.
  - Six net correct exits are needed.
- How the record runs manipulate it:
  - Fix ScummVM's random seed.
  - Take a wrong exit first, which "increases the chances of finding good seeds by a factor of 60".
  - Walk to the entrance while reading the compass. That is a cutscene with no RNG calls, so the
    number of frames from load to entry is fixed.
  - Save there, return to the ScummVM launcher (a plain quickload does not work), load, and hold
    Enter.
- Credit Early (the safe combination) uses the same save-and-reload method a set number of frames
  before entering the store.
- The C++ seed finder:
  - Options: `--fast-middle-rooms --all-seeds`, `--only SEED` and
    `--max-frames-from-store N --credit-early-frames K`.
  - It ships seeds with "at least 4 frames of credit early and the fastest possible catacombs".
  - Its source returned HTTP 403, so how it models the draws is unconfirmed.
- Results:
  - Seed 2756848129 works with or without CD's extra exit draw, with a save window 26-29 frames
    before the store.
  - The 21:52 record (2026-08-23, DOS CD, ScummVM 2026.3.0) used that seed, and its run comment still
    reports "inconsistent catacombs RNG".
  - In the record the catacombs take 50 s (`docs/research/anypercent-runner-sources.md`).
- Limits stated by the guide:
  - On CD the game sometimes makes 6 RNG calls instead of 5, and then "chances are the run is over".
  - Two wrong exits make the draws during walking inconsistent, for a "reason ... unknown".
  - Fester makes 0, 1 or 2 random calls.
- Inference: the launcher reload makes each event a short, predictable stretch of the seed's stream,
  not an exactly known one.

**PokéFinder** (Admiral_Fish et al., 2017 to present) `[POKEFINDER]` verified,
https://github.com/Admiral-Fish/PokeFinder

- Strategy: it reimplements each game's RNG and searches three ways.
  1. Target first (generations 3 and 4). It loops over the IV combinations the user wants, recovers
     at most 6 origin states per combination (12 for one method) with an attack credited to lattice
     reduction, then walks the reverse generator between a minimum and maximum advance. It outputs
     (initial seed, advance) pairs that fall within a delay window.
  2. Forward (generation 5). It scans days, the calibrated Timer0 range, the allowed key
     combinations, and every second of the day, hashing four seconds per SIMD batch. Some searchers
     use a precomputed table built by enumerating all 2^32 Mersenne Twister seeds and keeping
     near-perfect IV spreads.
  3. GF(2) jump polynomials to skip ahead in large generators.
- `ProfileSearcher5` calibrates hardware nuisance parameters from observed results.
- No benchmarks are published.
- What it means for us:
  - The answer to a seed search is a (seed, advance window) pair, not a seed.
  - Searching back from the target is possible when the generator is invertible.
  - A 2^32 table of good states is routine.
  - Nuisance parameters get pinned; for us that means the release, the music driver and the text
    speed.

**Project_Xs** (niart120, 2021-2022) `[PROJXS]` verified, https://github.com/niart120/Project_Xs

- Strategy: it recovers a 128-bit xorshift state from webcam-observed blinks in a Pokémon game,
  inferred to be Brilliant Diamond / Shining Pearl.
  - Each player blink gives 4 linear equations over GF(2).
  - 39 blinks form a 156x128 system, solved by Gauss-Jordan elimination.
  - It then re-identifies its position by scanning forward, allowing a stride for other consumers of
    draws (NPCs) and a Hamming-distance tolerance.
- The README calls it experimental, and the documented entry script is broken at HEAD.
- What it means for us: problems over xorshift-family generators are linear algebra, not search.

**Local analysis.** This came from the research pass; the PokéFinder fact-check re-checked the rank
computation.

- ScummVM's RandomSource is a 32-bit xorshift (>>13, <<21, >>11). Each draw returns
  `(state * 0xDEADBF03) % (max + 1)` (`third_party/scummvm/common/random.cpp`).
- The multiplier is odd and congruent to 3 mod 4, so mod-2 and mod-4 outputs are linear in the seed
  bits over GF(2). Mod-3, mod-5 and mod-8 outputs are not.
- The safe combination is four consecutive `getRandomNr(3)` draws
  (`data/scripts-dos-cd/room-030-store/local-205.txt [0005]`), and 1-1-1-1 means all four draws are 0.
  - At a fixed draw index n those constraints have rank 8, so 2^24 seeds give 1-1-1-1.
  - Requiring it at every index n to n+3 (seven draws) gives rank 14, so 2^18 seeds.
- The catch: n depends on the seed itself, through Fester's random calls and the catacombs' rejection
  loops. So a closed-form seed set still needs a simulation for each seed, or a check in the engine.

Unverified leads in this family:

- TASVideos' luck-manipulation pages (https://tasvideos.org/LuckManipulation,
  https://tasvideos.org/AdvancedLuckManipulation).
- FF12RNGHelper (https://github.com/Tranquilite0/FF12RNGHelper): "burn" actions with a known draw cost.
- php_mt_seed (https://www.openwall.com/php_mt_seed/): all 2^32 seeds in under a minute on four
  cores.
- Marsaglia's xorshift paper (https://www.jstatsoft.org/article/view/v008i14).
- LattiCG (https://github.com/mjtb49/LattiCG).
- Yurichev's Z3 attack on xorshift (https://yurichev.com/blog/xorshift/).
- Haramoto et al.'s jump-ahead paper (https://www.gerad.ca/en/papers/G-2006-62), abstract only.
- A search found no academic paper on speedrun or TAS RNG manipulation; the practice lives in
  community tools.

### 3.8 Same game and same engine

`related-work.md` §2 surveys this work; this section only judges the strategies. Each bracketed tag
is that document's reference ID and status (D: fetched directly, C: checked by a verification pass,
U: unverified).

**LLM agents through engine hooks: ScummBench, scummvm-mcp, monkey-island-webmcp and Luis HG's
screenshot loop** (2025-2026; `related-work.md` §2.1)

- Strategy: an LLM chooses each next action online. ScummBench, scummvm-mcp and webmcp show it
  symbolic SCUMM state; Luis HG's loop shows it screenshots with a numbered grid. scummvm-mcp sends
  actions through `doSentence` and `runInputScript(kVerbClickArea, verb, 1)`, the same two entry
  points our bridge uses `[MCP-SRC, C]`.
- Results: none reports a completion, a speedrun or a timed run `[AI-YT, C]`.
  - ScummBench's demonstration video shows Claude Opus 4.6 stuck at the troll bridge in Part I.
  - The "AI Adventure XP" videos say the agents "had a hard time" with the MI1 demo.
  - scummvm-mcp's README lists "Work reliably outside MI1" under what it cannot do
    `[MCP-README, C]`.
- Better than ours at: nothing has to be modelled before play.
- Worse than ours at: there is no plan, no time objective and no completion. Inference: on MI1 they
  repeat what the benchmarks in §3.3 found for other adventures. Online agents are far from finishing
  the game, let alone finishing it fast.

**ScummGameGraph** (mnuhn, a README-only proposal, 2026-06-28; `related-work.md` §2.1) `[SGG, U]`

- Strategy: decompile the SCUMM scripts with scummvm-tools, build a graph of global-variable reads
  and writes, and run Dijkstra or A* for "a mathematically optimized speedrun route" through MI1 and
  MI2.
- Results: none. It has no code.
- Comparison: it is our macro idea, proposed independently. It lacks what made ours work on Part I:
  costs measured in the engine and keyed by position, a state that includes where Guybrush stands,
  and replay. Inference: a graph of variable dependencies alone carries no walking time, and Part I's
  last improvement (path 686) was a walk.

**Hand-made libTAS TASes of LucasArts games** (mikeSpeedyAdventures, 2023-2024; `related-work.md`
§2.2) `[TW-MI2, D]` `[RENTRY, U]`

- Strategy: frame-by-frame input in libTAS on ScummVM with a fixed `--random-seed`. Monkey Island 2
  (German release) runs 29:47, and Day of the Tentacle 14:04.91. A later verification pass could not
  find the Day of the Tentacle video again `[YT-SEARCH, C]`. Neither is on TASVideos.
- His MI1 TAS is mentioned in a July 2025 forum thread, but no video or page could be found
  `[SRC-F-SAFE, D]`.
- Better than ours at: it uses the toolchain TASVideos accepts (libTAS on an official build) and
  finishes whole games.
- Worse than ours at: the search is done by hand, with no planner and no bot.
- Inference: no MI1 TAS is public, so "any possible TAS" has no published reference point. The human
  RTA record (21:52) is the only measured target.

**The rejected Loom TAS** (darkshoxx, TASVideos #9181, 2024; `related-work.md` §2.2) `[TV-LOOM, C]`

- It used libTAS on ScummVM 2.8.1 with `--random-seed=0` and ran at 1000 fps with Ctrl+G.
- The judge rejected it for "Emulation", calling Ctrl+G "unthrottled emulation speed".
- What it means for us: the only LucasArts SCUMM submission failed on a speed feature, not on its
  route. `rules/anypercent.md` bans the same feature.

**AutoHotkey-scripted runs** (ATK42 Games, The Curse of Monkey Island, 2023; `related-work.md` §2.2)
`[YT-TAS, C]`. These send timed input in wall-clock milliseconds, the opposite of our design:
scripted playback with no frame control.

What this family says about us: on MI1 and SCUMM, nothing combines a plan with measured time. The
agents share our injection points but not our planning. The TASes share our goal but not our search.

### 3.9 Monte Carlo and learned search

**AlphaZero, MuZero and Agent57** (Silver et al., arXiv 1712.01815, 2017; Schrittwieser et al.,
arXiv 1911.08265, Nature 2020; Badia et al., arXiv 2003.13350, 2020) `[ALPHAZERO]` `[MUZERO]`
`[AGENT57]` direct (abstracts), https://arxiv.org/abs/1712.01815, https://arxiv.org/abs/1911.08265
and https://arxiv.org/abs/2003.13350

- Strategy:
  - AlphaZero runs Monte Carlo tree search guided by a policy and value network trained by
    self-play, "given no domain knowledge except the game rules".
  - MuZero replaces the rules with a learned model that predicts "the reward, the action-selection
    policy, and the value function". Its motivation is that tree search has succeeded "where a
    perfect simulator is available", while real dynamics are "often complex and unknown".
  - Agent57 is model-free. One network covers policies "ranging from very exploratory to purely
    exploitative", and an adaptive mechanism chooses among them.
- Results:
  - AlphaZero reached superhuman chess and shogi within 24 hours of self-play.
  - MuZero set a new state of the art on 57 Atari games, and matched AlphaZero in Go, chess and shogi
    without being given the rules.
  - Agent57 is "the first deep RL agent that outperforms the standard human benchmark on all 57
    Atari games".
- They optimise winning or game score. None minimises completion time, and no application to a
  speedrun was found.
- What it means for us:
  - MuZero exists because there is no simulator. We have the exact engine and can fork it, which is
    AlphaZero's situation, not MuZero's.
  - The idea from AlphaZero that transfers is a learned value at the leaves of a search. The "one
    forked lookahead" in recommendation 3 is a hand-made stand-in for that value. Inference: training
    one needs many runs of a route that has not yet been executed once, so it is not a first step.

**Nested Monte Carlo search (NMCS) and nested rollout policy adaptation (NRPA)** (Cazenave, IJCAI
2009; Rosin, IJCAI 2011) `[NRPA]` direct,
https://cs.uwaterloo.ca/~jhoey/rgwiki/uploads/11IJCAI-NRPA-Rosin.pdf (NMCS:
https://www.lamsade.dauphine.fr/~cazenave/papers/nested.pdf, of which only a two-page copy was read)

- Strategy: both target deterministic single-player optimisation.
  - NMCS plays one game. At each step it tries every move, runs a nested search one level lower
    after each, and plays the best. Level 0 is a random playout.
  - It memorises the best sequence found so far and follows it whenever the randomised searches do
    worse.
  - NRPA keeps the nesting, but at each level it adapts the rollout policy towards the best
    sequence by gradient ascent.
- Results (Rosin):
  - On Morpion Solitaire, NMCS had set the record of 80 for the disjoint version. NRPA reached 82
    there, and 177 in the touching version. In the touching version a 1976 human record of 170 had
    stood until NRPA's 172 in 2010.
  - NRPA also beat the contest-winning solutions on two crossword-construction instances.
  - A run took about 1 hour (disjoint Morpion) to 1 week (touching Morpion).
- What it means for us:
  - This family is built for our micro problem: a deterministic single-player game, an exact
    simulator and one score. It needs no hand-written ranking, while our planned beam ranks by time
    to the milestone.
  - It needs many rollouts. For us a rollout is a forked engine run to the next milestone, and our
    branches per second are not measured (§3.4), so its cost is unknown.
  - Inference: a policy over (click-frame offset, click point) for each move is a natural NRPA
    policy. Nobody has tried it on a speedrun.

**Seify and Buro, "Single-Agent Optimization Through Policy Iteration Using Monte-Carlo Tree
Search"** (RL in Games workshop, AAAI 2020) `[SEIFY]` direct (abstract only),
https://arxiv.org/abs/2005.11335. An AlphaZero-style search for the single-player game SameGame, with
a self-play policy network and a value normalisation for unbounded scores. The abstract says it beats
baseline algorithms on several board sizes and is "competitive with" state-of-the-art search on a
public position set. It does not claim to beat nested search.

Unverified lead: **Single-Player Monte-Carlo Tree Search** (Schadd et al., Computers and Games 2008),
an MCTS variant for SameGame. Not fetched.

What this family says about us: learned search is the wrong tool at the macro level, where we have an
exact symbolic model and A* gives optimality relative to it. At the micro level, nested Monte Carlo
search is a published alternative to a hand-ranked beam for exactly our kind of problem. Whether it
is affordable depends on throughput we have not measured.

### 3.10 Exact combinatorial optimisation

**Libralesso, Bouhassoun, Cambazard and Jost, "Tree search algorithms for the Sequential Ordering
Problem"** (arXiv 1911.12427, 2019; ECAI 2020) `[SOP]` direct, https://arxiv.org/abs/1911.12427

- Problem: the sequential ordering problem (SOP), an asymmetric TSP with precedence constraints. Find
  an order of the vertices that respects the precedences and minimises the summed arc weights.
- Exact methods the paper reviews: cutting planes and branch and cut, Lagrangian relaxation,
  decision diagrams for bounds, and branch and bound with "History Cuts" taken from TSP dynamic
  programming.
- Its own method is an iterative beam search with a dominance cut. When two partial orders cover the
  same set of vertices and end at the same vertex, the costlier one is cut.
- Results: it "proves optimality on half of the SOPLIB instances" (200 to 700 vertices), and finds new
  best solutions for 6 of the 7 open ones.
- What it means for us:
  - Inference: Part I's puzzle order is close to an SOP. Actions are vertices, script dependencies
    are precedences, and position-keyed costs are arc weights from wherever the previous action left
    Guybrush. The dominance cut is what A*'s duplicate detection already does on our state (facts
    made true, position).
  - Better than ours at: SOP solvers return a lower bound and a gap for the SOP itself, at a few
    hundred vertices. Our Part I search is optimal relative to the model, but the Any% route has not
    been planned at all, so whether Fast Downward scales to it is unmeasured.
  - Worse than ours at: SOP has nothing but precedence. Money, inventory, alternative solutions,
    optional detours and actions that undo state need hand compilation; PDDL expresses them directly.
  - Use: if the Any% macro search does not finish, an SOP model (keep only precedence and
    position) gives an estimate to compare the current route with. It is a lower bound on our
    model's optimum only if every vertex is an action that all routes must take, every precedence
    holds in every route, and the arc costs underestimate. Any% fails all three as it stands: it has
    alternative solutions and optional detours, routes revisit rooms while an SOP visits each vertex
    once, and the cost table holds means and non-admissible defaults (§4.3). The solvers' bounds are
    for the SOP and do not carry over without those conditions.

**Bofill et al., "A Good Snowman is Hard to Plan"** (arXiv 2310.01471, 2023) `[SNOWMAN]` direct,
https://arxiv.org/abs/2310.01471

- Problem: the commercial puzzle game A Good Snowman Is Hard to Build, modelled in PDDL and encoded
  directly in SAT. The aim is certified optimal solutions, so that a designer can check that no much
  easier solution exists.
- Results: 51 levels (30 from the game, 9 hidden in its files, 12 crafted), with a 1-hour limit.
  - The SAT encodings with reachability solved 43, to plan-length optimality.
  - The best PDDL configuration, SymK on the basic model, solved 29; Fast Downward with the blind
    heuristic 27, and LAMA 14 to 16 on the sound models (20 on a model with unsound teleports, of
    which 11 solutions were valid). Fast Downward's Stone Soup portfolio "always resorted to the
    blind heuristic", so it was not reported.
  - Expressing reachability with PDDL axioms did not reduce solving time significantly.
- Optimality here is plan length, or the number of ball moves, not time.
- What it means for us: on a real puzzle game, a solver-specific encoding beat generic planners
  widely, mainly because grid walking was expensive as PDDL actions. Inference: our model avoids that
  cost the same way their reachability encoding does, by making each walk one action whose cost the
  engine measures (the engine does the pathfinding). Time-weighted costs, which we need, would put an
  optimisation layer on top of SAT; the paper does not test that.

What this family says about us: exact solvers give certificates that our planner gives only relative
to a model it has not yet been run on for Any%. They are a fallback and a comparison point, not a
replacement and not a source of bounds on our problem, because the game's state does not reduce to
precedence.

### 3.11 LLMs that write a planning model

`related-work.md` §3.3 lists this work as prior art for the extractor agents. This section compares
strategies.

**Guan, Valmeekam, Sreedharan and Kambhampati, "Leveraging Pre-trained Large Language Models to
Construct and Utilize World Models for Model-based Task Planning"** (NeurIPS 2023) `[P-GUAN]`
direct, https://arxiv.org/abs/2305.14909

- Strategy:
  - GPT-4 writes a PDDL domain from natural-language action descriptions, with two Blocksworld
    actions as examples.
  - PDDL validators catch syntax errors. Graduate students with PDDL expertise annotate the
    factual errors, and GPT-4 repairs the model from that feedback.
  - A sound planner then plans on the corrected model.
- Results (experiments done in May 2023):
  - GPT-4's models had 59 errors over three domains: 53 in Household (22 actions), 2 in Logistics and
    4 in Tyreworld. Three were syntax errors. The rest were factual, "such as missing preconditions
    and effects".
  - GPT-3.5-Turbo made over 350 errors.
  - GPT-4 needed 59 feedback messages to fix 56 factual errors. The corrected models solved 48
    planning tasks.
- What it means for us:
  - Their typical error, a missing precondition or effect, is the kind our three extractor defects
    and the SAM check (recommendation 6) concern.
  - Their verifier is a human expert. Ours is the decompiled script that each action cites, plus
    replay in the engine. Inference: a cited script line makes each fact checkable by a program,
    which a natural-language source does not allow.
  - Our evidence is not an error count. On Part I the extracted model matched the hand model on 97
    action signatures, with 9 only in the hand model and 33 only in the extracted one
    (`docs/extraction-diff.md`). Either side can be wrong there: path 686 was extracted-only and
    correct.

**Liu et al., "LLM+P: Empowering Large Language Models with Optimal Planning Proficiency"** (arXiv
2304.11477, 2023) `[LLMP]` direct, https://arxiv.org/abs/2304.11477

- Strategy: an LLM turns a natural-language task into a PDDL problem file, Fast Downward (an optimal
  portfolio, or LAMA) solves it, and the LLM translates the plan back.
- It assumes "a human expert can provide a domain description", so the LLM writes only the problem
  file.
- Result: LLM+P gives "optimal solutions for most problems, while LLMs fail to provide even feasible
  plans for most problems".
- What it means for us: ours is the harder case, because the domain is what has to be extracted.
  LLM+P shows that the planner half is not where the risk lies.

**Kambhampati et al., "LLMs Can't Plan, But Can Help Planning in LLM-Modulo Frameworks"** (ICML 2024,
position paper) `[LLMMODULO]` direct (abstract only), https://arxiv.org/abs/2402.01817

- Argument: auto-regressive LLMs "cannot, by themselves, do planning or self-verification". They
  should propose candidates for "external model-based verifiers" to check, and they can help acquire
  the models those verifiers use.
- What it means for us: our pipeline has this shape. Extractors propose a model; the planner, the
  diff against the hand model and the engine replay check it. The paper is a position, not evidence.

TheoryCoder (§3.2, unverified) automates the repair step from observed transitions. We repair by
hand.

What this family says about us: an LLM writing the model and a sound planner solving it is a
recognised pattern, and its known weakness is missing preconditions and effects. Our variant differs
in its source (bytecode, not prose) and its verifier (the engine). One segment supports "feasible",
not "reliable".

## 4. Our strategy, judged against them

### 4.1 What exists and what is only planned

- **Shipped, glitchless Part I** (Mac release, SCUMM v5).
  - A hand-written PDDL model whose actions cite their scripts.
  - Per-action measurement on seeds, and Fast Downward `astar(lmcut())` over means keyed by position.
  - An optimistic loop on seeds 1-5, then 43 candidate sources measured on seeds 1-30, and a paired
    report on held-out seeds 31-60.
  - Result: 67 actions, 22,292 ticks (`README.md`, `docs/optimization.md`).
- **Built for Any%.**
  - Bridge v3: raw input on exact frames, a per-frame log, an RTA clock in quarter-frames, and a
    control socket that forks the engine. A fork is an exact copy, RNG included
    (`src/speedrun/control.py`).
  - Releases `dos-vga` and `dos-cd`. CD music now advances in game time, so a fork made while a
    track plays matches the straight replay (`docs/anypercent/findings.md`;
    `tests/integration/test_control.py`).
  - The executor issues each move on the first frame the engine's input gate opens, at a click point
    the engine's own hit test confirms, and raises `RouteError` on failure (`src/speedrun/executor.py`).
- **Drafted, not yet executed.** The Any% route: six segment files in `routes/anypercent/` (S1 to
  S6) with 465 moves. The counts here are as of 2026-10-19; the files are uncommitted and still
  changing.
  - 22 moves carry a `glitch` field that cites the script lines of the mechanism: the logo glitch,
    Credit Early, the item slides and the moves that use them.
  - The files also list 34 RNG points and the crash rules.
  - Their notes cite runner sources and the scripts. No planner produced them, and nothing in
    `src/` or `tests/` loads them yet.
- **Planned only.**
  - The beam search over click points and frames.
  - The Any% macro search.
  - The seed search.
  - The bridge records the configured seed but has no hook on RNG draws.
- **Version.** The Any% plan's "Where things stand" section still says the floppy is the working
  release. The CD findings since then make DOS CD, the record's release, usable. Seeds are specific
  to the version and the music driver.

### 4.2 What is principled, and who backs it

- **Factored state with costs keyed by position, searched by A* with LM-cut** (glitchless Part I).
  This answers Groß et al.'s objection that static weights need an exploding state graph. It is
  cost-optimal relative to the model and the cost table, which is the scope Churchill and Buro also
  claim. A* with time as the cost over an exact forward model also won the 2009 Mario AI competition
  (§3.4).
- **Measure, then re-plan.** This is a recognised pattern (Khandelwal et al., LazySP, PDDLStream), so
  its shape is sound. Its stopping rule is not (§4.3).
- **Earliest-frame issue on the engine's own input gate.** This is the same idea as gb-tas-gen's
  `runToNextInputFrame` and JaffarPlus's gated input sets. It is optimal wherever waiting never
  helps.
- **Exact forks on the real engine, RNG included.** Go-Explore, Jaffar and gb-tas-gen depend on this
  primitive. We have it on the real engine, not on a reimplementation.
- **Selection on seeds 1-30, report on held-out seeds 31-60, paired** (glitchless Part I, whose
  rules forbid choosing a route for a seed). No work in this survey validates more carefully. It is
  standard practice, though, and not a contribution (`related-work.md` §4.2). Under Any% the seed is
  chosen and the run is deterministic, so a candidate is one run, as in a TAS. Held-out seeds there
  would answer a different question (how a route does on seeds it was not built for), not a harder
  one.
- **A model taken from code, with citations.** Lester and Verbrugge and Zhang also derive from code.
  Ours feeds a cost-optimal planner.
- **An LLM proposes the model and sound tools check it.** This is the LLM-Modulo pattern, and Guan et
  al. show the same shape with a human expert as the checker (§3.11). Our checkers are the cited
  script, the diff against the hand model and the engine.

### 4.3 What is ad hoc, and which work says so

1. **The default cost for an unmeasured (action, position) pair is the action's minimum measured
   duration.** It is not known to be admissible: nothing stops an unmeasured context from costing
   less than every measured one. So the stop rule ("only measured actions, and the plan repeats")
   carries no guarantee (LazySP Theorem 2; PDDLStream's cost 0). This is the "short-lived
   exploration" flaw Khandelwal et al. report. It matters because context changes costs a lot:
   `walk-into-bar` costs 6 to 396 ticks depending on context (`docs/optimization.md`).
2. **The position token is a hand-chosen relevance assumption.** 11 actions break it, each with a
   between-context share of 0.75 to 1.00, and spreads up to 390 ticks. SAHTN's optimality theorem
   assumes relevance is right. `docs/next.md` already lists finer keys.
3. **Candidates come from random perturbation.** 21 of the 43 sources produced the same plan, and
   there were 23 distinct candidates in all (`docs/optimization.md`). Top-k planning is already in
   `docs/next.md` (SymK). Diverse planning that forbids failing prefixes (Ortiz-Haro et al.,
   unverified) is the systematic version.
4. **The planned micro beam ranks only by time to the milestone, with no deduplication.** That is
   recursive optimality at best (MAXQ). Every bot here deduplicates: Jaffar by a property hash,
   LunarBot by an outcome hash, gb-tas-gen by (step, RNG state), Go-Explore by cell.
5. **Earliest-frame issue everywhere, including before random events.** A delay can buy a better
   draw. gb-tas-gen tries 0, 1, 2 and more delay frames per state. Churchill and Buro's argument for
   earliest issue assumes that waiting never helps.
6. **No feedback from execution to the model.** `RouteError` aborts the run. Srivastava et al. and
   Ortiz-Haro et al. (both unverified) turn a failure into a fact or a forbidden prefix and re-plan.
7. **Glitches come only from runner sources.** Nothing searches for them. Go-Explore's
   treasure-room bug and IJON's (unverified) wall-jumps were found by search.
8. **Model completeness is checked only by replay and by a second extraction.** That is how path
   686 was found. We have no coverage measure (ScAmPER) and no refutation check against logged
   transitions (SAM).
9. **The Any% route is not produced by the planner.** Its moves are written from runner sources and
   the scripts, which is the TAS bots' "pre-determined route" (Jaffar), not our Part I method. On
   Part I the extracted model found an exit the hand-written model lacked (path 686), so a route
   written this way can miss things a planned one would find.

### 4.4 What is missing

- **A framework for how the two levels interact.** TAMP supplies one:
  - The macro level should see admissible costs (LazySP, PDDLStream).
  - The interface between levels should carry end states, not single numbers (SAHTN, MAXQ).
  - Failures should flow back up (Srivastava et al.).
  - Claims should be scoped as "hierarchically optimal under the move hierarchy, the model and the
    measured costs" (AHA*), not "optimal".
- **Systematic state deduplication in the micro search.** A hash over room, ego position, walk
  target and facing, running scripts and their offsets, the route-relevant variables and bits,
  inventory, and the RNG state. Jaffar's warning applies: a hash that is too coarse loses solutions.
- **Automatic discovery of glitches and sequence breaks.** Path 686 teaches that the search must
  record time per alternative, not only state changes. A Jericho-style diff sweep would have merged
  686 with 685.
- **RNG handling.**
  - The bridge has no draw log and seed search is not written.
  - The plan assumes draw indices shift only with input timing; they also depend on the seed.
  - A fork inherits the RNG state, so trying another seed means replaying from boot or from a reseed
    point.
  - saruya's launcher reload is such a reseed point. `rules/anypercent.md` allows in-run saves and
    loads but says nothing about returning to the launcher.
- **A planned Any% route.** The Part I pipeline has not been run on the full game, so neither its
  result nor whether Fast Downward scales to it is known. If it does not scale, an SOP model gives
  an estimate to compare the current route with; it is a bound only under conditions Any% does not
  meet (§3.10).
- **A throughput figure** (branches per second), needed to size any micro search and to decide
  between a hand-ranked beam and nested Monte Carlo search (§3.9).
- **A replay on the accepted toolchain.** TASVideos accepts ScummVM runs through libTAS on official
  builds (`related-work.md` §2.2). Our bridge is patched.

### 4.5 Novelty, without overclaiming

What is not new:

- The two-level split. Jaffar searches "within a pre-determined route".
- The measure-and-replan loop with location-keyed costs (Khandelwal et al. 2014).
- Forks used as savestates. Every TAS tool has savestates.
- An MI1 seed finder (saruya).
- Routing MI1 by shortest path over its decompiled script dependencies, proposed without code by
  ScummGameGraph (§3.8).
- An LLM writing the PDDL model and a sound planner solving it (Guan et al., LLM+P; §3.11).
- A* with time as the cost over an exact copy of a game's engine (the Mario AI agent, 2009; §3.4).

What remains plausibly new is what `related-work.md` §4.2 already says, and nothing more:
cost-optimal classical planning over measured, position-keyed engine ticks, from a bytecode-cited
model, for a commercial graphic adventure. That rests on negative searches, not proof. It holds for
glitchless Part I only; the Any% route is not produced by the planner so far.

## 5. Recommendations, ranked

The ranking puts the expected effect on run time first and effort second. Time estimates come only
from local measurements; where none exists, the effect is "unknown". Changes that mostly save search
effort or add rigour say so.

The baseline is the 21:52 human record (DOS CD, saruya). Seeded catacombs, Credit Early and the CD
item slides are already in that run, so for us they are what it takes to match the record, not gains
beyond it. The effect column separates the two.

Already in progress, and not from this research: run Any% on DOS CD with the item slides. The CD
record beats the same runner's EGA run by 68 s, mostly in Part III
(`docs/research/anypercent-runner-sources.md`), and the `dos-cd` release now runs fork-safe
(`docs/anypercent/findings.md`). The Any% macro search is also planned (§4.1). The route in
`routes/anypercent/`, which no planner produced, is what it has to beat. If Fast Downward does not
scale to the full game, an SOP model (§3.10) gives a comparison estimate, not a bound.

| # | Change | Comes from | Expected effect on run time | Effort |
|---|---|---|---|---|
| 1 | **RNG as a first-class search dimension.** (a) Hook `getRandomNumber` in the bridge and log the RNG state and a cumulative draw count every frame. (b) In the executor, carry a capped buffer of branches keyed by (qf, RNG state) from one random event to the next, try 0..k delay frames before each event, and pick the branch late. (c) Score a seed by every random event it makes good at once, and prefer seeds whose good outcomes hold over a range of arrival frames. A human needs that width to hit the frame; a bot hits exact frames, but needs the width so that a later change upstream in the route does not force a new seed search (PokéFinder's delay windows). (d) Reproduce saruya's published CD seed 2756848129 as a regression test, and use his finder as the baseline to beat. (e) Decide with the rules whether the launcher reload is a legal route move. | gb-tas-gen, saruya, PokéFinder, Project_Xs | To match the record: seeded catacombs (50 s in the record; no source gives an unseeded time, so the gain over unseeded play is unmeasured) and Credit Early. Beyond the record: unknown. A joint score over every random event (insult fights, storekeeper, Stan) is something saruya's two-event finder does not do, and the record lost about 10 s to one missed Carla insult (`docs/research/anypercent-runner-sources.md`). | Medium. The hook is small; the buffer and the per-event models are the work. |
| 2 | **Search for glitches and sequence breaks.** (a) A per-object timing sweep: for every exit or object that leads to the same transition, measure the walk from each entry. This generalises path 686. (b) Targeted fork search at known slide sites (oars, monkey, feather) and at Credit Early, varying the click frame and point. (c) Later, a Go-Explore archive over forks, seeded along the current movie: cells built from SCUMM state (room, coarse ego position, inventory, progress bits), each keeping its fastest prefix in qf, with novelty measured by new script offsets executed. Turn every find into a PDDL action or route move and re-plan. Do not ship archive trajectories directly. | Go-Explore, Reveal-More, ScAmPER; IJON, TesterTron, CA2 (unverified) | To match the record: executing the CD item slides, which put the CD record 68 s ahead of the same runner's EGA run. Beyond the record: unknown. Path 686 alone saved 108 ticks in glitchless Part I. | (a) Low. (b) Medium. (c) Large. |
| 3 | **A micro beam that keeps end states.** Hash the relevant engine state and drop duplicate branches at equal or higher qf. At each milestone, keep the fastest branch for each distinct end state (ego position and facing, walk phase, RNG index), and score it by its qf plus one forked lookahead. Measure branches per second first, to size the beam. Once throughput is known, compare the beam with nested Monte Carlo search (NRPA), which needs no hand-written ranking. | Jaffar, LunarBot, gb-tas-gen; SAHTN, MAXQ; NRPA | Beyond the record, at milestone boundaries: end states can matter by hundreds of ticks (`walk dock lookout` spreads 366 ticks by context). Otherwise mostly search effort. | Medium |
| 4 | **A splice and polish pass over the finished movie.** Splice the fastest per-milestone stretches from different candidate movies, then re-verify end to end. Then perturb single clicks by ±k frames or ±n pixels, fork just before the change, and keep improvements. Then try deleting optional inputs. | SpeedrunBench; TMInterface, bxt-rs, learnfun (unverified) | Beyond the record, small: frames to a few seconds. SpeedrunBench's merge saved 26 frames on near-optimal input. | Low to medium |
| 5 | **Make the macro loop's guarantee real.** Cost an unmeasured (action, position) pair at an admissible floor: 0, or for walks the distance over the maximum walk speed from the walk-step code (`docs/research/anypercent-engine-mechanics.md`, including re-click effects if the category allows them). Key the 11 flagged actions by the previous action as well. Keep top-k planning as in `docs/next.md`. | LazySP, PDDLStream, Khandelwal et al.; SAHTN | Rigour first. It may find a faster context that the current default hides; size unknown. It costs more loop iterations, at about 43 s per Fast Downward run. | Low |
| 6 | **Close the loop from execution to the model.** After each move, compare the observed state with the atoms the PDDL plan predicts, and report the first mismatch. Turn executor failures into facts or forbidden prefixes and re-plan. Run a SAM-style refutation of extractor preconditions over logged transitions; conditional effects make a flag a lead, not a proof. | SAM; Silver et al., Srivastava et al., Ortiz-Haro et al., TheoryCoder (unverified) | Indirect: fewer silent model errors, fewer missed shortcuts | Medium |
| 7 | **Replay early on official ScummVM under libTAS.** Convert the JSONL movie to libTAS input, and check whether behaviour that exists only in the bridge (CD music timer, music decoded from memory) changes timing. | JaffarPlus (#8237's manual desync fixes; #8784's week of resync); TIM (#10017's preset cancellation) | None on time. It removes the risk that a fast movie is not acceptable or does not sync. | Medium |
| 8 | **Scope every claim.** Write "optimal within the PDDL model and the measured cost table, validated on held-out seeds", never "optimal". | Churchill and Buro, AHA*; Dragster (unverified) | None | Trivial |

## 6. References

Status: verified = fact-checked against the primary source, with corrections applied in this
document. direct = primary source fetched by the writer of the critical revision, with no separate
fact-check pass. unverified = fetched by a research pass, not fact-checked. No work was unconfirmed
as a whole; the specific claims marked "unconfirmed" in the text are listed in the Notes column. IDs
that start with `P-`, and `ICARUS`, are shared with `docs/related-work.md`. §3.8 also cites
`related-work.md` IDs (`MCP-SRC`, `MCP-README`, `AI-YT`, `SGG`, `TW-MI2`, `RENTRY`, `YT-SEARCH`,
`SRC-F-SAFE`, `TV-LOOM`, `YT-TAS`) with that document's D/C/U status; they are listed there, not
here.

| ID | Citation | URL | Status | Notes |
|---|---|---|---|---|
| P-LESTER | M. M. Lester, "Solving Interactive Fiction Games via Partial Evaluation and Bounded Model Checking", arXiv 2012.15365, 2020 | https://arxiv.org/abs/2012.15365 | verified |  |
| SCAMPER | M. M. Lester, "ScAmPER: Generating Test Suites to Maximise Code Coverage in Interactive Fiction Games", TAP 2020, LNCS, pp. 169-179 | https://centaur.reading.ac.uk/90238/ | verified |  |
| P-VERBRUGGE | C. Verbrugge and P. Zhang, "Analyzing Computer Game Narratives", ICEC 2010, LNCS 6243, pp. 224-231; and "Dataflow Analysis of Computer Game Narratives", CDP'08 talk | https://www.sable.mcgill.ca/~clump/papers/verbrugge-10-analyzing.pdf ; https://www.eecg.utoronto.ca/~steffan/workshops/08/cdp/verbrugge.pdf | verified |  |
| P-PICKETT | Pickett, Verbrugge and Martineau, "(P)NFG", GameOn'NA 2005 | https://www.sable.mcgill.ca/~clump/papers/pickett-05-pnfg.pdf | verified | Checked in related-work.md §3.1, not in this pass |
| VOLOKH | S. Volokh and W. G. J. Halfond, "Automatically Defining Game Action Spaces for Exploration Using Program Analysis", AIIDE 2023 | https://ojs.aaai.org/index.php/AIIDE/article/view/27510 | unverified |  |
| IJON | C. Aschermann et al., "IJON: Exploring Deep State Spaces via Fuzzing", IEEE S&P 2020 | https://www.casa.rub.de/forschung/publikationen/detail/ijon-exploring-deep-state-spaces-via-fuzzing | unverified |  |
| AIJON | J. M. Vadayath et al., "AIJon: Automated Generation of Annotations for Fuzzing", arXiv 2609.18457, 2026 | https://arxiv.org/abs/2609.18457 | unverified |  |
| SAM | A. Mordoch, S. S. Shperberg, R. Stern and B. Juba, "Learning Safe Numeric Planning Action Models", arXiv 2312.10705 v2, 2025 (builds on SAM, Juba, Le and Stern, KR 2021) | https://arxiv.org/abs/2312.10705 | verified |  |
| P-BALYO | T. Balyo et al., "On Automating Video Game Regression Testing by Planning and Learning", arXiv 2402.12393, 2024 | https://arxiv.org/abs/2402.12393 | unverified | Not checked in this pass; related-work.md §3.2 checked part of it |
| THEORYCODER | Z. Ahmed, J. B. Tenenbaum, C. J. Bates and S. J. Gershman, "Synthesizing world models for bilevel planning", TMLR 2025 | https://arxiv.org/abs/2503.20124 | unverified |  |
| BLACKOUT | A. Krishnan, A. Williams and C. Martens, "Towards Action Model Learning for Player Modeling", AIIDE 2020 | https://arxiv.org/abs/2103.05682 | unverified |  |
| SILVER | T. Silver et al., "Predicate Invention for Bilevel Planning", AAAI 2023 | https://arxiv.org/abs/2203.09634 | unverified |  |
| P-GOEXPLORE | A. Ecoffet, J. Huizinga, J. Lehman, K. O. Stanley and J. Clune, "Go-Explore: a New Approach for Hard-Exploration Problems", arXiv 1901.10995, 2019; "First return, then explore", Nature 590:580-586, 2021 | https://arxiv.org/abs/1901.10995 ; https://arxiv.org/abs/2004.12919 | verified |  |
| P-JERICHO | M. Hausknecht, P. Ammanabrolu, M.-A. Côté and X. Yuan, "Interactive Fiction Games: A Colossal Adventure", AAAI 2020, pp. 7903-7910 | https://arxiv.org/abs/1909.05398 | verified |  |
| REVEALMORE | K. Chang, B. Aytemiz and A. M. Smith, "Reveal-More: Amplifying Human Effort in Quality Assurance Testing Using Automated Exploration", IEEE CoG 2019 | https://adamsmith.as/papers/PID5953685.pdf | verified |  |
| ICARUS | J. Pfau, J. D. Smeddinck and R. Malaka, "Automated Game Testing with ICARUS", CHI PLAY '17 Extended Abstracts, pp. 153-164 | https://www.smeddinck.com/files/papers/AutomatedGameTestingwithICARUS.pdf | verified |  |
| POKEAGENT | S. Karten et al., "The PokeAgent Challenge: Competitive and Long-Context Learning at Scale", arXiv 2603.15563, 2026 | https://arxiv.org/abs/2603.15563 | verified |  |
| QBERT | P. Ammanabrolu, E. Tien, M. Hausknecht and M. O. Riedl, "How to Avoid Being Eaten by a Grue", arXiv 2006.07409, 2020 | https://arxiv.org/abs/2006.07409 | unverified |  |
| XTX | J. Tuyls, S. Yao, S. Kakade and K. Narasimhan, "Multi-Stage Episodic Control for Strategic Exploration in Text Games", ICLR 2022 | https://arxiv.org/abs/2201.01251 | unverified |  |
| IGE | C. Lu, S. Hu and J. Clune, "Intelligent Go-Explore", ICLR 2025 | https://arxiv.org/abs/2405.15143 | unverified |  |
| MADOTTO | A. Madotto et al., "Exploration Based Language Learning for Text-Based Games", arXiv 2001.08868, 2020 | https://arxiv.org/abs/2001.08868 | unverified |  |
| TESTERTRON | R. Gilbert, "The TesterTron 3000", Thimbleweed Park blog, 2016 | https://blog.thimbleweedpark.com/yay_testertron3000.html | unverified |  |
| ESATARI | P. Chrabaszcz, I. Loshchilov and F. Hutter, "Back to Basics: Benchmarking Canonical Evolution Strategies for Playing Atari", arXiv 1802.08842, 2018 | https://arxiv.org/abs/1802.08842 | unverified |  |
| CURIOSITY | Y. Burda et al., "Large-Scale Study of Curiosity-Driven Learning", arXiv 1808.04355, 2018 | https://arxiv.org/abs/1808.04355 | unverified |  |
| CA2 | Chidambaram Adaikkappan et al., "CA2: Code-Aware Agent for Automated Game Testing", arXiv 2605.13918, 2026 | https://arxiv.org/abs/2605.13918 | unverified |  |
| P-FLASH | J. Ahn et al., "FlashAdventure", EMNLP 2025 | https://arxiv.org/abs/2509.01052 | unverified |  |
| JAFFAR | S. Martin (eien86), Jaffar and "JaffarPlus: High-Performance Multi-Platform TAS Optimization Bot"; TASVideos submissions #7134 (published as 4477M, made with Jaffar), #8237, #8784 and #9533 (made with JaffarPlus) | https://github.com/ToolAssisted-run/jaffarPlus ; https://github.com/SergioMartin86/jaffar ; https://tasvideos.org/7134S ; https://tasvideos.org/8237S ; https://tasvideos.org/8784S ; https://tasvideos.org/9533S | verified | Unconfirmed: an RTA-record beat by a JaffarPlus (not Jaffar) run; the throughput ratio to scummrun. Direct, in the critical revision: 4477M was obsoleted by eien86's 5206M (https://tasvideos.org/5206M); #8784 improves on the published TAS 3271M |
| LUNARBOT | eien86, "NES Lunar Pool in 22:53.62", TASVideos #9499, published as 6368M, 2025 | https://tasvideos.org/9499S ; https://github.com/SergioMartin86/LunarBot | verified |  |
| GBTASGEN | MrWint, gb-tas-gen; TASVideos #5034 | https://github.com/MrWint/gb-tas-gen ; https://tasvideos.org/5034S | verified |  |
| TIM | eien86, "DOS The Incredible Machine in 26:23.238", TASVideos #10017 (cancelled); DrD2k9 and eien86, #10481 (published) | https://tasvideos.org/10017S ; https://tasvideos.org/10481S | verified |  |
| SRBENCH | Y. Fujinuma et al., "SpeedrunBench: Challenging LLM Agents with Video Game Speedrunning", arXiv 2610.08076, 2026 | https://arxiv.org/abs/2610.08076 | verified |  |
| TMI | donadigo, TMInterface, "What is Bruteforce?" | https://donadigo.com/tminterface/what-is-bf | unverified |  |
| BXT | YaLTeR et al., bxt-rs `tas_optimizer` | https://github.com/YaLTeR/bxt-rs/tree/master/src/modules/tas_optimizer | unverified |  |
| P-MURPHY | T. Murphy VII, "The First Level of Super Mario Bros. is Easy with Lexicographic Orderings and Time Travel", SIGBOVIK 2013 | http://tom7.org/mario/mario.pdf | unverified |  |
| DRAGSTER | Omnigamer, Dragster TAS forum thread; MrWint, dragster DP solver | https://tasvideos.org/Forum/Posts/453106 | unverified |  |
| BASICBOT | BizHawk Basic Bot, TASVideos wiki | https://tasvideos.org/Bizhawk/BasicBot | unverified |  |
| LIBTAS | C. Gallet et al., libTAS | https://github.com/clementgallet/libTAS | unverified |  |
| BRUTE | M. C. Machado et al., "Revisiting the Arcade Learning Environment", JAIR 61, 2018 (Appendix A, The Brute) | https://arxiv.org/abs/1709.06009 | unverified |  |
| P-COOK | M. Cook, M Charity, M. Awiszus, F. Carnovalini and A. Dockhorn, "We Call This Controller Skip: AI for Speedrunning", EXAG/INT 2025 | https://ceur-ws.org/Vol-4090/paper1.pdf | unverified |  |
| KHANDELWAL | P. Khandelwal, F. Yang, M. Leonetti, V. Lifschitz and P. Stone, "Planning in Action Language BC while Learning Action Costs for Mobile Robots", ICAPS 2014, pp. 472-480 | https://eprints.whiterose.ac.uk/97763/1/icaps14-bc.pdf | verified |  |
| LAZYSP | C. M. Dellin and S. S. Srinivasa, "A Unifying Formalism for Shortest Path Problems with Expensive Edge Evaluations via Lazy Best-First Search over Paths with Edge Selectors", ICAPS 2016, pp. 459-467 | https://arxiv.org/abs/1603.03490 | verified |  |
| PDDLSTREAM | C. R. Garrett, T. Lozano-Pérez and L. P. Kaelbling, "PDDLStream: Integrating Symbolic Planners and Blackbox Samplers via Optimistic Adaptive Planning", ICAPS 2020 | https://arxiv.org/abs/1802.08705 | verified |  |
| AHA | B. Marthi, S. Russell and J. Wolfe, "Angelic Hierarchical Planning: Optimal and Online Algorithms", ICAPS 2008 | https://aima.eecs.berkeley.edu/~russell/papers/icaps08-hla.pdf | verified |  |
| SAHTN | J. Wolfe, B. Marthi and S. Russell, "Combined Task and Motion Planning for Mobile Manipulation", ICAPS 2010, pp. 254-257 (tech report UCB/EECS-2010-27) | https://www2.eecs.berkeley.edu/Pubs/TechRpts/2010/EECS-2010-27.html | verified |  |
| MAXQ | T. G. Dietterich, "Hierarchical Reinforcement Learning with the MAXQ Value Function Decomposition", JAIR 13:227-303, 2000 | https://arxiv.org/abs/cs/9905014 | verified |  |
| SRIVASTAVA | S. Srivastava et al., "Combined Task and Motion Planning Through an Extensible Planner-Independent Interface Layer", ICRA 2014 | https://aima.eecs.berkeley.edu/~russell/papers/icra14-planrob.pdf | unverified |  |
| ORTIZHARO | J. Ortiz-Haro, E. Karpas, M. Toussaint and M. Katz, "Conflict-Directed Diverse Planning for Logic-Geometric Programming", ICAPS 2022 | https://ojs.aaai.org/index.php/ICAPS/article/view/19811 | unverified |  |
| MBTS | M. Toussaint and M. Lopes, "Multi-Bound Tree Search for Logic-Geometric Programming in Cooperative Manipulation Domains", ICRA 2017 | https://argmin.lis.tu-berlin.de/papers/17-toussaint-ICRA.pdf | unverified |  |
| GARRETT | C. R. Garrett et al., "Integrated Task and Motion Planning", Annual Review of Control, Robotics, and Autonomous Systems 4, 2021 | https://arxiv.org/abs/2010.01083 | unverified |  |
| HPN | L. P. Kaelbling and T. Lozano-Pérez, "Hierarchical Task and Motion Planning in the Now", ICRA 2011 | https://people.csail.mit.edu/tlp/pdf/2011/hpnICRA11Final.pdf | unverified |  |
| ROLLOUTIW | W. Bandres, B. Bonet and H. Geffner, "Planning with Pixels in (Almost) Real Time", AAAI 2018 | https://arxiv.org/abs/1801.03354 | unverified |  |
| CHURCHILL | D. Churchill and M. Buro, "Build Order Optimization in StarCraft", AIIDE 2011, pp. 14-19 | https://ojs.aaai.org/index.php/AIIDE/article/view/12435 | verified |  |
| P-GROSS | M. Groß, D. Zühlke and B. Naujoks, "Automating Speedrun Routing: Overview and Vision", EvoApplications 2022, LNCS 13224, pp. 471-486 | https://arxiv.org/abs/2106.01182 | verified |  |
| P-LAFOND | M. Lafond, "The complexity of speedrunning video games", FUN 2018, LIPIcs 100, 27:1-27:19 | https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.FUN.2018.27 | direct | Theorem 8 and the FPT result read in the PDF |
| ROUTERS | A. Iškovs, Morrowind all-faction route by simulated annealing (2018); JaV, "Finding the Optimum Nadeo Cut... With Science!!" (2019) | https://www.kimonote.com/@mildbyte/travelling-murderer-problem-planning-a-morrowind-all-faction-speedrun-with-simulated-annealing-part-1-41079/ (now redirects elsewhere) ; https://www.youtube.com/watch?v=1ZsAjvO9E1g | unverified | Primary sources not fetched; known only through P-GROSS |
| MARIOAI | J. Togelius, S. Karakovskiy and R. Baumgarten, "The 2009 Mario AI Competition", IEEE CEC 2010; J. Togelius, N. Shaker, S. Karakovskiy and G. N. Yannakakis, "The Mario AI Championship 2009-2012", AI Magazine 34(3), 2013 | http://julian.togelius.com/Togelius2010The.pdf ; https://www.um.edu.mt/library/oar/bitstream/123456789/29888/1/The_Mario_AI_championship_2009-2012_2013.pdf | direct | Moved from §3.6 leads to §3.4 |
| LINESIGHT | pb4git, Agade et al., Linesight | https://github.com/Linesight-RL/linesight | unverified |  |
| ALPHAZERO | D. Silver et al., "Mastering Chess and Shogi by Self-Play with a General Reinforcement Learning Algorithm", arXiv 1712.01815, 2017 | https://arxiv.org/abs/1712.01815 | direct | Abstract only |
| MUZERO | J. Schrittwieser et al., "Mastering Atari, Go, Chess and Shogi by Planning with a Learned Model", arXiv 1911.08265; Nature, 2020 (DOI 10.1038/s41586-020-03051-4) | https://arxiv.org/abs/1911.08265 | direct | Abstract only |
| AGENT57 | A. P. Badia et al., "Agent57: Outperforming the Atari Human Benchmark", arXiv 2003.13350, 2020 | https://arxiv.org/abs/2003.13350 | direct | Abstract only |
| NRPA | C. D. Rosin, "Nested Rollout Policy Adaptation for Monte Carlo Tree Search", IJCAI 2011; T. Cazenave, "Nested Monte-Carlo Search", IJCAI 2009 | https://cs.uwaterloo.ca/~jhoey/rgwiki/uploads/11IJCAI-NRPA-Rosin.pdf ; https://www.lamsade.dauphine.fr/~cazenave/papers/nested.pdf | direct | NMCS facts taken from Rosin's paper and a two-page copy of Cazenave's |
| SEIFY | A. Seify and M. Buro, "Single-Agent Optimization Through Policy Iteration Using Monte-Carlo Tree Search", RL in Games workshop, AAAI 2020 | https://arxiv.org/abs/2005.11335 | direct | Abstract only |
| SPMCTS | M. P. D. Schadd et al., "Single-Player Monte-Carlo Tree Search", Computers and Games 2008 |  | unverified | Not fetched |
| SOP | L. Libralesso, A.-M. Bouhassoun, H. Cambazard and V. Jost, "Tree search algorithms for the Sequential Ordering Problem", arXiv 1911.12427, 2019 (ECAI 2020) | https://arxiv.org/abs/1911.12427 | direct |  |
| SNOWMAN | M. Bofill, C. Borralleras, J. Espasa, G. Martín, G. Patow and M. Villaret, "A Good Snowman is Hard to Plan", arXiv 2310.01471, 2023 | https://arxiv.org/abs/2310.01471 | direct | `related-work.md` cites it as P-PUZZLE (unverified there) |
| P-GUAN | L. Guan, K. Valmeekam, S. Sreedharan and S. Kambhampati, "Leveraging Pre-trained Large Language Models to Construct and Utilize World Models for Model-based Task Planning", NeurIPS 2023 | https://arxiv.org/abs/2305.14909 | direct | Unverified in `related-work.md` |
| LLMP | B. Liu et al., "LLM+P: Empowering Large Language Models with Optimal Planning Proficiency", arXiv 2304.11477, 2023 | https://arxiv.org/abs/2304.11477 | direct |  |
| LLMMODULO | S. Kambhampati et al., "LLMs Can't Plan, But Can Help Planning in LLM-Modulo Frameworks", ICML 2024, PMLR 235 | https://arxiv.org/abs/2402.01817 | direct | Abstract only |
| SARUYA | saruya, "Catacombs RNG manipulation", speedrun.com guide hw8us; "Seed finder for The Secret of Monkey Island", resource 2z7qm, v1.0, 2026 | https://www.speedrun.com/tsomi/guides/hw8us ; https://www.speedrun.com/tsomi/resources/2z7qm | verified | Unconfirmed: the finder's internals (source returned HTTP 403) |
| POKEFINDER | Admiral_Fish, bumba and EzPzStreamz, PokéFinder (commit 47e09ac8, 2026-10-18) | https://github.com/Admiral-Fish/PokeFinder | verified |  |
| PROJXS | niart120, Project_Xs | https://github.com/niart120/Project_Xs | verified |  |
| LUCKMANIP | TASVideos wiki, "Luck manipulation" and "Advanced Luck Manipulation" | https://tasvideos.org/LuckManipulation ; https://tasvideos.org/AdvancedLuckManipulation | unverified |  |
| FF12 | Tranquilite0, FF12RNGHelper | https://github.com/Tranquilite0/FF12RNGHelper | unverified |  |
| PHPMT | Solar Designer, php_mt_seed | https://www.openwall.com/php_mt_seed/ | unverified |  |
| MARSAGLIA | G. Marsaglia, "Xorshift RNGs", Journal of Statistical Software 8(14), 2003 | https://www.jstatsoft.org/article/view/v008i14 | unverified |  |
| LATTICG | mjtb49 et al., LattiCG | https://github.com/mjtb49/LattiCG | unverified |  |
| YURICHEV | D. Yurichev, "Predicting Math.random() in Firefox using Z3 SMT-solver" | https://yurichev.com/blog/xorshift/ | unverified |  |
| HARAMOTO | H. Haramoto et al., "Efficient Jump Ahead for F2-Linear Random Number Generators", INFORMS Journal on Computing 20(3), 2008 | https://www.gerad.ca/en/papers/G-2006-62 | unverified | Only the abstract was read |

Local sources: `README.md`; `docs/optimization.md` (method, candidates, context variance);
`docs/comparison.md` (path 686, rows #16 and #58); `docs/anypercent/plan.md`;
`docs/anypercent/findings.md`; `docs/research/anypercent-runner-sources.md`;
`docs/research/anypercent-engine-mechanics.md`; `docs/next.md`; `docs/related-work.md`;
`rules/anypercent.md`; `src/speedrun/executor.py`; `src/speedrun/control.py`;
`tests/integration/test_control.py`; `data/scripts-dos-cd/room-030-store/local-205.txt`;
`routes/anypercent/S1-trials.json` to `S6-catacombs-ghostship.json`; `docs/extraction-diff.md`.

## 7. Corrections in the critical revision

These statements in the first version were wrong, out of date or unfair, and are corrected above.

- §4.1 listed glitch encoding as planned only. As of 2026-10-19, `routes/anypercent/` holds an Any%
  route of 465 moves, 22 of them with cited glitch mechanisms. No planner produced it, and it is not
  executed yet.
- §1 and §4.5 said our macro level holds up without saying that this is true for glitchless Part I
  only. The Any% route is not produced by the planner, so the table's "no route planner" applies to
  us too.
- The "ours" row and §4.2 gave the glitchless objective (mean ticks over seeds) as our only one.
  Under Any% the seed is chosen and the objective is one deterministic run, as in a TAS.
- §1 called Jaffar's DOS Prince of Persia TAS "the verified record-beater" because it was 27 s under
  the RTA record. That is the low bar for a TAS. The only earlier TAS the submission compares
  against had a different goal (level skip). The stronger evidence is JaffarPlus's SNES draft beating the published TAS
  3271M by about a minute.
- Row C said agents are "2x or more slower than human runners" where timed. ICARUS is timed and is
  compared with QA testers, not runners.
- Row F said routers were "never validated by a timed replay in the game". Groß et al. report that
  JaV's TrackMania route improved leaderboard times.
- §1 said solvers and agents "do not minimise time", although PokéAgent and SpeedrunBench score
  time.
- §4.3 called the default cost for an unmeasured pair "not admissible". It is not known to be
  admissible, which is what the argument needs.
- Recommendation 1 compared the record's 50 s catacombs with "many minutes unseeded". No source
  gives an unseeded time; the phrase came from a summary table in
  `docs/research/anypercent-runner-sources.md` that cites nothing for it.
- Recommendation 1(c) carried over a human criterion (the width of the timing window) without
  saying why a frame-exact bot needs it. It now gives the bot's reason: robustness to route changes
  upstream.
- Lafond (2018) and the Mario AI competition papers were unverified leads. Both now have direct
  status and their own entries (§3.6, §3.4).
