# Copyright (c) 2026 Kyle Clouthier / Clouthier Simulation Labs. Licensed under MIT OR Apache-2.0.
"""A pure-Python reference implementation of bitrep's RelSketch canonical
encoding, from the format alone (FORMAT.md, RelSketch section). It exists to
prove the format is language-neutral: this file, written independently of the
Rust crate, must reproduce the Rust `to_bytes()` and `state_hash` byte-for-byte
for `conformance/relsketch_vectors.json`.

Scope: the encoding and the integer bit-shift mapping. The collapse policy
(only triggered by an adversarial all-exponent stream) is not exercised by the
vectors, so this reference keeps collapse_shift = 0; add a collapse pass if you
ever feed it >2^16 distinct buckets.
"""
import hashlib

MASK64 = (1 << 64) - 1
EXP_MASK = 0x7FF
FRAC_MASK = (1 << 52) - 1


def _total_key(bits):
    """Map IEEE-754 f64 bits to an unsigned key ordered by f64::total_cmp."""
    if (bits >> 63) & 1:
        return (~bits) & MASK64
    return bits | (1 << 63)


def _uvarint(x):
    """Minimal unsigned LEB128."""
    out = bytearray()
    while True:
        b = x & 0x7F
        x >>= 7
        if x == 0:
            out.append(b)
            return bytes(out)
        out.append(b | 0x80)


class RelSketch:
    def __init__(self, sub_bits):
        assert 1 <= sub_bits <= 52
        self.sub_bits = sub_bits
        self.collapse_shift = 0
        self.shift = 52 - sub_bits  # collapse_shift = 0 for the vectors
        self.pos = {}
        self.neg = {}
        self.zero = 0
        self.nan = 0
        self.pos_inf = 0
        self.neg_inf = 0
        self.count = 0
        self.min_bits = 0x7FF0000000000000  # +inf sentinel
        self.max_bits = 0xFFF0000000000000  # -inf sentinel
        self.mismatched = False

    def add_bits(self, bits):
        bits &= MASK64
        self.count += 1
        exp = (bits >> 52) & EXP_MASK
        frac = bits & FRAC_MASK
        sign = (bits >> 63) & 1
        if exp == EXP_MASK and frac != 0:  # NaN
            self.nan += 1
            return
        # extrema over every non-NaN sample, IEEE total order
        if _total_key(bits) < _total_key(self.min_bits):
            self.min_bits = bits
        if _total_key(bits) > _total_key(self.max_bits):
            self.max_bits = bits
        if exp == EXP_MASK:  # +/-inf
            if sign:
                self.neg_inf += 1
            else:
                self.pos_inf += 1
        elif exp == 0 and frac == 0:  # +/-0.0
            self.zero += 1
        elif sign == 0:  # positive finite
            key = bits >> self.shift
            self.pos[key] = self.pos.get(key, 0) + 1
        else:  # negative finite: key of |x|
            mag = bits & 0x7FFFFFFFFFFFFFFF
            key = mag >> self.shift
            self.neg[key] = self.neg.get(key, 0) + 1

    def _write_map(self, m):
        out = bytearray(_uvarint(len(m)))
        prev = None
        for k in sorted(m):
            delta = k if prev is None else k - prev
            out += _uvarint(delta)
            out += _uvarint(m[k])
            prev = k
        return bytes(out)

    def to_bytes(self):
        out = bytearray([self.sub_bits, self.collapse_shift, int(self.mismatched)])
        for v in (self.nan, self.pos_inf, self.neg_inf, self.zero,
                  self.min_bits, self.max_bits, self.count):
            out += int(v).to_bytes(8, "little")
        out += self._write_map(self.pos)
        out += self._write_map(self.neg)
        return bytes(out)

    def state_hash(self):
        return hashlib.sha256(self.to_bytes()).digest()

    def merge(self, other):
        assert self.sub_bits == other.sub_bits
        for k, c in other.pos.items():
            self.pos[k] = self.pos.get(k, 0) + c
        for k, c in other.neg.items():
            self.neg[k] = self.neg.get(k, 0) + c
        self.zero += other.zero
        self.nan += other.nan
        self.pos_inf += other.pos_inf
        self.neg_inf += other.neg_inf
        if _total_key(other.min_bits) < _total_key(self.min_bits):
            self.min_bits = other.min_bits
        if _total_key(other.max_bits) > _total_key(self.max_bits):
            self.max_bits = other.max_bits
        self.count += other.count
