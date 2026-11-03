# E011 route component (round 2, item 3): Fast Downward's lama-first-costs in Rust

Round 1's Rust search (Gbfs: lazy GBFS with h_FF and preferred operators) plans another route than Fast Downward's
lama-first-costs on test (a) and on 3 of the 15 tries, and on test (a) that route stops without the goal. This component
ports the search E009 runs, `plan_fd.LAMA_FIRST_COSTS`, to Rust so that the fast settings play the reference's routes:

```text
let(hlm, eval_modify_costs(landmark_sum(lm_factory=lm_reasonable_orders_hps(lm_rhw()), pref=false), cost_type=plusone),
let(hff, eval_modify_costs(ff(), cost_type=plusone),
lazy_greedy([hff, hlm], preferred=[hff, hlm], reopen_closed=false)))
```

All runs: the laptop, seed 1, nice 10, 2026-11-02/03, with the load average at 8 to 25 from the other round-2
components. CPU seconds and instruction counts are given where walls were inflated.

## What was built

- `taskcore/src/stdhash.rs`: libstdc++'s `unordered_set` and `unordered_map` (GCC 16.2.1, the version Fast Downward is
  built with), reduced to what decides their iteration order (the bucket list, insertion at a bucket's front, the prime
  rehash policy, rehashing, erasure), with Fast Downward's lookup3 hash of `FactPair` and `std::hash<int>`. lm_rhw makes
  its landmarks in the order its sets and maps are iterated, so the port iterates them in the same order.
- `taskcore/src/landmarks.rs`: `lm_reasonable_orders_hps(lm_rhw())` on the in-memory SAS task, rule for rule: the goal
  landmarks, the backchaining queue (shared preconditions, critical predecessors in the domain transition graph,
  disjunctive preconditions per predicate class, overlap rules, a disjunctive landmark replaced by an atomic one), the
  forward orderings, the ids, the achievers' least adapted costs, and the reasonable orderings (Hoffmann et al.'s
  interference with no mutex groups: two values of one variable).
- `taskcore/src/lama.rs`: the landmark status per state (past and future, progressed once per state as the lazy search
  does), landmark_sum, the lazy search with the alternation of [hff, hff preferred, hlm, hlm preferred] and boost 1000,
  and Fast Downward's successor order as a trie. h_FF is search.rs's `Ff` through the `RelaxedEval` trait;
  `lama_first_costs_with` takes any h_FF with the same values and preferred operators (r2-plan's watched h_FF).
- `taskcore/src/bin/lama.rs`: `lama search --sas F`, `lama graph --sas F` (the graph as Fast Downward's debug dump),
  `lama stdhash F`.
- `rs/fmi-plan`: `Host::rust_lama` (`FMI_PLAN_LAMA=rust`; `fmi-plan seq --search lama`): `Search::FdLamaFirstCosts`
  served by lama.rs instead of Fast Downward's binary. Off by default.
- `route/`: the checks (`lama_same.sh`, `graph_cmp.py`, `g10.sh`, `b3lama.sh`, `bench_plan.sh`) and the libstdc++
  reference (`stdhash_ref.cc`, `stdhash_scripts.py`).

## Exactness

The port changes no result in two places, each argued in the code:

- **The reachability** (exploration.cc's fixpoint) is computed per precondition set and condition set, not per unary
  operator: the same fixpoint. A conditional effect on an atom an unconditional effect of the same operator reaches is
  left out (it fires only when the operator has fired), and so are conditions that are also preconditions.
- **First achievers of a landmark that holds at the start** are not computed: such a landmark is past at the start and
  stays past in every state the search makes from it, where landmark_sum reads only its possible achievers. On the five
  tasks below, every other least first-achiever cost equals the first version's (which computed them all).

## Gates

| gate | check | result |
|---|---|---|
| stdhash | 300 random scripts (inserts, erases, copies, intersections, constructions from vectors with repeats; sets up to 10,273 buckets) on libstdc++'s containers (`stdhash_ref.cc`) and on stdhash.rs | the same bytes in all 300 |
| G7, the lama clause | Fast Downward's lama-first-costs (its search binary, the landmark dump on) against `lama search` on taskcore's SAS files (`lama_same.sh`): the landmark graph (nodes in id order with their atoms in order, orderings with their types), the plan file, expanded / evaluated / generated | equal on 30 requests: serve38 6, serve33 7, test (a) 5 (two more start outside the reading: taskcore refuses them for both), ring(38) other goals 12 (2 of them unsolvable: both prove it at the start) |
| G7, PlanResult | `fmi-plan seq --search lama` against plan_fd.plan_taskcore's results (the plan component's `compare.py`) | serve38: all 6 the same (see below for serve33) |
| G10 | test (a) with `fmi try --search fd` and `FMI_PLAN_LAMA=rust` (`g10.sh`), cold, the reading made fresh | strict: `CHECK (a): 0 goal True 2728 3216 488 11712 2 36` (501 children, 8 entries); ready: the same line (320 children); ready with the old tail (the fast settings): the same line (319 children); in all three every new entry (8) verifies from boot in a fresh lean engine with 0 differences |
| G11 | B3's warm sequence (one fmi process, E009's 15 kitchen tries in log order, kept entries, warm planners, reading cache, memo and plan store empty at the start; `b3lama.sh`) with the fast settings (play when ready, the old tail first, `--search fd`, `FMI_PLAN_LAMA=rust`), each try against B0's (bench/cmp_tries.py) | 15 tries: the stop class B0's in 15, everything (target, frames, replans, refinements, inputs, the new entries' frames, ticks, events and checkpoints) B0's in 15; the 43 new entries verify from boot in a fresh lean engine, 0 differences. Round 1's fast settings (Gbfs) played another route in 3 |

Gbfs on test (a) stopped at 3174 (strict) or at the planner's 600 s limit (ready with the tail); with the Rust
lama-first-costs the fast settings reach the kitchen on the reference's route.

## Bench

**The landmark graph** (CPU of `landmarks::build`, the least of 3 at load 4-6, and its user-space instructions; Fast
Downward's "Landmark graph generation time" from its log on the same SAS file, at load 10-20):

| task | landmarks (disjunctive) | orderings | Fast Downward | Rust | instructions |
|---|---:|---:|---:|---:|---:|
| ring(38) j00091, bit:108=1 | 14 (1) | 24 | 0.32 s | 0.008 s | 62 M |
| ring(33) j00089, bit:108=1 | 16 (1) | 29 | 16.06 s | 0.13 s | 933 M |
| test (a), request 1 | 34 (1) | 138 | 8.66 s | 0.076 s | 516 M |
| test (a), request 2 | 28 (1) | 77 | 7.83 s | 0.107 s | 794 M |
| test (a), request 3 | 16 (0) | 36 | 6.89 s | 0.058 s | 437 M |

Of ring(33)'s 933 M instructions: the reachability's tables about 600 M (2.6 M effects of 97,547 operators, 1.0 M
conditional), the 6 explorations about 150 M, the providers 80 M.

**A whole search on a SAS file** (`lama search`, after the file is read; Fast Downward's total from its log includes its
own parse): test (a)'s first request, 30 steps, 2,474 evaluated: Fast Downward 65.5 s (search 54.3 s); Rust 84 s of CPU
with today's h_FF (search.rs `Ff`, FD's loop transcribed: 34 ms an evaluation, load 15-20), 7.56 s of CPU (21.8 G
instructions) with r2-plan's h_FF by watched preconditions (a scratch merge of e011-r2-plan, never pushed; load 6:
the graph 0.11 s, h_FF's relaxed task and the trie 0.25 s, the search 7.42 s of which h_FF 7.30 s, 2.95 ms an
evaluation), the same plan and counts. Gbfs on the same file and h_FF: 7.17 s (25.6 G instructions), 2,431 evaluated.

**Through fmi-plan, the Rust lama against the Rust Gbfs on the same requests** (one fmi-plan process per round, the
plan cache off, rounds interleaved; CPU of the whole request: the warm request's reductions, the in-memory task, the
search, the check, the decode):

| requests | Gbfs | lama | lama / Gbfs |
|---|---|---|---|
| serve38, round 1's reductions and h_FF (3 rounds, mean) | 0.947 s for the 6 | 1.050 s | 0.92-1.20 per request (least of 3), 1.11 the sequence |
| serve38, with r2-plan's fast switch (scratch merge; 10 rounds, mean) | 0.853 s for the 6 | 0.968 s | 1.04-1.21 per request (means), 1.135 the sequence |
| test (a)'s 5 requests (2 rounds, least) | a0 65.4, a1 6.4, a2 5.2, g1 6.4, g2 8.0 s | 61.6, 4.75, 3.04, 4.06, 7.02 s | 0.94, 0.74, 0.58, 0.63, 0.88 |
| serve33 (2 rounds, least; an earlier build of the graph) | 16.5, 7.3, 6.9, 6.5, 17.7, 16.4, 14.0 s | 17.2, 8.4, 7.9, 8.6, 17.4, 16.0, 10.5 s | 1.04, 1.15, 1.14, 1.34, 0.98, 0.98, 0.75 |

The two searches evaluate as many states on serve33, lama more on ring(38) (11 against 8 on three requests) and fewer
on test (a)'s second and fourth requests (20 against 40). lama's own fixed work per request is the landmark graph and
the successor trie (ring(38): 0.015-0.02 s; ring(33): 0.3-0.4 s). On test (a) lama is the cheaper: Gbfs's successors
are a scan of every operator per expansion, lama's a walk of the trie.

## Against the targets

| target | measured | met |
|---|---|---|
| plans byte for byte Fast Downward lama-first-costs' on serve38, serve33 and test (a)'s requests | 30 requests equal, with the graph and the counts | yes |
| CPU at most 1.2x Gbfs's on the same requests | ring(38) 1.04-1.21 (mean 1.135) with the fast switch; test (a) 0.58-0.94; serve33 0.75-1.34 (least of 2, an earlier build, the reductions' noise dominates: lama's search part adds 0.3-0.5 s to 6-17 s) | on the sequences yes; per request not everywhere |
| test (a)'s first request at most 5 s | the search 84 s of CPU with today's h_FF; 7.56 s with r2-plan's watched h_FF (2,474 evaluations at 2.95 ms = 7.30 s; the graph 0.11 s, the relaxed task and the trie 0.25 s, the search's own work 0.12 s), before the request's reductions | no: h_FF binds (r2-plan's item) |

The floor of this search is Fast Downward's number of h_FF evaluations (fixed: the plans must be its plans) times the
cost of one exact h_FF, plus the graph. lama's own parts on test (a)'s first request: 0.25 s of 7.56 s.
