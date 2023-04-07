use dojo_physics::fixed_type::vec2::Vec2Type;

// Traits for Vec2Swizzles
trait Vec2Swizzles {
    // Vec2Type -> Vec2Type
    fn xy(self: Vec2Type) -> Vec2Type;
    fn xx(self: Vec2Type) -> Vec2Type;
    fn yx(self: Vec2Type) -> Vec2Type;
    fn yy(self: Vec2Type) -> Vec2Type;
}
