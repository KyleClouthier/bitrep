// Copyright (c) 2026 Kyle Clouthier / Clouthier Simulation Labs. Licensed under MIT OR Apache-2.0.
//! RelSketch <-> OpenTelemetry / Prometheus exponential histograms.
//!
//! The observability stack already standardizes on an exponential-bucket
//! histogram: OpenTelemetry's *Exponential Histogram* and Prometheus's *native
//! histograms* are the same object — a bucket layout with `2^scale` buckets per
//! power of two, base `b = 2^(2^-scale)`, bucket `i` covering `(b^i, b^(i+1)]`.
//!
//! RelSketch's integer bit-shift mapping is the SAME family: it splits each
//! power-of-two octave into `2^sub_bits` sub-buckets. At `scale = sub_bits` the
//! two layouts have the SAME resolution (`2^scale` buckets per octave) and the
//! SAME octave alignment (both put a boundary at every power of two). They are
//! NOT the same mapping inside an octave, and this example measures the gap
//! honestly:
//!
//!  * OTel/Prometheus place interior boundaries GEOMETRICALLY (`b^i`);
//!    RelSketch places them by LINEAR mantissa interpolation (DDSketch's
//!    `BitwiseLinearlyInterpolatedMapping`). So the sub-bucket index of the
//!    same value can differ, worst case `2^scale · max|log2(1+m) − m| ≈
//!    0.0861 · 2^scale` sub-buckets, mid-octave — ~5-6 buckets at scale 6.
//!  * At the octave boundary the two coincide up to a fixed convention offset
//!    of one bucket: OTel buckets are upper-inclusive `(b^i, b^(i+1)]`, so a
//!    value exactly at `2^e` lands one index lower than RelSketch's
//!    lower-inclusive `[2^e, 2^(e+1))`.
//!
//! Both bound the relative error by ~`2^-(scale+1)`, so the two histograms are
//! interchangeable *for quantile estimation within that error* — but a faithful
//! conversion re-buckets through the mapping, it is not a bare index shift.
//! What round-trips EXACTLY is RelSketch <-> its own exponential index layout
//! (a bijection), which is what part (b) demonstrates: you can emit a **signed**
//! RelSketch receipt alongside the histogram you already export.
//!
//! Run: `cargo run --release --features quantile --example otel_bridge`
//!
//! NOTE: the OTel reference index here uses `f64::ln` — a transcendental, and
//! deliberately so: it is the *incumbent* mapping we are comparing against.
//! RelSketch's own mapping never calls it (that is the whole reproducibility
//! point); `ln` appears only in this comparison harness.

use bitrep::RelSketch;

/// A tiny deterministic xorshift so the demo needs no rng dependency.
struct Rng(u64);
impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn latency_ms(&mut self) -> f64 {
        let u = (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        let v = (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        let z = (-2.0 * (u + 1e-12).ln()).sqrt() * (std::f64::consts::TAU * v).cos();
        (2.0 + 0.6 * z).exp()
    }
}

/// The OTel / Prometheus exponential-histogram index of a positive `v` at the
/// given `scale`: the unique `i` with `b^i < v <= b^(i+1)`, `b = 2^(2^-scale)`.
/// This is the standard mapping (`ceil(log_b(v)) - 1`).
fn otel_index(v: f64, scale: i32) -> i64 {
    let base = 2.0f64.powf(2.0f64.powi(-scale));
    (v.ln() / base.ln()).ceil() as i64 - 1
}

fn main() {
    let sub_bits = 6u8; // alpha ~ 0.78%
    let sketch = RelSketch::with_sub_bits(sub_bits).unwrap();
    let scale = sketch.otel_scale();
    let base = 2.0f64.powf(2.0f64.powi(-scale));
    println!("RelSketch sub_bits = {sub_bits}  <->  OTel scale = {scale}");
    println!("OTel base b = 2^(2^-{scale}) = {base:.10}");
    println!("buckets per power of two: 2^{scale} = {}", 1i64 << scale);

    // (a) INDEX CORRESPONDENCE ------------------------------------------------
    // Measured, not assumed: at powers of two the two mappings coincide up to
    // the +1 upper-vs-lower-inclusive convention offset; in the interior they
    // diverge by up to ~0.0861·2^scale sub-buckets (geometric vs linear).
    println!("\n[index correspondence]  value      OTel idx   RelSketch idx   Δ");
    let probes = [
        1.0, 1.5, 2.0, 3.0, 4.0, 7.0, 8.0, 100.0, 128.0, 1000.0, 1024.0,
    ];
    let mut max_interior = 0i64;
    let mut max_boundary = 0i64;
    for &v in &probes {
        let mut s = RelSketch::with_sub_bits(sub_bits).unwrap();
        s.add(v);
        let rel_idx = s.otel_positive_indices()[0].0;
        let ot_idx = otel_index(v, scale);
        let d = rel_idx - ot_idx;
        let at_pow2 = v.to_bits() & ((1u64 << 52) - 1) == 0;
        if at_pow2 {
            max_boundary = max_boundary.max(d.abs());
        } else {
            max_interior = max_interior.max(d.abs());
        }
        println!(
            "                        {v:>8.1}   {ot_idx:>8}   {rel_idx:>13}   {d:>2}{}",
            if at_pow2 { "   <- power of two" } else { "" }
        );
    }
    // The honest, theory-backed envelope: the linear/geometric interpolation
    // gap is at most max|log2(1+m)-m| = 0.0861, times the 2^scale sub-buckets.
    let interior_bound = (0.0861 * (1i64 << scale) as f64).ceil() as i64;
    println!(
        "max interior |Δ| = {max_interior} (linear-vs-geometric bound {interior_bound}); \
         boundary |Δ| = {max_boundary} (inclusive-side convention, expected 1)"
    );
    assert!(
        max_interior <= interior_bound,
        "interior divergence {max_interior} exceeded the linear/geometric bound {interior_bound}"
    );
    assert!(
        max_boundary <= 1,
        "octave boundaries must align up to the convention offset"
    );

    // (b) ROUND-TRIP THROUGH THE OTEL LAYOUT ----------------------------------
    // Build a real sketch, export the (index, count) arrays exactly as you
    // would hand them to an OTel exporter, reconstruct, and confirm the bucket
    // layer is preserved byte-for-byte (indices) and read-for-read (quantiles).
    let mut rng = Rng(0x0BE1_5EED);
    let mut full = RelSketch::with_sub_bits(sub_bits).unwrap();
    for _ in 0..500_000 {
        full.add(rng.latency_ms());
    }
    let pos = full.otel_positive_indices();
    let neg = full.otel_negative_indices();
    println!(
        "\n[round-trip] {} positive buckets, {} negative, exported as OTel index+count arrays",
        pos.len(),
        neg.len()
    );

    let rebuilt = RelSketch::from_otel(sub_bits, &pos, &neg).expect("valid OTel layout");
    assert_eq!(
        rebuilt.otel_positive_indices(),
        pos,
        "exported/reimported positive indices must match"
    );
    assert_eq!(rebuilt.otel_negative_indices(), neg);

    // Quantile reads over the bucket mass survive the round-trip identically.
    for &q in &[0.5, 0.9, 0.99] {
        let a = full.quantile(q).unwrap();
        let b = rebuilt.quantile(q).unwrap();
        println!(
            "  p{:<5} original {a:>9.4}   via OTel layout {b:>9.4}",
            q * 100.0
        );
        assert_eq!(
            a.to_bits(),
            b.to_bits(),
            "quantile changed across the OTel round-trip"
        );
    }
    // A malformed layout (non-ascending indices) is rejected, never guessed.
    assert!(RelSketch::from_otel(sub_bits, &[(5, 1), (5, 1)], &[]).is_none());

    println!(
        "\nOK: RelSketch and OTel/Prometheus exponential histograms share resolution and octave"
    );
    println!(
        "alignment (scale = sub_bits); the interior mapping differs (linear vs geometric) within"
    );
    println!(
        "the shared alpha. RelSketch's own (index,count) layout round-trips exactly, so a signed"
    );
    println!("receipt travels with the histogram.");
}
