use dict::DictFelt252ToTrait;
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
    fn entity(id: usize) -> T;
    fn len() -> u32;
// fn insert(ref self: Query::<T>, key: felt252, value: T);
// fn get(ref self: Query::<T>, index: felt252) -> T;
}
// #[test]
// fn test_query() {
//     let mut query = QueryTrait::<felt252>::new();
//     query.insert(1, 1);
// }


