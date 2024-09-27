use lazy_static::lazy_static;
use num_bigint::{BigUint, ToBigUint};

lazy_static! {
    pub static ref TWO: BigUint = 2u32.to_biguint().unwrap();
}

pub fn fft(elements: Vec<BigUint>, xs: Vec<BigUint>, p: &BigUint) -> Vec<BigUint> {
    let n = elements.len();
    let mut transform = vec![BigUint::ZERO; n];

    for i in 0..n {
        for j in (0..n).rev() {
            let value = &transform[i] * &xs[i];
            let value = value + &elements[j];
            let value = value % p;
            transform[i] = value;
        }
    }

    transform
}

/// Performs the inverse Fast Fourier Transform on a vector of `BigUint`.
///
/// # Arguments
///
/// * `arr` - A vector of `BigUint` representing the input array.
/// * `xs` - A vector of `BigUint` representing the evaluation points.
/// * `p` - The modulus as a `BigUint`.
///
/// # Returns
///
/// A vector of `BigUint` representing the transformed array.
pub fn ifft(arr: Vec<BigUint>, xs: Vec<BigUint>, p: &BigUint) -> Vec<BigUint> {
    // Base case: return immediately if the array length is 1
    if arr.len() == 1 {
        return arr;
    }

    let n = arr.len() / 2;
    let mut res0 = Vec::with_capacity(n);
    let mut res1 = Vec::with_capacity(n);
    let mut new_xs = Vec::with_capacity(n);

    for i in (0..2 * n).step_by(2) {
        let a = &arr[i];
        let b = &arr[i + 1];
        let x = &xs[i];

        res0.push(div_mod(a + b, TWO.clone(), p));
        // Handle subtraction to avoid underflow
        let diff = if b > a { p - (b - a) } else { a - b };
        res1.push(div_mod(diff, TWO.clone() * x, p));

        new_xs.push(x.modpow(&TWO.clone(), p));
    }

    // Recursive calls
    let merged_res0 = ifft(res0, new_xs.clone(), p);
    let merged_res1 = ifft(res1, new_xs, p);

    // Merging the results
    let mut merged = Vec::with_capacity(arr.len());
    for i in 0..n {
        merged.push(merged_res0[i].clone());
        merged.push(merged_res1[i].clone());
    }
    merged
}

/// Divides two `BigUint` numbers modulo a third `BigUint` number.
///
/// # Arguments
///
/// * `a` - The numerator as a `BigUint`.
/// * `b` - The denominator as a `BigUint`.
/// * `p` - The modulus as a `BigUint`.
///
/// # Returns
///
/// The result of the division modulo `p` as a `BigUint`.
pub fn div_mod(a: BigUint, b: BigUint, p: &BigUint) -> BigUint {
    a * b.modpow(&(p - TWO.clone()), p) % p
}
