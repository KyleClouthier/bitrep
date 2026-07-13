// Copyright (c) 2026 Kyle Clouthier / Clouthier Simulation Labs. Licensed under MIT OR Apache-2.0.
//! The cross-architecture golden vector.
//!
//! CI runs this exact test on x86-64 Linux, ARM64 macOS, x86-64 Windows and
//! wasm32 — the SHA-256 below must match on every one of them, for every
//! permutation and sharding tried. This is the README badge's claim, with
//! teeth: "Any order. Any hardware. Same bits."
//!
//! The dataset is a deterministic xorshift stream shaped to be hostile:
//! magnitudes spread across ~600 decades (including subnormals), signs mixed,
//! plus injected exact cancellations.

#[cfg(feature = "std")]
use bitrep::DotF64;
use bitrep::{SumF32, SumF64};
use sha2::{Digest, Sha256};

/// Deterministic xorshift64* stream — no RNG dependency, no platform drift.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn f64(&mut self) -> f64 {
        // Random finite f64 from raw bits (reject non-finite): exercises the
        // full encoding space, subnormals included.
        loop {
            let x = f64::from_bits(self.next());
            if x.is_finite() {
                return x;
            }
        }
    }
}

fn golden_data() -> Vec<f64> {
    let mut r = Rng(0x9E3779B97F4A7C15); // golden ratio, nothing up the sleeve
    let mut v: Vec<f64> = (0..10_000).map(|_| r.f64()).collect();
    // Exact cancellation pairs and boundary values, interleaved.
    for i in 0..500 {
        let x = f64::from_bits(r.next() & 0x7FEF_FFFF_FFFF_FFFF); // finite positive
        v.insert((r.next() % v.len() as u64) as usize, x);
        v.insert((r.next() % v.len() as u64) as usize, -x);
        let _ = i;
    }
    v.extend_from_slice(&[
        f64::MAX,
        -f64::MAX,
        f64::MIN_POSITIVE,
        f64::from_bits(1), // min subnormal
        -f64::from_bits(1),
        0.0,
        -0.0,
    ]);
    v
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Expected digests. If an intentional format change ever alters these,
/// that is a breaking change and gets a major version bump.
const SUM_DIGEST: &str = "54f3ce5fbbd6130d70db87dffe73c26160b0d99a6f92f9dd3b276a99bb9f8441";
const F32_DIGEST: &str = "825e3c7a6a43bd9870c00c222850dbbcf5eb529a63c748064832cc9f7eeeea26";
#[cfg(feature = "std")]
const DOT_DIGEST: &str = "e0e967014704730292e6f8679b745f7284d3077fd59c01bfbe37523f32e61a5a";

#[test]
fn golden_sum_digest_is_architecture_independent() {
    let data = golden_data();

    // Sequential.
    let seq: SumF64 = data.iter().copied().collect();

    // Reversed.
    let mut rev = SumF64::new();
    for x in data.iter().rev() {
        rev.add(*x);
    }
    assert_eq!(seq.to_bytes(), rev.to_bytes());

    // Sharded into 13 chunks, merged in reverse order.
    let shards: Vec<SumF64> = data
        .chunks(data.len() / 13 + 1)
        .map(|c| c.iter().copied().collect())
        .collect();
    let mut merged = SumF64::new();
    for s in shards.iter().rev() {
        merged.merge(s);
    }
    assert_eq!(seq.to_bytes(), merged.to_bytes());

    let digest = hex(&Sha256::digest(seq.to_bytes()));
    assert_eq!(digest, SUM_DIGEST, "SumF64 golden digest changed — cross-arch bit-identity broken (or an intentional format change: bump major)");
}

#[test]
fn golden_f32_digest_is_architecture_independent() {
    let mut r = Rng(0xD1B54A32D192ED03);
    let mut acc = SumF32::new();
    for _ in 0..20_000 {
        loop {
            let x = f32::from_bits(r.next() as u32);
            if x.is_finite() {
                acc.add(x);
                break;
            }
        }
    }
    let digest = hex(&Sha256::digest(acc.to_bytes()));
    assert_eq!(digest, F32_DIGEST);
}

#[cfg(feature = "std")]
#[test]
fn golden_dot_digest_is_architecture_independent() {
    // Constrained magnitudes so products stay inside the exactness domain —
    // mul_add correctness across platforms is part of what this pins.
    let mut r = Rng(0xA0761D6478BD642F);
    let mut d = DotF64::new();
    for _ in 0..10_000 {
        let a = (r.next() >> 11) as f64 / (1u64 << 53) as f64 * 2e10 - 1e10;
        let b = (r.next() >> 11) as f64 / (1u64 << 53) as f64 * 2e-10 - 1e-10;
        d.push(a, b);
    }
    assert!(d.is_exact());
    let digest = hex(&Sha256::digest(d.to_bytes()));
    assert_eq!(digest, DOT_DIGEST);
}
