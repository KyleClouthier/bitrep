# Copyright (c) 2026 Kyle Clouthier. Licensed under MIT OR Apache-2.0.
"""Cross-language conformance check: the pure-Python reference must
reproduce the Rust crate's canonical bytes and rounded values exactly.

Run:  cargo run --example gen_vectors && python3 conformance/check.py
"""
import json
import os
import random
import sys

sys.path.insert(0, os.path.dirname(__file__))
from bitrep_ref import Sum, F32  # noqa: E402


def run_case(name, input_bits, state_hex, value_bits, fmt=None):
    # Forward order.
    a = Sum()
    for h in input_bits:
        a.add_bits(int(h, 16))
    # Shuffled order + two-shard merge: must be byte-identical.
    shuffled = list(input_bits)
    random.Random(name).shuffle(shuffled)
    cut = len(shuffled) // 3
    left, right = Sum(), Sum()
    for h in shuffled[:cut]:
        left.add_bits(int(h, 16))
    for h in shuffled[cut:]:
        right.add_bits(int(h, 16))
    left.merge(right)

    got_state = a.to_bytes().hex()
    assert got_state == left.to_bytes().hex(), f"{name}: order/shard variance in the reference!"
    assert got_state == state_hex, f"{name}: state mismatch\n  py  {got_state}\n  rust {state_hex}"

    got_value = a.value_bits(fmt) if fmt else a.value_bits()
    want = int(value_bits, 16)
    assert got_value == want, f"{name}: value mismatch py={got_value:x} rust={want:x}"

    # Round-trip the encoding.
    assert Sum.from_bytes(a.to_bytes()).to_bytes() == a.to_bytes(), f"{name}: codec roundtrip"
    print(f"  ok  {name} ({len(input_bits)} inputs)")


def main():
    path = os.path.join(os.path.dirname(__file__), "vectors.json")
    with open(path) as f:
        v = json.load(f)
    assert v["format"] == 1
    for c in v["cases"]:
        run_case(c["name"], c["input_bits"], c["state_hex"], c["value_bits"])
    # f32: inputs are f32 bit patterns; widen to f64 bits exactly, round to f32.
    c = v["f32_case"]
    import struct

    wide = ["%016x" % struct.unpack("<Q", struct.pack("<d", struct.unpack("<f", struct.pack("<I", int(h, 16)))[0]))[0] for h in c["input_bits"]]
    run_case(c["name"] + " (f32)", wide, c["state_hex"], c["value_bits"], fmt=F32)
    print("CONFORMANT: the Python reference reproduces the Rust crate byte-for-byte.")


if __name__ == "__main__":
    main()
