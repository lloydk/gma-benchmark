# Performance analysis

The largest speed differences here come from avoiding work: fitting a lower face instead of solving it, caching hue structure, reusing RGB values, or returning before a perceptual search. Which shortcut applies depends on the input as much as the method.

Measured 2026-10-02 on AMD Ryzen 7 9800X3D 8-Core Processor, using Node v26.10.0, Bun 1.4.2 and rustc 1.98.1 (48a229cea 2026-09-01). Coverage: **14 methods in Node, 14 methods in Bun, 15 methods in Rust f64, 15 methods in Rust f32**, across 3 gamuts. All times are **nanoseconds per color**, including encoded RGB output.

## Key findings

### 1. Dualray Fast earns its lead below the cusp

On shuffled P3 input, Fast cuts Dualray's time by **27.0%–37.8% across these runtimes**. Its shortcut
fits the lower boundary and the two nonzero RGB channels directly from hue,
then scales them by lightness cubed. That replaces trigonometry, channel-cubic
construction and root refinement with a small polynomial evaluation.

The difference is visible when the workload is split: Node Fast takes
**35.4 ns below the cusp and 100.4 ns above it**;
Rust f64 takes **25.2 and 78.8 ns**. The fresh Node
counts show no sine or cosine calls on the entire below-cusp workload, versus
1.22 of each per color above it. The branch probe explains the
extra calls: every above-cusp color enters the upper solve, none enters exact
recovery, and **21.7% first run the canonical precheck**.
Those colors fall below the fitted lower-boundary ratio plus its margin, so
the mapper converts them to test membership before the upper solve computes
the same hue trig again. The [cusp-side counts](#cusp-side-operation-and-path-counts)
show both the operations and their source paths.

| Runtime | Random: time saved by Fast | Above cusp: Fast vs Dualray |
| --- | ---: | ---: |
| Node | 27.0% | 7.3% more time |
| Bun | 32.3% | 2.5% more time |
| Rust f64 | 37.3% | 4.1% less time |
| Rust f32 | 37.8% | 7.7% less time |

This matters because 74.7% of the standard P3 random corpus lies
below the cusp. A bright-skewed workload loses much of the shortcut's benefit.
Fast's approximation buys a particularly cheap lower face, rather than a
uniform reduction in the cost of every Dualray path.
([Implementation](src/dualray-fast.js))

### 2. Caches win by removing whole phases of work

On P3 random inputs, caching speeds cubic up by 2.74–3.73× across runtimes, and
Bottosson by 1.74–2.23× across runtimes. Cubic stores the hue-only coefficients,
lower root and turning points; Bottosson stores its cusp and LMS direction
slopes. After warmup, both skip hue trig, and cached Bottosson also skips cusp
construction and its cube root. The current counters put uncached cubic at
**5.04 cbrt calls/color**, versus **0.56** cached.

Repeated integer hues can favor caches enough to change the ranking. The lowest mapped
P3 grid medians are bottosson-lightness (cached) in Node (44.0 ns) and dualray fast in Bun (42.6 ns). The price is persistent state—366/183 KiB
for cubic f64/f32 and 141/70 KiB for Bottosson—and 0.1° hue buckets. Cubic's two
rows use the same bucket semantics; cached Bottosson changes fractional-hue
evaluation compared with its uncached counterpart. Choose caching when that
memory and numerical trade-off fit the application.
([Cubic](src/oklch-cubic.js), [Bottosson](src/bottosson-factory.js))

### 3. A mapper can beat clipping by doing less conversion

Below the cusp, Rust f64 Fast takes **25.2 ns** against clip's
**40.3 ns**. Clipping must first convert the original OKLCh color:
trig, LMS cubes, a matrix multiply and output encoding. Fast already knows
the mapped face's two nonzero linear channels. Cached cubic likewise emits
RGB from the polynomials it solved. The solver's intermediate values are also
its conversion result.

That reuse is part of the algorithm's practical cost. Direct cubic, Halley
and Ostrowski retain LMS hue slopes but still cube and multiply after solving;
Raytrace already holds the RGB hit point; Edge Seeker obtains only a chroma
limit and therefore needs a full final conversion. A port that gives every
method a separate generic conversion would change these rankings. Preserve
the reusable values when making the implementation more modular.

### 4. CSS MINDE magnifies the cube-root cost

CSS MINDE takes **476.7 ns in Node**, **624.0 in Bun** and
**567.9 in Rust f64** on P3 random input. Each clipping-error comparison
converts clipped RGB back to Oklab using three cube roots. The current Node
counter run records **32.8 cbrt calls/color**, or **10.9
error comparisons**, on average.

The size of the Node/Bun gap is consistent with the cube-root difference.
The Node 26.10.0/Bun 1.4.2 operation probes supplied with the review measured
about 4.3 versus 8.7 ns for cbrt throughput; earlier investigation identified
V8's own implementation versus glibc on this host. Multiplying the current
**32.8 calls by that 4.4 ns difference gives about 144.4
ns**, close to the measured **147.3 ns** gap. This is a consistency
check on the engine comparison, rather than a profile assigning parts of the
mapper's total time. CSS MINDE is the strongest current case for investigating
a faster native cube root.

Raytrace is different in these measurements. Its **9.0** cube
roots would suggest about **39.6 ns** by the same arithmetic,
but the Node/Bun gap is only **9.5 ns**; it takes
**235.2 ns in Node, 244.6 ns in Bun, 220.8 ns in Rust f64**. Node is slower than Rust f64 here. The isolated operation
probe does not predict the full Raytrace gap; its dependency chains, argument
distribution and surrounding work need separate profiling. The old cube-root
story is not enough to explain today's Raytrace timings.
([CSS MINDE](src/css-minde.js), [Raytrace](src/raytrace.js))

### 5. Polynomial encoding buys more in f64

Replacing Fast's ordinary output power with the polynomial reduces its Rust
P3 random time from **41.8 to 33.4 ns in f64**, saving
**8.4 ns**. In f32 it moves from **32.7 to 30.3
ns**, saving **2.4 ns**. The boundary mapper is the same in each
pair, so this directly measures the benefit of the encoder variant.

The polynomial uses the same degree in both precisions. It does not spend
extra terms recovering binary64 accuracy, while the ordinary encoder uses
each lane's native power operation. The interior workload gives a second
piece of evidence: across the other canonical-checking methods, median time
is **29.8 ns in f32 versus 48.0 in f64**.
Those paths return before solving a boundary, directly showing the cheaper
f32 conversion-and-encoding path. Together, these results support a smaller
cost to replace in f32, although they do not isolate the power call itself.

Mostly interior traffic also removes the polynomial encoder's opportunity:
both Fast variants deliberately keep ordinary encoding for canonical in-gamut
output. Use the mapped fraction, precision and accepted error budget to decide
whether the extra approximation pays.
([Rust encoder](rust/src/dualray_fast.rs))

### 6. Edge Seeker's index addresses unpredictable lookup work

In Rust f32, plain Edge Seeker goes from **33.2 ns on the grid to
80.8 ns on random hues**, a **2.44×** increase. The indexed
version moves from **30.8 to 45.3 ns**, or
**1.47×**. Both variants evaluate the same interpolated boundary at
the exact input hue; the index changes how they find the interval.

Binary search follows a sequence of dependent comparisons. Repeated integer
hues make those decisions easier to predict; shuffled fractional hues disrupt
the pattern. A dense index jumps near the required interval and needs only
local correction. The remaining random/grid penalty includes conversion and
lightness-dependent branches. Above the cusp, the boundary itself changes
from a straight line to a quadratic arc: the current counts confirm **zero
versus two square roots** per call. Indexing is useful even without changing
the boundary approximation; its extra payload is 7 KiB in JS and 28 KiB with
the present Rust usize representation.
([Lookup and arc](src/edge-seeker/makeEdgeSeeker.js))

### 7. Higher-order convergence does not guarantee fewer cycles

On P3 random input, Halley and Ostrowski take **119.8/119.5
ns in Node** and **112.6/115.2 ns in Rust f64**. Halley
evaluates the constraints and derivatives together. Ostrowski takes a Newton
step and usually evaluates the constraints again before its higher-order
correction; that second evaluation depends on the first result.

The old instrumented comparison found fewer outer iterations but more total
constraint evaluations for Ostrowski, explaining why its convergence order
did not translate directly into a throughput win. The current close timings
fit that trade-off. Optimize the amount and dependency of work per color,
rather than choosing an iteration solely for its mathematical order.
([Halley](src/oklch-halley.js), [Ostrowski](src/oklch-ostrowski.js);
[earlier counters](PERFORMANCE-NOTES.md#solver-and-cusp-counter-history))

### 8. Uncached cubic is a real exception to the f32 advantage

Rust f32 has a lower P3 random median for **14/15 methods**,
but uncached cubic takes **232.8 ns versus 223.9 in f64**.
There is a concrete difference in the work: `first_root` sends every f32
Cardano candidate through `checked_cardano`; f64 returns the candidate directly.
The check finds the first crossing interval, validates and polishes the root,
and can recover by bisection.

Uncached cubic rebuilds three lower roots and three turning points per call,
in addition to its lightness-dependent work. The f32 safeguards recur in that
setup, whereas the cached version pays them when populating a hue bucket.
This gives a specific reason smaller arithmetic need not make this mapper
faster. The guards protect root selection, so removing them is not a free
precision optimization; profiling their frequency and cost is the next step.
([Precision-specific conditioning](rust/src/conditioning.rs))

### 9. In-gamut traffic changes which work matters

On the all-interior P3 workload, Node CSS MINDE falls to **62.5
ns**, from **476.4 ns** on identical-lightness/hue out-of-gamut
input. Its cube-root count falls to **0.0**: the canonical
conversion returns before the clipping-error search. Node Fast instead takes
**67.6 ns inside versus 59.4 ns outside**,
because its cheap mapped lower path can skip the conversion that pass-through
must preserve.

Most canonical-checking methods consequently converge toward conversion cost
on interior input. For example, checked Halley takes **48.1
ns in f64 and 29.5 ns in f32** there. Dualray retains its
first-exit policy and still does boundary work, taking **68.1
ns in f64**. For mostly in-gamut applications, the pass-through path and the
cost of a failed precheck deserve as much attention as the boundary solver.
The 50% mixture in the appendix shows the transition between those workloads.

## Comparing runtimes

Node and Bun are near parity in the median across shared methods, but that
overall similarity hides useful differences. Bun's cached cubic takes **79.9 ns
against Node's 95.2**, and Bun Fast takes **51.9 against
57.3 ns**. Node wins CSS MINDE, where its cube-root implementation
has much more work to influence. The median JS/Rust f64 ratios are around
1.2× on this workload; the individual method and math-library path matter more
than a blanket language multiplier.

| P3 random ratio | Median across shared methods | Range across methods |
| --- | ---: | ---: |
| Node / Bun | 1.01× | 0.76–1.19× |
| Node / Rust f64 | 1.17× | 0.84–1.73× |
| Bun / Rust f64 | 1.15× | 1.00–1.62× |

Ratios are for P3 random input; a value above one favors the denominator.
The interior medians in Finding 5 cover CSS MINDE and the checked cubic,
Halley, Ostrowski, Bottosson, Edge Seeker and Raytrace variants.

One practical engine difference remains: on Node 26.10.0, V8 still lowers
`x ** 3` to a general power call, while Bun 1.4.2's JavaScriptCore reduces it
to multiplies. Use `x * x * x` for cubes in hot JS color code on these engines.
The [earlier cube-expression experiment](PERFORMANCE-NOTES.md#recorded-node-beforeafter-cube-expression-experiment)
records its impact and the rounding changes to check when making that rewrite.

## Comparing gamuts

**A wider gamut has no uniform speed penalty.** sRGB and Display-P3 are close
in typical cost: the median per-method time ratio stays near one in every
runtime, on both random input and the ordered grid. Rec.2020 changes more,
especially on the grid, but the effect depends on the method and precision.

| Runtime | sRGB / P3, random | Rec.2020 / P3, random | sRGB / P3, grid | Rec.2020 / P3, grid |
| --- | ---: | ---: | ---: | ---: |
| Node | 1.00× | 1.03× | 1.00× | 1.07× |
| Bun | 1.01× | 1.05× | 1.02× | 1.05× |
| Rust f64 | 1.00× | 1.02× | 1.00× | 1.08× |
| Rust f32 | 1.00× | 0.98× | 1.00× | 1.05× |

Each entry is the median of the individual method's target/P3 time ratios,
including clip and every method available in that runtime. Below one means
less time than P3. These are the same input triples in the same order for all
three targets, so differences come from processing them for a different gamut.

**Dualray's performance carries across targets.** On random input, the largest
change from P3 across Dualray and Dualray Fast, all targets and all four runtimes,
is 6.0%. Fast remains the fastest mapped JS method on each
target; Fast with poly encode leads both Rust lanes. Choosing sRGB or Rec.2020
does not erase the lower-face shortcut's advantage on this distribution.

**The iterative solvers and cached cubic are more sensitive.** Bun Halley takes
138.7 ns for sRGB, 112.2 for P3 and
145.5 for Rec.2020 on random input. Halley and Ostrowski's
Rec.2020 penalties in Bun are 28.6%–29.6% there and
38.1%–38.3% on the grid. Cached cubic also costs more
for Rec.2020 in every runtime: 5.5%–11.8% on random input
and 10.0%–26.7% on the grid. P3 alone understates these costs.

The matrices have the same dimensions, but their coefficients change which
channel limits chroma, when cached cubic can skip an upper-face solve, and
when iterative solvers stop. Halley and Ostrowski also have a fold check and
special boundary path for sRGB and Rec.2020 that P3 does not need. These are
concrete places to investigate; the operation profile covers P3, so it cannot
assign the cross-gamut gaps to extra iterations, fold handling or JIT behavior.
([Cached cubic](src/oklch-cubic.js), [Halley](src/oklch-halley.js),
[fold policy](src/matrix-solver-policy.js))

**Output encoding changes too.** sRGB and P3 share a transfer function with a
linear segment near black. This repository's CSS Rec.2020 encoder uses a pure
gamma-2.4 power, with no linear segment. Even clip therefore changes cost:
Rec.2020 takes 5.7%–10.0% more time than P3 on random input in
Node, Bun and Rust f64. The direction reverses in Rust f32, at
35.2 ns for Rec.2020 versus 36.3 for P3. Extra power
evaluations, fewer branches and the runtime's math implementation can pull in
different directions; gamut width alone does not predict the result.
([JS transfer functions](src/rgb-spaces.js), [Rust transfer functions](rust/src/transfer.rs))

The comparison is almost entirely out of gamut: all sRGB/P3 inputs are outside,
while Rec.2020 contains 126 interior colors
(0.4%) on random input and 139
(0.4%) on the grid. The cusp-side and mixed/interior
experiments cover P3 only. Use the [full target tables](#appendix-a-standard-workloads)
for the chosen method; mostly in-gamut applications need their own target-specific
comparison.

## Choosing a method

These are starting points for the measured OKLCh kernels. The required mapping
policy decides which speed comparisons are relevant.

| Input or requirement | Starting point | Reason |
| --- | --- | --- |
| Out-of-gamut, mostly below the cusp | Dualray Fast, if its approximation budget is acceptable | Its fitted lower face avoids trig, solving and a generic conversion. |
| Constant-lightness/hue first-exit boundary semantics | Dualray | Guarded refinement and exact-search recovery avoid replacing the boundary with Fast's fitted lower face. Canonical pass-through and disconnected outer islands remain separate policy questions. |
| Repeated hues, with memory to spare | Cached Bottosson or cached cubic | Reuse removes hue-only setup; compare their approximation and bucket semantics before choosing. |
| Mostly in gamut | Prioritize canonical precheck and pass-through cost | Conversion dominates once the boundary solver is skipped. |
| Out-of-gamut, mostly above the cusp | Compare Dualray and Fast on that distribution | They are close here; Fast's large lower-face advantage does not carry over. Cached Bottosson and indexed Edge Seeker are also candidates when their boundary policies fit. |

## How to read these numbers

- Use the shuffled P3 workload as the main comparison, then check the cusp-side and in-gamut results for your input distribution. The integer-hue grid favors repeated lookup and branch patterns; the mixed/interior sets are controlled diagnostics. Full sRGB, Rec.2020 and diagnostic tables are in the appendices.
- Compare mapping policies as well as speed. Clipping and CSS MINDE can change lightness/hue; cached Bottosson uses 0.1° hue buckets; Fast targets an empirical ΔEOK budget of 1e-3 maximum / 1e-4 p99. Dualray retains first-exit boundary semantics, while canonical-checking paths preserve the authored conversion. [Method contracts](README.md#methods) describe the differences.
- Times are medians of 3 fresh-process medians after warmup, with all three RGB channels consumed. Setup and cold-cache costs are excluded. These are fixed-target OKLCh kernels; public color objects, input-space conversion, alpha and browser rendering would add different work.
- The explanations combine measured timings, untimed operation counts and source inspection. The cube-root library and branch-prediction explanations are supported mechanisms, not isolated shares of CPU time; f32 power latency remains an inference. Historical microbenchmarks and counter experiments are labeled in [PERFORMANCE-NOTES.md](PERFORMANCE-NOTES.md).
- Close rankings can change with JIT state, binary layout and scheduling. CPU affinity applies to worker threads too. The recorded process ranges below make the remaining variation visible; small median differences are not guarantees.

[Raw timing data](reports/performance-2026-10-02.json), [current Math-call counts](reports/performance-2026-10-02-math.json) accompany the report. These tables replace the historical timings that predated complete output consumption.

## Display-P3: the main comparison

Shuffled fractional hue/lightness, C=0.4, all inputs out of gamut. The findings above explain the differences in this table.

| Method | Node | Bun | Rust f64 | Rust f32 |
| --- | ---: | ---: | ---: | ---: |
| clip | 54.1 | 55.9 | 43.4 | 36.3 |
| css-minde | 476.7 | 624.0 | 567.9 | 389.5 |
| oklch-cubic (cached) | 95.2 | 79.9 | 70.8 | 62.4 |
| oklch-cubic (no cache) | 261.3 | 259.1 | 223.9 | 232.8 |
| oklch-cubic-direct | 230.0 | 240.3 | 217.0 | 181.5 |
| oklch-halley | 119.8 | 112.2 | 112.6 | 99.4 |
| oklch-ostrowski | 119.5 | 116.6 | 115.2 | 96.2 |
| dualray | 78.5 | 76.6 | 66.7 | 52.6 |
| dualray fast | 57.3 | 51.9 | 41.8 | 32.7 |
| dualray fast (poly encode) | — | — | 33.4 | 30.3 |
| bottosson-lightness | 119.6 | 122.6 | 102.4 | 82.3 |
| bottosson-lightness (cached) | 67.2 | 70.6 | 45.9 | 39.6 |
| edge-seeker | 155.9 | 146.1 | 90.0 | 80.8 |
| edge-seeker (indexed) | 77.4 | 77.2 | 56.7 | 45.3 |
| raytrace | 235.2 | 244.6 | 220.8 | 166.2 |

## Where the next experiments would pay

For Fast, measure whether sharing trig between a failed canonical precheck and the upper solve removes useful work in both JS and Rust. Any reuse must preserve authored-hue conversion and in-gamut output bits. CSS MINDE provides the clearest current evidence for testing a faster native cube root; Raytrace needs a separate dependency-chain profile because the same throughput calculation does not explain its measured gap. For uncached f32 cubic, profile candidate validation before changing arithmetic. On mostly interior input, measure conversion and membership handling first.

Memory-focused experiments have different goals: a u16 Rust Edge Seeker index could reduce its extra payload from 28 to 7 KiB; a seven-scalar cubic cache would exchange coefficient reconstruction for less storage. Earlier prototypes and the cube-expression/cache-layout history are preserved in [the notes](PERFORMANCE-NOTES.md). Each experiment needs its own output/accuracy checks and end-to-end timing.

## Measurement and reproduction

Jobs ran serially with logical CPU affinity `2,3`, one method/target/runtime per process, under linux 6.18.33.2-microsoft-standard-WSL2 and ldd (Ubuntu GLIBC 2.43-2ubuntu2.4) 2.43. Rust used `-C target-cpu=native`, opt-level=3, lto=true, codegen-units=1, panic=abort. f32 samples were rounded before timing; output channels were individually widened for the checksum. CPU frequency was not fixed.

Each process ran 50 complete warmup passes and 25 timed passes; the cell order rotated and reversed between rounds. All workloads contain 35,640 colors. Rust used input and checksum optimization barriers. Factories, table/index construction and initial cache filling preceded the measured passes.

There are **638 measured cells** and **1914 fresh timing processes**. The median process range, (maximum−minimum)/median, is **1.4%**; **8 cells** span more than 10%. The ranges are repeatability diagnostics, not confidence intervals.

Bun Fast has **0/11 cells** with a process range above 5%. The earlier single-CPU run showed multiple timing modes, motivating the current affinity setting; the [affinity history](PERFORMANCE-NOTES.md#affinity-experiment) records that investigation.

Every cell was validated in a separate process before timing. All outputs had to be finite and in range, and timed checksums had to agree with the validation sum over 75 passes. The regression suites and report-harness checks passed: **119 Rust tests**, **98 Bun tests**, **95 Node numerical tests**, **3 Node CLI tests**, **8 Node harness tests**, **8 Bun harness tests**. [Commands and complete validation logs](reports/performance-2026-10-02-validation.json) accompany the timing artifact.

The [Math-call profiler](scripts/profile-performance-math.mjs) replayed each P3 workload after warming caches, delegated every counted call to the original Math function, and required exact agreement with the Node validation checksum. A separate pass instruments Fast's precheck, lower, upper and exact-search entries in an in-memory source copy, checking every output channel against production. Source replacement markers must match exactly. Gamma powers and hardware branch misses were not counted. The artifact is bound to the timing artifact's hash.

The measured algorithm baseline is `ee5b48843570a2ed90df7cbd28d55e2d1029a087`. The artifact records input, source and Rust-binary hashes. Resume verifies source hashes, binary hash, runtime versions, CPU model and affinity.

```sh
# Generate fresh measurements on available logical CPUs.
bun scripts/run-performance.mjs --output reports/performance-new.json --cpu 2,3 --runs 3

# Resume an interrupted run with the same code, inputs and environment.
bun scripts/run-performance.mjs --output reports/performance-new.json --cpu 2,3 --runs 3 --resume

# Collect untimed operation counts, then regenerate the article and tables.
node scripts/profile-performance-math.mjs reports/performance-new.json
bun scripts/render-performance.mjs reports/performance-new.json
```

The [analysis template](scripts/templates/performance-analysis.md) is hand-written; the renderer fills its numbers from the artifacts. [Editorial checks](scripts/performance-analysis.mjs) stop regeneration if a central comparison changes, so the explanation can be revised with the data. [PERFORMANCE-NOTES.md](PERFORMANCE-NOTES.md) retains history rather than current ranking claims.

Use `--prepare-only` or `--validate-only`, then `--resume`, for staged measurement. The ignored `<output>.inputs/` directory holds common little-endian f64 triples regenerated by [the workload builder](scripts/performance-workloads.mjs). The [JS runner](scripts/bench-rgb-kernels.mjs) and [Rust runner](rust/examples/performance.rs) call production methods; [orchestration](scripts/run-performance.mjs) keeps timing and validation separate.

## Appendix A: standard workloads

### Display-P3: ordered integer hues

| Method | Node | Bun | Rust f64 | Rust f32 |
| --- | ---: | ---: | ---: | ---: |
| clip | 39.9 | 41.2 | 30.4 | 20.3 |
| css-minde | 422.4 | 572.0 | 526.6 | 361.0 |
| oklch-cubic (cached) | 65.4 | 59.0 | 50.4 | 45.5 |
| oklch-cubic (no cache) | 241.9 | 233.8 | 205.7 | 217.1 |
| oklch-cubic-direct | 206.0 | 213.0 | 194.0 | 162.2 |
| oklch-halley | 97.9 | 92.6 | 98.1 | 85.1 |
| oklch-ostrowski | 97.4 | 95.3 | 98.1 | 79.2 |
| dualray | 63.5 | 61.8 | 52.6 | 40.8 |
| dualray fast | 48.3 | 42.6 | 31.1 | 24.2 |
| dualray fast (poly encode) | — | — | 22.8 | 21.9 |
| bottosson-lightness | 101.3 | 105.8 | 90.0 | 71.9 |
| bottosson-lightness (cached) | 44.0 | 48.7 | 31.4 | 24.8 |
| edge-seeker | 91.5 | 98.0 | 47.5 | 33.2 |
| edge-seeker (indexed) | 60.7 | 60.6 | 41.2 | 30.8 |
| raytrace | 212.8 | 215.1 | 205.1 | 146.7 |

### sRGB: ordered integer hues

| Method | Node | Bun | Rust f64 | Rust f32 |
| --- | ---: | ---: | ---: | ---: |
| clip | 39.1 | 43.0 | 29.6 | 19.9 |
| css-minde | 429.7 | 591.6 | 521.9 | 349.9 |
| oklch-cubic (cached) | 70.9 | 63.3 | 50.6 | 45.1 |
| oklch-cubic (no cache) | 243.0 | 238.3 | 199.7 | 217.7 |
| oklch-cubic-direct | 212.6 | 220.3 | 191.4 | 161.5 |
| oklch-halley | 107.4 | 118.9 | 99.5 | 88.4 |
| oklch-ostrowski | 102.7 | 117.6 | 93.3 | 81.7 |
| dualray | 63.7 | 60.2 | 55.0 | 41.3 |
| dualray fast | 46.8 | 41.7 | 30.9 | 23.8 |
| dualray fast (poly encode) | — | — | 22.7 | 21.7 |
| bottosson-lightness | 99.6 | 106.0 | 89.5 | 74.2 |
| bottosson-lightness (cached) | 42.1 | 44.4 | 32.1 | 27.3 |
| edge-seeker | 91.2 | 96.2 | 47.6 | 35.9 |
| edge-seeker (indexed) | 58.7 | 60.0 | 41.4 | 29.8 |
| raytrace | 216.2 | 220.5 | 205.2 | 146.7 |

### sRGB: shuffled fractional hues

| Method | Node | Bun | Rust f64 | Rust f32 |
| --- | ---: | ---: | ---: | ---: |
| clip | 53.6 | 57.4 | 43.5 | 36.4 |
| css-minde | 473.2 | 636.9 | 574.6 | 390.4 |
| oklch-cubic (cached) | 96.4 | 81.4 | 70.9 | 63.2 |
| oklch-cubic (no cache) | 266.4 | 256.9 | 224.1 | 235.7 |
| oklch-cubic-direct | 235.7 | 243.4 | 217.7 | 184.0 |
| oklch-halley | 128.8 | 138.7 | 114.0 | 102.1 |
| oklch-ostrowski | 133.1 | 138.7 | 110.4 | 95.7 |
| dualray | 78.8 | 74.5 | 68.4 | 51.9 |
| dualray fast | 57.7 | 51.6 | 41.8 | 32.2 |
| dualray fast (poly encode) | — | — | 33.3 | 29.8 |
| bottosson-lightness | 119.7 | 126.3 | 103.3 | 85.7 |
| bottosson-lightness (cached) | 65.6 | 65.9 | 46.8 | 41.0 |
| edge-seeker | 153.9 | 146.6 | 90.7 | 76.5 |
| edge-seeker (indexed) | 77.1 | 75.2 | 56.8 | 43.8 |
| raytrace | 233.1 | 243.0 | 222.4 | 166.8 |

### Rec.2020: ordered integer hues

| Method | Node | Bun | Rust f64 | Rust f32 |
| --- | ---: | ---: | ---: | ---: |
| clip | 42.7 | 46.7 | 33.2 | 20.2 |
| css-minde | 420.2 | 583.1 | 515.8 | 345.0 |
| oklch-cubic (cached) | 82.9 | 72.7 | 59.3 | 50.1 |
| oklch-cubic (no cache) | 253.1 | 242.0 | 205.1 | 221.7 |
| oklch-cubic-direct | 221.2 | 232.6 | 197.4 | 164.6 |
| oklch-halley | 115.6 | 128.1 | 106.2 | 93.4 |
| oklch-ostrowski | 120.0 | 131.6 | 106.6 | 85.6 |
| dualray | 67.8 | 63.6 | 58.7 | 43.2 |
| dualray fast | 50.2 | 45.2 | 34.1 | 24.9 |
| dualray fast (poly encode) | — | — | 24.8 | 22.9 |
| bottosson-lightness | 105.4 | 111.0 | 94.7 | 76.8 |
| bottosson-lightness (cached) | 49.4 | 48.6 | 36.0 | 29.0 |
| edge-seeker | 92.0 | 108.2 | 51.4 | 35.8 |
| edge-seeker (indexed) | 64.7 | 62.5 | 44.5 | 30.9 |
| raytrace | 218.1 | 221.7 | 210.2 | 146.0 |

### Rec.2020: shuffled fractional hues

| Method | Node | Bun | Rust f64 | Rust f32 |
| --- | ---: | ---: | ---: | ---: |
| clip | 57.2 | 61.5 | 46.5 | 35.2 |
| css-minde | 463.6 | 612.1 | 554.4 | 380.9 |
| oklch-cubic (cached) | 105.0 | 89.3 | 78.2 | 65.9 |
| oklch-cubic (no cache) | 274.9 | 267.3 | 225.2 | 236.7 |
| oklch-cubic-direct | 242.3 | 254.7 | 220.5 | 181.6 |
| oklch-halley | 136.1 | 145.5 | 118.5 | 102.9 |
| oklch-ostrowski | 138.4 | 149.9 | 122.0 | 98.5 |
| dualray | 79.0 | 75.4 | 69.5 | 50.8 |
| dualray fast | 58.1 | 55.0 | 43.6 | 32.0 |
| dualray fast (poly encode) | — | — | 33.3 | 29.7 |
| bottosson-lightness | 123.0 | 129.8 | 106.9 | 85.2 |
| bottosson-lightness (cached) | 68.8 | 60.9 | 45.7 | 36.0 |
| edge-seeker | 156.9 | 148.1 | 90.5 | 75.1 |
| edge-seeker (indexed) | 79.7 | 77.1 | 57.6 | 43.0 |
| raytrace | 235.0 | 245.0 | 223.7 | 164.5 |

## Appendix B: cusp-side and membership workloads

### Below the P3 cusp

| Method | Node | Bun | Rust f64 | Rust f32 |
| --- | ---: | ---: | ---: | ---: |
| clip | 49.8 | 51.9 | 40.3 | 32.2 |
| css-minde | 482.1 | 614.6 | 566.3 | 387.4 |
| oklch-cubic (cached) | 72.7 | 61.3 | 54.7 | 43.8 |
| oklch-cubic (no cache) | 242.0 | 235.0 | 204.9 | 215.1 |
| oklch-cubic-direct | 220.7 | 230.8 | 208.8 | 175.3 |
| oklch-halley | 113.0 | 107.2 | 110.4 | 97.5 |
| oklch-ostrowski | 115.6 | 111.7 | 113.4 | 95.2 |
| dualray | 71.0 | 70.1 | 58.3 | 45.9 |
| dualray fast | 35.4 | 31.7 | 25.2 | 19.2 |
| dualray fast (poly encode) | — | — | 17.4 | 17.2 |
| bottosson-lightness | 111.5 | 110.8 | 93.9 | 75.4 |
| bottosson-lightness (cached) | 55.7 | 57.2 | 38.3 | 33.3 |
| edge-seeker | 146.7 | 137.2 | 82.6 | 75.5 |
| edge-seeker (indexed) | 68.8 | 69.7 | 48.3 | 39.8 |
| raytrace | 226.9 | 233.3 | 216.1 | 163.4 |

### Above the P3 cusp

| Method | Node | Bun | Rust f64 | Rust f32 |
| --- | ---: | ---: | ---: | ---: |
| clip | 53.9 | 54.4 | 40.6 | 37.3 |
| css-minde | 467.1 | 602.6 | 559.2 | 374.6 |
| oklch-cubic (cached) | 140.3 | 120.4 | 100.7 | 107.4 |
| oklch-cubic (no cache) | 315.6 | 298.6 | 247.3 | 272.9 |
| oklch-cubic-direct | 226.2 | 228.7 | 211.1 | 181.1 |
| oklch-halley | 123.3 | 114.7 | 113.1 | 106.5 |
| oklch-ostrowski | 116.7 | 112.1 | 106.9 | 93.4 |
| dualray | 93.5 | 89.5 | 82.1 | 66.4 |
| dualray fast | 100.4 | 91.7 | 78.8 | 61.3 |
| dualray fast (poly encode) | — | — | 68.8 | 58.9 |
| bottosson-lightness | 135.5 | 137.6 | 115.5 | 97.9 |
| bottosson-lightness (cached) | 86.4 | 95.0 | 58.2 | 50.6 |
| edge-seeker | 158.6 | 159.5 | 93.5 | 82.7 |
| edge-seeker (indexed) | 83.4 | 81.8 | 62.8 | 48.4 |
| raytrace | 235.3 | 240.9 | 220.4 | 161.1 |

### P3 random, checked entry

| Method | Node | Bun | Rust f64 | Rust f32 |
| --- | ---: | ---: | ---: | ---: |
| clip | 53.9 | 55.4 | 43.1 | 36.1 |
| css-minde | 476.4 | 632.4 | 570.0 | 390.7 |
| oklch-cubic (cached) | 117.5 | 102.0 | 87.2 | 81.0 |
| oklch-cubic (no cache) | 284.6 | 275.6 | 238.3 | 257.7 |
| oklch-cubic-direct | 245.7 | 262.3 | 221.0 | 188.9 |
| oklch-halley | 122.9 | 134.4 | 115.4 | 104.9 |
| oklch-ostrowski | 132.2 | 140.8 | 117.5 | 101.6 |
| dualray | 78.4 | 76.2 | 66.2 | 52.8 |
| dualray fast | 59.4 | 51.9 | 42.0 | 32.7 |
| dualray fast (poly encode) | — | — | 33.7 | 30.1 |
| bottosson-lightness | 131.5 | 128.1 | 102.9 | 91.9 |
| bottosson-lightness (cached) | 96.0 | 90.3 | 61.6 | 55.0 |
| edge-seeker | 153.2 | 149.1 | 88.8 | 86.6 |
| edge-seeker (indexed) | 98.5 | 96.6 | 69.2 | 59.6 |
| raytrace | 237.9 | 242.8 | 227.4 | 170.1 |

### P3 50% in gamut, checked entry

| Method | Node | Bun | Rust f64 | Rust f32 |
| --- | ---: | ---: | ---: | ---: |
| clip | 57.7 | 59.1 | 45.5 | 31.6 |
| css-minde | 272.7 | 344.8 | 309.1 | 209.4 |
| oklch-cubic (cached) | 91.8 | 83.6 | 67.9 | 57.0 |
| oklch-cubic (no cache) | 178.4 | 173.7 | 143.1 | 141.2 |
| oklch-cubic-direct | 154.9 | 171.5 | 134.5 | 109.7 |
| oklch-halley | 93.5 | 100.8 | 81.6 | 67.0 |
| oklch-ostrowski | 93.1 | 103.5 | 83.7 | 65.2 |
| dualray | 81.6 | 80.4 | 68.2 | 53.5 |
| dualray fast | 67.1 | 61.8 | 47.1 | 34.6 |
| dualray fast (poly encode) | — | — | 42.6 | 33.1 |
| bottosson-lightness | 93.2 | 96.2 | 75.9 | 61.1 |
| bottosson-lightness (cached) | 83.3 | 77.8 | 55.3 | 43.9 |
| edge-seeker | 108.7 | 109.1 | 68.5 | 57.1 |
| edge-seeker (indexed) | 85.7 | 80.8 | 58.9 | 44.3 |
| raytrace | 150.9 | 155.9 | 138.8 | 100.4 |

### P3 100% in gamut, checked entry

| Method | Node | Bun | Rust f64 | Rust f32 |
| --- | ---: | ---: | ---: | ---: |
| clip | 61.8 | 62.2 | 46.5 | 27.4 |
| css-minde | 62.5 | 58.1 | 48.8 | 30.0 |
| oklch-cubic (cached) | 62.3 | 59.3 | 47.7 | 32.6 |
| oklch-cubic (no cache) | 61.5 | 60.0 | 48.7 | 29.8 |
| oklch-cubic-direct | 61.8 | 59.1 | 48.0 | 29.6 |
| oklch-halley | 61.6 | 59.4 | 48.1 | 29.5 |
| oklch-ostrowski | 62.8 | 59.3 | 47.8 | 29.3 |
| dualray | 82.3 | 81.1 | 68.1 | 52.0 |
| dualray fast | 67.6 | 64.8 | 51.5 | 35.8 |
| dualray fast (poly encode) | — | — | 51.4 | 35.8 |
| bottosson-lightness | 63.5 | 59.2 | 47.8 | 31.5 |
| bottosson-lightness (cached) | 61.5 | 58.4 | 48.1 | 32.3 |
| edge-seeker | 64.0 | 58.4 | 47.7 | 28.6 |
| edge-seeker (indexed) | 63.7 | 58.4 | 47.9 | 28.7 |
| raytrace | 62.9 | 58.3 | 51.5 | 31.2 |

## Appendix C: inputs, operation counts and process ranges

| Workload | Target | Canonical in gamut (f64) | Entry mode |
| --- | --- | ---: | --- |
| grid | display-p3 | 0/35640 (0.0%) | plain |
| random | display-p3 | 0/35640 (0.0%) | plain |
| grid | srgb | 0/35640 (0.0%) | plain |
| random | srgb | 0/35640 (0.0%) | plain |
| grid | rec2020 | 139/35640 (0.4%) | plain |
| random | rec2020 | 126/35640 (0.4%) | plain |
| below-cusp | display-p3 | 0/35640 (0.0%) | plain |
| above-cusp | display-p3 | 0/35640 (0.0%) | plain |
| random-checked | display-p3 | 0/35640 (0.0%) | checked |
| mixed-checked | display-p3 | 17820/35640 (50.0%) | checked |
| inside-checked | display-p3 | 35640/35640 (100.0%) | checked |

The grid descends through integer hues at C=0.4; random uses independently stratified and shuffled fractional hue/lightness at the same chroma. Rec.2020 contains a small in-gamut subset, listed above. The artifact also records native f32 membership counts.

The P3 cusp-side inputs preserve random hue/order and redistribute lightness with a 0.01 margin either side of a cusp derived from the independent XYZ/stationary-interval reference. The mixed workload alternates C=0.4 with C=0.01·min(L,1−L); the interior workload uses the latter throughout.

Checked entry adds canonical conversion/membership where supported. CSS MINDE, Dualray and Fast retain intrinsic handling in both modes. Dualray uses its normalized-cubic first-exit policy; unchecked Bottosson deliberately projects even interior input to its fitted boundary. Narrow blue-fold paths have dedicated correctness tests, but the aggregate workloads do not characterize their worst-case latency.

### Current P3 random Math calls per color

| Method | cbrt | sqrt | sin | cos | acos |
| --- | ---: | ---: | ---: | ---: | ---: |
| clip | 0.00 | 0.00 | 1.00 | 1.00 | 0.00 |
| css-minde | 32.81 | 10.94 | 1.00 | 1.00 | 0.00 |
| oklch-cubic (cached) | 0.56 | 0.30 | 0.00 | 0.06 | 0.02 |
| oklch-cubic (no cache) | 5.04 | 5.01 | 1.00 | 3.33 | 0.78 |
| oklch-cubic-direct | 3.46 | 3.51 | 1.00 | 2.83 | 0.61 |
| oklch-halley | 0.00 | 0.00 | 1.00 | 1.00 | 0.00 |
| oklch-ostrowski | 0.00 | 0.00 | 1.00 | 1.00 | 0.00 |
| dualray | 0.00 | 0.15 | 1.00 | 1.00 | 0.00 |
| dualray fast | 0.00 | 0.33 | 0.34 | 0.34 | 0.00 |
| bottosson-lightness | 1.00 | 0.00 | 1.00 | 1.00 | 0.00 |
| bottosson-lightness (cached) | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 |
| edge-seeker | 0.00 | 0.51 | 1.00 | 1.00 | 0.00 |
| edge-seeker (indexed) | 0.00 | 0.51 | 1.00 | 1.00 | 0.00 |
| raytrace | 9.00 | 3.00 | 1.00 | 1.00 | 0.00 |

### Cusp-side operation and path counts

Untimed Node/P3 calls per color on the same below/above-cusp inputs as the timing tables.

| Method | Cusp side | cbrt | sqrt | sin | cos | acos |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| oklch-cubic (cached) | Below | 0.382 | 0.191 | 0.000 | 0.001 | 0.000 |
| dualray fast | Below | 0.000 | 0.329 | 0.000 | 0.000 | 0.000 |
| edge-seeker | Below | 0.000 | 0.000 | 1.000 | 1.000 | 0.000 |
| oklch-cubic (cached) | Above | 0.832 | 0.503 | 0.000 | 0.262 | 0.087 |
| dualray fast | Above | 0.000 | 0.329 | 1.217 | 1.217 | 0.000 |
| edge-seeker | Above | 0.000 | 2.000 | 1.000 | 1.000 | 0.000 |

| Dualray Fast cusp side | Lower entries | Upper entries | Exact-search entries | Canonical prechecks |
| --- | ---: | ---: | ---: | ---: |
| Below | 1.000 | 0.000 | 0.000 | 0.000 |
| Above | 0.000 | 1.000 | 0.000 | 0.217 |

Path entries overlap: a canonical precheck can precede upper solving, and rejected upper work can precede exact recovery. On this above-cusp corpus, the extra trig comes entirely from prechecks; exact-search entries are zero.

### Largest process ranges

| Workload | Runtime | Method | Median | Min–max | Range/median |
| --- | --- | --- | ---: | ---: | ---: |
| display-p3-above-cusp | Rust f64 | bottosson-lightness (cached) | 58.2 | 58.1–68.9 | 18.4% |
| display-p3-grid | Rust f32 | raytrace | 146.7 | 145.6–162.4 | 11.5% |
| rec2020-random | Bun | oklch-ostrowski | 149.9 | 148.1–165.2 | 11.4% |
| rec2020-random | Node | oklch-cubic (no cache) | 274.9 | 266.1–295.9 | 10.9% |
| display-p3-random | Node | oklch-halley | 119.8 | 119.6–132.5 | 10.7% |
| srgb-random | Bun | edge-seeker | 146.6 | 144.5–160.1 | 10.7% |
| display-p3-grid | Rust f64 | bottosson-lightness | 90.0 | 89.5–98.8 | 10.3% |
| display-p3-random-checked | Bun | edge-seeker (indexed) | 96.6 | 88.1–97.9 | 10.2% |
