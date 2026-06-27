# Native (Rust) benchmark

A native point of reference for the JS `gma-benchmark`, timed over the identical
35,640-color grid (`oklch(L 0.4 H)`).

## `gma-bench` — scalar, apples-to-apples

One color per call, same algorithms and same f64 conversion math as the JS
methods. Anchors "how much of the JS cost is the language/JIT vs the work."
The edge-seeker LUT is generated once by JS and embedded as `src/lut.rs`, so
only the per-call runtime is ported (the LUT build is irrelevant to timing).

```sh
RUSTFLAGS="-C target-cpu=native" cargo build --release --bin gma-bench
./target/release/gma-bench
```

It prints checksums (sum of all output channels) that match the JS port:
`clip` and `edge-seeker` bit-for-bit, `oklch-cubic` to ~3e-6 total (cbrt/acos
last-ULP differences between V8's libm and Rust's).

