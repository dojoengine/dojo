use serde::Serde;
use poseidon::poseidon_hash_span;

#[derive(Copy, Drop, Default, Serde, Introspect)]
struct Commitment {
    hash: felt252
}

trait CommitmentTrait {
    fn new() -> Commitment;
    fn commit(ref self: Commitment, hash: felt252);
    fn reveal<T, impl TSerde: Serde<T>, impl TDrop: Drop<T>>(self: @Commitment, reveal: T) -> bool;
}

impl CommitmentImpl of CommitmentTrait {
    fn new() -> Commitment {
        Commitment { hash: 0 }
    }

    fn commit(ref self: Commitment, hash: felt252) {
        assert(hash.is_non_zero(), 'can not commit zero');
        self.hash = hash;
    }

    fn reveal<T, impl TSerde: Serde<T>, impl TDrop: Drop<T>>(self: @Commitment, reveal: T) -> bool {
        let mut serialized = array![];
        reveal.serialize(ref serialized);
        let hash = poseidon_hash_span(serialized.span());
        return hash == *self.hash;
    }
}
