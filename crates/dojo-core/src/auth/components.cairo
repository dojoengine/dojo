use dojo_core::integer::u250;

#[derive(Component)]
struct Status {
    is_authorized: bool
}

#[derive(Component)]
struct Role {
    id: u250
}
