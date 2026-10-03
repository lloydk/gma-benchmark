# Edge Seeker on Apple ARM64

The Display-P3 f32 indexed Edge Seeker ranking was strongly affected by the
compiler's treatment of the lookup callback in the full benchmark. On this
Apple M2, the incumbent implementation measured about 56 ns/color in that
harness, but about 32 ns/color in an isolated harness compiling the same
mapper. Cached cubic stayed around 44–45 ns/color in both. This substantially
changes their relative rank without changing either algorithm's mathematics.

Removing the callback and calling `max_chroma` directly from `map_impl`
reproduced the faster result in the full harness. Independently, forcing just
the indexed lookup out of line also reproduced it. The retained change uses
direct calls and skips hue remainder arithmetic for inputs already in
`[0, 360)`. It adds no architecture-specific inlining attribute.

Measurements use rustc 1.99.0 / LLVM 23.1.1, native release/LTO, one codegen
unit, the canonical 35,640-color grid and shuffled random workloads, 50 warmup
passes and 25 measured passes. All output channels contribute to the timed
f64 checksum, matching the main benchmark's consumption. Measurements run
serially, without concurrent builds or tests. macOS core affinity was not
controlled. These are Apple M2 measurements; x86 performance of the new
source remains unmeasured.

The initial baseline was commit `b1cedb6`, including the AArch64 FMA and paired
trigonometry fixes. The diagnostic harness is appended to a disposable source
copy so it can call the actual private implementation helpers. Production
interfaces and table data are unchanged.

**Stage measurements.** Median of five fresh-process medians per cell, ns/color:

| Indexed Edge Seeker, f32 | Grid, original | Grid, hue shortcut | Random, original | Random, hue shortcut |
| --- | ---: | ---: | ---: | ---: |
| Read hue / consume output control | 0.90 | 0.85 | 0.85 | 0.85 |
| Normalize hue | 2.13 | 0.88 | 2.00 | 0.86 |
| Lookup on pre-normalized hues | 3.38 | 3.35 | 4.80 | 4.90 |
| Normalize and lookup | 4.14 | 3.65 | 6.75 | 5.05 |
| Boundary from precomputed table items | 1.41 | 1.41 | 2.90 | 2.81 |
| Normalize, lookup and boundary | 5.97 | 5.85 | 15.52 | 12.24 |
| Convert precomputed mapped OKLCh to encoded RGB | 16.40 | 16.37 | 26.93 | 26.98 |
| Complete indexed mapper, isolated harness | 32.28 | 30.43 | 52.08 | 48.45 |
| Complete binary-search mapper, isolated harness | 67.24 | 61.96 | 93.70 | 87.83 |
| Cached cubic control, isolated harness | 44.87 | 45.03 | 67.73 | 67.44 |

These costs are not additive. Separate stages have different input strides,
compiler optimization opportunities and dependencies; they include their own
load/checksum overhead. The boundary and conversion stages receive inputs
precomputed outside timing. The probe checks every reconstructed output
channel against both production Edge Seeker variants before timing. Output
fingerprints and stage checksums agree between the original and hue-shortcut
builds for both precisions and workloads.

Hue wrapping alone is a small part of the gap. In a separate full-harness
ABBA comparison, skipping it changed indexed f32 grid from 55.87 to 56.60 ns
and random from 64.85 to 63.78 ns. It helped ordinary binary-search Edge
Seeker, but did not resolve the indexed ranking reversal.

**Call-structure experiment.** All three builds below retain the original hue
normalization. Two fresh processes per variant, forward then reverse order:

| Full harness, f32 | Indexed grid | Indexed random |
| --- | ---: | ---: |
| Original callback | 56.08 | 66.28 |
| Direct calls | 32.10 | 51.98 |
| Outlined indexed lookup (`inline(never)`, diagnostic only) | 32.26 | 52.28 |

Disassembly of the original full-harness plain indexed loop calls
`EdgeSeekerIndexed::map_impl::{closure#0}`. That separate function contains
normalization, indexed lookup, interpolation and the boundary calculation.
In the faster isolated build, the compiler instead emits a separate
`get_lut_item_indexed` function. These two source interventions show that the
call structure is material to the regression. They do not identify a specific
hardware stall or establish that every call boundary is harmful.

**Retained implementation, full harness.** The production source combines
direct calls with the normalized-hue shortcut. Medians below use three original
and five final processes in order original/final/final/original/final/original/
final/final. This includes a confirmation after one noisy final random run.

| f32 method | Grid before | Grid after | Random before | Random after |
| --- | ---: | ---: | ---: | ---: |
| Indexed Edge Seeker | 55.75 | 30.42 | 65.02 | 47.83 |
| Binary-search Edge Seeker | 66.99 | 62.68 | 93.44 | 89.42 |
| Cached cubic control | 43.85 | 43.90 | 66.21 | 66.18 |

Indexed Edge Seeker improves 45.4% on the grid and 26.4% on random inputs by
these medians. It moves ahead of cached cubic in both workloads, matching that
pair's ordering in the supplied Ryzen results. The original full-harness ARM
ranking therefore did not establish an inherent algorithmic advantage for
cached cubic over the indexed lookup.

Final indexed grid medians were 30.68, 31.72, 30.42, 30.42 and 30.41 ns;
random medians were 49.20, **69.50**, 47.51, 47.83 and 47.50 ns. The outlier is
retained. Original random medians were 66.31, 64.53 and 65.02 ns. The unchanged
cached-cubic grid control also had one 49.08 ns original process among otherwise
approximately 44–45 ns results. The measurements support the large indexed
improvement, but not precise claims about small timing differences.

The f64 indexed medians change 45.56 to 45.25 ns on grid and 62.49 to 61.87 ns
on random, while binary-search f64 changes 89.01 to 87.66 and 110.33 to 107.23
ns. The main effect is specific to the f32 code generated in this full harness.
The final binary has a separate `get_lut_item_indexed` function for each f32
gamut and no indexed mapper callback symbol.

**Validation.** All 124 native release tests pass with the retained changes,
including all three gamuts, both precision lanes, both mapping modes and the
existing independent policy oracles. The added hue test compares normalization
bits against the original rule at signed zero, wrap boundaries and their
adjacent representable values, extremes, and 100,000 raw floating-point bit
patterns per lane. Non-finite hue checks preserve NaN results. Negative wraps
retain the original rule, including its rounding at the endpoints.

**Reproduction.** The runner reconstructs the original callback and hue helper
from either the incumbent or the fixed working tree, then builds the candidate
variants in the requested new output directory. It records source/input/binary
hashes, compiler metadata, raw pass timings, output fingerprints and process
medians. `--structure` includes direct calls, an outlined lookup, and the
combined direct-call/hue-shortcut candidate. `--full-runs` adds full-benchmark
measurements after the stage probes. The Rust changes are not written back by
the runner.

```sh
python3 scripts/bench-edge-seeker-stages.py \
  --output /tmp/edge-seeker-investigation \
  --runs 5 --structure --full-runs 2

# On Linux, add --cpu 2 (or another suitable idle logical CPU).
```

Raw artifacts from this investigation are in
`/tmp/gma-edge-stages-e8rfo7kt/run/`: `results.json`, `full-results.json`,
`structure-results.json`, `final-results.json`, `confirmation-results.json`,
`combined-final-results.json`, build/test logs, binary copies and disassembly. The stage run preceded the
runner's optional call-structure/full-harness integration; the follow-up
commands and source copies are also preserved in that directory.
