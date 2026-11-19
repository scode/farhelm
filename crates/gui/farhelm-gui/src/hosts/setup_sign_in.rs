//! A pending setup sign-in holds the initiating host operation while SSH asks its questions. The hold leaves
//! the UI channel free to send answers and ends on the helm's socket check, a reset, or dismissal.

use std::cell::Cell;
use std::time::Duration;

use farhelm_proto::provisioning::RunAccepted;
use farhelm_proto::ui_channel::{
    ApiPath, Method, PendingSshSignIn, SshQuestion, SshSignInCompletion, SshSignInOutcome,
};
use gpui::{AnyWindowHandle, App, AppContext as _, Context, Entity, Subscription};

use crate::focus::Mark;
use crate::host_text::HostText;
use crate::operations::{Command, NotStarted, OpKey, Operations, Outcome, Settled, Work};
use crate::sign_in_form::Handoff;
use crate::{session, ticker};

type Completion = Box<dyn FnOnce(Result<(), HostText>, Mark, &mut App)>;

/// The helm grants each question two minutes. Also bound a wait after a missing completion, allowing time for
/// the initial launch and final socket check. A new question renews this local inactivity bound.
const ABANDON_AFTER: Duration = Duration::from_secs(150);

/// Only planning routes use this 202 shape. Accepted setup and update runs have a different reply.
pub(super) fn pending(outcome: &Outcome) -> Option<PendingSshSignIn> {
    let Outcome::Accepted(Some(body)) = outcome else {
        return None;
    };
    serde::Deserialize::deserialize(body).ok()
}

/// A dismissed Add dialog has no place for a later question. Cancel by the pending reply's attempt id.
pub(super) fn cancel_draft(attempt: &str, cx: &mut App) {
    if let Ok(path) = ApiPath::api(&["ssh-sign-in-drafts", attempt, "cancel"]) {
        crate::operations::send(
            Method::Post,
            path,
            None,
            "Cancel SSH sign-in",
            cx,
            |_, _| {},
        );
    }
}

/// A dialog or window owns the continuation. The held key belongs to the operation store that accepted the
/// request. On reset we drop the continuation, even if an identical host key exists in the replacement store.
pub(crate) struct Waiting {
    attempt: String,
    operations: Entity<Operations>,
    key: OpKey,
    complete: Option<Completion>,
    mark: Mark,
    pending_sends: Cell<usize>,
    question: Option<String>,
    deadline: Option<ticker::Deadline>,
    _watch: session::ObservingCurrent,
    _release: Subscription,
}

impl Waiting {
    pub(super) fn start(
        attempt: String,
        key: OpKey,
        work: Work,
        mark: Mark,
        complete: impl FnOnce(Result<(), HostText>, Mark, &mut App) + 'static,
        cx: &mut App,
    ) -> Option<Entity<Self>> {
        let operations = session::current_operations(cx)?;
        operations
            .update(cx, |operations, cx| operations.hold(&key, work, cx))
            .ok()?;
        Some(cx.new(|cx| {
            let watch =
                session::observe_current(cx, |waiting: &mut Waiting, cx| waiting.changed(cx));
            let release = cx.on_release(|waiting: &mut Waiting, cx| waiting.release(cx));
            let mut waiting = Self {
                attempt,
                operations,
                key,
                complete: Some(Box::new(complete)),
                mark,
                pending_sends: Cell::new(0),
                question: None,
                deadline: None,
                _watch: watch,
                _release: release,
            };
            waiting.restart_deadline(cx);
            waiting
        }))
    }

    pub(crate) fn active(&self) -> bool {
        self.complete.is_some()
    }

    /// The initiating action's mark, refreshed by receipts for answers sent from the focused field.
    pub(crate) fn mark(&self) -> Mark {
        self.mark
    }

    /// A successful socket check and a Send receipt can arrive in either order. Hold a ready result until
    /// focused Sends have settled, so replay takes the accepted input mark even if the form is already gone.
    pub(crate) fn answer_handoff(waiter: &Entity<Self>, mark: Mark, cx: &App) -> Handoff {
        let pending = &waiter.read(cx).pending_sends;
        pending.set(pending.get() + 1);
        let waiter = waiter.downgrade();
        Box::new(move |accepted, cx| {
            let _ = waiter.update(cx, |waiting, cx| {
                waiting.pending_sends.set(waiting.pending_sends.get() - 1);
                if accepted && !crate::focus::has_moved_on(&mark, cx) {
                    waiting.mark = mark;
                }
                waiting.changed(cx);
            });
        })
    }

    fn release(&self, cx: &mut App) {
        if self.active() {
            self.operations
                .update(cx, |operations, cx| operations.release(&self.key, cx));
        }
    }

    fn restart_deadline(&mut self, cx: &mut Context<Self>) {
        let weak = cx.weak_entity();
        self.deadline = Some(ticker::after(ABANDON_AFTER, cx, move |cx| {
            let _ = weak.update(cx, |waiting, cx| {
                waiting.finish(
                    Err(HostText::from_host(
                        "SSH sign-in did not finish. Try the host action again.",
                    )),
                    cx,
                );
            });
        }));
    }

    fn finish(&mut self, result: Result<(), HostText>, cx: &mut Context<Self>) {
        self.release(cx);
        self.deadline = None;
        if let Some(complete) = self.complete.take() {
            let mark = self.mark;
            cx.defer(move |cx| complete(result, mark, cx));
        }
    }

    /// A stream change can arrive before the pending command reply. Read the current collection on the first
    /// observation too, keeping the result authoritative even when the question has already disappeared.
    fn changed(&mut self, cx: &mut Context<Self>) {
        if !self.active() {
            return;
        }
        if session::current_operations(cx).as_ref() != Some(&self.operations) {
            self.finish(
                Err(HostText::from_host(
                    "The connection closed during SSH sign-in.",
                )),
                cx,
            );
            return;
        }
        let Some(store) = session::current_store(cx) else {
            return;
        };
        let collections = store.read(cx).collections();
        if let Some(completion) = collections.get::<SshSignInCompletion>(&self.attempt) {
            let result = match &completion.outcome {
                SshSignInOutcome::Ready if self.pending_sends.get() != 0 => return,
                SshSignInOutcome::Ready => Ok(()),
                SshSignInOutcome::Failed { reason } => Err(HostText::from_host(reason.as_str())),
            };
            self.finish(result, cx);
        } else if let Some(question) = collections
            .ordered::<SshQuestion>()
            .into_iter()
            .find(|question| question.attempt == self.attempt)
            && self.question.as_deref() != Some(&question.id)
        {
            self.question = Some(question.id.clone());
            self.restart_deadline(cx);
        }
    }
}

/// Run a registered host's planning command normally until it returns a pending sign-in. After the setup
/// connection answers, send the same command through its ordinary planning path. Consumers handle the normal
/// refusal and success replies in their own terms, including whether a further confirmation is required.
pub(super) fn plan(
    key: OpKey,
    command: Command,
    mark: Mark,
    window: AnyWindowHandle,
    cx: &mut App,
    settled: impl FnOnce(Result<&Settled, HostText>, Mark, &mut App) + 'static,
) -> Result<(), NotStarted> {
    let operations = session::current_operations(cx).ok_or(NotStarted::Retired)?;
    let replay = command.clone();
    let planned = key.clone();
    operations.update(cx, |operations, cx| {
        operations.start(&key, command, cx, move |result, cx| {
            let Some(pending) = pending(&result.outcome) else {
                settled(Ok(result), mark, cx);
                return;
            };
            let attempt = pending.attempt;
            let waiting = Waiting::start(
                attempt.clone(),
                planned.clone(),
                replay.work(),
                mark,
                move |result, mark, cx| match result {
                    Ok(()) => {
                        let _ = plan(planned, replay, mark, window, cx, settled);
                    }
                    Err(reason) => settled(Err(reason), mark, cx),
                },
                cx,
            );
            if let Some(waiting) = waiting {
                let _ = window.update(cx, |_, window, cx| {
                    crate::sign_in::watch_setup(attempt, waiting, window, cx);
                });
            }
        })
    })
}

/// A confirmed run can start another setup master if the one used to plan has expired. For a new SSH host
/// the same action also allows the new host's sessions sign-in after the Add dialog closes.
pub(super) fn accepted(
    outcome: &Outcome,
    adding_ssh_host: bool,
    mark: Mark,
    window: AnyWindowHandle,
    cx: &mut App,
) {
    let Outcome::Accepted(Some(body)) = outcome else {
        return;
    };
    let Ok(run) = <RunAccepted as serde::Deserialize>::deserialize(body) else {
        return;
    };
    let _ = window.update(cx, |_, window, cx| {
        crate::sign_in::started_run(run.host_id, run.run_id, adding_ssh_host, mark, window, cx);
    });
}
