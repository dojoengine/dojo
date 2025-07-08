/// Computes the sum of the provided sizes.
/// If at least one size is None, returns None.
#[inline(always)]
pub fn sum_sizes(sizes: Array<Option<usize>>) -> Option<usize> {
    let mut total_size = 0;

    for s in sizes {
        if s.is_none() {
            return None;
        }
        total_size += s.unwrap();
    }

    Some(total_size)
}
