# Changelog

## 0.5.1 — 2026-07-17

* JavaScript/wasm bindings reach exact-tier parity with Python:
  `SumF64.try_unmerge`, `CovMatrixF64.regression_exact`,
  `CovMatrixF64.sub` now exposed on npm. Binding READMEs updated.
  No change to the Rust core or the byte format.

## 0.5.0 — 2026-07-17

**The exact tier: group subtraction and correctly rounded regression.**

* `SumF64::try_unmerge` — exactly remove a previously merged contribution.
  The limb accumulator is two's-complement, so finite states form an abelian
  group and subtraction is exact: after `a.merge(b); a.try_unmerge(b)` the
  state is byte-identical to never having merged. Refuses (state untouched)
  subtrahends carrying NaN/infinity flags — those are sticky semilattice
  flags, not group elements — or counts larger than the minuend's.
* `CovMatrixF64::try_sub` — all-or-nothing exact downdating of the full
  second-moment state: remove any contributor's rows, at any fraction of the
  data and any conditioning, exactly. This is the state-level primitive for
  leave-one-out diagnostics, influence analysis, and verifiable unlearning
  of least-squares models.
* `CovMatrixF64::try_regression_exact` / `regression_exact` — the **exact
  tier** above the existing deterministic tier: normal-equation entries are
  taken from the state's exact integers (no rounding), the system is solved
  by Cramer's rule with fraction-free Bareiss determinants (exact integer
  arithmetic throughout), and each coefficient is rounded once, correctly
  (nearest, ties-to-even). The returned bits are a mathematical function of
  the data alone — identical on any machine and any implementation. Where
  ill-conditioning makes f64 solves (including QR) fail or drift, this
  returns the exact solution's correct rounding, and reports exact
  singularity as `Degenerate` rather than a blurred answer.
* Verification, matched claim-for-claim: the merge/unmerge group inverse is
  **Kani-verified** at the bit level for all valid states
  (`unmerge_inverts_merge`, `unmerge_refusal_leaves_state_untouched`), and
  proved at the model level in **Lean** (`unmerge_inverts_merge`,
  `lsum_unmerge`; zero `sorry`, standard axiom base). `regression_exact`
  (BigInt arithmetic, outside Kani's reach) is verified by an **independent
  exact rational oracle** — a different algorithm (plain rational Gaussian
  elimination) bit-compared across randomized systems — and a new
  coverage-guided fuzz target (`sub_roundtrip`) exercising the group laws
  end to end. During development the fuzzer correctly flagged that finite
  inputs can still produce underflow-flagged product states, which `try_sub`
  rightly refuses; the refusal contract (decline ⇒ state untouched) is part
  of the tested surface.
* Python bindings: `SumF64.try_unmerge`, `CovMatrixF64.sub`,
  `CovMatrixF64.regression_exact`. Byte format unchanged; all existing
  golden/conformance vectors unaffected.

## 0.4.2 — 2026-07-16

* Python and JavaScript/wasm bindings now expose `count()` on the accumulator
  and statistics types (`SumF64`, `SumF32`, `FastSumF64`, `MomentsF64`,
  `Moments4F64`, `CovF64`, `WeightedMomentsF64`) — the number of values
  accumulated, already part of the serialized state and long available on the
  Rust core. This closes a binding gap where callers had to infer the count
  indirectly (which was unreliable for constant-valued data). No change to the
  Rust core, the byte format, or any golden/conformance vector; the wasm
  accessor returns a JS `BigInt`.

## 0.4.1 — 2026-07-14

* Docs & polish only, no functional change. README now leads with the full
  surface — sums, dot products, statistics, and reproducible quantiles
  (`RelSketch`) — and the "sign a p99" line, instead of just sums/dot products.
  Example doc headers reworded (`Probe:` → `Example:`); the toolkit test's
  orphaned `v0.3` tag dropped; the key-space regression test reformatted.

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
