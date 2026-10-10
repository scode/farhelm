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
pub struct DownloadGuard {
    client: Arc<SupervisorClient>,
    channel: u32,
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
        let mut guard = DownloadGuard {
            client: Arc::clone(self),
            channel,
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
        let req_id = self.req_id();
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
            self.cancel_download(channel);
        }
        true
    }

    /// Drop paths cannot await. The owned cleanup task retires routing and
    /// sends an idempotent cancellation, just like terminal/upload cleanup.
    fn cancel_download(self: &Arc<Self>, channel: u32) {
        let client = Arc::clone(self);
        tokio::spawn(async move {
            client.downloads.lock().await.remove(&channel);
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
                        self.finished = true;
                        self.client.downloads.lock().await.remove(&self.channel);
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
            self.client.cancel_download(self.channel);
        }
    }
}
