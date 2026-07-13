// Copyright (c) 2026 Kyle Clouthier / Clouthier Simulation Labs. Licensed under MIT OR Apache-2.0.
//! PROBE (feature `probe`): a reproducible, mergeable, byte-identical
//! relative-error quantile sketch.
//!
//! This is an **exploratory** module, gated behind the `probe` feature so it
//! ships in no default build. It asks one question: can bitrep's
//! "same bytes everywhere" contract be extended from exact reductions to an
//! *approximate* quantile sketch (p50/p95/p99), the way every SRE/observability
//! stack needs?
//!
//! ## The idea
//!
//! The relative-error log-bucket sketch is **DDSketch** (Masson, Rim & Lee,
//! PVLDB 2019, arXiv:1908.10693): map a positive value to a bucket by its
//! logarithm, keep integer counts per bucket, and a quantile is read off the
//! bucket boundaries with a bounded *relative* error `alpha`. Integer counts
//! make the sketch mergeable by construction (sum counts per bucket), and the
//! merge is commutative and associative — exactly the algebra bitrep already
//! packages for sums.
//!
//! bitrep's contribution is **not** the sketch. It is the same thing the
//! superaccumulator does for sums: a **canonical byte encoding** of the sketch
//! state (sparse buckets in sorted key order, little-endian, integer counts)
//! so that two sketches over the same multiset — in any insertion order, any
//! sharding, any merge tree — are **byte-identical** and therefore
//! [`state_hash`](crate::state_hash)-identical. A p99 you can sign, hash and
//! content-address; identical on every replica and every shard order.
//!
//! ## The reproducibility hazard, and the choice made here
//!
//! DDSketch's canonical mapping is `i = ceil(log(x) / log(gamma))` with
//! `gamma = (1+alpha)/(1-alpha)`. That uses `libm`'s `log`, which is **not**
//! a correctly-rounded IEEE-754 operation: implementations differ by up to an
//! ULP across platforms. A one-ULP disagreement at a bucket boundary sends the
//! same `x` into different buckets on different machines — different counts,
//! different bytes. That silently breaks byte-identity, which is the whole
//! point.
//!
//! So this probe does **not** use `log`. It uses DDSketch's own
//! *BitwiseLinearlyInterpolatedMapping*: the bucket key is a pure right-shift
//! of the IEEE-754 bit pattern,
//!
//! ```text
//! key(x) = bits(x) >> (52 - SUB_BITS)   // x > 0, SUB_BITS mantissa bits kept
//! ```
//!
//! Because positive `f64` bit patterns are monotonic in value, this is a
//! monotone bucketing that splits each binade (power-of-two octave) into
//! `2^SUB_BITS` equal-mantissa sub-buckets — linear interpolation of `log2`.
//! It is computed with **integer shifts only**: no floating rounding, no
//! `libm`, bit-identical on every architecture that stores an `f64` as
//! IEEE-754. The price is that buckets are geometric *per octave* and linear
//! *within* an octave, so the relative error is not uniform; its worst case is
//! `alpha = 2^-(SUB_BITS+1)`, attained at the bottom of each octave. That
//! guarantee is what the probe tests measure.
//!
//! ## Scope / named limits
//!
//! * NaN, +∞, −∞ are tracked as out-of-band counts (never folded into a
//!   bucket), like the accumulator's special-value flags. Exact zeros are a
//!   dedicated counter. Negatives go in a separate bucket map.
//! * `min`/`max` are tracked exactly (order-invariant extrema).
//! * Merging two sketches with different `SUB_BITS` poisons the state (reported
//!   via `None` reads), never silently blended — same policy as
//!   [`HistogramF64`](crate::HistogramF64).
//! * This is an approximate sketch: quantile reads carry relative error up to
//!   `alpha`. The *state* is exact and byte-identical; the *estimate* is not
//!   the exact quantile. That distinction is deliberate and honest.

use crate::Mergeable;
use std::collections::BTreeMap;

/// A reproducible, mergeable, byte-identical relative-error quantile sketch.
///
/// See the [module docs](self) for the model. Two sketches (same `sub_bits`)
/// that received the same multiset — in any order, any sharding, any merge
/// tree — return identical [`to_bytes`](Self::to_bytes) and therefore identical
/// [`state_hash`](crate::state_hash).
#[derive(Clone, PartialEq, Debug)]
pub struct RelSketch {
    /// Mantissa bits kept in the bucket key: `2^sub_bits` sub-buckets / octave.
    sub_bits: u8,
    /// Positive-value buckets: key -> count, kept sorted (canonical order).
    pos: BTreeMap<u64, u64>,
    /// Negative-value buckets, keyed by the bucket key of `|x|`.
    neg: BTreeMap<u64, u64>,
    /// Count of exact ±0.0 samples.
    zero: u64,
    nan: u64,
    pos_inf: u64,
    neg_inf: u64,
    /// Exact extrema over non-NaN samples (sentinels +∞ / −∞ when empty).
    min: f64,
    max: f64,
    /// Total `add` operations (saturating), the [`Mergeable::count`] value.
    count: u64,
    /// Poisoned by a `sub_bits`-mismatched merge; reads then return `None`.
    mismatched: bool,
}

impl RelSketch {
    /// A sketch with a worst-case relative accuracy of at least `alpha`
    /// (`0 < alpha < 1`). The number of mantissa bits kept is chosen as the
    /// smallest `s` with `2^-(s+1) <= alpha`; call
    /// [`guaranteed_alpha`](Self::guaranteed_alpha) for the exact value.
    ///
    /// `alpha = 0.01` (1%) yields `sub_bits = 6` (64 sub-buckets/octave,
    /// guaranteed alpha `2^-7 ≈ 0.0078`).
    pub fn new(alpha: f64) -> Option<Self> {
        if !(alpha > 0.0 && alpha < 1.0) {
            return None;
        }
        // smallest s such that 2^-(s+1) <= alpha, i.e. 2^(s+1) >= 1/alpha.
        // Integer loop: no transcendental, no rounding surprises.
        let mut s: u8 = 0;
        while s < 52 {
            // 2^-(s+1) as f64 is exact for these small s.
            let bound = (0.5f64).powi(s as i32 + 1);
            if bound <= alpha {
                break;
            }
            s += 1;
        }
        Self::with_sub_bits(s)
    }

    /// A sketch keeping `sub_bits` mantissa bits (`1..=52`): `2^sub_bits`
    /// sub-buckets per octave, worst-case relative error `2^-(sub_bits+1)`.
    pub fn with_sub_bits(sub_bits: u8) -> Option<Self> {
        if sub_bits == 0 || sub_bits > 52 {
            return None;
        }
        Some(Self {
            sub_bits,
            pos: BTreeMap::new(),
            neg: BTreeMap::new(),
            zero: 0,
            nan: 0,
            pos_inf: 0,
            neg_inf: 0,
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
            count: 0,
            mismatched: false,
        })
    }

    /// The guaranteed worst-case relative error, `2^-(sub_bits+1)`.
    pub fn guaranteed_alpha(&self) -> f64 {
        (0.5f64).powi(self.sub_bits as i32 + 1)
    }

    /// Mantissa bits kept in the bucket key.
    pub const fn sub_bits(&self) -> u8 {
        self.sub_bits
    }

    /// The bucket key for a strictly-positive finite `x`: a pure shift of the
    /// IEEE-754 bits (integer-only, reproducible on every architecture).
    #[inline]
    fn key_of_positive(&self, x: f64) -> u64 {
        debug_assert!(x > 0.0 && x.is_finite());
        x.to_bits() >> (52 - self.sub_bits as u32)
    }

    /// Add one sample. Order never matters.
    pub fn add(&mut self, x: f64) {
        self.count = self.count.saturating_add(1);
        if x.is_nan() {
            self.nan = self.nan.saturating_add(1);
            return;
        }
        // extrema over every non-NaN sample (±∞ included)
        if x.total_cmp(&self.min).is_lt() {
            self.min = x;
        }
        if x.total_cmp(&self.max).is_gt() {
            self.max = x;
        }
        if x == f64::INFINITY {
            self.pos_inf = self.pos_inf.saturating_add(1);
        } else if x == f64::NEG_INFINITY {
            self.neg_inf = self.neg_inf.saturating_add(1);
        } else if x == 0.0 {
            self.zero = self.zero.saturating_add(1);
        } else if x > 0.0 {
            let k = self.key_of_positive(x);
            let e = self.pos.entry(k).or_insert(0);
            *e = e.saturating_add(1);
        } else {
            let k = self.key_of_positive(-x);
            let e = self.neg.entry(k).or_insert(0);
            *e = e.saturating_add(1);
        }
    }

    /// Total non-NaN samples (the denominator for a quantile rank).
    fn rankable(&self) -> u64 {
        let mut n = self
            .neg_inf
            .saturating_add(self.zero)
            .saturating_add(self.pos_inf);
        for &c in self.pos.values() {
            n = n.saturating_add(c);
        }
        for &c in self.neg.values() {
            n = n.saturating_add(c);
        }
        n
    }

    /// The `[lo, hi)` float range covered by a positive bucket `key`.
    fn range_of_key(&self, key: u64) -> (f64, f64) {
        let shift = 52 - self.sub_bits as u32;
        let lo = f64::from_bits(key << shift);
        let hi = f64::from_bits((key + 1) << shift);
        (lo, hi)
    }

    /// A deterministic representative value for a positive bucket: the
    /// arithmetic midpoint of its range (max relative error `2^-(sub_bits+1)`).
    fn representative(&self, key: u64) -> f64 {
        let (lo, hi) = self.range_of_key(key);
        if hi.is_finite() {
            lo + (hi - lo) * 0.5
        } else {
            lo
        }
    }

    /// The estimated `q`-quantile (`0 <= q <= 1`) with relative error at most
    /// [`guaranteed_alpha`](Self::guaranteed_alpha). Uses the nearest-rank
    /// definition (same convention as [`HistogramF64`](crate::HistogramF64)),
    /// evaluated over the exact integer bucket counts, so the read is
    /// deterministic. `None` if empty or poisoned.
    pub fn quantile(&self, q: f64) -> Option<f64> {
        if self.mismatched || !(0.0..=1.0).contains(&q) {
            return None;
        }
        let n = self.rankable();
        if n == 0 {
            return None;
        }
        let rank = ((q * n as f64).ceil() as u64).clamp(1, n);
        let mut seen = 0u64;

        // ascending value order: −∞, negative buckets (|x| descending),
        // zero, positive buckets (ascending), +∞.
        seen = seen.saturating_add(self.neg_inf);
        if self.neg_inf > 0 && seen >= rank {
            return Some(f64::NEG_INFINITY);
        }
        for (&key, &c) in self.neg.iter().rev() {
            seen = seen.saturating_add(c);
            if seen >= rank {
                return Some(-self.representative(key));
            }
        }
        seen = seen.saturating_add(self.zero);
        if self.zero > 0 && seen >= rank {
            return Some(0.0);
        }
        for (&key, &c) in self.pos.iter() {
            seen = seen.saturating_add(c);
            if seen >= rank {
                return Some(self.representative(key));
            }
        }
        // remainder must be +∞
        Some(f64::INFINITY)
    }

    /// Total `add` operations folded in (saturating).
    pub const fn count(&self) -> u64 {
        self.count
    }

    /// Exact minimum over non-NaN samples, or `None` if there are none.
    pub fn min(&self) -> Option<f64> {
        if self.mismatched || self.rankable() == 0 {
            None
        } else {
            Some(self.min)
        }
    }

    /// Exact maximum over non-NaN samples, or `None` if there are none.
    pub fn max(&self) -> Option<f64> {
        if self.mismatched || self.rankable() == 0 {
            None
        } else {
            Some(self.max)
        }
    }

    /// NaN samples seen (tracked, never dropped).
    pub const fn nan_count(&self) -> u64 {
        self.nan
    }

    /// Number of distinct occupied buckets (positive + negative).
    pub fn bucket_count(&self) -> usize {
        self.pos.len() + self.neg.len()
    }

    /// Canonical byte encoding: equal multisets (same `sub_bits`) ⇒ equal
    /// bytes. Buckets are written in sorted key order; every field is
    /// little-endian; counts are integers. Hash or sign this.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(self.sub_bits);
        out.push(self.mismatched as u8);
        out.extend_from_slice(&self.nan.to_le_bytes());
        out.extend_from_slice(&self.pos_inf.to_le_bytes());
        out.extend_from_slice(&self.neg_inf.to_le_bytes());
        out.extend_from_slice(&self.zero.to_le_bytes());
        out.extend_from_slice(&self.min.to_bits().to_le_bytes());
        out.extend_from_slice(&self.max.to_bits().to_le_bytes());
        out.extend_from_slice(&self.count.to_le_bytes());
        out.extend_from_slice(&(self.pos.len() as u32).to_le_bytes());
        for (&k, &c) in self.pos.iter() {
            out.extend_from_slice(&k.to_le_bytes());
            out.extend_from_slice(&c.to_le_bytes());
        }
        out.extend_from_slice(&(self.neg.len() as u32).to_le_bytes());
        for (&k, &c) in self.neg.iter() {
            out.extend_from_slice(&k.to_le_bytes());
            out.extend_from_slice(&c.to_le_bytes());
        }
        out
    }

    /// Decode a canonical encoding produced by [`to_bytes`](Self::to_bytes).
    /// Rejects malformed input (bad lengths, out-of-range `sub_bits`,
    /// non-ascending or duplicate bucket keys).
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        // fixed header: 1 + 1 + 8*6 + 4 = 54 bytes, then pos section.
        const HEAD: usize = 1 + 1 + 8 * 6 + 4;
        if bytes.len() < HEAD {
            return None;
        }
        let sub_bits = bytes[0];
        if sub_bits == 0 || sub_bits > 52 {
            return None;
        }
        let mismatched = match bytes[1] {
            0 => false,
            1 => true,
            _ => return None,
        };
        let mut at = 2;
        let rd8 = |at: &mut usize| -> u64 {
            let mut w = [0u8; 8];
            w.copy_from_slice(&bytes[*at..*at + 8]);
            *at += 8;
            u64::from_le_bytes(w)
        };
        let nan = rd8(&mut at);
        let pos_inf = rd8(&mut at);
        let neg_inf = rd8(&mut at);
        let zero = rd8(&mut at);
        let min = f64::from_bits(rd8(&mut at));
        let max = f64::from_bits(rd8(&mut at));
        let count = rd8(&mut at);

        let read_map = |at: &mut usize| -> Option<BTreeMap<u64, u64>> {
            if bytes.len() < *at + 4 {
                return None;
            }
            let mut w4 = [0u8; 4];
            w4.copy_from_slice(&bytes[*at..*at + 4]);
            *at += 4;
            let n = u32::from_le_bytes(w4) as usize;
            let need = n.checked_mul(16)?;
            if bytes.len() < at.checked_add(need)? {
                return None;
            }
            let mut map = BTreeMap::new();
            let mut prev: Option<u64> = None;
            for _ in 0..n {
                let mut w = [0u8; 8];
                w.copy_from_slice(&bytes[*at..*at + 8]);
                let key = u64::from_le_bytes(w);
                *at += 8;
                w.copy_from_slice(&bytes[*at..*at + 8]);
                let cnt = u64::from_le_bytes(w);
                *at += 8;
                // canonical form is strictly ascending, no zero counts
                if cnt == 0 || prev.is_some_and(|p| key <= p) {
                    return None;
                }
                prev = Some(key);
                map.insert(key, cnt);
            }
            Some(map)
        };

        let pos = read_map(&mut at)?;
        let neg = read_map(&mut at)?;
        if at != bytes.len() {
            return None;
        }
        Some(Self {
            sub_bits,
            pos,
            neg,
            zero,
            nan,
            pos_inf,
            neg_inf,
            min,
            max,
            count,
            mismatched,
        })
    }
}

impl Mergeable for RelSketch {
    /// Sum bucket counts pairwise (commutative, associative, deterministic).
    /// A `sub_bits` mismatch poisons the state, never silently blends.
    fn merge(&mut self, other: &Self) {
        if self.sub_bits != other.sub_bits {
            self.mismatched = true;
            return;
        }
        self.mismatched |= other.mismatched;
        for (&k, &c) in other.pos.iter() {
            let e = self.pos.entry(k).or_insert(0);
            *e = e.saturating_add(c);
        }
        for (&k, &c) in other.neg.iter() {
            let e = self.neg.entry(k).or_insert(0);
            *e = e.saturating_add(c);
        }
        self.zero = self.zero.saturating_add(other.zero);
        self.nan = self.nan.saturating_add(other.nan);
        self.pos_inf = self.pos_inf.saturating_add(other.pos_inf);
        self.neg_inf = self.neg_inf.saturating_add(other.neg_inf);
        if other.min.total_cmp(&self.min).is_lt() {
            self.min = other.min;
        }
        if other.max.total_cmp(&self.max).is_gt() {
            self.max = other.max;
        }
        self.count = self.count.saturating_add(other.count);
    }

    fn count(&self) -> u64 {
        self.count
    }

    #[cfg(feature = "std")]
    fn encode(&self) -> Vec<u8> {
        self.to_bytes()
    }

    #[cfg(feature = "std")]
    fn decode(bytes: &[u8]) -> Option<Self> {
        Self::from_bytes(bytes)
    }
}

impl FromIterator<f64> for RelSketch {
    /// Collect into a default-accuracy (1%) sketch.
    fn from_iter<I: IntoIterator<Item = f64>>(iter: I) -> Self {
        let mut s = Self::new(0.01).expect("alpha 0.01 is valid");
        for x in iter {
            s.add(x);
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_reads_are_none() {
        let s = RelSketch::new(0.01).unwrap();
        assert_eq!(s.quantile(0.5), None);
        assert_eq!(s.min(), None);
        assert_eq!(s.max(), None);
        assert_eq!(s.count(), 0);
    }

    #[test]
    fn alpha_to_sub_bits() {
        assert_eq!(RelSketch::new(0.01).unwrap().sub_bits(), 6);
        assert!(RelSketch::new(0.01).unwrap().guaranteed_alpha() <= 0.01);
        assert!(RelSketch::new(0.001).unwrap().guaranteed_alpha() <= 0.001);
        assert_eq!(RelSketch::new(0.0), None);
        assert_eq!(RelSketch::new(1.0), None);
    }

    #[test]
    fn monotone_quantiles() {
        let mut s = RelSketch::new(0.01).unwrap();
        for i in 1..=1000 {
            s.add(i as f64);
        }
        let (mut prev, qs) = (f64::NEG_INFINITY, [0.1, 0.5, 0.9, 0.99]);
        for &q in &qs {
            let v = s.quantile(q).unwrap();
            assert!(v >= prev, "quantiles must be nondecreasing");
            prev = v;
        }
        // exact extrema
        assert_eq!(s.min(), Some(1.0));
        assert_eq!(s.max(), Some(1000.0));
    }

    #[test]
    fn specials_are_out_of_band() {
        let mut s = RelSketch::new(0.01).unwrap();
        for x in [1.0, 2.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0.0, -3.0] {
            s.add(x);
        }
        assert_eq!(s.nan_count(), 1);
        assert_eq!(s.count(), 7);
        // −∞ is the smallest, +∞ the largest
        assert_eq!(s.quantile(0.0), Some(f64::NEG_INFINITY));
        assert_eq!(s.quantile(1.0), Some(f64::INFINITY));
    }

    #[test]
    fn roundtrip_bytes() {
        let mut s = RelSketch::new(0.005).unwrap();
        for i in 0..500 {
            s.add((i as f64) * 0.5 - 100.0);
        }
        let bytes = s.to_bytes();
        let back = RelSketch::from_bytes(&bytes).unwrap();
        assert_eq!(s, back);
        assert_eq!(bytes, back.to_bytes());
        assert_eq!(RelSketch::from_bytes(&bytes[..bytes.len() - 1]), None);
    }

    #[test]
    fn merge_mismatch_poisons() {
        let mut a = RelSketch::with_sub_bits(6).unwrap();
        let b = RelSketch::with_sub_bits(7).unwrap();
        a.add(1.0);
        a.merge(&b);
        assert_eq!(a.quantile(0.5), None);
    }
}
