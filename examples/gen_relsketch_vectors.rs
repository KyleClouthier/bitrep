// Copyright (c) 2026 Kyle Clouthier / Clouthier Simulation Labs. Licensed under MIT OR Apache-2.0.
//! Generates `conformance/relsketch_vectors.json` — cross-language conformance
//! vectors for [`RelSketch`]. Each case lists the input f64 bit patterns, the
//! `sub_bits`, the canonical `to_bytes()` encoding and its SHA-256
//! `state_hash`, so a second implementation in a second language can reproduce
//! them byte-for-byte from the format alone.
//!
//! Run: `cargo run --features "quantile receipts" --example gen_relsketch_vectors`

use bitrep::{state_hash, RelSketch};
use std::fmt::Write as _;

fn hex(b: &[u8]) -> String {
    b.iter().fold(String::new(), |mut s, x| {
        let _ = write!(s, "{x:02x}");
        s
    })
}

struct Case {
    name: &'static str,
    sub_bits: u8,
    inputs: Vec<f64>,
}

fn main() {
    let cases = [
        Case {
            name: "basic_positive",
            sub_bits: 6,
            inputs: vec![1.0, 2.0, 3.0, 3.0, 5.0, 8.0, 13.0, 21.0, 100.0, 1000.5],
        },
        Case {
            name: "mixed_signs_and_specials",
            sub_bits: 6,
            inputs: vec![
                1.0,
                -1.0,
                2.5,
                -2.5,
                0.0,
                -0.0,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NAN,
                7.0,
                -7.0,
                7.0,
            ],
        },
        Case {
            name: "subnormals_and_extremes",
            sub_bits: 9,
            inputs: vec![
                f64::MIN_POSITIVE,
                f64::MIN_POSITIVE / 2.0,
                5e-324,
                -5e-324,
                f64::MAX,
                -f64::MAX,
                1e-300,
                1e300,
            ],
        },
    ];

    let mut out =
        String::from("{\n  \"see\": \"FORMAT.md (RelSketch section)\",\n  \"cases\": [\n");
    for (i, c) in cases.iter().enumerate() {
        let mut s = RelSketch::with_sub_bits(c.sub_bits).unwrap();
        for &x in &c.inputs {
            s.add(x);
        }
        let inputs: Vec<String> = c
            .inputs
            .iter()
            .map(|x| format!("\"{:016x}\"", x.to_bits()))
            .collect();
        let _ = write!(
            out,
            "    {{\n      \"name\": \"{}\",\n      \"sub_bits\": {},\n      \"input_bits\": [{}],\n      \"state_hex\": \"{}\",\n      \"state_hash\": \"{}\"\n    }}{}\n",
            c.name,
            c.sub_bits,
            inputs.join(", "),
            hex(&s.to_bytes()),
            hex(&state_hash(&s)),
            if i + 1 < cases.len() { "," } else { "" }
        );
    }
    out.push_str("  ]\n}\n");

    std::fs::create_dir_all("conformance").unwrap();
    std::fs::write("conformance/relsketch_vectors.json", out).unwrap();
    println!("wrote conformance/relsketch_vectors.json");
}
