import json
HZ=239.992156448
def load(run):
    return [json.loads(l) for l in open(f'{run}/samples.jsonl')]
for run in ['ref-m2n','ref-noslide']:
    S=load(run)
    print(run,len(S),'last',S[-1]['frame'],S[-1]['qf'])
    seen={}
    ever=set(); evt=[]
    prev196=0
    for s in S:
        for b in s['bits']:
            if ('bit',b) not in seen: seen[('bit',b)]=s
        if s['v196']!=prev196:
            for v in range(prev196+1,s['v196']+1): seen.setdefault(('v196',v),s)
            prev196=s['v196']
        inv=set(s['inv'])
        for o in inv-ever:
            evt.append((s['frame'],o))
        ever|=inv
        for o in (566,396,388,901,902,903,761,823,733):
            if o in inv and ('own',o) not in seen: seen[('own',o)]=s
    for k,s in sorted(seen.items(), key=lambda x:x[1]['frame']):
        print('  ',k,'frame',s['frame'],'tick',s['tick'],'rta',round((s['qf']-1100)/HZ,2),'room',s['room'])
    print('  objects ever in inventory',len(ever), 'final inv',len(S[-1]['inv']), 'max bits',max(s['nbits'] for s in S))
    json.dump({'seen':{f'{k[0]}:{k[1]}':{'frame':s['frame'],'tick':s['tick'],'qf':s['qf']} for k,s in seen.items()},'gain_events':evt},open(run+'/facts.json','w'))
