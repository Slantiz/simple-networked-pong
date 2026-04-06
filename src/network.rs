use bevy::prelude::*;
use bevy_matchbox::prelude::*;
use log::warn;

use crate::{
    config::{
        BUFFER_SIZE, FRAME_SIZE, FRAMES_TO_SEND, HEADER_SIZE, MAX_ROLLBACK_FRAMES, MESSAGE_SIZE,
        WIN_SCORE,
    },
    simulation::{
        GameState, InputBuffer, PreSimulate, SnapshotBuffer, step, take_input_and_predict,
    },
    states::{AppState, GameResult},
};

const CHANNEL_ID: usize = 0;

pub struct NetworkPlugin;

impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Connecting), open_socket)
            .add_systems(Update, wait_for_peer.run_if(in_state(AppState::Connecting)))
            .add_systems(OnEnter(AppState::Menu), cleanup_socket)
            .add_systems(OnEnter(AppState::GameOver), cleanup_socket)
            .insert_resource(NetworkState::new())
            .insert_resource(ConnectionError::default())
            .add_systems(OnEnter(AppState::Playing), assign_sides)
            .add_systems(
                PreSimulate,
                (
                    take_input_and_predict,
                    send_input,
                    receive_input,
                    rollback,
                    check_stall,
                )
                    .chain()
                    .run_if(in_state(AppState::Playing)),
            )
            .add_systems(Update, check_win.run_if(in_state(AppState::Playing)));
    }
}

// ——— Resources ———

#[derive(Clone, Copy, Resource)]
pub struct LocalPlayer {
    pub side: Side,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Side {
    Left,
    Right,
}

#[derive(Resource)]
pub struct NetworkState {
    pub rollback_to: Option<u64>,
    pub agreed_frame: Option<u64>,
    pub confirmed_for: [u64; BUFFER_SIZE],
    pub last_confirmed_opponent_input: Option<u8>,
    pub stalled: bool,
    pub local_drift: i16,
    pub remote_drift: i16,
}

impl NetworkState {
    pub fn new() -> Self {
        NetworkState {
            rollback_to: None,
            agreed_frame: None,
            confirmed_for: std::array::from_fn(|_| u64::MAX),
            last_confirmed_opponent_input: None,
            stalled: false,
            local_drift: 0,
            remote_drift: 0,
        }
    }
}

#[derive(Resource)]
pub struct SignalingUrl(pub String);

#[derive(Resource, Default)]
pub struct ConnectionError(pub Option<String>);

// ——— Systems ———

fn open_socket(mut commands: Commands, url: Res<SignalingUrl>) {
    let socket = MatchboxSocket::new_unreliable(url.0.clone());
    commands.insert_resource(socket);
}

fn wait_for_peer(
    mut socket: ResMut<MatchboxSocket>,
    mut commands: Commands,
    mut connection_error: ResMut<ConnectionError>,
) {
    match socket.try_update_peers() {
        Ok(peers) => {
            for (peer, state) in peers {
                info!("{peer}: {state:?}");
                if state == PeerState::Connected {
                    info!("Found peer! Starting game!");
                    commands.set_state(AppState::Playing);
                }
            }
        }
        Err(e) if connection_error.0.is_none() => {
            warn!("Signaling connection failed: {e}");
            connection_error.0 = Some(format!("Signaling connection failed: {e}"));
        }
        Err(_) => {}
    }
}

fn cleanup_socket(
    mut commands: Commands,
    mut state: ResMut<GameState>,
    mut network_state: ResMut<NetworkState>,
) {
    commands.remove_resource::<MatchboxSocket>();
    *state = GameState::new();
    *network_state = NetworkState::new();
}

fn assign_sides(mut commands: Commands, mut socket: ResMut<MatchboxSocket>) {
    let peers: Vec<PeerId> = socket.connected_peers().collect();
    if peers.len() < 1 {
        return;
    }

    let my_id = socket.id().unwrap();
    let peer_id = peers[0];

    // Sort to get a deterministic order both clients agree on
    let mut ids = [my_id, peer_id];
    ids.sort();

    if ids[0] == my_id {
        commands.insert_resource(LocalPlayer { side: Side::Left });
    } else {
        commands.insert_resource(LocalPlayer { side: Side::Right });
    }
}

fn send_input(
    state: Res<GameState>,
    input_buffer: Res<InputBuffer>,
    local_player: Res<LocalPlayer>,
    network_state: Res<NetworkState>,
    mut socket: ResMut<MatchboxSocket>,
) {
    let peers: Vec<PeerId> = socket.connected_peers().collect();
    if peers.is_empty() {
        return;
    }

    let mut bytes = [0u8; MESSAGE_SIZE];

    // Header: pack our local drift
    bytes[0..2].copy_from_slice(&network_state.local_drift.to_le_bytes());

    // Body: pack last frames' inputs and frame numbers
    for i in 0..(FRAMES_TO_SEND as u64) {
        let f = state.frame.saturating_sub(i);
        let idx = (f % BUFFER_SIZE as u64) as usize;
        let input = match local_player.side {
            Side::Left => input_buffer.inputs[idx].left,
            Side::Right => input_buffer.inputs[idx].right,
        };
        let offset = HEADER_SIZE + i as usize * FRAME_SIZE;
        bytes[offset..offset + 8].copy_from_slice(&f.to_le_bytes());
        bytes[offset + 8] = input;
    }

    for peer in peers {
        socket
            .get_channel_mut(CHANNEL_ID)
            .unwrap()
            .send(bytes.into(), peer);
    }
}

fn receive_input(
    state: Res<GameState>,
    mut input_buffer: ResMut<InputBuffer>,
    local_player: Res<LocalPlayer>,
    mut socket: ResMut<MatchboxSocket>,
    mut network_state: ResMut<NetworkState>,
) {
    for (_peer, packet) in socket.get_channel_mut(CHANNEL_ID).unwrap().receive() {
        if packet.len() != MESSAGE_SIZE {
            warn!("Packet rejected (wrong length)!");
            continue;
        }

        // Read header
        let remote_drift = i16::from_le_bytes(packet[0..2].try_into().unwrap());
        network_state.remote_drift = remote_drift;

        // Get sender's latest simulation frame.
        // The sender's latest frame is the highest frame in the packet (first entry).
        let remote_frame =
            u64::from_le_bytes(packet[HEADER_SIZE..HEADER_SIZE + 8].try_into().unwrap());
        network_state.local_drift = (state.frame as i64 - remote_frame as i64)
            .clamp(i16::MIN as i64, i16::MAX as i64) as i16;

        // Fill input buffer with incoming frames
        for i in 0..FRAMES_TO_SEND {
            let offset = HEADER_SIZE + i * FRAME_SIZE;
            let frame = u64::from_le_bytes(packet[offset..offset + 8].try_into().unwrap());
            let input = packet[offset + 8];
            let idx = (frame % BUFFER_SIZE as u64) as usize;

            if frame == network_state.confirmed_for[idx] {
                // We have already agreed on this frame. Skip.
                continue;
            }

            if let Some(agreed_frame) = network_state.agreed_frame
                && frame <= agreed_frame
            {
                // This is an ancient frame. Skip.
                warn!("Ancient frame skipped!");
                continue;
            }

            // Checking for rollback is only necessary for mismatched frames before this frame
            if frame < state.frame {
                // Grab predicted input
                let predicted = match local_player.side {
                    Side::Left => input_buffer.inputs[idx].right,
                    Side::Right => input_buffer.inputs[idx].left,
                };

                // Set rollback if incoming input doesn't match predicted input.
                // If multiple frames require rollback, the earliest frame will used.
                if predicted != input {
                    network_state.rollback_to = Some(match network_state.rollback_to {
                        Some(existing) => existing.min(frame),
                        None => frame,
                    });
                }
            }

            // Overwrite input and confirm it
            match local_player.side {
                Side::Left => input_buffer.inputs[idx].right = input,
                Side::Right => input_buffer.inputs[idx].left = input,
            }
            network_state.confirmed_for[idx] = frame;
        }
    }

    // Advance agreed_frame to a frame where both peers agree on it and all previous ones
    let start = network_state.agreed_frame.map(|f| f + 1).unwrap_or(0);
    for f in start.. {
        let idx = (f % (BUFFER_SIZE as u64)) as usize;
        if network_state.confirmed_for[idx] == f {
            network_state.agreed_frame = Some(f);
            network_state.last_confirmed_opponent_input = match local_player.side {
                Side::Left => Some(input_buffer.inputs[idx].right),
                Side::Right => Some(input_buffer.inputs[idx].left),
            };
        } else {
            break;
        }
    }
}

fn rollback(
    mut state: ResMut<GameState>,
    mut snapshot_buffer: ResMut<SnapshotBuffer>,
    mut input_buffer: ResMut<InputBuffer>,
    local_player: Res<LocalPlayer>,
    mut network_state: ResMut<NetworkState>,
) {
    // Check if major packet loss has occured (entire buffer overwritten
    // by predictions) which may cause the gamestates to be desync.
    // This should ideally trigger some synchronization event,
    // but will warn for now.
    if let Some(frame) = network_state.agreed_frame {
        let gap = state.frame.saturating_sub(frame);
        if gap >= BUFFER_SIZE as u64 {
            warn!("Severe packet loss: rollback gap of {} frames", gap);
            network_state.rollback_to = None;
            return;
        }
    }

    // Return early if there is no need for rollback
    let Some(rollback_frame) = network_state.rollback_to else {
        return;
    };

    info!(
        "Rolling back: {} -> {} ({} frames)",
        state.frame,
        rollback_frame,
        state.frame - rollback_frame
    );
    network_state.rollback_to = None;

    // Load snapshot at rollback point
    let idx = (rollback_frame % BUFFER_SIZE as u64) as usize;
    let Some(snapshot) = snapshot_buffer.states[idx].clone() else {
        warn!("Rolling back to empty (None) snapshot!");
        return;
    };

    // Update predictions for all unconfirmed frames in the rollback range
    // (and the current frame). This uses the latest confirmed opponent input
    // so re-simulation is more accurate and prevents cascading rollbacks.
    if let Some(confirmed_input) = network_state.last_confirmed_opponent_input {
        for f in rollback_frame..=state.frame {
            let idx = (f % BUFFER_SIZE as u64) as usize;
            if network_state.confirmed_for[idx] != f {
                match local_player.side {
                    Side::Left => input_buffer.inputs[idx].right = confirmed_input,
                    Side::Right => input_buffer.inputs[idx].left = confirmed_input,
                }
            }
        }
    }

    // Re-simulate from rollback point to current frame
    let mut resim = snapshot;
    for f in rollback_frame..state.frame {
        let idx = (f % BUFFER_SIZE as u64) as usize;
        let inputs = input_buffer.inputs[idx];
        snapshot_buffer.states[idx] = Some(resim.clone()); // Overwrite snapshots
        resim = step(&resim, inputs.left, inputs.right);
    }

    *state = resim;
}

fn check_win(
    state: Res<GameState>,
    local_player: Res<LocalPlayer>,
    mut commands: Commands,
    mut next: ResMut<NextState<AppState>>,
) {
    let winner = if state.score_left >= WIN_SCORE {
        Some(Side::Left)
    } else if state.score_right >= WIN_SCORE {
        Some(Side::Right)
    } else {
        None
    };

    if let Some(side) = winner {
        let won = local_player.side == side;
        commands.insert_resource(GameResult { won });
        next.set(AppState::GameOver);
    }
}

fn check_stall(state: Res<GameState>, mut network_state: ResMut<NetworkState>) {
    // Stall when agreed_frame is too far behind (so
    // far that unrecoverable packet loss will occur)
    network_state.stalled = match network_state.agreed_frame {
        Some(confirmed) => state.frame.saturating_sub(confirmed) >= MAX_ROLLBACK_FRAMES,
        None => true,
    };
    if network_state.stalled {
        if let Some(agreed_frame) = network_state.agreed_frame {
            warn!(
                "Stalling! frame: {}, agreed {} ({} diff)",
                state.frame,
                agreed_frame,
                state.frame - agreed_frame
            );
        } else {
            warn!("Stalling: agreed frame not set (None)!")
        }
    }
}
