# Changelog

## 2026-10-03

- Added `dualray fast (tables)` to Node, Bun, and Rust f64/f32 benchmarks
  for sRGB, Display-P3, and Rec.2020. Above the cusp it corrects the upper
  solve's seed from a generated 360-byte table per gamut, so one Householder
  step usually converges. Rust also takes the hue direction from a 22.5°
  (cos, sin) table instead of `sin_cos`; JavaScript keeps `Math.cos` and
  `Math.sin`, because that table slowed V8's grid workload. The accuracy
  sweep's p99 and maxima match `dualray fast`; in sampled inputs individual
  outputs differ from it by up to about `5e-7` deltaEOK in f64 (Rec.2020
  bright yellow; about `1.4e-7` in sRGB and P3, near white) and `2e-6` in
  Rust f32 (near white and bright yellow). In PERFORMANCE.md's P3 workloads it takes
  10–11% less time than `dualray fast` in Rust f32 on random and grid input
  and 18% less above the cusp; Node and Bun save 2–5% and 6–7%. The other
  rows' outputs are unchanged. Added generator output for both
  languages, validation against `dualray fast`, JS/Rust parity, and direction
  and agreement tests.
- Re-measured PERFORMANCE.md with the new row in every runtime (rustc 1.99.0;
  Node and Bun unchanged). The report now names the fastest mapped row per
  runtime from the data, covers the tables row's above-cusp saving, and
  replaces the 2026-10-02 timing, Math-call and validation artifacts with
  2026-10-03 ones.

## 2026-10-02

- Added sRGB and Rec.2020 targets to every method in Node, Bun, and Rust
  f64/f32. Select one with `--gamut srgb` or `--gamut rec2020`, or all three
  with `--gamut all`; Display-P3 remains the default. Rec.2020 uses the CSS
  Color 4 display-referred gamma 2.4 transfer. Edge Seeker, Bottosson, and
  Dualray use generated per-gamut tables, fits, and seeds. The README
  describes how the solvers handle the blue hues where the sRGB and Rec.2020
  boundaries fold. Added independent-reference tests for each target and
  JS/Rust parity checks.
- Added `dualray fast` to Node, Bun, and Rust f64/f32 benchmarks for sRGB,
  Display-P3, and Rec.2020. Uses fitted lower-face hue polynomials to bypass
  trigonometry and root solving, with Dualray's upper solve and exact-search
  fallback. Preserves canonical in-gamut conversion in both benchmark modes
  and targets an empirical ΔEOK budget of `1e-3` maximum and `1e-4` at p99.
  Rust also includes a separate `dualray fast (poly encode)` row that
  approximates output encoding for mapped colors. Added independent-reference
  accuracy checks, canonical preservation tests, and JS/Rust parity checks.

## 2026-09-28

- Added CSS Color 4 Local MINDE as `css-minde` in Node, Bun, and Rust f64/f32
  benchmarks. Uses the spec's JND, search tolerance, initial clipping shortcut,
  and last-clip return policy, with the mandatory in-gamut check in both modes.
  Added an independent spec reference, shared numerical fixtures, exact
  in-gamut preservation tests, and native-f32 threshold regressions.
- Added `dualray` to Node, Bun, and Rust f64/f32 benchmarks, with direction
  fits, a guarded upper-first shortcut, upper-face retry, first-root recovery,
  and intrinsic in-gamut handling in both benchmark modes. Added independent
  boundary-oracle tests and precision-specific guards for Rust f32.
- Fixed Edge Seeker's arc intersection in JavaScript and Rust f64: rounding
  near a cusp could select the opposite circle intersection, producing large
  negative chroma and turning yellow into magenta. Both lookup variants now
  use the rationalized quadratic already used by Rust f32. All precisions
  retain the exact zero-curvature identity and clamp endpoint roundoff to
  `[0, 1]`. Added independent arc-oracle and near-cusp/near-white regressions.
- Standardized Node and Bun benchmarks on separate validation and timing
  processes, with 50 explicit warmup passes per method/workload through the
  exact timed callback. Warmup consumes all output channels and is excluded
  from timing. Added `--warmup`, `--validate-only`, and diagnostic `--timing-only`
  options; validation and timing failures propagate to the caller.
- Improved Rust direct-cubic code generation by constructing the validation
  coefficients before computing the root candidate and letting LLVM choose
  whether to inline its wrapper. Retained forced inlining for uncached cubic.
  Solver arithmetic and numerical recovery are unchanged.
- Added native f32 Rust implementations of all 11 benchmarked methods.
  `gma-bench` now prints f64 and f32 checksums and grid/random timing tables
  on every run; `--in-gamut-check` applies to both precisions. Added numerical
  checks and f32 conditioning for cubic roots, Edge Seeker arcs, and Raytrace.
- Fixed Rust and JS/Bun benchmark loops to consume all three encoded RGB
  channels for every color. Previously, only red was consumed, allowing LLVM
  to remove green/blue output conversion and gamma encoding from the Rust
  timing path. Rust now uses per-pass input and checksum barriers, and the JS
  harness prints its accumulated checksum. Release assembly confirmed that
  the corrected Rust path retains all three gamma branches. Historical timing
  tables are flagged for remeasurement.
