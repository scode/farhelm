//! The operation store: work that finishes later, owned above the views that start it.
//!
//! SPEC_impl.md, "Work that finishes later: the operation stores". A Rename or a Delete is started from a row's menu or
//! a dialog, but its reply can arrive after that menu closed, after the user opened another session, or after the row
//! scrolled away; a template's or the preferences' save outlives its editor the same way. In GPUI, dropping a `Task`
//! cancels it, so work held by a view dies with the view, and the old app's answer (spawning into a scope that
//! outlived the view) is a detached task, which the GUI lints ban. Here the task is held by the [`Operations`] of the
//! current UI session, under the [`OpKey`] of what it acts on, until it settles or a reset retires the store.
//!
//! What work acts on is named by an [`OpKey`]: a session, a host, a template, the preferences, or a launch or a host
//! being added, neither of which has its id yet. Keys of different kinds never collide, so a host and a session with the same id string are independent.
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
//!   handling until it settles. Where an area says that a start was refused because its key already has work in
//!   flight, it gives the shared reason ([`NotStarted::BUSY_REASON`]) after a lead-in of its own ("Not saved:",
//!   "Not started:") rather than a reason of its own, unless the line ports the old app's words, which it keeps.
//! - A key can be held before its request is sent ([`Operations::hold`]): a bulk Stop or Delete row queued behind the
//!   other rows of its host reads as that work in flight, so it is drawn pending and refuses every other start, with
//!   nothing on the wire until its holder releases it and starts the request ([`Operations::release`]). A reset drops
//!   holds with everything else.
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
//!   session's row"). A partial result goes in the same place ([`Operations::note`]). Nothing here retries. A feature
//!   that showed a refusal somewhere else, such as a dialog, forgets the record ([`Operations::forget`]).
//! - A refusal is more than words. Its completion gets a [`Refusal`]: the words, the reply's status and the helm-only
//!   markers the reply carried ([`HelmMarker`]), so a feature tells a refusal it answers (a YOLO launch to confirm, a
//!   host busy with another operation, a timeout whose work goes on) from one it only shows, without reading the
//!   words, which can quote a supervisor. Only the words are kept on the key.
//!
//! A reset retires the store with its UI session ([`crate::session`]): every task, and with it every completion
//! callback, is dropped, as are the recorded failures. A completion that was already on its way reaches nobody, and
//! the generic connection-lost notice is the only report (SPEC_impl.md, "One failure path: the reset").
//!
//! Extension points (`docs/gpui/foundation-ownership.md`): a [`Work`] variant for a new kind of work, which also
//! decides whether it creates a session and whether it changes the host registry ([`Work::creates_a_session`],
//! [`Work::changes_host_registry`]). The commands themselves are built where their feature lives, with
//! [`Command::new`], keeping today's API semantics for their route (the session list builds its Rename and Delete):
//! this module holds the store, not a catalog of routes. A new kind of key ([`OpKey`]) is a change here.
//!
//! Not everything sent to the helm is an operation. A read (a GET) changes nothing on the helm, and some writes (a
//! remembered preference, the automatic seen mark, the host rail's session query) need nothing the store keeps for an
//! operation: nothing draws them busy and no refusal of theirs is recorded on a key. [`send`] sends such a command
//! through the live UI session and words its refusal exactly as the store words an operation's, so a write, a read and
//! a save that fail on the same helm say so in the same way; [`read`] is a [`send`] of a GET plus a decode. The
//! caller decides what, if anything, to show from the result.

use std::collections::HashMap;

use farhelm_helm_client::{ApiPath, ChannelHandle, CommandError, Method, PathError};
use farhelm_proto::DeleteGuard;
/// Re-exported so a feature that branches on a refusal names its markers from the module that hands them over.
pub use farhelm_proto::ui_channel::HelmMarker;
use farhelm_proto::ui_channel::Reply;
use gpui::{App, Context, Task};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::focus::{self, Mark};
use crate::host_text::HostText;

/// The kinds of work that can be in flight on a key. Exhaustive on purpose: each new kind decides here whether it is
/// supersedable, so shared code never has to guess.
///
/// A [`Command`] names its own work, and [`Operations::start`] takes the work from the command it sends, so what
/// [`Operations::in_flight`] reports always describes the request on the wire, or the work a hold reserved the key for
/// before its request is sent ([`Operations::hold`]).
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
    /// The session's Stop, with the preconditions of the row it was asked from (`session_list::stop`). Not
    /// supersedable: a second Stop of the same session waits for the first to settle.
    Stop,
    /// The session's plain Replace, which creates its replacement and then deletes it, with the guard of the row it
    /// was asked from (`session_list::replace`). Not supersedable, and not a Delete for the selection rules: only an
    /// admitted Delete holds the automatic choice back.
    Replace,
    /// The session's manual Mark read or Mark unread, from its row's menu or its status dot (`session_list::seen`).
    /// Not supersedable. It takes the session's key like any request, so while it is in flight the session's other
    /// actions are unavailable, as SPEC.md has every pending request refuse the session's competing ones. The
    /// automatic seen mark is not work here: it is a best-effort write nobody waits for.
    Seen { read: bool },
    /// The session's plain Restart from the open session's header, which resumes its own conversation, with the consent
    /// to stop a working agent that its question (or a skipped one) gave (`session_list::restart`). Not supersedable:
    /// Restart makes no promise that a second press is harmless (SPEC.md, "Lifecycle operations").
    Restart,
    /// A host probe (`POST /api/hosts/probe`): Add host's check of a new destination, which registers a supervisor that
    /// answers and otherwise returns the helm's setup plan or why setup is manual. Not supersedable: a second Add while
    /// one probe is out waits for it, so the helm never holds two competing one-use plans for one dialog.
    Probe,
    /// A new host's setup (`POST /api/hosts/provision`), confirming the plan a probe returned, under the probe's
    /// host-add key. Accepted (202) once the helm has registered the host and started the run, whose progress the helm
    /// pushes. Not supersedable: the plan is one-use.
    Setup,
    /// Opening a remote session's loopback link through a tunnel (`POST /api/sessions/{id}/tunnels/open`), on the
    /// session's tunnel key ([`OpKey::tunnels`]); the helm opens the browser itself. Not supersedable: the old app
    /// allowed one tunnel action at a time, and a second waits for the first.
    TunnelOpen,
    /// Closing one of a session's tunnels (`DELETE /api/sessions/{id}/tunnels/{tunnel}`), on the same key, so Close
    /// all closes them one after another. Not supersedable.
    TunnelClose,
    /// The session launcher's New or Clone (`POST /api/sessions`), under a fresh launch key ([`OpKey::new_launch`])
    /// for each submit (`launcher::launch`). Not supersedable: SPEC.md has one intended create yield one session or a
    /// clear error, so nothing replaces a create on the wire. Restart with and Replace with act on their source session
    /// and run as its [`Work::Restart`] and [`Work::Replace`] instead.
    Launch,
    /// A host's removal from the registry (`DELETE /api/hosts/{id}`), under the host's key: the helm forgets the entry
    /// and leaves the host's supervisor and sessions running (`hosts::remove`). Not supersedable: a second Remove of the
    /// same host waits for the first to settle.
    RemoveHost,
    /// The user's answer to an agent's approval request (`POST /api/approvals/{id}`), under the request's own key
    /// ([`OpKey::approval`]). Not supersedable: the first answer pressed wins, and a second waits for it to settle
    /// (SPEC.md, "Agent-spawned sessions").
    Answer,
    /// A host's Update (`POST /api/hosts/{id}/update`), under the host's key: first the plan request without a body,
    /// which answers 200 with a one-use plan, then the confirming request naming that plan, which the helm accepts
    /// (202) once it has started the run whose progress it pushes (`hosts::update`). Not supersedable: the plan is
    /// one-use, and a second Update of the same host waits for the first to settle.
    UpdateHost,
    /// A feedback submission (`POST /api/feedback`), from the Send feedback dialog, on the feedback key
    /// ([`OpKey::feedback`]). Not supersedable: the dialog sends once per press and keeps itself open until the send
    /// settles, so no second send can be waiting behind it.
    SendFeedback,
    /// A permanent cleanup of archived checkouts on one host (`POST /api/checkout-trash/delete`), under the host's trash
    /// key ([`OpKey::trash`]): the Trash dialog's own Delete or Retry of one checkout (`crate::trash`). Not supersedable:
    /// the trash control prevents overlapping deletes of its own (SPEC.md, "Managed checkouts").
    TrashDelete,
    /// Removing Farhelm from a remote host (`POST /api/hosts/{id}/uninstall`), under the host's key: first the plan
    /// request without a body, which answers 200 with the one-use plan the user confirms, then the confirming request
    /// naming that plan, which the helm accepts (202) once it has started the run whose progress it pushes
    /// (`hosts::uninstall`). Not supersedable: the plan is one-use. It changes the host registry, since the run's last
    /// step forgets the host.
    UninstallHost,
    /// Opening a terminal tab in a session (`POST /api/sessions/{id}/tabs`), on the session's own key, so an open
    /// lifecycle request refuses it and it holds the session's lifecycle actions while it runs (SPEC.md: a pending
    /// request refuses "another lifecycle action, New terminal or Clone on that session"). Not supersedable: each press
    /// opens one shell.
    TabOpen,
    /// Closing one terminal tab (`DELETE /api/sessions/{id}/tabs/{tab}`), on that tab's own key ([`OpKey::tab`]), so
    /// sibling tabs close side by side and a close holds none of the session's lifecycle actions. Not supersedable.
    TabClose,
    /// The host menu's Retry (`POST /api/hosts/{id}/retry`), under the host's key: the helm drops the host's
    /// connection and tries it again now, or starts a fresh connection actor for a retired entry (`hosts::remedies`).
    /// Answered at once, with the outcome in the pushed host list. Not supersedable. It changes no registry entry.
    RetryHost,
    /// The host menu's Adopt (`POST /api/hosts/{id}/adopt`), under the host's key, naming the identity the menu
    /// showed: the helm records that identity for the entry, resets the host's five permissions to asking and
    /// connects again (`hosts::remedies`). Not supersedable. It changes the registry's entry for the host.
    AdoptHost,
    /// Confirm management of a host. Add host registers the discovered
    /// entry. A frozen row only asks its existing connection to retry.
    TakeOverHost { registers: bool },
    /// The session's Pin to top or Unpin (`PUT /api/sessions/{id}/pin`), from its row's menu (`session_list::pins`),
    /// on the session's own key like Mark read, so its other actions are unavailable while it runs. Not supersedable.
    Pin,
    /// Write the session's shared mute, or end it and mark its current work seen.
    Mute,
    /// The session's Move up or Move down among the pins (`PUT /api/sessions/{id}/pin-order`), from its row's menu, on
    /// the session's own key. Apart from [`Work::Pin`] because the old app named the two refusals differently ("Pin:"
    /// and "Move pin:"). Not supersedable: each press names the neighbor it was drawn with.
    MovePin,
    /// Write membership or a group's attributes and order. Membership uses
    /// the session key. A header write uses its own group key.
    SessionGroup,
    /// The Suspend of an environment session's host (`POST /api/hosts/{id}/suspend`). A session's Suspend, from its
    /// row menu or header, runs on the session's own key, so its other lifecycle actions wait for it as the old app's
    /// session claim made them wait (`session_list::environment`); the Hosts panel's Suspend runs on the host's key
    /// (`OpKey::host`). The two surfaces hold separate requests, as in the old app, and the helm's provisioning lock
    /// answers an overlap as busy. Not supersedable. It changes no registry entry: SPEC.md says Suspend and Resume are
    /// not changes to the host registry, so host changes and launches elsewhere go on beside it.
    Suspend,
    /// The Resume of an environment session's host, or of an ordinary host a session's card offers to resume
    /// (`POST /api/hosts/{id}/resume`). A session's runs on the session's own key and the Hosts panel's on the host's
    /// key, like [`Work::Suspend`]. Not supersedable.
    Resume,
    /// An environment session's Stop and suspend (`POST /api/sessions/{id}/stop-and-suspend`): its ordinary Stop, then
    /// its host's Suspend, as one helm operation, on the session's own key. Not supersedable. The Stop half ends an
    /// agent, but it creates no session and changes no registry entry.
    StopAndSuspend,
    /// The user's Delete of a session in a disposable environment (`DELETE /api/sessions/{id}` with
    /// `delete_environment_session` or `delete_sandbox`, `session_list::environment_delete`), on the session's own
    /// key, so its other actions wait for it as they wait for an ordinary Delete. Not supersedable: each answer of its
    /// question sends one. It carries no guard, since the helm refuses a guarded environment Delete. The
    /// whole-environment lane destroys the environment through host removal's path, but it creates no session and adds
    /// or replaces no registry entry, and the old app put neither lane through the launch and registry exclusion.
    EnvironmentDelete,
}

/// The two halves of SPEC.md's "Launches and changes to the host registry still exclude each other". Each side refuses
/// its own action, with a reason, while the other side's work is in flight anywhere in the store (one of these methods
/// passed to [`Operations::any_in_flight`]); there is no shared lock (decided by the migration's orchestrator,
/// 2026-11-14).
///
/// Both sets live here, rather than as a match in each area that refuses, because they are one cross-area fact: two
/// hand-kept copies drift apart when a new kind of work arrives. Areas ask these methods whether work creates a session
/// or changes the host registry, rather than keeping their own match over kinds of work for either. Other questions
/// about work are not these sets and keep their own matches (the session list's check for a Delete in flight, a row's
/// wording per kind of work).
///
/// Both matches are exhaustive, so a new [`Work`] variant decides both in the same change that adds it. They keep one
/// arm per line so that a new variant's arms are pure additions, which the file's `append-only` extension rule admits
/// (`docs/gpui/foundation-ownership.md`); grouped arms (`A | B => false`) would turn each addition into a changed line,
/// which that rule does not admit. A PR that adds a variant pins its two answers in its own area's tests:
/// `operations_tests.rs` is not an extension point, and its table covers only the kinds of work that existed when
/// these methods were added.
impl Work {
    /// Whether this work creates a session on a host: what "a launch in flight" means to the host registry's side
    /// (`hosts::launch_in_flight`), which refuses Add host, a new host's setup, a host's Re-run, removal, Update and
    /// Uninstall, and a host's destination save while one is out. The launcher's New and Clone create one, and so do
    /// a session's Restart and Replace, which run as their source session's work rather than as [`Work::Launch`]. An
    /// answer to an agent's approval card does not, even when it allows a launch: the helm runs what was approved
    /// (decided by the migration's orchestrator, 2026-11-14).
    pub fn creates_a_session(self) -> bool {
        match self {
            Work::Rename => false,
            Work::Delete(_) => false,
            Work::Save => false,
            Work::Stop => false,
            Work::Replace => true,
            Work::Seen { .. } => false,
            Work::Restart => true,
            Work::Probe => false,
            Work::Setup => false,
            Work::TunnelOpen => false,
            Work::TunnelClose => false,
            Work::Launch => true,
            Work::RemoveHost => false,
            Work::Answer => false,
            Work::UpdateHost => false,
            Work::SendFeedback => false,
            Work::TrashDelete => false,
            Work::UninstallHost => false,
            Work::TabOpen => false,
            Work::TabClose => false,
            Work::RetryHost => false,
            Work::AdoptHost => false,
            Work::TakeOverHost { .. } => false,
            Work::Pin => false,
            Work::Mute => false,
            Work::MovePin => false,
            Work::SessionGroup => false,
            Work::Suspend => false,
            Work::Resume => false,
            Work::StopAndSuspend => false,
            Work::EnvironmentDelete => false,
        }
    }

    /// Whether this work changes the host registry: what "host registry work in flight" means to the launcher, which
    /// refuses a launch while one is out. A probe registers a supervisor that answers, a setup
    /// registers its new host, and a Remove forgets one. A host's destination save is not on the list: launches do not
    /// refuse during it, though it refuses during a launch (decided by the migration's orchestrator, 2026-11-14). It is
    /// [`Work::Save`] anyway, one kind for every editor's save.
    /// A host's Update counts too: it replaces the host's installation, and the orchestrator's set names it.
    pub fn changes_host_registry(self) -> bool {
        match self {
            Work::Rename => false,
            Work::Delete(_) => false,
            Work::Save => false,
            Work::Stop => false,
            Work::Replace => false,
            Work::Seen { .. } => false,
            Work::Restart => false,
            Work::Probe => true,
            Work::Setup => true,
            Work::TunnelOpen => false,
            Work::TunnelClose => false,
            Work::Launch => false,
            Work::RemoveHost => true,
            Work::Answer => false,
            Work::UpdateHost => true,
            Work::SendFeedback => false,
            Work::TrashDelete => false,
            Work::UninstallHost => true,
            Work::TabOpen => false,
            Work::TabClose => false,
            Work::RetryHost => false,
            Work::AdoptHost => true,
            Work::TakeOverHost { registers } => registers,
            Work::Pin => false,
            Work::Mute => false,
            Work::MovePin => false,
            Work::SessionGroup => false,
            Work::Suspend => false,
            Work::Resume => false,
            Work::StopAndSuspend => false,
            Work::EnvironmentDelete => false,
        }
    }
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
    SessionGroup(i64),
    NewSessionGroup,
    Host(String),
    Template(String),
    Preferences,
    /// Numbered by [`next_number`], so no two launches share a key.
    Launch(u64),
    /// Numbered the same way.
    HostAdd(u64),
    /// A session's tunnels, by the session's id: apart from the session's own key, so opening or closing a tunnel never
    /// makes the session's lifecycle actions busy.
    Tunnels(String),
    /// An agent's approval request, by the id the helm gave it.
    Approval(String),
    /// The Send feedback dialog's one submission at a time ([`OpKey::feedback`]).
    Feedback,
    /// A host's trash, by the host's registry id: apart from the host's own key, so a trash delete never blocks the
    /// host's other operations (SPEC.md: "Trash deletes do not block other actions").
    Trash(String),
    /// A named credential of the helm's catalog, by its name, which the helm never changes after creation.
    Credential(String),
    /// One terminal tab of a session, by the session's id and the tab's: apart from the session's own key and from its
    /// sibling tabs', so a tab closes beside the others and never makes the session's lifecycle actions busy.
    Tab(String, String),
    /// A recipe of the helm's shared catalog, by its name, which the helm never changes after creation.
    Recipe(String),
}

impl OpKey {
    /// A session, by the id the helm serves it under.
    pub fn session(id: &str) -> OpKey {
        OpKey(Key::Session(id.to_owned()))
    }

    /// Serialize writes to one shared group without blocking another group.
    pub(crate) fn session_group(id: i64) -> OpKey {
        OpKey(Key::SessionGroup(id))
    }

    /// The New group dialog shares one key because only one can be open.
    pub(crate) fn new_session_group() -> OpKey {
        OpKey(Key::NewSessionGroup)
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

    /// A fresh key for a launch, sent before any session exists for it. Every call gives a key no other call in this
    /// run of the app gives, so a launch never shares its key with an earlier one still in flight (SPEC.md lets New
    /// and the shortcuts start a fresh launch at once) or with a stale key a view held across a reset. Keep the key
    /// for as long as the launch's busy state or refusal is shown.
    pub fn new_launch() -> OpKey {
        OpKey(Key::Launch(next_number()))
    }

    /// A fresh key for adding a host, sent before the host has a registry id: it gets one when the helm registers it,
    /// which is when it accepts the setup (202), or straight away when the probe finds the host already set up (200).
    /// Fresh in the same way as [`OpKey::new_launch`]'s.
    pub fn new_host_add() -> OpKey {
        OpKey(Key::HostAdd(next_number()))
    }

    /// The tunnels of a session, by the id the helm serves the session under: one key for all of them, so one tunnel
    /// action runs at a time per session ([`Work::TunnelOpen`], [`Work::TunnelClose`]). Distinct from
    /// [`OpKey::session`], whose busy state is the session's lifecycle actions'.
    pub fn tunnels(session: &str) -> OpKey {
        OpKey(Key::Tunnels(session.to_owned()))
    }

    /// An agent's approval request, by the id the helm gave it: one answer at a time per request, and its refusal kept
    /// for the request's card to show.
    pub fn approval(id: &str) -> OpKey {
        OpKey(Key::Approval(id.to_owned()))
    }

    /// The one feedback send: the Send feedback dialog's submission (`crate::feedback`). One key for every send,
    /// because the dialog is modal and stays open while its send is out, so there is never a second one to keep apart.
    pub fn feedback() -> OpKey {
        OpKey(Key::Feedback)
    }
    /// The trash of a host, by its registry id: one key per host, so one trash delete runs at a time per host
    /// ([`Work::TrashDelete`]). Distinct from [`OpKey::host`], whose busy state is the host's registry operations'.
    pub fn trash(host: &str) -> OpKey {
        OpKey(Key::Trash(host.to_owned()))
    }
    /// A named credential of the helm's catalog, by its name: the key names the resource a save acts on (its creation,
    /// the replacement of its secret, its deletion), one at a time, as a host's or a template's key does. A credential
    /// not created yet takes the key of the name it is being created under, which the helm keeps unique across kinds.
    pub fn credential(name: &str) -> OpKey {
        OpKey(Key::Credential(name.to_owned()))
    }

    /// One terminal tab of a session, by the id the helm serves the session under and the tab's id
    /// ([`Work::TabClose`]).
    pub fn tab(session: &str, tab: &str) -> OpKey {
        OpKey(Key::Tab(session.to_owned(), tab.to_owned()))
    }

    /// A recipe of the helm's shared catalog, by its name: the key names the recipe the work acts on (a snapshot build
    /// asked from its row; later its editor's saves and Delete), one request at a time, as a credential's key does, and
    /// nothing on it is supersedable. A recipe not created yet takes the key of the name it is being created under,
    /// which the helm keeps unique.
    ///
    /// The key is shared, and so is its one refusal record. Settings → Recipes' build uses it now and draws any refusal
    /// recorded on it as the last build's, under the recipe's row; the editor's saves and Delete will share it. So the
    /// editor must forget each refusal it shows itself ([`Operations::forget`]), or a refused save would show under the
    /// row as a build refusal once the editor closes.
    ///
    /// Forgetting does not cover every refusal. One that lands after the user left the editor opens nothing, so by
    /// [`Operations::forget`]'s rule it stays on the key, and the row draws it under the build button; the first push
    /// of the catalog that shows the recipe building then drops whatever refusal the key holds (`settings/recipes.rs`,
    /// `settle_refusals`). The editor decides knowingly whether that is right for its refusals, or tells them apart by
    /// [`Failure::work`] after giving builds a kind of work of their own.
    ///
    /// The rule above, made precise: each Rebuild at one of the recipe's container places shares the key and its
    /// refusal record with the snapshot build, so the row draws a refused Rebuild in the same place, and a refusal is
    /// dropped by the first push that shows one of the recipe's sources start building, its snapshot or one of its
    /// container places, not by every push that shows it building: a refusal that lands while a source already builds
    /// stays while that source goes on.
    pub fn recipe(name: &str) -> OpKey {
        OpKey(Key::Recipe(name.to_owned()))
    }
}

/// The next number for a fresh key. One counter for the whole app rather than one per UI session's store: a view can
/// hold a key across a reset, and a number restarted with the new store would hand that view's key to someone else's
/// work. Relaxed ordering is enough, since all that matters is that no two calls get the same number.
fn next_number() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    NEXT.fetch_add(1, Ordering::Relaxed)
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
    /// A route that answers 202 (accepted long work, such as host setup or a confirmed Update) settles as
    /// [`Outcome::Accepted`], never as done: its result arrives later in the helm's pushed state.
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
    /// caused). The body is the route's own reply (the renamed session, a Delete's outcome with its notice). Every 2xx
    /// but 202.
    Done(Option<Value>),
    /// The helm accepted long work it carries on with (202): the key is no longer busy and nothing failed, but success
    /// is not implied, and no per-operation result is kept here. For host setup, a confirmed Update and a confirmed
    /// Uninstall, the progress and result arrive in the hosts' pushed progress, which is where a feature shows them;
    /// a route whose result is not pushed (a recipe build) needs its own way to learn it. The body is the route's own
    /// reply (`{host_id, run_id}` for the host routes).
    Accepted(Option<Value>),
    /// The helm refused: every reply that is not a 2xx. A host timeout is a refusal too (504), though the work goes on
    /// behind it, which a feature tells from the status ([`Refusal`]).
    Refused(Refusal),
}

/// A refusal as its completion receives it: the words to show, and what a feature branches on beside them.
///
/// The words alone used to be all a refusal carried, which left a feature that must answer a refusal (ask the YOLO
/// question, wait for a busy host, keep a launch's draft after a timeout) matching prose that the helm writes for
/// people and that can quote a supervisor verbatim. The status and markers are the reply's own, unchanged; nothing
/// here interprets them.
#[derive(Clone, Debug, PartialEq)]
pub struct Refusal {
    /// The helm's own words, which may quote host values (a supervisor's refusal names the title it refused), so they
    /// are host text; or, when it sent none, which command got which status ([`Outcome`]'s classification).
    pub words: HostText,
    /// The reply's HTTP status: never a 2xx.
    pub status: u16,
    /// The helm-only markers the reply carried, in [`HelmMarker::ALL`]'s order: facts only the helm's own code
    /// states, never a host's text. Usually empty.
    pub markers: Vec<HelmMarker>,
}

impl Refusal {
    /// Whether the reply carried `marker`.
    pub fn has(&self, marker: HelmMarker) -> bool {
        self.markers.contains(&marker)
    }
}

impl Outcome {
    /// Classify a reply to the command shown as `shown`. A refusal with no words says which command got which status,
    /// as the old app's `api::refusal_text` did. Those words are the app's own and name no host value, but they still
    /// travel as host text: a refusal is one type wherever its words came from, and escaping the app's words changes
    /// nothing.
    fn from_reply(reply: Reply, shown: &str) -> Outcome {
        match reply.status {
            202 => Outcome::Accepted(reply.body),
            200..=299 => Outcome::Done(reply.body),
            status => Outcome::Refused(Refusal {
                words: refusal_words(reply.body, status, shown),
                status,
                markers: reply.markers,
            }),
        }
    }
}

/// The words of a refusal: the helm's own (a plain string, trimmed, or the JSON it sent), or, when it sent none, which
/// command got which status, as the old app's `api::refusal_text` did. One wording for operations and for commands
/// without a key ([`send`], [`read`]) alike.
fn refusal_words(body: Option<Value>, status: u16, shown: &str) -> HostText {
    let words = match body {
        Some(Value::String(text)) => text.trim().to_owned(),
        Some(other) => other.to_string(),
        None => String::new(),
    };
    if words.is_empty() {
        HostText::from_host(format!("{shown}: {status}"))
    } else {
        HostText::from_host(words)
    }
}

// ---- Commands without a key: writes nobody shows, and reads ----------------------------------------------------------

/// Send `method` on `path` with `body` through the live UI session, as no operation, and hand `then` the body of the
/// helm's answer, or why there is none, worded for the user. Returns whether it was sent: `false` while there is no UI
/// session (before the first connect, or between a reset and the next), and then `then` never runs.
///
/// For a command that is not work on a key: nothing draws it busy, nothing competing is refused while it runs, and
/// the store records no refusal for it (a remembered preference, the automatic seen mark, the host rail's session
/// query). The caller decides what, if anything, to show from `then`: one write only logs a failure, the rail shows
/// its refusal, the sidebar's width write ends the hold that keeps its width on screen. Work that needs a busy state
/// or a refusal kept on its subject until the next try is an operation on a key instead ([`Operations::start`]), and
/// a read of what the helm does not push is [`read`].
///
/// `shown` names the command in a refusal without words, as [`Command::new`]'s does (`PUT /api/preferences`, with any
/// internal id hidden). A refusal is worded exactly as an operation's ([`Outcome::Refused`]). Every 2xx counts as an
/// answer, a 202 included, since nothing here waits for long work's result, and the answer is the reply's body as sent
/// (`None` for a reply without one). A command the channel lost says so, though that cannot arrive in practice: the
/// reset that follows drops the wait first.
///
/// `then` runs on its own, after the UI session's update that received the reply has returned, so it may read the
/// store, start an operation or send again; called inside that update, any of those would update or read the UI
/// session again and panic. It is told nothing about whether the user moved on: a caller that may move focus or open
/// something takes its own [`Mark`] when it sends and asks [`focus::has_moved_on`] when the answer comes.
///
/// The wait belongs to the UI session: a reset retires it and drops the wait, so `then` never runs for a command sent
/// before a reset. Nothing holds the caller: `then` should hold its view weakly, so that a reply after the view closed
/// reaches nobody.
pub fn send(
    method: Method,
    path: ApiPath,
    body: Option<Value>,
    shown: impl Into<String>,
    cx: &mut App,
    then: impl FnOnce(Result<Option<Value>, HostText>, &mut App) + 'static,
) -> bool {
    let Some(current) = crate::session::connection(cx).and_then(|c| c.read(cx).session().cloned())
    else {
        return false;
    };
    let shown = shown.into();
    current.update(cx, |current, cx| {
        current.command(method, path, body, cx, move |_, result, cx| {
            let answered = answer(result, &shown);
            cx.defer(move |cx| then(answered, cx));
        });
    });
    true
}

/// What the reply to the command shown as `shown` says, as [`send`] hands it on: the body of any 2xx, or the refusal
/// in the store's words ([`refusal_words`]). Split from [`send`] so the wording is tested without a channel.
///
/// Words only, unlike an operation's [`Refusal`]: what fails here is not always a reply (a lost command, and for
/// [`read`] a body that does not decode, have no status, so a status here would be made up), and the refusals a
/// feature answers rather than shows (such as a YOLO launch to confirm, a busy host, an environment Delete nobody could
/// ask, an uninstall plan to replace with removal, a stale connection, a create that never started) are refusals of
/// work on a key, which settles through
/// [`Operations::start`]. A keyless command whose caller must branch on a status or a marker is a change here.
pub(crate) fn answer(
    result: Result<Reply, CommandError>,
    shown: &str,
) -> Result<Option<Value>, HostText> {
    let reply = result.map_err(|error| HostText::from_host(error.to_string()))?;
    if (200..300).contains(&reply.status) {
        Ok(reply.body)
    } else {
        Err(refusal_words(reply.body, reply.status, shown))
    }
}

/// Read `path` with a plain GET through the live UI session, and hand `on_read` the reply decoded as `T`, or why there
/// is none, worded for the user. A [`send`] of the GET plus a decode, so everything [`send`] says about when it sends,
/// when its callback runs, a reset and a refusal's words holds here too. Returns whether it was sent.
///
/// Only for what the helm does not push. Pushed state (sessions, hosts, templates, the preferences) is read from the
/// store while drawing, never fetched: a fetched copy would go stale the moment the next push lands.
///
/// `shown` names the read in a refusal without words (`GET /api/sessions`, with any internal id hidden). A reply that
/// does not decode as `T`, or has no body, says so, naming `shown`.
pub fn read<T: DeserializeOwned + 'static>(
    path: ApiPath,
    shown: impl Into<String>,
    cx: &mut App,
    on_read: impl FnOnce(Result<T, HostText>, &mut App) + 'static,
) -> bool {
    let shown = shown.into();
    let named = shown.clone();
    send(Method::Get, path, None, shown, cx, move |answer, cx| {
        on_read(decode(answer, &named), cx)
    })
}

/// What [`send`]'s answer to the read shown as `shown` says, decoded as `T`: a refusal passes through as it is, and a
/// missing or misshapen body says so. Split from [`read`] so the wording is tested without a channel.
pub(crate) fn decode<T: DeserializeOwned>(
    answer: Result<Option<Value>, HostText>,
    shown: &str,
) -> Result<T, HostText> {
    match answer?.map(serde_json::from_value::<T>) {
        Some(Ok(value)) => Ok(value),
        Some(Err(error)) => Err(HostText::from_host(format!(
            "the helm's answer to {shown} could not be read: {error}"
        ))),
        None => Err(HostText::from_host(format!(
            "the helm sent no answer to {shown}"
        ))),
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

/// A refusal, or a partial result ([`Operations::note`]), recorded on the key it was for, for whatever shows that
/// subject (a session's row, an editor) to show.
#[derive(Clone, Debug, PartialEq)]
pub struct Failure {
    pub work: Work,
    pub message: HostText,
}

/// Why [`Operations::start`] sent nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotStarted {
    /// The key already has this work in flight. Nothing is queued: the user acts again once it settles. A view that
    /// says so gives [`NotStarted::BUSY_REASON`] after its own lead-in.
    Busy(Work),
    /// A reset retired this store, or the window is inert and there is none.
    Retired,
    /// An id in the command's route cannot be a path segment (an empty session id, say). The helm never serves one,
    /// so this is a bug report, not a user-facing state.
    Path(PathError),
}

impl NotStarted {
    /// Why a press refused with [`NotStarted::Busy`] did nothing, wherever its key's subject is shown: the store's one
    /// wording for "another request on this key was still out", as [`Refusal::words`] is its one wording for the helm's
    /// refusals. An area shows it after a lead-in of its own that names what did not happen, such as "Not saved:" for a
    /// save or "Not started:" for a start ("Not started: another operation was in progress. Try again."), rather than
    /// wording its own reason. The store cannot supply the lead-in: [`NotStarted::Busy`] carries the work holding the
    /// key, not the one refused, and only the feature knows what the press was for.
    ///
    /// The shared reason is for a busy line that has no old-app wording to port. A line that ports the old app's words
    /// verbatim keeps them, since the migration ports the old app rather than rewording it: the terminal's "Another
    /// tunnel action is still waiting", for one, stays as the old app had it. The migration's orchestrator decided this
    /// on 2026-11-14.
    ///
    /// Ported from the old app's session row, whose exclusion is the closest to the store's rule (one request per key):
    /// "Stop: not started, another operation is in progress; try again" (`list/lifecycle.rs` at reference commit
    /// `24bc0580`), which also led with what the press was for. The reason is in the past tense because the line stays
    /// shown after the key frees (until something in the view clears or replaces it), where "is in progress" would go
    /// stale; the GPUI Settings line chose the same tense for the same reason. The split into an area's lead-in and
    /// this shared reason was decided by the migration's orchestrator on 2026-11-14, so that a refused save never reads
    /// as a start that did not happen.
    ///
    /// A plain string, not [`HostText`]: the words are the app's own and quote nothing a host wrote.
    pub const BUSY_REASON: &str = "another operation was in progress. Try again.";
}

/// One operation in flight: what it is, and the task that waits for its reply. Dropping the entry drops the task, and
/// with it the completion, but never the helm's work.
struct InFlight {
    work: Work,
    /// The task waiting for the request's reply, or `None` for a hold ([`Operations::hold`]): a key held for work
    /// that is not sent yet, with nothing to wait for. Read through [`InFlight::is_held`].
    _task: Option<Task<()>>,
}

impl InFlight {
    /// Whether this entry is a hold rather than a request on the wire: only a hold may be released
    /// ([`Operations::release`]), so a release can never drop a live request's entry.
    fn is_held(&self) -> bool {
        self._task.is_none()
    }
}

/// The operation store of one UI session: the work in flight on each key, and the last refusal or partial result of
/// each.
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

    /// The last refusal or partial result of work on `key`, until the next operation on it starts or a feature forgets
    /// it ([`Operations::forget`]). A key whose subject is gone (a deleted session) keeps its entry until the UI
    /// session resets; nothing draws it, since nothing asks.
    pub fn failure(&self, key: &OpKey) -> Option<&Failure> {
        self.failures.get(key)
    }

    /// Send `command` on `key`, as the work it names ([`Command::work`]), and call `on_settled` once the helm
    /// answers, unless a reset retires the store first. Call it from the handler of the user's action: that is when the
    /// moved-on mark is taken.
    ///
    /// Starting clears the key's previous failure (the old app's rule: a row's error stays until the retry that
    /// supersedes it); short of a reset, the only other ways it goes are a hold of the key ([`Operations::hold`]),
    /// which starts the key's new work as far as its views go, and [`Operations::forget`], from a feature that showed
    /// it elsewhere.
    ///
    /// `on_settled` runs after the store has recorded the outcome and notified, outside the store's update, so it may
    /// read the store. It is never called for a command the reset lost.
    pub fn start(
        &mut self,
        key: &OpKey,
        command: Command,
        cx: &mut Context<Operations>,
        on_settled: impl FnOnce(&Settled, &mut App) + 'static,
    ) -> Result<(), NotStarted> {
        self.admit(key)?;
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
        self.in_flight.insert(
            key.clone(),
            InFlight {
                work,
                _task: Some(task),
            },
        );
        cx.notify();
        Ok(())
    }

    /// The store's one admission rule, for a start ([`Operations::start`]) and a hold ([`Operations::hold`]) alike:
    /// refused on a retired store, and while the key has work in flight or held; otherwise the key's last failure goes,
    /// since its new work supersedes it. A future "latest wins" rule for supersedable work belongs here, so it covers
    /// holds too.
    fn admit(&mut self, key: &OpKey) -> Result<(), NotStarted> {
        if self.retired {
            return Err(NotStarted::Retired);
        }
        if let Some(running) = self.in_flight.get(key) {
            return Err(NotStarted::Busy(running.work));
        }
        self.failures.remove(key);
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
        if let Outcome::Refused(refusal) = &outcome {
            self.failures.insert(
                key.clone(),
                Failure {
                    work,
                    message: refusal.words.clone(),
                },
            );
        }
        Some(Settled {
            outcome,
            user_moved_on: focus::has_moved_on(mark, cx),
        })
    }

    /// Record a partial result of `work` on `key` in the place a refusal goes ([`Operations::failure`]), with the same
    /// lifetime: the key's next [`Operations::start`], a [`Operations::forget`] or a reset clears it. For work the helm
    /// accepted that still left something for the user to read: a Stop whose host could not close a tab, a Replace
    /// whose removal of its source reported something. It replaces whatever the key held. A retired store records
    /// nothing.
    ///
    /// Call it from the completion of the work it reports: a note recorded while newer work runs on the key would
    /// outlive that work, since a successful finish does not clear the slot.
    pub fn note(
        &mut self,
        key: &OpKey,
        work: Work,
        message: HostText,
        cx: &mut Context<Operations>,
    ) {
        if self.retired {
            return;
        }
        self.failures.insert(key.clone(), Failure { work, message });
        cx.notify();
    }

    /// Drop the refusal or partial result recorded on `key`, for a feature that has already shown it somewhere else,
    /// such as a dialog, so that it does not also stay where the store's record is drawn (a host's row, say) and
    /// report the one refusal twice. Notifies only when there was a record to drop: forgetting a key that has none,
    /// which includes every key of a retired store, changes nothing and redraws nothing.
    ///
    /// Forgetting is never needed for cleanup, since the key's next [`Operations::start`] clears its failure anyway.
    /// Forget only a refusal the feature did show. One that arrived after the user moved on
    /// ([`Settled::user_moved_on`]) opened no dialog, so the record is the one place it is shown and must stay. Call it
    /// from the completion that showed the refusal, while the record is still that refusal's: called later, it could
    /// drop the refusal of newer work on the key, which nothing else would show.
    ///
    /// Nothing in flight changes: work out on `key` keeps its busy state and its completion, and a refusal it settles
    /// with is recorded as usual.
    pub fn forget(&mut self, key: &OpKey, cx: &mut Context<Operations>) {
        if self.failures.remove(key).is_some() {
            cx.notify();
        }
    }

    /// Hold `key` for `work` that the user accepted but that is not sent yet: a bulk Stop or Delete row queued behind
    /// the other rows of its host (`session_list::bulk`). SPEC.md, "Session list": "Accepted queued rows remain pending
    /// until their completions finish", and the old app reserved every admitted row at acceptance. While held, the key
    /// reads as `work` in flight ([`Operations::in_flight`], [`Operations::any_in_flight`]), so every view draws it
    /// pending and every other start on it is refused, but nothing is sent and nothing settles. Holding clears the
    /// key's last failure, as a start does.
    ///
    /// Refused, holding nothing, whenever a start would be (the store's one admission rule, `admit`): while the key
    /// already has work in flight or held, or the store is retired. The holder ends the hold with
    /// [`Operations::release`] right before it starts the request it held the key for; otherwise only a reset ends it,
    /// with everything else, so a hold never outlives its UI session.
    pub fn hold(
        &mut self,
        key: &OpKey,
        work: Work,
        cx: &mut Context<Operations>,
    ) -> Result<(), NotStarted> {
        self.admit(key)?;
        self.in_flight
            .insert(key.clone(), InFlight { work, _task: None });
        cx.notify();
        Ok(())
    }

    /// End the hold [`Operations::hold`] put on `key`. Only a hold is ever released: a request started on `key` (once
    /// the hold has given way to it, or any other) stays in flight with its completion, so a late or repeated release
    /// cannot drop live work. Does nothing when `key` is not held.
    pub fn release(&mut self, key: &OpKey, cx: &mut Context<Operations>) {
        if self.in_flight.get(key).is_some_and(InFlight::is_held) {
            self.in_flight.remove(key);
            cx.notify();
        }
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
