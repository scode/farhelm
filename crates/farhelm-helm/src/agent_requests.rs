//! The helm's answers to questions asked by an agent from inside a
//! session — the top of the relay whose supervisor half lives in
//! `farhelm-supervisor`'s `service::agent_relay`.
//!
//! # Why the question arrives here at all
//!
//! An agent's mental model is that it is talking to the HELM: the process
//! that knows the whole fleet and the one the user is looking at. It
//! cannot talk to it directly — the host it runs on has no route, address,
//! or credential back to the machine running the helm, and every
//! connection in this system is dialled by the helm outward. So the
//! request comes the only way it can: the agent hands it to its own
//! supervisor over the per-session credential, and the supervisor sends it
//! back UP the control connection the helm opened, as
//! [`ControlMsg::AgentRequest`]. This module is what answers it.
//!
//! # Same answers as the UI, on purpose
//!
//! Every verb here is served from the exact code path its REST counterpart
//! uses — `hosts::host_views` for hosts, `aggregate::session_page` for
//! sessions, and `sessions::do_rename_session`/`do_stop_session`/
//! `do_archive_session` for the three lifecycle verbs. Not for economy: the
//! point of routing an agent's questions (and now its actions) through the
//! helm at all is that the agent and the user see, and act on, one fleet.
//! Two listings assembled two ways would drift, and the drift would show
//! up as an agent confidently naming a host that is not in the panel the
//! user is reading; two rename implementations would drift on exactly the
//! validation rule that matters (SPEC.md's control-character refusal).
//!
//! # Lifecycle verbs act on ANY session, not only the asker's own
//!
//! `Rename`, `Stop` and `Archive` each carry `session_id: Option<String>`,
//! and `None` resolves to the ASKING session — the one the supervisor has
//! already proven this connection's credential belongs to. A `Some(id)`
//! names any OTHER session the helm knows, on any host, and that is
//! intentional: the feature's mental model is an agent talking to the helm
//! itself, which already has fleet-wide authority, and inventing a
//! narrower per-session permission for agents alone would be a second
//! authorization model with nothing else in this system to keep it
//! honest. What IS worth a paper trail is which session asked to act on
//! which — logged at `info` by [`resolve_target`], the one place all three
//! verbs resolve the substitution — so an operator reading the helm's log
//! can tell an agent renaming itself apart from one reaching across the
//! fleet.
//!
//! # What `current` means, and why only this side can compute it
//!
//! Neither endpoint of the relay can work out on its own which host the
//! asking session is on. The agent does not know (its host has no name it
//! has ever been told), and the supervisor knows only itself, not its
//! registry id here. The helm knows because of WHERE the upcall arrived:
//! it came up one host actor's connection, and that host is by
//! construction the asking session's host. That id is threaded through
//! [`AgentRequestHandler::handle`] as part of [`AgentOrigin`] for exactly
//! this.
//!
//! # What this side does NOT verify, and why that is deliberate
//!
//! The `session_id` on an upcall, and the claim that the connection it
//! arrived on is that session's host, are both taken on trust. The helm
//! never sees the per-session credential — only the supervisor can check
//! it, and it does, before forwarding — so there is nothing here to
//! re-verify against. That is sound because of what a full-authority
//! supervisor connection already is: the supervisor on the far end is the
//! helm's own provisioned install, holding complete authority over every
//! session on its host, and a helm that could not trust it could not route
//! a single operation to it either. The trust boundary is the connection,
//! not the message.

use std::sync::{Arc, Weak};

use async_trait::async_trait;
use farhelm_proto::{
    AgentHost, AgentOutcome, AgentReply, AgentSession, AgentVerb, ErrorKind, SessionStatus,
};
use tracing::info;

use crate::AppState;
use crate::store::HostId;

/// Which connection an upcall arrived on — the whole of what the helm
/// knows about who is asking, beyond the session id the supervisor
/// forwarded.
///
/// The pair is carried together because neither half answers the question
/// alone. `host` says which registry row this is about, which is what
/// makes `current` computable. `connection` says WHICH connection to that
/// row, which is what keeps the answer honest across a retarget: a
/// registry row's id survives having its machine swapped out from under
/// it, so a request forwarded by a connection that has since been replaced
/// would otherwise be attributed to the row's new occupant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AgentOrigin {
    /// The registry id of the host whose connection carried this request.
    pub host: HostId,
    /// That connection's identity — [`crate::SupervisorClient::connection_id`].
    pub connection: u64,
}

/// The helm-side answer to one agent request.
///
/// A trait rather than a concrete call so that `client` — which owns the
/// connection and the demultiplexer — has no dependency on the listing
/// modules, and so that a test can drive the whole relay end to end with a
/// handler it controls. The single production implementation is
/// [`HelmAgentRequests`].
///
/// Implementations must not panic and must answer every call: the
/// supervisor is holding an agent's `farhelm` process open until one
/// arrives or its budget expires, so a handler that returns nothing costs
/// a user a timeout rather than an error.
#[async_trait]
pub trait AgentRequestHandler: Send + Sync {
    /// Answer one verb on behalf of `session_id`.
    ///
    /// `origin` names the connection this request arrived on — the asking
    /// session's own host, which is the only way `current` can be answered
    /// (see the module docs). `session_id` has already been proven by the
    /// supervisor to be the session that authenticated; nothing here
    /// re-checks it, and nothing here may treat it as authority for
    /// anything beyond marking its own row.
    async fn handle(&self, origin: AgentOrigin, session_id: &str, verb: AgentVerb) -> AgentOutcome;

    /// Whether `origin`'s connection is STILL the one its host is served
    /// by, asked again once an answer is ready.
    ///
    /// Separate from [`Self::handle`], and synchronous, because of when it
    /// is called: the client asks it one step before it queues a successful
    /// answer (see `SupervisorClient::spawn_agent_answer`), to close the
    /// window between the entry check and the reply. The listing in between
    /// awaits on the database and the manager, and a host retargeted,
    /// adopted or reconnected in that window has a registry row whose
    /// machine has changed — so the answer's `current` marker would name a
    /// host that is no longer the asking session's.
    ///
    /// Defaults to `true` for the test doubles that have no fleet behind
    /// them: a handler that cannot tell a stale connection from a live one
    /// has nothing useful to say here, and refusing by default would make
    /// every such handler answer nothing at all.
    fn origin_is_live(&self, origin: AgentOrigin) -> bool {
        let _ = origin;
        true
    }
}

/// Where a connection finds the handler to use.
///
/// Filled ONCE at startup and read at request time, which is what closes a
/// window that a plain injected value would leave open: the connection
/// manager starts dialling hosts before `AppState` exists, so a handler
/// captured when a connection was opened would be permanently absent on
/// every connection that raced startup. A shared cell read per request has
/// no such window — the worst case is a request in the first moments of
/// process life being answered "not ready", instead of a host that can
/// never answer for the rest of the run.
pub type AgentRequestSlot = Arc<std::sync::OnceLock<Arc<dyn AgentRequestHandler>>>;

/// The production handler: the helm's own listings, projected down to what
/// an agent can name and act on.
///
/// Holds a `Weak`, deliberately. `AppState` owns the connection manager,
/// the manager hands every connection a clone of the slot this handler
/// sits in, so a strong handle here would close a cycle
/// (state → manager → slot → handler → state) that nothing would ever
/// break. A failed upgrade means the helm is shutting down, which is a
/// refusal rather than a panic: the connection may outlive the state by
/// moments, and an agent deserves a sentence rather than a dead socket.
pub(crate) struct HelmAgentRequests {
    state: Weak<AppState>,
}

impl HelmAgentRequests {
    /// Build the production handler for `state`, already erased to the
    /// trait object the slot holds.
    ///
    /// Returns the erased form rather than `Self` because there is exactly
    /// one caller and it needs the trait object; handing back the concrete
    /// type would only make every call site write the same `Arc::new` and
    /// coercion.
    pub(crate) fn for_state(state: &Arc<AppState>) -> Arc<dyn AgentRequestHandler> {
        Arc::new(HelmAgentRequests {
            state: Arc::downgrade(state),
        })
    }
}

/// The most sessions one `sessions` reply will ever carry.
///
/// A cut has to exist — the reply is one frame on a connection shared with
/// every terminal on that host, and an unbounded listing would eventually
/// be an unsendable one — and five thousand rows is already far past what
/// an agent can do anything with. What makes the cut acceptable is that it
/// is VISIBLE: reaching it sets `AgentReply::Sessions::truncated`, so the
/// difference between "that session does not exist" and "you were not
/// shown all of them" is on the wire rather than left for the reader to
/// guess. The verbs that would make a narrower answer right (filtering,
/// paging) do not exist yet; when they arrive the honest shape is a
/// parameter on the verb, not a smaller silent cut here.
///
/// It is the SECOND of two ceilings, and the weaker one: rows say nothing
/// about size, and [`AGENT_REPLY_BYTE_BUDGET`] is what actually keeps the
/// reply sendable.
const AGENT_SESSION_CAP: usize = 5_000;

/// The most ENCODED session bytes one `sessions` reply will ever carry,
/// accumulated across every page the listing walks.
///
/// The row cap alone was never a bound on the answer's size, and the gap
/// was not theoretical. `aggregate::session_page` applies its own byte
/// budget PER PAGE, so a fleet of legally fat records — session creation
/// admits tens of kilobytes of caller-supplied title, cwd and invocation
/// text — produces page after individually-valid page that this loop
/// concatenated into a reply no frame could carry. The whole answer was
/// then discarded at the writer's size backstop and the agent got
/// `Internal` instead of the partial listing with `truncated: true` that
/// the verb promises, after the helm had already paid to build and encode
/// it (up to `client::AGENT_ANSWER_SLOTS` times over per host).
///
/// Six MiB of ROWS against `MAX_FRAME_LEN`'s eight leaves the reply's
/// envelope — the `AgentResponse` wrapper, the `req_id`, the JSON
/// punctuation between rows — about two MiB of headroom, which is orders
/// of magnitude more than that envelope can be. Deliberately generous
/// rather than tight: this cut is meant to be unreachable by any real
/// fleet, and the size backstop in `client::agent_response_frame` is what
/// catches an arithmetic mistake here, so slack costs nothing while a
/// miscalculated ceiling would cost the whole answer.
///
/// Reaching it means the same thing reaching the row cap means, and says
/// so the same way: `truncated: true`, and no further page is fetched.
const AGENT_REPLY_BYTE_BUDGET: usize = 6 * 1024 * 1024;

#[async_trait]
impl AgentRequestHandler for HelmAgentRequests {
    async fn handle(&self, origin: AgentOrigin, session_id: &str, verb: AgentVerb) -> AgentOutcome {
        let Some(state) = self.state.upgrade() else {
            return AgentOutcome::Err {
                kind: ErrorKind::Unavailable,
                message: "the helm is shutting down; retry once it is back".to_string(),
            };
        };
        // Refused BEFORE any listing work, because the whole answer depends
        // on the origin: `current` is computed from the host id, and a host
        // id from a superseded connection names a registry row whose
        // machine may since have been replaced. Answering anyway would mark
        // an unrelated machine as the asking session's own.
        if !origin_is_live(&state, origin) {
            return AgentOutcome::Err {
                kind: ErrorKind::Unavailable,
                message: "the host connection was replaced; retry".to_string(),
            };
        }
        let reply = match verb {
            AgentVerb::Hosts {} => {
                crate::hosts::host_views(&state)
                    .await
                    .map(|views| AgentReply::Hosts {
                        hosts: views
                            .iter()
                            .map(|view| agent_host(view, origin.host))
                            .collect(),
                    })
            }
            AgentVerb::Sessions {} => session_listing(&state, origin.host, session_id).await,
            AgentVerb::Rename {
                session_id: target,
                title,
            } => {
                let target = resolve_target(target, session_id, "rename");
                crate::sessions::do_rename_session(&state, &target, &title)
                    .await
                    .map(|(claim, info)| {
                        agent_session_reply(&state, claim.host, info, origin.host, session_id)
                    })
            }
            AgentVerb::Stop { session_id: target } => {
                let target = resolve_target(target, session_id, "stop");
                crate::sessions::do_stop_session(&state, &target)
                    .await
                    .map(|()| AgentReply::Stopped {})
            }
            AgentVerb::Archive { session_id: target } => {
                let target = resolve_target(target, session_id, "archive");
                crate::sessions::do_archive_session(&state, &target)
                    .await
                    .map(|(claim, info)| {
                        agent_session_reply(&state, claim.host, info, origin.host, session_id)
                    })
            }
        };
        match reply {
            Ok(reply) => AgentOutcome::Ok { reply },
            // Classified the same way the REST surface classifies the SAME
            // failures (`crate::error_kind`), rather than flattened to
            // `Internal`: a lifecycle verb's refusal — an unknown session, a
            // rejected title, a non-connected host — is exactly the kind of
            // thing a caller can act on differently, and an agent deserves
            // the same distinction a browser gets. The two read-only verbs
            // above rarely produce a classifiable error at all (a listing
            // failure has nothing upstream to classify against), so this
            // arm falls back to `Internal` for them exactly as before.
            Err(error) => AgentOutcome::Err {
                kind: crate::error_kind(&error),
                message: format!("{error:#}"),
            },
        }
    }

    /// The same question `handle` asks on the way in, asked again for the
    /// caller that is about to put an answer on the wire.
    ///
    /// A failed upgrade answers `false`: a helm that is shutting down has
    /// no published client for anything, so there is no connection this
    /// answer could still be current for.
    fn origin_is_live(&self, origin: AgentOrigin) -> bool {
        self.state
            .upgrade()
            .is_some_and(|state| origin_is_live(&state, origin))
    }
}

/// Whether `origin`'s connection is still the one this host row is served
/// by.
///
/// The manager mints a fresh incarnation token every time a row's client
/// changes, but it does so when the connection is PUBLISHED — after the
/// connection itself exists — so a connection cannot capture its own token
/// on the way up. The client's own id is the same fact from the other
/// side: it is minted with the connection, it is never reused, and the
/// manager publishes exactly one client per incarnation, so "the published
/// client is the one that asked" and "the incarnation still matches" are
/// the same question.
///
/// A row with no actor, or one that is not currently connected, fails this
/// too — which is right: there is then no live connection this request
/// could have come from, so whatever forwarded it is a corpse.
fn origin_is_live(state: &AppState, origin: AgentOrigin) -> bool {
    state
        .manager
        .status(origin.host)
        .and_then(|status| status.client)
        .is_some_and(|client| client.connection_id() == origin.connection)
}

/// The session ONE lifecycle verb acts on: `target` if the verb named one,
/// else `asking` — the substitution [`AgentVerb`]'s own docs promise for
/// `Rename`/`Stop`/`Archive`.
///
/// Also where "which session asked to act on which" is logged, at `info`
/// rather than left to be reconstructed from a `RenameSession`/
/// `StopSession`/`ArchiveSession` line on whatever supervisor eventually
/// answers: an operator reading the HELM's own log wants to see, in one
/// place, that a session reached across the fleet (or renamed itself)
/// before the request ever leaves this process — see the module's own docs
/// for why no narrower authorization check accompanies it.
fn resolve_target(target: Option<String>, asking: &str, verb: &str) -> String {
    let target = target.unwrap_or_else(|| asking.to_string());
    info!(
        asking,
        target = target.as_str(),
        verb,
        "an agent is acting on a session"
    );
    target
}

/// Project a session a lifecycle verb just mutated into the same
/// [`AgentSession`] shape the `sessions` listing uses, so a rename/archive
/// reply and a later listing agree about the row they both describe.
///
/// Built from the [`crate::manager::SessionClaim`] `route_session` already
/// resolved while performing the mutation, rather than by re-listing the
/// fleet: the mutation's own reply already carries the fresh `SessionInfo`,
/// and asking again would be an extra round trip to relearn what the
/// supervisor just said. `stale` is unconditionally `false` — this row was
/// produced by a request this call just sent over a connection
/// `route_session` proved live a moment ago — which is the one field a
/// fresh listing could not improve on either.
fn agent_session_reply(
    state: &AppState,
    host: HostId,
    info: farhelm_proto::SessionInfo,
    asking_host: HostId,
    asking_session: &str,
) -> AgentReply {
    let host_name = state
        .manager
        .snapshots()
        .into_iter()
        .find(|snapshot| snapshot.id == host)
        .map(|snapshot| {
            crate::aggregate::host_display_name(snapshot.kind, snapshot.destination.as_deref())
        })
        .unwrap_or_default();
    let row = crate::aggregate::SessionRow {
        info,
        host,
        host_identity: None,
        host_name,
        stale: false,
    };
    AgentReply::Session {
        session: agent_session(&row, asking_host, asking_session),
    }
}

/// Drain the merged fleet listing into one reply, up to whichever of
/// [`AGENT_SESSION_CAP`] and [`AGENT_REPLY_BYTE_BUDGET`] is reached first.
///
/// Served from `aggregate::session_page` — the same function the UI's list
/// is built from, on purpose (see the module docs) — which is PAGINATED:
/// it stops at its own row limit, and stops earlier when the page's
/// encoded-size budget is spent, reporting either cut through
/// `next_cursor`. A single call was the original shape and was wrong for
/// exactly that reason: an agent asking for the fleet got whatever fitted
/// in one page, with no way to tell that from the whole answer. So this
/// walks the cursor until the listing ends or a ceiling is reached, and
/// says which of the two happened.
///
/// The byte allowance is CUMULATIVE across pages, which is the difference
/// that matters: the page function's own budget resets on every page, so
/// nothing below this function bounds the assembled reply. Each row is
/// measured as it is projected and the walk stops BEFORE the row that
/// would exceed the allowance — and stops fetching pages entirely, so the
/// helm does not pay to materialize a fleet it has already decided not to
/// send.
async fn session_listing(
    state: &AppState,
    asking_host: HostId,
    asking_session: &str,
) -> anyhow::Result<AgentReply> {
    // Archive-INCLUSIVE, unlike the UI's default browse view. The verb
    // promises every session the helm knows and an agent has no archive
    // switch to flip, so the honest listing carries them and flags them
    // (see `AgentSession::archived`) rather than silently omitting durable
    // history.
    let filter = crate::store::SessionFilter::default().include_archived(true);
    let sort = crate::store::ListSort::default();
    let mut sessions: Vec<AgentSession> = Vec::new();
    let mut spent = 0usize;
    let mut truncated = false;
    let mut cursor: Option<String> = None;
    'paging: loop {
        let remaining = AGENT_SESSION_CAP - sessions.len();
        let page = crate::aggregate::session_page(
            &state.manager,
            &state.store,
            &state.counts,
            cursor.as_deref(),
            remaining.min(crate::aggregate::MAX_PAGE_LIMIT),
            &filter,
            sort,
        )
        .await?;
        for row in &page.sessions {
            let row = agent_session(row, asking_host, asking_session);
            // The ENCODED size, because that is what has to fit in a frame;
            // a struct's in-memory footprint says nothing about it. A row
            // that will not encode at all is charged the whole budget
            // rather than failing the listing: it cannot be sent either
            // way, and the reply an agent can use is the truncated one.
            let encoded = serde_json::to_vec(&row).map_or(usize::MAX, |bytes| bytes.len());
            if spent.saturating_add(encoded) > AGENT_REPLY_BYTE_BUDGET {
                truncated = true;
                break 'paging;
            }
            spent += encoded;
            sessions.push(row);
        }
        match page.next_cursor {
            // The listing ended on its own: this is the whole fleet.
            None => break,
            // More exists. Keep walking while there is room, and report the
            // cut when there is not.
            Some(next) if sessions.len() < AGENT_SESSION_CAP => cursor = Some(next),
            Some(_) => {
                truncated = true;
                break;
            }
        }
    }
    Ok(AgentReply::Sessions {
        sessions,
        truncated,
    })
}

/// Project one host view down to what an agent is told about it.
///
/// A free function, not a method, so the mapping is testable without a
/// live helm: everything interesting about it — which word the phase
/// becomes, which row is marked current — is a pure function of the view
/// and the asking host's id.
fn agent_host(view: &crate::hosts::HostView, asking: HostId) -> AgentHost {
    AgentHost {
        name: view.name.clone(),
        kind: view.kind.to_string(),
        // The helm's own stable phase label, not a re-derivation: the
        // serialized `phase` tag, the UI's chip, the diagnostic trail and
        // this reply are one vocabulary, which is what lets an agent quote
        // a state word back to a user who is looking at the panel.
        state: view.state.phase().to_string(),
        current: view.id == asking,
    }
}

/// Project one merged session row down to what an agent is told about it.
///
/// Pure, for [`agent_host`]'s reason. The substitutions it makes — profile
/// name or program basename standing in for the raw invocation, empty
/// string standing in for an unclassified status — are the parts a reader
/// has to be able to check without standing up a fleet.
///
/// `current` is matched on BOTH identities. Session ids are supervisor-
/// minted and unique in practice, but the helm's merge already has a rule
/// for the case where two hosts claim one id (the first cache owner keeps
/// the row, the later claimant's is dropped and recorded as contested), and
/// an id-only comparison would then mark the RETAINED row — belonging to
/// the other host — as the asker's own. Requiring the host to match too
/// makes that state produce no marker at all, which is the fail-closed
/// answer the merge already chose.
fn agent_session(
    row: &crate::aggregate::SessionRow,
    asking_host: HostId,
    asking_session: &str,
) -> AgentSession {
    AgentSession {
        id: row.info.id.clone(),
        host: row.host_name.clone(),
        title: row.info.title.clone(),
        cwd: row.info.cwd.clone(),
        agent: agent_label(&row.info),
        status: status_word(&row.info.status).to_string(),
        current: row.host == asking_host && row.info.id == asking_session,
        archived: row.info.archived,
        stale: row.stale,
    }
}

/// The non-secret name for what is running in a session.
///
/// The profile's snapshotted name when the session came from one — that is
/// a label a user chose and the same one the UI's profile chip shows. For a
/// raw-invocation session there is no such label, and the answer is the
/// PROGRAM's basename and nothing else: `claude`, not
/// `claude --api-key sk-…`.
///
/// Dropping the arguments is the point, not a cosmetic trim. See
/// [`AgentSession::agent`] for the whole reasoning; the short version is
/// that this listing is fleet-wide and reachable with any one session's
/// credential, so a secret typed into one command line must not travel to
/// an unrelated session on another host.
///
/// An invocation that does not shell-split (an unbalanced quote in a
/// hand-edited row) falls back to its first whitespace-delimited token,
/// which is still the program spelling and still never an argument — a
/// slightly uglier label beats either an empty cell or a parse failure that
/// fails the whole listing.
fn agent_label(info: &farhelm_proto::SessionInfo) -> String {
    if let Some(profile) = &info.source_profile {
        return profile.name.clone();
    }
    let program = shell_words::split(&info.invocation)
        .ok()
        .and_then(|argv| argv.into_iter().next())
        .unwrap_or_else(|| {
            info.invocation
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_string()
        });
    basename(&program).to_string()
}

/// The last path component of `program`, or the whole string when it has
/// no separator.
///
/// Split on `/` alone: these are POSIX hosts, and a Windows-style path
/// would be a literal filename here rather than a path to shorten.
fn basename(program: &str) -> &str {
    program.rsplit('/').next().unwrap_or(program)
}

/// The status word a user sees for one session, or `""` for a session
/// nothing has classified.
///
/// The empty string is deliberate and is the whole reason this is a
/// function rather than a serde tag. `SessionStatus::Unknown` is plumbing
/// that must never render as a verdict (see that variant's docs): a client
/// showing the word "unknown" looks like it has decided something. Absent
/// text is this wire's way of saying the same thing the UI's absent badge
/// says.
///
/// Exit codes and stop annotations are deliberately NOT folded in the way
/// the UI's badge folds them. The badge is one capped string a person
/// reads; this is a column in a table an agent may go on to match against,
/// so the word stays a word. Staleness and archive state are separate
/// fields for the same reason — see [`AgentSession::stale`].
fn status_word(status: &SessionStatus) -> &'static str {
    match status {
        SessionStatus::Running => "running",
        SessionStatus::Waiting => "waiting",
        SessionStatus::Idle => "idle",
        SessionStatus::Exited { .. } => "exited",
        SessionStatus::Interrupted => "interrupted",
        SessionStatus::Error { .. } => "error",
        SessionStatus::Unknown => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hosts::{HostStateView, HostView, RefreshView};
    use farhelm_proto::{ProfileExistence, RestartOffer, SessionInfo, SourceProfile};

    fn host_view(id: i64, name: &str, kind: &'static str, state: HostStateView) -> HostView {
        HostView {
            id,
            kind,
            name: name.to_string(),
            destination: None,
            identity: None,
            remote_farhelm: None,
            remote_state_dir: None,
            state,
            incarnation: 1,
        }
    }

    fn session_info(id: &str, status: SessionStatus) -> SessionInfo {
        SessionInfo {
            id: id.to_string(),
            parent: None,
            title: "a title".to_string(),
            created_at: 1,
            last_activity_at: 1,
            creation_seq: None,
            cwd: "/w".to_string(),
            invocation: "claude --dangerously".to_string(),
            status,
            annotation: None,
            restart_offer: RestartOffer::FreshOnly,
            tabs: Vec::new(),
            archived: false,
            source_profile: None,
        }
    }

    fn session_row(info: SessionInfo, host_name: &str) -> crate::aggregate::SessionRow {
        crate::aggregate::SessionRow {
            info,
            host: 1,
            host_identity: None,
            host_name: host_name.to_string(),
            stale: false,
        }
    }

    /// A profile snapshot in a given existence state, for the two rows that
    /// differ only in whether the profile still exists.
    fn snapshot(name: &str, existence: ProfileExistence) -> SourceProfile {
        SourceProfile {
            id: "p1".to_string(),
            name: name.to_string(),
            existence,
        }
    }

    /// Spec: a host is reported by its display NAME, its kind word, the
    /// helm's own phase label, and a `current` flag true for exactly the
    /// asking session's host.
    ///
    /// This matters because every later verb an agent gets — create, clone
    /// — names its target by the `name` in this reply, so a projection that
    /// dropped or rewrote it would leave an agent with no way to name a
    /// host at all.
    ///
    /// The phase words are FROZEN LITERALS here, deliberately, and this
    /// test does not derive them from the manager. Deriving them would only
    /// assert that `phase()` equals itself; what is worth pinning is that
    /// the specific public words an agent reads (`connected`,
    /// `unreachable-reprobing`) are the ones it gets, since those are what
    /// a model quotes back to a user reading the same word in the hosts
    /// panel. `hosts::phase_matches_the_serialized_tag` is what holds those
    /// literals to the serializer's own vocabulary, so the two together
    /// cover the whole chain without either one restating the other.
    #[test]
    fn hosts_are_projected_with_the_helms_own_phase_vocabulary() {
        let connected = host_view(
            1,
            "this machine",
            "local",
            HostStateView::Connected {
                identity: None,
                build_version: "0.0.0".to_string(),
                refresh: RefreshView::Pending,
            },
        );
        let unreachable = host_view(
            2,
            "builder",
            "ssh",
            HostStateView::Unreachable {
                cause: "transport-failure",
                last_error: "no route".to_string(),
            },
        );

        let mine = agent_host(&connected, 1);
        assert_eq!(mine.name, "this machine");
        assert_eq!(mine.kind, "local");
        assert_eq!(mine.state, "connected");
        assert!(mine.current);

        let theirs = agent_host(&unreachable, 1);
        assert_eq!(theirs.name, "builder");
        assert_eq!(theirs.kind, "ssh");
        assert_eq!(theirs.state, "unreachable-reprobing");
        assert!(!theirs.current);
    }

    /// Spec: `agent` carries the source profile's snapshotted name when a
    /// session came from one — whether or not that profile still exists —
    /// and otherwise the invocation's PROGRAM BASENAME with every argument
    /// dropped.
    ///
    /// The argument-dropping is the clause that matters, and it is a
    /// security property rather than a formatting choice. Users put
    /// credentials in command lines; this listing is fleet-wide and any
    /// attached session's credential reads all of it; the reader is a model
    /// that will quote what it read. A projection that let `--api-key sk-…`
    /// through would leak one session's secret into an unrelated session's
    /// transcript, and no other test in the stack would notice.
    ///
    /// The deleted-profile row is here because the snapshot is the only
    /// handle left once a profile is gone: a later "does this profile still
    /// exist?" check that fell back to the invocation would be both
    /// misleading and, for a raw command line, potentially secret-bearing.
    #[test]
    fn a_sessions_agent_is_its_profile_name_or_its_program_basename() {
        // `session_info`'s invocation is `claude --dangerously`; only the
        // program survives.
        let raw = session_row(session_info("s1", SessionStatus::Running), "this machine");
        assert_eq!(agent_session(&raw, 1, "s1").agent, "claude");

        let mut with_secret = session_info("s2", SessionStatus::Running);
        with_secret.invocation = "/opt/bin/codex --api-key sk-secret-value".to_string();
        let row = session_row(with_secret, "this machine");
        assert_eq!(
            agent_session(&row, 1, "s1").agent,
            "codex",
            "arguments must never reach this wire"
        );

        let mut unparsable = session_info("s3", SessionStatus::Running);
        unparsable.invocation = "claude --title 'unbalanced".to_string();
        let row = session_row(unparsable, "this machine");
        assert_eq!(
            agent_session(&row, 1, "s1").agent,
            "claude",
            "an invocation that does not shell-split still yields only its program"
        );

        for existence in [ProfileExistence::Present, ProfileExistence::Deleted] {
            let mut from_profile = session_info("s4", SessionStatus::Running);
            from_profile.source_profile = Some(snapshot("Claude", existence));
            let row = session_row(from_profile, "this machine");
            assert_eq!(
                agent_session(&row, 1, "s1").agent,
                "Claude",
                "the snapshotted name stands regardless of the profile's existence"
            );
        }
    }

    /// Spec: exactly the asking session's row is marked `current`, matched
    /// on host AND session id, and the host name travels as the row's
    /// denormalized display name.
    ///
    /// `current` is the one value in this reply that neither the CLI nor
    /// the supervisor could reconstruct, so a bug here is invisible
    /// everywhere else in the stack.
    ///
    /// The BOTH-identities clause has a specific failure behind it. The
    /// helm's merge already handles two hosts claiming one session id — it
    /// keeps the first cache owner's row and drops the later claimant's —
    /// so if the asker is the later claimant, an id-only comparison marks
    /// the OTHER host's retained row as the asker's own. The merge chose to
    /// fail closed there; this keeps the projection from reopening it.
    #[test]
    fn only_the_asking_session_on_the_asking_host_is_current() {
        let mine = session_row(session_info("s1", SessionStatus::Idle), "this machine");
        let other = session_row(session_info("s2", SessionStatus::Idle), "builder");
        assert!(agent_session(&mine, 1, "s1").current);
        assert!(!agent_session(&other, 1, "s1").current);
        assert_eq!(agent_session(&other, 1, "s1").host, "builder");

        // Same id, different host: the collision case. `session_row` puts
        // every row on host 1, so asking as host 2 is the same shape as a
        // retained row belonging to someone else.
        assert!(
            !agent_session(&mine, 2, "s1").current,
            "a row on another host must not be marked as the asker's own"
        );
    }

    /// Spec: `archived` and `stale` travel as their own fields rather than
    /// being folded into `status` or dropped.
    ///
    /// Both carry information `status` cannot. A cached `running` from a
    /// host that went offline overnight is byte-identical to one observed a
    /// second ago, and SPEC.md requires retained rows from unreachable
    /// hosts to be clearly marked; an archived session is durable history
    /// rather than something to go and act on. Folding either into the
    /// status word would also break that column's promise of being a word
    /// an agent can match against.
    #[test]
    fn archive_and_staleness_travel_as_their_own_fields() {
        let mut archived = session_info("s1", SessionStatus::Running);
        archived.archived = true;
        let mut row = session_row(archived, "builder");
        row.stale = true;

        let projected = agent_session(&row, 1, "other");
        assert!(projected.archived);
        assert!(projected.stale);
        assert_eq!(
            projected.status, "running",
            "the status word stays a word; the two flags say the rest"
        );

        let live = session_row(session_info("s2", SessionStatus::Running), "this machine");
        let projected = agent_session(&live, 1, "other");
        assert!(!projected.archived);
        assert!(!projected.stale);
    }

    /// Spec: every live and ended status becomes the word the UI shows, and
    /// `Unknown` becomes the empty string rather than a word of its own
    /// beside the six real ones.
    ///
    /// The `Unknown` case is the reason this test exists. That variant is
    /// wire plumbing whose own documentation forbids rendering it, and the
    /// cheap implementation — serialize the tag — would put the word
    /// "unknown" in front of an agent as though the helm had decided
    /// something about the session.
    #[test]
    fn status_words_match_the_ui_and_unknown_renders_as_nothing() {
        assert_eq!(status_word(&SessionStatus::Running), "running");
        assert_eq!(status_word(&SessionStatus::Waiting), "waiting");
        assert_eq!(status_word(&SessionStatus::Idle), "idle");
        assert_eq!(status_word(&SessionStatus::Interrupted), "interrupted");
        assert_eq!(
            status_word(&SessionStatus::Exited { exit_code: Some(3) }),
            "exited"
        );
        assert_eq!(
            status_word(&SessionStatus::Error {
                detail: "no such file".to_string()
            }),
            "error"
        );
        assert_eq!(status_word(&SessionStatus::Unknown), "");
    }

    // ---------------------------------------------------------------
    // The production handler, against a real fleet.
    //
    // Everything above is the pure projection. What follows drives
    // `HelmAgentRequests::handle` itself over a real `AppState` — a real
    // connection manager, a real helm.db, scripted supervisors — because
    // the assembly is where the interesting mistakes live: the wrong
    // archive filter, one page instead of the fleet, the local host only,
    // a `current` marker computed from the wrong side.
    // ---------------------------------------------------------------

    use crate::rest_harness::{FleetBuilder, Harness, HostScript, local_id, session};

    /// The live connection's origin for `host`, as the client itself would
    /// have supplied it.
    ///
    /// Reading it from the manager rather than inventing a number is what
    /// makes the incarnation check a real gate in these tests: a handler
    /// that stopped consulting the manager would pass, but so would one
    /// that never checked at all — which is why
    /// [`a_superseded_connection_is_refused`] supplies a wrong one on
    /// purpose.
    fn origin_of(h: &Harness, host: HostId) -> AgentOrigin {
        let client = h
            .manager
            .status(host)
            .expect("an actor is running for this host")
            .client
            .expect("the host is connected");
        AgentOrigin {
            host,
            connection: client.connection_id(),
        }
    }

    /// Sessions the fleet builder scripts, differing in exactly the
    /// dimensions the reply has fields for.
    fn scripted(id: &str, created_at: i64, archived: bool, profile: Option<&str>) -> SessionInfo {
        SessionInfo {
            archived,
            invocation: "/usr/local/bin/claude --api-key sk-not-for-agents".to_string(),
            source_profile: profile.map(|name| snapshot(name, ProfileExistence::Present)),
            ..session(id, created_at)
        }
    }

    /// A local host with three sessions (one archived, one from a profile)
    /// and an ssh host that connects, caches two sessions, and then goes
    /// away — leaving stale rows behind.
    ///
    /// Built once and shared by the handler tests because standing a fleet
    /// up is the expensive part and every one of them wants the same
    /// interesting shape: two hosts, both kinds of session state, one host
    /// reachable and one not.
    async fn two_host_fleet() -> (Harness, HostId, HostId) {
        let (builder, remote) = FleetBuilder::new()
            .await
            .local(HostScript {
                identity: Some("identity-local".to_string()),
                sessions: vec![
                    scripted("local-live", 30, false, None),
                    scripted("local-archived", 20, true, None),
                    scripted("local-profile", 10, false, Some("Claude")),
                ],
                ..HostScript::default()
            })
            .await
            .ssh(
                "user@builder",
                HostScript {
                    identity: Some("identity-builder".to_string()),
                    sessions: vec![
                        scripted("remote-a", 40, false, None),
                        scripted("remote-b", 5, false, None),
                    ],
                    ..HostScript::default()
                },
            )
            .await;
        let h = builder.start().await;
        let local = local_id(&h.store).await;
        h.await_refreshed(local).await;
        h.await_refreshed(remote).await;
        // The remote's rows are now cached. Taking it down is what makes
        // them STALE without removing them, which is the state SPEC.md
        // requires to stay visible and marked.
        h.fleet.take_down(remote);
        h.await_state(remote, |state| state.phase() != "connected")
            .await;
        (h, local, remote)
    }

    /// Spec: `sessions` answered by the production handler carries the
    /// WHOLE fleet — both hosts, archived rows included, each row flagged
    /// for archive and staleness — with `current` on exactly the asking
    /// session's row and `truncated` false for a fleet that fits.
    ///
    /// This is the one test that exercises the assembly rather than the
    /// projection, and every clause is a bug that the pure unit tests above
    /// cannot see: a handler that listed only the local host, or passed the
    /// default archive-excluding filter, or took a single page and called
    /// it the fleet, would satisfy every one of them.
    #[tokio::test]
    async fn the_production_handler_lists_the_whole_fleet_with_its_flags() {
        let (h, local, _remote) = two_host_fleet().await;
        let handler = HelmAgentRequests::for_state(&h.state);

        let outcome = handler
            .handle(origin_of(&h, local), "local-live", AgentVerb::Sessions {})
            .await;
        let AgentOutcome::Ok {
            reply:
                AgentReply::Sessions {
                    sessions,
                    truncated,
                },
        } = outcome
        else {
            panic!("expected a sessions reply, got {outcome:?}");
        };

        assert!(!truncated, "five sessions is not a truncated fleet");
        let ids: Vec<&str> = sessions.iter().map(|s| s.id.as_str()).collect();
        for expected in [
            "local-live",
            "local-archived",
            "local-profile",
            "remote-a",
            "remote-b",
        ] {
            assert!(ids.contains(&expected), "{expected} missing from {ids:?}");
        }

        let by_id = |id: &str| {
            sessions
                .iter()
                .find(|s| s.id == id)
                .unwrap_or_else(|| panic!("{id} missing from {ids:?}"))
        };
        assert!(by_id("local-archived").archived);
        assert!(!by_id("local-live").archived);
        assert!(
            by_id("remote-a").stale,
            "a disconnected host's cached rows are last-known knowledge"
        );
        assert!(!by_id("local-live").stale);
        assert!(by_id("local-live").current);
        assert!(!by_id("remote-a").current);
        assert_eq!(
            by_id("local-live").agent,
            "claude",
            "the raw invocation's arguments must not reach the wire"
        );
        assert_eq!(by_id("local-profile").agent, "Claude");
    }

    /// One scripted session whose caller-supplied text is `bytes` long.
    ///
    /// The size goes in the TITLE because a title is free-form text a user
    /// types, admitted at tens of kilobytes by session creation — so a
    /// fleet of these is a legal fleet, not a corrupted one, which is the
    /// whole point of the test that uses it. Nothing here is malformed and
    /// nothing here would be rejected on the way in.
    fn fat_session(id: &str, created_at: i64, bytes: usize) -> SessionInfo {
        SessionInfo {
            title: "x".repeat(bytes),
            ..session(id, created_at)
        }
    }

    /// Encode `outcome` as the frame the connection would actually send,
    /// for the assertions about what fits on the wire.
    fn response_frame(outcome: &AgentOutcome) -> farhelm_proto::Frame {
        farhelm_proto::Frame::control(&farhelm_proto::ControlMsg::AgentResponse {
            req_id: 1,
            outcome: outcome.clone(),
        })
    }

    /// Spec: a legal fleet whose rows are large is cut at the reply's
    /// cumulative byte allowance — `truncated: true`, and a reply that
    /// still fits in one protocol frame — rather than assembled into
    /// something unsendable.
    ///
    /// This is the assembly's most consequential bound and no other test
    /// reaches it. `aggregate::session_page` applies its byte budget PER
    /// PAGE, so before the cumulative allowance existed, the row cap was
    /// the only limit on the whole answer: page after individually-valid
    /// page concatenated into a reply past `MAX_FRAME_LEN`, discarded whole
    /// at the writer's size backstop, reaching the agent as `Internal`
    /// instead of the partial listing the verb promises — after the helm
    /// had already paid to build and encode it, on up to four connections
    /// at once.
    ///
    /// The fixture is deliberately shaped to cross a page boundary: 120
    /// rows of 64 KiB is several times one page's own budget, so a listing
    /// that stopped counting at the page edge (or reset its tally there)
    /// fails here rather than passing by arithmetic accident.
    #[tokio::test]
    async fn a_fat_fleet_is_cut_at_the_reply_byte_allowance() {
        const ROWS_PER_HOST: usize = 60;
        const TITLE_BYTES: usize = 64 * 1024;
        // Newest first: a host's scripted list must arrive in the wire
        // order the drain validates (creation time descending), so the
        // timestamps count DOWN as the index counts up.
        let fat = |prefix: &str| -> Vec<SessionInfo> {
            (0..ROWS_PER_HOST)
                .map(|n| {
                    fat_session(
                        &format!("{prefix}-{n}"),
                        (ROWS_PER_HOST - n) as i64,
                        TITLE_BYTES,
                    )
                })
                .collect()
        };

        let (builder, remote) = FleetBuilder::new()
            .await
            .local(HostScript {
                identity: Some("identity-local".to_string()),
                sessions: fat("local"),
                ..HostScript::default()
            })
            .await
            .ssh(
                "user@builder",
                HostScript {
                    identity: Some("identity-builder".to_string()),
                    sessions: fat("remote"),
                    ..HostScript::default()
                },
            )
            .await;
        let h = builder.start().await;
        let local = local_id(&h.store).await;
        h.await_refreshed(local).await;
        h.await_refreshed(remote).await;

        let handler = HelmAgentRequests::for_state(&h.state);
        let outcome = handler
            .handle(origin_of(&h, local), "local-0", AgentVerb::Sessions {})
            .await;
        let AgentOutcome::Ok {
            reply:
                AgentReply::Sessions {
                    ref sessions,
                    truncated,
                },
        } = outcome
        else {
            panic!("expected a sessions reply, got {outcome:?}");
        };

        assert!(
            truncated,
            "a fleet past the byte allowance must say so; got {} rows",
            sessions.len()
        );
        assert!(
            sessions.len() < 2 * ROWS_PER_HOST,
            "the whole fleet was carried, so nothing was actually cut"
        );
        // More than one page's own byte budget came back, which is the
        // clause that proves the allowance survives the page boundary
        // rather than restarting at it.
        let carried = sessions.len() * TITLE_BYTES;
        assert!(
            carried > (farhelm_proto::MAX_FRAME_LEN / 2) as usize,
            "the reply did not even reach one page's budget ({carried} bytes), so the crossing \
             this test is about never happened"
        );
        let frame = response_frame(&outcome);
        assert!(
            !frame.exceeds_max_len(),
            "the reply must be sendable; it encoded to {} bytes",
            frame.encoded_len()
        );
    }

    /// Spec: a fleet past [`AGENT_SESSION_CAP`] rows stops at the cap and
    /// reports `truncated`.
    ///
    /// The second ceiling, and the one that decides the answer when the
    /// rows are small enough that bytes never bind. Both have to be
    /// exercised through the production loop, because the loop is where a
    /// cap can be applied to the wrong quantity — the page's limit rather
    /// than the accumulated total — and still look right for every fleet
    /// that fits in one page.
    #[tokio::test]
    async fn a_fleet_past_the_row_cap_is_cut_at_the_cap() {
        // Split across two hosts because no single host may serve more than
        // `LIST_SESSION_CAP` rows — the drain refuses past it — while the
        // MERGED fleet has no such limit, which is exactly the shape this
        // cap exists for. Newest first within each host, like any real
        // list; the drain refuses a list out of wire order.
        const PER_HOST: usize = AGENT_SESSION_CAP / 2 + 100;
        let many = |prefix: &str| -> Vec<SessionInfo> {
            (0..PER_HOST)
                .map(|n| session(&format!("{prefix}-{n}"), (PER_HOST - n) as i64))
                .collect()
        };
        let (builder, remote) = FleetBuilder::new()
            .await
            .local(HostScript {
                identity: Some("identity-local".to_string()),
                sessions: many("local"),
                ..HostScript::default()
            })
            .await
            .ssh(
                "user@builder",
                HostScript {
                    identity: Some("identity-builder".to_string()),
                    sessions: many("remote"),
                    ..HostScript::default()
                },
            )
            .await;
        let h = builder.start().await;
        let local = local_id(&h.store).await;
        h.await_refreshed(local).await;
        h.await_refreshed(remote).await;

        let handler = HelmAgentRequests::for_state(&h.state);
        let outcome = handler
            .handle(origin_of(&h, local), "local-0", AgentVerb::Sessions {})
            .await;
        let AgentOutcome::Ok {
            reply:
                AgentReply::Sessions {
                    sessions,
                    truncated,
                },
        } = outcome
        else {
            panic!("expected a sessions reply, got {outcome:?}");
        };
        assert_eq!(sessions.len(), AGENT_SESSION_CAP);
        assert!(truncated, "a fleet past the cap must say it was cut");
    }

    /// Spec: `hosts` answered by the production handler names every host
    /// the helm knows, with `current` on the connection the request arrived
    /// on and nowhere else.
    ///
    /// The asking host cannot be derived from anything in the request, so a
    /// handler that marked the local row, or the first row, or none, would
    /// look correct in every serialized shape — and would tell an agent it
    /// is sitting on a machine it is not.
    #[tokio::test]
    async fn the_production_handler_marks_the_asking_hosts_row() {
        let (h, local, _remote) = two_host_fleet().await;
        let handler = HelmAgentRequests::for_state(&h.state);

        let outcome = handler
            .handle(origin_of(&h, local), "local-live", AgentVerb::Hosts {})
            .await;
        let AgentOutcome::Ok {
            reply: AgentReply::Hosts { hosts },
        } = outcome
        else {
            panic!("expected a hosts reply, got {outcome:?}");
        };

        assert_eq!(hosts.len(), 2, "both hosts must be listed: {hosts:?}");
        let current: Vec<&str> = hosts
            .iter()
            .filter(|host| host.current)
            .map(|host| host.name.as_str())
            .collect();
        assert_eq!(
            current.len(),
            1,
            "exactly one host is the asker's: {hosts:?}"
        );
        assert!(
            hosts
                .iter()
                .any(|host| host.name == "user@builder" && !host.current),
            "the remote host must be listed and not marked current: {hosts:?}"
        );
    }

    /// Spec: a request whose origin names a connection the host is no
    /// longer served by is refused `Unavailable`, not answered.
    ///
    /// A registry row's id outlives the machine behind it — retarget and
    /// adoption both keep the row and replace what answers on it — so a
    /// request forwarded by a superseded connection would otherwise have
    /// its `current` marker computed against the row's new occupant. The
    /// manager already refuses to record a mutation against a stale
    /// incarnation for exactly this reason; this is the same rule on the
    /// read path.
    #[tokio::test]
    async fn a_superseded_connection_is_refused() {
        let (h, local, _remote) = two_host_fleet().await;
        let handler = HelmAgentRequests::for_state(&h.state);
        let stale = AgentOrigin {
            connection: origin_of(&h, local).connection + 1_000,
            host: local,
        };

        match handler
            .handle(stale, "local-live", AgentVerb::Hosts {})
            .await
        {
            AgentOutcome::Err { kind, message } => {
                assert_eq!(kind, ErrorKind::Unavailable);
                assert!(
                    message.contains("retry"),
                    "the refusal must name the remedy, got: {message}"
                );
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    // ---------------------------------------------------------------
    // The withdrawal seam: what happens to an answer in flight when the
    // manager stops publishing the connection it was asked on.
    // ---------------------------------------------------------------

    /// A handler that announces every call and then parks until released.
    ///
    /// The park is the fixture: it holds a listing open across the moment
    /// the manager replaces the host's connection, which is the window the
    /// test is about and which nothing else in this crate can produce (the
    /// real listing finishes in microseconds).
    struct ParkedHandler {
        entered: tokio::sync::mpsc::Sender<()>,
        gate: Arc<tokio::sync::Semaphore>,
    }

    #[async_trait]
    impl AgentRequestHandler for ParkedHandler {
        async fn handle(
            &self,
            _origin: AgentOrigin,
            _session_id: &str,
            _verb: AgentVerb,
        ) -> AgentOutcome {
            let _ = self.entered.send(()).await;
            let _permit = self.gate.acquire().await.expect("the gate is never closed");
            AgentOutcome::Ok {
                reply: AgentReply::Hosts { hosts: Vec::new() },
            }
        }
    }

    /// Spec: when the manager withdraws a host's connection while an answer
    /// is being assembled on it, the superseded peer receives no answer and
    /// its transport does not survive the abandoned work.
    ///
    /// Both halves were real. A registry row outlives the machine behind it
    /// — retarget, adoption and reconnect all keep the row and replace what
    /// answers on it — so an answer that lands after the swap describes one
    /// machine while claiming to be about another; for a `hosts` reply that
    /// is the `current` marker pointing at whatever now occupies the row.
    /// And the connection did not merely leak the answer: the answering
    /// task owns a clone of the writer channel, so dropping the manager's
    /// `Arc` closed nothing — the writer task stayed parked, the transport
    /// (in production an ssh child) stayed open, and the reply went out on
    /// it. Explicit retirement at the withdrawal seam is what ends both.
    ///
    /// The retarget is driven through the REAL seam (a store edit plus
    /// `sync_registry`) rather than by calling any teardown directly, which
    /// is the point: this test fails if the manager gains another way to
    /// withdraw a client that forgets to retire it.
    #[tokio::test]
    async fn a_withdrawn_connection_never_delivers_its_parked_answer() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        let (ours, theirs) = tokio::io::duplex(1 << 20);
        let (builder, remote) = FleetBuilder::new()
            .await
            .ssh(
                "user@builder",
                HostScript {
                    identity: Some("identity-builder".to_string()),
                    sessions: vec![session("remote-a", 1)],
                    peer: Some(theirs),
                    ..HostScript::default()
                },
            )
            .await;
        let h = builder.start().await;

        let (entered, mut calls) = tokio::sync::mpsc::channel(4);
        let gate = Arc::new(tokio::sync::Semaphore::new(0));
        h.manager.set_agent_requests(Arc::new(ParkedHandler {
            entered,
            gate: Arc::clone(&gate),
        }));

        let (r, w) = tokio::io::split(ours);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .expect("the peer's half of the hello exchange");
        h.await_refreshed(remote).await;

        writer
            .write_control(&farhelm_proto::ControlMsg::AgentRequest {
                req_id: 7,
                session_id: "remote-a".to_string(),
                request: AgentVerb::Hosts {},
            })
            .await
            .expect("send the agent request");
        // The barrier: the listing is genuinely inside the handler, so what
        // happens next happens DURING it rather than racing it.
        tokio::time::timeout(std::time::Duration::from_secs(10), calls.recv())
            .await
            .expect("the upcall never reached the handler")
            .expect("the handler's announcement channel is open");

        // Retarget: the row now points at another machine, and the
        // connection that carried the request is no longer the one that
        // serves it.
        h.store
            .update_ssh_destination(remote, "user@elsewhere")
            .await
            .expect("retarget the host");
        h.manager.sync_registry().await.expect("reconcile");

        // Only now is the parked listing allowed to finish, so anything
        // that arrives below is the abandoned answer and nothing else.
        gate.add_permits(1);
        let ending = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            while let Ok(Some(frame)) = reader.read_frame().await {
                if let Ok(farhelm_proto::ControlMsg::AgentResponse { req_id, outcome }) =
                    parse_control(&frame)
                {
                    panic!("the superseded peer received an answer for {req_id}: {outcome:?}");
                }
            }
        })
        .await;
        assert!(
            ending.is_ok(),
            "the withdrawn connection was still open; the abandoned answer kept its transport \
             alive"
        );
    }

    /// Spec: once the helm's state is gone, the handler refuses with
    /// `Unavailable` rather than panicking — and it really is gone, which
    /// is to say the handler holds no strong reference to it.
    ///
    /// Both halves are lifetime properties nothing else observes. The
    /// `Weak` exists to break the cycle `AppState → manager → handler slot
    /// → handler → AppState`, and a change to a strong reference would leak
    /// the entire helm state (every connection, every actor task) with no
    /// test failing. The refusal is the other half: a connection can
    /// outlive the state by moments, and an `unwrap` on the upgrade would
    /// turn a shutdown into a panic inside a spawned task — which the
    /// supervisor on the far end experiences as an upcall that never
    /// answers.
    #[tokio::test]
    async fn a_dropped_helm_state_is_refused_rather_than_panicking() {
        let (h, local, _remote) = two_host_fleet().await;
        let handler = HelmAgentRequests::for_state(&h.state);
        let origin = origin_of(&h, local);
        let observer = Arc::downgrade(&h.state);

        drop(h);
        assert!(
            observer.upgrade().is_none(),
            "the handler must not keep the helm's state alive"
        );

        match handler
            .handle(origin, "local-live", AgentVerb::Sessions {})
            .await
        {
            AgentOutcome::Err { kind, message } => {
                assert_eq!(kind, ErrorKind::Unavailable);
                assert!(
                    message.contains("shutting down"),
                    "the refusal must say what happened, got: {message}"
                );
            }
            other => panic!("expected a shutdown refusal, got {other:?}"),
        }
    }

    // ---------------------------------------------------------------
    // The lifecycle verbs — Rename, Stop, Archive — against the same
    // production handler and the same real fleet machinery as the read-only
    // verbs above. Each drives `do_rename_session`/`do_stop_session`/
    // `do_archive_session` (`sessions.rs`) through a REAL routed call to a
    // scripted supervisor, so what is under test is the whole seam: origin
    // validation, `session_id: None` resolving to the asker, the shared
    // helper functions the REST handlers also call, and the reply's
    // projection back into an `AgentSession`.
    // ---------------------------------------------------------------

    /// A single local host spliced to `client_side`, with `sessions`
    /// already cached — the fixture every scripted-exchange lifecycle test
    /// below needs. `route_session` requires a cached owner before it will
    /// forward anything.
    ///
    /// Takes the duplex HALF rather than creating the pair itself, and that
    /// is load-bearing rather than a style choice: the splice relays the
    /// manager's own hello handshake across to whatever answers on the
    /// OTHER half (`rest_harness::run_spliced`'s crossing-hellos relay), so
    /// this function's own `await_refreshed` call cannot resolve until a
    /// task is already running on that other half to complete it. A caller
    /// that built the duplex, awaited THIS function, and only then spawned
    /// its responder would deadlock — the responder's `tokio::spawn` line
    /// would never run because the awaiting test task is itself blocked
    /// inside this function. Every call site below therefore spawns its
    /// responder on the peer half FIRST and passes the other half in here
    /// second, so the two race properly instead of strictly sequencing.
    async fn spliced_local_fleet(
        client_side: tokio::io::DuplexStream,
        sessions: Vec<SessionInfo>,
    ) -> (Harness, HostId) {
        let harness = FleetBuilder::new()
            .await
            .local(HostScript {
                identity: Some("identity-local".to_string()),
                sessions,
                peer: Some(client_side),
                ..HostScript::default()
            })
            .await
            .start()
            .await;
        let local = local_id(&harness.store).await;
        harness.await_refreshed(local).await;
        (harness, local)
    }

    /// Spec: `Rename` can target ANY session the helm knows, not only the
    /// asking one — the wider authority `AgentVerb`'s own docs describe —
    /// and the reply is the RENAMED row, current-marked against the
    /// ASKING session rather than the one it acted on.
    ///
    /// Two sessions are cached so the asker and the target are provably
    /// different rows: a fixture with only one session could not
    /// distinguish "targeted the named session" from "always acts on the
    /// asker and ignored the field".
    #[tokio::test]
    async fn rename_can_target_any_named_session_and_returns_its_updated_row() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        // Spawned BEFORE the fleet is built — see `spliced_local_fleet`'s
        // own docs for why the order is load-bearing rather than cosmetic.
        let responder = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .expect("handshake");
            let frame = reader
                .read_frame()
                .await
                .expect("read frame")
                .expect("a request");
            let ControlMsg::RenameSession {
                req_id,
                session_id,
                title,
            } = parse_control(&frame).expect("decode request")
            else {
                panic!("expected RenameSession");
            };
            assert_eq!(session_id, "other", "the NAMED target, not the asker");
            assert_eq!(title, "new title");
            writer
                .write_control(&ControlMsg::SessionRenamed {
                    req_id,
                    session: SessionInfo {
                        title,
                        ..session("other", 2)
                    },
                })
                .await
                .expect("write reply");
        });
        let (h, local) =
            spliced_local_fleet(client_side, vec![session("other", 2), session("asker", 1)]).await;

        let handler = HelmAgentRequests::for_state(&h.state);
        let outcome = handler
            .handle(
                origin_of(&h, local),
                "asker",
                AgentVerb::Rename {
                    session_id: Some("other".to_string()),
                    title: "new title".to_string(),
                },
            )
            .await;
        responder.await.expect("join the scripted supervisor");

        match outcome {
            AgentOutcome::Ok {
                reply: AgentReply::Session { session },
            } => {
                assert_eq!(session.id, "other");
                assert_eq!(session.title, "new title");
                assert!(
                    !session.current,
                    "the row acted on is not the asking session's own"
                );
            }
            other => panic!("expected a session reply, got {other:?}"),
        }
    }

    /// Spec: a lifecycle verb naming no `session_id` acts on the ASKING
    /// session — the substitution `AgentVerb`'s own docs promise.
    ///
    /// Observed from the far end rather than only from the reply: the
    /// scripted supervisor asserts the `RenameSession` it received named
    /// "asker" explicitly, which is the only way to tell "the helm
    /// substituted the asker" apart from "the helm forwarded an empty or
    /// missing id and got lucky with a single-session fleet".
    #[tokio::test]
    async fn rename_with_no_session_id_targets_the_asking_session() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let responder = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .expect("handshake");
            let frame = reader
                .read_frame()
                .await
                .expect("read frame")
                .expect("a request");
            let ControlMsg::RenameSession {
                req_id,
                session_id,
                title,
            } = parse_control(&frame).expect("decode request")
            else {
                panic!("expected RenameSession");
            };
            assert_eq!(session_id, "asker", "None must resolve to the asker");
            writer
                .write_control(&ControlMsg::SessionRenamed {
                    req_id,
                    session: SessionInfo {
                        title,
                        ..session("asker", 1)
                    },
                })
                .await
                .expect("write reply");
        });
        let (h, local) = spliced_local_fleet(client_side, vec![session("asker", 1)]).await;

        let handler = HelmAgentRequests::for_state(&h.state);
        let outcome = handler
            .handle(
                origin_of(&h, local),
                "asker",
                AgentVerb::Rename {
                    session_id: None,
                    title: "self-renamed".to_string(),
                },
            )
            .await;
        responder.await.expect("join the scripted supervisor");

        match outcome {
            AgentOutcome::Ok {
                reply: AgentReply::Session { session },
            } => {
                assert_eq!(session.id, "asker");
                assert!(session.current, "the asker's own row is current");
            }
            other => panic!("expected a session reply, got {other:?}"),
        }
    }

    /// Spec: a title the supervisor refuses (SPEC.md's control-character
    /// rule) reaches the agent as the supervisor's OWN refusal text,
    /// verbatim — the same passthrough contract `rename_session`'s REST
    /// route holds, now proven over the agent path.
    ///
    /// A sentinel string stands in for the refusal so the assertion checks
    /// the exact bytes crossed the relay rather than merely that SOME
    /// error came back — `rename_session_invalid_title_returns_400_with_
    /// supervisor_message` in `sessions_tests.rs` pins the identical
    /// contract on the REST route with the same technique.
    #[tokio::test]
    async fn rename_refusal_reaches_the_agent_verbatim() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        const SENTINEL: &str = "SENTINEL-agent-rename: title must not contain control characters";

        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let responder = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .expect("handshake");
            let frame = reader
                .read_frame()
                .await
                .expect("read frame")
                .expect("a request");
            let ControlMsg::RenameSession { req_id, .. } =
                parse_control(&frame).expect("decode request")
            else {
                panic!("expected RenameSession");
            };
            writer
                .write_control(&ControlMsg::Error {
                    req_id,
                    message: SENTINEL.to_string(),
                    kind: ErrorKind::InvalidRequest,
                })
                .await
                .expect("write reply");
        });
        let (h, local) = spliced_local_fleet(client_side, vec![session("asker", 1)]).await;

        let handler = HelmAgentRequests::for_state(&h.state);
        let outcome = handler
            .handle(
                origin_of(&h, local),
                "asker",
                AgentVerb::Rename {
                    session_id: None,
                    title: "bad\u{7}title".to_string(),
                },
            )
            .await;
        responder.await.expect("join the scripted supervisor");

        match outcome {
            AgentOutcome::Err { kind, message } => {
                assert_eq!(kind, ErrorKind::InvalidRequest);
                assert_eq!(
                    message, SENTINEL,
                    "the supervisor's own refusal must reach the agent verbatim"
                );
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    /// Spec: `Stop` ends a running session's agent and answers with
    /// [`AgentReply::Stopped`] — empty, matching the REST route's own
    /// empty-object success body.
    #[tokio::test]
    async fn stop_ends_a_running_sessions_agent() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        // `session()` defaults to `SessionStatus::Running` — the "a running
        // session stops" case this test is named for.
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let responder = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .expect("handshake");
            let frame = reader
                .read_frame()
                .await
                .expect("read frame")
                .expect("a request");
            let ControlMsg::StopSession { req_id, session_id } =
                parse_control(&frame).expect("decode request")
            else {
                panic!("expected StopSession");
            };
            assert_eq!(session_id, "asker");
            writer
                .write_control(&ControlMsg::SessionStopped { req_id })
                .await
                .expect("write reply");
        });
        let (h, local) = spliced_local_fleet(client_side, vec![session("asker", 1)]).await;

        let handler = HelmAgentRequests::for_state(&h.state);
        let outcome = handler
            .handle(
                origin_of(&h, local),
                "asker",
                AgentVerb::Stop { session_id: None },
            )
            .await;
        responder.await.expect("join the scripted supervisor");

        assert!(
            matches!(
                outcome,
                AgentOutcome::Ok {
                    reply: AgentReply::Stopped {}
                }
            ),
            "expected an empty Stopped reply, got {outcome:?}"
        );
    }

    /// Spec: `Archive` flips the retained row's `archived` flag, reported
    /// through the same [`AgentReply::Session`] shape `Rename` uses.
    #[tokio::test]
    async fn archive_flips_the_archived_flag() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let responder = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .expect("handshake");
            let frame = reader
                .read_frame()
                .await
                .expect("read frame")
                .expect("a request");
            let ControlMsg::ArchiveSession { req_id, session_id } =
                parse_control(&frame).expect("decode request")
            else {
                panic!("expected ArchiveSession");
            };
            assert_eq!(session_id, "asker");
            writer
                .write_control(&ControlMsg::SessionArchived {
                    req_id,
                    session: SessionInfo {
                        archived: true,
                        ..session("asker", 1)
                    },
                })
                .await
                .expect("write reply");
        });
        let (h, local) = spliced_local_fleet(client_side, vec![session("asker", 1)]).await;

        let handler = HelmAgentRequests::for_state(&h.state);
        let outcome = handler
            .handle(
                origin_of(&h, local),
                "asker",
                AgentVerb::Archive { session_id: None },
            )
            .await;
        responder.await.expect("join the scripted supervisor");

        match outcome {
            AgentOutcome::Ok {
                reply: AgentReply::Session { session },
            } => {
                assert!(session.archived, "the archived flag must flip in the reply");
            }
            other => panic!("expected a session reply, got {other:?}"),
        }
    }

    /// Spec: a lifecycle verb naming a session the helm has never heard of
    /// is refused `NotFound`, before any supervisor is ever asked.
    ///
    /// A STANDALONE fleet (no scripted peer) is deliberate: if this ever
    /// regressed into forwarding the request anyway, there would be no
    /// script to answer it and the test would hang instead of failing
    /// cleanly — which is exactly the signal that would catch the
    /// regression this test is pinning against.
    #[tokio::test]
    async fn a_lifecycle_verb_on_an_unknown_session_is_not_found() {
        let harness = FleetBuilder::new()
            .await
            .local(HostScript {
                identity: Some("identity-local".to_string()),
                sessions: vec![session("asker", 1)],
                ..HostScript::default()
            })
            .await
            .start()
            .await;
        let local = local_id(&harness.store).await;
        harness.await_refreshed(local).await;

        let handler = HelmAgentRequests::for_state(&harness.state);
        let outcome = handler
            .handle(
                origin_of(&harness, local),
                "asker",
                AgentVerb::Stop {
                    session_id: Some("ghost".to_string()),
                },
            )
            .await;

        match outcome {
            AgentOutcome::Err { kind, message } => {
                assert_eq!(kind, ErrorKind::NotFound);
                assert!(
                    message.contains("ghost"),
                    "the refusal must name the id it could not place: {message}"
                );
            }
            other => panic!("expected a NotFound refusal, got {other:?}"),
        }
    }
}
