//! The helm's answers to questions asked — and actions requested — by an
//! agent from inside a session: the top of the relay whose supervisor half
//! lives in `farhelm-supervisor`'s `service::agent_relay`.
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
//! sessions, `sessions::do_rename_session`/`do_stop_session`/
//! `do_archive_session` for the three lifecycle verbs, and
//! `sessions::do_create_session` for `create` and `clone`. Not for economy:
//! the point of routing an agent's questions (and its actions) through the
//! helm at all is that the agent and the user see, and act on, one fleet.
//! Two listings assembled two ways would drift, and the drift would show
//! up as an agent confidently naming a host that is not in the panel the
//! user is reading; two rename implementations would drift on exactly the
//! validation rule that matters (SPEC.md's control-character refusal); two
//! create implementations would drift on the cache seed that makes a new
//! session appear in the UI without a refresh, and on the remembered-
//! default write that decides what the user's next create dialog suggests.
//!
//! # The creating verbs are what could not have been built anywhere else
//!
//! `create` and `clone` name their target host by DISPLAY NAME — the value
//! `hosts` reports — and that is the capability the whole relay exists for.
//! A supervisor-local implementation has no fleet, no host names, and no
//! route to another machine, so "clone this session onto that host" is
//! precisely the thing it could not do. Everything the verbs need lives on
//! this side: the host list, the create path, the target host's profile
//! catalog, and helm.db's remembered default.
//!
//! Cross-host profile resolution is the subtle half, and it has exactly one
//! rule: NAMES cross hosts, IDS do not. A profile id is minted per
//! supervisor and every fresh install seeds the same starter ids, so an id
//! carried to another host does not fail — it resolves, onto a profile
//! nobody chose. So a clone follows the id only when the target IS the
//! source's host, and otherwise resolves the snapshotted profile NAME
//! against the target's own catalog, refusing by name when there is no
//! match. There is deliberately no fallback to the source's raw invocation:
//! a command line written for one machine may name a binary that is absent,
//! a different build, or one that takes different flags on another.
//!
//! # Lifecycle verbs act on ANY session, not only the asker's own
//!
//! `Rename`, `Stop` and `Archive` each carry `session_id: Option<String>`,
//! and `None` resolves to the ASKING session — the one the supervisor has
//! already proven this connection's credential belongs to. A `Some(id)`
//! names any session the helm knows, on any host, BY ID — including the
//! asking session's own id, which [`resolve_target`] accepts exactly as it
//! would an explicit `None`; there is no separate "you may not name
//! yourself" rule to enforce. That is intentional: the feature's mental
//! model is an agent talking to the helm
//! itself, which already has fleet-wide authority, and inventing a
//! narrower per-session permission for agents alone would be a second
//! authorization model with nothing else in this system to keep it
//! honest. What IS worth a paper trail is which session asked to act on
//! which — logged at `info` by [`resolve_target`], the one place all three
//! verbs resolve the substitution — so an operator reading the helm's log
//! can tell an agent renaming itself apart from one reaching across the
//! fleet.
//!
//! `Clone` is deliberately NARROWER, and the asymmetry is not an
//! oversight: its source is always the asking session, never a named one.
//! "Clone that other session over there" is already expressible as a
//! `Create` naming the same profile and directory, so a `source_session`
//! field would add a second spelling for one thing while doubling the
//! resolution rules the verb has to carry.
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
///
/// That remains the CONTRACT, not merely a wish, even though the caller
/// now survives a breach of it: `client::SupervisorClient`'s answer task is
/// supervised and answers on a dead handler's behalf (see
/// `client::panic_fallback`). The backstop exists because the cost of a
/// silent handler is not one lost reply — a mutation's delete fence stays
/// claimed against the asking session until the connection dies — and it
/// can only ever produce the outcome-unknown ending, which is strictly
/// worse for the user than the real answer this trait promises.
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
    ///
    /// `session_id` has a SECOND job on the five acting verbs, and it is
    /// easy to miss from the signature alone. On the three lifecycle verbs
    /// it is the default TARGET: a `Rename`/`Stop`/`Archive` carrying
    /// `session_id: None` in its own field acts on this authenticated id
    /// (see [`resolve_target`]), which is what makes a bare `farhelm agent
    /// stop` mean "stop me". That is a substitution, not an authority check
    /// — an explicit target may name any session in the fleet, including
    /// this one. On `Clone` it is the SOURCE, and there is no override: a
    /// clone always copies the asking session, so this id is the only thing
    /// that says what is being copied.
    async fn handle(&self, origin: AgentOrigin, session_id: &str, verb: AgentVerb) -> AgentOutcome;

    /// Whether `origin`'s connection is STILL the one its host is served
    /// by, asked again once an answer is ready.
    ///
    /// Separate from [`Self::handle`], and synchronous, because of when it
    /// is called: the client asks it one step before it queues a successful
    /// answer to a READ-ONLY verb (see `SupervisorClient::spawn_agent_answer`),
    /// to close the window between the entry check and the reply. The
    /// listing in between awaits on the database and the manager, and a
    /// host retargeted, adopted or reconnected in that window has a
    /// registry row whose machine has changed — so the answer's `current`
    /// marker would name a host that is no longer the asking session's.
    ///
    /// A COMPLETED MUTATION SKIPS THIS CHECK. Every verb
    /// [`AgentVerb::is_mutating`] answers `true` for — the lifecycle three
    /// and the two creating verbs — is re-checked on the way IN by `handle`
    /// and not on the way out, because by then the change has already
    /// happened at its target and there is nothing to withdraw: converting
    /// it into the `Unavailable` refusal this check produces would tell the
    /// caller "nothing happened, retry freely" about an act that just took
    /// effect, and for a `Create`/`Clone` the act was starting a session
    /// whose id the caller would then never learn. See
    /// `SupervisorClient::spawn_agent_answer`'s own docs for the full
    /// argument, including why the one thing the exit check could still
    /// have caught for those verbs is handled by pinning the reply's host
    /// name instead ([`agent_session_reply`]).
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
        // Captured before `verb` is consumed by the dispatch below, for the
        // question only the failure arm asks: whether what was attempted
        // CHANGES something. See [`transport_outcome`].
        let mutating = verb.is_mutating();
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
                        agent_session_reply(&state, &claim, info, origin.host, session_id)
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
                        agent_session_reply(&state, &claim, info, origin.host, session_id)
                    })
            }
            AgentVerb::Create {
                host,
                cwd,
                profile_name,
                invocation,
                title,
                intent_key,
            } => {
                create_for_agent(
                    &state,
                    origin,
                    session_id,
                    CreateRequest {
                        host,
                        cwd,
                        profile_name,
                        invocation,
                        title,
                        intent_key,
                    },
                )
                .await
            }
            AgentVerb::Clone {
                host,
                cwd,
                title,
                intent_key,
            } => {
                clone_for_agent(
                    &state,
                    origin,
                    session_id,
                    CloneRequest {
                        host,
                        cwd,
                        title,
                        intent_key,
                    },
                )
                .await
            }
        };
        match reply {
            Ok(reply) => AgentOutcome::Ok { reply },
            // Classified the same way the REST surface classifies the SAME
            // failures (`crate::error_kind`), rather than flattened to
            // `Internal`: a lifecycle or creating verb's refusal — an unknown
            // session, a rejected title, a non-connected host, a profile name
            // the target does not have — is exactly the kind of thing a
            // caller can act on differently, and an agent deserves the same
            // distinction a browser gets. The two read-only verbs above
            // rarely produce a classifiable error at all (a listing failure
            // has nothing upstream to classify against), so this arm falls
            // back to `Internal` for them exactly as before.
            //
            // A dead target-supervisor connection is consulted FIRST,
            // because `error_kind` has no answer for it: nothing in that
            // chain is a `SupervisorError` (the peer never replied), so it
            // falls through to `Internal` — the one kind that tells a caller
            // nothing at all about retrying.
            Err(error) => transport_outcome(&error, mutating).unwrap_or(AgentOutcome::Err {
                kind: crate::error_kind(&error),
                message: format!("{error:#}"),
            }),
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

/// Classify a failure whose cause is the TARGET supervisor never giving a
/// usable answer, or `None` if that is not what went wrong.
///
/// The helm sits in the middle of two hops, and this is the far one: the
/// asking session's supervisor forwarded the verb up to the helm, and the
/// helm routed it down to the supervisor that owns the target session. A
/// MUTATION — `Rename`/`Stop`/`Archive`, or a `Create`/`Clone` — that
/// reached THAT supervisor and lost only its reply is the same
/// delivered-outcome-unknown ending the near hop already speaks about
/// (`service::agent_relay::connection_lost_after_queueing`) — and it used to
/// arrive at the agent as `Internal`, because [`crate::error_kind`] finds
/// nothing to classify in a chain whose peer never answered. `Internal` says
/// nothing about retrying, and a blind retry is not free either way: a
/// second stop can kill an agent somebody restarted after the first one took
/// effect, and a second create can leave a real session running on a host
/// under an id nobody was ever told.
///
/// So the phase [`crate::SupervisorTransportError`] records decides the
/// vocabulary, and only for a mutation:
///
/// - Never enqueued, either class: [`ErrorKind::Unavailable`] — nothing
///   left this process, so nothing happened and a retry is free. (This is a
///   change for listings too, and a strictly more accurate one: the old
///   `Internal` claimed a fault where there was a missing peer.)
/// - Enqueued, then no USABLE answer, MUTATION: [`ErrorKind::Timeout`],
///   whose documented contract is "delivered, outcome unknown", plus the
///   remedy that says to look before retrying. "No usable answer" covers
///   all three post-send endings — the connection dying without a reply, a
///   correlated reply of a variant the request's own wrapper does not
///   accept, and the right variant carrying a payload the ingress rules
///   refuse. They differ in how they look and not at all in what they let a
///   caller conclude: the request went out, and nothing came back that says
///   what became of it. A peer that answered a `stop` with a rename
///   confirmation may perfectly well have stopped the session first; one
///   that answered a `create` with anything but a `SessionCreated` may
///   perfectly well have started the session; and one that answered with a
///   `SessionCreated` whose id this helm had to refuse almost certainly
///   did, under an id nobody can now be told.
/// - Enqueued, then no answer, listing: `Unavailable` as well. A listing
///   has nothing to double-apply, so the retry-safe kind stays true however
///   far the request got; the mutation vocabulary is deliberately not
///   spread to a class that cannot need it.
/// - Enqueued, then an unusable reply (wrong variant or refused payload),
///   listing: not classified here at all (`None`), so
///   [`crate::error_kind`]'s `Internal` stands. A peer violating the
///   protocol is a fault rather than an unavailability, and for a class
///   with nothing at stake the honest word for it is the one that says
///   "this should not happen".
///
/// The class is the FAILED REQUEST's, not the verb's, and those are not the
/// same thing. `Clone` is mutating, but it begins by SNAPSHOTTING its source
/// with a plain listing, and a transport failure there is a failure of that
/// listing: no create has been dispatched anywhere, so retrying is free and
/// telling the agent to go inspect the fleet before it does would be a
/// fabricated hazard. A phase that is read-only inside a mutating verb marks
/// its failures with [`ReadOnlyPhase`], and this reads that marker as
/// overriding `mutating` — which is also why the marker is attached where
/// the read is issued rather than inferred here: only the caller knows which
/// of its requests had nothing at stake.
///
/// The message keeps the whole chain (`{error:#}`) rather than a sentence
/// of its own, because the context above the transport error names which
/// operation was attempted and the agent has no other way to learn it.
///
/// That makes the chain's SIZE this function's problem, since what it
/// returns is re-encoded into the asking agent's own reply frame: a
/// transport error carrying an unbounded rendering of the peer's message
/// pushes that frame past the protocol limit, and
/// `client::agent_response_frame`'s backstop then replaces the answer. That
/// is why [`crate::SupervisorTransportError::SentWrongReply`] keeps only a
/// variant name and its `SentInvalidReply` sibling only a fixed phrase, and
/// why the backstop preserves this function's `Timeout` and remedy when it
/// does have to replace an oversized outcome.
fn transport_outcome(error: &anyhow::Error, mutating: bool) -> Option<AgentOutcome> {
    use crate::SupervisorTransportError as Lost;
    let lost = crate::find_cause::<Lost>(error)?;
    // The verb's class, narrowed to the FAILED REQUEST's class: a read-only
    // phase of a mutating verb put nothing durable at stake, so it takes the
    // listing rules whatever the verb was.
    let mutating = mutating && crate::find_cause::<ReadOnlyPhase>(error).is_none();
    match (lost, mutating) {
        (
            Lost::SentUnanswered | Lost::SentWrongReply { .. } | Lost::SentInvalidReply { .. },
            true,
        ) => Some(AgentOutcome::Err {
            kind: ErrorKind::Timeout,
            message: format!(
                "{error:#}; the outcome is unknown — {}",
                farhelm_proto::AGENT_MUTATION_UNKNOWN_REMEDY
            ),
        }),
        (Lost::SentWrongReply { .. } | Lost::SentInvalidReply { .. }, false) => None,
        _ => Some(AgentOutcome::Err {
            kind: ErrorKind::Unavailable,
            message: format!("{error:#}"),
        }),
    }
}

/// Marks a transport failure as one a MUTATING verb suffered while doing
/// something READ-ONLY — gathering the input for the mutation, not
/// performing it.
///
/// Attached as anyhow context at the read itself, and consulted by
/// [`transport_outcome`], which is the only reader. There is exactly one
/// producer today: `clone_for_agent` snapshots its source session with a
/// listing before any create is dispatched, and without this marker a
/// supervisor that mishandles THAT listing would tell the agent its clone
/// might have happened — the one thing that is certainly untrue at that
/// point, since nothing has been sent to any target yet.
///
/// A marker rather than a second `mutating` parameter threaded down through
/// the request helpers because the fact belongs to one request out of
/// several inside one verb, and the classifier sees only the error that
/// escaped. The `&'static str` is the phase's name, and it is not
/// decoration: it becomes the context line the agent reads above the
/// transport failure, which is the only thing distinguishing "your clone's
/// source could not be read" from "your clone could not be created".
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
struct ReadOnlyPhase(&'static str);

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
///
/// Both ids go through [`escape_for_log`] on the way into that line. The
/// relay's own `validate_agent_verb` already refuses a target carrying a
/// `Cc` control character, so this is not the only thing standing between a
/// hostile id and the log — but it is the only one that covers the rest of
/// Unicode's presentation-bending characters, and it is the only one at all
/// for `asking`, which arrives from the supervisor's hello rather than from
/// a validated verb field.
fn resolve_target(target: Option<String>, asking: &str, verb: &str) -> String {
    let target = target.unwrap_or_else(|| asking.to_string());
    info!(
        asking = escape_for_log(asking).as_str(),
        target = escape_for_log(&target).as_str(),
        verb,
        "an agent is acting on a session"
    );
    target
}

/// Render an id for the AUDIT LOG with everything that could forge the
/// line's presentation replaced by a visible `\u{…}` escape.
///
/// Only for logging. The value the caller goes on to route with is the
/// original, because an escaped id is not the id.
///
/// Two families are escaped, and the second is why this exists rather than
/// a bare `is_control` filter. The first is Unicode's `Cc` category
/// ([`char::is_control`]) — a newline that forges a whole extra log line
/// being the case that matters. The second is the set of characters that
/// are not control codes at all but still change what a reader SEES: the
/// bidi overrides and isolates (U+202A–U+202E, U+2066–U+2069, U+061C),
/// which can silently reverse the apparent order of a line so an id reads
/// as another id; the zero-width and invisible formatting characters
/// (U+200B–U+200F, U+2060–U+2064, U+00AD, U+FEFF), which let two different
/// ids render identically; and the line/paragraph separators (U+2028,
/// U+2029), which some log viewers break lines on exactly as they would on
/// a newline.
///
/// An explicit list rather than a whole-category test because the standard
/// library exposes no Unicode general-category API, and the alternative
/// available without a dependency — escaping everything non-ASCII — would
/// mangle every legitimately non-English id for no gain. This is the set
/// with a known presentation attack behind it.
fn escape_for_log(id: &str) -> String {
    fn is_presentation_bending(c: char) -> bool {
        matches!(
            c,
            '\u{00AD}'
                | '\u{061C}'
                | '\u{200B}'..='\u{200F}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{2069}'
                | '\u{FEFF}'
        )
    }
    // The common case is an id with nothing to escape, and the borrow-free
    // early return keeps this off the allocation path for it.
    if !id
        .chars()
        .any(|c| c.is_control() || is_presentation_bending(c))
    {
        return id.to_string();
    }
    let mut out = String::with_capacity(id.len());
    for c in id.chars() {
        if c.is_control() || is_presentation_bending(c) {
            out.push_str(&format!("\\u{{{:04x}}}", c as u32));
        } else {
            out.push(c);
        }
    }
    out
}

/// One `create` verb's fields, moved out of [`AgentVerb`] so the handler
/// arm stays a dispatch and the policy lives in [`create_for_agent`].
///
/// A struct rather than six parameters because four of them are
/// `Option<String>`: a call site that transposed `profile_name` and
/// `invocation`, or `title` and `intent_key`, would compile and be wrong.
struct CreateRequest {
    host: Option<String>,
    cwd: String,
    profile_name: Option<String>,
    invocation: Option<String>,
    title: Option<String>,
    intent_key: Option<String>,
}

/// One `clone` verb's fields. See [`CreateRequest`] for why it is a struct.
struct CloneRequest {
    host: Option<String>,
    cwd: Option<String>,
    title: Option<String>,
    intent_key: Option<String>,
}

/// The host a creating verb targets: the named one, or the ASKING
/// session's own when the verb named none.
///
/// Resolved against `hosts::host_views` — the very listing `AgentVerb::
/// Hosts` is projected from — rather than against the registry directly,
/// so the names an agent can successfully pass here are exactly the names
/// it was shown. Any other source would let the two drift, and the drift
/// would surface as an agent being refused a host it can see in its own
/// `farhelm agent hosts` output.
///
/// Matching is EXACT: no case folding, no trimming, no prefix match. Host
/// names are user-chosen and an ssh host's name is its destination, so
/// `builder` and `Builder` are two names a fleet may legitimately carry;
/// guessing between them would put a session on the wrong machine, which
/// is precisely the mistake SPEC.md's ask-don't-guess rule forbids.
///
/// An AMBIGUOUS name is refused, never arbitrated, and that is the one
/// behavior here worth stating twice. Display names are not unique by
/// construction: the local row renders as `this machine`, and nothing stops
/// an ssh destination from being spelled exactly that. A `.find` would hand
/// the create to whichever row the listing happened to order first, which
/// is a session started on a machine nobody chose — the same class of
/// mistake as resolving a profile id across hosts, and the same answer.
///
/// Returns the display NAME alongside the id because every refusal below
/// this point names the host it was aimed at, and for `host: None` that
/// name is something only this lookup knows.
async fn resolve_host(
    state: &AppState,
    origin: AgentOrigin,
    name: Option<String>,
) -> anyhow::Result<(HostId, String)> {
    let views = crate::hosts::host_views(state).await?;
    match name {
        None => views
            .iter()
            .find(|view| view.id == origin.host)
            .map(|view| (view.id, view.name.clone()))
            .ok_or_else(|| {
                // The asking session's own host has no registry row any
                // more — removed while the request was in flight. Not
                // `NotFound` on a name the caller supplied (it supplied
                // none), so it is reported as the transient it is.
                anyhow::Error::new(crate::SupervisorError {
                    kind: ErrorKind::Unavailable,
                    message: "this session's own host is no longer registered with the helm"
                        .to_string(),
                })
            }),
        Some(name) => {
            let mut matches = views.iter().filter(|view| view.name == name);
            let Some(view) = matches.next() else {
                let names: Vec<&str> = views.iter().map(|view| view.name.as_str()).collect();
                return Err(anyhow::Error::new(crate::SupervisorError {
                    kind: ErrorKind::NotFound,
                    // The name is quoted back so a typo is visible as a
                    // typo, and the alternatives are listed because the
                    // agent's next move is to pick one — the same shape
                    // the supervisor's own profile-name refusal takes.
                    message: format!(
                        "no host named {name:?} is registered with this helm; known hosts: {}{}",
                        known_hosts(&names),
                        unnameable_hosts(&names).unwrap_or_default()
                    ),
                }));
            };
            if matches.next().is_some() {
                let duplicates = views.iter().filter(|view| view.name == name).count();
                return Err(anyhow::Error::new(crate::SupervisorError {
                    kind: ErrorKind::Conflict,
                    // `Conflict`, not `NotFound` or `InvalidRequest`: the
                    // request is well formed and the fleet is the thing
                    // that is incoherent, which is the same reading
                    // `HostStoreError::SessionOwnerAmbiguous` gets. The
                    // remedy is a rename the agent cannot perform, so the
                    // message says who has to do it.
                    message: format!(
                        "{duplicates} hosts registered with this helm are named {name:?}, so the \
                         target is ambiguous; rename one in the Farhelm UI, or name a host whose \
                         display name is unique"
                    ),
                }));
            }
            Ok((view.id, view.name.clone()))
        }
    }
}

/// The sentence a not-found refusal adds when the fleet holds a host no
/// agent could have named, or `None` when every name is usable.
///
/// The listing an agent reads is a terminal table, so a display name
/// carrying a control character is shown ESCAPED — and the relay refuses a
/// `--host` value containing one outright, which means such a host is
/// visible and permanently unreachable. Saying so is the difference between
/// an agent retrying a name it will never get right and an agent reporting
/// something an operator can fix; the fix is a rename, which is not a verb
/// an agent has.
///
/// Only the control-character case is called out. Length is no longer a way
/// to be unnameable: the hosts table prints its NAME column whole (see
/// `render_agent_reply`), so a long name is still exactly copyable.
fn unnameable_hosts(names: &[&str]) -> Option<String> {
    let count = names
        .iter()
        .filter(|name| name.chars().any(char::is_control))
        .count();
    (count > 0).then(|| {
        format!(
            " ({count} further host(s) carry control characters in their display names and cannot \
             be named as a target at all; rename them in the Farhelm UI)"
        )
    })
}

/// The known-host list an unknown-name refusal ends with, under a fixed
/// byte budget.
///
/// Budgeted rather than joined outright because this string is built from
/// the WHOLE registry and ends up inside an `AgentOutcome` that has to fit
/// in one 8 MiB protocol frame. A fleet large enough — or carrying names
/// long enough — to blow past that turns a useful `NotFound` into the
/// generic `Internal` the oversized-response backstop substitutes, which is
/// the one outcome this diagnostic exists to avoid. Each name is also cut
/// on its own so a single enormous name cannot spend the whole allowance,
/// and the count of what was left out is reported: "and 412 more" is a
/// usable answer, a silently short list is not.
fn known_hosts(names: &[&str]) -> String {
    /// Total bytes the joined list may occupy. Small next to the frame
    /// limit on purpose — this is a hint for a reader, and a listing verb
    /// is the right way to see the whole fleet.
    const BUDGET: usize = 4096;
    /// Longest any single name is rendered at, so one pathological name
    /// cannot crowd out every other.
    const PER_NAME: usize = 128;

    /// One name, cut on a CHARACTER boundary so the result is still UTF-8,
    /// with the cut made visible. A silently shortened host name is worse
    /// than an obviously shortened one: the reader's next move is to type
    /// it back as `--host`.
    fn capped(name: &str) -> String {
        match name.char_indices().nth(PER_NAME) {
            None => name.to_string(),
            Some((cut, _)) => format!("{}…", &name[..cut]),
        }
    }

    if names.is_empty() {
        return "none".to_string();
    }
    let mut out = String::new();
    let mut shown = 0usize;
    for name in names {
        let name = capped(name);
        let separator = if out.is_empty() { "" } else { ", " };
        if !out.is_empty() && out.len() + separator.len() + name.len() > BUDGET {
            break;
        }
        out.push_str(separator);
        out.push_str(&name);
        shown += 1;
    }
    let omitted = names.len() - shown;
    if omitted > 0 {
        out.push_str(&format!(", and {omitted} more"));
    }
    out
}

/// Name the HOST in a refusal the target supervisor produced.
///
/// The target's own refusals are written for a caller that already knows
/// which machine it is talking to, and say "this host". An agent does not:
/// it named a host by display name and may have several in view, so a bare
/// "no profile named X exists on this host" leaves it unable to tell a typo
/// in the profile from a typo in the host. This wraps rather than rewrites,
/// so the target's sentence survives verbatim inside the chain and
/// `crate::error_kind` still finds the [`crate::SupervisorError`] under it —
/// the classification an agent acts on is the target's, not this helm's.
///
/// One refusal it wraps is the helm's own: the clone verb's
/// `accept_result` veto, which travels out of `do_create_session` like any
/// other create failure. That one is about the same machine, and it is
/// written in the target's voice ("this host") so the composed sentence
/// reads as one.
fn on_host<T>(result: anyhow::Result<T>, host_name: &str) -> anyhow::Result<T> {
    result.map_err(|error| error.context(format!("on host {host_name:?}")))
}

/// `create`: one session on any host, from a profile name, a raw
/// invocation, or the target host's remembered default.
///
/// ## The selector rules, and why each refusal is loud
///
/// Naming BOTH a profile and an invocation is refused rather than
/// arbitrated, exactly as the REST create refuses the same body: a profile
/// already says what to run, so there is no honest merge, and picking a
/// winner would launch something the caller did not choose.
///
/// Naming NEITHER falls back to this host's remembered default profile —
/// the same one the create dialog preselects and the same intent
/// `farhelm spawn` with no `--agent` has. The fallback is the HELM's
/// memory (helm.db's `remembered_profiles`), not the target supervisor's
/// own last-used profile, and the difference is worth knowing: a session
/// created on that host by some other client updates the supervisor's
/// notion and not this one's. The helm's is the right answer here because
/// this whole feature is an agent talking to the helm, and it is the
/// helm's create dialog the user would otherwise have used.
///
/// A remembered default naming a profile that has since been DELETED is
/// not softened: the create fails with the target supervisor's own
/// "no such profile" refusal. SPEC.md's ask-don't-guess rule applies, and
/// silently falling through to some other profile is the guess it forbids.
///
/// ## A NAME travels; this helm never turns it into an id
///
/// `--profile` is forwarded as `CreateMode::ProfileName` and resolved by
/// the TARGET supervisor inside creation. This helm reading the catalog
/// first would put two windows around the lookup — one before the create is
/// sent, one before the target reserves the intent key — and both are real:
/// a rename in the first makes the create fail on a name the agent can
/// still see, and an edit in the second launches settings the agent's name
/// no longer describes. The refusals (no such name, an ambiguous name) are
/// the target's own, wrapped by [`on_host`] so the agent can tell which
/// machine said it.
///
/// ## What a keyed retry is bound to
///
/// The target fingerprints the SELECTOR it was sent, so two attempts under
/// one `--idempotency-key` must carry the SAME selector to replay rather
/// than conflict. That is why the name is forwarded rather than resolved:
/// a name is stable across attempts, whereas a helm-resolved id would
/// change under a rename and turn a replay into a fingerprint conflict.
/// The one selector this helm still resolves is the "no selector" fallback,
/// and it carries the corresponding caveat: if the remembered default
/// changes between two attempts under one key, the second attempt arrives
/// with a different fingerprint and is refused as a conflict rather than
/// replayed. An agent that wants a retry to be safe should name its
/// selector explicitly. `farhelm spawn` binds its keys the same way — the
/// supervisor's fingerprint covers the selector, not a snapshot of what the
/// selector once meant.
///
/// ## What this function does NOT decide
///
/// The working directory. A directory that does not exist on the target is
/// the TARGET supervisor's refusal, reported verbatim through
/// [`AgentOutcome::Err`] like every other create precondition — this side
/// never stats a path on another machine, and could not.
///
/// The INSTALLATION behind the host. `resolve_host` answers with the
/// registry's durable [`HostId`], and `sessions::host_client` takes the
/// connection currently published for that row — exactly what the lifecycle
/// verbs do through `route_session`, and exactly what the REST create does
/// through `create_target`. A row retargeted or adopted between the two
/// reads sends the create to the new installation, and nothing here pins an
/// incarnation to prevent that. The claim is what makes it safe rather than
/// silent: every write below revalidates against the connection the create
/// was actually sent on, so the create either lands on one coherent
/// installation or fails. Pinning an incarnation for these two verbs alone
/// would give the agent surface a stricter contract than the UI's own
/// create, which is not a difference this feature should invent.
async fn create_for_agent(
    state: &AppState,
    origin: AgentOrigin,
    asking_session: &str,
    request: CreateRequest,
) -> anyhow::Result<AgentReply> {
    let (host, host_name) = resolve_host(state, origin, request.host).await?;
    // The claim and the client come from ONE read, which is what lets every
    // write the create goes on to make revalidate against the connection it
    // was actually sent on (see `sessions::host_client`).
    let (claim, client) = crate::sessions::host_client(state, host)?;
    let mode = match (request.profile_name, request.invocation) {
        (Some(_), Some(_)) => {
            return Err(anyhow::Error::new(crate::SupervisorError {
                kind: ErrorKind::InvalidRequest,
                message: "a create names either a profile or an invocation, never both: a \
                          profile already says what to run, and there is no honest way to merge \
                          the two"
                    .to_string(),
            }));
        }
        (Some(profile_name), None) => crate::sessions::CreateMode::ProfileName(profile_name),
        (None, Some(invocation)) => crate::sessions::CreateMode::Raw(invocation),
        (None, None) => {
            let default = state.store.remembered_profile(host).await?;
            crate::sessions::CreateMode::Profile(default.ok_or_else(|| {
                anyhow::Error::new(crate::SupervisorError {
                    kind: ErrorKind::InvalidRequest,
                    message: format!(
                        "no session has been created from a profile on host {host_name:?} yet, \
                         so there is no default to fall back on; name a profile or an invocation"
                    ),
                })
            })?)
        }
    };
    // The same paper trail [`resolve_target`] leaves for the lifecycle
    // verbs, and the values are safe for the same reasons: the supervisor
    // validated `asking_session` before forwarding, and `host_name` is the
    // REGISTRY's own rendering of the matched row rather than the string
    // the request carried — `resolve_host` returns the view's name, not the
    // caller's, so nothing attacker-chosen reaches this line.
    info!(
        asking = asking_session,
        host = host_name.as_str(),
        verb = "create",
        "an agent is creating a session"
    );
    let session = on_host(
        crate::sessions::do_create_session(
            state,
            &claim,
            &client,
            crate::sessions::CreateSpec {
                cwd: request.cwd,
                mode,
                title: request.title,
                cols: crate::sessions::default_cols(),
                rows: crate::sessions::default_rows(),
                intent_key: request.intent_key,
                // Neither override is reachable from an agent, and that is
                // deliberate rather than an omission: both are
                // profile-editor concerns (which integrated kind this is,
                // and what its resume invocation looks like), and a profile
                // already states them.
                agent_kind: None,
                resume_template: None,
                // `create` names a directory and an agent rather than a
                // session, so no answer of the target's is forbidden — a
                // keyed replay is the caller's own earlier create coming
                // back, which is what the key is for. Contrast
                // `clone_for_agent`, whose replay can be the ASKING session.
                accept_result: None,
            },
        )
        .await,
        &host_name,
    )?;
    Ok(agent_created_reply(
        state,
        &claim,
        session,
        origin.host,
        asking_session,
    ))
}

/// `clone`: another session like the asking one, on any host.
///
/// ## The source is read LIVE
///
/// From the asking session's own host, by draining its session list — the
/// same live read `sessions::get_session` performs for a connected host,
/// and for the same reason: the helm's cache is for the stale list, not a
/// serving layer, and a clone built from a cached row could copy a title
/// or a working directory the session no longer has.
///
/// ## Agent resolution, in order, with no silent fallback
///
/// 1. **Same host: ALWAYS by ID, whatever became of the profile.** The
///    target and the source are one supervisor, so the snapshotted id is
///    exact and is the thing being followed — a rename does not change it,
///    and neither does a DELETE. A deleted id is sent anyway and comes back
///    as the target's ordinary "no such profile" refusal. That is the point
///    rather than an oversight: falling through to the snapshotted NAME
///    would let a profile created since, reusing the old name, launch
///    settings nobody chose, which is the exact substitution SPEC.md's
///    ask-don't-guess rule forbids. The refusal is loud, and the agent can
///    name a profile itself with `create`.
/// 2. **Another host: by NAME, resolved by the TARGET.** The name travels
///    as `CreateMode::ProfileName`; no match and an ambiguous match are
///    both the target's refusals, wrapped by [`on_host`] so the agent knows
///    which machine answered. NEVER a quiet downgrade to the source's raw
///    invocation: that invocation was written for the source machine, and
///    on another one it may name a binary that is absent, a different
///    build, or one that takes different flags. SPEC.md's agent section
///    ("The agent is resolved by NAME, never by profile id") states this as
///    the contract.
/// 3. **A source with no profile at all: its raw invocation.** There is no
///    name to resolve and nothing to guess — the user created that session
///    from a command line, and a clone of it is that command line.
///
/// ## What is copied and what can be overridden
///
/// The working directory and the title default to the source's, which is
/// what makes a same-host clone mean "another session on this project".
/// Both can be overridden. A directory that does not exist on the TARGET
/// is the target supervisor's own refusal, reported verbatim — cloning
/// onto a machine that does not have the source's checkout is a real and
/// expected failure, and inventing a directory would be worse.
///
/// ## A KNOWN GAP: a raw source's integration overrides are not copied
///
/// A profile-backed clone carries everything, because the profile states
/// it. A RAW clone copies the invocation and nothing else, so the target
/// re-derives the integrated kind from the invocation's first token and
/// takes that kind's default resume template. A source created with an
/// explicit `agent_kind` — including the explicit "no integration" the
/// tri-state can express — or with a custom `resume_template` therefore
/// clones into a session whose conversation capture, status classification
/// and restart behavior may differ from the original's.
///
/// It is stated rather than fixed because the fix is not local: nothing
/// this function can read carries those values. `SessionInfo` — the shape
/// `drain_sessions` returns and the only view the helm has of another
/// host's session — exposes `invocation` and `source_profile` and no
/// integration fields at all, so copying them means adding them to the
/// wire, populating them at every point a supervisor builds a session row
/// (create, reload, restart), and persisting them for a reload to find.
/// Refusing the raw clone instead is not available either: SPEC.md's agent
/// section promises that a session created from a raw invocation "clones as
/// that invocation".
async fn clone_for_agent(
    state: &AppState,
    origin: AgentOrigin,
    asking_session: &str,
    request: CloneRequest,
) -> anyhow::Result<AgentReply> {
    // The asking session's own host, by construction of where the upcall
    // arrived (see the module docs) — so this needs no owner lookup, unlike
    // the lifecycle verbs, which may name a session anywhere.
    let (_source_claim, source_client) = crate::sessions::host_client(state, origin.host)?;
    // Marked as the read-only phase it is: this listing runs before any
    // create is dispatched, so a transport failure here is retry-safe and
    // must not inherit the clone's own outcome-unknown vocabulary. See
    // [`ReadOnlyPhase`].
    let source = crate::manager::drain_sessions(&source_client)
        .await
        .map_err(|error| error.context(ReadOnlyPhase("reading the session to clone")))?
        .into_iter()
        .find(|session| session.id == asking_session)
        .ok_or_else(|| {
            anyhow::Error::new(crate::SupervisorError {
                kind: ErrorKind::NotFound,
                message: "this session's own host no longer lists it, so there is nothing to \
                          clone"
                    .to_string(),
            })
        })?;

    let (host, host_name) = resolve_host(state, origin, request.host).await?;
    let (claim, client) = crate::sessions::host_client(state, host)?;
    let mode = match &source.source_profile {
        // Same host: the id, unconditionally. `ProfileExistence` is not
        // consulted at all — see this function's resolution order for why a
        // deleted id must reach the target as an id rather than degrade
        // into its own stale name.
        Some(profile) if host == origin.host => {
            crate::sessions::CreateMode::Profile(profile.id.clone())
        }
        Some(profile) => crate::sessions::CreateMode::ProfileName(profile.name.clone()),
        None => crate::sessions::CreateMode::Raw(source.invocation.clone()),
    };
    // See `create_for_agent`'s own note on why both logged values are safe.
    info!(
        asking = asking_session,
        host = host_name.as_str(),
        verb = "clone",
        "an agent is cloning a session"
    );
    let session = on_host(
        crate::sessions::do_create_session(
            state,
            &claim,
            &client,
            crate::sessions::CreateSpec {
                cwd: request.cwd.unwrap_or(source.cwd),
                mode,
                // The source's title is copied VERBATIM, empty string
                // included: a clone that let the target derive a title from
                // the directory would silently rename the copy, which is
                // the one thing a user reading two rows side by side would
                // notice first.
                title: Some(request.title.unwrap_or(source.title)),
                cols: crate::sessions::default_cols(),
                rows: crate::sessions::default_rows(),
                intent_key: request.intent_key,
                agent_kind: None,
                resume_template: None,
                // A clone that comes back as the ASKING session is refused
                // rather than reported, and this is not a defensive nicety —
                // it is reachable by ordinary means. A same-host clone with
                // no overrides reconstructs exactly the fingerprint that
                // created the asking session in the first place: same
                // directory, same title, same profile id, no parent. An agent
                // that passes the key its own create used therefore hits a
                // legitimate reservation REPLAY at the target, which answers
                // with the original session — and this helm would otherwise
                // go on to report the asking session as a session it just
                // made, with `current: true`, to a caller whose next move is
                // to act on the id it was handed as if it were a new one.
                // Answering "no new session was created" is the only honest
                // outcome, and `Conflict` says the caller's key is the thing
                // to change.
                //
                // It rides `accept_result` rather than an `if` after the call
                // because refusing AFTER the create's bookkeeping is refusing
                // too late: the seed and the remembered-default write are
                // durable effects of a create this very refusal says did not
                // happen. See `sessions::do_create_session`'s "Two phases".
                accept_result: Some(Box::new({
                    let asking = asking_session.to_string();
                    move |created: &farhelm_proto::SessionInfo| {
                        if created.id != asking {
                            return Ok(());
                        }
                        // "this host" rather than the name, because `on_host`
                        // wraps this refusal with the host it is about,
                        // exactly as it wraps the target's own.
                        Err(anyhow::Error::new(crate::SupervisorError {
                            kind: ErrorKind::Conflict,
                            message: "the idempotency key replayed the create that made this \
                                      session, so no copy was made; retry the clone with a key \
                                      that has not been used on this host, or with none at all"
                                .to_string(),
                        }))
                    }
                })),
            },
        )
        .await,
        &host_name,
    )?;
    Ok(agent_created_reply(
        state,
        &claim,
        session,
        origin.host,
        asking_session,
    ))
}

/// Project a session a lifecycle verb just mutated into the same
/// [`AgentSession`] shape the `sessions` listing uses, so a rename/archive
/// reply and a later listing agree about the row they both describe.
///
/// Built from the [`crate::manager::SessionClaim`] `route_session` already
/// resolved while performing the mutation, rather than by re-listing the
/// fleet: the mutation's own reply already carries the fresh `SessionInfo`,
/// and asking again would be an extra round trip to relearn what the
/// supervisor just said.
///
/// The host's display NAME, though, is NOT read from a fresh, unchecked
/// snapshot lookup — a prior version did exactly that, and it was wrong
/// twice over. A snapshot lookup that finds no row for `claim.host` (the
/// row was removed between the mutation returning and this projection
/// running) used to fall back to an empty string while still asserting
/// `stale: false` — a silent lie about a name nobody could vouch for. And a
/// snapshot lookup that DOES find a row says nothing about whether it is
/// the SAME install `route_session` just sent the mutation to: a retarget
/// or adoption in that same window keeps the row but swaps the machine
/// behind it, so a fresh lookup would combine THIS session's freshly
/// mutated data with a DIFFERENT connection's host name and call the
/// mix current.
///
/// The fix is to require the snapshot's own
/// [`crate::manager::HostSnapshot::incarnation`] to still match
/// [`crate::manager::SessionClaim::incarnation`] —
/// the same connection identity `route_session` captured before the
/// mutation went out — before trusting its name at all. A mismatch (or no
/// row) means there is no name this reply can vouch for, so it answers
/// `host: None` and marks itself `stale` rather than asserting freshness
/// the mutation cannot back up. The `None` is load-bearing: this used to be
/// an empty string, which a reader could not tell from a host whose name
/// really was empty.
fn agent_session_reply(
    state: &AppState,
    claim: &crate::manager::SessionClaim,
    info: farhelm_proto::SessionInfo,
    asking_host: HostId,
    asking_session: &str,
) -> AgentReply {
    AgentReply::Session {
        session: agent_row_of_mutation(state, claim, info, asking_host, asking_session),
    }
}

/// [`agent_session_reply`]'s twin for the two CREATING verbs: the same row,
/// under [`AgentReply::Created`]'s tag.
///
/// A separate function rather than a boolean on the one above, because the
/// tag is the only thing distinguishing "the row you asked me to change"
/// from "what your creating verb produced" and a caller keys on it — see
/// [`AgentReply::Created`]'s own docs, including why the tag makes no
/// novelty claim (a keyed replay returns an existing session under it). A
/// flag parameter at each call site would be one `true`/`false` away from
/// telling an agent it created something it merely renamed.
fn agent_created_reply(
    state: &AppState,
    claim: &crate::manager::SessionClaim,
    info: farhelm_proto::SessionInfo,
    asking_host: HostId,
    asking_session: &str,
) -> AgentReply {
    AgentReply::Created {
        session: agent_row_of_mutation(state, claim, info, asking_host, asking_session),
    }
}

/// The row both reply shapes above carry — the projection, and the
/// incarnation check that decides whether its host name can be vouched
/// for. See [`agent_session_reply`] for the whole reasoning.
fn agent_row_of_mutation(
    state: &AppState,
    claim: &crate::manager::SessionClaim,
    info: farhelm_proto::SessionInfo,
    asking_host: HostId,
    asking_session: &str,
) -> AgentSession {
    let current =
        state.manager.snapshots().into_iter().find(|snapshot| {
            snapshot.id == claim.host && snapshot.incarnation == claim.incarnation
        });
    let (host_name, stale) = match current {
        Some(snapshot) => (
            Some(crate::aggregate::host_display_name(
                snapshot.kind,
                snapshot.destination.as_deref(),
            )),
            false,
        ),
        None => (None, true),
    };
    let row = crate::aggregate::SessionRow {
        info,
        host: claim.host,
        host_identity: None,
        // The row's own copy of the name is unused by the projection below,
        // which takes it as a parameter precisely so absence can be said
        // out loud; this keeps the struct's field consistent with what is
        // reported rather than leaving a stale second copy beside it.
        host_name: host_name.clone().unwrap_or_default(),
        stale,
    };
    agent_session(&row, host_name, asking_host, asking_session)
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
            let row = agent_session(
                row,
                Some(row.host_name.clone()),
                asking_host,
                asking_session,
            );
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
///
/// The host NAME is a parameter rather than read from `row.host_name`, and
/// the redundancy is deliberate: `SessionRow` has no way to say "there is
/// no name I can vouch for", so the reply path that discovers exactly that
/// ([`agent_session_reply`], when the mutation's connection is no longer
/// the row's current one) had to encode absence as an empty string —
/// indistinguishable, on the wire, from a host actually named nothing.
/// Passing the name in makes the absence a `None` the caller states
/// explicitly. Listing callers pass the row's own name, which is always one
/// the fleet snapshot vouched for.
fn agent_session(
    row: &crate::aggregate::SessionRow,
    host_name: Option<String>,
    asking_host: HostId,
    asking_session: &str,
) -> AgentSession {
    AgentSession {
        id: row.info.id.clone(),
        host: host_name,
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

    /// [`agent_session`] as the LISTING path calls it: with the row's own
    /// host name, which is the name a fleet snapshot already vouched for.
    ///
    /// A helper rather than the argument spelled out at a dozen call sites
    /// because the interesting variable in these tests is never the name —
    /// it is `current`, `agent`, `archived`, `stale`. The one caller that
    /// passes something else is `agent_session_reply`, whose own test
    /// exercises the `None` case deliberately.
    fn projected(
        row: &crate::aggregate::SessionRow,
        asking_host: HostId,
        asking_session: &str,
    ) -> AgentSession {
        agent_session(
            row,
            Some(row.host_name.clone()),
            asking_host,
            asking_session,
        )
    }

    /// Spec: a lost target-supervisor connection is classified by PHASE and
    /// by verb class — never enqueued is retry-safe `Unavailable` for both
    /// classes, and a MUTATION that was enqueued and never answered is
    /// `Timeout` carrying the check-before-retrying remedy.
    ///
    /// This is the far hop's half of the vocabulary the near hop already
    /// speaks (`service::agent_relay`), and the table is asserted whole
    /// rather than by its interesting cell: the three non-mutating outcomes
    /// are what keep the remedy from spreading to callers who cannot act on
    /// it, and the `Internal` fallback is what keeps this classifier from
    /// swallowing failures that have nothing to do with the transport — an
    /// unknown session, a refused title — whose own kinds a caller needs.
    ///
    /// The error is wrapped in anyhow CONTEXT rather than handed over bare,
    /// because that is how it arrives in production (every call site adds
    /// which operation it was attempting) and because a `downcast_ref` on
    /// the chain alone would not see through it — the reason
    /// [`crate::find_cause`] exists.
    #[test]
    fn a_dead_target_connection_is_classified_by_phase_and_verb_class() {
        let wrapped = |lost: crate::SupervisorTransportError| {
            anyhow::Error::new(lost).context("stopping session s9 on host builder")
        };

        for mutating in [true, false] {
            let outcome =
                transport_outcome(&wrapped(crate::SupervisorTransportError::NotSent), mutating)
                    .expect("a transport failure must be classified");
            let AgentOutcome::Err { kind, message } = outcome else {
                panic!("a dead connection cannot succeed");
            };
            assert_eq!(
                kind,
                ErrorKind::Unavailable,
                "nothing was sent, so nothing happened, whatever the verb was"
            );
            assert!(
                !message.contains(farhelm_proto::AGENT_MUTATION_UNKNOWN_REMEDY),
                "an unsent request has no outcome to check: {message}"
            );
            assert!(
                message.contains("stopping session s9"),
                "the chain names what was attempted and must survive: {message}"
            );
        }

        let AgentOutcome::Err { kind, message } = transport_outcome(
            &wrapped(crate::SupervisorTransportError::SentUnanswered),
            true,
        )
        .expect("a transport failure must be classified") else {
            panic!("a dead connection cannot succeed");
        };
        assert_eq!(
            kind,
            ErrorKind::Timeout,
            "a mutation the target may already have applied is outcome-unknown, not retry-safe"
        );
        assert!(
            message.contains(farhelm_proto::AGENT_MUTATION_UNKNOWN_REMEDY),
            "and must say what to do about it: {message}"
        );

        let AgentOutcome::Err { kind, .. } = transport_outcome(
            &wrapped(crate::SupervisorTransportError::SentUnanswered),
            false,
        )
        .expect("a transport failure must be classified") else {
            panic!("a dead connection cannot succeed");
        };
        assert_eq!(
            kind,
            ErrorKind::Unavailable,
            "a listing has nothing to double-apply however far it got"
        );

        assert!(
            transport_outcome(&anyhow::anyhow!("no such session: s9"), true).is_none(),
            "a refusal that is not a transport failure must keep its own classification"
        );
    }

    /// Spec: a target supervisor that answers a MUTATION with a correlated
    /// reply of the wrong variant is the same outcome-unknown ending a dead
    /// connection is, while the same fault on a LISTING keeps its own
    /// classification.
    ///
    /// The asymmetry is the content. Both cases are a peer violating the
    /// protocol, but only one of them is a question about durable state: the
    /// `stop` was sent, and a peer broken enough to answer it with a rename
    /// confirmation is exactly as likely to have stopped the session first
    /// as not. An `Internal` there — which is what an untyped "unexpected
    /// reply" error classifies as — hands the agent the one kind that says
    /// nothing about retrying, for the situation where that is the whole
    /// question. A listing has nothing to double-apply, so this classifier
    /// declines it and `error_kind`'s `Internal` stands, which is the honest
    /// word for a peer that should not have said that.
    #[test]
    fn a_wrong_lifecycle_reply_is_outcome_unknown_for_a_mutation_only() {
        let wrapped = anyhow::Error::new(crate::SupervisorTransportError::SentWrongReply {
            request: "StopSession",
            reply: "SessionRenamed",
        })
        .context("stopping session s9 on host builder");

        let AgentOutcome::Err { kind, message } = transport_outcome(&wrapped, true)
            .expect("a wrong reply to a mutation must be classified")
        else {
            panic!("a wrong reply cannot succeed");
        };
        assert_eq!(
            kind,
            ErrorKind::Timeout,
            "the request was sent, so its outcome is unknown rather than retry-safe"
        );
        assert!(
            message.contains(farhelm_proto::AGENT_MUTATION_UNKNOWN_REMEDY),
            "and must say what to do about it: {message}"
        );
        assert!(
            message.contains("stopping session s9"),
            "the chain names what was attempted and must survive: {message}"
        );

        assert!(
            transport_outcome(&wrapped, false).is_none(),
            "a listing's wrong reply is a protocol fault, not an unavailability"
        );
    }

    /// Spec: a transport failure marked [`ReadOnlyPhase`] is classified by
    /// the LISTING rules even when the verb around it is mutating — an
    /// unusable reply keeps its own `Internal`, and a lost one is retry-safe
    /// `Unavailable` with no mutation remedy attached.
    ///
    /// The phase and the verb are different questions, and `clone` is where
    /// they come apart: it SNAPSHOTS its source with a plain listing before
    /// dispatching any create. A classifier that read only the verb's class
    /// would answer a source-snapshot failure with "delivered, outcome
    /// unknown — go check the fleet before retrying", about a create that
    /// provably never left this process. That is worse than unhelpful: the
    /// remedy costs the agent a fleet listing and a decision, and the
    /// honest answer is that retrying is free.
    ///
    /// Pinned here rather than end to end because the spliced fleet harness
    /// answers `ListSessions` itself on every scripted peer's behalf
    /// (`rest_harness`'s module docs), so no handler-level fixture can make
    /// a source snapshot fail at the transport. The error below is built the
    /// way `clone_for_agent` builds it — the marker over the drain's own
    /// context over the typed cause — so the layering it depends on is part
    /// of what is asserted.
    #[test]
    fn a_read_only_phase_of_a_mutating_verb_is_classified_as_a_listing() {
        let snapshot_failure = |lost: crate::SupervisorTransportError| {
            anyhow::Error::new(lost)
                .context("listing a page of the host's sessions")
                .context(ReadOnlyPhase("reading the session to clone"))
        };

        assert!(
            transport_outcome(
                &snapshot_failure(crate::SupervisorTransportError::SentWrongReply {
                    request: "ListSessions",
                    reply: "SessionCreated",
                }),
                true,
            )
            .is_none(),
            "a source snapshot's wrong reply is a protocol fault, not the clone's outcome-unknown"
        );

        for lost in [
            crate::SupervisorTransportError::NotSent,
            crate::SupervisorTransportError::SentUnanswered,
        ] {
            let error = snapshot_failure(lost.clone());
            let AgentOutcome::Err { kind, message } =
                transport_outcome(&error, true).expect("a transport failure must be classified")
            else {
                panic!("a dead connection cannot succeed");
            };
            assert_eq!(
                kind,
                ErrorKind::Unavailable,
                "no create has been dispatched yet, so {lost:?} is retry-safe"
            );
            assert!(
                !message.contains(farhelm_proto::AGENT_MUTATION_UNKNOWN_REMEDY),
                "a snapshot that failed has no mutation outcome to check: {message}"
            );
            assert!(
                message.contains("reading the session to clone"),
                "the agent must be told WHICH half of the clone failed: {message}"
            );
        }
    }

    /// Spec: [`escape_for_log`] neutralizes the two families that can forge
    /// an audit line's PRESENTATION — control characters and the invisible
    /// or direction-changing formatting characters — and leaves ordinary
    /// text, including ordinary non-ASCII text, exactly as it was.
    ///
    /// The last clause is half the point. An escaper that mangled every
    /// non-ASCII character would be trivially "safe" and would make the
    /// audit trail useless for any fleet whose session ids or hosts are not
    /// English, so the test pins what must NOT be touched as firmly as what
    /// must. The bidi and zero-width cases are the ones a bare
    /// `is_control` filter misses: U+202E reverses how the rest of the line
    /// renders, and U+200B lets two visibly identical ids be different
    /// strings — both of which turn "which session acted on which" into a
    /// question the log answers wrongly rather than not at all.
    #[test]
    fn log_escaping_covers_bidi_and_zero_width_as_well_as_control_characters() {
        assert_eq!(escape_for_log("ordinary-id"), "ordinary-id");
        assert_eq!(
            escape_for_log("café-Ω-日本"),
            "café-Ω-日本",
            "ordinary non-ASCII text must survive intact"
        );
        assert_eq!(
            escape_for_log("one\ntwo"),
            "one\\u{000a}two",
            "a newline must not be able to forge a second log line"
        );
        assert_eq!(
            escape_for_log("a\u{202e}b"),
            "a\\u{202e}b",
            "a bidi override must not reorder the line around it"
        );
        assert_eq!(
            escape_for_log("a\u{200b}b"),
            "a\\u{200b}b",
            "a zero-width space must not make two ids look like one"
        );
        assert_eq!(
            escape_for_log("a\u{2028}b"),
            "a\\u{2028}b",
            "a line separator breaks lines in some viewers exactly as a newline does"
        );
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
        assert_eq!(projected(&raw, 1, "s1").agent, "claude");

        let mut with_secret = session_info("s2", SessionStatus::Running);
        with_secret.invocation = "/opt/bin/codex --api-key sk-secret-value".to_string();
        let row = session_row(with_secret, "this machine");
        assert_eq!(
            projected(&row, 1, "s1").agent,
            "codex",
            "arguments must never reach this wire"
        );

        let mut unparsable = session_info("s3", SessionStatus::Running);
        unparsable.invocation = "claude --title 'unbalanced".to_string();
        let row = session_row(unparsable, "this machine");
        assert_eq!(
            projected(&row, 1, "s1").agent,
            "claude",
            "an invocation that does not shell-split still yields only its program"
        );

        for existence in [ProfileExistence::Present, ProfileExistence::Deleted] {
            let mut from_profile = session_info("s4", SessionStatus::Running);
            from_profile.source_profile = Some(snapshot("Claude", existence));
            let row = session_row(from_profile, "this machine");
            assert_eq!(
                projected(&row, 1, "s1").agent,
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
        assert!(projected(&mine, 1, "s1").current);
        assert!(!projected(&other, 1, "s1").current);
        assert_eq!(projected(&other, 1, "s1").host.as_deref(), Some("builder"));

        // Same id, different host: the collision case. `session_row` puts
        // every row on host 1, so asking as host 2 is the same shape as a
        // retained row belonging to someone else.
        assert!(
            !projected(&mine, 2, "s1").current,
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

        let filed = projected(&row, 1, "other");
        assert!(filed.archived);
        assert!(filed.stale);
        assert_eq!(
            filed.status, "running",
            "the status word stays a word; the two flags say the rest"
        );

        let live = session_row(session_info("s2", SessionStatus::Running), "this machine");
        let live = projected(&live, 1, "other");
        assert!(!live.archived);
        assert!(!live.stale);
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

    /// Spec: `agent_session_reply` names a host ONLY when the claim's own
    /// incarnation still matches the CURRENT snapshot for that host id, and
    /// otherwise reports no name at all and marks the row `stale` — rather
    /// than either an empty-string name asserted as fresh (the row's host
    /// vanished between the mutation and this projection) or a retargeted
    /// host's NEW name asserted as fresh (the row's machine changed in that
    /// same window). Both used to slip through a fresh, unchecked snapshot
    /// lookup with no incarnation check at all.
    #[tokio::test]
    async fn agent_session_reply_only_trusts_a_host_name_pinned_to_the_claims_incarnation() {
        let (h, local, _remote) = two_host_fleet().await;
        let state = &h.state;
        let info = session_info("local-live", SessionStatus::Idle);
        let live_incarnation = h
            .manager
            .status(local)
            .expect("the local host has a status")
            .incarnation;

        // The claim matches the connection that is STILL current: the
        // reply carries the real name and asserts freshness.
        let fresh_claim = crate::manager::SessionClaim {
            host: local,
            incarnation: live_incarnation,
            identity: None,
        };
        let AgentReply::Session { session } =
            agent_session_reply(state, &fresh_claim, info.clone(), local, "local-live")
        else {
            panic!("agent_session_reply always answers with a Session");
        };
        assert_eq!(session.host.as_deref(), Some("this machine"));
        assert!(!session.stale);

        // The claim's incarnation no longer matches — the shape a retarget
        // or reconnect leaves behind — so the reply carries NO name rather
        // than whatever now occupies the row, and marks itself stale.
        let retargeted_claim = crate::manager::SessionClaim {
            host: local,
            incarnation: live_incarnation + 1,
            identity: None,
        };
        let AgentReply::Session { session } =
            agent_session_reply(state, &retargeted_claim, info.clone(), local, "local-live")
        else {
            panic!("agent_session_reply always answers with a Session");
        };
        assert_eq!(
            session.host, None,
            "no name it can vouch for is said as None, not as an empty name"
        );
        assert!(session.stale);

        // A claim naming a host id that is not registered at all — the
        // shape a removed row leaves behind — is the same failure and gets
        // the same honest answer rather than a silently different one.
        let missing_host_claim = crate::manager::SessionClaim {
            host: local + 9999,
            incarnation: live_incarnation,
            identity: None,
        };
        let AgentReply::Session { session } =
            agent_session_reply(state, &missing_host_claim, info, local, "local-live")
        else {
            panic!("agent_session_reply always answers with a Session");
        };
        assert_eq!(
            session.host, None,
            "no name it can vouch for is said as None, not as an empty name"
        );
        assert!(session.stale);
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
    /// A thin wrapper over `rest_harness::spliced_helm_listing`, which
    /// builds the identical fleet for the REST-side splice tests; the only
    /// thing this adds is handing back the local host's id alongside the
    /// harness, which every lifecycle test here needs to build an
    /// `AgentOrigin` and none of `spliced_helm_listing`'s own callers do.
    ///
    /// Takes the duplex HALF rather than creating the pair itself, and that
    /// is load-bearing rather than a style choice: the splice relays the
    /// manager's own hello handshake across to whatever answers on the
    /// OTHER half (`rest_harness::run_spliced`'s crossing-hellos relay), so
    /// this function's own `await_refreshed` call (inside
    /// `spliced_helm_listing`) cannot resolve until a task is already
    /// running on that other half to complete it. A caller that built the
    /// duplex, awaited THIS function, and only then spawned its responder
    /// would deadlock — the responder's `tokio::spawn` line would never run
    /// because the awaiting test task is itself blocked inside this
    /// function. Every call site below therefore spawns its responder on
    /// the peer half FIRST and passes the other half in here second, so the
    /// two race properly instead of strictly sequencing.
    async fn spliced_local_fleet(
        client_side: tokio::io::DuplexStream,
        sessions: Vec<SessionInfo>,
    ) -> (Harness, HostId) {
        let harness = crate::rest_harness::spliced_helm_listing(client_side, sessions).await;
        let local = local_id(&harness.store).await;
        (harness, local)
    }

    /// How long a scripted supervisor gets to finish its exchange before
    /// the test calls it a routing regression.
    ///
    /// Generous rather than tight, for the same reason `silent_supervisor`'s
    /// window is: the failure this bounds is a request that was never sent
    /// at all, which is instant, so a long wait costs nothing except on a
    /// machine slow enough that the whole suite is already suspect.
    const RESPONDER_JOIN_BUDGET: std::time::Duration = std::time::Duration::from_secs(10);

    /// Join a scripted supervisor, turning "it is still waiting for a frame"
    /// into a failure with a diagnosis instead of a hang.
    ///
    /// Every lifecycle test below ends by joining the peer it scripted, and
    /// an unbounded join is the wrong shape for that: the responder blocks
    /// in `read_frame()` until the request arrives, so ANY regression that
    /// makes the handler answer without forwarding — the wrong routing
    /// decision, a target the owner cache has not learned yet, a refusal
    /// raised too early — wedges the join forever rather than failing. That
    /// is not hypothetical: the cross-host archive test hung exactly this
    /// way under a loaded suite, and the hang carried no clue as to why.
    ///
    /// The handler's own answer goes into the panic because it is the whole
    /// diagnosis. A responder that never heard anything plus an outcome of
    /// `NotFound` says "routing resolved locally"; the same silence plus an
    /// `Ok` would say something far stranger. Printing it turns a wedged
    /// run into a one-line explanation.
    async fn join_responder(responder: tokio::task::JoinHandle<()>, outcome: &AgentOutcome) {
        match tokio::time::timeout(RESPONDER_JOIN_BUDGET, responder).await {
            Ok(joined) => joined.expect("the scripted supervisor's own assertions"),
            Err(_) => panic!(
                "the scripted supervisor is still waiting for the request it was written to \
                 answer, so the handler resolved without forwarding one. It answered: {outcome:?}"
            ),
        }
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
        join_responder(responder, &outcome).await;

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
        join_responder(responder, &outcome).await;

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

    /// Spec: naming your OWN session id explicitly is the same request as
    /// omitting `--session` — there is no "you may not name yourself" rule,
    /// and [`resolve_target`] treats the two paths identically.
    ///
    /// The module docs make that promise out loud ("including the asking
    /// session's own id, which `resolve_target` accepts exactly as it would
    /// an explicit `None`"), and nothing tested it: every self-targeting
    /// test above reaches the substitution through `None`, so a future
    /// self-reference check bolted onto the explicit path would break a
    /// documented behavior with no failure to show for it. This is the
    /// twin of [`rename_with_no_session_id_targets_the_asking_session`],
    /// asserted from the same place — the far end, where the forwarded
    /// `RenameSession` names the id.
    ///
    /// `Rename` rather than `Stop`/`Archive` on purpose: it is the one
    /// lifecycle verb whose self-targeting form does not also end the
    /// asking session, so the scenario stays about target resolution.
    #[tokio::test]
    async fn naming_the_asking_session_explicitly_matches_omitting_the_target() {
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
            assert_eq!(
                session_id, "asker",
                "an explicit self-target forwards the same id the omitted form would"
            );
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
                    session_id: Some("asker".to_string()),
                    title: "explicitly self-renamed".to_string(),
                },
            )
            .await;
        join_responder(responder, &outcome).await;

        match outcome {
            AgentOutcome::Ok {
                reply: AgentReply::Session { session },
            } => {
                assert_eq!(session.id, "asker");
                assert_eq!(session.title, "explicitly self-renamed");
                assert!(
                    session.current,
                    "the asker's own row is current whichever way it was named"
                );
            }
            other => panic!("expected a session reply, got {other:?}"),
        }
    }

    /// Spec: an ALREADY-ARCHIVED session is a legal lifecycle target —
    /// `AgentSession::archived`'s own docs promise rename/stop/archive may
    /// all name one — and the helm forwards such a request rather than
    /// short-circuiting it.
    ///
    /// The promise was documented and untested, which is the combination
    /// that rots: a future "archived sessions are read-only" guard added
    /// anywhere on this path would contradict the wire's own documentation
    /// with nothing failing. Asserted at the far end, because the whole
    /// question is whether the request LEAVES the helm — a reply-only
    /// assertion could not tell a forwarded request from a locally
    /// synthesized one.
    ///
    /// `Rename` is the verb, for [`naming_the_asking_session_explicitly_matches_omitting_the_target`]'s
    /// reason: it leaves the asking session alive and keeps the test about
    /// the one property it names.
    #[tokio::test]
    async fn a_lifecycle_verb_may_target_an_already_archived_session() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        let archived = SessionInfo {
            archived: true,
            ..session("filed-away", 2)
        };
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
            assert_eq!(
                session_id, "filed-away",
                "an archived session is forwarded like any other target"
            );
            writer
                .write_control(&ControlMsg::SessionRenamed {
                    req_id,
                    session: SessionInfo {
                        title,
                        archived: true,
                        ..session("filed-away", 2)
                    },
                })
                .await
                .expect("write reply");
        });
        let (h, local) =
            spliced_local_fleet(client_side, vec![archived, session("asker", 1)]).await;

        let handler = HelmAgentRequests::for_state(&h.state);
        let outcome = handler
            .handle(
                origin_of(&h, local),
                "asker",
                AgentVerb::Rename {
                    session_id: Some("filed-away".to_string()),
                    title: "renamed while archived".to_string(),
                },
            )
            .await;
        join_responder(responder, &outcome).await;

        match outcome {
            AgentOutcome::Ok {
                reply: AgentReply::Session { session },
            } => {
                assert_eq!(session.id, "filed-away");
                assert_eq!(session.title, "renamed while archived");
                assert!(
                    session.archived,
                    "the row is still archived; renaming it does not un-file it"
                );
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
        join_responder(responder, &outcome).await;

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

    /// Spec: `Stop` routes to the target's owning host as `StopSession` and
    /// answers with [`AgentReply::Stopped`] — empty, matching the REST
    /// route's own empty-object success body.
    ///
    /// Despite this test's session starting `Running`, what it proves is
    /// ROUTING and the REPLY SHAPE, not that any process actually stopped:
    /// the scripted responder answers `SessionStopped` unconditionally,
    /// without touching a real pane or process tree, so a `Stop` that
    /// routed correctly and one that silently no-op'd would look identical
    /// here. (This is a renamed, re-scoped version of what used to be
    /// called `stop_ends_a_running_sessions_agent`, whose name overclaimed
    /// exactly this.) Proving a real kill happened is
    /// `tests/e2e/agent_listing_real_stack.rs`'s job, against a real
    /// supervisor and a real fake-agent process.
    #[tokio::test]
    async fn stop_routes_to_the_target_and_returns_the_empty_stopped_reply() {
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
        join_responder(responder, &outcome).await;

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

    /// Spec: `Stop`, like `Rename`, can target ANY session the helm knows,
    /// not only the asking one.
    ///
    /// `Rename`'s own cross-session test
    /// (`rename_can_target_any_named_session_and_returns_its_updated_row`)
    /// covers this for `Rename`; before this test, `Stop`'s only coverage
    /// used `session_id: None`, which cannot distinguish "targeted the
    /// named session" from "always acts on the asker and ignored the
    /// field" — a bug that field-substitution mistake would have shipped
    /// invisibly.
    #[tokio::test]
    async fn stop_can_target_a_named_session_other_than_the_asker() {
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
            let ControlMsg::StopSession { req_id, session_id } =
                parse_control(&frame).expect("decode request")
            else {
                panic!("expected StopSession");
            };
            assert_eq!(session_id, "other", "the NAMED target, not the asker");
            writer
                .write_control(&ControlMsg::SessionStopped { req_id })
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
                AgentVerb::Stop {
                    session_id: Some("other".to_string()),
                },
            )
            .await;
        join_responder(responder, &outcome).await;

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

    /// Spec: a self-targeting `Archive` reaches the owning supervisor as an
    /// `ArchiveSession` naming the asker, and its `SessionArchived` reply is
    /// projected back through the same [`AgentReply::Session`] shape
    /// `Rename` uses, `archived` flag and all.
    ///
    /// What this does NOT prove, despite the flag assertion below, is that
    /// anything was actually archived: the flag in the reply is the one the
    /// SCRIPTED supervisor was told to send back, so a helm that fabricated
    /// the row without asking anyone would fail this test only because of
    /// the far-end assertion on the forwarded frame, not because of the
    /// flag. The earlier name (`archive_flips_the_archived_flag`) claimed
    /// the stronger property and invited exactly that misreading. Real
    /// archiving is `do_archive_session`'s own contract, exercised against
    /// a real supervisor in the e2e suite.
    #[tokio::test]
    async fn archive_routes_to_the_owner_and_projects_the_reply() {
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
        join_responder(responder, &outcome).await;

        match outcome {
            AgentOutcome::Ok {
                reply: AgentReply::Session { session },
            } => {
                assert!(
                    session.archived,
                    "the owner's archived flag must survive the projection"
                );
            }
            other => panic!("expected a session reply, got {other:?}"),
        }
    }

    /// Spec: `Archive`, like `Rename` and `Stop`, can target ANY session
    /// the helm knows, not only the asking one — see
    /// `stop_can_target_a_named_session_other_than_the_asker`'s docs for
    /// why `session_id: None` alone cannot prove this.
    #[tokio::test]
    async fn archive_can_target_a_named_session_other_than_the_asker() {
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
            assert_eq!(session_id, "other", "the NAMED target, not the asker");
            writer
                .write_control(&ControlMsg::SessionArchived {
                    req_id,
                    session: SessionInfo {
                        archived: true,
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
                AgentVerb::Archive {
                    session_id: Some("other".to_string()),
                },
            )
            .await;
        join_responder(responder, &outcome).await;

        match outcome {
            AgentOutcome::Ok {
                reply: AgentReply::Session { session },
            } => {
                assert_eq!(session.id, "other");
                assert!(session.archived, "the archived flag must flip in the reply");
                assert!(
                    !session.current,
                    "the row acted on is not the asking session's own"
                );
            }
            other => panic!("expected a session reply, got {other:?}"),
        }
    }

    /// Spec: a lifecycle verb can target a session on a DIFFERENT host than
    /// the asker's own — the fleet-wide authority every prior cross-session
    /// test in this module (`rename_can_target_any_named_session_...`,
    /// `stop_can_target_a_named_session_...`,
    /// `archive_can_target_a_named_session_...`) exercised only within ONE
    /// host's cache.
    ///
    /// This matters as its own case because routing a MUTATION across
    /// hosts touches machinery the same-host tests never reach:
    /// `route_session` resolving an owner from helm.db rather than from
    /// the asking host's own in-memory list, and the mutation traveling
    /// out over a DIFFERENT supervisor connection than the one the request
    /// arrived on. Only the REMOTE host needs a scripted peer — the
    /// mutation is the only frame that ever reaches it; the local
    /// (asking) host is served standalone, exactly as every read-only
    /// fleet test above already does.
    #[tokio::test]
    async fn archive_can_target_a_session_on_a_different_host_than_the_asker() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        // Spawned BEFORE the fleet is built — see `spliced_local_fleet`'s
        // docs for why the order is load-bearing rather than cosmetic; the
        // same ordering constraint applies here even though the spliced
        // host is `ssh` rather than `local`.
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
            assert_eq!(session_id, "remote-target");
            writer
                .write_control(&ControlMsg::SessionArchived {
                    req_id,
                    session: SessionInfo {
                        archived: true,
                        ..session("remote-target", 2)
                    },
                })
                .await
                .expect("write reply");
        });

        let (builder, remote) = FleetBuilder::new()
            .await
            .local(HostScript {
                identity: Some("identity-local".to_string()),
                sessions: vec![session("asker", 1)],
                ..HostScript::default()
            })
            .await
            .ssh(
                "user@builder",
                HostScript {
                    identity: Some("identity-builder".to_string()),
                    sessions: vec![session("remote-target", 2)],
                    peer: Some(client_side),
                    ..HostScript::default()
                },
            )
            .await;
        let h = builder.start().await;
        let local = local_id(&h.store).await;
        // BOTH refreshes, and the remote one is the load-bearing half. The
        // owner cache this dispatch routes by is written when a host's
        // first refresh lands, and the target lives on the REMOTE host —
        // so waiting only for the local host's refresh (which is all this
        // test used to do) left the dispatch racing the very lookup it
        // depends on. Losing that race did not fail the test; it HUNG it,
        // because `route_session` then answered `NotFound` without
        // forwarding anything and the scripted responder below waited
        // forever for a frame that was never going to be sent. Rare when
        // the test ran alone and reproducible under a loaded suite: two of
        // twenty-four concurrent runs wedged before this line existed.
        h.await_refreshed(local).await;
        h.await_refreshed(remote).await;

        let handler = HelmAgentRequests::for_state(&h.state);
        let outcome = handler
            .handle(
                origin_of(&h, local),
                "asker",
                AgentVerb::Archive {
                    session_id: Some("remote-target".to_string()),
                },
            )
            .await;
        join_responder(responder, &outcome).await;

        match outcome {
            AgentOutcome::Ok {
                reply: AgentReply::Session { session },
            } => {
                assert_eq!(session.id, "remote-target");
                assert_eq!(
                    session.host.as_deref(),
                    Some("user@builder"),
                    "the reply names the TARGET's host, not the asker's"
                );
                assert!(session.archived);
                assert!(
                    !session.current,
                    "a session on another host is never the asker's own row"
                );
            }
            other => panic!("expected a session reply, got {other:?}"),
        }
    }

    /// Spec: a lifecycle verb naming a session whose owning host is CACHED
    /// but not currently connected is refused `Conflict`, naming that
    /// host's state — the same refusal `route_session` produces for the
    /// REST routes, now proven over the agent path.
    ///
    /// Distinct from [`a_lifecycle_verb_on_an_unknown_session_is_not_found`]
    /// below: that case is a session nothing has ever heard of (`NotFound`,
    /// caught before any routing decision), while this one is a session
    /// the helm knows perfectly well but currently has no live connection
    /// to act through (`Conflict`, caught BY routing) — two different
    /// refusals a caller needs to tell apart, since only one of them
    /// clears on its own once the host reconnects.
    #[tokio::test]
    async fn a_lifecycle_verb_on_a_disconnected_hosts_session_is_a_conflict() {
        let (h, local, _remote) = two_host_fleet().await;
        let handler = HelmAgentRequests::for_state(&h.state);

        // `remote-a` is cached from before `two_host_fleet` took the
        // remote host down, so this is a session the helm knows about —
        // unlike the unknown-session case below — with no live connection
        // to send the mutation through.
        let outcome = handler
            .handle(
                origin_of(&h, local),
                "local-live",
                AgentVerb::Stop {
                    session_id: Some("remote-a".to_string()),
                },
            )
            .await;

        match outcome {
            AgentOutcome::Err { kind, message } => {
                assert_eq!(kind, ErrorKind::Conflict);
                // NOT a check for the specific phase word ("unreachable-
                // reprobing"): `take_down` marks the host permanently
                // unreachable, but the manager still cycles it through a
                // brief `connecting` re-dial attempt on every retry
                // interval before it settles back to unreachable, and
                // asserting the exact phase would make this test flaky
                // against whichever one it happened to land on. What
                // `refusal_text` actually promises — and what this checks
                // — is the shared phrase EVERY non-connected state
                // produces, which is the very contract that lets a caller
                // treat them uniformly (see `sessions::route_session`'s
                // module docs).
                assert!(
                    message.contains("nothing was queued"),
                    "every non-connected state must refuse with the shared refusal shape: \
                     {message}"
                );
            }
            other => panic!("expected a Conflict refusal, got {other:?}"),
        }
    }

    /// Spec: a lifecycle verb naming a session the helm has never heard of
    /// is refused `NotFound`, before any supervisor is ever asked.
    ///
    /// A STANDALONE fleet (no scripted peer) is deliberate: if this ever
    /// regressed into forwarding the request anyway, there would be no
    /// script to answer it. Without the explicit timeout below, that
    /// regression would hang this ONE test (and, on a suite run without
    /// per-test isolation, potentially the whole binary) rather than
    /// failing cleanly with a diagnosis pointing at what actually broke.
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
        let outcome = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            handler.handle(
                origin_of(&harness, local),
                "asker",
                AgentVerb::Stop {
                    session_id: Some("ghost".to_string()),
                },
            ),
        )
        .await
        .expect(
            "an unknown-session refusal must be immediate, with no supervisor ever asked; a hang \
             here means the request was forwarded anyway",
        );

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

    // ---------------------------------------------------------------
    // The CREATING verbs.
    //
    // These need a scripted supervisor that answers TWO kinds of
    // request rather than the single one-shot exchange the lifecycle
    // tests script: a profile-catalog read (which is how a name is
    // resolved on the target host) and then the create itself. The
    // shared responder below is what makes that readable; every test
    // after it is one fleet plus one verb plus assertions on what the
    // helm actually put on the wire.
    // ---------------------------------------------------------------

    /// One `CreateSession` the helm sent, reduced to the fields these
    /// tests judge.
    ///
    /// Recorded rather than merely replied to, because almost every
    /// interesting property of the creating verbs is a property of the
    /// REQUEST: which profile id a name resolved to on which host, whether
    /// a clone copied the source's directory, whether the idempotency key
    /// survived the two hops. A test that only inspected the reply would
    /// pass against a helm that resolved the wrong profile and echoed the
    /// right row back.
    #[derive(Debug, Clone)]
    struct SeenCreate {
        cwd: String,
        invocation: Option<String>,
        profile_id: Option<String>,
        /// The SELECTOR a name-backed create carries, which is the whole
        /// point of these tests since the helm stopped resolving names
        /// itself: a request arriving with `profile_id` where the agent
        /// named a profile is the regression, and it is invisible from the
        /// reply.
        profile_name: Option<String>,
        title: Option<String>,
        intent_key: Option<String>,
    }

    /// A profile with the two fields a name resolution cares about.
    fn profile(id: &str, name: &str) -> farhelm_proto::Profile {
        farhelm_proto::Profile {
            id: id.to_string(),
            name: name.to_string(),
            invocation: format!("run-{name}"),
            agent_kind: farhelm_proto::AgentKind::Generic,
            resume_template: None,
        }
    }

    /// Script a supervisor that serves `catalog` to every `ListProfiles`
    /// and answers every `CreateSession`, recording what it was asked for.
    ///
    /// Spawned on the PEER half of a duplex whose other half is handed to
    /// [`FleetBuilder`] — and spawned BEFORE the fleet is built, for the
    /// ordering reason [`spliced_local_fleet`] documents.
    ///
    /// Loops rather than answering once, unlike the lifecycle tests'
    /// one-shot responders, because a single create is TWO exchanges when a
    /// profile name is involved (catalog, then create) and because the
    /// remembered-default test deliberately makes two creates in a row.
    /// Nothing joins this task: a refusal test's helm never sends the
    /// create at all, so a joinable responder would hang exactly the tests
    /// that are asserting an early failure. The runtime ends it with the
    /// test.
    ///
    /// The reply ECHOES the request's cwd, title and invocation back as the
    /// created `SessionInfo`, which is what a real supervisor does and what
    /// lets a clone test assert the copied values through the reply as well
    /// as through the recorded request.
    fn spawn_create_responder(
        peer: tokio::io::DuplexStream,
        catalog: Vec<farhelm_proto::Profile>,
        refusal: Option<String>,
    ) -> std::sync::Arc<std::sync::Mutex<Vec<SeenCreate>>> {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let recorded = std::sync::Arc::clone(&seen);
        tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .expect("handshake");
            let mut created = 0usize;
            while let Ok(Some(frame)) = reader.read_frame().await {
                let reply = match parse_control(&frame).expect("decode request") {
                    ControlMsg::ListProfiles { req_id } => ControlMsg::ProfileList {
                        req_id,
                        profiles: catalog.clone(),
                    },
                    ControlMsg::CreateSession {
                        req_id,
                        cwd,
                        invocation,
                        profile_id,
                        profile_name,
                        title,
                        intent_key,
                        ..
                    } => {
                        recorded.lock().expect("seen mutex").push(SeenCreate {
                            cwd: cwd.clone(),
                            invocation: invocation.clone(),
                            profile_id: profile_id.clone(),
                            profile_name: profile_name.clone(),
                            title: title.clone(),
                            intent_key,
                        });
                        // A NAME is resolved here, against this responder's
                        // own catalog, exactly as the real supervisor
                        // resolves one inside creation. The fixture has to
                        // do it for the tests to mean anything: since the
                        // helm forwards the name, "the target refuses an
                        // unknown name" is now a property of the target,
                        // and a responder that accepted any name would let
                        // a helm sending garbage pass.
                        let resolved = profile_name.as_ref().map(|name| {
                            catalog
                                .iter()
                                .filter(|p| &p.name == name)
                                .collect::<Vec<_>>()
                        });
                        let name_refusal = match (&profile_name, &resolved) {
                            (Some(name), Some(matches)) if matches.is_empty() => Some(format!(
                                "no profile named {name:?} exists on this host; available \
                                 profiles: {}",
                                catalog
                                    .iter()
                                    .map(|p| p.name.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )),
                            (Some(name), Some(matches)) if matches.len() > 1 => {
                                Some(format!("profile name {name:?} is ambiguous"))
                            }
                            _ => None,
                        };
                        match refusal.clone().or(name_refusal) {
                            Some(message) => ControlMsg::Error {
                                req_id,
                                message,
                                kind: ErrorKind::InvalidRequest,
                            },
                            None => {
                                created += 1;
                                let source_profile = profile_id
                                    .as_ref()
                                    .and_then(|id| catalog.iter().find(|p| &p.id == id))
                                    .or_else(|| resolved.as_ref().and_then(|m| m.first().copied()))
                                    .map(|p| SourceProfile {
                                        id: p.id.clone(),
                                        name: p.name.clone(),
                                        existence: ProfileExistence::Present,
                                    });
                                ControlMsg::SessionCreated {
                                    req_id,
                                    session: SessionInfo {
                                        cwd,
                                        title: title.unwrap_or_default(),
                                        invocation: invocation.unwrap_or_default(),
                                        source_profile,
                                        ..session(&format!("created-{created}"), 100)
                                    },
                                }
                            }
                        }
                    }
                    // Everything else is the manager's own housekeeping,
                    // which the splice already answers for us.
                    _ => continue,
                };
                if writer.write_control(&reply).await.is_err() {
                    return;
                }
            }
        });
        seen
    }

    /// A two-host fleet whose SSH host is scripted by
    /// [`spawn_create_responder`] — the shape every cross-host creating
    /// test needs.
    ///
    /// The local host is standalone: it only has to exist, be connected,
    /// and list the asking session, which is exactly what a standalone
    /// script does.
    async fn creating_fleet(
        client_side: tokio::io::DuplexStream,
        local_sessions: Vec<SessionInfo>,
    ) -> (Harness, HostId, HostId) {
        let (builder, remote) = FleetBuilder::new()
            .await
            .local(HostScript {
                identity: Some("identity-local".to_string()),
                sessions: local_sessions,
                ..HostScript::default()
            })
            .await
            .ssh(
                "user@builder",
                HostScript {
                    identity: Some("identity-builder".to_string()),
                    sessions: Vec::new(),
                    peer: Some(client_side),
                    ..HostScript::default()
                },
            )
            .await;
        let h = builder.start().await;
        let local = local_id(&h.store).await;
        h.await_refreshed(local).await;
        h.await_refreshed(remote).await;
        (h, local, remote)
    }

    /// Spec: `create --host <name> --profile <name>` resolves the profile
    /// against the TARGET host's own catalog and creates there, answering
    /// with `Created` (not `Session`) and naming the target host.
    ///
    /// This is the verb SPEC.md's "The verbs also CREATE" paragraph
    /// describes, minus the agent, and every clause below is a distinct way
    /// to get it wrong. Naming
    /// the host by DISPLAY NAME is the only handle an agent has, since it
    /// has never been shown a registry id. Resolving the profile on the
    /// TARGET is what keeps a create off the wrong machine's catalog: ids
    /// collide across installs by construction (every fresh supervisor
    /// seeds the same starter ids), so a helm that resolved the name
    /// locally and shipped the id would silently launch a profile nobody
    /// chose. And `Created` rather than `Session` is what tells the CLI this
    /// row is its creating verb's RESULT rather than a row it changed — the
    /// two payloads are identical, so the tag is the whole signal. (It is
    /// not a claim that the row is new: a keyed replay answers with a
    /// session that already existed, under the same tag.)
    ///
    /// The wire assertion is the load-bearing one and it is deliberately
    /// stated as "a NAME, and no id": the helm must not turn the name into
    /// an id at all, because the resolution and the create would then be
    /// two operations with a rename-sized window between them, and a keyed
    /// retry would fingerprint whatever id the catalog held at each attempt
    /// rather than the name the caller sent both times.
    #[tokio::test]
    async fn create_resolves_a_profile_name_on_the_named_target_host() {
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let seen = spawn_create_responder(
            peer,
            // The name the test asks for exists here under an id that
            // means nothing anywhere else — which is the point.
            vec![
                profile("remote-p7", "Claude"),
                profile("remote-p8", "Codex"),
            ],
            None,
        );
        let (h, local, _remote) = creating_fleet(client_side, vec![session("asker", 1)]).await;

        let handler = HelmAgentRequests::for_state(&h.state);
        let outcome = handler
            .handle(
                origin_of(&h, local),
                "asker",
                AgentVerb::Create {
                    host: Some("user@builder".to_string()),
                    cwd: "/srv/work".to_string(),
                    profile_name: Some("Claude".to_string()),
                    invocation: None,
                    title: Some("over there".to_string()),
                    intent_key: Some("key-1".to_string()),
                },
            )
            .await;

        match outcome {
            AgentOutcome::Ok {
                reply: AgentReply::Created { session },
            } => {
                assert_eq!(session.id, "created-1");
                assert_eq!(
                    session.host.as_deref(),
                    Some("user@builder"),
                    "the reply names the host the session was created on"
                );
                assert_eq!(session.title, "over there");
                assert_eq!(session.cwd, "/srv/work");
                assert_eq!(
                    session.agent, "Claude",
                    "the created row reports the profile it came from"
                );
                assert!(
                    !session.current,
                    "a freshly created session is never the asking one"
                );
            }
            other => panic!("expected a Created reply, got {other:?}"),
        }

        let seen = seen.lock().expect("seen mutex").clone();
        assert_eq!(seen.len(), 1, "exactly one create reached the target");
        assert_eq!(
            seen[0].profile_name.as_deref(),
            Some("Claude"),
            "the NAME travels to the target, which resolves it against its own catalog"
        );
        assert_eq!(
            seen[0].profile_id, None,
            "the helm must not pre-resolve the name into an id of its own choosing"
        );
        assert_eq!(seen[0].invocation, None, "profile mode sends no invocation");
        assert_eq!(seen[0].cwd, "/srv/work");
        assert_eq!(seen[0].title.as_deref(), Some("over there"));
        assert_eq!(
            seen[0].intent_key.as_deref(),
            Some("key-1"),
            "the idempotency key survives both hops"
        );
    }

    /// Spec: a profile name absent from the TARGET host's catalog is
    /// refused, naming both the host and the name, with no session created.
    ///
    /// SPEC.md's agent section states the rule this pins: "No match is a
    /// refusal naming the host and the profile", with no fallback to the
    /// source's raw invocation. A helm that fell back to any other profile
    /// — or to some invocation — would launch an agent the user did not
    /// choose on a machine they did not inspect, and the reply would look
    /// like a success.
    ///
    /// The refusal must name BOTH halves because either alone leaves the
    /// agent unable to act: the name alone does not say which catalog was
    /// searched, and the host alone does not say what was looked for. The
    /// name half is the TARGET's own sentence and the host half is added by
    /// `on_host` on the way back, which is the division of labour this test
    /// is really checking — the target knows its catalog, and only the helm
    /// knows which of several hosts the agent meant.
    #[tokio::test]
    async fn create_refuses_a_profile_name_the_target_host_does_not_have() {
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let seen = spawn_create_responder(peer, vec![profile("remote-p8", "Codex")], None);
        let (h, local, _remote) = creating_fleet(client_side, vec![session("asker", 1)]).await;

        let handler = HelmAgentRequests::for_state(&h.state);
        let outcome = handler
            .handle(
                origin_of(&h, local),
                "asker",
                AgentVerb::Create {
                    host: Some("user@builder".to_string()),
                    cwd: "/srv/work".to_string(),
                    profile_name: Some("Claude".to_string()),
                    invocation: None,
                    title: None,
                    intent_key: None,
                },
            )
            .await;

        match outcome {
            AgentOutcome::Err { kind, message } => {
                assert_eq!(kind, ErrorKind::InvalidRequest);
                assert!(
                    message.contains("user@builder") && message.contains("Claude"),
                    "the refusal must name the host AND the profile: {message}"
                );
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
        let seen = seen.lock().expect("seen mutex").clone();
        assert_eq!(
            seen.len(),
            1,
            "the create is what carries the name, so exactly one reaches the target"
        );
        assert_eq!(
            seen[0].profile_name.as_deref(),
            Some("Claude"),
            "the unresolvable name is what was sent; the refusal is the target's answer to it"
        );
    }

    /// Spec: `create --host <name>` naming an unregistered host is
    /// `NotFound`, quoting the name and listing what does exist.
    ///
    /// The known-hosts list is part of the contract rather than decoration:
    /// the agent's next move after this refusal is to pick a real host, and
    /// a bare "no such host" would send it back for a second round trip to
    /// `farhelm agent hosts` — which is the listing this refusal is
    /// summarizing.
    #[tokio::test]
    async fn create_on_an_unknown_host_name_is_not_found() {
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let _seen = spawn_create_responder(peer, Vec::new(), None);
        let (h, local, _remote) = creating_fleet(client_side, vec![session("asker", 1)]).await;

        let handler = HelmAgentRequests::for_state(&h.state);
        let outcome = handler
            .handle(
                origin_of(&h, local),
                "asker",
                AgentVerb::Create {
                    host: Some("nowhere".to_string()),
                    cwd: "/srv/work".to_string(),
                    profile_name: None,
                    invocation: Some("sh".to_string()),
                    title: None,
                    intent_key: None,
                },
            )
            .await;

        match outcome {
            AgentOutcome::Err { kind, message } => {
                assert_eq!(kind, ErrorKind::NotFound);
                assert!(
                    message.contains("nowhere"),
                    "the refusal quotes the name that failed: {message}"
                );
                assert!(
                    message.contains("user@builder") && message.contains("this machine"),
                    "the refusal lists the names that would have worked: {message}"
                );
            }
            other => panic!("expected a NotFound refusal, got {other:?}"),
        }
    }

    /// Spec: `create` naming neither a profile nor an invocation falls back
    /// to the target host's REMEMBERED default — the profile a successful
    /// create on that host last used — and naming both is refused.
    ///
    /// Driven as two creates in a row, deliberately, rather than by seeding
    /// helm.db directly: the first create is what WRITES the remembered
    /// default (`sessions::remember_default_profile`, reached through the
    /// shared create path), and the second is what reads it. A test that
    /// seeded the row would pass even if the agent create path had stopped
    /// recording defaults at all — which would make an agent's creates
    /// silently diverge from the create dialog's suggestion.
    ///
    /// The refusal half is here rather than in its own test because it is
    /// the same decision point: a body naming both selectors has no honest
    /// reading (does the profile's invocation win, or the caller's?), so it
    /// is refused rather than arbitrated, exactly as the REST create
    /// refuses the same shape.
    #[tokio::test]
    async fn create_with_no_selector_uses_the_targets_remembered_default() {
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let seen = spawn_create_responder(peer, vec![profile("remote-p7", "Claude")], None);
        let (h, local, _remote) = creating_fleet(client_side, vec![session("asker", 1)]).await;
        let handler = HelmAgentRequests::for_state(&h.state);
        let origin = origin_of(&h, local);

        let both = handler
            .handle(
                origin,
                "asker",
                AgentVerb::Create {
                    host: Some("user@builder".to_string()),
                    cwd: "/srv/work".to_string(),
                    profile_name: Some("Claude".to_string()),
                    invocation: Some("sh".to_string()),
                    title: None,
                    intent_key: None,
                },
            )
            .await;
        match both {
            AgentOutcome::Err { kind, message } => {
                assert_eq!(kind, ErrorKind::InvalidRequest);
                assert!(message.contains("never both"), "{message}");
            }
            other => panic!("naming both selectors must be refused, got {other:?}"),
        }

        // A create that establishes the default...
        let first = handler
            .handle(
                origin,
                "asker",
                AgentVerb::Create {
                    host: Some("user@builder".to_string()),
                    cwd: "/srv/work".to_string(),
                    profile_name: Some("Claude".to_string()),
                    invocation: None,
                    title: None,
                    intent_key: None,
                },
            )
            .await;
        assert!(
            matches!(
                first,
                AgentOutcome::Ok {
                    reply: AgentReply::Created { .. }
                }
            ),
            "the seeding create must succeed: {first:?}"
        );

        // ...and one that must find it, naming nothing at all.
        let second = handler
            .handle(
                origin,
                "asker",
                AgentVerb::Create {
                    host: Some("user@builder".to_string()),
                    cwd: "/srv/other".to_string(),
                    profile_name: None,
                    invocation: None,
                    title: None,
                    intent_key: None,
                },
            )
            .await;
        assert!(
            matches!(
                second,
                AgentOutcome::Ok {
                    reply: AgentReply::Created { .. }
                }
            ),
            "a selectorless create must fall back to the remembered default: {second:?}"
        );

        let seen = seen.lock().expect("seen mutex").clone();
        assert_eq!(
            seen.len(),
            2,
            "the refused both-selectors create sent nothing"
        );
        assert_eq!(
            seen[0].profile_name.as_deref(),
            Some("Claude"),
            "the seeding create names the profile; the target resolves it"
        );
        // The remembered default is an ID, and that asymmetry with the
        // named create above is the point: a default is something this helm
        // chose from its own memory of what worked on this host, so it has
        // an id to send and no name to defer. Only a name the CALLER typed
        // is left for the target to resolve.
        assert_eq!(seen[1].profile_id.as_deref(), Some("remote-p7"));
        assert_eq!(
            seen[1].profile_name, None,
            "a remembered default is already an id and needs no resolution"
        );
        assert_eq!(seen[1].cwd, "/srv/other");
    }

    /// Spec: `clone` onto the asking session's OWN host follows the
    /// source's profile ID, and copies its cwd and title.
    ///
    /// By id rather than by name because the target IS the source's
    /// supervisor: the id is exact, survives a rename, and needs no catalog
    /// read at all. That last part is what this test really pins — a helm
    /// that resolved by name even here would break a clone of a session
    /// whose profile has since been RENAMED, since the snapshotted name no
    /// longer matches anything in the catalog.
    #[tokio::test]
    async fn clone_on_the_same_host_follows_the_sources_profile_id() {
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        // The catalog is deliberately EMPTY: a name resolution would find
        // nothing and refuse, so a passing test proves the id path was
        // taken.
        let seen = spawn_create_responder(peer, Vec::new(), None);

        let source = SessionInfo {
            cwd: "/srv/project".to_string(),
            title: "the original".to_string(),
            invocation: "should-not-be-used".to_string(),
            source_profile: Some(SourceProfile {
                id: "local-p3".to_string(),
                name: "Claude".to_string(),
                existence: ProfileExistence::Present,
            }),
            ..session("asker", 1)
        };
        // The SPLICED host is the local one here: the clone's source drain
        // and its create both land on the asking session's own host.
        let harness = crate::rest_harness::spliced_helm_listing(client_side, vec![source]).await;
        let local = local_id(&harness.store).await;

        let handler = HelmAgentRequests::for_state(&harness.state);
        let outcome = handler
            .handle(
                origin_of(&harness, local),
                "asker",
                AgentVerb::Clone {
                    host: None,
                    cwd: None,
                    title: None,
                    intent_key: None,
                },
            )
            .await;

        match outcome {
            AgentOutcome::Ok {
                reply: AgentReply::Created { session },
            } => {
                assert_eq!(session.cwd, "/srv/project");
                assert_eq!(session.title, "the original");
            }
            other => panic!("expected a Created reply, got {other:?}"),
        }

        let seen = seen.lock().expect("seen mutex").clone();
        assert_eq!(seen.len(), 1);
        assert_eq!(
            seen[0].profile_id.as_deref(),
            Some("local-p3"),
            "a same-host clone follows the id, with no catalog read"
        );
        assert_eq!(
            seen[0].cwd, "/srv/project",
            "the source's directory is copied"
        );
        assert_eq!(
            seen[0].title.as_deref(),
            Some("the original"),
            "the source's title is copied verbatim, not re-derived from the directory"
        );
    }

    /// Spec: `clone --host <other>` resolves the source's profile by NAME
    /// against the other host's catalog, and `--cwd`/`--title` override
    /// what would otherwise be copied.
    ///
    /// This is SPEC.md's cross-host clone expressed as a unit: it is
    /// exactly the operation a supervisor-local implementation could not
    /// perform, and name resolution is what makes it land on the right
    /// agent. The source's profile id (`local-p3`) is deliberately ALSO
    /// present in the target's catalog under a DIFFERENT name, which is
    /// the collision that makes id-carrying dangerous in production and
    /// would make a lazy implementation pass every other assertion here
    /// while launching the wrong agent.
    #[tokio::test]
    async fn clone_to_another_host_resolves_the_profile_by_name() {
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let seen = spawn_create_responder(
            peer,
            vec![
                // Same ID as the source's profile, different name: an
                // implementation that carried the id across hosts would
                // launch THIS.
                profile("local-p3", "Something Else"),
                profile("remote-p9", "Claude"),
            ],
            None,
        );

        let source = SessionInfo {
            cwd: "/srv/project".to_string(),
            title: "the original".to_string(),
            source_profile: Some(SourceProfile {
                id: "local-p3".to_string(),
                name: "Claude".to_string(),
                existence: ProfileExistence::Present,
            }),
            ..session("asker", 1)
        };
        let (h, local, _remote) = creating_fleet(client_side, vec![source]).await;

        let handler = HelmAgentRequests::for_state(&h.state);
        let outcome = handler
            .handle(
                origin_of(&h, local),
                "asker",
                AgentVerb::Clone {
                    host: Some("user@builder".to_string()),
                    cwd: Some("/srv/elsewhere".to_string()),
                    title: Some("the copy".to_string()),
                    intent_key: None,
                },
            )
            .await;

        match outcome {
            AgentOutcome::Ok {
                reply: AgentReply::Created { session },
            } => {
                assert_eq!(session.host.as_deref(), Some("user@builder"));
                assert_eq!(session.cwd, "/srv/elsewhere");
                assert_eq!(session.title, "the copy");
            }
            other => panic!("expected a Created reply, got {other:?}"),
        }

        let seen = seen.lock().expect("seen mutex").clone();
        assert_eq!(seen.len(), 1);
        assert_eq!(
            seen[0].profile_name.as_deref(),
            Some("Claude"),
            "the SNAPSHOTTED NAME is what crosses the wire, for the target to resolve"
        );
        assert_eq!(
            seen[0].profile_id, None,
            "the source's id must never be carried onto another host's catalog"
        );
        assert_eq!(
            seen[0].cwd, "/srv/elsewhere",
            "--cwd overrides the source's"
        );
        assert_eq!(
            seen[0].title.as_deref(),
            Some("the copy"),
            "--title overrides the source's"
        );
    }

    /// Spec: a cross-host clone whose profile NAME is absent from the
    /// target's catalog is refused, naming the host and the name — never
    /// falling back to the source's raw invocation.
    ///
    /// The fallback is the specific mistake SPEC.md's agent section forbids
    /// ("There is deliberately no fallback to the source's raw
    /// invocation"), and it is tempting precisely because the source row is
    /// carrying a perfectly good invocation. That invocation was written
    /// for the SOURCE machine: on another host it may name a binary that is
    /// not installed, a different build, or one that takes different flags
    /// — and the failure would arrive as a broken session rather than as a
    /// refusal anyone could act on.
    ///
    /// What "no session may be created" means here is now a claim about the
    /// TARGET rather than about the helm: the name is forwarded and the
    /// target refuses it, so the assertion below is that exactly one
    /// request went out carrying the NAME and none carrying the source's
    /// invocation.
    #[tokio::test]
    async fn clone_to_another_host_refuses_rather_than_falling_back_to_the_invocation() {
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let seen = spawn_create_responder(peer, vec![profile("remote-p8", "Codex")], None);

        let source = SessionInfo {
            cwd: "/srv/project".to_string(),
            invocation: "/opt/local-only/claude --flag".to_string(),
            source_profile: Some(SourceProfile {
                id: "local-p3".to_string(),
                name: "Claude".to_string(),
                existence: ProfileExistence::Present,
            }),
            ..session("asker", 1)
        };
        let (h, local, _remote) = creating_fleet(client_side, vec![source]).await;

        let handler = HelmAgentRequests::for_state(&h.state);
        let outcome = handler
            .handle(
                origin_of(&h, local),
                "asker",
                AgentVerb::Clone {
                    host: Some("user@builder".to_string()),
                    cwd: None,
                    title: None,
                    intent_key: None,
                },
            )
            .await;

        match outcome {
            AgentOutcome::Err { kind, message } => {
                assert_eq!(kind, ErrorKind::InvalidRequest);
                assert!(
                    message.contains("user@builder") && message.contains("Claude"),
                    "the refusal names the host and the profile: {message}"
                );
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
        let seen = seen.lock().expect("seen mutex").clone();
        assert_eq!(seen.len(), 1, "exactly one request reached the target");
        assert_eq!(
            seen[0].profile_name.as_deref(),
            Some("Claude"),
            "the snapshotted name is what was sent"
        );
        assert_eq!(
            seen[0].invocation, None,
            "no session may be created from the source's own invocation as a fallback"
        );
    }

    /// Spec: cloning a source that came from NO profile sends its raw
    /// invocation to the target.
    ///
    /// There is no name to resolve and nothing to guess here — the user
    /// created that session from a command line, so a copy of it is that
    /// command line. This is the one case where the invocation legitimately
    /// crosses hosts, which is exactly why the refusal above must not.
    #[tokio::test]
    async fn clone_of_a_raw_session_sends_its_invocation() {
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let seen = spawn_create_responder(peer, Vec::new(), None);

        let source = SessionInfo {
            cwd: "/srv/project".to_string(),
            invocation: "sh -c 'echo hi'".to_string(),
            source_profile: None,
            ..session("asker", 1)
        };
        let (h, local, _remote) = creating_fleet(client_side, vec![source]).await;

        let handler = HelmAgentRequests::for_state(&h.state);
        let outcome = handler
            .handle(
                origin_of(&h, local),
                "asker",
                AgentVerb::Clone {
                    host: Some("user@builder".to_string()),
                    cwd: None,
                    title: None,
                    intent_key: Some("clone-key".to_string()),
                },
            )
            .await;
        assert!(
            matches!(
                outcome,
                AgentOutcome::Ok {
                    reply: AgentReply::Created { .. }
                }
            ),
            "a raw-invocation clone must succeed: {outcome:?}"
        );

        let seen = seen.lock().expect("seen mutex").clone();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].invocation.as_deref(), Some("sh -c 'echo hi'"));
        assert_eq!(seen[0].profile_id, None);
        assert_eq!(seen[0].intent_key.as_deref(), Some("clone-key"));
    }

    /// Spec: a create the TARGET supervisor refuses — a directory that does
    /// not exist there is the case this stands for — reaches the agent as
    /// that supervisor's own refusal text, verbatim.
    ///
    /// SPEC.md's agent section requires it in those words ("a directory
    /// that does not exist on the target is that supervisor's own refusal,
    /// reported verbatim rather than paraphrased on the way back"), and it
    /// is worth pinning because this side has every
    /// opportunity to paraphrase: the helm knows the host name, the
    /// directory and the verb, and a friendlier sentence assembled here
    /// would replace the only description of what actually went wrong on a
    /// machine nobody is looking at. A sentinel string is what proves the
    /// exact bytes crossed both hops, the same technique
    /// [`rename_refusal_reaches_the_agent_verbatim`] uses.
    #[tokio::test]
    async fn a_create_refusal_from_the_target_reaches_the_agent_verbatim() {
        const SENTINEL: &str = "SENTINEL-agent-create: no such directory: /srv/absent";

        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let seen = spawn_create_responder(
            peer,
            vec![profile("remote-p7", "Claude")],
            Some(SENTINEL.to_string()),
        );
        let (h, local, _remote) = creating_fleet(client_side, vec![session("asker", 1)]).await;

        let handler = HelmAgentRequests::for_state(&h.state);
        let outcome = handler
            .handle(
                origin_of(&h, local),
                "asker",
                AgentVerb::Create {
                    host: Some("user@builder".to_string()),
                    cwd: "/srv/absent".to_string(),
                    profile_name: Some("Claude".to_string()),
                    invocation: None,
                    title: None,
                    intent_key: None,
                },
            )
            .await;

        match outcome {
            AgentOutcome::Err { kind, message } => {
                assert_eq!(kind, ErrorKind::InvalidRequest);
                // The COMPLETE message, not a `contains`: a substring
                // check passes against a helm that wrapped the refusal in
                // a paragraph of its own invention, which is exactly the
                // failure this test exists to catch. The one addition
                // this side is allowed is the host prefix `on_host` adds,
                // because the target's own sentence says "this host" and
                // an agent that named one of several needs to know which.
                assert_eq!(
                    message,
                    format!("on host \"user@builder\": {SENTINEL}"),
                    "the target's own refusal arrives whole, under the host name it came from"
                );
            }
            other => panic!("expected the target's refusal, got {other:?}"),
        }
        assert_eq!(
            seen.lock().expect("seen mutex").len(),
            1,
            "the create really was attempted on the target before it refused"
        );
    }

    /// Answer every `CreateSession` with a `SessionCreated` carrying exactly
    /// `id`, however malformed.
    ///
    /// [`spawn_create_responder`] cannot stand in: it always mints a
    /// well-formed id of its own, and the malformed id IS the fixture here.
    /// Loops rather than answering once so a single responder serves a whole
    /// table of shapes; nothing joins it, exactly as that function documents.
    fn spawn_created_id_responder(peer: tokio::io::DuplexStream, id: String) {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .expect("handshake");
            while let Ok(Some(frame)) = reader.read_frame().await {
                let ControlMsg::CreateSession { req_id, .. } =
                    parse_control(&frame).expect("decode request")
                else {
                    continue;
                };
                let reply = ControlMsg::SessionCreated {
                    req_id,
                    session: SessionInfo {
                        id: id.clone(),
                        ..session("placeholder", 100)
                    },
                };
                if writer.write_control(&reply).await.is_err() {
                    return;
                }
            }
        });
    }

    /// Spec: a target that answers a create with a `SessionCreated` whose id
    /// this helm must refuse leaves the agent with `Timeout` and the
    /// check-before-retrying remedy — never a bare `Internal` — for every
    /// shape the ingress rules reject.
    ///
    /// The refusal itself belongs to `client::created_session`; what this
    /// test is about is what the refusal COSTS the caller. The target
    /// accepted the create and almost certainly started the session: it
    /// answered with the very variant that says so. The one thing that could
    /// address that session afterwards is the id the helm just threw away,
    /// so an agent told "internal error" retries an unkeyed create and ends
    /// up with a second real session while the first runs on under an id
    /// nobody will ever be told. The end-to-end `AgentOutcome` is the
    /// assertion (not the helper's error) because the classification travels
    /// through three layers — the typed transport error, `transport_outcome`,
    /// and the verb's own mutating flag — and only the last of them is what
    /// an agent reads.
    #[tokio::test]
    async fn an_unusable_created_session_id_is_outcome_unknown_for_every_shape() {
        let oversized = "x".repeat(crate::manager::MAX_SESSION_ID_BYTES + 1);
        for (shape, id) in [
            ("an empty id", String::new()),
            ("an id past the ingress cap", oversized),
            (
                "an id carrying a control character",
                "sess\nforged".to_string(),
            ),
        ] {
            let (client_side, peer) = tokio::io::duplex(64 * 1024);
            spawn_created_id_responder(peer, id.clone());
            let (h, local, _remote) = creating_fleet(client_side, vec![session("asker", 1)]).await;

            let outcome = HelmAgentRequests::for_state(&h.state)
                .handle(
                    origin_of(&h, local),
                    "asker",
                    AgentVerb::Create {
                        host: Some("user@builder".to_string()),
                        cwd: "/srv/work".to_string(),
                        profile_name: None,
                        invocation: Some("agent".to_string()),
                        title: None,
                        intent_key: None,
                    },
                )
                .await;

            let AgentOutcome::Err { kind, message } = outcome else {
                panic!("{shape} must not be reported as a created session");
            };
            assert_eq!(
                kind,
                ErrorKind::Timeout,
                "{shape}: the create was sent and answered, so its outcome is unknown rather \
                 than an internal fault: {message}"
            );
            assert!(
                message.contains(farhelm_proto::AGENT_MUTATION_UNKNOWN_REMEDY),
                "{shape}: and the agent must be told to look before retrying: {message}"
            );
            // The refused id is the peer's own bytes: quoting it would put an
            // unbounded — and, for the control-character shape, terminal-
            // forging — value into the frame the agent prints.
            if !id.is_empty() {
                assert!(
                    !message.contains(&id),
                    "{shape}: the refused id must not be echoed back: {message}"
                );
            }
        }
    }

    /// Spec: the known-host list an unknown-name refusal carries stays
    /// inside its byte budget, caps each name on its own, and says how many
    /// it left out.
    ///
    /// The budget is not cosmetic. This string is built from the WHOLE
    /// registry and ends up inside an `AgentOutcome` that has to fit in one
    /// 8 MiB frame; a reply that does not fit is discarded and reaches the
    /// agent as a generic `Internal`, so an unbounded diagnostic destroys
    /// exactly the refusal it was trying to make useful. The per-name cap is
    /// the second half of the same argument: without it one pathological
    /// name spends the whole allowance and every other host disappears —
    /// which is worse than a truncated list, because the omission would be
    /// invisible. Hence the count.
    #[test]
    fn the_known_host_list_is_bounded_and_says_what_it_left_out() {
        assert_eq!(known_hosts(&[]), "none");
        assert_eq!(
            known_hosts(&["this machine", "user@builder"]),
            "this machine, user@builder",
            "an ordinary fleet is listed whole"
        );

        let long = "n".repeat(500);
        let capped = known_hosts(&[&long]);
        assert!(
            capped.chars().count() < 200 && capped.ends_with('…'),
            "one enormous name is cut on its own, visibly: {capped:?}"
        );

        // Enough names that the total cannot fit, each individually short.
        let many: Vec<String> = (0..500)
            .map(|n| format!("host-{n:04}-{}", "x".repeat(40)))
            .collect();
        let many: Vec<&str> = many.iter().map(String::as_str).collect();
        let listed = known_hosts(&many);
        assert!(
            listed.len() < 8 * 1024,
            "the list stays far inside a frame: {} bytes",
            listed.len()
        );
        assert!(
            listed.contains("more"),
            "a cut list says how many hosts it did not name: {listed}"
        );
    }

    /// Spec: a fleet holding a host whose display name carries a control
    /// character says so in the unknown-name refusal.
    ///
    /// Such a host is visible in the listing (escaped) and permanently
    /// unreachable, because the relay refuses a `--host` value containing a
    /// control character. Without this sentence an agent has no way to tell
    /// "I typed the name wrong" from "that host cannot be named at all",
    /// and the fix — a rename — is not a verb it has.
    #[test]
    fn the_unknown_host_refusal_names_hosts_that_can_never_be_targeted() {
        assert_eq!(unnameable_hosts(&["this machine", "user@builder"]), None);
        let warned = unnameable_hosts(&["ok", "bad\nname"]).expect("one host is unnameable");
        assert!(
            warned.contains('1') && warned.contains("control characters"),
            "the sentence counts them and says why: {warned}"
        );
    }

    /// Spec: a clone whose target replays the ASKING session — because the
    /// idempotency key was the one that created it — is refused as a
    /// `Conflict`, not reported as a new session.
    ///
    /// Reachable by ordinary means, which is why it is guarded at all. A
    /// same-host clone with no overrides rebuilds precisely the fingerprint
    /// that created the asking session: same directory, same title, same
    /// profile id, no parent. An agent that reuses the key its own create
    /// used therefore triggers a legitimate reservation replay at the
    /// target, which answers with the ORIGINAL session. Reporting that as
    /// `Created` would hand the caller its own id as a new one — with
    /// `current: true`, no less — and the caller's next move is to act on
    /// what it believes is a copy.
    ///
    /// The second half of the test is about WHEN the refusal happens, and it
    /// is the half with durable consequences. Refusing after
    /// `do_create_session`'s bookkeeping still refuses, but by then the
    /// replayed row has been seeded into the cache and its profile written
    /// as the host's remembered default — effects of a create the agent is
    /// simultaneously being told did not occur. The seeded default is
    /// deliberately PROVENANCE-LESS (an upgraded database's row, or an
    /// administrative write) and names a different profile, because that is
    /// the shape the store's ordering guard cannot reject: a candidate
    /// carrying a session provenance beats a stored row carrying none, so
    /// the replay would move the default and bump the fleet revision, waking
    /// every client to a change nobody made. The replayed payload also
    /// carries a title the cache has never seen, which is what makes "the
    /// row was not seeded" observable at all.
    #[tokio::test]
    async fn a_clone_that_replays_the_asking_session_is_refused() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let source = SessionInfo {
            cwd: "/srv/project".to_string(),
            title: "the original".to_string(),
            source_profile: Some(SourceProfile {
                id: "local-p3".to_string(),
                name: "Claude".to_string(),
                existence: ProfileExistence::Present,
            }),
            ..session("asker", 1)
        };
        // A target that REPLAYS: it answers the create with the very
        // session the clone was made from, exactly as a reservation lookup
        // under an already-spent key would. Written inline rather than
        // through `spawn_create_responder` because the replayed id is the
        // whole fixture, and that responder always mints a fresh one.
        //
        // The title differs from the cached row's on purpose — a replay
        // reports the session as the TARGET knows it now, which need not
        // match the last snapshot this helm drained. It is the tell for
        // whether the row was seeded.
        let replayed = SessionInfo {
            title: "renamed since the helm last looked".to_string(),
            ..source.clone()
        };
        let responder = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .expect("handshake");
            while let Ok(Some(frame)) = reader.read_frame().await {
                if let ControlMsg::CreateSession { req_id, .. } =
                    parse_control(&frame).expect("decode request")
                {
                    writer
                        .write_control(&ControlMsg::SessionCreated {
                            req_id,
                            session: replayed.clone(),
                        })
                        .await
                        .expect("write reply");
                    return;
                }
            }
        });
        let h = crate::rest_harness::spliced_helm_listing(client_side, vec![source]).await;
        let local = local_id(&h.store).await;

        // Written through the store's administrative entry point, which is
        // the one that records no source session — see the docstring.
        h.store
            .remember_profile_default(local, Some("local-identity"), "local-p9")
            .await
            .expect("seeding a remembered default");
        let handler = HelmAgentRequests::for_state(&h.state);
        let listing_before = agent_sessions(&handler, origin_of(&h, local), "asker").await;
        let revision_before = h.manager.events().revision();

        let outcome = handler
            .handle(
                origin_of(&h, local),
                "asker",
                AgentVerb::Clone {
                    host: None,
                    cwd: None,
                    title: None,
                    intent_key: Some("the-key-that-made-me".to_string()),
                },
            )
            .await;
        responder.abort();

        match outcome {
            AgentOutcome::Err { kind, message } => {
                assert_eq!(kind, ErrorKind::Conflict);
                assert!(
                    message.contains("no copy was made"),
                    "the refusal must say plainly that nothing new exists: {message}"
                );
            }
            other => panic!("a replayed self must not be reported as a clone, got {other:?}"),
        }

        assert_eq!(
            h.store
                .remembered_profile(local)
                .await
                .expect("reading the remembered default"),
            Some("local-p9".to_string()),
            "a clone that made nothing must not rewrite the host's remembered default"
        );
        assert_eq!(
            h.manager.events().revision(),
            revision_before,
            "and must not wake every client with a fleet revision for it"
        );
        assert_eq!(
            agent_sessions(&handler, origin_of(&h, local), "asker").await,
            listing_before,
            "nor seed the replayed row into the cache — the title it carried must not appear"
        );
    }

    /// The fleet listing as the agent surface itself reports it — the
    /// cache's contents, read the way a test can compare two moments of it.
    async fn agent_sessions(
        handler: &std::sync::Arc<dyn AgentRequestHandler>,
        origin: AgentOrigin,
        asking: &str,
    ) -> Vec<AgentSession> {
        match handler.handle(origin, asking, AgentVerb::Sessions {}).await {
            AgentOutcome::Ok {
                reply: AgentReply::Sessions { sessions, .. },
            } => sessions,
            other => panic!("the listing verb must answer with a listing, got {other:?}"),
        }
    }

    /// Spec: a same-host clone of a session whose profile was RENAMED still
    /// follows the id.
    ///
    /// The sibling
    /// [`clone_on_the_same_host_follows_the_sources_profile_id`] proves the
    /// id path with a `Present` profile, which is the case where an
    /// accidental name resolution would also work if the name happened to
    /// still match. `Renamed` is the state that tells the two apart: the
    /// snapshotted name is stale by definition, so a helm that resolved by
    /// name here would send a name the catalog no longer has and break a
    /// clone that must simply work. The catalog below deliberately holds
    /// the profile under its NEW name only.
    #[tokio::test]
    async fn clone_on_the_same_host_follows_a_renamed_profiles_id() {
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let seen = spawn_create_responder(peer, vec![profile("local-p3", "The New Name")], None);

        let source = SessionInfo {
            cwd: "/srv/project".to_string(),
            source_profile: Some(SourceProfile {
                id: "local-p3".to_string(),
                name: "The Old Name".to_string(),
                existence: ProfileExistence::Renamed,
            }),
            ..session("asker", 1)
        };
        // The SPLICED host is the local one, so the source drain and the
        // create both land on the asking session's own host — which is what
        // "same host" means here.
        let h = crate::rest_harness::spliced_helm_listing(client_side, vec![source]).await;
        let local = local_id(&h.store).await;

        let outcome = HelmAgentRequests::for_state(&h.state)
            .handle(
                origin_of(&h, local),
                "asker",
                AgentVerb::Clone {
                    host: None,
                    cwd: None,
                    title: None,
                    intent_key: None,
                },
            )
            .await;
        assert!(
            matches!(
                outcome,
                AgentOutcome::Ok {
                    reply: AgentReply::Created { .. }
                }
            ),
            "a renamed profile still clones by id: {outcome:?}"
        );

        let seen = seen.lock().expect("seen mutex").clone();
        assert_eq!(
            seen.last().expect("a create reached the target").profile_id,
            Some("local-p3".to_string()),
            "the snapshotted ID is followed; the stale NAME is never sent"
        );
    }

    /// Spec: a same-host clone whose source profile has been DELETED still
    /// sends that id, and takes the target's own missing-profile refusal
    /// rather than resolving the stale name onto a replacement.
    ///
    /// This is the sharpest case in the resolution order and the reason it
    /// says "always by id". A user who deletes a profile and creates a new
    /// one under the same name has expressed no opinion about sessions that
    /// came from the old one; a clone that silently followed the NAME would
    /// launch the replacement's settings under the appearance of copying
    /// the original. The catalog below is exactly that trap — same name,
    /// different id — and the assertion is that the trap is not entered.
    #[tokio::test]
    async fn clone_on_the_same_host_of_a_deleted_profile_refuses_rather_than_reusing_the_name() {
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let seen = spawn_create_responder(
            peer,
            // A replacement profile carrying the deleted one's NAME under a
            // new id: the substitution this rule exists to prevent.
            vec![profile("local-p9", "Claude")],
            None,
        );

        let source = SessionInfo {
            cwd: "/srv/project".to_string(),
            source_profile: Some(SourceProfile {
                id: "local-p3".to_string(),
                name: "Claude".to_string(),
                existence: ProfileExistence::Deleted,
            }),
            ..session("asker", 1)
        };
        let h = crate::rest_harness::spliced_helm_listing(client_side, vec![source]).await;
        let local = local_id(&h.store).await;

        let outcome = HelmAgentRequests::for_state(&h.state)
            .handle(
                origin_of(&h, local),
                "asker",
                AgentVerb::Clone {
                    host: None,
                    cwd: None,
                    title: None,
                    intent_key: None,
                },
            )
            .await;
        // The responder answers a by-id create it cannot match with a
        // session carrying no source profile rather than a refusal, so what
        // this test pins is the REQUEST: the deleted id was sent, and the
        // replacement's name never was. The target's own "no such profile"
        // refusal is its business, and `create_refuses_a_profile_name_the_
        // target_host_does_not_have` covers that half.
        let seen = seen.lock().expect("seen mutex").clone();
        assert_eq!(seen.len(), 1, "one create was attempted: {outcome:?}");
        assert_eq!(
            seen[0].profile_id,
            Some("local-p3".to_string()),
            "the DELETED id is what is sent"
        );
        assert_eq!(
            seen[0].profile_name, None,
            "the stale name must not be offered to the catalog as a fallback"
        );
    }

    /// Spec: `create --invocation` succeeds through the shared agent path,
    /// sending the raw command line and no profile of any kind.
    ///
    /// Every other create test here is profile-backed, so the raw selector
    /// reached the target only in refusal cases. A regression that dropped
    /// raw routing — or seeded the cache wrongly for it, or projected the
    /// reply from the wrong side — would leave all of them green.
    #[tokio::test]
    async fn create_from_a_raw_invocation_reaches_the_target_whole() {
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let seen = spawn_create_responder(peer, Vec::new(), None);
        let (h, local, _remote) = creating_fleet(client_side, vec![session("asker", 1)]).await;

        let outcome = HelmAgentRequests::for_state(&h.state)
            .handle(
                origin_of(&h, local),
                "asker",
                AgentVerb::Create {
                    host: Some("user@builder".to_string()),
                    cwd: "/srv/raw".to_string(),
                    profile_name: None,
                    invocation: Some("sh -c 'sleep 1'".to_string()),
                    title: Some("raw one".to_string()),
                    intent_key: Some("raw-key".to_string()),
                },
            )
            .await;

        match outcome {
            AgentOutcome::Ok {
                reply: AgentReply::Created { session },
            } => {
                assert_eq!(session.host.as_deref(), Some("user@builder"));
                assert_eq!(session.cwd, "/srv/raw");
                assert_eq!(session.title, "raw one");
                assert!(!session.current);
            }
            other => panic!("expected a Created reply, got {other:?}"),
        }

        let seen = seen.lock().expect("seen mutex").clone();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].invocation.as_deref(), Some("sh -c 'sleep 1'"));
        assert_eq!(seen[0].profile_id, None);
        assert_eq!(seen[0].profile_name, None);
        assert_eq!(seen[0].intent_key.as_deref(), Some("raw-key"));
    }

    /// Spec: a selectorless create against a host with NO remembered
    /// default is refused, and nothing is sent to that host.
    ///
    /// The sibling test seeds the default with a create of its own and then
    /// reads it back, which says nothing about the FIRST create an agent
    /// ever makes on a host. That is the state a real fleet spends its
    /// early life in, and the two ways to get it wrong are both silent: a
    /// fallback to some other profile launches an agent nobody chose, and a
    /// create sent with no selector at all is refused by the target with a
    /// sentence about the wire rather than about the missing default.
    #[tokio::test]
    async fn create_with_no_selector_and_no_remembered_default_is_refused() {
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let seen = spawn_create_responder(peer, vec![profile("remote-p7", "Claude")], None);
        let (h, local, _remote) = creating_fleet(client_side, vec![session("asker", 1)]).await;

        let outcome = HelmAgentRequests::for_state(&h.state)
            .handle(
                origin_of(&h, local),
                "asker",
                AgentVerb::Create {
                    host: Some("user@builder".to_string()),
                    cwd: "/srv/work".to_string(),
                    profile_name: None,
                    invocation: None,
                    title: None,
                    intent_key: None,
                },
            )
            .await;

        match outcome {
            AgentOutcome::Err { kind, message } => {
                assert_eq!(kind, ErrorKind::InvalidRequest);
                assert!(
                    message.contains("no default") || message.contains("default to fall back"),
                    "the refusal must say the DEFAULT is what is missing: {message}"
                );
                assert!(
                    message.contains("user@builder"),
                    "and which host has none: {message}"
                );
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
        assert!(
            seen.lock().expect("seen mutex").is_empty(),
            "a host with no default is refused before anything is sent to it"
        );
    }

    /// Spec: a target catalog holding two profiles under the requested name
    /// refuses the create, and the refusal names the host.
    ///
    /// The ambiguity branch belongs to the TARGET now that names are
    /// forwarded rather than resolved here, which makes this test's real
    /// subject the seam: the helm must pass the name through and must add
    /// the host to whatever comes back, because the target's own sentence
    /// can only say "this host".
    #[tokio::test]
    async fn create_with_an_ambiguous_profile_name_on_the_target_is_refused() {
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let seen = spawn_create_responder(
            peer,
            vec![
                profile("remote-p1", "Claude"),
                profile("remote-p2", "Claude"),
            ],
            None,
        );
        let (h, local, _remote) = creating_fleet(client_side, vec![session("asker", 1)]).await;

        let outcome = HelmAgentRequests::for_state(&h.state)
            .handle(
                origin_of(&h, local),
                "asker",
                AgentVerb::Create {
                    host: Some("user@builder".to_string()),
                    cwd: "/srv/work".to_string(),
                    profile_name: Some("Claude".to_string()),
                    invocation: None,
                    title: None,
                    intent_key: None,
                },
            )
            .await;

        match outcome {
            AgentOutcome::Err { kind, message } => {
                assert_eq!(kind, ErrorKind::InvalidRequest);
                assert!(
                    message.contains("ambiguous") && message.contains("user@builder"),
                    "the refusal names the ambiguity and the host: {message}"
                );
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
        assert_eq!(
            seen.lock().expect("seen mutex").len(),
            1,
            "the name reached the target, which is what refused it"
        );
    }

    /// Spec: two hosts registered under one display name make `--host`
    /// ambiguous, and the create is refused rather than routed to whichever
    /// row came first.
    ///
    /// Reachable without malice: the local row is called `this machine`,
    /// and nothing stops an ssh destination from being spelled the same. A
    /// `.find` — which this used to be — would send the session to a
    /// machine the agent never chose, and the reply would look like an
    /// ordinary success naming the name the agent asked for.
    #[tokio::test]
    async fn create_on_an_ambiguous_host_name_is_refused() {
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let seen = spawn_create_responder(peer, vec![profile("remote-p7", "Claude")], None);
        // The colliding row is registered BEFORE the fleet starts, because
        // an ssh row added to a running fleet gets an actor that dials, and
        // the scripted transport has no script for a row it never heard of.
        // Its destination — which IS its display name — is spelled exactly
        // as the local row renders; the registry has no uniqueness rule
        // over display names, which is precisely why the resolver needs
        // one. Scripted UNREACHABLE, since the point is that the name is
        // ambiguous, not that the impostor answers.
        let (builder, _impostor) = FleetBuilder::new()
            .await
            .local(HostScript {
                identity: Some("identity-local".to_string()),
                sessions: vec![session("asker", 1)],
                ..HostScript::default()
            })
            .await
            .ssh(
                "this machine",
                HostScript {
                    reachable: false,
                    ..HostScript::default()
                },
            )
            .await;
        let (builder, _remote) = builder
            .ssh(
                "user@builder",
                HostScript {
                    identity: Some("identity-builder".to_string()),
                    peer: Some(client_side),
                    ..HostScript::default()
                },
            )
            .await;
        let h = builder.start().await;
        let local = local_id(&h.store).await;
        h.await_refreshed(local).await;

        let outcome = HelmAgentRequests::for_state(&h.state)
            .handle(
                origin_of(&h, local),
                "asker",
                AgentVerb::Create {
                    host: Some("this machine".to_string()),
                    cwd: "/srv/work".to_string(),
                    profile_name: Some("Claude".to_string()),
                    invocation: None,
                    title: None,
                    intent_key: None,
                },
            )
            .await;

        match outcome {
            AgentOutcome::Err { kind, message } => {
                assert_eq!(kind, ErrorKind::Conflict);
                assert!(
                    message.contains("ambiguous") && message.contains("this machine"),
                    "the refusal names the collision: {message}"
                );
            }
            other => panic!("an ambiguous host name must be refused, got {other:?}"),
        }
        assert!(
            seen.lock().expect("seen mutex").is_empty(),
            "nothing is created while the target is ambiguous"
        );
    }
}
