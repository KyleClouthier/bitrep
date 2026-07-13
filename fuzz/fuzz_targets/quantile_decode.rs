// Copyright (c) 2026 Kyle Clouthier / Clouthier Simulation Labs. Licensed under MIT OR Apache-2.0.
//! Fuzz the RelSketch decoder (the delta-varint bucket parser — the real
//! attack surface). Arbitrary bytes either decode to a state that re-encodes
//! byte-for-byte (canonical form is unique) or are cleanly rejected; a decoded
//! state must survive reads and a self-merge without panicking or wrapping.
//! Mirrors the toolkit_decoders target.
#![no_main]

use bitrep::{Mergeable, RelSketch};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Some(state) = RelSketch::from_bytes(data) {
        // Canonical form is unique: an accepted encoding must round-trip.
        assert_eq!(state.to_bytes(), data, "decode/encode must round-trip exactly");

        // Reads never panic on any decodable state.
        for &q in &[0.0, 0.5, 0.99, 1.0] {
            let _ = state.quantile(q);
        }
        let _ = state.min();
        let _ = state.max();
        let _ = state.count();
        let _ = state.bucket_count();
        let _ = state.guaranteed_alpha();
        let _ = state.otel_positive_indices();

        // Self-merge must never panic or wrap the state.
        let mut m = state.clone();
        m.merge(&state);
        let _ = m.count();
        let _ = m.quantile(0.5);
        // The merged state is itself canonical.
        assert_eq!(RelSketch::from_bytes(&m.to_bytes()), Some(m));
    }
});
