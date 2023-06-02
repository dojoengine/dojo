use core::option::OptionTrait;
use array::ArrayTrait;
use debug::PrintTrait;
use array::ArrayTCloneImpl;
use array::SpanTrait;
use clone::Clone;
use traits::PartialOrd;

use cubit::test::helpers::assert_precise;
use cubit::types::fixed::{Fixed, FixedPartialOrd, FixedTrait, ONE_u128};
use cubit::math::trig;

use dojo_physics::vec2::{Vec2, Vec2Trait};

fn main() -> (usize, Array::<Fixed>, Array::<Fixed>) {
    // to be inputs for #[view] function
    // v_0_mag_felt: felt252, theta_0_deg_felt: felt252, x_0_felt: felt252, y_0_felt: felt252

    //
    // Projectile parameters 
    // 
    /// Inputs: to be contract inputs for view function `main`
    /// Launch velocity magnitude, 0 <= v_0_felt <= 100
    let v_0_mag_felt = 100;
    /// Launch angle in degrees, -180 <= theta_0_deg_felt <= 180
    let theta_0_deg_felt = 65;
    /// Initial horizontal position, x_min <= x_0_felt <= x_max
    let x_0_felt = 0;
    /// Initial vertical position, y_min <= y_0_felt <= y_max
    let y_0_felt = 0;
    /// Convert inputs to signed fixed-point
    let v_0_mag = FixedTrait::from_unscaled_felt(v_0_mag_felt);
    let theta_0_deg = FixedTrait::from_unscaled_felt(theta_0_deg_felt);
    let x_0 = FixedTrait::from_unscaled_felt(x_0_felt);
    let y_0 = FixedTrait::from_unscaled_felt(y_0_felt);

    /// Convert theta_0_deg to radians
    let theta_0 = deg_to_rad(theta_0_deg);

    // Gravitational acceleration magnitude
    let g = FixedTrait::new(98 * ONE_u128 / 10, false); // 9.8
    // Plot parameters
    let x_max = FixedTrait::from_unscaled_felt(1000);
    let x_min = FixedTrait::from_unscaled_felt(-1000);
    let y_max = FixedTrait::from_unscaled_felt(500);
    let y_min = FixedTrait::from_unscaled_felt(-500);
    // Check that inputs are within required ranges
    assert(v_0_mag.mag <= 100 * ONE_u128, 'need v_0_mag_felt <= 100');
    assert(v_0_mag.mag > 0 * ONE_u128, 'need v_0_mag_felt > 0');
    assert(v_0_mag.sign == false, 'need v_0_mag_felt > 0');
    // `theta_0_deg.mag` not exact after conversion, so use 180.0000001 
    assert(theta_0_deg.mag <= 180000001 * ONE_u128 / 1000000, '-180 <= theta_0_deg_felt <= 180');
    assert(FixedPartialOrd::le(x_0, x_max), 'need x_0 <= x_max');
    assert(FixedPartialOrd::ge(x_0, x_min), 'need x_0 >= x_min');
    assert(FixedPartialOrd::le(y_0, y_max), 'need y_0 <= y_max');
    assert(FixedPartialOrd::ge(y_0, y_min), 'need y_0 >= y_min');
    // Initial position vector
    let r_0 = Vec2Trait::<Fixed>::new(x_0, y_0);
    // Initial velocity vector
    let v_0 = vec2_from_mag_theta(v_0_mag, theta_0);

    // Time interval between plotted points
    let delta_t = FixedTrait::from_unscaled_felt(2); // arbitrary value 2 chosen

    // Tuples to pass to functions
    let plot_params = (x_max, x_min, y_max, y_min);
    let motion_params = (r_0, v_0, g, delta_t);
    let (mut x_s, mut y_s) = fill_position_s(plot_params, motion_params);
    (x_s.len(), x_s, y_s)
}

fn deg_to_rad(theta_deg: Fixed) -> Fixed {
    let pi = FixedTrait::new(trig::PI_u128, false);
    let one_eighty = FixedTrait::new(180 * ONE_u128, false);
    theta_deg * pi / one_eighty
}

// Creates Fixed type Vec2 from magnitude, theta in radians
fn vec2_from_mag_theta(mag: Fixed, theta: Fixed) -> Vec2<Fixed> {
    let x_comp = mag * trig::cos(theta); // trig::cos works only for Fixed type
    let y_comp = mag * trig::sin(theta); // trig::sin works only for Fixed type
    Vec2::<Fixed> { x: x_comp, y: y_comp }
}

fn fill_position_s(
    plot_params: (Fixed, Fixed, Fixed, Fixed),
    motion_params: (Vec2<Fixed>, Vec2<Fixed>, Fixed, Fixed)
) -> (Array::<Fixed>, Array::<Fixed>) {
    let (x_max, x_min, _y_max, y_min) = plot_params;
    let (r_0, v_0, g, delta_t) = motion_params;
    let mut x_s = ArrayTrait::<Fixed>::new();
    let mut y_s = ArrayTrait::<Fixed>::new();

    let one = FixedTrait::new(ONE_u128, false);
    let mut n = FixedTrait::new(0, false);

    loop {
        // match withdraw_gas() {
        //     Option::Some(_) => {},
        //     Option::None(_) => {
        //         let mut data = ArrayTrait::new();
        //         data.append('Out of gas');
        //         panic(data);
        //     },
        // }
        let t = n * delta_t;
        // 'n'.print();
        // n.mag.print();
        let x = calc_x(r_0.x, v_0.x, t);
        // 'x'.print();
        // x.mag.print();
        // x.sign.print();
        let y = calc_y(r_0.y, v_0.y, g, t);
        // 'y'.print();
        // y.mag.print();
        // y.sign.print();
        if x >= x_max | x <= x_min | y <= y_min {
            break ();
        }

        x_s.append(x);
        y_s.append(y);

        n += one;
    };

    (x_s, y_s)
}

fn calc_x(x_0: Fixed, v_0x: Fixed, t: Fixed) -> Fixed {
    x_0 + v_0x * t
}

fn calc_y(y_0: Fixed, v_0y: Fixed, g: Fixed, t: Fixed) -> Fixed {
    let half = FixedTrait::new(5 * ONE_u128 / 10, false);
    y_0 + v_0y * t - half * g * t * t
}

#[test]
#[available_gas(2000000)]
fn test_deg_to_rad() {
    let sixty = FixedTrait::new(60 * ONE_u128, false);
    let theta = deg_to_rad(sixty);
    assert_precise(theta, 19317385221538994246, 'invalid PI/3', Option::None(()));
    assert(theta.sign == false, 'invalid sign');

    let minus_120 = FixedTrait::new(120 * ONE_u128, true);
    let theta = deg_to_rad(minus_120);
    assert_precise(theta, -38634770443077988493, 'invalid -2*PI/3', Option::None(()));
    assert(theta.sign == true, 'invalid sign');
}

#[test]
#[available_gas(20000000)]
fn test_vec2_from_mag_theta() {
    let mag = FixedTrait::new(100 * ONE_u128, false);
    let sixty = FixedTrait::new(60 * ONE_u128, false);
    let theta = deg_to_rad(sixty);
    let vec2 = vec2_from_mag_theta(mag, theta);
    assert_precise(vec2.x, 922337203685477580800, 'invalid vec2.x mag', Option::None(())); // 50
    assert(vec2.x.sign == false, 'invalid vec2.x.sign');
    assert_precise(vec2.y, 1597534898494251510150, 'invalid vec2.y mag', Option::None(())); // 86.6
    assert(vec2.y.sign == false, 'invalid vec2.y.sig');

    let minus_120 = FixedTrait::new(120 * ONE_u128, true);
    let theta = deg_to_rad(minus_120);
    let vec2 = vec2_from_mag_theta(mag, theta);
    assert_precise(vec2.x, -922337203685477580800, 'invalid vec2.x mag', Option::None(())); // -50
    assert(vec2.x.sign == true, 'invalid vec2.x.sign');
    assert_precise(
        vec2.y, -1597534898494251510150, 'invalid vec2.y mag', Option::None(())
    ); // -86.6
    assert(vec2.y.sign == true, 'invalid vec2.y.sig');
}

#[test]
#[available_gas(20000000)]
fn test_fill_position_s() {
    let v_0_mag = FixedTrait::from_unscaled_felt(100);
    let theta_0_deg = FixedTrait::from_unscaled_felt(65);
    let theta_0 = deg_to_rad(theta_0_deg);
    let x_0 = FixedTrait::from_unscaled_felt(0);
    let y_0 = FixedTrait::from_unscaled_felt(0);

    let x_max = FixedTrait::from_unscaled_felt(1000);
    let x_min = FixedTrait::from_unscaled_felt(-1000);
    let y_max = FixedTrait::from_unscaled_felt(500);
    let y_min = FixedTrait::from_unscaled_felt(-500);

    let r_0 = Vec2Trait::<Fixed>::new(x_0, y_0);
    let v_0 = vec2_from_mag_theta(v_0_mag, theta_0);
    let g = FixedTrait::new(98 * ONE_u128 / 10, false);
    let delta_t = FixedTrait::from_unscaled_felt(2);

    let plot_params = (x_max, x_min, y_max, y_min);
    let motion_params = (r_0, v_0, g, delta_t);

    let mut position_s: (Array<Fixed>, Array<Fixed>) = fill_position_s(plot_params, motion_params);

    let (x_s, y_s) = position_s;
    let length = x_s.len();
    assert(length == 12, 'invalid length');

    assert_precise(
        *x_s[5], 7795930915206679528264, 'invalid x_s[5]', Option::None(())
    ); // 422.61826174069944
    assert(*x_s.at(5).sign == false, 'invalid sign');
    assert_precise(
        *y_s[5], 7679523203357457794972, 'invalid y_s[5]', Option::None(())
    ); // 416.3077870366498
    assert(*y_s.at(5).sign == false, 'invalid sign');

    assert_precise(
        *x_s[10], 15591861830413359425462, 'invalid x_s[10]', Option::None(())
    ); // 845.2365234813989, custom precision 1e-6
    assert(*x_s.at(10).sign == false, 'invalid sign');
    assert_precise(
        *y_s[10], -2718762785520446838411, 'invalid y_s[10]', Option::None(())
    ); // -147.3844259267005, custom precision 1e-6
    assert(*y_s.at(10).sign == true, 'invalid sign');
}

#[test]
#[available_gas(2000000)]
fn test_calc_x() {
    let x_0 = FixedTrait::new(100 * ONE_u128, false);
    let v_0x = FixedTrait::new(50 * ONE_u128, false);
    let t = FixedTrait::new(16 * ONE_u128, false);
    let x = calc_x(x_0, v_0x, t);
    assert(x.mag == 900 * ONE_u128, 'invalid mag');
    assert(x.sign == false, 'invalid sign');
}

#[test]
#[available_gas(2000000)]
fn test_calc_y() {
    let y_0 = FixedTrait::new(100 * ONE_u128, false);
    let v_0y = FixedTrait::new(50 * ONE_u128, false);
    let t = FixedTrait::new(16 * ONE_u128, false);
    let g = FixedTrait::new(98 * ONE_u128 / 10, false);

    let y = calc_y(y_0, v_0y, g, t);
    assert_precise(y, -6537526099722665092710, 'invalid y', Option::None(())); // -354.4
    assert(y.sign == true, 'invalid sign');
}
