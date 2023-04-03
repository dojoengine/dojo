use eternum::components::config::WorldConfig;
use eternum::utils::math::get_past_time;
use eternum::constants::WORLD_CONFIG_ID;

#[derive(Component)]
struct Tick {
    last_update: u128
}
trait TickTrait {
    fn needs_update(self: Tick, world_config: WorldConfig, block_timestamp: u128) -> bool;
}

impl TickImpl of TickTrait {
    fn needs_update(self: Tick, world_config: WorldConfig, block_timestamp: u128) -> bool {
        get_past_time(self.last_update + world_config.tick_time, block_timestamp)
    }
}

