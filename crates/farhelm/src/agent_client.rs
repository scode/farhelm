//! The in-session client: how `farhelm spawn` and `farhelm agent` reach the
//! supervisor of the session they run inside.
//!
//! Both commands read the same injected environment, dial the same socket,
//! authenticate with the same session credential, send exactly one request,
//! and read exactly one reply. They differ in who answers (a spawn is
//! answered by the supervisor itself, an agent request by the helm through
//! the supervisor's relay) and in what they do with the answer, and nothing
//! else. That shared round trip lives in [`one_shot_request`], so the rules
//! for everything that can go wrong once a request is on the socket — the
//! outcome-unknown warning for a lost reply to a mutation ([`lost_reply`])
//! and the escaping of peer-written error prose — cannot drift apart
//! between the two commands again. They once had: `spawn` printed a
//! supervisor's refusal unescaped and said nothing when the reply to its
//! create was lost, where `agent create` did both.
//!
//! The environment itself is read once, into [`SessionEnv`], by `main`'s
//! dispatch. The in-session hook reads the same three variables for a
//! different purpose and with laxer rules; both readings live on that one
//! type so they cannot disagree about which variables a session carries.

use crate::hook::HookCredential;
use crate::render::safe_error_message;
use anyhow::Context;
use farhelm_proto::io::{FrameReader, FrameWriter, handshake_with_session_auth, parse_control};
use farhelm_proto::{AgentOutcome, AgentReply, ControlMsg, SessionAuth};
use farhelm_supervisor::launch::{
    SESSION_ID_ENV_VAR, SESSION_TOKEN_ENV_VAR, SUPERVISOR_SOCK_ENV_VAR,
};
use std::ffi::OsString;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// The injected session environment
// ---------------------------------------------------------------------------

/// The three values a Farhelm session injects into every process it runs,
/// exactly as read: the session's id, its credential, and the supervisor
/// socket to present them to.
///
/// Held raw (`OsString`, each possibly absent) because the two consumers
/// judge them differently. [`SessionEnv::dial`] is strict: a command that is
/// about to act on a supervisor refuses, naming the variable, when anything
/// is missing or unreadable. The hook's readings ([`SessionEnv::hook_log`],
/// [`SessionEnv::hook_credential`]) are lenient and partial: a hook must
/// never fail the agent it runs under, and it still wants to log whatever
/// it can when only some of the values are present.
///
/// Built only by [`SessionEnv::from_env`] outside tests. Tests construct it
/// directly instead, so none of them reads or changes the environment of
/// the process running them (a test suite started inside a Farhelm session
/// already carries all three variables).
#[derive(Debug, Default, Clone)]
pub(crate) struct SessionEnv {
    pub(crate) session_id: Option<OsString>,
    pub(crate) token: Option<OsString>,
    pub(crate) socket: Option<OsString>,
}

/// A validated [`SessionEnv`]: everything needed to dial and authenticate.
#[derive(Debug, Clone)]
pub(crate) struct SessionDial {
    /// The session this process runs inside, which the credential proves.
    pub(crate) session_id: String,
    token: String,
    socket: PathBuf,
}

impl SessionEnv {
    /// Read the three injected variables from this process's environment.
    /// The one place outside `farhelm helm setup` and the supervisor's own
    /// startup that this binary reads them.
    pub(crate) fn from_env() -> SessionEnv {
        SessionEnv {
            session_id: std::env::var_os(SESSION_ID_ENV_VAR),
            token: std::env::var_os(SESSION_TOKEN_ENV_VAR),
            socket: std::env::var_os(SUPERVISOR_SOCK_ENV_VAR),
        }
    }

    /// Validate the injected contract before the first socket operation.
    ///
    /// A session id with no token is the recognizable upgrade edge: a
    /// session started before spawn support carries no credential and must
    /// be restarted. Every other missing value names the exact variable; no
    /// default supervisor is ever dialed.
    ///
    /// `command` is the user-facing name of the command being run —
    /// `farhelm spawn` or `farhelm agent` — because an error that named the
    /// wrong one sends a user to diagnose a feature they did not invoke.
    pub(crate) fn dial(&self, command: &str) -> anyhow::Result<SessionDial> {
        if self.session_id.is_some() && self.token.is_none() {
            anyhow::bail!(
                "this session predates spawn support (it carries no injected session credential) \
                 and must be restarted before running {command}"
            );
        }
        let socket = self
            .socket
            .clone()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "{SUPERVISOR_SOCK_ENV_VAR} is required; {command} will not guess which \
                     supervisor to dial"
                )
            })?
            .into_string()
            .map_err(|_| anyhow::anyhow!("{SUPERVISOR_SOCK_ENV_VAR} is not valid UTF-8"))?;
        let session_id = self
            .session_id
            .clone()
            .with_context(|| format!("{SESSION_ID_ENV_VAR} is required inside a Farhelm session"))?
            .into_string()
            .map_err(|_| anyhow::anyhow!("{SESSION_ID_ENV_VAR} is not valid UTF-8"))?;
        let token = self
            .token
            .clone()
            .with_context(|| {
                format!("{SESSION_TOKEN_ENV_VAR} is required inside a Farhelm session")
            })?
            .into_string()
            .map_err(|_| anyhow::anyhow!("{SESSION_TOKEN_ENV_VAR} is not valid UTF-8"))?;
        Ok(SessionDial {
            session_id,
            token,
            socket: PathBuf::from(socket),
        })
    }

    /// Where the hook writes its diagnostic log for this session:
    /// `<state_dir>/hook-log/<session>.log`, where the state directory is
    /// the socket's own directory.
    ///
    /// The supervisor mirrors this derivation in `hook_log_path`; change
    /// one and you must change the other. Needs only the id and the socket,
    /// not the token, on purpose: a half-configured environment (id and
    /// socket present, token missing) is exactly the situation whose only
    /// evidence is this file, so it must still get one. A non-UTF-8 value
    /// counts as absent: the supervisor's own ids and paths are UTF-8, so a
    /// value that is not cannot be ours.
    pub(crate) fn hook_log(&self) -> Option<PathBuf> {
        let id = self.session_id.as_ref()?.to_str()?;
        let socket = std::path::Path::new(self.socket.as_ref()?.to_str()?);
        Some(socket.parent()?.join("hook-log").join(format!("{id}.log")))
    }

    /// The credential the hook reports with, or `None` unless all three
    /// values are present and UTF-8.
    ///
    /// Anything short of all three is "no credential": there is no
    /// supervisor to report to, whether the environment is absent entirely
    /// or a session predating spawn support left the token out.
    pub(crate) fn hook_credential(&self) -> Option<HookCredential> {
        Some(HookCredential {
            session_id: self.session_id.as_ref()?.to_str()?.to_string(),
            token: self.token.as_ref()?.to_str()?.to_string(),
            socket: PathBuf::from(self.socket.as_ref()?.to_str()?),
        })
    }
}

// ---------------------------------------------------------------------------
// The shared round trip
// ---------------------------------------------------------------------------

/// The `req_id` of the one request a [`one_shot_request`] connection carries.
pub(crate) const REQUEST_ID: u64 = 1;

/// Connect, authenticate, send `request`, and return the one reply.
///
/// `request` must carry [`REQUEST_ID`]. `what` names the request in errors
/// ("spawn", "agent"); `mutating` says whether it can change anything,
/// which decides the wording of every failure after the write (see
/// [`lost_reply`]).
///
/// Handled here, identically for every caller: failures before the request
/// is sent are plain errors (nothing happened); a closed socket, a read
/// error, or an undecodable frame after it is sent goes through
/// [`lost_reply`]; and a `ControlMsg::Error` the supervisor wrote, whether
/// for this request or for the credential itself (`req_id` 0), is returned
/// as its own escaped, bounded message with no doubt appended, because it
/// is the peer's definitive statement that the request failed. Every other
/// reply comes back for the caller to match; one it cannot use should go
/// through [`unexpected_reply`].
///
/// NO TIMEOUT, deliberately. For an agent request the supervisor bounds the
/// upcall (`AGENT_UPCALL_TIMEOUT`) and is the only party that can tell "no
/// helm is attached" from "a helm has it and is slow"; a deadline here would
/// collapse the two and fire first, hiding the specific error the relay was
/// about to send. A spawn is answered by the local supervisor alone.
pub(crate) async fn one_shot_request(
    dial: &SessionDial,
    request: ControlMsg,
    mutating: bool,
    what: &str,
) -> anyhow::Result<ControlMsg> {
    let stream = tokio::net::UnixStream::connect(&dial.socket)
        .await
        .with_context(|| format!("connecting to supervisor socket {}", dial.socket.display()))?;
    let (read, write) = tokio::io::split(stream);
    let mut reader = FrameReader::new(read);
    let mut writer = FrameWriter::new(write);
    // The hello this sends carries `role: "spawn"` — the one spelling
    // `handshake_with_session_auth` has, shared with the conversation hook,
    // which is not a spawn either. It is deliberately harmless: `role` is
    // diagnostic free text and never an authorization input (see
    // `ControlMsg::Hello::role`); presence of `auth` is what selects
    // restricted admission. Left as-is rather than widened here so all
    // session-authenticated callers keep one handshake.
    handshake_with_session_auth(
        &mut reader,
        &mut writer,
        SessionAuth {
            session_id: dial.session_id.clone(),
            token: dial.token.clone(),
        },
    )
    .await
    .context("performing the authenticated supervisor handshake")?;

    writer
        .write_control(&request)
        .await
        .with_context(|| format!("sending the {what} request"))?;

    // Past the write, every way of not getting a reply is an ending
    // [`lost_reply`] has to classify: the request is already on the socket,
    // and the supervisor may have acted on it (or, for an agent request,
    // forwarded it to a helm) before dying.
    let frame = match reader.read_frame().await {
        Ok(Some(frame)) => frame,
        Ok(None) => {
            return Err(lost_reply(
                "the supervisor closed before answering",
                mutating,
            ));
        }
        Err(error) => {
            return Err(lost_reply(
                &format!("reading the {what} reply failed: {error}"),
                mutating,
            ));
        }
    };
    let reply = match parse_control(&frame) {
        Ok(reply) => reply,
        // A frame arrived and could not be read. That is not the same as no
        // frame arriving, but it licenses exactly the same conclusion: the
        // request went out and nothing came back that says what became of
        // it. Classified rather than returned as a decode error for the
        // reason [`lost_reply`] exists — prose about JSON tells the reader
        // nothing about whether their request landed.
        Err(error) => {
            return Err(lost_reply(
                &format!("the {what} reply could not be decoded: {error}"),
                mutating,
            ));
        }
    };
    match reply {
        // Still possible, and not a protocol violation: the supervisor
        // sends an uncorrelated `Error` when it refuses the CREDENTIAL,
        // before any request has been read (see
        // `io::handshake_with_session_auth`).
        //
        // Deliberately NOT routed through `lost_reply`, unlike every other
        // arm here: this is the peer's own definitive statement that the
        // request failed, in its own words, which is precisely the thing an
        // outcome-unknown ending exists for the absence of. Appending "the
        // outcome is unknown" to a refusal the supervisor authored would
        // manufacture doubt it did not express.
        //
        // Escaped like every other piece of peer prose this process prints:
        // `main`'s default `Result` printer puts the message on stderr with
        // no escaping of its own.
        ControlMsg::Error {
            req_id: 0 | REQUEST_ID,
            message,
            ..
        } => anyhow::bail!(safe_error_message(&message)),
        other => Ok(other),
    }
}

/// The error for a reply this process cannot correlate or interpret: a
/// response carrying somebody else's `req_id`, an unrelated control
/// message, an `Error` for a request that was never made.
///
/// The request itself went out, so a mutation's ending here is
/// outcome-unknown for the same reason a decode failure's is. Names the
/// reply's variant rather than printing it, because a message carries
/// session ids, invocations, and working directories, and a broken peer's
/// reply can be as large as it chose to make it.
pub(crate) fn unexpected_reply(what: &str, reply: &ControlMsg, mutating: bool) -> anyhow::Error {
    lost_reply(
        &format!(
            "the supervisor sent an unexpected {what} reply ({})",
            reply.variant_name()
        ),
        mutating,
    )
}

// ---------------------------------------------------------------------------
// `farhelm spawn`
// ---------------------------------------------------------------------------

/// Parsed spawn inputs after clap has enforced the one required flag.
pub(crate) struct SpawnArgs {
    pub(crate) cwd: PathBuf,
    pub(crate) title: Option<String>,
    pub(crate) agent: Option<String>,
    pub(crate) profile_id: Option<String>,
    pub(crate) inherit_agent: bool,
    pub(crate) parent: Option<String>,
    pub(crate) idempotency_key: Option<String>,
}

/// Create one child under the environment's session authority.
///
/// This is the centralized scripting contract for `farhelm spawn`: validate
/// all three injected environment values before dialing, preserve the cwd's
/// lexical spelling (an ordinary relative input resolves against this
/// process's cwd; a `~`-prefixed input is forwarded verbatim for the
/// supervisor's own expansion — see the branch below for why absolutizing
/// it would be wrong), authenticate the connection, and return the child id
/// for the sole stdout line. A `SessionCreated` reply means creation
/// succeeded regardless of the status snapshot it carries; every refusal
/// and protocol mismatch is an error and therefore produces no id.
///
/// A create is a mutation, so a reply lost after the request went out
/// carries the outcome-unknown warning: the child may already be running.
pub(crate) async fn spawn_session(env: &SessionEnv, args: SpawnArgs) -> anyhow::Result<String> {
    let dial = env.dial("farhelm spawn")?;
    // `~`-prefixed paths are forwarded verbatim: the SUPERVISOR owns that
    // contract (SPEC.md — `~` expands against its own home, `~user` is its
    // refusal to give), and a spawn always targets the same host it runs
    // on, so nothing is gained by resolving locally. Absolutizing them
    // here would instead manufacture `<cwd>/~...` — a path that at best
    // fails as nonexistent and at worst names a real directory literally
    // called `~user`, silently dodging the supervisor's refusal. Ordinary
    // relative paths keep resolving against this process's cwd, which is
    // the spelling a shell user means.
    let cwd = if args.cwd.is_absolute() || args.cwd.to_str().is_some_and(|c| c.starts_with('~')) {
        args.cwd
    } else {
        std::env::current_dir()
            .context("reading farhelm spawn's current directory")?
            .join(args.cwd)
    };
    let cwd = cwd
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("spawn working directory is not valid UTF-8"))?
        .to_string();

    let request = ControlMsg::CreateSession {
        req_id: REQUEST_ID,
        parent: args.parent,
        cwd,
        invocation: None,
        profile_name: args.agent,
        profile_id: args.profile_id,
        inherit_agent: args.inherit_agent,
        title: args.title,
        cols: 80,
        rows: 24,
        intent_key: args.idempotency_key,
        agent_kind: None,
        resume_template: None,
        source_profile: None,
        // Explicit inheritance has no structured selector. The
        // supervisor copies the authenticated parent's stored launch
        // bundle, which is the only safe source of that provenance.
        launch: None,
        // Fresh-checkout payloads are helm-supplied only; a restricted
        // spawn never carries one (and the supervisor refuses it).
        github_checkout: None,
    };
    let reply = one_shot_request(&dial, request, true, "spawn").await?;
    match reply {
        ControlMsg::SessionCreated {
            req_id: REQUEST_ID,
            session,
        } => Ok(session.id),
        other => Err(unexpected_reply("spawn", &other, true)),
    }
}

// ---------------------------------------------------------------------------
// `farhelm agent`
// ---------------------------------------------------------------------------

/// Ask the helm one question, or tell it to act, from inside a session, and
/// return the asking session's own id alongside the answer.
///
/// Mechanically `spawn_session`'s twin — same injected environment
/// (validated by the same [`SessionEnv::dial`], so the two commands can
/// never disagree about what a Farhelm session guarantees), same
/// [`one_shot_request`] round trip — and semantically its
/// opposite. A spawn is answered by the supervisor on the other end of the
/// socket. This is answered by the HELM, which is not on the other end of
/// anything this process can reach: the session's host has no route,
/// address, or credential back to the machine the user is sitting at, so
/// the supervisor forwards the question up the connection the helm itself
/// opened and relays the answer back. See `farhelm-supervisor`'s
/// `service::agent_relay`.
///
/// The asking session's id travels back out because it is the one thing a
/// caller cannot otherwise recover after this call: `Stop`'s reply
/// ([`farhelm_proto::AgentReply::Stopped`]) carries no fields at all, so a
/// confirmation naming WHICH session stopped — when the caller sent no
/// `--session` and meant "this one" — has nowhere else to read that id
/// from. `Rename` does not need it; its reply is the
/// updated row.
///
/// NO TIMEOUT here, deliberately. The supervisor bounds the upcall
/// (`AGENT_UPCALL_TIMEOUT`) and is the only party that can tell "no helm is
/// attached" from "a helm has it and is slow"; a deadline on this side
/// would collapse those into one unactionable failure and would fire first,
/// hiding the specific error the relay was about to send.
pub(crate) async fn agent_request(
    env: &SessionEnv,
    request: farhelm_proto::AgentVerb,
) -> anyhow::Result<(String, AgentReply)> {
    // Captured before the request goes out, because the reply's own tag is
    // the only thing that can be checked against it — see [`ReplyKind`].
    let expected = ReplyKind::of_verb(&request);
    // Also captured before `request` is moved into the frame below, for the
    // other question this function can no longer answer afterwards: whether
    // the thing that went out CHANGES something. See [`lost_reply`].
    let mutating = request.is_mutating();
    let dial = env.dial("farhelm agent")?;
    let session_id = dial.session_id.clone();
    let reply = one_shot_request(
        &dial,
        ControlMsg::AgentRequest {
            req_id: REQUEST_ID,
            session_id: session_id.clone(),
            request,
        },
        mutating,
        "agent",
    )
    .await?;
    match reply {
        ControlMsg::AgentResponse {
            req_id: REQUEST_ID,
            outcome,
        } => match outcome {
            AgentOutcome::Ok { reply } => {
                // The tag exists precisely so this can be checked. A
                // response is handed back by `req_id` alone across two
                // hops, so a peer that correlated a sessions listing with a
                // hosts request would otherwise have that listing printed
                // under `farhelm agent hosts` — authoritative-looking output
                // answering a question nobody asked.
                let got = ReplyKind::of(&reply);
                if got != expected {
                    // Verb- and shape-neutral wording. Two of the five
                    // requests this can report on are not questions and
                    // three of the four replies are not listings, so the
                    // old "answered the X question with a Y listing" was
                    // wrong for most of the pairs it could actually print
                    // — a rename answered with a stop confirmation being
                    // the plainest case.
                    //
                    // Routed through `lost_reply` rather than bailed
                    // outright: a peer that answered a `stop` with a
                    // session row is broken, but a broken peer is at least
                    // as likely to have stopped the session and then
                    // mis-answered as to have done nothing, and this
                    // process cannot tell those apart. The remedy belongs
                    // to every post-write ending of a mutation, not only
                    // the tidy ones.
                    return Err(lost_reply(
                        &format!(
                            "the helm answered with a {} where a {} was expected",
                            got.noun(),
                            expected.noun()
                        ),
                        mutating,
                    ));
                }
                Ok((session_id, reply))
            }
            // The TEXT is rendered verbatim whoever wrote it — the
            // supervisor's relay, or the helm's own listing — because
            // SPEC.md's actionable-error rule applies to both hops and
            // neither side's prose improves by being paraphrased here. The
            // BYTES are not, though: with the lifecycle verbs landed, this
            // is the first `AgentOutcome::Err` that can carry a TARGET
            // supervisor's own free-text refusal (a rejected rename title,
            // say) rather than only this build's own fixed sentences, and
            // `main`'s default `Result` printer puts an uncaught `bail!`
            // string on stderr with no escaping of its own — unlike every
            // successful confirmation, which already runs its dynamic
            // fields through `safe_cell`. `safe_error_message` gives this
            // path the same floor.
            AgentOutcome::Err { message, .. } => anyhow::bail!(safe_error_message(&message)),
        },
        other => Err(unexpected_reply("agent", &other, mutating)),
    }
}

/// The error for a reply that never usably arrived, in the two vocabularies
/// that situation has once the request itself is known to have gone out.
///
/// EVERY post-write ending but one comes through here, and the breadth is
/// the point rather than an accident of where the calls happen to sit: the
/// socket dying, a frame that will not decode, a response correlated with
/// somebody else's `req_id`, a control message that answers nothing, a
/// success reply of the wrong shape. They look nothing alike and they all
/// license exactly one conclusion — the request went out, and nothing came
/// back that says what became of it. The single exclusion is a
/// `ControlMsg::Error` the supervisor itself wrote, which IS a statement
/// about the outcome and must not have doubt appended to it.
///
/// The local socket dying takes every `ErrorKind` with it — there is no
/// `AgentOutcome` left to carry a classification — so this is the one place
/// the distinction can still be made, and the CLI is the one party that can
/// still make it: it knows the verb it sent and that its write completed.
/// The facts are identical either way (the request reached the supervisor's
/// socket; the supervisor may have forwarded it to a helm, which may have
/// applied it on another host; the answer was lost), and what differs is
/// what the reader should do next. A listing has nothing to double-apply
/// and gets the plain transport wording, which already reads as "ask
/// again". A MUTATION — a rename/stop/restart, or a create/clone that may
/// by now have a session running on some host — may ALREADY have taken
/// effect, so it gets
/// the same "look before you retry" remedy the relay's own delivered-but-
/// unanswered endings carry — one sentence across every hop, rather than a
/// helpful answer that stops at the process boundary.
fn lost_reply(cause: &str, mutating: bool) -> anyhow::Error {
    if !mutating {
        return anyhow::anyhow!("{cause}");
    }
    anyhow::anyhow!(
        "{cause}; the request had already been sent, so the outcome is unknown — {}",
        farhelm_proto::AGENT_MUTATION_UNKNOWN_REMEDY
    )
}

/// Which reply shape a verb must be answered with.
///
/// A retained expectation, because the protocol's `reply` tag is only
/// useful to a client that remembers what it asked. The relay hands a
/// response back by `req_id` across two hops and nothing on either hop
/// re-checks the shape, so this is the only place a mismatch can be caught.
///
/// `Session` is the rename reply shape, so this check confirms the helm
/// answered with the kind of payload that command promises.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReplyKind {
    Hosts,
    Sessions,
    Profiles,
    Session,
    Restarted,
    Stopped,
    Created,
    ResolvedProfile,
}

impl ReplyKind {
    fn of_verb(verb: &farhelm_proto::AgentVerb) -> ReplyKind {
        match verb {
            farhelm_proto::AgentVerb::Hosts {} => ReplyKind::Hosts,
            farhelm_proto::AgentVerb::Sessions {} => ReplyKind::Sessions,
            farhelm_proto::AgentVerb::Profiles {} => ReplyKind::Profiles,
            farhelm_proto::AgentVerb::Rename { .. } => ReplyKind::Session,
            farhelm_proto::AgentVerb::Stop { .. } => ReplyKind::Stopped,
            farhelm_proto::AgentVerb::Restart { .. } => ReplyKind::Restarted,
            // `Created`, not `Session`: the two payloads are identical and
            // the tag is the only thing separating "a row that did not
            // exist" from "the row you changed". Checking it here is what
            // stops this CLI printing an EXISTING session's id as though
            // it had just created one — a target an agent might then go on
            // to stop.
            farhelm_proto::AgentVerb::Create { .. } | farhelm_proto::AgentVerb::Clone { .. } => {
                ReplyKind::Created
            }
            // This is an internal supervisor-to-helm query, never a CLI
            // verb. Classifying it keeps a malformed peer reply recoverable
            // instead of letting a new wire variant abort this process.
            farhelm_proto::AgentVerb::ResolveProfile { .. } => ReplyKind::ResolvedProfile,
        }
    }

    fn of(reply: &AgentReply) -> ReplyKind {
        match reply {
            AgentReply::Hosts { .. } => ReplyKind::Hosts,
            AgentReply::Sessions { .. } => ReplyKind::Sessions,
            AgentReply::Profiles { .. } => ReplyKind::Profiles,
            AgentReply::Session { .. } => ReplyKind::Session,
            AgentReply::Restarted { .. } => ReplyKind::Restarted,
            AgentReply::Stopped {} => ReplyKind::Stopped,
            AgentReply::Created { .. } => ReplyKind::Created,
            AgentReply::ResolvedProfile { .. } => ReplyKind::ResolvedProfile,
        }
    }

    /// What this reply shape is CALLED in an error a user reads.
    ///
    /// A noun phrase for the reply itself, not for the verb that asked for
    /// it: the mismatch message names two of these and has no idea which
    /// one was the request, so anything verb-flavored reads backwards half
    /// the time.
    fn noun(self) -> &'static str {
        match self {
            ReplyKind::Hosts => "hosts listing",
            ReplyKind::Sessions => "sessions listing",
            ReplyKind::Profiles => "profiles listing",
            ReplyKind::Session => "session row",
            ReplyKind::Restarted => "restarted session row",
            ReplyKind::Stopped => "stop confirmation",
            // "created", not "new": the whole point of this noun is to
            // read differently from `Session`'s in a message that names
            // both, and a reader who sees "session row" against "created
            // session row" can tell which end of the mismatch is which.
            ReplyKind::Created => "created session row",
            ReplyKind::ResolvedProfile => "resolved profile",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::ffi::OsStringExt;

    /// A complete, well-formed session environment.
    fn full_env() -> SessionEnv {
        SessionEnv {
            session_id: Some("s-1".into()),
            token: Some("secret".into()),
            socket: Some("/state/supervisor.sock".into()),
        }
    }

    /// A value no UTF-8 decoder accepts.
    fn not_utf8() -> OsString {
        OsString::from_vec(vec![0x66, 0xff, 0x6f])
    }

    /// Spec: the hook log sits in `hook-log/<session>.log` beside the
    /// socket, and needs only the id and the socket.
    ///
    /// The token-less half is the reason the two hook readings are separate
    /// at all: a session whose credential is missing is precisely the one
    /// whose only diagnostic is this log, so losing the path whenever the
    /// credential is incomplete would erase the evidence of the failure.
    #[farhelm_testtrace::test]
    fn the_hook_log_needs_only_the_id_and_socket() {
        let expected = Some(PathBuf::from("/state/hook-log/s-1.log"));
        assert_eq!(full_env().hook_log(), expected);
        let tokenless = SessionEnv {
            token: None,
            ..full_env()
        };
        assert_eq!(tokenless.hook_log(), expected);
        assert!(tokenless.hook_credential().is_none());
        assert!(
            SessionEnv {
                socket: None,
                ..full_env()
            }
            .hook_log()
            .is_none()
        );
    }

    /// Spec: the hook reports only with all three values, and treats a
    /// non-UTF-8 value as absent rather than failing.
    ///
    /// A hook runs inside the agent's own tool loop, so a hard error there
    /// is an error the user sees in their agent; absence is the one answer
    /// that keeps it silent.
    #[farhelm_testtrace::test]
    fn the_hook_credential_needs_all_three_readable_values() {
        let credential = full_env().hook_credential().expect("complete env");
        assert_eq!(credential.session_id, "s-1");
        assert_eq!(credential.token, "secret");
        assert_eq!(credential.socket, PathBuf::from("/state/supervisor.sock"));
        let garbled = SessionEnv {
            session_id: Some(not_utf8()),
            ..full_env()
        };
        assert!(garbled.hook_credential().is_none());
        assert!(garbled.hook_log().is_none());
    }

    /// Spec: `dial` refuses an incomplete environment with an error naming
    /// what is wrong, and a token-less session is told to restart.
    ///
    /// `tests/spawn_cli.rs` pins these through the built binary for
    /// `farhelm spawn`; this pins the shared validation directly, so the
    /// `agent` path that also depends on it is covered without a process.
    #[farhelm_testtrace::test]
    fn dial_refuses_incomplete_environments_by_name() {
        let error = |env: SessionEnv| env.dial("farhelm spawn").unwrap_err().to_string();
        assert!(
            error(SessionEnv {
                token: None,
                ..full_env()
            })
            .contains("must be restarted before running farhelm spawn")
        );
        assert!(
            error(SessionEnv {
                socket: None,
                ..full_env()
            })
            .contains(SUPERVISOR_SOCK_ENV_VAR)
        );
        assert!(
            error(SessionEnv {
                token: Some(not_utf8()),
                ..full_env()
            })
            .contains(SESSION_TOKEN_ENV_VAR)
        );
        let dial = full_env().dial("farhelm spawn").expect("complete env");
        assert_eq!(dial.session_id, "s-1");
    }

    /// Profile resolution is an internal reply shape, so it must not share
    /// the creating verbs' classification: otherwise a malformed create or
    /// clone reply can evade the outcome-unknown remedy.
    #[farhelm_testtrace::test]
    fn profile_resolution_has_its_own_reply_kind() {
        let create = ReplyKind::of_verb(&farhelm_proto::AgentVerb::Create {
            host: None,
            cwd: "/w".to_string(),
            profile_name: None,
            profile_id: None,
            invocation: Some("claude".to_string()),
            title: None,
            intent_key: None,
        });
        let resolve = ReplyKind::of_verb(&farhelm_proto::AgentVerb::ResolveProfile {
            name: Some("claude".to_string()),
            id: None,
        });
        assert_ne!(create, resolve);
        assert_eq!(resolve.noun(), "resolved profile");
    }
}
