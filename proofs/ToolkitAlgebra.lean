-- Copyright (c) 2026 Kyle Clouthier. Licensed under MIT OR Apache-2.0.
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
