# Native (Rust) benchmark

A native point of reference for the JS `gma-benchmark`, timed over the same two
35,640-color workloads: the canonical grid (`oklch(L 0.4 H)`) and a random
hue/lightness workload (stratified/jittered, shuffled).

Display-P3 remains the default target with all 15 methods: the 13 shared with
JavaScript plus two Rust-only Dualray Fast rows. Milestone two adds
sRGB and Rec.2020 versions of the matrix-driven solvers in native f64 and f32:
clip, CSS MINDE, cached/uncached cubic, direct cubic, Halley, Ostrowski and Raytrace.
Milestone three adds both Edge Seeker variants, both constant-lightness
Bottosson variants and Dualray in all three targets, with generated tables,
cusp fits and lower-root seeds. See the
[milestone-three progress](MILESTONE-3.md) for scope and validation.

| Target | Methods |
| --- | --- |
| `display-p3` (default) | All 15 |
| `srgb` | All 15 |
| `rec2020` | All 15 |

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
- `clip::Clip<G>`, `css_minde::CssMinde<G>` and the six `rgb_solvers` types use
  static gamut dispatch. The single ordered method registry contains all
  fifteen generic methods. A cubic cache belongs to its gamut type;
  buckets retain 13 native scalars with no runtime gamut tag. Cached cubic
  borrows its hue entry after lazy initialization, avoiding the full-record
  stack copy whose native f64 timing depended on caller stack alignment.
  See the [investigation](reports/cached-cubic-stack-investigation.md).
- `polynomial.rs` holds shared polynomial candidates and targeted conditioning
  for cancellation-prone roots and ill-conditioned Newton refinements. The existing
  native f32 Cardano conditioning remains in `conditioning.rs`. There is no
  per-mapping, all-face first-exit guard. No fitted constants are used by these
  six ports; the iterative solvers retain upstream's gamut-specific fold windows.
- `p3_compat.rs` retains test-only historical P3 conversion routines.
- `compensated.rs` shares product residuals and accurate multiply-adds across
  Dualray, Bottosson and the iterative fold solver. FMA-enabled targets use
  hardware FMA; other targets use native two-product/two-sum arithmetic.
  The portable multiply-add is a compensated approximation, not a general
  correctly-rounded FMA emulator. Both builds retain the same error budgets.
- `dualray::DualrayData` owns the normalized channel basis and fitted seeds.
  `dualray_config.rs` supplies the root limit and exports the shared
  `rgb_spaces::blue_fold_window` to the generator. The f32 fold solver uses native two-component
  arithmetic; ordinary calls keep the fitted shortcut/retry path.
- `bottosson::BottossonData` owns primary-hue sectors and saturation fits.
  sRGB/Rec.2020 fits are generated; the same generator retains the pinned P3
  coefficients. All three targets use the same generated data layout.
  The two variants share one intersection kernel; the cache stores five
  native scalars per 0.1-degree hue bucket. Checked mapping first converts
  the authored hue, preserving accepted canonical results bit-for-bit.
- `edge_seeker::EdgeSeekerData` owns per-target tables in `generated/`. Both
  lookup variants use static dispatch and native-precision data; the physical
  `RgbGamut` contract has no LUT or approximation policy. The indexed variant
  shares a compile-time 3,600-entry index per gamut/precision; instances allocate
  no index. Sharp-interval bands and residuals are compile-time table data.

The f32 mappers, including cube roots, powers, trigonometry and CSS MINDE
comparisons, stay entirely f32. Only benchmark checksums/statistics and
validation/reference calculations widen values, outside the mapper kernels.
All three existing P3 JavaScript cubics share the conditioned root helper;
the direct cubic also receives the Newton-refinement fix. JavaScript Clip,
CSS MINDE and all six matrix solvers now support all three targets; the fitted
and table-based ports are tracked
in [milestone four](../MILESTONE-4.md). That port also exposed and fixed
f64 Raytrace near-white direction flushing and premature dark-fold bisection
termination in both languages.

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
- **Direct cubic:** decoded linear RGB, `5e-5` f32/f64 workload limit, plus
  an independent `2e-5` ΔEOK limit. Raw
  boundary chroma is checked against an independent first-exit oracle
  (`2e-5` f32, `1e-8` f64).
- **Halley and Ostrowski:** the same linear and perceptual workload limits.
  Within the padded blue windows `[264.03, 264.23]` for sRGB and
  `[245.04, 245.31]` for Rec.2020, they select the greatest feasible chroma at or below the input; a gap input
  maps to the preceding exit and valid re-entry colors retain their chroma.
  Stationary intervals isolate each crossing, and all channels
  must be feasible within arithmetic roundoff. This replaces the upstream
  second iteration, which can stall at an out-of-gamut corner. P3 has no fold
  solve. Shared binary32 endpoints make both lanes classify rounded input hues
  identically; hue wrapping does not change canonical conversion.
  Raw chroma checks cover `L >= 0.001`; near-black stopping is assessed in RGB.
- **Cached and uncached cubic:** decoded linear RGB, `5e-5` workload limit,
  plus a `2e-5` ΔEOK limit, with both lanes aligned to the same 0.1-degree bucket. Independent mapping
  tests use each lane's actual bucket; canonical checks use the authored hue.
  The f64 reference limit is `1e-6` linear RGB (measured maximum `8.45e-7`),
  retaining the incumbent Cardano approximation without a full boundary guard.
  The geometric f64 mapping limits are `1e-8` for direct cubic and `2e-8`
  for the iterative solvers. The latter includes their existing `1e-9` chroma
  stopping rule; the new geometric oracle measures up to `1.28e-8` linear
  error where the old copied iteration masked that error.
- **Raytrace:** decoded linear RGB, `2e-4` f32/f64 workload limit for sRGB,
  `5e-5` for P3/Rec.2020. Independent four-cast reference limits are `5e-4`
  for native f32 sRGB and `5e-5` for native f32 P3/Rec.2020. Raytrace is checked
  against its mapping policy rather than treated as an exact first-exit solver.
  Additional f32/f64 ΔEOK limits are `0.002` sRGB, `0.0015` P3 and `0.0035`
  Rec.2020. Measured maxima across the validation corpora are approximately
  `0.00180`, `0.00102` and `0.00294`, respectively; these larger errors are
  specific to Raytrace's native f32 convergence and anchor decisions.
- **Edge Seeker:** `1e-5` linear RGB and `3e-6` DeltaEOK f32/f64 limits.
  Independent table-policy tests check chroma, conversion and canonical
  preservation separately from approximation quality against the geometric
  boundary. Both lookups must be bit-identical in every target and mode.
- **Bottosson:** `1.2e-5` linear RGB and `5e-6` DeltaEOK f32/f64 limits.
  Cached comparisons align hue buckets. Independent tests check sector
  geometry, cusp fits, one-Halley mapping arithmetic and canonical pass-through.
  Approximation quality is measured separately: this is not a first-exit solver.
  Near blue, native f32 uses a small-angle rotation and compensated residual
  for the same single Halley step; it does not widen to f64. Dense primary-hue
  sweeps cover every f32 value within 0.1 degrees and a shifted wider grid.
- **Dualray:** `2e-5` linear RGB and `5e-6` DeltaEOK f32/f64 limits.
  The historical P3 `1e-4` encoded-channel gate is also retained. sRGB and
  Rec.2020 are checked against an independent geometric first-exit oracle,
  including fold tangencies, primaries, handoffs and near-white scales.
  Both modes retain intrinsic first-exit handling and must be bit-identical.

| Native f32 CSS MINDE target | ΔEOK limit |
| --- | ---: |
| sRGB | 0.0002 |
| Display-P3 | 0.00025 |
| Rec.2020 | 0.0015 |

These policies are explicit per target in `validation.rs`, separate from the
physical gamut definitions. They are empirical corpus limits, not full-domain
accuracy guarantees. A new target must provide its own validation policy.
Non-finite outputs and individual error metrics fail explicitly. Cross-method
CLI checks run for every target, comparing only matching policies: bucketed
cubics on grid hues, direct/Halley outside folds, Halley/Ostrowski everywhere, and the two
Edge Seeker lookups bit-for-bit. Dualray/direct comparisons use the same
first-exit policy in every target, including folds.

Linear budgets imply finite encoded bounds even for Rec.2020: a linear error
`e` permits at most `e^(1/2.4)` encoded error there. That global bound is loose
near zero, so the separate ΔEOK gate limits perceptual error as well; encoded
maxima remain visible in the validation output.

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

At a folded gamut boundary, the f32 and f64 canonical prechecks can disagree:
for sRGB at f32-rounded `[0.17938177, 0.12429921, -95.947975]`, one lane can
preserve a re-entered color while the other maps to the earlier exit. Validation
checks accepted output bits against each lane's canonical conversion and checks
rejected outputs against that lane's plain mapper. When classifications differ,
it compares the two plain solvers and reports the branch count and original
encoded-output difference separately. It does not introduce a membership epsilon.

`test_oracle.rs` derives matrices through the independent XYZ reference. Its
geometric oracle splits all six face polynomials at stationary points and
bisects crossings. The first-exit reference skips tangencies that stay inside;
it uses normalized `C/L` and a polynomial root bound. The iterative reference
selects the greatest feasible chroma no larger than the input inside fold
windows and the first exit elsewhere. Gap inputs cannot be retained and clipped. It does not copy Halley/Ostrowski iterations or stopping rules.

Near a blue primary, floating-point rounding can change whether the outer
island touches the RGB cube. A first-exit result and a vivid outer result can
then differ by about `0.05` DeltaEOK without representing numerical error within
one branch. Tests enumerate feasible outward intersections independently. They
accept an alternate branch only in the fold window when a corner or stationary
contact is within `2e-6` linear feasibility slack for f32 or `2e-14` for f64.
Ordinary within-branch accuracy limits are unchanged. Dedicated stable-island regressions require the outer branch, so an
implementation that always returns the inner boundary cannot pass.

The workload validator reports differing iterative fold branches separately.
Each such output must be on a cube face, retain authored L/h within `2e-6`
in Oklab, and not increase chroma. This is a validation allowance, not a
production gamut-membership epsilon. Canonical prechecks remain strict.

The corpus includes all six cube faces, fractional hues, RGB primary/secondary
neighbours, mixed chroma, near-black/near-white rays and benchmark workloads:
241,483 mapping inputs per gamut in f64 and 224,203 in f32. Added near-white
scales cover `1-2^-14` through `1-2^-28` where representable. A separate fold
sweep covers 109,382 lightness/hue pairs per precision across sRGB and Rec.2020,
including the reported `L=0.414, h=264.0425` corner. Canonical accepted outputs are
checked bit-for-bit. These are sampled checks, not full-domain proofs.

The mechanical port preserved all 7,121,920 P3 mappings in the milestone-one
snapshot. The final implementation intentionally includes these targeted fixes:

- Cardano can round a tiny positive upper-face root negative near white, or
  a tiny negative root positive. A derivative-bound test selects a local Newton
  solve with no arbitrary `d/c` cutoff. A recovered negative root is deflated
  before selecting the next positive root. Both Rust and JavaScript cached
  cubics now accept upper roots below the former `1e-9` cutoff.
- Exactly touching a cube face exits only when the ray points outward. Inward
  contacts factor out the zero root and continue to the next positive root.
- Direct cubic's Newton polish skips nearly singular derivatives. Only poorly
  conditioned derivatives need the additional residual evaluation in f64. Native
  f32 refines once inside its existing bracket validation. This keeps the
  double-root regression fixed without duplicate unbracketed refinement.
- The blue-fold second iteration can stop off the boundary at an active-face
  corner. Geometric solving is restricted to the fold windows and retains
  valid outer islands. The search is bounded by input chroma so colors in
  a disconnected gap map to the preceding feasible exit rather than being
  clipped. The rest of the iterative path is unchanged.
- Raytrace checks reprojection against the last hit as well as the anchor in
  both precisions. This prevents roundoff from casting to the opposite face.
  Its independent reference has the same convergence protection, plus a
  physical near-white regression assertion. Non-positive chroma uses the
  canonical achromatic conversion in both checked and unchecked modes.

The earlier blanket first-exit guard has been removed. It added substantial
cost and imposed a first-exit policy on iterative algorithms whose upstream
behavior deliberately seeks vivid blue outer intersections. The near-black
Halley discrepancy remains: at `L=1e-6, h=270.25` in P3 its relative chroma error
is large, but its final output difference is only about `1.4e-7` DeltaEOK.

The [milestone-two report](reports/multi-gamut-milestone-2.json) records milestone-two
source hashes, numerical measurements and a balanced before/after review-fix
comparison. The earlier milestone-one/blanket-guard comparison is retained as
historical evidence; its pre-review validation claims have been superseded.
The [milestone-one report](reports/multi-gamut-milestone-1.json) and
[review-fix report](reports/multi-gamut-review-fixes.json) are historical snapshots.

| P3 f64 grid, ns/call | Before review fixes | After review fixes |
| --- | ---: | ---: |
| oklch-cubic (cached) | 58.50 | 55.76 |
| oklch-halley | 98.06 | 97.45 |
| oklch-ostrowski | 97.92 | 97.51 |
| oklch-cubic-direct | 184.41 | 190.37 |
| oklch-cubic (no cache) | 197.25 | 195.78 |

These are medians of process medians on Ryzen 7 9800X3D / WSL2, rustc 1.98.1,
native release/LTO, two processes per build pinned to CPU 2 with alternating
order. Every process consumes all output channels over the same 35,640 inputs,
50 warmup passes and 25 measured passes. Timings include code-layout effects.
The report includes both precisions, both workloads, and separate runs of every
target in both precheck modes. JavaScript performance was not measured.

At the milestone-two commit, all 77 Rust tests passed in debug, native release
and portable x86-64 release.
All four JavaScript test files pass under Node 26.10.0. The cubic regressions
include 57,600 near-white inputs per variant in both modes, plus probes around
the double-root failure. Native f32 sRGB Raytrace reaches `4.37e-4` linear error
against the four-cast reference on the expanded corpus, within its existing
`5e-4` reference limit. This is separate from the boundary solvers' limits.

The legacy f64 conversion multiplies degrees by PI before dividing by 180;
finite hues large enough to overflow that multiplication remain outside its
finite-output domain. Native f32 reduces out-of-range hue before conversion.
The numerical tests retain these existing input policies.

## `gma-bench` — scalar, apples-to-apples

For the default P3 target: one color per call, with native f64 and f32
implementations of all 15 methods.
Each timing run prints validation, both precisions' checksums, then four
sorted timing tables (`--validate-only` omits them): f64 grid/random and f32
grid/random. No precision flag is needed. `--in-gamut-check` selects the prechecked path in both precisions;
`dualray` retains its intrinsic boundary checks in either mode, both
`dualray fast` rows always include their canonical in-gamut check, and
`css-minde` retains the in-gamut check required by CSS Color 4 in either mode.
The f64 lane retains the JS conversion math. Rust also has native f32
conditioning and the target-specific iterative blue-fold policy described above.

```sh
RUSTFLAGS="-C target-cpu=native" cargo build --release --bin gma-bench
./target/release/gma-bench

# time the in-gamut-precheck variants (css-minde, dualray and dualray fast keep their checks):
./target/release/gma-bench --in-gamut-check
```

The checksums sum all output channels. Compare output policies as well as
arithmetic when checking Rust against JavaScript: native f32 convergence and
multi-gamut fold handling can intentionally change mapped colors.

The f64 timing inputs retain their original values. The f32 inputs are rounded
once before timing. Every mapper, intermediate, cache entry, LUT value, and
transcendental in the f32 lane uses f32; there is no f64 solver fallback. Only
the output checksum and timing statistics use f64. Widening all three f32
output channels for the checksum is included in the timed region. Both lanes
use 50 warmup passes and 25 measured passes over 35,640 colors per workload.

`algorithms.rs` and `timings.rs` are compiled twice with concrete scalar aliases.
The Edge Seeker tables in `generated/` are likewise stored at each precision.
Regenerate or verify them from the repository root with:

```sh
node scripts/generate-edge-seeker.mjs
node scripts/generate-edge-seeker.mjs --check
```

The generator exports the Rust gamut profiles, then reuses the existing JS
`makeLut` builder. Generation is a development step; building or running the
Rust benchmark does not require Node. Recorded generation uses Node 26.10.0;
platform math-library differences can change the final bits of generated data.

`methods.rs` supplies one ordered registry of all fifteen generic methods for both
lanes and validation. This keeps algorithm and benchmark coverage aligned.

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

`node scripts/generate-dualray.mjs` regenerates the target basis and seeds;
add `--check` to verify freshness. Both modes also run the actual Rust f32
window-edge regression; the Cargo build uses the caller's target flags.
P3's incumbent coefficients remain pinned.
The shared sRGB `[264.03,264.23]` and Rec.2020 `[245.04,245.31]` degree windows
(with f32-rounded endpoints in both lanes) bypass fitted lower roots and isolate
the first exit. The generator requires at least 0.02 degrees between either
window edge and the sector switch/fold; compile-time checks bound the
small-angle series domain. The Rust regression visits every f32 hue within
0.02 degrees of each edge, including negative and wrapped hues.
Native f32 uses split constants, polynomial trigonometry and compensated
evaluation inside those windows to prevent tangent-root errors. Products use
the shared hardware-FMA/native-split helper, without software `fmaf` or f64
widening. The fold direction consumes the already-reduced hue.
Newton steps stay inside the first monotone crossing bracket, with bisection
as a fallback. Searches stop at the input chroma or the nearest exit already
found, and the selected exit channel is set to exactly zero or one. Interior
inputs do not snap to a face. The f64 fold path skips the seed. For inside endpoints, it bounds all
six face searches by the input chroma and nearest root found so far; this still
finds an earlier exit before an in-gamut outer island. Outside endpoints retain
the faster upper-face refinement. Recovery ignores stationary face touches that
do not exit gamut. See the
[second review follow-up](reports/dualray-review-followup.md) for accuracy and timings.

Dualray preserves its existing normalized-cubic policy for inputs below the
first exit. It does not promise canonical conversion bits or preserve an outer
in-gamut island beyond the first exit. Its checked entry point is the same
algorithm; it does not add the canonical precheck used by the other exact-hue
mappers. This distinction is tested and remains visible in the benchmark.

`dualray_fast.rs` (rows `dualray fast` and `dualray fast (poly encode)`, Rust
only) is an approximate, cache-free constant-lightness/hue mapper for OKLCh
input. It does not call Dualray. Below the cusp, per-sector hue polynomials give
the lower-face boundary ratio and the two nonzero linear channels directly, so
about three quarters of the out-of-gamut workload colors need no trigonometry,
conversion or root solve. Other colors take, in order: the canonical in-gamut
check (an in-gamut color returns that conversion, including colors in a
blue-fold re-entry island); the fitted lower output for the margin band just
beyond the lower face; above the cusp, the chord seed and two Householder steps
on the brighter lower channel, retried on a channel that exceeds one; and for
the rest (mostly blue-fold hues, at most about 0.1% of colors) an exact first
exit by bisection between the channels' stationary points. Its checked entry
point is the same algorithm. The poly-encode row replaces the transfer
function's `pow` with a polynomial on out-of-gamut results only.

Its target is a deltaEOK of at most `1e-3`, and `1e-4` at the 99th percentile,
from the constant-lightness/hue first exit. Measured maxima are about `2.7e-4`
with a p99 of `3e-5` to `5e-5` in all three gamuts and both precisions, against
exact Dualray and gma-accuracy's certified reference. These are sampled
results, not bounds. The f32 lane is pure f32: at the red fold, where rounding
cannot decide whether the red channel's tangent dip is an exit, the binary64
fold hue (stored as two f32 constants) decides. `node scripts/generate-dualray-fast.mjs`
fits the per-gamut data from exact roots; add `--check` to verify freshness.
Validation compares the lanes against per-gamut regression ceilings, checks
exact canonical pass-through in both modes, and keeps the f64 rows within the
`1e-3` deltaEOK design budget of Dualray outside blue-fold windows.
`cargo run --release --example dualray-fast` runs the full accuracy sweep,
path statistics and paired timing; `scripts/dualray-fast-oracle.py` runs the
same checks against gma-accuracy's independent oracle.

`conditioning.rs` contains the f32 numerical adjustments, selected at compile
time: stationary-interval validation and bisection recovery for Cardano roots,
sign-directed Raytrace intersections with a representable interior margin,
and f32 convergence/stagnation checks. Hues
outside one turn are reduced before f32 trigonometry to avoid overflow.
These precision-specific adjustments leave the f64 path unchanged.

Both precisions share a rationalized Edge Seeker arc in `edge_seeker.rs`.
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
above. Dualray retains P3's historical `1e-4` encoded-channel gate alongside
its new linear/perceptual gates for all targets. Bottosson now uses the separate linear/perceptual limits above;
its cache's hue quantization is kept separate from arithmetic error.

The independent tests separately check conversion and transfer arithmetic,
MINDE output error, and exact canonical pass-through. All fifteen methods are
validated on mixed chroma and RGB boundary neighbours in all three gamuts.

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
neighbours. The historical P3 encoded-channel regression budgets are `1e-8` for f64 and
`1e-4` for f32. New all-target tests use `2e-11` linear/DeltaEOK limits for
f64, and `2e-5` linear / `5e-6` DeltaEOK for f32 over their documented corpus.
Both precision lanes have regression tests for complete output consumption.
CSS MINDE has shared reference vectors generated from the spec's pseudocode
and uncomposed XYZ conversions, plus tests for exact in-gamut preservation,
black/white endpoints, achromatic inputs, and extreme finite hues.
Edge Seeker tests compare both lookup variants with an independent circle
residual/bisection oracle at 576,016 near-cusp/near-white inputs per target and precision,
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
