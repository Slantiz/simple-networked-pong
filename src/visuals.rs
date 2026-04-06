use bevy::prelude::*;

use crate::{
    config::{ARENA_HEIGHT, ARENA_WIDTH, BALL_RADIUS, PADDLE_HEIGHT, PADDLE_WIDTH},
    simulation::{GameState, PostSimulate},
    states::AppState,
};

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Playing), spawn_entities)
            .add_systems(
                PostSimulate,
                sync_simulation.run_if(in_state(AppState::Playing)),
            );
    }
}

// ——— Components & Resources ———

#[derive(Component)]
pub struct BallMarker;

#[derive(Component)]
pub struct RightPaddleMarker;

#[derive(Component)]
pub struct LeftPaddleMarker;

// ——— Systems ———

fn spawn_entities(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let paddle_x = ARENA_WIDTH / 2.0 - PADDLE_WIDTH * 2.0;

    // Spawn paddles
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::new(PADDLE_WIDTH, PADDLE_HEIGHT))),
        MeshMaterial2d(materials.add(Color::WHITE)),
        Transform::from_xyz(-paddle_x, 0.0, 0.0),
        LeftPaddleMarker,
    ));
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::new(PADDLE_WIDTH, PADDLE_HEIGHT))),
        MeshMaterial2d(materials.add(Color::WHITE)),
        Transform::from_xyz(paddle_x, 0.0, 0.0),
        RightPaddleMarker,
    ));

    // Spawn arena boundary lines
    let wall_x = ARENA_WIDTH / 2.0 + PADDLE_WIDTH / 2.0;
    let wall_mesh = meshes.add(Rectangle::new(PADDLE_WIDTH, ARENA_HEIGHT * 2.0));
    let wall_material = materials.add(Color::srgb(0.3, 0.3, 0.3));
    commands.spawn((
        Mesh2d(wall_mesh.clone()),
        MeshMaterial2d(wall_material.clone()),
        Transform::from_xyz(-wall_x, 0.0, 0.0),
    ));
    commands.spawn((
        Mesh2d(wall_mesh),
        MeshMaterial2d(wall_material),
        Transform::from_xyz(wall_x, 0.0, 0.0),
    ));

    // Spawn ball
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::new(BALL_RADIUS * 2.0, BALL_RADIUS * 2.0))),
        MeshMaterial2d(materials.add(Color::WHITE)),
        Transform::from_xyz(0.0, 0.0, 0.0),
        BallMarker,
    ));
}

fn sync_simulation(
    state: Res<GameState>,
    mut ball: Query<&mut Transform, With<BallMarker>>,
    mut left_paddle: Query<&mut Transform, (With<LeftPaddleMarker>, Without<BallMarker>)>,
    mut right_paddle: Query<
        &mut Transform,
        (
            With<RightPaddleMarker>,
            Without<BallMarker>,
            Without<LeftPaddleMarker>,
        ),
    >,
) {
    // Move ball
    if let Ok(mut t) = ball.single_mut() {
        t.translation.x = state.ball_pos.x;
        t.translation.y = state.ball_pos.y;
    }

    // Move paddles
    if let Ok(mut t) = left_paddle.single_mut() {
        t.translation.y = state.paddle_left_y;
    }
    if let Ok(mut t) = right_paddle.single_mut() {
        t.translation.y = state.paddle_right_y;
    }
}
