//! File reads through the same multiplexed connection as session operations.
//! The fixture creates a real session and private files; no live installation
//! or ambient environment is used to choose what the supervisor reads.

use crate::harness::*;
use farhelm_proto::DownloadFileStatus;

/// A file larger than the initial window proves acknowledgements route both
/// ways. The exact body and resolved path prove reads use the session cwd;
/// folders and vanished paths must be refused rather than produce empty files.
#[farhelm_testtrace::test]
async fn stat_and_credit_paced_download_use_the_session_host() {
    let h = harness().await;
    let (session, work) = basic_session(&h).await;
    let path = work.path().join("result.bin");
    let contents = vec![0x73; farhelm_proto::UPLOAD_WINDOW_BYTES as usize + 12345];
    tokio::fs::write(&path, &contents).await.unwrap();
    assert_eq!(tokio::fs::read(&path).await.unwrap(), contents);
    let info = h.client.stat_file(&session.id, "result.bin").await.unwrap();
    assert_eq!(info.path, path.to_str().unwrap());
    assert_eq!(info.status, DownloadFileStatus::Ready);
    assert_eq!(info.size, Some(contents.len() as u64));
    let mut download = h
        .client
        .begin_download(&session.id, "result.bin")
        .await
        .unwrap();
    let mut received = Vec::new();
    tokio::time::timeout(Duration::from_secs(15), async {
        while let Some(chunk) = download.recv().await.unwrap() {
            received.extend_from_slice(&chunk);
        }
    })
    .await
    .expect("credit-paced download stalled");
    assert_eq!(received, contents);
    assert_eq!(
        h.client.stat_file(&session.id, ".").await.unwrap().status,
        DownloadFileStatus::Folder
    );
    assert!(h.client.begin_download(&session.id, ".").await.is_err());
    tokio::fs::remove_file(&path).await.unwrap();
    assert!(!path.exists());
    assert_eq!(
        h.client
            .stat_file(&session.id, "result.bin")
            .await
            .unwrap()
            .status,
        DownloadFileStatus::NotFound
    );
    assert!(
        h.client
            .begin_download(&session.id, "result.bin")
            .await
            .is_err()
    );
}

/// A download parked at its credit boundary must leave the shared connection
/// free for control requests. Abort cancels that parked read and ends that
/// read, so the same connection can start another read on a fresh channel.
#[farhelm_testtrace::test]
async fn paused_download_allows_control_and_abort_allows_a_fresh_read() {
    let h = harness().await;
    let (session, work) = basic_session(&h).await;
    let path = work.path().join("large.bin");
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(farhelm_proto::UPLOAD_WINDOW_BYTES * 2)
        .unwrap();
    assert_eq!(
        file.metadata().unwrap().len(),
        farhelm_proto::UPLOAD_WINDOW_BYTES * 2
    );
    let mut peer = RawPeer::connect(&h.sup).await;
    let begin = |req_id, channel| ControlMsg::BeginDownload {
        req_id,
        session_id: session.id.clone(),
        path: "large.bin".into(),
        channel,
    };
    peer.control(&begin(1, 7)).await;
    assert!(matches!(
        peer.next_control(5).await,
        ControlMsg::DownloadStarted {
            req_id: 1,
            channel: 7,
            ..
        }
    ));
    let mut received = 0;
    while received < farhelm_proto::UPLOAD_WINDOW_BYTES {
        let frame = tokio::time::timeout(Duration::from_secs(5), peer.reader.read_frame())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(frame.kind, FrameKind::Data);
        assert_eq!(frame.channel, 7);
        received += frame.body.len() as u64;
    }
    assert_eq!(received, farhelm_proto::UPLOAD_WINDOW_BYTES);
    peer.control(&ControlMsg::StatFile {
        req_id: 2,
        session_id: session.id.clone(),
        path: "large.bin".into(),
    })
    .await;
    let frame = tokio::time::timeout(Duration::from_secs(5), peer.reader.read_frame())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(matches!(
        parse_control(&frame).unwrap(),
        ControlMsg::FileStat { req_id: 2, .. }
    ));
    peer.control(&ControlMsg::AbortDownload { channel: 7 })
        .await;
    peer.control(&begin(3, 8)).await;
    assert!(matches!(
        peer.next_control(5).await,
        ControlMsg::DownloadStarted {
            req_id: 3,
            channel: 8,
            ..
        }
    ));
    peer.control(&ControlMsg::AbortDownload { channel: 8 })
        .await;
}

/// Hold the shared writer at the first file frame, before any byte crosses.
/// This names the backlog boundary rather than guessing when a producer has
/// queued enough data. Dropping the release sender unblocks teardown on panic.
struct GatedFileBytes {
    inner: tokio::io::DuplexStream,
    entered: Arc<tokio::sync::Notify>,
    release: Option<tokio::sync::oneshot::Receiver<()>>,
    seen: bool,
}

impl AsyncRead for GatedFileBytes {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(context, buffer)
    }
}

impl AsyncWrite for GatedFileBytes {
    fn poll_write(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        use std::future::Future;
        if !self.seen
            && Frame::decode(bytes)
                .ok()
                .flatten()
                .is_some_and(|(frame, _)| frame.kind == FrameKind::Data && frame.channel == 7)
        {
            self.seen = true;
            self.entered.notify_one();
        }
        if self.seen
            && let Some(release) = &mut self.release
        {
            if Pin::new(release).poll(context).is_pending() {
                return Poll::Pending;
            }
            self.release = None;
        }
        Pin::new(&mut self.inner).poll_write(context, bytes)
    }
    fn poll_flush(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(context)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(context)
    }
}

/// Already-queued file bytes and end events outlive abort. Reserving their channel until the
/// connection ends prevents them from becoming another download or terminal's
/// bytes, without a writer-drain handshake. A fresh id still works immediately.
#[farhelm_testtrace::test]
async fn queued_download_bytes_cannot_enter_a_reused_channel() {
    let h = harness().await;
    let (session, work) = basic_session(&h).await;
    let path = work.path().join("queued.bin");
    tokio::fs::write(&path, vec![0x71; farhelm_proto::UPLOAD_CHUNK_BYTES * 2])
        .await
        .unwrap();
    assert_eq!(
        std::fs::metadata(&path).unwrap().len(),
        (farhelm_proto::UPLOAD_CHUNK_BYTES * 2) as u64
    );
    let (client_side, server_side) = tokio::io::duplex(4096);
    let entered = Arc::new(tokio::sync::Notify::new());
    let (release, receiver) = tokio::sync::oneshot::channel();
    let server = GatedFileBytes {
        inner: server_side,
        entered: Arc::clone(&entered),
        release: Some(receiver),
        seen: false,
    };
    let sup = Arc::clone(&h.sup);
    tokio::spawn(async move {
        let _ = handle_connection(sup, server).await;
    });
    let (read, write) = tokio::io::split(client_side);
    let mut peer = RawPeer {
        reader: FrameReader::new(read),
        writer: FrameWriter::new(write),
    };
    handshake(&mut peer.reader, &mut peer.writer, "helm")
        .await
        .unwrap();
    let begin = |req_id, channel| ControlMsg::BeginDownload {
        req_id,
        session_id: session.id.clone(),
        path: "queued.bin".into(),
        channel,
    };
    peer.control(&begin(1, 7)).await;
    assert!(matches!(
        peer.next_control(5).await,
        ControlMsg::DownloadStarted {
            req_id: 1,
            channel: 7,
            ..
        }
    ));
    tokio::time::timeout(Duration::from_secs(5), entered.notified())
        .await
        .expect("file writer never reached its gate");
    peer.control(&ControlMsg::AbortDownload { channel: 7 })
        .await;
    peer.control(&begin(2, 7)).await;
    peer.control(&ControlMsg::Attach {
        req_id: 3,
        session_id: session.id.clone(),
        channel: 7,
        terminal: TerminalSelector::Agent,
        cols: 80,
        rows: 24,
        if_unowned: false,
        lease: "reuse-refused".into(),
    })
    .await;
    peer.control(&ControlMsg::BeginUpload {
        req_id: 4,
        session_id: session.id.clone(),
        channel: 7,
        filename: "upload".into(),
        size: 0,
    })
    .await;
    peer.control(&begin(5, 8)).await;
    release.send(()).unwrap();
    let old = tokio::time::timeout(Duration::from_secs(5), peer.reader.read_frame())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(old.kind, FrameKind::Data);
    assert_eq!(
        old.channel, 7,
        "the old file frame is still in flight after abort"
    );
    // Requests written to the peer are not yet proof of remote dispatch.
    // Old bulk frames may cross before the ordinary-queue refusals exist;
    // assert correlated outcomes without imposing order across those queues.
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut refused = std::collections::HashSet::new();
        let mut fresh_started = false;
        while refused.len() < 3 || !fresh_started {
            let frame = peer.reader.read_frame().await.unwrap().unwrap();
            if frame.kind == FrameKind::Data {
                assert!(matches!(frame.channel, 7 | 8));
                continue;
            }
            match parse_control(&frame).unwrap() {
                ControlMsg::Error {
                    req_id,
                    kind: ErrorKind::InvalidRequest,
                    ..
                } if matches!(req_id, 2..=4) => {
                    assert!(refused.insert(req_id), "duplicate reuse refusal");
                }
                ControlMsg::DownloadStarted {
                    req_id: 5,
                    channel: 8,
                    ..
                } => {
                    assert!(!fresh_started, "duplicate fresh start");
                    fresh_started = true;
                }
                ControlMsg::DownloadEnded {
                    channel: 7 | 8,
                    reason: None,
                } => {}
                other => panic!("unexpected reply while checking channel reuse: {other:?}"),
            }
        }
    })
    .await
    .expect("channel reuse replies stalled");
    peer.control(&ControlMsg::AbortDownload { channel: 8 })
        .await;
}

/// The request fits the wire but its echoed refusal does not. The ordinary
/// bounded-reply helper must substitute a correlated refusal and preserve the
/// connection, including when the begin reply travels on the bulk queue.
#[farhelm_testtrace::test]
async fn oversized_file_refusals_preserve_the_shared_connection() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let mut peer = RawPeer::connect(&h.sup).await;
    for req_id in [1, 2] {
        let request = |path| {
            if req_id == 1 {
                ControlMsg::StatFile {
                    req_id,
                    session_id: session.id.clone(),
                    path,
                }
            } else {
                ControlMsg::BeginDownload {
                    req_id,
                    session_id: session.id.clone(),
                    path,
                    channel: 7,
                }
            }
        };
        let overhead = Frame::control(&request(String::new())).encoded_len();
        let length = farhelm_proto::MAX_FRAME_LEN as usize - overhead;
        let path = format!(
            "~unsupported/{}",
            "x".repeat(length - "~unsupported/".len())
        );
        let message = request(path);
        assert!(
            !Frame::control(&message).exceeds_max_len(),
            "fixture request must fit"
        );
        peer.control(&message).await;
        assert!(
            matches!(peer.next_control(10).await, ControlMsg::Error { req_id: id, kind: ErrorKind::Internal, message } if id == req_id && message.contains("exceeding"))
        );
    }
    peer.control(&ControlMsg::StatFile {
        req_id: 3,
        session_id: session.id,
        path: ".".into(),
    })
    .await;
    assert!(matches!(
        peer.next_control(5).await,
        ControlMsg::FileStat { req_id: 3, .. }
    ));
}
