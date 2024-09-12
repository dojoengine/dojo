use katana_runner::KatanaRunner;
use migrate::World;
use saya::{commands::*, l2::L2};
use serde::{Deserialize, Serialize};

pub static MANIFEST_PATH: &str = "examples/spawn-and-move/Scarb.toml";

fn main() {
    let l2 = L2::Katana(KatanaRunner::new().unwrap());
    let state = State::WorldBuild(WorldBuildState { manifest_path: MANIFEST_PATH.to_string(), l2 });
    let finish_state = state.execute();
    println!("{:?}", finish_state);
}

#[derive(Debug, Serialize, Deserialize)]
pub enum StateKind {
    WorldBuild,
    WorldMigrate,
    WorldPrepare,
    Finish,
}

impl StateKind {
    pub fn id(&self) -> u32 {
        match self {
            StateKind::WorldBuild => 1,
            StateKind::WorldMigrate => 2,
            StateKind::WorldPrepare => 3,
            StateKind::Finish => 4,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum State {
    WorldBuild(WorldBuildState),
    WorldMigrate(WorldMigrateState),
    WorldPrepare(WorldPrepareState),
    Finish(FinishState),
}

impl State {
    pub fn id(&self) -> u32 {
        match self {
            State::WorldBuild(_) => 1,
            State::WorldMigrate(_) => 2,
            State::WorldPrepare(_) => 3,
            State::Finish(_) => 4,
        }
    }
    pub fn execute_until(self, until: StateKind) -> Self {
        let mut state = self;
        while state.id() < until.id() {
            state = state.transition();
        }
        state
    }
    pub fn transition(self) -> Self {
        match self {
            State::WorldBuild(state) => State::WorldMigrate(state.to_migrate_state()),
            State::WorldMigrate(state) => State::WorldPrepare(state.to_world_prepare_state()),
            State::WorldPrepare(_state) => State::Finish(FinishState {}),
            State::Finish(state) => State::Finish(state),
        }
    }
    pub fn execute(self) -> FinishState {
        let state = self.execute_until(StateKind::Finish);
        match state {
            State::Finish(state) => state,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorldBuildState {
    pub manifest_path: String,
    pub l2: L2,
}

impl WorldBuildState {
    pub fn to_migrate_state(self) -> WorldMigrateState {
        BuildCommandSetup::new(MANIFEST_PATH)
            .command()
            .inherit_io_wait_with_output()
            .status
            .unwrap();
        WorldMigrateState { manifest_path: self.manifest_path, l2: self.l2 }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorldMigrateState {
    pub manifest_path: String,
    pub l2: L2,
}

impl WorldMigrateState {
    pub fn to_world_prepare_state(self) -> WorldPrepareState {
        let world =
            MigrateComandSetup::new(&self.manifest_path, &self.l2).command().wait_get_unwrap();
        WorldPrepareState { manifest_path: self.manifest_path, l2: self.l2, world }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorldPrepareState {
    pub manifest_path: String,
    pub l2: L2,
    pub world: World,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FinishState {}
