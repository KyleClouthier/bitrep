# Copyright (c) 2026 Kyle Clouthier. Licensed under MIT OR Apache-2.0.
"""Probe C: bit-identical data-parallel training via exact gradient aggregation.

CLAIM UNDER TEST (scoped, honest):
  In data-parallel SGD the gradient all-reduce is a float sum whose order
  depends on worker count and merge schedule, so the "same" training run
  produces different model bytes with 1 vs 4 vs 16 workers even at fp64.
  Routing ONLY the gradient aggregation through bitrep's exact accumulator
  makes the final model BYTE-IDENTICAL for any worker count / merge order.

SCOPE (named limit): worker-local compute must itself be deterministic and
shard-shape-invariant. Here each per-sample gradient is computed identically
regardless of sharding (batch=1 per sample), which isolates the reduction —
the part bitrep owns. Batch-variant worker kernels are the other half of the
problem (see Thinking Machines' batch-invariance work) and are NOT claimed.

KILL CRITERIA:
  * bitrep arm produces different model hashes across worker counts -> DEAD;
  * naive fp64 arm ALSO produces identical hashes -> bitrep adds nothing
    here -> DEAD (fp64 suffices);
  * exact aggregation changes training outcome vs naive beyond rounding ->
    suspicious, investigate.

Run: python probes/probe_c_training.py   (from repo root)
"""

import hashlib
import struct
import sys

import numpy as np

sys.path.insert(0, "conformance")
from bitrep_ref import Sum  # the pure-Python reference implementation


class SumF64(Sum):
    """Float-in / float-out convenience over the reference's bit-level API."""

    def add(self, x: float):
        self.add_bits(struct.unpack("<Q", struct.pack("<d", x))[0])

    def value(self) -> float:
        return struct.unpack("<d", struct.pack("<Q", self.value_bits()))[0]

# --- tiny MLP, pure numpy, fp64 everywhere -------------------------------
D_IN, D_H = 16, 32
N, EPOCHS, LR = 256, 3, 0.05


def init_params(seed=7):
    r = np.random.default_rng(seed)
    return {
        "w1": r.normal(0, 0.5, (D_IN, D_H)),
        "b1": np.zeros(D_H),
        "w2": r.normal(0, 0.5, (D_H, 1)),
        "b2": np.zeros(1),
    }


def make_data(seed=11):
    r = np.random.default_rng(seed)
    x = r.normal(0, 1.0, (N, D_IN))
    # mixed-magnitude sample weights: makes the reduction order actually bite
    scale = 10.0 ** r.integers(-6, 7, N)
    y = (np.sin(x.sum(axis=1)) > 0).astype(float)
    return x, y, scale


def per_sample_grad(p, xi, yi, wi):
    """Gradient for ONE sample (batch=1: identical no matter how data is
    sharded), scaled by the sample weight wi."""
    h_pre = xi @ p["w1"] + p["b1"]
    h = np.tanh(h_pre)
    o = float(h @ p["w2"][:, 0] + p["b2"][0])
    # weighted squared error
    d_o = 2.0 * (o - yi) * wi
    g_w2 = (h * d_o)[:, None]
    g_b2 = np.array([d_o])
    d_h = p["w2"][:, 0] * d_o * (1.0 - h * h)
    g_w1 = np.outer(xi, d_h)
    g_b1 = d_h
    return {"w1": g_w1, "b1": g_b1, "w2": g_w2, "b2": g_b2}


def shard(indices, workers):
    return [indices[i::workers] for i in range(workers)]


def train(workers, exact, merge_reversed=False):
    """One full training run. Aggregation:
    exact=True  -> per-parameter bitrep accumulators per worker, merged
    exact=False -> naive fp64 partial sums per worker, added in worker order
    """
    p = init_params()
    x, y, scale = make_data()
    keys = ["w1", "b1", "w2", "b2"]
    shapes = {k: p[k].shape for k in keys}
    sizes = {k: p[k].size for k in keys}

    for _ in range(EPOCHS):
        idx = list(range(N))
        shards = shard(idx, workers)

        if exact:
            # each worker: one SumF64 per parameter scalar
            worker_accs = []
            for sh in shards:
                accs = {k: [SumF64() for _ in range(sizes[k])] for k in keys}
                for i in sh:
                    g = per_sample_grad(p, x[i], y[i], scale[i])
                    for k in keys:
                        flat = g[k].ravel()
                        a = accs[k]
                        for j in range(sizes[k]):
                            a[j].add(float(flat[j]))
                worker_accs.append(accs)
            order = list(reversed(worker_accs)) if merge_reversed else worker_accs
            total = {k: [SumF64() for _ in range(sizes[k])] for k in keys}
            for wa in order:
                for k in keys:
                    for j in range(sizes[k]):
                        total[k][j].merge(wa[k][j])
            grad = {
                k: np.array([a.value() for a in total[k]]).reshape(shapes[k])
                for k in keys
            }
        else:
            partials = []
            for sh in shards:
                psum = {k: np.zeros(shapes[k]) for k in keys}
                for i in sh:
                    g = per_sample_grad(p, x[i], y[i], scale[i])
                    for k in keys:
                        psum[k] += g[k]
                partials.append(psum)
            order = list(reversed(partials)) if merge_reversed else partials
            grad = {k: np.zeros(shapes[k]) for k in keys}
            for ps in order:
                for k in keys:
                    grad[k] += ps[k]

        for k in keys:
            p[k] = p[k] - LR * grad[k] / N

    h = hashlib.sha256()
    for k in keys:
        h.update(p[k].tobytes())
    return h.hexdigest()


def main():
    configs = [(1, False), (4, False), (16, False), (4, True)]  # (workers, rev-merge)

    naive = {c: train(c[0], exact=False, merge_reversed=c[1]) for c in configs}
    exact = {c: train(c[0], exact=True, merge_reversed=c[1]) for c in configs}

    print("model-bytes SHA-256 after training:")
    print(f"{'config':>18}  {'naive fp64':>16}  {'bitrep exact':>16}")
    for c in configs:
        tag = f"{c[0]}w{' rev' if c[1] else ''}"
        print(f"{tag:>18}  {naive[c][:16]:>16}  {exact[c][:16]:>16}")

    naive_unique = len(set(naive.values()))
    exact_unique = len(set(exact.values()))
    print(f"\nnaive fp64: {naive_unique} distinct models across {len(configs)} configs")
    print(f"bitrep:     {exact_unique} distinct models across {len(configs)} configs")

    if exact_unique != 1:
        print("PROBE RESULT: DEAD - exact aggregation is not worker-count-invariant")
    elif naive_unique == 1:
        print("PROBE RESULT: DEAD - fp64 already bit-identical here; bitrep adds nothing")
    else:
        print(
            "PROBE RESULT: LANDS - same model bytes from any worker count/merge "
            "order; fp64 is not"
        )


if __name__ == "__main__":
    main()
