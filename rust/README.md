# Native (Rust) benchmark

A native point of reference for the JS `gma-benchmark`, timed over the same two
35,640-color workloads: the canonical grid (`oklch(L 0.4 H)`) and a random
hue/lightness workload (stratified/jittered, shuffled).

## `gma-bench` — scalar, apples-to-apples

One color per call, same algorithms and same f64 conversion math as the JS
methods. Anchors "how much of the JS cost is the language/JIT vs the work."
The edge-seeker LUT is generated once by JS and embedded as `src/lut.rs`, so
only the per-call runtime is ported (the LUT build is irrelevant to timing).

```sh
RUSTFLAGS="-C target-cpu=native" cargo build --release --bin gma-bench
./target/release/gma-bench

# time the in-gamut-precheck variant of every method instead:
./target/release/gma-bench --in-gamut-check
```

It prints checksums (sum of all output channels) that match the JS port:
`clip` and `edge-seeker` bit-for-bit, and the cubic variants to a few last-place
digits (from cbrt/acos libm differences). `oklch-cubic-direct` currently matches
across JS and Rust to all 10 printed decimal places.

The timed passes use the same all-channel checksum as validation. Each pass's
input slice goes through `black_box`, and its checksum is consumed through
`black_box` before the timer stops. A separate validation checksum does not
protect the timed loop: the previous red-only timing sink allowed LLVM to
remove green/blue output conversion and gamma encoding. Timings collected with
that sink undercounted the work; rerun comparisons with the corrected harness.

Verified with rustc 1.98.1 / LLVM 22.1.8, `target-cpu=native`, release/LTO on
an AMD Ryzen 7 9800X3D: the old timed `clip` loop contains only the red matrix
row and its gamma branch. The corrected pass contains all three matrix rows,
three gamma branches, and an RGB sum feeding the consumed checksum. The
checksum barrier appears before `Instant::elapsed` in the generated code.
This is a check of that build; [`black_box`](https://doc.rust-lang.org/std/hint/fn.black_box.html)
is a best-effort compiler barrier. To emit assembly for inspection:

```sh
RUSTFLAGS="-C target-cpu=native" cargo rustc --release --bin gma-bench -- --emit=asm
# Inspect target/release/deps/gma_bench-*.s, starting at run_timings.
```
