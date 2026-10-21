# Outline

Title: *Speedrunning as Search: Script-Derived Planning and Engine-Verified Optimization for ScummVM Games*

Research question: can a deterministic system, given only a game's files and rules, finish a ScummVM
adventure game and find the fastest route it can prove within a defined search space?

1. **Introduction.** Speedrunning adventure games; why it is a hard search problem; the research question
   and RQ1-RQ4; contributions.
2. **Background and related work.** Speedrun routing complexity, automated planning, TAS tools,
   game-playing agents, program analysis of game scripts (`docs/related-work.md`,
   `docs/research/strategy-comparison.md`).
3. **The engine as an oracle.** Deterministic bridge: game-tick clock, input movies, process forks
   with the random state, from-boot verification.
4. **From scripts to goals (RQ1).** Script export, world model, goal regression, exploration.
5. **Finding fast routes (RQ2).** Planning on measured costs, frame-level optimization, random-event
   control, bounds on the gap.
6. **Exploits (RQ3).** Generic detectors and how they are tested and priced.
7. **Evaluation.** Monkey Island (tuned on Part I, tested on Parts II-IV), Monkey Island 2 as the
   second game (RQ4); baselines in `notes/baselines.md`.
8. **What failed and why.** Negative results from the notebook.
9. **Discussion and limitations.**
10. **Conclusion.**
