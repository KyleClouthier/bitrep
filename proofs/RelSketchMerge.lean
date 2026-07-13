-- Copyright (c) 2026 Kyle Clouthier / Clouthier Simulation Labs. Licensed under MIT OR Apache-2.0.
/-
  bitrep — machine-checked merge laws for the RelSketch quantile sketch.

  MODEL. A RelSketch's bucket store is a sparse map from bucket key to an
  integer count. Modeled faithfully (as FloatGCounter models a counter state
  by a total function `Fin R → Nat`, absent keys holding 0), a bucket store is
  a total function `key → Nat`, and a key absent from the sparse map holds
  count 0 — the identity of the merge. The merge sums counts per key.

  This file proves, over that pointwise model, the laws that make a RelSketch a
  lawful mergeable state — the same contract SumF64 has, now for the sketch's
  bucket counts:

   * MERGE IS COMMUTATIVE and ASSOCIATIVE — any shard order or merge tree over
     the same buckets yields the same counts (`relsketch_merge_comm`,
     `relsketch_merge_assoc`); this is what makes the canonical byte encoding
     order/shard/merge-invariant, hence the state hash a signable receipt.
   * EMPTY IS THE IDENTITY — merging in an empty sketch changes nothing
     (`relsketch_merge_empty`).

  Unlike the count-wins join of FloatGCounter, this merge is a SUM, so it is
  deliberately NOT idempotent — re-merging the same shard double-counts, as in
  every counter CRDT; deduplication is the replica-map layer's job. The full
  RelSketch state is a PRODUCT of two such bucket maps (positive and negative)
  and the scalar special counters (NaN / ±∞ / zero / total), each merged the
  same way; ToolkitAlgebra.lean's `prod_merge_*` and `satAdd_*` lift these
  componentwise laws to the whole state and its saturating counters.

  HONEST SCOPE (bounded claim, named limits): this is the value-level algebra
  of the bucket counts. That the Rust `RelSketch::merge` realizes it bit-for-bit
  — sparse map merge, canonical delta-varint encoding, the collapse policy, the
  saturating cap on each u64 count — is the job of the differential/red-team
  tests (tests/quantile_redteam.rs) and the decode fuzzer; the saturating cap
  itself is ToolkitAlgebra's `satAdd_comm`/`satAdd_assoc`. Lean 4 core only; no
  mathlib; zero `sorry`.
-/

namespace Bitrep

/-! ### Bucket stores and their merge -/

/-- A bucket store: the integer count held at each key (absent keys hold 0). -/
def Buckets (κ : Type) := κ → Nat

/-- The RelSketch bucket merge: sum counts per key. -/
def bmerge {κ : Type} (m n : Buckets κ) : Buckets κ := fun k => m k + n k

/-- The empty store (no bucket occupied). -/
def bempty {κ : Type} : Buckets κ := fun _ => 0

/-- COMMUTATIVITY: shard order never changes the merged counts. -/
theorem relsketch_merge_comm {κ : Type} (m n : Buckets κ) :
    bmerge m n = bmerge n m :=
  funext fun k => Nat.add_comm (m k) (n k)

/-- ASSOCIATIVITY: any merge-tree shape over the same shards agrees. -/
theorem relsketch_merge_assoc {κ : Type} (m n p : Buckets κ) :
    bmerge (bmerge m n) p = bmerge m (bmerge n p) :=
  funext fun k => Nat.add_assoc (m k) (n k) (p k)

/-- IDENTITY: merging an empty sketch in changes nothing. -/
theorem relsketch_merge_empty {κ : Type} (m : Buckets κ) :
    bmerge bempty m = m :=
  funext fun k => Nat.zero_add (m k)

end Bitrep
