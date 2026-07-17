# bitrep (Python)

Exact, **order-invariant, bit-identical** floating-point reductions and CRDTs —
a thin Python binding over the Rust [`bitrep`](https://github.com/KyleClouthier/bitrep)
engine, so every result is the same across any machine, any merge order, any
sharding.

```bash
pip install bitrep
```

```python
import bitrep

# Exact sum — naive float gives 0.0; this gives 1.0, and it's order-invariant.
s = bitrep.SumF64()
for x in (1e16, 1.0, -1e16):
    s.add(x)
print(s.value())            # 1.0

# Exactly-rounded statistics
m = bitrep.MomentsF64()
for x in (2, 4, 4, 4, 5, 5, 7, 9):
    m.add(x)
print(m.mean(), m.variance(), m.count())   # 5.0 4.0 8

# Receipt: same multiset -> same 32-byte hash, regardless of order.
print(s.state_hash().hex())

# CRDT map that converges without a coordinator
a = bitrep.SumMap(); a.add("k", 1.5); a.add("k", 2.5)
b = bitrep.SumMap(); b.add("k", 2.5); b.add("k", 1.5)
a.merge(b)                   # idempotent, order-invariant
```

## What's included
Exact accumulators (`SumF64`, `SumF32`, `FastSumF64`), exact dot product
(`DotF64`, `dot`), convergent statistics (`MomentsF64`, `Moments4F64` with
skewness/kurtosis, `CovF64` regression, `WeightedMomentsF64`, `PnMomentsF64`
retractable, `CovMatrixF64` multiple regression — with the v0.5 exact tier:
`regression_exact()` for correctly rounded coefficients and `sub()` for exact
downdating/unlearning, plus `SumF64.try_unmerge`), `HistogramF64` (exact counts +
honest quantile bounds), `ExtremaF64`, signed **receipts** (`state_hash`), and the
CRDT layer (`SumMap`, `MomentsMap`, `ReplicatedSum`, `DeltasSum`).

The convergence laws of the core are machine-checked in Lean 4. See the
[main repository](https://github.com/KyleClouthier/bitrep) for proofs, the Rust
crate, and the paper.

License: MIT OR Apache-2.0.
