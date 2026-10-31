"""Measure a native executable's idle memory and CPU usage on Linux."""

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import time


def counters(root):
    stat = (root / "stat").read_text().rsplit(")", 1)[1].split()
    status = (root / "status").read_text()
    switches = re.search(r"^voluntary_ctxt_switches:\s*(\d+)", status, re.M)
    return int(stat[11]) + int(stat[12]), int(switches[1])


def measure(args):
    executable = Path(args.executable).resolve(strict=True)
    environment = dict(os.environ, CUI_OVERRIDE_RENDER_BACKEND=args.backend)
    process = subprocess.Popen([str(executable)], env=environment)
    try:
        time.sleep(args.settle)
        if process.poll() is not None:
            raise RuntimeError(f"application exited with status {process.returncode}")
        root = Path("/proc") / str(process.pid)
        first_ticks, first_switches = counters(root)
        started = time.monotonic()
        time.sleep(args.seconds)
        last_ticks, last_switches = counters(root)
        elapsed = time.monotonic() - started
        memory = {
            key: int(value)
            for key, value in re.findall(
                r"^(Rss|Pss|Anonymous|Private_Clean|Private_Dirty|Shared_Clean|Shared_Dirty):\s*(\d+)",
                (root / "smaps_rollup").read_text(),
                re.M,
            )
        }
        return {
            "executable": str(executable),
            "backend": args.backend,
            "file_bytes": executable.stat().st_size,
            "memory_kib": memory,
            "sample_seconds": elapsed,
            "cpu_percent": (last_ticks - first_ticks)
            / os.sysconf("SC_CLK_TCK")
            / elapsed
            * 100,
            "main_thread_voluntary_switches_per_second": (last_switches - first_switches)
            / elapsed,
        }
    finally:
        process.terminate()
        try:
            process.wait(timeout=3)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()


def positive_seconds(value):
    seconds = float(value)
    if not 0 < seconds <= 60:
        raise argparse.ArgumentTypeError("duration must be greater than 0 and at most 60 seconds")
    return seconds


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("executable")
    parser.add_argument("--backend", choices=["cpu", "gpu"], default="cpu")
    parser.add_argument("--settle", type=positive_seconds, default=3)
    parser.add_argument("--seconds", type=positive_seconds, default=3)
    args = parser.parse_args()
    print(json.dumps(measure(args), indent=2))


if __name__ == "__main__":
    main()
