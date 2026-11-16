import json, time
from gf import *
t=time.time()
s = Session('base', first_P=20000, skips=True, start_frame=46, frame_log=True) if False else None
out = HERE/'out'/'base'; out.mkdir(parents=True, exist_ok=True)
cfg = EngineConfig(out_dir=out, game_path=GAME, release='dos-cd', timer_next='any', start_frame=46, max_ticks=10**7, seed=SEED,
                   raw_inputs=MOVIE, skip_text=True, skip_cutscenes=True, frame_log=True, game_settings={'talkspeed': '85'})
with Controller(cfg, timeout_s=900) as c:
    b = c.root
    st = b.run(until=[{"room": 0}, {"var": 293, "eq": 161}], max_frame=16000)
    print(json.dumps(brief(st)), st.get('qf0'))
    b.quit()
print('wall', time.time()-t)
