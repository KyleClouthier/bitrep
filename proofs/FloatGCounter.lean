-- Copyright (c) 2026 Kyle Clouthier. Licensed under MIT OR Apache-2.0.
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
    count strictly contains the smaller, so max loses nothing. -/
def ejoin (a b : Nat) : Nat := Nat.max a b

theorem ejoin_comm (a b : Nat) : ejoin a b = ejoin b a :=
  Nat.max_comm a b

theorem ejoin_assoc (a b c : Nat) :
    ejoin (ejoin a b) c = ejoin a (ejoin b c) :=
  Nat.max_assoc a b c

theorem ejoin_idem (a : Nat) : ejoin a a = a :=
  Nat.max_self a

/-! ### Counter states over R replicas -/

/-- A counter state: how many adds of each replica this state contains. -/
def GC (R : Nat) := Fin R → Nat

/-- The CRDT join: pointwise count-wins. -/
def join {R : Nat} (s t : GC R) : GC R := fun r => ejoin (s r) (t r)

/-- The empty state (knows no adds). -/
def bot {R : Nat} : GC R := fun _ => 0

theorem join_comm {R : Nat} (s t : GC R) : join s t = join t s :=
  funext fun r => ejoin_comm (s r) (t r)

theorem join_assoc {R : Nat} (s t u : GC R) :
    join (join s t) u = join s (join t u) :=
  funext fun r => ejoin_assoc (s r) (t r) (u r)

theorem join_idem {R : Nat} (s : GC R) : join s s = s :=
  funext fun r => ejoin_idem (s r)

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
