# bitrep

**Any order. Any hardware. Same bits.**

Order-invariant, bit-identical floating-point reductions for Rust — exact
sums and dot products whose results (and whole accumulator *state*) are
byte-identical regardless of summation order, thread count, shard split,
batch size, SIMD width, or CPU architecture.

[![CI](https://github.com/KyleClouthier/bitrep/actions/workflows/ci.yml/badge.svg)](https://github.com/KyleClouthier/bitrep/actions/workflows/ci.yml)
*The badge is the claim: CI computes golden test vectors on x86-64 Linux,
ARM64 macOS, x86-64 Windows and wasm32, and asserts one SHA-256 across all of
them, over multiple permutations and shardings, on every commit.*

**[Try it in your browser](https://simgen.dev/bitrep/)** — the same crate,
compiled to wasm, reproduces the CI-pinned hash on your device, live; then
shuffle the data, shard it, and merge accumulator states across two of your
devices. Your machine is the fifth architecture in the proof.

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

## What this makes possible

Four things that were previously blocked by the same missing property —
float addition whose *state* survives reordering — each demonstrated by a
runnable construction in this repo:

* **Float counter CRDTs** — counter CRDTs have been integer-only for fifteen
  years; the [CRDT section](#bitrep-as-a-crdt-building-block) gives the
  recipe and [`float_gcounter`](examples/float_gcounter.rs) tortures it.
* **Floats in replicated state machines** — replicas that route aggregates
  through an accumulator compute identical bytes; the float ban becomes
  selective instead of total.
* **Authenticated float aggregates** — Merkle trees over exact sums: signed
  totals with O(log n) verifiable updates
  ([`merkle_sum_tree`](examples/merkle_sum_tree.rs)).
* **Worker-count-invariant gradient aggregation** — the same model bytes
  from any number of workers
  ([`deterministic_training`](examples/deterministic_training.rs)).

## Who this is for

Each of these is a real, documented pain — and each was blocked by the same
missing property: float addition whose *state* survives reordering.

* **Replicated state machines.** Replicas that carry float state drift when
  reduction order differs across nodes; deterministic-simulation-testing
  shops famously ban floats for exactly this reason. Order-invariant
  reductions make float aggregates safe to replicate: every replica computes
  the same bytes, and a hash comparison proves it.
* **Distributed aggregation.** Parallel frameworks sum partitions in
  whatever order execution delivers them, so the same job on the same data
  returns different answers run to run — a
  [documented Spark example](https://arxiv.org/abs/2101.09408) computes an
  integral that should be 0 and gets anything from −8192 to +12288. Sum a
  billion numbers on a hundred workers and merge the 289-byte states in
  whatever order they arrive — retries, stragglers and rebalancing stop
  mattering. The combined result is exact and identical no matter how the
  work was split.
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
criterion on x86-64 (mixed magnitudes across ~12 decades; medians; run
`cargo bench` for your hardware). The [`xsum` crate](https://crates.io/crates/xsum)
(Neal's superaccumulator, also exact) is included because it's the honest
comparison, fed through its fast path (`add_list`, size-recommended variant):

| n | naive | Kahan | xsum | **bitrep** | vs naive | vs Kahan | vs xsum |
|---|---|---|---|---|---|---|---|
| 1,000 | 368 ns | 1.58 µs | 1.52 µs | **1.82 µs** | 4.9× | 1.2× | 1.2× |
| 100,000 | 40.8 µs | 163 µs | 137 µs | **395 µs** | 9.7× | 2.4× | 2.9× |
| 1,000,000 | 409 µs | 1.65 ms | 1.36 ms | **4.20 ms** | 10.3× | 2.5× | 3.1× |
| merge 100 shards of 10k | — | — | — | **1.35 µs total** | shard-combining is effectively free |

Read the xsum column honestly: for raw single-machine exact sums at large n,
**xsum is ~3× faster** — if that's your whole problem, use xsum. bitrep's
price buys the properties xsum doesn't offer: a mergeable, serializable,
canonically-encoded accumulator state (the distributed contract above),
exact f32 and dot products, and the cross-architecture proof harness.
Against Kahan — the compensated summation people already pay for accuracy
alone — bitrep is ~1.2–2.5× and is *exact*, *order-invariant*, and
*mergeable*. Use it where bits matter — replicated state, signed or hashed
outputs, cross-machine aggregation, ill-conditioned sums — not in your inner
render loop.

**v0.2 adds `FastSumF64`**, a streaming front-end using Neal's
small-accumulator technique (the same algorithm family as xsum) that finishes
into the *same canonical bytes* — verified differentially against the direct
path on every test run. Measured: ~800 Melem/s at n=1k (xsum-parity+) and
~370 Melem/s at n≥100k (+45% over `SumF64::add`; xsum's large-n variant
remains ~2× faster there — its radix-by-exponent batching is future work).
And because merge order is free, **parallel exact summation scales with zero
determinism caveats**: `examples/parallel_sum.rs` measures ~1.2 Gelem/s on
four threads — byte-identical for every thread count, which no naive
parallel sum can say.

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

The construction's convergence laws are machine-checked in
[`proofs/FloatGCounter.lean`](proofs/FloatGCounter.lean): the count-wins
join is a semilattice (commutative, associative, idempotent), folding *any*
delivery schedule — any order, any duplicates — yields the same state, and
the converged read equals the exact sum of every add that ever happened.
For calibration: existing counter CRDTs are integer-valued (Redis
Active-Active documents 59-bit integer counters; Akka and Riak counters are
integers), and the mechanized-CRDT literature (e.g. the Isabelle/HOL
framework of Gomes et al., OOPSLA'17) verifies integer counters — an
*exact float* replicated aggregate needs exactly the merge properties float
addition lacks and bitrep restores.

## Convergent statistics (feature `stats`, v0.2)

The counter construction generalizes to a statistics algebra. Any statistic
whose sufficient state is a set of *exact monomial sums* (Σx, Σx², Σx³, Σx⁴,
Σxy) inherits the whole contract — and the read is computed from the exact
integer state in big-integer arithmetic with **one** final
round-to-nearest-even, so it is the **correctly rounded value of the true
statistic**, bit-identical across any sharding, arrival order, or merge tree:

* [`MomentsF64`] — exactly rounded `mean`, `variance` (population & sample);
  `stddev` (one extra IEEE-`sqrt` rounding, still bit-invariant);
* [`Moments4F64`] — adds **exactly rounded kurtosis** (μ₄/μ₂² is a pure
  rational of the state — the n and unit factors cancel) and skewness;
* [`CovF64`] — exactly rounded covariance, least-squares `slope`,
  `intercept`, and `R²`; correlation via one IEEE `sqrt`.

Why this beats the classical art: Chan/Golub/LeVeque parallel moments (the
standard since 1979) are *algebraically* exact but computed in floats — the
bits depend on the merge tree, and the merge double-counts on re-delivery.
These states are bit-invariant, honestly bounded (`StatsError` reports
overflow/underflow of the two-product domain — never a silent wrong value),
and CRDT-lawful under the same per-replica map layer
(`examples/convergent_stats.rs` checks the laws and demonstrates a variance
the textbook formula returns as *negative* — exactly rounded here). Every
read is verified in CI against an independent big-integer oracle with a
neighbor-comparison correct-rounding check (`tests/stats.rs`).

Named limits, stated: products must stay clear of overflow and the subnormal
range (|x| ≲ 1.3e154 for squares; 3rd/4th moments narrow it further) —
violations are detected and reported. Order statistics (median, quantiles)
and arrival-order-dependent aggregates (EWMA) are outside this family.

The rest of the toolkit rounds out what real aggregation needs, all under
one [`Mergeable`] trait so containers and transports are generic:

* [`WeightedMomentsF64`] — exactly rounded weighted mean/variance (weights
  travel with samples, so timestamp-derived weights stay order-invariant);
* [`PnMomentsF64`] — **exact retraction** (`add`/`remove`, PN-counter
  style): insert-then-delete returns reads to byte-identical values — the
  incremental-view-maintenance primitive;
* [`CovMatrixF64`] — exact covariance *matrices* and deterministic multiple
  linear regression (normal equations over exactly rounded entries;
  fixed-pivot solve — bit-invariant, honestly not exactly rounded);
* [`ExtremaF64`] — exact min/max (`no_std`, idempotent by nature);
* [`HistogramF64`] — fixed-bucket exact counts with honest quantile
  *bounds* (order statistics have no exact mergeable form — stated, not
  worked around);
* [`ConvergentMap`] — keyed states: `GROUP BY`, tumbling windows,
  per-metric fleets; [`Replicated`] — the lawful per-replica CRDT layer,
  generic over any state; [`Deltas`] — delta-state transport
  (Almeida–Shoker–Baquero style);
* `state_hash` (feature `receipts`) — the canonical 32-byte commitment for
  signing converged aggregates.

## Demos that assert

Two runnable constructions in [`examples/`](examples/) — each is a probe
that would have failed loudly if the property it rests on were weaker than
claimed:

* **`cargo run --example float_gcounter`** — the counter CRDT above,
  tortured: 8 replicas, 300 random gossip schedules with duplicate and stale
  delivery, hostile values (subnormals, exact cancellations). Every replica
  converges byte-identically and every total equals the exactly rounded sum.
  The built-in contrast: re-summing the *same* converged entries forward vs
  backward in naive f64 disagreed in 184/300 schedules — exactness is
  load-bearing, not decorative.
* **`cargo run --example merkle_sum_tree`** — authenticated float
  aggregates: a Merkle tree whose nodes carry merged accumulator states, so
  the root commits to every leaf *and* the exact total. Change one leaf in a
  4096-leaf total and recompute O(log n) nodes — byte-identical to a full
  rebuild; verify any leaf against the root with 12 hashes. Meaningless with
  ordinary float sums (no canonical bytes to hash); routine with bitrep.
* **`cargo run --release --example deterministic_training`** — bit-identical
  data-parallel training. The gradient all-reduce is a float sum whose order
  depends on worker count, so the "same" SGD run yields different model
  bytes at 1 vs 4 vs 16 workers even in pure f64 — measured here: 4 worker
  configurations, 4 distinct naive-f64 models, **1** identical bitrep model.
  Named limit: this fixes the *reduction*; batch-invariant worker kernels
  are the other half of the problem and are not claimed.

## Verification

The claim is proved, checked, fuzzed, and cross-examined — each by an
independent method, so no single mistake can hide:

| Layer | Tool | What it establishes |
|---|---|---|
| **Proof (math)** | **Lean 4** ([`proofs/`](proofs/), zero `sorry`, axiom-audited in CI) | Order/merge-tree/permutation invariance of exact accumulation; the rounding kernel is round-to-nearest-ties-to-even in full (half-ulp bound, minimality over *every* grid point, tie parity, exactness); the float-G-Counter convergence laws; and the toolkit merge algebra ([`proofs/ToolkitAlgebra.lean`](proofs/ToolkitAlgebra.lean)): products, per-key maps, min/max and boolean joins, saturating counters — the laws every v0.2 state instantiates |
| **Proof (bits)** | **Kani / CBMC** ([`src/kani_proofs.rs`](src/kani_proofs.rs)) | The Rust implementation's merges commute and associate and the codecs round-trip — for **all** inputs, symbolically, proven on every push (six harnesses: sum merge/codec + extrema merge laws/codec). Kani's first catch on v0.2: adversarial `ExtremaF64` decodes broke merge commutativity — the decoder now rejects non-canonical states. The add-path harnesses (add commutes, exact cancellation) decompose a symbolic f64 across all 34 limbs and are beyond CBMC's practical reach (did not close in ~3h on CI), so they're `kani_slow`-gated for local runs; those properties are proved at the model level in **Lean** and exercised by the oracle tests and the fuzzer |
| **Differential fuzzing** | cargo-fuzz vs a BigInt oracle | 290M+ executions hunting order variance, oracle disagreement, codec breakage. Catches so far: a real `count`-overflow bug (fixed), a bug in **its own oracle** (`powi(-1067)` = 1/∞ = 0 — the crate was right), and on v0.2 a length-prefix overflow in the `CovMatrixF64` decoder, found in under a minute of fuzzing the new [`toolkit_decoders`](fuzz/fuzz_targets/toolkit_decoders.rs) target (fixed, with the crashing input kept in-tree under fuzz/artifacts as a regression record) |
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
and Kahan summation solve related problems with different trade-offs. The
closest database-side work is
[reproducible aggregation in RDBMSs](https://arxiv.org/abs/1802.09883)
(ICDE'18) — single-node GroupBy reproducibility, without a mergeable or
serializable accumulator state.

What `bitrep` adds is the *packaging for distributed systems*: a mergeable,
serializable, canonically-encoded accumulator state with breadth beyond sum
(f32, dot), a named-limits API that refuses to be silently wrong, and a
CI harness that proves bit-identity across architectures on every commit.
(An exactly rounded `mean()` — one correct rounding of the exact sum divided
by the count — is planned; means today are `value()/count`, one extra
rounding, which is how the NIST means below are reproduced.)
If you need raw single-machine exact-sum speed, `xsum` is ~3× faster at
large n (measured above) — pick per workload.

## Non-goals

Making your *existing* pipeline bit-reproducible (that depends on your
kernels' order — see batch-invariant kernels for that approach); general
arbitrary-precision arithmetic; being the fastest sum on one machine.

## License

MIT or Apache-2.0, at your option.
