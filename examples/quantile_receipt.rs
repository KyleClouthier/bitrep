// Copyright (c) 2026 Kyle Clouthier / Clouthier Simulation Labs. Licensed under MIT OR Apache-2.0.
//! Demo: a p99 you can sign — byte-identical on every shard order.
//!
//! Run: `cargo run --release --features "quantile receipts" --example quantile_receipt`

use bitrep::{state_hash, Mergeable, RelSketch};

/// A tiny deterministic xorshift so the demo needs no rng dependency.
struct Rng(u64);
impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    /// A lognormal-ish latency sample in milliseconds.
    fn latency_ms(&mut self) -> f64 {
        let u = (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        let v = (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        // Box–Muller -> standard normal, then exp for a heavy right tail.
        let z = (-2.0 * (u + 1e-12).ln()).sqrt() * (std::f64::consts::TAU * v).cos();
        (2.0 + 0.6 * z).exp()
    }
}

fn main() {
    // one "true" stream of latencies
    let mut rng = Rng(0x1234_5678_9abc_def0);
    let data: Vec<f64> = (0..1_000_000).map(|_| rng.latency_ms()).collect();

    // Replica A ingests sequentially.
    let seq: RelSketch = data.iter().copied().collect();

    // Replica B sees the same data sharded 8 ways, merged in a scrambled order.
    let mut shards: Vec<RelSketch> = (0..8).map(|_| RelSketch::new(0.01).unwrap()).collect();
    for (i, &x) in data.iter().enumerate() {
        // deterministic but value-independent shard assignment
        shards[(i * 2654435761) % 8].add(x);
    }
    let order = [5usize, 2, 7, 0, 3, 6, 1, 4];
    let mut merged = RelSketch::new(0.01).unwrap();
    for &s in &order {
        merged.merge(&shards[s]);
    }

    // The headline: same bytes, same hash, regardless of order or sharding.
    let hseq = state_hash(&seq);
    let hmerged = state_hash(&merged);
    println!("sequential p99 = {:.4} ms", seq.quantile(0.99).unwrap());
    println!("sharded    p99 = {:.4} ms", merged.quantile(0.99).unwrap());
    println!("count = {}   buckets = {}", seq.count(), seq.bucket_count());
    println!("serialized state = {} bytes", seq.to_bytes().len());
    println!("state_hash(sequential) = {}", hex(&hseq));
    println!("state_hash(sharded)    = {}", hex(&hmerged));
    assert_eq!(seq.to_bytes(), merged.to_bytes(), "bytes differ!");
    assert_eq!(hseq, hmerged, "hashes differ!");
    println!("\nOK: identical bytes and identical SHA-256 receipt across shardings.");

    // Accuracy against the exact sorted quantile.
    let mut sorted = data.clone();
    sorted.sort_by(f64::total_cmp);
    println!(
        "\n quantile   exact        estimate     rel.error   (guarantee {:.4})",
        seq.guaranteed_alpha()
    );
    for &q in &[0.5, 0.9, 0.95, 0.99, 0.999] {
        let idx = ((q * sorted.len() as f64).ceil() as usize).clamp(1, sorted.len()) - 1;
        let exact = sorted[idx];
        let est = seq.quantile(q).unwrap();
        let rel = (est - exact).abs() / exact.abs();
        println!(
            "  p{:<7} {:>10.4}   {:>10.4}   {:>9.5}",
            (q * 100.0),
            exact,
            est,
            rel
        );
    }
}

fn hex(b: &[u8; 32]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
