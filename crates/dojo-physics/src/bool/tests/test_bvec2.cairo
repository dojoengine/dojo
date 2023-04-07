use dojo_physics::bool::bvec2::BVec2;
use dojo_physics::bool::bvec2::BVec2Type;

#[test]
fn test_bvec2() {
    // test `BVec2::new`
    let bvec2tf = BVec2::new(true, false);
    assert(bvec2tf.x == true, 'invalid new x');
    assert(bvec2tf.y == false, 'invalid new y');

    let bvec2ft = BVec2::new(false, true);
    assert(bvec2ft.x == false, 'invalid new x');
    assert(bvec2ft.y == true, 'invalid new y');

    // test `Vec2::splat`
    let bvec2tt = BVec2::splat(true);
    assert(bvec2tt.x == true, 'invalid new x');
    assert(bvec2tt.y == true, 'invalid new y');

    let bvec2ff = BVec2::splat(false);
    assert(bvec2ff.x == false, 'invalid new x');
    assert(bvec2ff.y == false, 'invalid new y');
}
