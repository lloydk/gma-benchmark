# Bottosson optimization integration

The JSON artifacts cited here were removed from the working tree to keep the
repository small. Restore one with `git restore --source=696f4c2 -- <path>`.

The benchmark still defaults to unchecked mapping. Bottosson may move an already in-gamut color in that mode; `--in-gamut-check` opts into the original-coordinate membership check. This integration changes execution cost, not those policies. The earlier [investigation](milestone-4-bottosson-investigation.md) explains the archive comparison and the candidate selection.

## Implementation

- Reuse the uncached checked mapper's sine/cosine direction when its precheck rejects the input. Plain calls retain the gray shortcut and compute the direction once; warm cached plain calls compute neither sine nor cosine.
- Place saturation evaluation inside cusp calculation. Emit the same intersection equations into both mapper bodies, avoiding the scalar helper calls identified in the investigation.
- Maintain the mapper template and one intersection fragment under `scripts/templates/bottosson-*.js`. The precheck arithmetic is emitted from the marked canonical conversion in `src/rgb-convert.js`, including its strict membership predicate and evaluation order.
- `node scripts/generate-bottosson.mjs --check` now checks fits and the emitted kernel. `generate-bottosson-kernel.mjs` also runs independently. No runtime code generation or eval is used. The hue table still fills lazily at runtime, at the same 0.1-degree resolution.
- No Rust mapping changes. Target fits, primary-contact guards, factories, output buffers and cache ownership remain unchanged.

## Full-harness measurements

Before is `be03abd`; after is the integrated source recorded in the evidence JSON. Ryzen 7 9800X3D / WSL2, CPU 2, Node 26.10.0 and Bun 1.4.2. Serial before/after/after/before order, common script path, fresh processes: 16 complete P3 harness runs, two per version/runtime/mode. Each run retains all 13 methods, 50 complete warmup passes, reused output buffers and output-consuming checksums. No tests, profiling or builds ran concurrently with the timed processes.

Values are ns per mapping: medians of the two Mitata process averages divided by 35,640 inputs. Grid and stratified/shuffled random workloads both use C=0.4; setup is excluded. Percentages inherit the precision of the printed timing results.

| Runtime / mapper / mode | Grid before → after | Random before → after |
| --- | ---: | ---: |
| node / uncached / plain | 125.14 → 116.72 (-6.7%) | 142.68 → 136.22 (-4.5%) |
| node / cached / plain | 63.27 → 58.64 (-7.3%) | 86.98 → 79.97 (-8.1%) |
| node / uncached / checked | 139.59 → 117.14 (-16.1%) | 159.93 → 138.61 (-13.3%) |
| node / cached / checked | 80.95 → 76.88 (-5.0%) | 112.37 → 108.59 (-3.4%) |
| bun / uncached / plain | 113.22 → 112.93 (-0.2%) | 132.15 → 128.51 (-2.8%) |
| bun / cached / plain | 57.24 → 54.57 (-4.7%) | 76.74 → 76.46 (-0.4%) |
| bun / uncached / checked | 133.00 → 121.91 (-8.3%) | 156.43 → 140.29 (-10.3%) |
| bun / cached / checked | 74.78 → 71.83 (-3.9%) | 99.75 → 96.80 (-3.0%) |

These measurements compare the integrated implementation with the immediate pre-optimization version, not with the archive. They establish no other-target throughput claim or universal speedup. Both process values for every row and all full-harness logs are retained in the evidence. Full-harness checksums consume outputs but scale with adaptive Mitata iteration counts, so they are not compared across processes. The separate exact-output check establishes compatibility.

## Setup and size

Cold setup here means module import plus constructing the two selected P3 factories, in fresh processes with a warm filesystem; process launch and hue-cache population are excluded. Eight samples per version/runtime are interleaved with the four timing blocks. These small timings are noisy and are not end-to-end application startup measurements.

| Runtime | Before median | After median |
| --- | ---: | ---: |
| node | 3.048 ms | 3.390 ms |
| bun | 2.715 ms | 2.845 ms |

Bun's minified browser bundle of `src/bottosson-lightness.js` (all exported factories) grows from 9,883 to 10,899 bytes; gzip grows from 4,960 to 5,454 bytes. The emitted factory source is 9,467 → 12,609 bytes. Cache allocation and table resolution are unchanged.

## Verification

- Node: all 95 tests pass. Bun: the pre-existing full 94-test suite passes; the final 10-test Bottosson file, including the new mode regression, passes.
- Exact output compatibility against `be03abd`: 1,717,812 comparisons per runtime across three gamuts, both mappers and both modes. Grid/random and the existing boundary, primary-contact, hue-wrap, gray, negative-chroma and extreme-lightness probes are included. Outputs are bit-identical, including signed zeros; return-buffer identity and input/output aliasing are checked. Counts include repeated samples.
- Independent Bottosson approximation-policy tests and JS/Rust f64 parity pass in Node and Bun. Parity continues to report native membership disagreements at rounding-scale boundaries separately; this change does not harmonize runtime sine/cosine implementations.
- All-gamut `--validate-only` passes under Node and Bun, with and without `--in-gamut-check`. Fit/kernel freshness checks pass.
- The new regression uses the actual in-gamut Rec.2020 grid sample `[0.85, 0.4, 148]`. Default/false mode may change it; true mode returns the independent canonical expectation.
- Initial sandboxed Node CLI tests failed with child-process `EPERM`; the full suite and subprocess-dependent checks passed outside the sandbox. Timing also ran outside it for both versions. Rust mapping code is unchanged, so the Rust release suite was not rerun.

## Evidence and reproduction

The evidence JSON (`reports/milestone-4-bottosson-optimization.json`) contains raw runs, setup samples, source hashes, before/after changed sources, generator/templates, test logs, parity and compatibility results, and the exact measurement scripts. To reproduce, archive its `baselineCommit` into `before` and `after` directories beneath a new temporary root, link each `node_modules` to this repository's installed dependencies, and overwrite the two `after` files from `sources.after`. Extract `reproduction.measure.py` and `reproduction.compat.mjs` into that root. The compatibility runner uses the archived baseline workloads and probe helpers.

Run the compatibility script with `after` in Node and Bun and save its JSON as `checks/compat-node.json` and `checks/compat-bun.json`; copy the recorded parity objects to `checks/parity-node-final.json` and `checks/parity-bun-final.json` (or rerun parity on the reconstructed candidate). Run `python3 <root>/measure.py` from this repository. The runner requires Linux taskset with CPU 2 available, Node, Bun, Python 3.12+, and no concurrent benchmark/build load. Source snapshots and the full benchmark loops are otherwise untouched. Repeated measurements will vary.
