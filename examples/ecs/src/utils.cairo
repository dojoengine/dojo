use dojo_examples::models::{Position, Direction};

fn next_position(mut position: Position, direction: Direction) -> Position {
    match direction {
        Direction::None(()) => {
            return position;
        },
        Direction::Left(()) => {
            position.vec.x -= 1;
        },
        Direction::Right(()) => {
            position.vec.x += 1;
        },
        Direction::Up(()) => {
            position.vec.y -= 1;
        },
        Direction::Down(()) => {
            position.vec.y += 1;
        },
    };

    position
}
