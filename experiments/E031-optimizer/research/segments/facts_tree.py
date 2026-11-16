import pickle,json,re
path=pickle.load(open('path.pkl','rb'))
ids={(o['leg'],o['id']) for o in path}
# rung nodes
for leg in range(4):
    for l in open(f'/var/tmp/fast-mi/direct/full5/leg{leg:02d}/rung-stops.jsonl'):
        d=json.loads(l)
        print('rung-stop leg',leg,'id',d['id'],'frame',d['frame'],'room',d['room'],'on path',(leg,d['id']) in ids, d.get('rung'), d.get('watch'), [k for k in d if k not in('events','novel','news','ever','id','parent','depth','frame','room','level','owned','class','how','t')])
want_bits=[72,65,98,19,304,51,89,88,76,565]
want_own=[566,396,388,901,902,903,761,823,733]
seen=set()
for o in path:
    for k in o['news']:
        if k in [f'bit:{b}' for b in want_bits] and k not in seen:
            seen.add(k); print('news',k,'leg',o['leg'],'id',o['id'],'frame',o['frame'],'room',o['room'],o['how'])
    for x in want_own:
        if x in o['ever'] and ('own',x) not in seen:
            seen.add(('own',x)); print('ever',x,'leg',o['leg'],'id',o['id'],'frame',o['frame'],'room',o['room'],o['how'])
    for lab in o['novel']:
        if lab.startswith('var:196=') and lab not in seen:
            seen.add(lab); print('novel',lab,'leg',o['leg'],'id',o['id'],'frame',o['frame'],'room',o['room'],o['how'])
