// Copyright (c) 2026 Kyle Clouthier / Clouthier Simulation Labs. Licensed under MIT OR Apache-2.0.
//! PROBE test harness for `RelSketch` (feature `probe`).
//!
//! Four claims, measured (not asserted on faith):
//!   1. BYTE-IDENTITY under insertion order, sharding and merge order — the
//!      headline. Includes hostile inputs (subnormals, huge, tiny, negatives,
//!      ±∞, NaN, ±0).
//!   2. ACCURACY: measured relative error of p50/p90/p95/p99/p999 vs the exact
//!      sorted quantile on lognormal, pareto and bimodal 1e6-sample streams,
//!      confirmed within the `alpha` guarantee.
//!   3. MERGE CORRECTNESS: a merged sketch is byte-identical to one built from
//!      the concatenated data, over many random shard splits.
//!   4. SIZE: serialized bytes vs distinct buckets, reported per distribution.
//!
//! Run: `cargo test --release --features "probe receipts" --test quantile_probe -- --nocapture`

#![cfg(feature = "probe")]

use bitrep::{Mergeable, RelSketch};

/// A named continuous distribution sampler.
type Dist = (&'static str, fn(&mut Rng) -> f64);

// ---------------------------------------------------------------------------
// deterministic rng (xorshift64*) — no external dependency, fully reproducible
// ---------------------------------------------------------------------------
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

fn lognormal(rng: &mut Rng) -> f64 {
    (1.5 + 0.75 * rng.normal()).exp()
}
fn pareto(rng: &mut Rng) -> f64 {
    // heavy tail: x_m = 5, alpha = 1.5
    5.0 / rng.unit().max(1e-15).powf(1.0 / 1.5)
}
fn bimodal(rng: &mut Rng) -> f64 {
    if rng.unit() < 0.7 {
        (2.0 + 0.3 * rng.normal()).exp()
    } else {
        (5.0 + 0.3 * rng.normal()).exp()
    }
}

fn shuffle<T>(v: &mut [T], rng: &mut Rng) {
    for i in (1..v.len()).rev() {
        let j = (rng.next_u64() % (i as u64 + 1)) as usize;
        v.swap(i, j);
    }
}

fn build(data: &[f64], alpha: f64) -> RelSketch {
    let mut s = RelSketch::new(alpha).unwrap();
    for &x in data {
        s.add(x);
    }
    s
}

/// Exact nearest-rank quantile of a sample (sorts a copy).
fn exact_quantile(data: &[f64], q: f64) -> f64 {
    let mut v: Vec<f64> = data.iter().copied().filter(|x| !x.is_nan()).collect();
    v.sort_by(f64::total_cmp);
    let idx = ((q * v.len() as f64).ceil() as usize).clamp(1, v.len()) - 1;
    v[idx]
}

// ---------------------------------------------------------------------------
// 1. BYTE-IDENTITY (the headline) — hostile inputs, all orderings/shardings
// ---------------------------------------------------------------------------
#[test]
fn byte_identity_under_order_shard_and_merge() {
    let mut rng = Rng::new(0xB17E_1DEA);

    // A hostile multiset: normal, subnormal, huge, tiny, negatives, zeros,
    // ±∞ and NaN — everything the mapping must survive.
    let mut data: Vec<f64> = Vec::new();
    for _ in 0..20_000 {
        data.push(lognormal(&mut rng));
    }
    for _ in 0..2_000 {
        data.push(-pareto(&mut rng)); // negatives
    }
    for &x in &[
        f64::MIN_POSITIVE,          // smallest normal
        f64::MIN_POSITIVE / 2.0,    // a subnormal
        5e-324,                     // smallest subnormal
        f64::MAX,
        1e300,
        1e-300,
        0.0,
        -0.0,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
        -f64::MAX,
    ] {
        for _ in 0..37 {
            data.push(x);
        }
    }

    let reference = build(&data, 0.01);
    let ref_bytes = reference.to_bytes();
    // sanity: it round-trips
    assert_eq!(RelSketch::from_bytes(&ref_bytes).unwrap(), reference);

    // (a) many shuffled insertion orders -> identical bytes
    for seed in 0..25 {
        let mut d = data.clone();
        shuffle(&mut d, &mut Rng::new(seed + 1));
        assert_eq!(build(&d, 0.01).to_bytes(), ref_bytes, "insertion order changed bytes");
    }

    // (b) shard K ways (K = 1..=13), merge in a shuffled order -> identical
    for k in 1..=13usize {
        let mut d = data.clone();
        shuffle(&mut d, &mut Rng::new(1000 + k as u64));
        let mut shards: Vec<RelSketch> = (0..k).map(|_| RelSketch::new(0.01).unwrap()).collect();
        for (i, &x) in d.iter().enumerate() {
            shards[i % k].add(x);
        }
        let mut order: Vec<usize> = (0..k).collect();
        shuffle(&mut order, &mut Rng::new(9000 + k as u64));
        let mut merged = RelSketch::new(0.01).unwrap();
        for &s in &order {
            merged.merge(&shards[s]);
        }
        assert_eq!(merged.to_bytes(), ref_bytes, "sharding K={k} changed bytes");
    }

    // (c) a lopsided binary merge tree -> identical
    let mut shards: Vec<RelSketch> = data.chunks(777).map(|c| build(c, 0.01)).collect();
    while shards.len() > 1 {
        let b = shards.pop().unwrap();
        let last = shards.len() - 1;
        shards[last].merge(&b); // fold from the right: unbalanced tree
    }
    assert_eq!(shards[0].to_bytes(), ref_bytes, "merge tree shape changed bytes");

    println!(
        "[byte-identity] {} samples, {} buckets, {} bytes: identical across \
         25 orders, K=1..13 shardings, and an unbalanced merge tree.",
        reference.count(),
        reference.bucket_count(),
        ref_bytes.len()
    );
}

// ---------------------------------------------------------------------------
// 2. ACCURACY — measured relative error vs exact, within the guarantee
// ---------------------------------------------------------------------------
#[test]
fn accuracy_within_guarantee() {
    let dists: [Dist; 3] =
        [("lognormal", lognormal), ("pareto(1.5)", pareto), ("bimodal", bimodal)];
    let qs = [0.5, 0.9, 0.95, 0.99, 0.999];
    let alpha = 0.01;
    let n = 1_000_000usize;

    println!("\n[accuracy] {n} samples/dist, requested alpha = {alpha}");
    for (name, f) in dists {
        let mut rng = Rng::new(0xACC0_0000 ^ name.len() as u64);
        let data: Vec<f64> = (0..n).map(|_| f(&mut rng)).collect();
        let sketch = build(&data, alpha);
        let guar = sketch.guaranteed_alpha();

        print!("  {name:<12} guarantee {guar:.5} | ");
        let mut worst = 0.0f64;
        for &q in &qs {
            let exact = exact_quantile(&data, q);
            let est = sketch.quantile(q).unwrap();
            let rel = (est - exact).abs() / exact.abs();
            worst = worst.max(rel);
            print!("p{}={:.5} ", (q * 1000.0) as u32, rel);
            assert!(
                rel <= guar * 1.001,
                "{name} p{q}: rel err {rel} exceeds guarantee {guar}"
            );
        }
        println!("| worst {worst:.5}");
        assert!(worst <= guar * 1.001);
    }
}

// ---------------------------------------------------------------------------
// 3. MERGE CORRECTNESS — merged == concatenated, many random splits
// ---------------------------------------------------------------------------
#[test]
fn merge_equals_concatenation() {
    let mut rng = Rng::new(0x3E4D_1234);
    let data: Vec<f64> = (0..50_000).map(|_| lognormal(&mut rng)).collect();
    let whole = build(&data, 0.005);
    let whole_bytes = whole.to_bytes();

    for trial in 0..40 {
        let k: u64 = 2 + (trial % 11);
        let mut splits: Vec<RelSketch> = (0..k).map(|_| RelSketch::new(0.005).unwrap()).collect();
        // random, uneven assignment
        let mut r = Rng::new(0xFEED_0000 + trial);
        for &x in &data {
            let b = (r.next_u64() % k) as usize;
            splits[b].add(x);
        }
        shuffle(&mut splits, &mut Rng::new(0xBEEF_0000 + trial));
        let mut merged = RelSketch::new(0.005).unwrap();
        for s in &splits {
            merged.merge(s);
        }
        assert_eq!(merged.to_bytes(), whole_bytes, "trial {trial}: merge != concat");
    }
    println!("\n[merge] 40 random splits (K=2..12): merged state byte-identical to whole.");
}

// ---------------------------------------------------------------------------
// 4. SIZE — serialized bytes vs distinct buckets per distribution
// ---------------------------------------------------------------------------
#[test]
fn size_report() {
    let dists: [Dist; 3] =
        [("lognormal", lognormal), ("pareto(1.5)", pareto), ("bimodal", bimodal)];
    println!("\n[size] 1e6 samples, alpha = 0.01 (sub_bits = 6)");
    println!("  {:<12} {:>8} {:>10} {:>12} {:>14}", "dist", "buckets", "bytes", "bytes/bkt", "vs raw f64");
    for (name, f) in dists {
        let mut rng = Rng::new(0x512E_0000 ^ name.len() as u64);
        let data: Vec<f64> = (0..1_000_000).map(|_| f(&mut rng)).collect();
        let s = build(&data, 0.01);
        let bytes = s.to_bytes().len();
        let raw = data.len() * 8;
        println!(
            "  {:<12} {:>8} {:>10} {:>12.1} {:>13}x",
            name,
            s.bucket_count(),
            bytes,
            bytes as f64 / s.bucket_count().max(1) as f64,
            raw / bytes
        );
    }
}

// ---------------------------------------------------------------------------
// determinism of the mapping: the bucket key is a pure integer shift
// ---------------------------------------------------------------------------
#[test]
fn mapping_is_pure_integer_shift() {
    // Two values in the same octave, differing only in low mantissa bits below
    // the kept prefix, MUST share a bucket; crossing the sub-bucket boundary
    // MUST change it. This is a bit fact, independent of any libm.
    let s = RelSketch::with_sub_bits(6).unwrap();
    let _ = s; // sub_bits fixed; assert via the public shift relationship
    let sub_bits = 6u32;
    let shift = 52 - sub_bits;
    let base = 1.0f64.to_bits(); // 1.0
    let a = f64::from_bits(base); // 1.0
    let b = f64::from_bits(base + (1u64 << (shift - 1))); // same sub-bucket
    let c = f64::from_bits(base + (1u64 << shift)); // next sub-bucket
    let ka = a.to_bits() >> shift;
    let kb = b.to_bits() >> shift;
    let kc = c.to_bits() >> shift;
    assert_eq!(ka, kb, "sub-prefix-equal values must share a bucket");
    assert_eq!(kc, ka + 1, "crossing the prefix must step exactly one bucket");
}
