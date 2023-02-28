use hash::LegacyHash;

impl LegacyHashContractAddressUsizePair of LegacyHash::<(ContractAddress, usize)> {
    fn hash(state: felt, pair: (ContractAddress, usize)) -> felt {
        let (first, second) = pair;
        let state = LegacyHash::hash(state, first);
        LegacyHash::hash(state, second)
    }
}
