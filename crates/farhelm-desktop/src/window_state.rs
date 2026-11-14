//! The desktop window's saved frame: its on-disk format and the arithmetic that decides whether and where a saved
//! frame is restored. A saved frame is reused only when it still fits a connected display, and the maximized state
//! never overwrites the last ordinary frame.
//!
//! This is the framework-free half of the old Dioxus window's frame tracking, moved when that UI was deleted (GPUI
//! migration, F1). The half that read and set a live window (Tao's monitor list, outer and inner sizes, the X11 versus
//! Wayland handle check, and the event hook that saved at close) went with it; the GPUI window supplies those facts as
//! plain values: the connected monitors as [`WindowRect`]s with the primary first, the current outer frame size and
//! position, and whether the backend can report global positions at all.

use std::fs::File;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::state::atomic_write_json;

/// The window frame's file in the app's state directory.
pub const WINDOW_STATE_FILE: &str = "desktop-window.json";
// Version 2 changes the saved size from the client area to the complete
// native frame, so older records must take the safe fallback path.
pub const WINDOW_STATE_VERSION: u32 = 2;
const WINDOW_STATE_MAX_BYTES: u64 = 4096;
/// Physical pixels for the native frame's outer position and size.
///
/// Keeping one unit in the file matters when a window crosses monitors with
/// different scale factors. A window toolkit that sizes the inner (client) area must convert using its current
/// decoration extent.
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
/// `initial_size` is the default outer frame the window opened with; it sizes the fallback when the saved frame is
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

/// The ordinary frame to remember from the window's current outer `size` and, when the backend reports one, its global
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
}
