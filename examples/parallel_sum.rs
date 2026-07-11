// Copyright (c) 2026 Kyle Clouthier. Licensed under MIT OR Apache-2.0.
//! Parallel exact summation: linear thread scaling with ZERO determinism
//! caveats. Naive parallel sums change bits with thread count; bitrep's
//! order-invariance means any thread count, any chunking, any merge order —
//! identical bytes. Parallelism is free, correctness-wise.
//!
//! Run: `cargo run --release --example parallel_sum`

use bitrep::{FastSumF64, SumF64};
use std::time::Instant;

fn sum_with_threads(xs: &[f64], threads: usize) -> SumF64 {
    std::thread::scope(|s| {
        let chunk = xs.len().div_ceil(threads);
        let handles: Vec<_> = xs
            .chunks(chunk)
            .map(|sh| {
                s.spawn(move || {
                    let mut a = FastSumF64::new();
                    a.extend_from_slice(sh);
                    a.finish()
                })
            })
            .collect();
        let mut total = SumF64::new();
        for h in handles {
            match h.join() {
                Ok(part) => total.merge(&part),
                Err(_) => unreachable!("worker panicked"),
            }
        }
        total
    })
}

fn main() {
    const N: usize = 20_000_000;
    let mut s = 0x243F_6A88_85A3_08D3u64;
    let xs: Vec<f64> = (0..N)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let mant = (s >> 11) as f64 / (1u64 << 53) as f64 - 0.5;
            mant * 2f64.powi(((s >> 1) % 40) as i32 - 20)
        })
        .collect();

    println!("exact sum of {N} mixed-magnitude f64s:");
    let mut reference: Option<[u8; SumF64::BYTES]> = None;
    for threads in [1usize, 2, 4, 8] {
        let t = Instant::now();
        let total = sum_with_threads(&xs, threads);
        let el = t.elapsed().as_secs_f64();
        let bytes = total.to_bytes();
        let same = match &reference {
            None => {
                reference = Some(bytes);
                true
            }
            Some(r) => *r == bytes,
        };
        println!(
            "  {threads:>2} thread(s): {:>7.1} Melem/s   value {:+.17e}   bytes identical: {same}",
            N as f64 / el / 1e6,
            total.value()
        );
        assert!(same, "thread count changed the bytes — impossible");
    }
    println!("\nAny thread count, same bytes — the naive parallel sum can't say that.");
}
