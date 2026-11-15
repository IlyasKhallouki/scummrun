import json, os, subprocess, sys, shutil
SP=sys.argv[1]; MOVIE=sys.argv[2]; OUT=sys.argv[3]
E016=os.environ.get('E016','/var/tmp/fast-mi/target-e019/release/e016')
os.makedirs(OUT, exist_ok=True)
clicks=[json.loads(l) for l in open(MOVIE) if l.strip()]
clicks=[c for c in clicks if 'click' in c]
n=[0]
def test(fs):
    n[0]+=1; d=f'{OUT}/t{n[0]}'; f=d+'.jsonl'
    evs=[]; placed=False
    for c in clicks:
        if not placed and c['frame']>fs: evs.append({'frame':fs,'skips':{'text':1,'cutscenes':1}}); placed=True
        evs.append(c)
    if not placed: evs.append({'frame':fs,'skips':{'text':1,'cutscenes':1}})
    open(f,'w').write(''.join(json.dumps(e)+'\n' for e in evs))
    subprocess.run([E016,'inlog',d,'--events',f,'--from','100','--to','25000'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=600)
    end=None
    try:
        for l in open(d+'/boot/trace.jsonl'):
            if '"type":"end"' in l: j=json.loads(l); end=(j['frame'],j['tick'],j['reason'])
    except FileNotFoundError: pass
    shutil.rmtree(d,ignore_errors=True)
    return end
base=test(10**9)
print('no skips', base, flush=True)
lo, hi = 1, clicks[-1]['frame']   # find the earliest fs whose run ends at the door no later than base
ok_hi=test(hi); print('skips from', hi, ok_hi, flush=True)
best=(hi, ok_hi) if ok_hi and ok_hi[2]==base[2] else None
while best and hi - lo > 50:
    mid=(lo+hi)//2
    r=test(mid)
    good = r is not None and r[2]==base[2] and r[0] <= base[0]
    print('skips from', mid, r, 'ok' if good else 'no', flush=True)
    if good: hi=mid; best=(mid,r)
    else: lo=mid
print('earliest', best, flush=True)
