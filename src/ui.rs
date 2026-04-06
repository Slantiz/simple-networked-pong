use bevy::prelude::*;
use bevy_egui::{EguiContexts, prelude::*};

use crate::{
    config::DEFAULT_SIGNALING_URL,
    network::{ConnectionError, SignalingUrl},
    simulation::GameState,
    states::AppState,
};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin::default())
            .add_systems(EguiPrimaryContextPass, load_font.run_if(run_once))
            .insert_resource(MenuInput {
                url: DEFAULT_SIGNALING_URL.to_string(),
            })
            .add_systems(
                EguiPrimaryContextPass,
                (
                    menu_ui.run_if(in_state(AppState::Menu)),
                    connecting_ui.run_if(in_state(AppState::Connecting)),
                    game_ui.run_if(in_state(AppState::Playing)),
                ),
            );
    }
}

#[derive(Resource)]
struct MenuInput {
    url: String,
}

fn load_font(mut contexts: EguiContexts) {
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        "alagard".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/alagard.ttf"
        ))),
    );

    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "alagard".to_owned());

    contexts.ctx_mut().unwrap().set_fonts(fonts);
}

fn game_ui(mut contexts: EguiContexts, state: Res<GameState>) -> Result {
    let ctx = contexts.ctx_mut()?;

    egui::Area::new(egui::Id::new("score_panel"))
        .anchor(egui::Align2::CENTER_TOP, [0.0, 20.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let score_width = 50.0;
                ui.add_sized(
                    [score_width, 48.0],
                    egui::Label::new(
                        egui::RichText::new(state.score_left.to_string())
                            .size(48.0)
                            .color(egui::Color32::WHITE),
                    ),
                );
                ui.add_space(20.0);
                ui.add_sized(
                    [score_width, 48.0],
                    egui::Label::new(
                        egui::RichText::new(state.score_right.to_string())
                            .size(48.0)
                            .color(egui::Color32::WHITE),
                    ),
                );
            });
        });

    Ok(())
}

fn menu_ui(
    mut contexts: EguiContexts,
    mut menu_input: ResMut<MenuInput>,
    mut commands: Commands,
    mut next: ResMut<NextState<AppState>>,
) -> Result {
    let ctx = contexts.ctx_mut()?;

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ctx, |ui| {
            ui.add_space(ui.available_height() / 4.0);

            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("Pong")
                        .size(48.0)
                        .color(egui::Color32::WHITE),
                );

                ui.add_space(ui.available_height() / 4.0);

                ui.label(
                    egui::RichText::new("Signaling Server URL")
                        .size(18.0)
                        .color(egui::Color32::WHITE),
                );
                ui.add_space(4.0);

                let response = ui.add(
                    egui::TextEdit::singleline(&mut menu_input.url)
                        .desired_width(360.0)
                        .font(egui::TextStyle::Monospace),
                );

                ui.add_space(12.0);

                let connect = ui.button(egui::RichText::new("Connect").size(20.0));

                let enter_pressed =
                    response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                if (connect.clicked() || enter_pressed) && !menu_input.url.trim().is_empty() {
                    let url = menu_input.url.trim().to_string();
                    info!("Connecting to {url}");
                    commands.insert_resource(SignalingUrl(url));
                    next.set(AppState::Connecting);
                }
            });
        });

    Ok(())
}

fn connecting_ui(
    mut contexts: EguiContexts,
    url: Res<SignalingUrl>,
    mut connection_error: ResMut<ConnectionError>,
    mut next: ResMut<NextState<AppState>>,
) -> Result {
    let ctx = contexts.ctx_mut()?;

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() / 3.0);

                if let Some(error) = &connection_error.0 {
                    ui.label(
                        egui::RichText::new("Connection failed")
                            .size(18.0)
                            .color(egui::Color32::from_rgb(255, 100, 100)),
                    );
                    ui.add_space(8.0);

                    let mut error_text = error.clone();
                    ui.add(
                        egui::TextEdit::multiline(&mut error_text)
                            .desired_width(400.0)
                            .desired_rows(4)
                            .font(egui::TextStyle::Monospace)
                            .interactive(false),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(
                            "The endpoint is probably not a valid signaling server.",
                        )
                        .size(14.0)
                        .color(egui::Color32::GRAY),
                    );
                    ui.add_space(12.0);

                    if ui.button(egui::RichText::new("Back").size(20.0)).clicked() {
                        connection_error.0 = None;
                        next.set(AppState::Menu);
                    }
                } else {
                    ui.label(
                        egui::RichText::new("Connecting to")
                            .size(18.0)
                            .color(egui::Color32::WHITE),
                    );
                    ui.label(
                        egui::RichText::new(&url.0)
                            .size(14.0)
                            .color(egui::Color32::WHITE)
                            .monospace(),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("Waiting for peer")
                            .size(14.0)
                            .color(egui::Color32::GRAY),
                    );
                    ui.add_space(16.0);
                    ui.label(
                        egui::RichText::new("If no one joins, make sure another player is connecting to the same URL.\nIf both players are stuck, one of you may be behind a symmetric NAT\n(common on cellular, school, and corporate networks).")
                            .size(12.0)
                            .color(egui::Color32::GRAY),
                    );
                    ui.add_space(12.0);
                    if ui
                        .button(egui::RichText::new("Cancel").size(20.0))
                        .clicked()
                    {
                        next.set(AppState::Menu);
                    }
                }
            });
        });

    Ok(())
}
