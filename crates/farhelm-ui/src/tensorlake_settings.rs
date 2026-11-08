//! Settings → Tensorlake: the account side of Tensorlake sandbox hosts.
//!
//! Three groups, each with its own save lifecycle:
//!
//! - The API key is write-only, like the ntfy token. The form never receives
//!   the saved key, a blank field never sends one, and Save, Test and Clear
//!   each report their own outcome inline. Test lists sandboxes with the
//!   SAVED key, never the field's text.
//! - Farhelm's ssh key: until it exists, a Create SSH key button; then the
//!   one-time `tl` command that registers it, with a Copy button. Loading
//!   the section never creates the key, because Settings mounts every section
//!   when it opens and a key made then would be left on helms whose users
//!   never asked for Tensorlake.
//! - "Create Tensorlake sandboxes without asking" is an acknowledged save, as
//!   tunnel consent is: it can authorize billed resources, so the switch shows
//!   only a choice the helm confirmed.
//!
//! Requests survive the dialog closing, but component state is touched only
//! while mounted, as in the other explicit-save fields.
//! The account context belongs to the Settings opening: Credentials mounts the
//! same key form with an independent secret draft and shared saved-key metadata.
//!
//! The recipe list sits between the ssh key and the switch. It is its own
//! component (`crate::tensorlake::recipes::RecipeList`) with its own reads
//! and saves; this section only places it.
//! The orphan list also owns its reads. Acknowledged key saves advance its
//! revision so discovery can recover from a rejected key without retrying
//! on unrelated renders or fleet changes. A successful saved-key Test also
//! resumes failed discovery once. Refresh and remount recover independently;
//! another browser tab's key save does not advance this opening's revisions.

use crate::{
    ApiBase,
    api::{
        create_tensorlake_ssh_key, fetch_tensorlake_settings, save_tensorlake_settings,
        test_tensorlake_settings,
    },
};
use dioxus::prelude::*;
use farhelm_proto::tensorlake::{SaveSettings, Settings, SshKeyStatus, TestResult};
use std::{cell::Cell, rc::Rc};

/// The save the key's Save button sends: the pasted key and nothing else, so
/// a form that loaded before another window changed the switch cannot undo
/// that change.
fn key_save(key: String) -> SaveSettings {
    SaveSettings {
        api_key: Some(key),
        ..SaveSettings::default()
    }
}

/// The Clear button's save: remove the key, touch nothing else.
fn key_clear() -> SaveSettings {
    SaveSettings {
        clear_api_key: true,
        ..SaveSettings::default()
    }
}

/// The switch's save: its own field only.
fn ask_save(create_without_asking: bool) -> SaveSettings {
    SaveSettings {
        create_without_asking: Some(create_without_asking),
        ..SaveSettings::default()
    }
}

/// What a successful Test says, counting every sandbox the key can see.
fn test_message(result: TestResult) -> String {
    match result.sandboxes {
        1 => "The key works. It can see 1 sandbox.".into(),
        count => format!("The key works. It can see {count} sandboxes."),
    }
}

/// The warning at the top of the Tensorlake section, with the maintainer's
/// approved wording and spelling correction. Browser tests compare it verbatim.
pub(crate) const EXPERIMENTAL_WARNING: &str = "EXPERIMENTAL: Tensorlake support is currently early and \
    experimental. User experience is not yet reasonable. Expect rough edges. Things will change randomly between \
    releases. Try at your own risk. You are responsible for making sure sandboxes are not running and billing \
    you when you don't want them to.";

/// One saved account snapshot for both mounted Settings sections. Secret drafts
/// stay with each form; only acknowledged metadata and discovery revisions are
/// shared. Serializing key actions also prevents two hidden forms racing saves.
#[derive(Clone, Copy)]
pub(crate) struct AccountState {
    load: Resource<Result<Settings, String>>,
    settings: Signal<Option<Settings>>,
    key_revision: Signal<u64>,
    test_revision: Signal<u64>,
    busy: Signal<bool>,
}

/// Install the account owner above both panels for this opening of Settings.
/// Closing drops unsaved fields; a section change only hides their components.
pub(crate) fn provide_account() {
    let base = use_context::<ApiBase>().0;
    let load = use_resource(move || {
        let base = base.clone();
        async move { fetch_tensorlake_settings(&base).await }
    });
    let mut settings = use_signal(|| None::<Settings>);
    let key_revision = use_signal(|| 0_u64);
    let test_revision = use_signal(|| 0_u64);
    let busy = use_signal(|| false);
    use_effect(move || {
        if settings.peek().is_none()
            && let Some(Ok(loaded)) = load.read().as_ref()
        {
            settings.set(Some(loaded.clone()));
        }
    });
    use_context_provider(|| AccountState {
        load,
        settings,
        key_revision,
        test_revision,
        busy,
    });
}

/// The same write-only key form in both panels, with separate replacement drafts.
/// Saved metadata and discovery revisions belong to the enclosing Settings dialog.
/// In Credentials the overview's one notice already covers this key, and the
/// label names Tensorlake to distinguish it from the named secrets. On the
/// Tensorlake page the form shows its own notice and the page names the product.
#[component]
pub(crate) fn TensorlakeKey(#[props(default)] credentials_location: bool) -> Element {
    let base = use_context::<ApiBase>().0;
    let account = use_context::<AccountState>();
    let mut load = account.load;
    let mut settings = account.settings;
    let mut key_revision = account.key_revision;
    let mut test_revision = account.test_revision;
    let mut busy = account.busy;
    let mut key = use_signal(String::new);
    let mut outcome = use_signal(|| None::<Result<String, String>>);
    let mounted = use_hook(|| Rc::new(Cell::new(true)));
    let drop_mounted = mounted.clone();
    use_drop(move || drop_mounted.set(false));
    let load_error = load
        .read()
        .as_ref()
        .and_then(|result| result.as_ref().err())
        .cloned();
    let snapshot = settings();
    let loaded = snapshot.is_some();
    let key_set = snapshot.as_ref().is_some_and(|s| s.api_key_set);
    let locked = !loaded || busy();
    let save_base = base.clone();
    let save_mounted = mounted.clone();
    let run_key_save = use_callback(move |(request, success): (SaveSettings, &'static str)| {
        if *busy.peek() {
            return;
        }
        busy.set(true);
        outcome.set(None);
        let mounted = save_mounted.clone();
        let saving = save_tensorlake_settings(&save_base, request);
        dioxus::core::spawn_forever(async move {
            let result = saving.await;
            if !mounted.get() {
                return;
            }
            busy.set(false);
            match result {
                Ok(saved) => {
                    // Other account controls own their fields; an older key
                    // reply cannot roll back a newer acknowledged switch.
                    if let Some(current) = settings.write().as_mut() {
                        current.api_key_set = saved.api_key_set;
                    }
                    key_revision += 1;
                    key.set(String::new());
                    outcome.set(Some(Ok(success.into())));
                }
                Err(error) => outcome.set(Some(Err(error))),
            }
        });
    });
    let test_base = base.clone();
    let test_mounted = mounted.clone();
    rsx! {
        section { class: "host-settings-section tensorlake-settings", aria_label: "Tensorlake API key",
            // A failed load seeds nothing editable: every control stays
            // locked until a retry brings the helm's own snapshot.
            if let Some(error) = load_error {
                p { class: "checkout-root-error", role: "alert", "{error}" }
                button { r#type: "button", class: "btn btn-neutral",
                    "data-tooltip": "Load the saved Tensorlake settings again",
                    onclick: move |_| load.restart(), "Retry loading" }
            }
            if !credentials_location {
                StorageWarning { multiple: false }
            }
            label { class: "checkout-root-label",
                span { if credentials_location { "Tensorlake API key" } else { "API key" } }
                // `new-password`, not `off`: browsers ignore `off` when they
                // fill saved logins into a password field, and the helm's own
                // web token is typically saved for this same origin. Filled in
                // here and saved, it would be sent to Tensorlake as the key.
                input { r#type: "password", class: "tensorlake-key", value: "{key}", readonly: locked,
                    autocomplete: "new-password", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                    "data-tooltip": if key_set { "Paste a new Tensorlake API key to replace the saved one" } else { "Paste a Tensorlake API key" },
                    placeholder: if !loaded { "" } else if key_set { "Key saved; paste a new one to replace it" } else { "No key saved" },
                    oninput: move |event| { key.set(event.value()); outcome.set(None); } }
            }
            p { class: "host-settings-help",
                "Create a key in the Tensorlake dashboard, in the project you want sandboxes in. Test lists your sandboxes to check that the saved key works."
            }
            p { class: "host-settings-help",
                if credentials_location { "The same key as in Settings → Tensorlake. Changing it here changes it there." }
                else { "You can also change it in Settings → Credentials." }
            }
            crate::modal_dialog::DialogActions {
                button { r#type: "button", class: "btn btn-neutral", disabled: locked || !key_set,
                    "data-tooltip": "List your Tensorlake sandboxes with the saved key, to check that it works",
                    onclick: move |_| {
                        if locked || !key_set { return; }
                        busy.set(true); outcome.set(None);
                        let base = test_base.clone(); let mounted = test_mounted.clone();
                        dioxus::core::spawn_forever(async move {
                            let result = test_tensorlake_settings(&base).await;
                            if !mounted.get() { return; }
                            busy.set(false);
                            if result.is_ok() { test_revision += 1; }
                            outcome.set(Some(result.map(test_message)));
                        });
                    }, "Test"
                }
                button { r#type: "button", class: "btn btn-neutral", disabled: locked || !key_set,
                    "data-tooltip": "Remove the saved API key from this helm",
                    onclick: move |_| {
                        if locked || !key_set { return; }
                        run_key_save.call((key_clear(), "API key cleared"));
                    }, "Clear"
                }
                button { r#type: "button", class: "btn btn-primary", disabled: locked || key().trim().is_empty(),
                    "data-tooltip": "Save this API key for every window using this helm",
                    onclick: move |_| {
                        if locked || key().trim().is_empty() { return; }
                        run_key_save.call((key_save(key()), "API key saved"));
                    }, "Save"
                }
            }
            if busy() { p { role: "status", "Working…" } }
            if let Some(result) = outcome() {
                match result {
                    Ok(message) => rsx! { p { class: "checkout-root-success", role: "status", "{message}" } },
                    Err(error) => rsx! { p { class: "checkout-root-error", role: "alert", "{error}" } },
                }
            }
        }
    }
}

/// Plaintext storage is a deliberate policy the user sees before entering a secret.
/// The Credentials overview names the Tensorlake key because its one notice
/// covers that form too; individual credential editors cover only their values.
/// `includes_tensorlake` applies only to the plural notice (`multiple: true`).
#[component]
pub(crate) fn StorageWarning(
    multiple: bool,
    #[props(default)] includes_tensorlake: bool,
) -> Element {
    rsx! {
        div { class: "credential-storage-warning", role: "note",
            strong { "Not encrypted on disk" }
            p {
                if multiple {
                    "Farhelm stores these"
                    if includes_tensorlake { ", including the Tensorlake API key," }
                    " in plain text in this helm's state, on the machine the helm runs on. Anyone who can read that folder can read them. Farhelm never shows a saved value again."
                }
                else { "Farhelm stores this key in plain text in this helm's state, on the machine the helm runs on. Anyone who can read that folder can read it. Farhelm never shows a saved value again." }
            }
        }
    }
}

/// Account controls, recipes and cleanup share the key's acknowledged state.
/// This panel owns the non-key fields and never stores a credential draft.
#[component]
pub(crate) fn TensorlakeSettings() -> Element {
    let base = use_context::<ApiBase>().0;
    let account = use_context::<AccountState>();
    let mut settings = account.settings;
    let key_revision = account.key_revision;
    let test_revision = account.test_revision;
    let mut toggling = use_signal(|| false);
    let mut toggle_error = use_signal(|| None::<String>);
    let mut copied = use_signal(|| None::<u64>);
    let mut copy_clicks = use_signal(|| 0_u64);
    let mut creating = use_signal(|| false);
    let mut create_error = use_signal(|| None::<String>);
    let mounted = use_hook(|| Rc::new(Cell::new(true)));
    let drop_mounted = mounted.clone();
    use_drop(move || drop_mounted.set(false));
    let snapshot = settings();
    let loaded = snapshot.is_some();
    let key_set = snapshot.as_ref().is_some_and(|s| s.api_key_set);
    let ask_skipped = snapshot.as_ref().is_some_and(|s| s.create_without_asking);
    let create_base = base.clone();
    let create_mounted = mounted.clone();
    let toggle_base = base.clone();
    let toggle_mounted = mounted.clone();
    rsx! {
        // First in the section, ahead of the API key that every Tensorlake user
        // has to set up, so nobody starts without seeing it. The wording is the
        // maintainer's, reproduced exactly (2026-11-04); change it only on their
        // say. It shows unconditionally, even when Settings cannot load.
        div { class: "tensorlake-experimental", role: "note", aria_label: "Tensorlake support is experimental",
            p { "{EXPERIMENTAL_WARNING}" }
        }
        p { class: "host-settings-help",
            "Tensorlake sandboxes are cloud machines Farhelm can create and use as hosts. Farhelm creates them in the Tensorlake project your API key belongs to."
        }
        TensorlakeKey {}
        section { class: "host-settings-section tensorlake-settings", aria_label: "Farhelm's SSH key",
            h4 { class: "host-settings-section-title", "SSH key" }
            p { class: "host-settings-help",
                "Farhelm reaches sandboxes over SSH with a key of its own, kept in this helm's state. Register it once with your Tensorlake account by running the command below on any machine where Tensorlake's tl CLI is signed in."
            }
            match snapshot.as_ref().map(|s| &s.ssh_key) {
                None => rsx! {},
                Some(SshKeyStatus::NotCreated) => rsx! {
                    p { class: "host-settings-help", "This helm has no SSH key for Tensorlake yet." }
                    button { r#type: "button", class: "btn btn-neutral", disabled: creating(),
                        "data-tooltip": "Create the SSH key Farhelm uses for Tensorlake sandboxes, in this helm's state",
                        onclick: move |_| {
                            if *creating.peek() { return; }
                            creating.set(true); create_error.set(None);
                            let base = create_base.clone(); let mounted = create_mounted.clone();
                            dioxus::core::spawn_forever(async move {
                                let result = create_tensorlake_ssh_key(&base).await;
                                if !mounted.get() { return; }
                                creating.set(false);
                                match result {
                                    Ok(saved) => { if let Some(current) = settings.write().as_mut() { current.ssh_key = saved.ssh_key; } },
                                    Err(error) => create_error.set(Some(error)),
                                }
                            });
                        }, "Create SSH key"
                    }
                },
                Some(SshKeyStatus::Ready { register_command, .. }) => {
                    let command = register_command.clone();
                    rsx! {
                        div { class: "tensorlake-command",
                            pre { class: "tensorlake-register-command", dir: "ltr", "{register_command}" }
                            button { r#type: "button", class: "btn btn-neutral", aria_label: "Copy the registration command",
                                "data-tooltip": "Copy: the command that registers Farhelm's SSH key with your Tensorlake account",
                                onclick: move |_| {
                                    crate::session_view::copy_to_clipboard(&command);
                                    let click = { let mut clicks = copy_clicks.write(); *clicks += 1; *clicks };
                                    copied.set(Some(click));
                                    spawn(async move {
                                        crate::reader::sleep_ms(1500).await;
                                        if *copied.peek() == Some(click) { copied.set(None); }
                                    });
                                },
                                "Copy" }
                        }
                        if copied().is_some() { p { class: "checkout-root-success", role: "status", "Command copied" } }
                    }
                }
                Some(SshKeyStatus::Unavailable { reason }) => rsx! {
                    p { class: "checkout-root-error", role: "alert", "Farhelm has no usable SSH key for Tensorlake: {reason}" }
                },
            }
            if let Some(error) = create_error() {
                p { class: "checkout-root-error", role: "alert", "{error}" }
            }
            p { class: "host-settings-help",
                "Farhelm cannot check that the key is registered, because an API key cannot list SSH keys. An unregistered key shows up as an error the first time Farhelm sets up a sandbox."
            }
        }
        // Account, recipes and explicit cleanup share the saved API key.
        crate::tensorlake::recipes::RecipeList { api_key_set: key_set }
        crate::tensorlake::orphans::OrphanList { api_key_set: key_set, key_revision: key_revision(), test_revision: test_revision() }
        section { class: "host-settings-section tensorlake-settings", aria_label: "Creating sandboxes",
            h4 { class: "host-settings-section-title", "Creating sandboxes" }
            div { class: "setting-group",
                label { class: "app-settings-choice setting-row",
                    span { "Create Tensorlake sandboxes without asking" }
                    input { r#type: "checkbox", class: "control-switch tensorlake-ask-switch",
                        checked: ask_skipped, disabled: !loaded || toggling(), aria_describedby: "tensorlake-ask-help",
                        "data-tooltip": "Skip the confirmation when adding a Tensorlake host, which creates a sandbox that bills while it runs",
                        onchange: move |event| {
                            let wanted = event.checked();
                            // The native box has already flipped. Keep it at the
                            // acknowledged choice until the helm confirms the save.
                            document::eval(&format!("document.querySelector('.tensorlake-ask-switch').checked = {ask_skipped};"));
                            if *toggling.peek() { return; }
                            toggling.set(true); toggle_error.set(None);
                            let mounted = toggle_mounted.clone();
                            let saving = save_tensorlake_settings(&toggle_base, ask_save(wanted));
                            dioxus::core::spawn_forever(async move {
                                let result = saving.await;
                                if !mounted.get() { return; }
                                toggling.set(false);
                                match result {
                                    Ok(saved) => { if let Some(current) = settings.write().as_mut() { current.create_without_asking = saved.create_without_asking; } },
                                    Err(error) => toggle_error.set(Some(error)),
                                }
                            });
                        }
                    }
                }
                p { id: "tensorlake-ask-help", class: "host-settings-help",
                    "Adding a Tensorlake host creates a sandbox, which bills while it runs, so Farhelm asks first. This skips that question. \"Set up new hosts without asking\" under Hosts never does."
                }
                if let Some(error) = toggle_error() {
                    p { class: "app-settings-error", role: "alert", "{error}" }
                }
            }
            p { class: "host-settings-help",
                "On Tensorlake's Usage Credits plan, Tensorlake suspends a sandbox after 24 hours without a connection from Farhelm, for example while this helm's machine is asleep. Its sessions continue where they were when you resume it."
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Why: the key is write-only and the switch is a separate save, so each
    /// control must send only the field it owns; anything more would let a
    /// form that never saw a newer value overwrite it.
    ///
    /// Spec: Save sends only the key, Clear only the clear flag, the switch
    /// only its choice, and none of them serializes another field.
    #[test]
    fn each_control_sends_only_its_own_field() {
        let wire = |save: SaveSettings| serde_json::to_value(save).unwrap();
        assert_eq!(
            wire(key_save("k".into())),
            serde_json::json!({ "api_key": "k" })
        );
        assert_eq!(
            wire(key_clear()),
            serde_json::json!({ "clear_api_key": true })
        );
        assert_eq!(
            wire(ask_save(false)),
            serde_json::json!({ "create_without_asking": false })
        );
    }

    /// Why: the count is the visible proof that the key reached the account.
    ///
    /// Spec: the message counts sandboxes with the right plural.
    #[test]
    fn the_test_message_counts_sandboxes() {
        assert_eq!(
            test_message(TestResult { sandboxes: 1 }),
            "The key works. It can see 1 sandbox."
        );
        assert_eq!(
            test_message(TestResult { sandboxes: 0 }),
            "The key works. It can see 0 sandboxes."
        );
    }
}
