# Changelog

## 0.4.0 — 2026-07-14

* `RelSketch` (feature `quantile`) — a reproducible, byte-identical,
  relative-error quantile sketch (DDSketch-family). Integer bit-shift bucket
  mapping (no `libm log`), so buckets are identical across architectures; the
  canonical state is order/shard/merge-invariant and signable via `state_hash`.
  Delta-varint bucket encoding (~2.7 bytes/bucket, ~6× smaller than a flat
  layout); an order-invariant collapse policy bounds memory under a hostile
  all-exponent stream. OpenTelemetry / Prometheus exponential-histogram
  correspondence (`otel_scale`/`from_otel`, `examples/otel_bridge.rs`).
* Verification for the sketch: merge laws machine-checked in Lean 4
  (`proofs/RelSketchMerge.lean`, wired through the comparator — now 37 audited
  theorems); an adversarial red-team suite and a `quantile_decode` fuzz target;
  accuracy/size tests on both realistic synthetic latency and a **committed,
  license-clean real dataset** (6 421 NASA-HTTP Jul 1995 response sizes,
  `tests/data/`), validated hermetically in CI for accuracy **and** byte-identity
  under reordering/sharding; and a second-language conformance reference
  (`conformance/relsketch_ref.py`) reproducing the bytes byte-for-byte.
* `RelSketch` equality is now **bitwise on every field** (`min`/`max` compared
  via `f64::to_bits`, matching `ExtremaF64`) instead of the `derive`d IEEE-value
  comparison. The old comparison made a decoded state carrying a `NaN` extremum
  unequal to itself — so `from_bytes(m.to_bytes()) == Some(m)` could fail after a
  merge even though the bytes round-tripped perfectly (found by the
  `quantile_decode` fuzz target) — and made byte-distinct `+0.0`/`−0.0` extrema
  compare equal. Equality now mirrors `to_bytes` exactly; byte-identity and the
  canonical encoding are unchanged.
* `RelSketch` decoders now reject bucket keys outside the reachable key space
  (a key above `key_of(f64::MAX)` can never be produced by `add`/collapse).
  Such a key was non-canonical and overflowed the signed OpenTelemetry index
  arithmetic in `otel_positive_indices`/`otel_negative_indices` (found by the
  `quantile_decode` fuzz soak). Both `from_bytes` and `from_otel` enforce the
  ceiling; `from_otel` now also honors its documented "outside the key space"
  contract. Every accepted state round-trips through `from_bytes`.

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
