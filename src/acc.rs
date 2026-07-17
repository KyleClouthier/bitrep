// Copyright (c) 2026 Kyle Clouthier / Clouthier Simulation Labs. Licensed under MIT OR Apache-2.0.
//! The superaccumulator: a fixed-point two's-complement integer wide enough to
//! hold any finite `f64` exactly, in units of 2^-1074.
//!
//! Layout (little-endian limbs, bit 0 of limb 0 = weight 2^-1074):
//!
//! * finite f64 values occupy bit positions 0..=2097
//!   (min subnormal 2^-1074 -> bit 0; max value bit 2^1023 -> bit 2097)
//! * bits 2098..=2175 are headroom: 2^63 additions of the largest finite
//!   f64 cannot overflow into the sign bit, so wrap-around never occurs
//!   within the documented capacity.
//!
//! Integer addition on this representation is associative and commutative,
//! which is the entire point: the *state* is order-invariant, not merely the
//! rounded result.

/// Number of 64-bit limbs in the accumulator (34 * 64 = 2176 bits).
const LIMBS: usize = 34;

/// Bit position of the units for the f64 grid: value = integer * 2^-1074.
const BIAS: i32 = 1074;

/// Special-value flags, kept out of the integer so ±∞/NaN never poison limbs.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
struct Specials {
    nan: bool,
    pos_inf: bool,
    neg_inf: bool,
}

impl Specials {
    #[inline]
    fn merge(&mut self, o: &Specials) {
        self.nan |= o.nan;
        self.pos_inf |= o.pos_inf;
        self.neg_inf |= o.neg_inf;
    }
    #[inline]
    fn any(&self) -> bool {
        self.nan || self.pos_inf || self.neg_inf
    }
}

/// An exact, order-invariant, mergeable sum of `f64` values.
///
/// See the [crate docs](crate) for the model. Two accumulators that received
/// the same multiset of values — in any order, in any sharding — are
/// bit-identical ([`to_bytes`](Self::to_bytes) returns the same bytes), and
/// [`value`](Self::value) is the exactly rounded (nearest, ties-to-even) sum.
///
/// Size: 289 bytes serialized; `Copy` is deliberately not implemented (it is
/// a large value; clone explicitly).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SumF64 {
    limbs: [u64; LIMBS],
    specials: Specials,
    count: u64,
}

impl Default for SumF64 {
    fn default() -> Self {
        Self::new()
    }
}

impl SumF64 {
    /// Serialized size in bytes: 34 limbs * 8 + 1 flag byte + 8 count bytes.
    pub const BYTES: usize = LIMBS * 8 + 1 + 8;

    /// An empty accumulator (sum = +0.0, count = 0).
    #[inline]
    pub const fn new() -> Self {
        Self {
            limbs: [0; LIMBS],
            specials: Specials {
                nan: false,
                pos_inf: false,
                neg_inf: false,
            },
            count: 0,
        }
    }

    /// Number of values added (including zeros, NaNs and infinities; merges add counts).
    #[inline]
    pub const fn count(&self) -> u64 {
        self.count
    }

    /// Add one value. Order never matters.
    ///
    /// The count saturates at `u64::MAX`; the documented exactness capacity
    /// (2^63 additions) is far below that, so saturation is unreachable in
    /// legitimate use but keeps adversarial deserialized states panic-free.
    pub fn add(&mut self, x: f64) {
        self.count = self.count.saturating_add(1);
        let bits = x.to_bits();
        let neg = bits >> 63 != 0;
        let expf = ((bits >> 52) & 0x7ff) as i32;
        let frac = bits & ((1u64 << 52) - 1);

        if expf == 0x7ff {
            // NaN or infinity: flag, never touch the integer.
            if frac != 0 {
                self.specials.nan = true;
            } else if neg {
                self.specials.neg_inf = true;
            } else {
                self.specials.pos_inf = true;
            }
            return;
        }
        // Decompose |x| = m * 2^e with integer m < 2^53.
        let (m, e) = if expf == 0 {
            if frac == 0 {
                return; // ±0.0 contributes nothing (canonical zero policy).
            }
            (frac, 1 - 1075) // subnormal: m = frac, e = -1074
        } else {
            (frac | (1u64 << 52), expf - 1075)
        };
        let pos = (e + BIAS) as usize; // 0..=2045: bit position of m's LSB
        let limb = pos >> 6;
        let off = pos & 63;
        // m shifted left by `off` spans at most 117 bits -> two limbs.
        let wide = (m as u128) << off;
        let (lo, hi) = (wide as u64, (wide >> 64) as u64);
        if neg {
            self.sub_wide(limb, lo, hi);
        } else {
            self.add_wide(limb, lo, hi);
        }
    }

    /// Merge another accumulator into this one (shard combining).
    /// Merging is associative and commutative; any merge tree over the same
    /// shards yields identical bytes.
    pub fn merge(&mut self, other: &SumF64) {
        let mut carry = 0u64;
        for i in 0..LIMBS {
            let (a, c1) = self.limbs[i].overflowing_add(other.limbs[i]);
            let (b, c2) = a.overflowing_add(carry);
            self.limbs[i] = b;
            carry = (c1 as u64) + (c2 as u64);
        }
        // Top-limb wrap is unreachable within capacity (headroom analysis in
        // the module docs); two's complement makes signed merge "just add".
        self.specials.merge(&other.specials);
        self.count = self.count.saturating_add(other.count);
    }

    /// The exactly rounded sum (round-to-nearest, ties-to-even).
    ///
    /// NaN if any NaN was added, or if both +∞ and −∞ were; ±∞ if a single
    /// sign of infinity was added (or the exact sum overflows f64); `+0.0`
    /// for an exactly-zero sum.
    pub fn value(&self) -> f64 {
        if self.specials.any() {
            return match (
                self.specials.nan,
                self.specials.pos_inf,
                self.specials.neg_inf,
            ) {
                (true, _, _) | (_, true, true) => f64::NAN,
                (_, true, false) => f64::INFINITY,
                (_, false, true) => f64::NEG_INFINITY,
                _ => unreachable!(),
            };
        }
        let negative = self.limbs[LIMBS - 1] >> 63 != 0;
        let mag = if negative {
            negate(&self.limbs)
        } else {
            self.limbs
        };
        round_mag(&mag, negative, 52, -1022, 1023)
    }

    /// Canonical byte encoding of the full state (little-endian limbs, one
    /// flag byte, little-endian count). Equal multisets -> equal bytes; feed
    /// this to your hash or signature.
    pub fn to_bytes(&self) -> [u8; Self::BYTES] {
        let mut out = [0u8; Self::BYTES];
        for (i, l) in self.limbs.iter().enumerate() {
            out[i * 8..i * 8 + 8].copy_from_slice(&l.to_le_bytes());
        }
        out[LIMBS * 8] = (self.specials.nan as u8)
            | ((self.specials.pos_inf as u8) << 1)
            | ((self.specials.neg_inf as u8) << 2);
        out[LIMBS * 8 + 1..].copy_from_slice(&self.count.to_le_bytes());
        out
    }

    /// Decode a state produced by [`to_bytes`](Self::to_bytes). Returns `None`
    /// if the flag byte has unknown bits set (wrong version / corruption).
    pub fn from_bytes(b: &[u8; Self::BYTES]) -> Option<Self> {
        let mut limbs = [0u64; LIMBS];
        for (i, l) in limbs.iter_mut().enumerate() {
            let mut le = [0u8; 8];
            le.copy_from_slice(&b[i * 8..i * 8 + 8]);
            *l = u64::from_le_bytes(le);
        }
        let f = b[LIMBS * 8];
        if f & !0b111 != 0 {
            return None;
        }
        let mut le = [0u8; 8];
        le.copy_from_slice(&b[LIMBS * 8 + 1..]);
        Some(Self {
            limbs,
            specials: Specials {
                nan: f & 1 != 0,
                pos_inf: f & 2 != 0,
                neg_inf: f & 4 != 0,
            },
            count: u64::from_le_bytes(le),
        })
    }

    /// Add a positive 117-bit quantity (lo, hi) at limb index `limb`,
    /// rippling the carry upward. Bounded by LIMBS; headroom guarantees the
    /// ripple terminates before the top within capacity.
    #[inline]
    fn add_wide(&mut self, limb: usize, lo: u64, hi: u64) {
        let (v, mut carry) = self.limbs[limb].overflowing_add(lo);
        self.limbs[limb] = v;
        let mut i = limb + 1;
        if i < LIMBS {
            let (v, c1) = self.limbs[i].overflowing_add(hi);
            let (v, c2) = v.overflowing_add(carry as u64);
            self.limbs[i] = v;
            carry = c1 || c2;
            i += 1;
        }
        while carry && i < LIMBS {
            let (v, c) = self.limbs[i].overflowing_add(1);
            self.limbs[i] = v;
            carry = c;
            i += 1;
        }
    }

    /// Subtract a positive 117-bit quantity (two's complement borrow ripple).
    #[inline]
    fn sub_wide(&mut self, limb: usize, lo: u64, hi: u64) {
        let (v, mut borrow) = self.limbs[limb].overflowing_sub(lo);
        self.limbs[limb] = v;
        let mut i = limb + 1;
        if i < LIMBS {
            let (v, b1) = self.limbs[i].overflowing_sub(hi);
            let (v, b2) = v.overflowing_sub(borrow as u64);
            self.limbs[i] = v;
            borrow = b1 || b2;
            i += 1;
        }
        while borrow && i < LIMBS {
            let (v, b) = self.limbs[i].overflowing_sub(1);
            self.limbs[i] = v;
            borrow = b;
            i += 1;
        }
    }
}

impl core::iter::FromIterator<f64> for SumF64 {
    fn from_iter<I: IntoIterator<Item = f64>>(iter: I) -> Self {
        let mut acc = Self::new();
        for x in iter {
            acc.add(x);
        }
        acc
    }
}

impl core::iter::Extend<f64> for SumF64 {
    fn extend<I: IntoIterator<Item = f64>>(&mut self, iter: I) {
        for x in iter {
            self.add(x);
        }
    }
}

/// An exact, order-invariant, mergeable sum of `f32` values.
///
/// Every `f32` is exactly representable as `f64`, so the state is a [`SumF64`]
/// over the widened inputs; [`value`](Self::value) rounds the exact sum
/// **directly to f32** (a single rounding — no double-rounding through f64).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct SumF32 {
    inner: SumF64,
}

impl SumF32 {
    /// Serialized size in bytes (same encoding as [`SumF64`]).
    pub const BYTES: usize = SumF64::BYTES;

    /// An empty accumulator.
    #[inline]
    pub const fn new() -> Self {
        Self {
            inner: SumF64::new(),
        }
    }

    /// Number of values added.
    #[inline]
    pub const fn count(&self) -> u64 {
        self.inner.count()
    }

    /// Add one value. Order never matters.
    #[inline]
    pub fn add(&mut self, x: f32) {
        self.inner.add(x as f64);
    }

    /// Merge another accumulator into this one.
    #[inline]
    pub fn merge(&mut self, other: &SumF32) {
        self.inner.merge(&other.inner);
    }

    /// The exactly rounded `f32` sum (nearest, ties-to-even) — rounded once,
    /// from the exact integer state, not via an intermediate f64.
    pub fn value(&self) -> f32 {
        let s = &self.inner;
        if s.specials.any() {
            return match (s.specials.nan, s.specials.pos_inf, s.specials.neg_inf) {
                (true, _, _) | (_, true, true) => f32::NAN,
                (_, true, false) => f32::INFINITY,
                (_, false, true) => f32::NEG_INFINITY,
                _ => unreachable!(),
            };
        }
        let negative = s.limbs[LIMBS - 1] >> 63 != 0;
        let mag = if negative { negate(&s.limbs) } else { s.limbs };
        round_mag(&mag, negative, 23, -126, 127) as f32
    }

    /// Canonical byte encoding (see [`SumF64::to_bytes`]).
    #[inline]
    pub fn to_bytes(&self) -> [u8; Self::BYTES] {
        self.inner.to_bytes()
    }

    /// Decode a state produced by [`to_bytes`](Self::to_bytes).
    #[inline]
    pub fn from_bytes(b: &[u8; Self::BYTES]) -> Option<Self> {
        SumF64::from_bytes(b).map(|inner| Self { inner })
    }
}

impl core::iter::FromIterator<f32> for SumF32 {
    fn from_iter<I: IntoIterator<Item = f32>>(iter: I) -> Self {
        let mut acc = Self::new();
        for x in iter {
            acc.add(x);
        }
        acc
    }
}

/// Serde support: both types serialize as their canonical byte encoding —
/// the same bytes as [`SumF64::to_bytes`], so every wire format (JSON,
/// bincode, ...) carries one canonical representation.
#[cfg(feature = "serde")]
mod serde_impls {
    use super::{SumF32, SumF64};
    use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

    impl Serialize for SumF64 {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            s.serialize_bytes(&self.to_bytes())
        }
    }

    struct BytesVisitor;

    impl<'de> de::Visitor<'de> for BytesVisitor {
        type Value = SumF64;

        fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            write!(f, "{} canonical bitrep accumulator bytes", SumF64::BYTES)
        }

        fn visit_bytes<E: de::Error>(self, v: &[u8]) -> Result<SumF64, E> {
            let arr: &[u8; SumF64::BYTES] = v
                .try_into()
                .map_err(|_| E::invalid_length(v.len(), &self))?;
            SumF64::from_bytes(arr).ok_or_else(|| E::custom("invalid bitrep flag byte"))
        }

        fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<SumF64, A::Error> {
            let mut arr = [0u8; SumF64::BYTES];
            for (i, slot) in arr.iter_mut().enumerate() {
                *slot = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(i, &self))?;
            }
            if seq.next_element::<u8>()?.is_some() {
                return Err(de::Error::invalid_length(SumF64::BYTES + 1, &self));
            }
            SumF64::from_bytes(&arr).ok_or_else(|| de::Error::custom("invalid bitrep flag byte"))
        }
    }

    impl<'de> Deserialize<'de> for SumF64 {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            d.deserialize_bytes(BytesVisitor)
        }
    }

    impl Serialize for SumF32 {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            s.serialize_bytes(&self.to_bytes())
        }
    }

    impl<'de> Deserialize<'de> for SumF32 {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            let inner = SumF64::deserialize(d)?;
            Ok(SumF32 { inner })
        }
    }
}

/// Two's-complement negation of the limb array.
fn negate(l: &[u64; LIMBS]) -> [u64; LIMBS] {
    let mut out = [0u64; LIMBS];
    let mut carry = 1u64;
    for i in 0..LIMBS {
        let (v, c) = (!l[i]).overflowing_add(carry);
        out[i] = v;
        carry = c as u64;
    }
    out
}

/// Bit `i` of the magnitude.
#[inline]
fn bit(mag: &[u64; LIMBS], i: usize) -> bool {
    (mag[i >> 6] >> (i & 63)) & 1 != 0
}

/// Highest set bit index, or None if zero.
fn highest_bit(mag: &[u64; LIMBS]) -> Option<usize> {
    for i in (0..LIMBS).rev() {
        if mag[i] != 0 {
            return Some(i * 64 + 63 - mag[i].leading_zeros() as usize);
        }
    }
    None
}

/// Extract `n+1` bits of the magnitude starting at bit `shift` (little-endian
/// weight), i.e. floor(mag / 2^shift) mod 2^(n+1). n+1 <= 64.
fn extract(mag: &[u64; LIMBS], shift: usize, nbits: u32) -> u64 {
    let limb = shift >> 6;
    let off = (shift & 63) as u32;
    let lo = mag[limb] >> off;
    let hi = if off == 0 || limb + 1 >= LIMBS {
        0
    } else {
        mag[limb + 1] << (64 - off)
    };
    let v = lo | hi;
    if nbits == 64 {
        v
    } else {
        v & ((1u64 << nbits) - 1)
    }
}

/// Any set bit strictly below `end`?
fn any_below(mag: &[u64; LIMBS], end: usize) -> bool {
    let limb = end >> 6;
    let off = end & 63;
    for l in mag.iter().take(limb) {
        if *l != 0 {
            return true;
        }
    }
    if off > 0 && limb < LIMBS && mag[limb] & ((1u64 << off) - 1) != 0 {
        return true;
    }
    false
}

/// Round the exact magnitude (units of 2^-1074) to a float with `mant` stored
/// mantissa bits and unbiased exponent range [min_exp, max_exp], returning it
/// as f64 bits-value (for f32 the caller casts; the cast of an exactly
/// f32-representable f64 is exact, so rounding still happens exactly once).
///
/// Round-to-nearest, ties-to-even. Overflow -> ±∞. Exact zero -> +0.0.
fn round_mag(mag: &[u64; LIMBS], negative: bool, mant: u32, min_exp: i32, max_exp: i32) -> f64 {
    let h = match highest_bit(mag) {
        None => return 0.0, // canonical +0.0
        Some(h) => h as i32,
    };
    // value = mag * 2^-1074; unbiased exponent of the leading bit:
    let e = h - BIAS;
    // Highest leading-bit index that still lands in the subnormal range:
    // |v| < 2^min_exp  <=>  h < min_exp + BIAS  <=>  h <= min_exp + BIAS - 1.
    let sub_top = min_exp + BIAS - 1; // f64: 51, f32: 947
    let (q, exp) = if h <= sub_top {
        // Subnormal target: quantum is 2^(min_exp - mant); grid bit index:
        let grid = (min_exp - mant as i32 + BIAS) as usize; // f64: 0 (exact), f32: 925
        (round_at(mag, grid, h as usize), min_exp)
    } else {
        // Normal target: keep mant+1 significant bits; quantum bit index h-mant.
        let grid = (h - mant as i32) as usize;
        (round_at(mag, grid, h as usize), e)
    };
    // q holds the rounded significand; it may have carried out one bit.
    let (q, exp) = if q >> (mant + 1) != 0 {
        (q >> 1, exp + 1)
    } else {
        (q, exp)
    };
    // Re-check subnormal->normal promotion: if we rounded a subnormal up to
    // exactly 2^min_exp, q == 2^mant and exp == min_exp — that is the smallest
    // normal, and the encoding below handles it uniformly.
    if exp > max_exp {
        return if negative {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        };
    }
    let sig = q as f64; // q < 2^(mant+1) <= 2^53: exact
    let v = sig * pow2(exp - mant as i32);
    if negative {
        -v
    } else {
        v
    }
}

/// Round the magnitude at grid bit `grid` (nearest, ties-to-even), returning
/// floor(mag / 2^grid) rounded — the significand in units of 2^grid.
/// `h` is the highest set bit (h >= grid is not required; h < grid yields 0 or 1).
fn round_at(mag: &[u64; LIMBS], grid: usize, h: usize) -> u64 {
    let width = if h >= grid { (h - grid + 1) as u32 } else { 0 };
    debug_assert!(
        width <= 54,
        "significand extraction width {width} exceeds 54 bits"
    );
    let q = if width == 0 {
        0
    } else {
        extract(mag, grid, width)
    };
    if grid == 0 {
        return q; // exact: nothing below the grid
    }
    let round_bit = bit(mag, grid - 1);
    if !round_bit {
        return q;
    }
    let sticky = any_below(mag, grid - 1);
    if sticky || q & 1 == 1 {
        q + 1
    } else {
        q
    }
}

/// 2^k as f64 for k in the finite range we produce (min_exp - mant ..= max_exp).
/// Built from bits to avoid any libm dependency; exact for k >= -1074.
fn pow2(k: i32) -> f64 {
    debug_assert!((-1074..=1023).contains(&k));
    if k >= -1022 {
        f64::from_bits(((k + 1023) as u64) << 52)
    } else {
        // subnormal power of two: single mantissa bit
        f64::from_bits(1u64 << (k + 1074))
    }
}

// ---------------------------------------------------------------------------
// Exact-tier extension (branch: exact-tier): group subtraction.
// The limb accumulator is two's-complement, so finite states form an abelian
// GROUP — subtraction is exact. The special flags (NaN/±inf) are sticky
// booleans (a semilattice, not a group): a state that absorbed a special can
// never attribute it, so subtraction refuses specials-carrying subtrahends.
impl SumF64 {
    /// Exactly remove a previously merged contribution: `self -= other`.
    ///
    /// Returns `false` (leaving `self` untouched) if `other` carries
    /// NaN/infinity flags (non-cancellative) or a larger count than `self`.
    /// On `true`, the state is exactly what it would have been had `other`'s
    /// values never been added — byte-identical.
    pub fn try_unmerge(&mut self, other: &SumF64) -> bool {
        if other.specials.nan || other.specials.pos_inf || other.specials.neg_inf {
            return false;
        }
        if other.count > self.count {
            return false;
        }
        let mut borrow = 0u64;
        for i in 0..LIMBS {
            let (a, b1) = self.limbs[i].overflowing_sub(other.limbs[i]);
            let (b, b2) = a.overflowing_sub(borrow);
            self.limbs[i] = b;
            borrow = (b1 as u64) + (b2 as u64);
        }
        // final borrow wraps the two's-complement representation: correct for
        // signed values by construction.
        self.count -= other.count;
        true
    }
}
