# Dualray Rust optimization follow-up: f32 priority

The JSON artifacts cited here were removed from the working tree to keep the
repository small. Restore one with `git restore --source=696f4c2 -- <path>`.

No additional mapper change was retained. The Rust priority is an improvement
in f32, either alone or alongside f64. Specializing Fast's polynomial evaluation
by hue sector helped f64 but did not establish an f32 benefit on confirmation.
The earlier f64-only trig-reuse patch is separate from these experiments.

## Candidates and results

Both candidates start from `fdd2eb24a588d4f42bc5ef19be45e8079f0fb952`, without
the f64-only trig-reuse patch:

- **Sector specialization:** dispatch the existing hue sectors to a const-generic
  helper. This exposes coefficient rows and output channel indices to LLVM at
  compile time. Polynomial grouping, coefficients and sector boundaries stay
  unchanged.
- **Sector specialization plus zero-tail removal:** also omit arithmetic on
  trailing zero coefficients, preserving the Estrin grouping of nonzero terms.

Values are percentage changes in elapsed time, using geometric means of the
twelve equally weighted gamut/distribution ratios. Negative means less time.

| Candidate | Screening f32 | Screening f64 | Confirmation f32 | Confirmation f64 |
| --- | ---: | ---: | ---: | ---: |
| Sector specialization | −1.20% | −2.39% | −0.14% | −3.24% |
| Plus zero-tail removal | −1.22% | −2.15% | Not repeated | Not repeated |

Explicit zero-tail removal did not establish an advantage over specialization
alone, so only the simpler candidate advanced to confirmation.

In confirmation, unchanged ordinary Dualray moved −0.82% in f32 and −0.19% in
f64. Fast f32's distribution aggregates were +3.29% for variable-chroma volume,
+0.30% for bright input, −4.37% around the boundary and +0.36% for interior
input. Several process ranges exceeded 5%, and some exceeded 10%. This is mixed
evidence, not a reliable general f32 improvement. Neither candidate was adopted.

## Measurement and validation

The unchanged experiment runner used sRGB, Display-P3 and Rec.2020; four
independently generated distributions; 8,192 colors per cell; 50 warmup and 25
measured passes; serial fresh processes pinned to logical CPUs `2,3`; and
checksums consuming all three output channels. Screening used seed `0x243f6a88`
and three processes per cell (432 timing processes). Confirmation used seed
`0x13198a2e` and five processes per cell (480 timing processes).

Hardware/compiler: AMD Ryzen 7 9800X3D, rustc 1.98.1, LLVM 22.1.8,
`-C target-cpu=native`, release opt-level 3, LTO, one codegen unit. These are
scalar OKLCh-to-encoded-RGB kernel measurements with ordinary encoding.

The more invasive zero-tail candidate passed all 119 native Rust tests,
including both precisions, independent first-exit oracle coverage and the
polynomial result-bit compatibility test. Every baseline/candidate validation
checksum matched exactly: 48 pairs for each screening candidate and 48 pairs
in confirmation. Aggregate checksum agreement is not a full mapping bitwise
comparison; no broader mapping-bit replay was performed for these rejected
candidates. No fit, tolerance, classification threshold or workload was tuned.

Raw results include source/input/binary hashes, changed candidate source files,
all process timings and environment details:

- Screening (`reports/dualray-optimization-2026-10-02-f32-screen.json`)
- Confirmation (`reports/dualray-optimization-2026-10-02-f32-confirmation.json`)
- Validation evidence (`reports/dualray-optimization-2026-10-02-f32-validation.json`)

The source reconstruction procedure in the [earlier report](dualray-optimization-2026-10-02.md#replay)
also applies: reconstruct the named candidate from `trees[].changedSources`,
then run `scripts/compare-dualray-optimizations.mjs` with the recorded trees,
seed, process count and baseline revision.
