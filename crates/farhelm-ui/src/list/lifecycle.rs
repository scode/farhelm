//! Batch lifecycle work keeps the ordinary row admission and completion owners.
//!
//! Admission happens before these futures are polled. Keeping queued rows admitted
//! holds navigation and automatic selection until all hosts finish, while polling
//! one core at a time per host avoids the supervisor's management admission limit.
//! Delete consent is a separate snapshot: reconciliation can shrink its identities
//! and replace its wording, but an answer can never discover additional targets.

use std::future::{Future, poll_fn};
use std::pin::Pin;
use std::task::Poll;

use dioxus::prelude::*;

use crate::modal_dialog::{
    DialogActions, DialogFocus, DialogFooter, DialogHandle, DialogHeader, ModalDialog,
    RecoveryDelay, RecoveryFocus, ScrollRegion,
};
use crate::ops::ConfirmToken;
use crate::peer::PeerText;
use crate::status::{StatusBadgeView, confirm_consequence, status_badge};
use crate::{HostId, Session, SessionStatus};

use super::row::PeerTitle;
use super::shared::{DeleteTarget, HostLocality, SessionHostName, session_locality};

/// An admitted row's entire completion, including its ordinary read/bookkeeping.
/// Constructing it must not send the request: the host queue controls first poll.
pub(super) type RowWork = Pin<Box<dyn Future<Output = ()>>>;

/// Only these lifecycle actions have no single-session follow-up to resolve.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum LifecycleAction {
    Stop,
    Delete,
}

impl LifecycleAction {
    /// Share the ordinary row error prefix for a batch admission refusal.
    pub(super) fn not_started(self) -> String {
        let label = match self {
            Self::Stop => "Stop",
            Self::Delete => "Delete",
        };
        format!("{label}: not started, another operation is in progress; try again")
    }
}

/// Reserve every accepted row before the host runner can poll its first request.
/// Refusals are reported separately, so one engaged row cannot silently vanish
/// or prevent the other hosts' eligible work from being admitted.
pub(super) fn admit_batch(
    targets: Vec<DeleteSnapshot>,
    mut admit: impl FnMut(String, crate::DeleteGuard) -> Option<RowWork>,
    mut refused: impl FnMut(String),
) -> Vec<(Option<HostId>, RowWork)> {
    let mut work = Vec::new();
    for target in targets {
        let id = target.id.raw().to_owned();
        if let Some(accepted) = admit(id.clone(), target.guard()) {
            work.push((target.host, accepted));
        } else {
            refused(id);
        }
    }
    work
}

/// Drive all hosts together, but poll only one admitted row core on each host.
///
/// All queued futures already own row claims. Finishing a fast host cannot unlock
/// navigation while another host still has a held request or queued rows. Errors
/// are handled by each core, so one refusal never prevents the next row running.
pub(super) async fn run_paced(work: Vec<(Option<HostId>, RowWork)>) {
    let mut hosts: Vec<(Option<HostId>, Vec<RowWork>)> = Vec::new();
    for (host, work) in work {
        if let Some((_, queue)) = hosts.iter_mut().find(|(key, _)| *key == host) {
            queue.push(work);
        } else {
            hosts.push((host, vec![work]));
        }
    }
    let mut queues: Vec<RowWork> = hosts
        .into_iter()
        .map(|(_, queue)| -> RowWork {
            Box::pin(async move {
                for work in queue {
                    work.await;
                }
            })
        })
        .collect();
    poll_fn(move |cx| {
        queues.retain_mut(|work| work.as_mut().poll(cx).is_pending());
        if queues.is_empty() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    })
    .await;
}

/// Everything this modal draws, excluding unrelated activity and notification data.
/// Equality means the same question; a changed snapshot needs a new opening token.
#[derive(Clone, PartialEq)]
pub(super) struct DeleteSnapshot {
    pub(super) id: PeerText,
    pub(super) host: Option<HostId>,
    title: PeerText,
    host_name: Option<PeerText>,
    locality: HostLocality,
    status: SessionStatus,
    annotation: Option<PeerText>,
    tabs: usize,
    managed: bool,
}

impl DeleteSnapshot {
    /// Freeze the identity and consequences the user can actually inspect.
    pub(super) fn for_session(session: &Session, local_host: Option<HostId>) -> Self {
        Self {
            id: session.id.clone(),
            host: session.host,
            title: session.title.clone(),
            host_name: session.host_name.clone(),
            locality: session_locality(session.host, local_host),
            status: session.status.clone(),
            annotation: session.annotation.clone(),
            tabs: session.tabs.len(),
            managed: session.working_copy.is_some(),
        }
    }

    /// Every request carries this snapshot's guard, even after waiting in a queue.
    pub(super) fn guard(&self) -> crate::DeleteGuard {
        crate::status::delete_guard(&self.status, self.tabs)
    }

    /// Preserve the single-row confirmation predicate for mixed batches.
    fn needs_confirmation(&self) -> bool {
        DeleteTarget {
            id: self.id.raw().to_owned(),
            status: self.status.clone(),
            tabs: self.tabs,
        }
        .needs_confirmation()
    }
}

/// One row needing consent is sufficient to show the whole accepted selection.
pub(super) fn needs_confirmation(targets: &[DeleteSnapshot], skip: bool) -> bool {
    !skip && targets.iter().any(DeleteSnapshot::needs_confirmation)
}

/// Reword known targets and drop only proven departures; never grow an opening.
///
/// Call only for admitted successful listings. A filtered, partial or operation's
/// non-authoritative read may update present facts but its omissions prove nothing.
/// Removing a target here is permanent for this question, even if it later returns.
pub(super) fn reconcile(
    drawn: &[DeleteSnapshot],
    listed: &[Session],
    absence_is_evidence: bool,
    local_host: Option<HostId>,
) -> Vec<DeleteSnapshot> {
    drawn
        .iter()
        .filter_map(
            |target| match listed.iter().find(|session| session.id == target.id) {
                Some(session) => Some(DeleteSnapshot::for_session(session, local_host)),
                None if absence_is_evidence => None,
                None => Some(target.clone()),
            },
        )
        .collect()
}

/// A queued answer can lose hidden or unmarked targets, but keeps drawn guards.
/// This is admission, not disappearance evidence: omitted snapshots remain in an
/// unanswered question until authoritative reconciliation says they are gone.
pub(super) fn admitted_targets(
    drawn: Vec<DeleteSnapshot>,
    displayed: &[Session],
    marked: &std::collections::HashSet<String>,
) -> Vec<DeleteSnapshot> {
    drawn
        .into_iter()
        .filter(|target| {
            marked.contains(target.id.raw()) && displayed.iter().any(|row| row.id == target.id)
        })
        .collect()
}

/// The persistent list addresses the same installed modal on every close path.
pub(super) const DIALOG: &str = ".bulk-delete-dialog";
/// The count remains focusable after confirmation disables every action button.
pub(super) const ACCEPTED_FOCUS: &str = "(document.querySelector('.marked-count') ?? document.querySelector('.sort-title'))?.focus({preventScroll:true});";
/// Cancellation retains marks and can return to their ordinary enabled action.
pub(super) const CANCEL_FOCUS: &str = "(document.querySelector('.marked-menu-toggle') ?? document.querySelector('.sort-title'))?.focus({preventScroll:true});";

/// Draw one structured question; all answers carry the opening this render drew.
/// The persistent list owns the slot and dispatch, so hiding marks cannot unmount
/// this question or cancel accepted work.
#[component]
pub(super) fn DeleteDialog(
    shown: ConfirmToken<Vec<DeleteSnapshot>>,
    disabled: bool,
    on_confirm: EventHandler<ConfirmToken<Vec<DeleteSnapshot>>>,
    on_cancel: EventHandler<ConfirmToken<Vec<DeleteSnapshot>>>,
) -> Element {
    let targets = shown.key().clone();
    let count = targets.len();
    let noun = if count == 1 { "session" } else { "sessions" };
    let cancel = shown.clone();
    let escape = shown.clone();
    let close = shown.clone();
    rsx! {
        div { class: "host-settings-backdrop", role: "presentation",
            ModalDialog {
                class: "bulk-delete-dialog host-dialog-framed", role: "dialog",
                aria_modal: "true", aria_labelledby: "bulk-delete-title", tabindex: "-1",
                handle: DialogHandle::new(DIALOG),
                focus: DialogFocus::remembered(
                    if disabled { ".bulk-delete-cancel" } else { ".bulk-delete-confirm" },
                    Some(".bulk-delete-cancel"), false, Some(".bulk-delete-confirm")),
                on_escape: move |event: KeyboardEvent| {
                    if !event.is_composing() {
                        event.prevent_default();
                        event.stop_propagation();
                        on_cancel.call(escape.clone());
                    }
                },
                DialogHeader {
                    title: rsx! { span { id: "bulk-delete-title", "Delete {count} {noun}?" } },
                    close: rsx! {
                        button {
                            r#type: "button", class: "dialog-close", aria_label: "Cancel bulk deletion",
                            "data-tooltip": "Cancel: keep the marked sessions",
                            onclick: move |_| on_cancel.call(close.clone()), "×"
                        }
                    },
                }
                ScrollRegion {
                    class: "host-settings-body bulk-delete-body", handle: DialogHandle::new(DIALOG),
                    selector: ".bulk-delete-body", name: "Sessions to delete",
                    recovery: RecoveryFocus::new(".bulk-delete-cancel", None, true, RecoveryDelay::OneFrame),
                    for target in &targets {
                        div { class: "bulk-delete-row", "data-delete-target": target.id.raw(),
                            div { class: "bulk-delete-identity",
                                if let Some(badge) = status_badge(&target.status, target.annotation.as_ref().map(PeerText::raw), None) {
                                    StatusBadgeView { badge, dot_onclick: |_| {}, dot_title: None }
                                } else {
                                    // Rows omit unknown badges; this structured
                                    // question still names the unresolved state.
                                    span { class: "status-badge unknown", "Unknown" }
                                }
                                PeerTitle { class: "bulk-delete-title", title: target.title.clone(), quoted: false }
                            }
                            div { class: "bulk-delete-host",
                                if let Some(name) = &target.host_name {
                                    SessionHostName { name: name.clone(), locality: target.locality }
                                } else { "Host identity unavailable" }
                            }
                            p { class: "bulk-delete-consequence", "{confirm_consequence(&target.status, target.tabs)}" }
                            if target.managed {
                                p { class: "bulk-delete-consequence",
                                    "The managed checkout stays while another session uses it. When its last session is deleted, its folder moves to the trash; no files are deleted."
                                }
                            }
                        }
                    }
                }
                DialogFooter {
                    DialogActions {
                        button { r#type: "button", class: "btn btn-neutral bulk-delete-cancel",
                            "data-tooltip": "Cancel: keep the marked sessions",
                            onclick: move |_| on_cancel.call(cancel.clone()), "Cancel" }
                        button { r#type: "button", class: "btn btn-danger bulk-delete-confirm", disabled,
                            "data-tooltip": "Delete the sessions listed in this question",
                            onkeydown: crate::guard_confirmation_enter,
                            onclick: move |_| on_confirm.call(shown.clone()), "Delete {noun}" }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::collections::HashSet;
    use std::rc::Rc;
    use std::task::{Context, Waker};

    use futures_channel::oneshot;

    use super::*;
    use crate::list::row::row_specimen;
    use crate::list::view::{begin_row_operation, end_row_operation};
    use crate::ops::{ConfirmSlot, OpLock, PaneGate, use_confirm_slot, use_op_lock};

    /// Real hooks let the pacing tests inspect the same live row count that
    /// excludes cross-pane operations and automatic selection in ListView.
    #[derive(Clone, Copy)]
    struct State {
        ops: OpLock,
        pending: Signal<HashSet<String>>,
        count: Signal<u32>,
        question: ConfirmSlot<Vec<DeleteSnapshot>>,
    }

    thread_local! {
        static STATE: Cell<Option<State>> = const { Cell::new(None) };
    }

    /// Keep every handle alive in a mounted list-like owner for a whole test.
    fn owner() -> Element {
        let state = State {
            ops: use_op_lock(),
            pending: use_signal(HashSet::new),
            count: use_signal(|| 0),
            question: use_confirm_slot(),
        };
        STATE.with(|cell| cell.set(Some(state)));
        rsx! {}
    }

    /// Supply full host-qualified references so lanes cannot pass by name alone.
    fn session(id: &str, host: HostId) -> Session {
        let mut row = row_specimen(id);
        row.host = Some(host);
        row.host_name = Some(format!("Host {host} (destination.example)").into());
        row
    }

    /// Snapshot fixtures through the exact constructor used by toolbar clicks.
    fn snapshots(rows: &[Session]) -> Vec<DeleteSnapshot> {
        rows.iter()
            .map(|row| DeleteSnapshot::for_session(row, Some(1)))
            .collect()
    }

    /// First poll is the observable request-dispatch boundary, not construction.
    fn poll(work: Pin<&mut impl Future<Output = ()>>) -> Poll<()> {
        work.poll(&mut Context::from_waker(Waker::noop()))
    }

    /// A held first request on host 1 must not start host 1's second request,
    /// while host 2 must start before host 1 is released. Queued claims keep the
    /// actual pane gate locked when either host finishes earlier than the other.
    #[farhelm_testtrace::test]
    fn admission_is_upfront_and_requests_are_sequential_per_host_parallel_across_hosts() {
        let mut dom = VirtualDom::new(owner);
        dom.rebuild_in_place();
        dom.in_runtime(|| {
            let state = STATE.with(|cell| cell.get().unwrap());
            let mut gate = PaneGate::new(state.ops, state.count);
            let rows: Vec<_> = (1..=9)
                .map(|n| session(&format!("a{n}"), 1))
                .chain([session("b1", 2)])
                .collect();
            let dispatched = Rc::new(RefCell::new(Vec::new()));
            let mut releases = Vec::new();
            let work = admit_batch(
                snapshots(&rows),
                |id, _guard| {
                    assert!(begin_row_operation(
                        state.ops,
                        state.pending,
                        state.count,
                        false,
                        &id
                    ));
                    let (tx, rx) = oneshot::channel();
                    releases.push(tx);
                    let dispatched = dispatched.clone();
                    Some(Box::pin(async move {
                        dispatched.borrow_mut().push(id.clone());
                        rx.await.expect("the test owns this request's completion");
                        end_row_operation(state.pending, state.count, &id);
                    }))
                },
                |id| panic!("unexpected refusal for {id}"),
            );
            assert_eq!(
                *state.count.peek(),
                10,
                "all rows hold claims before first poll"
            );
            assert!(dispatched.borrow().is_empty(), "admission sends nothing");
            assert!(gate.claim_guard().is_none());
            let mut run = Box::pin(run_paced(work));
            assert!(poll(run.as_mut()).is_pending());
            assert_eq!(*dispatched.borrow(), ["a1", "b1"]);

            // Finish the other host first. All nine rows on host 1 retain
            // claims, although only its first request has been dispatched.
            releases.pop().unwrap().send(()).unwrap();
            assert!(poll(run.as_mut()).is_pending());
            assert_eq!(*state.count.peek(), 9);
            assert!(gate.busy_now());
            assert!(gate.claim_guard().is_none());
            for finished in 1..=9 {
                releases.remove(0).send(()).unwrap();
                let result = poll(run.as_mut());
                assert_eq!(*state.count.peek(), 9 - finished, "no unlocked request gap");
                if finished < 9 {
                    assert!(result.is_pending());
                    assert!(gate.busy_now());
                    assert!(gate.claim_guard().is_none());
                    assert_eq!(
                        dispatched.borrow().last().unwrap(),
                        &format!("a{}", finished + 1)
                    );
                    assert_eq!(dispatched.borrow().len(), finished as usize + 2);
                } else {
                    assert!(result.is_ready());
                }
            }
            assert_eq!(*state.count.peek(), 0);
            assert!(state.pending.peek().is_empty());
            assert!(gate.claim_guard().is_some());
        });
    }

    /// Each refused row gets its own ordinary error. No request is started for
    /// that row, and a held post-request completion still paces the next row.
    #[farhelm_testtrace::test]
    fn refusals_are_row_local_and_completion_includes_post_request_work() {
        let mut dom = VirtualDom::new(owner);
        dom.rebuild_in_place();
        dom.in_runtime(|| {
            let state = STATE.with(|cell| cell.get().unwrap());
            let rows = [
                session("question", 1),
                session("first", 1),
                session("next", 1),
            ];
            let errors = RefCell::new(Vec::new());
            let starts = Rc::new(RefCell::new(Vec::new()));
            let (request_done, request) = oneshot::channel();
            let (read_done, post_request_read) = oneshot::channel();
            let mut held = Some((request, post_request_read));
            let work = admit_batch(
                snapshots(&rows),
                |id, _| {
                    let asking = id == "question";
                    if !begin_row_operation(state.ops, state.pending, state.count, asking, &id) {
                        return None;
                    }
                    let reads = if id == "first" { held.take() } else { None };
                    let starts = starts.clone();
                    Some(Box::pin(async move {
                        starts.borrow_mut().push(id.clone());
                        if let Some((request, post_request_read)) = reads {
                            request.await.unwrap();
                            post_request_read.await.unwrap();
                        }
                        end_row_operation(state.pending, state.count, &id);
                    }))
                },
                |id| {
                    errors
                        .borrow_mut()
                        .push((id, LifecycleAction::Stop.not_started()))
                },
            );
            assert_eq!(
                *errors.borrow(),
                [(
                    "question".to_string(),
                    "Stop: not started, another operation is in progress; try again".to_string()
                )]
            );
            assert!(!state.pending.peek().contains("question"));
            let mut run = Box::pin(run_paced(work));
            assert!(poll(run.as_mut()).is_pending());
            request_done.send(()).unwrap();
            assert!(poll(run.as_mut()).is_pending());
            assert_eq!(
                *starts.borrow(),
                ["first"],
                "a reply alone is not core completion"
            );
            assert_eq!(*state.count.peek(), 2);
            read_done.send(()).unwrap();
            assert!(poll(run.as_mut()).is_ready());
            assert_eq!(*starts.borrow(), ["first", "next"]);
            assert_eq!(*state.count.peek(), 0);
            assert_eq!(
                LifecycleAction::Delete.not_started(),
                "Delete: not started, another operation is in progress; try again"
            );
        });
    }

    /// Ending without tabs deletes directly; a mixed live/tabbed batch asks
    /// once. Skipping questions retains the guards each drawn row authorized.
    #[farhelm_testtrace::test]
    fn confirmation_and_skipping_follow_the_drawn_single_row_contract() {
        let mut ended = session("ended", 1);
        ended.status = SessionStatus::Exited { exit_code: Some(0) };
        let mut tabbed = DeleteSnapshot::for_session(&ended, Some(1));
        tabbed.tabs = 2;
        let mut live = session("live", 2);
        live.status = SessionStatus::Running;
        let bare = snapshots(&[ended]);
        assert!(!needs_confirmation(&bare, false));
        assert_eq!(bare[0].guard(), crate::DeleteGuard::NothingAlive);
        assert_eq!(tabbed.guard(), crate::DeleteGuard::AgentEnded);
        assert!(needs_confirmation(&[tabbed], false));
        let mut mixed = bare;
        mixed.extend(snapshots(&[live]));
        assert!(needs_confirmation(&mixed, false));
        assert!(!needs_confirmation(&mixed, true));
        assert_eq!(mixed[0].guard(), crate::DeleteGuard::NothingAlive);
        assert_eq!(mixed[1].guard(), crate::DeleteGuard::Unconditional);
    }

    /// Failed/filtered omission cannot discard consent targets, and changed
    /// wording must retire queued old answers before any request is admitted.
    #[farhelm_testtrace::test]
    fn reconciliation_replaces_changed_openings_without_growing_or_strengthening_old_answers() {
        let mut dom = VirtualDom::new(owner);
        dom.rebuild_in_place();
        dom.in_runtime(|| {
            let mut question = STATE.with(|cell| cell.get().unwrap()).question;
            let mut old = session("old", 1);
            old.status = SessionStatus::Exited { exit_code: Some(0) };
            let retained = session("retained", 2);
            let drawn = snapshots(&[old.clone(), retained.clone()]);
            let first = question.open(drawn.clone(), ());
            let unchanged = reconcile(&drawn, &[], false, Some(1));
            assert!(unchanged == drawn);
            let mut irrelevant = old.clone();
            irrelevant.last_activity_at += 1;
            assert!(reconcile(&drawn, &[irrelevant], false, Some(1)) == drawn);
            old.status = SessionStatus::Running;
            let extra = session("never-drawn", 1);
            let replaced = reconcile(&drawn, &[old.clone(), extra.clone()], false, Some(1));
            assert_eq!(replaced.len(), 2);
            assert_eq!(replaced[0].guard(), crate::DeleteGuard::Unconditional);
            assert_eq!(first.key()[0].guard(), crate::DeleteGuard::NothingAlive);
            let second = question.open(replaced.clone(), ());
            assert!(question.take(&first).is_none());
            assert!(question.is_current(&second));

            let shrunk = reconcile(&replaced, &[old.clone(), extra], true, Some(1));
            assert_eq!(shrunk.len(), 1);
            assert!(shrunk[0].id == old.id);
            assert_eq!(
                reconcile(&shrunk, &[old.clone(), retained], true, Some(1)).len(),
                1
            );
            let third = question.open(shrunk, ());
            assert!(question.take(&second).is_none());
            assert!(question.take(&third).is_some());
            assert!(question.take(&third).is_none());

            // Answer-time visibility shrinks identities without reading the
            // fresh status as stronger authority for an old rendered guard.
            let eligible = admitted_targets(
                first.key().clone(),
                &[old],
                &HashSet::from(["old".to_string(), "new-mark".to_string()]),
            );
            assert_eq!(eligible.len(), 1);
            assert_eq!(eligible[0].guard(), crate::DeleteGuard::NothingAlive);
            assert!(
                admitted_targets(
                    first.key().clone(),
                    &[],
                    &HashSet::from(["old".to_string()])
                )
                .is_empty()
            );
        });
    }

    /// A read issued before a rename cannot roll the question's displayed name
    /// back while the row's existing optimistic-title fence still protects it.
    #[farhelm_testtrace::test]
    fn reconciliation_preserves_the_rows_displayed_rename_overlay() {
        let old = session("renamed", 1);
        let mut current = old.clone();
        current.title = "new title".into();
        let drawn = snapshots(&[current]);
        let mut renamed = std::collections::HashMap::from([(
            "renamed".to_string(),
            ("new title".to_string(), 10),
        )]);
        crate::rows::settle_optimistic_renames(&mut renamed, std::slice::from_ref(&old), 9);
        assert!(
            renamed.contains_key("renamed"),
            "premise: this read predates the rename"
        );
        let shown = crate::rows::apply_optimistic_renames(&[old], &renamed);
        assert!(reconcile(&drawn, &shown, true, Some(1)) == drawn);
    }

    /// Long and hostile names must not hide the status, full host, or checkout
    /// condition. Unknown explicitly reports uncertainty, never running state.
    #[farhelm_testtrace::test]
    fn structured_dialog_draws_separate_escaped_rows_and_conditional_checkout_consequences() {
        use dioxus::core::{DynamicNode, ScopeId, TemplateNode, VNode};

        /// Static text is part of Dioxus's template, not its mutation stream.
        /// Inspect both halves of the mounted tree so fixed safety wording is
        /// held to the same content assertions as escaped peer values.
        fn collect_text(dom: &VirtualDom, vnode: &VNode, found: &mut Vec<String>) {
            /// Dynamic placeholders are resolved separately from their mounted
            /// vnode, so this walk collects only the template's fixed strings.
            fn fixed(node: &TemplateNode, found: &mut Vec<String>) {
                match node {
                    TemplateNode::Text { text } => found.push((*text).to_string()),
                    TemplateNode::Element { children, .. } => {
                        for child in *children {
                            fixed(child, found);
                        }
                    }
                    TemplateNode::Dynamic { .. } => {}
                }
            }
            for root in vnode.template.roots {
                fixed(root, found);
            }
            for (index, node) in vnode.dynamic_nodes.iter().enumerate() {
                match node {
                    DynamicNode::Text(text) => found.push(text.value.clone()),
                    DynamicNode::Fragment(children) => {
                        for child in children {
                            collect_text(dom, child, found);
                        }
                    }
                    DynamicNode::Component(component) => {
                        if let Some(scope) = component.mounted_scope(index, vnode, dom) {
                            collect_text(dom, scope.root_node(), found);
                        }
                    }
                    DynamicNode::Placeholder(_) => {}
                }
            }
        }

        /// Mount the actual modal with one managed and one unknown target.
        fn app() -> Element {
            let mut question = use_confirm_slot::<Vec<DeleteSnapshot>, ()>();
            let mut live = session("live", 1);
            live.title = "name\u{202e}<b>".into();
            live.status = SessionStatus::Running;
            let mut unknown = session("unknown", 2);
            unknown.status = SessionStatus::Unknown;
            let mut targets = snapshots(&[live, unknown]);
            targets[0].managed = true;
            let shown = use_hook(move || question.open(targets, ()));
            rsx! { DeleteDialog { shown, disabled: false, on_confirm: |_| {}, on_cancel: |_| {} } }
        }
        let mut dom = VirtualDom::new(app);
        let edits = dom.rebuild_to_vec().edits;
        let mut text = Vec::new();
        collect_text(
            &dom,
            dom.get_scope(ScopeId::APP).unwrap().root_node(),
            &mut text,
        );
        let text: Vec<_> = text.iter().map(String::as_str).collect();
        assert!(
            text.contains(&"name<U+202E><b>"),
            "peer title is escaped: {text:?}"
        );
        assert!(text.contains(&"Host 1 (destination.example)"));
        assert!(text.contains(&"Host 2 (destination.example)"));
        assert!(text.contains(&"Unknown"));
        assert!(text.contains(&confirm_consequence(&SessionStatus::Unknown, 0).as_str()));
        assert_eq!(
            text.iter()
                .filter(|value| value.contains("The managed checkout stays"))
                .count(),
            1
        );
        let rows = edits
            .iter()
            .filter(|edit| {
                matches!(
                    edit,
                    dioxus::core::Mutation::SetAttribute {
                        name: "data-delete-target",
                        ..
                    }
                )
            })
            .count();
        assert_eq!(rows, 2);
    }
}
