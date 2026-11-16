import json, time
from gf import *
s = Session('slide2', first_P=8825)
ev = s.ev
res = {}
head = [e for e in ev if e['frame'] <= 8917]            # up to the store entry, automatic skips off (event at 8863)
FL = {'frame': 8918, 'click': [300, 141]}; KEY = {'frame': 8925, 'key': 'l'}; SLOT = {'frame': 8926, 'click': [284, 155]}
def det(c):
    """the anomaly detector's view: any frame with input above the cutscene nesting, sampled every 3 frames"""
    hits = []; rest = False; last = None
    for f in range(8921, 9060, 3):
        st = c.run(frame=f)
        if 'end' in st: return {'end': str(st['end'])[:80], 'hits': len(hits)}
        if st['up'] + st['cs'] - 1 >= 1:
            hits.append(f)
            if st['cs'] == 0: rest = True
        last = (st['room'], st['up'], st['cs'])
    return {'hits': len(hits), 'first': hits[0] if hits else None, 'lastf': hits[-1] if hits else None, 'rest': rest, 'last': last}
res['base_noesc'] = s.do(head + [FL, KEY, SLOT], det, tag='base')
# timing of the reading relative to the floor click, no Esc, nothing after
res['pair'] = [dict(s.do(head + [FL, dict(KEY, frame=8925 + d), dict(SLOT, frame=8926 + d)], det, tag='p%d' % d), d=d) for d in range(-6, 61)]
print('pair', time.time() - s.t0, flush=True)
rows = []
for x in range(8, 320, 16):
    for y in range(6, 144, 12):
        rows.append(dict(s.do(head + [dict(FL, click=[x, y]), KEY, SLOT], det, tag='xy%d_%d' % (x, y)), x=x, y=y))
res['floor_xy'] = rows
print('xy', time.time() - s.t0, flush=True)
rows = []
for key in 'gocpltusy':
    for (sx, sy) in [(164, 155), (204, 155), (244, 155), (284, 155), (164, 179), (204, 179), (244, 179), (284, 179)]:
        rows.append(dict(s.do(head + [FL, dict(KEY, key=key), dict(SLOT, click=[sx, sy])], det, tag='vs%s%d_%d' % (key, sx, sy)), key=key, slot=[sx, sy]))
res['verb_slot'] = rows
# no floor click at all; reading only
res['no_floor'] = s.do(head + [KEY, SLOT], det, tag='nofloor')
# the floor click later, with Esc following it by the same shift (the reading fixed)
ESC = {'frame': 8946, 'key': 'esc'}
res['floor_esc'] = [dict(s.do(head + [dict(FL, frame=8918 + d), KEY, SLOT, dict(ESC, frame=8946 + d)], det, tag='fe%d' % d), d=d) for d in range(0, 31)]
res['wall'] = time.time() - s.t0; res['trials'] = s.trials
s.close(); json.dump(res, open(HERE / 'out' / 'slide2.json', 'w'))
print('done', res['wall'], res['trials'])
