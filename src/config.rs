// ——— Game ———
pub const BALL_SPEED: f32 = 100.0;
pub const PADDLE_SPEED: f32 = 150.0;
pub const BALL_RADIUS: f32 = 4.0;
pub const PADDLE_WIDTH: f32 = 6.0;
pub const PADDLE_HEIGHT: f32 = 40.0;
pub const ARENA_WIDTH: f32 = 480.0;
pub const ARENA_HEIGHT: f32 = 360.0;

// ——— Simulation ———
pub const SIMULATION_STEP: f64 = 1.0 / 60.0;
pub const BUFFER_SIZE: usize = 64; // Number of frames to remember input of
pub const MAX_ROLLBACK_FRAMES: u64 = 8;
pub const MAX_DRIFT_CORRECTION: f64 = 0.05; // 5% cap
pub const DRIFT_CORRECTION_FACTOR: f64 = 0.001;

// ——— Network ———
pub const DEFAULT_SIGNALING_URL: &str = "wss://matchbox.slantiz.net/pong?next=2";
pub const FRAMES_TO_SEND: usize = 8;
pub const FRAME_SIZE: usize = 9; // 8 bytes frame number + 1 byte input
pub const HEADER_SIZE: usize = 2; // i16 drift
pub const MESSAGE_SIZE: usize = HEADER_SIZE + FRAMES_TO_SEND * FRAME_SIZE;
