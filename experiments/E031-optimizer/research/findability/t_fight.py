import json, time
from gf import *
s = Session('fight', first_P=4400)
ev = s.ev
res = {}
def save(): json.dump(res, open(HERE / 'out' / 'fight.json', 'w'))
def fg(c, d):
    st = c.run(until=[{"room": 85}], max_frame=4641 + max(d, 0) + 80)
    if 'end' in st: return {'end': 1}
    return {'b': {k: st.get(k) for k in ('stop', 'frame', 'rng')}}
res['base'] = s.do(ev, lambda c: fg(c, 0), tag='base')
T = {}
for i, lo in ((485, -3), (486, -10), (487, -5), (488, -1), (489, 0)):
    T[str(i)] = [dict(s.do(delay_from(ev, i, d), lambda c, d=d: fg(c, d), tag='f%d_%d' % (i, d)), d=d) for d in range(lo, 31)]
    print(i, time.time() - s.t0, flush=True)
res['fight1'] = T
# rng per frame while a menu waits: no input after the menu at frame 4438
def rngs(c):
    out = []
    for f in range(4439, 4470):
        st = c.run(frame=f); out.append(st.get('rng'))
    return {'rng': out}
res['menu_rng'] = s.do([e for e in ev if e['frame'] < 4442], rngs, tag='menurng')
save()
# ---- the book that carries the slide across the head dialogue
s.advance(12345)
def bk(c):
    o = {}
    st = c.run(frame=12560)
    if 'end' in st: return {'end': str(st['end'])[:100]}
    o['a'] = {k: st.get(k) for k in ('room', 'up', 'cs')}
    st = c.run(until=[{"room": 77}], max_frame=12800)
    if 'end' in st: o['end'] = str(st['end'])[:100]; return o
    o['b'] = {k: st.get(k) for k in ('stop', 'frame', 'up', 'cs')}
    return o
res['bk_base'] = s.do(ev, bk, tag='bkbase')
B = {}
B['look1632'] = [dict(s.do(shift_idx(ev, (1632,), d), bk, tag='bl%d' % d), d=d) for d in range(-3, 15)]
B['book1634'] = [dict(s.do(shift_idx(ev, (1634,), d), bk, tag='bb%d' % d), d=d) for d in range(-12, 21)]
B['talk1635_6'] = [dict(s.do(shift_idx(ev, (1635, 1636), d), bk, tag='bt%d' % d), d=d) for d in range(-2, 21)]
B['esc1642'] = [dict(s.do(shift_idx(ev, (1642,), d), bk, tag='be%d' % d), d=d) for d in range(-15, 16)]
B['no_book'] = [s.do(drop_idx(ev, (1634,)), bk, tag='nobook')]
res['book'] = B
res['wall'] = time.time() - s.t0; res['trials'] = s.trials
s.close(); save()
print('done', res['wall'], res['trials'])
