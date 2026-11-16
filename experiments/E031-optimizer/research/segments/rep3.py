import pickle,collections,json
from load import load,HZ
d=pickle.load(open('seg.pkl','rb')); res=d['res']; B=d['B']
TS=4/HZ
F=load('full5'); t=F['t']; cl=F['cls']
path=pickle.load(open('path.pkl','rb'))
END=108830
bs=B['full5']
def seg_of(f):
    for i in range(17):
        if bs[i]<f<=bs[i+1]: return i
    return None
# wait decomposition by trial type
W=[collections.Counter() for _ in range(17)]
N=[collections.Counter() for _ in range(17)]
prev=None; prev_ever=set(); 
for k,o in enumerate(path):
    if k==0: prev=o; prev_ever=set(o['ever']); continue
    a=prev['frame']; b=min(o['frame'],END)
    how=o['how']
    typ='waittrial' if how.startswith('wait') and '+' not in how else ('probe' if how.endswith('~') else 'plain')
    if how.startswith('wait') and '+' in how: typ='wait+act'
    # tail idle: consecutive idle frames ending at b
    f=b; tail=0
    while f>a and cl[f]=='wait': tail+=t[f]-t[f-1]; f-=1
    tailstart=f
    for g in range(a+1,b+1):
        if cl[g]=='wait':
            i=seg_of(g)
            if i is None: continue
            W[i][(typ,'tail' if g>tailstart else 'mid')]+=t[g]-t[g-1]
    i=seg_of(b) if b>bs[0] else None
    if i is not None and o['frame']<=END+6200:
        N[i]['stops']+=1; N[i][typ]+=1
        ev=set(o['ever'])
        prog=bool(o['news']) or bool(ev-prev_ever)
        if prog: N[i]['progress']+=1
        if o['room']!=prev['room'] and not prog: N[i]['roomchange_nogain']+=1
        if o['room']==prev['room'] and not prog: N[i]['inroom_nogain']+=1
    prev=o; prev_ever=set(o['ever'])
print('full5 waiting by the kind of trial it follows (seconds): probe-tail, waittrial(all), plain-tail(calm), mid (between the clicks of one trial), wait+act; counts: stops, progress, probe stops, wait trials')
tot=collections.Counter(); totn=collections.Counter()
for i in range(17):
    w=W[i]
    pt=w[('probe','tail')]*TS; wt=(w[('waittrial','tail')]+w[('waittrial','mid')])*TS; ct=w[('plain','tail')]*TS; mid=(w[('probe','mid')]+w[('plain','mid')])*TS; wa=(w[('wait+act','tail')]+w[('wait+act','mid')])*TS
    allw=res['full5'][i]['cls'].get('wait',0)*TS
    print(f"S{i:02d} wait {allw:7.1f} | probe-tail {pt:7.1f} waittrial {wt:6.1f} calm-tail {ct:6.1f} mid {mid:6.1f} wait+act {wa:5.1f} unassigned {allw-pt-wt-ct-mid-wa:5.1f} | stops {N[i]['stops']:3d} progress {N[i]['progress']:3d} probe {N[i]['probe']:3d} waittrials {N[i]['waittrial']+N[i]['wait+act']:2d} roomchange-nogain {N[i]['roomchange_nogain']:3d} inroom-nogain {N[i]['inroom_nogain']:3d}")
    for k,v in (('probe',pt),('waittrial',wt),('calm',ct),('mid',mid),('wa',wa),('all',allw)): tot[k]+=v
    totn.update(N[i])
print('TOT',{k:round(v,1) for k,v in tot.items()},dict(totn))
# reference goals per segment
R='/home/ilyask/projects/scummrun/runs/anypercent/'
for name,run in (('slide','m2n'),('noslide','noslide-p7')):
    G=[json.loads(l) for l in open(f'{R}{run}/results.jsonl')]
    b=B[name]
    cnt=[sum(1 for g in G if b[i]<g['done']<=b[i+1] and not g.get('skipped')) for i in range(18)]
    print(name,'goals per segment',cnt,sum(cnt))
