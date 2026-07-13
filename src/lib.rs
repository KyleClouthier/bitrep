// Copyright (c) 2026 Kyle Clouthier / Clouthier Simulation Labs. Licensed under MIT OR Apache-2.0.
//! # bitrep — Any order. Any hardware. Same bits.
//!
//! Order-invariant, **bit-identical** floating-point reductions.
//!
//! Add floats in any order, on any number of threads, across any shard split,
//! on any architecture — the result is the **exactly rounded** sum, and its
//! bytes are identical everywhere. `fp64` fixes your decisions; it can't fix
//! your hashes. This crate fixes your hashes.
//!
//! ## How
//!
//! A [`SumF64`] is a fixed-point *superaccumulator* (a 2176-bit two's-complement
//! integer, in units of 2⁻¹⁰⁷⁴) wide enough to hold every finite `f64` exactly,
//! with headroom for 2⁶³ additions. Integer addition is associative and
//! commutative, so the accumulated state — not just the rounded result — is
//! independent of insertion order *by construction*. One rounding happens at
//! the very end, and it is correct rounding (round-to-nearest, ties-to-even).
//!
//! This is the classic long-accumulator idea (Kulisch; see also Neal's
//! superaccumulators, arXiv:1505.05571, and Demmel–Nguyen reproducible
//! summation). The primitives are textbook; what this crate packages is the
//! **distributed contract**: accumulators are [mergeable](SumF64::merge) and
//! [serializable](SumF64::to_bytes), so shards computed on different machines
//! combine — in any order — into the same bytes.
//!
//! ## Example
//!
//! ```
//! use bitrep::SumF64;
//!
//! let data = [0.5_f64, 1e100, -1e100, 0.25, 0.125, -0.875, 1e-300];
//!
//! // Sequential, reversed, and sharded-then-merged: identical state, identical bits.
//! let a: SumF64 = data.iter().copied().collect();
//!
//! let mut b = SumF64::new();
//! for x in data.iter().rev() { b.add(*x); }
//!
//! let (left, right) = data.split_at(3);
//! let mut c: SumF64 = left.iter().copied().collect();
//! c.merge(&right.iter().copied().collect::<SumF64>());
//!
//! assert_eq!(a.to_bytes(), b.to_bytes());
//! assert_eq!(a.to_bytes(), c.to_bytes());
//! assert_eq!(a.value().to_bits(), b.value().to_bits());
//! // And the value is the exactly rounded sum (naive summation is not):
//! assert_eq!(a.value(), 1e-300);
//! ```
//!
//! ## What becomes possible
//!
//! Each of these was previously blocked by one missing property — order-proof
//! float addition with a mergeable, canonically-encoded state:
//!
//! * **Float counter CRDTs.** Counter CRDTs (G-Counter, PN-Counter) have been
//!   integer-only for fifteen years because CRDT merge must commute and
//!   associate, and float addition does neither. [`SumF64::merge`] restores
//!   both, exactly — the standard counter recipe now works for floats, with
//!   convergence provable by hash instead of epsilon. The construction's
//!   convergence laws (join semilattice, delivery-schedule invariance, exact
//!   reads) are machine-checked in `proofs/FloatGCounter.lean`; the README's
//!   CRDT section gives the recipe.
//! * **Floats in replicated state machines.** Deterministic-simulation-testing
//!   shops ban floats in replicated state because reduction order differs
//!   across replicas and the states drift. Aggregates routed through an
//!   accumulator produce identical bytes on every replica.
//! * **Retry-immune distributed aggregation.** Parallel frameworks sum
//!   partitions in whatever order execution happens to deliver, so the same
//!   job can return different answers run to run. Merge order stops mattering:
//!   any merge tree, any retry, any straggler — same bytes, exactly rounded.
//! * **Numeric outputs you can sign or content-address.** "Recompute this
//!   anywhere and the hash matches" is the property that makes signatures,
//!   receipts and content-addressing meaningful for float pipelines — and it
//!   holds across languages, via the canonical encoding (`FORMAT.md`).
//!
//! ## What this costs (honest numbers)
//!
//! Exactness is not free: expect roughly an order of magnitude over a naive
//! scalar loop for random data (see `benches/`, run them on your hardware).
//! Use it where bit-identity or exactness *matters* — replicated state,
//! signed/hashed outputs, cross-machine aggregation, ill-conditioned sums —
//! not in your inner render loop.
//!
//! ## Scope and named limits
//!
//! * [`SumF64`] / [`SumF32`]: exact, order-invariant sums. `no_std` compatible.
//! * [`FastSumF64`]: a high-throughput streaming front-end (Neal's
//!   small-accumulator technique) that finishes into the same canonical
//!   [`SumF64`] bytes — differentially verified against the direct path.
//! * [`DotF64`] (feature `std`): exact dot products via FMA two-products.
//!   **Named limit:** each partial product `a*b` must not overflow, and must
//!   not fall in the range where FMA two-products lose exactness
//!   (|a·b| < ~2⁻⁹⁶⁹); see [`DotF64`] docs. Inputs outside that domain are
//!   detected and reported, never silently wrong.
//! * [`MomentsF64`] / [`Moments4F64`] / [`CovF64`] (feature `stats`):
//!   convergent statistics — mergeable, order-invariant moment states with
//!   **exactly rounded** reads (mean, variance, kurtosis, covariance,
//!   regression slope/intercept, R²), derived from the exact integer state
//!   with a single final rounding.
//! * NaN/±∞ are tracked as flags (any NaN, or +∞ and −∞ together, yields NaN;
//!   a single infinity sign is preserved). An exactly-zero sum returns `+0.0`
//!   (canonical zero: `-0.0` inputs are sign-preserving in IEEE addition only
//!   for empty-ish cases; a canonical result keeps bytes stable).
//!
//! ## Non-goals
//!
//! Reproducing *your existing* float pipeline bit-for-bit (that depends on
//! your kernels' order); this crate replaces order-sensitive reductions with
//! order-free ones. It is also not a general bignum: it holds sums of floats,
//! nothing else.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod acc;
#[cfg(feature = "stats")]
mod covmat;
#[cfg(feature = "std")]
mod dot;
mod fast;
#[cfg(feature = "stats")]
mod hist;
#[cfg(kani)]
mod kani_proofs;
mod lattice;
mod merge;
#[cfg(feature = "receipts")]
mod receipt;
#[cfg(feature = "std")]
mod replicated;
#[cfg(feature = "stats")]
mod stats;

pub use acc::{SumF32, SumF64};
#[cfg(feature = "stats")]
pub use covmat::CovMatrixF64;
#[cfg(feature = "std")]
pub use dot::{dot, DotError, DotF64};
pub use fast::FastSumF64;
#[cfg(feature = "stats")]
pub use hist::HistogramF64;
pub use lattice::ExtremaF64;
pub use merge::Mergeable;
#[cfg(feature = "receipts")]
pub use receipt::state_hash;
#[cfg(feature = "std")]
pub use replicated::{ConvergentMap, Deltas, Replicated};
#[cfg(feature = "stats")]
pub use stats::{CovF64, Moments4F64, MomentsF64, PnMomentsF64, StatsError, WeightedMomentsF64};
