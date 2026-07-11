// Copyright (c) 2026 Kyle Clouthier. Licensed under MIT OR Apache-2.0.
//! Honest benchmarks: bitrep vs the things you'd use instead.
//!
//! Run: `cargo bench`. Numbers vary by hardware; the point is the *multiple*,
//! stated plainly in the README. Exactness costs — know what you're paying.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

/// Deterministic xorshift so benches are reproducible without an RNG dep.
fn data(n: usize) -> Vec<f64> {
    let mut s = 0x243F6A8885A308D3u64; // pi digits, nothing up the sleeve
    (0..n)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            // Mix magnitudes across ~12 decades so this isn't a softball.
            let mant = (s >> 11) as f64 / (1u64 << 53) as f64 - 0.5;
            let exp = ((s >> 1) % 40) as i32 - 20;
            mant * 2f64.powi(exp)
        })
        .collect()
}

fn naive(xs: &[f64]) -> f64 {
    let mut s = 0.0;
    for x in xs {
        s += x;
    }
    s
}

fn kahan(xs: &[f64]) -> f64 {
    let (mut s, mut c) = (0.0f64, 0.0f64);
    for &x in xs {
        let y = x - c;
        let t = s + y;
        c = (t - s) - y;
        s = t;
    }
    s
}

fn bitrep_sum(xs: &[f64]) -> f64 {
    let mut a = bitrep::SumF64::new();
    for &x in xs {
        a.add(x);
    }
    a.value()
}

/// The v0.2 fast streaming front-end: same canonical bytes, chunked adds.
fn bitrep_fast_sum(xs: &[f64]) -> f64 {
    let mut a = bitrep::FastSumF64::new();
    a.extend_from_slice(xs);
    a.finish().value()
}

/// Neal's superaccumulator (the `xsum` crate), fed through its fast path
/// (`add_list`), using the variant its docs recommend per size: `XsumSmall`
/// for n <= 1000, `XsumLarge` above. Exact like bitrep, but not mergeable
/// and no canonical byte encoding — see the README's prior-art section.
fn xsum_sum(xs: &[f64]) -> f64 {
    use xsum::Xsum;
    if xs.len() <= 1_000 {
        let mut a = xsum::XsumSmall::new();
        a.add_list(xs);
        a.sum()
    } else {
        let mut a = xsum::XsumLarge::new();
        a.add_list(xs);
        a.sum()
    }
}

fn bench_sums(c: &mut Criterion) {
    let mut g = c.benchmark_group("sum");
    for n in [1_000usize, 100_000, 1_000_000] {
        let xs = data(n);
        g.throughput(Throughput::Elements(n as u64));
        g.bench_with_input(BenchmarkId::new("naive", n), &xs, |b, xs| {
            b.iter(|| naive(black_box(xs)))
        });
        g.bench_with_input(BenchmarkId::new("kahan", n), &xs, |b, xs| {
            b.iter(|| kahan(black_box(xs)))
        });
        g.bench_with_input(BenchmarkId::new("xsum", n), &xs, |b, xs| {
            b.iter(|| xsum_sum(black_box(xs)))
        });
        g.bench_with_input(BenchmarkId::new("bitrep", n), &xs, |b, xs| {
            b.iter(|| bitrep_sum(black_box(xs)))
        });
        g.bench_with_input(BenchmarkId::new("bitrep_fast", n), &xs, |b, xs| {
            b.iter(|| bitrep_fast_sum(black_box(xs)))
        });
    }
    g.finish();
}

fn bench_merge(c: &mut Criterion) {
    // The distributed story: cost of combining shard accumulators.
    let xs = data(1_000_000);
    let shards: Vec<bitrep::SumF64> = xs
        .chunks(10_000)
        .map(|c| c.iter().copied().collect())
        .collect();
    c.bench_function("merge/100-shards-of-10k", |b| {
        b.iter(|| {
            let mut total = bitrep::SumF64::new();
            for s in black_box(&shards) {
                total.merge(s);
            }
            total.value()
        })
    });
}

criterion_group!(benches, bench_sums, bench_merge);
criterion_main!(benches);
