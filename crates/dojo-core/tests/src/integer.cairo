use integer::BoundedInt;
use option::OptionTrait;
use traits::TryInto;
use zeroable::Zeroable;

use dojo_core::integer::u250;
use dojo_core::integer::Felt252TryIntoU250;

#[test]
fn test_u250_felt252_conv() {
    let a: Option<u250> = 1_felt252.try_into();
    assert(a.is_some(), '1 try_into u250');

    // 250^2 - 1, max u250
    let m: Option<u250> =
        0x3ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff_felt252.try_into();
    assert(m.is_some(), 'max try_into u250');

    // 250^2, max u250 + 1
    let o: Option<u250> =
        0x400000000000000000000000000000000000000000000000000000000000000_felt252.try_into();
    assert(o.is_none(), 'max + 1 try_into u250');
}

#[test]
fn test_u250_zeroable() {
    let zero: u250 = Zeroable::zero();
    assert(zero.inner == 0, 'u250 Zeroable::zero');
    assert(zero.is_zero(), 'u250 Zeroable::is_zero');
    assert(u250 { inner: 250 }.is_non_zero(), 'u250 Zeroable::is_non_zero');
}

#[test]
fn test_u250_addition() {
    let one = u250 { inner: 1 };
    let two = u250 { inner: 2 };
    let three = u250 { inner: 3 };
    assert(one + two == three, 'u250 1 + 2 = 3');

    let mut n = three;
    n += one;
    assert(n == u250 { inner: 4 }, 'u250 (3 += 1) = 4')
}

#[test]
#[should_panic(expected: ('u250 overflow', ))]
fn test_u250_addition_overflow() {
    let _ = BoundedInt::max() + u250 { inner: 1 };
}

#[test]
fn test_u250_subtraction() {
    let one = u250 { inner: 1 };
    let two = u250 { inner: 2 };
    let three = u250 { inner: 3 };

    assert(three - one == two, 'u250 3 - 1 = 2');

    let mut n = three;
    n -= one;
    assert(n == two, 'u250 (3 -= 1) = 2');
}
