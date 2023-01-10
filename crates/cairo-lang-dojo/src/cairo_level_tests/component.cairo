trait Component<T> {
    fn get<T>(entity_id: felt) -> T;
}

#[derive(Component)]
struct Position { x: felt, y: felt }
