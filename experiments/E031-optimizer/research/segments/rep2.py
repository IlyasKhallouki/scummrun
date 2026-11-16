import pickle,collections,json
from load import load,HZ
d=pickle.load(open('seg.pkl','rb')); res=d['res']; SEGN=d['SEGN']; B=d['B']
TS=4/HZ
D={k:load(k) for k in ('full5','slide','noslide')}
CL=['wait','walk','cut','act']
def piece(dd,f0,f1):
    c=collections.Counter()
    t=dd['t']; cl=dd['cls']
    for f in range(f0,f1+1): c[cl[f]]+=t[f]-t[f-1]
    return c
out={}
for ref in ('slide','noslide'):
    rows=[]
    for i in range(17):
        F=res['full5'][i]; Rr=res[ref][i]
        fv=collections.defaultdict(list)
        for r,f0,f1 in F['visits']:
            if r==0: continue
            fv[r].append(piece(D['full5'],f0,f1))
        rv=collections.Counter(r for r,_,_ in Rr['visits'] if r!=0)
        det=collections.Counter(); lo=0; hi=0; exv=0; never_rooms=0; never_t=0
        for r,ps in fv.items():
            n=len(ps); e=max(0,n-rv.get(r,0))
            if e==0: continue
            exv+=e
            busy=[p['walk']+p['cut']+p['act'] for p in ps]
            for c in ('walk','cut','act'):
                det[c]+=sum(p[c] for p in ps)*e/n
            sb=sorted(busy)
            lo+=sum(sb[:e]); hi+=sum(sb[-e:])
            if rv.get(r,0)==0: never_rooms+=1; never_t+=sum(busy)
        excess=F['sec']-Rr['sec']
        wF=F['cls'].get('wait',0)*TS; wR=Rr['cls'].get('wait',0)*TS
        dsum=sum(det.values())*TS
        busyF=sum(F['cls'].get(c,0) for c in('walk','cut','act'))*TS; busyR=sum(Rr['cls'].get(c,0) for c in('walk','cut','act'))*TS
        rest=excess-(wF-wR)-dsum
        rows.append({'i':i,'F':F['sec'],'R':Rr['sec'],'ratio':F['sec']/Rr['sec'],'excess':excess,'wait_ex':wF-wR,'detour':dsum,'det_lo':lo*TS,'det_hi':hi*TS,'rest':rest,
                     'det_walk':det['walk']*TS,'det_cut':det['cut']*TS,'det_act':det['act']*TS,'exvis':exv,'never_rooms':never_rooms,'never_t':never_t*TS,
                     'visF':F['nvis'],'visR':Rr['nvis'],'roomsF':F['rooms'],'roomsR':Rr['rooms'],'busyF':busyF,'busyR':busyR,
                     'walkF':F['cls'].get('walk',0)*TS,'walkR':Rr['cls'].get('walk',0)*TS,'cutF':F['cls'].get('cut',0)*TS,'cutR':Rr['cls'].get('cut',0)*TS})
    out[ref]=rows
    print('== full5 vs',ref)
    print(' seg   full5      ref  ratio   excess | wait     detour(lo..hi)        rest | shares w/d/r % | vis F/R  rooms F/R exvis never(rooms,s) | rest split: walk cut act')
    tot=collections.Counter()
    for r in rows:
        ex=r['excess']
        rw=(r['walkF']-r['det_walk'])-r['walkR']; rc=(r['cutF']-r['det_cut'])-r['cutR']
        ra=r['rest']-rw-rc
        print(f" S{r['i']:02d} {r['F']:8.1f} {r['R']:8.1f} {r['ratio']:6.2f} {ex:8.1f} | {r['wait_ex']:7.1f} {r['detour']:7.1f} ({r['det_lo']:6.1f}..{r['det_hi']:6.1f}) {r['rest']:7.1f} | {100*r['wait_ex']/ex:4.0f} {100*r['detour']/ex:4.0f} {100*r['rest']/ex:4.0f} | {r['visF']:3d}/{r['visR']:3d} {r['roomsF']:2d}/{r['roomsR']:2d} {r['exvis']:3d} {r['never_rooms']:2d},{r['never_t']:6.1f} | {rw:7.1f} {rc:7.1f} {ra:6.1f}")
        for k in ('F','R','excess','wait_ex','detour','det_lo','det_hi','rest','exvis','visF','visR','never_t'): tot[k]+=r[k]
        tot['rw']+=rw; tot['rc']+=rc; tot['ra']+=ra
    ex=tot['excess']
    print(f" TOT {tot['F']:8.1f} {tot['R']:8.1f} {tot['F']/tot['R']:6.2f} {ex:8.1f} | {tot['wait_ex']:7.1f} {tot['detour']:7.1f} ({tot['det_lo']:6.1f}..{tot['det_hi']:6.1f}) {tot['rest']:7.1f} | {100*tot['wait_ex']/ex:4.0f} {100*tot['detour']/ex:4.0f} {100*tot['rest']/ex:4.0f} | {tot['visF']}/{tot['visR']} exvis {tot['exvis']} never_s {tot['never_t']:.1f} | {tot['rw']:.1f} {tot['rc']:.1f} {tot['ra']:.1f}")
pickle.dump(out,open('detour.pkl','wb'))
