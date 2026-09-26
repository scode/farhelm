//! A fixed-destination editor for restarting one captured conversation.
//!
//! The session view owns the restart request and its result. This dialog owns
//! only a draft of the stored launch selection, so a refusal can leave that
//! draft visible without changing the session's authoritative state.
//!
//! ## Focus: the dialog sits over a live agent
//!
//! The session view stays mounted under this modal, agent terminal included,
//! so any keystroke that lands outside the dialog can become agent input.
//! Three review rounds each found another way focus could leave, so the
//! guarantee is layered:
//!
//! 1. **Structural (primary).** While mounted, the dialog isolates itself
//!    with [`crate::modal_isolation`]: everything outside it is `inert`, and
//!    a capture-phase `keydown` net pulls stray focus back, swallowing that
//!    key (Escape still cancels unless busy). Other code calling `focus()`
//!    behind the modal is a no-op, and Tab from `body` can only reach the
//!    dialog.
//! 2. **Per-mechanism (second layer, for engines without `inert`).**
//!    `terminal.js` refuses to focus a terminal on reveal or on a selection
//!    change while `.restart-with-dialog` is mounted; the dialog's own Tab
//!    trap wraps at its first and last real tab stops; and the controls
//!    follow the rule below.
//!
//! The rule for anyone editing this dialog or the controls it renders:
//! **never natively disable or unmount the element that holds focus inside
//! the dialog.** No outside listener can catch that drop reliably: focus
//! falls to `body` with no `focusin`, and whether `blur` fires on removal or
//! disabling differs by engine. Keep such a control focusable and make it
//! unavailable with `aria-disabled` plus its handler's own guard (the busy
//! submit), keep transient elements out of the Tab order (the model list's
//! `tabindex="-1"` rows), and give the trap a focusable container to fall
//! back to. The safety net recovers from a drop on the next key, but the
//! drop still costs the user that keystroke.
//!
//! One live instance of that hazard is the submit click on WebKit. A click
//! there does not focus the button, so focus stays on whichever control the
//! user touched last until the submit handler runs
//! [`focus_restart_with_submit`]. The handler does that only when the
//! render-time `may_submit` is still true. Anything that makes `may_submit`
//! false, or natively disables the previously focused control, between the
//! pointer press and the handler (a blur-driven validation error, say) skips
//! that focus move, and the busy render then drops focus to `body`.

use dioxus::prelude::*;

use crate::api;
use crate::launch_composer::{self, ModelEnterTarget, ModelOption};
use crate::launch_controls::LaunchControls;
use crate::modal_isolation;
use crate::peer::display_peer;
use crate::{ApiBase, LaunchEffort, LaunchPermission, LaunchSelection, Session};

/// Selector for the mounted dialog, shared by its focus trap, its isolation
/// and the session view's close path that releases that isolation.
pub(crate) const RESTART_WITH_DIALOG_SELECTOR: &str = r#".restart-with-dialog[role="dialog"]"#;

/// Take focus into the modal and isolate the page behind it.
///
/// Initial focus goes to cancel, then [`crate::modal_isolation`] marks
/// everything outside the dialog `inert` and arms its keydown net (see the
/// module docs for why this dialog needs it). The order matters: inerting an
/// ancestor of whatever held focus before the dialog opened would blur it to
/// `body` instead of handing it to the dialog.
///
/// The Tab trap below is the second layer. `aria-modal` does not trap Tab in
/// every renderer, and with the rest of the page inert, native Tab past the
/// last control would leave for the browser chrome instead of wrapping. If a
/// render ever leaves nothing focusable, Tab parks focus on the dialog
/// container (`tabindex="-1"`) rather than letting the browser walk out to
/// the page behind it, which matters most in an engine without `inert`.
///
/// The container itself counts as the start of the cycle: focus parked there
/// by the fallback or by the keydown net would otherwise leave the page on
/// Shift+Tab, since everything before it is inert.
///
/// The trap counts only real tab stops. The model combobox's open option rows
/// are `tabindex="-1"` buttons (they are reached through the input's active
/// descendant, and blur unmounts them), so listing them would give the trap
/// boundaries that native Tab never visits.
fn install_focus_trap() {
    let trap = r#"(() => {
            const dialog = document.querySelector(__DIALOG__);
            if (!dialog || dialog.__farhelmRestartWithTrap) return;
            dialog.__farhelmRestartWithTrap = true;
            dialog.querySelector('.restart-with-cancel')?.focus({ preventScroll: true });
            const focusable = () => [...dialog.querySelectorAll(
                'button:not([disabled]):not([tabindex="-1"]), input:not([disabled]):not([tabindex="-1"])',
            )].filter(node => !node.hidden && node.getClientRects().length);
            dialog.addEventListener('keydown', event => {
                if (event.key !== 'Tab') return;
                const nodes = focusable();
                if (!nodes.length) {
                    event.preventDefault();
                    dialog.focus({ preventScroll: true });
                    return;
                }
                const first = nodes[0], last = nodes[nodes.length - 1];
                // The container precedes every control, so Shift+Tab from it
                // (where the fallback and the isolation's keydown net park
                // focus) wraps to the last stop like Shift+Tab from the first.
                const atStart = document.activeElement === first || document.activeElement === dialog;
                if (event.shiftKey ? atStart : document.activeElement === last) {
                    event.preventDefault();
                    (event.shiftKey ? last : first).focus();
                }
            });
        })();"#
        .replace(
            "__DIALOG__",
            &serde_json::to_string(RESTART_WITH_DIALOG_SELECTOR).expect("a string always serializes"),
        );
    let isolate =
        modal_isolation::install_js(RESTART_WITH_DIALOG_SELECTOR, Some(".restart-with-cancel"));
    document::eval(&format!("{trap}\n{isolate}"));
}

/// Put focus on the submit button as its click starts a request.
///
/// Busy natively disables every other dialog control, and a disabled control
/// cannot hold focus, so the submit button (kept enabled while busy, see the
/// footer) is the one in-dialog target that survives the request. A click
/// normally focuses it already, but WebKit on macOS does not focus a button
/// on click, which would leave focus on whichever control the user touched
/// last and let the busy render drop it to the page body. The call is gated
/// on the render-time `may_submit`, which is the ordering hazard the module
/// docs describe.
fn focus_restart_with_submit() {
    document::eval(
        "document.querySelector('.restart-with-dialog .restart-with-submit')?.focus({ preventScroll: true })",
    );
}

/// Edit only the launch fields the resumed conversation can change.
///
/// `session` is the opening snapshot and stays the comparison baseline while
/// this dialog is mounted. The parent rechecks live availability before it
/// sends a request, and passes a refusal back through `error`.
#[component]
pub(crate) fn RestartWithDialog(
    session: Session,
    busy: bool,
    error: Option<String>,
    stop_first: bool,
    stop_uncertain: bool,
    offer_label: String,
    on_submit: EventHandler<LaunchSelection>,
    on_cancel: EventHandler<()>,
) -> Element {
    let Some(baseline) = session.launch.clone() else {
        return rsx! {};
    };
    let base = use_context::<ApiBase>().0;
    let catalog_base = base.clone();
    let catalog_resource = use_resource(move || {
        let base = catalog_base.clone();
        async move { api::fetch_launch_catalog(&base).await }
    });
    let catalog_result = catalog_resource.read();
    let catalog = catalog_result
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .cloned()
        .unwrap_or_default();
    let catalog_error = catalog_result
        .as_ref()
        .and_then(|result| result.as_ref().err())
        .cloned();

    let seed = baseline.clone();
    let mut selection = use_signal(move || seed);
    let mut model_draft = use_signal(String::new);
    let mut model_open = use_signal(|| false);
    let mut model_active = use_signal(|| None::<usize>);
    let mut model_error = use_signal(|| None::<String>);
    let mut reset_reason = use_signal(|| None::<String>);
    let mut model_edited = use_signal(|| false);
    let current = selection();
    let draft_pending = model_open() && !model_draft().is_empty();
    // The catalog describes offered choices; an outage cannot establish that
    // a stored selection became invalid. The helm validates the final request.
    let compatible =
        catalog_error.is_some() || launch_composer::selection_is_compatible(&current, &catalog);
    let may_submit =
        !busy && current != baseline && !draft_pending && model_error().is_none() && compatible;
    let host = session.host_name.as_deref().unwrap_or("unknown host");
    let title = display_peer(&session.title);
    let folder = display_peer(&session.cwd);
    let harness = crate::launch_composer::harness_label(baseline.harness).to_ascii_lowercase();

    let apply_option = Callback::<ModelOption>::new({
        let catalog = catalog.clone();
        move |option| {
            let model = match option {
                ModelOption::HarnessDefault => None,
                ModelOption::Model { id, harness } if harness == baseline.harness => Some(id),
                _ => return,
            };
            let before = selection();
            let mut after = before.clone();
            after.model = model;
            if !launch_composer::compatible_efforts(after.harness, after.model.as_deref(), &catalog)
                .contains(&after.effort.unwrap_or(LaunchEffort::Off))
                && after.effort.is_some()
                && !catalog.is_empty()
            {
                after.effort = None;
            }
            reset_reason.set(launch_composer::reconciliation_reset_reason(
                &before, &after,
            ));
            selection.set(after);
            model_edited.set(true);
            model_draft.set(String::new());
            model_open.set(false);
            model_active.set(None);
            model_error.set(None);
        }
    });

    rsx! {
        div { class: "restart-with-backdrop", role: "presentation",
            div {
                class: "restart-with-dialog",
                role: "dialog",
                aria_modal: "true",
                aria_label: "restart with · {title}",
                // Programmatically focusable only: where the focus trap's
                // fallback and the isolation's keydown net put focus back.
                // `-1` keeps the container itself out of the Tab order.
                tabindex: "-1",
                onmounted: move |_| install_focus_trap(),
                onkeydown: move |evt: KeyboardEvent| {
                    if evt.key() == Key::Escape && !evt.is_composing() && !busy {
                        on_cancel.call(());
                    }
                },
                h2 { class: "restart-with-title", "restart with · {title}" }
                dl { class: "restart-with-context",
                    dt { "harness" } dd { "{harness}" }
                    dt { "host" } dd { "{display_peer(host)}" }
                    dt { "folder" } dd { "{folder}" }
                    dt { "resumes" } dd { "this session's conversation" }
                }
                p { class: "restart-with-fixed-note", "harness, host and folder stay fixed; use replace with for another harness or folder, or clone for another host" }
                hr { class: "restart-with-rule" }
                LaunchControls {
                    harness: Some(baseline.harness),
                    model: current.model.clone(),
                    model_raw_seed: baseline.model.clone(),
                    model_edited: model_edited(),
                    effort: current.effort,
                    permissions: current.permissions,
                    workspace_trust: current.workspace_trust,
                    catalog: catalog.clone(),
                    busy,
                    model_draft: model_draft(),
                    model_open: model_open(),
                    model_active: model_active(),
                    model_show_all: false,
                    model_draft_error: model_error(),
                    choice_error: if compatible { None } else { Some("the selected settings are not compatible".to_string()) },
                    reset_reason: reset_reason(),
                    model_id_prefix: "restart-with-model".to_string(),
                    baseline: Some(baseline.clone()),
                    fixed_harness: true,
                    on_model_focus: move |_| {
                        model_draft.set(String::new());
                        model_open.set(true);
                        model_active.set(None);
                        model_error.set(None);
                    },
                    on_model_input: move |value: String| {
                        model_error.set((!value.is_empty()).then(|| "press Enter to choose this model".to_string()));
                        model_draft.set(value);
                        model_open.set(true);
                        model_active.set(None);
                    },
                    on_model_blur: move |_| {
                        if !model_draft().is_empty() {
                            model_error.set(Some("model change was not selected; choose a model".to_string()));
                        }
                        model_draft.set(String::new());
                        model_open.set(false);
                        model_active.set(None);
                    },
                    on_model_escape: move |_| {
                        model_draft.set(String::new());
                        model_open.set(false);
                        model_active.set(None);
                        model_error.set(None);
                    },
                    on_model_active: move |index| model_active.set(index),
                    on_model_enter: {
                        move |target| match target {
                            ModelEnterTarget::Nothing => {
                                model_draft.set(String::new());
                                model_open.set(false);
                            }
                            ModelEnterTarget::Option(option) => apply_option.call(option),
                            ModelEnterTarget::Custom { id, harness } if harness == baseline.harness => {
                                apply_option.call(ModelOption::Model { id, harness });
                            }
                            ModelEnterTarget::Custom { .. } | ModelEnterTarget::NeedsHarness(_) => {
                                model_error.set(Some("choose a model for this harness".to_string()));
                            }
                        }
                    },
                    on_model_option: move |option| apply_option.call(option),
                    on_effort: move |effort| {
                        let mut after = selection();
                        after.effort = effort;
                        selection.set(after);
                        reset_reason.set(None);
                    },
                    on_permissions: move |permissions: Option<LaunchPermission>| {
                        let mut after = selection();
                        after.permissions = permissions;
                        selection.set(after);
                    },
                    on_workspace_trust: move |workspace_trust| {
                        let mut after = selection();
                        after.workspace_trust = workspace_trust;
                        selection.set(after);
                    },
                }
                if let Some(message) = catalog_error {
                    p { class: "restart-with-catalog-error", role: "status", "model catalog unavailable: {message}" }
                }
                if let Some(message) = error {
                    p { class: "restart-with-error", role: "alert", "{message}" }
                }
                div { class: "restart-with-footer",
                    if stop_first {
                        p { class: "restart-with-stop-note",
                            if stop_uncertain {
                                "agent may be running; it is stopped first"
                            } else {
                                "agent is running; it is stopped first"
                            }
                        }
                    }
                    button {
                        class: "btn btn-neutral restart-with-cancel",
                        r#type: "button",
                        disabled: busy,
                        onclick: move |_| on_cancel.call(()),
                        "cancel"
                    }
                    // While a request is in flight this button is unavailable
                    // through `aria-disabled` and its handler guards (here and
                    // in the parent's `restarting()` check), NOT through native
                    // `disabled`. It is the focused control of a
                    // click-submitted request, and a natively disabled button
                    // would drop that focus to the page body (the module docs'
                    // rule). `RenameForm`'s save follows the same rule. `busy`
                    // already makes `may_submit` false, so `!busy` below only
                    // lifts the native attribute.
                    button {
                        class: "btn btn-primary restart-with-submit",
                        r#type: "button",
                        title: "{offer_label}",
                        disabled: !may_submit && !busy,
                        aria_disabled: if busy { "true" },
                        onclick: move |_| {
                            if may_submit {
                                focus_restart_with_submit();
                                on_submit.call(selection());
                            }
                        },
                        if stop_first { "stop and restart" } else { "restart" }
                    }
                }
            }
        }
    }
}
