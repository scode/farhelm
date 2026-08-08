//! Attachment uploads driven at the raw protocol level, including the
//! malformed shapes no well-behaved client would produce but the wire
//! format must still handle.

use crate::harness::*;

use crate::terminal_backpressure::flood_session;
use farhelm_supervisor::service::{ArchiveGate, ArchiveStage};

// ---------------------------------------------------------------------
// Attachment uploads (PLAN_M4.md item 4)
//
// Driven at the raw protocol level: the helm's streaming relay is a later
// step, so there is no client API to go through yet — and several of these
// scenarios are shapes no well-behaved client would ever produce anyway
// (an oversized chunk, a commit for a channel carrying no upload, a stream
// that simply stops mid-transfer), which is exactly what has to be pinned.
// ---------------------------------------------------------------------

/// One connection to a supervisor, spoken as frames rather than through
/// `SupervisorClient`.
///
/// Owns both halves of an in-process duplex pipe, like `connect_client`,
/// so a test can send anything the protocol can express and observe every
/// frame that comes back — including the unsolicited `UploadAck` and
/// `UploadAborted` events, which correlate by channel and which a
/// request/reply client API would have no place to surface.
struct RawPeer {
    reader: FrameReader<tokio::io::ReadHalf<tokio::io::DuplexStream>>,
    writer: FrameWriter<tokio::io::WriteHalf<tokio::io::DuplexStream>>,
}

/// A filesystem that takes its time at one named stage.
///
/// The only way to reach the supervisor's own time bounds and
/// cancellation windows from a test: a real disk answers in
/// microseconds, and what needs exercising is what happens when one does
/// not — a write that outlives its bound, a publication that holds the
/// session's lifecycle claim, an ack that must not be sent before its
/// bytes are safely written.
struct SlowFs {
    stage: &'static str,
    delay: Duration,
}

impl SlowFs {
    fn seam(
        stage: &'static str,
        delay: Duration,
    ) -> Arc<dyn farhelm_supervisor::files::FaultSeam + Send + Sync> {
        Arc::new(SlowFs { stage, delay })
    }

    fn pause(&self, stage: &'static str) {
        if self.stage == stage {
            // A blocking sleep on purpose: these run inside
            // `spawn_blocking`, which is exactly where a real filesystem
            // would block.
            std::thread::sleep(self.delay);
        }
    }
}

impl farhelm_supervisor::files::FaultSeam for SlowFs {
    fn write(&self, file: &mut std::fs::File, bytes: &[u8]) -> io::Result<()> {
        self.pause("write");
        farhelm_supervisor::files::RealFs.write(file, bytes)
    }
    fn fsync_file(&self, file: &std::fs::File) -> io::Result<()> {
        self.pause("fsync_file");
        farhelm_supervisor::files::RealFs.fsync_file(file)
    }
    fn link(&self, from: &std::path::Path, to: &std::path::Path) -> io::Result<()> {
        self.pause("link");
        farhelm_supervisor::files::RealFs.link(from, to)
    }
    fn remove_temp(&self, path: &std::path::Path) -> io::Result<()> {
        self.pause("remove_temp");
        farhelm_supervisor::files::RealFs.remove_temp(path)
    }
}

/// A filesystem that fails one named stage outright.
struct FailingFs {
    stage: &'static str,
    /// How many calls to that stage succeed before the failures start —
    /// 0 fails immediately, 1 lets the first one through (the shape a
    /// disk filling up mid-transfer takes).
    after: usize,
    seen: std::sync::atomic::AtomicUsize,
}

impl FailingFs {
    fn seam(
        stage: &'static str,
        after: usize,
    ) -> Arc<dyn farhelm_supervisor::files::FaultSeam + Send + Sync> {
        Arc::new(FailingFs {
            stage,
            after,
            seen: std::sync::atomic::AtomicUsize::new(0),
        })
    }

    fn should_fail(&self, stage: &'static str) -> bool {
        self.stage == stage && self.seen.fetch_add(1, Ordering::SeqCst) >= self.after
    }
}

impl farhelm_supervisor::files::FaultSeam for FailingFs {
    fn write(&self, file: &mut std::fs::File, bytes: &[u8]) -> io::Result<()> {
        if self.should_fail("write") {
            return Err(io::Error::other("no space left on device (injected)"));
        }
        farhelm_supervisor::files::RealFs.write(file, bytes)
    }
    fn fsync_file(&self, file: &std::fs::File) -> io::Result<()> {
        if self.should_fail("fsync_file") {
            return Err(io::Error::other("the disk gave up at fsync (injected)"));
        }
        farhelm_supervisor::files::RealFs.fsync_file(file)
    }
    fn link(&self, from: &std::path::Path, to: &std::path::Path) -> io::Result<()> {
        if self.should_fail("link") {
            return Err(io::Error::other("the disk gave up at publish (injected)"));
        }
        farhelm_supervisor::files::RealFs.link(from, to)
    }
}

/// A harness whose supervisor uses `seam` for every upload's file
/// operations, with the upload timeouts shortened to what a test can
/// afford to wait out.
async fn upload_harness(
    seam: Arc<dyn farhelm_supervisor::files::FaultSeam + Send + Sync>,
    timeouts: SupervisorTimeouts,
) -> Harness {
    harness_with_seams(
        timeouts,
        SupervisorSeams {
            upload_fs: seam,
            ..SupervisorSeams::default()
        },
    )
    .await
}

impl RawPeer {
    /// Connect and complete the hello, leaving the peer ready to send.
    async fn connect(sup: &Arc<Supervisor>) -> RawPeer {
        RawPeer::connect_with_buffer(sup, 1 << 20).await
    }

    /// [`RawPeer::connect`] with an explicit transport buffer.
    ///
    /// A small buffer is how a test observes the supervisor's own
    /// queueing: with a megabyte of slack, everything the writer produces
    /// disappears into the pipe and nothing about its ORDERING is
    /// visible.
    async fn connect_with_buffer(sup: &Arc<Supervisor>, bytes: usize) -> RawPeer {
        let (client_side, server_side) = tokio::io::duplex(bytes);
        let sup = Arc::clone(sup);
        tokio::spawn(async move {
            let _ = handle_connection(sup, server_side).await;
        });
        let (read_half, write_half) = tokio::io::split(client_side);
        let mut peer = RawPeer {
            reader: FrameReader::new(read_half),
            writer: FrameWriter::new(write_half),
        };
        handshake(&mut peer.reader, &mut peer.writer, "helm")
            .await
            .expect("handshake");
        peer
    }

    async fn control(&mut self, msg: &ControlMsg) {
        self.writer.write_control(msg).await.expect("write control");
    }

    /// Send one upload chunk as a data frame on `channel`.
    async fn chunk(&mut self, channel: u32, bytes: Vec<u8>) {
        self.writer
            .write_frame(&Frame::data(channel, bytes))
            .await
            .expect("write chunk");
    }

    /// The next control message, failing the test rather than hanging if
    /// none arrives.
    async fn next_control(&mut self, secs: u64) -> ControlMsg {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            let frame = tokio::time::timeout(remaining, self.reader.read_frame())
                .await
                .expect("timed out waiting for a control message")
                .expect("read frame")
                .expect("connection closed while waiting for a control message");
            if frame.kind == FrameKind::Control {
                return parse_control(&frame).expect("parse control");
            }
        }
    }

    /// The next control message that is not an `UploadAck`.
    ///
    /// Acks are progress events, not outcomes: they arrive per chunk and
    /// interleave with everything else, so a test asserting on a
    /// transfer's OUTCOME has to look past them. Their own contract
    /// (monotonic, never past the declared size) is checked where it is
    /// the subject, not everywhere they appear.
    async fn next_outcome(&mut self, secs: u64) -> ControlMsg {
        loop {
            match self.next_control(secs).await {
                ControlMsg::UploadAck { .. } => continue,
                other => return other,
            }
        }
    }

    /// Send a `BeginUpload` and return whatever answers it.
    async fn begin(
        &mut self,
        req_id: u64,
        session_id: &str,
        channel: u32,
        filename: &str,
        size: u64,
    ) -> ControlMsg {
        self.control(&ControlMsg::BeginUpload {
            req_id,
            session_id: session_id.to_string(),
            channel,
            filename: filename.to_string(),
            size,
        })
        .await;
        self.next_outcome(20).await
    }

    /// The whole happy-path sequence — begin, chunks, commit — returning
    /// the transfer's final outcome (`UploadCommitted`, or whatever
    /// refused it).
    async fn upload(
        &mut self,
        session_id: &str,
        channel: u32,
        filename: &str,
        content: &[u8],
    ) -> ControlMsg {
        let started = self
            .begin(1, session_id, channel, filename, content.len() as u64)
            .await;
        assert!(
            matches!(started, ControlMsg::UploadStarted { .. }),
            "begin must be accepted, got: {started:?}"
        );
        for piece in content.chunks(UPLOAD_CHUNK_BYTES) {
            self.chunk(channel, piece.to_vec()).await;
        }
        self.control(&ControlMsg::CommitUpload { req_id: 2, channel })
            .await;
        self.next_outcome(20).await
    }
}

/// The sorted contents of one directory, or an empty vector when the
/// directory does not exist.
///
/// Only `NotFound` is treated as empty. Any OTHER read failure — a
/// permission error, a path that is unexpectedly a file — is a bug in the
/// test or in what it is testing, and silently reporting it as "nothing
/// here" would turn an assertion about cleanup into one that passes
/// because it could not look.
fn dir_names(dir: &std::path::Path) -> Vec<String> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Vec::new(),
        Err(e) => panic!("reading {}: {e}", dir.display()),
    };
    let mut names: Vec<String> = entries
        .map(|entry| {
            entry
                .expect("dir entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        // The reserved staging directory is not an attachment; it is
        // where in-flight ones live (see `staging_names`).
        .filter(|name| name != ".staging")
        .collect();
    names.sort();
    names
}

/// The PUBLISHED attachments of a session, sorted.
fn attachment_names(state: &std::path::Path, session_id: &str) -> Vec<String> {
    dir_names(&farhelm_supervisor::attachments::session_dir(
        state, session_id,
    ))
}

/// What is currently staged (in flight, or left behind) for a session.
fn staging_names(state: &std::path::Path, session_id: &str) -> Vec<String> {
    dir_names(&farhelm_supervisor::attachments::staging_dir(
        state, session_id,
    ))
}

/// Poll until a session's published attachments are exactly `expected`
/// AND nothing is left staged, failing the test if it never happens.
///
/// Both halves in one helper because every ending of a transfer owes
/// both: an aborted upload must publish nothing and leave no staging
/// file, and checking only the first would pass against an implementation
/// that quietly accumulates debris. Polled because cleanup happens in the
/// transfer's own task, so it is observable only after the fact — a
/// single read would race it.
async fn wait_for_attachments(
    state: &std::path::Path,
    session_id: &str,
    expected: &[&str],
    secs: u64,
) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    loop {
        let published = attachment_names(state, session_id);
        let staged = staging_names(state, session_id);
        if published == expected && staged.is_empty() {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "attachments never settled to {expected:?} with nothing staged; last saw \
             published {published:?}, staged {staged:?}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// The happy path end to end: bytes streamed over the protocol land under
/// the session's own attachments directory, with the exact content, and
/// the commit reports the real host-side path.
///
/// The path is the PRODUCT here — the client inserts it into a terminal —
/// so this pins the reply's path against the file that actually exists
/// rather than against a formatting rule, and pins the location against
/// SPEC.md's "a per-session attachments directory under the supervisor's
/// own data area, never in the working directory".
#[tokio::test]
async fn an_upload_lands_in_the_sessions_attachments_directory_at_the_reported_path() {
    use std::os::unix::fs::PermissionsExt;

    let h = harness().await;
    let (session, work) = basic_session(&h).await;
    let mut peer = RawPeer::connect(&h.sup).await;

    let content = b"\x89PNG\r\n\x1a\n not really a png, but bytes are bytes".to_vec();
    let outcome = peer
        .upload(&session.id, 1, "screenshot.png", &content)
        .await;

    let ControlMsg::UploadCommitted { path, req_id } = outcome else {
        panic!("upload must commit, got: {outcome:?}");
    };
    assert_eq!(req_id, 2, "the commit's reply must correlate to the commit");
    assert_eq!(
        std::path::Path::new(&path),
        farhelm_supervisor::attachments::session_dir(h.state.path(), &session.id)
            .join("screenshot.png"),
        "the published path must be the session's own attachments directory"
    );
    assert_eq!(std::fs::read(&path).expect("published file"), content);
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(
        mode, 0o600,
        "a published attachment must be 0600, got {mode:o}"
    );
    assert_eq!(
        std::fs::read_dir(work.path()).unwrap().count(),
        0,
        "nothing may be written into the session's working directory"
    );
}

/// A transfer that sends FEWER bytes than it declared publishes nothing,
/// and says so as a correlated error at its commit.
///
/// The declaration is the only integrity check this path has: the bytes
/// ride a stream with no length framing of their own, so a truncated
/// transfer that published anyway would hand the agent a silently
/// half-written file.
#[tokio::test]
async fn an_upload_shorter_than_it_declared_publishes_nothing() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let mut peer = RawPeer::connect(&h.sup).await;

    let started = peer.begin(1, &session.id, 1, "notes.txt", 64).await;
    assert!(matches!(started, ControlMsg::UploadStarted { .. }));
    peer.chunk(1, vec![b'x'; 8]).await;
    peer.control(&ControlMsg::CommitUpload {
        req_id: 2,
        channel: 1,
    })
    .await;

    let outcome = peer.next_outcome(20).await;
    let ControlMsg::Error {
        req_id,
        message,
        kind,
    } = outcome
    else {
        panic!("a short upload must be refused, got: {outcome:?}");
    };
    assert_eq!(req_id, 2);
    assert_eq!(
        kind,
        ErrorKind::InvalidRequest,
        "a size mismatch is the caller's error, not a server fault"
    );
    assert!(
        message.contains("64") && message.contains('8'),
        "the refusal must name both counts, got: {message}"
    );
    wait_for_attachments(h.state.path(), &session.id, &[], 10).await;
}

/// A sender that goes PAST its declaration is stopped at the chunk that
/// would exceed it — before those bytes are written — rather than at the
/// commit.
///
/// Two things ride on rejecting early rather than late. The acks stay
/// truthful: `received` is what was actually written, so it can never be
/// a capped number that claims less than the file on disk holds. And the
/// disk stays bounded: a sender that keeps going past its declaration
/// would otherwise grow a staging file that the commit's exact size check
/// guarantees can never publish — unbounded bytes for a file with no
/// possible future.
#[tokio::test]
async fn an_upload_past_its_declared_size_is_aborted_before_the_bytes_land() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let mut peer = RawPeer::connect(&h.sup).await;

    let started = peer.begin(1, &session.id, 1, "notes.txt", 8).await;
    assert!(matches!(started, ControlMsg::UploadStarted { .. }));
    peer.chunk(1, vec![b'x'; 8]).await;
    // Everything declared has arrived and been acked; this one is past it.
    peer.chunk(1, vec![b'x'; 56]).await;

    let outcome = peer.next_outcome(20).await;
    let ControlMsg::UploadAborted { channel, reason } = outcome else {
        panic!("an over-long sender must be aborted, got: {outcome:?}");
    };
    assert_eq!(channel, 1);
    assert!(
        reason.contains('8'),
        "the reason must name the declaration that was exceeded, got: {reason}"
    );
    wait_for_attachments(h.state.path(), &session.id, &[], 10).await;
}

/// Two uploads of the same filename both publish, under distinct paths —
/// including two in flight at the same instant, which is the case a
/// check-then-create implementation loses.
///
/// SPEC.md never promises a name, only that the file lands and its path
/// comes back; what it cannot tolerate is one upload silently replacing
/// another's file. The concurrent half is the real test: both transfers
/// stage their temp files before either commits, so both resolve the same
/// free name and the publication itself is what has to keep them apart.
#[tokio::test]
async fn two_uploads_of_one_name_both_publish_under_distinct_paths() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let mut first = RawPeer::connect(&h.sup).await;
    let mut second = RawPeer::connect(&h.sup).await;
    for (peer, size) in [(&mut first, 5u64), (&mut second, 6)] {
        let started = peer.begin(1, &session.id, 1, "shot.png", size).await;
        assert!(matches!(started, ControlMsg::UploadStarted { .. }));
    }
    first.chunk(1, b"first".to_vec()).await;
    second.chunk(1, b"second".to_vec()).await;

    // Both commits in flight together: neither transfer knows the other
    // exists, and both proposed the same name.
    let (first_outcome, second_outcome) = tokio::join!(
        async {
            first
                .control(&ControlMsg::CommitUpload {
                    req_id: 2,
                    channel: 1,
                })
                .await;
            first.next_outcome(20).await
        },
        async {
            second
                .control(&ControlMsg::CommitUpload {
                    req_id: 2,
                    channel: 1,
                })
                .await;
            second.next_outcome(20).await
        }
    );

    let mut paths = Vec::new();
    for outcome in [first_outcome, second_outcome] {
        let ControlMsg::UploadCommitted { path, .. } = outcome else {
            panic!("both concurrent uploads must publish, got: {outcome:?}");
        };
        paths.push(path);
    }
    assert_ne!(
        paths[0], paths[1],
        "two concurrent uploads published under the SAME path — one silently replaced the other"
    );
    let contents: Vec<Vec<u8>> = paths
        .iter()
        .map(|path| std::fs::read(path).expect("published file"))
        .collect();
    assert!(
        contents.contains(&b"first".to_vec()) && contents.contains(&b"second".to_vec()),
        "each upload's own bytes must survive, got: {contents:?}"
    );
    assert_eq!(
        attachment_names(h.state.path(), &session.id),
        ["shot-1.png", "shot.png"],
        "the collision must resolve with a numeric suffix on the stem"
    );
    assert!(
        staging_names(h.state.path(), &session.id).is_empty(),
        "both publications must leave their staging directory empty"
    );
}

/// A shell-hostile filename publishes under a sanitized name, and a
/// proposal with no usable name at all publishes under a generated one —
/// never a refusal.
///
/// Both halves are contract, not convenience. The path is inserted as
/// terminal input, so a name a shell would split or expand breaks exactly
/// the flow attachments exist for; and SPEC.md rejects only directories,
/// never a file for what it is called, so "no usable name" must still
/// produce a file.
#[tokio::test]
async fn hostile_and_empty_filenames_publish_under_safe_generated_names() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let mut peer = RawPeer::connect(&h.sup).await;

    let outcome = peer
        .upload(
            &session.id,
            1,
            "../../my shot; rm -rf $HOME.png",
            b"hostile",
        )
        .await;
    let ControlMsg::UploadCommitted { path, .. } = outcome else {
        panic!("a hostile filename must still publish, got: {outcome:?}");
    };
    let name = std::path::Path::new(&path)
        .file_name()
        .expect("a published path has a filename")
        .to_string_lossy()
        .into_owned();
    assert_eq!(
        name, "my_shot__rm_-rf__HOME.png",
        "the published name must be shell-safe and carry no path structure"
    );
    assert_eq!(std::fs::read(&path).expect("published file"), b"hostile");

    let mut peer = RawPeer::connect(&h.sup).await;
    let outcome = peer.upload(&session.id, 1, "", b"nameless").await;
    let ControlMsg::UploadCommitted { path, .. } = outcome else {
        panic!("an empty filename must not be refused, got: {outcome:?}");
    };
    assert!(
        std::path::Path::new(&path)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("attachment-"),
        "a nameless upload must publish under a generated name, got: {path}"
    );
    assert_eq!(std::fs::read(&path).expect("published file"), b"nameless");
}

/// An abandoned transfer leaves nothing behind — whether the client says
/// so or simply disappears.
///
/// The two paths are one mechanism on purpose (dropping the channel's
/// route is what ends the transfer either way), and the failure they
/// prevent is a state directory that accumulates a half-written temp file
/// for every cancelled drop and every browser tab that closed mid-paste.
#[tokio::test]
async fn an_abandoned_transfer_leaves_no_temp_file_behind() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    // Half 1: the client aborts explicitly.
    let mut peer = RawPeer::connect(&h.sup).await;
    let started = peer.begin(1, &session.id, 1, "shot.png", 4096).await;
    assert!(matches!(started, ControlMsg::UploadStarted { .. }));
    peer.chunk(1, vec![b'x'; 128]).await;
    peer.control(&ControlMsg::AbortUpload { channel: 1 }).await;
    wait_for_attachments(h.state.path(), &session.id, &[], 10).await;

    // Half 2: the connection simply dies mid-transfer.
    let mut peer = RawPeer::connect(&h.sup).await;
    let started = peer.begin(1, &session.id, 1, "shot.png", 4096).await;
    assert!(matches!(started, ControlMsg::UploadStarted { .. }));
    peer.chunk(1, vec![b'x'; 128]).await;
    drop(peer);
    wait_for_attachments(h.state.path(), &session.id, &[], 10).await;
}

/// A transfer that stops making progress is given up on, with the reason
/// clients render verbatim, and its temp file cleaned.
///
/// This is the forever-pending upload SPEC.md's health-check requirement
/// exists to prevent: the connection is perfectly healthy, so nothing
/// else in the system would ever notice. Without it the paste never
/// resolves, the temp file never goes away, and the user is told nothing
/// at all.
#[tokio::test]
async fn a_transfer_that_stops_progressing_is_aborted_as_stalled() {
    let h = harness_with_timeouts(SupervisorTimeouts {
        upload_progress: Duration::from_millis(300),
        ..SupervisorTimeouts::default()
    })
    .await;
    let (session, _work) = basic_session(&h).await;
    let mut peer = RawPeer::connect(&h.sup).await;

    let started = peer.begin(1, &session.id, 1, "shot.png", 1 << 20).await;
    assert!(matches!(started, ControlMsg::UploadStarted { .. }));
    peer.chunk(1, vec![b'x'; 64]).await;
    // ...and then nothing, with the connection still wide open.

    let outcome = peer.next_outcome(20).await;
    let ControlMsg::UploadAborted { channel, reason } = outcome else {
        panic!("a stalled transfer must be aborted, got: {outcome:?}");
    };
    assert_eq!(channel, 1, "the abort correlates by channel, not by req_id");
    assert_eq!(
        reason, UPLOAD_ABORT_REASON_STALLED,
        "the reason must be the shared constant both emitters and their tests match on"
    );
    wait_for_attachments(h.state.path(), &session.id, &[], 10).await;
}

/// An oversized chunk is rejected the channel-correlated way, and takes
/// the whole transfer with it.
///
/// A data frame has no `req_id` to hang an `Error` on, so an uncorrelated
/// one could be claimed by any of a client's concurrent transfers — which
/// is why `UPLOAD_CHUNK_BYTES`'s own contract names `UploadAborted` as the
/// answer. Failing the transfer rather than dropping the chunk is the
/// other half: a file missing the middle of itself must never publish.
#[tokio::test]
async fn an_oversized_chunk_aborts_the_transfer() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let mut peer = RawPeer::connect(&h.sup).await;

    let started = peer
        .begin(
            1,
            &session.id,
            1,
            "shot.png",
            (UPLOAD_CHUNK_BYTES + 1) as u64,
        )
        .await;
    assert!(matches!(started, ControlMsg::UploadStarted { .. }));
    peer.chunk(1, vec![b'x'; UPLOAD_CHUNK_BYTES + 1]).await;

    let outcome = peer.next_outcome(20).await;
    let ControlMsg::UploadAborted { channel, reason } = outcome else {
        panic!("an oversized chunk must abort the transfer, got: {outcome:?}");
    };
    assert_eq!(channel, 1);
    assert!(
        reason.contains(&UPLOAD_CHUNK_BYTES.to_string()),
        "the reason must name the limit that was exceeded, got: {reason}"
    );
    wait_for_attachments(h.state.path(), &session.id, &[], 10).await;
}

/// A storage failure mid-stream aborts the transfer visibly and publishes
/// nothing — SPEC_impl.md's no-cap policy from the other side: a full disk
/// is a FAILED upload, never a truncated file at the published path.
///
/// Injected through the write seam because a genuine ENOSPC around a temp
/// directory is neither portable nor deterministic, and because what needs
/// exercising is the supervisor's reaction rather than the kernel's.
#[tokio::test]
async fn a_storage_failure_mid_stream_aborts_the_transfer_and_publishes_nothing() {
    use farhelm_supervisor::files::{FaultSeam, RealFs};

    /// A filesystem whose second write fails, leaving the first one's
    /// bytes really on disk — the shape a disk filling up mid-transfer
    /// takes.
    struct FailSecondWrite {
        writes: std::sync::atomic::AtomicUsize,
    }

    impl FaultSeam for FailSecondWrite {
        fn write(&self, file: &mut std::fs::File, bytes: &[u8]) -> io::Result<()> {
            if self.writes.fetch_add(1, Ordering::SeqCst) >= 1 {
                return Err(io::Error::other("no space left on device (injected)"));
            }
            RealFs.write(file, bytes)
        }
    }

    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            upload_fs: Arc::new(FailSecondWrite {
                writes: std::sync::atomic::AtomicUsize::new(0),
            }),
            ..SupervisorSeams::default()
        },
    )
    .await;
    let (session, _work) = basic_session(&h).await;
    let mut peer = RawPeer::connect(&h.sup).await;

    let started = peer.begin(1, &session.id, 1, "big.bin", 256).await;
    assert!(matches!(started, ControlMsg::UploadStarted { .. }));
    peer.chunk(1, vec![b'x'; 128]).await;
    peer.chunk(1, vec![b'x'; 128]).await;

    let outcome = peer.next_outcome(20).await;
    let ControlMsg::UploadAborted { channel, reason } = outcome else {
        panic!("a storage failure must abort the transfer, got: {outcome:?}");
    };
    assert_eq!(channel, 1);
    assert!(
        reason.contains("no space left on device"),
        "the reason must carry what actually went wrong, got: {reason}"
    );
    wait_for_attachments(h.state.path(), &session.id, &[], 10).await;
}

/// Deleting a session removes its attachments directory outright —
/// SPEC.md's "attachment files are removed when their session is
/// deleted", including the directory itself.
#[tokio::test]
async fn deleting_a_session_removes_its_attachments_directory() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let mut peer = RawPeer::connect(&h.sup).await;

    let outcome = peer.upload(&session.id, 1, "shot.png", b"bytes").await;
    assert!(matches!(outcome, ControlMsg::UploadCommitted { .. }));
    let dir = farhelm_supervisor::attachments::session_dir(h.state.path(), &session.id);
    assert!(dir.exists(), "the upload must have created the directory");

    h.client
        .delete_session(&session.id)
        .await
        .expect("delete a session with attachments");
    assert!(
        !dir.exists(),
        "delete must remove the attachments directory, not just its contents"
    );
}

/// A delete racing an in-flight upload leaves no file and no directory —
/// whether it lands while the transfer is streaming or exactly at its
/// commit.
///
/// This is the ordering PLAN_M4.md item 4 calls delete serialization, and
/// the two ways to get it wrong are opposite: abort the transfers too
/// late and one is still writing while the directory is removed, or let a
/// commit publish into a directory a delete has already emptied — which
/// would leave a file (and a recreated directory) behind for a session
/// that no longer exists, with the client told its paste succeeded.
///
/// Both winners are acceptable outcomes; what is not is a survivor on
/// disk or a commit that claims success without one.
#[tokio::test]
async fn a_delete_racing_an_upload_leaves_no_file_and_no_directory() {
    for commit_first in [false, true] {
        let h = harness().await;
        let (session, _work) = basic_session(&h).await;
        let mut peer = RawPeer::connect(&h.sup).await;

        let started = peer.begin(1, &session.id, 1, "shot.png", 5).await;
        assert!(matches!(started, ControlMsg::UploadStarted { .. }));
        peer.chunk(1, b"bytes".to_vec()).await;
        if commit_first {
            peer.control(&ControlMsg::CommitUpload {
                req_id: 2,
                channel: 1,
            })
            .await;
        }

        h.client
            .delete_session(&session.id)
            .await
            .expect("a delete must not be defeated by an upload in flight");

        if !commit_first {
            // Streaming when the delete landed: the transfer is torn down
            // on its channel first, and only then does the client commit.
            let aborted = peer.next_outcome(20).await;
            assert!(
                matches!(aborted, ControlMsg::UploadAborted { channel: 1, .. }),
                "a delete must abort the transfers it is erasing, got: {aborted:?}"
            );
            peer.control(&ControlMsg::CommitUpload {
                req_id: 2,
                channel: 1,
            })
            .await;
        }
        // The commit's own answer, which may arrive after an
        // `UploadAborted` for the same channel (a delete that catches a
        // commit already queued tells the client both: the channel is
        // dead, and this request failed).
        let mut answered = false;
        for _ in 0..3 {
            match peer.next_outcome(20).await {
                // The commit won the race outright: its file was
                // published and then removed with the session, which is
                // the honest outcome for a delete that arrived second.
                ControlMsg::UploadCommitted { path, .. } => {
                    assert!(
                        commit_first,
                        "a transfer aborted by a delete must never publish, got: {path}"
                    );
                    answered = true;
                    break;
                }
                ControlMsg::Error {
                    req_id: 2, message, ..
                } => {
                    assert!(
                        message.contains("deleted")
                            || message.contains("no upload")
                            || message.contains("aborted"),
                        "a commit that lost to a delete must say so, got: {message}"
                    );
                    answered = true;
                    break;
                }
                ControlMsg::UploadAborted { channel: 1, .. } => continue,
                other => panic!("unexpected outcome: {other:?}"),
            }
        }
        assert!(
            answered,
            "commit_first={commit_first}: the client's commit was never answered"
        );
        assert!(
            !farhelm_supervisor::attachments::session_dir(h.state.path(), &session.id).exists(),
            "commit_first={commit_first}: nothing may recreate a deleted session's attachments \
             directory"
        );
        // The quarantine is where a delete parks a session's attachments
        // between detaching them and removing them; a delete that ran to
        // completion leaves it empty. Anything here would be a file the
        // user's deleted session still owns on disk.
        let quarantine =
            farhelm_supervisor::attachments::attachments_root(h.state.path()).join(".quarantine");
        assert!(
            dir_names(&quarantine).is_empty(),
            "commit_first={commit_first}: a completed delete must leave nothing quarantined, \
             found {:?}",
            dir_names(&quarantine)
        );
    }
}

/// Archive cancels and joins a stalled transfer without calling the
/// retained session deleted or allowing a partial file to publish.
#[tokio::test]
async fn archive_cancels_a_stalled_upload_truthfully_and_cleans_its_stage() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let mut peer = RawPeer::connect(&h.sup).await;
    let started = peer.begin(1, &session.id, 1, "partial.txt", 10).await;
    assert!(matches!(started, ControlMsg::UploadStarted { .. }));
    peer.chunk(1, b"half".to_vec()).await;

    h.client
        .archive_session(&session.id)
        .await
        .expect("archive joins the transfer it cancels");
    let outcome = peer.next_outcome(20).await;
    let ControlMsg::UploadAborted { channel, reason } = outcome else {
        panic!("archive must abort the stalled upload, got: {outcome:?}");
    };
    assert_eq!(channel, 1);
    assert!(reason.contains("archived"), "truthful reason: {reason}");
    assert!(
        !reason.contains("deleted"),
        "archive is not deletion: {reason}"
    );

    let session_dir = farhelm_supervisor::attachments::session_dir(h.state.path(), &session.id);
    let staging = farhelm_supervisor::attachments::staging_dir(h.state.path(), &session.id);
    assert!(
        session_dir.exists(),
        "archive retains the attachment directory"
    );
    assert!(
        dir_names(&staging).is_empty(),
        "the partial stage must be removed"
    );
    assert!(
        !session_dir.join("partial.txt").exists(),
        "a cancelled partial upload must never publish"
    );
}

/// A read-only archive preflight failure leaves an in-flight upload alone.
///
/// The archive reply says nothing was archived at these boundaries. That
/// must cover the upload too: cancelling it before a pane, tab, scope, or
/// sweep check fails would make the refusal itself a destructive result.
#[tokio::test]
async fn archive_preflight_failures_do_not_discard_uploads() {
    for stage in [
        ArchiveStage::PaneProbe,
        ArchiveStage::TabRediscovery,
        ArchiveStage::ScopeEnumeration,
        ArchiveStage::Sweep,
    ] {
        let gate: ArchiveGate = Arc::new(move |reached| {
            Box::pin(async move {
                if reached == stage {
                    anyhow::bail!("injected archive preflight failure at {stage:?}");
                }
                Ok(())
            })
        });
        let h = harness_with_seams(
            SupervisorTimeouts::default(),
            SupervisorSeams {
                archive_gate: Some(gate),
                ..SupervisorSeams::default()
            },
        )
        .await;
        let (session, _work) = basic_session(&h).await;
        let mut peer = RawPeer::connect(&h.sup).await;
        let started = peer.begin(1, &session.id, 1, "survives.txt", 5).await;
        assert!(matches!(started, ControlMsg::UploadStarted { .. }));
        peer.chunk(1, b"alive".to_vec()).await;

        h.client
            .archive_session(&session.id)
            .await
            .expect_err("the injected preflight must refuse archive");
        peer.control(&ControlMsg::CommitUpload {
            req_id: 2,
            channel: 1,
        })
        .await;
        let outcome = peer.next_outcome(20).await;
        assert!(
            matches!(outcome, ControlMsg::UploadCommitted { req_id: 2, .. }),
            "{stage:?} must leave the upload usable, got {outcome:?}"
        );
        wait_for_attachments(h.state.path(), &session.id, &["survives.txt"], 10).await;
    }
}

/// A begin admitted after archive's transfer drain cannot start once the
/// lifecycle claim is released.
///
/// Archive retains the row, so existence alone is not a sufficient staging
/// check. The transfer must re-read `archived` under the same claim archive
/// held through publication, otherwise it recreates writable attachment
/// state behind the completed archive.
#[tokio::test]
async fn an_upload_waiting_behind_archive_is_refused_after_publication() {
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let gate_entered = Arc::clone(&entered);
    let gate_release = Arc::clone(&release);
    let gate: ArchiveGate = Arc::new(move |stage| {
        let entered = Arc::clone(&gate_entered);
        let release = Arc::clone(&gate_release);
        Box::pin(async move {
            if stage == ArchiveStage::ArtifactRemoval {
                entered.notify_one();
                release.notified().await;
            }
            Ok(())
        })
    });
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            archive_gate: Some(gate),
            ..SupervisorSeams::default()
        },
    )
    .await;
    let (session, _work) = basic_session(&h).await;
    let archiver = connect_client(&h.sup).await;
    let archive_id = session.id.clone();
    let archive = tokio::spawn(async move { archiver.archive_session(&archive_id).await });
    tokio::time::timeout(Duration::from_secs(5), entered.notified())
        .await
        .expect("archive reached the post-drain artifact boundary");

    let mut peer = RawPeer::connect(&h.sup).await;
    peer.control(&ControlMsg::BeginUpload {
        req_id: 1,
        session_id: session.id.clone(),
        channel: 1,
        filename: "too-late.txt".to_string(),
        size: 1,
    })
    .await;
    tokio::task::yield_now().await;
    release.notify_waiters();
    archive
        .await
        .expect("archive task")
        .expect("archive completes");

    let outcome = peer.next_outcome(20).await;
    let ControlMsg::Error { kind, message, .. } = outcome else {
        panic!("the post-drain begin must be refused, got {outcome:?}");
    };
    assert_eq!(kind, ErrorKind::InvalidRequest);
    assert!(
        message.contains("archived") && message.contains("restart"),
        "the refusal must name the retained lifecycle state: {message}"
    );
    wait_for_attachments(h.state.path(), &session.id, &[], 10).await;
}

/// Every `BeginUpload` refusal, and the property they share: nothing is
/// created and the connection carries on.
///
/// Channel 0 is the control channel, a channel already carrying an upload
/// (or an attachment) cannot also carry another, and the receiver's
/// admission bound is what keeps a client from opening transfers without
/// end — each one costing a staged temp file, a task, and a credit
/// window's worth of queue.
#[tokio::test]
async fn begin_upload_refuses_a_bad_channel_and_an_over_full_connection() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let mut peer = RawPeer::connect(&h.sup).await;

    let refused = peer.begin(1, &session.id, 0, "shot.png", 4).await;
    assert!(
        matches!(
            refused,
            ControlMsg::Error {
                req_id: 1,
                kind: ErrorKind::InvalidRequest,
                ..
            }
        ),
        "channel 0 is reserved, got: {refused:?}"
    );

    let unknown = peer.begin(2, "no-such-session", 1, "shot.png", 4).await;
    assert!(
        matches!(
            unknown,
            ControlMsg::Error {
                req_id: 2,
                kind: ErrorKind::NotFound,
                ..
            }
        ),
        "an unknown session is a not-found, got: {unknown:?}"
    );

    // One live transfer, then the same channel again.
    let started = peer.begin(3, &session.id, 1, "shot.png", 4096).await;
    assert!(matches!(started, ControlMsg::UploadStarted { .. }));
    let reused = peer.begin(4, &session.id, 1, "shot.png", 4).await;
    assert!(
        matches!(
            reused,
            ControlMsg::Error {
                req_id: 4,
                kind: ErrorKind::InvalidRequest,
                ..
            }
        ),
        "a channel already carrying an upload must be refused, got: {reused:?}"
    );

    // Fill the connection's admission bound (one transfer is already
    // open), then ask for one more.
    for channel in 2..=8 {
        let started = peer
            .begin(
                10 + u64::from(channel),
                &session.id,
                channel,
                "shot.png",
                4096,
            )
            .await;
        assert!(
            matches!(started, ControlMsg::UploadStarted { .. }),
            "transfer {channel} must be admitted, got: {started:?}"
        );
    }
    let over_cap = peer.begin(99, &session.id, 9, "shot.png", 4).await;
    let ControlMsg::Error {
        req_id: 99,
        message,
        kind,
    } = over_cap
    else {
        panic!("the ninth concurrent transfer must be refused, got: {over_cap:?}");
    };
    assert_eq!(kind, ErrorKind::InvalidRequest);
    assert!(
        message.contains("in flight"),
        "the refusal must say what the limit is about, got: {message}"
    );

    // The connection is still usable for ordinary work afterwards.
    peer.control(&ControlMsg::ListSessions {
        req_id: 100,
        cursor: None,
        limit: None,
    })
    .await;
    assert!(matches!(
        peer.next_outcome(20).await,
        ControlMsg::SessionList { req_id: 100, .. }
    ));
}

/// Startup reconciliation removes a crashed transfer's staging file and
/// keeps every published attachment — including one whose NAME looks
/// exactly like staging debris.
///
/// In-process cleanup covers every path a live supervisor can take, so
/// this is specifically about the one it cannot: a `kill -9` between
/// staging and finishing. The two published files are the other half of
/// the test, and the `report.tmp-backup` decoy is the one that matters:
/// under a sweep that recognized debris by NAME rather than by LOCATION,
/// a user's perfectly ordinary filename would be deleted on every
/// restart. That is data loss, and no other test in this suite would see
/// it.
#[tokio::test]
async fn startup_reconciliation_sweeps_staging_and_keeps_published_attachments() {
    let _slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = tempfile::tempdir().expect("tempdir");
    let sup = Supervisor::new_with_exe(state.path(), farhelm_bin().into())
        .await
        .expect("supervisor");
    let _tmux = TmuxServerGuard(state.path().join("tmux.sock"));

    // A session the supervisor knows nothing about: its whole directory
    // is debris a delete never finished removing, and reconciliation must
    // take it (the sessions it DOES know are covered by the live
    // upload tests, which all end with their published files intact).
    let deleted = farhelm_supervisor::attachments::session_dir(state.path(), "deleted-session");
    std::fs::create_dir_all(&deleted).expect("attachments dir");
    std::fs::write(deleted.join("gone.png"), b"orphan").expect("plant orphan session file");

    // ...and a session that does exist, with debris of its own.
    let (session, _work) = {
        let client = connect_client(&sup).await;
        let work = tempfile::tempdir().expect("workdir");
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
        (session, work)
    };
    farhelm_supervisor::attachments::ensure_session_dirs(state.path(), &session.id)
        .await
        .expect("attachment dirs");
    let dir = farhelm_supervisor::attachments::session_dir(state.path(), &session.id);
    let staged = farhelm_supervisor::attachments::staging_dir(state.path(), &session.id)
        .join(".shot.png.tmp-0123456789");
    let published = dir.join("shot.png");
    let decoy = dir.join("report.tmp-backup");
    std::fs::write(&staged, b"half an upload").expect("plant staging debris");
    std::fs::write(&published, b"a real attachment").expect("plant published");
    std::fs::write(&decoy, b"also a real attachment").expect("plant decoy");

    let serving = Arc::clone(&sup);
    tokio::spawn(async move {
        let _ = serving.serve().await;
    });
    // Readiness is proved by a REQUEST completing, not by the socket
    // file: `serve` binds before it reconciles, so a connect (or a mere
    // `exists`) can succeed while the sweep has not run. Only the accept
    // loop — which starts after reconciliation — can answer this.
    wait_for_supervisor_ready(state.path()).await;
    let stream = farhelm_supervisor::service::connect(state.path())
        .await
        .expect("connect to the serving supervisor");
    let (r, w) = tokio::io::split(stream);
    let client = SupervisorClient::start(r, w).await.expect("handshake");
    client.list_sessions().await.expect("a served request");

    assert!(
        !staged.exists(),
        "startup must sweep an interrupted transfer's staging file"
    );
    assert!(
        !deleted.exists(),
        "startup must remove the attachments of a session that no longer exists"
    );
    assert_eq!(
        std::fs::read(&published).expect("published file"),
        b"a real attachment",
        "startup must never touch a published attachment"
    );
    assert_eq!(
        std::fs::read(&decoy).expect("decoy file"),
        b"also a real attachment",
        "a published file whose NAME resembles staging debris must survive a restart"
    );
}

/// `UploadStarted` echoes the request it answers and the channel it
/// accepted — both taken from the request rather than defaulted.
///
/// A reply that hard-coded either would work perfectly for a client that
/// uses channel 1 and one request at a time, and would misroute every
/// concurrent paste for one that does not. Non-default values on both is
/// the only way to tell the two apart.
#[tokio::test]
async fn upload_started_echoes_the_request_id_and_channel_it_was_given() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let mut peer = RawPeer::connect(&h.sup).await;

    let started = peer.begin(41, &session.id, 9, "shot.png", 4).await;
    assert!(
        matches!(
            started,
            ControlMsg::UploadStarted {
                req_id: 41,
                channel: 9
            }
        ),
        "got: {started:?}"
    );
}

/// An empty upload publishes an empty file — it does not become a
/// refusal, a no-op, or a file with content.
///
/// Zero is a legal declared size and empty files are legal attachments
/// (a user dropping one is not an error), so every count in this path
/// has to be right at the boundary: the commit's exact size check must
/// accept 0 == 0, and publication must happen for a stream nothing was
/// ever written to.
#[tokio::test]
async fn a_zero_byte_upload_publishes_an_empty_file() {
    use std::os::unix::fs::PermissionsExt;

    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let mut peer = RawPeer::connect(&h.sup).await;

    let outcome = peer.upload(&session.id, 1, "empty.txt", b"").await;
    let ControlMsg::UploadCommitted { path, .. } = outcome else {
        panic!("an empty upload must publish, got: {outcome:?}");
    };
    let published = std::fs::metadata(&path).expect("published file");
    assert_eq!(published.len(), 0);
    assert_eq!(published.permissions().mode() & 0o777, 0o600);
    assert!(
        std::path::Path::new(&path).is_absolute(),
        "the reported path must be absolute, got: {path}"
    );
}

/// The reported path is ABSOLUTE even when the supervisor was started
/// with a relative state directory.
///
/// `UploadCommitted::path` promises the raw absolute host-side path, and
/// the client's whole use for it is to paste it into a terminal whose
/// working directory is the SESSION's — not the supervisor's. A relative
/// path would resolve against the wrong directory, or nothing at all.
#[tokio::test]
async fn a_relative_state_directory_still_reports_an_absolute_path() {
    let _slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = tempfile::tempdir().expect("tempdir");
    // A relative path to the same directory, which is what
    // `--state-dir ./state` produces.
    let relative = pathdiff_to_current(state.path());
    let sup = Supervisor::new_with_exe(&relative, farhelm_bin().into())
        .await
        .expect("supervisor on a relative state dir");
    let _tmux = TmuxServerGuard(state.path().join("tmux.sock"));
    let client = connect_client(&sup).await;
    let work = tempfile::tempdir().expect("workdir");
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

    let mut peer = RawPeer::connect(&sup).await;
    let outcome = peer.upload(&session.id, 1, "shot.png", b"bytes").await;
    let ControlMsg::UploadCommitted { path, .. } = outcome else {
        panic!("upload must commit, got: {outcome:?}");
    };
    assert!(
        std::path::Path::new(&path).is_absolute(),
        "a relative state dir must not leak a relative published path, got: {path}"
    );
    assert_eq!(std::fs::read(&path).expect("published file"), b"bytes");
}

/// A path to `dir` relative to the current working directory, for the
/// test above. Falls back to the absolute path when the two share no
/// prefix, which only makes that test weaker, never wrong.
fn pathdiff_to_current(dir: &std::path::Path) -> std::path::PathBuf {
    let cwd = std::env::current_dir().expect("cwd");
    match dir.strip_prefix(&cwd) {
        Ok(rest) => std::path::PathBuf::from("./").join(rest),
        Err(_) => {
            // `/tmp/...` against a checkout elsewhere: walk up to the
            // root and back down, which is still a relative path.
            let ups = cwd.components().count();
            let mut relative = std::path::PathBuf::new();
            for _ in 0..ups.saturating_sub(1) {
                relative.push("..");
            }
            relative.join(dir.strip_prefix("/").unwrap_or(dir))
        }
    }
}

/// Acks are cumulative, monotonic, never past what was sent or declared,
/// and never arrive before the bytes they claim are safely written.
///
/// The last property is the one that needs a slow filesystem to see: an
/// implementation that acked on receipt rather than after the write would
/// satisfy every other assertion here while telling the sender its bytes
/// were safe before they were — and the sender's credit, its progress
/// timeout, and its own idea of what has to be resent all rest on that
/// claim.
#[tokio::test]
async fn acks_are_cumulative_and_never_precede_the_write_they_claim() {
    let h = upload_harness(
        SlowFs::seam("write", Duration::from_millis(250)),
        SupervisorTimeouts::default(),
    )
    .await;
    let (session, _work) = basic_session(&h).await;
    let mut peer = RawPeer::connect(&h.sup).await;

    let started = peer.begin(1, &session.id, 1, "notes.txt", 30).await;
    assert!(matches!(started, ControlMsg::UploadStarted { .. }));

    let mut acked = Vec::new();
    let mut sent_total = 0u64;
    for chunk in [10usize, 10, 10] {
        let before = tokio::time::Instant::now();
        peer.chunk(1, vec![b'x'; chunk]).await;
        sent_total += chunk as u64;
        let ControlMsg::UploadAck { channel, received } = peer.next_control(20).await else {
            panic!("each chunk must be acked");
        };
        assert_eq!(channel, 1);
        assert!(
            before.elapsed() >= Duration::from_millis(200),
            "an ack arrived in {:?}, before its write could have completed",
            before.elapsed()
        );
        assert!(
            received <= sent_total && received <= 30,
            "ack {received} claims more than the {sent_total} sent (or the 30 declared)"
        );
        acked.push(received);
    }
    assert_eq!(
        acked,
        [10, 20, 30],
        "acks must be CUMULATIVE and monotonic, not per-chunk counts"
    );
}

/// An upload's acks are not queued behind a busy terminal's output.
///
/// `UploadAck`'s contract says acks "must not queue behind bulk frames",
/// and the reason is mechanical: the ack is the sender's credit, so an
/// ack stuck behind a megabyte of terminal output stalls a healthy
/// transfer — and stalls it for long enough that the sender's own
/// progress timeout may call the receiver dead.
///
/// Measured as the ack's POSITION in a frozen backlog: where in the
/// queued terminal output it landed, as a fraction of that queue's whole
/// length. Under one shared FIFO the ack can only come out after
/// essentially all of the backlog that was already queued ahead of it —
/// up to `CONNECTION_WRITER_QUEUE` frames — so an ack arriving inside the
/// first quarter is a fact a FIFO cannot produce. On a prioritized path
/// only what was already in flight precedes it.
///
/// # Why the backlog is frozen, and why the reader waits
///
/// The measurement used to be a latency race: the flood kept producing
/// while the reader drained, so "bytes before the ack" was a contest
/// between the producer and the reader and the number moved with machine
/// load — which made a fixed byte threshold either loose enough to prove
/// nothing or tight enough to fail on a busy CI box. Pausing the
/// attachment first (`PauseOutput`, which parks the forwarder) turns the
/// queue into a fixed object, and the question into a bimodal one about
/// ordering rather than a continuous one about speed.
///
/// "Frozen" is very nearly exact rather than exact, and the residual is
/// worth naming: the forwarder parks at the TOP of its pump loop, so one
/// that was already blocked inside a `send` on the full writer queue when
/// the pause arrived completes THAT send as soon as draining frees a slot,
/// and only then parks. The denominator is therefore the queue as it stood
/// at the pause plus at most one more frame — a bound, not a moving
/// target, and one frame against `CONNECTION_WRITER_QUEUE` of them does
/// not move the fraction this asserts on.
///
/// Freezing the queue is necessary but not sufficient: reading is
/// unthrottled, so a reader that starts draining while an upload round
/// trip is still in flight empties the whole backlog in well under the
/// millisecond that round trip costs, and the ack then trivially comes
/// out last no matter how it is queued. So the reader deliberately does
/// nothing while each upload message is being produced, and only then
/// drains. What that leaves ahead of a prioritized ack is just the bytes
/// already handed to the transport, which is why the transport buffer is
/// deliberately tiny — smaller than one round of the queue being measured,
/// so the supervisor's queue rather than the pipe dominates both the
/// numerator and the denominator.
///
/// The unread soak has its own premise: the flood must already be LIVE.
/// Attach replay cannot establish that — even an empty pane emits replay
/// data for modes and cursor position — so the test reads through
/// `ReplayComplete` and waits for one later Data frame first. That readiness
/// frame is deliberately excluded from `terminal_bytes`; the denominator and
/// its 32 KiB floor describe only bytes left unread during the soak and then
/// frozen by `PauseOutput`.
///
/// # The residual race, and its margin
///
/// Those two settles are a time bound, not an observation, and there is no
/// honest way to make them one: nothing the supervisor already sends says
/// "the ack is now enqueued", and any in-band signal this test could wait
/// for (`UploadCommitted` rides the same priority queue as the ack) can
/// only be observed by CONSUMING the measured stream — which perturbs the
/// very queue positions under measurement. Adding a product seam to observe it
/// would be a worse trade — a seam that exists only for this test, on the
/// upload path.
///
/// So the settle is sized honestly instead. What has to fit inside it is
/// one local round trip: the supervisor reads a frame off a duplex pipe,
/// writes 16 bytes to a staging file through `spawn_blocking`, and hands
/// one small frame to the priority queue. That is sub-millisecond
/// unloaded, and the 5s budget is roughly three orders of magnitude of
/// headroom on it. The failure mode if it is ever exceeded is a clean,
/// legible one — the ack simply reports as arriving at the end of the
/// backlog — rather than a silent weakening.
#[tokio::test]
async fn an_ack_arrives_ahead_of_a_backlog_of_terminal_output() {
    // How long the reader stays silent while the supervisor produces each
    // of the two upload messages. See this test's docs for the margin
    // math: it covers a sub-millisecond local round trip.
    const SETTLE: Duration = Duration::from_secs(5);

    let h = harness().await;
    let (session, _work) = flood_session(&h).await;
    let mut peer = RawPeer::connect_with_buffer(&h.sup, 1024).await;

    // Attach the flooding terminal on channel 1 and prove that the LIVE
    // producer is flowing before treating an unread interval as a backlog
    // soak. Without that evidence a scheduler-starved flood can spend the
    // whole soak producing nothing, only to report the broken premise at the
    // final backlog-size assertion after the rest of this test has run.
    peer.control(&ControlMsg::Attach {
        req_id: 1,
        session_id: session.id.clone(),
        channel: 1,
        cols: 80,
        rows: 24,
        terminal: TerminalSelector::default(),
        lease: "flooding-lease".to_string(),
        if_unowned: false,
    })
    .await;
    let readiness_deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    let mut replay_complete = false;
    loop {
        let remaining = readiness_deadline.saturating_duration_since(tokio::time::Instant::now());
        let frame = tokio::time::timeout(remaining, peer.reader.read_frame())
            .await
            .expect("the flood never produced live output")
            .expect("read frame")
            .expect("connection stayed open");
        match frame.kind {
            FrameKind::Data if replay_complete => break,
            FrameKind::Control => {
                if matches!(
                    parse_control(&frame).expect("parse control"),
                    ControlMsg::ReplayComplete { channel: 1 }
                ) {
                    replay_complete = true;
                }
            }
            FrameKind::Data => {}
        }
    }
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Freeze what piled up. From here the terminal queue only shrinks
    // (modulo the one in-flight frame this test's docs account for), so
    // its total length is a real denominator rather than a moving target.
    // The stall detach that a held pause eventually triggers
    // (`STALL_DETACH_TIMEOUT`, a minute) is far away from the ~15s this
    // whole sequence spends paused.
    peer.control(&ControlMsg::PauseOutput { channel: 1 }).await;

    // Now an upload on its own channel, and then a settle with NO reading
    // at all, so the supervisor has finished producing `UploadStarted`
    // before a single queued byte is consumed. See this test's own docs
    // for why the settle is a duration rather than an observation, and for
    // the margin `SETTLE` carries over the round trip it has to cover.
    peer.control(&ControlMsg::BeginUpload {
        req_id: 2,
        session_id: session.id.clone(),
        channel: 2,
        filename: "shot.png".to_string(),
        size: 16,
    })
    .await;
    tokio::time::sleep(SETTLE).await;

    // One reader for all three phases (await the start, then the ack, then
    // the rest of the backlog), because every phase has to keep counting
    // the same terminal bytes and a second reader would lose the running
    // total. The chunk and its settle are sent from inside it for the same
    // reason: the count must not restart across them.
    //
    // `UploadStarted` is awaited before the chunk goes out — the same
    // order the sibling ack test uses — because a chunk sent before the
    // supervisor has accepted the transfer is answered by an abort rather
    // than the ack this test is trying to position.
    let mut terminal_bytes = 0usize;
    let mut started = false;
    let mut acked_at: Option<usize> = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the backlog never drained: {terminal_bytes} bytes read, started={started}, \
             ack at {acked_at:?}"
        );
        // With the forwarder parked, a quiet stretch means the queue is
        // genuinely empty rather than merely slow — which is what makes
        // "the whole backlog" a knowable quantity at all.
        let quiet = tokio::time::timeout(Duration::from_secs(3), peer.reader.read_frame()).await;
        let Ok(frame) = quiet else {
            assert!(
                started && acked_at.is_some(),
                "the queue went quiet before the upload was answered: {terminal_bytes} bytes \
                 read, started={started}, ack at {acked_at:?}"
            );
            break;
        };
        let frame = frame.expect("read frame").expect("connection stayed open");
        match frame.kind {
            FrameKind::Data => terminal_bytes += frame.body.len(),
            FrameKind::Control => match parse_control(&frame).expect("parse control") {
                ControlMsg::UploadStarted { .. } => {
                    started = true;
                    peer.chunk(2, vec![b'x'; 16]).await;
                    // Same reason as the settle above: let the ack be
                    // produced while the backlog is still whole, so what
                    // the drain then reports is where the ack sits in that
                    // backlog rather than how fast this loop can read.
                    tokio::time::sleep(SETTLE).await;
                }
                ControlMsg::UploadAck { channel: 2, .. } => {
                    assert!(started, "an ack arrived for a transfer never started");
                    acked_at = Some(terminal_bytes);
                }
                _ => {}
            },
        }
    }

    let acked_at = acked_at.expect("the chunk must be acked");
    // Both numbers are dominated by `CONNECTION_WRITER_QUEUE` frames of
    // real terminal output; this floor only catches the degenerate case
    // where the flood never got going and the ratio below would be
    // measuring nothing.
    assert!(
        terminal_bytes >= 32 * 1024,
        "test premise: the soak must leave a real backlog for the ack to be positioned \
         within, but only {terminal_bytes} bytes were queued"
    );
    assert!(
        acked_at * 4 <= terminal_bytes,
        "the ack arrived after {acked_at} of {terminal_bytes} queued terminal bytes — past \
         the first quarter, which is where sharing the bulk queue would put it"
    );
}

/// A write that outlives its bound fails the transfer rather than
/// hanging it, and publishes nothing.
///
/// The progress timeout cannot see this case: it runs BETWEEN commands,
/// so a filesystem that never answers leaves the transfer neither
/// progressing nor timing out — and, at commit time, holding the
/// session's lifecycle claim while it does. The disk-stage bound is what
/// turns "wedged forever" into "failed visibly".
#[tokio::test]
async fn a_write_that_outlives_its_bound_fails_the_transfer() {
    let h = upload_harness(
        SlowFs::seam("write", Duration::from_secs(5)),
        SupervisorTimeouts {
            upload_disk_stage: Duration::from_millis(200),
            ..SupervisorTimeouts::default()
        },
    )
    .await;
    let (session, _work) = basic_session(&h).await;
    let mut peer = RawPeer::connect(&h.sup).await;

    let started = peer.begin(1, &session.id, 1, "shot.png", 8).await;
    assert!(matches!(started, ControlMsg::UploadStarted { .. }));
    peer.chunk(1, vec![b'x'; 8]).await;

    let outcome = peer.next_outcome(20).await;
    let ControlMsg::UploadAborted { channel, reason } = outcome else {
        panic!("a wedged write must abort the transfer, got: {outcome:?}");
    };
    assert_eq!(channel, 1);
    assert!(
        reason.contains("did not answer"),
        "the reason must say the filesystem stopped answering, got: {reason}"
    );
    // The abandoned write finishes on its own later; its stream's `Drop`
    // is what removes the staging file, so this is a wait rather than an
    // instantaneous check.
    wait_for_attachments(h.state.path(), &session.id, &[], 30).await;
}

/// A publication that outlives its bound fails the commit AND releases
/// the session, which is the half that matters.
///
/// Publication runs under the session's lifecycle claim, so an unbounded
/// hold would make one stuck disk enough to render a session
/// unmanageable — its stop, restart, and delete all queue behind a
/// transfer nobody can finish. The delete at the end is the assertion:
/// it has to complete promptly, not after the wedged filesystem
/// eventually answers.
#[tokio::test]
async fn a_publication_that_outlives_its_bound_frees_the_session() {
    let h = upload_harness(
        SlowFs::seam("link", Duration::from_secs(10)),
        SupervisorTimeouts {
            upload_disk_stage: Duration::from_millis(300),
            ..SupervisorTimeouts::default()
        },
    )
    .await;
    let (session, _work) = basic_session(&h).await;
    let mut peer = RawPeer::connect(&h.sup).await;

    let outcome = peer.upload(&session.id, 1, "shot.png", b"bytes").await;
    let ControlMsg::Error { req_id, kind, .. } = outcome else {
        panic!("a wedged publication must fail the commit, got: {outcome:?}");
    };
    assert_eq!(req_id, 2);
    assert_eq!(kind, ErrorKind::Internal);

    let started = tokio::time::Instant::now();
    h.client
        .delete_session(&session.id)
        .await
        .expect("a session must stay manageable through a wedged publication");
    assert!(
        started.elapsed() < Duration::from_secs(8),
        "the delete waited {:?} — the publication is holding the session's lifecycle claim",
        started.elapsed()
    );
}

/// A commit-time filesystem fault is a CORRELATED error, with no
/// `UploadAborted` and no debris.
///
/// The distinction is `CommitUpload`'s own contract: failures at commit
/// have a `req_id` waiting on them, so they are `Error`s; only
/// post-start failures with nothing to correlate against become channel
/// events. A client that received both would have to guess which one its
/// paste failed under.
#[tokio::test]
async fn a_commit_time_filesystem_fault_is_a_correlated_error_with_no_debris() {
    let h = upload_harness(
        FailingFs::seam("fsync_file", 0),
        SupervisorTimeouts::default(),
    )
    .await;
    let (session, _work) = basic_session(&h).await;
    let mut peer = RawPeer::connect(&h.sup).await;

    let outcome = peer.upload(&session.id, 1, "shot.png", b"bytes").await;
    let ControlMsg::Error {
        req_id,
        message,
        kind,
    } = outcome
    else {
        panic!("a failing publish must fail the commit, got: {outcome:?}");
    };
    assert_eq!(req_id, 2, "the failure must answer the commit");
    assert_eq!(kind, ErrorKind::Internal);
    assert!(
        message.contains("gave up at fsync"),
        "the error must carry what actually failed, got: {message}"
    );

    // Nothing else may arrive on the channel: an `UploadAborted` here
    // would be a second, uncorrelated report of the same failure.
    let extra = tokio::time::timeout(Duration::from_millis(500), peer.reader.read_frame()).await;
    assert!(
        extra.is_err(),
        "a commit failure must not also emit a channel event: {extra:?}"
    );
    wait_for_attachments(h.state.path(), &session.id, &[], 10).await;
}

/// A commit already queued behind a chunk that fails is still answered.
///
/// A client that sends its bytes and its commit back to back has a
/// `req_id` outstanding when the write fails. Answering only the channel
/// (with `UploadAborted`) would leave that request pending forever, so
/// the paste would hang rather than fail — the one outcome SPEC.md's
/// "upload failures must be visible" rules out.
#[tokio::test]
async fn a_commit_queued_behind_a_failing_chunk_is_still_answered() {
    let h = upload_harness(FailingFs::seam("write", 1), SupervisorTimeouts::default()).await;
    let (session, _work) = basic_session(&h).await;
    let mut peer = RawPeer::connect(&h.sup).await;

    let started = peer.begin(1, &session.id, 1, "shot.png", 16).await;
    assert!(matches!(started, ControlMsg::UploadStarted { .. }));
    peer.chunk(1, vec![b'x'; 8]).await;
    peer.chunk(1, vec![b'x'; 8]).await;
    peer.control(&ControlMsg::CommitUpload {
        req_id: 2,
        channel: 1,
    })
    .await;

    let mut aborted = false;
    let mut answered = false;
    for _ in 0..4 {
        match peer.next_control(20).await {
            ControlMsg::UploadAborted { channel: 1, .. } => aborted = true,
            ControlMsg::Error { req_id: 2, .. } => {
                answered = true;
                break;
            }
            ControlMsg::UploadAck { .. } => continue,
            other => panic!("unexpected message: {other:?}"),
        }
    }
    assert!(aborted, "the channel must be told its transfer died");
    assert!(answered, "the queued commit must still be answered");
    wait_for_attachments(h.state.path(), &session.id, &[], 10).await;
}

/// An abort stops a transfer that still has chunks queued, rather than
/// letting it write them all first.
///
/// The queue can legitimately hold a credit window of data, so a
/// cancellation delivered THROUGH it would be served only after every
/// buffered byte had been written to a file nobody will publish — on a
/// slow disk, seconds of pointless writes after the user cancelled, and
/// on a session delete, writes into a directory that is being taken
/// away. Measured by counting the acks that keep arriving after the
/// abort: each one is a chunk that was written anyway.
#[tokio::test]
async fn an_abort_stops_a_transfer_with_chunks_still_queued() {
    let h = upload_harness(
        SlowFs::seam("write", Duration::from_millis(200)),
        SupervisorTimeouts::default(),
    )
    .await;
    let (session, _work) = basic_session(&h).await;
    let mut peer = RawPeer::connect(&h.sup).await;

    let chunks = 12usize;
    let started = peer
        .begin(1, &session.id, 1, "shot.png", (chunks * 8) as u64)
        .await;
    assert!(matches!(started, ControlMsg::UploadStarted { .. }));
    for _ in 0..chunks {
        peer.chunk(1, vec![b'x'; 8]).await;
    }
    peer.control(&ControlMsg::AbortUpload { channel: 1 }).await;

    // Whatever was mid-write when the abort landed may still be acked;
    // everything queued behind it must not be.
    let mut acks_after_abort = 0;
    let deadline = tokio::time::Instant::now() + Duration::from_millis(1500);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(300), peer.reader.read_frame()).await {
            Ok(Ok(Some(frame))) if frame.kind == FrameKind::Control => {
                if matches!(
                    parse_control(&frame).expect("parse control"),
                    ControlMsg::UploadAck { .. }
                ) {
                    acks_after_abort += 1;
                }
            }
            Ok(_) => break,
            Err(_) => continue,
        }
    }
    assert!(
        acks_after_abort <= 3,
        "{acks_after_abort} chunks were written after the abort — the cancellation is queued \
         behind the data instead of overtaking it"
    );
    wait_for_attachments(h.state.path(), &session.id, &[], 10).await;
}

/// A channel and its admission slot belong to a transfer until that
/// transfer has genuinely finished — a commit does not free either one
/// early.
///
/// Both halves are how a client could otherwise be misled. A slot freed
/// at commit lets a pipelining client hold unbounded transfers open
/// (each still writing, each still holding a staging file) while the
/// admission bound reads as satisfied. And a channel freed at commit lets
/// the NEXT transfer be started on a number the previous one is still
/// enqueueing events for, so an ack or an abort for a finished upload
/// lands on a live one.
#[tokio::test]
async fn a_commit_frees_neither_the_channel_nor_the_slot_until_the_transfer_ends() {
    let h = upload_harness(
        SlowFs::seam("link", Duration::from_millis(800)),
        SupervisorTimeouts::default(),
    )
    .await;
    let (session, _work) = basic_session(&h).await;
    let mut peer = RawPeer::connect(&h.sup).await;

    let started = peer.begin(1, &session.id, 1, "shot.png", 4).await;
    assert!(matches!(started, ControlMsg::UploadStarted { .. }));
    peer.chunk(1, b"abcd".to_vec()).await;
    peer.control(&ControlMsg::CommitUpload {
        req_id: 2,
        channel: 1,
    })
    .await;

    // The publication is still running: the channel is not reusable...
    let reused = peer.begin(3, &session.id, 1, "other.png", 4).await;
    assert!(
        matches!(reused, ControlMsg::Error { req_id: 3, .. }),
        "a channel whose transfer is still publishing must not be reusable, got: {reused:?}"
    );
    // ...and a second commit is refused rather than queued behind the
    // first, which is what stops a pipelined flood from looking like
    // progress.
    peer.control(&ControlMsg::CommitUpload {
        req_id: 4,
        channel: 1,
    })
    .await;

    let mut committed = false;
    let mut second_commit_refused = false;
    for _ in 0..4 {
        match peer.next_control(20).await {
            ControlMsg::UploadCommitted { req_id: 2, .. } => committed = true,
            ControlMsg::Error {
                req_id: 4, message, ..
            } => {
                assert!(
                    message.contains("already been committed"),
                    "a second commit must say so, got: {message}"
                );
                second_commit_refused = true;
            }
            ControlMsg::UploadAck { .. } => continue,
            other => panic!("unexpected message: {other:?}"),
        }
        if committed && second_commit_refused {
            break;
        }
    }
    assert!(committed && second_commit_refused);

    // Once the transfer has ended, the channel is reusable again — and
    // the new transfer receives only its OWN events.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        match peer.begin(5, &session.id, 1, "again.png", 4).await {
            ControlMsg::UploadStarted { req_id: 5, .. } => break,
            ControlMsg::Error { .. } if tokio::time::Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            other => panic!("a finished transfer's channel must become reusable, got: {other:?}"),
        }
    }
    peer.chunk(1, b"efgh".to_vec()).await;
    let ControlMsg::UploadAck {
        channel: 1,
        received,
    } = peer.next_control(20).await
    else {
        panic!("the new transfer must be the one acked");
    };
    assert_eq!(
        received, 4,
        "the ack must count the NEW transfer's bytes, not the old one's"
    );
}

/// The admission bound is per CONNECTION: one client filling its own
/// does not touch another's.
///
/// A supervisor-wide bound would let one busy client stop every other
/// client's pastes, which is a denial of service by accident. Pinned
/// with two connections against one supervisor, which is exactly the
/// shape two helm-side clients take.
#[tokio::test]
async fn the_upload_admission_bound_is_per_connection() {
    let h = upload_harness(
        SlowFs::seam("write", Duration::from_millis(400)),
        SupervisorTimeouts::default(),
    )
    .await;
    let (session, _work) = basic_session(&h).await;
    let mut busy = RawPeer::connect(&h.sup).await;
    let mut other = RawPeer::connect(&h.sup).await;

    for channel in 1..=8u32 {
        let started = busy
            .begin(u64::from(channel), &session.id, channel, "shot.png", 4096)
            .await;
        assert!(
            matches!(started, ControlMsg::UploadStarted { .. }),
            "transfer {channel} must be admitted, got: {started:?}"
        );
    }
    let over = busy.begin(99, &session.id, 9, "shot.png", 4).await;
    assert!(
        matches!(over, ControlMsg::Error { req_id: 99, .. }),
        "the ninth transfer on one connection must be refused, got: {over:?}"
    );

    let elsewhere = other.begin(1, &session.id, 1, "shot.png", 4).await;
    assert!(
        matches!(elsewhere, ControlMsg::UploadStarted { .. }),
        "another connection must be unaffected by the first's bound, got: {elsewhere:?}"
    );
}

/// A flood of EMPTY chunks does not keep a stalled transfer alive.
///
/// Empty frames are not progress, and a select that served whatever
/// command was ready before checking an expired deadline would serve them
/// forever — a transfer that never advances, never times out, and holds
/// its staging file indefinitely, which is precisely the forever-pending
/// upload the timeout exists to prevent.
#[tokio::test]
async fn an_empty_chunk_flood_does_not_defeat_the_stall_timeout() {
    let h = harness_with_timeouts(SupervisorTimeouts {
        upload_progress: Duration::from_millis(300),
        ..SupervisorTimeouts::default()
    })
    .await;
    let (session, _work) = basic_session(&h).await;
    let mut peer = RawPeer::connect(&h.sup).await;

    let started = peer.begin(1, &session.id, 1, "shot.png", 1 << 20).await;
    assert!(matches!(started, ControlMsg::UploadStarted { .. }));

    let flooding = tokio::spawn(async move {
        // Faster than the progress window, for well past it.
        for _ in 0..60 {
            peer.chunk(1, Vec::new()).await;
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        peer
    });
    let mut peer = flooding.await.expect("the flood task must not panic");

    let outcome = peer.next_outcome(20).await;
    let ControlMsg::UploadAborted { reason, .. } = outcome else {
        panic!("an empty-chunk flood must not keep a transfer alive, got: {outcome:?}");
    };
    assert_eq!(reason, UPLOAD_ABORT_REASON_STALLED);
    wait_for_attachments(h.state.path(), &session.id, &[], 10).await;
}

/// A transfer that keeps making progress is never stalled, however long
/// it takes in total.
///
/// The complement of the test above, and the reason the timeout is
/// written as a PROGRESS bound rather than a duration cap: a large
/// attachment on a slow link legitimately outlives any total-duration
/// bound, and "no size cap" would be a lie if it did not.
#[tokio::test]
async fn a_transfer_that_keeps_progressing_is_never_stalled() {
    let h = harness_with_timeouts(SupervisorTimeouts {
        upload_progress: Duration::from_millis(300),
        ..SupervisorTimeouts::default()
    })
    .await;
    let (session, _work) = basic_session(&h).await;
    let mut peer = RawPeer::connect(&h.sup).await;

    let chunks = 10usize;
    let started = peer
        .begin(1, &session.id, 1, "slow.bin", (chunks * 4) as u64)
        .await;
    assert!(matches!(started, ControlMsg::UploadStarted { .. }));
    // Well past the progress window in total (~1.5s against 300ms), but
    // never a gap that reaches it.
    for _ in 0..chunks {
        peer.chunk(1, b"abcd".to_vec()).await;
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    peer.control(&ControlMsg::CommitUpload {
        req_id: 2,
        channel: 1,
    })
    .await;

    let outcome = peer.next_outcome(20).await;
    assert!(
        matches!(outcome, ControlMsg::UploadCommitted { req_id: 2, .. }),
        "a steadily progressing transfer must not be stalled, got: {outcome:?}"
    );
}

/// A delete cancels EVERY upload of its session and waits for all of
/// them before finishing.
///
/// One transfer is the easy case; the failure this pins is a delete that
/// signals the first and moves on, leaving the second writing into a
/// directory the delete is removing. Both are blocked on a slow
/// filesystem so they are genuinely mid-write when the delete lands.
#[tokio::test]
async fn a_delete_cancels_and_waits_for_every_upload_of_its_session() {
    let h = upload_harness(
        SlowFs::seam("write", Duration::from_millis(300)),
        SupervisorTimeouts::default(),
    )
    .await;
    let (session, _work) = basic_session(&h).await;
    let mut peer = RawPeer::connect(&h.sup).await;

    for channel in 1..=2u32 {
        let started = peer
            .begin(u64::from(channel), &session.id, channel, "shot.png", 4096)
            .await;
        assert!(matches!(started, ControlMsg::UploadStarted { .. }));
        peer.chunk(channel, vec![b'x'; 512]).await;
    }

    h.client
        .delete_session(&session.id)
        .await
        .expect("a delete must not be defeated by uploads in flight");

    let mut aborted = std::collections::HashSet::new();
    while aborted.len() < 2 {
        match peer.next_control(20).await {
            ControlMsg::UploadAborted { channel, reason } => {
                assert!(
                    reason.contains("deleted"),
                    "the abort must say why, got: {reason}"
                );
                aborted.insert(channel);
            }
            ControlMsg::UploadAck { .. } => continue,
            other => panic!("unexpected message: {other:?}"),
        }
    }
    assert_eq!(aborted, std::collections::HashSet::from([1, 2]));
    assert!(
        !farhelm_supervisor::attachments::session_dir(h.state.path(), &session.id).exists(),
        "the delete must have removed the directory both transfers were writing into"
    );
}

/// `AbortUpload` is idempotent and harmless for a channel that never had
/// an upload — `Detach`'s contract, applied to transfers.
///
/// A client tearing down (a cancelled drop, a closed view, a reconnect)
/// must never have to reason about who won a race, so neither a repeat
/// nor a stray abort may produce an error, and neither may disturb the
/// connection.
#[tokio::test]
async fn duplicate_and_unknown_upload_aborts_are_harmless() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let mut peer = RawPeer::connect(&h.sup).await;

    peer.control(&ControlMsg::AbortUpload { channel: 7 }).await;

    let started = peer.begin(1, &session.id, 1, "shot.png", 4096).await;
    assert!(matches!(started, ControlMsg::UploadStarted { .. }));
    peer.chunk(1, vec![b'x'; 8]).await;
    peer.control(&ControlMsg::AbortUpload { channel: 1 }).await;
    peer.control(&ControlMsg::AbortUpload { channel: 1 }).await;
    peer.control(&ControlMsg::AbortUpload { channel: 1 }).await;

    // The connection is still perfectly usable, and nothing was
    // published or left staged.
    peer.control(&ControlMsg::ListSessions {
        req_id: 9,
        cursor: None,
        limit: None,
    })
    .await;
    let mut listed = false;
    for _ in 0..4 {
        match peer.next_control(20).await {
            ControlMsg::SessionList { req_id: 9, .. } => {
                listed = true;
                break;
            }
            ControlMsg::UploadAck { .. } => continue,
            other => panic!("an abort must produce no reply of its own, got: {other:?}"),
        }
    }
    assert!(listed, "the connection must survive repeated aborts");
    wait_for_attachments(h.state.path(), &session.id, &[], 10).await;
}

/// A begin that cannot stage refuses correlatedly, and its channel is
/// usable again as soon as the storage problem is fixed.
///
/// The refusal has to be about the storage, not about the channel: a
/// begin that failed before creating anything must not leave the channel
/// looking permanently in use, or a client hitting one transient failure
/// would lose that channel number for the life of its connection.
#[tokio::test]
async fn a_begin_that_cannot_stage_refuses_and_leaves_its_channel_usable() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let mut peer = RawPeer::connect(&h.sup).await;

    // A regular file where the session's attachments directory belongs:
    // creating the directory cannot succeed.
    let dir = farhelm_supervisor::attachments::session_dir(h.state.path(), &session.id);
    std::fs::create_dir_all(dir.parent().expect("attachments root")).expect("attachments root");
    std::fs::write(&dir, b"not a directory").expect("plant the blocker");

    let refused = peer.begin(1, &session.id, 1, "shot.png", 4).await;
    let ControlMsg::Error {
        req_id: 1,
        message,
        kind,
    } = refused
    else {
        panic!("an unusable attachments directory must refuse the begin, got: {refused:?}");
    };
    assert_eq!(
        kind,
        ErrorKind::Internal,
        "storage failure is not the caller's fault"
    );
    assert!(
        !message.contains("already in use"),
        "the refusal must be about the storage, not the channel: {message}"
    );

    std::fs::remove_file(&dir).expect("remove the blocker");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        match peer.begin(2, &session.id, 1, "shot.png", 4).await {
            ControlMsg::UploadStarted { req_id: 2, .. } => break,
            ControlMsg::Error { .. } if tokio::time::Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            other => panic!("the channel must become usable again, got: {other:?}"),
        }
    }
}

/// A delete whose attachments cannot be detached FAILS, with the session
/// row retained for a retry.
///
/// This is the fail-closed half of the delete contract: the alternative
/// is reporting a delete that left the user's files on disk with nothing
/// left to find them by. The row surviving is what makes a retry
/// meaningful.
#[tokio::test]
async fn a_delete_fails_closed_when_attachments_cannot_be_detached() {
    use std::os::unix::fs::PermissionsExt;

    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let mut peer = RawPeer::connect(&h.sup).await;
    let outcome = peer.upload(&session.id, 1, "shot.png", b"bytes").await;
    assert!(matches!(outcome, ControlMsg::UploadCommitted { .. }));

    // A read-only attachments root: the rename that detaches this
    // session's directory cannot succeed.
    let root = farhelm_supervisor::attachments::attachments_root(h.state.path());
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o500))
        .expect("make the attachments root read-only");

    let failed = h
        .client
        .delete_session(&session.id)
        .await
        .expect_err("a delete that cannot detach attachments must fail");
    assert!(
        failed.to_string().contains("attachments"),
        "the failure must name what could not be removed, got: {failed:#}"
    );
    let listed = h.client.list_sessions().await.expect("list");
    assert!(
        listed.sessions.iter().any(|s| s.id == session.id),
        "a failed delete must retain the session row for a retry"
    );

    // Restored so the retry — and the harness teardown — can succeed.
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
        .expect("restore the attachments root");
    h.client
        .delete_session(&session.id)
        .await
        .expect("the retry must succeed once the storage problem is fixed");
}

/// The staging file is gone BEFORE the client is told its transfer
/// failed.
///
/// The ordering is what makes a client's obvious reaction — retry the
/// paste — safe: a retry that started while the previous attempt's
/// staging file was still around would be racing this transfer's own
/// debris. Observed by making the removal itself slow: if the notice
/// were sent first, it would arrive well before the removal finished.
#[tokio::test]
async fn the_staging_file_is_gone_before_the_abort_is_announced() {
    let h = upload_harness(
        SlowFs::seam("remove_temp", Duration::from_millis(700)),
        SupervisorTimeouts {
            upload_progress: Duration::from_millis(200),
            ..SupervisorTimeouts::default()
        },
    )
    .await;
    let (session, _work) = basic_session(&h).await;
    let mut peer = RawPeer::connect(&h.sup).await;

    let started = peer.begin(1, &session.id, 1, "shot.png", 1 << 20).await;
    assert!(matches!(started, ControlMsg::UploadStarted { .. }));
    peer.chunk(1, vec![b'x'; 64]).await;

    let outcome = peer.next_outcome(20).await;
    assert!(
        matches!(outcome, ControlMsg::UploadAborted { .. }),
        "the stalled transfer must be aborted, got: {outcome:?}"
    );
    assert!(
        staging_names(h.state.path(), &session.id).is_empty(),
        "the staging file must be gone by the time the client hears about the abort, found {:?}",
        staging_names(h.state.path(), &session.id)
    );
}

/// The transfer trail SPEC.md's logging section requires: begin,
/// publish, and abort events carrying the session, the transfer id, the
/// channel, and byte counts — and never the content.
///
/// Diagnostics are the only way an operator can follow a paste that went
/// wrong across the two hops it takes, so the FIELDS are the contract,
/// not the prose. Captured through a tracing layer rather than asserted
/// by eye, because a field silently dropped in a refactor is exactly the
/// kind of regression nobody notices until they need the log.
#[tokio::test]
async fn the_transfer_trail_carries_identifiers_and_byte_counts() {
    let events = install_diagnostic_capture();
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let mut peer = RawPeer::connect(&h.sup).await;

    let secret = b"a screenshot's bytes, which must never be logged".to_vec();
    let outcome = peer.upload(&session.id, 3, "trail.png", &secret).await;
    assert!(matches!(outcome, ControlMsg::UploadCommitted { .. }));

    // A second transfer that ends the other way, so begin/publish and
    // begin/abort are both covered.
    let started = peer.begin(7, &session.id, 4, "aborted.png", 4096).await;
    assert!(matches!(started, ControlMsg::UploadStarted { .. }));
    peer.chunk(4, vec![b'y'; 8]).await;
    peer.control(&ControlMsg::CommitUpload {
        req_id: 8,
        channel: 4,
    })
    .await;
    let mismatch = peer.next_outcome(20).await;
    assert!(matches!(mismatch, ControlMsg::Error { req_id: 8, .. }));

    let captured = events.lock().expect("diagnostics mutex").clone();
    let mine: Vec<&CapturedEvent> = captured
        .iter()
        .filter(|event| {
            event
                .fields
                .get("session")
                .is_some_and(|s| *s == session.id)
        })
        .collect();

    let started_event = mine
        .iter()
        .find(|event| event.message == "attachment upload started")
        .expect("a begin must be recorded");
    assert!(started_event.fields.contains_key("transfer"));
    assert!(started_event.fields.contains_key("channel"));
    assert_eq!(
        started_event
            .fields
            .get("declared_bytes")
            .map(String::as_str),
        Some(secret.len().to_string()).as_deref()
    );

    let published = mine
        .iter()
        .find(|event| event.message == "attachment upload published")
        .expect("a publish must be recorded");
    assert_eq!(
        published.fields.get("bytes").map(String::as_str),
        Some(secret.len().to_string()).as_deref()
    );
    assert!(published.fields.contains_key("path"));
    assert!(published.fields.contains_key("transfer"));

    let failed = mine
        .iter()
        .find(|event| event.message == "attachment upload failed at commit")
        .expect("a commit failure must be recorded");
    assert_eq!(
        failed.fields.get("received_bytes").map(String::as_str),
        Some("8")
    );
    assert!(failed.fields.contains_key("reason"));

    // Never contents: the bytes themselves must appear in no field of
    // any event.
    let content = String::from_utf8_lossy(&secret).into_owned();
    for event in &captured {
        for (field, value) in &event.fields {
            assert!(
                !value.contains(&content),
                "event {:?} leaked upload content in field {field}",
                event.message
            );
        }
    }
}

/// One captured `tracing` event: its message and its structured fields.
#[derive(Clone, Debug)]
struct CapturedEvent {
    message: String,
    fields: std::collections::HashMap<String, String>,
}

/// Install (once per test process) a layer that records the attachment
/// transfer trail, and hand back the shared buffer it writes into.
///
/// Global because `tracing`'s dispatcher is, and because the supervisor
/// under test runs on the shared runtime rather than on the calling
/// thread — a thread-local subscriber would capture nothing. Only
/// attachment-upload events are retained, so the buffer stays small even
/// though every test in this file shares it, and each test filters by its
/// own session id.
fn install_diagnostic_capture() -> Arc<std::sync::Mutex<Vec<CapturedEvent>>> {
    use tracing_subscriber::layer::SubscriberExt;

    static CAPTURED: std::sync::OnceLock<Arc<std::sync::Mutex<Vec<CapturedEvent>>>> =
        std::sync::OnceLock::new();

    struct Capture(Arc<std::sync::Mutex<Vec<CapturedEvent>>>);

    #[derive(Default)]
    struct Fields(std::collections::HashMap<String, String>);

    impl tracing::field::Visit for Fields {
        fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
            self.0
                .insert(field.name().to_string(), format!("{value:?}"));
        }
        fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
            self.0.insert(field.name().to_string(), value.to_string());
        }
        fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
            self.0.insert(field.name().to_string(), value.to_string());
        }
        fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
            self.0.insert(field.name().to_string(), value.to_string());
        }
    }

    impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Capture {
        fn on_event(
            &self,
            event: &tracing::Event<'_>,
            _ctx: tracing_subscriber::layer::Context<'_, S>,
        ) {
            let mut fields = Fields::default();
            event.record(&mut fields);
            let Some(message) = fields.0.remove("message") else {
                return;
            };
            if !message.starts_with("attachment upload") {
                return;
            }
            self.0
                .lock()
                .expect("diagnostics mutex")
                .push(CapturedEvent {
                    message,
                    fields: fields.0,
                });
        }
    }

    let events = CAPTURED
        .get_or_init(|| Arc::new(std::sync::Mutex::new(Vec::new())))
        .clone();
    // `try_init` rather than `init`: another test may have installed this
    // already, and a second attempt is a no-op rather than a panic.
    let _ = tracing_subscriber::util::SubscriberInitExt::try_init(
        tracing_subscriber::registry().with(Capture(events.clone())),
    );
    events
}

/// A state directory whose path is not valid UTF-8 is refused where it
/// first matters — at session creation — so no attachment can ever be
/// asked to report a path this protocol cannot represent.
///
/// `UploadCommitted::path` is the product of the whole upload path, and
/// the protocol has no representation for a non-UTF-8 one; reporting a
/// lossily-converted path would be worse than failing, because the client
/// would insert a path that merely RESEMBLES the real file. This pins
/// where that is actually decided: creation refuses first (the launch
/// spec has the same constraint), so a session on such a state directory
/// does not exist to upload into. The upload path keeps its own check as
/// defence in depth — see `stage_upload` — and this test is what records
/// that the check is unreachable through a real session rather than
/// merely untested.
#[tokio::test]
async fn a_non_utf8_state_directory_is_refused_before_any_session_can_exist() {
    use std::os::unix::ffi::OsStringExt;

    let _slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let parent = tempfile::tempdir().expect("tempdir");
    // One invalid byte is enough, and keeps the rest of the path (which
    // tmux and SQLite also have to live with) ordinary.
    let mut raw = parent.path().as_os_str().to_os_string().into_vec();
    raw.extend_from_slice(b"/state-\xff");
    let state = std::path::PathBuf::from(std::ffi::OsString::from_vec(raw));
    std::fs::create_dir(&state).expect("non-UTF-8 state dir");

    let sup = Supervisor::new_with_exe(&state, farhelm_bin().into())
        .await
        .expect("a supervisor on a non-UTF-8 state dir");
    let _tmux = TmuxServerGuard(state.join("tmux.sock"));
    let client = connect_client(&sup).await;
    let work = tempfile::tempdir().expect("workdir");
    let refused = client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script basic"),
            None,
            80,
            24,
        )
        .await
        .expect_err("a non-UTF-8 state dir must refuse creation rather than launder its paths");
    assert!(
        refused.to_string().contains("UTF-8"),
        "the refusal must say what is wrong, got: {refused:#}"
    );
    assert!(
        attachment_names(&state, "any-session").is_empty(),
        "nothing may have been created for a session that does not exist"
    );
}
