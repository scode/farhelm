//! Prepare Linux's native client backing before its only startup reveal.
//!
//! The page remains the sole appearance owner. This module borrows its computed
//! opaque ground once; it owns no preference, media subscription or later theme
//! change. Readiness means ordinary root content reached the document, not that
//! authentication succeeded or the compositor painted. Native recordings must
//! establish the latter. macOS retains Dioxus's existing reveal behavior.

use super::{DESKTOP_STARTUP_TIMEOUT, refuse_and_exit};
use dioxus::desktop::tao::{event::Event, event::WindowEvent, window::WindowId};
use dioxus::prelude::*;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// One window's deadline and cancellation edge, shared with its root hook.
///
/// The native creation callback arms this before document initialization. The
/// independent bounded waiter can refuse even if the root never mounts. Locks
/// cover only these small bookkeeping operations, never eval or GTK calls.
#[derive(Clone, Default)]
pub(super) struct Startup(Arc<Shared>);

/// The deadline waiter owns no native objects and never holds this lock in GTK.
#[derive(Default)]
struct Shared {
    attempt: Mutex<Option<Attempt>>,
    changed: Condvar,
}

/// Match cancellation to the window whose creation armed this lifetime.
struct Attempt {
    window: WindowId,
    lifetime: Lifetime,
}

impl Startup {
    /// Arm at native creation, while the configured builder is still hidden.
    pub(super) fn created(&self, window: WindowId) {
        let mut attempt = self.0.attempt.lock().expect("native startup poisoned");
        assert!(attempt.is_none(), "native startup armed twice");
        *attempt = Some(Attempt {
            window,
            lifetime: Lifetime::new(Instant::now(), DESKTOP_STARTUP_TIMEOUT),
        });
        drop(attempt);
        let startup = self.clone();
        std::thread::Builder::new()
            .name("farhelm-native-startup".into())
            .spawn(move || startup.await_completion())
            .unwrap_or_else(|error| {
                refuse_and_exit(&format!(
                    "farhelm-desktop: could not monitor native startup: {error}"
                ));
            });
        tracing::debug!(target: "desktop_startup", "hidden native window created");
    }

    /// Bound failures before App initialization, independently of its executor.
    fn await_completion(&self) {
        let mut attempt = self.0.attempt.lock().expect("native startup poisoned");
        loop {
            let lifetime = &mut attempt.as_mut().expect("native startup armed").lifetime;
            match lifetime.remaining(Instant::now()) {
                Ok(Some(wait)) => {
                    (attempt, _) = self
                        .0
                        .changed
                        .wait_timeout(attempt, wait)
                        .expect("native startup poisoned");
                }
                Ok(None) => return,
                Err(Expired) => {
                    drop(attempt);
                    startup_timeout();
                }
            }
        }
    }

    /// Native close events precede Dioxus removing the window and its tasks.
    pub(super) fn observe<T: 'static>(&self, event: &Event<'_, T>) {
        let mut attempt = self.0.attempt.lock().expect("native startup poisoned");
        let Some(attempt) = attempt.as_mut() else {
            return;
        };
        let closing = match event {
            Event::LoopDestroyed => true,
            Event::WindowEvent {
                window_id,
                event: WindowEvent::CloseRequested | WindowEvent::Destroyed,
                ..
            } => *window_id == attempt.window,
            _ => false,
        };
        if closing {
            attempt.lifetime.cancel();
            self.0.changed.notify_all();
        }
    }

    /// Apply a bookkeeping transition and wake the independent deadline waiter.
    fn change<T>(&self, change: impl FnOnce(&mut Lifetime) -> T) -> T {
        let mut attempt = self.0.attempt.lock().expect("native startup poisoned");
        let result = change(&mut attempt.as_mut().expect("native startup armed").lifetime);
        self.0.changed.notify_all();
        result
    }

    /// Return the creation budget left, or stop a cancelled/completed launch.
    fn remaining(&self) -> Option<Duration> {
        self.change(|lifetime| lifetime.remaining(Instant::now()))
            .unwrap_or_else(|Expired| startup_timeout())
    }

    /// Consume the sole reveal permission; an expired or cancelled result is inert.
    fn permit_reveal(&self) -> bool {
        self.change(|lifetime| lifetime.permit_reveal(Instant::now()))
            .unwrap_or_else(|Expired| startup_timeout())
    }

    /// End the deadline only after the requested window is actually visible.
    fn complete(&self) -> bool {
        self.change(|lifetime| lifetime.complete(Instant::now()))
            .unwrap_or_else(|Expired| startup_timeout())
    }

    /// A deliberate minimize/close must never be undone by a late readiness reply.
    fn cancel(&self) {
        self.change(Lifetime::cancel);
    }

    /// Refuse once; cancellation racing a failed eval is an ordinary close.
    fn fail(&self, detail: &str) {
        if self.change(Lifetime::fail) {
            refuse_and_exit(&format!(
                "farhelm-desktop: native window startup failed: {detail}"
            ));
        }
    }
}

/// Use the established fatal contract, including the managed child's stdin tether.
fn startup_timeout() -> ! {
    refuse_and_exit(&format!(
        "farhelm-desktop: native window startup did not finish within {} seconds",
        DESKTOP_STARTUP_TIMEOUT.as_secs()
    ))
}

/// Prepare and reveal the ordinary root independently of authentication success.
///
/// Dioxus chooses direct/headless edits from initial hidden visibility and keeps
/// that scheduling after reveal. The DOM handshake therefore requires no rAF.
/// This root hook runs once; Settings and authentication retries cannot refocus
/// a later minimized window. Native callbacks and this task run on the same UI
/// thread, so close cannot interleave the synchronous backing/show operations.
pub(super) fn use_prepared_window() {
    use_hook(|| {
        let startup = consume_context::<Startup>();
        spawn(async move {
            let Some(wait) = startup.remaining() else {
                return;
            };
            let script = format!("const timeoutMs = {};\n{READY_SCRIPT}", wait.as_millis());
            let rgb =
                match tokio::time::timeout(wait, document::eval(&script).join::<[u8; 3]>()).await {
                    Ok(Ok(rgb)) => rgb,
                    Ok(Err(error)) => {
                        startup.fail(&format!("document readiness: {error}"));
                        return;
                    }
                    Err(_) => {
                        startup.fail("document readiness timed out");
                        return;
                    }
                };
            let window = dioxus::desktop::window();
            if window.window.is_minimized() {
                startup.cancel();
                return;
            }
            if !startup.permit_reveal() {
                return;
            }
            tracing::debug!(target: "desktop_startup", ?rgb, "root content acknowledged");
            if let Err(error) = prepare_backing(&window, rgb) {
                startup.fail(error);
                return;
            }
            // Tao queues the window ground before this show request. Both
            // native and WebKit backing are needed at first mapping; the menu
            // retains its own GTK background rather than borrowing that RGB.
            // Preparation does not acknowledge rasterization before GTK maps.
            tracing::debug!(target: "desktop_startup", ?rgb, "native backing prepared; requesting reveal");
            window.window.set_visible(true);
            loop {
                if window.window.is_minimized() {
                    startup.cancel();
                    return;
                }
                let Some(wait) = startup.remaining() else {
                    return;
                };
                if window.window.is_visible() {
                    if startup.complete() {
                        tracing::debug!(target: "desktop_startup", "native window observed visible; focusing once");
                        window.window.set_focus();
                    }
                    return;
                }
                // sleep-ok: bounded polling of Tao's actual visibility after its queued show request.
                tokio::time::sleep(wait.min(Duration::from_millis(25))).await;
            }
        });
    });
}

/// Prepare both native backing surfaces without recoloring GTK's menu labels.
///
/// Wry 0.53.5 casts its u8 channels directly to GDK's floating-point channels;
/// those values need division by 255. The public typed WebKitGTK API avoids a
/// framework fork. The window also needs the same startup ground until WebKit
/// presents. GTK's menu keeps its own theme's background and foreground through
/// the standard background role. Readback establishes assignment, not pixels.
fn prepare_backing(
    window: &dioxus::desktop::DesktopContext,
    rgb: [u8; 3],
) -> Result<(), &'static str> {
    use dioxus::desktop::tao::platform::unix::WindowExtUnix;
    use dioxus::desktop::wry::WebViewExtUnix;
    use gtk::prelude::*;
    use webkit2gtk::WebViewExt;

    // Dioxus 0.7.10 attaches Muda's one GtkMenuBar directly to Tao's default
    // box before App mounts. Style that actual widget, never its siblings or
    // the whole container: a transparent bar otherwise exposes the borrowed
    // page ground beneath system-themed labels under an inverse override.
    let menu = window
        .window
        .default_vbox()
        .ok_or("native content box is missing from the desktop window")?
        .children()
        .into_iter()
        .find_map(|child| child.downcast::<gtk::MenuBar>().ok())
        .ok_or("native menu is missing from the desktop window")?;
    menu.style_context().add_class(gtk::STYLE_CLASS_BACKGROUND);

    let webview = window.webview.webview();
    let expected = rgb.map(|channel| f64::from(channel) / 255.0);
    let mut color = webview.background_color();
    color.set_red(expected[0]);
    color.set_green(expected[1]);
    color.set_blue(expected[2]);
    color.set_alpha(1.0);
    webview.set_background_color(&color);
    let actual = webview.background_color();
    if [actual.red(), actual.green(), actual.blue(), actual.alpha()]
        .into_iter()
        .zip([expected[0], expected[1], expected[2], 1.0])
        .any(|(actual, expected)| (actual - expected).abs() > 0.000_001)
    {
        return Err("WebKit backing color did not retain the document ground");
    }
    window
        .window
        .set_background_color(Some((rgb[0], rgb[1], rgb[2], 255)));
    Ok(())
}

/// The only page acknowledgement: applied ordinary content and its opaque ground.
///
/// Mutation observation works while the native window is hidden. The finite JS
/// timer also disconnects it if the Rust eval future is dropped; dropping that
/// future alone does not cancel page work in Dioxus's eval bridge.
const READY_SCRIPT: &str = r#"
return await new Promise((resolve, reject) => {
  const finish = (rgb) => {
    observer.disconnect();
    clearTimeout(timer);
    resolve(rgb);
  };
  const inspect = () => {
    const root = document.documentElement;
    const frame = document.querySelector('.window-root');
    if (!window.farhelmAppearance || !['dark', 'light'].includes(root.dataset.appearance)
        || !document.getElementById('farhelm-app-css') || !frame?.textContent.trim()
        || !document.body) return;
    const ground = getComputedStyle(root).backgroundColor;
    if (getComputedStyle(document.body).backgroundColor !== ground) return;
    const match = /^rgb\(\s*(\d+),\s*(\d+),\s*(\d+)\s*\)$/.exec(ground);
    if (!match) return;
    const rgb = match.slice(1).map(Number);
    if (rgb.every(value => value >= 0 && value <= 255)) finish(rgb);
  };
  const observer = new MutationObserver(inspect);
  const timer = setTimeout(() => {
    observer.disconnect();
    reject(new Error('ordinary native root content did not become ready'));
  }, timeoutMs);
  observer.observe(document.documentElement, {subtree: true, childList: true, characterData: true, attributes: true});
  inspect();
});
"#;

/// Only Waiting grants a reveal; only Revealing accepts visible completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Waiting,
    Revealing,
    Complete,
    Cancelled,
    Failed,
}

/// The caller which first expires the lifetime owns the fatal diagnostic.
#[derive(Debug, PartialEq, Eq)]
struct Expired;

/// A one-shot permit whose deadline is measured from creation, never root mount.
struct Lifetime {
    deadline: Instant,
    phase: Phase,
}

impl Lifetime {
    /// Preserve one absolute creation deadline through all readiness phases.
    fn new(created: Instant, budget: Duration) -> Self {
        Self {
            deadline: created + budget,
            phase: Phase::Waiting,
        }
    }

    /// Terminal outcomes cannot regain a budget, even with an earlier supplied time.
    fn remaining(&mut self, now: Instant) -> Result<Option<Duration>, Expired> {
        if !matches!(self.phase, Phase::Waiting | Phase::Revealing) {
            return Ok(None);
        }
        if now >= self.deadline {
            self.phase = Phase::Failed;
            return Err(Expired);
        }
        Ok(Some(self.deadline - now))
    }

    /// Only one ready response can request visibility; visibility is not completion.
    fn permit_reveal(&mut self, now: Instant) -> Result<bool, Expired> {
        if self.remaining(now)?.is_some() && self.phase == Phase::Waiting {
            self.phase = Phase::Revealing;
            return Ok(true);
        }
        Ok(false)
    }

    /// A visible acknowledgement must still belong to the pending reveal and budget.
    fn complete(&mut self, now: Instant) -> Result<bool, Expired> {
        if self.remaining(now)?.is_some() && self.phase == Phase::Revealing {
            self.phase = Phase::Complete;
            return Ok(true);
        }
        Ok(false)
    }

    /// Once a user cancels, delayed page or visibility acknowledgements are inert.
    fn cancel(&mut self) {
        if matches!(self.phase, Phase::Waiting | Phase::Revealing) {
            self.phase = Phase::Cancelled;
        }
    }

    /// Claim the fatal diagnostic once, without turning a close into an error.
    fn fail(&mut self) -> bool {
        if matches!(self.phase, Phase::Waiting | Phase::Revealing) {
            self.phase = Phase::Failed;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A slow initialize cannot reset the outer deadline at root mount, and a
    /// show request alone must not discharge failure to become visible.
    #[test]
    fn creation_budget_covers_readiness_and_actual_visibility() {
        let created = Instant::now();
        let mut startup = Lifetime::new(created, Duration::from_secs(30));
        assert_eq!(startup.complete(created), Ok(false));
        let ready = created + Duration::from_secs(29);
        assert_eq!(startup.permit_reveal(ready), Ok(true));
        assert_eq!(startup.remaining(ready), Ok(Some(Duration::from_secs(1))));
        assert_eq!(startup.permit_reveal(ready), Ok(false));
        assert_eq!(
            startup.complete(created + Duration::from_secs(30)),
            Err(Expired)
        );
        assert_eq!(startup.phase, Phase::Failed);
        assert_eq!(startup.permit_reveal(ready), Ok(false));
    }

    /// Closing before readiness or minimizing during reveal must make every
    /// late acknowledgement harmless, including one delivered past the deadline.
    #[test]
    fn cancellation_never_reopens_or_reports_a_late_failure() {
        let now = Instant::now();
        for revealing in [false, true] {
            let mut startup = Lifetime::new(now, Duration::from_secs(30));
            if revealing {
                assert_eq!(startup.permit_reveal(now), Ok(true));
            }
            startup.cancel();
            assert_eq!(startup.permit_reveal(now), Ok(false));
            assert_eq!(startup.complete(now), Ok(false));
            assert_eq!(startup.remaining(now + Duration::from_secs(31)), Ok(None));
            assert!(!startup.fail());
            assert_eq!(startup.phase, Phase::Cancelled);
        }
    }

    /// A successful first reveal is final: remounts and later minimize/close
    /// events cannot grant a second focus or turn a finished launch into refusal.
    #[test]
    fn visible_completion_is_once_and_terminal() {
        let now = Instant::now();
        let mut startup = Lifetime::new(now, Duration::from_secs(30));
        assert_eq!(startup.permit_reveal(now), Ok(true));
        assert_eq!(startup.complete(now), Ok(true));
        startup.cancel();
        assert_eq!(startup.complete(now), Ok(false));
        assert_eq!(startup.permit_reveal(now), Ok(false));
        assert_eq!(startup.remaining(now + Duration::from_secs(31)), Ok(None));
        assert!(!startup.fail());
        assert_eq!(startup.phase, Phase::Complete);
    }

    /// Missing initialization and a failed backing/eval both have one fatal
    /// outcome; a duplicate diagnostic never becomes another startup action.
    #[test]
    fn failure_without_a_root_and_explicit_failure_are_terminal() {
        let now = Instant::now();
        let mut missing = Lifetime::new(now, Duration::from_secs(30));
        assert_eq!(
            missing.remaining(now + Duration::from_secs(30)),
            Err(Expired)
        );
        assert!(!missing.fail());
        assert_eq!(missing.permit_reveal(now), Ok(false));
        let mut failed = Lifetime::new(now, Duration::from_secs(30));
        assert!(failed.fail());
        assert!(!failed.fail());
        assert_eq!(failed.complete(now), Ok(false));
        assert_eq!(failed.remaining(now + Duration::from_secs(31)), Ok(None));
    }
}
