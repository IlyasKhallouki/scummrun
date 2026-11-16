import json, time
from gf import *
s = Session('monkey', first_P=10390)
ev = s.ev
res = {}
def save(): json.dump(res, open(HERE / 'out' / 'monkey.json', 'w'))
# ---- oars: where on the screen do the two clicks work
def oars(c):
    o = {}
    st = c.run(frame=10469)
    o['a'] = {k: st.get(k) for k in ('room', 'up', 'cs')}
    st = c.run(until=[{"room": 131}], max_frame=10600)
    if 'end' in st: o['end'] = 1; return o
    o['b'] = {k: st.get(k) for k in ('stop', 'frame')}
    return o
rows = []
for x in range(10, 320, 20):
    for y in range(9, 144, 18):
        e2 = [dict(e) for e in ev]; e2[1233]['click'] = [x, y]
        rows.append(dict(s.do(e2, oars, tag='g1_%d_%d' % (x, y)), x=x, y=y))
res['grid1233'] = rows
rows = []
for x in range(11, 320, 24):
    for y in range(11, 200, 24):
        e2 = [dict(e) for e in ev]; e2[1234]['click'] = [x, y]
        rows.append(dict(s.do(e2, oars, tag='g2_%d_%d' % (x, y)), x=x, y=y))
res['grid1234'] = rows
print('grids', time.time() - s.t0, flush=True); save()
# ---- monkey skip
s.advance(10595)
print('at 10595', brief(s.root.status), flush=True)
def mk(c, want=None):
    o = {}
    if want:
        st = c.run(frame=want)
        o['a'] = brief(st); o['state_a'] = core(c.state())
    st = c.run(until=[{"room": 69}], max_frame=10800)
    if 'end' in st: o['end'] = str(st['end'])[:100]; return o
    o['b'] = {k: st.get(k) for k in ('stop', 'frame', 'room', 'up', 'cs')}
    return o
base = s.do(ev, lambda c: mk(c, 10670), tag='base'); res['mk_base'] = {k: v for k, v in base.items() if k != 'state_a'}
T = {}
for name, idxs in (('arrow1283', (1283,)), ('read1285_6', (1285, 1286)), ('floor1287', (1287,)), ('esc1288', (1288,)), ('click1289', (1289,))):
    T[name] = [dict(s.do(shift_idx(ev, idxs, d), mk, tag='%s_%d' % (name, d)), d=d) for d in range(-30, 31)]
    print(name, time.time() - s.t0, flush=True)
T['delay_from1283'] = [dict(s.do(delay_from(ev, 1283, d), mk, tag='md%d' % d), d=d) for d in range(1, 31)]
res['mk'] = T
wo = s.do(drop_idx(ev, (1285, 1286)), lambda c: mk(c, 10670), tag='without')
res['mk_without'] = {k: v for k, v in wo.items() if k != 'state_a'}
res['mk_signal'] = sdiff(wo['state_a'], base['state_a'])
rows = []
for x in range(200, 320, 12):
    for y in range(40, 130, 12):
        e2 = [dict(e) for e in ev]; e2[1287]['click'] = [x, y]
        rows.append(dict(s.do(e2, mk, tag='mg_%d_%d' % (x, y)), x=x, y=y))
res['mk_grid1287'] = rows
st = s.root.state()
res['scan'] = {'room_objects': len(st['room_objects']), 'inventory': len(st['inventory'])}
res['wall'] = time.time() - s.t0; res['trials'] = s.trials
s.close(); save()
print('done', res['wall'], res['trials'])
