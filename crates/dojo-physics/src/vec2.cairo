use cubit::core::Fixed;
use cubit::core::FixedType;
use cubit::core::ONE_u128;

struct Vec2<T> {
    x: T,
    y: T
}

impl Vec2Copy<T, impl TCopy: Copy<T>> of Copy<Vec2<T>>;
impl Vec2Drop<T, impl TDrop: Drop<T>> of Drop<Vec2<T>>;

trait Vec2Trait<T> {
    // Constructors
    fn new(x: T, y: T) -> Vec2<T>;
    fn splat(self: T) -> Vec2<T>;
    // Masks
    fn select(mask: Vec2<bool>, if_true: Vec2<T>, if_false: Vec2<T>) -> Vec2<T>;
    // Math
    fn dot<impl TMul: Mul<T>, impl TAdd: Add<T>>(self: Vec2<T>, rhs: Vec2<T>) -> T;
    fn dot_into_vec<impl TMul: Mul<T>, impl TAdd: Add<T>>(self: Vec2<T>, rhs: Vec2<T>) -> Vec2<T>;
    // Swizzles

    fn xy(self: Vec2<T>) -> Vec2<T>;
    fn xx(self: Vec2<T>) -> Vec2<T>;
    fn yx(self: Vec2<T>) -> Vec2<T>;
    fn yy(self: Vec2<T>) -> Vec2<T>;
}

impl Vec2Impl<T, impl TCopy: Copy<T>, impl TDrop: Drop<T>> of Vec2Trait<T> {
    // Constructors

    /// Creates a new vector.
    #[inline(always)]
    fn new(x: T, y: T) -> Vec2<T> {
        Vec2 { x: x, y: y }
    }
    /// Creates a vector with all elements set to `v`.
    #[inline(always)]
    fn splat(v: T) -> Vec2<T> {
        Vec2 { x: v, y: v }
    }

    // Masks

    /// Creates a vector from the elements in `if_true` and `if_false`, 
    /// selecting which to use for each element of `self`.
    ///
    /// A true element in the mask uses the corresponding element from
    /// `if_true`, and false uses the element from `if_false`.
    #[inline(always)]
    fn select(mask: Vec2<bool>, if_true: Vec2<T>, if_false: Vec2<T>) -> Vec2<T> {
        Vec2 {
            x: if mask.x {
                if_true.x
            } else {
                if_false.x
            },
            y: if mask.y {
                if_true.y
            } else {
                if_false.y
            },
        }
    }

    // Math

    /// Computes the dot product of `self` and `rhs` . 
    // #[inline(always)] is not allowed for functions with impl generic parameters.
    fn dot<impl TMul: Mul<T>, impl TAdd: Add<T>>(self: Vec2<T>, rhs: Vec2<T>) -> T {
        (self.x * rhs.x) + (self.y * rhs.y)
    }
    /// Returns a vector where every component is the dot product
    /// of `self` and `rhs`.
    fn dot_into_vec<impl TMul: Mul<T>, impl TAdd: Add<T>>(self: Vec2<T>, rhs: Vec2<T>) -> Vec2<T> {
        Vec2Trait::splat(Vec2Trait::dot(self, rhs))
    }

    // Swizzles
    /// Vec2<T> -> Vec2<T>
    #[inline(always)]
    fn xx(self: Vec2<T>) -> Vec2<T> {
        Vec2 { x: self.x, y: self.x,  }
    }

    #[inline(always)]
    fn xy(self: Vec2<T>) -> Vec2<T> {
        Vec2 { x: self.x, y: self.y,  }
    }

    #[inline(always)]
    fn yx(self: Vec2<T>) -> Vec2<T> {
        Vec2 { x: self.y, y: self.x,  }
    }

    #[inline(always)]
    fn yy(self: Vec2<T>) -> Vec2<T> {
        Vec2 { x: self.y, y: self.y,  }
    }
}

#[test]
#[available_gas(2000000)]
fn test_new() {
    let var1_pos = Fixed::new(ONE_u128, false);
    let var2_neg = Fixed::new(2 * ONE_u128, true);

    // with FixedType
    let vec2 = Vec2Trait::new(var1_pos, var2_neg);
    assert(vec2.x.mag == ONE_u128, 'invalid x.mag');
    assert(vec2.x.sign == false, 'invalid x.sign');
    assert(vec2.y.mag == 2 * ONE_u128, 'invalid y.mag');
    assert(vec2.y.sign == true, 'invalid y.sign');

    // with bool
    let bvec2tf = Vec2Trait::new(true, false);
    assert(bvec2tf.x == true, 'invalid new x');
    assert(bvec2tf.y == false, 'invalid new y');
}

#[test]
#[available_gas(2000000)]
fn test_splat() {
    let var = Fixed::new(ONE_u128, false);

    // with FixedType
    let vec2 = Vec2Trait::splat(var);
    assert(vec2.x.mag == ONE_u128, 'invalid x.mag');
    assert(vec2.x.sign == false, 'invalid x.sign');
    assert(vec2.y.mag == ONE_u128, 'invalid y.mag');
    assert(vec2.y.sign == false, 'invalid y.sign');

    // with bool
    let bvec2tt = Vec2Trait::splat(true);
    assert(bvec2tt.x == true, 'invalid x');
    assert(bvec2tt.y == true, 'invalid y');
}

#[test]
#[available_gas(2000000)]
fn test_select() {
    let var1_pos = Fixed::new(ONE_u128, false);
    let var2_neg = Fixed::new(2 * ONE_u128, true);
    let var3_neg = Fixed::new(3 * ONE_u128, true);
    let var4_pos = Fixed::new(4 * ONE_u128, false);

    let vec2a = Vec2Trait::new(var1_pos, var2_neg);
    let vec2b = Vec2Trait::new(var3_neg, var4_pos);

    let mask = Vec2Trait::new(true, false);
    let vec2 = Vec2Trait::select(mask, vec2a, vec2b);
    assert(vec2.x.mag == ONE_u128, 'invalid x.mag');
    assert(vec2.x.sign == false, 'invalid x.sign');
    assert(vec2.y.mag == 4 * ONE_u128, 'invalid y.mag');
    assert(vec2.y.sign == false, 'invalid y.sign');

    let mask = Vec2Trait::new(false, true);
    let vec2 = Vec2Trait::select(mask, vec2a, vec2b);
    assert(vec2.x.mag == 3 * ONE_u128, 'invalid x.mag');
    assert(vec2.x.sign == true, 'invalid x.sign');
    assert(vec2.y.mag == 2 * ONE_u128, 'invalid y.mag');
    assert(vec2.y.sign == true, 'invalid y.sign');
}

#[test]
#[available_gas(2000000)]
fn test_dot() {
    let var1_pos = Fixed::new(ONE_u128, false);
    let var2_neg = Fixed::new(2 * ONE_u128, true);
    let var3_neg = Fixed::new(3 * ONE_u128, true);
    let var4_pos = Fixed::new(4 * ONE_u128, false);

    let vec2a = Vec2Trait::new(var1_pos, var2_neg);
    let vec2b = Vec2Trait::new(var3_neg, var4_pos);

    let a_dot_b = vec2a.dot(vec2b);
    assert(a_dot_b.mag == 11 * ONE_u128, 'invalid mag');
    assert(a_dot_b.sign == true, 'invalid sign');

    let a_dot_b = Vec2Trait::dot(vec2a, vec2b); // alt syntax
    assert(a_dot_b.mag == 11 * ONE_u128, 'invalid mag');
    assert(a_dot_b.sign == true, 'invalid sign');
}

#[test]
#[available_gas(2000000)]
fn test_dot_into_vec() {
    let var1_pos = Fixed::new(ONE_u128, false);
    let var2_neg = Fixed::new(2 * ONE_u128, true);
    let var3_neg = Fixed::new(3 * ONE_u128, true);
    let var4_pos = Fixed::new(4 * ONE_u128, false);

    let vec2a = Vec2Trait::new(var1_pos, var2_neg);
    let vec2b = Vec2Trait::new(var3_neg, var4_pos);

    let vec2 = vec2a.dot_into_vec(vec2b);
    assert(vec2.x.mag == 11 * ONE_u128, 'invalid x.mag');
    assert(vec2.x.sign == true, 'invalid x.sign');
    assert(vec2.y.mag == 11 * ONE_u128, 'invalid y.mag');
    assert(vec2.y.sign == true, 'invalid y.sign');

    let vec2 = Vec2Trait::dot_into_vec(vec2a, vec2b); // alt syntax
    assert(vec2.x.mag == 11 * ONE_u128, 'invalid x.mag');
    assert(vec2.x.sign == true, 'invalid x.sign');
    assert(vec2.y.mag == 11 * ONE_u128, 'invalid y.mag');
    assert(vec2.y.sign == true, 'invalid y.sign');
}

#[test]
#[available_gas(2000000)]
fn test_xx() {
    let var1_pos = Fixed::new(ONE_u128, false);
    let var2_neg = Fixed::new(2 * ONE_u128, true);
    let vec2 = Vec2Trait::new(var1_pos, var2_neg);

    let vec2xx = vec2.xx();
    assert(vec2xx.x.mag == ONE_u128, 'invalid x.mag');
    assert(vec2xx.x.sign == false, 'invalid x.sign');
    assert(vec2xx.y.mag == ONE_u128, 'invalid y.mag');
    assert(vec2xx.y.sign == false, 'invalid y.sign');
}

#[test]
#[available_gas(2000000)]
fn test_xy() {
    let var1_pos = Fixed::new(ONE_u128, false);
    let var2_neg = Fixed::new(2 * ONE_u128, true);
    let vec2 = Vec2Trait::new(var1_pos, var2_neg);

    let vec2xy = vec2.xy();
    assert(vec2xy.x.mag == ONE_u128, 'invalid x.mag');
    assert(vec2xy.x.sign == false, 'invalid x.sign');
    assert(vec2xy.y.mag == 2 * ONE_u128, 'invalid xy.mag');
    assert(vec2xy.y.sign == true, 'invalid y.sign');
}

#[test]
#[available_gas(2000000)]
fn test_yx() {
    let var1_pos = Fixed::new(ONE_u128, false);
    let var2_neg = Fixed::new(2 * ONE_u128, true);
    let vec2 = Vec2Trait::new(var1_pos, var2_neg);

    let vec2yx = vec2.yx();
    assert(vec2yx.x.mag == 2 * ONE_u128, 'invalid x.mag');
    assert(vec2yx.x.sign == true, 'invalid x.sign');
    assert(vec2yx.y.mag == ONE_u128, 'invalid y.mag');
    assert(vec2yx.y.sign == false, 'invalid y.sign');
}

#[test]
#[available_gas(2000000)]
fn test_yy() {
    let var1_pos = Fixed::new(ONE_u128, false);
    let var2_neg = Fixed::new(2 * ONE_u128, true);
    let vec2 = Vec2Trait::new(var1_pos, var2_neg);

    let vec2yy = vec2.yy();
    assert(vec2yy.x.mag == 2 * ONE_u128, 'invalid x.mag');
    assert(vec2yy.x.sign == true, 'invalid x.sign');
    assert(vec2yy.y.mag == 2 * ONE_u128, 'invalid y.mag');
    assert(vec2yy.y.sign == true, 'invalid y.sign');
}
