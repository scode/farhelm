//! Shared launch choices for the session launcher's creation and restart modes.
//!
//! Values enter as props and user changes leave through callbacks. The caller owns
//! the selected harness, model draft, and every consequential state transition;
//! these controls do not choose a harness, promote history, or manage create
//! idempotency. Keeping that boundary lets a fixed-harness restart dialog use
//! the same model, effort, and permission controls as the launcher.

use dioxus::prelude::*;

use crate::api::LaunchCatalogModel;
use crate::launch_composer::{self, ModelEnterTarget, ModelOption, harness_label};
use crate::peer::{DetailPart, PeerLine, display_peer};
use crate::{LaunchEffort, LaunchHarness, LaunchPermission, LaunchSelection};

/// Present the same destination vocabulary in the launcher and templates.
///
/// This control only reports an explicit choice. The caller owns restoring
/// folder drafts, invalidating previews and deciding whether a repository is
/// launchable. Enter selects the type without invoking either dialog's primary.
#[component]
pub(crate) fn DestinationTypeControl(
    managed: bool,
    disabled: bool,
    on_change: EventHandler<bool>,
) -> Element {
    rsx! {
        div { class: "segmented-control destination-type-control", role: "group", aria_label: "Destination type",
            for (choice, label) in [(false, "Folder"), (true, "Managed checkout")] {
                button { r#type: "button", disabled, class: if choice == managed { "selected" } else { "" },
                    aria_pressed: choice == managed, onclick: move |_| on_change.call(choice),
                    "data-tooltip": if choice { "Managed checkout: Farhelm clones a repository into a new folder for this session" } else { "Folder: start in an existing folder on this host" },
                    if choice { crate::icons::BranchIcon {} } else { crate::icons::FolderIcon {} }
                    "{label}"
                }
            }
        }
    }
}

/// Apply a focused choice before invoking its dialog's ordinary primary action.
///
/// Only controls with a caller-supplied primary opt in. Action buttons and
/// nested editors keep native or dedicated Enter handling. Consuming repeat
/// and composition events prevents the browser from activating the button as
/// a fallback; `apply` retains the caller's live transition guards, while the
/// primary action remains responsible for admitting a launch or restart.
pub(crate) fn enter_choice(
    event: KeyboardEvent,
    busy: bool,
    apply: impl FnOnce(),
    primary: Option<EventHandler<()>>,
) {
    let Some(primary) = primary else { return };
    if event.key() != Key::Enter {
        return;
    }
    event.prevent_default();
    event.stop_propagation();
    if busy || event.is_auto_repeating() || event.is_composing() {
        return;
    }
    apply();
    primary.call(());
}

/// Render a baseline value only when this field now differs from it.
///
/// The marker compares wire values, so restoring a choice clears it even if
/// the person visited other options while editing.
/// App-authored baseline words complete the `was …` clause in lowercase;
/// acronym spelling such as YOLO still applies inside that clause.
///
/// `old_label` arrives as a [`DetailPart`] because the fields differ in who
/// wrote it. Effort, permissions, and the `default` placeholder are
/// this UI's own vocabulary, but a stored model id is a relayed string that
/// may carry invisible or directional characters. That run must be escaped
/// and direction-isolated (see `peer.rs`); interpolated raw, it could look
/// identical to a different id or reorder the marker around it. The raw
/// values stay what is compared here and what a caller submits; only the
/// rendered run is escaped.
fn changed_marker<T: PartialEq>(
    current: &T,
    baseline: &T,
    old_label: DetailPart,
) -> Option<Vec<DetailPart>> {
    (current != baseline).then(|| vec![DetailPart::text("● Changed · was "), old_label])
}

/// The old-value run for a model marker: the stored id is a peer value, while
/// its absence is this UI's own `default`.
fn model_marker_label(old: Option<&str>) -> DetailPart {
    old.map_or_else(|| DetailPart::text("default"), DetailPart::peer)
}

/// Render one caller-owned structured selection without changing its state.
///
/// `model_id_prefix` scopes the transient listbox and option IDs when two
/// surfaces are mounted together. The launcher passes its historical prefix
/// so its DOM and existing browser selectors remain unchanged.
/// `on_choice_enter` opts choice controls into apply-then-primary behavior;
/// longer effort lists use a native select with the same callback and values.
/// Its input event updates the caller; Enter invokes primary against that
/// live draft without replaying a render's old value. Space and arrow
/// handling stay with the native select. Omitting primary leaves choice
/// editing without launch admission.
/// The model field has a separate policy: `on_model_primary_enter`, when set,
/// runs after an open pick is applied or directly from the closed field. Its
/// caller must validate the live draft, including a pick the model handler refused.
/// Omitting it keeps New session's selection-only model Enter behavior.
/// `on_conversation` opts a launcher into the one-time selector. Template
/// editors omit it because that value must never become a reusable choice.
/// `on_prompt` separately opts New/Clone/Replace into a first-message editor;
/// Restart omits it because that message must never be sent a second time.
#[component]
pub(crate) fn LaunchControls(
    #[props(default)] prompt: String,
    #[props(default)] on_prompt: Option<EventHandler<String>>,
    #[props(default)] conversation: String,
    #[props(default)] conversation_required: bool,
    #[props(default)] on_conversation: Option<EventHandler<String>>,
    harness: Option<LaunchHarness>,
    model: Option<String>,
    model_raw_seed: Option<String>,
    model_edited: bool,
    effort: Option<LaunchEffort>,
    permissions: Option<LaunchPermission>,
    permission_flags: String,
    wrapper_prefix: String,
    #[props(default)] container: crate::container_controls::ContainerDraft,
    #[props(default = true)] container_available: bool,
    #[props(default)] on_container: Option<EventHandler<crate::container_controls::ContainerEdit>>,
    catalog: Vec<LaunchCatalogModel>,
    busy: bool,
    model_draft: String,
    model_open: bool,
    model_active: Option<usize>,
    model_show_all: bool,
    model_draft_error: Option<String>,
    choice_error: Option<String>,
    reset_reason: Option<String>,
    model_id_prefix: String,
    #[props(default)] baseline: Option<LaunchSelection>,
    #[props(default)] fixed_harness: bool,
    #[props(default)] on_choice_enter: Option<EventHandler<()>>,
    #[props(default)] on_model_primary_enter: Option<EventHandler<()>>,
    on_model_focus: EventHandler<()>,
    on_model_input: EventHandler<String>,
    on_model_blur: EventHandler<()>,
    on_model_escape: EventHandler<()>,
    on_model_active: EventHandler<Option<usize>>,
    on_model_enter: EventHandler<ModelEnterTarget>,
    on_model_option: EventHandler<ModelOption>,
    on_effort: EventHandler<Option<LaunchEffort>>,
    on_permissions: EventHandler<Option<LaunchPermission>>,
    on_permission_flags: EventHandler<String>,
    on_wrapper_prefix: EventHandler<String>,
) -> Element {
    let omitted_permission = harness.and_then(LaunchHarness::omitted_permission);
    let efforts = harness
        .map(|harness| {
            launch_composer::displayed_efforts(
                harness,
                model.as_deref(),
                &catalog,
                effort,
                baseline.as_ref().and_then(|old| old.effort),
            )
        })
        .unwrap_or_default();
    // The default is a choice too. Longer vocabularies use a native select,
    // but retain the same displayed list, including inherited old values.
    let effort_select_options = efforts.clone();
    let mut options =
        launch_composer::model_options(&catalog, harness, &model_draft, model_show_all);
    if fixed_harness {
        options.retain(|option| !matches!(option, ModelOption::ShowAll));
    }
    let option_count = options.len();
    // A closed combobox shows the committed selection. An untouched seed keeps
    // its escaped display spelling; an edited choice uses the chosen model ID.
    let model_display = if model_open {
        model_draft.clone()
    } else {
        match model.as_ref() {
            Some(model) if model_edited => model.clone(),
            Some(model) => model_raw_seed
                .as_deref()
                .map(display_peer)
                .unwrap_or_else(|| model.clone()),
            None => "Harness default".to_string(),
        }
    };
    let model_hint = harness
        .map(|harness| {
            format!(
                "{} for {}",
                catalog
                    .iter()
                    .filter(|model| model.harness == harness)
                    .count(),
                harness_label(harness)
            )
        })
        .unwrap_or_else(|| "Choose a harness".to_string());
    let model_results_id = format!("{model_id_prefix}-results");
    // Templates may supply formatting controls. Preserve the raw message for
    // submission, but never let an invisible value masquerade as ordinary text.
    let prompt_read_only = prompt
        .chars()
        .any(|ch| !matches!(ch, '\n' | '\t') && farhelm_proto::text::is_presentation_unsafe(ch));
    // Shared controls can coexist in different dialogs. Scope accessible help
    // with their existing model prefix so each button describes its own mode.
    let permission_help_id = format!("{model_id_prefix}-permission-help");
    let guarded_help_id = format!("{model_id_prefix}-guarded-help");
    let own_help_id = format!("{model_id_prefix}-own-permission-help");
    let model_change = baseline.as_ref().and_then(|old| {
        changed_marker(&model, &old.model, model_marker_label(old.model.as_deref()))
    });
    let effort_change = baseline.as_ref().and_then(|old| {
        changed_marker(
            &effort,
            &old.effort,
            DetailPart::text(
                old.effort
                    .map(launch_composer::effort_value)
                    .unwrap_or("default"),
            ),
        )
    });
    let permission_change = baseline.as_ref().and_then(|old| {
        changed_marker(
            &permissions,
            &old.permissions,
            DetailPart::text(
                old.permissions
                    .map(|permission| match permission {
                        LaunchPermission::Yolo => "YOLO",
                        value => launch_composer::permission_value(value),
                    })
                    .unwrap_or("agent's own setting"),
            ),
        )
    });
    let wrapper_error = (!wrapper_prefix.is_empty())
        .then(|| farhelm_proto::split_wrapper_prefix(&wrapper_prefix).err())
        .flatten()
        .map(crate::local_validation_notice);
    // Prefixes can come from another host or an agent-written template. Show
    // unsafe characters as text, while keeping the original words untouched
    // until the user explicitly clears this inherited choice.
    let wrapper_unsafe = wrapper_prefix
        .chars()
        .any(farhelm_proto::text::is_presentation_unsafe);
    let shown_wrapper = if wrapper_unsafe {
        crate::peer::editable_peer(&wrapper_prefix)
    } else {
        wrapper_prefix.clone()
    };

    rsx! {
        if harness.is_none_or(LaunchHarness::offers_model) {
            div { class: "launch-composer-choice launch-composer-model-choice",
                span { class: "launch-composer-section-label", "Model" }
                if let Some(parts) = model_change { PeerLine { class: "launch-composer-changed-marker", parts } }
                div { class: "launch-composer-model",
                    input {
                        r#type: "text",
                        class: "font-mono",
                        role: "combobox",
                        aria_label: "Model",
                        aria_expanded: model_open,
                        aria_controls: "{model_results_id}",
                        aria_activedescendant: model_open
                            .then(|| model_active.map(|index| format!("{model_id_prefix}-option-{index}")))
                            .flatten(),
                        aria_invalid: model_draft_error.is_some(),
                        autocomplete: "off",
                        autocorrect: "off",
                        autocapitalize: "none",
                        spellcheck: false,
                        dir: "ltr",
                        disabled: busy,
                        value: "{model_display}",
                        onfocus: move |_| on_model_focus.call(()),
                        oninput: move |evt| on_model_input.call(evt.value()),
                        onblur: move |_| on_model_blur.call(()),
                        onkeydown: {
                            let options = options.clone();
                            let catalog = catalog.clone();
                            let model_draft = model_draft.clone();
                            let model_id_prefix = model_id_prefix.clone();
                            move |evt: KeyboardEvent| {
                                match evt.key() {
                                    Key::Escape if model_open => {
                                        // The transient list owns Escape before the dialog can cancel.
                                        evt.prevent_default();
                                        evt.stop_propagation();
                                        on_model_escape.call(());
                                    }
                                    Key::ArrowDown if model_open && option_count > 0 => {
                                        evt.prevent_default();
                                        let index = model_active.map_or(0, |index| (index + 1) % option_count);
                                        on_model_active.call(Some(index));
                                        scroll_model_result(&model_id_prefix, index);
                                    }
                                    Key::ArrowUp if model_open && option_count > 0 => {
                                        evt.prevent_default();
                                        let index = model_active.map_or(option_count - 1, |index| {
                                            (index + option_count - 1) % option_count
                                        });
                                        on_model_active.call(Some(index));
                                        scroll_model_result(&model_id_prefix, index);
                                    }
                                    // The model owns Enter even when closed. Restart with opts
                                    // into choose-then-primary; New session only chooses. The
                                    // caller's live validation decides whether a refused pick
                                    // can proceed, rather than a render-time choice snapshot.
                                    Key::Enter => {
                                        evt.prevent_default();
                                        evt.stop_propagation();
                                        if busy || evt.is_auto_repeating() || evt.is_composing() { return; }
                                        if model_open {
                                            on_model_enter.call(launch_composer::model_enter_target(
                                                &options, model_active, &model_draft, &catalog, harness,
                                            ));
                                        }
                                        if let Some(primary) = on_model_primary_enter { primary.call(()); }
                                    }
                                    _ => {}
                                }
                            }
                        },
                    }
                    span { class: "launch-composer-model-hint", "{model_hint}" }
                    if model_open {
                        div { id: "{model_results_id}", role: "listbox", class: "launch-composer-listbox",
                            for (index, option) in options.iter().cloned().enumerate() {
                                button {
                                    id: "{model_id_prefix}-option-{index}",
                                    r#type: "button",
                                    role: "option",
                                    dir: "ltr",
                                    aria_selected: model_active == Some(index),
                                    "data-tooltip": "Use this model for the session",
                                    class: if model_active == Some(index) { "selected" } else { "" },
                                    disabled: busy,
                                    // Rows are not sequential tab stops. The keyboard reaches
                                    // them through the input's `aria-activedescendant` (Arrow
                                    // and Enter above), which is the ARIA combobox pattern. A
                                    // tabbable row would be the target of Tab from the input,
                                    // but the input's blur closes the list and unmounts that
                                    // row, so focus would fall to the document: out of the
                                    // launcher modal and its Tab loop.
                                    tabindex: "-1",
                                    // Keep focus on the input until a transient row's click
                                    // applies the choice; blur would otherwise unmount the row.
                                    onmousedown: move |evt| evt.prevent_default(),
                                    onclick: {
                                        let option = option.clone();
                                        move |_| on_model_option.call(option.clone())
                                    },
                                    match &option {
                                        ModelOption::HarnessDefault => rsx! { "Harness default" },
                                        ModelOption::Model { id, harness: owner } => rsx! {
                                            "{display_peer(id)}"
                                            if model_show_all || harness.is_none() { " ({harness_label(*owner)})" }
                                        },
                                        ModelOption::ShowAll => rsx! {
                                            if model_show_all {
                                                "Show chosen harness's models"
                                            } else {
                                                "Show every harness's models"
                                            }
                                        },
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if let Some(ref reason) = model_draft_error {
                div { class: "launch-composer-choice-error", "{reason}" }
            }
        }
        // A harness without an effort vocabulary leaves permissions alone in
        // the pair so a blank sibling does not imply a missing setting.
        div { class: "launch-composer-choice-pair",
            if harness.is_none_or(LaunchHarness::offers_effort) {
                div { class: "launch-composer-choice launch-composer-effort-choice",
                    span { class: "launch-composer-section-label", "Effort" }
                    if let Some(parts) = effort_change { PeerLine { class: "launch-composer-changed-marker", parts } }
                    if efforts.len() + 1 > 5 {
                        select {
                            class: "launch-composer-effort-select",
                            aria_label: "Effort",
                            value: effort.map(launch_composer::effort_value).unwrap_or("default"),
                            disabled: busy,
                            // The native input event must update the caller
                            // before Enter admits a primary action. Reusing its
                            // controlled value avoids a second selected-state
                            // owner or a harness-specific effort parser here.
                            oninput: move |event| {
                                if busy { return; }
                                let value = event.value();
                                if value == "default" {
                                    on_effort.call(None);
                                } else if let Some(offered) = effort_select_options.iter().copied()
                                    .find(|offered| launch_composer::effort_value(*offered) == value) {
                                    on_effort.call(Some(offered));
                                }
                            },
                            // The caller already owns the last input. Replaying
                            // this render's `effort` could undo an input whose
                            // callback ran before the next render completed.
                            onkeydown: move |event| enter_choice(event, busy, || {}, on_choice_enter),
                            option { value: "default", selected: effort.is_none(), "Default" }
                            for offered in efforts {
                                option {
                                    value: launch_composer::effort_value(offered),
                                    selected: effort == Some(offered),
                                    "{launch_composer::effort_label(Some(offered))}"
                                }
                            }
                        }
                    } else {
                        div { class: "segmented-control", role: "group", aria_label: "Effort",
                            button {
                                r#type: "button",
                                class: if effort.is_none() { "selected" } else { "" },
                                "data-tooltip": "Use the agent's default reasoning effort",
                                aria_pressed: effort.is_none(),
                                disabled: busy,
                                onkeydown: move |event| enter_choice(event, busy, || on_effort.call(None), on_choice_enter),
                                onclick: move |_| on_effort.call(None),
                                "Default"
                            }
                            for offered in efforts {
                                button {
                                    key: "{launch_composer::effort_value(offered)}",
                                    r#type: "button",
                                    class: if effort == Some(offered) { "selected" } else { "" },
                                    "data-tooltip": "Use {launch_composer::effort_value(offered)} reasoning effort",
                                    aria_pressed: effort == Some(offered),
                                    disabled: busy,
                                    onkeydown: move |event| enter_choice(event, busy, || on_effort.call(Some(offered)), on_choice_enter),
                                    onclick: move |_| on_effort.call(Some(offered)),
                                    "{launch_composer::effort_label(Some(offered))}"
                                }
                            }
                        }
                    }
                }
            }
            div { class: "launch-composer-choice launch-composer-permissions-choice",
                span { class: "launch-composer-section-label", "Permissions" }
                if let Some(parts) = permission_change { PeerLine { class: "launch-composer-changed-marker", parts } }
                div { class: "launch-composer-permission-controls",
                    // Search results have a separate Permissions group. Name
                    // these choices by their role while keeping the visible label.
                    div { class: "segmented-control", role: "group", aria_label: "Permission mode",
                        if harness.is_some_and(|harness| harness.omitted_permission().is_none()) {
                            span { class: "visually-hidden", id: "{own_help_id}",
                                {launch_composer::permission_help(harness.expect("own-setting has a harness"), None)}
                            }
                            button {
                                r#type: "button",
                                class: if permissions.is_none() { "selected" } else { "" },
                                "data-tooltip": "Agent's own setting: Farhelm cannot force or classify this permission mode",
                                aria_describedby: "{own_help_id}",
                                aria_pressed: permissions.is_none(),
                                disabled: busy,
                                onkeydown: move |event| enter_choice(event, busy, || on_permissions.call(None), on_choice_enter),
                                onclick: move |_| on_permissions.call(None),
                                "Agent's own setting"
                            }
                        }
                        if harness.is_none_or(|harness| harness.offers_permission(LaunchPermission::Guarded)) {
                            span { class: "visually-hidden", id: "{guarded_help_id}",
                                {harness.map(|harness| launch_composer::permission_help(harness, Some(LaunchPermission::Guarded))).unwrap_or_else(|| "Guarded: choose an agent to see its automatic safety mode".to_string())}
                            }
                            button {
                                r#type: "button",
                                class: if permissions == Some(LaunchPermission::Guarded)
                                    || (permissions.is_none() && (harness.is_none() || omitted_permission == Some(LaunchPermission::Guarded))) { "selected" } else { "" },
                                "data-tooltip": harness.map(|harness| launch_composer::permission_help(harness, Some(LaunchPermission::Guarded))).unwrap_or_else(|| "Guarded: choose an agent to see its automatic safety mode".to_string()),
                                "data-tooltip-paragraphs": "true",
                                aria_describedby: "{guarded_help_id}",
                                aria_pressed: permissions == Some(LaunchPermission::Guarded)
                                    || (permissions.is_none() && (harness.is_none() || omitted_permission == Some(LaunchPermission::Guarded))),
                                disabled: busy,
                                onkeydown: move |event| enter_choice(event, busy, || on_permissions.call(Some(LaunchPermission::Guarded)), on_choice_enter),
                                onclick: move |_| on_permissions.call(Some(LaunchPermission::Guarded)),
                                "Guarded"
                            }
                        }
                        if harness.is_none_or(|harness| harness.offers_permission(LaunchPermission::Yolo)) {
                            button {
                                r#type: "button",
                                class: if permissions == Some(LaunchPermission::Yolo)
                                    || (permissions.is_none() && omitted_permission == Some(LaunchPermission::Yolo)) {
                                    "selected launch-composer-segment-danger"
                                } else {
                                    "launch-composer-segment-danger"
                                },
                                aria_pressed: permissions == Some(LaunchPermission::Yolo)
                                    || (permissions.is_none() && omitted_permission == Some(LaunchPermission::Yolo)),
                                "data-tooltip": launch_composer::permission_tooltip(LaunchPermission::Yolo),
                                disabled: busy,
                                onkeydown: move |event| enter_choice(event, busy, || on_permissions.call(Some(LaunchPermission::Yolo)), on_choice_enter),
                                onclick: move |_| on_permissions.call(Some(LaunchPermission::Yolo)),
                                "YOLO"
                            }
                        }
                        if harness.is_some_and(|harness| harness.offers_permission(LaunchPermission::Custom)) {
                            button {
                                r#type: "button", class: if permissions == Some(LaunchPermission::Custom) { "selected" } else { "" },
                                aria_pressed: permissions == Some(LaunchPermission::Custom), disabled: busy,
                                "data-tooltip": launch_composer::permission_tooltip(LaunchPermission::Custom),
                                onclick: move |_| {
                                    on_permissions.call(Some(LaunchPermission::Custom));
                                    document::eval("requestAnimationFrame(() => document.querySelector('.custom-permission-flags')?.focus())");
                                },
                                // This choice opens an editor. Native button Enter
                                // follows the same focus handoff as a click, rather
                                // than invoking primary on an incomplete choice.
                                "Custom"
                            }
                        }
                        // The approval modes after guarded and YOLO, in their
                        // fixed order, shown for whichever harness offers them
                        // (Goose all three, OMP Approve alone). Filtered in the
                        // iterator so each keyed button stays the loop's root.
                        for permission in [
                            LaunchPermission::Approve,
                            LaunchPermission::SmartApprove,
                            LaunchPermission::Chat,
                        ]
                        .into_iter()
                        .filter(|permission| {
                            harness.is_some_and(|harness| harness.offers_permission(*permission))
                        })
                        {
                            button {
                                key: "{launch_composer::permission_value(permission)}",
                                r#type: "button",
                                class: if permissions == Some(permission) { "selected" } else { "" },
                                "data-tooltip": launch_composer::permission_tooltip(permission),
                                aria_pressed: permissions == Some(permission),
                                disabled: busy,
                                onkeydown: move |event| enter_choice(event, busy, || on_permissions.call(Some(permission)), on_choice_enter),
                                onclick: move |_| on_permissions.call(Some(permission)),
                                "{launch_composer::permission_display_value(permission)}"
                            }
                        }
                    }
                    if let Some(harness) = harness {
                        span { class: "visually-hidden", id: "{permission_help_id}",
                            "{launch_composer::permission_help(harness, harness.request_permission(permissions))}"
                        }
                        button {
                            r#type: "button",
                            class: "btn launch-permission-help",
                            aria_label: "About permission modes",
                            aria_describedby: "{permission_help_id}",
                            "data-tooltip": launch_composer::permission_help(harness, harness.request_permission(permissions)),
                            "data-tooltip-paragraphs": "true",
                            onclick: move |_| crate::app_bar::open_external_page("https://farhelm.io/docs/agents/#permission-modes"),
                            "?"
                        }
                    }
                }
            }
        }
            if let (Some(harness), Some(on_change)) = (harness.filter(|h| h.offers_named_conversation()), on_conversation) {
                div { class: "launch-composer-choice named-conversation-choice",
                    label {
                        span { class: "launch-composer-section-label", "{launch_composer::conversation_label(harness)}" }
                        input {
                            class: "named-conversation", r#type: "text", value: conversation,
                            disabled: busy, autocomplete: "off", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                            placeholder: if conversation_required { "Required for this restart" } else { "Optional, this start only" },
                            oninput: move |event| { if !busy { on_change.call(event.value()); } },
                            onkeydown: move |event| enter_choice(event, busy, || {}, on_choice_enter),
                        }
                    }
                    if conversation_required {
                        p { "Farhelm has no captured conversation to resume. Enter its ID or session file to restart here." }
                    }
                    if !harness.captures_named_conversation() {
                        p { "Farhelm will not capture this conversation after a named start, so Restart will not be available." }
                    }
                }
            }
            if let (Some(harness), Some(on_change)) = (harness.filter(|h| h.supports_initial_prompt(false)), on_prompt) {
                div { class: "launch-composer-choice initial-prompt-choice",
                    label {
                        span { class: "launch-composer-section-label", "Initial prompt" }
                        // Extra intrinsic rows would defeat the compact
                        // minimum height of .initial-prompt in app.css.
                        textarea {
                            class: "initial-prompt", rows: 3,
                            value: if prompt_read_only { display_peer(&prompt) } else { prompt.clone() },
                            readonly: prompt_read_only,
                            disabled: busy, autocomplete: "off", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                            "data-tooltip": "Initial prompt: sent once when this session starts",
                            oninput: move |event| { if !busy && !prompt_read_only { on_change.call(event.value()); } },
                            // Enter belongs to the message, including held or IME
                            // Enter. The form's single-line guard must not eat it.
                            onkeydown: move |event| { if event.key() == Key::Enter { event.stop_propagation(); } },
                        }
                    }
                    p { "Sent once, as {harness_label(harness)}'s first message when it starts. Restart, Clone and Replace do not send it again." }
                    if prompt_read_only {
                        p { "Invisible formatting is shown escaped. Clear the message to replace it." }
                    }
                    if !prompt.is_empty() {
                        button { r#type: "button", class: "btn btn-neutral initial-prompt-clear", disabled: busy,
                            "data-tooltip": "Clear initial prompt: remove this first message from the draft",
                            onclick: move |_| { if !busy { on_change.call(String::new()); } }, "Clear initial prompt"
                        }
                    }
                }
            }
            if permissions == Some(LaunchPermission::Custom) {
                CustomPermissionFlags { text: permission_flags, busy, harness, on_change: on_permission_flags }
            }
            div { class: "launch-composer-choice launch-composer-wrapper-choice",
                label {
                    span { class: "launch-composer-section-label", "Wrapper prefix" }
                    input {
                        class: "launch-composer-wrapper", r#type: "text", value: shown_wrapper,
                        disabled: busy, readonly: wrapper_unsafe, aria_invalid: wrapper_error.is_some(),
                        autocomplete: "off", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                        placeholder: "with-endpoint --profile work --",
                        "data-tooltip": "Wrapper prefix: Bash text before Farhelm's agent command; quotes and expansions keep their shell meaning; no placeholders",
                        oninput: move |event| { if !busy { on_wrapper_prefix.call(event.value()); } },
                        onkeydown: move |event| enter_choice(event, busy, || {}, on_choice_enter),
                    }
                }
                if wrapper_unsafe {
                    button {
                        class: "btn btn-neutral launch-composer-wrapper-clear",
                        r#type: "button", disabled: busy,
                        "data-tooltip": "Clear wrapper prefix: remove this value before editing",
                        onclick: move |_| on_wrapper_prefix.call(String::new()),
                        "Clear wrapper prefix"
                    }
                }
                p { class: "launch-composer-choice-help", "Runs through Bash before the agent command; blank means none." }
                if let Some(reason) = wrapper_error.as_ref() {
                    div { class: "launch-composer-choice-error", role: "status", "{reason}" }
                }
            }
        if let Some(on_change) = on_container {
            crate::container_controls::ContainerControls {
                value: container, busy, available: container_available, on_change,
            }
        }
        if let Some(reason) = choice_error {
            div { class: "launch-composer-choice-error", "{reason}" }
        }
        if let Some(reason) = reset_reason {
            div { class: "launch-composer-choice-error", role: "status", "{reason}" }
        }
    }
}

/// Scroll the active option without moving focus out of the combobox input.
///
/// Keyboard navigation uses `aria-activedescendant`; moving DOM focus to a
/// transient option would close the list on the input's blur.
fn scroll_model_result(prefix: &str, index: usize) {
    document::eval(&format!(
        r#"document.getElementById('{prefix}-option-{index}')?.scrollIntoView({{ block: 'nearest' }});"#,
    ));
}

/// Edit literal permission words without advertising an incomplete command.
/// The input retains invalid text so a quote can be repaired; only successfully
/// split words appear in the preview or can become a submitted selection.
#[component]
pub(crate) fn CustomPermissionFlags(
    text: String,
    busy: bool,
    harness: Option<LaunchHarness>,
    on_change: EventHandler<String>,
) -> Element {
    let parsed =
        farhelm_proto::split_permission_flags(&text).map_err(crate::local_validation_notice);
    // Stored words may be relayed by a host or an agent request. Show hidden
    // characters visibly, while untouched props retain their exact bytes for
    // parsing and submission. An input event replaces them with the edited text.
    // Empty and whitespace-only editors stay literal rather than using the
    // read-only display labels, which could become arguments during an edit.
    let shown_text = crate::peer::editable_peer(&text);
    let agent = harness
        .map(launch_composer::harness_label)
        .unwrap_or("the agent");
    rsx! {
        div { class: "launch-composer-choice custom-permission-choice",
            label { "Permission flags"
                input { class: "custom-permission-flags", value: "{shown_text}", disabled: busy,
                    autocomplete: "off", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                    oninput: move |event| on_change.call(event.value()),
                }
            }
            p { "Passed to {agent} instead of Farhelm's permission flags. You are responsible for ensuring they only change permissions. Farhelm does not check them, so this session counts as YOLO." }
            match parsed {
                Ok(words) => rsx! { div { class: "custom-permission-words", aria_label: "Permission arguments",
                    for word in words { code { dir: "auto", "{crate::peer::display_peer(&word)}" } }
                } },
                Err(error) => rsx! { p { class: "launch-composer-error", role: "alert", "{error}" } },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{changed_marker, model_marker_label};
    use crate::peer::{DetailPart, detail_text};

    /// A restored field must stop claiming a change, and a changed default
    /// must name the value it replaces so the restart consent stays legible.
    #[test]
    fn marker_tracks_final_value_against_baseline() {
        assert_eq!(
            changed_marker(&None::<bool>, &None, DetailPart::text("default")),
            None
        );
        let marker = changed_marker(&Some(true), &None, DetailPart::text("default"))
            .expect("a changed field is marked");
        assert_eq!(detail_text(&marker), "● Changed · was default");
    }

    /// The old model id in a changed marker is someone else's string, so it
    /// must render through the peer primitives rather than as marker text.
    ///
    /// A stored id is validated only for control characters and whitespace, so
    /// a zero-width space or a bidi override can reach the dialog. Shown raw,
    /// the first makes the old id look identical to a different one, and the
    /// second can reorder the explanation around it. The marker must carry the
    /// raw id as a separate peer run (escaped and direction-isolated when
    /// rendered), keep the app's own words as plain text, and leave a missing
    /// model as the app-authored `default`.
    #[test]
    fn marker_renders_a_stored_model_id_as_an_escaped_peer_run() {
        let raw = "gpt-6-\u{200B}astra\u{202E}x";
        let marker = changed_marker(
            &Some("other".to_string()),
            &Some(raw.to_string()),
            model_marker_label(Some(raw)),
        )
        .expect("a changed model is marked");
        assert_eq!(
            marker,
            vec![DetailPart::text("● Changed · was "), DetailPart::peer(raw)],
            "the raw id stays intact in its own peer run; only rendering escapes it"
        );
        assert_eq!(
            detail_text(&marker),
            "● Changed · was gpt-6-<U+200B>astra<U+202E>x"
        );
        assert_eq!(model_marker_label(None), DetailPart::text("default"));
    }
}
