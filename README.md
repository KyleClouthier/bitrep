# bitrep

**Any order. Any hardware. Same bits.**

Order-invariant, bit-identical floating-point reductions for Rust — exact sums,
dot products and means whose results (and whole accumulator *state*) are
byte-identical regardless of summation order, thread count, shard split,
batch size, SIMD width, or CPU architecture.

[![CI](https://github.com/kyleclouthier/bitrep/actions/workflows/ci.yml/badge.svg)](https://github.com/kyleclouthier/bitrep/actions/workflows/ci.yml)
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
* `#![forbid(unsafe_code)]`, zero runtime dependencies.

## What it costs (honest numbers)

Exactness is not free. On random data expect roughly **an order of magnitude**
over a naive scalar loop (run `cargo bench` for your hardware; the suite
compares naive / Kahan / bitrep and prices shard-merging). Use it where bits
matter — replicated state, signed or hashed outputs, cross-machine
aggregation, ill-conditioned sums — not in your inner render loop.

## Verification

The claim is checked, not asserted:

* **Independent oracle:** property tests sum in `BigInt` at 2⁻¹⁰⁷⁴ resolution
  and round with a separately written IEEE reference — bitrep must match
  bit-for-bit on arbitrary finite inputs, including subnormals and ±MAX.
* **Order/shard invariance:** random permutations and random shardings with
  random merge trees must produce byte-identical state.
* **NIST StRD:** certified means of the [NumAcc1–4 numerical-accuracy
  datasets](https://www.itl.nist.gov/div898/strd/univ/homepage.html) are
  reproduced to the representational limit (LRE ≥ 14.5).
* **Golden cross-arch vectors:** hostile 11k-element datasets (600 decades of
  magnitude, subnormals, exact cancellations) hashed to pinned SHA-256s on
  every CI platform.
* **Miri** on the whole suite; clippy `-D warnings`; rustfmt.

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
