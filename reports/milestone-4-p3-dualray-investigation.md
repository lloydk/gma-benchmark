# P3 Dualray slowdown investigation

The strongest identified cause is the out-of-line seed evaluator introduced by
the JS port. V8 declines to inline it, adds a JS call on about 88% of these
workloads, and boxes the two floating-point arguments and returned seed. A
scratch variant that embeds exactly the same seed expressions into the mapper
removes that boundary without changing coefficients, arithmetic or guards.

Production is unchanged after checkpoint `99cc900` (the completed port and
review fixes). The incumbent comparison is `1c06277`, immediately before the
Dualray port. Every experiment was made in an isolated scratch copy. This is an
investigation, not a committed optimization.

## Reproduction and controls

Ryzen 7 9800X3D / WSL2, Node 26.10.0 (V8 14.6.202.34-node.34), Bun 1.4.2.
CPU 2, fresh processes, common script paths, no concurrent tests/builds. Grid and
seeded-random inputs each contain 35,640 mappings. Both full-harness comparisons
use before/after/after/before ordering with two process results per version and
runtime. Focused comparisons reuse output arrays, consume every channel, and
use 50 warmup plus 25 measured passes per process. The repeated focused screen
uses six process medians per variant/runtime, in mirrored order.

Tracing, assembly capture, path counters and compatibility checks ran separately
from timing. Raw passes, input hashes, source hashes, runtime versions, complete
logs, assembly and reproduction scripts are in
[the evidence report](milestone-4-p3-dualray-investigation.json).

## The regression remains on the committed port

Full-harness medians, ns per mapping (Mitata's printed batch averages divided by
35,640; their displayed precision limits the derived percentages):

| Runtime / workload | Before port | Committed port | Change |
| --- | ---: | ---: | ---: |
| Node grid | 72.67 | 77.86 | +7.1% |
| Node random | 92.03 | 95.68 | +4.0% |
| Bun grid | 65.38 | 67.06 | +2.6% |
| Bun random | 79.69 | 82.91 | +4.0% |

## It is not extra fallback work

Separate instrumented runs count identical branches in the incumbent, committed
port and an experiment restoring the incumbent's derivative arithmetic. Both
Node and Bun give these same counts:

| Workload | Near-white gate | Near-white accepted | Seed evaluations | Upper retries | First-root calls |
| --- | ---: | ---: | ---: | ---: | ---: |
| Grid | 4,368 | 4,357 | 31,283 | 13 | 0 |
| Random | 4,268 | 4,249 | 31,391 | 16 | 0 |

P3 has no fold window. Its slowdown is not the new sRGB/Rec.2020 bisection path,
and there is no evidence here for removing correctness guards.

## V8 evidence

`--trace-turbo-inlining` explicitly rejects `seed_display_p3` with
`reason: exceeds bytecode limit`. The helper is 1,200 bytecode bytes; Node's
reported default single-function limit is 460. The mapper itself is 4,996 bytes
and does not inline into the benchmark loop. In contrast, `encodeClamped`,
`polish`, `inBlueFold` and `interiorWithin` do inline into the mapper.

The optimized mapper assembly constructs two 16-byte heap-number objects for
noninteger `A` and `B` immediately before the seed call. The seed's optimized
assembly also allocates a heap-number result, which the caller unboxes before
Halley refinement. Integer-representable directions can avoid argument boxing;
this is not a claim that every call allocates exactly the same number of bytes.
The scratch inline variant has no seed-call boundary. This establishes a concrete
Node mechanism beyond source-level function-call counting; it does not measure
an exact allocation cost in ns or diagnose Bun's internal representation.

## Controlled variants

Six-process medians from the focused comparison, ns per mapping:

| Variant | Node grid | Node random | Bun grid | Bun random |
| --- | ---: | ---: | ---: | ---: |
| Committed port | 71.10 | 87.43 | 64.20 | 79.88 |
| Incumbent | 67.94 | 83.96 | 61.25 | 74.79 |
| Inline seed only | 66.08 | 81.94 | 61.52 | 76.25 |
| Inline seed + remove P3 fold check | 66.12 | 81.47 | 61.31 | 75.51 |
| Inline seed + literal coefficients | 67.00 | 81.86 | 61.55 | 76.68 |
| Inline seed + literal coefficients + no fold check | 66.68 | 81.57 | 60.59 | 75.69 |

The seed-only variant is the smallest consistently helpful source change:
−7.1%/−6.3% Node grid/random and −4.2%/−4.5% Bun. A preliminary two-process
screen also tested literal coefficients alone, a static transfer-function import,
removing only the fold check, and restoring old derivative arithmetic. None
showed the same consistent benefit across all four measurements. Coefficient
capture alone is not established as the main cause. The repeated runs also show
process-to-process variation; individual screen timings should not be treated as
precise attribution or universal runtime behavior.

All source-specialization variants preserve outputs exactly on 71,054 P3 inputs
in each runtime. The incumbent and old-arithmetic variant differ from the port
only by at most 6.67e-16 encoded/linear on this corpus. The experiments have only
been validated for P3; they are not production multi-gamut replacements.

## Full-harness confirmation

A second ABBA comparison uses the committed port and the seed-only inline
variant. All other production sources and the full thirteen-method harness are
identical. Median batch averages converted to ns per mapping:

| Runtime / workload | Committed port | Inline seed | Change |
| --- | ---: | ---: | ---: |
| Node grid | 77.72 | 73.93 | -4.9% |
| Node random | 96.80 | 90.21 | -6.8% |
| Bun grid | 66.08 | 65.66 | -0.6% |
| Bun random | 83.33 | 79.41 | -4.7% |

The Node improvement carries through to both full-harness workloads. Bun's
random workload also improves, while its grid improvement is small (0.6%) and
should not be treated as a robust gain. This supports addressing the seed call
first; it does not establish that every part of the port's regression is solved.
Absolute numbers from separate experiments should not be subtracted to assign
costs to individual instructions.

## Recommended implementation

Keep the target descriptors, factory API, current arithmetic and guards. Have the
generator emit the balanced seed expression inside the mapper from one shared
solver template and the existing target fit data. This retains one maintained
algorithm while removing the hot call boundary. It does not require runtime
`eval`/`Function`, hand-maintained copies of the solver, or a larger color model.

Then validate all three gamuts and both runtimes, enforce existing output-buffer
and exact-face contracts, run strict Rust parity, and measure setup/code size as
well as focused and full-harness throughput. Neither the source of truth nor
safety guards need to be weakened to recover this performance.
