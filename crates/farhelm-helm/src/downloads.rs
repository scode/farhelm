//! Authenticated file reads routed to the host that owns the session.
//! Browser bodies stream through the helm; only the browser buffers a Blob.
//! The embedded desktop helm instead stages in Downloads and publishes only
//! after EOF. An explicit open request may dispatch only that completed save,
//! under one shared extension policy. Neither a hover nor a previous successful
//! read reserves a file.

use crate::sessions::{resolve_session_ref, route_session};
use crate::{AppState, DownloadGuard, http_error};
use axum::extract::{Path as AxPath, State};
use axum::response::IntoResponse;
use axum::{Json, body::Body};
use farhelm_proto::DownloadFileInfo;
use farhelm_proto::open_extensions::OpenExtensions;
use farhelm_supervisor::files::{RealFs, StagedStream};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};

/// Weak owners bound tracking to live saves without delaying their ordinary cleanup.
/// None closes admission before Quit removes directories; creation holds the same lock.
struct NativeStaging(Mutex<Option<Vec<Weak<tempfile::TempDir>>>>);
static NATIVE_STAGING: NativeStaging = NativeStaging(Mutex::new(Some(Vec::new())));

impl NativeStaging {
    /// Remove this process's partial saves even though hard exit skips their destructors.
    /// No transfer is awaited, and a completed publication outside staging survives.
    fn cleanup(&self) {
        for directory in self.0.lock().unwrap().take().unwrap_or_default() {
            if let Some(directory) = directory.upgrade()
                && let Err(error) = std::fs::remove_dir_all(directory.path())
                && error.kind() != std::io::ErrorKind::NotFound
            {
                tracing::warn!(%error, "could not remove download staging on quit");
            }
        }
    }
}

/// Call from the desktop's LoopDestroyed event, before the event loop exits.
/// Crashes and force-quits cannot run this process-local cleanup.
pub fn cleanup_native_downloads() {
    NATIVE_STAGING.cleanup();
}

/// Open only the completed save supplied by the helm, never a caller's path.
/// The native shell owns platform dispatch; tests inject an observer instead.
pub type FileOpener = Arc<dyn Fn(&Path) -> Result<(), String> + Send + Sync>;

/// The desktop's save directory and local-file dispatch travel together.
/// A server helm supplies neither, since its clients may be on other machines.
pub struct NativeDownloads {
    /// Publication and staging share this filesystem for atomic saves.
    pub directory: PathBuf,
    /// OS dispatch runs only after a successful save and extension admission.
    pub opener: FileOpener,
}

/// Admit only a saved file whose final extension is on the helm's current list.
///
/// An earlier hover is never authority to open: the list can change before
/// the click, and the host can change the file before publication.
fn may_open(path: &Path, list: &OpenExtensions) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| list.contains(extension))
}

/// Explain an admission refusal using the effective policy, including off.
fn open_refusal(list: &OpenExtensions) -> String {
    let allowed = list.normalized();
    if allowed.is_empty() {
        "opening downloaded files is turned off in settings".into()
    } else {
        format!("only these downloaded file types can be opened: {allowed}")
    }
}

/// Whether two paths end in the same extension, ASCII case-insensitively.
///
/// Opening is decided on the saved file, but the user chose it from the
/// hover's earlier disclosure of the requested path. The host resolves that
/// path again when the transfer starts, so an agent can swap the checked file
/// for a symlink to a file of another type between hover and click. Requiring
/// the saved file to keep the requested file's type means a click only ever
/// opens the kind of file the hover described. This matters when the list
/// includes a type whose default app can run code: changing a symlink after
/// hover must not silently turn a disclosed document into that file type.
fn same_type(requested: &Path, saved: &Path) -> bool {
    match (
        requested.extension().and_then(|e| e.to_str()),
        saved.extension().and_then(|e| e.to_str()),
    ) {
        (Some(requested), Some(saved)) => requested.eq_ignore_ascii_case(saved),
        _ => false,
    }
}

/// Agent-provided path text is data, never a shell expression. Save mode is
/// an explicit request to the desktop capability; server helms refuse it.
#[derive(Deserialize)]
pub(crate) struct FileRequest {
    path: String,
    #[serde(default)]
    save_to_downloads: bool,
    #[serde(default)]
    /// Opening is meaningful only with native save; it never names a local path.
    open: bool,
}

/// Fresh host evidence with the host's display name for the hover disclosure.
/// Flattening preserves the protocol's typed refusal and optional size.
#[derive(serde::Serialize)]
struct FileStatBody {
    host: String,
    /// Absent on server helms: only native saves can have a local open action.
    #[serde(skip_serializing_if = "Option::is_none")]
    can_open: Option<bool>,
    #[serde(flatten)]
    info: DownloadFileInfo,
}

/// Resolve and inspect without moving bytes. The normal session-owner route
/// supplies the same disconnected-host refusal as other session operations.
/// Public references select the host/session pair here; the supervisor receives only its suffix.
pub(crate) async fn stat_file(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
    Json(request): Json<FileRequest>,
) -> axum::response::Response {
    let result = async {
        let key = resolve_session_ref(&state, &id).await?;
        let (claim, client) = route_session(&state, &key).await?;
        let info = client.stat_file(&key.session_id, &request.path).await?;
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
        let can_open = if state.file_opener.is_some() {
            let preferences = state.store.preferences().await?;
            let list = OpenExtensions::from_stored(preferences.open_extensions.as_deref());
            Some(
                info.status == farhelm_proto::DownloadFileStatus::Ready
                    && list.is_some_and(|list| may_open(Path::new(&info.path), &list)),
            )
        } else {
            None
        };
        Ok::<_, anyhow::Error>(FileStatBody {
            host,
            can_open,
            info,
        })
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
    if request.open && (!request.save_to_downloads || state.file_opener.is_none()) {
        return (
            axum::http::StatusCode::NOT_FOUND,
            "native file opening is unavailable\n",
        )
            .into_response();
    }
    if request.save_to_downloads && state.downloads_dir.is_none() {
        return (
            axum::http::StatusCode::NOT_FOUND,
            "native Downloads save is unavailable\n",
        )
            .into_response();
    }
    let result = async {
        let key = resolve_session_ref(&state, &id).await?;
        let (_, client) = route_session(&state, &key).await?;
        client.begin_download(&key.session_id, &request.path).await
    }
    .await;
    let download = match result {
        Ok(download) => download,
        Err(error) => return http_error(error),
    };
    if request.save_to_downloads {
        return match save_download(download, state.downloads_dir.clone().unwrap()).await {
            Ok(path) => {
                // Publication is already successful. An opener refusal or OS
                // error must never tell the user that this saved file was lost.
                let mut body = serde_json::json!({"path": path.to_string_lossy()});
                if request.open {
                    // A preference read failure is an opening failure, not a
                    // download failure: publication has already succeeded.
                    let policy = state.store.preferences().await.map(|preferences| {
                        OpenExtensions::from_stored(preferences.open_extensions.as_deref())
                    });
                    let result = match policy {
                        Err(error) => Err(format!("could not read opening settings: {error}")),
                        Ok(None) => Err("opening settings are invalid; no file types can be opened".into()),
                        Ok(Some(list)) if !may_open(&path, &list) => Err(open_refusal(&list)),
                        Ok(Some(_)) if !same_type(Path::new(&request.path), &path) => {
                            Err("the file changed type after it was checked, so it was saved but not opened".into())
                        }
                        Ok(Some(_)) => {
                            let opener = state.file_opener.clone().unwrap();
                            tokio::task::spawn_blocking(move || opener(&path))
                                .await
                                .unwrap_or_else(|error| Err(format!("file opener failed: {error}")))
                        }
                    };
                    body["opened"] = serde_json::json!(result.is_ok());
                    if let Err(reason) = result {
                        body["open_error"] = serde_json::json!(reason);
                    }
                }
                Json(body).into_response()
            }
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
/// Numbered copies shorten only the stem to fit a 255-byte component, at a
/// UTF-8 boundary. The original name is untouched; an extension that leaves no
/// room for a stem keeps the existing filesystem refusal rather than changing
/// the file's type to make it fit.
fn candidates(name: String) -> impl Iterator<Item = String> {
    let path = Path::new(&name);
    let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
    let extension = path
        .extension()
        .map(|ext| format!(".{}", ext.to_string_lossy()))
        .unwrap_or_default();
    std::iter::once(name).chain((1u64..).map(move |n| {
        let suffix = format!(" ({n}){extension}");
        let shortened = match 255usize.checked_sub(suffix.len()) {
            Some(limit) if limit > 0 => {
                let mut end = stem.len().min(limit);
                while !stem.is_char_boundary(end) {
                    end -= 1;
                }
                &stem[..end]
            }
            _ => &stem,
        };
        format!("{shortened}{suffix}")
    }))
}

/// One owner keeps the stream and its private directory together through
/// blocking work. File teardown precedes directory teardown, including when
/// an async save is cancelled while a chunk write is still in progress.
/// Quit may briefly upgrade the directory's weak registration to remove it.
struct SaveFiles {
    file: StagedStream,
    directory: Arc<tempfile::TempDir>,
}

/// Async ownership must not turn a failed transfer into synchronous filesystem
/// cleanup on a Tokio worker. Drop schedules that cleanup on the blocking pool;
/// reported receive failures await the same cleanup before answering.
struct NativeSave(Option<SaveFiles>);

impl NativeSave {
    /// Request private permissions at creation rather than relying on umask.
    /// The temp directory shares the publication filesystem for atomic links.
    fn create(directory: PathBuf, registry: &NativeStaging) -> anyhow::Result<Self> {
        use std::os::unix::fs::PermissionsExt;
        let mut owners = registry.0.lock().unwrap();
        let owners = owners
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("the app is quitting"))?;
        std::fs::create_dir_all(&directory)?;
        let staging = Arc::new(
            tempfile::Builder::new()
                .prefix(".farhelm-download-")
                .permissions(std::fs::Permissions::from_mode(0o700))
                .tempdir_in(&directory)?,
        );
        owners.retain(|owner| owner.strong_count() > 0);
        owners.push(Arc::downgrade(&staging));
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
            let directory = std::fs::remove_dir_all(files.directory.path());
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
    let mut save =
        tokio::task::spawn_blocking(move || NativeSave::create(directory, &NATIVE_STAGING))
            .await??;
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
    /// Quit removes live partial bytes without waiting for Drop, closes admission,
    /// and leaves completed saves and another owner's staging untouched.
    #[farhelm_testtrace::test]
    async fn quit_cleans_only_owned_staging_and_refuses_new_saves() {
        let downloads = tempfile::tempdir().unwrap();
        let registry = NativeStaging(Mutex::new(Some(Vec::new())));
        let other = NativeStaging(Mutex::new(Some(Vec::new())));
        let create = |registry| NativeSave::create(downloads.path().into(), registry).unwrap();
        let save = create(&registry).write_chunk(b"partial".to_vec()).unwrap();
        let staging = save.0.as_ref().unwrap().directory.path().to_path_buf();
        let partial = std::fs::read_dir(&staging)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(std::fs::read(partial).unwrap(), b"partial");
        let foreign = create(&other).write_chunk(b"other".to_vec()).unwrap();
        let foreign_path = foreign.0.as_ref().unwrap().directory.path().to_path_buf();
        let published = create(&registry)
            .write_chunk(b"complete".to_vec())
            .unwrap()
            .publish("saved.txt".into())
            .unwrap();
        registry.cleanup();
        assert!(!staging.exists());
        assert!(foreign_path.exists());
        assert_eq!(std::fs::read(published).unwrap(), b"complete");
        assert!(NativeSave::create(downloads.path().join("new"), &registry).is_err());
        assert!(!downloads.path().join("new").exists());
        foreign.cleanup().await.unwrap();
    }

    /// Build this fixture's public reference from its registered entry, keeping wire IDs bare.
    fn session_path(harness: &crate::rest_harness::Harness, path: &str) -> String {
        let prefix = "/api/sessions/";
        let Some(suffix) = path.strip_prefix(prefix) else {
            return path.to_string();
        };
        let entry = harness
            .manager
            .snapshots()
            .into_iter()
            .find(|host| host.kind == crate::store::HostKind::Local)
            .expect("the fixture has a local host")
            .entry_uuid;
        format!("{prefix}{entry}:{suffix}")
    }

    use super::*;
    use crate::rest_harness::{FakeSupervisor, session, spliced_helm_listing};
    use axum::http::{Request, StatusCode};
    use farhelm_proto::{ControlMsg, DownloadFileStatus, Frame};
    use tower::ServiceExt;

    /// Collision numbering must keep ordinary names and the final extension
    /// stable, including compound extensions and leading-dot names.
    #[farhelm_testtrace::test]
    fn numbered_names_keep_the_existing_convention() {
        for (name, first, second) in [
            ("report.pdf", "report (1).pdf", "report (2).pdf"),
            ("archive.tar.gz", "archive.tar (1).gz", "archive.tar (2).gz"),
            (".env", ".env (1)", ".env (2)"),
        ] {
            assert_eq!(
                candidates(name.into()).take(3).collect::<Vec<_>>(),
                [name, first, second]
            );
        }
    }

    /// A real no-clobber publication must save a second copy even when the
    /// original leaf fills the byte limit. Multibyte stems must stay valid UTF-8
    /// and existing bytes must survive; candidate generation alone proves neither.
    #[farhelm_testtrace::test]
    fn long_numbered_names_publish_without_overwriting_the_original() {
        for (stem, shortened) in [
            ("a".repeat(251), "a".repeat(247)),
            (format!("{}a", "é".repeat(125)), "é".repeat(123)),
        ] {
            let downloads = tempfile::tempdir().unwrap();
            let name = format!("{stem}.pdf");
            let original = downloads.path().join(&name);
            assert_eq!(
                name.len(),
                255,
                "fixture starts at the component byte limit"
            );
            std::fs::write(&original, b"existing").unwrap();
            assert_eq!(std::fs::read(&original).unwrap(), b"existing");
            assert_eq!(candidates(name.clone()).next().unwrap(), name);
            let staging = tempfile::tempdir_in(downloads.path()).unwrap();
            let mut stream =
                StagedStream::create(staging.path(), downloads.path(), "download", &RealFs)
                    .unwrap();
            stream.write_chunk(&RealFs, b"second copy").unwrap();
            let saved = stream
                .publish_no_clobber(&RealFs, candidates(name))
                .unwrap();
            assert_eq!(saved, downloads.path().join(format!("{shortened} (1).pdf")));
            assert!(saved.file_name().unwrap().to_str().unwrap().len() <= 255);
            assert_eq!(std::fs::read(saved).unwrap(), b"second copy");
            assert_eq!(std::fs::read(original).unwrap(), b"existing");
        }
    }

    /// More collision digits consume the same fixed component budget. An
    /// extension that leaves no stem space is not silently shortened or dropped.
    #[farhelm_testtrace::test]
    fn numbered_names_budget_for_digits_without_changing_oversized_extensions() {
        let name = format!("{}.pdf", "a".repeat(251));
        assert_eq!(
            candidates(name).nth(10).unwrap(),
            format!("{} (10).pdf", "a".repeat(246))
        );
        for extension in ["x".repeat(250), "x".repeat(251)] {
            let name = format!("a.{extension}");
            assert_eq!(
                candidates(name).nth(1).unwrap(),
                format!("a (1).{extension}")
            );
        }
    }

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

    /// A stale browser's bare ID must fail locally, even while an identically named session
    /// is routable. The silent peer distinguishes refusal from supervisor-side validation.
    #[farhelm_testtrace::test]
    async fn bare_file_targets_are_refused_before_supervisor_dispatch() {
        let (client, server) = tokio::io::duplex(64 * 1024);
        let (observe, peer) = crate::rest_harness::silent_supervisor(server);
        let h = spliced_helm_listing(client, vec![session("sess-1", 1)]).await;
        let host = h
            .manager
            .snapshots()
            .into_iter()
            .find(|host| host.kind == crate::store::HostKind::Local)
            .unwrap()
            .id;
        crate::sessions::route_session(&h.state, &crate::store::SessionKey::new(host, "sess-1"))
            .await
            .expect("premise: the matching bare suffix is routable");
        for operation in ["stat", "download"] {
            let response = h.router().oneshot(request(operation, false)).await.unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            let body = axum::body::to_bytes(response.into_body(), 4096)
                .await
                .unwrap();
            assert!(
                String::from_utf8(body.to_vec())
                    .unwrap()
                    .contains("full session ID")
            );
        }
        observe.send(()).unwrap();
        peer.await.unwrap();
    }

    /// Qualify only this fixture's selected request, leaving raw builders available for refusals.
    fn qualified(
        harness: &crate::rest_harness::Harness,
        mut request: Request<Body>,
    ) -> Request<Body> {
        *request.uri_mut() = session_path(harness, &request.uri().to_string())
            .parse()
            .unwrap();
        request
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
        reply_download_with_info(peer, failure, "out/report.pdf", info()).await;
    }

    /// A host's admitted basename, rather than the caller's spelling, controls
    /// native publication. Vary it to exercise opening after name allocation.
    /// `requested` is the path the helm must forward unchanged to the host.
    async fn reply_download_with_info(
        peer: &mut FakeSupervisor,
        failure: bool,
        requested: &str,
        info: DownloadFileInfo,
    ) {
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
        assert_eq!(path, requested);
        peer.send(&ControlMsg::DownloadStarted {
            req_id,
            channel,
            info,
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
        let response = h
            .router()
            .oneshot(qualified(&h, request("stat", false)))
            .await
            .unwrap();
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
            .oneshot(qualified(&h, request("download", false)))
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
            .oneshot(qualified(&h, request("download", false)))
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
                    .oneshot(qualified(&h, request(operation, false)))
                    .await
                    .unwrap()
                    .status(),
                StatusCode::UNAUTHORIZED
            );
        }
        assert_eq!(
            h.router()
                .oneshot(qualified(&h, request("download", true)))
                .await
                .unwrap()
                .status(),
            StatusCode::NOT_FOUND
        );
        let mut missing = request("stat", false);
        *missing.uri_mut() = session_path(&h, "/api/sessions/missing/files/stat")
            .parse()
            .unwrap();
        assert_eq!(
            h.router().oneshot(missing).await.unwrap().status(),
            StatusCode::NOT_FOUND
        );
    }

    /// Only the final extension controls admission; a user-added type is
    /// effective immediately, while an empty or invalid stored list opens none.
    #[farhelm_testtrace::test]
    fn only_listed_final_extensions_may_open() {
        let default = OpenExtensions::from_stored(None).unwrap();
        for name in ["notes.md", "notes.TXT", "archive.tar.md", "data.JSON"] {
            assert!(may_open(Path::new(name), &default), "{name}");
        }
        for name in [
            "notes",
            ".md",
            "notes.txt.command",
            "notes.md.exe",
            "md/report",
            "notes.txt.",
            "report.pdf",
            "notes.htm",
            "report.html",
            "report.HtMl",
        ] {
            assert!(!may_open(Path::new(name), &default), "{name}");
        }
        let widened = OpenExtensions::from_stored(Some("json, html, command")).unwrap();
        assert!(may_open(Path::new("report.HTML"), &widened));
        assert!(may_open(Path::new("notes.command"), &widened));
        assert!(!may_open(Path::new("notes.md"), &widened));
        assert!(!may_open(
            Path::new("notes.md"),
            &OpenExtensions::from_stored(Some("")).unwrap()
        ));
        assert!(OpenExtensions::from_stored(Some("tar.gz")).is_none());
    }

    /// Even a valid device cannot ask a server to open a local path. A missing
    /// session makes the pre-dispatch error distinguishable from routing: no
    /// lookup or transfer may precede this capability check.
    #[farhelm_testtrace::test]
    async fn open_is_refused_before_transfer_without_native_save_and_opener() {
        let mut h = crate::rest_harness::idle_helm().await;
        for save in [false, true] {
            let response = h
                .router()
                .oneshot(qualified(&h, open_request(save)))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
            assert_eq!(
                axum::body::to_bytes(response.into_body(), 4096)
                    .await
                    .unwrap(),
                "native file opening is unavailable\n"
            );
        }
        // An opener alone still cannot authorize opening without a native save.
        Arc::get_mut(&mut h.state).unwrap().file_opener =
            Some(Arc::new(|_| panic!("no saved file exists")));
        let response = h
            .router()
            .oneshot(qualified(&h, open_request(false)))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            axum::body::to_bytes(response.into_body(), 4096)
                .await
                .unwrap(),
            "native file opening is unavailable\n"
        );
    }

    /// The API accepts only the remote path as input, never a local open path.
    /// Keep construction separate so server and embedded tests exercise the
    /// same extractor and authentication boundary as an actual modifier-click.
    fn open_request(save: bool) -> Request<Body> {
        open_request_for(save, "out/report.pdf")
    }

    /// The same request naming a specific requested path, so a test can make
    /// the host's resolved file agree or disagree with what was checked.
    fn open_request_for(save: bool, path: &str) -> Request<Body> {
        let mut req = request("download", save);
        *req.body_mut() = Body::from(
            serde_json::json!({
                "path": path, "save_to_downloads": save, "open": true
            })
            .to_string(),
        );
        req
    }

    /// Successful saves stay successful after an open refusal or OS failure.
    /// The observer sees only the completed, uniquely named private save, which
    /// proves neither request text nor the host's absolute path reaches it.
    #[farhelm_testtrace::test]
    async fn native_open_uses_only_published_allowlisted_save_and_preserves_failures() {
        // The fourth case is a swap between hover and click: the user chose
        // a text file, and the host now resolves it to a Markdown file. Both
        // types are allowed, but it must be saved and not opened, because
        // the user never saw the type it would open as. HTML remains unlisted
        // in the default policy, while JSON now opens by default.
        for (requested, name, policy, allowed, fail) in [
            ("out/notes.TXT", "notes.TXT", None, true, false),
            ("out/notes.md", "notes.md", None, true, true),
            ("out/data.json", "data.json", None, true, false),
            (
                "out/notes.txt.command",
                "notes.txt.command",
                None,
                false,
                false,
            ),
            ("out/notes.txt", "payload.md", None, false, false),
            ("out/report.html", "report.html", None, false, false),
            ("out/report.html", "report.html", Some("html"), true, false),
            ("out/notes.md", "notes.md", Some("json"), false, false),
            ("out/notes.md", "notes.md", Some(""), false, false),
            ("out/notes.md", "notes.md", Some("tar.gz"), false, false),
        ] {
            let downloads = tempfile::tempdir().unwrap();
            std::fs::write(downloads.path().join(name), b"existing").unwrap();
            let (client, server) = tokio::io::duplex(65536);
            let peer = tokio::spawn(async move {
                let mut peer = FakeSupervisor::accept(server).await;
                let mut answer = info();
                answer.path = format!("/host-only/{name}");
                reply_download_with_info(&mut peer, false, requested, answer).await;
            });
            let observed = Arc::new(std::sync::Mutex::new(Vec::new()));
            let calls = observed.clone();
            let directory = downloads.path().to_path_buf();
            let mut h = spliced_helm_listing(client, vec![session("sess-1", 1)]).await;
            if let Some(policy) = policy {
                h.state
                    .store
                    .update_preferences(crate::store::PreferencePatch {
                        open_extensions: Some(Some(policy.into())),
                        ..Default::default()
                    })
                    .await
                    .unwrap();
            }
            let state = Arc::get_mut(&mut h.state).unwrap();
            state.downloads_dir = Some(directory.clone());
            state.file_opener = Some(Arc::new(move |saved| {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(saved.parent(), Some(directory.as_path()));
                assert_eq!(std::fs::read(saved).unwrap(), b"report-body");
                assert_eq!(
                    std::fs::metadata(saved).unwrap().permissions().mode() & 0o777,
                    0o600
                );
                calls.lock().unwrap().push(saved.to_path_buf());
                if fail {
                    Err("opener sentinel".into())
                } else {
                    Ok(())
                }
            }));
            let response = h
                .router()
                .oneshot(qualified(&h, open_request_for(true, requested)))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let body: serde_json::Value = serde_json::from_slice(
                &axum::body::to_bytes(response.into_body(), 4096)
                    .await
                    .unwrap(),
            )
            .unwrap();
            let saved = Path::new(body["path"].as_str().unwrap());
            assert_eq!(std::fs::read(saved).unwrap(), b"report-body");
            assert_eq!(
                std::fs::read(downloads.path().join(name)).unwrap(),
                b"existing"
            );
            assert_ne!(saved, downloads.path().join(name));
            assert_eq!(body["opened"], allowed && !fail);
            let calls = observed.lock().unwrap().clone();
            if allowed {
                assert_eq!(&*calls, &[saved.to_path_buf()]);
            } else {
                assert!(calls.is_empty());
                let reason = body["open_error"].as_str().unwrap();
                if name == "payload.md" {
                    assert!(reason.contains("changed type"), "{reason}");
                } else if policy == Some("") {
                    assert!(reason.contains("turned off"), "{reason}");
                } else if policy == Some("json") {
                    assert!(reason.contains("json"), "{reason}");
                } else if policy == Some("tar.gz") {
                    assert!(reason.contains("invalid"), "{reason}");
                } else {
                    assert!(reason.contains("md, txt, json"), "{reason}");
                }
            }
            if fail {
                assert_eq!(body["open_error"], "opener sentinel");
            }
            peer.await.unwrap();
        }
    }

    /// The hover's capability answer uses the same extension rule as opening,
    /// and is absent on servers. Only the desktop's own window can reach the
    /// embedded helm, so a server helm's browsers never see an opening hint.
    #[farhelm_testtrace::test]
    async fn stat_exposes_open_hint_only_with_native_capability() {
        for (native, name, policy, expected) in [
            (false, "notes.md", None, None),
            (true, "notes.TXT", None, Some(true)),
            (true, "data.JSON", None, Some(true)),
            (true, "notes.txt.command", None, Some(false)),
            (true, "notes.md", Some("json"), Some(false)),
            (true, "notes.md", Some(""), Some(false)),
            (true, "notes.md", Some("tar.gz"), Some(false)),
        ] {
            let (client, server) = tokio::io::duplex(65536);
            let peer = tokio::spawn(async move {
                let mut peer = FakeSupervisor::accept(server).await;
                let ControlMsg::StatFile { req_id, .. } = peer.recv().await else {
                    panic!("expected stat");
                };
                let mut answer = info();
                answer.path = format!("/work/{name}");
                peer.send(&ControlMsg::FileStat {
                    req_id,
                    info: answer,
                })
                .await;
            });
            let mut h = spliced_helm_listing(client, vec![session("sess-1", 1)]).await;
            if let Some(policy) = policy {
                h.state
                    .store
                    .update_preferences(crate::store::PreferencePatch {
                        open_extensions: Some(Some(policy.into())),
                        ..Default::default()
                    })
                    .await
                    .unwrap();
            }
            if native {
                Arc::get_mut(&mut h.state).unwrap().file_opener =
                    Some(Arc::new(|_| panic!("stat never opens")));
            }
            let response = h
                .router()
                .oneshot(qualified(&h, request("stat", false)))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let body: serde_json::Value = serde_json::from_slice(
                &axum::body::to_bytes(response.into_body(), 4096)
                    .await
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(
                body.get("can_open").and_then(|value| value.as_bool()),
                expected
            );
            peer.await.unwrap();
        }
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
            let response = h
                .router()
                .oneshot(qualified(&h, request("download", true)))
                .await
                .unwrap();
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
        let handler = tokio::spawn(h.router().oneshot(qualified(&h, request("download", true))));
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
