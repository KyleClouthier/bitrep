#!/usr/bin/env bash
# Copyright (c) 2026 Kyle Clouthier. Licensed under MIT OR Apache-2.0.
# Machine-check the bitrep proofs and FAIL if any stops being a real proof:
#  1. `lean` must type-check OrderInvariance.lean and RoundNearestEven.lean (exit 0).
#  2. `#print axioms` on every theorem must report only Lean's standard base
#     (propext / Classical.choice / Quot.sound) or no axioms at all —
#     no sorryAx, no added axioms.
# Run from anywhere: bash proofs/check.sh
set -euo pipefail
cd "$(dirname "$0")"

LEAN="${LEAN:-lean}"
command -v "$LEAN" >/dev/null || LEAN="$HOME/.elan/bin/lean"

echo "[1/2] type-checking proofs with $($LEAN --version)"
"$LEAN" OrderInvariance.lean
"$LEAN" RoundNearestEven.lean
"$LEAN" ToolkitAlgebra.lean
"$LEAN" RelSketchMerge.lean
# FloatGCounter builds on OrderInvariance's model (lsum): check them together.
TMPGC="$(mktemp /tmp/bitrep_gc_XXXX.lean)"
cat OrderInvariance.lean FloatGCounter.lean > "$TMPGC"
"$LEAN" "$TMPGC"
rm -f "$TMPGC"

echo "[2/2] axiom audit (no sorry, standard base only)"
TMP="$(mktemp /tmp/bitrep_axcheck_XXXX.lean)"
cat OrderInvariance.lean RoundNearestEven.lean FloatGCounter.lean ToolkitAlgebra.lean RelSketchMerge.lean > "$TMP"
cat >> "$TMP" <<'AX'

-- order invariance (OrderInvariance.lean)
#print axioms Bitrep.perm_sum_invariant
#print axioms Bitrep.lsum_append
#print axioms Bitrep.merge_tree_invariant
#print axioms Bitrep.add_placement_invariant
#print axioms Bitrep.merge_comm
#print axioms Bitrep.merge_assoc
-- rounding kernel (RoundNearestEven.lean)
#print axioms Bitrep.roundAt_half_ulp
#print axioms Bitrep.roundAt_nearest
#print axioms Bitrep.roundAt_ties_even
#print axioms Bitrep.roundAt_exact
-- float G-Counter convergence laws (FloatGCounter.lean)
#print axioms Bitrep.join_comm
#print axioms Bitrep.join_assoc
#print axioms Bitrep.join_idem
#print axioms Bitrep.joinAll_perm_invariant
#print axioms Bitrep.joinAll_dup_invariant
#print axioms Bitrep.le_join_left
#print axioms Bitrep.full_absorbs
#print axioms Bitrep.read_full_exact
-- toolkit merge algebra (ToolkitAlgebra.lean)
#print axioms Bitrep.prod_merge_comm
#print axioms Bitrep.prod_merge_assoc
#print axioms Bitrep.prod_merge_idem
#print axioms Bitrep.map_merge_comm
#print axioms Bitrep.map_merge_assoc
#print axioms Bitrep.max_join_comm
#print axioms Bitrep.max_join_assoc
#print axioms Bitrep.max_join_idem
#print axioms Bitrep.min_join_comm
#print axioms Bitrep.min_join_assoc
#print axioms Bitrep.min_join_idem
#print axioms Bitrep.or_join_comm
#print axioms Bitrep.or_join_assoc
#print axioms Bitrep.or_join_idem
#print axioms Bitrep.satAdd_comm
#print axioms Bitrep.satAdd_assoc
-- RelSketch quantile-sketch merge laws (RelSketchMerge.lean)
#print axioms Bitrep.relsketch_merge_comm
#print axioms Bitrep.relsketch_merge_assoc
#print axioms Bitrep.relsketch_merge_empty
AX
OUT="$("$LEAN" "$TMP" 2>&1)"
rm -f "$TMP"
echo "$OUT"
if echo "$OUT" | grep -q "sorryAx"; then
  echo "FAIL: a proof depends on sorryAx (a hole was introduced)"; exit 1
fi
if echo "$OUT" | grep "depends on axioms:" | grep -vE "^'[a-zA-Z0-9_.]+' depends on axioms: \[(propext|Classical\.choice|Quot\.sound)(, (propext|Classical\.choice|Quot\.sound))*\]$" | grep -q .; then
  echo "FAIL: a theorem depends on a non-standard axiom"; exit 1
fi
echo "OK: order-invariance + rounding kernel + G-Counter + toolkit merge algebra + RelSketch merge proved; axioms = standard base only"
