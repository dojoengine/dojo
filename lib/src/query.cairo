use dict::DictFeltToTrait;
use array::ArrayTrait;

#[derive(Copy, Drop)]
struct Entity {}
struct Or<S, T> {}
#[derive(Copy, Drop)]
struct With<S, T> {}
#[derive(Copy, Drop)]
struct Without<S, T> {}
#[derive(Copy, Drop)]
struct Caller {}
#[derive(Copy, Drop)]
struct Input {}
#[derive(Copy, Drop)]
struct EntityID<T> {}
#[derive(Copy, Drop)]
struct Query<T> {}

trait QueryTrait<T> {
    fn ids() -> Array::<usize>;
    fn id() -> usize;
    fn len() -> u32;
// fn insert(ref self: Query::<T>, key: felt, value: T);
// fn get(ref self: Query::<T>, index: felt) -> T;
}

impl QueryImpl<T> of QueryTrait::<T> {
    #[inline(always)]
    fn ids() -> Array::<usize> {
        let mut arr = ArrayTrait::<usize>::new();
        arr.append(0_u32);
        arr
    }

    #[inline(always)]
    fn id() -> usize {
        0_u32
    }

    fn len() -> u32 {
        0_u32
    }
}

// #[test]
// fn test_query() {
//     let mut query = QueryTrait::<felt>::new();
//     query.insert(1, 1);
// }


