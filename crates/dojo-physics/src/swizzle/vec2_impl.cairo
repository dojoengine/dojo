use cubit::dojo_physics::fixed::vec2::Vec2;
use cubit::dojo_physics::swizzle::vec_traits::Vec2Swizzle;

impl Vec2SwizzleImpl<T, impl TCopy: Copy<T>, impl TDrop: Drop<T>> of Vec2Swizzle<T> {
    #[inline(always)]
    fn xx(self: Vec2<T>) -> Vec2<T> {
        Vec2::<T> { x: self.x, y: self.x,  }
    }

    #[inline(always)]
    fn xy(self: Vec2<T>) -> Vec2<T> {
        Vec2::<T> { x: self.x, y: self.y,  }
    }

    #[inline(always)]
    fn yx(self: Vec2<T>) -> Vec2<T> {
        Vec2::<T> { x: self.y, y: self.x,  }
    }

    #[inline(always)]
    fn yy(self: Vec2<T>) -> Vec2<T> {
        Vec2::<T> { x: self.y, y: self.y,  }
    }
}

