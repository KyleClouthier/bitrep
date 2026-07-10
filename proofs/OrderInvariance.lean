-- Copyright (c) 2026 Kyle Clouthier. Licensed under MIT OR Apache-2.0.
/-
  bitrep — machine-checked order-invariance of exact accumulation.

  MODEL. Every finite f64 is an exact integer multiple of 2^-1074 (its
  "units"); `SumF64.add` adds that integer into a wide two's-complement
  integer, and `merge` adds two such integers. So the accumulator MODEL is:
  state = the integer sum of the units added. This file proves, over that
  model, the crate's headline claims:

   * PERMUTATION INVARIANCE — accumulating a list in ANY order yields the
     same state (`perm_sum_invariant`);
   * SHARD/MERGE-TREE INVARIANCE — splitting the inputs into ANY tree of
     shards and merging in ANY association yields the same state
     (`merge_tree_invariant`);
   * PLACEMENT INVARIANCE — adding a value to either side of a merge yields
     the same state (`add_placement_invariant`).

  HONEST SCOPE (bounded claim, named limit): this is the value-level algebra
  of the accumulator. That the 34-limb two's-complement Rust implementation
  faithfully implements integer addition is checked at the bit level by the
  Kani/CBMC harnesses (src/kani_proofs.rs) for all inputs, and the two layers
  are tied together by the cross-architecture golden vectors. Rounding of the
  final state is proved separately in RoundNearestEven.lean.

  Lean 4 core only; no mathlib; zero `sorry`.
-/

namespace Bitrep

/-- Sum of a list of integers (the accumulator model: state after adding
    every element). -/
def lsum : List Int → Int
  | [] => 0
  | x :: xs => x + lsum xs

/-- Permutations of a list, as the standard inductive relation
    (identity, head-cons, adjacent swap, transitivity). -/
inductive Perm : List Int → List Int → Prop
  | nil : Perm [] []
  | cons (x : Int) {l₁ l₂ : List Int} : Perm l₁ l₂ → Perm (x :: l₁) (x :: l₂)
  | swap (x y : Int) (l : List Int) : Perm (x :: y :: l) (y :: x :: l)
  | trans {l₁ l₂ l₃ : List Int} : Perm l₁ l₂ → Perm l₂ l₃ → Perm l₁ l₃

/-- THEOREM (permutation invariance): accumulating in any order gives the
    same exact state. `add` is the only state mutation, and its model is
    integer addition, so this is precisely the crate's order-invariance. -/
theorem perm_sum_invariant {l₁ l₂ : List Int} (h : Perm l₁ l₂) :
    lsum l₁ = lsum l₂ := by
  induction h with
  | nil => rfl
  | cons x _ ih => simp [lsum, ih]
  | swap x y l => simp [lsum]; omega
  | trans _ _ ih₁ ih₂ => exact ih₁.trans ih₂

/-- Appending input lists adds their sums: the two-shard case. -/
theorem lsum_append (l₁ l₂ : List Int) :
    lsum (l₁ ++ l₂) = lsum l₁ + lsum l₂ := by
  induction l₁ with
  | nil => simp [lsum]
  | cons x xs ih => simp [lsum, ih]; omega

/-- A merge tree: leaves are shards (lists of inputs), nodes are `merge`. -/
inductive Tree where
  | leaf : List Int → Tree
  | node : Tree → Tree → Tree

/-- The inputs a tree covers, flattened left to right. -/
def Tree.leaves : Tree → List Int
  | .leaf l => l
  | .node a b => a.leaves ++ b.leaves

/-- The accumulator state a tree of merges produces: each leaf accumulates
    its shard; each node merges (adds) its children. -/
def Tree.sum : Tree → Int
  | .leaf l => lsum l
  | .node a b => a.sum + b.sum

/-- THEOREM (merge-tree invariance): ANY tree of shard-merges over the same
    inputs yields exactly the state of accumulating the whole input
    sequentially. Combined with `perm_sum_invariant`, any sharding, any merge
    association, any order — same state. -/
theorem merge_tree_invariant (t : Tree) :
    t.sum = lsum t.leaves := by
  induction t with
  | leaf l => rfl
  | node a b iha ihb => simp [Tree.sum, Tree.leaves, iha, ihb, lsum_append]

/-- THEOREM (placement invariance): adding a value into the left or the
    right operand of a merge yields the same merged state. -/
theorem add_placement_invariant (x a b : Int) :
    (x + a) + b = a + (x + b) := by omega

/-- Merging commutes (shards can arrive in any order). -/
theorem merge_comm (a b : Int) : a + b = b + a := by omega

/-- Merging associates (any merge tree — restated pointwise). -/
theorem merge_assoc (a b c : Int) : (a + b) + c = a + (b + c) := by omega

end Bitrep
