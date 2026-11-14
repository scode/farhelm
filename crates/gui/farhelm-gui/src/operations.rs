//! The operation store: work that finishes later, owned above the views that start it.
//!
//! SPEC_impl.md, "Work that finishes later: the operation stores". A Rename or a Delete is started from a row's menu or
//! a dialog, but its reply can arrive after that menu closed, after the user opened another session, or after the row
//! scrolled away; a template's or the preferences' save outlives its editor the same way. In GPUI, dropping a `Task`
//! cancels it, so work held by a view dies with the view, and the old app's answer (spawning into a scope that
//! outlived the view) is a detached task, which the GUI lints ban. Here the task is held by the [`Operations`] of the
//! current UI session, under the [`OpKey`] of what it acts on, until it settles or a reset retires the store.
//!
//! What work acts on is named by an [`OpKey`]: a session, a host, a template, the preferences, or a launch that has no
//! session yet. Keys of different kinds never collide, so a host and a session with the same id string are independent.
//!
//! Here's the TLDR of the rules:
//!
//! - One request per key at a time. While a session's Rename is in flight, a Delete of it is refused at
//!   [`Operations::start`], and a view draws the session's controls as busy from [`Operations::in_flight`]. This is
//!   the request half of SPEC.md's rule that "an open lifecycle confirmation or a pending request refuses another
//!   lifecycle action" (the old app's row exclusion for requests, `admit_row_op`
//!   and `row_pending` in `list/view.rs` at reference commit `24bc0580`). The confirmation half is not here: an open
//!   Delete question belongs to the session list, which keeps the row's menu button inert while its question is open,
//!   and the Rename editor to its own feature. Work on different keys is independent: each keeps its own completion
//!   handling until it settles.
//! - "Latest wins" is not built. It applies only to work that is explicitly supersedable for the same resource (a
//!   search being retyped), and none of the work here is. The first supersedable work adds it here, as a property of
//!   its [`Work`] variant, so that no other work can be superseded by accident.
//! - Dropping a waiter never cancels the helm's work. The command is on the channel once [`Operations::start`]
//!   returns, and nothing here ever sends a cancel: a retired store drops only its own waiting (SPEC_impl.md, "Who owns
//!   an accepted action").
//! - A completion asks the focus coordinator. [`Settled::user_moved_on`] says whether the user pressed a key or
//!   clicked since the work started (the [`Mark`] is taken at the start). When they have, the completion moves no
//!   focus, opens no dialog and switches no session (SPEC.md, "Who may move focus"). Closing its own subject is not on
//!   that list: a successful Rename closes its editor either way, as the field's docs say.
//! - A refusal is reported where its subject is shown: [`Operations::failure`] keeps the helm's words until the next
//!   operation on that key starts (SPEC.md, for sessions: "A failure or partial result appears on the source
//!   session's row"). Nothing here retries.
//!
//! A reset retires the store with its UI session ([`crate::session`]): every task, and with it every completion
//! callback, is dropped, as are the recorded failures. A completion that was already on its way reaches nobody, and
//! the generic connection-lost notice is the only report (SPEC_impl.md, "One failure path: the reset").
//!
//! Extension points (`docs/gpui/foundation-ownership.md`): a [`Work`] variant for a new kind of work. The commands
//! themselves are built where their feature lives, with [`Command::new`], keeping today's API semantics for their
//! route (the session list builds its Rename and Delete): this module holds the store, not a catalog of routes. A
//! new kind of key ([`OpKey`]) is a change here.

use std::collections::HashMap;

use farhelm_helm_client::{ApiPath, ChannelHandle, CommandError, Method, PathError};
use farhelm_proto::DeleteGuard;
use farhelm_proto::ui_channel::Reply;
use gpui::{App, Context, Task};
use serde_json::Value;

use crate::focus::{self, Mark};
use crate::host_text::HostText;

/// The kinds of work that can be in flight on a key. Exhaustive on purpose: each new kind decides here whether it is
/// supersedable, so shared code never has to guess.
///
/// A [`Command`] names its own work, and [`Operations::start`] takes the work from the command it sends, so what
/// [`Operations::in_flight`] reports always describes the request on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Work {
    /// A title change, from the Rename editor.
    Rename,
    /// The session's Delete, after its confirmation, with the precondition that confirmation covered. The guard is
    /// what lets the row say what is happening while the Delete runs (SPEC.md, "Lifecycle operations"): an
    /// [`DeleteGuard::Unconditional`] Delete is stopping the agent, an [`DeleteGuard::AgentEnded`] one is stopping its
    /// terminal tabs, and a [`DeleteGuard::NothingAlive`] one has only state to remove ("deleting"). The old app chose
    /// its in-progress label from the same status that chose the guard (`delete_guard` and `delete_progress_label` in
    /// `status.rs` at reference commit `24bc0580`); here the row reads the guard back instead of recomputing it from a
    /// status that may have moved since.
    Delete(DeleteGuard),
    /// An editor's explicit save of its drafts ([`crate::drafts::save`]): a template, the preferences, a host's
    /// settings. One kind for every editor, because the store treats them alike: none is supersedable, and each is
    /// busy only on its own key.
    Save,
}

/// What a piece of work acts on: the key the store files it under, one request at a time per key. Opaque, and built
/// only by its constructors, so two kinds of thing can never share a key by accident (a host and a session whose ids
/// happen to be equal stay independent), and code that holds one cannot read an id back out of it to act on.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OpKey(Key);

/// The kinds of thing work acts on. Private: [`OpKey`]'s constructors are the only way to make one.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Key {
    Session(String),
    Host(String),
    Template(String),
    Preferences,
    Launch(u64),
}

impl OpKey {
    /// A session, by the id the helm serves it under.
    pub fn session(id: &str) -> OpKey {
        OpKey(Key::Session(id.to_owned()))
    }

    /// A host, by its registry id.
    pub fn host(id: &str) -> OpKey {
        OpKey(Key::Host(id.to_owned()))
    }

    /// A template, by its name.
    pub fn template(name: &str) -> OpKey {
        OpKey(Key::Template(name.to_owned()))
    }

    /// The preferences, which the helm keeps as one entry.
    pub fn preferences() -> OpKey {
        OpKey(Key::Preferences)
    }

    /// A launch, sent before any session exists for it. `n` must be unique for the life of the UI session's store
    /// (across the launcher's openings, and quick launches, which never open it): a number reused while an earlier
    /// launch with it is in flight would be refused as busy, and SPEC.md lets New and the shortcuts start a fresh
    /// launch at once. A counter kept beside the store, not in a launcher view, gives that.
    pub fn launch(n: u64) -> OpKey {
        OpKey(Key::Launch(n))
    }
}

/// One command to the helm, with the work it does and how a refusal without words names it. Built where its feature
/// lives, keeping today's API semantics for its route; nothing here knows any route.
#[derive(Clone, Debug, PartialEq)]
pub struct Command {
    work: Work,
    method: Method,
    path: ApiPath,
    body: Option<Value>,
    /// The command as a refusal without words names it: the method and route with any internal id hidden, as the old
    /// app's `api::diagnostic_url` hid a session's. Such an id (a host's uuid, a session's own id) is long and
    /// meaningless to the user.
    shown: String,
}

impl Command {
    /// A command doing `work`: `method` on `path` with `body`, and `shown` as a refusal without words names it (the
    /// method and route, with any internal id hidden, as `POST /api/sessions/[session]/rename`).
    ///
    /// NOTE: Only for a route that answers done or refused. A route that can answer 202 (accepted long work) would be
    /// reported done here before its work succeeded ([`Outcome::Done`]'s NOTE); the accepted outcome comes first.
    pub fn new(
        work: Work,
        method: Method,
        path: ApiPath,
        body: Option<Value>,
        shown: impl Into<String>,
    ) -> Command {
        Command {
            work,
            method,
            path,
            body,
            shown: shown.into(),
        }
    }

    /// The work this command does, as [`Operations::in_flight`] reports it while the command runs.
    pub fn work(&self) -> Work {
        self.work
    }

    /// The route the command goes to, query included: what a feature's tests check its own commands by.
    pub fn path(&self) -> &ApiPath {
        &self.path
    }
}

/// How the helm answered an operation's command.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// The helm did it, and the store already shows the effects (the channel orders a reply after the changes it
    /// caused). The body is the route's own reply (the renamed session, a Delete's outcome with its notice).
    ///
    /// NOTE: Every 2xx is done, which is right only because no command sent through the store goes to a route that
    /// answers 202 (accepted long work, whose success is not implied). Some helm routes do (host setup and update,
    /// for instance); the first feature that sends one through the store adds the accepted outcome here first, as
    /// [`Command::new`] says.
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

/// A refusal recorded on the key it was for, for whatever shows that subject (a session's row, an editor) to show.
#[derive(Clone, Debug, PartialEq)]
pub struct Failure {
    pub work: Work,
    pub message: HostText,
}

/// Why [`Operations::start`] sent nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotStarted {
    /// The key already has this work in flight. Nothing is queued: the user acts again once it settles.
    Busy(Work),
    /// A reset retired this store, or the window is inert and there is none.
    Retired,
    /// An id in the command's route cannot be a path segment (an empty session id, say). The helm never serves one,
    /// so this is a bug report, not a user-facing state.
    Path(PathError),
}

/// One operation in flight: what it is, and the task that waits for its reply. Dropping the entry drops the task, and
/// with it the completion, but never the helm's work.
struct InFlight {
    work: Work,
    _task: Task<()>,
}

/// The operation store of one UI session: the work in flight on each key, and the last refusal of each.
///
/// Views read it while drawing, through [`crate::session::current_operations`], and keep no handle across draws, as
/// with the store. Every change notifies, so a row drawn busy redraws when its operation settles.
pub struct Operations {
    handle: ChannelHandle,
    in_flight: HashMap<OpKey, InFlight>,
    failures: HashMap<OpKey, Failure>,
    retired: bool,
}

impl Operations {
    /// An empty store sending through `handle`, made with its UI session.
    pub(crate) fn new(handle: ChannelHandle) -> Operations {
        Operations {
            handle,
            in_flight: HashMap::new(),
            failures: HashMap::new(),
            retired: false,
        }
    }

    /// The work in flight on `key`.
    pub fn in_flight(&self, key: &OpKey) -> Option<Work> {
        self.in_flight.get(key).map(|running| running.work)
    }

    /// Whether any key has work in flight that `matches`: the session list asks it about Deletes, whose settling
    /// its automatic selection waits for (SPEC.md: "Automatic selection after Delete waits until every admitted Delete
    /// has settled"), wherever they were started.
    pub fn any_in_flight(&self, matches: impl Fn(Work) -> bool) -> bool {
        self.in_flight.values().any(|running| matches(running.work))
    }

    /// The last refusal of work on `key`, until the next operation on it starts. A key whose subject is gone (a deleted
    /// session) keeps its entry until the UI session resets; nothing draws it, since nothing asks.
    pub fn failure(&self, key: &OpKey) -> Option<&Failure> {
        self.failures.get(key)
    }

    /// Send `command` on `key`, as the work it names ([`Command::work`]), and call `on_settled` once the helm
    /// answers, unless a reset retires the store first. Call it from the handler of the user's action: that is when the
    /// moved-on mark is taken.
    ///
    /// Starting clears the key's previous failure, which is the only thing that clears it (the old app's rule:
    /// a row's error stays until the retry that supersedes it). `on_settled` runs after the store has recorded the
    /// outcome and notified, outside the store's update, so it may read the store. It is never called for a command
    /// the reset lost.
    pub fn start(
        &mut self,
        key: &OpKey,
        command: Command,
        cx: &mut Context<Operations>,
        on_settled: impl FnOnce(&Settled, &mut App) + 'static,
    ) -> Result<(), NotStarted> {
        if self.retired {
            return Err(NotStarted::Retired);
        }
        if let Some(running) = self.in_flight.get(key) {
            return Err(NotStarted::Busy(running.work));
        }
        let mark = Mark::take(cx);
        let Command {
            work,
            method,
            path,
            body,
            shown,
        } = command;
        let reply = self.handle.command(method, path, body);
        let settling = key.clone();
        let task = cx.spawn(async move |operations, cx| {
            let result = reply.await;
            cx.update(|cx| {
                let settled = operations.update(cx, |operations, cx| {
                    operations.settle(&settling, work, result, &shown, &mark, cx)
                });
                if let Ok(Some(settled)) = settled {
                    on_settled(&settled, cx);
                }
            });
        });
        self.failures.remove(key);
        self.in_flight
            .insert(key.clone(), InFlight { work, _task: task });
        cx.notify();
        Ok(())
    }

    /// Record how `key`'s operation ended. Returns what its completion is handed, or `None` for a command the
    /// reset lost. That case cannot arrive in practice: the channel announces a break before it fails the commands
    /// it lost, the announcement retires this store, and retiring drops the task that would have run this.
    fn settle(
        &mut self,
        key: &OpKey,
        work: Work,
        result: Result<Reply, CommandError>,
        shown: &str,
        mark: &Mark,
        cx: &mut Context<Operations>,
    ) -> Option<Settled> {
        // This drops the handle of the very task that is running this code. That is safe, and the completion after it
        // still runs: dropping a running task's handle cancels only its next poll, and this poll is its last. Moving
        // the removal after the completion would let the completion see its own key as still busy.
        self.in_flight.remove(key);
        cx.notify();
        let outcome = Outcome::from_reply(result.ok()?, shown);
        if let Outcome::Refused(message) = &outcome {
            self.failures.insert(
                key.clone(),
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
    pub(crate) fn retire(&mut self, cx: &mut Context<Operations>) {
        self.retired = true;
        self.in_flight.clear();
        self.failures.clear();
        cx.notify();
    }
}
