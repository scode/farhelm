//! The updater's state as words and marks: the pure presentation the desktop app shows for it.
//!
//! The updater itself is [`crate::updater`]. This module turns its published state into what a window shows next to
//! the version (the readout, its hover text, the short inline status after a check, and the marks that lead it), with
//! no UI framework involved, so the wording and its rules are tested without a window. It moved here unchanged from the
//! old Dioxus UI crate, minus that UI's components and hooks; the GPUI app renders the same values.

/// The updater activity exposed to presentation, without a renderer dependency.
///
/// The native worker publishes non-idle check states only for a run the user
/// requested or joined.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Activity {
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
pub struct Readout {
    /// A newer version is installed than the one running and is not known to
    /// be withdrawn: the readout turns red with an up-arrow.
    pub update_ready: bool,
    /// This app can no longer verify its updates and must be reinstalled:
    /// the readout carries a warning mark, and the hover text says how.
    pub needs_reinstall: bool,
    /// A running or waiting release was withdrawn. The same warning mark can
    /// carry both withdrawal and reinstall advice; the hover preserves both.
    pub withdrawn: bool,
    /// The readout's hover text.
    pub tooltip: String,
    /// Activity identity, kept apart from installation and verification changes
    /// so those changes cannot restart inline expiry or undo a dismissal.
    pub activity: Activity,
    /// Identity of a visible request or qualifying automatic failure,
    /// including when watch delivery skips intermediate publications.
    pub notice_id: u64,
    /// Readiness captured at successful user-check completion, so automatic or
    /// external installations do not acquire a check-completion announcement.
    pub user_update_ready: bool,
}

/// Which action the version offers, with motion only for user-visible work.
///
/// The absence of an updater (an uninstalled build) keeps the version plain text.
/// A waiting update owns the menu even during another check; only a check
/// button owns the refresh glyph, so that menu still needs a separate spinner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadoutControl {
    Plain,
    Check { spinning: bool },
    UpdateMenu,
}

/// Choose the version's control from the same published state as inline status.
/// Reinstall warnings retain their mark without taking away the retry action.
pub fn readout_control(readout: Option<&Readout>) -> ReadoutControl {
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
pub fn separate_progress_spinner(readout: &Readout, shown: InlineStatus) -> bool {
    shown.spinning() && !matches!(readout_control(Some(readout)), ReadoutControl::Check { .. })
}

/// Short inline words and styling, independent of the hover and version marks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineStatus {
    Checking,
    Installing,
    UpToDate,
    Failed,
}

impl InlineStatus {
    /// The same words are visible and announced by the mounted live region.
    pub fn text(self) -> &'static str {
        match self {
            Self::Checking => "Checking…",
            Self::Installing => "Installing…",
            Self::UpToDate => "Up to date",
            Self::Failed => "Check failed",
        }
    }

    /// Only active work needs motion; the existing spinner handles reduced motion.
    pub fn spinning(self) -> bool {
        matches!(self, Self::Checking | Self::Installing)
    }
}

/// Hide only the exact outcome dismissed or expired in this component.
///
/// An installed update owns the up-to-date answer, but not active progress or a
/// failed check. Restart failures remain hover-only. Reinstall warnings and build
/// skew have their existing independent presentation and do not alter this choice.
pub fn inline_status(
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

/// The inline feedback's actual in-flow contribution after local suppression.
///
/// Hidden readiness announcements, outcome identity and hover details occupy no
/// width. Keeping them out of this value lets an unchanged visible status retain
/// an update menu measured against it. Spinner animation is likewise irrelevant;
/// only the presence of the separate ring contributes another inline element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InlinePresentation {
    pub shown: Option<InlineStatus>,
    pub separate_progress_spinner: bool,
}

/// Resolve the same status words and ring that the status component renders.
/// Suppression belongs to that component, so callers must supply its current
/// dismissed or expired activity rather than deriving visibility from readout alone.
pub fn inline_presentation(readout: &Readout, suppressed: Option<&Activity>) -> InlinePresentation {
    let shown = inline_status(&readout.activity, readout.update_ready, suppressed);
    InlinePresentation {
        shown,
        separate_progress_spinner: shown
            .is_some_and(|shown| separate_progress_spinner(readout, shown)),
    }
}

/// Announce a successful waiting update without competing with its red readout.
/// Failure and progress own their visible words; readiness from background work
/// alone carries no user-check provenance and must remain silent.
pub fn announce_update_ready(readout: &Readout) -> bool {
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
pub struct InlineFeedback {
    activity: Activity,
    notice_id: u64,
    suppressed: Option<Activity>,
}

impl InlineFeedback {
    /// Observe a new check/activity and request expiry only for a new success.
    /// Installation-record changes with unchanged activity must not stretch its
    /// five-second lifetime or revive a failure the user dismissed.
    pub fn observe(&mut self, activity: &Activity, notice_id: u64) -> bool {
        if self.activity == *activity && self.notice_id == notice_id {
            return false;
        }
        self.activity = activity.clone();
        self.notice_id = notice_id;
        self.suppressed = None;
        matches!(activity, Activity::UpToDate(_))
    }

    /// Expire the success that scheduled this callback, never a later result.
    pub fn expire(&mut self, activity: &Activity, notice_id: u64) {
        if self.notice_id == notice_id
            && self.activity == *activity
            && matches!(activity, Activity::UpToDate(_))
        {
            self.suppressed = Some(self.activity.clone());
        }
    }

    /// Dismiss only the failure currently visible; progress and success do not
    /// acquire an accidental dismissal path through a stale click handler.
    pub fn dismiss(&mut self, activity: &Activity, notice_id: u64) {
        if self.notice_id == notice_id
            && self.activity == *activity
            && matches!(activity, Activity::Failed(_))
        {
            self.suppressed = Some(activity.clone());
        }
    }
}

/// One glyph leading the version readout, with the class that styles it.
/// Hidden from screen readers; the readout's label or hidden text says the
/// same in words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadoutMark {
    pub glyph: &'static str,
    pub class: &'static str,
}

/// The warning that this app can no longer verify its updates.
pub const WARNING_MARK: ReadoutMark = ReadoutMark {
    glyph: "⚠",
    class: "app-version-warning",
};

/// The marker that a restart finishes an update.
pub const UPDATE_MARK: ReadoutMark = ReadoutMark {
    glyph: "↑",
    class: "app-version-arrow",
};

/// The glyphs that lead the readout, in order, whichever element shows it
/// (the plain readout, or the update menu's toggle while an update waits).
///
/// Both renderings take their marks from here so that neither can drop one:
/// an update can wait while a newer release failed verification, and the
/// warning must not vanish behind the update marker then.
pub fn readout_marks(readout: &Readout) -> Vec<ReadoutMark> {
    let mut marks = Vec::new();
    if readout.needs_reinstall || readout.withdrawn {
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
pub fn idle_tooltip(running: &str, has_updater: bool) -> String {
    if farhelm_proto::is_development_build(running) {
        "This is a development build of Farhelm, not a release.".to_owned()
    } else if has_updater {
        let running = crate::local_host::display_peer(running);
        format!(
            "This is Farhelm {running}. When a newer version is installed, this turns red and opens the Restart to update / What's new menu."
        )
    } else {
        let running = crate::local_host::display_peer(running);
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
pub fn version_tooltip(running: &str, reported: Option<&str>, updater: Option<&Readout>) -> String {
    if let Some(reported) = reported.filter(|reported| *reported != running) {
        let helm = crate::local_host::display_peer(reported);
        if farhelm_proto::is_development_build(running) {
            return format!(
                "The helm runs Farhelm {helm}; this window is a development build of Farhelm."
            );
        }
        let window = crate::local_host::display_peer(running);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Movement follows the visible words and ring after dismissal/expiry.
    /// Readiness announcements, notice identity and failure details must not
    /// acquire width merely because the native worker published a new readout.
    #[farhelm_testtrace::test]
    fn inline_presentation_tracks_only_rendered_feedback() {
        let mut readout = Readout {
            update_ready: true,
            needs_reinstall: false,
            withdrawn: false,
            tooltip: String::new(),
            activity: Activity::Idle,
            notice_id: 1,
            user_update_ready: false,
        };
        let empty = InlinePresentation::default();
        assert_eq!(inline_presentation(&readout, None), empty);
        readout.user_update_ready = true;
        assert!(announce_update_ready(&readout));
        assert_eq!(inline_presentation(&readout, None), empty);

        for (activity, shown) in [
            (Activity::Checking, InlineStatus::Checking),
            (
                Activity::Installing("1.2.4".into()),
                InlineStatus::Installing,
            ),
        ] {
            readout.activity = activity;
            assert_eq!(
                inline_presentation(&readout, None),
                InlinePresentation {
                    shown: Some(shown),
                    separate_progress_spinner: true
                },
            );
            readout.update_ready = false;
            assert_eq!(
                inline_presentation(&readout, None),
                InlinePresentation {
                    shown: Some(shown),
                    separate_progress_spinner: false
                },
            );
            readout.update_ready = true;
        }

        readout.activity = Activity::Failed("offline".into());
        let failure = inline_presentation(&readout, None);
        assert_eq!(failure.shown, Some(InlineStatus::Failed));
        assert!(!failure.separate_progress_spinner);
        readout.activity = Activity::Failed("different reason".into());
        readout.notice_id += 1;
        readout.tooltip = "different hover".into();
        readout.needs_reinstall = true;
        assert_eq!(inline_presentation(&readout, None), failure);

        let mut feedback = InlineFeedback::default();
        feedback.observe(&readout.activity, readout.notice_id);
        feedback.dismiss(&readout.activity, readout.notice_id);
        assert_eq!(
            inline_presentation(&readout, feedback.suppressed.as_ref()),
            empty
        );
        readout.activity = Activity::UpToDate("1.2.3".into());
        assert_eq!(inline_presentation(&readout, None), empty);
        readout.update_ready = false;
        assert_eq!(
            inline_presentation(&readout, None).shown,
            Some(InlineStatus::UpToDate)
        );
        feedback.observe(&readout.activity, readout.notice_id);
        feedback.expire(&readout.activity, readout.notice_id);
        assert_eq!(
            inline_presentation(&readout, feedback.suppressed.as_ref()),
            empty
        );
    }

    /// Spec: the version control is plain without an updater, the update menu
    /// once an update is ready, and otherwise a check button that spins while
    /// work runs; a reinstall warning never takes the retry away. The status
    /// line's own spinner shows only when the control is the update menu, so
    /// a running check is never shown spinning twice.
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
                        withdrawn: false,
                        tooltip: String::new(),
                        activity,
                        notice_id: 0,
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

    /// An uninstalled desktop build has no update control to promise.
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
            withdrawn: false,
            tooltip: idle_tooltip("1.2.3", true),
            activity: Activity::Idle,
            notice_id: 0,
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

    /// Spec: either a withdrawal or reinstall notice leads with one warning
    /// mark, followed by the arrow for a safe waiting update. Withdrawal alone
    /// must be visible; coexistence must neither duplicate nor hide its mark.
    ///
    /// Why: the first version of the toggle drew only the arrow, so a
    /// verification failure during a waiting update showed no warning at
    /// all; both renderings now take their marks from this one function.
    #[farhelm_testtrace::test]
    fn the_warning_mark_survives_a_waiting_update() {
        let readout = |update_ready, needs_reinstall, withdrawn| Readout {
            update_ready,
            needs_reinstall,
            withdrawn,
            tooltip: String::new(),
            activity: Activity::Idle,
            notice_id: 0,
            user_update_ready: false,
        };
        for (ready, reinstall, withdrawn, expected) in [
            (false, false, false, vec![]),
            (true, false, false, vec![UPDATE_MARK]),
            (false, true, false, vec![WARNING_MARK]),
            (true, true, false, vec![WARNING_MARK, UPDATE_MARK]),
            (false, false, true, vec![WARNING_MARK]),
            (true, false, true, vec![WARNING_MARK, UPDATE_MARK]),
            (false, true, true, vec![WARNING_MARK]),
            (true, true, true, vec![WARNING_MARK, UPDATE_MARK]),
        ] {
            assert_eq!(
                readout_marks(&readout(ready, reinstall, withdrawn)),
                expected,
                "ready={ready}, reinstall={reinstall}, withdrawn={withdrawn}"
            );
        }
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
        assert_eq!(InlineStatus::Checking.text(), "Checking…");
        assert_eq!(InlineStatus::Installing.text(), "Installing…");
        assert_eq!(InlineStatus::UpToDate.text(), "Up to date");
        assert_eq!(InlineStatus::Failed.text(), "Check failed");
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
                        withdrawn: false,
                        tooltip: String::new(),
                        activity: activity.clone(),
                        notice_id: 1,
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
