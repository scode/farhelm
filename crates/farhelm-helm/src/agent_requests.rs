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
//! uses — `hosts::host_views` for hosts, `aggregate::session_list` for
//! sessions, `sessions::do_rename_session`/`do_stop_session`/
//! `do_restart_session` for the three lifecycle verbs,
//! `sessions::do_create_session` for `create` and `clone`, and
//! `templates::store_template`/`remove_template` for the template writes.
//! Not for economy:
//! the point of routing an agent's questions (and its actions) through the
//! helm at all is that the agent and the user see, and act on, one fleet.
//! Two listings assembled two ways would drift, and the drift would show
//! up as an agent confidently naming a host that is not in the panel the
//! user is reading; two rename implementations would drift on exactly the
//! validation rule that matters (SPEC.md's control-character refusal); two
//! create implementations would drift on the cache seed that makes a new
//! session appear in the UI without a refresh. Recording the launch as the
//! user's explicit choice is deliberately not shared: only a user-originated
//! REST create decides what the user's next create dialog suggests.
//!
//! # The creating verbs are what could not have been built anywhere else
//!
//! `create` and `clone` name their target host by DISPLAY NAME — the value
//! `hosts` reports — and that is the capability the whole relay exists for.
//! A supervisor-local implementation has no fleet, no host names, and no
//! route to another machine, so "clone this session onto that host" is
//! precisely the thing it could not do. Everything the verbs need lives on
//! this side: the host list and the create path.
//!
//! # Lifecycle verbs act on ANY session, not only the asker's own
//!
//! `Rename`, `Stop`, and `Restart` each carry `session_id: Option<String>` so
//! an old wire shape can still be decoded and refused. The relay and this
//! authoritative boundary both require `Some(id)`, including for a deliberate
//! self-action. The id may name any session the helm knows, on any host.
//! [`resolve_target`] logs both asking and target identities so an operator
//! can distinguish a self-action from one reaching across the fleet.
//!
//! `Clone` names its source explicitly. The helm resolves the live owner,
//! reads the source from that pinned connection, then rechecks the owner
//! before dispatching to the destination. The asking identity remains
//! separate for fences, audit, and replay refusal.
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
//! # Acting verbs wait for the user first
//!
//! Every verb that changes something (rename, stop, restart, create, clone,
//! `farhelm spawn` through create, and the template writes) is carried out
//! only once the user has
//! approved it on a card in the GUI, or the requesting host's "run farhelm
//! commands from this host without asking" setting is on (SPEC.md,
//! Agent-spawned sessions). [`approved`] is that gate: it asks
//! (`approvals::ask`) after the verb has resolved exactly what it would do,
//! so the card shows that, and before any of it happens; it then rechecks
//! that the request's connection is still the one serving its host, so an
//! approval cannot carry over to whatever replaced it during the wait. New
//! sessions also pass the agent YOLO rule ([`approve_launch`],
//! `yolo_guard::check_agent`) before the card and again after it. A
//! lifecycle verb routes its target before asking, so a session no host
//! knows is refused without a card. Listings never ask.
//!
//! # What this side does NOT verify, and why that is deliberate
//!
//! The `session_id` on an upcall, and the claim that the connection it
//! arrived on is that session's host, are both taken on trust. The helm
//! never sees the per-session credential — only the supervisor can check
//! it, and it does, before forwarding — so there is nothing here to
//! re-verify against.
//!
//! That is sound only because of how far the claim reaches. The helm's
//! trust in a supervisor is scoped by EFFECT (SPEC_impl.md, "What the helm
//! believes from a supervisor"): it is believed about things that affect
//! only its own host, and "which of my sessions is asking" is one of them —
//! a supervisor lying about it can misattribute only its own sessions, over
//! which it already has full authority. It is NOT believed, by virtue of
//! the connection, about anything that reaches past its own host: another
//! host's sessions, another supervisor, the helm's machine, or helm-owned
//! state. Each verb below answers only what the spec grants any agent, and
//! the connection adds nothing to that grant; a supervisor cannot, for
//! example, change a host's settings through this relay. The template
//! writes do change helm-owned state, but on the user's authority, not the
//! connection's: each one is the write the user approved on its card (or
//! that the user's "without asking" setting for the requesting host lets
//! through), and it lands only on the template that card showed.

use std::sync::{Arc, Weak};

use anyhow::Context as _;
use async_trait::async_trait;
use farhelm_proto::approvals::{ApprovalAction, LaunchVerb};
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
    /// On mutating verbs, `session_id` remains the authenticated CALLER.
    /// Lifecycle targets and clone sources come from their explicit verb
    /// fields. Keeping those identities separate is required for mutation
    /// fences, audit, and refusing a clone replay that returns either the
    /// source row or the caller row as the alleged new child.
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
    /// [`AgentVerb::is_mutating`] answers `true` for — the lifecycle four
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

/// The most ENCODED session bytes one `sessions` reply will ever carry.
///
/// The listing itself is bounded by rows (`farhelm_proto::LIST_SESSIONS_CAP`,
/// applied by every supervisor and by the helm's merge), but rows say
/// nothing about size: session creation admits tens of kilobytes of
/// caller-supplied title, cwd and invocation text, so a fleet of legally
/// fat records could produce a reply no frame could carry. That answer
/// would be discarded at the writer's size backstop and the agent would
/// get `Internal` instead of the partial listing with `truncated: true`
/// that the verb promises, after the helm had already paid to build and
/// encode it (up to `client::AGENT_ANSWER_SLOTS` times over per host).
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
/// Reaching it means the same thing the listing cap means, and says so the
/// same way: `truncated: true`.
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
        if let Err(message) = validate_authoritative_verb(&verb) {
            return AgentOutcome::Err {
                kind: ErrorKind::InvalidRequest,
                message,
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
                        complete: true,
                        caller_host_id: origin.host.to_string(),
                    })
            }
            AgentVerb::Sessions {} => session_listing(&state, origin.host, session_id).await,
            AgentVerb::Rename {
                session_id: target,
                expected_title,
                title,
            } => {
                let target = resolve_target(target.expect("validated"), session_id, "rename");
                let expected_title = expected_title.expect("validated");
                if let Err(unroutable) = crate::sessions::route_session(&state, &target).await {
                    return outcome_of(Err(unroutable), mutating);
                }
                let action = ApprovalAction::Rename {
                    target: crate::approvals::describe_session(&state, &target).await,
                    title: title.clone(),
                };
                match approved(&state, origin, session_id, action).await {
                    Ok(()) => crate::sessions::do_rename_session(
                        &state,
                        &target,
                        &title,
                        Some(&expected_title),
                    )
                    .await
                    .map(|(claim, info)| {
                        agent_session_reply(&state, &claim, info, origin.host, session_id)
                    }),
                    Err(refused) => Err(refused),
                }
            }
            AgentVerb::Stop { session_id: target } => {
                let target = resolve_target(target.expect("validated"), session_id, "stop");
                if let Err(unroutable) = crate::sessions::route_session(&state, &target).await {
                    return outcome_of(Err(unroutable), mutating);
                }
                let action = ApprovalAction::Stop {
                    target: crate::approvals::describe_session(&state, &target).await,
                };
                match approved(&state, origin, session_id, action).await {
                    Ok(()) => crate::sessions::do_stop_session(&state, &target)
                        .await
                        .map(|()| AgentReply::Stopped {}),
                    Err(refused) => Err(refused),
                }
            }
            AgentVerb::Restart {
                session_id: target,
                stop_if_running,
            } => {
                let target = resolve_target(target.expect("validated"), session_id, "restart");
                if let Err(unroutable) = crate::sessions::route_session(&state, &target).await {
                    return outcome_of(Err(unroutable), mutating);
                }
                // A plain restart resumes the session's own stored launch, so
                // the YOLO rule does not apply (SPEC.md); the card shows that
                // launch, resume command included, as the cache knows it.
                // Without the launch there is nothing to show on the card and
                // nothing to hold the restart to, so the user would approve a
                // restart of whatever the session holds by then: refused, to
                // be retried once the helm has the session's details again.
                let Some(shown) = cached_launch(&state, &target).await else {
                    return outcome_of(
                        Err(anyhow::Error::new(crate::SupervisorError {
                            origin: crate::client::ErrorOrigin::Helm,
                            kind: ErrorKind::Unavailable,
                            message: "Farhelm cannot read this session's launch right now, so \
                                      it cannot show you what a restart would run; nothing was \
                                      done, retry shortly"
                                .to_string(),
                        })),
                        mutating,
                    );
                };
                let action = ApprovalAction::Restart {
                    target: crate::approvals::describe_session(&state, &target).await,
                    stop_if_running,
                    launch: Some(shown.clone()),
                };
                // The card showed `shown`; a Restart with in the GUI during the
                // wait would make the approved restart resume a different
                // launch. A change the cache already saw is refused here with
                // the clearer message; the supervisor then holds the restart
                // to `shown` under its lifecycle claim, which catches one
                // landing after this check (`expected_launch`).
                let approval = match approved(&state, origin, session_id, action).await {
                    Ok(()) if cached_launch(&state, &target).await.as_ref() != Some(&shown) => {
                        Err(anyhow::Error::new(crate::SupervisorError {
                            origin: crate::client::ErrorOrigin::Helm,
                            kind: ErrorKind::Conflict,
                            message: "the session's launch changed while this restart waited \
                                      for approval, so it was not restarted; retry to see the \
                                      new launch on the card"
                                .to_string(),
                        }))
                    }
                    other => other,
                };
                match approval {
                    Ok(()) => crate::sessions::do_restart_session(
                        &state,
                        &target,
                        stop_if_running,
                        None,
                        false,
                        Some(shown),
                    )
                    .await
                    .map(|(claim, info)| {
                        agent_restarted_reply(&state, &claim, info, origin.host, session_id)
                    }),
                    Err(refused) => Err(refused),
                }
            }
            AgentVerb::Templates {} => template_listing(&state, origin.host).await,
            AgentVerb::TemplateCreate { name, fields, host } => {
                write_template_for_agent(
                    &state,
                    origin,
                    session_id,
                    TemplateWrite {
                        name,
                        fields,
                        host,
                        create: true,
                    },
                )
                .await
            }
            AgentVerb::TemplateEdit { name, fields, host } => {
                write_template_for_agent(
                    &state,
                    origin,
                    session_id,
                    TemplateWrite {
                        name,
                        fields,
                        host,
                        create: false,
                    },
                )
                .await
            }
            AgentVerb::TemplateDelete { name } => {
                delete_template_for_agent(&state, origin, session_id, name).await
            }
            // `confirm_yolo` is ignored on both creating verbs: an agent has
            // no YOLO override (SPEC.md, Agent-spawned sessions), whatever a
            // modified CLI sends; `yolo_guard::check_agent` decides instead.
            AgentVerb::Create {
                host,
                templates,
                edits,
                intent_key,
                confirm_yolo: _,
                spawn,
            } => {
                create_for_agent(
                    &state,
                    origin,
                    session_id,
                    CreateRequest {
                        edits: LaunchEditsRequest {
                            host,
                            templates,
                            edits,
                            spawn,
                        },
                        intent_key,
                    },
                )
                .await
            }
            AgentVerb::Clone {
                source_session_id,
                host,
                cwd,
                title,
                intent_key,
                confirm_yolo: _,
            } => {
                clone_for_agent(
                    &state,
                    origin,
                    session_id,
                    CloneRequest {
                        source_session_id: source_session_id.expect("validated"),
                        host: host.expect("validated"),
                        cwd,
                        title,
                        intent_key,
                    },
                )
                .await
            }
        };
        outcome_of(reply, mutating)
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

/// Turn a verb's result into the answer the agent gets.
///
/// Classified the same way the REST surface classifies the SAME failures
/// (`crate::error_kind`), rather than flattened to `Internal`: a lifecycle or
/// creating verb's refusal (an unknown session, a rejected title, a
/// non-connected host, a directory the target does not have, a request the
/// user declined) is exactly the kind of thing a caller can act on
/// differently, and an agent deserves the same distinction a browser gets.
/// The read-only verbs rarely produce a classifiable error at all (a listing
/// failure has nothing upstream to classify against), so this falls back to
/// `Internal` for them.
///
/// A dead target-supervisor connection is consulted FIRST, because
/// `error_kind` has no answer for it: nothing in that chain is a
/// `SupervisorError` (the peer never replied), so it falls through to
/// `Internal`, the one kind that tells a caller nothing at all about
/// retrying.
fn outcome_of(reply: anyhow::Result<AgentReply>, mutating: bool) -> AgentOutcome {
    match reply {
        Ok(reply) => AgentOutcome::Ok { reply },
        Err(error) => transport_outcome(&error, mutating).unwrap_or(AgentOutcome::Err {
            kind: crate::error_kind(&error),
            message: format!("{error:#}"),
        }),
    }
}

/// Classify a failure whose cause is the TARGET supervisor never giving a
/// usable answer, or `None` if that is not what went wrong.
///
/// The helm sits in the middle of two hops, and this is the far one: the
/// asking session's supervisor forwarded the verb up to the helm, and the
/// helm routed it down to the supervisor that owns the target session. A
/// MUTATION — `Rename`/`Stop`/`Restart`, or a `Create`/`Clone` — that
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
pub(crate) fn origin_is_live(state: &AppState, origin: AgentOrigin) -> bool {
    state
        .manager
        .status(origin.host)
        .and_then(|status| status.client)
        .is_some_and(|client| client.connection_id() == origin.connection)
}

/// Recheck consequential selectors at the helm boundary so a handcrafted
/// or older relay request cannot regain an implicit default.
fn validate_authoritative_verb(verb: &AgentVerb) -> Result<(), String> {
    let required = |value: Option<&str>, flag: &str| match value {
        Some(value) if !value.is_empty() => Ok(()),
        _ => Err(format!("{flag} is required and must not be empty")),
    };
    match verb {
        AgentVerb::Rename {
            session_id,
            expected_title,
            ..
        } => {
            required(session_id.as_deref(), "--session")?;
            if expected_title.is_none() {
                return Err(
                    "--expected-title is required (an explicitly empty title is legal)".to_string(),
                );
            }
            Ok(())
        }
        AgentVerb::Stop { session_id } | AgentVerb::Restart { session_id, .. } => {
            required(session_id.as_deref(), "--session")
        }
        // Everything else a create needs may come from a template, so only
        // the helm's resolution can tell what is missing (`agent_launch`).
        AgentVerb::Create { host, .. } => {
            if host.as_deref() == Some("") {
                return Err("--host must not be empty".to_string());
            }
            Ok(())
        }
        AgentVerb::Clone {
            source_session_id,
            host,
            cwd,
            ..
        } => {
            required(source_session_id.as_deref(), "--source-session")?;
            required(host.as_deref(), "--host")?;
            // Optional, but not empty: an absent cwd falls back to the source
            // session's directory, while `Some("")` would replace it with an
            // empty path that only the target supervisor then refuses.
            if cwd.as_deref() == Some("") {
                return Err(
                    "--cwd must not be empty; omit it to use the source session's directory"
                        .to_string(),
                );
            }
            Ok(())
        }
        AgentVerb::TemplateCreate { name, host, .. }
        | AgentVerb::TemplateEdit { name, host, .. } => {
            required(Some(name.as_str()), "the template name")?;
            if host.as_deref() == Some("") {
                return Err("--host must not be empty".to_string());
            }
            Ok(())
        }
        AgentVerb::TemplateDelete { name } => required(Some(name.as_str()), "the template name"),
        AgentVerb::Hosts {} | AgentVerb::Sessions {} | AgentVerb::Templates {} => Ok(()),
    }
}

/// Record the explicit session one lifecycle verb acts on.
///
/// The caller has already rejected the old omitted-target wire shape, so
/// this function never substitutes `asking`. Keeping both values here is
/// still necessary: an explicit self-action and a cross-fleet action need
/// different identities in the audit record even though they share the same
/// routing path.
///
/// Also where "which session asked to act on which" is logged, at `info`
/// rather than left to be reconstructed from a `RenameSession` or
/// `StopSession` line on whatever supervisor eventually
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
fn resolve_target(target: String, asking: &str, verb: &str) -> String {
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
/// Escapes exactly [`farhelm_proto::text::is_presentation_unsafe`]: control
/// characters (a newline forges a whole extra log line) and the characters
/// that are not controls but still change what a reader SEES (bidi overrides
/// that reorder a line, zero-width characters that make two ids render
/// identically, line separators some viewers break on). The set is shared with
/// every other surface that shows peer-supplied text, so none of them can fall
/// behind the others.
pub(crate) fn escape_for_log(id: &str) -> String {
    // The common case is an id with nothing to escape, and the borrow-free
    // early return keeps this off the allocation path for it.
    if !id.chars().any(farhelm_proto::text::is_presentation_unsafe) {
        return id.to_string();
    }
    let mut out = String::with_capacity(id.len());
    for c in id.chars() {
        if farhelm_proto::text::is_presentation_unsafe(c) {
            out.push_str(&format!("\\u{{{:04x}}}", c as u32));
        } else {
            out.push(c);
        }
    }
    out
}

/// One `create` verb's fields, moved out of [`AgentVerb`] so the handler
/// arm stays a dispatch and the policy lives in [`create_for_agent`].
struct CreateRequest {
    edits: LaunchEditsRequest,
    intent_key: Option<String>,
}

/// What an agent's create asked for, as it asked: the templates and flags,
/// the `--host` name, and for `farhelm spawn` where it creates.
struct LaunchEditsRequest {
    host: Option<String>,
    templates: Vec<String>,
    edits: farhelm_proto::launcher::TemplateFields,
    spawn: Option<farhelm_proto::SpawnPlacement>,
}

impl LaunchEditsRequest {
    /// The request fingerprint a keyed create of this request sends, or
    /// `None` for an inheriting spawn, which keeps the resolved-launch
    /// fingerprint: its launch is the asking session's own, copied by its
    /// supervisor, with no template for an edit to change.
    fn request_fingerprint(&self) -> anyhow::Result<Option<String>> {
        if self
            .spawn
            .as_ref()
            .is_some_and(|placement| placement.inherit_agent)
        {
            return Ok(None);
        }
        AgentRequestDigest::Create {
            host: self.host.as_deref(),
            templates: &self.templates,
            edits: &self.edits,
            spawned: self.spawn.is_some(),
            parent: self
                .spawn
                .as_ref()
                .and_then(|placement| placement.parent.as_deref()),
        }
        .digest()
        .map(Some)
    }
}

/// The request an agent's keyed create or clone is matched by on its host,
/// as the helm digests it (`ControlMsg::CreateSession::request_fingerprint`;
/// SPEC.md, Agent-spawned sessions): what the agent sent, never what it
/// resolved to, so a retry repeating the request replays its first attempt
/// even after a template it names, or a clone's source, was edited.
///
/// ## The serialized shape is frozen
///
/// The digest lands in reservations that outlive builds, and a digest that
/// moved between builds would turn every outstanding key into a key-reuse
/// refusal after an upgrade. So this type, not the protocol request, is
/// what is hashed, each verb under its own versioned tag so a create and a
/// clone can never collide under one key; a different shape gets a new tag
/// rather than an edit to an existing one. The one protocol type inside it
/// is `TemplateFields`, whose JSON is already a stored format (the helm's
/// template catalog keeps it) and which omits every unset field, so a field
/// the launcher gains later leaves existing digests alone. `--confirm-yolo`
/// is in neither: a retry that adds the confirmation it was asked for is the
/// same request.
#[derive(serde::Serialize)]
#[serde(tag = "request")]
enum AgentRequestDigest<'a> {
    /// `farhelm agent create`, or `farhelm spawn` with launch flags.
    #[serde(rename = "agent_create_v1")]
    Create {
        host: Option<&'a str>,
        templates: &'a [String],
        edits: &'a farhelm_proto::launcher::TemplateFields,
        spawned: bool,
        parent: Option<&'a str>,
    },
    /// `farhelm agent clone`, with its overrides as given (absent stays
    /// absent, distinct from an override that repeats the source's value).
    #[serde(rename = "agent_clone_v1")]
    Clone {
        source: &'a str,
        host: &'a str,
        cwd: Option<&'a str>,
        title: Option<&'a str>,
    },
}

impl AgentRequestDigest<'_> {
    /// SHA-256 of the JSON encoding, as the 64 lowercase hex characters the
    /// supervisor checks for.
    fn digest(&self) -> anyhow::Result<String> {
        use sha2::Digest as _;
        let json = serde_json::to_vec(self).context("encoding the agent request")?;
        Ok(sha2::Sha256::digest(&json)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect())
    }
}

/// Where an agent's create resolved to: the host chosen (with the registry's
/// name for it, for the audit line and the approval card) and what the
/// templates and flags produced.
struct ResolvedCreate {
    host: HostId,
    host_name: String,
    resolution: crate::agent_launch::Resolution,
    /// The install identity a template named the host by, when the host
    /// came from a template and from nothing else (not `--host`, not a
    /// spawn's own host). Dispatch refuses unless the connection it sends on
    /// still reaches that installation: the row can be pointed at another
    /// machine, and the new installation adopted, between resolution and
    /// dispatch (the approval's own recheck covers only the wait for the
    /// user, not the stretches on either side of it), and SPEC.md has a
    /// template's host then stop applying "rather than silently aiming at
    /// the successor".
    template_identity: Option<String>,
}

/// One `clone` verb's fields, moved out of [`AgentVerb`] so the handler arm
/// stays a dispatch. A struct rather than positional parameters because
/// several are `Option<String>`: a call site that transposed `cwd` and
/// `title` would compile and be wrong.
struct CloneRequest {
    source_session_id: String,
    host: String,
    cwd: Option<String>,
    title: Option<String>,
    intent_key: Option<String>,
}

/// Resolve the exact host name a creating verb is required to carry.
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
/// is a session started on a machine nobody chose.
///
/// Returns the registry's display NAME alongside the id so every refusal
/// below this point names the host it was actually aimed at rather than
/// echoing untrusted request text.
async fn resolve_host(
    state: &AppState,
    _origin: AgentOrigin,
    name: String,
) -> anyhow::Result<(HostId, String)> {
    let views = crate::hosts::host_views(state).await?;
    {
        let mut matches = views.iter().filter(|view| view.name == name);
        let Some(view) = matches.next() else {
            let names: Vec<&str> = views.iter().map(|view| view.name.as_str()).collect();
            return Err(anyhow::Error::new(crate::SupervisorError {
                origin: crate::client::ErrorOrigin::Helm,
                kind: ErrorKind::NotFound,
                // The name is quoted back so a typo is visible as a
                // typo, and the alternatives are listed because the
                // agent's next move is to pick one.
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
                origin: crate::client::ErrorOrigin::Helm,
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
/// "working directory does not exist" leaves it unable to tell a typo in the
/// path from a typo in the host. This wraps rather than rewrites,
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

/// The intent key an agent's create or clone is stored under on the target:
/// the agent's own key, scoped to the session that asked.
///
/// Keys are chosen by agents, and nothing else in a request records who
/// asked, so an unscoped key let one session replay another's result: a
/// child re-running its parent's keyed `farhelm agent create` got the
/// parent's create back, which is the child itself, printed as "the new
/// session" (SPEC.md "Agent-spawned sessions": keys are scoped to the asking
/// session). Hashing the agent's key gives a fixed length, so the scoped key
/// stays inside the target's intent-key limit whatever the agent sent. Keys
/// reserved before this scoping no longer match, which only means such a
/// retry creates afresh.
fn asker_scoped_intent_key(asking_session: &str, key: Option<String>) -> Option<String> {
    use sha2::Digest as _;
    key.map(|key| {
        let digest: String = sha2::Sha256::digest(key.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        format!("agent-{asking_session}-{digest}")
    })
}

/// `create`: one session from the templates and flags an agent named
/// (`farhelm agent create`, or `farhelm spawn` with launch flags).
///
/// The templates and flags are resolved here (`agent_launch::resolve`), on
/// an empty launcher: an agent request never consults the create dialog's
/// remembered settings, so a field nothing set is a refusal naming its flag.
///
/// ## Which host
///
/// A spawn creates on the asking session's own host, the one its request
/// arrived from. Its parent, when it names one, must be the asking session;
/// the relay already holds the asking session's delete fence for the whole
/// request, so the parent cannot be deleted under the create. Its key is
/// marked to live only as long as the child, as every spawn's is. An
/// inheriting spawn takes its launch from its supervisor
/// ([`inherited_spawn_resolution`]) rather than from templates and flags. Otherwise an explicit `--host` name
/// wins; failing that, the install a template named; failing both, the
/// create is refused, since SPEC.md's creation contract has no default host.
///
/// ## What a keyed retry is matched by
///
/// The request as the agent sent it, not what it resolves to (SPEC.md,
/// Agent-spawned sessions). Every attempt resolves the templates and flags
/// afresh and sends the target the request's digest
/// ([`AgentRequestDigest`]) beside its key; the target compares the digest,
/// so a retry repeating the request replays the first attempt's outcome even
/// when a template it names was edited in between, and the same key with a
/// different request is refused as key reuse. A retry that no longer
/// resolves (a template deleted, the host unconnected) is refused here
/// before anything is sent, which never starts a second session. An
/// inheriting spawn is the exception: it keeps the resolved-launch
/// fingerprint ([`LaunchEditsRequest::request_fingerprint`]).
///
/// A keyed `create` must name its host with `--host`; see the refusal below
/// for why.
///
/// ## What this function does NOT decide
///
/// The working directory. A directory that does not exist on the target is
/// the TARGET supervisor's refusal, reported verbatim through
/// [`AgentOutcome::Err`] like every other create precondition — this side
/// never stats a path on another machine, and could not.
///
/// The INSTALLATION behind a host named by `--host`. The host is the
/// registry's durable [`HostId`], and `sessions::host_client` takes the
/// connection currently published for that row — exactly what the lifecycle
/// verbs do through `route_session`, and exactly what the REST create does
/// through `create_target`. A row retargeted or adopted between the two reads
/// sends the create to the new installation, and nothing here pins an
/// incarnation to prevent that. The claim is what makes it safe rather than
/// silent: every write below revalidates against the connection the create
/// was actually sent on, so the create either lands on one coherent
/// installation or fails.
///
/// A host that came from a TEMPLATE is different: the template names an
/// installation, not a row (SPEC.md, Launch templates), so its identity is
/// carried from resolution to dispatch (`ResolvedCreate::template_identity`)
/// and dispatch refuses when the connection's claim names another one
/// (`template_host_moved`). That is an installation check; the approval
/// path's own recheck (`approved`) is about the REQUESTING host's connection,
/// not the target's.
async fn create_for_agent(
    state: &AppState,
    origin: AgentOrigin,
    asking_session: &str,
    request: CreateRequest,
) -> anyhow::Result<AgentReply> {
    let CreateRequest { edits, intent_key } = request;
    let spawned = edits.spawn.is_some();
    let parent = edits.spawn.as_ref().and_then(|spawn| spawn.parent.clone());
    if let Some(parent) = &parent
        && parent != asking_session
    {
        return Err(anyhow::Error::new(crate::SupervisorError {
            origin: crate::client::ErrorOrigin::Helm,
            kind: ErrorKind::Unauthorized,
            message: format!(
                "a session-authenticated peer may name only itself ({}) as parent",
                escape_for_log(asking_session)
            ),
        }));
    }
    // A key's record lives only on the host the first attempt reached, so a
    // retry has to reach that same host to be matched against it. A host
    // taken from a template could change between the attempt and its retry
    // (the template edited to name another machine), and the retry would then
    // start a second session there, so a keyed create must name its host
    // explicitly; `--host` also makes the templates' host fields irrelevant.
    // A host NAME reassigned to another machine between the two attempts is
    // the remaining gap, accepted (SPEC.md, Agent-spawned sessions): the
    // retry is then an ordinary create on that machine, with every check a
    // fresh create gets. A spawn always creates on its own host and takes no
    // `--host`.
    if intent_key.is_some() && !spawned && edits.host.is_none() {
        return Err(crate::sessions::invalid_request(
            "an idempotency key needs --host: the key is kept on the host the first attempt \
             reached, so a retry must name that host rather than take it from a template"
                .to_string(),
        ));
    }
    let request_fingerprint = match &intent_key {
        Some(_) => edits.request_fingerprint()?,
        None => None,
    };
    let intent_key = asker_scoped_intent_key(asking_session, intent_key);
    let resolved = resolve_agent_create(state, origin, &edits).await?;
    let verb = match &edits.spawn {
        Some(placement) if placement.inherit_agent => LaunchVerb::SpawnInherited,
        Some(_) => LaunchVerb::Spawn,
        None => LaunchVerb::Create,
    };
    approve_launch(
        state,
        origin,
        asking_session,
        LaunchApproval {
            verb,
            host: resolved.host,
            host_name: resolved.host_name.clone(),
            cwd: resolved.resolution.cwd.clone(),
            title: resolved.resolution.title.clone(),
            launch: resolved.resolution.launch.clone(),
            source: None,
        },
    )
    .await?;
    dispatch_agent_create(
        state,
        origin,
        asking_session,
        resolved,
        AgentCreateDispatch {
            intent_key,
            parent,
            spawned,
            request_fingerprint,
        },
    )
    .await
}

/// Resolve an agent's create request to a host and a launch; see
/// [`create_for_agent`] for the host rule.
async fn resolve_agent_create(
    state: &AppState,
    origin: AgentOrigin,
    edits: &LaunchEditsRequest,
) -> anyhow::Result<ResolvedCreate> {
    let views = crate::hosts::host_views(state).await?;
    if let Some(placement) = &edits.spawn
        && placement.inherit_agent
    {
        return inherited_spawn_resolution(&views, origin, edits, placement);
    }
    let templates = state.store.launch_templates().await?;
    let resolution = crate::agent_launch::resolve(
        &templates,
        &edits.templates,
        &edits.edits,
        &crate::agent_launch::template_host_identities(&views),
        edits.spawn.is_some(),
        edits.host.is_some(),
    )?;
    let mut template_identity = None;
    let (host, host_name) = if edits.spawn.is_some() {
        if edits.host.is_some() {
            return Err(crate::sessions::invalid_request(
                "farhelm spawn always creates on its own host and takes no --host".to_string(),
            ));
        }
        let name = views
            .iter()
            .find(|view| view.id == origin.host)
            .map(|view| view.name.clone())
            .unwrap_or_default();
        (origin.host, name)
    } else if let Some(name) = edits.host.clone() {
        resolve_host(state, origin, name).await?
    } else if let Some(identity) = &resolution.template_host {
        // `agent_launch::resolve` accepted this identity against the same
        // rows, so a miss here is a defect rather than a stale template.
        let view = crate::agent_launch::template_host_row(&views, identity).ok_or_else(|| {
            anyhow::anyhow!("a template's host matched no host row it was checked against")
        })?;
        template_identity = Some(identity.clone());
        (view.id, view.name.clone())
    } else {
        return Err(crate::sessions::invalid_request(
            "--host is required unless a template sets the host".to_string(),
        ));
    };
    Ok(ResolvedCreate {
        host,
        host_name,
        resolution,
        template_identity,
    })
}

/// The resolution of `farhelm spawn --inherit-agent`: the asking session's
/// own stored launch, as its supervisor filled it in on the way up
/// (`SpawnPlacement::inherited_launch`), in the folder and under the title
/// the CLI gave, on the asking session's own host.
///
/// SPEC.md makes `--inherit-agent` exclusive with every launch flag; the CLI
/// refuses the combination, and so does this, since only a folder and a title
/// may ride beside it. The launch is believed as the supervisor sent it: a
/// spawn acts only on its own host, which the threat model already trusts
/// (SPEC.md accepts that a compromised supervisor could misreport it).
fn inherited_spawn_resolution(
    views: &[crate::hosts::HostView],
    origin: AgentOrigin,
    edits: &LaunchEditsRequest,
    placement: &farhelm_proto::SpawnPlacement,
) -> anyhow::Result<ResolvedCreate> {
    let Some(launch) = placement.inherited_launch.as_deref().cloned() else {
        return Err(crate::sessions::invalid_request(
            "the asking session's supervisor did not supply the launch to inherit".to_string(),
        ));
    };
    let farhelm_proto::launcher::TemplateFields {
        destination, name, ..
    } = &edits.edits;
    let only_folder_and_title = farhelm_proto::launcher::TemplateFields {
        destination: destination.clone(),
        name: name.clone(),
        ..Default::default()
    };
    if !edits.templates.is_empty() || edits.edits != only_folder_and_title || edits.host.is_some() {
        return Err(crate::sessions::invalid_request(
            "--inherit-agent is exclusive with every launch flag and template".to_string(),
        ));
    }
    let Some(farhelm_proto::launcher::TemplateDestination::Folder(cwd)) = destination.clone()
    else {
        return Err(crate::sessions::invalid_request(
            "farhelm spawn --inherit-agent needs --cwd".to_string(),
        ));
    };
    let Some(host_name) = views
        .iter()
        .find(|view| view.id == origin.host)
        .map(|view| view.name.clone())
    else {
        return Err(crate::sessions::no_such_host(origin.host));
    };
    Ok(ResolvedCreate {
        host: origin.host,
        host_name,
        resolution: crate::agent_launch::Resolution {
            cwd,
            launch,
            title: name.clone(),
            template_host: None,
        },
        template_identity: None,
    })
}

/// Everything an agent's new session would be, for the approval card and
/// the YOLO rule.
struct LaunchApproval {
    verb: LaunchVerb,
    host: HostId,
    host_name: String,
    cwd: String,
    title: Option<String>,
    launch: farhelm_proto::SessionLaunch,
    source: Option<farhelm_proto::approvals::ApprovalSession>,
}

/// The gate in front of every new session an agent asks for: the agent YOLO
/// rule, then the user's approval, then the rule, the request's own
/// connection and the target host's connection all rechecked for the time the
/// user took. The create path applies the rule once more at dispatch
/// (`sessions::do_create_session`).
///
/// The YOLO rule comes first so the user is never asked to approve something
/// the target host's own setting rules out (`yolo_guard::check_agent`), and
/// again after the approval, because the setting can change during the wait
/// (SPEC.md: the rule is applied again when the user approves). The origin
/// recheck is [`approved`]'s.
async fn approve_launch(
    state: &AppState,
    origin: AgentOrigin,
    asking_session: &str,
    approval: LaunchApproval,
) -> anyhow::Result<()> {
    crate::yolo_guard::check_agent(state, approval.host, &approval.launch).await?;
    let host = approval.host;
    let launch = approval.launch.clone();
    // The target as the card names it: the connection serving its registry
    // row now. A row can be retargeted or adopt a new install while the user
    // takes their time, and the approval was for the machine on the card.
    let target_before = state.manager.status(host).map(|status| status.incarnation);
    let action = ApprovalAction::Launch {
        verb: approval.verb,
        host_name: approval.host_name,
        cwd: approval.cwd,
        title: approval.title,
        launch: approval.launch,
        source: approval.source,
    };
    approved(state, origin, asking_session, action).await?;
    if state.manager.status(host).map(|status| status.incarnation) != target_before {
        return Err(anyhow::Error::new(crate::SupervisorError {
            origin: crate::client::ErrorOrigin::Helm,
            kind: ErrorKind::Unavailable,
            message: "the target host's connection changed while this request waited for \
                      approval, so nothing was started; retry"
                .to_string(),
        }));
    }
    crate::yolo_guard::check_agent(state, host, &launch).await
}

/// Ask the user to approve `action` for `asking_session` (see
/// `approvals::ask`) and, once approved, confirm that the connection the
/// request arrived on is still the one serving its host.
///
/// The recheck is what keeps an approval from carrying over to whatever
/// replaced or re-identified the host while the user took their time
/// (SPEC.md: an approval holds only for the connection the request arrived
/// on). It is the same check `handle` makes on the way in.
async fn approved(
    state: &AppState,
    origin: AgentOrigin,
    asking_session: &str,
    action: ApprovalAction,
) -> anyhow::Result<()> {
    crate::approvals::ask(state, origin, asking_session, action).await?;
    if !origin_is_live(state, origin) {
        return Err(anyhow::Error::new(crate::SupervisorError {
            origin: crate::client::ErrorOrigin::Helm,
            kind: ErrorKind::Unavailable,
            message: crate::approvals::ORIGIN_GONE_REFUSAL.to_string(),
        }));
    }
    Ok(())
}

/// A session's stored launch as the helm's cache knows it, for a restart
/// card. `None` when the cache does not know the session.
async fn cached_launch(state: &AppState, session_id: &str) -> Option<farhelm_proto::SessionLaunch> {
    let host = state.store.host_of_session(session_id).await.ok()??;
    state
        .store
        .cached_session(host, session_id)
        .await
        .ok()?
        .map(|info| info.launch)
}

/// The per-attempt inputs of an agent create's dispatch, beside its
/// resolution.
struct AgentCreateDispatch {
    intent_key: Option<String>,
    parent: Option<String>,
    spawned: bool,
    /// What the target matches a keyed retry by; see
    /// [`LaunchEditsRequest::request_fingerprint`].
    request_fingerprint: Option<String>,
}

/// The refusal for a create whose host came from a template, when the
/// connection it would be sent on reaches a different installation than the
/// one the template names; `None` when it may go ahead.
///
/// Compared against the claim taken with the client, which is the identity
/// of the connection the create would actually travel on, so a row pointed
/// at another machine, and its new installation adopted, after resolution
/// cannot carry the create to the replacement machine. A connection whose identity is
/// unknown is refused too: it is not shown to be the template's install.
fn template_host_moved(
    template_identity: Option<&str>,
    claim: &crate::manager::SessionClaim,
    host_name: &str,
) -> Option<anyhow::Error> {
    let expected = template_identity?;
    if claim.identity.as_deref() == Some(expected) {
        return None;
    }
    Some(anyhow::Error::new(crate::SupervisorError {
        origin: crate::client::ErrorOrigin::Helm,
        kind: ErrorKind::Conflict,
        message: format!(
            "the host {host_name} that the template names now reaches a different Farhelm \
             installation than the one the template was made for, so nothing was created; \
             check that host in the host list, or name a host with --host"
        ),
    }))
}

/// Send one resolved agent create to its host.
async fn dispatch_agent_create(
    state: &AppState,
    origin: AgentOrigin,
    asking_session: &str,
    resolved: ResolvedCreate,
    dispatch: AgentCreateDispatch,
) -> anyhow::Result<AgentReply> {
    // The claim and the client come from ONE read, which is what lets every
    // write the create goes on to make revalidate against the connection it
    // was actually sent on (see `sessions::host_client`).
    let (claim, client) = crate::sessions::host_client(state, resolved.host)?;
    let host_name = resolved.host_name.as_str();
    if let Some(refusal) =
        template_host_moved(resolved.template_identity.as_deref(), &claim, host_name)
    {
        return Err(refusal);
    }
    // The same paper trail [`resolve_target`] leaves for the lifecycle
    // verbs. `host_name` is the REGISTRY's own rendering of the matched row
    // rather than the string the request carried, so nothing
    // attacker-chosen reaches this line.
    info!(
        asking = asking_session,
        host = host_name,
        verb = "create",
        "an agent is creating a session"
    );
    let resolution = resolved.resolution;
    let session = on_host(
        crate::sessions::do_create_session(
            state,
            &claim,
            &client,
            crate::sessions::CreateSpec {
                cwd: resolution.cwd,
                mode: resolution.launch,
                title: resolution.title,
                cols: crate::sessions::default_cols(),
                rows: crate::sessions::default_rows(),
                intent_key: dispatch.intent_key,
                // Agent creates never carry a fresh-checkout payload: the
                // CLI refuses a template that names one.
                github_checkout: None,
                origin: crate::sessions::CreateOrigin::Agent,
                // `create` names a directory and a launch rather than a
                // session, so no answer of the target's is forbidden — a
                // keyed replay is the caller's own earlier create coming
                // back, which is what the key is for. Contrast
                // `clone_for_agent`, whose replay can be the ASKING session.
                accept_result: None,
                // Never an agent's to give: `approve_launch` applied the
                // agent YOLO rule, which refuses everything this override
                // would let through on a host that asks.
                confirm_yolo: false,
                settings_from_source: false,
                parent: dispatch.parent,
                spawned: dispatch.spawned,
                request_fingerprint: dispatch.request_fingerprint,
            },
        )
        .await,
        host_name,
    )?;
    Ok(agent_created_reply(
        state,
        &claim,
        session,
        origin.host,
        asking_session,
    ))
}

/// One `template create` or `template edit`, moved out of [`AgentVerb`].
struct TemplateWrite {
    name: String,
    fields: farhelm_proto::launcher::TemplateFields,
    /// The host to pin the template to, by display name.
    host: Option<String>,
    /// `create` refuses an existing name; `edit` refuses a missing one.
    create: bool,
}

/// `template create` and `template edit`: build the template the write would
/// store, show the whole of it on a card, and store it through the GUI
/// editor's own path (`templates::store_template`) once the user approves.
///
/// The host is named by display name and written as that host's recorded
/// install identity, which is how every template names a host; a `host` field
/// in the request itself is refused, so an agent cannot pin a template to an
/// install the helm did not resolve. A fresh-checkout destination is refused,
/// as the CLI refuses applying one. An edit sets only the fields it is given
/// (SPEC.md: no way to unset one). The template is read again after the
/// approval, and a write whose template changed during the wait is refused
/// rather than applied over a change the card did not show.
async fn write_template_for_agent(
    state: &Arc<AppState>,
    origin: AgentOrigin,
    asking_session: &str,
    write: TemplateWrite,
) -> anyhow::Result<AgentReply> {
    let TemplateWrite {
        name,
        mut fields,
        host,
        create,
    } = write;
    if fields.host.is_some() {
        return Err(crate::sessions::invalid_request(
            "name the template's host with --host, by the name farhelm agent hosts shows"
                .to_string(),
        ));
    }
    if matches!(
        fields.destination,
        Some(farhelm_proto::launcher::TemplateDestination::Github(_))
    ) {
        return Err(crate::sessions::invalid_request(
            "a template written from the farhelm command line cannot use a managed \
             checkout; give --cwd a folder"
                .to_string(),
        ));
    }
    if !create && fields == farhelm_proto::launcher::TemplateFields::default() && host.is_none() {
        return Err(crate::sessions::invalid_request(
            "give at least one field to change".to_string(),
        ));
    }
    // A YOLO assertion is about one command line, so a new command line
    // needs its own: keeping the old one would carry "not YOLO" over to a
    // command nobody said it about.
    if fields.command.is_some() && fields.yolo.is_none() {
        return Err(crate::sessions::invalid_request(
            "--command needs --yolo or --no-yolo".to_string(),
        ));
    }
    let mut host_name = None;
    if let Some(host) = host {
        let (id, name) = resolve_host(state, origin, host).await?;
        let identity = crate::hosts::host_views(state)
            .await?
            .into_iter()
            .find(|view| view.id == id)
            .and_then(|view| view.identity);
        let Some(identity) = identity else {
            return Err(crate::sessions::invalid_request(format!(
                "{name} has not reported an install yet, so a template cannot name it; retry \
                 once it has connected"
            )));
        };
        fields.host = Some(identity);
        host_name = Some(name);
    }
    let before = find_template(state, &name).await?;
    let template = match (&before, create) {
        (Some(_), true) => {
            return Err(anyhow::Error::new(crate::SupervisorError {
                origin: crate::client::ErrorOrigin::Helm,
                kind: ErrorKind::Conflict,
                message: format!(
                    "a template named {name:?} already exists; change it with farhelm agent \
                     template edit"
                ),
            }));
        }
        (None, false) => return Err(no_such_template(&name)),
        (None, true) => farhelm_proto::launcher::LaunchTemplate {
            name,
            fields: with_launch_kind(fields),
        },
        (Some(existing), false) => {
            // The merge would let `--command`'s kind replace the stored one;
            // a template that says which kind it is keeps saying so.
            if matches!((existing.fields.kind, fields.kind), (Some(was), Some(now)) if was != now) {
                return Err(changes_launch_kind(&name));
            }
            farhelm_proto::launcher::LaunchTemplate {
                name,
                fields: merge_template_fields(existing.fields.clone(), fields),
            }
        }
    };
    if mixes_launch_kinds(&template.fields) {
        return Err(changes_launch_kind(&template.name));
    }
    farhelm_proto::launcher::check_template_shape(&template)
        .map_err(crate::sessions::invalid_request)?;
    let shown_host = match (&host_name, &template.fields.host) {
        (Some(name), _) => Some(name.clone()),
        (None, Some(identity)) => template_host_name(state, identity).await,
        (None, None) => None,
    };
    approved(
        state,
        origin,
        asking_session,
        ApprovalAction::TemplateWrite {
            name: template.name.clone(),
            replaces_existing: !create,
            fields: template.fields.clone(),
            host_name: shown_host.clone(),
        },
    )
    .await?;
    info!(
        asking = escape_for_log(asking_session).as_str(),
        template = escape_for_log(&template.name).as_str(),
        "an agent is writing a launch template"
    );
    // The write lands only on the template the card was built from: another
    // agent's create of the same name, or a GUI edit made while the card
    // waited, refuses this one instead of being overwritten by it.
    let unchanged = crate::templates::Precondition::Unchanged(before.map(|t| t.fields));
    if !crate::templates::store_template(state, template.clone(), unchanged).await? {
        return Err(template_changed());
    }
    Ok(AgentReply::TemplateWritten {
        template: farhelm_proto::AgentTemplate::listed(&template, shown_host),
    })
}

/// `template delete`: show the template as it is on a card, and remove it
/// through the GUI's own path once the user approves, unless it changed
/// during the wait.
async fn delete_template_for_agent(
    state: &Arc<AppState>,
    origin: AgentOrigin,
    asking_session: &str,
    name: String,
) -> anyhow::Result<AgentReply> {
    let Some(existing) = find_template(state, &name).await? else {
        return Err(no_such_template(&name));
    };
    let host_name = match &existing.fields.host {
        Some(identity) => template_host_name(state, identity).await,
        None => None,
    };
    approved(
        state,
        origin,
        asking_session,
        ApprovalAction::TemplateDelete {
            name: name.clone(),
            fields: existing.fields.clone(),
            host_name,
        },
    )
    .await?;
    info!(
        asking = escape_for_log(asking_session).as_str(),
        template = escape_for_log(&name).as_str(),
        "an agent is deleting a launch template"
    );
    // Only the definition the card showed is deleted; one that changed or
    // went away while the card waited is reported as changed.
    let unchanged = crate::templates::Precondition::Unchanged(Some(existing.fields));
    if !crate::templates::remove_template(state, name, unchanged).await? {
        return Err(template_changed());
    }
    Ok(AgentReply::TemplateDeleted {})
}

/// Give a new template the launch kind its choices require, leaving placement-only
/// templates usable under either kind. An agent type beside command fields is
/// the command's declaration; without them it selects an agent launch.
///
/// Explicit kinds stay explicit, and mixed choices are still refused by the
/// caller. This inference runs only on create: an edit must keep the stored
/// template's kind, and no existing template is migrated.
fn with_launch_kind(
    mut fields: farhelm_proto::launcher::TemplateFields,
) -> farhelm_proto::launcher::TemplateFields {
    fields.kind = fields.kind.or(fields.implied_kind());
    fields
}

/// Whether `fields` holds choices only an agent launch takes beside choices
/// only a command launch takes (or a kind that contradicts either).
///
/// Such a template can never apply (`launcher::apply_template` refuses the
/// side that does not match the kind), and an agent cannot repair one: an
/// edit cannot unset a field, and an edit's flags never write the agent kind. Only
/// the agent verbs' own merge can produce one, so it is refused there; the
/// GUI's editor stores shape only (SPEC.md).
fn mixes_launch_kinds(fields: &farhelm_proto::launcher::TemplateFields) -> bool {
    use farhelm_proto::launcher::LauncherKind;
    let agent_side = fields.kind == Some(LauncherKind::Agent) || fields.sets_agent_only_fields();
    let command_side = fields.kind == Some(LauncherKind::Command) || fields.sets_command_fields();
    agent_side && command_side
}

/// `existing` with every field `edits` sets replaced, and every other field
/// left as it was. `None` in `edits` means "not given", never "unset": an
/// edit has no way to remove a field (SPEC.md).
fn merge_template_fields(
    existing: farhelm_proto::launcher::TemplateFields,
    edits: farhelm_proto::launcher::TemplateFields,
) -> farhelm_proto::launcher::TemplateFields {
    let farhelm_proto::launcher::TemplateFields {
        kind,
        agent,
        model,
        effort,
        permissions,
        workspace_trust,
        command,
        yolo,
        resume_command,
        host,
        destination,
        name,
    } = edits;
    farhelm_proto::launcher::TemplateFields {
        kind: kind.or(existing.kind),
        agent: agent.or(existing.agent),
        model: model.or(existing.model),
        effort: effort.or(existing.effort),
        permissions: permissions.or(existing.permissions),
        workspace_trust: workspace_trust.or(existing.workspace_trust),
        command: command.or(existing.command),
        yolo: yolo.or(existing.yolo),
        resume_command: resume_command.or(existing.resume_command),
        host: host.or(existing.host),
        destination: destination.or(existing.destination),
        name: name.or(existing.name),
    }
}

/// The template named `name`, if the helm holds one.
async fn find_template(
    state: &AppState,
    name: &str,
) -> anyhow::Result<Option<farhelm_proto::launcher::LaunchTemplate>> {
    Ok(state
        .store
        .launch_templates()
        .await?
        .into_iter()
        .find(|template| template.name == name))
}

/// The display name of the host whose recorded install is `identity`, if
/// one is registered.
async fn template_host_name(state: &AppState, identity: &str) -> Option<String> {
    let views = crate::hosts::host_views(state).await.ok()?;
    crate::agent_launch::template_host_row(&views, identity).map(|view| view.name.clone())
}

fn no_such_template(name: &str) -> anyhow::Error {
    anyhow::Error::new(crate::SupervisorError {
        origin: crate::client::ErrorOrigin::Helm,
        kind: ErrorKind::NotFound,
        message: format!("no template is named {name:?}; farhelm agent templates lists them"),
    })
}

/// The refusal of a template write that would switch a template between
/// the agent and command launch kinds, or leave it holding both kinds'
/// choices (see [`mixes_launch_kinds`]).
fn changes_launch_kind(name: &str) -> anyhow::Error {
    crate::sessions::invalid_request(format!(
        "this would change template {name:?} between an agent launch and a command launch, \
         or leave it with choices for both, which no launch can apply; an edit cannot change \
         a template's launch kind, so delete it and create it again"
    ))
}

fn template_changed() -> anyhow::Error {
    anyhow::Error::new(crate::SupervisorError {
        origin: crate::client::ErrorOrigin::Helm,
        kind: ErrorKind::Conflict,
        message: "the template changed while this request waited for approval, so nothing was \
                  written; look at it again and retry"
            .to_string(),
    })
}

/// `templates`: every template the helm holds, its command texts withheld
/// (`AgentTemplate::listed`), with the host each one's install identity
/// currently names.
async fn template_listing(state: &AppState, caller: HostId) -> anyhow::Result<AgentReply> {
    let views = crate::hosts::host_views(state).await?;
    let templates = state.store.launch_templates().await?;
    Ok(AgentReply::Templates {
        templates: templates
            .iter()
            .map(|template| {
                let host_name = template.fields.host.as_deref().and_then(|identity| {
                    crate::agent_launch::template_host_row(&views, identity)
                        .map(|view| view.name.clone())
                });
                farhelm_proto::AgentTemplate::listed(template, host_name)
            })
            .collect(),
        caller_host_id: caller.to_string(),
    })
}

/// `clone`: another session like an explicitly named source, on an
/// explicitly named host.
///
/// ## The source is read LIVE
///
/// From the source's current owning host, by draining its session list — the
/// same live read `sessions::get_session` performs for a connected host,
/// and for the same reason: the helm's cache is for the stale list, not a
/// serving layer, and a clone built from a cached row could copy a title
/// or a working directory the session no longer has.
///
/// ## Agent resolution
///
/// Copied by `sessions::mode_from_source`, as plain Replace copies it: an
/// agent or command launch is copied whole as stored (choices, composed or
/// written commands, YOLO answer, declared type, resume command), and a
/// legacy source (created before launch kinds) is refused with a remedy,
/// because its launch carries no YOLO answer to copy.
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
/// ## What a keyed retry is matched by
///
/// The request as the agent sent it ([`AgentRequestDigest::Clone`]): the
/// source session, the target host's name, and the overrides as given, as
/// for `create`. A retry repeating them replays the first attempt's session
/// even when the source's stored launch, folder or title changed in between;
/// the copy is still read live on every attempt, and a retry whose source is
/// gone, unreachable or legacy is refused before anything is sent.
async fn clone_for_agent(
    state: &AppState,
    origin: AgentOrigin,
    asking_session: &str,
    request: CloneRequest,
) -> anyhow::Result<AgentReply> {
    let request_fingerprint = request
        .intent_key
        .as_ref()
        .map(|_| {
            AgentRequestDigest::Clone {
                source: &request.source_session_id,
                host: &request.host,
                cwd: request.cwd.as_deref(),
                title: request.title.as_deref(),
            }
            .digest()
        })
        .transpose()?;
    let (source_claim, source_client) =
        crate::sessions::route_session(state, &request.source_session_id).await?;
    // Marked as the read-only phase it is: this listing runs before any
    // create is dispatched, so a transport failure here is retry-safe and
    // must not inherit the clone's own outcome-unknown vocabulary. See
    // [`ReadOnlyPhase`].
    let source = crate::manager::drain_sessions(&source_client)
        .await
        .map_err(|error| error.context(ReadOnlyPhase("reading the session to clone")))?
        .sessions
        .into_iter()
        .find(|session| session.id == request.source_session_id)
        .ok_or_else(|| {
            anyhow::Error::new(crate::SupervisorError {
 origin: crate::client::ErrorOrigin::Helm,
                kind: ErrorKind::NotFound,
                message: format!(
                    "the selected source host no longer lists session {:?}, so there is nothing to clone",
                    request.source_session_id
                ),
            })
        })?;

    let mode = crate::sessions::mode_from_source(
        &source,
        "the user can start a copy with Clone or Replace with, which choose a launch",
    )?;
    let (host, host_name) = resolve_host(state, origin, request.host).await?;
    let cwd = request.cwd.unwrap_or(source.cwd);
    // The source's title is copied VERBATIM, empty string included: a clone
    // that let the target derive a title from the directory would silently
    // rename the copy, which is the one thing a user reading two rows side by
    // side would notice first.
    let title = request.title.unwrap_or(source.title);
    approve_launch(
        state,
        origin,
        asking_session,
        LaunchApproval {
            verb: LaunchVerb::Clone,
            host,
            host_name: host_name.clone(),
            cwd: cwd.clone(),
            title: Some(title.clone()),
            launch: mode.clone(),
            source: Some(
                crate::approvals::describe_session(state, &request.source_session_id).await,
            ),
        },
    )
    .await?;
    // The source row is authoritative only while it still belongs to the
    // same owner connection. Re-resolving closes the read/dispatch window
    // (which now includes the user's approval) without trusting the helm's
    // stale cache as source truth.
    let (confirmed_claim, confirmed_client) =
        crate::sessions::route_session(state, &request.source_session_id).await?;
    if confirmed_claim != source_claim || !Arc::ptr_eq(&confirmed_client, &source_client) {
        return Err(anyhow::Error::new(crate::SupervisorError {
 origin: crate::client::ErrorOrigin::Helm,
            kind: ErrorKind::Conflict,
            message: "the source session's owner changed while it was being read; run discovery again before retrying the clone".to_string(),
        }));
    }
    let (claim, client) = crate::sessions::host_client(state, host)?;
    // See `create_for_agent`'s own note on why both logged values are safe.
    info!(
        asking = asking_session,
        source = request.source_session_id.as_str(),
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
                cwd,
                mode,
                title: Some(title),
                cols: crate::sessions::default_cols(),
                rows: crate::sessions::default_rows(),
                intent_key: asker_scoped_intent_key(asking_session, request.intent_key),
                github_checkout: None,
                origin: crate::sessions::CreateOrigin::Agent,
                // A clone that comes back as the SOURCE or ASKING session is
                // refused rather than reported. Either is reachable through
                // an idempotency-key replay, and this helm would otherwise
                // report an existing row as a new child to a caller whose next
                // move is to act on that id.
                // Answering "no new session was created" is the only honest
                // outcome, and `Conflict` says the caller's key is the thing
                // to change.
                //
                // It rides `accept_result` rather than an `if` after the call
                // because refusing AFTER the create's bookkeeping is refusing
                // too late: the cache seed and revision are effects of a
                // create this very refusal says did not happen. See
                // `sessions::do_create_session`'s "Two phases".
                accept_result: Some(Box::new({
                    let asking = asking_session.to_string();
                    let source = request.source_session_id.clone();
                    move |created: &farhelm_proto::SessionInfo| {
                        reject_clone_replay(&asking, &source, created)
                    }
                })),
                // Never an agent's to give; see `dispatch_agent_create`.
                confirm_yolo: false,
                settings_from_source: false,
                parent: None,
                spawned: false,
                request_fingerprint,
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

/// Refuse a clone result that is an existing participant rather than a new
/// child.
///
/// A target can legitimately replay either identity when an idempotency key
/// was already spent there. This check runs inside `do_create_session`'s
/// acceptance boundary so a refused replay cannot seed the helm cache or
/// publish a fleet revision first.
fn reject_clone_replay(
    asking_session: &str,
    source_session: &str,
    created: &farhelm_proto::SessionInfo,
) -> anyhow::Result<()> {
    if created.id != asking_session && created.id != source_session {
        return Ok(());
    }
    // "this host" rather than the name, because `on_host` wraps this
    // refusal with the host it is about, exactly as it wraps the target's
    // own refusals.
    Err(anyhow::Error::new(crate::SupervisorError {
 origin: crate::client::ErrorOrigin::Helm,
        kind: ErrorKind::Conflict,
        message: "the idempotency key replayed the create that made the source or caller session, so no copy was made; retry the clone with a key that has not been used on this host, or with none at all".to_string(),
    }))
}

/// Project a session a lifecycle verb just mutated into the same
/// [`AgentSession`] shape the `sessions` listing uses, so a rename
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

/// [`agent_session_reply`]'s restart twin, kept under a distinct reply tag.
///
/// A successful restart is an observed relaunch, not merely a session row
/// that happened to change. Keeping that fact in the response prevents a
/// malformed relay from making the CLI acknowledge a different lifecycle
/// operation as a restart.
fn agent_restarted_reply(
    state: &AppState,
    claim: &crate::manager::SessionClaim,
    info: farhelm_proto::SessionInfo,
    asking_host: HostId,
    asking_session: &str,
) -> AgentReply {
    AgentReply::Restarted {
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
                snapshot.alias.as_deref(),
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
        // Never read by `agent_session` below — the agent-facing `AgentSession`
        // reply carries no seen-state field at all, since that state is a
        // human-viewer fact (SPEC.md, Status) an agent verb has no use for.
        seen_activity_at: None,
        // Likewise never read: notifications are a human-viewer surface.
        notifications_read_through: 0,
        stale,
    };
    agent_session(&row, host_name, asking_host, asking_session)
}

/// Project the merged fleet listing into one reply, cut at
/// [`AGENT_REPLY_BYTE_BUDGET`] if the rows will not fit a frame.
///
/// Served from `aggregate::session_list` — the same function the UI's list
/// is built from, on purpose (see the module docs) — which answers with the
/// whole view up to the listing cap and says when that cap was hit. That
/// flag carries straight through: an agent asking for the fleet must be
/// able to tell a partial answer from the whole one, and the listing's own
/// `truncated` is exactly that distinction.
///
/// Each row is measured as it is projected and the projection stops BEFORE
/// the row that would exceed the allowance, so the helm never encodes a
/// reply it has already decided not to send.
async fn session_listing(
    state: &AppState,
    asking_host: HostId,
    asking_session: &str,
) -> anyhow::Result<AgentReply> {
    let filter = crate::store::SessionFilter::default();
    let sort = crate::store::ListSort::default();
    let listing =
        crate::aggregate::session_list(&state.manager, &state.store, &filter, sort).await?;
    let mut sessions: Vec<AgentSession> = Vec::new();
    let mut spent = 0usize;
    let mut truncated = listing.truncated;
    for row in &listing.sessions {
        let row = agent_session(
            row,
            Some(row.host_name.clone()),
            asking_host,
            asking_session,
        );
        // The ENCODED size, because that is what has to fit in a frame;
        // a struct's in-memory footprint says nothing about it. The
        // expect is a statement, not a hope: `AgentSession` is strings,
        // integers and booleans, which serde_json serializes infallibly,
        // so a failure here is a serializer defect — and it must be LOUD,
        // not laundered into a plausible-looking `truncated: true` that
        // hides the fault behind ordinary wording.
        let encoded = serde_json::to_vec(&row)
            .expect("an AgentSession is plain strings and numbers; serialization cannot fail")
            .len();
        if spent + encoded > AGENT_REPLY_BYTE_BUDGET {
            truncated = true;
            break;
        }
        spent += encoded;
        sessions.push(row);
    }
    Ok(AgentReply::Sessions {
        sessions,
        truncated,
        caller_host_id: asking_host.to_string(),
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
        id: view.id.to_string(),
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
/// Pure, for [`agent_host`]'s reason. The substitutions it makes — the
/// recorded agent kind's word standing in for the raw invocation, empty
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
        host_id: row.host.to_string(),
        host: host_name,
        title: row.info.title.clone(),
        cwd: row.info.cwd.clone(),
        agent: agent_label(&row.info),
        status: status_word(&row.info.status).to_string(),
        current: row.host == asking_host && row.info.id == asking_session,
        restart_offer: row.info.restart_offer,
        stale: row.stale,
    }
}

/// The non-secret name for what is running in a session.
///
/// The word for the integrated agent kind the supervisor recorded (`claude`,
/// `codex`, `goose`, `pi`, `omp`, `grok`), or [`CUSTOM_AGENT_LABEL`] when
/// there is no supported agent.
///
/// The invocation is never consulted, and that is the whole design. See
/// [`AgentSession::agent`] for why this label must not carry a credential:
/// the listing is fleet-wide and reachable with any one session's
/// credential. An earlier version took the invocation's first word and
/// stripped it to a basename, which leaked a leading `KEY=secret` whole
/// (`ANTHROPIC_API_KEY=sk-… claude` became the label) and read every `env …`
/// launch as `env`. Parsing the command line more cleverly would only move
/// the next such shape somewhere else; a label drawn from a closed vocabulary
/// cannot leak anything, whatever the user typed.
fn agent_label(info: &farhelm_proto::SessionInfo) -> String {
    match info.agent_kind {
        farhelm_proto::AgentKind::Generic => CUSTOM_AGENT_LABEL.to_string(),
        kind => kind.word().to_string(),
    }
}

/// The `agent` label for a session with no supported agent kind: a generic
/// program, or a row from a supervisor too old to report a kind at all.
const CUSTOM_AGENT_LABEL: &str = "custom";

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
/// so the word stays a word. Staleness is separate
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

/// A test's `farhelm agent create --command <command> --no-yolo` on a named
/// host: the create shape most relay tests need, spelled once.
#[cfg(test)]
pub(crate) fn command_create(
    host: &str,
    cwd: &str,
    command: &str,
    title: Option<&str>,
    intent_key: Option<&str>,
) -> AgentVerb {
    AgentVerb::Create {
        host: Some(host.to_string()),
        templates: Vec::new(),
        edits: farhelm_proto::launcher::TemplateFields {
            kind: Some(farhelm_proto::launcher::LauncherKind::Command),
            command: Some(command.to_string()),
            yolo: Some(false),
            destination: Some(farhelm_proto::launcher::TemplateDestination::Folder(
                cwd.to_string(),
            )),
            name: title.map(str::to_string),
            ..Default::default()
        },
        intent_key: intent_key.map(str::to_string),
        confirm_yolo: false,
        spawn: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hosts::{HostStateView, HostView, RefreshView};
    use farhelm_proto::{RestartOffer, SessionInfo};

    /// An agent's intent key reaches the target scoped to the session that
    /// asked, so two sessions using the same key never share a reservation.
    ///
    /// Why it matters: nothing else in a create or clone records who asked,
    /// so a child re-running its parent's keyed create used to get the
    /// parent's result back, which is the child itself, reported as the new
    /// session (SPEC.md "Agent-spawned sessions": keys are scoped to the
    /// asking session). Specified: the same key from two askers yields two
    /// different stored keys, the same asker and key always the same one,
    /// no key stays no key, and a key at the target's 512-byte limit still
    /// yields a scoped key within it.
    #[farhelm_testtrace::test]
    fn agent_intent_keys_are_scoped_to_the_asking_session() {
        let parent = asker_scoped_intent_key("parent-session", Some("k".into()));
        let child = asker_scoped_intent_key("child-session", Some("k".into()));
        assert_ne!(parent, child);
        assert_eq!(
            parent,
            asker_scoped_intent_key("parent-session", Some("k".into()))
        );
        assert_eq!(asker_scoped_intent_key("parent-session", None), None);
        let long = asker_scoped_intent_key(
            "7c9d1e2f-3a4b-4c5d-8e6f-7a8b9c0d1e2f",
            Some("x".repeat(512)),
        )
        .unwrap();
        assert!(long.len() <= 512, "{}", long.len());
    }

    /// The helm's own clone validation refuses an explicitly empty cwd.
    ///
    /// Why it matters: the helm treats supervisors as untrusted, and a clone
    /// with `cwd: Some("")` would otherwise replace the source session's
    /// directory with an empty path that only the target supervisor then
    /// refuses, possibly after the request's idempotency key was recorded.
    /// Spec: `Some("")` is refused here; an absent cwd stays legal.
    #[farhelm_testtrace::test]
    fn clone_validation_refuses_an_explicitly_empty_cwd() {
        let clone = |cwd: Option<&str>| AgentVerb::Clone {
            source_session_id: Some("source".to_string()),
            host: Some("host".to_string()),
            cwd: cwd.map(str::to_string),
            title: None,
            intent_key: None,
            confirm_yolo: false,
        };
        assert!(
            validate_authoritative_verb(&clone(Some("")))
                .unwrap_err()
                .contains("--cwd must not be empty")
        );
        assert!(validate_authoritative_verb(&clone(None)).is_ok());
        assert!(validate_authoritative_verb(&clone(Some("/work"))).is_ok());
    }

    fn host_view(id: i64, name: &str, kind: &'static str, state: HostStateView) -> HostView {
        HostView {
            id,
            kind,
            name: name.to_string(),
            destination: None,
            alias: None,
            identity: None,
            remote_farhelm: None,
            remote_state_dir: None,
            yolo_without_asking: false,
            commands_without_asking: false,
            icon: Default::default(),
            color: Default::default(),
            state,
            incarnation: 1,
        }
    }

    fn session_info(id: &str, status: SessionStatus) -> SessionInfo {
        SessionInfo {
            // Matches the `claude` invocation below, as a supervisor's
            // basename recognition would have recorded it.
            agent_kind: farhelm_proto::AgentKind::Claude,
            id: id.to_string(),
            parent: None,
            title: "a title".to_string(),
            created_at: 1,
            last_activity_at: 1,
            last_work_started_at: 0,
            creation_seq: None,
            cwd: "/w".to_string(),
            canonical_cwd: None,
            invocation: "claude --dangerously".to_string(),
            launch: farhelm_proto::SessionLaunch::plain_command("claude --dangerously"),
            status,
            annotation: None,
            restart_offer: RestartOffer::NotCaptured,
            tabs: Vec::new(),
            github_repo: None,
            working_copy: None,
            notifications: Vec::new(),
        }
    }

    fn session_row(info: SessionInfo, host_name: &str) -> crate::aggregate::SessionRow {
        crate::aggregate::SessionRow {
            info,
            host: 1,
            host_identity: None,
            host_name: host_name.to_string(),
            seen_activity_at: None,
            notifications_read_through: 0,
            stale: false,
        }
    }

    /// [`agent_session`] as the LISTING path calls it: with the row's own
    /// host name, which is the name a fleet snapshot already vouched for.
    ///
    /// A helper rather than the argument spelled out at a dozen call sites
    /// because the interesting variable in these tests is never the name —
    /// it is `current`, `agent`, `stale`. The one caller that
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
    #[farhelm_testtrace::test]
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
    #[farhelm_testtrace::test]
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
    #[farhelm_testtrace::test]
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
    #[farhelm_testtrace::test]
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
    #[farhelm_testtrace::test]
    fn hosts_are_projected_with_the_helms_own_phase_vocabulary() {
        let connected = host_view(
            1,
            "this machine",
            "local",
            HostStateView::Connected {
                identity: None,
                build_version: "0.0.0".to_string(),
                old_version: true,
                newer_version: false,
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

    /// Spec: a session's `agent` label is the supervisor-recorded agent
    /// kind's word, or `custom`; the invocation never contributes a single
    /// character.
    ///
    /// This is the fleet-wide label any attached session's credential can
    /// read, so it is a redaction boundary. The two command-line shapes
    /// pinned here are the ones that broke the old "first word's basename"
    /// rule: a leading `NAME=secret` assignment surfaced the secret whole,
    /// and every `env …` launch (including the helm's own composed Goose
    /// launches) read as `env`.
    #[farhelm_testtrace::test]
    fn a_sessions_agent_is_its_agent_kind_or_custom() {
        let raw = session_row(session_info("s1", SessionStatus::Running), "this machine");
        assert_eq!(projected(&raw, 1, "s1").agent, "claude");

        let mut leading_secret = session_info(
            "s2",
            SessionStatus::Error {
                detail: "exec failed".to_string(),
            },
        );
        leading_secret.invocation = "ANTHROPIC_API_KEY=sk-secret-value claude".to_string();
        leading_secret.agent_kind = farhelm_proto::AgentKind::Generic;
        let row = session_row(leading_secret, "this machine");
        assert_eq!(
            projected(&row, 1, "s1").agent,
            "custom",
            "a leading assignment must never reach this wire"
        );

        let mut composed_goose = session_info("s3", SessionStatus::Running);
        composed_goose.invocation =
            "env GOOSE_THINKING_EFFORT=high GOOSE_MODE=auto goose session".to_string();
        composed_goose.agent_kind = farhelm_proto::AgentKind::Goose;
        let row = session_row(composed_goose, "this machine");
        assert_eq!(projected(&row, 1, "s1").agent, "goose");

        let mut unrecognized = session_info("s4", SessionStatus::Running);
        unrecognized.invocation = "/opt/bin/my-tool --token abc123".to_string();
        unrecognized.agent_kind = farhelm_proto::AgentKind::Generic;
        let row = session_row(unrecognized, "this machine");
        assert_eq!(projected(&row, 1, "s1").agent, "custom");
    }

    /// Duplicate host labels must not erase the identity an agent needs to
    /// resolve a session from host context. JSON preserves the registry id
    /// from each row, even though both display names are identical.
    #[farhelm_testtrace::test]
    fn session_discovery_distinguishes_duplicate_host_names() {
        let first = session_row(session_info("s1", SessionStatus::Idle), "builder");
        let mut second = session_row(session_info("s2", SessionStatus::Idle), "builder");
        second.host = 2;
        let first = serde_json::to_value(projected(&first, 1, "s1")).unwrap();
        let second = serde_json::to_value(projected(&second, 1, "s1")).unwrap();
        assert_eq!(first["host"], second["host"]);
        assert_eq!(first["host_id"], "1");
        assert_eq!(second["host_id"], "2");
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
    #[farhelm_testtrace::test]
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
    /// Spec: every live and ended status becomes the word the UI shows, and
    /// `Unknown` becomes the empty string rather than a word of its own
    /// beside the six real ones.
    ///
    /// The `Unknown` case is the reason this test exists. That variant is
    /// wire plumbing whose own documentation forbids rendering it, and the
    /// cheap implementation — serialize the tag — would put the word
    /// "unknown" in front of an agent as though the helm had decided
    /// something about the session.
    #[farhelm_testtrace::test]
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
    // one host instead of the fleet, the local host only,
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
    fn scripted(id: &str, created_at: i64, agent_kind: farhelm_proto::AgentKind) -> SessionInfo {
        SessionInfo {
            invocation: "/usr/local/bin/claude --api-key sk-not-for-agents".to_string(),
            // What the supervisor records for the session; the label comes
            // from this, never the command line.
            agent_kind,
            ..session(id, created_at)
        }
    }

    /// A local host with three sessions (including one with another agent)
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
                    scripted("local-live", 30, farhelm_proto::AgentKind::Claude),
                    scripted("local-old", 20, farhelm_proto::AgentKind::Claude),
                    scripted("local-codex", 10, farhelm_proto::AgentKind::Codex),
                ],
                ..HostScript::default()
            })
            .await
            .ssh(
                "user@builder",
                HostScript {
                    identity: Some("identity-builder".to_string()),
                    sessions: vec![
                        scripted("remote-a", 40, farhelm_proto::AgentKind::Claude),
                        scripted("remote-b", 5, farhelm_proto::AgentKind::Claude),
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
        trust_without_asking(&h).await;
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
    #[farhelm_testtrace::test]
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
    /// WHOLE fleet — both hosts, each row flagged for staleness — with
    /// `current` on exactly the asking
    /// session's row and `truncated` false for a fleet that fits.
    ///
    /// This is the one test that exercises the assembly rather than the
    /// projection, and every clause is a bug that the pure unit tests above
    /// cannot see: a handler that listed only the local host, or passed the
    /// default filter, or listed one host and called it
    /// the fleet, would satisfy every one of them.
    #[farhelm_testtrace::test]
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
                    ..
                },
        } = outcome
        else {
            panic!("expected a sessions reply, got {outcome:?}");
        };

        assert!(!truncated, "five sessions is not a truncated fleet");
        let ids: Vec<&str> = sessions.iter().map(|s| s.id.as_str()).collect();
        for expected in [
            "local-live",
            "local-old",
            "local-codex",
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
        assert_eq!(by_id("local-codex").agent, "codex");
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
    /// reaches it. The listing's row cap says nothing about bytes, so
    /// without this allowance a fleet of fat records would assemble into a
    /// reply past `MAX_FRAME_LEN`, discarded whole at the writer's size
    /// backstop, reaching the agent as `Internal` instead of the partial
    /// listing the verb promises — after the helm had already paid to build
    /// and encode it, on up to four connections at once.
    ///
    /// The fixture is deliberately well past the allowance: 120 rows of
    /// 64 KiB is 7.5 MiB against a 6 MiB budget, split across two hosts so
    /// the cut lands inside a merged list rather than at one host's edge.
    #[farhelm_testtrace::test]
    async fn a_fat_fleet_is_cut_at_the_reply_byte_allowance() {
        const ROWS_PER_HOST: usize = 60;
        const TITLE_BYTES: usize = 64 * 1024;
        // Newest first, so the fixture reads like a real host's list; the
        // order is not validated anywhere, the merge sorts for itself.
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
                    ..
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
        // The BOUNDARY, pinned exactly: the retained rows fit the
        // allowance and the next (uniform) row would cross it. Encoded
        // sizes are measured the same way production measures them, so
        // this fails on an off-by-one (`>=` for `>`), a tally that resets
        // per host, or a budget change nobody meant.
        let spent: usize = sessions
            .iter()
            .map(|row| {
                serde_json::to_vec(row)
                    .expect("projected rows serialize")
                    .len()
            })
            .sum();
        let one_row = serde_json::to_vec(&sessions[0])
            .expect("projected rows serialize")
            .len();
        assert!(
            spent <= AGENT_REPLY_BYTE_BUDGET,
            "the retained rows must fit the allowance ({spent} > {AGENT_REPLY_BYTE_BUDGET})"
        );
        assert!(
            spent + one_row > AGENT_REPLY_BYTE_BUDGET,
            "the next row must be the one that crossed ({spent} + {one_row})"
        );
        let frame = response_frame(&outcome);
        assert!(
            !frame.exceeds_max_len(),
            "the reply must be sendable; it encoded to {} bytes",
            frame.encoded_len()
        );
    }

    /// Spec: a merged fleet past `farhelm_proto::LIST_SESSIONS_CAP` rows
    /// stops at the cap and reports `truncated`.
    ///
    /// The second ceiling, and the one that decides the answer when the
    /// rows are small enough that bytes never bind. Exercised through the
    /// production handler rather than `aggregate::session_list` alone,
    /// because this is where the listing's own `truncated` has to survive
    /// the projection into the agent reply.
    #[farhelm_testtrace::test]
    async fn a_fleet_past_the_row_cap_is_cut_at_the_cap() {
        // Split across two hosts because no single host may serve more than
        // the cap — the drain refuses past it — while the MERGED fleet can
        // exceed it, which is exactly the shape the merge's own cut exists
        // for.
        const PER_HOST: usize = farhelm_proto::LIST_SESSIONS_CAP / 2 + 100;
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
                    ..
                },
        } = outcome
        else {
            panic!("expected a sessions reply, got {outcome:?}");
        };
        assert_eq!(sessions.len(), farhelm_proto::LIST_SESSIONS_CAP);
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
    #[farhelm_testtrace::test]
    async fn the_production_handler_marks_the_asking_hosts_row() {
        let (h, local, _remote) = two_host_fleet().await;
        let handler = HelmAgentRequests::for_state(&h.state);

        let outcome = handler
            .handle(origin_of(&h, local), "local-live", AgentVerb::Hosts {})
            .await;
        let AgentOutcome::Ok {
            reply: AgentReply::Hosts { hosts, .. },
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
    #[farhelm_testtrace::test]
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
                reply: AgentReply::Hosts {
                    caller_host_id: "host-local".to_string(),
                    hosts: Vec::new(),
                    complete: true,
                },
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
    #[farhelm_testtrace::test]
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
    #[farhelm_testtrace::test]
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
    // The lifecycle verbs — Rename, Stop, Restart — against the same
    // production handler and the same real fleet machinery as the read-only
    // verbs above. Each drives the matching helper in `sessions.rs` through
    // a REAL routed call to a
    // scripted supervisor, so what is under test is the whole seam: origin
    // validation, explicit self and cross-session targets, the shared helper
    // functions the REST handlers also call, and the reply's
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
        trust_without_asking(&harness).await;
        (harness, local)
    }

    /// Turn on, for every registered host, both settings that let agent
    /// requests through without a person: "run farhelm commands from this
    /// host without asking" and "start YOLO sessions here without asking".
    ///
    /// The relay tests here are about routing, replies and refusals from the
    /// far end, not about asking; with the settings off every acting verb would
    /// wait for an approval card nobody answers, and every command launch would
    /// be refused by the agent YOLO rule. Turning the real settings on, rather
    /// than any bypass, is what the plan's R4 asks for. The gate itself is
    /// tested separately, with the settings off.
    async fn trust_without_asking(h: &Harness) {
        for row in h.store.list_hosts().await.unwrap() {
            h.store
                .set_commands_without_asking(row.id, true)
                .await
                .unwrap();
            h.store.set_yolo_without_asking(row.id, true).await.unwrap();
        }
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
    /// is not hypothetical: a cross-host lifecycle test hung exactly this
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

    /// Spec: an agent's restart of a session the helm still routes but whose
    /// cached launch it cannot read is refused as unavailable, with no card
    /// and nothing sent to the supervisor.
    ///
    /// Why: the approval card shows the launch a restart would resume, and
    /// the approved restart carries that launch to the supervisor as its
    /// precondition (`RestartSession::expected_launch`). With no launch to
    /// show, the user would be approving whatever the session holds by the
    /// time the restart runs, which SPEC.md's "an approval carries out
    /// exactly what the card showed" rules out. The owner lookup reads only
    /// the cached row's host, so an undecodable payload still routes; that is
    /// the state this test builds.
    #[farhelm_testtrace::test]
    async fn a_restart_whose_launch_cannot_be_read_is_refused_without_a_card() {
        let (client_side, _peer) = tokio::io::duplex(64 * 1024);
        let (h, local) =
            spliced_local_fleet(client_side, vec![session("target", 2), session("asker", 1)]).await;
        h.store
            .set_commands_without_asking(local, false)
            .await
            .unwrap();
        let _gui = h.state.manager.events().admit(1).expect("a GUI seat");
        let changed = h
            .store
            .connection_for_test()
            .lock()
            .execute(
                "UPDATE session_cache SET info_json = '{\"not\": \"a session\"}' \
                 WHERE session_id = 'target'",
                [],
            )
            .unwrap();
        assert_eq!(
            changed, 1,
            "fixture premise: the target's cached row exists"
        );
        assert_eq!(
            h.store.host_of_session("target").await.unwrap(),
            Some(local),
            "fixture premise: the target still routes"
        );
        assert_eq!(cached_launch(&h.state, "target").await, None);

        let outcome = HelmAgentRequests::for_state(&h.state)
            .handle(
                origin_of(&h, local),
                "asker",
                AgentVerb::Restart {
                    session_id: Some("target".to_string()),
                    stop_if_running: true,
                },
            )
            .await;
        match outcome {
            AgentOutcome::Err { kind, message } => {
                assert_eq!(kind, ErrorKind::Unavailable, "{message}");
                assert!(
                    message.contains("cannot read this session's launch"),
                    "{message}"
                );
            }
            other => panic!("refused, got {other:?}"),
        }
        assert!(h.state.approvals.list().is_empty(), "no card was shown");
    }

    /// A correlated wrong reply does not establish whether restart ran.
    /// Keep its payload private and require inspection before another mutation.
    #[farhelm_testtrace::test]
    async fn a_wrong_restart_reply_preserves_uncertainty_without_leaking_launch() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let (release, held) = tokio::sync::oneshot::channel();
        let responder = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .expect("handshake");
            let frame = reader.read_frame().await.unwrap().expect("restart request");
            let ControlMsg::RestartSession { req_id, .. } =
                parse_control(&frame).expect("decode restart")
            else {
                panic!("expected RestartSession");
            };
            let mut wrong = session("target", 2);
            // In both places a session's command text travels: the derived
            // display string and the launch itself.
            wrong.invocation = "private-launch-sentinel".to_string();
            wrong.launch = farhelm_proto::SessionLaunch::plain_command("private-launch-sentinel");
            writer
                .write_control(&ControlMsg::SessionRenamed {
                    req_id,
                    session: wrong,
                })
                .await
                .expect("send correlated wrong reply");
            // Keep the peer connected so connection loss cannot satisfy the assertion.
            held.await.expect("release after the relay answers");
        });
        let (h, local) =
            spliced_local_fleet(client_side, vec![session("target", 2), session("asker", 1)]).await;
        let outcome = tokio::time::timeout(
            RESPONDER_JOIN_BUDGET,
            HelmAgentRequests::for_state(&h.state).handle(
                origin_of(&h, local),
                "asker",
                AgentVerb::Restart {
                    session_id: Some("target".to_string()),
                    stop_if_running: true,
                },
            ),
        )
        .await
        .expect("relay must resolve the wrong reply");
        release.send(()).expect("peer remains connected");
        join_responder(responder, &outcome).await;
        let AgentOutcome::Err { kind, message } = outcome else {
            panic!("a wrong reply cannot acknowledge restart");
        };
        assert_eq!(kind, ErrorKind::Timeout, "{message}");
        assert!(message.contains("RestartSession"), "{message}");
        assert!(message.contains("SessionRenamed"), "{message}");
        assert!(
            message.contains(farhelm_proto::AGENT_MUTATION_UNKNOWN_REMEDY),
            "{message}"
        );
        assert!(!message.contains("private-launch-sentinel"), "{message}");
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
    #[farhelm_testtrace::test]
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
                expected_title,
                title,
            } = parse_control(&frame).expect("decode request")
            else {
                panic!("expected RenameSession");
            };
            assert_eq!(session_id, "other", "the NAMED target, not the asker");
            assert_eq!(expected_title.as_deref(), Some("old title"));
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
                    expected_title: Some("old title".to_string()),
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

    /// Spec: an omitted lifecycle target is refused at the authoritative
    /// helm boundary before any target supervisor is contacted.
    ///
    /// The relay performs the same check, but the helm must defend itself
    /// against an older or handcrafted request that bypassed that hop.
    #[farhelm_testtrace::test]
    async fn rename_with_no_session_id_is_refused_without_target_dispatch() {
        let (h, local, _remote) = two_host_fleet().await;
        let handler = HelmAgentRequests::for_state(&h.state);
        let outcome = handler
            .handle(
                origin_of(&h, local),
                "local-live",
                AgentVerb::Rename {
                    session_id: None,
                    expected_title: Some("old title".to_string()),
                    title: "self-renamed".to_string(),
                },
            )
            .await;
        match outcome {
            AgentOutcome::Err { kind, message } => {
                assert_eq!(kind, ErrorKind::InvalidRequest);
                assert!(message.contains("--session"), "{message}");
            }
            other => panic!("an omitted target must be refused, got {other:?}"),
        }
    }

    /// Spec: naming your OWN session id explicitly is legal; there is no
    /// "you may not name yourself" rule.
    ///
    /// This is asserted at the far end, where the forwarded `RenameSession`
    /// names the id. That distinguishes a deliberate self target from the
    /// old omitted-target shape, which the preceding test refuses before
    /// dispatch.
    ///
    /// `Rename` rather than `Stop` on purpose: it is the one
    /// lifecycle verb whose self-targeting form does not also end the
    /// asking session, so the scenario stays about target resolution.
    #[farhelm_testtrace::test]
    async fn naming_the_asking_session_explicitly_targets_it() {
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
                expected_title,
                title,
            } = parse_control(&frame).expect("decode request")
            else {
                panic!("expected RenameSession");
            };
            assert_eq!(
                session_id, "asker",
                "an explicit self-target forwards the asking session's id"
            );
            assert_eq!(expected_title.as_deref(), Some("old title"));
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
                    expected_title: Some("old title".to_string()),
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
                    "the explicitly named asker's row is current"
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
    #[farhelm_testtrace::test]
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
                    session_id: Some("asker".to_string()),
                    expected_title: Some("old title".to_string()),
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
    #[farhelm_testtrace::test]
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
                AgentVerb::Stop {
                    session_id: Some("asker".to_string()),
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

    /// Spec: `Stop`, like `Rename`, can target ANY session the helm knows,
    /// not only the asking one.
    ///
    /// `Rename`'s own cross-session test
    /// (`rename_can_target_any_named_session_and_returns_its_updated_row`)
    /// covers this for `Rename`; before this test, `Stop`'s only coverage
    /// used only an explicit self target, which cannot distinguish "targeted
    /// the named session" from "always acts on the asker and ignored the
    /// field" — a bug that field-substitution mistake would have shipped
    /// invisibly.
    #[farhelm_testtrace::test]
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
    #[farhelm_testtrace::test]
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
    #[farhelm_testtrace::test]
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
    // These need a scripted target supervisor that records the resolved
    // `CreateSession` bundle the helm sends. The catalog lookup has already
    // happened inside the helm; every test after this signpost is one fleet
    // plus one verb plus assertions on what crossed the target connection.
    // ---------------------------------------------------------------

    /// One `CreateSession` the helm sent, reduced to the fields these
    /// tests judge.
    ///
    /// Recorded rather than merely replied to, because almost every
    /// interesting property of the creating verbs is a property of the
    /// REQUEST: which launch bundle was sent, whether a clone copied the
    /// source's directory, whether the idempotency key survived the two hops.
    /// A test that only inspected the reply would pass against a helm that
    /// sent the wrong bundle and echoed the right row back.
    #[derive(Debug, Clone)]
    struct SeenCreate {
        cwd: String,
        launch: Option<farhelm_proto::SessionLaunch>,
        title: Option<String>,
        intent_key: Option<String>,
        parent: Option<String>,
        request_fingerprint: Option<String>,
    }

    /// Script a supervisor that answers every `CreateSession`, recording
    /// the fully resolved bundle it was asked to launch.
    ///
    /// Spawned on the PEER half of a duplex whose other half is handed to
    /// [`FleetBuilder`] — and spawned BEFORE the fleet is built, for the
    /// ordering reason [`spliced_local_fleet`] documents.
    ///
    /// Loops rather than answering once because a test may make several
    /// creates in a row.
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
        refusal: Option<String>,
    ) -> std::sync::Arc<std::sync::Mutex<Vec<SeenCreate>>> {
        spawn_create_responder_answering_in(peer, refusal, None)
    }

    /// [`spawn_create_responder`], answering every create with a session in
    /// `reply_cwd` when it is set, whatever folder the request named: how a
    /// supervisor answers a keyed retry it replays, with the session an
    /// earlier attempt created.
    fn spawn_create_responder_answering_in(
        peer: tokio::io::DuplexStream,
        refusal: Option<String>,
        reply_cwd: Option<String>,
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
                    ControlMsg::CreateSession {
                        req_id,
                        cwd,
                        launch,
                        title,
                        intent_key,
                        parent,
                        request_fingerprint,
                        ..
                    } => {
                        recorded.lock().expect("seen mutex").push(SeenCreate {
                            cwd: cwd.clone(),
                            launch: launch.clone(),
                            title: title.clone(),
                            intent_key,
                            parent,
                            request_fingerprint,
                        });
                        match refusal.clone() {
                            Some(message) => ControlMsg::Error {
                                req_id,
                                message,
                                kind: ErrorKind::InvalidRequest,
                            },
                            None => {
                                created += 1;
                                ControlMsg::SessionCreated {
                                    req_id,
                                    session: SessionInfo {
                                        cwd: reply_cwd.clone().unwrap_or(cwd),
                                        title: title.unwrap_or_default(),
                                        invocation: launch
                                            .as_ref()
                                            .map(farhelm_proto::SessionLaunch::display_command)
                                            .unwrap_or_default(),
                                        launch: launch.unwrap_or_else(|| {
                                            farhelm_proto::SessionLaunch::plain_command("")
                                        }),
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
        trust_without_asking(&h).await;
        (h, local, remote)
    }

    /// Store `fields` as the template `name` on `h`'s helm.
    async fn put_template(
        h: &Harness,
        name: &str,
        fields: farhelm_proto::launcher::TemplateFields,
    ) {
        h.state
            .store
            .put_launch_template(farhelm_proto::launcher::LaunchTemplate {
                name: name.to_string(),
                fields,
            })
            .await
            .expect("store template");
    }

    /// A command template: `command`, asserted not YOLO, in `/srv/t`.
    fn command_template(command: &str) -> farhelm_proto::launcher::TemplateFields {
        farhelm_proto::launcher::TemplateFields {
            kind: Some(farhelm_proto::launcher::LauncherKind::Command),
            command: Some(command.to_string()),
            yolo: Some(false),
            destination: Some(farhelm_proto::launcher::TemplateDestination::Folder(
                "/srv/t".to_string(),
            )),
            ..Default::default()
        }
    }

    /// A create naming only templates (and `key`), with no flags.
    fn template_create(templates: &[&str], key: Option<&str>) -> AgentVerb {
        AgentVerb::Create {
            host: None,
            templates: templates.iter().map(|name| name.to_string()).collect(),
            edits: farhelm_proto::launcher::TemplateFields::default(),
            intent_key: key.map(str::to_string),
            confirm_yolo: false,
            spawn: None,
        }
    }

    /// Spec: a create naming only a template takes its launch, folder,
    /// title and host from it: the template's host is matched by install
    /// identity, and `--cwd`/`--host` may then be omitted.
    ///
    /// Why: SPEC.md lets "a flag required by the verb, such as `--cwd` or
    /// the target host" be omitted when a template sets it; this pins that
    /// the helm, which owns the templates and the registry, resolves them,
    /// and that the identity selects the right installation.
    #[farhelm_testtrace::test]
    async fn a_template_supplies_the_launch_folder_title_and_host() {
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let seen = spawn_create_responder(peer, None);
        let (h, local, _remote) = creating_fleet(client_side, vec![session("asker", 1)]).await;
        put_template(
            &h,
            "builder-shell",
            farhelm_proto::launcher::TemplateFields {
                host: Some("identity-builder".to_string()),
                name: Some("from-template".to_string()),
                ..command_template("sh")
            },
        )
        .await;
        let outcome = HelmAgentRequests::for_state(&h.state)
            .handle(
                origin_of(&h, local),
                "asker",
                template_create(&["builder-shell"], None),
            )
            .await;
        assert!(
            matches!(
                outcome,
                AgentOutcome::Ok {
                    reply: AgentReply::Created { .. }
                }
            ),
            "{outcome:?}"
        );
        let seen = seen.lock().expect("seen mutex").clone();
        assert_eq!(seen.len(), 1, "the create reached the template's host");
        assert_eq!(seen[0].cwd, "/srv/t");
        assert_eq!(seen[0].title.as_deref(), Some("from-template"));
        assert_eq!(
            seen[0].launch,
            Some(farhelm_proto::SessionLaunch::Command(
                farhelm_proto::CommandLaunch {
                    command: "sh".to_string(),
                    yolo: false,
                    agent: None,
                    resume: None,
                }
            ))
        );
    }

    /// Spec: a keyed create sends its host a digest of the request as the
    /// agent sent it, and a retry repeating the request after the template it
    /// names was edited sends the same digest and key while launching the
    /// edited template; a different request under the key (another flag)
    /// sends another digest; an unkeyed create sends none; a keyed create
    /// without `--host` is refused naming it, with nothing sent.
    ///
    /// Why: the host replays a keyed create by that digest (SPEC.md,
    /// Agent-spawned sessions), so a retry across a template edit gets the
    /// first session back rather than a key-reuse refusal, and a reused key
    /// for another request is still refused. Only `--host` keeps a retry on
    /// the host holding the key's record.
    #[farhelm_testtrace::test]
    async fn a_keyed_create_sends_its_request_digest_across_a_template_edit() {
        let launched = |seen: &SeenCreate| match &seen.launch {
            Some(farhelm_proto::SessionLaunch::Command(command)) => command.command.clone(),
            other => panic!("a command launch: {other:?}"),
        };
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let seen = spawn_create_responder(peer, None);
        let (h, local, _remote) = creating_fleet(client_side, vec![session("asker", 1)]).await;
        let handler = HelmAgentRequests::for_state(&h.state);
        let keyed = |key: Option<&str>, title: Option<&str>| {
            let mut verb = template_create(&["t"], key);
            if let AgentVerb::Create { host, edits, .. } = &mut verb {
                *host = Some("user@builder".to_string());
                edits.name = title.map(str::to_string);
            }
            verb
        };
        let send = async |verb: AgentVerb| {
            let outcome = handler.handle(origin_of(&h, local), "asker", verb).await;
            assert!(
                matches!(
                    outcome,
                    AgentOutcome::Ok {
                        reply: AgentReply::Created { .. }
                    }
                ),
                "{outcome:?}"
            );
        };
        put_template(&h, "t", command_template("sh")).await;
        send(keyed(Some("k"), None)).await;
        put_template(&h, "t", command_template("bash")).await;
        send(keyed(Some("k"), None)).await;
        send(keyed(Some("k"), Some("renamed"))).await;
        send(keyed(None, None)).await;

        let sent = seen.lock().expect("seen mutex").clone();
        assert_eq!(sent.len(), 4, "{sent:?}");
        assert!(sent[0].request_fingerprint.is_some());
        assert_eq!(sent[0].intent_key, sent[1].intent_key);
        assert_eq!(
            sent[0].request_fingerprint, sent[1].request_fingerprint,
            "the retry repeats the request, so it repeats the digest"
        );
        assert_eq!(
            (launched(&sent[0]), launched(&sent[1])),
            ("sh".to_string(), "bash".to_string()),
            "every attempt resolves the templates afresh"
        );
        assert_ne!(
            sent[2].request_fingerprint, sent[0].request_fingerprint,
            "another request under the key is another digest"
        );
        assert_eq!(
            sent[3].request_fingerprint, None,
            "an unkeyed create sends none"
        );

        // A template that supplies the host is enough for an unkeyed create,
        // and exactly what a keyed one may not rely on.
        put_template(
            &h,
            "hosted",
            farhelm_proto::launcher::TemplateFields {
                host: Some("identity-builder".to_string()),
                ..command_template("sh")
            },
        )
        .await;
        send(template_create(&["hosted"], None)).await;
        let outcome = handler
            .handle(
                origin_of(&h, local),
                "asker",
                template_create(&["hosted"], Some("k")),
            )
            .await;
        let AgentOutcome::Err { message, .. } = outcome else {
            panic!("a keyed create without --host is refused: {outcome:?}");
        };
        assert!(
            message.contains("idempotency key needs --host"),
            "{message}"
        );
        assert_eq!(
            seen.lock().expect("seen mutex").len(),
            5,
            "the unkeyed create was sent and the refused keyed one was not"
        );
    }

    /// Spec: when a keyed create is answered with a session in another
    /// folder than the one this attempt resolved (a replay of an earlier
    /// attempt, made before a template moved the folder), the host's folder
    /// history records the folder the session has, not the retry's.
    ///
    /// Why: a request-matched retry resolves afresh, so its folder can differ
    /// from the replayed session's; recording the retry's would offer the
    /// user a folder no create used, paired with another folder's identity.
    #[farhelm_testtrace::test]
    async fn a_replayed_keyed_create_records_the_sessions_own_folder() {
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let seen = spawn_create_responder_answering_in(peer, None, Some("/srv/first".to_string()));
        let (h, local, remote) = creating_fleet(client_side, vec![session("asker", 1)]).await;
        put_template(&h, "t", command_template("sh")).await;
        let mut keyed = template_create(&["t"], Some("k"));
        if let AgentVerb::Create { host, .. } = &mut keyed {
            *host = Some("user@builder".to_string());
        }
        let outcome = HelmAgentRequests::for_state(&h.state)
            .handle(origin_of(&h, local), "asker", keyed)
            .await;
        assert!(
            matches!(
                outcome,
                AgentOutcome::Ok {
                    reply: AgentReply::Created { .. }
                }
            ),
            "{outcome:?}"
        );
        assert_eq!(
            seen.lock().expect("seen mutex")[0].cwd,
            "/srv/t",
            "premise: this attempt sent the template's current folder"
        );
        let folders = h
            .store
            .folder_history(remote, "identity-builder")
            .await
            .unwrap();
        assert_eq!(
            folders
                .iter()
                .map(|entry| entry.display_cwd.as_str())
                .collect::<Vec<_>>(),
            ["/srv/first"],
            "the template now resolves to /srv/t, which this session never used"
        );
    }

    /// Spec: the request digest covers the request as the agent sent it and
    /// nothing else. For a create: the `--host` name, the template list (in
    /// order), the flags (a sample here: title, folder, and a reset to the
    /// default), whether it is a spawn and its parent. For a clone:
    /// the source, the host name, and each override, an absent override
    /// distinct from one that names a value. A create and a clone never
    /// share a digest, and an inheriting spawn sends none. One request of
    /// each kind is pinned to its encoding, because the shape is frozen.
    ///
    /// Why: each field the digest missed would let a reused key for a
    /// different request replay the first session, and each field it wrongly
    /// covered (the resolution) would refuse an honest retry; the inheriting
    /// spawn keeps the resolved-launch fingerprint (Decision 5 of the plan
    /// that made this change).
    #[farhelm_testtrace::test]
    fn agent_request_digests_follow_the_request_as_sent() {
        use farhelm_proto::launcher::{TemplateDestination, TemplateFields};
        let create =
            |host: Option<&str>, templates: &[&str], edits: TemplateFields| LaunchEditsRequest {
                host: host.map(str::to_string),
                templates: templates.iter().map(|name| name.to_string()).collect(),
                edits,
                spawn: None,
            };
        let fingerprint = |request: &LaunchEditsRequest| {
            request
                .request_fingerprint()
                .unwrap()
                .expect("a create has a digest")
        };
        let base = fingerprint(&create(Some("a"), &["x", "y"], TemplateFields::default()));
        assert_eq!(base.len(), 64);
        assert_eq!(
            base,
            fingerprint(&create(Some("a"), &["x", "y"], TemplateFields::default())),
            "the same request is the same digest"
        );
        let variants = [
            create(Some("b"), &["x", "y"], TemplateFields::default()),
            create(None, &["x", "y"], TemplateFields::default()),
            create(Some("a"), &["y", "x"], TemplateFields::default()),
            create(
                Some("a"),
                &["x", "y"],
                TemplateFields {
                    name: Some("t".to_string()),
                    ..Default::default()
                },
            ),
            create(
                Some("a"),
                &["x", "y"],
                TemplateFields {
                    destination: Some(TemplateDestination::Folder("/w".to_string())),
                    ..Default::default()
                },
            ),
            create(
                Some("a"),
                &["x", "y"],
                TemplateFields {
                    resume_command: Some(None),
                    ..Default::default()
                },
            ),
        ];
        for variant in &variants {
            assert_ne!(fingerprint(variant), base, "{:?}", variant.edits);
        }
        let mut spawned = create(None, &["x", "y"], TemplateFields::default());
        let unspawned = fingerprint(&spawned);
        spawned.spawn = Some(farhelm_proto::SpawnPlacement::default());
        let orphan = fingerprint(&spawned);
        spawned.spawn = Some(farhelm_proto::SpawnPlacement {
            parent: Some("asker".to_string()),
            ..Default::default()
        });
        let parented = fingerprint(&spawned);
        assert!(unspawned != orphan && orphan != parented && unspawned != parented);
        spawned.spawn = Some(farhelm_proto::SpawnPlacement {
            parent: Some("asker".to_string()),
            inherit_agent: true,
            ..Default::default()
        });
        assert_eq!(spawned.request_fingerprint().unwrap(), None);

        let clone = |cwd: Option<&str>, title: Option<&str>| {
            AgentRequestDigest::Clone {
                source: "s",
                host: "a",
                cwd,
                title,
            }
            .digest()
            .unwrap()
        };
        let clones = [
            clone(None, None),
            clone(Some(""), None),
            clone(None, Some("")),
            AgentRequestDigest::Clone {
                source: "other",
                host: "a",
                cwd: None,
                title: None,
            }
            .digest()
            .unwrap(),
            AgentRequestDigest::Clone {
                source: "s",
                host: "b",
                cwd: None,
                title: None,
            }
            .digest()
            .unwrap(),
        ];
        let distinct: std::collections::HashSet<_> = clones.iter().collect();
        assert_eq!(distinct.len(), clones.len(), "{clones:?}");
        assert_eq!(clone(None, None), clone(None, None));
        let as_create = AgentRequestDigest::Create {
            host: Some("a"),
            templates: &[],
            edits: &TemplateFields::default(),
            spawned: false,
            parent: None,
        }
        .digest()
        .unwrap();
        assert!(
            !clones.contains(&as_create),
            "a create never shares a clone's digest"
        );

        // Pinned: outstanding keys on every host are stored under digests of
        // these encodings. A failure here means the encoding moved (a renamed
        // field, or a change inside `TemplateFields` or its enums), which
        // must get a new tag rather than an updated constant.
        let pinned_create = AgentRequestDigest::Create {
            host: Some("builder"),
            templates: &["base".to_string()],
            edits: &TemplateFields {
                agent: Some(farhelm_proto::LaunchHarness::Codex),
                model: Some(None),
                destination: Some(TemplateDestination::Folder("/srv/w".to_string())),
                name: Some("t".to_string()),
                ..Default::default()
            },
            spawned: true,
            parent: Some("asker"),
        };
        assert_eq!(
            serde_json::to_string(&pinned_create).unwrap(),
            r#"{"request":"agent_create_v1","host":"builder","templates":["base"],"edits":{"agent":"codex","model":null,"destination":{"folder":"/srv/w"},"name":"t"},"spawned":true,"parent":"asker"}"#
        );
        let pinned_clone = AgentRequestDigest::Clone {
            source: "s",
            host: "builder",
            cwd: Some("/srv/w"),
            title: None,
        };
        assert_eq!(
            serde_json::to_string(&pinned_clone).unwrap(),
            r#"{"request":"agent_clone_v1","source":"s","host":"builder","cwd":"/srv/w","title":null}"#
        );
    }

    /// Spec: an explicit `--host` wins over a template's host, even one
    /// naming an install no host has any more, and a create with neither is
    /// refused naming `--host`.
    ///
    /// Why: SPEC.md has explicit flags win over templates, and agents cannot
    /// edit templates, so a template pinned to a reinstalled host would
    /// otherwise be unusable from the CLI; the refusal is how an agent
    /// learns the host is its to name.
    #[farhelm_testtrace::test]
    async fn an_explicit_host_wins_over_a_templates_host() {
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let seen = spawn_create_responder(peer, None);
        let (h, local, _remote) = creating_fleet(client_side, vec![session("asker", 1)]).await;
        let handler = HelmAgentRequests::for_state(&h.state);
        put_template(
            &h,
            "stale",
            farhelm_proto::launcher::TemplateFields {
                host: Some("identity-gone".to_string()),
                ..command_template("sh")
            },
        )
        .await;
        put_template(&h, "plain", command_template("sh")).await;
        let mut with_host = template_create(&["stale"], None);
        if let AgentVerb::Create { host, .. } = &mut with_host {
            *host = Some("user@builder".to_string());
        }
        let outcome = handler
            .handle(origin_of(&h, local), "asker", with_host)
            .await;
        assert!(
            matches!(
                outcome,
                AgentOutcome::Ok {
                    reply: AgentReply::Created { .. }
                }
            ),
            "{outcome:?}"
        );
        assert_eq!(
            seen.lock().expect("seen mutex").len(),
            1,
            "it reached --host's host"
        );
        let outcome = handler
            .handle(
                origin_of(&h, local),
                "asker",
                template_create(&["plain"], None),
            )
            .await;
        let AgentOutcome::Err { message, .. } = outcome else {
            panic!("no host anywhere is refused: {outcome:?}");
        };
        assert!(message.contains("--host is required"), "{message}");
    }

    /// Spec: a create whose host came from a template is sent only while the
    /// host's connection still reaches the installation the template names.
    /// When the machine behind the host entry is replaced and its new
    /// identity adopted between resolution and dispatch, the create is
    /// refused saying so and nothing is created; a create resolved and sent
    /// before the replacement goes through. Explicit `--host` is not checked
    /// this way (`an_explicit_host_wins_over_a_templates_host`).
    ///
    /// Why: SPEC.md names a template's host by install identity so that a
    /// retargeted row makes the template's host stop applying "rather than
    /// silently aiming at the successor"; resolution checks the identity,
    /// but dispatch comes later and used to send on whatever the row then
    /// reached. The resolutions come from the
    /// real `resolve_agent_create`, so a resolution that stopped carrying
    /// the template's identity fails here too.
    #[farhelm_testtrace::test]
    async fn a_template_host_that_now_reaches_another_install_is_refused() {
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let seen = spawn_create_responder(peer, None);
        let (h, local, remote) = creating_fleet(client_side, vec![session("asker", 1)]).await;
        put_template(
            &h,
            "builder-shell",
            farhelm_proto::launcher::TemplateFields {
                host: Some("identity-builder".to_string()),
                ..command_template("sh")
            },
        )
        .await;
        let request = LaunchEditsRequest {
            host: None,
            templates: vec!["builder-shell".to_string()],
            edits: farhelm_proto::launcher::TemplateFields::default(),
            spawn: None,
        };
        let resolve = async || {
            resolve_agent_create(&h.state, origin_of(&h, local), &request)
                .await
                .expect("the template's host resolves")
        };
        let dispatch = || AgentCreateDispatch {
            intent_key: None,
            parent: None,
            spawned: false,
            request_fingerprint: None,
        };
        let before = resolve().await;
        let held = resolve().await;
        assert_eq!(held.host, remote);
        assert_eq!(held.template_identity.as_deref(), Some("identity-builder"));

        dispatch_agent_create(&h.state, origin_of(&h, local), "asker", before, dispatch())
            .await
            .expect("sent while the host still reaches the template's install");
        assert_eq!(seen.lock().expect("seen mutex").len(), 1);

        // The machine behind the entry is replaced and the user adopts the
        // new install, all between `held`'s resolution and its dispatch.
        h.fleet.edit(remote, |script| {
            script.identity = Some("identity-replacement".to_string());
        });
        h.fleet.kill_connection(remote);
        h.await_state(remote, |state| state.phase() == "identity-mismatch")
            .await;
        h.manager
            .adopt(remote, "identity-replacement")
            .await
            .expect("adopt the replacement");
        h.await_refreshed_as(remote, "identity-replacement", 0)
            .await;

        let refused =
            dispatch_agent_create(&h.state, origin_of(&h, local), "asker", held, dispatch())
                .await
                .expect_err("a template host now reaching another install is refused");
        let message = format!("{refused:#}");
        assert!(
            message.contains("different Farhelm installation"),
            "{message}"
        );
        assert_eq!(
            seen.lock().expect("seen mutex").len(),
            1,
            "no second create reached the template's install; the refusal above came before any send \
             (the scripted replacement answers only listings, so a sent create would hang, not return)"
        );
    }

    /// Spec: a spawn with launch flags creates on the asking session's own
    /// host and records the parent it names; a template that sets a host,
    /// and a parent other than the asking session, are refused for a spawn,
    /// with nothing sent.
    ///
    /// Why: SPEC.md keeps `farhelm spawn` to its own host whichever way its
    /// launch is described, and a parent may only ever be the asking session;
    /// the helm is the one creating it here.
    #[farhelm_testtrace::test]
    async fn a_spawn_creates_on_its_own_host_with_its_parent() {
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let seen = spawn_create_responder(peer, None);
        let (h, _local, remote) = creating_fleet(client_side, vec![session("asker", 1)]).await;
        let handler = HelmAgentRequests::for_state(&h.state);
        let spawn = |templates: Vec<String>| AgentVerb::Create {
            host: None,
            templates,
            edits: farhelm_proto::launcher::TemplateFields {
                destination: Some(farhelm_proto::launcher::TemplateDestination::Folder(
                    "/srv/child".to_string(),
                )),
                ..command_template("sh")
            },
            intent_key: None,
            confirm_yolo: false,
            spawn: Some(farhelm_proto::SpawnPlacement {
                parent: Some("asker".to_string()),
                ..Default::default()
            }),
        };
        let outcome = handler
            .handle(origin_of(&h, remote), "asker", spawn(Vec::new()))
            .await;
        assert!(
            matches!(
                outcome,
                AgentOutcome::Ok {
                    reply: AgentReply::Created { .. }
                }
            ),
            "{outcome:?}"
        );
        put_template(
            &h,
            "elsewhere",
            farhelm_proto::launcher::TemplateFields {
                host: Some("identity-local".to_string()),
                ..Default::default()
            },
        )
        .await;
        let outcome = handler
            .handle(
                origin_of(&h, remote),
                "asker",
                spawn(vec!["elsewhere".to_string()]),
            )
            .await;
        let AgentOutcome::Err { kind, message } = outcome else {
            panic!("a host-setting template is refused for a spawn: {outcome:?}");
        };
        assert_eq!(kind, ErrorKind::InvalidRequest);
        assert!(
            message.contains("farhelm spawn always creates on its own host"),
            "{message}"
        );
        let mut foreign = spawn(Vec::new());
        if let AgentVerb::Create { spawn, .. } = &mut foreign {
            *spawn = Some(farhelm_proto::SpawnPlacement {
                parent: Some("someone-else".to_string()),
                ..Default::default()
            });
        }
        let outcome = handler
            .handle(origin_of(&h, remote), "asker", foreign)
            .await;
        let AgentOutcome::Err { kind, message } = outcome else {
            panic!("a foreign parent is refused: {outcome:?}");
        };
        assert_eq!(kind, ErrorKind::Unauthorized);
        assert!(message.contains("may name only itself"), "{message}");
        let seen = seen.lock().expect("seen mutex").clone();
        assert_eq!(seen.len(), 1, "only the first spawn was sent: {seen:?}");
        assert_eq!(seen[0].cwd, "/srv/child");
        assert_eq!(seen[0].parent.as_deref(), Some("asker"));
    }

    /// Spec: the templates verb lists every template by name with what it
    /// sets, withholding command and resume-command text, and names the
    /// host a template's install identity currently resolves to.
    ///
    /// Why: this is an agent's only view of templates (SPEC.md: "listed as
    /// set without their text"), and the host name is how an agent reads a
    /// template's identity-valued host field.
    #[farhelm_testtrace::test]
    async fn the_templates_listing_withholds_command_text() {
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let _seen = spawn_create_responder(peer, None);
        let (h, local, _remote) = creating_fleet(client_side, vec![session("asker", 1)]).await;
        put_template(
            &h,
            "builder-shell",
            farhelm_proto::launcher::TemplateFields {
                host: Some("identity-builder".to_string()),
                ..command_template("secret-tool --token x")
            },
        )
        .await;
        let outcome = HelmAgentRequests::for_state(&h.state)
            .handle(origin_of(&h, local), "asker", AgentVerb::Templates {})
            .await;
        let AgentOutcome::Ok {
            reply: AgentReply::Templates { templates, .. },
        } = outcome
        else {
            panic!("a templates listing: {outcome:?}");
        };
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].name, "builder-shell");
        assert!(templates[0].sets_command);
        assert_eq!(templates[0].host_name.as_deref(), Some("user@builder"));
        assert!(!format!("{templates:?}").contains("secret-tool"));
    }

    /// Spec: `create --host <name>` naming an unregistered host is
    /// `NotFound`, quoting the name and listing what does exist.
    ///
    /// The known-hosts list is part of the contract rather than decoration:
    /// the agent's next move after this refusal is to pick a real host, and
    /// a bare "no such host" would send it back for a second round trip to
    /// `farhelm agent hosts` — which is the listing this refusal is
    /// summarizing.
    #[farhelm_testtrace::test]
    async fn create_on_an_unknown_host_name_is_not_found() {
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let _seen = spawn_create_responder(peer, None);
        let (h, local, _remote) = creating_fleet(client_side, vec![session("asker", 1)]).await;

        let handler = HelmAgentRequests::for_state(&h.state);
        let outcome = handler
            .handle(
                origin_of(&h, local),
                "asker",
                command_create("nowhere", "/srv/work", "sh", None, None),
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

    /// Spec: once a host carries an alias, `create --host <alias>` resolves
    /// it exactly as the raw destination used to — `resolve_host` matches
    /// `view.name`, and the alias IS that name once set
    /// (`aggregate::host_display_name`), so this needs no code path of its
    /// own to prove, only that the wire actually reaches the aliased host.
    #[farhelm_testtrace::test]
    async fn create_resolves_by_alias_once_one_is_set() {
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let seen = spawn_create_responder(peer, None);
        let (h, local, remote) = creating_fleet(client_side, vec![session("asker", 1)]).await;
        h.store
            .update_alias(remote, Some("builder-alias"))
            .await
            .expect("alias the remote host");
        // A direct store write, unlike the REST route, does not itself
        // reach the manager's live snapshot — `set_alias`'s handler
        // explicitly resyncs afterward (hosts.rs), and `resolve_host` reads
        // `host_views`, which is built from that snapshot rather than the
        // store. Skipping this would leave the alias written but invisible
        // to resolution, silently testing nothing.
        h.manager
            .sync_registry()
            .await
            .expect("resync the manager after aliasing directly through the store");

        let handler = HelmAgentRequests::for_state(&h.state);
        let outcome = handler
            .handle(
                origin_of(&h, local),
                "asker",
                command_create("builder-alias", "/srv/work", "sh", None, None),
            )
            .await;
        match outcome {
            AgentOutcome::Ok {
                reply: AgentReply::Created { session },
            } => {
                // The mutation-reply projection (agent_requests.rs's own
                // `Some(crate::aggregate::host_display_name(...))` call) is
                // an INDEPENDENT derivation from the merged-listing one
                // `session_rows_carry_the_alias_in_host_name` (hosts.rs)
                // pins — asserting only `Created { .. }` above would still
                // pass if that line were deleted while alias-based
                // RESOLUTION kept working, since resolution and the
                // reply's own host name are two different pieces of code.
                assert_eq!(
                    session.host.as_deref(),
                    Some("builder-alias"),
                    "the reply must name the host by its alias, not its destination"
                );
            }
            other => panic!(
                "the alias must resolve to the same host the raw destination used to name: {other:?}"
            ),
        }
        assert_eq!(
            seen.lock().expect("seen mutex").len(),
            1,
            "the create must have actually reached the remote host, not merely been accepted"
        );
    }

    /// Spec: once a host is aliased, its RAW destination stops resolving —
    /// the alias replaces it as the display name rather than adding a
    /// second name for the same host — and the refusal's known-hosts list
    /// shows the alias, since that is genuinely the only name left that
    /// would have worked. This is host-aliases' one deliberate behavior
    /// change to existing name resolution, so it gets its own test rather
    /// than riding along with the alias-resolves case above.
    #[farhelm_testtrace::test]
    async fn create_on_the_raw_destination_is_refused_once_aliased() {
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let _seen = spawn_create_responder(peer, None);
        let (h, local, remote) = creating_fleet(client_side, vec![session("asker", 1)]).await;
        h.store
            .update_alias(remote, Some("builder-alias"))
            .await
            .expect("alias the remote host");
        // A direct store write, unlike the REST route, does not itself
        // reach the manager's live snapshot — `set_alias`'s handler
        // explicitly resyncs afterward (hosts.rs), and `resolve_host` reads
        // `host_views`, which is built from that snapshot rather than the
        // store. Skipping this would leave the alias written but invisible
        // to resolution, silently testing nothing.
        h.manager
            .sync_registry()
            .await
            .expect("resync the manager after aliasing directly through the store");

        let handler = HelmAgentRequests::for_state(&h.state);
        let outcome = handler
            .handle(
                origin_of(&h, local),
                "asker",
                command_create("user@builder", "/srv/work", "sh", None, None),
            )
            .await;
        match outcome {
            AgentOutcome::Err { kind, message } => {
                assert_eq!(kind, ErrorKind::NotFound);
                assert!(
                    message.contains("user@builder"),
                    "the refusal quotes the name that failed: {message}"
                );
                let known_hosts_part = message
                    .split_once("known hosts: ")
                    .map(|(_, rest)| rest)
                    .expect("the refusal must list known hosts");
                assert!(
                    known_hosts_part.contains("builder-alias"),
                    "the known-hosts list must show the ALIAS: {message}"
                );
                assert!(
                    !known_hosts_part.contains("user@builder"),
                    "the raw destination must no longer appear as a resolvable name: {message}"
                );
            }
            other => panic!("expected a NotFound refusal, got {other:?}"),
        }
    }

    /// Spec: `farhelm agent clone` sends the source's stored launch to the
    /// target verbatim, for a command launch and for an agent launch alike.
    ///
    /// Why: the clone must run what the source runs, with the same YOLO
    /// answer, declared agent type and resume command, not something the
    /// helm rebuilds. The agent case uses a start command today's catalog
    /// would not compose, so a clone that recompiled the selection instead
    /// of copying would fail it. The source deliberately differs from the
    /// caller and the destination is another host, proving neither identity
    /// is inferred.
    #[farhelm_testtrace::test]
    async fn clone_sends_the_source_launch_verbatim() {
        let command = farhelm_proto::SessionLaunch::Command(farhelm_proto::CommandLaunch {
            command: "sh -c 'echo hi' {farhelm_args}".to_string(),
            yolo: false,
            agent: Some(farhelm_proto::LaunchHarness::Claude),
            resume: Some("sh -c 'echo hi' --resume {conversation} {farhelm_args}".to_string()),
        });
        let agent = farhelm_proto::SessionLaunch::Agent {
            selection: farhelm_proto::LaunchSelection {
                harness: farhelm_proto::LaunchHarness::Claude,
                model: None,
                effort: None,
                permissions: Some(farhelm_proto::LaunchPermission::Approve),
                workspace_trust: None,
            },
            start: ["claude", "--from-an-older-catalog", "{farhelm_args}"]
                .map(String::from)
                .to_vec(),
            resume: None,
        };
        for launch in [command, agent] {
            let (client_side, peer) = tokio::io::duplex(64 * 1024);
            let seen = spawn_create_responder(peer, None);

            let source = SessionInfo {
                cwd: "/srv/project".to_string(),
                canonical_cwd: None,
                invocation: launch.display_command(),
                launch: launch.clone(),
                ..session("source", 1)
            };
            let (h, local, _remote) = creating_fleet(client_side, vec![source]).await;

            let handler = HelmAgentRequests::for_state(&h.state);
            let outcome = handler
                .handle(
                    origin_of(&h, local),
                    "asker",
                    AgentVerb::Clone {
                        source_session_id: Some("source".to_string()),
                        host: Some("user@builder".to_string()),
                        cwd: None,
                        title: None,
                        intent_key: Some("clone-key".to_string()),
                        confirm_yolo: false,
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
                "the clone must succeed: {outcome:?}"
            );

            let seen = seen.lock().expect("seen mutex").clone();
            assert_eq!(seen.len(), 1);
            assert_eq!(
                seen[0].launch,
                Some(launch.clone()),
                "the launch reaches the target whole, with nothing added"
            );
            assert_eq!(
                seen[0].cwd, "/srv/project",
                "the clone copies the source's directory"
            );
            assert_eq!(seen[0].title.as_deref(), Some("source"));
            assert_eq!(
                seen[0].intent_key,
                asker_scoped_intent_key("asker", Some("clone-key".to_string())),
                "the target stores the key scoped to the asking session"
            );
        }
    }

    /// Spec: a keyed clone retried with the same source, host and overrides
    /// sends the same request digest and key even after the source's stored
    /// launch and title changed, while copying the source as it is now; an
    /// override that names a value, even the source's own, is another
    /// request and another digest.
    ///
    /// Why: the target replays a keyed clone by that digest (SPEC.md,
    /// Agent-spawned sessions), so the retry gets the first copy back rather
    /// than a key-reuse refusal after the source was edited; an override the
    /// agent spelled out differs from one it left to the source.
    #[farhelm_testtrace::test]
    async fn a_keyed_clone_sends_its_request_digest_across_a_source_change() {
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let seen = spawn_create_responder(peer, None);
        let source = |command: &str, title: &str| {
            let launch = farhelm_proto::SessionLaunch::plain_command(command);
            SessionInfo {
                cwd: "/srv/project".to_string(),
                canonical_cwd: None,
                invocation: launch.display_command(),
                launch,
                title: title.to_string(),
                ..session("source", 1)
            }
        };
        let (h, local, _remote) = creating_fleet(client_side, vec![source("sh", "before")]).await;
        let handler = HelmAgentRequests::for_state(&h.state);
        let clone = |title: Option<&str>| AgentVerb::Clone {
            source_session_id: Some("source".to_string()),
            host: Some("user@builder".to_string()),
            cwd: None,
            title: title.map(str::to_string),
            intent_key: Some("clone-key".to_string()),
            confirm_yolo: false,
        };
        let created = |outcome: &AgentOutcome| {
            matches!(
                outcome,
                AgentOutcome::Ok {
                    reply: AgentReply::Created { .. }
                }
            )
        };
        let first = handler
            .handle(origin_of(&h, local), "asker", clone(None))
            .await;
        assert!(created(&first), "{first:?}");
        h.fleet.edit(local, |script| {
            script.sessions = vec![source("bash", "after")];
        });
        let retry = handler
            .handle(origin_of(&h, local), "asker", clone(None))
            .await;
        assert!(created(&retry), "{retry:?}");
        let spelled = handler
            .handle(origin_of(&h, local), "asker", clone(Some("after")))
            .await;
        assert!(created(&spelled), "{spelled:?}");

        let seen = seen.lock().expect("seen mutex").clone();
        assert_eq!(seen.len(), 3, "{seen:?}");
        assert!(seen[0].request_fingerprint.is_some());
        assert_eq!(seen[0].intent_key, seen[1].intent_key);
        assert_eq!(seen[0].request_fingerprint, seen[1].request_fingerprint);
        assert_eq!(
            (seen[0].title.as_deref(), seen[1].title.as_deref()),
            (Some("before"), Some("after")),
            "each attempt copies the source as it is now"
        );
        assert_ne!(
            seen[2].request_fingerprint, seen[1].request_fingerprint,
            "a title spelled out is another request than one left to the source"
        );
    }

    /// A clone refuses a source row whose owner changes after the live read.
    ///
    /// The held list reply is a snapshot from the original owner. While it is
    /// in flight, the manager's ordinary fenced cache APIs move the source to
    /// the destination host, and a fresh routing lookup proves that the move
    /// is visible before the old reply is released. Accepting that reply would
    /// let stale source state cross the ownership boundary and create a child
    /// on a host the source did not belong to when dispatch began.
    #[farhelm_testtrace::test]
    async fn clone_refuses_when_the_source_owner_changes_after_its_live_read() {
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let seen = spawn_create_responder(peer, None);
        let source = SessionInfo {
            cwd: "/srv/source".to_string(),
            canonical_cwd: None,
            invocation: "agent --source".to_string(),
            ..session("source", 1)
        };
        let (h, local, remote) =
            creating_fleet(client_side, vec![source.clone(), session("asker", 2)]).await;
        let (old_claim, _) = crate::sessions::route_session(&h.state, "source")
            .await
            .expect("the source starts on the asking host");
        assert_eq!(old_claim.host, local, "the fixture starts with one owner");
        let (new_claim, _) = crate::sessions::host_client(&h.state, remote)
            .expect("the destination host is connected");

        // The first two list requests are the fleet's initial refreshes.
        // Holding the next one parks clone_for_agent's live source read after
        // the harness has already built its reply from the old owner's row.
        let release_old_read = h.fleet.hold_next_list(local);
        let state = Arc::clone(&h.state);
        let origin = origin_of(&h, local);
        let clone = tokio::spawn(async move {
            HelmAgentRequests::for_state(&state)
                .handle(
                    origin,
                    "asker",
                    AgentVerb::Clone {
                        source_session_id: Some("source".to_string()),
                        host: Some("user@builder".to_string()),
                        cwd: None,
                        title: None,
                        intent_key: Some("owner-race".to_string()),
                        confirm_yolo: false,
                    },
                )
                .await
        });
        h.fleet.await_list_requests(3).await;

        h.manager
            .forget_session(&old_claim, "source")
            .await
            .expect("remove the old ownership claim");
        h.manager
            .remember_session(&new_claim, &source)
            .await
            .expect("publish the new ownership claim");
        let (moved_claim, _) = crate::sessions::route_session(&h.state, "source")
            .await
            .expect("the moved source is routable");
        assert_eq!(
            moved_claim.host, remote,
            "the owner changes before the stale read completes"
        );

        release_old_read
            .send(())
            .expect("the clone still owns the held read");
        let outcome = clone.await.expect("the clone task must not panic");
        match outcome {
            AgentOutcome::Err { kind, message } => {
                assert_eq!(kind, ErrorKind::Conflict);
                assert!(
                    message.contains("owner changed") && message.contains("discovery"),
                    "the refusal must explain the stale selection and remedy: {message}"
                );
            }
            other => panic!("an owner-changing clone must be refused, got {other:?}"),
        }
        assert!(
            seen.lock().expect("seen mutex").is_empty(),
            "the destination must not receive a create from a stale source read"
        );
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
    #[farhelm_testtrace::test]
    async fn a_create_refusal_from_the_target_reaches_the_agent_verbatim() {
        const SENTINEL: &str = "SENTINEL-agent-create: no such directory: /srv/absent";

        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let seen = spawn_create_responder(peer, Some(SENTINEL.to_string()));
        let (h, local, _remote) = creating_fleet(client_side, vec![session("asker", 1)]).await;

        let handler = HelmAgentRequests::for_state(&h.state);
        let outcome = handler
            .handle(
                origin_of(&h, local),
                "asker",
                command_create("user@builder", "/srv/absent", "claude", None, None),
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
    #[farhelm_testtrace::test]
    async fn an_unusable_created_session_id_is_outcome_unknown_for_every_shape() {
        let oversized = "x".repeat(crate::session_cache::MAX_SESSION_ID_BYTES + 1);
        for (shape, id) in [
            ("an empty id", String::new()),
            ("an id past the ingress cap", oversized),
            (
                "an id carrying a control character",
                "sess\nforged".to_string(),
            ),
            ("a dot segment", "..".to_string()),
        ] {
            let (client_side, peer) = tokio::io::duplex(64 * 1024);
            spawn_created_id_responder(peer, id.clone());
            let (h, local, _remote) = creating_fleet(client_side, vec![session("asker", 1)]).await;

            let outcome = HelmAgentRequests::for_state(&h.state)
                .handle(
                    origin_of(&h, local),
                    "asker",
                    command_create("user@builder", "/srv/work", "agent", None, None),
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
    #[farhelm_testtrace::test]
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
    #[farhelm_testtrace::test]
    fn the_unknown_host_refusal_names_hosts_that_can_never_be_targeted() {
        assert_eq!(unnameable_hosts(&["this machine", "user@builder"]), None);
        let warned = unnameable_hosts(&["ok", "bad\nname"]).expect("one host is unnameable");
        assert!(
            warned.contains('1') && warned.contains("control characters"),
            "the sentence counts them and says why: {warned}"
        );
    }

    /// Both existing identities are forbidden clone results, while an
    /// unrelated child is accepted.
    ///
    /// Source and caller can live on different hosts and must remain separate
    /// all the way to the replay fence. Testing distinct IDs prevents a check
    /// of only one from accidentally satisfying both cases.
    #[farhelm_testtrace::test]
    fn clone_replay_refuses_both_source_and_caller_ids() {
        for replayed in ["caller", "source"] {
            let row = session(replayed, 1);
            let error = reject_clone_replay("caller", "source", &row)
                .expect_err("an existing participant is not a new child");
            assert_eq!(crate::error_kind(&error), ErrorKind::Conflict);
        }
        reject_clone_replay("caller", "source", &session("child", 1))
            .expect("an unrelated child is a valid clone result");
    }

    /// Spec: a clone whose target replays the ASKING session — because the
    /// idempotency key was the one that created it — is refused as a
    /// `Conflict`, not reported as a new session.
    ///
    /// Reachable by ordinary means, which is why it is guarded at all. A
    /// same-host clone with no overrides rebuilds precisely the fingerprint
    /// that created the asking session: same directory, same title, same
    /// command, no parent. An agent that reuses the key its own create
    /// used therefore triggers a legitimate reservation replay at the
    /// target, which answers with the ORIGINAL session. Reporting that as
    /// `Created` would hand the caller its own id as a new one — with
    /// `current: true`, no less — and the caller's next move is to act on
    /// what it believes is a copy.
    ///
    /// The second half of the test is about WHEN the refusal happens, and it
    /// is the half with durable consequences. Refusing after
    /// `do_create_session`'s bookkeeping still refuses, but by then the
    /// replayed row has been seeded into the cache and its revision published
    /// — effects of a create the agent is simultaneously being told did not
    /// occur. The replayed payload carries a title the cache has never seen,
    /// which is what makes "the row was not seeded" observable at all.
    #[farhelm_testtrace::test]
    async fn a_clone_that_replays_the_asking_session_is_refused() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let source = SessionInfo {
            cwd: "/srv/project".to_string(),
            canonical_cwd: None,
            title: "the original".to_string(),
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
        trust_without_asking(&h).await;
        let handler = HelmAgentRequests::for_state(&h.state);
        let listing_before = agent_sessions(&handler, origin_of(&h, local), "asker").await;
        let revision_before = h.manager.events().revision();

        let outcome = handler
            .handle(
                origin_of(&h, local),
                "asker",
                AgentVerb::Clone {
                    source_session_id: Some("asker".to_string()),
                    host: Some("this machine".to_string()),
                    cwd: None,
                    title: None,
                    intent_key: Some("the-key-that-made-me".to_string()),
                    confirm_yolo: false,
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
            h.manager.events().revision(),
            revision_before,
            "a clone that made nothing must not wake every client with a fleet revision for it"
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

    /// Spec: `create --command` succeeds through the shared agent path,
    /// sending the command launch exactly as the agent wrote it.
    ///
    /// The other create tests here reach the target only in refusal or
    /// host-resolution cases. A regression that dropped raw routing — or
    /// seeded the cache wrongly for it, or projected the reply from the
    /// wrong side — would leave all of them green.
    #[farhelm_testtrace::test]
    async fn create_from_a_raw_invocation_reaches_the_target_whole() {
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let seen = spawn_create_responder(peer, None);
        let (h, local, _remote) = creating_fleet(client_side, vec![session("asker", 1)]).await;

        let outcome = HelmAgentRequests::for_state(&h.state)
            .handle(
                origin_of(&h, local),
                "asker",
                command_create(
                    "user@builder",
                    "/srv/raw",
                    "sh -c 'sleep 1'",
                    Some("raw one"),
                    Some("raw-key"),
                ),
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
        assert_eq!(
            seen[0].launch,
            Some(farhelm_proto::SessionLaunch::plain_command(
                "sh -c 'sleep 1'"
            )),
            "the command launch reaches the target whole, with nothing added"
        );
        assert_eq!(
            seen[0].intent_key,
            asker_scoped_intent_key("asker", Some("raw-key".to_string())),
            "the target stores the key scoped to the asking session"
        );
    }

    /// Spec: `farhelm agent clone` of a legacy source (created before
    /// launch kinds) is refused with a remedy and dispatches nothing.
    ///
    /// Why: a legacy launch has no YOLO answer and was never classified, so
    /// an agent copying it would start an unclassified launch on its own
    /// authority; the remedy names what a person can do instead.
    #[farhelm_testtrace::test]
    async fn clone_of_a_legacy_source_is_refused_with_a_remedy() {
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let seen = spawn_create_responder(peer, None);
        let source = SessionInfo {
            invocation: "claude --model opus".to_string(),
            launch: farhelm_proto::SessionLaunch::Legacy {
                invocation: "claude --model opus".to_string(),
                agent_kind: farhelm_proto::AgentKind::Claude,
                resume_template: None,
            },
            ..session("source", 1)
        };
        let (h, local, _remote) = creating_fleet(client_side, vec![source]).await;
        let outcome = HelmAgentRequests::for_state(&h.state)
            .handle(
                origin_of(&h, local),
                "asker",
                AgentVerb::Clone {
                    source_session_id: Some("source".to_string()),
                    host: Some("user@builder".to_string()),
                    cwd: None,
                    title: None,
                    intent_key: Some("clone-key".to_string()),
                    confirm_yolo: false,
                },
            )
            .await;
        match outcome {
            AgentOutcome::Err { kind, message } => {
                assert_eq!(kind, ErrorKind::InvalidRequest);
                assert!(message.contains("before launch kinds"), "{message}");
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
        assert!(
            seen.lock().expect("seen mutex").is_empty(),
            "nothing may be dispatched for a refused clone"
        );
    }

    /// Spec: a create that names neither an agent type nor a command, and
    /// applies no template that does, is refused naming `--agent` before
    /// any target is contacted.
    ///
    /// Why: this pins the authoritative helm boundary independently of
    /// clap. An agent's create never borrows the GUI's remembered agent
    /// type (SPEC.md), so the helm must not choose an agent on its behalf.
    #[farhelm_testtrace::test]
    async fn create_with_no_launch_is_refused() {
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let seen = spawn_create_responder(peer, None);
        let (h, local, _remote) = creating_fleet(client_side, vec![session("asker", 1)]).await;

        let outcome = HelmAgentRequests::for_state(&h.state)
            .handle(
                origin_of(&h, local),
                "asker",
                AgentVerb::Create {
                    host: Some("user@builder".to_string()),
                    templates: Vec::new(),
                    edits: farhelm_proto::launcher::TemplateFields {
                        destination: Some(farhelm_proto::launcher::TemplateDestination::Folder(
                            "/srv/work".to_string(),
                        )),
                        ..Default::default()
                    },
                    intent_key: None,
                    confirm_yolo: false,
                    spawn: None,
                },
            )
            .await;

        match outcome {
            AgentOutcome::Err { kind, message } => {
                assert_eq!(kind, ErrorKind::InvalidRequest);
                assert!(
                    message.contains("--agent is required"),
                    "the refusal must name what is missing: {message}"
                );
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
        assert!(
            seen.lock().expect("seen mutex").is_empty(),
            "an omitted-command request is refused before anything is sent to the host"
        );
    }

    // -----------------------------------------------------------------
    // The approval gate (SPEC.md, Agent-spawned sessions)
    // -----------------------------------------------------------------

    /// Answer the one waiting card over the real REST route.
    async fn answer_card(h: &Harness, answer: &str) -> axum::http::StatusCode {
        use tower::ServiceExt;
        let id = h
            .state
            .approvals
            .list()
            .into_iter()
            .next()
            .expect("premise: a card is waiting")
            .id;
        h.router()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri(format!("/api/approvals/{id}"))
                    .header("host", "127.0.0.1:7433")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::json!({ "answer": answer }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap()
            .status()
    }

    /// Wait (bounded) for a card to be listed while `task` is still waiting,
    /// failing with the task's outcome if it ended without one.
    async fn await_card(
        h: &Harness,
        task: &tokio::task::JoinHandle<AgentOutcome>,
    ) -> farhelm_proto::approvals::PendingApproval {
        let mut changes = h.state.manager.events().subscribe();
        let card = tokio::time::timeout(std::time::Duration::from_secs(20), async {
            loop {
                if let Some(card) = h.state.approvals.list().into_iter().next() {
                    return Some(card);
                }
                if task.is_finished() {
                    return None;
                }
                // The feed wakes this when a card appears; the tick only notices
                // a request that ended without one, which bumps nothing.
                tokio::select! {
                    _ = changes.changed() => {}
                    // sleep-ok: polling interval for the task's own ending, beside the feed wake-up.
                    () = tokio::time::sleep(std::time::Duration::from_millis(50)) => {}
                }
            }
        })
        .await
        .expect("a card or an ending within the bound");
        card.unwrap_or_else(|| panic!("the request ended without a card"))
    }

    /// Spec: with the requesting host's "run farhelm commands without asking"
    /// setting off and a GUI connected, an agent's create waits for a card
    /// showing the target host, folder and whole launch. Denying it refuses
    /// the request as declined and sends nothing to the target; allowing the
    /// next one creates the session.
    ///
    /// Why: this is the guarantee the permission prompts exist for. A create
    /// reaching the target before the user's answer, or after a denial, is an
    /// agent acting on the fleet without the user (SPEC.md, Local authority).
    #[farhelm_testtrace::test]
    async fn an_agent_create_waits_for_approval_and_a_denial_sends_nothing() {
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let seen = spawn_create_responder(peer, None);
        let (h, local, _remote) = creating_fleet(client_side, vec![session("asker", 1)]).await;
        h.store
            .set_commands_without_asking(local, false)
            .await
            .unwrap();
        let _gui = h.state.manager.events().admit(1).expect("a GUI seat");
        let handler = HelmAgentRequests::for_state(&h.state);
        let origin = origin_of(&h, local);
        let create = || command_create("user@builder", "/srv/w", "echo hi", Some("t"), None);

        let asking = Arc::clone(&handler);
        let task = tokio::spawn(async move { asking.handle(origin, "asker", create()).await });
        let card = await_card(&h, &task).await;
        match &card.action {
            ApprovalAction::Launch {
                verb,
                host_name,
                cwd,
                launch,
                ..
            } => {
                assert_eq!(*verb, LaunchVerb::Create);
                assert_eq!(host_name, "user@builder");
                assert_eq!(cwd, "/srv/w");
                assert_eq!(launch.display_command(), "echo hi");
            }
            other => panic!("a launch card, got {other:?}"),
        }
        assert_eq!(card.session.id, "asker");
        assert!(
            seen.lock().unwrap().is_empty(),
            "nothing is sent while it waits"
        );
        assert_eq!(
            answer_card(&h, "deny").await,
            axum::http::StatusCode::NO_CONTENT
        );
        match task.await.unwrap() {
            AgentOutcome::Err { kind, message } => {
                assert_eq!(kind, ErrorKind::Unauthorized);
                assert!(message.contains("declined"), "{message}");
            }
            other => panic!("declined, got {other:?}"),
        }
        assert!(seen.lock().unwrap().is_empty(), "a denial sends nothing");

        let asking = Arc::clone(&handler);
        let task = tokio::spawn(async move { asking.handle(origin, "asker", create()).await });
        await_card(&h, &task).await;
        assert_eq!(
            answer_card(&h, "allow").await,
            axum::http::StatusCode::NO_CONTENT
        );
        let outcome = task.await.unwrap();
        assert!(
            matches!(
                outcome,
                AgentOutcome::Ok {
                    reply: AgentReply::Created { .. }
                }
            ),
            "{outcome:?}"
        );
        assert_eq!(seen.lock().unwrap().len(), 1);
    }

    /// Spec: on a host that asks before YOLO launches, an agent's command
    /// launch is refused at once whatever its YOLO assertion says, and so is
    /// a YOLO agent launch, with no card and nothing sent, even when the
    /// request carries the old `confirm_yolo` override; an agent launch that
    /// is not YOLO gets the ordinary card.
    ///
    /// Why: Farhelm cannot check a command's assertion, and SPEC.md lets an
    /// agent start on such a host only what runs the way the user approved.
    /// The override was the agent's own word, which the rule replaces.
    #[farhelm_testtrace::test]
    async fn the_agent_yolo_rule_refuses_without_a_card() {
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let seen = spawn_create_responder(peer, None);
        let (h, local, remote) = creating_fleet(client_side, vec![session("asker", 1)]).await;
        h.store
            .set_commands_without_asking(local, false)
            .await
            .unwrap();
        h.store
            .set_yolo_without_asking(remote, false)
            .await
            .unwrap();
        let _gui = h.state.manager.events().admit(1).expect("a GUI seat");
        let handler = HelmAgentRequests::for_state(&h.state);
        let origin = origin_of(&h, local);

        let agent_launch = |permissions| AgentVerb::Create {
            host: Some("user@builder".to_string()),
            templates: Vec::new(),
            edits: farhelm_proto::launcher::TemplateFields {
                kind: Some(farhelm_proto::launcher::LauncherKind::Agent),
                agent: Some(farhelm_proto::LaunchHarness::Claude),
                permissions: Some(permissions),
                destination: Some(farhelm_proto::launcher::TemplateDestination::Folder(
                    "/srv/w".to_string(),
                )),
                ..Default::default()
            },
            intent_key: None,
            confirm_yolo: true,
            spawn: None,
        };
        let mut overridden = command_create("user@builder", "/srv/w", "sh", None, None);
        if let AgentVerb::Create { confirm_yolo, .. } = &mut overridden {
            *confirm_yolo = true;
        }
        for refused in [
            command_create("user@builder", "/srv/w", "sh", None, None),
            overridden,
            agent_launch(Some(farhelm_proto::LaunchPermission::Yolo)),
        ] {
            match handler.handle(origin, "asker", refused).await {
                AgentOutcome::Err { kind, message } => {
                    assert_eq!(kind, ErrorKind::Unauthorized);
                    assert!(message.contains("may not start a YOLO launch"), "{message}");
                    assert!(message.contains("user@builder"), "{message}");
                }
                other => panic!("refused, got {other:?}"),
            }
            assert!(
                h.state.approvals.list().is_empty(),
                "no card for a refused launch"
            );
        }
        assert!(seen.lock().unwrap().is_empty());

        let asking = Arc::clone(&handler);
        let task =
            tokio::spawn(async move { asking.handle(origin, "asker", agent_launch(None)).await });
        let card = await_card(&h, &task).await;
        assert!(matches!(card.action, ApprovalAction::Launch { .. }));
        task.abort();
    }

    /// Spec: a lifecycle verb on a session no host knows is refused as not
    /// found before any card is shown, and a stop of a known session waits
    /// for a card naming its target, after which a denial refuses it.
    ///
    /// Why: a card for a session that does not exist asks the user a question
    /// with no right answer; and the stop, like every acting verb, must wait
    /// for the user (SPEC.md).
    #[farhelm_testtrace::test]
    async fn a_stop_waits_for_approval_and_an_unknown_target_gets_no_card() {
        let (h, local, _remote) = two_host_fleet().await;
        h.store
            .set_commands_without_asking(local, false)
            .await
            .unwrap();
        let _gui = h.state.manager.events().admit(1).expect("a GUI seat");
        let handler = HelmAgentRequests::for_state(&h.state);
        let origin = origin_of(&h, local);
        match handler
            .handle(
                origin,
                "local-live",
                AgentVerb::Stop {
                    session_id: Some("ghost".to_string()),
                },
            )
            .await
        {
            AgentOutcome::Err { kind, .. } => assert_eq!(kind, ErrorKind::NotFound),
            other => panic!("not found, got {other:?}"),
        }
        assert!(h.state.approvals.list().is_empty());

        let asking = Arc::clone(&handler);
        let task = tokio::spawn(async move {
            asking
                .handle(
                    origin,
                    "local-live",
                    AgentVerb::Stop {
                        session_id: Some("local-old".to_string()),
                    },
                )
                .await
        });
        let card = await_card(&h, &task).await;
        match card.action {
            ApprovalAction::Stop { target } => {
                assert_eq!(target.id, "local-old");
                assert!(target.title.is_some(), "the cache knows the target");
            }
            other => panic!("a stop card, got {other:?}"),
        }
        assert_eq!(
            answer_card(&h, "deny").await,
            axum::http::StatusCode::NO_CONTENT
        );
        assert!(matches!(
            task.await.unwrap(),
            AgentOutcome::Err {
                kind: ErrorKind::Unauthorized,
                ..
            }
        ));
    }

    /// Spec: `farhelm spawn --inherit-agent`, as its supervisor relays it with
    /// the session's stored launch filled in, creates that launch on the
    /// asking session's own host, in the folder and with the title the CLI
    /// gave; one that also names a template is refused.
    ///
    /// Why: inheritance now goes through the helm so the user is asked, and
    /// SPEC.md keeps `--inherit-agent` exclusive with every launch flag and
    /// on the asking host only.
    #[farhelm_testtrace::test]
    async fn an_inherited_spawn_creates_the_supplied_launch_on_its_own_host() {
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let seen = spawn_create_responder(peer, None);
        let (h, _local, remote) = creating_fleet(client_side, vec![session("asker", 1)]).await;
        let handler = HelmAgentRequests::for_state(&h.state);
        let inherited = farhelm_proto::SessionLaunch::plain_command("my-agent --flag");
        let spawn = |templates: Vec<String>| AgentVerb::Create {
            host: None,
            templates,
            edits: farhelm_proto::launcher::TemplateFields {
                destination: Some(farhelm_proto::launcher::TemplateDestination::Folder(
                    "/srv/child".to_string(),
                )),
                name: Some("child".to_string()),
                ..Default::default()
            },
            intent_key: None,
            confirm_yolo: false,
            spawn: Some(farhelm_proto::SpawnPlacement {
                parent: Some("asker".to_string()),
                inherit_agent: true,
                inherited_launch: Some(Box::new(inherited.clone())),
            }),
        };
        let outcome = handler
            .handle(origin_of(&h, remote), "asker", spawn(Vec::new()))
            .await;
        assert!(
            matches!(
                outcome,
                AgentOutcome::Ok {
                    reply: AgentReply::Created { .. }
                }
            ),
            "{outcome:?}"
        );
        {
            let seen = seen.lock().unwrap();
            assert_eq!(seen.len(), 1);
            assert_eq!(seen[0].launch.as_ref(), Some(&inherited));
            assert_eq!(seen[0].cwd, "/srv/child");
            assert_eq!(seen[0].title.as_deref(), Some("child"));
            assert_eq!(seen[0].parent.as_deref(), Some("asker"));
        }
        match handler
            .handle(origin_of(&h, remote), "asker", spawn(vec!["t".to_string()]))
            .await
        {
            AgentOutcome::Err { kind, message } => {
                assert_eq!(kind, ErrorKind::InvalidRequest);
                assert!(message.contains("exclusive"), "{message}");
            }
            other => panic!("refused, got {other:?}"),
        }
        assert_eq!(seen.lock().unwrap().len(), 1);
    }

    /// Spec: the agent YOLO rule is applied again when the user approves: a
    /// command launch carded while its target allowed YOLO launches is refused,
    /// and nothing is sent, if the target was set to ask before the answer.
    ///
    /// Why: SPEC.md has the helm judge the setting as it stands when the user
    /// approves; a launch carried out on the strength of a setting the user
    /// has since turned off would start what the host now says not to.
    #[farhelm_testtrace::test]
    async fn the_yolo_rule_is_applied_again_after_approval() {
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let seen = spawn_create_responder(peer, None);
        let (h, local, remote) = creating_fleet(client_side, vec![session("asker", 1)]).await;
        h.store
            .set_commands_without_asking(local, false)
            .await
            .unwrap();
        let _gui = h.state.manager.events().admit(1).expect("a GUI seat");
        let handler = HelmAgentRequests::for_state(&h.state);
        let origin = origin_of(&h, local);
        let asking = Arc::clone(&handler);
        let task = tokio::spawn(async move {
            asking
                .handle(
                    origin,
                    "asker",
                    command_create("user@builder", "/srv/w", "sh", None, None),
                )
                .await
        });
        await_card(&h, &task).await;
        h.store
            .set_yolo_without_asking(remote, false)
            .await
            .unwrap();
        assert_eq!(
            answer_card(&h, "allow").await,
            axum::http::StatusCode::NO_CONTENT
        );
        match task.await.unwrap() {
            AgentOutcome::Err { kind, message } => {
                assert_eq!(kind, ErrorKind::Unauthorized);
                assert!(message.contains("may not start a YOLO launch"), "{message}");
            }
            other => panic!("refused, got {other:?}"),
        }
        assert!(seen.lock().unwrap().is_empty(), "nothing is sent");
    }

    /// Spec: an inheriting spawn is carded as one, showing the inherited
    /// launch; on a host that asks before YOLO launches, an inheriting spawn
    /// whose launch is a command launch is refused without a card.
    ///
    /// Why: `--inherit-agent` used to be answered by the session's own
    /// supervisor with nobody asked; it now goes through the same gate and
    /// the same agent YOLO rule as every other spawn (SPEC.md).
    #[farhelm_testtrace::test]
    async fn an_inherited_spawn_is_carded_and_held_to_the_yolo_rule() {
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let _seen = spawn_create_responder(peer, None);
        let (h, _local, remote) = creating_fleet(client_side, vec![session("asker", 1)]).await;
        h.store
            .set_commands_without_asking(remote, false)
            .await
            .unwrap();
        let _gui = h.state.manager.events().admit(1).expect("a GUI seat");
        let handler = HelmAgentRequests::for_state(&h.state);
        let origin = origin_of(&h, remote);
        let spawn = || AgentVerb::Create {
            host: None,
            templates: Vec::new(),
            edits: farhelm_proto::launcher::TemplateFields {
                destination: Some(farhelm_proto::launcher::TemplateDestination::Folder(
                    "/srv/child".to_string(),
                )),
                ..Default::default()
            },
            intent_key: None,
            confirm_yolo: false,
            spawn: Some(farhelm_proto::SpawnPlacement {
                parent: None,
                inherit_agent: true,
                inherited_launch: Some(Box::new(farhelm_proto::SessionLaunch::plain_command(
                    "inherited-agent",
                ))),
            }),
        };
        let asking = Arc::clone(&handler);
        let task = tokio::spawn(async move { asking.handle(origin, "asker", spawn()).await });
        let card = await_card(&h, &task).await;
        match card.action {
            ApprovalAction::Launch { verb, launch, .. } => {
                assert_eq!(verb, LaunchVerb::SpawnInherited);
                assert_eq!(launch.display_command(), "inherited-agent");
            }
            other => panic!("a launch card, got {other:?}"),
        }
        assert_eq!(
            answer_card(&h, "deny").await,
            axum::http::StatusCode::NO_CONTENT
        );
        task.await.unwrap();

        h.store
            .set_yolo_without_asking(remote, false)
            .await
            .unwrap();
        match handler.handle(origin, "asker", spawn()).await {
            AgentOutcome::Err { kind, .. } => assert_eq!(kind, ErrorKind::Unauthorized),
            other => panic!("refused, got {other:?}"),
        }
        assert!(
            h.state.approvals.list().is_empty(),
            "no card for a refused launch"
        );
    }

    // -----------------------------------------------------------------
    // Template writes (SPEC.md, Agent-spawned sessions)
    // -----------------------------------------------------------------

    fn template_fields(command: &str) -> farhelm_proto::launcher::TemplateFields {
        farhelm_proto::launcher::TemplateFields {
            kind: Some(farhelm_proto::launcher::LauncherKind::Command),
            command: Some(command.to_string()),
            yolo: Some(false),
            ..Default::default()
        }
    }

    /// An agent type alone must keep its meaning when applied from the command
    /// tab; beside command fields it instead declares the command's agent.
    /// A command choice without an agent also pins the command tab, so a
    /// YOLO- or resume-only template can switch away from the agent tab.
    /// Explicit kinds stay authoritative; common placement edits remain
    /// usable under either launch kind.
    #[test]
    fn template_create_infers_kind_without_reinterpreting_command_agents() {
        use farhelm_proto::launcher::{LauncherKind, TemplateFields};
        let agent = TemplateFields {
            agent: Some(farhelm_proto::LaunchHarness::Claude),
            ..Default::default()
        };
        assert_eq!(
            with_launch_kind(agent.clone()).kind,
            Some(LauncherKind::Agent)
        );
        let explicit = TemplateFields {
            kind: Some(LauncherKind::Command),
            ..agent.clone()
        };
        assert_eq!(with_launch_kind(explicit.clone()), explicit);
        for fields in [
            TemplateFields {
                command: Some("claude {farhelm_args}".into()),
                ..agent.clone()
            },
            TemplateFields {
                yolo: Some(false),
                ..agent.clone()
            },
            TemplateFields {
                resume_command: Some(None),
                ..agent
            },
            TemplateFields {
                yolo: Some(false),
                ..Default::default()
            },
            TemplateFields {
                resume_command: Some(Some("resume {conversation}".into())),
                ..Default::default()
            },
        ] {
            let inferred = with_launch_kind(fields.clone());
            assert_eq!(inferred.kind, Some(LauncherKind::Command));
            assert_eq!(
                inferred,
                TemplateFields {
                    kind: Some(LauncherKind::Command),
                    ..fields
                }
            );
        }
        let placement = TemplateFields {
            name: Some("review".into()),
            ..Default::default()
        };
        assert_eq!(with_launch_kind(placement.clone()), placement);
    }

    /// Spec: `template create` stores the template with its host written as
    /// that host's install identity and refuses a taken name; `template edit`
    /// sets only the fields it is given, keeping the command text the agent
    /// never saw; `template delete` removes it; a template naming its host
    /// directly, or a fresh-checkout destination, is refused. An agent-type-only
    /// create pins the agent kind, so a later command edit is refused rather
    /// than reinterpreting the template as a different launch.
    ///
    /// Why: these are SPEC.md's template verbs for agents. An edit that
    /// replaced the whole template would silently drop command text the
    /// listing withholds, and a template whose host field an agent could set
    /// itself could pin launches to an install the helm never resolved.
    #[farhelm_testtrace::test]
    async fn template_writes_create_merge_and_delete() {
        let (h, local, _remote) = two_host_fleet().await;
        let handler = HelmAgentRequests::for_state(&h.state);
        let origin = origin_of(&h, local);
        let ask = |verb| {
            let handler = Arc::clone(&handler);
            async move { handler.handle(origin, "local-live", verb).await }
        };
        let created = ask(AgentVerb::TemplateCreate {
            name: "t".to_string(),
            fields: template_fields("secret-tool --go"),
            host: Some("this machine".to_string()),
        })
        .await;
        match created {
            AgentOutcome::Ok {
                reply: AgentReply::TemplateWritten { template },
            } => {
                assert!(template.sets_command);
                assert_eq!(
                    template.fields.command, None,
                    "the reply withholds command text"
                );
                assert_eq!(template.host_name.as_deref(), Some("this machine"));
            }
            other => panic!("created, got {other:?}"),
        }
        let stored = h.store.launch_templates().await.unwrap();
        assert_eq!(stored[0].fields.host.as_deref(), Some("identity-local"));

        let taken = ask(AgentVerb::TemplateCreate {
            name: "t".to_string(),
            fields: template_fields("other"),
            host: None,
        })
        .await;
        assert!(
            matches!(
                &taken,
                AgentOutcome::Err {
                    kind: ErrorKind::Conflict,
                    ..
                }
            ),
            "{taken:?}"
        );

        let edited = ask(AgentVerb::TemplateEdit {
            name: "t".to_string(),
            fields: farhelm_proto::launcher::TemplateFields {
                name: Some("titled".to_string()),
                ..Default::default()
            },
            host: None,
        })
        .await;
        assert!(matches!(edited, AgentOutcome::Ok { .. }), "{edited:?}");
        let stored = h.store.launch_templates().await.unwrap();
        assert_eq!(
            stored[0].fields.command.as_deref(),
            Some("secret-tool --go")
        );
        assert_eq!(stored[0].fields.name.as_deref(), Some("titled"));
        assert_eq!(stored[0].fields.host.as_deref(), Some("identity-local"));

        // Use the real create path: its newly inferred kind must also make
        // the existing edit refusal apply to an agent-type-only template.
        let agent_created = ask(AgentVerb::TemplateCreate {
            name: "agent-kind".into(),
            fields: farhelm_proto::launcher::TemplateFields {
                agent: Some(farhelm_proto::LaunchHarness::Claude),
                ..Default::default()
            },
            host: None,
        })
        .await;
        assert!(
            matches!(agent_created, AgentOutcome::Ok { .. }),
            "{agent_created:?}"
        );
        let agent_stored = h
            .store
            .launch_templates()
            .await
            .unwrap()
            .into_iter()
            .find(|template| template.name == "agent-kind")
            .expect("the successful create stored its template");
        assert_eq!(
            agent_stored.fields.kind,
            Some(farhelm_proto::launcher::LauncherKind::Agent)
        );
        // Each refusal is told apart by its kind and its own wording, so a
        // case that is refused for some other reason fails here.
        let refusals: Vec<(AgentVerb, ErrorKind, &str)> = vec![
            (
                AgentVerb::TemplateEdit {
                    name: "missing".to_string(),
                    fields: template_fields("x"),
                    host: None,
                },
                ErrorKind::NotFound,
                "missing",
            ),
            (
                AgentVerb::TemplateCreate {
                    name: "pinned".to_string(),
                    fields: farhelm_proto::launcher::TemplateFields {
                        host: Some("identity-local".to_string()),
                        ..template_fields("x")
                    },
                    host: None,
                },
                ErrorKind::InvalidRequest,
                "with --host",
            ),
            (
                AgentVerb::TemplateCreate {
                    name: "checkout".to_string(),
                    fields: farhelm_proto::launcher::TemplateFields {
                        destination: Some(farhelm_proto::launcher::TemplateDestination::Github(
                            "o/r".to_string(),
                        )),
                        ..template_fields("x")
                    },
                    host: None,
                },
                ErrorKind::InvalidRequest,
                "managed checkout",
            ),
            (
                AgentVerb::TemplateEdit {
                    name: "t".to_string(),
                    fields: Default::default(),
                    host: None,
                },
                ErrorKind::InvalidRequest,
                "at least one field",
            ),
            (
                AgentVerb::TemplateEdit {
                    name: "t".to_string(),
                    fields: farhelm_proto::launcher::TemplateFields {
                        command: Some("new-command".to_string()),
                        ..Default::default()
                    },
                    host: None,
                },
                ErrorKind::InvalidRequest,
                "--yolo or --no-yolo",
            ),
            (
                AgentVerb::TemplateEdit {
                    name: "t".to_string(),
                    fields: farhelm_proto::launcher::TemplateFields {
                        model: Some(Some("opus".to_string())),
                        ..Default::default()
                    },
                    host: None,
                },
                ErrorKind::InvalidRequest,
                "launch kind",
            ),
            // An explicit kind is never replaced, even when the result
            // would hold nothing contradictory (`agent` is both kinds').
            (
                AgentVerb::TemplateEdit {
                    name: "agent-kind".to_string(),
                    fields: template_fields("claude {farhelm_args}"),
                    host: None,
                },
                ErrorKind::InvalidRequest,
                "launch kind",
            ),
        ];
        for (verb, expected_kind, fragment) in refusals {
            match ask(verb).await {
                AgentOutcome::Err { kind, message } => {
                    assert_eq!(kind, expected_kind, "{message}");
                    assert!(message.contains(fragment), "{fragment:?} in {message}");
                }
                other => panic!("refused, got {other:?}"),
            }
        }
        assert_eq!(h.store.launch_templates().await.unwrap().len(), 2);
        let after_refusal = h
            .store
            .launch_templates()
            .await
            .unwrap()
            .into_iter()
            .find(|template| template.name == "agent-kind")
            .expect("the refused edit preserves the template");
        assert_eq!(
            after_refusal, agent_stored,
            "the refused edit wrote nothing"
        );

        let deleted = ask(AgentVerb::TemplateDelete {
            name: "t".to_string(),
        })
        .await;
        assert!(
            matches!(
                deleted,
                AgentOutcome::Ok {
                    reply: AgentReply::TemplateDeleted {}
                }
            ),
            "{deleted:?}"
        );
        assert_eq!(
            h.store.launch_templates().await.unwrap().len(),
            1,
            "only the agent-kind fixture is left"
        );
    }

    /// Spec: with the host asking, a template edit waits for a card that
    /// shows the whole resulting template, the existing command text
    /// included, and a denial leaves the template as it was.
    ///
    /// Why: a template's command line may later run on any host, and the
    /// agent never saw it (the listing withholds it); the card is the one
    /// place the user can see what the write would leave behind.
    #[farhelm_testtrace::test]
    async fn a_template_write_card_shows_the_whole_template() {
        let (h, local, _remote) = two_host_fleet().await;
        h.store
            .put_launch_template(farhelm_proto::launcher::LaunchTemplate {
                name: "t".to_string(),
                fields: template_fields("hidden-command --flag"),
            })
            .await
            .unwrap();
        h.store
            .set_commands_without_asking(local, false)
            .await
            .unwrap();
        let _gui = h.state.manager.events().admit(1).expect("a GUI seat");
        let handler = HelmAgentRequests::for_state(&h.state);
        let origin = origin_of(&h, local);
        let asking = Arc::clone(&handler);
        let task = tokio::spawn(async move {
            asking
                .handle(
                    origin,
                    "local-live",
                    AgentVerb::TemplateEdit {
                        name: "t".to_string(),
                        fields: farhelm_proto::launcher::TemplateFields {
                            name: Some("new title".to_string()),
                            ..Default::default()
                        },
                        host: None,
                    },
                )
                .await
        });
        let card = await_card(&h, &task).await;
        match card.action {
            ApprovalAction::TemplateWrite {
                name,
                replaces_existing,
                fields,
                ..
            } => {
                assert_eq!(name, "t");
                assert!(replaces_existing);
                assert_eq!(fields.command.as_deref(), Some("hidden-command --flag"));
                assert_eq!(fields.name.as_deref(), Some("new title"));
            }
            other => panic!("a template card, got {other:?}"),
        }
        assert_eq!(
            answer_card(&h, "deny").await,
            axum::http::StatusCode::NO_CONTENT
        );
        match task.await.unwrap() {
            AgentOutcome::Err { message, .. } => {
                assert!(message.contains("declined"), "{message}");
            }
            other => panic!("declined, got {other:?}"),
        }
        assert_eq!(
            h.store.launch_templates().await.unwrap()[0].fields.name,
            None,
            "a denied edit writes nothing"
        );
    }

    /// Spec: an approved template write or delete lands only on the template
    /// its card showed. Two creates of one name waiting at once both get
    /// cards, but only the first allowed is written; the second is refused
    /// as changed. An edit or delete whose template the GUI changed while
    /// the card waited is refused the same way, and the GUI's version stays.
    ///
    /// Why: SPEC.md lets the GUI's own template writes win last, but an
    /// agent's write was approved against a specific card. Without the
    /// condition, the second create would silently replace the first (the
    /// store upserts) and an approved delete could remove a definition the
    /// user never saw on any card. The helm enforces this inside the store
    /// transaction that writes (`HelmStore::put_launch_template_if`), whose
    /// comparison `conditional_template_writes_refuse_any_other_template` in
    /// the store tests pins; this test pins that the agent verbs use it. That
    /// the comparison and the write share one transaction is structural and
    /// not exercised by a forced interleaving here.
    #[farhelm_testtrace::test]
    async fn an_approved_template_write_lands_only_on_what_its_card_showed() {
        use tower::ServiceExt;
        let (h, local, _remote) = two_host_fleet().await;
        h.store
            .set_commands_without_asking(local, false)
            .await
            .unwrap();
        let _gui = h.state.manager.events().admit(1).expect("a GUI seat");
        let handler = HelmAgentRequests::for_state(&h.state);
        let origin = origin_of(&h, local);
        let ask = |verb: AgentVerb| {
            let handler = Arc::clone(&handler);
            tokio::spawn(async move { handler.handle(origin, "local-live", verb).await })
        };
        let changed = |outcome: AgentOutcome| match outcome {
            AgentOutcome::Err {
                kind: ErrorKind::Conflict,
                message,
                ..
            } => assert!(message.contains("changed"), "{message}"),
            other => panic!("refused as changed, got {other:?}"),
        };
        // The GUI's own write: the REST editor, which has no precondition.
        let gui_put = |command: &'static str| {
            let router = h.router();
            async move {
                let status = router
                    .oneshot(
                        axum::http::Request::builder()
                            .method("PUT")
                            .uri("/api/templates/t")
                            .header("host", "127.0.0.1:7433")
                            .header("content-type", "application/json")
                            .body(axum::body::Body::from(
                                serde_json::to_string(&template_fields(command)).unwrap(),
                            ))
                            .unwrap(),
                    )
                    .await
                    .unwrap()
                    .status();
                assert_eq!(status, axum::http::StatusCode::OK);
            }
        };

        // Two creates of one name, both waiting on cards.
        let first = ask(AgentVerb::TemplateCreate {
            name: "t".to_string(),
            fields: template_fields("first"),
            host: None,
        });
        await_card(&h, &first).await;
        let second = ask(AgentVerb::TemplateCreate {
            name: "t".to_string(),
            fields: template_fields("second"),
            host: None,
        });
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(20);
        while h.state.approvals.list().len() < 2 {
            assert!(
                tokio::time::Instant::now() < deadline && !second.is_finished(),
                "premise: both creates wait on cards"
            );
            // sleep-ok: polling interval while the second card is listed.
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        assert_eq!(
            answer_card(&h, "allow").await,
            axum::http::StatusCode::NO_CONTENT
        );
        assert_eq!(
            answer_card(&h, "allow").await,
            axum::http::StatusCode::NO_CONTENT
        );
        let outcomes = [first.await.unwrap(), second.await.unwrap()];
        let written = outcomes
            .iter()
            .filter(|outcome| matches!(outcome, AgentOutcome::Ok { .. }))
            .count();
        assert_eq!(written, 1, "exactly one create lands: {outcomes:?}");
        for outcome in outcomes {
            if !matches!(outcome, AgentOutcome::Ok { .. }) {
                changed(outcome);
            }
        }
        let after_creates = h.store.launch_templates().await.unwrap();
        assert_eq!(after_creates.len(), 1);

        // An edit whose template the GUI rewrites while the card waits.
        let edit = ask(AgentVerb::TemplateEdit {
            name: "t".to_string(),
            fields: farhelm_proto::launcher::TemplateFields {
                name: Some("agent title".to_string()),
                ..Default::default()
            },
            host: None,
        });
        await_card(&h, &edit).await;
        gui_put("gui-edit").await;
        assert_eq!(
            answer_card(&h, "allow").await,
            axum::http::StatusCode::NO_CONTENT
        );
        changed(edit.await.unwrap());
        let stored = h.store.launch_templates().await.unwrap();
        assert_eq!(stored[0].fields, template_fields("gui-edit"));

        // A delete whose template the GUI rewrites while the card waits.
        let delete = ask(AgentVerb::TemplateDelete {
            name: "t".to_string(),
        });
        await_card(&h, &delete).await;
        gui_put("gui-again").await;
        assert_eq!(
            answer_card(&h, "allow").await,
            axum::http::StatusCode::NO_CONTENT
        );
        changed(delete.await.unwrap());
        let stored = h.store.launch_templates().await.unwrap();
        assert_eq!(
            stored[0].fields,
            template_fields("gui-again"),
            "the GUI's version survives an approved delete of the old one"
        );
    }
}
