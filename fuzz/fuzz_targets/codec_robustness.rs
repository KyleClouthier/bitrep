// Copyright (c) 2026 Kyle Clouthier / Clouthier Simulation Labs. Licensed under MIT OR Apache-2.0.
//! Fuzz the byte codec: arbitrary bytes either decode to a state that
//! re-encodes to the same bytes, or are rejected — never panic, never mangle.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() < bitrep::SumF64::BYTES {
        return;
    }
    let arr: &[u8; bitrep::SumF64::BYTES] = data[..bitrep::SumF64::BYTES].try_into().unwrap();
    if let Some(acc) = bitrep::SumF64::from_bytes(arr) {
        assert_eq!(&acc.to_bytes(), arr, "decode/encode must round-trip exactly");
        let _ = acc.value(); // rounding must never panic on any valid state
        let mut m = acc.clone();
        m.merge(&acc); // self-merge must never panic
        let _ = m.value();
    }
});
