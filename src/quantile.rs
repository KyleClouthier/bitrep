// Copyright (c) 2026 Kyle Clouthier / Clouthier Simulation Labs. Licensed under MIT OR Apache-2.0.
//! Feature `quantile`: a reproducible, mergeable, byte-identical
//! relative-error quantile sketch.
//!
//! Order statistics (p50/p95/p99) are outside the exact-monomial algebra the
//! rest of the crate lives in — no exact mergeable representation exists (the
//! honest exact primitive is [`HistogramF64`](crate::HistogramF64)'s bucket
//! *bounds*). What *does* fit the crate's contract is an **approximate**
//! sketch whose *state* is exact and byte-identical: the estimate carries a
//! bounded relative error, but the bytes you sign do not.
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
//! state (sparse buckets in sorted key order, delta-varint keys, little-endian
//! header, integer counts) so that two sketches over the same multiset — in
//! any insertion order, any sharding, any merge tree, on any architecture —
//! are **byte-identical** and therefore
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
//! So this sketch does **not** use `log`. It uses DDSketch's own
//! *BitwiseLinearlyInterpolatedMapping*: the bucket key is a pure right-shift
//! of the IEEE-754 bit pattern,
//!
//! ```text
//! key(x) = bits(x) >> (52 - sub_bits)   // x > 0, sub_bits mantissa bits kept
//! ```
//!
//! Because positive `f64` bit patterns are monotonic in value, this is a
//! monotone bucketing that splits each binade (power-of-two octave) into
//! `2^sub_bits` equal-mantissa sub-buckets — linear interpolation of `log2`.
//! It is computed with **integer shifts only**: no floating rounding, no
//! `libm`, bit-identical on every architecture that stores an `f64` as
//! IEEE-754. The price is that buckets are geometric *per octave* and linear
//! *within* an octave, so the relative error is not uniform; its worst case is
//! `alpha = 2^-(sub_bits+1)`, attained at the bottom of each octave. That
//! guarantee is what the tests measure. This same integer mapping is, up to a
//! choice of scale, the one OpenTelemetry exponential histograms and
//! Prometheus native histograms use — see [`RelSketch::otel_scale`].
//!
//! ## Scope / named limits
//!
//! * NaN, +∞, −∞ are tracked as out-of-band counts (never folded into a
//!   bucket), like the accumulator's special-value flags. Exact zeros are a
//!   dedicated counter. Negatives go in a separate bucket map.
//! * `min`/`max` are tracked exactly (order-invariant extrema).
//! * Merging two sketches with different `sub_bits` poisons the state (reported
//!   via `None` reads), never silently blended — same policy as
//!   [`HistogramF64`](crate::HistogramF64).
//! * A hostile stream spanning every exponent cannot blow memory: the bucket
//!   count is capped at [`RelSketch::MAX_BUCKETS`] and, past that, resolution is
//!   deterministically halved (a *collapse*, tracked in the state). Collapse is
//!   a pure function of the multiset, so byte-identity survives it; it only
//!   coarsens the guarantee — see [`RelSketch::guaranteed_alpha`].
//! * This is an approximate sketch: quantile reads carry relative error up to
//!   `alpha`. The *state* is exact and byte-identical; the *estimate* is not
//!   the exact quantile. That distinction is deliberate and honest.

use crate::Mergeable;
use std::collections::BTreeMap;

/// A reproducible, mergeable, byte-identical relative-error quantile sketch.
///
/// See the crate-level and module documentation for the model. Two sketches (same `sub_bits`)
/// that received the same multiset — in any order, any sharding, any merge
/// tree — return identical [`to_bytes`](Self::to_bytes) and therefore identical
/// [`state_hash`](crate::state_hash).
#[derive(Clone, PartialEq, Debug)]
pub struct RelSketch {
    /// Mantissa bits kept in the bucket key: `2^sub_bits` sub-buckets / octave.
    sub_bits: u8,
    /// Resolution collapses applied (DoS guard): stored keys are the ideal
    /// keys right-shifted by this much. 0 for any non-adversarial input.
    collapse_shift: u8,
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
    /// Maximum number of occupied buckets (positive + negative) before the
    /// sketch collapses resolution to bound memory. Chosen so that a
    /// worst-case adversarial stream (every exponent × sub-bucket) is held to
    /// roughly a megabyte of state while ordinary metric data — a few thousand
    /// buckets — never collapses at all.
    pub const MAX_BUCKETS: usize = 1 << 16;

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
            collapse_shift: 0,
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

    /// The guaranteed worst-case relative error at the sketch's *current*
    /// resolution, `2^-(sub_bits - collapse_shift + 1)`. Equals `2^-(sub_bits+1)`
    /// for any input that never triggered a collapse (all realistic data);
    /// after `k` collapses the guarantee coarsens by a factor of `2^k`.
    pub fn guaranteed_alpha(&self) -> f64 {
        let eff = self.sub_bits as i32 - self.collapse_shift as i32;
        if eff <= 0 {
            1.0
        } else {
            (0.5f64).powi(eff + 1)
        }
    }

    /// Mantissa bits kept in the bucket key at construction.
    pub const fn sub_bits(&self) -> u8 {
        self.sub_bits
    }

    /// Resolution collapses applied so far (0 for any non-adversarial input).
    /// The effective resolution is `sub_bits - collapse_shift` mantissa bits.
    pub const fn collapse_shift(&self) -> u8 {
        self.collapse_shift
    }

    /// The bit-shift applied to `f64` bits to form a stored bucket key,
    /// `52 - sub_bits + collapse_shift`. This is the exponent-of-two the OTel
    /// / Prometheus exponential-histogram `scale` is defined against — see
    /// [`otel_scale`](Self::otel_scale).
    #[inline]
    pub const fn effective_shift(&self) -> u32 {
        52 - self.sub_bits as u32 + self.collapse_shift as u32
    }

    /// The bucket key for a strictly-positive finite `x`: a pure shift of the
    /// IEEE-754 bits (integer-only, reproducible on every architecture),
    /// already coarsened by any collapse in effect.
    #[inline]
    fn key_of_positive(&self, x: f64) -> u64 {
        debug_assert!(x > 0.0 && x.is_finite());
        x.to_bits() >> self.effective_shift()
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
            self.enforce_bucket_cap();
        } else {
            let k = self.key_of_positive(-x);
            let e = self.neg.entry(k).or_insert(0);
            *e = e.saturating_add(1);
            self.enforce_bucket_cap();
        }
    }

    /// Collapse one map (`key -> key >> 1`, summing counts). Halving the key
    /// space is a pure function of the map: applying it to the same multiset,
    /// however that multiset was accumulated, yields the same result.
    fn collapse_map(m: &BTreeMap<u64, u64>) -> BTreeMap<u64, u64> {
        let mut out = BTreeMap::new();
        for (&k, &c) in m.iter() {
            let e = out.entry(k >> 1).or_insert(0u64);
            *e = e.saturating_add(c);
        }
        out
    }

    /// Restore the bucket-count invariant by halving resolution until the
    /// occupied-bucket count is within [`MAX_BUCKETS`](Self::MAX_BUCKETS).
    /// Confluent: the number of collapses needed is a function of the final
    /// multiset (the smallest shift whose key-image fits the cap), so any
    /// order/sharding/merge-tree reaches the same collapsed state.
    fn enforce_bucket_cap(&mut self) {
        while self.pos.len() + self.neg.len() > Self::MAX_BUCKETS {
            // Never let the reconstruction shift reach 64 (would be UB in a
            // raw shift); at that point resolution is already annihilated.
            if self.effective_shift() + 1 >= 64 {
                break;
            }
            self.collapse_shift += 1;
            self.pos = Self::collapse_map(&self.pos);
            self.neg = Self::collapse_map(&self.neg);
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
        let shift = self.effective_shift();
        let lo = key
            .checked_shl(shift)
            .map(f64::from_bits)
            .unwrap_or(f64::INFINITY);
        let hi = (key + 1)
            .checked_shl(shift)
            .map(f64::from_bits)
            .unwrap_or(f64::INFINITY);
        (lo, hi)
    }

    /// A deterministic representative value for a positive bucket: the
    /// arithmetic midpoint of its range (max relative error at the current
    /// resolution).
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
    /// deterministic. `None` if empty, poisoned, or `q` is outside `[0, 1]`.
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

    // -- OpenTelemetry / Prometheus exponential-histogram correspondence -----

    /// The OpenTelemetry / Prometheus exponential-histogram **scale** whose
    /// bucket boundaries coincide with this sketch's octave sub-bucketing.
    ///
    /// OTel scale `s` uses base `2^(2^-s)`, i.e. `2^s` buckets per power of two.
    /// This sketch keeps `sub_bits - collapse_shift` mantissa bits, i.e.
    /// `2^(sub_bits - collapse_shift)` sub-buckets per octave. The two layouts
    /// have the same **resolution and octave alignment** exactly when
    /// `scale = sub_bits - collapse_shift`.
    ///
    /// They are **not** the same mapping inside an octave: OTel spaces interior
    /// boundaries geometrically (`base^i`), this sketch spaces them by linear
    /// mantissa interpolation (DDSketch's `BitwiseLinearlyInterpolatedMapping`).
    /// A value's sub-bucket index therefore differs by up to
    /// `≈ 0.0861 · 2^scale` mid-octave, coinciding at power-of-two boundaries up
    /// to the one-bucket upper-vs-lower-inclusive convention offset. Both bound
    /// the relative error by `~2^-(scale+1)`, so the histograms are
    /// interchangeable within that error, but a faithful conversion re-buckets
    /// through the mapping rather than shifting indices. See
    /// `examples/otel_bridge.rs`, which measures the gap.
    pub fn otel_scale(&self) -> i32 {
        self.sub_bits as i32 - self.collapse_shift as i32
    }

    /// The positive buckets as `(index, count)` pairs, ascending by index, in
    /// this sketch's own exponential-histogram layout at
    /// [`otel_scale`](Self::otel_scale): the index is the sketch key minus the
    /// key of `1.0`, so index `0` is the sub-bucket at `[1.0, …)`.
    ///
    /// This layout is a bijection with the buckets, so it round-trips exactly
    /// through [`from_otel`](Self::from_otel) — emit it alongside the histogram
    /// you already export to attach a signed [`state_hash`](crate::state_hash).
    /// It shares OTel/Prometheus resolution and octave alignment but keeps this
    /// sketch's linear interior mapping (see [`otel_scale`](Self::otel_scale)),
    /// so it is not identical to an OTel geometric index array.
    pub fn otel_positive_indices(&self) -> Vec<(i64, u64)> {
        let one_key = 1.0f64.to_bits() >> self.effective_shift();
        self.pos
            .iter()
            .map(|(&k, &c)| (k as i64 - one_key as i64, c))
            .collect()
    }

    /// The negative buckets as `(otel_index, count)` pairs (index computed from
    /// `|x|`, as OTel records negatives in a separate `negative` field).
    pub fn otel_negative_indices(&self) -> Vec<(i64, u64)> {
        let one_key = 1.0f64.to_bits() >> self.effective_shift();
        self.neg
            .iter()
            .map(|(&k, &c)| (k as i64 - one_key as i64, c))
            .collect()
    }

    /// Rebuild the **bucket structure** of a sketch from OTel-style
    /// `(index, count)` arrays (as produced by
    /// [`otel_positive_indices`](Self::otel_positive_indices) /
    /// [`otel_negative_indices`](Self::otel_negative_indices)) at the given
    /// `sub_bits`. The exponential-histogram layout carries only the buckets:
    /// specials (zero/NaN/±∞) and exact extrema are not part of it and start
    /// empty (`count` is set to the reconstructed bucket mass). Returns `None`
    /// on a malformed layout — `sub_bits` out of range, non-ascending or
    /// duplicate indices, zero counts, or an index that maps outside the key
    /// space. Round-trips the bucket layer exactly, so an exporter can attach a
    /// signed [`state_hash`](crate::state_hash) to the histogram it already
    /// emits.
    pub fn from_otel(
        sub_bits: u8,
        positive: &[(i64, u64)],
        negative: &[(i64, u64)],
    ) -> Option<Self> {
        let mut s = Self::with_sub_bits(sub_bits)?;
        let one_key = (1.0f64.to_bits() >> s.effective_shift()) as i64;
        fn build(arr: &[(i64, u64)], one_key: i64) -> Option<BTreeMap<u64, u64>> {
            let mut m = BTreeMap::new();
            let mut prev: Option<i64> = None;
            for &(idx, c) in arr {
                if c == 0 {
                    return None;
                }
                if prev.is_some_and(|p| idx <= p) {
                    return None; // indices must be strictly ascending / unique
                }
                prev = Some(idx);
                let key = one_key.checked_add(idx)?;
                if key < 0 {
                    return None;
                }
                m.insert(key as u64, c);
            }
            Some(m)
        }
        s.pos = build(positive, one_key)?;
        s.neg = build(negative, one_key)?;
        s.count = s
            .pos
            .values()
            .chain(s.neg.values())
            .fold(0u64, |a, &c| a.saturating_add(c));
        Some(s)
    }

    // -- Canonical encoding --------------------------------------------------

    /// Canonical byte encoding: equal multisets (same `sub_bits`) ⇒ equal
    /// bytes. Buckets are written in sorted key order as **delta-varint** keys
    /// (LEB128 gaps, always minimal) and varint counts; every header field is
    /// little-endian. Hash or sign this. Roughly halves the 16-bytes/bucket of
    /// a flat `(u64, u64)` layout because sorted keys have small gaps and
    /// counts fit in one or two bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(self.sub_bits);
        out.push(self.collapse_shift);
        out.push(self.mismatched as u8);
        out.extend_from_slice(&self.nan.to_le_bytes());
        out.extend_from_slice(&self.pos_inf.to_le_bytes());
        out.extend_from_slice(&self.neg_inf.to_le_bytes());
        out.extend_from_slice(&self.zero.to_le_bytes());
        out.extend_from_slice(&self.min.to_bits().to_le_bytes());
        out.extend_from_slice(&self.max.to_bits().to_le_bytes());
        out.extend_from_slice(&self.count.to_le_bytes());
        Self::write_map(&mut out, &self.pos);
        Self::write_map(&mut out, &self.neg);
        out
    }

    /// Write a bucket map as varint length, then delta-varint keys + varint
    /// counts. The first key is stored absolutely; subsequent keys as the gap
    /// from the previous (always `>= 1` since keys are strictly ascending).
    fn write_map(out: &mut Vec<u8>, m: &BTreeMap<u64, u64>) {
        write_uvarint(out, m.len() as u64);
        let mut prev: Option<u64> = None;
        for (&k, &c) in m.iter() {
            let delta = match prev {
                None => k,
                Some(p) => k - p, // strictly ascending ⇒ k > p
            };
            write_uvarint(out, delta);
            write_uvarint(out, c);
            prev = Some(k);
        }
    }

    /// Decode a canonical encoding produced by [`to_bytes`](Self::to_bytes).
    ///
    /// **Strict**: rejects every non-canonical encoding — non-minimal varints,
    /// unsorted or duplicate keys (delta `0`), zero counts, out-of-range
    /// `sub_bits`/`collapse_shift`, trailing bytes, and key sums that overflow
    /// `u64`. A decoder that accepted a non-canonical form would let two
    /// distinct byte strings decode to the same state, breaking the
    /// merge-law/receipt guarantee — the class of bug Kani caught on the v0.2
    /// [`ExtremaF64`](crate::ExtremaF64) decoder.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        // fixed header: sub_bits + collapse_shift + mismatched + 4×u64 + 2×f64bits + count
        const HEAD: usize = 1 + 1 + 1 + 8 * 4 + 8 * 2 + 8;
        if bytes.len() < HEAD {
            return None;
        }
        let sub_bits = bytes[0];
        if sub_bits == 0 || sub_bits > 52 {
            return None;
        }
        let collapse_shift = bytes[1];
        // The reconstruction shift must stay < 64 (a raw shift of 64 is UB);
        // this is exactly the invariant enforce_bucket_cap maintains.
        if 52 - sub_bits as u32 + collapse_shift as u32 >= 64 {
            return None;
        }
        let mismatched = match bytes[2] {
            0 => false,
            1 => true,
            _ => return None,
        };
        let mut at = 3;
        let rd8 = |bytes: &[u8], at: &mut usize| -> u64 {
            let mut w = [0u8; 8];
            w.copy_from_slice(&bytes[*at..*at + 8]);
            *at += 8;
            u64::from_le_bytes(w)
        };
        let nan = rd8(bytes, &mut at);
        let pos_inf = rd8(bytes, &mut at);
        let neg_inf = rd8(bytes, &mut at);
        let zero = rd8(bytes, &mut at);
        let min = f64::from_bits(rd8(bytes, &mut at));
        let max = f64::from_bits(rd8(bytes, &mut at));
        let count = rd8(bytes, &mut at);

        let pos = read_map(bytes, &mut at)?;
        let neg = read_map(bytes, &mut at)?;
        if at != bytes.len() {
            return None; // trailing bytes are not canonical
        }
        Some(Self {
            sub_bits,
            collapse_shift,
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

/// Append `v` as an unsigned LEB128 varint (minimal by construction).
fn write_uvarint(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let byte = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

/// Read a **strict canonical** unsigned LEB128 varint: rejects overlong /
/// non-minimal encodings (a multi-byte encoding whose terminating group is
/// zero) and any encoding that would overflow `u64`.
fn read_uvarint(bytes: &[u8], at: &mut usize) -> Option<u64> {
    let mut result: u64 = 0;
    let mut shift: u32 = 0;
    loop {
        let b = *bytes.get(*at)?;
        *at += 1;
        let low = (b & 0x7f) as u64;
        if shift >= 64 {
            return None; // more groups than u64 can hold
        }
        // A group at shift 63 may only carry the single remaining bit.
        if shift == 63 && low > 1 {
            return None;
        }
        result |= low << shift;
        if b & 0x80 == 0 {
            // Terminating group. Minimal encoding: the terminator is zero only
            // when it is the whole encoding (the value 0). Any multi-byte
            // encoding ending in a zero group is overlong ⇒ reject.
            if b == 0 && shift != 0 {
                return None;
            }
            return Some(result);
        }
        shift += 7;
    }
}

/// Read a bucket map written by [`RelSketch::write_map`], enforcing every
/// canonical-form invariant (see [`RelSketch::from_bytes`]).
fn read_map(bytes: &[u8], at: &mut usize) -> Option<BTreeMap<u64, u64>> {
    let n = read_uvarint(bytes, at)?;
    let mut map = BTreeMap::new();
    let mut prev: Option<u64> = None;
    for _ in 0..n {
        let delta = read_uvarint(bytes, at)?;
        let key = match prev {
            None => delta,
            Some(p) => {
                if delta == 0 {
                    return None; // duplicate / non-ascending key
                }
                p.checked_add(delta)? // key sum must not overflow u64
            }
        };
        let cnt = read_uvarint(bytes, at)?;
        if cnt == 0 {
            return None; // a canonical map never stores an empty bucket
        }
        prev = Some(key);
        map.insert(key, cnt);
    }
    Some(map)
}

impl Mergeable for RelSketch {
    /// Sum bucket counts pairwise (commutative, associative, deterministic).
    /// A `sub_bits` mismatch poisons the state, never silently blends. If the
    /// two sides sit at different collapse levels they are first brought to the
    /// coarser common resolution, so the merge stays a pure function of the
    /// combined multiset.
    fn merge(&mut self, other: &Self) {
        if self.sub_bits != other.sub_bits {
            self.mismatched = true;
            return;
        }
        self.mismatched |= other.mismatched;

        // Bring both operands to the coarser of the two collapse levels.
        let target = self.collapse_shift.max(other.collapse_shift);
        while self.collapse_shift < target {
            self.collapse_shift += 1;
            self.pos = Self::collapse_map(&self.pos);
            self.neg = Self::collapse_map(&self.neg);
        }
        let (mut o_pos, mut o_neg) = (other.pos.clone(), other.neg.clone());
        let mut oc = other.collapse_shift;
        while oc < target {
            o_pos = Self::collapse_map(&o_pos);
            o_neg = Self::collapse_map(&o_neg);
            oc += 1;
        }

        for (&k, &c) in o_pos.iter() {
            let e = self.pos.entry(k).or_insert(0);
            *e = e.saturating_add(c);
        }
        for (&k, &c) in o_neg.iter() {
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

        // The combined map may exceed the cap even if neither operand did.
        self.enforce_bucket_cap();
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
        for x in [
            1.0,
            2.0,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            0.0,
            -3.0,
        ] {
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

    #[test]
    fn varint_roundtrip_minimal() {
        for v in [0u64, 1, 127, 128, 300, u64::MAX, u64::MAX - 1, 1 << 63] {
            let mut buf = Vec::new();
            write_uvarint(&mut buf, v);
            let mut at = 0;
            assert_eq!(read_uvarint(&buf, &mut at), Some(v));
            assert_eq!(at, buf.len());
        }
        // non-minimal (overlong) encoding of 0 must be rejected
        let mut at = 0;
        assert_eq!(read_uvarint(&[0x80, 0x00], &mut at), None);
        // overlong encoding of 1 must be rejected
        let mut at = 0;
        assert_eq!(read_uvarint(&[0x81, 0x00], &mut at), None);
    }
}
