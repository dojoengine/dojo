use array::{ArrayTrait, SpanTrait};
use option::OptionTrait;
use traits::Into;

use dojo_core::integer::u250;
use dojo_core::storage::utils::find_matching;

fn build_fake_entity(v: felt252) -> Span<felt252> {
    let mut e = ArrayTrait::new();
    e.append(v);
    e.append(v);
    e.append(v);
    e.span()
}

fn assert_entity(entity: Span<felt252>, v: felt252) {
    assert(entity.len() == 3, 'entity len');
    assert(*entity[0] == v, 'entity 0');
    assert(*entity[1] == v, 'entity 1');
    assert(*entity[2] == v, 'entity 2');
}

#[test]
#[available_gas(1000000000)]
fn test_find_matching() {
    let mut ids1: Array<u250> = ArrayTrait::new();
    let mut ids2: Array<u250> = ArrayTrait::new();
    let mut ids3: Array<u250> = ArrayTrait::new();

    ids1.append(u250 { inner: 1 } );
    ids1.append(u250 { inner: 3 } );
    ids1.append(u250 { inner: 6 } );
    ids1.append(u250 { inner: 5 } );

    ids2.append(u250 { inner: 4} );
    ids2.append(u250 { inner: 5} );
    ids2.append(u250 { inner: 3} );

    ids3.append(u250 { inner: 3} );
    ids3.append(u250 { inner: 2} );
    ids3.append(u250 { inner: 1} );
    ids3.append(u250 { inner: 7} );
    ids3.append(u250 { inner: 5} );

    let mut ids: Array<Span<u250>> = ArrayTrait::new();
    ids.append(ids1.span());
    ids.append(ids2.span());
    ids.append(ids3.span());

    let mut e1: Array<Span<felt252>> = ArrayTrait::new();
    e1.append(build_fake_entity(1));
    e1.append(build_fake_entity(3));
    e1.append(build_fake_entity(6));
    e1.append(build_fake_entity(5));

    let mut e2: Array<Span<felt252>> = ArrayTrait::new();
    e2.append(build_fake_entity(40));
    e2.append(build_fake_entity(50));
    e2.append(build_fake_entity(30));

    let mut e3: Array<Span<felt252>> = ArrayTrait::new();
    e3.append(build_fake_entity(300));
    e3.append(build_fake_entity(200));
    e3.append(build_fake_entity(100));
    e3.append(build_fake_entity(700));
    e3.append(build_fake_entity(500));

    let mut entities: Array<Span<Span<felt252>>> = ArrayTrait::new();
    entities.append(e1.span());
    entities.append(e2.span());
    entities.append(e3.span());

    let matching = find_matching(ids.span(), entities.span());

    // there is a match only on entities with IDs 3 and 5
    // and matching should look like:
    // [
    //   [[3, 3, 3], [5, 5, 5]],
    //   [[30, 30, 30], [50, 50, 50]],
    //   [[300, 300, 300], [500, 500, 500]]
    // ]

    assert(matching.len() == 3, 'matching len');

    let entities0 = *matching[0];
    assert(entities0.len() == 2, 'entities0 len');
    assert_entity(*entities0[0], 3);
    assert_entity(*entities0[1], 5);

    let entities1 = *matching[1];
    assert(entities1.len() == 2, 'entities1 len');
    assert_entity(*entities1[0], 30);
    assert_entity(*entities1[1], 50);

    let entities2 = *matching[2];
    assert(entities2.len() == 2, 'entities2 len');
    assert_entity(*entities2[0], 300);
    assert_entity(*entities2[1], 500);
}

#[test]
#[available_gas(1000000000)]
#[should_panic(expected: ('lengths dont match', ))]
fn test_find_matching_wrong_arg_len() {
    let mut ids1: Array<u250> = ArrayTrait::new();
    let mut ids2: Array<u250> = ArrayTrait::new();
    let mut ids3: Array<u250> = ArrayTrait::new();

    ids1.append(u250 { inner: 1 } );
    ids1.append(u250 { inner: 3 } );
    ids1.append(u250 { inner: 6 } );
    ids1.append(u250 { inner: 5 } );

    ids2.append(u250 { inner: 4} );
    ids2.append(u250 { inner: 5} );
    ids2.append(u250 { inner: 3} );

    let mut ids: Array<Span<u250>> = ArrayTrait::new();
    ids.append(ids1.span());
    ids.append(ids2.span());

    let mut e1: Array<Span<felt252>> = ArrayTrait::new();
    e1.append(build_fake_entity(1));
    e1.append(build_fake_entity(3));
    e1.append(build_fake_entity(6));
    e1.append(build_fake_entity(5));

    let mut e2: Array<Span<felt252>> = ArrayTrait::new();
    e2.append(build_fake_entity(40));
    e2.append(build_fake_entity(50));
    e2.append(build_fake_entity(30));

    let mut e3: Array<Span<felt252>> = ArrayTrait::new();
    e3.append(build_fake_entity(300));
    e3.append(build_fake_entity(200));
    e3.append(build_fake_entity(100));
    e3.append(build_fake_entity(700));
    e3.append(build_fake_entity(500));

    let mut entities: Array<Span<Span<felt252>>> = ArrayTrait::new();
    entities.append(e1.span());
    entities.append(e2.span());
    entities.append(e3.span());

    let matching = find_matching(ids.span(), entities.span());
}
