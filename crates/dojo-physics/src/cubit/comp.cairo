use option::OptionTrait;
use traits::Into;

use dojo_physics::cubit::core::Fixed;
use dojo_physics::cubit::core::FixedType;
use dojo_physics::cubit::core::FixedImpl;
use dojo_physics::cubit::core::FixedPartialOrd;


// PUBLIC

fn max (a: FixedType, b: FixedType) -> FixedType {
    if (a >= b) {
        return a;
    } else {
        return b;
    }
}

fn min (a: FixedType, b: FixedType) -> FixedType {
    if (a <= b) {
        return a;
    } else {
        return b;
    }
}
