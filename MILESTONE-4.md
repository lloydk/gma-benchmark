# Milestone four: JavaScript target factories

Step one adds sRGB and Rec.2020 conversions, Clip and CSS MINDE.
[Step two](#step-two-matrix-solvers) adds the six matrix solvers. All thirteen
existing P3 methods retain their exports. [Step three](#step-three-bottosson)
ports both Bottosson variants. [Step four](#step-four-edge-seeker) adds both
Edge Seeker variants. [Step five](#step-five-dualray) adds Dualray, completing
all thirteen methods across all three gamuts. Earlier measurements below are historical.

## Architecture

`src/rgb-spaces.js` exports `SRGB`, `DISPLAY_P3`, `REC2020`, `RGB_SPACES` and
`getRgbSpace(id)`. These immutable descriptors contain physical D65 RGB/XYZ
matrices, composed LMS conversion matrices and transfer functions. They contain
no fitted seeds, fold windows, mapping budgets or mutable caches.

`scripts/generate-rgb-spaces.mjs` consumes the existing Rust profile bridge and
writes `src/generated/rgb-spaces.js`. P3 uses the incumbent composed coefficients
rather than recomposing them and changing their last bits. Runtime JavaScript
requires neither Rust nor another color library. The other algorithm generators
continue to use the same bridge; its added physical-matrix fields are additive.

`createRgbConversions(space)` captures scalar coefficients and transfer functions
once. It exposes `oklabToClippedRgb(L,a,b,out)`,
`oklchToClippedRgb(L,C,H,out)`, `oklchToRgbIfInGamut(L,C,H,out)` and
`rgbToOklch(r,g,b)`. The last retains the existing `{l,c,h}` LUT-building adapter;
forward conversion and mapping reuse the caller's output array. The membership
conversion writes nothing unless all three linear channels are inside `[0,1]`,
with zero tolerance; non-finite channels fail membership.

`createClip(space)` and `createCssMinde(space)` return `(oklch, out) => out`.
Both support aliased input/output arrays. Target selection, profile lookup and
factory allocation happen outside mapping. Default P3 Clip shares the
conversion instance used by the existing P3 solvers, following the Bun
performance experiment recorded below. CSS MINDE retains its intrinsic
membership check, initial-clip shortcut, negative-chroma/zero-chroma handling,
and last-clip result when the chroma interval exhausts. This step adds no
missing-component or general color-object model.

```js
import { REC2020 } from "./src/rgb-spaces.js";
import { createCssMinde } from "./src/css-minde.js";

const map = createCssMinde(REC2020);
const out = [0, 0, 0];
map([0.7, 0.4, 245.1], out);
```

Spectre's `oklab-rgb-profile.ts` and `spaces/rec2020.ts` were inspected at commit
`099edb4f76884fcc16e7e7a410a1782be1b24bf0`. This adopts the separation of physical
profiles from algorithm policy and setup-time conversion selection. No Spectre
source was copied, and its registry, conversion graph, ColorSpace/Gma classes
and missing-component handling are unnecessary for these fixed-target kernels.

## Step-one harness and verification

`node bench.js --gamut display-p3|srgb|rec2020|all` selects targets; Bun accepts
the same flags. P3 remains the default and runs all thirteen methods. sRGB and
Rec.2020 run Clip and CSS MINDE. Both methods have the same behavior in the
plain and `--in-gamut-check` modes. Help states that coverage explicitly.

Each target runs in a separate process under `--gamut all`; ordinary runs also
separate validation from timing. P3-only solvers and LUTs are imported only in
P3 children. This avoids training another target's factory call sites and avoids
building P3 caches when running sRGB/Rec.2020. Workloads are identical across
targets, with grid followed by random timing in each child.

```sh
node --test tests/*.test.js
bun test tests
node scripts/generate-rgb-spaces.mjs --check
node scripts/check-rgb-parity.mjs
bun scripts/check-rgb-parity.mjs
node bench.js --gamut all --validate-only
node bench.js --gamut all --validate-only --in-gamut-check
```

Independent references keep the rational RGB/XYZ matrices and separate XYZ/LMS
stages in test code. They do not import generated production matrices. Tests
cover the grid, mixed chroma, folded hue neighbourhoods, endpoints, near-white
inputs, signed transfer functions, inverse conversion, canonical pass-through,
output aliasing, immutable descriptors and interleaved factories. CLI tests
check invalid/missing gamut values, conflicting modes and help coverage.

The Rust parity check executes the actual f64 Clip and CSS MINDE kernels through
`rgb-mapping-probes`; it supplements, rather than replaces, the independent
reference. Its 44,192 inputs per target cover 265,152 mapped arrays. Linear RGB
and DeltaEOK comparisons use `2e-11` limits, with encoded maxima reported too.
The separate linear/perceptual gates avoid treating Rec.2020's steep transfer
near zero as a large physical error. Finite-output checks precede all maxima.

Node and Bun each pass 24 tests and all-gamut CLI validation in both modes.
Generated profiles are fresh, and the Rust development examples compile. P3 before/after compatibility checks compare
79,472 inputs across all thirteen methods and both entry modes: **2,066,272
output arrays per runtime**, all bit-identical. This is sampled compatibility,
not a full-domain or cross-platform guarantee. The benchmarks preserve current
P3 policies; later solver ports will need their own geometry and policy tests.

## Performance

See [the measurement report](reports/milestone-4-step-1.json) for source hashes,
input hashes, runtime/machine details, validation maxima and raw timing rows.
The baseline is commit `7d6c9e0`. Before/after/after/before measurements use frozen
source copies, one executable/script pathname, CPU 2, fresh processes and no
concurrent builds, tests or other benchmarks. Both Node and Bun are measured.
Factory construction and LUT setup are excluded from warm throughput.
For a focused run (or substitute Bun):

```sh
taskset -c 2 node scripts/bench-rgb-kernels.mjs --method css-minde --gamut rec2020
```
The same focused script can be copied into the baseline checkout for P3.

The isolated script `scripts/bench-rgb-kernels.mjs` runs one target/method per
process, consumes all three outputs, warms 50 full passes, then measures 25
passes over each 35,640-color workload. The primary result is the median of two
process medians in before/after/after/before order. New-target numbers are two
post-change process medians; no pre-existing sRGB/Rec.2020 JS implementation is
claimed as a baseline.

| Runtime / P3 method | Grid before → after, ns | Random before → after, ns |
| --- | ---: | ---: |
| node / clip | 42.83 → 40.02 (-6.6%) | 57.02 → 54.07 (-5.2%) |
| node / css-minde | 427.68 → 429.30 (+0.4%) | 483.31 → 484.16 (+0.2%) |
| bun / clip | 41.93 → 42.14 (+0.5%) | 58.74 → 57.63 (-1.9%) |
| bun / css-minde | 599.00 → 618.99 (+3.3%) | 655.89 → 658.53 (+0.4%) |

| Runtime / target | Clip grid / random, ns | MINDE grid / random, ns |
| --- | ---: | ---: |
| node / srgb | 39.46 / 54.02 | 439.16 / 484.81 |
| node / rec2020 | 43.65 / 59.10 | 424.41 / 472.24 |
| bun / srgb | 43.42 / 59.33 | 591.09 / 650.71 |
| bun / rec2020 | 48.26 / 64.21 | 581.27 / 629.50 |

The full Mitata harness also measures all thirteen P3 methods in both runtimes.
Its process-average timings and spreads are retained separately from the
isolated medians.

The initial factory implementation regressed Bun Edge Seeker grid by 8.2%,
and a repeat measured 9.3% (104.38 → 114.06 ns). Default P3 Clip had created a
second conversion instance. Sharing the P3 conversion functions with the other
P3 solvers changed the final full-harness result to 104.66 →
106.76 ns (+2.0%). This is an observed runtime sensitivity to
instance sharing; no engine-level mechanism is claimed. The initial candidate
and repeat are retained in the JSON, alongside the final results.

| Final full-harness P3 method | Node grid / random change | Bun grid / random change |
| --- | ---: | ---: |
| clip | -5.2% / -5.1% | -2.3% / -1.2% |
| css-minde | +1.2% / +0.1% | -4.0% / -2.7% |
| edge-seeker | -7.8% / -1.0% | +2.0% / +1.3% |

Other method comparisons and raw ranges are retained in the report. Small
changes have limited statistical weight; the two harnesses have different
call histories, so an isolated result does not establish a full-harness win.

The full-harness results include code placement and JIT variation and should
not be interpreted as a measured cost of browser color objects or dispatch.

## Step two: matrix solvers

The six modules now export `createOklchCubic(space)`,
`createOklchCubicNoCache(space)`, `createOklchCubicDirect(space)`,
`createOklchHalley(space)`, `createOklchOstrowski(space)` and
`createRaytrace(space)`. Their existing function exports are P3 instances.
Each factory captures scalar coefficients and transfer functions at setup;
there is no target lookup in a mapping call. Cached cubic tables belong to
individual factory instances. The ordinary mapping paths reuse the caller's
output array; the rare outer-fold solve allocates small temporary arrays.

At the step-two checkpoint, the benchmark ran eight methods for sRGB/Rec.2020 and thirteen for P3,
with the same ordering, inputs, precheck flag and separate-process timing.
At that checkpoint, fitted/table methods were still dynamically imported only for P3.
Importing a matrix module also creates its legacy P3 instance; a new-target
run therefore has an unused P3 cubic cache, but never trains it with mappings.

```js
import { SRGB } from "./src/rgb-spaces.js";
import { createOklchHalley } from "./src/oklch-halley.js";
const map = createOklchHalley(SRGB);
const out = [0, 0, 0];
map([0.414, 0.4, 264.0425], out, true);
```

The mapping policies are deliberately distinct:

- Cached/no-cache cubic use the first exit at a 0.1-degree hue bucket and are
  bit-identical to each other on the validation corpus.
- Direct cubic uses the first exit at the authored hue. Face contact only
  exits at zero if the polynomial points outward; face identity is explicit.
- Halley/Ostrowski use ordinary bracketed iteration outside blue folds. Inside
  the window they select the greatest feasible chroma no larger than the input,
  using compensated binary64 evaluation near cancellations. A gap input maps
  to the preceding exit; a valid re-entry input retains its chroma.
- Raytrace retains four successive RGB-box intersections and chroma
  reprojections. Its output is checked against that policy, not against a
  first-exit solver. Checked entry points preserve canonical conversions
  before any hue bucketing or solver arithmetic.

`generate-rgb-spaces.mjs` now also emits a separate
`src/generated/matrix-solver-policy.js` from Rust's `blue_fold_window`.
The windows retain their binary32 endpoint values when widened to JS Number.
They do not become properties of physical color-space descriptors.

### Numerical findings during the port

Independent XYZ tests exposed two shared Rust f64 issues beyond the mechanical
port. Both implementations were corrected together:

- Raytrace's fixed `1e-12` direction flush could ignore the nearest face near
  white. At P3 `[1 - 2**-42, 0.4, 150]` it returned a saturated color instead
  of near-white. Direct division retains nonzero directions. Reprojection
  also needed a 32-epsilon binary64 convergence allowance: eight epsilons
  still cast rounding noise at `[1 - 2**-52, 0.4, 117.75]`. The Rust f32
  allowance remains eight epsilons. Physical near-white assertions supplement
  the algorithm reference; all three targets are swept at 0.125-degree hues
  over 30 lightness scales (259,200 mappings per JS runtime).
- The outer-fold bisection bracket is `0.5/L`. A fixed mantissa-length iteration
  cap could reject the correct outer boundary near black, including sRGB
  `[2**-14, 0.4, 264.05]` and Rec.2020 `[2**-15, 0.4, 245.1]`. The search now
  terminates at adjacent floats, with an exponent-plus-mantissa safety cap.
  A Rust regression checks normalized boundary invariance across lightness.

JavaScript also receives the Rust canonical-gray and convergence paths,
explicit upper-face handling, and the direct cubic's gray-origin face check.
No f32-style blanket Cardano guard is added to JavaScript.

### Validation and compatibility

The independent oracle composes its own rational physical XYZ matrices,
partitions channel cubics at derivative roots and bisects outward crossings.
Its feasibility tolerance scales with polynomial magnitude, including near
black. It never imports production matrices or root solvers. Raytrace uses a
separate vector/XYZ reference. Validation checks finite output before maxima,
linear RGB and DeltaEOK together, and reports encoded differences separately.
The cubic hue-quantization policy is assessed separately from arithmetic error.

Commands:

```sh
node --test tests/*.test.js
bun test tests
node scripts/check-matrix-parity.mjs
bun scripts/check-matrix-parity.mjs
node scripts/generate-rgb-spaces.mjs --check
node bench.js --gamut all --validate-only
node bench.js --gamut all --validate-only --in-gamut-check
cargo test --release --manifest-path rust/Cargo.toml
```

At the initial step-two checkpoint, the suites passed **37 tests in Node and Bun**, plus **115 Rust release
tests in both portable and `target-cpu=native` builds**. All-gamut CLI
validation passes in both JS runtimes and both precheck modes, and in Rust.
Generated policy/profile freshness and formatting checks pass.

The deterministic corpus contains 54,690 inputs per target: integer grid,
mixed chroma, negative/repeated hues, dense fold neighbourhoods, endpoints,
and near-white/black powers of two. Each runtime checks 1,968,840 solver
outputs across three targets, six methods and two entry modes against both
the independent policies and actual Rust f64 kernels. Additional tests cover
window edges, canonical pass-through, output aliasing and interleaved caches.

Independent linear RGB / DeltaEOK budgets are `1e-6` for the bucketed cubics
and `2e-8` for the other four methods; these are sampled regression budgets.
The largest measured linear error is `3.56e-7` (sRGB cached/no-cache cubic),
and the largest DeltaEOK is `7.33e-8`. Encoded Rec.2020 residuals reach
`2.35e-4` despite much smaller linear/perceptual errors near zero. No shared
encoded tolerance is asserted across transfer functions.
Rust parity uses `2e-8` linear and DeltaEOK gates: Node's maxima are `8.50e-9`
and `3.53e-9`; Bun has zero measured numerical difference on this corpus. These are runtime-specific
observations, not cross-platform guarantees.

P3 before/after compatibility covers 1,421,940 output arrays per runtime.
All twelve methods other than Raytrace are bit-identical on the corpus.
Raytrace changes 37,434 arrays in Node and 37,636 in Bun, including ordinary
rounding differences and corrected near-white colors. The largest difference
is the P3 `[1 - 2**-42, 0.4, 150]` case: approximately `[0,1,0.2492]` becomes
`[1,1,1]`. The report retains counts, extrema and reproducer inputs.

### Step-two performance

Measurements and source hashes are recorded in
[the step-two report](reports/milestone-4-step-2.json). The baseline is the
uncommitted, completed step-one snapshot, **not** `7d6c9e0` directly.
The report includes a patch from that commit to reproduce the baseline runtime.

The focused harness uses 50 warmup passes and 25 measured passes over each
35,640-color workload. Estimates below are medians of two fresh-process
medians in before/after/after/before order, pinned to CPU 2, without concurrent
builds/tests/benchmarks. Input and source hashes, runtime versions, raw passes,
checksums and orchestration source are in the report. Factory setup is excluded.
The machine is an AMD Ryzen 7 9800X3D under WSL2; Node 26.10.0 and Bun 1.4.2.

| Runtime / P3 method | Grid before → after, ns | Random before → after, ns |
| --- | ---: | ---: |
| node / oklch-cubic | 67.93 → 65.72 (-3.2%) | 96.29 → 93.44 (-3.0%) |
| node / oklch-cubic-no-cache | 243.75 → 234.83 (-3.7%) | 265.92 → 261.83 (-1.5%) |
| node / oklch-cubic-direct | 208.87 → 206.06 (-1.3%) | 236.02 → 232.34 (-1.6%) |
| node / oklch-halley | 102.44 → 97.92 (-4.4%) | 122.01 → 120.39 (-1.3%) |
| node / oklch-ostrowski | 100.37 → 96.09 (-4.3%) | 122.95 → 120.30 (-2.2%) |
| node / raytrace | 201.01 → 208.41 (+3.7%) | 218.82 → 233.71 (+6.8%) |
| bun / oklch-cubic | 61.68 → 62.99 (+2.1%) | 81.74 → 81.99 (+0.3%) |
| bun / oklch-cubic-no-cache | 239.65 → 233.98 (-2.4%) | 255.71 → 253.46 (-0.9%) |
| bun / oklch-cubic-direct | 217.22 → 218.76 (+0.7%) | 242.16 → 245.69 (+1.5%) |
| bun / oklch-halley | 90.41 → 90.91 (+0.6%) | 112.03 → 111.87 (-0.1%) |
| bun / oklch-ostrowski | 94.18 → 93.35 (-0.9%) | 116.40 → 116.10 (-0.3%) |
| bun / raytrace | 254.01 → 232.56 (-8.4%) | 273.24 → 253.82 (-7.1%) |

New-target timings have no earlier JS solver baseline:

| Runtime / target | Cached cubic | No-cache | Direct | Halley | Ostrowski | Raytrace |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| node / srgb | 71.26 / 101.10 | 248.08 / 266.20 | 211.48 / 234.13 | 107.31 / 130.83 | 103.48 / 127.15 | 211.48 / 231.68 |
| node / rec2020 | 83.04 / 106.82 | 251.93 / 272.61 | 223.04 / 246.69 | 119.00 / 139.31 | 119.76 / 140.94 | 215.83 / 234.67 |
| bun / srgb | 66.72 / 85.42 | 245.32 / 265.54 | 218.45 / 244.26 | 117.25 / 139.05 | 118.02 / 139.74 | 232.71 / 264.55 |
| bun / rec2020 | 74.07 / 91.82 | 251.61 / 270.84 | 235.22 / 254.45 | 127.12 / 148.86 | 130.69 / 154.22 | 240.43 / 269.85 |

Cells in the new-target table are grid / random nanoseconds per mapping.
The integer-hue grid misses the narrow fold windows; random timing includes
them at their natural frequency, not as a dedicated worst-case latency test.
Small differences are subject to JIT placement and process variation.
Rust kernel timings were not rerun; its two shared numerical fixes are covered
by release tests and cross-language parity.

The full thirteen-method P3 Mitata harness was also measured in balanced
before/after/after/before order. It has a different call history from the
isolated harness; its raw rows and all comparisons are retained in the report.

| Full-harness P3 method | Node grid / random change | Bun grid / random change |
| --- | ---: | ---: |
| oklch-cubic (cached) | -2.5% / -4.1% | -1.1% / +0.0% |
| oklch-cubic (no cache) | -1.6% / -2.7% | +0.9% / +0.7% |
| oklch-cubic-direct | -1.5% / -4.4% | +1.0% / +1.8% |
| oklch-halley | -6.1% / -2.7% | -0.1% / +0.7% |
| oklch-ostrowski | -5.7% / -2.8% | -0.6% / -0.1% |
| raytrace | +7.6% / +5.6% | -6.0% / -2.8% |

The factory conversion produced no broad P3 solver regression. The material
tradeoff is Node Raytrace: the full harness measures +7.6% grid and +5.6%
random after fixing its numerical paths; Bun measures -6.0% and -2.8%.
These compare complete implementations, not an isolated cost attribution to
the factory or any one guard. Other P3 methods have no measured slowdown over
5% in the full harness. The isolated and full results are both retained.

## Step-two review follow-up

The fold-gap policy is now explicit: **reduce chroma to the greatest feasible
value at or below the input**. Previously, `min(input, outerBoundary)` retained
out-of-gamut colors between disconnected intervals, leaving the final RGB
clamp to change lightness and hue. Both JavaScript and Rust now bound their
fold search by input chroma and retain the input only if it is inside.

At sRGB `[0.3, 0.177897, 264.053]`, the mapped chroma is about `0.1763191673`;
at Rec.2020 `[0.2, 0.15757, 245.067]`, it is about `0.1563465935`.
Tests sweep the gap and valid re-entry intervals, check Oklab coordinates
against the authored lightness/hue line, and assert that chroma never grows.
Expected matrix-mapper colors are no longer clipped in the JS oracle; the
Rust gap regression also compares unclipped reference colors.

The other review findings are addressed as follows:

- The cache-isolation test derives expected colors from independent XYZ
  geometry and alternates target order. It passes when run alone and rejects
  a mutant with a global hue cache.
- Algebraic cancellation fixtures check compensated evaluation independently:
  `(x-1)^3` at exact binary `x` has a known nonzero residual that ordinary
  Horner loses. A plain-Horner mutant fails this test. Evaluation now indexes
  the coefficient array without allocating another array per call; the rare
  fold solve still allocates small setup arrays.
- CLI smoke tests run the real validation-to-timing child flow for sRGB and
  Rec.2020 from an unrelated working directory, using an absolute script path.
  They assert target headings and actual timing rows. Dropping child `--gamut`
  is detected. Explicit test timeouts allow the real subprocesses to finish.
- `--validate-only` checks the registered callbacks used for timing, including
  mixed-chroma probes, fold gaps, canonical pass-through and checked/plain
  agreement outside gamut for every target. A wrong timed callback is rejected.
  Policies, benchmark bindings and Rust parity columns are selected by name.
- Rust near-white Raytrace tests use precision-specific exponents and assert
  every probe satisfies `0 < L < 1`; f32 no longer tests the white early return
  accidentally for its deepest probes.
- The literal MINDE search reference accepts normalized finite input and
  contains no negative-chroma or huge-hue normalization. Separate tests use
  independent neutral XYZ conversion and exact BigInt hue remainders for the
  API's normalization behavior.
- `getRgbConversions(space)` shares immutable conversion instances by descriptor
  identity in a WeakMap for every target, including the legacy P3 adapters.
  The common LMS-to-Oklab coefficients have one production definition in
  `src/oklab.js`; the independent test matrices remain separate.
- `benchmark-workloads.js` supplies the PRNG and both workloads to the full and
  focused harnesses. Historical input hashes remain unchanged.
- The obsolete Raytrace flushing explanation has been removed from both ports.

Raytrace's gray path intentionally follows canonical matrix conversion.
At P3 `[0.001, 0, h]`, this can produce last-bit channel differences rather
than three identical encodings of `L^3`. Checked and plain Raytrace agree with
the canonical conversion; Dualray retains its own exactly-neutral policy.
This change already belonged to the initial step-two port and is now explicit.

Five mutation probes were run only in scratch copies: shared target cache,
plain Horner, unbounded fold mapping, wrong timed callback, and missing child
gamut forwarding. Every mutation was rejected by its corresponding check.

Follow-up measurements and test evidence are recorded separately in
[the review report](reports/milestone-4-step-2-review.json); the original
step-one and step-two reports above retain their historical source hashes.

The follow-up passes **44 tests in each JS runtime** and **117 Rust tests**
in both portable and native release builds. All-gamut validation passes in
both JS entry modes and in Rust. Rust/JS parity includes 55,304 inputs per
target; P3 compatibility compares all thirteen methods in both modes against
the pre-review snapshot. All P3 outputs are bit-identical on that corpus.

Performance uses the same Ryzen 7 9800X3D/WSL2 machine, Node 26.10.0 and
Bun 1.4.2, with before/after/after/before order, CPU 2, fresh processes, 50
warmup passes and 25 measured passes for focused runs. All outputs are consumed;
setup is excluded. No tests/builds ran alongside timing. Both shared-workload
hashes match the original step-two manifest. Small changes remain subject to
JIT and process variation. The full harness and isolated results are recorded
separately.

| Full-harness P3 method | Node grid / random change | Bun grid / random change |
| --- | ---: | ---: |
| clip | -1.5% / -0.9% | -0.3% / -0.5% |
| css-minde | -0.4% / -1.4% | -6.2% / -6.6% |
| oklch-cubic (cached) | -1.2% / -0.4% | -1.3% / -5.0% |
| oklch-cubic (no cache) | -0.6% / +0.7% | +1.5% / +2.1% |
| oklch-cubic-direct | +0.3% / +1.5% | -0.1% / +0.1% |
| oklch-halley | +0.7% / +0.1% | +1.0% / -3.7% |
| oklch-ostrowski | +0.7% / -0.5% | +1.3% / -1.5% |
| raytrace | +0.4% / +0.5% | -0.8% / -0.7% |

Across all thirteen P3 methods, the largest full-harness slowdown was 2.1%.
The isolated harness did measure a Bun sRGB cached-cubic slowdown: grid
63.87 → 67.75 ns (+6.1%) and random 83.63 → 88.18 ns (+5.4%). No other
isolated runtime/target/method case slowed by more than 5%. These measurements
do not establish a cause for that isolated change; raw process results are
retained in the report.

The dedicated fold workload has 128 colors per case, with four lightnesses
and 32 hues inside each target window. The table shows Halley nanoseconds per
mapping; Ostrowski results and all pass timings are in the report. Early
acceptance of valid input colors and shorter brackets contribute alongside
the allocation cleanup, so these are combined implementation changes.

| Runtime / target | Interior before → after | Mixed before → after | Exterior before → after |
| --- | ---: | ---: | ---: |
| node / srgb | 4380.9 → 180.2 | 4180.6 → 1063.8 | 4143.4 → 1532.3 |
| node / rec2020 | 3832.2 → 169.6 | 4212.1 → 1045.1 | 3949.6 → 1337.4 |
| bun / srgb | 5841.5 → 708.9 | 5769.7 → 1730.8 | 5759.5 → 2400.9 |
| bun / rec2020 | 5344.5 → 720.5 | 5413.8 → 1570.6 | 5321.7 → 1574.7 |

An experiment with factory-local copies of the shared Oklab coefficients
did not improve paired Bun timings; the final implementation keeps direct
immutable imports. The experiment is retained separately from final estimates.

## Step three: Bottosson

The initial corpus and measurements below are historical. The
[review follow-up](#review-follow-up-validation-and-boundary-parity) expands
boundary coverage and supersedes the original parity and compatibility maxima.

Both `createBottossonLightness(space)` and
`createBottossonLightnessCached(space)` now support all three targets. Legacy
exports remain P3 instances. `scripts/generate-bottosson.mjs` emits immutable
JavaScript fit data alongside the existing Rust files from the same fit result.
The Rust files are unchanged; P3 seed coefficients remain pinned. Algorithm
fits and primary-sector boundaries stay separate from physical RGB descriptors.

Factories capture scalar matrix coefficients, transfer functions and shared
conversion kernels. Cached instances own a 3,601-entry Float64Array of
`[cuspL,cuspC,q0,q1,q2]`; plain factories do not allocate that table. Neither
mapping path allocates a new output or temporary color array per call.

The port follows the Rust f64 policy:

- Plain Bottosson evaluates the authored hue. Cached Bottosson uses 0.1-degree
  hue buckets for its fitted cusp, intersection and output conversion.
- Without a precheck, even an interior color is projected to the approximate
  boundary. This deliberately differs from the first-exit matrix solvers.
- With a precheck, canonical conversion uses the original coordinates before
  consulting the cached hue. A successful check preserves its exact output.
- A five-term saturation seed receives one Halley correction. The upper
  intersection receives one further correction. Final clipping belongs to this
  approximation; it is not evidence of first-exit or exact-line accuracy.
- Primary-sector contacts use Rust's authored-hue tie handling. The lightness
  blend uses `l0 + t*(l-l0)`, retaining constant lightness exactly when `l0=l`.

Compared with legacy P3 JavaScript, checked cached colors now retain their
original hue instead of a quantized hue. Checked zero/tiny/negative chroma also
uses canonical input conversion when it is in gamut; the unchecked gray policy
remains unchanged. The radian arithmetic and lightness blend now match Rust,
so last-bit differences are expected in ordinary P3 mappings too.

The independent policy reference recovers channel cubics from four XYZ
conversions and differentiates those polynomials. It shares only the fitted
seed parameters, not production matrices, sector selection or derivative code.
Within 1e-10 degrees of a primary, independently rounded geometry may select
either adjacent face; the test accepts only those two policies, at the usual
error limits. Separate geometry tests measure saturation, first-exit chroma
and clipping-induced Oklab error. They retain the existing Rust approximation
envelopes, rather than weakening the arithmetic parity limit to fit them.

Tests cover both modes, endpoints, mixed chroma, primary contacts and wraps,
blue folds, near-white/black, output aliasing, exact pass-through and alternating
target caches. A wrong supplied callback or NaN is rejected. The CLI validates
its actual timed callbacks and ran ten methods for sRGB/Rec.2020 at this
checkpoint; Edge Seeker and Dualray were still P3-only. Both Node and Bun smoke tests require the
new cached Bottosson timing row from the normal validation-to-timing flow.

```sh
node scripts/generate-bottosson.mjs --check
node --test tests/bottosson.test.js
bun test tests/bottosson.test.js
node scripts/check-bottosson-parity.mjs
bun scripts/check-bottosson-parity.mjs
node scripts/bench-rgb-kernels.mjs --method bottosson-lightness-cached --gamut srgb
```

### Validation results

Node and Bun each pass **53 tests**; Rust release passes **117 tests**.
All-gamut CLI validation passes in both runtimes and both entry modes. Both
profile and Bottosson generators pass their freshness checks. The independent
policy and Rust parity corpora each contain **56,641 inputs per target**, or
679,692 mapped outputs per runtime across two methods and two entry modes.

The policy gates remain `2e-11` in both linear RGB and DeltaEOK. Node's maximum
independent policy linear error is `3.39e-14`; its maximum Rust parity linear
error is `6.44e-15` (DeltaEOK `1.79e-15`). Bun matches Rust exactly on this
corpus. Encoded Node/Rust differences reach `2.23e-8` near zero in Rec.2020.
These are corpus/runtime-specific observations, not full-domain guarantees.

The separate quality sweep uses 4,001 hues per target, with dense blue-primary
contacts and nine lightness positions per non-ambiguous hue. The following
Node results measure the fitted policy, independently of porting error:

| Target | Saturation error | First-exit chroma error | Clipping DeltaEOK |
| --- | ---: | ---: | ---: |
| srgb | 0.000384 | 0.0471 | 0.000159 |
| display-p3 | 0.028 | 0.00839 | 5.18e-05 |
| rec2020 | 0.000168 | 0.0513 | 9.59e-05 |

P3 compatibility compares all thirteen methods over 56,641 inputs in both
modes. The eleven methods outside Bottosson remain bit-identical in both
runtimes. Bottosson changes include the canonical-check corrections described
above, primary-sector contacts, and ordinary arithmetic rounding. Counts and
worst inputs by mode/chroma category are retained in the report.

The original 56,641-input comparison observed 0.03169 encoded for uncached
positive-chroma output and 0.00422 for cached checked output. Those were sampled
maxima, not limits. The expanded 221,721-input comparison now observes **0.15141**
uncached at `[0.412,0.4,-95.94797738363006]`, a blue-primary sector contact, and
**0.03423** cached checked at a boundary near that primary. Cached unchecked
changes remain below `1.84e-13`. The largest negative-chroma change remains
0.26753 at `[0.5,-0.1,-95.9]`. See the review follow-up for the corpus and report.

### Performance results

[The Bottosson report](reports/milestone-4-bottosson.json) records source and
input hashes, validation maxima, raw timings and reproduction scripts. The
baseline is commit `c0a5715`. Balanced before/after/after/before runs use the
Ryzen 7 9800X3D under WSL2, Node 26.10.0 and Bun 1.4.2, CPU 2, fresh processes
and a common script path, without concurrent tests/builds/benchmarks. Focused
runs use 50 workload warmups and 25 measured passes; every output channel is
consumed. Cache/factory setup is excluded. Values are medians of two process
medians; full-harness values below use two Mitata process averages.

| Isolated P3 runtime / method | Grid ns, before → after | Random ns, before → after |
| --- | ---: | ---: |
| node / bottosson-lightness | 110.07 → 111.84 (+1.6%) | 121.35 → 125.60 (+3.5%) |
| node / bottosson-lightness-cached | 62.01 → 54.77 (-11.7%) | 84.24 → 79.87 (-5.2%) |
| bun / bottosson-lightness | 100.76 → 108.62 (+7.8%) | 117.07 → 126.74 (+8.3%) |
| bun / bottosson-lightness-cached | 46.43 → 50.41 (+8.6%) | 66.24 → 71.38 (+7.8%) |

| Full-harness P3 runtime / method | Grid ns, before → after | Random ns, before → after |
| --- | ---: | ---: |
| node / bottosson-lightness | 116.02 → 124.44 (+7.3%) | 133.14 → 141.84 (+6.5%) |
| node / bottosson-lightness (cached) | 66.22 → 64.53 (-2.5%) | 90.49 → 87.12 (-3.7%) |
| bun / bottosson-lightness | 104.10 → 113.64 (+9.2%) | 120.65 → 131.17 (+8.7%) |
| bun / bottosson-lightness (cached) | 51.77 → 57.94 (+11.9%) | 72.95 → 77.02 (+5.6%) |

The full harness measures an uncached P3 regression of 6.5–9.2% across the
two runtimes/workloads. Cached P3 improves 2.5–3.7% in Node but regresses
5.6–11.9% in Bun. These costs remain a performance follow-up; they are not
treated as equivalent to the faster isolated Node results.

| New target / runtime | Plain grid / random, ns | Cached grid / random, ns |
| --- | ---: | ---: |
| srgb / node | 111.66 / 129.20 | 54.68 / 80.80 |
| rec2020 / node | 121.08 / 129.70 | 62.13 / 77.55 |
| srgb / bun | 112.57 / 131.71 | 50.34 / 71.70 |
| rec2020 / bun | 122.15 / 143.12 | 59.76 / 73.65 |

The comparison includes numerical/canonical corrections as well as factory
conversion; it does not isolate the cost of abstraction or any single check.
sRGB/Rec.2020 have no prior JS Bottosson implementation, so their table reports
final throughput only. Small changes remain subject to JIT/process variation.
Rust mapping code is unchanged; Rust timings were not repeated.

## Step four: Edge Seeker

The initial generated-table implementation and its measurements are recorded
below. The [runtime-generation follow-up](#runtime-table-generation-follow-up)
supersedes its JavaScript setup and cross-runtime parity claims.

`createEdgeSeeker(space)` and `createEdgeSeekerIndexed(space)` now support
sRGB, Display-P3 and Rec.2020. Existing `edgeSeeker`/`edgeSeekerIndexed` exports
remain P3 instances. The benchmark runs twelve methods for sRGB/Rec.2020 and
thirteen for P3, with actual timed callbacks included in validation.

`scripts/generate-edge-seeker.mjs` emits immutable JavaScript rows alongside
the existing Rust tables from the same `makeLut(..., 400)` result. Rust table
files and runtime code are unchanged. Rows are algorithm data, separate from
physical RGB descriptors. Table counts are 695 / 710 / 790 for sRGB / P3 /
Rec.2020. JavaScript uses the binary64 knots directly; it does not need Rust's
f32 split-hue conditioning around the repaired folds.

The lookup kernels retain parallel numeric columns. A private WeakMap shares
those columns by table identity, and indexed factories share a lazily built
3,600-entry Uint16 interval index (~7 KiB per target). Arrays are never exposed
through the returned mapper. Runtime factory creation performs no gamut-edge
sampling; the original converter-based builders remain available for tests and
experiments. Mapping reuses the supplied output and allocates no color arrays.
The generated row arrays and shared runtime columns are both retained; this
trades additional data storage for deterministic tables and the existing hot
lookup layout. Setup and import costs are excluded from warm timing below.

Both variants preserve the existing policy: interpolate cusp lightness,
chroma and curvature at the exact normalized hue, use a straight lower edge
and a rationalized circle arc above the cusp, cap input chroma, then convert
and clip. The index only finds the interval; it never rounds the hue. Negative
chroma follows the incumbent signed conversion policy. Checked calls preserve
canonical authored-coordinate conversion before table lookup or endpoint
handling, matching Rust f64.

Validation uses a separate linear scan, difference-form interpolation,
bisection of the circle residual, and independent XYZ conversions. It shares
table rows as policy parameters, not production lookup or arc formulas.
Separate tests compare the approximation with geometric first exit and measure
clipping error under the existing Rust envelopes. In particular, the table's
repaired blue fold is an approximation, not an exact first-exit solver.

Coverage includes every knot and its adjacent floats, negative and repeated
hues, cusp and white neighbours, dense samples inside the ~0.0001-degree fold
ramps, mixed/negative/zero chroma, canonical pass-through, aliased outputs and
interleaved targets. Both lookup variants must agree exactly. Wrong callbacks,
a wrong target and NaN are rejected. CLI smoke tests require the new indexed
row from each target's real validation-to-timing child flow.

```sh
node scripts/generate-edge-seeker.mjs --check
node --test tests/edge-seeker.test.js tests/edge-seeker-targets.test.js
bun test tests/edge-seeker.test.js tests/edge-seeker-targets.test.js
node scripts/check-edge-seeker-parity.mjs
bun scripts/check-edge-seeker-parity.mjs
node scripts/bench-rgb-kernels.mjs --method edge-seeker-indexed --gamut rec2020
```

### Validation results

Node and Bun each pass **65 tests**; Rust release passes **117 tests**.
All-gamut CLI validation passes in both runtimes and both entry modes. The
RGB, Bottosson and Edge Seeker generators all pass freshness checks.

| Target | Policy/parity inputs | Lookup endpoint probes |
| --- | ---: | ---: |
| srgb | 152,075 | 43,785 |
| display-p3 | 151,559 | 44,730 |
| rec2020 | 164,900 | 49,770 |

Each runtime checks **1,874,136 mapping outputs** across three targets, two
variants and two modes. The independent policy limit is `3e-12` in linear RGB
and DeltaEOK; the maximum Node linear error is `8.47e-15`. The Rust
f64 parity maximum is `3.11e-15` linear (`3.9e-07` encoded); Bun matches Rust
exactly on this corpus. Lookup chroma error is independently limited to
`2e-12`. These are sampled, runtime-specific regression results.

Approximation results from the Node geometry sweep are separate from those
arithmetic limits:

| Target | Quality inputs | First-exit chroma error | First-exit DeltaEOK | Clipping DeltaEOK |
| --- | ---: | ---: | ---: | ---: |
| srgb | 40,464 | 0.0471 | 0.0471 | 0.0028 |
| display-p3 | 38,790 | 0.0217 | 0.0206 | 0.00258 |
| rec2020 | 41,319 | 0.0513 | 0.0513 | 0.00403 |

P3 compatibility covers all thirteen methods over 151,559 inputs in both
entry modes. The other eleven methods are bit-identical in Node and Bun. In
Node, both Edge Seeker variants change only sixteen checked-white samples
(`[1,0,h]`, including repeated inputs): canonical conversion produces channels
within `3.33e-16` of one instead of the old exact-white shortcut. Unchecked
Node output remains bit-identical.

Bun previously built its LUT using its own runtime math. Its table differs
from the recorded Node-generated table by at most `2.27e-13` per component.
Using the fixed table changes 45,805 outputs per variant on this corpus,
including checked white, with maximum encoded difference `5.38e-14`. This
keeps the runtime table identical to Rust's generated binary64 data.

### Performance results

[The Edge Seeker report](reports/milestone-4-edge-seeker.json) records hashes,
raw measurements, validation evidence, reproduction scripts and a baseline
patch. The baseline is the completed **uncommitted Bottosson snapshot**, not
commit `c0a5715` directly; its patch from that commit is included.

Before/after/after/before measurements use the Ryzen 7 9800X3D under WSL2,
Node 26.10.0 and Bun 1.4.2, CPU 2, fresh processes and a common script path.
No tests/builds/other benchmarks run concurrently. Focused runs warm 50 full
workload passes and measure 25; each output channel is consumed. Setup, table
generation/import and index construction are excluded from warm timing.
Focused results are medians of two process medians; full-harness results
are medians of two rendered Mitata process averages.

| Isolated P3 runtime / method | Grid ns, before → after | Random ns, before → after |
| --- | ---: | ---: |
| node / edge-seeker | 92.72 → 92.64 (-0.1%) | 149.88 → 149.23 (-0.4%) |
| node / edge-seeker-indexed | 63.91 → 60.79 (-4.9%) | 78.83 → 78.57 (-0.3%) |
| bun / edge-seeker | 95.71 → 95.98 (+0.3%) | 144.39 → 145.29 (+0.6%) |
| bun / edge-seeker-indexed | 59.14 → 60.25 (+1.9%) | 74.21 → 75.55 (+1.8%) |

| Full-harness P3 runtime / method | Grid ns, before → after | Random ns, before → after |
| --- | ---: | ---: |
| node / edge-seeker | 105.22 → 101.01 (-4.0%) | 164.70 → 160.21 (-2.7%) |
| node / edge-seeker (indexed) | 76.74 → 73.37 (-4.4%) | 89.79 → 88.66 (-1.3%) |
| bun / edge-seeker | 103.54 → 104.24 (+0.7%) | 149.97 → 148.01 (-1.3%) |
| bun / edge-seeker (indexed) | 60.19 → 61.73 (+2.6%) | 78.70 → 79.69 (+1.2%) |

| New target / runtime | Binary grid / random, ns | Indexed grid / random, ns |
| --- | ---: | ---: |
| srgb / node | 93.77 / 151.35 | 63.20 / 78.37 |
| rec2020 / node | 98.27 / 152.05 | 64.45 / 80.18 |
| srgb / bun | 107.23 / 148.42 | 62.06 / 78.15 |
| rec2020 / bun | 125.67 / 153.77 | 66.48 / 79.66 |

The comparison measures the complete generated-table/factory implementation,
including shared lookup setup and the canonical endpoint correction. It does
not isolate a cost for any single change. sRGB/Rec.2020 have no prior JS
baseline; their numbers report final throughput only. Small changes remain
subject to JIT/process variation. Rust mapping code is unchanged and its
timings were not repeated.

## Next step at the Edge Seeker checkpoint

Dualray was the remaining port; it is implemented in [step five](#step-five-dualray).


### Runtime table generation follow-up

JavaScript now builds tables at runtime, as requested. `makeLut(..., 400)`
runs once per conversion-function identity, supplied by the descriptor-keyed
conversion cache. Binary and indexed factories share the sampled rows and
parallel numeric columns. The index is still allocated only on first indexed
use. Repeated factories do not resample; separate descriptors with identical
names do not share data. Mapping has no additional cache lookup or generation.

The default P3 exports build once at import, preserving the original lifecycle.
sRGB and Rec.2020 build on their first factory call. The roughly 220 KiB static
JS table moved out of production source into `tests/fixtures/edge-seeker.js`;
the generator maintains this Rust-parity fixture, while Rust tables and runtime
code remain unchanged. Sampled rows and lookup columns are both retained.

Node and Bun each pass **67 tests**, including counted conversion calls proving
one build per descriptor in either factory order, and independent checks of
runtime knots against the fixed fixtures. Lookup/arc checks rebuild the table
in the current runtime, then use independent scan/interpolation/arc math;
their existing `3e-12` linear RGB and DeltaEOK limits remain unchanged. Every
runtime knot and its adjacent floats are covered. The same geometric
approximation envelopes still pass.

The recorded Node runtime reproduces the fixture rows. Bun's maximum knot
component difference is `2.27e-13`; steep repaired fold intervals amplify it.
On the 1,874,136-output parity corpus per runtime, Bun versus Rust reaches
`1.42e-10` linear RGB and `3.01e-11` DeltaEOK (sRGB). Rec.2020 reaches
`6.77e-11` linear and `2.49e-5` encoded near zero. P3 remains within `9.66e-15`
linear and `5.38e-14` encoded. Node's maximum remains `3.11e-15` linear.
These measured differences arise from runtime table generation. The parity
script allows `2e-10` linear / `5e-11` DeltaEOK only inside the existing blue-fold
windows, retaining `3e-12` elsewhere. This is separate from same-runtime lookup
accuracy and from the much larger geometric approximation envelopes.

[The runtime-generation report](reports/milestone-4-edge-seeker-runtime.json)
records the preceding uncommitted generated-table baseline, source hashes,
measurement script, all raw passes, and parity results. This is a new comparison,
not a replacement for the earlier port report. Ryzen 7 9800X3D / WSL2,
Node 26.10.0 and Bun 1.4.2, CPU 2, before/after/after/before fresh processes at a
common script path, with no concurrent tests or builds. Warm results are medians
of two process medians, each with 50 warmup and 25 measured output-consuming
passes. Imports, generation and index construction are excluded here.

| Runtime / target / method | Grid ns, fixed → runtime | Random ns, fixed → runtime |
| --- | ---: | ---: |
| node / display-p3 / edge-seeker | 91.15 → 94.07 (+3.2%) | 152.62 → 151.97 (-0.4%) |
| node / display-p3 / edge-seeker-indexed | 64.68 → 64.48 (-0.3%) | 79.90 → 79.15 (-0.9%) |
| node / srgb / edge-seeker | 95.05 → 93.62 (-1.5%) | 153.75 → 156.19 (+1.6%) |
| node / srgb / edge-seeker-indexed | 65.32 → 63.82 (-2.3%) | 80.73 → 80.51 (-0.3%) |
| node / rec2020 / edge-seeker | 103.87 → 102.91 (-0.9%) | 156.11 → 154.76 (-0.9%) |
| node / rec2020 / edge-seeker-indexed | 70.00 → 66.75 (-4.6%) | 81.62 → 79.92 (-2.1%) |
| bun / display-p3 / edge-seeker | 98.42 → 95.44 (-3.0%) | 147.24 → 150.95 (+2.5%) |
| bun / display-p3 / edge-seeker-indexed | 61.15 → 61.44 (+0.5%) | 78.52 → 74.85 (-4.7%) |
| bun / srgb / edge-seeker | 110.37 → 110.44 (+0.1%) | 148.52 → 149.82 (+0.9%) |
| bun / srgb / edge-seeker-indexed | 65.03 → 63.50 (-2.4%) | 76.85 → 77.94 (+1.4%) |
| bun / rec2020 / edge-seeker | 129.24 → 126.30 (-2.3%) | 159.90 → 155.07 (-3.0%) |
| bun / rec2020 / edge-seeker-indexed | 68.79 → 67.95 (-1.2%) | 80.58 → 80.24 (-0.4%) |

Warm changes span −4.7% to +3.2%; this does not establish an improvement from
runtime generation. Lookup kernels are unchanged. Startup was measured
separately over ten fresh processes per runtime/version (warm filesystem,
process launch excluded). Median import including P3 setup rose from 7.87 to
16.58 ms in Node and 7.25 to 15.19 ms in Bun. First sRGB/Rec.2020 binary factory
calls take 5.11/3.83 ms in Node and 4.67/6.95 ms in Bun, in that target order
after P3 setup. Following indexed factories take about 0.015–0.049 ms and reuse
the sampled table. These startup costs are paid once per descriptor.

All-gamut `--validate-only` also passes in Node and Bun with and without
`--in-gamut-check`; the generator freshness check passes.


### Review follow-up: validation and boundary parity

Validators now invoke registered callbacks as `(input, out)`, exactly like the
timing loop. The checked flag controls the expected policy, never the invocation.
Default test mappers are explicitly bound to their mode. The benchmark selects
the actual timing callbacks by name through one shared family-validation loop.
A planted raw Bottosson mapper in the checked timing slot now makes
`--validate-only` fail. Direct regression tests cover all three mapper families.

Canonical membership and pass-through expectations no longer call production
conversion functions. A native-operation-order reference uses the descriptor's
coefficients with its own arithmetic, encoding and strict membership predicate;
a separate XYZ calculation constrains the physical result. This preserves exact
canonical-coordinate checks while detecting a tolerance added to production
membership. The planted ±1e-7 predicate now fails Bottosson and Edge Seeker policy
tests, rather than being accepted by both actual and expected paths.

The Edge Seeker reference still shares edge sampling/filtering as algorithm
parameters, but independently recovers curvature from the circle equation and
uses separate lookup and arc evaluation. It no longer calls `makeLut` or
`calculateCurvature`. A planted curvature multiplier of `1+1e-6` fails all three
target policy tests as well as the fixture check. Fixtures are explicitly
**recorded shared-generator data**, not an independent Rust implementation.
Runtime tables are validated once before column/index construction: finite
values, physical cusp/curvature ranges, strictly increasing hue, matching wrap
endpoints at 0/360, and an indexable size. Invalid tables throw during setup.

Bottosson parity now includes independently located first-exit boundaries,
adjacent chroma floats, points just inside/outside, primary neighbours and hue
wraps: **71,871 inputs per target**, 862,452 outputs per runtime. Rust probes
return each runtime's membership decision and canonical channels. Shared parity
code verifies both checked outputs against their own runtime's branch; unchecked
output and same-branch checked output must still meet `2e-11` linear RGB and
DeltaEOK. A branch disagreement is accepted only when native linear conversions
agree within `3e-14` and independent XYZ places the input at a rounding-scale
gamut face. These events are counted and their output differences reported,
not folded into a looser numerical threshold.

Node/Rust Bottosson membership differs on 15 / 21 / 12 inputs for sRGB / P3 /
Rec.2020 in this corpus. The largest checked difference is **0.00262 linear**
for cached sRGB. Plain and same-branch checked errors remain below `6.56e-15`
linear. Bun/Rust has no membership disagreements and matches these outputs
exactly on this machine. These are runtime/corpus observations, not a claim of
full-domain cross-runtime equivalence. NaN and Infinity behavior remains outside
the finite-input parity contract and has not been harmonized.

Edge Seeker's parity allowance now follows its own steep table intervals,
derived from runtime and fixture knots, instead of matrix-solver fold windows.
The `2e-10` linear / `5e-11` DeltaEOK allowance applies only there; ordinary
regions retain `3e-12`. Wrapped knot neighbours are computed **after** adding the
hue offset, and probe serialization preserves negative zero. The shared Rust
transport confirms its sign. Both new parity scripts share their transport,
branch checks and reporting; both Rust examples share their probe implementation.

Bottosson factories reject arbitrary descriptors borrowing a known gamut id:
pre-fitted data is supported only for the exported physical descriptors. Edge
Seeker continues to support descriptors by runtime sampling. New factory-only
modules (`bottosson-factory.js` and `edge-seeker/factory.js`) perform no default P3
setup. The benchmark and parity tools import these modules, creating only their
selected target. Legacy modules retain default P3 instances and re-export the
factories. Their import-time P3 behavior is intentional compatibility behavior;
the repeated P3-special-case registry branches have been removed from both ports.

[The review evidence report](reports/milestone-4-review.json) records expanded Node/Bun compatibility and parity
results, source hashes, and the three planted mutations. No finite-input mapping
formula changed in this follow-up; production changes are setup validation,
descriptor checks and module organization.


Final checks: **79/79 Node**, **79/79 Bun**, **117/117 Rust release** tests;
all-gamut `--validate-only` in both entry modes and both JS runtimes; both
parity scripts in Node/Bun; all four generator `--check` commands; formatting
and whitespace checks. The scratch mutations each exit nonzero. The new
P3 compatibility maximum was also compared directly with Rust f64.

Factory separation was measured against the immediately preceding uncommitted
snapshot (including the validation fixes), not HEAD. Ryzen 7 9800X3D / WSL2,
Node 26.10.0, Bun 1.4.2; CPU 2, fresh processes, common script path,
before/after/after/before, no concurrent tests/builds. Setup is import plus
selected-target factory creation, excluding process launch, with a warm
filesystem; values are medians of six fresh processes per version/target.

| Family / target | Node setup ms, before → after | Bun setup ms, before → after |
| --- | ---: | ---: |
| bottosson / display-p3 | 2.37 → 1.64 | 2.47 → 2.56 |
| bottosson / srgb | 2.38 → 2.30 | 2.62 → 2.38 |
| bottosson / rec2020 | 2.13 → 2.20 | 2.46 → 2.43 |
| edge-seeker / display-p3 | 15.57 → 16.21 | 16.25 → 13.94 |
| edge-seeker / srgb | 22.90 → 15.61 | 20.78 → 14.35 |
| edge-seeker / rec2020 | 22.56 → 15.55 | 21.52 → 13.45 |

Warm throughput uses two process medians per version, 50 warmup and 25 measured
passes, reused output buffers and all-channel checksums. Checksums agree exactly
before/after. Across four methods, three gamuts, two runtimes and both workloads,
timing changes span −24.5% to +4.6%; the largest measured slowdown is Node P3
indexed Edge Seeker grid, 64.24 → 67.22 ns. The report retains every raw pass and
all per-method summaries. These isolated measurements do not attribute gains
to a particular JIT mechanism or replace full-harness timing evidence.


## Step five: Dualray

`createDualray(space)` in `src/dualray-factory.js` supports all three exported
RGB descriptors. `src/dualray.js` retains the default P3 instance and re-exports
the factory. Factories bind scalar basis coefficients, sectors, root limit,
seed function and transfer function once; custom descriptors borrowing a known
id are rejected. The benchmark now runs **all thirteen methods for every gamut**.

The existing generator emits JavaScript seed functions with the same balanced
expression trees as Rust, plus deeply frozen basis/sector data. P3 data remains
pinned to the incumbent coefficients. These are fixed polynomial coefficients,
not hue-indexed LUTs; there is no per-hue cache or runtime fitting. Rust mapping
code and generated coefficient files are unchanged.

The shared kernel retains the guarded upper-first shortcut, one lower Halley
step, two upper Householder steps, competing-face retry and stationary-interval
first-root fallback. Horner intermediates follow Rust f64. A stationary touch
is not an exit; exact endpoint roots require an outward derivative. The
sRGB/Rec.2020 fold windows use Rust's first-exit isolation policy, tracking the
smallest root. Interior endpoints are searched only up to the authored chroma;
re-entry does not authorize passing through the first exit. The selected exit
channel is assigned exactly 0 or 1 before encoding the remaining channels.
Ordinary calls allocate no arrays; fold/fallback paths use small temporary
arrays. Target selection and data binding stay outside mapping.

Dualray's checked row deliberately invokes the same intrinsic boundary policy
as its plain row. It does not add the canonical input-conversion precheck used
by the other optional-check methods. The parity runner has an explicit intrinsic
mode that checks plain/checked equality on both sides, while retaining the
existing canonical branch validation for Bottosson and Edge Seeker.

Independent tests use uncomposed XYZ conversions and first-exit geometry,
including input chroma in folded gaps and re-entry regions, exact face identity,
primary/boundary neighbours, fold-window edges and negative wraps, stationary
touches, large finite hue, extreme lightness and aliasing. They also check
interleaved factories and reject wrong-target/NaN callbacks. Runtime CLI
validation invokes the actual timed callback with two arguments in every gamut.

One reference defect was found while adding near-white probes: a neutral point
rounded exactly onto an upper face left the initial residual at zero. The old
bisection used that residual's sign and moved its lower endpoint outside.
The independent reference now uses the explicit face orientation to select the
inside half, with a direct regression at Rec.2020 `L=1-2^-53`, hue 80 degrees.
This changes the test oracle, not mapping policy.

Initial port validation (before the follow-up below): **89/89 Node**,
**89/89 Bun**, **117/117 Rust release** tests;
all-gamut `--validate-only` in both modes and runtimes; Dualray generator
`--check`; Bottosson, Edge Seeker and Dualray parity in both runtimes. The
Dualray parity corpus contains 109,576 sRGB, 71,054 P3 and 109,576 Rec.2020
inputs (580,412 plain/checked mappings per runtime). Its maximum JS/Rust linear
difference is **1.69e-14** in Node; Bun agrees exactly on this corpus. Both
linear and DeltaEOK parity gates are 2e-11, with no membership-branch exemptions.
Independent first-exit comparisons stay below **6.6e-12 linear**. Rec.2020's
encoded maximum against that reference is 5.83e-7: encoding magnifies tiny
linear residuals near zero. These are sampled results, not full-domain bounds.

Against checkpoint `1c06277`, P3 changes on 125 of 71,054 Node inputs and 122
Bun inputs, by at most **6.67e-16 encoded**. These last-bit changes follow the
Rust f64 arithmetic grouping. The report preserves compatibility inputs,
metrics, checksums and source hashes.

Performance was measured against `1c06277` on Ryzen 7 9800X3D / WSL2,
Node 26.10.0 and Bun 1.4.2. CPU 2, fresh processes, common script path,
before/after/after/before, no concurrent tests/builds. Focused kernels use
50 warmup and 25 measured passes, reused output buffers and all-channel
checksums. Numbers below are medians of two process results, in ns per mapping.

| P3 workload | Node before → after | Bun before → after |
| --- | ---: | ---: |
| Focused grid | 67.74 → 71.21 (+5.1%) | 63.08 → 67.37 (+6.8%) |
| Focused random | 86.74 → 89.10 (+2.7%) | 76.82 → 80.47 (+4.7%) |
| Full harness grid | 77.30 → 77.16 (−0.2%) | 64.67 → 69.87 (+8.0%) |
| Full harness random | 93.57 → 99.33 (+6.1%) | 79.69 → 84.88 (+6.5%) |

The P3 throughput regression remains. A separate balanced scratch experiment
split the seed evaluator's sector branches into smaller named functions; it
did not improve throughput consistently, so it was not adopted. These timings
do not establish whether the remaining cost comes from function calls, captured
target data or JIT code layout. No other method in the full harness slowed by
more than 5% in this run.

| New target / workload | Node ns | Bun ns |
| --- | ---: | ---: |
| sRGB grid / random | 70.35 / 90.65 | 64.08 / 83.76 |
| Rec.2020 grid / random | 74.90 / 89.28 | 68.66 / 84.27 |
| sRGB ordinary interior / mapped | 95.56 / 86.24 | 83.45 / 80.30 |
| sRGB fold interior / mapped | 239.19 / 536.08 | 207.12 / 414.00 |
| Rec.2020 ordinary interior / mapped | 94.61 / 91.79 | 88.03 / 82.60 |
| Rec.2020 fold interior / mapped | 237.75 / 568.37 | 255.60 / 448.21 |

The dedicated ordinary/fold workloads contain 4,096 fractional-hue inputs each;
interior chroma is `0.01*L`, mapped chroma is 0.6. They expose the cost of
first-exit isolation that integer-hue grid timings would miss. New targets have
no prior JS Dualray implementation to use as a before measurement.

Import plus factory setup, excluding process launch, uses medians of six fresh
processes per version/target with a warm filesystem. P3 is 1.98 → 1.80 ms in
Node and 2.43 → 2.68 ms in Bun. New sRGB/Rec.2020 setup is 2.50/2.07 ms in
Node and 2.41/2.38 ms in Bun. All three seed functions are imported; only the
selected target's mapper is created. No runtime table or cache is allocated.

[The Dualray report](reports/milestone-4-dualray.json) contains raw passes,
full-harness logs, setup measurements, the rejected seed-split experiment,
reproduction scripts and validation evidence. This completes the JavaScript
ports for milestone four: all thirteen methods support all three gamuts.

### Dualray review follow-up

Validation now enforces the benchmark's output-buffer contract: the callback
must return the supplied buffer, write every channel, and the validator reads
that buffer. The shared parity runner uses the same check for all three ported
families. A deliberately correct replacement array and an unwritten buffer are
both rejected.

The independent first-exit oracle now enforces exact selected-face values for
mapped inputs away from primary ties and rounding-scale neutral contacts.
Interior fold cases have their own numerical and strict-interior assertions.
A synthetic fold-kernel regression has a positive residual at its isolated
root, so deleting the snap cannot hide behind clipping or a numerical tolerance.
Separate regressions preserve exact upper-face snaps at rounding-scale contacts.

Dualray's Node parity limits are now **1e-13 linear and DeltaEOK**; Bun requires
**exact encoded equality** with Rust. Intrinsic parity invokes the JS function
once with the same two arguments as timing and reports one intrinsic category.
It still checks Rust's two distinct entry methods agree. It no longer calls
the same JS function with an ignored third argument or emits an empty boundary
disagreement category. Grey accuracy expectations use the independent XYZ
reference; exact neutrality remains a separate output contract.

Six scratch mutations are rejected: returning a fresh fold buffer, removing
ordinary face snapping, removing fold snapping, making Bernstein certification
unconditional, restoring the zero-residual bisection bug, and reducing the
Householder loop to one step. The last fails Node parity at sRGB
`[0.49,0.4,280]` (1.55e-13 linear and 1.74e-13 DeltaEOK).

The near-white Bernstein rejection branch still lacks a supported-gamut
reproducer. Its predicate is now shared with the fold shortcut and directly
tested with synthetic cubics whose endpoints are inside but whose interiors
cross a lower or upper face. This checks the certificate's mathematics, not
end-to-end reachability of the near-white fallback. The defensive guard remains.

JavaScript and Rust f64 now skip fold isolation when endpoint checks and
Bernstein controls certify the entire neutral-to-input interval. Inputs on
disconnected re-entry islands still use first-exit isolation. Contacts within
32 machine epsilons of a face also retain isolation/snapping; a first attempt
without that exclusion changed last-bit outputs at exact upper-face contacts.
With the exclusion, all 290,206 before/after probes are bit-identical in both JS
runtimes, and Bun still agrees exactly with Rust. Rust's f32 fold kernel is
unchanged.

The generator emits Rust and JS directly from the same fit data and expression
builder, with no source-code regex translation. `--check` confirms all generated
JS/Rust files are byte-identical to the prior port. Production matrix fold
bisection now uses explicit face orientation at a zero starting residual,
matching the reference fix, with a direct bisection regression. The focused
benchmark resolves the descriptor once and uses a factory-module lookup.

Final checks: **94/94 Node**, **94/94 Bun**, **119/119 Rust release** tests;
Node/Bun all-gamut CLI validation in both modes; Rust all-gamut validation;
all three shared parity scripts in Node/Bun; generator freshness and formatting.
The measured Node Dualray parity maximum remains 1.69e-14 linear; Bun is exact.

Follow-up timings compare the reviewed, uncommitted port with these fixes,
not checkpoint `1c06277`. Ryzen 7 9800X3D / WSL2, Node 26.10.0, Bun 1.4.2;
CPU 2, fresh processes, common JS script path, before/after/after/before,
no concurrent tests/builds. Each process uses 50 warmup and 25 measured passes,
reuses output buffers and consumes all channels. Values below are medians of
two process medians, in ns per mapping. Rust f64 uses portable release-equivalent
flags (`opt-level=3`, LTO, one codegen unit, panic abort) and `black_box`.

| Interior fold workload | Before ns | After ns | Change |
| --- | ---: | ---: | ---: |
| Node sRGB | 236.66 | 64.47 | −72.8% |
| Node Rec.2020 | 237.72 | 65.45 | −72.5% |
| Bun sRGB | 251.26 | 99.66 | −60.3% |
| Bun Rec.2020 | 215.01 | 96.70 | −55.0% |
| Rust f64 sRGB | 87.74 | 47.88 | −45.4% |
| Rust f64 Rec.2020 | 89.58 | 48.89 | −45.4% |

The fold workloads use the same 4,096-input manifest as the initial port's
dedicated measurements. Mapped fold workloads still isolate roots; their timing
changes range from −1.9% to +2.0% across the three runtimes. All corresponding
before/after checksums match exactly.

| Focused P3 workload | Node before → after ns | Bun before → after ns |
| --- | ---: | ---: |
| Grid | 72.22 → 70.87 (−1.9%) | 63.93 → 66.21 (+3.6%) |
| Random | 88.91 → 88.56 (−0.4%) | 80.78 → 79.06 (−2.1%) |

These focused measurements do not supersede the initial full-harness comparison
or establish that its P3 slowdown has been eliminated. Rust f32 is unchanged
and was not re-benchmarked. [The follow-up report](reports/milestone-4-dualray-review.json)
records all passes, source hashes, validation logs, parity, compatibility,
planted mutations and reproduction scripts.

### P3 performance investigation after checkpoint `99cc900`

The committed port still shows a P3 regression against `1c06277`. Instrumented
workloads have identical seed, shortcut, retry and fallback counts. V8 refuses
to inline the new 1,200-bytecode seed helper and boxes its floating-point
arguments and result. A scratch variant embedding the same seed expression in
the mapper improves throughput without changing sampled outputs. Descriptor
capture alone did not show the same consistent effect.

See [the investigation](reports/milestone-4-p3-dualray-investigation.md) for
controlled variants, assembly evidence, repeated focused measurements and full
harness confirmation. The experiments do not change production code. The
recommended next implementation is generated inline seeds from one shared
solver template, preserving current target data, arithmetic and guards.

### Inline seed generation

The recommendation is now implemented for all three gamuts. The maintained
mapper body lives in `scripts/templates/dualray-mapper.js`; the existing Dualray
generator inserts each target's balanced seed expression into that template.
Generated modules are `src/generated/dualray-{srgb,display-p3,rec2020}.js`.
`src/generated/dualray.js` contains the frozen basis/sector data and factory
registry, with no separate seed evaluator. `createDualray(space)` selects the
target once and returns its mapper; public imports and default P3 behavior are
unchanged.

Root isolation, fold mapping, polynomial evaluation and Halley polishing stay
shared in `src/dualray-kernel.js`. Physical target descriptors, current arithmetic,
near-white/first-exit guards, exact face snapping and the output-buffer contract
are retained. This adds no runtime fitting, LUT generation or dynamic code
evaluation. The generator requires exactly one explicit seed insertion point,
checks every emitted file with `--check`, and avoids rewriting unchanged files.
Rust source and generated files remain byte-identical to `99cc900`.

Before/after comparisons against `99cc900` are bit-identical on all **290,206
inputs per runtime** (109,576 sRGB, 71,054 P3 and 109,576 Rec.2020), including
boundary neighbours, folds, hue wraps and extreme lightness. **94/94 Node** and
**94/94 Bun** tests pass; all-gamut CLI validation passes in both entry modes.
Strict Rust parity remains at most **1.69e-14 linear** in Node and exactly equal
in Bun. Generator freshness passes, including its existing Rust f32 fold-window
edge regression. The full Rust suite was not rerun for this JS-only change.

V8's trace confirms that there is no separate seed evaluator to call or box
arguments for; the small shared numerical helpers still inline. Code-size
measurement with `bun build src/dualray.js --minify --target=browser` includes
the default P3 export and all-gamut factory. Its payload grows from **18,340 to
25,576 bytes**; gzip grows **8,575 → 8,876 bytes**, and Brotli **7,108 → 7,216
bytes**. Those are bundle payload measurements, separate from the unbundled
module timings below.

Performance is compared directly with `99cc900` on Ryzen 7 9800X3D / WSL2,
Node 26.10.0 and Bun 1.4.2. CPU 2, fresh processes, common script path,
before/after/after/before, no concurrent tests/builds. Focused kernels use
50 warmup and 25 measured passes, reused output buffers and all-channel
checksums. All corresponding checksums and workload hashes match exactly.
Tables report medians of two process results, in ns per mapping; full-harness
values come from Mitata's rounded batch averages.

| P3 full harness | Before ns | After ns | Change |
| --- | ---: | ---: | ---: |
| Node grid | 79.26 | 73.51 | -7.3% |
| Node random | 95.82 | 93.29 | -2.6% |
| Bun grid | 66.50 | 64.53 | -3.0% |
| Bun random | 81.93 | 79.12 | -3.4% |

| Focused target / workload | Node before → after ns | Bun before → after ns |
| --- | ---: | ---: |
| display-p3 grid | 71.91 → 67.98 (-5.5%) | 63.66 → 61.75 (-3.0%) |
| display-p3 random | 87.50 → 81.84 (-6.5%) | 79.54 → 76.08 (-4.4%) |
| srgb grid | 73.64 → 67.94 (-7.7%) | 65.10 → 60.75 (-6.7%) |
| srgb random | 87.70 → 85.23 (-2.8%) | 82.18 → 77.36 (-5.9%) |
| rec2020 grid | 76.89 → 70.67 (-8.1%) | 68.38 → 64.66 (-5.4%) |
| rec2020 random | 87.59 → 83.31 (-4.9%) | 81.72 → 80.14 (-1.9%) |

One Bun Rec.2020 fold-interior process was slower in the initial comparison,
yielding an 8.7% median increase from only two process medians. A targeted
three-ABBA repeat (six processes per version, identical workloads) did not
reproduce it: fold-interior **98.63 → 96.71 ns**, fold-mapped **425.38 → 425.99 ns**.
The original passes and the follow-up are both retained in the report.

Import plus selected-factory setup excludes process launch and uses a warm
filesystem, with six fresh-process samples per version/target:

| Target | Node before → after ms | Bun before → after ms |
| --- | ---: | ---: |
| display-p3 | 2.04 → 4.50 | 2.52 → 3.19 |
| srgb | 3.70 → 4.53 | 2.44 → 3.34 |
| rec2020 | 2.92 → 4.26 | 2.55 → 3.16 |

The hot-path improvement therefore has a cold-setup cost, about 0.6–2.5 ms in
this comparison. A separate twelve-sample-per-version experiment packed the
three generated kernels into one module: it reduced Node setup but increased
Bun setup, so that layout was not adopted. Import-time samples vary; these
numbers do not include process startup, network loading or bundled execution.

[The inline-seed report](reports/milestone-4-dualray-inline.json) records every
pass, setup sample, bundle measurement, source hash, compatibility/parity result,
validation log and reproduction script. This production layout is measured
separately from the earlier P3-only scratch experiment; its gains should not be
assumed identical across workloads, runtimes or module layouts.
