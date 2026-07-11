// Copyright (c) 2026 Kyle Clouthier. Licensed under MIT OR Apache-2.0.
//! Generic containers over [`Mergeable`] states: keyed aggregation
//! (GROUP BY / windows), the per-replica CRDT layer, and delta-state
//! transport.
//!
//! These are the packaging that turns individual states into a toolkit:
//! * [`ConvergentMap`] — key → state, merged per key: `GROUP BY`, tumbling
//!   windows (key by window id), per-metric fleets.
//! * [`Replicated`] — the lawful CRDT wrapper from the float-counter paper,
//!   generalized: one entry per replica, joined by highest-count-wins.
//!   Idempotent, commutative, associative; duplicate delivery is harmless.
//! * [`Deltas`] — delta-state transport: ship only what changed since the
//!   last sync instead of the full state (à la delta-state CRDTs of Almeida,
//!   Shoker & Baquero), correct because merge is additive.

use crate::Mergeable;
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// ConvergentMap — GROUP BY / windows
// ---------------------------------------------------------------------------

/// A keyed family of mergeable states: `GROUP BY key` for the convergent
/// world. Merging merges per key; encoding is canonical (keys sorted, from
/// the `BTreeMap`).
///
/// Tumbling windows are this map keyed by window id
/// (`timestamp / window_len`), plus a deterministic retention cutoff via
/// [`expire_before`](Self::expire_before) — expiry must be driven by an
/// *agreed* cutoff (e.g. watermark), not local clocks, to preserve
/// convergence; that contract is the caller's.
#[derive(Clone, PartialEq, Debug)]
pub struct ConvergentMap<K: Ord + Clone, M: Mergeable> {
    entries: BTreeMap<K, M>,
}

impl<K: Ord + Clone, M: Mergeable> Default for ConvergentMap<K, M> {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }
}

impl<K: Ord + Clone, M: Mergeable> ConvergentMap<K, M> {
    /// An empty map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Mutable access to the state under `key`, inserting `init()` if absent.
    pub fn entry_or(&mut self, key: K, init: impl FnOnce() -> M) -> &mut M {
        self.entries.entry(key).or_insert_with(init)
    }

    /// The state under `key`, if any.
    pub fn get(&self, key: &K) -> Option<&M> {
        self.entries.get(key)
    }

    /// Iterate entries in key order.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &M)> {
        self.entries.iter()
    }

    /// Number of keys.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True if no keys.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Merge another map: per-key state merge (shard combining).
    pub fn merge(&mut self, other: &Self) {
        for (k, m) in &other.entries {
            match self.entries.get_mut(k) {
                Some(mine) => mine.merge(m),
                None => {
                    self.entries.insert(k.clone(), m.clone());
                }
            }
        }
    }

    /// Drop every key strictly below `cutoff` (deterministic retention:
    /// callers must apply the same agreed cutoff on every replica to
    /// preserve convergence).
    pub fn expire_before(&mut self, cutoff: &K) {
        self.entries = self.entries.split_off(cutoff);
    }

    /// Total operation count across all entries (saturating).
    pub fn count(&self) -> u64 {
        self.entries
            .values()
            .fold(0u64, |a, m| a.saturating_add(m.count()))
    }
}

impl<K: Ord + Clone + AsRef<[u8]>, M: Mergeable> ConvergentMap<K, M> {
    /// Canonical byte encoding (keys in sorted order; length-prefixed).
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(self.entries.len() as u32).to_le_bytes());
        for (k, m) in &self.entries {
            let kb = k.as_ref();
            out.extend_from_slice(&(kb.len() as u32).to_le_bytes());
            out.extend_from_slice(kb);
            let mb = m.encode();
            out.extend_from_slice(&(mb.len() as u32).to_le_bytes());
            out.extend_from_slice(&mb);
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Replicated — the lawful CRDT layer
// ---------------------------------------------------------------------------

/// The per-replica CRDT wrapper: replica id → that replica's own state,
/// joined per entry by **highest count wins**.
///
/// Contract (same as the counter paper): each replica only ever adds to its
/// *own* entry, so a replica's states are totally ordered by `count` and the
/// entry-wise count-wins join is a lattice join — idempotent, commutative,
/// associative. Duplicated, re-ordered or re-delivered states are harmless.
#[derive(Clone, PartialEq, Debug)]
pub struct Replicated<M: Mergeable> {
    entries: BTreeMap<u64, M>,
}

impl<M: Mergeable> Default for Replicated<M> {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }
}

impl<M: Mergeable> Replicated<M> {
    /// An empty object.
    pub fn new() -> Self {
        Self::default()
    }

    /// Mutable access to `replica`'s own entry, inserting `init()` if absent.
    /// A replica must only ever mutate its own entry.
    pub fn local_mut(&mut self, replica: u64, init: impl FnOnce() -> M) -> &mut M {
        self.entries.entry(replica).or_insert_with(init)
    }

    /// Lattice join: entry-wise highest-count-wins. Idempotent.
    pub fn join(&mut self, other: &Self) {
        for (r, m) in &other.entries {
            match self.entries.get(r) {
                Some(mine) if mine.count() >= m.count() => {}
                _ => {
                    self.entries.insert(*r, m.clone());
                }
            }
        }
    }

    /// The converged global state: the merge of every replica's entry.
    /// Byte-identical on every replica that has joined the same set.
    pub fn global(&self, empty: impl FnOnce() -> M) -> M {
        let mut g = empty();
        for m in self.entries.values() {
            g.merge(m);
        }
        g
    }

    /// Canonical byte encoding (replica ids sorted).
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(self.entries.len() as u32).to_le_bytes());
        for (r, m) in &self.entries {
            out.extend_from_slice(&r.to_le_bytes());
            let mb = m.encode();
            out.extend_from_slice(&(mb.len() as u32).to_le_bytes());
            out.extend_from_slice(&mb);
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Deltas — delta-state transport
// ---------------------------------------------------------------------------

/// Delta-state transport for additive states: keep a full state and a
/// pending delta; ship only the delta since the last sync. The receiver
/// simply merges the delta into its copy — correct because merge is
/// additive and associative (delta-state CRDTs, Almeida–Shoker–Baquero).
///
/// Note the duplication caveat: merging the *same delta twice* double-counts
/// (additive semantics). Deliver deltas at-most-once, or use
/// [`Replicated`] full-state joins where the network may re-deliver.
#[derive(Clone, Debug)]
pub struct Deltas<M: Mergeable> {
    full: M,
    pending: M,
    make_empty: fn() -> M,
}

impl<M: Mergeable> Deltas<M> {
    /// A new tracker; `make_empty` constructs an empty state.
    pub fn new(make_empty: fn() -> M) -> Self {
        Self {
            full: make_empty(),
            pending: make_empty(),
            make_empty,
        }
    }

    /// Apply an operation to the state (recorded in both the full state and
    /// the pending delta).
    pub fn apply(&mut self, op: impl Fn(&mut M)) {
        op(&mut self.full);
        op(&mut self.pending);
    }

    /// The full local state.
    pub fn full(&self) -> &M {
        &self.full
    }

    /// Take the pending delta (resets it): ship this to peers.
    pub fn take_delta(&mut self) -> M {
        core::mem::replace(&mut self.pending, (self.make_empty)())
    }

    /// Merge a delta received from a peer into the full state.
    pub fn merge_delta(&mut self, delta: &M) {
        self.full.merge(delta);
    }
}
