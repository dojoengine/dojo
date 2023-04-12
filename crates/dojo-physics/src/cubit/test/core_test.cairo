// use option::OptionTrait;
// use traits::Into;

// use dojo_physics::cubit::core::ONE;
// use dojo_physics::cubit::core::ONE_u128;
// use dojo_physics::cubit::core::HALF;
// use dojo_physics::cubit::core::_felt252_abs;
// use dojo_physics::cubit::core::_felt252_sign;
// use dojo_physics::cubit::core::Fixed;
// use dojo_physics::cubit::core::FixedInto;
// use dojo_physics::cubit::core::FixedPartialEq;
// use dojo_physics::cubit::core::FixedPartialOrd;
// use dojo_physics::cubit::core::FixedAdd;
// use dojo_physics::cubit::core::FixedAddEq;
// use dojo_physics::cubit::core::FixedSub;
// use dojo_physics::cubit::core::FixedSubEq;
// use dojo_physics::cubit::core::FixedMul;
// use dojo_physics::cubit::core::FixedMulEq;
// use dojo_physics::cubit::core::FixedDiv;

// use dojo_physics::cubit::trig::HALF_PI_u128;
// use dojo_physics::cubit::trig::PI_u128;

// #[test]
// fn test_into() {
//     let a = Fixed::from_unscaled_felt252(5);
//     assert(a.into() == 5 * ONE, 'invalid result');
// }

// #[test]
// #[should_panic]
// fn test_overflow_large() {
//     let too_large = 0x100000000000000000000000000000000;
//     Fixed::from_felt252(too_large);
// }

// #[test]
// #[should_panic]
// fn test_overflow_small() {
//     let too_small = -0x100000000000000000000000000000000;
//     Fixed::from_felt252(too_small);
// }

// #[test]
// fn test_sign() {
//     let min = -1809251394333065606848661391547535052811553607665798349986546028067936010240;
//     let max = 1809251394333065606848661391547535052811553607665798349986546028067936010240;
//     assert(_felt252_sign(min) == true, 'invalid result');
//     assert(_felt252_sign(-1) == true, 'invalid result');
//     assert(_felt252_sign(0) == false, 'invalid result');
//     assert(_felt252_sign(1) == false, 'invalid result');
//     assert(_felt252_sign(max) == false, 'invalid result');
// }

// #[test]
// fn test_abs() {
//     assert(_felt252_abs(5) == 5, 'abs of pos should be pos');
//     assert(_felt252_abs(-5) == 5, 'abs of neg should be pos');
//     assert(_felt252_abs(0) == 0, 'abs of 0 should be 0');
// }

// #[test]
// #[available_gas(10000000)]
// fn test_acos() {
//     let a = Fixed::new(ONE_u128, false);
//     assert(a.acos().into() == 0, 'invalid one');
// }

// #[test]
// #[available_gas(10000000)]
// fn test_asin() {
//     let a = Fixed::new(ONE_u128, false);
//     assert(a.asin().into() == 28976077832308491370, 'invalid one'); // PI / 2
// }

// #[test]
// #[available_gas(10000000)]
// fn test_atan() {
//     let a = Fixed::new(2_u128 * ONE_u128, false);
//     assert(a.atan().into() == 20423289054736244917, 'invalid two');
// }

// #[test]
// fn test_ceil() {
//     let a = Fixed::from_felt252(53495557813757699680); // 2.9
//     assert(a.ceil().into() == 3 * ONE, 'invalid pos decimal');
// }

// #[test]
// fn test_floor() {
//     let a = Fixed::from_felt252(53495557813757699680); // 2.9
//     assert(a.floor().into() == 2 * ONE, 'invalid pos decimal');
// }

// #[test]
// fn test_round() {
//     let a = Fixed::from_felt252(53495557813757699680); // 2.9
//     assert(a.round().into() == 3 * ONE, 'invalid pos decimal');
// }

// #[test]
// #[should_panic]
// fn test_sqrt_fail() {
//     let a = Fixed::from_unscaled_felt252(-25);
//     a.sqrt();
// }

// #[test]
// fn test_sqrt() {
//     let a = Fixed::from_unscaled_felt252(0);
//     assert(a.sqrt().into() == 0, 'invalid zero root');
// }

// #[test]
// #[available_gas(10000000)]
// fn test_pow() {
//     let a = Fixed::from_unscaled_felt252(3);
//     let b = Fixed::from_unscaled_felt252(4);
//     assert(a.pow(b).into() == 81 * ONE, 'invalid pos base power');
// }

// #[test]
// #[available_gas(10000000)]
// fn test_exp() {
//     let a = Fixed::from_unscaled_felt252(2);
//     assert(a.exp().into() == 136304030830375888892, 'invalid exp of 2'); // 7.389056317241236
// }

// #[test]
// #[available_gas(10000000)]
// fn test_exp2() {
//     let a = Fixed::from_unscaled_felt252(2);
//     assert(a.exp2().into() == 73786968408486180064, 'invalid exp2 of 2'); // 3.99999957248 = 4
// }

// #[test]
// #[available_gas(10000000)]
// fn test_ln() {
//     let a = Fixed::from_unscaled_felt252(1);
//     assert(a.ln().into() == 0, 'invalid ln of 1');
// }

// #[test]
// #[available_gas(10000000)]
// fn test_log2() {
//     let a = Fixed::from_unscaled_felt252(32);
//     assert(a.log2().into() == 92233719587853510925, 'invalid log2'); // 4.99999995767848
// }

// #[test]
// #[available_gas(10000000)]
// fn test_log10() {
//     let a = Fixed::from_unscaled_felt252(100);
//     assert(a.log10().into() == 36893487914963460128, 'invalid log10'); // 1.9999999873985543
// }

// #[test]
// fn test_eq() {
//     let a = Fixed::from_unscaled_felt252(42);
//     let b = Fixed::from_unscaled_felt252(42);
//     let c = a == b;
//     assert(c == true, 'invalid result');
// }

// #[test]
// fn test_ne() {
//     let a = Fixed::from_unscaled_felt252(42);
//     let b = Fixed::from_unscaled_felt252(42);
//     let c = a != b;
//     assert(c == false, 'invalid result');
// }

// #[test]
// fn test_add() {
//     let a = Fixed::from_unscaled_felt252(1);
//     let b = Fixed::from_unscaled_felt252(2);
//     assert(a + b == Fixed::from_unscaled_felt252(3), 'invalid result');
// }

// #[test]
// fn test_add_eq() {
//     let mut a = Fixed::from_unscaled_felt252(1);
//     let b = Fixed::from_unscaled_felt252(2);
//     a += b;
//     assert(a.into() == 3 * ONE, 'invalid result');
// }

// #[test]
// fn test_sub() {
//     let a = Fixed::from_unscaled_felt252(5);
//     let b = Fixed::from_unscaled_felt252(2);
//     let c = a - b;
//     assert(c.into() == 3 * ONE, 'false result invalid');
// }

// #[test]
// fn test_sub_eq() {
//     let mut a = Fixed::from_unscaled_felt252(5);
//     let b = Fixed::from_unscaled_felt252(2);
//     a -= b;
//     assert(a.into() == 3 * ONE, 'invalid result');
// }

// #[test]
// fn test_mul_pos() {
//     let a = Fixed::from_unscaled_felt252(5);
//     let b = Fixed::from_unscaled_felt252(2);
//     let c = a * b;
//     assert(c.into() == 10 * ONE, 'invalid result');
// }

// #[test]
// fn test_mul_neg() {
//     let a = Fixed::from_unscaled_felt252(5);
//     let b = Fixed::from_unscaled_felt252(-2);
//     let c = a * b;
//     assert(c.into() == -10 * ONE, 'true result invalid');
// }

// #[test]
// fn test_mul_eq() {
//     let mut a = Fixed::from_unscaled_felt252(5);
//     let b = Fixed::from_unscaled_felt252(-2);
//     a *= b;
//     assert(a.into() == -10 * ONE, 'invalid result');
// }

// #[test]
// fn test_div() {
//     let a = Fixed::from_unscaled_felt252(10);
//     let b = Fixed::from_felt252(53495557813757699680); // 2.9
//     let c = a / b;
//     assert(c.into() == 63609462323136384890, 'invalid pos decimal'); // 3.4482758620689653
// }

// #[test]
// fn test_le() {
//     let a = Fixed::from_unscaled_felt252(1);
//     let b = Fixed::from_unscaled_felt252(0);
//     let c = Fixed::from_unscaled_felt252(-1);

//     assert(a <= a, 'a <= a');
//     assert(a <= b == false, 'a <= b');
//     assert(a <= c == false, 'a <= c');

//     assert(b <= a, 'b <= a');
//     assert(b <= b, 'b <= b');
//     assert(b <= c == false, 'b <= c');

//     assert(c <= a, 'c <= a');
//     assert(c <= b, 'c <= b');
//     assert(c <= c, 'c <= c');
// }

// #[test]
// fn test_lt() {
//     let a = Fixed::from_unscaled_felt252(1);
//     let b = Fixed::from_unscaled_felt252(0);
//     let c = Fixed::from_unscaled_felt252(-1);

//     assert(a < a == false, 'a < a');
//     assert(a < b == false, 'a < b');
//     assert(a < c == false, 'a < c');

//     assert(b < a, 'b < a');
//     assert(b < b == false, 'b < b');
//     assert(b < c == false, 'b < c');

//     assert(c < a, 'c < a');
//     assert(c < b, 'c < b');
//     assert(c < c == false, 'c < c');
// }

// #[test]
// fn test_ge() {
//     let a = Fixed::from_unscaled_felt252(1);
//     let b = Fixed::from_unscaled_felt252(0);
//     let c = Fixed::from_unscaled_felt252(-1);

//     assert(a >= a, 'a >= a');
//     assert(a >= b, 'a >= b');
//     assert(a >= c, 'a >= c');

//     assert(b >= a == false, 'b >= a');
//     assert(b >= b, 'b >= b');
//     assert(b >= c, 'b >= c');

//     assert(c >= a == false, 'c >= a');
//     assert(c >= b == false, 'c >= b');
//     assert(c >= c, 'c >= c');
// }

// #[test]
// fn test_gt() {
//     let a = Fixed::from_unscaled_felt252(1);
//     let b = Fixed::from_unscaled_felt252(0);
//     let c = Fixed::from_unscaled_felt252(-1);

//     assert(a > a == false, 'a > a');
//     assert(a > b, 'a > b');
//     assert(a > c, 'a > c');

//     assert(b > a == false, 'b > a');
//     assert(b > b == false, 'b > b');
//     assert(b > c, 'b > c');

//     assert(c > a == false, 'c > a');
//     assert(c > b == false, 'c > b');
//     assert(c > c == false, 'c > c');
// }

// #[test]
// #[available_gas(10000000)]
// fn test_cos() {
//     let a = Fixed::new(HALF_PI_u128, false);
//     assert(a.cos().into() == 0, 'invalid half pi');
// }

// #[test]
// #[available_gas(10000000)]
// fn test_sin() {
//     let a = Fixed::new(HALF_PI_u128, false);
//     assert(a.sin().into() == 18446744073598439112, 'invalid half pi'); // 0.9999999999939766
// }

// #[test]
// #[available_gas(10000000)]
// fn test_tan() {
//     let a = Fixed::new(HALF_PI_u128 / 2_u128, false);
//     assert(a.tan().into() == ONE, 'invalid quarter pi');
// }


