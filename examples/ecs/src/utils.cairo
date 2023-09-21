use dojo_examples::components::{Position, Direction};

fn next_position(mut position: Position, direction: Direction) -> Position {
    match direction {
        Direction::None(()) => {
            return position;
        },
        Direction::Left(()) => {
            position.x -= 1;
        },
        Direction::Right(()) => {
            position.x += 1;
        },
        Direction::Up(()) => {
            position.y -= 1;
        },
        Direction::Down(()) => {
            position.y += 1;
        },
    };

    position
}
