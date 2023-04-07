use cubit::core::ONE_u128;
use cubit::core::Fixed;

use dojo_physics::fixed_type::vec2::Vec2;
use dojo_physics::swizzles::vec_traits::Vec2Swizzles;
use dojo_physics::swizzles::vec2_impl::Vec2SwizzlesImpl;

#[test]
fn test_vec2_impl() {
    let var1_pos = Fixed::new(ONE_u128, false);
    let var2_neg = Fixed::new(2_u128 * ONE_u128, true);
    let vec2 = Vec2::new(var1_pos, var2_neg);

    // tests Vec2Type -> Vec2Type
    let vec2xx = vec2.xx();
    assert(vec2xx.x.mag == ONE_u128, 'invalid xx x.mag');
    assert(vec2xx.x.sign == false, 'invalid xx x.sign');
    assert(vec2xx.y.mag == ONE_u128, 'invalid xx y.mag');
    assert(vec2xx.y.sign == false, 'invalid xx y.sign');

    let vec2xy = vec2.xy();
    assert(vec2xy.x.mag == ONE_u128, 'invalid xy x.mag');
    assert(vec2xy.x.sign == false, 'invalid xy x.sign');
    assert(vec2xy.y.mag == 2_u128 * ONE_u128, 'invalid xy y.mag');
    assert(vec2xy.y.sign == true, 'invalid xy y.sign');

    let vec2yx = vec2.yx();
    assert(vec2yx.x.mag == 2_u128 * ONE_u128, 'invalid yx x.mag');
    assert(vec2yx.x.sign == true, 'invalid yx x.sign');
    assert(vec2yx.y.mag == ONE_u128, 'invalid yx y.mag');
    assert(vec2yx.y.sign == false, 'invalid yx y.sign');

    let vec2yy = vec2.yy();
    assert(vec2yy.x.mag == 2_u128 * ONE_u128, 'invalid yy x.mag');
    assert(vec2yy.x.sign == true, 'invalid yy x.sign');
    assert(vec2yy.y.mag == 2_u128 * ONE_u128, 'invalid yy y.mag');
    assert(vec2yy.y.sign == true, 'invalid yy y.sign');
}
