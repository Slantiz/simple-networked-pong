use bevy::{ecs::schedule::ScheduleLabel, prelude::*};

use crate::{
    config::{
        ARENA_HEIGHT, ARENA_WIDTH, BALL_RADIUS, BALL_SPEED, BUFFER_SIZE, DRIFT_CORRECTION_FACTOR,
        MAX_DRIFT_CORRECTION, PADDLE_HEIGHT, PADDLE_SPEED, PADDLE_WIDTH, SIMULATION_STEP,
    },
    network::{LocalPlayer, NetworkState, Side},
    states::AppState,
};

pub struct SimulationPlugin;

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        app.init_schedule(Simulate)
            .add_systems(Update, fixed_simulate.run_if(in_state(AppState::Playing)))
            .insert_resource(FixedTimestepState {
                accumulator: 0.0,
                step: SIMULATION_STEP,
            })
            .insert_resource(GameState::new())
            .insert_resource(InputBuffer::new())
            .insert_resource(SnapshotBuffer::new())
            .add_systems(
                Simulate,
                tick_simulation.run_if(in_state(AppState::Playing)),
            );
    }
}

// ——— Simulation schedule ———

/// Simulation steps happen here.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Simulate;

/// For doing stuff before simulation e.g. taking input.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct PreSimulate;

/// For doing stuff that requries the simulation being done e.g. syncing visuals.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct PostSimulate;

#[derive(Resource)]
struct FixedTimestepState {
    accumulator: f64,
    step: f64,
}

/// Runs at fixed intervals like FixedUpdate,
/// but with peer-relative drift adjustment
fn fixed_simulate(world: &mut World) {
    world.resource_scope(|world, mut state: Mut<FixedTimestepState>| {
        let time = world.resource::<Time>();
        state.accumulator += time.delta_secs_f64();

        let network = world.resource::<NetworkState>();
        let stalled = network.stalled;
        let drift_diff = (network.local_drift - network.remote_drift) as f64;
        let correction = (drift_diff * DRIFT_CORRECTION_FACTOR)
            .clamp(-MAX_DRIFT_CORRECTION, MAX_DRIFT_CORRECTION);
        let effective_step = state.step * (1.0 + correction);

        while state.accumulator >= effective_step {
            world.run_schedule(PreSimulate);
            if !stalled {
                world.run_schedule(Simulate);
                world.run_schedule(PostSimulate);
            }
            state.accumulator -= effective_step;
        }
    });
}

// ——— Encoding & decoding of input ———

// bit 0 = up, bit 1 = down
const INPUT_UP: u8 = 1 << 0;
const INPUT_DOWN: u8 = 1 << 1;

fn encode_input(keys: &ButtonInput<KeyCode>, up: KeyCode, down: KeyCode) -> u8 {
    let mut input = 0u8;
    if keys.pressed(up) {
        input |= INPUT_UP;
    }
    if keys.pressed(down) {
        input |= INPUT_DOWN;
    }
    input
}

fn is_up(input: u8) -> bool {
    input & INPUT_UP != 0
}

fn is_down(input: u8) -> bool {
    input & INPUT_DOWN != 0
}

// ——— Resources ———

/// One frame's worth of inputs for both players.
/// This drives the simulation.
#[derive(Clone, Copy, Default)]
pub struct FrameInputs {
    pub left: u8,
    pub right: u8,
}

/// Per-frame inputs
#[derive(Resource)]
pub struct InputBuffer {
    pub inputs: [FrameInputs; BUFFER_SIZE],
}

impl InputBuffer {
    fn new() -> Self {
        InputBuffer {
            inputs: std::array::from_fn(|_| FrameInputs::default()),
        }
    }
}

/// Per-frame game state required for rollback
#[derive(Resource)]
pub struct SnapshotBuffer {
    pub states: [Option<GameState>; BUFFER_SIZE],
}

impl SnapshotBuffer {
    fn new() -> Self {
        SnapshotBuffer {
            states: std::array::from_fn(|_| None),
        }
    }
}

/// Stores all simulation state that must be identical for both peers.
#[derive(Clone, Resource, Default)]
pub struct GameState {
    pub ball_pos: Vec2,
    pub ball_vel: Vec2,
    pub paddle_left_y: f32,
    pub paddle_right_y: f32,
    pub score_left: u32,
    pub score_right: u32,
    pub frame: u64,
}

impl GameState {
    fn new() -> Self {
        Self {
            ball_pos: Vec2::ZERO,
            ball_vel: Vec2::new(BALL_SPEED, BALL_SPEED),
            paddle_left_y: 0.0,
            paddle_right_y: 0.0,
            score_left: 0,
            score_right: 0,
            frame: 0,
        }
    }
}

// ——— Systems ———

pub fn take_input_and_predict(
    mut input_buffer: ResMut<InputBuffer>,
    state: Res<GameState>,
    keys: Res<ButtonInput<KeyCode>>,
    local_player: Res<LocalPlayer>,
    network_state: Res<NetworkState>,
) {
    // Don't register or predict input when stalling
    if network_state.stalled {
        return;
    }

    let idx = (state.frame % (BUFFER_SIZE as u64)) as usize;

    // Take local player's input this frame
    let input = encode_input(&keys, KeyCode::KeyW, KeyCode::KeyS)
        | encode_input(&keys, KeyCode::ArrowUp, KeyCode::ArrowDown);

    match local_player.side {
        Side::Left => input_buffer.inputs[idx].left = input,
        Side::Right => input_buffer.inputs[idx].right = input,
    }

    // Predict opponent's input this frame.
    // If the input is already confirmed, skip.
    if network_state.confirmed_for[idx] == state.frame {
        info!("Input: frame arealdy confirmed; no need to predict");
        return;
    }

    let prediction = match network_state.last_confirmed_opponent_input {
        Some(input) => input,
        None => 0u8,
    };
    match local_player.side {
        Side::Left => input_buffer.inputs[idx].right = prediction,
        Side::Right => input_buffer.inputs[idx].left = prediction,
    }
}

pub fn tick_simulation(
    mut state: ResMut<GameState>,
    mut snapshot_buffer: ResMut<SnapshotBuffer>,
    input_buffer: Res<InputBuffer>,
) {
    let idx = state.frame as usize % BUFFER_SIZE;

    // Snapshot before stepping
    snapshot_buffer.states[idx] = Some(state.clone());

    let inputs = input_buffer.inputs[idx];
    *state = step(&state, inputs.left, inputs.right);
}

// Simulate exactly one frame forward (pure function, no side effects)
pub fn step(state: &GameState, input_left: u8, input_right: u8) -> GameState {
    let mut next = state.clone();
    let dt = SIMULATION_STEP as f32;
    next.frame += 1;

    // Move paddles
    let half_arena = ARENA_HEIGHT / 2.0;
    let half_paddle = PADDLE_HEIGHT / 2.0;

    if is_up(input_left) {
        next.paddle_left_y += PADDLE_SPEED * dt;
    }
    if is_down(input_left) {
        next.paddle_left_y -= PADDLE_SPEED * dt;
    }
    next.paddle_left_y = next
        .paddle_left_y
        .clamp(-half_arena + half_paddle, half_arena - half_paddle);

    if is_up(input_right) {
        next.paddle_right_y += PADDLE_SPEED * dt;
    }
    if is_down(input_right) {
        next.paddle_right_y -= PADDLE_SPEED * dt;
    }
    next.paddle_right_y = next
        .paddle_right_y
        .clamp(-half_arena + half_paddle, half_arena - half_paddle);

    // Move ball
    next.ball_pos += next.ball_vel * dt;

    // top/bottom wall bounce
    let half_height = ARENA_HEIGHT / 2.0 - BALL_RADIUS;
    if next.ball_pos.y > half_height {
        next.ball_pos.y = half_height;
        next.ball_vel.y = -next.ball_vel.y.abs();
    }
    if next.ball_pos.y < -half_height {
        next.ball_pos.y = -half_height;
        next.ball_vel.y = next.ball_vel.y.abs();
    }

    // paddle collision
    let paddle_x = ARENA_WIDTH / 2.0 - PADDLE_WIDTH * 2.0;
    let hit_left = next.ball_vel.x < 0.0
        && next.ball_pos.x - BALL_RADIUS < -paddle_x + PADDLE_WIDTH
        && next.ball_pos.x + BALL_RADIUS > -paddle_x
        && (next.ball_pos.y - next.paddle_left_y).abs() < half_paddle + BALL_RADIUS;

    let hit_right = next.ball_vel.x > 0.0
        && next.ball_pos.x + BALL_RADIUS > paddle_x - PADDLE_WIDTH
        && next.ball_pos.x - BALL_RADIUS < paddle_x
        && (next.ball_pos.y - next.paddle_right_y).abs() < half_paddle + BALL_RADIUS;

    if hit_left {
        next.ball_vel.x = next.ball_vel.x.abs();
    }
    if hit_right {
        next.ball_vel.x = -next.ball_vel.x.abs();
    }

    // scoring
    if next.ball_pos.x < -ARENA_WIDTH / 2.0 {
        next.score_right += 1;
        next.ball_pos = Vec2::ZERO;
        next.ball_vel = Vec2::new(BALL_SPEED, BALL_SPEED);
    }
    if next.ball_pos.x > ARENA_WIDTH / 2.0 {
        next.score_left += 1;
        next.ball_pos = Vec2::ZERO;
        next.ball_vel = Vec2::new(-BALL_SPEED, BALL_SPEED);
    }

    next
}
