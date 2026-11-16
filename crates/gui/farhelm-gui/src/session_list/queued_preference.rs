//! A best-effort preference the session list writes on the user's choice: the list's order, Compact and the host rail's
//! expansion. Each is one field of the helm's shared preference row, written as a sparse `PUT /api/preferences` through
//! a [`Draft`] under the row's key ([`OpKey::preferences`]).
//!
//! **A choice is never dropped.** The row's key admits one request at a time, shared with Settings, the Templates
//! dialog and every other preference save. A choice made while it is busy is queued here, shown at once, and sent once
//! the key frees; the newest choice per field wins, and fields queued together go out one after the other. This ports
//! the old app's per-field latest-wins writer (`api.rs`'s `PreferenceWrites` at reference commit `24bc0580`), which
//! ran fields concurrently. The migration's orchestrator decided on 2026-11-16 that these choices queue rather than
//! drop, after the milestone M2 parity review held the drop against SPEC.md's "An action the user started is still
//! never lost silently" ("Losing the connection to the helm").
//!
//! The queue merges choices before the operation store admits them, so it changes no store rule: the store still admits
//! one request per key and supersedes nothing in flight, and Settings keeps its accepted refusal of a press while a
//! preference save is out (`crate::settings`' "Quick flips" NOTE). `docs/gpui/parity/sidebar-and-session-view.md`
//! records the two rules side by side as a deviation.
//!
//! **What the field shows**: the queued choice, else the one in flight, else what the helm stores. A refused write is
//! logged and shows nothing, so the field goes back to the stored value at once (SPEC.md, "Errors and diagnostics");
//! it is never resent (SPEC.md, "One GUI at a time"), but a choice queued behind it is still sent, as the old writer
//! sent a newer choice whatever the earlier one's outcome.
//!
//! **When the queue sends.** Each field observes the current UI session's operation store from its first choice on, so
//! another feature's write settling frees the key for it. GPUI runs that observer when the store's update flushes,
//! after the completion has settled the field's own draft; the field's own completion also tries to send, so a choice
//! queued behind its own write does not depend on that ordering. A reset retires the store, and a choice queued under
//! it is never sent, as the reset drops every pending save (SPEC_impl.md, "One failure path: the reset").

use std::fmt::Debug;

use farhelm_helm_client::PathError;
use gpui::{Context, EntityId, Subscription};

use crate::drafts::{self, Draft};
use crate::operations::{Command, OpKey, Outcome};
use crate::session;

/// How a view reaches one of its queued preferences: a plain function, so the observer and the save's completion can
/// reach the same field again later.
pub(crate) type Field<V, T> = fn(&mut V) -> &mut QueuedPreference<T>;

/// The sparse write of one choice of a field.
pub(crate) type Write<T> = fn(&T) -> Result<Command, PathError>;

/// One preference field's writes: the one in flight, the newest choice waiting for the preference row's key, and the
/// subscription that sends it (the module docs).
pub(crate) struct QueuedPreference<T> {
    /// The choice being saved while its write is in flight; its value means nothing at any other moment.
    draft: Draft<T>,
    /// The newest choice not sent yet. Replaced, never appended to: the latest choice wins.
    queued: Option<T>,
    /// The operation store observed, by entity, and the subscription to it. Made on the first choice under a UI
    /// session, so a field nobody touches observes nothing.
    watching: Option<(EntityId, Subscription)>,
}

impl<T: Clone + PartialEq + Debug + 'static> QueuedPreference<T> {
    /// Nothing chosen yet. `value` only seeds the draft, which is read only while a write is in flight.
    pub(crate) fn new(value: T) -> Self {
        QueuedPreference {
            draft: Draft::load(value),
            queued: None,
            watching: None,
        }
    }

    /// The value the field shows, `stored` being the helm's: the queued choice, else the one in flight, else the
    /// stored one.
    pub(crate) fn shown(&self, stored: T) -> T {
        match &self.queued {
            Some(queued) => queued.clone(),
            None if self.draft.is_saving() => self.draft.value().clone(),
            None => stored,
        }
    }

    /// The user chose `choice` for the field `field` reaches in `view`: queue it in place of any earlier choice not
    /// sent yet, and send it now if the preference row's key is free. `what` names the preference in the log. Nothing
    /// happens while the window is inert.
    pub(crate) fn choose<V: 'static>(
        view: &mut V,
        field: Field<V, T>,
        choice: T,
        write: Write<T>,
        what: &'static str,
        cx: &mut Context<V>,
    ) {
        let Some(operations) = session::current_operations(cx) else {
            return;
        };
        let queue = field(view);
        queue.queued = Some(choice);
        if queue.watching.as_ref().map(|(id, _)| *id) != Some(operations.entity_id()) {
            let subscription = cx.observe(&operations, move |view, operations, cx| {
                // Only the current store's key: a retired store (a reset) sends nothing more.
                if session::current_operations(cx).is_some_and(|current| current == operations) {
                    send(view, field, write, what, cx);
                }
            });
            field(view).watching = Some((operations.entity_id(), subscription));
        }
        send(view, field, write, what, cx);
        cx.notify();
    }
}

/// Send the field's queued choice if there is one and the preference row's key is free: neither the field's own write
/// nor anyone else's is in flight. Otherwise the choice waits for the next change of the operation store, or for the
/// field's own write to settle.
fn send<V: 'static, T: Clone + PartialEq + Debug + 'static>(
    view: &mut V,
    field: Field<V, T>,
    write: Write<T>,
    what: &'static str,
    cx: &mut Context<V>,
) {
    let busy = session::current_operations(cx).is_none_or(|operations| {
        operations
            .read(cx)
            .in_flight(&OpKey::preferences())
            .is_some()
    });
    let queue = field(view);
    if busy || queue.draft.is_saving() {
        return;
    }
    let Some(choice) = queue.queued.take() else {
        return;
    };
    // The draft is not saving, so it takes the edit. Were that ever untrue, the choice goes back in the queue rather
    // than being dropped, and the next change of the store or the field's own completion sends it.
    if !queue.draft.edit(choice.clone()) {
        tracing::warn!(
            ?choice,
            "{what} stays queued: its idle draft refused the choice"
        );
        queue.queued = Some(choice);
        return;
    }
    let started = drafts::save(
        view,
        move |view: &mut V| &mut field(view).draft,
        &OpKey::preferences(),
        write,
        cx,
        move |view, settled, cx| {
            if let Outcome::Refused(refusal) = &settled.outcome {
                let words = &refusal.words;
                tracing::warn!(?choice, ?words, "the helm refused {what}");
            }
            // A choice queued behind this write goes out now, whatever the order the store's observers run in (the
            // module docs, "When the queue sends").
            send(view, field, write, what, cx);
        },
    );
    if let Err(not_started) = started {
        tracing::debug!(?not_started, "{what} was not saved");
    }
}
