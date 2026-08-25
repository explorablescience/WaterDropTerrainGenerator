use bevy::prelude::*;
use wde::prelude::*;

use crate::ui::editor::EditorPanelsPlugin;

mod editor;
mod editor_behavior;
mod panel_graph;
mod panel_properties;
pub mod theme;

pub struct UIPlugin;
impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EditorPanelsPlugin)
            .add_systems(Startup, install_theme);
    }
}

/// Applies the editor's fonts and style to the shared egui context.
fn install_theme(ctx: Res<UIContext>, mut ui_menu: ResMut<UIMenu>) {
    theme::install(&ctx.0);
    ui_menu.set_style(Some(theme::menu_style()));
}
