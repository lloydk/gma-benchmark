# Changelog

## 2026-09-28

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
