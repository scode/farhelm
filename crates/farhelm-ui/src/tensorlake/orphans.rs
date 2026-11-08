//! Settings' explicit cleanup list. No action is inferred from age or tag;
//! each deletion names the sandbox and asks before destroying its files.
use crate::{
    ApiBase,
    api::{delete_orphan_sandbox, fetch_orphan_sandboxes},
    ops::{ConfirmSlot, ConfirmToken, use_confirm_slot},
    peer::display_peer,
};
use dioxus::prelude::*;
use farhelm_proto::tensorlake::{OrphanCleanup, OrphanSandbox};
use std::{cell::Cell, rc::Rc};

/// Consume only the opening the answer displayed, before constructing a
/// destructive request. A queued answer after Cancel must authorize nothing.
fn delete_for_offer<T>(
    slot: &mut ConfirmSlot<OrphanSandbox>,
    shown: &ConfirmToken<OrphanSandbox>,
    busy: bool,
    deleting: impl FnOnce(&str) -> T,
) -> Option<T> {
    if busy {
        return None;
    }
    slot.take(shown)?;
    Some(deleting(&shown.key().id))
}

/// Record a completed check in the viewer's local time on either renderer.
///
/// Using the webview's Date keeps both browser and desktop labels in their
/// viewer's timezone without a timezone dependency or timer. A failed bridge
/// must not invent a time or turn successful discovery into an API failure.
async fn checked_time() -> String {
    document::eval(
        "const now = new Date(); return String(now.getHours()).padStart(2, '0') + ':' + String(now.getMinutes()).padStart(2, '0');",
    )
    .join::<String>()
    .await
    .unwrap_or_else(|_| "time unavailable".into())
}

/// Keep discovery feedback visible beside explicit cleanup. Reads are generation-ordered;
/// a slower pre-delete read cannot restore a row after deletion succeeds.
/// Unfinished cleanup comes from the helm's list, so closing this component
/// discards only the confirmation, not the operation's retry action.
/// A failed discovery stays paused until Refresh, this dialog's key save or
/// clear, a successful saved-key Test, or remount. Another tab's key save is
/// not observed here; ordinary fleet changes must not retry a rejected key.
/// A saved key keeps the section visible even when discovery finds nothing;
/// its pending state and last successful check belong only to this viewer.
#[component]
pub(crate) fn OrphanList(api_key_set: bool, key_revision: u64, test_revision: u64) -> Element {
    let base = use_context::<ApiBase>().0;
    let delete_base = base.clone();
    let mut rows = use_signal(Vec::<OrphanSandbox>::new);
    let mut error = use_signal(|| None::<String>);
    let mut checking = use_signal(|| false);
    let mut checked_at = use_signal(|| None::<String>);
    // Request/admission failures also need a reason even when the helm has
    // not admitted a cleanup operation. List refresh must not erase it.
    let mut action_error = use_signal(|| None::<String>);
    let mut generation = use_signal(|| 0_u64);
    let mut confirm = use_confirm_slot::<OrphanSandbox, ()>();
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
            checking.set(false);
            checked_at.set(None);
            return;
        }
        // Keep an earlier failure until a successful reply: it also pauses
        // feed-triggered reads while this explicit recovery is in flight.
        checking.set(true);
        let base = base.clone();
        spawn(async move {
            let result = fetch_orphan_sandboxes(&base).await;
            if *generation.peek() != mine {
                return;
            }
            match result {
                Ok(list) => {
                    let checked = checked_time().await;
                    // The key may have been cleared or a newer read admitted
                    // while the webview supplied this completion's timestamp.
                    if *generation.peek() != mine {
                        return;
                    }
                    rows.set(list);
                    error.set(None);
                    checked_at.set(Some(checked));
                }
                Err(reason) => error.set(Some(reason)),
            }
            checking.set(false);
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
        if api_key_set || !rows().is_empty() || error().is_some() || confirm.is_open() || action_error().is_some() {
            section { class: "host-settings-section tensorlake-orphans", aria_label: "Sandboxes not on this helm",
                div { class: "tensorlake-orphans-heading",
                    h4 { class: "host-settings-section-title", "Sandboxes not on this helm" }
                    if api_key_set {
                        button { r#type: "button", class: "btn btn-neutral tensorlake-orphans-refresh", disabled: busy() || checking(),
                            "data-tooltip": "Refresh: check Tensorlake again for sandboxes not on this helm",
                            onclick: move |_| reload.call(()), "Refresh" }
                    }
                }
                p { class: "host-settings-help", "These Farhelm sandboxes have no host or snapshot build on this helm. Nothing is deleted automatically." }
                if checking() {
                    p { class: "host-settings-help tensorlake-orphans-status", role: "status", "Checking…" }
                } else if rows().is_empty() && error().is_none() {
                    if let Some(checked) = checked_at() {
                        p { class: "host-settings-help tensorlake-orphans-status", role: "status", "None found · checked {checked}" }
                    }
                }
                for sandbox in rows() {
                    div { key: "{sandbox.id}", class: "tensorlake-orphan", "data-sandbox-id": "{sandbox.id}",
                        p { class: "peer-value", dir: "ltr", "{display_peer(&sandbox.name)}" }
                        p { class: "host-settings-help", "{display_peer(&sandbox.state)}"
                            if sandbox.from_another_helm { " · from another helm" }
                            if let Some(created) = sandbox.created_at { " · Created {super::recipes::utc_minute(created / 1000)}" }
                        }
                        if let Some(OrphanCleanup::Failed { reason }) = sandbox.cleanup.as_ref()
                            && confirm.current().is_none_or(|shown| shown.key().id != sandbox.id) {
                            p { class: "checkout-root-error", role: "alert", "Cleanup unfinished: {display_peer(reason)}" }
                        }
                        button { r#type: "button", class: "btn btn-neutral", disabled: busy() || sandbox.cleanup == Some(OrphanCleanup::Running),
                            onclick: move |_| { action_error.set(None); confirm.open(sandbox.clone(), ()); },
                            match sandbox.cleanup {
                                Some(OrphanCleanup::Running) => "Cleaning up…",
                                Some(OrphanCleanup::Failed { .. }) => "Retry cleanup",
                                None => "Delete",
                            }
                        }
                    }
                }
                if let Some((shown, cancel_shown)) = confirm.current().map(|shown| (shown.clone(), shown)) {
                    div { class: "templates-departure orphan-delete-confirm", role: "status",
                        p { "Delete sandbox {display_peer(&shown.key().name)}? Its files, sessions and snapshots will be deleted." }
                        crate::modal_dialog::DialogActions {
                            button { r#type: "button", class: "btn btn-neutral", disabled: busy(), onclick: move |_| { if confirm.is_current(&cancel_shown) { confirm.cancel_for(&cancel_shown); action_error.set(None); } }, "Cancel" }
                            button { r#type: "button", class: "btn btn-danger", disabled: busy(), onclick: move |_| {
                                let Some(deleting) = delete_for_offer(&mut confirm, &shown, *busy.peek(), |id| delete_orphan_sandbox(&delete_base, id)) else { return; };
                                busy.set(true);
                                action_error.set(None);
                                let mounted = mounted.clone();
                                dioxus::core::spawn_forever(async move {
                                    let result = deleting.await;
                                    if !mounted.get() { return; }
                                    busy.set(false);
                                    match result { Ok(()) => reload.call(()), Err(reason) => action_error.set(Some(reason)) }
                                });
                            }, "Delete sandbox" }
                        }
                    }
                }
                if let Some(reason) = error() { p { class: "checkout-root-error", role: "alert", "{display_peer(&reason)}" } }
                if let Some(reason) = action_error() { p { class: "checkout-root-error", role: "alert", "{display_peer(&reason)}" } }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cancel and a queued Delete share one render: cancellation must prevent
    /// request construction. Reopening cannot revive that answer, and a fresh
    /// answer still constructs exactly one request rather than disabling Delete.
    #[farhelm_testtrace::test]
    fn cancelled_and_stale_delete_answers_send_nothing() {
        std::thread_local! {
            static SLOT: Cell<Option<ConfirmSlot<OrphanSandbox>>> = const { Cell::new(None) };
        }
        /// Own a real hook slot while the test delivers answers without a render.
        fn app() -> Element {
            let slot = use_confirm_slot::<OrphanSandbox, ()>();
            SLOT.with(|cell| cell.set(Some(slot)));
            rsx! {}
        }
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        dom.in_runtime(|| {
            let mut slot = SLOT.with(|cell| cell.get().unwrap());
            let sandbox = OrphanSandbox {
                id: "sandbox-id".into(),
                name: "sandbox".into(),
                created_at: None,
                state: "running".into(),
                from_another_helm: false,
                cleanup: None,
            };
            let cancelled = slot.open(sandbox.clone(), ());
            let requests = std::cell::RefCell::new(Vec::new());
            let deleting = |id: &str| requests.borrow_mut().push(id.to_string());
            assert!(
                slot.is_current(&cancelled),
                "the question is open before Cancel"
            );
            slot.cancel_for(&cancelled);
            assert!(delete_for_offer(&mut slot, &cancelled, false, deleting).is_none());
            let current = slot.open(sandbox, ());
            assert!(delete_for_offer(&mut slot, &cancelled, false, deleting).is_none());
            assert!(
                slot.is_current(&current),
                "a stale answer leaves the new question open"
            );
            assert!(delete_for_offer(&mut slot, &current, true, deleting).is_none());
            assert!(
                slot.is_current(&current),
                "busy refuses without consuming the question"
            );
            assert!(
                requests.borrow().is_empty(),
                "no refused answer constructs a request"
            );
            assert!(delete_for_offer(&mut slot, &current, false, deleting).is_some());
            assert!(delete_for_offer(&mut slot, &current, false, deleting).is_none());
            assert_eq!(*requests.borrow(), ["sandbox-id"]);
        });
    }
}
