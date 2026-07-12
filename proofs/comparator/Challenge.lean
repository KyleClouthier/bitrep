-- Copyright (c) 2026 Kyle Clouthier. Licensed under MIT OR Apache-2.0.
/-
  bitrep — the TRUSTED STATEMENT SPEC for the comparator CI job.

  This file is the human-auditable list of what the Lean proofs claim:
  every definition the theorem statements depend on (copied VERBATIM from
  proofs/OrderInvariance.lean, proofs/RoundNearestEven.lean,
  proofs/FloatGCounter.lean and proofs/ToolkitAlgebra.lean), plus each of
  the 34 audited theorems restated verbatim with a `sorry` proof.

  The comparator (github.com/leanprover/comparator) then guarantees that
  Solution.lean — the concatenation of the four real proof files — proves
  EXACTLY these statements, using only the axioms propext / Quot.sound /
  Classical.choice, and that the proofs replay in the Lean kernel.

  Read the doc comments in the four proof files for the model and the
  honest-scope notes; this file intentionally repeats none of them.
-/

namespace Bitrep

/-! ## Order invariance of exact accumulation (OrderInvariance.lean) -/

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

theorem perm_sum_invariant {l₁ l₂ : List Int} (h : Perm l₁ l₂) :
    lsum l₁ = lsum l₂ := sorry

theorem lsum_append (l₁ l₂ : List Int) :
    lsum (l₁ ++ l₂) = lsum l₁ + lsum l₂ := sorry

theorem merge_tree_invariant (t : Tree) :
    t.sum = lsum t.leaves := sorry

theorem add_placement_invariant (x a b : Int) :
    (x + a) + b = a + (x + b) := sorry

theorem merge_comm (a b : Int) : a + b = b + a := sorry

theorem merge_assoc (a b c : Int) : (a + b) + c = a + (b + c) := sorry

/-! ## The rounding kernel is round-to-nearest, ties-to-even
    (RoundNearestEven.lean) -/

/-- The rounding kernel, exactly as implemented. -/
def roundAt (n g : Nat) : Nat :=
  if n % 2 ^ g > 2 ^ (g - 1) ∨ (n % 2 ^ g = 2 ^ (g - 1) ∧ (n / 2 ^ g) % 2 = 1) then
    n / 2 ^ g + 1
  else
    n / 2 ^ g

/-- |2^g·k − n| in ℕ: with truncated subtraction, (a−b)+(b−a) = |a−b|. -/
def gdist (n g k : Nat) : Nat :=
  (2 ^ g * k - n) + (n - 2 ^ g * k)

theorem roundAt_half_ulp (n g : Nat) (hg : 1 ≤ g) :
    gdist n g (roundAt n g) ≤ 2 ^ (g - 1) := sorry

theorem roundAt_nearest (n g : Nat) (hg : 1 ≤ g) (k : Nat) :
    gdist n g (roundAt n g) ≤ gdist n g k := sorry

theorem roundAt_ties_even (n g : Nat) (_hg : 1 ≤ g)
    (htie : n % 2 ^ g = 2 ^ (g - 1)) :
    roundAt n g % 2 = 0 := sorry

theorem roundAt_exact (n g : Nat) (_hg : 1 ≤ g) (hex : n % 2 ^ g = 0) :
    roundAt n g * 2 ^ g = n := sorry

/-! ## Float G-Counter convergence laws (FloatGCounter.lean) -/

/-- Replica-entry join: count-wins. On an append-only history, the larger
    count strictly contains the smaller, so max loses nothing. -/
def ejoin (a b : Nat) : Nat := Nat.max a b

/-- A counter state: how many adds of each replica this state contains. -/
def GC (R : Nat) := Fin R → Nat

/-- The CRDT join: pointwise count-wins. -/
def join {R : Nat} (s t : GC R) : GC R := fun r => ejoin (s r) (t r)

/-- The empty state (knows no adds). -/
def bot {R : Nat} : GC R := fun _ => 0

/-- The state a replica holds after receiving a list of snapshots. -/
def joinAll {R : Nat} : List (GC R) → GC R
  | [] => bot
  | s :: rest => join s (joinAll rest)

/-- Permutations of snapshot lists (same four generators as
    OrderInvariance.Perm, at this type). -/
inductive PermG {R : Nat} : List (GC R) → List (GC R) → Prop
  | nil : PermG [] []
  | cons (s : GC R) {l₁ l₂ : List (GC R)} :
      PermG l₁ l₂ → PermG (s :: l₁) (s :: l₂)
  | swap (s t : GC R) (l : List (GC R)) :
      PermG (s :: t :: l) (t :: s :: l)
  | trans {l₁ l₂ l₃ : List (GC R)} :
      PermG l₁ l₂ → PermG l₂ l₃ → PermG l₁ l₃

/-- The first n values of an add-stream, oldest first. -/
def taken (f : Nat → Int) : Nat → List Int
  | 0 => []
  | n + 1 => taken f n ++ [f n]

/-- Sum of `g` over replicas 0..k-1 (the merge of all entries; merge order
    is irrelevant by `merge_comm`/`merge_assoc`, proven in
    OrderInvariance.lean). -/
def sumOver (k : Nat) (g : Nat → Int) : Int :=
  match k with
  | 0 => 0
  | k + 1 => sumOver k g + g k

/-- The value a state reads, given each replica's add-stream: the merge of
    every entry's exact sum. -/
def read {R : Nat} (streams : Fin R → Nat → Int) (s : GC R) : Int :=
  sumOver R (fun r => if h : r < R then lsum (taken (streams ⟨r, h⟩) (s ⟨r, h⟩)) else 0)

/-- The full-knowledge state: every add of every replica, where replica r
    has performed `total r` adds. -/
def full {R : Nat} (total : Fin R → Nat) : GC R := total

theorem join_comm {R : Nat} (s t : GC R) : join s t = join t s := sorry

theorem join_assoc {R : Nat} (s t u : GC R) :
    join (join s t) u = join s (join t u) := sorry

theorem join_idem {R : Nat} (s : GC R) : join s s = s := sorry

theorem joinAll_perm_invariant {R : Nat} {l₁ l₂ : List (GC R)}
    (h : PermG l₁ l₂) : joinAll l₁ = joinAll l₂ := sorry

theorem joinAll_dup_invariant {R : Nat} (s : GC R) (l : List (GC R)) :
    joinAll (s :: s :: l) = joinAll (s :: l) := sorry

theorem le_join_left {R : Nat} (s t : GC R) (r : Fin R) :
    s r ≤ join s t r := sorry

theorem full_absorbs {R : Nat} (total : Fin R → Nat) (s : GC R)
    (hb : ∀ r, s r ≤ total r) : join (full total) s = full total := sorry

theorem read_full_exact {R : Nat} (streams : Fin R → Nat → Int)
    (total : Fin R → Nat) :
    read streams (full total) =
      sumOver R (fun r => if h : r < R then
        lsum (taken (streams ⟨r, h⟩) (total ⟨r, h⟩)) else 0) := sorry

/-! ## Toolkit merge algebra (ToolkitAlgebra.lean) -/

/-- Componentwise merge on a pair. Every multi-accumulator toolkit state
    (Moments, Cov, Weighted, Pn, CovMatrix, Histogram) is an iterated pair
    of component states merged this way. -/
def prodMerge (f : α → α → α) (g : β → β → β) (p q : α × β) : α × β :=
  (f p.1 q.1, g p.2 q.2)

/-- Pointwise merge of keyed families. -/
def mapMerge (f : α → α → α) (m n : κ → α) : κ → α :=
  fun k => f (m k) (n k)

/-- Saturating addition with cap `c`. -/
def satAdd (c a b : Nat) : Nat := min c (a + b)

theorem prod_merge_comm (f : α → α → α) (g : β → β → β)
    (hf : ∀ a b, f a b = f b a) (hg : ∀ a b, g a b = g b a) :
    ∀ p q, prodMerge f g p q = prodMerge f g q p := sorry

theorem prod_merge_assoc (f : α → α → α) (g : β → β → β)
    (hf : ∀ a b c, f (f a b) c = f a (f b c))
    (hg : ∀ a b c, g (g a b) c = g a (g b c)) :
    ∀ p q r, prodMerge f g (prodMerge f g p q) r
           = prodMerge f g p (prodMerge f g q r) := sorry

theorem prod_merge_idem (f : α → α → α) (g : β → β → β)
    (hf : ∀ a, f a a = a) (hg : ∀ a, g a a = a) :
    ∀ p, prodMerge f g p p = p := sorry

theorem map_merge_comm (f : α → α → α)
    (hf : ∀ a b, f a b = f b a) :
    ∀ m n : κ → α, mapMerge f m n = mapMerge f n m := sorry

theorem map_merge_assoc (f : α → α → α)
    (hf : ∀ a b c, f (f a b) c = f a (f b c)) :
    ∀ m n o : κ → α, mapMerge f (mapMerge f m n) o
                   = mapMerge f m (mapMerge f n o) := sorry

theorem max_join_comm : ∀ a b : Nat, max a b = max b a := sorry

theorem max_join_assoc : ∀ a b c : Nat, max (max a b) c = max a (max b c) := sorry

theorem max_join_idem : ∀ a : Nat, max a a = a := sorry

theorem min_join_comm : ∀ a b : Nat, min a b = min b a := sorry

theorem min_join_assoc : ∀ a b c : Nat, min (min a b) c = min a (min b c) := sorry

theorem min_join_idem : ∀ a : Nat, min a a = a := sorry

theorem or_join_comm : ∀ a b : Bool, (a || b) = (b || a) := sorry

theorem or_join_assoc : ∀ a b c : Bool, ((a || b) || c) = (a || (b || c)) := sorry

theorem or_join_idem : ∀ a : Bool, (a || a) = a := sorry

theorem satAdd_comm (c : Nat) : ∀ a b, satAdd c a b = satAdd c b a := sorry

theorem satAdd_assoc (c : Nat) :
    ∀ a b d, satAdd c (satAdd c a b) d = satAdd c a (satAdd c b d) := sorry

end Bitrep
