//! Named secrets have local drafts and metadata-only shared snapshots.
//!
//! These editors deliberately have no recipe-style departure question. Closing
//! discards an unsaved draft; an explicitly submitted save belongs to the API
//! save lifecycle and continues even when its editor disappears.

use dioxus::prelude::*;
use farhelm_proto::credentials::{
    CODEX_LOGIN_CAP, CreateCredential, Credential, CredentialKind, check_secret,
};
use std::{cell::Cell, rc::Rc};

use crate::{
    ApiBase, api,
    tensorlake_settings::{StorageWarning, TensorlakeKey},
};

/// One editor replaces the catalog, while the account form stays mounted behind
/// it so an accepted key save can still publish its shared account metadata.
#[derive(Clone, PartialEq)]
struct Editing {
    kind: CredentialKind,
    existing: Option<Credential>,
}

/// Recipe references are names, so the usage line contains no host file data.
fn usage(credential: &Credential) -> String {
    if credential.used_by.is_empty() {
        "No recipes use this credential.".into()
    } else {
        format!("Used by recipes: {}", credential.used_by.join(", "))
    }
}

/// Epoch seconds are labelled UTC rather than pretending to be the viewer's time.
fn saved_status(credential: &Credential) -> String {
    let secs = credential.updated_at;
    let (year, month, day) = farhelm_proto::time::civil_from_days(secs.div_euclid(86_400));
    let time = secs.rem_euclid(86_400);
    let verb = if credential.revision == 1 {
        "Saved"
    } else {
        "Replaced"
    };
    format!(
        "{verb} {year:04}-{month:02}-{day:02}, {:02}:{:02} UTC",
        time / 3600,
        time % 3600 / 60
    )
}

/// The agent-login list includes two concrete kinds with one shared Add control.
fn in_group(kind: CredentialKind, group: CredentialKind) -> bool {
    kind == group || (group == CredentialKind::ClaudeLogin && kind == CredentialKind::CodexLogin)
}

/// Reveal this opening once, without redirecting input a user already began.
/// Capturing the mounted node also keeps a retired animation frame from
/// focusing a replacement editor after a quick Back and Add.
fn focus_editor() {
    document::eval(
        r#"(() => {
        const editor = document.querySelector('.credential-editor');
        requestAnimationFrame(() => {
            if (!editor?.isConnected || !editor.getClientRects().length) return;
            const active = document.activeElement;
            if (active instanceof HTMLElement && editor.contains(active)) return;
            const body = editor.closest('.host-settings-body');
            if (body) body.scrollTop += editor.getBoundingClientRect().top - body.getBoundingClientRect().top;
            editor.querySelector('.credential-editor-title')?.focus({preventScroll: true});
        });
    })();"#,
    );
}

/// Refreshes preserve the last good catalog and never treat a failure as empty.
#[component]
pub(crate) fn CredentialsSettings() -> Element {
    let base = use_context::<ApiBase>().0;
    let mut rows = use_signal(|| None::<Vec<Credential>>);
    let mut error = use_signal(|| None::<String>);
    let mut generation = use_signal(|| 0_u64);
    let mut editing = use_signal(|| None::<Editing>);
    let reload = use_callback(move |()| {
        generation += 1;
        let mine = generation();
        let base = base.clone();
        spawn(async move {
            let result = api::fetch_credentials(&base).await;
            if generation() != mine {
                return;
            }
            match result {
                Ok(loaded) => {
                    rows.set(Some(loaded));
                    error.set(None);
                }
                Err(reason) => error.set(Some(reason)),
            }
        });
    });
    use_hook(move || reload.call(()));
    crate::feed::use_feed_reader(move || reload.call(()));
    let listed = rows();
    rsx! {
        div { class: "credential-overview", hidden: editing().is_some(),
            p { class: "host-settings-help", "Save credentials under names you choose. Replacing a saved value keeps its name." }
            StorageWarning { multiple: true }
            TensorlakeKey { credentials_location: true }
            if listed.is_none() && error().is_none() {
                p { role: "status", "Loading credentials…" }
            }
            if let Some(reason) = error() {
                p { class: "checkout-root-error", role: "alert", "{reason}" }
                button { r#type: "button", class: "btn btn-neutral", onclick: move |_| reload.call(()),
                    "data-tooltip": "Load the credential catalog again", "Retry loading" }
            }
            for (heading, add, group) in [
                ("GitHub tokens", "Add GitHub token", CredentialKind::GithubToken),
                ("Agent subscription logins", "Add agent login", CredentialKind::ClaudeLogin),
                ("API endpoint tokens", "Add endpoint token", CredentialKind::EndpointToken),
            ] {
                section { class: "host-settings-section credential-group", aria_label: heading,
                    div { class: "credential-group-heading",
                        h4 { class: "host-settings-section-title", "{heading}" }
                        button { r#type: "button", class: "btn btn-neutral", disabled: listed.is_none(),
                            "data-tooltip": add,
                            onclick: move |_| editing.set(Some(Editing { kind: group, existing: None })), "{add}" }
                    }
                    if let Some(entries) = &listed {
                        if !entries.iter().any(|row| in_group(row.kind, group)) {
                            p { class: "host-settings-help", "None saved." }
                        }
                        ul { class: "recipe-list credential-list", aria_label: heading,
                            for credential in entries.iter().filter(|row| in_group(row.kind, group)).cloned() {
                                li { key: "{credential.name}", class: "recipe-row credential-row",
                                    div { class: "recipe-row-main",
                                        span { class: "recipe-row-name font-mono", "{credential.name}" }
                                        span { class: "recipe-row-summary", "{usage(&credential)}" }
                                        span { class: "recipe-row-status", "{saved_status(&credential)}" }
                                    }
                    button { r#type: "button", class: "btn btn-neutral", aria_label: "Edit {credential.name}", "data-tooltip": "Edit {credential.name}",
                                        onclick: move |_| editing.set(Some(Editing { kind: credential.kind, existing: Some(credential.clone()) })), "Edit" }
                                }
                            }
                        }
                    }
                }
            }
        }
        if let Some(target) = editing() {
            CredentialEditor { key: "{target.existing.as_ref().map_or(\"new\", |row| row.name.as_str())}", target,
                on_done: move |_| {
                    editing.set(None); reload.call(());
                    // A fast Add after Back retires this handoff. Capture the
                    // panel now, then yield to any replacement editor or
                    // control the user chose before this frame runs.
                    document::eval(r#"(() => {
                        const panel = document.querySelector('#settings-credentials');
                        requestAnimationFrame(() => {
                            if (!panel?.isConnected || !panel.getClientRects().length || panel.querySelector('.credential-editor')) return;
                            const active = document.activeElement;
                            if (active instanceof HTMLElement && active.matches('button, input, textarea, select, a[href]') && active.getClientRects().length) return;
                            panel.querySelector('h3')?.focus({preventScroll: true});
                        });
                    })();"#);
                } }
        }
    }
}

/// Names and consumers are fixed after creation; only a new secret is editable.
/// A selected Codex file populates the same bounded draft as paste and never
/// saves automatically. Native file-read failures never expose its contents.
#[component]
fn CredentialEditor(target: Editing, on_done: EventHandler<()>) -> Element {
    let base = use_context::<ApiBase>().0;
    let existing = target.existing.clone();
    let replacing = existing.is_some();
    let mut kind = use_signal(move || target.kind);
    let initial_name = existing
        .as_ref()
        .map(|row| row.name.clone())
        .unwrap_or_default();
    let mut name = use_signal(move || initial_name);
    let mut secret = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    let mut file_generation = use_signal(|| 0_u64);
    let mounted = use_hook(|| Rc::new(Cell::new(true)));
    let drop_mounted = mounted.clone();
    use_drop(move || drop_mounted.set(false));
    let save_mounted = mounted.clone();
    let saved = existing.clone();
    let on_save = use_callback(move |()| {
        if *busy.peek() {
            return;
        }
        if farhelm_proto::tensorlake::recipes::check_recipe_name(&name()).is_err() {
            error.set(Some("Use lowercase letters, digits and hyphens; start with a letter, end with a letter or digit, and use at most 32 characters.".into()));
            return;
        }
        if (!replacing || !secret().trim().is_empty())
            && let Err(reason) = check_secret(kind(), &secret())
        {
            error.set(Some(reason.into()));
            return;
        }
        busy.set(true);
        // A selected file is only a draft. Once submitted, its late read must
        // not replace the explicit value whose response this editor will show.
        file_generation += 1;
        error.set(None);
        let request = api::write_credential(
            &base,
            replacing,
            CreateCredential {
                name: name(),
                kind: kind(),
                secret: secret(),
            },
        );
        let mounted = save_mounted.clone();
        dioxus::core::spawn_forever(async move {
            let result = request.await;
            if !mounted.get() {
                return;
            }
            busy.set(false);
            match result {
                Ok(_) => {
                    secret.set(String::new());
                    on_done.call(());
                }
                Err(reason) => error.set(Some(reason)),
            }
        });
    });
    let delete_base = use_context::<ApiBase>().0;
    let on_delete = use_callback(move |()| {
        let Some(row) = &saved else {
            return;
        };
        if *busy.peek() || !row.used_by.is_empty() {
            return;
        }
        busy.set(true);
        error.set(None);
        let request = api::delete_credential(&delete_base, &row.name);
        let mounted = mounted.clone();
        dioxus::core::spawn_forever(async move {
            let result = request.await;
            if !mounted.get() {
                return;
            }
            busy.set(false);
            match result {
                Ok(()) => on_done.call(()),
                Err(reason) => error.set(Some(reason)),
            }
        });
    });
    let agent = matches!(
        kind(),
        CredentialKind::ClaudeLogin | CredentialKind::CodexLogin
    );
    let label = match kind() {
        CredentialKind::GithubToken => "GitHub token",
        CredentialKind::ClaudeLogin | CredentialKind::CodexLogin => "Agent login",
        CredentialKind::EndpointToken => "Endpoint token",
    };
    let title = if replacing {
        format!("{label}: {}", name())
    } else {
        match kind() {
            CredentialKind::GithubToken => "New GitHub token".into(),
            CredentialKind::ClaudeLogin | CredentialKind::CodexLogin => "New agent login".into(),
            CredentialKind::EndpointToken => "New endpoint token".into(),
        }
    };
    let in_use = existing.as_ref().is_some_and(|row| !row.used_by.is_empty());
    rsx! {
        div { class: "credential-editor", onmounted: move |_| focus_editor(),
            button { r#type: "button", class: "btn btn-neutral", "data-tooltip": "Return to credentials without saving this draft",
                onclick: move |_| on_done.call(()), "‹ Credentials" }
            h4 { class: "host-settings-section-title credential-editor-title", tabindex: "-1", "{title}" }
            if agent {
                div { class: "segmented-control", role: "radiogroup", aria_label: "Agent",
                    for (value, label) in [(CredentialKind::ClaudeLogin, "Claude Code"), (CredentialKind::CodexLogin, "Codex")] {
                        button { r#type: "button", role: "radio", aria_checked: kind() == value, disabled: replacing || busy(),
                            onclick: move |_| { file_generation += 1; kind.set(value); secret.set(String::new()); error.set(None); }, "{label}" }
                    }
                }
            }
            label { class: "checkout-root-label", span { "Name" }
                input { class: "credential-name", value: "{name}", readonly: replacing || busy(),
                    autocomplete: "off", autocapitalize: "none", spellcheck: "false",
                    oninput: move |event| { name.set(event.value()); error.set(None); } }
            }
            label { class: "checkout-root-label", span { if kind() == CredentialKind::CodexLogin { "auth.json" } else { "Token" } }
                if kind() == CredentialKind::CodexLogin {
                    textarea { class: "credential-value font-mono", value: "{secret}", readonly: busy(),
                        autocomplete: "off", autocapitalize: "none", spellcheck: "false", rows: "8",
                        placeholder: if replacing { "Login saved; paste a new one to replace it" } else { "Paste the contents of auth.json" },
                        oninput: move |event| { file_generation += 1; secret.set(event.value()); error.set(None); } }
                } else {
                    input { r#type: "password", class: "credential-value", value: "{secret}", readonly: busy(),
                        autocomplete: "new-password", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                        placeholder: if replacing { "Token saved; paste a new one to replace it" } else { "Paste the token" },
                        oninput: move |event| { secret.set(event.value()); error.set(None); } }
                }
            }
            if kind() == CredentialKind::CodexLogin {
                label { class: "credential-file-label", "Choose file…"
                    input { r#type: "file", accept: ".json,application/json", disabled: busy(), aria_label: "Choose auth.json file",
                        onchange: move |event| {
                            let Some(file) = event.files().into_iter().next() else { return; };
                            file_generation += 1;
                            let mine = file_generation();
                            if file.size() > CODEX_LOGIN_CAP as u64 {
                                error.set(Some("A Codex login file must be at most 64 KiB.".into())); return;
                            }
                            spawn(async move {
                                let result = file.read_string().await;
                                if file_generation() != mine { return; }
                                match result {
                                    Ok(text) => { secret.set(text); error.set(None); }
                                    Err(_) => error.set(Some("Could not read that login file.".into())),
                                }
                            });
                        } }
                }
                p { class: "host-settings-help", "On a machine with a browser, run codex login and sign in with ChatGPT, then paste or choose ~/.codex/auth.json." }
            } else if kind() == CredentialKind::ClaudeLogin {
                p { class: "host-settings-help", "Run claude setup-token on a machine where you can sign in to Claude, then paste the token it prints. It starts with sk-ant-oat01- and lasts a year. You need a Pro, Max, Team or Enterprise plan." }
            }
            p { class: "host-settings-help",
                if let Some(row) = &existing { "{usage(row)}" }
                else { "No recipes use this credential." }
            }
            StorageWarning { multiple: true }
            div { class: "credential-editor-actions",
                if let Some(reason) = error() { p { class: "checkout-root-error", role: "alert", "{reason}" } }
                if in_use { p { class: "host-settings-help", "Delete is unavailable while a recipe uses this credential." } }
                crate::modal_dialog::DialogActions {
                    if replacing { button { r#type: "button", class: "btn btn-danger", disabled: busy() || in_use,
                        "data-tooltip": "Delete this saved credential", onclick: move |_| on_delete.call(()), "Delete" } }
                    button { r#type: "button", class: "btn btn-neutral", "data-tooltip": "Discard this unsaved credential draft",
                        onclick: move |_| on_done.call(()), "Cancel" }
                    button { r#type: "button", class: "btn btn-primary", disabled: busy() || (!replacing && secret().trim().is_empty()),
                        "data-tooltip": "Save this credential in the helm's state", onclick: move |_| on_save.call(()), "Save" }
                }
                if busy() { p { role: "status", "Saving…" } }
            }
        }
    }
}
