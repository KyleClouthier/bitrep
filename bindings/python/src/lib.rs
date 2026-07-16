// Copyright (c) 2026 Kyle Clouthier / Clouthier Simulation Labs. Licensed under MIT OR Apache-2.0.
//! Python bindings (PyO3) for the full public API of `bitrep`.
//!
//! Every class wraps the real Rust engine, so results are the same exact,
//! order-invariant, bit-identical values as the native crate. Fallible reads
//! raise `ValueError`; byte encodings are Python `bytes`.
// `?` on an error that is already `PyErr` trips clippy::useless_conversion at every
// `from_bytes` site; the pattern is idiomatic PyO3, so allow it crate-wide.
#![allow(clippy::useless_conversion)]
// Python constructors are exposed via `__new__` — a Rust `Default` impl adds nothing.
#![allow(clippy::new_without_default)]

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;

fn ve<E: std::fmt::Debug>(e: E) -> PyErr {
    PyValueError::new_err(format!("{e:?}"))
}
fn none_err() -> PyErr {
    PyValueError::new_err("undefined (insufficient data or invalid input)")
}
fn hash_of<'py, M: bitrep_core::Mergeable>(py: Python<'py>, m: &M) -> Bound<'py, PyBytes> {
    PyBytes::new_bound(py, &bitrep_core::state_hash(m))
}

#[pyclass]
struct SumF64(bitrep_core::SumF64);
#[pymethods]
impl SumF64 {
    #[new]
    fn new() -> Self {
        Self(bitrep_core::SumF64::default())
    }
    fn add(&mut self, x: f64) {
        self.0.add(x);
    }
    fn merge(&mut self, other: &SumF64) {
        self.0.merge(&other.0);
    }
    fn count(&self) -> u64 {
        self.0.count()
    }
    fn value(&self) -> f64 {
        self.0.value()
    }
    fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new_bound(py, &self.0.to_bytes())
    }
    #[staticmethod]
    fn from_bytes(b: &[u8]) -> PyResult<Self> {
        let a: [u8; bitrep_core::SumF64::BYTES] = b
            .try_into()
            .map_err(|_| PyValueError::new_err("wrong byte length"))?;
        bitrep_core::SumF64::from_bytes(&a)
            .map(Self)
            .ok_or_else(none_err)
    }
    fn state_hash<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        hash_of(py, &self.0)
    }
}

#[pyclass]
struct SumF32(bitrep_core::SumF32);
#[pymethods]
impl SumF32 {
    #[new]
    fn new() -> Self {
        Self(bitrep_core::SumF32::default())
    }
    fn add(&mut self, x: f32) {
        self.0.add(x);
    }
    fn merge(&mut self, other: &SumF32) {
        self.0.merge(&other.0);
    }
    fn count(&self) -> u64 {
        self.0.count()
    }
    fn value(&self) -> f32 {
        self.0.value()
    }
    fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new_bound(py, &self.0.to_bytes())
    }
    #[staticmethod]
    fn from_bytes(b: &[u8]) -> PyResult<Self> {
        let a: [u8; bitrep_core::SumF32::BYTES] = b
            .try_into()
            .map_err(|_| PyValueError::new_err("wrong byte length"))?;
        bitrep_core::SumF32::from_bytes(&a)
            .map(Self)
            .ok_or_else(none_err)
    }
    fn state_hash<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        hash_of(py, &self.0)
    }
}

#[pyclass]
struct FastSumF64(bitrep_core::FastSumF64);
#[pymethods]
impl FastSumF64 {
    #[new]
    fn new() -> Self {
        Self(bitrep_core::FastSumF64::default())
    }
    fn add(&mut self, x: f64) {
        self.0.add(x);
    }
    fn extend(&mut self, xs: Vec<f64>) {
        self.0.extend_from_slice(&xs);
    }
    fn count(&self) -> u64 {
        self.0.count()
    }
    fn finish(&self) -> SumF64 {
        SumF64(self.0.finish())
    }
}

#[pyclass]
struct DotF64(bitrep_core::DotF64);
#[pymethods]
impl DotF64 {
    #[new]
    fn new() -> Self {
        Self(bitrep_core::DotF64::default())
    }
    fn push(&mut self, a: f64, b: f64) {
        self.0.push(a, b);
    }
    fn extend(&mut self, xs: Vec<f64>, ys: Vec<f64>) {
        self.0.extend_from_slices(&xs, &ys);
    }
    fn merge(&mut self, other: &DotF64) {
        self.0.merge(&other.0);
    }
    fn value(&self) -> PyResult<f64> {
        self.0.try_value().map_err(ve)
    }
    fn state_hash<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        hash_of(py, &self.0)
    }
}

#[pyfunction]
fn dot(xs: Vec<f64>, ys: Vec<f64>) -> PyResult<f64> {
    bitrep_core::dot(&xs, &ys).map_err(ve)
}

#[pyclass]
struct MomentsF64(bitrep_core::MomentsF64);
#[pymethods]
impl MomentsF64 {
    #[new]
    fn new() -> Self {
        Self(bitrep_core::MomentsF64::new())
    }
    fn add(&mut self, x: f64) {
        self.0.add(x);
    }
    fn merge(&mut self, o: &MomentsF64) {
        self.0.merge(&o.0);
    }
    fn count(&self) -> u64 {
        self.0.count()
    }
    fn mean(&self) -> PyResult<f64> {
        self.0.try_mean().map_err(ve)
    }
    fn variance(&self) -> PyResult<f64> {
        self.0.try_variance().map_err(ve)
    }
    fn sample_variance(&self) -> PyResult<f64> {
        self.0.try_sample_variance().map_err(ve)
    }
    fn stddev(&self) -> PyResult<f64> {
        self.0.try_stddev().map_err(ve)
    }
    fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new_bound(py, &self.0.to_bytes())
    }
    #[staticmethod]
    fn from_bytes(b: &[u8]) -> PyResult<Self> {
        let a: [u8; bitrep_core::MomentsF64::BYTES] = b
            .try_into()
            .map_err(|_| PyValueError::new_err("wrong byte length"))?;
        bitrep_core::MomentsF64::from_bytes(&a)
            .map(Self)
            .ok_or_else(none_err)
    }
    fn state_hash<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        hash_of(py, &self.0)
    }
}

#[pyclass]
struct Moments4F64(bitrep_core::Moments4F64);
#[pymethods]
impl Moments4F64 {
    #[new]
    fn new() -> Self {
        Self(bitrep_core::Moments4F64::new())
    }
    fn add(&mut self, x: f64) {
        self.0.add(x);
    }
    fn merge(&mut self, o: &Moments4F64) {
        self.0.merge(&o.0);
    }
    fn count(&self) -> u64 {
        self.0.count()
    }
    fn mean(&self) -> PyResult<f64> {
        self.0.try_mean().map_err(ve)
    }
    fn variance(&self) -> PyResult<f64> {
        self.0.try_variance().map_err(ve)
    }
    fn kurtosis(&self) -> PyResult<f64> {
        self.0.try_kurtosis().map_err(ve)
    }
    fn excess_kurtosis(&self) -> PyResult<f64> {
        self.0.try_excess_kurtosis().map_err(ve)
    }
    fn skewness(&self) -> PyResult<f64> {
        self.0.try_skewness().map_err(ve)
    }
    fn skewness_squared(&self) -> PyResult<f64> {
        self.0.try_skewness_squared().map_err(ve)
    }
    fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new_bound(py, &self.0.to_bytes())
    }
    #[staticmethod]
    fn from_bytes(b: &[u8]) -> PyResult<Self> {
        let a: [u8; bitrep_core::Moments4F64::BYTES] = b
            .try_into()
            .map_err(|_| PyValueError::new_err("wrong byte length"))?;
        bitrep_core::Moments4F64::from_bytes(&a)
            .map(Self)
            .ok_or_else(none_err)
    }
    fn state_hash<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        hash_of(py, &self.0)
    }
}

#[pyclass]
struct CovF64(bitrep_core::CovF64);
#[pymethods]
impl CovF64 {
    #[new]
    fn new() -> Self {
        Self(bitrep_core::CovF64::new())
    }
    fn add(&mut self, x: f64, y: f64) {
        self.0.add(x, y);
    }
    fn merge(&mut self, o: &CovF64) {
        self.0.merge(&o.0);
    }
    fn count(&self) -> u64 {
        self.0.count()
    }
    fn covariance(&self) -> PyResult<f64> {
        self.0.try_covariance().map_err(ve)
    }
    fn slope(&self) -> PyResult<f64> {
        self.0.try_slope().map_err(ve)
    }
    fn intercept(&self) -> PyResult<f64> {
        self.0.try_intercept().map_err(ve)
    }
    fn r_squared(&self) -> PyResult<f64> {
        self.0.try_r_squared().map_err(ve)
    }
    fn correlation(&self) -> PyResult<f64> {
        self.0.try_correlation().map_err(ve)
    }
    fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new_bound(py, &self.0.to_bytes())
    }
    #[staticmethod]
    fn from_bytes(b: &[u8]) -> PyResult<Self> {
        let a: [u8; bitrep_core::CovF64::BYTES] = b
            .try_into()
            .map_err(|_| PyValueError::new_err("wrong byte length"))?;
        bitrep_core::CovF64::from_bytes(&a)
            .map(Self)
            .ok_or_else(none_err)
    }
    fn state_hash<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        hash_of(py, &self.0)
    }
}

#[pyclass]
struct WeightedMomentsF64(bitrep_core::WeightedMomentsF64);
#[pymethods]
impl WeightedMomentsF64 {
    #[new]
    fn new() -> Self {
        Self(bitrep_core::WeightedMomentsF64::new())
    }
    fn add(&mut self, x: f64, w: f64) {
        self.0.add(x, w);
    }
    fn merge(&mut self, o: &WeightedMomentsF64) {
        self.0.merge(&o.0);
    }
    fn count(&self) -> u64 {
        self.0.count()
    }
    fn mean(&self) -> PyResult<f64> {
        self.0.try_mean().map_err(ve)
    }
    fn variance(&self) -> PyResult<f64> {
        self.0.try_variance().map_err(ve)
    }
    fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new_bound(py, &self.0.to_bytes())
    }
    #[staticmethod]
    fn from_bytes(b: &[u8]) -> PyResult<Self> {
        let a: [u8; bitrep_core::WeightedMomentsF64::BYTES] = b
            .try_into()
            .map_err(|_| PyValueError::new_err("wrong byte length"))?;
        bitrep_core::WeightedMomentsF64::from_bytes(&a)
            .map(Self)
            .ok_or_else(none_err)
    }
    fn state_hash<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        hash_of(py, &self.0)
    }
}

#[pyclass]
struct PnMomentsF64(bitrep_core::PnMomentsF64);
#[pymethods]
impl PnMomentsF64 {
    #[new]
    fn new() -> Self {
        Self(bitrep_core::PnMomentsF64::new())
    }
    fn add(&mut self, x: f64) {
        self.0.add(x);
    }
    fn remove(&mut self, x: f64) {
        self.0.remove(x);
    }
    fn merge(&mut self, o: &PnMomentsF64) {
        self.0.merge(&o.0);
    }
    fn live_count(&self) -> PyResult<u64> {
        self.0.live_count().ok_or_else(none_err)
    }
    fn mean(&self) -> PyResult<f64> {
        self.0.try_mean().map_err(ve)
    }
    fn variance(&self) -> PyResult<f64> {
        self.0.try_variance().map_err(ve)
    }
    fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new_bound(py, &self.0.to_bytes())
    }
    #[staticmethod]
    fn from_bytes(b: &[u8]) -> PyResult<Self> {
        let a: [u8; bitrep_core::PnMomentsF64::BYTES] = b
            .try_into()
            .map_err(|_| PyValueError::new_err("wrong byte length"))?;
        bitrep_core::PnMomentsF64::from_bytes(&a)
            .map(Self)
            .ok_or_else(none_err)
    }
    fn state_hash<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        hash_of(py, &self.0)
    }
}

#[pyclass]
struct CovMatrixF64(bitrep_core::CovMatrixF64);
#[pymethods]
impl CovMatrixF64 {
    #[new]
    fn new(d: usize) -> Self {
        Self(bitrep_core::CovMatrixF64::new(d))
    }
    fn count(&self) -> u64 {
        self.0.count()
    }
    fn add(&mut self, x: Vec<f64>, y: f64) {
        self.0.add(&x, y);
    }
    fn merge(&mut self, o: &CovMatrixF64) {
        self.0.merge(&o.0);
    }
    fn covariance(&self, i: usize, j: usize) -> PyResult<f64> {
        self.0.try_covariance(i, j).map_err(ve)
    }
    fn regression(&self) -> PyResult<Vec<f64>> {
        self.0.try_regression().map_err(ve)
    }
    fn encode<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new_bound(py, &self.0.encode())
    }
    #[staticmethod]
    fn decode(b: &[u8]) -> PyResult<Self> {
        bitrep_core::CovMatrixF64::decode(b)
            .map(Self)
            .ok_or_else(none_err)
    }
    fn state_hash<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        hash_of(py, &self.0)
    }
}

#[pyclass]
struct HistogramF64(bitrep_core::HistogramF64);
#[pymethods]
impl HistogramF64 {
    #[new]
    fn new(edges: Vec<f64>) -> PyResult<Self> {
        bitrep_core::HistogramF64::new(edges)
            .map(Self)
            .ok_or_else(|| PyValueError::new_err("edges must be sorted, finite, len>=2"))
    }
    fn add(&mut self, x: f64) {
        self.0.add(x);
    }
    fn edges(&self) -> Vec<f64> {
        self.0.edges().to_vec()
    }
    fn counts(&self) -> PyResult<Vec<u64>> {
        self.0.counts().map(|c| c.to_vec()).ok_or_else(none_err)
    }
    fn total(&self) -> u64 {
        self.0.total()
    }
    fn quantile_bounds(&self, q: f64) -> PyResult<(f64, f64)> {
        self.0.quantile_bounds(q).ok_or_else(none_err)
    }
    fn encode<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new_bound(py, &self.0.encode_bytes())
    }
    #[staticmethod]
    fn decode(b: &[u8]) -> PyResult<Self> {
        bitrep_core::HistogramF64::decode_bytes(b)
            .map(Self)
            .ok_or_else(none_err)
    }
    fn state_hash<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        hash_of(py, &self.0)
    }
}

#[pyclass]
struct ExtremaF64(bitrep_core::ExtremaF64);
#[pymethods]
impl ExtremaF64 {
    #[new]
    fn new() -> Self {
        Self(bitrep_core::ExtremaF64::default())
    }
    fn add(&mut self, x: f64) {
        self.0.add(x);
    }
    fn min(&self) -> PyResult<f64> {
        self.0.min().ok_or_else(none_err)
    }
    fn max(&self) -> PyResult<f64> {
        self.0.max().ok_or_else(none_err)
    }
    fn range(&self) -> PyResult<f64> {
        self.0.range().ok_or_else(none_err)
    }
    fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new_bound(py, &self.0.to_bytes())
    }
    #[staticmethod]
    fn from_bytes(b: &[u8]) -> PyResult<Self> {
        let a: [u8; bitrep_core::ExtremaF64::BYTES] = b
            .try_into()
            .map_err(|_| PyValueError::new_err("wrong byte length"))?;
        bitrep_core::ExtremaF64::from_bytes(&a)
            .map(Self)
            .ok_or_else(none_err)
    }
    fn state_hash<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        hash_of(py, &self.0)
    }
}

// --------- CRDT layer: concrete instantiations of the generic types --------
#[pyclass]
struct SumMap(bitrep_core::ConvergentMap<String, bitrep_core::SumF64>);
#[pymethods]
impl SumMap {
    #[new]
    fn new() -> Self {
        Self(bitrep_core::ConvergentMap::new())
    }
    fn add(&mut self, key: String, x: f64) {
        self.0.entry_or(key, bitrep_core::SumF64::default).add(x);
    }
    fn value(&self, key: &str) -> PyResult<f64> {
        self.0
            .get(&key.to_string())
            .map(|s| s.value())
            .ok_or_else(none_err)
    }
    fn merge(&mut self, other: &SumMap) {
        self.0.merge(&other.0);
    }
    fn len(&self) -> usize {
        self.0.len()
    }
    fn count(&self) -> u64 {
        self.0.count()
    }
    fn encode<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new_bound(py, &self.0.encode())
    }
}

#[pyclass]
struct MomentsMap(bitrep_core::ConvergentMap<String, bitrep_core::MomentsF64>);
#[pymethods]
impl MomentsMap {
    #[new]
    fn new() -> Self {
        Self(bitrep_core::ConvergentMap::new())
    }
    fn add(&mut self, key: String, x: f64) {
        self.0.entry_or(key, bitrep_core::MomentsF64::new).add(x);
    }
    fn mean(&self, key: &str) -> PyResult<f64> {
        self.0
            .get(&key.to_string())
            .ok_or_else(none_err)?
            .try_mean()
            .map_err(ve)
    }
    fn variance(&self, key: &str) -> PyResult<f64> {
        self.0
            .get(&key.to_string())
            .ok_or_else(none_err)?
            .try_variance()
            .map_err(ve)
    }
    fn merge(&mut self, other: &MomentsMap) {
        self.0.merge(&other.0);
    }
    fn len(&self) -> usize {
        self.0.len()
    }
    fn encode<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new_bound(py, &self.0.encode())
    }
}

#[pyclass]
struct ReplicatedSum(bitrep_core::Replicated<bitrep_core::SumF64>);
#[pymethods]
impl ReplicatedSum {
    #[new]
    fn new() -> Self {
        Self(bitrep_core::Replicated::new())
    }
    fn add(&mut self, replica: u64, x: f64) {
        self.0
            .local_mut(replica, bitrep_core::SumF64::default)
            .add(x);
    }
    fn join(&mut self, other: &ReplicatedSum) {
        self.0.join(&other.0);
    }
    fn value(&self) -> f64 {
        self.0.global(bitrep_core::SumF64::default).value()
    }
    fn encode<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new_bound(py, &self.0.encode())
    }
}

#[pyclass]
struct DeltasSum(bitrep_core::Deltas<bitrep_core::SumF64>);
#[pymethods]
impl DeltasSum {
    #[new]
    fn new() -> Self {
        Self(bitrep_core::Deltas::new(bitrep_core::SumF64::default))
    }
    fn add(&mut self, x: f64) {
        self.0.apply(|m| m.add(x));
    }
    fn value(&self) -> f64 {
        self.0.full().value()
    }
}

#[pymodule]
fn bitrep(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<SumF64>()?;
    m.add_class::<SumF32>()?;
    m.add_class::<FastSumF64>()?;
    m.add_class::<DotF64>()?;
    m.add_class::<MomentsF64>()?;
    m.add_class::<Moments4F64>()?;
    m.add_class::<CovF64>()?;
    m.add_class::<WeightedMomentsF64>()?;
    m.add_class::<PnMomentsF64>()?;
    m.add_class::<CovMatrixF64>()?;
    m.add_class::<HistogramF64>()?;
    m.add_class::<ExtremaF64>()?;
    m.add_class::<SumMap>()?;
    m.add_class::<MomentsMap>()?;
    m.add_class::<ReplicatedSum>()?;
    m.add_class::<DeltasSum>()?;
    m.add_function(wrap_pyfunction!(dot, m)?)?;
    Ok(())
}
