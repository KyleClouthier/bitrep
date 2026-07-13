// Copyright (c) 2026 Kyle Clouthier / Clouthier Simulation Labs. Licensed under MIT OR Apache-2.0.
//! Lattice atoms: exact, order-invariant min/max.
//!
//! Min and max are join-semilattice operations already — exact, idempotent,
//! commutative, associative — so [`ExtremaF64`] needs no accumulator tricks.
//! Ordering uses `f64::total_cmp` (the IEEE total order) so the state is
//! deterministic for every input, including `-0.0` vs `+0.0`.

use crate::Mergeable;

/// Exact, mergeable running minimum and maximum (plus count).
///
/// NaN inputs are tracked as a flag and surfaced by
/// [`min`](Self::min)/[`max`](Self::max) returning `None` (never silently
/// dropped or propagated as a garbage value). `-0.0 < +0.0` in the total
/// order, so bytes are canonical.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExtremaF64 {
    /// IEEE bits of the current min/max; meaningful only when `count > 0`.
    min_bits: u64,
    max_bits: u64,
    nan: bool,
    seen: bool,
    count: u64,
}

impl Default for ExtremaF64 {
    fn default() -> Self {
        Self::new()
    }
}

impl ExtremaF64 {
    /// Serialized size in bytes.
    pub const BYTES: usize = 8 + 8 + 1 + 8;

    /// An empty state.
    pub const fn new() -> Self {
        Self {
            min_bits: 0,
            max_bits: 0,
            nan: false,
            seen: false,
            count: 0,
        }
    }

    /// Add one sample. Order never matters; adding is idempotent in value
    /// (count aside).
    pub fn add(&mut self, x: f64) {
        self.count = self.count.saturating_add(1);
        if x.is_nan() {
            self.nan = true;
            return;
        }
        if !self.seen {
            self.min_bits = x.to_bits();
            self.max_bits = x.to_bits();
            self.seen = true;
            return;
        }
        if x.total_cmp(&f64::from_bits(self.min_bits)).is_lt() {
            self.min_bits = x.to_bits();
        }
        if x.total_cmp(&f64::from_bits(self.max_bits)).is_gt() {
            self.max_bits = x.to_bits();
        }
    }

    /// The minimum, or `None` if empty or a NaN was added.
    pub fn min(&self) -> Option<f64> {
        if self.nan || !self.seen {
            return None;
        }
        Some(f64::from_bits(self.min_bits))
    }

    /// The maximum, or `None` if empty or a NaN was added.
    pub fn max(&self) -> Option<f64> {
        if self.nan || !self.seen {
            return None;
        }
        Some(f64::from_bits(self.max_bits))
    }

    /// `max - min`, or `None` if empty or a NaN was added.
    pub fn range(&self) -> Option<f64> {
        Some(self.max()? - self.min()?)
    }

    /// Number of samples.
    pub const fn count(&self) -> u64 {
        self.count
    }

    /// Canonical byte encoding.
    pub fn to_bytes(&self) -> [u8; Self::BYTES] {
        let mut out = [0u8; Self::BYTES];
        out[..8].copy_from_slice(&self.min_bits.to_le_bytes());
        out[8..16].copy_from_slice(&self.max_bits.to_le_bytes());
        out[16] = (self.nan as u8) | ((self.seen as u8) << 1);
        out[17..].copy_from_slice(&self.count.to_le_bytes());
        out
    }

    /// Decode a state produced by [`to_bytes`](Self::to_bytes). Rejects
    /// non-canonical encodings (unknown flag bits, or garbage min/max bits on
    /// an empty state — which would break merge commutativity; found by
    /// Kani).
    pub fn from_bytes(b: &[u8; Self::BYTES]) -> Option<Self> {
        if b[16] & !0b11 != 0 {
            return None;
        }
        let mut w = [0u8; 8];
        w.copy_from_slice(&b[..8]);
        let min_bits = u64::from_le_bytes(w);
        w.copy_from_slice(&b[8..16]);
        let max_bits = u64::from_le_bytes(w);
        if b[16] & 2 == 0 && (min_bits != 0 || max_bits != 0) {
            return None; // empty state must carry canonical zero bits
        }
        w.copy_from_slice(&b[17..]);
        Some(Self {
            min_bits,
            max_bits,
            nan: b[16] & 1 != 0,
            seen: b[16] & 2 != 0,
            count: u64::from_le_bytes(w),
        })
    }
}

impl Mergeable for ExtremaF64 {
    fn merge(&mut self, other: &Self) {
        self.count = self.count.saturating_add(other.count);
        self.nan |= other.nan;
        if !other.seen {
            return;
        }
        if !self.seen {
            self.min_bits = other.min_bits;
            self.max_bits = other.max_bits;
            self.seen = true;
            return;
        }
        let (omin, omax) = (
            f64::from_bits(other.min_bits),
            f64::from_bits(other.max_bits),
        );
        if omin.total_cmp(&f64::from_bits(self.min_bits)).is_lt() {
            self.min_bits = other.min_bits;
        }
        if omax.total_cmp(&f64::from_bits(self.max_bits)).is_gt() {
            self.max_bits = other.max_bits;
        }
    }
    fn count(&self) -> u64 {
        self.count
    }
    #[cfg(feature = "std")]
    fn encode(&self) -> Vec<u8> {
        self.to_bytes().to_vec()
    }
    #[cfg(feature = "std")]
    fn decode(bytes: &[u8]) -> Option<Self> {
        let arr: &[u8; Self::BYTES] = bytes.try_into().ok()?;
        Self::from_bytes(arr)
    }
}
