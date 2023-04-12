// use option::OptionTrait;
// use traits::Into;

// use dojo_physics::cubit::core::ONE;
// use dojo_physics::cubit::core::ONE_u128;
// use dojo_physics::cubit::core::Fixed;
// use dojo_physics::cubit::core::FixedInto;
// use dojo_physics::cubit::core::FixedPartialEq;

// use dojo_physics::cubit::trig::HALF_PI_u128;
// use dojo_physics::cubit::trig::PI_u128;
// use dojo_physics::cubit::trig;

// #[test]
// #[available_gas(10000000)]
// fn test_acos() {
//     let a = Fixed::new(ONE_u128, false);
//     assert(trig::acos(a).into() == 0, 'invalid one');

//     let a = Fixed::new(ONE_u128 / 2_u128, false);
//     assert(trig::acos(a).into() == 19317385211018935530, 'invalid half'); // 1.0471975506263043

//     let a = Fixed::new(0_u128, false);
//     assert(trig::acos(a).into() == 28976077832308491370, 'invalid zero'); // PI / 2

//     let a = Fixed::new(ONE_u128 / 2_u128, true);
//     assert(trig::acos(a).into() == 38634770453598047209, 'invalid neg half'); // 2.094395102963489

//     let a = Fixed::new(ONE_u128, true);
//     assert(trig::acos(a).into() == 57952155664616982739, 'invalid neg one'); // PI
// }

// #[test]
// #[should_panic]
// #[available_gas(10000000)]
// fn test_acos_fail() {
//     let a = Fixed::new(2_u128 * ONE_u128, true);
//     trig::acos(a).into();
// }

// #[test]
// #[available_gas(10000000)]
// fn test_atan() {
//     let a = Fixed::new(2_u128 * ONE_u128, false);
//     assert(trig::atan(a).into() == 20423289054736244917, 'invalid two');

//     let a = Fixed::new(ONE_u128, false);
//     assert(trig::atan(a).into() == 14488038909386489874, 'invalid one');

//     let a = Fixed::new(ONE_u128 / 2_u128, false);
//     assert(trig::atan(a).into() == 8552788777572246454, 'invalid half');

//     let a = Fixed::new(0_u128, false);
//     assert(trig::atan(a).into() == 0, 'invalid zero');

//     let a = Fixed::new(ONE_u128 / 2_u128, true);
//     assert(trig::atan(a).into() == -8552788777572246454, 'invalid neg half');

//     let a = Fixed::new(ONE_u128, true);
//     assert(trig::atan(a).into() == -14488038909386489874, 'invalid neg one');

//     let a = Fixed::new(2_u128 * ONE_u128, true);
//     assert(trig::atan(a).into() == -20423289054736244917, 'invalid neg two');
// }

// #[test]
// #[available_gas(10000000)]
// fn test_asin() {
//     let a = Fixed::new(ONE_u128, false);
//     assert(trig::asin(a).into() == 28976077832308491370, 'invalid one'); // PI / 2

//     let a = Fixed::new(ONE_u128 / 2_u128, false);
//     assert(trig::asin(a).into() == 9658692617570005102, 'invalid half');

//     let a = Fixed::new(0_u128, false);
//     assert(trig::asin(a).into() == 0, 'invalid zero');

//     let a = Fixed::new(ONE_u128 / 2_u128, true);
//     assert(trig::asin(a).into() == -9658692617570005102, 'invalid neg half');

//     let a = Fixed::new(ONE_u128, true);
//     assert(trig::asin(a).into() == -28976077832308491370, 'invalid neg one'); // -PI / 2
// }

// #[test]
// #[should_panic]
// #[available_gas(10000000)]
// fn test_asin_fail() {
//     let a = Fixed::new(2_u128 * ONE_u128, false);
//     trig::asin(a).into();
// }

// #[test]
// #[available_gas(10000000)]
// fn test_cos() {
//     let a = Fixed::new(HALF_PI_u128, false);
//     assert(trig::cos(a).into() == 0, 'invalid half pi');

//     let a = Fixed::new(HALF_PI_u128 / 2_u128, false);
//     assert(trig::cos(a).into() == 13043817825332781360, 'invalid quarter pi'); // 0.7071067811865475

//     let a = Fixed::new(PI_u128, false);
//     assert(trig::cos(a).into() == -18446744073598439113, 'invalid pi');

//     let a = Fixed::new(HALF_PI_u128, true);
//     assert(trig::cos(a).into() == -1, 'invalid neg half pi'); // -0.000...

//     let a = Fixed::new_unscaled(17_u128, false);
//     assert(trig::cos(a).into() == -5075864723929312153, 'invalid 17'); // -0.2751631780463348

//     let a = Fixed::new_unscaled(17_u128, true);
//     assert(trig::cos(a).into() == -5075864723929312150, 'invalid -17'); // -0.2751631780463348
// }

// #[test]
// #[available_gas(10000000)]
// fn test_sin() {
//     let a = Fixed::new(HALF_PI_u128, false);
//     assert(trig::sin(a).into() == 18446744073598439112, 'invalid half pi'); // 0.9999999999939766

//     let a = Fixed::new(HALF_PI_u128 / 2_u128, false);
//     assert(trig::sin(a).into() == 13043817825332781360, 'invalid quarter pi'); // 0.7071067811865475

//     let a = Fixed::new(PI_u128, false);
//     assert(trig::sin(a).into() == 0, 'invalid pi');

//     let a = Fixed::new(HALF_PI_u128, true);
//     assert(trig::sin(a).into() == -18446744073598439112, 'invalid neg half pi'); // 0.9999999999939766

//     let a = Fixed::new_unscaled(17_u128, false);
//     assert(trig::sin(a).into() == -17734653485804420554, 'invalid 17'); // -0.9613974918793389

//     let a = Fixed::new_unscaled(17_u128, true);
//     assert(trig::sin(a).into() == 17734653485804420554, 'invalid -17'); // 0.9613974918793389
// }

// #[test]
// #[available_gas(100000000)]
// fn test_tan() {
//     let a = Fixed::new(HALF_PI_u128 / 2_u128, false);
//     assert(trig::tan(a).into() == ONE, 'invalid quarter pi');

//     let a = Fixed::new(PI_u128, false);
//     assert(trig::tan(a).into() == 0, 'invalid pi');

//     let a = Fixed::new_unscaled(17_u128, false);
//     assert(trig::tan(a).into() == 64451405205161859944, 'invalid 17'); // 3.493917677159002

//     let a = Fixed::new_unscaled(17_u128, true);
//     assert(trig::tan(a).into() == -64451405205161859982, 'invalid -17'); // -3.493917677159002
// }

