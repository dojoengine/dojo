use dojo_physics::fixed_type::vec2::Vec2Type;

use dojo_physics::swizzles::vec_traits::Vec2Swizzles;

impl Vec2SwizzlesImpl of Vec2Swizzles {
    #[inline(always)]
    fn xx(self: Vec2Type) -> Vec2Type {
        Vec2Type { x: self.x, y: self.x,  }
    }

    #[inline(always)]
    fn xy(self: Vec2Type) -> Vec2Type {
        Vec2Type { x: self.x, y: self.y,  }
    }

    #[inline(always)]
    fn yx(self: Vec2Type) -> Vec2Type {
        Vec2Type { x: self.y, y: self.x,  }
    }

    #[inline(always)]
    fn yy(self: Vec2Type) -> Vec2Type {
        Vec2Type { x: self.y, y: self.y,  }
    }
}
