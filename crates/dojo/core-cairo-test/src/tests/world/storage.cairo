use dojo::model::ModelStorage;

use crate::tests::helpers::{deploy_world_and_foo, Foo, NotCopiable, EnumOne, WithOptionAndEnums};

#[test]
fn write_simple() {
    let (mut world, _) = deploy_world_and_foo();

    let bob = 0xb0b.try_into().unwrap();

    let foo: Foo = world.read_model(bob);
    assert_eq!(foo.caller, bob);
    assert_eq!(foo.a, 0);
    assert_eq!(foo.b, 0);

    let foo = Foo { caller: bob, a: 1, b: 2 };
    world.write_model(@foo);

    let foo: Foo = world.read_model(bob);
    assert_eq!(foo.caller, bob);
    assert_eq!(foo.a, 1);
    assert_eq!(foo.b, 2);

    world.erase_model(@foo);

    let foo: Foo = world.read_model(bob);
    assert_eq!(foo.caller, bob);
    assert_eq!(foo.a, 0);
    assert_eq!(foo.b, 0);
}

#[test]
fn write_multiple_copiable() {
    let (mut world, _) = deploy_world_and_foo();

    let mut models_snaps: Array<@Foo> = array![];
    let mut keys: Array<starknet::ContractAddress> = array![];

    for i in 0_u128..10_u128 {
        let felt: felt252 = i.into();
        let caller: starknet::ContractAddress = felt.try_into().unwrap();
        keys.append(caller);

        if i % 2 == 0 {
            let foo = Foo { caller, a: felt, b: i };
            models_snaps.append(@foo);
        } else {
            let foo = Foo { caller, a: felt, b: i };
            models_snaps.append(@foo);
        }
    };

    world.write_models(models_snaps.span());

    let mut models: Array<Foo> = world.read_models(keys.span());

    assert_eq!(models.len(), 10);

    for i in 0_u128..10_u128 {
        let felt: felt252 = i.into();
        let caller: starknet::ContractAddress = felt.try_into().unwrap();
        // Can desnap as copiable.
        let model: Foo = *models[i.try_into().unwrap()];
        assert_eq!(model.caller, caller);
        assert_eq!(model.a, felt);
        assert_eq!(model.b, i);
    };

    world.erase_models(models_snaps.span());

    let mut models: Array<Foo> = world.read_models(keys.span());

    while let Option::Some(m) = models.pop_front() {
        assert_eq!(m.a, 0);
        assert_eq!(m.b, 0);
    };
}

#[test]
fn write_multiple_not_copiable() {
    let (mut world, _) = deploy_world_and_foo();

    let mut models_snaps: Array<@NotCopiable> = array![];
    let mut keys: Array<starknet::ContractAddress> = array![];

    for i in 0_u128..10_u128 {
        let felt: felt252 = i.into();
        let caller: starknet::ContractAddress = felt.try_into().unwrap();
        keys.append(caller);

        if i % 2 == 0 {
            let foo = NotCopiable { caller, a: array![felt], b: "ab" };
            models_snaps.append(@foo);
        } else {
            let foo = NotCopiable { caller, a: array![felt], b: "ab" };
            models_snaps.append(@foo);
        }
    };

    world.write_models(models_snaps.span());

    let mut models: Array<NotCopiable> = world.read_models(keys.span());

    assert_eq!(models.len(), 10);

    for i in 0_u128..10_u128 {
        let felt: felt252 = i.into();
        let caller: starknet::ContractAddress = felt.try_into().unwrap();
        // Can desnap as copiable.
        let model: NotCopiable = models.pop_front().unwrap();
        assert_eq!(model.caller, caller);
        assert_eq!(model.a, array![felt]);
        assert_eq!(model.b, "ab");
    };

    world.erase_models(models_snaps.span());

    let mut models: Array<NotCopiable> = world.read_models(keys.span());

    while let Option::Some(m) = models.pop_front() {
        assert_eq!(m.a, array![]);
        assert_eq!(m.b, "");
    };
}

#[test]
fn write_read_option_enums() {
    let (mut world, _) = deploy_world_and_foo();

    let key: u32 = 1;

    let wo: WithOptionAndEnums = world.read_model(key);
    assert_eq!(wo.a, EnumOne::Two(0));
    assert_eq!(wo.b, Option::None);
}
