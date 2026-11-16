import json,collections
R='/home/ilyask/projects/scummrun/runs/anypercent/'
T={'full5':'/var/tmp/fast-mi/direct/video-full5/verify-out/eng/b0/trace.jsonl',
   'slideN':R+'m2n-verify/auto-skips/trace.jsonl','slide':R+'m2-verify/auto-skips/trace.jsonl','noslide':R+'verify-b1/auto-skips/trace.jsonl'}
HZ=239.992156448
first={}
for k,p in T.items():
    tr=[json.loads(l) for l in open(p)]
    rooms=[d for d in tr if d['type']=='room']
    f={}
    for d in rooms:
        f.setdefault(d['room'],d)
    first[k]=f
    seg=[d for d in tr if d['type']=='segment_start']
    print(k,'records',len(tr),'room changes',len(rooms),'distinct rooms',len(f),'last',tr[-1]['frame'],tr[-1]['qf'],'seg',seg[0]['frame'],seg[0]['qf'])
import itertools
keys=list(T)
common=set(first['full5'])
for k in keys: common&=set(first[k])
print('common rooms',len(common))
# order of first entry in each
order={k:[r for r,_ in sorted(first[k].items(), key=lambda x:x[1]['frame']) if r in common] for k in keys}
for k in keys: print(k,order[k])
# LCS across all 4 (iteratively)
def lcs(a,b):
    n,m=len(a),len(b)
    L=[[0]*(m+1) for _ in range(n+1)]
    for i in range(n):
        for j in range(m):
            L[i+1][j+1]=L[i][j]+1 if a[i]==b[j] else max(L[i][j+1],L[i+1][j])
    out=[];i,j=n,m
    while i and j:
        if a[i-1]==b[j-1]: out.append(a[i-1]);i-=1;j-=1
        elif L[i-1][j]>=L[i][j-1]: i-=1
        else: j-=1
    return out[::-1]
s=order['full5']
for k in keys[1:]: s=lcs(s,order[k])
print('LCS',s)
for r in s:
    print(r, {k:(first[k][r]['frame'], round(first[k][r]['qf']/HZ,1)) for k in keys})
