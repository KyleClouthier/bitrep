// Copyright (c) 2026 Kyle Clouthier. Licensed under MIT OR Apache-2.0.
//! Probe: bit-identical data-parallel training via exact gradient aggregation.
//!
//! In data-parallel SGD the gradient all-reduce is a float sum whose order
//! depends on worker count and merge schedule — so the "same" run produces
//! different model bytes with 1 vs 4 vs 16 workers, even in pure f64.
//! Routing only the gradient aggregation through `SumF64` makes the final
//! model byte-identical for every worker count and merge order.
//!
//! Named limit (the other half of the problem): worker-local compute must
//! itself be deterministic and shard-shape-invariant. Here each per-sample
//! gradient is computed identically regardless of sharding (batch of one),
//! which isolates the reduction — the part bitrep owns. Making *batched*
//! kernels shape-invariant is separate work (see the batch-invariant-kernels
//! line of research); this example does not claim it.
//!
//! Run: `cargo run --release --example deterministic_training`

use bitrep::SumF64;
use sha2::{Digest, Sha256};

const D_IN: usize = 16;
const D_H: usize = 32;
const N: usize = 256;
const EPOCHS: usize = 3;
const LR: f64 = 0.05;

/// Parameters flattened into one vector: [w1 | b1 | w2 | b2].
const P_W1: usize = 0;
const P_B1: usize = P_W1 + D_IN * D_H;
const P_W2: usize = P_B1 + D_H;
const P_B2: usize = P_W2 + D_H;
const P_LEN: usize = P_B2 + 1;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn normalish(&mut self) -> f64 {
        // sum of uniforms — deterministic, good enough for a probe
        (0..4).map(|_| self.unit()).sum::<f64>() - 2.0
    }
}

fn init_params() -> Vec<f64> {
    let mut r = Rng(7 ^ 0x9E3779B97F4A7C15);
    let mut p = vec![0.0; P_LEN];
    for w in p[P_W1..P_B1].iter_mut() {
        *w = r.normalish() * 0.5;
    }
    for w in p[P_W2..P_B2].iter_mut() {
        *w = r.normalish() * 0.5;
    }
    p
}

/// (inputs, target, per-sample weight spanning 13 decades — the mixed
/// magnitudes that make reduction order actually bite)
fn make_data() -> Vec<([f64; D_IN], f64, f64)> {
    let mut r = Rng(11 ^ 0xD1B54A32D192ED03);
    (0..N)
        .map(|_| {
            let mut x = [0.0; D_IN];
            for xi in x.iter_mut() {
                *xi = r.normalish();
            }
            let y = if x.iter().sum::<f64>().sin() > 0.0 {
                1.0
            } else {
                0.0
            };
            let scale = 10f64.powi((r.next() % 13) as i32 - 6);
            (x, y, scale)
        })
        .collect()
}

/// Gradient for ONE sample (batch of one: identical bytes no matter how the
/// dataset is sharded), scaled by the sample weight.
fn per_sample_grad(p: &[f64], x: &[f64; D_IN], y: f64, wt: f64, out: &mut [f64]) {
    let mut h = [0.0f64; D_H];
    for (j, hj) in h.iter_mut().enumerate() {
        let mut s = p[P_B1 + j];
        for (i, xi) in x.iter().enumerate() {
            s += xi * p[P_W1 + i * D_H + j];
        }
        *hj = s.tanh();
    }
    let mut o = p[P_B2];
    for (j, hj) in h.iter().enumerate() {
        o += hj * p[P_W2 + j];
    }
    let d_o = 2.0 * (o - y) * wt;
    for (j, hj) in h.iter().enumerate() {
        out[P_W2 + j] = hj * d_o;
        let d_h = p[P_W2 + j] * d_o * (1.0 - hj * hj);
        out[P_B1 + j] = d_h;
        for (i, xi) in x.iter().enumerate() {
            out[P_W1 + i * D_H + j] = xi * d_h;
        }
    }
    out[P_B2] = d_o;
}

enum Agg {
    Exact,
    NaiveF64,
}

/// One full training run; returns SHA-256 of the final parameter bytes.
fn train(workers: usize, agg: Agg, merge_reversed: bool) -> String {
    let mut p = init_params();
    let data = make_data();
    let mut g = vec![0.0f64; P_LEN];

    for _ in 0..EPOCHS {
        // shard round-robin, as a scheduler would
        let shards: Vec<Vec<usize>> = (0..workers)
            .map(|w| (w..N).step_by(workers).collect())
            .collect();

        let grad: Vec<f64> = match agg {
            Agg::Exact => {
                let mut per_worker: Vec<Vec<SumF64>> = Vec::with_capacity(workers);
                for sh in &shards {
                    let mut accs: Vec<SumF64> = (0..P_LEN).map(|_| SumF64::new()).collect();
                    for &i in sh {
                        per_sample_grad(&p, &data[i].0, data[i].1, data[i].2, &mut g);
                        for (a, gi) in accs.iter_mut().zip(&g) {
                            a.add(*gi);
                        }
                    }
                    per_worker.push(accs);
                }
                if merge_reversed {
                    per_worker.reverse();
                }
                let mut total: Vec<SumF64> = (0..P_LEN).map(|_| SumF64::new()).collect();
                for wa in &per_worker {
                    for (t, a) in total.iter_mut().zip(wa) {
                        t.merge(a);
                    }
                }
                total.iter().map(|a| a.value()).collect()
            }
            Agg::NaiveF64 => {
                let mut per_worker: Vec<Vec<f64>> = Vec::with_capacity(workers);
                for sh in &shards {
                    let mut psum = vec![0.0f64; P_LEN];
                    for &i in sh {
                        per_sample_grad(&p, &data[i].0, data[i].1, data[i].2, &mut g);
                        for (s, gi) in psum.iter_mut().zip(&g) {
                            *s += *gi;
                        }
                    }
                    per_worker.push(psum);
                }
                if merge_reversed {
                    per_worker.reverse();
                }
                let mut total = vec![0.0f64; P_LEN];
                for ps in &per_worker {
                    for (t, s) in total.iter_mut().zip(ps) {
                        *t += *s;
                    }
                }
                total
            }
        };

        for (pi, gi) in p.iter_mut().zip(&grad) {
            *pi -= LR * gi / N as f64;
        }
    }

    let mut h = Sha256::new();
    for v in &p {
        h.update(v.to_bits().to_le_bytes());
    }
    format!("{:x}", h.finalize())
}

fn main() {
    // (workers, reversed merge order) — the schedules a real cluster produces
    let configs = [(1, false), (4, false), (16, false), (4, true)];

    println!("model-bytes SHA-256 after training:");
    println!(
        "{:>10}  {:>18}  {:>18}",
        "config", "naive f64", "bitrep exact"
    );
    let mut naive_hashes = Vec::new();
    let mut exact_hashes = Vec::new();
    for &(w, rev) in &configs {
        let nh = train(w, Agg::NaiveF64, rev);
        let eh = train(w, Agg::Exact, rev);
        println!(
            "{:>8}w{}  {:>18}  {:>18}",
            w,
            if rev { " rev" } else { "    " },
            &nh[..16],
            &eh[..16]
        );
        naive_hashes.push(nh);
        exact_hashes.push(eh);
    }
    naive_hashes.sort();
    naive_hashes.dedup();
    exact_hashes.sort();
    exact_hashes.dedup();

    println!(
        "\nnaive f64: {} distinct models across {} configs",
        naive_hashes.len(),
        configs.len()
    );
    println!(
        "bitrep:    {} distinct models across {} configs",
        exact_hashes.len(),
        configs.len()
    );

    assert_eq!(exact_hashes.len(), 1, "exact aggregation must be invariant");
    assert!(
        naive_hashes.len() > 1,
        "naive f64 must differ across configs, else exactness adds nothing here"
    );
    println!("\nPROBE RESULT: LANDS — same model bytes from any worker count or merge order.");
}
