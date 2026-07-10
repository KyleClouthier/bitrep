// Copyright (c) 2026 Kyle Clouthier. Licensed under MIT OR Apache-2.0.
//! Probe: authenticated float aggregates — a Merkle tree over exact sums.
//!
//! Float sums never had authenticated data structures because a float sum
//! isn't canonical: two honest parties summing the same leaves in different
//! orders get different bytes, so a hash over the result proves nothing.
//! `SumF64`'s state is canonical and mergeable, which makes the classic
//! Merkle-sum-tree construction meaningful for floats:
//!
//! * each leaf hashes (value bits, accumulator state of that leaf);
//! * each internal node stores the merge of its children's accumulators and
//!   hashes (its state, left child hash, right child hash);
//! * the root commits to every leaf AND the exact total.
//!
//! What this probe must demonstrate (or the idea dies):
//! 1. incremental update — change one leaf, recompute one root-path — yields
//!    the byte-identical root a full rebuild yields;
//! 2. the root total equals the exact sum of all leaves;
//! 3. tampering with any leaf changes the root;
//! 4. an O(log n) inclusion proof verifies a leaf against the root.
//!
//! Run: `cargo run --example merkle_sum_tree`

use bitrep::SumF64;
use sha2::{Digest, Sha256};

type Hash = [u8; 32];

struct MerkleSumTree {
    /// levels[0] = leaves, levels.last() = [root]. Power-of-two padded.
    hashes: Vec<Vec<Hash>>,
    accs: Vec<Vec<SumF64>>,
}

fn leaf_acc(x: f64) -> SumF64 {
    let mut a = SumF64::new();
    a.add(x);
    a
}

fn hash_leaf(acc: &SumF64) -> Hash {
    let mut h = Sha256::new();
    h.update(b"leaf");
    h.update(acc.to_bytes());
    h.finalize().into()
}

fn hash_node(acc: &SumF64, l: &Hash, r: &Hash) -> Hash {
    let mut h = Sha256::new();
    h.update(b"node");
    h.update(acc.to_bytes());
    h.update(l);
    h.update(r);
    h.finalize().into()
}

impl MerkleSumTree {
    fn build(values: &[f64]) -> Self {
        let n = values.len().next_power_of_two();
        let mut acc_level: Vec<SumF64> = values.iter().copied().map(leaf_acc).collect();
        acc_level.resize_with(n, SumF64::new); // pad with empty accumulators
        let mut hash_level: Vec<Hash> = acc_level.iter().map(hash_leaf).collect();

        let mut accs = vec![acc_level];
        let mut hashes = vec![hash_level.clone()];
        while hashes.last().unwrap().len() > 1 {
            let prev_acc = accs.last().unwrap();
            let mut next_acc = Vec::with_capacity(prev_acc.len() / 2);
            let mut next_hash = Vec::with_capacity(prev_acc.len() / 2);
            for i in (0..prev_acc.len()).step_by(2) {
                let mut m = prev_acc[i].clone();
                m.merge(&prev_acc[i + 1]);
                next_hash.push(hash_node(&m, &hash_level[i], &hash_level[i + 1]));
                next_acc.push(m);
            }
            hash_level = next_hash.clone();
            accs.push(next_acc);
            hashes.push(next_hash);
        }
        MerkleSumTree { hashes, accs }
    }

    fn root(&self) -> Hash {
        self.hashes.last().unwrap()[0]
    }

    fn total(&self) -> f64 {
        self.accs.last().unwrap()[0].value()
    }

    /// Replace leaf `i` and recompute only the path to the root: O(log n).
    fn update(&mut self, i: usize, x: f64) {
        self.accs[0][i] = leaf_acc(x);
        self.hashes[0][i] = hash_leaf(&self.accs[0][i]);
        let mut idx = i;
        for lvl in 0..self.accs.len() - 1 {
            let parent = idx / 2;
            let (l, r) = (parent * 2, parent * 2 + 1);
            let mut m = self.accs[lvl][l].clone();
            m.merge(&self.accs[lvl][r]);
            self.hashes[lvl + 1][parent] =
                hash_node(&m, &self.hashes[lvl][l], &self.hashes[lvl][r]);
            self.accs[lvl + 1][parent] = m;
            idx = parent;
        }
    }

    /// Inclusion proof: sibling hashes along the path, plus each parent state.
    fn prove(&self, i: usize) -> Vec<(Hash, SumF64, bool)> {
        let mut proof = Vec::new();
        let mut idx = i;
        for lvl in 0..self.accs.len() - 1 {
            let sib = idx ^ 1;
            let parent = idx / 2;
            proof.push((
                self.hashes[lvl][sib],
                self.accs[lvl + 1][parent].clone(),
                idx % 2 == 0,
            ));
            idx = parent;
        }
        proof
    }
}

/// Verify a leaf against a root using only the proof — log(n) hashes.
fn verify(leaf: &SumF64, proof: &[(Hash, SumF64, bool)], root: &Hash) -> bool {
    let mut h = hash_leaf(leaf);
    for (sib, parent_acc, leaf_is_left) in proof {
        h = if *leaf_is_left {
            hash_node(parent_acc, &h, sib)
        } else {
            hash_node(parent_acc, sib, &h)
        };
    }
    h == *root
}

fn hostile_values(n: usize) -> Vec<f64> {
    // Mixed magnitudes and exact-cancellation pairs: the data that breaks
    // naive float Merkle-sum attempts.
    let mut s = 0x9E3779B97F4A7C15u64;
    let mut v: Vec<f64> = Vec::with_capacity(n);
    for i in 0..n {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        let mant = (s >> 11) as f64 / (1u64 << 53) as f64 - 0.5;
        let exp = ((s >> 1) % 120) as i32 - 60;
        let x = mant * 2f64.powi(exp);
        v.push(if i % 7 == 3 { -v[i - 1] } else { x });
    }
    v
}

fn main() {
    let n = 4096;
    let mut values = hostile_values(n);

    // (2) Root commits to the exact total.
    let mut tree = MerkleSumTree::build(&values);
    let direct: SumF64 = values.iter().copied().collect();
    assert_eq!(tree.total().to_bits(), direct.value().to_bits());
    println!("root total == exact sum of {n} hostile leaves: OK");

    // (1) Incremental update == full rebuild, byte for byte, repeatedly.
    let mut s = 0xD1B54A32D192ED03u64;
    for round in 0..200 {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        let i = (s % n as u64) as usize;
        let x = (s >> 11) as f64 / (1u64 << 53) as f64 * 1e30 - 5e29;
        values[i] = x;
        tree.update(i, x);
        if round % 50 == 0 {
            let rebuilt = MerkleSumTree::build(&values);
            assert_eq!(tree.root(), rebuilt.root(), "incremental != rebuild");
            assert_eq!(tree.total().to_bits(), rebuilt.total().to_bits());
        }
    }
    println!("200 incremental O(log n) updates == full rebuilds: OK");

    // (3) Tampering any leaf changes the root.
    let honest_root = tree.root();
    let mut tampered = MerkleSumTree::build(&values);
    tampered.update(1234, -values[1234]);
    assert_ne!(honest_root, tampered.root());
    println!("tampered leaf changes the root: OK");

    // (4) log(n) inclusion proof.
    let proof = tree.prove(777);
    assert_eq!(proof.len(), (n as f64).log2() as usize);
    assert!(verify(&tree.accs[0][777], &proof, &honest_root));
    let mut bad = tree.accs[0][777].clone();
    bad.add(1e-300);
    assert!(!verify(&bad, &proof, &honest_root));
    println!(
        "inclusion proof: {} hashes verify one leaf in a {n}-leaf total; forged leaf rejected: OK",
        proof.len()
    );

    println!("\nPROBE RESULT: LANDS — authenticated float aggregates work.");
}
