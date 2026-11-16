import json, time
from gf import *
s = Session('fight2', first_P=4425)
ev = s.ev
res = {'root': brief(s.root.status)}
def fg(c, d):
    st = c.run(until=[{"room": 85}], max_frame=4641 + max(d, 0) + 120)
    if 'end' in st: return {'end': 1}
    return {'b': {k: st.get(k) for k in ('stop', 'frame', 'rng', 'room')}}
res['base'] = s.do(ev, lambda c: fg(c, 0), tag='base')
T = {}
for i, lo in ((485, -3), (486, -10), (487, -5), (488, -1), (489, 0)):
    T[str(i)] = [dict(s.do(delay_from(ev, i, d), lambda c, d=d: fg(c, d), tag='f%d_%d' % (i, d)), d=d) for d in range(lo, 31)]
    print(i, time.time() - s.t0, flush=True)
res['fight1'] = T
res['wall'] = time.time() - s.t0; res['trials'] = s.trials
s.close(); json.dump(res, open(HERE / 'out' / 'fight2.json', 'w'))
print('done', res['wall'], res['trials'])
