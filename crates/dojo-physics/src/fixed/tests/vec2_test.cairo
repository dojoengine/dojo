use cubit::core::ONE_u128;
use cubit::core::Fixed;
use cubit::core::FixedType;

use dojo_physics::fixed::vec2::Vec2Trait;

#[test]
fn test_vec2() {
    let var1_pos = Fixed::new(ONE_u128, false);
    let var2_neg = Fixed::new(2_u128 * ONE_u128, true);
    let var3_neg = Fixed::new(3_u128 * ONE_u128, true);
    let var4_pos = Fixed::new(4_u128 * ONE_u128, false);

    // test `Vec2Trait::<FixedType>::new`
    let vec2a = Vec2Trait::<FixedType>::new(var1_pos, var2_neg);
    assert(vec2a.x.mag == ONE_u128, 'invalid new x.mag');
    assert(vec2a.x.sign == false, 'invalid new x.sign');
    assert(vec2a.y.mag == 2_u128 * ONE_u128, 'invalid new y.mag');
    assert(vec2a.y.sign == true, 'invalid new y.sign');

    let vec2b = Vec2Trait::<FixedType>::new(var3_neg, var4_pos);
    assert(vec2b.x.mag == 3_u128 * ONE_u128, 'invalid new x.mag');
    assert(vec2b.x.sign == true, 'invalid new x.sign');
    assert(vec2b.y.mag == 4_u128 * ONE_u128, 'invalid new y.mag');
    assert(vec2b.y.sign == false, 'invalid new y.sign');

    // test `Vec2Trait::<FixedType>::splat`
    let vec2 = Vec2Trait::<FixedType>::splat(var1_pos);
    assert(vec2.x.mag == ONE_u128, 'invalid splat x.mag');
    assert(vec2.x.sign == false, 'invalid splat x.sign');
    assert(vec2.y.mag == ONE_u128, 'invalid splat y.mag');
    assert(vec2.y.sign == false, 'invalid splat y.sign');

    // test `Vec2Trait::<bool>::new`
    let bvec2tf = Vec2Trait::<bool>::new(true, false);
    assert(bvec2tf.x == true, 'invalid new x');
    assert(bvec2tf.y == false, 'invalid new y');

    let bvec2ft = Vec2Trait::<bool>::new(false, true);
    assert(bvec2ft.x == false, 'invalid new x');
    assert(bvec2ft.y == true, 'invalid new y');

    // test `Vec2Trait::<bool>::splat`
    let bvec2tt = Vec2Trait::<bool>::splat(true);
    assert(bvec2tt.x == true, 'invalid new x');
    assert(bvec2tt.y == true, 'invalid new y');

    let bvec2ff = Vec2Trait::<bool>::splat(false);
    assert(bvec2ff.x == false, 'invalid new x');
    assert(bvec2ff.y == false, 'invalid new y');

    // test `Vec2Trait::select`
    let mask = Vec2Trait::<bool>::new(true, false);
    let vec2 = Vec2Trait::select(mask, vec2a, vec2b);
    assert(vec2.x.mag == ONE_u128, 'invalid select x.mag');
    assert(vec2.x.sign == false, 'invalid select x.sign');
    assert(vec2.y.mag == 4_u128 * ONE_u128, 'invalid select y.mag');
    assert(vec2.y.sign == false, 'invalid select y.sign');

    let mask = Vec2Trait::<bool>::new(false, true);
    let vec2 = Vec2Trait::select(mask, vec2a, vec2b);
    assert(vec2.x.mag == 3_u128 * ONE_u128, 'invalid select x.mag');
    assert(vec2.x.sign == true, 'invalid select x.sign');
    assert(vec2.y.mag == 2_u128 * ONE_u128, 'invalid select y.mag');
    assert(vec2.y.sign == true, 'invalid select y.sign');

    // test `Vec2Trait::dot`
    // let a_dot_b = vec2a.dot(vec2b); // gets "error: Type annotations needed"
    let a_dot_b = Vec2Trait::<FixedType>::dot(vec2a, vec2b);
    assert(a_dot_b.mag == 11_u128 * ONE_u128, 'invalid dot mag');
    assert(a_dot_b.sign == true, 'invalid dot sign');

    // test `Vec2Trait::dot_into_vec`
    // let vec2 = vec2a.dot_into_vec(vec2b); // gets "error: Type annotations needed"
    let vec2 = Vec2Trait::<FixedType>::dot_into_vec(vec2a, vec2b);
    assert(vec2.x.mag == 11_u128 * ONE_u128, 'invalid dot_into_vec x.mag');
    assert(vec2.x.sign == true, 'invali  dot_into_vec x.sig');
    assert(vec2.y.mag == 11_u128 * ONE_u128, 'invalid dot_into_vec y.mag');
    assert(vec2.y.sign == true, 'invalid dot_into_vec y.sig');
}
