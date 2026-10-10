//! Authenticated file reads routed to the host that owns the session.
//! Browser bodies stream through the helm; only the browser buffers a Blob.
//! The embedded desktop helm instead stages in Downloads and publishes only
//! after EOF. Neither a hover nor a previous successful read reserves a file.

use crate::sessions::route_session;
use crate::{AppState, DownloadGuard, http_error};
use axum::extract::{Path as AxPath, State};
use axum::response::IntoResponse;
use axum::{Json, body::Body};
use farhelm_proto::DownloadFileInfo;
use farhelm_supervisor::files::{RealFs, StagedStream};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Agent-provided path text is data, never a shell expression. Save mode is
/// an explicit request to the desktop capability; server helms refuse it.
#[derive(Deserialize)]
pub(crate) struct FileRequest {
    path: String,
    #[serde(default)]
    save_to_downloads: bool,
}

/// Fresh host evidence with the host's display name for the hover disclosure.
/// Flattening preserves the protocol's typed refusal and optional size.
#[derive(serde::Serialize)]
struct FileStatBody {
    host: String,
    #[serde(flatten)]
    info: DownloadFileInfo,
}

/// Resolve and inspect without moving bytes. The normal session-owner route
/// supplies the same disconnected-host refusal as other session operations.
pub(crate) async fn stat_file(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
    Json(request): Json<FileRequest>,
) -> axum::response::Response {
    let result = async {
        let (claim, client) = route_session(&state, &id).await?;
        let info = client.stat_file(&id, &request.path).await?;
        let host = state
            .manager
            .snapshots()
            .into_iter()
            .find(|row| row.id == claim.host)
            .map(|row| {
                crate::aggregate::host_display_name(
                    row.kind,
                    row.destination.as_deref(),
                    row.alias.as_deref(),
                )
            })
            .unwrap_or_else(|| "host".into());
        Ok::<_, anyhow::Error>(FileStatBody { host, info })
    }
    .await;
    match result {
        Ok(body) => Json(body).into_response(),
        Err(error) => http_error(error),
    }
}

/// Reopen on click and either relay bytes or save natively. Browser errors
/// after headers become body errors, so fetch must finish the whole Blob
/// before offering it to the browser's save action. No opening Content-Length
/// is promised: a file may grow or shrink while it is read, up to the cap.
pub(crate) async fn download_file(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
    Json(request): Json<FileRequest>,
) -> axum::response::Response {
    if request.save_to_downloads && state.downloads_dir.is_none() {
        return (
            axum::http::StatusCode::NOT_FOUND,
            "native Downloads save is unavailable\n",
        )
            .into_response();
    }
    let result = async {
        let (_, client) = route_session(&state, &id).await?;
        client.begin_download(&id, &request.path).await
    }
    .await;
    let download = match result {
        Ok(download) => download,
        Err(error) => return http_error(error),
    };
    if request.save_to_downloads {
        return match save_download(download, state.downloads_dir.clone().unwrap()).await {
            Ok(path) => Json(serde_json::json!({"path": path.to_string_lossy()})).into_response(),
            Err(error) => http_error(error),
        };
    }
    let filename = match basename(&download.info.path) {
        Ok(name) => name,
        Err(error) => return http_error(error),
    };
    let disposition = format!(
        "attachment; filename*=UTF-8''{}",
        percent_encoding::utf8_percent_encode(&filename, percent_encoding::NON_ALPHANUMERIC)
    );
    let stream = futures_util::stream::try_unfold(download, |mut download| async move {
        match download.recv().await? {
            Some(bytes) => Ok::<_, anyhow::Error>(Some((bytes, download))),
            None => Ok(None),
        }
    });
    (
        [
            (axum::http::header::CONTENT_TYPE, "application/octet-stream"),
            (
                axum::http::header::CONTENT_DISPOSITION,
                disposition.as_str(),
            ),
        ],
        Body::from_stream(stream),
    )
        .into_response()
}

/// A host path supplies a leaf name only. Even a malformed remote answer must
/// never turn desktop publication into a path outside the injected directory.
fn basename(path: &str) -> anyhow::Result<String> {
    let name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty() && !name.contains('\0'))
        .ok_or_else(|| anyhow::anyhow!("download has no usable file name"))?;
    Ok(name.into())
}

/// The desktop's naming convention preserves the final extension, including
/// dotfiles. No existence check precedes publication; atomic no-clobber links
/// resolve concurrent saves against this same candidate sequence.
fn candidates(name: String) -> impl Iterator<Item = String> {
    let path = Path::new(&name);
    let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
    let extension = path
        .extension()
        .map(|ext| format!(".{}", ext.to_string_lossy()))
        .unwrap_or_default();
    std::iter::once(name).chain((1u64..).map(move |n| format!("{stem} ({n}){extension}")))
}

/// One owner keeps the stream and its private directory together through
/// blocking work. File teardown precedes directory teardown, including when
/// an async save is cancelled while a chunk write is still in progress.
struct SaveFiles {
    file: StagedStream,
    directory: tempfile::TempDir,
}

/// Async ownership must not turn a failed transfer into synchronous filesystem
/// cleanup on a Tokio worker. Drop schedules that cleanup on the blocking pool;
/// reported receive failures await the same cleanup before answering.
struct NativeSave(Option<SaveFiles>);

impl NativeSave {
    /// Request private permissions at creation rather than relying on umask.
    /// The temp directory shares the publication filesystem for atomic links.
    fn create(directory: PathBuf) -> anyhow::Result<Self> {
        use std::os::unix::fs::PermissionsExt;
        std::fs::create_dir_all(&directory)?;
        let staging = tempfile::Builder::new()
            .prefix(".farhelm-download-")
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir_in(&directory)?;
        let file = StagedStream::create(staging.path(), &directory, "download", &RealFs)?;
        Ok(Self(Some(SaveFiles {
            file,
            directory: staging,
        })))
    }

    /// Consume and return the entire owner, so cancellation of the caller's
    /// await cannot remove the staging source while this blocking write runs.
    fn write_chunk(mut self, bytes: Vec<u8>) -> anyhow::Result<Self> {
        self.0.as_mut().unwrap().file.write_chunk(&RealFs, &bytes)?;
        Ok(self)
    }

    /// Publish only a fully received stream. Ownership stays on the blocking
    /// thread through the no-clobber link and cosmetic staging cleanup.
    fn publish(mut self, name: String) -> anyhow::Result<PathBuf> {
        let files = self.0.take().unwrap();
        let result = files.file.publish_no_clobber(&RealFs, candidates(name));
        drop(files.directory);
        result.map_err(Into::into)
    }

    /// A reported transfer error waits for removal before returning a refusal.
    /// Attempt both removals even when one fails, preserving cleanup evidence.
    async fn cleanup(mut self) -> anyhow::Result<()> {
        let files = self.0.take().unwrap();
        tokio::task::spawn_blocking(move || {
            let stream = files.file.abandon(&RealFs);
            let directory = files.directory.close();
            stream?;
            directory?;
            Ok::<_, anyhow::Error>(())
        })
        .await?
    }
}

impl Drop for NativeSave {
    fn drop(&mut self) {
        if let Some(files) = self.0.take() {
            tokio::task::spawn_blocking(move || drop(files));
        }
    }
}

/// Private staging shares the Downloads filesystem, so publication is atomic
/// even when the system temp directory is on another volume. The owned temp
/// directory and StagedStream clean up on errors and task cancellation; no
/// partially received file is published under a user-visible save name.
async fn save_download(mut download: DownloadGuard, directory: PathBuf) -> anyhow::Result<PathBuf> {
    let name = basename(&download.info.path)?;
    let mut save = tokio::task::spawn_blocking(move || NativeSave::create(directory)).await??;
    loop {
        match download.recv().await {
            Ok(Some(bytes)) => {
                save = tokio::task::spawn_blocking(move || save.write_chunk(bytes)).await??;
            }
            Ok(None) => break,
            Err(error) => {
                if let Err(cleanup) = save.cleanup().await {
                    return Err(error.context(format!("download cleanup failed: {cleanup}")));
                }
                return Err(error);
            }
        }
    }
    tokio::task::spawn_blocking(move || save.publish(name)).await?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rest_harness::{FakeSupervisor, session, spliced_helm_listing};
    use axum::http::{Request, StatusCode};
    use farhelm_proto::{ControlMsg, DownloadFileStatus, Frame};
    use tower::ServiceExt;

    /// Drive the real router rather than a handler call, including the JSON
    /// extractor and device authentication supplied by the REST harness.
    fn request(operation: &str, save: bool) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(format!("/api/sessions/sess-1/files/{operation}"))
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::json!({"path": "out/report.pdf", "save_to_downloads": save})
                    .to_string(),
            ))
            .unwrap()
    }

    /// Open metadata is deliberately smaller than the body: HTTP must neither
    /// truncate growth below the cap nor advertise the stale opening length.
    fn info() -> DownloadFileInfo {
        DownloadFileInfo {
            path: "/work/report.pdf".into(),
            size: Some(3),
            status: DownloadFileStatus::Ready,
        }
    }

    /// Script one real download exchange. Waiting for credit proves the body
    /// reached its consumer before the terminal outcome, including failure.
    async fn reply_download(peer: &mut FakeSupervisor, failure: bool) {
        let ControlMsg::BeginDownload {
            req_id,
            session_id,
            path,
            channel,
        } = peer.recv().await
        else {
            panic!("expected session-host begin");
        };
        assert_eq!(session_id, "sess-1");
        assert_eq!(path, "out/report.pdf");
        peer.send(&ControlMsg::DownloadStarted {
            req_id,
            channel,
            info: info(),
        })
        .await;
        peer.send_frame(&Frame::data(channel, b"report-body".to_vec()))
            .await;
        assert!(
            matches!(peer.recv().await, ControlMsg::DownloadAck { channel: ack, received: 11 } if ack == channel)
        );
        peer.send(&ControlMsg::DownloadEnded {
            channel,
            reason: failure.then(|| "read failed sentinel".into()),
        })
        .await;
    }

    /// Host refusal states must stay typed and the disclosed path must come
    /// from that host, rather than from the request or the helm filesystem.
    #[farhelm_testtrace::test]
    async fn stat_routes_and_returns_fresh_host_evidence() {
        let (client, server) = tokio::io::duplex(65536);
        let peer = tokio::spawn(async move {
            let mut peer = FakeSupervisor::accept(server).await;
            let ControlMsg::StatFile {
                req_id,
                session_id,
                path,
            } = peer.recv().await
            else {
                panic!("expected stat");
            };
            assert_eq!(session_id, "sess-1");
            assert_eq!(path, "out/report.pdf");
            let mut answer = info();
            answer.status = DownloadFileStatus::TooLarge;
            answer.size = Some(100_000_001);
            peer.send(&ControlMsg::FileStat {
                req_id,
                info: answer,
            })
            .await;
        });
        let h = spliced_helm_listing(client, vec![session("sess-1", 1)]).await;
        let response = h.router().oneshot(request("stat", false)).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 4096)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(body["path"], "/work/report.pdf");
        assert_eq!(body["size"], 100_000_001u64);
        assert_eq!(body["status"], "too_large");
        assert_eq!(body["host"], "this machine");
        peer.await.unwrap();
    }

    /// A streaming response preserves every byte through EOF without pinning
    /// its length to admission metadata, and encodes the basename as data.
    #[farhelm_testtrace::test]
    async fn browser_body_streams_to_eof_with_attachment_name() {
        let (client, server) = tokio::io::duplex(65536);
        let peer = tokio::spawn(async move {
            let mut peer = FakeSupervisor::accept(server).await;
            reply_download(&mut peer, false).await;
        });
        let h = spliced_helm_listing(client, vec![session("sess-1", 1)]).await;
        let response = h
            .router()
            .oneshot(request("download", false))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            response
                .headers()
                .get(axum::http::header::CONTENT_LENGTH)
                .is_none()
        );
        assert_eq!(
            response.headers()[axum::http::header::CONTENT_DISPOSITION],
            "attachment; filename*=UTF-8''report%2Epdf"
        );
        assert_eq!(
            axum::body::to_bytes(response.into_body(), 4096)
                .await
                .unwrap(),
            b"report-body"[..]
        );
        peer.await.unwrap();
    }

    /// An upstream read error after HTTP headers must invalidate the whole
    /// body; a successful HTTP status alone is not a completed download.
    #[farhelm_testtrace::test]
    async fn failed_browser_body_is_not_successful_eof() {
        let (client, server) = tokio::io::duplex(65536);
        let peer = tokio::spawn(async move {
            let mut peer = FakeSupervisor::accept(server).await;
            reply_download(&mut peer, true).await;
        });
        let h = spliced_helm_listing(client, vec![session("sess-1", 1)]).await;
        let response = h
            .router()
            .oneshot(request("download", false))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            axum::body::to_bytes(response.into_body(), 4096)
                .await
                .is_err()
        );
        peer.await.unwrap();
    }

    /// Both file operations require device auth. A server helm must refuse a
    /// native-save request even with valid credentials and a routable session.
    #[farhelm_testtrace::test]
    async fn auth_and_native_capability_are_enforced_before_dispatch() {
        let h = crate::rest_harness::idle_helm().await;
        for operation in ["stat", "download"] {
            assert_eq!(
                h.unauthenticated_router()
                    .oneshot(request(operation, false))
                    .await
                    .unwrap()
                    .status(),
                StatusCode::UNAUTHORIZED
            );
        }
        assert_eq!(
            h.router()
                .oneshot(request("download", true))
                .await
                .unwrap()
                .status(),
            StatusCode::NOT_FOUND
        );
        let mut missing = request("stat", false);
        *missing.uri_mut() = "/api/sessions/missing/files/stat".parse().unwrap();
        assert_eq!(
            h.router().oneshot(missing).await.unwrap().status(),
            StatusCode::NOT_FOUND
        );
    }

    /// Save twice without overwriting the earlier file, then fail after bytes
    /// arrive. Only successful EOF may publish, and staging must be cleaned.
    #[farhelm_testtrace::test]
    async fn desktop_save_uses_unique_names_and_cleans_a_failed_transfer() {
        let downloads = tempfile::tempdir().unwrap();
        std::fs::write(downloads.path().join("report.pdf"), b"existing").unwrap();
        assert_eq!(
            std::fs::read(downloads.path().join("report.pdf")).unwrap(),
            b"existing"
        );
        for failure in [false, false, true] {
            let (client, server) = tokio::io::duplex(65536);
            let peer = tokio::spawn(async move {
                let mut peer = FakeSupervisor::accept(server).await;
                reply_download(&mut peer, failure).await;
            });
            let mut h = spliced_helm_listing(client, vec![session("sess-1", 1)]).await;
            Arc::get_mut(&mut h.state).unwrap().downloads_dir = Some(downloads.path().into());
            let response = h.router().oneshot(request("download", true)).await.unwrap();
            if failure {
                assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
            } else {
                assert_eq!(response.status(), StatusCode::OK);
                let body: serde_json::Value = serde_json::from_slice(
                    &axum::body::to_bytes(response.into_body(), 4096)
                        .await
                        .unwrap(),
                )
                .unwrap();
                assert_eq!(
                    std::fs::read(body["path"].as_str().unwrap()).unwrap(),
                    b"report-body"
                );
            }
            peer.await.unwrap();
        }
        let mut names: Vec<_> = std::fs::read_dir(downloads.path())
            .unwrap()
            .map(|row| row.unwrap().file_name())
            .collect();
        names.sort();
        assert_eq!(
            names,
            ["report (1).pdf", "report (2).pdf", "report.pdf"].map(std::ffi::OsString::from)
        );
        assert_eq!(
            std::fs::read(downloads.path().join("report.pdf")).unwrap(),
            b"existing"
        );
    }

    /// Cancellation after staged bytes have reached the receiver must abort
    /// upstream and remove the private staging directory, with no final file.
    #[farhelm_testtrace::test]
    async fn cancelled_desktop_save_removes_staging_and_aborts_the_host() {
        let downloads = tempfile::tempdir().unwrap();
        let (client, server) = tokio::io::duplex(65536);
        let (entered, ready) = tokio::sync::oneshot::channel();
        let peer = tokio::spawn(async move {
            let mut peer = FakeSupervisor::accept(server).await;
            let ControlMsg::BeginDownload {
                req_id, channel, ..
            } = peer.recv().await
            else {
                panic!("expected begin");
            };
            peer.send(&ControlMsg::DownloadStarted {
                req_id,
                channel,
                info: info(),
            })
            .await;
            peer.send_frame(&Frame::data(channel, b"partial".to_vec()))
                .await;
            assert!(
                matches!(peer.recv().await, ControlMsg::DownloadAck { channel: ack, received: 7 } if ack == channel)
            );
            entered.send(()).unwrap();
            assert!(
                matches!(peer.recv().await, ControlMsg::AbortDownload { channel: aborted } if aborted == channel)
            );
        });
        let mut h = spliced_helm_listing(client, vec![session("sess-1", 1)]).await;
        Arc::get_mut(&mut h.state).unwrap().downloads_dir = Some(downloads.path().into());
        let handler = tokio::spawn(h.router().oneshot(request("download", true)));
        tokio::time::timeout(std::time::Duration::from_secs(5), ready)
            .await
            .unwrap()
            .unwrap();
        // Ack proves the native consumer is active; its private directory is
        // already owned, but a blocking chunk write may still be finishing.
        use std::os::unix::fs::PermissionsExt;
        let entries: Vec<_> = std::fs::read_dir(downloads.path())
            .unwrap()
            .map(|entry| entry.unwrap())
            .collect();
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].metadata().unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert!(!downloads.path().join("report.pdf").exists());
        handler.abort();
        assert!(handler.await.unwrap_err().is_cancelled());
        tokio::time::timeout(std::time::Duration::from_secs(5), peer)
            .await
            .unwrap()
            .unwrap();
        await_download_cleanup(downloads.path()).await;
    }

    /// Cancellation schedules blocking cleanup rather than performing it on the
    /// async worker. Directory disappearance is the completion oracle; abort
    /// acknowledgement by itself says nothing about that independent work.
    async fn await_download_cleanup(directory: &Path) {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                if std::fs::read_dir(directory).unwrap().next().is_none() {
                    return;
                }
                // sleep-ok: poll the owned staging-directory removal; upstream abort is not a cleanup oracle.
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("cancelled download staging did not disappear");
    }

    /// Saving is not a webview Blob operation. Embedded preflights are public,
    /// but actual native saves still require a credential, with readable CORS.
    #[farhelm_testtrace::test]
    async fn desktop_file_cors_wraps_auth_and_preflight() {
        let h = crate::rest_harness::idle_helm().await.embedded();
        for operation in ["stat", "download"] {
            let mut req = request(operation, false);
            req.headers_mut()
                .insert("origin", "dioxus://index.html".parse().unwrap());
            let response = h.unauthenticated_router().oneshot(req).await.unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
            assert_eq!(
                response.headers()["access-control-allow-origin"],
                "dioxus://index.html"
            );
            let req = Request::builder()
                .method("OPTIONS")
                .uri(format!("/api/sessions/sess-1/files/{operation}"))
                .header("host", "127.0.0.1:7433")
                .header("origin", "dioxus://index.html")
                .header("access-control-request-method", "POST")
                .header(
                    "access-control-request-headers",
                    "authorization,content-type",
                )
                .body(Body::empty())
                .unwrap();
            let response = h.unauthenticated_router().oneshot(req).await.unwrap();
            assert_eq!(response.status(), StatusCode::NO_CONTENT);
            assert_eq!(
                response.headers()["access-control-allow-origin"],
                "dioxus://index.html"
            );
        }
    }
}
