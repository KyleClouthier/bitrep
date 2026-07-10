// Copyright (c) 2026 Kyle Clouthier. Licensed under MIT OR Apache-2.0.
//! Verification against NIST Statistical Reference Datasets (StRD),
//! Univariate Summary Statistics: NumAcc1–NumAcc4.
//! https://www.itl.nist.gov/div898/strd/univ/homepage.html
//!
//! These are constructed numerical-accuracy stress datasets with *certified*
//! exact sample means (Simon & Lesage 1989). The patterns below reproduce the
//! published data files exactly (verified against the .dat files, 2026-07-10):
//!
//! * NumAcc1: 10000001, 10000003, 10000002              (n = 3,    mean 10000002, exact)
//! * NumAcc2: 1.2 then 500 x (1.1, 1.3)                 (n = 1001, mean 1.2, exact)
//! * NumAcc3: 1000000.2 then 500 x (1000000.1, 1000000.3)   (n = 1001, mean 1000000.2)
//! * NumAcc4: 10000000.2 then 500 x (10000000.1, 10000000.3) (n = 1001, mean 10000000.2)
//!
//! NIST scores results by LRE (log10 relative error) against the certified
//! value. The inputs are decimal and not exactly representable in binary, so
//! the theoretical ceiling for ANY double-precision routine is ~15 digits.
//! We require LRE >= 14.5 for every dataset — i.e. bitrep's mean is accurate
//! to the limit the input representation allows.

use bitrep::SumF64;

fn lre(computed: f64, certified: f64) -> f64 {
    if computed == certified {
        return 16.0; // conventional cap: exact to full precision
    }
    -((computed - certified).abs() / certified.abs()).log10()
}

fn mean(values: impl Iterator<Item = f64>, n: u64) -> f64 {
    let acc: SumF64 = values.collect();
    assert_eq!(acc.count(), n);
    // Exact sum, then a single rounding in the division.
    acc.value() / n as f64
}

fn numacc_pattern(base: &str, low: &str, high: &str) -> Vec<f64> {
    let mut v = vec![base.parse::<f64>().unwrap()];
    for _ in 0..500 {
        v.push(low.parse().unwrap());
        v.push(high.parse().unwrap());
    }
    v
}

#[test]
fn numacc1() {
    let m = mean([10000001.0, 10000003.0, 10000002.0].into_iter(), 3);
    // Inputs are exact integers: the certified mean must be hit exactly.
    assert_eq!(
        m, 10000002.0,
        "NumAcc1 certified mean is exact-representable; no excuse"
    );
}

#[test]
fn numacc2() {
    let data = numacc_pattern("1.2", "1.1", "1.3");
    let m = mean(data.into_iter(), 1001);
    let score = lre(m, 1.2);
    assert!(
        score >= 14.5,
        "NumAcc2 LRE {score:.2} < 14.5 (mean {m:.17})"
    );
}

#[test]
fn numacc3() {
    let data = numacc_pattern("1000000.2", "1000000.1", "1000000.3");
    let m = mean(data.into_iter(), 1001);
    let score = lre(m, 1000000.2);
    assert!(
        score >= 14.5,
        "NumAcc3 LRE {score:.2} < 14.5 (mean {m:.17})"
    );
}

#[test]
fn numacc4() {
    let data = numacc_pattern("10000000.2", "10000000.1", "10000000.3");
    let m = mean(data.into_iter(), 1001);
    let score = lre(m, 10000000.2);
    assert!(
        score >= 14.5,
        "NumAcc4 LRE {score:.2} < 14.5 (mean {m:.17})"
    );
}

/// The distributed contract on real NIST data: shard NumAcc4 across "nodes",
/// merge, and require byte-identical state vs sequential — then check the
/// serialized state round-trips.
#[test]
fn numacc4_sharded_bitwise() {
    let data = numacc_pattern("10000000.2", "10000000.1", "10000000.3");
    let whole: SumF64 = data.iter().copied().collect();

    let mut merged = SumF64::new();
    for chunk in data.chunks(97) {
        // deliberately odd shard size
        let shard: SumF64 = chunk.iter().copied().collect();
        merged.merge(&shard);
    }
    assert_eq!(whole.to_bytes(), merged.to_bytes());

    let revived = SumF64::from_bytes(&merged.to_bytes()).unwrap();
    assert_eq!(revived.value().to_bits(), whole.value().to_bits());
}
