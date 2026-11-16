import json, time
from gf import *
s = Session('carry', first_P=8990)
ev = s.ev
res = {'root': brief(s.root.status)}
SAMPLES = (9362, 9420)
def samp(c):
    o = {'st': []}
    for f in SAMPLES:
        st = c.run(frame=f)
        if 'end' in st:
            o['end'] = str(st['end'])[:120]; break
        o['st'].append({k: st.get(k) for k in ('frame', 'room', 'up', 'cs')})
    return o
res['base'] = s.do(ev, samp, tag='base')
rows = []
for i in range(1004, 1089):
    for kind, e2 in (('-1', shift_idx(ev, (i,), -1)), ('+1', shift_idx(ev, (i,), 1)), ('drop', drop_idx(ev, (i,)))):
        r = s.do(e2, samp, tag='%d%s' % (i, kind)); r.update(i=i, kind=kind, frame=ev[i]['frame'], ev={k: v for k, v in ev[i].items() if k != 'frame'})
        rows.append(r)
    if i % 10 == 0:
        print(i, time.time() - s.t0, flush=True)
        json.dump({'rows': rows}, open(HERE / 'out' / 'carry.part.json', 'w'))
res['rows'] = rows
res['wall'] = time.time() - s.t0; res['trials'] = s.trials
s.close()
json.dump(res, open(HERE / 'out' / 'carry.json', 'w'))
print('done', res['wall'], res['trials'])
