use array::ArrayTrait;
use gas::withdraw_gas;
use option::OptionTrait;
use traits::Into;

use dojo_physics::cubit::core::ONE_u128;
use dojo_physics::cubit::core::Fixed;
use dojo_physics::cubit::core::FixedType;
use dojo_physics::cubit::core::FixedImpl;
use dojo_physics::cubit::core::FixedInto;
use dojo_physics::cubit::core::FixedAdd;
use dojo_physics::cubit::core::FixedSub;
use dojo_physics::cubit::core::FixedMul;
use dojo_physics::cubit::core::FixedDiv;


// CONSTANTS

const PI_u128: u128 = 57952155664616982739_u128;
const HALF_PI_u128: u128 = 28976077832308491370_u128;

// PUBLIC

// Calculates arccos(a) for -1 <= a <= 1 (fixed point)
// arccos(a) = arcsin(sqrt(1 - a^2)) - arctan identity has discontinuity at zero
fn acos(a: FixedType) -> FixedType {
    assert(a.mag <= ONE_u128, 'out of range');
    let asin_arg = (Fixed::new(ONE_u128, false) - a * a).sqrt();
    let asin_res = asin(asin_arg);

    if (a.sign) {
        return Fixed::new(PI_u128, false) - asin_res;
    } else {
        return asin_res;
    }
}

// Calculates arcsin(a) for -1 <= a <= 1 (fixed point)
// arcsin(a) = arctan(a / sqrt(1 - a^2))
fn asin(a: FixedType) -> FixedType {
    assert(a.mag <= ONE_u128, 'out of range');

    if (a.mag == ONE_u128) {
        return Fixed::new(HALF_PI_u128, a.sign);
    }

    let div = (Fixed::new(ONE_u128, false) - a * a).sqrt();
    return atan(a / div);
}

// Calculates arctan(x) (fixed point)
// See https://stackoverflow.com/a/50894477 for range adjustments
fn atan(a: FixedType) -> FixedType {
    let mut at = a.abs();
    let mut shift = false;
    let mut invert = false;

    // Invert value when a > 1
    if (at.mag > ONE_u128) {
        at = Fixed::new(ONE_u128, false) / at;
        invert = true;
    }

    // Account for lack of precision in polynomaial when x > 0.7
    if (at.mag > 12912720851596686131_u128) {
        let sqrt3_3 = Fixed::new(10650232656328343401_u128, false); // sqrt(3) / 3
        at = (at - sqrt3_3) / (Fixed::new(ONE_u128, false) + at * sqrt3_3);
        shift = true;
    }

    let t10 = Fixed::new(33784601907694228_u128, true);
    let t9 = Fixed::new(863077567022907619_u128, true);
    let t8 = Fixed::new(3582351446937658863_u128, false);
    let t7 = Fixed::new(4833057334070945981_u128, true);
    let t6 = Fixed::new(806366139934153963_u128, false);
    let t5 = Fixed::new(3505955710573417812_u128, false);
    let t4 = Fixed::new(25330242983263508_u128, false);
    let t3 = Fixed::new(6150896368532115927_u128, true);
    let t2 = Fixed::new(75835542453775_u128, false);
    let t1 = Fixed::new(18446743057812048409_u128, false);

    let r10 = t10 * at;
    let r9 = (r10 + t9) * at;
    let r8 = (r9 + t8) * at;
    let r7 = (r8 + t7) * at;
    let r6 = (r7 + t6) * at;
    let r5 = (r6 + t5) * at;
    let r4 = (r5 + t4) * at;
    let r3 = (r4 + t3) * at;
    let r2 = (r3 + t2) * at;
    let mut res = (r2 + t1) * at;

    // Adjust for sign change, inversion, and shift
    if (shift) {
        res = res + Fixed::new(9658692610769497123_u128, false); // pi / 6
    }

    if (invert) {
        res = res - Fixed::new(HALF_PI_u128, false);
    }

    return Fixed::new(res.mag, a.sign);
}

// Calculates cos(x) with x in radians (fixed point)
fn cos(a: FixedType) -> FixedType {
    return sin(Fixed::new(HALF_PI_u128, false) - a);
}

fn sin(a: FixedType) -> FixedType {
    let a1_u128 = a.mag % (2_u128 * PI_u128);
    let whole_rem = a1_u128 / PI_u128;
    let a2 = FixedType { mag: a1_u128 % PI_u128, sign: false };
    let mut partial_sign = false;

    if (whole_rem == 1_u128) {
        partial_sign = true;
    }

    let acc = FixedType { mag: ONE_u128, sign: false };
    let loop_res = a2 * _sin_loop(a2, 6_u128, acc);
    let res_sign = a.sign ^ partial_sign;
    return FixedType { mag: loop_res.mag, sign: res_sign };
}

// Calculates tan(x) with x in radians (fixed point)
fn tan(a: FixedType) -> FixedType {
    let sinx = sin(a);
    let cosx = cos(a);
    assert(cosx.mag != 0_u128, 'tan undefined');
    return sinx / cosx;
}

// Helper function to calculate Taylor series for sin
fn _sin_loop(a: FixedType, i: u128, acc: FixedType) -> FixedType {
    match gas::withdraw_gas() {
        Option::Some(_) => {},
        Option::None(_) => {
            let mut data = ArrayTrait::new();
            data.append('Out of gas');
            panic(data);
        },
    }

    let div_u128 = (2_u128 * i + 2_u128) * (2_u128 * i + 3_u128);
    let term = a * a * acc / Fixed::new_unscaled(div_u128, false);
    let new_acc = Fixed::new(ONE_u128, false) - term;

    if (i == 0_u128) {
        return new_acc;
    }

    return _sin_loop(a, i - 1_u128, new_acc);
}
