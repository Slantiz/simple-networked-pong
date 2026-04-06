use bevy::prelude::*;

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    Menu,
    Connecting,
    Playing,
    GameOver,
}

#[derive(Resource)]
pub struct GameResult {
    pub won: bool,
}
