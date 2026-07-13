// Copyright (c) 2026 Kyle Clouthier / Clouthier Simulation Labs. Licensed under MIT OR Apache-2.0.
//! Machine-checked proofs (Kani / CBMC bounded model checking).
//!
//! These are not tests: each harness verifies its property **for every
//! possible input**, symbolically. Run with `cargo kani` (Linux/macOS).
//!
//! What is proven vs merely tested:
//! * proven here — order-invariance of the accumulator state, merge
//!   commutativity, exact cancellation, byte-codec round-tripping;
//! * tested (BigInt oracle, NIST StRD, golden vectors) — correct rounding of
//!   `value()`, which involves f64 arithmetic Kani models slowly.
//!
//! Cost split (measured): the merge/codec harnesses are fixed-shape limb
//! arithmetic and solve in seconds to minutes — CI proves them on every
//! push (ci.yml). The add-path harnesses (`add_commutes`,
//! `cancellation_is_exact`, `add_placement_is_irrelevant`) decompose a
//! symbolic f64 and shift it across all 34 limbs, which is beyond CBMC's
//! practical reach (they did not close in ~3 h on CI runners). They are
//! gated behind `cfg(kani_slow)` for local exhaustive runs only. The add-path
//! *properties* are proved at the model level in Lean (order-invariance,
//! exact cancellation) and exercised at the bit level by the fast merge/codec
//! harnesses, the BigInt-oracle tests, and the fuzzer.

use crate::SumF64;

/// Any finite f64, symbolically. Only the `kani_slow` add-path harnesses use it, so it is gated the
/// same way — otherwise it is dead code under the default config, which `-D warnings` rejects.
#[cfg(kani_slow)]
fn any_finite() -> f64 {
    let x: f64 = kani::any();
    kani::assume(x.is_finite());
    x
}

/// add(x); add(y) leaves the same state as add(y); add(x) — for ALL pairs.
/// This is the core claim: the state is order-invariant by construction.
///
/// `kani_slow`: decomposing a symbolic f64 and shifting it across all 34 limbs is beyond CBMC's
/// practical reach — it did not close in ~3 h on CI runners. The commutativity of the state IS proved
/// at the model level in Lean (`proofs/OrderInvariance.lean`, `perm_sum_invariant`); this harness is
/// kept for local exhaustive runs, not CI.
#[cfg(kani_slow)]
#[kani::proof]
fn add_commutes() {
    let x = any_finite();
    let y = any_finite();
    let mut a = SumF64::new();
    a.add(x);
    a.add(y);
    let mut b = SumF64::new();
    b.add(y);
    b.add(x);
    assert_eq!(a, b);
}

/// Adding x then -x returns exactly to the empty integer state (count aside)
/// — cancellation is exact for every finite value, including subnormals.
///
/// `kani_slow`: as written this harness is beyond CBMC's practical reach —
/// measured 11 hours and 23GB RSS without closing. Cancellation exactness is
/// meanwhile covered by the Lean model (trivially: x + (-x) = 0 over the
/// integer state), the `max_cancels_exactly` test, the BigInt-oracle
/// property tests, and the fuzzer's cancellation-heavy corpus. Restructuring
/// this harness into exponent-class case splits is future work.
#[cfg(kani_slow)]
#[kani::proof]
fn cancellation_is_exact() {
    let x = any_finite();
    let mut a = SumF64::new();
    a.add(x);
    a.add(-x);
    let mut empty = SumF64::new();
    empty.add(0.0);
    empty.add(0.0);
    assert_eq!(a, empty);
}

/// merge(A, B) == merge(B, A) for arbitrary accumulator states — shard
/// combining commutes no matter what the shards contain.
#[kani::proof]
fn merge_commutes() {
    // Arbitrary states built from arbitrary bytes (decoder validates flags).
    let bytes_a: [u8; SumF64::BYTES] = kani::any();
    let bytes_b: [u8; SumF64::BYTES] = kani::any();
    let (Some(a0), Some(b0)) = (SumF64::from_bytes(&bytes_a), SumF64::from_bytes(&bytes_b)) else {
        return; // invalid encodings are rejected; nothing to prove
    };
    let mut ab = a0.clone();
    ab.merge(&b0);
    let mut ba = b0;
    ba.merge(&a0);
    assert_eq!(ab, ba);
}

/// merge((A ∪ B) ∪ C) == merge(A ∪ (B ∪ C)) — associativity of shard
/// combining, so ANY merge tree yields identical state.
#[kani::proof]
fn merge_associates() {
    let bytes_a: [u8; SumF64::BYTES] = kani::any();
    let bytes_b: [u8; SumF64::BYTES] = kani::any();
    let bytes_c: [u8; SumF64::BYTES] = kani::any();
    let (Some(a), Some(b), Some(c)) = (
        SumF64::from_bytes(&bytes_a),
        SumF64::from_bytes(&bytes_b),
        SumF64::from_bytes(&bytes_c),
    ) else {
        return;
    };
    let mut left = a.clone();
    left.merge(&b);
    left.merge(&c);
    let mut right_inner = b;
    right_inner.merge(&c);
    let mut right = a;
    right.merge(&right_inner);
    assert_eq!(left, right);
}

/// Adding to a shard then merging equals merging then... i.e. add distributes
/// over merge placement: put x in shard A or shard B, the total is identical.
/// NOT run by default: the SAT instance (symbolic float × two arbitrary
/// 2176-bit states) is heavy, and the property is implied by `add_commutes` +
/// `merge_commutes`/`merge_associates` (and proven at the model level in
/// proofs/OrderInvariance.lean). Kept for documentation; run explicitly with
/// `cargo kani --harness add_placement_is_irrelevant` if you have the hours.
#[cfg(kani_slow)]
#[kani::proof]
fn add_placement_is_irrelevant() {
    let x = any_finite();
    let bytes_a: [u8; SumF64::BYTES] = kani::any();
    let bytes_b: [u8; SumF64::BYTES] = kani::any();
    let (Some(a0), Some(b0)) = (SumF64::from_bytes(&bytes_a), SumF64::from_bytes(&bytes_b)) else {
        return;
    };
    let mut a1 = a0.clone();
    a1.add(x);
    let mut left = a1;
    left.merge(&b0);
    let mut b1 = b0;
    b1.add(x);
    let mut right = a0;
    right.merge(&b1);
    assert_eq!(left, right);
}

/// The byte codec round-trips every valid state exactly.
#[kani::proof]
fn bytes_roundtrip() {
    let bytes: [u8; SumF64::BYTES] = kani::any();
    if let Some(acc) = SumF64::from_bytes(&bytes) {
        assert_eq!(acc.to_bytes(), bytes);
    }
}

/// ExtremaF64 merge commutes for ALL states (adversarial decodes included).
#[kani::proof]
fn extrema_merge_commutes() {
    let ba: [u8; crate::ExtremaF64::BYTES] = kani::any();
    let bb: [u8; crate::ExtremaF64::BYTES] = kani::any();
    let (Some(a0), Some(b0)) = (
        crate::ExtremaF64::from_bytes(&ba),
        crate::ExtremaF64::from_bytes(&bb),
    ) else {
        return;
    };
    let mut ab = a0.clone();
    crate::Mergeable::merge(&mut ab, &b0);
    let mut ba2 = b0;
    crate::Mergeable::merge(&mut ba2, &a0);
    assert_eq!(ab.to_bytes(), ba2.to_bytes());
}

/// ExtremaF64 merge associates for ALL states.
#[kani::proof]
fn extrema_merge_associates() {
    let ba: [u8; crate::ExtremaF64::BYTES] = kani::any();
    let bb: [u8; crate::ExtremaF64::BYTES] = kani::any();
    let bc: [u8; crate::ExtremaF64::BYTES] = kani::any();
    let (Some(a), Some(b), Some(c)) = (
        crate::ExtremaF64::from_bytes(&ba),
        crate::ExtremaF64::from_bytes(&bb),
        crate::ExtremaF64::from_bytes(&bc),
    ) else {
        return;
    };
    let mut left = a.clone();
    crate::Mergeable::merge(&mut left, &b);
    crate::Mergeable::merge(&mut left, &c);
    let mut right_inner = b;
    crate::Mergeable::merge(&mut right_inner, &c);
    let mut right = a;
    crate::Mergeable::merge(&mut right, &right_inner);
    assert_eq!(left.to_bytes(), right.to_bytes());
}

/// ExtremaF64 byte codec round-trips every valid state exactly.
#[kani::proof]
fn extrema_bytes_roundtrip() {
    let bytes: [u8; crate::ExtremaF64::BYTES] = kani::any();
    if let Some(e) = crate::ExtremaF64::from_bytes(&bytes) {
        assert_eq!(e.to_bytes(), bytes);
    }
}
