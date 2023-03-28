#[system]
mod Spawn {
    use array::ArrayTrait;
    use traits::Into;   
    use starknet::contract_address::ContractAddressIntoFelt252;

    use dojo_examples::components::Position;
    use dojo_examples::components::Moves;

    fn execute() {
        let caller = starknet::get_caller_address();
        let player = commands::set(caller.into(), (
            Moves { remaining: 10_u8 },
            Position { x: 0_u32, y: 0_u32 },
        ));
        return ();
    }
}

#[system]
mod Move {
    use array::ArrayTrait;
    use traits::Into;
    use option::OptionTrait;
    use starknet::contract_address::ContractAddressIntoFelt252;

    use dojo_examples::components::Position;
    use dojo_examples::components::Moves;

    // TODO: Use enum once serde is derivable
    // left: 0, right: 1, up: 2, down: 3
    fn execute(direction: felt252) {
        let caller = starknet::get_caller_address();
        let (position, moves) = commands::<Position, Moves>::get(caller.into());
        let next = next_position(position.unwrap(), direction);
        let uh = commands::set(caller.into(), (
            Moves { remaining: moves.unwrap().remaining - 1_u8 },
            Position { x: next.x, y: next.y },
        ));
        return ();
    }

    fn next_position(position: Position, direction: felt252) -> Position {
        // TODO: Use match once supported
        // error: Only match zero (match ... { 0 => ..., _ => ... }) is currently supported.
        if direction == 0 {
            Position { x: position.x - 1_u32, y: position.y }
        } else if direction == 1 {
            Position { x: position.x + 1_u32, y: position.y }
        } else if direction == 2 {
            Position { x: position.x, y: position.y - 1_u32 }
        } else if direction == 3 {
            Position { x: position.x, y: position.y + 1_u32 }
        } else {
            position
        }
    }
}
