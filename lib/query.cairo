use array::ArrayTrait;

#[derive(Copy, Drop)]
struct Entity {}

struct Or<T> {}

struct Option<T> {}

struct Query<T> {
    data: Array::<T>, 
}

trait QueryTrait<T> {
    fn new() -> Query::<T>;
    fn append(ref self: Query::<T>, value: T);
    fn get(ref self: Query::<T>, index: usize) -> Option::<T>;
    fn at(ref self: Query::<T>, index: usize) -> T;
    fn len(ref self: Query::<T>) -> usize;
}

impl QueryImpl<T> of QueryTrait::<T> {
    #[inline(always)]
    fn new() -> Query::<T> {
        Query { data: ArrayTrait::new(),  }
    }

    fn append(ref self: Query::<T>, value: T) {
        let mut data = self.data;
        array_append(ref data, value)
    }

    fn get(ref self: Query::<T>, index: usize) -> Option::<T> {
        let mut data = self.data;
        array_get(ref data, index)
    }

    fn at(ref self: Query::<T>, index: usize) -> T {
        let mut data = self.data;
        array_at(ref data, index)
    }

    fn len(ref self: Query::<T>) -> usize {
        let mut data = self.data;
        array_len(ref data)
    }
}

impl QueryDrop of Drop::<Query::<felt>>;

#[test]
fn test_query() {
    let mut query = QueryTrait::<felt>::new();
    query.append(1);
}
