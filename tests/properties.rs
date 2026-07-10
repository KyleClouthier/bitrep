// Copyright (c) 2026 Kyle Clouthier. Licensed under MIT OR Apache-2.0.
//! Property tests against an independent exact oracle.
//!
//! The oracle represents every finite f64 as an exact integer multiple of
//! 2^-1074 in a `num_bigint::BigInt`, sums exactly, and rounds with an
//! independently written nearest-even reference. Any disagreement between
//! bitrep and the oracle fails the suite.

use num_bigint::BigInt;
use num_traits::{Signed, Zero};
use proptest::prelude::*;

use bitrep::{DotF64, SumF32, SumF64};

/// Exact value of a finite f64 as an integer in units of 2^-1074.
fn to_units(x: f64) -> BigInt {
    assert!(x.is_finite());
    let bits = x.to_bits();
    let neg = bits >> 63 != 0;
    let expf = ((bits >> 52) & 0x7ff) as i64;
    let frac = bits & ((1u64 << 52) - 1);
    if expf == 0 && frac == 0 {
        return BigInt::zero();
    }
    let (m, e) = if expf == 0 {
        (frac, -1074i64)
    } else {
        (frac | (1 << 52), expf - 1075)
    };
    let v = BigInt::from(m) << (e + 1074) as u32;
    if neg {
        -v
    } else {
        v
    }
}

/// Independent reference rounding: exact integer (units 2^-1074) -> nearest
/// float with `mant` stored mantissa bits, min normal exponent `min_exp`,
/// max exponent `max_exp`. Ties to even. Written against the IEEE definition,
/// deliberately NOT sharing structure with the crate's implementation.
fn round_reference(units: &BigInt, mant: u32, min_exp: i32, max_exp: i32) -> f64 {
    if units.is_zero() {
        return 0.0;
    }
    let neg = units.is_negative();
    let mag = units.abs();
    let h = (mag.bits() - 1) as i32; // index of the leading bit
    let e = h - 1074;

    // Quantum (grid) exponent in units-space: subnormal grid is fixed, normal
    // grid follows the leading bit.
    let grid: i32 = if e < min_exp {
        min_exp - mant as i32 + 1074
    } else {
        h - mant as i32
    };

    let (q, rem) = if grid <= 0 {
        (mag.clone() << (-grid) as u32, BigInt::zero())
    } else {
        let div = BigInt::from(1) << grid as u32;
        let q = &mag >> grid as u32;
        let rem = &mag - (&q << grid as u32);
        let _ = div;
        (q, rem)
    };
    // Nearest-even on the remainder.
    let mut q = q;
    if grid > 0 {
        let half = BigInt::from(1) << (grid - 1) as u32;
        if rem > half || (rem == half && (&q % 2) == BigInt::from(1)) {
            q += 1;
        }
    }
    // Rebuild the value as q * 2^(grid - 1074) using exact f64 steps.
    // q fits in u128 for all cases we exercise (<= 2^(mant+2)).
    let q_u: u128 = {
        let (_, digits) = q.to_u64_digits();
        match digits.len() {
            0 => 0,
            1 => digits[0] as u128,
            2 => (digits[1] as u128) << 64 | digits[0] as u128,
            _ => panic!("rounded significand unexpectedly wide"),
        }
    };
    let exp2 = grid - 1074;
    // Overflow check: value = q * 2^exp2; compare against max finite.
    let leading = if q_u == 0 {
        return if neg { -0.0 } else { 0.0 };
    } else {
        127 - q_u.leading_zeros() as i32
    };
    if leading + exp2 > max_exp {
        return if neg {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        };
    }
    let v = (q_u as f64) * pow2_exact(exp2);
    if neg {
        -v
    } else {
        v
    }
}

/// 2^k built from bits. `powi` is WRONG here: powi(-1067) evaluates via
/// 1/2^1067 = 1/inf = 0 — a bug the differential fuzzer caught in this very
/// oracle (the crate was right, the reference was wrong). Exact for
/// k in [-1074, 1023].
fn pow2_exact(k: i32) -> f64 {
    assert!((-1074..=1023).contains(&k), "pow2_exact out of range: {k}");
    if k >= -1022 {
        f64::from_bits(((k + 1023) as u64) << 52)
    } else {
        f64::from_bits(1u64 << (k + 1074))
    }
}

fn oracle_sum_f64(xs: &[f64]) -> f64 {
    let total: BigInt = xs.iter().map(|&x| to_units(x)).sum();
    round_reference(&total, 52, -1022, 1023)
}

fn oracle_sum_f32(xs: &[f32]) -> f32 {
    let total: BigInt = xs.iter().map(|&x| to_units(x as f64)).sum();
    round_reference(&total, 23, -126, 127) as f32
}

/// Finite f64s spanning the whole dynamic range, including subnormals,
/// powers of two, negatives, zeros, and catastrophic-cancellation magnets.
fn finite_f64() -> impl Strategy<Value = f64> {
    prop_oneof![
        // raw bit patterns filtered to finite: hits subnormals + extremes hard
        any::<u64>()
            .prop_map(f64::from_bits)
            .prop_filter("finite", |x| x.is_finite()),
        // human-scale values
        -1e6f64..1e6,
        // exact powers of two across the range
        (-1074i32..=1023).prop_map(|e| if e >= -1022 {
            f64::from_bits(((e + 1023) as u64) << 52)
        } else {
            f64::from_bits(1u64 << (e + 1074))
        }),
        Just(0.0),
        Just(-0.0),
        Just(f64::MIN_POSITIVE),
        Just(f64::MAX),
        Just(-f64::MAX),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// The rounded value equals the independent oracle's exactly rounded sum.
    #[test]
    fn matches_oracle(xs in prop::collection::vec(finite_f64(), 0..200)) {
        let acc: SumF64 = xs.iter().copied().collect();
        let want = oracle_sum_f64(&xs);
        prop_assert_eq!(acc.value().to_bits(), want.to_bits(),
            "bitrep {} vs oracle {}", acc.value(), want);
    }

    /// Any permutation yields byte-identical state.
    #[test]
    fn order_invariant(xs in prop::collection::vec(finite_f64(), 0..200), seed in any::<u64>()) {
        let a: SumF64 = xs.iter().copied().collect();
        let mut shuffled = xs.clone();
        // Fisher-Yates with a deterministic xorshift (no rng dep).
        let mut s = seed | 1;
        for i in (1..shuffled.len()).rev() {
            s ^= s << 13; s ^= s >> 7; s ^= s << 17;
            shuffled.swap(i, (s % (i as u64 + 1)) as usize);
        }
        let b: SumF64 = shuffled.iter().copied().collect();
        prop_assert_eq!(a.to_bytes(), b.to_bytes());
    }

    /// Any sharding + any merge order yields byte-identical state.
    #[test]
    fn shard_invariant(xs in prop::collection::vec(finite_f64(), 0..200), cuts in prop::collection::vec(any::<usize>(), 0..6)) {
        let whole: SumF64 = xs.iter().copied().collect();
        //

        let mut bounds: Vec<usize> = cuts.iter().map(|c| if xs.is_empty() { 0 } else { c % (xs.len() + 1) }).collect();
        bounds.push(0); bounds.push(xs.len());
        bounds.sort_unstable();
        let shards: Vec<SumF64> = bounds.windows(2)
            .map(|w| xs[w[0]..w[1]].iter().copied().collect())
            .collect();
        // Merge right-to-left (a different association than left-to-right).
        let mut merged = SumF64::new();
        for s in shards.iter().rev() {
            merged.merge(s);
        }
        prop_assert_eq!(whole.to_bytes(), merged.to_bytes());
        prop_assert_eq!(whole.count(), xs.len() as u64);
    }

    /// Serialization round-trips the full state.
    #[test]
    fn bytes_roundtrip(xs in prop::collection::vec(finite_f64(), 0..64)) {
        let a: SumF64 = xs.iter().copied().collect();
        let b = SumF64::from_bytes(&a.to_bytes()).expect("valid bytes");
        prop_assert_eq!(&a, &b);
        prop_assert_eq!(a.value().to_bits(), b.value().to_bits());
    }

    /// f32 sums are exactly rounded — including where double rounding
    /// through f64 would give the wrong answer.
    #[test]
    fn f32_matches_oracle(xs in prop::collection::vec(any::<u32>().prop_map(f32::from_bits).prop_filter("finite", |x| x.is_finite()), 0..200)) {
        let acc: SumF32 = xs.iter().copied().collect();
        let want = oracle_sum_f32(&xs);
        prop_assert_eq!(acc.value().to_bits(), want.to_bits());
    }

    /// Dot products match the oracle computed over exact rational products.
    #[test]
    fn dot_matches_oracle(pairs in prop::collection::vec((-1e150f64..1e150, -1e150f64..1e150), 0..100)) {
        let mut d = DotF64::new();
        let mut total = BigInt::zero();
        let mut skip = false;
        for (a, b) in &pairs {
            d.push(*a, *b);
            // Oracle: exact product in units of 2^-2148 = units(a) * units(b) / 2^0...
            // Each f64 is m*2^e; product exact in BigInt at combined scale.
            // Using to_units at 2^-1074 each gives product at 2^-2148.
            total += to_units(*a) * to_units(*b);
            let p = a * b;
            if *a != 0.0 && *b != 0.0 && p.abs() < f64::MIN_POSITIVE { skip = true; }
        }
        prop_assume!(!skip); // underflow domain is excluded BY CONTRACT (and flagged by the API)
        prop_assert!(d.is_exact());
        // Round the 2^-2148-scaled exact dot with the reference: rescale by
        // shifting the reference grid (equivalently: round units*2^-2148).
        // round_reference expects units of 2^-1074, so shift down 1074 with
        // exact halving only when possible — instead reuse it by treating the
        // value as (total / 2^1074) in units of 2^-1074: do the division via
        // rounding directly at the wider scale below.
        let want = round_units_2148(&total);
        prop_assert_eq!(d.value().to_bits(), want.to_bits());
    }
}

/// Reference rounding for integers in units of 2^-2148 (dot-product scale).
fn round_units_2148(units: &BigInt) -> f64 {
    if units.is_zero() {
        return 0.0;
    }
    let neg = units.is_negative();
    let mag = units.abs();
    let h = (mag.bits() - 1) as i64;
    let e = h - 2148; // unbiased exponent of the leading bit
    let grid: i64 = if e < -1022 { -1022 - 52 + 2148 } else { h - 52 };
    let (mut q, rem) = if grid <= 0 {
        (mag.clone() << (-grid) as u32, BigInt::zero())
    } else {
        let q = &mag >> grid as u32;
        let rem = &mag - (&q << grid as u32);
        (q, rem)
    };
    if grid > 0 {
        let half = BigInt::from(1) << (grid - 1) as u32;
        if rem > half || (rem == half && (&q % 2) == BigInt::from(1)) {
            q += 1;
        }
    }
    let (_, digits) = q.to_u64_digits();
    let q_u: u128 = match digits.len() {
        0 => return if neg { -0.0 } else { 0.0 },
        1 => digits[0] as u128,
        2 => (digits[1] as u128) << 64 | digits[0] as u128,
        _ => panic!("rounded significand unexpectedly wide"),
    };
    let exp2 = (grid - 2148) as i32;
    let leading = 127 - q_u.leading_zeros() as i32;
    if leading + exp2 > 1023 {
        return if neg {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        };
    }
    // exp2 >= -1074 always holds at this scale (the subnormal grid bottoms
    // out at exp2 = -1074), so the bit-built power is exact — never powi.
    let v = (q_u as f64) * pow2_exact(exp2);
    if neg {
        -v
    } else {
        v
    }
}

// ---------- deterministic edge-case tests ----------

#[test]
fn catastrophic_cancellation() {
    // Naive summation returns 0.0 here; the exact answer is 1e-300.
    let xs = [1e100, 1e-300, -1e100];
    let naive: f64 = xs.iter().sum();
    let exact: SumF64 = xs.iter().copied().collect();
    assert_eq!(naive, 0.0, "premise: naive loses the small term");
    assert_eq!(exact.value(), 1e-300);
}

#[test]
fn double_rounding_trap_f32() {
    // Exact sum = 1 + 2^-24 + 2^-60. Correct f32 rounding: 1 + 2^-23
    // (the 2^-60 term breaks the tie upward). Rounding through f64 first
    // collapses the tie-breaker and ties-to-even gives 1.0 — the wrong bits.
    let xs = [1.0f32, 2f32.powi(-24), 2f32.powi(-60)];
    let via_f64 = (xs.iter().map(|&x| x as f64).sum::<f64>()) as f32;
    let direct: SumF32 = xs.iter().copied().collect();
    assert_eq!(
        via_f64, 1.0,
        "premise: double rounding through f64 is wrong"
    );
    assert_eq!(
        direct.value(),
        1.0 + 2f32.powi(-23),
        "bitrep rounds once, correctly"
    );
}

#[test]
fn specials_semantics() {
    let mut a = SumF64::new();
    a.add(f64::INFINITY);
    a.add(1.0);
    assert_eq!(a.value(), f64::INFINITY);

    let mut b = SumF64::new();
    b.add(f64::INFINITY);
    b.add(f64::NEG_INFINITY);
    assert!(b.value().is_nan());

    let mut c = SumF64::new();
    c.add(f64::NAN);
    c.add(42.0);
    assert!(c.value().is_nan());

    // Merging carries flags.
    let mut d = SumF64::new();
    d.add(f64::NEG_INFINITY);
    let mut e = SumF64::new();
    e.add(f64::INFINITY);
    d.merge(&e);
    assert!(d.value().is_nan());
}

#[test]
fn zeros_and_empty() {
    let empty = SumF64::new();
    assert_eq!(empty.value().to_bits(), 0.0f64.to_bits());
    let zeros: SumF64 = [0.0, -0.0, 0.0].iter().copied().collect();
    assert_eq!(zeros.value().to_bits(), 0.0f64.to_bits(), "canonical +0.0");
    assert_eq!(zeros.count(), 3);
}

#[test]
fn overflow_to_infinity() {
    let xs = [f64::MAX, f64::MAX];
    let acc: SumF64 = xs.iter().copied().collect();
    assert_eq!(
        acc.value(),
        f64::INFINITY,
        "exact sum 2*MAX overflows: correctly rounds to +inf"
    );
    let xs = [-f64::MAX, -f64::MAX, 1.0];
    let acc: SumF64 = xs.iter().copied().collect();
    assert_eq!(acc.value(), f64::NEG_INFINITY);
}

#[test]
fn max_cancels_exactly() {
    let xs = [f64::MAX, f64::MAX, -f64::MAX, -f64::MAX, 5.0];
    let acc: SumF64 = xs.iter().copied().collect();
    assert_eq!(
        acc.value(),
        5.0,
        "the overflow never happened in exact arithmetic"
    );
}

#[test]
fn subnormal_accumulation() {
    // 2^52 copies of the min subnormal sum to exactly 2^-1022 (min normal).
    // We can't add 2^52 values in a test, but 2 * 2^51-batch merging works:
    // instead verify a modest exact identity: 1024 * MIN = 2^-1064.
    let mut acc = SumF64::new();
    for _ in 0..1024 {
        acc.add(f64::from_bits(1)); // 2^-1074
    }
    // NB expected value built from bits, not powi (powi is untrustworthy at
    // extreme exponents — see pow2_exact).
    assert_eq!(acc.value(), f64::from_bits(1u64 << 10)); // 2^-1064
}

#[test]
fn dot_underflow_is_flagged_never_silent() {
    let mut d = DotF64::new();
    d.push(1e-200, 1e-200); // product 1e-400: subnormal-underflow domain
    assert!(!d.is_exact());
    assert!(d.try_value().is_err());
    let _ = d.value(); // still usable, documented semantics
}

#[test]
fn from_bytes_rejects_unknown_flags() {
    let a = SumF64::new();
    let mut b = a.to_bytes();
    b[bitrep::SumF64::BYTES - 9] = 0xFF; // flag byte
    assert!(SumF64::from_bytes(&b).is_none());
}
