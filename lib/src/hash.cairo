use hash::LegacyHash;

impl LegacyHashContractAddressUsizePair of LegacyHash::<(ContractAddress, usize)> {
    fn hash(state: felt252, pair: (ContractAddress, usize)) -> felt252 {
        let (first, second) = pair;
        let state = LegacyHash::hash(state, first);
        LegacyHash::hash(state, second)
    }
}
