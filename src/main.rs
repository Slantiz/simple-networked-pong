use bevy::{camera::ScalingMode, prelude::*, window::WindowResolution};

mod config;
mod network;
mod simulation;
mod states;
mod ui;
mod visuals;

use crate::{
    config::{ARENA_HEIGHT, ARENA_WIDTH},
    network::NetworkPlugin,
    simulation::SimulationPlugin,
    states::AppState,
    ui::UiPlugin,
    visuals::GamePlugin,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                // SVGA resolution (4:3)
                resolution: WindowResolution::new(480, 360),
                resize_constraints: WindowResizeConstraints {
                    min_width: 480.0,
                    min_height: 360.0,
                    ..default()
                },
                fit_canvas_to_parent: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(SimulationPlugin)
        .add_plugins(NetworkPlugin)
        .add_plugins(GamePlugin)
        .add_plugins(UiPlugin)
        .add_systems(Startup, setup)
        .init_state::<AppState>()
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Projection::from(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin {
                min_width: ARENA_WIDTH,
                min_height: ARENA_HEIGHT,
            },
            ..OrthographicProjection::default_2d()
        }),
    ));
}
