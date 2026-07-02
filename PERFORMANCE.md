# Performance analysis

A deep dive into where each gamut-mapping method spends its time, how the two
sides of the gamut cusp differ, and how the three runtimes (Rust, Node/V8,
Bun/JavaScriptCore) compare. All numbers were measured on the same machine in
one session; treat them as relative indicators, subject to the caveats in
[README.md](README.md#caveats).

The JS numbers include the `x ** 3` → `x * x * x` fix in `convert.js` and the
oklch-cubic solvers (see §2) — V8 compiles `** 3` to a full pow call, and
removing it sped Node up 11–32% per method. Pre-fix Node numbers appear in §2
for reference.

**Environment**: AMD Ryzen 7 9800X3D, Linux 6.18 (WSL2), glibc 2.43,
Node v26.3.1 (V8), Bun 1.3.14 (JavaScriptCore), rustc 1.96.0
(`-C target-cpu=native`, LTO, `opt-level=3`).

**Methodology**: every number is a median of 25 interleaved passes over a
35,640-color workload (30 warmup passes), one call per color, reused output
vector. Four workloads, all at C = 0.4 (out of P3 gamut everywhere):

- **grid** — the canonical benchmark grid: integer hues 0–359 repeated at 99
  fixed lightness steps. Hue-repetitive, cache- and branch-predictor-friendly.
- **random** — stratified/jittered fractional hue and lightness, shuffled.
- **below-cusp** — stratified fractional hues; lightness drawn strictly below
  that hue's cusp lightness (margin 0.01).
- **above-cusp** — same, strictly above the cusp.

The cusp here is the P3 gamut cusp: for each hue, the lightness at which the
gamut reaches maximum chroma (computed with the Bottosson approximation, the
same one `bottosson-lightness` uses internally). Below/above should be compared
against *random* (they also use fractional hues), not against *grid*.

## 1. Headline numbers (ns/call)

### grid workload

| method                 | Rust  | Node  | Bun   | Node/Rust | Bun/Rust |
|------------------------|------:|------:|------:|----------:|---------:|
| clip                   |  18.0 |  42.6 |  42.4 |     2.4×  |    2.4×  |
| oklch-cubic (cached)   |  37.4 |  75.4 |  63.9 |     2.0×  |    1.7×  |
| oklch-cubic (no cache) | 177.8 | 238.3 | 228.2 |     1.3×  |    1.3×  |
| bottosson-lightness    |  71.9 | 109.4 | 100.8 |     1.5×  |    1.4×  |
| edge-seeker            |  31.8 | 101.9 |  98.4 |     3.2×  |    3.1×  |
| edge-seeker (indexed)  |  27.7 |  67.5 |  59.5 |     2.4×  |    2.1×  |
| raytrace               | 200.4 | 200.9 | 210.0 |     1.0×  |    1.0×  |

### random workload

| method                 | Rust  | Node  | Bun   | Node/Rust | Bun/Rust |
|------------------------|------:|------:|------:|----------:|---------:|
| clip                   |  28.8 |  58.1 |  57.9 |     2.0×  |    2.0×  |
| oklch-cubic (cached)   |  56.0 | 116.0 |  97.5 |     2.1×  |    1.7×  |
| oklch-cubic (no cache) | 202.4 | 267.7 | 251.2 |     1.3×  |    1.2×  |
| bottosson-lightness    |  83.2 | 127.7 | 117.1 |     1.5×  |    1.4×  |
| edge-seeker            |  75.3 | 157.9 | 148.7 |     2.1×  |    2.0×  |
| edge-seeker (indexed)  |  40.6 |  84.5 |  75.2 |     2.1×  |    1.9×  |
| raytrace               | 222.2 | 221.1 | 228.1 |     1.0×  |    1.0×  |

Two facts stand out and are explained in §2: **raytrace is the one method
where JavaScript matches native Rust**, and after the `** 3` fix **Node and
Bun are nearly at parity** (they weren't before).

## 2. The primitive costs that explain almost everything

Measured cost per operation (throughput, tight loop over varied inputs,
baseline subtracted):

| operation           | Node (V8) | Bun (JSC) | Rust (glibc) |
|---------------------|----------:|----------:|-------------:|
| multiply            |     ~0    |     ~0    |      ~0      |
| divide              |     0.35  |     0.36  |      0.10    |
| `sqrt`              |     1.05  |     1.05  |      1.12    |
| `cbrt`              |   **3.69**|   **7.81**|    **7.92**  |
| `x ** (1/2.4)` (γ)  |     7.22  |     6.45  |      5.99    |
| `x ** 2`            |     ~0    |     ~0    |      ~0      |
| `x ** 3`            |   **7.24**|   **0.29**|      ~0      |
| `sin`               |     7.12  |     6.47  |      5.98    |
| `cos`               |     7.11  |     6.43  |      6.06    |
| `sin` + `cos` pair  |    12.28  |    12.91  |     10.56    |
| `acos`              |     2.50  |     2.93  |      2.25    |

(Rust `sin_cos()` measured 10.66 ns — no faster than separate calls on glibc.)

Two runtime quirks dominate the cross-runtime story:

1. **V8 does not strength-reduce `x ** 3`** — it emits a full `pow` call
   (7.2 ns), where JSC and rustc compile it to two multiplications (~free).
   V8 *does* reduce `x ** 2` (measured free). The repo originally cubed with
   `** 3` in `convert.js` and the oklch-cubic solvers; rewriting those as
   explicit multiplications produced these Node gains (random workload,
   ns/call):

   | method                 | Node before | Node after | change |
   |------------------------|------------:|-----------:|-------:|
   | clip                   |       85.4  |      58.1  |  −32%  |
   | oklch-cubic (cached)   |      129.9  |     116.0  |  −11%  |
   | oklch-cubic (no cache) |      349.1  |     267.7  |  −23%  |
   | edge-seeker            |      186.6  |     157.9  |  −15%  |
   | edge-seeker (indexed)  |      110.9  |      84.5  |  −24%  |
   | bottosson-lightness    |      125.3  |     127.7  |   ~0   |
   | raytrace               |      218.1  |     221.1  |   ~0   |

   bottosson and raytrace never used `** 3` (control group ✓). Bun and Rust
   are unchanged — JSC's codegen is bit-identical either way, and the fuzz
   confirms Bun outputs are bitwise-identical pre/post fix. On Node, outputs
   move at the last-ULP level (max 1.1e-14 for clip, 7.2e-15 for edge-seeker
   over 685k cases); the oklch-cubic methods can flip a discriminant-threshold
   branch in `firstRoot` on a 1-ULP change, with max observed output diff
   1.4e-7 — a different but equally valid boundary root. The no-cache variant
   gained far more than the ~57 ns the op model predicts (−81 ns): removing
   seven pow calls also removes V8 call-site overhead around them.

2. **V8 ships its own `cbrt` (3.7 ns) that is 2.1× faster than glibc's
   (7.9 ns), which both Bun and Rust use.** The only heavily cbrt-bound method
   is raytrace (9 calls per color), and it is exactly the method where Node
   matches Rust and beats Bun.

These are throughput costs. In serial dependency chains (e.g. raytrace's
chroma corrections, where each iteration needs the previous cbrt results), the
*effective* cost is higher: substituting `sqrt` for `cbrt` inside raytrace
saved ~53 ns/call on Node where the throughput model predicts ~24. Shares
below are therefore lower bounds for chain-bound code.

## 3. Where each method spends its time

Per-call operation counts were measured by instrumenting the actual
implementations (random workload; counts are runtime-independent). "γ-pow" is
how many of the three output channels take the nonlinear branch of the
transfer function (dark channels ≤ 0.0031308 linear take a cheap linear
branch instead).

| method                 | cbrt | sqrt | sin+cos | cos | acos | γ-pow |
|------------------------|-----:|-----:|--------:|----:|-----:|------:|
| clip                   |   —  |   —  |    1    |  —  |  —   | 1.78  |
| oklch-cubic (cached)   | 0.82 | 0.46 |    —    | 0.17| 0.06 | 1.99  |
| oklch-cubic (no cache) | 5.30 | 5.17 |    1    | 2.45| 0.82 | 1.99  |
| bottosson-lightness    | 1.00 |   —  |    1    |  —  |  —   | 1.99  |
| edge-seeker (both)     |   —  | 1.01 |    1    |  —  |  —   | 1.99  |
| raytrace               | 9.00 | 3.00 |    1    |  —  |  —   | 1.99  |

Multiplying counts by the per-op costs and comparing against the measured
totals (random workload) gives the attribution below. "transcendental" =
cbrt + sqrt + trig + γ-pow; "everything else" = polynomial arithmetic,
table/cache lookups, branches, loads/stores, and engine overhead.

| method                 | runtime | measured | transcendental | everything else |
|------------------------|---------|---------:|---------------:|----------------:|
| clip                   | Rust    |    28.8  |   21.2 (74%)   |       7.6       |
|                        | Node    |    58.1  |   25.1 (43%)   |      33.0       |
|                        | Bun     |    57.9  |   24.4 (42%)   |      33.5       |
| oklch-cubic (cached)   | Rust    |    56.0  |   20.0 (36%)   |      36.0       |
|                        | Node    |   116.0  |   19.2 (17%)   |      96.8       |
|                        | Bun     |    97.5  |   21.0 (22%)   |      76.5       |
| oklch-cubic (no cache) | Rust    |   202.4  |   88.4 (44%)   |     114.0       |
|                        | Node    |   267.7  |   73.0 (27%)   |     194.7       |
|                        | Bun     |   251.2  |   90.7 (36%)   |     160.5       |
| bottosson-lightness    | Rust    |    83.2  |   30.4 (37%)   |      52.8       |
|                        | Node    |   127.7  |   30.4 (24%)   |      97.3       |
|                        | Bun     |   117.1  |   33.5 (29%)   |      83.6       |
| edge-seeker            | Rust    |    75.3  |   23.6 (31%)   |      51.7       |
|                        | Node    |   157.9  |   27.7 (18%)   |     130.2       |
|                        | Bun     |   148.7  |   26.8 (18%)   |     121.9       |
| edge-seeker (indexed)  | Rust    |    40.6  |   23.6 (58%)   |      17.0       |
|                        | Node    |    84.5  |   27.7 (33%)   |      56.8       |
|                        | Bun     |    75.2  |   26.8 (36%)   |      48.4       |
| raytrace               | Rust    |   222.2  |   97.2 (44%)   |     125.0       |
|                        | Node    |   221.1  |   63.1 (29%)   |     158.0       |
|                        | Bun     |   228.1  |   99.1 (43%)   |     129.0       |

Per-method notes:

- **clip** is one hue `sin`/`cos`, the 3×3 conversion, and three transfer
  functions. In Rust it is ~74% libm calls — there is essentially nothing left
  to optimize but the transcendentals themselves. Post-fix, Node and Bun are
  within 0.5% of each other.
- **oklch-cubic (cached)** retrieves the per-hue cubic structure from the
  0.1°-bucket cache (no trig on the hot path), then solves at most three
  "channel hits white" cubics — but guard tests skip most of them (see §4).
  The dominant remaining costs are the Horner evaluations, the root-solve
  arithmetic, and the cache lookup: the 3,601-entry cache is ~380 KB of
  scattered structures, and random fractional hues make it an L2-resident
  pointer chase (grid → random adds ~19 ns in Rust, ~41 ns in Node).
- **oklch-cubic (no cache)** pays ~146 ns (Rust) / ~152 ns (Node) per call to
  rebuild the hue structure: the trig, the Q/A/B/D matrix products, and above
  all six `firstRoot`/`firstTurn` solves for `tLower` and the per-channel turn
  points. This is what the cache buys.
- **bottosson-lightness** is one `cbrt` (in `findCusp`), one `sin`/`cos` pair,
  the γ-pows, and a large slab of straight-line polynomial arithmetic
  (saturation polynomial, cusp conversion, intersection). Its "everything
  else" share is the biggest of the simple methods — it does the most raw
  flops — but they pipeline well, so it lands mid-pack everywhere.
  An independent phase-probe analysis on the same machine attributes roughly a
  third of the Node total to the cusp approximation alone (~45 ns), a *fixed*
  cost paid for every non-achromatic color, with the intersection step adding
  only ~15 ns below the cusp vs ~45 ns above. That fixed cusp cost is why
  bottosson never gets as cheap on the dark side as cubic-cached or
  edge-seeker (indexed) do.
- **edge-seeker** is the final conversion (same as clip) plus the LUT lookup:
  a binary search over the 710-entry hue table, three lerps, and the
  chroma-boundary evaluation. The binary search is pure dependent branches:
  its cost is the difference to the indexed variant — ~35 ns in Rust and
  ~73 ns in JS per call on random hues, and much less on the grid where
  the branch history repeats every 360 colors (Rust grid: 31.8 vs random
  75.3 ns/call for the whole method).
- **edge-seeker (indexed)** replaces the search with a dense hue→interval
  index (one array read + ≤2 corrections), leaving conversion + lerp +
  boundary math. In Rust it is within ~12 ns of clip.
- **raytrace** is fixed-shape: 1 initial conversion, 4 box intersections, 3
  chroma corrections (each 3 `cbrt` + matrix + `sqrt`), 3 rebuild conversions,
  3 γ-pows. The 9 serial-chained cbrts dominate: they are ~44% of the Rust
  and Bun totals (glibc cbrt) and the reason Node — with V8's faster cbrt —
  matches native speed here and nowhere else.

### Output-conversion reuse

The numbers above bake in how much of the final OKLCh → Display-P3 conversion
each implementation *reuses* from its own solver, and the methods differ a lot.
A naive port that ends every method with a generic "convert the mapped OKLCh to
P3" call would redo work these implementations avoid, and it would shift the
rankings — worth knowing before comparing these results to another codebase:

- **oklch-cubic (cached)** reuses everything. Each linear-P3 channel is exactly
  `L³·Pᵢ(t)` in the solver's own cubic coefficients, so once the boundary
  `maxT` is known the output is three Horner evaluations
  (`src/oklch-cubic.js`) — no second trig, no LMS cubing, no 3×3 matrix. This
  is why its steady-state op counts show *zero* `sin`/`cos` (trig runs only
  when a hue bucket is first built).
- **raytrace** never leaves the output space: the iteration works in linear P3
  and the "final conversion" is just the three γ encodes of the last hit
  point. The hue `sin`/`cos` is computed once and reused by all three chroma
  rebuilds.
- **bottosson-lightness** reuses only the hue trig (`unitA`/`unitB` thread
  through cusp, intersection, and final conversion); the final
  `oklabToLinearP3` redoes the LMS + cube + matrix work, including the
  `kl`/`km`/`ks` slope products the intersection already computed (reusing
  those would save only ~6 multiplies — negligible next to its ~45 ns cusp
  phase).
- **edge-seeker (both)** reuses nothing: the LUT lookup yields only a
  max-chroma scalar, and the single full conversion (trig included) happens at
  the end. There is no redundancy to remove — the lookup produces no
  conversion intermediates — but also nothing shared.

The cost of getting this wrong is not small relative to the fast methods: a
redundant `sin`/`cos` pair alone is 10.6–12.9 ns, and the cube + matrix stage
another ~5–10 ns. Appending a naive full conversion to oklch-cubic (cached)
would add roughly 20 ns — ~35–50% of its Rust total and ~20–30% of its JS
total — and erode exactly the advantage that makes it the fastest real method. For
bottosson and edge-seeker the numbers here already include an (almost) full
final conversion, so they translate more directly to naive ports.

## 4. Cusp-side breakdown

The cusp sits high: averaged across hues, the P3 cusp lightness is ≈ 0.74, so
on the uniform-lightness grid/random workloads about 74.5% of samples fall
below it and only a quarter above. The below-cusp column therefore dominates
mixed-workload behavior; the columns here weight the two sides equally.

Same C = 0.4, fractional hues; lightness strictly below vs strictly above the
hue's cusp (ns/call, with above/below ratio):

| method                 | Rust below | Rust above | ratio | Node below | Node above | ratio | Bun below | Bun above | ratio |
|------------------------|-----------:|-----------:|------:|-----------:|-----------:|------:|----------:|----------:|------:|
| clip                   |      27.5  |      25.0  | 0.91× |      54.4  |      56.1  | 1.03× |     54.0  |     55.9  | 1.04× |
| oklch-cubic (cached)   |      38.8  |      92.2  | **2.37×** |   97.0  |     161.9  | **1.67×** |  79.3  |    142.5  | **1.80×** |
| oklch-cubic (no cache) |     183.4  |     227.5  | 1.24× |     246.6  |     305.9  | 1.24× |    233.4  |    294.5  | 1.26× |
| bottosson-lightness    |      75.5  |      95.8  | 1.27× |     115.1  |     140.7  | 1.22× |    106.8  |    129.2  | 1.21× |
| edge-seeker            |      68.3  |      78.0  | 1.14× |     145.9  |     165.9  | 1.14× |    138.8  |    154.2  | 1.11× |
| edge-seeker (indexed)  |      34.7  |      46.9  | 1.35× |      73.9  |      92.1  | 1.25× |     68.6  |     82.2  | 1.20× |
| raytrace               |     210.4  |     210.6  | 1.00× |     213.4  |     218.9  | 1.03× |    222.5  |    225.1  | 1.01× |

The instrumented op counts show exactly why (per call, below → above):

| method                 | what changes across the cusp |
|------------------------|------------------------------|
| oklch-cubic (cached)   | cbrt 0.38 → 1.93, sqrt 0.19 → 1.26, acos 0.00 → 0.30, cos 0.00 → 0.90. Below the cusp the binding constraint is a channel hitting **0**, and that bound (`tLower`) is hue-only — precomputed and cached. The guard tests (`turn[i] > maxT`, `A[i] ≤ 0`, `P(maxT) < target`) then skip nearly every "channel hits **1**" solve. Above the cusp those guards stop helping: ~1.26 `firstRoot` solves run per call (0.96 via the sqrt+2·cbrt path, 0.30 via the acos+3·cos path). |
| bottosson-lightness    | The gamut-intersection branch. With constant-lightness mapping the test reduces to exactly L ≤ L_cusp: below takes a one-division projective formula; above adds a full Halley refinement step (~60 flops + 3 divides). On the mixed workloads 74.5% of samples take the cheap lower branch. |
| edge-seeker (both)     | sqrt 0 → 4, abs 0 → 1. Below the cusp the boundary is modeled as a straight line (one divide + multiply). Above, it is a circular-arc intersection: four sqrts plus extra divides per call. |
| raytrace               | Nothing structural — always 4 box traces + 3 corrections. Flat within 3%. |
| clip                   | Nothing structural. In Rust it is slightly *faster* above the cusp: bright outputs always take the γ-pow branch (predictable), while the dark side mixes linear/pow branches and 0-clamps (mispredicts). |
| γ transfer (all)       | γ-pow count 1.65 → 3.00: dark outputs put 1–2 channels on the linear branch, saving ~8–10 ns/call below the cusp for every method. Despite that tailwind being *shared*, every asymmetric method is still faster below — the algorithmic effects above dominate. |

Summary: **oklch-cubic (cached) is by far the most cusp-sensitive method**
(≈1.7–2.4× slower above), bottosson and the edge-seekers are mildly sensitive
(≈1.1–1.35×), and raytrace/clip are flat. If an input distribution skews dark
(below-cusp), cubic-cached widens its lead; if it skews bright, edge-seeker
(indexed) and bottosson close the gap.

## 5. Rust vs JavaScript

- For **arithmetic-heavy** code the gap is solid but not huge after the `** 3`
  fix: Rust is ~2× faster than JS on clip, cubic-cached, and the edge-seekers
  (random workload), and 1.2–1.5× on bottosson and cubic-no-cache. The
  remaining JS overhead is spread across boxed array access, bounds checks,
  and less aggressive instruction scheduling.
- For **libm-bound** code the gap disappears: raytrace is 222 ns in Rust,
  221 ns in Node. Nine serial cbrts per call put both runtimes on their math
  library's latency, and V8's cbrt is twice as fast as glibc's. A faster cbrt
  (e.g. a hand-tuned bit-hack + Newton implementation) is the single biggest
  native-side lever: at glibc's 7.9 ns vs V8's 3.7 ns, raytrace-in-Rust leaves
  ~35–40 ns/call on the table relative to what its own structure allows.
- Rust's floor is libm: clip is ~74% transcendental calls. Cutting anything
  else barely moves it.

## 6. Node vs Bun

Before the `** 3` fix, Bun beat Node by 1.25–1.45× on every method that cubes
through `convert.js` (clip, edge-seeker, oklch-cubic) because JSC
strength-reduces `** 3` and V8 does not. With the fix applied, the engines are
close to parity (random workload, Node/Bun): clip 1.00×, cubic-cached 1.19×,
cubic-no-cache 1.07×, bottosson 1.09×, edge-seeker 1.06×, edge-seeker-indexed
1.12×, raytrace 0.97×.

What remains of the gap:

1. **`cbrt`**: V8's own implementation is 2.1× faster than the glibc one JSC
   calls. Raytrace (9 cbrts) is the only method where this dominates — and
   the only one where Node still wins.
2. **cubic-cached (1.19×)** is the largest remaining Bun edge; the hot loop is
   cache lookups and Horner arithmetic, where JSC's codegen appears simply
   tighter on this workload.
3. Everything else is within ~10% — noise territory for cross-engine
   comparisons.

## 7. Grid vs random: predictability effects

The grid repeats 360 integer hues, keeping branch history and caches hot;
random fractional hues defeat both. The methods hurt most (random vs grid):

- **edge-seeker**: Rust 31.8 → 75.3 ns (2.4×) — the binary search's dependent
  branches go from perfectly predicted to ~50/50 mispredicted. The indexed
  variant only degrades 27.7 → 40.6 ns. In JS the same pattern holds
  (Node 101.9 → 157.9 vs indexed 67.5 → 84.5).
- **oklch-cubic (cached)**: Rust 37.4 → 56.0, Node 75.4 → 116.0 — the hue
  cache goes from 360 hot buckets to 3,601 L2-resident ones.
- **clip / bottosson / raytrace**: +11–60% effects (largest on Rust clip,
  where there is little else to hide it), mostly the shared γ-branch and
  workload-value patterns.

Real-world inputs look more like *random* than *grid*; grid numbers flatter
any method with per-hue state.

## 8. Actionable observations

1. **(Applied — Node) `** 3` → explicit multiplication** in `convert.js` and
   the oklch-cubic solvers. Measured impact in §2: Node −11–32% per affected
   method, Bun/Rust unchanged. Node outputs move ≤1.1e-14 (clip/edge-seeker)
   with rare discriminant-branch flips in oklch-cubic (≤1.4e-7); Bun outputs
   are bitwise-identical; grid checksums for clip/edge-seeker/bottosson/
   raytrace are unchanged at 10 decimals and the cubics move in the 9th
   decimal.
2. **(Rust) A faster `cbrt` is the only meaningful raytrace lever** — glibc's
   is the bottleneck (~44% of the method with chain effects on top). Everything
   structural has been done (see the interior-anchor box test, README). Beyond
   that, making raytrace materially faster means an approximate variant with
   fewer correction passes — an algorithm change, not an implementation tweak.
3. **(Rust) `sin_cos()` is not worth it** on glibc — measured no faster than
   separate calls.
4. **(Idea) Memoize bottosson's cusp per hue.** The cusp approximation depends
   only on hue and is a fixed ~third of bottosson's Node cost; it could be
   cached in 0.1° buckets exactly like oklch-cubic's hue structure, with the
   same bucketed-hue semantics and cache-residency trade-offs.
5. **Method choice by input distribution**: across all three runtimes the
   fastest full-quality rows are oklch-cubic (cached) and edge-seeker
   (indexed). For dark-skewed content, oklch-cubic (cached) extends its lead;
   for bright-skewed content its advantage halves and edge-seeker (indexed) /
   bottosson close in — edge-seeker (indexed) is the most balanced across cusp
   sides. Raytrace's cost is flat but always at the top of the range.

## 9. How the numbers were produced

The measurements came from one-off harnesses built on the repo's unmodified
implementations (they are not part of the repo; this section records the
techniques so the results can be reproduced):

- **Timings** — the same shape as `bench.js` and the Rust harness: one
  dedicated pass function per method (keeping each JS call site monomorphic),
  30 warmup passes, then 25 measured rounds with the methods interleaved
  round-robin to cancel clock/thermal drift; the reported number is the median
  ns/call.
- **Cusp-side workloads** — stratified fractional hues; per sample, the cusp
  lightness for its hue is computed with the Bottosson approximation (the same
  `findCusp` the bottosson method uses) and lightness is drawn uniformly from
  strictly below or strictly above it with a 0.01 margin. Every sample's side
  and out-of-P3-gamut status is asserted before timing.
- **Per-op costs (§2)** — tight loops over 4,096 varied inputs (2,000 passes,
  median of 15 rounds) accumulating into a live sum, minus the same loop
  without the operation. Throughput measures, not latency.
- **Op counts (§3, §4)** — the `Math.*` functions were replaced with counting
  wrappers (identical semantics) and each method run over each workload in
  steady state, after a warm pass so the cubic hue cache is populated. γ-pow
  counts come from inspecting outputs (a channel above the transfer function's
  linear-branch threshold took the pow path); the bottosson branch split from
  its exact predicate L ≤ L_cusp(H).
- **Attribution (§3)** — op counts × per-op costs, cross-checked against
  substitution probes (e.g. swapping `cbrt` for `sqrt` inside raytrace) which
  bound the effect of serial dependency chains.
