//! The loud confirmation a YOLO launch asks for on a host that asks first.
//!
//! The helm refuses to start a YOLO or own-setting session on a host
//! the user has not set to start YOLO sessions
//! without asking, and says so with a header only its own code sets
//! (`api::asks_yolo_confirmation`). Every GUI flow that can hit that refusal
//! (create, replace, replace with, restart with) shows this block instead of
//! an ordinary error. The refused request started nothing, though in the
//! launcher an earlier attempt under the same request may have (the
//! `may_have_started` prop says so).
//!
//! Deliberately hard to miss: accidental YOLO launches on the wrong machine
//! are the failure it exists to stop, and a subtle prompt is one people
//! click through. For the same reason it scrolls itself into view when it
//! appears: in the launcher it once rendered below the dialog's visible edge,
//! so pressing Launch seemed to do nothing until the user scrolled.
//!
//! It explains itself rather than assuming the user knows the term: what
//! YOLO means, why THIS launch is one (the user picked YOLO where the harness
//! offers other modes, the harness offers only YOLO, the command was asserted
//! to bypass checks, or the agent's own setting cannot be classified), and
//! that nothing has started. The helm's own
//! refusal sentence is not shown: it is written for the command line (it
//! names a CLI flag), and the GUI has everything it needs to say it better.
//!
//! The launcher and Restart with can also offer "start guarded session instead"
//! when the user explicitly chose YOLO on an agent that offers guarded mode.
//! That answer changes the draft and submits it without YOLO consent or a host
//! setting write. Plain Replace keeps the stored launch, so has no such answer.
//!
//! "Start YOLO session anyway" is a one-off override. "Start,
//! and don't ask again on this host" first sets the host to start YOLO
//! sessions without asking (the same helm write as the host settings
//! checkbox), then starts the same way; the flow that owns the launch does both, in that order, and
//! launches nothing if the first step fails. Cancel has the initial focus.
//! The one-off keeps the filled danger style and the permanent choice is only
//! outlined: a filled button draws the reflex click, and the choice that
//! turns the question off for good should not be the reflex target.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::modal_dialog::DialogActions;

use dioxus::prelude::*;
use farhelm_proto::{LaunchHarness, LaunchPermission, LaunchSelection};

use crate::HostId;
use crate::peer::{DetailPart, PeerLine, display_peer};

/// Why the launch being confirmed counts as YOLO, as far as the GUI can tell
/// from what it sent. The helm has already decided this launch requires the
/// conservative YOLO gate, so this only picks the explanation; own-setting
/// remains distinct from a known bypass mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum YoloReason {
    /// Farhelm never infers safety from user-supplied permission words.
    CustomFlags,
    /// A structured launch whose harness offers other permission modes, with
    /// YOLO chosen.
    ChosenPermission,
    /// A structured launch on a harness that offers only YOLO.
    OnlyMode(LaunchHarness),
    /// A command launch its user (or an agent) asserted is YOLO. Farhelm
    /// never reads the command to check the assertion.
    Asserted,
    /// A stored launch or Cursor request whose vendor configuration decides.
    OwnSetting,
}

impl YoloReason {
    /// The reason for a launch described by its agent selection, or by
    /// `None` for a command launch, which reaches this question only when
    /// it was asserted YOLO.
    ///
    /// Derives the wording from the harness capability table rather than
    /// naming harnesses here, so adding a single-mode harness gets the right
    /// sentence without touching this.
    pub(crate) fn of_launch(selection: Option<&LaunchSelection>) -> Self {
        match selection {
            None => YoloReason::Asserted,
            Some(selection)
                if selection.permissions == Some(farhelm_proto::LaunchPermission::Custom) =>
            {
                YoloReason::CustomFlags
            }
            Some(selection) if selection.harness.offers_only_yolo() => {
                YoloReason::OnlyMode(selection.harness)
            }
            Some(selection)
                if selection
                    .harness
                    .stored_permission(selection.permissions)
                    .is_none() =>
            {
                YoloReason::OwnSetting
            }
            Some(_) => YoloReason::ChosenPermission,
        }
    }

    /// Offer a draft change only for an explicit YOLO choice with a guarded mode.
    ///
    /// The reason belongs to the refused request, while the selection supplies
    /// its harness capability. An own-setting launch, custom flags or a command
    /// must never acquire a different launch merely because it reached this box.
    pub(crate) fn can_launch_guarded(self, selection: Option<&LaunchSelection>) -> bool {
        self == Self::ChosenPermission
            && selection.is_some_and(|selection| {
                selection.permissions == Some(LaunchPermission::Yolo)
                    && selection
                        .harness
                        .offers_permission(LaunchPermission::Guarded)
            })
    }

    /// The sentence the confirmation shows for this reason.
    fn sentence(self) -> String {
        match self {
            YoloReason::CustomFlags => "This launch uses custom permission flags. Farhelm does not check what they do, so it treats the session as YOLO.".into(),
            YoloReason::ChosenPermission => "This launch uses YOLO permissions. Another permission mode would keep the agent's safety checks instead.".to_string(),
            YoloReason::OnlyMode(harness) => {
                let name = crate::launch_composer::harness_label(harness);
                format!("Farhelm offers {name} only in YOLO mode, so every {name} session runs this way.")
            }
            YoloReason::Asserted => {
                "This command was asserted to bypass its safety checks.".to_string()
            }
            YoloReason::OwnSetting => "This launch uses the agent's own setting, which Farhelm cannot override or classify. It is treated as YOLO on this host.".to_string(),
        }
    }
}

/// What a refused launch carries into the confirmation: the host it targeted
/// and why it counts as YOLO. Captured when the refusal arrives, from the
/// request that was sent, so the question describes that launch even if the
/// page has moved on since.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct YoloAsk {
    /// The host to stop asking on for "don't ask again". `None` when the flow does
    /// not know it (a session row from a helm that sent no host), in which
    /// case that button is not offered.
    pub(crate) host: Option<HostId>,
    /// The host's display name, as the rest of the GUI shows it.
    pub(crate) host_name: String,
    pub(crate) reason: YoloReason,
}

/// The first step of "start, and don't ask again on this host": set `host`
/// to start YOLO sessions without asking, the same helm write as the host
/// settings checkbox.
///
/// Every flow runs this inside the task that sends its launch, before the
/// launch and under whatever operation claim that task already holds, and
/// sends the launch only when this returns `Ok`. A reply this build could not
/// decode still means the write committed, so it counts as success. The
/// error, when it fails, is the sentence the confirmation shows: the helm's
/// reason plus what did not happen, because "nothing started and the host
/// still asks" is the part the user decides from.
pub(crate) async fn stop_asking(base: &str, host: HostId, host_name: &str) -> Result<(), String> {
    match crate::api::set_yolo_without_asking(base, host, true).await {
        Ok(_) => Ok(()),
        // "May still ask": an error can also be a lost reply to a write
        // that committed, so the host's state is not known here. Retrying is
        // harmless, the write is idempotent.
        Err(reason) => Err(format!(
            "Could not stop asking for {host_name}: {}. This answer started nothing; {host_name} may still ask before YOLO launches.",
            reason.trim_end_matches('.')
        )),
    }
}

/// A distinct id for each mounted confirmation, rendered as
/// `data-yolo-confirm-id` so the scripts below act on THIS block. Several can
/// be up at once (a sidebar replace's question stays while the user starts
/// another flow), and a page-wide `.yolo-confirmation` selector would reach
/// whichever comes first in the document.
static NEXT_INSTANCE: AtomicU64 = AtomicU64::new(1);

/// The selector for one confirmation's element, as a JavaScript string.
fn instance_selector(instance: u64) -> String {
    serde_json::to_string(&format!(r#"[data-yolo-confirm-id="{instance}"]"#))
        .expect("a string always serializes")
}

/// Scroll this confirmation into view, as little as it takes.
///
/// `scrollIntoView` with `nearest` does not move at all when the block is
/// already fully visible. Done through a script rather than Dioxus's
/// `MountedData::scroll_to_with_options`, because the desktop renderer
/// passes that call's options to `scrollIntoView` under field names it does
/// not read, which falls back to aligning the block with the top of every
/// scrollable ancestor; in the launcher that pushed the Launch row above it
/// out of view.
fn scroll_into_view(instance: u64) {
    let selector = instance_selector(instance);
    document::eval(&format!(
        "document.querySelector({selector})?.scrollIntoView({{ block: 'nearest', inline: 'nearest', behavior: 'instant' }})"
    ));
}

/// Focus this confirmation's cancel button, its safe default answer.
///
/// `only_if_lost` limits it to recovering focus that fell to the page body
/// (a button the user pressed was disabled while its request ran), so it
/// never pulls focus away from a control the user moved to on purpose.
/// Reveal the answer itself: a bounded notice region can clip Cancel even
/// after the larger question block has scrolled into view.
fn focus_cancel(instance: u64, only_if_lost: bool) {
    let selector = instance_selector(instance);
    let guard = if only_if_lost {
        "const active = document.activeElement; if (active && active !== document.body) return;"
    } else {
        ""
    };
    document::eval(&format!(
        "requestAnimationFrame(() => {{ {guard} const cancel = document.querySelector({selector})?.querySelector('.yolo-cancel'); cancel?.focus({{ preventScroll: true }}); cancel?.scrollIntoView({{ block: 'nearest', inline: 'nearest', behavior: 'instant' }}); }})"
    ));
}

/// The confirmation block.
///
/// `busy` disables every answer while either step of a confirmed launch
/// is in flight. `error` is the reason a "don't ask again" failed at its
/// first step, shown with the question so the user can pick again.
/// `confirm_submits` opts both YOLO answers into native form submission; false
/// leaves resubmission entirely to the caller's handlers, as current callers
/// require for their opening-bound retries. A supplied guarded handler owns its draft change
/// and resubmission, including rejecting a stale opening before any mutation.
/// `may_have_started` is for a refused retry of an attempt whose outcome is
/// unknown: the question then cannot say that nothing has been started.
/// Answers follow native reading order: Cancel, optional guarded start,
/// permanent YOLO, then one-time YOLO. This stays inline; the shared action
/// row adds no modal boundary or additional close target.
#[component]
pub(crate) fn YoloConfirmation(
    ask: YoloAsk,
    #[props(default)] may_have_started: bool,
    busy: bool,
    error: Option<String>,
    confirm_submits: bool,
    #[props(default)] on_launch_guarded: Option<EventHandler<()>>,
    on_confirm: EventHandler<()>,
    on_confirm_and_stop_asking: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    let host = display_peer(&ask.host_name);
    let start_type = if confirm_submits { "submit" } else { "button" };
    let instance = use_hook(|| NEXT_INSTANCE.fetch_add(1, Ordering::Relaxed));

    // Cancel gets the initial focus, as the safe answer, but only once the
    // buttons are usable: a question can mount while the refused request is
    // still finishing, with every button disabled. Not the `autofocus`
    // attribute, whose timing differs by engine: WebKit keeps a disabled
    // autofocus element pending and focuses it whenever it becomes enabled,
    // which can be after the user has already moved focus to another answer.
    // Later, whenever a request that disabled the buttons ends with the
    // question still up (a failed "don't ask again"), focus that fell to the
    // page body comes back to cancel.
    let mut focused_once = use_signal(|| false);
    let mut last_busy = use_signal(|| busy);
    use_effect(use_reactive((&busy,), move |(busy,)| {
        let was_busy = *last_busy.peek();
        last_busy.set(busy);
        if busy {
            return;
        }
        if !*focused_once.peek() {
            focused_once.set(true);
            focus_cancel(instance, false);
        } else if was_busy {
            focus_cancel(instance, true);
        }
    }));
    rsx! {
        div {
            class: "yolo-confirmation",
            role: "alertdialog",
            aria_label: "Confirm a YOLO launch",
            "data-yolo-confirm-id": "{instance}",
            // Instant rather than smooth: the point is that it is on screen
            // by the time the user looks. See `scroll_into_view`.
            onmounted: move |_| scroll_into_view(instance),
            p { class: "yolo-confirmation-heading", "Confirm YOLO launch" }
            p { class: "yolo-confirmation-consequence",
                "YOLO means the agent bypasses its safety checks: no person, classifier, AI reviewer or sandbox stops its actions on "
                span { class: "peer-value", dir: "ltr", "{host}" }
                " without asking you."
            }
            p { class: "yolo-confirmation-reason", "{ask.reason.sentence()}" }
            p { class: "yolo-confirmation-asks",
                span { class: "peer-value", dir: "ltr", "{host}" }
                if may_have_started {
                    " asks before every YOLO launch. An earlier attempt at this launch may already have started it."
                } else {
                    " asks before every YOLO launch. Nothing has been started."
                }
            }
            if let Some(error) = error {
                PeerLine {
                    class: "yolo-confirmation-error action-error".to_string(),
                    parts: vec![DetailPart::peer(error)],
                }
            }
            DialogActions { class: "yolo-confirmation-actions",
                button {
                    r#type: "button",
                    class: "btn btn-neutral yolo-cancel",
                    "data-tooltip": "Cancel: go back without starting anything",
                    disabled: busy,
                    onclick: move |_| on_cancel.call(()),
                    "Cancel"
                }
                if let Some(on_launch_guarded) = on_launch_guarded {
                    button {
                        r#type: "button",
                        class: "btn btn-primary yolo-launch-guarded",
                        "data-tooltip": "Start this session in guarded mode instead, where the agent's own safety checks stay on",
                        disabled: busy,
                        onclick: move |_| on_launch_guarded.call(()),
                        "Start guarded session instead"
                    }
                }
                if ask.host.is_some() {
                    button {
                        r#type: start_type,
                        class: "btn btn-danger btn-outline yolo-confirm-stop-asking",
                        "data-tooltip": "Start it, and start YOLO sessions on this host without asking from now on",
                        disabled: busy,
                        onclick: move |_| on_confirm_and_stop_asking.call(()),
                        "Start, and don't ask again on this host"
                    }
                }
                button {
                    r#type: start_type,
                    class: "btn btn-danger yolo-confirm",
                    "data-tooltip": "Start the session in YOLO mode, where the agent acts without asking",
                    disabled: busy,
                    onclick: move |_| on_confirm.call(()),
                    "Start YOLO session anyway"
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Keep each case focused on permission provenance and the harness table,
    /// without unrelated model or wrapper choices influencing availability.
    fn selection(harness: LaunchHarness, permissions: Option<LaunchPermission>) -> LaunchSelection {
        LaunchSelection {
            permission_flags: None,
            harness,
            model: None,
            effort: None,
            permissions,
            wrapper_prefix: None,
            container: None,
            workspace_trust: None,
        }
    }

    /// A guarded answer must change an explicit YOLO choice, never guess what
    /// custom flags, an omitted permission or a command would become. Capability
    /// comes from the shared harness table rather than another list in the UI.
    #[test]
    fn guarded_answers_require_an_explicit_supported_yolo_choice() {
        let yolo = selection(LaunchHarness::Codex, Some(LaunchPermission::Yolo));
        assert!(YoloReason::ChosenPermission.can_launch_guarded(Some(&yolo)));
        assert!(
            !YoloReason::ChosenPermission.can_launch_guarded(Some(&selection(
                LaunchHarness::Pi,
                Some(LaunchPermission::Yolo),
            )))
        );
        for permission in [
            None,
            Some(LaunchPermission::Custom),
            Some(LaunchPermission::Guarded),
        ] {
            assert!(
                !YoloReason::ChosenPermission
                    .can_launch_guarded(Some(&selection(LaunchHarness::Codex, permission,)))
            );
        }
        for reason in [
            YoloReason::CustomFlags,
            YoloReason::OnlyMode(LaunchHarness::Pi),
            YoloReason::Asserted,
            YoloReason::OwnSetting,
        ] {
            assert!(!reason.can_launch_guarded(Some(&yolo)));
        }
        assert!(!YoloReason::ChosenPermission.can_launch_guarded(None));
    }

    /// The confirmation tells the user WHY the launch is YOLO, and its distinct
    /// reasons call for different reactions: pick another permission mode,
    /// accept that this harness offers only YOLO, or fix the command line. A wrong
    /// reason sends the user looking for a setting that does not exist.
    /// Specifies: no selection means a command line; a harness
    /// whose only mode is YOLO gets the only-mode sentence naming it; any
    /// other structured launch was YOLO by choice, except custom flags whose
    /// permission meaning Farhelm does not interpret.
    #[test]
    fn the_reason_follows_the_kind_of_launch_that_was_sent() {
        assert_eq!(
            YoloReason::of_launch(Some(&selection(
                LaunchHarness::Cursor,
                Some(LaunchPermission::Custom)
            ))),
            YoloReason::CustomFlags,
            "custom takes priority even on a harness without a guarded mode"
        );
        assert!(
            YoloReason::CustomFlags
                .sentence()
                .contains("does not check what they do")
        );
        assert_eq!(YoloReason::of_launch(None), YoloReason::Asserted);
        assert_eq!(
            YoloReason::of_launch(Some(&selection(LaunchHarness::Pi, None))),
            YoloReason::OnlyMode(LaunchHarness::Pi),
            "Pi's only mode is YOLO, whether or not the selection says so"
        );
        assert_eq!(
            YoloReason::of_launch(Some(&selection(
                LaunchHarness::Claude,
                Some(LaunchPermission::Yolo)
            ))),
            YoloReason::ChosenPermission
        );
        assert_eq!(
            YoloReason::OnlyMode(LaunchHarness::Pi).sentence(),
            "Farhelm offers Pi only in YOLO mode, so every Pi session runs this way."
        );
        assert_eq!(
            YoloReason::of_launch(Some(&selection(LaunchHarness::OpenCode, None))),
            YoloReason::OnlyMode(LaunchHarness::OpenCode)
        );
    }
}
