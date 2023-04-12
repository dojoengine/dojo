use gas::withdraw_gas;
use option::OptionTrait;
use result::ResultTrait;
use result::ResultTraitImpl;
use traits::Into;

use dojo_physics::cubit::math;
use dojo_physics::cubit::trig;

// CONSTANTS

const PRIME: felt252 = 3618502788666131213697322783095070105623107215331596699973092056135872020480;
const HALF_PRIME: felt252 =
    1809251394333065606848661391547535052811553607665798349986546028067936010240;
const ONE: felt252 = 18446744073709551616; // 2 ** 64
const ONE_u128: u128 = 18446744073709551616_u128; // 2 ** 64
const HALF: felt252 = 9223372036854775808; // 2 ** 63
const HALF_u128: u128 = 9223372036854775808_u128; // 2 ** 63
const WIDE_SHIFT_u128: u128 = 18446744073709551616_u128; // 2 ** 64
const MAX_u128: u128 = 340282366920938463463374607431768211455_u128; // 2 ** 128 - 1

// STRUCTS

#[derive(Copy, Drop)]
struct FixedType {
    mag: u128,
    sign: bool
}

// TRAITS

trait Fixed {
    // Constructors
    fn new(mag: u128, sign: bool) -> FixedType;
    fn new_unscaled(mag: u128, sign: bool) -> FixedType;
    fn from_felt252(val: felt252) -> FixedType;
    fn from_unscaled_felt252(val: felt252) -> FixedType;
    // Math
    fn abs(self: FixedType) -> FixedType;
    fn ceil(self: FixedType) -> FixedType;
    fn exp(self: FixedType) -> FixedType;
    fn exp2(self: FixedType) -> FixedType;
    fn floor(self: FixedType) -> FixedType;
    fn ln(self: FixedType) -> FixedType;
    fn log2(self: FixedType) -> FixedType;
    fn log10(self: FixedType) -> FixedType;
    fn pow(self: FixedType, b: FixedType) -> FixedType;
    fn round(self: FixedType) -> FixedType;
    fn sqrt(self: FixedType) -> FixedType;
    // Trigonometry
    fn acos(self: FixedType) -> FixedType;
    fn asin(self: FixedType) -> FixedType;
    fn atan(self: FixedType) -> FixedType;
    fn cos(self: FixedType) -> FixedType;
    fn sin(self: FixedType) -> FixedType;
    fn tan(self: FixedType) -> FixedType;
}

// IMPLS

impl FixedImpl of Fixed {
    fn new(mag: u128, sign: bool) -> FixedType {
        return FixedType { mag: mag, sign: sign };
    }

    fn new_unscaled(mag: u128, sign: bool) -> FixedType {
        return Fixed::new(mag * ONE_u128, sign);
    }

    fn from_felt252(val: felt252) -> FixedType {
        let mag = integer::u128_try_from_felt252(_felt252_abs(val)).unwrap();
        return Fixed::new(mag, _felt252_sign(val));
    }

    fn from_unscaled_felt252(val: felt252) -> FixedType {
        return Fixed::from_felt252(val * ONE);
    }

    fn abs(self: FixedType) -> FixedType {
        return math::abs(self);
    }

    fn acos(self: FixedType) -> FixedType {
        return trig::acos(self);
    }

    fn asin(self: FixedType) -> FixedType {
        return trig::asin(self);
    }

    fn atan(self: FixedType) -> FixedType {
        return trig::atan(self);
    }

    fn ceil(self: FixedType) -> FixedType {
        return math::ceil(self);
    }

    fn cos(self: FixedType) -> FixedType {
        return trig::cos(self);
    }

    fn floor(self: FixedType) -> FixedType {
        return math::floor(self);
    }

    // Calculates the natural exponent of x: e^x
    fn exp(self: FixedType) -> FixedType {
        return math::exp(self);
    }

    // Calculates the binary exponent of x: 2^x
    fn exp2(self: FixedType) -> FixedType {
        return math::exp2(self);
    }

    // Calculates the natural logarithm of x: ln(x)
    // self must be greater than zero
    fn ln(self: FixedType) -> FixedType {
        return math::ln(self);
    }

    // Calculates the binary logarithm of x: log2(x)
    // self must be greather than zero
    fn log2(self: FixedType) -> FixedType {
        return math::log2(self);
    }

    // Calculates the base 10 log of x: log10(x)
    // self must be greater than zero
    fn log10(self: FixedType) -> FixedType {
        return math::log10(self);
    }

    // Calclates the value of x^y and checks for overflow before returning
    // self is a fixed point value
    // b is a fixed point value
    fn pow(self: FixedType, b: FixedType) -> FixedType {
        return math::pow(self, b);
    }

    fn round(self: FixedType) -> FixedType {
        return math::round(self);
    }

    fn sin(self: FixedType) -> FixedType {
        return trig::sin(self);
    }

    // Calculates the square root of a fixed point value
    // x must be positive
    fn sqrt(self: FixedType) -> FixedType {
        return math::sqrt(self);
    }

    fn tan(self: FixedType) -> FixedType {
        return trig::tan(self);
    }
}

impl FixedInto of Into<FixedType, felt252> {
    fn into(self: FixedType) -> felt252 {
        let mag_felt252 = self.mag.into();

        if (self.sign == true) {
            return mag_felt252 * -1;
        } else {
            return mag_felt252;
        }
    }
}

impl FixedPartialEq of PartialEq<FixedType> {
    #[inline(always)]
    fn eq(a: FixedType, b: FixedType) -> bool {
        return math::eq(a, b);
    }

    #[inline(always)]
    fn ne(a: FixedType, b: FixedType) -> bool {
        return math::ne(a, b);
    }
}

impl FixedAdd of Add<FixedType> {
    fn add(a: FixedType, b: FixedType) -> FixedType {
        return math::add(a, b);
    }
}

impl FixedAddEq of AddEq<FixedType> {
    #[inline(always)]
    fn add_eq(ref self: FixedType, other: FixedType) {
        self = Add::add(self, other);
    }
}

impl FixedSub of Sub<FixedType> {
    fn sub(a: FixedType, b: FixedType) -> FixedType {
        return math::sub(a, b);
    }
}

impl FixedSubEq of SubEq<FixedType> {
    #[inline(always)]
    fn sub_eq(ref self: FixedType, other: FixedType) {
        self = Sub::sub(self, other);
    }
}

impl FixedMul of Mul<FixedType> {
    fn mul(a: FixedType, b: FixedType) -> FixedType {
        return math::mul(a, b);
    }
}

impl FixedMulEq of MulEq<FixedType> {
    #[inline(always)]
    fn mul_eq(ref self: FixedType, other: FixedType) {
        self = Mul::mul(self, other);
    }
}

impl FixedDiv of Div<FixedType> {
    fn div(a: FixedType, b: FixedType) -> FixedType {
        return math::div(a, b);
    }
}

impl FixedDivEq of DivEq<FixedType> {
    #[inline(always)]
    fn div_eq(ref self: FixedType, other: FixedType) {
        self = Div::div(self, other);
    }
}

impl FixedPartialOrd of PartialOrd<FixedType> {
    #[inline(always)]
    fn ge(a: FixedType, b: FixedType) -> bool {
        return math::ge(a, b);
    }

    #[inline(always)]
    fn gt(a: FixedType, b: FixedType) -> bool {
        return math::gt(a, b);
    }

    #[inline(always)]
    fn le(a: FixedType, b: FixedType) -> bool {
        return math::le(a, b);
    }

    #[inline(always)]
    fn lt(a: FixedType, b: FixedType) -> bool {
        return math::lt(a, b);
    }
}

impl FixedNeg of Neg<FixedType> {
    #[inline(always)]
    fn neg(a: FixedType) -> FixedType {
        return math::neg(a);
    }
}

// INTERNAL

// Returns the sign of a signed `felt252` as with signed magnitude representation
// true = negative
// false = positive
fn _felt252_sign(a: felt252) -> bool {
    return integer::u256_from_felt252(a) > integer::u256_from_felt252(HALF_PRIME);
}

// Returns the absolute value of a signed `felt252`
fn _felt252_abs(a: felt252) -> felt252 {
    let a_sign = _felt252_sign(a);

    if (a_sign == true) {
        return a * -1;
    } else {
        return a;
    }
}

// Ignores sign and always returns false
fn _split_unsigned(a: FixedType) -> (u128, u128) {
    return integer::u128_safe_divmod(a.mag, integer::u128_as_non_zero(ONE_u128));
}
