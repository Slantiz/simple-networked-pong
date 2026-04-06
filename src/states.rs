use bevy::prelude::*;

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    Menu,
    Connecting,
    Playing,
    GameOver,
    Abort,
}

#[derive(Resource)]
pub struct GameResult {
    pub won: bool,
}

#[derive(Resource)]
pub struct AbortReason {
    pub title: String,
    pub subtitle: String,
}
