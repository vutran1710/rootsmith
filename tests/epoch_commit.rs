use rootsmith::types::LegacyCommitment;
use rootsmith::crypto::MerkleTree;

#[test]
fn test_epoch_commit() {
    let mut merkle = MerkleTree::new();
    merkle.add_leaf(vec![1, 2, 3]);
    merkle.add_leaf(vec![4, 5, 6]);
    merkle.add_leaf(vec![7, 8, 9]);
    let root = merkle.root().expect("Failed to get merkle root");
    let root_hex = hex::encode(&root);
    let commitment = LegacyCommitment {
        epoch: 1,
        merkle_root: root_hex,
        timestamp: 1000,
    };
    assert_eq!(commitment.epoch, 1);
    assert!(!commitment.merkle_root.is_empty());
}

#[test]
fn test_proof_generation() {
    let mut merkle = MerkleTree::new();
    merkle.add_leaf(vec![1, 2, 3]);
    merkle.add_leaf(vec![4, 5, 6]);
    let proof = merkle.generate_proof(0).expect("Failed to generate proof");
    assert!(!proof.is_empty());
}

mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>()
    }
}
