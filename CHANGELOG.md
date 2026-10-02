# Changelog

## 2026-10-02

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
