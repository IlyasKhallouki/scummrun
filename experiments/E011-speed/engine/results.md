# E011 engine component, round 1: gates and bench

What was built: patch 0002 (`patches/0002-scumm-speedrun-lean.patch`); the slim build in
`/var/tmp/fast-mi/svm-e011` (`build.sh`); and `rs/fmi-engine`, the Rust driver over the binary channel. `WIRE.md`
describes both ends of the channel.

The final build:
- the scummvm binary's sha1 is c1b8db05;
- patch 0002's sha1 is 7831475e;
- `git apply` of 0001 and 0002 on v2026.3.0 reproduces the built source tree exactly (`diff -r`: no difference).

All runs: the laptop, seed 1, nice 10, 2026-11-02. Other jobs kept the load average at 13 to 31 and the cores at
about 1.5 to 1.9 GHz throughout. Wall and CPU times are therefore inflated against an idle laptop; the cycle counts
below do not depend on the load.

## Gates (final build)

| gate | check | result |
|---|---|---|
| G1 | E003 `fork_exact.py run` on the lean build (`svm-lean.sh`, all four switches): 3,000 frames from 235, 7 load children, exact dumps | 6,002 C and P comparisons, 0 differ; 5,836 load children, 27,623 comparisons, 0 differ; P's trace equals F's; the control (one RNG bit flipped) differs from frame 236 |
| G2 | door chain (E009 door archive, entry 5, frame 3129): 37 exact dumps (the profile's `chain.py`, copied here) against svm-w3b | 0 differences for each of: the E011 build with every switch off, each switch alone (splash, present, events, audio), all four, and fmi-engine over the binary channel with all four and `joystick_num=-1`; state:316 = 1 at 3129 |
| G3 | kitchen chain (E009 off check, `jobs/ta/new` entry 7, frame 3216): 40 exact dumps, the same seven runs | 0 differences; owner:566 = 1 at 3216 |
| G4 | the binary `Obs` (names and the exact object) equals the `Obs` parsed from the JSON dump, and `facts()` from F2 equals `facts_of` | 0 differences at the 77 dumps of G2 and G3, and at every frame of 400 from 2728 on the kitchen movie (one Step each, with runs and explain) |
| G5 | E009's step test (a) through the Python reference on the lean build | `CHECK (a): 0 goal True 2728 3216 488 11712 2 36 501 8` (501 children); verify from boot 0 differences at 9 checkpoints, owner:566 = 1 |

The G4 walk checks more at every frame:
- the Step's block (a delta) equals a State block taken from a fork at the same stop;
- the facts kept up to date through the deltas equal those built whole: 399 of the 400 frames followed the delta, 1
  was built whole;
- a second engine played the same 400 frames by Step alone, with no JSON and no Obs, and its final dump equals the
  observed engine's.

Other checks:
- **Return by fork, and a rebuilt entry.** The kept-entry pool returned to every entry of the door archive (6) and of
  the kitchen job (8) by fork, three times each. It then evicted all of them but the root and made them again from it
  (replaying their movies). Every return's last checkpoint compared with 0 differences.
- **fmi-engine's verify.** The door chain from boot took 0.73 s, with 0 differences at 6 checkpoints. For
  comparison, the reference's verify of the kitchen chain took 4.0 s on svm-w3b (E009's off check) and 1.1 s on the
  lean build (G5).
- **Engine tests** (`cargo test -- --ignored`):
  - forks from four threads at once each reach their own engine;
  - the bridge's errors come back to the caller and the engine serves the next command.

## Bench: before and after, back to back (`bench.sh`, round 1 at load 13.5)

"Before" is the Python reference on svm-w3b over JSON:
- E010's `frame_costs.py` (300 frames);
- the profile's `chain.py speed` (the door chain, 3 runs).

"After" is `fmi-engine bench` on the E011 build with the four switches, over the binary channel. Round 2 ran at the
same load average with more spread; where the two rounds differ, both are given.

| measure | before | after | target | floor (profile) |
|---|---:|---:|---:|---:|
| a. parent frame, observed (one Step: frame, runs, explain, block delta, facts) | 12.5 ms | 0.18-0.19 ms wall (engine 0.14 ms CPU, driver 0.043 ms CPU) | ≤ 0.15 ms | the frame alone, 0.07 ms |
| b. parent frame with fork_each 7 (each child's runs, explain, facts, quit) | 19.1 ms | 2.3-2.6 ms | | |
| c. bare frame, one command each | 1.4 ms | 0.148 ms | | |
| c. frame inside a 50-frame command (engine CPU) | 0.8 ms wall | 0.066-0.074 ms | ≤ 0.10 ms | 15 µs logic; 30-40 µs with the unbuilt cuts |
| c. frame + state | 7.3 ms (JSON dump) | 0.160 ms (Step with the block) | | |
| full binary Obs (no names) | 2.8 ms of JSON + parse | 0.23-0.24 ms wall (engine 0.07 ms CPU, decode 0.16 ms CPU) | ≤ 0.10 ms | |
| F2 block (State round trip) | | 0.036 ms wall (engine 0.022 ms CPU) | | 0.3 µs copy |
| round trip (Holds, no condition) | 0.08-0.15 ms (profile) | 16-24 µs | | |
| d. fork + quit | 2.9 ms | 1.07-2.07 ms wall, parent 0.85-1.10 ms CPU | ≤ 1.0 ms parent CPU | 0.67-1.33 ms |
| start to the first stop | 0.89-0.92 s wall | 73-84 ms wall, 60-70 ms engine CPU | ≤ 0.1 s CPU | 80-90 ms |
| return to a kept entry (fork, connect, hello, status) | 4.4-4.8 s (replay from boot) | 1.06 ms median (p90 1.3 ms); round 2 4.4 ms (p90 9.1 ms) | ≤ 3 ms | 1-2 ms |
| door chain replayed from boot (start + 3,129 frames) | 4.4-4.8 s | 0.65-0.68 s wall, 0.64 s engine CPU | ≤ 0.5 s | |

**The channel alone** (the same bench with the switches off) separates the two changes:
- An observed frame costs 1.5-1.9 ms, a frame in a 50-frame command 1.1-1.2 ms of engine CPU, and the door chain from
  boot 4.7-10.5 s.
- So the switches buy the frame costs (about 16 times). The binary channel buys the observation:
  - an observed frame's overhead over the bare frame falls from about 11 ms (12.5 against 1.4) to about 0.03 ms
    (0.179 against 0.148);
  - a frame with its state takes 0.160 ms (Step with the block) where the run and a JSON dump took 7.3 ms.

**Cycle counts** (user space, `perf stat` on the engine, three runs each):
- **The door chain from boot.** 1.34-1.37 G cycles and 1.954 G instructions, as in the profile's slim lean build
  (1.35 G, 2.03 G). At the laptop's 3.6 GHz turbo that is 0.37-0.38 s; the 0.64 s of CPU measured here ran at about
  2 GHz.
- **Frames at 2728+ in 50-frame commands:** 162 k cycles each (45 µs at 3.6 GHz).
- **A Step:** the frame plus 43 k cycles (runs, explain entries, the block and its delta).
- **A full binary Obs:** 90 k cycles in the engine.

## Against the targets

**Met:**
- a frame inside a command, 0.066-0.074 ms of engine CPU (≤ 0.10);
- the start, 60-70 ms of CPU (≤ 0.1 s);
- a return by fork, 1.06 ms median in round 1 (≤ 3 ms).

**At the target:** fork + quit, 0.85-1.10 ms of parent CPU (≤ 1.0). It is mostly the kernel's fork of the slim
process (89 VMAs).

**Not met at this load:**
- **The door chain from boot.** 0.65-0.68 s of wall against ≤ 0.5 s. Its cycle count gives 0.38 s at turbo clock. What
  is left per frame is costume and strip pixels and the silent streams' rate conversion. The cuts for those (a
  count-only mixer, skipping pixel output) are later-round items that need their exactness argument first (plan.md
  "Later rounds").
- **The observed frame.** 0.18-0.19 ms of wall against ≤ 0.15 ms: the frame's engine CPU is 0.13-0.15 ms at this
  load (0.07 ms in batch). Its driver side fell from 0.067 to 0.043 ms once the facts followed the deltas.
- **The full binary Obs.** 0.23 ms against ≤ 0.10 ms. The engine side is 0.07 ms of CPU. The rest is decoding into
  fmi-core's `Obs`: 696 class lists, each its own `Vec`. The hot path does not use the full Obs: it uses `facts()` from
  F2 (0.036 ms with its round trip).
- **The return by fork in round 2.** 4.4 ms median, with a p90 of 9 ms, under the same load average as round 1's
  1.06 ms.

## Deviations from the spec

- **`step` is not in fmi-core's `Engine`.** It is `fmi_engine::BinEngine::step`, an inherent method returning
  `fmi_engine::StepOut`. COORD.md has an IFACE entry proposing it for the trait. After a Step, and on a fork's child,
  `facts()` costs no round trip.
- **What F1 adds to plan.md's table** (WIRE.md has the details):
  - the StepReply layout and its delta;
  - End carries the frame, tick, qf and room, plus the children a fork_each made before the end;
  - a fork child drops whatever its parent had received but not read.
- **One control socket per root engine** and its forks, as control.py's Controller has. The root is found by its hello
  (parent 0), not by the pid spawned, so a wrapper such as `perf stat` works.
- **The Python reference's runs on the lean build go through `svm-lean.sh`.** The reference's launcher drops the
  caller's SPEEDRUN_* variables, so the wrapper sets the switches in the engine's own environment.
- **`joystick_num=-1`** is written only by fmi-engine's launcher, into its ini when lean. The reference's ini is
  unchanged, so G5 ran without it. G2 and G3's fmi-engine runs ran with it.
- **The kept-entry pool's `keep()`** takes the caller's word that the engine has no input scheduled past the entry's
  frame. `ret()` of an entry not kept makes it from the nearest kept ancestor whose movie is a prefix of the entry's,
  else from boot.
- **G1 and G5 ran twice.** The engine was rebuilt during the first runs (binary dump changes only). Both were run
  again on the final build; the table gives those runs.
