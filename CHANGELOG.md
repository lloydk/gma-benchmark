# Changelog

## 2026-09-28

- Fixed Rust and JS/Bun benchmark loops to consume all three encoded RGB
  channels for every color. Previously, only red was consumed, allowing LLVM
  to remove green/blue output conversion and gamma encoding from the Rust
  timing path. Rust now uses per-pass input and checksum barriers, and the JS
  harness prints its accumulated checksum. Release assembly confirmed that
  the corrected Rust path retains all three gamma branches. Historical timing
  tables are flagged for remeasurement.
