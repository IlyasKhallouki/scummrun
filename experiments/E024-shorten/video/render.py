"""render.py CAPDIR OUT.mp4: the capture (cap.rgb, one 320x200 frame per jiffy; cap.pcm, audio per tick) and the
standalone launch's trace, as a video: the game at 4x, a cursor and a ring at each click, a badge at each skip key,
a click counter and a real-time timer (jiffies / 59.998). The audio gets silence where the screen waits (room
transitions, skipped cutscenes) without the tick clock moving, so it stays on the picture."""
import json, os, subprocess, sys
import numpy as np
from PIL import Image, ImageDraw, ImageFont

cap, out = sys.argv[1], sys.argv[2]
W, H, S = 320, 200, 4
BAR = 112
FPS = 59.998039
HOLD = 150                      # frames the last picture stays, the timer stopped
info = open(f"{cap}/cap.txt").read().split()
rate = int(info[info.index("audio_rate") + 1])
trace = [json.loads(l) for l in open(f"{cap}/verify/standalone/trace.jsonl")]
end = next(t for t in trace if t["type"] == "end")
nframes = os.path.getsize(f"{cap}/cap.rgb") // (W * H * 3)

clicks = [(t["qf"] // 4, t["click"]) for t in trace if t["type"] == "input" and "click" in t]
skips = [(t["qf"] // 4, "ESC" if t["kind"] == "cutscene" else ".") for t in trace if t["type"] == "skip"]
print(f"frames {nframes}, clicks {len(clicks)}, skips {len(skips)}, end tick {end['tick']} qf {end['qf']}", file=sys.stderr)

# audio: d = jiffies - ticks at the trace's inputs, controls and end (one point per frame start); where d grows, the
# screen waited without ticks: that many jiffies of silence go in at the tick of the room change or skip in between
pts = sorted((t["tick"], t["qf"] / 4 - t["tick"]) for t in trace if t["type"] in ("input", "control", "end", "boot"))
marks = sorted(t["tick"] for t in trace if t["type"] in ("room", "skip"))
pcm = np.fromfile(f"{cap}/cap.pcm", dtype="<i2").reshape(-1, 2)
per_tick = len(pcm) / end["tick"]
pieces, pos, prev = [], 0, pts[0]
for tk, d in pts[1:]:
    gap = d - prev[1]
    if gap > 0:
        at = next((m for m in marks if prev[0] <= m <= tk), tk)
        a = int(round(at * per_tick))
        pieces.append(pcm[pos:a]); pieces.append(np.zeros((int(round(gap * rate / FPS)), 2), dtype="<i2"))
        pos = a
    prev = (tk, d)
pieces.append(pcm[pos:])
audio = np.concatenate(pieces)
audio = np.concatenate([audio, np.zeros((max(0, int((nframes + HOLD) * rate / FPS) - len(audio)), 2), dtype="<i2")])
audio.tofile(f"{cap}/aligned.pcm")
print(f"audio {len(pcm)/rate:.2f} s -> {len(audio)/rate:.2f} s; video {(nframes + HOLD)/FPS:.2f} s", file=sys.stderr)

big = ImageFont.truetype("/usr/share/fonts/noto/NotoSansMono-Bold.ttf", 54)
mid = ImageFont.truetype("/usr/share/fonts/noto/NotoSansMono-Bold.ttf", 26)
small = ImageFont.truetype("/usr/share/fonts/noto/NotoSansMono-Bold.ttf", 18)
OW, OH = W * S, H * S + BAR
ff = subprocess.Popen(["ffmpeg", "-hide_banner", "-loglevel", "error",
                       "-f", "rawvideo", "-pixel_format", "rgb24", "-video_size", f"{OW}x{OH}", "-framerate", f"{FPS}", "-i", "-",
                       "-f", "s16le", "-ar", str(rate), "-ac", "2", "-i", f"{cap}/aligned.pcm",
                       "-c:v", "libx264", "-preset", "slow", "-crf", "18", "-pix_fmt", "yuv420p", "-tune", "animation",
                       "-c:a", "aac", "-b:a", "160k", "-shortest", "-movflags", "+faststart", "-y", out], stdin=subprocess.PIPE)

def clock(j):
    s = j / FPS
    return f"{int(s // 60)}:{int(s % 60):02d}.{int(s * 100) % 100:02d}"

RING = 22
f = open(f"{cap}/cap.rgb", "rb")
ci, si, last = 0, 0, None
for j in range(nframes + HOLD):
    if j < nframes:
        raw = np.frombuffer(f.read(W * H * 3), dtype=np.uint8).reshape(H, W, 3)
        game = raw.repeat(S, 0).repeat(S, 1)
    img = Image.new("RGB", (OW, OH), (16, 16, 20))
    img.paste(Image.fromarray(game), (0, 0))
    d = ImageDraw.Draw(img)
    jj = min(j, nframes - 1)
    while ci < len(clicks) and clicks[ci][0] <= jj:
        last = clicks[ci]; ci += 1
    while si < len(skips) and skips[si][0] <= jj:
        si += 1
    # ring at each recent click, then the cursor where the last click was
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
    # skip keys pressed in the last 0.25 s
    recent = [k for k in skips[max(0, si - 4):si] if jj - k[0] < 15]
    if recent:
        key = recent[-1][1]
        bx, by = 18, H * S - 58
        tw = d.textlength(key, font=mid)
        d.rounded_rectangle([bx, by, bx + tw + 28, by + 42], radius=8, fill=(30, 30, 36), outline=(255, 255, 255), width=3)
        d.text((bx + 14, by + 4), key, font=mid, fill=(255, 255, 255))
    # the bar
    y0 = H * S
    d.text((20, y0 + 12), "Indiana Jones and the Fate of Atlantis (demo)", font=small, fill=(170, 170, 180))
    d.text((20, y0 + 40), f"clicks {ci:>2} / {len(clicks)}    skip keys {si:>2} / {len(skips)}", font=mid, fill=(235, 235, 240))
    d.text((20, y0 + 80), "route found and shortened by scummrun, no hand input; seed 1", font=small, fill=(120, 120, 130))
    done = j >= nframes - 1
    t = clock(nframes - 1 if done else j)
    tw = d.textlength(t, font=big)
    d.text((OW - tw - 24, y0 + 20), t, font=big, fill=(90, 230, 120) if done else (255, 255, 255))
    ff.stdin.write(img.tobytes())
    if j % 1000 == 0:
        print(f"{j}/{nframes + HOLD}", file=sys.stderr)
ff.stdin.close()
ff.wait()
print(f"final time {clock(nframes - 1)}", file=sys.stderr)
