import pickle,collections
from load import HZ
d=pickle.load(open('seg.pkl','rb')); res=d['res']; SEGN=d['SEGN']
TS=4/HZ
CL=['wait','walk','cut','act']
print('class seconds (ticks*4/HZ): wait walk cut act | other(qf outside main loop)')
for k in ('full5','slide','noslide'):
    print('==',k)
    tot=collections.Counter()
    for i,s in enumerate(res[k]):
        if s is None: continue
        c=[s['cls'].get(x,0)*TS for x in CL]
        oth=s['sec']-s['ticks']*TS
        for x,v in zip(CL,c): tot[x]+=v
        tot['oth']+=oth; tot['sec']+=s['sec']
        print(f"S{i:02d} {s['sec']:8.1f} | "+' '.join(f'{v:8.1f}' for v in c)+f" | {oth:5.2f} | frames {s['frames']:6d} vis {s['nvis']:3d} rooms {s['rooms']:2d} clicks {s['clicks']:4d} keys {s['keys']:3d} tskip {s['tskips']:4d} cskip {s['cskips']:3d} objs {s['objs']:2d}"+(f" stops {s['stops']}" if 'stops' in s else ''))
    print('TOT',' '.join(f'{x}={v:.1f}' for x,v in tot.items()))
