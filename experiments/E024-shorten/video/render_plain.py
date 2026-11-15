"""render_plain.py CAPDIR OUT.mp4: the capture (cap.rgb, one 320x200 frame per jiffy; cap.pcm, audio per tick) and
the standalone launch's trace, as a plain video: the game at 4x, a ring at each click with a cursor where it was, and
a real-time timer (jiffies / 59.998) in a bar below. Nothing else is drawn. At the end the timer stops, the last
picture stays, fades to black and the time stays on: the capture ends at the route's last frame, and the video should
not. The audio gets silence where the screen waits without the tick clock moving (as in render.py), and a short
fade at its last samples."""
import json, os, subprocess, sys
import numpy as np
from PIL import Image, ImageDraw, ImageFont

cap, out = sys.argv[1], sys.argv[2]
W, H, S = 320, 200, 4
BAR = 88
FPS = 59.998039
HOLD, FADE, DARK = 120, 90, 180     # frames: the last picture, its fade to black, the time on black
info = open(f"{cap}/cap.txt").read().split()
rate = int(info[info.index("audio_rate") + 1])
trace = [json.loads(l) for l in open(f"{cap}/verify/standalone/trace.jsonl")]
end = next(t for t in trace if t["type"] == "end")
nframes = os.path.getsize(f"{cap}/cap.rgb") // (W * H * 3)
tail = HOLD + FADE + DARK

clicks = [(t["qf"] // 4, t["click"]) for t in trace if t["type"] == "input" and "click" in t]
print(f"frames {nframes}, clicks {len(clicks)}, end tick {end['tick']} qf {end['qf']}", file=sys.stderr)

# audio: d = jiffies - ticks at the trace's inputs, controls and end (one point per frame start); where d grows, the
# screen waited without ticks: that many jiffies of silence go in at the tick of the room change or skip in between
pts = sorted((t["tick"], t["qf"] / 4 - t["tick"]) for t in trace if t["type"] in ("input", "control", "end", "boot"))
marks = sorted(t["tick"] for t in trace if t["type"] in ("room", "skip"))
pcm = np.fromfile(f"{cap}/cap.pcm", dtype="<i2").reshape(-1, 2)
per_tick = len(pcm) / end["tick"]
EDGE = rate * 15 // 1000
pieces, pos, prev, rise = [], 0, pts[0], False
for tk, d in pts[1:]:
    gap = d - prev[1]
    if gap > 0:
        at = next((m for m in marks if prev[0] <= m <= tk), tk)
        a = int(round(at * per_tick))
        # (the sound goes down before the silence and comes up after it, 15 ms each: no click at the cut)
        seg = pcm[pos:a].astype(np.float32)
        n = min(len(seg), EDGE)
        if n:
            seg[len(seg) - n:] *= np.linspace(1.0, 0.0, n, dtype=np.float32)[:, None]
        if rise and len(seg):
            m = min(len(seg), EDGE)
            seg[:m] *= np.linspace(0.0, 1.0, m, dtype=np.float32)[:, None]
        pieces.append(seg.astype("<i2")); pieces.append(np.zeros((int(round(gap * rate / FPS)), 2), dtype="<i2"))
        pos, rise = a, True
    prev = (tk, d)
seg = pcm[pos:].astype(np.float32)
if rise and len(seg):
    m = min(len(seg), EDGE)
    seg[:m] *= np.linspace(0.0, 1.0, m, dtype=np.float32)[:, None]
pieces.append(seg.astype("<i2"))
audio = np.concatenate(pieces).astype(np.float32)
# the sound stops with the capture: its last third of a second fades, so it does not end on a cut
n = min(len(audio), rate // 3)
audio[len(audio) - n:] *= np.linspace(1.0, 0.0, n, dtype=np.float32)[:, None]
audio = audio.astype("<i2")
audio = np.concatenate([audio, np.zeros((max(0, int((nframes + tail) * rate / FPS) - len(audio)), 2), dtype="<i2")])
audio.tofile(f"{cap}/aligned.pcm")
print(f"audio {len(pcm)/rate:.2f} s -> {len(audio)/rate:.2f} s; video {(nframes + tail)/FPS:.2f} s", file=sys.stderr)

big = ImageFont.truetype("/usr/share/fonts/noto/NotoSansMono-Bold.ttf", 56)
huge = ImageFont.truetype("/usr/share/fonts/noto/NotoSansMono-Bold.ttf", 120)
OW, OH = W * S, H * S + BAR
BG = (16, 16, 20)
ff = subprocess.Popen(["ffmpeg", "-hide_banner", "-loglevel", "error",
                       "-f", "rawvideo", "-pixel_format", "rgb24", "-video_size", f"{OW}x{OH}", "-framerate", f"{FPS}", "-i", "-",
                       "-f", "s16le", "-ar", str(rate), "-ac", "2", "-i", f"{cap}/aligned.pcm",
                       "-c:v", "libx264", "-preset", "slow", "-crf", "18", "-pix_fmt", "yuv420p", "-tune", "animation",
                       "-c:a", "aac", "-b:a", "160k", "-shortest", "-movflags", "+faststart", "-y", out], stdin=subprocess.PIPE)

def clock(j):
    s = j / FPS
    return f"{int(s // 60)}:{int(s % 60):02d}.{int(s * 100) % 100:02d}"

RING = 22
GREEN, WHITE = (90, 230, 120), (255, 255, 255)
final = clock(nframes - 1)
f = open(f"{cap}/cap.rgb", "rb")
ci, last, game = 0, None, None
for j in range(nframes + tail):
    if j < nframes:
        raw = np.frombuffer(f.read(W * H * 3), dtype=np.uint8).reshape(H, W, 3)
        game = raw.repeat(S, 0).repeat(S, 1)
    over = j - (nframes - 1)                                   # frames since the last picture
    dim = 0.0 if over <= HOLD else min(1.0, (over - HOLD) / FADE)   # how far the picture has gone to black
    img = Image.new("RGB", (OW, OH), BG)
    if dim < 1.0:
        top = Image.fromarray(game)
        d = ImageDraw.Draw(top)
        jj = min(j, nframes - 1)
        while ci < len(clicks) and clicks[ci][0] <= jj:
            last = clicks[ci]; ci += 1
        # a ring at each recent click, then the cursor where the last click was
        for k in range(max(0, ci - 3), ci):
            cj, (x, y) = clicks[k]
            age = jj - cj
            if 0 <= age < RING:
                r = 10 + age * 2.2
                cx, cy = x * S + S // 2, y * S + S // 2
                col = (255, 214, 0) if age < RING * 0.6 else (200, 170, 0)
                d.ellipse([cx - r, cy - r, cx + r, cy + r], outline=col, width=max(2, 6 - age // 4))
        if last is not None:
            x, y = last[1]
            cx, cy = x * S + S // 2, y * S + S // 2
            for w_, c in ((7, (0, 0, 0)), (3, (255, 255, 255))):
                d.line([cx - 18, cy, cx - 5, cy], fill=c, width=w_); d.line([cx + 5, cy, cx + 18, cy], fill=c, width=w_)
                d.line([cx, cy - 18, cx, cy - 5], fill=c, width=w_); d.line([cx, cy + 5, cx, cy + 18], fill=c, width=w_)
        if dim > 0.0:
            top = Image.blend(top, Image.new("RGB", top.size, BG), dim)
        img.paste(top, (0, 0))
    d = ImageDraw.Draw(img)
    done = over >= 0
    if dim <= 0.0:
        # the timer in the bar: running, then stopped at the last frame
        t = final if done else clock(j)
        tw = d.textlength(t, font=big)
        d.text((OW - tw - 28, H * S + 10), t, font=big, fill=GREEN if done else WHITE)
    else:
        # as the picture goes, the time moves to the middle and stays there
        k = dim
        size = int(56 + (120 - 56) * k)
        font = huge if k >= 1.0 else ImageFont.truetype("/usr/share/fonts/noto/NotoSansMono-Bold.ttf", size)
        tw = d.textlength(final, font=font)
        x0, y0 = OW - tw - 28, H * S + 10
        x1, y1 = (OW - tw) / 2, (OH - size) / 2 - 12
        d.text((x0 + (x1 - x0) * k, y0 + (y1 - y0) * k), final, font=font, fill=GREEN)
    ff.stdin.write(img.tobytes())
    if j % 2000 == 0:
        print(f"{j}/{nframes + tail}", file=sys.stderr)
ff.stdin.close()
ff.wait()
print(f"final time {final}", file=sys.stderr)
