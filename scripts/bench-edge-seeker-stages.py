#!/usr/bin/env python3
"""Compare Edge Seeker stages and a normalized-hue fast path in scratch builds.

Uses the working Rust sources and canonical JS workloads. Serial fresh processes,
rotated/reversed order, native release/LTO, 50 warmup and 25 measured passes.
Stage inputs are precomputed: stage timings must not be added or subtracted as
an accounting of total cost. Full mapper timings are measured independently.
Pass --cpu on Linux to pin each measurement process. No source edits are made.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import statistics
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
LEGACY = """fn normalized_hue(h: Float) -> Float {
    if h < 0.0 {
        (h % 360.0) + 360.0
    } else {
        h % 360.0
    }
}"""
FAST = """fn normalized_hue(h: Float) -> Float {
    if h >= 0.0 && h < 360.0 {
        h
    } else if h < 0.0 {
        (h % 360.0) + 360.0
    } else {
        h % 360.0
    }
}"""
CALLBACK = '''        map_with_lookup::<G>(oklch, out, check_in_gamut, || {
            self.max_chroma(oklch[0], oklch[2])
        });'''
DIRECT = '''        if check_in_gamut && oklch_to_rgb_if_in_gamut::<G>(oklch[0], oklch[1], oklch[2], out) {
            return;
        }
        let max_chroma = self.max_chroma(oklch[0], oklch[2]);
        map_edge_seeker::<G>(oklch, max_chroma, out);'''
WRAPPER = '''#[inline(always)]
fn map_with_lookup<G: EdgeSeekerData>(
    input: &[Float; 3],
    out: &mut [Float; 3],
    check: bool,
    lookup: impl FnOnce() -> Float,
) {
    if check && oklch_to_rgb_if_in_gamut::<G>(input[0], input[1], input[2], out) {
        return;
    }
    map_edge_seeker::<G>(input, lookup(), out);
}

'''
EXAMPLE = r'''
#![allow(dead_code, unused_imports, unused_macros)]
#[path = "../src/dualray_config.rs"] mod dualray_config;
#[path = "../src/rgb_spaces.rs"] mod rgb_spaces;
mod float64 {
    type Float = f64;
    const SINGLE: bool = false;
    include!("../src/algorithms.rs");
}
mod float32 {
    type Float = f32;
    const SINGLE: bool = true;
    include!("../src/algorithms.rs");
}
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    assert_eq!(args.len(), 3, "edge-stages f32|f64 STAGE INPUT");
    let bytes = std::fs::read(&args[2]).unwrap();
    assert!(!bytes.is_empty() && bytes.len() % 24 == 0);
    let input: Vec<[f64; 3]> = bytes.chunks_exact(24).map(|r| {
        std::array::from_fn(|i| f64::from_le_bytes(r[i*8..i*8+8].try_into().unwrap()))
    }).collect();
    assert!(input.iter().all(|s| s.iter().all(|v| v.is_finite()) && s[0] > 0.0 && s[0] < 1.0));
    match args[0].as_str() {
        "f32" => float32::edge_seeker::profile::<rgb_spaces::DisplayP3>(&args[1], &input),
        "f64" => float64::edge_seeker::profile::<rgb_spaces::DisplayP3>(&args[1], &input),
        _ => panic!("expected f32 or f64"),
    }
}
'''
WORKLOADS = r'''
import { buildWorkloads } from './benchmark-workloads.js';
import { writeFileSync } from 'node:fs';
const { samples, randomSamples } = buildWorkloads();
for (const [name, rows] of [['grid', samples], ['random', randomSamples]]) {
  const bytes = Buffer.alloc(rows.length * 24);
  rows.forEach((row,i) => row.forEach((v,j) => bytes.writeDoubleLE(v, i*24+j*8)));
  writeFileSync(`${process.argv[1]}/${name}.bin`, bytes);
}
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--runs', type=int, default=5)
    parser.add_argument('--cpu', type=str)
    parser.add_argument('--prepare-only', action='store_true')
    parser.add_argument('--structure', action='store_true',
                        help='also test direct mapping calls and an outlined indexed lookup')
    parser.add_argument('--full-runs', type=int, default=0,
                        help='full-harness processes per variant, after the stage measurements')
    args = parser.parse_args()
    assert args.runs >= 3
    assert args.full_runs >= 0
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    env = {**os.environ, 'RUSTFLAGS': '-C target-cpu=native'}
    versions = ['legacy', 'fast-hue']
    if args.structure:
        versions += ['direct-call', 'lookup-noinline', 'direct-fast']
    report = dict(
        compiler=subprocess.check_output(['rustc', '-Vv'], text=True),
        machine=platform.uname()._asdict(),
        commit=subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
        status=subprocess.check_output(['git', 'status', '--short'], cwd=ROOT, text=True),
        rustflags=env['RUSTFLAGS'], runs=args.runs, cpu=args.cpu,
        warmup=50, measured=25, gamut='display-p3', mode='plain',
        sourceHashes={}, inputHashes={}, binaryHashes={}, measurements=[],
        fullRuns=args.full_runs, fullMeasurements=[],
    )
    digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
    for path in sorted((ROOT / 'rust/src').rglob('*.rs')):
        report['sourceHashes'][str(path.relative_to(ROOT))] = digest(path)
    for name in ['scripts/bench-edge-seeker-stages.py', 'scripts/templates/edge-seeker-stages.rs',
                 'benchmark-workloads.js', 'rust/Cargo.toml', 'rust/Cargo.lock']:
        report['sourceHashes'][name] = digest(ROOT / name)
    subprocess.run(['node', '--input-type=module', '-e', WORKLOADS, str(output)], cwd=ROOT, check=True)
    for workload in ['grid', 'random']:
        report['inputHashes'][workload] = digest(output / f'{workload}.bin')
    template = (ROOT / 'scripts/templates/edge-seeker-stages.rs').read_text()
    (output / 'probe-template.rs').write_text(template)
    shutil.copy2(__file__, output / 'runner.py')
    for version in versions:
        dest = output / version
        dest.mkdir()
        shutil.copytree(ROOT / 'rust/src', dest / 'src')
        for name in ['Cargo.toml', 'Cargo.lock']:
            shutil.copy2(ROOT / 'rust' / name, dest / name)
        edge = dest / 'src/edge_seeker.rs'
        source = edge.read_text()
        # Reconstruct the old callback shape when replaying from the fixed tree.
        if source.count(DIRECT) == 2:
            source = source.replace(DIRECT, CALLBACK)
            if 'fn map_with_lookup<' not in source:
                source = source.replace('pub(crate) struct EdgeSeeker<G>',
                                        WRAPPER + 'pub(crate) struct EdgeSeeker<G>')
        present = LEGACY if LEGACY in source else FAST
        assert source.count(present) == 1, 'normalization helper changed; update the experiment'
        source = source.replace(present, FAST if version in ['fast-hue', 'direct-fast'] else LEGACY)
        if version in ['direct-call', 'direct-fast']:
            assert source.count(CALLBACK) == 2, 'mapping wrapper changed; update the experiment'
            source = source.replace(CALLBACK, DIRECT)
        elif version == 'lookup-noinline':
            source = source.replace('fn get_lut_item_indexed<',
                                    '#[inline(never)]\nfn get_lut_item_indexed<')
        edge.write_text(source + '\n' + template)
        (dest / 'examples').mkdir()
        (dest / 'examples/edge-stages.rs').write_text(EXAMPLE)
        print(f'Building {version}', flush=True)
        with (output / f'build-{version}.log').open('w') as log:
            subprocess.run(['cargo', 'build', '--release', '--bin', 'gma-bench', '--example', 'edge-stages'],
                           cwd=dest, env={**env, 'CARGO_TARGET_DIR': str(dest / 'target')},
                           stdout=log, stderr=subprocess.STDOUT, check=True)
        for binary in ['gma-bench', 'examples/edge-stages']:
            report['binaryHashes'][f'{version}/{binary}'] = digest(dest / 'target/release' / binary)
    def save():
        (output / 'results.json').write_text(json.dumps(report, indent=2) + '\n')
    save()
    if args.prepare_only:
        return
    stages = ['read-hue', 'normalize', 'lookup-only', 'lookup', 'boundary', 'lookup-boundary',
              'conversion', 'full-indexed', 'full-binary', 'cubic-cached', 'clip']
    cells = [(precision, workload, stage) for precision in ['f32', 'f64']
             for workload in ['grid', 'random'] for stage in stages]
    for run in range(args.runs):
        offset = len(cells) * run // args.runs
        order = cells[offset:] + cells[:offset]
        if run % 2:
            order.reverse()
        for precision, workload, stage in order:
            pair = []
            for version in versions[::(-1 if run % 2 else 1)]:
                binary = output / version / 'target/release/examples/edge-stages'
                command = [str(binary), precision, stage, str(output / f'{workload}.bin')]
                if args.cpu:
                    command = ['taskset', '-c', args.cpu] + command
                result = json.loads(subprocess.check_output(command, text=True))
                report['measurements'].append(dict(run=run, version=version, precision=precision,
                                                   workload=workload, stage=stage, **result))
                pair.append(result)
            assert all(r['fingerprint'] == pair[0]['fingerprint'] for r in pair), (precision, workload, stage)
            assert all(r['checksum'] == pair[0]['checksum'] for r in pair), (precision, workload, stage)
        save()
        print(f'Finished process round {run+1}/{args.runs}', flush=True)
    summary = []
    for precision, workload, stage in cells:
        values = {version: [r['ns'] for r in report['measurements']
                            if (r['version'], r['precision'], r['workload'], r['stage']) ==
                            (version, precision, workload, stage)] for version in versions}
        before = statistics.median(values['legacy'])
        for variant in versions[1:]:
            after = statistics.median(values[variant])
            summary.append(dict(precision=precision, workload=workload, stage=stage, variant=variant,
                                processes={v: values[v] for v in ['legacy', variant]},
                                before=before, after=after, percent=100*(after/before-1)))
            if precision == 'f32':
                print(f'{variant:15} {workload:6} {stage:16} {before:7.2f} -> {after:7.2f} ns ({100*(after/before-1):+.1f}%)')
    report['summary'] = summary
    save()
    for run in range(args.full_runs):
        for variant in versions[::(-1 if run % 2 else 1)]:
            command = [str(output / variant / 'target/release/gma-bench')]
            if args.cpu:
                command = ['taskset', '-c', args.cpu] + command
            log = subprocess.check_output(command, text=True)
            (output / f'full-{run}-{variant}.txt').write_text(log)
            precision = workload = None
            rows = []
            for line in log.splitlines():
                header = re.match(r'── display-p3 / (f32|f64) / (grid|random)', line)
                if header:
                    precision, workload = header.groups()
                timing = re.match(r'  (.+?)\s+([0-9.]+) ns/call', line)
                if timing and precision:
                    method, ns = timing.groups()
                    rows.append(dict(precision=precision, workload=workload, method=method, ns=float(ns)))
            assert len(rows) == 60
            report['fullMeasurements'].append(dict(run=run, variant=variant, rows=rows))
            save()
            print(f'Full harness {run+1}/{args.full_runs}: {variant}', flush=True)
    report['completed'] = time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())
    save()


if __name__ == '__main__':
    main()
