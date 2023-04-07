struct BVec2Type {
    x: bool,
    y: bool,
}

// Traits for BVec2Type
trait BVec2 {
    // Constructors
    fn new(x: bool, y: bool) -> BVec2Type;
    fn splat(v: bool) -> BVec2Type;
// Math

}

impl BVec2Impl of BVec2 {
    // Constructors

    /// Creates a new vector.
    #[inline(always)]
    fn new(x: bool, y: bool) -> BVec2Type {
        BVec2Type { x: x, y: y }
    }

    /// Creates a vector with all elements set to `v`.
    #[inline(always)]
    fn splat(v: bool) -> BVec2Type {
        BVec2Type { x: v, y: v }
    }
// Math

}
