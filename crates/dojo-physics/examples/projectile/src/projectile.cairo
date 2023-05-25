use array::ArrayTrait;
use debug::PrintTrait;
// use array::ArrayTCloneImpl;
// use array::SpanTrait;
// use clone::Clone;
// use traits::PartialOrd;

use cubit::core::FixedType;
use cubit::core::Fixed;
use cubit::core::FixedPartialOrd;
use cubit::core::ONE_u128;

use cubit::trig;

use dojo_physics::vec2::Vec2;
use dojo_physics::vec2::Vec2Trait;

fn main() {
    //
    // Projectile parameters 
    // 
    /// Inputs: to be felt252 contract inputs for view function `main`
    /// Launch velocity magnitude, 0 <= v_0_felt <= 100
    let v_0_mag_felt = 100;
    /// Launch angle in degrees, -180 <= theta_0_deg_felt <= 180
    let theta_0_deg_felt = 65;
    /// Initial horizontal position, x_min <= x_0_felt <= x_max
    let x_0_felt = 0;
    /// Initial vertical position, y_min <= y_0_felt <= y_max
    let y_0_felt = 0;

    /// Convert inputs to signed fixed-point
    let v_0_mag = Fixed::from_unscaled_felt(v_0_mag_felt);
    let theta_0_deg = Fixed::from_unscaled_felt(theta_0_deg_felt);
    let x_0 = Fixed::from_unscaled_felt(x_0_felt);
    let y_0 = Fixed::from_unscaled_felt(y_0_felt);

    /// Convert theta_0_deg to radians
    let theta_0 = deg_to_rad(theta_0_deg);

    // Gravitational acceleration magnitude
    let g = Fixed::new(98 * ONE_u128 / 10, false); // 9.8

    // Plot parameters
    let x_max = Fixed::from_unscaled_felt(1000);
    let x_min = Fixed::from_unscaled_felt(-1000);
    let y_max = Fixed::from_unscaled_felt(500);
    let y_min = Fixed::from_unscaled_felt(-500);

    // Check that inputs are within required ranges
    assert(v_0_mag.mag <= 100, 'need v_0_felt <= 100');
    assert(v_0_mag.mag > 0, 'need v_0_felt > 0');
    assert(v_0_mag.sign == false, 'need v_0_felt > 0');
    assert(theta_0_deg.mag <= 180, '-180 <= theta_0_deg_felt <= 180');
    assert(FixedPartialOrd::le(x_0, x_max), 'need x_0_felt <= 1000');
    assert(FixedPartialOrd::ge(x_0, x_min), 'need x_0_felt >= -1000');
    assert(FixedPartialOrd::le(y_0, y_max), 'need y_0_felt <= 500');
    assert(FixedPartialOrd::ge(y_0, y_min), 'need y_0_felt >= -500');

    // Initial position vector
    let r_0 = Vec2Trait::<FixedType>::new(x_0, y_0);
    // Initial velocity vector
    let v_0 = vec2_from_mag_theta(v_0_mag, theta_0);

    // Time interval between plotted points
    let delta_t = Fixed::from_unscaled_felt(2); // arbitrary value 2 chosen

    // Tuples to pass to functions
    let plot_params = (x_max, x_min, y_max, y_min);
    let motion_params = (r_0, v_0, g, delta_t);

    let mut r_s = fill_position_s(plot_params, motion_params);
    let n = r_s.len();
    n.print();
// r_s.span().snapshot.clone().print();
}

fn deg_to_rad(theta_deg: FixedType) -> FixedType {
    let pi = Fixed::new(trig::PI_u128, false);
    let one_eighty = Fixed::new(180_u128 * ONE_u128, false);
    theta_deg * pi / one_eighty
}

// Creates FixedType Vec2 from magnitude, theta in radians
fn vec2_from_mag_theta(mag: FixedType, theta: FixedType) -> Vec2<FixedType> {
    let x_comp = mag * trig::cos(theta); // trig::cos works only for FixedType
    let y_comp = mag * trig::sin(theta); // trig::sin works only for FixedType
    Vec2::<FixedType> { x: x_comp, y: y_comp }
}

fn fill_position_s(
    plot_params: (FixedType, FixedType, FixedType, FixedType),
    motion_params: (Vec2<FixedType>, Vec2<FixedType>, FixedType, FixedType)
) -> Array::<Vec2<FixedType>> {
    let (x_max, x_min, _y_max, y_min) = plot_params;
    let (r_0, v_0, g, delta_t) = motion_params;
    let mut r_s = ArrayTrait::<Vec2<FixedType>>::new();

    let one = Fixed::new(ONE_u128, false);
    let mut n = Fixed::new(0_u128, false);

    loop {
        match gas::withdraw_gas() {
            Option::Some(_) => {},
            Option::None(_) => {
                let mut data = ArrayTrait::new();
                data.append('Out of gas');
                panic(data);
            },
        }
        let t = n * delta_t;
        let x = calc_x(r_0.x, v_0.x, t);
        let y = calc_y(r_0.y, v_0.y, g, t);

        if x >= x_max | x <= x_min | y <= y_min {
            break ();
        }

        let r = Vec2Trait::<FixedType>::new(x, y);
        r_s.append(r);

        n += one;
    };

    r_s
}

fn calc_x(x_0: FixedType, v_0x: FixedType, t: FixedType) -> FixedType {
    x_0 + v_0x * t
}

fn calc_y(y_0: FixedType, v_0y: FixedType, g: FixedType, t: FixedType) -> FixedType {
    let half = Fixed::new(5 * ONE_u128 / 10, false);
    y_0 + v_0y * t - half * g * t * t
}

#[test]
#[available_gas(2000000)]
fn test_deg_to_rad() {
    let sixty = Fixed::new(60_u128 * ONE_u128, false);
    let theta = deg_to_rad(sixty);
    // trig::PI/3 = 19317385221538994246
    // trig::PI/3 * 1.00000001
    assert(theta.mag < 19317385414712846461_u128, 'invalid theta, PI/3');
    // trig::PI/3 * 0.99999999
    assert(theta.mag > 19317385028365142031_u128, 'invalid theta, PI/3');
    assert(theta.sign == false, 'invalid sign');

    let minus_120 = Fixed::new(120_u128 * ONE_u128, true);
    let theta = deg_to_rad(minus_120);
    // trig::PI*2/3 = 38634770443077988493
    // trig::PI*2/3 * 1.00000001
    assert(theta.mag < 38634770829425692923_u128, 'invalid theta, PI');
    // trig::PI*2/3 * 0.99999999
    assert(theta.mag > 38634770056730284062_u128, 'invalid theta, PI');
    assert(theta.sign == true, 'invalid sign');
}

#[test]
#[available_gas(20000000)]
fn test_vec2_from_mag_theta() {
    let mag = Fixed::new(100_u128 * ONE_u128, false);
    let sixty = Fixed::new(60_u128 * ONE_u128, false);
    let theta = deg_to_rad(sixty);
    let vec2 = vec2_from_mag_theta(mag, theta);
    // expected value vec2.x = 922337203685477580800, false // 50
    // vec2.x.mag * 1.0000001
    assert(vec2.x.mag < 922337295919197949348_u128, 'invalid vec2.x.mag');
    // vec2.x.mag * 0.9999999
    assert(vec2.x.mag > 922337111451757212252_u128, 'invalid vec2.x.mag');
    assert(vec2.x.sign == false, 'invalid vec2.x.sign');

    // expected value vec2.y = 1597534898494251510150, false // 86.6
    // vec2.y.mag * 1.0000001
    assert(vec2.y.mag < 1597535058247741359576_u128, 'invalid vec2.y.mag');
    // vec2.y.mag * 0.9999999
    assert(vec2.y.mag > 1597534738740761660725_u128, 'invalid vec2.y.mag');
    assert(vec2.y.sign == false, 'invalid vec2.y.sig');

    let minus_120 = Fixed::new(120_u128 * ONE_u128, true);
    let theta = deg_to_rad(minus_120);
    let vec2 = vec2_from_mag_theta(mag, theta);
    // expected value vec2.x = 922337203685477580800, true // -50
    // vec2.x.mag * 1.0000001
    assert(vec2.x.mag < 922337295919197949348_u128, 'invalid vec2.x.mag');
    // vec2.x.mag * 0.9999999
    assert(vec2.x.mag > 922337111451757212252_u128, 'invalid vec2.x.mag');
    assert(vec2.x.sign == true, 'invalid vec2.x.sign');

    // expected value vec2.y = 1597534898494251510150, true // -86.6
    // vec2.y.mag * 1.0000001
    assert(vec2.y.mag < 1597535058247741359576_u128, 'invalid vec2.y.mag');
    // vec2.y.mag * 0.9999999
    assert(vec2.y.mag > 1597534738740761660725_u128, 'invalid vec2.y.mag');
    assert(vec2.y.sign == true, 'invalid vec2.y.sig');
}

#[test]
#[available_gas(20000000)]
fn test_fill_position_s() {
    //     plot_params: (FixedType, FixedType, FixedType, FixedType),
    //     motion_params: (Vec2<FixedType>, Vec2<FixedType>, FixedType, FixedType)
    // ) -> Array::<Vec2<FixedType>> {

    let v_0_mag = Fixed::from_unscaled_felt(100);
    let theta_0_deg = Fixed::from_unscaled_felt(65);
    let theta_0 = deg_to_rad(theta_0_deg);
    let x_0 = Fixed::from_unscaled_felt(0);
    let y_0 = Fixed::from_unscaled_felt(0);

    let x_max = Fixed::from_unscaled_felt(1000);
    let x_min = Fixed::from_unscaled_felt(-1000);
    let y_max = Fixed::from_unscaled_felt(500);
    let y_min = Fixed::from_unscaled_felt(-500);

    let r_0 = Vec2Trait::<FixedType>::new(x_0, y_0);
    let v_0 = vec2_from_mag_theta(v_0_mag, theta_0);
    let g = Fixed::new(98 * ONE_u128 / 10, false);
    let delta_t = Fixed::from_unscaled_felt(2);

    let plot_params = (x_max, x_min, y_max, y_min);
    let motion_params = (r_0, v_0, g, delta_t);

    let r_s = fill_position_s(plot_params, motion_params);
    let length = r_s.len();
    assert(length == 12, 'invalid length');

    // expected value of *r_s.at(5).x = 7795930915206679528264, false // 422.61826174069944
    // expected value * 1.0000001
    assert(*r_s.at(5).x.mag < 7795931694799771048932, 'invalid mag');
    // expected value * 0.9999999
    assert(*r_s.at(5).x.mag > 7795930135613588007596, 'invalid mag');
    assert(*r_s.at(5).x.sign == false, 'invalid sign');
    // expected value of *r_s.at(5).y = 7679523203357457794972, false // 416.3077870366498
    // expected value * 1.0000001
    assert(*r_s.at(5).y.mag < 7679523971309778130718, 'invalid mag');
    // expected value * 0.9999999
    assert(*r_s.at(5).y.mag > 7679522435405137459226, 'invalid mag');
    assert(*r_s.at(5).y.sign == false, 'invalid sign');

    // expected value of *r_s.at(10).x = 15591861830413359425462, false // 845.2365234813989
    // expected value * 1.0000001
    assert(*r_s.at(10).x.mag < 15591863389599542466798, 'invalid mag');
    // expected value * 0.9999999
    assert(*r_s.at(10).x.mag > 15591860271227176384126, 'invalid mag');
    assert(*r_s.at(10).x.sign == false, 'invalid sign');
    // expected value of *r_s.at(10).y = 2718762785520446838411, true // -147.3844259267005
    // expected value * 1.0000001
    assert(*r_s.at(10).y.mag < 2718763057396725390455, 'invalid mag');
    // expected value * 0.9999999
    assert(*r_s.at(10).y.mag > 2718762513644168286366, 'invalid mag');
    assert(*r_s.at(10).y.sign == true, 'invalid sign');
}

#[test]
#[available_gas(2000000)]
fn test_calc_x() {
    let x_0 = Fixed::new(100_u128 * ONE_u128, false);
    let v_0x = Fixed::new(50_u128 * ONE_u128, false);
    let t = Fixed::new(16_u128 * ONE_u128, false);
    let x = calc_x(x_0, v_0x, t);
    assert(x.mag == 900_u128 * ONE_u128, 'invalid mag');
    assert(x.sign == false, 'invalid sign');
}

#[test]
#[available_gas(2000000)]
fn test_calc_y() {
    let y_0 = Fixed::new(100 * ONE_u128, false);
    let v_0y = Fixed::new(50 * ONE_u128, false);
    let t = Fixed::new(16 * ONE_u128, false);
    let g = Fixed::new(98 * ONE_u128 / 10, false);
    let y = calc_y(y_0, v_0y, g, t);

    // expected y = -354.4
    // 354.4 * 1.0000001
    assert(y.mag < 35440003544 * ONE_u128 / 100000000, 'invalid mag');
    // 354.4 * 0.9999999
    assert(y.mag > 35439996456 * ONE_u128 / 100000000, 'invalid mag');
    assert(y.sign == true, 'invalid sign');
}
