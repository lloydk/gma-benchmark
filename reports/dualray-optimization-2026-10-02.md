# Dualray optimization experiments

The JSON artifacts cited here were removed from the working tree to keep the
repository small. Restore one with `git restore --source=696f4c2 -- <path>`.

Retained one change: Rust f64 Dualray Fast reuses the canonical precheck's
sine/cosine only when its angle equals the solver's angle. Both expressions
remain unchanged. On this Ryzen 9800X3D, confirmation with a second input seed
saved 2.1% of time across twelve equally weighted workload/target cells, with
4.0–8.1% savings on bright inputs. Interior input was 1.5% slower in aggregate;
this is a modest distribution-dependent improvement, not a universal speedup.

Ordinary Dualray, JavaScript Fast, and the native f32 algorithm retain their
existing paths. The initial three candidates were isolated from baseline
`fdd2eb24a588d4f42bc5ef19be45e8079f0fb952` until validation and timing finished.

## Protocol fixed before timing

- Keep all fitted coefficients, numerical tolerances, fold windows, and the
  existing near-white gate unchanged. Do not search thresholds against timing
  inputs or add special cases for their chroma, hue, or lightness values.
- Use independently seeded, variable-chroma volume, bright, boundary-relative,
  and interior distributions across sRGB, Display-P3, and Rec.2020. Generate
  boundary locations from the separate XYZ/stationary-interval oracle.
  Validation and timing use different seeds. The workload generator is
  `scripts/dualray-experiment-workloads.mjs`; these distributions were fixed
  before candidate timing began.
- Validate against each method's existing contract. Fast preserves canonical
  in-gamut outputs bit for bit; ordinary Dualray retains its first-exit policy,
  including folded re-entry regions. Retain the established f32 corrections in
  the Halley candidate. Check f32 separately for the other candidates.
- Time Node, Bun, Rust f64, and Rust f32 in serial, fresh, CPU-pinned processes.
  Rotate and reverse process order between three rounds. Consume all three
  output channels and compare timed checksums with separate validation runs.
- Judge changes across distributions and targets, including regressions and
  unchanged-method controls. A speedup in the original fixed-chroma benchmark
  alone is insufficient. Preserve source/input hashes and candidate sources.
- Consider a combined implementation only after evaluating the individual
  changes. Recheck the combination; do not assume their savings add.

The comparison runner is `scripts/compare-dualray-optimizations.mjs`. It takes
named source trees, records the baseline revision, embeds changed candidate
sources, regenerates timing inputs, and records hashes, environment, process
medians, individual passes, and checksums.

## Results

The screening artifact (`reports/dualray-optimization-2026-10-02-screen.json`) contains
384 cells and 1,152 timing processes. Values below are percentage changes in
elapsed time for Fast, using geometric means of ratios across the three
gamuts and four equally weighted distributions. Positive means slower.

| Candidate | Node | Bun | Rust f64 | Rust f32 |
| --- | ---: | ---: | ---: | ---: |
| Second chord correction uses Halley | −0.8% | +0.4% | −1.2% | +1.6% |
| Add existing near-white predictor | +2.3% | +1.3% | +1.2% | +3.7% |
| Reuse canonical trig unconditionally at ordinary hues | −2.4% | +2.1% | −4.2% | −1.2% |

Halley gave no convincing general gain. Ordinary Dualray also failed to gain
consistently: +1.1% Node, +0.2% Bun, and +0.4% Rust f64. The Halley candidate's
f32 arithmetic was unchanged; its timing differences illustrate compilation,
layout and process variation. The near-white predictor added enough setup and
certification work to lose overall. Neither candidate was retained.

Trig reuse warranted a narrower follow-up. Canonical conversion computes
`h * PI / 180`; the solver computes `h * (PI / 180)`. Sharing their directions
unconditionally changes some mapped result bits. The retained variant requires
these angles to compare equal, keeps the original canonical conversion, and
leaves out-of-range authored hues on their existing path. This equality test
comes from the numerical contract; no threshold was fitted to the workload.
The f32 path and JS implementations were retained because their initial
results did not establish a consistent improvement.

### Confirmation with a second seed

The confirmation artifact (`reports/dualray-optimization-2026-10-02-confirmation.json`)
uses seed `0x13198a2e`, five fresh processes per cell, and 480 timing processes.
Initial timing used `0x243f6a88`; oracle validation uses `0x6d2b79f5`.

| Rust f64 Fast distribution | Time change across three gamuts |
| --- | ---: |
| Variable-chroma volume | −2.48% |
| Bright | −5.44% |
| Boundary-relative | −1.85% |
| Interior | +1.48% |
| All twelve cells, equally weighted | −2.11% |

| Bright inputs | Baseline ns/color | Retained variant ns/color | Time saved |
| --- | ---: | ---: | ---: |
| sRGB | 79.77 | 76.41 | 4.21% |
| Display-P3 | 80.56 | 77.33 | 4.01% |
| Rec.2020 | 82.30 | 75.67 | 8.05% |

The unchanged ordinary Dualray control moved −0.09% in f64 and +1.01% in
f32 overall. Unchanged Fast f32 moved −0.16%. Individual process ranges are
recorded in the artifact; several small differences overlap that variation.
Interior sRGB/P3 medians were 2.3%/2.5% slower, and the sRGB boundary-relative
median was 1.8% slower. These regressions are included in the aggregate.

These are scalar OKLCh-to-encoded-RGB kernels with ordinary output encoding,
not public color-object or browser measurements. The polynomial-encoder row
was not timed. The host was AMD Ryzen 7 9800X3D, logical CPUs `2,3`, Node
26.10.0, Bun 1.4.2, and rustc 1.98.1 / LLVM 22.1.8. Rust used
`-C target-cpu=native`, opt-level 3, LTO, and one codegen unit. f32 inputs were
rounded before timing. All three output channels were consumed.

No combined candidate was needed: only the restricted Rust f64 reuse change
was retained. No fitting coefficients, tolerances, or classification gates
changed, and the original fixed-`C=0.4` timing dataset was not used for selection.

### Correctness

- The baseline and three initial candidates passed 3,108,080 independent JS
  oracle checks, combining separate-seed samples with existing boundary,
  fold, endpoint and authored-hue probes. Each candidate also passed focused
  Node tests and all 119 native Rust tests, including f32.
- The retained Rust implementation passed all 119 tests in both portable and
  native builds. Its final source is byte-identical to the measured
  `rust/src/dualray_fast.rs`.
- Portable and native f64 probe output matched baseline byte for byte on
  388,510 records each: 777,020 total, covering all three gamuts. Those JSONL
  records contain mapped output, canonical membership/conversion, both entry
  modes, and signed-zero hue information. This establishes observed bitwise
  equivalence on this compiler/host, not a cross-platform libm guarantee.
- Final JS oracle replay passed 777,020 checks, and both generated-data
  freshness checks passed. No JS production source was changed.

Validation results and logs (`reports/dualray-optimization-2026-10-02-validation.json`)
include initial accuracy maxima, native/Node logs, and portable/native output
hashes. Final JS replay (`reports/dualray-optimization-2026-10-02-final-js-validation.json`)
records reference and source hashes.

### Replay

Reconstruct the measured candidate from the confirmation artifact, then run
the comparison from this checkout. The artifact includes a formatting-only
change to the unused `dualray-fast` example caused by formatting the isolated
tree; that file is not part of the performance binary and was not retained.

```sh
dualray_baseline=$(mktemp -d)
dualray_candidate=$(mktemp -d)
git archive fdd2eb24a588d4f42bc5ef19be45e8079f0fb952 | tar -x -C "$dualray_baseline"
cp -a "$dualray_baseline/." "$dualray_candidate/"
node --input-type=module - "$dualray_candidate" <<'JS'
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
const report=JSON.parse(readFileSync('reports/dualray-optimization-2026-10-02-confirmation.json'));
for(const [path,source] of Object.entries(report.trees[1].changedSources))
 writeFileSync(resolve(process.argv[2],path),source);
JS
node scripts/compare-dualray-optimizations.mjs \
  --tree baseline="$dualray_baseline" --tree exact-reuse="$dualray_candidate" \
  --baseline-revision fdd2eb24a588d4f42bc5ef19be45e8079f0fb952 \
  --runtimes rust-f64,rust-f32 --runs 5 --seed 0x13198a2e \
  --output /tmp/dualray-confirmation.json
node scripts/validate-dualray-optimizations.mjs \
  --tree current=. --output /tmp/dualray-js-validation.json
cargo test --release --manifest-path rust/Cargo.toml --bin gma-bench
RUSTFLAGS='-C target-cpu=native' cargo test --release --manifest-path rust/Cargo.toml --bin gma-bench
```

For the native bit comparison, serialize the union of
`Object.values(dualrayExperimentWorkloads(gamut,true)).flat()` and
`dualraySamples(gamut)` with `serializeProbes`. Feed those lines to the existing
`dualray-fast-mapping-probes` example in each tree, once with default Rust
flags and once with `-C target-cpu=native`, using file-backed stdin/stdout.
Compare the resulting JSONL bytes. Expected counts and hashes are in the
validation artifact's `exactReuse.bitComparisons` array.
