// Copyright (c) 2026 Kyle Clouthier / Clouthier Simulation Labs. Licensed under MIT OR Apache-2.0.
//! Exact, order-invariant dot products via FMA two-products.

use crate::SumF64;

/// The dot product could not be computed exactly (see [`DotF64`] docs).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DotError {
    /// At least one partial product `a*b` fell into the subnormal range
    /// (0 < |a·b| < 2⁻¹⁰²²), where the FMA two-product transformation can
    /// lose the low part. The accumulated value is still a high-quality
    /// result (each affected term's error is below 2⁻¹⁰⁷⁴ · ½ulp of the
    /// subnormal grid), but bit-exactness of the *mathematical* dot product
    /// can no longer be guaranteed, so we tell you instead of pretending.
    ExactnessLost,
}

impl core::fmt::Display for DotError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "dot product exactness lost: a partial product underflowed to the subnormal range"
        )
    }
}

impl std::error::Error for DotError {}

/// An exact, order-invariant, mergeable dot product of `f64` pairs.
///
/// Each pair contributes `a*b` **exactly**, as two floats via the FMA
/// two-product transformation (Dekker/Ogita–Rump; `lo = fma(a, b, -hi)` is the
/// exact rounding error of `hi = a*b` whenever the product does not
/// under/overflow). Both parts land in a [`SumF64`], so the state is
/// order-invariant and mergeable exactly like a sum.
///
/// **Named limit:** if a partial product underflows (0 < |a·b| < 2⁻¹⁰²²), the
/// two-product loses exactness. This is *detected per pair* and surfaced via
/// [`is_exact`](Self::is_exact) / [`try_value`](Self::try_value) — never
/// silent. Products that overflow to ±∞, and NaN inputs, follow the same
/// flag semantics as [`SumF64`].
///
/// `mul_add` is IEEE-correctly-rounded on every Rust target (hardware FMA or
/// a correctly rounded soft fallback), so results are bit-identical across
/// architectures — the soft path is merely slower.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DotF64 {
    acc: SumF64,
    underflowed: bool,
}

impl DotF64 {
    /// An empty accumulator (dot = +0.0).
    #[inline]
    pub const fn new() -> Self {
        Self {
            acc: SumF64::new(),
            underflowed: false,
        }
    }

    /// Number of pairs pushed (each pair adds 2 to the underlying count).
    #[inline]
    pub const fn pairs(&self) -> u64 {
        self.acc.count() / 2
    }

    /// Accumulate one pair's exact product. Order never matters.
    pub fn push(&mut self, a: f64, b: f64) {
        let hi = a * b;
        let lo = a.mul_add(b, -hi);
        // Exactness holds unless the product underflowed into subnormals.
        // (Overflow gives hi = ±inf and lo = NaN; the NaN must NOT poison the
        // accumulator — infinity semantics are carried by hi alone.)
        if hi.is_infinite() {
            self.acc.add(hi);
            self.acc.add(0.0); // keep counts consistent: every push adds two
            return;
        }
        // The true product is nonzero exactly when both inputs are nonzero;
        // if it then lands below the normal range (subnormal OR flushed all
        // the way to zero), the two-product may be inexact. Conservative by
        // design: an exactly-representable subnormal product also trips the
        // flag — we prefer a false "inexact" to a silent wrong bit.
        if a != 0.0 && b != 0.0 && hi.abs() < f64::MIN_POSITIVE {
            self.underflowed = true;
        }
        self.acc.add(hi);
        self.acc.add(lo);
    }

    /// Accumulate the exact dot product of two slices.
    ///
    /// # Panics
    /// Panics if the slices have different lengths.
    pub fn extend_from_slices(&mut self, xs: &[f64], ys: &[f64]) {
        assert_eq!(
            xs.len(),
            ys.len(),
            "dot product requires equal-length slices"
        );
        for (x, y) in xs.iter().zip(ys) {
            self.push(*x, *y);
        }
    }

    /// Merge another accumulator into this one (shard combining).
    pub fn merge(&mut self, other: &DotF64) {
        self.acc.merge(&other.acc);
        self.underflowed |= other.underflowed;
    }

    /// True if every partial product so far was transformed exactly.
    #[inline]
    pub const fn is_exact(&self) -> bool {
        !self.underflowed
    }

    /// The correctly rounded dot product. If [`is_exact`](Self::is_exact) is
    /// false this is still returned (it remains far more accurate than a
    /// naive loop), but bit-level meaning is weakened — prefer
    /// [`try_value`](Self::try_value) when exactness is the contract.
    #[inline]
    pub fn value(&self) -> f64 {
        self.acc.value()
    }

    /// The exactly rounded dot product, or [`DotError::ExactnessLost`] if any
    /// partial product underflowed.
    pub fn try_value(&self) -> Result<f64, DotError> {
        if self.underflowed {
            Err(DotError::ExactnessLost)
        } else {
            Ok(self.acc.value())
        }
    }

    /// Canonical byte encoding of the state: the inner sum's bytes plus one
    /// exactness byte.
    pub fn to_bytes(&self) -> [u8; SumF64::BYTES + 1] {
        let mut out = [0u8; SumF64::BYTES + 1];
        out[..SumF64::BYTES].copy_from_slice(&self.acc.to_bytes());
        out[SumF64::BYTES] = self.underflowed as u8;
        out
    }

    /// Decode a state produced by [`to_bytes`](Self::to_bytes). Returns `None`
    /// if the inner sum is invalid or the exactness byte is not 0/1.
    pub fn from_bytes(b: &[u8; SumF64::BYTES + 1]) -> Option<Self> {
        let mut inner = [0u8; SumF64::BYTES];
        inner.copy_from_slice(&b[..SumF64::BYTES]);
        let acc = SumF64::from_bytes(&inner)?;
        let underflowed = match b[SumF64::BYTES] {
            0 => false,
            1 => true,
            _ => return None,
        };
        Some(Self { acc, underflowed })
    }
}

/// Convenience: the exactly rounded dot product of two slices.
///
/// # Panics
/// Panics if the slices have different lengths.
pub fn dot(xs: &[f64], ys: &[f64]) -> Result<f64, DotError> {
    let mut d = DotF64::new();
    d.extend_from_slices(xs, ys);
    d.try_value()
}

// Exact-tier extension (branch: exact-tier): group subtraction.
impl DotF64 {
    /// Exactly remove a previously merged contribution. Refuses (returns
    /// `false`, self untouched) if `other` recorded a product underflow —
    /// the underflow flag is sticky (semilattice), not subtractable.
    pub(crate) fn unmerge_assign(&mut self, other: &DotF64) -> bool {
        if other.underflowed {
            return false;
        }
        self.acc.try_unmerge(&other.acc)
    }
}
