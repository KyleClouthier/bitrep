// Copyright (c) 2026 Kyle Clouthier. Licensed under MIT OR Apache-2.0.
//! Rigorous verification of the `stats` feature.
//!
//! Three independent layers:
//! 1. an ORACLE that recomputes each statistic's exact rational from the RAW
//!    samples (independent accumulation path — per-sample IEEE decomposition
//!    into BigInt, no accumulator code shared);
//! 2. a CORRECT-ROUNDING CHECKER that proves the returned f64 is the nearest
//!    representable neighbor of the exact rational WITHOUT re-implementing
//!    the rounding algorithm (it compares the error of the result against
//!    both f64 neighbors by exact integer arithmetic);
//! 3. bit-invariance sweeps across shardings, orders and merge trees.

#![cfg(feature = "stats")]

use bitrep::{CovF64, Moments4F64, MomentsF64, StatsError, SumF64};
use num_bigint::BigInt;
use num_traits::{Signed, Zero};

// ---------- independent exact oracle over raw samples -----------------------

/// x as an exact integer multiple of 2^-1074 — straight from the IEEE fields.
fn f64_units(x: f64) -> BigInt {
    let bits = x.to_bits();
    let neg = bits >> 63 != 0;
    let e = ((bits >> 52) & 0x7FF) as i64;
    let frac = bits & ((1u64 << 52) - 1);
    assert!(e != 0x7FF, "oracle handles finite only");
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

/// Exact Σx^k (k = 1..=4) over raw samples, in units 2^(-1074·k).
fn oracle_power_sums(xs: &[f64]) -> [BigInt; 4] {
    let mut acc = [
        BigInt::zero(),
        BigInt::zero(),
        BigInt::zero(),
        BigInt::zero(),
    ];
    for &x in xs {
        let u = f64_units(x);
        let u2 = &u * &u;
        let u3 = &u2 * &u;
        let u4 = &u3 * &u;
        acc[0] += u;
        acc[1] += u2;
        acc[2] += u3;
        acc[3] += u4;
    }
    acc
}

const U: usize = 1074;

fn upow(k: usize) -> BigInt {
    BigInt::from(1u8) << (U * k)
}

/// Exact population-variance rational (num, den) from raw samples.
fn oracle_variance(xs: &[f64]) -> (BigInt, BigInt) {
    let n = BigInt::from(xs.len() as u64);
    let [s, q2, _, _] = oracle_power_sums(xs);
    // Σx in u, Σx² in u²: var = (n·Q2 − S²)/(n²·u²)
    let num = &n * q2 - (&s * &s);
    let den = &n * &n * upow(2);
    (num, den)
}

/// Exact A2/A3/A4 central-moment numerators (common-denominator form) from
/// raw samples: μk = Ak·u^k... with all sums already in matched u^k units the
/// n-scalings are: A2 = nQ2−S², A3 = n²Q3−3nQ2'S+2S³ (units aligned), etc.
fn oracle_a234(xs: &[f64]) -> (BigInt, BigInt, BigInt) {
    let n = BigInt::from(xs.len() as u64);
    let [s, q2, q3, q4] = oracle_power_sums(xs);
    // units: S ~ u, Q2 ~ u², Q3 ~ u³, Q4 ~ u⁴ — already homogeneous:
    let a2 = &n * &q2 - (&s * &s); // ~u²
    let a3 = &n * &n * &q3 - BigInt::from(3u8) * &n * &q2 * &s + BigInt::from(2u8) * &s * &s * &s; // ~u³
    let a4 = &n * &n * &n * &q4 - BigInt::from(4u8) * &n * &n * &q3 * &s
        + BigInt::from(6u8) * &n * &q2 * &s * &s
        - BigInt::from(3u8) * &s * &s * &s * &s; // ~u⁴
    (a2, a3, a4)
}

// ---------- correct-rounding checker (no shared rounding code) ---------------

/// Exact integer of v·2^1300 (valid for every finite f64: 2^-1074·2^1300 ∈ ℤ).
fn scaled_1300(v: f64) -> BigInt {
    f64_units(v) << (1300 - 1074)
}

/// Assert `got` is the round-to-nearest (ties-to-even) f64 of exact `p/q`
/// (q > 0), by comparing exact errors against both neighbors.
fn assert_correctly_rounded(got: f64, p: &BigInt, q: &BigInt, what: &str) {
    assert!(got.is_finite(), "{what}: expected finite, got {got}");
    let err = |v: f64| -> BigInt { ((p << 1300usize) - scaled_1300(v) * q).abs() };
    let e_got = err(got);
    let below = f64::from_bits(if got.to_bits() & !(1 << 63) == 0 {
        1 | (1u64 << 63) // below +0.0 is -min_subnormal
    } else if got > 0.0 {
        got.to_bits() - 1
    } else {
        got.to_bits() + 1
    });
    let above = f64::from_bits(if got.to_bits() & !(1 << 63) == 0 {
        1 // above ±0.0 is +min_subnormal
    } else if got > 0.0 {
        got.to_bits() + 1
    } else {
        got.to_bits() - 1
    });
    let e_below = err(below);
    let e_above = err(above);
    assert!(
        e_got <= e_below && e_got <= e_above,
        "{what}: {got:e} is not nearest (below {below:e}, above {above:e})"
    );
    // ties must land on even mantissa
    if e_got == e_below || e_got == e_above {
        assert_eq!(got.to_bits() & 1, 0, "{what}: tie not broken to even");
    }
}

// ---------- data generators ---------------------------------------------------

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
    fn perm(&mut self, n: usize) -> Vec<usize> {
        let mut v: Vec<usize> = (0..n).collect();
        for i in (1..n).rev() {
            let j = (self.next() % (i as u64 + 1)) as usize;
            v.swap(i, j);
        }
        v
    }
}

// ---------- tests -------------------------------------------------------------

#[test]
fn mean_of_single_value_matches_lean_verified_kernel() {
    // mean with n=1 must reproduce SumF64::value() (the Lean-proven rounding
    // kernel) bit-for-bit — cross-validation of the bigint read path.
    let mut r = Rng(42);
    for _ in 0..20_000 {
        let x = r.mixed(300);
        let mut m = MomentsF64::new();
        m.add(x);
        let mut s = SumF64::new();
        s.add(x);
        assert_eq!(m.mean().to_bits(), s.value().to_bits(), "x={x:e}");
    }
    // subnormals too
    for bits in [1u64, 2, 3, 0xF_FFFF_FFFF_FFFF, (1 << 52) - 1] {
        let x = f64::from_bits(bits);
        let mut m = MomentsF64::new();
        m.add(x);
        assert_eq!(m.mean().to_bits(), x.to_bits());
    }
}

#[test]
fn variance_is_exactly_rounded_vs_independent_oracle() {
    let mut r = Rng(7);
    for trial in 0..50 {
        let n = 3 + (r.next() % 500) as usize;
        let xs: Vec<f64> = (0..n).map(|_| r.mixed(6)).collect();
        let mut m = MomentsF64::new();
        for &x in &xs {
            m.add(x);
        }
        let v = m.try_variance().expect("finite data");
        let (p, q) = oracle_variance(&xs);
        assert_correctly_rounded(v, &p, &q, &format!("variance trial {trial}"));
    }
}

#[test]
fn catastrophic_cancellation_is_exact() {
    // mean ~1e8, spread ~1e-3: the textbook formula loses ~22 digits.
    let mut r = Rng(99);
    let xs: Vec<f64> = (0..4096)
        .map(|_| 1.0e8 + (r.unit() * 2.0 - 1.0) * 1e-3)
        .collect();
    let mut m = MomentsF64::new();
    for &x in &xs {
        m.add(x);
    }
    let v = m.try_variance().expect("finite");
    let (p, q) = oracle_variance(&xs);
    assert_correctly_rounded(v, &p, &q, "cancellation variance");
    // the naive formula really does fail here (sanity that the regime bites)
    let sumsq: f64 = xs.iter().map(|x| x * x).sum();
    let mean: f64 = xs.iter().sum::<f64>() / xs.len() as f64;
    let naive = sumsq / xs.len() as f64 - mean * mean;
    let rel = ((naive - v) / v).abs();
    assert!(
        rel > 1e3,
        "expected naive to be catastrophically wrong, rel={rel:e}"
    );
}

#[test]
fn subnormal_and_huge_variance_round_correctly() {
    // spread ~1e-160 around a 1e-150 base (representable: ulp(1e-150)~1e-166):
    // squares (~1e-300) stay normal, the variance (~1e-321) lands subnormal.
    let base = 1.0e-150;
    let xs: Vec<f64> = vec![
        base + 1.0e-160,
        base - 1.0e-160,
        base + 3.0e-160,
        base - 3.0e-160,
    ];
    let mut m = MomentsF64::new();
    for &x in &xs {
        m.add(x);
    }
    let v = m
        .try_variance()
        .expect("squares ~1e-200 are normal: exactness holds");
    assert!(
        v > 0.0 && v < f64::MIN_POSITIVE,
        "variance should be subnormal, got {v:e}"
    );
    let (p, q) = oracle_variance(&xs);
    assert_correctly_rounded(v, &p, &q, "tiny variance");

    // huge data: x² overflows inside the accumulator -> the state is
    // non-finite and the error is reported, never a fabricated value.
    // (This is DotF64's named limit: |x| must stay ≤ ~1.34e154 for x².)
    let xs: Vec<f64> = vec![1.0e300, -1.0e300];
    let mut m = MomentsF64::new();
    for &x in &xs {
        m.add(x);
    }
    assert_eq!(m.try_variance(), Err(StatsError::NonFinite));
}

#[test]
fn bit_invariance_across_shardings_orders_trees() {
    let mut r = Rng(0xBEEF);
    let xs: Vec<f64> = (0..3000).map(|_| r.mixed(5)).collect();
    let mut reference: Option<Vec<u8>> = None;
    for _ in 0..60 {
        let shards = 1 + (r.next() % 24) as usize;
        let order = r.perm(xs.len());
        let mut parts: Vec<MomentsF64> = (0..shards).map(|_| MomentsF64::new()).collect();
        for (pos, &i) in order.iter().enumerate() {
            parts[pos % shards].add(xs[i]);
        }
        while parts.len() > 1 {
            let a = (r.next() % parts.len() as u64) as usize;
            let mut m = parts.swap_remove(a);
            let b = (r.next() % parts.len() as u64) as usize;
            m.merge(&parts[b]);
            parts[b] = m;
        }
        let bytes = parts[0].to_bytes().to_vec();
        match &reference {
            None => reference = Some(bytes),
            Some(rf) => assert_eq!(&bytes, rf, "sharding changed the state bytes"),
        }
    }
}

#[test]
fn moments4_skewness_kurtosis_exactly_rounded() {
    let mut r = Rng(1234);
    for trial in 0..25 {
        let n = 4 + (r.next() % 200) as usize;
        // keep |x| within the certified 3rd/4th-moment domain
        let xs: Vec<f64> = (0..n).map(|_| r.mixed(3)).collect();
        let mut m = Moments4F64::new();
        for &x in &xs {
            m.add(x);
        }
        let (a2, a3, a4) = oracle_a234(&xs);
        if a2.is_zero() {
            continue;
        }
        let kurt = m.try_kurtosis().expect("in-domain data");
        // kurtosis = (A4/u⁴-scale)/(A2/u²-scale)² — homogeneous: A4·u⁰/A2²
        // oracle units: A2 ~ u², A4 ~ u⁴ -> A4/A2² is unitless. Same for impl.
        assert_correctly_rounded(kurt, &a4, &(&a2 * &a2), &format!("kurtosis {trial}"));
        let s2 = m.try_skewness_squared().expect("in-domain");
        let p = &a3 * &a3;
        let q = &a2 * &a2 * &a2;
        assert_correctly_rounded(s2.abs(), &p, &q, &format!("skew^2 {trial}"));
        assert_eq!(s2.is_sign_negative(), a3.is_negative(), "skew sign {trial}");
    }
}

#[test]
fn moments4_symmetric_data_has_zero_skewness() {
    let mut m = Moments4F64::new();
    for x in [-3.0f64, -1.0, 1.0, 3.0, -2.5, 2.5] {
        m.add(x);
    }
    assert_eq!(m.try_skewness().expect("finite"), 0.0);
}

#[test]
fn covariance_regression_exact_on_exact_line() {
    // y = 2x + 1 exactly representable: slope/intercept/r² must be EXACT.
    let mut c = CovF64::new();
    for i in 0..1000 {
        let x = (i as f64) * 0.5 - 250.0;
        c.add(x, 2.0 * x + 1.0);
    }
    assert_eq!(c.try_slope().expect("nondegenerate"), 2.0);
    assert_eq!(c.try_intercept().expect("nondegenerate"), 1.0);
    assert_eq!(c.try_r_squared().expect("nondegenerate"), 1.0);
    assert_eq!(c.try_correlation().expect("nondegenerate"), 1.0);
}

#[test]
fn covariance_slope_exactly_rounded_vs_oracle() {
    let mut r = Rng(777);
    for trial in 0..25 {
        let n = 3 + (r.next() % 300) as usize;
        let xs: Vec<f64> = (0..n).map(|_| r.mixed(4)).collect();
        let ys: Vec<f64> = xs.iter().map(|&x| 0.75 * x + r.mixed(2)).collect();
        let mut c = CovF64::new();
        for (&x, &y) in xs.iter().zip(&ys) {
            c.add(x, y);
        }
        // oracle: Bxy = nΣxy − ΣxΣy, Bxx = nΣx² − (Σx)², slope = Bxy/Bxx
        let n_b = BigInt::from(n as u64);
        let sx: BigInt = xs.iter().map(|&x| f64_units(x)).sum();
        let sy: BigInt = ys.iter().map(|&y| f64_units(y)).sum();
        let sxy: BigInt = xs
            .iter()
            .zip(&ys)
            .map(|(&x, &y)| f64_units(x) * f64_units(y))
            .sum();
        let sxx: BigInt = xs
            .iter()
            .map(|&x| {
                let u = f64_units(x);
                &u * &u
            })
            .sum();
        let bxy = &n_b * sxy - &sx * &sy;
        let bxx = &n_b * sxx - &sx * &sx;
        if bxx.is_zero() {
            continue;
        }
        let slope = c.try_slope().expect("nondegenerate");
        let (p, q) = if bxx.is_negative() {
            (-bxy, -bxx)
        } else {
            (bxy, bxx)
        };
        assert_correctly_rounded(slope, &p, &q, &format!("slope {trial}"));
    }
}

#[test]
fn merge_is_commutative_and_associative() {
    let mut r = Rng(31337);
    let make = |r: &mut Rng, n: usize| {
        let mut m = MomentsF64::new();
        for _ in 0..n {
            m.add(r.mixed(4));
        }
        m
    };
    let a = make(&mut r, 100);
    let b = make(&mut r, 57);
    let c = make(&mut r, 211);
    let mut ab = a.clone();
    ab.merge(&b);
    let mut ba = b.clone();
    ba.merge(&a);
    assert_eq!(
        ab.to_bytes().to_vec(),
        ba.to_bytes().to_vec(),
        "commutativity"
    );
    let mut ab_c = ab.clone();
    ab_c.merge(&c);
    let mut bc = b.clone();
    bc.merge(&c);
    let mut a_bc = a.clone();
    a_bc.merge(&bc);
    assert_eq!(
        ab_c.to_bytes().to_vec(),
        a_bc.to_bytes().to_vec(),
        "associativity"
    );
}

#[test]
fn codecs_round_trip() {
    let mut r = Rng(4242);
    let mut m = MomentsF64::new();
    let mut m4 = Moments4F64::new();
    let mut c = CovF64::new();
    for _ in 0..500 {
        let x = r.mixed(3);
        let y = r.mixed(3);
        m.add(x);
        m4.add(x);
        c.add(x, y);
    }
    let m2 = MomentsF64::from_bytes(&m.to_bytes()).expect("valid");
    assert_eq!(m2.to_bytes().to_vec(), m.to_bytes().to_vec());
    assert_eq!(m2.variance().to_bits(), m.variance().to_bits());
    let m42 = Moments4F64::from_bytes(&m4.to_bytes()).expect("valid");
    assert_eq!(m42.to_bytes().to_vec(), m4.to_bytes().to_vec());
    assert_eq!(m42.kurtosis().to_bits(), m4.kurtosis().to_bits());
    let c2 = CovF64::from_bytes(&c.to_bytes()).expect("valid");
    assert_eq!(c2.to_bytes().to_vec(), c.to_bytes().to_vec());
    assert_eq!(c2.slope().to_bits(), c.slope().to_bits());
}

#[test]
fn errors_are_honest() {
    // empty
    assert_eq!(MomentsF64::new().try_mean(), Err(StatsError::Empty));
    // non-finite
    let mut m = MomentsF64::new();
    m.add(f64::NAN);
    assert_eq!(m.try_mean(), Err(StatsError::NonFinite));
    let mut m = MomentsF64::new();
    m.add(f64::INFINITY);
    assert_eq!(m.try_variance(), Err(StatsError::NonFinite));
    // exactness lost: x² underflows to subnormal
    let mut m = MomentsF64::new();
    m.add(1.0e-200);
    m.add(1.0);
    assert_eq!(m.try_variance(), Err(StatsError::ExactnessLost));
    // Moments4: x³ tail underflows well before x² does
    let mut m4 = Moments4F64::new();
    m4.add(1.0e-120);
    m4.add(1.0);
    assert!(matches!(m4.try_kurtosis(), Err(StatsError::ExactnessLost)));
    // degenerate: zero variance in regression denominator
    let mut c = CovF64::new();
    c.add(2.0, 1.0);
    c.add(2.0, 5.0);
    assert_eq!(c.try_slope(), Err(StatsError::Degenerate));
}
