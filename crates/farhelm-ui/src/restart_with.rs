//! A fixed-destination editor for restarting one captured conversation.
//!
//! The session view owns the restart request and its result. This dialog owns
//! only a draft of the stored launch, so a refusal can leave that draft
//! visible without changing the session's authoritative state. It edits
//! whichever launch kind the session has (SPEC.md, Restart with): an agent
//! launch's model, effort, permissions and workspace trust, or a command
//! launch's command, resume command and YOLO answer. The launch kind and the
//! agent type stay fixed in both.
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
//! live draft is valid and changed. Every admitted primary action moves focus
//! before asking the parent to restart, including an Enter that just applied
//! a choice. Before that choice renders, the submit can still be disabled;
//! the dialog container is then the surviving focus target. Validation and
//! focus transfer must stay in that shared path.

use dioxus::prelude::*;

use crate::api;
use crate::launch_composer::{self, ModelEnterTarget, ModelOption};
use crate::launch_controls::{LaunchControls, enter_choice};
use crate::modal_isolation;
use crate::peer::display_peer;
use crate::{ApiBase, CommandLaunch, LaunchEffort, LaunchPermission, LaunchSelection, Session};

/// The changed launch Restart with sends, one per launch kind.
///
/// An agent launch sends only its new choices, which the helm composes into
/// commands exactly as it does for a create; a command launch sends the
/// edited command launch whole. The two travel under different request
/// fields (`with` and `with_command`), so the helm never has to guess which
/// kind an edit is.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum RestartEdit {
    Agent(LaunchSelection),
    Command(CommandLaunch),
}

impl RestartEdit {
    /// The agent selection an agent edit carries, for the YOLO question's
    /// wording; `None` for a command edit, whose YOLO is its assertion.
    pub(crate) fn selection(&self) -> Option<&LaunchSelection> {
        match self {
            RestartEdit::Agent(selection) => Some(selection),
            RestartEdit::Command(_) => None,
        }
    }
}

/// What a session's Restart with edits, or `None` when there is nothing it
/// could edit: an agent launch's choices, or a command launch that declared
/// an agent type and a resume command (the only command launches that can
/// resume at all). The session view's availability rule decides whether the
/// dialog opens; this is only how it reads the opening snapshot.
fn restart_baseline(session: &Session) -> Option<RestartEdit> {
    match &session.launch {
        Some(crate::SessionLaunch::Agent { selection, .. }) => {
            let mut selection = selection.clone();
            // Old sessions can omit a permission that now means YOLO. Compare
            // the effective choices, so pressing the already-selected mode is
            // not a change.
            selection.permissions = selection
                .harness
                .effective_permission(selection.permissions);
            Some(RestartEdit::Agent(selection))
        }
        Some(crate::SessionLaunch::Command(command))
            if command.agent.is_some() && command.resume.is_some() =>
        {
            Some(RestartEdit::Command(command.clone()))
        }
        Some(crate::SessionLaunch::Command(_))
        | Some(crate::SessionLaunch::Legacy { .. })
        | None => None,
    }
}

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

/// Keep focus inside the modal before a primary action makes its controls busy.
///
/// The submit button survives busy renders, but a same-event choice Enter can
/// admit a changed draft before the unchanged render's native `disabled` has
/// cleared. Focusing that button would do nothing. The dialog container is the
/// existing focus-trap fallback and also survives; use it until the button is
/// focusable. A pointer submit normally already focuses the button, except on
/// WebKit, where the previous field can still hold focus and need this handoff.
fn focus_restart_with_submit() {
    document::eval(
        r#"(() => {
            const dialog = document.querySelector('.restart-with-dialog');
            const submit = dialog?.querySelector('.restart-with-submit');
            (submit && !submit.disabled ? submit : dialog)?.focus({ preventScroll: true });
        })()"#,
    );
}

/// Hand focus to the dialog's own cancel button before the YOLO question
/// goes away.
///
/// Dismissing the question unmounts the control that holds focus, and the
/// same module rule as [`focus_restart_with_submit`] applies: focus must
/// move to a control that survives, or it drops to the page body while the
/// modal is still open. Cancel is also where the dialog opened.
fn focus_restart_with_cancel() {
    document::eval(
        "document.querySelector('.restart-with-dialog .restart-with-cancel')?.focus({ preventScroll: true })",
    );
}

/// Edit only the launch fields the resumed conversation can change.
///
/// `session` is the opening snapshot and stays the comparison baseline while
/// this dialog is mounted. The parent rechecks live availability before it
/// sends a request, and passes a refusal back through `error`.
///
/// `yolo_confirmation` is what the confirmation for the helm's refusal of a YOLO restart on
/// a host that asks before YOLO launches explains, and `yolo_error` why a "don't ask again"
/// failed at its first step. It is shown inside the dialog because the dialog is modal:
/// anything rendered beside it is inert. Confirming resubmits what the dialog shows NOW,
/// with the override (`on_submit`'s second element); the third element asks the parent to
/// mark the host safe first. The parent accepts the override only by taking the live
/// question, and only for the settings that question was about: settings edited since are
/// refused with a reason, and restarting again asks about them.
#[component]
pub(crate) fn RestartWithDialog(
    session: Session,
    busy: bool,
    error: Option<String>,
    yolo_confirmation: Option<crate::yolo_confirm::YoloAsk>,
    yolo_error: Option<String>,
    stop_first: bool,
    offer_label: String,
    on_submit: EventHandler<(RestartEdit, bool, bool)>,
    on_yolo_cancel: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    // Fixed for the dialog's life: `session` is the opening snapshot, so
    // the same branch (and the same hooks below) runs on every render.
    let Some(opening) = restart_baseline(&session) else {
        return rsx! {};
    };
    let opening_command = match &opening {
        RestartEdit::Command(command) => Some(command.clone()),
        RestartEdit::Agent(_) => None,
    };
    // The agent controls' state. A command launch's dialog never shows
    // them; it seeds them from its declared agent type (which
    // `restart_baseline` guarantees) only because hooks run unconditionally.
    let baseline = match &opening {
        RestartEdit::Agent(selection) => selection.clone(),
        RestartEdit::Command(command) => LaunchSelection {
            harness: command
                .agent
                .expect("restart_baseline admits only a command launch with a declared agent type"),
            model: None,
            effort: None,
            permissions: None,
            workspace_trust: None,
        },
    };
    let base = use_context::<ApiBase>().0;
    let catalog_base = base.clone();
    // Only the agent controls use the model catalog; a command launch's
    // dialog runs the hook but never fetches it.
    let wants_catalog = opening_command.is_none();
    let catalog_resource = use_resource(move || {
        let base = catalog_base.clone();
        async move {
            if wants_catalog {
                api::fetch_launch_catalog(&base).await
            } else {
                Ok(Default::default())
            }
        }
    });
    let catalog_result = catalog_resource.read();
    // `None` until a catalog is in hand: still loading, or the read failed.
    let catalog_answer = catalog_result
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .cloned();
    let catalog = catalog_answer.clone().unwrap_or_default();
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
    // The command launch's draft: its two command lines and its YOLO
    // answer. Unused for an agent launch. The stored commands are
    // peer-relayed text (an agent can write them), so each field shows them
    // escaped and remembers whether it was edited; an untouched field sends
    // the stored bytes exactly, as the launcher's own fields do
    // (`list::create_form`'s `reseed_cloned_field` and `submitted_field`).
    // Without that, a hidden or direction-changing character could make the
    // field read differently from what Restart with sends.
    let stored_command = opening_command
        .as_ref()
        .map(|c| c.command.clone())
        .unwrap_or_default();
    let stored_resume = opening_command
        .as_ref()
        .and_then(|c| c.resume.clone())
        .unwrap_or_default();
    let shown_command = display_peer(&stored_command);
    let mut command_text = use_signal(move || shown_command);
    let mut command_edited = use_signal(|| false);
    let shown_resume = display_peer(&stored_resume);
    let mut resume_text = use_signal(move || shown_resume);
    let mut resume_edited = use_signal(|| false);
    let yolo_seed = opening_command.as_ref().is_some_and(|c| c.yolo);
    let mut command_yolo = use_signal(move || yolo_seed);

    // Both paint and submission read the same draft contract. Signals are read
    // inside the callback: a choice followed by Enter submits in the same event,
    // before Dioxus can render that choice into props or button availability.
    // Command edits use create validation before anything stops. Untouched
    // escaped fields still send the stored bytes, rather than their display text.
    let read_edit = Callback::new({
        let opening_command = opening_command.clone();
        let stored_command = stored_command.clone();
        let stored_resume = stored_resume.clone();
        move |()| {
            let current = selection();
            let catalog = catalog_resource.read();
            let catalog = catalog.as_ref().and_then(|answer| answer.as_ref().ok());
            let compatible = launch_composer::selection_fits_catalog(
                &current,
                catalog.map(|models| models.as_slice()),
            );
            let (edit, problem) = match &opening_command {
                None => (RestartEdit::Agent(current), None),
                Some(stored) => {
                    let draft = CommandLaunch {
                        command: if command_edited() {
                            command_text()
                        } else {
                            stored_command.clone()
                        },
                        yolo: command_yolo(),
                        agent: stored.agent,
                        resume: Some(if resume_edited() {
                            resume_text()
                        } else {
                            stored_resume.clone()
                        }),
                    };
                    let problem = draft.validate().err();
                    (RestartEdit::Command(draft), problem)
                }
            };
            let ready = match &edit {
                RestartEdit::Agent(_) => {
                    !(model_open() && !model_draft().is_empty())
                        && model_error().is_none()
                        && compatible
                }
                RestartEdit::Command(_) => problem.is_none(),
            };
            (edit, problem, ready)
        }
    });
    let current = selection();
    // A failed catalog read cannot prove that a stored choice became invalid.
    let compatible = launch_composer::selection_fits_catalog(&current, catalog_answer.as_deref());
    let (edit, edit_problem, ready) = read_edit.call(());
    // Which fields the edit changes, judged on what it would send (an
    // untouched field sends the stored bytes), for each field's marker.
    let (command_changed, resume_changed, yolo_changed) = match (&edit, &opening_command) {
        (RestartEdit::Command(draft), Some(stored)) => (
            draft.command != stored.command,
            draft.resume != stored.resume,
            draft.yolo != stored.yolo,
        ),
        _ => (false, false, false),
    };
    let may_submit = !busy && edit != opening && ready;
    // The parent still owns live operation admission and the rendered stop
    // consent. Reading the draft live must not widen that consent to a working
    // agent the displayed action did not say it would stop.
    let primary = Callback::new({
        let opening = opening.clone();
        move |(confirm_yolo, stop_asking)| {
            let (edit, _, ready) = read_edit.call(());
            if busy || !ready || edit == opening {
                return;
            }
            // Move focus before the busy render disables the choice that held
            // it. This ordering applies to keyboard submits and YOLO answers too.
            focus_restart_with_submit();
            on_submit.call((edit, confirm_yolo, stop_asking));
        }
    });
    let choice_primary = EventHandler::new(move |()| primary.call((false, false)));
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
                    if evt.key() == Key::Enter && (evt.is_auto_repeating() || evt.is_composing()) {
                        // Native action-button activation needs the same held-key
                        // boundary as the explicit choice and text-field paths.
                        evt.prevent_default();
                    }
                    if evt.key() == Key::Escape && !evt.is_composing() && !busy {
                        on_cancel.call(());
                    }
                },
                h2 { class: "restart-with-title", "restart with · {title}" }
                dl { class: "restart-with-context",
                    if opening_command.is_some() {
                        dt { "launch" } dd { "command" }
                        dt { "agent type" } dd { "{harness}" }
                    } else {
                        dt { "harness" } dd { "{harness}" }
                    }
                    dt { "host" } dd { "{display_peer(host)}" }
                    dt { "folder" } dd { "{folder}" }
                    dt { "resumes" } dd { "this session's conversation" }
                }
                if opening_command.is_some() {
                    p { class: "restart-with-fixed-note", "launch kind, agent type, host and folder stay fixed; restart runs the resume command, so a changed command applies the next time this session is cloned or replaced" }
                } else {
                    p { class: "restart-with-fixed-note", "harness, host and folder stay fixed; use replace with for another harness or folder, or clone for another host" }
                }
                hr { class: "restart-with-rule" }
                if opening_command.is_some() {
                    div { class: "restart-with-command",
                        label {
                            span {
                                "command"
                                if command_changed { span { class: "restart-with-edited", " (edited)" } }
                            }
                            input {
                                r#type: "text",
                                class: "restart-with-command-input",
                                autocomplete: "off",
                                autocorrect: "off",
                                autocapitalize: "none",
                                spellcheck: "false",
                                dir: "ltr",
                                value: "{command_text}",
                                disabled: busy,
                                onkeydown: move |evt| enter_choice(evt, busy, || {}, Some(choice_primary)),
                                oninput: move |evt| {
                                    command_text.set(evt.value());
                                    command_edited.set(true);
                                },
                            }
                        }
                        label {
                            span {
                                "resume command"
                                if resume_changed { span { class: "restart-with-edited", " (edited)" } }
                            }
                            input {
                                r#type: "text",
                                class: "restart-with-resume-input",
                                autocomplete: "off",
                                autocorrect: "off",
                                autocapitalize: "none",
                                spellcheck: "false",
                                dir: "ltr",
                                value: "{resume_text}",
                                disabled: busy,
                                onkeydown: move |evt| enter_choice(evt, busy, || {}, Some(choice_primary)),
                                oninput: move |evt| {
                                    resume_text.set(evt.value());
                                    resume_edited.set(true);
                                },
                            }
                        }
                        fieldset { class: "launch-command-yolo",
                            legend {
                                "runs without approval prompts"
                                if yolo_changed { span { class: "restart-with-edited", " (edited)" } }
                            }
                            label {
                                "data-tooltip": "yes: you assert this command acts without asking for approval; Farhelm marks it YOLO and does not check",
                                input {
                                    r#type: "radio",
                                    name: "restart-with-command-yolo",
                                    checked: command_yolo(),
                                    disabled: busy,
                                    onkeydown: move |evt| enter_choice(evt, busy, || {}, Some(choice_primary)),
                                    onchange: move |_| command_yolo.set(true),
                                }
                                "yes (YOLO)"
                            }
                            label {
                                "data-tooltip": "no: you assert this command asks before acting; Farhelm does not check",
                                input {
                                    r#type: "radio",
                                    name: "restart-with-command-yolo",
                                    checked: !command_yolo(),
                                    disabled: busy,
                                    onkeydown: move |evt| enter_choice(evt, busy, || {}, Some(choice_primary)),
                                    onchange: move |_| command_yolo.set(false),
                                }
                                "no"
                            }
                        }
                        if let Some(problem) = edit_problem.clone() {
                            p { class: "restart-with-command-error", role: "status", "{problem}" }
                        }
                    }
                } else {
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
                    on_choice_enter: Some(choice_primary),
                    on_model_primary_enter: Some(choice_primary),
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
                                // An obsolete highlighted row must not discard an
                                // unselected draft and restart the previous model.
                                if !model_draft().trim().is_empty() {
                                    model_error.set(Some("choose a model before restarting".to_string()));
                                } else {
                                    model_draft.set(String::new());
                                    model_open.set(false);
                                }
                            }
                            ModelEnterTarget::Option(option) => apply_option.call(option),
                            ModelEnterTarget::Custom { id, harness } if harness == baseline.harness => {
                                apply_option.call(ModelOption::Model { id, harness });
                            }
                            ModelEnterTarget::Custom { .. }
                            | ModelEnterTarget::NeedsHarness(_)
                            | ModelEnterTarget::OwnedElsewhere { .. } => {
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
                }
                if let Some(message) = catalog_error.filter(|_| opening_command.is_none()) {
                    p { class: "restart-with-catalog-error", role: "status", "model catalog unavailable: {message}" }
                }
                if let Some(message) = error {
                    p { class: "restart-with-error", role: "alert", "{message}" }
                }
                if let Some(ask) = yolo_confirmation {
                    crate::yolo_confirm::YoloConfirmation {
                        ask,
                        busy,
                        error: yolo_error,
                        confirm_submits: false,
                        // All three hand focus to a control that outlives
                        // the question, or that stays enabled while the
                        // request runs, before the parent unmounts or
                        // disables the clicked one; see
                        // `focus_restart_with_cancel` and
                        // `focus_restart_with_submit`.
                        on_confirm: move |_| {
                            primary.call((true, false));
                        },
                        // The answer takes the question down while the host
                        // is marked safe, which would drop the focus of this
                        // button with it; the submit button is the control
                        // that stays enabled.
                        on_confirm_and_stop_asking: move |_| {
                            primary.call((true, true));
                        },
                        on_cancel: move |_| {
                            focus_restart_with_cancel();
                            on_yolo_cancel.call(());
                        },
                    }
                }
                div { class: "restart-with-footer",
                    if stop_first {
                        p { class: "restart-with-stop-note",
                            "agent is working; it is stopped first"
                        }
                    }
                    button {
                        class: "btn btn-neutral restart-with-cancel",
                        "data-tooltip": "cancel: close without restarting",
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
                        "data-tooltip": "{offer_label}",
                        disabled: !may_submit && !busy,
                        aria_disabled: if busy { "true" },
                        onclick: move |_| {
                            primary.call((false, false));
                        },
                        if stop_first { "stop and restart" } else { "restart" }
                    }
                }
            }
        }
    }
}
