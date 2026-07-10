-- Copyright (c) 2026 Kyle Clouthier. Licensed under MIT OR Apache-2.0.
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
