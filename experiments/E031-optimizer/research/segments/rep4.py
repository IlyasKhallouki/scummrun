import pickle,collections,json
from load import load,HZ
d=pickle.load(open('seg.pkl','rb')); res=d['res']; B=d['B']
TS=4/HZ
D={k:load(k) for k in ('full5','slide','noslide')}
def piece(dd,f0,f1):
    c=collections.Counter(); t=dd['t']; cl=dd['cls']
    for f in range(f0,f1+1): c[cl[f]]+=t[f]-t[f-1]
    return c
# wait inside excess visits, and top detour rooms
for ref in ('slide','noslide'):
    tw=0; print('== vs',ref)
    allrooms=collections.Counter(); allvis=collections.Counter()
    for i in range(1,17):
        F=res['full5'][i]; Rr=res[ref][i]
        fv=collections.defaultdict(list)
        for r,f0,f1 in F['visits']:
            if r: fv[r].append(piece(D['full5'],f0,f1))
        rv=collections.Counter(r for r,_,_ in Rr['visits'] if r)
        w=0; top=[]
        for r,ps in fv.items():
            n=len(ps); e=max(0,n-rv.get(r,0))
            if not e: continue
            w+=sum(p['wait'] for p in ps)*e/n
            b=sum(p['walk']+p['cut']+p['act'] for p in ps)*e/n
            top.append((round(b*TS,1),r,n,rv.get(r,0)))
            allrooms[r]+=b*TS; allvis[r]+=e
        tw+=w
        top.sort(reverse=True)
        print(f" S{i:02d} wait inside excess visits {w*TS:7.1f} s of {F['cls'].get('wait',0)*TS:7.1f}; top detour rooms (s, room, full5 visits, ref visits): {top[:5]}")
    print(' total wait inside excess visits',round(tw*TS,1))
    print(' top detour rooms overall (room, s, excess visits):',[(r,round(v,1),allvis[r]) for r,v in allrooms.most_common(12)])
# progress stops by cumulative news/objects definition
path=pickle.load(open('path.pkl','rb'))
bs=B['full5']; END=108830
cum=set(); ev=set(path[0]['ever']); cnt=collections.Counter(); kinds=collections.Counter()
prev=path[0]
for o in path[1:]:
    new=set(o['news'])-cum; ne=set(o['ever'])-ev
    cum|=set(o['news']); ev|=set(o['ever'])
    f=min(o['frame'],END)
    i=next((i for i in range(17) if bs[i]<f<=bs[i+1]),None)
    if new or ne: cnt[i]+=1
    prev=o
print('progress stops per segment (new key in cumulative news or new object):',[cnt[i] for i in range(17)],sum(cnt.values()))
