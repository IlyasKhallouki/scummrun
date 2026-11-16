import json,pickle,collections
path=pickle.load(open('path.pkl','rb'))
END5=108830
# full5 object first gains (stop frames)
f5={}
prev=set()
for o in path:
    ev=set(o['ever'])
    for x in ev-prev: f5[('obj',x)]=min(o['frame'],END5)
    prev=ev
# rooms from traces
def rooms(p):
    f={}
    for l in open(p):
        d=json.loads(l)
        if d['type']=='room': f.setdefault(('room',d['room']),d['frame'])
    return f
r5=rooms('/var/tmp/fast-mi/direct/video-full5/verify-out/eng/b0/trace.jsonl')
f5.update({k:v for k,v in r5.items() if v<=END5})
bits5={72:7906,65:32712,98:34100,19:36858,304:56686,88:58176,51:61197,76:65601,89:68055,565:108830}
for b,v in bits5.items(): f5[('bit',b)]=v
f5[('v196',1)]=24083; f5[('v196',2)]=48128; f5[('v196',3)]=56215
refs={}
for run in ['ref-m2n','ref-noslide']:
    d=json.load(open(run+'/facts.json'))
    f={}
    for fr,o in d['gain_events']: f.setdefault(('obj',o),fr)
    for k,v in d['seen'].items():
        a,b=k.split(':')
        if a in('bit','v196'): f[(a,int(b))]=v['frame']
    f.update(rooms(run+'/trace.jsonl'))
    refs[run]=f
print('full5 facts',len(f5),'objs',sum(1 for k in f5 if k[0]=='obj'),'rooms',sum(1 for k in f5 if k[0]=='room'))
for run,f in refs.items(): print(run,len(f),'objs',sum(1 for k in f if k[0]=='obj'),'rooms',sum(1 for k in f if k[0]=='room'))
def cuts(routes):
    common=set(routes[0])
    for r in routes[1:]: common&=set(r)
    orders=[sorted(common,key=lambda k:(r[k],k)) for r in routes]
    # times with ties: use set-prefix by time threshold: cut after fact k in route0 if set of facts with time<=t0 equal in all routes for some thresholds
    out=[]
    n=len(common)
    # candidate prefix sets from route0 at each distinct time
    r0=routes[0]
    times0=sorted(set(r0[k] for k in common))
    for t in times0:
        S={k for k in common if r0[k]<=t}
        ok=True; ts=[t]
        for r in routes[1:]:
            tt=max(r[k] for k in S)
            S2={k for k in common if r[k]<=tt}
            if S2!=S: ok=False;break
            ts.append(tt)
        if ok: out.append((len(S),ts,[k for k in S if r0[k]==t]))
    return common,out
for name,rs in [('all3',[f5,refs['ref-m2n'],refs['ref-noslide']]),('full5-vs-slide',[f5,refs['ref-m2n']]),('full5-vs-noslide',[f5,refs['ref-noslide']])]:
    common,out=cuts(rs)
    print('==',name,'common facts',len(common),'objs',sum(1 for k in common if k[0]=='obj'),'cuts',len(out))
    for n,ts,ks in out: print('  ',n,ts,ks)
pickle.dump({'f5':f5,'refs':refs},open('facts.pkl','wb'))
# objects only in one
o5={k[1] for k in f5 if k[0]=='obj'}; om={k[1] for k in refs['ref-m2n'] if k[0]=='obj'}; on={k[1] for k in refs['ref-noslide'] if k[0]=='obj'}
print('objs full5',len(o5),'m2n',len(om),'noslide',len(on),'full5&m2n',len(o5&om),'full5&noslide',len(o5&on),'full5 only',sorted(o5-om-on),'m2n not full5',sorted(om-o5),'noslide not full5',sorted(on-o5))
