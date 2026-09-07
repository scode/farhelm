//! The attach replay-complete marker, pinned against real tmux.
//!
//! Driven at the raw protocol level, and it has to be: the marker is an
//! unsolicited `ControlMsg::ReplayComplete` correlated by channel, and
//! `SupervisorClient` has no place to surface one until the helm's own
//! pass-through lands (PLAN_M5.md item 4). Reading frames directly is also
//! what makes the ORDER assertable at all — the whole contract is a
//! statement about where the marker sits between data frames, and any
//! client API that turned frames into events would have to preserve that
//! order to be worth testing through anyway.

use crate::harness::*;

use crate::terminal_backpressure::force_tmux_pause;

// ---------------------------------------------------------------------
// Replay-complete marker (PLAN_M5.md item 2)
//
// The contract in one sentence: exactly one marker per attach THAT
// COMPLETES ITS CATCH-UP, after every byte that attach replays from
// history and before the first byte of live output. An attach ended early
// may never receive one at all — and the two endings differ in what the
// client sees: a takeover, delete or stall sends a `Detached` that ends
// the catch-up phase in the marker's place, while a client's own `Detach`
// sends nothing back, since the client that asked already knows. The
// qualifier has its own test below rather than being left as a gap
// between the other ones.
//
// Every other test here is some way of trying to break the main clause — a
// deep replay chunked across many frames, live output generated while the
// catch-up is still in flight, a replay path that appends a snapshot after
// the prefill, a second attach to the same terminal, a takeover, and the
// one catch-up that must stay markerless.
// ---------------------------------------------------------------------

/// One thing that arrived on the connection under test, in arrival order.
///
/// Data is kept as bytes rather than folded into one buffer so a marker's
/// position relative to individual frames survives — "the marker came
/// after the last replay chunk" is not expressible once the transcript has
/// been flattened. The marker carries the channel it named, because
/// channel correlation is part of the contract and an implementation that
/// hard-coded a channel would otherwise pass every test here.
#[derive(Debug)]
enum Arrival {
    Data(Vec<u8>),
    Marker(u32),
}

/// One raw connection plus the ordered transcript of a single attachment.
///
/// Deliberately one channel per peer. Channel ids are connection-local and
/// a takeover from the same connection would need a second one, but every
/// scenario here is about what ONE client saw, and giving each peer its
/// own connection is also what makes the takeover tests honest: two
/// clients of the same supervisor, not two channels of one.
struct MarkerPeer {
    reader: FrameReader<tokio::io::ReadHalf<tokio::io::DuplexStream>>,
    writer: FrameWriter<tokio::io::WriteHalf<tokio::io::DuplexStream>>,
    channel: u32,
    arrivals: Vec<Arrival>,
    /// The reason from a `Detached` for this channel, once one arrives.
    /// Recorded rather than treated as an error: a takeover's incumbent is
    /// SUPPOSED to see one, and what matters is that it never sees the
    /// replacement's marker.
    detached: Option<String>,
}

/// `req_id` for the barrier request [`MarkerPeer::detach`] uses. Distinct
/// from every real request here so its reply cannot be mistaken for one.
const BARRIER_REQ: u64 = 9_999;

impl MarkerPeer {
    /// Connect, complete the hello, and take `channel` as this peer's
    /// attachment channel for the rest of the test.
    ///
    /// The channel is a parameter rather than a constant because one test
    /// deliberately uses a non-default value: the marker correlates by
    /// channel, and a supervisor that always said `1` would be
    /// indistinguishable from a correct one if every test attached on
    /// channel 1.
    async fn connect(sup: &Arc<Supervisor>, channel: u32) -> MarkerPeer {
        MarkerPeer::connect_with_buffer(sup, channel, 1 << 20).await
    }

    /// [`MarkerPeer::connect`] with an explicit transport buffer.
    ///
    /// A SMALL buffer is how a test makes the supervisor's delivery
    /// observably in progress: with a megabyte of slack a whole replay
    /// disappears into the pipe before the client has read a byte, so
    /// "this happened during the catch-up" becomes unprovable from the
    /// client's side.
    async fn connect_with_buffer(sup: &Arc<Supervisor>, channel: u32, bytes: usize) -> MarkerPeer {
        let (client_side, server_side) = tokio::io::duplex(bytes);
        let sup = Arc::clone(sup);
        tokio::spawn(async move {
            let _ = handle_connection(sup, server_side).await;
        });
        let (read_half, write_half) = tokio::io::split(client_side);
        let mut peer = MarkerPeer {
            reader: FrameReader::new(read_half),
            writer: FrameWriter::new(write_half),
            channel,
            arrivals: Vec::new(),
            detached: None,
        };
        handshake(&mut peer.reader, &mut peer.writer, "helm")
            .await
            .expect("handshake");
        peer
    }

    /// Attach `selector`, returning once the `Attached` reply has arrived.
    ///
    /// Asserts on the way that NOTHING for this channel preceded that
    /// reply. That ordering is not incidental: the supervisor enqueues the
    /// reply before the forwarder exists precisely so a client can start
    /// draining before the replay lands on it, and a marker test that let
    /// replay bytes arrive first would be measuring a different sequence
    /// than the one clients see. It is also what makes "everything after
    /// this call is live" true for the input the tests send next.
    async fn attach(&mut self, session_id: &str, selector: TerminalSelector, lease: &str) {
        self.writer
            .write_control(&ControlMsg::Attach {
                req_id: 1,
                session_id: session_id.to_string(),
                channel: self.channel,
                cols: 80,
                rows: 24,
                terminal: selector,
                lease: lease.to_string(),
                if_unowned: false,
            })
            .await
            .expect("write attach");
        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
        loop {
            let frame = self.read_frame(deadline, "the Attached reply").await;
            if frame.kind == FrameKind::Control {
                match parse_control(&frame).expect("parse control") {
                    ControlMsg::Attached { channel, .. } => {
                        assert_eq!(channel, self.channel, "Attached named another channel");
                        assert!(
                            self.arrivals.is_empty(),
                            "replay reached the client before its Attached reply: {:?}",
                            self.arrivals
                        );
                        return;
                    }
                    ControlMsg::Error { message, kind, .. } => {
                        panic!("attach refused ({kind:?}): {message}")
                    }
                    other => self.record_control(other),
                }
            } else {
                self.record_data(frame);
            }
        }
    }

    /// Send terminal input as a data frame on this peer's channel.
    ///
    /// Whatever the pane makes of it comes back as LIVE output on the same
    /// channel: the pty echoes the keystrokes and the fake agent answers
    /// on a line of its own, and both are produced strictly after this
    /// call — so an assertion about where they land relative to the marker
    /// is an assertion about the cutover, not about timing.
    async fn input(&mut self, bytes: &[u8]) {
        self.writer
            .write_frame(&Frame::data(self.channel, bytes.to_vec()))
            .await
            .expect("write input");
    }

    /// Give the attachment up and WAIT until the supervisor has acted on
    /// it, so a later attach is a reattach rather than a takeover.
    ///
    /// The wait is the whole point. `Detach` has no reply of its own, so
    /// returning at the write would leave the next attach racing this
    /// teardown — and the two paths differ (a takeover kicks an incumbent
    /// and reports a different reason), so a test that meant to exercise
    /// the reattach would silently exercise the takeover on a loaded
    /// machine. The barrier is a deliberately invalid attach: channel 0 is
    /// reserved, so it is refused immediately and synchronously, and one
    /// connection's control frames are handled strictly in order — so its
    /// refusal arriving proves the `Detach` ahead of it was fully handled.
    async fn detach(&mut self) {
        self.writer
            .write_control(&ControlMsg::Detach {
                channel: self.channel,
            })
            .await
            .expect("write detach");
        self.writer
            .write_control(&ControlMsg::Attach {
                req_id: BARRIER_REQ,
                session_id: String::new(),
                channel: 0,
                cols: 80,
                rows: 24,
                terminal: TerminalSelector::Agent,
                lease: String::new(),
                if_unowned: false,
            })
            .await
            .expect("write the detach barrier");
        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
        loop {
            let frame = self
                .read_frame(deadline, "the detach barrier's reply")
                .await;
            if frame.kind == FrameKind::Control {
                match parse_control(&frame).expect("parse control") {
                    ControlMsg::Error {
                        req_id: BARRIER_REQ,
                        ..
                    } => return,
                    other => self.record_control(other),
                }
            } else {
                self.record_data(frame);
            }
        }
    }

    /// Read frames until `done` holds over everything recorded so far,
    /// failing the test (with the transcript) rather than hanging.
    async fn wait_until(&mut self, secs: u64, what: &str, done: impl Fn(&MarkerPeer) -> bool) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
        while !done(self) {
            let frame = self.read_frame(deadline, what).await;
            if frame.kind == FrameKind::Control {
                let msg = parse_control(&frame).expect("parse control");
                self.record_control(msg);
            } else {
                self.record_data(frame);
            }
        }
    }

    /// Read until this attach's marker arrives, failing the test if it
    /// never does. A hang here is the regression the whole module exists
    /// to catch: a client waiting for a marker that is never sent shows
    /// the user a terminal that never appears.
    async fn wait_for_marker(&mut self, secs: u64) {
        self.wait_until(secs, "the replay-complete marker", |peer| {
            peer.markers() > 0
        })
        .await;
    }

    /// Read until `needle` has appeared anywhere in this channel's data,
    /// leaving everything received recorded IN ORDER for the caller's own
    /// position assertions — which is why this returns nothing itself.
    async fn wait_for_text(&mut self, needle: &str, secs: u64) {
        self.wait_until(secs, &format!("terminal text {needle:?}"), |peer| {
            peer.text(..).contains(needle)
        })
        .await;
    }

    /// Keep reading for `window`, recording everything, to observe the
    /// ABSENCE of something. A needle-driven wait cannot express that.
    ///
    /// A stream that ENDS during the window fails the test rather than
    /// counting as quiet: every caller is about to assert that something
    /// did not arrive, and "the connection died" must never be allowed to
    /// masquerade as "nothing more was sent".
    async fn drain_for(&mut self, window: Duration) {
        let deadline = tokio::time::Instant::now() + window;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return;
            }
            match tokio::time::timeout(remaining, self.reader.read_frame()).await {
                // The window expired with nothing more arriving, which is
                // what every caller is here to establish.
                Err(_) => return,
                Ok(Ok(Some(frame))) if frame.kind == FrameKind::Control => {
                    let msg = parse_control(&frame).expect("parse control");
                    self.record_control(msg);
                }
                Ok(Ok(Some(frame))) => self.record_data(frame),
                Ok(Ok(None)) => panic!(
                    "the connection closed while draining; transcript so far:\n{}",
                    self.transcript()
                ),
                Ok(Err(e)) => panic!(
                    "the connection failed while draining ({e}); transcript so far:\n{}",
                    self.transcript()
                ),
            }
        }
    }

    async fn read_frame(&mut self, deadline: tokio::time::Instant, what: &str) -> Frame {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        tokio::time::timeout(remaining, self.reader.read_frame())
            .await
            .unwrap_or_else(|_| {
                panic!(
                    "timed out waiting for {what}; transcript so far:\n{}",
                    self.transcript()
                )
            })
            .expect("read frame")
            .unwrap_or_else(|| {
                panic!(
                    "connection closed while waiting for {what}; transcript so far:\n{}",
                    self.transcript()
                )
            })
    }

    /// Record a data frame, ignoring one for another channel — a peer
    /// holding one attachment should never see another's, and silently
    /// folding one in would corrupt exactly the assertions below.
    fn record_data(&mut self, frame: Frame) {
        if frame.channel == self.channel {
            self.arrivals.push(Arrival::Data(frame.body));
        }
    }

    /// Record the two unsolicited notices this peer cares about. The
    /// marker joins the arrival sequence because its POSITION there is the
    /// whole subject, and it is recorded WHATEVER channel it names, so a
    /// mis-correlated marker surfaces as a failure rather than as silence.
    /// A `Detached` is kept to one side, since it ends the attachment
    /// rather than sitting inside it.
    fn record_control(&mut self, msg: ControlMsg) {
        match msg {
            ControlMsg::ReplayComplete { channel } => self.arrivals.push(Arrival::Marker(channel)),
            ControlMsg::Detached { channel, reason } if channel == self.channel => {
                self.detached = Some(reason);
            }
            _ => {}
        }
    }

    /// How many markers arrived. Counted rather than merely detected: the
    /// contract is exactly one per attach, and "at least one" would pass
    /// against an implementation that emitted one per replay chunk.
    fn markers(&self) -> usize {
        self.arrivals
            .iter()
            .filter(|a| matches!(a, Arrival::Marker(_)))
            .count()
    }

    /// The position of the first marker among the arrivals, panicking with
    /// the whole transcript when none arrived — every caller is about to
    /// slice the transcript around it, and an `Option` here would only
    /// move the same failure somewhere less legible.
    fn marker_at(&self) -> usize {
        self.arrivals
            .iter()
            .position(|a| matches!(a, Arrival::Marker(_)))
            .unwrap_or_else(|| panic!("no marker arrived; transcript:\n{}", self.transcript()))
    }

    /// The data frames in `range`, decoded loosely as text. Lossy on
    /// purpose: replay bytes are arbitrary terminal history, and a chunk
    /// boundary can split a UTF-8 sequence.
    fn text(&self, range: impl std::slice::SliceIndex<[Arrival], Output = [Arrival]>) -> String {
        let mut bytes = Vec::new();
        for arrival in &self.arrivals[range] {
            if let Arrival::Data(chunk) = arrival {
                bytes.extend_from_slice(chunk);
            }
        }
        String::from_utf8_lossy(&bytes).into_owned()
    }

    /// The whole transcript with the marker's position marked, for failure
    /// messages: every assertion here is about order, so a failure that
    /// only printed the bytes would leave the reader guessing.
    fn transcript(&self) -> String {
        let mut out = String::new();
        for arrival in &self.arrivals {
            match arrival {
                Arrival::Data(chunk) => out.push_str(&String::from_utf8_lossy(chunk)),
                Arrival::Marker(channel) => {
                    out.push_str(&format!("\n<<<REPLAY-COMPLETE channel={channel}>>>\n"));
                }
            }
        }
        out
    }
}

/// How long [`assert_marker_splits`] keeps reading before it counts
/// markers. Short enough to pay in every test that asserts the contract,
/// long enough that a marker enqueued behind the frame the caller stopped
/// at has landed: everything here is a local pipe, so the gap being
/// covered is scheduling, not transfer.
const SETTLE_WINDOW: Duration = Duration::from_millis(750);

/// The whole contract, as one assertion over a settled transcript: exactly
/// one marker, on this peer's own channel, with `replayed` on the history
/// side of it and `live` only on the far side.
///
/// Every clause is needed. A marker that fires too early splits a replay
/// and shows up as `replayed` missing from the prefix; one that fires too
/// late (or after live output has started) shows up as `live` appearing in
/// the prefix; and a `live` string appearing on both sides would be output
/// delivered twice, once through the replay and once live.
///
/// It drains before counting, and takes `&mut` for that reason alone.
/// Callers stop reading the instant their live text appears, so a SECOND
/// marker emitted just behind that frame would still be sitting in the
/// pipe — unread, uncounted, and exactly the regression the count exists
/// to catch.
async fn assert_marker_splits(peer: &mut MarkerPeer, replayed: &str, live: &str) {
    peer.drain_for(SETTLE_WINDOW).await;
    assert_eq!(
        peer.markers(),
        1,
        "expected exactly one marker for this attach; transcript:\n{}",
        peer.transcript()
    );
    let at = peer.marker_at();
    assert!(
        matches!(peer.arrivals[at], Arrival::Marker(channel) if channel == peer.channel),
        "the marker named the wrong channel; transcript:\n{}",
        peer.transcript()
    );
    let before = peer.text(..at);
    let after = peer.text(at..);
    assert!(
        before.contains(replayed),
        "replayed text {replayed:?} must arrive BEFORE the marker; transcript:\n{}",
        peer.transcript()
    );
    assert!(
        !before.contains(live),
        "live text {live:?} arrived before the marker, so the marker did not bound the \
         catch-up; transcript:\n{}",
        peer.transcript()
    );
    assert!(
        after.contains(live),
        "live text {live:?} must arrive after the marker; transcript:\n{}",
        peer.transcript()
    );
}

/// The `basic` fake agent's answer to a line — the one form of a typed
/// string that appears EXACTLY once in a transcript, since the pty echoes
/// the keystrokes as well and the bare string is therefore always there
/// twice over.
fn agent_echo_of(line: &str) -> String {
    format!("echo:\x1b[36m{line}")
}

/// Fill a `basic` fake agent's scrollback through a throwaway attachment
/// so a LATER attach has a deep replay to deliver: a uniquely spelled
/// first row, then `lines` numbered rows on top of it.
///
/// The sentinel is spelled unlike the numbered rows on purpose. Asserting
/// on `spam-line-1` would also match `spam-line-1000`, so losing the head
/// of a replay — precisely what a marker emitted too early produces —
/// could hide behind a line printed a thousand rows later.
///
/// Detaching before returning is what makes the next attach a genuine
/// reattach rather than a takeover, keeping the scenario the one users
/// actually hit (close the session view, open it again).
async fn seed_deep_scrollback(sup: &Arc<Supervisor>, session_id: &str, lines: usize) {
    let mut seeder = MarkerPeer::connect(sup, 1).await;
    seeder
        .attach(session_id, TerminalSelector::Agent, "seeder")
        .await;
    seeder.wait_for_text("FAKE-AGENT READY", 30).await;
    seeder.input(b"OLDEST-RETAINED-ROW\r").await;
    seeder
        .wait_for_text(&agent_echo_of("OLDEST-RETAINED-ROW"), 30)
        .await;
    seeder.input(format!("spam {lines}\r").as_bytes()).await;
    seeder
        .wait_for_text(&format!("spam-line-{lines}"), 90)
        .await;
    seeder.detach().await;
}

/// The headline contract, with live output generated WHILE the catch-up is
/// still being delivered: the marker lands after every replayed byte and
/// before that output, which arrives exactly once.
///
/// The ordering of the test itself is the point. Sending the input only
/// after the marker had already arrived would make "before any live byte"
/// unfalsifiable — there would be no live byte that COULD have come first.
/// Here the input goes out the instant the attach is acknowledged, with a
/// transport buffer far smaller than the replay, so the catch-up cannot
/// have been handed over in one go. The pane's answer cannot be in the
/// replay either way — the replay was captured before the `Attached` reply
/// this test waits for, so anything the input produces is live by
/// construction.
///
/// What that does and does not establish, stated plainly. The client can
/// be holding replay bytes it has not recorded: `FrameReader` reads ahead
/// into its own buffer, so up to roughly a transport buffer's worth may
/// be sitting there decoded-but-unparsed while the pipe has already
/// refilled with about as much again. Two buffers is therefore the bound
/// on what could have been handed over, and the assertion below requires
/// the replay to exceed it — which is what makes "delivery was still
/// under way when the input went out" follow rather than merely sound
/// likely.
///
/// It still does NOT establish that the supervisor's forwarder was mid-
/// write, which is the strongest form. The forwarder can only be held
/// inside a replay by filling the connection's writer queue, and a replay
/// cannot do that (retained history is far smaller than 64 frames of
/// 32 KiB), so that version needs a product seam. This exercises the
/// boundary from the client's side instead.
///
/// This is the test the whole milestone rests on — the UI batches
/// everything before the marker into one write and reveals the terminal at
/// it, so a marker on the wrong side of either boundary is either a
/// half-painted catch-up or a terminal that never appears.
#[tokio::test]
async fn an_agent_reattach_marks_the_boundary_before_output_made_during_the_catch_up() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    seed_deep_scrollback(&h.sup, &session.id, 4000).await;

    const TRANSPORT_BUFFER: usize = 4 * 1024;
    let mut peer = MarkerPeer::connect_with_buffer(&h.sup, 1, TRANSPORT_BUFFER).await;
    peer.attach(&session.id, TerminalSelector::Agent, "viewer")
        .await;
    // `attach` has already asserted that no replay frame was RECORDED
    // before the `Attached` reply. At most about two transport buffers
    // could have been handed over anyway — one sitting unparsed in the
    // reader, one back in the refilled pipe — and the replay is larger
    // than that (asserted below), so delivery was still under way when
    // this input went out. See the docs above for what that does not
    // claim.
    peer.input(b"SENT-DURING-CATCH-UP\r").await;
    let live = agent_echo_of("SENT-DURING-CATCH-UP");
    peer.wait_for_text(&live, 60).await;

    assert_marker_splits(&mut peer, "OLDEST-RETAINED-ROW", &live).await;
    let at = peer.marker_at();
    assert!(
        peer.text(..at).len() > 2 * TRANSPORT_BUFFER,
        "test premise: the replay must exceed what the transport plus the reader's own \
         read-ahead could already have taken, or delivery could have finished before the \
         input was written"
    );
    assert_eq!(
        peer.text(at..).matches(&live).count(),
        1,
        "output produced during the catch-up must be delivered exactly once; transcript:\n{}",
        peer.transcript()
    );
}

/// A replay deep enough to span many data frames still gets exactly one
/// marker, and it still comes after the LAST of them.
///
/// The single-frame case cannot distinguish "the marker follows the
/// replay" from "the marker follows the first replay frame". 4000 spam
/// rows are tens of kilobytes — comfortably past the 32 KiB chunk the
/// supervisor splits a replay at, which the size assertion below keeps
/// honest — so a marker emitted anywhere but after the final chunk lands
/// with the tail of the history still to come, which is precisely the
/// half-painted reattach M5 exists to remove.
#[tokio::test]
async fn a_deep_scrollback_reattach_marks_only_after_the_last_replay_chunk() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    seed_deep_scrollback(&h.sup, &session.id, 4000).await;

    let mut peer = MarkerPeer::connect(&h.sup, 1).await;
    peer.attach(&session.id, TerminalSelector::Agent, "viewer")
        .await;
    peer.wait_for_marker(60).await;
    peer.input(b"AFTER-DEEP-REPLAY\r").await;
    let live = agent_echo_of("AFTER-DEEP-REPLAY");
    peer.wait_for_text(&live, 60).await;

    // The uniquely spelled first row proves the whole retained history
    // replayed, not just the visible screen; the last spam row is what the
    // marker must not have overtaken.
    assert_marker_splits(&mut peer, "OLDEST-RETAINED-ROW", &live).await;
    let at = peer.marker_at();
    let before = peer.text(..at);
    assert!(
        before.contains("spam-line-4000"),
        "the marker preceded the end of the replay; transcript:\n{}",
        peer.transcript()
    );
    assert!(
        before.len() > 32 * 1024,
        "test premise: the replay must span more than one 32 KiB chunk, got {} bytes",
        before.len()
    );
}

/// A tab attach gets the same marker as the agent attach — on the channel
/// that attach named, which here is deliberately not the default.
///
/// Two things at once, because they share a fixture. Tabs and the agent
/// terminal share the cutover machinery, so a marker for a tab SHOULD fall
/// out — which is exactly why it is worth pinning: a change that
/// special-cased the agent terminal would leave every tab hidden behind a
/// catch-up that never ends. And the channel: `ReplayComplete` correlates
/// by channel, so with every test attaching on channel 1 a hard-coded
/// `channel: 1` would pass them all. This attach runs on channel 7, and
/// [`assert_marker_splits`] checks the marker names it.
#[tokio::test]
async fn a_tab_attach_is_marked_on_the_channel_it_attached() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let tab = h.client.open_tab(&session.id).await.expect("open tab");

    // A shell, not the fake agent: `echo` is what produces both the
    // replayed line and the live one here.
    let mut seeder = MarkerPeer::connect(&h.sup, 1).await;
    seeder
        .attach(
            &session.id,
            TerminalSelector::Tab { id: tab.id.clone() },
            "seeder",
        )
        .await;
    seeder.input(b"echo TAB-REPLAYED\r").await;
    seeder.wait_for_text("TAB-REPLAYED", 30).await;
    seeder.detach().await;

    let mut peer = MarkerPeer::connect(&h.sup, 7).await;
    peer.attach(&session.id, TerminalSelector::Tab { id: tab.id }, "viewer")
        .await;
    peer.wait_for_marker(30).await;
    peer.input(b"echo TAB-LIVE\r").await;
    peer.wait_for_text("TAB-LIVE", 30).await;

    assert_marker_splits(&mut peer, "TAB-REPLAYED", "TAB-LIVE").await;
}

/// An alternate-screen session takes the visible-snapshot replay path,
/// and gets its marker at the same place.
///
/// A different capture, a different mode sequence, and content that must
/// be written AFTER the alternate-screen switch — none of which the
/// normal-screen tests exercise. The pin is that the marker follows the
/// whole of that assembled replay rather than, say, the prefill alone.
#[tokio::test]
async fn an_alternate_screen_attach_marks_after_its_visible_snapshot() {
    let h = harness().await;
    let work = farhelm_teststate::tempdir().expect("workdir");
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

    let mut seeder = MarkerPeer::connect(&h.sup, 1).await;
    seeder
        .attach(&session.id, TerminalSelector::Agent, "seeder")
        .await;
    seeder.wait_for_text("ALT-SCREEN APP", 30).await;
    seeder.detach().await;

    let mut peer = MarkerPeer::connect(&h.sup, 1).await;
    peer.attach(&session.id, TerminalSelector::Agent, "viewer")
        .await;
    peer.wait_for_marker(30).await;
    // Keystrokes WITHOUT a newline: the pty echoes them straight back as
    // live output, which is all this test needs, while a complete line
    // would make the fixture leave the alternate screen and exit — putting
    // the echo it is waiting for in a race with the pane's death.
    peer.input(b"ALT-LIVE").await;
    peer.wait_for_text("ALT-LIVE", 30).await;

    assert_marker_splits(&mut peer, "ALT-SCREEN APP", "ALT-LIVE").await;
}

/// A dead pane's attach is marked after its whole replay, with no OUTPUT
/// after it.
///
/// A session whose agent exited produces no live output, which makes this
/// the sharpest form of the "before any live byte" half: no data frame may
/// follow the marker at all. Control frames still may and do — a dead
/// pane's control client ends, and the `Detached` that reports it is
/// legitimate — so what is asserted is the absence of DATA within the
/// drained window, not silence.
///
/// The retained content is seeded before the agent exits so the replay has
/// something deterministic in it; the alternative, asserting on whatever a
/// dead pane's captured SCREEN holds, is version-dependent and would be
/// asserting on tmux rather than on the marker.
///
/// Without a marker here a client that waits for one before revealing the
/// terminal would leave an exited session's scrollback hidden forever,
/// which is what makes "a dead terminal emits it anyway" load-bearing
/// rather than tidy.
#[tokio::test]
async fn a_dead_pane_attach_is_marked_after_its_replay_with_no_output_after_it() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let mut seeder = MarkerPeer::connect(&h.sup, 1).await;
    seeder
        .attach(&session.id, TerminalSelector::Agent, "seeder")
        .await;
    seeder.wait_for_text("FAKE-AGENT READY", 30).await;
    seeder.input(b"BEFORE-THE-AGENT-EXITED\r").await;
    seeder
        .wait_for_text(&agent_echo_of("BEFORE-THE-AGENT-EXITED"), 30)
        .await;
    seeder.input(b"quit\r").await;
    seeder.detach().await;
    // Waiting on tmux's own `pane_dead`, not on the farewell text: the
    // agent's last bytes and its exit race, and the attach below has to
    // happen against a genuinely dead pane to exercise the path.
    let sock = h.state.path().join("tmux.sock");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        let out = tmux_query(&sock, &["display-message", "-p", "#{pane_dead}"]).await;
        if String::from_utf8_lossy(&out.stdout).trim() == "1" {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the agent never exited after quit"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let mut peer = MarkerPeer::connect(&h.sup, 1).await;
    peer.attach(&session.id, TerminalSelector::Agent, "viewer")
        .await;
    peer.wait_for_marker(30).await;
    peer.drain_for(Duration::from_secs(2)).await;

    assert_eq!(
        peer.markers(),
        1,
        "a dead-pane attach must be marked exactly once; transcript:\n{}",
        peer.transcript()
    );
    let at = peer.marker_at();
    assert!(
        peer.text(..at).contains("BEFORE-THE-AGENT-EXITED"),
        "the retained content must be replayed before the marker; transcript:\n{}",
        peer.transcript()
    );
    assert!(
        peer.arrivals[at..]
            .iter()
            .all(|a| matches!(a, Arrival::Marker(_))),
        "a dead pane has no live output, so no data frame may follow the marker; \
         transcript:\n{}",
        peer.transcript()
    );
}

/// The marker does not wait for output that will never come.
///
/// `flood-gated` says READY and then produces NOTHING until it is sent its
/// gate byte, so this reattach replays real history and then has no live
/// byte to deliver at all. An implementation that emitted the marker on
/// first live output — an easy way to make every other test here pass —
/// hangs at exactly that point, which this test observes as a timeout
/// rather than as a subtly late marker.
#[tokio::test]
async fn an_attach_is_marked_even_when_no_live_output_ever_follows() {
    let h = harness().await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script flood-gated"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    let mut seeder = MarkerPeer::connect(&h.sup, 1).await;
    seeder
        .attach(&session.id, TerminalSelector::Agent, "seeder")
        .await;
    seeder.wait_for_text("FAKE-AGENT READY", 30).await;
    seeder.detach().await;

    let mut peer = MarkerPeer::connect(&h.sup, 1).await;
    peer.attach(&session.id, TerminalSelector::Agent, "viewer")
        .await;
    peer.wait_for_marker(30).await;
    // Settle before counting: stopping at the first marker would leave a
    // second one sitting unread in the pipe, which is exactly what the
    // count exists to catch (see `assert_marker_splits`).
    peer.drain_for(SETTLE_WINDOW).await;
    let at = peer.marker_at();
    assert!(
        peer.text(..at).contains("FAKE-AGENT READY"),
        "the replay must have been delivered before the marker; transcript:\n{}",
        peer.transcript()
    );
    assert_eq!(
        peer.markers(),
        1,
        "an attach with nothing live to follow must still be marked exactly once; \
         transcript:\n{}",
        peer.transcript()
    );
}

/// A fresh terminal is marked without needing anything to replay.
///
/// The attach races the fake agent's first output, so this cannot
/// guarantee an EMPTY replay — what it pins is that an attach made
/// immediately after create is marked exactly once and keeps working
/// afterwards. A marker gated on "there was something to replay" passes
/// every other test here and hangs a freshly created session's terminal.
#[tokio::test]
async fn a_fresh_terminal_attach_is_marked_once() {
    let h = harness().await;
    // Keep the attach racing startup: waiting for agent readiness would
    // remove the empty-history boundary this test is meant to exercise.
    let (session, _work) = basic_session_mid_launch(&h).await;

    let mut peer = MarkerPeer::connect(&h.sup, 1).await;
    peer.attach(&session.id, TerminalSelector::Agent, "viewer")
        .await;
    peer.wait_for_marker(30).await;
    peer.wait_for_text("FAKE-AGENT READY", 30).await;
    // Settle before counting, for the same reason every other count here
    // does: a second marker just behind the frame this stopped at would
    // otherwise go unread and uncounted.
    peer.drain_for(SETTLE_WINDOW).await;
    assert_eq!(
        peer.markers(),
        1,
        "a fresh attach must be marked exactly once; transcript:\n{}",
        peer.transcript()
    );
}

/// A takeover gives the REPLACEMENT its own marker and never sends one to
/// the incumbent it displaced.
///
/// The marker describes one attachment's catch-up, not the session's, and
/// the two clients here are the sharpest way to say so: the incumbent has
/// already finished its own catch-up, and a marker leaking onto its
/// channel would tell a UI to re-hide and re-reveal a terminal that never
/// re-attached. It also pins the once-per-ATTACH shape from the other
/// side — the second attach to the same terminal must be marked, not
/// treated as already caught up.
#[tokio::test]
async fn a_takeover_marks_the_replacement_and_never_the_incumbent() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let mut incumbent = MarkerPeer::connect(&h.sup, 1).await;
    incumbent
        .attach(&session.id, TerminalSelector::Agent, "first-client")
        .await;
    incumbent.wait_for_marker(30).await;
    incumbent.wait_for_text("FAKE-AGENT READY", 30).await;

    let mut replacement = MarkerPeer::connect(&h.sup, 1).await;
    replacement
        .attach(&session.id, TerminalSelector::Agent, "second-client")
        .await;
    replacement.wait_for_marker(30).await;
    replacement.input(b"AFTER-TAKEOVER\r").await;
    replacement
        .wait_for_text(&agent_echo_of("AFTER-TAKEOVER"), 30)
        .await;

    // Both sides are drained before either is counted. The incumbent's
    // window starts AFTER the replacement already has its own marker and
    // live output in hand, so a marker leaking onto the incumbent's
    // channel has had its chance to arrive; the replacement is drained
    // too, since it stopped reading at its live text and a second marker
    // behind that frame is exactly what its count must catch. (Windows,
    // not the streams' ends: both connections stay open, so there is no
    // end to read to.)
    incumbent.drain_for(Duration::from_secs(2)).await;
    replacement.drain_for(SETTLE_WINDOW).await;
    assert!(
        incumbent.detached.is_some(),
        "test premise: the second attach must have taken the first one over"
    );
    assert_eq!(
        incumbent.markers(),
        1,
        "the incumbent must see only its OWN marker; transcript:\n{}",
        incumbent.transcript()
    );
    assert_eq!(
        replacement.markers(),
        1,
        "the replacement's attach must be marked exactly once; transcript:\n{}",
        replacement.transcript()
    );
}

/// A takeover landing while the incumbent is still catching up ends that
/// catch-up with a `Detached` — with or without a marker — while the
/// replacement is still marked exactly once.
///
/// The semantics being pinned (see `ControlMsg::ReplayComplete`'s docs): a
/// successful attach is NOT promised a marker. `Attached` and the marker
/// are separated by the whole replay, and an attachment torn down inside
/// that window never reaches it. Consumers must therefore treat `Detached`
/// as ending the catch-up phase too; a presentation that waited for the
/// marker alone would hide a terminal forever on exactly this race.
///
/// The incumbent's marker count is asserted as "at most one" rather than
/// "none" deliberately. Which side of the race lands first is genuinely
/// timing-dependent — forcing the no-marker outcome would need a seam the
/// supervisor does not have, since the replay would have to be held
/// mid-flight — so "none" would be a flake generator, while tolerating
/// either outcome is exactly what the contract says.
#[tokio::test]
async fn a_takeover_during_the_catch_up_ends_it_with_or_without_a_marker() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    seed_deep_scrollback(&h.sup, &session.id, 4000).await;

    let mut incumbent = MarkerPeer::connect(&h.sup, 1).await;
    incumbent
        .attach(&session.id, TerminalSelector::Agent, "first-client")
        .await;
    // No wait for the incumbent's marker: the takeover is issued the
    // instant its attach is acknowledged, which is the earliest a client
    // can act. Whether that lands inside the incumbent's replay is a race
    // rather than a fact — see this test's docs for why it is allowed to
    // be, and what is asserted instead.
    let mut replacement = MarkerPeer::connect(&h.sup, 1).await;
    replacement
        .attach(&session.id, TerminalSelector::Agent, "second-client")
        .await;
    replacement.wait_for_marker(60).await;

    incumbent
        .wait_until(60, "the incumbent's Detached", |peer| {
            peer.detached.is_some()
        })
        .await;
    incumbent.drain_for(Duration::from_secs(2)).await;
    // The replacement stopped reading at its first marker, so a second one
    // just behind that frame would be unread and uncounted — the same
    // settle every other exactly-once count here performs.
    replacement.drain_for(SETTLE_WINDOW).await;
    assert!(
        incumbent.markers() <= 1,
        "the incumbent must never see more than its own marker; transcript:\n{}",
        incumbent.transcript()
    );
    assert_eq!(
        replacement.markers(),
        1,
        "the replacement's attach must be marked exactly once; transcript:\n{}",
        replacement.transcript()
    );
}

/// M2.5's flow-control catch-up replays history into a live attachment
/// and must stay MARKERLESS.
///
/// This is the one deliberate narrowing in the contract (PLAN_M5.md item
/// 2): the marker bounds an ATTACH's catch-up, not every catch-up. A
/// marker here would tell a consumer that a fresh catch-up phase had just
/// ended when none began — and, in the UI M5 builds on it, would re-hide a
/// terminal the user is actively watching. The pause is forced rather than
/// waited for, because whether tmux's delay-driven pause fires depends on
/// how far it read ahead (see `force_tmux_pause`), and a test that waits
/// for one is asserting a race.
///
/// The recovery is followed all the way through — reset, replayed history,
/// and live output advancing past everything that replay contained —
/// before the count is asserted. Stopping at the reset would assert only
/// that no marker accompanied the START of a catch-up, leaving the far
/// more plausible regression (a marker where the recovery's replay ENDS,
/// exactly as the attach path emits one) undetected.
#[tokio::test]
async fn a_tmux_pause_catch_up_replays_without_a_marker() {
    let h = harness().await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    // `counter` runs until its session is killed, so live output is still
    // arriving when the pause is forced — the shape a real `%pause` takes.
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
    let pane = pane_id_of(
        &h.state.path().join("tmux.sock"),
        &format!("fh-{}", session.id),
    )
    .await;

    let mut peer = MarkerPeer::connect(&h.sup, 1).await;
    peer.attach(&session.id, TerminalSelector::Agent, "viewer")
        .await;
    peer.wait_for_marker(30).await;
    // Enough history that the catch-up has something real to replay.
    peer.wait_for_text("CUTOVER-00000200", 60).await;

    force_tmux_pause(&h, &pane).await;

    // The terminal reset is sent only on the pause-recovery replay path,
    // so it is what proves the catch-up ran at all.
    peer.wait_for_text("\x1bc", 60).await;
    // Then let the replay itself land, so "after the recovery" is measured
    // from its tail rather than from its start.
    peer.drain_for(Duration::from_secs(3)).await;
    let replayed = counter_records(peer.text(..).as_bytes())
        .last()
        .copied()
        .expect("the catch-up must have replayed counter records");
    // Live output resuming past that tail is what makes this window cover
    // the recovery's END, which is where a mistakenly emitted marker would
    // appear.
    peer.wait_for_text(&format!("CUTOVER-{:08}", replayed + 50), 60)
        .await;

    assert_eq!(
        peer.markers(),
        1,
        "the pause catch-up emitted a marker; only the ATTACH's own replay may be marked. \
         transcript:\n{}",
        peer.transcript()
    );
}
