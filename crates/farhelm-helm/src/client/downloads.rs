//! Receive-direction file channels, independent of terminals and uploads.
//! A guard is registered before BeginDownload leaves, so fast data and a
//! cancellation during admission both have an owner. Demux never waits for a
//! consumer: credit bounds normal traffic, and overflow fails that transfer.

use super::*;
use farhelm_proto::{DOWNLOAD_LIMIT_BYTES, DownloadFileInfo, DownloadFileStatus};

/// Demux-side route. End is separate from the byte queue so failure is still
/// reportable when that bounded queue is full. The guard drains queued bytes
/// before observing a successful end, preserving the wire's EOF ordering.
pub(super) struct DownloadHandle {
    pub(super) bytes: mpsc::Sender<Vec<u8>>,
    pub(super) end: watch::Sender<Option<Result<(), String>>>,
}

/// One credit-paced file read. Drop cancels upstream on every unfinished exit,
/// including a cancelled HTTP body or a cancelled BeginDownload request.
/// Admission owns its pending reply entry too: AbortDownload may stop the
/// supervisor before it can answer, so connection teardown cannot be its only cleanup.
pub struct DownloadGuard {
    client: Arc<SupervisorClient>,
    channel: u32,
    admission_req_id: u64,
    bytes: mpsc::Receiver<Vec<u8>>,
    end: watch::Receiver<Option<Result<(), String>>>,
    pub info: DownloadFileInfo,
    received: u64,
    finished: bool,
}

impl SupervisorClient {
    /// Fresh host-side evidence for one path in a session's recorded directory.
    pub async fn stat_file(
        &self,
        session_id: &str,
        path: &str,
    ) -> anyhow::Result<DownloadFileInfo> {
        let req_id = self.req_id();
        match self
            .request(
                req_id,
                ControlMsg::StatFile {
                    req_id,
                    session_id: session_id.into(),
                    path: path.into(),
                },
            )
            .await?
        {
            ControlMsg::FileStat { info, .. } => Ok(info),
            other => Err(wrong_reply("StatFile", &other)),
        }
    }

    /// Open a regular file on the session host, independent of any earlier
    /// hover. Size/status are validated again here against an untrusted peer.
    pub async fn begin_download(
        self: &Arc<Self>,
        session_id: &str,
        path: &str,
    ) -> anyhow::Result<DownloadGuard> {
        let channel = allocate_channel(&self.next_channel)?;
        // The supervisor fills each frame except at EOF. Byte credit therefore
        // bounds frame count too; the extra slot holds the final short frame.
        let (bytes, receiver) = mpsc::channel(
            (farhelm_proto::UPLOAD_WINDOW_BYTES as usize)
                .div_ceil(farhelm_proto::UPLOAD_CHUNK_BYTES)
                + 1,
        );
        let (end, ended) = watch::channel(None);
        self.downloads
            .lock()
            .await
            .insert(channel, DownloadHandle { bytes, end });
        let req_id = self.req_id();
        let mut guard = DownloadGuard {
            client: Arc::clone(self),
            channel,
            admission_req_id: req_id,
            bytes: receiver,
            end: ended,
            info: DownloadFileInfo {
                path: String::new(),
                size: None,
                status: DownloadFileStatus::NotFound,
            },
            received: 0,
            finished: false,
        };
        let reply = self
            .request(
                req_id,
                ControlMsg::BeginDownload {
                    req_id,
                    session_id: session_id.into(),
                    path: path.into(),
                    channel,
                },
            )
            .await?;
        match reply {
            ControlMsg::DownloadStarted {
                channel: started,
                info,
                ..
            } if started == channel
                && info.status == DownloadFileStatus::Ready
                && info.size.is_some_and(|size| size <= DOWNLOAD_LIMIT_BYTES) =>
            {
                guard.info = info;
                Ok(guard)
            }
            other => Err(wrong_reply("BeginDownload", &other)),
        }
    }

    /// A channel-local overflow cannot park the shared reader or freeze terminals.
    pub(super) async fn route_download_bytes(
        self: &Arc<Self>,
        channel: u32,
        bytes: Vec<u8>,
    ) -> bool {
        let routes = self.downloads.lock().await;
        let Some(route) = routes.get(&channel) else {
            return false;
        };
        if bytes.len() > farhelm_proto::UPLOAD_CHUNK_BYTES || route.bytes.try_send(bytes).is_err() {
            route
                .end
                .send_replace(Some(Err("download receiver overflow".into())));
            self.cancel_download(channel, None);
        }
        true
    }

    /// Drop paths cannot await. The owned cleanup task retires routing and
    /// sends an idempotent cancellation, just like terminal/upload cleanup.
    /// A dropped guard also retires its admission request before sending Abort:
    /// that abort need not produce a reply. Already answered ids are harmless
    /// no-ops, and a late reply to a retired id is discarded by ordinary demux.
    fn cancel_download(self: &Arc<Self>, channel: u32, admission_req_id: Option<u64>) {
        let client = Arc::clone(self);
        tokio::spawn(async move {
            client.downloads.lock().await.remove(&channel);
            if let Some(req_id) = admission_req_id {
                client.pending.lock().await.map.remove(&req_id);
            }
            let _ = client
                .writer_tx
                .send(Frame::control(&ControlMsg::AbortDownload { channel }).into())
                .await;
        });
    }
}

impl DownloadGuard {
    /// Yield one chunk and return cumulative credit. A success is EOF only
    /// after all queued bytes have been consumed; any error invalidates every
    /// previously yielded chunk. Consumers must publish a save only after EOF.
    pub async fn recv(&mut self) -> anyhow::Result<Option<Vec<u8>>> {
        if self.finished {
            return Ok(None);
        }
        loop {
            if let Ok(bytes) = self.bytes.try_recv() {
                return self.consume(bytes).await;
            }
            let ended = self.end.borrow_and_update().clone();
            if let Some(result) = ended {
                // Demux publishes end only after enqueueing preceding bytes.
                // Check again because a frame may have arrived between the
                // first queue check and this end snapshot.
                if let Ok(bytes) = self.bytes.try_recv() {
                    return self.consume(bytes).await;
                }
                match result {
                    Ok(()) => {
                        self.client.downloads.lock().await.remove(&self.channel);
                        // Until removal completes, cancellation still owes the
                        // route cleanup that Drop hands to its independent task.
                        self.finished = true;
                        return Ok(None);
                    }
                    Err(reason) => anyhow::bail!(reason),
                }
            }
            tokio::select! {
                bytes = self.bytes.recv() => match bytes {
                    Some(bytes) => return self.consume(bytes).await,
                    None => anyhow::bail!("host disconnected during download"),
                },
                changed = self.end.changed() => { changed.map_err(|_| anyhow::anyhow!("host disconnected during download"))?; }
            }
        }
    }

    /// Enforce the cap here too: remote-host framing and metadata are untrusted.
    async fn consume(&mut self, bytes: Vec<u8>) -> anyhow::Result<Option<Vec<u8>>> {
        self.received += bytes.len() as u64;
        anyhow::ensure!(
            self.received <= DOWNLOAD_LIMIT_BYTES,
            "too large to download (100 MB limit)"
        );
        self.client
            .writer_tx
            .send(
                Frame::control(&ControlMsg::DownloadAck {
                    channel: self.channel,
                    received: self.received,
                })
                .into(),
            )
            .await
            .map_err(|_| anyhow::anyhow!("host disconnected during download"))?;
        Ok(Some(bytes))
    }
}

impl Drop for DownloadGuard {
    fn drop(&mut self) {
        if !self.finished {
            self.client
                .cancel_download(self.channel, Some(self.admission_req_id));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{DuplexStream, ReadHalf};
    use tokio::time::timeout;

    /// Read one scripted-peer control message with a deadline. The peer stays
    /// in the test's scope so withholding an admission reply cannot look like
    /// a transport loss that would clear every pending entry on its own.
    async fn read_control(reader: &mut FrameReader<ReadHalf<DuplexStream>>) -> ControlMsg {
        let frame = timeout(Duration::from_secs(5), reader.read_frame())
            .await
            .expect("the live peer did not receive a control message")
            .unwrap()
            .expect("the client closed the transport");
        parse_control(&frame).unwrap()
    }

    /// EOF does not disarm cancellation while route removal is waiting on
    /// demux's mutex. Holding that mutex and polling once fixes the exact
    /// boundary; the live peer's Abort proves Drop completed the cleanup.
    #[farhelm_testtrace::test]
    async fn cancelled_download_eof_cleanup_retires_its_route() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            (reader, writer)
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();
        let (mut reader, mut writer) = peer.await.unwrap();
        let admission = tokio::spawn({
            let client = Arc::clone(&client);
            async move { client.begin_download("session", "empty.txt").await }
        });
        let ControlMsg::BeginDownload {
            req_id, channel, ..
        } = read_control(&mut reader).await
        else {
            panic!("expected download admission");
        };
        writer
            .write_control(&ControlMsg::DownloadStarted {
                req_id,
                channel,
                info: DownloadFileInfo {
                    path: "/fixture/empty.txt".into(),
                    size: Some(0),
                    status: DownloadFileStatus::Ready,
                },
            })
            .await
            .unwrap();
        let mut download = admission.await.unwrap().unwrap();
        let routes = client.downloads.lock().await;
        let route = routes.get(&channel).expect("admitted route exists");
        // This is the same publication demux makes while holding this lock.
        route.end.send_replace(Some(Ok(())));
        assert_eq!(*download.end.borrow(), Some(Ok(())));
        assert!(download.bytes.is_empty(), "EOF must have no queued data");
        assert!(!client.pending.lock().await.closed, "peer must stay live");
        let mut receive = Box::pin(download.recv());
        std::future::poll_fn(|cx| {
            assert!(std::future::Future::poll(receive.as_mut(), cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        drop(receive);
        drop(download);
        assert!(routes.contains_key(&channel), "cleanup is still contended");
        drop(routes);

        // Abort follows route retirement in the owned cleanup task. No reply
        // or transport loss is allowed to supply the removal being asserted.
        assert_eq!(
            read_control(&mut reader).await,
            ControlMsg::AbortDownload { channel }
        );
        assert!(!client.downloads.lock().await.contains_key(&channel));
        assert!(!client.pending.lock().await.closed);
    }

    /// Cancelling admission must retire exactly its unanswered request while
    /// the connection stays live. Repeat the boundary to expose accumulation;
    /// an unrelated stat and a late admission reply prove cleanup neither drains
    /// other requests nor depends on the supervisor answering the cancelled one.
    #[farhelm_testtrace::test]
    async fn cancelled_download_admission_retires_only_its_pending_request() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            (reader, writer)
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();
        let (mut reader, mut writer) = peer.await.unwrap();
        let info = DownloadFileInfo {
            path: "/fixture/file.txt".into(),
            size: Some(1),
            status: DownloadFileStatus::Ready,
        };

        for _ in 0..3 {
            let admission = tokio::spawn({
                let client = Arc::clone(&client);
                async move { client.begin_download("session", "file.txt").await }
            });
            let ControlMsg::BeginDownload {
                req_id, channel, ..
            } = read_control(&mut reader).await
            else {
                panic!("expected download admission");
            };
            assert!(client.pending.lock().await.map.contains_key(&req_id));
            assert!(client.downloads.lock().await.contains_key(&channel));

            let stat = tokio::spawn({
                let client = Arc::clone(&client);
                async move { client.stat_file("session", "other.txt").await }
            });
            let ControlMsg::StatFile {
                req_id: stat_id, ..
            } = read_control(&mut reader).await
            else {
                panic!("expected the unrelated stat request");
            };
            assert_eq!(client.pending.lock().await.map.len(), 2);
            admission.abort();
            let cancelled = admission.await;
            assert!(matches!(cancelled, Err(error) if error.is_cancelled()));

            // Abort is enqueued after the owned task retires both entries.
            // Observing it pins cleanup without a sleep or a reply that could
            // accidentally clear the admission through the ordinary path.
            assert_eq!(
                read_control(&mut reader).await,
                ControlMsg::AbortDownload { channel }
            );
            assert!(!client.downloads.lock().await.contains_key(&channel));
            {
                let pending = client.pending.lock().await;
                assert!(
                    !pending.closed,
                    "connection teardown must not supply cleanup"
                );
                assert!(!pending.map.contains_key(&req_id));
                assert_eq!(pending.map.len(), 1);
                assert!(pending.map.contains_key(&stat_id));
            }

            writer
                .write_control(&ControlMsg::DownloadStarted {
                    req_id,
                    channel,
                    info: info.clone(),
                })
                .await
                .unwrap();
            writer
                .write_control(&ControlMsg::FileStat {
                    req_id: stat_id,
                    info: info.clone(),
                })
                .await
                .unwrap();
            let observed = timeout(Duration::from_secs(5), stat)
                .await
                .expect("the live connection did not answer stat after the late admission reply")
                .unwrap()
                .unwrap();
            assert_eq!(observed, info);
            let pending = client.pending.lock().await;
            assert!(!pending.closed);
            assert!(pending.map.is_empty());
        }
    }
}
