//! The supervisor's agent→helm relay, driven end to end against a real
//! supervisor: a session-authenticated peer asks, the supervisor forwards
//! to whichever connection holds that session's attachment, and the answer
//! comes back under the asking peer's own request id.
//!
//! Everything here is about the SUPERVISOR's half. The helm-side
//! projection (which host is `current`, what a status word is) is unit
//! tested where it lives, in `farhelm-helm`'s `agent_requests`, and the
//! helm client's own upcall behavior (the handler slot's startup window,
//! keeping a slow handler off the demultiplexer, admission, oversized
//! replies) in `farhelm-helm`'s `client`. What cannot be tested in either
//! is the part that only exists when several real connections are open at
//! once — the routing rule, the two request-id namespaces, credential
//! revocation on an already-open socket, and the ways an upcall can fail to
//! produce an answer.

use crate::harness::*;
use farhelm_helm::agent_requests::{AgentRequestHandler, AgentRequestSlot};
use farhelm_proto::io::handshake_with_session_auth;
use farhelm_proto::{AgentOutcome, AgentReply, AgentVerb, SessionAuth};

/// A helm-side handler under the test's control.
///
/// Answers whatever the test told it to and records what it was asked, so
/// a test can assert that the SUPERVISOR passed the session id and verb
/// through unchanged — the relay's only job, and one that no assertion on
/// the returned value alone can distinguish from a lucky default.
///
/// The optional GATE is what lets a test hold several upcalls in flight at
/// once and finish them in an order of its choosing: it announces every
/// call on `entered` and then waits for a permit. Without that, a test
/// about two concurrent upcalls could only ever observe them one at a time.
struct ScriptedHandler {
    reply: Option<AgentReply>,
    asked: std::sync::Mutex<Vec<(String, AgentVerb)>>,
    entered: Option<tokio::sync::mpsc::Sender<String>>,
    /// Per-session gates, so a test can release the two peers' upcalls in
    /// whichever order it wants to prove routing against.
    gates: std::sync::Mutex<std::collections::HashMap<String, Arc<tokio::sync::Semaphore>>>,
}

impl ScriptedHandler {
    /// A handler that answers with `reply`.
    fn answering(reply: AgentReply) -> Arc<ScriptedHandler> {
        Arc::new(ScriptedHandler {
            reply: Some(reply),
            asked: std::sync::Mutex::new(Vec::new()),
            entered: None,
            gates: std::sync::Mutex::new(std::collections::HashMap::new()),
        })
    }

    /// A handler that never answers — the wedged-helm case the upcall
    /// timeout exists for. `None` is the whole script: `handle` parks
    /// forever rather than returning.
    fn silent() -> Arc<ScriptedHandler> {
        Arc::new(ScriptedHandler {
            reply: None,
            asked: std::sync::Mutex::new(Vec::new()),
            entered: None,
            gates: std::sync::Mutex::new(std::collections::HashMap::new()),
        })
    }

    /// A handler that announces each call and then waits to be released.
    ///
    /// It answers with a `Hosts` reply whose single host is named after the
    /// asking session, which is what makes one peer's answer distinguishable
    /// from the other's — the whole point when two are in flight.
    fn gated() -> (Arc<ScriptedHandler>, tokio::sync::mpsc::Receiver<String>) {
        let (entered, calls) = tokio::sync::mpsc::channel(8);
        (
            Arc::new(ScriptedHandler {
                reply: None,
                asked: std::sync::Mutex::new(Vec::new()),
                entered: Some(entered),
                gates: std::sync::Mutex::new(std::collections::HashMap::new()),
            }),
            calls,
        )
    }

    /// The gate for `session`, created on first use by whichever side gets
    /// there first — the handler entering, or the test releasing.
    fn gate(&self, session: &str) -> Arc<tokio::sync::Semaphore> {
        Arc::clone(
            self.gates
                .lock()
                .unwrap()
                .entry(session.to_string())
                .or_insert_with(|| Arc::new(tokio::sync::Semaphore::new(0))),
        )
    }

    /// Let `session`'s parked upcall finish.
    fn release(&self, session: &str) {
        self.gate(session).add_permits(1);
    }
}

#[async_trait::async_trait]
impl AgentRequestHandler for ScriptedHandler {
    async fn handle(
        &self,
        _origin: farhelm_helm::agent_requests::AgentOrigin,
        session_id: &str,
        verb: AgentVerb,
    ) -> AgentOutcome {
        self.asked
            .lock()
            .unwrap()
            .push((session_id.to_string(), verb));
        if let Some(entered) = &self.entered {
            let _ = entered.send(session_id.to_string()).await;
            let gate = self.gate(session_id);
            let _permit = gate.acquire().await.expect("the gate is never closed");
            return AgentOutcome::Ok {
                reply: AgentReply::Hosts {
                    hosts: vec![farhelm_proto::AgentHost {
                        name: session_id.to_string(),
                        kind: "local".to_string(),
                        state: "connected".to_string(),
                        current: true,
                    }],
                },
            };
        }
        match &self.reply {
            Some(reply) => AgentOutcome::Ok {
                reply: reply.clone(),
            },
            None => std::future::pending().await,
        }
    }
}

/// Connect a client that answers agent upcalls with `handler`.
///
/// `harness::connect_client`'s twin, kept here rather than in the harness
/// because no other module needs a connection with an upcall handler on
/// it. Registry id 1 is arbitrary: the supervisor never inspects it, and
/// the helm-side meaning of the id (which host is `current`) is tested
/// where that projection lives.
async fn connect_helm(
    sup: &Arc<Supervisor>,
    handler: Arc<dyn AgentRequestHandler>,
) -> Arc<SupervisorClient> {
    let (client_side, server_side) = tokio::io::duplex(1 << 20);
    let sup = Arc::clone(sup);
    tokio::spawn(async move {
        let _ = handle_connection(sup, server_side).await;
    });
    let (r, w) = tokio::io::split(client_side);
    let slot: AgentRequestSlot = Arc::new(std::sync::OnceLock::from(handler));
    SupervisorClient::start_for_host(r, w, slot, 1)
        .await
        .expect("handshake")
}

/// Like [`connect_helm`], but with a frame-level TAP on the supervisor's
/// outbound leg, recording the `req_id` of every `AgentRequest` the
/// supervisor sends UP this connection.
///
/// The tap exists because the two request-id namespaces are otherwise
/// unobservable from a test. A handler is handed the session id and the
/// verb — everything except the number the supervisor minted — so a relay
/// that forwarded the asking peer's `req_id` unchanged looks identical from
/// inside the handler. Splicing a relay between the two duplex halves is
/// the smallest way to see the actual frames without reimplementing the
/// helm's client.
async fn connect_helm_tapping_request_ids(
    sup: &Arc<Supervisor>,
    handler: Arc<dyn AgentRequestHandler>,
) -> (Arc<SupervisorClient>, Arc<std::sync::Mutex<Vec<u64>>>) {
    let (supervisor_side, server_side) = tokio::io::duplex(1 << 20);
    let sup_for_conn = Arc::clone(sup);
    tokio::spawn(async move {
        let _ = handle_connection(sup_for_conn, server_side).await;
    });

    let (helm_side, tap_side) = tokio::io::duplex(1 << 20);
    let seen: Arc<std::sync::Mutex<Vec<u64>>> = Arc::new(std::sync::Mutex::new(Vec::new()));

    let (sup_read, sup_write) = tokio::io::split(supervisor_side);
    let (tap_read, tap_write) = tokio::io::split(tap_side);
    // Supervisor → helm, inspected on the way past.
    let recorder = Arc::clone(&seen);
    tokio::spawn(async move {
        let mut reader = FrameReader::new(sup_read);
        let mut writer = FrameWriter::new(tap_write);
        while let Ok(Some(frame)) = reader.read_frame().await {
            if let Ok(ControlMsg::AgentRequest { req_id, .. }) = parse_control(&frame) {
                recorder.lock().unwrap().push(req_id);
            }
            if writer.write_frame(&frame).await.is_err() {
                break;
            }
        }
    });
    // Helm → supervisor, forwarded untouched.
    tokio::spawn(async move {
        let mut reader = FrameReader::new(tap_read);
        let mut writer = FrameWriter::new(sup_write);
        while let Ok(Some(frame)) = reader.read_frame().await {
            if writer.write_frame(&frame).await.is_err() {
                break;
            }
        }
    });

    let (r, w) = tokio::io::split(helm_side);
    let slot: AgentRequestSlot = Arc::new(std::sync::OnceLock::from(handler));
    let client = SupervisorClient::start_for_host(r, w, slot, 1)
        .await
        .expect("handshake");
    (client, seen)
}

/// One raw session-authenticated peer, as `farhelm agent` is: hello with
/// the session credential, then frames by hand.
///
/// Deliberately NOT a `SupervisorClient`: that type is the helm's
/// full-authority client, and the whole point of these tests is the
/// narrow admission a session credential buys. Driving frames directly is
/// also what lets a test send a `session_id` that does not match its
/// credential, which no typed client would let it construct.
struct SessionPeer {
    reader: FrameReader<tokio::io::ReadHalf<tokio::io::DuplexStream>>,
    writer: FrameWriter<tokio::io::WriteHalf<tokio::io::DuplexStream>>,
}

impl SessionPeer {
    async fn connect(sup: &Arc<Supervisor>, session_id: &str, token: &str) -> SessionPeer {
        let (client_side, server_side) = tokio::io::duplex(1 << 20);
        let sup = Arc::clone(sup);
        tokio::spawn(async move {
            let _ = handle_connection(sup, server_side).await;
        });
        let (r, w) = tokio::io::split(client_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake_with_session_auth(
            &mut reader,
            &mut writer,
            SessionAuth {
                session_id: session_id.to_string(),
                token: token.to_string(),
            },
        )
        .await
        .expect("authenticated handshake");
        SessionPeer { reader, writer }
    }

    /// Send one question without waiting for its answer.
    ///
    /// Split from [`Self::ask`] so a test can hold several peers' requests
    /// in flight at once, which is the only way to observe that the relay
    /// keeps two connection-local `req_id` 1s apart.
    ///
    /// `asking_as` is separate from the credential on purpose: the
    /// mismatch case needs to name a session this peer is not. The `req_id`
    /// is always 1 — every peer numbers its own connection from 1, which is
    /// exactly the collision the relay has to survive.
    async fn send(&mut self, asking_as: &str, request: AgentVerb) {
        self.writer
            .write_control(&ControlMsg::AgentRequest {
                req_id: 1,
                session_id: asking_as.to_string(),
                request,
            })
            .await
            .expect("send the agent request");
    }

    /// Read the single frame answering this peer's outstanding request.
    async fn answer(&mut self) -> ControlMsg {
        let frame = tokio::time::timeout(Duration::from_secs(20), self.reader.read_frame())
            .await
            .expect("the supervisor never answered the agent request")
            .expect("read the answer")
            .expect("the supervisor closed instead of answering");
        parse_control(&frame).expect("decode the answer")
    }

    /// Ask one question and read the single frame that answers it.
    async fn ask(&mut self, asking_as: &str, request: AgentVerb) -> ControlMsg {
        self.send(asking_as, request).await;
        self.answer().await
    }

    /// Send a raw restricted-control message and read its correlated reply.
    ///
    /// Spawn uses `CreateSession` directly rather than the `AgentRequest`
    /// envelope used by `farhelm agent`. Keeping this seam raw is what lets
    /// the forgery regression submit provenance the typed CLI never emits.
    async fn control(&mut self, request: ControlMsg) -> ControlMsg {
        self.writer
            .write_control(&request)
            .await
            .expect("send the restricted control request");
        self.answer().await
    }
}

/// The outcome of an `AgentResponse` correlated with the peer's own
/// request id, failing loudly on anything else — a bare `Error`, or a
/// response for some other request.
fn outcome_of(reply: ControlMsg) -> AgentOutcome {
    match reply {
        ControlMsg::AgentResponse { req_id: 1, outcome } => outcome,
        other => panic!("expected an AgentResponse for req_id 1, got {other:?}"),
    }
}

/// The token the supervisor minted for `session`, read out of its own
/// database — the credential a real `farhelm agent` finds in its
/// environment.
async fn credential_for(h: &Harness, session: &str) -> String {
    let store = SessionStore::open(&h.state.path().join("supervisor.db"), false)
        .await
        .expect("open the supervisor's store");
    store
        .session_token(session)
        .await
        .expect("read the session token")
        .expect("a created session has a credential")
}

/// Spec: a session-authenticated `AgentRequest` reaches the handler of the
/// connection that holds that session's attachment, with the session id
/// and verb intact, and the handler's reply comes back to the asking peer
/// under the peer's own request id.
///
/// This is the whole feature in one test, and every clause is load-bearing.
/// The ROUTING is what makes "ask the helm the user is looking at"
/// meaningful — the harness's own client is connected the whole time and
/// must NOT be the one asked, because it holds no attachment. The two
/// request-id namespaces get their own test below, where two peers can
/// collide on the same local number.
#[tokio::test]
async fn an_agent_request_is_answered_by_the_helm_holding_the_attachment() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let token = credential_for(&h, &session.id).await;

    let handler = ScriptedHandler::answering(AgentReply::Hosts {
        hosts: vec![farhelm_proto::AgentHost {
            name: "this machine".to_string(),
            kind: "local".to_string(),
            state: "connected".to_string(),
            current: true,
        }],
    });
    let helm = connect_helm(&h.sup, handler.clone()).await;
    // The attachment is what makes this connection the one asked. Held for
    // the rest of the test: dropping the stream would tear it down and
    // change which link the relay finds.
    let (_channel, _stream) = helm
        .attach(&session.id, 80, 24)
        .await
        .expect("the second client attaches");

    let mut peer = SessionPeer::connect(&h.sup, &session.id, &token).await;
    let outcome = outcome_of(peer.ask(&session.id, AgentVerb::Hosts {}).await);

    match outcome {
        AgentOutcome::Ok {
            reply: AgentReply::Hosts { hosts },
        } => {
            assert_eq!(hosts.len(), 1);
            assert_eq!(hosts[0].name, "this machine");
            assert!(hosts[0].current);
        }
        other => panic!("expected the handler's own reply, got {other:?}"),
    }
    assert_eq!(
        *handler.asked.lock().unwrap(),
        vec![(session.id.clone(), AgentVerb::Hosts {})],
        "the relay must forward the asking session and verb unchanged"
    );
}

/// Spec: with no helm attached to the session, the peer is refused
/// `Unavailable` with the relay's own remedy sentence, and no request is
/// forwarded anywhere.
///
/// The design chose to route EVERY verb through the helm, including
/// questions the supervisor could have answered about its own host, so
/// that there is one code path and one place for policy. This refusal is
/// the price of that choice, and it has to be the honest failure rather
/// than a silent downgrade to local semantics — an agent that sometimes
/// sees the fleet and sometimes sees one machine, with nothing saying
/// which, is worse off than one that is told to open the session.
///
/// The harness's client is connected throughout: a relay that picked "any
/// full-authority connection" instead of "the one holding the attachment"
/// would answer here and pass nothing.
#[tokio::test]
async fn a_session_no_helm_is_attached_to_is_told_so() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let token = credential_for(&h, &session.id).await;
    // A helm with a handler is connected but attached to NOTHING, which is
    // the case a "is any helm connected?" check would get wrong.
    let _helm = connect_helm(
        &h.sup,
        ScriptedHandler::answering(AgentReply::Hosts { hosts: Vec::new() }),
    )
    .await;

    let mut peer = SessionPeer::connect(&h.sup, &session.id, &token).await;
    match outcome_of(peer.ask(&session.id, AgentVerb::Hosts {}).await) {
        AgentOutcome::Err { kind, message } => {
            assert_eq!(kind, ErrorKind::Unavailable);
            assert_eq!(
                message,
                "no helm is attached to this session — open the session in the farhelm UI and \
                 try again"
            );
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// Spec: a helm that takes the request and never answers costs the asking
/// session one upcall budget and then a `Timeout`, not a hang.
///
/// The asking process has no deadline of its own — deliberately, since only
/// the supervisor can tell "nobody to ask" from "asked, still waiting" —
/// so this bound is the only thing between a wedged helm and an agent's
/// shell command blocked forever. `Timeout` rather than `Unavailable`
/// because the request WAS delivered: the far side may still be working,
/// which is exactly the distinction that decides whether a retry is free.
#[tokio::test]
async fn a_helm_that_never_answers_times_the_request_out() {
    let h = harness_with_timeouts(SupervisorTimeouts {
        // Short enough to wait out in a test, long enough that a merely
        // busy runner cannot reach it before the handler has parked.
        agent_upcall: Duration::from_millis(500),
        ..SupervisorTimeouts::default()
    })
    .await;
    let (session, _work) = basic_session(&h).await;
    let token = credential_for(&h, &session.id).await;

    let helm = connect_helm(&h.sup, ScriptedHandler::silent()).await;
    let (_channel, _stream) = helm
        .attach(&session.id, 80, 24)
        .await
        .expect("the silent client attaches");

    let mut peer = SessionPeer::connect(&h.sup, &session.id, &token).await;
    match outcome_of(peer.ask(&session.id, AgentVerb::Sessions {}).await) {
        AgentOutcome::Err { kind, message } => {
            assert_eq!(kind, ErrorKind::Timeout);
            assert!(
                message.contains("did not answer"),
                "the timeout must say what did not happen, got: {message}"
            );
        }
        other => panic!("expected a timeout refusal, got {other:?}"),
    }
}

/// Spec: a credential for one session is not authority to ask as another;
/// the mismatch is refused `Unauthorized` before anything is forwarded.
///
/// The check has to live at this hop and nowhere else. The helm never sees
/// the credential — by the time a request reaches it, `session_id` is the
/// only claim about who is asking — so a supervisor that forwarded an
/// unchecked id would hand the helm a lie it has no way to detect, and
/// every later verb (rename, stop, archive) would act on it.
#[tokio::test]
async fn a_peer_may_not_ask_as_another_session() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let (other, _other_work) = basic_session(&h).await;
    let token = credential_for(&h, &session.id).await;

    let helm = connect_helm(
        &h.sup,
        ScriptedHandler::answering(AgentReply::Hosts { hosts: Vec::new() }),
    )
    .await;
    let (_channel, _stream) = helm
        .attach(&other.id, 80, 24)
        .await
        .expect("the helm attaches to the OTHER session");

    let mut peer = SessionPeer::connect(&h.sup, &session.id, &token).await;
    match outcome_of(peer.ask(&other.id, AgentVerb::Hosts {}).await) {
        AgentOutcome::Err { kind, message } => {
            assert_eq!(kind, ErrorKind::Unauthorized);
            assert!(
                message.contains("under its own identity"),
                "the refusal must say what the rule is, got: {message}"
            );
            // The rule bounds WHO IS ASKING, not what may be acted on, and
            // the refusal has to make that clear: a reader who took it as
            // "you may not touch another session" would conclude the
            // lifecycle verbs' documented fleet-wide target was a lie.
            assert!(
                message.contains("name that session as the verb's own target"),
                "the refusal must point at the target field it is NOT about, got: {message}"
            );
        }
        other => panic!("expected an authorization refusal, got {other:?}"),
    }
}

/// Spec: two peers that both number their request `1` are relayed under
/// DIFFERENT supervisor-side request ids, and each answer comes back to the
/// peer that asked — even when the helm answers them in the opposite order.
///
/// The two legs live in separate `req_id` namespaces, and the reason is
/// structural rather than stylistic: `req_id` has only ever meant "request N
/// on THIS connection", so every peer starts at 1 and two peers colliding
/// is the ordinary case rather than an edge one. A relay that forwarded the
/// asking peer's number would put two entries under key 1 in one link's
/// pending table, and the first would be silently overwritten.
///
/// The tap is what makes the claim checkable. A handler sees only the
/// session id and the verb, so the forwarded number is invisible from
/// inside it — an implementation that reused the downstream id would look
/// identical there. Reverse-order release covers the other half: two
/// answers arriving out of order must still be matched by id rather than by
/// arrival.
#[tokio::test]
async fn two_peers_using_the_same_request_id_are_relayed_and_answered_apart() {
    let h = harness().await;
    let (first, _first_work) = basic_session(&h).await;
    let (second, _second_work) = basic_session(&h).await;
    let first_token = credential_for(&h, &first.id).await;
    let second_token = credential_for(&h, &second.id).await;

    let (handler, mut calls) = ScriptedHandler::gated();
    let (helm, upstream_ids) = connect_helm_tapping_request_ids(&h.sup, handler.clone()).await;
    // ONE helm connection holding BOTH attachments, which is what puts the
    // two upcalls on one link and one pending table.
    let (_c1, _s1) = helm
        .attach(&first.id, 80, 24)
        .await
        .expect("attach the first session");
    let (_c2, _s2) = helm
        .attach(&second.id, 80, 24)
        .await
        .expect("attach the second session");

    let mut peer_one = SessionPeer::connect(&h.sup, &first.id, &first_token).await;
    let mut peer_two = SessionPeer::connect(&h.sup, &second.id, &second_token).await;
    peer_one.send(&first.id, AgentVerb::Hosts {}).await;
    peer_two.send(&second.id, AgentVerb::Hosts {}).await;

    // Both upcalls are now parked inside the handler, so both are in the
    // link's pending table at once — the state a shared namespace breaks.
    let mut seen = Vec::new();
    for _ in 0..2 {
        seen.push(
            tokio::time::timeout(Duration::from_secs(20), calls.recv())
                .await
                .expect("an upcall never reached the handler")
                .expect("the handler's announcement channel is open"),
        );
    }
    seen.sort();
    let mut expected = vec![first.id.clone(), second.id.clone()];
    expected.sort();
    assert_eq!(seen, expected, "both sessions must reach the handler");

    let ids = upstream_ids.lock().unwrap().clone();
    assert_eq!(ids.len(), 2, "two upcalls travelled up: {ids:?}");
    assert_ne!(
        ids[0], ids[1],
        "the supervisor must mint its own request ids: {ids:?}"
    );

    // Reverse order on purpose: the second peer's answer is produced first.
    handler.release(&second.id);
    handler.release(&first.id);

    let one = outcome_of(peer_one.answer().await);
    let two = outcome_of(peer_two.answer().await);
    for (expected_session, outcome) in [(&first.id, one), (&second.id, two)] {
        match outcome {
            AgentOutcome::Ok {
                reply: AgentReply::Hosts { hosts },
            } => assert_eq!(
                hosts[0].name, *expected_session,
                "each peer must receive its OWN answer"
            ),
            other => panic!("expected the handler's reply, got {other:?}"),
        }
    }
}

/// Spec: a helm that accepts an upcall and then loses its connection ends
/// the request promptly with `Unavailable` and the relay's remedy — not
/// with the full upcall budget's silence.
///
/// This is the third ending the relay module describes and the one most
/// likely to actually happen: a helm restarts, an ssh channel dies, a
/// laptop closes. `HelmLink::fail_all` is what converts it from a
/// thirty-second wait for an answer that provably cannot arrive into an
/// immediate refusal, and nothing else in the suite would notice if that
/// call were removed or deferred behind the slower attachment teardown —
/// the request would still end, eventually, by timing out.
///
/// The KIND matters as much as the speed. `Unavailable` says nobody has the
/// request any more, which is safe to retry; `Timeout` would say the helm
/// may still be working on it.
#[tokio::test]
#[ignore = "load flake: fails 2 of 5 full-suite runs on a 4-vCPU runner and blocked two release gates; TODO.md has the evidence and the ladder to climb"]
async fn a_helm_that_dies_mid_upcall_ends_the_request_at_once() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let token = credential_for(&h, &session.id).await;

    let (handler, mut calls) = ScriptedHandler::gated();
    let helm = connect_helm(&h.sup, handler.clone()).await;
    let (_channel, stream) = helm
        .attach(&session.id, 80, 24)
        .await
        .expect("the helm attaches");

    let mut peer = SessionPeer::connect(&h.sup, &session.id, &token).await;
    peer.send(&session.id, AgentVerb::Hosts {}).await;
    // The barrier: the helm has the request. Anything that ends it from
    // here is the mid-upcall case rather than a race with delivery.
    tokio::time::timeout(Duration::from_secs(20), calls.recv())
        .await
        .expect("the upcall never reached the handler")
        .expect("the handler's announcement channel is open");

    // Dropping every external handle closes the helm's write half, which
    // the supervisor sees as EOF — the same thing a killed helm looks like.
    drop(stream);
    drop(helm);

    let started = std::time::Instant::now();
    match outcome_of(peer.answer().await) {
        AgentOutcome::Err { kind, message } => {
            assert_eq!(kind, ErrorKind::Unavailable);
            assert!(
                message.contains("open the session in the farhelm UI"),
                "a connection-loss refusal must carry the remedy, got: {message}"
            );
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "the refusal must be prompt rather than the upcall budget expiring; took {:?}",
        started.elapsed()
    );
}

/// Spec: a credential revoked AFTER the handshake stops working
/// immediately — the request is refused `Unauthorized` and never reaches
/// the helm.
///
/// The restricted arm re-checks the credential on every agent request
/// rather than trusting the socket's own admission, and that is the whole
/// point: a session can be deleted while its agent's connection is still
/// open, and a socket that retained authority from its handshake would let
/// a deleted session go on reading the fleet. Existing tests pin this
/// re-check for restricted creation and conversation reporting; this verb
/// was newly admitted to the same arm and had none.
///
/// "No request reached the helm" is asserted separately because a refusal
/// AFTER the relay would still exit non-zero while having already leaked
/// the fleet listing to whatever forwarded it.
#[tokio::test]
async fn a_deleted_session_cannot_ask_on_a_connection_it_already_opened() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let token = credential_for(&h, &session.id).await;

    let handler = ScriptedHandler::answering(AgentReply::Hosts { hosts: Vec::new() });
    let helm = connect_helm(&h.sup, handler.clone()).await;
    let (_channel, _stream) = helm
        .attach(&session.id, 80, 24)
        .await
        .expect("the helm attaches");

    // Authenticated while the session still exists.
    let mut peer = SessionPeer::connect(&h.sup, &session.id, &token).await;

    let store = SessionStore::open(&h.state.path().join("supervisor.db"), false)
        .await
        .expect("open the supervisor's store");
    store
        .delete_session_settling_reservations(&session.id)
        .await
        .expect("delete the session row");

    match outcome_of(peer.ask(&session.id, AgentVerb::Hosts {}).await) {
        AgentOutcome::Err { kind, message } => {
            assert_eq!(kind, ErrorKind::Unauthorized);
            assert!(
                message.contains("no longer exists") || message.contains("invalid"),
                "the refusal must say why, got: {message}"
            );
        }
        other => panic!("a revoked credential must be refused, got {other:?}"),
    }
    assert!(
        handler.asked.lock().unwrap().is_empty(),
        "no request may reach the helm after the credential is gone"
    );
}

/// A direct session-authenticated `ResolveProfile` request is refused at the
/// supervisor and never reaches the attached helm.
///
/// The resolved bundle contains the full invocation and resume template,
/// which ordinary agent-facing listings intentionally redact. The same wire
/// verb remains available to the supervisor's internal named-spawn path; the
/// sibling test below proves that path still reaches the scripted helm.
#[tokio::test]
async fn a_session_cannot_call_the_internal_profile_resolver_directly() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let token = credential_for(&h, &session.id).await;

    let handler = ScriptedHandler::answering(AgentReply::ResolvedProfile {
        invocation: "secret-agent --token hidden".to_string(),
        agent_kind: farhelm_proto::AgentKind::Generic,
        resume_template: None,
        source_profile: farhelm_proto::ProfileSnapshot {
            id: "secret-profile".to_string(),
            name: "Secret profile".to_string(),
        },
    });
    let helm = connect_helm(&h.sup, handler.clone()).await;
    let (_channel, _stream) = helm
        .attach(&session.id, 80, 24)
        .await
        .expect("the helm attaches");

    let mut peer = SessionPeer::connect(&h.sup, &session.id, &token).await;
    match outcome_of(
        peer.ask(
            &session.id,
            AgentVerb::ResolveProfile {
                name: "Secret profile".to_string(),
            },
        )
        .await,
    ) {
        AgentOutcome::Err { kind, message } => {
            assert_eq!(kind, ErrorKind::InvalidRequest);
            assert_eq!(message, "profile resolution is not available to sessions");
        }
        other => panic!("the internal resolver must be refused, got {other:?}"),
    }
    assert!(
        handler.asked.lock().unwrap().is_empty(),
        "a refused direct resolution must not reach the helm"
    );
}

/// A named spawn resolves through the attached helm and stores the exact
/// bundle the helm returned.
///
/// This is the protocol-15 path no component-level test covers alone: the
/// restricted `CreateSession` enters `resolve_create_selector`, the
/// supervisor sends an internal `ResolveProfile` up the attachment-owning
/// connection, and the resulting invocation, integration settings, resume
/// template, and immutable snapshot become the child's durable launch data.
#[tokio::test]
async fn a_named_spawn_resolves_and_stores_the_attached_helms_bundle() {
    let h = harness().await;
    let (session, work) = basic_session(&h).await;
    let token = credential_for(&h, &session.id).await;
    let snapshot = farhelm_proto::ProfileSnapshot {
        id: "profile-scripted".to_string(),
        name: "Scripted agent".to_string(),
    };
    let resume_template = vec![
        "codex".to_string(),
        "resume".to_string(),
        "{conversation}".to_string(),
    ];
    let handler = ScriptedHandler::answering(AgentReply::ResolvedProfile {
        invocation: "sh -c 'sleep 60'".to_string(),
        agent_kind: farhelm_proto::AgentKind::Codex,
        resume_template: Some(resume_template.clone()),
        source_profile: snapshot.clone(),
    });
    let helm = connect_helm(&h.sup, handler.clone()).await;
    let (_channel, _stream) = helm
        .attach(&session.id, 80, 24)
        .await
        .expect("the helm attaches");

    let mut peer = SessionPeer::connect(&h.sup, &session.id, &token).await;
    let reply = peer
        .control(ControlMsg::CreateSession {
            req_id: 81,
            parent: Some(session.id.clone()),
            cwd: work.path().to_string_lossy().into_owned(),
            invocation: None,
            profile_name: Some("Scripted agent".to_string()),
            title: Some("resolved child".to_string()),
            cols: 80,
            rows: 24,
            intent_key: Some("resolved-spawn".to_string()),
            agent_kind: None,
            resume_template: None,
            source_profile: None,
        })
        .await;
    let ControlMsg::SessionCreated {
        req_id,
        session: child,
    } = reply
    else {
        panic!("the resolved spawn must succeed: {reply:?}");
    };
    assert_eq!(req_id, 81);
    assert_eq!(
        *handler.asked.lock().unwrap(),
        vec![(
            session.id.clone(),
            AgentVerb::ResolveProfile {
                name: "Scripted agent".to_string(),
            },
        )],
        "the supervisor must issue the internal resolution under the asking session"
    );

    let store = SessionStore::open(&h.state.path().join("supervisor.db"), false)
        .await
        .expect("open the supervisor's store");
    let stored = store
        .session(&child.id)
        .await
        .expect("read the child")
        .expect("the child exists");
    assert_eq!(stored.invocation, "sh -c 'sleep 60'");
    assert_eq!(stored.agent_kind, farhelm_proto::AgentKind::Codex);
    assert_eq!(stored.resume_template, Some(resume_template));
    assert_eq!(
        stored
            .source_profile
            .map(|profile| (profile.id, profile.name)),
        Some((snapshot.id, snapshot.name))
    );
}

/// A raw restricted create cannot forge the profile provenance attached to
/// its invocation.
///
/// The request is sent as literal protocol vocabulary because the shipped
/// spawn CLI never constructs this hostile shape. Refusal must happen before
/// reservation or launch work, leaving no child behind under the supplied
/// key.
#[tokio::test]
async fn a_restricted_create_cannot_supply_source_profile() {
    let h = harness().await;
    let (session, work) = basic_session(&h).await;
    let token = credential_for(&h, &session.id).await;
    let mut peer = SessionPeer::connect(&h.sup, &session.id, &token).await;

    let reply = peer
        .control(ControlMsg::CreateSession {
            req_id: 82,
            parent: Some(session.id.clone()),
            cwd: work.path().to_string_lossy().into_owned(),
            invocation: Some("sh -c 'sleep 60'".to_string()),
            profile_name: None,
            title: Some("forged child".to_string()),
            cols: 80,
            rows: 24,
            intent_key: Some("forged-provenance".to_string()),
            agent_kind: Some(farhelm_proto::AgentKind::Generic),
            resume_template: None,
            source_profile: Some(farhelm_proto::ProfileSnapshot {
                id: "starter-codex".to_string(),
                name: "codex".to_string(),
            }),
        })
        .await;
    let ControlMsg::Error {
        req_id,
        kind,
        message,
    } = reply
    else {
        panic!("forged provenance must be refused: {reply:?}");
    };
    assert_eq!(req_id, 82);
    assert_eq!(kind, ErrorKind::InvalidRequest);
    assert!(message.contains("source_profile"));

    let store = SessionStore::open(&h.state.path().join("supervisor.db"), false)
        .await
        .expect("open the supervisor's store");
    assert_eq!(store.reservation("forged-provenance").await.unwrap(), None);
    assert!(
        store
            .load_all()
            .await
            .expect("load sessions")
            .into_iter()
            .all(|row| row.id == session.id),
        "the hostile request must not create a child"
    );
}
