# Bottosson P3 slowdown investigation

The JSON artifacts cited here were removed from the working tree to keep the
repository small. Restore one with `git restore --source=696f4c2 -- <path>`.

This records the investigation at `be03abd`. The subsequent
[integration report](milestone-4-bottosson-optimization.md) covers the integrated
source layout, fresh before/after measurements and full verification.

The checked slowdown has two clear sources: the cached mapper now performs a
canonical authored-hue membership conversion, and the uncached mapper repeats
trigonometry after that conversion rejects an input. Scalar helper calls and
runtime inlining decisions add avoidable overhead. Their impact differs between
Node and Bun; the plain-path regression is not attributable to one universal
constant-loading or guard cost.

A scratch prototype keeps the current coefficients, contact guards, canonical
membership rules, factory API and runtime hue cache. It reuses the uncached
precheck's direction and places the intersection math inside both mapping
functions, with saturation math inside cusp calculation. In the full harness,
checked uncached mapping improves **12.1–16.9% in Node** and **10.3–16.1% in Bun**.
Other Bottosson rows improve **1.7–10.4%** in this comparison. Production mapping
code is unchanged; this is an investigation and a tested prototype, not a
shipped optimization.

## What changed during the port

### Cached checked mapping: an intentional policy cost

The original P3 mapper checked gamut membership using its 0.1-degree cached
LMS slopes. The current mapper first converts the **authored** hue and chroma,
using the canonical RGB matrix arithmetic, and preserves that result when it
is in gamut. Only rejected inputs consult the quantized mapping cache.

That correction means a warm cached checked call now evaluates one sine and
cosine, plus the canonical conversion. Warm cached plain mapping still needs
neither. This is not table generation leaking into the timed loop: untimed
instrumentation records zero cusp/cube-root evaluations on the warmed cached
workloads.

Restoring the old-style quantized precheck recovers much of the checked cost
in a diagnostic ablation, but changes outputs. On positive-chroma interior-
lightness probes, its largest P3 difference from the current policy is
**0.034232 encoded**, at
`[0.3, 0.20757590317248611, -455.95797738363007]`. The same counterfactual changes
sRGB and Rec.2020 outputs too. Reverting the precheck is therefore not a valid
way to recover performance under the current pass-through contract. These
numbers compare the counterfactual with current output, not a new approximation
error allowance.

### Uncached checked mapping: duplicated direction calculation

The original uncached mapper computed the hue direction once and used it for
both membership and projection. The current mapper calls
`oklchToRgbIfInGamut(L, rawC, H, out)`, which computes sine and cosine internally.
On rejection, the mapper calculates them again before computing the cusp.
The prototype keeps the exact `H * Math.PI / 180` arithmetic and raw-chroma
canonical check, and reuses that direction for projection.

Untimed source-call counts after warming each cache, per input:

| Path | Original sine/cosine pairs | Current | Reuse prototype |
| --- | ---: | ---: | ---: |
| Uncached plain | 1 | 1 | 1 |
| Uncached checked | 1 | 2 | 1 |
| Cached plain | 0 | 0 | 0 |
| Cached checked | 0 | 1 | 1 |

Each count was observed over both 35,640-input workloads. The probes wrap Math
functions and are deliberately separate from timing. They expose duplicated
source work, not exact optimized machine-instruction counts for either engine.

### Call boundaries and scalar interfaces

V8's trace rejects `findGamutIntersectionQ` (**987 bytecode bytes**) and
`computeMaxSaturationRgb` (**582 bytes**) for inlining; its default individual
limit here is 460. The canonical precheck is eligible by size but is left out
of line in the inspected optimized mapper. The disassembly contains heap-number
allocation sequences before scalar calls, including the precheck and
intersection. This establishes an allocation/call mechanism in Node, not an
exact per-input allocation total or a measured number of nanoseconds per box.

The old implementation also had non-inlined saturation and intersection
helpers. Inlining them removes some pre-existing overhead as well as costs of
the new arrangement; its entire gain must not be labeled port overhead.

The uncached interface changed from `(a, b, L, C, L0, cusp)` to
`(q0, q1, q2, L, C, L0, cuspL, cuspC)`. It passes more numeric values and computes
all three slopes before the helper, including when the lower branch does not
use them. Restoring only the old interface while retaining current arithmetic,
canonical checks and contact guards helps Bun, but does not reliably improve
Node. This is a measured runtime-specific effect; Bun assembly was not audited,
and V8's heap-number behavior must not be assumed for Bun.

Four-process focused medians for that interface-only experiment (ns):

| Runtime / mode | Current grid / random | Old interface grid / random |
| --- | ---: | ---: |
| node / plain | 113.28 / 127.71 | 113.03 / 129.14 |
| node / checked | 133.98 / 153.72 | 129.00 / 152.13 |
| bun / plain | 111.65 / 128.66 | 105.73 / 123.98 |
| bun / checked | 128.83 / 150.94 | 108.19 / 128.82 |

Moving sector selection before the saturation call, to pass an integer sector
instead of a hue, did not yield a consistent improvement. P3 literal-coefficient
experiments helped some Bun rows but not consistently Node. Removing the
primary-contact guard or restoring the old radian multiplication order also
failed to provide a consistent cross-runtime recovery. Those exploratory
ablations are not proposed changes. In particular, this investigation supplies
no reason to weaken the numerical fixes or abandon target factories.

The warm cached plain path does not execute the contact guard, and its
intersection already took scalar slopes before the port. Its Bun regression
therefore cannot be attributed to either of those newly added uncached costs.
It remains partly a runtime/code-layout effect: inlining helps, but the
experiments do not identify a unique instruction-level cause for every plain
regression.

## Behavior-preserving prototype

`reuse-and-math` changes source placement and reuse, preserving evaluation order:

- The uncached canonical check computes the direction once. Rejected inputs
  reuse it; plain calls retain their gray fast path.
- The current intersection body appears inside both mapper bodies, eliminating
  that scalar call boundary. The saturation body appears inside cusp evaluation.
  Cusp evaluation can still be a separate call; this is not a claim that every
  helper becomes machine-level inline code.
- Fits, hue buckets, contact classification, matrix/transfer arithmetic, output
  buffers, and the authored-input membership policy remain unchanged.

Two leading prototypes were compared exactly against current output over
**3,435,624 sample comparisons per runtime**, across all three gamuts, cached
and uncached mapping, both modes, grid/random inputs and existing boundary,
primary-contact, hue-wrap, gray and extreme-lightness probes. Both Node and Bun
were **bit-identical**, including signed zeros. Return-buffer identity and
aliased input/output arrays were checked. This includes **1,717,812 comparisons
per runtime for the full-harness candidate**; counts include repeated samples.
The three call-interface diagnostics received another 5,153,436 exact comparisons
per runtime, also without differences.

These are compatibility checks against the already-validated current mapper,
not a new independent accuracy proof. No full Node/Bun/Rust suite was rerun for
this investigation. Integration should still run the independent policy suites,
strict parity tools and generator checks.

## Full-harness confirmation

Current `be03abd` versus the scratch `reuse-and-math` prototype, with the entire
13-method P3 harness retained. All values are ns per mapping; negative changes
mean faster. These are direct paired measurements against the current tree,
not a claim of full recovery against the pre-port baseline.

| Runtime / method / mode | Grid, current → prototype | Random, current → prototype |
| --- | ---: | ---: |
| node / uncached / plain | 123.88 → 116.72 (-5.8%) | 141.55 → 139.17 (-1.7%) |
| node / uncached / checked | 139.45 → 115.88 (-16.9%) | 159.37 → 140.15 (-12.1%) |
| node / cached / plain | 63.69 → 57.10 (-10.4%) | 86.28 → 80.11 (-7.2%) |
| node / cached / checked | 81.65 → 75.06 (-8.1%) | 112.51 → 107.04 (-4.9%) |
| bun / uncached / plain | 116.44 → 110.69 (-4.9%) | 133.42 → 126.40 (-5.3%) |
| bun / uncached / checked | 134.26 → 120.37 (-10.3%) | 162.74 → 136.50 (-16.1%) |
| bun / cached / plain | 59.34 → 53.87 (-9.2%) | 76.74 → 73.93 (-3.7%) |
| bun / cached / checked | 78.14 → 71.55 (-8.4%) | 101.15 → 97.22 (-3.9%) |

Checked cached random mapping retains most of the authored-hue conversion cost;
its gain is about 4–5% here. The isolated screen showed larger gains in some
rows, particularly Node cached plain mapping. The full-harness numbers above
are the relevant confirmation and should not be replaced by the best screen
result.

## Measurement controls and evidence

Ryzen 7 9800X3D / WSL2, Node 26.10.0 and Bun 1.4.2, CPU 2. All measured
processes are fresh, serial and use the same path. Focused kernels use the
existing 35,640-input grid and stratified/shuffled random workloads at C=0.4,
50 complete warmup passes, 25 measured passes, reused output arrays and
three-channel checksums. Current and behavior-preserving variants have identical
workload hashes and focused checksums.

Initial screens use two mirrored sweeps. The leading candidates and the
call-interface experiments have four process results per variant/runtime/mode.
The full confirmation uses current/prototype/prototype/current order: **16
processes**, two per version/runtime/mode, with all 13 methods and both workloads.
Node uses `--expose-gc`. Full values are medians of Mitata's printed process
averages divided by 35,640; their displayed precision limits the derived
percentages. Validation and profiling are separate from controlled repeated and
full timing runs. A few syntax-check processes overlapped the exploratory first
screen; that screen is used for selection, not the final performance claim.

Raw results, all process passes, full harness logs, output comparisons, variant
source, V8 traces/disassembly, source hashes, and construction/reproduction
scripts are in the evidence JSON (`reports/milestone-4-bottosson-investigation.json`).
The earlier [original-P3 comparison](milestone-4-p3-baseline-comparison.md)
remains the direct pre-port comparison. Do not subtract absolute times across
the two experiments to attribute individual instruction costs.

This is one CPU/runtime pair and a high-chroma workload. Cold setup, bundle
size, in-gamut-heavy throughput and other-target performance were not measured.
The three-gamut checks establish sampled output compatibility, not performance
claims for sRGB or Rec.2020.

## Recommendation

Implement the direction reuse and measured helper layout, while retaining
canonical membership and primary-contact guards. Keep one maintained source
for the equations and canonical conversion; emit the hot bodies from that
source if needed, as with Dualray. Avoid hand-maintained duplicate solvers.
The existing target descriptors, generated fits, public factories and lazy
runtime hue caches can stay.

The measured prototype is the concrete starting point, not production-ready
source organization. After integration, rerun the independent tests/parity and
both runtime benchmarks, and measure code-size/setup changes before drawing a
final performance conclusion.

## Reproduce the experiments

The evidence embeds `bootstrap.py` and each experiment's exact source. From
the repository, extract the bootstrap to a temporary file:

```sh
python3 - <<'PYCODE'
import json
from pathlib import Path
p = json.loads(Path('reports/milestone-4-bottosson-investigation.json').read_text())
Path('/tmp/reproduce-bottosson.py').write_text(p['scripts']['bootstrap.py'])
PYCODE
python3 /tmp/reproduce-bottosson.py \
  reports/milestone-4-bottosson-investigation.json "$PWD" \
  /tmp/gma-bottosson-repeat
```

Use a new output directory, installed repository dependencies, Node, Bun,
Python 3.12+, and Linux `taskset` with CPU 2 available. The bootstrap archives
the pinned commits and runs construction, focused screens/repeats, the full
comparison, call-interface probes and exact-output comparisons serially. The
embedded `count.mjs` and recorded diagnostic commands cover the untimed call
counts and V8 investigation. Repeated runs need not produce identical timings.
