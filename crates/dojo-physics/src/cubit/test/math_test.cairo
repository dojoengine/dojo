// use option::OptionTrait;
// use traits::Into;

// use dojo_physics::cubit::core::ONE;
// use dojo_physics::cubit::core::HALF;
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

// use dojo_physics::cubit::math;

// #[test]
// fn test_ceil() {
//     let a = Fixed::from_felt252(53495557813757699680); // 2.9
//     assert(math::ceil(a).into() == 3 * ONE, 'invalid pos decimal');

//     let a = Fixed::from_felt252(-53495557813757699680); // -2.9
//     assert(math::ceil(a).into() == -2 * ONE, 'invalid neg decimal');

//     let a = Fixed::from_unscaled_felt252(4);
//     assert(math::ceil(a).into() == 4 * ONE, 'invalid pos integer');

//     let a = Fixed::from_unscaled_felt252(-4);
//     assert(math::ceil(a).into() == -4 * ONE, 'invalid neg integer');

//     let a = Fixed::from_unscaled_felt252(0);
//     assert(math::ceil(a).into() == 0, 'invalid zero');

//     let a = Fixed::from_felt252(HALF);
//     assert(math::ceil(a).into() == 1 * ONE, 'invalid pos half');

//     let a = Fixed::from_felt252(-1 * HALF);
//     assert(math::ceil(a).into() == 0, 'invalid neg half');
// }

// #[test]
// fn test_floor() {
//     let a = Fixed::from_felt252(53495557813757699680); // 2.9
//     assert(math::floor(a).into() == 2 * ONE, 'invalid pos decimal');

//     let a = Fixed::from_felt252(-53495557813757699680); // -2.9
//     assert(math::floor(a).into() == -3 * ONE, 'invalid neg decimal');

//     let a = Fixed::from_unscaled_felt252(4);
//     assert(math::floor(a).into() == 4 * ONE, 'invalid pos integer');

//     let a = Fixed::from_unscaled_felt252(-4);
//     assert(math::floor(a).into() == -4 * ONE, 'invalid neg integer');

//     let a = Fixed::from_unscaled_felt252(0);
//     assert(math::floor(a).into() == 0, 'invalid zero');

//     let a = Fixed::from_felt252(HALF);
//     assert(math::floor(a).into() == 0, 'invalid pos half');

//     let a = Fixed::from_felt252(-1 * HALF);
//     assert(math::floor(a).into() == -1 * ONE, 'invalid neg half');
// }

// #[test]
// fn test_round() {
//     let a = Fixed::from_felt252(53495557813757699680); // 2.9
//     assert(math::round(a).into() == 3 * ONE, 'invalid pos decimal');

//     let a = Fixed::from_felt252(-53495557813757699680); // -2.9
//     assert(math::round(a).into() == -3 * ONE, 'invalid neg decimal');

//     let a = Fixed::from_unscaled_felt252(4);
//     assert(math::round(a).into() == 4 * ONE, 'invalid pos integer');

//     let a = Fixed::from_unscaled_felt252(-4);
//     assert(math::round(a).into() == -4 * ONE, 'invalid neg integer');

//     let a = Fixed::from_unscaled_felt252(0);
//     assert(math::round(a).into() == 0, 'invalid zero');

//     let a = Fixed::from_felt252(HALF);
//     assert(math::round(a).into() == 1 * ONE, 'invalid pos half');

//     let a = Fixed::from_felt252(-1 * HALF);
//     assert(math::round(a).into() == -1 * ONE, 'invalid neg half');
// }

// #[test]
// #[should_panic]
// fn test_sqrt_fail() {
//     let a = Fixed::from_unscaled_felt252(-25);
//     math::sqrt(a);
// }

// #[test]
// fn test_sqrt() {
//     let a = Fixed::from_unscaled_felt252(0);
//     assert(math::sqrt(a).into() == 0, 'invalid zero root');

//     let a = Fixed::from_unscaled_felt252(1);
//     assert(math::sqrt(a).into() == ONE, 'invalid one root');

//     let a = Fixed::from_unscaled_felt252(25);
//     assert(math::sqrt(a).into() == 92233720368547758080, 'invalid 25 root'); // 5

//     let a = Fixed::from_unscaled_felt252(81);
//     assert(math::sqrt(a).into() == 166020696663385964544, 'invalid 81 root'); // 9

//     let a = Fixed::from_felt252(1152921504606846976); // 0.0625
//     assert(math::sqrt(a).into() == 4611686018427387904, 'invalid decimal root'); // 0.25
// }

// #[test]
// #[available_gas(10000000)]
// fn test_pow_int() {
//     let a = Fixed::from_unscaled_felt252(3);
//     let b = Fixed::from_unscaled_felt252(4);
//     assert(math::pow(a, b).into() == 81 * ONE, 'invalid pos base power');

//     let a = Fixed::from_unscaled_felt252(50);
//     let b = Fixed::from_unscaled_felt252(5);
//     assert(math::pow(a, b).into() == 312500000 * ONE, 'invalid big power');

//     let a = Fixed::from_unscaled_felt252(-3);
//     let b = Fixed::from_unscaled_felt252(2);
//     assert(math::pow(a, b).into() == 9 * ONE, 'invalid neg base');

//     let a = Fixed::from_unscaled_felt252(3);
//     let b = Fixed::from_unscaled_felt252(-2);
//     assert(
//         math::pow(a, b).into() == 2049638230412172401, 'invalid neg power'
//     ); // 0.1111111111111111

//     let a = Fixed::from_unscaled_felt252(-3);
//     let b = Fixed::from_unscaled_felt252(-2);
//     assert(math::pow(a, b).into() == 2049638230412172401, 'invalid neg base power');

//     let a = Fixed::from_felt252(9223372036854775808);
//     let b = Fixed::from_unscaled_felt252(2);
//     assert(math::pow(a, b).into() == 4611686018427387904, 'invalid frac base power');
// }

// #[test]
// #[available_gas(10000000)]
// fn test_pow_frac() {
//     let a = Fixed::from_unscaled_felt252(3);
//     let b = Fixed::from_felt252(9223372036854775808);
//     assert(
//         math::pow(a, b).into() == 31950696165187714181, 'invalid pos base power'
//     ); // 1.7320507097360398

//     let a = Fixed::from_felt252(2277250555899444146995); // 123.45
//     let b = Fixed::from_felt252(-27670116110564327424); // -1.5
//     assert(
//         math::pow(a, b).into() == 13448785356302935, 'invalid pos base power'
//     ); // 0.0007290601150297441
// }

// #[test]
// #[available_gas(10000000)]
// fn test_exp() {
//     let a = Fixed::from_unscaled_felt252(2);
//     assert(math::exp(a).into() == 136304030830375888892, 'invalid exp of 2'); // 7.389056317241236

//     let a = Fixed::from_unscaled_felt252(0);
//     assert(math::exp(a).into() == ONE, 'invalid exp of 0');

//     let a = Fixed::from_unscaled_felt252(-2);
//     assert(math::exp(a).into() == 2496495260249524483, 'invalid exp of -2'); // 0.13533527923811497
// }

// #[test]
// #[available_gas(10000000)]
// fn test_exp2() {
//     let a = Fixed::from_unscaled_felt252(2);
//     assert(math::exp2(a).into() == 73786968408486180064, 'invalid exp2 of 2'); // 3.99999957248 = 4

//     let a = Fixed::from_unscaled_felt252(0);
//     assert(math::exp2(a).into() == ONE, 'invalid exp2 of 0');

//     let a = Fixed::from_unscaled_felt252(-2);
//     assert(
//         math::exp2(a).into() == 4611686511324442234, 'invalid exp of -2'
//     ); // 0.2500000267200029 = 0.25
// }

// #[test]
// #[available_gas(10000000)]
// fn test_ln() {
//     let a = Fixed::from_unscaled_felt252(1);
//     assert(math::ln(a).into() == 0, 'invalid ln of 1');

//     let a = Fixed::from_felt252(50143449209799256683); // e
//     assert(math::ln(a).into() == 18446744490532965082, 'invalid ln of e'); // 1.0000000225960426

//     let a = Fixed::from_felt252(9223372036854775808); // 0.5
//     assert(math::ln(a).into() == -12786308104066639394, 'invalid ln of 0.5'); // -0.6931471512249031
// }

// #[test]
// #[available_gas(10000000)]
// fn test_log2() {
//     let a = Fixed::from_unscaled_felt252(32);
//     assert(math::log2(a).into() == 92233719587853510925, 'invalid log2'); // 4.99999995767848

//     let a = Fixed::from_unscaled_felt252(1234);
//     assert(math::log2(a).into() == 189431951110156820629, 'invalid log2'); // 10.269126646589994

//     let a = Fixed::from_felt252(1035286617648801165344); // 56.123
//     assert(math::log2(a).into() == 107185180242499619003, 'invalid log2'); // 5.810520263858423
// }

// #[test]
// #[available_gas(10000000)]
// fn test_log10() {
//     let a = Fixed::from_unscaled_felt252(100);
//     assert(math::log10(a).into() == 36893487914963460128, 'invalid log10'); // 1.9999999873985543

//     let a = Fixed::from_unscaled_felt252(1);
//     assert(math::log10(a).into() == 0, 'invalid log10');
// }

// #[test]
// fn test_eq() {
//     let a = Fixed::from_unscaled_felt252(42);
//     let b = Fixed::from_unscaled_felt252(42);
//     let c = math::eq(a, b);
//     assert(c == true, 'invalid result');

//     let a = Fixed::from_unscaled_felt252(42);
//     let b = Fixed::from_unscaled_felt252(-42);
//     let c = math::eq(a, b);
//     assert(c == false, 'invalid result');
// }

// #[test]
// fn test_ne() {
//     let a = Fixed::from_unscaled_felt252(42);
//     let b = Fixed::from_unscaled_felt252(42);
//     let c = math::ne(a, b);
//     assert(c == false, 'invalid result');

//     let a = Fixed::from_unscaled_felt252(42);
//     let b = Fixed::from_unscaled_felt252(-42);
//     let c = math::ne(a, b);
//     assert(c == true, 'invalid result');
// }

// #[test]
// fn test_add() {
//     let a = Fixed::from_unscaled_felt252(1);
//     let b = Fixed::from_unscaled_felt252(2);
//     assert(math::add(a, b) == Fixed::from_unscaled_felt252(3), 'invalid result');
// }

// #[test]
// fn test_sub() {
//     let a = Fixed::from_unscaled_felt252(5);
//     let b = Fixed::from_unscaled_felt252(2);
//     let c = math::sub(a, b);
//     assert(c.into() == 3 * ONE, 'false result invalid');

//     let c = math::sub(b, a);
//     assert(c.into() == -3 * ONE, 'true result invalid');
// }

// #[test]
// fn test_mul_pos() {
//     let a = Fixed::from_unscaled_felt252(5);
//     let b = Fixed::from_unscaled_felt252(2);
//     let c = math::mul(a, b);
//     assert(c.into() == 10 * ONE, 'invalid result');

//     let a = Fixed::from_unscaled_felt252(9);
//     let b = Fixed::from_unscaled_felt252(9);
//     let c = math::mul(a, b);
//     assert(c.into() == 81 * ONE, 'invalid result');

//     let a = Fixed::from_unscaled_felt252(4294967295);
//     let b = Fixed::from_unscaled_felt252(4294967295);
//     let c = math::mul(a, b);
//     assert(c.into() == 18446744065119617025 * ONE, 'invalid huge mul');

//     let a = Fixed::from_felt252(23058430092136939520); // 1.25
//     let b = Fixed::from_felt252(42427511369531968716); // 2.3
//     let c = math::mul(a, b);
//     assert(c.into() == 53034389211914960895, 'invalid result'); // 2.875

//     let a = Fixed::from_unscaled_felt252(0);
//     let b = Fixed::from_felt252(42427511369531968716); // 2.3
//     let c = math::mul(a, b);
//     assert(c.into() == 0, 'invalid result');
// }

// #[test]
// fn test_mul_neg() {
//     let a = Fixed::from_unscaled_felt252(5);
//     let b = Fixed::from_unscaled_felt252(-2);
//     let c = math::mul(a, b);
//     assert(c.into() == -10 * ONE, 'true result invalid');

//     let a = Fixed::from_unscaled_felt252(-5);
//     let b = Fixed::from_unscaled_felt252(-2);
//     let c = math::mul(a, b);
//     assert(c.into() == 10 * ONE, 'false result invalid');
// }

// #[test]
// fn test_div() {
//     let a = Fixed::from_unscaled_felt252(10);
//     let b = Fixed::from_felt252(53495557813757699680); // 2.9
//     let c = math::div(a, b);
//     assert(c.into() == 63609462323136384890, 'invalid pos decimal'); // 3.4482758620689653

//     let a = Fixed::from_unscaled_felt252(10);
//     let b = Fixed::from_unscaled_felt252(5);
//     let c = math::div(a, b);
//     assert(c.into() == 36893488147419103230, 'invalid pos integer'); // 2

//     let a = Fixed::from_unscaled_felt252(-2);
//     let b = Fixed::from_unscaled_felt252(5);
//     let c = math::div(a, b);
//     assert(c.into() == -7378697629483820646, 'invalid neg decimal'); // 0.4

//     let a = Fixed::from_unscaled_felt252(-1000);
//     let b = Fixed::from_unscaled_felt252(12500);
//     let c = math::div(a, b);
//     assert(c.into() == -1475739525896764000, 'invalid neg decimal'); // 0.08

//     let a = Fixed::from_unscaled_felt252(-10);
//     let b = Fixed::from_unscaled_felt252(123456789);
//     let c = math::div(a, b);
//     assert(c.into() == -1494186283560, 'invalid neg decimal'); // 8.100000073706917e-8

//     let a = Fixed::from_unscaled_felt252(123456789);
//     let b = Fixed::from_unscaled_felt252(-10);
//     let c = math::div(a, b);
//     assert(c.into() == -227737579084496056040038029, 'invalid neg decimal'); // -12345678.9
// }

// #[test]
// fn test_le() {
//     let a = Fixed::from_unscaled_felt252(1);
//     let b = Fixed::from_unscaled_felt252(0);
//     let c = Fixed::from_unscaled_felt252(-1);

//     assert(math::le(a, a), 'a <= a');
//     assert(math::le(a, b) == false, 'a <= b');
//     assert(math::le(a, c) == false, 'a <= c');

//     assert(math::le(b, a), 'b <= a');
//     assert(math::le(b, b), 'b <= b');
//     assert(math::le(b, c) == false, 'b <= c');

//     assert(math::le(c, a), 'c <= a');
//     assert(math::le(c, b), 'c <= b');
//     assert(math::le(c, c), 'c <= c');
// }

// #[test]
// fn test_lt() {
//     let a = Fixed::from_unscaled_felt252(1);
//     let b = Fixed::from_unscaled_felt252(0);
//     let c = Fixed::from_unscaled_felt252(-1);

//     assert(math::lt(a, a) == false, 'a < a');
//     assert(math::lt(a, b) == false, 'a < b');
//     assert(math::lt(a, c) == false, 'a < c');

//     assert(math::lt(b, a), 'b < a');
//     assert(math::lt(b, b) == false, 'b < b');
//     assert(math::lt(b, c) == false, 'b < c');

//     assert(math::lt(c, a), 'c < a');
//     assert(math::lt(c, b), 'c < b');
//     assert(math::lt(c, c) == false, 'c < c');
// }

// #[test]
// fn test_ge() {
//     let a = Fixed::from_unscaled_felt252(1);
//     let b = Fixed::from_unscaled_felt252(0);
//     let c = Fixed::from_unscaled_felt252(-1);

//     assert(math::ge(a, a), 'a >= a');
//     assert(math::ge(a, b), 'a >= b');
//     assert(math::ge(a, c), 'a >= c');

//     assert(math::ge(b, a) == false, 'b >= a');
//     assert(math::ge(b, b), 'b >= b');
//     assert(math::ge(b, c), 'b >= c');

//     assert(math::ge(c, a) == false, 'c >= a');
//     assert(math::ge(c, b) == false, 'c >= b');
//     assert(math::ge(c, c), 'c >= c');
// }

// #[test]
// fn test_gt() {
//     let a = Fixed::from_unscaled_felt252(1);
//     let b = Fixed::from_unscaled_felt252(0);
//     let c = Fixed::from_unscaled_felt252(-1);

//     assert(math::gt(a, a) == false, 'a > a');
//     assert(math::gt(a, b), 'a > b');
//     assert(math::gt(a, c), 'a > c');

//     assert(math::gt(b, a) == false, 'b > a');
//     assert(math::gt(b, b) == false, 'b > b');
//     assert(math::gt(b, c), 'b > c');

//     assert(math::gt(c, a) == false, 'c > a');
//     assert(math::gt(c, b) == false, 'c > b');
//     assert(math::gt(c, c) == false, 'c > c');
// }

