# Copyright (c) 2026 Kyle Clouthier / Clouthier Simulation Labs. Licensed under MIT OR Apache-2.0.
"""Pure-Python reference implementation of the bitrep accumulator (FORMAT.md).

Deliberately independent of the Rust code: Python's unbounded integers play
the role of the 2176-bit limb array. Interoperability is proven by
check.py against conformance/vectors.json, which the Rust crate generates.
"""

LIMBS = 34
BITS = LIMBS * 64
MASK = (1 << BITS) - 1  # two's-complement domain
BYTES = LIMBS * 8 + 1 + 8

F64 = (52, -1022, 1023)
F32 = (23, -126, 127)


class Sum:
    """Exact, order-invariant, mergeable sum of IEEE binary64 values."""

    def __init__(self):
        self.acc = 0  # two's-complement integer in units of 2^-1074, mod 2^BITS
        self.nan = False
        self.pos_inf = False
        self.neg_inf = False
        self.count = 0

    def add_bits(self, bits: int):
        """Add one f64 given as its 64-bit IEEE encoding."""
        self.count = min(self.count + 1, (1 << 64) - 1)
        sign = bits >> 63
        e_field = (bits >> 52) & 0x7FF
        frac = bits & ((1 << 52) - 1)
        if e_field == 0x7FF:
            if frac:
                self.nan = True
            elif sign:
                self.neg_inf = True
            else:
                self.pos_inf = True
            return
        if e_field == 0:
            if frac == 0:
                return  # +-0.0
            m, e = frac, -1074
        else:
            m, e = frac | (1 << 52), e_field - 1075
        units = m << (e + 1074)
        self.acc = (self.acc + (-units if sign else units)) & MASK

    def merge(self, other: "Sum"):
        self.acc = (self.acc + other.acc) & MASK
        self.nan |= other.nan
        self.pos_inf |= other.pos_inf
        self.neg_inf |= other.neg_inf
        self.count = min(self.count + other.count, (1 << 64) - 1)

    # -- encoding ---------------------------------------------------------

    def to_bytes(self) -> bytes:
        flags = self.nan | (self.pos_inf << 1) | (self.neg_inf << 2)
        return (
            self.acc.to_bytes(LIMBS * 8, "little")
            + bytes([flags])
            + self.count.to_bytes(8, "little")
        )

    @classmethod
    def from_bytes(cls, b: bytes) -> "Sum":
        assert len(b) == BYTES
        flags = b[LIMBS * 8]
        if flags & ~0b111:
            raise ValueError("unknown flag bits")
        s = cls()
        s.acc = int.from_bytes(b[: LIMBS * 8], "little")
        s.nan, s.pos_inf, s.neg_inf = bool(flags & 1), bool(flags & 2), bool(flags & 4)
        s.count = int.from_bytes(b[LIMBS * 8 + 1 :], "little")
        return s

    # -- rounding (FORMAT.md kernel; proven in RoundNearestEven.lean) ------

    def value_bits(self, fmt=F64) -> int:
        """The correctly rounded value as IEEE bits (f64 by default)."""
        mant, min_exp, max_exp = fmt
        width = 64 if fmt == F64 else 32
        e_bits = 11 if fmt == F64 else 8
        if self.nan or (self.pos_inf and self.neg_inf):
            return (((1 << e_bits) - 1) << mant) | (1 << (mant - 1))  # quiet NaN
        if self.pos_inf or self.neg_inf:
            inf = ((1 << e_bits) - 1) << mant
            return inf | (1 << (width - 1)) if self.neg_inf else inf
        n = self.acc if self.acc < (1 << (BITS - 1)) else self.acc - (1 << BITS)
        if n == 0:
            return 0  # canonical +0.0
        sign, mag = (1, -n) if n < 0 else (0, n)
        h = mag.bit_length() - 1
        if h <= min_exp + 1074 - 1:
            grid, exp = min_exp - mant + 1074, min_exp
        else:
            grid, exp = h - mant, h - 1074
        q = round_at(mag, grid)
        if q >> (mant + 1):
            q >>= 1
            exp += 1
        if exp > max_exp:
            return (sign << (width - 1)) | (((1 << e_bits) - 1) << mant)  # +-inf
        # assemble: value = q * 2^(exp - mant); subnormal iff q < 2^mant
        if q < (1 << mant):
            frac, e_field = q, 0
        else:
            frac, e_field = q & ((1 << mant) - 1), exp + (max_exp - 0) + 1 - 1
            e_field = exp + ((1 << (e_bits - 1)) - 1)  # exp + bias
        return (sign << (width - 1)) | (e_field << mant) | frac


def round_at(mag: int, grid: int) -> int:
    """Round-to-nearest, ties-to-even at grid 2^grid (FORMAT.md kernel)."""
    q = mag >> grid
    if grid == 0:
        return q
    half = 1 << (grid - 1)
    r = mag & ((1 << grid) - 1)
    if r > half or (r == half and q & 1):
        q += 1
    return q
