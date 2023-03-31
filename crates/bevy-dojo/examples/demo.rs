use bevy::prelude::*;
use bevy::sprite::MaterialMesh2dBundle;
use bevy::window::PresentMode;
use bevy_dojo::IndexerPlugin;

// Contains the ID of the player for tracking and applying updates
#[derive(Component)]
struct Player(String);

// Marks this player
#[derive(Component)]
struct ThisPlayer;

// Point on the grid
#[derive(Component, Clone)]
struct Position {
    x: i8,
    y: i8,
}

// Util: Position from x and y
const fn pos(x: i8, y: i8) -> Position {
    Position { x, y }
}

const SCREEN_SIZE: (f32, f32) = (800., 500.);
const GRID_GAP: f32 = 50.;
const GRID_COLOR: Color = Color::rgb(0.9, 0.9, 0.9);

// Mocks other players
const EXISTING_PLAYERS_MOCK: [Position; 4] = [pos(2, 3), pos(5, -4), pos(-5, -1), pos(-7, 3)];

fn main() {
    App::new()
		// Window config
        .add_plugins(DefaultPlugins.set(window_plugin()))
		// Background color
        .insert_resource(ClearColor(Color::WHITE))
		// Setups up the grid
        .add_startup_system(setup_grid)
		// Sets up all the player
        .add_startup_system(setup_players)
		// Moves the player after input
        .add_system(player_input)
		// Le indexeur
        .add_plugin(IndexerPlugin)
        .run();
}

fn window_plugin() -> WindowPlugin {
    WindowPlugin {
        primary_window: Some(Window {
            title: "Dojo testing".into(),
            resolution: SCREEN_SIZE.into(),
            present_mode: PresentMode::AutoVsync,
            fit_canvas_to_parent: true,
            prevent_default_event_handling: false,
            ..default()
        }),
        ..default()
    }
}

fn setup_grid(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());

    // Rectangle
    let mut x = -1. * SCREEN_SIZE.0 / 2. + GRID_GAP / 2.;
    while x < SCREEN_SIZE.0 / 2. {
        commands.spawn(SpriteBundle {
            sprite: Sprite {
                color: GRID_COLOR,
                custom_size: Some(Vec2::new(1.0, 9999.0)),
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(x, 0., 0.)),
            ..default()
        });
        x += GRID_GAP;
    }
    let mut y = -1. * SCREEN_SIZE.1 / 2. + GRID_GAP / 2.;
    while y < SCREEN_SIZE.1 / 2. {
        commands.spawn(SpriteBundle {
            sprite: Sprite {
                color: GRID_COLOR,
                custom_size: Some(Vec2::new(9999.0, 1.0)),
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(0., y, 0.)),
            ..default()
        });
        y += GRID_GAP;
    }
}

fn other_player_positions() -> Vec<(Position, String)> {
    EXISTING_PLAYERS_MOCK.into_iter().map(|pos| (pos, "player_addr".into())).collect()
}

fn setup_players(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    for (player_pos, player_addr) in other_player_positions() {
        let mut transform = Transform::from_translation(Vec3::ZERO);
        set_transform_transition_from_pos(&mut transform, &player_pos);
        commands.spawn((
            MaterialMesh2dBundle {
                mesh: meshes.add(shape::Circle::new(GRID_GAP / 3.5).into()).into(),
                material: materials.add(ColorMaterial::from(Color::SALMON)),
                transform,
                ..default()
            },
            player_pos,
            Player(player_addr),
        ));
    }

    commands.spawn((
        MaterialMesh2dBundle {
            mesh: meshes.add(shape::Circle::new(GRID_GAP / 3.5).into()).into(),
            material: materials.add(ColorMaterial::from(Color::CRIMSON)),
            transform: Transform::from_translation(Vec3::ZERO),
            ..default()
        },
        Player("0xf00dba291834e2423dc4842342a34".into()),
        Position { x: 0, y: 0 },
        ThisPlayer,
    ));
}

fn set_transform_transition_from_pos(transform: &mut Transform, pos: &Position) {
    transform.translation.x = pos.x as f32 * GRID_GAP;
    transform.translation.y = pos.y as f32 * GRID_GAP;
}

fn player_input(
    keyboard_input: Res<Input<KeyCode>>,
    mut qry: Query<(&mut Position, &mut Transform, &ThisPlayer)>,
) {
    for (mut pos, mut transform, _) in qry.iter_mut() {
        if keyboard_input.just_pressed(KeyCode::Up) {
            pos.y += 1;
        }
        if keyboard_input.just_pressed(KeyCode::Down) {
            pos.y -= 1;
        }
        if keyboard_input.just_pressed(KeyCode::Right) {
            pos.x += 1;
        }
        if keyboard_input.just_pressed(KeyCode::Left) {
            pos.x -= 1;
        }
        // Update transform from position
        transform.translation.x = pos.x as f32 * GRID_GAP;
        transform.translation.y = pos.y as f32 * GRID_GAP;
    }
}
