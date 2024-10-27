/// A `EventStorage` trait that abstracts where the storage is and how events are emitted.
pub trait EventStorage<S, E> {
    fn emit(ref self: S, event: @E);
}

pub trait EventStorageTest<S, E> {
    fn emit_test(ref self: S, event: @E);
}
