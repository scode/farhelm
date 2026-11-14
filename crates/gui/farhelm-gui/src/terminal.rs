//! The terminal's renderer: one session terminal's committed grid, painted as a single element.
//!
//! The supervisor emulates the terminal; the GUI only draws the grid protocol's committed screen
//! (`farhelm-grid-model`). [`TerminalView`] owns that model, because a terminal's grid is bulk data streamed for this
//! one attachment, not shared store state. It is deliberately uncached and notifies itself: a terminal's inputs are its
//! whole grid, which cannot go through the cache wrapper's computed-inputs comparison, and the terminal is the one
//! view that must redraw on every output it shows (SPEC_impl.md, "The store, and how views redraw"). Every change goes
//! through a method here that decides whether the screen changed and notifies, so a missed notification cannot leave a
//! stale grid on screen.
//!
//! Ported from the GPUI prototype's renderer (`crates/light/src/terminal` in the prototype repository), whose design
//! the port review kept: one custom element with no per-cell layout nodes, a shaped-row cache that survives scrolling,
//! printable ASCII shaped as one batch and split at its known one-byte cell boundaries, every other run kept whole so
//! combining marks and emoji sequences stay together, and ink clipped to the cell box the wire assigns so a fallback
//! glyph never moves later columns. Selection, link and scrollback views are not here yet; they come with the
//! terminal area's later work.

mod style;
#[cfg(test)]
mod tests;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use farhelm_grid_model::{
    ApplyEffects, Cursor, CursorShape, Damage, GridError, GridModel, GridOutput, GridRow,
    TerminalState, display_text,
};
use farhelm_input_mapping::pointer::Geometry;
use gpui::{
    App, Bounds, ContentMask, Context, Element, ElementId, FontFeatures, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, PathBuilder, Pixels, Render, ShapedLine, Size,
    TextAlign, TextRun, Window, fill, font, point, px, size,
};

use crate::assets::TERMINAL_FONT;
use crate::render_count;
use crate::theme::{TERMINAL, TerminalColor};
use style::{ResolvedStyle, Underline};

/// The terminal's text size in whole points, within the old app's range.
///
/// Only whole sizes from 9 to 28 ever reach the text system (SPEC_impl.md's glyph-atlas rule): GPUI never evicts a
/// rasterized glyph, so each distinct size a terminal is drawn at stays in memory for the app's life, and a bounded set
/// of sizes bounds what zooming adds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextSize(u8);

impl TextSize {
    /// The smallest size, in points.
    pub const MIN: u8 = 9;
    /// The largest size, in points.
    pub const MAX: u8 = 28;
    /// The old app's default terminal text size.
    pub const DEFAULT: TextSize = TextSize(14);

    /// A size in points, clamped into the supported range.
    pub fn new(points: u8) -> Self {
        Self(points.clamp(Self::MIN, Self::MAX))
    }

    /// The size in points.
    pub fn points(self) -> u8 {
        self.0
    }
}

/// The terminal font's measured character in logical pixels, and the font size its glyphs are shaped at.
///
/// This is the old app's measurement (`measureChar` in grid-terminal.js at the reference commit): the advance of "W"
/// and the font's ascent plus descent, both unrounded. It is not yet the cell: `CellMetrics::cell_size` rounds it the
/// way the old app did, which depends on the grid's size and the display's pixel ratio, so the element works the cell
/// out each time it lays out. The only way to make one is [`CellMetrics::measure`], so a font size outside
/// [`TextSize`]'s whole points can never reach the text system.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellMetrics {
    char_width: f32,
    char_height: f32,
    font_size: f32,
}

impl CellMetrics {
    /// A measurement from explicit values, or `None` when any is not a positive finite number: a zero or NaN cell
    /// would reach layout and clipping, and a panic here would abort inside a platform callback. Private, so the
    /// font size always comes from a [`TextSize`] (tests in this module use it with made-up numbers).
    fn new(char_width: f32, char_height: f32, font_size: f32) -> Option<Self> {
        [char_width, char_height, font_size]
            .into_iter()
            .all(|v| v.is_finite() && v > 0.0)
            .then_some(Self {
                char_width,
                char_height,
                font_size,
            })
    }

    /// Measure the terminal font at `size`, as the old app measured its canvas text: the advance of "W", and the
    /// font's ascent plus descent (line height 1, no leading). `None` when the text system cannot measure the font,
    /// which the caller treats as "keep the previous metrics".
    pub fn measure(size: TextSize, cx: &App) -> Option<Self> {
        let font_size = px(f32::from(size.points()));
        let text_system = cx.text_system();
        let id = text_system.resolve_font(&font(TERMINAL_FONT));
        let advance = text_system.advance(id, font_size, 'W').ok()?.width;
        // The platforms disagree on the descent's sign: the Linux text system reports it as a negative offset from
        // the baseline, while GPUI's own baseline arithmetic treats it as a distance. Its size is what matters here.
        let height = text_system.ascent(id, font_size) + text_system.descent(id, font_size).abs();
        Self::new(f32::from(advance), f32::from(height), f32::from(font_size))
    }

    /// The cell, in logical pixels, that a `cols` by `rows` grid is drawn at on a display of `scale` device pixels per
    /// logical pixel: the character rounded as the old app's `cellFor` did ([`Geometry::for_character`]). Zero when
    /// the grid has no size yet, which draws nothing.
    fn cell_size(self, cols: u16, rows: u16, scale: f32) -> Size<f32> {
        Geometry::for_character(
            f64::from(self.char_width),
            f64::from(self.char_height),
            cols,
            rows,
            f64::from(scale),
        )
        .map_or(Size::default(), |g| {
            size(g.cell_width as f32, g.cell_height as f32)
        })
    }

    /// The character's height rounded up to whole device pixels: the height `cell_size` starts from before it spreads
    /// a grid's rounding over the rows, and the step between two rows' glyph baselines ([`Baselines`]). Computed in
    /// `f64` exactly as [`Geometry::for_character`] does, so the two never disagree by a pixel.
    fn device_row(self, scale: f32) -> f32 {
        (f64::from(self.char_height) * f64::from(scale)).ceil() as f32
    }

    /// The font size glyphs are shaped at.
    fn font_size(self) -> Pixels {
        px(self.font_size)
    }
}

/// Where the rows' glyphs go vertically: each row's baseline on a whole device pixel, exactly
/// [`CellMetrics::device_row`] device pixels below the row above's, starting where GPUI would put the first row's.
///
/// The cell height is the old app's (`CellMetrics::cell_size`), which keeps the whole grid a whole number of logical
/// pixels and so is usually not a whole number of device pixels: 167/9 logical pixels at 14 points for a 9-row grid on
/// a 2x display, which is 37.11 device pixels. GPUI rounds every glyph's baseline to a device pixel on its own, so
/// glyphs placed at their row's top land 37 or 38 device pixels below the row above, depending on where the
/// accumulated fraction crosses a half. A full block is 37 device pixels tall at that size (measured in the styles
/// golden), so a 38 step leaves a one-device-pixel stripe of background between two rows, once every few rows. Snapping
/// each row on its own to its rounded top does not help: the 38 step only moves to another pair of rows. Stepping by
/// the character's own height in device pixels removes that seam.
///
/// The price is that glyphs drift against their cells by the fraction the grid spreads over its rows. That stays under
/// half a logical pixel at the last row, because the grid's total height is the characters' whole device pixels
/// rounded to a whole logical pixel ([`Geometry::for_character`]); at scale 1 there is no drift at all. Backgrounds,
/// decorations, clipping and the cursor's box stay on the cells.
///
/// NOTE: This closes only the seam between rows, and stacked full blocks (U+2588) still stripe at most text sizes. On
/// a 2x display they meet only at 14 (the default), 17, 20 and 28 points. At the other sizes measured (9 to 13, 15,
/// 16, 18, 19, 22, 24 and 26) the font's block glyph is shorter than the cell, so a stripe of background, faint or a
/// whole device pixel, shows below every row, with or without this. That was measured with a probe scene in the
/// goldens harness; 1x displays were not measured. Box-drawing verticals met at every size measured, because their
/// glyphs reach past their box. The old app drew block elements as font glyphs too, so this is parity with it, not a
/// regression. Drawing the block elements (U+2580 to U+259F) as rectangles filling the cell, as many terminals do,
/// would close the stripe; that improvement awaits the maintainer's decision.
#[derive(Clone, Copy, Debug)]
struct Baselines {
    /// The grid's top edge, in logical pixels.
    top: f32,
    /// The cell height in logical pixels: the line height GPUI centers each glyph row in.
    cell_height: f32,
    /// The step between consecutive rows' baselines, in whole device pixels.
    device_row: f32,
    /// Device pixels per logical pixel.
    scale: f32,
}

impl Baselines {
    /// The `y` to hand GPUI as the origin of a line in grid row `row` with the given ascent and descent (the shaped
    /// line's own), so GPUI's baseline arithmetic in `paint_line` (the line height minus ascent and descent, halved,
    /// plus the ascent) lands the baseline on this row's whole device pixel. The same metrics from the same font give
    /// every row the same offset, so the step between rows is exactly `device_row`; a fallback font's run keeps its
    /// own centering, and its rows step the same way.
    fn origin_y(self, row: usize, ascent: f32, descent: f32) -> f32 {
        let offset = (self.cell_height - ascent - descent) / 2. + ascent;
        let first = ((self.top + offset) * self.scale).round();
        (first + row as f32 * self.device_row) / self.scale - offset
    }
}

/// Whether the terminal has focus and where its cursor blink is. An unfocused terminal draws its cursor outlined,
/// whatever the blink phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CursorPhase {
    pub focused: bool,
    pub on: bool,
}

impl Default for CursorPhase {
    fn default() -> Self {
        Self {
            focused: false,
            on: true,
        }
    }
}

/// One screen row and its shaped glyphs, kept until the row's contents, colors or geometry change. `Rc` keeps the
/// cache on GPUI's foreground thread.
struct CachedRow {
    row: GridRow,
    shaped: RefCell<Option<Vec<PaintRun>>>,
}

/// One wire run: its resolved style shared by the glyph pieces it was split into.
struct PaintRun {
    col: u16,
    width: u16,
    style: ResolvedStyle,
    pieces: Vec<PaintPiece>,
    plain_flow: bool,
}

/// A shaped fragment with its wire-relative column and box width, kept apart from the font's advance so a glyph wider
/// or narrower than its cells never moves the cells after it.
struct PaintPiece {
    col: u16,
    width: u16,
    line: ShapedLine,
}

/// One terminal's committed screen, and the view that paints it.
pub struct TerminalView {
    model: GridModel,
    rows: Vec<Rc<CachedRow>>,
    cols: u16,
    cursor: Option<Cursor>,
    visible: bool,
    state: Option<TerminalState>,
    metrics: CellMetrics,
    phase: CursorPhase,
}

impl TerminalView {
    /// A view for a new attachment: an empty model behind its catch-up placeholder. A reconnect makes a new view, as a
    /// new attachment must start a new model.
    pub fn new(metrics: CellMetrics) -> Self {
        Self {
            model: GridModel::new(),
            rows: Vec::new(),
            cols: 0,
            cursor: None,
            visible: false,
            state: None,
            metrics,
            phase: CursorPhase::default(),
        }
    }

    /// Apply one delivery of decoded grid output, then redraw if what the view shows changed.
    ///
    /// The owner hands over a bounded batch per turn; damage is drained once per batch, so a flood redraws once per
    /// delivery, not once per message. A message the protocol refuses ends the batch with its error and leaves the
    /// model as it stood before that message; the owner must end the attachment (the wire protocol's rule). Messages
    /// before it in the batch stay applied and are shown. Completed OSC 52 writes are merged into the returned effects
    /// (the last completed clipboard text wins), for the owner to act on.
    ///
    /// The effects come back beside the result, not inside its `Ok`, because the model hands out a completed write only
    /// once: a write that completed before a refused message is still the user's copy, and the owner must deliver it
    /// even as it ends the attachment.
    #[must_use = "completed OSC 52 writes are returned only once, even when a later message is refused"]
    pub fn apply(
        &mut self,
        messages: impl IntoIterator<Item = GridOutput>,
        cx: &mut Context<Self>,
    ) -> (ApplyEffects, Result<(), GridError>) {
        let mut merged = ApplyEffects::default();
        let mut result = Ok(());
        for message in messages {
            match self.model.apply_output(message) {
                Ok(effects) => {
                    merged.clipboard_written |= effects.clipboard_written;
                    if effects.clipboard_text.is_some() {
                        merged.clipboard_text = effects.clipboard_text;
                    }
                }
                Err(error) => {
                    result = Err(error);
                    break;
                }
            }
        }
        self.sync(cx);
        (merged, result)
    }

    /// Release the catch-up placeholder once the helm says the replay is complete; refuses an incomplete first screen.
    pub fn mark_replay_complete(&mut self, cx: &mut Context<Self>) -> Result<(), GridError> {
        let result = self.model.mark_replay_complete();
        self.sync(cx);
        result
    }

    /// Change focus or the blink phase; repaints the cursor without reshaping anything.
    pub fn set_cursor_phase(&mut self, phase: CursorPhase, cx: &mut Context<Self>) {
        if self.phase != phase {
            self.phase = phase;
            cx.notify();
        }
    }

    /// The visible screen's text, one line per row with trailing blanks trimmed, for the test bridge's `terminal_text`
    /// probe; `None` behind the catch-up placeholder. Read from the paint cache, not the model, on purpose: a damage
    /// delivery the view missed must stay visible to automation instead of being hidden by the newer model.
    pub fn terminal_text(&self) -> Option<String> {
        self.visible.then(|| {
            self.rows
                .iter()
                .map(|r| r.row.text(0, self.cols).trim_end_matches(' ').to_owned())
                .collect::<Vec<_>>()
                .join("\n")
        })
    }

    /// Drain the model's damage into the row cache and notify when anything shown changed.
    fn sync(&mut self, cx: &mut Context<Self>) {
        let damage = self.model.take_damage();
        if self.update_grid(damage) {
            cx.notify();
        }
    }

    /// Bring the row cache up to the model. Returns whether a redraw is needed; a row rewritten with equal contents
    /// keeps its shaped glyphs. `Damage::shift` moves the cache before the damaged rows, which name final positions.
    fn update_grid(&mut self, damage: Damage) -> bool {
        let model = &self.model;
        let old_visible = self.visible;
        let old_cursor = self.cursor;
        let colors_changed = style::defaults(self.state) != style::defaults(model.state());
        self.visible = model.is_visible();
        self.cursor = model.cursor();
        self.state = model.state();
        let full = damage.all
            || damage.size
            || self.cols != model.size().0
            || self.rows.len() != model.screen().len();
        self.cols = model.size().0;
        let mut changed =
            full || colors_changed || old_visible != self.visible || old_cursor != self.cursor;
        if full {
            self.rows = model.screen().iter().cloned().map(cache_row).collect();
        } else {
            if damage.shift > 0 {
                self.rows
                    .drain(..usize::from(damage.shift).min(self.rows.len()));
                self.rows
                    .resize_with(model.screen().len(), || cache_row(GridRow::default()));
                changed = true;
            }
            for y in damage.rows {
                let i = usize::from(y);
                if let (Some(old), Some(new)) = (self.rows.get(i), model.screen().get(i))
                    && old.row != *new
                {
                    self.rows[i] = cache_row(new.clone());
                    changed = true;
                }
            }
        }
        if colors_changed {
            self.invalidate_shapes();
        }
        changed
    }

    /// Drop every row's shaped glyphs: the default colors changed, and glyph colors are part of the shape.
    fn invalidate_shapes(&self) {
        for row in &self.rows {
            row.shaped.replace(None);
        }
    }
}

/// A fresh cache entry for a row whose glyphs are not shaped yet.
fn cache_row(row: GridRow) -> Rc<CachedRow> {
    Rc::new(CachedRow {
        row,
        shaped: RefCell::new(None),
    })
}

impl Render for TerminalView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render_count::record("terminal", cx);
        TerminalElement {
            rows: self.rows.clone(),
            cols: self.cols,
            cursor: self.cursor,
            visible: self.visible,
            state: self.state,
            metrics: self.metrics,
            phase: self.phase,
            cell: Size::default(),
        }
    }
}

/// The single element a terminal draws as: no per-cell layout nodes and no event handlers.
struct TerminalElement {
    rows: Vec<Rc<CachedRow>>,
    cols: u16,
    cursor: Option<Cursor>,
    visible: bool,
    state: Option<TerminalState>,
    metrics: CellMetrics,
    phase: CursorPhase,
    /// The cell in logical pixels, worked out from `metrics` when the element lays out, where the window's scale
    /// factor is known; paint reads it from here.
    cell: Size<f32>,
}

impl TerminalElement {
    /// The glyph rows' placement for this frame's cell, inside a grid whose top edge is `top`.
    fn baselines(&self, top: Pixels, window: &Window) -> Baselines {
        let scale = window.scale_factor();
        Baselines {
            top: f32::from(top),
            cell_height: self.cell.height,
            device_row: self.metrics.device_row(scale),
            scale,
        }
    }

    /// Shape one row: printable ASCII as a batch split per cell, everything else whole. Text is drawn at the wire's
    /// widths, never the font's advances, so overflowing fallback ink is clipped rather than moving later cells.
    fn shape(&self, row: &CachedRow, window: &mut Window) -> Vec<PaintRun> {
        row.row
            .runs
            .iter()
            .map(|run| {
                let style = style::resolve(run.style, self.state);
                // Controls print no ink, and a newline cannot reach GPUI's single-line shaper; the run still occupies
                // its cells, and copy and automation keep the received text.
                let text: String = display_text(&run.text)
                    .chars()
                    .filter(|c| !c.is_control())
                    .collect();
                let text_len = text.len();
                let text_runs = [TextRun {
                    len: text_len,
                    font: terminal_face(style),
                    color: style.fg.hsla(style.alpha),
                    ..Default::default()
                }];
                let line = window.text_system().shape_line(
                    text.into(),
                    self.metrics.font_size(),
                    &text_runs,
                    None,
                );
                let pieces = if run.is_plain_flow() {
                    let mut cursor = line.cursor();
                    (0..run.width)
                        .map(|col| PaintPiece {
                            col,
                            width: 1,
                            line: cursor.take_until(usize::from(col) + 1),
                        })
                        .collect()
                } else {
                    vec![PaintPiece {
                        col: 0,
                        width: run.width,
                        line,
                    }]
                };
                PaintRun {
                    col: run.col,
                    width: run.width,
                    style,
                    pieces,
                    plain_flow: run.is_plain_flow(),
                }
            })
            .collect()
    }

    /// The cells the cursor covers: a wide character's whole box, otherwise one cell.
    fn cursor_cell(&self, cursor: Cursor) -> (u16, u16) {
        self.rows
            .get(usize::from(cursor.y))
            .and_then(|row| {
                row.row
                    .runs
                    .iter()
                    .find(|run| run.col <= cursor.x && cursor.x < run.col + run.width)
            })
            .filter(|run| !run.is_plain_flow())
            .map_or((cursor.x, 1), |run| (run.col, run.width))
    }
}

impl IntoElement for TerminalElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for TerminalElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        self.cell = self.metrics.cell_size(
            self.cols,
            u16::try_from(self.rows.len()).unwrap_or(u16::MAX),
            window.scale_factor(),
        );
        let m = self.cell;
        let style = gpui::Style {
            size: Size {
                width: px(f32::from(self.cols) * m.width).into(),
                height: px(self.rows.len() as f32 * m.height).into(),
            },
            flex_shrink: 0.0,
            ..Default::default()
        };
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        _: &mut App,
    ) {
        if self.visible {
            for row in &self.rows {
                if row.shaped.borrow().is_none() {
                    row.shaped.replace(Some(self.shape(row, window)));
                }
            }
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let (_, bg) = style::defaults(self.state);
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            window.paint_quad(fill(bounds, bg.hsla(1.)));
            if !self.visible {
                return;
            }
            self.paint_rows(bounds, window, cx);
            self.paint_cursor(bounds, window, cx);
        });
    }
}

impl TerminalElement {
    /// Paint every row's backgrounds, glyphs and decorations from the shaped cache.
    fn paint_rows(&self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let m = self.cell;
        let baselines = self.baselines(bounds.origin.y, window);
        for (y, row) in self.rows.iter().enumerate() {
            let shaped = row.shaped.borrow();
            for run in shaped.as_ref().into_iter().flatten() {
                let origin = bounds.origin
                    + point(px(f32::from(run.col) * m.width), px(y as f32 * m.height));
                let area = Bounds::new(
                    origin,
                    size(px(f32::from(run.width) * m.width), px(m.height)),
                );
                window.paint_quad(fill(area, run.style.bg.hsla(1.)));
                if !run.style.hidden {
                    for piece in &run.pieces {
                        let origin = origin + point(px(f32::from(piece.col) * m.width), px(0.));
                        let cell = Bounds::new(
                            origin,
                            size(px(f32::from(piece.width) * m.width), px(m.height)),
                        );
                        // ASCII keeps its run's italic overhang (once an italic face is bundled); other text is
                        // clipped to its own box.
                        let clip = if run.plain_flow { area } else { cell };
                        // The cell stays the clip; only the glyphs move, by less than half a logical pixel, so
                        // stacked rows' block and box-drawing glyphs meet (`Baselines`).
                        let glyphs = point(
                            origin.x,
                            px(baselines.origin_y(
                                y,
                                f32::from(piece.line.ascent),
                                f32::from(piece.line.descent),
                            )),
                        );
                        window.with_content_mask(Some(ContentMask { bounds: clip }), |window| {
                            if let Err(error) = piece.line.paint(
                                glyphs,
                                px(m.height),
                                TextAlign::Left,
                                None,
                                window,
                                cx,
                            ) {
                                tracing::warn!("terminal glyph paint failed: {error}");
                            }
                        });
                    }
                }
                // Hidden text keeps its background and decorations.
                paint_decoration(area, run.style, window);
            }
        }
    }

    /// Paint the cursor: outlined while unfocused, otherwise in its shape, with a block re-painting its glyph in the
    /// accent color. A blinking cursor is skipped during the blink's off phase; an unfocused one never blinks.
    fn paint_cursor(&self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let Some(cursor) = self
            .cursor
            .filter(|c| c.visible && (!self.phase.focused || !c.blink || self.phase.on))
        else {
            return;
        };
        let m = self.cell;
        let cursor_color = TerminalColor::of(TERMINAL.cursor).hsla(1.);
        let (start, cells) = self.cursor_cell(cursor);
        let area = Bounds::new(
            bounds.origin
                + point(
                    px(f32::from(start) * m.width),
                    px(f32::from(cursor.y) * m.height),
                ),
            size(px(f32::from(cells) * m.width), px(m.height)),
        );
        if !self.phase.focused {
            outline(area, cursor_color, window);
        } else if cursor.shape == CursorShape::Block {
            window.paint_quad(fill(area, cursor_color));
            self.paint_cursor_glyph(cursor, area, bounds, window, cx);
        } else {
            let shape = if cursor.shape == CursorShape::Bar {
                Bounds::new(area.origin, size(px(1.), area.size.height))
            } else {
                Bounds::new(
                    point(area.left(), area.bottom() - px(1.)),
                    size(area.size.width, px(1.)),
                )
            };
            window.paint_quad(fill(shape, cursor_color));
        }
    }

    /// Re-paint the glyph under a block cursor in the accent color, reusing its shaped layout: only this cell needs the
    /// second color, so shaping every row twice is never necessary.
    fn paint_cursor_glyph(
        &self,
        cursor: Cursor,
        area: Bounds<Pixels>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let m = self.cell;
        let Some(row) = self.rows.get(usize::from(cursor.y)) else {
            return;
        };
        let shaped = row.shaped.borrow();
        let accent = TerminalColor::of(TERMINAL.cursor_accent).hsla(1.);
        for run in shaped
            .as_ref()
            .into_iter()
            .flatten()
            .filter(|r| !r.style.hidden)
        {
            for piece in &run.pieces {
                let col = run.col + piece.col;
                if col <= cursor.x && cursor.x < col + piece.width {
                    // Placed exactly as `paint_rows` placed the glyph it covers, so the accent copy lands on it.
                    let origin = point(
                        bounds.origin.x + px(f32::from(col) * m.width),
                        px(self.baselines(bounds.origin.y, window).origin_y(
                            usize::from(cursor.y),
                            f32::from(piece.line.ascent),
                            f32::from(piece.line.descent),
                        )),
                    );
                    window.with_content_mask(Some(ContentMask { bounds: area }), |window| {
                        let decoration = gpui::DecorationRun {
                            len: piece.line.len() as u32,
                            color: accent,
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        };
                        if let Err(error) = gpui::LineLayout::paint(
                            &piece.line,
                            origin,
                            px(m.height),
                            TextAlign::Left,
                            None,
                            &[decoration],
                            window,
                            cx,
                        ) {
                            tracing::warn!("terminal cursor glyph paint failed: {error}");
                        }
                    });
                }
            }
        }
    }
}

/// The terminal face for a resolved style, with ligatures off so every character keeps its own cell.
///
/// NOTE: italic draws upright for now. Only the regular and bold faces are bundled (the old app shipped no italic face
/// and let the browser slant the regular one), and GPUI does not synthesize an oblique. The italic request stays, so
/// the Italic and Bold Italic faces a follow-up change bundles are picked up here without a code change.
fn terminal_face(style: ResolvedStyle) -> gpui::Font {
    let mut face = font(TERMINAL_FONT);
    if style.bold {
        face = face.bold();
    }
    if style.italic {
        face = face.italic();
    }
    face.features = FontFeatures(Arc::new(vec![
        ("calt".into(), 0),
        ("liga".into(), 0),
        ("dlig".into(), 0),
    ]));
    face
}

/// A one-pixel outline inside the cell, so the cursor in the last column stays inside the viewport.
fn outline(area: Bounds<Pixels>, color: gpui::Hsla, window: &mut Window) {
    for rect in [
        Bounds::new(area.origin, size(area.size.width, px(1.))),
        Bounds::new(
            point(area.left(), area.bottom() - px(1.)),
            size(area.size.width, px(1.)),
        ),
        Bounds::new(area.origin, size(px(1.), area.size.height)),
        Bounds::new(
            point(area.right() - px(1.), area.top()),
            size(px(1.), area.size.height),
        ),
    ] {
        window.paint_quad(fill(rect, color));
    }
}

/// Underline and strikethrough, drawn from the cell's edges rather than font metrics. The underline sits inside the
/// cell so a double or wavy stroke never reaches the next row; strikethrough shares the underline's color and shape,
/// as the old app's single CSS text decoration did.
fn paint_decoration(area: Bounds<Pixels>, style: ResolvedStyle, window: &mut Window) {
    let color = style.underline_color.hsla(style.underline_alpha);
    for (kind, y) in [
        (style.underline, area.bottom() - px(2.)),
        (
            style
                .strike
                .then_some(style.underline.unwrap_or(Underline::Solid)),
            area.top() + area.size.height * 0.55,
        ),
    ] {
        let Some(kind) = kind else {
            continue;
        };
        if matches!(kind, Underline::Solid | Underline::Double) {
            window.paint_quad(fill(
                Bounds::new(point(area.left(), y), size(area.size.width, px(1.))),
                color,
            ));
            if kind == Underline::Double {
                window.paint_quad(fill(
                    Bounds::new(
                        point(area.left(), y - px(2.)),
                        size(area.size.width, px(1.)),
                    ),
                    color,
                ));
            }
        } else {
            let mut path = PathBuilder::stroke(px(1.));
            if kind == Underline::Dotted {
                path = path.dash_array(&[px(1.), px(2.)]);
            }
            if kind == Underline::Dashed {
                path = path.dash_array(&[px(4.), px(3.)]);
            }
            path.move_to(point(area.left(), y));
            if kind == Underline::Wavy {
                let width = f32::from(area.size.width);
                for x in 1..=width.ceil() as u32 {
                    let dx = (x as f32).min(width);
                    path.line_to(point(
                        area.left() + px(dx),
                        y - px((dx * std::f32::consts::PI / 3.).sin()),
                    ));
                }
            } else {
                path.line_to(point(area.right(), y));
            }
            if let Ok(path) = path.build() {
                window.paint_path(path, color);
            }
        }
    }
}
