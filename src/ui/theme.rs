//! Global visual theme for the editor's egui UI.

use wde::prelude::ui::egui;

use crate::{
    core::node::{NodeCategory, NodeMessageSeverity},
    ui::theme::palette::BG_PANEL
};
use egui::{CornerRadius, FontFamily, FontId, Stroke, TextStyle};
use palette::*;

pub mod palette {
    use wde::prelude::ui::egui::Color32;
    pub const UNSET_ERROR: Color32 = Color32::from_rgb(255, 0, 255);

    // Backgrounds, darkest to lightest
    pub const BG_EXTREME: Color32 = Color32::from_rgb(16, 16, 16);
    pub const BG_PANEL: Color32 = Color32::from_rgb(23, 23, 23);
    pub const BG_WINDOW: Color32 = BG_CARD;
    pub const BG_CARD: Color32 = Color32::from_rgb(30, 30, 30);
    pub const BG_WIDGET: Color32 = BG_PANEL;
    pub const BG_WIDGET_HOVERED: Color32 = Color32::from_rgb(50, 50, 50);
    pub const BG_WIDGET_ACTIVE: Color32 = Color32::from_rgb(80, 80, 80);
    pub const BG_WIDGET_OPEN: Color32 = BG_EXTREME;
    pub const BG_GRAPH: Color32 = BG_PANEL;
    /// Shared track color behind every segment of a compact enum selector, so it reads as one piece rather than separate buttons.
    pub const BG_MAIN_COLOR: Color32 = BG_CARD;

    // Borders / separators
    pub const BORDER: Color32 = BG_WIDGET_HOVERED;
    pub const BORDER_STRONG: Color32 = Color32::from_rgb(72, 72, 72);

    // Text
    pub const TEXT: Color32 = Color32::from_rgb(240, 240, 240);
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(210, 210, 210);
    pub const TEXT_DISABLED: Color32 = Color32::from_rgb(110, 110, 110);

    // Accent - the editor's one interactive color, used sparingly (active-tab outline, selected node, hyperlinks, "WDE" header mark).
    pub const ACCENT: Color32 = Color32::from_rgb(128, 186, 255);
    pub const ACCENT_HOVERED: Color32 = Color32::from_rgb(168, 210, 255);
    pub const ACCENT_ACTIVE: Color32 = Color32::from_rgb(92, 152, 232);
    pub const ACCENT_MUTED: Color32 = Color32::from_rgb(70, 95, 135);

    // Semantic - kept in the same muted register as accent/category colors so error/warning/info banners don't clash with the palette.
    pub const HIGHLIGHT_ERROR: Color32 = Color32::from_rgb(214, 112, 112);
    pub const HIGHLIGHT_WARNING: Color32 = Color32::from_rgb(219, 179, 92);
    pub const HIGHLIGHT_INFO: Color32 = Color32::from_rgb(104, 189, 178);

    // Fallback for a selected node whose category can't be resolved.
    pub const NODE_SELECTED: Color32 = ACCENT;

    // Node-graph category colors - one distinct hue per `NodeCategory`, spaced around the
    // wheel and kept clear of ACCENT/HIGHLIGHT_* so a category chip is never mistaken for
    // a selection or message-severity color.
    pub const CATEGORY_GENERATION: Color32 = Color32::from_rgb(132, 188, 118);
    pub const CATEGORY_MODIFICATION: Color32 = Color32::from_rgb(210, 157, 127);
    pub const CATEGORY_SURFACE: Color32 = Color32::from_rgb(116, 180, 156);
    pub const CATEGORY_SIMULATION: Color32 = Color32::from_rgb(154, 161, 208);
    pub const CATEGORY_DATA_EXTRACTION: Color32 = Color32::from_rgb(187, 157, 205);
    pub const CATEGORY_TEXTURING: Color32 = Color32::from_rgb(209, 153, 192);
    pub const CATEGORY_UTILITY: Color32 = Color32::from_rgb(173, 169, 164);
    pub const CATEGORY_EXPORT: Color32 = Color32::from_rgb(199, 202, 119);

    // Neutral default fill for graph pins before a category color is applied.
    pub const PIN_DEFAULT: Color32 = Color32::from_rgb(120, 120, 120);
}

pub mod layout {
    pub const PANEL_BORDER_INSET: f32 = 9.0;
    /// Gap to the top menu bar specifically, not other panels.
    pub const PANEL_TOP_INSET: f32 = 2.0;
    pub const PANEL_BORDER_ROUNDING: u8 = 8;

    /// Graph/Properties only.
    pub const CARD_PADDING: f32 = 4.0;
    /// Shared by the card itself, the properties title bar, and the parameter category sections.
    pub const CARD_ROUNDING: u8 = 6;
    /// E.g. the category badge.
    pub const CHIP_ROUNDING: u8 = 4;

    /// Shared by windows, menus, and interactive widgets (buttons, sliders, text fields).
    pub const WIDGET_ROUNDING: u8 = 3;

    /// Kept small and identical on both sides so the input/output columns read as symmetric.
    pub const NODE_PIN_LABEL_SPACING: f32 = 6.0;
    /// Taller than the pin dot/label content itself, so `egui-snarl` centers it with breathing room.
    pub const NODE_PIN_ROW_HEIGHT: f32 = 28.0;
}

pub mod fonts {
    pub const FONT_REGULAR: &str = "Inter";
    pub const FONT_SEMIBOLD: &str = "Inter-SemiBold";

    pub const FONT_SIZE_SMALL: f32 = 12.0;
    pub const FONT_SIZE_BODY: f32 = 12.0;
    pub const FONT_SIZE_BUTTON: f32 = 12.0;
    pub const FONT_SIZE_HEADING: f32 = 13.0;
    pub const FONT_SIZE_MONOSPACE: f32 = 12.0;
    pub const FONT_SIZE_TITLE: f32 = 15.0;

    pub const FONT_SIZE_NODE_TITLE: f32 = FONT_SIZE_TITLE;
    pub const FONT_SIZE_NODE_PIN: f32 = 12.0;
}

pub fn category_color(category: NodeCategory) -> egui::Color32 {
    match category {
        NodeCategory::Generation => palette::CATEGORY_GENERATION,
        NodeCategory::Modification => palette::CATEGORY_MODIFICATION,
        NodeCategory::Surface => palette::CATEGORY_SURFACE,
        NodeCategory::Simulation => palette::CATEGORY_SIMULATION,
        NodeCategory::DataExtraction => palette::CATEGORY_DATA_EXTRACTION,
        NodeCategory::Texturing => palette::CATEGORY_TEXTURING,
        NodeCategory::Utility => palette::CATEGORY_UTILITY,
        NodeCategory::Export => palette::CATEGORY_EXPORT
    }
}

/// Used for a message banner's border, text, and - at reduced opacity - background tint.
pub fn severity_color(severity: NodeMessageSeverity) -> egui::Color32 {
    match severity {
        NodeMessageSeverity::Error => palette::HIGHLIGHT_ERROR,
        NodeMessageSeverity::Warning => palette::HIGHLIGHT_WARNING,
        NodeMessageSeverity::Info => palette::HIGHLIGHT_INFO
    }
}

pub fn body_font(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Proportional)
}

pub fn heading_font(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Name(fonts::FONT_SEMIBOLD.into()))
}

/// Meant to be called once, at startup, before any UI is drawn.
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

    // Keep egui's bundled fonts as fallback for emoji, symbols, and any glyphs Inter doesn't cover.
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, fonts::FONT_REGULAR.to_owned());

    let mut heading_family = fonts
        .families
        .get(&egui::FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    heading_family.insert(0, fonts::FONT_SEMIBOLD.to_owned());
    fonts.families.insert(
        egui::FontFamily::Name(fonts::FONT_SEMIBOLD.into()),
        heading_family
    );

    fonts
}

fn style() -> egui::Style {
    let mut style = egui::Style {
        text_styles: [
            (
                TextStyle::Small,
                FontId::new(fonts::FONT_SIZE_SMALL, FontFamily::Proportional)
            ),
            (
                TextStyle::Body,
                FontId::new(fonts::FONT_SIZE_BODY, FontFamily::Proportional)
            ),
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

    // Text is not selectable with the mouse; the editor's UI is not meant for copying text out.
    style.interaction.selectable_labels = false;

    let spacing = &mut style.spacing;
    spacing.item_spacing = egui::vec2(17.0, 7.0);
    spacing.window_margin = egui::Margin::same(9);
    spacing.menu_margin = egui::Margin::same(8);
    spacing.button_padding = egui::vec2(18.0, 7.0);
    spacing.indent = 28.0;
    spacing.interact_size = egui::vec2(32.0, 26.0);
    spacing.slider_width = 140.0;
    spacing.combo_width = 120.0;
    spacing.icon_width = 16.0;
    spacing.icon_width_inner = 10.0;
    spacing.icon_spacing = 8.0;
    spacing.scroll.bar_width = 8.0;
    spacing.scroll.floating = true;

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
    visuals.window_corner_radius = CornerRadius::same(layout::WIDGET_ROUNDING);
    visuals.menu_corner_radius = CornerRadius::same(layout::WIDGET_ROUNDING);
    visuals.resize_corner_size = 10.0;
    visuals.indent_has_left_vline = true;
    visuals.collapsing_header_frame = false;
    visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);

    let widgets = &mut visuals.widgets;
    let default_widget = egui::style::WidgetVisuals {
        bg_fill: BG_WIDGET,
        weak_bg_fill: BG_WIDGET,
        bg_stroke: Stroke::new(1.0, BORDER),
        fg_stroke: Stroke::new(1.0, TEXT),
        corner_radius: CornerRadius::same(layout::WIDGET_ROUNDING),
        expansion: 0.0
    };
    widgets.noninteractive = egui::style::WidgetVisuals {
        bg_stroke: Stroke::NONE, // Includes separators, borders of panels, etc.
        fg_stroke: Stroke::new(1.0, TEXT_MUTED),
        ..default_widget
    };
    widgets.inactive = egui::style::WidgetVisuals {
        // `bg_fill` is what egui::Slider paints its rail with (always "inactive" regardless of hover) - keep it a clear mid-grey so it doesn't blend into BG_EXTREME/BG_CARD.
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

    let spacing = &mut style.spacing;
    spacing.item_spacing = egui::vec2(20.0, 8.0);
    spacing.menu_margin = egui::Margin::same(10);
    spacing.button_padding = egui::vec2(20.0, 8.0);

    let visuals = &mut style.visuals;
    let widgets = &mut visuals.widgets;
    let default_widget = egui::style::WidgetVisuals {
        bg_fill: BG_WIDGET,
        weak_bg_fill: BG_WIDGET,
        bg_stroke: Stroke::new(1.0, BORDER),
        fg_stroke: Stroke::new(1.0, TEXT),
        corner_radius: CornerRadius::same(layout::WIDGET_ROUNDING),
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
    widgets.open = egui::style::WidgetVisuals { ..default_widget };

    style
}
