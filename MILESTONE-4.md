# Milestone four: JavaScript target factories

Step one adds sRGB and Rec.2020 conversions, Clip and CSS MINDE.
[Step two](#step-two-matrix-solvers) adds the six matrix solvers. All thirteen
existing P3 methods retain their exports. Fitted/table-based JavaScript ports
remain the next step. The step-one measurements below are historical.

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

## Harness and verification

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

The benchmark now runs eight methods for sRGB/Rec.2020 and thirteen for P3,
with the same ordering, inputs, precheck flag and separate-process timing.
P3-only fitted/table methods are still dynamically imported only for P3.
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

## Next step

Emit JavaScript Bottosson, Edge Seeker and Dualray data from the existing
Rust generators, then port and validate each mapping policy separately.
