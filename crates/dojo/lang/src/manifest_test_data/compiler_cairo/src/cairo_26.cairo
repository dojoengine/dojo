//! Cairo 2.6.0 feature testing.
#[starknet::contract]
mod cairo_v260 {
    // Constants.
    enum ThreeOptions {
        A: felt252,
        B: (u256, u256),
        C,
    }

    struct ThreeOptionsPair {
        a: ThreeOptions,
        b: ThreeOptions,
    }

    const V: ThreeOptionsPair = ThreeOptionsPair {
        a: ThreeOptions::A(1337),
        b: ThreeOptions::C,
    };

    #[storage]
    struct Storage {}

    #[derive(Drop)]
    enum MyEnum {
        Foo,
        Bar
    }

    fn if_let() {
        let number = Option::Some(5);
        let foo_or_bar = MyEnum::Foo;

        if let Option::Some(i) = number {
            println!("{}", i);
        }

        if let MyEnum::Bar = foo_or_bar {
            println!("bar");
        }
    }

    fn while_let(mut arr: Array<felt252>) -> felt252 {
        let mut sum = 0;
        while let Option::Some(x) = arr.pop_front() {
            sum += x;
        };
        sum
    }

    fn const_reference() -> ThreeOptionsPair { V }
    fn const_box() -> Box<ThreeOptionsPair> { BoxTrait::new(V) }
}
