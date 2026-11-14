//! Client-side mouse admission, coordinate rounding, wheel and focus policy, from the pinned grid-input.js /
//! xterm.js 6.0.0 Mouse.ts and CoreMouseService (MIT; notices in LICENSE-PORTED). Byte encoding and a final
//! stale-size/mode check remain in the supervisor.

pub use farhelm_proto::grid::MouseMode;
use farhelm_proto::grid::{GridInput, MouseEventKind};

/// A finite positive cell rectangle; invalid geometry cannot produce reports.
#[derive(Clone, Copy, Debug)]
pub struct Geometry {
    pub cell_width: f64,
    pub cell_height: f64,
    pub columns: u16,
    pub rows: u16,
}

impl Geometry {
    /// The cell rectangle the pinned grid-terminal.js drew a `columns` by `rows` grid at, for a character
    /// `char_width` by `char_height` logical pixels (the font's advance, and its ascent plus descent) on a display of
    /// `pixel_ratio` device pixels per logical pixel. The renderer draws at this rectangle and pointer mapping reads
    /// the same one, so a click lands on the cell drawn under it.
    ///
    /// The height starts from the character's height rounded up to whole device pixels and is then spread so the whole
    /// grid is a whole number of logical pixels; it therefore depends on `rows`, and rows generally start a fraction of
    /// a device pixel off whole ones (a 9-row grid at 14 points on a 2x display has rows of 167/9 logical pixels, 37.11
    /// device pixels). This rectangle alone does not make glyphs meet across rows: the GUI's terminal renderer places
    /// each row's glyphs on whole-device-pixel baselines of its own (`Baselines` in its terminal module), with which
    /// box-drawing verticals meet at every size measured and full blocks only at some sizes. The width stays
    /// fractional, adjusted only so a whole row is a whole number of logical pixels; it therefore depends on
    /// `columns`, and a resize can move it by a fraction of a pixel. `None` for a zero size or a measurement that is
    /// not a positive finite number, which a caller must not draw at.
    ///
    /// Ported from xterm.js 6.0.0's `DomRenderer._updateDimensions` through the pinned grid-terminal.js `cellFor`, with
    /// no letter spacing and a line height of 1 (MIT; notices in LICENSE-PORTED).
    pub fn for_character(
        char_width: f64,
        char_height: f64,
        columns: u16,
        rows: u16,
        pixel_ratio: f64,
    ) -> Option<Self> {
        let measured = [char_width, char_height, pixel_ratio]
            .into_iter()
            .all(|v| v.is_finite() && v > 0.);
        if !measured || columns == 0 || rows == 0 {
            return None;
        }
        let (cols, lines) = (f64::from(columns), f64::from(rows));
        let device_width = char_width * pixel_ratio;
        let device_height = (char_height * pixel_ratio).ceil();
        Some(Self {
            cell_width: (device_width * cols / pixel_ratio).round() / cols,
            cell_height: (device_height * lines / pixel_ratio).round() / lines,
            columns,
            rows,
        })
    }

    /// Shared guard for program cells and selection boundaries. Both mappings
    /// reject invalid geometry rather than casting NaN or zero-sized coordinates.
    fn valid_point(self, x: f64, y: f64) -> bool {
        x.is_finite()
            && y.is_finite()
            && self.cell_width.is_finite()
            && self.cell_height.is_finite()
            && self.cell_width > 0.
            && self.cell_height > 0.
            && self.columns > 0
            && self.rows > 0
    }

    /// Match grid-input.js: an exact boundary belongs to the preceding cell,
    /// and captured drags outside the screen clamp to the edge.
    pub fn cell(self, x: f64, y: f64) -> Option<(u16, u16)> {
        if !self.valid_point(x, y) {
            return None;
        }
        let col = (x / self.cell_width)
            .ceil()
            .clamp(1.0, f64::from(self.columns));
        let row = (y / self.cell_height)
            .ceil()
            .clamp(1.0, f64::from(self.rows));
        Some(((col - 1.0) as u16, (row - 1.0) as u16))
    }

    /// Local selection uses floor for rows/cells, and nearest boundaries for a
    /// character/column drag. An exact midpoint belongs to its left boundary,
    /// matching the pinned grid client's xterm-derived selection coordinates.
    /// Boundaries may name `columns`; cell positions stop at `columns - 1`.
    pub fn selection_cell(self, x: f64, y: f64, boundary: bool) -> Option<(u16, u16)> {
        if !self.valid_point(x, y) {
            return None;
        }
        let raw = x / self.cell_width;
        let col = if boundary {
            (raw + 0.5).ceil() - 1.
        } else {
            raw.floor()
        };
        let maximum = self.columns - u16::from(!boundary);
        let col = col.clamp(0., f64::from(maximum)) as u16;
        let row = (y / self.cell_height)
            .floor()
            .clamp(0., f64::from(self.rows - 1)) as u16;
        Some((col, row))
    }
}

/// Decide the program's reporting level before recording an event for deduping.
pub fn mode_reports(mode: MouseMode, kind: MouseEventKind, button: i8) -> bool {
    mode != MouseMode::Off
        && (kind != MouseEventKind::Move
            || mode == MouseMode::Motion
            || (mode == MouseMode::Drag && button != -1))
}

/// Last *sent* mouse report, so rejected events cannot suppress later motion.
#[derive(Default)]
pub struct MouseDeduper {
    last: Option<(MouseEventKind, i8, u16, u16, u8)>,
}

impl MouseDeduper {
    /// Call only after mode/selection/link admission, immediately before sending.
    /// Meta is deliberately ignored. A press or wheel at the same cell resets the
    /// comparison, so the next move is sent even when no cell boundary was crossed.
    pub fn admit(&mut self, kind: MouseEventKind, button: i8, x: u16, y: u16, mods: u8) -> bool {
        let next = (kind, button, x, y, mods & 7);
        if kind == MouseEventKind::Move && self.last == Some(next) {
            return false;
        }
        self.last = Some(next);
        true
    }

    /// Forget old coordinates after attachment or geometry changes.
    pub fn reset(&mut self) {
        self.last = None;
    }
}

/// GPUI has pixel and line scroll deltas; Page exists for the DOM policy oracle.
#[derive(Clone, Copy)]
pub enum WheelUnit {
    Pixels,
    Lines,
    Pages,
}

/// Pixel fractions belong to one terminal, not to a window or process.
#[derive(Default)]
pub struct WheelAccumulator {
    partial: f64,
}

impl WheelAccumulator {
    /// Return the old client's signed line amount. Any nonzero amount means one
    /// wire wheel report, not `abs(amount)` reports; the existing client contract
    /// uses the amount only for admission and the original delta's sign for button.
    pub fn consume(
        &mut self,
        delta: f64,
        unit: WheelUnit,
        cell_height: f64,
        rows: u16,
        mods: u8,
    ) -> f64 {
        if !delta.is_finite()
            || !cell_height.is_finite()
            || delta == 0.0
            || mods & 1 != 0
            || cell_height <= 0.0
        {
            return 0.0;
        }
        let mut amount = if mods & 6 != 0 { delta * 5.0 } else { delta };
        match unit {
            WheelUnit::Pixels => {
                amount /= cell_height;
                if delta.abs() < 50.0 {
                    amount *= 0.3;
                }
                self.partial += amount;
                amount = self.partial.trunc();
                self.partial %= 1.0;
            }
            WheelUnit::Pages => amount *= f64::from(rows),
            WheelUnit::Lines => {}
        }
        amount
    }
}

/// How many pixels one wheel event scrolls the terminal's own scrollback, positive toward older lines: 50 pixels a
/// notch, five times that with Alt (Option), and nothing for a Shift wheel off a Mac, which the platform turns
/// horizontal. `delta` is GPUI's vertical delta, positive when the content moves down (toward older lines) on every
/// platform, in `unit`: a wheel's whole lines are notches, and a trackpad's pixels count 1.25 each.
///
/// The old app read the browser's wheel event (`wheelPixels` in grid-terminal.js at the reference commit): notches
/// were `wheelDeltaY / 120` where the engine had it, else `-deltaY / 40` in pixels or `-deltaY` in lines. A
/// trackpad's pixel deltas come out at 50 / 40 = 1.25 of a pixel each on the second rule, and on WebKit's
/// `wheelDeltaY`, three times the platform's delta, also at 1.25; a wheel notch is one line. The browser ceiled the
/// magnitude to a whole pixel, which this keeps.
///
/// Ported from Visual Studio Code's `StandardWheelEvent` and `ScrollableElement._onMouseWheel` as compiled into
/// xterm.js 6.0.0 (MIT; notices in this crate's LICENSE-PORTED), through the old app.
pub fn wheel_pixels(delta: f64, unit: WheelUnit, alt: bool, shift: bool, mac: bool) -> f64 {
    if (shift && !mac) || !delta.is_finite() {
        return 0.0;
    }
    let notches = match unit {
        WheelUnit::Pixels => delta / 40.0,
        WheelUnit::Lines => delta,
        WheelUnit::Pages => delta * 3.0,
    };
    let mut pixels = 50.0 * notches;
    if alt {
        pixels *= 5.0;
    }
    if pixels < 0.0 {
        pixels.floor()
    } else {
        pixels.ceil()
    }
}

/// Normal-screen wheel without reporting scrolls local history; alternate-screen
/// wheel goes as `Mouse::Wheel` and the supervisor emits cursor keys for it.
pub fn wheel_goes_to_program(mode: MouseMode, alternate_screen: bool) -> bool {
    mode != MouseMode::Off || alternate_screen
}

/// Report terminal focus transitions only while mode 1004 is enabled.
pub fn focus_report(focused: bool, reporting: bool) -> Option<GridInput> {
    reporting.then_some(GridInput::Focus { focused })
}

#[cfg(test)]
mod wheel_tests {
    use super::*;

    /// The old app's scroll distances, which users' hands are tuned to: a wheel notch is 50 pixels, Alt makes it five
    /// times that, a trackpad pixel counts 1.25 and a part pixel rounds away from zero, and off a Mac a Shift wheel
    /// scrolls nothing (the platform turned it horizontal). The sign follows GPUI's: positive is toward older lines.
    #[test]
    fn wheel_pixels_keeps_the_old_distances() {
        assert_eq!(wheel_pixels(1.0, WheelUnit::Lines, false, false, false), 50.0);
        assert_eq!(wheel_pixels(-1.0, WheelUnit::Lines, false, false, false), -50.0);
        assert_eq!(wheel_pixels(1.0, WheelUnit::Lines, true, false, false), 250.0);
        assert_eq!(wheel_pixels(8.0, WheelUnit::Pixels, false, false, false), 10.0);
        assert_eq!(wheel_pixels(1.0, WheelUnit::Pixels, false, false, false), 2.0);
        assert_eq!(wheel_pixels(-1.0, WheelUnit::Pixels, false, false, false), -2.0);
        assert_eq!(wheel_pixels(1.0, WheelUnit::Lines, false, true, false), 0.0);
        assert_eq!(wheel_pixels(1.0, WheelUnit::Lines, false, true, true), 50.0);
        assert_eq!(wheel_pixels(f64::NAN, WheelUnit::Lines, false, false, false), 0.0);
    }
}

#[cfg(test)]
mod selection_tests {
    use super::*;

    /// A midpoint drag keeps the character on its right, while double-click
    /// lookup uses the cell under the pointer. Captured outside points clamp.
    #[test]
    fn selection_midpoints_cells_and_captured_edges() {
        let geometry = Geometry {
            cell_width: 10.,
            cell_height: 20.,
            columns: 10,
            rows: 3,
        };
        assert_eq!(geometry.selection_cell(5., 20., true), Some((0, 1)));
        assert_eq!(geometry.selection_cell(5.01, 20., true), Some((1, 1)));
        assert_eq!(geometry.selection_cell(9.9, 20., false), Some((0, 1)));
        assert_eq!(geometry.selection_cell(-100., -100., true), Some((0, 0)));
        assert_eq!(geometry.selection_cell(200., 200., true), Some((10, 2)));
        assert_eq!(geometry.selection_cell(200., 200., false), Some((9, 2)));
        assert_eq!(geometry.selection_cell(f64::NAN, 0., true), None);
        assert_eq!(
            Geometry {
                rows: 0,
                ..geometry
            }
            .selection_cell(0., 0., true),
            None
        );
    }
}
