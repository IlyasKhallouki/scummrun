"""Glitch-findability probes on the reference movie (EVALUATION ONLY; nothing here is optimizer input).
One Session = one engine launch; trials are forks of the root, run one at a time."""
import json, os, signal, sys, time
from pathlib import Path
assert os.environ.get("FAST_MI_SCUMMVM_BIN"), "set FAST_MI_SCUMMVM_BIN"
from speedrun.control import Controller, ControlError
from speedrun.engine import EngineConfig

GAME = Path('/home/ilyask/projects/fast-mi-sandboxes/nocd/game/dos-cd')
MOVIE = Path('/home/ilyask/projects/scummrun/runs/anypercent/m2n/movie.jsonl')
SEED = 836557856
HERE = Path(__file__).resolve().parent

def load_movie(path=MOVIE):
    return [json.loads(l) for l in Path(path).read_text().splitlines()]

def order(events):
    """stable sort by frame; within a frame: skips, key, mouse (the movie's own order)"""
    def rank(e):
        return 0 if 'skips' in e else 1 if 'key' in e else 2
    idx = {id(e): i for i, e in enumerate(events)}
    return sorted(events, key=lambda e: (e['frame'], rank(e), idx[id(e)]))

class Session:
    def __init__(self, name, first_P=0, skips=True, start_frame=46, talk="85", events=None, frame_log=False, seed=SEED):
        self.out = HERE / 'out' / name
        self.out.mkdir(parents=True, exist_ok=True)
        self.ev = load_movie() if events is None else events
        prefix = None
        if first_P > 0:
            prefix = self.out / 'prefix.jsonl'
            prefix.write_text(''.join(json.dumps(e) + '\n' for e in self.ev if e['frame'] <= first_P))
        cfg = EngineConfig(out_dir=self.out, game_path=GAME, release='dos-cd', timer_next='any', start_frame=start_frame,
                           max_ticks=10**7, seed=seed, raw_inputs=prefix, skip_text=skips, skip_cutscenes=skips,
                           frame_log=frame_log, game_settings={'talkspeed': talk})
        self.c = Controller(cfg, timeout_s=900)
        self.root = self.c.start()
        self.fed = first_P
        if first_P > 0:
            self.root.run(frame=first_P)
        self.t0 = time.time()
        self.trials = 0

    def advance(self, P):
        assert P > self.fed
        self.root.inputs([e for e in self.ev if self.fed < e['frame'] <= P])
        st = self.root.run(frame=P)
        self.fed = P
        return st

    def after(self, P=None):
        P = self.fed if P is None else P
        return [dict(e) for e in self.ev if e['frame'] > P]

    def trial(self, events, stops, want_state=None, timeout=45.0, tag=None):
        """events: the child's inputs after the root's frame (already perturbed). stops: list of run kwargs, each
        gives one status. want_state: indexes of stops at which the full state dump is taken."""
        self.trials += 1
        out = {'st': [], 'state': {}}
        try:
            child = self.c.fork(self.root)
        except Exception as e:  # noqa
            return {'error': 'fork: %r' % (e,)}
        child._conn.settimeout(timeout)
        try:
            evs = order([e for e in events if e['frame'] > self.fed])
            child.inputs(evs)
            for i, kw in enumerate(stops):
                st = child.run(**kw)
                out['st'].append(st)
                if 'end' in st:
                    break
                if want_state and i in want_state:
                    out['state'][i] = child.state()
        except ControlError as e:
            out['error'] = str(e)[:200]
        except (TimeoutError, OSError) as e:
            out['error'] = 'timeout: %r' % (e,)
            try:
                os.kill(child.pid, signal.SIGKILL)
            except Exception:
                pass
        finally:
            with open(self.out / 'trials.log', 'a') as fh:
                fh.write(json.dumps({'n': self.trials, 'tag': tag, 't': round(time.time() - self.t0, 2), 'err': out.get('error'),
                                     'st': [brief(x) for x in out['st']]}) + '\n')
            try:
                child.quit()
            except Exception:
                pass
        return out

    def do(self, events, fn, tag=None, timeout=45.0):
        """fork the root, give the child `events` (those after the root's frame), call fn(child) -> dict"""
        self.trials += 1
        out = {}
        try:
            child = self.c.fork(self.root)
        except Exception as e:  # noqa
            return {'error': 'fork: %r' % (e,)}
        child._conn.settimeout(timeout)
        try:
            child.inputs(order([e for e in events if e['frame'] > self.fed]))
            out = fn(child) or {}
        except ControlError as e:
            out = {'error': str(e)[:200]}
        except (TimeoutError, OSError) as e:
            out = {'error': 'timeout: %r' % (e,)}
            try:
                os.kill(child.pid, signal.SIGKILL)
            except Exception:
                pass
        finally:
            try:
                child.quit()
            except Exception:
                pass
            with open(self.out / 'trials.log', 'a') as fh:
                fh.write(json.dumps({'n': self.trials, 'tag': tag, 't': round(time.time() - self.t0, 2), 'out': {k: v for k, v in out.items() if k != 'state'}}, default=str)[:600] + '\n')
        return out

    def close(self):
        try:
            self.c.close()
        except Exception as e:
            print('close:', e, file=sys.stderr)

def shift(events, idxs_frames, d):
    """shift the events whose (frame, kind-key) identity is in idxs (a set of original frames+kind) by d"""
    out = []
    for e in events:
        e2 = dict(e)
        if ident(e) in idxs_frames:
            e2['frame'] = e['frame'] + d
        out.append(e2)
    return out

def ident(e):
    k = [k for k in e if k != 'frame'][0]
    return (e['frame'], k)

def shift_from(events, f0, d):
    """delay every event at frame >= f0 by d"""
    return [dict(e, frame=e['frame'] + d) if e['frame'] >= f0 else dict(e) for e in events]

def core(state):
    """the parts of a state dump compared between trials"""
    return {
        'room': state['room'], 'ego_pos': state.get('ego_pos'), 'vars': state['vars'], 'bits': sorted(state['bits_set']),
        'inv': state['inventory'], 'owners': state['owners'], 'states': state['states'], 'classes': state['classes'],
        'slots': sorted((s['where'], s['number'], s['status'], s['frozen'], 1 if s['delay'] else 0, s['cutscene_override']) for s in state['slots']),
        'cut': state['cutscene'],
    }

def sdiff(a, b):
    """differences between two core() dicts: a = without, b = with"""
    d = {}
    if a['room'] != b['room']: d['room'] = [a['room'], b['room']]
    if a['ego_pos'] != b['ego_pos']: d['ego_pos'] = [a['ego_pos'], b['ego_pos']]
    d['vars'] = {i: [x, y] for i, (x, y) in enumerate(zip(a['vars'], b['vars'])) if x != y}
    d['bits_only_a'] = sorted(set(a['bits']) - set(b['bits'])); d['bits_only_b'] = sorted(set(b['bits']) - set(a['bits']))
    if a['inv'] != b['inv']: d['inv'] = [a['inv'], b['inv']]
    for k in ('owners', 'states', 'classes'):
        d[k] = {o: [a[k].get(o), b[k].get(o)] for o in set(a[k]) | set(b[k]) if a[k].get(o) != b[k].get(o)}
    sa, sb = set(map(tuple, a['slots'])), set(map(tuple, b['slots']))
    d['slots_only_a'] = sorted(sa - sb); d['slots_only_b'] = sorted(sb - sa)
    if a['cut'] != b['cut']: d['cut'] = [a['cut'], b['cut']]
    return d

def brief(st):
    if 'end' in st: return {'end': st['end']}
    return {k: st.get(k) for k in ('stop', 'frame', 'tick', 'qf', 'room', 'up', 'cur', 'cs', 'v19', 'rng', 'ego', 'mv', 'box', 'idle', 'blk') if k in st}

def E(st):
    """input counter above what the cutscene nesting implies (0 = normal at rest or inside cutscenes)"""
    return st['up'] + st['cs'] - 1

def shift_idx(events_all, idxs, d):
    """events_all: the whole movie (list); shift the events at movie indexes idxs by d; returns a new full list"""
    out = [dict(e) for e in events_all]
    for i in idxs:
        out[i]['frame'] += d
    return out

def drop_idx(events_all, idxs):
    return [dict(e) for i, e in enumerate(events_all) if i not in set(idxs)]

def delay_from(events_all, i0, d):
    return [dict(e, frame=e['frame'] + d) if i >= i0 else dict(e) for i, e in enumerate(events_all)]
