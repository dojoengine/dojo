use cubit::dojo_physics::fixed::vec2::Vec2;

trait Vec2Swizzle<T> {
    // Vec2<T> -> Vec2<T>
    fn xy(self: Vec2<T>) -> Vec2<T>;
    fn xx(self: Vec2<T>) -> Vec2<T>;
    fn yx(self: Vec2<T>) -> Vec2<T>;
    fn yy(self: Vec2<T>) -> Vec2<T>;
}

