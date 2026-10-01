#!/usr/bin/env python3
"""Compare committed P3 implementations without changing the working sources.

Requires installed node_modules, Node, Bun, Linux taskset, and git. Raw results
and archived checkouts go to a new output directory. Runs serially; do not run
other benchmarks/builds alongside it. Default: four processes per version,
runtime and mode (two ABBA blocks), plus focused Dualray and output checks.
"""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import platform
import re
import shutil
import statistics
import subprocess
import tarfile
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--before", default="7d6c9e0")
parser.add_argument("--after", default="be03abd")
parser.add_argument("--output", type=Path, required=True)
parser.add_argument("--cpu", type=int, default=2)
parser.add_argument("--blocks", type=int, default=2)
args = parser.parse_args()
assert args.blocks > 0
repo = Path(__file__).resolve().parents[1]
root = args.output.resolve()
root.mkdir(parents=True, exist_ok=False)
result = {
    "description": "P3 pre-JavaScript-port versus current; serial ABBA blocks, fresh processes, common timed path, CPU affinity. Native full harness, both modes; separate focused Dualray and output checks. Setup excluded.",
    "machine": platform.uname()._asdict(),
    "cpu": subprocess.check_output(["lscpu"], text=True),
    "affinity": args.cpu, "blocks": args.blocks,
    "commits": {}, "sourceSha256": {}, "full": [], "focused": [],
    "validation": {}, "compatibility": {},
    "runners": {p.name: p.read_text() for p in [Path(__file__), repo / "scripts/compare-p3-outputs.mjs"]},
    "commands": [],
}

def run(command, cwd, log_name):
    start = time.monotonic()
    command = [str(x) for x in command]
    completed = subprocess.run(command, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    (root / log_name).write_text(completed.stdout)
    result["commands"].append({"command": command, "cwd": str(cwd), "log": log_name,
                               "seconds": time.monotonic() - start, "exitCode": completed.returncode})
    completed.check_returncode()
    return completed.stdout

def save():
    (root / "results.json").write_text(json.dumps(result, indent=2) + "\n")

for version, ref in [("before", args.before), ("after", args.after)]:
    sha = subprocess.check_output(["git", "rev-parse", ref], cwd=repo, text=True).strip()
    result["commits"][version] = sha
    dest = root / version
    dest.mkdir()
    archive = subprocess.check_output(["git", "archive", sha], cwd=repo)
    with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
        tar.extractall(dest, filter="data")
    (dest / "node_modules").symlink_to(repo / "node_modules", target_is_directory=True)
    files = sorted((dest / "src").rglob("*.js")) + [dest / "bench.js", dest / "package-lock.json"]
    result["sourceSha256"][version] = {str(p.relative_to(dest)): hashlib.sha256(p.read_bytes()).hexdigest() for p in files}

# The only difference in the timed harness is the current gamut-name prefix.
old = (root / "before/bench.js").read_text()
new = (root / "after/bench.js").read_text()
timing_start = 'console.log(`warmup:'
assert old[old.index(timing_start):] == new[new.index(timing_start):].replace(
    '`${gamut} / ${name}`', 'name').replace('`${gamut} / ${name} (random hues)`', '`${name} (random hues)`')
result["timedLoopsIdenticalAfterLabelNormalization"] = True

# Verify the extracted common workloads against the original inlined builder.
probe = old[old.index("const CHROMA ="):old.index("const oklchCubicChecked")]
probe += '\nconsole.log(JSON.stringify([samples, randomSamples]));\n'
(root / "old-workloads.mjs").write_text(probe)
inputs = json.loads(run(["node", "old-workloads.mjs"], root, "old-workloads.log").splitlines()[-1])
common = (root / "after/benchmark-workloads.js").read_text()
(root / "new-workloads.mjs").write_text(common + '\nconst {samples,randomSamples}=buildWorkloads();console.log(JSON.stringify([samples,randomSamples]));\n')
new_inputs = json.loads(run(["node", "new-workloads.mjs"], root, "new-workloads.log"))
assert inputs == new_inputs
result["workloadsIdentical"] = True
del inputs, new_inputs

# Existing independent validation runs in separate processes, before timing.
for runtime in ["node", "bun"]:
    flags = ["--expose-gc"] if runtime == "node" else []
    result["validation"][runtime] = {}
    for checked in [False, True]:
        mode = "checked" if checked else "plain"
        result["validation"][runtime][mode] = run(
            ["taskset", "-c", args.cpu, runtime, *flags, "bench.js", "--validate-only",
             *(["--in-gamut-check"] if checked else [])], root / "after", f"validate-{runtime}-{mode}.log")
    result["compatibility"][runtime] = json.loads(run(
        ["taskset", "-c", args.cpu, runtime, repo / "scripts/compare-p3-outputs.mjs", root / "before", root / "after"],
        root, f"outputs-{runtime}.json"))
    print(f"Validated {runtime}; output compatibility recorded", flush=True)
save()

# Baseline has no rgb-spaces module. P3 uses default exports in both versions;
# remove only the unused target lookup from the common focused harness.
focused = (root / "after/scripts/bench-rgb-kernels.mjs").read_text().replace(
    'const { getRgbSpace } = await import("../src/rgb-spaces.js");\nconst space = getRgbSpace(values.gamut);',
    'const space = null; // P3-only comparison; no target lookup needed.')
result["focusedRunner"] = focused

def parse_full(log):
    log = re.sub(r"\x1b\[[0-9;]*m", "", log)
    rows = []
    for match in re.finditer(r"^(.+?)\s+([\d.]+)\s+(ns|µs|us|ms|s)/iter", log, re.M):
        name = match[1].strip().removeprefix("display-p3 / ")
        random = name.endswith(" (random hues)")
        if random:
            name = name.removesuffix(" (random hues)")
        ns = float(match[2]) * {"ns": 1, "µs": 1e3, "us": 1e3, "ms": 1e6, "s": 1e9}[match[3]] / 35640
        rows.append({"method": name, "workload": "random" if random else "grid", "ns": ns})
    assert len(rows) == 26 and len({(r["method"], r["workload"]) for r in rows}) == 26
    assert "benchmark checksum:" in log
    return rows

for block in range(args.blocks):
    for position, version in enumerate(["before", "after", "after", "before"]):
        dest = root / "timed"
        if dest.exists():
            shutil.rmtree(dest)
        shutil.copytree(root / version, dest, symlinks=True)
        (dest / "benchmark-workloads.js").write_text(common)
        (dest / "scripts").mkdir(exist_ok=True)
        (dest / "scripts/bench-rgb-kernels.mjs").write_text(focused)
        # Alternate the runtime and mode order between blocks.
        runtimes = ["node", "bun"] if block % 2 == 0 else ["bun", "node"]
        for runtime in runtimes:
            for checked in ([False, True] if block % 2 == 0 else [True, False]):
                mode = "checked" if checked else "plain"
                log_name = f"full-{block}-{position}-{version}-{runtime}-{mode}.log"
                log = run(["taskset", "-c", args.cpu, runtime,
                    *(["--expose-gc"] if runtime == "node" else []), "bench.js", "--timing-only",
                    *(["--in-gamut-check"] if checked else [])], dest, log_name)
                result["full"].append({"block": block, "position": position, "version": version,
                    "runtime": runtime, "checked": checked, "rows": parse_full(log), "log": log})
                save()
                print(f"Full {block}:{position} {version} {runtime} {mode}", flush=True)
            log_name = f"focused-{block}-{position}-{version}-{runtime}.json"
            row = json.loads(run(["taskset", "-c", args.cpu, runtime, "scripts/bench-rgb-kernels.mjs", "--method", "dualray"], dest, log_name))
            row.update(block=block, position=position, version=version, runtimeName=runtime)
            result["focused"].append(row)
            save()

result["summary"] = []
for runtime in ["node", "bun"]:
    for checked in [False, True]:
        keys = {(r["method"], r["workload"]) for run_ in result["full"] for r in run_["rows"]}
        for method, workload in sorted(keys):
            values = [[r["ns"] for run_ in result["full"] if run_["runtime"] == runtime and run_["checked"] == checked and run_["version"] == version
                       for r in run_["rows"] if r["method"] == method and r["workload"] == workload] for version in ["before", "after"]]
            before, after = map(statistics.median, values)
            result["summary"].append({"runtime": runtime, "checked": checked, "method": method, "workload": workload,
                "beforeNs": before, "afterNs": after, "percent": 100 * (after / before - 1),
                "beforeProcesses": values[0], "afterProcesses": values[1]})
save()
print(f"Completed: {root / 'results.json'}", flush=True)
