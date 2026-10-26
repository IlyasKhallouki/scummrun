# Decisions

Each decision records who made it, when, and why, in their own words.

## D1 (2026-10-21): No language model inside the system

Decided by: Ilyas.

Using an LLM would mostly measure what the LLM can already do once it has a harness and some guidance;
that wouldn't be our contribution. Our goal is a system that finds the fastest route deterministically,
by search and mathematical reasoning, so every result can be reproduced and explained.

Clarified 2026-10-22, by Ilyas: model agents may do the repetitive work of building the system, such as
writing the engine table from ScummVM's source. Ilyas designed the approach and the way of thinking about
what we do; the agents do repetitive work under it, like anybody using a tool to code, even a simple
website. At the end of the day the system is deterministic. After this, we put in another game and it
should work.

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

## D3 (2026-10-22): Start from the full cloud of causes, SCUMM v5 only for now

Decided by: Ilyas.

A small cloud is better, but we start with all the extra variables and then use specific techniques to
eliminate the ones we can. We focus on SCUMM v5 for now, and make sure other engines (Sierra's AGI and
SCI, with things like King's Quest's speed dial) can be added later.

The size cap on what an analysis hands over is decided after the cloud is extracted, once we see how
many causes there are and what that means for the cloud.

## D4 (2026-10-22): No size cap; find a path through the full cloud first, then optimize

Decided by: Ilyas.

If we make the cloud smaller, we would probably never get to a good number. The optimization is not in
capping the number of variables or actions we use. We first draw a path from the beginning to the end
through the full cloud, using those actions, and then work out how to make it shorter.

An idea for that second step, not decided yet: patterns in the items a traversal crosses could let the
search skip certain paths and go much faster.

This settles the size cap that D3 left open: there is none.

## D5 (2026-10-23): The next move does not wait for the game to settle

Decided by: Ilyas.

One part of speedrunning these games is giving the next input before the game would normally allow it.
So we shouldn't do one move, wait until the game freezes, and then do the next. That also would not work
with glitches.

This replaces row 3's rule of letting time run until every script stops. After a move, scripts that are
still running stay running, and the next move can land on any frame where the engine takes that input.

## D6 (2026-10-23): Try the Part I search before the 90% gate, on a cloud machine with a $20 cap

Decided by: Ilyas.

If we run it and it is able to find a start to end path, that is a huge milestone that answers at least
one of the paper questions. So we do it now instead of waiting for the model check to reach 90%. The heavy
runs go to a cloud machine in the lumind-mvp project, on a $20 budget that we stick to and do not cross.

This changes E003's plan: the first full Part I search no longer waits for step 3's gate. The gate still
decides what the paper reports about the model's precision, and every plan is still checked by replay in
the engine (step 5).

## D4, clarified (2026-10-24): No cap, but optimizing is allowed

Decided by: Ilyas.

When we said we don't cap the cloud, that did not mean we don't optimize it. A cap was what we had at the
beginning: mindlessly not using some things. Optimizing is correctly, mathematically removing things that
won't be used in the final optimum route.

## D7 (2026-10-24): Move the heavy machinery to Rust, starting with the reader

Decided by: Ilyas.

Memory and speed are what block the Part I search, and we should control the memory and how it flows in
and out. Rust with controlled memory and I/O is much faster than Python, so we port the task building
(stage 1) and the reader itself (stage 2) to Rust, as fast as possible, because it unlocks everything
else. We should have done this before.

The Python code stays the reference: the Rust version must reproduce its outputs byte for byte (the
model check's rows 0-5 and the kitchen reading) before it replaces it. Model fixes (fix round 1) wait
until the port is done and are then made in Rust.

## D8 (2026-10-25): Fix round 1 approved as designed, with its open choices

Decided by: Ilyas.

"Of course I approved it, because otherwise how would we move on?" Approved: the fix round 1 design
(experiments/E003-path-through-cloud/fix-round1.md) and its order; const_polls on; inputs read after the frame's
scripts, except what processInput does itself (Esc's override test and '.'), with due events first in the
planner; '.', the talk-stop key, counts as a player input; the bridge dumps each live slot's locals. Also the
replay's match measure: an unpredicted change counts against a step only when a script the step ran or started
made it, and the strict count is reported next to it.

Also today: "Do the vm if it's faster, why wait" (the first Part I attempt runs on the cloud VM, within D6's cap).

## D9 (2026-10-26): Only proven reductions, no hand-written rules

Decided by: Ilyas.

"I don't really go with the idea of writing down like specific rules. I'm more into some mathematically proven
stuff." Moves that lead to nothing are dead-end branches of the cloud, and they come from the scripts' structure.
So the search is not guided or pruned by a written list of genre or speedrun rules; what removes or orders paths
must be proven, as in D4 (clarified).
