//! The operation store: work on a session that finishes later, owned above the views that start it.
//!
//! SPEC_impl.md, "Work that finishes later: the operation stores". A Rename or a Delete is started from a row's menu or
//! a dialog, but its reply can arrive after that menu closed, after the user opened another session, or after the row
//! scrolled away. In GPUI, dropping a `Task` cancels it, so work held by a view dies with the view, and the old app's
//! answer (spawning into a scope that outlived the view) is a detached task, which the GUI lints ban. Here the task is
//! held by the [`SessionOperations`] of the current UI session, under the session it acts on, until it settles or a
//! reset retires the store.
//!
//! Here's the TLDR of the rules:
//!
//! - One request per session at a time. While a session's Rename is in flight, a Delete of it is refused at
//!   [`SessionOperations::start`], and a view draws the session's controls as busy from
//!   [`SessionOperations::in_flight`]. This is the request half of SPEC.md's rule that "an open lifecycle confirmation
//!   or a pending request refuses another lifecycle action" (the old app's row exclusion for requests, `admit_row_op`
//!   and `row_pending` in `list/view.rs` at reference commit `24bc0580`). The confirmation half is not here: an open
//!   Delete question or Rename editor belongs to the slice's prompt and editor, and today their modal veil keeps the
//!   row menu out of reach while they are open. Work on different sessions is independent: each keeps its own
//!   completion handling until it settles.
//! - "Latest wins" is not built. It applies only to work that is explicitly supersedable for the same resource (a
//!   search being retyped), and none of the work here is. The first supersedable work adds it here, as a property of
//!   its [`Work`] variant, so that no other work can be superseded by accident.
//! - Dropping a waiter never cancels the helm's work. The command is on the channel once [`SessionOperations::start`]
//!   returns, and nothing here ever sends a cancel: a retired store drops only its own waiting (SPEC_impl.md, "Who owns
//!   an accepted action").
//! - A completion asks the focus coordinator. [`Settled::user_moved_on`] says whether the user pressed a key or
//!   clicked since the work started (the [`Mark`] is taken at the start). When they have, the completion moves no
//!   focus, opens no dialog and switches no session (SPEC.md, "Who may move focus"). Closing its own subject is not on
//!   that list: a successful Rename closes its editor either way, as the field's docs say.
//! - A refusal is reported on its session's row: [`SessionOperations::failure`] keeps the helm's words until the next
//!   operation on that session starts (SPEC.md: "A failure or partial result appears on the source session's row").
//!   Nothing here retries.
//!
//! A reset retires the store with its UI session ([`crate::session`]): every task, and with it every completion
//! callback, is dropped, as are the recorded failures. A completion that was already on its way reaches nobody, and
//! the generic connection-lost notice is the only report (SPEC_impl.md, "One failure path: the reset").
//!
//! Extension points (`docs/gpui/foundation-ownership.md`): a [`Work`] variant for new session work, with a [`Command`]
//! constructor that keeps today's API semantics for its route. Work on something other than a session (a host, a
//! template) needs a store of its own here, keyed by what it acts on.

use std::collections::HashMap;

use farhelm_helm_client::{ApiPath, ChannelHandle, CommandError, Method, PathError};
use farhelm_proto::DeleteGuard;
use farhelm_proto::ui_channel::Reply;
use gpui::{App, Context, Task};
use serde_json::Value;

use crate::focus::{self, Mark};
use crate::host_text::HostText;

/// The kinds of work a session can have in flight. Exhaustive on purpose: each new kind decides here whether it is
/// supersedable, so shared code never has to guess.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Work {
    /// A title change, from the Rename editor.
    Rename,
    /// The session's Delete, after its confirmation.
    Delete,
}

/// One command to the helm, built only by the constructors below so its route keeps today's API semantics.
#[derive(Clone, Debug, PartialEq)]
pub struct Command {
    method: Method,
    path: ApiPath,
    body: Option<Value>,
    /// The command as a refusal without words names it: the method and route with the session's id hidden, as the old
    /// app's `api::diagnostic_url` hid it. The id is an internal reference (a host's uuid and the session's own id),
    /// long and meaningless to the user.
    shown: String,
}

impl Command {
    /// `POST /api/sessions/{id}/rename` with `title` exactly as given: no trimming, no emptiness check, no screening
    /// of control characters. The supervisor owns every rule about what a title may hold, and its refusal is the text
    /// the user acts on; an empty title is a legal rename. Ported from the old app's `api::rename_session`.
    pub fn rename(session: &str, title: &str) -> Result<Command, PathError> {
        Ok(Command {
            method: Method::Post,
            path: ApiPath::api(&["sessions", session, "rename"])?,
            body: Some(serde_json::json!({ "title": title })),
            shown: "POST /api/sessions/[session]/rename".to_owned(),
        })
    }

    /// The ordinary session Delete, `DELETE /api/sessions/{id}`, with the precondition the user's confirmation covered:
    /// `guard` is what that confirmation showed, so a session that became more alive since is refused rather than
    /// killed. Only a guard that asks for something adds its flag, as in the old app's `api::delete_url`.
    ///
    /// NOTE: The helm refuses this for a session in a disposable environment, whose Delete states what happens to the
    /// environment and asks for it with a flag of its own. That refusal is correct, not a bug; the area that ports
    /// environment sessions adds their Delete.
    pub fn delete(session: &str, guard: DeleteGuard) -> Result<Command, PathError> {
        let path = ApiPath::api(&["sessions", session])?;
        let flag = match guard {
            DeleteGuard::NothingAlive => Some("only_if_nothing_alive"),
            DeleteGuard::AgentEnded => Some("only_if_agent_ended"),
            DeleteGuard::Unconditional => None,
        };
        let (path, shown) = match flag {
            Some(flag) => (
                path.with_query(&[(flag, "true")])?,
                format!("DELETE /api/sessions/[session]?{flag}=true"),
            ),
            None => (path, "DELETE /api/sessions/[session]".to_owned()),
        };
        Ok(Command {
            method: Method::Delete,
            path,
            body: None,
            shown,
        })
    }
}

/// How the helm answered an operation's command.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// The helm did it, and the store already shows the effects (the channel orders a reply after the changes it
    /// caused). The body is the route's own reply: the renamed session, or a Delete's outcome with its notice.
    ///
    /// NOTE: Every 2xx is done, because neither route answers 202 (accepted long work, whose success is not implied).
    /// The first route converted to accepted work adds that outcome here.
    Done(Option<Value>),
    /// The helm refused, in its own words, which may quote host values (a supervisor's refusal names the title it
    /// refused), so they are host text. A host timeout is a refusal too, though the work goes on behind it.
    Refused(HostText),
}

impl Outcome {
    /// Classify a reply to the command shown as `shown`. A refusal with no words says which command got which status,
    /// as the old app's `api::refusal_text` did. Those words are the app's own and name no host value, but they still
    /// travel as host text: a refusal is one type wherever its words came from, and escaping the app's words changes
    /// nothing.
    fn from_reply(reply: Reply, shown: &str) -> Outcome {
        match reply.status {
            200..=299 => Outcome::Done(reply.body),
            status => {
                let words = match reply.body {
                    Some(Value::String(text)) => text.trim().to_owned(),
                    Some(other) => other.to_string(),
                    None => String::new(),
                };
                let words = if words.is_empty() {
                    format!("{shown}: {status}")
                } else {
                    words
                };
                Outcome::Refused(HostText::from_host(words))
            }
        }
    }
}

/// What a completion is handed when its operation settles.
#[derive(Clone, Debug, PartialEq)]
pub struct Settled {
    pub outcome: Outcome,
    /// Whether the user pressed a key or clicked inside Farhelm since the operation started. When true, the completion
    /// moves no focus, opens no dialog and switches no session; its result shows where it belongs instead (SPEC.md,
    /// "Who may move focus").
    ///
    /// A completion that closes its own subject still closes it. The Rename editor's rule, decided for the GPUI port:
    /// a successful save closes the editor whether or not the user moved on (SPEC.md: "A successful rename closes that
    /// editor"), and focus goes back to the editor's opener only if focus is still inside the editor; otherwise it
    /// stays where the user put it (an agent approval card, say). The old app did the same (`on_rename_submit` at
    /// reference commit `24bc0580` returned focus only while the dialog owned it).
    pub user_moved_on: bool,
}

/// A refusal recorded on the session it was for, for that session's row to show.
#[derive(Clone, Debug, PartialEq)]
pub struct Failure {
    pub work: Work,
    pub message: HostText,
}

/// Why [`SessionOperations::start`] sent nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotStarted {
    /// The session already has this work in flight. Nothing is queued: the user acts again once it settles.
    Busy(Work),
    /// A reset retired this store, or the window is inert and there is none.
    Retired,
    /// The session's id cannot be a path segment (an empty id, say). The helm never serves one, so this is a bug
    /// report, not a user-facing state.
    Path(PathError),
}

/// One operation in flight: what it is, and the task that waits for its reply. Dropping the entry drops the task, and
/// with it the completion, but never the helm's work.
struct InFlight {
    work: Work,
    _task: Task<()>,
}

/// The operation store of one UI session: the work in flight on each session, and the last refusal of each.
///
/// Views read it while drawing, through [`crate::session::current_operations`], and keep no handle across draws, as
/// with the store. Every change notifies, so a row drawn busy redraws when its operation settles.
pub struct SessionOperations {
    handle: ChannelHandle,
    in_flight: HashMap<String, InFlight>,
    failures: HashMap<String, Failure>,
    retired: bool,
}

impl SessionOperations {
    /// An empty store sending through `handle`, made with its UI session.
    pub(crate) fn new(handle: ChannelHandle) -> SessionOperations {
        SessionOperations {
            handle,
            in_flight: HashMap::new(),
            failures: HashMap::new(),
            retired: false,
        }
    }

    /// The work in flight on `session`, keyed as the helm files the session.
    pub fn in_flight(&self, session: &str) -> Option<Work> {
        self.in_flight.get(session).map(|running| running.work)
    }

    /// The last refusal of work on `session`, until the next operation on it starts. A session that is gone keeps its
    /// entry until the UI session resets; nothing draws it, since no row asks.
    pub fn failure(&self, session: &str) -> Option<&Failure> {
        self.failures.get(session)
    }

    /// Send `command` as `work` on `session`, and call `on_settled` once the helm answers, unless a reset retires the
    /// store first. Call it from the handler of the user's action: that is when the moved-on mark is taken.
    ///
    /// Starting clears the session's previous failure, which is the only thing that clears it (the old app's rule:
    /// a row's error stays until the retry that supersedes it). `on_settled` runs after the store has recorded the
    /// outcome and notified, outside the store's update, so it may read the store. It is never called for a command
    /// the reset lost.
    pub fn start(
        &mut self,
        session: &str,
        work: Work,
        command: Command,
        cx: &mut Context<SessionOperations>,
        on_settled: impl FnOnce(&Settled, &mut App) + 'static,
    ) -> Result<(), NotStarted> {
        if self.retired {
            return Err(NotStarted::Retired);
        }
        if let Some(running) = self.in_flight.get(session) {
            return Err(NotStarted::Busy(running.work));
        }
        let mark = Mark::take(cx);
        let Command {
            method,
            path,
            body,
            shown,
        } = command;
        let reply = self.handle.command(method, path, body);
        let key = session.to_owned();
        let task = cx.spawn(async move |operations, cx| {
            let result = reply.await;
            cx.update(|cx| {
                let settled = operations.update(cx, |operations, cx| {
                    operations.settle(&key, work, result, &shown, &mark, cx)
                });
                if let Ok(Some(settled)) = settled {
                    on_settled(&settled, cx);
                }
            });
        });
        self.failures.remove(session);
        self.in_flight
            .insert(session.to_owned(), InFlight { work, _task: task });
        cx.notify();
        Ok(())
    }

    /// Record how `session`'s operation ended. Returns what its completion is handed, or `None` for a command the
    /// reset lost. That case cannot arrive in practice: the channel announces a break before it fails the commands
    /// it lost, the announcement retires this store, and retiring drops the task that would have run this.
    fn settle(
        &mut self,
        session: &str,
        work: Work,
        result: Result<Reply, CommandError>,
        shown: &str,
        mark: &Mark,
        cx: &mut Context<SessionOperations>,
    ) -> Option<Settled> {
        // This drops the handle of the very task that is running this code. That is safe, and the completion after it
        // still runs: dropping a running task's handle cancels only its next poll, and this poll is its last. Moving
        // the removal after the completion would let the completion see its own session as still busy.
        self.in_flight.remove(session);
        cx.notify();
        let outcome = Outcome::from_reply(result.ok()?, shown);
        if let Outcome::Refused(message) = &outcome {
            self.failures.insert(
                session.to_owned(),
                Failure {
                    work,
                    message: message.clone(),
                },
            );
        }
        Some(Settled {
            outcome,
            user_moved_on: focus::has_moved_on(mark, cx),
        })
    }

    /// The reset retired this store's UI session: drop every waiting task and recorded failure, and start nothing
    /// more. The helm's work goes on; only the waiting ends.
    pub(crate) fn retire(&mut self, cx: &mut Context<SessionOperations>) {
        self.retired = true;
        self.in_flight.clear();
        self.failures.clear();
        cx.notify();
    }
}
