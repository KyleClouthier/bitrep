// Copyright (c) 2026 Kyle Clouthier. Licensed under MIT OR Apache-2.0.
//! Coverage-guided differential fuzzing: bitrep's exact sum vs an independent
//! BigInt oracle, plus order/shard invariance, on fuzzer-chosen inputs.
#![no_main]

use libfuzzer_sys::fuzz_target;
use num_bigint::BigInt;
use num_traits::{Signed, Zero};

fn to_units(x: f64) -> BigInt {
    let bits = x.to_bits();
    let neg = bits >> 63 != 0;
    let expf = ((bits >> 52) & 0x7ff) as i64;
    let frac = bits & ((1u64 << 52) - 1);
    if expf == 0 && frac == 0 {
        return BigInt::zero();
    }
    let (m, e) = if expf == 0 { (frac, -1074i64) } else { (frac | (1 << 52), expf - 1075) };
    let v = BigInt::from(m) << (e + 1074) as u32;
    if neg { -v } else { v }
}

/// Reference rounding to f64 (nearest-even), written independently.
fn round_reference(units: &BigInt) -> f64 {
    if units.is_zero() {
        return 0.0;
    }
    let neg = units.is_negative();
    let mag = units.abs();
    let h = (mag.bits() - 1) as i32;
    let grid: i32 = if h - 1074 < -1022 { 0 } else { h - 52 };
    let (mut q, rem) = if grid <= 0 {
        (mag.clone(), BigInt::zero())
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
        _ => unreachable!("significand too wide"),
    };
    let exp2 = grid - 1074;
    let leading = 127 - q_u.leading_zeros() as i32;
    if leading + exp2 > 1023 {
        return if neg { f64::NEG_INFINITY } else { f64::INFINITY };
    }
    let v = (q_u as f64) * pow2_exact(exp2);
    if neg { -v } else { v }
}

/// 2^k from bits. `powi` is WRONG at extreme exponents (powi(-1067)
/// evaluates via 1/2^1067 = 1/inf = 0) — the first bug this fuzzer found
/// was in its own oracle, right here. The crate was correct.
fn pow2_exact(k: i32) -> f64 {
    assert!((-1074..=1023).contains(&k));
    if k >= -1022 {
        f64::from_bits(((k + 1023) as u64) << 52)
    } else {
        f64::from_bits(1u64 << (k + 1074))
    }
}

fuzz_target!(|data: &[u8]| {
    // Interpret input as f64s; keep finite ones. Cap length for throughput.
    let xs: Vec<f64> = data
        .chunks_exact(8)
        .take(256)
        .map(|c| f64::from_bits(u64::from_le_bytes(c.try_into().unwrap())))
        .filter(|x| x.is_finite())
        .collect();

    let forward: bitrep::SumF64 = xs.iter().copied().collect();

    // Reversed order must be byte-identical.
    let mut reversed = bitrep::SumF64::new();
    for x in xs.iter().rev() {
        reversed.add(*x);
    }
    assert_eq!(forward.to_bytes(), reversed.to_bytes());

    // Fuzzer-chosen shard split must be byte-identical.
    if !xs.is_empty() {
        let cut = (data.first().copied().unwrap_or(0) as usize) % (xs.len() + 1);
        let mut sharded: bitrep::SumF64 = xs[..cut].iter().copied().collect();
        sharded.merge(&xs[cut..].iter().copied().collect());
        assert_eq!(forward.to_bytes(), sharded.to_bytes());
    }

    // Rounded value must equal the independent oracle.
    let total: BigInt = xs.iter().map(|&x| to_units(x)).sum();
    let want = round_reference(&total);
    assert_eq!(forward.value().to_bits(), want.to_bits());
});
