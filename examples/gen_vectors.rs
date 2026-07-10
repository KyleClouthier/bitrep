// Copyright (c) 2026 Kyle Clouthier. Licensed under MIT OR Apache-2.0.
//! Generates `conformance/vectors.json` — the cross-language conformance
//! vectors described in FORMAT.md. Run: `cargo run --example gen_vectors`.

use bitrep::{SumF32, SumF64};
use std::fmt::Write as _;

fn hex(b: &[u8]) -> String {
    b.iter().fold(String::new(), |mut s, x| {
        let _ = write!(s, "{x:02x}");
        s
    })
}

struct Case {
    name: &'static str,
    inputs: Vec<f64>,
}

fn xorshift_data(seed: u64, n: usize) -> Vec<f64> {
    let mut s = seed;
    let mut out = Vec::with_capacity(n);
    while out.len() < n {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        let x = f64::from_bits(s);
        if x.is_finite() {
            out.push(x);
        }
    }
    out
}

fn main() {
    let cases = vec![
        Case {
            name: "empty",
            inputs: vec![],
        },
        Case {
            name: "zeros_and_neg_zero",
            inputs: vec![0.0, -0.0, 0.0],
        },
        Case {
            name: "cancellation",
            inputs: vec![1e100, 1e-300, -1e100],
        },
        Case {
            name: "subnormals",
            inputs: vec![
                f64::from_bits(1),
                f64::from_bits(1),
                -f64::from_bits(2),
                f64::MIN_POSITIVE,
            ],
        },
        Case {
            name: "overflow_to_inf",
            inputs: vec![f64::MAX, f64::MAX, -1.0],
        },
        Case {
            name: "single_inf",
            inputs: vec![f64::INFINITY, 1.0],
        },
        Case {
            name: "inf_conflict_nan",
            inputs: vec![f64::INFINITY, f64::NEG_INFINITY],
        },
        Case {
            name: "nan",
            inputs: vec![f64::NAN, 2.0],
        },
        Case {
            name: "extremes",
            inputs: vec![f64::MAX, -f64::MAX, f64::MIN_POSITIVE, -f64::from_bits(1)],
        },
        Case {
            name: "random_1000_full_range",
            inputs: xorshift_data(0x9E3779B97F4A7C15, 1000),
        },
    ];

    let mut out = String::from("{\n  \"format\": 1,\n  \"see\": \"FORMAT.md\",\n  \"cases\": [\n");
    for (i, c) in cases.iter().enumerate() {
        let acc: SumF64 = c.inputs.iter().copied().collect();
        let inputs: Vec<String> = c
            .inputs
            .iter()
            .map(|x| format!("\"{:016x}\"", x.to_bits()))
            .collect();
        let _ = write!(
            out,
            "    {{\n      \"name\": \"{}\",\n      \"input_bits\": [{}],\n      \"state_hex\": \"{}\",\n      \"value_bits\": \"{:016x}\"\n    }}{}\n",
            c.name,
            inputs.join(", "),
            hex(&acc.to_bytes()),
            acc.value().to_bits(),
            if i + 1 < cases.len() { "," } else { "" }
        );
    }
    out.push_str("  ],\n");

    // One f32 case: the double-rounding trap, rounded once from exact state.
    let f32_inputs = [
        1.0f32,
        f32::from_bits(0x33800000 /* 2^-24 */),
        f32::from_bits(0x21800000 /* 2^-60 */),
    ];
    let acc32: SumF32 = f32_inputs.iter().copied().collect();
    let in32: Vec<String> = f32_inputs
        .iter()
        .map(|x| format!("\"{:08x}\"", x.to_bits()))
        .collect();
    let _ = write!(
        out,
        "  \"f32_case\": {{\n    \"name\": \"double_rounding_trap\",\n    \"input_bits\": [{}],\n    \"state_hex\": \"{}\",\n    \"value_bits\": \"{:08x}\"\n  }}\n}}\n",
        in32.join(", "),
        hex(&acc32.to_bytes()),
        acc32.value().to_bits()
    );

    std::fs::create_dir_all("conformance").unwrap();
    std::fs::write("conformance/vectors.json", out).unwrap();
    println!("wrote conformance/vectors.json");
}
