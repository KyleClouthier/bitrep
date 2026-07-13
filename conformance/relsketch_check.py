# Copyright (c) 2026 Kyle Clouthier / Clouthier Simulation Labs. Licensed under MIT OR Apache-2.0.
"""Cross-language conformance check for RelSketch: the pure-Python reference
must reproduce the Rust crate's canonical bytes AND state_hash exactly, and be
order/shard invariant in its own right.

Run:  cargo run --features "quantile receipts" --example gen_relsketch_vectors
      && python3 conformance/relsketch_check.py
"""
import json
import os
import random
import sys

sys.path.insert(0, os.path.dirname(__file__))
from relsketch_ref import RelSketch  # noqa: E402


def run_case(case):
    name = case["name"]
    sub_bits = case["sub_bits"]
    inputs = [int(h, 16) for h in case["input_bits"]]

    # Forward order.
    a = RelSketch(sub_bits)
    for b in inputs:
        a.add_bits(b)

    # Shuffled order + three-shard merge: must be byte-identical.
    shuffled = list(inputs)
    random.Random(name).shuffle(shuffled)
    shards = [RelSketch(sub_bits) for _ in range(3)]
    for i, b in enumerate(shuffled):
        shards[i % 3].add_bits(b)
    merged = RelSketch(sub_bits)
    for s in shards:
        merged.merge(s)

    got = a.to_bytes().hex()
    assert got == merged.to_bytes().hex(), f"{name}: order/shard variance in the reference!"
    assert got == case["state_hex"], (
        f"{name}: state mismatch\n  py   {got}\n  rust {case['state_hex']}"
    )
    assert a.state_hash().hex() == case["state_hash"], f"{name}: state_hash mismatch"
    print(f"  ok  {name} ({len(inputs)} inputs, sub_bits={sub_bits})")


def main():
    path = os.path.join(os.path.dirname(__file__), "relsketch_vectors.json")
    with open(path) as f:
        v = json.load(f)
    for c in v["cases"]:
        run_case(c)
    print("CONFORMANT: the Python RelSketch reference reproduces the Rust crate byte-for-byte.")


if __name__ == "__main__":
    main()
