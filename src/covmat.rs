// Copyright (c) 2026 Kyle Clouthier / Clouthier Simulation Labs. Licensed under MIT OR Apache-2.0.
//! Multivariate convergent statistics: exact covariance matrices and
//! deterministic multiple linear regression.
//!
//! State: exact Σxᵢ for each of `d` features plus exact Σxᵢxⱼ for the upper
//! triangle — every entry order-invariant and mergeable, so the covariance
//! MATRIX is bit-identical on every replica. Reads come in two honestly
//! separated tiers:
//!
//! * **exactly rounded** — each covariance entry is a rational of the exact
//!   state, rounded once ([`CovMatrixF64::try_covariance`]);
//! * **deterministic** — multiple-regression coefficients solve the normal
//!   equations with a fixed-pivot Gaussian elimination over exactly-rounded
//!   inputs: bit-identical everywhere (identical input bits, fixed
//!   algorithm), but the *solve* itself is ordinary f64 arithmetic, so the
//!   coefficients are not exactly rounded. Stated, not hidden.

use crate::stats::{dot_int, round_rational, sum_int, unit_pow, StatsError, UNIT_LOG2};
use crate::{DotF64, SumF64};
use num_bigint::BigInt;

const DOT_BYTES: usize = SumF64::BYTES + 1;

/// Exact, mergeable d-dimensional second-moment state: covariance matrices
/// bit-identical across any sharding, and deterministic multiple regression.
///
/// Rows are added with [`add`](Self::add) (a `d`-length feature slice plus a
/// response `y` for regression; pass `y = 0.0` if unused). Merging requires
/// equal dimensions — a mismatch poisons the state and surfaces as an error.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CovMatrixF64 {
    d: usize,
    /// Σxᵢ, i in 0..d, then Σy at index d.
    sums: Vec<SumF64>,
    /// Upper triangle Σxᵢxⱼ (i ≤ j), then Σxᵢ·y per i, laid out row-major.
    prods: Vec<DotF64>,
    mismatched: bool,
}

impl CovMatrixF64 {
    /// A new state for `d` features (plus one response column for
    /// regression).
    pub fn new(d: usize) -> Self {
        let tri = d * (d + 1) / 2;
        Self {
            d,
            sums: vec![SumF64::new(); d + 1],
            prods: vec![DotF64::new(); tri + d],
            mismatched: false,
        }
    }

    /// Number of features.
    pub const fn dim(&self) -> usize {
        self.d
    }

    /// Number of rows added.
    pub fn count(&self) -> u64 {
        self.sums.last().map_or(0, SumF64::count)
    }

    fn tri_index(&self, i: usize, j: usize) -> usize {
        // upper triangle, i <= j, row-major; underflow-safe form of
        // i*d - i(i-1)/2 + (j-i)
        let (i, j) = if i <= j { (i, j) } else { (j, i) };
        (i * (2 * self.d - i + 1)) / 2 + (j - i)
    }

    /// Add one row of features and its response value.
    ///
    /// # Panics
    /// Panics if `x.len() != dim()`.
    pub fn add(&mut self, x: &[f64], y: f64) {
        assert_eq!(x.len(), self.d, "row length must equal dim()");
        for (s, &v) in self.sums.iter_mut().zip(x) {
            s.add(v);
        }
        if let Some(sy) = self.sums.last_mut() {
            sy.add(y);
        }
        let mut k = 0usize;
        for i in 0..self.d {
            for j in i..self.d {
                self.prods[k].push(x[i], x[j]);
                k += 1;
            }
        }
        for &xi in x.iter() {
            self.prods[k].push(xi, y);
            k += 1;
        }
    }

    /// Merge another state. Dimensions must match; a mismatch poisons the
    /// state (reported on read, never silent).
    pub fn merge(&mut self, o: &CovMatrixF64) {
        if self.d != o.d {
            self.mismatched = true;
            return;
        }
        self.mismatched |= o.mismatched;
        for (a, b) in self.sums.iter_mut().zip(&o.sums) {
            a.merge(b);
        }
        for (a, b) in self.prods.iter_mut().zip(&o.prods) {
            a.merge(b);
        }
    }

    fn check(&self) -> Result<u64, StatsError> {
        if self.mismatched {
            return Err(StatsError::Degenerate);
        }
        let n = self.count();
        if n == 0 {
            return Err(StatsError::Empty);
        }
        Ok(n)
    }

    /// Exact B-term for a feature pair: n·Σxᵢxⱼ·u − Σxᵢ·Σxⱼ.
    fn b_term(&self, i: usize, j: usize, n: u64) -> Result<BigInt, StatsError> {
        let si = sum_int(&self.sums[i])?;
        let sj = sum_int(&self.sums[j])?;
        let q = dot_int(&self.prods[self.tri_index(i, j)])?;
        Ok(((BigInt::from(n) * q) << UNIT_LOG2) - si * sj)
    }

    /// The exactly rounded population covariance between features `i` and
    /// `j` (i == j gives the variance of feature i).
    pub fn try_covariance(&self, i: usize, j: usize) -> Result<f64, StatsError> {
        let n = self.check()?;
        if i >= self.d || j >= self.d {
            return Err(StatsError::Degenerate);
        }
        let b = self.b_term(i, j, n)?;
        Ok(round_rational(
            &b,
            &(BigInt::from(n) * BigInt::from(n) * unit_pow(2)),
        ))
    }

    /// Multiple linear regression y ≈ β₀ + Σᵢ βᵢ·xᵢ: returns
    /// `[β₀, β₁, …, β_d]`.
    ///
    /// **Deterministic tier**: the normal-equation entries are exactly
    /// rounded from the exact state, then solved by fixed-pivot Gaussian
    /// elimination in f64 — bit-identical on every machine and sharding,
    /// but the solve step is not exactly rounded (stated limit). Returns
    /// [`StatsError::Degenerate`] if the system is singular.
    pub fn try_regression(&self) -> Result<Vec<f64>, StatsError> {
        let n = self.check()?;
        let d = self.d;
        // Build the (d+1) x (d+2) augmented normal-equation system over
        // exactly rounded entries: rows/cols ordered [1, x0..x_{d-1}], rhs y.
        let m = d + 1;
        let mut a = vec![vec![0.0f64; m + 1]; m];
        // exactly rounded means of features and products
        let mean = |s: &SumF64| -> Result<f64, StatsError> {
            let v = sum_int(s)?;
            Ok(round_rational(&v, &(BigInt::from(n) * unit_pow(1))))
        };
        a[0][0] = 1.0;
        for i in 0..d {
            let mi = mean(&self.sums[i])?;
            a[0][i + 1] = mi;
            a[i + 1][0] = mi;
        }
        a[0][m] = mean(&self.sums[d])?;
        let tri = d * (d + 1) / 2;
        for i in 0..d {
            for j in i..d {
                let q = dot_int(&self.prods[self.tri_index(i, j)])?;
                let v = round_rational(&q, &(BigInt::from(n) * unit_pow(1)));
                a[i + 1][j + 1] = v;
                a[j + 1][i + 1] = v;
            }
            let qy = dot_int(&self.prods[tri + i])?;
            a[i + 1][m] = round_rational(&qy, &(BigInt::from(n) * unit_pow(1)));
        }
        // fixed-pivot (partial, ties by lowest row index) Gaussian elimination:
        // deterministic given identical input bits.
        for col in 0..m {
            let mut piv = col;
            for (r, row) in a.iter().enumerate().skip(col) {
                if row[col].abs() > a[piv][col].abs() {
                    piv = r;
                }
            }
            if a[piv][col] == 0.0 {
                return Err(StatsError::Degenerate);
            }
            a.swap(col, piv);
            let p = a[col][col];
            for r in 0..m {
                if r == col {
                    continue;
                }
                let f = a[r][col] / p;
                if f == 0.0 {
                    continue;
                }
                let pivot_row = a[col].clone();
                for (c, pv) in pivot_row.iter().enumerate().skip(col) {
                    a[r][c] -= f * pv;
                }
            }
        }
        Ok((0..m).map(|r| a[r][m] / a[r][r]).collect())
    }

    /// Convenience: [`try_covariance`](Self::try_covariance), errors as NaN.
    pub fn covariance(&self, i: usize, j: usize) -> f64 {
        self.try_covariance(i, j).unwrap_or(f64::NAN)
    }

    /// Canonical byte encoding.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(self.d as u32).to_le_bytes());
        out.push(self.mismatched as u8);
        for s in &self.sums {
            out.extend_from_slice(&s.to_bytes());
        }
        for p in &self.prods {
            out.extend_from_slice(&p.to_bytes());
        }
        out
    }

    /// Decode a canonical encoding produced by [`encode`](Self::encode).
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 5 {
            return None;
        }
        let mut w = [0u8; 4];
        w.copy_from_slice(&bytes[..4]);
        let d = u32::from_le_bytes(w) as usize;
        let mismatched = match bytes[4] {
            0 => false,
            1 => true,
            _ => return None,
        };
        // fully checked size arithmetic: a hostile length prefix must be
        // rejected, never allowed to overflow (found by fuzzing).
        let tri = d.checked_mul(d.checked_add(1)?)? / 2;
        let n_sums = d.checked_add(1)?;
        let n_prods = tri.checked_add(d)?;
        let need = 5usize
            .checked_add(n_sums.checked_mul(SumF64::BYTES)?)?
            .checked_add(n_prods.checked_mul(DOT_BYTES)?)?;
        if bytes.len() != need {
            return None;
        }
        let mut at = 5;
        let mut sums = Vec::with_capacity(n_sums);
        for _ in 0..n_sums {
            let mut sb = [0u8; SumF64::BYTES];
            sb.copy_from_slice(&bytes[at..at + SumF64::BYTES]);
            sums.push(SumF64::from_bytes(&sb)?);
            at += SumF64::BYTES;
        }
        let mut prods = Vec::with_capacity(n_prods);
        for _ in 0..n_prods {
            let mut db = [0u8; DOT_BYTES];
            db.copy_from_slice(&bytes[at..at + DOT_BYTES]);
            prods.push(DotF64::from_bytes(&db)?);
            at += DOT_BYTES;
        }
        Some(Self {
            d,
            sums,
            prods,
            mismatched,
        })
    }
}

impl crate::Mergeable for CovMatrixF64 {
    fn merge(&mut self, other: &Self) {
        CovMatrixF64::merge(self, other);
    }
    fn count(&self) -> u64 {
        CovMatrixF64::count(self)
    }
    fn encode(&self) -> Vec<u8> {
        CovMatrixF64::encode(self)
    }
    fn decode(bytes: &[u8]) -> Option<Self> {
        CovMatrixF64::decode(bytes)
    }
}

// ---------------------------------------------------------------------------
// Exact-tier extension (branch: exact-tier): downdating + correctly-rounded
// regression. See probe474/probe475 (2026-07-17) for the validation trail.

/// Fraction-free Bareiss determinant of an integer matrix (exact; the
/// interior division is exact by the Bareiss identity). Partial row pivoting
/// on zero pivots only — the value is order-independent regardless.
fn bareiss_det(mut m: Vec<Vec<BigInt>>) -> BigInt {
    let n = m.len();
    let mut sign = 1i8;
    let mut prev = BigInt::from(1u8);
    for k in 0..n {
        if m[k][k].bits() == 0 {
            match (k + 1..n).find(|&r| m[r][k].bits() != 0) {
                Some(p) => {
                    m.swap(k, p);
                    sign = -sign;
                }
                None => return BigInt::from(0u8),
            }
        }
        for i in k + 1..n {
            for j in k + 1..n {
                let t = &m[i][j] * &m[k][k] - &m[i][k] * &m[k][j];
                m[i][j] = t / &prev;
            }
            m[i][k] = BigInt::from(0u8);
        }
        prev = m[k][k].clone();
    }
    let det = m[n - 1][n - 1].clone();
    if sign < 0 {
        -det
    } else {
        det
    }
}

impl CovMatrixF64 {
    /// Exactly remove a previously merged contribution: `self -= other`
    /// (downdating / unlearning). All-or-nothing: on any error the state is
    /// unchanged. Errors if dimensions mismatch, if `other` carries
    /// non-finite flags (sticky, non-cancellative), or if `other`'s counts
    /// exceed `self`'s. On success the state is byte-identical to one that
    /// never contained `other`'s rows — exact downdating at any removal
    /// fraction and any conditioning.
    pub fn try_sub(&mut self, o: &CovMatrixF64) -> Result<(), StatsError> {
        if self.d != o.d || self.mismatched || o.mismatched {
            return Err(StatsError::Degenerate);
        }
        let mut tmp = self.clone();
        for (a, b) in tmp.sums.iter_mut().zip(&o.sums) {
            if !a.try_unmerge(b) {
                return Err(StatsError::Degenerate);
            }
        }
        for (a, b) in tmp.prods.iter_mut().zip(&o.prods) {
            if !a.unmerge_assign(b) {
                return Err(StatsError::Degenerate);
            }
        }
        *self = tmp;
        Ok(())
    }

    /// **Exact tier**: correctly rounded multiple-regression coefficients.
    ///
    /// The normal-equation system is formed from the state's EXACT integers
    /// (no rounding), solved by Cramer's rule with fraction-free Bareiss
    /// determinants (exact), and each coefficient is rounded ONCE, correctly
    /// (round-to-nearest, ties-to-even). The returned bits are therefore a
    /// mathematical function of the data alone — identical on any machine,
    /// any implementation, by definition rather than by construction.
    /// Immune to conditioning: at condition numbers where QR and the
    /// deterministic tier fail outright, this returns the exact solution's
    /// correct rounding (probe474). Cost: O(d) exact determinants of
    /// (d+1)-square integer matrices — fine for regression-sized d.
    pub fn try_regression_exact(&self) -> Result<Vec<f64>, StatsError> {
        let n = self.check()?;
        let d = self.d;
        let m = d + 1;
        let u = unit_pow(1);
        let zero = BigInt::from(0u8);
        // Row-scaled integer augmented system (row scaling preserves the
        // solution). sum_int AND dot_int both return integers on the
        // 2^-1074 grid (TwoProduct keeps exact products on the f64 grid),
        // so every row becomes integral after scaling by u = 2^1074:
        //   row0:  [n*u, S_0 .. S_{d-1}        | Sy   ]
        //   rowi:  [S_i, Q_{i,0} .. Q_{i,d-1}  | Qy_i ]
        let mut a = vec![vec![zero.clone(); m]; m];
        let mut b = vec![zero; m];
        let mut s = Vec::with_capacity(d);
        for i in 0..d {
            s.push(sum_int(&self.sums[i])?);
        }
        a[0][0] = BigInt::from(n) * &u;
        for i in 0..d {
            a[0][i + 1] = s[i].clone();
            a[i + 1][0] = s[i].clone();
        }
        b[0] = sum_int(&self.sums[d])?;
        let tri = d * (d + 1) / 2;
        for i in 0..d {
            for j in 0..d {
                let (lo, hi) = if i <= j { (i, j) } else { (j, i) };
                a[i + 1][j + 1] = dot_int(&self.prods[self.tri_index(lo, hi)])?;
            }
            b[i + 1] = dot_int(&self.prods[tri + i])?;
        }
        let mut det = bareiss_det(a.clone());
        if det.bits() == 0 {
            return Err(StatsError::Degenerate);
        }
        let flip = det < BigInt::from(0u8);
        if flip {
            det = -det;
        }
        let mut out = Vec::with_capacity(m);
        for col in 0..m {
            let mut ai = a.clone();
            for r in 0..m {
                ai[r][col] = b[r].clone();
            }
            let mut di = bareiss_det(ai);
            if flip {
                di = -di;
            }
            out.push(round_rational(&di, &det));
        }
        Ok(out)
    }

    /// Convenience: [`try_regression_exact`](Self::try_regression_exact),
    /// errors as NaN.
    pub fn regression_exact(&self) -> Vec<f64> {
        self.try_regression_exact()
            .unwrap_or_else(|_| vec![f64::NAN; self.d + 1])
    }
}
