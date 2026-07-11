# Changelog

## 0.1.0 — 2026-07-11

Initial release.

* `SumF64` — exact, order-invariant, mergeable f64 sum with a canonical
  289-byte serialized state (nearest-even rounding, once, at the end).
* `SumF32` — exact f32 sums rounded once from the exact state (immune to
  double rounding through f64).
* `DotF64` / `dot()` — exact order-invariant dot products via FMA
  two-products; underflow below the normal range is detected per pair and
  reported (`is_exact()` / `try_value()`), never silent.
* Optional `serde` (canonical bytes in any format); `no_std` sums;
  `#![forbid(unsafe_code)]`; zero runtime dependencies.
* [`FORMAT.md`](FORMAT.md) — language-neutral state encoding, with a
  pure-Python reference implementation that reproduces the Rust crate
  byte-for-byte.
* Verification: Lean 4 proofs (order invariance + rounding kernel), Kani
  symbolic checks, differential fuzzing vs a BigInt oracle, NIST StRD
  datasets, golden cross-architecture SHA-256 vectors in CI (x86-64 Linux,
  ARM64 macOS, x86-64 Windows, wasm32).
