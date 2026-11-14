//! The desktop window's saved frame: its on-disk format, the arithmetic that decides whether and where a saved frame is
//! restored, and [`FrameKeeper`], which follows the live window and writes the frame as it changes. A saved frame is
//! reused only when it still fits a connected display, and the maximized state never overwrites the last ordinary
//! frame (SPEC.md, "Desktop window chrome").
//!
//! This is the framework-free half of the window's frame tracking. Most of it moved unchanged from the old Dioxus
//! window when that UI was deleted; the half that read and set a live window (Tao's monitor list, outer and inner
//! sizes, the X11 versus Wayland handle check) went with it. The GPUI window supplies those facts as plain values
//! instead: the displays the window is placed among as [`WindowRect`]s (the one a fallback is sized for first), the
//! window's ordinary frame, whether it is maximized or fullscreen, and whether its position can be trusted (never on
//! Wayland).
//!
//! Every rectangle here is in the logical pixels GPUI reports for windows and displays alike, in the coordinates the
//! window side reports (on macOS relative to the primary display, which is the only one it reports). The old app saved
//! physical pixels (version 2), which Tao reported for both; GPUI reports no scale factor for a display, only for a
//! window once it is open, so a physical frame could not be converted back before the window exists, and not at all
//! across displays with different scales. Version 3 is therefore logical, and a version 2 record from the old app reads
//! as invalid and takes the safe fallback once.
//!
//! The old app wrote the frame once, when the window closed. GPUI gives the app's quit handlers 200 ms in all
//! (SPEC_impl.md, "Startup and quit: the bootstrap"), so the frame is written while the app runs instead: the keeper
//! holds the latest change ([`FrameKeeper::observe`] never writes) and writes it when the window has settled, which the
//! window side says a second after the window stops changing (`farhelm_gui::desktop::follow_frame`), and the quit
//! handler writes whatever is still pending (one small file, well inside the deadline). A drag is written once, when it
//! ends, not on every pixel. The cost is that a crash within about a second of a move or resize can lose that last
//! change.

use std::fs::File;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::state::atomic_write_json;

/// The window frame's file in the app's state directory.
pub const WINDOW_STATE_FILE: &str = "desktop-window.json";
/// The record's version. Version 2 changed the saved size from the client area to the complete native frame, and
/// version 3 changes the unit from physical to logical pixels (see the module docs), so older records take the safe
/// fallback path.
pub const WINDOW_STATE_VERSION: u32 = 3;
const WINDOW_STATE_MAX_BYTES: u64 = 4096;
/// A window's frame (its origin and content size) or a display's bounds, in GPUI's logical pixels and in the
/// coordinates the window side reports them in (`farhelm_gui::desktop::facts`: on macOS relative to the primary
/// display).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub struct WindowRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// The maximized frame must never replace the last ordinary rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub struct WindowState {
    pub version: u32,
    pub normal: WindowRect,
    pub maximized: bool,
}

/// Missing is an ordinary first launch; invalid state needs a safe placement.
#[derive(Debug, Clone, Copy)]
pub enum LoadedWindowState {
    /// No file: an ordinary first launch, which keeps the window's default placement.
    Missing,
    /// Unreadable, oversized, corrupt or another version: restore a safe placement instead.
    Invalid,
    /// A record this build can trust.
    Valid(WindowState),
}

/// Where a launch should place the window before its first frame, or `None` to keep the window's own default.
///
/// `initial_size` is the default size the window opened with; it sizes the fallback when the saved frame is
/// unusable. With `wayland` set the saved position is ignored (see [`restore_rectangle`]).
pub fn restore_frame(
    saved: LoadedWindowState,
    monitors: &[WindowRect],
    wayland: bool,
    initial_size: (u32, u32),
) -> Option<WindowRect> {
    match saved {
        LoadedWindowState::Missing => None,
        LoadedWindowState::Invalid => safe_fallback(monitors, initial_size),
        LoadedWindowState::Valid(state) => restore_rectangle(state.normal, monitors, wayland)
            .or_else(|| safe_fallback(monitors, initial_size)),
    }
}

/// Save a frame record, owner-only and atomically (see [`atomic_write_json`]).
pub fn save_window_state(path: &Path, normal: WindowRect, maximized: bool) -> anyhow::Result<()> {
    let state = WindowState {
        version: WINDOW_STATE_VERSION,
        normal,
        maximized,
    };
    atomic_write_json(path, &state)
}

/// Read at most one small record so corrupt state cannot allocate without bound.
pub fn load_window_state(path: &Path) -> LoadedWindowState {
    use std::io::Read;
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return LoadedWindowState::Missing;
        }
        Err(error) => {
            tracing::warn!(?error, "could not read native window state");
            return LoadedWindowState::Invalid;
        }
    };
    let mut bytes = Vec::new();
    if let Err(error) = file
        .take(WINDOW_STATE_MAX_BYTES + 1)
        .read_to_end(&mut bytes)
    {
        tracing::warn!(?error, "could not read native window state");
        return LoadedWindowState::Invalid;
    }
    match parse_window_state(&bytes) {
        Some(state) => LoadedWindowState::Valid(state),
        None => {
            tracing::warn!("ignoring invalid native window state");
            LoadedWindowState::Invalid
        }
    }
}

/// Reject unknown schemas and oversized files before they reach geometry logic.
fn parse_window_state(bytes: &[u8]) -> Option<WindowState> {
    if bytes.len() as u64 > WINDOW_STATE_MAX_BYTES {
        return None;
    }
    let state: WindowState = serde_json::from_slice(bytes).ok()?;
    (state.version == WINDOW_STATE_VERSION).then_some(state)
}

/// Keep the restore snapshot fenced until the window reports the requested maximize state.
pub fn restore_maximized_after_event(pending: Option<bool>, observed: bool) -> Option<bool> {
    pending.filter(|expected| *expected != observed)
}
/// Keep a saved frame only when it fits fully on one connected display.
pub(crate) fn usable_rect(rect: WindowRect, monitors: &[WindowRect]) -> Option<WindowRect> {
    if rect.width < 320 || rect.height < 240 {
        return None;
    }
    monitors.iter().find_map(|monitor| {
        if rect.width > monitor.width || rect.height > monitor.height {
            return None;
        }
        let left = i64::from(rect.x);
        let top = i64::from(rect.y);
        let right = left + i64::from(rect.width);
        let bottom = top + i64::from(rect.height);
        let monitor_right = i64::from(monitor.x) + i64::from(monitor.width);
        let monitor_bottom = i64::from(monitor.y) + i64::from(monitor.height);
        (left >= i64::from(monitor.x)
            && top >= i64::from(monitor.y)
            && right <= monitor_right
            && bottom <= monitor_bottom)
            .then_some(rect)
    })
}

/// Wayland has no trustworthy global position, even for a valid saved size.
pub(crate) fn restore_rectangle(
    saved: WindowRect,
    monitors: &[WindowRect],
    wayland: bool,
) -> Option<WindowRect> {
    if !wayland {
        return usable_rect(saved, monitors);
    }
    monitors
        .iter()
        .find(|monitor| {
            saved.width >= 320
                && saved.height >= 240
                && saved.width <= monitor.width
                && saved.height <= monitor.height
        })
        .map(|monitor| center_on_monitor((saved.width, saved.height), *monitor))
}

/// Keep a complete frame on the primary display when the saved one is unsafe.
pub(crate) fn safe_fallback(
    monitors: &[WindowRect],
    default_size: (u32, u32),
) -> Option<WindowRect> {
    let monitor = *monitors.first()?;
    let size = (default_size.0.min(1100), default_size.1.min(760));
    Some(center_on_monitor(size, monitor))
}

/// Clamp the requested size to a display before calculating its physical origin.
pub(crate) fn center_on_monitor(size: (u32, u32), monitor: WindowRect) -> WindowRect {
    let width = size.0.min(monitor.width).max(1);
    let height = size.1.min(monitor.height).max(1);
    let x = i64::from(monitor.x) + i64::from((monitor.width - width) / 2);
    let y = i64::from(monitor.y) + i64::from((monitor.height - height) / 2);
    WindowRect {
        x: x.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        y: y.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        width,
        height,
    }
}

/// The ordinary frame to remember from the window's current `size` and, when the backend reports one, its global
/// `position`. Without a position (Wayland) the prior frame's origin is kept, or a safe fallback's.
pub fn current_normal(
    size: (u32, u32),
    position: Option<(i32, i32)>,
    prior: Option<WindowRect>,
    monitors: &[WindowRect],
) -> WindowRect {
    let fallback = prior.or_else(|| safe_fallback(monitors, size));
    WindowRect {
        x: position.map_or_else(|| fallback.map_or(0, |rect| rect.x), |point| point.0),
        y: position.map_or_else(|| fallback.map_or(0, |rect| rect.y), |point| point.1),
        width: size.0,
        height: size.1,
    }
}

/// Tracks maximize state across a fullscreen transition, whose first exit
/// resize can report the unmaximized native frame before the window manager
/// restores the user's ordinary state.
#[derive(Debug, Clone, Copy, Default)]
pub struct MaximizeState {
    current: bool,
    pre_fullscreen: Option<bool>,
    saw_fullscreen_exit: bool,
}

impl MaximizeState {
    /// Start from the window's current maximize state.
    pub fn new(current: bool) -> Self {
        Self {
            current,
            ..Self::default()
        }
    }

    /// Preserve the pre-fullscreen state through the first post-exit event.
    pub fn observe(&mut self, current: bool, fullscreen: bool) {
        if fullscreen {
            let prior = *self.pre_fullscreen.get_or_insert(self.current);
            self.current = prior;
            self.saw_fullscreen_exit = false;
        } else if let Some(prior) = self.pre_fullscreen {
            if self.saw_fullscreen_exit {
                self.pre_fullscreen = None;
                self.saw_fullscreen_exit = false;
                self.current = current;
            } else {
                self.saw_fullscreen_exit = true;
                self.current = prior;
            }
        } else {
            self.current = current;
        }
    }

    /// Close may arrive on the first post-fullscreen event, before the
    /// window manager has reported the ordinary maximize state again.
    pub fn for_snapshot(self, current: bool, fullscreen: bool) -> bool {
        if fullscreen {
            self.pre_fullscreen.unwrap_or(self.current)
        } else {
            self.pre_fullscreen.unwrap_or(current)
        }
    }
}

/// A maximized observation cannot replace the frame needed by Restore.
pub fn remember_normal(
    previous: Option<WindowRect>,
    observed: Option<WindowRect>,
    maximized: bool,
) -> Option<WindowRect> {
    if maximized {
        previous
    } else {
        observed.or(previous)
    }
}

// ---- Following the live window -------------------------------------------------------------------------------------

/// What the window reports about itself, whenever its bounds, maximized or fullscreen state change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Observed {
    /// The window's frame while it is neither maximized nor fullscreen; not used while it is either. Its position means
    /// nothing where positions are not trusted.
    pub frame: WindowRect,
    pub maximized: bool,
    pub fullscreen: bool,
}

/// Follows the main window for one launch: places it from the saved record before it opens, then remembers its last
/// ordinary frame and maximized state and writes them to the record when asked to (see the module docs).
///
/// Owned by the desktop binary and driven from the GUI's host callbacks on the main thread; it keeps no timer and no
/// thread: the window's reports, its settling and the quit are the only moments it acts.
pub struct FrameKeeper {
    path: PathBuf,
    saved: LoadedWindowState,
    /// The last ordinary frame, which a maximized or fullscreen observation never replaces.
    normal: Option<WindowRect>,
    maximized: MaximizeState,
    /// While the restored window has not yet reported the maximized state it was opened with, its observations are
    /// transient and ignored, so a first unmaximized report cannot overwrite the saved state. As in the old app, a
    /// window that never confirms that state saves nothing new this launch, which keeps the saved record rather than
    /// corrupting it.
    restoring: Option<bool>,
    /// What the record holds now, so an unchanged frame is never rewritten.
    written: Option<WindowState>,
    /// The latest state the record should hold but does not yet, for the next [`Self::flush`].
    pending: Option<WindowState>,
    /// The window's latest report, with the displays and position trust it came with, not yet applied: the rules apply
    /// to it only once the window has settled ([`Self::observe`]).
    latest: Option<(Observed, Vec<WindowRect>, bool)>,
}

impl FrameKeeper {
    /// A keeper for the record at `path`, read now (a missing or unusable record is not an error).
    pub fn load(path: PathBuf) -> Self {
        let saved = load_window_state(&path);
        let written = match saved {
            LoadedWindowState::Valid(state) => Some(state),
            LoadedWindowState::Missing | LoadedWindowState::Invalid => None,
        };
        Self {
            path,
            saved,
            normal: None,
            maximized: MaximizeState::default(),
            restoring: None,
            written,
            pending: None,
            latest: None,
        }
    }

    /// Where to open the window, before it opens: its frame and whether it opens maximized, or `None` to keep the
    /// window's own default (a first launch). `monitors` are the displays the window is placed among, the one a
    /// fallback is sized for first; `positions` is false where the backend reports no global positions (Wayland), so
    /// only the saved size is kept and the frame is centered; `initial_size` is the default frame's size, which sizes
    /// the fallback.
    pub fn placement(
        &mut self,
        monitors: &[WindowRect],
        positions: bool,
        initial_size: (u32, u32),
    ) -> Option<(WindowRect, bool)> {
        let frame = restore_frame(self.saved, monitors, !positions, initial_size)?;
        let maximized = match self.saved {
            LoadedWindowState::Valid(state) => state.maximized,
            LoadedWindowState::Missing | LoadedWindowState::Invalid => false,
        };
        self.normal = Some(frame);
        self.maximized = MaximizeState::new(maximized);
        self.restoring = Some(maximized);
        Some((frame, maximized))
    }

    /// Take one report from the window. Only the latest report counts: it is kept, and the saved-frame rules apply to
    /// it when the window has settled ([`Self::flush`]). The reports in between are not the user's frames: a zoom, a
    /// resize or a move animates, and macOS reports each step of a zoom's animation as an ordinary, unmaximized frame,
    /// which, followed report by report, would become the remembered ordinary frame in place of the one before the zoom
    /// (seen on the test Mac, 2026-11-14).
    ///
    /// A window restored maximized is not followed until some report says it is maximized: checked on every report, not
    /// only the settled one, because the report that confirms it need not be the one the window settles on (a zoom
    /// animates, and the user may move on before it settles).
    pub fn observe(&mut self, observed: Observed, monitors: &[WindowRect], positions: bool) {
        let confirming = self.restoring.is_some();
        self.restoring = restore_maximized_after_event(self.restoring, observed.maximized);
        if confirming {
            // The window has not yet reported the state it was opened in; nothing it says is the user's yet. The
            // confirming report itself is not the user's either: it is where the launch put the window.
            return;
        }
        self.latest = Some((observed, monitors.to_vec(), positions));
    }

    /// Apply the saved-frame rules to one settled report: what the record should hold now becomes pending, or nothing
    /// is pending when the record already holds it.
    fn apply(&mut self, observed: Observed, monitors: &[WindowRect], positions: bool) {
        self.maximized
            .observe(observed.maximized, observed.fullscreen);
        if !observed.maximized && !observed.fullscreen {
            let frame = observed.frame;
            let position = positions.then_some((frame.x, frame.y));
            self.normal = Some(current_normal(
                (frame.width, frame.height),
                position,
                self.normal,
                monitors,
            ));
        }
        let Some(normal) = self.normal else {
            return;
        };
        let state = WindowState {
            version: WINDOW_STATE_VERSION,
            normal,
            maximized: self
                .maximized
                .for_snapshot(observed.maximized, observed.fullscreen),
        };
        self.pending = (self.written != Some(state)).then_some(state);
    }

    /// Apply the latest report and write what is then pending, at once: when the window has settled, and from the quit
    /// handler inside GPUI's quit deadline. A failed write is logged, never shown (the frame is a convenience), and
    /// stays pending for the next flush.
    pub fn flush(&mut self) {
        if let Some((observed, monitors, positions)) = self.latest.take() {
            self.apply(observed, &monitors, positions);
        }
        let Some(state) = self.pending else {
            return;
        };
        match save_window_state(&self.path, state.normal, state.maximized) {
            Ok(()) => {
                self.pending = None;
                self.written = Some(state);
            }
            Err(error) => tracing::warn!(?error, "could not save the window's frame"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRIMARY: WindowRect = WindowRect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };
    const LEFT: WindowRect = WindowRect {
        x: -2560,
        y: -200,
        width: 2560,
        height: 1440,
    };

    /// `restore_frame` is the one call a launch makes to place its window, so each kind of saved record must reach the
    /// right branch: a first launch keeps the window's own default (no file is not evidence of a bad frame), a
    /// corrupt record and a saved frame that no longer fits both get the bounded fallback on the primary display, and a
    /// usable saved frame comes back as saved (or, on Wayland, re-centered at its saved size).
    #[test]
    fn restore_frame_picks_default_saved_or_fallback_by_record() {
        let saved = WindowRect {
            x: 100,
            y: 50,
            width: 1200,
            height: 800,
        };
        let valid = |normal| {
            LoadedWindowState::Valid(WindowState {
                version: WINDOW_STATE_VERSION,
                normal,
                maximized: false,
            })
        };
        let fallback = safe_fallback(&[PRIMARY], (1600, 1000));
        // Premise: the fallback is bounded below the window's own initial size.
        assert_eq!(
            fallback.map(|rect| (rect.width, rect.height)),
            Some((1100, 760))
        );

        assert_eq!(
            restore_frame(LoadedWindowState::Missing, &[PRIMARY], false, (1600, 1000)),
            None
        );
        assert_eq!(
            restore_frame(LoadedWindowState::Invalid, &[PRIMARY], false, (1600, 1000)),
            fallback
        );
        assert_eq!(
            restore_frame(valid(saved), &[PRIMARY], false, (1600, 1000)),
            Some(saved)
        );
        let off_screen = WindowRect { x: -2400, ..saved };
        assert_eq!(
            restore_frame(valid(off_screen), &[PRIMARY], false, (1600, 1000)),
            fallback
        );
        assert_eq!(
            restore_frame(valid(off_screen), &[PRIMARY], true, (1600, 1000)),
            Some(center_on_monitor((1200, 800), PRIMARY)),
            "Wayland ignores the saved position and keeps the size"
        );
        assert_eq!(
            restore_frame(LoadedWindowState::Invalid, &[], false, (1600, 1000)),
            None
        );
    }

    /// A removed monitor must not strand the window at its old global origin.
    #[test]
    fn removed_monitor_uses_bounded_primary_fallback() {
        let saved = WindowRect {
            x: -2400,
            y: 100,
            width: 1200,
            height: 800,
        };
        assert_eq!(usable_rect(saved, &[PRIMARY, LEFT]), Some(saved));
        assert_eq!(usable_rect(saved, &[PRIMARY]), None);
        let fallback = safe_fallback(&[PRIMARY], (800, 600)).unwrap();
        assert_eq!(
            fallback,
            WindowRect {
                x: 560,
                y: 240,
                width: 800,
                height: 600
            }
        );
    }

    /// A title bar visible by only a few pixels is not a usable restoration.
    #[test]
    fn sliver_and_oversize_are_rejected() {
        let sliver = WindowRect {
            x: 1900,
            y: 100,
            width: 800,
            height: 600,
        };
        let oversize = WindowRect {
            x: 0,
            y: 0,
            width: 3000,
            height: 600,
        };
        let hidden_title_edge = WindowRect {
            x: 100,
            y: -100,
            width: 800,
            height: 600,
        };
        assert_eq!(usable_rect(sliver, &[PRIMARY]), None);
        assert_eq!(usable_rect(oversize, &[PRIMARY]), None);
        assert_eq!(usable_rect(hidden_title_edge, &[PRIMARY]), None);
        assert_eq!(
            safe_fallback(
                &[WindowRect {
                    width: 300,
                    height: 200,
                    ..PRIMARY
                }],
                (800, 600)
            )
            .unwrap()
            .width,
            300
        );
    }

    /// Frame-fit checks use the same outer dimensions that restoration saves.
    #[test]
    fn outer_frame_validation_rejects_decoration_sized_overflow() {
        let valid = WindowRect {
            x: 0,
            y: 0,
            width: PRIMARY.width,
            height: PRIMARY.height,
        };
        let overflow = WindowRect { x: 1, ..valid };
        assert_eq!(usable_rect(valid, &[PRIMARY]), Some(valid));
        assert_eq!(usable_rect(overflow, &[PRIMARY]), None);
    }

    /// Physical coordinates retain meaning across a negative-origin display.
    #[test]
    fn negative_origin_and_fractional_scale_physical_size_restore() {
        // A 1.25-scale 800x600 logical window is 1000x750 physical pixels.
        let saved = WindowRect {
            x: -2200,
            y: -100,
            width: 1000,
            height: 750,
        };
        assert_eq!(
            restore_rectangle(saved, &[PRIMARY, LEFT], false),
            Some(saved)
        );
        // A 1.25-scale 800x600 logical window occupies 1000x750 physical pixels.
        assert_eq!(
            center_on_monitor((1000, 750), LEFT),
            WindowRect {
                x: -1780,
                y: 145,
                width: 1000,
                height: 750,
            }
        );
    }

    /// Wayland can preserve a usable size but cannot trust saved global x/y.
    #[test]
    fn wayland_centers_size_without_saved_position() {
        let saved = WindowRect {
            x: -9000,
            y: 7000,
            width: 900,
            height: 700,
        };
        assert_eq!(
            restore_rectangle(saved, &[PRIMARY], true),
            Some(WindowRect {
                x: 510,
                y: 190,
                width: 900,
                height: 700,
            })
        );
        assert_eq!(restore_rectangle(saved, &[PRIMARY], false), None);
    }

    /// Close while maximized must save the last ordinary frame, not the screen frame.
    #[test]
    fn maximized_transitions_preserve_last_normal_bounds() {
        let first = WindowRect {
            x: 100,
            y: 80,
            width: 900,
            height: 650,
        };
        let maximized = PRIMARY;
        let restored = WindowRect {
            x: 120,
            y: 90,
            width: 850,
            height: 620,
        };
        let normal = remember_normal(Some(first), Some(maximized), true);
        assert_eq!(normal, Some(first));
        assert_eq!(remember_normal(normal, None, true), Some(first));
        assert_eq!(
            remember_normal(normal, Some(restored), false),
            Some(restored)
        );
    }

    /// Fullscreen exit events cannot erase the maximize state before close.
    #[test]
    fn fullscreen_exit_preserves_maximized_state_for_the_first_close() {
        let mut state = MaximizeState::new(true);
        state.observe(false, true);
        assert!(state.for_snapshot(false, true));
        state.observe(false, false);
        assert!(state.for_snapshot(false, false));
        state.observe(false, false);
        assert!(!state.for_snapshot(false, false));
    }

    /// A saved maximized frame stays fenced until the native transition settles.
    #[test]
    fn restore_maximize_guard_ignores_transient_unmaximized_event() {
        let pending = Some(true);
        assert_eq!(restore_maximized_after_event(pending, false), Some(true));
        assert_eq!(restore_maximized_after_event(pending, true), None);
    }

    /// Corrupt and future-version records must not block startup or be trusted.
    #[test]
    fn record_parser_rejects_corrupt_and_unsupported_state() {
        let valid = WindowState {
            version: WINDOW_STATE_VERSION,
            normal: PRIMARY,
            maximized: true,
        };
        let bytes = serde_json::to_vec(&valid).unwrap();
        assert_eq!(parse_window_state(&bytes), Some(valid));
        assert_eq!(parse_window_state(b"{not-json"), None);
        assert_eq!(
            parse_window_state(&vec![b' '; WINDOW_STATE_MAX_BYTES as usize + 1]),
            None
        );
        let future = WindowState {
            version: WINDOW_STATE_VERSION + 1,
            ..valid
        };
        assert_eq!(
            parse_window_state(&serde_json::to_vec(&future).unwrap()),
            None
        );
    }

    /// A failed atomic write leaves no state record and can be retried later.
    #[test]
    fn missing_directory_rejects_write_without_partial_state() {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("absent");
        assert!(!parent.exists(), "fixture must lack the state directory");
        let path = parent.join(WINDOW_STATE_FILE);
        let state = WindowState {
            version: WINDOW_STATE_VERSION,
            normal: PRIMARY,
            maximized: false,
        };
        assert!(save_window_state(&path, state.normal, state.maximized).is_err());
        assert!(!path.exists());
        std::fs::create_dir(&parent).unwrap();
        save_window_state(&path, state.normal, state.maximized).unwrap();
        assert_eq!(
            parse_window_state(&std::fs::read(&path).unwrap()),
            Some(state)
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let updated = WindowState {
            normal: WindowRect {
                x: 120,
                y: 80,
                ..PRIMARY
            },
            maximized: true,
            ..state
        };
        save_window_state(&path, updated.normal, updated.maximized).unwrap();
        assert_eq!(
            parse_window_state(&std::fs::read(&path).unwrap()),
            Some(updated)
        );
    }

    // ---- FrameKeeper ---------------------------------------------------------------------------------------------

    /// A keeper on a fresh state directory, with the directory kept alive beside it.
    fn keeper() -> (tempfile::TempDir, PathBuf, FrameKeeper) {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(WINDOW_STATE_FILE);
        let keeper = FrameKeeper::load(path.clone());
        (root, path, keeper)
    }

    /// What the record on disk holds now, if anything, as its ordinary frame and maximized flag.
    fn on_disk(path: &Path) -> Option<(WindowRect, bool)> {
        std::fs::read(path)
            .ok()
            .and_then(|bytes| parse_window_state(&bytes))
            .map(|state| (state.normal, state.maximized))
    }

    /// An ordinary report of `frame`.
    fn windowed(frame: WindowRect) -> Observed {
        Observed {
            frame,
            maximized: false,
            fullscreen: false,
        }
    }

    const FRAME: WindowRect = WindowRect {
        x: 100,
        y: 80,
        width: 900,
        height: 650,
    };

    /// GPUI gives the quit only 200 ms, so the frame is written while the app runs, but not on every report: a drag
    /// reports on every pixel. Reports only make the latest state pending; the record is written when the keeper is
    /// flushed (the window settling, or the quit), with the last state reported, once.
    #[test]
    fn reports_only_make_the_frame_pending_and_a_flush_writes_the_last_one() {
        let (_root, path, mut keeper) = keeper();
        let moved = WindowRect { x: 300, ..FRAME };
        keeper.observe(windowed(FRAME), &[PRIMARY], true);
        keeper.observe(windowed(moved), &[PRIMARY], true);
        assert_eq!(on_disk(&path), None, "a report writes nothing");
        keeper.flush();
        assert_eq!(
            on_disk(&path),
            Some((moved, false)),
            "the flush writes the last report"
        );
        std::fs::remove_file(&path).unwrap();
        keeper.flush();
        assert!(!path.exists(), "nothing is left pending after a write");
    }

    /// The record is rewritten only for a real change: a report of what it already holds leaves nothing pending, also
    /// after a change was pending, so a window moved and moved back writes nothing. Shown by removing the file: no
    /// flush brings it back.
    #[test]
    fn an_unchanged_frame_is_never_rewritten() {
        let (_root, path, mut keeper) = keeper();
        keeper.observe(windowed(FRAME), &[PRIMARY], true);
        keeper.flush();
        assert!(path.exists(), "the premise: the first frame was written");
        std::fs::remove_file(&path).unwrap();
        keeper.observe(windowed(FRAME), &[PRIMARY], true);
        keeper.flush();
        keeper.observe(windowed(WindowRect { x: 7, ..FRAME }), &[PRIMARY], true);
        keeper.observe(windowed(FRAME), &[PRIMARY], true);
        keeper.flush();
        assert!(!path.exists());
    }

    /// A failed write is not the end of that frame: it stays pending, and the next flush writes it once the record can
    /// be written again.
    #[test]
    fn a_failed_write_stays_pending_for_the_next_flush() {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("absent");
        let path = parent.join(WINDOW_STATE_FILE);
        let mut keeper = FrameKeeper::load(path.clone());
        keeper.observe(windowed(FRAME), &[PRIMARY], true);
        keeper.flush();
        assert!(!path.exists(), "the premise: the write failed");
        std::fs::create_dir(&parent).unwrap();
        keeper.flush();
        assert_eq!(on_disk(&path), Some((FRAME, false)));
    }

    /// SPEC.md: the app remembers "its last ordinary window rectangle and whether it was maximized", and "native
    /// fullscreen is not persisted". Maximizing keeps the ordinary frame and saves the flag; going fullscreen from a
    /// maximized window keeps both as they were before it (fullscreen itself is never saved); unmaximizing to a new
    /// frame saves that frame.
    #[test]
    fn maximize_keeps_the_ordinary_frame_and_fullscreen_is_not_saved() {
        let (_root, path, mut keeper) = keeper();
        let step = |keeper: &mut FrameKeeper, observed| {
            keeper.observe(observed, &[PRIMARY], true);
            keeper.flush();
            on_disk(&path)
        };
        assert_eq!(step(&mut keeper, windowed(FRAME)), Some((FRAME, false)));
        // Maximized and fullscreen reports carry the screen-filling frame, which must never become the saved one.
        let maximized = Observed {
            frame: PRIMARY,
            maximized: true,
            fullscreen: false,
        };
        assert_eq!(step(&mut keeper, maximized), Some((FRAME, true)));
        let fullscreen = Observed {
            frame: PRIMARY,
            maximized: false,
            fullscreen: true,
        };
        assert_eq!(
            step(&mut keeper, fullscreen),
            Some((FRAME, true)),
            "fullscreen keeps the maximized state from before it"
        );
        let restored = WindowRect { x: 40, ..FRAME };
        // Leaving fullscreen reports the window once more before the ordinary state settles.
        step(&mut keeper, maximized);
        assert_eq!(
            step(&mut keeper, windowed(restored)),
            Some((restored, false))
        );
    }

    /// SPEC.md: "On Wayland ... Farhelm keeps a usable saved size and the maximized state while leaving placement to
    /// the compositor." Where positions are not reported, a report's position is ignored and the prior frame's origin
    /// kept; the placement then centers the saved size.
    #[test]
    fn without_positions_only_the_size_is_kept_and_the_placement_centers_it() {
        let (_root, path, mut keeper) = keeper();
        assert_eq!(
            keeper.placement(&[PRIMARY], false, (1100, 760)),
            None,
            "a first launch keeps the window's own default"
        );
        let reported = WindowRect {
            x: 0,
            y: 0,
            width: 1000,
            height: 700,
        };
        keeper.observe(windowed(reported), &[PRIMARY], false);
        keeper.flush();
        let saved = on_disk(&path).map(|(rect, _)| rect);
        assert_eq!(
            saved.map(|rect| (rect.width, rect.height)),
            Some((1000, 700))
        );
        assert_ne!(
            saved.map(|rect| (rect.x, rect.y)),
            Some((0, 0)),
            "the reported origin is not trusted"
        );
        let mut next = FrameKeeper::load(path.clone());
        assert_eq!(
            next.placement(&[PRIMARY], false, (1100, 760)),
            Some((center_on_monitor((1000, 700), PRIMARY), false))
        );
    }

    /// The next launch opens where the last left off: a saved frame that fits a display comes back with its maximized
    /// state; one that fits no display, and a record of the old app's physical-pixel version, take the bounded
    /// fallback (the safe size centered on the primary display) and open unmaximized.
    #[test]
    fn the_placement_restores_a_fitting_frame_and_falls_back_otherwise() {
        let (_root, path, _) = keeper();
        save_window_state(&path, FRAME, true).unwrap();
        assert_eq!(
            FrameKeeper::load(path.clone()).placement(&[PRIMARY], true, (1100, 760)),
            Some((FRAME, true))
        );
        let fallback = safe_fallback(&[PRIMARY], (1100, 760)).map(|rect| (rect, false));
        save_window_state(&path, WindowRect { x: -5000, ..FRAME }, false).unwrap();
        assert_eq!(
            FrameKeeper::load(path.clone()).placement(&[PRIMARY], true, (1100, 760)),
            fallback
        );
        let old_app = WindowState {
            version: 2,
            normal: FRAME,
            maximized: true,
        };
        std::fs::write(&path, serde_json::to_vec(&old_app).unwrap()).unwrap();
        assert_eq!(
            FrameKeeper::load(path.clone()).placement(&[PRIMARY], true, (1100, 760)),
            fallback,
            "a version 2 record is in physical pixels and is not trusted"
        );
    }

    /// A window restored maximized may first report itself unmaximized while the platform is still maximizing it. Those
    /// reports are not the user's and must not overwrite the saved state; once the window reports the maximized state
    /// it opened with, reports count again.
    #[test]
    fn a_window_restored_maximized_is_followed_only_once_it_is_maximized() {
        let (_root, path, _) = keeper();
        save_window_state(&path, FRAME, true).unwrap();
        let mut keeper = FrameKeeper::load(path.clone());
        assert_eq!(
            keeper.placement(&[PRIMARY], true, (1100, 760)),
            Some((FRAME, true))
        );
        keeper.observe(windowed(WindowRect { x: 7, ..FRAME }), &[PRIMARY], true);
        keeper.flush();
        assert_eq!(
            on_disk(&path),
            Some((FRAME, true)),
            "the transient report changed nothing"
        );
        let maximized = Observed {
            frame: PRIMARY,
            maximized: true,
            fullscreen: false,
        };
        keeper.observe(maximized, &[PRIMARY], true);
        keeper.flush();
        assert_eq!(
            on_disk(&path),
            Some((FRAME, true)),
            "the confirming report itself is not the user's: nothing changed"
        );
        let moved = WindowRect { x: 9, ..FRAME };
        keeper.observe(windowed(moved), &[PRIMARY], true);
        keeper.flush();
        assert_eq!(on_disk(&path), Some((moved, false)));
    }

    /// A zoom animates, and macOS reports every step of it as an ordinary, unmaximized frame before the last report
    /// says the window is zoomed (seen on the test Mac). Only the report the window settles on counts, so those steps
    /// never become the remembered ordinary frame: after a settled ordinary frame, a run of growing unmaximized reports
    /// ending in a maximized one saves the frame from before the zoom, maximized.
    #[test]
    fn the_steps_of_a_zoom_never_become_the_ordinary_frame() {
        let (_root, path, mut keeper) = keeper();
        keeper.observe(windowed(FRAME), &[PRIMARY], true);
        keeper.flush();
        for step in 1..=5 {
            let growing = WindowRect {
                x: FRAME.x - step * 20,
                width: FRAME.width + step as u32 * 40,
                ..FRAME
            };
            keeper.observe(windowed(growing), &[PRIMARY], true);
        }
        let zoomed = Observed {
            frame: PRIMARY,
            maximized: true,
            fullscreen: false,
        };
        keeper.observe(zoomed, &[PRIMARY], true);
        keeper.flush();
        assert_eq!(on_disk(&path), Some((FRAME, true)));
    }

    /// A window restored maximized confirms it by reporting itself maximized, but the window can settle on a later
    /// report: once it is confirmed, every report after it counts, whether or not the confirming one was the settled
    /// one. Shown with the sequence a keeper that confirmed only at the flush gets wrong: the opening frame, the zoomed
    /// confirmation, then the user's unzoom, flushed together, save the unzoomed frame, unmaximized.
    #[test]
    fn a_restored_maximized_window_is_confirmed_by_any_report() {
        let (_root, path, _) = keeper();
        save_window_state(&path, FRAME, true).unwrap();
        let mut keeper = FrameKeeper::load(path.clone());
        keeper.placement(&[PRIMARY], true, (1100, 760));
        let zoomed = Observed {
            frame: PRIMARY,
            maximized: true,
            fullscreen: false,
        };
        let unzoomed = WindowRect { x: 9, ..FRAME };
        keeper.observe(windowed(FRAME), &[PRIMARY], true);
        keeper.observe(zoomed, &[PRIMARY], true);
        keeper.observe(windowed(unzoomed), &[PRIMARY], true);
        keeper.flush();
        assert_eq!(on_disk(&path), Some((unzoomed, false)));
    }
}
