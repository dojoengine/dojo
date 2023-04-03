#[derive(Component)]
struct WorldConfig {
    day_unix: u128,
    vault_bp: u128,
    base_resources_per_day: u128,
    vault_unix: u128,
    lords_per_day: u128,
    tick_time: u128,
}

