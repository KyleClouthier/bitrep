// Copyright (c) 2026 Kyle Clouthier / Clouthier Simulation Labs. Licensed under MIT OR Apache-2.0.
//! Real-data-shaped validation for `RelSketch` (feature `quantile`).
//!
//! Synthetic where it is honest to be synthetic: a *realistic web-latency*
//! stream — a lognormal body, a heavy Pareto tail (the slow-request minority),
//! and periodic spikes (a recurring GC-pause / deploy-degradation window) — at
//! a couple million samples. This is the shape real request-latency histograms
//! take; the point is to measure the sketch against the EXACT sorted quantile
//! at the tail percentiles SREs actually page on (p99/p999/p9999), for both a
//! 1% and a 0.1% accuracy target, and to report the serialized size.
//!
//! The accuracy assertion is the guarantee, not a hope: every measured relative
//! error must sit within the sketch's `guaranteed_alpha()` (a hair of slack for
//! the deterministic bucket-midpoint read).
//!
//! Run: `cargo test --release --features quantile --test quantile_realdata -- --nocapture`

#![cfg(feature = "quantile")]

use bitrep::RelSketch;

// deterministic xorshift64* — reproducible, no external dependency
struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed | 1)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn normal(&mut self) -> f64 {
        let (u, v) = (self.unit().max(1e-15), self.unit());
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }
}

/// A realistic web-request latency in milliseconds for the `i`-th request:
/// lognormal body (median ~5 ms), a 2% heavy Pareto tail, and a recurring
/// degradation window (every 10k requests, 50 requests run ~8x slower).
fn web_latency(rng: &mut Rng, i: usize) -> f64 {
    let mut x = (1.6 + 0.5 * rng.normal()).exp(); // lognormal body
    if rng.unit() < 0.02 {
        // heavy tail: Pareto(alpha=1.3) multiplier — the slow minority
        x *= 1.0 / rng.unit().max(1e-9).powf(1.0 / 1.3);
    }
    if i % 10_000 < 50 {
        x *= 8.0; // periodic spike window (GC pause / deploy)
    }
    x
}

/// Exact nearest-rank quantile of a sample (sorts a copy).
fn exact_quantile(sorted: &[f64], q: f64) -> f64 {
    let idx = ((q * sorted.len() as f64).ceil() as usize).clamp(1, sorted.len()) - 1;
    sorted[idx]
}

#[test]
fn realistic_latency_accuracy_and_size_within_guarantee() {
    const N: usize = 2_000_000;
    let mut rng = Rng::new(0x1A7E_9C1E);
    let data: Vec<f64> = (0..N).map(|i| web_latency(&mut rng, i)).collect();
    let mut sorted = data.clone();
    sorted.sort_by(f64::total_cmp);

    let qs = [0.5, 0.9, 0.95, 0.99, 0.999, 0.9999];
    let raw_bytes = data.len() * 8;

    println!(
        "\n[real-data] realistic web-latency, N = {N} ({} MB raw f64)",
        raw_bytes >> 20
    );
    println!(
        "  {:>6} | {:>9} | {:>10} {:>10} {:>9}",
        "alpha", "guarantee", "buckets", "bytes", "vs raw"
    );

    for &alpha in &[0.01, 0.001] {
        let mut sketch = RelSketch::new(alpha).unwrap();
        for &x in &data {
            sketch.add(x);
        }
        let guar = sketch.guaranteed_alpha();
        let bytes = sketch.to_bytes().len();
        println!(
            "  {:>6} | {:>9.5} | {:>10} {:>10} {:>8}x",
            alpha,
            guar,
            sketch.bucket_count(),
            bytes,
            raw_bytes / bytes
        );
        print!("         quantile rel-err:");
        let mut worst = 0.0f64;
        for &q in &qs {
            let exact = exact_quantile(&sorted, q);
            let est = sketch.quantile(q).unwrap();
            let rel = (est - exact).abs() / exact.abs();
            worst = worst.max(rel);
            let label = if q >= 0.999 {
                format!("p{:.2}", q * 100.0)
            } else {
                format!("p{}", (q * 100.0) as u32)
            };
            print!(" {label}={rel:.5}");
            assert!(
                rel <= guar * 1.001,
                "alpha {alpha}: {label} rel err {rel} exceeds guarantee {guar}"
            );
        }
        println!("  (worst {worst:.5})");
        // Size is constant in N and tiny: assert the sketch is far smaller than
        // the raw data (the compression claim, measured not asserted on faith).
        assert!(
            bytes < raw_bytes / 100,
            "sketch should be >100x smaller than raw"
        );
        // No collapse should trigger on realistic data: the guarantee is the
        // clean 2^-(sub_bits+1), not a coarsened one.
        assert_eq!(
            sketch.collapse_shift(),
            0,
            "realistic data must not collapse"
        );
    }
}
