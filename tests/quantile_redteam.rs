// Copyright (c) 2026 Kyle Clouthier / Clouthier Simulation Labs. Licensed under MIT OR Apache-2.0.
//! RED TEAM for `RelSketch` (feature `quantile`).
//!
//! Adversarial and pathological inputs aimed squarely at the invariants an
//! attacker would try to break:
//!   * BYTE-IDENTITY on hostile multisets (subnormals, ±0, ±∞, NaN, f64::MAX,
//!     huge negatives, exact bucket-boundary values) under every reordering,
//!     sharding and merge-tree shape.
//!   * ADVERSARIAL DECODE: every non-canonical serialized state is rejected —
//!     non-minimal varints, duplicate/unsorted keys, zero counts, truncation,
//!     trailing bytes, oversized length claims, out-of-range headers. A decoder
//!     that accepted one would let two byte strings mean one state, breaking the
//!     merge-law/receipt guarantee (the ExtremaF64 decoder-hardening story).
//!   * MERGE OVERFLOW: counts near u64::MAX saturate, never wrap.
//!   * RANGE-EXPLOSION DoS: a stream spanning every exponent is bounded by the
//!     collapse policy, and byte-identity survives the collapse.
//!   * quantile(q) DOMAIN: q outside [0,1], empty, all-NaN, single value.
//!   * DIFFERENTIAL vs a brute-force exact oracle (proptest) for many q.
//!
//! Run: `cargo test --release --features quantile --test quantile_redteam -- --nocapture`

#![cfg(feature = "quantile")]

use bitrep::{Mergeable, RelSketch};
use proptest::prelude::*;

// deterministic xorshift64* — reproducible, no external dependency
struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed | 1)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

fn shuffle<T>(v: &mut [T], rng: &mut Rng) {
    for i in (1..v.len()).rev() {
        let j = (rng.next_u64() % (i as u64 + 1)) as usize;
        v.swap(i, j);
    }
}

fn build(data: &[f64], alpha: f64) -> RelSketch {
    let mut s = RelSketch::new(alpha).unwrap();
    for &x in data {
        s.add(x);
    }
    s
}

// ---------------------------------------------------------------------------
// 1. BYTE-IDENTITY on a hostile multiset, including exact bucket boundaries
// ---------------------------------------------------------------------------
#[test]
fn byte_identity_on_boundary_and_special_values() {
    let sub_bits = 6u32;
    let shift = 52 - sub_bits;
    // Values whose bits are exact multiples of 2^shift sit EXACTLY on bucket
    // boundaries; boundary±1 ULP straddle them. The mapping must place these
    // deterministically regardless of order.
    let mut data: Vec<f64> = Vec::new();
    for e in [1000u64, 1010, 1023, 1024, 1050, 1074] {
        let base = e << 52; // exponent field e, mantissa 0 -> a power of two
        for t in 0..8u64 {
            let boundary = base | (t << shift);
            data.push(f64::from_bits(boundary)); // exactly on a boundary
            data.push(f64::from_bits(boundary + 1)); // one ULP above
            data.push(f64::from_bits(boundary.wrapping_sub(1))); // one ULP below
        }
    }
    // Full special zoo.
    for &x in &[
        0.0,
        -0.0,
        f64::MIN_POSITIVE,
        f64::MIN_POSITIVE / 2.0,
        5e-324,
        -5e-324,
        f64::MAX,
        -f64::MAX,
        1e308,
        -1e308,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
        -f64::NAN,
    ] {
        for _ in 0..11 {
            data.push(x);
        }
    }

    let reference = build(&data, 0.01);
    let ref_bytes = reference.to_bytes();
    assert_eq!(RelSketch::from_bytes(&ref_bytes).unwrap(), reference);

    let mut rng = Rng::new(0xB0A7);
    for _ in 0..20 {
        let mut d = data.clone();
        shuffle(&mut d, &mut rng);
        assert_eq!(build(&d, 0.01).to_bytes(), ref_bytes, "order changed bytes");
    }
    // shard + merge in a scrambled tree
    for k in 2..=9usize {
        let mut d = data.clone();
        shuffle(&mut d, &mut rng);
        let mut shards: Vec<RelSketch> = (0..k).map(|_| RelSketch::new(0.01).unwrap()).collect();
        for (i, &x) in d.iter().enumerate() {
            shards[i % k].add(x);
        }
        let mut merged = RelSketch::new(0.01).unwrap();
        for s in shards.iter().rev() {
            merged.merge(s);
        }
        assert_eq!(merged.to_bytes(), ref_bytes, "sharding K={k} changed bytes");
    }
}

// ---------------------------------------------------------------------------
// 2. ADVERSARIAL DECODE — every non-canonical form must be rejected
// ---------------------------------------------------------------------------

/// Minimal unsigned-LEB128 encoder, for building hostile inputs by hand.
fn uv(mut x: u64) -> Vec<u8> {
    let mut o = Vec::new();
    loop {
        let b = (x & 0x7f) as u8;
        x >>= 7;
        if x == 0 {
            o.push(b);
            return o;
        }
        o.push(b | 0x80);
    }
}

/// A fixed 59-byte header (sub_bits, collapse, mismatched, 4×u64 specials,
/// min bits, max bits, count) with empty extrema and zero specials.
fn header(sub_bits: u8, collapse: u8) -> Vec<u8> {
    let mut v = vec![sub_bits, collapse, 0];
    for _ in 0..4 {
        v.extend_from_slice(&0u64.to_le_bytes()); // nan, pos_inf, neg_inf, zero
    }
    v.extend_from_slice(&f64::INFINITY.to_bits().to_le_bytes()); // min
    v.extend_from_slice(&f64::NEG_INFINITY.to_bits().to_le_bytes()); // max
    v.extend_from_slice(&0u64.to_le_bytes()); // count
    v
}

#[test]
fn decode_rejects_every_non_canonical_form() {
    // A hand-built, well-formed one-positive-bucket state: key 100, count 5.
    let mut good = header(6, 0);
    good.extend(uv(1)); // pos len = 1
    good.extend(uv(100)); // first key (absolute)
    good.extend(uv(5)); // count
    good.extend(uv(0)); // neg len = 0
    assert!(
        RelSketch::from_bytes(&good).is_some(),
        "the control must decode"
    );

    // helper: header + custom pos-map body + empty neg map
    let with_pos = |body: Vec<u8>| -> Vec<u8> {
        let mut v = header(6, 0);
        v.extend(body);
        v.extend(uv(0)); // neg len 0
        v
    };

    // non-minimal (overlong) varint for the key: 100 as [0xE4, 0x00]
    let mut b = uv(1);
    b.extend_from_slice(&[0xE4, 0x00]);
    b.extend(uv(5));
    assert!(
        RelSketch::from_bytes(&with_pos(b)).is_none(),
        "overlong key varint"
    );

    // duplicate / non-ascending key: delta 0 on the second bucket
    let mut b = uv(2);
    b.extend(uv(100)); // key 100
    b.extend(uv(1)); // count
    b.extend(uv(0)); // delta 0 -> duplicate key
    b.extend(uv(1)); // count
    assert!(
        RelSketch::from_bytes(&with_pos(b)).is_none(),
        "duplicate key"
    );

    // zero count is never canonical (empty bucket)
    let mut b = uv(1);
    b.extend(uv(100));
    b.extend(uv(0)); // count 0
    assert!(RelSketch::from_bytes(&with_pos(b)).is_none(), "zero count");

    // oversized length claim: says 1000 buckets, provides none
    let mut b = uv(1000);
    b.push(0x01); // a stray byte, nowhere near enough
    assert!(
        RelSketch::from_bytes(&with_pos(b)).is_none(),
        "oversized length"
    );

    // truncated mid-varint: a lone continuation byte
    let b = vec![0x80];
    assert!(
        RelSketch::from_bytes(&with_pos(b)).is_none(),
        "truncated varint"
    );

    // key-sum overflow: first key u64::MAX, delta 1 -> wraps
    let mut b = uv(2);
    b.extend(uv(u64::MAX));
    b.extend(uv(1));
    b.extend(uv(1)); // delta 1 -> MAX + 1 overflow
    b.extend(uv(1));
    assert!(
        RelSketch::from_bytes(&with_pos(b)).is_none(),
        "key overflow"
    );

    // header abuse
    assert!(RelSketch::from_bytes(&{
        let mut v = good.clone();
        v[0] = 0; // sub_bits 0
        v
    })
    .is_none());
    assert!(RelSketch::from_bytes(&{
        let mut v = good.clone();
        v[0] = 53; // sub_bits > 52
        v
    })
    .is_none());
    assert!(RelSketch::from_bytes(&{
        let mut v = good.clone();
        v[1] = 18; // collapse: 52 - 6 + 18 = 64 -> reconstruction shift out of range
        v
    })
    .is_none());
    assert!(RelSketch::from_bytes(&{
        let mut v = good.clone();
        v[2] = 2; // mismatched flag is not a bool
        v
    })
    .is_none());
    // trailing byte and truncation
    assert!(RelSketch::from_bytes(&{
        let mut v = good.clone();
        v.push(0);
        v
    })
    .is_none());
    assert!(RelSketch::from_bytes(&good[..good.len() - 1]).is_none());

    // Every accepted state must re-encode to exactly its input (canonical form
    // is unique), fuzzed over structured byte strings.
    let mut rng = Rng::new(0xDEC0DE);
    for _ in 0..20_000 {
        let mut bytes = good.clone();
        let i = (rng.next_u64() as usize) % bytes.len();
        bytes[i] ^= (rng.next_u64() as u8) | 1;
        if let Some(s) = RelSketch::from_bytes(&bytes) {
            assert_eq!(s.to_bytes(), bytes, "accepted a non-canonical encoding");
        }
    }
}

// ---------------------------------------------------------------------------
// 3. MERGE OVERFLOW — counts near u64::MAX saturate, never wrap
// ---------------------------------------------------------------------------
#[test]
fn merge_counts_saturate_never_wrap() {
    // Build two sketches whose single bucket already holds ~u64::MAX via the
    // OTel-layout constructor (no need to add MAX times).
    let a = RelSketch::from_otel(6, &[(0, u64::MAX - 1)], &[]).unwrap();
    let mut m = a.clone();
    m.merge(&a); // (MAX-1) + (MAX-1) must saturate to MAX, not wrap to a small value
    let bytes = m.to_bytes();
    let back = RelSketch::from_bytes(&bytes).unwrap();
    assert_eq!(bytes, back.to_bytes());
    // The read must still be sane (finite, no panic) with a saturated bucket.
    let q = m.quantile(0.5).unwrap();
    assert!(q.is_finite());
    // top-level count also saturates
    let mut c = a.clone();
    c.merge(&a);
    assert_eq!(c.count(), u64::MAX);
}

// ---------------------------------------------------------------------------
// 4. RANGE-EXPLOSION DoS — collapse bounds memory, byte-identity survives
// ---------------------------------------------------------------------------
#[test]
fn range_explosion_is_bounded_and_still_byte_identical() {
    // ~90k distinct bucket keys (exp 1..=1405 × 64 top-mantissa slots),
    // exceeding MAX_BUCKETS so the collapse policy must fire.
    let shift = 52 - 6u32;
    let mut data: Vec<f64> = Vec::with_capacity(1405 * 64);
    for e in 1u64..=1405 {
        for t in 0..64u64 {
            let bits = (e << 52) | (t << shift);
            let x = f64::from_bits(bits);
            debug_assert!(x.is_finite() && x > 0.0);
            data.push(x);
        }
    }

    let reference = build(&data, 0.01);
    assert!(reference.collapse_shift() >= 1, "collapse must have fired");
    assert!(
        reference.bucket_count() <= RelSketch::MAX_BUCKETS,
        "bucket count {} exceeded the cap {}",
        reference.bucket_count(),
        RelSketch::MAX_BUCKETS
    );
    // The guarantee coarsens by exactly the collapse depth, honestly reported.
    let expect_alpha = (0.5f64).powi(6 - reference.collapse_shift() as i32 + 1);
    assert_eq!(reference.guaranteed_alpha(), expect_alpha);

    let ref_bytes = reference.to_bytes();
    assert_eq!(RelSketch::from_bytes(&ref_bytes).unwrap(), reference);

    // Byte-identity must survive the collapse under reordering and sharding
    // (the collapse is a pure function of the multiset).
    let mut rng = Rng::new(0xE0F1);
    for _ in 0..4 {
        let mut d = data.clone();
        shuffle(&mut d, &mut rng);
        assert_eq!(
            build(&d, 0.01).to_bytes(),
            ref_bytes,
            "collapse not order-invariant"
        );
    }
    for k in [3usize, 7, 16] {
        let mut d = data.clone();
        shuffle(&mut d, &mut rng);
        let mut shards: Vec<RelSketch> = (0..k).map(|_| RelSketch::new(0.01).unwrap()).collect();
        for (i, &x) in d.iter().enumerate() {
            shards[i % k].add(x);
        }
        let mut merged = RelSketch::new(0.01).unwrap();
        for s in shards.iter().rev() {
            merged.merge(s);
        }
        assert_eq!(
            merged.to_bytes(),
            ref_bytes,
            "collapse not merge-invariant K={k}"
        );
    }
    println!(
        "[dos] {} distinct keys -> collapse_shift {}, {} buckets, {} bytes (cap {})",
        data.len(),
        reference.collapse_shift(),
        reference.bucket_count(),
        ref_bytes.len(),
        RelSketch::MAX_BUCKETS
    );
}

// ---------------------------------------------------------------------------
// 5. quantile(q) DOMAIN — defined behavior on every edge
// ---------------------------------------------------------------------------
#[test]
fn quantile_domain_edges() {
    // q outside [0,1]
    let mut s = RelSketch::new(0.01).unwrap();
    s.add(1.0);
    assert_eq!(s.quantile(-0.001), None);
    assert_eq!(s.quantile(1.001), None);
    assert_eq!(s.quantile(f64::NAN), None);

    // empty
    let e = RelSketch::new(0.01).unwrap();
    assert_eq!(e.quantile(0.5), None);

    // all-NaN: rankable is zero, reads are None, count/nan tracked
    let mut n = RelSketch::new(0.01).unwrap();
    for _ in 0..10 {
        n.add(f64::NAN);
    }
    assert_eq!(n.quantile(0.5), None);
    assert_eq!(n.min(), None);
    assert_eq!(n.nan_count(), 10);
    assert_eq!(n.count(), 10);

    // single value: every quantile is that value within the guarantee
    let mut one = RelSketch::new(0.01).unwrap();
    one.add(42.0);
    for &q in &[0.0, 0.5, 1.0] {
        let v = one.quantile(q).unwrap();
        assert!((v - 42.0).abs() / 42.0 <= one.guaranteed_alpha() * 1.001);
    }
}

// ---------------------------------------------------------------------------
// 6. DIFFERENTIAL vs a brute-force exact oracle (proptest)
// ---------------------------------------------------------------------------
fn exact_quantile(sorted: &[f64], q: f64) -> f64 {
    let idx = ((q * sorted.len() as f64).ceil() as usize).clamp(1, sorted.len()) - 1;
    sorted[idx]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// On positive finite inputs the estimate is within the relative-error
    /// guarantee of the exact sorted quantile, for arbitrary q.
    #[test]
    fn differential_positive_within_guarantee(
        data in prop::collection::vec(1e-6f64..1e9f64, 1..800),
        qs in prop::collection::vec(0.0f64..=1.0, 1..8),
    ) {
        let sketch = build(&data, 0.01);
        let guar = sketch.guaranteed_alpha();
        let mut sorted = data.clone();
        sorted.sort_by(f64::total_cmp);
        for q in qs {
            let exact = exact_quantile(&sorted, q);
            let est = sketch.quantile(q).unwrap();
            let rel = (est - exact).abs() / exact.abs();
            prop_assert!(
                rel <= guar * 1.002,
                "q={q} exact={exact} est={est} rel={rel} > guar={guar}"
            );
        }
    }

    /// On ARBITRARY f64 (NaN/∞/subnormal included), quantile is total and
    /// monotone nondecreasing in q, and byte-identity holds under a reversal.
    #[test]
    fn differential_arbitrary_monotone_and_reproducible(
        bits in prop::collection::vec(any::<u64>(), 1..400),
    ) {
        let data: Vec<f64> = bits.iter().map(|&b| f64::from_bits(b)).collect();
        let forward = build(&data, 0.01);
        let mut rev = RelSketch::new(0.01).unwrap();
        for &x in data.iter().rev() {
            rev.add(x);
        }
        prop_assert_eq!(forward.to_bytes(), rev.to_bytes(), "reversal changed bytes");

        // monotone in q (skip if the sketch is all-NaN -> reads are None)
        if forward.quantile(0.0).is_some() {
            let mut prev = f64::NEG_INFINITY;
            for i in 0..=20 {
                let q = i as f64 / 20.0;
                let v = forward.quantile(q).unwrap();
                prop_assert!(!v.is_nan());
                prop_assert!(v.total_cmp(&prev).is_ge(), "quantile not monotone at q={}", q);
                prev = v;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 7. CANONICAL IDEMPOTENCE — merging decoded states re-encodes to itself, and
//    state-equality is byte-identity (bitwise on min/max), NOT IEEE value.
//
// Regression for a libFuzzer crash (target `quantile_decode`, ~8k execs): a
// decoded state carrying a `NaN` in `min` round-trips its BYTES perfectly, but
// the old `#[derive(PartialEq)]` compared `min`/`max` by IEEE value, so
// `NaN != NaN` made the state unequal to itself and
// `from_bytes(m.to_bytes()) == Some(m)` failed after a self-merge. The fix is a
// bitwise `PartialEq`/`Eq` (min/max via `to_bits`), so state-equality exactly
// mirrors `to_bytes` — the same choice `ExtremaF64` makes.
// ---------------------------------------------------------------------------

/// The exact fuzzer-minimized crash input. It decodes to a bucketless state
/// with a `NaN` `min`; its bytes round-trip, and after a self-merge the decoded
/// state must equal the merged state (the assertion the fuzz target makes).
const CRASH_NAN_MIN: &[u8] = &[
    0xa, 0x0, 0x0, 0xf9, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x40, 0x0, 0x2f, 0x0, 0x0, 0xd6, 0xab, 0xd6, 0xd6, 0xd6, 0xd6, 0xd6, 0xd6, 0xd6,
    0xd6, 0xd6, 0xd6, 0xbf, 0xff, 0xff, 0xff, 0xd6, 0xd6, 0xd6, 0xd6, 0xd6, 0xd6, 0xd6, 0xd6, 0xd6,
    0x0, 0x0, 0x0, 0x0, 0x0, 0xff, 0x0, 0x0, 0x0,
];

#[test]
fn regression_fuzz_crash_nan_min_roundtrips_after_merge() {
    let state = RelSketch::from_bytes(CRASH_NAN_MIN).expect("the minimized crash input decodes");
    // Byte-identity of the decoded state itself (fuzz target invariant #1).
    assert_eq!(
        state.to_bytes(),
        CRASH_NAN_MIN,
        "decoded state must be byte-canonical"
    );
    assert_eq!(
        state.bucket_count(),
        0,
        "this crash is the all-special, zero-bucket shape"
    );
    assert!(
        state.min().unwrap().is_nan(),
        "the crash hinges on a NaN min extremum"
    );

    // Reads never panic on the decoded state.
    for &q in &[0.0, 0.5, 0.99, 1.0] {
        let _ = state.quantile(q);
    }
    let _ = (
        state.min(),
        state.max(),
        state.count(),
        state.guaranteed_alpha(),
    );

    // Self-merge, then the exact assertion the fuzz target `quantile_decode`
    // makes on line ~34 — a decoded-then-merged state is itself canonical.
    let mut m = state.clone();
    m.merge(&state);
    assert_eq!(
        RelSketch::from_bytes(&m.to_bytes()),
        Some(m.clone()),
        "merged state must round-trip to an EQUAL state (bitwise, NaN included)"
    );
    // A merged state is bitwise-equal to itself (the derived value-PartialEq
    // this replaced would return false here because of the NaN min).
    assert_eq!(
        m,
        m.clone(),
        "a state with a NaN extremum must equal itself"
    );
}

#[test]
fn real_all_special_sketch_roundtrips_and_self_merges() {
    // A LEGITIMATE all-special sketch, built only through the public `.add`
    // API with zeros / signed zeros / NaN / ±∞ — no buckets ever occupied.
    // This must round-trip and self-merge canonically (the fix must not reject
    // real all-special states, only stop miscomparing them).
    let mut s = RelSketch::new(0.01).unwrap();
    for _ in 0..5 {
        s.add(0.0);
        s.add(-0.0);
        s.add(f64::NAN);
        s.add(f64::INFINITY);
        s.add(f64::NEG_INFINITY);
    }
    assert_eq!(s.bucket_count(), 0, "only specials were added");
    assert_eq!(s.nan_count(), 5);
    // Extrema of a real all-special sketch are never NaN.
    assert!(!s.min().unwrap().is_nan() && !s.max().unwrap().is_nan());

    // Byte round-trip to an EQUAL state.
    let back = RelSketch::from_bytes(&s.to_bytes()).unwrap();
    assert_eq!(back, s);
    assert_eq!(back.to_bytes(), s.to_bytes());

    // Self-merge is canonical and idempotent under encode/decode.
    let mut m = s.clone();
    m.merge(&s);
    assert_eq!(RelSketch::from_bytes(&m.to_bytes()), Some(m.clone()));
    assert_eq!(m.count(), s.count().saturating_mul(2));
    assert_eq!(m.nan_count(), 10);
}

#[test]
fn equality_is_bitwise_not_ieee_value() {
    // Build two one-sample sketches whose ONLY difference is the sign of a zero
    // extremum: +0.0 vs -0.0. Their bytes differ (−0.0 has the sign bit set),
    // so — under the byte-identity contract — they must compare UNEQUAL. A
    // derived value-PartialEq would wrongly call them equal (+0.0 == -0.0).
    let mut pos_zero = RelSketch::new(0.01).unwrap();
    pos_zero.add(0.0);
    let mut neg_zero = RelSketch::new(0.01).unwrap();
    neg_zero.add(-0.0);
    assert_ne!(
        pos_zero.to_bytes(),
        neg_zero.to_bytes(),
        "±0.0 extrema must produce different bytes"
    );
    assert_ne!(
        pos_zero, neg_zero,
        "state-equality must track byte-identity, not IEEE value"
    );

    // And conversely: equal bytes ⟺ equal states, NaN extrema included.
    let nan_state = RelSketch::from_bytes(CRASH_NAN_MIN).unwrap();
    assert_eq!(nan_state, nan_state.clone());
    assert_eq!(
        RelSketch::from_bytes(&nan_state.to_bytes()),
        Some(nan_state)
    );
}

// ---------------------------------------------------------------------------
// 8. KEY-SPACE CEILING — decoded/constructed bucket keys must stay within the
//    range reachable from real f64 samples (key of at most f64::MAX). A key
//    above it is non-canonical AND overflows the signed OTel index arithmetic.
//    Regression for the two decoder fuzz catches (2026-07-13/14):
//      * from_bytes accepting an out-of-range key -> i64 subtract overflow in
//        otel_positive_indices  (crash-c6b293db, 119 bytes).
//      * from_otel not enforcing the ceiling its own doc promises -> a state
//        that could not round-trip through from_bytes.
// ---------------------------------------------------------------------------
#[test]
fn out_of_range_bucket_key_is_rejected_and_reads_never_panic() {
    // from_otel: an index that maps a key beyond f64::MAX's key must be None.
    // At sub_bits=6, effective_shift = 52-6 = 46, so max_key = f64::MAX bits
    // >> 46. one_key = 1.0 bits >> 46. An index far past (max_key - one_key)
    // lands outside the key space.
    let sub_bits = 6u8;
    let shift = 52 - sub_bits as u32;
    let max_key = f64::MAX.to_bits() >> shift;
    let one_key = (1.0f64.to_bits() >> shift) as i64;
    let too_big_idx = (max_key as i64 - one_key) + 1; // one past the ceiling
    assert!(
        RelSketch::from_otel(sub_bits, &[(too_big_idx, 1)], &[]).is_none(),
        "from_otel must reject an index outside the key space (doc contract)"
    );
    // The largest in-range index is still accepted and round-trips.
    let ok_idx = max_key as i64 - one_key;
    let s = RelSketch::from_otel(sub_bits, &[(ok_idx, 1)], &[])
        .expect("max in-range key must be accepted");
    assert_eq!(RelSketch::from_bytes(&s.to_bytes()), Some(s.clone()));
    // Every read on the boundary state is finite/sane — no panic, no overflow.
    let _ = s.otel_positive_indices();
    let _ = s.otel_negative_indices();
    assert!(s.quantile(0.5).map(|q| q.is_finite() || q.is_infinite()).unwrap_or(true));

    // Negatives share the ceiling.
    assert!(
        RelSketch::from_otel(sub_bits, &[], &[(too_big_idx, 1)]).is_none(),
        "from_otel must reject an out-of-range negative index too"
    );
}
