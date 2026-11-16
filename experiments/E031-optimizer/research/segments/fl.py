import json,collections,sys
def load(p):
    rows=[]
    for l in open(p):
        try: rows.append(json.loads(l))
        except Exception as e: print('bad line',len(rows)); break
    return rows
P={'full5':'/var/tmp/fast-mi/direct/video-full5/run/eng/frames.jsonl','m2n':'ref-m2n/frames.jsonl','noslide':'ref-noslide/frames.jsonl'}
for k,p in P.items():
    rows=load(p)
    print(k,len(rows),'first',rows[0]['f'],'last',rows[-1]['f'], 'keys',collections.Counter(tuple(sorted(r.keys())) for r in rows).most_common(6))
    c=collections.Counter()
    for r in rows:
        c[('idle:'+str(r['idle'])) if 'idle' in r else ('blk:'+str(r.get('blk')))]+=1
    print('  ',c.most_common())
    print('  up',collections.Counter(r['up'] for r in rows).most_common(), 'cs',collections.Counter(r['cs'] for r in rows).most_common())
    print('  v19',collections.Counter(r['v19'] for r in rows).most_common())
    # contiguity
    gaps=sum(1 for a,b in zip(rows,rows[1:]) if b['f']!=a['f']+1)
    print('  gaps',gaps, 'dt',collections.Counter(b['t']-a['t'] for a,b in zip(rows,rows[1:])).most_common(8))
