import json, time
from gf import *
s = Session('oars', first_P=8990)
ev = s.ev
res = {}
# ---- carry: delay everything from event i on by d
def samp(c, d):
    o = {'st': []}
    for f in (9362 + d, 9420 + d):
        st = c.run(frame=f)
        if 'end' in st:
            o['end'] = str(st['end'])[:100]; break
        o['st'].append({k: st.get(k) for k in ('frame', 'room', 'up', 'cs')})
    return o
rows = []
for i in range(1004, 1089):
    for d in (1, 3):
        r = s.do(delay_from(ev, i, d), lambda c, d=d: samp(c, d), tag='cd%d_%d' % (i, d)); r.update(i=i, d=d); rows.append(r)
res['carry_delay'] = rows
print('carry', time.time() - s.t0, flush=True)
json.dump(res, open(HERE / 'out' / 'oars.json', 'w'))
# ---- oars skip
s.advance(10390)
print('at 10390', brief(s.root.status), flush=True)
def oars(c, want=None):
    o = {}
    st = c.run(frame=10469)
    o['a'] = {k: st.get(k) for k in ('frame', 'room', 'up', 'cs', 'v19', 'ego')}
    if want: o['state_a'] = core(c.state())
    st = c.run(until=[{"room": 131}], max_frame=10660)
    if 'end' in st: o['end'] = str(st['end'])[:100]; return o
    o['b'] = {k: st.get(k) for k in ('stop', 'frame', 'room', 'up', 'cs', 'ego')}
    return o
base = s.do(ev, lambda c: oars(c, True), tag='base'); res['oars_base'] = {k: v for k, v in base.items() if k != 'state_a'}
T = {}
for i in (1233, 1234, 1235, 1236):
    T['shift%d' % i] = [dict(s.do(shift_idx(ev, (i,), d), oars, tag='o%d_%d' % (i, d)), d=d) for d in range(-30, 31)]
    print(i, time.time() - s.t0, flush=True)
T['delay_from1233'] = [dict(s.do(delay_from(ev, 1233, d), oars, tag='od%d' % d), d=d) for d in range(1, 31)]
T['delay_from1230'] = [dict(s.do(delay_from(ev, 1230, d), oars, tag='ot%d' % d), d=d) for d in (1, 2, 3, 5, 8, 13)]
res['oars'] = T
wo = s.do(drop_idx(ev, (1233, 1234, 1235, 1236)), lambda c: oars(c, True), tag='without')
res['oars_without'] = {k: v for k, v in wo.items() if k != 'state_a'}
res['oars_signal'] = sdiff(wo['state_a'], base['state_a'])
st = s.root.state()
res['scan'] = {'room_objects': len(st['room_objects']), 'inventory': len(st['inventory'])}
res['wall'] = time.time() - s.t0; res['trials'] = s.trials
s.close()
json.dump(res, open(HERE / 'out' / 'oars.json', 'w'))
print('done', res['wall'], res['trials'])
