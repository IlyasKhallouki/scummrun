# Decisions

Each decision records who made it, when, and why, in their own words.

## D1 (2026-10-21): No language model inside the system

Decided by: Ilyas.

Using an LLM would mostly measure what the LLM can already do once it has a harness and some guidance;
that wouldn't be our contribution. Our goal is a system that finds the fastest route deterministically,
by search and mathematical reasoning, so every result can be reproduced and explained.

## D2 (2026-10-21): Research question and working title

Decided by: Ilyas.

Research question: can a deterministic system, given only a game's files and rules, finish a ScummVM
adventure game and find the fastest route it can prove within a defined search space?

- RQ1, completion: do the game's scripts and engine contain enough information to finish it with no
  human route?
- RQ2, optimization: how close does search over engine-measured costs get to the fastest possible run,
  and can we bound the gap?
- RQ3, exploits: can glitches be found by general detectors rather than game-specific knowledge?
- RQ4, generality: how much of the system carries over to a second game unchanged?

Working title: *Speedrunning as Search: Script-Derived Planning and Engine-Verified Optimization for
ScummVM Games*.

Why this title: it is about the method, not only our ScummVM fork. The failures and successes along the
way are what led to the conclusions.
