# Second Dualray review follow-up — 2026-09-30

The JSON artifacts cited here were removed from the working tree to keep the
repository small. Restore one with `git restore --source=696f4c2 -- <path>`.

The fold regression now checks the oracle's selected face and uses an input
inside the narrowed window. Compensated arithmetic is shared by Dualray,
Bottosson and the iterative fold solver: FMA-enabled builds use hardware FMA,
while portable builds use native two-product/two-sum arithmetic. No accuracy
budget changed.

The machine-readable report (`rust/reports/dualray-review-followup.json`) includes source and
binary hashes, validation, the assembly audit and every timing row. The baseline
is the first review's fixes, with the cached-cubic borrow change already applied.
Its binary hash matches the final binary in `rust/reports/dualray-review-fixes.json`.

## Changes

- The explicit Rec.2020 reproducer is `[0.45, 0.4, 245.1]`, with an assertion
  that it lies inside the fold window. The oracle identifies its lower face;
  the mapped channel must be exactly zero. Dense sweeps check that a snapped
  face belongs to the oracle boundary and reject any extra, incorrect face.
  Corner ties allow either incident face within the precision-specific slack.
- `compensated.rs` supplies the product residual and compensated sum in each
  precision lane. `#[cfg(target_feature = "fma")]` selects hardware FMA;
  the fallback stays in the lane's native precision. Its compensated sum is
  accurate for these bounded operands, not a general correctly-rounded IEEE
  FMA emulator. This also removes portable software `fmaf` dependencies from
  Bottosson and the iterative solver's cancellation-sensitive fold evaluation.
- The f64 fold solver bounds first-exit isolation by the input chroma for
  inside endpoints. It still checks every face for an earlier exit/re-entry,
  so an in-gamut outer island cannot bypass the first-exit policy. Outside
  endpoints retain the existing fast upper-face refinement.
- The f32 direction helper consumes the already-reduced hue. There is no
  extra remainder in the fold helper and no second f32 window check.
- Validation and the oracle reuse production's window membership policy,
  including direct negative-hue comparisons. The oracle's physical XYZ
  conversion and boundary geometry remain independent. Tests explicitly cover
  adjacent f32 values at positive and negative edges.
- The generator requires a fold window, removes the dead fallback and asserts
  a minimum 0.02-degree margin around both the sector switch and fold point.
  Current minimum margins are 0.022024 degrees for sRGB and 0.025606 degrees
  for Rec.2020. Both generation and `--check` run the actual Rust f32 edge
  regression; a missing or renamed test cannot silently pass with zero tests.
- Earlier milestone counts are labelled as historical checkpoints. The current
  suite contains 111 tests.

## Validation

All 111 release tests pass with `target-cpu=native` and `target-cpu=x86-64`.
The native build is warning-free. All-gamut startup validation, generated-table
freshness, Cargo formatting and explicit formatting of included modules pass.
Scratch mutation checks confirm that both removing the exit snap and assigning
it to the wrong channel make the strengthened regression fail.

The new edge regression visits every f32 hue within 0.02 degrees of both window
edges, with positive, negative and wrapped variants; nine lightness values,
including immediately below white; and three chromas, including the independent
oracle boundary. Both builds produce the same measured maxima:

| Target | Mappings | Max linear RGB error | Max DeltaEOK |
| --- | ---: | ---: | ---: |
| sRGB | 212,382 | 6.210e-6 | 1.264e-6 |
| Rec.2020 | 424,926 | 5.031e-6 | 1.163e-6 |

The limits remain `2e-5` linear RGB and `5e-6` DeltaEOK. These 637,308 mappings
supplement the existing oracle corpus. They establish sampled edge coverage,
not a full-domain or cross-platform proof.

All **8,217,600** sampled output arrays are bit-identical to the previous fixes
in the native build: 136,960 inputs, five affected methods, three targets, both
precisions and both entry modes. This includes all 2,739,200 P3 arrays and all
4,108,800 f32 arrays in that comparison. Portable builds are checked against
their accuracy limits; portable/native bit identity is not promised.

Assembly audits traverse 30 f32 wrappers and their local callees in each build:
Dualray, direct/cached Bottosson, Halley and Ostrowski, all targets and both
entry modes. Neither build contains f64 arithmetic/conversions or software
`fma`/`fmaf` calls in those paths. The native assembly contains hardware f32 FMA;
the portable assembly does not. Outlined Dualray fold helpers contain no
`fmodf` calls. Standard f32 math-library internals are outside this audit.

## Timing

Ryzen 7 9800X3D, WSL2, rustc 1.98.1, release/LTO. Both binaries run on CPU 2,
with ASLR disabled and the same executable pathname, in before/after/after/before
order. Results are medians of two process medians. Every timed pass consumes
all three outputs. No builds, tests or competing benchmarks run during timing.

Focused fold inputs use 8,192 evenly spaced hues: sRGB 264.04–264.22 degrees,
Rec.2020 245.05–245.30 degrees, at L=0.45. Outside inputs have C=0.4; interior
inputs have C=0.001. Each run uses 10 warmup and 25 measured passes.

| Native focused case | Before, ns | After, ns | Change |
| --- | ---: | ---: | ---: |
| sRGB f32 outside fold | 773.63 | 504.84 | -34.7% |
| Rec.2020 f32 outside fold | 883.05 | 576.85 | -34.7% |
| sRGB f32 interior fold | 369.37 | 245.53 | -33.5% |
| Rec.2020 f32 interior fold | 361.92 | 239.83 | -33.7% |
| sRGB f64 outside fold | 522.28 | 519.99 | -0.4% |
| Rec.2020 f64 outside fold | 534.23 | 532.69 | -0.3% |
| sRGB f64 interior fold | 532.24 | 84.63 | -84.1% |
| Rec.2020 f64 interior fold | 541.17 | 82.78 | -84.7% |

Portable f32 fold costs are effectively unchanged (790/894 ns for sRGB/Rec.2020),
as expected without hardware FMA. Portable f64 interior folds improve by about
80%, to 90/88 ns. Native near-white f32 fold cases improve by 37%, to 492/494 ns.
Fold handling still costs substantially more than ordinary mapping; this change
does not eliminate that numerical-conditioning cost.

The full native harness uses `--gamut all`, 35,640 inputs per workload, 50 warmup
and 25 measured passes. All 156 method/target/precision/workload comparisons and
624 raw rows are retained in the JSON. This follow-up measures plain mode;
Dualray's two entry modes use the same implementation, but checked timings for
other methods are not inferred from these rows.

| Full-harness Dualray | Before, ns | After, ns | Change |
| --- | ---: | ---: | ---: |
| display-p3 f64 grid | 52.41 | 52.84 | +0.8% |
| display-p3 f64 random | 66.00 | 66.19 | +0.3% |
| display-p3 f32 grid | 41.05 | 40.94 | -0.3% |
| display-p3 f32 random | 53.02 | 52.17 | -1.6% |
| srgb f64 grid | 52.33 | 52.28 | -0.1% |
| srgb f64 random | 66.00 | 65.61 | -0.6% |
| srgb f32 grid | 41.44 | 41.27 | -0.4% |
| srgb f32 random | 52.91 | 53.00 | +0.2% |
| rec2020 f64 grid | 56.09 | 56.34 | +0.5% |
| rec2020 f64 random | 67.31 | 66.06 | -1.9% |
| rec2020 f32 grid | 43.20 | 43.30 | +0.2% |
| rec2020 f32 random | 51.72 | 51.54 | -0.3% |

Dualray grid/random changes range from -1.9% to +0.8%. Across all
156 comparisons, changes range from -4.1% to +2.5%; none exceeds a 5% slowdown.

Small changes include placement/noise effects; the full-build comparison does
not isolate generic dispatch or arithmetic instructions. The focused cases
provide the direct evidence for the fold improvements.
