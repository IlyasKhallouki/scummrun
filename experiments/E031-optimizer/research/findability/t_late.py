import json, time
from gf import *
s = Session('late', first_P=11150)
ev = s.ev
res = {}
def save(): json.dump(res, open(HERE / 'out' / 'late.json', 'w'))
# ---- regain of the slide at the village
def rg(c, d=0, want=False):
    o = {'st': []}
    for f in (11445 + d, 11475 + d):
        st = c.run(frame=f)
        if 'end' in st: o['end'] = str(st['end'])[:100]; break
        o['st'].append({k: st.get(k) for k in ('frame', 'room', 'up', 'cs')})
        if want and f == 11445 + d: o['state'] = core(c.state())
    return o
base = s.do(ev, lambda c: rg(c, 0, True), tag='base'); res['rg_base'] = {k: v for k, v in base.items() if k != 'state'}
T = {}
T['read_shift'] = [dict(s.do(shift_idx(ev, (1414, 1415), d), rg, tag='rs%d' % d), d=d) for d in range(-20, 31)]
T['read_delay_rest'] = [dict(s.do(delay_from(ev, 1414, d), lambda c, d=d: rg(c, d), tag='rd%d' % d), d=d) for d in range(1, 21)]
rows = []
for i in [1420] + list(range(1422, 1430)) + list(range(1438, 1454)):
    for kind, e2 in (('-1', shift_idx(ev, (i,), -1)), ('+1', shift_idx(ev, (i,), 1)), ('drop', drop_idx(ev, (i,)))):
        rows.append(dict(s.do(e2, rg, tag='dot%d%s' % (i, kind)), i=i, kind=kind))
T['dots'] = rows
e2 = [dict(e) for e in ev]; e2[1413]['skips'] = {'text': 1, 'cutscenes': 1}
T['no_spare'] = [s.do(e2, rg, tag='nospare')]
wo = s.do(drop_idx(ev, (1414, 1415)), lambda c: rg(c, 0, True), tag='noread')
T['no_read'] = [{k: v for k, v in wo.items() if k != 'state'}]
res['rg'] = T
res['rg_signal'] = sdiff(wo['state'], base['state']) if 'state' in wo and 'state' in base else None
print('regain', time.time() - s.t0, flush=True); save()
# ---- catacombs: delay before an exit click
s.advance(12265)
print('at 12265', brief(s.root.status), flush=True)
def cat(c, d, fv):
    o = {}
    st = c.run(frame=fv + d)
    o['a'] = {k: st.get(k) for k in ('frame', 'room', 'rng')}
    o['vars'] = c.state()['vars']
    st = c.run(until=[{"room": 70}], max_frame=12353 + d + 40)
    if 'end' in st: o['end'] = 1; return o
    o['b'] = {k: st.get(k) for k in ('stop', 'frame')}
    return o
res['cat1'] = [dict(s.do(delay_from(ev, 1623, d), lambda c, d=d: cat(c, d, 12288), tag='c1_%d' % d), d=d) for d in range(0, 31)]
res['cat2'] = [dict(s.do(delay_from(ev, 1624, d), lambda c, d=d: cat(c, d, 12298), tag='c2_%d' % d), d=d) for d in range(0, 31)]
print('cat', time.time() - s.t0, flush=True); save()
# ---- head variant and necklace skip: where ego stands, and the frame of the head sentence
s.advance(12345)
def hd(c):
    st = c.run(until=[{"room": 77}], max_frame=12800)
    if 'end' in st: return {'end': str(st['end'])[:100]}
    return {'b': {k: st.get(k) for k in ('stop', 'frame', 'up', 'cs')}}
res['hd_base'] = s.do(ev, hd, tag='hdbase')
rows = []
for x in range(40, 71, 3):
    for y in range(77, 90, 2):
        e2 = [dict(e) for e in ev]; e2[1631]['click'] = [x, y]
        rows.append(dict(s.do(e2, hd, tag='hx%d_%d' % (x, y)), x=x, y=y))
res['hd_grid'] = rows
res['hd_headshift'] = [dict(s.do(shift_idx(ev, (1629, 1630), d), hd, tag='hh%d' % d), d=d) for d in range(-3, 4)]
res['hd_delay_rest'] = [dict(s.do(delay_from(ev, 1629, d), hd, tag='hr%d' % d), d=d) for d in range(1, 9)]
print('head', time.time() - s.t0, flush=True); save()
# ---- feather skip: the click inside the rat cutscene
s.advance(12760)
def ft(c, want=False):
    o = {}
    if want:
        st = c.run(frame=12830); o['a'] = brief(st); o['state_a'] = core(c.state())
    st = c.run(frame=12861)
    if 'end' in st: return {'end': str(st['end'])[:100]}
    o['b'] = {k: st.get(k) for k in ('frame', 'room', 'up', 'cs')}
    o['inv'] = c.state()['inventory']
    return o
base = s.do(ev, lambda c: ft(c, True), tag='ftbase'); res['ft_base'] = {k: v for k, v in base.items() if k != 'state_a'}
T = {}
T['click1675'] = [dict(s.do(shift_idx(ev, (1675,), d), ft, tag='fc%d' % d), d=d) for d in range(-30, 31)]
T['key1674'] = [dict(s.do(shift_idx(ev, (1674,), d), ft, tag='fk%d' % d), d=d) for d in range(-30, 31)]
rows = []
for x in range(236, 320, 8):
    for y in range(88, 136, 8):
        e2 = [dict(e) for e in ev]; e2[1675]['click'] = [x, y]
        rows.append(dict(s.do(e2, ft, tag='fx%d_%d' % (x, y)), x=x, y=y))
T['grid1675'] = rows
wo = s.do(drop_idx(ev, (1675,)), lambda c: ft(c, True), tag='ftwithout')
T['without'] = [{k: v for k, v in wo.items() if k != 'state_a'}]
res['ft'] = T
res['ft_signal'] = sdiff(wo['state_a'], base['state_a'])
res['wall'] = time.time() - s.t0; res['trials'] = s.trials
s.close(); save()
print('done', res['wall'], res['trials'])
