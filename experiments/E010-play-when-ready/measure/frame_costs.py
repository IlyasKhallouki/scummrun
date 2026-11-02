"""E010 baseline: what a frame costs the window search's branches, from the boot (seed 1), on this laptop.
(a) the parent's frame as window_strict steps it without fork_each: run one frame, runs(), explain_log(), state();
(b) the same with fork_each 7 (a child per frame, its runs, explain log and state read, then quit);
(c) a bare frame with no observation, one command per frame, and 50 frames in one command;
(d) a fork and its quit; a fork, one state dump, quit (an interface child that raises at once)."""
import sys, time
from pathlib import Path
sys.path.insert(0, "/var/tmp/fast-mi/wt-e010/experiments/E003-path-through-cloud")
sys.path.insert(0, "/var/tmp/fast-mi/wt-e010/src")
import replay_part1 as P
from facts import facts_of
out = Path(sys.argv[1])
N = int(sys.argv[2]) if len(sys.argv) > 2 else 200
ctl, b, row = P.boot(1, out / "boot", dump_locals=True)
res = {}
try:
    p = ctl.fork(b)
    p.segment(True); p.runs()
    p.explain(p.frame + 1, p.frame + 100000, None); p.explain_log()
    t = time.monotonic()
    for i in range(N):
        p.run(frame=p.frame + 1); p.runs(); p.explain_log(); facts_of(p.state())
    res["a parent frame, observed (ms)"] = 1000 * (time.monotonic() - t) / N
    t = time.monotonic(); n = 0
    while n < N:
        st, kids = ctl.fork_each(p, 7)
        p.runs(); p.explain_log()
        for c in kids:
            c.runs(); c.explain_log(); facts_of(c.state()); c.quit(); n += 1
    res["b parent frame, fork_each 7 (ms)"] = 1000 * (time.monotonic() - t) / n
    p.quit()
    q = ctl.fork(b)
    t = time.monotonic()
    for i in range(N):
        q.run(frame=q.frame + 1)
    res["c bare frame (ms)"] = 1000 * (time.monotonic() - t) / N
    t = time.monotonic()
    for i in range(N):
        q.run(frame=q.frame + 1); q.state()
    res["c frame + state dump (ms)"] = 1000 * (time.monotonic() - t) / N
    t = time.monotonic()
    for i in range(N // 50):
        q.run(frame=q.frame + 50)
    res["c frame in 50-frame commands (ms)"] = 1000 * (time.monotonic() - t) / (N // 50 * 50)
    t = time.monotonic()
    for i in range(20):
        c = ctl.fork(q); c.quit()
    res["d fork + quit (ms)"] = 1000 * (time.monotonic() - t) / 20
    t = time.monotonic()
    for i in range(20):
        c = ctl.fork(q); c.state(); c.quit()
    res["d fork + state + quit (ms)"] = 1000 * (time.monotonic() - t) / 20
    q.quit()
finally:
    ctl.close()
for k, v in res.items():
    print(f"{k}: {v:.1f}")
