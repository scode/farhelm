//! Remembering the desktop window's frame across launches and restoring it
//! safely: a saved frame is reused only when it still fits a connected
//! display, and the maximized state never overwrites the last ordinary frame.

use super::state::atomic_write_json;
use super::*;

pub(super) const WINDOW_STATE_FILE: &str = "desktop-window.json";
// Version 2 changes the saved size from the client area to the complete
// native frame, so older records must take the safe fallback path.
const WINDOW_STATE_VERSION: u32 = 2;
const WINDOW_STATE_MAX_BYTES: u64 = 4096;
/// Physical pixels for the native frame's outer position and size.
///
/// Keeping one unit in the file matters when a window crosses monitors with
/// different scale factors. Restore converts the saved outer size to Tao's
/// inner-size API using the current decoration extent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
struct WindowRect {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

/// The maximized frame must never replace the last ordinary rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
struct WindowState {
    version: u32,
    normal: WindowRect,
    maximized: bool,
}

/// Missing is an ordinary first launch; invalid state needs a safe placement.
#[derive(Debug, Clone, Copy)]
enum LoadedWindowState {
    Missing,
    Invalid,
    Valid(WindowState),
}

/// Retain the last ordinary frame until Dioxus's close handler removes Tao's window.
pub(super) struct WindowTracker {
    saved: LoadedWindowState,
    normal: Option<WindowRect>,
    maximized: MaximizeState,
    /// Suppress geometry events until a requested maximize transition settles.
    restore_maximized: Option<bool>,
    window: Option<Arc<dioxus::desktop::tao::window::Window>>,
    flushed: bool,
}

impl WindowTracker {
    /// Start tracking from whatever frame the previous launch saved at
    /// `path`, if it saved a usable one.
    pub(super) fn load(path: &Path) -> Self {
        Self::new(load_window_state(path))
    }

    fn new(saved: LoadedWindowState) -> Self {
        Self {
            saved,
            normal: None,
            maximized: MaximizeState::default(),
            restore_maximized: None,
            window: None,
            flushed: false,
        }
    }

    /// Restore before the webview's first frame and seed the normal-frame cache.
    pub(super) fn attach(&mut self, window: Arc<dioxus::desktop::tao::window::Window>) {
        let monitors = connected_monitors(&window);
        let wayland = uses_wayland(&window);
        let initial_size = frame_size(&window);
        let restored = match self.saved {
            LoadedWindowState::Missing => None,
            LoadedWindowState::Invalid => safe_fallback(&monitors, initial_size),
            LoadedWindowState::Valid(state) => restore_rectangle(state.normal, &monitors, wayland)
                .or_else(|| safe_fallback(&monitors, initial_size)),
        };
        if let Some(rect) = restored {
            if !wayland {
                window.set_outer_position(dioxus::desktop::tao::dpi::PhysicalPosition::new(
                    rect.x, rect.y,
                ));
            }
            set_frame_size(&window, rect);
        }
        self.normal = restored.or_else(|| current_normal(&window, None, &monitors, wayland));
        self.window = Some(Arc::clone(&window));
        if let LoadedWindowState::Valid(state) = self.saved {
            self.maximized = MaximizeState::new(state.maximized);
            self.restore_maximized = Some(state.maximized);
            window.set_maximized(state.maximized);
        } else {
            self.maximized = MaximizeState::new(window.is_maximized());
        }
    }

    /// Observe only this window; a Dioxus event can also belong to another one.
    pub(super) fn observe(
        &mut self,
        event: &dioxus::desktop::tao::event::Event<'_, impl Sized>,
        path: &Path,
    ) {
        use dioxus::desktop::tao::event::{Event, WindowEvent};
        let Some(window) = self.window.as_ref().cloned() else {
            return;
        };
        match event {
            Event::WindowEvent {
                window_id, event, ..
            } if *window_id == window.id() => {
                let fullscreen = window.fullscreen().is_some();
                let restoring = self.restore_maximized.is_some();
                self.restore_maximized =
                    restore_maximized_after_event(self.restore_maximized, window.is_maximized());
                self.maximized.observe(window.is_maximized(), fullscreen);
                match event {
                    WindowEvent::Moved(position)
                        if !restoring && !fullscreen && !window.is_maximized() =>
                    {
                        if !uses_wayland(&window)
                            && let Some(normal) = self.normal.as_mut()
                        {
                            normal.x = position.x;
                            normal.y = position.y;
                        }
                    }
                    WindowEvent::Resized(_)
                        if !restoring && !fullscreen && !window.is_maximized() =>
                    {
                        self.normal = remember_normal(
                            self.normal,
                            current_normal(
                                &window,
                                self.normal,
                                &connected_monitors(&window),
                                uses_wayland(&window),
                            ),
                            false,
                        );
                    }
                    WindowEvent::CloseRequested => self.flush(&window, path),
                    _ => {}
                }
            }
            Event::LoopDestroyed => self.flush(&window, path),
            _ => {}
        }
    }

    /// Flush at close; a failed write may get one more attempt at loop teardown.
    fn flush(&mut self, window: &dioxus::desktop::tao::window::Window, path: &Path) {
        if self.flushed {
            return;
        }
        let fullscreen = window.fullscreen().is_some();
        let maximized = self.restore_maximized.unwrap_or_else(|| {
            self.maximized
                .for_snapshot(window.is_maximized(), fullscreen)
        });
        self.normal = remember_normal(
            self.normal,
            (!maximized && !fullscreen)
                .then(|| {
                    current_normal(
                        window,
                        self.normal,
                        &connected_monitors(window),
                        uses_wayland(window),
                    )
                })
                .flatten(),
            maximized,
        );
        let Some(normal) = self.normal else {
            tracing::warn!("could not capture ordinary native window bounds");
            return;
        };
        let state = WindowState {
            version: WINDOW_STATE_VERSION,
            normal,
            maximized,
        };
        match atomic_write_json(path, &state) {
            Ok(()) => self.flushed = true,
            Err(error) => tracing::warn!(?error, "could not save native window state"),
        }
    }
}

/// Read at most one small record so corrupt state cannot allocate without bound.
fn load_window_state(path: &Path) -> LoadedWindowState {
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

/// Keep the restore snapshot fenced until Tao reports the requested state.
fn restore_maximized_after_event(pending: Option<bool>, observed: bool) -> Option<bool> {
    pending.filter(|expected| *expected != observed)
}
/// Prefer the primary display for a fallback; coordinates may be negative.
fn connected_monitors(window: &dioxus::desktop::tao::window::Window) -> Vec<WindowRect> {
    let to_rect = |monitor: dioxus::desktop::tao::monitor::MonitorHandle| {
        let position = monitor.position();
        let size = monitor.size();
        WindowRect {
            x: position.x,
            y: position.y,
            width: size.width,
            height: size.height,
        }
    };
    let mut monitors: Vec<_> = window.available_monitors().map(to_rect).collect();
    if let Some(primary) = window.primary_monitor().map(to_rect) {
        monitors.retain(|monitor| *monitor != primary);
        monitors.insert(0, primary);
    }
    monitors
}

/// Keep a saved frame only when it fits fully on one connected display.
fn usable_rect(rect: WindowRect, monitors: &[WindowRect]) -> Option<WindowRect> {
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
fn restore_rectangle(
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
fn safe_fallback(monitors: &[WindowRect], default_size: (u32, u32)) -> Option<WindowRect> {
    let monitor = *monitors.first()?;
    let size = (default_size.0.min(1100), default_size.1.min(760));
    Some(center_on_monitor(size, monitor))
}

/// Clamp the requested size to a display before calculating its physical origin.
fn center_on_monitor(size: (u32, u32), monitor: WindowRect) -> WindowRect {
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

/// Capture the complete native frame in physical pixels.
fn frame_size(window: &dioxus::desktop::tao::window::Window) -> (u32, u32) {
    let size = window.outer_size();
    (size.width, size.height)
}

/// Estimate the current decoration extent so a saved outer frame can use Tao's
/// inner-size setter without mixing geometry units.
fn decoration_size(window: &dioxus::desktop::tao::window::Window) -> (u32, u32) {
    let outer = window.outer_size();
    let inner = window.inner_size();
    (
        outer.width.saturating_sub(inner.width),
        outer.height.saturating_sub(inner.height),
    )
}

/// Restore an outer frame through Tao's inner-size API and current decorations.
fn set_frame_size(window: &dioxus::desktop::tao::window::Window, rect: WindowRect) {
    let decoration = decoration_size(window);
    let width = rect.width.saturating_sub(decoration.0).max(1);
    let height = rect.height.saturating_sub(decoration.1).max(1);
    #[cfg(target_os = "macos")]
    {
        let logical = dioxus::desktop::tao::dpi::PhysicalSize::new(width, height)
            .to_logical::<f64>(window.scale_factor());
        window.set_inner_size(logical);
    }
    #[cfg(not(target_os = "macos"))]
    {
        window.set_inner_size(dioxus::desktop::tao::dpi::PhysicalSize::new(width, height));
    }
}

/// Trust global positions only when the actual Linux handle proves X11.
fn uses_wayland(window: &dioxus::desktop::tao::window::Window) -> bool {
    #[cfg(target_os = "linux")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        !matches!(
            window.window_handle().map(|handle| handle.as_raw()),
            Ok(RawWindowHandle::Xlib(_) | RawWindowHandle::Xcb(_))
        )
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = window;
        false
    }
}

/// Preserve the prior position when a backend cannot report global movement.
fn current_normal(
    window: &dioxus::desktop::tao::window::Window,
    prior: Option<WindowRect>,
    monitors: &[WindowRect],
    wayland: bool,
) -> Option<WindowRect> {
    let size = frame_size(window);
    let position = if wayland {
        None
    } else {
        window
            .outer_position()
            .ok()
            .map(|position| (position.x, position.y))
    };
    let fallback = prior.or_else(|| safe_fallback(monitors, size));
    Some(WindowRect {
        x: position.map_or_else(|| fallback.map_or(0, |rect| rect.x), |point| point.0),
        y: position.map_or_else(|| fallback.map_or(0, |rect| rect.y), |point| point.1),
        width: size.0,
        height: size.1,
    })
}

/// Tracks maximize state across a fullscreen transition, whose first exit
/// resize can report the unmaximized native frame before the window manager
/// restores the user's ordinary state.
#[derive(Debug, Clone, Copy, Default)]
struct MaximizeState {
    current: bool,
    pre_fullscreen: Option<bool>,
    saw_fullscreen_exit: bool,
}

impl MaximizeState {
    fn new(current: bool) -> Self {
        Self {
            current,
            ..Self::default()
        }
    }

    /// Preserve the pre-fullscreen state through the first post-exit event.
    fn observe(&mut self, current: bool, fullscreen: bool) {
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
    fn for_snapshot(self, current: bool, fullscreen: bool) -> bool {
        if fullscreen {
            self.pre_fullscreen.unwrap_or(self.current)
        } else {
            self.pre_fullscreen.unwrap_or(current)
        }
    }
}

/// A maximized observation cannot replace the frame needed by Restore.
fn remember_normal(
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
        let scaled =
            dioxus::desktop::tao::dpi::LogicalSize::new(800.0, 600.0).to_physical::<u32>(1.25);
        assert_eq!((scaled.width, scaled.height), (1000, 750));
        let saved = WindowRect {
            x: -2200,
            y: -100,
            width: scaled.width,
            height: scaled.height,
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
        assert!(atomic_write_json(&path, &state).is_err());
        assert!(!path.exists());
        std::fs::create_dir(&parent).unwrap();
        atomic_write_json(&path, &state).unwrap();
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
        atomic_write_json(&path, &updated).unwrap();
        assert_eq!(
            parse_window_state(&std::fs::read(&path).unwrap()),
            Some(updated)
        );
    }
}
