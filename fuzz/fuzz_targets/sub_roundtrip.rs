// Copyright (c) 2026 Kyle Clouthier / Clouthier Simulation Labs. Licensed under MIT OR Apache-2.0.
//! Coverage-guided check of the exact-tier group laws: for fuzzer-chosen
//! float vectors split A|B, state(A ++ B).try_unmerge(state(B)) must be
//! byte-identical to state(A) — for SumF64 and for CovMatrixF64 (d=2),
//! including the regression_exact readout when defined.
#![no_main]

use bitrep::{CovMatrixF64, SumF64};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() < 9 {
        return;
    }
    let split = data[0] as usize;
    let vals: Vec<f64> = data[1..]
        .chunks_exact(8)
        .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
        .filter(|x| x.is_finite())
        .take(64)
        .collect();
    if vals.len() < 2 {
        return;
    }
    let k = split % vals.len();
    let (a, b) = vals.split_at(k);

    // SumF64 group law
    let sa: SumF64 = a.iter().copied().collect();
    let sb: SumF64 = b.iter().copied().collect();
    let mut sab: SumF64 = vals.iter().copied().collect();
    assert!(sab.try_unmerge(&sb), "finite unmerge must be accepted");
    assert_eq!(sab.to_bytes(), sa.to_bytes(), "sum downdate must be exact");

    // CovMatrixF64 group law (rows of 3: x0, x1, y)
    let rows: Vec<(&[f64], f64)> = vals.chunks_exact(3).map(|c| (&c[..2], c[2])).collect();
    if rows.len() >= 2 {
        let k2 = split % rows.len();
        let mut ca = CovMatrixF64::new(2);
        let mut cb = CovMatrixF64::new(2);
        let mut cab = CovMatrixF64::new(2);
        for (i, (x, y)) in rows.iter().enumerate() {
            cab.add(x, *y);
            if i < k2 {
                ca.add(x, *y);
            } else {
                cb.add(x, *y);
            }
        }
        // Contract: acceptance => exact roundtrip; refusal (e.g. a product's
        // TwoProduct error term underflowed, setting the sticky semilattice
        // flag -- possible even for finite inputs) => state untouched.
        let before = cab.encode();
        match cab.try_sub(&cb) {
            Ok(()) => {
                assert_eq!(cab.encode(), ca.encode(), "cov downdate must be exact");
                if let (Ok(x), Ok(y)) = (ca.try_regression_exact(), cab.try_regression_exact()) {
                    for (p, q) in x.iter().zip(&y) {
                        assert_eq!(p.to_bits(), q.to_bits(), "exact regression must agree");
                    }
                }
            }
            Err(_) => {
                assert_eq!(cab.encode(), before, "refusal must leave state untouched");
            }
        }
    }
});
