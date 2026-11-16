import pickle,collections,json
from load import load,HZ
d=pickle.load(open('seg.pkl','rb')); res=d['res']; B=d['B']; SEGN=d['SEGN']
det=pickle.load(open('detour.pkl','rb'))
TS=4/HZ
D={k:load(k) for k in ('full5','slide','noslide')}
def fr(dd,a,b,room_excl=()):
    c=collections.Counter(); ct=collections.Counter()
    for f in range(a+1,b+1):
        if dd['room'][f] in room_excl: c['excl']+=1; ct['excl']+=dd['t'][f]-dd['t'][f-1]; continue
        c[dd['cls'][f]]+=1; ct[dd['cls'][f]]+=dd['t'][f]-dd['t'][f-1]
    return c,ct
print('after the dock (S01..S16), card room 95 excluded: visits, busy frames, busy ticks, ticks per busy frame, busy frames per visit, busy s per visit, wait s')
summ={}
for k in ('full5','slide','noslide'):
    dd=D[k]; a=B[k][1]; b=B[k][17]
    c,ct=fr(dd,a,b,room_excl=(95,))
    vis=sum(s['nvis'] for s in res[k][1:17])- sum(1 for s in res[k][1:17] for r,_,_ in s['visits'] if r==95)
    bf=c['walk']+c['cut']+c['act']; bt=ct['walk']+ct['cut']+ct['act']
    summ[k]=dict(vis=vis,busy_frames=bf,busy_ticks=bt,tpf=bt/bf,fpv=bf/vis,spv=bt*TS/vis,wait_s=ct['wait']*TS,wait_frames=c['wait'],card_s=ct['excl']*TS,
                 walk_f=c['walk'],cut_f=c['cut'],act_f=c['act'],walk_s=ct['walk']*TS,cut_s=ct['cut']*TS,act_s=ct['act']*TS)
    print(k,{x:(round(v,3) if isinstance(v,float) else v) for x,v in summ[k].items()})
for ref in ('slide','noslide'):
    f=summ['full5']; r=summ[ref]
    print('full5 /',ref,': visits x%.2f, busy frames per visit x%.2f, ticks per busy frame x%.3f, product x%.2f (busy seconds x%.2f); waiting %.0f s vs %.0f s'%(f['vis']/r['vis'],f['fpv']/r['fpv'],f['tpf']/r['tpf'],f['vis']/r['vis']*f['fpv']/r['fpv']*f['tpf']/r['tpf'],f['busy_ticks']/r['busy_ticks'],f['wait_s'],r['wait_s']))
# per segment table rows -> json
rows=[]
for i in range(18):
    row={'seg':SEGN[i]}
    for k in ('full5','slide','noslide'):
        s=res[k][i]
        if s is None: continue
        row[k]={'from_frame':s['a'],'to_frame':s['b'],'frames':s['frames'],'ticks':s['ticks'],'seconds':round(s['sec'],2),
                'wait_s':round(s['cls'].get('wait',0)*TS,1),'walk_s':round(s['cls'].get('walk',0)*TS,1),'cut_s':round(s['cls'].get('cut',0)*TS,1),'act_s':round(s['cls'].get('act',0)*TS,1),
                'wait_ticks':s['cls'].get('wait',0),'walk_ticks':s['cls'].get('walk',0),'cut_ticks':s['cls'].get('cut',0),'act_ticks':s['cls'].get('act',0),
                'room_visits':s['nvis'],'rooms':s['rooms'],'clicks':s['clicks'],'keys':s['keys'],'text_skips':s['tskips'],'cutscene_skips':s['cskips'],'objects_first_owned':s['objs']}
        if 'stops' in s: row[k]['stops']=s['stops']
    if i<17:
        for ref in ('slide','noslide'):
            x=det[ref][i]
            row['vs_'+ref]={k:round(v,2) if isinstance(v,float) else v for k,v in x.items() if k not in('i',)}
    rows.append(row)
json.dump({'segments':rows,'summary_after_dock_no_card':summ,'boundaries':B,'note':'seconds on the quarter-frame clock (qf/239.992156448); class seconds = main-loop ticks*4/239.992156448; slide = the 21:01.483 route (track 6 absent); the 23:11.320 route = slide + 129.84 s in S16; noslide = the 28:01.4 route (track 6 present)'},open('segments.json','w'),indent=1)
print('wrote segments.json')
