//! The component tree's view of the desktop app's updater, in a shape that
//! compiles for every target.
//!
//! The updater itself is native desktop code (`desktop/updater.rs`). The
//! components that show it (the sidebar's version readout, the `?` menu, the
//! local host row) are shared with the web build, which has no updater. So
//! they all go through [`use_app_updater`], which is `None` wherever no
//! updater runs: always in the web build, and in a desktop app that is not
//! the installed release bundle. On the web, [`AppUpdater`] has no values at
//! all, so the update-only branches compile there without a `cfg` at every
//! call site and can never be taken.

/// How the sidebar's version readout looks for the updater's current state
/// (see `desktop::updater::readout`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Readout {
    /// A newer version is installed than the one running: the readout turns
    /// red with an up-arrow.
    pub(crate) update_ready: bool,
    /// This app can no longer verify its updates and must be reinstalled:
    /// the readout carries a warning mark, and the hover text says how.
    pub(crate) needs_reinstall: bool,
    /// The readout's hover text.
    pub(crate) tooltip: String,
}

/// One glyph leading the version readout, with the class that styles it.
/// Hidden from screen readers; the readout's label or hidden text says the
/// same in words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReadoutMark {
    pub(crate) glyph: &'static str,
    pub(crate) class: &'static str,
}

/// The warning that this app can no longer verify its updates.
pub(crate) const WARNING_MARK: ReadoutMark = ReadoutMark {
    glyph: "⚠",
    class: "app-version-warning",
};

/// The marker that a restart finishes an update.
pub(crate) const UPDATE_MARK: ReadoutMark = ReadoutMark {
    glyph: "↑",
    class: "app-version-arrow",
};

/// The glyphs that lead the readout, in order, whichever element shows it
/// (the plain readout, or the update menu's toggle while an update waits).
///
/// Both renderings take their marks from here so that neither can drop one:
/// an update can wait while a newer release failed verification, and the
/// warning must not vanish behind the update marker then.
pub(crate) fn readout_marks(readout: &Readout) -> Vec<ReadoutMark> {
    let mut marks = Vec::new();
    if readout.needs_reinstall {
        marks.push(WARNING_MARK);
    }
    if readout.update_ready {
        marks.push(UPDATE_MARK);
    }
    marks
}

/// The version readout's hover when there is nothing about updates to say:
/// everywhere no updater runs, and while one is idle.
pub(crate) fn idle_tooltip(running: &str) -> String {
    format!("this client was built as farhelm {running}")
}

#[cfg(native_desktop)]
mod native {
    use super::Readout;
    use crate::desktop::{UpdaterHandle, UpdaterState, readout};
    use dioxus::prelude::*;

    /// A running updater and its latest published state, as one component
    /// sees them.
    ///
    /// `state` is the hook's own signal, the same one on every render, so
    /// holding it allocates nothing per render and two renders' values
    /// compare equal as props. It is `Some` whenever there is a handle.
    #[derive(Clone, PartialEq)]
    pub(crate) struct AppUpdater {
        handle: UpdaterHandle,
        state: Signal<Option<UpdaterState>>,
    }

    impl AppUpdater {
        /// Ask the updater to check now (the `?` menu's item and the local
        /// row's Update).
        pub(crate) fn check_now(&self) {
            self.handle.check_now();
        }

        /// Quit and reopen on the installed version (the update menu's
        /// Restart to update). A helper that relaunches the app is started
        /// first; only when it started does the window close, which quits
        /// the app the way closing it always does. When it could not start,
        /// the app stays open and the readout's hover says so.
        pub(crate) fn restart_to_update(&self) {
            if self.handle.start_relaunch() {
                dioxus::desktop::window().close();
            }
        }

        /// Whether automatic updates are on (the settings dialog's
        /// checkbox).
        pub(crate) fn automatic_updates(&self) -> bool {
            self.handle.automatic_updates()
        }

        /// Turn automatic updates on or off; on failure, a short reason
        /// for the dialog to show.
        pub(crate) fn set_automatic_updates(&self, on: bool) -> Result<(), String> {
            self.handle.set_automatic_updates(on)
        }

        /// How the version readout looks right now. Reading it subscribes
        /// the calling component to changes, like any signal read.
        pub(crate) fn readout(&self) -> Readout {
            readout(
                self.state
                    .read()
                    .as_ref()
                    .expect("seeded from the handle whenever there is one"),
            )
        }
    }

    /// The running updater, or `None` when this app has none.
    ///
    /// Every call keeps its own subscription: the published state is copied
    /// into a component-local signal by a future that waits on the
    /// updater's watch channel, which needs no particular runtime, so the
    /// component re-renders on each change without polling.
    pub(crate) fn use_app_updater() -> Option<AppUpdater> {
        let handle = try_use_context::<UpdaterHandle>();
        let initial = handle
            .as_ref()
            .map(|handle| handle.subscribe().borrow().clone());
        let mut state = use_signal(|| initial);
        let watched = handle.clone();
        use_future(move || {
            let watched = watched.clone();
            async move {
                let Some(handle) = watched else {
                    return;
                };
                let mut receiver = handle.subscribe();
                loop {
                    let next = receiver.borrow_and_update().clone();
                    if state.peek().as_ref() != Some(&next) {
                        state.set(Some(next));
                    }
                    if receiver.changed().await.is_err() {
                        return;
                    }
                }
            }
        });
        Some(AppUpdater {
            handle: handle?,
            state,
        })
    }
}

#[cfg(native_desktop)]
pub(crate) use native::{AppUpdater, use_app_updater};

/// The web build's stand-in: there is no updater, so no value of this type
/// can exist and every method is unreachable.
#[cfg(not(native_desktop))]
#[derive(Clone, PartialEq)]
pub(crate) enum AppUpdater {}

#[cfg(not(native_desktop))]
impl AppUpdater {
    pub(crate) fn check_now(&self) {
        match *self {}
    }

    pub(crate) fn readout(&self) -> Readout {
        match *self {}
    }

    pub(crate) fn restart_to_update(&self) {
        match *self {}
    }

    pub(crate) fn automatic_updates(&self) -> bool {
        match *self {}
    }

    pub(crate) fn set_automatic_updates(&self, _on: bool) -> Result<(), String> {
        match *self {}
    }
}

/// The web build never has an updater.
#[cfg(not(native_desktop))]
pub(crate) fn use_app_updater() -> Option<AppUpdater> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec: the readout leads with the warning mark whenever the reinstall
    /// notice is up, followed by the update arrow whenever an update waits,
    /// so with both the update menu's toggle shows both.
    ///
    /// Why: the first version of the toggle drew only the arrow, so a
    /// verification failure during a waiting update showed no warning at
    /// all; both renderings now take their marks from this one function.
    #[farhelm_testtrace::test]
    fn the_warning_mark_survives_a_waiting_update() {
        let readout = |update_ready, needs_reinstall| Readout {
            update_ready,
            needs_reinstall,
            tooltip: String::new(),
        };
        assert_eq!(readout_marks(&readout(false, false)), vec![]);
        assert_eq!(readout_marks(&readout(true, false)), vec![UPDATE_MARK]);
        assert_eq!(readout_marks(&readout(false, true)), vec![WARNING_MARK]);
        assert_eq!(
            readout_marks(&readout(true, true)),
            vec![WARNING_MARK, UPDATE_MARK]
        );
    }
}
