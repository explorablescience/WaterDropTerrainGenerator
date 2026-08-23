use bevy::prelude::*;

use crate::ui::editor::EditorPanelsPlugin;

mod editor;
mod editor_behavior;
mod panel_graph;
mod panel_properties;

pub struct UIPlugin;
impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EditorPanelsPlugin);
    }
}
