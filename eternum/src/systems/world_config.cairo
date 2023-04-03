// Harvests Labor

#[system]
mod WorldConfig {
    use array::ArrayTrait;
    use traits::Into;

    use eternum::components::resources::Wood;

    use eternum::components::config::WorldConfig;

    use eternum::constants::WORLD_CONFIG_ID;

    fn execute(
        day_unix: u128,
        vault_bp: u128,
        base_resources_per_day: u128,
        vault_unix: u128,
        lords_per_day: u128,
        tick_time: u128
    ) { // can only be executed by Governance Vote
        let _ = commands::set(
            (WORLD_CONFIG_ID).into(),
            (WorldConfig {
                day_unix: day_unix,
                vault_bp: vault_bp,
                base_resources_per_day: base_resources_per_day,
                vault_unix: vault_unix,
                lords_per_day: lords_per_day,
                tick_time: tick_time
            })
        );
    }
}
