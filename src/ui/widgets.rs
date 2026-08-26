use wde::prelude::ui::egui;
use crate::{
    core::{
        node::NodeIcon,
        node_message::NodeMessage,
        node_parameters::{NParamConstraints, NParamDesc, NParamValue}
    }, ui::theme::{self, palette::{self, BG_CARD}}
};

/// Draws a node's error/warning/info feedback as a small stack of banners, meant to sit between
/// a node's title and its parameters: a desaturated tint of the message's severity color for the
/// background, and the full color for the border and text. Each message gets its own rounded
/// rectangle, with a small gap separating it from the one before it.
pub fn node_messages(ui: &mut egui::Ui, messages: &[NodeMessage]) {
    for message in messages {
        ui.add_space(6.0);

        let color = theme::severity_color(message.severity);
        egui::Frame::new()
            .fill(color.gamma_multiply(0.18))
            .stroke(egui::Stroke::new(1.0, color))
            .corner_radius(egui::CornerRadius::same(theme::layout::CHIP_ROUNDING))
            .inner_margin(egui::Margin::symmetric(10, 6))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(&message.text)
                            .font(theme::body_font(theme::fonts::FONT_SIZE_BODY))
                            .color(color)
                    )
                    .wrap()
                );
            });
    }
}

/// A draggable numeric value pill for a node parameter
pub fn slider(
    ui: &mut egui::Ui,
    desc: &NParamDesc,
    color: egui::Color32,
    value: &mut f32
) -> egui::Response {
    let id = ui.make_persistent_id(desc.key);
    let (min, max) = match &desc.constraints {
        Some(NParamConstraints::FloatRange { min, max }) => (*min, *max),
        Some(NParamConstraints::IntRange { min, max }) => (*min as f32, *max as f32),
        _ => (f32::MIN, f32::MAX)
    };
    let is_int = matches!(desc.default, NParamValue::Int(_));
    let width = ui.available_width();
    let response = value_pill(ui, id, width, desc.label, color, value, min, max, is_int);

    ui.add_space(2.0);

    response
}

/// A row of two draggable numeric pills (X/Y) sharing one range/type, for a `Vector2`/`Vector2Int`
/// node parameter. Sits below a small dim header carrying the parameter's own label, matching the
/// category-header style used elsewhere in the properties panel.
pub fn vector2(
    ui: &mut egui::Ui,
    desc: &NParamDesc,
    color: egui::Color32,
    value: &mut (f32, f32)
) -> egui::Response {
    let id = ui.make_persistent_id(desc.key);
    let (min, max) = match &desc.constraints {
        Some(NParamConstraints::Vector2Range { min, max }) => (*min, *max),
        Some(NParamConstraints::Vector2IntRange { min, max }) => {
            ((min.0 as f32, min.1 as f32), (max.0 as f32, max.1 as f32))
        }
        _ => ((f32::MIN, f32::MIN), (f32::MAX, f32::MAX))
    };
    let is_int = matches!(desc.default, NParamValue::Vector2Int(_, _));

    ui.label(
        egui::RichText::new(desc.label)
            .font(theme::body_font(theme::fonts::FONT_SIZE_BODY))
            .color(palette::TEXT_DISABLED)
    );

    let gap = 6.0;
    let half_width = (ui.available_width() - gap) * 0.5;
    let response = ui
        .horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            let x_response =
                value_pill(ui, id.with("x"), half_width, "X", color, &mut value.0, min.0, max.0, is_int);
            let y_response =
                value_pill(ui, id.with("y"), half_width, "Y", color, &mut value.1, min.1, max.1, is_int);
            x_response | y_response
        })
        .inner;

    ui.add_space(2.0);

    response
}

/// Shared painting/interaction logic behind [`slider`] and [`vector2`]: a `width`-wide draggable
/// pill showing `label` on the left and the current value on the right, with double-click-to-type
/// editing. Each pill needs its own persistent `id` so co-located pills (e.g. a vector2's X/Y)
/// don't fight over drag/edit state.
#[allow(clippy::too_many_arguments)]
fn value_pill(
    ui: &mut egui::Ui,
    id: egui::Id,
    width: f32,
    label: &str,
    color: egui::Color32,
    value: &mut f32,
    min: f32,
    max: f32,
    is_int: bool
) -> egui::Response {
    let decimals = if is_int { 0 } else { 2 };
    let has_range = min.is_finite() && max.is_finite();

    let padding = 5.0;
    let desired_size = egui::Vec2::new(width, 20.0);
    let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::click_and_drag());

    // Handle double-click to enter edit mode
    let mut editing = ui.data_mut(|d| d.get_temp::<String>(id));
    if response.double_clicked() {
        editing = Some(format!("{value:.decimals$}"));
        ui.data_mut(|d| d.insert_temp(id, editing.clone().unwrap()));
        ui.memory_mut(|m| m.request_focus(id));
    }
 
    // Handle dragging
    if response.dragged() && editing.is_none() {
        let new_value = if has_range {
            response.interact_pointer_pos().map(|pos| {
                let t = ((pos.x - rect.left()) / rect.width().max(1.0)).clamp(0.0, 1.0);
                min + t * (max - min)
            })
        } else {
            let speed = if is_int { 1.0 } else { 0.01 };
            Some(*value + response.drag_delta().x * speed)
        };
        if let Some(new_value) = new_value {
            let new_value = if is_int { new_value.round() } else { new_value };
            if *value != new_value {
                *value = new_value;
                response.mark_changed();
            }
        }
    }
    if has_range {
        *value = value.clamp(min, max);
    }
 
    // Painting
    let rounding = egui::CornerRadius::same(2);
    let background_color = color.gamma_multiply(0.3);
    let text_color = color.gamma_multiply(1.3);
    let marker_color = color.gamma_multiply(1.5);

    if ui.is_rect_visible(rect) && editing.is_none() {
        let painter = ui.painter();

        painter.rect_filled(rect, rounding, BG_CARD);

        let t = if has_range { (*value - min) / (max - min).max(f32::EPSILON) } else { 0.0 };
        let fill_width = rect.width() * t.clamp(0.0, 1.0);
        if fill_width > 0.0 {
            let fill_rect = egui::Rect::from_min_size(rect.min, egui::Vec2::new(fill_width, rect.height()));
            painter.rect_filled(fill_rect, rounding, background_color);
        }

        painter.rect_stroke(rect, rounding, egui::Stroke::new(4.0, BG_CARD), egui::StrokeKind::Outside);

        // A bright vertical bar marking the exact current-value position on the track.
        if has_range {
            let marker_width = 2.0;
            let marker_x = (rect.left() + fill_width)
                .clamp(rect.left() + marker_width * 0.5, rect.right() - marker_width * 0.5);
            let marker_rect = egui::Rect::from_center_size(
                egui::pos2(marker_x, rect.center().y),
                egui::vec2(marker_width, rect.height() - 4.0),
            );
            painter.rect_filled(marker_rect, egui::CornerRadius::ZERO, marker_color);
        }

        painter.text(
            rect.left_center() + egui::Vec2::new(padding, 0.0),
            egui::Align2::LEFT_CENTER,
            label,
            theme::body_font(theme::fonts::FONT_SIZE_BODY),
            text_color,
        );

        painter.text(
            rect.right_center() - egui::Vec2::new(padding, 0.0),
            egui::Align2::RIGHT_CENTER,
            format!("{value:.decimals$}"),
            theme::body_font(theme::fonts::FONT_SIZE_BODY),
            text_color,
        );
    }
 
    // Editing text field
    if let Some(mut text) = editing.clone() {
        let text_response = ui.put(
            rect,
            egui::TextEdit::singleline(&mut text)
                .font(theme::heading_font(theme::fonts::FONT_SIZE_BODY))
                .horizontal_align(egui::Align::Center)
                .margin(egui::Margin::symmetric(padding as i8, 0)),
        );
 
        if text_response.lost_focus() {
            if let Ok(parsed) = text.trim().parse::<f32>() {
                let parsed = if is_int { parsed.round() } else { parsed };
                *value = parsed.clamp(min, max);
                response.mark_changed();
            }
            ui.data_mut(|d| d.remove::<String>(id));
        } else {
            ui.data_mut(|d| d.insert_temp(id, text));
        }
        text_response.request_focus();
    }

    response
}

/// A full-width text input for a string node parameter, styled to match [`slider`]: a flat
/// BG_CARD pill with the parameter's label baked in on the left and its value editable across
/// the rest of the row.
pub fn text_field(
    ui: &mut egui::Ui,
    label: &str,
    color: egui::Color32,
    value: &mut String,
    char_limit: Option<usize>
) -> egui::Response {
    let padding = 5.0;
    let padding_right = 12.0;
    let rounding = egui::CornerRadius::same(2);
    let text_color = color.gamma_multiply(1.3);
    let font = theme::body_font(theme::fonts::FONT_SIZE_BODY);

    let label_width = ui
        .fonts_mut(|f| f.layout_no_wrap(label.to_owned(), font.clone(), text_color))
        .size()
        .x;

    let desired_size = egui::Vec2::new(ui.available_width(), 20.0);
    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();

        painter.rect_filled(rect, rounding, BG_CARD);
        painter.rect_stroke(rect, rounding, egui::Stroke::new(4.0, BG_CARD), egui::StrokeKind::Outside);

        painter.text(
            rect.left_center() + egui::Vec2::new(padding, 0.0),
            egui::Align2::LEFT_CENTER,
            label,
            font.clone(),
            palette::TEXT_DISABLED,
        );
    }

    // The editable value fills the right side of the pill, centered within that space, with a
    // bit of margin so it never touches the label or the pill's right edge.
    let field_rect = egui::Rect::from_min_max(
        egui::pos2(rect.left() + padding * 2.0 + label_width, rect.top()),
        egui::pos2(rect.right() - padding_right, rect.bottom()),
    );

    let mut edit = egui::TextEdit::singleline(value)
        .font(font)
        .text_color(text_color)
        .horizontal_align(egui::Align::Center)
        .vertical_align(egui::Align::Center)
        .frame(false)
        .margin(egui::Margin { left: 16, right: 0, top: 0, bottom: 0 });
    if let Some(char_limit) = char_limit {
        edit = edit.char_limit(char_limit);
    }
    let response = ui.put(field_rect, edit);

    ui.add_space(2.0);

    response
}

/// A full-width, filled button for a stateless `NParamValue::Action` parameter, styled to match
/// [`slider`]/[`text_field`]: a flat pill with a centered label. `on_click` is invoked once,
/// synchronously, the moment the button is clicked - the caller supplies whatever data the click
/// needs to act on (e.g. the node's resolved inputs and other parameters) already baked into the
/// closure.
pub fn button(
    ui: &mut egui::Ui,
    label: &str,
    color: egui::Color32,
    on_click: impl FnOnce()
) -> egui::Response {
    let rounding = egui::CornerRadius::same(2);
    let desired_size = egui::Vec2::new(ui.available_width(), 20.0);
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        painter.rect_filled(rect, rounding, BG_CARD);

        let fill = if response.hovered() {
            color.gamma_multiply(0.5)
        } else {
            color.gamma_multiply(0.3)
        };
        painter.rect_filled(rect, rounding, fill);

        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            theme::body_font(theme::fonts::FONT_SIZE_BODY),
            color.gamma_multiply(1.3),
        );
    }

    if response.clicked() {
        on_click();
    }

    ui.add_space(2.0);

    response
}

const ENUM_ROW_HEIGHT: f32 = 22.0;
/// Displays a label and a toggle switch
pub fn toggle_switch(
    ui: &mut egui::Ui,
    label: &str,
    color: egui::Color32,
    on: &mut bool
) -> egui::Response {
    enum_label(ui, &format!("  {}", label));

    let switch_size = ui.spacing().interact_size.y * egui::vec2(1.3, 0.6);
    let cell_size = egui::vec2(ui.available_width(), ENUM_ROW_HEIGHT);
    let (cell_rect, cell_response) = ui.allocate_exact_size(cell_size, egui::Sense::hover());
    let rect = egui::Rect::from_center_size(cell_rect.center(), switch_size);

    let switch_id = cell_response.id.with("toggle-switch");
    let mut response = ui.interact(rect, switch_id, egui::Sense::click());
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
                color
            } else {
                color.gamma_multiply(0.7)
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

        let knob_radius = radius - 2.0;
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

/// A parameter row control for an `NParamValue::Enum`: a dim label vertically centered against
/// either a compact segmented control (2-3 options) or a dropdown (4+ options, so the row never
/// grows wider than the panel).
pub fn enum_selector(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    label: &str,
    color: egui::Color32,
    options: &[String],
    current: &mut String,
) -> egui::Response {
    enum_label(ui, &format!("  {}", label));

    if !options.is_empty() && options.len() <= 3 {
        enum_pills(ui, color, options, current)
    } else {
        enum_dropdown(ui, id_salt, options, current)
    }
}

/// Draws `label` sized to exactly its own text width and [`ENUM_ROW_HEIGHT`] tall, vertically
/// centered, so it lines up with the control drawn in the next grid cell regardless of that
/// control's own height.
fn enum_label(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        theme::body_font(theme::fonts::FONT_SIZE_BODY),
        palette::TEXT_DISABLED,
    );
    let desired_size = egui::vec2(galley.size().x, ENUM_ROW_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        let text_pos = rect.left_center() - egui::vec2(0.0, galley.size().y * 0.5);
        ui.painter().galley(text_pos, galley, palette::TEXT_DISABLED);
    }
    response
}

/// A compact segmented control for a small enum: every segment shares one `BG_MAIN_COLOR` track
/// rect, split evenly across the cell's available width so the whole control adapts to the panel
/// instead of overflowing it.
fn enum_pills(
    ui: &mut egui::Ui,
    color: egui::Color32,
    options: &[String],
    current: &mut String,
) -> egui::Response {
    let text_padding_y = 4.0;
    let desired_size = egui::vec2(ui.available_width(), ENUM_ROW_HEIGHT + text_padding_y * 2.0);
    let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    let rounding = egui::CornerRadius::same(theme::layout::WIDGET_ROUNDING);

    if ui.is_rect_visible(rect) {
        ui.painter().rect_filled(rect, rounding, palette::BG_MAIN_COLOR);
    }

    let padding = 3.0;
    let padding_y = 3.0;
    let seg_width = rect.width() / options.len() as f32;
    for (i, option) in options.iter().enumerate() {
        let seg_rect = egui::Rect::from_min_size(
            rect.min + egui::vec2(seg_width * i as f32, 0.0),
            egui::vec2(seg_width, rect.height()),
        );
        let seg_id = response.id.with(("enum-pill", i));
        let seg_response = ui.interact(seg_rect, seg_id, egui::Sense::click());
        let selected = option == current;
        if seg_response.clicked() && !selected {
            *current = option.clone();
            response.mark_changed();
        }

        if ui.is_rect_visible(seg_rect) {
            let fill_rect = seg_rect.shrink2(egui::vec2(padding, padding_y));
            if selected {
                ui.painter().rect_filled(fill_rect, rounding, color.gamma_multiply(0.3));
            } else if seg_response.hovered() {
                ui.painter().rect_filled(fill_rect, rounding, palette::BG_WIDGET_HOVERED);
            }
            let text_color = if selected { color.gamma_multiply(1.3) } else { palette::TEXT_MUTED };
            ui.painter().with_clip_rect(seg_rect).text(
                seg_rect.center(),
                egui::Align2::CENTER_CENTER,
                option,
                theme::body_font(theme::fonts::FONT_SIZE_BODY),
                text_color,
            );
        }

        response |= seg_response;
    }

    response
}

/// A dropdown for a larger enum, stretched to the cell's available width so it stays within the
/// panel instead of using the style's fixed `combo_width`.
fn enum_dropdown(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    options: &[String],
    current: &mut String,
) -> egui::Response {
    let width = ui.available_width();
    ui.allocate_ui_with_layout(
        egui::vec2(width, ENUM_ROW_HEIGHT),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            let mut changed = false;
            let mut response = egui::ComboBox::from_id_salt(id_salt)
                .selected_text(current.clone())
                .width(width)
                .show_ui(ui, |ui| {
                    for option in options {
                        if ui.selectable_value(current, option.clone(), option).clicked() {
                            changed = true;
                        }
                    }
                })
                .response;
            if changed {
                response.mark_changed();
            }
            response
        },
    )
    .inner
}

/// Builds a node's PNG logo as an egui image, tinted with `color` so it always matches its
/// node's category color. The source image is expected to be a white glyph on a transparent
/// background.
pub fn node_icon_image(icon: NodeIcon, color: egui::Color32) -> egui::Image<'static> {
    egui::Image::from_bytes(format!("bytes://{}.png", icon.id), icon.png_bytes).tint(color)
}

/// Paints a node's PNG logo inside `rect`, tinted with `color` so it always matches its node's
/// category color.
pub fn paint_node_icon(ui: &egui::Ui, rect: egui::Rect, icon: NodeIcon, color: egui::Color32) {
    node_icon_image(icon, color).paint_at(ui, rect);
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

