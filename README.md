# bitrep

**Any order. Any hardware. Same bits.**

Order-invariant, bit-identical floating-point reductions for Rust — exact sums,
dot products and means whose results (and whole accumulator *state*) are
byte-identical regardless of summation order, thread count, shard split,
batch size, SIMD width, or CPU architecture.

[![CI](https://github.com/DigitalMax321/bitrep/actions/workflows/ci.yml/badge.svg)](https://github.com/DigitalMax321/bitrep/actions/workflows/ci.yml)
*The badge is the claim: CI computes golden test vectors on x86-64 Linux,
ARM64 macOS, x86-64 Windows and wasm32, and asserts one SHA-256 across all of
them, over multiple permutations and shardings, on every commit.*

## Why

Floating-point addition isn't associative, so the *order* of a reduction
changes the answer. Parallelism, SIMD, sharding and batch size all change the
order. That's why your replicas drift, your temperature-0 LLM gives different
answers under load, your distributed aggregates won't hash the same twice,
and your lockstep game desyncs across platforms.

`fp64` fixes your *decisions* — more precision makes the wrong bits smaller.
**It can't fix your *hashes*** — if you sign, hash, replicate or compare
results, "smaller error" is still a different byte string.

`bitrep` accumulates floats into a fixed-point superaccumulator (a 2176-bit
integer in units of 2⁻¹⁰⁷⁴) that holds every finite `f64` **exactly**. Integer
addition is associative and commutative, so the state is order-invariant *by
construction* — not by kernel discipline. One correct rounding happens at the
end (nearest, ties-to-even).

## The distributed contract

Accumulators **merge** and **serialize**. Sum shards on different machines,
ship the 289-byte states anywhere, merge in any order — the bytes come out
identical, and the value is the exactly rounded sum of everything:

```rust
use bitrep::SumF64;

let data = [0.5_f64, 1e100, -1e100, 0.25, 0.125, -0.875, 1e-300];

let sequential: SumF64 = data.iter().copied().collect();

let (a, b) = data.split_at(3);              // "two machines"
let mut left: SumF64  = a.iter().copied().collect();
let right:    SumF64  = b.iter().copied().collect();
left.merge(&right);                          // any merge tree, any order

assert_eq!(sequential.to_bytes(), left.to_bytes());   // identical state
assert_eq!(sequential.value(), 1e-300);               // exactly rounded
// naive summation returns 0.0 here — the 1e-300 is annihilated by 1e100
```

Also in the box:

* **`SumF32`** — exact `f32` sums, rounded *once* from the exact state
  (immune to the classic double-rounding-through-f64 trap; there's a test
  that proves the trap on your machine, then dodges it).
* **`DotF64`** — exact, order-invariant dot products via FMA two-products.
  *Named limit:* partial products that underflow below the normal range lose
  exactness — this is **detected per pair and reported**
  (`is_exact()` / `try_value()`), never silent.
* **`serde`** (optional feature) — accumulators serialize as their canonical
  bytes in any format.
* **`no_std`** — sums work without std (dot needs `std` for `mul_add`).
* **A language-neutral format** — [`FORMAT.md`](FORMAT.md) specifies the
  289-byte state; a pure-Python reference implementation in
  [`conformance/`](conformance/) reproduces the Rust crate **byte-for-byte**
  from that spec alone. Shard in Python, merge in Rust, verify anywhere.
* `#![forbid(unsafe_code)]`, zero runtime dependencies.

## Who this is for

* **Replicated state machines.** Replicas that carry float state drift when
  reduction order differs across nodes; deterministic-simulation-testing
  shops famously ban floats for exactly this reason. Order-invariant
  reductions make float aggregates safe to replicate: every replica computes
  the same bytes, and a hash comparison proves it.
* **Distributed aggregation.** Sum a billion numbers on a hundred workers
  and merge the 289-byte states in whatever order they arrive — retries,
  stragglers and rebalancing stop mattering. The combined result is exact
  and identical no matter how the work was split.
* **Anything you sign, hash, or audit.** "This total came from these
  inputs — verify it yourself" only works if recomputation is bit-identical.
  bitrep gives float pipelines the property that makes signatures and
  content-addressing meaningful.
* **Reproducible ML and science.** Batch size, thread count and hardware
  change reduction order, which is why temperature-0 LLMs answer differently
  under load. Batch-invariant kernels pin the order; bitrep removes the
  order from the equation entirely for the reductions you route through it.
* **Lockstep and rollback netcode.** Cross-platform float determinism has
  been a two-decade pain in game networking. A deterministic reduction for
  scores, physics aggregates and state checksums removes a whole class of
  desyncs.
* **Regulated computation.** When an auditor asks "prove this number,"
  an exact, replayable, byte-stable aggregation is the difference between
  an argument and a receipt.

## What it costs (honest, measured numbers)

Exactness is not free — but it's cheaper than its reputation. Measured with
criterion on x86-64 (mixed magnitudes across ~12 decades; run `cargo bench`
for your hardware):

| n | naive | Kahan | **bitrep** | vs naive | vs Kahan |
|---|---|---|---|---|---|
| 1,000 | 395 ns | 1.67 µs | **2.03 µs** | 5.1× | 1.2× |
| 100,000 | 44.2 µs | 171 µs | **435 µs** | 9.8× | 2.5× |
| 1,000,000 | 440 µs | 1.71 ms | **4.58 ms** | 10.4× | 2.7× |
| merge 100 shards of 10k | — | — | **1.6 µs total** | shard-combining is effectively free |

So: ~5–10× a naive loop, and only ~1.2–2.7× Kahan — the compensated
summation people already pay for accuracy alone, except this one is *exact*,
*order-invariant*, and *mergeable*. Still ~220 million elements/second on
one core. Use it where bits matter — replicated state, signed or hashed
outputs, cross-machine aggregation, ill-conditioned sums — not in your inner
render loop.

## bitrep as a CRDT building block

Integer counters have had conflict-free replicated types (G-Counter,
PN-Counter) for fifteen years. Float sums never did, because the construction
requires merge to be commutative and associative — and float addition is
neither. bitrep restores exactly those two properties (machine-checked in
Kani, proved at the model level in Lean), which makes an **exact float
counter CRDT** the standard recipe:

* each replica keeps its own accumulator and only ever `add`s to it
  (append-only, so a replica's states are totally ordered by `count`);
* the replicated object is a map `replica-id -> accumulator state`, merged
  per-entry by **highest count wins** (idempotent, monotone — a join);
* the value anyone reads is the `merge` of all entries — exact,
  order-invariant, and byte-identical on every converged replica.

Stated honestly: `SumF64::merge` alone is *not* idempotent (merging the same
shard twice double-counts, like adding any counter twice) — deduplication is
the map layer's job, same as every counter CRDT. What bitrep contributes is
the part that was actually missing for floats: a deterministic, exact,
commutative-associative merge, plus a canonical byte encoding so replicas
can prove convergence with a hash instead of an epsilon.

## Verification

The claim is proved, checked, fuzzed, and cross-examined — each by an
independent method, so no single mistake can hide:

| Layer | Tool | What it establishes |
|---|---|---|
| **Proof (math)** | **Lean 4** ([`proofs/`](proofs/), zero `sorry`, axiom-audited in CI) | Order/merge-tree/permutation invariance of exact accumulation, and the rounding kernel is round-to-nearest-ties-to-even in full: half-ulp bound, minimality over *every* grid point, tie parity, exactness |
| **Proof (bits)** | **Kani / CBMC** ([`src/kani_proofs.rs`](src/kani_proofs.rs)) | The Rust implementation's add commutes, cancellation is exact, merges commute and associate, and the codec round-trips — for **all** inputs, symbolically, not sampled |
| **Differential fuzzing** | cargo-fuzz vs a BigInt oracle | 290M+ executions hunting order variance, oracle disagreement, codec breakage. Its first two catches: a real `count`-overflow bug (fixed) and a bug in **its own oracle** (`powi(-1067)` = 1/∞ = 0 — the crate was right) |
| **Independent oracle** | proptest + `BigInt` + a separately written IEEE reference rounding | Correct rounding on arbitrary finite inputs, subnormals and ±MAX included; f32 rounds once (no double-rounding) |
| **Real datasets** | [NIST StRD NumAcc1–4](https://www.itl.nist.gov/div898/strd/univ/homepage.html) | Certified means reproduced to the representational limit (LRE ≥ 14.5) |
| **Cross-architecture** | golden SHA-256 vectors in CI | Identical hashes on x86-64 Linux, ARM64 macOS, x86-64 Windows and wasm32, over permutations and shardings, every commit |
| **Cross-language** | [`FORMAT.md`](FORMAT.md) + pure-Python reference ([`conformance/`](conformance/)) | A second implementation in a second language reproduces the canonical bytes and rounded values exactly, from a spec — the format, proven portable |
| **Hygiene** | Miri, clippy `-D warnings`, rustfmt, MSRV 1.74, `forbid(unsafe_code)`, zero runtime deps | The boring foundations |

The honest division of labor: Lean proves the *algorithm's mathematics*,
Kani checks the *Rust bits*, the oracle and NIST check the *encoding
plumbing*, the golden vectors tie all of it to *hardware reality*, and the
Python reference proves the *format* stands on its own. No single layer is
asked to carry a claim it can't.

## Prior art (stand on shoulders, cite them)

The long-accumulator idea is classic: Kulisch's accumulator, [Neal's
superaccumulators](https://arxiv.org/abs/1505.05571) (see the [`xsum`
crate](https://crates.io/crates/xsum) for a direct port), Demmel–Nguyen /
[ReproBLAS](https://bebop.cs.berkeley.edu/reproblas/) reproducible BLAS, and
Ogita–Rump–Oishi error-free transformations. Shewchuk's adaptive arithmetic
and Kahan summation solve related problems with different trade-offs.

What `bitrep` adds is the *packaging for distributed systems*: a mergeable,
serializable, canonically-encoded accumulator state with breadth beyond sum
(f32, dot, mean), a named-limits API that refuses to be silently wrong, and a
CI harness that proves bit-identity across architectures on every commit.
If you need raw single-machine exact-sum speed, benchmark `xsum` against this
crate and pick per workload.

## Non-goals

Making your *existing* pipeline bit-reproducible (that depends on your
kernels' order — see batch-invariant kernels for that approach); general
arbitrary-precision arithmetic; being the fastest sum on one machine.

## License

MIT or Apache-2.0, at your option.
