// Copyright (c) 2026 Kyle Clouthier. Licensed under MIT OR Apache-2.0.
//! The [`Mergeable`] trait: the one abstraction every bitrep state shares.
//!
//! Everything in this crate is a *mergeable state*: adding is local, merging
//! is associative and commutative, and the canonical encoding makes equal
//! multisets yield equal bytes. This trait names that contract so containers
//! ([`crate::ConvergentMap`]), CRDT wrappers ([`crate::Replicated`]),
//! delta-state transports ([`crate::Deltas`]) and receipts can be written
//! once, generically.

#[cfg(feature = "std")]
use crate::DotF64;
use crate::{SumF32, SumF64};

/// A mergeable, canonically-encodable accumulator state.
///
/// Laws (each implementor's tests check them):
/// * `merge` is **associative** and **commutative**: any merge tree over the
///   same shards yields the same bytes.
/// * `count` is monotone under `add`/`merge` (saturating) — this is what
///   makes per-replica *highest-count-wins* joins ([`crate::Replicated`]) a
///   valid lattice.
/// * `encode`/`decode` round-trip: equal multisets ⇒ equal bytes.
///
/// `merge` alone is deliberately **not** idempotent (merging the same shard
/// twice double-counts, like any counter) — deduplication belongs to the
/// replica map layer, exactly as in every counter CRDT.
pub trait Mergeable: Clone {
    /// Merge another state into this one (shard combining).
    fn merge(&mut self, other: &Self);

    /// Number of samples/operations folded in (saturating, monotone).
    fn count(&self) -> u64;

    /// Canonical byte encoding: equal multisets ⇒ equal bytes. Hash or sign
    /// this.
    #[cfg(feature = "std")]
    fn encode(&self) -> Vec<u8>;

    /// Decode a canonical encoding produced by [`encode`](Self::encode).
    /// Returns `None` on malformed input.
    #[cfg(feature = "std")]
    fn decode(bytes: &[u8]) -> Option<Self>
    where
        Self: Sized;
}

impl Mergeable for SumF64 {
    fn merge(&mut self, other: &Self) {
        SumF64::merge(self, other);
    }
    fn count(&self) -> u64 {
        SumF64::count(self)
    }
    #[cfg(feature = "std")]
    fn encode(&self) -> Vec<u8> {
        self.to_bytes().to_vec()
    }
    #[cfg(feature = "std")]
    fn decode(bytes: &[u8]) -> Option<Self> {
        let arr: &[u8; SumF64::BYTES] = bytes.try_into().ok()?;
        SumF64::from_bytes(arr)
    }
}

impl Mergeable for SumF32 {
    fn merge(&mut self, other: &Self) {
        SumF32::merge(self, other);
    }
    fn count(&self) -> u64 {
        SumF32::count(self)
    }
    #[cfg(feature = "std")]
    fn encode(&self) -> Vec<u8> {
        self.to_bytes().to_vec()
    }
    #[cfg(feature = "std")]
    fn decode(bytes: &[u8]) -> Option<Self> {
        let arr: &[u8; SumF32::BYTES] = bytes.try_into().ok()?;
        SumF32::from_bytes(arr)
    }
}

#[cfg(feature = "std")]
impl Mergeable for DotF64 {
    fn merge(&mut self, other: &Self) {
        DotF64::merge(self, other);
    }
    fn count(&self) -> u64 {
        self.pairs()
    }
    fn encode(&self) -> Vec<u8> {
        self.to_bytes().to_vec()
    }
    fn decode(bytes: &[u8]) -> Option<Self> {
        let arr: &[u8; SumF64::BYTES + 1] = bytes.try_into().ok()?;
        DotF64::from_bytes(arr)
    }
}
