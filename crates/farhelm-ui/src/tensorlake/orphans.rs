//! Settings' explicit cleanup list. No action is inferred from age or tag;
//! each deletion names the sandbox and asks before destroying its files.
use crate::{
    ApiBase,
    api::{delete_orphan_sandbox, fetch_orphan_sandboxes},
    peer::display_peer,
};
use dioxus::prelude::*;
use farhelm_proto::tensorlake::{OrphanCleanup, OrphanSandbox};
use std::{cell::Cell, rc::Rc};

/// Keep a failed refresh visible with Retry. Reads are generation-ordered;
/// a slower pre-delete read cannot restore a row after deletion succeeds.
/// Unfinished cleanup comes from the helm's list, so closing this component
/// discards only the confirmation, not the operation's retry action.
/// A failed discovery stays paused until Refresh, this dialog's key save or
/// clear, a successful saved-key Test, or remount. Another tab's key save is
/// not observed here; ordinary fleet changes must not retry a rejected key.
#[component]
pub(crate) fn OrphanList(api_key_set: bool, key_revision: u64, test_revision: u64) -> Element {
    let base = use_context::<ApiBase>().0;
    let delete_base = base.clone();
    let mut rows = use_signal(Vec::<OrphanSandbox>::new);
    let mut error = use_signal(|| None::<String>);
    // Request/admission failures also need a reason even when the helm has
    // not admitted a cleanup operation. List refresh must not erase it.
    let mut action_error = use_signal(|| None::<String>);
    let mut generation = use_signal(|| 0_u64);
    let mut confirm = use_signal(|| None::<OrphanSandbox>);
    let mut busy = use_signal(|| false);
    let mounted = use_hook(|| Rc::new(Cell::new(true)));
    let drop_mounted = mounted.clone();
    use_drop(move || drop_mounted.set(false));
    let reload = use_callback(move |_: ()| {
        // Clearing the key also invalidates a read already in flight.
        let mine = *generation.peek() + 1;
        generation.set(mine);
        if !api_key_set {
            rows.set(vec![]);
            error.set(None);
            return;
        }
        let base = base.clone();
        spawn(async move {
            let result = fetch_orphan_sandboxes(&base).await;
            if *generation.peek() != mine {
                return;
            }
            match result {
                Ok(list) => {
                    rows.set(list);
                    error.set(None);
                }
                Err(reason) => error.set(Some(reason)),
            }
        });
    });
    // A replacement key can recover a failed read even while key_set stays
    // true. Other section renders and unrelated fleet notices must not keep
    // sending a key discovery has already rejected.
    use_effect(use_reactive((&api_key_set, &key_revision), move |_| {
        reload.call(())
    }));
    // Test establishes that the saved credential works, including after
    // another tab replaced it. Healthy discovery needs no extra read.
    use_effect(use_reactive((&test_revision,), move |_| {
        if error.peek().is_some() {
            reload.call(());
        }
    }));
    let feed_acted_on = crate::feed::use_feed_reader(move || {
        if error.peek().is_none() {
            reload.call(());
        }
    });
    // Observe the consumer itself: another reader's HTTP completion cannot
    // prove this scope has handled a notice. Generation also exposes an
    // admitted reload before its spawned request reaches the network.
    let snapshot_feed = crate::feed::FLEET_FEED.signal();
    crate::reader::use_test_reader_snapshot(move || {
        serde_json::json!({
            "role": "tensorlake-orphans",
            "notices": snapshot_feed.peek().notices.to_string(),
            "acted_on": feed_acted_on.peek().to_string(),
            "readers": [],
            "generation": generation.peek().to_string(),
            "failed": error.peek().is_some(),
        })
    });
    // A manual refresh also discovers resources created outside this helm;
    // the invalidation feed cannot observe another helm or the vendor CLI.
    rsx! {
        if !rows().is_empty() || error().is_some() || confirm().is_some() || action_error().is_some() {
            section { class: "host-settings-section tensorlake-orphans", aria_label: "Sandboxes not on this helm",
                h4 { class: "host-settings-section-title", "Sandboxes not on this helm" }
                p { class: "host-settings-help", "These Farhelm sandboxes have no host or snapshot build on this helm. Nothing is deleted automatically." }
                for sandbox in rows() {
                    div { key: "{sandbox.id}", class: "tensorlake-orphan", "data-sandbox-id": "{sandbox.id}",
                        p { class: "peer-value", dir: "ltr", "{display_peer(&sandbox.name)}" }
                        p { class: "host-settings-help", "{display_peer(&sandbox.state)}"
                            if sandbox.from_another_helm { " · from another helm" }
                            if let Some(created) = sandbox.created_at { " · Created {super::recipes::utc_minute(created / 1000)}" }
                        }
                        if let Some(OrphanCleanup::Failed { reason }) = sandbox.cleanup.as_ref()
                            && confirm().as_ref().is_none_or(|shown| shown.id != sandbox.id) {
                            p { class: "checkout-root-error", role: "alert", "Cleanup unfinished: {display_peer(reason)}" }
                        }
                        button { r#type: "button", class: "btn btn-neutral", disabled: busy() || sandbox.cleanup == Some(OrphanCleanup::Running),
                            onclick: move |_| { action_error.set(None); confirm.set(Some(sandbox.clone())); },
                            match sandbox.cleanup {
                                Some(OrphanCleanup::Running) => "Cleaning up…",
                                Some(OrphanCleanup::Failed { .. }) => "Retry cleanup",
                                None => "Delete",
                            }
                        }
                    }
                }
                if let Some(sandbox) = confirm() {
                    div { class: "templates-departure orphan-delete-confirm", role: "status",
                        p { "Delete sandbox {display_peer(&sandbox.name)}? Its files, sessions and snapshots will be deleted." }
                        crate::modal_dialog::DialogActions {
                            button { r#type: "button", class: "btn btn-neutral", disabled: busy(), onclick: move |_| { confirm.set(None); action_error.set(None); }, "Cancel" }
                            button { r#type: "button", class: "btn btn-danger", disabled: busy(), onclick: move |_| {
                                if *busy.peek() { return; }
                                busy.set(true);
                                action_error.set(None);
                                let deleting = delete_orphan_sandbox(&delete_base, &sandbox.id);
                                let mounted = mounted.clone();
                                dioxus::core::spawn_forever(async move {
                                    let result = deleting.await;
                                    if !mounted.get() { return; }
                                    busy.set(false);
                                    match result { Ok(()) => { confirm.set(None); reload.call(()); }, Err(reason) => action_error.set(Some(reason)) }
                                });
                            }, "Delete sandbox" }
                        }
                    }
                }
                if let Some(reason) = error() { p { class: "checkout-root-error", role: "alert", "{display_peer(&reason)}" } }
                if let Some(reason) = action_error() { p { class: "checkout-root-error", role: "alert", "{display_peer(&reason)}" } }
            }
        }
        if api_key_set {
            button { r#type: "button", class: "btn btn-neutral", disabled: busy(), onclick: move |_| reload.call(()), "Refresh sandboxes" }
        }
    }
}
