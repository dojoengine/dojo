use array::ArrayTrait;
use gas::withdraw_gas;
use option::OptionTrait;
use result::ResultTrait;
use result::ResultTraitImpl;
use traits::Into;

use dojo_physics::cubit::core::HALF_u128;
use dojo_physics::cubit::core::MAX_u128;
use dojo_physics::cubit::core::ONE_u128;
use dojo_physics::cubit::core::WIDE_SHIFT_u128;
use dojo_physics::cubit::core::Fixed;
use dojo_physics::cubit::core::FixedInto;
use dojo_physics::cubit::core::FixedType;
use dojo_physics::cubit::core::FixedAdd;
use dojo_physics::cubit::core::FixedDiv;
use dojo_physics::cubit::core::FixedMul;
use dojo_physics::cubit::core::FixedNeg;


// PUBLIC

fn abs(a: FixedType) -> FixedType {
    return Fixed::new(a.mag, false);
}

fn add(a: FixedType, b: FixedType) -> FixedType {
    return Fixed::from_felt252(a.into() + b.into());
}

fn ceil(a: FixedType) -> FixedType {
    let (div_u128, rem_u128) = _split_unsigned(a);

    if (rem_u128 == 0_u128) {
        return a;
    } else if (a.sign == false) {
        return Fixed::new_unscaled(div_u128 + 1_u128, false);
    } else {
        return Fixed::from_unscaled_felt252(div_u128.into() * -1);
    }
}

fn div(a: FixedType, b: FixedType) -> FixedType {
    let res_sign = a.sign ^ b.sign;

    // Invert b to preserve precision as much as possible
    // TODO: replace if / when there is a felt252 div_rem supported
    let (a_high, a_low) = integer::u128_wide_mul(a.mag, ONE_u128);
    let b_inv = MAX_u128 / b.mag;
    let res_u128 = a_low / b.mag + a_high * b_inv;

    // Re-apply sign
    return Fixed::new(res_u128, res_sign);
}

fn eq(a: FixedType, b: FixedType) -> bool {
    return a.mag == b.mag & a.sign == b.sign;
}

// Calculates the natural exponent of x: e^x
fn exp(a: FixedType) -> FixedType {
    return exp2(Fixed::new(26613026195688644984_u128, false) * a);
}

// Calculates the binary exponent of x: 2^x
fn exp2(a: FixedType) -> FixedType {
    if (a.mag == 0_u128) {
        return Fixed::new(ONE_u128, false);
    }

    let (int_part, frac_part) = _split_unsigned(a);
    let int_res = _pow_int(Fixed::new_unscaled(2_u128, false), int_part, false);

    // 1.069e-7 maximum error
    let a1 = Fixed::new(18446742102121545016_u128, false);
    let a2 = Fixed::new(12786448315833223256_u128, false);
    let a3 = Fixed::new(4429795821981912136_u128, false);
    let a4 = Fixed::new(1030550312125424568_u128, false);
    let a5 = Fixed::new(164966079091297224_u128, false);
    let a6 = Fixed::new(34983544691898416_u128, false);

    let frac_fixed = Fixed::new(frac_part, false);
    let r6 = a6 * frac_fixed;
    let r5 = (r6 + a5) * frac_fixed;
    let r4 = (r5 + a4) * frac_fixed;
    let r3 = (r4 + a3) * frac_fixed;
    let r2 = (r3 + a2) * frac_fixed;
    let frac_res = r2 + a1;
    let res_u = int_res * frac_res;

    if (a.sign == true) {
        return Fixed::new(ONE_u128, false) / res_u;
    } else {
        return res_u;
    }
}

fn floor(a: FixedType) -> FixedType {
    let (div_u128, rem_u128) = _split_unsigned(a);

    if (rem_u128 == 0_u128) {
        return a;
    } else if (a.sign == false) {
        return Fixed::new_unscaled(div_u128, false);
    } else {
        return Fixed::from_unscaled_felt252(-1 * div_u128.into() - 1);
    }
}

fn ge(a: FixedType, b: FixedType) -> bool {
    if (a.sign != b.sign) {
        return !a.sign;
    } else {
        return (a.mag == b.mag) | ((a.mag > b.mag) ^ a.sign);
    }
}

fn gt(a: FixedType, b: FixedType) -> bool {
    if (a.sign != b.sign) {
        return !a.sign;
    } else {
        return (a.mag != b.mag) & ((a.mag > b.mag) ^ a.sign);
    }
}

fn le(a: FixedType, b: FixedType) -> bool {
    if (a.sign != b.sign) {
        return a.sign;
    } else {
        return (a.mag == b.mag) | ((a.mag < b.mag) ^ a.sign);
    }
}

// Calculates the natural logarithm of x: ln(x)
// self must be greater than zero
fn ln(a: FixedType) -> FixedType {
    return Fixed::new(12786308645202655660_u128, false) * log2(a); // ln(2) = 0.693...
}

// Calculates the binary logarithm of x: log2(x)
// self must be greather than zero
fn log2(a: FixedType) -> FixedType {
    match gas::withdraw_gas() {
        Option::Some(_) => {},
        Option::None(_) => {
            let mut data = ArrayTrait::new();
            data.append('Out of gas');
            panic(data);
        },
    }

    assert(a.sign == false, 'must be positive');

    if (a.mag == ONE_u128) {
        return Fixed::new(0_u128, false);
    } else if (a.mag < ONE_u128) {
        // Compute true inverse binary log if 0 < x < 1
        let div = Fixed::new_unscaled(1_u128, false) / a;
        return -log2(div);
    }

    let msb_u128 = _msb(a.mag / 2_u128);
    let divisor = _pow_int(Fixed::new_unscaled(2_u128, false), msb_u128, false);
    let norm = a / divisor;

    // 4.233e-8 maximum error
    let a1 = Fixed::new(63187350828072553424_u128, true);
    let a2 = Fixed::new(150429590981271126408_u128, false);
    let a3 = Fixed::new(184599081115266689944_u128, true);
    let a4 = Fixed::new(171296190111888966192_u128, false);
    let a5 = Fixed::new(110928274989790216568_u128, true);
    let a6 = Fixed::new(48676798788932142400_u128, false);
    let a7 = Fixed::new(13804762162529339368_u128, true);
    let a8 = Fixed::new(2284550827067371376_u128, false);
    let a9 = Fixed::new(167660832607149504_u128, true);

    let r9 = a9 * norm;
    let r8 = (r9 + a8) * norm;
    let r7 = (r8 + a7) * norm;
    let r6 = (r7 + a6) * norm;
    let r5 = (r6 + a5) * norm;
    let r4 = (r5 + a4) * norm;
    let r3 = (r4 + a3) * norm;
    let r2 = (r3 + a2) * norm;
    return r2 + a1 + Fixed::new_unscaled(msb_u128, false);
}

// Calculates the base 10 log of x: log10(x)
// self must be greater than zero
fn log10(a: FixedType) -> FixedType {
    return Fixed::new(5553023288523357132_u128, false) * log2(a); // log10(2) = 0.301...
}

fn lt(a: FixedType, b: FixedType) -> bool {
    if (a.sign != b.sign) {
        return a.sign;
    } else {
        return (a.mag != b.mag) & ((a.mag < b.mag) ^ a.sign);
    }
}

fn mul(a: FixedType, b: FixedType) -> FixedType {
    let res_sign = a.sign ^ b.sign;

    // Use u128 to multiply and shift back down
    // TODO: replace if / when there is a felt252 div_rem supported
    let (high, low) = integer::u128_wide_mul(a.mag, b.mag);
    let res_u128 = high * WIDE_SHIFT_u128 + (low / ONE_u128);

    // Re-apply sign
    return Fixed::new(res_u128, res_sign);
}

fn ne(a: FixedType, b: FixedType) -> bool {
    return a.mag != b.mag | a.sign != b.sign;
}

fn neg(a: FixedType) -> FixedType {
    if (a.sign == false) {
        return Fixed::new(a.mag, true);
    } else {
        return Fixed::new(a.mag, false);
    }
}

// Calclates the value of x^y and checks for overflow before returning
// self is a fixed point value
// b is a fixed point value
fn pow(a: FixedType, b: FixedType) -> FixedType {
    let (div_u128, rem_u128) = _split_unsigned(b);

    // use the more performant integer pow when y is an int
    if (rem_u128 == 0_u128) {
        return _pow_int(a, b.mag / ONE_u128, b.sign);
    }

    // x^y = exp(y*ln(x)) for x > 0 will error for x < 0
    return exp(b * ln(a));
}

fn round(a: FixedType) -> FixedType {
    let (div_u128, rem_u128) = _split_unsigned(a);

    if (HALF_u128 <= rem_u128) {
        return Fixed::new(ONE_u128 * (div_u128 + 1_u128), a.sign);
    } else {
        return Fixed::new(ONE_u128 * div_u128, a.sign);
    }
}

// Calculates the square root of a fixed point value
// x must be positive
fn sqrt(a: FixedType) -> FixedType {
    assert(a.sign == false, 'must be positive');
    let root = integer::u128_sqrt(a.mag);
    let scale_root = integer::u128_sqrt(ONE_u128);
    let res_u128 = root * ONE_u128 / scale_root;
    return Fixed::new(res_u128, false);
}

fn sub(a: FixedType, b: FixedType) -> FixedType {
    return Fixed::from_felt252(a.into() - b.into());
}

// INTERNAL

// Calculates the most significant bit
fn _msb(a: u128) -> u128 {
    match gas::withdraw_gas() {
        Option::Some(_) => {},
        Option::None(_) => {
            let mut data = ArrayTrait::new();
            data.append('Out of gas');
            panic(data);
        },
    }

    if (a <= ONE_u128) {
        return 0_u128;
    }

    return 1_u128 + _msb(a / 2_u128);
}

// Calclates the value of x^y and checks for overflow before returning
// TODO: swap to signed int when available
fn _pow_int(a: FixedType, b: u128, sign: bool) -> FixedType {
    match gas::withdraw_gas() {
        Option::Some(_) => {},
        Option::None(_) => {
            let mut data = ArrayTrait::new();
            data.append('Out of gas');
            panic(data);
        },
    }

    if (sign == true) {
        return Fixed::new(ONE_u128, false) / _pow_int(a, b, false);
    }

    let (div, rem) = integer::u128_safe_divmod(b, integer::u128_as_non_zero(2_u128));

    if (b == 0_u128) {
        return Fixed::new(ONE_u128, false);
    } else if (rem == 0_u128) {
        return _pow_int(a * a, div, false);
    } else {
        return a * _pow_int(a * a, div, false);
    }
}

// Ignores sign and always returns false
fn _split_unsigned(a: FixedType) -> (u128, u128) {
    return integer::u128_safe_divmod(a.mag, integer::u128_as_non_zero(ONE_u128));
}
