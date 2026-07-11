// Copyright (c) 2026 Kyle Clouthier. Licensed under MIT OR Apache-2.0.
//! Raw C-ABI exports for the browser demo — no wasm-bindgen, no JS glue
//! generation, just pointers. Each `unsafe` is a straight dereference of a
//! pointer this module handed out (or a buffer the JS side allocated via
//! `buf_alloc`). The crate under test (`bitrep`) remains `forbid(unsafe)`.

use bitrep::SumF64;

#[no_mangle]
pub extern "C" fn state_len() -> usize {
    SumF64::BYTES
}

/// Allocate a byte buffer the JS side can write into (leaked deliberately —
/// the demo allocates a handful of small buffers for the page's lifetime).
#[no_mangle]
pub extern "C" fn buf_alloc(len: usize) -> *mut u8 {
    let mut v = vec![0u8; len];
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

#[no_mangle]
pub extern "C" fn acc_new() -> *mut SumF64 {
    Box::into_raw(Box::new(SumF64::new()))
}

/// # Safety
/// `a` must be a pointer returned by `acc_new`/`acc_from_bytes`, not yet freed.
#[no_mangle]
pub unsafe extern "C" fn acc_free(a: *mut SumF64) {
    drop(Box::from_raw(a));
}

/// # Safety
/// `a` must be a live accumulator pointer from this module.
#[no_mangle]
pub unsafe extern "C" fn acc_add(a: *mut SumF64, x: f64) {
    (*a).add(x);
}

/// # Safety
/// `a` and `b` must be live accumulator pointers from this module.
#[no_mangle]
pub unsafe extern "C" fn acc_merge(a: *mut SumF64, b: *const SumF64) {
    (*a).merge(&*b);
}

/// # Safety
/// `a` must be a live accumulator pointer from this module.
#[no_mangle]
pub unsafe extern "C" fn acc_value(a: *const SumF64) -> f64 {
    (*a).value()
}

/// # Safety
/// `a` live accumulator; `out` a buffer of at least `state_len()` bytes.
#[no_mangle]
pub unsafe extern "C" fn acc_write_bytes(a: *const SumF64, out: *mut u8) {
    let b = (*a).to_bytes();
    std::ptr::copy_nonoverlapping(b.as_ptr(), out, b.len());
}

/// Returns null if the bytes are not a valid canonical encoding.
///
/// # Safety
/// `p` must point at `state_len()` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn acc_from_bytes(p: *const u8) -> *mut SumF64 {
    let mut buf = [0u8; SumF64::BYTES];
    std::ptr::copy_nonoverlapping(p, buf.as_mut_ptr(), buf.len());
    match SumF64::from_bytes(&buf) {
        Some(a) => Box::into_raw(Box::new(a)),
        None => std::ptr::null_mut(),
    }
}

// ---- the golden dataset, verbatim from tests/golden.rs -------------------
// The page reproduces the exact accumulator state whose SHA-256 is pinned in
// CI across x86-64 Linux, ARM64 macOS, x86-64 Windows and wasm32.

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn f64(&mut self) -> f64 {
        loop {
            let x = f64::from_bits(self.next());
            if x.is_finite() {
                return x;
            }
        }
    }
}

fn golden_data() -> Vec<f64> {
    let mut r = Rng(0x9E3779B97F4A7C15);
    let mut v: Vec<f64> = (0..10_000).map(|_| r.f64()).collect();
    for i in 0..500 {
        let x = f64::from_bits(r.next() & 0x7FEF_FFFF_FFFF_FFFF);
        v.insert((r.next() % v.len() as u64) as usize, x);
        v.insert((r.next() % v.len() as u64) as usize, -x);
        let _ = i;
    }
    v.extend_from_slice(&[
        f64::MAX,
        -f64::MAX,
        f64::MIN_POSITIVE,
        f64::from_bits(1),
        -f64::from_bits(1),
        0.0,
        -0.0,
    ]);
    v
}

#[no_mangle]
pub extern "C" fn golden_len() -> usize {
    golden_data().len()
}

/// # Safety
/// `out` must have room for `golden_len()` f64s.
#[no_mangle]
pub unsafe extern "C" fn golden_fill(out: *mut f64) {
    let d = golden_data();
    std::ptr::copy_nonoverlapping(d.as_ptr(), out, d.len());
}
