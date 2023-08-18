use starknet::ContractAddress;
use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};

#[derive(Component, Copy, Drop, Serde, SerdeLen)]
struct BaseUri {
    #[key]
    token: ContractAddress,
    uri: felt252
}

trait BaseUriTrait {
    fn get_base_uri(world: IWorldDispatcher, token: ContractAddress) -> felt252;
    fn set_base_uri(world: IWorldDispatcher, token: ContractAddress, new_base_uri: felt252);
}

impl BaseUriImpl of BaseUriTrait {
    fn get_base_uri(world: IWorldDispatcher, token: ContractAddress, ) -> felt252 {
        let base_uri = get!(world, (token), BaseUri);
        base_uri.uri
    }

    // perform safety checks before calling this fn
    fn set_base_uri(world: IWorldDispatcher, token: ContractAddress, new_base_uri: felt252) {
        let mut base_uri = get!(world, (token), BaseUri);
        base_uri.uri = new_base_uri;
        set!(world, (base_uri))
    }
}
