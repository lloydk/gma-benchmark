# Native (Rust) benchmark

A native point of reference for the JS `gma-benchmark`, timed over the same two
35,640-color workloads: the canonical grid (`oklch(L 0.4 H)`) and a random
hue/lightness workload (stratified/jittered, shuffled).

Display-P3 remains the default target with all 13 methods. The first multi-gamut
milestone adds **clip** and **css-minde** for sRGB and Rec.2020 in native f64 and
f32. The other methods still use their P3 fits, caches and tables.

| Target | Methods |
| --- | --- |
| `display-p3` (default) | All 13 |
| `srgb` | clip, css-minde |
| `rec2020` | clip, css-minde |

```sh
./target/release/gma-bench --gamut srgb
./target/release/gma-bench --gamut rec2020 --in-gamut-check
./target/release/gma-bench --gamut all --validate-only
```

`--gamut all` runs each target's supported methods. `--validate-only` runs the
workload equivalence/finiteness checks and checksums without timing; run
`cargo test --release` for the independent conversion and policy oracles.
Gamut and mode selection happen outside the timed loops. All targets share one
set of workloads, output consumption, warmup and repetition counts, and table
order: f64 grid/random, then f32 grid/random. `--gamut all` runs P3, sRGB, then
Rec.2020; use separate processes with balanced target order for performance
comparisons across gamuts. A fixed order does not eliminate thermal/order effects.

Unknown arguments are rejected with exit code 2 (older versions ignored them).
A missing `--gamut` value, including a following flag, is reported explicitly.

Rec.2020 uses the **display-referred gamma 2.4** definition in
[CSS Color 4, 26 September 2026](https://www.w3.org/TR/2026/CRD-css-color-4-20260926/#predefined-rec2020),
including its inverse for output encoding. This differs from the piecewise
BT.2020 camera OETF used in older CSS conversion examples. Comparisons with
other implementations must use the same encoding.

## Gamut and coordinate structure

- `rgb_spaces.rs` holds shared gamut marker types and D65 RGB↔XYZ definitions,
  with source links. One marker implements both precision lanes, so validation
  cannot accidentally pair different f32/f64 gamuts.
- Each precision lane compiles `gamut.rs`, `transfer.rs` and `color.rs`.
  `RgbGamut` supplies composed matrices and an associated transfer function.
  sRGB/Rec.2020 matrices are composed in f64 **at compile time**, then rounded
  to the lane's scalar type. P3 retains the incumbent coefficient literals;
  consistency tests check them against composition from the shared definitions.
- `Oklch`, `Oklab`, `LinearRgb<G>` and `EncodedRgb<G>` hold three native scalars.
  RGB gamut markers have no storage. Construction preserves the coordinates;
  conversion, clipping and encoding are explicit operations.
- `clip::Clip<G>` and `css_minde::CssMinde<G>` use static gamut dispatch. The
  method registry combines these core methods with P3-only extras.
- `p3_compat.rs` preserves the conversion entry points and evaluation order of
  the remaining P3 solvers. Its multiply-based conversion delegates to the
  typed kernel; historical `powi` and membership-check semantics are retained.
  `p3_fits.rs` holds their Bottosson approximation constants. Dualray's fits and the EdgeSeeker LUT remain algorithm-owned P3
  data, pending their separate multi-gamut ports.

The f32 mappers, including cube roots, powers, trigonometry and CSS MINDE
comparisons, stay entirely f32. Only benchmark checksums/statistics and
validation/reference calculations widen values, outside the mapper kernels.
JavaScript mappers are unchanged.

## Multi-gamut numerical validation

`rgb_reference.rs` derives RGB matrices independently from primary
chromaticities and the D65 white point. It uses uncomposed XYZ conversions,
encoded-channel clipping and the CSS Local MINDE pseudocode. The tests cover
both precisions, all six MINDE exit states, mixed chroma, RGB faces and adjacent
representable OKLCh inputs, transfer breakpoints, endpoints and extreme hues.
Accepted in-gamut inputs are compared bit-for-bit with their lane's canonical
conversion. Each precision's existing hue/achromatic input policy is retained.

Validation separates conversion arithmetic from MINDE stopping decisions:

- **Clip:** f32/f64 outputs are decoded with the independent reference transfer
  and compared in linear RGB (`2e-6` absolute channel limit). Conversion tests
  also check the unclipped linear coordinates and transfer accuracy separately;
  an encoded acceptance interval is derived from the linear error allowance.
- **CSS MINDE:** output differences are checked in ΔEOK. Encoded-channel
  differences remain diagnostics, with no blanket `0.1` acceptance budget.
- **P3-only solvers:** retain their existing encoded-channel workload budgets.

| Native f32 CSS MINDE target | ΔEOK limit |
| --- | ---: |
| sRGB | 0.0002 |
| Display-P3 | 0.00025 |
| Rec.2020 | 0.0015 |

These policies are explicit per target in `validation.rs`, separate from the
physical gamut definitions. They are empirical corpus limits, not full-domain
accuracy guarantees. A new target must provide its own validation policy.

At f32-rounded `oklch(0.88 0.4643206 18.5)`, Rec.2020 clip can differ by
`0.002213233` in encoded green while differing by only about `4.3e-7` in linear
green. Gamma encoding amplifies rounding near zero; the old fixed encoded limit
incorrectly rejected this input. Conversion and core-method validation now cover
RGB faces and adjacent representable coordinates as well as this regression.

Native f32 CSS MINDE is sensitive to comparisons at `JND` and `JND - epsilon`.
For example, on the recorded Linux build, f32-rounded `oklch(0.98 0.4 104)` in
Rec.2020 produces about `0.092906` encoded-channel difference from f64, but only
`0.00136649` ΔEOK. The regression checks the perceptual error and proximity of the
midpoint to JND; it permits either valid stopping decision across math libraries.
No JND adjustment or wider mapper arithmetic is used.

The [initial milestone report](reports/multi-gamut-milestone-1.json) preserves
measurements from before the review fixes. The
[review-fix report](reports/multi-gamut-review-fixes.json) records the current
source/binary hashes, expanded validation, CLI checks and controlled timings.
All 13 P3 methods still produce identical output bits to `5835529` in both modes
and precisions on 136,960 inputs (7,121,920 mappings). Debug and native release
builds each pass 51 tests.

On Ryzen 7 9800X3D / WSL2 (rustc 1.98.1, LLVM 22.1.8, `target-cpu=native`,
release/LTO), three runs per build pinned to CPU 2 and with balanced build order
measured current P3 CSS MINDE at +3.9%/+1.8% versus the original f32 grid/random
baseline, and +2.1%/-4.8% for f64. The f64 cached-cubic grid median was +1.6%;
the earlier +9.1% result was not reproduced consistently. Individual process
results are retained because this row varied substantially between runs.

Four isolated MINDE code-generation experiments (forced map inlining, local
candidate scope, an out-of-line delta helper, and scalar search state) showed
slowdowns or workload tradeoffs; none was adopted. The small f32 MINDE regression
remains a known limitation. These are build/workload measurements, not universal
abstraction costs. Standalone f32 kernel assembly still uses native
`sincosf`/`cbrtf`/`powf` with no double-precision arithmetic.

## `gma-bench` — scalar, apples-to-apples

For the default P3 target: one color per call, with native f64 and f32
implementations of all 13 methods.
Each timing run prints validation, both precisions' checksums, then four
sorted timing tables (`--validate-only` omits them): f64 grid/random and f32
grid/random. No precision flag is needed. `--in-gamut-check` selects the prechecked path in both precisions;
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
`methods.rs` supplies a core method list and a P3-only extras list for both lanes
and validation. This keeps
the algorithms and benchmark coverage aligned without copying implementations
or generating code by string replacement.

`css_minde.rs` implements the [CSS Color 4 Local MINDE search](https://www.w3.org/TR/css-color-4/#binsearch)
with native arithmetic in both lanes, `JND = 0.02`, and `epsilon = 0.0001`.
It preserves the canonical conversion for in-gamut inputs and returns the
last clipped candidate on interval exhaustion, as specified. Clipped linear
RGB in the selected target converts directly to Oklab for deltaEOK; gamma encoding is deferred until return.

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
and gamut membership. The table labels each acceptance metric and also prints
the maximum encoded-channel difference. Clip and CSS MINDE use the policies
above. The P3-only encoded-channel limits are `1e-4` for direct cubic, Halley,
Ostrowski, Dualray, and both Edge Seeker variants; `2e-4` for Raytrace; `1e-3` for
Bottosson; and `2e-3` for hue-quantized cubic/Bottosson variants. These remain
budgets for their existing workloads, not arbitrary boundary inputs: adjacent
cache-bucket choices can exceed them on other corpora.

The independent tests separately check conversion and transfer arithmetic,
MINDE output error, and exact canonical pass-through. Core clip/MINDE validation
includes mixed chroma and RGB boundary neighbours in all three gamuts.

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
