//! Shared structured launch controls for the session launcher and restart-with dialog.
//!
//! Values enter as props and user changes leave through callbacks. The caller owns
//! the selected harness, model draft, and every consequential state transition;
//! these controls do not choose a harness, promote history, or manage create
//! idempotency. Keeping that boundary lets a fixed-harness restart dialog use
//! the same model, effort, permission, and trust controls as the launcher.

use dioxus::prelude::*;

use crate::api::LaunchCatalogModel;
use crate::launch_composer::{self, ModelEnterTarget, ModelOption};
use crate::peer::{DetailPart, PeerLine, display_peer};
use crate::{LaunchEffort, LaunchHarness, LaunchPermission, LaunchSelection};

/// Render a baseline value only when this field now differs from it.
///
/// The marker compares wire values, so restoring a choice clears it even if
/// the person visited other options while editing.
///
/// `old_label` arrives as a [`DetailPart`] because the fields differ in who
/// wrote it. Effort, permissions, trust, and the `default` placeholder are
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
    (current != baseline).then(|| vec![DetailPart::text("● changed · was "), old_label])
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
#[component]
pub(crate) fn LaunchControls(
    harness: Option<LaunchHarness>,
    model: Option<String>,
    model_raw_seed: Option<String>,
    model_edited: bool,
    effort: Option<LaunchEffort>,
    permissions: Option<LaunchPermission>,
    workspace_trust: Option<bool>,
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
    on_model_focus: EventHandler<()>,
    on_model_input: EventHandler<String>,
    on_model_blur: EventHandler<()>,
    on_model_escape: EventHandler<()>,
    on_model_active: EventHandler<Option<usize>>,
    on_model_enter: EventHandler<ModelEnterTarget>,
    on_model_option: EventHandler<ModelOption>,
    on_effort: EventHandler<Option<LaunchEffort>>,
    on_permissions: EventHandler<Option<LaunchPermission>>,
    on_workspace_trust: EventHandler<Option<bool>>,
) -> Element {
    let efforts = harness
        .map(|harness| launch_composer::compatible_efforts(harness, model.as_deref(), &catalog))
        .unwrap_or_default();
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
            None => "harness default".to_string(),
        }
    };
    let model_hint = harness
        .map(|harness| {
            format!(
                "{} for {harness:?}",
                catalog
                    .iter()
                    .filter(|model| model.harness == harness)
                    .count()
            )
        })
        .unwrap_or_else(|| "choose a harness".to_string());
    let model_results_id = format!("{model_id_prefix}-results");
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
                    .map(launch_composer::permission_value)
                    .unwrap_or("default"),
            ),
        )
    });
    let trust_change = baseline.as_ref().and_then(|old| {
        changed_marker(
            &workspace_trust,
            &old.workspace_trust,
            DetailPart::text(
                old.workspace_trust
                    .map(|value| if value { "true" } else { "false" })
                    .unwrap_or("default"),
            ),
        )
    });

    rsx! {
        if harness != Some(LaunchHarness::Grok) {
            div { class: "launch-composer-choice launch-composer-model-choice",
                span { class: "launch-composer-section-label", "model" }
                if let Some(parts) = model_change { PeerLine { class: "launch-composer-changed-marker", parts } }
                div { class: "launch-composer-model",
                    input {
                        r#type: "text",
                        role: "combobox",
                        aria_label: "model",
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
                                    // Enter is consumed even with the list closed: a model field
                                    // keystroke must never implicitly submit its containing form.
                                    Key::Enter if !evt.is_composing() => {
                                        evt.prevent_default();
                                        if model_open {
                                            on_model_enter.call(launch_composer::model_enter_target(
                                                &options, model_active, &model_draft, &catalog, harness,
                                            ));
                                        }
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
                                    class: if model_active == Some(index) { "selected" } else { "" },
                                    disabled: busy,
                                    // Rows are not sequential tab stops. The keyboard reaches
                                    // them through the input's `aria-activedescendant` (Arrow
                                    // and Enter above), which is the ARIA combobox pattern. A
                                    // tabbable row would be the target of Tab from the input,
                                    // but the input's blur closes the list and unmounts that
                                    // row, so focus would fall to the document: out of the
                                    // restart-with modal, and out of the launcher's Tab loop.
                                    tabindex: "-1",
                                    // Keep focus on the input until a transient row's click
                                    // applies the choice; blur would otherwise unmount the row.
                                    onmousedown: move |evt| evt.prevent_default(),
                                    onclick: {
                                        let option = option.clone();
                                        move |_| on_model_option.call(option.clone())
                                    },
                                    match &option {
                                        ModelOption::HarnessDefault => rsx! { "harness default" },
                                        ModelOption::Model { id, harness: owner } => rsx! {
                                            "{display_peer(id)}"
                                            if model_show_all || harness.is_none() { " ({owner:?})" }
                                        },
                                        ModelOption::ShowAll => rsx! {
                                            if model_show_all {
                                                "show chosen harness's models"
                                            } else {
                                                "show every harness's models"
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
        // These harnesses offer no effort vocabulary. Permissions occupies
        // the pair alone so a blank sibling does not imply a missing setting.
        div { class: "launch-composer-choice-pair",
            if !matches!(harness, Some(LaunchHarness::OpenCode | LaunchHarness::Cursor | LaunchHarness::Grok)) {
                div { class: "launch-composer-choice launch-composer-effort-choice",
                    span { class: "launch-composer-section-label", "effort" }
                    if let Some(parts) = effort_change { PeerLine { class: "launch-composer-changed-marker", parts } }
                    div { class: "launch-composer-segmented",
                        button {
                            r#type: "button",
                            class: if effort.is_none() { "selected" } else { "" },
                            aria_pressed: effort.is_none(),
                            disabled: busy,
                            onclick: move |_| on_effort.call(None),
                            "default"
                        }
                        for offered in efforts {
                            button {
                                key: "{launch_composer::effort_value(offered)}",
                                r#type: "button",
                                class: if effort == Some(offered) { "selected" } else { "" },
                                aria_pressed: effort == Some(offered),
                                disabled: busy,
                                onclick: move |_| on_effort.call(Some(offered)),
                                "{launch_composer::effort_value(offered)}"
                            }
                        }
                    }
                }
            }
            div { class: "launch-composer-choice launch-composer-permissions-choice",
                span { class: "launch-composer-section-label", "permissions" }
                if let Some(parts) = permission_change { PeerLine { class: "launch-composer-changed-marker", parts } }
                div { class: "launch-composer-segmented",
                    if harness == Some(LaunchHarness::Pi) {
                        // Pi has no tool-approval gate. Its sole mode cannot
                        // expose the unrelated workspace `--approve` flag.
                        button {
                            r#type: "button",
                            class: "selected launch-composer-segment-danger",
                            aria_pressed: true,
                            disabled: busy,
                            onclick: move |_| on_permissions.call(Some(LaunchPermission::Yolo)),
                            "yolo"
                        }
                    } else {
                        button {
                            r#type: "button",
                            class: if permissions.is_none() { "selected" } else { "" },
                            aria_pressed: permissions.is_none(),
                            disabled: busy,
                            onclick: move |_| on_permissions.call(None),
                            "default"
                        }
                        button {
                            r#type: "button",
                            class: if permissions == Some(LaunchPermission::Yolo) {
                                "selected launch-composer-segment-danger"
                            } else {
                                "launch-composer-segment-danger"
                            },
                            aria_pressed: permissions == Some(LaunchPermission::Yolo),
                            disabled: busy,
                            onclick: move |_| on_permissions.call(Some(LaunchPermission::Yolo)),
                            "yolo"
                        }
                    }
                    if harness == Some(LaunchHarness::Goose) {
                        for (permission, label) in [
                            (LaunchPermission::Approve, "approve"),
                            (LaunchPermission::SmartApprove, "smart approve"),
                            (LaunchPermission::Chat, "chat"),
                        ] {
                            button {
                                key: "{label}",
                                r#type: "button",
                                class: if permissions == Some(permission) { "selected" } else { "" },
                                aria_pressed: permissions == Some(permission),
                                disabled: busy,
                                onclick: move |_| on_permissions.call(Some(permission)),
                                "{label}"
                            }
                        }
                    }
                    if harness == Some(LaunchHarness::Omp) {
                        // OMP has an omitted default and only one offered
                        // approval mode beside YOLO, unlike Pi or Goose.
                        for (permission, label) in [(LaunchPermission::Approve, "approve")] {
                            button {
                                key: "{label}",
                                r#type: "button",
                                class: if permissions == Some(permission) { "selected" } else { "" },
                                aria_pressed: permissions == Some(permission),
                                disabled: busy,
                                onclick: move |_| on_permissions.call(Some(permission)),
                                "{label}"
                            }
                        }
                    }
                }
            }
        }
        if harness.is_some_and(|harness| matches!(harness, LaunchHarness::Codex | LaunchHarness::Muse | LaunchHarness::Pi)) {
            div { class: "launch-composer-choice launch-composer-trust-choice",
                span { class: "launch-composer-section-label", "workspace trust" }
                if let Some(parts) = trust_change { PeerLine { class: "launch-composer-changed-marker", parts } }
                if harness == Some(LaunchHarness::Muse) {
                    p { class: "launch-composer-choice-help", "Muse false adds no trust flag; YOLO or vendor settings may still trust this workspace." }
                }
                if harness == Some(LaunchHarness::Codex) {
                    p { class: "launch-composer-choice-help", "Codex true trusts this directory for this launch; false runs it as untrusted. Default uses Codex's own setting or prompt." }
                }
                div { class: "launch-composer-segmented",
                    for (choice, label) in [(None, "default"), (Some(true), "true"), (Some(false), "false")] {
                        button {
                            key: "{label}",
                            r#type: "button",
                            class: if workspace_trust == choice { "selected" } else { "" },
                            aria_pressed: workspace_trust == choice,
                            disabled: busy,
                            onclick: move |_| on_workspace_trust.call(choice),
                            "{label}"
                        }
                    }
                }
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
        assert_eq!(detail_text(&marker), "● changed · was default");
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
            vec![DetailPart::text("● changed · was "), DetailPart::peer(raw)],
            "the raw id stays intact in its own peer run; only rendering escapes it"
        );
        assert_eq!(
            detail_text(&marker),
            "● changed · was gpt-6-<U+200B>astra<U+202E>x"
        );
        assert_eq!(model_marker_label(None), DetailPart::text("default"));
    }
}
