#!/usr/bin/env python3
"""compress.py MOVIE.jsonl OUT_DIR [WORKERS]: shorten a movie that ends the game, judged by the engine alone.

Every test replays the whole movie from boot (seed 1, e016 inlog, ~1.2 s) and reads the engine's end line. A movie is
accepted when the game ends at the same site (the end's reason) at a frame no later than the best so far. In order,
each click is first dropped (kept dropped when accepted), else moved with every later click as early as it can go:
K candidate frames between the previous click + 2 and its frame tested in parallel, the earliest accepted taken, then
the gap below it refined once. The result is written as OUT_DIR/best.jsonl with OUT_DIR/log.jsonl (one line a step)."""
import json, os, shutil, subprocess, sys, threading, time
from concurrent.futures import ThreadPoolExecutor

MOVIE, OUT = sys.argv[1], sys.argv[2]
W = int(sys.argv[3]) if len(sys.argv) > 3 else 8
E016 = os.environ.get('E016', '/var/tmp/fast-mi/target-e019/release/e016')
os.makedirs(OUT, exist_ok=True)
TMP = os.path.join(OUT, 'tmp')
os.makedirs(TMP, exist_ok=True)
SPACING = 2
counter = [0]
lock = threading.Lock()


def run(mv, limit):
    """The end (frame, reason) of movie mv from boot, or None."""
    with lock:
        counter[0] += 1
        n = counter[0]
    d = os.path.join(TMP, f't{n}')
    f = d + '.jsonl'
    with open(f, 'w') as h:
        for fr, x, y in mv:
            h.write(json.dumps({'frame': fr, 'click': [x, y]}) + '\n')
    subprocess.run([E016, 'inlog', d, '--events', f, '--from', '100', '--to', str(limit + 600)],
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=600)
    end = None
    try:
        for l in open(os.path.join(d, 'boot', 'trace.jsonl')):
            if '"type":"end"' in l or '"type": "end"' in l:
                j = json.loads(l)
                end = (j['frame'], j['reason'])
    except FileNotFoundError:
        pass
    shutil.rmtree(d, ignore_errors=True)
    os.remove(f)
    return end


mv = []
for l in open(MOVIE):
    j = json.loads(l)
    mv.append((j['frame'], j['click'][0], j['click'][1]))
mv.sort()
end0 = run(mv, mv[-1][0] + 20000)
assert end0, 'the movie does not end the game'
best, reason = end0
log = open(os.path.join(OUT, 'log.jsonl'), 'w')
log.write(json.dumps({'start': {'clicks': len(mv), 'end': best, 'reason': reason}}) + '\n')
log.flush()
print('start', len(mv), 'clicks, end', best, reason, flush=True)
pool = ThreadPoolExecutor(W)
t0 = time.time()


def accept(m):
    e = run(m, best)
    return e[0] if e and e[1] == reason and e[0] <= best else None


def shifted(m, i, t):
    d = t - m[i][0]
    return m[:i] + [(fr + d, x, y) for fr, x, y in m[i:]]


i = 0
while i < len(mv):
    # drop
    r = accept(mv[:i] + mv[i + 1:])
    if r is not None:
        mv = mv[:i] + mv[i + 1:]
        log.write(json.dumps({'i': i, 'drop': True, 'end': r}) + '\n'); log.flush()
        best = r
        continue
    lo = (mv[i - 1][0] + SPACING) if i > 0 else 1
    hi = mv[i][0]
    if lo < hi:
        cands = sorted(set(lo + (hi - lo) * k // W for k in range(W)))
        res = list(pool.map(lambda t: (t, accept(shifted(mv, i, t))), cands))
        ok = [(t, r) for t, r in res if r is not None]
        if ok:
            t, r = min(ok)
            below = max([c for c in cands if c < t], default=None)
            if below is not None and t - below > 1:
                c2 = sorted(set(below + 1 + (t - below - 1) * k // W for k in range(W)))
                res2 = list(pool.map(lambda u: (u, accept(shifted(mv, i, u))), c2))
                ok2 = [(u, r2) for u, r2 in res2 if r2 is not None]
                if ok2:
                    t, r = min(ok2)
            if t < hi:
                mv = shifted(mv, i, t)
                best = r
                log.write(json.dumps({'i': i, 'moved': hi - t, 'end': r}) + '\n'); log.flush()
    i += 1
    if i % 10 == 0:
        print(f'{i}/{len(mv)} clicks, end {best}, {time.time() - t0:.0f} s, {counter[0]} replays', flush=True)

final = run(mv, best)
with open(os.path.join(OUT, 'best.jsonl'), 'w') as h:
    for fr, x, y in mv:
        h.write(json.dumps({'frame': fr, 'click': [x, y]}) + '\n')
log.write(json.dumps({'final': {'clicks': len(mv), 'end': final, 'from': end0}}) + '\n')
print('final', len(mv), 'clicks, end', final, 'from', end0, f'{time.time() - t0:.0f} s, {counter[0]} replays', flush=True)
