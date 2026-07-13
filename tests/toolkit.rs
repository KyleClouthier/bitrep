// Copyright (c) 2026 Kyle Clouthier / Clouthier Simulation Labs. Licensed under MIT OR Apache-2.0.
//! Verification of the v0.3 toolkit: lattice atoms, weighted/PN moments,
//! histograms, covariance matrices, containers, deltas, receipts.
//! Same discipline as tests/stats.rs: independent oracles, law checks,
//! codec round-trips, bit-invariance sweeps.

#![cfg(all(feature = "stats", feature = "receipts"))]

use bitrep::{
    state_hash, ConvergentMap, CovMatrixF64, Deltas, ExtremaF64, HistogramF64, Mergeable,
    MomentsF64, PnMomentsF64, Replicated, StatsError, WeightedMomentsF64,
};
use num_bigint::BigInt;
use num_traits::Signed;

// ---------- shared helpers ----------------------------------------------------

fn f64_units(x: f64) -> BigInt {
    let bits = x.to_bits();
    let neg = bits >> 63 != 0;
    let e = ((bits >> 52) & 0x7FF) as i64;
    let frac = bits & ((1u64 << 52) - 1);
    assert!(e != 0x7FF);
    let (m, ex) = if e == 0 {
        (frac, 0i64)
    } else {
        (frac | (1 << 52), e - 1)
    };
    let v = BigInt::from(m) << usize::try_from(ex).expect("nonneg");
    if neg {
        -v
    } else {
        v
    }
}

fn scaled_1300(v: f64) -> BigInt {
    f64_units(v) << (1300 - 1074)
}

fn assert_correctly_rounded(got: f64, p: &BigInt, q: &BigInt, what: &str) {
    assert!(got.is_finite(), "{what}: got {got}");
    let err = |v: f64| -> BigInt { ((p << 1300usize) - scaled_1300(v) * q).abs() };
    let e_got = err(got);
    let step = |up: bool| -> f64 {
        let b = got.to_bits();
        if b & !(1 << 63) == 0 {
            return f64::from_bits(if up { 1 } else { 1 | (1 << 63) });
        }
        let towards_zero = (got > 0.0) != up;
        f64::from_bits(if towards_zero { b - 1 } else { b + 1 })
    };
    assert!(
        e_got <= err(step(false)) && e_got <= err(step(true)),
        "{what}: {got:e} not nearest"
    );
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn mixed(&mut self, decades: i32) -> f64 {
        let m = 10f64.powi((self.next() % (2 * decades as u64 + 1)) as i32 - decades);
        (self.unit() * 2.0 - 1.0) * m
    }
}

// ---------- ExtremaF64 --------------------------------------------------------

#[test]
fn extrema_laws_and_totals() {
    let mut r = Rng(1);
    let xs: Vec<f64> = (0..500).map(|_| r.mixed(4)).collect();
    let mut a = ExtremaF64::new();
    let mut b = ExtremaF64::new();
    for (i, &x) in xs.iter().enumerate() {
        if i % 2 == 0 {
            a.add(x)
        } else {
            b.add(x)
        }
    }
    let mut ab = a.clone();
    Mergeable::merge(&mut ab, &b);
    let mut ba = b.clone();
    Mergeable::merge(&mut ba, &a);
    assert_eq!(ab.to_bytes(), ba.to_bytes(), "commutative");
    let lo = xs.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    assert_eq!(ab.min().expect("nonempty"), lo);
    assert_eq!(ab.max().expect("nonempty"), hi);
    // value-idempotent: merging a copy changes count only, not min/max
    let mut dup = ab.clone();
    Mergeable::merge(&mut dup, &ab.clone());
    assert_eq!(dup.min(), ab.min());
    assert_eq!(dup.max(), ab.max());
    // -0.0 < +0.0 in the total order (canonical bytes)
    let mut z = ExtremaF64::new();
    z.add(0.0);
    z.add(-0.0);
    assert_eq!(z.min().expect("nonempty").to_bits(), (-0.0f64).to_bits());
    assert_eq!(z.max().expect("nonempty").to_bits(), 0.0f64.to_bits());
    // NaN honesty
    let mut n = ExtremaF64::new();
    n.add(1.0);
    n.add(f64::NAN);
    assert_eq!(n.min(), None);
    // codec
    let rt = ExtremaF64::from_bytes(&ab.to_bytes()).expect("valid");
    assert_eq!(rt.to_bytes(), ab.to_bytes());
}

// ---------- WeightedMomentsF64 -------------------------------------------------

#[test]
fn weighted_moments_exactly_rounded_vs_oracle() {
    let mut r = Rng(7);
    for trial in 0..20 {
        let n = 3 + (r.next() % 200) as usize;
        let xs: Vec<f64> = (0..n).map(|_| r.mixed(4)).collect();
        let ws: Vec<f64> = (0..n).map(|_| r.unit() * 10.0).collect();
        let mut m = WeightedMomentsF64::new();
        for (&x, &w) in xs.iter().zip(&ws) {
            m.add(x, w);
        }
        // oracle: A = Σ uw·ux, B = Σ uw, C = Σ uw·ux²  (per-sample exact ints)
        let (mut a, mut b, mut c) = (BigInt::from(0u8), BigInt::from(0u8), BigInt::from(0u8));
        for (&x, &w) in xs.iter().zip(&ws) {
            let uw = f64_units(w);
            let ux = f64_units(x);
            a += &uw * &ux;
            c += &uw * &ux * &ux;
            b += uw;
        }
        // mean = A / (B·2^1074); variance = (C·B − A²) / (B²·2^2148)
        let mean = m.try_mean().expect("finite");
        assert_correctly_rounded(mean, &a, &(&b << 1074usize), &format!("wmean {trial}"));
        let var = m.try_variance().expect("finite");
        let p = &c * &b - &a * &a;
        let q = (&b * &b) << 2148usize;
        assert_correctly_rounded(var, &p, &q, &format!("wvar {trial}"));
    }
}

#[test]
fn weighted_moments_invariance_and_codec() {
    let mut r = Rng(9);
    let pairs: Vec<(f64, f64)> = (0..800).map(|_| (r.mixed(3), r.unit() * 5.0)).collect();
    let mut reference: Option<Vec<u8>> = None;
    for shards in [1usize, 3, 7, 16] {
        let mut parts: Vec<WeightedMomentsF64> =
            (0..shards).map(|_| WeightedMomentsF64::new()).collect();
        for (i, &(x, w)) in pairs.iter().enumerate() {
            parts[i % shards].add(x, w);
        }
        let mut whole = WeightedMomentsF64::new();
        for p in &parts {
            whole.merge(p);
        }
        let bytes = whole.to_bytes().to_vec();
        match &reference {
            None => reference = Some(bytes),
            Some(rf) => assert_eq!(&bytes, rf),
        }
    }
    let bytes = reference.expect("set");
    let arr: [u8; WeightedMomentsF64::BYTES] = bytes.as_slice().try_into().expect("len");
    let rt = WeightedMomentsF64::from_bytes(&arr).expect("valid");
    assert_eq!(rt.to_bytes().to_vec(), bytes);
}

// ---------- PnMomentsF64 --------------------------------------------------------

#[test]
fn pn_retraction_restores_reads_bit_exactly() {
    let mut r = Rng(21);
    let xs: Vec<f64> = (0..300).map(|_| r.mixed(4)).collect();
    let extra: Vec<f64> = (0..40).map(|_| r.mixed(4)).collect();
    // reference: adds only
    let mut base = PnMomentsF64::new();
    for &x in &xs {
        base.add(x);
    }
    // insert the extras, then retract them (in a different order)
    let mut churn = PnMomentsF64::new();
    for &x in &xs {
        churn.add(x);
    }
    for &x in &extra {
        churn.add(x);
    }
    for &x in extra.iter().rev() {
        churn.remove(x);
    }
    assert_eq!(churn.live_count(), base.live_count());
    assert_eq!(
        churn.try_mean().expect("ok").to_bits(),
        base.try_mean().expect("ok").to_bits(),
        "mean must return to identical bits after insert+retract"
    );
    assert_eq!(
        churn.try_variance().expect("ok").to_bits(),
        base.try_variance().expect("ok").to_bits(),
        "variance must return to identical bits after insert+retract"
    );
    // over-remove is an error, not a value
    let mut bad = PnMomentsF64::new();
    bad.add(1.0);
    bad.remove(1.0);
    bad.remove(2.0);
    assert_eq!(bad.try_mean(), Err(StatsError::Degenerate));
    // codec
    let rt = PnMomentsF64::from_bytes(&churn.to_bytes()).expect("valid");
    assert_eq!(rt.to_bytes().to_vec(), churn.to_bytes().to_vec());
}

// ---------- HistogramF64 --------------------------------------------------------

#[test]
fn histogram_counts_exact_and_quantile_bounds_hold() {
    let edges: Vec<f64> = (0..=20).map(|i| i as f64 * 0.5 - 5.0).collect();
    let mut r = Rng(33);
    let xs: Vec<f64> = (0..5000).map(|_| (r.unit() * 2.0 - 1.0) * 4.9).collect();
    // sharded build must equal whole build
    let mut whole = HistogramF64::new(edges.clone()).expect("valid edges");
    let mut a = HistogramF64::new(edges.clone()).expect("valid edges");
    let mut b = HistogramF64::new(edges.clone()).expect("valid edges");
    for (i, &x) in xs.iter().enumerate() {
        whole.add(x);
        if i % 2 == 0 {
            a.add(x)
        } else {
            b.add(x)
        }
    }
    Mergeable::merge(&mut a, &b);
    assert_eq!(a.encode_bytes(), whole.encode_bytes());
    assert_eq!(whole.total(), xs.len() as u64);
    // quantile bounds contain the true nearest-rank sample
    let mut sorted = xs.clone();
    sorted.sort_by(f64::total_cmp);
    for &q in &[0.1, 0.25, 0.5, 0.75, 0.9] {
        let rank = ((q * sorted.len() as f64).ceil() as usize).clamp(1, sorted.len());
        let truth = sorted[rank - 1];
        let (lo, hi) = whole.quantile_bounds(q).expect("in-range");
        assert!(
            lo <= truth && truth <= hi,
            "q={q}: {truth} not in [{lo}, {hi}]"
        );
    }
    // mismatched edges poison, reported
    let mut other = HistogramF64::new(vec![0.0, 1.0]).expect("valid edges");
    other.add(0.5);
    Mergeable::merge(&mut whole, &other);
    assert!(whole.counts().is_none());
    // NaN counted, not dropped
    let mut h = HistogramF64::new(vec![0.0, 1.0]).expect("valid edges");
    h.add(f64::NAN);
    assert_eq!(h.nan_count(), 1);
    // codec
    let rt = HistogramF64::decode_bytes(&a.encode_bytes()).expect("valid");
    assert_eq!(rt.encode_bytes(), a.encode_bytes());
}

// ---------- CovMatrixF64 --------------------------------------------------------

#[test]
fn covmatrix_entries_exactly_rounded_vs_oracle() {
    let mut r = Rng(55);
    let d = 3usize;
    let n = 400usize;
    let rows: Vec<Vec<f64>> = (0..n)
        .map(|_| (0..d).map(|_| r.mixed(3)).collect())
        .collect();
    let mut cm = CovMatrixF64::new(d);
    for row in &rows {
        cm.add(row, 0.0);
    }
    for i in 0..d {
        for j in i..d {
            let got = cm.try_covariance(i, j).expect("finite");
            // oracle: (n·Σuxi·uxj − Σuxi·Σuxj) / (n²·2^2148)
            let nb = BigInt::from(n as u64);
            let si: BigInt = rows.iter().map(|r| f64_units(r[i])).sum();
            let sj: BigInt = rows.iter().map(|r| f64_units(r[j])).sum();
            let qij: BigInt = rows.iter().map(|r| f64_units(r[i]) * f64_units(r[j])).sum();
            let p = &nb * qij - si * sj;
            let q = (&nb * &nb) << 2148usize;
            assert_correctly_rounded(got, &p, &q, &format!("cov({i},{j})"));
        }
    }
}

#[test]
fn covmatrix_regression_recovers_plane_and_is_bit_invariant() {
    let mut r = Rng(77);
    let d = 2usize;
    let rows: Vec<(Vec<f64>, f64)> = (0..600)
        .map(|_| {
            let x: Vec<f64> = (0..d).map(|_| (r.unit() * 2.0 - 1.0) * 10.0).collect();
            let y = 1.0 + 2.0 * x[0] - 3.0 * x[1];
            (x, y)
        })
        .collect();
    let mut reference: Option<Vec<u64>> = None;
    for shards in [1usize, 4, 9] {
        let mut parts: Vec<CovMatrixF64> = (0..shards).map(|_| CovMatrixF64::new(d)).collect();
        for (i, (x, y)) in rows.iter().enumerate() {
            parts[i % shards].add(x, *y);
        }
        let mut whole = CovMatrixF64::new(d);
        for p in &parts {
            CovMatrixF64::merge(&mut whole, p);
        }
        let beta = whole.try_regression().expect("well-conditioned");
        assert!((beta[0] - 1.0).abs() < 1e-9, "intercept {beta:?}");
        assert!((beta[1] - 2.0).abs() < 1e-9, "b1 {beta:?}");
        assert!((beta[2] + 3.0).abs() < 1e-9, "b2 {beta:?}");
        let bits: Vec<u64> = beta.iter().map(|b| b.to_bits()).collect();
        match &reference {
            None => reference = Some(bits),
            Some(rf) => assert_eq!(&bits, rf, "regression must be bit-invariant"),
        }
    }
    // dim mismatch poisons, reported
    let mut a = CovMatrixF64::new(2);
    a.add(&[1.0, 2.0], 3.0);
    let b = CovMatrixF64::new(3);
    CovMatrixF64::merge(&mut a, &b);
    assert_eq!(a.try_covariance(0, 0), Err(StatsError::Degenerate));
    // codec
    let mut c = CovMatrixF64::new(2);
    c.add(&[0.5, -1.5], 2.5);
    let rt = CovMatrixF64::decode(&Mergeable::encode(&c)).expect("valid");
    assert_eq!(Mergeable::encode(&rt), Mergeable::encode(&c));
}

// ---------- ConvergentMap / Replicated / Deltas ---------------------------------

#[test]
fn convergent_map_group_by_matches_whole() {
    let mut r = Rng(88);
    let keys = ["alpha", "beta", "gamma"];
    let data: Vec<(&str, f64)> = (0..900).map(|i| (keys[i % 3], r.mixed(3))).collect();
    let mut whole: ConvergentMap<String, MomentsF64> = ConvergentMap::new();
    let mut a: ConvergentMap<String, MomentsF64> = ConvergentMap::new();
    let mut b: ConvergentMap<String, MomentsF64> = ConvergentMap::new();
    for (i, (k, x)) in data.iter().enumerate() {
        whole.entry_or(k.to_string(), MomentsF64::new).add(*x);
        if i % 2 == 0 {
            a.entry_or(k.to_string(), MomentsF64::new).add(*x);
        } else {
            b.entry_or(k.to_string(), MomentsF64::new).add(*x);
        }
    }
    a.merge(&b);
    assert_eq!(a.encode(), whole.encode(), "sharded GROUP BY == whole");
    // expire_before drops strictly-below keys deterministically
    let mut w: ConvergentMap<String, MomentsF64> = ConvergentMap::new();
    for k in ["w01", "w02", "w03"] {
        w.entry_or(k.to_string(), MomentsF64::new).add(1.0);
    }
    w.expire_before(&"w02".to_string());
    assert_eq!(w.len(), 2);
    assert!(w.get(&"w01".to_string()).is_none());
}

#[test]
fn replicated_layer_is_lawful_for_any_mergeable() {
    let mut r = Rng(99);
    let mut a: Replicated<MomentsF64> = Replicated::new();
    let mut b: Replicated<MomentsF64> = Replicated::new();
    let mut c: Replicated<MomentsF64> = Replicated::new();
    for i in 0..600 {
        let x = r.mixed(3);
        match i % 3 {
            0 => a.local_mut(0, MomentsF64::new).add(x),
            1 => b.local_mut(1, MomentsF64::new).add(x),
            _ => c.local_mut(2, MomentsF64::new).add(x),
        }
    }
    // idempotent
    let mut aa = a.clone();
    aa.join(&a.clone());
    assert_eq!(aa.encode(), a.encode());
    // commutative
    let (mut ab, mut ba) = (a.clone(), b.clone());
    ab.join(&b);
    ba.join(&a);
    assert_eq!(ab.encode(), ba.encode());
    // associative + dup-safe
    let mut abc1 = ab.clone();
    abc1.join(&c);
    let mut bc = b.clone();
    bc.join(&c);
    let mut abc2 = a.clone();
    abc2.join(&bc);
    abc2.join(&ab); // re-delivery
    assert_eq!(abc1.encode(), abc2.encode());
    // converged reads identical
    let g1 = abc1.global(MomentsF64::new);
    let g2 = abc2.global(MomentsF64::new);
    assert_eq!(g1.variance().to_bits(), g2.variance().to_bits());
}

#[test]
fn deltas_transport_converges() {
    let mut sender: Deltas<MomentsF64> = Deltas::new(MomentsF64::new);
    let mut receiver = MomentsF64::new();
    let mut r = Rng(111);
    for _round in 0..5 {
        for _ in 0..100 {
            let x = r.mixed(3);
            sender.apply(|m| m.add(x));
        }
        let delta = sender.take_delta();
        assert_eq!(delta.count(), 100, "delta carries only the new adds");
        receiver.merge(&delta);
    }
    assert_eq!(
        receiver.to_bytes().to_vec(),
        sender.full().to_bytes().to_vec(),
        "receiver converges to sender's full state via deltas alone"
    );
}

// ---------- receipts -------------------------------------------------------------

#[test]
fn receipts_are_order_invariant_and_tamper_evident() {
    let mut r = Rng(123);
    let xs: Vec<f64> = (0..500).map(|_| r.mixed(4)).collect();
    let mut fwd = MomentsF64::new();
    let mut rev = MomentsF64::new();
    for &x in &xs {
        fwd.add(x);
    }
    for &x in xs.iter().rev() {
        rev.add(x);
    }
    assert_eq!(state_hash(&fwd), state_hash(&rev));
    let mut tampered = fwd.clone();
    tampered.add(1e-9);
    assert_ne!(state_hash(&fwd), state_hash(&tampered));
}
