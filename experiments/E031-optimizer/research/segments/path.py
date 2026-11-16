import json,pickle
D='/var/tmp/fast-mi/direct/full5/'
tops={0:15923,1:11189,2:22320}
end=7895
path=[]
cur_leg=3; target=end
while cur_leg>=0:
    nodes={}
    for l in open(f'{D}leg{cur_leg:02d}/nodes.jsonl'):
        # cheap parse
        d=json.loads(l)
        nodes[d['id']]=d
    seg=[]
    i=target
    while i is not None:
        n=nodes[i]; seg.append((cur_leg,n)); i=n['parent']
    # drop the root (id 0) for legs >0: it repeats the previous leg's top
    if cur_leg>0: seg=seg[:-1]
    path=seg[::-1]+path
    cur_leg-=1
    if cur_leg>=0: target=tops[cur_leg]
out=[]
for leg,n in path:
    out.append({'leg':leg,'id':n['id'],'parent':n['parent'],'frame':n['frame'],'room':n['room'],'level':n['level'],'how':n['how'],'novel':n['novel'],'news':n['news'],'ever':n['ever'],'nev':len(n['events']),'clicks':sum(1 for e in n['events'] if 'click' in e) if n.get('movie')=='own' else None,'own':n.get('movie')=='own','events':n['events'] if n.get('movie')=='own' else None})
print(len(out), out[0]['frame'], out[-1]['frame'])
pickle.dump(out,open('path.pkl','wb'))
# checks
fr=[o['frame'] for o in out]
print('monotone',all(a<=b for a,b in zip(fr,fr[1:])))
print(sum(1 for o in out if not o['own']), 'stops without own movie')
