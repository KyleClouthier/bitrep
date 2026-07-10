# The bitrep accumulator format (v1)

A language-neutral specification of the 289-byte canonical accumulator state.
Any implementation that follows this page interoperates bit-for-bit with any
other — the repository ships conformance vectors (`conformance/vectors.json`)
and a pure-Python reference implementation that proves it.

## State

An accumulator is three fields:

| field  | size | meaning |
|--------|------|---------|
| `acc`  | 2176-bit two's-complement integer (34 × u64 little-endian limbs, least-significant limb first) | the exact sum, in units of 2⁻¹⁰⁷⁴ |
| `flags`| 1 byte | bit 0 = saw NaN · bit 1 = saw +∞ · bit 2 = saw −∞ · bits 3–7 MUST be zero |
| `count`| u64 little-endian | number of `add` operations (saturating) |

**Encoding** (`to_bytes`, 289 bytes): limbs 0..34 as little-endian u64s
(272 bytes), then `flags` (1 byte), then `count` (8 bytes).
Decoders MUST reject a nonzero bit in flags bits 3–7.

## Adding a finite f64

Let `bits` be the IEEE-754 binary64 encoding.

```
sign = bits >> 63
E    = (bits >> 52) & 0x7FF
frac = bits & (2^52 - 1)

E = 0x7FF          -> set the NaN flag (frac != 0) or the ±∞ flag; return
E = 0, frac = 0    -> ±0.0: no-op (count still increments)
E = 0 (subnormal)  -> m = frac,          e = -1074
otherwise          -> m = frac + 2^52,   e = E - 1075
```

Then `acc += (-1)^sign * m * 2^(e + 1074)` as a plain (wide) integer add.
Every finite f64 lands in bit positions 0..=2097; bits 2098..=2175 are
headroom sized for 2⁶³ additions of the largest finite value, so the
two's-complement integer never wraps within that documented capacity.
`count` increments (saturating at 2⁶⁴−1).

## Merge

`acc += other.acc` (2176-bit add, wrap ignored per the headroom bound),
`flags |= other.flags`, `count = saturating(count + other.count)`.
Because integer addition is associative and commutative, any merge tree over
the same inputs yields identical bytes (proved in
`proofs/OrderInvariance.lean`).

## Reading the value (round to nearest, ties to even)

If NaN flag, or both ∞ flags: NaN. One ∞ flag: that infinity.
Otherwise interpret `acc` as a signed integer `n` (units 2⁻¹⁰⁷⁴); take
`mag = |n|`, remember the sign, and let `h` = index of the highest set bit
(if `mag = 0` return `+0.0` — canonical zero).

For target format (f64: `mant` = 52, `min_exp` = −1022, `max_exp` = 1023;
f32: 23 / −126 / 127):

```
if h <= min_exp + 1074 - 1:            # subnormal target
    grid = min_exp - mant + 1074       # f64: 0 (exact), f32: 925
    exp  = min_exp
else:                                  # normal target
    grid = h - mant
    exp  = h - 1074

q = round_at(mag, grid)                # see kernel below
if q >= 2^(mant+1): q >>= 1; exp += 1  # rounding carried out a bit
if exp > max_exp: return ±infinity
return (-1)^sign * q * 2^(exp - mant)
```

The rounding kernel (`round_at`) is the round/sticky rule:

```
q = mag >> grid
if grid == 0: return q                 # exact
half   = 1 << (grid - 1)
r      = mag & ((1 << grid) - 1)
if r > half or (r == half and q is odd): q += 1
return q
```

`round_at` is proven round-to-nearest-ties-to-even in
`proofs/RoundNearestEven.lean` (half-ulp bound, minimality over all grid
multiples, tie parity, exactness), machine-checked with zero `sorry`.

For f32 results the same state is rounded **once**, directly to the f32
grid — never through an intermediate f64 (which double-rounds).

## Conformance

An implementation conforms iff, for `conformance/vectors.json`:
1. accumulating each vector's inputs (any order) yields the listed
   289-byte state, and
2. the rounded value equals the listed IEEE-754 bit pattern.
