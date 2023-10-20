use serde::Serde;
use poseidon::poseidon_hash_span;
use dojo_primitives::commit_reveal::{Commitment, CommitmentTrait};

#[test]
#[available_gas(1000000)]
fn test_commit_reveal() {
    let mut commitment = CommitmentTrait::new();

    let value = array!['ohayo'].span();
    let hash = poseidon_hash_span(value);
    commitment.commit(hash);
    let valid = commitment.reveal('ohayo');
    assert(valid, 'invalid reveal for commitment')
}
