import json, time
from gf import *
s = Session('slide', first_P=2108)
ev = s.ev
res = {}
# ---- (i) store entry: keeper away per delay, and a generic opening (the right count of handle sentences)
rows = []
for d in range(-3, 58):
    def fn(c, d=d):
        o = {'d': d}
        st = c.run(until=[{"room": 30}], max_frame=2175 + d + 60)
        if st.get('stop') != 'until': return o
        st = c.run(frame=st['frame'] + 2)
        o['away'] = c.holds([{"actor_room": 11, "eq": 0}])
        o['digits'] = c.state()['vars'][221:225]
        if o['away']:
            t = 2245 + d
            more = []
            for i, n in enumerate(o['digits']):
                for _ in range(n):
                    more += [{'frame': t, 'key': 'y' if i % 2 == 0 else 's'}, {'frame': t + 1, 'click': [187, 67]}]
                    t += 11
            c.inputs(more)
            o['n_sent'] = sum(o['digits'])
            o['safe'] = brief(c.run(until=[{"bit": 320, "eq": 1}], max_frame=t + 60))
            o['t_first'] = 2245 + d
        return o
    rows.append(s.do(delay_from(ev[:354], 349, d), fn, tag='entry%d' % d))
res['entry'] = rows
print('entry', time.time() - s.t0, flush=True)
json.dump(res, open(HERE / 'out' / 'slide.json', 'w'))
# ---- (ii) the slide setup in the store
s.advance(8825)
print('at 8825', brief(s.root.status), time.time() - s.t0, flush=True)
SAMPLES = (8948, 8956, 8964, 8975, 9000)
def samp(c, want=None):
    o = {'st': []}
    for f in SAMPLES:
        st = c.run(frame=f)
        o['st'].append({k: st.get(k) for k in ('frame', 'room', 'up', 'cs', 'cur')})
        if 'end' in st: break
        if want == f: o['state'] = core(c.state())
    return o
base = s.do(ev, lambda c: samp(c, 8964), tag='base'); res['base'] = {k: v for k, v in base.items() if k != 'state'}
T = {}
T['pair'] = [dict(s.do(shift_idx(ev, (987, 988), d), samp, tag='pair%d' % d), d=d) for d in range(-30, 31)]
T['esc'] = [dict(s.do(shift_idx(ev, (989,), d), samp, tag='esc%d' % d), d=d) for d in range(-30, 31)]
T['esc_drop'] = [s.do(drop_idx(ev, (989,)), samp, tag='escdrop')]
T['floor'] = [dict(s.do(shift_idx(ev, (986,), d), samp, tag='floor%d' % d), d=d) for d in range(-30, 31)]
T['all_delay'] = [dict(s.do(delay_from(ev, 986, d), samp, tag='all%d' % d), d=d) for d in (1, 2, 3, 5, 8, 12, 20, 30)]
T['skips_on'] = [s.do(drop_idx(ev, (984,)), lambda c: samp(c, 8964), tag='skipson')]
nofloor = s.do(drop_idx(ev, (986,)), lambda c: samp(c, 8964), tag='nofloor')
T['no_floor'] = [{k: v for k, v in nofloor.items() if k != 'state'}]
print('timing', time.time() - s.t0, flush=True)
rows = []
for x in range(236, 320, 12):
    for y in (112, 120, 128, 135, 141, 146):
        e2 = [dict(e) for e in ev]; e2[986]['click'] = [x, y]
        rows.append(dict(s.do(e2, samp, tag='xy%d_%d' % (x, y)), x=x, y=y))
T['floor_xy'] = rows
rows = []
for key in 'gocpltusy':
    for (sx, sy) in [(164, 155), (204, 155), (244, 155), (284, 155), (164, 179), (204, 179), (244, 179), (284, 179)]:
        e2 = [dict(e) for e in ev]; e2[987]['key'] = key; e2[988]['click'] = [sx, sy]
        rows.append(dict(s.do(e2, samp, tag='vs%s_%d_%d' % (key, sx, sy)), key=key, slot=[sx, sy]))
T['verb_slot'] = rows
res['T'] = T
for k in T:
    for r in T[k]: r.pop('state', None)
res['signal_skipson'] = sdiff(T and s.do(drop_idx(ev, (984,)), lambda c: samp(c, 8964))['state'], base['state'])
res['signal_nofloor'] = sdiff(nofloor['state'] if 'state' in nofloor else s.do(drop_idx(ev, (986,)), lambda c: samp(c, 8964))['state'], base['state'])
st = s.root.state()
res['scan'] = {'room_objects': len(st['room_objects']), 'inventory': len(st['inventory'])}
res['wall'] = time.time() - s.t0; res['trials'] = s.trials
s.close()
json.dump(res, open(HERE / 'out' / 'slide.json', 'w'))
print('done', res['wall'], res['trials'])
