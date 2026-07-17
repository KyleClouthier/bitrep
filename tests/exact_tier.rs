#![cfg(feature = "stats")]
// Exact-tier integration tests (branch: exact-tier): group subtraction +
// correctly rounded regression. Validation lineage: probes 472/474/475.
use bitrep::{CovMatrixF64, SumF64};

fn cov_of(rows: &[(&[f64], f64)], d: usize) -> CovMatrixF64 {
    let mut c = CovMatrixF64::new(d);
    for (x, y) in rows {
        c.add(x, *y);
    }
    c
}

fn borrow(v: &[(Vec<f64>, f64)]) -> Vec<(&[f64], f64)> {
    v.iter().map(|(x, y)| (x.as_slice(), *y)).collect()
}

#[test]
fn sum_unmerge_roundtrip_bytes() {
    let a = [0.5, 1e100, -1e100, 0.25, 1e-300, -0.875];
    let b = [3.25, -7.5, 1e-17, 2.0_f64.powi(-60)];
    let sa: SumF64 = a.iter().copied().collect();
    let sb: SumF64 = b.iter().copied().collect();
    let mut sab: SumF64 = a.iter().chain(&b).copied().collect();
    assert!(sab.try_unmerge(&sb));
    assert_eq!(sab.to_bytes(), sa.to_bytes());
    assert_eq!(sab.value().to_bits(), sa.value().to_bits());
}

#[test]
fn sum_unmerge_refuses_specials_and_overcount() {
    let mut s: SumF64 = [1.0, 2.0].iter().copied().collect();
    let mut nanful = SumF64::new();
    nanful.add(f64::NAN);
    assert!(!s.try_unmerge(&nanful));
    let big: SumF64 = [1.0, 2.0, 3.0].iter().copied().collect();
    assert!(!s.try_unmerge(&big)); // count would go negative
    assert_eq!(s.count(), 2); // untouched on refusal
}

#[test]
fn cov_downdate_matches_never_added() {
    let rows_a: Vec<(Vec<f64>, f64)> = (0..40)
        .map(|i| {
            let x = i as f64 * 0.37;
            (vec![x, x * x * 0.01], 1.0 + 2.0 * x)
        })
        .collect();
    let rows_b: Vec<(Vec<f64>, f64)> = (0..25)
        .map(|i| {
            let x = 100.0 - i as f64 * 1.7;
            (vec![x, x.sqrt()], 5.0 - 0.3 * x)
        })
        .collect();
    let ca = cov_of(&borrow(&rows_a), 2);
    let cb = cov_of(&borrow(&rows_b), 2);
    let mut cab = cov_of(&borrow(&rows_a), 2);
    cab.merge(&cb);
    cab.try_sub(&cb).expect("downdate must succeed");
    assert_eq!(cab.encode(), ca.encode()); // byte-identical: exact unlearning
}

#[test]
fn cov_sub_all_or_nothing_on_refusal() {
    let mut c = cov_of(&[(&[1.0, 2.0][..], 3.0), (&[2.0, 1.0][..], 4.0)], 2);
    let snapshot = c.encode();
    let mut bad = CovMatrixF64::new(2);
    bad.add(&[f64::INFINITY, 1.0], 1.0);
    assert!(c.try_sub(&bad).is_err());
    assert_eq!(c.encode(), snapshot); // untouched
}

#[test]
fn regression_exact_exact_fit_is_exact() {
    // y = 1 + 2x exactly: zero residual, coefficients exactly representable.
    let rows: Vec<(Vec<f64>, f64)> = (1..=6)
        .map(|i| (vec![i as f64], 1.0 + 2.0 * i as f64))
        .collect();
    let c = cov_of(
        &rows
            .iter()
            .map(|(x, y)| (x.as_slice(), *y))
            .collect::<Vec<_>>(),
        1,
    );
    let beta = c.try_regression_exact().unwrap();
    assert_eq!(beta[0].to_bits(), 1.0_f64.to_bits());
    assert_eq!(beta[1].to_bits(), 2.0_f64.to_bits());
}

#[test]
fn regression_exact_agrees_with_deterministic_when_well_conditioned() {
    let rows: Vec<(Vec<f64>, f64)> = (0..200)
        .map(|i| {
            let x1 = (i as f64 * 0.7).sin() * 3.0;
            let x2 = (i as f64 * 0.3).cos() * 2.0;
            (
                vec![x1, x2],
                0.5 + 1.5 * x1 - 0.75 * x2 + ((i * 37 % 11) as f64 - 5.0) * 0.01,
            )
        })
        .collect();
    let c = cov_of(
        &rows
            .iter()
            .map(|(x, y)| (x.as_slice(), *y))
            .collect::<Vec<_>>(),
        2,
    );
    let det = c.try_regression().unwrap();
    let exact = c.try_regression_exact().unwrap();
    for (a, b) in det.iter().zip(&exact) {
        assert!((a - b).abs() < 1e-9, "det {a} vs exact {b}");
    }
}

#[test]
fn regression_exact_order_and_shard_invariant_bits() {
    let rows: Vec<(Vec<f64>, f64)> = (0..60)
        .map(|i| {
            (
                vec![i as f64 * 1.1, (i as f64).powi(2) * 0.02],
                i as f64 * 3.3 - 7.0,
            )
        })
        .collect();
    let all = rows
        .iter()
        .map(|(x, y)| (x.as_slice(), *y))
        .collect::<Vec<_>>();
    let c1 = cov_of(&all, 2);
    let mut rev = all.clone();
    rev.reverse();
    let c2 = cov_of(&rev, 2);
    let mut c3 = cov_of(&all[..17], 2);
    c3.merge(&cov_of(&all[17..], 2));
    let b1 = c1.try_regression_exact().unwrap();
    for c in [&c2, &c3] {
        let b = c.try_regression_exact().unwrap();
        for (x, y) in b1.iter().zip(&b) {
            assert_eq!(x.to_bits(), y.to_bits());
        }
    }
}

#[test]
fn downdate_then_regress_equals_regress_of_remainder() {
    let keep: Vec<(Vec<f64>, f64)> = (0..30)
        .map(|i| (vec![i as f64, (i as f64).powi(2) * 0.03], i as f64 * 2.0))
        .collect();
    let gone: Vec<(Vec<f64>, f64)> = (0..500)
        .map(|i| (vec![i as f64 * 0.01, 3.0], 42.0))
        .collect();
    let ck = cov_of(&borrow(&keep), 2);
    let cg = cov_of(&borrow(&gone), 2);
    let mut pooled = cov_of(&borrow(&keep), 2);
    pooled.merge(&cg);
    pooled.try_sub(&cg).unwrap();
    // removing 94% of the data: exact at any removal fraction
    let a = ck.try_regression_exact().unwrap();
    let b = pooled.try_regression_exact().unwrap();
    for (x, y) in a.iter().zip(&b) {
        assert_eq!(x.to_bits(), y.to_bits());
    }
}

// ---------------------------------------------------------------------------
// Independent exact oracle: plain rational Gaussian elimination over
// (BigInt, BigInt) fractions — a DIFFERENT algorithm from the library's
// fraction-free Bareiss + Cramer — must agree with try_regression_exact to
// the bit on randomized systems. Kani cannot model BigInt (heap); this
// differential oracle is the verification instrument for the exact solve.
mod oracle {
    use num_bigint::BigInt;

    #[derive(Clone)]
    pub struct Q(pub BigInt, pub BigInt); // num/den, den > 0, not normalized

    impl Q {
        pub fn from_f64(x: f64) -> Q {
            // exact: x = m * 2^e
            let bits = x.to_bits();
            let neg = bits >> 63 != 0;
            let expf = ((bits >> 52) & 0x7ff) as i64;
            let frac = bits & ((1u64 << 52) - 1);
            if expf == 0 && frac == 0 {
                return Q(BigInt::from(0), BigInt::from(1));
            }
            let (m, e) = if expf == 0 {
                (frac, -1074i64)
            } else {
                (frac | (1 << 52), expf - 1075)
            };
            let m = if neg {
                -BigInt::from(m)
            } else {
                BigInt::from(m)
            };
            if e >= 0 {
                Q(m << e as u32, BigInt::from(1))
            } else {
                Q(m, BigInt::from(1) << (-e) as u32)
            }
        }
        pub fn add(&self, o: &Q) -> Q {
            Q(&self.0 * &o.1 + &o.0 * &self.1, &self.1 * &o.1)
        }
        pub fn mul(&self, o: &Q) -> Q {
            Q(&self.0 * &o.0, &self.1 * &o.1)
        }
        pub fn sub(&self, o: &Q) -> Q {
            Q(&self.0 * &o.1 - &o.0 * &self.1, &self.1 * &o.1)
        }
        pub fn div(&self, o: &Q) -> Q {
            let (mut n, mut d) = (&self.0 * &o.1, &self.1 * &o.0);
            if d < BigInt::from(0) {
                n = -n;
                d = -d;
            }
            Q(n, d)
        }
        pub fn is_zero(&self) -> bool {
            self.0 == BigInt::from(0)
        }
        pub fn reduce(&mut self) {
            use num_bigint::Sign;
            let g = gcd(
                self.0.magnitude().clone().into(),
                self.1.magnitude().clone().into(),
            );
            if g > BigInt::from(1) {
                self.0 = &self.0 / &g;
                self.1 = &self.1 / &g;
            }
            if self.1.sign() == Sign::Minus {
                self.0 = -self.0.clone();
                self.1 = -self.1.clone();
            }
        }
    }

    fn gcd(mut a: BigInt, mut b: BigInt) -> BigInt {
        while b != BigInt::from(0) {
            let r = &a % &b;
            a = std::mem::replace(&mut b, r);
        }
        a
    }

    /// Plain rational GE with partial pivot on exact fractions.
    pub fn solve(mut m: Vec<Vec<Q>>) -> Option<Vec<Q>> {
        let n = m.len();
        for c in 0..n {
            let p = (c..n).find(|&r| !m[r][c].is_zero())?;
            m.swap(c, p);
            for r in 0..n {
                if r != c && !m[r][c].is_zero() {
                    let f = m[r][c].div(&m[c][c]);
                    for k in c..=n {
                        let t = m[r][k].sub(&f.mul(&m[c][k]));
                        m[r][k] = t;
                        m[r][k].reduce();
                    }
                }
            }
        }
        Some((0..n).map(|r| m[r][n].div(&m[r][r])).collect())
    }

    /// Correct rounding of a rational to f64 via successive approximation
    /// against exact comparison (independent of the library's rounder):
    /// binary-search the f64 neighbors around num/den.
    pub fn round(q: &Q) -> f64 {
        let (n, d) = (&q.0, &q.1);
        if *n == BigInt::from(0) {
            return 0.0;
        }
        // start from the f64 estimate, then check both neighbors exactly
        let est = big_to_f64_approx(n) / big_to_f64_approx(d);
        let mut best = est;
        let mut best_err: Option<(BigInt, BigInt)> = None;
        for cand in [
            est,
            f64::from_bits(est.to_bits().wrapping_add(1)),
            f64::from_bits(est.to_bits().wrapping_sub(1)),
            f64::from_bits(est.to_bits().wrapping_add(2)),
            f64::from_bits(est.to_bits().wrapping_sub(2)),
        ] {
            if !cand.is_finite() {
                continue;
            }
            let qc = Q::from_f64(cand);
            // |n/d - qc| = |n*qc.1 - qc.0*d| / (d*qc.1)
            let num = (n * &qc.1 - &qc.0 * d).magnitude().clone();
            let den = (d * &qc.1).magnitude().clone();
            let better = match &best_err {
                None => true,
                Some((bn, bd)) => {
                    BigInt::from(num.clone()) * bd
                        < BigInt::from(bn.magnitude().clone()) * BigInt::from(den.clone())
                }
            };
            if better {
                best = cand;
                best_err = Some((BigInt::from(num), BigInt::from(den)));
            }
        }
        best
    }

    fn big_to_f64_approx(b: &BigInt) -> f64 {
        use num_traits::ToPrimitive;
        b.to_f64().unwrap_or(f64::MAX)
    }
}

#[test]
fn regression_exact_agrees_with_independent_rational_oracle() {
    use oracle::Q;
    let mut seed = 4711u64;
    let mut rnd = move || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((seed >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
    };
    for case in 0..40 {
        let d = 1 + case % 3;
        let n_rows = 25 + case * 3;
        let rows: Vec<(Vec<f64>, f64)> = (0..n_rows)
            .map(|_| {
                let x: Vec<f64> = (0..d).map(|_| rnd() * 10.0).collect();
                let y = 1.0 + x.iter().sum::<f64>() * 0.5 + rnd() * 0.1;
                (x, y)
            })
            .collect();
        let c = cov_of(&borrow(&rows), d);
        let lib = match c.try_regression_exact() {
            Ok(v) => v,
            Err(_) => continue, // singular draw: oracle would also fail
        };
        // oracle: build exact normal equations from raw rows in rationals
        let m = d + 1;
        let mut a = vec![vec![Q(0.into(), 1.into()); m + 1]; m];
        for (x, y) in &rows {
            let av: Vec<Q> = std::iter::once(1.0)
                .chain(x.iter().copied())
                .map(Q::from_f64)
                .collect();
            let qy = Q::from_f64(*y);
            for i in 0..m {
                for j in 0..m {
                    let t = a[i][j].add(&av[i].mul(&av[j]));
                    a[i][j] = t;
                    a[i][j].reduce();
                }
                let t = a[i][m].add(&av[i].mul(&qy));
                a[i][m] = t;
                a[i][m].reduce();
            }
        }
        let sol = oracle::solve(a).expect("oracle solvable");
        for (i, (l, q)) in lib.iter().zip(&sol).enumerate() {
            let o = oracle::round(q);
            assert_eq!(
                l.to_bits(),
                o.to_bits(),
                "case {case} coeff {i}: lib {l} vs oracle {o}"
            );
        }
    }
}
