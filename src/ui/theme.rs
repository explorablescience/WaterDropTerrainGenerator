//! Global visual theme for the editor's egui UI.

use wde::prelude::ui::egui;

use crate::{core::node::NodePortType, ui::theme::palette::BG_PANEL};
use egui::{CornerRadius, FontFamily, FontId, Stroke, TextStyle};
use palette::*;

/// Color palette used throughout the editor
pub mod palette {
    use wde::prelude::ui::egui::Color32;
    pub const ERROR: Color32 = Color32::from_rgb(255, 0, 255);


    // Backgrounds, darkest to lightest
    pub const BG_EXTREME: Color32 = Color32::from_rgb(16, 16, 16);
    pub const BG_PANEL: Color32 = Color32::from_rgb(25, 25, 25);
    pub const BG_WINDOW: Color32 = BG_CARD;
    pub const BG_CARD: Color32 = Color32::from_rgb(34, 34, 34);
    pub const BG_WIDGET: Color32 = BG_PANEL;
    pub const BG_WIDGET_HOVERED: Color32 = Color32::from_rgb(50, 50, 50);
    pub const BG_WIDGET_ACTIVE: Color32 = Color32::from_rgb(80, 80, 80);
    pub const BG_WIDGET_OPEN: Color32 = BG_EXTREME;

    // Borders / separators
    pub const BORDER: Color32 = BG_WIDGET_HOVERED;
    pub const BORDER_STRONG: Color32 = Color32::from_rgb(72, 72, 72);

    // Text
    pub const TEXT: Color32 = Color32::from_rgb(240, 240, 240);
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(210, 210, 210);
    pub const TEXT_DISABLED: Color32 = Color32::from_rgb(110, 110, 110);

    // Accent - the editor's one interactive color. A light, airy blue used sparingly: the
    // active-tab outline, selected node, hyperlinks, and the "WDE" header mark.
    pub const ACCENT: Color32 = Color32::from_rgb(128, 186, 255);
    pub const ACCENT_HOVERED: Color32 = Color32::from_rgb(168, 210, 255);
    pub const ACCENT_ACTIVE: Color32 = Color32::from_rgb(92, 152, 232);
    pub const ACCENT_MUTED: Color32 = Color32::from_rgb(70, 95, 135);

    // Secondary accent - For parameters that have been edited away from their default value, or toggles in their "on"
    pub const MODIFIED: Color32 = Color32::from_rgb(224, 168, 82);
    pub const MODIFIED_HOVERED: Color32 = Color32::from_rgb(240, 190, 110);

    // Semantic
    pub const HIGHLIGHT_ERROR: Color32 = Color32::from_rgb(255, 100, 100);
    pub const HIGHLIGHT_WARNING: Color32 = Color32::from_rgb(255, 200, 0);

    // Neutral highlight used for the currently selected node in the graph editor. Reuses the
    // accent so focus/selection reads as one consistent color across the editor.
    pub const NODE_SELECTED: Color32 = ACCENT;

    // Node-graph socket colors - one distinct hue per port data type.
    pub const PORT_HEIGHT: Color32 = Color32::from_rgb(196, 154, 108);
    pub const PORT_MASK: Color32 = Color32::from_rgb(140, 150, 165);
    pub const PORT_COLOR: Color32 = Color32::from_rgb(216, 118, 150);
    pub const PORT_VECTOR: Color32 = Color32::from_rgb(118, 194, 128);
    pub const PORT_SCALAR: Color32 = Color32::from_rgb(94, 190, 196);
}

/// Layout constants for the editor's per-panel chrome.
pub mod layout {
    /// Gap left between a tile's edge and its own "inside panel" border.
    pub const PANEL_BORDER_INSET: f32 = 9.0;
    /// Gap between the top menu bar and the panels directly below it
    pub const PANEL_TOP_INSET: f32 = 2.0;
    /// Corner radius of each panel's inside border.
    pub const PANEL_BORDER_ROUNDING: u8 = 8;

    /// Padding between a panel's inside border and its content card (Graph/Properties only).
    pub const CARD_PADDING: f32 = 4.0;
    /// Corner radius of the content card.
    pub const CARD_ROUNDING: u8 = 3;
}

pub mod fonts {
    pub const FONT_REGULAR: &str = "Inter";
    pub const FONT_SEMIBOLD: &str = "Inter-SemiBold";

    pub const FONT_SIZE_SMALL: f32 = 13.0;
    pub const FONT_SIZE_BODY: f32 = 13.0;
    pub const FONT_SIZE_BUTTON: f32 = 13.0;
    pub const FONT_SIZE_HEADING: f32 = 15.0;
    pub const FONT_SIZE_MONOSPACE: f32 = 13.0;
    /// Graph-editor node title, sized clearly above body text so nodes read at a glance.
    pub const FONT_SIZE_NODE_TITLE: f32 = 17.0;
}

/// The color used for a node-graph pin/wire of the given data type.
pub fn port_color(dtype: NodePortType) -> egui::Color32 {
    match dtype {
        NodePortType::Height => palette::PORT_HEIGHT,
        NodePortType::Mask => palette::PORT_MASK,
        NodePortType::Color => palette::PORT_COLOR,
        NodePortType::Vector => palette::PORT_VECTOR,
        NodePortType::Scalar => palette::PORT_SCALAR
    }
}

/// Themed widgets that don't come from a stock egui type.
pub mod widgets {
    use wde::prelude::ui::egui;

    use super::palette;

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
                    palette::MODIFIED_HOVERED
                } else {
                    palette::MODIFIED
                }
            } else if response.hovered() {
                palette::BG_WIDGET_HOVERED
            } else {
                palette::BG_WIDGET
            };
            let track_stroke = egui::Stroke::new(1.0, if *on { track_fill } else { palette::BORDER });
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
            let knob_color = if *on { palette::BG_EXTREME } else { palette::TEXT_MUTED };
            ui.painter()
                .circle(knob_center, knob_radius, knob_color, egui::Stroke::NONE);
        }

        response
    }
}

/// Font used for pane headers and other emphasized labels.
pub fn heading_font(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Name(fonts::FONT_SEMIBOLD.into()))
}

/// Installs the editor's fonts and style onto the given egui context. Meant to be called once,
/// at startup, before any UI is drawn.
pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(fonts());
    ctx.set_style(style());
}

fn fonts() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        fonts::FONT_REGULAR.to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../../assets/fonts/Inter-Light.otf"
        )))
    );
    fonts.font_data.insert(
        fonts::FONT_SEMIBOLD.to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../../assets/fonts/Inter-SemiBold.otf"
        )))
    );

    // Use Inter as the primary proportional font, keeping egui's bundled fonts as fallback
    // (emoji, symbols, and any glyphs Inter doesn't cover).
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, fonts::FONT_REGULAR.to_owned());

    // Dedicated family for headings/emphasis, falling back to the same set as Proportional.
    let mut heading_family = fonts
        .families
        .get(&egui::FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    heading_family.insert(0, fonts::FONT_SEMIBOLD.to_owned());
    fonts
        .families
        .insert(egui::FontFamily::Name(fonts::FONT_SEMIBOLD.into()), heading_family);

    fonts
}

fn style() -> egui::Style {
    let mut style = egui::Style {
        text_styles: [
            (
                TextStyle::Small,
                FontId::new(fonts::FONT_SIZE_SMALL, FontFamily::Proportional)
            ),
            (TextStyle::Body, FontId::new(fonts::FONT_SIZE_BODY, FontFamily::Proportional)),
            (
                TextStyle::Button,
                FontId::new(fonts::FONT_SIZE_BUTTON, FontFamily::Proportional)
            ),
            (TextStyle::Heading, heading_font(fonts::FONT_SIZE_HEADING)),
            (
                TextStyle::Monospace,
                FontId::new(fonts::FONT_SIZE_MONOSPACE, FontFamily::Monospace)
            )
        ]
        .into(),
        ..Default::default()
    };

    // Set spacing and sizing for various UI elements.
    let spacing = &mut style.spacing;
    spacing.item_spacing = egui::vec2(17.0, 7.0);
    spacing.window_margin = egui::Margin::same(9);
    spacing.menu_margin = egui::Margin::same(8);
    spacing.button_padding = egui::vec2(18.0, 7.0);
    spacing.indent = 28.0;
    // spacing.interact_size = egui::vec2(28.0, 22.0);
    // spacing.slider_width = 76.0;
    // spacing.combo_width = 90.0;
    // spacing.icon_width = 15.0;
    // spacing.icon_width_inner = 9.0;
    // spacing.icon_spacing = 6.0;
    // spacing.scroll.bar_width = 10.0;
    // spacing.scroll.floating = true;

    // Set the default visuals for all widgets, which will be overridden by specific widget types below.
    let visuals = &mut style.visuals;
    visuals.dark_mode = true;
    visuals.extreme_bg_color = BG_EXTREME;
    visuals.window_fill = BG_PANEL;
    visuals.window_stroke = Stroke::new(1.0, BORDER);
    visuals.selection.bg_fill = BG_WIDGET_HOVERED;
    visuals.selection.stroke = Stroke::new(1.0, TEXT);

    visuals.override_text_color = None;
    visuals.weak_text_color = Some(TEXT_DISABLED);
    visuals.hyperlink_color = ACCENT;
    visuals.faint_bg_color = BG_CARD;
    visuals.code_bg_color = BG_EXTREME;
    visuals.warn_fg_color = HIGHLIGHT_WARNING;
    visuals.error_fg_color = HIGHLIGHT_ERROR;
    visuals.panel_fill = BG_PANEL;
    visuals.window_corner_radius = CornerRadius::same(3);
    visuals.menu_corner_radius = CornerRadius::same(3);
    visuals.resize_corner_size = 10.0;
    visuals.indent_has_left_vline = true;
    visuals.collapsing_header_frame = false;
    visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);

    // Set the default visuals for all widgets, which will be overridden by specific widget types below.
    let widgets = &mut visuals.widgets;
    let default_widget = egui::style::WidgetVisuals {
        bg_fill: BG_WIDGET,
        weak_bg_fill: BG_WIDGET,
        bg_stroke: Stroke::new(1.0, BORDER),
        fg_stroke: Stroke::new(1.0, TEXT),
        corner_radius: CornerRadius::same(3),
        expansion: 0.0
    };
    widgets.noninteractive = egui::style::WidgetVisuals {
        bg_stroke: Stroke::NONE, // Includes separators, borders of panels, etc.
        fg_stroke: Stroke::new(1.0, TEXT_MUTED),
        ..default_widget
    };
    widgets.inactive = egui::style::WidgetVisuals {
        // `bg_fill` is what egui::Slider paints its rail with (always in the "inactive" state,
        // regardless of hover) - keep it a clear mid-grey so it reads against BG_EXTREME/BG_CARD
        // instead of blending into them.
        bg_fill: BG_WIDGET_HOVERED,
        weak_bg_fill: BG_WIDGET,
        fg_stroke: Stroke::new(1.0, TEXT_MUTED),
        bg_stroke: Stroke::NONE,
        ..default_widget
    };
    widgets.hovered = egui::style::WidgetVisuals {
        bg_fill: BG_WIDGET_ACTIVE,
        weak_bg_fill: BG_WIDGET_HOVERED,
        fg_stroke: Stroke::new(1.0, TEXT),
        bg_stroke: Stroke::NONE,
        ..default_widget
    };
    widgets.active = egui::style::WidgetVisuals {
        bg_fill: BG_WIDGET_ACTIVE,
        weak_bg_fill: BG_WIDGET_ACTIVE,
        fg_stroke: Stroke::new(1.0, TEXT),
        bg_stroke: Stroke::NONE,
        ..default_widget
    };
    widgets.open = egui::style::WidgetVisuals {
        weak_bg_fill: BG_WIDGET_OPEN,
        fg_stroke: Stroke::new(1.0, TEXT_MUTED),
        ..default_widget
    };

    style
}

pub fn menu_style() -> egui::Style {
    let mut style = style();

    // Set spacing and sizing for various UI elements.
    let spacing = &mut style.spacing;
    spacing.item_spacing = egui::vec2(20.0, 8.0);
    spacing.menu_margin = egui::Margin::same(10);
    spacing.button_padding = egui::vec2(20.0, 8.0);

    // Set the default visuals for all widgets, which will be overridden by specific widget types below.
    let visuals = &mut style.visuals;
    let widgets = &mut visuals.widgets;
    let default_widget = egui::style::WidgetVisuals {
        bg_fill: BG_WIDGET,
        weak_bg_fill: BG_WIDGET,
        bg_stroke: Stroke::new(1.0, BORDER),
        fg_stroke: Stroke::new(1.0, TEXT),
        corner_radius: CornerRadius::same(3),
        expansion: 0.0
    };
    widgets.noninteractive = egui::style::WidgetVisuals {
        bg_stroke: Stroke::NONE, // Includes separators, borders of panels, etc.
        fg_stroke: Stroke::new(1.0, TEXT_MUTED),
        ..default_widget
    };
    widgets.inactive = egui::style::WidgetVisuals {
        weak_bg_fill: BG_EXTREME,
        bg_stroke: Stroke::NONE,
        fg_stroke: Stroke::new(1.0, TEXT_MUTED),
        ..default_widget
    };
    widgets.hovered = egui::style::WidgetVisuals {
        weak_bg_fill: BG_WIDGET_HOVERED,
        fg_stroke: Stroke::new(1.0, TEXT),
        bg_stroke: Stroke::NONE,
        ..default_widget
    };
    widgets.active = egui::style::WidgetVisuals {
        weak_bg_fill: BG_WIDGET_ACTIVE,
        fg_stroke: Stroke::new(1.0, TEXT),
        bg_stroke: Stroke::NONE,
        ..default_widget
    };
    widgets.open = egui::style::WidgetVisuals {
        ..default_widget
    };

    style
}
