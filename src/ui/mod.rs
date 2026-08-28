use bevy::prelude::*;
use wde::prelude::*;

use crate::ui::{
    editor::EditorPanelsPlugin, footer::FooterPlugin, panel_terrain_settings::draw_terrain_settings
};

mod editor;
mod editor_behavior;
mod footer;
mod panel_graph;
mod panel_properties;
mod panel_terrain_settings;
pub mod theme;
pub mod widgets;

/// Plugin that adds the UI to the app, including the editor panels and footer.
/// It links the ui graph editor to the physical graph, and updates the selected node.
/// It doesn't handle the actual terrain generation, which is done in the `render` plugin.
pub struct UIPlugin;
impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((EditorPanelsPlugin, FooterPlugin))
            .add_systems(Startup, install_theme)
            .add_systems(Update, draw_terrain_settings.after(EditorMenuBarSet));
    }
}

/// Replaces the default WDE theme with our custom theme, and installs image loaders for egui_extras.
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
