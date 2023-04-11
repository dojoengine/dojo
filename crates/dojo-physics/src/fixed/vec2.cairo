use cubit::core::FixedType;

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
    fn dot(self: Vec2<FixedType>, rhs: Vec2<FixedType>) -> FixedType;
    fn dot_into_vec(self: Vec2<FixedType>, rhs: Vec2<FixedType>) -> Vec2<FixedType>;
}

impl Vec2Impl<T, impl TCopy: Copy<T>, impl TDrop: Drop<T>> of Vec2Trait<T> {
    // Constructors

    /// Creates a new vector.
    #[inline(always)]
    fn new(x: T, y: T) -> Vec2<T> {
        Vec2::<T> { x: x, y: y }
    }
    /// Creates a vector with all elements set to `v`.
    #[inline(always)]
    fn splat(v: T) -> Vec2<T> {
        Vec2::<T> { x: v, y: v }
    }

    // Masks

    /// Creates a vector from the elements in `if_true` and `if_false`, 
    /// selecting which to use for each element of `self`.
    ///
    /// A true element in the mask uses the corresponding element from
    /// `if_true`, and false uses the element from `if_false`.
    #[inline(always)]
    fn select(mask: Vec2<bool>, if_true: Vec2<T>, if_false: Vec2<T>) -> Vec2<T> {
        Vec2::<T> {
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
    #[inline(always)]
    fn dot(self: Vec2<FixedType>, rhs: Vec2<FixedType>) -> FixedType {
        (self.x * rhs.x) + (self.y * rhs.y)
    }
    /// Returns a vector where every component is the dot product
    /// of `self` and `rhs`.
    #[inline(always)]
    fn dot_into_vec(self: Vec2<FixedType>, rhs: Vec2<FixedType>) -> Vec2<FixedType> {
        Vec2Trait::<FixedType>::splat(Vec2Trait::<FixedType>::dot(self, rhs))
    }
}
