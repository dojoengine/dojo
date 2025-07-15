#[derive(Introspect, Serde, Drop, Default)]
pub struct S1 {
    x: u16,
    y: u64,
    z: u128,
}

#[derive(Introspect, Serde, Drop, Default)]
pub enum E1 {
    #[default]
    A: u16,
    B: u64,
    C: u128,
}

#[derive(Introspect, Serde, Drop, Default)]
pub struct S2 {
    x: (S1, S1, S1),
    y: Array<S1>,
    z: Array<E1>,
}

#[derive(Introspect, Serde, Drop, Default)]
pub enum E2 {
    #[default]
    A: (S2, S2, S2),
    B: Array<S2>,
    C: Array<E1>,
}

#[dojo::model]
pub struct SmallModel {
    #[key]
    pub k1: felt252,
    pub k2: felt252,
    pub v1: u16,
    pub v2: (u8, u16),
}

#[dojo::model]
pub struct MediumModel {
    #[key]
    pub k1: felt252,
    pub k2: felt252,
    pub v1: u16,
    pub v2: (u8, u16),
    pub v3: S1,
    pub v4: E1,
    pub v5: Option<u8>,
    pub v6: Array<u16>,
}

#[dojo::model]
pub struct BigModel {
    #[key]
    pub k1: felt252,
    pub k2: felt252,
    pub v1: u16,
    pub v2: (u8, u16),
    pub v3: S2,
    pub v4: E2,
    pub v5: Option<u8>,
}


#[derive(IntrospectPacked, Serde, Drop, Default)]
pub struct S1Packed {
    x: u16,
    y: u64,
    z: u128,
}

#[derive(IntrospectPacked, Serde, Drop, Default)]
pub enum E1Packed {
    #[default]
    A: u128,
    B: u128,
    C: u128,
}

#[derive(IntrospectPacked, Serde, Drop, Default)]
pub struct S2Packed {
    x: (S1Packed, S1Packed, S1Packed),
    y: (S1Packed, S1Packed, S1Packed),
    z: (E1Packed, E1Packed, E1Packed),
}

#[derive(IntrospectPacked, Serde, Drop, Default)]
pub enum E2Packed {
    #[default]
    A: (S2Packed, S2Packed, S2Packed),
    B: (S2Packed, S2Packed, S2Packed),
    C: (S2Packed, S2Packed, S2Packed),
}

#[derive(IntrospectPacked)]
#[dojo::model]
pub struct SmallPackedModel {
    #[key]
    pub k1: felt252,
    pub k2: felt252,
    pub v1: u16,
    pub v2: (u8, u16),
}

#[derive(IntrospectPacked)]
#[dojo::model]
pub struct MediumPackedModel {
    #[key]
    pub k1: felt252,
    pub k2: felt252,
    pub v1: u16,
    pub v2: (u8, u16),
    pub v3: S1Packed,
    pub v4: E1Packed,
}

#[derive(IntrospectPacked)]
#[dojo::model]
pub struct BigPackedModel {
    #[key]
    pub k1: felt252,
    pub k2: felt252,
    pub v1: u16,
    pub v2: (u8, u16),
    pub v3: S2Packed,
    pub v4: E2Packed,
}

pub const fn s1() -> S1 {
    S1 { x: 123, y: 34789, z: 679678 }
}

pub const fn s1_packed() -> S1Packed {
    S1Packed { x: 123, y: 34789, z: 679678 }
}

pub fn s2() -> S2 {
    S2 {
        x: (s1(), s1(), s1()),
        y: array![s1(), s1(), s1(), s1(), s1(), s1(), s1(), s1(), s1()],
        z: array![
            E1::A(123), E1::B(234), E1::C(345), E1::A(123), E1::B(234), E1::C(345), E1::A(123),
            E1::B(234),
        ],
    }
}

pub fn s2_packed() -> S2Packed {
    S2Packed {
        x: (s1_packed(), s1_packed(), s1_packed()),
        y: (s1_packed(), s1_packed(), s1_packed()),
        z: (E1Packed::A(7886768), E1Packed::A(7886768), E1Packed::A(7886768)),
    }
}

pub fn e2() -> E2 {
    E2::B(array![s2(), s2(), s2(), s2()])
}

pub fn e2_packed() -> E2Packed {
    E2Packed::A((s2_packed(), s2_packed(), s2_packed()))
}

pub fn get_model_keys() -> (felt252, felt252) {
    (1, 2)
}

pub fn build_small_model() -> SmallModel {
    let (k1, k2) = get_model_keys();

    SmallModel { k1, k2, v1: 123, v2: (42, 789) }
}

pub fn build_medium_model() -> MediumModel {
    let (k1, k2) = get_model_keys();

    MediumModel {
        k1,
        k2,
        v1: 123,
        v2: (42, 789),
        v3: s1(),
        v4: E1::B(24789),
        v5: Some(35),
        v6: array![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
    }
}

pub fn build_big_model() -> BigModel {
    let (k1, k2) = get_model_keys();

    BigModel { k1, k2, v1: 35464, v2: (123, 678), v3: s2(), v4: e2(), v5: Some(35) }
}

pub fn build_small_packed_model() -> SmallPackedModel {
    let (k1, k2) = get_model_keys();
    SmallPackedModel { k1, k2, v1: 123, v2: (42, 789) }
}

pub fn build_medium_packed_model() -> MediumPackedModel {
    let (k1, k2) = get_model_keys();
    MediumPackedModel { k1, k2, v1: 123, v2: (42, 789), v3: s1_packed(), v4: E1Packed::A(67868) }
}

pub fn build_big_packed_model() -> BigPackedModel {
    let (k1, k2) = get_model_keys();
    BigPackedModel { k1, k2, v1: 123, v2: (42, 789), v3: s2_packed(), v4: e2_packed() }
}
