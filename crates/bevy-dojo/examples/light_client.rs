use bevy::log;
use bevy::prelude::*;
use bevy_dojo::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // Set up Starknet light client plugin.
        .add_plugin(LightClientPlugin)
        .add_startup_system(spawn_buttons)
        // TODO: show node response in UI
        // .add_system(show_node_response)
        .add_system(on_click_starknet_block_number)
        .add_system(starknet_block_number)
        .run();
}

const NORMAL_BUTTON: Color = Color::rgb(0.15, 0.15, 0.15);
// const HOVERED_BUTTON: Color = Color::rgb(0.25, 0.25, 0.25);
// const PRESSED_BUTTON: Color = Color::rgb(0.35, 0.35, 0.35);

#[derive(Component)]
struct BlockNumberButton;

// TODO: spawn when NodeClient is added
/// Spawn buttons
fn spawn_buttons(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2dBundle::default());

    // Scan devices button
    commands
        .spawn(NodeBundle {
            style: Style {
                size: Size::width(Val::Percent(100.0)),
                justify_content: JustifyContent::Center,
                padding: UiRect::vertical(Val::Px(10.0)),
                ..default()
            },
            ..default()
        })
        .with_children(|parent| {
            parent
                .spawn((
                    BlockNumberButton,
                    ButtonBundle {
                        style: Style {
                            size: Size::new(Val::Px(240.0), Val::Px(40.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        background_color: NORMAL_BUTTON.into(),
                        ..default()
                    },
                ))
                .with_children(|parent| {
                    parent.spawn(TextBundle::from_section(
                        "starknet__block_number",
                        TextStyle {
                            font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                            font_size: 20.0,
                            color: Color::rgb(0.9, 0.9, 0.9),
                        },
                    ));
                });
        });
}

/// Emit `ScanDevices` event when user clicks button
fn on_click_starknet_block_number(
    query: Query<&Interaction, (Changed<Interaction>, With<Button>, With<BlockNumberButton>)>,
    mut starknet_block_number: EventWriter<StarknetBlockNumber>,
) {
    query.for_each(|interaction| {
        match *interaction {
            Interaction::Clicked => {
                starknet_block_number.send(StarknetBlockNumber);
            }
            _ => {}
        };
    });
}

fn starknet_block_number(mut events: EventReader<StarknetBlockNumber>, query: Query<&NodeClient>) {
    events.iter().for_each(|_e| {
        if let Ok(node) = query.get_single() {
            let _ = node.request(NodeRequest::starknet_block_number());
        } else {
            log::error!("Light client is not ready yet. Be patient.");
        }
    });
}
