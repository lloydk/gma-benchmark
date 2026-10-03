# Dualray review follow-up — 2026-09-30

The JSON artifacts cited here were removed from the working tree to keep the
repository small. Restore one with `git restore --source=696f4c2 -- <path>`.

The f32 fold solver now sets its selected exit channel to exactly zero or one.
On the benchmark validation workloads, Rec.2020's maximum f32/f64 encoded
difference falls from `4.869e-4` to `1.162e-5`. The first-exit policy and all
existing accuracy budgets are retained. Interior inputs do not snap to a face.

The machine-readable report (`rust/reports/dualray-review-fixes.json`) records source and
binary hashes, checks, input manifests, assembly audits and every timing row.
The baseline is the uncommitted Dualray port **with the cached-cubic borrow fix
already applied**, as recorded in `rust/reports/cached-cubic-borrow-fix.json`.

## Changes

- Compensated multiplication uses a native two-product split instead of
  `mul_add`. Portable builds no longer depend on a software `fmaf` call.
  All split constants, including the degree conversion, use const contexts.
- The f32 solver caps searches at the input chroma and the nearest exit found,
  tries up to eight safeguarded Newton steps, and retains bounded bisection
  for rejected or poorly conditioned steps.
- The f64 fold path skips seed evaluation and Halley polishing, and each
  lower-face solve uses the smallest root found so far as its limit.
- Dualray and the iterative solvers share the same fold membership function
  and f32-rounded window endpoints: sRGB `[264.03,264.23]`, Rec.2020
  `[245.04,245.31]`. Ordinary authored hues use range comparisons; only hues
  outside `(-360,360)` need a remainder. Negative hues are checked directly.
- The regenerated fits exclude these narrower windows. Generation asserts
  that each contains both the sector switch and fold; compile-time assertions
  bound the small-angle series domain. A single generated basis supplies both
  the ordinary and compensated constants through compile-time conversion.
- Validation maps direct cubic once per sample and removes the redundant
  P3-only Dualray comparison. The generic comparison still covers all targets.
- The encoded-gate injection is now `[0.00011f32,1,1]` against `[0,1,1]`.
  It explicitly passes both other budgets and must fail with the encoded-budget
  diagnostic. The earlier injection could fail on DeltaEOK alone.

The reported NaN omission was already fixed in the reviewed tree: outputs and
metrics are checked for finiteness before updating maxima. Injection tests now
cover both lanes and both modes for every policy, with NaN and both infinities.

## Correctness evidence

All 108 Rust tests pass in native and portable x86-64 release builds.
The native build is warning-free; all-gamut startup validation, generator
freshness and formatting checks pass. Assembly checks traverse all six f32
target/mode entry points and their local callees in both builds: no double
arithmetic, scalar widening or `fma`/`fmaf` calls. Standard f32 math-library
internals are outside that assembly audit.

The independent XYZ/physical-chroma oracle checks 14,601,024 mappings across
both precisions and modes. Coverage includes grids, boundary neighbours,
near-white scales, tiny/subnormal lightness, primary/secondary neighbourhoods,
dense fold sweeps and the area outside the narrowed windows. Counts include
the identical checked-mode mappings. These are corpus results, not a
full-domain or cross-platform proof.

| Target | f32 mappings | f64 mappings | Max f32 linear RGB | Max f32 DeltaEOK |
| --- | ---: | ---: | ---: | ---: |
| sRGB | 2,796,300 | 3,046,860 | 1.227e-5 | 3.450e-6 |
| Display-P3 | 1,332,072 | 1,582,632 | 6.816e-6 | 2.116e-6 |
| Rec.2020 | 2,796,300 | 3,046,860 | 9.555e-6 | 2.933e-6 |

The maximum f64 linear error is `6.502e-12`, with DeltaEOK `1.524e-12`.
Separate regressions require an exact selected face on dense out-of-gamut
fold inputs, preserve interior inputs, and cover wrapped window endpoints,
tangent touches, first crossings and compensated-product cancellation.

A before/after comparison of 136,960 inputs per target checks 1,643,520 output
arrays across both modes and precisions. All 547,840 P3 arrays are bit-identical.
New-target f64 changes stay below `5.5e-15` encoded RGB. The largest Rec.2020 f32
change is the corrected `4.869e-4` face residual. No general encoded budget was
added for sRGB/Rec.2020; the face regression tests directly enforce this fix.

## Focused timings

Ryzen 7 9800X3D, WSL2, rustc 1.98.1, CPU 2, ASLR disabled, identical executable
pathname. Both variants use release optimization, LTO, one codegen unit and
`panic=abort`. Each CPU setting uses before/after/after/before process order;
figures are medians of two process medians. Every timed pass consumes all three
output channels. No builds, tests or other benchmarks run concurrently.

Focused samples contain 8,192 hues at `L=0.45, C=0.4`, evenly spaced over
`264.04..264.22` for sRGB and `245.05..245.30` for Rec.2020. Each process uses
10 warmup and 25 measured passes. The portable build runs on this same FMA-capable
CPU; it is not a timing measurement of a processor without FMA.

| Native fold path, ns/call | Before | After | Change |
| --- | ---: | ---: | ---: |
| sRGB f64 | 978.05 | 522.09 | -46.6% |
| Rec.2020 f64 | 983.06 | 537.57 | -45.3% |
| sRGB f32 | 841.33 | 773.29 | -8.1% |
| Rec.2020 f32 | 1060.18 | 885.74 | -16.5% |

For the same hues at `L=0.999`, f32 falls from about 2.15–2.16 us to
0.78–0.81 us. Interior inputs with `C=0.001` fall from 0.86–1.06 us to
0.36–0.37 us. The portable f32 fold improves by 22.5% for sRGB and 28.5% for
Rec.2020. The compensated path remains slower than ordinary mapping, but its
windows are much narrower and contain no integer grid hues.

The focused executable's native f32 grid timings improve by about 21% for
both new targets; random timings improve by 13.5% for sRGB and 10.1% for
Rec.2020. Small P3 changes vary by workload and do not establish a P3 speedup.
Full-harness results, including unrelated methods and their placement-sensitive
timing changes, are recorded separately below and in the JSON report.

## Full-harness timings

The unchanged workload protocol uses 35,640 inputs, 50 warmup and 25 measured
passes, native release flags, and plain mode. The four full processes run
`--gamut all` in before/after/after/before order. Dualray's two modes are
identical; this timing comparison does not measure checked-mode placement.

| Target / precision | Grid before → after, ns/call | Random before → after, ns/call |
| --- | ---: | ---: |
| Display-P3 f64 | 52.93 → 55.11 (+4.1%) | 66.02 → 65.71 (-0.5%) |
| Display-P3 f32 | 41.07 → 41.80 (+1.8%) | 53.48 → 53.49 (+0.0%) |
| sRGB f64 | 59.26 → 52.43 (-11.5%) | 71.75 → 65.77 (-8.3%) |
| sRGB f32 | 52.59 → 41.62 (-20.9%) | 60.93 → 53.56 (-12.1%) |
| Rec.2020 f64 | 63.39 → 56.24 (-11.3%) | 72.51 → 66.91 (-7.7%) |
| Rec.2020 f32 | 54.02 → 44.45 (-17.7%) | 61.36 → 51.61 (-15.9%) |

The initial P3 f64 grid increase did not repeat in a separate four-process
P3-only ABBA comparison: `53.07 → 53.47 ns` (+0.7%) on grid and
`66.04 → 65.40 ns` (-1.0%) on random. P3 f32 measured -0.6% on grid and +1.4%
on random in that follow-up. Both result sets are retained. These small changes
have limited statistical weight; no P3 speedup is claimed. The JSON also retains
all unrelated-method rows, whose changes include increases up to 3.6% in the
all-target comparison.

The frozen baseline binary matches the earlier cached-cubic report exactly.
An initial run using a different existing executable was discarded. Final
formatting and constant placement produced a byte-identical release benchmark
binary, so the reported after timings apply to the current implementation.
