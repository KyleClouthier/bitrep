// Copyright (c) 2026 Kyle Clouthier. Licensed under MIT OR Apache-2.0.
//! Convergent statistics (feature `stats`): mergeable, order-invariant
//! moment states with **exactly rounded** reads.
//!
//! The construction extends the float-counter-CRDT idea from counters to a
//! statistics algebra. Each type's state is a set of *exact monomial sums*
//! held in [`SumF64`]/[`DotF64`] accumulators, so the state — and therefore
//! every derived statistic — is **bit-identical across any sharding, arrival
//! order, or merge tree**. Reads are computed from the exact integer state in
//! big-integer arithmetic with a **single** round-to-nearest-ties-to-even at
//! the end: the returned `f64` is the correctly rounded value of the true
//! statistic of the multiset.
//!
//! What this buys over the classical art:
//! * Chan/Golub/LeVeque parallel moments are *algebraically* exact but
//!   computed in floats — their bits depend on the merge tree, and their
//!   merge double-counts on re-delivery. These states are bit-invariant and,
//!   layered per-replica (see `examples/convergent_stats.rs`), CRDT-lawful.
//! * Catastrophic cancellation cannot corrupt reads: `variance()` of data
//!   with mean 10⁸ and spread 10⁻³ is exactly rounded, where the textbook
//!   formula returns garbage (even negative values).
//!
//! Reads that are a *rational* function of the state — mean, variance,
//! covariance, regression slope/intercept, R², kurtosis, skewness² — are
//! exactly rounded. Reads involving a square root (stddev, correlation,
//! skewness) apply IEEE `sqrt` (itself correctly rounded) to an exactly
//! rounded radicand: ≤ 2 roundings total, still bit-invariant everywhere.
//!
//! **Named limits.** Sums of squares/products route through FMA
//! two-products: a partial product that overflows or lands in the subnormal
//! range loses exactness — detected and reported ([`StatsError`]), never
//! silent. Third/fourth moments narrow the exact domain further (|x|³, |x|⁴
//! and their two-product tails must stay in range). NaN/±∞ inputs are
//! tracked as flags and surface as [`StatsError::NonFinite`].

use crate::{DotF64, SumF64};
use num_bigint::{BigInt, BigUint, Sign};
use num_traits::{Signed, Zero};

const LIMBS: usize = 34;
const STATE_BITS: usize = LIMBS * 64;
const DOT_BYTES: usize = SumF64::BYTES + 1;
/// The accumulator's fixed-point unit is 2⁻¹⁰⁷⁴ (one ULP of the smallest
/// subnormal); states are integers in this unit.
pub(crate) const UNIT_LOG2: usize = 1074;

/// Why a statistic could not be produced. Mirrors [`crate::DotError`]'s
/// philosophy: degraded meaning is reported, never silently returned.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StatsError {
    /// No samples (or not enough for the requested statistic).
    Empty,
    /// A NaN or infinity was added (or an intermediate product overflowed).
    NonFinite,
    /// A two-product underflowed to the subnormal range; the exactly-rounded
    /// contract cannot be certified (see [`crate::DotError::ExactnessLost`]).
    ExactnessLost,
    /// The statistic is undefined for this data (e.g. zero variance in a
    /// correlation or regression denominator).
    Degenerate,
}

impl core::fmt::Display for StatsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            StatsError::Empty => write!(f, "not enough samples"),
            StatsError::NonFinite => write!(f, "non-finite input or overflow"),
            StatsError::ExactnessLost => {
                write!(f, "two-product underflow: exact rounding not certified")
            }
            StatsError::Degenerate => write!(f, "statistic undefined for this data"),
        }
    }
}

impl std::error::Error for StatsError {}

// ---------------------------------------------------------------------------
// exact integer views of accumulator states
// ---------------------------------------------------------------------------

/// The flag byte of a canonical `SumF64` encoding.
fn sum_flags(bytes: &[u8; SumF64::BYTES]) -> u8 {
    bytes[LIMBS * 8]
}

/// Exact two's-complement integer of a `SumF64` state (units 2⁻¹⁰⁷⁴).
pub(crate) fn sum_int(s: &SumF64) -> Result<BigInt, StatsError> {
    let bytes = s.to_bytes();
    if sum_flags(&bytes) != 0 {
        return Err(StatsError::NonFinite);
    }
    Ok(twos_complement(&bytes[..LIMBS * 8]))
}

/// Exact integer of a `DotF64` state (units 2⁻¹⁰⁷⁴), checking exactness.
pub(crate) fn dot_int(d: &DotF64) -> Result<BigInt, StatsError> {
    if !d.is_exact() {
        return Err(StatsError::ExactnessLost);
    }
    let bytes = d.to_bytes();
    let mut inner = [0u8; SumF64::BYTES];
    inner.copy_from_slice(&bytes[..SumF64::BYTES]);
    if sum_flags(&inner) != 0 {
        return Err(StatsError::NonFinite);
    }
    Ok(twos_complement(&inner[..LIMBS * 8]))
}

fn twos_complement(le: &[u8]) -> BigInt {
    let mag = BigUint::from_bytes_le(le);
    let half = BigUint::from(1u8) << (STATE_BITS - 1);
    if mag >= half {
        BigInt::from_biguint(Sign::Minus, (BigUint::from(1u8) << STATE_BITS) - mag)
    } else {
        BigInt::from_biguint(Sign::Plus, mag)
    }
}

// ---------------------------------------------------------------------------
// exact rational -> correctly rounded f64 (full range: subnormals, overflow)
// ---------------------------------------------------------------------------

/// Round the exact rational `p/q` (any sign in `p`; `q > 0`) to the nearest
/// f64, ties to even, over the FULL IEEE range: results round into the
/// subnormal grid below 2⁻¹⁰²², and to ±∞ beyond the finite range.
pub(crate) fn round_rational(p: &BigInt, q: &BigInt) -> f64 {
    debug_assert!(q.is_positive());
    if p.is_zero() {
        return 0.0; // canonical +0.0, matching SumF64's zero policy
    }
    let neg = p.is_negative() != q.is_negative();
    let p = p.magnitude().clone();
    let q = q.magnitude().clone();

    // e = floor(log2(p/q)): 2^e <= p/q < 2^(e+1)
    let mut e = p.bits() as i64 - q.bits() as i64;
    let ge = if e >= 0 {
        p >= (&q << e as usize)
    } else {
        (&p << (-e) as usize) >= q
    };
    if !ge {
        e -= 1;
    }
    // rounding grid: 2^g, clamped to the subnormal grid
    let g = core::cmp::max(e - 52, -(UNIT_LOG2 as i64));
    let (num, den) = if g >= 0 {
        (p, q << g as usize)
    } else {
        (p << (-g) as usize, q)
    };
    let mut m = &num / &den;
    let r = &num - &m * &den;
    let two_r = &r << 1usize;
    let odd = (&m & BigUint::from(1u8)) == BigUint::from(1u8);
    if two_r > den || (two_r == den && odd) {
        m += 1u8;
    }
    let mut g = g;
    if m.bits() > 53 {
        // rounded up to 2^53: renormalize
        m >>= 1usize;
        g += 1;
    }
    let m64 = m.iter_u64_digits().next().unwrap_or(0);
    if m64 == 0 {
        return if neg { -0.0 } else { 0.0 };
    }
    let bits = if m64 >= (1u64 << 52) {
        // normal (g = e-52 kept, or promoted): value = m * 2^g, 2^52 <= m < 2^53
        let e_unb = g + 52;
        if e_unb > 1023 {
            f64::INFINITY.to_bits()
        } else {
            (((e_unb + 1023) as u64) << 52) | (m64 & ((1u64 << 52) - 1))
        }
    } else {
        // subnormal: only reachable on the clamped grid g = -1074
        m64
    };
    f64::from_bits(bits | ((neg as u64) << 63))
}

pub(crate) fn unit_pow(k: usize) -> BigInt {
    BigInt::from(1u8) << (UNIT_LOG2 * k)
}

// ---------------------------------------------------------------------------
// MomentsF64 — mean / variance / stddev
// ---------------------------------------------------------------------------

/// Exact, mergeable first and second moments: `mean`, `variance`, `stddev`.
///
/// State: exact Σx ([`SumF64`]) and exact Σx² ([`DotF64`]); the sample count
/// rides in the sum's counter. Merging shards — in any order, any tree —
/// yields the same bytes and therefore the same reads.
///
/// ```
/// use bitrep::MomentsF64;
///
/// let mut a = MomentsF64::new();
/// let mut b = MomentsF64::new();
/// for x in [1.5, 2.5, 3.5] { a.add(x); }
/// for x in [4.5, 5.5]      { b.add(x); }
/// let mut whole = MomentsF64::new();
/// for x in [4.5, 1.5, 5.5, 3.5, 2.5] { whole.add(x); } // any order
/// a.merge(&b);
/// assert_eq!(a.to_bytes().to_vec(), whole.to_bytes().to_vec());
/// assert_eq!(a.mean(), 3.5);
/// ```
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct MomentsF64 {
    sum: SumF64,
    sumsq: DotF64,
}

impl MomentsF64 {
    /// Serialized size in bytes.
    pub const BYTES: usize = SumF64::BYTES + DOT_BYTES;

    /// An empty state.
    pub fn new() -> Self {
        Self {
            sum: SumF64::new(),
            sumsq: DotF64::new(),
        }
    }

    /// Add one sample. Order never matters.
    pub fn add(&mut self, x: f64) {
        self.sum.add(x);
        self.sumsq.push(x, x);
    }

    /// Merge another state (shard combining). Associative and commutative.
    pub fn merge(&mut self, o: &MomentsF64) {
        self.sum.merge(&o.sum);
        self.sumsq.merge(&o.sumsq);
    }

    /// Number of samples.
    pub const fn count(&self) -> u64 {
        self.sum.count()
    }

    /// The exactly rounded mean.
    pub fn try_mean(&self) -> Result<f64, StatsError> {
        let n = self.count();
        if n == 0 {
            return Err(StatsError::Empty);
        }
        let s = sum_int(&self.sum)?;
        Ok(round_rational(&s, &(BigInt::from(n) * unit_pow(1))))
    }

    /// `A2 = n·Q·2^1074 − S²` and `n` — the exact variance numerator (in
    /// units 2⁻²¹⁴⁸ over n²) shared by variance flavors.
    fn a2(&self) -> Result<(BigInt, u64), StatsError> {
        let n = self.count();
        if n == 0 {
            return Err(StatsError::Empty);
        }
        let s = sum_int(&self.sum)?;
        let q = dot_int(&self.sumsq)?;
        let a2 = ((BigInt::from(n) * q) << UNIT_LOG2) - (&s * &s);
        Ok((a2, n))
    }

    /// The exactly rounded population variance (denominator `n`).
    pub fn try_variance(&self) -> Result<f64, StatsError> {
        let (a2, n) = self.a2()?;
        Ok(round_rational(
            &a2,
            &(BigInt::from(n) * BigInt::from(n) * unit_pow(2)),
        ))
    }

    /// The exactly rounded sample variance (denominator `n − 1`).
    pub fn try_sample_variance(&self) -> Result<f64, StatsError> {
        let (a2, n) = self.a2()?;
        if n < 2 {
            return Err(StatsError::Empty);
        }
        Ok(round_rational(
            &a2,
            &(BigInt::from(n) * BigInt::from(n - 1) * unit_pow(2)),
        ))
    }

    /// Population standard deviation: IEEE `sqrt` (correctly rounded) of the
    /// exactly rounded variance — ≤ 2 roundings, bit-invariant everywhere.
    pub fn try_stddev(&self) -> Result<f64, StatsError> {
        Ok(self.try_variance()?.sqrt())
    }

    /// Convenience: [`try_mean`](Self::try_mean), with errors as NaN.
    pub fn mean(&self) -> f64 {
        self.try_mean().unwrap_or(f64::NAN)
    }

    /// Convenience: [`try_variance`](Self::try_variance), with errors as NaN.
    pub fn variance(&self) -> f64 {
        self.try_variance().unwrap_or(f64::NAN)
    }

    /// Convenience: [`try_stddev`](Self::try_stddev), with errors as NaN.
    pub fn stddev(&self) -> f64 {
        self.try_stddev().unwrap_or(f64::NAN)
    }

    /// Canonical byte encoding (sum bytes, then Σx² bytes). Equal multisets
    /// yield equal bytes: hash or sign this.
    pub fn to_bytes(&self) -> [u8; Self::BYTES] {
        let mut out = [0u8; Self::BYTES];
        out[..SumF64::BYTES].copy_from_slice(&self.sum.to_bytes());
        out[SumF64::BYTES..].copy_from_slice(&self.sumsq.to_bytes());
        out
    }

    /// Decode a state produced by [`to_bytes`](Self::to_bytes).
    pub fn from_bytes(b: &[u8; Self::BYTES]) -> Option<Self> {
        let mut sb = [0u8; SumF64::BYTES];
        sb.copy_from_slice(&b[..SumF64::BYTES]);
        let mut db = [0u8; DOT_BYTES];
        db.copy_from_slice(&b[SumF64::BYTES..]);
        Some(Self {
            sum: SumF64::from_bytes(&sb)?,
            sumsq: DotF64::from_bytes(&db)?,
        })
    }
}

// ---------------------------------------------------------------------------
// Moments4F64 — through the 4th moment: skewness and kurtosis
// ---------------------------------------------------------------------------

/// Exact, mergeable moments through the 4th: adds `skewness` and `kurtosis`.
///
/// Σx³ and Σx⁴ are accumulated exactly by two-product decomposition:
/// x² = h + l exactly (FMA), then Σx³ = Σ(h·x) + Σ(l·x) and
/// Σx⁴ = Σ(h·h) + 2Σ(h·l) + Σ(l·l), every product routed through [`DotF64`].
///
/// Kurtosis is a *rational* function of the exact state (μ₄/μ₂² — the n and
/// unit factors cancel), so it is **exactly rounded**; likewise
/// [`try_skewness_squared`](Self::try_skewness_squared) (μ₃²/μ₂³). `skewness` itself
/// carries one extra IEEE-`sqrt` rounding and is bit-invariant.
///
/// **Named limit:** the exact domain narrows to inputs whose x³/x⁴
/// two-product tails stay clear of the subnormal range (roughly
/// |x| ∈ [1e-77, 1e77] — flagged via [`StatsError::ExactnessLost`], never
/// silent).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Moments4F64 {
    sum: SumF64,
    m2: DotF64,
    m3: DotF64,
    m4: DotF64,
    /// x² two-product underflowed: h/l split not certified.
    split_underflow: bool,
}

impl Moments4F64 {
    /// Serialized size in bytes.
    pub const BYTES: usize = SumF64::BYTES + 3 * DOT_BYTES + 1;

    /// An empty state.
    pub fn new() -> Self {
        Self {
            sum: SumF64::new(),
            m2: DotF64::new(),
            m3: DotF64::new(),
            m4: DotF64::new(),
            split_underflow: false,
        }
    }

    /// Add one sample. Order never matters.
    pub fn add(&mut self, x: f64) {
        self.sum.add(x);
        self.m2.push(x, x);
        // exact split x^2 = h + l (Dekker/Ogita–Rump two-product via FMA)
        let h = x * x;
        let l = x.mul_add(x, -h);
        if x != 0.0 && h.is_finite() && h.abs() < f64::MIN_POSITIVE {
            self.split_underflow = true;
        }
        // Σx³ = Σ h·x + Σ l·x   (exact: each term two-product exact)
        self.m3.push(h, x);
        self.m3.push(l, x);
        // Σx⁴ = (h+l)² = h² + 2hl + l²
        self.m4.push(h, h);
        self.m4.push(h, l);
        self.m4.push(l, h);
        self.m4.push(l, l);
    }

    /// Merge another state. Associative and commutative.
    pub fn merge(&mut self, o: &Moments4F64) {
        self.sum.merge(&o.sum);
        self.m2.merge(&o.m2);
        self.m3.merge(&o.m3);
        self.m4.merge(&o.m4);
        self.split_underflow |= o.split_underflow;
    }

    /// Number of samples.
    pub const fn count(&self) -> u64 {
        self.sum.count()
    }

    /// Exact central-moment numerators A2, A3, A4 (common-denominator form).
    fn a234(&self) -> Result<(BigInt, BigInt, BigInt, u64), StatsError> {
        let n = self.count();
        if n == 0 {
            return Err(StatsError::Empty);
        }
        if self.split_underflow {
            return Err(StatsError::ExactnessLost);
        }
        let s = sum_int(&self.sum)?;
        let q2 = dot_int(&self.m2)?;
        let q3 = dot_int(&self.m3)?;
        let q4 = dot_int(&self.m4)?;
        let n_b = BigInt::from(n);
        // A2 = n·Q2·u − S²                                  (× u² / n²  = μ2)
        let a2 = ((&n_b * &q2) << UNIT_LOG2) - (&s * &s);
        // A3 = n²·Q3·u² − 3n·Q2·S·u + 2S³                    (× u³ / n³  = μ3)
        let a3 = ((&n_b * &n_b * &q3) << (2 * UNIT_LOG2))
            - ((BigInt::from(3u8) * &n_b * &q2 * &s) << UNIT_LOG2)
            + BigInt::from(2u8) * &s * &s * &s;
        // A4 = n³·Q4·u³ − 4n²·Q3·S·u² + 6n·Q2·S²·u − 3S⁴     (× u⁴ / n⁴  = μ4)
        let a4 = ((&n_b * &n_b * &n_b * &q4) << (3 * UNIT_LOG2))
            - ((BigInt::from(4u8) * &n_b * &n_b * &q3 * &s) << (2 * UNIT_LOG2))
            + ((BigInt::from(6u8) * &n_b * &q2 * &s * &s) << UNIT_LOG2)
            - BigInt::from(3u8) * &s * &s * &s * &s;
        Ok((a2, a3, a4, n))
    }

    /// The exactly rounded mean.
    pub fn try_mean(&self) -> Result<f64, StatsError> {
        let n = self.count();
        if n == 0 {
            return Err(StatsError::Empty);
        }
        let s = sum_int(&self.sum)?;
        Ok(round_rational(&s, &(BigInt::from(n) * unit_pow(1))))
    }

    /// The exactly rounded population variance.
    pub fn try_variance(&self) -> Result<f64, StatsError> {
        let (a2, _, _, n) = self.a234()?;
        Ok(round_rational(
            &a2,
            &(BigInt::from(n) * BigInt::from(n) * unit_pow(2)),
        ))
    }

    /// The exactly rounded **kurtosis** (μ₄/μ₂², Pearson; normal = 3).
    /// The n and unit factors cancel: kurtosis = A4/A2², a pure rational of
    /// the exact state — one rounding.
    pub fn try_kurtosis(&self) -> Result<f64, StatsError> {
        let (a2, _, a4, _) = self.a234()?;
        if a2.is_zero() {
            return Err(StatsError::Degenerate);
        }
        Ok(round_rational(&a4, &(&a2 * &a2)))
    }

    /// The exactly rounded **excess kurtosis**: (A4 − 3A2²)/A2².
    pub fn try_excess_kurtosis(&self) -> Result<f64, StatsError> {
        let (a2, _, a4, _) = self.a234()?;
        if a2.is_zero() {
            return Err(StatsError::Degenerate);
        }
        let a2sq = &a2 * &a2;
        Ok(round_rational(&(a4 - BigInt::from(3u8) * &a2sq), &a2sq))
    }

    /// The exactly rounded **squared skewness** (μ₃²/μ₂³ = A3²/A2³), with
    /// the sign of the skewness attached to the result.
    pub fn try_skewness_squared(&self) -> Result<f64, StatsError> {
        let (a2, a3, _, _) = self.a234()?;
        if a2.is_zero() {
            return Err(StatsError::Degenerate);
        }
        let v = round_rational(&(&a3 * &a3), &(&a2 * &a2 * &a2));
        Ok(if a3.is_negative() { -v } else { v })
    }

    /// Skewness: sign·√(A3²/A2³). One extra IEEE-`sqrt` rounding on an
    /// exactly rounded radicand; bit-invariant everywhere.
    pub fn try_skewness(&self) -> Result<f64, StatsError> {
        let s2 = self.try_skewness_squared()?;
        Ok(if s2 < 0.0 { -(-s2).sqrt() } else { s2.sqrt() })
    }

    /// Convenience wrappers (errors as NaN).
    pub fn kurtosis(&self) -> f64 {
        self.try_kurtosis().unwrap_or(f64::NAN)
    }

    /// Convenience: [`try_skewness`](Self::try_skewness), errors as NaN.
    pub fn skewness(&self) -> f64 {
        self.try_skewness().unwrap_or(f64::NAN)
    }

    /// Canonical byte encoding. Equal multisets yield equal bytes.
    pub fn to_bytes(&self) -> [u8; Self::BYTES] {
        let mut out = [0u8; Self::BYTES];
        let mut at = 0;
        out[at..at + SumF64::BYTES].copy_from_slice(&self.sum.to_bytes());
        at += SumF64::BYTES;
        for d in [&self.m2, &self.m3, &self.m4] {
            out[at..at + DOT_BYTES].copy_from_slice(&d.to_bytes());
            at += DOT_BYTES;
        }
        out[at] = self.split_underflow as u8;
        out
    }

    /// Decode a state produced by [`to_bytes`](Self::to_bytes).
    pub fn from_bytes(b: &[u8; Self::BYTES]) -> Option<Self> {
        let mut sb = [0u8; SumF64::BYTES];
        sb.copy_from_slice(&b[..SumF64::BYTES]);
        let sum = SumF64::from_bytes(&sb)?;
        let mut dots = [DotF64::new(), DotF64::new(), DotF64::new()];
        let mut at = SumF64::BYTES;
        for d in dots.iter_mut() {
            let mut db = [0u8; DOT_BYTES];
            db.copy_from_slice(&b[at..at + DOT_BYTES]);
            *d = DotF64::from_bytes(&db)?;
            at += DOT_BYTES;
        }
        let split_underflow = match b[at] {
            0 => false,
            1 => true,
            _ => return None,
        };
        let [m2, m3, m4] = dots;
        Some(Self {
            sum,
            m2,
            m3,
            m4,
            split_underflow,
        })
    }
}

// ---------------------------------------------------------------------------
// CovF64 — covariance, correlation, simple linear regression
// ---------------------------------------------------------------------------

/// Exact, mergeable bivariate state: covariance, correlation, and simple
/// least-squares regression `y ≈ intercept + slope·x` — with exactly rounded
/// `covariance`, `slope`, `intercept` and `r_squared`.
///
/// State: exact Σx, Σy, Σx², Σy², Σxy. Everything derived is a rational
/// function of the exact state except `correlation` (one extra IEEE-`sqrt`
/// rounding; bit-invariant).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct CovF64 {
    sx: SumF64,
    sy: SumF64,
    sxx: DotF64,
    syy: DotF64,
    sxy: DotF64,
}

impl CovF64 {
    /// Serialized size in bytes.
    pub const BYTES: usize = 2 * SumF64::BYTES + 3 * DOT_BYTES;

    /// An empty state.
    pub fn new() -> Self {
        Self {
            sx: SumF64::new(),
            sy: SumF64::new(),
            sxx: DotF64::new(),
            syy: DotF64::new(),
            sxy: DotF64::new(),
        }
    }

    /// Add one (x, y) pair. Order never matters.
    pub fn add(&mut self, x: f64, y: f64) {
        self.sx.add(x);
        self.sy.add(y);
        self.sxx.push(x, x);
        self.syy.push(y, y);
        self.sxy.push(x, y);
    }

    /// Merge another state. Associative and commutative.
    pub fn merge(&mut self, o: &CovF64) {
        self.sx.merge(&o.sx);
        self.sy.merge(&o.sy);
        self.sxx.merge(&o.sxx);
        self.syy.merge(&o.syy);
        self.sxy.merge(&o.sxy);
    }

    /// Number of pairs.
    pub const fn count(&self) -> u64 {
        self.sx.count()
    }

    /// Exact `B` terms: Bxy = n·Sxy·u − Sx·Sy (and xx, yy analogues).
    fn b_terms(&self) -> Result<(BigInt, BigInt, BigInt, u64), StatsError> {
        let n = self.count();
        if n == 0 {
            return Err(StatsError::Empty);
        }
        let sx = sum_int(&self.sx)?;
        let sy = sum_int(&self.sy)?;
        let qxx = dot_int(&self.sxx)?;
        let qyy = dot_int(&self.syy)?;
        let qxy = dot_int(&self.sxy)?;
        let n_b = BigInt::from(n);
        let bxy = ((&n_b * qxy) << UNIT_LOG2) - (&sx * &sy);
        let bxx = ((&n_b * qxx) << UNIT_LOG2) - (&sx * &sx);
        let byy = ((&n_b * qyy) << UNIT_LOG2) - (&sy * &sy);
        Ok((bxy, bxx, byy, n))
    }

    /// The exactly rounded population covariance.
    pub fn try_covariance(&self) -> Result<f64, StatsError> {
        let (bxy, _, _, n) = self.b_terms()?;
        Ok(round_rational(
            &bxy,
            &(BigInt::from(n) * BigInt::from(n) * unit_pow(2)),
        ))
    }

    /// The exactly rounded least-squares slope: Bxy / Bxx.
    pub fn try_slope(&self) -> Result<f64, StatsError> {
        let (bxy, bxx, _, _) = self.b_terms()?;
        if bxx.is_zero() {
            return Err(StatsError::Degenerate);
        }
        Ok(round_rational(&bxy, &bxx))
    }

    /// The exactly rounded least-squares intercept:
    /// (Sy·Bxx − Bxy·Sx) / (n·Bxx·u) — a single rounding, not `mean − slope·mean`.
    pub fn try_intercept(&self) -> Result<f64, StatsError> {
        let n = self.count();
        let (bxy, bxx, _, _) = self.b_terms()?;
        if bxx.is_zero() {
            return Err(StatsError::Degenerate);
        }
        let sx = sum_int(&self.sx)?;
        let sy = sum_int(&self.sy)?;
        let num = sy * &bxx - bxy * sx;
        let mut den = (BigInt::from(n) * bxx) << UNIT_LOG2;
        // keep the denominator positive for round_rational's contract
        let num = if den.is_negative() {
            den = -den;
            -num
        } else {
            num
        };
        Ok(round_rational(&num, &den))
    }

    /// The exactly rounded coefficient of determination R² = Bxy²/(Bxx·Byy).
    pub fn try_r_squared(&self) -> Result<f64, StatsError> {
        let (bxy, bxx, byy, _) = self.b_terms()?;
        let den = &bxx * &byy;
        if den.is_zero() {
            return Err(StatsError::Degenerate);
        }
        Ok(round_rational(&(&bxy * &bxy), &den))
    }

    /// Pearson correlation: sign(Bxy)·√R². One extra IEEE-`sqrt` rounding on
    /// an exactly rounded radicand; bit-invariant everywhere.
    pub fn try_correlation(&self) -> Result<f64, StatsError> {
        let (bxy, ..) = self.b_terms()?;
        let r2 = self.try_r_squared()?;
        let r = r2.sqrt();
        Ok(if bxy.is_negative() { -r } else { r })
    }

    /// Convenience: [`try_slope`](Self::try_slope), errors as NaN.
    pub fn slope(&self) -> f64 {
        self.try_slope().unwrap_or(f64::NAN)
    }

    /// Convenience: [`try_intercept`](Self::try_intercept), errors as NaN.
    pub fn intercept(&self) -> f64 {
        self.try_intercept().unwrap_or(f64::NAN)
    }

    /// Convenience: [`try_covariance`](Self::try_covariance), errors as NaN.
    pub fn covariance(&self) -> f64 {
        self.try_covariance().unwrap_or(f64::NAN)
    }

    /// Convenience: [`try_correlation`](Self::try_correlation), errors as NaN.
    pub fn correlation(&self) -> f64 {
        self.try_correlation().unwrap_or(f64::NAN)
    }

    /// Canonical byte encoding. Equal multisets of pairs yield equal bytes.
    pub fn to_bytes(&self) -> [u8; Self::BYTES] {
        let mut out = [0u8; Self::BYTES];
        let mut at = 0;
        for s in [&self.sx, &self.sy] {
            out[at..at + SumF64::BYTES].copy_from_slice(&s.to_bytes());
            at += SumF64::BYTES;
        }
        for d in [&self.sxx, &self.syy, &self.sxy] {
            out[at..at + DOT_BYTES].copy_from_slice(&d.to_bytes());
            at += DOT_BYTES;
        }
        out
    }

    /// Decode a state produced by [`to_bytes`](Self::to_bytes).
    pub fn from_bytes(b: &[u8; Self::BYTES]) -> Option<Self> {
        let mut at = 0;
        let mut sums = [SumF64::new(), SumF64::new()];
        for s in sums.iter_mut() {
            let mut sb = [0u8; SumF64::BYTES];
            sb.copy_from_slice(&b[at..at + SumF64::BYTES]);
            *s = SumF64::from_bytes(&sb)?;
            at += SumF64::BYTES;
        }
        let mut dots = [DotF64::new(), DotF64::new(), DotF64::new()];
        for d in dots.iter_mut() {
            let mut db = [0u8; DOT_BYTES];
            db.copy_from_slice(&b[at..at + DOT_BYTES]);
            *d = DotF64::from_bytes(&db)?;
            at += DOT_BYTES;
        }
        let [sx, sy] = sums;
        let [sxx, syy, sxy] = dots;
        Some(Self {
            sx,
            sy,
            sxx,
            syy,
            sxy,
        })
    }
}

// ---------------------------------------------------------------------------
// WeightedMomentsF64 — weighted mean / variance
// ---------------------------------------------------------------------------

/// Exact, mergeable **weighted** moments: weighted mean and variance with
/// exactly rounded reads.
///
/// State: exact Σw ([`SumF64`]), Σw·x and Σw·x² (via [`DotF64`] two-products;
/// w·x² routes through the exact split x² = h + l). Weights are expected to
/// be non-negative and finite; a non-positive total weight or non-finite
/// input surfaces as an error, never a silent value.
///
/// Because a sample's weight travels *with the sample* (not with arrival
/// order), timestamp-derived weights give order-invariant time-weighted
/// statistics.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct WeightedMomentsF64 {
    sw: SumF64,
    swx: DotF64,
    swx2: DotF64,
    /// x² two-product underflowed: h/l split not certified.
    split_underflow: bool,
}

impl WeightedMomentsF64 {
    /// Serialized size in bytes.
    pub const BYTES: usize = SumF64::BYTES + 2 * DOT_BYTES + 1;

    /// An empty state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add one sample with weight `w`. Order never matters.
    pub fn add(&mut self, x: f64, w: f64) {
        self.sw.add(w);
        self.swx.push(w, x);
        // exact split x² = h + l, then Σw·x² = Σw·h + Σw·l
        let h = x * x;
        let l = x.mul_add(x, -h);
        if x != 0.0 && h.is_finite() && h.abs() < f64::MIN_POSITIVE {
            self.split_underflow = true;
        }
        self.swx2.push(w, h);
        self.swx2.push(w, l);
    }

    /// Merge another state. Associative and commutative.
    pub fn merge(&mut self, o: &WeightedMomentsF64) {
        self.sw.merge(&o.sw);
        self.swx.merge(&o.swx);
        self.swx2.merge(&o.swx2);
        self.split_underflow |= o.split_underflow;
    }

    /// Number of samples.
    pub const fn count(&self) -> u64 {
        self.sw.count()
    }

    fn ints(&self) -> Result<(BigInt, BigInt, BigInt), StatsError> {
        if self.count() == 0 {
            return Err(StatsError::Empty);
        }
        if self.split_underflow {
            return Err(StatsError::ExactnessLost);
        }
        Ok((
            sum_int(&self.sw)?,
            dot_int(&self.swx)?,
            dot_int(&self.swx2)?,
        ))
    }

    /// The exactly rounded weighted mean: Σwx / Σw.
    pub fn try_mean(&self) -> Result<f64, StatsError> {
        let (sw, swx, _) = self.ints()?;
        if !sw.is_positive() {
            return Err(StatsError::Degenerate);
        }
        Ok(round_rational(&swx, &sw))
    }

    /// The exactly rounded weighted population variance:
    /// (Σw·Σwx² − (Σwx)²) / (Σw)².
    pub fn try_variance(&self) -> Result<f64, StatsError> {
        let (sw, swx, swx2) = self.ints()?;
        if !sw.is_positive() {
            return Err(StatsError::Degenerate);
        }
        // sw itself carries one unit power (unlike an integer n), so the
        // expression is homogeneous: no unit shift.
        let num = (&sw * swx2) - (&swx * &swx);
        let den = &sw * &sw;
        Ok(round_rational(&num, &den))
    }

    /// Convenience: [`try_mean`](Self::try_mean), errors as NaN.
    pub fn mean(&self) -> f64 {
        self.try_mean().unwrap_or(f64::NAN)
    }

    /// Convenience: [`try_variance`](Self::try_variance), errors as NaN.
    pub fn variance(&self) -> f64 {
        self.try_variance().unwrap_or(f64::NAN)
    }

    /// Canonical byte encoding.
    pub fn to_bytes(&self) -> [u8; Self::BYTES] {
        let mut out = [0u8; Self::BYTES];
        out[..SumF64::BYTES].copy_from_slice(&self.sw.to_bytes());
        let mut at = SumF64::BYTES;
        for d in [&self.swx, &self.swx2] {
            out[at..at + DOT_BYTES].copy_from_slice(&d.to_bytes());
            at += DOT_BYTES;
        }
        out[at] = self.split_underflow as u8;
        out
    }

    /// Decode a state produced by [`to_bytes`](Self::to_bytes).
    pub fn from_bytes(b: &[u8; Self::BYTES]) -> Option<Self> {
        let mut sb = [0u8; SumF64::BYTES];
        sb.copy_from_slice(&b[..SumF64::BYTES]);
        let sw = SumF64::from_bytes(&sb)?;
        let mut at = SumF64::BYTES;
        let mut dots = [DotF64::new(), DotF64::new()];
        for d in dots.iter_mut() {
            let mut db = [0u8; DOT_BYTES];
            db.copy_from_slice(&b[at..at + DOT_BYTES]);
            *d = DotF64::from_bytes(&db)?;
            at += DOT_BYTES;
        }
        let split_underflow = match b[at] {
            0 => false,
            1 => true,
            _ => return None,
        };
        let [swx, swx2] = dots;
        Some(Self {
            sw,
            swx,
            swx2,
            split_underflow,
        })
    }
}

// ---------------------------------------------------------------------------
// PnMomentsF64 — moments with exact retraction (adds + removes)
// ---------------------------------------------------------------------------

/// Moments with **exact retraction**: `add(x)` and `remove(x)`, PN-counter
/// style (two grow-only states; the reads are computed on their exact
/// difference). Inserting then removing a sample returns the *derived
/// statistics* to byte-identical values — the primitive incremental view
/// maintenance needs.
///
/// Contract (same as every PN construction): only remove samples that were
/// added. Removing more than was added surfaces as
/// [`StatsError::Degenerate`], never a fabricated value.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PnMomentsF64 {
    adds: MomentsF64,
    removes: MomentsF64,
}

impl PnMomentsF64 {
    /// Serialized size in bytes.
    pub const BYTES: usize = 2 * MomentsF64::BYTES;

    /// An empty state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add one sample.
    pub fn add(&mut self, x: f64) {
        self.adds.add(x);
    }

    /// Exactly retract one previously added sample.
    pub fn remove(&mut self, x: f64) {
        self.removes.add(x);
    }

    /// Merge another state. Associative and commutative.
    pub fn merge(&mut self, o: &PnMomentsF64) {
        self.adds.merge(&o.adds);
        self.removes.merge(&o.removes);
    }

    /// Live sample count (adds − removes), or `None` if more removed than
    /// added.
    pub fn live_count(&self) -> Option<u64> {
        self.adds.count().checked_sub(self.removes.count())
    }

    /// Total operations (adds + removes; monotone — drives count-wins joins).
    pub const fn count(&self) -> u64 {
        self.adds.count().saturating_add(self.removes.count())
    }

    fn net(&self) -> Result<(BigInt, BigInt, u64), StatsError> {
        let n = self.live_count().ok_or(StatsError::Degenerate)?;
        if n == 0 {
            return Err(StatsError::Empty);
        }
        let s = sum_int(&self.adds.sum)? - sum_int(&self.removes.sum)?;
        let q = dot_int(&self.adds.sumsq)? - dot_int(&self.removes.sumsq)?;
        Ok((s, q, n))
    }

    /// The exactly rounded mean of the live multiset.
    pub fn try_mean(&self) -> Result<f64, StatsError> {
        let (s, _, n) = self.net()?;
        Ok(round_rational(&s, &(BigInt::from(n) * unit_pow(1))))
    }

    /// The exactly rounded population variance of the live multiset.
    pub fn try_variance(&self) -> Result<f64, StatsError> {
        let (s, q, n) = self.net()?;
        let a2 = ((BigInt::from(n) * q) << UNIT_LOG2) - (&s * &s);
        Ok(round_rational(
            &a2,
            &(BigInt::from(n) * BigInt::from(n) * unit_pow(2)),
        ))
    }

    /// Convenience: [`try_mean`](Self::try_mean), errors as NaN.
    pub fn mean(&self) -> f64 {
        self.try_mean().unwrap_or(f64::NAN)
    }

    /// Convenience: [`try_variance`](Self::try_variance), errors as NaN.
    pub fn variance(&self) -> f64 {
        self.try_variance().unwrap_or(f64::NAN)
    }

    /// Canonical byte encoding (adds bytes, then removes bytes).
    pub fn to_bytes(&self) -> [u8; Self::BYTES] {
        let mut out = [0u8; Self::BYTES];
        out[..MomentsF64::BYTES].copy_from_slice(&self.adds.to_bytes());
        out[MomentsF64::BYTES..].copy_from_slice(&self.removes.to_bytes());
        out
    }

    /// Decode a state produced by [`to_bytes`](Self::to_bytes).
    pub fn from_bytes(b: &[u8; Self::BYTES]) -> Option<Self> {
        let mut ab = [0u8; MomentsF64::BYTES];
        ab.copy_from_slice(&b[..MomentsF64::BYTES]);
        let mut rb = [0u8; MomentsF64::BYTES];
        rb.copy_from_slice(&b[MomentsF64::BYTES..]);
        Some(Self {
            adds: MomentsF64::from_bytes(&ab)?,
            removes: MomentsF64::from_bytes(&rb)?,
        })
    }
}

// ---------------------------------------------------------------------------
// Mergeable impls for the stats types
// ---------------------------------------------------------------------------

macro_rules! impl_mergeable_stats {
    ($($t:ty),+) => {$(
        impl crate::Mergeable for $t {
            fn merge(&mut self, other: &Self) {
                <$t>::merge(self, other);
            }
            fn count(&self) -> u64 {
                <$t>::count(self)
            }
            fn encode(&self) -> Vec<u8> {
                self.to_bytes().to_vec()
            }
            fn decode(bytes: &[u8]) -> Option<Self> {
                let arr: &[u8; <$t>::BYTES] = bytes.try_into().ok()?;
                <$t>::from_bytes(arr)
            }
        }
    )+};
}

impl_mergeable_stats!(
    MomentsF64,
    Moments4F64,
    CovF64,
    WeightedMomentsF64,
    PnMomentsF64
);
