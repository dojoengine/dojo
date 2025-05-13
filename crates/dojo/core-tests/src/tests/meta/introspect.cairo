use dojo::meta::introspect::{Enum, Introspect, Member, Struct, Ty, TyCompareTrait};
use dojo::meta::{FieldLayout, Layout};
use crate::utils::GasCounterTrait;

#[derive(Drop, Introspect)]
struct Base {
    value: u32,
}

#[derive(Drop, Introspect)]
struct WithArray {
    value: u32,
    arr: Array<u8>,
}

#[derive(Drop, Introspect)]
struct WithByteArray {
    value: u32,
    arr: ByteArray,
}

#[derive(Drop, Introspect)]
struct WithTuple {
    value: u32,
    arr: (u8, u16, u32),
}

#[derive(Drop, Introspect)]
struct WithNestedTuple {
    value: u32,
    arr: (u8, (u16, u128, u256), u32),
}

#[derive(Drop, Introspect)]
struct WithNestedArrayInTuple {
    value: u32,
    arr: (u8, (u16, Array<u128>, u256), u32),
}

#[derive(Drop, IntrospectPacked)]
struct Vec3 {
    x: u32,
    y: u32,
    z: u32,
}

#[derive(IntrospectPacked)]
struct Translation {
    from: Vec3,
    to: Vec3,
}

#[derive(Drop, IntrospectPacked)]
struct StructInnerNotPacked {
    x: Base,
}

#[derive(Drop, Introspect)]
enum EnumNoData {
    One,
    Two,
    Three,
}

#[derive(Drop, Introspect)]
enum EnumWithSameData {
    One: u256,
    Two: u256,
    Three: u256,
}

#[derive(Drop, Introspect)]
enum EnumWithSameTupleData {
    One: (u256, u32),
    Two: (u256, u32),
    Three: (u256, u32),
}

#[derive(Drop, Introspect)]
enum EnumWithVariousData {
    One: u32,
    Two: (u8, u16),
    Three: Array<u128>,
}


#[derive(Drop, IntrospectPacked)]
enum EnumPacked {
    A: u32,
    B: u32,
}

#[derive(Drop, IntrospectPacked)]
enum EnumInnerPacked {
    A: (EnumPacked, Vec3),
    B: (EnumPacked, Vec3),
}

#[derive(Drop, IntrospectPacked)]
enum EnumInnerNotPacked {
    A: (EnumPacked, Base),
    B: (EnumPacked, Base),
}

// no variant data or unit type must be equivalent and
// so, must compile successfully
#[derive(Drop, IntrospectPacked)]
enum EnumPackedWithUnitType {
    A,
    B: (),
}

#[derive(Drop, Introspect)]
struct StructWithOption {
    x: Option<u16>,
}

#[derive(Drop, Introspect)]
struct Generic<T> {
    value: T,
}

fn field(selector: felt252, layout: Layout) -> FieldLayout {
    FieldLayout { selector, layout }
}

fn fixed(values: Array<u8>) -> Layout {
    Layout::Fixed(values.span())
}

fn tuple(values: Array<Layout>) -> Layout {
    Layout::Tuple(values.span())
}

fn _enum(values: Array<Option<Layout>>) -> Layout {
    let mut items = array![];
    let mut i = 0;

    loop {
        if i >= values.len() {
            break;
        }

        let v = *values.at(i);
        match v {
            Option::Some(v) => { items.append(field(i.into(), v)); },
            Option::None => { items.append(field(i.into(), fixed(array![]))) },
        }

        i += 1;
    }

    Layout::Enum(items.span())
}

fn arr(item_layout: Layout) -> Layout {
    Layout::Array([item_layout].span())
}

#[test]
#[available_gas(l2_gas: 2000000)]
fn test_generic_introspect() {
    let _generic = Generic { value: Base { value: 123 } };
}

#[test]
fn test_size_basic_struct() {
    let size = Introspect::<Base>::size();
    assert!(size.is_some());
    assert!(size.unwrap() == 1);
}

#[test]
fn test_size_with_array() {
    assert!(Introspect::<WithArray>::size().is_none());
}

#[test]
fn test_size_with_byte_array() {
    assert!(Introspect::<WithByteArray>::size().is_none());
}

#[test]
fn test_size_with_tuple() {
    let size = Introspect::<WithTuple>::size();
    assert!(size.is_some());
    assert!(size.unwrap() == 4);
}

#[test]
fn test_size_with_nested_tuple() {
    let size = Introspect::<WithNestedTuple>::size();
    assert!(size.is_some());
    assert!(size.unwrap() == 7);
}

#[test]
fn test_size_with_nested_array_in_tuple() {
    let size = Introspect::<WithNestedArrayInTuple>::size();
    assert!(size.is_none());
}

#[test]
fn test_size_of_enum_without_variant_data() {
    let size = Introspect::<EnumNoData>::size();
    assert!(size.is_some());
    assert!(size.unwrap() == 1);
}

#[test]
fn test_size_of_enum_with_same_variant_data() {
    let size = Introspect::<EnumWithSameData>::size();
    assert!(size.is_some());
    assert!(size.unwrap() == 3);
}

#[test]
fn test_size_of_enum_with_same_tuple_variant_data() {
    let size = Introspect::<EnumWithSameTupleData>::size();
    assert!(size.is_some());
    assert!(size.unwrap() == 4);
}


#[test]
fn test_size_of_struct_with_option() {
    let size = Introspect::<StructWithOption>::size();
    assert!(size.is_none());
}

#[test]
fn test_size_of_enum_with_variant_data() {
    let size = Introspect::<EnumWithVariousData>::size();
    assert!(size.is_none());
}

#[test]
fn test_layout_of_enum_without_variant_data() {
    let layout = Introspect::<EnumNoData>::layout();
    let expected = _enum(array![ // One
    Option::None, // Two
    Option::None, // Three
    Option::None]);

    assert!(layout == expected);
}

#[test]
fn test_layout_of_enum_with_variant_data() {
    let layout = Introspect::<EnumWithVariousData>::layout();
    let expected = _enum(
        array![
            // One
            Option::Some(fixed(array![32])),
            // Two
            Option::Some(tuple(array![fixed(array![8]), fixed(array![16])])),
            // Three
            Option::Some(arr(fixed(array![128]))),
        ],
    );

    assert!(layout == expected);
}

#[test]
fn test_layout_of_struct_with_option() {
    let layout = Introspect::<StructWithOption>::layout();
    let expected = Layout::Struct(
        array![field(selector!("x"), _enum(array![Option::Some(fixed(array![16])), Option::None]))]
            .span(),
    );

    assert!(layout == expected);
}

#[test]
fn test_layout_of_packed_struct() {
    let layout = Introspect::<Vec3>::layout();
    let expected = Layout::Fixed([32, 32, 32].span());

    assert!(layout == expected);
}

#[test]
fn test_layout_of_inner_packed_struct() {
    let layout = Introspect::<Translation>::layout();
    let expected = Layout::Fixed([32, 32, 32, 32, 32, 32].span());

    assert!(layout == expected);
}

#[test]
#[should_panic(expected: "A packed model layout must contain Fixed layouts only.")]
fn test_layout_of_not_packed_inner_struct() {
    let _ = Introspect::<StructInnerNotPacked>::layout();
}


#[test]
fn test_layout_of_packed_enum() {
    let layout = Introspect::<EnumPacked>::layout();
    let expected = Layout::Fixed([8, 32].span());

    assert!(layout == expected);
}

#[test]
fn test_layout_of_inner_packed_enum() {
    let layout = Introspect::<EnumInnerPacked>::layout();
    let expected = Layout::Fixed([8, 8, 32, 32, 32, 32].span());

    assert!(layout == expected);
}

#[test]
#[should_panic(expected: "A packed model layout must contain Fixed layouts only.")]
fn test_layout_of_not_packed_inner_enum() {
    let _ = Introspect::<EnumInnerNotPacked>::layout();
}

#[test]
fn test_introspect_upgrade() {
    let p = Ty::Primitive('u8');
    let s = Ty::Struct(Struct { name: 's', attrs: [].span(), children: [].span() });
    let e = Ty::Enum(Enum { name: 'e', attrs: [].span(), children: [].span() });
    let t = Ty::Tuple([Ty::Primitive('u8')].span());
    let a = Ty::Array([Ty::Primitive('u8')].span());
    let b = Ty::ByteArray;

    assert!(p.is_an_upgrade_of(@p));
    assert!(!p.is_an_upgrade_of(@s));
    assert!(!p.is_an_upgrade_of(@e));
    assert!(!p.is_an_upgrade_of(@t));
    assert!(!p.is_an_upgrade_of(@a));
    assert!(!p.is_an_upgrade_of(@b));

    assert!(!s.is_an_upgrade_of(@p));
    assert!(s.is_an_upgrade_of(@s));
    assert!(!s.is_an_upgrade_of(@e));
    assert!(!s.is_an_upgrade_of(@t));
    assert!(!s.is_an_upgrade_of(@a));
    assert!(!s.is_an_upgrade_of(@b));

    assert!(!e.is_an_upgrade_of(@p));
    assert!(!e.is_an_upgrade_of(@s));
    assert!(e.is_an_upgrade_of(@e));
    assert!(!e.is_an_upgrade_of(@t));
    assert!(!e.is_an_upgrade_of(@a));
    assert!(!e.is_an_upgrade_of(@b));

    assert!(!t.is_an_upgrade_of(@p));
    assert!(!t.is_an_upgrade_of(@s));
    assert!(!t.is_an_upgrade_of(@e));
    assert!(t.is_an_upgrade_of(@t));
    assert!(!t.is_an_upgrade_of(@a));
    assert!(!t.is_an_upgrade_of(@b));

    assert!(!a.is_an_upgrade_of(@p));
    assert!(!a.is_an_upgrade_of(@s));
    assert!(!a.is_an_upgrade_of(@e));
    assert!(!a.is_an_upgrade_of(@t));
    assert!(a.is_an_upgrade_of(@a));
    assert!(!a.is_an_upgrade_of(@b));

    assert!(!b.is_an_upgrade_of(@p));
    assert!(!b.is_an_upgrade_of(@s));
    assert!(!b.is_an_upgrade_of(@e));
    assert!(!b.is_an_upgrade_of(@t));
    assert!(!b.is_an_upgrade_of(@a));
    assert!(b.is_an_upgrade_of(@b));
}

#[test]
fn test_primitive_upgrade() {
    let primitives = [
        'bool', 'u8', 'u16', 'u32', 'u64', 'u128', 'u256', 'i8', 'i16', 'i32', 'i64', 'i128',
        'felt252', 'ClassHash', 'ContractAddress', 'EthAddress',
    ]
        .span();

    let mut allowed_upgrades: Span<(felt252, Span<felt252>)> = [
        ('bool', ['felt252'].span()),
        ('u8', ['u16', 'u32', 'usize', 'u64', 'u128', 'felt252'].span()),
        ('u16', ['u32', 'usize', 'u64', 'u128', 'felt252'].span()),
        ('u32', ['usize', 'u64', 'u128', 'felt252'].span()), ('u128', ['felt252'].span()),
        ('u256', [].span()), ('i8', ['i16', 'i32', 'i64', 'i128', 'felt252'].span()),
        ('i16', ['i32', 'i64', 'i128', 'felt252'].span()),
        ('i32', ['i64', 'i128', 'felt252'].span()), ('i64', ['i128', 'felt252'].span()),
        ('i128', ['felt252'].span()), ('felt252', ['ClassHash', 'ContractAddress'].span()),
        ('ClassHash', ['felt252', 'ContractAddress'].span()),
        ('ContractAddress', ['felt252', 'ClassHash'].span()),
        ('EthAddress', ['felt252', 'ClassHash', 'ContractAddress'].span()),
    ]
        .span();

    loop {
        match allowed_upgrades.pop_front() {
            Option::Some((
                src, allowed,
            )) => {
                for dest in primitives {
                    let expected = if src == dest {
                        true
                    } else {
                        let allowed = *allowed;
                        let mut i = 0;

                        loop {
                            if i >= allowed.len() {
                                break false;
                            }

                            if *allowed.at(i) == *dest {
                                break true;
                            }

                            i += 1;
                        }
                    };

                    assert_eq!(
                        Ty::Primitive(*dest).is_an_upgrade_of(@Ty::Primitive(*src)),
                        expected,
                        "src: {} dest: {}",
                        *src,
                        *dest,
                    );
                }
            },
            Option::None => { break; },
        };
    }
}

#[test]
fn test_primitive_upgrade_backward_compatibility() {
    // Some models may have been deployed with `ContractAddress` and `ClassHash`
    // primitives, stored with the `starknet::` prefix in the introspection data structure.
    assert!(
        Ty::Primitive('starknet::ContractAddress')
            .is_an_upgrade_of(@Ty::Primitive('starknet::Classhash')),
    );
}

#[test]
#[should_panic(
    expected: "The introspection of the primitive type 33053979968501614 is not supported.",
)]
fn test_unknown_primitive() {
    let _ = Ty::Primitive('unknown').is_an_upgrade_of(@Ty::Primitive('u8'));
}

#[test]
#[should_panic(
    expected: "Prefer using u32 instead of usize as usize size is architecture-dependent.",
)]
fn test_usize_primitive() {
    let _ = Ty::Primitive('usize').is_an_upgrade_of(@Ty::Primitive('u8'));
}

#[test]
fn test_struct_upgrade() {
    let s = Struct {
        name: 's',
        attrs: ['one'].span(),
        children: [
            Member { name: 'x', attrs: ['two'].span(), ty: Ty::Primitive('u8') },
            Member { name: 'y', attrs: ['three'].span(), ty: Ty::Primitive('u16') },
        ]
            .span(),
    };

    // different name
    let mut upgraded = s;
    upgraded.name = 'upgraded';
    assert!(!upgraded.is_an_upgrade_of(@s), "different name");

    // different attributes
    let mut upgraded = s;
    upgraded.attrs = [].span();
    assert!(!upgraded.is_an_upgrade_of(@s), "different attributes");

    // member name changed
    let mut upgraded = s;
    upgraded
        .children =
            [
                Member { name: 'new', attrs: ['two'].span(), ty: Ty::Primitive('u8') },
                Member { name: 'y', attrs: ['three'].span(), ty: Ty::Primitive('u16') },
            ]
        .span();
    assert!(!upgraded.is_an_upgrade_of(@s), "member name changed");

    // member attr changed
    let mut upgraded = s;
    upgraded
        .children =
            [
                Member { name: 'x', attrs: [].span(), ty: Ty::Primitive('u8') },
                Member { name: 'y', attrs: ['three'].span(), ty: Ty::Primitive('u16') },
            ]
        .span();
    assert!(!upgraded.is_an_upgrade_of(@s), "member attr changed");

    // allowed member change
    let mut upgraded = s;
    upgraded
        .children =
            [
                Member { name: 'x', attrs: ['two'].span(), ty: Ty::Primitive('u16') },
                Member { name: 'y', attrs: ['three'].span(), ty: Ty::Primitive('u16') },
            ]
        .span();
    assert!(upgraded.is_an_upgrade_of(@s), "allowed member change");

    // wrong member change
    let mut upgraded = s;
    upgraded
        .children =
            [
                Member { name: 'x', attrs: ['two'].span(), ty: Ty::Primitive('u8') },
                Member { name: 'y', attrs: ['three'].span(), ty: Ty::Primitive('u8') },
            ]
        .span();
    assert!(!upgraded.is_an_upgrade_of(@s), "wrong member change");

    // new member
    let mut upgraded = s;
    upgraded
        .children =
            [
                Member { name: 'x', attrs: ['two'].span(), ty: Ty::Primitive('u8') },
                Member { name: 'y', attrs: ['three'].span(), ty: Ty::Primitive('u16') },
                Member { name: 'z', attrs: ['four'].span(), ty: Ty::Primitive('u32') },
            ]
        .span();
    assert!(upgraded.is_an_upgrade_of(@s), "new member");
}

#[test]
fn test_enum_upgrade() {
    let e = Enum {
        name: 'e',
        attrs: ['one'].span(),
        children: [('x', Ty::Primitive('u8')), ('y', Ty::Primitive('u16'))].span(),
    };

    // different name
    let mut upgraded = e;
    upgraded.name = 'upgraded';
    assert!(!upgraded.is_an_upgrade_of(@e), "different name");

    // different attributes
    let mut upgraded = e;
    upgraded.attrs = [].span();
    assert!(!upgraded.is_an_upgrade_of(@e), "different attrs");

    // variant name changed
    let mut upgraded = e;
    upgraded.children = [('new', Ty::Primitive('u8')), ('y', Ty::Primitive('u16'))].span();
    assert!(!upgraded.is_an_upgrade_of(@e), "variant name changed");

    // allowed variant change
    let mut upgraded = e;
    upgraded.children = [('x', Ty::Primitive('u16')), ('y', Ty::Primitive('u16'))].span();
    assert!(upgraded.is_an_upgrade_of(@e), "allowed variant change");

    // wrong variant change
    let mut upgraded = e;
    upgraded.children = [('x', Ty::Primitive('u8')), ('y', Ty::Primitive('u8'))].span();
    assert!(!upgraded.is_an_upgrade_of(@e), "wrong variant change");

    // new member
    let mut upgraded = e;
    upgraded
        .children =
            [('x', Ty::Primitive('u8')), ('y', Ty::Primitive('u16')), ('z', Ty::Primitive('u32'))]
        .span();
    assert!(upgraded.is_an_upgrade_of(@e), "new member");

    let e = Enum {
        name: 'e',
        attrs: [].span(),
        children: [('x', Ty::Tuple([].span())), ('y', Ty::Tuple([].span()))].span(),
    };

    // A variant without data (empty tuple / unit type) cannot be upgraded with data
    let mut upgraded = e;
    upgraded.children = [('x', Ty::Primitive('u8')), ('y', Ty::Tuple([].span()))].span();

    assert!(!upgraded.is_an_upgrade_of(@e), "variant without data");

    // special case: Option<T>
    let e = Introspect::<Option<u8>>::ty();
    let upgraded = Introspect::<Option<u32>>::ty();

    assert!(upgraded.is_an_upgrade_of(@e), "Option<T>");
}

#[test]
fn test_tuple_upgrade() {
    let t = Ty::Tuple([Ty::Primitive('u8'), Ty::Primitive('u16')].span());

    // tuple item is upgradable
    let upgraded = Ty::Tuple([Ty::Primitive('u16'), Ty::Primitive('u16')].span());
    assert!(upgraded.is_an_upgrade_of(@t));

    // tuple item is not upgradable
    let upgraded = Ty::Tuple([Ty::Primitive('bool'), Ty::Primitive('u16')].span());
    assert!(!upgraded.is_an_upgrade_of(@t));

    // tuple length changed
    let upgraded = Ty::Tuple(
        [Ty::Primitive('u8'), Ty::Primitive('u16'), Ty::Primitive('u32')].span(),
    );
    assert!(upgraded.is_an_upgrade_of(@t));
}

#[test]
fn test_array_upgrade() {
    let a = Ty::Array([Ty::Primitive('u8')].span());

    // array item is upgradable
    let upgraded = Ty::Array([Ty::Primitive('u16')].span());
    assert!(upgraded.is_an_upgrade_of(@a));

    // array item is not upgradable
    let upgraded = Ty::Array([Ty::Primitive('bool')].span());
    assert!(!upgraded.is_an_upgrade_of(@a));
}

#[test]
#[available_gas(l2_gas: 360000)]
fn test_primitive_upgrade_performance() {
    let gas = GasCounterTrait::start();
    let _ = Ty::Primitive('ClassHash').is_an_upgrade_of(@Ty::Primitive('ContractAddress'));
    gas.end("Upgrade from ContractAddress to ClassHash");
}

#[test]
fn test_key_member_upgrade() {
    let s = Struct {
        name: 's',
        attrs: [].span(),
        children: [
            Member { name: 'x', attrs: ['key'].span(), ty: Ty::Primitive('u8') },
            Member {
                name: 'y',
                attrs: ['key'].span(),
                ty: Ty::Enum(
                    Enum {
                        name: 'e',
                        attrs: [].span(),
                        children: [('A', Ty::Primitive('u8')), ('B', Ty::Primitive('u16'))].span(),
                    },
                ),
            },
        ]
            .span(),
    };

    // primitive type
    let mut upgraded = s;
    upgraded
        .children =
            [Member { name: 'x', attrs: ['key'].span(), ty: Ty::Primitive('u128') }, *s.children[1]]
        .span();

    assert!(upgraded.is_an_upgrade_of(@s), "key primitive type upgrade");

    // enum type / new variant
    let mut upgraded = s;
    upgraded
        .children =
            [
                *s.children[0],
                Member {
                    name: 'y',
                    attrs: ['key'].span(),
                    ty: Ty::Enum(
                        Enum {
                            name: 'e',
                            attrs: [].span(),
                            children: [
                                ('A', Ty::Primitive('u8')), ('B', Ty::Primitive('u16')),
                                ('C', Ty::Primitive('u32')),
                            ]
                                .span(),
                        },
                    ),
                },
            ]
        .span();

    assert!(upgraded.is_an_upgrade_of(@s), "key enum type upgrade (variant added)");

    // enum type / variant data upgrade (not allowed)
    let mut upgraded = s;
    upgraded
        .children =
            [
                *s.children[0],
                Member {
                    name: 'y',
                    attrs: ['key'].span(),
                    ty: Ty::Enum(
                        Enum {
                            name: 'e',
                            attrs: [].span(),
                            children: [('A', Ty::Primitive('u8')), ('B', Ty::Primitive('u32'))]
                                .span(),
                        },
                    ),
                },
            ]
        .span();

    assert!(!upgraded.is_an_upgrade_of(@s), "key enum type upgrade (variant data upgraded)");

    // struct type (not allowed)
    let s = Struct {
        name: 's',
        attrs: [].span(),
        children: [
            Member {
                name: 'x',
                attrs: ['key'].span(),
                ty: Ty::Struct(Struct { name: 'n', attrs: [].span(), children: [].span() }),
            },
        ]
            .span(),
    };

    let mut upgraded = s;
    upgraded
        .children =
            [
                Member {
                    name: 'x',
                    attrs: ['key'].span(),
                    ty: Ty::Struct(
                        Struct {
                            name: 'n',
                            attrs: [].span(),
                            children: [
                                Member { name: 'y', attrs: [].span(), ty: Ty::Primitive('u16') },
                            ]
                                .span(),
                        },
                    ),
                },
            ]
        .span();

    assert!(!upgraded.is_an_upgrade_of(@s), "key struct type upgrade");

    // array type (not allowed)
    let s = Struct {
        name: 's',
        attrs: [].span(),
        children: [
            Member {
                name: 'x', attrs: ['key'].span(), ty: Ty::Array([Ty::Primitive('u8')].span()),
            },
        ]
            .span(),
    };

    let mut upgraded = s;
    upgraded
        .children =
            [
                Member {
                    name: 'x', attrs: ['key'].span(), ty: Ty::Array([Ty::Primitive('u16')].span()),
                },
            ]
        .span();

    assert!(!upgraded.is_an_upgrade_of(@s), "key array type upgrade");

    // tuple type (not allowed)
    let s = Struct {
        name: 's',
        attrs: [].span(),
        children: [
            Member {
                name: 'x', attrs: ['key'].span(), ty: Ty::Tuple([Ty::Primitive('u8')].span()),
            },
        ]
            .span(),
    };

    let mut upgraded = s;
    upgraded
        .children =
            [
                Member {
                    name: 'x', attrs: ['key'].span(), ty: Ty::Tuple([Ty::Primitive('u16')].span()),
                },
            ]
        .span();

    assert!(!upgraded.is_an_upgrade_of(@s), "key tuple type upgrade");
}
