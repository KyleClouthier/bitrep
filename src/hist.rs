// Copyright (c) 2026 Kyle Clouthier. Licensed under MIT OR Apache-2.0.
//! Fixed-bucket histograms: exact counts, honest quantile *bounds*.
//!
//! Order statistics (median, quantiles) are outside the exact-monomial
//! algebra — no exact mergeable representation exists. The honest primitive
//! is a histogram with **fixed bucket boundaries**: bucket counts are
//! integers (exact, order-invariant, mergeable), and a quantile query
//! returns the *bucket interval* that provably contains it — a deterministic
//! bound with stated resolution, not a point estimate dressed up as one.

use crate::Mergeable;

/// An exact, mergeable fixed-bucket histogram.
///
/// Construction fixes the bucket edges; values below the first edge land in
/// an underflow bucket, values at/above the last edge in an overflow bucket
/// (`edges.len() + 1` buckets total). Merging requires identical edges — a
/// mismatch poisons the state and is reported, never silently blended.
/// NaN inputs are counted in a dedicated flag, never dropped.
#[derive(Clone, PartialEq, Debug)]
pub struct HistogramF64 {
    edges: Vec<f64>,
    counts: Vec<u64>,
    nan_count: u64,
    mismatched: bool,
}

impl HistogramF64 {
    /// A histogram with the given ascending, finite bucket edges.
    ///
    /// Returns `None` if `edges` is empty, non-ascending (ties included), or
    /// contains non-finite values.
    pub fn new(edges: Vec<f64>) -> Option<Self> {
        if edges.is_empty() || edges.iter().any(|e| !e.is_finite()) {
            return None;
        }
        if edges.windows(2).any(|w| w[0].total_cmp(&w[1]).is_ge()) {
            return None;
        }
        let buckets = edges.len() + 1;
        Some(Self {
            edges,
            counts: vec![0; buckets],
            nan_count: 0,
            mismatched: false,
        })
    }

    /// Add one sample. Order never matters.
    pub fn add(&mut self, x: f64) {
        if x.is_nan() {
            self.nan_count = self.nan_count.saturating_add(1);
            return;
        }
        // first edge greater than x -> bucket index (underflow = 0)
        let idx = self.edges.partition_point(|e| e.total_cmp(&x).is_le());
        self.counts[idx] = self.counts[idx].saturating_add(1);
    }

    /// The bucket edges.
    pub fn edges(&self) -> &[f64] {
        &self.edges
    }

    /// The bucket counts (underflow first, overflow last), or `None` if the
    /// state was poisoned by a mismatched merge.
    pub fn counts(&self) -> Option<&[u64]> {
        if self.mismatched {
            return None;
        }
        Some(&self.counts)
    }

    /// NaN samples seen (tracked, never silently dropped).
    pub const fn nan_count(&self) -> u64 {
        self.nan_count
    }

    /// Total non-NaN samples.
    pub fn total(&self) -> u64 {
        self.counts.iter().fold(0u64, |a, &c| a.saturating_add(c))
    }

    /// The interval `(lower, upper)` that provably contains the q-quantile
    /// (`0 <= q <= 1`) of the non-NaN samples, at bucket resolution.
    /// `None` if empty, poisoned, or the quantile falls in an unbounded
    /// (under/overflow) bucket — stated limits, not guesses.
    pub fn quantile_bounds(&self, q: f64) -> Option<(f64, f64)> {
        if self.mismatched || !(0.0..=1.0).contains(&q) {
            return None;
        }
        let total = self.total();
        if total == 0 {
            return None;
        }
        // rank in 1..=total (nearest-rank definition: deterministic)
        let rank = ((q * total as f64).ceil() as u64).clamp(1, total);
        let mut seen = 0u64;
        for (i, &c) in self.counts.iter().enumerate() {
            seen = seen.saturating_add(c);
            if seen >= rank {
                if i == 0 || i == self.edges.len() {
                    return None; // unbounded bucket: no honest finite bound
                }
                return Some((self.edges[i - 1], self.edges[i]));
            }
        }
        None
    }

    /// Canonical byte encoding.
    pub fn encode_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(self.edges.len() as u32).to_le_bytes());
        out.push(self.mismatched as u8);
        for e in &self.edges {
            out.extend_from_slice(&e.to_bits().to_le_bytes());
        }
        for c in &self.counts {
            out.extend_from_slice(&c.to_le_bytes());
        }
        out.extend_from_slice(&self.nan_count.to_le_bytes());
        out
    }

    /// Decode a canonical encoding produced by
    /// [`encode_bytes`](Self::encode_bytes).
    pub fn decode_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 5 {
            return None;
        }
        let mut w4 = [0u8; 4];
        w4.copy_from_slice(&bytes[..4]);
        let n_edges = u32::from_le_bytes(w4) as usize;
        let mismatched = match bytes[4] {
            0 => false,
            1 => true,
            _ => return None,
        };
        // fully checked size arithmetic: a hostile length prefix must be
        // rejected, never allowed to overflow (32-bit targets included).
        let need = 5usize
            .checked_add(n_edges.checked_mul(8)?)?
            .checked_add(n_edges.checked_add(1)?.checked_mul(8)?)?
            .checked_add(8)?;
        if bytes.len() != need || n_edges == 0 {
            return None;
        }
        let mut at = 5;
        let mut w8 = [0u8; 8];
        let mut edges = Vec::with_capacity(n_edges);
        for _ in 0..n_edges {
            w8.copy_from_slice(&bytes[at..at + 8]);
            edges.push(f64::from_bits(u64::from_le_bytes(w8)));
            at += 8;
        }
        let mut counts = Vec::with_capacity(n_edges + 1);
        for _ in 0..=n_edges {
            w8.copy_from_slice(&bytes[at..at + 8]);
            counts.push(u64::from_le_bytes(w8));
            at += 8;
        }
        w8.copy_from_slice(&bytes[at..at + 8]);
        Some(Self {
            edges,
            counts,
            nan_count: u64::from_le_bytes(w8),
            mismatched,
        })
    }
}

impl Mergeable for HistogramF64 {
    fn merge(&mut self, other: &Self) {
        if self.edges.len() != other.edges.len()
            || self
                .edges
                .iter()
                .zip(&other.edges)
                .any(|(a, b)| a.to_bits() != b.to_bits())
        {
            self.mismatched = true;
            return;
        }
        self.mismatched |= other.mismatched;
        for (a, b) in self.counts.iter_mut().zip(&other.counts) {
            *a = a.saturating_add(*b);
        }
        self.nan_count = self.nan_count.saturating_add(other.nan_count);
    }
    fn count(&self) -> u64 {
        self.total().saturating_add(self.nan_count)
    }
    fn encode(&self) -> Vec<u8> {
        self.encode_bytes()
    }
    fn decode(bytes: &[u8]) -> Option<Self> {
        Self::decode_bytes(bytes)
    }
}
