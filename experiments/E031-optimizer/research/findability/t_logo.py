import json, sys, time
from gf import *
s = Session('logo', first_P=0, skips=False, start_frame=None)
res = {'root': brief(s.root.status)}
print('root', res['root'], flush=True)
ref = load_movie()
# (a) single Esc at frame f
rows = []
for f in list(range(2, 131)) + [150, 200, 400, 800, 950]:
    r = s.trial([{'frame': f, 'key': 'esc'}], [dict(frame=f + 4), dict(frame=f + 60)], tag='esc%d' % f)
    rows.append({'f': f, 'st': [brief(x) for x in r['st']], 'err': r.get('error')})
res['esc_scan'] = rows
print('scan done', time.time() - s.t0, flush=True)
# (b) other inputs at frame 60 and 70
other = []
for f in (60, 70):
    for ev in ({'key': '.'}, {'key': 'space'}, {'key': 'enter'}, {'key': 'a'}, {'key': '1'}, {'click': [160, 100]}, {'rclick': [160, 100]}, {'move': [10, 10]}):
        r = s.trial([dict(ev, frame=f)], [dict(frame=f + 4), dict(frame=f + 60)], tag='other%d%s' % (f, json.dumps(ev)), timeout=20)
        other.append({'f': f, 'ev': ev, 'st': [brief(x) for x in r['st']], 'err': r.get('error')})
res['other'] = other
# no input at all: when does the logo end by itself, v19 then
r = s.trial([], [dict(frame=44), dict(frame=45), dict(frame=46), dict(frame=92), dict(frame=93), dict(frame=94), dict(frame=1100)], tag='noinput')
res['noinput'] = [brief(x) for x in r['st']]
# (c) pair on the first stretch: reference events to the bar (room 28), and the same shifted +47 (Esc at 93: v19 6)
head = [e for e in ref if e['frame'] <= 203]
A = s.trial(head, [dict(until=[{'room': 28}], max_frame=500)])
B = s.trial([dict(e, frame=e['frame'] + 47) for e in head], [dict(until=[{'room': 28}], max_frame=500)])
res['pair'] = {'A': [brief(x) for x in A['st']], 'B': [brief(x) for x in B['st']]}
# also to first control on the dock (room 33) for both
# (d) signal: same relative frame after the Esc, inside (92) and outside (93) the window
a = s.trial([{'frame': 92, 'key': 'esc'}, {'frame': 95, 'key': 'esc'}, {'frame': 97, 'key': 'esc'}], [dict(frame=92 + 20)], want_state=[0])
b = s.trial([{'frame': 93, 'key': 'esc'}, {'frame': 96, 'key': 'esc'}, {'frame': 98, 'key': 'esc'}], [dict(frame=93 + 20)], want_state=[0])
res['signal'] = {'with': brief(a['st'][0]), 'without': brief(b['st'][0]), 'diff': sdiff(core(b['state'][0]), core(a['state'][0]))}
res['wall'] = time.time() - s.t0; res['trials'] = s.trials
s.close()
json.dump(res, open(HERE / 'out' / 'logo.json', 'w'), indent=0)
print('done', res['wall'], res['trials'])
