use dojo_core::integer::u250;

#[derive(Component, Copy, Drop, Serde)]
struct Status {
    is_authorized: bool
}

#[derive(Component, Copy, Drop, Serde)]
struct Role {
    id: u250
}
