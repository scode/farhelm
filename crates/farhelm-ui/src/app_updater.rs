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

use dioxus::prelude::*;

/// The updater activity exposed to presentation, without a renderer dependency.
///
/// The native worker publishes non-idle check states only for a run the user
/// requested or joined. Keeping the same type here lets the inline feedback and
/// its expiry rules be tested in both builds; it does not activate a web updater.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
// No native worker constructs these variants in the web build, but the same
// presentation functions and tests must compile there without target forks.
#[cfg_attr(not(native_desktop), allow(dead_code))]
pub(crate) enum Activity {
    #[default]
    Idle,
    Checking,
    /// The installer is running for this version.
    Installing(String),
    /// This latest stable release is no newer than the installed version.
    UpToDate(String),
    /// A user-visible check or install failure; its reason stays in the hover.
    Failed(String),
    /// Relaunch failed, which is not an outcome of checking for updates.
    RestartFailed,
}

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
    /// Activity identity, kept apart from installation and verification changes
    /// so those changes cannot restart inline expiry or undo a dismissal.
    pub(crate) activity: Activity,
    /// Persistent identity of the user-visible request, including when watch
    /// delivery skips its intermediate checking/installing publications.
    pub(crate) user_check_id: u64,
    /// Readiness captured at successful user-check completion, so automatic or
    /// external installations do not acquire a check-completion announcement.
    pub(crate) user_update_ready: bool,
}

/// Which action the version offers, with motion only for user-visible work.
///
/// The absence of an updater keeps the web and uninstalled builds plain text.
/// A waiting update owns the menu even during another check; only a check
/// button owns the refresh glyph, so that menu still needs a separate spinner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReadoutControl {
    Plain,
    Check { spinning: bool },
    UpdateMenu,
}

/// Choose the version's control from the same published state as inline status.
/// Reinstall warnings retain their mark without taking away the retry action.
pub(crate) fn readout_control(readout: Option<&Readout>) -> ReadoutControl {
    match readout {
        None => ReadoutControl::Plain,
        Some(readout) if readout.update_ready => ReadoutControl::UpdateMenu,
        Some(readout) => ReadoutControl::Check {
            spinning: matches!(
                readout.activity,
                Activity::Checking | Activity::Installing(_)
            ),
        },
    }
}

/// Keep one progress indicator: the refresh glyph when present, otherwise the ring.
/// Automatic work publishes idle and cannot acquire visible motion through this rule.
pub(crate) fn separate_progress_spinner(readout: &Readout, shown: InlineStatus) -> bool {
    shown.spinning() && !matches!(readout_control(Some(readout)), ReadoutControl::Check { .. })
}

/// Render the real check action independently of the native worker subscription.
///
/// The app bar supplies its updater's check callback, shared with the help menu.
/// Keeping this hook-free makes its button semantics, action wording and event
/// dispatch testable without creating a webview or pretending a web updater exists.
pub(crate) fn check_readout(
    version: &str,
    tooltip: &str,
    readout: &Readout,
    on_check: EventHandler<()>,
) -> Element {
    let spinning = matches!(
        readout_control(Some(readout)),
        ReadoutControl::Check { spinning: true }
    );
    let marks = readout_marks(readout);
    rsx! {
        button {
            r#type: "button",
            class: if readout.needs_reinstall { "app-version app-version-check app-version-unverified peer-value" } else { "app-version app-version-check peer-value" },
            dir: "ltr",
            aria_label: "Farhelm {version}; check for updates. {tooltip}",
            // Retry advice leads so a reinstall command at the end of the
            // hover remains unambiguous to copy.
            "data-tooltip": "Select this to check for updates. {tooltip}",
            onclick: move |_| on_check.call(()),
            for mark in marks {
                span { class: mark.class, "aria-hidden": "true", "{mark.glyph}" }
            }
            span { class: "app-version-text", "{version}" }
            span {
                class: if spinning { "app-version-refresh app-version-refresh-spinning" } else { "app-version-refresh" },
                "aria-hidden": "true",
                "↻"
            }
        }
    }
}

/// Short inline words and styling, independent of the hover and version marks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InlineStatus {
    Checking,
    Installing,
    UpToDate,
    Failed,
}

impl InlineStatus {
    /// The same words are visible and announced by the mounted live region.
    pub(crate) fn text(self) -> &'static str {
        match self {
            Self::Checking => "checking…",
            Self::Installing => "installing…",
            Self::UpToDate => "up to date",
            Self::Failed => "check failed",
        }
    }

    /// Only active work needs motion; the existing spinner handles reduced motion.
    pub(crate) fn spinning(self) -> bool {
        matches!(self, Self::Checking | Self::Installing)
    }
}

/// Hide only the exact outcome dismissed or expired in this component.
///
/// An installed update owns the up-to-date answer, but not active progress or a
/// failed check. Restart failures remain hover-only. Reinstall warnings and build
/// skew have their existing independent presentation and do not alter this choice.
pub(crate) fn inline_status(
    activity: &Activity,
    update_ready: bool,
    suppressed: Option<&Activity>,
) -> Option<InlineStatus> {
    match activity {
        Activity::Checking => Some(InlineStatus::Checking),
        Activity::Installing(_) => Some(InlineStatus::Installing),
        Activity::UpToDate(_) if !update_ready && suppressed != Some(activity) => {
            Some(InlineStatus::UpToDate)
        }
        Activity::Failed(_) if suppressed != Some(activity) => Some(InlineStatus::Failed),
        _ => None,
    }
}

/// Announce a successful waiting update without competing with its red readout.
/// Failure and progress own their visible words; readiness from background work
/// alone carries no user-check provenance and must remain silent.
pub(crate) fn announce_update_ready(readout: &Readout) -> bool {
    readout.user_update_ready
        && readout.update_ready
        && matches!(readout.activity, Activity::Idle | Activity::UpToDate(_))
}

/// Component-local visibility, never a second copy of the updater's work state.
///
/// Activity and producer request identity together clear suppression. Unlike a
/// counter of observed transitions, the producer identity survives a watch
/// subscriber missing checking between identical outcomes. Hover words are separate.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct InlineFeedback {
    activity: Activity,
    user_check_id: u64,
    suppressed: Option<Activity>,
}

impl InlineFeedback {
    /// Observe a new check/activity and request expiry only for a new success.
    /// Installation-record changes with unchanged activity must not stretch its
    /// five-second lifetime or revive a failure the user dismissed.
    pub(crate) fn observe(&mut self, activity: &Activity, user_check_id: u64) -> bool {
        if self.activity == *activity && self.user_check_id == user_check_id {
            return false;
        }
        self.activity = activity.clone();
        self.user_check_id = user_check_id;
        self.suppressed = None;
        matches!(activity, Activity::UpToDate(_))
    }

    /// Expire the success that scheduled this callback, never a later result.
    pub(crate) fn expire(&mut self, activity: &Activity, user_check_id: u64) {
        if self.user_check_id == user_check_id
            && self.activity == *activity
            && matches!(activity, Activity::UpToDate(_))
        {
            self.suppressed = Some(self.activity.clone());
        }
    }

    /// Dismiss only the failure currently visible; progress and success do not
    /// acquire an accidental dismissal path through a stale click handler.
    pub(crate) fn dismiss(&mut self, activity: &Activity, user_check_id: u64) {
        if self.user_check_id == user_check_id
            && self.activity == *activity
            && matches!(activity, Activity::Failed(_))
        {
            self.suppressed = Some(activity.clone());
        }
    }
}

/// Show a user-requested desktop check beside either version readout.
///
/// The empty live region stays mounted so WebKit can announce later text. The
/// memo filters unrelated publications before they can reset expiry. Scoped tasks
/// die with the component; their activity/request pairs reject older callbacks.
#[component]
// The shared tree still typechecks this component on the web, where its
// uninhabited updater argument deliberately makes the whole body unreachable.
#[cfg_attr(not(native_desktop), allow(unreachable_code, unused_variables))]
pub(crate) fn UpdateCheckStatus(updater: AppUpdater) -> Element {
    let watched = updater.clone();
    let activity = use_memo(move || {
        let readout = watched.readout();
        (readout.activity, readout.user_check_id)
    });
    let mut feedback = use_signal(InlineFeedback::default);
    use_effect(move || {
        let (current, request) = activity();
        let expires = feedback.write().observe(&current, request);
        if expires {
            spawn(async move {
                crate::reader::sleep_ms(5_000).await;
                if *activity.peek() == (current.clone(), request) {
                    feedback.write().expire(&current, request);
                }
            });
        }
    });
    let readout = updater.readout();
    let shown = inline_status(
        &readout.activity,
        readout.update_ready,
        feedback.read().suppressed.as_ref(),
    );
    let announce_ready = announce_update_ready(&readout);
    rsx! {
        span {
            class: if shown.is_some() { "app-update-status app-update-status-visible" } else { "app-update-status" },
            role: "status",
            "aria-live": "polite",
            "aria-atomic": "true",
            // A new request replaces the child even if watch delivery skipped
            // progress and the final words match. The live parent stays mounted.
            if shown.is_some() || announce_ready {
                span {
                    key: "{readout.user_check_id}",
                    class: "app-update-status-content",
                    if let Some(shown) = shown {
                        if separate_progress_spinner(&readout, shown) {
                            span { class: "delete-spinner", "aria-hidden": "true" }
                        }
                        if shown == InlineStatus::Failed {
                            button {
                                r#type: "button",
                                class: "app-update-failure",
                                aria_label: "check failed; dismiss update check status",
                                "data-tooltip": "{readout.tooltip}",
                                onclick: move |_| {
                                    feedback.write().dismiss(&readout.activity, readout.user_check_id);
                                    // Dismissing removes this focused button; without a
                                    // new target, keyboard focus falls to the page body.
                                    // The gear beside it is the natural next stop.
                                    dioxus::document::eval(
                                        "requestAnimationFrame(() => document.querySelector('.app-settings-toggle')?.focus({preventScroll:true}))",
                                    );
                                },
                                "{shown.text()}"
                            }
                        } else {
                            span { class: "app-update-status-text", "{shown.text()}" }
                        }
                    } else if announce_ready {
                        span { class: "visually-hidden", "update ready" }
                    }
                }
            }
        }
    }
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
            "This is Farhelm {running}. When a newer version is installed, this turns red and opens the Restart to update / What's new menu."
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
        /// Ask the updater to check now (the version, `?` menu and local host).
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

    /// Activating the production native button reaches its check callback.
    /// Inspect the actual template and attributes first so a no-op handler,
    /// non-button element or lost action wording cannot hide behind state tests.
    /// Desktop already enables Dioxus's serialized event adapter; nextest runs
    /// this headless dispatch in its own process, without a native window.
    #[cfg(native_desktop)]
    #[farhelm_testtrace::test]
    fn the_check_readout_is_a_named_button_that_dispatches_a_check() {
        use dioxus::core::{AttributeValue, Mutation, TemplateAttribute, TemplateNode};
        use std::{cell::Cell, rc::Rc};

        /// Inject only the requested check's observable effect, leaving the production markup intact.
        fn render(clicks: Rc<Cell<usize>>) -> Element {
            let readout = Readout {
                update_ready: false,
                needs_reinstall: false,
                tooltip: "current version".into(),
                activity: Activity::Idle,
                user_check_id: 0,
                user_update_ready: false,
            };
            check_readout(
                "1.2.3",
                &readout.tooltip,
                &readout,
                EventHandler::new(move |_| clicks.set(clicks.get() + 1)),
            )
        }
        let clicks = Rc::new(Cell::new(0));
        let mut dom = VirtualDom::new_with_props(render, clicks.clone());
        let mutations = dom.rebuild_to_vec();
        // ROOT belongs to Dioxus's suspense/error wrappers; APP is the
        // supplied render function whose button this test must inspect.
        let vnode = dom
            .get_scope(ScopeId::APP)
            .expect("app scope rendered")
            .root_node();
        let TemplateNode::Element { tag, attrs, .. } = vnode.template.roots[0] else {
            panic!("check control must render an element");
        };
        assert_eq!(tag, "button");
        assert!(attrs.iter().any(|attr| matches!(
            attr,
            TemplateAttribute::Static {
                name: "type",
                value: "button",
                ..
            }
        )));
        let text_attr = |name: &str| {
            let attr = vnode
                .dynamic_attrs
                .iter()
                .flatten()
                .find(|attr| attr.name == name)
                .expect("action attribute must be rendered");
            match &attr.value {
                AttributeValue::Text(text) => text.clone(),
                _ => panic!("action attribute must be text"),
            }
        };
        assert!(text_attr("aria-label").contains("check for updates"));
        assert!(text_attr("data-tooltip").starts_with("Select this to check for updates."));
        let button = mutations
            .edits
            .iter()
            .find_map(|edit| match edit {
                Mutation::NewEventListener { name, id } if name == "click" => Some(*id),
                _ => None,
            })
            .expect("production button must register click");
        dioxus::html::set_event_converter(Box::new(dioxus::html::SerializedHtmlEventConverter));
        let data: Rc<dyn std::any::Any> = Rc::new(dioxus::html::PlatformEventData::new(Box::new(
            dioxus::html::SerializedMouseData::default(),
        )));
        dom.runtime()
            .handle_event("click", dioxus::core::Event::new(data, true), button);
        assert_eq!(clicks.get(), 1, "one activation must request one check");
    }

    /// Every updater state keeps its correct action and exactly one motion owner.
    /// A verification warning adds information without removing the retry, and a
    /// waiting update remains a menu even during a check joined from elsewhere.
    #[farhelm_testtrace::test]
    fn the_readout_control_and_progress_owner_follow_the_updater_state() {
        assert_eq!(readout_control(None), ReadoutControl::Plain);
        for ready in [false, true] {
            for warning in [false, true] {
                for activity in [
                    Activity::Idle,
                    Activity::Checking,
                    Activity::Installing("1.2.4".into()),
                    Activity::UpToDate("1.2.3".into()),
                    Activity::Failed("offline".into()),
                    Activity::RestartFailed,
                ] {
                    let active = matches!(activity, Activity::Checking | Activity::Installing(_));
                    let readout = Readout {
                        update_ready: ready,
                        needs_reinstall: warning,
                        tooltip: String::new(),
                        activity,
                        user_check_id: 0,
                        user_update_ready: false,
                    };
                    assert_eq!(
                        readout_control(Some(&readout)),
                        if ready {
                            ReadoutControl::UpdateMenu
                        } else {
                            ReadoutControl::Check { spinning: active }
                        },
                        "{:?}, ready={ready}, warning={warning}",
                        readout.activity
                    );
                    if let Some(status) = inline_status(&readout.activity, ready, None) {
                        assert_eq!(separate_progress_spinner(&readout, status), ready && active);
                    }
                }
            }
        }
    }

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
            activity: Activity::Idle,
            user_check_id: 0,
            user_update_ready: false,
        };
        assert_eq!(
            version_tooltip("1.2.3", None, Some(&state)),
            "This is Farhelm 1.2.3. When a newer version is installed, this turns red and opens the Restart to update / What's new menu."
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
            activity: Activity::Idle,
            user_check_id: 0,
            user_update_ready: false,
        };
        assert_eq!(readout_marks(&readout(false, false)), vec![]);
        assert_eq!(readout_marks(&readout(true, false)), vec![UPDATE_MARK]);
        assert_eq!(readout_marks(&readout(false, true)), vec![WARNING_MARK]);
        assert_eq!(
            readout_marks(&readout(true, true)),
            vec![WARNING_MARK, UPDATE_MARK]
        );
    }

    /// Progress and failure remain visible alongside a waiting update, while
    /// success yields to the red readout. Restart failures and idle automatic
    /// work never acquire inline check feedback.
    #[farhelm_testtrace::test]
    fn inline_feedback_follows_activity_and_waiting_update_precedence() {
        for ready in [false, true] {
            for (activity, expected) in [
                (Activity::Idle, None),
                (Activity::Checking, Some(InlineStatus::Checking)),
                (
                    Activity::Installing("1.2.0".into()),
                    Some(InlineStatus::Installing),
                ),
                (
                    Activity::UpToDate("1.2.0".into()),
                    (!ready).then_some(InlineStatus::UpToDate),
                ),
                (
                    Activity::Failed("offline".into()),
                    Some(InlineStatus::Failed),
                ),
                (Activity::RestartFailed, None),
            ] {
                assert_eq!(
                    inline_status(&activity, ready, None),
                    expected,
                    "{activity:?}, ready={ready}"
                );
            }
        }
        assert_eq!(InlineStatus::Checking.text(), "checking…");
        assert_eq!(InlineStatus::Installing.text(), "installing…");
        assert_eq!(InlineStatus::UpToDate.text(), "up to date");
        assert_eq!(InlineStatus::Failed.text(), "check failed");
        for status in [
            InlineStatus::Checking,
            InlineStatus::Installing,
            InlineStatus::UpToDate,
            InlineStatus::Failed,
        ] {
            assert_eq!(
                status.spinning(),
                matches!(status, InlineStatus::Checking | InlineStatus::Installing)
            );
        }
    }

    /// A success expires once and unrelated published changes cannot revive it.
    /// A newer result, even for the same release, must get its own full lifetime.
    #[farhelm_testtrace::test]
    fn expiry_belongs_to_one_success_not_the_next_check() {
        let success = Activity::UpToDate("1.2.0".into());
        let mut feedback = InlineFeedback::default();
        assert!(feedback.observe(&success, 1));
        assert_eq!(
            inline_status(&success, false, feedback.suppressed.as_ref()),
            Some(InlineStatus::UpToDate)
        );
        assert!(!feedback.observe(&success, 1));
        feedback.expire(&success, 1);
        assert_eq!(
            inline_status(&success, false, feedback.suppressed.as_ref()),
            None
        );
        assert!(!feedback.observe(&success, 1));
        assert_eq!(
            inline_status(&success, false, feedback.suppressed.as_ref()),
            None
        );

        feedback.observe(&Activity::Checking, 2);
        feedback.expire(&success, 1);
        assert_eq!(
            inline_status(&Activity::Checking, false, feedback.suppressed.as_ref()),
            Some(InlineStatus::Checking)
        );
        assert!(feedback.observe(&success, 2));
        feedback.expire(&success, 1);
        assert_eq!(
            inline_status(&success, false, feedback.suppressed.as_ref()),
            Some(InlineStatus::UpToDate)
        );
        feedback.expire(&success, 2);
        assert_eq!(
            inline_status(&success, false, feedback.suppressed.as_ref()),
            None
        );
    }

    /// Only a click on the current failure suppresses it. The next check clears
    /// that dismissal, including when it later fails for the identical reason.
    #[farhelm_testtrace::test]
    fn failure_dismissal_survives_refreshes_but_not_another_check() {
        let failure = Activity::Failed("offline".into());
        let mut feedback = InlineFeedback::default();
        feedback.observe(&failure, 1);
        feedback.expire(&failure, 1);
        assert_eq!(
            inline_status(&failure, true, feedback.suppressed.as_ref()),
            Some(InlineStatus::Failed)
        );
        feedback.dismiss(&Activity::Failed("different failure".into()), 1);
        assert_eq!(
            inline_status(&failure, true, feedback.suppressed.as_ref()),
            Some(InlineStatus::Failed)
        );
        feedback.dismiss(&failure, 1);
        feedback.observe(&failure, 1);
        assert_eq!(
            inline_status(&failure, true, feedback.suppressed.as_ref()),
            None
        );
        feedback.observe(&Activity::Checking, 2);
        feedback.dismiss(&failure, 1);
        assert_eq!(
            inline_status(&Activity::Checking, false, feedback.suppressed.as_ref()),
            Some(InlineStatus::Checking)
        );
        feedback.observe(&failure, 2);
        assert_eq!(
            inline_status(&failure, false, feedback.suppressed.as_ref()),
            Some(InlineStatus::Failed)
        );

        let success = Activity::UpToDate("1.2.0".into());
        feedback.observe(&success, 3);
        feedback.dismiss(&success, 3);
        assert_eq!(
            inline_status(&success, false, feedback.suppressed.as_ref()),
            Some(InlineStatus::UpToDate)
        );
    }

    /// Watch delivery can skip every intermediate activity. Request identity
    /// must still give identical outcomes a fresh lifetime and dismissal state.
    #[farhelm_testtrace::test]
    fn coalesced_identical_results_belong_to_different_requests() {
        let success = Activity::UpToDate("1.2.0".into());
        let mut feedback = InlineFeedback::default();
        assert!(feedback.observe(&success, 1));
        feedback.expire(&success, 1);
        assert_eq!(
            inline_status(&success, false, feedback.suppressed.as_ref()),
            None
        );
        assert!(feedback.observe(&success, 2));
        feedback.expire(&success, 1);
        assert_eq!(
            inline_status(&success, false, feedback.suppressed.as_ref()),
            Some(InlineStatus::UpToDate)
        );

        let failure = Activity::Failed("offline".into());
        feedback.observe(&failure, 3);
        feedback.dismiss(&failure, 3);
        assert_eq!(
            inline_status(&failure, false, feedback.suppressed.as_ref()),
            None
        );
        feedback.observe(&failure, 4);
        feedback.dismiss(&failure, 3);
        assert_eq!(
            inline_status(&failure, false, feedback.suppressed.as_ref()),
            Some(InlineStatus::Failed)
        );
    }

    /// The red toggle replaces visible success, but user-check completion still
    /// needs words in the live region. Automatic readiness and failures cannot
    /// masquerade as that successful outcome.
    #[farhelm_testtrace::test]
    fn ready_announcements_require_a_successful_user_check() {
        for update_ready in [false, true] {
            for captured in [false, true] {
                for activity in [
                    Activity::Idle,
                    Activity::Checking,
                    Activity::Installing("1.2.0".into()),
                    Activity::UpToDate("1.2.0".into()),
                    Activity::Failed("offline".into()),
                    Activity::RestartFailed,
                ] {
                    let readout = Readout {
                        update_ready,
                        needs_reinstall: false,
                        tooltip: String::new(),
                        activity: activity.clone(),
                        user_check_id: 1,
                        user_update_ready: captured,
                    };
                    assert_eq!(
                        announce_update_ready(&readout),
                        update_ready
                            && captured
                            && matches!(activity, Activity::Idle | Activity::UpToDate(_)),
                        "{activity:?}, ready={update_ready}, captured={captured}"
                    );
                }
            }
        }
    }
}
