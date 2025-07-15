use core::array::ArrayTrait;

pub fn build_array<T, +Drop<T>, +Copy<T>>(size: u32, value: T) -> Array<T> {
    let mut array = ArrayTrait::<T>::new();

    for _ in 0..size {
        array.append(value);
    }

    array
}

pub fn build_array_from_values<T, +Drop<T>, +Copy<T>>(size: u32, values: Span<T>) -> Array<T> {
    let mut array = ArrayTrait::<T>::new();

    for _ in 0..size {
        array.append_span(values);
    }

    array
}

pub fn build_span<T, +Drop<T>, +Copy<T>>(size: u32, value: T) -> Span<T> {
    build_array(size, value).span()
}

pub fn build_span_from_values<T, +Drop<T>, +Copy<T>>(size: u32, values: Span<T>) -> Span<T> {
    build_array_from_values(size, values).span()
}
