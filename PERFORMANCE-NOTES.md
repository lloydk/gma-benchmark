# Performance experiment history

The current findings, mechanism explanations and measurements are together in
[PERFORMANCE.md](PERFORMANCE.md). This hand-maintained file preserves the
experiments behind earlier implementation choices; the renderer does not
rewrite it.

## Sources and historical scope

The predecessor document is available with `git show ee5b488:PERFORMANCE.md`.
It described Ryzen 9800X3D, WSL2/Linux 6.18, glibc 2.43, Node 26.3.1, Bun
1.3.14 and Rust 1.96.0, with native CPU tuning and LTO. Its mapping experiments
used P3, C=0.4 and f64. The one-off probes were not retained, and its full
mapper timings predated the three-channel output-consumption correction.
Consequently, the old timings below record experiment history rather than a
current performance baseline. Fresh timings and operation counts are linked
from the main article.

The 2026-10-02 review additionally supplied operation-throughput probes on
Node 26.10.0/Bun 1.4.2 and an affinity comparison. Their numbers are attributed
to that review, whose raw probes are not in the timing artifact.

## Engine behavior and the cube-expression change

**Node 26.3.1 / Bun 1.3.14, historical; behavior rechecked in the review on
Node 26.10.0 / Bun 1.4.2:** V8's `x ** 3` path was reported to use a general
power operation, while JavaScriptCore reduced the cube to multiplies. Both
engines optimized `x ** 2`. The review measured `x ** 3` at 7.58 ns in Node
versus 0.55 ns in Bun, with explicit `x * x * x` at 0.43/0.38 ns and `x ** 2`
at 0.39/0.38 ns. These are loop-throughput observations, not instruction
latencies or costs to add mechanically to the full mapper.

Replacing `** 3` with multiplication in the conversion and cubic solvers
historically improved the affected Node methods by 11–32%. That accounted for
the old 1.25–1.45× Bun advantage on methods using those expressions; Bottosson
and Raytrace already multiplied explicitly and served as controls. Keep this
history separate from the current Node/Bun ratios. The surviving implementation
comment is in [rgb-convert.js](src/rgb-convert.js).

The change also affected rounding. Historical probes reported bit-identical
Bun output, but Node differences up to 1.1e-14 for clipping and 7.2e-15 for
Edge Seeker over roughly 685,000 cases. Cubic discriminant decisions could
amplify a last-bit change into an output difference up to 1.4e-7. That history
is a reason to compare outputs and boundary decisions after algebraic changes;
it is not a current accuracy tolerance or proof that every changed root is valid.

**Node 26.10.0 / Bun 1.4.2, review observation:** `Math.cbrt` measured about
4.3 ns in Node and 8.7 ns in Bun. The historical engine investigation attributed
the difference to V8's own implementation versus the glibc routine used by
Bun and native Rust on that Linux host. This helps explain Raytrace's relatively
small JS/native gap and Node's advantage over Bun in that method. It does not
imply Raytrace is the only method where Node can win: the fresh report also
contains CSS MINDE and other implementations absent from the old comparison.

**Rust 1.96.0 / glibc 2.43, historical only:** `sin_cos()` measured no faster
than separate sine and cosine calls (about 10.66 versus 10.56 ns for the pair).
That observation is specific to the compiler, library, inputs and call shape;
it has not been remeasured on Rust 1.98.1 for this refresh.

**Node 26.3.1, historical dependency probe:** substituting `sqrt` for `cbrt`
inside Raytrace saved about 53 ns where operation-throughput multiplication
predicted about 24 ns. The substituted mapper was numerically wrong and was
only a diagnostic. Within a correction, the three LMS cube roots can be
independent; successive corrections depend on the preceding result. Such
dependencies, call overhead and instruction overlap explain why a sum of
isolated operation costs is not a measured time breakdown.

### Recorded Node before/after cube-expression experiment

These are the historical random-workload values from the predecessor document.
Bottosson and Raytrace already multiplied explicitly and were controls.

| Method | Before, ns/color | After, ns/color | Reported change |
| --- | ---: | ---: | ---: |
| clip | 85.4 | 58.1 | −32% |
| cubic cached | 129.9 | 116.0 | −11% |
| cubic no cache | 349.1 | 267.7 | −23% |
| Edge Seeker | 186.6 | 157.9 | −15% |
| Edge Seeker indexed | 110.9 | 84.5 | −24% |
| Bottosson | 125.3 | 127.7 | approximately unchanged |
| Raytrace | 218.1 | 221.1 | approximately unchanged |

## Solver and cusp counter history

The following counts preserve the historical P3 random-workload instrumentation.
They are averages per call after warming caches, not counts for every input or
for the current f32, mixed-input or multi-gamut runs. A trig pair means one sine
and one cosine; the separate cosine column is additional root-solving work.

| Historical method | cbrt | sqrt | Trig pairs | Extra cos | acos | Nonlinear output channels¹ |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| clip | 0 | 0 | 1 | 0 | 0 | 1.78 |
| cubic cached | 0.82 | 0.46 | 0 | 0.17 | 0.06 | 1.99 |
| cubic no cache | 5.30 | 5.17 | 1 | 2.45 | 0.82 | 1.99 |
| Halley | 0 | 0 | 1 | 0 | 0 | 1.99 |
| Ostrowski | 0 | 0 | 1 | 0 | 0 | 1.99 |
| Bottosson | 1 | 0 | 1 | 0 | 0 | 1.99 |
| Bottosson cached | 0 | 0 | 0 | 0 | 0 | 1.99 |
| Edge Seeker, either lookup | 0 | 1.01 | 1 | 0 | 0 | 1.99 |
| Raytrace | 9 | 3 | 1 | 0 | 0 | 1.99 |

¹ The historical gamma estimate was inferred from output values crossing the
transfer threshold. It must not be treated as a literal count of present-day
power calls: clamped channels and exactly emitted faces can skip encoding, and
Fast's polynomial encoder removes power calls on mapped output. P3/sRGB's
linear transfer branch also does not describe Rec.2020's transfer here.

| Historical P3 workload | Solver | Outer iterations | Constraint evaluations |
| --- | --- | ---: | ---: |
| grid | Halley | 3.076 | 3.076 |
| grid | Ostrowski | 2.350 | 4.205 |
| random | Halley | 3.076 | 3.076 |
| random | Ostrowski | 2.356 | 4.217 |

Ostrowski used about 23% fewer iterations but 37% more constraint evaluations.
Historical Node 26.3.1 hardware counters showed about 7% fewer instructions,
with IPC falling from 3.47 to 3.27 on the grid and 2.92 to 2.68 on random
inputs; total cycles stayed within about 1%. The extra dependent evaluation
is the useful explanation. These counters have not been recollected for the
current guarded, multi-gamut implementations.

The historical cusp-side probe used a Bottosson-derived cusp, unlike today's
independent boundary reference. Cached cubic's recorded below/above counts
were cbrt 0.38/1.93, sqrt 0.19/1.26, acos 0/0.30 and extra cosine 0/0.90.
It reported about 1.26 upper root solves per above-cusp color. Those counts
have changed with root conditioning and guards; the current article uses new
counter data.

The earlier Edge Seeker arc used four square roots and one absolute value.
The rationalized quadratic in the current implementation uses two square
roots. The old gamma estimate rose from 1.65 to 3.00 nonlinear output channels
across the cusp; it was inferred from output thresholds, so it cannot account
for today's directly emitted faces or polynomial encoding.

Old Rust clip timings slightly favored the upper side, attributed to more
predictable gamma branches. That direction did not persist in the refreshed
f64 tables. It is an example of why the mechanism should be preserved without
carrying a historical ranking forward.

## Cache-layout and conversion experiments

**Node 26.3.1 / Bun 1.3.14, historical cache experiment:** one object per cubic
hue bucket, with several small arrays, consumed roughly 1.7 MiB in V8 and
1.0 MiB in JavaScriptCore for 366 KiB of numeric payload. That was about
2.5–4× the payload and roughly six heap objects touched per lookup. Flattening
the same doubles into one array preserved output bits and gave a reported
1.16× speedup on random hues. The rationale remains useful even though the
exact allocation overhead and timing belong to those old engines.

The historical notes estimated that appending a naive generic conversion to
cached cubic would add roughly 20 ns, then 35–50% of the Rust total. This was
an operation-cost estimate, not a retained controlled before/after experiment,
and the denominator came from the now-unverified timing tables. The design
lesson survives: avoid recomputing trig, LMS cubes and a matrix multiply when
the solver already has the output. Measure any proposed modular replacement
end to end rather than carrying that percentage forward.

### Smaller-cache prototype

The historical seven-scalar cubic-cache prototype stored hue slopes, the lower
root and three turning points, rebuilding channel coefficients on lookup.
It reported bit-identical f64 output, about 197 KiB instead of 366 KiB, and a
3–8% slowdown. This remains evidence of the memory/compute trade-off explored
then. The current implementation keeps 13 scalars per bucket.

## Instrumentation and layout history

The original probes wrapped Math functions with counting delegates, warmed
caches before counting, and used temporary solver copies to count iterations,
constraint evaluations, derivatives and bisections. Gamma counts came from
output thresholds. Substitution probes and hardware counters tested proposed
explanations. The old cost breakdown multiplied counts by isolated throughput;
that arithmetic did not account for overlap, call overhead or dependencies,
so the current report does not use it as an attribution of CPU time.

The replacement [Math-call profiler](scripts/profile-performance-math.mjs) is
retained in the repository, delegates to the original functions, checks exact
output checksums, and runs separately from timing. Iteration/hardware counters
from the older experiments have not been recollected.

**Rust 1.96.0/native/LTO, historical layout observation:** adding an unrelated
method moved cached-cubic timing by about 15%. Binary layout, inlining and
instruction-cache effects can alter a comparison without a source change to
the method. Keep the binary hash and compare repeated complete builds when
investigating small native-code changes.

## Affinity experiment

**Bun 1.4.2, 2026-10-02 review affinity experiment:** P3 random Fast with
`taskset -c 2` alternated between about 51–52 and 60–62 ns/color; allowing
`2,3` or running unpinned produced 51–52 ns in the reported reruns. Plain
Dualray's single-CPU reruns were about 75–78 ns. Both JS input paths showed
the Fast behavior. Competition with background compiler threads is a plausible
explanation, not a demonstrated JIT trace. A fixed number of warmup passes
does not by itself prove identical optimization state across processes.

The revised runner defaults to CPU set `2,3`, accepts other comma-separated
sets, records affinity and uses it consistently across methods and runtimes.
On this machine 2 and 3 are SMT siblings. This permits concurrent worker
progress but still shares a physical core; it is not exclusive CPU isolation.
Process ranges remain part of the report. If multimodality persists, inspect
tiering and convergence rather than selecting only the fastest mode.
