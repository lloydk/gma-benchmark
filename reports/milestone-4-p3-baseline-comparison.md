# P3 comparison against the pre-port implementation

Direct comparison of **`7d6c9e0` → `be03abd`** on the Ryzen 7 9800X3D under
WSL2, Node 26.10.0 and Bun 1.4.2. `7d6c9e0` is the last committed snapshot
before the JavaScript multi-gamut work; every method is compared with that
same snapshot. Its Dualray body is byte-identical to `1c06277`, immediately
before the Dualray port. Earlier ports changed the surrounding conversion
module layout, so this comparison deliberately uses the original whole tree.

The raw [measurement report](milestone-4-p3-baseline-comparison.json) contains
all process logs, individual timings, output comparisons, source/dependency
hashes, runtime versions, commands, and the exact runner sources.

## Findings

- **Dualray has recovered the substantial P3 throughput loss on these workloads.**
  Node full-harness medians are 1.7–2.7% faster than the original; Bun medians
  are 0.7–1.6% slower, with overlapping process ranges. The isolated Bun
  comparison is nearly flat: -0.2% grid and +0.6% random. This is evidence for
  recovery to roughly original throughput, not a proof of exact performance
  equality or a universal speedup.
- **Bottosson is the clearest remaining regression.** Uncached plain mapping
  is 4.5–8.3% slower. Checked uncached mapping is 8.0–21.4% slower, and checked
  cached mapping is 14.9–21.6% slower. Plain cached mapping improves about 6%
  in Node; Bun's plain cached grid is 10.6% slower and random is 1.9% slower.
- **Node Raytrace is 8.3–10.1% slower.** Bun Raytrace improves 7.0–11.3%.
  No other full-harness row regresses by more than 5% in these medians.
- These figures compare complete implementations, including the correctness
  and canonical-conversion changes made during the ports. They do not isolate
  abstraction overhead or justify removing numerical guards.

The next targeted investigation should be Bottosson, beginning with checked
mapping and Bun's cached grid path. Node Raytrace is a separate follow-up.
No mapper implementation was changed by this measurement pass.

## Controls and validation

- Two serial **ABBA blocks**: before/after/after/before, repeated. Four fresh
  processes per version/runtime/mode: **32 full-harness processes** in total.
  Runtime and plain/checked order reverse in the second block. All timed
  processes use CPU 2 and the same filesystem path. No tests, builds or other
  benchmark jobs were launched concurrently.
- The original and current native `bench.js` harnesses run all 13 methods in
  the same order. Their warmup, measured loops and three-channel output
  consumption are byte-identical after normalizing the printed gamut prefix.
  Node runs with `--expose-gc`; both use the same installed Mitata 1.0.34 and
  `@mitata/counters` 0.0.8. Factory/cache setup is excluded.
- Each workload has **35,640** reused-array OKLCh inputs at C=0.4. Grid: 360
  integer hues and L=0.99 through 0.01. Random: the same deterministic
  stratified/shuffled lightness and hue workload. The original inlined input
  builder and current shared builder were compared and produced identical
  arrays. These high-chroma workloads do not represent arbitrary in-gamut
  mixtures, extreme lightness or cold startup.
- Full-harness tables show the **median of four process averages**, in ns per
  mapping. Mitata's printed batch averages are divided by 35,640; their display
  rounding limits precision. Raw process samples and both blocks' percentage
  changes are retained. There is no statistical equivalence test here.
- **16 additional fresh processes** measure only Dualray: four per version
  and runtime, 50 full-workload warmups and 25 measured passes. Reported values
  are medians of process medians. The common focused script removes only an
  unused P3 descriptor lookup to run on the original pre-factory tree.
- Current P3 `--validate-only` passes in both modes under both runtimes,
  including the existing independent-reference checks. Validation and output
  comparison run before timing, in separate processes.
- A separate compatibility pass checks all 13 methods, both modes and both
  workloads: **1,853,280 before/after output pairs per runtime**. It poisons
  reused output buffers, asserts return-buffer identity, checks finite [0,1]
  outputs, consumes all channels, and records exact output hashes. The maximum
  encoded difference is **2.4206e-14**, with **6.6614e-16** for Dualray. For each
  runtime, 36 of 52 method/mode/workload rows are bit-identical. This describes
  only the timed inputs; it does not erase the documented boundary fixes.
- All production source hashes match `be03abd`. Rust is unchanged and was not
  re-benchmarked or retested for this JavaScript measurement pass.

## Dualray full-harness comparison

All values are ns per mapping. A positive percentage means slower.

| Runtime / mode | Grid, before → current | Random, before → current |
| --- | ---: | ---: |
| node / plain | 73.09 → 71.13 (-2.7%) | 90.91 → 89.37 (-1.7%) |
| node / checked | 73.09 → 71.83 (-1.7%) | 91.75 → 89.93 (-2.0%) |
| bun / plain | 63.83 → 64.25 (+0.7%) | 77.02 → 78.00 (+1.3%) |
| bun / checked | 63.55 → 64.25 (+1.1%) | 77.44 → 78.70 (+1.6%) |

Dualray uses the same intrinsic mapping checks in both benchmark modes. The
mode-dependent timing differences reflect the full-harness context, not an
extra checked Dualray implementation. Both Node blocks favor the current
version. Bun's plain comparisons change sign between blocks; checked Bun has
small positive changes in both. The per-process ranges overlap for Bun.

## Isolated Dualray comparison

Ranges below are the observed minimum and maximum process medians, not
confidence intervals.

| Runtime / workload | Before median [range], ns | Current median [range], ns | Change |
| --- | ---: | ---: | ---: |
| node / grid | 67.58 [66.19, 68.52] | 65.87 [65.13, 66.73] | -2.5% |
| node / random | 83.56 [83.14, 84.24] | 81.50 [80.88, 83.21] | -2.5% |
| bun / grid | 61.07 [60.25, 62.83] | 60.96 [60.82, 62.25] | -0.2% |
| bun / random | 74.46 [74.01, 74.86] | 74.91 [74.69, 77.37] | +0.6% |

## All methods: plain

Values are before → current ns per mapping (percentage change).

| Method | Node grid | Node random | Bun grid | Bun random |
| --- | ---: | ---: | ---: | ---: |
| clip | 48.12 → 46.30 (-3.8%) | 64.81 → 60.89 (-6.1%) | 42.79 → 42.37 (-1.0%) | 58.22 → 58.22 (+0.0%) |
| css-minde | 432.10 → 437.99 (+1.4%) | 487.93 → 486.25 (-0.3%) | 631.87 → 631.31 (-0.1%) | 685.33 → 680.13 (-0.8%) |
| oklch-cubic (cached) | 79.26 → 78.98 (-0.4%) | 106.76 → 106.48 (-0.3%) | 63.55 → 63.27 (-0.4%) | 85.44 → 85.58 (+0.2%) |
| oklch-cubic (no cache) | 249.58 → 251.68 (+0.8%) | 276.23 → 271.32 (-1.8%) | 238.64 → 237.23 (-0.6%) | 260.38 → 260.10 (-0.1%) |
| oklch-cubic-direct | 222.78 → 219.42 (-1.5%) | 248.88 → 241.16 (-3.1%) | 220.40 → 221.38 (+0.4%) | 249.72 → 247.19 (-1.0%) |
| oklch-halley | 113.92 → 108.02 (-5.2%) | 135.38 → 130.47 (-3.6%) | 95.12 → 94.14 (-1.0%) | 116.86 → 115.18 (-1.4%) |
| oklch-ostrowski | 114.34 → 106.62 (-6.7%) | 136.36 → 131.87 (-3.3%) | 96.10 → 97.36 (+1.3%) | 119.11 → 118.69 (-0.4%) |
| dualray | 73.09 → 71.13 (-2.7%) | 90.91 → 89.37 (-1.7%) | 63.83 → 64.25 (+0.7%) | 77.02 → 78.00 (+1.3%) |
| bottosson-lightness | 114.90 → 122.19 (+6.3%) | 133.98 → 140.01 (+4.5%) | 104.80 → 113.50 (+8.3%) | 121.63 → 130.33 (+7.2%) |
| bottosson-lightness (cached) | 66.64 → 62.71 (-5.9%) | 90.77 → 85.30 (-6.0%) | 51.77 → 57.24 (+10.6%) | 73.79 → 75.20 (+1.9%) |
| edge-seeker | 110.83 → 103.68 (-6.5%) | 165.40 → 160.91 (-2.7%) | 105.22 → 102.83 (-2.3%) | 149.13 → 148.01 (-0.8%) |
| edge-seeker (indexed) | 74.64 → 74.07 (-0.8%) | 91.47 → 91.05 (-0.5%) | 61.31 → 60.61 (-1.1%) | 79.55 → 79.12 (-0.5%) |
| raytrace | 207.21 → 224.47 (+8.3%) | 228.54 → 251.68 (+10.1%) | 262.91 → 233.16 (-11.3%) | 282.69 → 258.28 (-8.6%) |

## All methods: checked

Values are before → current ns per mapping (percentage change).

| Method | Node grid | Node random | Bun grid | Bun random |
| --- | ---: | ---: | ---: | ---: |
| clip | 47.98 → 46.44 (-3.2%) | 64.81 → 60.75 (-6.3%) | 43.35 → 43.21 (-0.3%) | 58.36 → 58.64 (+0.5%) |
| css-minde | 434.34 → 434.76 (+0.1%) | 486.11 → 487.23 (+0.2%) | 620.37 → 629.63 (+1.5%) | 663.86 → 680.70 (+2.5%) |
| oklch-cubic (cached) | 95.68 → 92.45 (-3.4%) | 130.19 → 126.68 (-2.7%) | 85.30 → 83.19 (-2.5%) | 111.25 → 110.41 (-0.8%) |
| oklch-cubic (no cache) | 265.43 → 261.92 (-1.3%) | 299.80 → 300.51 (+0.2%) | 253.09 → 252.67 (-0.2%) | 285.35 → 282.13 (-1.1%) |
| oklch-cubic-direct | 238.22 → 230.78 (-3.1%) | 267.40 → 255.89 (-4.3%) | 244.81 → 243.27 (-0.6%) | 278.20 → 271.32 (-2.5%) |
| oklch-halley | 129.49 → 106.76 (-17.6%) | 153.20 → 131.87 (-13.9%) | 117.00 → 117.28 (+0.2%) | 146.32 → 142.82 (-2.4%) |
| oklch-ostrowski | 130.05 → 106.06 (-18.4%) | 154.60 → 132.44 (-14.3%) | 117.00 → 117.99 (+0.8%) | 145.76 → 145.06 (-0.5%) |
| dualray | 73.09 → 71.83 (-1.7%) | 91.75 → 89.93 (-2.0%) | 63.55 → 64.25 (+1.1%) | 77.44 → 78.70 (+1.6%) |
| bottosson-lightness | 125.70 → 135.80 (+8.0%) | 143.38 → 157.27 (+9.7%) | 110.41 → 130.75 (+18.4%) | 128.79 → 156.29 (+21.4%) |
| bottosson-lightness (cached) | 68.46 → 78.98 (+15.4%) | 96.10 → 110.41 (+14.9%) | 59.76 → 72.67 (+21.6%) | 84.60 → 97.50 (+15.3%) |
| edge-seeker | 135.24 → 115.04 (-14.9%) | 165.40 → 158.67 (-4.1%) | 128.65 → 127.24 (-1.1%) | 163.72 → 161.34 (-1.5%) |
| edge-seeker (indexed) | 87.12 → 80.39 (-7.7%) | 110.83 → 106.20 (-4.2%) | 80.11 → 79.41 (-0.9%) | 102.27 → 100.45 (-1.8%) |
| raytrace | 207.35 → 225.31 (+8.7%) | 228.11 → 248.60 (+9.0%) | 260.66 → 235.55 (-9.6%) | 285.49 → 265.43 (-7.0%) |

## Reproduction

From the repository with its Node dependencies installed, Node and Bun on
PATH, Python 3.12+ and Linux `taskset` available:

```sh
python3 scripts/compare-p3-performance.py \
  --before 7d6c9e0 --after be03abd --cpu 2 --blocks 2 \
  --output /tmp/gma-p3-baseline-repeat
```

Use a new output directory and an available CPU. The runner archives both git
commits, shares the installed dependencies, checks the workloads/timing loops,
validates current P3, invokes `scripts/compare-p3-outputs.mjs`, and runs the
balanced full and focused comparisons serially. `results.json` retains the raw
logs, summary and runner source. Installed dependencies must match the recorded
versions/hashes to reproduce this environment. The checked-in evidence adds
focused medians, per-block percentages and the final source/dependency audit.

Import time, bundle size, other gamuts, other CPUs and Rust performance are
outside this comparison. The earlier inline-seed report still records the
separate setup and bundle-size tradeoff.
