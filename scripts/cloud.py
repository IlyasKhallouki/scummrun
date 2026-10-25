"""Run heavy jobs on one Google Cloud VM without crossing a hard spending cap.

usage: python3 scripts/cloud.py status
       python3 scripts/cloud.py create --type n2-standard-8 --hours 1
       python3 scripts/cloud.py start --type n2-standard-64 --hours 2
       python3 scripts/cloud.py up --job NAME              (copy the inputs to the VM)
       python3 scripts/cloud.py sh --job NAME -- CMD...    (run CMD in the VM's repo copy)
       python3 scripts/cloud.py down --job NAME REMOTE_DIR (copy a VM folder to out/cloud/down/)
       python3 scripts/cloud.py release --job NAME
       python3 scripts/cloud.py stop [--force]
       python3 scripts/cloud.py delete

One job holds the VM at a time: up and sh take the VM's lock for --job, and refuse while another job
holds it; release frees it, and stop refuses while a job holds it unless --force.

The cap holds in two places:
1. Google stops the VM by itself at an absolute termination time, set before every start. It holds if
   this script, the shell or the laptop dies.
2. The ledger (out/cloud/ledger.jsonl) refuses a start or a create whose full allowance, added to what
   is already committed, would pass STOP_AT. A run still open is counted at its full allowance; a closed
   run at its measured time. The disk is booked up front until its planned deletion, and a reserve covers
   the copies back.

Prices are read live from Google's price list (Cloud Billing Catalog) at every start: Spot N2 cores and
RAM, the Spot VM's external IP, and zonal balanced disk.
"""

import argparse
import datetime as dt
import json
import math
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LEDGER = ROOT / "out" / "cloud" / "ledger.jsonl"
DOWN = ROOT / "out" / "cloud" / "down"

PROJECT = "lumind-mvp"
ZONE = "us-central1-a"
NAME = "fast-mi-1"
LABELS = "purpose=fast-mi"
IMAGE = ("ubuntu-os-cloud", "ubuntu-2404-lts-amd64")
DISK_GB = 30
DISK_DAYS = 7  # planned life of the disk; booked in full at create
CAP = 20.00
STOP_AT = 19.50  # the ledger refuses anything past this, leaving room for price drift
EGRESS_RESERVE = 0.25  # copies back are megabytes; Internet egress is about $0.12 per GB
REMOTE = "fast-mi"  # the repo copy, in the VM user's home

# What the model check, the reader, the planner and the engine read. Game-derived files (the decompiled
# scripts, the room blocks) and the game files themselves go to the VM's private disk, which is deleted at
# the end. The engine's bridge patch goes too; ScummVM and Fast Downward are cloned on the VM at their pins.
UP = ["src", "scripts", "experiments", "data", "build/blocks-dos-cd", "runs/anypercent/noslide-p7",
      "runs/anypercent/verify-b1", "pyproject.toml", "uv.lock", "README.md", "patches", "game/dos-cd"]

COMPUTE = "6F81-5844-456A"
SKU = {"core": "Spot Preemptible N2 Instance Core running in Americas",
       "ram": "Spot Preemptible N2 Instance Ram running in Americas",
       "ip": "External IP Charge on a Spot Preemptible VM",
       "disk": "Balanced PD Capacity"}


def now() -> dt.datetime:
    return dt.datetime.now(dt.timezone.utc)


def gcloud(*args: str, check: bool = True, capture: bool = True) -> str:
    cmd = ["gcloud", *args, f"--project={PROJECT}"]
    r = subprocess.run(cmd, text=True, capture_output=capture)
    if check and r.returncode:
        sys.exit(f"gcloud failed ({r.returncode}): {' '.join(cmd)}\n{r.stderr if capture else ''}")
    return r.stdout if capture else ""


def prices() -> dict:
    """Live unit prices in USD: per core-hour, per GiB-hour of RAM, per IP-hour, per GiB-month of disk."""
    tok = subprocess.check_output(["gcloud", "auth", "print-access-token"], text=True).strip()
    url = f"https://cloudbilling.googleapis.com/v1/services/{COMPUTE}/skus?pageSize=5000&currencyCode=USD"
    found, page = {}, ""
    while True:
        req = urllib.request.Request(url + (f"&pageToken={page}" if page else ""),
                                     headers={"Authorization": f"Bearer {tok}"})
        d = json.load(urllib.request.urlopen(req, timeout=60))
        for s in d.get("skus", []):
            for k, desc in SKU.items():
                if s["description"] == desc and {"us-central1", "global"} & set(s.get("serviceRegions", [])):
                    rate = s["pricingInfo"][0]["pricingExpression"]["tieredRates"][-1]["unitPrice"]
                    found[k] = int(rate.get("units", "0")) + rate["nanos"] / 1e9
        page = d.get("nextPageToken")
        if not page:
            break
    missing = set(SKU) - set(found)
    if missing:
        sys.exit(f"price list: no price for {sorted(missing)}; refusing to start without one")
    return found


def shape(machine: str) -> tuple[int, float]:
    """vCPUs and GiB of RAM of a machine type, from Google."""
    d = json.loads(gcloud("compute", "machine-types", "describe", machine, f"--zone={ZONE}", "--format=json"))
    return int(d["guestCpus"]), d["memoryMb"] / 1024


def hourly(machine: str, p: dict) -> float:
    cpus, gib = shape(machine)
    return cpus * p["core"] + gib * p["ram"] + p["ip"]


def ledger() -> list[dict]:
    if not LEDGER.exists():
        return []
    return [json.loads(line) for line in LEDGER.read_text().splitlines() if line.strip()]


def write(rows: list[dict]) -> None:
    LEDGER.parent.mkdir(parents=True, exist_ok=True)
    LEDGER.write_text("".join(json.dumps(r) + "\n" for r in rows))


def committed(rows: list[dict]) -> float:
    """Spend counted against the cap: closed runs as measured, open runs at their full allowance."""
    total = 0.0
    for r in rows:
        if r["kind"] == "run":
            hours = r["hours"] if r.get("end") is None else r["actual_hours"]
            total += hours * r["usd_per_hour"]
        elif r["kind"] in ("disk", "reserve"):
            total += r["usd"]
    return total


def instance() -> dict | None:
    out = gcloud("compute", "instances", "list", f"--filter=name={NAME}", "--format=json")
    found = json.loads(out)
    return found[0] if found else None


def reconcile(rows: list[dict]) -> list[dict]:
    """Close an open run once the VM is stopped (by us, by Google's timer, or by a Spot preemption)."""
    vm = instance()
    for r in rows:
        if r["kind"] != "run" or r.get("end") is not None:
            continue
        if vm is None or vm["status"] in ("TERMINATED", "STOPPED"):
            stop = vm.get("lastStopTimestamp") if vm else None
            end = dt.datetime.fromisoformat(stop) if stop else now()
            start = dt.datetime.fromisoformat(r["start"])
            r["end"] = end.isoformat()
            # Google bills per second with a one-minute minimum; add a minute for start and stop.
            r["actual_hours"] = min(r["hours"], max((end - start).total_seconds(), 60) / 3600 + 1 / 60)
    return rows


def allow(rows: list[dict], cost: float, what: str) -> None:
    have = committed(rows)
    if have + cost > STOP_AT:
        sys.exit(f"refused: {what} would commit ${cost:.2f} on top of ${have:.2f}, past the ${STOP_AT:.2f} stop "
                 f"(cap ${CAP:.2f}); ${max(STOP_AT - have, 0):.2f} left")


def until(hours: float) -> str:
    return (now() + dt.timedelta(hours=hours)).strftime("%Y-%m-%dT%H:%M:%SZ")


def open_run(rows: list[dict], machine: str, hours: float, rate: float) -> None:
    rows.append({"kind": "run", "machine": machine, "hours": hours, "usd_per_hour": rate,
                 "start": now().isoformat(), "end": None})


def create(a) -> None:
    rows = reconcile(ledger())
    if instance():
        sys.exit(f"{NAME} exists already")
    p = prices()
    rate = hourly(a.type, p)
    disk = DISK_GB * p["disk"] * DISK_DAYS * 24 / 730
    allow(rows, a.hours * rate + disk + EGRESS_RESERVE, f"create ({a.type}, {a.hours} h, disk for {DISK_DAYS} days)")
    rows.append({"kind": "disk", "gb": DISK_GB, "days": DISK_DAYS, "usd": disk, "start": now().isoformat()})
    rows.append({"kind": "reserve", "what": "copies back (egress)", "usd": EGRESS_RESERVE})
    open_run(rows, a.type, a.hours, rate)
    write(rows)
    gcloud("compute", "instances", "create", NAME, f"--zone={ZONE}", f"--machine-type={a.type}",
           "--provisioning-model=SPOT", "--instance-termination-action=STOP",
           f"--termination-time={until(a.hours)}",
           f"--image-project={IMAGE[0]}", f"--image-family={IMAGE[1]}",
           f"--boot-disk-size={DISK_GB}GB", "--boot-disk-type=pd-balanced",
           f"--labels={LABELS}", "--no-service-account", "--no-scopes",
           "--metadata=block-project-ssh-keys=TRUE", capture=False)
    print(f"created {NAME} ({a.type}) at ${rate:.3f}/h; Google stops it at {until(a.hours)}")


def start(a) -> None:
    rows = reconcile(ledger())
    vm = instance()
    if vm is None:
        sys.exit(f"{NAME} does not exist; use create")
    if vm["status"] not in ("TERMINATED", "STOPPED"):
        sys.exit(f"{NAME} is {vm['status']}; stop it first")
    p = prices()
    rate = hourly(a.type, p)
    allow(rows, a.hours * rate, f"start ({a.type}, {a.hours} h)")
    if vm["machineType"].rsplit("/", 1)[-1] != a.type:
        gcloud("compute", "instances", "set-machine-type", NAME, f"--zone={ZONE}", f"--machine-type={a.type}")
    gcloud("compute", "instances", "set-scheduling", NAME, f"--zone={ZONE}",
           "--instance-termination-action=STOP", f"--termination-time={until(a.hours)}")
    open_run(rows, a.type, a.hours, rate)
    write(rows)
    r = subprocess.run(["gcloud", "compute", "instances", "start", NAME, f"--zone={ZONE}", f"--project={PROJECT}"])
    if r.returncode:
        vm = instance()
        if vm is None or vm["status"] in ("TERMINATED", "STOPPED"):
            rows[-1].update(end=now().isoformat(), actual_hours=0.0, failed=True)  # never ran: nothing billed
            write(rows)
        sys.exit(f"start failed ({r.returncode}); VM is {vm['status'] if vm else 'gone'}")
    print(f"started {NAME} ({a.type}) at ${rate:.3f}/h; Google stops it at {until(a.hours)}")


def stop(a) -> None:
    vm = instance()
    if vm and vm["status"] not in ("TERMINATED", "STOPPED"):
        keys()
        held = holder()
        if held and not a.force:
            sys.exit(f"refused: job '{held}' holds the VM; release it first, or stop --force")
        gcloud("compute", "instances", "stop", NAME, f"--zone={ZONE}", capture=False)
    write(reconcile(ledger()))
    status(a)


def delete(a) -> None:
    if instance():
        gcloud("compute", "instances", "delete", NAME, f"--zone={ZONE}", "--quiet", capture=False)
    rows = reconcile(ledger())
    for r in rows:
        if r["kind"] == "disk" and r.get("end") is None:
            r["end"] = now().isoformat()
    write(rows)
    status(a)


def status(a) -> None:
    rows = reconcile(ledger())
    write(rows)
    vm = instance()
    print(f"VM {NAME}: {vm['status'] + ' ' + vm['machineType'].rsplit('/', 1)[-1] if vm else 'none'}")
    for r in rows:
        if r["kind"] == "run":
            used = r["hours"] if r.get("end") is None else r["actual_hours"]
            state = "open, counted in full" if r.get("end") is None else "closed"
            print(f"  run {r['start'][:16]} {r['machine']:15s} {used:6.2f} h x ${r['usd_per_hour']:.3f} = "
                  f"${used * r['usd_per_hour']:.2f} ({state})")
        else:
            print(f"  {r['kind']:5s} ${r['usd']:.2f}")
    have = committed(rows)
    print(f"committed ${have:.2f} of the ${CAP:.2f} cap; the ledger stops at ${STOP_AT:.2f}, "
          f"${max(STOP_AT - have, 0):.2f} left")


def ssh_target() -> tuple[str, list[str]]:
    vm = instance()
    if vm is None or vm["status"] != "RUNNING":
        sys.exit(f"{NAME} is not running")
    ip = vm["networkInterfaces"][0]["accessConfigs"][0]["natIP"]
    key = Path.home() / ".ssh" / "google_compute_engine"
    opts = ["-i", str(key), "-o", "StrictHostKeyChecking=no", "-o", "UserKnownHostsFile=/dev/null",
            "-o", "ServerAliveInterval=30", "-o", "LogLevel=ERROR"]
    return ip, opts


def keys() -> None:
    """Let gcloud put this machine's key on the instance (not the project); waits for sshd."""
    for _ in range(30):
        r = subprocess.run(["gcloud", "compute", "ssh", NAME, f"--zone={ZONE}", f"--project={PROJECT}",
                            "--command=true", "--quiet", "--strict-host-key-checking=no"],
                           capture_output=True, text=True)
        if r.returncode == 0:
            return
        time.sleep(10)
    sys.exit(f"ssh to {NAME} did not come up:\n{r.stderr}")


def remote(cmd: str) -> subprocess.CompletedProcess:
    ip, opts = ssh_target()
    user = subprocess.check_output(["whoami"], text=True).strip()
    return subprocess.run(["ssh", *opts, f"{user}@{ip}", cmd], capture_output=True, text=True)


def holder() -> str:
    """The job that holds the VM's lock, or ''."""
    r = remote("cat ~/vm.lock 2>/dev/null || true")
    if r.returncode:
        sys.exit(f"ssh failed:\n{r.stderr}")
    return r.stdout.strip()


def lock(job: str) -> None:
    if not job or not job.replace("-", "").replace("_", "").isalnum():
        sys.exit("--job must be a name of letters, digits, - and _")
    keys()
    held = holder()
    if held and held != job:
        sys.exit(f"refused: the VM is held by job '{held}'")
    remote(f"echo {job} > ~/vm.lock")


def release(a) -> None:
    keys()
    held = holder()
    if held and held != a.job and not a.force:
        sys.exit(f"refused: the VM is held by job '{held}', not '{a.job}'")
    remote("rm -f ~/vm.lock")
    print(f"released (was '{held}')")


def up(a) -> None:
    lock(a.job)
    ip, opts = ssh_target()
    user = subprocess.check_output(["whoami"], text=True).strip()
    subprocess.run(["ssh", *opts, f"{user}@{ip}", f"mkdir -p {REMOTE}/build {REMOTE}/runs/anypercent {REMOTE}/game"],
                   check=True)
    for item in UP:
        dest = f"{user}@{ip}:{REMOTE}/{Path(item).parent}/" if "/" in item else f"{user}@{ip}:{REMOTE}/"
        # Cargo's target/ dirs stay out: the laptop's binaries are built for its own system, and --delete would
        # wipe the VM's builds on every copy. A worktree links its ignored files (data, results, run traces) into
        # the main checkout: --copy-unsafe-links copies what such a link points to.
        subprocess.run(["rsync", "-a", "--delete", "--exclude=__pycache__", "--exclude=.pytest_cache",
                        "--exclude=target/", "--copy-unsafe-links", "-e", "ssh " + " ".join(opts), str(ROOT / item),
                        dest], check=True)
    print(f"copied {len(UP)} items to {ip}:{REMOTE}")


def sh(a) -> None:
    lock(a.job)
    ip, opts = ssh_target()
    user = subprocess.check_output(["whoami"], text=True).strip()
    cmd = f"cd {REMOTE} && export PATH=$HOME/.local/bin:$PATH && " + " ".join(a.cmd)
    sys.exit(subprocess.run(["ssh", *opts, f"{user}@{ip}", cmd]).returncode)


def down(a) -> None:
    ip, opts = ssh_target()
    user = subprocess.check_output(["whoami"], text=True).strip()
    dest = DOWN / now().strftime("%Y%m%d-%H%M%S")
    dest.mkdir(parents=True)
    subprocess.run(["rsync", "-a", "-e", "ssh " + " ".join(opts), f"{user}@{ip}:{REMOTE}/{a.remote}", str(dest)],
                   check=True)
    print(dest)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd_name", required=True)
    for name, fn in (("create", create), ("start", start)):
        p = sub.add_parser(name)
        p.add_argument("--type", required=True)
        p.add_argument("--hours", type=float, required=True)
        p.set_defaults(fn=fn)
    for name, fn in (("status", status), ("delete", delete)):
        sub.add_parser(name).set_defaults(fn=fn)
    p = sub.add_parser("stop")
    p.add_argument("--force", action="store_true")
    p.set_defaults(fn=stop)
    p = sub.add_parser("release")
    p.add_argument("--job", required=True)
    p.add_argument("--force", action="store_true")
    p.set_defaults(fn=release)
    p = sub.add_parser("up")
    p.add_argument("--job", required=True)
    p.set_defaults(fn=up)
    p = sub.add_parser("sh")
    p.add_argument("--job", required=True)
    p.add_argument("cmd", nargs=argparse.REMAINDER)
    p.set_defaults(fn=sh)
    p = sub.add_parser("down")
    p.add_argument("--job", required=True)
    p.add_argument("remote")
    p.set_defaults(fn=down)
    a = ap.parse_args()
    if getattr(a, "hours", 1) <= 0 or getattr(a, "hours", 1) > 24 or math.isnan(getattr(a, "hours", 1)):
        sys.exit("--hours must be in (0, 24]")
    if a.cmd_name == "sh" and a.cmd[:1] == ["--"]:
        a.cmd = a.cmd[1:]
    a.fn(a)


if __name__ == "__main__":
    main()
