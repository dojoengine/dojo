use cubit::core::FixedType;

use dojo_physics::bool::bvec2::BVec2Type;

/// A 2-dimensional vector.
#[derive(Copy, Drop)]
struct Vec2Type {
    x: FixedType,
    y: FixedType,
}

// Traits for Vec2Type
trait Vec2 {
    // Constructors
    fn new(x: FixedType, y: FixedType) -> Vec2Type;
    fn splat(v: FixedType) -> Vec2Type;
    fn select(mask: BVec2Type, if_true: Vec2Type, if_false: Vec2Type) -> Vec2Type;
    // Math
    fn dot(self: Vec2Type, rhs: Vec2Type) -> FixedType;
    fn dot_into_vec(self: Vec2Type, rhs: Vec2Type) -> Vec2Type;
}

impl Vec2Impl of Vec2 {
    // Constructors

    /// Creates a new vector.
    #[inline(always)]
    fn new(x: FixedType, y: FixedType) -> Vec2Type {
        Vec2Type { x: x, y: y }
    }

    /// Creates a vector with all elements set to `v`.
    #[inline(always)]
    fn splat(v: FixedType) -> Vec2Type {
        Vec2Type { x: v, y: v }
    }

    /// Creates a vector from the elements in `if_true` and `if_false`, selecting which to use
    /// for each element of `self`.
    ///
    /// A true element in the mask uses the corresponding element from `if_true`, and false
    /// uses the element from `if_false`.
    #[inline(always)]
    fn select(mask: BVec2Type, if_true: Vec2Type, if_false: Vec2Type) -> Vec2Type {
        Vec2Type {
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

    /// Computes the dot product of `self` and `rhs`.
    #[inline(always)]
    fn dot(self: Vec2Type, rhs: Vec2Type) -> FixedType {
        (self.x * rhs.x) + (self.y * rhs.y)
    }

    /// Returns a vector where every component is the dot product of `self` and `rhs`.
    #[inline(always)]
    fn dot_into_vec(self: Vec2Type, rhs: Vec2Type) -> Vec2Type {
        Vec2::splat(self.dot(rhs))
    }
}

