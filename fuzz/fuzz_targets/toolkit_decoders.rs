// Copyright (c) 2026 Kyle Clouthier / Clouthier Simulation Labs. Licensed under MIT OR Apache-2.0.
//! Fuzz the v0.2 toolkit decoders — the variable-length parsers
//! (HistogramF64, CovMatrixF64) and the fixed-size stats codecs. Arbitrary
//! bytes either decode to a state that re-encodes identically, or are
//! rejected; decoded states must survive reads and self-merge without
//! panicking.
#![no_main]

use bitrep::Mergeable;
use libfuzzer_sys::fuzz_target;

fn check<M: Mergeable>(data: &[u8]) {
    if let Some(state) = M::decode(data) {
        assert_eq!(state.encode(), data, "decode/encode must round-trip exactly");
        let mut m = state.clone();
        m.merge(&state); // self-merge must never panic
        let _ = m.count();
    }
}

fuzz_target!(|data: &[u8]| {
    // variable-length parsers (the real attack surface)
    check::<bitrep::HistogramF64>(data);
    check::<bitrep::CovMatrixF64>(data);
    // fixed-size stats codecs
    check::<bitrep::MomentsF64>(data);
    check::<bitrep::Moments4F64>(data);
    check::<bitrep::CovF64>(data);
    check::<bitrep::WeightedMomentsF64>(data);
    check::<bitrep::PnMomentsF64>(data);
    check::<bitrep::ExtremaF64>(data);
    // reads must never panic on any decodable state
    if let Some(m) = bitrep::MomentsF64::decode(data) {
        let _ = m.mean();
        let _ = m.variance();
    }
    if let Some(h) = bitrep::HistogramF64::decode(data) {
        let _ = h.quantile_bounds(0.5);
        let _ = h.total();
    }
    if let Some(c) = bitrep::CovMatrixF64::decode(data) {
        if c.dim() > 0 && c.dim() <= 8 {
            let _ = c.try_covariance(0, 0);
            let _ = c.try_regression();
        }
    }
});
