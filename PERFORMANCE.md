# Performance analysis

The largest speed differences here come from avoiding work: fitting a lower face instead of solving it, caching hue structure, reusing RGB values, or returning before a perceptual search. Which shortcut applies depends on the input as much as the method.

Measured 2026-10-03 on AMD Ryzen 7 9800X3D 8-Core Processor, using Node v26.10.0, Bun 1.4.2 and rustc 1.99.0 (b940084d7 2026-09-28). Coverage: **15 methods in Node, 15 methods in Bun, 16 methods in Rust f64, 16 methods in Rust f32**, across 3 gamuts. All times are **nanoseconds per color**, including encoded RGB output.

## Key findings

### 1. Dualray Fast earns its lead below the cusp

On shuffled P3 input, Fast cuts Dualray's time by **26.6%–39.0% across these runtimes**. Its shortcut
fits the lower boundary and the two nonzero RGB channels directly from hue,
then scales them by lightness cubed. That replaces trigonometry, channel-cubic
construction and root refinement with a small polynomial evaluation.

The difference is visible when the workload is split: Node Fast takes
**34.4 ns below the cusp and 100.1 ns above it**;
Rust f64 takes **24.9 and 76.6 ns**. The fresh Node
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
| Node | 26.6% | 10.2% more time |
| Bun | 32.9% | 1.9% more time |
| Rust f64 | 39.0% | 5.5% less time |
| Rust f32 | 38.7% | 6.1% less time |

This matters because 74.7% of the standard P3 random corpus lies
below the cusp. A bright-skewed workload loses much of the shortcut's benefit.
Fast's approximation buys a particularly cheap lower face, rather than a
uniform reduction in the cost of every Dualray path.

The `dualray fast (tables)` row reduces the above-cusp cost itself. A
generated 360-byte table per gamut corrects the upper solve's seed, so one
Householder step usually converges; Rust also takes the hue direction from a
22.5° table instead of `sin_cos`, while JavaScript keeps `Math.sin` and
`Math.cos`. Above the cusp it takes **62.3 ns in Rust f64 and
49.6 in f32**, against Fast's 76.6 and
61.0, and **92.0 ns in Node and
85.7 in Bun**, against 100.1 and
91.2. Below the cusp the two rows run the same code, so the P3
random saving is **3.4%–10.0% across runtimes**. Rust's larger gain includes its
direction table.
([Implementation](src/dualray-fast.js))

### 2. Caches win by removing whole phases of work

On P3 random inputs, caching speeds cubic up by 2.83–3.89× across runtimes, and
Bottosson by 1.72–2.26× across runtimes. Cubic stores the hue-only coefficients,
lower root and turning points; Bottosson stores its cusp and LMS direction
slopes. After warmup, both skip hue trig, and cached Bottosson also skips cusp
construction and its cube root. The current counters put uncached cubic at
**5.04 cbrt calls/color**, versus **0.56** cached.

Repeated integer hues can favor caches enough to change the ranking. The lowest mapped
P3 grid medians are bottosson-lightness (cached) in Node (44.5 ns) and dualray fast (tables) in Bun (40.9 ns). The price is persistent state—366/183 KiB
for cubic f64/f32 and 141/70 KiB for Bottosson—and 0.1° hue buckets. Cubic's two
rows use the same bucket semantics; cached Bottosson changes fractional-hue
evaluation compared with its uncached counterpart. Choose caching when that
memory and numerical trade-off fit the application.
([Cubic](src/oklch-cubic.js), [Bottosson](src/bottosson-factory.js))

### 3. A mapper can beat clipping by doing less conversion

Below the cusp, Rust f64 Fast takes **24.9 ns** against clip's
**40.6 ns**. Clipping must first convert the original OKLCh color:
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

CSS MINDE takes **476.1 ns in Node**, **615.5 in Bun** and
**587.5 in Rust f64** on P3 random input. Each clipping-error comparison
converts clipped RGB back to Oklab using three cube roots. The current Node
counter run records **32.8 cbrt calls/color**, or **10.9
error comparisons**, on average.

The size of the Node/Bun gap is consistent with the cube-root difference.
The Node 26.10.0/Bun 1.4.2 operation probes supplied with the review measured
about 4.3 versus 8.7 ns for cbrt throughput; earlier investigation identified
V8's own implementation versus glibc on this host. Multiplying the current
**32.8 calls by that 4.4 ns difference gives about 144.4
ns**, close to the measured **139.4 ns** gap. This is a consistency
check on the engine comparison, rather than a profile assigning parts of the
mapper's total time. CSS MINDE is the strongest current case for investigating
a faster native cube root.

Raytrace is different in these measurements. Its **9.0** cube
roots would suggest about **39.6 ns** by the same arithmetic,
but the Node/Bun gap is only **3.6 ns**; it takes
**237.2 ns in Node, 240.8 ns in Bun, 218.2 ns in Rust f64**. Node is slower than Rust f64 here. The isolated operation
probe does not predict the full Raytrace gap; its dependency chains, argument
distribution and surrounding work need separate profiling. The old cube-root
story is not enough to explain today's Raytrace timings.
([CSS MINDE](src/css-minde.js), [Raytrace](src/raytrace.js))

### 5. Polynomial encoding buys more in f64

Replacing Fast's ordinary output power with the polynomial reduces its Rust
P3 random time from **40.4 to 32.2 ns in f64**, saving
**8.3 ns**. In f32 it moves from **32.2 to 29.8
ns**, saving **2.5 ns**. The boundary mapper is the same in each
pair, so this directly measures the benefit of the encoder variant.

The polynomial uses the same degree in both precisions. It does not spend
extra terms recovering binary64 accuracy, while the ordinary encoder uses
each lane's native power operation. The interior workload gives a second
piece of evidence: across the other canonical-checking methods, median time
is **30.5 ns in f32 versus 47.3 in f64**.
Those paths return before solving a boundary, directly showing the cheaper
f32 conversion-and-encoding path. Together, these results support a smaller
cost to replace in f32, although they do not isolate the power call itself.

Mostly interior traffic also removes the polynomial encoder's opportunity:
every Fast row deliberately keeps ordinary encoding for canonical in-gamut
output. Use the mapped fraction, precision and accepted error budget to decide
whether the extra approximation pays.
([Rust encoder](rust/src/dualray_fast.rs))

### 6. Edge Seeker's index addresses unpredictable lookup work

In Rust f32, plain Edge Seeker goes from **35.5 ns on the grid to
78.7 ns on random hues**, a **2.21×** increase. The indexed
version moves from **30.4 to 43.8 ns**, or
**1.44×**. Both variants evaluate the same interpolated boundary at
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

On P3 random input, Halley and Ostrowski take **119.5/119.6
ns in Node** and **107.9/109.9 ns in Rust f64**. Halley
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
but uncached cubic takes **227.3 ns versus 221.2 in f64**.
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

On the all-interior P3 workload, Node CSS MINDE falls to **62.2
ns**, from **475.0 ns** on identical-lightness/hue out-of-gamut
input. Its cube-root count falls to **0.0**: the canonical
conversion returns before the clipping-error search. Node Fast instead takes
**68.2 ns inside versus 57.5 ns outside**,
because its cheap mapped lower path can skip the conversion that pass-through
must preserve.

Most canonical-checking methods consequently converge toward conversion cost
on interior input. For example, checked Halley takes **47.5
ns in f64 and 30.4 ns in f32** there. Dualray retains its
first-exit policy and still does boundary work, taking **68.1
ns in f64**. For mostly in-gamut applications, the pass-through path and the
cost of a failed precheck deserve as much attention as the boundary solver.
The 50% mixture in the appendix shows the transition between those workloads.

## Comparing runtimes

Node and Bun are near parity in the median across shared methods, but that
overall similarity hides useful differences. Bun's cached cubic takes **79.1 ns
against Node's 92.7**, and Bun Fast takes **51.8 against
57.9 ns**. Node wins CSS MINDE, where its cube-root implementation
has much more work to influence. The median JS/Rust f64 ratios are around
1.2× on this workload; the individual method and math-library path matter more
than a blanket language multiplier.

| P3 random ratio | Median across shared methods | Range across methods |
| --- | ---: | ---: |
| Node / Bun | 1.02× | 0.77–1.17× |
| Node / Rust f64 | 1.20× | 0.81–1.74× |
| Bun / Rust f64 | 1.16× | 1.04–1.63× |

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
| Node | 1.00× | 1.03× | 1.01× | 1.06× |
| Bun | 1.02× | 1.06× | 1.01× | 1.07× |
| Rust f64 | 1.00× | 1.02× | 1.00× | 1.07× |
| Rust f32 | 1.00× | 0.99× | 1.00× | 1.03× |

Each entry is the median of the individual method's target/P3 time ratios,
including clip and every method available in that runtime. Below one means
less time than P3. These are the same input triples in the same order for all
three targets, so differences come from processing them for a different gamut.

**Dualray's performance carries across targets.** On random input, the largest
change from P3 across Dualray and Dualray Fast, all targets and all four runtimes,
is 5.8%. A Dualray Fast row is the fastest mapped method on
every target: `dualray fast (tables)` in Node, Bun and Rust f32 and `dualray fast (poly encode)` in Rust f64. Choosing sRGB or Rec.2020
does not erase the lower-face shortcut's advantage on this distribution.

**The iterative solvers and cached cubic are more sensitive.** Bun Halley takes
137.8 ns for sRGB, 111.8 for P3 and
146.0 for Rec.2020 on random input. Halley and Ostrowski's
Rec.2020 penalties in Bun are 29.5%–30.6% there and
38.8%–41.3% on the grid. Cached cubic also costs more
for Rec.2020 in every runtime: 2.3%–14.1% on random input
and 9.4%–26.8% on the grid. P3 alone understates these costs.

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
Rec.2020 takes 5.9%–12.4% more time than P3 on random input in
Node, Bun and Rust f64. Rust f32 shows no penalty, at 35.4 ns
for Rec.2020 versus 36.1 for P3. Extra power
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
| Out-of-gamut, mostly above the cusp | In Rust, Dualray Fast (tables); in JavaScript, compare Dualray and the Fast rows on that distribution | In Rust the tables row takes 23.1%–23.7% less time than Dualray here. The JavaScript methods are within 10.2% of Dualray; Fast's large lower-face advantage does not carry over. Cached Bottosson and indexed Edge Seeker are also candidates when their boundary policies fit. |

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
| clip | 53.9 | 55.2 | 43.2 | 36.1 |
| css-minde | 476.1 | 615.5 | 587.5 | 393.2 |
| oklch-cubic (cached) | 92.7 | 79.1 | 70.4 | 58.4 |
| oklch-cubic (no cache) | 262.1 | 254.6 | 221.2 | 227.3 |
| oklch-cubic-direct | 231.6 | 240.2 | 216.8 | 174.9 |
| oklch-halley | 119.5 | 111.8 | 107.9 | 98.9 |
| oklch-ostrowski | 119.6 | 116.7 | 109.9 | 94.7 |
| dualray | 78.8 | 77.2 | 66.3 | 52.6 |
| dualray fast | 57.9 | 51.8 | 40.4 | 32.2 |
| dualray fast (poly encode) | — | — | 32.2 | 29.8 |
| dualray fast (tables) | 55.5 | 50.0 | 37.0 | 29.0 |
| bottosson-lightness | 119.6 | 120.9 | 99.8 | 79.0 |
| bottosson-lightness (cached) | 66.5 | 70.4 | 44.1 | 37.6 |
| edge-seeker | 154.3 | 144.6 | 88.7 | 78.7 |
| edge-seeker (indexed) | 76.8 | 75.8 | 55.8 | 43.8 |
| raytrace | 237.2 | 240.8 | 218.2 | 161.4 |

## Where the next experiments would pay

For Fast, measure whether sharing trig between a failed canonical precheck and the upper solve removes useful work in both JS and Rust. Any reuse must preserve authored-hue conversion and in-gamut output bits. CSS MINDE provides the clearest current evidence for testing a faster native cube root; Raytrace needs a separate dependency-chain profile because the same throughput calculation does not explain its measured gap. For uncached f32 cubic, profile candidate validation before changing arithmetic. On mostly interior input, measure conversion and membership handling first.

Memory-focused experiments have different goals: a u16 Rust Edge Seeker index could reduce its extra payload from 28 to 7 KiB; a seven-scalar cubic cache would exchange coefficient reconstruction for less storage. Earlier prototypes and the cube-expression/cache-layout history are preserved in [the notes](PERFORMANCE-NOTES.md). Each experiment needs its own output/accuracy checks and end-to-end timing.

## Measurement and reproduction

Jobs ran serially with logical CPU affinity `2,3`, one method/target/runtime per process, under linux 6.18.33.2-microsoft-standard-WSL2 and ldd (Ubuntu GLIBC 2.43-2ubuntu2.4) 2.43. Rust used `-C target-cpu=native`, opt-level=3, lto=true, codegen-units=1, panic=abort. f32 samples were rounded before timing; output channels were individually widened for the checksum. CPU frequency was not fixed.

Each process ran 50 complete warmup passes and 25 timed passes; the cell order rotated and reversed between rounds. All workloads contain 35,640 colors. Rust used input and checksum optimization barriers. Factories, table/index construction and initial cache filling preceded the measured passes.

There are **682 measured cells** and **2046 fresh timing processes**. The median process range, (maximum−minimum)/median, is **1.0%**; **6 cells** span more than 10%. The ranges are repeatability diagnostics, not confidence intervals.

Bun Fast has **1/11 cells** with a process range above 5%. The earlier single-CPU run showed multiple timing modes, motivating the current affinity setting; the [affinity history](PERFORMANCE-NOTES.md#affinity-experiment) records that investigation.

Every cell was validated in a separate process before timing. All outputs had to be finite and in range, and timed checksums had to agree with the validation sum over 75 passes. The regression suites and report-harness checks passed: **126 Rust tests**, **107 Bun tests**, **96 Node numerical tests**, **3 Node CLI tests**, **8 Node harness tests**, **8 Bun harness tests**. [Commands and complete validation logs](reports/performance-2026-10-03-validation.json) accompany the timing artifact.

The [Math-call profiler](scripts/profile-performance-math.mjs) replayed each P3 workload after warming caches, delegated every counted call to the original Math function, and required exact agreement with the Node validation checksum. A separate pass instruments Fast's precheck, lower, upper and exact-search entries in an in-memory source copy, checking every output channel against production. Source replacement markers must match exactly. Gamma powers and hardware branch misses were not counted. The artifact is bound to the timing artifact's hash.

The measured algorithm baseline is `afec3bb55ad9e704123c01bf7d0cf11c38ae8447`. The artifact records input, source and Rust-binary hashes. Resume verifies source hashes, binary hash, runtime versions, CPU model and affinity.

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
| clip | 40.1 | 40.6 | 30.2 | 20.2 |
| css-minde | 422.2 | 572.9 | 541.2 | 357.3 |
| oklch-cubic (cached) | 65.1 | 59.1 | 49.5 | 42.4 |
| oklch-cubic (no cache) | 237.5 | 234.4 | 197.1 | 210.4 |
| oklch-cubic-direct | 205.3 | 212.1 | 193.0 | 157.3 |
| oklch-halley | 97.3 | 91.7 | 94.6 | 84.5 |
| oklch-ostrowski | 96.2 | 94.8 | 93.2 | 78.1 |
| dualray | 63.4 | 61.6 | 52.5 | 40.1 |
| dualray fast | 47.8 | 42.5 | 30.4 | 23.7 |
| dualray fast (poly encode) | — | — | 22.0 | 21.9 |
| dualray fast (tables) | 45.8 | 40.9 | 27.6 | 20.9 |
| bottosson-lightness | 101.0 | 105.4 | 87.9 | 68.9 |
| bottosson-lightness (cached) | 44.5 | 49.3 | 30.3 | 24.0 |
| edge-seeker | 89.4 | 98.8 | 47.4 | 35.5 |
| edge-seeker (indexed) | 58.4 | 59.9 | 40.8 | 30.4 |
| raytrace | 213.1 | 215.8 | 201.7 | 142.7 |

### sRGB: ordered integer hues

| Method | Node | Bun | Rust f64 | Rust f32 |
| --- | ---: | ---: | ---: | ---: |
| clip | 39.2 | 43.1 | 29.5 | 19.7 |
| css-minde | 425.2 | 584.0 | 540.3 | 356.0 |
| oklch-cubic (cached) | 71.2 | 62.6 | 49.8 | 42.8 |
| oklch-cubic (no cache) | 242.4 | 239.0 | 198.8 | 211.5 |
| oklch-cubic-direct | 211.8 | 219.1 | 192.5 | 156.4 |
| oklch-halley | 107.1 | 118.4 | 95.8 | 87.9 |
| oklch-ostrowski | 103.4 | 116.7 | 89.3 | 82.1 |
| dualray | 63.6 | 60.0 | 53.2 | 40.8 |
| dualray fast | 47.6 | 42.0 | 30.0 | 23.5 |
| dualray fast (poly encode) | — | — | 21.8 | 21.6 |
| dualray fast (tables) | 44.9 | 40.3 | 27.5 | 20.7 |
| bottosson-lightness | 100.1 | 106.4 | 87.9 | 71.0 |
| bottosson-lightness (cached) | 42.2 | 44.2 | 30.6 | 26.0 |
| edge-seeker | 89.3 | 98.0 | 48.4 | 34.6 |
| edge-seeker (indexed) | 58.8 | 59.6 | 40.9 | 30.6 |
| raytrace | 215.9 | 218.8 | 200.3 | 143.1 |

### sRGB: shuffled fractional hues

| Method | Node | Bun | Rust f64 | Rust f32 |
| --- | ---: | ---: | ---: | ---: |
| clip | 53.2 | 57.6 | 43.2 | 36.5 |
| css-minde | 477.9 | 631.5 | 600.6 | 394.5 |
| oklch-cubic (cached) | 96.4 | 82.4 | 71.1 | 58.4 |
| oklch-cubic (no cache) | 267.7 | 260.0 | 222.3 | 228.3 |
| oklch-cubic-direct | 235.5 | 252.1 | 217.8 | 175.1 |
| oklch-halley | 127.7 | 137.8 | 111.1 | 101.8 |
| oklch-ostrowski | 126.3 | 137.3 | 106.7 | 96.1 |
| dualray | 79.3 | 74.7 | 67.2 | 51.2 |
| dualray fast | 57.6 | 52.1 | 40.1 | 31.5 |
| dualray fast (poly encode) | — | — | 32.0 | 29.6 |
| dualray fast (tables) | 55.0 | 50.3 | 37.2 | 28.8 |
| bottosson-lightness | 119.4 | 125.4 | 101.2 | 82.6 |
| bottosson-lightness (cached) | 65.1 | 65.8 | 44.8 | 38.7 |
| edge-seeker | 154.3 | 144.5 | 88.4 | 76.6 |
| edge-seeker (indexed) | 76.9 | 75.3 | 55.8 | 44.1 |
| raytrace | 234.9 | 242.3 | 215.3 | 162.9 |

### Rec.2020: ordered integer hues

| Method | Node | Bun | Rust f64 | Rust f32 |
| --- | ---: | ---: | ---: | ---: |
| clip | 42.8 | 47.4 | 32.9 | 20.6 |
| css-minde | 421.8 | 574.6 | 535.1 | 351.2 |
| oklch-cubic (cached) | 82.6 | 72.4 | 58.7 | 46.4 |
| oklch-cubic (no cache) | 246.4 | 243.9 | 204.0 | 215.0 |
| oklch-cubic-direct | 222.2 | 233.3 | 197.6 | 158.3 |
| oklch-halley | 116.1 | 129.6 | 101.5 | 92.9 |
| oklch-ostrowski | 118.1 | 131.5 | 102.0 | 85.8 |
| dualray | 67.3 | 64.1 | 57.0 | 42.5 |
| dualray fast | 49.9 | 44.9 | 32.3 | 24.4 |
| dualray fast (poly encode) | — | — | 22.7 | 22.6 |
| dualray fast (tables) | 47.6 | 43.9 | 29.6 | 21.5 |
| bottosson-lightness | 106.4 | 112.3 | 92.7 | 74.1 |
| bottosson-lightness (cached) | 49.4 | 48.2 | 35.3 | 27.4 |
| edge-seeker | 93.9 | 105.3 | 51.6 | 35.5 |
| edge-seeker (indexed) | 61.9 | 63.0 | 43.7 | 31.3 |
| raytrace | 218.2 | 224.4 | 202.8 | 143.0 |

### Rec.2020: shuffled fractional hues

| Method | Node | Bun | Rust f64 | Rust f32 |
| --- | ---: | ---: | ---: | ---: |
| clip | 57.1 | 62.1 | 45.9 | 35.4 |
| css-minde | 470.5 | 618.7 | 577.2 | 387.7 |
| oklch-cubic (cached) | 105.7 | 89.2 | 76.8 | 59.7 |
| oklch-cubic (no cache) | 266.7 | 262.3 | 225.2 | 226.3 |
| oklch-cubic-direct | 240.1 | 254.0 | 219.7 | 175.1 |
| oklch-halley | 136.7 | 146.0 | 113.8 | 102.7 |
| oklch-ostrowski | 140.0 | 151.1 | 116.8 | 98.2 |
| dualray | 79.2 | 75.4 | 68.8 | 50.5 |
| dualray fast | 58.5 | 54.8 | 41.4 | 31.0 |
| dualray fast (poly encode) | — | — | 31.9 | 29.6 |
| dualray fast (tables) | 56.5 | 52.8 | 38.3 | 28.8 |
| bottosson-lightness | 122.8 | 129.5 | 104.4 | 82.4 |
| bottosson-lightness (cached) | 69.1 | 61.2 | 43.8 | 34.1 |
| edge-seeker | 156.6 | 148.3 | 88.6 | 76.3 |
| edge-seeker (indexed) | 80.0 | 81.3 | 56.5 | 43.3 |
| raytrace | 236.0 | 244.7 | 214.9 | 159.6 |

## Appendix B: cusp-side and membership workloads

### Below the P3 cusp

| Method | Node | Bun | Rust f64 | Rust f32 |
| --- | ---: | ---: | ---: | ---: |
| clip | 50.1 | 52.7 | 40.6 | 32.1 |
| css-minde | 473.3 | 618.6 | 585.4 | 393.1 |
| oklch-cubic (cached) | 72.0 | 60.9 | 54.0 | 39.8 |
| oklch-cubic (no cache) | 244.6 | 237.1 | 204.6 | 210.8 |
| oklch-cubic-direct | 220.6 | 233.9 | 210.5 | 170.3 |
| oklch-halley | 112.7 | 108.1 | 106.4 | 97.8 |
| oklch-ostrowski | 114.7 | 111.1 | 108.1 | 94.3 |
| dualray | 72.2 | 69.4 | 58.1 | 46.1 |
| dualray fast | 34.4 | 31.8 | 24.9 | 18.8 |
| dualray fast (poly encode) | — | — | 17.7 | 17.0 |
| dualray fast (tables) | 35.2 | 32.0 | 24.8 | 18.6 |
| bottosson-lightness | 110.6 | 111.7 | 93.0 | 72.8 |
| bottosson-lightness (cached) | 55.7 | 57.6 | 36.9 | 31.7 |
| edge-seeker | 147.1 | 135.8 | 81.6 | 73.4 |
| edge-seeker (indexed) | 68.2 | 69.1 | 47.7 | 38.6 |
| raytrace | 227.4 | 233.4 | 211.6 | 159.4 |

### Above the P3 cusp

| Method | Node | Bun | Rust f64 | Rust f32 |
| --- | ---: | ---: | ---: | ---: |
| clip | 53.9 | 54.2 | 40.8 | 37.4 |
| css-minde | 467.5 | 600.8 | 580.4 | 376.8 |
| oklch-cubic (cached) | 139.1 | 120.2 | 100.8 | 101.9 |
| oklch-cubic (no cache) | 304.7 | 297.8 | 245.1 | 266.7 |
| oklch-cubic-direct | 224.0 | 226.9 | 210.7 | 175.9 |
| oklch-halley | 123.3 | 114.5 | 108.2 | 106.5 |
| oklch-ostrowski | 117.0 | 112.0 | 101.8 | 91.6 |
| dualray | 90.9 | 89.5 | 81.0 | 65.0 |
| dualray fast | 100.1 | 91.2 | 76.6 | 61.0 |
| dualray fast (poly encode) | — | — | 66.2 | 58.8 |
| dualray fast (tables) | 92.0 | 85.7 | 62.3 | 49.6 |
| bottosson-lightness | 135.7 | 136.6 | 112.6 | 93.7 |
| bottosson-lightness (cached) | 87.8 | 94.3 | 55.6 | 47.3 |
| edge-seeker | 161.6 | 157.1 | 93.4 | 81.0 |
| edge-seeker (indexed) | 83.7 | 82.1 | 61.6 | 48.0 |
| raytrace | 236.3 | 239.2 | 214.9 | 156.9 |

### P3 random, checked entry

| Method | Node | Bun | Rust f64 | Rust f32 |
| --- | ---: | ---: | ---: | ---: |
| clip | 53.9 | 55.7 | 43.3 | 35.9 |
| css-minde | 475.0 | 618.6 | 585.4 | 395.8 |
| oklch-cubic (cached) | 118.6 | 102.4 | 87.0 | 75.3 |
| oklch-cubic (no cache) | 285.3 | 272.8 | 237.8 | 247.8 |
| oklch-cubic-direct | 245.5 | 257.7 | 220.3 | 182.1 |
| oklch-halley | 122.3 | 134.7 | 111.9 | 104.9 |
| oklch-ostrowski | 124.2 | 140.6 | 113.6 | 99.8 |
| dualray | 79.2 | 76.1 | 66.2 | 52.5 |
| dualray fast | 57.5 | 52.1 | 40.4 | 31.9 |
| dualray fast (poly encode) | — | — | 31.9 | 29.8 |
| dualray fast (tables) | 56.6 | 50.1 | 37.1 | 29.1 |
| bottosson-lightness | 123.9 | 128.8 | 101.9 | 86.5 |
| bottosson-lightness (cached) | 96.1 | 89.3 | 60.2 | 54.1 |
| edge-seeker | 166.2 | 158.5 | 90.9 | 88.6 |
| edge-seeker (indexed) | 105.0 | 97.2 | 68.5 | 61.2 |
| raytrace | 238.4 | 245.5 | 219.5 | 166.4 |

### P3 50% in gamut, checked entry

| Method | Node | Bun | Rust f64 | Rust f32 |
| --- | ---: | ---: | ---: | ---: |
| clip | 57.7 | 58.7 | 45.6 | 31.5 |
| css-minde | 268.4 | 345.3 | 317.8 | 212.4 |
| oklch-cubic (cached) | 90.4 | 84.5 | 68.0 | 54.7 |
| oklch-cubic (no cache) | 179.7 | 171.4 | 143.6 | 139.0 |
| oklch-cubic-direct | 155.7 | 164.2 | 133.9 | 106.7 |
| oklch-halley | 92.8 | 99.6 | 80.2 | 67.4 |
| oklch-ostrowski | 94.3 | 103.0 | 81.1 | 64.9 |
| dualray | 81.7 | 79.9 | 68.9 | 52.8 |
| dualray fast | 67.2 | 62.0 | 46.8 | 34.7 |
| dualray fast (poly encode) | — | — | 43.5 | 33.0 |
| dualray fast (tables) | 66.1 | 60.8 | 45.3 | 32.8 |
| bottosson-lightness | 93.6 | 96.1 | 75.4 | 58.3 |
| bottosson-lightness (cached) | 82.2 | 76.7 | 54.6 | 43.8 |
| edge-seeker | 111.8 | 108.6 | 68.8 | 59.6 |
| edge-seeker (indexed) | 82.2 | 79.5 | 58.1 | 46.9 |
| raytrace | 150.8 | 154.3 | 133.6 | 98.4 |

### P3 100% in gamut, checked entry

| Method | Node | Bun | Rust f64 | Rust f32 |
| --- | ---: | ---: | ---: | ---: |
| clip | 61.7 | 62.3 | 46.5 | 27.6 |
| css-minde | 62.2 | 58.1 | 48.7 | 31.2 |
| oklch-cubic (cached) | 62.2 | 58.9 | 47.1 | 33.8 |
| oklch-cubic (no cache) | 61.8 | 60.0 | 47.7 | 30.1 |
| oklch-cubic-direct | 61.7 | 59.1 | 47.1 | 30.5 |
| oklch-halley | 61.6 | 58.6 | 47.5 | 30.4 |
| oklch-ostrowski | 62.8 | 58.8 | 47.3 | 30.4 |
| dualray | 82.8 | 80.8 | 68.1 | 51.7 |
| dualray fast | 68.2 | 64.7 | 53.4 | 36.2 |
| dualray fast (poly encode) | — | — | 53.8 | 36.1 |
| dualray fast (tables) | 68.0 | 65.7 | 52.9 | 36.2 |
| bottosson-lightness | 62.3 | 58.7 | 48.5 | 30.3 |
| bottosson-lightness (cached) | 61.1 | 59.0 | 48.6 | 33.0 |
| edge-seeker | 61.8 | 58.5 | 47.2 | 32.8 |
| edge-seeker (indexed) | 61.7 | 58.9 | 47.2 | 32.8 |
| raytrace | 61.6 | 58.4 | 46.6 | 30.3 |

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
| srgb-random | Rust f64 | oklch-cubic (cached) | 71.1 | 70.9–81.3 | 14.6% |
| display-p3-below-cusp | Bun | dualray fast (tables) | 32.0 | 31.8–35.4 | 11.4% |
| display-p3-random-checked | Node | edge-seeker (indexed) | 105.0 | 95.6–107.2 | 11.0% |
| display-p3-grid | Node | dualray fast (tables) | 45.8 | 45.7–50.6 | 10.6% |
| rec2020-random | Bun | bottosson-lightness (cached) | 61.2 | 61.0–67.4 | 10.4% |
| display-p3-inside-checked | Rust f64 | oklch-cubic (no cache) | 47.7 | 47.6–52.4 | 10.1% |
| rec2020-random | Node | raytrace | 236.0 | 234.0–255.8 | 9.2% |
| rec2020-grid | Rust f32 | oklch-cubic-direct | 158.3 | 158.1–171.3 | 8.4% |
