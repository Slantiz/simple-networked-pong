# Simple Networked Pong

This repo contains a simple networked Pong clone written with Bevy/Rust! The game uses a WebRTC peer-to-peer connection to synchronize game state and performs input prediction, rollbacks, drift-based clock synchronization, and stalling. I made this project because I wanted to explore Bevy/Rust as well as how prediction-based deterministic netcode could be implemented. The source code should therefore be taken with a grain of salt ;)

## Prerequisites

- [Rust](https://rustup.rs/) (edition 2024)

## Running

Launch two instances of the game using:

```sh
cargo run
```

To start playing, the peers need to initiate a connection through a signaling server. You can create your own using [matchbox](https://github.com/johanhelsing/matchbox):

```sh
cargo install matchbox_server
matchbox_server
```

Enter the signaling server URL (defaults to `wss://localhost:3536/pong?next=2`) and press Enter to connect. Both clients get matched and the game begins.

## Features

- **Deterministic simulation** at 60fps with fixed timestep.
- **Rollback netcode** — local input is applied instantly, opponent input is predicted (repeat-last-confirmed), and the simulation is rolled back and re-simulated when predictions are wrong.
- **Redundant input transmission** — each packet includes the last 8 frames of input to handle packet loss on the unreliable UDP/WebRTC channel.
- **Drift-based clock synchronization** — peers exchange frame drift values and adjust their simulation speed to stay in sync without adding input delay.
- **Stalling** — simulation halts when too far ahead of the opponent to prevent exceeding the rollback buffer.
- **Cascading rollback prevention** — after a rollback, predictions for all remaining unconfirmed frames are updated using the latest confirmed input.

## Architecture

The game is split into a few modules:

| Module | Purpose |
|---|---|
| `simulation.rs` | Deterministic game state, fixed timestep loop, input encoding, snapshot/rollback buffers |
| `network.rs` | Matchbox socket management, input send/receive, rollback triggering, stall detection, drift sync |
| `visuals.rs` | Bevy ECS rendering — sprites for paddles, ball, arena |
| `ui.rs` | Score display and menus |
| `config.rs` | Central constants for game, simulation, and network tuning |
| `states.rs` | App state machine |

Simulation runs in custom Bevy schedules (`PreSimulate`, `Simulate`, `PostSimulate`) driven by a manual fixed timestep loop. The network systems run in `PreSimulate` in a specific order: `take_input_and_predict -> send_input -> receive_input -> rollback -> check_stall`.

## Limitations

- ⚠️ **NAT traversal** — the game uses WebRTC for peer-to-peer connections, which relies on STUN to punch through NAT. Players behind symmetric NATs (common on cellular networks, universities, corporate networks, and public WiFi) will fail to connect. A TURN relay server would solve this, but is not currently configured.

## Missing Parts

- **Win condition / game over screen** — Currently, the game lasts forever. A win condition is still to be implemented.
- **Disconnect detection and handling** — no recovery or notification if a peer drops mid-game; the game just stalls and waits forever.
- **Desync detection** — peers don't verify that their game states match; severe packet loss is warned about but not reconciled
- **Sound effects** — no sound effects yet :P
