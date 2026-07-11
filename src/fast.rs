// Copyright (c) 2026 Kyle Clouthier. Licensed under MIT OR Apache-2.0.
//! High-throughput streaming front-end producing canonical [`SumF64`] state.
//!
//! [`FastSumF64`] is Neal's small-accumulator technique (arXiv:1505.05571,
//! the same algorithm family as the `xsum` crate): a bank of signed 64-bit
//! chunks at 32-bit stride over the 2176-bit fixed-point number line. One add
//! splits the shifted mantissa at a 32-bit boundary and touches exactly
//! **two** chunks — no carry propagation. The largest per-add contribution is
//! 2⁵³, so chunks are folded into the canonical limb representation every 512
//! adds (2⁵³·2⁹ = 2⁶² < 2⁶³: overflow is unreachable), and at
//! [`finish`](FastSumF64::finish). Batch feeding additionally interleaves
//! four independent banks to break the store-to-load dependency chain on
//! narrow-magnitude data — exactness makes any such sharding produce
//! identical final bytes.
//!
//! The contract: for any input sequence, `FastSumF64` produces **the same
//! canonical bytes** as feeding [`SumF64::add`] directly (verified by
//! differential tests over finite values, specials, subnormals and
//! cancellation-heavy streams). The proven core type is untouched — this is
//! a feeder, not a fork; `finish` goes through the public byte codec.

use crate::SumF64;

const LIMBS: usize = 34;
const CHUNKS: usize = 66; // 32-bit stride windows covering bits 0..=2112
/// Fold after this many adds: per-add chunk contribution is < 2^53, so at
/// the threshold |chunk| < 2^53 · 2^9 = 2^62 — i64 overflow is unreachable.
const FOLD_EVERY: u64 = 512;

/// One small-accumulator bank: 66 signed chunks plus special flags.
#[derive(Clone)]
struct Bank {
    chunks: [i64; CHUNKS],
    nan: bool,
    pos_inf: bool,
    neg_inf: bool,
}

impl Bank {
    const fn new() -> Self {
        Self {
            chunks: [0; CHUNKS],
            nan: false,
            pos_inf: false,
            neg_inf: false,
        }
    }

    /// The two-chunk hot path; fold budgeting is the caller's job.
    #[inline(always)]
    fn add_raw(&mut self, x: f64) {
        let bits = x.to_bits();
        let expf = ((bits >> 52) & 0x7ff) as i32;
        let frac = bits & ((1u64 << 52) - 1);

        if expf == 0x7ff {
            if frac != 0 {
                self.nan = true;
            } else if bits >> 63 != 0 {
                self.neg_inf = true;
            } else {
                self.pos_inf = true;
            }
            return;
        }
        let (m, e) = if expf == 0 {
            if frac == 0 {
                return; // ±0.0 contributes nothing
            }
            (frac, -1074i32)
        } else {
            (frac | (1u64 << 52), expf - 1075)
        };
        let pos = (e + 1074) as usize; // bit position of m's LSB: 0..=2045
        let w = pos >> 5;
        let off = pos & 31;
        // split m << off (<= 85 bits) at the 32-bit boundary: exactly two
        // chunk updates — lo < 2^32 into w, hi < 2^53 into w+1.
        let wide = (m as u128) << off;
        let lo = (wide & 0xFFFF_FFFF) as i64;
        let hi = (wide >> 32) as i64;
        if bits >> 63 != 0 {
            self.chunks[w] -= lo;
            self.chunks[w + 1] -= hi;
        } else {
            self.chunks[w] += lo;
            self.chunks[w + 1] += hi;
        }
    }

    /// Fold the chunk bank into canonical limbs and clear it.
    fn fold_into(&mut self, folded: &mut [u64; LIMBS]) {
        for w in 0..CHUNKS {
            let c = self.chunks[w];
            if c == 0 {
                continue;
            }
            self.chunks[w] = 0;
            let limb = w >> 1;
            let off = (w & 1) * 32;
            let mag = (c.unsigned_abs() as u128) << off; // <= 95 bits
            let lo = mag as u64;
            let hi = (mag >> 64) as u64;
            if c > 0 {
                add_at(folded, limb, lo, hi);
            } else {
                sub_at(folded, limb, lo, hi);
            }
        }
    }
}

/// A fast streaming accumulator that finishes into a canonical [`SumF64`].
///
/// ```
/// use bitrep::{FastSumF64, SumF64};
///
/// let data = [1e100_f64, 0.5, -1e100, 2.5e-300];
/// let mut fast = FastSumF64::new();
/// fast.extend_from_slice(&data);
/// let mut slow = SumF64::new();
/// for x in data {
///     slow.add(x);
/// }
/// assert_eq!(fast.finish().to_bytes(), slow.to_bytes());
/// ```
#[derive(Clone)]
pub struct FastSumF64 {
    bank: Bank,
    count: u64,
    since_fold: u64,
    /// Canonical limbs holding everything folded so far (two's complement).
    folded: [u64; LIMBS],
}

impl Default for FastSumF64 {
    fn default() -> Self {
        Self::new()
    }
}

impl FastSumF64 {
    /// An empty accumulator.
    pub const fn new() -> Self {
        Self {
            bank: Bank::new(),
            count: 0,
            since_fold: 0,
            folded: [0u64; LIMBS],
        }
    }

    /// Add one value. Order never matters; the finished bytes are identical
    /// to feeding [`SumF64::add`] in any order.
    #[inline]
    pub fn add(&mut self, x: f64) {
        self.count = self.count.saturating_add(1);
        self.bank.add_raw(x);
        self.since_fold += 1;
        if self.since_fold >= FOLD_EVERY {
            self.bank.fold_into(&mut self.folded);
            self.since_fold = 0;
        }
    }

    /// Add every element of a slice — the fast path.
    ///
    /// Inputs that fit in L1 use an interleaved single pass over four
    /// independent banks. Larger inputs switch to two-pass blocking: pass 1
    /// streams the input and decomposes each value to `(window, lo, hi)`
    /// into stack buffers (no value-dependent store addresses, so loads
    /// pipeline freely even from L2/RAM); pass 2 scatters with now-known
    /// addresses. Exactness makes every such regrouping produce identical
    /// final bytes.
    pub fn extend_from_slice(&mut self, xs: &[f64]) {
        if xs.len() <= 4096 {
            self.extend_small(xs);
            return;
        }
        const BLOCK: usize = 512; // per-bank adds per block = 128 ≤ FOLD_EVERY
        self.count = self.count.saturating_add(xs.len() as u64);
        let mut b1 = Bank::new();
        let mut b2 = Bank::new();
        let mut b3 = Bank::new();
        let mut w_buf = [0u16; BLOCK];
        let mut lo_buf = [0i64; BLOCK];
        let mut hi_buf = [0i64; BLOCK];
        for block in xs.chunks(BLOCK) {
            // pass 1: decompose (streaming loads, sequential stack stores)
            let mut k = 0usize;
            for &x in block {
                let bits = x.to_bits();
                let expf = ((bits >> 52) & 0x7ff) as i32;
                let frac = bits & ((1u64 << 52) - 1);
                if expf == 0x7ff {
                    if frac != 0 {
                        self.bank.nan = true;
                    } else if bits >> 63 != 0 {
                        self.bank.neg_inf = true;
                    } else {
                        self.bank.pos_inf = true;
                    }
                    continue;
                }
                let (m, e) = if expf == 0 {
                    if frac == 0 {
                        continue; // ±0.0 contributes nothing
                    }
                    (frac, -1074i32)
                } else {
                    (frac | (1u64 << 52), expf - 1075)
                };
                let pos = (e + 1074) as usize;
                let wide = (m as u128) << (pos & 31);
                let (lo, hi) = ((wide & 0xFFFF_FFFF) as i64, (wide >> 32) as i64);
                let neg = -((bits >> 63) as i64); // 0 or -1
                w_buf[k] = (pos >> 5) as u16;
                lo_buf[k] = (lo ^ neg) - neg; // conditional negate, branch-free
                hi_buf[k] = (hi ^ neg) - neg;
                k += 1;
            }
            // pass 2: scatter with known addresses, 4 independent banks
            let banks: [&mut Bank; 4] = [&mut self.bank, &mut b1, &mut b2, &mut b3];
            for (i, b) in banks.into_iter().enumerate() {
                let mut j = i;
                while j < k {
                    let w = w_buf[j] as usize;
                    b.chunks[w] += lo_buf[j];
                    b.chunks[w + 1] += hi_buf[j];
                    j += 4;
                }
            }
            // fold all banks every block: 128 adds/bank ≤ FOLD_EVERY budget
            self.bank.fold_into(&mut self.folded);
            b1.fold_into(&mut self.folded);
            b2.fold_into(&mut self.folded);
            b3.fold_into(&mut self.folded);
        }
        self.since_fold = 0;
    }

    /// Single-pass interleaved feeding for L1-resident inputs.
    fn extend_small(&mut self, xs: &[f64]) {
        self.count = self.count.saturating_add(xs.len() as u64);
        let mut b1 = Bank::new();
        let mut b2 = Bank::new();
        let mut b3 = Bank::new();
        let mut rem = xs;
        while !rem.is_empty() {
            let take = rem.len().min(4 * FOLD_EVERY as usize);
            let mut it = rem[..take].chunks_exact(4);
            for q in it.by_ref() {
                self.bank.add_raw(q[0]);
                b1.add_raw(q[1]);
                b2.add_raw(q[2]);
                b3.add_raw(q[3]);
            }
            for &x in it.remainder() {
                self.bank.add_raw(x);
            }
            self.bank.fold_into(&mut self.folded);
            b1.fold_into(&mut self.folded);
            b2.fold_into(&mut self.folded);
            b3.fold_into(&mut self.folded);
            rem = &rem[take..];
        }
        self.since_fold = 0;
        self.bank.nan |= b1.nan | b2.nan | b3.nan;
        self.bank.pos_inf |= b1.pos_inf | b2.pos_inf | b3.pos_inf;
        self.bank.neg_inf |= b1.neg_inf | b2.neg_inf | b3.neg_inf;
    }

    /// Number of values added so far.
    pub const fn count(&self) -> u64 {
        self.count
    }

    /// Finish into a canonical [`SumF64`] — same bytes as the direct path.
    pub fn finish(&self) -> SumF64 {
        let mut me = self.clone();
        me.bank.fold_into(&mut me.folded);
        let mut bytes = [0u8; SumF64::BYTES];
        for (i, l) in me.folded.iter().enumerate() {
            bytes[i * 8..i * 8 + 8].copy_from_slice(&l.to_le_bytes());
        }
        bytes[LIMBS * 8] =
            (me.bank.nan as u8) | ((me.bank.pos_inf as u8) << 1) | ((me.bank.neg_inf as u8) << 2);
        bytes[LIMBS * 8 + 1..].copy_from_slice(&me.count.to_le_bytes());
        // The flag byte above only ever sets the three known bits, so the
        // decode cannot fail; the fallback is unreachable but keeps this
        // panic-free under `deny(unwrap_used)`.
        SumF64::from_bytes(&bytes).unwrap_or_default()
    }
}

impl core::iter::FromIterator<f64> for FastSumF64 {
    fn from_iter<I: IntoIterator<Item = f64>>(iter: I) -> Self {
        let mut a = FastSumF64::new();
        for x in iter {
            a.add(x);
        }
        a
    }
}

/// `dst += (lo, hi) << (64*limb)` over the full two's-complement width,
/// wrapping at the top (same modular arithmetic as `SumF64::merge`).
#[inline]
fn add_at(dst: &mut [u64; LIMBS], limb: usize, lo: u64, hi: u64) {
    let (v, mut carry) = dst[limb].overflowing_add(lo);
    dst[limb] = v;
    let mut i = limb + 1;
    if i < LIMBS {
        let (v, c1) = dst[i].overflowing_add(hi);
        let (v, c2) = v.overflowing_add(carry as u64);
        dst[i] = v;
        carry = c1 || c2;
        i += 1;
    }
    while carry && i < LIMBS {
        let (v, c) = dst[i].overflowing_add(1);
        dst[i] = v;
        carry = c;
        i += 1;
    }
}

/// `dst -= (lo, hi) << (64*limb)`, two's complement with borrow ripple.
#[inline]
fn sub_at(dst: &mut [u64; LIMBS], limb: usize, lo: u64, hi: u64) {
    let (v, mut borrow) = dst[limb].overflowing_sub(lo);
    dst[limb] = v;
    let mut i = limb + 1;
    if i < LIMBS {
        let (v, b1) = dst[i].overflowing_sub(hi);
        let (v, b2) = v.overflowing_sub(borrow as u64);
        dst[i] = v;
        borrow = b1 || b2;
        i += 1;
    }
    while borrow && i < LIMBS {
        let (v, b) = dst[i].overflowing_sub(1);
        dst[i] = v;
        borrow = b;
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::vec::Vec;

    fn slow(xs: &[f64]) -> [u8; SumF64::BYTES] {
        let mut s = SumF64::new();
        for &x in xs {
            s.add(x);
        }
        s.to_bytes()
    }

    fn fast(xs: &[f64]) -> [u8; SumF64::BYTES] {
        let mut f = FastSumF64::new();
        f.extend_from_slice(xs);
        f.finish().to_bytes()
    }

    #[test]
    fn matches_slow_on_edges() {
        let cases: &[&[f64]] = &[
            &[],
            &[0.0, -0.0],
            &[1.0, 2.0, 3.0],
            &[f64::MAX, f64::MAX, -f64::MAX],
            &[f64::MIN_POSITIVE, -f64::MIN_POSITIVE],
            &[5e-324, 5e-324, -5e-324], // subnormals
            &[1e100, 0.5, -1e100, 2.5e-300],
            &[f64::INFINITY, 1.0],
            &[f64::NEG_INFINITY, f64::INFINITY],
            &[f64::NAN],
            &[1e308, 1e308, 1e308, -1e308, -1e308, -1e308],
        ];
        for xs in cases {
            assert_eq!(fast(xs), slow(xs), "mismatch on {xs:?}");
        }
    }

    #[test]
    fn matches_slow_on_mixed_magnitude_stream() {
        // deterministic xorshift; magnitudes spanning the whole exponent range
        let mut s = 0x1234_5678_9ABC_DEF0u64;
        let mut xs = Vec::new();
        for _ in 0..200_000 {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let exp = (s % 2000) as i32 - 1000;
            let mant = 1.0 + (s >> 40) as f64 / (1u64 << 24) as f64;
            let v = mant * 2f64.powi(exp);
            xs.push(if s & 1 == 0 { v } else { -v });
        }
        assert_eq!(fast(&xs), slow(&xs));
    }

    #[test]
    fn single_add_path_matches_batch_path() {
        // 64-bit arithmetic explicitly: `usize` is 32-bit on wasm32 and this
        // product would overflow there.
        let xs: Vec<f64> = (0..40_000u64)
            .map(|i| (i.wrapping_mul(2_654_435_761) % 9973) as f64 * 1e-4 - 0.5)
            .collect();
        let mut one = FastSumF64::new();
        for &x in &xs {
            one.add(x);
        }
        assert_eq!(one.finish().to_bytes(), fast(&xs));
    }

    #[test]
    fn mixed_single_and_batch_feeding_matches() {
        let xs: Vec<f64> = (0..9_999)
            .map(|i| (((i * 31) % 997) as f64 - 498.0) * 1e-2)
            .collect();
        let mut f = FastSumF64::new();
        f.add(xs[0]);
        f.extend_from_slice(&xs[1..5_000]);
        f.add(xs[5_000]);
        f.extend_from_slice(&xs[5_001..]);
        assert_eq!(f.finish().to_bytes(), slow(&xs));
    }

    #[test]
    fn fold_threshold_and_mid_stream_fold() {
        let mut f = FastSumF64::new();
        let mut s = SumF64::new();
        for i in 0..1_000_000u64 {
            let x = ((i % 1023) as f64 - 511.0) * 1e-3;
            f.add(x);
            s.add(x);
        }
        for i in 0..1_000u64 {
            let x = (i as f64) * 1e300;
            f.add(x);
            s.add(x);
        }
        assert_eq!(f.finish().to_bytes(), s.to_bytes());
    }

    #[test]
    fn merge_of_finished_shards_matches_single_pass() {
        let xs: Vec<f64> = (0..10_000)
            .map(|i| ((i * 37) % 101) as f64 * 1e-5 - 5e-3)
            .collect();
        let mut whole = FastSumF64::new();
        whole.extend_from_slice(&xs);
        let (a, b) = xs.split_at(3_333);
        let mut fa = FastSumF64::new();
        fa.extend_from_slice(a);
        let mut fb = FastSumF64::new();
        fb.extend_from_slice(b);
        let mut merged = fa.finish();
        merged.merge(&fb.finish());
        assert_eq!(merged.to_bytes(), whole.finish().to_bytes());
    }
}
