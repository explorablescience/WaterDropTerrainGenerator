use bevy::prelude::*;
use wde::prelude::*;

use crate::ui::{editor::EditorPanelsPlugin, footer::FooterPlugin};

mod editor;
mod editor_behavior;
mod footer;
mod panel_graph;
mod panel_properties;
mod project_io;
pub mod theme;
pub mod widgets;

pub struct UIPlugin;
impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((EditorPanelsPlugin, FooterPlugin))
            .add_systems(Startup, install_theme);
    }
}

/// Applies the editor's fonts and style to the shared egui context, installs the image loaders
/// needed to decode node icons from PNG bytes, and disables WaterDropEngine's own frame-data
/// overlay in favor of the equivalent stats in the editor's own footer (see `ui::footer`).
fn install_theme(
    ctx: Res<UIContext>,
    mut ui_menu: ResMut<UIMenu>,
    mut frame_data_overlay: ResMut<FrameDataOverlayConfig>
) {
    theme::install(&ctx.0);
    egui_extras::install_image_loaders(&ctx.0);
    ui_menu.set_style(Some(theme::menu_style()));
    ui_menu.set_title_color(Some(theme::palette::ACCENT));
    frame_data_overlay.enabled = false;
}
