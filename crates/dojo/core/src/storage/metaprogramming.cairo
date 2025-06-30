//! Metaprogramming utilities based on:
// https://github.com/starkware-libs/cairo/blob/main/corelib/src/metaprogramming.cairo

/// A trait that can be used to disable implementations based on the types of the generic args.
/// Assumes that `DojoTypeEqualImpl<T>` is the only implementation of this trait.
///
/// Primarily used for optimizations by enabling type-specific implementations.
/// Since `DojoTypeEqualImpl<T>` is the only implementation, adding `-DojoTypeEqual<T, U>` as a
/// trait bound ensures the implementation is only available when T and U are different types.
pub trait DojoTypeEqual<S, T> {}

impl DojoTypeEqualImpl<T> of DojoTypeEqual<T, T>;

/// Marker trait for types that are tuples.
/// Currently supports tuples of size 0 to 10.
pub(crate) trait DojoIsTuple<T>;


/// A trait for splitting a tuple into head element and a tail tuple, as well as reconstructing from
/// them.
pub(crate) trait DojoTupleSplit<T> {
    /// The type of the first element of the tuple.
    type Head;
    /// The type of the rest of the tuple.
    type Rest;
    /// Splits the tuple into the head and the rest.
    fn split_head(self: T) -> (Self::Head, Self::Rest) nopanic;
    /// Reconstructs the tuple from the head and the rest.
    fn reconstruct(head: Self::Head, rest: Self::Rest) -> T nopanic;
}


/// A trait for extending a tuple from the front.
pub(crate) trait DojoTupleExtendFront<T, E> {
    /// The type of the resulting tuple.
    type Result;
    /// Creates a new tuple from the `value` tuple with `element` in front of it.
    fn extend_front(value: T, element: E) -> Self::Result nopanic;
}


/// A trait for forwarding a wrapping snapshot from a tuple style struct into a tuple style struct
/// of the snapshots.
pub trait DojoTupleSnapForward<T> {
    type SnapForward;
    fn snap_forward(self: @T) -> Self::SnapForward nopanic;
}

/// A trait for removing a wrapping snapshot from the types in tuple style struct.
pub trait DojoSnapRemove<T> {
    type Result;
}

impl DojoSnapRemoveSnap<T> of DojoSnapRemove<@T> {
    type Result = T;
}

