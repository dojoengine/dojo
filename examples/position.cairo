// #[component]
// mod PositionComponent {
//     struct Position {
//         x: felt,
//         y: felt
//     }

//     #[view]
//     fn is_zero(self: Position) -> bool {
//         match self.x - self.y {
//             0 => bool::True(()),
//             _ => bool::False(()),
//         }
//     }

//     #[view]
//     fn is_equal(self: Position, b: Position) -> bool {
//         self.x == b.x & self.y == b.y
//     }
// }

#[contract]
mod HelloStarknet {
    struct Storage {
        balance: felt, 
    }

    // Increases the balance by the given amount.
    #[external]
    fn increase_balance(amount: felt) {
        balance::write(balance::read() + amount);
    }

    // Returns the current balance.
    #[view]
    fn get_balance() -> felt {
        balance::read()
    }
}
