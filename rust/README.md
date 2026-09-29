# Native (Rust) benchmark

A native point of reference for the JS `gma-benchmark`, timed over the same two
35,640-color workloads: the canonical grid (`oklch(L 0.4 H)`) and a random
hue/lightness workload (stratified/jittered, shuffled).

## `gma-bench` — scalar, apples-to-apples

One color per call, with native f64 and f32 implementations of all 13 methods.
Every invocation prints validation, both precisions' checksums, then four
sorted timing tables: f64 grid/random and f32 grid/random. No precision flag
is needed. `--in-gamut-check` selects the prechecked path in both precisions;
`dualray` retains its intrinsic boundary checks in either mode, and `css-minde`
retains the in-gamut check required by CSS Color 4 in either mode.
The f64 lane uses the same conversion math as the JS methods.

```sh
RUSTFLAGS="-C target-cpu=native" cargo build --release --bin gma-bench
./target/release/gma-bench

# time the in-gamut-precheck variants (css-minde and dualray retain their checks):
./target/release/gma-bench --in-gamut-check
```

The f64 checksums (sum of all output channels) match the JS port:
`clip` and `edge-seeker` bit-for-bit, and the cubic variants to a few last-place
digits (from cbrt/acos libm differences). `oklch-cubic-direct` currently matches
across JS and Rust to all 10 printed decimal places.

The f64 timing inputs retain their original values. The f32 inputs are rounded
once before timing. Every mapper, intermediate, cache entry, LUT value, and
transcendental in the f32 lane uses f32; there is no f64 solver fallback. Only
the output checksum and timing statistics use f64. Widening all three f32
output channels for the checksum is included in the timed region. Both lanes
use 50 warmup passes and 25 measured passes over 35,640 colors per workload.

`algorithms.rs` and `timings.rs` are compiled twice with concrete scalar aliases.
The Edge Seeker LUT in `lut.rs` is likewise stored separately at each precision.
`methods.rs` supplies one method list for both lanes and validation. This keeps
the algorithms and benchmark coverage aligned without copying implementations
or generating code by string replacement.

`css_minde.rs` implements the [CSS Color 4 Local MINDE search](https://www.w3.org/TR/css-color-4/#binsearch)
with native arithmetic in both lanes, `JND = 0.02`, and `epsilon = 0.0001`.
It preserves the canonical conversion for in-gamut inputs and returns the
last clipped candidate on interval exhaustion, as specified. Clipped linear
P3 converts directly to Oklab for deltaEOK; gamma encoding is deferred until return.

`dualray.rs` is compiled in both precision modules. It uses fitted seeds, a
guarded upper-first path, competing-face retry, first-root fallback, and
intrinsic in-gamut handling. Its f32 policy uses constants for the
residual/containment tolerance `8 * f32::EPSILON` and hue
reduction outside `(-360, 360)`. The f64 tolerance remains `1e-12`, with hue
reduction outside `(-1e9, 1e9)`.

`conditioning.rs` contains the f32 numerical adjustments, selected at compile
time: stationary-interval validation and bisection recovery for Cardano roots,
sign-directed Raytrace intersections with a representable interior margin,
and f32 convergence/stagnation checks. Hues
outside one turn are reduced before f32 trigonometry to avoid overflow.
These precision-specific adjustments leave the f64 path unchanged.

Both precisions share a rationalized Edge Seeker arc in `algorithms.rs`.
The former f64 radius/center formula could select the opposite circle root
after endpoint rounding and return large negative chroma. The shared formula
avoids that root switch and cancellation near zero curvature, preserves the
exact straight-line identity, and clamps normalized endpoint roundoff to `[0, 1]`.

The direct-cubic wrapper evaluates its coefficient array before the candidate
and leaves inlining to LLVM. Controlled native builds found both choices
improve f32 timing; the uncached cubic wrapper benefits from forced inlining
and retains it. These choices affect code generation without changing solver
arithmetic, convergence thresholds, or recovery paths.

Before timing, both mapper modes are compared on identical f32-rounded inputs
(widened to f64 for the reference), and every output is checked for finiteness
and gamut membership. The printed errors are maximum absolute encoded-channel
differences. Regression limits are `1e-4` for clip, direct cubic, Halley,
Ostrowski, Dualray, and both Edge Seeker variants; `2e-4` for Raytrace; `1e-3` for
Bottosson; `2e-3` for the hue-quantized cubic/Bottosson variants; and `4e-3` for
CSS MINDE. Its threshold comparisons can choose a different stopping iteration
in f32: for example, at f32-rounded `oklch(0.68 0.4 143)`, an error just across
`JND - epsilon` changes the final chroma and produces a `4.75e-4` encoded-channel
difference. Shared spec-reference vectors, including threshold regressions,
also require a deltaEOK difference no greater than `2e-4` for f32 (`2e-12` for
f64). The wider channel limit accounts for amplification near dark RGB
channels; it does not change the algorithm's JND or epsilon. Rounding
can choose adjacent 0.1-degree buckets, so these variants have a separate
comparison budget. These are corpus regression limits, not universal accuracy
guarantees or claims that f32 reproduces f64 bit for bit.

Run the numerical tests with:

```sh
RUSTFLAGS="-C target-cpu=native" cargo test --release
```

Tests also cover mixed chroma, in-gamut colors, cache equivalence, native f32
storage, extreme hues/endpoints, and known cancellation/stagnation regressions.
An independent f64 stationary-interval/bisection oracle checks the exact-hue
solvers near boundaries and the cached cubics at their selected bucket hue.
Dualray is checked against this independent oracle in both precisions, with
mixed chroma, boundary neighbours, upper-face handoffs, and upper-first gate
neighbours. The encoded-channel regression budgets are `1e-8` for f64 and
`1e-4` for f32 over these corpora.
Both precision lanes have regression tests for complete output consumption.
CSS MINDE has shared reference vectors generated from the spec's pseudocode
and uncomposed XYZ conversions, plus tests for exact in-gamut preservation,
black/white endpoints, achromatic inputs, and extreme finite hues.
Edge Seeker tests compare both lookup variants with an independent circle
residual/bisection oracle at 576,016 near-cusp/near-white inputs per precision,
plus small-curvature cases and the yellow-to-magenta regression in both modes.

The timed passes use the same all-channel checksum as validation. Each pass's
input slice goes through `black_box`, and its checksum is consumed through
`black_box` before the timer stops. A separate validation checksum does not
protect the timed loop: the previous red-only timing sink allowed LLVM to
remove green/blue output conversion and gamma encoding. Timings collected with
that sink undercounted the work; rerun comparisons with the corrected harness.

Verified with rustc 1.98.1 / LLVM 22.1.8, `target-cpu=native`, release/LTO on
an AMD Ryzen 7 9800X3D: the old timed `clip` loop contains only the red matrix
row and its gamma branch. The corrected pass contains all three matrix rows,
three gamma branches, and an RGB sum feeding the consumed checksum. The
checksum barrier appears before `Instant::elapsed` in the generated code.
The native f32 timed `clip` pass was also inspected: it uses single-precision
arithmetic, `sincosf`/`powf`, and retains all three gamma branches.
This is a check of that build; [`black_box`](https://doc.rust-lang.org/std/hint/fn.black_box.html)
is a best-effort compiler barrier. To emit assembly for inspection:

```sh
RUSTFLAGS="-C target-cpu=native" cargo rustc --release --bin gma-bench -- --emit=asm
# Inspect target/release/deps/gma_bench-*.s, starting at run_timings.
```
