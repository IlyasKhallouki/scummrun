import json,pickle,collections,sys
from load import load,HZ
CL=['wait','walk','cut','act']
SEGN=['S00 boot/logo skip -> dock (room 33 entered)',
      'S01 -> P1+P2 (owner:566, bit:72)',
      'S02 -> P3+P4+P6 (bit:65, bit:98 + 2 owners, var:196>=1)',
      'S03 -> P5 (bit:19)',
      'S04 -> P7 (var:196>=2)',
      'S05 -> P8 (var:196>=3)',
      'S06 -> P9 (bit:304)',
      'S07 -> P10-P13 (bits 51,88,76,89 all)',
      'S08 -> Part II begins (room 7 entered)',
      'S09 -> Part III begins (room 20 entered)',
      'S10 -> object 884 owned',
      'S11 -> object 269 owned',
      'S12 -> room 39 entered',
      'S13 -> room 70 entered',
      'S14 -> owner 823',
      'S15 -> last part card (room 95 entered)',
      'S16 -> end flag (bit:565)',
      'S17 -> final fade (room 0, var:293=161)']
B={'full5':[0,2638,12332,34100,36858,48128,56215,56686,68055,68367,73457,77221,98507,99731,100761,105263,105728,108830,None],
   'slide':[46,51,1056,3626,4216,5066,6111,6341,9056,9331,10110,10286,11541,12274,12353,13121,13434,14336,14820],
   'noslide':[46,51,1056,3626,4216,5201,6281,6521,9141,9558,10449,10651,14176,15005,15563,16576,16902,19376,19859]}
D={k:load(k) for k in B}
path=pickle.load(open('path.pkl','rb'))
facts=pickle.load(open('facts.pkl','rb'))
def objs_first(name):
    f=facts['f5'] if name=='full5' else facts['refs']['ref-m2n' if name=='slide' else 'ref-noslide']
    return {k[1]:v for k,v in f.items() if k[0]=='obj'}
def qf_at(d,f):
    # qf = 4*tick + offset from the last trace record at or before f
    if 'qoff' not in d:
        d['qoff']=sorted((x['frame'],x['qf']-4*x['tick']) for x in d['trace'])
    import bisect
    i=bisect.bisect_right(d['qoff'],(f,10**12))-1
    off=d['qoff'][i][1] if i>=0 else 0
    return 4*d['t'][f]+off
def visits(d,a,b):
    # visit pieces in frames (a, b]: list of (room, f0, f1) with frames f0..f1 inclusive
    out=[];f=a+1
    while f<=b:
        g=f
        while g+1<=b and d['room'][g+1]==d['room'][f]: g+=1
        out.append((d['room'][f],f,g)); f=g+1
    return out
def ticks(d,f0,f1,cl=None):
    # ticks of frames f0..f1 inclusive
    if cl is None: return d['t'][f1]-d['t'][f0-1]
    return sum(d['t'][f]-d['t'][f-1] for f in range(f0,f1+1) if d['cls'][f]==cl)
res={}
for name in B:
    d=D[name]; t=d['t']; bs=B[name]
    of=objs_first(name)
    inputs=[x for x in d['trace'] if x['type']=='input']
    skips=[x for x in d['trace'] if x['type']=='skip']
    out=[]
    for i in range(len(bs)-1):
        a,b=bs[i],bs[i+1]
        if b is None: out.append(None); continue
        s={'a':a,'b':b,'frames':b-a,'ticks':t[b]-t[a],'qf':qf_at(d,b)-qf_at(d,a)}
        s['sec']=s['qf']/HZ
        c=collections.Counter(); cb=collections.Counter()
        for f in range(a+1,b+1):
            dt=t[f]-t[f-1]; c[d['cls'][f]]+=dt; cb[d['blk'][f]]+=dt
        s['cls']=dict(c); s['blk']=dict(cb)
        v=visits(d,a,b)
        s['visits']=[(r,f0,f1) for r,f0,f1 in v]
        s['nvis']=sum(1 for r,_,_ in v if r!=0); s['rooms']=len({r for r,_,_ in v if r!=0})
        s['clicks']=sum(1 for x in inputs if a<x['frame']<=b and ('click' in x or 'rclick' in x))
        s['keys']=sum(1 for x in inputs if a<x['frame']<=b and 'key' in x)
        s['tskips']=sum(1 for x in skips if a<x['frame']<=b and x['kind']=='text')
        s['cskips']=sum(1 for x in skips if a<x['frame']<=b and x['kind']=='cutscene')
        s['objs']=sum(1 for o,f in of.items() if a<f<=b)
        if name=='full5':
            st=[o for o in path if a<o['frame']<=b]
            s['stops']=len(st)
        out.append(s)
    res[name]=out
pickle.dump({'res':res,'B':B,'SEGN':SEGN},open('seg.pkl','wb'))
def fm(x): return f'{x:9.1f}'
print('segment | full5 s | slide s | noslide s | ratio f5/slide | f5/noslide')
for i,nm in enumerate(SEGN):
    r=[res[k][i] for k in ('full5','slide','noslide')]
    if r[0] is None:
        print(f'{nm:58s} {"-":>9s} {r[1]["sec"]:9.1f} {r[2]["sec"]:9.1f}'); continue
    print(f'{nm:58s} {r[0]["sec"]:9.1f} {r[1]["sec"]:9.1f} {r[2]["sec"]:9.1f}  {r[0]["sec"]/r[1]["sec"]:7.2f} {r[0]["sec"]/r[2]["sec"]:7.2f}')
for k in ('full5','slide','noslide'):
    tot=sum(s['sec'] for s in res[k] if s); print(k,'total s',round(tot,2),'to 565',round(sum(s['sec'] for s in res[k][:17] if s),2))
