# Cached f64 cubic: stack-alignment investigation

The JSON artifacts cited here were removed from the working tree to keep the
repository small. Restore one with `git restore --source=696f4c2 -- <path>`.

2026-09-30. Follow-up to the checked-mode Display-P3 regression in
the milestone-three comparison (`rust/reports/multi-gamut-milestone-3-dualray.json`).
Measurements, hashes and prototype diff (`rust/reports/cached-cubic-stack-investigation.json`)
accompany this report. The investigation initially left the solver unchanged;
the borrowed-entry fix has since been applied in `rust/src/rgb_solvers.rs`.

The regression is caused by stack alignment around a by-value cache-entry
copy. A change to the benchmark header's `println!` reduced the generated
`run_gamut::<DisplayP3>` stack allocation from 88 to 72 bytes. This shifted the
cached-cubic timing closure's stack by 16 bytes. On this native Ryzen build,
that change makes its cache-entry copy and subsequent field loads encounter
substantially more store-to-load forwarding conflicts.

The unchanged hot loop contains 969 instructions in 5,188 bytes in both
binaries. Its instructions, constants and call targets match after relocation
normalization. The source of the sensitivity is `OklchCubic::hue_data` in
`rust/src/rgb_solvers.rs`: it returns the cached `HueData` by value. That record
contains thirteen f64 values, or 104 bytes. On a cache hit, the compiler copies
it to the stack using two overlapping 64-byte AVX-512 stores, then loads its
individual fields into further temporaries. Returning a reference removes this
intermediate copy from the warm path.

**The decisive experiment changed only two bytes in each executable.** Both
patches enlarge the caller's stack allocation and matching deallocation;
neither relocates code or static data, nor changes the solver instructions.
All local-variable offsets remain valid in the enlarged frames. These are
diagnostic copies, not proposed production binaries; their unwind metadata
was not updated.

| Executable | Caller allocation | Grid, ns/call | Random, ns/call |
| --- | ---: | ---: | ---: |
| `c2f99ba`, original | 88 bytes | 62.645 | 89.895 |
| Dualray work, original | 72 bytes | 72.885 | 94.200 |
| Dualray work, two-byte patch | 88 bytes | 62.875 | 89.290 |
| `c2f99ba`, reverse alignment experiment | 104 bytes | 73.500 | 94.290 |

Thus restoring the caller's previous allocation removes the regression in the
new binary; shifting the older binary's allocation makes it slow as well.
Separate, untimed instrumentation confirmed that the closure's post-prologue
stack address modulo 64 was respectively 0, 16, 0 and 48 in these four cases.
Validation and checksum output remained identical between each original and
its frame-patched copy.

These are full-harness measurements, medians of two process medians. The
process order was the table's first two variants, patched new, patched old,
then the reverse. Every process used CPU 2, disabled ASLR, the same executable
pathname, `--in-gamut-check`, 35,640 colors per workload, 50 warmup and 25 timed
passes. All output channels were consumed. No builds, tests or other benchmarks
ran alongside the measurements.

The two-byte changes, expressed as ELF virtual addresses, were:

| Binary | Prologue immediate | Epilogue immediate | Byte change |
| --- | --- | --- | --- |
| Before | `0x5748d` | `0x5765e` | `0x58` to `0x68` |
| After | `0x5919d` | `0x5934f` | `0x48` to `0x58` |

The JSON report pins the original and patched binary SHA-256 values. These
addresses apply only to those binaries. Reproduction scripts and frozen
artifacts are in `/tmp/gma-cached-investigation`; `frame-patch.py` translates
the virtual addresses through the ELF load segments, asserts the original
bytes, and writes separate copies. `frame-bench.py` runs the paired comparison.

**Independent control of the stack reproduced the effect.** A C probe loads
the existing Rust executable as a shared object after clearing only its
`DF_1_PIE` flag. It calls the original timing closure with the existing context
layout, workloads and a preallocated hue cache. An assembly wrapper fixes the
caller's stack alignment and selects offsets of 0, 16, 32 or 48 bytes. Both
original binaries show the same fast/slow behavior under this control.

An initial experiment also copied the existing closure to several executable
addresses, repairing its relative references without recompiling its
instructions. Moving that code did not reproduce the large gap. The later
two-byte experiment provides the stronger causal evidence because it preserves
the original executable's code and data placement entirely.

Hardware counters corroborate the stack diagnosis. The following are
process-wide counts for the focused grid probe, including setup, one initial
pass, 50 warmups, 700 measured passes and teardown. All six requested events
ran simultaneously with 100% reported running time. Each counter row is one
run, rather than a statistical performance estimate.

| Focused probe | Cycles | Instructions | Store-to-load conflicts |
| --- | ---: | ---: | ---: |
| Original loop, fast stack alignment | 8,720,546,676 | 22,020,322,604 | 50,050,079 |
| Original loop, slow stack alignment | 10,158,935,494 | 22,020,319,798 | 165,895,885 |
| Borrowed-entry prototype | 8,440,772,523 | 21,913,255,633 | 7,124,972 |

The conflict event was `ls_bad_status2.stli_other`. Sampling that event put
71.44% of the slow run's estimated events in the cached-cubic closure. Its
sampling is imprecise: individual sampled instruction addresses do not prove
the exact instruction responsible, or a particular undocumented CPU forwarding
rule. The controlled stack changes, instruction comparison and disappearance
of the sensitivity after removing the copy establish the actionable cause.

**Recommended change: borrow the cache entry.** A scratch-only prototype uses:

```rust
fn hue_data(&mut self, h: Float) -> &HueData {
    let key = hue_bucket(h);
    if self.cache[key].t_lower == 0.0 {
        self.cache[key] = get_hue_data::<G>(key as Float / 10.0);
    }
    &self.cache[key]
}
```

Its generated warm path reads the required fields directly from the cache,
eliminating the intermediate 104-byte stack copy. The paired focused probe
results below are medians of two process medians, each with 100 measured
passes. Offsets refer to the probe caller's explicitly aligned stack, not the
callee's frame address.

| Caller offset | Original grid | Borrowed grid | Original random | Borrowed random |
| ---: | ---: | ---: | ---: | ---: |
| 0 bytes | 73.24 | 60.95 | 95.06 | 86.57 |
| 16 bytes | 63.45 | 60.85 | 89.15 | 86.37 |
| 32 bytes | 72.99 | 61.08 | 96.93 | 86.61 |
| 48 bytes | 72.52 | 60.77 | 93.80 | 86.50 |

All numbers are ns/call. The prototype passed all 102 native release tests.
The existing 136,960-input P3 compatibility manifest produced 7,121,920
bit-identical outputs across all thirteen methods, both precisions and both
modes. Focused grid and random checksums also matched exactly on every pass.

These investigation findings apply to the AMD Ryzen 7 9800X3D, WSL2/glibc 2.43, rustc 1.98.1 /
LLVM 22.1.8 native release/LTO build. They do not establish performance on
other CPUs, portable x86-64 or JavaScript. The implementation retains the
current numerical checks and cache layout; adding padding to the benchmark
caller would leave the underlying copy sensitivity in place.

The archive comparison also distinguishes the two languages. At archive commit
`1e50de184415588551d0f26780773d6a1cd8eb45`, Rust's
`rust/src/main.rs::OklchCubic::hue_data` returns `HueData` by value, so that
copying pattern predates the multi-gamut refactor. The archived JavaScript
`src/oklch-cubic.js::cachedHueData` returns an index into a flat `Float64Array`;
its caller reads the coefficients directly. The Rust fix follows that
direct-access approach without changing the hue buckets or mapping policy.

The implementation follow-up report (`rust/reports/cached-cubic-borrow-fix.json`) records the
applied fix, current source and binary hashes, archive comparison, and full
before/after timings. All 102 tests pass in native release, portable x86-64
release and debug builds; all-target `--validate-only` passes. A fresh snapshot
compares 136,960 inputs in each target, precision and mode: all 1,643,520 cached
cubic outputs are bit-identical. No accuracy budget, cache shape or mapping
policy changed.

The following full-harness results are before → after, in ns/call, with two
process medians per build, target and mode. Separate target processes run in
forward/reverse gamut and build order, with CPU 2, disabled ASLR, the same
executable pathname and native release/LTO flags. No builds, tests or other
benchmarks run alongside timing. P3 uses its default CLI invocation.

| Target / precision | Plain grid | Plain random | Checked grid | Checked random |
| --- | ---: | ---: | ---: | ---: |
| display-p3 f64 | 56.50 → 48.80 | 73.50 → 69.97 | 72.37 → 61.42 | 95.60 → 86.32 |
| display-p3 f32 | 45.05 → 42.97 | 62.83 → 62.20 | 54.30 → 53.13 | 79.39 → 80.79 |
| srgb f64 | 50.72 → 49.33 | 72.81 → 73.27 | 73.10 → 61.66 | 96.68 → 88.22 |
| srgb f32 | 45.54 → 43.42 | 63.54 → 61.73 | 54.59 → 54.28 | 80.43 → 77.84 |
| rec2020 f64 | 66.91 → 58.46 | 82.78 → 77.56 | 73.05 → 69.54 | 95.12 → 96.29 |
| rec2020 f32 | 50.58 → 49.91 | 65.93 → 64.02 | 59.30 → 58.19 | 86.34 → 83.83 |

The P3 f64 checked grid improves by 15.1% and checked random by 9.7%. The three
cached-cubic rows with increases are sRGB f64 plain random (+0.6%), Rec.2020 f64
checked random (+1.2%), and P3 f32 checked random (+1.8%). Small changes have
limited statistical weight; the report retains individual process medians.

The report also retains all 312 method comparisons. Unchanged kernels showed
some larger timing shifts. In a separate fresh checked-mode pair, sRGB f32
cached Bottosson still increased 35.89 → 38.35 ns (+6.9%); Rec.2020 f32 cached
Bottosson increased 37.26 → 38.67 ns (+3.8%), and Rec.2020 f32 Edge Seeker
increased 38.78 → 39.95 ns (+3.0%). The latter two increases were smaller than
their initial +6.8% and +8.1%. All three timing closures have identical
normalized instructions, constants and branch structure across the builds,
with different addresses. These full-build timing effects are recorded for
follow-up; this investigation does not establish their precise cause or claim
an improvement for every method.
