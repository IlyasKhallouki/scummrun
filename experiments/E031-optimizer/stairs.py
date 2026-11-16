# The model's centre staircase in the plan's milestone order (reads estimate.py, changes nothing in it).
import sys
sys.path.insert(0, "experiments/E031-optimizer")
import estimate as E
C = dict((k, E.cen(k)) for k in E.REG)
fc = E.f_wait(C["r_inner"], C["r_xwait"])
sdc = E.s_drop(C["b_inroom"]); xc = E.x_drop(C["q_atom"])
pool3 = E.DTOT - E.S_WALK - sdc
u = C["u_frame"]
phi_w = E.S_WALK * C["v_walk"] / E.DTOT
phi_d = sdc * xc / E.DTOT
phi_n = pool3 * C["y_need"] / E.DTOT
phi_o = pool3 * (1 - C["y_need"]) * C["y_order"] / E.DTOT
phi_all = phi_w + phi_d + phi_n + phi_o
fb = C["phi_fb"]
def mm(s):
    return E.mmss(s)
args = dict(f=0.0, phi=0.0, e_walk=0.0, e_cut=0.0, u=u, ds=1.0, iota=0.0, logo=False, intro=False)
def row(name, **ch):
    args.update(ch)
    a, b = E.route_R(**args)
    print("%-52s %10s %10s   %8.1f %8.1f" % (name, mm(a), mm(b), a, b))
    return a, b
print("DTOT %.1f  S_WALK %.1f  sdc %.1f xc %.3f pool3 %.1f fc %.4f" % (E.DTOT, E.S_WALK, sdc, xc, pool3, fc))
print("phi_w %.3f phi_d %.3f phi_n %.3f phi_o %.3f all %.3f fb %.3f" % (phi_w, phi_d, phi_n, phi_o, phi_all, fb))
row("M0 full5, menus answered")
row("M1 re-timed", f=fc)
for lo_hi, f in (("M1 lo", E.f_wait(0.8, 0.6)), ("M1 hi", E.f_wait(0.99, 0.97))):
    a, b = E.route_R(f, 0, 0, 0, u, 1.0, 0.0, False, False); print("  %-50s %10s %10s" % (lo_hi, mm(a), mm(b)))
row("M2a intro skip", intro=True)
row("M2b logo (frame length 5)", logo=True)
keep = dict(args)
print("-- with logo kept --")
row("M3 loops+no-gain drops, no executor (phi_fb 0.18)", phi=fb)
for nm, v in (("  phi_fb 0.05", 0.05), ("  phi_fb 0.45", 0.45), ("  design's 1560 s", 1560.0 / E.DTOT), ("  walks+drops all (phi_w+phi_d)", phi_w + phi_d)):
    a, b = E.route_R(**dict(args, phi=v)); print("  %-50s %10s %10s  share %.3f" % (nm, mm(a), mm(b), v))
row("M4 direct walks realised (phi_w, plus max with fb)", phi=max(fb, phi_w))
row("M4+ drops of no-gain steps (phi_w+phi_d)", phi=phi_w + phi_d)
row("M5 unneeded progress (+phi_n)", phi=phi_w + phi_d + phi_n)
row("M6 order (+phi_o)", phi=phi_all)
row("M7 in-visit execution", e_walk=C["e_walk"], e_cut=C["e_cut"])
a, b = row("   interactions (iota)", iota=C["iota"])
ce = E.cov_exit(C["rng_values_in_scan"])
gl = E.CAT_GAIN * ce + C["fight_luck_s"]
gt = C["e_tune"] * (E.TUNE_FIXED + C["t_share"] * E.ORDER_POOL)
gw = E.N_WALK_ALL * C["walk_gain_rate"] * C["h_new"] * C["sv_draw"]
g = gl + gt + gw
print("G: luck %.1f tuning %.1f walking %.1f sum %.1f" % (gl, gt, gw, g))
print("%-52s %10s %10s" % ("M7 luck+tuning+walking", mm(a - g), mm(b - g)))
print("%-52s %10s %10s" % ("M8 head+feather", mm(a - g - E.HEAD - C["feather_net_s"]), mm(b - g - E.HEAD - C["feather_net_s"])))
print("-- without logo (frame length 6) --")
args = dict(keep, logo=False)
row("M3 no executor (phi_fb)", phi=fb)
row("M4 walks+drops", phi=phi_w + phi_d)
row("M5 +needs", phi=phi_w + phi_d + phi_n)
row("M6 +order", phi=phi_all)
row("M7 in-visit", e_walk=C["e_walk"], e_cut=C["e_cut"])
a, b = row("   iota", iota=C["iota"])
print("%-52s %10s %10s" % ("M7 luck+tuning+walking", mm(a - g), mm(b - g)))
print("-- executor fails: only phi_fb, logo kept, in-visit e_walk only (e_cut needs the executor) --")
args = dict(keep, logo=True)
row("fb only", phi=fb)
row("fb + e_walk", e_walk=C["e_walk"])
a, b = row("   iota", iota=C["iota"])
print("%-52s %10s %10s" % ("then luck+tuning+walking", mm(a - g), mm(b - g)))
