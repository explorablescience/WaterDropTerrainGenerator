//! "Terrain / Active Tasks" window: every background evaluation task currently in flight on the
//! async compute pool - one per chunk for a `Local` node's parallel batch, or the single whole-domain
//! pass for a `Global` one - alongside the pool's worker thread count.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use bevy::prelude::*;
use bevy::tasks::AsyncComputeTaskPool;
use wde::prelude::{ui::egui, *};

use crate::{
    TerrainInstanceHolder,
    core::{ChunkCoord, TaskScope},
    ui::theme
};

pub fn draw_active_tasks(
    ctx: Res<UIContext>,
    mut ui_menu: ResMut<UIMenu>,
    terrain_graph: Res<TerrainInstanceHolder>
) {
    let open = ui_menu.clicked_mut("Terrain/Active Tasks");
    if !*open {
        return;
    }

    let mut tasks = terrain_graph.read().graph().active_tasks();
    tasks.sort_by_key(|t| t.started_at);
    let worker_threads = AsyncComputeTaskPool::get().thread_num();

    egui::Window::new("Active Tasks")
        .open(open)
        .resizable(true)
        .default_width(460.0)
        .show(&ctx.0, |ui| {
            ui.horizontal(|ui| {
                stat(ui, "Worker Threads", worker_threads.to_string());
                ui.add_space(16.0);
                stat(ui, "Active Tasks", tasks.len().to_string());
            });
            ui.add_space(6.0);
            ui.separator();
            ui.add_space(4.0);

            if tasks.is_empty() {
                ui.label(
                    egui::RichText::new("Idle - nothing processing right now.")
                        .font(theme::body_font(theme::fonts::FONT_SIZE_BODY))
                        .color(theme::palette::TEXT_DISABLED)
                );
                return;
            }

            egui::ScrollArea::vertical()
                .max_height(360.0)
                .show(ui, |ui| {
                    egui::Grid::new("active-tasks-grid")
                        .num_columns(4)
                        .spacing([18.0, 6.0])
                        .striped(true)
                        .show(ui, |ui| {
                            header_cell(ui, "Node");
                            header_cell(ui, "Scope");
                            header_cell(ui, "Started");
                            header_cell(ui, "Elapsed");
                            ui.end_row();

                            for task in &tasks {
                                cell(ui, &task.node_label);
                                cell(ui, &scope_label(task.scope));
                                cell(ui, &format_started(task.started_at));
                                cell(ui, &format_duration(task.started_at.elapsed()));
                                ui.end_row();
                            }
                        });
                });
        });
}

fn scope_label(scope: TaskScope) -> String {
    match scope {
        TaskScope::Local(ChunkCoord(x, y)) => format!("Local - Chunk ({x}, {y})"),
        TaskScope::Global { native_resolution } => format!("Global - {native_resolution}px")
    }
}

/// `Instant` has no wall-clock reading of its own, so the start time is reconstructed by
/// offsetting the current `SystemTime` by how long ago the task started - accurate to within a
/// frame, which is plenty for a diagnostic panel.
fn format_started(started_at: Instant) -> String {
    let wall = SystemTime::now()
        .checked_sub(started_at.elapsed())
        .unwrap_or(UNIX_EPOCH);
    let secs_today = wall
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        % 86400;
    format!(
        "{:02}:{:02}:{:02} UTC",
        secs_today / 3600,
        (secs_today % 3600) / 60,
        secs_today % 60
    )
}

fn format_duration(d: Duration) -> String {
    let secs = d.as_secs_f64();
    if secs >= 60.0 {
        format!("{:.0}m {:02.0}s", (secs / 60.0).floor(), secs % 60.0)
    } else {
        format!("{secs:.1}s")
    }
}

fn header_cell(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .font(theme::body_font(theme::fonts::FONT_SIZE_SMALL))
            .color(theme::palette::TEXT_DISABLED)
    );
}

fn cell(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .font(theme::body_font(theme::fonts::FONT_SIZE_BODY))
            .color(theme::palette::TEXT_MUTED)
    );
}

fn stat(ui: &mut egui::Ui, label: &str, value: String) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(
            egui::RichText::new(label)
                .font(theme::body_font(theme::fonts::FONT_SIZE_BODY))
                .color(theme::palette::TEXT_DISABLED)
        );
        ui.label(
            egui::RichText::new(value)
                .font(theme::body_font(theme::fonts::FONT_SIZE_BODY))
                .color(theme::palette::TEXT_MUTED)
        );
    });
}
