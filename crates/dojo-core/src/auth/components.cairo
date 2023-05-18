use dojo_core::integer::u250;

#[derive(Component, Copy, Drop, Serde)]
struct AuthStatus {
    is_authorized: bool
}

#[derive(Component, Copy, Drop, Serde)]
struct AuthRole {
    id: u250
}
