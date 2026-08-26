# Vendored patches

## egui-snarl 0.9.0

Vendored from the published crate (unpacked from the crates.io `.crate` archive) with one
upstream bug fixed in `src/ui.rs`, `draw_outputs`.

`draw_outputs` reserves room for each output pin's dot by calling
`Rect::from_min_size(pin_ui.next_widget_position(), vec2(output_spacing, pin_size))`. That
helper's row uses a right-to-left layout, so `next_widget_position()` returns the *right* edge of the row's remaining space, not its left edge — but `Rect::from_min_size` always grows the rect to the right of the point it's given. The reserved rect therefore overflows past the row's right bound by `output_spacing` on every call, which permanently widens the row's `max_rect` (via `advance_cursor_after_rect` -> `expand_to_include_rect`), and that widened bound carries into the next output row's layout, compounding row over row. The visible effect: a node's output pin labels drift further right on every row instead of lining up in a column (most visible on nodes with several outputs; a single-output node only gets a small constant offset, easy to miss). The equivalent input-side code doesn't have this bug — for a left-to-right layout, growing the reserved rect to the right of `next_widget_position()` is correct.

Fix: anchor the reserved rect so it grows to the *left* of that point instead, matching the row's right-to-left layout.

Remove this vendoring (and the `[patch.crates-io]` entry in the workspace `Cargo.toml`) once the fix lands upstream and the `egui-snarl` version is bumped past it.
