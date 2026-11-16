import pickle
d=pickle.load(open('facts.pkl','rb')); f5=d['f5']; m=d['refs']['ref-m2n']; n=d['refs']['ref-noslide']
def cuts(routes,keep):
    common=set(k for k in routes[0] if keep(k))
    for r in routes[1:]: common&=set(r)
    r0=routes[0]; out=[]
    for t in sorted(set(r0[k] for k in common)):
        S={k for k in common if r0[k]<=t}; ok=True; ts=[t]
        for r in routes[1:]:
            tt=max(r[k] for k in S)
            if {k for k in common if r[k]<=tt}!=S: ok=False;break
            ts.append(tt)
        if ok: out.append((len(S),ts,sorted(k for k in S if r0[k]==t)))
    return common,out
partrooms={('room',x) for x in (33,7,20,95)}
for name,keep in [('objects+rungs+part rooms',lambda k:k[0]!='room' or k in partrooms),('rungs+part rooms',lambda k:k[0] in('bit','v196') or k in partrooms or k in {('obj',x) for x in (566,901,902,903,761,823,733)}), ('rungs+watch rooms',lambda k:k[0] in('bit','v196') or k in partrooms or k in {('room',x) for x in (12,69,39,70,75,59,83)} or k in {('obj',x) for x in (566,901,902,903,761,823,733)})]:
    for rn,rs in [('all3',[f5,m,n]),('vs-slide',[f5,m]),('vs-noslide',[f5,n])]:
        common,out=cuts(rs,keep)
        print('==',name,rn,'facts',len(common),'cuts',len(out))
        if rn=='all3' or name.startswith('objects'):
            for c in out: print('   ',c)
