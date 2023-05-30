use bevy::log;
use bevy::prelude::*;
use bevy_dojo::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // Set up Starknet light client plugin.
        .add_plugins(DojoPlugins)
        .add_startup_system(setup_ui)
        .add_systems((
            spawn_buttons,
            button_hover_style,
            // TODO: show node response in UI
            // show_node_response
            on_click_starknet_block_number,
            log_starknet_block_number,
            on_click_eth_get_block_number,
            log_eth_block_number,
        ))
        .run();
}

const NORMAL_BUTTON: Color = Color::rgb(0.15, 0.15, 0.15);
const HOVERED_BUTTON: Color = Color::rgb(0.25, 0.25, 0.25);
const PRESSED_BUTTON: Color = Color::rgb(0.35, 0.35, 0.35);

#[derive(Component)]
struct StarknetBlockNumberButton;

#[derive(Component)]
struct EthGetBlockNumberButton;

#[derive(Component)]
struct StartingMessage;

fn setup_ui(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2dBundle::default());

    commands
        .spawn((
            NodeBundle {
                style: Style {
                    size: Size::width(Val::Percent(100.0)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                ..default()
            },
            StartingMessage,
        ))
        .with_children(|parent| {
            parent.spawn(TextBundle::from_section(
                "Starting a light client...",
                TextStyle {
                    font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                    font_size: 24.0,
                    color: Color::rgb(0.9, 0.9, 0.9),
                },
            ));
            parent.spawn(TextBundle::from_section(
                "Check console for logs. It may take a minute.",
                TextStyle {
                    font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                    font_size: 20.0,
                    color: Color::rgb(0.6, 0.6, 0.6),
                },
            ));
        });
}

/// Spawn buttons
///
/// ### TODO
/// - Reduce boilerplate to spawn each buttons
fn spawn_buttons(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    light_client_query: Query<&LightClient, Added<LightClient>>,
    starting_message_query: Query<Entity, With<StartingMessage>>,
) {
    if !light_client_query.is_empty() {
        if let Ok(entity) = starting_message_query.get_single() {
            commands.entity(entity).despawn_recursive();
        } else {
            log::error!("StartingMessage component doesn't exist. Make sure to spawn in startup");
        }

        commands
            .spawn(NodeBundle {
                style: Style { flex_direction: FlexDirection::Column, ..default() },
                ..default()
            })
            .with_children(|parent| {
                parent
                    .spawn(NodeBundle {
                        style: Style { padding: UiRect::all(Val::Px(10.0)), ..default() },
                        ..default()
                    })
                    .with_children(|parent| {
                        parent
                            .spawn((
                                StarknetBlockNumberButton,
                                ButtonBundle {
                                    style: Style {
                                        justify_content: JustifyContent::Center,
                                        align_items: AlignItems::Center,
                                        padding: UiRect::all(Val::Px(10.0)),
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

                parent
                    .spawn(NodeBundle {
                        style: Style { padding: UiRect::all(Val::Px(10.0)), ..default() },
                        ..default()
                    })
                    .with_children(|parent| {
                        parent
                            .spawn((
                                EthGetBlockNumberButton,
                                ButtonBundle {
                                    style: Style {
                                        justify_content: JustifyContent::Center,
                                        align_items: AlignItems::Center,
                                        padding: UiRect::all(Val::Px(10.0)),
                                        ..default()
                                    },
                                    background_color: NORMAL_BUTTON.into(),
                                    ..default()
                                },
                            ))
                            .with_children(|parent| {
                                parent.spawn(TextBundle::from_section(
                                    "ethereum__get_block_number",
                                    TextStyle {
                                        font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                                        font_size: 20.0,
                                        color: Color::rgb(0.9, 0.9, 0.9),
                                    },
                                ));
                            });
                    });
            });
    }
}

/// Change button color on mouse hover
fn button_hover_style(
    mut query: Query<(&Interaction, &mut BackgroundColor), (Changed<Interaction>, With<Button>)>,
) {
    query.for_each_mut(|(interaction, mut color)| {
        match *interaction {
            Interaction::Clicked => {
                *color = PRESSED_BUTTON.into();
            }
            Interaction::Hovered => {
                *color = HOVERED_BUTTON.into();
            }
            Interaction::None => {
                *color = NORMAL_BUTTON.into();
            }
        };
    });
}

/// Emit [StarknetBlockNumber] event when user clicks button
fn on_click_starknet_block_number(
    query: Query<
        &Interaction,
        (Changed<Interaction>, With<Button>, With<StarknetBlockNumberButton>),
    >,
    mut event: EventWriter<StarknetBlockNumber>,
) {
    query.for_each(|interaction| {
        match *interaction {
            Interaction::Clicked => {
                event.send(StarknetBlockNumber);
            }
            _ => {}
        };
    });
}

/// Emit [EthGetBlockNumber] event when user clicks button
fn on_click_eth_get_block_number(
    query: Query<&Interaction, (Changed<Interaction>, With<Button>, With<EthGetBlockNumberButton>)>,
    mut event: EventWriter<EthereumBlockNumber>,
) {
    query.for_each(|interaction| {
        match *interaction {
            Interaction::Clicked => {
                event.send(EthereumBlockNumber);
            }
            _ => {}
        };
    });
}

/// Log Starknet block number when [BlockNumber] with [Starknet] label component spawned
fn log_starknet_block_number(
    query: Query<(Entity, &BlockNumber), With<Starknet>>,
    mut commands: Commands,
) {
    query.iter().for_each(|(entity, block_number)| {
        log::info!("Starknet Block Number: {}", block_number.value);
        commands.entity(entity).despawn();
    })
}

/// Log Eth block number when [BlockNumber] with [Eth] label component spawned
fn log_eth_block_number(
    query: Query<(Entity, &BlockNumber), With<Ethereum>>,
    mut commands: Commands,
) {
    query.iter().for_each(|(entity, block_number)| {
        log::info!("Ethereum Block Number: {}", block_number.value);
        commands.entity(entity).despawn();
    })
}
