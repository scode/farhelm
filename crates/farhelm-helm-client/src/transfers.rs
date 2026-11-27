//! Plain-HTTP file transfers to the helm: the hover check of a file path a session printed (stat), saving that file
//! into the user's Downloads (download), reading a small file's contents into memory for the clipboard (download
//! bytes), and attaching a local file or pasted bytes to a session (upload).
//!
//! NOTE: None of this travels on the UI channel. Downloads and uploads are bulk data, which SPEC_impl.md, "Bulk data
//! stays separate", keeps off it: each transfer is its own HTTP request, so a large upload never delays a click, and a
//! failed transfer is that transfer's own outcome, which the user retries. Stat is small, and the channel could carry
//! it, but a channel command cannot be cancelled on its own. A hover check sent there would keep running after the
//! pointer left the link, counting against the channel's watchdog all the while, and SPEC.md says leaving cancels the
//! check. As an HTTP request it is cancelled by dropping its [`Transfer`] (SPEC_impl.md, "The UI channel").
//!
//! Every request goes to the loopback [`HelmEndpoint`] the UI channel uses, with the device credential as a bearer
//! token. Saving and opening happen in the helm: the desktop app hands its embedded helm the Downloads directory and a
//! file opener, so a download here only asks for the save and reports where the file went, and none of its bytes pass
//! through this process. Two kinds of transfer do carry bytes here. A download of bytes asks the same route to relay
//! the file instead of saving it, and stops reading at the caller's cap, so that copying a file into the clipboard
//! never touches Downloads. An upload's file is read from disk and streamed on this crate's runtime, because the GUI
//! may neither block nor start threads.
//!
//! [`Transfers`] is the GUI's handle. Each call returns a [`Transfer`], a future any executor can poll. Dropping it
//! abandons the request and closes its connection, and the helm then ends the transfer on its side: an unfinished
//! upload is aborted and an unfinished save is removed. A transfer dropped after its request went out whole may still
//! complete on the helm's side, though. An upload may be published: SPEC.md, "Attachments", says cancelling during
//! final publication can leave a complete attachment with no path acknowledged, so the GUI must treat a dropped
//! upload's outcome as unknown, not as undone. Likewise a download dropped after the helm published its save leaves a
//! file in Downloads that the GUI never hears about.
//!
//! There are no deadlines here, and nothing ties a transfer to the UI channel's reset: the channel's watchdog is the
//! only per-request deadline the GUI has (SPEC_impl.md, "One failure path: the reset"), and a download has no progress
//! timer (SPEC_impl.md, "Session-host file reads"). A transfer to a stalled host stays pending until the GUI drops it,
//! so abandoning one (when the pointer leaves, a terminal closes, or the channel resets) is the GUI's call.
//!
//! [`TransferError`] keeps apart the facts the GUI's wording needs: the helm answered with a refusal, the request never
//! reached the helm, or the request went out and its answer was lost. For an upload,
//! [`TransferError::upload_outcome`] turns those into the distinction SPEC.md, "Attachments", requires every failure
//! to state: definitely not published, or publication unknown.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll};

use farhelm_proto::file_transfers::{DownloadReply, FileRequest, UploadReply};
use farhelm_proto::ui_channel::ApiPath;
use futures_util::Stream;
use serde::de::DeserializeOwned;
use tokio::io::AsyncReadExt;
use tokio::sync::{mpsc, oneshot};

use crate::ws::HelmEndpoint;

/// How much of an upload is read from disk at a time. The helm rechunks the body for the supervisor anyway, so this
/// only bounds the memory one upload holds here.
const UPLOAD_READ_BYTES: usize = 64 * 1024;

/// The most of a helm answer this reads. Every answer here is a small JSON object or a line of text, so a bigger one is
/// a broken peer, the same stance the UI channel's message cap takes.
const ANSWER_MAX_BYTES: usize = 4 * 1024 * 1024;

/// What the GUI shows for a refused credential. The helm's 401 body is a JSON object meant for code, not words for a
/// person, so it is replaced; [`TransferError::Refused`] keeps the 401 status for the GUI to branch on.
const CREDENTIAL_REFUSED: &str = "the helm did not accept this app's credential";

// ---------------------------------------------------------------------------
// What the GUI sees
// ---------------------------------------------------------------------------

/// The helm's answer to a hover check, as the helm itself writes it (the wire type is shared, see "The routes' wire
/// shapes" below). A refusal such as a folder or an oversized file is data here, not a [`TransferError`]: the request
/// succeeded and the host said no.
pub use farhelm_proto::file_transfers::FileStat;

/// A completed save into Downloads: where the file is, and what came of the request to open it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedDownload {
    /// The saved file's local path, under a no-clobber name (`report (1).pdf`) when the plain one was taken.
    pub path: String,
    pub open: OpenOutcome,
}

/// Opening never turns a save into a failure: the helm has already published the file when it decides this.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenOutcome {
    /// The download did not ask to open the file.
    NotAsked,
    Opened,
    /// The helm refused to open it or the opener failed, in the helm's words (the type is not on the list, say).
    NotOpened(String),
}

/// Why a transfer did not produce its answer. The variants are facts about how far the request got, which is what
/// decides an upload's [`UploadOutcome`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TransferError {
    /// The helm answered with an error status. `message` is its body, trimmed: the helm's (or supervisor's) own
    /// words, which the GUI shows. It is empty when the helm sent none or the body was lost after the status. For a
    /// 401 (a refused credential) it is a fixed sentence instead, since that body is JSON for code.
    #[error("the helm refused with HTTP {status}: {message}")]
    Refused { status: u16, message: String },
    /// The request never reached the helm: nothing was listening, or it could not be built.
    #[error("the request did not reach the helm: {0}")]
    NotSent(String),
    /// The request went out and no complete answer came back: the connection died, or this crate's runtime stopped.
    #[error("the connection to the helm was lost: {0}")]
    Lost(String),
    /// The helm answered success with a body that is not the route's answer.
    #[error("the helm's answer could not be read: {0}")]
    BadReply(String),
    /// A download of bytes ([`Transfers::download_bytes`]) found the file longer than the caller's cap, and stopped
    /// reading there. Kept apart from [`TransferError::BadReply`] because nothing is broken: the file is simply too
    /// big for what the caller wants it for, and the GUI says so in those words.
    #[error("the file is larger than {max_bytes} bytes")]
    TooLarge { max_bytes: usize },
    /// An upload's local file could not be opened or read, or changed size while it was read. Its bytes never all
    /// reached the helm.
    #[error("the file could not be read: {0}")]
    File(String),
}

/// What a failed upload says about the attachment, as SPEC.md, "Attachments", requires every failure to state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadOutcome {
    NotPublished,
    Unknown,
}

impl TransferError {
    /// Whether an upload that failed this way may still have published its attachment on the host.
    ///
    /// The helm publishes only after the whole body arrived and the supervisor committed it, so a request that never
    /// arrived or whose body was cut short ([`TransferError::NotSent`], [`TransferError::File`]) published nothing. A
    /// lost answer or an unreadable success may follow a publication. A 4xx is the helm refusing the request itself
    /// (no session, no `Content-Length`, a size the supervisor rejected at commit), so nothing was published. A 5xx is
    /// not safe to read that way: the helm also answers 5xx when its connection to the supervisor dies during the
    /// commit, when the file may already be published. Any other status is not one the helm sends, so it is unknown.
    ///
    /// NOTE: A large upload the helm refuses before reading its body (no session, a disconnected host) may come back
    /// as [`TransferError::Lost`] instead of the 4xx, depending on timing. The helm answers and closes with the body
    /// unread, and hyper may report the failed send rather than the answer. That reads as unknown: truthful, but
    /// without the helm's words.
    ///
    /// [`TransferError::TooLarge`] belongs to downloads of bytes and no upload ends with it. It reads as unknown, the
    /// reading that never promises more than is known.
    pub fn upload_outcome(&self) -> UploadOutcome {
        match self {
            TransferError::NotSent(_) | TransferError::File(_) => UploadOutcome::NotPublished,
            TransferError::Refused { status, .. } if (400..500).contains(status) => {
                UploadOutcome::NotPublished
            }
            TransferError::Refused { .. }
            | TransferError::Lost(_)
            | TransferError::BadReply(_)
            | TransferError::TooLarge { .. } => UploadOutcome::Unknown,
        }
    }
}

/// The GUI's handle on file transfers. Clones share one HTTP client and its connection pool. Get one from
/// [`crate::HelmConnection::transfers`], or from `fake::transfers::FakeTransfers` in GUI tests.
///
/// The GUI owns abandonment: a transfer runs until it is answered or its [`Transfer`] is dropped, with no deadline
/// and no tie to the UI channel, so a GUI that resets after a broken channel drops the transfers it no longer wants
/// (see the module docs).
#[derive(Clone)]
pub struct Transfers {
    requests: mpsc::UnboundedSender<Request>,
}

impl Transfers {
    /// Check `path` as `session`'s host resolves it (against the session's working directory and home), moving no
    /// bytes. `session` is the session's served id, as the sessions collection keys it.
    pub fn stat(&self, session: &str, path: &str) -> Transfer<FileStat> {
        self.send(|reply| Request::Stat {
            session: session.to_owned(),
            path: path.to_owned(),
            reply,
        })
    }

    /// Save `path` from `session`'s host into Downloads, and with `open` also open the saved file with its default
    /// app. The host reopens the file, so this does not rely on an earlier [`Transfers::stat`]. The helm refuses `open`
    /// unless it has a native opener, which only the desktop app's helm has.
    pub fn download(&self, session: &str, path: &str, open: bool) -> Transfer<SavedDownload> {
        self.send(|reply| Request::Download {
            session: session.to_owned(),
            path: path.to_owned(),
            open,
            reply,
        })
    }

    /// Read `path` from `session`'s host into memory, as long as it is at most `max_bytes` long, for a caller that
    /// wants the contents themselves (copying a file into the clipboard) rather than a file in Downloads.
    ///
    /// It is the download route asked to relay instead of save, so the host reopens and checks the file as it does
    /// for a save, and its own download limit still applies. The cap is enforced on the stream as it arrives: reading
    /// stops, and the connection is dropped, at the first chunk that would pass `max_bytes`, which ends the transfer
    /// with [`TransferError::TooLarge`] having held no more than `max_bytes` here. The helm promises no length up front
    /// (the file may change while it is read), so the cap cannot be checked before reading. A host read that fails
    /// after the helm sent its headers ends the body early and comes back as [`TransferError::Lost`].
    pub fn download_bytes(&self, session: &str, path: &str, max_bytes: usize) -> Transfer<Vec<u8>> {
        self.send(|reply| Request::DownloadBytes {
            session: session.to_owned(),
            path: path.to_owned(),
            max_bytes,
            reply,
        })
    }

    /// Attach the local file `file` to `session`, proposing `filename` as its name on the host (the supervisor
    /// sanitizes it, and an empty proposal gets a generated name). Answers the attachment's absolute host path. The
    /// file is read as it is sent; see [`TransferError::File`] for one that changes meanwhile.
    pub fn upload(&self, session: &str, file: PathBuf, filename: &str) -> Transfer<String> {
        self.upload_from(session, UploadSource::File(file), filename)
    }

    /// Attach `bytes` to `session`, as [`Transfers::upload`] does a file, with the same outcome rules and the same
    /// cancellation: dropping the transfer mid-body stops the body short. For data that exists only in memory, such as
    /// a pasted image, which the GUI cannot write to a file of its own.
    pub fn upload_bytes(&self, session: &str, bytes: Vec<u8>, filename: &str) -> Transfer<String> {
        self.upload_from(session, UploadSource::Bytes(bytes), filename)
    }

    fn upload_from(&self, session: &str, source: UploadSource, filename: &str) -> Transfer<String> {
        self.send(|reply| Request::Upload {
            session: session.to_owned(),
            source,
            filename: filename.to_owned(),
            reply,
        })
    }

    fn send<T>(&self, request: impl FnOnce(Reply<T>) -> Request) -> Transfer<T> {
        let (reply, answer) = oneshot::channel();
        // If the serving task is gone, the request drops with its reply and the future answers `Lost`.
        let _ = self.requests.send(request(reply));
        Transfer { answer }
    }
}

/// One transfer's answer. Runtime-agnostic; dropping it abandons the transfer (see the module docs).
#[derive(Debug)]
pub struct Transfer<T> {
    answer: oneshot::Receiver<Result<T, TransferError>>,
}

impl<T> Future for Transfer<T> {
    type Output = Result<T, TransferError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.answer).poll(cx).map(|answer| {
            answer.unwrap_or_else(|_| {
                Err(TransferError::Lost(
                    "the helm client stopped before answering".into(),
                ))
            })
        })
    }
}

/// Where an upload's bytes come from.
#[derive(Clone, PartialEq, Eq)]
pub enum UploadSource {
    /// A local file, streamed from disk as the request goes out.
    File(PathBuf),
    /// Bytes already in memory, streamed in chunks the same way.
    Bytes(Vec<u8>),
}

impl std::fmt::Debug for UploadSource {
    /// Prints a byte count rather than the bytes, so a failing assertion on a pasted image stays readable.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UploadSource::File(path) => f.debug_tuple("File").field(path).finish(),
            UploadSource::Bytes(bytes) => f
                .debug_tuple("Bytes")
                .field(&format_args!("{} bytes", bytes.len()))
                .finish(),
        }
    }
}

// ---------------------------------------------------------------------------
// Serving requests
// ---------------------------------------------------------------------------

pub(crate) type Reply<T> = oneshot::Sender<Result<T, TransferError>>;

/// A transfer as the GUI asked for it. The fake receives these directly; the real side serves them over HTTP.
pub(crate) enum Request {
    Stat {
        session: String,
        path: String,
        reply: Reply<FileStat>,
    },
    Download {
        session: String,
        path: String,
        open: bool,
        reply: Reply<SavedDownload>,
    },
    DownloadBytes {
        session: String,
        path: String,
        max_bytes: usize,
        reply: Reply<Vec<u8>>,
    },
    Upload {
        session: String,
        source: UploadSource,
        filename: String,
        reply: Reply<String>,
    },
}

/// A handle and the receiver its requests arrive on.
pub(crate) fn request_channel() -> (Transfers, mpsc::UnboundedReceiver<Request>) {
    let (requests, received) = mpsc::unbounded_channel();
    (Transfers { requests }, received)
}

/// Serve transfers to `endpoint` on the current tokio runtime, until every [`Transfers`] clone is dropped. Panics
/// outside a runtime.
pub(crate) fn spawn(endpoint: HelmEndpoint) -> Transfers {
    let (transfers, requests) = request_channel();
    tokio::spawn(serve(endpoint, requests));
    transfers
}

/// Hand each request its own task, so a long upload never holds up a hover check behind it.
async fn serve(endpoint: HelmEndpoint, mut requests: mpsc::UnboundedReceiver<Request>) {
    // The helm is always loopback, so a system proxy setting must not catch these requests, and a redirect would only
    // ever lead away from it. Building can fail on a machine without CA certificates (see reqwest's entry in the
    // workspace manifest), even for plain HTTP; then every transfer says so instead of the app failing to start.
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| TransferError::NotSent(format!("could not set up HTTP: {error}")));
    while let Some(request) = requests.recv().await {
        tokio::spawn(run(client.clone(), endpoint.clone(), request));
    }
}

/// Run one transfer to its answer, or stop it as soon as the GUI drops its [`Transfer`].
async fn run(
    client: Result<reqwest::Client, TransferError>,
    endpoint: HelmEndpoint,
    request: Request,
) {
    match request {
        Request::Stat {
            session,
            path,
            reply,
        } => {
            let body = FileRequest {
                path,
                save_to_downloads: false,
                open: false,
            };
            let segments = ["sessions", session.as_str(), "files", "stat"];
            finish(reply, async {
                post_json(&client?, &endpoint, &segments, &body).await
            })
            .await;
        }
        Request::Download {
            session,
            path,
            open,
            reply,
        } => {
            // The GUI is a desktop app: it always asks for the helm's native save, never an octet body to save itself.
            let body = FileRequest {
                path,
                save_to_downloads: true,
                open,
            };
            let segments = ["sessions", session.as_str(), "files", "download"];
            finish(reply, async {
                let saved: DownloadReply = post_json(&client?, &endpoint, &segments, &body).await?;
                Ok(saved.into())
            })
            .await;
        }
        Request::DownloadBytes {
            session,
            path,
            max_bytes,
            reply,
        } => {
            // No save and no open: the helm then relays the file's bytes as the answer's body.
            let body = FileRequest {
                path,
                save_to_downloads: false,
                open: false,
            };
            let segments = ["sessions", session.as_str(), "files", "download"];
            finish(reply, async {
                let response = post_file_request(&client?, &endpoint, &segments, &body).await?;
                let response = check_status(response).await?;
                read_capped(response, max_bytes, || TransferError::TooLarge {
                    max_bytes,
                })
                .await
            })
            .await;
        }
        Request::Upload {
            session,
            source,
            filename,
            reply,
        } => {
            finish(reply, async {
                upload(&client?, &endpoint, &session, source, &filename).await
            })
            .await;
        }
    }
}

/// Answer `reply` with `work`'s result, or drop `work` (and with it the request's connection) once nobody waits.
async fn finish<T>(mut reply: Reply<T>, work: impl Future<Output = Result<T, TransferError>>) {
    let result = tokio::select! {
        result = work => Some(result),
        () = reply.closed() => None,
    };
    if let Some(result) = result {
        let _ = reply.send(result);
    }
}

// ---------------------------------------------------------------------------
// The routes' wire shapes
// ---------------------------------------------------------------------------
//
// NOTE: The bodies are `farhelm_proto::file_transfers` types, the same ones the helm encodes and decodes, so neither
// side restates the other's. Their literal JSON is pinned in that module and again here, by the stub server's answers
// and the exact request bytes; the real-helm test (`tests/real_helm.rs`) checks the helm decodes the request before it
// refuses. What stays local is the GUI's reading of an answer: a save becomes a [`SavedDownload`] with an
// [`OpenOutcome`], and an upload answer without a path is refused.

/// A save answer read for the GUI: `opened` is present only when the request asked to open, and `open_error` only when
/// that open failed.
impl From<DownloadReply> for SavedDownload {
    fn from(reply: DownloadReply) -> SavedDownload {
        let open = match reply.opened {
            None => OpenOutcome::NotAsked,
            Some(true) => OpenOutcome::Opened,
            Some(false) => OpenOutcome::NotOpened(reply.open_error.unwrap_or_default()),
        };
        SavedDownload {
            path: reply.path,
            open,
        }
    }
}

fn url(endpoint: &HelmEndpoint, path: &ApiPath) -> String {
    format!("http://{}{}", endpoint.addr, path.as_str())
}

fn invalid_path(error: crate::PathError) -> TransferError {
    TransferError::NotSent(format!("invalid request path: {error}"))
}

/// Classify a request that got no response. Only a failed connect proves the helm saw nothing of it.
fn send_failed(error: reqwest::Error) -> TransferError {
    if error.is_connect() || error.is_builder() {
        TransferError::NotSent(error.to_string())
    } else {
        TransferError::Lost(error.to_string())
    }
}

/// POST `body` as JSON to the file route named by `segments`, and decode its answer.
async fn post_json<T: DeserializeOwned>(
    client: &reqwest::Client,
    endpoint: &HelmEndpoint,
    segments: &[&str],
    body: &FileRequest,
) -> Result<T, TransferError> {
    read_answer(post_file_request(client, endpoint, segments, body).await?).await
}

/// POST `body` as JSON to the file route named by `segments`, with the device credential, and hand back the response
/// whatever its status. Stat, download and download of bytes all send this one request shape.
async fn post_file_request(
    client: &reqwest::Client,
    endpoint: &HelmEndpoint,
    segments: &[&str],
    body: &FileRequest,
) -> Result<reqwest::Response, TransferError> {
    let path = ApiPath::api(segments).map_err(invalid_path)?;
    client
        .post(url(endpoint, &path))
        .bearer_auth(endpoint.secret())
        .json(body)
        .send()
        .await
        .map_err(send_failed)
}

/// Decode a response: an error status is the helm's refusal, anything else must be the route's JSON answer.
async fn read_answer<T: DeserializeOwned>(response: reqwest::Response) -> Result<T, TransferError> {
    let body = read_capped(check_status(response).await?, ANSWER_MAX_BYTES, || {
        TransferError::BadReply(format!(
            "the helm's answer is over {ANSWER_MAX_BYTES} bytes"
        ))
    })
    .await?;
    serde_json::from_slice(&body).map_err(|error| TransferError::BadReply(error.to_string()))
}

/// Pass a success response on; turn any other status into the helm's refusal, with its words. The helm writes a
/// refusal as a line of plain text, except a 401's, which is JSON for code and is replaced by [`CREDENTIAL_REFUSED`].
async fn check_status(response: reqwest::Response) -> Result<reqwest::Response, TransferError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    // The status alone is the refusal; a body lost or oversized after it only costs the helm's words.
    let message = if status == reqwest::StatusCode::UNAUTHORIZED {
        CREDENTIAL_REFUSED.to_owned()
    } else {
        // Any failure to read the words, the cap included, leaves the message empty, so the error the cap would
        // raise is never seen and only has to satisfy the signature.
        let body = read_capped(response, ANSWER_MAX_BYTES, || {
            TransferError::BadReply(String::new())
        })
        .await
        .unwrap_or_default();
        String::from_utf8_lossy(&body).trim().to_owned()
    };
    Err(TransferError::Refused {
        status: status.as_u16(),
        message,
    })
}

/// Read a whole body, up to `max_bytes`, failing with `over_cap()` at the first chunk that would pass it. Nothing past
/// the cap is kept: the chunk that crosses it is refused before it is copied, and dropping the response on that error
/// closes the connection, which stops the helm sending the rest.
async fn read_capped(
    mut response: reqwest::Response,
    max_bytes: usize,
    over_cap: impl FnOnce() -> TransferError,
) -> Result<Vec<u8>, TransferError> {
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| TransferError::Lost(error.to_string()))?
    {
        if body.len() + chunk.len() > max_bytes {
            return Err(over_cap());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// Send `source` to the session's attachments route and answer the host path the helm reports.
async fn upload(
    client: &reqwest::Client,
    endpoint: &HelmEndpoint,
    session: &str,
    source: UploadSource,
    filename: &str,
) -> Result<String, TransferError> {
    let path = ApiPath::api(&["sessions", session, "attachments"])
        .and_then(|path| path.with_query(&[("filename", filename)]))
        .map_err(invalid_path)?;
    // Set only when a file's read fails mid-body, which is the one failure reqwest cannot name.
    let failure = Arc::new(OnceLock::new());
    let (body, size) = match source {
        UploadSource::Bytes(bytes) => {
            let size = bytes.len() as u64;
            // Streamed, not handed over whole: hyper writes a whole body to its end even after the caller's future is
            // dropped, which would publish an abandoned paste. A stream stops when it is dropped with the request.
            let body = if bytes.is_empty() {
                reqwest::Body::from(Vec::new())
            } else {
                reqwest::Body::wrap_stream(memory_chunks(bytes))
            };
            (body, size)
        }
        UploadSource::File(file) => {
            let (body, size) = FileBody::open(&file, Arc::clone(&failure))
                .await
                .map_err(|error| TransferError::File(error.to_string()))?;
            // hyper never polls a body declared empty, so an empty file goes out as an empty body;
            // `FileBody::open` has already made its growth check.
            let body = if size == 0 {
                reqwest::Body::from(Vec::new())
            } else {
                reqwest::Body::wrap_stream(body.into_stream())
            };
            (body, size)
        }
    };
    // The helm declares the attachment's size to the supervisor from `Content-Length` and refuses a request without
    // one, so the streamed body carries the size measured at open rather than going out chunked.
    let response = client
        .post(url(endpoint, &path))
        .bearer_auth(endpoint.secret())
        .header(reqwest::header::CONTENT_LENGTH, size)
        .body(body)
        .send()
        .await
        .map_err(|error| match failure.get() {
            Some(reason) => TransferError::File(reason.clone()),
            None => send_failed(error),
        })?;
    let reply: UploadReply = read_answer(response).await?;
    if reply.path.is_empty() {
        return Err(TransferError::BadReply(
            "the upload answer carried no path".into(),
        ));
    }
    Ok(reply.path)
}

/// `bytes` as a stream of [`UPLOAD_READ_BYTES`] chunks, holding no copy beyond the chunk being sent.
fn memory_chunks(bytes: Vec<u8>) -> impl Stream<Item = std::io::Result<Vec<u8>>> + Send + 'static {
    futures_util::stream::unfold((bytes, 0), |(bytes, start)| async move {
        let end = (start + UPLOAD_READ_BYTES).min(bytes.len());
        (start < bytes.len()).then(|| (Ok(bytes[start..end].to_vec()), (bytes, end)))
    })
}

/// An upload's body, read from disk as the request goes out.
///
/// It sends exactly the size it declared, or fails before the helm has all of it. The helm publishes only once the
/// declared length has arrived, so a file that shrinks or grows while it is read must never complete the body: the
/// growth check runs before the last chunk is handed over (for an empty file, in [`FileBody::open`], since an empty
/// body is never read), which is what lets [`TransferError::File`] promise that nothing was published.
struct FileBody {
    file: tokio::fs::File,
    /// Bytes still to send. The stream ends when this reaches zero, after the last chunk or a failure.
    remaining: u64,
    /// Why reading failed, for the error the request then ends with (reqwest reports only that the body failed).
    failure: Arc<OnceLock<String>>,
}

impl FileBody {
    /// Open a regular file, answering the body and its size. An empty file is checked for growth here. A read that
    /// fails later records its reason in `failure`.
    async fn open(path: &Path, failure: Arc<OnceLock<String>>) -> std::io::Result<(FileBody, u64)> {
        // Checked before opening, following symlinks, because opening a FIFO waits for a writer that may never come.
        // The check after opening is the one that counts: the path can change in between.
        if !tokio::fs::metadata(path).await?.is_file() {
            return Err(not_regular());
        }
        let mut file = tokio::fs::File::open(path).await?;
        let metadata = file.metadata().await?;
        if !metadata.is_file() {
            return Err(not_regular());
        }
        if metadata.len() == 0 && file.read(&mut [0u8; 1]).await? != 0 {
            return Err(changed_size());
        }
        let body = FileBody {
            file,
            remaining: metadata.len(),
            failure,
        };
        Ok((body, metadata.len()))
    }

    fn into_stream(self) -> impl Stream<Item = std::io::Result<Vec<u8>>> + Send + 'static {
        futures_util::stream::unfold(self, |mut body| async move {
            if body.remaining == 0 {
                return None;
            }
            let chunk = body.next_chunk().await;
            if let Err(error) = &chunk {
                let _ = body.failure.set(error.to_string());
                body.remaining = 0;
            }
            Some((chunk, body))
        })
    }

    /// Read the next chunk; call only while bytes remain. The last one comes only after the file proved no longer.
    async fn next_chunk(&mut self) -> std::io::Result<Vec<u8>> {
        let length = usize::try_from(self.remaining).map_or(UPLOAD_READ_BYTES, |remaining| {
            remaining.min(UPLOAD_READ_BYTES)
        });
        let mut chunk = vec![0; length];
        self.file
            .read_exact(&mut chunk)
            .await
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::UnexpectedEof => changed_size(),
                _ => error,
            })?;
        self.remaining -= length as u64;
        if self.remaining == 0 && self.file.read(&mut [0u8; 1]).await? != 0 {
            return Err(changed_size());
        }
        Ok(chunk)
    }
}

fn not_regular() -> std::io::Error {
    std::io::Error::other("it is not a regular file")
}

fn changed_size() -> std::io::Error {
    std::io::Error::other("the file changed size while it was being attached")
}

#[cfg(test)]
mod tests;
