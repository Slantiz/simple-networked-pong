use bevy::{
    prelude::*,
    reflect::TypePath,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
    sprite_render::{Material2d, Material2dPlugin},
};

use crate::{
    config::{ARENA_HEIGHT, ARENA_WIDTH, BALL_RADIUS, PADDLE_HEIGHT, PADDLE_WIDTH},
    simulation::{GameState, PostSimulate},
    states::AppState,
};

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Material2dPlugin::<BackgroundMaterial>::default())
            .add_systems(Startup, (spawn_background, load_sounds))
            .add_systems(
                OnEnter(AppState::Playing),
                (reset_sound_state, spawn_entities),
            )
            .add_systems(OnExit(AppState::Playing), despawn_entities)
            .init_resource::<PrevBallVel>()
            .init_resource::<PrevScore>()
            .add_systems(Update, update_background_time)
            .add_systems(
                PostSimulate,
                sync_simulation.run_if(in_state(AppState::Playing)),
            );
    }
}

// ——— Background shader ———

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct BackgroundMaterial {
    #[uniform(0)]
    time: f32,
}

impl Material2d for BackgroundMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/background.wgsl".into()
    }
}

// ——— Components & Resources ———

#[derive(Component)]
pub struct GameEntity;

#[derive(Component)]
pub struct BallMarker;

#[derive(Component)]
pub struct RightPaddleMarker;

#[derive(Component)]
pub struct LeftPaddleMarker;

#[derive(Resource)]
struct SoundEffects {
    bounce: Handle<AudioSource>,
    score: Handle<AudioSource>,
}

#[derive(Resource, Default)]
struct PrevBallVel(Vec2);

#[derive(Resource, Default)]
struct PrevScore(u32, u32);

// ——— Systems ———

fn reset_sound_state(mut prev_vel: ResMut<PrevBallVel>, mut prev_score: ResMut<PrevScore>) {
    *prev_vel = PrevBallVel::default();
    *prev_score = PrevScore::default();
}

fn load_sounds(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(SoundEffects {
        bounce: asset_server.load("sounds/pong.wav"),
        score: asset_server.load("sounds/score.wav"),
    });
}

fn spawn_background(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<BackgroundMaterial>>,
) {
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::new(ARENA_WIDTH * 3.0, ARENA_HEIGHT * 3.0))),
        MeshMaterial2d(materials.add(BackgroundMaterial { time: 0.0 })),
        Transform::from_xyz(0.0, 0.0, -10.0),
    ));
}

fn update_background_time(time: Res<Time>, mut materials: ResMut<Assets<BackgroundMaterial>>) {
    for (_, material) in materials.iter_mut() {
        material.time = time.elapsed_secs();
    }
}

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
        GameEntity,
    ));
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::new(PADDLE_WIDTH, PADDLE_HEIGHT))),
        MeshMaterial2d(materials.add(Color::WHITE)),
        Transform::from_xyz(paddle_x, 0.0, 0.0),
        RightPaddleMarker,
        GameEntity,
    ));

    // Spawn arena boundary lines
    let wall_x = ARENA_WIDTH / 2.0 + PADDLE_WIDTH / 2.0;
    let wall_mesh = meshes.add(Rectangle::new(PADDLE_WIDTH, ARENA_HEIGHT * 2.0));
    let wall_material = materials.add(Color::srgba(1.0, 1.0, 1.0, 0.2));
    commands.spawn((
        Mesh2d(wall_mesh.clone()),
        MeshMaterial2d(wall_material.clone()),
        Transform::from_xyz(-wall_x, 0.0, 0.0),
        GameEntity,
    ));
    commands.spawn((
        Mesh2d(wall_mesh),
        MeshMaterial2d(wall_material),
        Transform::from_xyz(wall_x, 0.0, 0.0),
        GameEntity,
    ));

    // Spawn ball
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::new(BALL_RADIUS * 2.0, BALL_RADIUS * 2.0))),
        MeshMaterial2d(materials.add(Color::WHITE)),
        Transform::from_xyz(0.0, 0.0, 0.0),
        BallMarker,
        GameEntity,
    ));
}

fn despawn_entities(mut commands: Commands, entities: Query<Entity, With<GameEntity>>) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

fn sync_simulation(
    state: Res<GameState>,
    mut prev_vel: ResMut<PrevBallVel>,
    mut prev_score: ResMut<PrevScore>,
    sounds: Res<SoundEffects>,
    mut commands: Commands,
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
    // Detect score change
    let scored = state.score_left != prev_score.0 || state.score_right != prev_score.1;
    if scored {
        commands.spawn(AudioPlayer(sounds.score.clone()));
        prev_score.0 = state.score_left;
        prev_score.1 = state.score_right;
    }

    // Detect bounce: velocity direction changed on either axis (but not on score)
    let vel = state.ball_vel;
    if !scored
        && ((vel.x.signum() != prev_vel.0.x.signum() && prev_vel.0.x != 0.0)
            || (vel.y.signum() != prev_vel.0.y.signum() && prev_vel.0.y != 0.0))
    {
        commands.spawn(AudioPlayer(sounds.bounce.clone()));
    }
    prev_vel.0 = vel;

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
