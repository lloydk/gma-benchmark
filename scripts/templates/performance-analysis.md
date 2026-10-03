## Key findings

### 1. Dualray Fast earns its lead below the cusp

On shuffled P3 input, Fast cuts Dualray's time by {{fast_savings}}. Its shortcut
fits the lower boundary and the two nonzero RGB channels directly from hue,
then scales them by lightness cubed. That replaces trigonometry, channel-cubic
construction and root refinement with a small polynomial evaluation.

The difference is visible when the workload is split: Node Fast takes
**{{fast_node_below}} ns below the cusp and {{fast_node_above}} ns above it**;
Rust f64 takes **{{fast_f64_below}} and {{fast_f64_above}} ns**. The fresh Node
counts show no sine or cosine calls on the entire below-cusp workload, versus
{{fast_above_trig}} of each per color above it. The branch probe explains the
extra calls: every above-cusp color enters the upper solve, none enters exact
recovery, and **{{fast_precheck_share}} first run the canonical precheck**.
Those colors fall below the fitted lower-boundary ratio plus its margin, so
the mapper converts them to test membership before the upper solve computes
the same hue trig again. The [cusp-side counts](#cusp-side-operation-and-path-counts)
show both the operations and their source paths.

{{fast_comparison_table}}

This matters because {{below_share}} of the standard P3 random corpus lies
below the cusp. A bright-skewed workload loses much of the shortcut's benefit.
Fast's approximation buys a particularly cheap lower face, rather than a
uniform reduction in the cost of every Dualray path.
([Implementation](src/dualray-fast.js))

### 2. Caches win by removing whole phases of work

On P3 random inputs, caching speeds cubic up by {{cubic_cache_ratios}}, and
Bottosson by {{bottosson_cache_ratios}}. Cubic stores the hue-only coefficients,
lower root and turning points; Bottosson stores its cusp and LMS direction
slopes. After warmup, both skip hue trig, and cached Bottosson also skips cusp
construction and its cube root. The current counters put uncached cubic at
**{{cubic_uncached_cbrt}} cbrt calls/color**, versus **{{cubic_cached_cbrt}}** cached.

Repeated integer hues can favor caches enough to change the ranking. The lowest mapped
P3 grid medians are {{grid_winners}}. The price is persistent state—366/183 KiB
for cubic f64/f32 and 141/70 KiB for Bottosson—and 0.1° hue buckets. Cubic's two
rows use the same bucket semantics; cached Bottosson changes fractional-hue
evaluation compared with its uncached counterpart. Choose caching when that
memory and numerical trade-off fit the application.
([Cubic](src/oklch-cubic.js), [Bottosson](src/bottosson-factory.js))

### 3. A mapper can beat clipping by doing less conversion

Below the cusp, Rust f64 Fast takes **{{fast_f64_below}} ns** against clip's
**{{clip_f64_below}} ns**. Clipping must first convert the original OKLCh color:
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

CSS MINDE takes **{{minde_node}} ns in Node**, **{{minde_bun}} in Bun** and
**{{minde_f64}} in Rust f64** on P3 random input. Each clipping-error comparison
converts clipped RGB back to Oklab using three cube roots. The current Node
counter run records **{{minde_cbrt}} cbrt calls/color**, or **{{minde_errors}}
error comparisons**, on average.

The size of the Node/Bun gap is consistent with the cube-root difference.
The Node 26.10.0/Bun 1.4.2 operation probes supplied with the review measured
about 4.3 versus 8.7 ns for cbrt throughput; earlier investigation identified
V8's own implementation versus glibc on this host. Multiplying the current
**{{minde_cbrt}} calls by that 4.4 ns difference gives about {{minde_cbrt_gap}}
ns**, close to the measured **{{minde_engine_gap}} ns** gap. This is a consistency
check on the engine comparison, rather than a profile assigning parts of the
mapper's total time. CSS MINDE is the strongest current case for investigating
a faster native cube root.

Raytrace is different in these measurements. Its **{{raytrace_cbrt}}** cube
roots would suggest about **{{raytrace_cbrt_gap}} ns** by the same arithmetic,
but the Node/Bun gap is only **{{raytrace_engine_gap}} ns**; it takes
**{{raytrace_times}}**. Node is slower than Rust f64 here. The isolated operation
probe does not predict the full Raytrace gap; its dependency chains, argument
distribution and surrounding work need separate profiling. The old cube-root
story is not enough to explain today's Raytrace timings.
([CSS MINDE](src/css-minde.js), [Raytrace](src/raytrace.js))

### 5. Polynomial encoding buys more in f64

Replacing Fast's ordinary output power with the polynomial reduces its Rust
P3 random time from **{{fast_f64}} to {{poly_f64}} ns in f64**, saving
**{{poly_saving_f64}} ns**. In f32 it moves from **{{fast_f32}} to {{poly_f32}}
ns**, saving **{{poly_saving_f32}} ns**. The boundary mapper is the same in each
pair, so this directly measures the benefit of the encoder variant.

The polynomial uses the same degree in both precisions. It does not spend
extra terms recovering binary64 accuracy, while the ordinary encoder uses
each lane's native power operation. The interior workload gives a second
piece of evidence: across the other canonical-checking methods, median time
is **{{interior_median_f32}} ns in f32 versus {{interior_median_f64}} in f64**.
Those paths return before solving a boundary, directly showing the cheaper
f32 conversion-and-encoding path. Together, these results support a smaller
cost to replace in f32, although they do not isolate the power call itself.

Mostly interior traffic also removes the polynomial encoder's opportunity:
both Fast variants deliberately keep ordinary encoding for canonical in-gamut
output. Use the mapped fraction, precision and accepted error budget to decide
whether the extra approximation pays.
([Rust encoder](rust/src/dualray_fast.rs))

### 6. Edge Seeker's index addresses unpredictable lookup work

In Rust f32, plain Edge Seeker goes from **{{edge_grid}} ns on the grid to
{{edge_random}} ns on random hues**, a **{{edge_ratio}}×** increase. The indexed
version moves from **{{index_grid}} to {{index_random}} ns**, or
**{{index_ratio}}×**. Both variants evaluate the same interpolated boundary at
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

On P3 random input, Halley and Ostrowski take **{{halley_node}}/{{ostrowski_node}}
ns in Node** and **{{halley_f64}}/{{ostrowski_f64}} ns in Rust f64**. Halley
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

Rust f32 has a lower P3 random median for **{{f32_wins}}/{{rust_methods}} methods**,
but uncached cubic takes **{{uncached_f32}} ns versus {{uncached_f64}} in f64**.
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

On the all-interior P3 workload, Node CSS MINDE falls to **{{minde_inside_node}}
ns**, from **{{minde_checked_node}} ns** on identical-lightness/hue out-of-gamut
input. Its cube-root count falls to **{{minde_inside_cbrt}}**: the canonical
conversion returns before the clipping-error search. Node Fast instead takes
**{{fast_inside_node}} ns inside versus {{fast_checked_node}} ns outside**,
because its cheap mapped lower path can skip the conversion that pass-through
must preserve.

Most canonical-checking methods consequently converge toward conversion cost
on interior input. For example, checked Halley takes **{{halley_inside_f64}}
ns in f64 and {{halley_inside_f32}} ns in f32** there. Dualray retains its
first-exit policy and still does boundary work, taking **{{dualray_inside_f64}}
ns in f64**. For mostly in-gamut applications, the pass-through path and the
cost of a failed precheck deserve as much attention as the boundary solver.
The 50% mixture in the appendix shows the transition between those workloads.

## Comparing runtimes

Node and Bun are near parity in the median across shared methods, but that
overall similarity hides useful differences. Bun's cached cubic takes **{{cubic_bun}} ns
against Node's {{cubic_node}}**, and Bun Fast takes **{{fast_bun}} against
{{fast_node}} ns**. Node wins CSS MINDE, where its cube-root implementation
has much more work to influence. The median JS/Rust f64 ratios are around
1.2× on this workload; the individual method and math-library path matter more
than a blanket language multiplier.

{{runtime_comparison_table}}

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

{{gamut_comparison_table}}

Each entry is the median of the individual method's target/P3 time ratios,
including clip and every method available in that runtime. Below one means
less time than P3. These are the same input triples in the same order for all
three targets, so differences come from processing them for a different gamut.

**Dualray's performance carries across targets.** On random input, the largest
change from P3 across Dualray and Dualray Fast, all targets and all four runtimes,
is {{dualray_gamut_change}}. Fast remains the fastest mapped JS method on each
target; Fast with poly encode leads both Rust lanes. Choosing sRGB or Rec.2020
does not erase the lower-face shortcut's advantage on this distribution.

**The iterative solvers and cached cubic are more sensitive.** Bun Halley takes
{{halley_bun_srgb}} ns for sRGB, {{halley_bun_display-p3}} for P3 and
{{halley_bun_rec2020}} for Rec.2020 on random input. Halley and Ostrowski's
Rec.2020 penalties in Bun are {{iterative_rec2020_random_penalty}} there and
{{iterative_rec2020_grid_penalty}} on the grid. Cached cubic also costs more
for Rec.2020 in every runtime: {{cubic_rec2020_random_penalty}} on random input
and {{cubic_rec2020_grid_penalty}} on the grid. P3 alone understates these costs.

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
Rec.2020 takes {{clip_rec2020_penalty}} more time than P3 on random input in
Node, Bun and Rust f64. The direction reverses in Rust f32, at
{{clip_rec2020_f32}} ns for Rec.2020 versus {{clip_p3_f32}} for P3. Extra power
evaluations, fewer branches and the runtime's math implementation can pull in
different directions; gamut width alone does not predict the result.
([JS transfer functions](src/rgb-spaces.js), [Rust transfer functions](rust/src/transfer.rs))

The comparison is almost entirely out of gamut: all sRGB/P3 inputs are outside,
while Rec.2020 contains {{rec2020_random_inside}} interior colors
({{rec2020_random_inside_share}}) on random input and {{rec2020_grid_inside}}
({{rec2020_grid_inside_share}}) on the grid. The cusp-side and mixed/interior
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
