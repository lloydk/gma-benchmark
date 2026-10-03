# Performance analysis

The largest speed differences here come from avoiding work: fitting a lower face instead of solving it, caching hue structure, reusing RGB values, or returning before a perceptual search. Which shortcut applies depends on the input as much as the method.

Measured 2026-10-03 on AMD Ryzen 7 9800X3D 8-Core Processor, using Node v26.10.0, Bun 1.4.2 and rustc 1.99.0 (b940084d7 2026-09-28). Coverage: **15 methods in Node, 15 methods in Bun, 16 methods in Rust f64, 16 methods in Rust f32**, across 3 gamuts. All times are **nanoseconds per color**, including encoded RGB output.

## Key findings

### 1. Dualray Fast earns its lead below the cusp

On shuffled P3 input, Fast cuts Dualray's time by **26.6%–38.5% across these runtimes**. Its shortcut
fits the lower boundary and the two nonzero RGB channels directly from hue,
then scales them by lightness cubed. That replaces trigonometry, channel-cubic
construction and root refinement with a small polynomial evaluation.

The difference is visible when the workload is split: Node Fast takes
**34.7 ns below the cusp and 100.3 ns above it**;
Rust f64 takes **24.8 and 76.6 ns**. The fresh Node
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
| Node | 26.6% | 10.9% more time |
| Bun | 32.5% | 1.4% more time |
| Rust f64 | 38.5% | 5.7% less time |
| Rust f32 | 38.5% | 7.2% less time |

This matters because 74.7% of the standard P3 random corpus lies
below the cusp. A bright-skewed workload loses much of the shortcut's benefit.
Fast's approximation buys a particularly cheap lower face, rather than a
uniform reduction in the cost of every Dualray path.

The `dualray fast (tables)` row reduces the above-cusp cost itself. A
generated 360-byte table per gamut corrects the upper solve's seed, so one
Householder step usually converges; Rust also takes the hue direction from a
22.5° table instead of `sin_cos`, while JavaScript keeps `Math.sin` and
`Math.cos`. Above the cusp it takes **62.7 ns in Rust f64 and
50.3 in f32**, against Fast's 76.6 and
61.1, and **92.9 ns in Node and
86.0 in Bun**, against 100.3 and
91.2. Below the cusp the two rows run the same code, so the P3
random saving is **1.9%–10.0% across runtimes**. Rust's larger gain includes its
direction table.
([Implementation](src/dualray-fast.js))

### 2. Caches win by removing whole phases of work

On P3 random inputs, caching speeds cubic up by 2.83–3.88× across runtimes, and
Bottosson by 1.71–2.27× across runtimes. Cubic stores the hue-only coefficients,
lower root and turning points; Bottosson stores its cusp and LMS direction
slopes. After warmup, both skip hue trig, and cached Bottosson also skips cusp
construction and its cube root. The current counters put uncached cubic at
**5.04 cbrt calls/color**, versus **0.56** cached.

Repeated integer hues can favor caches enough to change the ranking. The lowest mapped
P3 grid medians are bottosson-lightness (cached) in Node (45.0 ns) and dualray fast (tables) in Bun (40.7 ns). The price is persistent state—366/183 KiB
for cubic f64/f32 and 141/70 KiB for Bottosson—and 0.1° hue buckets. Cubic's two
rows use the same bucket semantics; cached Bottosson changes fractional-hue
evaluation compared with its uncached counterpart. Choose caching when that
memory and numerical trade-off fit the application.
([Cubic](src/oklch-cubic.js), [Bottosson](src/bottosson-factory.js))

### 3. A mapper can beat clipping by doing less conversion

Below the cusp, Rust f64 Fast takes **24.8 ns** against clip's
**40.7 ns**. Clipping must first convert the original OKLCh color:
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

CSS MINDE takes **475.2 ns in Node**, **629.3 in Bun** and
**590.1 in Rust f64** on P3 random input. Each clipping-error comparison
converts clipped RGB back to Oklab using three cube roots. The current Node
counter run records **32.8 cbrt calls/color**, or **10.9
error comparisons**, on average.

The size of the Node/Bun gap is consistent with the cube-root difference.
The Node 26.10.0/Bun 1.4.2 operation probes supplied with the review measured
about 4.3 versus 8.7 ns for cbrt throughput; earlier investigation identified
V8's own implementation versus glibc on this host. Multiplying the current
**32.8 calls by that 4.4 ns difference gives about 144.4
ns**, close to the measured **154.1 ns** gap. This is a consistency
check on the engine comparison, rather than a profile assigning parts of the
mapper's total time. CSS MINDE is the strongest current case for investigating
a faster native cube root.

Raytrace is different in these measurements. Its **9.0** cube
roots would suggest about **39.6 ns** by the same arithmetic,
but the Node/Bun gap is only **10.4 ns**; it takes
**234.9 ns in Node, 245.4 ns in Bun, 216.2 ns in Rust f64**. Node is slower than Rust f64 here. The isolated operation
probe does not predict the full Raytrace gap; its dependency chains, argument
distribution and surrounding work need separate profiling. The old cube-root
story is not enough to explain today's Raytrace timings.
([CSS MINDE](src/css-minde.js), [Raytrace](src/raytrace.js))

### 5. Polynomial encoding buys more in f64

Replacing Fast's ordinary output power with the polynomial reduces its Rust
P3 random time from **40.8 to 32.3 ns in f64**, saving
**8.5 ns**. In f32 it moves from **32.3 to 29.8
ns**, saving **2.5 ns**. The boundary mapper is the same in each
pair, so this directly measures the benefit of the encoder variant.

The polynomial uses the same degree in both precisions. It does not spend
extra terms recovering binary64 accuracy, while the ordinary encoder uses
each lane's native power operation. The interior workload gives a second
piece of evidence: across the other canonical-checking methods, median time
is **31.0 ns in f32 versus 47.9 in f64**.
Those paths return before solving a boundary, directly showing the cheaper
f32 conversion-and-encoding path. Together, these results support a smaller
cost to replace in f32, although they do not isolate the power call itself.

Mostly interior traffic also removes the polynomial encoder's opportunity:
every Fast row deliberately keeps ordinary encoding for canonical in-gamut
output. Use the mapped fraction, precision and accepted error budget to decide
whether the extra approximation pays.
([Rust encoder](rust/src/dualray_fast.rs))

### 6. Edge Seeker's index addresses unpredictable lookup work

In Rust f32, plain Edge Seeker goes from **36.0 ns on the grid to
79.7 ns on random hues**, a **2.21×** increase. The indexed
version moves from **30.3 to 43.5 ns**, or
**1.43×**. Both variants evaluate the same interpolated boundary at
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

On P3 random input, Halley and Ostrowski take **120.1/119.7
ns in Node** and **108.0/110.6 ns in Rust f64**. Halley
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

Rust f32 has a lower P3 random median for **15/16 methods**,
but uncached cubic takes **228.1 ns versus 223.6 in f64**.
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

On the all-interior P3 workload, Node CSS MINDE falls to **62.9
ns**, from **476.5 ns** on identical-lightness/hue out-of-gamut
input. Its cube-root count falls to **0.0**: the canonical
conversion returns before the clipping-error search. Node Fast instead takes
**67.3 ns inside versus 57.5 ns outside**,
because its cheap mapped lower path can skip the conversion that pass-through
must preserve.

Most canonical-checking methods consequently converge toward conversion cost
on interior input. For example, checked Halley takes **48.0
ns in f64 and 30.4 ns in f32** there. Dualray retains its
first-exit policy and still does boundary work, taking **69.6
ns in f64**. For mostly in-gamut applications, the pass-through path and the
cost of a failed precheck deserve as much attention as the boundary solver.
The 50% mixture in the appendix shows the transition between those workloads.

## Comparing runtimes

Node and Bun are near parity in the median across shared methods, but that
overall similarity hides useful differences. Bun's cached cubic takes **83.6 ns
against Node's 94.2**, and Bun Fast takes **52.6 against
57.9 ns**. Node wins CSS MINDE, where its cube-root implementation
has much more work to influence. The median JS/Rust f64 ratios are around
1.2× on this workload; the individual method and math-library path matter more
than a blanket language multiplier.

| P3 random ratio | Median across shared methods | Range across methods |
| --- | ---: | ---: |
| Node / Bun | 1.03× | 0.76–1.13× |
| Node / Rust f64 | 1.20× | 0.81–1.73× |
| Bun / Rust f64 | 1.18× | 1.05–1.64× |

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
| Node | 1.00× | 1.02× | 1.01× | 1.07× |
| Bun | 1.01× | 1.05× | 1.01× | 1.07× |
| Rust f64 | 0.99× | 1.01× | 1.00× | 1.07× |
| Rust f32 | 1.00× | 0.99× | 1.00× | 1.03× |

Each entry is the median of the individual method's target/P3 time ratios,
including clip and every method available in that runtime. Below one means
less time than P3. These are the same input triples in the same order for all
three targets, so differences come from processing them for a different gamut.

**Dualray's performance carries across targets.** On random input, the largest
change from P3 across Dualray and Dualray Fast, all targets and all four runtimes,
is 4.7%. A Dualray Fast row is the fastest mapped method on
every target: `dualray fast (tables)` in Node, Bun and Rust f32 and `dualray fast (poly encode)` in Rust f64. Choosing sRGB or Rec.2020
does not erase the lower-face shortcut's advantage on this distribution.

**The iterative solvers and cached cubic are more sensitive.** Bun Halley takes
140.8 ns for sRGB, 113.2 for P3 and
148.0 for Rec.2020 on random input. Halley and Ostrowski's
Rec.2020 penalties in Bun are 30.8%–31.1% there and
39.2%–40.4% on the grid. Cached cubic also costs more
for Rec.2020 in every runtime: 1.7%–13.2% on random input
and 10.4%–26.6% on the grid. P3 alone understates these costs.

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
Rec.2020 takes 0.8%–13.3% more time than P3 on random input in
Node, Bun and Rust f64. Rust f32 shows no penalty, at 36.0 ns
for Rec.2020 versus 36.0 for P3. Extra power
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
| Out-of-gamut, mostly above the cusp | In Rust, Dualray Fast (tables); in JavaScript, compare Dualray and the Fast rows on that distribution | In Rust the tables row takes 22.9%–23.5% less time than Dualray here. The JavaScript methods are within 10.9% of Dualray; Fast's large lower-face advantage does not carry over. Cached Bottosson and indexed Edge Seeker are also candidates when their boundary policies fit. |

## How to read these numbers

- Use the shuffled P3 workload as the main comparison, then check the cusp-side and in-gamut results for your input distribution. The integer-hue grid favors repeated lookup and branch patterns; the mixed/interior sets are controlled diagnostics. Full sRGB, Rec.2020 and diagnostic tables are in the appendices.
- Compare mapping policies as well as speed. Clipping and CSS MINDE can change lightness/hue; cached Bottosson uses 0.1° hue buckets; Fast targets an empirical ΔEOK budget of 1e-3 maximum / 1e-4 p99. Dualray retains first-exit boundary semantics, while canonical-checking paths preserve the authored conversion. [Method contracts](README.md#methods) describe the differences.
- Times are medians of 3 fresh-process medians after warmup, with all three RGB channels consumed. Setup and cold-cache costs are excluded. These are fixed-target OKLCh kernels; public color objects, input-space conversion, alpha and browser rendering would add different work.
- The explanations combine measured timings, untimed operation counts and source inspection. The cube-root library and branch-prediction explanations are supported mechanisms, not isolated shares of CPU time; f32 power latency remains an inference. Historical microbenchmarks and counter experiments are labeled in [PERFORMANCE-NOTES.md](PERFORMANCE-NOTES.md).
- Close rankings can change with JIT state, binary layout and scheduling. CPU affinity applies to worker threads too. The recorded process ranges below make the remaining variation visible; small median differences are not guarantees.

[Raw timing data](reports/performance-2026-10-03.json), [current Math-call counts](reports/performance-2026-10-03-math.json) accompany the report. These tables replace the historical timings that predated complete output consumption.

## Display-P3: the main comparison

Shuffled fractional hue/lightness, C=0.4, all inputs out of gamut. The findings above explain the differences in this table.

| Method | Node | Bun | Rust f64 | Rust f32 |
| --- | ---: | ---: | ---: | ---: |
| clip | 55.0 | 55.6 | 45.8 | 36.0 |
| css-minde | 475.2 | 629.3 | 590.1 | 394.8 |
| oklch-cubic (cached) | 94.2 | 83.6 | 71.4 | 58.8 |
| oklch-cubic (no cache) | 266.9 | 256.1 | 223.6 | 228.1 |
| oklch-cubic-direct | 230.1 | 242.9 | 217.8 | 176.2 |
| oklch-halley | 120.1 | 113.2 | 108.0 | 99.5 |
| oklch-ostrowski | 119.7 | 115.8 | 110.6 | 94.9 |
| dualray | 78.8 | 77.9 | 66.3 | 52.6 |
| dualray fast | 57.9 | 52.6 | 40.8 | 32.3 |
| dualray fast (poly encode) | — | — | 32.3 | 29.8 |
| dualray fast (tables) | 56.8 | 50.3 | 37.5 | 29.1 |
| bottosson-lightness | 120.6 | 121.4 | 100.3 | 79.6 |
| bottosson-lightness (cached) | 66.3 | 71.0 | 44.2 | 37.6 |
| edge-seeker | 154.4 | 146.7 | 89.3 | 79.7 |
| edge-seeker (indexed) | 80.1 | 76.5 | 56.2 | 43.5 |
| raytrace | 234.9 | 245.4 | 216.2 | 162.2 |

## Where the next experiments would pay

For Fast, measure whether sharing trig between a failed canonical precheck and the upper solve removes useful work in both JS and Rust. Any reuse must preserve authored-hue conversion and in-gamut output bits. CSS MINDE provides the clearest current evidence for testing a faster native cube root; Raytrace needs a separate dependency-chain profile because the same throughput calculation does not explain its measured gap. For uncached f32 cubic, profile candidate validation before changing arithmetic. On mostly interior input, measure conversion and membership handling first.

Memory-focused experiments have different goals: a u16 Rust Edge Seeker index could reduce its extra payload from 28 to 7 KiB; a seven-scalar cubic cache would exchange coefficient reconstruction for less storage. Earlier prototypes and the cube-expression/cache-layout history are preserved in [the notes](PERFORMANCE-NOTES.md). Each experiment needs its own output/accuracy checks and end-to-end timing.

## Measurement and reproduction

Jobs ran serially with logical CPU affinity `2,3`, one method/target/runtime per process, under linux 6.18.33.2-microsoft-standard-WSL2 and ldd (Ubuntu GLIBC 2.43-2ubuntu2.4) 2.43. Rust used `-C target-cpu=native`, opt-level=3, lto=true, codegen-units=1, panic=abort. f32 samples were rounded before timing; output channels were individually widened for the checksum. CPU frequency was not fixed.

Each process ran 50 complete warmup passes and 25 timed passes; the cell order rotated and reversed between rounds. All workloads contain 35,640 colors. Rust used input and checksum optimization barriers. Factories, table/index construction and initial cache filling preceded the measured passes.

There are **682 measured cells** and **2046 fresh timing processes**. The median process range, (maximum−minimum)/median, is **1.8%**; **7 cells** span more than 10%. The ranges are repeatability diagnostics, not confidence intervals.

Bun Fast has **3/11 cells** with a process range above 5%. The earlier single-CPU run showed multiple timing modes, motivating the current affinity setting; the [affinity history](PERFORMANCE-NOTES.md#affinity-experiment) records that investigation.

Every cell was validated in a separate process before timing. All outputs had to be finite and in range, and timed checksums had to agree with the validation sum over 75 passes. The regression suites and report-harness checks passed: **126 Rust tests**, **107 Bun tests**, **96 Node numerical tests**, **3 Node CLI tests**, **8 Node harness tests**, **8 Bun harness tests**. [Commands and complete validation logs](reports/performance-2026-10-03-validation.json) accompany the timing artifact.

The [Math-call profiler](scripts/profile-performance-math.mjs) replayed each P3 workload after warming caches, delegated every counted call to the original Math function, and required exact agreement with the Node validation checksum. A separate pass instruments Fast's precheck, lower, upper and exact-search entries in an in-memory source copy, checking every output channel against production. Source replacement markers must match exactly. Gamma powers and hardware branch misses were not counted. The artifact is bound to the timing artifact's hash.

The measured algorithm baseline is `58e586b0473f30e9934ba2b1454224a6a46d37c3` with uncommitted changes, which the recorded source hashes identify. The artifact records input, source and Rust-binary hashes. Resume verifies source hashes, binary hash, runtime versions, CPU model and affinity.

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
| clip | 40.1 | 42.7 | 30.3 | 20.4 |
| css-minde | 422.6 | 576.3 | 538.0 | 356.8 |
| oklch-cubic (cached) | 66.1 | 59.8 | 50.2 | 42.5 |
| oklch-cubic (no cache) | 238.4 | 236.2 | 196.5 | 213.2 |
| oklch-cubic-direct | 206.9 | 211.9 | 192.9 | 158.2 |
| oklch-halley | 98.5 | 91.8 | 95.2 | 85.0 |
| oklch-ostrowski | 97.8 | 94.5 | 93.2 | 78.2 |
| dualray | 63.9 | 62.0 | 52.8 | 40.3 |
| dualray fast | 48.4 | 41.9 | 30.4 | 23.6 |
| dualray fast (poly encode) | — | — | 22.0 | 22.4 |
| dualray fast (tables) | 46.0 | 40.7 | 27.7 | 21.1 |
| bottosson-lightness | 101.0 | 105.6 | 88.2 | 69.7 |
| bottosson-lightness (cached) | 45.0 | 49.0 | 30.5 | 24.0 |
| edge-seeker | 89.7 | 100.1 | 47.4 | 36.0 |
| edge-seeker (indexed) | 61.3 | 60.1 | 40.6 | 30.3 |
| raytrace | 214.9 | 218.4 | 203.3 | 143.4 |

### sRGB: ordered integer hues

| Method | Node | Bun | Rust f64 | Rust f32 |
| --- | ---: | ---: | ---: | ---: |
| clip | 39.6 | 42.8 | 29.4 | 20.0 |
| css-minde | 432.3 | 587.3 | 547.7 | 358.4 |
| oklch-cubic (cached) | 71.6 | 62.9 | 50.2 | 43.0 |
| oklch-cubic (no cache) | 242.8 | 240.4 | 203.4 | 213.9 |
| oklch-cubic-direct | 211.7 | 221.3 | 192.5 | 157.6 |
| oklch-halley | 108.0 | 120.1 | 95.9 | 88.4 |
| oklch-ostrowski | 104.1 | 118.4 | 90.0 | 82.7 |
| dualray | 64.2 | 60.2 | 53.9 | 40.5 |
| dualray fast | 48.8 | 42.4 | 30.1 | 23.5 |
| dualray fast (poly encode) | — | — | 22.0 | 21.6 |
| dualray fast (tables) | 46.3 | 40.2 | 27.3 | 20.7 |
| bottosson-lightness | 101.8 | 105.5 | 87.3 | 71.2 |
| bottosson-lightness (cached) | 42.7 | 44.9 | 30.9 | 26.1 |
| edge-seeker | 88.5 | 98.1 | 48.4 | 35.1 |
| edge-seeker (indexed) | 63.7 | 60.4 | 40.8 | 30.4 |
| raytrace | 216.4 | 223.5 | 201.5 | 144.6 |

### sRGB: shuffled fractional hues

| Method | Node | Bun | Rust f64 | Rust f32 |
| --- | ---: | ---: | ---: | ---: |
| clip | 53.6 | 57.1 | 43.0 | 36.4 |
| css-minde | 482.3 | 640.3 | 589.1 | 394.6 |
| oklch-cubic (cached) | 98.7 | 81.7 | 70.9 | 58.3 |
| oklch-cubic (no cache) | 269.0 | 263.8 | 222.7 | 228.4 |
| oklch-cubic-direct | 240.8 | 245.7 | 218.4 | 175.5 |
| oklch-halley | 130.1 | 140.8 | 112.7 | 101.1 |
| oklch-ostrowski | 125.5 | 137.0 | 106.7 | 96.0 |
| dualray | 80.2 | 74.6 | 67.6 | 51.6 |
| dualray fast | 57.8 | 52.0 | 39.8 | 31.6 |
| dualray fast (poly encode) | — | — | 32.0 | 29.8 |
| dualray fast (tables) | 56.8 | 50.9 | 36.7 | 28.7 |
| bottosson-lightness | 119.8 | 126.4 | 101.6 | 82.8 |
| bottosson-lightness (cached) | 65.9 | 66.4 | 44.9 | 38.6 |
| edge-seeker | 154.6 | 145.2 | 88.2 | 76.2 |
| edge-seeker (indexed) | 77.0 | 76.3 | 55.4 | 44.0 |
| raytrace | 235.9 | 241.9 | 216.1 | 161.7 |

### Rec.2020: ordered integer hues

| Method | Node | Bun | Rust f64 | Rust f32 |
| --- | ---: | ---: | ---: | ---: |
| clip | 44.3 | 48.3 | 32.9 | 20.2 |
| css-minde | 422.8 | 582.0 | 544.7 | 352.0 |
| oklch-cubic (cached) | 83.7 | 72.2 | 58.5 | 46.9 |
| oklch-cubic (no cache) | 249.9 | 246.2 | 206.2 | 215.3 |
| oklch-cubic-direct | 222.4 | 238.7 | 199.2 | 159.7 |
| oklch-halley | 117.3 | 127.8 | 101.5 | 93.5 |
| oklch-ostrowski | 118.7 | 132.7 | 102.7 | 85.7 |
| dualray | 67.5 | 64.0 | 58.0 | 42.6 |
| dualray fast | 55.7 | 45.5 | 32.2 | 24.5 |
| dualray fast (poly encode) | — | — | 23.0 | 22.8 |
| dualray fast (tables) | 48.6 | 43.6 | 29.6 | 21.5 |
| bottosson-lightness | 107.7 | 114.8 | 92.8 | 74.4 |
| bottosson-lightness (cached) | 50.4 | 50.2 | 35.1 | 27.6 |
| edge-seeker | 97.0 | 104.6 | 51.9 | 35.2 |
| edge-seeker (indexed) | 64.8 | 63.7 | 43.4 | 31.4 |
| raytrace | 221.7 | 226.2 | 202.9 | 144.0 |

### Rec.2020: shuffled fractional hues

| Method | Node | Bun | Rust f64 | Rust f32 |
| --- | ---: | ---: | ---: | ---: |
| clip | 57.7 | 63.0 | 46.2 | 36.0 |
| css-minde | 471.5 | 621.9 | 581.8 | 391.9 |
| oklch-cubic (cached) | 106.6 | 89.0 | 77.0 | 59.8 |
| oklch-cubic (no cache) | 269.9 | 268.8 | 225.9 | 230.1 |
| oklch-cubic-direct | 242.5 | 262.6 | 220.4 | 175.2 |
| oklch-halley | 138.6 | 148.0 | 114.7 | 102.3 |
| oklch-ostrowski | 138.9 | 151.8 | 117.2 | 97.7 |
| dualray | 79.4 | 76.4 | 68.9 | 50.2 |
| dualray fast | 58.9 | 55.0 | 41.3 | 31.2 |
| dualray fast (poly encode) | — | — | 31.7 | 29.6 |
| dualray fast (tables) | 57.2 | 53.5 | 38.9 | 28.6 |
| bottosson-lightness | 123.2 | 130.5 | 104.7 | 82.1 |
| bottosson-lightness (cached) | 68.5 | 62.3 | 44.4 | 34.1 |
| edge-seeker | 156.6 | 150.5 | 92.1 | 76.4 |
| edge-seeker (indexed) | 79.0 | 77.4 | 57.0 | 43.4 |
| raytrace | 236.9 | 246.8 | 215.5 | 160.7 |

## Appendix B: cusp-side and membership workloads

### Below the P3 cusp

| Method | Node | Bun | Rust f64 | Rust f32 |
| --- | ---: | ---: | ---: | ---: |
| clip | 49.8 | 52.6 | 40.7 | 32.2 |
| css-minde | 472.2 | 616.6 | 592.6 | 397.6 |
| oklch-cubic (cached) | 74.2 | 61.1 | 54.1 | 40.0 |
| oklch-cubic (no cache) | 244.8 | 237.8 | 204.5 | 209.9 |
| oklch-cubic-direct | 223.9 | 234.7 | 210.4 | 170.4 |
| oklch-halley | 114.4 | 108.1 | 107.3 | 97.7 |
| oklch-ostrowski | 114.9 | 112.4 | 111.2 | 94.0 |
| dualray | 71.5 | 70.0 | 58.3 | 46.4 |
| dualray fast | 34.7 | 35.0 | 24.8 | 18.8 |
| dualray fast (poly encode) | — | — | 17.4 | 17.1 |
| dualray fast (tables) | 35.6 | 31.7 | 25.5 | 18.7 |
| bottosson-lightness | 112.5 | 111.7 | 93.3 | 73.3 |
| bottosson-lightness (cached) | 56.1 | 57.8 | 37.3 | 31.7 |
| edge-seeker | 144.5 | 135.7 | 81.9 | 74.1 |
| edge-seeker (indexed) | 69.7 | 69.3 | 47.5 | 39.0 |
| raytrace | 226.5 | 236.5 | 213.9 | 159.2 |

### Above the P3 cusp

| Method | Node | Bun | Rust f64 | Rust f32 |
| --- | ---: | ---: | ---: | ---: |
| clip | 54.5 | 54.5 | 41.0 | 37.4 |
| css-minde | 470.1 | 605.6 | 584.9 | 382.1 |
| oklch-cubic (cached) | 139.7 | 120.8 | 100.8 | 102.4 |
| oklch-cubic (no cache) | 309.2 | 298.6 | 247.9 | 268.2 |
| oklch-cubic-direct | 221.3 | 228.6 | 210.7 | 178.1 |
| oklch-halley | 122.9 | 114.9 | 108.4 | 106.8 |
| oklch-ostrowski | 116.7 | 112.8 | 102.0 | 91.4 |
| dualray | 90.5 | 89.9 | 81.3 | 65.8 |
| dualray fast | 100.3 | 91.2 | 76.6 | 61.1 |
| dualray fast (poly encode) | — | — | 68.7 | 59.0 |
| dualray fast (tables) | 92.9 | 86.0 | 62.7 | 50.3 |
| bottosson-lightness | 141.3 | 136.4 | 117.2 | 92.8 |
| bottosson-lightness (cached) | 88.0 | 94.2 | 56.7 | 47.3 |
| edge-seeker | 159.4 | 165.5 | 94.1 | 81.3 |
| edge-seeker (indexed) | 81.5 | 84.2 | 63.2 | 48.4 |
| raytrace | 238.5 | 238.5 | 214.8 | 157.9 |

### P3 random, checked entry

| Method | Node | Bun | Rust f64 | Rust f32 |
| --- | ---: | ---: | ---: | ---: |
| clip | 54.2 | 56.5 | 43.8 | 36.0 |
| css-minde | 476.5 | 623.9 | 591.0 | 413.5 |
| oklch-cubic (cached) | 119.3 | 101.8 | 88.0 | 78.8 |
| oklch-cubic (no cache) | 286.2 | 272.6 | 239.2 | 259.5 |
| oklch-cubic-direct | 252.2 | 257.9 | 222.9 | 183.6 |
| oklch-halley | 122.7 | 135.1 | 112.3 | 106.4 |
| oklch-ostrowski | 125.5 | 139.0 | 114.6 | 100.2 |
| dualray | 79.5 | 76.2 | 65.9 | 52.8 |
| dualray fast | 57.5 | 52.2 | 40.5 | 32.3 |
| dualray fast (poly encode) | — | — | 32.3 | 29.7 |
| dualray fast (tables) | 56.0 | 50.1 | 36.9 | 29.0 |
| bottosson-lightness | 123.7 | 129.8 | 102.3 | 86.4 |
| bottosson-lightness (cached) | 96.2 | 92.1 | 60.8 | 54.4 |
| edge-seeker | 165.6 | 157.6 | 91.7 | 89.5 |
| edge-seeker (indexed) | 97.3 | 97.0 | 68.6 | 61.3 |
| raytrace | 238.4 | 248.3 | 221.6 | 165.7 |

### P3 50% in gamut, checked entry

| Method | Node | Bun | Rust f64 | Rust f32 |
| --- | ---: | ---: | ---: | ---: |
| clip | 58.7 | 59.2 | 46.1 | 31.5 |
| css-minde | 278.0 | 339.9 | 323.8 | 212.6 |
| oklch-cubic (cached) | 90.9 | 85.0 | 67.8 | 54.6 |
| oklch-cubic (no cache) | 180.7 | 171.7 | 143.8 | 139.3 |
| oklch-cubic-direct | 158.1 | 164.5 | 134.0 | 106.3 |
| oklch-halley | 93.9 | 99.2 | 80.6 | 67.1 |
| oklch-ostrowski | 93.7 | 102.2 | 81.0 | 65.1 |
| dualray | 81.6 | 80.8 | 68.7 | 52.9 |
| dualray fast | 66.8 | 62.9 | 46.7 | 34.3 |
| dualray fast (poly encode) | — | — | 42.6 | 32.9 |
| dualray fast (tables) | 65.6 | 60.9 | 45.2 | 32.9 |
| bottosson-lightness | 93.7 | 95.7 | 75.6 | 58.0 |
| bottosson-lightness (cached) | 82.7 | 77.0 | 54.4 | 43.9 |
| edge-seeker | 116.0 | 110.6 | 68.3 | 60.2 |
| edge-seeker (indexed) | 81.8 | 80.6 | 58.3 | 46.9 |
| raytrace | 151.2 | 155.9 | 134.3 | 101.0 |

### P3 100% in gamut, checked entry

| Method | Node | Bun | Rust f64 | Rust f32 |
| --- | ---: | ---: | ---: | ---: |
| clip | 62.2 | 62.6 | 47.0 | 27.4 |
| css-minde | 62.9 | 58.1 | 48.8 | 31.7 |
| oklch-cubic (cached) | 61.9 | 59.4 | 47.9 | 34.0 |
| oklch-cubic (no cache) | 62.0 | 59.8 | 47.3 | 30.3 |
| oklch-cubic-direct | 62.8 | 59.1 | 47.2 | 30.6 |
| oklch-halley | 63.8 | 58.8 | 48.0 | 30.4 |
| oklch-ostrowski | 62.5 | 60.0 | 47.6 | 31.0 |
| dualray | 83.7 | 80.4 | 69.6 | 51.3 |
| dualray fast | 67.3 | 64.3 | 52.9 | 36.2 |
| dualray fast (poly encode) | — | — | 53.6 | 36.3 |
| dualray fast (tables) | 68.6 | 63.9 | 53.4 | 36.3 |
| bottosson-lightness | 61.7 | 60.5 | 48.7 | 30.6 |
| bottosson-lightness (cached) | 61.9 | 58.2 | 49.1 | 33.4 |
| edge-seeker | 64.0 | 58.3 | 47.2 | 33.0 |
| edge-seeker (indexed) | 61.2 | 59.1 | 48.1 | 32.6 |
| raytrace | 62.2 | 58.2 | 47.7 | 30.3 |

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
| dualray fast (tables) | 0.00 | 0.33 | 0.34 | 0.34 | 0.00 |
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
| display-p3-inside-checked | Rust f32 | bottosson-lightness (cached) | 33.4 | 32.8–40.6 | 23.3% |
| display-p3-below-cusp | Bun | dualray fast | 35.0 | 31.7–37.1 | 15.7% |
| display-p3-random | Rust f64 | clip | 45.8 | 43.1–49.5 | 13.8% |
| display-p3-grid | Bun | clip | 42.7 | 41.3–46.6 | 12.3% |
| display-p3-mixed-checked | Node | edge-seeker | 116.0 | 109.8–123.5 | 11.8% |
| display-p3-below-cusp | Bun | dualray fast (tables) | 31.7 | 31.6–35.4 | 11.8% |
| srgb-grid | Node | bottosson-lightness (cached) | 42.7 | 41.7–46.0 | 10.0% |
| display-p3-above-cusp | Rust f32 | edge-seeker (indexed) | 48.4 | 48.0–52.6 | 9.4% |
