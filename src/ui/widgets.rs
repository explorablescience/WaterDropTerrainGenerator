use wde::prelude::ui::egui;
use crate::{
    core::{
        node::NodeIcon,
        node_parameters::{NParamConstraints, NParamDesc, NParamValue}
    },
    ui::theme::{self, palette}
};

/// A draggable numeric value pill for a node parameter
pub fn param_number_field(
    ui: &mut egui::Ui,
    desc: &NParamDesc,
    color: egui::Color32,
    value: &mut f32
) -> egui::Response {
    let (min, max) = match &desc.constraints {
        Some(NParamConstraints::FloatRange { min, max }) => (*min, *max),
        Some(NParamConstraints::IntRange { min, max }) => (*min as f32, *max as f32),
        _ => (f32::MIN, f32::MAX)
    };
    let is_int = matches!(desc.default, NParamValue::Int(_));

    egui::Frame::new()
        .fill(color.gamma_multiply(0.18))
        .corner_radius(egui::CornerRadius::same(theme::layout::CHIP_ROUNDING))
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.visuals_mut().override_text_color = Some(color);
            let mut drag = egui::DragValue::new(value).range(min..=max);
            drag = if is_int {
                drag.fixed_decimals(0).speed(1.0)
            } else {
                let span = (max - min).abs();
                let speed = if span.is_finite() { (span * 0.002).max(0.001) } else { 0.01 };
                drag.fixed_decimals(2).speed(speed)
            };
            ui.add(drag)
        })
        .inner
}

/// A pill-shaped on/off switch, amber when on.
pub fn toggle_switch(ui: &mut egui::Ui, on: &mut bool) -> egui::Response {
    let desired_size = ui.spacing().interact_size.y * egui::vec2(1.7, 0.85);
    let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *on, "")
    });

    if ui.is_rect_visible(rect) {
        let how_on = ui.ctx().animate_bool(response.id, *on);
        let radius = 0.5 * rect.height();
        let track_fill = if *on {
            if response.hovered() {
                palette::ACCENT_ACTIVE
            } else {
                palette::ACCENT_MUTED
            }
        } else if response.hovered() {
            palette::BG_WIDGET_HOVERED
        } else {
            palette::BG_WIDGET
        };
        let track_stroke =
            egui::Stroke::new(1.0, if *on { track_fill } else { palette::BORDER });
        ui.painter().rect(
            rect,
            radius,
            track_fill,
            track_stroke,
            egui::StrokeKind::Inside
        );

        let knob_radius = radius - 2.5;
        let knob_x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), how_on);
        let knob_center = egui::pos2(knob_x, rect.center().y);
        let knob_color = if *on {
            palette::BG_EXTREME
        } else {
            palette::TEXT_MUTED
        };
        ui.painter()
            .circle(knob_center, knob_radius, knob_color, egui::Stroke::NONE);
    }

    response
}

/// Paints a small flat-design icon representing `icon` inside `rect`, tinted with `color` so it
/// always matches its node's category color.
pub fn paint_node_icon(
    painter: &egui::Painter,
    rect: egui::Rect,
    icon: NodeIcon,
    color: egui::Color32
) {
    let stroke = egui::Stroke::new(1.4, color);
    match icon {
        NodeIcon::Plane => {
            // A flat plateau sitting on a baseline - flat terrain.
            let top = rect.top() + rect.height() * 0.34;
            let base = rect.bottom() - rect.height() * 0.22;
            let left_in = rect.left() + rect.width() * 0.24;
            let right_in = rect.right() - rect.width() * 0.24;
            let points = vec![
                egui::pos2(rect.left(), base),
                egui::pos2(left_in, top),
                egui::pos2(right_in, top),
                egui::pos2(rect.right(), base),
            ];
            painter.add(egui::Shape::convex_polygon(
                points,
                color.gamma_multiply(0.35),
                stroke
            ));
        }
        NodeIcon::Wave => {
            // A single sine-like wave - noise.
            let points: Vec<egui::Pos2> = (0..=16)
                .map(|i| {
                    let t = i as f32 / 16.0;
                    let x = egui::lerp(rect.left()..=rect.right(), t);
                    let y =
                        rect.center().y - (t * std::f32::consts::TAU).sin() * rect.height() * 0.3;
                    egui::pos2(x, y)
                })
                .collect();
            painter.add(egui::Shape::line(points, stroke));
        }
        NodeIcon::Droplet => {
            // A rounded body under a pointed tip - water / erosion. The tip sits directly above
            // the circle's center, so the arc must leave its gap at the top (angle -FRAC_PI_2)
            // rather than to the side, otherwise the two straight edges cross the arc instead of
            // meeting it cleanly.
            let tip = egui::pos2(rect.center().x, rect.top());
            let radius = rect.width() * 0.32;
            let center = egui::pos2(rect.center().x, rect.bottom() - radius);
            let gap_half_angle = 0.5;
            let start = -std::f32::consts::FRAC_PI_2 + gap_half_angle;
            let sweep = std::f32::consts::TAU - 2.0 * gap_half_angle;
            let segments = 20;
            let mut points = vec![tip];
            for i in 0..=segments {
                let t = i as f32 / segments as f32;
                let angle = start + t * sweep;
                points.push(center + radius * egui::vec2(angle.cos(), angle.sin()));
            }
            painter.add(egui::Shape::convex_polygon(
                points,
                color.gamma_multiply(0.35),
                stroke
            ));
        }
    }
}

/// Paints a chevron for a collapsible menu.
pub fn menu_icon(ui: &mut egui::Ui, openness: f32, response: &egui::Response) {
    let color = if response.hovered() {
        palette::TEXT
    } else {
        palette::TEXT_DISABLED
    };

    let rect = response.rect;
    let rect = egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(rect.height()) * 0.5);

    let mut points = vec![rect.left_top(), rect.center_bottom(), rect.right_top()];
    use std::f32::consts::TAU;
    let rotation = egui::emath::Rot2::from_angle(egui::remap(openness, 0.0..=1.0, -TAU / 4.0..=0.0));
    for p in &mut points {
        *p = rect.center() + rotation * (*p - rect.center());
    }

    ui.painter()
        .add(egui::Shape::line(points, egui::Stroke::new(1.5, color)));
}

