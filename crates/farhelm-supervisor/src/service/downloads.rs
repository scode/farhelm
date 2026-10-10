//! Helm-only filesystem reads on a session's host. Hover checks and transfers
//! open independently: a successful hover cannot reserve a pathname or promise
//! that the same file will still be there when the user clicks.
//!
//! Download data uses the connection's bulk queue, below terminal traffic.
//! Credit limits outstanding bytes; cancellation and connection teardown abort
//! the task even when it is parked waiting for credit or writer capacity.

use super::connection::{reply_frame, send_reply};
use super::core::{RequestError, Supervisor, error_kind};
use farhelm_proto::{ControlMsg, DownloadFileInfo, DownloadFileStatus, ErrorKind, Frame};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tokio::io::AsyncReadExt;
use tokio::sync::{mpsc, watch};

/// A connection owns the cancellation handle and the receiver's credit. The
/// sent counter rejects invented credit before it can release more file bytes.
pub(crate) struct DownloadRoute {
    pub(crate) task: tokio::task::AbortHandle,
    credit: watch::Sender<u64>,
    sent: Arc<AtomicU64>,
}

impl Drop for DownloadRoute {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl DownloadRoute {
    /// Invalid credit ends only this download; a duplicate grants no new bytes.
    pub(crate) fn ack(&self, received: u64) {
        if *self.credit.borrow() == u64::MAX {
            return;
        }
        if received < *self.credit.borrow() || received > self.sent.load(Ordering::Acquire) {
            // Closing credit wakes the sender with a protocol refusal, instead
            // of aborting silently and leaving the helm waiting for EOF.
            self.credit.send_replace(u64::MAX);
        } else {
            self.credit.send_replace(received);
        }
    }
}

impl Supervisor {
    /// Session metadata, rather than this daemon's cwd, anchors relative paths.
    /// Home expansion uses the account home captured at startup, just as Create.
    pub(crate) async fn download_path(&self, session: &str, path: &str) -> anyhow::Result<PathBuf> {
        let row = self
            .store
            .session(session)
            .await?
            .ok_or_else(|| RequestError::new(ErrorKind::NotFound, "session not found"))?;
        resolve_path(&row.cwd, self.user_home.as_deref(), path)
    }
}

/// Resolve without shell expansion. `~user` is unsupported, and malformed
/// empty/NUL paths are refused before they reach filesystem operations.
fn resolve_path(cwd: &str, home: Option<&Path>, path: &str) -> anyhow::Result<PathBuf> {
    if path.is_empty() || path.contains('\0') {
        return Err(RequestError::new(ErrorKind::InvalidRequest, "invalid file path").into());
    }
    let expanded = super::core::expand_tilde_cwd(path, home)?;
    let path = Path::new(expanded.as_ref());
    Ok(if path.is_absolute() {
        path.to_owned()
    } else {
        Path::new(cwd).join(path)
    })
}

/// Open with ordinary Unix account authority, following symlinks. O_NONBLOCK
/// prevents a race replacing a checked path with a FIFO from parking the open;
/// the opened descriptor, not the earlier metadata, must be a regular file.
async fn inspect(path: PathBuf) -> anyhow::Result<(DownloadFileInfo, Option<tokio::fs::File>)> {
    let mut info = DownloadFileInfo {
        path: path
            .to_str()
            .ok_or_else(|| {
                RequestError::new(ErrorKind::InvalidRequest, "file path is not valid UTF-8")
            })?
            .to_string(),
        size: None,
        status: DownloadFileStatus::NotFound,
    };
    let canonical = match tokio::fs::canonicalize(&path).await {
        Ok(path) => path,
        Err(error) => {
            info.status = io_status(&error);
            return Ok((info, None));
        }
    };
    info.path = canonical
        .to_str()
        .ok_or_else(|| {
            RequestError::new(
                ErrorKind::InvalidRequest,
                "resolved file path is not valid UTF-8",
            )
        })?
        .to_string();
    let metadata = match tokio::fs::metadata(&canonical).await {
        Ok(metadata) => metadata,
        Err(error) => {
            info.status = io_status(&error);
            return Ok((info, None));
        }
    };
    info.size = Some(metadata.len());
    if metadata.is_dir() {
        info.status = DownloadFileStatus::Folder;
        return Ok((info, None));
    }
    if !metadata.is_file() {
        info.status = DownloadFileStatus::NotRegular;
        return Ok((info, None));
    }
    let file = match tokio::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(&canonical)
        .await
    {
        Ok(file) => file,
        Err(error) => {
            info.status = io_status(&error);
            return Ok((info, None));
        }
    };
    let metadata = file.metadata().await?;
    info.size = Some(metadata.len());
    info.status = if !metadata.is_file() {
        DownloadFileStatus::NotRegular
    } else if metadata.len() > farhelm_proto::DOWNLOAD_LIMIT_BYTES {
        DownloadFileStatus::TooLarge
    } else {
        DownloadFileStatus::Ready
    };
    let ready = info.status == DownloadFileStatus::Ready;
    Ok((info, ready.then_some(file)))
}

/// Missing and unreadable paths have distinct hover refusals. Other filesystem
/// failures remain refusals; healthy-local-filesystem policy adds no retry loop.
fn io_status(error: &std::io::Error) -> DownloadFileStatus {
    if error.kind() == std::io::ErrorKind::NotFound {
        DownloadFileStatus::NotFound
    } else {
        DownloadFileStatus::NotReadable
    }
}

/// A stat never occupies the connection reader while the filesystem answers.
pub(crate) async fn stat(
    sup: Arc<Supervisor>,
    tx: mpsc::Sender<Frame>,
    req_id: u64,
    session: String,
    path: String,
) {
    let result = async {
        inspect(sup.download_path(&session, &path).await?)
            .await
            .map(|(info, _)| info)
    }
    .await;
    let reply = match result {
        Ok(info) => ControlMsg::FileStat { req_id, info },
        Err(error) => refusal(req_id, &error),
    };
    send_reply(&tx, &reply).await;
}

/// Register before spawning so an immediate cancellation/ack can find its route.
/// Begin, bytes and end share one FIFO even though other traffic has priority.
pub(crate) fn start(
    tasks: &mut tokio::task::JoinSet<()>,
    sup: Arc<Supervisor>,
    bulk: mpsc::Sender<Frame>,
    req_id: u64,
    session: String,
    path: String,
    channel: u32,
) -> DownloadRoute {
    let (credit, receiver) = watch::channel(0);
    let sent = Arc::new(AtomicU64::new(0));
    let task_sent = Arc::clone(&sent);
    let task = tasks.spawn(async move {
        let opened = async { inspect(sup.download_path(&session, &path).await?).await }.await;
        let (info, file) = match opened {
            Ok((info, Some(file))) => (info, file),
            Ok((info, None)) => {
                let error = RequestError::new(
                    ErrorKind::InvalidRequest,
                    format!("file is not downloadable: {:?}", info.status),
                );
                let _ = bulk
                    .send(reply_frame(&refusal(req_id, &error.into())))
                    .await;
                return;
            }
            Err(error) => {
                let _ = bulk.send(reply_frame(&refusal(req_id, &error))).await;
                return;
            }
        };
        let started = ControlMsg::DownloadStarted {
            req_id,
            channel,
            info,
        };
        let frame = reply_frame(&started);
        let refused = Frame::control(&started).exceeds_max_len();
        if bulk.send(frame).await.is_err() || refused {
            return;
        }
        let result = stream(file, channel, &bulk, receiver, task_sent).await;
        let _ = bulk
            .send(Frame::control(&ControlMsg::DownloadEnded {
                channel,
                reason: result.err().map(|error| error.to_string()),
            }))
            .await;
    });
    DownloadRoute { task, credit, sent }
}

/// Read to EOF, refusing growth past the cap before forwarding the excess.
/// Full-sized frames (except at EOF) also bound frame count within byte credit,
/// matching the helm's finite receive queue even when the filesystem reads short.
/// The credit wait is deliberately unbounded: a slow peer is allowed to be
/// slow, while AbortDownload and connection death cancel its owned task.
async fn stream(
    mut file: impl tokio::io::AsyncRead + Unpin,
    channel: u32,
    bulk: &mpsc::Sender<Frame>,
    mut credit: watch::Receiver<u64>,
    sent: Arc<AtomicU64>,
) -> anyhow::Result<()> {
    let mut total = 0u64;
    loop {
        let mut chunk = vec![0; farhelm_proto::UPLOAD_CHUNK_BYTES];
        let mut count = 0;
        while count < chunk.len() {
            let read = file.read(&mut chunk[count..]).await?;
            if read == 0 {
                break;
            }
            count += read;
        }
        if count == 0 {
            return Ok(());
        }
        chunk.truncate(count);
        total += count as u64;
        anyhow::ensure!(
            total <= farhelm_proto::DOWNLOAD_LIMIT_BYTES,
            "too large to download (100 MB limit)"
        );
        loop {
            let received = *credit.borrow_and_update();
            anyhow::ensure!(received != u64::MAX, "invalid download credit");
            if total <= received.saturating_add(farhelm_proto::UPLOAD_WINDOW_BYTES) {
                break;
            }
            credit.changed().await?;
        }
        sent.store(total, Ordering::Release);
        if bulk.send(Frame::data(channel, chunk)).await.is_err() {
            anyhow::bail!("connection closed");
        }
        // A partial frame means EOF was observed. Appends while waiting for
        // credit must not turn it into an intermediate short frame.
        if count < farhelm_proto::UPLOAD_CHUNK_BYTES {
            return Ok(());
        }
    }
}

/// Preserve request correlation and the repository's ordinary error mapping.
fn refusal(req_id: u64, error: &anyhow::Error) -> ControlMsg {
    ControlMsg::Error {
        req_id,
        kind: error_kind(error),
        message: format!("{error:#}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    /// A legal source whose reads stop at page boundaries, independent of the
    /// requested buffer. The signal marks read-ahead beyond the first window,
    /// so the test can inspect a stalled consumer without a scheduling sleep.
    struct ShortReads {
        remaining: usize,
        read_ahead: Option<tokio::sync::oneshot::Sender<()>>,
        observed_eof: Arc<AtomicU64>,
    }

    impl tokio::io::AsyncRead for ShortReads {
        fn poll_read(
            mut self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
            buffer: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            let count = self.remaining.min(buffer.remaining()).min(4096);
            if count == 0 {
                // The first EOF is final for this transfer. A later read would
                // discover an append, deliberately exposing a broken producer.
                self.observed_eof.fetch_add(1, Ordering::Relaxed);
                self.remaining = 4096;
                return std::task::Poll::Ready(Ok(()));
            }
            buffer.put_slice(&[0x73; 4096][..count]);
            self.remaining -= count;
            if self.remaining == 7
                && let Some(signal) = self.read_ahead.take()
            {
                let _ = signal.send(());
            }
            std::task::Poll::Ready(Ok(()))
        }
    }

    /// A paused consumer can retain an entire byte window without frame
    /// overflow, even if its source returns only 4 KiB per filesystem read.
    /// The final seven bytes prove that only EOF permits a short wire frame.
    #[farhelm_testtrace::test]
    async fn short_reads_fill_frames_before_consuming_byte_credit() {
        let window = farhelm_proto::UPLOAD_WINDOW_BYTES as usize;
        let chunk = farhelm_proto::UPLOAD_CHUNK_BYTES;
        let (signal, read_ahead) = tokio::sync::oneshot::channel();
        let observed_eof = Arc::new(AtomicU64::new(0));
        let source = ShortReads {
            remaining: window + chunk + 7,
            read_ahead: Some(signal),
            observed_eof: Arc::clone(&observed_eof),
        };
        let frame_capacity = window.div_ceil(chunk) + 1;
        let (bulk, mut frames) = mpsc::channel(frame_capacity);
        let (credit, receiver) = watch::channel(0);
        let sent = Arc::new(AtomicU64::new(0));
        let observed = Arc::clone(&sent);
        let task = tokio::spawn(async move { stream(source, 7, &bulk, receiver, sent).await });
        tokio::time::timeout(std::time::Duration::from_secs(5), read_ahead)
            .await
            .expect("producer did not reach its first credit wait")
            .unwrap();
        assert_eq!(observed.load(Ordering::Acquire), window as u64);
        assert!(!task.is_finished());
        assert_eq!(frames.len(), window / chunk);
        let mut body = Vec::new();
        while let Ok(frame) = frames.try_recv() {
            assert_eq!(frame.channel, 7);
            assert_eq!(frame.body.len(), chunk);
            body.extend(frame.body);
        }
        credit.send_replace(window as u64);
        while let Some(frame) =
            tokio::time::timeout(std::time::Duration::from_secs(5), frames.recv())
                .await
                .expect("producer did not finish after credit")
        {
            assert_eq!(frame.channel, 7);
            assert_eq!(
                frame.body.len(),
                if body.len() == window { chunk } else { 7 }
            );
            body.extend(frame.body);
        }
        task.await.unwrap().unwrap();
        assert_eq!(observed_eof.load(Ordering::Relaxed), 1);
        assert_eq!(body, vec![0x73; window + chunk + 7]);
    }

    /// Relative and home paths must resolve on the session host, regardless of
    /// the helm's cwd. Unsupported shell-style expansion must stay literal/refused.
    #[test]
    fn paths_use_session_cwd_and_captured_home() {
        let home = Path::new("/home/session");
        for (input, expected) in [
            ("out.txt", "/work/out.txt"),
            ("src/main.rs", "/work/src/main.rs"),
            ("~/out.txt", "/home/session/out.txt"),
            ("/srv/out.txt", "/srv/out.txt"),
        ] {
            assert_eq!(
                resolve_path("/work", Some(home), input).unwrap(),
                Path::new(expected)
            );
        }
        for input in ["", "bad\0path", "~another/out"] {
            assert!(
                resolve_path("/work", Some(home), input).is_err(),
                "{input:?}"
            );
        }
        assert!(resolve_path("/work", None, "~/out").is_err());
    }

    /// Hover distinguishes refused filesystem objects without granting a later
    /// read. Symlinks retain normal same-account access and show their full target.
    #[farhelm_testtrace::test]
    async fn stat_checks_kind_readability_size_and_symlink_target() {
        let root = farhelm_teststate::tempdir().unwrap();
        let file = root.path().join("report.txt");
        tokio::fs::write(&file, b"report").await.unwrap();
        symlink(&file, root.path().join("link.txt")).unwrap();
        for path in [&file, &root.path().join("link.txt")] {
            let (info, opened) = inspect(path.to_owned()).await.unwrap();
            assert_eq!(info.status, DownloadFileStatus::Ready);
            assert_eq!(info.size, Some(6));
            assert_eq!(info.path, file.to_str().unwrap());
            assert!(opened.is_some());
        }
        assert_eq!(
            inspect(root.path().to_owned()).await.unwrap().0.status,
            DownloadFileStatus::Folder
        );
        assert_eq!(
            inspect(root.path().join("missing")).await.unwrap().0.status,
            DownloadFileStatus::NotFound
        );
        let large = root.path().join("large");
        let large_file = std::fs::File::create(&large).unwrap();
        large_file
            .set_len(farhelm_proto::DOWNLOAD_LIMIT_BYTES + 1)
            .unwrap();
        assert_eq!(
            large_file.metadata().unwrap().len(),
            farhelm_proto::DOWNLOAD_LIMIT_BYTES + 1
        );
        assert_eq!(
            inspect(large).await.unwrap().0.status,
            DownloadFileStatus::TooLarge
        );
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o000)).unwrap();
        // Root bypasses Unix permission bits; require the refused-open premise
        // explicitly rather than silently skipping the promised coverage.
        assert_eq!(
            std::fs::File::open(&file).unwrap_err().kind(),
            std::io::ErrorKind::PermissionDenied
        );
        assert_eq!(
            inspect(file).await.unwrap().0.status,
            DownloadFileStatus::NotReadable
        );
    }

    /// A credit wait is an observable boundary: after exactly one window has
    /// arrived, the sender cannot finish a larger file until the receiver acks.
    /// Growth after that boundary must fail, not publish a successful prefix.
    #[farhelm_testtrace::test]
    async fn credit_paces_read_and_growth_past_limit_fails() {
        let root = farhelm_teststate::tempdir().unwrap();
        let path = root.path().join("growing");
        let writer = std::fs::File::create(&path).unwrap();
        writer
            .set_len(farhelm_proto::UPLOAD_WINDOW_BYTES + farhelm_proto::UPLOAD_CHUNK_BYTES as u64)
            .unwrap();
        let (info, file) = inspect(path).await.unwrap();
        assert_eq!(info.status, DownloadFileStatus::Ready);
        let (bulk, mut frames) = mpsc::channel(17);
        let (credit, receiver) = watch::channel(0);
        let sent = Arc::new(AtomicU64::new(0));
        let mut task =
            tokio::spawn(async move { stream(file.unwrap(), 7, &bulk, receiver, sent).await });
        let mut received = 0;
        while received < farhelm_proto::UPLOAD_WINDOW_BYTES {
            let frame = tokio::time::timeout(std::time::Duration::from_secs(5), frames.recv())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(frame.channel, 7);
            received += frame.body.len() as u64;
        }
        assert_eq!(received, farhelm_proto::UPLOAD_WINDOW_BYTES);
        assert!(!task.is_finished());
        assert!(frames.try_recv().is_err());
        writer
            .set_len(farhelm_proto::DOWNLOAD_LIMIT_BYTES + 1)
            .unwrap();
        assert_eq!(
            writer.metadata().unwrap().len(),
            farhelm_proto::DOWNLOAD_LIMIT_BYTES + 1
        );
        credit.send_replace(received);
        let result = loop {
            tokio::select! {
                result = &mut task => break result,
                frame = frames.recv() => {
                    let Some(frame) = frame else { break task.await; };
                    received += frame.body.len() as u64;
                    assert!(received <= farhelm_proto::DOWNLOAD_LIMIT_BYTES);
                    credit.send_replace(received);
                }
            }
        };
        let error = result.unwrap().unwrap_err();
        assert!(error.to_string().contains("too large"), "{error}");
    }
}
