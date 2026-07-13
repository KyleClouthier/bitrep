// Copyright (c) 2026 Kyle Clouthier / Clouthier Simulation Labs. Licensed under MIT OR Apache-2.0.
//! Convergent statistics as a lawful CRDT: replicated mean/variance/regression
//! that converge to the same bits on every replica, in any merge order.
//!
//! The `stats` types are exact and mergeable; this example adds the CRDT map
//! layer from the float-counter construction (one entry per replica,
//! highest-count-wins) so the object is a state-based CRDT in the lawful
//! sense: idempotent, commutative, associative — duplicated or re-delivered
//! states are harmless. Chan/Golub/LeVeque merging (the classical art)
//! double-counts on re-delivery and its bits depend on the merge tree; this
//! doesn't, and the reads are exactly rounded.
//!
//! Run: `cargo run --release --features stats --example convergent_stats`

use bitrep::MomentsF64;
use std::collections::BTreeMap;

/// A replicated statistics object: replica id -> that replica's local state.
/// Join = per-entry highest-count-wins (each replica's state grows
/// monotonically, so the entry lattice is a total order per key).
#[derive(Clone, PartialEq, Default)]
struct ReplicatedStats {
    entries: BTreeMap<u32, MomentsF64>,
}

impl ReplicatedStats {
    fn local_add(&mut self, replica: u32, x: f64) {
        self.entries.entry(replica).or_default().add(x);
    }
    fn join(&mut self, other: &Self) {
        for (r, m) in &other.entries {
            match self.entries.get(r) {
                Some(mine) if mine.count() >= m.count() => {}
                _ => {
                    self.entries.insert(*r, m.clone());
                }
            }
        }
    }
    fn global(&self) -> MomentsF64 {
        let mut g = MomentsF64::new();
        for m in self.entries.values() {
            g.merge(m);
        }
        g
    }
    fn bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for (r, m) in &self.entries {
            out.extend_from_slice(&r.to_le_bytes());
            out.extend_from_slice(&m.to_bytes());
        }
        out
    }
}

fn main() {
    // three replicas ingest disjoint samples while disconnected
    let mut a = ReplicatedStats::default();
    let mut b = ReplicatedStats::default();
    let mut c = ReplicatedStats::default();
    let mut s = 0x9E3779B97F4A7C15u64;
    let mut sample = || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        // catastrophic-cancellation regime: mean 1e8, spread 1e-3
        1.0e8 + ((s >> 11) as f64 / (1u64 << 53) as f64 - 0.5) * 2e-3
    };
    for i in 0..30_000 {
        let x = sample();
        match i % 3 {
            0 => a.local_add(0, x),
            1 => b.local_add(1, x),
            _ => c.local_add(2, x),
        }
    }

    // sync in two different orders — and once with a duplicated delivery
    let mut order1 = a.clone();
    order1.join(&b);
    order1.join(&c);
    let mut order2 = c.clone();
    order2.join(&a);
    order2.join(&b);
    order2.join(&b); // re-delivered: must be harmless
    assert_eq!(
        order1.bytes(),
        order2.bytes(),
        "join order / re-delivery changed state"
    );

    // CRDT laws, checked
    let mut idem = a.clone();
    idem.join(&a.clone());
    assert_eq!(idem.bytes(), a.bytes(), "idempotence");
    let (mut ab, mut ba) = (a.clone(), b.clone());
    ab.join(&b);
    ba.join(&a);
    assert_eq!(ab.bytes(), ba.bytes(), "commutativity");

    let g = order1.global();
    println!(
        "replicated stats over {} samples, mean 1e8, spread 1e-3:",
        g.count()
    );
    println!("  mean     = {:.17e}   (exactly rounded)", g.mean());
    println!("  variance = {:.17e}   (exactly rounded)", g.variance());
    println!("  stddev   = {:.17e}", g.stddev());

    // the textbook formula in f64, for contrast (it collapses in this regime)
    let n = g.count() as f64;
    let naive_var = {
        // recompute naively from the same stream
        let mut s2 = 0.0f64;
        let mut s1 = 0.0f64;
        let mut st = 0x9E3779B97F4A7C15u64;
        let mut sample = || {
            st ^= st << 13;
            st ^= st >> 7;
            st ^= st << 17;
            1.0e8 + ((st >> 11) as f64 / (1u64 << 53) as f64 - 0.5) * 2e-3
        };
        for _ in 0..30_000 {
            let x = sample();
            s1 += x;
            s2 += x * x;
        }
        s2 / n - (s1 / n) * (s1 / n)
    };
    println!("  naive f64 textbook variance = {naive_var:.6e}  <- catastrophic cancellation");
    println!("\nCRDT laws verified: idempotent, commutative, join-order/duplicate safe.");
}
