// Copyright (c) 2026 Kyle Clouthier. Licensed under MIT OR Apache-2.0.
//! Receipts (feature `receipts`): canonical state hashes for signing.
//!
//! Every bitrep state has a canonical encoding — equal multisets, equal
//! bytes — so "prove we computed the same aggregate" reduces to comparing a
//! hash. This module names that pattern: [`state_hash`] is the 32-byte
//! commitment every probe and demo hand-rolled.
//!
//! Signing is deliberately out of scope (bring your own keys — ed25519,
//! ML-DSA, whatever your trust plane uses): sign the returned digest, and a
//! verifier recomputes the aggregate from the shards it trusts and compares.

use crate::Mergeable;
use sha2::{Digest, Sha256};

/// The SHA-256 of a state's canonical encoding.
///
/// Two parties that hold the same multiset (in any order, any sharding, any
/// merge tree) get the same digest; any dropped, duplicated or altered
/// contribution changes it.
///
/// ```
/// # #[cfg(all(feature = "stats", feature = "receipts"))] {
/// use bitrep::{state_hash, MomentsF64};
///
/// let mut a = MomentsF64::new();
/// let mut b = MomentsF64::new();
/// for x in [1.5, 2.5, 3.5] { a.add(x); }
/// for x in [3.5, 1.5, 2.5] { b.add(x); } // different order
/// assert_eq!(state_hash(&a), state_hash(&b));
/// # }
/// ```
pub fn state_hash<M: Mergeable>(state: &M) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(state.encode());
    h.finalize().into()
}
