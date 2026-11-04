//! The Tensorlake half of Add host. A recipe is saved before probing; the
//! billed creation confirmation uses only Tensorlake's acknowledged setting.
//! The general SSH setup preference never authorizes sandbox creation.

use crate::hosts::settings_dialog;
use crate::modal_dialog::{DialogActions, DialogFocus, ModalDialog};
use crate::tensorlake::recipe_editor::{DialogCloseGuard, RecipeEditor};
use crate::{
    ApiBase,
    api::{self, ProbeResponse, ProvisioningSubmission, SubmissionError},
    ops::{ConfirmSlot, ConfirmToken, OpLock, use_confirm_slot},
};
use dioxus::prelude::*;

/// Take only the opening the answer displayed, then claim the page before
/// consuming it. Cancellation and replacement must authorize no billed POST.
fn take_create_offer<G>(
    slot: &mut ConfirmSlot<ProbeResponse>,
    shown: &ConfirmToken<ProbeResponse>,
    claim: impl FnOnce() -> Option<G>,
) -> Option<G> {
    if !slot.is_current(shown) {
        return None;
    }
    let claim = claim()?;
    slot.take(shown)?;
    Some(claim)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// A cancelled or replaced opening must authorize no create, even when
    /// the next question has identical contents. Only its own fresh token
    /// may claim the page and consume the one-use probe.
    #[farhelm_testtrace::test]
    fn cancelled_and_stale_creation_answers_send_nothing() {
        std::thread_local! {
            static SLOT: Cell<Option<ConfirmSlot<ProbeResponse>>> = const { Cell::new(None) };
        }
        fn app() -> Element {
            let slot = use_confirm_slot::<ProbeResponse, ()>();
            SLOT.with(|cell| cell.set(Some(slot)));
            rsx! {}
        }
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        dom.in_runtime(|| {
            let mut slot = SLOT.with(|cell| cell.get().unwrap());
            let plan = ProbeResponse::TensorlakeProvisionable {
                probe_id: "one-use".into(),
                sandbox: "sandbox".into(),
                destination: "destination".into(),
                recipe: "recipe".into(),
                resources: farhelm_proto::tensorlake::recipes::RecipeSize {
                    cpus: 2,
                    memory_gb: 4,
                    disk_gb: 20,
                },
                confirmation: "bills while running".into(),
                steps: None,
            };
            let claims = Cell::new(0);
            let claim = || {
                claims.set(claims.get() + 1);
                Some(())
            };
            let cancelled = slot.open(plan.clone(), ());
            slot.clear(); // Accepted close retires before parent unmount.
            assert!(take_create_offer(&mut slot, &cancelled, claim).is_none());
            let replaced = slot.open(plan.clone(), ());
            let current = slot.open(plan, ());
            assert!(take_create_offer(&mut slot, &replaced, claim).is_none());
            assert_eq!(claims.get(), 0, "stale answers cannot even claim the page");
            assert!(take_create_offer(&mut slot, &current, || None::<()>).is_none());
            assert!(
                slot.is_current(&current),
                "a busy page leaves the question open"
            );
            assert!(take_create_offer(&mut slot, &current, claim).is_some());
            assert!(take_create_offer(&mut slot, &current, claim).is_none());
            assert_eq!(claims.get(), 1);
        });
    }
}

/// A single add attempt. Taking the offer before posting makes double-clicks
/// harmless; an ambiguous reply closes and refreshes, since it may have
/// created a host even though the browser did not learn its id.
#[component]
pub(crate) fn TensorlakeAddDialog(
    mut ops: OpLock,
    on_ssh: EventHandler<()>,
    on_added: EventHandler<Option<String>>,
    on_cancel: EventHandler<()>,
) -> Element {
    let base = use_context::<ApiBase>().0;
    let mut name = use_signal(String::new);
    let mut recipe = use_signal(String::new);
    let mut editing = use_signal(|| false);
    let mut settings_open = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    let mut offer = use_confirm_slot::<ProbeResponse, ()>();
    let mounted = use_hook(|| std::rc::Rc::new(std::cell::Cell::new(true)));
    let drop_mounted = mounted.clone();
    use_drop(move || drop_mounted.set(false));
    let close_guard = DialogCloseGuard::provide();
    let load_base = base.clone();
    let mut load = use_resource(move || {
        let base = load_base.clone();
        async move {
            let settings = api::fetch_tensorlake_settings(&base).await?;
            let recipes = api::fetch_recipes(&base).await?;
            Ok::<_, String>((settings, recipes))
        }
    });
    let submit_base = base.clone();
    let confirm = use_callback(move |shown: ConfirmToken<ProbeResponse>| {
        if busy() {
            return;
        }
        let Some(claim) = take_create_offer(&mut offer, &shown, || ops.claim_guard()) else {
            return;
        };
        let ProbeResponse::TensorlakeProvisionable { probe_id, .. } = shown.key().clone() else {
            return;
        };
        busy.set(true);
        let base = submit_base.clone();
        let mounted = mounted.clone();
        dioxus::core::spawn_forever(async move {
            let _claim = claim;
            let result = api::provision_host(&base, &probe_id).await;
            if !mounted.get() {
                return;
            }
            match result {
                Ok(ProvisioningSubmission::Accepted(_)) => on_added.call(None),
                Ok(ProvisioningSubmission::Unvalidated(reason))
                | Err(SubmissionError::Ambiguous(reason)) => on_added.call(Some(reason)),
                Err(SubmissionError::Refused(reason)) => {
                    busy.set(false);
                    error.set(Some(reason));
                }
            }
        });
    });
    let probe_base = base.clone();
    let probe = use_callback(move |_: ()| {
        if busy() || offer.peek_key().is_some() {
            return;
        }
        if name.peek().trim().is_empty() {
            error.set(Some("Enter a name for the host".into()));
            return;
        }
        if recipe.peek().is_empty() {
            error.set(Some("Choose a recipe".into()));
            return;
        }
        let skip = load
            .peek()
            .as_ref()
            .and_then(|result| result.as_ref().ok())
            .is_some_and(|(settings, _)| settings.create_without_asking);
        let base = probe_base.clone();
        let name = name();
        let recipe = recipe();
        busy.set(true);
        error.set(None);
        spawn(async move {
            let result = api::probe_tensorlake_host(&base, &name, &recipe, None).await;
            busy.set(false);
            match result {
                Ok(planned @ ProbeResponse::TensorlakeProvisionable { .. }) => {
                    let opened = offer.open(planned, ());
                    if skip {
                        confirm.call(opened);
                    }
                }
                Ok(_) => error.set(Some(
                    "The helm did not return a Tensorlake creation plan".into(),
                )),
                Err(reason) => error.set(Some(reason)),
            }
        });
    });
    // The editor may defer departure. Retire authority in the accepted
    // close itself, before parent notification and the next render/unmount.
    let close = use_callback(move |_| {
        offer.clear();
        on_cancel.call(());
    });
    let cancel = use_callback(move |_| {
        close_guard.request(close);
    });
    if settings_open() {
        return rsx! { crate::settings::SettingsDialog { focus_tensorlake: true,
            on_close: move |_| { settings_open.set(false); load.restart(); }
        } };
    }
    let loaded = load.read().clone();
    let shown = offer.current();
    let planned = shown.as_ref().map(|shown| shown.key().clone());
    let locked = busy() || ops.busy();
    rsx! {
        div { class: "host-settings-backdrop", role: "presentation",
            ModalDialog {
                handle: settings_dialog::ADD_DIALOG,
                focus: DialogFocus::remembered(".add-tensorlake-name", Some(".add-host-cancel"), false, None),
                class: "host-settings-dialog host-add-dialog host-dialog-framed add-host-form",
                role: "dialog", aria_modal: "true", aria_label: "Add host", tabindex: "-1",
                on_escape: move |evt: KeyboardEvent| { if !evt.is_composing() { cancel.call(()); } },
                settings_dialog::HostDialogHeader {
                    title: rsx! { "Add host" }, summary: "A Tensorlake sandbox created from a recipe",
                    dismiss_label: "Cancel adding host", dismiss_tooltip: "Close without adding a host",
                    on_dismiss: move |_| cancel.call(()),
                }
                div { class: "dialog-body host-settings-body",
                    if let Some(ProbeResponse::TensorlakeProvisionable { sandbox, resources, steps, .. }) = planned {
                        h3 { "Create Tensorlake sandbox?" }
                        p { class: "peer-value", "{crate::peer::display_peer(sandbox.raw())}" }
                        p { "{resources.cpus} CPUs · {resources.memory_gb} GB memory · {resources.disk_gb} GB disk" }
                        if let Some(steps) = steps {
                            ol { class: "provisioning-plan-steps",
                                for step in steps.steps { li { "{crate::peer::display_peer(step.name.raw())}" } }
                            }
                        }
                        p { "This sandbox bills while running. "
                            a { href: "https://www.tensorlake.ai/pricing", target: "_blank", rel: "noopener noreferrer", "Tensorlake pricing" }
                        }
                        p { "On Usage Credits, Tensorlake suspends a sandbox after 24 hours without a connection. Farhelm never resumes it on its own." }
                    } else if editing() {
                        RecipeEditor { existing: None, allow_delete: false,
                            on_saved: move |saved: farhelm_proto::tensorlake::recipes::RecipeView| { recipe.set(saved.name); editing.set(false); load.restart(); },
                            on_cancel: move |_| editing.set(false),
                        }
                    } else {
                        div { class: "segmented-control", role: "group", aria_label: "Host kind",
                            button { r#type: "button", aria_pressed: "false", disabled: locked, onclick: move |_| on_ssh.call(()), "SSH" }
                            button { r#type: "button", aria_pressed: "true", "Tensorlake sandbox" }
                        }
                        match loaded {
                            None => rsx! { p { "Loading Tensorlake settings…" } },
                            Some(Err(reason)) => rsx! { p { role: "alert", "{reason}" } button { r#type: "button", onclick: move |_| load.restart(), "Retry" } },
                            Some(Ok((settings, recipes))) => rsx! {
                                if !settings.api_key_set {
                                    p { "Save a Tensorlake API key in Settings → Tensorlake before adding a sandbox." }
                                    button { r#type: "button", class: "btn btn-primary", onclick: move |_| { settings_dialog::ADD_DIALOG.release(); settings_open.set(true); }, "Open Settings → Tensorlake" }
                                } else {
                                    label { "Name"
                                        input { class: "add-tensorlake-name", r#type: "text", required: true, value: "{name}", disabled: locked,
                                            autocomplete: "off", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                                            oninput: move |evt| name.set(evt.value()),
                                            onkeydown: move |evt: KeyboardEvent| { if evt.key() == Key::Enter && !evt.is_composing() { evt.prevent_default(); probe.call(()); } },
                                        }
                                    }
                                    label { "Recipe"
                                        select { class: "add-tensorlake-recipe", value: "{recipe}", disabled: locked,
                                            onchange: move |evt| { if evt.value() == "__new__" { editing.set(true); } else { recipe.set(evt.value()); } },
                                            option { value: "", disabled: true, "Choose a recipe" }
                                            for item in recipes { option { value: "{item.name}", "{item.name}" } }
                                            option { value: "__new__", "New recipe…" }
                                        }
                                    }
                                }
                            },
                        }
                    }
                    if let Some(reason) = error() { p { class: "create-session-error add-host-error", role: "alert", "{crate::peer::display_peer(&reason)}" } }
                }
                if !editing() {
                    crate::modal_dialog::DialogFooter { class: "host-dialog-footer",
                        DialogActions {
                            button { r#type: "button", class: "btn btn-neutral add-host-cancel", onclick: move |_| cancel.call(()), "Cancel" }
                            if let Some(shown) = shown {
                                button { r#type: "button", class: "btn btn-primary provisioning-confirm", disabled: locked, onclick: move |_| confirm.call(shown.clone()), "Create sandbox" }
                            } else {
                                button { r#type: "button", class: "btn btn-primary add-host-submit", disabled: locked || recipe.read().is_empty() || name.read().trim().is_empty(), onclick: move |_| probe.call(()),
                                    if busy() { "Checking…" } else { "Continue" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
