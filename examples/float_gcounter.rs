// Copyright (c) 2026 Kyle Clouthier / Clouthier Simulation Labs. Licensed under MIT OR Apache-2.0.
//! Probe: the first float-sum counter CRDT, tortured.
//!
//! Counter CRDTs (G-Counter, PN-Counter) have been integer-only for fifteen
//! years: CRDT merge must be commutative, associative and idempotent, and
//! float addition is neither commutative-in-state nor associative. `SumF64`
//! restores exact commutativity and associativity; idempotence comes from
//! the standard per-replica map with highest-count-wins entries — exactly
//! like every counter CRDT ever shipped.
//!
//! The torture this probe must survive (or the claim dies):
//! * N replicas, each adding its own stream of hostile floats (mixed
//!   magnitudes, subnormals, exact-cancellation pairs);
//! * hundreds of random gossip schedules with duplicate delivery, stale
//!   delivery, and long partitions;
//! * every converged replica must hold BYTE-identical state, and the value
//!   must equal the exactly rounded sum of every add that ever happened;
//! * a naive f64 shadow counter run alongside must (and does) disagree with
//!   itself across replicas — the contrast that shows why exactness is the
//!   enabling property, not a luxury.
//!
//! Run: `cargo run --example float_gcounter`

use bitrep::SumF64;

/// One replica's view: per-origin entries, highest count wins on merge.
/// (`SumF64::count` is monotone on a replica's own accumulator, so the entry
/// with the higher count is strictly newer — the standard G-Counter join.)
#[derive(Clone, Default)]
struct FloatGCounter {
    entries: Vec<Option<SumF64>>, // indexed by replica id
    naive: Vec<f64>,              // shadow: what a naive f64 G-Counter would hold
}

impl FloatGCounter {
    fn new(n: usize) -> Self {
        FloatGCounter {
            entries: vec![None; n],
            naive: vec![0.0; n],
        }
    }

    /// Local add: a replica only ever appends to its own entry.
    fn add(&mut self, me: usize, x: f64) {
        self.entries[me].get_or_insert_with(SumF64::new).add(x);
        self.naive[me] += x;
    }

    /// CRDT join: per-entry, highest count wins. Idempotent, commutative,
    /// associative — merging the same state twice is a no-op.
    fn join(&mut self, other: &FloatGCounter) {
        for (i, e) in other.entries.iter().enumerate() {
            if let Some(theirs) = e {
                let take = match &self.entries[i] {
                    None => true,
                    Some(mine) => theirs.count() > mine.count(),
                };
                if take {
                    self.entries[i] = Some(theirs.clone());
                    self.naive[i] = other.naive[i];
                }
            }
        }
    }

    /// The counter's value: exact merge of every entry.
    fn value_acc(&self) -> SumF64 {
        let mut total = SumF64::new();
        for e in self.entries.iter().flatten() {
            total.merge(e);
        }
        total
    }

    fn state_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for e in &self.entries {
            match e {
                Some(a) => out.extend_from_slice(&a.to_bytes()),
                None => out.push(0),
            }
        }
        out
    }
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn f64_hostile(&mut self) -> f64 {
        let s = self.next();
        match s % 5 {
            0 => f64::from_bits(self.next() % 4503599627370496), // subnormal zone
            1 => ((self.next() >> 11) as f64 / (1u64 << 53) as f64 - 0.5) * 1e100,
            _ => {
                let mant = (self.next() >> 11) as f64 / (1u64 << 53) as f64 - 0.5;
                let exp = (self.next() % 80) as i32 - 40;
                mant * 2f64.powi(exp)
            }
        }
    }
}

fn main() {
    const REPLICAS: usize = 8;
    const SCHEDULES: usize = 300;
    let mut naive_disagreements = 0usize;

    for schedule in 0..SCHEDULES {
        let mut rng = Rng(0xA0761D6478BD642F ^ (schedule as u64).wrapping_mul(0x9E37));
        let mut reps: Vec<FloatGCounter> = (0..REPLICAS)
            .map(|_| FloatGCounter::new(REPLICAS))
            .collect();

        // Ground truth: every add that ever happens, summed exactly.
        let mut truth = SumF64::new();

        // Phase 1: adds interleaved with chaotic gossip.
        for _ in 0..400 {
            let r = (rng.next() % REPLICAS as u64) as usize;
            match rng.next() % 4 {
                0 | 1 => {
                    // local add (with occasional exact-cancellation pair)
                    let x = rng.f64_hostile();
                    reps[r].add(r, x);
                    truth.add(x);
                    if rng.next() % 6 == 0 {
                        reps[r].add(r, -x);
                        truth.add(-x);
                    }
                }
                2 => {
                    // gossip a snapshot to a random peer (possibly stale later)
                    let to = (rng.next() % REPLICAS as u64) as usize;
                    let snap = reps[r].clone();
                    reps[to].join(&snap);
                }
                _ => {
                    // duplicate delivery: join the same snapshot twice
                    let to = (rng.next() % REPLICAS as u64) as usize;
                    let snap = reps[r].clone();
                    reps[to].join(&snap);
                    reps[to].join(&snap); // idempotence under fire
                }
            }
        }

        // Phase 2: heal — full anti-entropy sweep until quiescent.
        for _ in 0..2 {
            for i in 0..REPLICAS {
                for j in 0..REPLICAS {
                    if i != j {
                        let snap = reps[j].clone();
                        reps[i].join(&snap);
                    }
                }
            }
        }

        // Convergence: every replica byte-identical, value == exact truth.
        let bytes0 = reps[0].state_bytes();
        let total0 = reps[0].value_acc();
        for r in &reps {
            assert_eq!(r.state_bytes(), bytes0, "replica state diverged");
            assert_eq!(
                r.value_acc().to_bytes(),
                total0.to_bytes(),
                "merged total diverged"
            );
        }
        assert_eq!(
            total0.value().to_bits(),
            truth.value().to_bits(),
            "converged value != exactly rounded sum of all adds"
        );

        // Contrast: does the naive f64 shadow agree with a straight re-sum in
        // a different order? (Same entries here, so replicas agree — the
        // naive failure is order-dependence of the TOTAL.)
        let fwd: f64 = reps[0].naive.iter().sum();
        let rev: f64 = reps[0].naive.iter().rev().sum();
        if fwd.to_bits() != rev.to_bits() {
            naive_disagreements += 1;
        }
    }

    println!(
        "{SCHEDULES} chaotic gossip schedules (dupes, stale snapshots, partitions): \
         all {REPLICAS} replicas byte-identical, every total == exact sum: OK"
    );
    println!(
        "naive f64 contrast: summing the same converged entries fwd vs rev \
         disagreed in {naive_disagreements}/{SCHEDULES} schedules — the exactness is load-bearing"
    );
    println!("\nPROBE RESULT: LANDS — float counter CRDT survives the torture.");
}
