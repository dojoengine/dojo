use core::num::traits::Zero;
use core::ops::AddAssign;
use core::option::Option;


/// Indicates if at least one array item is None.
pub fn any_none<T>(arr: @Array<Option<T>>) -> bool {
    let mut i = 0;
    let mut res = false;
    loop {
        if i >= arr.len() {
            break;
        }

        if arr.at(i).is_none() {
            res = true;
            break;
        }
        i += 1;
    };
    res
}

/// Compute the sum of array items.
/// Note that there is no overflow check as we expect small array items.
pub fn sum<T, +Drop<T>, +Copy<T>, +AddAssign<T, T>, +Zero<T>>(arr: Array<Option<T>>) -> T {
    let mut i = 0;
    let mut res = Zero::<T>::zero();

    loop {
        if i >= arr.len() {
            break res;
        }

        match *arr.at(i) {
            Option::Some(x) => res += x,
            Option::None => {}
        }

        i += 1;
    }
}

