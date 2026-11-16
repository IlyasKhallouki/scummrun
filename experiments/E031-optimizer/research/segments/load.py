import json,pickle,collections,os
HZ=239.992156448
R='/home/ilyask/projects/scummrun/runs/anypercent/'
SRC={'full5':('/var/tmp/fast-mi/direct/video-full5/run/eng/frames.jsonl','/var/tmp/fast-mi/direct/video-full5/verify-out/eng/b0/trace.jsonl'),
     'slide':('ref-m2n/frames.jsonl','ref-m2n/trace.jsonl'),
     'noslide':('ref-noslide/frames.jsonl','ref-noslide/trace.jsonl')}
def cls(r):
    if 'idle' in r: return 'wait'
    if r['up']>0 and r.get('mv',0): return 'walk'
    if r['up']<=0 or r['cs']>0 or r.get('blk') in('message','cutscene'): return 'cut'
    return 'act'
def load(name):
    pk=f'cache-{name}.pkl'
    if os.path.exists(pk): return pickle.load(open(pk,'rb'))
    fp,tp=SRC[name]
    rows=[None]
    for l in open(fp): rows.append(json.loads(l))
    n=len(rows)-1
    t=[0]*(n+1); room=[0]*(n+1); c=['']*(n+1); blk=['']*(n+1)
    for f in range(1,n+1):
        r=rows[f]; t[f]=r['t']; room[f]=r['r']; c[f]=cls(r); blk[f]=('idle:'+r['idle']) if 'idle' in r else r.get('blk','?')
    tr=[json.loads(l) for l in open(tp)]
    d={'n':n,'t':t,'room':room,'cls':c,'blk':blk,'trace':tr,'rows':rows}
    pickle.dump(d,open(pk,'wb'))
    return d
if __name__=='__main__':
    for k in SRC:
        d=load(k)
        fi=next(f for f in range(1,d['n']+1) if d['cls'][f]=='wait')
        fu=next(f for f in range(1,d['n']+1) if d['rows'][f]['up']>0 and d['room'][f]==33)
        print(k,d['n'],'first idle frame',fi,'room',d['room'][fi],'first up>0 in room 33',fu, collections.Counter(d['cls'][1:]).most_common())
