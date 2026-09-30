# Milestone three: target-specific Rust algorithms

Milestone two is committed as `9fcb2d3`. This milestone finishes the remaining
Rust ports before JavaScript gains multi-gamut support in milestone four.
Keep conversion definitions, algorithm data and numerical policy separate;
keep compile-time target selection and independent native f32/f64 kernels.

- [x] Edge Seeker and indexed Edge Seeker for sRGB, Display-P3 and Rec.2020.
- [x] Bottosson and cached Bottosson: derive target sector boundaries and cusp
  fits; validate the fits and both precision lanes against geometric references.
- [ ] Dualray: derive target seeds and audit channel bounds, fold policy and
  recovery against independent references.
- [ ] Final all-13-method comparison across every target, precision and mode.

## Edge Seeker implementation

`edge_seeker.rs` contains one generic implementation of each lookup variant.
`EdgeSeekerData` owns the target's table and any narrow-interval conditioning
metadata. It extends `RgbGamut` without adding algorithm data to that physical
space contract. Both methods are in the common registry; sRGB and Rec.2020 now
ran 10 methods at this checkpoint (now 12 with Bottosson). The original indexed
lookup allocated 3,600 `usize` entries outside timing. Following review, that
index is shared compile-time data per gamut/precision; instances are zero-sized.
On this 64-bit host the shared index is 28,800 bytes, with no per-instance allocation.

Generate or check tables from the repository root:

```sh
node scripts/generate-edge-seeker.mjs
node scripts/generate-edge-seeker.mjs --check
```

The script exports the production Rust matrices and encoding checks through
`rust/examples/rgb-gamut-profiles.rs`, then reuses `makeLut` with 400 slices.
It checks the JS Oklab conversion against Rust samples, validates table closure,
finite values, curvature and strictly increasing hue in both precisions.
The generated tables are checked in; ordinary Rust builds do not invoke Node.
Exact regeneration was checked with Node 26.10.0 on Linux; transcendental math
can produce last-bit differences on other platforms or runtime versions.

| Target | Rows | f32 table bytes | f64 table bytes | Sharp intervals |
| --- | ---: | ---: | ---: | ---: |
| sRGB | 695 | 11,120 | 22,240 | 1 |
| Display-P3 | 710 | 11,360 | 22,720 | 0 |
| Rec.2020 | 790 | 12,640 | 25,280 | 1 |

These sizes exclude tiny conditioning metadata and the optional interval index.
All 2,840 P3 values reproduce the incumbent table exactly. A before/after
snapshot preserved all 1,095,680 sampled P3 mappings: 136,960 inputs, two lookup
variants, two modes and two precisions. Inputs include both benchmark workloads,
mixed and low chroma, wrapped/extreme hues and lightness endpoints.

The repaired blue folds in sRGB and Rec.2020 have a large chroma change across
0.0001 degrees. Rounded f32 knots alone distort interpolation over that span.
The first face-neighbour test exposed about 0.00397 DeltaEOK difference at
sRGB `[0.17938176, 0.12429921, -95.94798]`. Generated rounding residuals retain
the lost hue bits around these intervals. The native f32 lookup uses those
residuals and recovers low bits lost when normalizing negative hue; there is no
f64 runtime fallback. The f64 path and the P3 table need no such correction.

The plain mapper retains its existing table/arc policy, including negative-C
conversion and endpoint handling. The checked mapper preserves every accepted
canonical conversion bit-for-bit, using the authored hue. Different f32/f64
canonical classifications are reported separately, as for the matrix solvers.

## Numerical validation

An independent lookup scans original f64 knots and bisects the circle residual.
An independent XYZ conversion then checks the mapped result. Tests cover table
knots and adjacent representable inputs, wrapping, all cube faces and their
neighbours, both benchmark workloads, gray and negative chroma, endpoints and
every f32 hue in a band around each sharp interval. Both lookup variants must
produce identical bits in both modes.

The table-policy tests cover 346,975 inputs for sRGB, 349,705 for P3 and 382,600
for Rec.2020, per precision. The maximum native f32 chroma error was `1.20e-6`,
linear RGB error `5.74e-6` and DeltaEOK `1.22e-6`; f64 maxima were below `9e-15`.
The independent policy limits are `2e-6` chroma, `1e-5` linear RGB and `3e-6`
DeltaEOK for f32. The f32/f64 workload validator uses the same linear and
perceptual limits; encoded differences remain visible, including Rec.2020's
near-zero gamma amplification. An additional 576,016 near-cusp/near-white samples
per target and precision check the arc against circle-residual bisection.

Approximation quality is measured separately against the stationary-interval
geometric first-exit oracle. These differences describe Edge Seeker's table/arc
mapping policy, not arithmetic error. In particular, its blue-fold table can
retain a vivid outer branch, and its top arc can miss the true boundary even
without a fold. Clipping can then move lightness and hue.

| Target | Quality samples per precision | Max first-exit DeltaEOK | Max clipping DeltaEOK |
| --- | ---: | ---: | ---: |
| sRGB | 196,789 | 0.04740 | 0.003224 |
| Display-P3 | 98,260 | 0.02344 | 0.003778 |
| Rec.2020 | 200,399 | 0.05137 | 0.004605 |

The largest sRGB/Rec.2020 first-exit differences occur near their blue folds.
P3's largest sampled difference occurs near yellow at `L=0.98339, h=109.19027`.
These inherited approximation differences are meaningful; the port does not
claim exact boundary solving or a universal perceptual bound. The tests keep
explicit per-target regression envelopes for this corpus. Improving the
approximation would be a separate algorithm change.

The [Edge Seeker report](reports/multi-gamut-milestone-3-edge-seeker.json)
records source hashes, environment, measurements, compatibility and timing.

## Performance checkpoint

P3 was compared with the committed milestone-two binary on Ryzen 7 9800X3D /
WSL2, rustc 1.98.1, native release/LTO. Each mode used two processes per build
in ABBA order, pinned to CPU 2, with 50 warmup and 25 measured passes over each
35,640-input workload. All output channels are consumed. No tests, compilation
or competing benchmarks ran during timing.

| P3 plain mode, ns/call | Before | After |
| --- | ---: | ---: |
| f64 grid, binary lookup | 49.09 | 48.65 |
| f64 grid, indexed lookup | 42.05 | 42.00 |
| f64 random, binary lookup | 90.77 | 90.65 |
| f64 random, indexed lookup | 57.14 | 57.08 |
| f32 grid, binary lookup | 34.51 | 34.35 |
| f32 grid, indexed lookup | 31.53 | 31.62 |
| f32 random, binary lookup | 80.75 | 81.55 |
| f32 random, indexed lookup | 45.79 | 46.02 |

Edge Seeker changes range from -0.9% to +1.0% in plain mode and -1.8% to +3.5%
with prechecking. These include code-layout and run-to-run effects; this small
sample is not an isolated estimate of generic-dispatch cost. Separate single
process runs cover sRGB and Rec.2020 in both modes, without claiming a balanced
cross-target ranking.

The initial Edge Seeker timing checkpoint raised a performance follow-up:
unchanged **P3 f64 cached cubic with prechecking** increased from 67.49 to
72.82 ns/call on the grid (+7.9%) and 91.13 to 94.51 on random (+3.7%). Its source
kernel was not changed by this port. Follow-up disassembly found identical
hot-path instructions, constants and branches, with different function placement.
Repeated runs did not reproduce a stable 7.9% penalty; disabling ASLR and using
one executable path brought the grid comparison within about 1%. Forcing 64-byte
function alignment did not produce a reliable improvement, so no alignment flag
was adopted. These measurements include placement and process noise; the final
milestone comparison must still use balanced runs and report that uncertainty.
The original plain-mode P3 harness had no measured regression above 1.9%.

All 81 tests passed in native release, portable x86-64 release and debug;
all four JavaScript test files passed. The final native build has no warnings,
all-target `--validate-only` passes, and the table freshness check passes.
Standalone native-f32 assembly probes for each target contain no double
arithmetic or widening instructions. These checks describe this build and
sampled corpus, not a guarantee for every compiler or input.


## Bottosson implementation

`bottosson.rs` now supplies generic constant-lightness and cached mappers for
all three gamuts. `BottossonData` owns its approximation data separately from
physical RGB definitions. The two mappers share one intersection kernel and
retain a five-term saturation seed followed by one Halley refinement. The
warm cached plain path uses a 0.1-degree hue bucket with five native scalars;
allocation and cache warming stay outside measured passes.

Generate or check the new fits from the repository root:

```sh
node scripts/generate-bottosson.mjs
node scripts/generate-bottosson.mjs --check
```

Both generators share `rgb-gamut-profiles.mjs` and the Rust profile exporter.
The Bottosson generator derives primary hues and sector half-planes from the
exported matrices, fits 4,096 midpoint samples per face, then minimizes the
post-Halley error using damped, reweighted Gauss-Newton steps. Its reference
roots use stationary intervals and bisection. Verification uses 65,536 held-out
hues plus eight one-sided endpoint samples per face. Measured maximum absolute
saturation errors are `0.00038482` for sRGB and `0.00016825` for Rec.2020.
These are sampled fit errors, not full-domain bounds. The P3 coefficients are pinned in the generator; its generated file uses the
same layout as the other two gamuts, with regenerated primary-hue data. Ordinary builds use the
checked-in files and do not run fitting. Exact regeneration was checked with
Node 26.10.0 on this Linux host; platform math may affect final bits.

The port fixes three inherited arithmetic/contract issues:

- Constant-lightness interpolation returns the common lightness directly.
  The former `L*(1-t)+t*L` loses significant bits when tiny input chroma makes
  `t` large. At `[0.43879423, 0.000038166054, 341.1062]`, that caused about
  `6.3e-5` f32/f64 DeltaEOK difference in the original P3 path.
- Near primary hues, half-plane rounding can choose the wrong sector, even
  the third face when both relevant tests round to equality. Narrow contacts
  now compare the authored hue with generated primary thresholds. f32 uses
  compile-time high/low thresholds, including negative wrapped hues; mapping
  remains entirely native f32.
- Cached checked mapping uses the authored hue for its canonical conversion,
  before consulting the rounded-hue cache. Gray, tiny and negative chroma also
  preserve an accepted canonical result. The plain path retains its original
  boundary-projection policy, including increasing positive in-gamut chroma;
  callers needing pass-through should use the checked entry point.

`bottosson_tests.rs` checks primary sectors and selected lower-face roots using
independently derived XYZ matrices and stationary-interval bisection. A separate
one-Halley policy reference expands channel polynomials from XYZ conversion
samples, rather than reusing the production matrix/derivative code. At exact
binary64 primary contacts, independent matrix roundoff may choose either of the
two adjacent sectors; both must meet the ordinary arithmetic limits. Away from
those narrow contacts a single sector is required.

At the initial Bottosson checkpoint, per target and precision, the cusp tests
cover 38,880 hues and the mapping tests
cover 86,192 inputs in both variants and modes: grid/random workloads, all RGB
faces and neighbours, primary hues, wrapped hues, tiny/negative/zero chroma,
lightness endpoints and powers of two approaching black and white. Maximum
native f32 policy errors **on that initial corpus** were `9.34e-6` linear RGB and `3.82e-6` DeltaEOK; f64
maxima were below `2.5e-14`. The regression limits are `1.2e-5` linear RGB and
`5e-6` DeltaEOK for f32, and `2e-11` for both f64 metrics. Workload validation
uses those f32 limits and also reports encoded-channel differences.

Approximation quality remains a separate contract. Tests compare mapped chroma
against the geometric first exit, and measure the lightness/hue displacement
introduced by output clipping. The new fits retain the inherited outer-branch
behavior at blue folds; they do not make Bottosson an exact first-exit mapper.

| Target | Quality samples per precision | Max first-exit DeltaEOK | Max clipping DeltaEOK |
| --- | ---: | ---: | ---: |
| sRGB | 202,779 | 0.04740 | 0.000918 |
| Display-P3 | 103,680 | 0.01194 | 0.0000755 |
| Rec.2020 | 202,779 | 0.05137 | 0.001664 |

The pinned P3 fit also has a larger sampled saturation error near blue:
`0.02797`, compared with `0.000387` sRGB and `0.000171` Rec.2020 in the native
cusp tests. Its two-coordinate cusp error reaches `0.00960`. These are measured
limitations of the incumbent approximation, distinct from porting roundoff.
Refitting P3 or changing the projection policy would be a separate change.

The [Bottosson report](reports/multi-gamut-milestone-3-bottosson.json) records
source/binary hashes, accuracy, compatibility and timing. The Edge Seeker report
above remains the historical checkpoint before this family was ported.

### Bottosson compatibility and timing checkpoint

A before/after sweep covered 136,960 inputs, both modes and both precisions.
All 6,026,240 mappings from the other eleven P3 methods remained bit-identical.
Bottosson output changes are intentional: constant-lightness cancellation and
sector fixes, plus authored-hue canonical prechecking. The largest plain f32
change was `0.24982` in an encoded channel at
`[0.9703982, 2.546478e-7, 116.87784]`: the old cancellation-prone projection
stayed near gray, while the corrected projection reaches the intended boundary.
The checked path preserves that in-gamut input. The largest cached checked
change was `0.003209`, reflecting the corrected authored-hue precheck.
The report contains counts and maxima for each method, precision and mode.

The baseline here is the completed Edge Seeker checkpoint, before this Bottosson
port. P3 used two processes per build per mode in ABBA order, pinned to CPU 2,
with ASLR disabled and the same executable pathname. Compiler flags, workloads,
method order and output consumption were unchanged. No compilation, tests or
competing benchmarks ran during timing. Values below are the median of the two
process medians, on the same Ryzen 7 9800X3D / WSL2 native release build described
above; they include code-layout effects and the correctness fixes.

| P3 plain mode, ns/call | Before | After | Change |
| --- | ---: | ---: | ---: |
| f64 grid, direct | 88.19 | 88.68 | +0.6% |
| f64 random, direct | 99.92 | 101.55 | +1.6% |
| f32 grid, direct | 69.74 | 74.33 | +6.6% |
| f32 random, direct | 80.69 | 84.20 | +4.4% |
| f64 grid, cached | 31.63 | 32.12 | +1.6% |
| f64 random, cached | 45.97 | 45.94 | -0.1% |
| f32 grid, cached | 24.66 | 25.09 | +1.7% |
| f32 random, cached | 38.27 | 38.30 | +0.1% |

Cached checked mapping now pays for the authored-hue conversion. It increased
from 32.86 to 45.05 ns on the f64 grid (+37.1%), 53.16 to 62.13 on f64 random
(+16.9%), 26.59 to 35.48 on the f32 grid (+33.5%), and 43.77 to 54.88 on f32
random (+25.4%). This is a change to the canonical precheck contract, not an
isolated measurement of abstraction overhead. The full report includes direct
checked timings and every other method. No unchanged P3 method regressed by
more than 3% in this comparison. Single-process timings for sRGB and Rec.2020
cover both precisions and modes; they do not establish a balanced cross-target
ranking. The final all-method milestone comparison remains pending.

At that initial Bottosson checkpoint, all 87 Rust tests passed in native release,
portable x86-64 release and debug;
all four JavaScript test files pass. Both generators reproduce their checked-in
data, all-target `--validate-only` passes and the native build has no warnings.
Assembly probes for all twelve f32 target/method/mode combinations contain no
double arithmetic or widening. Dualray is the remaining Rust algorithm family.

## Review follow-up

The review's sRGB Bottosson reproducer was confirmed. At the identical
f32-rounded input `[0.49, 0.4, 264.04913]`, the plain f32/f64 linear RGB difference
was `1.53342e-5`, exceeding the unchanged `1.2e-5` limit. It is now `9.30213e-7`.
The earlier `9.34e-6` maximum described the original fixed corpus; it was never
a full-domain bound, and that corpus missed this narrow window.

The failure combines degree-to-radian rounding at a large f32 angle with
cancellation in the channel residual used by the saturation refinement. Within
one degree of each target's blue primary, native f32 now rotates a small angle
around 270 degrees and compensates the cube/product/sum residual using fused
operations. The coefficients and single Halley refinement are retained. The
canonical conversion and the f64 saturation calculation keep their prior paths.
No mapper arithmetic widens to f64.

The new test visits every positive f32 hue within 0.1 degrees of each primary,
plus 3,001 shifted hues over a 2.2-degree window that crosses the conditioning
cutovers. Each is exercised with -360/0/+360 wrapping, ten lightnesses, both
variants and both modes. Counts below include repeated wrapped mappings. The
reference remains the independent XYZ/polynomial policy evaluator; geometric
approximation error is measured separately by the existing tests.

| Target | Mappings per precision | Max f32 linear RGB error | Max f32 DeltaEOK |
| --- | ---: | ---: | ---: |
| sRGB | 1,146,600 | 4.649e-6 | 1.028e-6 |
| Display-P3 | 1,146,600 | 4.296e-6 | 9.282e-7 |
| Rec.2020 | 1,933,080 | 3.628e-6 | 8.923e-7 |

The `1.2e-5` linear and `5e-6` perceptual f32 limits remain unchanged. The exact
reported input also passes the all-method startup validator. These sweeps are
stronger regression coverage, not a proof over all colors or platforms.

Other review changes:

- CLI help, both READMEs and stale source comments now describe twelve methods
  for sRGB/Rec.2020 and Dualray alone remaining P3-only.
- The printed P3 plain/checked “equivalence” statistic is removed. Generic
  validation asserts canonical bit preservation for accepted prechecks and
  exact agreement with plain mapping for rejected prechecks. A test injects
  matching f32/f64 mode errors to prove that the latter check can fail.
- The NaN finding was stale: finite-output and finite-metric assertions already
  precede maximum accumulation. Injected NaN/Infinity tests continue to pass.
- Upper/lower face identity is explicit in `first_face_root`; a zero lower
  bound no longer chooses the face implicitly. Direction tests cover both.
- Conversion pass-through, clipping, hue-bucket calculation and Edge Seeker's
  mapping policy share helpers. Constant-lightness blending uses
  `L0 + t*(L1-L0)`, which preserves equal endpoints without a special branch.
- All Bottosson targets use the same generated data layout. The generator pins
  the incumbent P3 coefficients rather than refitting them.
- Edge Seeker residuals subtract the actual compiled table knot, eliminating
  the independent literal-parsing assumption. Tests verify those residuals.
  Conditioning bands and interval indices are compile-time data. The indexed
  mapper no longer allocates or constructs a 28,800-byte index per instance.

The fold-repair knots are deliberately retained. Moving them to other f32
coordinates changes the interpolation policy and the boundary geometry; it is
not needed to fix the residual-generation issue. A different knot-placement
policy needs its own approximation comparison.

The earlier near-white/tiny-root, geometric fold, gray Raytrace, dark-error gate
and all-gamut cross-method regression tests pass. In particular, fold tests
include sRGB `L=0.414, h=264.0425`. Linear-RGB gates remain paired with DeltaEOK
gates; the old encoded-channel bounds are not asserted for every solver.

All 95 Rust tests pass in native release, portable x86-64 release and debug;
all four JavaScript test files pass. Both generators reproduce their data and
the native build is warning-free. The P3 compatibility sweep preserves every
sampled f64 output and all outputs from the other eleven methods in both lanes.
Only the two f32 Bottosson variants change sampled bits, by at most `1.789e-6`
in an encoded channel. Native assembly probes, including outlined local
callees, contain no double arithmetic or widening.

The [review-fix report](reports/milestone-3-review-fixes.json) records the
reproducer, dense sweep, source/binary hashes, compatibility and updated timing.
The two earlier milestone-three reports remain historical checkpoints.

### Review-fix timing

This comparison rebuilds the frozen Bottosson checkpoint and the corrected
source with identical native release flags. The initial comparison against an
existing executable was discarded because its hash differed from the recorded
native build. The rebuilt baseline matches that recorded hash.

The matching builds use the same P3 workloads and output-consuming harness,
two processes per build per mode in ABBA order, CPU 2, ASLR disabled and the
same executable pathname. Each process uses 50 warmup and 25 measured passes.
No builds or tests ran during timing. The machine remains the Ryzen 7 9800X3D
/ WSL2 system above. These medians include code-layout effects and all review
edits; they do not isolate the cost of the numerical conditioning.

| P3 Bottosson, ns/call | Plain before | Plain after | Checked before | Checked after |
| --- | ---: | ---: | ---: | ---: |
| f64 grid, direct | 90.11 | 88.73 | 89.90 | 92.22 |
| f64 random, direct | 101.65 | 101.65 | 109.64 | 104.05 |
| f32 grid, direct | 74.56 | 71.77 | 74.01 | 70.79 |
| f32 random, direct | 84.98 | 83.31 | 91.81 | 92.31 |
| f64 grid, cached | 32.20 | 31.23 | 44.85 | 43.66 |
| f64 random, cached | 46.77 | 45.53 | 62.28 | 61.23 |
| f32 grid, cached | 25.33 | 24.16 | 35.48 | 35.31 |
| f32 random, cached | 38.73 | 40.06 | 55.01 | 54.75 |

Bottosson changes range from -5.1% to +3.4% across these rows. Indexed Edge
Seeker improves by 0.3–6.6%, while binary-search Edge Seeker ranges from -4.3%
to +4.2%. The largest other slowdown is cached cubic on the plain f64 grid,
50.52 to 52.61 ns (+4.1%). The report includes all 104 P3 comparison rows and
single-process timings for both new targets and modes. Those target runs are
not a balanced cross-target comparison. Portable correctness was tested;
portable timing was not measured.
