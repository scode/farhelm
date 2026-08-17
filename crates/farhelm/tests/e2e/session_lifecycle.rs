//! The unbannered lifecycle coverage: create/attach/resize/stop/delete
//! session lifecycle, cgroup and process-tree teardown, launch-artifact
//! and alt-screen-snapshot handling. This region predates the file's
//! section-banner convention (and kept accumulating lifecycle tests after
//! it), so unlike the banner-derived modules it is not one
//! banner's worth of tests — it is what remains of the pre-banner region
//! once the shared infrastructure was extracted to `harness`, kept as one
//! unit because the source gives it no finer seams to split along. A few
//! small fixtures live here too (`ToggleWriteFailure`, `with_status`,
//! `hex_tokens`, and friends) because nothing outside this module ever
//! calls them.

use crate::harness::*;

// Two takeover-refusal tests below verify the incumbent attachment
// receives NO detach notice, and use `terminal_backpressure`'s chunked
// reader for the drain rather than `wait_for`'s quadratic full-rescan
// (see that module's doc comment on `drain_for`).
use crate::terminal_backpressure::drain_for;

/// A duplex endpoint whose write direction can fail independently.
///
/// Real sockets can remain readable after their peer stops accepting
/// replies. Tokio's in-memory duplex stream does not expose that state,
/// so this wrapper lets the connection-lifecycle test reproduce it
/// without depending on transport-specific half-close behavior.
struct ToggleWriteFailure {
    inner: tokio::io::DuplexStream,
    fail_writes: Arc<AtomicBool>,
}

impl AsyncRead for ToggleWriteFailure {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for ToggleWriteFailure {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        if self.fail_writes.load(Ordering::SeqCst) {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "injected server write failure",
            )));
        }
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

/// Poll tmux until the window reports `expected` ("COLSxROWS"), failing
/// the test if it never does. Resizes are fire-and-forget, so there is
/// no completion to await — polling is the only observation available.
async fn wait_for_geometry(h: &Harness, expected: &str) {
    let sock = h.state.path().join("tmux.sock");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let out = tmux_query(
            &sock,
            &["display-message", "-p", "#{window_width}x#{window_height}"],
        )
        .await;
        let got = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if got == expected {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "window geometry never reached {expected} (last: {got})"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Require the window geometry to STAY at `expected` for a settle
/// window. Asserting an absence needs a period of observation, not a
/// single read: the resize that must be ignored is in flight, and a
/// single check could run before it would have landed.
async fn assert_geometry_stays(h: &Harness, expected: &str, why: &str) {
    let sock = h.state.path().join("tmux.sock");
    let settle = tokio::time::Instant::now() + Duration::from_secs(2);
    while tokio::time::Instant::now() < settle {
        let out = tmux_query(
            &sock,
            &["display-message", "-p", "#{window_width}x#{window_height}"],
        )
        .await;
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).trim(),
            expected,
            "{why}"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Attach an expected `status` to a cloned `SessionInfo` before an
/// equality assertion.
///
/// `list_sessions` computes `status` fresh from tmux on every call
/// (`service.rs`'s `ListSessions` handler) rather than trusting whatever a
/// caller last saw — so a listing assertion built from a `SessionInfo`
/// returned by `create_session` (always `Unknown`, a create-time
/// placeholder — see `service.rs`'s `create_session` doc comment) must
/// say explicitly what status that same row is expected to carry by the
/// time THIS call observes it, instead of silently reusing the
/// create-time value (which would make the assertion pass or fail on an
/// unrelated coincidence whenever the two happen to agree).
fn with_status(mut session: SessionInfo, status: SessionStatus) -> SessionInfo {
    session.status = status;
    session
}

/// Accumulate one attachment until the counter has advanced past a
/// caller-chosen sequence number.
async fn collect_counter_through(rx: &mut TermStream, target: u64) -> Vec<u8> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    let mut transcript = Vec::new();
    loop {
        if counter_records(&transcript)
            .last()
            .is_some_and(|last| *last >= target)
        {
            return transcript;
        }
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(TermEvent::Data(bytes))) => transcript.extend_from_slice(&bytes),
            // The replay-complete marker (PLAN_M5.md item 4) is
            // presentation metadata, not a counter record — nothing for
            // this accumulator to add. Its own ordering contract is
            // pinned at the protocol level in replay_marker.rs and,
            // helm-side, in farhelm-helm's client.rs/lib.rs; this test is
            // about the counter reaching `target`, not about the marker.
            Ok(Some(TermEvent::ReplayComplete)) => {}
            Ok(Some(TermEvent::Detached(reason))) => {
                panic!("counter attachment ended before {target}: {reason}")
            }
            Ok(None) => panic!("counter attachment closed before {target}"),
            Err(_) => panic!(
                "counter never reached {target}; last records: {:?}",
                counter_records(&transcript)
                    .into_iter()
                    .rev()
                    .take(5)
                    .collect::<Vec<_>>()
            ),
        }
    }
}

/// The core walking-skeleton path: create a session running the fake
/// agent through the real login-shell + shim launch chain, see its
/// output arrive over the attach stream, and round-trip input. This is
/// PLAN_M1.md acceptance criterion 5's "create, output rendering, input
/// round-trip" at the Rust layer.
#[tokio::test]
async fn create_attach_and_roundtrip_input() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    h.client.send_input(chan, b"hello-farhelm\r".to_vec()).await;
    wait_for(&mut rx, &mut seen, "echo:", 10).await;
    wait_for(&mut rx, &mut seen, "hello-farhelm", 5).await;
}

/// Reconnect-with-replay: detach, reattach, and require the replay to
/// contain output produced before the reattach AND the bracketed-paste
/// mode the fake agent enabled. Mode restoration is the audited
/// silent-loss case (SPEC_impl.md) — content alone passing this test
/// would be the bug.
#[tokio::test]
async fn reattach_replays_history_and_modes() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    h.client
        .send_input(chan, b"before-reattach\r".to_vec())
        .await;
    wait_for(&mut rx, &mut seen, "echo:", 10).await;
    h.client.detach(chan).await;

    let (chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("reattach");
    let mut replay = Vec::new();
    wait_for(&mut rx2, &mut replay, "before-reattach", 10).await;
    // Input-mode restoration follows the content prefill, so wait for it
    // explicitly rather than asserting on a prefix of the replay. On a
    // tmux whose format vocabulary predates `bracket_paste_flag`, that
    // one field degrades to "off" (see PaneModes::parse) and there is
    // nothing to assert — hence the probe rather than an unconditional
    // expectation.
    if tmux_has_format(&h, "bracket_paste_flag").await {
        wait_for(&mut rx2, &mut replay, "\x1b[?2004h", 5).await;
    } else {
        eprintln!("tmux lacks bracket_paste_flag; skipping mode-restoration assertion");
    }
    let replay_text = String::from_utf8_lossy(&replay);
    assert!(
        replay_text.contains("FAKE-AGENT READY"),
        "replay missing pre-detach history"
    );

    // A fresh echo, not just replay: detach-then-reattach is one of the
    // three triggers of the frozen-replay hazard (a control-mode client
    // overlap renders the replay and then never updates), and replay
    // content arrives either way — only new output distinguishes a live
    // terminal from a frozen one.
    h.client
        .send_input(chan2, b"live-after-reattach\r".to_vec())
        .await;
    wait_for(&mut rx2, &mut replay, "echo:", 15).await;
    wait_for(&mut rx2, &mut replay, "live-after-reattach", 10).await;
}

/// Replay and live output meet at one exact tmux command boundary.
///
/// The fixture writes numbered records continuously while this test
/// repeatedly replaces the attachment. Every new transcript must be one
/// consecutive range with no duplicate. Capturing before opening the
/// control client loses records here; enabling a second client before
/// capture duplicates them or triggers the frozen-stream regression.
#[tokio::test]
async fn reattach_cutover_has_no_missing_or_duplicated_output() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script counter"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    let mut target = 100;
    let mut final_channel = None;
    for attempt in 0..8 {
        let (channel, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
        final_channel = Some(channel);
        let transcript = collect_counter_through(&mut rx, target).await;
        let records = counter_records(&transcript);
        let first = *records.first().expect("snapshot contains counter output");
        let last = *records.last().expect("checked above");
        let expected: Vec<u64> = (first..=last).collect();
        assert_eq!(
            records, expected,
            "replay/live cutover {attempt} lost or duplicated a counter record"
        );
        target = last + 40;
    }
    h.client
        .detach(final_channel.expect("at least one attachment"))
        .await;
}

/// Invalid UTF-8 is legitimate terminal output and must cross the live
/// control-mode stream byte-for-byte. Any conversion through `String`
/// would replace 0xff while ordinary TUI tests continued to pass.
#[tokio::test]
async fn non_utf8_terminal_output_survives_live_stream() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script binary"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    let (_channel, mut live) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut live_bytes = Vec::new();
    wait_for(&mut live, &mut live_bytes, "BINARY-MARKER", 20).await;
    assert!(
        live_bytes.contains(&0xff),
        "live output replaced or dropped the invalid byte: {live_bytes:?}"
    );
}

/// Last attach wins (SPEC.md): a second attach visibly detaches the
/// first — the old stream gets a Detached event, and input keeps working
/// on the new attachment.
#[tokio::test]
async fn second_attach_detaches_first() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (_c1, mut rx1) = h.client.attach(&session.id, 80, 24).await.expect("attach1");
    let mut seen1 = Vec::new();
    wait_for(&mut rx1, &mut seen1, "FAKE-AGENT READY", 20).await;

    let (c2, mut rx2) = h.client.attach(&session.id, 80, 24).await.expect("attach2");

    // First attachment must observe its own takeover.
    let deadline = Duration::from_secs(10);
    let detached = tokio::time::timeout(deadline, async {
        while let Some(ev) = rx1.recv().await {
            if let TermEvent::Detached(reason) = ev {
                return reason;
            }
        }
        panic!("first attachment stream ended without Detached");
    })
    .await
    .expect("timed out waiting for Detached on first attachment");
    assert!(detached.contains("another client"));

    // Second attachment is live.
    let mut seen2 = Vec::new();
    wait_for(&mut rx2, &mut seen2, "FAKE-AGENT READY", 10).await;
    h.client.send_input(c2, b"still-alive\r".to_vec()).await;
    wait_for(&mut rx2, &mut seen2, "still-alive", 10).await;
}

/// Two DISTINCT leases are two clients, so the second attach takes the
/// first over — SPEC.md's one-attached-client rule, now enforced by lease
/// identity rather than by "any second attach wins" (PLAN_M4.md item 3).
///
/// The loser must learn about it (the takeover reason on its own channel)
/// AND stop being able to type: a takeover that detached the stream but
/// left the input route live would leave a kicked client executing
/// commands in the winner's agent terminal. Both halves are asserted
/// because the lease check is what decides the first half and the
/// per-terminal cutover is what decides the second — a lease sweep that
/// forgot to remove the attachment would still send the notice.
#[tokio::test]
async fn an_attach_under_a_different_lease_takes_over_and_silences_the_loser() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (loser_chan, mut rx1) = h
        .client
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, "lease-one")
        .await
        .expect("attach1");
    let mut seen1 = Vec::new();
    wait_for(&mut rx1, &mut seen1, "FAKE-AGENT READY", 20).await;

    let (winner_chan, mut rx2) = h
        .client
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, "lease-two")
        .await
        .expect("attach2");
    let reason = expect_detached(&mut rx1, 10).await;
    assert!(
        reason.contains("another client"),
        "the loser must be told it was taken over, got: {reason}"
    );

    let mut seen2 = Vec::new();
    wait_for(&mut rx2, &mut seen2, "FAKE-AGENT READY", 15).await;
    // Ghost then marker on the SAME connection, so the supervisor has
    // decided the ghost's fate by the time the marker echoes back (the
    // ordering trick `kicked_client_cannot_still_send_input` uses).
    h.client
        .send_input(loser_chan, b"ghost-lease\r".to_vec())
        .await;
    h.client
        .send_input(winner_chan, b"marker-lease\r".to_vec())
        .await;
    wait_for(&mut rx2, &mut seen2, "marker-lease", 15).await;
    let transcript = String::from_utf8_lossy(&seen2);
    assert!(
        !transcript.contains("ghost-lease"),
        "input from a lease that lost the takeover reached the pane:\n{transcript}"
    );
}

/// A NON-DISPLACING attach (`ControlMsg::Attach::if_unowned`) is refused
/// while another lease holds the session, and refuses without touching
/// anything — PLAN_M6.md item 7's auto-reconnect safety property, at the
/// layer that enforces it.
///
/// The failure it exists to prevent is an eviction with nobody behind it.
/// A client recovering from transport loss has no socket, so a takeover
/// that happens while it is away reaches it nowhere; its next automatic
/// attach carries the same lease it always had, and a displacing one would
/// take the session back from whoever legitimately holds it now. Nobody
/// asked for that, and the displaced winner would learn about it as its own
/// terminal going dead.
///
/// Both halves are asserted because either alone is a different bug. The
/// refusal must be a CONFLICT carrying the takeover wording — that exact
/// string is what lets a browser render the refusal as the takeover it is
/// rather than as a mysterious error. And the incumbent must still be
/// LIVE afterwards: a refusal that swept first and refused second would
/// leave the winner detached by an attach that was itself rejected, which
/// is strictly worse than the displacement it was meant to prevent.
#[tokio::test]
async fn an_unattended_attach_is_refused_while_another_lease_holds_the_session() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (owner_chan, mut owner_rx) = h
        .client
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, "lease-owner")
        .await
        .expect("the owner attaches normally");
    let mut seen = Vec::new();
    wait_for(&mut owner_rx, &mut seen, "FAKE-AGENT READY", 20).await;

    let refused = h
        .client
        .attach_terminal_if_unowned(
            &session.id,
            80,
            24,
            TerminalSelector::Agent,
            "lease-latecomer",
        )
        .await
        .expect_err("an unattended attach must not take a session someone else holds");
    let supervised = refused
        .downcast_ref::<farhelm_helm::SupervisorError>()
        .expect("the refusal is the supervisor's own error");
    assert_eq!(
        supervised.kind,
        farhelm_proto::ErrorKind::Conflict,
        "a session held by someone else is a conflict, not a bad request"
    );
    assert_eq!(
        supervised.message,
        farhelm_proto::ATTACH_REFUSED_TAKEN_OVER,
        "the refusal must carry the takeover wording verbatim: a browser matches on it to decide \
         it lost the session"
    );

    // The owner never noticed: no detach notice, and input still lands.
    h.client
        .send_input(owner_chan, b"still-mine\r".to_vec())
        .await;
    wait_for(&mut owner_rx, &mut seen, "still-mine", 15).await;

    // And the same client CAN still take the session deliberately — the
    // refusal is about unattended attaches, not about this lease.
    let (_taken, mut taken_rx) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Agent,
            "lease-latecomer",
        )
        .await
        .expect("a displacing attach is unaffected");
    let reason = expect_detached(&mut owner_rx, 10).await;
    assert!(
        reason.contains("another client"),
        "the deliberate takeover still displaces, got: {reason}"
    );
    let mut seen_taken = Vec::new();
    wait_for(&mut taken_rx, &mut seen_taken, "FAKE-AGENT READY", 15).await;
}

/// The SAME lease reattaching to the SAME terminal is an ordinary
/// reconnect: the incumbent channel is still cut over, but it is told a
/// REPLACED reason rather than a takeover one.
///
/// Two failures in one test. The mechanism could plausibly go wrong in
/// the "helpful" direction — recognizing the incumbent as the same client
/// and leaving it in place would give one terminal two live forwarders,
/// the overlapping-control-client state the whole attach path exists to
/// avoid — so the cutover and its replay must still happen. And the
/// REASON must not be the takeover string: equal non-empty leases are one
/// client reconnecting (`ControlMsg::Attach`'s contract), so "another
/// client attached" would raise a takeover banner accusing a second user
/// who does not exist. A client that renders detach reasons verbatim
/// makes that difference visible to the user, which is why it is pinned
/// here rather than left to the supervisor's internal accounting.
#[tokio::test]
async fn a_same_lease_reattach_to_the_same_terminal_is_an_ordinary_cutover() {
    const LEASE: &str = "one-client-reconnecting";
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (chan1, mut rx1) = h
        .client
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, LEASE)
        .await
        .expect("attach");
    let mut seen1 = Vec::new();
    wait_for(&mut rx1, &mut seen1, "FAKE-AGENT READY", 20).await;
    h.client
        .send_input(chan1, b"before-same-lease\r".to_vec())
        .await;
    wait_for(&mut rx1, &mut seen1, "before-same-lease", 15).await;

    let (chan2, mut rx2) = h
        .client
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, LEASE)
        .await
        .expect("reattach");
    let reason = expect_detached(&mut rx1, 10).await;
    assert!(
        reason.contains("replaced by a newer attachment"),
        "a same-lease reattach must tell the incumbent it was replaced, got: {reason}"
    );
    assert!(
        !reason.contains("another client"),
        "a client reconnecting under its own lease must never be told another client took \
         over, got: {reason}"
    );

    // Replay, then live: exactly what a reconnect promises.
    let mut seen2 = Vec::new();
    wait_for(&mut rx2, &mut seen2, "before-same-lease", 20).await;
    h.client
        .send_input(chan2, b"after-same-lease\r".to_vec())
        .await;
    wait_for(&mut rx2, &mut seen2, "after-same-lease", 15).await;
}

/// The empty lease is not a lease: an un-leased attach takes over a
/// leased one, and a leased attach takes over an un-leased one.
///
/// Both directions in one test because they are one rule — the empty
/// lease matches nothing, not even another empty lease — and it is the
/// entire compatibility story for every pre-M4 client (and for the helm,
/// which sends no lease until PLAN_M4.md item 5). Get it wrong by
/// treating empty as a shared identity and two unrelated legacy clients
/// silently share a session; get it wrong in the other direction and a
/// legacy client can never reclaim a session a leased client holds.
#[tokio::test]
async fn the_empty_lease_takes_over_everything_and_is_taken_over_by_anything() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    // Leased incumbent, un-leased newcomer.
    let (_leased_chan, mut leased_rx) = h
        .client
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, "held-lease")
        .await
        .expect("leased attach");
    let mut leased_seen = Vec::new();
    wait_for(&mut leased_rx, &mut leased_seen, "FAKE-AGENT READY", 20).await;

    let (_legacy_chan, mut legacy_rx) = h.client.attach(&session.id, 80, 24).await.expect("legacy");
    let reason = expect_detached(&mut leased_rx, 10).await;
    assert!(
        reason.contains("another client"),
        "an un-leased attach must take over a leased holder, got: {reason}"
    );
    let mut legacy_seen = Vec::new();
    wait_for(&mut legacy_rx, &mut legacy_seen, "FAKE-AGENT READY", 15).await;

    // Un-leased incumbent, leased newcomer.
    let (_new_chan, mut new_rx) = h
        .client
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, "fresh-lease")
        .await
        .expect("leased reattach");
    let reason = expect_detached(&mut legacy_rx, 10).await;
    assert!(
        reason.contains("another client"),
        "a leased attach must take over an un-leased holder, got: {reason}"
    );
    let mut new_seen = Vec::new();
    wait_for(&mut new_rx, &mut new_seen, "FAKE-AGENT READY", 15).await;
}

/// An over-cap lease is refused as a bad REQUEST, and refused before the
/// attach has taken anything over.
///
/// The lease is retained for the life of every attachment made under it,
/// so an unbounded one is retained memory a client can mint from a single
/// oversized control frame — the reason the cap exists at all. Both
/// halves matter: the refusal itself, and its placement ahead of the
/// takeover, because a check that ran after the lease sweep would let any
/// client detach any other by sending garbage it knows will be rejected.
#[tokio::test]
async fn an_over_cap_lease_is_refused_without_disturbing_the_incumbent() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (holder_chan, mut holder_rx) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Agent,
            "holding-lease",
        )
        .await
        .expect("attach the agent terminal");
    let mut holder_seen = Vec::new();
    wait_for(&mut holder_rx, &mut holder_seen, "FAKE-AGENT READY", 20).await;

    // One byte over: the cap is 128, and a request that is refused must
    // be refused for its size alone, not for anything else about it.
    let over_cap = "x".repeat(129);
    let err = h
        .client
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, &over_cap)
        .await
        .expect_err("an over-cap lease must be refused");
    assert!(
        err.to_string().contains("lease"),
        "the error must say which field was too big, got: {err:#}"
    );
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("an over-cap lease must carry a SupervisorError")
            .kind,
        ErrorKind::InvalidRequest,
        "an over-cap lease is a malformed request, not a not-found or a server fault"
    );

    // The incumbent is untouched: no detach notice, and still typing.
    let detached = drain_for(&mut holder_rx, &mut holder_seen, Duration::from_secs(1)).await;
    assert_eq!(
        detached, None,
        "a refused over-cap attach detached the session's live attachment"
    );
    h.client
        .send_input(holder_chan, b"survived-the-lease\r".to_vec())
        .await;
    wait_for(&mut holder_rx, &mut holder_seen, "survived-the-lease", 15).await;
}

/// The lease cap counts BYTES, not characters, and admits a lease that
/// sits exactly on it.
///
/// What is bounded is retained memory and frame content, both of which
/// are byte quantities — so a `chars().count()` cap would let a
/// multibyte lease carry several times the memory the cap names. The
/// exact-cap case is the other half of the same boundary: an off-by-one
/// that refused it would break any client that sizes its ids to the
/// documented limit.
#[tokio::test]
async fn the_lease_cap_counts_bytes_not_characters() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let exactly_at_cap = "x".repeat(128);
    let (_chan, mut rx) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Agent,
            &exactly_at_cap,
        )
        .await
        .expect("a lease exactly at the cap must be accepted");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    // 64 two-byte characters: 128 bytes, right on the cap.
    let multibyte_at_cap = "é".repeat(64);
    assert_eq!(multibyte_at_cap.len(), 128, "test fixture is 128 bytes");
    let (_chan2, mut rx2) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Agent,
            &multibyte_at_cap,
        )
        .await
        .expect("a multibyte lease at the byte cap must be accepted");
    let mut seen2 = Vec::new();
    wait_for(&mut rx2, &mut seen2, "FAKE-AGENT READY", 15).await;

    // 65 of them: 65 characters — comfortably under any character-count
    // reading of the cap — but 130 bytes, which is over it.
    let multibyte_over_cap = "é".repeat(65);
    assert_eq!(multibyte_over_cap.chars().count(), 65);
    assert_eq!(multibyte_over_cap.len(), 130);
    let err = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Agent,
            &multibyte_over_cap,
        )
        .await
        .expect_err("a lease over the BYTE cap must be refused even when few characters");
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("an over-cap lease must carry a SupervisorError")
            .kind,
        ErrorKind::InvalidRequest,
        "the cap must be counted in bytes, not characters"
    );
}

/// Attaching a terminal tab is a `NotFound` that names the tab, because
/// no supervisor serves tabs yet (PLAN_M4.md item 2 is the next PR).
///
/// The alternative a selector-shaped attach path could drift into is
/// silently falling back to the agent terminal, which `TerminalSelector`
/// explicitly forbids: attaching the WRONG terminal is worse than
/// failing.
///
/// The refusal must also be free of SIDE EFFECTS, which the incumbent
/// under a different lease pins: terminal resolution happens before the
/// takeover, so an attach nobody can honor must never cost the session's
/// current client its attachment. Get that order wrong and any client
/// could detach any other by naming a tab that does not exist.
#[tokio::test]
async fn attaching_a_terminal_tab_is_a_not_found_that_names_the_tab() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (holder_chan, mut holder_rx) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Agent,
            "holding-lease",
        )
        .await
        .expect("attach the agent terminal");
    let mut holder_seen = Vec::new();
    wait_for(&mut holder_rx, &mut holder_seen, "FAKE-AGENT READY", 20).await;

    let err = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab {
                id: "tab-does-not-exist".to_string(),
            },
            "intruding-lease",
        )
        .await
        .expect_err("attaching a tab must fail while no tabs exist");
    assert!(
        err.to_string().contains("tab-does-not-exist"),
        "the error must name the tab that could not be found, got: {err:#}"
    );
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("a tab attach must carry a SupervisorError")
            .kind,
        ErrorKind::NotFound,
        "a terminal that does not exist is a not-found, not a bad request or a server fault"
    );

    // The incumbent is untouched: no detach notice, and still typing.
    let detached = drain_for(&mut holder_rx, &mut holder_seen, Duration::from_secs(1)).await;
    assert_eq!(
        detached, None,
        "a refused tab attach detached the session's live attachment"
    );
    h.client
        .send_input(holder_chan, b"survived-the-tab\r".to_vec())
        .await;
    wait_for(&mut holder_rx, &mut holder_seen, "survived-the-tab", 15).await;
}

/// Attachment channels are connection-local routing keys, so zero and
/// reuse are protocol errors rather than harmless client choices.
///
/// Reusing a live channel previously overwrote its input route while two
/// forwarders emitted onto the same data channel. The raw client is
/// intentional: `SupervisorClient` normally allocates unique channels
/// and cannot express the hostile protocol sequence this validates.
#[tokio::test]
async fn attachment_channels_must_be_nonzero_and_unique() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let (client_side, server_side) = tokio::io::duplex(1 << 20);
    let sup = Arc::clone(&h.sup);
    tokio::spawn(async move {
        let _ = handle_connection(sup, server_side).await;
    });
    let (read_half, write_half) = tokio::io::split(client_side);
    let mut reader = FrameReader::new(read_half);
    let mut writer = FrameWriter::new(write_half);
    handshake(&mut reader, &mut writer, "helm")
        .await
        .expect("handshake");

    writer
        .write_control(&ControlMsg::Attach {
            req_id: 1,
            session_id: session.id.clone(),
            channel: 0,
            cols: 80,
            rows: 24,
            // Vocabulary only for now: this test predates tabs/leases
            // (PLAN_M4.md step 4) and is only exercising the channel-0/
            // channel-reuse rejection paths, so the agent terminal with
            // no lease — today's only meaning — is exactly what belongs
            // here.
            terminal: TerminalSelector::default(),
            lease: String::new(),
            if_unowned: false,
        })
        .await
        .unwrap();
    assert!(matches!(
        parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap(),
        ControlMsg::Error {
            req_id: 1,
            kind: ErrorKind::InvalidRequest,
            ..
        }
    ));

    writer
        .write_control(&ControlMsg::Attach {
            req_id: 2,
            session_id: session.id.clone(),
            channel: 7,
            cols: 80,
            rows: 24,
            terminal: TerminalSelector::default(),
            lease: String::new(),
            if_unowned: false,
        })
        .await
        .unwrap();
    assert!(matches!(
        parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap(),
        ControlMsg::Attached {
            req_id: 2,
            channel: 7
        }
    ));

    writer
        .write_control(&ControlMsg::Attach {
            req_id: 3,
            session_id: session.id,
            channel: 7,
            cols: 80,
            rows: 24,
            terminal: TerminalSelector::default(),
            lease: String::new(),
            if_unowned: false,
        })
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let frame = reader.read_frame().await.unwrap().unwrap();
            if frame.kind != FrameKind::Control {
                continue;
            }
            match parse_control(&frame).unwrap() {
                ControlMsg::Error {
                    req_id: 3,
                    kind: ErrorKind::InvalidRequest,
                    ..
                } => break,
                ControlMsg::Attached { req_id: 3, .. } => {
                    panic!("duplicate attachment channel was accepted")
                }
                _ => {}
            }
        }
    })
    .await
    .expect("supervisor did not reject duplicate attachment channel");
}

/// A peer speaking the LAST protocol version before the non-displacing
/// attach is refused at the handshake, so its `Attach` never reaches the
/// handler at all.
///
/// This is the machinery `PROTOCOL_VERSION`'s bump to 9 exists to engage,
/// pinned at the layer where the engaging happens. `if_unowned` is
/// decode-additive — a version-8 supervisor drops it without complaint and
/// performs the DISPLACING attach the caller explicitly asked it not to —
/// and a silent wrong answer is exactly what SPEC.md's version rule
/// forbids ("Incompatible versions refuse to connect with a clear,
/// actionable error; there is no silent degradation"). Since the old
/// binary cannot be recompiled to prove that here, this proves the thing
/// that MAKES it unreachable: an old-version peer is refused before it can
/// send anything, with a diagnostic naming both versions.
///
/// The refusal is asserted to arrive as an unsolicited `Error` and to be
/// followed by teardown, because both halves matter to the fleet story: a
/// helm reads that message into its host's `version-skew` state (with the
/// remediation the panel renders), and the teardown is what stops the
/// connection from limping on in a state where attaches would be honored
/// with the wrong semantics.
#[tokio::test]
async fn a_peer_one_protocol_version_behind_is_refused_before_it_can_attach() {
    let h = harness().await;
    let (client_side, server_side) = tokio::io::duplex(1 << 20);
    let sup = Arc::clone(&h.sup);
    tokio::spawn(async move {
        let _ = handle_connection(sup, server_side).await;
    });
    let (read_half, write_half) = tokio::io::split(client_side);
    let mut reader = FrameReader::new(read_half);
    let mut writer = FrameWriter::new(write_half);

    // A hello from the version this milestone's field was added AFTER —
    // the exact peer that would ignore `if_unowned` and displace anyway.
    writer
        .write_control(&ControlMsg::Hello {
            protocol_version: farhelm_proto::PROTOCOL_VERSION - 1,
            build_version: "0.0.0-before-if-unowned".to_string(),
            role: "helm".to_string(),
            host_identity: None,
            auth: None,
        })
        .await
        .unwrap();

    let _their_hello = reader.read_frame().await.unwrap().unwrap();
    let refusal =
        farhelm_proto::io::parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
    let ControlMsg::Error { message, kind, .. } = refusal else {
        panic!("a version-skewed peer must be refused, got {refusal:?}");
    };
    assert_eq!(kind, farhelm_proto::ErrorKind::Internal);
    // BOTH numbers, not just the shape of the complaint. Which side is
    // behind is the first thing anyone reading this asks, and it is what
    // the host's version-skew chip is built from — a diagnostic naming
    // only one version cannot answer it, and a test asserting only a
    // generic phrase would not notice if one went missing.
    assert!(
        message.contains("protocol version mismatch")
            && message.contains(&format!("v{}", farhelm_proto::PROTOCOL_VERSION - 1))
            && message.contains(&format!("v{}", farhelm_proto::PROTOCOL_VERSION))
            && message.contains("0.0.0-before-if-unowned"),
        "the refusal must name both protocol versions and the peer's build, so a host can render \
         it with a remedy: {message}"
    );

    // And the connection is gone: an attach sent after the refusal is not
    // answered, which is what makes the refusal a gate rather than a
    // warning.
    let _ = writer
        .write_control(&ControlMsg::Attach {
            req_id: 1,
            session_id: "whatever".to_string(),
            channel: 7,
            cols: 80,
            rows: 24,
            terminal: TerminalSelector::Agent,
            lease: "lease-from-an-old-peer".to_string(),
            if_unowned: true,
        })
        .await;
    let outcome = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while let Ok(Some(frame)) = reader.read_frame().await {
            let msg = farhelm_proto::io::parse_control(&frame).unwrap();
            assert!(
                !matches!(msg, ControlMsg::Attached { .. }),
                "a refused peer must never get an attachment"
            );
        }
    })
    .await;
    outcome.expect("a version-refused connection must be torn down, not left serving");
}

/// An unknown control-message tag tears down the whole connection — the
/// loop-level half of the contract whose parse-layer half lives in the
/// proto crate (`unknown_control_message_tag_fails_decode`). This is the
/// behavior that forced PLAN_M2_5.md's `PROTOCOL_VERSION` bump to 4: new
/// `ControlMsg` variants are not additive, so a peer speaking a newer
/// message set must be kept out by the version handshake, because once
/// past it a single unknown message kills the connection. Pinning the
/// teardown here means a later refactor that catches and swallows the
/// parse error inside the connection loop — silently converting
/// "connection-fatal" into "ignored", and with it invalidating the whole
/// version-bump rationale — fails a test instead of going unnoticed.
#[tokio::test]
async fn unknown_control_message_tears_down_the_connection() {
    let h = harness().await;
    let (client_side, server_side) = tokio::io::duplex(1 << 20);
    let sup = Arc::clone(&h.sup);
    tokio::spawn(async move {
        let _ = handle_connection(sup, server_side).await;
    });
    let (read_half, write_half) = tokio::io::split(client_side);
    let mut reader = FrameReader::new(read_half);
    let mut writer = FrameWriter::new(write_half);
    handshake(&mut reader, &mut writer, "helm")
        .await
        .expect("handshake");

    writer
        .write_frame(&Frame {
            kind: FrameKind::Control,
            channel: 0,
            body: br#"{"type":"message_from_the_future"}"#.to_vec(),
        })
        .await
        .unwrap();

    // The connection must die: the reader sees EOF or an error, never a
    // reply, and never a silently-continuing session. A tolerant loop
    // would leave the stream open and this read hanging, so the timeout
    // is the failure detector for that regression.
    let outcome = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            match reader.read_frame().await {
                Ok(Some(_)) => continue, // drain any in-flight frame
                Ok(None) => break,       // clean shutdown: connection torn down
                Err(_) => break,         // error shutdown: equally torn down
            }
        }
    })
    .await;
    outcome.expect("connection must be torn down after an unknown control message, not left open");
}

/// Precondition failures fail the create with a visible error and no
/// session (SPEC.md's creation-failure split).
///
/// The in-memory check alone only proves this process's own map stayed
/// empty; it says nothing about whether a row still landed in SQLite
/// despite the rejection (this validation runs before `create_session`
/// ever touches tmux or the store, so today it cannot, but a future
/// reordering could reintroduce exactly that gap silently). Constructing
/// a second, independent `Supervisor` on the same state dir and listing
/// through IT is what actually proves nothing was persisted — a row
/// present only in SQLite, invisible to the original process's map, would
/// still surface here.
#[tokio::test]
async fn create_in_missing_directory_errors() {
    let h = harness().await;
    let err = h
        .client
        .create_session("/nonexistent/definitely/not/here", "true", None, 80, 24)
        .await
        .expect_err("create should fail");
    assert!(err.to_string().contains("working directory does not exist"));
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("a bad-cwd failure must carry a SupervisorError")
            .kind,
        ErrorKind::InvalidRequest,
        "a missing directory is the caller's mistake, not a server fault"
    );
    assert!(h.client.list_sessions().await.unwrap().sessions.is_empty());

    let sup2 = Supervisor::new_with_exe(h.state.path(), farhelm_bin().into())
        .await
        .expect("second supervisor construction reading the same state dir");
    let client2 = connect_client(&sup2).await;
    assert!(
        client2.list_sessions().await.unwrap().sessions.is_empty(),
        "a rejected create must not have persisted a row visible to a fresh supervisor"
    );
}

/// A relative cwd must be refused at create time, not merely mis-resolved
/// later.
///
/// tmux resolves a relative working directory against the SUPERVISOR
/// DAEMON's own cwd, not the client's — so accepting one here would store
/// a path whose meaning depends on wherever the daemon happened to be
/// started, and would shift again on every daemon restart (manually
/// reproduced: a session created this way either fails to restart with
/// "working directory does not exist", or — if a same-named directory
/// happens to exist relative to the daemon's new cwd — silently
/// relaunches the agent in the wrong directory). Refusing it up front in
/// `ensure_cwd_usable`, shared by create and restart, closes the create
/// path and also makes a pre-existing stored relative cwd refuse to
/// restart with a clear error instead of mis-resolving.
#[tokio::test]
async fn create_with_relative_cwd_is_rejected() {
    let h = harness().await;
    let err = h
        .client
        .create_session("crates", "true", None, 80, 24)
        .await
        .expect_err("create should reject a relative cwd");
    let message = err.to_string();
    assert!(
        message.contains("crates") && message.contains("absolute"),
        "the refusal must name the offending path and explain the absoluteness requirement: {message}"
    );
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("a relative-cwd failure must carry a SupervisorError")
            .kind,
        ErrorKind::InvalidRequest,
        "a relative cwd is the caller's mistake, not a server fault"
    );
    assert!(
        h.client.list_sessions().await.unwrap().sessions.is_empty(),
        "a rejected create must not have created a session (in-memory or tmux)"
    );
    // The in-memory check above only proves the supervisor's own bookkeeping
    // saw nothing; a rejected create could in principle still have raced a
    // tmux `new-session` before validation ran. Probe the private socket
    // directly. No prior test in this harness created a tmux session, so
    // the server may not even be running yet — that absence itself proves
    // there is no session, the same shape `harness::kill_tmux_server_and_wait`
    // relies on.
    let probe = tmux_query(&h.state.path().join("tmux.sock"), &["list-sessions"]).await;
    if probe.status.success() {
        assert!(
            String::from_utf8_lossy(&probe.stdout).trim().is_empty(),
            "a rejected create must not have left a tmux session behind"
        );
    } else {
        assert!(
            String::from_utf8_lossy(&probe.stderr).contains("no server running"),
            "tmux list-sessions failed for a reason other than an absent server: {}",
            String::from_utf8_lossy(&probe.stderr)
        );
    }
}

/// A `~`-prefixed cwd expands against the SUPERVISOR's home, the session
/// DURABLY stores the expanded absolute path, and everything derived from
/// the cwd derives from the expanded form (BUGS_BURNDOWN.md issue 1).
///
/// The durable form matters as much as the acceptance: `~` is only safe
/// because expansion happens once, at create, against a home resolved at
/// supervisor construction — a stored literal `~` would re-resolve on
/// every restart and reintroduce exactly the drift the absolute-path rule
/// exists to prevent. The reply's `cwd` alone cannot prove that (reply
/// and row are built from the same in-memory value), so a SECOND
/// supervisor is constructed over the same state dir and asked — a row
/// holding `~` would surface here. The default titles pin the derivation
/// point: a title of `ws` (not `~`) means the basename came off the
/// expanded path. The home comes from the `user_home` seam because this
/// repo's tests never mutate the test process's environment.
#[tokio::test]
async fn create_with_tilde_cwd_expands_against_the_supervisors_home() {
    let home = tempfile::tempdir().unwrap();
    let workdir = home.path().join("ws");
    std::fs::create_dir(&workdir).unwrap();
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            user_home: Some(home.path().to_path_buf()),
            ..SupervisorSeams::default()
        },
    )
    .await;

    // The invocation writes its own $PWD: metadata alone cannot prove the
    // LAUNCH happened in the expanded directory (the row could be right
    // while tmux was handed something else), but a marker file appearing
    // inside the expanded path can.
    let subdir = h
        .client
        .create_session("~/ws", "sh -c 'pwd > where-i-ran.txt'", None, 80, 24)
        .await
        .expect("a ~/path cwd should be accepted and expanded");
    assert_eq!(
        subdir.cwd,
        workdir.to_string_lossy(),
        "the stored cwd must be the expanded absolute path, not the literal ~ form"
    );
    assert_eq!(
        subdir.title, "ws",
        "the default title derives from the EXPANDED path's basename"
    );
    let marker = workdir.join("where-i-ran.txt");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        if let Ok(contents) = std::fs::read_to_string(&marker) {
            assert_eq!(
                contents.trim(),
                workdir.to_string_lossy(),
                "the agent's own $PWD must be the expanded directory"
            );
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the launch never ran in the expanded directory (no marker at {marker:?})"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let bare = h
        .client
        .create_session("~", "true", None, 80, 24)
        .await
        .expect("a bare ~ cwd should be accepted and expanded");
    assert_eq!(
        bare.cwd,
        home.path().to_string_lossy(),
        "a bare ~ is the home directory itself"
    );
    assert_eq!(
        bare.title,
        home.path()
            .file_name()
            .expect("a tempdir has a basename")
            .to_string_lossy(),
        "a bare ~ create's default title is the home directory's basename, not ~"
    );

    let sup2 = Supervisor::new_with_exe(h.state.path(), farhelm_bin().into())
        .await
        .expect("second supervisor construction reading the same state dir");
    let client2 = connect_client(&sup2).await;
    let listed = client2.list_sessions().await.unwrap().sessions;
    let stored = listed
        .iter()
        .find(|s| s.id == subdir.id)
        .expect("the ~/ws session is durably present");
    assert_eq!(
        stored.cwd,
        workdir.to_string_lossy(),
        "a fresh supervisor must see the expanded path — the durable row, not the reply"
    );
}

/// A `~user` cwd is refused with a message naming the supported forms —
/// not mangled into a bogus expansion, and not the generic "not absolute"
/// refusal that would send the user hunting for a typo.
#[tokio::test]
async fn create_with_tilde_user_cwd_is_rejected() {
    let h = harness().await;
    let err = h
        .client
        .create_session("~other/ws", "true", None, 80, 24)
        .await
        .expect_err("a ~user cwd must be refused");
    let message = err.to_string();
    assert!(
        message.contains("~other") && message.contains("~/path"),
        "the refusal must name the offending path and the supported forms: {message}"
    );
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("a ~user refusal must carry a SupervisorError")
            .kind,
        ErrorKind::InvalidRequest,
        "a ~user cwd is the caller's mistake, not a server fault"
    );
    assert!(
        h.client.list_sessions().await.unwrap().sessions.is_empty(),
        "a rejected create must not have created a session"
    );
}

/// An existing file is a different caller error from a missing path.
/// Keeping that distinction visible prevents a correct path from being
/// misdiagnosed as a typo.
#[tokio::test]
async fn create_in_a_regular_file_reports_not_a_directory() {
    let h = harness().await;
    let file = tempfile::NamedTempFile::new().unwrap();
    let err = h
        .client
        .create_session(&file.path().to_string_lossy(), "true", None, 80, 24)
        .await
        .expect_err("create should reject a regular file as cwd");
    assert!(err.to_string().contains("is not a directory"));
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("a not-a-directory failure must carry a SupervisorError")
            .kind,
        ErrorKind::InvalidRequest,
        "cwd being a file is the caller's mistake, not a server fault"
    );
    assert!(h.client.list_sessions().await.unwrap().sessions.is_empty());
}

/// A cwd nested UNDER a regular file (`/tmp/somefile/child`) is a
/// different OS error than either "missing" or "cwd itself is a file": the
/// non-final path component being a file surfaces as
/// `io::ErrorKind::NotADirectory`, not `NotFound`. Still the caller's
/// mistake — a typo'd path segment, most likely — so it must classify the
/// same way as the sibling cases above, not fall through to the
/// catch-all `Internal` default.
#[tokio::test]
async fn create_under_a_regular_file_is_invalid_request() {
    let h = harness().await;
    let file = tempfile::NamedTempFile::new().unwrap();
    let nested = file.path().join("child");
    let err = h
        .client
        .create_session(&nested.to_string_lossy(), "true", None, 80, 24)
        .await
        .expect_err("create should reject a cwd nested under a regular file");
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("a not-a-directory failure must carry a SupervisorError")
            .kind,
        ErrorKind::InvalidRequest,
        "a path nested under a file is the caller's mistake, not a server fault"
    );
    assert!(h.client.list_sessions().await.unwrap().sessions.is_empty());
}

/// A NUL byte in the cwd text cannot address anything on a POSIX
/// filesystem; the OS rejects it before `create_session` ever reaches a
/// syscall that could distinguish "missing" from "exists". This surfaces
/// as `io::ErrorKind::InvalidInput`, the same caller-fault bucket as the
/// other malformed-path cases.
#[tokio::test]
async fn create_with_nul_byte_in_cwd_is_invalid_request() {
    let h = harness().await;
    let cwd = "/tmp/has-a-\u{0}-nul-byte";
    let err = h
        .client
        .create_session(cwd, "true", None, 80, 24)
        .await
        .expect_err("create should reject a cwd containing a NUL byte");
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("an invalid-cwd-text failure must carry a SupervisorError")
            .kind,
        ErrorKind::InvalidRequest,
        "a NUL byte in the cwd is the caller's mistake, not a server fault"
    );
    assert!(h.client.list_sessions().await.unwrap().sessions.is_empty());
}

/// The helm must claim its HTTP port before it touches anything else.
///
/// A busy port is the likely startup failure — another helm is already
/// running — and it is retryable, so it has to happen before any durable
/// side effect. This test's subject moved with PLAN_M6.md item 5: the argv
/// startup session it used to guard is gone, and what a premature failure
/// would now strand is helm.db and whatever `--ensure-hosts` was about to
/// register into it. The ordering rule is the same one, and it is asserted
/// against the strongest observable there is — the database file must not
/// exist at all.
#[tokio::test]
async fn helm_bind_failure_happens_before_any_durable_setup() {
    let state = tempfile::tempdir().expect("state");
    let ensure = state.path().join("ensure-hosts.json5");
    tokio::fs::write(&ensure, r#"{ hosts: [{ ssh: "user@never-registered" }] }"#)
        .await
        .expect("write the ensure-hosts file");
    let occupied = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("reserve loopback port");
    let port = occupied.local_addr().unwrap().port();

    let output = tokio::time::timeout(
        Duration::from_secs(10),
        tokio::process::Command::new(farhelm_bin())
            .args(["helm", "run", "--state-dir"])
            .arg(state.path())
            .arg("--port")
            .arg(port.to_string())
            .arg("--ensure-hosts")
            .arg(&ensure)
            .output(),
    )
    .await
    .expect("helm did not fail promptly on occupied port")
    .expect("run helm");
    assert!(!output.status.success(), "occupied port must fail startup");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("binding"),
        "bind failure should retain its context: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !state.path().join("helm.db").exists(),
        "the bind must fail before helm.db is created, and therefore before --ensure-hosts \
         registers anything into it"
    );
}

/// Replay must reach into scrollback, not just the visible screen.
///
/// This is the test that would fail if `capture-pane`'s `-S` history
/// range were dropped: the earlier assertions all fit inside one 24-row
/// viewport, so a screen-only capture would pass them while silently
/// violating SPEC.md's replay floor.
#[tokio::test]
async fn reattach_replays_content_scrolled_off_screen() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    // 80 lines against a 24-row window: spam-line-1 is far off screen by
    // the time spam-line-80 lands.
    h.client.send_input(chan, b"spam 80\r".to_vec()).await;
    wait_for(&mut rx, &mut seen, "spam-line-80", 15).await;
    h.client.detach(chan).await;

    let (_chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("reattach");
    let mut replay = Vec::new();
    wait_for(&mut rx2, &mut replay, "spam-line-1", 15).await;
}

/// Reattaching to a full-screen (alternate-screen) app must show the
/// app, not a blank screen.
///
/// The failure this pins is subtle and was live: `\x1b[?1049h` switches
/// to a *cleared* alternate buffer, so emitting it after the content
/// prefill erases the replay. Ordering is the whole point, which is why
/// the assertion checks the switch precedes the content.
#[tokio::test]
async fn reattach_to_alt_screen_app_preserves_content() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();

    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script altscreen"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "ALT-SCREEN APP", 20).await;
    h.client.detach(chan).await;

    let (_chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("reattach");
    let mut replay = Vec::new();
    wait_for(&mut rx2, &mut replay, "ALT-SCREEN APP", 15).await;
    let text = String::from_utf8_lossy(&replay);
    let switch = text
        .find("\x1b[?1049h")
        .expect("replay must re-enter the alternate screen");
    let content = text.find("ALT-SCREEN APP").expect("checked above");
    assert!(
        switch < content,
        "alt-screen switch must precede replayed content, else it clears it"
    );
}

/// Resize must reach tmux, not merely leave the terminal usable.
///
/// Asserting "typing still works after a resize" would pass even if
/// every resize message were dropped; this checks the window geometry
/// tmux actually holds.
#[tokio::test]
async fn resize_reaches_tmux() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    h.client.resize(&session.id, chan, 100, 30).await;
    wait_for_geometry(&h, "100x30").await;
}

/// Attach-time resize must happen before replay capture, not merely
/// before the attach request returns.
///
/// The payload fits on one 80-column row but reflows across rows at 40
/// columns. The old capture-before-resize ordering replayed the whole
/// payload contiguously even though tmux itself already reported the new
/// geometry. A fresh agent echo is the replay-completion barrier: unlike
/// bracketed-paste restoration, it is available on every supported tmux
/// version, and it cannot arrive before the replay queued ahead of it.
#[tokio::test]
async fn attach_replay_uses_the_requested_geometry() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (channel, mut first) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut initial = Vec::new();
    wait_for(&mut first, &mut initial, "FAKE-AGENT READY", 20).await;
    let payload = format!("geometry-{}", "x".repeat(50));
    h.client
        .send_input(channel, format!("{payload}\r").into_bytes())
        .await;
    wait_for(&mut first, &mut initial, &payload, 10).await;
    h.client.detach(channel).await;

    let (channel, mut second) = h
        .client
        .attach(&session.id, 40, 24)
        .await
        .expect("reattach");
    let mut replay = Vec::new();
    h.client
        .send_input(channel, b"geometry-barrier\r".to_vec())
        .await;
    wait_for(
        &mut second,
        &mut replay,
        "echo:\x1b[36mgeometry-barrier",
        10,
    )
    .await;

    assert!(
        !replay
            .windows(payload.len())
            .any(|window| window == payload.as_bytes()),
        "payload stayed contiguous, so replay was captured before the attach-time resize"
    );
}

/// A resize from a kicked CONNECTION must be dropped — the
/// connection-identity (`same_channel`) half of the Resize check.
///
/// The colliding channel ids are the point: both connections number
/// from 1, so the channel-id comparison passes for the kicked client and
/// only connection identity rejects it. Delete the `same_channel` half
/// and this fails. (The channel-id half is pinned separately by
/// `resize_from_a_stale_channel_on_the_same_connection_is_ignored`.)
#[tokio::test]
async fn resize_from_a_kicked_connection_is_ignored() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (loser_chan, mut rx1) = h.client.attach(&session.id, 80, 24).await.expect("attach1");
    let mut seen1 = Vec::new();
    wait_for(&mut rx1, &mut seen1, "FAKE-AGENT READY", 20).await;

    let winner = h.second_client().await;
    let (winner_chan, mut rx2) = winner.attach(&session.id, 80, 24).await.expect("attach2");
    // The colliding ids are the point: if this ever fails, the test has
    // stopped exercising the case it exists for.
    assert_eq!(loser_chan, winner_chan, "both connections number from 1");
    let mut seen2 = Vec::new();
    wait_for(&mut rx2, &mut seen2, "FAKE-AGENT READY", 15).await;

    // Winner establishes a known geometry first.
    winner.resize(&session.id, winner_chan, 100, 30).await;
    wait_for_geometry(&h, "100x30").await;

    h.client.resize(&session.id, loser_chan, 111, 33).await;
    h.client
        .list_sessions()
        .await
        .expect("barrier after kicked resize");
    assert_geometry_stays(
        &h,
        "100x30",
        "a kicked client's resize reflowed the winner's terminal",
    )
    .await;
}

/// A resize from a stale CHANNEL on the still-attached connection must
/// be dropped — the channel-id half of the Resize check.
///
/// Within one connection (one helm, two browser tabs), a takeover
/// assigns the session a new channel; `same_channel` passes for the old
/// tab's in-flight resize, and only the channel-id comparison rejects
/// it. Delete that comparison and this fails. The sibling test above
/// pins the connection-identity half.
#[tokio::test]
async fn resize_from_a_stale_channel_on_the_same_connection_is_ignored() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (stale_chan, mut rx1) = h.client.attach(&session.id, 80, 24).await.expect("attach1");
    let mut seen1 = Vec::new();
    wait_for(&mut rx1, &mut seen1, "FAKE-AGENT READY", 20).await;

    // Second attach on the SAME connection: new channel, kicks the first.
    let (live_chan, mut rx2) = h.client.attach(&session.id, 80, 24).await.expect("attach2");
    assert_ne!(
        stale_chan, live_chan,
        "one connection numbers channels uniquely"
    );
    let mut seen2 = Vec::new();
    wait_for(&mut rx2, &mut seen2, "FAKE-AGENT READY", 15).await;

    h.client.resize(&session.id, live_chan, 100, 30).await;
    wait_for_geometry(&h, "100x30").await;

    h.client.resize(&session.id, stale_chan, 111, 33).await;
    h.client
        .list_sessions()
        .await
        .expect("barrier after stale-channel resize");
    assert_geometry_stays(
        &h,
        "100x30",
        "a stale channel's resize reflowed the live attachment's terminal",
    )
    .await;
}

/// A session whose agent exits stays viewable and replayable.
///
/// This is what `remain-on-exit on` buys (SPEC.md: a stopped or exited
/// session's terminal stays viewable while its host is up). Without that
/// config line the tmux session disappears on exit and the reattach
/// below fails outright.
#[tokio::test]
async fn exited_agent_leaves_a_viewable_terminal() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    h.client.send_input(chan, b"quit\r".to_vec()).await;

    // Wait for the pane to actually be dead by asking tmux, not by
    // watching for the agent's farewell text. Output-watching would race
    // the process teardown this test deliberately provokes; `pane_dead`
    // is the state the assertion below actually depends on.
    //
    // There is no `Detached` to WAIT for — `remain-on-exit` keeps the
    // session alive after the process dies, which is the property under
    // test — but one ARRIVING means this attachment is gone and no further
    // `quit` can ever reach the pane, so the wait below would burn its
    // whole budget on an outcome already decided. That is monitored
    // through the DETACH SIGNAL rather than by draining the event queue:
    // the signal is out of band precisely so a consumer parked on
    // something else can be woken by it, and `TermStream`'s queue-facing
    // API deliberately prefers buffered data over the detach — so a poll
    // of the queue can be empty for the entire life of a detached
    // attachment and never report it.
    let mut detached = rx.detach_signal();
    let sock = h.state.path().join("tmux.sock");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        let out = tmux_query(&sock, &["display-message", "-p", "#{pane_dead}"]).await;
        if String::from_utf8_lossy(&out.stdout).trim() == "1" {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "agent never exited after quit"
        );
        // Re-sent every pass, not just once. `quit` is idempotent against
        // the basic fake agent (it exits on the first one and the pane is
        // dead for every later one, which tmux simply drops), and each
        // send is a bounded exchange with tmux that a loaded machine can
        // lose outright — one lost send would otherwise leave this loop
        // waiting the full 30s for an exit nobody ever asked for.
        h.client.send_input(chan, b"quit\r".to_vec()).await;
        // The pause between polls is also where the detach is watched.
        // Both arms of this select are cancel-safe (a watch `changed()`
        // and a sleep), so losing either race costs nothing.
        tokio::select! {
            reason = detached.detached() => match reason {
                Some(reason) => panic!(
                    "the attachment was detached ({reason}) before the agent exited; no \
                     further quit can reach the pane"
                ),
                None => panic!(
                    "the supervisor client went away before the agent exited; no further \
                     quit can reach the pane"
                ),
            },
            () = tokio::time::sleep(Duration::from_millis(100)) => {}
        }
    }
    h.client.detach(chan).await;

    // The attach succeeding IS the contract: without `remain-on-exit on`
    // the window closes when the process exits, taking the only-window
    // session with it, and every tmux call in the attach path then fails.
    // (The replayed content is deliberately not asserted — a dead pane's
    // captured screen depends on what the exiting program left behind.)
    //
    // Retried while the failure looks transient, because the contract is
    // "an exited session's terminal is still attachable", not "attachable
    // on the first try". An attach is several bounded tmux exchanges (the
    // replay command group among them), and a machine loaded enough to
    // blow one of those budgets says nothing about whether the window
    // survived the exit. Anything that does NOT look like a timeout fails
    // at once: a closed window is a permanent, differently-shaped error
    // and must not be retried into a slow pass.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    let (_chan2, _rx2) = loop {
        match h.client.attach(&session.id, 80, 24).await {
            Ok(attached) => break attached,
            Err(e) => {
                assert!(
                    looks_like_a_tmux_timeout(&e),
                    "a session whose agent exited must still be attachable, and this failure \
                     is not a transient one: {e:#}"
                );
                assert!(
                    tokio::time::Instant::now() < deadline,
                    "a session whose agent exited never became attachable within 60s; last \
                     error: {e:#}"
                );
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        }
    };
}

/// Whether a failed request failed because a tmux exchange ran out of
/// budget, as opposed to for a reason retrying cannot fix.
///
/// Text matching, deliberately, and the alternative deserves recording
/// because it is the obvious one: the supervisor's timeouts really do
/// carry a `tokio::time::error::Elapsed` at the root of their `anyhow`
/// chain, so downcasting to it looks like the precise answer. It is not
/// REACHABLE here. Every handler failure is flattened to a string on the
/// wire (`ControlMsg::Error`'s `message`, built with `format!("{e:#}")`)
/// and rebuilt client-side as a `SupervisorError`, so no source error
/// survives the round trip at all — a downcast to `Elapsed` would match
/// nothing and silently turn this into a no-retry loop, which is worse
/// than the imprecision it was meant to remove.
///
/// The match is narrowed as far as the wire allows: the kind must be
/// `Internal` (a timeout is never a refusal the caller could have
/// avoided), and only the supervisor's own message is searched rather than
/// the rendered chain, so context this test's own layers might add can
/// never make an unrelated failure look retryable.
fn looks_like_a_tmux_timeout(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<SupervisorError>()
        .is_some_and(|e| e.kind == ErrorKind::Internal && e.message.contains("timed out"))
}

/// The adopted-server gap: tmux reads a `-f` config only when it STARTS a
/// server, so `ensure_server`'s adopt-a-surviving-server path (the
/// ordinary case across a supervisor restart or upgrade) never rereads
/// `TmuxDriver::config_body` at all — which is exactly why `focus-events`
/// is not in that config in the first place, and is instead reconciled by
/// an explicit, unconditional `set-option` every time `ensure_server` runs
/// (see that call's own doc for the full rationale, including what this
/// option does and does not actually change for us). A test that only
/// ever hits the fresh-start path would keep passing even if that
/// explicit reconciliation silently regressed back to "rely on the config
/// file", because fresh starts read the config regardless. This test
/// provokes adoption specifically: a server is started by hand, on this
/// state dir's socket, with focus-events deliberately off — standing in
/// for a survived server an upgraded supervisor binary reattaches to,
/// whose config predates this option (or simply had it off) — and only
/// THEN does a `Supervisor` get constructed against the same socket,
/// which `ensure_server` must adopt rather than start fresh.
///
/// `focus-events` is a SERVER option (`set -s`), so the live query below
/// uses `show-options -s` to match — a `-g` query would rely on tmux's
/// scope inference rather than pinning the same table the fix itself
/// names explicitly.
#[tokio::test]
async fn adopted_tmux_server_gets_focus_events_explicitly_not_just_from_config() {
    let _slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = tempfile::tempdir().expect("state");
    let sock = state.path().join("tmux.sock");
    let _tmux = TmuxServerGuard(sock.clone());

    // Hand-roll a server on this socket BEFORE any farhelm code touches
    // this state dir, deliberately with the option off — this is the
    // "survived server" half of the adoption gap, so it must exist first.
    // `start-server` alone (rather than `new-session`) is enough to leave
    // a live, queryable server: `exit-empty off` keeps it up with no
    // sessions, so there is no need to spawn a pointless shell just to
    // give it something to hold open.
    let off_conf = state.path().join("pre-existing.conf");
    tokio::fs::write(
        &off_conf,
        "set -s exit-empty off\nset -s focus-events off\n",
    )
    .await
    .expect("write throwaway pre-existing config");
    let started = tokio::process::Command::new("tmux")
        .arg("-S")
        .arg(&sock)
        .arg("-f")
        .arg(&off_conf)
        .arg("start-server")
        .status()
        .await
        .expect("spawn scratch tmux");
    assert!(started.success(), "test setup: scratch tmux must start");

    // Now let the real code run: `ensure_server` (via `Supervisor::new_with_exe`)
    // finds this socket already live and must ADOPT it, not start a fresh
    // server whose config it would otherwise get to read.
    Supervisor::new_with_exe(state.path(), farhelm_bin().into())
        .await
        .expect("supervisor construction must adopt the pre-existing server");

    let out = tmux_query(&sock, &["show-options", "-s", "focus-events"]).await;
    assert!(
        out.status.success(),
        "show-options -s focus-events failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "focus-events on",
        "adopting a pre-existing server must still bring focus-events on, not just \
         a fresh server's config"
    );
}

/// PLAN_M2.md's list-status contract: once an agent exits ON ITS OWN — no
/// stop or delete involved — the next `ListSessions` must reflect that as
/// `Exited` with the exact exit code tmux observed, not stay live
/// forever. `exited_agent_leaves_a_viewable_terminal` already proves the
/// terminal itself survives; this proves the status field tracks the same
/// event. The basic fake agent's own `quit` path exits 0, which is what
/// makes this an easy code to pin exactly (unlike a signal death).
#[tokio::test]
async fn exited_agent_lists_as_exited_with_its_exit_code() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    h.client.send_input(chan, b"quit\r".to_vec()).await;

    // Exit-code precision, version-gated: see `wait_for_exit_code`.
    wait_for_exit_code(&h.client, &session.id, 0, 30).await;
}

/// A nonzero exit is reported precisely, not just "not alive" — the whole
/// point of carrying `exit_code` through instead of a boolean liveness
/// flag. A plain shell exit needs no fake-agent script at all: its code
/// is exactly what tmux's `#{pane_dead_status}` reports.
///
/// The half-second sleep before the exit is load-bearing, not padding: a
/// pane whose process dies while tmux is still setting the pane up can
/// lose the recorded exit status entirely (observed on loaded CI runners
/// as a permanent `Exited { exit_code: None }`; never reproduced locally,
/// where `exit 3` alone always raced in tmux's favor). An agent that
/// exits before its terminal even finishes materializing is not the
/// behavior this test pins, so the fixture deliberately outlives pane
/// setup instead.
#[tokio::test]
async fn nonzero_exit_lists_with_its_precise_code() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            "sh -c 'sleep 0.5; exit 3'",
            None,
            80,
            24,
        )
        .await
        .expect("create");

    // Exit-code precision, version-gated: see `wait_for_exit_code`.
    wait_for_exit_code(&h.client, &session.id, 3, 30).await;
}

/// Invocations that cannot become an argv fail the create outright, with
/// no session left behind — the same contract as a missing directory.
#[tokio::test]
async fn unparseable_invocations_error_without_creating_a_session() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let cwd = work.path().to_string_lossy().into_owned();

    let empty = h
        .client
        .create_session(&cwd, "", None, 80, 24)
        .await
        .expect_err("empty invocation must fail");
    assert!(empty.to_string().contains("empty"));
    assert_eq!(
        empty
            .downcast_ref::<SupervisorError>()
            .expect("an empty invocation must carry a SupervisorError")
            .kind,
        ErrorKind::InvalidRequest,
        "an empty invocation is the caller's mistake, not a server fault"
    );

    let unterminated = h
        .client
        .create_session(&cwd, "claude 'unterminated", None, 80, 24)
        .await
        .expect_err("unparseable invocation must fail");
    let unterminated_text = unterminated.to_string();
    assert!(unterminated_text.contains("parsing agent invocation"));
    // `RequestError` is attached as `.context(...)` over the `shell_words`
    // parse failure specifically so its own diagnostic keeps reaching the
    // user (see that struct's docs) — pin that it actually does, not just
    // that our own classification message survives.
    assert!(
        unterminated_text.contains("missing closing quote"),
        "error lost shell_words's own diagnostic: {unterminated_text}"
    );
    assert_eq!(
        unterminated
            .downcast_ref::<SupervisorError>()
            .expect("an unparseable invocation must carry a SupervisorError")
            .kind,
        ErrorKind::InvalidRequest,
        "a shell-syntax error in the invocation is the caller's mistake, not a server fault"
    );

    assert!(h.client.list_sessions().await.unwrap().sessions.is_empty());
}

/// The launch shim's sentinel is what separates "could not start" from
/// "ran and exited" (SPEC.md's error/exited split), and it is the entire
/// reason the shim exists — a shell-side sentinel after `exec` never
/// fires under zsh. Both directions are checked: failure writes it,
/// success does not. The shim must also unlink the spec on every path —
/// exec failure, malformed spec, and success alike — because it holds
/// the agent's full command line plus its spawn bearer credential, and
/// nothing else removes it before the next supervisor restart's sweep.
///
/// A plain `#[test]`: everything here is synchronous process spawning,
/// and it needs no tmux, no supervisor, and no runtime.
#[test]
fn launch_shim_records_exec_failure_only_on_failure() {
    let dir = tempfile::tempdir().unwrap();

    let write_spec = |name: &str, argv: Vec<&str>| {
        let status_file = dir.path().join(format!("{name}.status"));
        let spec_path = dir.path().join(format!("{name}.json"));
        let spec = serde_json::json!({
            "argv": argv,
            "status_file": status_file.to_string_lossy(),
            "session_id": format!("test-{name}"),
            "session_token": format!("credential-{name}"),
            "supervisor_sock": dir.path().join("supervisor.sock"),
            "farhelm_bin_dir": dir.path(),
        });
        std::fs::write(&spec_path, spec.to_string()).unwrap();
        (spec_path, status_file)
    };

    let (bad_spec, bad_status) = write_spec("bad", vec!["/nonexistent/definitely-not-here"]);
    let out = std::process::Command::new(farhelm_bin())
        .args(["internal", "launch"])
        .arg(&bad_spec)
        .output()
        .expect("run shim");
    assert!(!out.status.success(), "failed exec must exit nonzero");
    let sentinel = std::fs::read_to_string(&bad_status).expect("sentinel must exist");
    assert!(
        sentinel.contains("exec_failed") && sentinel.contains("errno="),
        "sentinel must name the failure and its errno, got: {sentinel}"
    );
    assert!(
        !bad_spec.exists(),
        "the shim must unlink the credential-bearing spec even when exec fails"
    );

    let (ok_spec, ok_status) = write_spec("ok", vec!["true"]);
    let out = std::process::Command::new(farhelm_bin())
        .args(["internal", "launch"])
        .arg(&ok_spec)
        .output()
        .expect("run shim");
    assert!(out.status.success(), "successful exec must exit zero");
    assert!(
        !ok_status.exists(),
        "a successful exec must leave no sentinel — its absence is what makes an exit 'exited', not 'error'"
    );
    assert!(
        !ok_spec.exists(),
        "the shim must unlink the spec before exec — after it, no code of ours runs"
    );

    // A malformed spec takes the early-return path, which must unlink
    // too: a truncated spec still holds a credential prefix.
    let malformed_spec = dir.path().join("malformed.json");
    std::fs::write(&malformed_spec, b"{ not json").unwrap();
    let out = std::process::Command::new(farhelm_bin())
        .args(["internal", "launch"])
        .arg(&malformed_spec)
        .output()
        .expect("run shim");
    assert!(!out.status.success(), "malformed spec must exit nonzero");
    assert!(
        !malformed_spec.exists(),
        "the shim must unlink the spec even when it cannot parse it"
    );
}

/// A client kicked by a takeover must not keep typing into the pane.
///
/// The supervisor enforces this rather than trusting clients to stop, so
/// deleting that check must fail a test — before this one, it did not.
#[tokio::test]
async fn kicked_client_cannot_still_send_input() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (c1, mut rx1) = h.client.attach(&session.id, 80, 24).await.expect("attach1");
    let mut seen1 = Vec::new();
    wait_for(&mut rx1, &mut seen1, "FAKE-AGENT READY", 20).await;

    let (c2, mut rx2) = h.client.attach(&session.id, 80, 24).await.expect("attach2");
    let mut seen2 = Vec::new();
    wait_for(&mut rx2, &mut seen2, "FAKE-AGENT READY", 15).await;

    // Ghost first, marker second, both on the same connection so the
    // supervisor processes them in order — by the time the marker echo
    // arrives, the ghost has already been accepted or dropped.
    h.client.send_input(c1, b"ghost-input\r".to_vec()).await;
    h.client.send_input(c2, b"marker-input\r".to_vec()).await;
    wait_for(&mut rx2, &mut seen2, "marker-input", 15).await;

    let transcript = String::from_utf8_lossy(&seen2);
    assert!(
        !transcript.contains("ghost-input"),
        "input from a kicked attachment reached the pane:\n{transcript}"
    );
}

/// Input authorization must hold ACROSS connections, not just within one.
///
/// Channel ids are only unique per connection — every client numbers
/// from 1 — so when the winner attaches from a different connection, its
/// channel id collides with the kicked client's. The channel-id half of
/// the check passes for the ghost input; only the connection-identity
/// half (`same_channel`) drops it. The single-connection test above
/// cannot see that half, and before this test, deleting it failed
/// nothing.
#[tokio::test]
async fn input_from_a_kicked_connection_is_dropped() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (loser_chan, mut rx1) = h.client.attach(&session.id, 80, 24).await.expect("attach1");
    let mut seen1 = Vec::new();
    wait_for(&mut rx1, &mut seen1, "FAKE-AGENT READY", 20).await;

    let winner = h.second_client().await;
    let (winner_chan, mut rx2) = winner.attach(&session.id, 80, 24).await.expect("attach2");
    assert_eq!(loser_chan, winner_chan, "both connections number from 1");
    let mut seen2 = Vec::new();
    wait_for(&mut rx2, &mut seen2, "FAKE-AGENT READY", 15).await;

    // The two inputs travel on different connections, so the winner's
    // later marker is not an ordering barrier for the loser. A
    // request/reply on the LOSING connection is: its reply proves the
    // supervisor processed the ghost before the winner marker lets this
    // test finish.
    h.client
        .send_input(loser_chan, b"ghost-xconn\r".to_vec())
        .await;
    h.client
        .list_sessions()
        .await
        .expect("barrier after kicked input");
    winner
        .send_input(winner_chan, b"marker-xconn\r".to_vec())
        .await;
    wait_for(&mut rx2, &mut seen2, "marker-xconn", 15).await;

    let transcript = String::from_utf8_lossy(&seen2);
    assert!(
        !transcript.contains("ghost-xconn"),
        "input from a kicked connection reached the pane:\n{transcript}"
    );
}

/// Losing the supervisor connection must fail everything promptly:
/// attached terminals get an explicit `Detached`, and later requests
/// error instead of hanging their HTTP handler forever. The client
/// carries a deliberate lock-ordering invariant for exactly this
/// (`fail_all`), and nothing else exercises it. Every wait is under a
/// timeout because a hang is precisely the failure under test.
#[tokio::test]
async fn connection_loss_detaches_terminals_and_fails_requests() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();

    // A separate connection routed through a severable relay: two duplex
    // pipes joined by copy tasks. Aborting the copy tasks drops every
    // relay half at once, so BOTH endpoints see a dead transport — which
    // is what a dead socket or broken ssh pipe looks like. (Aborting the
    // server's connection task instead would not work: the split write
    // half lives on in its writer task, so the client would never see
    // EOF.)
    let (client_side, relay_a) = tokio::io::duplex(1 << 20);
    let (relay_b, server_side) = tokio::io::duplex(1 << 20);
    let sup = Arc::clone(&h.sup);
    tokio::spawn(async move {
        let _ = handle_connection(sup, server_side).await;
    });
    let (mut ar, mut aw) = tokio::io::split(relay_a);
    let (mut br, mut bw) = tokio::io::split(relay_b);
    let relay_up = tokio::spawn(async move {
        let _ = tokio::io::copy(&mut ar, &mut bw).await;
    });
    let relay_down = tokio::spawn(async move {
        let _ = tokio::io::copy(&mut br, &mut aw).await;
    });
    let (r, w) = tokio::io::split(client_side);
    let client = SupervisorClient::start(r, w).await.expect("handshake");

    let session = client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script basic"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let (_chan, mut rx) = client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    // Sever the transport.
    relay_up.abort();
    relay_down.abort();

    let detached = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            match rx.recv().await {
                Some(TermEvent::Detached(reason)) => return reason,
                // The replay-complete marker (PLAN_M5.md item 4) is
                // presentation metadata this loop is only waiting past,
                // exactly like ordinary data — its ordering contract has
                // its own coverage in replay_marker.rs and, helm-side, in
                // farhelm-helm's client.rs/lib.rs.
                Some(TermEvent::Data(_)) | Some(TermEvent::ReplayComplete) => continue,
                None => panic!("terminal stream closed without a Detached event"),
            }
        }
    })
    .await
    .expect("timed out waiting for Detached after connection loss");
    assert!(
        detached.contains("connection lost"),
        "detach reason should say the connection is gone, got: {detached}"
    );

    let err = tokio::time::timeout(Duration::from_secs(10), client.list_sessions())
        .await
        .expect("request after connection loss must fail fast, not hang")
        .expect_err("request on a dead connection must error");
    assert!(
        err.to_string().contains("connection closed"),
        "unexpected error: {err:#}"
    );

    // The session must still be usable from a healthy connection. This
    // is the third trigger of the frozen-replay hazard (the other two —
    // takeover and voluntary detach — have their own tests): the dead
    // connection's forwarder must be aborted AND awaited before a new
    // attach opens its control-mode client, or the reattach renders the
    // replay and then never updates. Asserting on a FRESH echo, not the
    // replay, is what tells those apart — replay alone arrives either
    // way.
    let (chan, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("reattach");
    let mut seen2 = Vec::new();
    wait_for(&mut rx2, &mut seen2, "FAKE-AGENT READY", 20).await;
    h.client
        .send_input(chan, b"alive-after-loss\r".to_vec())
        .await;
    wait_for(&mut rx2, &mut seen2, "echo:", 15).await;
    wait_for(&mut rx2, &mut seen2, "alive-after-loss", 10).await;
}

/// A client may stop reading while it continues writing. The
/// supervisor's writer failure must terminate `handle_connection`
/// without waiting for read EOF, or that half-broken connection retains
/// its attachment state indefinitely.
#[tokio::test]
async fn supervisor_writer_failure_ends_a_half_broken_connection() {
    let h = harness().await;
    let (client_side, server_inner) = tokio::io::duplex(64 * 1024);
    let fail_writes = Arc::new(AtomicBool::new(false));
    let server_side = ToggleWriteFailure {
        inner: server_inner,
        fail_writes: Arc::clone(&fail_writes),
    };
    let sup = Arc::clone(&h.sup);
    let connection = tokio::spawn(async move { handle_connection(sup, server_side).await });
    let (read_half, write_half) = tokio::io::split(client_side);
    let mut reader = FrameReader::new(read_half);
    let mut writer = FrameWriter::new(write_half);
    handshake(&mut reader, &mut writer, "helm")
        .await
        .expect("handshake");

    // Keep the request direction healthy while making the supervisor's
    // next reply fail. The connection task must not wait for us to close
    // the still-open request writer.
    fail_writes.store(true, Ordering::SeqCst);
    writer
        .write_control(&ControlMsg::ListSessions {
            req_id: 42,
            cursor: None,
            limit: None,
        })
        .await
        .expect("request reaches supervisor");

    let result = tokio::time::timeout(Duration::from_secs(5), connection)
        .await
        .expect("supervisor connection task hung after writer failure")
        .expect("connection task panicked");
    assert!(
        result
            .expect_err("writer failure must end the connection")
            .to_string()
            .contains("frame write to client failed")
    );
}

/// A peer that stops reading — without ever erroring — must not pin
/// `handle_connection` open forever.
///
/// Before `WRITER_DRAIN_TIMEOUT` existed, the shutdown tail did
/// `drop(tx); writer_task.await;` unconditionally. That is fine for the
/// write-*error* case (the `writer_failed` oneshot already ends the
/// connection promptly, pinned by the test above), but it has no answer
/// for a peer that just stops reading: a full TCP/pipe window with
/// nothing on the other end. The writer task's `write_frame` call parks
/// with no error to report, `writer_task.await` never resolves, and
/// `handle_connection` — plus every reply still queued for it — leaks
/// for the process lifetime. This test reproduces exactly that: flood
/// the supervisor with requests without ever reading a reply (so a real
/// backlog queues up), then close only the peer's write half so the
/// supervisor's read loop sees EOF and runs the shutdown tail with the
/// writer parked mid-write and a backlog behind it. This peer makes zero
/// progress for the rest of the test, so it stays a "gone" peer under
/// `drain_writer`'s no-progress window too — the case that test coverage
/// still holds for, even though the shutdown tail no longer enforces a
/// flat deadline. Without the fix this hangs forever; with it,
/// `handle_connection` returns once a full `WRITER_DRAIN_TIMEOUT` window
/// passes without a frame landing.
///
/// M2.5 bounded the writer queue, which changed how this same peer
/// misbehaves and made the test's original shape unable to reach its own
/// half-close: once every admission permit is held by a handler parked on
/// a full queue, `handle_control` blocks the read loop too, so the flood
/// below backs up into the request direction. `WRITER_STALL_TIMEOUT` is
/// what breaks that — shortened here, since the production value is a
/// minute — and the request count is now sized to fit comfortably inside
/// the transport buffer either way, so the half-close is reachable
/// whether or not the read loop is still draining when it happens.
#[tokio::test]
async fn writer_never_reading_peer_does_not_hang_connection_shutdown() {
    let h = harness_with_timeouts(SupervisorTimeouts {
        writer_stall: Duration::from_secs(2),
        ..SupervisorTimeouts::default()
    })
    .await;

    // A SMALL duplex buffer — unlike the 1 MiB transports the other
    // tests in this file use — so the reply direction fills from a
    // modest, fast-to-send backlog instead of requiring an impractical
    // flood to reproduce the stall.
    let (client_side, server_side) = tokio::io::duplex(4 * 1024);
    let sup = Arc::clone(&h.sup);
    let handle = tokio::spawn(async move { handle_connection(sup, server_side).await });

    let (read_half, write_half) = tokio::io::split(client_side);
    let mut reader = FrameReader::new(read_half);
    let mut writer = FrameWriter::new(write_half);
    handshake(&mut reader, &mut writer, "helm")
        .await
        .expect("handshake");

    // Enough cheap requests to fill the reply direction several times
    // over and leave a real backlog queued behind the parked writer, but
    // few enough (a `ListSessions` request is a few dozen bytes) that
    // they all fit in this 4 KiB transport buffer on their own. That
    // second property is what keeps the flood from blocking the TEST:
    // since M2.5's bounded writer queue, the supervisor's read loop can
    // itself stall behind a peer that never reads, so this must not
    // depend on the read loop keeping up.
    for req_id in 0..64u64 {
        writer
            .write_control(&ControlMsg::ListSessions {
                req_id,
                cursor: None,
                limit: None,
            })
            .await
            .expect("request direction stays open; the supervisor keeps reading it");
    }

    // Half-close: the write half goes away while the read half (which
    // this test never touches) stays open. That is what makes the
    // supervisor's read loop observe EOF and enter the shutdown tail —
    // with the writer task still parked on an unwritable reply and a
    // full backlog queued behind it.
    writer.shutdown().await.expect("half-close write side");

    tokio::time::timeout(Duration::from_secs(30), handle)
        .await
        .expect("handle_connection must return within the bounded writer drain, not hang forever")
        .expect("connection task panicked")
        .expect("a peer closing its write half cleanly is not itself a connection error");
}

/// The real socket transport: `Supervisor::serve` plus
/// `farhelm internal stdio`, which is the remote-host path with ssh
/// removed. Every other test bypasses both via an in-process pipe, so
/// without this the proxy's half-close, its final flush, and the
/// socket-path agreement between serve and connect are unexercised.
///
/// This is also the one test that sees the served socket on disk, so it
/// doubles as the check on serve()'s security-boundary side effects:
/// the launch-dir and socket modes (the ONLY authentication the
/// protocol has — dropping either mode-setting call silently yields
/// world-readable defaults under umask 022), and the startup sweep of
/// orphaned launch specs.
#[tokio::test]
async fn stdio_proxy_carries_a_real_session() {
    let _slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = tempfile::tempdir().expect("tempdir");
    let sup =
        Supervisor::new_with_exe_and_timeouts(state.path(), farhelm_bin().into(), suite_timeouts())
            .await
            .expect("supervisor");
    // Declared after `state`, so it drops first: kill the server, then
    // delete the directory holding its socket. Without this guard a
    // panic anywhere below leaked the tmux server (plus login shell and
    // fake agent) forever — the exact accumulation Harness exists to
    // prevent, on the one test that cannot use it.
    let _tmux = TmuxServerGuard(state.path().join("tmux.sock"));

    // An orphaned launch spec from a "previous run": serve() must sweep
    // it once — and only once — it owns the socket. It holds an agent
    // command line, which is why nothing may leave it behind.
    // Named the way a real launch names its files — per session AND
    // generation (`launch::spec_path_for_launch`) — because the sweep
    // recognizes its own naming and leaves anything else alone.
    let orphan = spec_path_for_launch(state.path(), "orphan", 0);
    std::fs::write(&orphan, b"{}").expect("plant orphan spec");

    let serving = Arc::clone(&sup);
    tokio::spawn(async move {
        let _ = serving.serve().await;
    });
    wait_for_supervisor_ready(state.path()).await;

    let mut child = tokio::process::Command::new(farhelm_bin())
        .args(["internal", "stdio", "--state-dir"])
        .arg(state.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn stdio proxy");
    let stdout = child.stdout.take().expect("piped stdout");
    let stdin = child.stdin.take().expect("piped stdin");
    let client = SupervisorClient::start(stdout, stdin)
        .await
        .expect("handshake over the stdio proxy");

    // The handshake above required serve()'s accept loop, which starts
    // only after the sweep — so the orphan must be gone by now.
    assert!(
        !orphan.exists(),
        "serve() must sweep orphaned launch specs at startup"
    );
    {
        use std::os::unix::fs::PermissionsExt;
        // The launch dir, not the state dir: tempfile creates the state
        // dir 0700 itself, so asserting on it would pass with the mode
        // logic deleted. The launch dir is created by ensure_private_dir
        // in this very flow, so its mode is actually the code's doing.
        let launch = state.path().join("launch");
        let dir_mode = std::fs::metadata(&launch).unwrap().permissions().mode() & 0o777;
        assert_eq!(dir_mode, 0o700, "launch dir must be owner-only");
        let sock = state.path().join("supervisor.sock");
        let sock_mode = std::fs::metadata(&sock).unwrap().permissions().mode() & 0o777;
        assert_eq!(sock_mode, 0o600, "supervisor socket must be owner-only");
    }

    let work = tempfile::tempdir().unwrap();
    let session = client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script basic"),
            None,
            80,
            24,
        )
        .await
        .expect("create over proxy");
    let (chan, mut rx) = client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 25).await;
    client
        .send_input(chan, b"through-the-proxy\r".to_vec())
        .await;
    wait_for(&mut rx, &mut seen, "through-the-proxy", 15).await;
}

/// Input larger than one protocol frame must arrive intact and in order.
///
/// One `send_input` call of ~48 KiB crosses the frame-chunking boundary:
/// the helm client splits it into two 32 KiB-capped frames, with a line
/// straddling the split, and each arriving frame is handed to
/// `InputClient::send`, which further chunks it into many 256-byte
/// `send-keys -H` commands against the same dedicated input client (see
/// `tmux.rs`). Every other test sends a dozen bytes, so a truncation, a
/// reorder, or a dropped chunk at either boundary — the frame split or
/// any of the many `send-keys` chunk splits inside it — would otherwise
/// go unnoticed.
///
/// The payload is many short lines, not one long one, by necessity: the
/// pane's PTY is in canonical mode, where the kernel caps a single input
/// line at MAX_CANON (4095 bytes on Linux) and silently discards the
/// excess — a single >32 KiB line can never round-trip, no matter how
/// correct the chunking is.
#[tokio::test]
async fn large_input_survives_chunking() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();

    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script basic"),
            None,
            200,
            50,
        )
        .await
        .expect("create");
    let (chan, mut rx) = h.client.attach(&session.id, 200, 50).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    // 3200 numbered lines ≈ 48 KiB in one send_input call. Numbering
    // makes both loss and reordering visible: every line must come back
    // as its own echo, in order.
    const LINES: usize = 3200;
    let mut input = Vec::new();
    for i in 0..LINES {
        input.extend_from_slice(format!("chunkline-{i:04}\r").as_bytes());
    }
    assert!(
        input.len() > 32 * 1024,
        "payload must exceed one 32 KiB frame to exercise the frame-chunking layer"
    );
    h.client.send_input(chan, input).await;

    // Wait for the final echo, then verify every line echoed in order.
    // The needle includes the fake agent's echo prefix and color code so
    // it cannot match the PTY's input echo of the same text.
    let last = format!("echo:\x1b[36mchunkline-{:04}", LINES - 1);
    wait_for(&mut rx, &mut seen, &last, 60).await;
    let transcript = String::from_utf8_lossy(&seen);
    let mut pos = 0;
    for i in 0..LINES {
        let needle = format!("echo:\x1b[36mchunkline-{i:04}");
        match transcript[pos..].find(&needle) {
            Some(at) => pos += at + needle.len(),
            None => panic!(
                "echo for line {i} missing or out of order after byte {pos} — \
                 a chunk was dropped or reordered at a chunking boundary"
            ),
        }
    }
}

/// A zero-sized attach must still produce a working terminal, at the
/// clamped 1x1 geometry.
///
/// A browser can report 0 columns mid-layout. tmux rejects `resize-window
/// -x 0` outright ("width too small"), so the driver clamps to 1. Both
/// halves are asserted: the stream still flows, AND tmux actually holds
/// the clamped geometry — without the second assertion, deleting the
/// clamp passes this test, because a failed resize during attach is
/// deliberately warn-only.
#[tokio::test]
async fn attach_with_degenerate_size_still_works() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let (_chan, mut rx) = h
        .client
        .attach(&session.id, 0, 0)
        .await
        .expect("attach with 0x0 must succeed");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    // The attach's resize ran before the replay that carried the marker
    // above, so a single read is race-free here.
    let out = tmux_query(
        &h.state.path().join("tmux.sock"),
        &["display-message", "-p", "#{window_width}x#{window_height}"],
    )
    .await;
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "1x1",
        "0x0 must clamp to 1x1, not fail or leave the old geometry"
    );
}

/// Attaching to a tmux session that does not exist must fail with tmux's
/// own diagnostic, not report success.
///
/// Pins a bug found in review (recorded in lore/): `%begin` only opens a
/// control-mode reply block, but was treated as "attached" — a failed
/// attach reported success and discarded tmux's reason. Nothing else
/// reaches this path: the service layer rejects unknown session ids
/// before tmux is ever asked, so only a driver-level test can regress it.
#[tokio::test]
async fn control_mode_attach_to_missing_session_reports_tmux_reason() {
    use farhelm_supervisor::tmux::TmuxDriver;

    let _slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = tempfile::tempdir().expect("tempdir");
    let driver = TmuxDriver::new(state.path());
    driver.ensure_server().await.expect("ensure server");
    let _tmux = TmuxServerGuard(state.path().join("tmux.sock"));
    // A decoy session, so tmux's refusal names the missing session
    // ("can't find session: ...") instead of the generic "no sessions" a
    // sessionless server answers with.
    driver
        .create_session(
            "decoy",
            state.path().to_str().expect("tempdir path is UTF-8"),
            80,
            24,
            &[],
            &["sleep".to_string(), "60".to_string()],
        )
        .await
        .expect("decoy session");

    let err = match driver.open_replay_stream("no-such-session", "%0").await {
        Ok(_) => panic!("attaching to a missing session must fail"),
        Err(err) => err,
    };
    assert!(
        format!("{err:#}").contains("no-such-session"),
        "the error must carry tmux's own diagnostic naming the session, got: {err:#}"
    );
}

/// An untitled session takes its title from the working directory, and a
/// created session actually appears in the list — the positive form of
/// the assertion the error tests only make negatively.
#[tokio::test]
async fn created_sessions_are_listed_with_a_derived_title() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let basename = work
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let invocation = agent_cmd("internal fake-agent --script basic");

    let session = h
        .client
        .create_session(&work.path().to_string_lossy(), &invocation, None, 80, 24)
        .await
        .expect("create");
    assert_eq!(session.title, basename);
    assert_eq!(
        session.status,
        SessionStatus::Unknown,
        "SessionCreated's own reply must carry the create-time placeholder, not a fabricated \
         live — creation establishes only that the session and terminal exist, not that the \
         agent's exec succeeded (see ControlMsg::SessionCreated's own docs)"
    );

    let listed = h.client.list_sessions().await.expect("list");
    // Liveness and metadata are asserted SEPARATELY, deliberately: which
    // live status a session reports is the sampler's business (and changes
    // as it sharpens), while "every other field round-trips" is this
    // test's. Comparing the whole record against one hard-coded live
    // status would couple this test to a classification it is not about.
    let [row] = listed.sessions.as_slice() else {
        panic!(
            "exactly one session must be listed, got {:?}",
            listed.sessions
        );
    };
    assert!(
        row.status.is_live(),
        "a session that has never been touched must list as live once ListSessions computes \
         the real answer from tmux — even though the create-time reply itself said Unknown"
    );
    assert_eq!(
        *row,
        with_status(session.clone(), row.status.clone()),
        "and every other field must match what the create reply reported"
    );
    assert_eq!(listed.sessions[0].invocation, invocation);
}

/// Attaching to a session id the supervisor does not know must fail with
/// an error naming the session, and must not damage the connection — the
/// handler's contract is that per-request failures answer with an Error
/// message, never by killing the shared connection. This also exercises
/// the client's attach-failure cleanup (the pre-registered terminal
/// channel must be released, not leaked).
#[tokio::test]
async fn attach_to_unknown_session_errors_and_connection_survives() {
    let h = harness().await;

    let err = h
        .client
        .attach("definitely-not-a-session", 80, 24)
        .await
        .expect_err("attach to an unknown session must fail");
    assert!(
        err.to_string().contains("no such session"),
        "error must name the problem, got: {err:#}"
    );
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("an unknown-session attach must carry a SupervisorError")
            .kind,
        ErrorKind::NotFound,
        "an unknown session id is a not-found, not a bad request or server fault"
    );

    // The connection is still serviceable after the refused request.
    assert!(
        h.client
            .list_sessions()
            .await
            .expect("connection must survive a refused attach")
            .sessions
            .is_empty()
    );
}

/// A tmux failure during cutover belongs to one attach request, not the
/// multiplexed supervisor connection.
///
/// The session remains in the supervisor's M1 in-memory index after its
/// tmux session is killed behind the supervisor's back. That creates a
/// known session whose resize and control-mode attach both fail, reaching
/// the post-takeover error path rather than the early unknown-id check.
#[tokio::test]
async fn cutover_failure_is_request_local() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let socket = h.state.path().join("tmux.sock");
    let sessions = tmux_query(&socket, &["list-sessions", "-F", "#{session_name}"]).await;
    assert!(sessions.status.success(), "list private tmux sessions");
    let tmux_name = String::from_utf8(sessions.stdout)
        .expect("tmux session names are UTF-8")
        .trim()
        .to_string();
    let killed = tmux_query(&socket, &["kill-session", "-t", &tmux_name]).await;
    assert!(killed.status.success(), "kill private tmux session");

    let error = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect_err("attach must report the missing tmux session");
    assert!(
        format!("{error:#}").contains("no sessions"),
        "attach error lost tmux's diagnostic: {error:#}"
    );
    // A tmux hiccup has no `RequestError` opinion attached anywhere in the
    // supervisor, so `error_kind` falls through to its `Internal` default —
    // pin that default explicitly, since it is the realistic path most
    // unclassified supervisor failures take.
    assert_eq!(
        error
            .downcast_ref::<SupervisorError>()
            .expect("a tmux failure during cutover must still carry a SupervisorError")
            .kind,
        ErrorKind::Internal,
        "an unclassified tmux failure is a server fault, not the caller's mistake"
    );
    let listed = h
        .client
        .list_sessions()
        .await
        .expect("connection survives cutover failure");
    assert_eq!(
        listed.sessions,
        vec![with_status(
            session,
            SessionStatus::Exited { exit_code: None }
        )],
        "the stored tmux_name no longer resolves to a live pane, so this must list as \
         exited rather than fabricating liveness — the same honesty rule as the restart gap"
    );
}

/// Two supervisors must never own one state dir: the second `serve()`
/// has to refuse while the first is alive (atomically — the lock, not a
/// probe-then-remove dance, is what prevents the TOCTOU where the loser
/// unlinks the winner's freshly bound socket), and a stale socket file
/// left by a dead supervisor must not block the next one from binding.
#[tokio::test]
async fn serve_refuses_a_second_supervisor_but_replaces_a_stale_socket() {
    let _slot = SLOTS.acquire().await.expect("semaphore is never closed");

    // Half 1: live supervisor → second serve() refuses.
    let state = tempfile::tempdir().expect("tempdir");
    let sup1 = Supervisor::new_with_exe(state.path(), farhelm_bin().into())
        .await
        .expect("supervisor 1");
    let _tmux = TmuxServerGuard(state.path().join("tmux.sock"));
    let serving = Arc::clone(&sup1);
    tokio::spawn(async move {
        let _ = serving.serve().await;
    });
    let sock = state.path().join("supervisor.sock");
    wait_for_supervisor_ready(state.path()).await;
    let sup2 = Supervisor::new_with_exe(state.path(), farhelm_bin().into())
        .await
        .expect("supervisor 2");
    let err = sup2
        .serve()
        .await
        .expect_err("a second supervisor on the same state dir must refuse");
    assert!(
        err.to_string().contains("already running"),
        "refusal must say why, got: {err:#}"
    );
    // The winner's socket must still be there — the loser must not have
    // unlinked it on its way out.
    assert!(
        sock.exists(),
        "refused supervisor must not remove the live socket"
    );

    // Half 2: stale socket file (no listener behind it) → serve() binds.
    let state2 = tempfile::tempdir().expect("tempdir");
    let sup3 = Supervisor::new_with_exe(state2.path(), farhelm_bin().into())
        .await
        .expect("supervisor 3");
    let _tmux2 = TmuxServerGuard(state2.path().join("tmux.sock"));
    let stale = state2.path().join("supervisor.sock");
    std::fs::write(&stale, b"").expect("plant stale socket file");
    let serving = Arc::clone(&sup3);
    tokio::spawn(async move {
        let _ = serving.serve().await;
    });
    let connected = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if farhelm_supervisor::service::connect(state2.path())
                .await
                .is_ok()
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(
        connected.is_ok(),
        "a stale socket file must not stop the next supervisor from binding"
    );
}

/// The stdio proxy's half-close contract: stdin EOF must not tear the
/// proxy down before replies still in flight from the supervisor reach
/// stdout — and once the supervisor closes, the proxy process must
/// actually exit (its stdin read parks on the blocking pool, which a
/// plain runtime drop would wait on forever; over ssh a lingering proxy
/// keeps the channel open and turns a supervisor crash into a silently
/// frozen terminal). The wait_with_output timeout pins the exit; the
/// reply assertion pins the half-close.
#[tokio::test]
async fn stdio_proxy_half_close_delivers_in_flight_replies() {
    use tokio::io::AsyncWriteExt;

    let _slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = tempfile::tempdir().expect("tempdir");
    let sup = Supervisor::new_with_exe(state.path(), farhelm_bin().into())
        .await
        .expect("supervisor");
    let _tmux = TmuxServerGuard(state.path().join("tmux.sock"));
    let serving = Arc::clone(&sup);
    tokio::spawn(async move {
        let _ = serving.serve().await;
    });
    wait_for_supervisor_ready(state.path()).await;

    // Raw frames, no SupervisorClient: hello then a request, then EOF —
    // the reply is "in flight" precisely because stdin closed first.
    let mut input = Vec::new();
    Frame::control(&ControlMsg::hello("helm"))
        .encode(&mut input)
        .unwrap();
    Frame::control(&ControlMsg::ListSessions {
        req_id: 1,
        cursor: None,
        limit: None,
    })
    .encode(&mut input)
    .unwrap();

    let mut child = tokio::process::Command::new(farhelm_bin())
        .args(["internal", "stdio", "--state-dir"])
        .arg(state.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn stdio proxy");
    let mut stdin = child.stdin.take().expect("piped stdin");
    stdin.write_all(&input).await.expect("write frames");
    drop(stdin); // EOF: half-closes the proxy's upstream

    let out = tokio::time::timeout(Duration::from_secs(15), child.wait_with_output())
        .await
        .expect("proxy must exit once the supervisor side closes — a hang here is the bug")
        .expect("proxy output");
    assert!(out.status.success(), "proxy must exit cleanly");

    let mut rest: &[u8] = &out.stdout;
    let mut got_reply = false;
    while let Some((frame, used)) = Frame::decode(rest).expect("well-formed frames on stdout") {
        if frame.kind == FrameKind::Control
            && let Ok(ControlMsg::SessionList { req_id: 1, .. }) = parse_control(&frame)
        {
            got_reply = true;
        }
        rest = &rest[used..];
    }
    assert!(
        got_reply,
        "the reply in flight at stdin EOF must still reach stdout"
    );
}

/// A Detach from a kicked connection must not tear down the winner's
/// attachment.
///
/// The helm calls `detach` unconditionally on every terminal teardown
/// path, so after a cross-connection takeover the kicked helm's routine
/// cleanup carries the COLLIDING channel id (both connections number
/// from 1) — only the connection-identity half of the Detach guard
/// stands between that cleanup and the winner's live attachment. Its
/// siblings (input, resize) have this exact test; before this one,
/// deleting `same_channel` from the Detach arm failed nothing.
#[tokio::test]
async fn detach_from_a_kicked_connection_does_not_kill_the_winner() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (loser_chan, mut rx1) = h.client.attach(&session.id, 80, 24).await.expect("attach1");
    let mut seen1 = Vec::new();
    wait_for(&mut rx1, &mut seen1, "FAKE-AGENT READY", 20).await;

    let winner = h.second_client().await;
    let (winner_chan, mut rx2) = winner.attach(&session.id, 80, 24).await.expect("attach2");
    assert_eq!(loser_chan, winner_chan, "both connections number from 1");
    let mut seen2 = Vec::new();
    wait_for(&mut rx2, &mut seen2, "FAKE-AGENT READY", 15).await;

    // The kicked helm's routine cleanup: same channel id, wrong
    // connection. The winner's terminal must stay live — proven by a
    // fresh echo, which a torn-down forwarder can never deliver.
    h.client.detach(loser_chan).await;
    h.client
        .list_sessions()
        .await
        .expect("barrier after foreign detach");
    winner
        .send_input(winner_chan, b"survived-foreign-detach\r".to_vec())
        .await;
    wait_for(&mut rx2, &mut seen2, "survived-foreign-detach", 15).await;
}

/// Creating a session with degenerate dimensions must clamp, exactly
/// like the resize path: `new-session -x 0` is a hard tmux error, so
/// without the clamp the create fails outright — and every other test
/// creates at sane sizes, so deleting the clamp used to fail nothing.
#[tokio::test]
async fn create_with_degenerate_size_clamps_to_1x1() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();

    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script basic"),
            None,
            0,
            0,
        )
        .await
        .expect("create with 0x0 must succeed via the clamp");
    wait_for_geometry(&h, "1x1").await;

    // And the session is real, not just accepted: it must be listed.
    let listed = h.client.list_sessions().await.expect("list");
    assert!(listed.sessions.iter().any(|s| s.id == session.id));
}

/// Extract every two-hex-digit token from `hexecho`'s output, discarding
/// which line or read() call each token arrived on.
///
/// `hexecho` flushes a fresh line per raw `read()`, and reads can split
/// arbitrarily at PTY/tmux boundaries — a single input byte sequence can
/// legitimately arrive as hex tokens on two or more separate lines. A
/// prior version of this test's assertion instead required the whole
/// expected payload's hex to appear on one line, which is only true when
/// the PTY happens not to split that particular read; this reassembles
/// the byte stream in order regardless of where the line breaks fell, so
/// the assertion below holds independent of read-boundary behavior.
fn hex_tokens(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .filter(|token| token.len() == 2 && token.bytes().all(|b| b.is_ascii_hexdigit()))
        .map(|token| u8::from_str_radix(token, 16).expect("validated two ASCII hex digits"))
        .collect()
}

/// Byte-verbatim input delivery, pinned end to end through a raw-mode
/// fixture the paste-buffer bug could not hide from.
///
/// This is the regression test for the paste-buffer input-mangling bug:
/// `paste-buffer -d -r`, the mechanism this replaced, caret-escaped
/// control bytes on their way into the pane (DEL arrived as the two
/// characters `^?`, ESC as `^[`, ctrl-C as `^C` — verified against tmux
/// 3.7b) while passing every other test in this file, because `basic`'s
/// canonical-mode reading let the pty's own line discipline mask the
/// difference. `hexecho` reads its stdin in raw mode specifically so
/// nothing between the wire and this assertion can paper over a mangled
/// byte.
#[tokio::test]
async fn input_bytes_survive_verbatim_through_hexecho() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script hexecho"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    // Exactly the bytes paste-buffer was observed to mangle: DEL, ESC
    // (as the opener of the ArrowUp sequence "\x1b[A"), and ETX (ctrl-C).
    h.client
        .send_input(chan, b"a\x7fb\x1b[A\x03".to_vec())
        .await;
    // A plain printable byte with no special meaning to tmux or a
    // raw-mode pty, sent as a separate call. Its own hex line is the sync
    // point that proves the control-byte input above already made it
    // through, without depending on how `hexecho`'s read() calls happen
    // to chunk the payload into lines.
    h.client.send_input(chan, b"z".to_vec()).await;
    wait_for(&mut rx, &mut seen, "7a", 10).await;

    // Reassemble the hex byte stream across every line before asserting:
    // see `hex_tokens` for why line boundaries cannot be trusted here.
    let transcript = String::from_utf8_lossy(&seen);
    let bytes = hex_tokens(&transcript);
    let contains_sequence = |needle: &[u8]| bytes.windows(needle.len()).any(|w| w == needle);
    assert!(
        contains_sequence(&[0x61, 0x7f, 0x62, 0x1b, 0x5b, 0x41, 0x03]),
        "control bytes must arrive verbatim; transcript:\n{transcript}"
    );
    assert!(
        !contains_sequence(&[0x5e, 0x3f]),
        "DEL must not arrive caret-escaped as ^?: {transcript}"
    );
    assert!(
        !contains_sequence(&[0x5e, 0x5b]),
        "ESC must not arrive caret-escaped as ^[: {transcript}"
    );
    assert!(
        !contains_sequence(&[0x5e, 0x43]),
        "ETX (ctrl-C) must not arrive caret-escaped as ^C: {transcript}"
    );
}

/// PLAN_M2.md's headline SQLite behavior: session metadata must survive
/// the supervisor process, not just the tmux server underneath it.
///
/// A brand-new `Supervisor` on the harness's state dir stands in for a
/// restarted process — `new_with_exe` (unlike `serve()`) takes no
/// socket-exclusivity lock, so nothing here fights the harness's own
/// supervisor for the same reason `serve_refuses_a_second_supervisor_...`
/// already runs several `Supervisor`s side by side. The harness's private
/// tmux server is left running throughout (its `TmuxServerGuard` only
/// tears it down when the harness itself drops, at the end of the test),
/// which is the normal shape PLAN_M2.md describes: the tmux server
/// outliving a supervisor restart. Listing alone would not catch a bug
/// that persists metadata but loses the live reconnect, so this also
/// attaches and round-trips input through the reloaded entry.
#[tokio::test]
async fn persisted_sessions_survive_a_supervisor_restart() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let sup2 = Supervisor::new_with_exe_and_timeouts(
        h.state.path(),
        farhelm_bin().into(),
        suite_timeouts(),
    )
    .await
    .expect("second supervisor construction on the same state dir");
    let client2 = connect_client(&sup2).await;

    // The listing asserted on is the one the wait SETTLED, not a fresh
    // read taken afterwards: re-listing would put an unguarded single-shot
    // observation back in front of the equality below, which is exactly
    // what a tolerated tmux diagnostic (see `wait_for_listing`) turns into
    // a spurious `Exited` on the status field.
    let listed = wait_for_listing(
        &client2,
        30,
        "the restarted supervisor lists the session as live",
        |sessions| {
            sessions
                .iter()
                .any(|s| s.id == session.id && s.status.is_live())
        },
    )
    .await;
    let [row] = listed.as_slice() else {
        panic!("exactly one session must be listed, got {listed:?}");
    };
    assert!(
        row.status.is_live(),
        "a session whose tmux server survived the restart must still list as live"
    );
    assert_eq!(
        *row,
        with_status(session.clone(), row.status.clone()),
        "and its metadata must round-trip identically from SQLite"
    );

    let (chan, mut rx) = client2
        .attach(&session.id, 80, 24)
        .await
        .expect("attach must succeed: the tmux session is still alive");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    client2.send_input(chan, b"still-alive\r".to_vec()).await;
    wait_for(&mut rx, &mut seen, "echo:", 10).await;
    wait_for(&mut rx, &mut seen, "still-alive", 5).await;
}

/// PLAN_M2.md's "restart gap": a session whose tmux server did NOT
/// survive a supervisor restart must still be listed — the whole point of
/// persisting metadata separately from tmux liveness — but attaching to
/// it must fail loudly rather than fabricate a terminal that no longer
/// exists.
///
/// The private tmux server is killed directly on its socket, standing in
/// for "the host rebooted" or "tmux crashed independently of the
/// supervisor" — the case M1 had no answer for at all (the session simply
/// vanished from the in-memory map). The second `Supervisor` construction
/// starts a fresh, empty tmux server on the same socket (an ordinary
/// consequence of `ensure_server`'s idempotent-adopt-or-start behavior),
/// so `has_session` genuinely finds nothing for the reloaded row's
/// `tmux_name` — this is not a mocked failure.
#[tokio::test]
async fn restart_gap_lists_sessions_without_a_terminal_and_attach_fails() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let sock = h.state.path().join("tmux.sock");
    kill_tmux_server_and_wait(&sock).await;

    let sup2 = Supervisor::new_with_exe(h.state.path(), farhelm_bin().into())
        .await
        .expect("second supervisor construction after the tmux server died");
    let client2 = connect_client(&sup2).await;

    let listed = client2
        .list_sessions()
        .await
        .expect("list after restart gap");
    assert_eq!(
        listed.sessions,
        vec![with_status(
            session.clone(),
            SessionStatus::Exited { exit_code: None }
        )],
        "a session must stay listed even once its tmux server is gone — vanishing is \
         exactly what this PR exists to prevent — and the restart-gap entry (no terminal \
         at all) must list as exited with no exit code to fabricate, PLAN_M2.md's \
         restart-gap status contract"
    );

    let err = client2
        .attach(&session.id, 80, 24)
        .await
        .expect_err("attach must fail: this entry's terminal did not survive the restart");
    assert!(
        err.to_string().contains("no terminal"),
        "error must name the missing terminal, got: {err:#}"
    );
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("a terminal-less attach must carry a SupervisorError")
            .kind,
        ErrorKind::NotFound,
        "a vanished terminal is a not-found, not a bad request or server fault"
    );
}

/// A dead private tmux server must not take the whole session list down —
/// `TmuxDriver::pane_states`'s "no server running" tolerance, exercised
/// against a STILL-LIVE supervisor (no reconstruction, no restart-gap
/// reload, unlike the sibling `restart_gap_*` test above). `session` here
/// is tracked with a live `terminal: Some(..)` in this process's own map,
/// so this is exactly the case real-stack dogfooding found: the private
/// tmux server dying (crash, OOM, an operator killing it) while the
/// supervisor keeps running.
///
/// This PINS THE OPPOSITE of what this test asserted before this change:
/// an earlier version required `list_sessions` to fail here, reasoning
/// that reporting every tracked session `Exited` off a "fabricated" empty
/// pane-states map would be indistinguishable from an honestly observed
/// mass exit. That conflated two different things. An empty pane-states
/// MAP is not an empty session LISTING — `pane_states`'s return value
/// plays no part in WHICH rows `ListSessions` selects for its reply (the
/// session cap and byte budget decide that, independent of tmux
/// entirely); the map only ever feeds `session_status`'s per-entry
/// liveness lookup for whichever rows that selection already kept. And
/// `"no server running"` is not a guess: it is tmux's own DEFINITIVE
/// statement that no pane exists anywhere on this socket, so reporting
/// every terminal-bearing entry as gone is accurate reporting, not
/// fabrication — the same honest `Exited { exit_code: None }` a
/// restart-gap row already gets. The old behavior instead turned a dead
/// tmux server into a hard `ListSessions` failure: every session
/// unreachable THROUGH THE UI (which has no session ids left to act on,
/// including for delete, once the list that would supply them fails to
/// load) even though every one of them was intact in SQLite and
/// `DeleteSession`'s own handler was never itself refused. `TmuxDriver::
/// pane_states`'s own docs carry the full version of this reasoning.
///
/// The connection must also stay usable afterward: proven here by a
/// SECOND, genuinely different request (creating a fresh session)
/// succeeding right after the first request observed the dead server — a
/// repeat of the identical `list_sessions` call would only prove that one
/// request shape still works, not that the connection generally still
/// serves.
///
/// This does NOT attempt to restart or resurrect the vanished tmux server
/// — recovery is M3 (PLAN.md). Until then the session simply reports
/// `Exited`: a plain supervisor restart would reload its row
/// terminal-less (the ordinary restart-gap case), still `Exited`, not
/// "recovered" — there is no plain-restart path back to a live status for a
/// session whose tmux is actually gone.
#[tokio::test]
async fn list_sessions_survives_when_the_tmux_server_is_gone() {
    let h = harness().await;
    let (session, work) = basic_session(&h).await;

    let sock = h.state.path().join("tmux.sock");
    kill_tmux_server_and_wait(&sock).await;

    let expected = vec![with_status(
        session,
        SessionStatus::Exited { exit_code: None },
    )];
    let listed = h
        .client
        .list_sessions()
        .await
        .expect("list_sessions must succeed even once the private tmux server is gone");
    assert_eq!(
        listed.sessions, expected,
        "a session tracked with a live terminal must still be listed — never dropped — and \
         must report the same honest 'terminal gone' status a restart-gap row gets, since a \
         vanished tmux server makes that a definitive fact rather than a guess"
    );

    h.client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script basic"),
            None,
            80,
            24,
        )
        .await
        .expect(
            "the connection must stay usable for an unrelated request after one ListSessions \
             observed the dead tmux server",
        );
}

// No sibling test provokes a NON-"no server running" `pane_states`
// failure at this end-to-end layer: every other tmux failure this
// module's own tolerance list (and the one above it) does not cover
// would need something this harness has no honest way to arrange — a
// malformed or corrupted tmux invocation, a permissions failure on the
// socket, a tmux binary that emits an unrecognized diagnostic — without
// resorting to fault injection this test suite does not otherwise use
// (mocking or wrapping the tmux binary itself). Rather than invent a fake
// seam for it, that classification is pinned at the unit level instead:
// see `farhelm-supervisor`'s `tmux.rs`,
// `is_tolerated_list_panes_diagnostic_pins_all_three_tolerated_cases`
// (plus its path-embedding sibling), which exercise every diagnostic
// outcome directly against constructed stderr: the three tolerated ones
// (a running-but-empty server, an absent server, and a server caught
// mid-teardown) and the unclassified failure that must still propagate.

/// `session_status`'s pane-identity contract (`service.rs`): pane ids
/// reset to `%0` on a FRESH tmux server (verified empirically — killing
/// the server and creating a new session hands its first pane `%0`
/// again), so a stale, never-reloaded `SessionEntry` whose OLD pane
/// happened to be `%0` must not silently inherit a brand-new, unrelated
/// session's liveness just because the two share that recycled number.
///
/// Deliberately NOT the restart-gap case (the `restart_gap_*` tests):
/// the whole tmux server is killed and a SECOND session created on this
/// SAME live process, without ever reconstructing the `Supervisor`
/// (which would instead reload `terminal: None` for the dead row via
/// `has_session`). `old_session` is the very first session this harness
/// creates, so its pane is genuinely `%0`; killing the server and
/// creating `new_session` right after gives it the exact same number on
/// the freshly auto-started replacement server. Matching pane id alone
/// would let `old_session` read as live off of `new_session`'s real
/// liveness; `session_status`'s `session_name` cross-check
/// (`TmuxDriver::pane_states`'s `#{session_name}` field) is what tells
/// these two same-numbered panes apart.
#[tokio::test]
async fn stale_pane_id_after_server_restart_does_not_inherit_a_new_sessions_status() {
    let h = harness().await;
    let (old_session, _work1) = basic_session(&h).await;
    let sock = h.state.path().join("tmux.sock");
    let old_pane_id = pane_id_of(&sock, &format!("fh-{}", old_session.id)).await;

    kill_tmux_server_and_wait(&sock).await;

    // A brand-new session on the SAME live supervisor: tmux auto-starts a
    // fresh server for the socket (no `-N` flag anywhere in this
    // module — see `TmuxDriver::command`), whose pane-id counter starts
    // back at `%0`, the same number `old_session`'s terminal remembers.
    let (new_session, _work2) = basic_session(&h).await;
    let new_pane_id = pane_id_of(&sock, &format!("fh-{}", new_session.id)).await;
    assert_eq!(
        old_pane_id, new_pane_id,
        "test precondition: the old and new sessions must actually share the same recycled \
         pane id, or this test is not exercising the cross-check it claims to — if tmux's \
         pane-id-reset behavior ever changed, this assertion is what would catch it rather \
         than the test silently passing for an unrelated reason"
    );

    // Both rows are read out of ONE settled listing. The discrimination
    // under test is between two entries of the same reply — "this pane id
    // is alive for the new session and dead for the old one" is a single
    // claim about a single observation — so the wait's predicate is what
    // picks the reply, and neither row is re-read afterwards. Waiting is
    // also what keeps a tolerated tmux diagnostic (an empty pane map, see
    // `wait_for_listing`) from failing the live half right after the
    // server was killed and restarted underneath it.
    let listed = wait_for_listing(
        &h.client,
        30,
        "the new session lists as live on its recycled pane id",
        |sessions| {
            sessions
                .iter()
                .any(|s| s.id == new_session.id && s.status.is_live())
        },
    )
    .await;
    let find = |id: &str| {
        listed
            .iter()
            .find(|s| s.id == id)
            .cloned()
            .unwrap_or_else(|| panic!("session {id} missing from the list"))
    };
    assert_eq!(
        find(&old_session.id).status,
        SessionStatus::Exited { exit_code: None },
        "the old session's tmux is really gone; it must not inherit the new session's \
         liveness just because both happen to reuse pane %0"
    );
    assert!(
        find(&new_session.id).status.is_live(),
        "the new session's own pane really is alive"
    );
}

/// The wedge the four lifecycle regressions below all start from: one
/// session whose recorded pane id has been taken over by a DIFFERENT
/// session on a replacement tmux server.
///
/// This is the 2026-08-16 production incident reduced to its mechanism.
/// The private tmux server segfaulted while the supervisor kept running;
/// nothing refreshes in-memory `Terminal` rows mid-flight, so the wedged
/// session went on pointing at `%0` while the replacement server — whose
/// pane counter restarts from scratch — handed that same `%0` to the next
/// session created. Every lifecycle verb for the wedged session then
/// refused, because `pane_process` used to treat a session mismatch as a
/// hard error, and the session stayed unusable until the supervisor was
/// restarted.
///
/// Built exactly like `stale_pane_id_after_server_restart_does_not_inherit_a_new_sessions_status`,
/// which pins the sibling half of the contract (a stale pane must not
/// inherit the new session's STATUS): the old session is the first this
/// harness creates, so its pane really is `%0`; the server is killed; and
/// a second session on the SAME live supervisor lands on the auto-started
/// replacement's `%0`.
struct RecycledPaneWedge {
    h: Harness,
    /// The wedged session: its `Terminal` still records a pane that now
    /// belongs to `new`.
    old: SessionInfo,
    /// The innocent bystander that inherited the recycled pane id. Every
    /// lifecycle test below asserts this one comes through untouched — a
    /// fix that reached for the foreign pane's pid would reap THIS
    /// session's tree. It doubles as the discriminator's positive case:
    /// because the supervisor knows it, its name is what proves the
    /// wedged session's recorded pane is a RECYCLE rather than a rename.
    new: SessionInfo,
    /// The recycled id itself, so assertions can name it rather than
    /// hard-coding `%0`.
    pane: String,
    /// The sessions' working directories, which must outlive their
    /// launches (they are the sessions' cwds).
    _work_old: tempfile::TempDir,
    _work_new: tempfile::TempDir,
}

/// Construct the wedge, asserting its own precondition.
///
/// The precondition assertion is not optional decoration: if tmux ever
/// stopped restarting its pane-id counter with the server, every test
/// below would keep passing while exercising nothing at all. This is
/// what fails instead.
async fn recycled_pane_wedge() -> RecycledPaneWedge {
    let h = harness().await;
    let (old, _work_old) = basic_session(&h).await;
    let sock = h.state.path().join("tmux.sock");
    let old_pane_id = pane_id_of(&sock, &format!("fh-{}", old.id)).await;

    kill_tmux_server_and_wait(&sock).await;

    let (new, _work_new) = basic_session(&h).await;
    let new_pane_id = pane_id_of(&sock, &format!("fh-{}", new.id)).await;
    assert_eq!(
        old_pane_id, new_pane_id,
        "test precondition: the wedge only exists if the two sessions really share a recycled \
         pane id — without that, these tests exercise an ordinary dead-pane path and prove \
         nothing about the incident"
    );

    RecycledPaneWedge {
        h,
        old,
        new,
        pane: old_pane_id,
        _work_old,
        _work_new,
    }
}

/// Deleting a session whose recorded pane was recycled onto another
/// session must SUCCEED — the headline fix for the 2026-08-16 incident,
/// where delete was the verb the operator actually needed and could not
/// use.
///
/// Also pins the driver-level contract the other three regressions rest
/// on: `pane_process` classifies the recycled id as a foreign owner and
/// names that owner, rather than erroring. Delete recognizes that owner
/// as another of its own sessions, reads the pane as "no live root", and
/// falls back to the marker sweep and the session's cgroup scopes — which
/// is why the bystander's process tree is untouched: nothing here ever
/// reads the stranger's pid.
#[tokio::test]
async fn delete_succeeds_when_the_recorded_pane_was_recycled_onto_another_session() {
    let w = recycled_pane_wedge().await;

    // The driver-level classification, asked directly rather than
    // inferred from the lifecycle verb's success: a delete that started
    // succeeding for some unrelated reason would otherwise look like this
    // fix working.
    let driver = farhelm_supervisor::tmux::TmuxDriver::new(w.h.state.path());
    let probe = driver
        .pane_process(&format!("fh-{}", w.old.id), &w.pane)
        .await
        .expect("probing a recycled pane id is an answer, not a failure");
    assert_eq!(
        probe,
        farhelm_supervisor::tmux::PaneProbe::ForeignOwner {
            owner: format!("fh-{}", w.new.id)
        },
        "the probe must name the session that now owns the pane, so callers can log the \
         incident shape and refuse to touch that pid"
    );

    w.h.client
        .delete_session(&w.old.id)
        .await
        .expect("delete must tolerate a pane id recycled onto another session");

    let listed = wait_for_listing(
        &w.h.client,
        30,
        "the deleted session is gone and the bystander is still live",
        |sessions| {
            sessions.iter().all(|s| s.id != w.old.id)
                && sessions
                    .iter()
                    .any(|s| s.id == w.new.id && s.status.is_live())
        },
    )
    .await;
    assert_eq!(
        listed.len(),
        1,
        "exactly the bystander must remain: {listed:?}"
    );
    assert_eq!(
        pane_id_of(
            &w.h.state.path().join("tmux.sock"),
            &format!("fh-{}", w.new.id)
        )
        .await,
        w.pane,
        "and it must still hold the recycled pane itself — a delete that killed the pane it \
         found would have taken down the wrong session's terminal"
    );
}

/// Restarting a session whose recorded pane was recycled must succeed,
/// WITHOUT stop consent, and build a FRESH terminal.
///
/// The incident wedged restart alongside delete. Two things are pinned
/// here, and the consent half is the one a reader is most likely to get
/// wrong. `stop_if_running: false` is what the UI actually sends for a
/// session it lists as exited, so a recycled pane classified as "maybe a
/// live agent" would turn into a `Conflict` demanding confirmation for an
/// agent that provably died with its tmux server — the wedge in a new
/// costume. The terminal half is the other: the recorded pane did not
/// survive, so `terminal_survives` must be false and the relaunch must
/// build its own terminal rather than attempt a reuse tmux would reject
/// outright (`relaunch_in_pane` targets session and pane together).
#[tokio::test]
async fn restart_succeeds_when_the_recorded_pane_was_recycled_onto_another_session() {
    let w = recycled_pane_wedge().await;
    let sock = w.h.state.path().join("tmux.sock");

    let restarted =
        w.h.client
            .restart_session(&w.old.id, farhelm_proto::RestartMode::Fresh, false)
            .await
            .expect(
                "restart must tolerate a pane id recycled onto another session, and must not \
                 demand stop consent for an agent that died with its server",
            );
    assert_eq!(restarted.id, w.old.id);

    wait_for_live_status(&w.h.client, &w.old.id, 30).await;
    let restarted_pane = pane_id_of(&sock, &format!("fh-{}", w.old.id)).await;
    assert_ne!(
        restarted_pane, w.pane,
        "the relaunch must own a new pane on the current server, not the one another session \
         holds"
    );
    assert_eq!(
        pane_id_of(&sock, &format!("fh-{}", w.new.id)).await,
        w.pane,
        "and the bystander must keep the pane it legitimately owns"
    );
    let listed = wait_for_listing(
        &w.h.client,
        30,
        "both the restarted session and the bystander are live",
        |sessions| {
            sessions
                .iter()
                .filter(|s| (s.id == w.old.id || s.id == w.new.id) && s.status.is_live())
                .count()
                == 2
        },
    )
    .await;
    assert_eq!(listed.len(), 2, "no session may have been lost: {listed:?}");
}

/// Stopping a session whose recorded pane was recycled must record the
/// dead-or-absent classification rather than erroring.
///
/// Stop is the verb whose refusal is least defensible in the incident
/// state: there is provably nothing of this session's running in that
/// pane, so "I cannot tell you" was never the honest answer. `Exited` is
/// — and the marker sweep that runs on the same path is what still
/// collects any survivor of the pre-crash run.
#[tokio::test]
async fn stop_succeeds_when_the_recorded_pane_was_recycled_onto_another_session() {
    let w = recycled_pane_wedge().await;

    w.h.client
        .stop_session(&w.old.id)
        .await
        .expect("stop must tolerate a pane id recycled onto another session");

    let listed = wait_for_listing(
        &w.h.client,
        30,
        "the stopped session reads as exited while the bystander stays live",
        |sessions| {
            sessions.iter().any(|s| {
                s.id == w.old.id && matches!(s.status, SessionStatus::Exited { exit_code: None })
            }) && sessions
                .iter()
                .any(|s| s.id == w.new.id && s.status.is_live())
        },
    )
    .await;
    assert_eq!(listed.len(), 2, "no session may have been lost: {listed:?}");
    assert_eq!(
        pane_id_of(
            &w.h.state.path().join("tmux.sock"),
            &format!("fh-{}", w.new.id)
        )
        .await,
        w.pane,
        "and the bystander must still hold the recycled pane: a stop that walked the foreign \
         pid would have killed its agent"
    );
}

/// Archiving a session whose recorded pane was recycled must succeed.
///
/// Archive shares delete's teardown shape but publishes a retained row
/// instead of removing one, so it needs its own coverage: the archive
/// flag is committed only after every process and terminal artifact is
/// gone, which means a probe refusal left the session neither archived
/// nor cleanly deletable.
#[tokio::test]
async fn archive_succeeds_when_the_recorded_pane_was_recycled_onto_another_session() {
    let w = recycled_pane_wedge().await;

    let archived =
        w.h.client
            .archive_session(&w.old.id)
            .await
            .expect("archive must tolerate a pane id recycled onto another session");
    assert!(
        archived.archived,
        "the archive flag is committed only after teardown finished: {archived:?}"
    );

    // An archived session stays LISTED (archiving retains the row, it
    // does not remove it) — what changes is the flag and the settled
    // exited status, which is why this asserts on the row rather than on
    // its absence.
    let listed = wait_for_listing(
        &w.h.client,
        30,
        "the archived session settles as an archived, exited row while the bystander stays live",
        |sessions| {
            sessions.iter().any(|s| {
                s.id == w.old.id
                    && s.archived
                    && matches!(s.status, SessionStatus::Exited { exit_code: None })
            }) && sessions
                .iter()
                .any(|s| s.id == w.new.id && s.status.is_live())
        },
    )
    .await;
    assert_eq!(listed.len(), 2, "no session may have been lost: {listed:?}");
    assert_eq!(
        pane_id_of(
            &w.h.state.path().join("tmux.sock"),
            &format!("fh-{}", w.new.id)
        )
        .await,
        w.pane,
        "and the bystander must still hold the recycled pane"
    );
}

/// `pane_process` must report the owning session's FULL name, even when
/// that name begins with exactly the name the caller asked about.
///
/// This is the parse contract the whole foreign-owner policy stands on,
/// and it was broken in the first cut of this change: the tmux format put
/// `#{session_name}` FIRST and the parser split on whitespace, so a name
/// like `fh-<id> trailing` matched the expected session on its first
/// token and then wedged on `trailing` as a pane pid — a parse error,
/// which every lifecycle verb treats as a hard failure. A poisoned name
/// could therefore re-create the very wedge this change exists to remove,
/// and — worse in the other direction — a name that merely PREFIX-matched
/// could be mistaken for our own session. tmux allows spaces in session
/// names and the private socket is reachable by whoever owns the account,
/// so neither is hypothetical.
///
/// Built on the recycled-pane wedge only because it is the cheapest way
/// to get a pane owned by a session other than the one being asked about;
/// nothing here depends on the recycle itself.
#[tokio::test]
async fn the_pane_probe_reports_a_spaced_owner_name_whole() {
    let w = recycled_pane_wedge().await;
    let sock = w.h.state.path().join("tmux.sock");
    let expected_owner = format!("fh-{} trailing", w.old.id);

    // The bystander — which really does hold the recycled pane — takes on
    // a name whose FIRST TOKEN is exactly what the probe below asks
    // about. Anything that reads the owner one token at a time now sees a
    // match where there is none.
    let renamed = tmux_query(
        &sock,
        &[
            "rename-session",
            "-t",
            &format!("fh-{}", w.new.id),
            &expected_owner,
        ],
    )
    .await;
    assert!(
        renamed.status.success(),
        "test setup: rename-session must succeed, got: {}",
        String::from_utf8_lossy(&renamed.stderr)
    );

    let driver = farhelm_supervisor::tmux::TmuxDriver::new(w.h.state.path());
    let probe = driver
        .pane_process(&format!("fh-{}", w.old.id), &w.pane)
        .await
        .expect("a poisoned owner name is an answer to classify, not a query failure");
    assert_eq!(
        probe,
        farhelm_supervisor::tmux::PaneProbe::ForeignOwner {
            owner: expected_owner
        },
        "the probe must carry the whole owner name: truncating it at the first space would \
         both destroy the caller's ability to recognize the owner and, in this case, make the \
         pane look like the asking session's own"
    );
}

/// The restart-gap decision is PER SESSION, not one answer applied to the
/// whole reloaded batch.
///
/// Two sessions exist; only one's tmux session is killed directly (the
/// other, and the private tmux server itself, are left untouched). An
/// implementation that probes `has_session` once and reuses the answer
/// for every row — or that otherwise conflates "the server is gone" with
/// "this one session is gone" — would either lose the live session too or
/// wrongly keep the dead one attachable; this test fails either way,
/// which is exactly the coverage gap a single-session restart-gap test
/// cannot close.
#[tokio::test]
async fn restart_gap_is_decided_per_session() {
    let h = harness().await;
    let (alive_session, _work1) = basic_session(&h).await;
    let (dead_session, _work2) = basic_session(&h).await;

    // Mirrors `create_session`'s own derivation (`service.rs`): the tmux
    // session name is `fh-` plus the FULL session id (not a truncated
    // prefix — see that call site for why a prefix is unsafe).
    let dead_tmux_name = format!("fh-{}", dead_session.id);
    let sock = h.state.path().join("tmux.sock");
    let killed = tmux_query(&sock, &["kill-session", "-t", &dead_tmux_name]).await;
    assert!(
        killed.status.success(),
        "test setup: kill-session must succeed, got: {}",
        String::from_utf8_lossy(&killed.stderr)
    );

    let sup2 = Supervisor::new_with_exe_and_timeouts(
        h.state.path(),
        farhelm_bin().into(),
        suite_timeouts(),
    )
    .await
    .expect("second supervisor construction after one session's tmux died");
    let client2 = connect_client(&sup2).await;

    // One settled listing carries both rows, for the same reason as the
    // sibling test above: the assertion is an equality over the WHOLE
    // reply, so it has to be the reply the wait accepted rather than a
    // fresh single-shot read that a tolerated tmux diagnostic (see
    // `wait_for_listing`) could catch mid-degradation on the live half.
    let mut listed = wait_for_listing(
        &client2,
        30,
        "the surviving session lists as live after a partial restart gap",
        |sessions| {
            sessions
                .iter()
                .any(|s| s.id == alive_session.id && s.status.is_live())
        },
    )
    .await;
    listed.sort_by(|a, b| a.id.cmp(&b.id));
    let find = |id: &str| {
        listed
            .iter()
            .find(|s| s.id == id)
            .unwrap_or_else(|| panic!("session {id} missing from the list: {listed:?}"))
    };
    // The surviving session's live status is asserted as LIVENESS while
    // the dead one's is asserted exactly: which of running/waiting/idle a
    // healthy agent reports is the sampler's call, but "exited with no
    // code" is a specific claim about a session whose tmux really is gone.
    let survivor = find(&alive_session.id);
    assert!(
        survivor.status.is_live(),
        "the session whose tmux survived must still list as live: {survivor:?}"
    );
    assert_eq!(
        *survivor,
        with_status(alive_session.clone(), survivor.status.clone()),
        "and the rest of its metadata must round-trip unchanged"
    );
    assert_eq!(
        *find(&dead_session.id),
        with_status(
            dead_session.clone(),
            SessionStatus::Exited { exit_code: None },
        ),
        "only the one whose tmux session actually died must list as exited"
    );
    assert_eq!(
        listed.len(),
        2,
        "both sessions must remain listed regardless of which one's terminal died"
    );

    let (chan, mut rx) = client2
        .attach(&alive_session.id, 80, 24)
        .await
        .expect("the untouched session must still attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    client2.send_input(chan, b"still-alive\r".to_vec()).await;
    wait_for(&mut rx, &mut seen, "echo:", 10).await;
    wait_for(&mut rx, &mut seen, "still-alive", 5).await;

    let err = client2
        .attach(&dead_session.id, 80, 24)
        .await
        .expect_err("the killed session's attach must fail");
    assert!(
        err.to_string().contains("no terminal"),
        "error must name the missing terminal, got: {err:#}"
    );
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("a terminal-less attach must carry a SupervisorError")
            .kind,
        ErrorKind::NotFound,
        "a vanished terminal is a not-found, not a bad request or server fault"
    );
}

// Not tested here: a DB write failure during `create_session` (the
// kill-the-just-created-tmux-session unwind path). Reproducing it needs
// fault injection into the SQLite connection or the filesystem beneath
// it, which M3 is expected to bring a seam for (PLAN.md's milestone
// ladder). A filesystem hack (a read-only database file, say) would buy
// little signal this far ahead of that seam existing, so it is skipped
// rather than improvised. The unwind logic itself — kill tmux, still
// return the DB error — is covered by code review and the ordinary
// create-path tests exercising the happy side of the same call.

/// The acceptance test for process-tree stop (PLAN_M2.md step 4 / M2
/// acceptance criterion 2): stopping a session must kill not just the
/// agent but every descendant it spawned, three levels deep — the
/// spawner process itself, the `sh` it forks, and the `sleep` `sh` forks
/// in turn. The `spawner` fixture exists exactly for this — a plain
/// script has nothing whose death would prove tree-kill rather than
/// single-process kill.
///
/// Also covers stop's other headline properties in the same run: the
/// session stays listed (both through this process's own client AND a
/// FRESH `Supervisor` on the same state dir, which is what actually
/// proves the DB row survived rather than merely this process's
/// in-memory map), and a fresh attach still works and replays the
/// pre-stop scrollback — stop leaves the terminal viewable, it does not
/// tear anything down.
#[tokio::test]
async fn stop_kills_the_whole_process_tree() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    let self_pid = extract_pid(&seen, "SELF-PID:");
    let child_pid = extract_pid(&seen, "CHILD-PID:");
    assert_ne!(
        self_pid, child_pid,
        "test fixture must report two distinct pids"
    );
    let grandchild_pid = wait_for_child(child_pid, 10).await;

    h.client.stop_session(&session.id).await.expect("stop");

    wait_until_pid_gone(self_pid, 15).await;
    wait_until_pid_gone(child_pid, 15).await;
    wait_until_pid_gone(grandchild_pid, 15).await;

    // Status is computed fresh from tmux at list time, not pushed, so the
    // pane's `pane_dead` flag flipping and this list call race each
    // other — this polls for the EVENTUAL exited classification rather
    // than asserting on a single read that might land before the flip.
    // What is under test is only that the session ends up classified
    // `Exited`, not which exact code it carries: PLAN_M2.md's status test
    // list, item (e), says "assert exited, don't over-pin the code" —
    // a SIGKILL death's `pane_dead_status` is not pinned to one value
    // across tmux versions, so the code is deliberately left unasserted.
    let found = wait_for_non_live_status(&h.client, &session.id, 15).await;
    assert_eq!(found.id, session.id);
    assert_eq!(found.title, session.title);
    assert_eq!(found.cwd, session.cwd);
    assert_eq!(found.invocation, session.invocation);
    assert!(
        matches!(found.status, SessionStatus::Exited { .. }),
        "a stopped session must list as exited, got {:?}",
        found.status
    );

    // A fresh Supervisor on the same state dir is what actually proves
    // the row survived in SQLite, not just this process's own map — the
    // same reasoning `persisted_sessions_survive_a_supervisor_restart`
    // applies to create.
    let sup2 = Supervisor::new_with_exe(h.state.path(), farhelm_bin().into())
        .await
        .expect("second supervisor construction on the same state dir");
    let client2 = connect_client(&sup2).await;
    let listed2 = client2
        .list_sessions()
        .await
        .expect("list from fresh supervisor");
    assert_eq!(
        listed2.sessions.len(),
        1,
        "a stopped session's row must survive a supervisor restart"
    );
    assert_eq!(listed2.sessions[0].id, session.id);
    assert!(
        matches!(listed2.sessions[0].status, SessionStatus::Exited { .. }),
        "the row's session is still dead after the restart too, got {:?}",
        listed2.sessions[0].status
    );

    h.client.detach(chan).await;
    let (_chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("a stopped session's terminal must still be attachable");
    let mut replay = Vec::new();
    wait_for(&mut rx2, &mut replay, "SELF-PID", 15).await;
}

/// The SIGKILL half of `kill_process_tree`'s sequence: a child that traps
/// and discards SIGTERM must still die, because the sweep escalates to
/// SIGSTOP-quiesce and then SIGKILL rather than giving up once SIGTERM
/// alone fails. The `spawner-stubborn` fixture's child would survive
/// forever under a SIGTERM-only kill, so its death here is what pins the
/// escalation actually runs, not just that SIGTERM is sent.
///
/// Waits for `stubborn-ready` (written by the child itself, AFTER
/// installing the trap) before stopping — without that wait, a stop
/// racing the child's own startup could catch it before `trap ''` has
/// run, and SIGTERM would kill it the ordinary way, silently defeating
/// the point of this test.
#[tokio::test]
async fn stop_kills_a_child_that_ignores_sigterm() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner-stubborn"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    let self_pid = extract_pid(&seen, "SELF-PID:");
    let child_pid = extract_pid(&seen, "CHILD-PID:");
    wait_for_file(&work.path().join("stubborn-ready"), 10).await;

    h.client.stop_session(&session.id).await.expect("stop");

    wait_until_pid_gone(self_pid, 15).await;
    wait_until_pid_gone(child_pid, 15).await;
}

/// The whole point of spawning `ListSessions`/`StopSession`/
/// `DeleteSession` (`service.rs`'s `handle_control`, per those arms' own
/// comments): a slow one in flight must not stall a cheap, unrelated
/// request on the SAME connection behind it. `stop_session` against a
/// `spawner` session is the slow one here — `kill_process_tree`'s grace
/// period alone is half a second, before quiesce and kill-confirmation
/// even start — and an unknown-session `attach` is about as cheap as a
/// request gets: one lock-guarded map lookup, no tmux call at all.
///
/// Reverting the handlers to plain inline `await`s would fail this: the
/// connection's single serial read loop would not even read the attach
/// request's frame off the wire until the stop request ahead of it had
/// been handled to completion, let alone reply to it first.
#[tokio::test]
async fn cheap_request_completes_before_a_slow_spawned_handler_in_flight() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    // Kick off the slow stop without awaiting it yet.
    let stop_client = Arc::clone(&h.client);
    let stop_session_id = session.id.clone();
    let stop_done = Arc::new(AtomicBool::new(false));
    let stop_done_writer = Arc::clone(&stop_done);
    let stop_task = tokio::spawn(async move {
        stop_client
            .stop_session(&stop_session_id)
            .await
            .expect("stop");
        stop_done_writer.store(true, Ordering::SeqCst);
    });

    // Give the stop request time to actually be dispatched and its kill
    // sweep started (well inside its 500ms grace period) before firing
    // the cheap request — otherwise this could race the connection's own
    // read loop picking up the stop frame at all, rather than exercising
    // the "already in flight" scenario this test is about.
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        !stop_done.load(Ordering::SeqCst),
        "test setup: the slow stop must still be in flight at this point"
    );

    let cheap_result = h.client.attach("definitely-not-a-session", 80, 24).await;
    assert!(
        cheap_result.is_err(),
        "an unknown-session attach must still fail fast"
    );
    assert!(
        !stop_done.load(Ordering::SeqCst),
        "the cheap request must complete WHILE the slow stop is still in flight"
    );

    stop_task.await.expect("stop task panicked");
}

/// Stop must be idempotent both in the ordinary sense (calling it twice on
/// a live session) and across the restart gap (a session whose terminal
/// never came back has nothing running, so "make sure nothing is running"
/// already holds).
#[tokio::test]
async fn stop_is_idempotent() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    h.client
        .stop_session(&session.id)
        .await
        .expect("first stop");
    h.client
        .stop_session(&session.id)
        .await
        .expect("second stop on an already-stopped session must also succeed");

    // A restart-gap (terminal-less) session, mirroring
    // `restart_gap_lists_sessions_without_a_terminal_and_attach_fails`.
    let (gap_session, _work2) = basic_session(&h).await;
    let gap_tmux_name = format!("fh-{}", gap_session.id);
    let sock = h.state.path().join("tmux.sock");
    let killed = tmux_query(&sock, &["kill-session", "-t", &gap_tmux_name]).await;
    assert!(
        killed.status.success(),
        "test setup: kill-session must succeed, got: {}",
        String::from_utf8_lossy(&killed.stderr)
    );
    let sup2 = Supervisor::new_with_exe(h.state.path(), farhelm_bin().into())
        .await
        .expect("second supervisor construction after one session's tmux died");
    let client2 = connect_client(&sup2).await;
    client2
        .stop_session(&gap_session.id)
        .await
        .expect("stopping a terminal-less session must succeed: nothing can be running");
}

/// Unknown ids are the one failure mode stop and delete share, and both
/// must report it the same way `Attach` does.
#[tokio::test]
async fn stop_unknown_session_is_not_found() {
    let h = harness().await;
    let err = h
        .client
        .stop_session("does-not-exist")
        .await
        .expect_err("stop of an unknown session must fail");
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("must carry a SupervisorError")
            .kind,
        ErrorKind::NotFound
    );
}

/// See `stop_unknown_session_is_not_found`.
#[tokio::test]
async fn delete_unknown_session_is_not_found() {
    let h = harness().await;
    let err = h
        .client
        .delete_session("does-not-exist")
        .await
        .expect_err("delete of an unknown session must fail");
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("must carry a SupervisorError")
            .kind,
        ErrorKind::NotFound
    );
}

/// Delete must remove all three of a session's traces: the in-memory
/// entry, the tmux session backing its terminal, and the SQLite row —
/// the last checked through a SECOND, independent `Supervisor` on the same
/// state dir, exactly like `create_in_missing_directory_errors` does for
/// creation, since only that proves the row is really gone rather than
/// merely absent from this one process's map.
#[tokio::test]
async fn delete_removes_session_terminal_and_row() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let tmux_name = format!("fh-{}", session.id);

    h.client.delete_session(&session.id).await.expect("delete");

    assert!(
        h.client.list_sessions().await.unwrap().sessions.is_empty(),
        "a deleted session must not stay listed"
    );

    let sock = h.state.path().join("tmux.sock");
    let out = tmux_query(&sock, &["has-session", "-t", &format!("={tmux_name}")]).await;
    assert!(
        !out.status.success(),
        "the tmux session backing a deleted session's terminal must be gone"
    );

    let sup2 = Supervisor::new_with_exe(h.state.path(), farhelm_bin().into())
        .await
        .expect("second supervisor construction on the same state dir");
    let client2 = connect_client(&sup2).await;
    assert!(
        client2.list_sessions().await.unwrap().sessions.is_empty(),
        "the row must really be gone, not just absent from the original process's map"
    );
}

/// Deleting a session out from under an attached client must detach it
/// with an explicit notice rather than leaving its stream hanging —
/// mirroring how `second_attach_detaches_first` asserts a takeover's
/// `Detached` event.
#[tokio::test]
async fn delete_while_attached_detaches_the_client() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    let deleter = h.second_client().await;
    deleter
        .delete_session(&session.id)
        .await
        .expect("delete from a second client");

    let detached = tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(ev) = rx.recv().await {
            if let TermEvent::Detached(reason) = ev {
                return reason;
            }
        }
        panic!("attached client's stream ended without a Detached event");
    })
    .await
    .expect("timed out waiting for Detached after delete");
    assert!(
        detached.contains("deleted"),
        "detach reason should say the session was deleted, got: {detached}"
    );
}

/// Delete must work on a restart-gap (terminal-less) session too — SPEC.md
/// promises delete "in any state" — mirroring the restart-gap setup in
/// `restart_gap_lists_sessions_without_a_terminal_and_attach_fails`.
#[tokio::test]
async fn delete_works_on_a_terminal_less_session() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let sock = h.state.path().join("tmux.sock");
    kill_tmux_server_and_wait(&sock).await;

    let sup2 = Supervisor::new_with_exe(h.state.path(), farhelm_bin().into())
        .await
        .expect("second supervisor construction after the tmux server died");
    let client2 = connect_client(&sup2).await;

    client2
        .delete_session(&session.id)
        .await
        .expect("delete on a terminal-less session must succeed");
    assert!(
        client2.list_sessions().await.unwrap().sessions.is_empty(),
        "a deleted session must not stay listed"
    );
}

/// Delete's process-tree reaping is the same `kill_process_tree` stop
/// uses, but exercised on its own path (delete's handler, not stop's) and
/// down to the same three-level chain — every discovered descendant
/// (agent, its `sh` child, that child's own `sleep`) must actually be
/// gone once delete returns, not merely the tmux session removed around
/// them.
#[tokio::test]
async fn delete_kills_the_whole_process_tree() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    let self_pid = extract_pid(&seen, "SELF-PID:");
    let child_pid = extract_pid(&seen, "CHILD-PID:");
    let grandchild_pid = wait_for_child(child_pid, 10).await;

    h.client.delete_session(&session.id).await.expect("delete");

    wait_until_pid_gone(self_pid, 15).await;
    wait_until_pid_gone(child_pid, 15).await;
    wait_until_pid_gone(grandchild_pid, 15).await;
}

/// Stop must leave an existing attachment exactly as it was: no
/// unexpected `Detached`, and the attachment stays a normal, kickable one
/// — a second client attaching afterwards must produce the ordinary
/// takeover notice on the first, proving stop did not itself already
/// tear the attachment down or leave it in some half-detached state.
#[tokio::test]
async fn stop_does_not_disturb_the_existing_attachment() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (_chan1, mut rx1) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen1 = Vec::new();
    wait_for(&mut rx1, &mut seen1, "FAKE-AGENT READY", 20).await;

    h.client.stop_session(&session.id).await.expect("stop");

    // Give stop a moment, then require that nothing unexpected arrived on
    // the existing attachment: no Detached (stop must not touch it) and
    // no closed stream. The three arms below are all ordinary: trailing
    // pre-stop output racing the agent's death; a still-open connection
    // with nothing new; and a catch-up marker (PLAN_M5.md item 4) queued
    // right behind the replay that contained "FAKE-AGENT READY" — `wait_for`
    // above returns the instant its needle appears, without necessarily
    // having drained everything already queued behind it, so finding the
    // marker here is the ordinary shape of a healthy attach, not a race.
    // The marker's own ordering contract has its coverage elsewhere
    // (replay_marker.rs; farhelm-helm's client.rs/lib.rs) — this test's
    // only concern is that stop does not itself disturb the attachment.
    match tokio::time::timeout(Duration::from_millis(500), rx1.recv()).await {
        Err(_) | Ok(Some(TermEvent::Data(_))) | Ok(Some(TermEvent::ReplayComplete)) => {}
        Ok(Some(TermEvent::Detached(reason))) => {
            panic!("stop must not detach the existing attachment: {reason}")
        }
        Ok(None) => panic!("attachment stream closed unexpectedly after stop"),
    }

    // The attachment must still be live and kickable: a second attach
    // takes it over exactly like `second_attach_detaches_first`.
    let (chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("second attach");
    let detached = tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(ev) = rx1.recv().await {
            if let TermEvent::Detached(reason) = ev {
                return reason;
            }
        }
        panic!("first attachment stream ended without a takeover Detached");
    })
    .await
    .expect("timed out waiting for the takeover Detached");
    assert!(
        detached.contains("another client"),
        "takeover reason changed unexpectedly: {detached}"
    );
    // The second attachment is otherwise ordinary — same session, same
    // (now-dead) pane, still attachable.
    let mut seen2 = Vec::new();
    wait_for(&mut rx2, &mut seen2, "FAKE-AGENT READY", 15).await;
    h.client.detach(chan2).await;
}

/// Stop followed by delete on the same live session must both succeed:
/// delete's own pane query must see the (by-then-dead) pane and skip
/// straight to tmux teardown rather than erroring on an already-stopped
/// agent.
#[tokio::test]
async fn stop_then_delete_both_succeed() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    h.client.stop_session(&session.id).await.expect("stop");
    h.client
        .delete_session(&session.id)
        .await
        .expect("delete after stop must succeed");
    assert!(
        h.client.list_sessions().await.unwrap().sessions.is_empty(),
        "a deleted session must not stay listed"
    );
}

/// Stopping an alt-screen agent must not silently discard its last frame.
///
/// A real alt-screen agent (claude, chiefly) restores the primary screen
/// on its way out of a SIGTERM, and tmux never records alternate-screen
/// content in history at all — without a pre-kill snapshot, the app's
/// final frame is unreachable forever the instant the kill lands, and a
/// reattach shows only a blank primary screen plus tmux's "pane is dead"
/// text. This pins the fix end to end: stop an `altscreen` fake-agent
/// session, attach fresh, and require BOTH the app's own marker text and
/// the "last screen before stop" divider that only the snapshot path
/// produces — the divider alone would not prove the CONTENT survived, and
/// the marker alone (with no divider) would not prove it came from the
/// snapshot path rather than some other replay quirk.
#[tokio::test]
async fn stop_replays_the_alt_screen_snapshot() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script altscreen"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "ALT-SCREEN APP", 20).await;
    h.client.detach(chan).await;

    h.client.stop_session(&session.id).await.expect("stop");

    let (_chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("reattach after stop");
    let mut replay = Vec::new();
    // The needle is the CONTENT marker, not the divider: `send_alt_screen
    // _snapshot` (service.rs) now streams the divider and the snapshot
    // content as separate frames (chunked, matching the ordinary prefill's
    // own frame-per-piece shape), and `wait_for` returns the instant its
    // needle appears in the accumulated bytes — waiting on the divider
    // text would risk returning right after that FIRST frame, before the
    // content frame(s) sent immediately after it have necessarily been
    // drained yet. Waiting on content that can only ever arrive in a
    // LATER frame guarantees every frame before it, divider included, is
    // already in `replay` by the time this returns.
    wait_for(&mut rx2, &mut replay, "ALT-SCREEN APP", 15).await;
    let text = String::from_utf8_lossy(&replay);
    let divider_at = text.find("last screen before stop");
    assert!(
        divider_at.is_some(),
        "the app's content must be preceded by the snapshot divider: {text}"
    );
    // Pins `-e` on the underlying `capture-pane`: the fixture draws its
    // banner in reverse video (`\x1b[7m`), and a capture taken WITHOUT
    // escape sequences would carry the plain text with none of its
    // styling. Without this check, a regression that dropped `-e` would
    // still pass every other assertion here (the text marker survives
    // either way).
    assert!(
        text.contains("\x1b[7m"),
        "the snapshot must preserve the fixture's reverse-video SGR sequence, proving \
         capture-pane ran with -e: {text:?}"
    );
    // Pins `sanitize_snapshot_lines` (tmux.rs): `capture-pane -e` emits no
    // attribute reset at a line's end, so a line that ends while a
    // background/inverse attribute is still active leaves it running —
    // a real terminal's scroll/line-feed handling then fills every cell
    // from there onward with that still-active background
    // (background-color-erase), producing a highlight band the real
    // `claude` never showed. Only the SNAPSHOT segment is asserted here;
    // full xterm.js cell-attribute verification belongs to the
    // Playwright suite at a later stack layer.
    //
    // The segment must start AFTER the divider's own trailing `\r\n`, not
    // at the divider's own text: service.rs's divider line
    // (`"\r\n\x1b[2m-- last screen before stop --\x1b[0m\r\n"`) already
    // contains its own literal `\x1b[0m\r\n` — slicing from the divider's
    // text onward would make this assertion pass on the divider's OWN
    // bytes regardless of whether `sanitize_snapshot_lines` ever ran,
    // which is exactly the vacuous check a from-day-one review of this
    // test caught. Reuses `divider_at` (found once above) rather than
    // scanning for the divider text a second time.
    let after_divider = &text[divider_at.expect("checked above")..];
    let (_divider_line, snapshot_segment) = after_divider
        .split_once("\r\n")
        .expect("the divider line itself always ends in its own \\r\\n (service.rs)");
    assert!(
        snapshot_segment.contains("\x1b[0m\r\n"),
        "the snapshot segment (excluding the divider's own trailing reset) must carry an SGR \
         reset immediately before at least one line terminator: {snapshot_segment:?}"
    );
}

/// `-N` coverage: a styled background painted with erase-to-end-of-line
/// (`\x1b[K`, no literal trailing space characters — see `altscreen`'s
/// `STATUS BAR` row) must survive into the stored snapshot. Verified
/// empirically (scratch tmux session, not through this test) that such a
/// row captures as ~19 bytes without `-N` (trimmed right after the label)
/// versus padded out to the full 80-column pane width with it — so a
/// length threshold comfortably between those two shapes is what
/// discriminates "removing -N" from "keeping it" without depending on the
/// EXACT escape-sequence bytes tmux happens to re-serialize, which is not
/// this test's business to pin.
#[tokio::test]
async fn stop_snapshot_preserves_trailing_styled_padding_via_capture_n() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script altscreen"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "STATUS BAR", 20).await;
    h.client.detach(chan).await;

    h.client.stop_session(&session.id).await.expect("stop");

    let (_chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("reattach after stop");
    let mut replay = Vec::new();
    wait_for(&mut rx2, &mut replay, "STATUS BAR", 15).await;
    let text = String::from_utf8_lossy(&replay);

    // Isolate the status-bar ROW: from the label up to the fixture's own
    // trailing CRLF, so padding from LATER lines (or the divider itself)
    // cannot inflate this measurement.
    let start = text
        .find("STATUS BAR")
        .expect("status bar label must be present");
    let row = &text[start..];
    let row = &row[..row.find("\r\n").unwrap_or(row.len())];
    assert!(
        row.len() > 40,
        "the status-bar row must carry its trailing erase-to-end-of-line padding, proving \
         capture-pane ran with -N — got a {}-byte row: {row:?}",
        row.len()
    );
    // `sanitize_snapshot_lines` (tmux.rs) must have closed this row off
    // with its own SGR reset immediately before the `\r\n` this slice was
    // cut at, regardless of the `-N` padding's own attribute bytes. The
    // risk this guards against is a real terminal's scroll/line-feed
    // handling filling cells with THIS row's still-active background
    // (background-color-erase) on replay — not that background leaking
    // into the divider row that follows: this fixture's divider happens
    // to differ enough in styling that `capture-pane -e` reserializes an
    // explicit `\x1b[49m` background reset at the divider's own start
    // regardless of what this test does. That reserialization is a
    // property of THIS SPECIFIC fixture content, not a general guarantee
    // — see `sanitize_snapshot_lines`'s own "why a bare boundary reset is
    // not enough" docs (tmux.rs) for the general case, where a following
    // row's UNCHANGED style is never re-stated at all and would be lost
    // without this transform's own restore.
    assert!(
        row.ends_with("\x1b[0m"),
        "the captured-and-sanitized status-bar row must end with an SGR reset before its line \
         terminator: {row:?}"
    );
}

/// The third replay state alongside the other two alt-screen tests here:
/// alive (ordinary reattach), dead-and-restored-to-primary (the divider
/// case, `stop_replays_the_alt_screen_snapshot`), and this one —
/// dead-but-STILL-on-the-alternate-screen. SIGKILLs the pane's own
/// process directly, bypassing the supervisor's `stop` path (and so its
/// SIGTERM-based restore handler AND its stop-time snapshot capture)
/// entirely, so the pane dies without ever leaving the alternate screen
/// and without `StopSession` ever having run to capture anything.
///
/// Pins the negative case only: the divider must NOT be appended. The
/// `Attach` handler's gate is snapshot EXISTENCE now (file or pending
/// map), not the pane's alternate-screen state — and no snapshot exists
/// here at all, because `StopSession` (the only thing that ever creates
/// one) never ran; `send_alt_screen_snapshot` finds neither source and
/// returns before ever touching `modes.alternate_on`. It does NOT assert
/// the app's own content survives, because (verified empirically,
/// scratch tmux session, not through this codebase) it does not: tmux
/// replaces a pane's LIVE grid with its own
/// "Pane is dead" placeholder the moment the process backing it exits,
/// whether or not that pane was on the alternate screen — capturing an
/// alt-screen pane that died this way shows only that placeholder, same
/// as capturing its (nonexistent) history would. That total loss is
/// exactly the failure this whole feature exists to prevent, but ONLY
/// for stops that went through `StopSession`'s own capture-before-kill
/// path; a pane killed some other way (an externally-issued SIGKILL, as
/// here) was never going to have a snapshot to fall back on in the first
/// place, and this test's job is only to confirm that absence does not
/// somehow manifest as a stray divider.
#[tokio::test]
async fn dead_pane_still_on_alt_screen_replays_without_a_divider() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script altscreen"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "ALT-SCREEN APP", 20).await;
    h.client.detach(chan).await;

    let tmux_name = format!("fh-{}", session.id);
    let sock = h.state.path().join("tmux.sock");
    let pid_out = tmux_query(
        &sock,
        &["display-message", "-p", "-t", &tmux_name, "#{pane_pid}"],
    )
    .await;
    let pane_pid = String::from_utf8_lossy(&pid_out.stdout).trim().to_string();
    let killed = tokio::process::Command::new("kill")
        .arg("-9")
        .arg(&pane_pid)
        .status()
        .await
        .expect("running kill(1)");
    assert!(
        killed.success(),
        "SIGKILL of the pane's own process must succeed"
    );

    // Wait for tmux to actually mark the pane dead before attaching, so
    // the attach cannot race a not-yet-updated `pane_dead` flag.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let out = tmux_query(
            &sock,
            &["display-message", "-p", "-t", &tmux_name, "#{pane_dead}"],
        )
        .await;
        if String::from_utf8_lossy(&out.stdout).trim() == "1" {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "pane never went dead after SIGKILL"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let (_chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("attach to a dead-on-alt-screen pane");
    let mut replay = Vec::new();
    // What the replay of a dead-on-alt-screen pane contains is
    // environment-dependent in a way that burned this test twice: some
    // environments retain the app's last frame in the capture (the
    // placeholder arriving, if at all, only via live output after
    // attach), others substitute tmux's "Pane is dead" placeholder into
    // the capture itself — and which one a given tmux 3.4 produces has
    // varied even between this repo's CI and a local install of the same
    // version. The assertion this test exists for is the NEGATIVE below
    // (no snapshot divider), so the anchor deliberately accepts either
    // marker: both prove the replay delivered the dead pane's content.
    let anchor_deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let text = String::from_utf8_lossy(&replay);
        if text.contains("Pane is dead") || text.contains("ALT-SCREEN APP") {
            break;
        }
        let remaining = anchor_deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(remaining, rx2.recv()).await {
            Ok(Some(TermEvent::Data(bytes))) => replay.extend_from_slice(&bytes),
            Ok(other) => panic!("attachment ended before any dead-pane content: {other:?}"),
            Err(_) => panic!(
                "timed out waiting for dead-pane replay content; transcript so far:\n{}",
                String::from_utf8_lossy(&replay)
            ),
        }
    }
    // Settle-then-drain, same pattern as the other negative-divider
    // assertions in this file: absence needs a short observation window,
    // not a single immediate check.
    tokio::time::sleep(Duration::from_millis(300)).await;
    while let Ok(TermEvent::Data(bytes)) = rx2.try_recv() {
        replay.extend_from_slice(&bytes);
    }
    let text = String::from_utf8_lossy(&replay);
    assert!(
        !text.contains("last screen before stop"),
        "a pane that died still on the alternate screen (never having gone through StopSession) \
         must never gain a snapshot divider: {text}"
    );
}

/// The positive counterpart to `dead_pane_still_on_alt_screen_replays_
/// without_a_divider`: here `StopSession` DOES run (and DOES capture,
/// since the pane is alive and on the alternate screen when `stop` is
/// called), and its own `kill_process_tree` is what finally kills the
/// app — via the `altscreen-ignores-term` fixture, which never restores
/// the primary screen because it never runs any code on SIGTERM at all
/// (`SIG_IGN`), so `kill_process_tree` must escalate through its full
/// grace/SIGSTOP-quiesce/SIGKILL sequence before the pane actually dies.
/// This is the exact scenario the alt-screen snapshot feature exists
/// for, and the one the earlier `dead && !alternate_on` gate silently
/// blanked: a dead pane still on the alternate screen, with a REAL
/// snapshot on disk this time. Requires both the divider and the app's
/// own marker to replay, AND the alt-exit escape (`\x1b[?1049l`) to
/// precede the divider — landing the snapshot on the primary screen's
/// scrollback rather than inside the scrollback-less alternate buffer the
/// ordinary mode-replay just re-entered.
#[tokio::test]
async fn stop_replays_the_alt_screen_snapshot_when_the_agent_ignores_term_and_never_restores() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script altscreen-ignores-term"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "ALT-SCREEN APP", 20).await;
    h.client.detach(chan).await;

    // Runs to completion: `kill_process_tree`'s own escalation (grace,
    // SIGSTOP-quiesce, SIGKILL, confirm) is what actually kills a process
    // that ignores SIGTERM outright, so this call does not return until
    // that whole sequence has finished.
    h.client
        .stop_session(&session.id)
        .await
        .expect("stop must still succeed against a SIGTERM-ignoring alt-screen app");

    let (_chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("reattach after stop");
    let mut replay = Vec::new();
    // Ordered wait, anchored on the divider: this fixture died still ON
    // the alternate screen, and whether the dead pane's PREFILL already
    // contains the app's frame is tmux-version-dependent (see
    // `wait_for_after`'s docs) — a plain content wait can return before
    // the snapshot suffix ever arrives.
    wait_for_after(
        &mut rx2,
        &mut replay,
        "last screen before stop",
        "ALT-SCREEN APP",
        15,
    )
    .await;
    let text = String::from_utf8_lossy(&replay);
    let exit_alt_screen = text.find("\x1b[?1049l").expect(
        "the alt-exit escape must precede the snapshot, since the pane died still on \
                 the alternate screen",
    );
    let divider = text.find("last screen before stop").expect("checked above");
    assert!(
        exit_alt_screen < divider,
        "the alt-exit escape must land the snapshot on the primary screen's scrollback, so it \
         must precede the divider, not follow it: {text:?}"
    );
}

/// The gap `Supervisor::pending_snapshots` (service.rs) exists to close:
/// an `Attach` landing AFTER the pane has gone dead but BEFORE
/// `StopSession` has finished (`kill_process_tree` can take a real
/// fraction of a second against an uncooperative tree) must still see
/// the snapshot, served from the in-memory pending map rather than a file
/// that has not been written yet.
///
/// Uses `altscreen-stubborn-child`: this process's own pid restores the
/// primary screen and exits within milliseconds of SIGTERM (so the pane
/// goes dead almost immediately), while its spawned child ignores
/// SIGTERM and forces `kill_process_tree` through its full SIGSTOP-
/// quiesce-then-SIGKILL escalation — several hundred milliseconds beyond
/// `KILL_GRACE` (500ms, service.rs) alone. A fixed delay comfortably
/// inside that window is what this test uses to land the concurrent
/// attach; see the delay's own comment for the honest limits of that
/// approach (best-effort, not a deterministic barrier — REJECTED per the
/// review round that requested this test).
#[tokio::test]
async fn attach_mid_stop_sees_the_pending_alt_screen_snapshot() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script altscreen-stubborn-child"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "ALT-SCREEN APP", 20).await;
    h.client.detach(chan).await;

    let stopper = h.second_client().await;
    let stop_session_id = session.id.clone();
    let stop_task = tokio::spawn(async move { stopper.stop_session(&stop_session_id).await });

    // The fixture's own root process restores the primary screen and
    // exits within single-digit milliseconds of receiving SIGTERM, but
    // its stubborn child forces `kill_process_tree` through its full
    // escalation. 250ms is comfortably inside the resulting window on any
    // reasonable machine: long enough that the pane is certainly already
    // dead, short enough that `stop_session` is, with very high
    // confidence but not a hard guarantee, still in flight — a faster-
    // than-expected sweep would simply mean this attach lands after
    // publish instead, reading the same content back from the file
    // rather than the pending map. Either way the assertions below still
    // hold; only the CODE PATH exercised would differ, which is the
    // honest limit of a fixed-delay approach to a race this narrow.
    tokio::time::sleep(Duration::from_millis(250)).await;

    let (chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("attach mid-stop");
    let mut replay = Vec::new();
    // Wait on the CONTENT marker, not the divider — same reasoning as
    // `stop_replays_the_alt_screen_snapshot`'s identical comment: the
    // divider and the snapshot content are separate, sequential frames,
    // and `wait_for` returns the instant its OWN needle appears, so only
    // a needle that can exclusively come from a LATER frame guarantees
    // everything before it (the divider included) has already arrived.
    wait_for(&mut rx2, &mut replay, "ALT-SCREEN APP", 10).await;
    let text = String::from_utf8_lossy(&replay);
    assert!(
        text.contains("last screen before stop"),
        "the app's content must be preceded by the snapshot divider: {text}"
    );
    h.client.detach(chan2).await;

    stop_task
        .await
        .expect("stop task must not panic")
        .expect("stop must still succeed despite the stubborn child");
}

/// The still-on-the-alternate-screen counterpart to `attach_mid_stop_
/// sees_the_pending_alt_screen_snapshot`: re-checks the pending-map
/// fallback against the CORRECTED replay rule (snapshot existence, not
/// `!alternate_on`), and specifically pins that the `\x1b[?1049l`
/// alt-exit escape composes correctly with a pending-map-served (not yet
/// written to disk) snapshot, not just a file-served one.
///
/// Uses `altscreen-stubborn-child-stays-alt`: this process's own pid dies
/// to the DEFAULT SIGTERM disposition within milliseconds (still on the
/// alternate screen, no restore — unlike `AltscreenStubbornChild`'s
/// restore-then-exit), while its spawned child ignores SIGTERM and forces
/// `kill_process_tree` through its full escalation regardless. Same
/// fixed-delay, best-effort timing approach as the sibling test above —
/// see its own comment for the honest limits of that.
#[tokio::test]
async fn attach_mid_stop_sees_the_pending_snapshot_while_still_on_the_alt_screen() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script altscreen-stubborn-child-stays-alt"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "ALT-SCREEN APP", 20).await;
    h.client.detach(chan).await;

    let stopper = h.second_client().await;
    let stop_session_id = session.id.clone();
    let stop_task = tokio::spawn(async move { stopper.stop_session(&stop_session_id).await });

    // See `attach_mid_stop_sees_the_pending_alt_screen_snapshot`'s
    // identical comment: 250ms lands comfortably inside the window
    // between the pane going dead (near-instant, default SIGTERM
    // disposition) and `kill_process_tree` finishing (bounded below by
    // `KILL_GRACE`, 500ms, plus escalating against the stubborn child).
    tokio::time::sleep(Duration::from_millis(250)).await;

    let (chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("attach mid-stop");
    let mut replay = Vec::new();
    // Ordered wait for the same tmux-version reason as the
    // ignores-term test above (`wait_for_after`'s docs): this fixture's
    // root died still on the alternate screen.
    wait_for_after(
        &mut rx2,
        &mut replay,
        "last screen before stop",
        "ALT-SCREEN APP",
        10,
    )
    .await;
    let text = String::from_utf8_lossy(&replay);
    let exit_alt_screen = text
        .find("\x1b[?1049l")
        .expect("the alt-exit escape must precede the pending-served snapshot too");
    let divider = text
        .find("last screen before stop")
        .expect("the app's content must be preceded by the snapshot divider");
    assert!(
        exit_alt_screen < divider,
        "the alt-exit escape must precede the divider even when the snapshot is served from \
         the pending map rather than the file: {text:?}"
    );
    h.client.detach(chan2).await;

    stop_task
        .await
        .expect("stop task must not panic")
        .expect("stop must still succeed despite the stubborn child");
}

/// A primary-screen agent's stop must never even capture a snapshot, let
/// alone replay one: its real scrollback already survives via ordinary
/// tmux history, so a synthetic "last screen" block would be clutter with
/// no lost content to recover. Pins the alt-screen-only gating in
/// `capture_alt_screen_before_stop` (fed by
/// `TmuxDriver::capture_alt_screen_if_active`'s own alternate-on check)
/// two ways: the snapshot FILE must not exist on disk at all (a
/// deterministic check on the actual artifact this feature writes, not a
/// proxy for it), and the replayed divider text must not appear either
/// (the user-visible consequence, kept as a second, independent
/// assertion).
#[tokio::test]
async fn stop_replay_has_no_snapshot_divider_for_a_primary_screen_agent() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    h.client.detach(chan).await;

    h.client.stop_session(&session.id).await.expect("stop");

    let snapshot_path = h.state.path().join("snapshots").join(&session.id);
    assert!(
        !snapshot_path.exists(),
        "a primary-screen stop must never write a snapshot file at all"
    );

    let (_chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("reattach after stop");
    let mut replay = Vec::new();
    wait_for(&mut rx2, &mut replay, "FAKE-AGENT READY", 15).await;
    // A missing divider needs a settle window, not a single check: the
    // (incorrect) extra frames this test guards against would arrive
    // immediately after the prefill, same as everything else asserted on
    // above, so draining once more after a short wait is enough to catch
    // them without an open-ended sleep.
    tokio::time::sleep(Duration::from_millis(300)).await;
    while let Ok(TermEvent::Data(bytes)) = rx2.try_recv() {
        replay.extend_from_slice(&bytes);
    }
    let text = String::from_utf8_lossy(&replay);
    assert!(
        !text.contains("last screen before stop"),
        "a primary-screen stop replay must not gain the alt-screen divider: {text}"
    );
}

/// A snapshot file must never be consulted for a LIVE pane, no matter
/// what is sitting on disk at its path — the `Attach` handler gates the
/// whole feature on the pane being dead (see `send_alt_screen_snapshot`'s
/// call site), so a leftover or tampered-with file from some earlier
/// state must not leak into an otherwise-ordinary attach. Plants a
/// snapshot file directly (bypassing `stop` entirely) against a session
/// whose agent is still running, so this is a pure "was the file even
/// looked at" check, independent of whether stop's own capture logic
/// would ever have produced this content.
#[tokio::test]
async fn attach_ignores_a_stale_snapshot_file_for_a_live_pane() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let snapshot_dir = h.state.path().join("snapshots");
    std::fs::create_dir_all(&snapshot_dir).expect("create snapshots dir");
    std::fs::write(snapshot_dir.join(&session.id), b"stale content")
        .expect("plant a stale snapshot file");

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    // Same settle-then-drain pattern as the primary-screen negative test
    // above: absence needs a short observation window, not a single
    // immediate check.
    tokio::time::sleep(Duration::from_millis(300)).await;
    while let Ok(TermEvent::Data(bytes)) = rx.try_recv() {
        seen.extend_from_slice(&bytes);
    }
    let text = String::from_utf8_lossy(&seen);
    assert!(
        !text.contains("last screen before stop"),
        "a live pane's attach must never consult a stale snapshot file: {text}"
    );
}

/// Stop's contract is killing the process tree — a storage failure while
/// trying to capture or persist the alt-screen snapshot must never block
/// that. Pre-creates a regular FILE at the path the snapshots
/// subdirectory would occupy, so `ensure_private_dir` fails when
/// `publish_alt_screen_snapshot` tries to create it; `stop_session` must
/// still report success, and the pane must still actually be dead
/// afterwards, proving the kill ran to completion despite the storage
/// failure rather than merely "not erroring".
#[tokio::test]
async fn stop_still_kills_when_the_snapshots_directory_cannot_be_created() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script altscreen"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "ALT-SCREEN APP", 20).await;
    h.client.detach(chan).await;

    std::fs::write(
        h.state.path().join("snapshots"),
        b"blocks directory creation",
    )
    .expect("plant a regular file where the snapshots directory belongs");

    h.client
        .stop_session(&session.id)
        .await
        .expect("stop must still succeed despite a storage failure");

    let tmux_name = format!("fh-{}", session.id);
    let sock = h.state.path().join("tmux.sock");
    let out = tmux_query(
        &sock,
        &["display-message", "-p", "-t", &tmux_name, "#{pane_dead}"],
    )
    .await;
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "1",
        "the process tree must still be killed even when the snapshot cannot be stored"
    );
}

/// A snapshot that cannot be READ must degrade the attach to the plain
/// prefill, not fail it — best-effort by design (see
/// `send_alt_screen_snapshot`'s docs). Plants a DIRECTORY at the snapshot
/// path for an already-dead-pane session (`tokio::fs::read` on a
/// directory fails, unlike the ordinary "file absent" case), and requires
/// the attach to still succeed and still replay the ordinary content —
/// just without the divider.
#[tokio::test]
async fn attach_degrades_to_plain_prefill_when_the_snapshot_path_is_unreadable() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    h.client.detach(chan).await;

    h.client.stop_session(&session.id).await.expect("stop");

    let snapshot_path = h.state.path().join("snapshots").join(&session.id);
    std::fs::create_dir_all(&snapshot_path).expect("plant a directory at the snapshot path");

    let (_chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("attach must still succeed despite an unreadable snapshot");
    let mut replay = Vec::new();
    wait_for(&mut rx2, &mut replay, "FAKE-AGENT READY", 15).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    while let Ok(TermEvent::Data(bytes)) = rx2.try_recv() {
        replay.extend_from_slice(&bytes);
    }
    let text = String::from_utf8_lossy(&replay);
    assert!(
        !text.contains("last screen before stop"),
        "an unreadable snapshot must degrade to the plain prefill, not appear: {text}"
    );
}

/// The chunked send path (`send_alt_screen_snapshot`) must deliver a
/// snapshot LARGER than one `REPLAY_CHUNK` (32 KiB) completely and in
/// order, across however many frames that takes — not just the
/// single-frame case every other snapshot test here happens to exercise
/// (the fixtures' own captured content is far smaller than 32 KiB).
/// Plants a snapshot with a head marker, a marker straddling the
/// (assumed, matching service.rs's own `REPLAY_CHUNK`) 32 KiB chunk
/// boundary, and a tail marker; requires all three to arrive, in that
/// relative order, in the reassembled replay.
#[tokio::test]
async fn dead_pane_snapshot_replay_delivers_a_multi_chunk_snapshot_intact() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    h.client.detach(chan).await;

    h.client.stop_session(&session.id).await.expect("stop");

    const ASSUMED_REPLAY_CHUNK: usize = 32 * 1024;
    let mut content = Vec::new();
    content.extend_from_slice(b"HEAD-MARKER");
    content.resize(ASSUMED_REPLAY_CHUNK - 5, b'x');
    content.extend_from_slice(b"BOUNDARY-MARKER");
    content.resize(ASSUMED_REPLAY_CHUNK + 4000, b'y');
    content.extend_from_slice(b"TAIL-MARKER");

    let snapshot_dir = h.state.path().join("snapshots");
    std::fs::create_dir_all(&snapshot_dir).expect("create snapshots dir");
    std::fs::write(snapshot_dir.join(&session.id), &content).expect("plant a multi-chunk snapshot");

    let (_chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("reattach");
    let mut replay = Vec::new();
    wait_for(&mut rx2, &mut replay, "TAIL-MARKER", 15).await;
    let text = String::from_utf8_lossy(&replay);
    let head = text.find("HEAD-MARKER").expect("head marker must arrive");
    let boundary = text
        .find("BOUNDARY-MARKER")
        .expect("chunk-boundary marker must arrive");
    let tail = text.find("TAIL-MARKER").expect("tail marker must arrive");
    assert!(
        head < boundary && boundary < tail,
        "markers must arrive in order across multiple chunks: {text:?}"
    );
}

/// Fail-closed cleanup applies to the alt-screen snapshot exactly like
/// the launch artifacts (`delete_fails_closed_when_a_launch_artifact_
/// cannot_be_removed`): an unremovable snapshot must fail the WHOLE
/// delete, row and map entry intact, rather than silently losing the last
/// handle on a file that may hold secrets. A non-empty DIRECTORY at the
/// snapshot path (rather than a permission trick) is what actually makes
/// `remove_file` fail here — `unlink` refuses any directory regardless of
/// permissions.
#[tokio::test]
async fn delete_fails_closed_when_the_alt_screen_snapshot_cannot_be_removed() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let snapshot_path = h.state.path().join("snapshots").join(&session.id);
    std::fs::create_dir_all(&snapshot_path).expect("plant a directory at the snapshot path");
    std::fs::write(snapshot_path.join("inner"), b"x").expect("make the directory non-empty");

    let result = h.client.delete_session(&session.id).await;
    let err = result.expect_err("delete must fail closed when the snapshot cannot be removed");
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("must carry a SupervisorError")
            .kind,
        ErrorKind::Internal,
        "an unremovable snapshot is a server-side sweep problem, not a caller precondition"
    );

    let sup2 = Supervisor::new_with_exe(h.state.path(), farhelm_bin().into())
        .await
        .expect("second supervisor construction on the same state dir");
    let client2 = connect_client(&sup2).await;
    assert_eq!(
        client2
            .list_sessions()
            .await
            .expect("list from fresh supervisor")
            .sessions,
        // The failed delete already tore the tmux session down before the
        // snapshot removal refused, so the surviving row lists as exited
        // with no code — the same honest answer as any restart-gap row.
        vec![with_status(
            session.clone(),
            SessionStatus::Exited { exit_code: None }
        )],
        "a failed delete must leave the row in place for a retry"
    );
}

/// Snapshot files are plain session-id-keyed state under the
/// supervisor's own state dir, so they must survive exactly like the
/// SQLite row does across a supervisor restart (mirroring
/// `stop_kills_the_whole_process_tree`'s own restart check): stop an
/// alt-screen session, construct a SECOND, independent `Supervisor` on
/// the same state dir, and attach through IT — the divider and the app's
/// own marker must both replay, proving the snapshot was read from disk
/// rather than from any in-process state the first supervisor held.
#[tokio::test]
async fn alt_screen_snapshot_survives_a_supervisor_restart() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script altscreen"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "ALT-SCREEN APP", 20).await;
    h.client.detach(chan).await;

    h.client.stop_session(&session.id).await.expect("stop");

    let sup2 = Supervisor::new_with_exe_and_timeouts(
        h.state.path(),
        farhelm_bin().into(),
        suite_timeouts(),
    )
    .await
    .expect("second supervisor construction on the same state dir");
    let client2 = connect_client(&sup2).await;

    let (_chan2, mut rx2) = client2
        .attach(&session.id, 80, 24)
        .await
        .expect("attach through a fresh supervisor on the same state dir");
    let mut replay = Vec::new();
    // Waits on the content marker, not the divider — see
    // `stop_replays_the_alt_screen_snapshot`'s identical comment for why:
    // the divider and the snapshot content are separate, sequential
    // frames, and `wait_for` returns as soon as ITS needle appears, so
    // only a needle that can exclusively come from a LATER frame
    // guarantees everything before it (divider included) already
    // arrived.
    wait_for(&mut rx2, &mut replay, "ALT-SCREEN APP", 15).await;
    let text = String::from_utf8_lossy(&replay);
    assert!(
        text.contains("last screen before stop"),
        "the snapshot must survive a supervisor restart and still replay behind its divider: \
         {text}"
    );
}

/// Delete must tolerate a tmux session that disappeared out from under a
/// LIVE `SessionEntry` — someone (or something) else killed it directly
/// on the private socket, distinct from the restart-gap case
/// (`delete_works_on_a_terminal_less_session`) where the whole tmux
/// server, not just one session, failed to survive. `pane_process`'s
/// tolerated-absence path (the same tmux diagnostics `has_session`/
/// `kill_session` already treat as "not there") is what makes this
/// succeed rather than fail-closed.
#[tokio::test]
async fn delete_after_externally_killed_tmux_session_succeeds() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let tmux_name = format!("fh-{}", session.id);
    let sock = h.state.path().join("tmux.sock");
    let killed = tmux_query(&sock, &["kill-session", "-t", &tmux_name]).await;
    assert!(
        killed.status.success(),
        "test setup: kill-session must succeed, got: {}",
        String::from_utf8_lossy(&killed.stderr)
    );

    // This process's own Supervisor still has a LIVE SessionEntry for
    // this session (entries are never demoted from Some to None within
    // one process's lifetime) — unlike the restart-gap tests, no second
    // Supervisor construction is involved here.
    h.client
        .delete_session(&session.id)
        .await
        .expect("delete must tolerate an externally killed tmux session");
    assert!(
        h.client.list_sessions().await.unwrap().sessions.is_empty(),
        "a deleted session must not stay listed"
    );
}

/// A pane whose tmux session was RENAMED out from under a live
/// `SessionEntry` must make delete FAIL CLOSED — the other half of the
/// foreign-owner policy, and the half with an agent's life riding on it.
///
/// Renaming the session (verified empirically — `display-message -t
/// <pane>` happily resolves the renamed session and reports its NEW name)
/// makes the stored `tmux_name` mismatch what tmux reports, so
/// `pane_process` classifies the pane as foreign exactly as a recycled
/// pane id does. The two are not interchangeable, which is the whole
/// point of `Supervisor::known_session_tmux_name`: a recycled id is owned
/// by another session THIS supervisor launched and therefore proves the
/// recorded terminal died with a previous tmux server, while an owner
/// nobody recognizes may be this very session's live terminal wearing a
/// different name. Proceeding on the second would kill a running agent
/// without consent and still leave the renamed session, its scrollback,
/// and its tabs behind while reporting the session deleted.
///
/// So this pins the refusal end to end: `Internal`, the row and map entry
/// left in place for a retry, and — the assertion that makes the refusal
/// worth anything — the agent still ALIVE afterwards. A refusal that
/// killed on its way out would be strictly worse than proceeding.
///
/// The new name deliberately CONTAINS SPACES. tmux allows that, and
/// `pane_process`'s format puts the session name last precisely so it
/// survives whole; a parser that split on whitespace would truncate this
/// to `renamed`, and the owner the policy compares against would be a
/// name that never existed.
#[tokio::test]
async fn delete_after_renamed_tmux_session_fails_closed() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script basic"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());
    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    // The pane's own process, read from tmux BEFORE the rename: after it,
    // the pane can no longer be resolved under the recorded name at all.
    // This pid is what the liveness assertion at the end reads, and the
    // basic script prints no pid of its own to use instead.
    let tmux_name = format!("fh-{}", session.id);
    let sock = h.state.path().join("tmux.sock");
    let pane_pid_out = tmux_query(
        &sock,
        &["display-message", "-p", "-t", &tmux_name, "#{pane_pid}"],
    )
    .await;
    assert!(
        pane_pid_out.status.success(),
        "test setup: querying the pane pid must succeed, got: {}",
        String::from_utf8_lossy(&pane_pid_out.stderr)
    );
    let pane_pid: u32 = String::from_utf8_lossy(&pane_pid_out.stdout)
        .trim()
        .parse()
        .expect("tmux reports a numeric pane pid");

    let renamed = tmux_query(
        &sock,
        &["rename-session", "-t", &tmux_name, "renamed out from under"],
    )
    .await;
    assert!(
        renamed.status.success(),
        "test setup: rename-session must succeed, got: {}",
        String::from_utf8_lossy(&renamed.stderr)
    );

    let err = h.client.delete_session(&session.id).await.expect_err(
        "delete must fail closed when the pane's session was renamed out from under it",
    );
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("a failed teardown must carry a SupervisorError")
            .kind,
        ErrorKind::Internal,
        "a teardown failure is a server-side sweep problem, not a caller precondition"
    );
    assert_eq!(
        h.client.list_sessions().await.unwrap().sessions,
        vec![with_status(
            session.clone(),
            SessionStatus::Exited { exit_code: None }
        )],
        "a failed delete must leave the row and map entry in place for a retry; \
         session_status requires BOTH the remembered pane id AND the remembered tmux \
         session name to match what tmux currently reports (see that function's own docs) \
         — the rename changes the session name tmux reports for this pane, so the identity \
         can no longer be positively confirmed, and the honest answer is Exited, not a \
         guess either way"
    );
    assert!(
        !process_is_gone(pane_pid),
        "the refusal must kill NOTHING: the agent is still running under the renamed \
         session, and a delete that swept first and refused afterwards would have destroyed \
         exactly what the refusal exists to protect"
    );
}

/// The acceptance test for the environment-marker half of
/// `kill_process_tree` (lore/2026-07-27-m2-process-tree-stop.md): a
/// daemon that has fully reparented to init — no longer any descendant of
/// the pane's process at all — must still be killed, because only the
/// `FARHELM_SESSION_ID` marker (never a PPID walk) can find it. This must
/// fail if EITHER half of that mechanism is removed: marker injection at
/// launch (`launch.rs`'s `SESSION_ID_ENV_VAR`) or marker enumeration
/// during the sweep (`environ_has_marker`/`enumerate_tree`) — a bare PPID
/// closure from the pane root would never reach this pid at all.
#[tokio::test]
async fn stop_kills_a_reparented_marked_daemon() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner-reparent"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    let self_pid = extract_pid(&seen, "SELF-PID:");
    let daemon_pid = wait_for_pid_file(&work.path().join("reparented.pid"), 10).await;
    assert_ne!(
        self_pid, daemon_pid,
        "the reparented daemon must be a genuinely different process"
    );

    h.client.stop_session(&session.id).await.expect("stop");

    wait_until_pid_gone(self_pid, 15).await;
    wait_until_pid_gone(daemon_pid, 15).await;
}

/// The marker-only sweep must find and kill a REAL reparented survivor
/// even when there is no live pane process at all to walk ancestry
/// from — not a hypothetical, but the exact scenario `kill_process_tree`'s
/// `root_pid: None` handling exists for (see that function's docs). This
/// must fail if stop ever goes back to SKIPPING the sweep when the pane
/// looks dead or absent, which is what the first cut of this code did.
///
/// The pane is made dead by killing the agent process directly (not by
/// calling stop first, which would already reap the daemon via the live-
/// pane path and prove nothing about the dead-pane path specifically).
/// `remain-on-exit` keeps the pane around to report `pane_dead`, exactly
/// like `exited_agent_leaves_a_viewable_terminal` relies on elsewhere.
#[tokio::test]
async fn stop_kills_a_reparented_daemon_with_no_live_pane_to_walk_from() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner-reparent"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    let self_pid = extract_pid(&seen, "SELF-PID:");
    let daemon_pid = wait_for_pid_file(&work.path().join("reparented.pid"), 10).await;

    // Kill the pane's own process directly: the pane goes dead
    // (remain-on-exit keeps the terminal), leaving no live pid for
    // kill_process_tree to walk ancestry from at all.
    // SAFETY: self_pid is a real, currently-live pid this test just
    // extracted from the fake agent's own output.
    unsafe {
        libc::kill(self_pid as libc::pid_t, libc::SIGKILL);
    }
    wait_until_pid_gone(self_pid, 10).await;

    h.client.stop_session(&session.id).await.expect("stop");
    wait_until_pid_gone(daemon_pid, 15).await;
}

/// Closure seeding, not just the marker scan alone: the reparented
/// daemon's own child has its `FARHELM_SESSION_ID` marker deliberately
/// stripped (`env -u`), so the marker scan alone would never find it —
/// only reaching it by walking the PPID closure FROM the daemon proves
/// that marker pids seed the closure before it expands, per
/// `enumerate_tree`'s docs. This must fail if that seeding is ever
/// demoted back to appending marker pids as closure LEAVES instead of
/// roots.
#[tokio::test]
async fn stop_kills_an_unmarked_child_of_a_reparented_daemon_via_closure_seeding() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner-reparent"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    let daemon_pid = wait_for_pid_file(&work.path().join("reparented.pid"), 10).await;
    let unmarked_child_pid = wait_for_pid_file(&work.path().join("unmarked-child.pid"), 10).await;
    assert!(
        marked_pids(&session.id).contains(&daemon_pid),
        "test setup: the daemon must actually carry the marker"
    );
    assert!(
        !marked_pids(&session.id).contains(&unmarked_child_pid),
        "test setup: the child must NOT carry the marker — that is the point"
    );

    h.client.stop_session(&session.id).await.expect("stop");

    wait_until_pid_gone(daemon_pid, 15).await;
    wait_until_pid_gone(unmarked_child_pid, 15).await;
}

/// The headline cgroup acceptance test (PLAN_M3.md item 10, acceptance
/// 10): on a host with a systemd user manager, stop must kill through the
/// launch's own scope AND still run the backstop sweep afterwards.
///
/// Both halves are asserted through processes only ONE mechanism can
/// reach, because the end state is otherwise identical:
///
/// - the cloaked daemon (double-forked to init, marker stripped) is
///   invisible to both halves of `kill_process_tree`, so its death can
///   only have come from the cgroup — this is the residual
///   lore/2026-07-27-m2-process-tree-stop.md accepted and this milestone
///   closes;
/// - the marked decoy is outside the scope entirely, so its death can only
///   have come from the marker sweep — which is SPEC_impl.md's
///   belt-and-suspenders rule made observable.
///
/// The recorded selection is checked too: a run where the manager was
/// present but the launch fell back would kill the decoy and leave the
/// daemon, and reading the column is what turns that into a clear failure
/// rather than a puzzling one.
#[tokio::test]
async fn a_scope_launched_stop_kills_through_the_cgroup_and_still_runs_the_sweep() {
    let Some((h, _scopes)) = scope_gated_harness(
        "a_scope_launched_stop_kills_through_the_cgroup_and_still_runs_the_sweep",
    )
    .await
    else {
        return;
    };
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner-cloaked"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    let self_pid = extract_pid(&seen, "SELF-PID:");
    let cloaked_pid = wait_for_pid_file(&work.path().join("cloaked.pid"), 10).await;
    let _cloaked_cleanup = PidKillGuard::arm(cloaked_pid);
    assert!(
        !marked_pids(&session.id).contains(&cloaked_pid),
        "test setup: the cloaked daemon must NOT carry the marker — the whole point is that \
         only a cgroup can find it"
    );

    // The tree-shape audit, asserted rather than merely reasoned about:
    // `systemd-run --user --scope` must `exec` in place, so the pane's
    // process IS the agent, exactly as it is without the wrapper. Anything
    // that forked instead would leave the pane pointing at an intermediary
    // — and `pane_process` liveness, `pane_dead_status` exit codes, and the
    // sweep's PPID closure all read that pid.
    let pane_pid_out = tmux_query(
        &h.state.path().join("tmux.sock"),
        &[
            "display-message",
            "-p",
            "-t",
            &format!("fh-{}", session.id),
            "#{pane_pid}",
        ],
    )
    .await;
    assert_eq!(
        String::from_utf8_lossy(&pane_pid_out.stdout).trim(),
        self_pid.to_string(),
        "the scope wrapper must exec in place: the pane's process must still be the agent \
         itself, with nothing spliced in between"
    );

    let decoy = MarkedDecoy::spawn(&session.id);
    let decoy_pid = decoy.pid();

    assert_eq!(
        launch_scope_of(&h, &session.id).await,
        Some(format!("farhelm-{}-0.scope", session.id)),
        "a launch on a manager-equipped host must record its generation-scoped unit"
    );

    h.client.stop_session(&session.id).await.expect("stop");

    wait_until_pid_gone(self_pid, 15).await;
    wait_until_pid_gone(cloaked_pid, 15).await;
    wait_until_pid_gone(decoy_pid, 15).await;
}

/// The recorded selection must survive a supervisor restart, and the
/// RESTARTED supervisor must still be able to kill through the scope
/// (PLAN_M3.md item 10's reload interplay, acceptance 10).
///
/// This is the case the durable column exists for: the restarted process
/// never ran the launch, never saw the probe that chose the scope, and
/// re-derives the unit name from the row's id and generation. If the
/// column were dropped — or the name derived from anything the restart
/// changes — the cloaked daemon would survive, since nothing else in the
/// system can reach it.
///
/// The restarted supervisor below is deliberately seamed with the SAME
/// probed `ScopeManager` the first one used, rather than left to build its
/// own — a test-only choice, not a claim about production. In production a
/// restart genuinely re-probes (a real reboot can gain or lose a user
/// manager between runs, and re-probing is the correct answer to that).
/// Here it would just be a second independent probe of a manager that
/// has not gone anywhere, and `exists`/`kill` — which the stop below
/// reaches — touch that probe on their own first call exactly as
/// `available` does. Leaving the restart unseamed would reopen, on the
/// SECOND supervisor, the exact divergence this file's `scope_gated_harness`
/// exists to close on the first.
#[tokio::test]
async fn a_recorded_scope_survives_a_supervisor_restart_and_still_kills() {
    let Some((h, scopes)) =
        scope_gated_harness("a_recorded_scope_survives_a_supervisor_restart_and_still_kills").await
    else {
        return;
    };
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner-cloaked"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    let cloaked_pid = wait_for_pid_file(&work.path().join("cloaked.pid"), 10).await;
    let _cloaked_cleanup = PidKillGuard::arm(cloaked_pid);
    let scope_before = launch_scope_of(&h, &session.id).await;
    assert!(
        scope_before.is_some(),
        "test setup: this launch must be scoped"
    );

    // The predecessor is RELEASED before its replacement is built, and the
    // replacement's ownership is asserted rather than assumed. An
    // overlapping successor starts read-only (`Supervisor::owns_state_dir`)
    // and reconciles nothing, so a test that skipped this would exercise a
    // path production never takes — and, worse here, would prove nothing
    // about the restart at all: a read-only supervisor's stop is not the
    // stop under test. `_tmux` is bound AFTER `state` on purpose; see
    // `TmuxServerGuard`'s docs.
    let Harness {
        client,
        sup,
        state,
        _tmux,
        _slot,
    } = h;
    drop(client);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while Arc::strong_count(&sup) > 1 {
        assert!(tokio::time::Instant::now() < deadline, "connection drain");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    drop(sup);

    // The SAME probed manager, not a fresh `ScopeManager::systemd()`: the
    // stop below reaches `exists`/`kill` (sweep.rs's `kill_scope`), and
    // those touch the manager's `OnceCell` on their own first call exactly
    // as `available` does — independent of the FIRST supervisor's verdict.
    // A restarted supervisor built with default seams would re-probe right
    // here, under the same load that can make any probe lose, and this
    // test's assertion that the restart still kills through the cgroup
    // would flake for the identical reason `scope_gated_harness` exists to
    // prevent on the first supervisor. See `probed_scope_manager`'s docs.
    let restarted = Supervisor::new_with_seams(
        state.path(),
        farhelm_bin().into(),
        // Built by hand for the scope seam below, so `suite_timeouts()`
        // rather than `harness_with_seams` — the stop this restarted
        // supervisor runs still does real tmux work through the sweep.
        suite_timeouts(),
        SupervisorSeams {
            scopes,
            ..SupervisorSeams::default()
        },
    )
    .await
    .expect("second supervisor construction on the same state dir");
    assert!(
        restarted.owns_state_dir(),
        "the predecessor must be gone, or this proves nothing about a restart"
    );
    let client2 = connect_client(&restarted).await;
    assert_eq!(
        stored_launch_scope(state.path(), &session.id).await,
        scope_before,
        "the recorded selection must be unchanged by a supervisor restart"
    );

    client2
        .stop_session(&session.id)
        .await
        .expect("stop through the restarted supervisor");
    wait_until_pid_gone(cloaked_pid, 15).await;
}

/// A launch whose cgroup WRAPPER failed must classify as error, not as a
/// plain exit — PLAN_M3.md item 10's one new failure mode, and the one gap
/// the wrapper opened in item 3's sentinel contract.
///
/// The gap: every other launch failure is reported by farhelm's own exec
/// shim, which writes a sentinel before dying. `systemd-run` runs BEFORE the
/// shim, so a wrapper that fails (the user manager died since the probe, the
/// unit was refused) exits the pane with no sentinel at all — leaving a
/// session that reports "your agent ran and finished" about an agent that
/// never started, and a launch spec holding its full command line on disk
/// with nothing left to consume it.
///
/// The shape is PLANTED rather than provoked, exactly as
/// `a_planted_malformed_spec_sentinel_classifies_error_with_its_detail`
/// plants its sentinel: making a real `systemd-run` fail from inside a test
/// would mean sabotaging the host's user manager. What is planted is only
/// the evidence — an unconsumed spec on a dead pane — while the scope
/// selection under it is the real one this host's real probe made.
#[tokio::test]
async fn a_failed_scope_wrapper_classifies_as_error_rather_than_a_plain_exit() {
    let Some((h, _scopes)) =
        scope_gated_harness("a_failed_scope_wrapper_classifies_as_error_rather_than_a_plain_exit")
            .await
    else {
        return;
    };
    let (session, _work) = basic_session(&h).await;
    assert!(
        launch_scope_of(&h, &session.id).await.is_some(),
        "test setup: this launch must have selected a scope"
    );

    // Kill the agent outright so the pane is dead with no sentinel — the
    // state a failed wrapper leaves, reached the only way a test can.
    let sock = h.state.path().join("tmux.sock");
    let pid_out = tmux_query(
        &sock,
        &[
            "display-message",
            "-p",
            "-t",
            &format!("fh-{}", session.id),
            "#{pane_pid}",
        ],
    )
    .await;
    let pane_pid: u32 = String::from_utf8_lossy(&pid_out.stdout)
        .trim()
        .parse()
        .expect("a live pane must report a pid");
    // SAFETY: a real, currently-live pid this test just read from tmux.
    unsafe {
        libc::kill(pane_pid as libc::pid_t, libc::SIGKILL);
    }
    wait_until_pid_gone(pane_pid, 10).await;

    // The shim consumed and unlinked its own spec on the way past; putting
    // one back is what stands in for a wrapper that died before the shim
    // ever ran.
    let spec = spec_path_for_launch(h.state.path(), &session.id, 0);
    std::fs::write(&spec, b"{}").expect("plant an unconsumed launch spec");

    let found = wait_for_non_live_status(&h.client, &session.id, 15).await;
    let SessionStatus::Error { detail } = &found.status else {
        panic!("a launch that never reached the shim must classify as error, got {found:?}");
    };
    assert!(
        detail.contains("never reached farhelm's exec shim"),
        "the error must say the agent never started, got {detail:?}"
    );
    assert!(
        !spec.exists(),
        "classifying the failure must also clean up the credential-bearing spec the wrapper \
         left behind"
    );
}

/// The fallback proof, run on EVERY host including the ones that have a
/// manager: a supervisor with no usable user manager records no scope and
/// stops exactly as M2 did (PLAN_M3.md item 10, acceptance 10's second
/// half).
///
/// CI proves this incidentally by having no manager at all; this test
/// makes it provable on a developer machine too, through the injected
/// `ScopeManager::disabled()`. Without it, the fallback would be exercised
/// only where nobody is looking — and the assertion that matters most
/// (`launch_scope` is NULL, so stop is sweep-only) would never run beside
/// the scope path it must stay distinguishable from.
///
/// The cloaked daemon is deliberately NOT part of this test: with no
/// cgroup, nothing can reach it, and asserting its survival would pin a
/// known gap as if it were a feature.
#[tokio::test]
async fn without_a_user_manager_a_launch_records_the_fallback_and_stops_like_m2() {
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            scopes: Arc::new(farhelm_supervisor::scope::ScopeManager::disabled()),
            ..SupervisorSeams::default()
        },
    )
    .await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner-reparent"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    let self_pid = extract_pid(&seen, "SELF-PID:");
    let daemon_pid = wait_for_pid_file(&work.path().join("reparented.pid"), 10).await;

    assert_eq!(
        launch_scope_of(&h, &session.id).await,
        None,
        "a launch with no usable user manager must durably record the fallback"
    );

    h.client.stop_session(&session.id).await.expect("stop");

    // Exactly M2's guarantee, unchanged: the pane's own process and the
    // reparented marked daemon both die to the sweep alone.
    wait_until_pid_gone(self_pid, 15).await;
    wait_until_pid_gone(daemon_pid, 15).await;
}

/// Read one session's recorded cgroup SELECTION straight out of SQLite,
/// together with the unit name that selection derives.
///
/// Through the store rather than through any wire reply on purpose: the
/// selection is deliberately NOT wire vocabulary (PLAN_M3.md item 1 froze
/// M3's protocol before item 10 landed), so the durable column is the only
/// place it exists — and the durability is the property the tests are
/// actually about. The NAME is derived here exactly as the supervisor
/// derives it, never read back, because the database deliberately does not
/// store one (`store::StoredSession::launch_scoped`).
async fn launch_scope_of(h: &Harness, session_id: &str) -> Option<String> {
    stored_launch_scope(h.state.path(), session_id).await
}

/// [`launch_scope_of`] against a state directory rather than a live
/// harness, for the tests that dismantle their harness to release the
/// state dir before asking.
async fn stored_launch_scope(state_dir: &std::path::Path, session_id: &str) -> Option<String> {
    let store = SessionStore::open(&state_dir.join("supervisor.db"), false)
        .await
        .expect("opening the supervisor database read-only");
    let row = store
        .session(session_id)
        .await
        .expect("reading the session row")
        .expect("the session must still have a row");
    row.launch_scoped
        .then(|| farhelm_supervisor::scope::unit_name(session_id, row.generation))
        .flatten()
}

/// Delete must remove a session's launch artifacts, not just the row and
/// the terminal — `launch/<id>.json` can hold the agent's full command
/// line (credentials included, per launch.rs's own docs), and the shim
/// usually unlinks both files itself, but the ordinary case is exactly
/// what makes this easy to leave untested: this plants both files by hand
/// (standing in for a delete that outraces the shim, or a spec that was
/// never launched at all) so the removal path actually runs.
#[tokio::test]
async fn delete_removes_launch_artifacts() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    // Named per LAUNCH, not per session (`launch::spec_path_for_launch`):
    // this session has only ever launched once, so generation 0 is where
    // its files live.
    let spec_path = spec_path_for_launch(h.state.path(), &session.id, 0);
    let status_path = status_path_for_spec(&spec_path);
    wait_for_shim_to_consume_spec(&spec_path).await;
    std::fs::write(&spec_path, b"{}").expect("plant a launch spec");
    std::fs::write(&status_path, b"exec_failed").expect("plant a launch status file");

    h.client.delete_session(&session.id).await.expect("delete");

    assert!(
        !spec_path.exists(),
        "delete must remove the launch spec, which may hold credentials"
    );
    assert!(
        !status_path.exists(),
        "delete must remove the launch status file"
    );
}

/// Delete must remove a session's alt-screen stop snapshot — same
/// confidentiality class as the launch artifacts above (terminal content
/// can hold secrets an agent echoed), and delete is the last moment
/// anything comes back to clean it up. Stops an `altscreen` session first
/// so a snapshot genuinely exists, rather than asserting the absence of a
/// file that was never going to be there.
#[tokio::test]
async fn delete_removes_the_alt_screen_snapshot() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script altscreen"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "ALT-SCREEN APP", 20).await;
    h.client.detach(chan).await;

    h.client.stop_session(&session.id).await.expect("stop");

    let snapshot_path = h.state.path().join("snapshots").join(&session.id);
    assert!(
        snapshot_path.exists(),
        "test setup: stopping an alt-screen session must write a snapshot"
    );

    h.client.delete_session(&session.id).await.expect("delete");

    assert!(
        !snapshot_path.exists(),
        "delete must remove the alt-screen snapshot, which may hold secrets an agent echoed"
    );
}

/// Wait for the shim to have consumed and unlinked the REAL launch spec
/// at `spec_path` before a test plants a fake one at the same path —
/// otherwise planting could race the shim's own read and hand it garbage
/// instead of the real spec it needs to exec the fake agent.
async fn wait_for_shim_to_consume_spec(spec_path: &std::path::Path) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while spec_path.exists() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the real launch spec was never consumed by the shim"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Fail-closed artifact removal (SPEC.md/lore/2026-07-27-m2-process-tree-
/// stop.md): a launch spec that cannot be removed must fail the WHOLE
/// delete, row and map entry intact — never silently proceed and lose the
/// last handle on a file that may hold credentials. Removing WRITE
/// permission on the launch directory itself (not the file) is what
/// actually makes a file undeletable on POSIX: `unlink` needs write+exec
/// on the containing directory, not any particular mode on the file.
///
/// Skipped under euid 0: root bypasses directory permission checks
/// entirely, which would make this test pass trivially without
/// exercising the fail-closed path it exists to pin.
#[tokio::test]
async fn delete_fails_closed_when_a_launch_artifact_cannot_be_removed() {
    // SAFETY: geteuid takes no arguments and cannot fail.
    if unsafe { libc::geteuid() } == 0 {
        eprintln!(
            "skipping: running as root, which bypasses the directory permission this test relies on"
        );
        return;
    }

    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let launch_dir = h.state.path().join("launch");
    let spec_path = launch_dir.join(format!("{}.json", session.id));
    wait_for_shim_to_consume_spec(&spec_path).await;
    std::fs::write(&spec_path, b"{}").expect("plant a launch spec");

    use std::os::unix::fs::PermissionsExt;
    let original_mode = std::fs::metadata(&launch_dir)
        .expect("stat launch dir")
        .permissions()
        .mode();
    std::fs::set_permissions(&launch_dir, std::fs::Permissions::from_mode(0o500))
        .expect("restrict launch dir to read+execute only");

    let result = h.client.delete_session(&session.id).await;

    // Restored FIRST and unconditionally, before any assertion that could
    // panic — a permission-broken state dir must not outlive this test
    // regardless of how it ends.
    std::fs::set_permissions(&launch_dir, std::fs::Permissions::from_mode(original_mode))
        .expect("restore launch dir permissions");

    let err = result.expect_err("delete must fail closed when a launch artifact cannot be removed");
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("must carry a SupervisorError")
            .kind,
        ErrorKind::Internal,
        "an unremovable artifact is a server-side sweep problem, not a caller precondition"
    );

    let sup2 = Supervisor::new_with_exe(h.state.path(), farhelm_bin().into())
        .await
        .expect("second supervisor construction on the same state dir");
    let client2 = connect_client(&sup2).await;
    // Delete's process-tree sweep ran to completion (it happens before
    // the artifact removal that actually failed — see the handler's
    // ordering), so the agent is already dead by the time the row is
    // still-listed here — but it must be a genuinely EXITED row, not
    // live, even though the delete itself failed closed. Status is
    // computed fresh from tmux at list time, so this polls rather than a
    // single read (same reasoning as `wait_for_non_live_status`'s docs).
    let found = wait_for_non_live_status(&client2, &session.id, 15).await;
    assert_eq!(found.id, session.id);
    assert_eq!(found.title, session.title);
    assert_eq!(found.cwd, session.cwd);
    assert_eq!(found.invocation, session.invocation);
    assert!(
        matches!(found.status, SessionStatus::Exited { .. }),
        "a delete that already killed the process tree before failing closed must still \
         list the row as exited, not live, got {:?}",
        found.status
    );
    assert_eq!(
        client2
            .list_sessions()
            .await
            .expect("list from fresh supervisor")
            .sessions
            .len(),
        1,
        "a failed delete must leave the row in place for a retry, provable only through a \
         SEPARATE supervisor construction, not this process's own map"
    );

    let _ = std::fs::remove_file(&spec_path);
}

/// A best-effort race: attach from a second client, in a retry loop,
/// concurrently with a delete in flight. `DeleteSession`'s teardown sweep
/// deliberately runs BEFORE it takes the `attachments` lock (see that
/// handler's own comment), which is exactly what lets a concurrent Attach
/// install itself while the sweep is still running — the lock-held phase
/// then tears down WHATEVER attachment exists once it runs, new or old.
/// This test does not try to land in one specific interleaving; it
/// asserts that WHICHEVER one happens is internally consistent: an
/// attach either fails `NotFound` (delete's row/map removal already
/// happened) or succeeds and then receives a `Detached` (the lock-held
/// phase caught it) — never a hang, and never a "succeeded and stayed
/// attached forever" outcome once delete has actually finished. The
/// session must be gone by the end either way.
#[tokio::test]
async fn attach_during_delete_race_ends_in_a_consistent_state() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let deleter = Arc::clone(&h.client);
    let delete_session_id = session.id.clone();
    let delete_task = tokio::spawn(async move { deleter.delete_session(&delete_session_id).await });

    let second = h.second_client().await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        assert!(
            tokio::time::Instant::now() < deadline,
            "attach-during-delete race never reached a consistent outcome"
        );
        match second.attach(&session.id, 80, 24).await {
            Ok((_channel, mut rx)) => {
                // An attach that succeeded must be told WHY it no longer
                // holds the session, truthfully — a `Detached` naming
                // deletion, exactly as `DeleteSession`'s handler sends
                // once the row is confirmed gone (see its "notice after
                // commit" comment). A bare stream close (`None`) is NOT
                // an acceptable alternative: it would mean the client
                // learned the session vanished with no explanation, which
                // is the same silent-disappearance failure the handler's
                // ordering exists to prevent.
                let reason = tokio::time::timeout(Duration::from_secs(15), async {
                    loop {
                        match rx.recv().await {
                            Some(TermEvent::Detached(reason)) => return reason,
                            // Presentation metadata this loop is only
                            // waiting past — see the identical treatment
                            // (and its rationale) in
                            // `connection_loss_detaches_terminals_and_fails_requests`
                            // above.
                            Some(TermEvent::Data(_)) | Some(TermEvent::ReplayComplete) => continue,
                            None => panic!(
                                "an attachment that raced a delete closed without a Detached \
                                 notice — the client learned nothing about why"
                            ),
                        }
                    }
                })
                .await
                .expect("an attachment that raced a delete must resolve to Detached");
                assert!(
                    reason.contains("delete"),
                    "Detached reason for a racer that saw a successful delete must name \
                     deletion, got: {reason:?}"
                );
                break;
            }
            Err(e) => {
                // `NotFound` is delete's row/map removal having already
                // landed. `Internal` is the OTHER legitimate shape of
                // this exact race: the entry is still in the map (not
                // removed yet) but delete's teardown already killed the
                // tmux session underneath it, so Attach's own tmux calls
                // fail with an ordinary (unclassified) tmux error. Both
                // are consistent outcomes of "delete got there first";
                // anything else is retried, bounded by the outer
                // deadline, since it may just be a transient blip rather
                // than the race settling.
                let expected_race_outcome = e
                    .downcast_ref::<SupervisorError>()
                    .is_some_and(|se| matches!(se.kind, ErrorKind::NotFound | ErrorKind::Internal));
                if expected_race_outcome {
                    break;
                }
            }
        }
    }

    delete_task
        .await
        .expect("delete task panicked")
        .expect("delete must succeed");
    assert!(
        h.client.list_sessions().await.unwrap().sessions.is_empty(),
        "the session must be gone once the race settles"
    );
}

/// The acceptance test for `kill_process_tree`'s SIGSTOP-quiesce phase: a
/// child that continuously forks new marked grandchildren — each one
/// deliberately long-lived (`sleep 3600`, never exiting on its own) —
/// must leave NONE alive after stop, including ones that forked in the
/// narrow gap between SIGTERM and the sweep's later signals — the exact
/// race quiesce exists to close (see that function's docs and
/// lore/2026-07-27-m2-process-tree-stop.md).
///
/// Long-lived grandchildren are the point, not an incidental detail: a
/// SHORT-lived grandchild dies of natural causes within a few hundred
/// milliseconds regardless of whether the sweep ever reaches it, which
/// would let this test pass even with quiescing removed — the opposite
/// of what it exists to catch. Checked immediately after `stop_session`
/// returns, with no bounded retry: `kill_process_tree` already waits out
/// its own confirmation window (`confirm_gone`) before returning `Ok`, so
/// a survivor at this point is a survivor, not a straggler about to die
/// on its own.
///
/// This test's discriminating power was verified empirically while
/// writing it: temporarily disabling BOTH the post-grace SIGSTOP
/// re-enumeration and the `for _ in 0..MAX_QUIESCE_PASSES` fixpoint loop
/// in `kill_process_tree` (so the sweep goes straight from round one's
/// SIGTERM snapshot to a final SIGKILL of that same stale set, with no
/// re-enumeration at all) made this test fail reliably — a marked
/// grandchild that forked during the grace period, after round one's
/// snapshot, survived indefinitely, since nothing ever signaled it —
/// across repeated runs, while the real code passes just as reliably.
/// (An earlier attempt at this same verification, with the fork-storm
/// fixture's forking loop process left to die to a plain `SIGTERM`,
/// passed even with quiescing disabled: the loop's own death — and, it
/// turned out, a `SIGHUP` cascade to its whole foreground process group
/// once the pane's session-leader process died — stopped the storm
/// almost immediately either way, closing the race window before it
/// could matter. That is why the fixture ignores both `TERM` and `HUP`.)
/// The disabling change was reverted before this test was committed; it
/// must never be left disabled in the source.
#[tokio::test]
async fn stop_quiesce_survives_no_marked_process() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner-fork-storm"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    // Let the storm actually produce a few generations before stopping,
    // so there is something for the sweep to race against rather than a
    // trivially-empty tree.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !marked_pids(&session.id).is_empty(),
        "test setup: the fork storm must have produced at least one live marked process by now"
    );

    h.client.stop_session(&session.id).await.expect("stop");

    let survivors = marked_pids(&session.id);
    assert!(
        survivors.is_empty(),
        "marked process(es) survived stop: {survivors:?} — the quiesce fixpoint let a fork \
         through"
    );
}
