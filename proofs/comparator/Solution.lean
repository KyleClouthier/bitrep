-- Copyright (c) 2026 Kyle Clouthier / Clouthier Simulation Labs. Licensed under MIT OR Apache-2.0.
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

/-- Exact-tier (v0.5.0): unmerging is the group inverse of merging at the
model level — subtracting a merged contribution restores the original state
exactly. Bit-level counterpart: the `unmerge_inverts_merge` Kani harness
(borrow chain inverts carry chain across all limbs). -/
theorem unmerge_inverts_merge (a b : Int) : (a + b) - b = a := by omega

/-- List form: removing a sub-list's contribution from a concatenation's sum
yields exactly the remaining list's sum — the model statement of exact
downdating / verifiable unlearning (probe475, U1). -/
theorem lsum_unmerge (l₁ l₂ : List Int) :
    lsum (l₁ ++ l₂) - lsum l₂ = lsum l₁ := by
  rw [lsum_append]; omega

end Bitrep

-- Copyright (c) 2026 Kyle Clouthier / Clouthier Simulation Labs. Licensed under MIT OR Apache-2.0.
/-
  bitrep — machine-checked correctness of the rounding kernel.

  The crate's `round_at` reduces an exact magnitude n (in units of 2^-1074)
  to a significand on a coarser grid 2^g by the round-bit/sticky-bit rule:

      q  = n / 2^g            (kept bits)     r = n % 2^g   (discarded bits)
      up = (r > 2^(g-1)) or (r = 2^(g-1) and q odd)
      result = q + (if up then 1 else 0)

  This file proves that rule IS round-to-nearest, ties-to-even:

   * HALF-ULP     — |result·2^g − n| ≤ 2^(g-1)                (`roundAt_half_ulp`)
   * NEAREST      — |result·2^g − n| ≤ |k·2^g − n| for EVERY k (`roundAt_nearest`)
   * TIES-TO-EVEN — an exact halfway n rounds to an even significand
                                                              (`roundAt_ties_even`)
   * EXACTNESS    — grid values round to themselves            (`roundAt_exact`)

  HONEST SCOPE (bounded claim, named limit): this proves the arithmetic
  kernel on magnitudes — the mathematically substantive step; the sign is
  carried separately, and the f64 ENCODING around the kernel (exponent
  packing, subnormal boundary, overflow to ±∞) is exercised against an
  independent BigInt oracle, the NIST StRD datasets, and the cross-
  architecture golden vectors, while the Rust implementation of this kernel
  is bit-level checked by Kani. One property, three independent watchdogs.

  Lean 4 core only; no mathlib; zero `sorry`.
  Distances live in ℕ: with truncated subtraction, (a−b)+(b−a) = |a−b|,
  which keeps every proof inside `omega`'s linear fragment; the only
  nonlinear steps are monotonicity of multiplication, taken as explicit
  `Nat.mul_le_mul_left` facts. Products are oriented `2^g * k` to match
  core's `Nat.div_add_mod`.
-/

namespace Bitrep

/-- The rounding kernel, exactly as implemented. -/
def roundAt (n g : Nat) : Nat :=
  if n % 2 ^ g > 2 ^ (g - 1) ∨ (n % 2 ^ g = 2 ^ (g - 1) ∧ (n / 2 ^ g) % 2 = 1) then
    n / 2 ^ g + 1
  else
    n / 2 ^ g

/-- |2^g·k − n| in ℕ: with truncated subtraction, (a−b)+(b−a) = |a−b|. -/
def gdist (n g k : Nat) : Nat :=
  (2 ^ g * k - n) + (n - 2 ^ g * k)

/-- 2^(g-1) + 2^(g-1) = 2^g for g ≥ 1. -/
theorem two_halves {g : Nat} (h : 1 ≤ g) : 2 ^ (g - 1) + 2 ^ (g - 1) = 2 ^ g := by
  have hs : g - 1 + 1 = g := by omega
  have hp : 2 ^ (g - 1 + 1) = 2 ^ (g - 1) * 2 := Nat.pow_succ ..
  rw [hs] at hp
  omega

/-- Shared linear facts about q, r, 2^g, half, and the rounded-up grid
    point. Everything downstream of these is pure `omega`. -/
theorem kernel_facts (n g : Nat) (hg : 1 ≤ g) :
    2 ^ (g - 1) + 2 ^ (g - 1) = 2 ^ g ∧
    2 ^ g * (n / 2 ^ g) + n % 2 ^ g = n ∧
    n % 2 ^ g < 2 ^ g ∧
    2 ^ g * (n / 2 ^ g + 1) = 2 ^ g * (n / 2 ^ g) + 2 ^ g :=
  ⟨two_halves hg, Nat.div_add_mod n (2 ^ g),
   Nat.mod_lt _ (Nat.two_pow_pos g), Nat.mul_succ ..⟩

/-- HALF-ULP: the kernel's result is within half a grid step of n. -/
theorem roundAt_half_ulp (n g : Nat) (hg : 1 ≤ g) :
    gdist n g (roundAt n g) ≤ 2 ^ (g - 1) := by
  obtain ⟨h2, hdm, hlt, hsucc⟩ := kernel_facts n g hg
  unfold roundAt gdist
  by_cases hup : n % 2 ^ g > 2 ^ (g - 1) ∨
      (n % 2 ^ g = 2 ^ (g - 1) ∧ (n / 2 ^ g) % 2 = 1)
  · rw [if_pos hup]
    -- rounded up: distance = 2^g − r, and here r ≥ half
    obtain h | ⟨h, _⟩ := hup <;> omega
  · rw [if_neg hup]
    -- rounded down: distance = r, and here r ≤ half
    have : ¬(n % 2 ^ g > 2 ^ (g - 1)) := fun hc => hup (Or.inl hc)
    omega

/-- NEAREST: no grid multiple is strictly closer to n than the result. -/
theorem roundAt_nearest (n g : Nat) (hg : 1 ≤ g) (k : Nat) :
    gdist n g (roundAt n g) ≤ gdist n g k := by
  obtain ⟨h2, hdm, hlt, hsucc⟩ := kernel_facts n g hg
  -- Position k's grid point linearly relative to q's:
  have hkP : 2 ^ g * k ≤ 2 ^ g * (n / 2 ^ g) ∨
      2 ^ g * (n / 2 ^ g + 1) ≤ 2 ^ g * k := by
    have : k ≤ n / 2 ^ g ∨ n / 2 ^ g + 1 ≤ k := by omega
    obtain h | h := this
    · exact Or.inl (Nat.mul_le_mul_left _ h)
    · exact Or.inr (Nat.mul_le_mul_left _ h)
  unfold roundAt gdist
  by_cases hup : n % 2 ^ g > 2 ^ (g - 1) ∨
      (n % 2 ^ g = 2 ^ (g - 1) ∧ (n / 2 ^ g) % 2 = 1)
  · rw [if_pos hup]
    have hge : n % 2 ^ g ≥ 2 ^ (g - 1) := by
      obtain h | ⟨h, _⟩ := hup <;> omega
    obtain h | h := hkP <;> omega
  · rw [if_neg hup]
    have hle : ¬(n % 2 ^ g > 2 ^ (g - 1)) := fun hc => hup (Or.inl hc)
    obtain h | h := hkP <;> omega

/-- TIES-TO-EVEN: an exact halfway point rounds to an even significand. -/
theorem roundAt_ties_even (n g : Nat) (_hg : 1 ≤ g)
    (htie : n % 2 ^ g = 2 ^ (g - 1)) :
    roundAt n g % 2 = 0 := by
  unfold roundAt
  by_cases hodd : (n / 2 ^ g) % 2 = 1
  · rw [if_pos (Or.inr ⟨htie, hodd⟩)]
    omega
  · have hno : ¬(n % 2 ^ g > 2 ^ (g - 1) ∨
        (n % 2 ^ g = 2 ^ (g - 1) ∧ (n / 2 ^ g) % 2 = 1)) := by
      rw [htie]
      exact fun h => by
        obtain h | ⟨_, h⟩ := h
        · omega
        · exact hodd h
    rw [if_neg hno]
    omega

/-- EXACTNESS: values already on the grid round to themselves. -/
theorem roundAt_exact (n g : Nat) (_hg : 1 ≤ g) (hex : n % 2 ^ g = 0) :
    roundAt n g * 2 ^ g = n := by
  have hhalf : 0 < 2 ^ (g - 1) := Nat.two_pow_pos (g - 1)
  have hdm := Nat.div_add_mod n (2 ^ g)
  unfold roundAt
  have hno : ¬(n % 2 ^ g > 2 ^ (g - 1) ∨
      (n % 2 ^ g = 2 ^ (g - 1) ∧ (n / 2 ^ g) % 2 = 1)) := by
    rw [hex]
    omega
  rw [if_neg hno]
  -- q·2^g = 2^g·q = n − r = n
  have hcomm : n / 2 ^ g * 2 ^ g = 2 ^ g * (n / 2 ^ g) := Nat.mul_comm ..
  omega

end Bitrep
-- Copyright (c) 2026 Kyle Clouthier / Clouthier Simulation Labs. Licensed under MIT OR Apache-2.0.
/-
  bitrep — machine-checked convergence laws for the float G-Counter.

  MODEL. A replica's entry is append-only: replica r's accumulator after n
  local adds is the exact sum (OrderInvariance.lean model: an integer in
  units of 2^-1074) of the first n values of r's add-stream. Because entries
  are append-only, an entry is faithfully represented by HOW MANY adds it
  contains — `count-wins` on entries from the same replica is `Nat.max` on
  those counts (equal counts ⇒ equal states, by construction). A counter
  state is one count per replica; the CRDT join is the pointwise max.

  This file proves, over that model, the laws that make the construction a
  state-based CRDT (a join-semilattice whose joins converge regardless of
  delivery schedule), and ties the converged read value to the exact sum of
  every add that ever happened:

   * JOIN IS A SEMILATTICE — commutative, associative, idempotent
     (`join_comm`, `join_assoc`, `join_idem`);
   * DELIVERY-SCHEDULE INVARIANCE — folding any permutation of any snapshot
     list, with any duplicate deliveries, yields the same state
     (`joinAll_perm_invariant`, `joinAll_dup_invariant`);
   * MONOTONICITY — a join never loses adds (`le_join_left`);
   * EXACT READ — the converged full-knowledge state reads exactly the sum
     of every add from every replica (`read_full_exact`), which by
     `perm_sum_invariant`/`merge_tree_invariant` (OrderInvariance.lean) is
     the same state any sequential or sharded computation of those adds
     produces.

  HONEST SCOPE (bounded claim, named limits): this is the value-level
  algebra. That the Rust `SumF64` faithfully implements the integer model is
  the Kani harnesses' job; that `count` increments per add and the map layer
  takes count-wins per entry is the (small) trusted implementation surface
  (exercised by examples/float_gcounter.rs across 300 randomized delivery
  schedules). Deduplication semantics are the map layer's, as in every
  counter CRDT. Lean 4 core only; no mathlib; zero `sorry`.
-/

namespace Bitrep

/-! ### Entries and the count-wins join -/

/-- Replica-entry join: count-wins. On an append-only history, the larger
    count strictly contains the smaller, so max loses nothing. Definitionally
    `Nat.max`, so the CRDT laws below defer to the core lemmas directly. -/
def ejoin (a b : Nat) : Nat := Nat.max a b

/-! ### Counter states over R replicas -/

/-- A counter state: how many adds of each replica this state contains. -/
def GC (R : Nat) := Fin R → Nat

/-- The CRDT join: pointwise count-wins. -/
def join {R : Nat} (s t : GC R) : GC R := fun r => ejoin (s r) (t r)

/-- The empty state (knows no adds). -/
def bot {R : Nat} : GC R := fun _ => 0

theorem join_comm {R : Nat} (s t : GC R) : join s t = join t s :=
  funext fun r => Nat.max_comm (s r) (t r)

theorem join_assoc {R : Nat} (s t u : GC R) :
    join (join s t) u = join s (join t u) :=
  funext fun r => Nat.max_assoc (s r) (t r) (u r)

theorem join_idem {R : Nat} (s : GC R) : join s s = s :=
  funext fun r => Nat.max_self (s r)

theorem join_bot {R : Nat} (s : GC R) : join bot s = s :=
  funext fun r => Nat.zero_max (s r)

/-- MONOTONICITY: joining never discards an add either side knows. -/
theorem le_join_left {R : Nat} (s t : GC R) (r : Fin R) :
    s r ≤ join s t r :=
  Nat.le_max_left (s r) (t r)

/-! ### Delivery schedules: fold any list of snapshots -/

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

/-- Joins can be re-rooted: join s (join t u) = join t (join s u). -/
theorem join_left_comm {R : Nat} (s t u : GC R) :
    join s (join t u) = join t (join s u) := by
  rw [← join_assoc, join_comm s t, join_assoc]

/-- THEOREM (delivery-order invariance): snapshots received in ANY order
    produce the same state. -/
theorem joinAll_perm_invariant {R : Nat} {l₁ l₂ : List (GC R)}
    (h : PermG l₁ l₂) : joinAll l₁ = joinAll l₂ := by
  induction h with
  | nil => rfl
  | cons s _ ih => simp [joinAll, ih]
  | swap s t l => simp [joinAll, join_left_comm]
  | trans _ _ ih₁ ih₂ => exact ih₁.trans ih₂

/-- THEOREM (duplicate-delivery invariance): receiving the same snapshot
    twice in a row changes nothing. With `joinAll_perm_invariant`, ANY
    duplicate anywhere in the schedule is absorbed. -/
theorem joinAll_dup_invariant {R : Nat} (s : GC R) (l : List (GC R)) :
    joinAll (s :: s :: l) = joinAll (s :: l) := by
  simp [joinAll, ← join_assoc, join_idem]

/-! ### Reading the counter: the exact value -/

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

/-- Convergence is state equality, so equal states read equally — stated
    for the record. -/
theorem read_of_eq {R : Nat} (streams : Fin R → Nat → Int) {s t : GC R}
    (h : s = t) : read streams s = read streams t := by rw [h]

/-- THEOREM (exact read): the full-knowledge state reads the sum of every
    add of every replica — each entry contributes the exact sum of ALL its
    replica's adds, and entries combine by exact merge. By
    `perm_sum_invariant` and `merge_tree_invariant`, this is byte-for-byte
    the state ANY ordering or sharding of those same adds produces. -/
theorem read_full_exact {R : Nat} (streams : Fin R → Nat → Int)
    (total : Fin R → Nat) :
    read streams (full total) =
      sumOver R (fun r => if h : r < R then
        lsum (taken (streams ⟨r, h⟩) (total ⟨r, h⟩)) else 0) := by
  rfl

/-- Joining a snapshot into the full state changes nothing: the full state
    absorbs every schedule (`full` is the top of the reachable lattice when
    each snapshot's entries are bounded by `total`). -/
theorem full_absorbs {R : Nat} (total : Fin R → Nat) (s : GC R)
    (hb : ∀ r, s r ≤ total r) : join (full total) s = full total :=
  funext fun r => Nat.max_eq_left (hb r)

end Bitrep
-- Copyright (c) 2026 Kyle Clouthier / Clouthier Simulation Labs. Licensed under MIT OR Apache-2.0.
/-
  bitrep — merge algebra for the convergent toolkit (v0.2).

  MODEL. Every toolkit state is a PRODUCT of components whose merges are
  already lawful:
   * exact sums (OrderInvariance.lean: integer addition in units of 2^-1074
     — `merge_comm`, `merge_assoc`);
   * saturating counters (`satAdd` below);
   * boolean flags (or);
   * min/max under a total order (ExtremaF64 orders IEEE bits by total_cmp,
     an order-embedding into the naturals — modeled as Nat min/max below);
   * per-key families of the above (ConvergentMap: pointwise merge).

  This file proves the LIFTING laws: commutativity/associativity are
  preserved by products and by pointwise (per-key) application, the min/max
  join is a semilattice (idempotent as well), boolean-or is a semilattice,
  and saturating addition is commutative and associative. Together with the
  component laws these give merge commutativity/associativity for
  MomentsF64, Moments4F64, CovF64, WeightedMomentsF64, PnMomentsF64,
  CovMatrixF64, HistogramF64, ExtremaF64 and ConvergentMap at the model
  level. The per-replica count-wins layer (`Replicated`) is exactly the
  join proved in FloatGCounter.lean.

  Division of labor, stated: Lean proves the value-level algebra; that the
  Rust implementation realizes it bit-for-bit is checked by Kani harnesses
  (merge/codec, extrema laws) and differential/property tests — see the
  README's verification table.
-/

namespace Bitrep

/-! ### Products preserve merge laws -/

/-- Componentwise merge on a pair. Every multi-accumulator toolkit state
    (Moments, Cov, Weighted, Pn, CovMatrix, Histogram) is an iterated pair
    of component states merged this way. -/
def prodMerge (f : α → α → α) (g : β → β → β) (p q : α × β) : α × β :=
  (f p.1 q.1, g p.2 q.2)

theorem prod_merge_comm (f : α → α → α) (g : β → β → β)
    (hf : ∀ a b, f a b = f b a) (hg : ∀ a b, g a b = g b a) :
    ∀ p q, prodMerge f g p q = prodMerge f g q p := by
  intro p q
  simp [prodMerge, hf p.1 q.1, hg p.2 q.2]

theorem prod_merge_assoc (f : α → α → α) (g : β → β → β)
    (hf : ∀ a b c, f (f a b) c = f a (f b c))
    (hg : ∀ a b c, g (g a b) c = g a (g b c)) :
    ∀ p q r, prodMerge f g (prodMerge f g p q) r
           = prodMerge f g p (prodMerge f g q r) := by
  intro p q r
  simp [prodMerge, hf p.1 q.1 r.1, hg p.2 q.2 r.2]

theorem prod_merge_idem (f : α → α → α) (g : β → β → β)
    (hf : ∀ a, f a a = a) (hg : ∀ a, g a a = a) :
    ∀ p, prodMerge f g p p = p := by
  intro p
  simp [prodMerge, hf p.1, hg p.2]

/-! ### Per-key (pointwise) merge preserves laws — the ConvergentMap model.

A keyed family is modeled as a total function from keys to states (absent
keys hold the empty state, which is `merge`'s identity in every component). -/

/-- Pointwise merge of keyed families. -/
def mapMerge (f : α → α → α) (m n : κ → α) : κ → α :=
  fun k => f (m k) (n k)

theorem map_merge_comm (f : α → α → α)
    (hf : ∀ a b, f a b = f b a) :
    ∀ m n : κ → α, mapMerge f m n = mapMerge f n m := by
  intro m n
  funext k
  exact hf (m k) (n k)

theorem map_merge_assoc (f : α → α → α)
    (hf : ∀ a b c, f (f a b) c = f a (f b c)) :
    ∀ m n o : κ → α, mapMerge f (mapMerge f m n) o
                   = mapMerge f m (mapMerge f n o) := by
  intro m n o
  funext k
  exact hf (m k) (n k) (o k)

/-! ### Min/max join — the Extrema lattice.

`f64::total_cmp` is a total order on IEEE bit patterns; the standard
sign-magnitude transform embeds it order-isomorphically into the naturals,
so Nat `min`/`max` are a faithful model of the ExtremaF64 join. -/

theorem max_join_comm : ∀ a b : Nat, max a b = max b a := by
  intro a b; omega

theorem max_join_assoc : ∀ a b c : Nat, max (max a b) c = max a (max b c) := by
  intro a b c; omega

theorem max_join_idem : ∀ a : Nat, max a a = a := by
  intro a; omega

theorem min_join_comm : ∀ a b : Nat, min a b = min b a := by
  intro a b; omega

theorem min_join_assoc : ∀ a b c : Nat, min (min a b) c = min a (min b c) := by
  intro a b c; omega

theorem min_join_idem : ∀ a : Nat, min a a = a := by
  intro a; omega

/-! ### Boolean flags (NaN / infinity / poisoned) join by `or`. -/

theorem or_join_comm : ∀ a b : Bool, (a || b) = (b || a) := by
  decide

theorem or_join_assoc : ∀ a b c : Bool, ((a || b) || c) = (a || (b || c)) := by
  decide

theorem or_join_idem : ∀ a : Bool, (a || a) = a := by
  decide

/-! ### Saturating counters.

Every state's operation count merges by saturating addition (cap = 2^64 - 1
in the implementation; any cap works). Not idempotent — deduplication is the
count-wins map layer's job (FloatGCounter.lean), exactly as documented. -/

/-- Saturating addition with cap `c`. -/
def satAdd (c a b : Nat) : Nat := min c (a + b)

theorem satAdd_comm (c : Nat) : ∀ a b, satAdd c a b = satAdd c b a := by
  intro a b
  simp [satAdd]
  omega

theorem satAdd_assoc (c : Nat) :
    ∀ a b d, satAdd c (satAdd c a b) d = satAdd c a (satAdd c b d) := by
  intro a b d
  simp [satAdd]
  omega

end Bitrep
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
