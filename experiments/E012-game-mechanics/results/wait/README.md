# E012 `wait`: gates W1-W4

Branch e012-wait. `fmi check wait` (fmi-explore/src/waitcheck.rs) through `wait/gates.sh`. Laptop, seed 1, round 2's
engine build (svm-e011-r2int, read only) with every round-2 switch on (ON3), this branch's fmi, nice 10 under the guard.
The kitchen archive of E011's check (`/var/tmp/fast-mi/e011-archives/kitchen`) is read only: each entry is rebuilt from
boot from its movie, and its return equals its checkpoint (14 of 14).

Records: `room28.json` (entries 203, 205, 225), `room28b.json` (the other ten room-28 entries), `e4.json` (entry 4),
`w4-28.json` and `w4-38.json` (W4's timing under the heavy lock), `w4b-28.json` and `w4b-38.json` (W4's timing again at
a lower load), `load.txt` (the load at each group's start and end).

## T

- Room 28 entries: the kept reading 28-33-41-j00065 (copied). By `tested`'s rule: 60 keys tested by inputs, 46 by other
  units' events, 97 in all, 154 atoms. These are tested's T1 numbers on the same reading (results/tested/t1.md). 96 keys
  are observed: actorroom:6 is not.
- Entry 4: a fresh reading of its job's scope (33, 38, 83, 85, 96) from its state: 43 input keys, 27 event keys, 66 keys,
  97 atoms, all observed.

## The waits

Per entry: the paused slots at the entry (unit, delay in ticks, due frame), the stops (frames after the entry), the end,
the frames run, and the checks:

- W1: stops verified from boot;
- due: paused slots still in their wait at due - 1 with the delay the rule gives, and run at due;
- view: frames where T's values from the changes differ from `start::view` of the whole facts;
- idle: the wait's E009 record against E009's rule on the whole facts of each of 1,600 frames.

| entry | frame | room | paused at the entry | stops | end | frames | W1 | due | view | idle |
|---|---|---|---|---|---|---|---|---|---|---|
| 203 | 3393 | 28 | local:210 26 → +5, local:207 54 → +10, local:211 2760 → +461 | +2, +5, +461 | horizon | 2079 | 3/3 | 3/3 | 0/2079 | same |
| 205 | 3443 | 28 | local:210 9 → +2, local:207 120 → +21, local:211 2460 → +411 | +2, +7, +411 | horizon | 2016 | 3/3 | 3/3 | 0/2016 | same |
| 225 | 3530 | 28 | local:210 10 → +2, local:207 48 → +9, local:211 1938 → +324 | +2, +13, +324 | horizon | 1929 | 3/3 | 3/3 | 0/1929 | same |
| 204 | 3395 | 28 | local:210 14 → +3, local:207 42 → +8, local:211 2748 → +459 | +1, +3, +27, +44, +48, +459 | horizon | 2064 | 6/6 | 3/3 | 0/2064 | same |
| 206 | 3444 | 28 | local:210 3 → +1, local:207 114 → +20, local:211 2454 → +410 | +1, +6, +410 | horizon | 2015 | 3/3 | 3/3 | 0/2015 | same |
| 207 | 3444 | 28 | local:210 3 → +1, local:207 114 → +20, local:211 2454 → +410 | +1, +6, +410 | horizon | 2015 | 3/3 | 3/3 | 0/2015 | same |
| 208 | 3396 | 28 | local:210 8 → +2, local:207 36 → +7, local:211 2742 → +458 | +2, +26, +43, +47, +458 | horizon | 2063 | 5/5 | 3/3 | 0/2063 | same |
| 209 | 3422 | 28 | local:207 6 → +2, local:211 2586 → +432 | +1, +17, +21, +432 | horizon | 2037 | 4/4 | 2/2 | 0/2037 | same |
| 220 | 3396 | 28 | local:210 8 → +2, local:207 36 → +7, local:211 2742 → +458 | +2, +26, +43, +47, +458 | horizon | 2063 | 5/5 | 3/3 | 0/2063 | same |
| 221 | 3422 | 28 | local:207 6 → +2, local:211 2586 → +432 | +1, +17, +21, +432 | horizon | 2037 | 4/4 | 2/2 | 0/2037 | same |
| 222 | 3432 | 28 | local:210 41 → +7, local:207 60 → +11, local:211 2526 → +422 | +6, +7, +11, +422 | horizon | 2027 | 4/4 | 3/3 | 0/2027 | same |
| 223 | 3444 | 28 | local:210 3 → +1, local:207 114 → +20, local:211 2454 → +410 | +1, +6, +410 | horizon | 2015 | 3/3 | 3/3 | 0/2015 | same |
| 224 | 3445 | 28 | local:210 26 → +5, local:207 108 → +19, local:211 2448 → +409 | +3, +5, +85, +409 | horizon | 2014 | 4/4 | 3/3 | 0/2014 | same |
| 4 | 2862 | 38 | none | none | static | 1600 | — | — | 0/1600 | same |

In all: 50 stops, each verified from boot with 0 differences; 37 paused slots, each due frame exact on the engine; 27,974
frames with T's values equal to the harness's view; 14 idle records equal to E009's rule. Every frame's tick advance was
var 19 at the frame before (0 misses), and every stop's facts from the changes equalled its own dump (0 misses).

**What stops.**

- The animation locals 208 and 209 start or stop at +1 to +17: one stop for each, its first truth change.
- At entries 204, 208 and 220, which start in the cutscene that ends a sentence, control comes back at +26 or +27 (the
  verbs, var 32, var 52-53, script 23 and its position). The dialog lines (verbs 120-123) appear at +47 or +48.
- At entries 209, 221, 222 and 224 the dialog lines appear at +11 to +85.
- Then the cook's door: state:316 and state:570 open, and local 211 ends, at the frame `due` gives for local 211 at the
  entry.

The door opens at frame 3854 from every room-28 entry. The local's timer was set before these entries, and no input of
theirs moved it.

## W1: every wait entry verifies from boot

Each stop was verified as an entry of the archive would be: the entry's movie, its chain's checkpoints and the stop's
checkpoint (51-58 checkpoints), in a fresh lean engine from boot. 50 of 50 stops: 0 differences. **Pass.**

The check boots to the first checkpoint and gives the rest of the movie with `inputs`. fmi-engine's boot keeps only the
events up to its frame, so E011's `HostVerifier::replay_verify` does not serve here (COORD, 07:50).

## W2: the door's stop and `due`

| entry | the door's stop | due(local 211) at the entry |
|---|---|---|
| 203 | +461 | +461 (delay 2,760, Δ 6) |
| 205 | +411 | +411 (delay 2,460) |
| 225 | +324 | +324 (delay 1,938) |

The plan's +461, +411 and +324, and `due` equal to them. The other ten room-28 entries agree as well: the door's stop
is at due(local 211) in 13 of 13 entries. **Pass.**

## W3: entry 4 is static on its scope's T

Entry 4 (room 38, idle in a dialog) has no paused slot. Over W = 1,600 frames none of its 97 atoms changes truth, so the
wait ends static at +1,600. Its idle record is E009's: only var 100 returns. **Pass.**

## W4: cost per wait

Timing alone (`gates.sh w4`), under the heavy lock, at load 7.4-7.7 (the lock keeps out heavy jobs, not other agents'
light ones). Each entry's wait ran 11 times: the first, then `--repeat 10`. The floor is the same number of frames
from a fork of the same entry with nothing done between them: `Engine::wait` with a callback that never stops, ten
times. The floor is what the engine's frames cost; the idle measurement that the wait replaces pays the same per frame.

| entry | frames | stops | median of 10 | max of 11 | floor, median | floor per frame | waits over 0.3 s |
|---|---|---|---|---|---|---|---|
| 203 | 2079 | 3 | 0.314 s | 0.353 s | 0.310 s | 149 µs | 9 of 11 |
| 205 | 2016 | 3 | 0.227 s | 0.328 s | 0.207 s | 103 µs | 1 |
| 225 | 1929 | 3 | 0.214 s | 0.272 s | 0.184 s | 96 µs | 0 |
| 204 | 2064 | 6 | 0.229 s | 0.247 s | 0.202 s | 98 µs | 0 |
| 206 | 2015 | 3 | 0.219 s | 0.233 s | 0.190 s | 94 µs | 0 |
| 207 | 2015 | 3 | 0.219 s | 0.316 s | 0.207 s | 103 µs | 2 |
| 208 | 2063 | 5 | 0.220 s | 0.259 s | 0.192 s | 93 µs | 0 |
| 209 | 2037 | 4 | 0.232 s | 0.283 s | 0.198 s | 97 µs | 0 |
| 220 | 2063 | 5 | 0.219 s | 0.349 s | 0.201 s | 97 µs | 1 |
| 221 | 2037 | 4 | 0.222 s | 0.256 s | 0.185 s | 91 µs | 0 |
| 222 | 2027 | 4 | 0.218 s | 0.250 s | 0.193 s | 95 µs | 0 |
| 223 | 2015 | 3 | 0.224 s | 0.230 s | 0.196 s | 97 µs | 0 |
| 224 | 2014 | 4 | 0.231 s | 0.248 s | 0.217 s | 108 µs | 0 |
| 4 | 1600 | 0 | 0.115 s | 0.133 s | 0.115 s | 72 µs | 0 |

**Summary.** Over the 154 waits: median 0.221 s, p90 0.289 s, max 0.353 s.

- 13 of 154 are over 0.3 s, 9 of them at entry 203, where the floor alone took 0.310 s at the time.
- The wait's own work (the view of T, the truths, the idle rule, the stops' dumps and checkpoints) is about the
  difference from the floor: 0.00-0.04 s between the medians. The two are timed one after the other, not interleaved.
- The cost is the engine's frames: one Step round trip per frame, 91-149 µs per frame at this load.

**In the try,** the wait replaces the idle measurement of 1,600 frames and runs those same frames. So what it adds to a
try is its frames past 1,600, plus its own work:

- room-28 entries: 329-479 frames, about 0.03-0.07 s at the floor's rate;
- entry 4: none.

The fork's engine CPU was 50-91 µs per frame (cpu_s in `w4-*.json`). The wall was 1.3-1.6 times that.

**Again at load 1.6-2.2** (`w4b-*.json`): the same run, 30 minutes later, without the lock (the check is under 1 GB).

| entry | frames | stops | median of 10 | max of 11 | floor, median | floor per frame | waits over 0.3 s |
|---|---|---|---|---|---|---|---|
| 203 | 2079 | 3 | 0.148 s | 0.180 s | 0.140 s | 67 µs | 0 |
| 205 | 2016 | 3 | 0.133 s | 0.159 s | 0.127 s | 63 µs | 0 |
| 225 | 1929 | 3 | 0.121 s | 0.156 s | 0.104 s | 54 µs | 0 |
| 204 | 2064 | 6 | 0.143 s | 0.151 s | 0.128 s | 62 µs | 0 |
| 206 | 2015 | 3 | 0.150 s | 0.170 s | 0.120 s | 60 µs | 0 |
| 207 | 2015 | 3 | 0.141 s | 0.153 s | 0.101 s | 50 µs | 0 |
| 208 | 2063 | 5 | 0.129 s | 0.149 s | 0.152 s | 74 µs | 0 |
| 209 | 2037 | 4 | 0.165 s | 0.241 s | 0.134 s | 66 µs | 0 |
| 220 | 2063 | 5 | 0.173 s | 0.203 s | 0.112 s | 54 µs | 0 |
| 221 | 2037 | 4 | 0.152 s | 0.179 s | 0.154 s | 76 µs | 0 |
| 222 | 2027 | 4 | 0.131 s | 0.161 s | 0.121 s | 60 µs | 0 |
| 223 | 2015 | 3 | 0.129 s | 0.159 s | 0.147 s | 73 µs | 0 |
| 224 | 2014 | 4 | 0.131 s | 0.165 s | 0.103 s | 51 µs | 0 |
| 4 | 1600 | 0 | 0.076 s | 0.107 s | 0.086 s | 54 µs | 0 |

Over the 154 waits: median 0.134 s, p90 0.168 s, max 0.241 s, 0 over 0.3 s. The floor, 50-76 µs per frame, is the rate of
round 2's idle measurement in B3 (about 62 µs per frame at load 1.9-5.1).

The medians differ from the floor's by -0.02 to +0.06 s.

**W4.**

- At load 1.6-2.2 it holds: 154 of 154 waits are at most 0.3 s (max 0.241 s).
- At load 7.4-7.7, 13 of 154 are over (max 0.353 s). 9 of them are at entry 203, where the engine's frames alone took
  0.310 s by median.
- The load sets W4, through the engine's per-frame Step. The wait's own work on top is a few hundredths of a second.

The checked runs of the other groups (`room28`, `room28b`, `e4`; view check on) took 0.32-0.77 s for the first wait. Their
five repeats without checks took 0.15-0.29 s by median, at load 5.8-6.8.

## G0

This branch changes no existing code path, so G0 holds by construction:

- fmi-core gains a provided `Engine::wait` that nothing existing calls, plus `due`, `delay_step` and three constants;
- fmi-explore gains the modules `wait` and `waitcheck`;
- fmi gains the subcommand `check wait`.

The switch `FMI_WAIT=1` is read by the try, on e012-try, and by `wait::on()`.
