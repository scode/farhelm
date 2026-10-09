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

/// The idle hover promises update behavior only where an updater actually runs.
///
/// Development stamps keep their own wording even if a caller supplies an
/// updater: those builds are not releases, and the installed-app activation
/// gate never starts an updater for them.
pub(crate) fn idle_tooltip(running: &str, has_updater: bool) -> String {
    if farhelm_proto::is_development_build(running) {
        "This is a development build of Farhelm, not a release.".to_owned()
    } else if has_updater {
        let running = crate::peer::display_peer(running);
        format!(
            "This is Farhelm {running}. When a newer version has been installed, this turns red; select it then to restart into the new version."
        )
    } else {
        let running = crate::peer::display_peer(running);
        format!("This is Farhelm {running}.")
    }
}

/// Choose the hover from the window's build, the helm's stamp and updater readout.
///
/// The readout is the updater's existing pure presentation of its state, not a
/// second update state machine. A mismatch names both builds because the bar
/// shows the helm's version then; otherwise an active updater owns its activity
/// wording. No reported stamp means no mismatch is known. Peer build text goes
/// through the same display boundary as the visible version number.
pub(crate) fn version_tooltip(
    running: &str,
    reported: Option<&str>,
    updater: Option<&Readout>,
) -> String {
    if let Some(reported) = reported.filter(|reported| *reported != running) {
        let helm = crate::peer::display_peer(reported);
        if farhelm_proto::is_development_build(running) {
            return format!(
                "The helm runs Farhelm {helm}; this window is a development build of Farhelm."
            );
        }
        let window = crate::peer::display_peer(running);
        return format!("The helm runs Farhelm {helm}; this window was built as Farhelm {window}.");
    }
    if farhelm_proto::is_development_build(running) {
        return idle_tooltip(running, false);
    }
    updater.map_or_else(
        || idle_tooltip(running, false),
        |state| state.tooltip.clone(),
    )
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

    /// A browser or uninstalled desktop build has no update control to promise.
    /// Release candidates and dev releases still identify their real version;
    /// only the shared development-build rule replaces it with non-release text.
    #[farhelm_testtrace::test]
    fn a_window_without_an_updater_names_its_build_without_update_advice() {
        for release in ["1.2.3", "1.2.3-rc.1", "1.2.3-dev.1"] {
            assert_eq!(
                version_tooltip(release, None, None),
                format!("This is Farhelm {release}.")
            );
            assert_eq!(
                version_tooltip(release, Some(release), None),
                format!("This is Farhelm {release}.")
            );
        }
        for development in ["0.0.0-unreleased", "0.0.0-dev.1+build"] {
            assert_eq!(
                version_tooltip(development, None, None),
                "This is a development build of Farhelm, not a release."
            );
        }
    }

    /// When the bar displays a different helm build, its hover must explain
    /// both identities instead of describing that number as the window's.
    /// Development windows must not claim that their stamp names a release.
    #[farhelm_testtrace::test]
    fn a_build_mismatch_names_the_helm_and_the_window() {
        assert_eq!(
            version_tooltip("1.2.3", Some("1.2.4"), None),
            "The helm runs Farhelm 1.2.4; this window was built as Farhelm 1.2.3."
        );
        assert_eq!(
            version_tooltip("0.0.0-unreleased", Some("1.2.4"), None),
            "The helm runs Farhelm 1.2.4; this window is a development build of Farhelm."
        );
        assert_eq!(
            version_tooltip("1.2.3", Some("1.2.4\u{202e}"), None),
            "The helm runs Farhelm 1.2.4<U+202E>; this window was built as Farhelm 1.2.3."
        );
    }

    /// The existing updater owns activity words; the selector must preserve
    /// them for a matching release window and explain a mismatch instead when
    /// one is reported. This keeps the two app-bar renderings on the same text.
    #[farhelm_testtrace::test]
    fn an_updater_readout_supplies_the_release_windows_hover() {
        let state = Readout {
            update_ready: false,
            needs_reinstall: false,
            tooltip: idle_tooltip("1.2.3", true),
        };
        assert_eq!(
            version_tooltip("1.2.3", None, Some(&state)),
            "This is Farhelm 1.2.3. When a newer version has been installed, this turns red; select it then to restart into the new version."
        );
        assert_eq!(
            version_tooltip("1.2.3", Some("1.2.3"), Some(&state)),
            state.tooltip
        );
        assert_eq!(
            version_tooltip("1.2.3", Some("1.2.4"), Some(&state)),
            "The helm runs Farhelm 1.2.4; this window was built as Farhelm 1.2.3."
        );
    }

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
