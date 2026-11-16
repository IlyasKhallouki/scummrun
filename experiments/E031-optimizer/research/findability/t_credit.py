import json, time
from gf import *
s = Session('credit', first_P=2108)
ev = s.ev
res = {'root': brief(s.root.status)}
# (A) delay of the store entry (everything from the door-open key on)
rows = []
for d in range(-3, 58):
    def fn(c, d=d):
        o = {'d': d}
        st = c.run(until=[{"room": 30}], max_frame=2175 + d + 60)
        o['enter'] = brief(st)
        if st.get('stop') != 'until': return o
        st = c.run(frame=st['frame'] + 2)
        v = c.state()['vars']
        o['digits'] = v[221:225]; o['keeper'] = v[100]
        st = c.run(until=[{"bit": 320, "eq": 1}], max_frame=2283 + d + 60)
        o['safe'] = brief(st)
        return o
    rows.append(s.do(delay_from(ev, 349, d), fn, tag='entry%d' % d))
res['entry'] = rows
print('A', time.time() - s.t0, flush=True)
# (B) handle sentences: delay sentence k and all after by d; or move sentence k alone earlier
pairs = [(354, 355), (356, 357), (358, 359), (360, 361)]
rows = []
def safe(c, d=0):
    return {'safe': brief(c.run(until=[{"bit": 320, "eq": 1}], max_frame=2283 + max(d, 0) + 60))}
for k, pr in enumerate(pairs):
    for d in (1, 2, 3, 4, 6, 8, 12, 20, 30):
        r = s.do(delay_from(ev, pr[0], d), lambda c, d=d: safe(c, d), tag='hdelay%d_%d' % (k, d)); r.update(k=k, d=d, kind='delay_rest'); rows.append(r)
    for d in (-1, -2, -3, -4, -6):
        r = s.do(shift_idx(ev, pr, d), lambda c: safe(c), tag='hearly%d_%d' % (k, d)); r.update(k=k, d=d, kind='early_alone'); rows.append(r)
    for d in (1, 2, 3, 4, 6):
        r = s.do(shift_idx(ev, pr, d), lambda c: safe(c), tag='hlate%d_%d' % (k, d)); r.update(k=k, d=d, kind='late_alone'); rows.append(r)
res['handle'] = rows
# (C) scan size at control in the store; (D) signal
def fn(c):
    c.run(frame=2244)
    st = c.state()
    o = {'room_objects': len(st['room_objects']), 'inventory': len(st['inventory']), 'verbs': len(st['verbs']),
         'verbs_list': st['verbs'][:40], 'room_objects_sample': st['room_objects'][:3]}
    stx = c.run(frame=2283)
    o['with'] = brief(stx); o['state'] = core(c.state())
    return o
w = s.do(ev, fn, tag='with')
def fn2(c):
    stx = c.run(frame=2283)
    return {'without': brief(stx), 'state': core(c.state())}
wo = s.do(drop_idx(ev, range(354, 362)), fn2, tag='without')
res['scan'] = {k: v for k, v in w.items() if k != 'state'}
res['signal'] = {'with': w.get('with'), 'without': wo.get('without'), 'diff': sdiff(wo['state'], w['state'])}
res['wall'] = time.time() - s.t0; res['trials'] = s.trials
s.close()
json.dump(res, open(HERE / 'out' / 'credit.json', 'w'))
print('done', res['wall'], res['trials'])
