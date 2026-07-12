# bitrep (JavaScript / WebAssembly)

Exact, **order-invariant, bit-identical** floating-point reductions and CRDTs —
a WebAssembly binding over the Rust [`bitrep`](https://github.com/KyleClouthier/bitrep)
engine, so every result is the same across any machine, any merge order, any
sharding. Runs in the browser and in Node.

```bash
npm install bitrep
```

```js
import init, { SumF64, MomentsF64, SumMap } from "bitrep";
await init(); // load the wasm

// Exact sum — naive float gives 0; this gives 1, and it's order-invariant.
const s = new SumF64();
[1e16, 1.0, -1e16].forEach((x) => s.add(x));
console.log(s.value());            // 1

// Exactly-rounded statistics
const m = new MomentsF64();
[2, 4, 4, 4, 5, 5, 7, 9].forEach((x) => m.add(x));
console.log(m.mean(), m.variance()); // 5 4

// Receipt: same multiset -> same 32-byte hash regardless of order.
console.log(Buffer.from(s.state_hash()).toString("hex"));

// CRDT map that converges without a coordinator
const a = new SumMap(); a.add("k", 1.5); a.add("k", 2.5);
const b = new SumMap(); b.add("k", 2.5); b.add("k", 1.5);
a.merge(b); // idempotent, order-invariant
```

## What's included
Exact accumulators (`SumF64`, `SumF32`, `FastSumF64`), exact dot product
(`DotF64`, `dot`), convergent statistics (`MomentsF64`, `Moments4F64` with
skewness/kurtosis, `CovF64` regression, `WeightedMomentsF64`, `PnMomentsF64`
retractable, `CovMatrixF64` multiple regression), `HistogramF64` (exact counts +
honest quantile bounds), `ExtremaF64`, signed **receipts** (`state_hash`), and the
CRDT layer (`SumMap`, `MomentsMap`, `ReplicatedSum`, `DeltasSum`).

The convergence laws of the core are machine-checked in Lean 4. See the
[main repository](https://github.com/KyleClouthier/bitrep) for proofs, the Rust
crate, and the paper.

License: MIT OR Apache-2.0.
