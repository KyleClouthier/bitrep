// Copyright (c) 2026 Kyle Clouthier. Licensed under MIT OR Apache-2.0.
//! WebAssembly / JavaScript bindings for the full public API of `bitrep`.
//!
//! Every type here wraps the real Rust engine, so results are the same exact,
//! order-invariant, bit-identical values as the native crate — usable from JS/TS.
//! Fallible reads throw; byte encodings map to `Uint8Array`.
#![allow(non_snake_case)]
// JS constructors are exposed via `new` — a Rust `Default` impl adds nothing to the wasm surface,
// and `is_empty` would be surfaced to JS as a redundant method next to `len`.
#![allow(clippy::new_without_default, clippy::len_without_is_empty)]

use wasm_bindgen::prelude::*;

fn je<E: core::fmt::Debug>(e: E) -> JsError {
    JsError::new(&format!("{e:?}"))
}
fn none() -> JsError {
    JsError::new("undefined (insufficient data or invalid input)")
}

// ---------------------------------------------------------------------------
// macro: the common accumulator shape (new / add / merge / bytes / state_hash)
// ---------------------------------------------------------------------------
macro_rules! bytes_hash {
    ($W:ty, $Inner:ty, $N:expr) => {
        #[wasm_bindgen]
        impl $W {
            pub fn to_bytes(&self) -> Vec<u8> {
                self.0.to_bytes().to_vec()
            }
            pub fn from_bytes(b: &[u8]) -> Result<$W, JsError> {
                let arr: [u8; $N] = b
                    .try_into()
                    .map_err(|_| JsError::new("wrong byte length"))?;
                <$Inner>::from_bytes(&arr).map(Self).ok_or_else(none)
            }
            /// 32-byte SHA-256 of the canonical state (receipts): equal multiset -> equal hash.
            pub fn state_hash(&self) -> Vec<u8> {
                bitrep::state_hash(&self.0).to_vec()
            }
        }
    };
}

// ----------------------------- exact sums ----------------------------------
#[wasm_bindgen]
pub struct SumF64(bitrep::SumF64);
#[wasm_bindgen]
impl SumF64 {
    #[wasm_bindgen(constructor)]
    pub fn new() -> SumF64 {
        Self(bitrep::SumF64::default())
    }
    pub fn add(&mut self, x: f64) {
        self.0.add(x);
    }
    pub fn merge(&mut self, other: &SumF64) {
        self.0.merge(&other.0);
    }
    pub fn value(&self) -> f64 {
        self.0.value()
    }
}
bytes_hash!(SumF64, bitrep::SumF64, bitrep::SumF64::BYTES);

#[wasm_bindgen]
pub struct SumF32(bitrep::SumF32);
#[wasm_bindgen]
impl SumF32 {
    #[wasm_bindgen(constructor)]
    pub fn new() -> SumF32 {
        Self(bitrep::SumF32::default())
    }
    pub fn add(&mut self, x: f32) {
        self.0.add(x);
    }
    pub fn merge(&mut self, other: &SumF32) {
        self.0.merge(&other.0);
    }
    pub fn value(&self) -> f32 {
        self.0.value()
    }
}
bytes_hash!(SumF32, bitrep::SumF32, bitrep::SumF32::BYTES);

// ----------------------------- fast sum ------------------------------------
#[wasm_bindgen]
pub struct FastSumF64(bitrep::FastSumF64);
#[wasm_bindgen]
impl FastSumF64 {
    #[wasm_bindgen(constructor)]
    pub fn new() -> FastSumF64 {
        Self(bitrep::FastSumF64::default())
    }
    pub fn add(&mut self, x: f64) {
        self.0.add(x);
    }
    pub fn extend_from_slice(&mut self, xs: &[f64]) {
        self.0.extend_from_slice(xs);
    }
    /// Fold into an exact, mergeable SumF64.
    pub fn finish(&self) -> SumF64 {
        SumF64(self.0.finish())
    }
}

// ----------------------------- exact dot -----------------------------------
#[wasm_bindgen]
pub struct DotF64(bitrep::DotF64);
#[wasm_bindgen]
impl DotF64 {
    #[wasm_bindgen(constructor)]
    pub fn new() -> DotF64 {
        Self(bitrep::DotF64::default())
    }
    pub fn push(&mut self, a: f64, b: f64) {
        self.0.push(a, b);
    }
    pub fn extend_from_slices(&mut self, xs: &[f64], ys: &[f64]) {
        self.0.extend_from_slices(xs, ys);
    }
    pub fn merge(&mut self, other: &DotF64) {
        self.0.merge(&other.0);
    }
    pub fn value(&self) -> Result<f64, JsError> {
        self.0.try_value().map_err(je)
    }
    pub fn state_hash(&self) -> Vec<u8> {
        bitrep::state_hash(&self.0).to_vec()
    }
}
/// One-shot exact dot product of two equal-length slices.
#[wasm_bindgen]
pub fn dot(xs: &[f64], ys: &[f64]) -> Result<f64, JsError> {
    bitrep::dot(xs, ys).map_err(je)
}

// ----------------------------- moments (mean/var/std) ----------------------
#[wasm_bindgen]
pub struct MomentsF64(bitrep::MomentsF64);
#[wasm_bindgen]
impl MomentsF64 {
    #[wasm_bindgen(constructor)]
    pub fn new() -> MomentsF64 {
        Self(bitrep::MomentsF64::new())
    }
    pub fn add(&mut self, x: f64) {
        self.0.add(x);
    }
    pub fn merge(&mut self, o: &MomentsF64) {
        self.0.merge(&o.0);
    }
    pub fn mean(&self) -> Result<f64, JsError> {
        self.0.try_mean().map_err(je)
    }
    pub fn variance(&self) -> Result<f64, JsError> {
        self.0.try_variance().map_err(je)
    }
    pub fn sample_variance(&self) -> Result<f64, JsError> {
        self.0.try_sample_variance().map_err(je)
    }
    pub fn stddev(&self) -> Result<f64, JsError> {
        self.0.try_stddev().map_err(je)
    }
}
bytes_hash!(MomentsF64, bitrep::MomentsF64, {
    bitrep::MomentsF64::BYTES
});

// ----------------------------- moments incl. skew/kurtosis -----------------
#[wasm_bindgen]
pub struct Moments4F64(bitrep::Moments4F64);
#[wasm_bindgen]
impl Moments4F64 {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Moments4F64 {
        Self(bitrep::Moments4F64::new())
    }
    pub fn add(&mut self, x: f64) {
        self.0.add(x);
    }
    pub fn merge(&mut self, o: &Moments4F64) {
        self.0.merge(&o.0);
    }
    pub fn mean(&self) -> Result<f64, JsError> {
        self.0.try_mean().map_err(je)
    }
    pub fn variance(&self) -> Result<f64, JsError> {
        self.0.try_variance().map_err(je)
    }
    pub fn kurtosis(&self) -> Result<f64, JsError> {
        self.0.try_kurtosis().map_err(je)
    }
    pub fn excess_kurtosis(&self) -> Result<f64, JsError> {
        self.0.try_excess_kurtosis().map_err(je)
    }
    pub fn skewness(&self) -> Result<f64, JsError> {
        self.0.try_skewness().map_err(je)
    }
    pub fn skewness_squared(&self) -> Result<f64, JsError> {
        self.0.try_skewness_squared().map_err(je)
    }
}
bytes_hash!(Moments4F64, bitrep::Moments4F64, {
    bitrep::Moments4F64::BYTES
});

// ----------------------------- pairwise covariance / regression ------------
#[wasm_bindgen]
pub struct CovF64(bitrep::CovF64);
#[wasm_bindgen]
impl CovF64 {
    #[wasm_bindgen(constructor)]
    pub fn new() -> CovF64 {
        Self(bitrep::CovF64::new())
    }
    pub fn add(&mut self, x: f64, y: f64) {
        self.0.add(x, y);
    }
    pub fn merge(&mut self, o: &CovF64) {
        self.0.merge(&o.0);
    }
    pub fn covariance(&self) -> Result<f64, JsError> {
        self.0.try_covariance().map_err(je)
    }
    pub fn slope(&self) -> Result<f64, JsError> {
        self.0.try_slope().map_err(je)
    }
    pub fn intercept(&self) -> Result<f64, JsError> {
        self.0.try_intercept().map_err(je)
    }
    pub fn r_squared(&self) -> Result<f64, JsError> {
        self.0.try_r_squared().map_err(je)
    }
    pub fn correlation(&self) -> Result<f64, JsError> {
        self.0.try_correlation().map_err(je)
    }
}
bytes_hash!(CovF64, bitrep::CovF64, bitrep::CovF64::BYTES);

// ----------------------------- weighted moments ----------------------------
#[wasm_bindgen]
pub struct WeightedMomentsF64(bitrep::WeightedMomentsF64);
#[wasm_bindgen]
impl WeightedMomentsF64 {
    #[wasm_bindgen(constructor)]
    pub fn new() -> WeightedMomentsF64 {
        Self(bitrep::WeightedMomentsF64::new())
    }
    pub fn add(&mut self, x: f64, w: f64) {
        self.0.add(x, w);
    }
    pub fn merge(&mut self, o: &WeightedMomentsF64) {
        self.0.merge(&o.0);
    }
    pub fn mean(&self) -> Result<f64, JsError> {
        self.0.try_mean().map_err(je)
    }
    pub fn variance(&self) -> Result<f64, JsError> {
        self.0.try_variance().map_err(je)
    }
}
bytes_hash!(WeightedMomentsF64, bitrep::WeightedMomentsF64, {
    bitrep::WeightedMomentsF64::BYTES
});

// ----------------------------- positive/negative (retractable) moments -----
#[wasm_bindgen]
pub struct PnMomentsF64(bitrep::PnMomentsF64);
#[wasm_bindgen]
impl PnMomentsF64 {
    #[wasm_bindgen(constructor)]
    pub fn new() -> PnMomentsF64 {
        Self(bitrep::PnMomentsF64::new())
    }
    pub fn add(&mut self, x: f64) {
        self.0.add(x);
    }
    pub fn remove(&mut self, x: f64) {
        self.0.remove(x);
    }
    pub fn merge(&mut self, o: &PnMomentsF64) {
        self.0.merge(&o.0);
    }
    pub fn live_count(&self) -> Result<f64, JsError> {
        self.0.live_count().map(|c| c as f64).ok_or_else(none)
    }
    pub fn mean(&self) -> Result<f64, JsError> {
        self.0.try_mean().map_err(je)
    }
    pub fn variance(&self) -> Result<f64, JsError> {
        self.0.try_variance().map_err(je)
    }
}
bytes_hash!(PnMomentsF64, bitrep::PnMomentsF64, {
    bitrep::PnMomentsF64::BYTES
});

// ----------------------------- covariance matrix + regression --------------
#[wasm_bindgen]
pub struct CovMatrixF64(bitrep::CovMatrixF64);
#[wasm_bindgen]
impl CovMatrixF64 {
    #[wasm_bindgen(constructor)]
    pub fn new(d: usize) -> CovMatrixF64 {
        Self(bitrep::CovMatrixF64::new(d))
    }
    pub fn count(&self) -> f64 {
        self.0.count() as f64
    }
    pub fn add(&mut self, x: &[f64], y: f64) {
        self.0.add(x, y);
    }
    pub fn merge(&mut self, o: &CovMatrixF64) {
        self.0.merge(&o.0);
    }
    pub fn covariance(&self, i: usize, j: usize) -> Result<f64, JsError> {
        self.0.try_covariance(i, j).map_err(je)
    }
    pub fn regression(&self) -> Result<Vec<f64>, JsError> {
        self.0.try_regression().map_err(je)
    }
    pub fn encode(&self) -> Vec<u8> {
        self.0.encode()
    }
    pub fn decode(bytes: &[u8]) -> Result<CovMatrixF64, JsError> {
        bitrep::CovMatrixF64::decode(bytes)
            .map(Self)
            .ok_or_else(none)
    }
    pub fn state_hash(&self) -> Vec<u8> {
        bitrep::state_hash(&self.0).to_vec()
    }
}

// ----------------------------- histogram (exact counts, quantile bounds) ---
#[wasm_bindgen]
pub struct HistogramF64(bitrep::HistogramF64);
#[wasm_bindgen]
impl HistogramF64 {
    #[wasm_bindgen(constructor)]
    pub fn new(edges: Vec<f64>) -> Result<HistogramF64, JsError> {
        bitrep::HistogramF64::new(edges)
            .map(Self)
            .ok_or_else(|| JsError::new("edges must be sorted, finite, len>=2"))
    }
    pub fn add(&mut self, x: f64) {
        self.0.add(x);
    }
    pub fn edges(&self) -> Vec<f64> {
        self.0.edges().to_vec()
    }
    pub fn counts(&self) -> Result<Vec<f64>, JsError> {
        self.0
            .counts()
            .map(|c| c.iter().map(|&n| n as f64).collect())
            .ok_or_else(none)
    }
    pub fn total(&self) -> f64 {
        self.0.total() as f64
    }
    /// [lower, upper] exact bounds on the q-quantile (order stats aren't exactly mergeable).
    pub fn quantile_bounds(&self, q: f64) -> Result<Vec<f64>, JsError> {
        self.0
            .quantile_bounds(q)
            .map(|(lo, hi)| vec![lo, hi])
            .ok_or_else(none)
    }
    pub fn encode(&self) -> Vec<u8> {
        self.0.encode_bytes()
    }
    pub fn decode(bytes: &[u8]) -> Result<HistogramF64, JsError> {
        bitrep::HistogramF64::decode_bytes(bytes)
            .map(Self)
            .ok_or_else(none)
    }
    pub fn state_hash(&self) -> Vec<u8> {
        bitrep::state_hash(&self.0).to_vec()
    }
}

// ----------------------------- extrema (min/max/range) ---------------------
#[wasm_bindgen]
pub struct ExtremaF64(bitrep::ExtremaF64);
#[wasm_bindgen]
impl ExtremaF64 {
    #[wasm_bindgen(constructor)]
    pub fn new() -> ExtremaF64 {
        Self(bitrep::ExtremaF64::default())
    }
    pub fn add(&mut self, x: f64) {
        self.0.add(x);
    }
    pub fn min(&self) -> Result<f64, JsError> {
        self.0.min().ok_or_else(none)
    }
    pub fn max(&self) -> Result<f64, JsError> {
        self.0.max().ok_or_else(none)
    }
    pub fn range(&self) -> Result<f64, JsError> {
        self.0.range().ok_or_else(none)
    }
}
bytes_hash!(ExtremaF64, bitrep::ExtremaF64, {
    bitrep::ExtremaF64::BYTES
});

// ---------------------------------------------------------------------------
// CRDT layer — concrete instantiations of the generic ConvergentMap/Replicated
// (generics can't cross the wasm boundary; these are the useful concretions).
// ---------------------------------------------------------------------------
#[wasm_bindgen]
pub struct SumMap(bitrep::ConvergentMap<String, bitrep::SumF64>);
#[wasm_bindgen]
impl SumMap {
    #[wasm_bindgen(constructor)]
    pub fn new() -> SumMap {
        Self(bitrep::ConvergentMap::new())
    }
    pub fn add(&mut self, key: String, x: f64) {
        self.0.entry_or(key, bitrep::SumF64::default).add(x);
    }
    pub fn value(&self, key: &str) -> Result<f64, JsError> {
        self.0
            .get(&key.to_string())
            .map(|s| s.value())
            .ok_or_else(none)
    }
    pub fn merge(&mut self, other: &SumMap) {
        self.0.merge(&other.0);
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn count(&self) -> f64 {
        self.0.count() as f64
    }
    pub fn encode(&self) -> Vec<u8> {
        self.0.encode()
    }
}

#[wasm_bindgen]
pub struct MomentsMap(bitrep::ConvergentMap<String, bitrep::MomentsF64>);
#[wasm_bindgen]
impl MomentsMap {
    #[wasm_bindgen(constructor)]
    pub fn new() -> MomentsMap {
        Self(bitrep::ConvergentMap::new())
    }
    pub fn add(&mut self, key: String, x: f64) {
        self.0.entry_or(key, bitrep::MomentsF64::new).add(x);
    }
    pub fn mean(&self, key: &str) -> Result<f64, JsError> {
        self.0
            .get(&key.to_string())
            .ok_or_else(none)?
            .try_mean()
            .map_err(je)
    }
    pub fn variance(&self, key: &str) -> Result<f64, JsError> {
        self.0
            .get(&key.to_string())
            .ok_or_else(none)?
            .try_variance()
            .map_err(je)
    }
    pub fn merge(&mut self, other: &MomentsMap) {
        self.0.merge(&other.0);
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn encode(&self) -> Vec<u8> {
        self.0.encode()
    }
}

#[wasm_bindgen]
pub struct ReplicatedSum(bitrep::Replicated<bitrep::SumF64>);
#[wasm_bindgen]
impl ReplicatedSum {
    #[wasm_bindgen(constructor)]
    pub fn new() -> ReplicatedSum {
        Self(bitrep::Replicated::new())
    }
    pub fn add(&mut self, replica: u64, x: f64) {
        self.0.local_mut(replica, bitrep::SumF64::default).add(x);
    }
    pub fn join(&mut self, other: &ReplicatedSum) {
        self.0.join(&other.0);
    }
    pub fn value(&self) -> f64 {
        self.0.global(bitrep::SumF64::default).value()
    }
    pub fn encode(&self) -> Vec<u8> {
        self.0.encode()
    }
}

#[wasm_bindgen]
pub struct DeltasSum(bitrep::Deltas<bitrep::SumF64>);
#[wasm_bindgen]
impl DeltasSum {
    #[wasm_bindgen(constructor)]
    pub fn new() -> DeltasSum {
        Self(bitrep::Deltas::new(bitrep::SumF64::default))
    }
    pub fn add(&mut self, x: f64) {
        self.0.apply(|m| m.add(x));
    }
    pub fn value(&self) -> f64 {
        self.0.full().value()
    }
}
