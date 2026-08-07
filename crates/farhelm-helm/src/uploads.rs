//! `POST /api/sessions/{id}/attachments` — an attachment's bytes, streamed
//! from the browser to the host the session lives on.
//!
//! The only route in the helm that moves a body of unbounded size, and the
//! only one whose failure modes are interesting enough to need their own
//! module. Everything here exists to keep one promise from SPEC.md: an
//! upload that fails must SAY so. The worst available outcome is a transfer
//! that publishes on the host while the caller is told it failed, or a
//! caller left waiting on a transfer nobody is still working on.
//!
//! ## Three ways a transfer stops that are not "it finished"
//!
//! The browser streams a body, this handler rechunks it onto the wire, and
//! the supervisor acks what it has written. Each of those can stop without
//! saying so, and each needs its own answer:
//!
//! - **The client disconnects mid-body.** Usually this arrives as an error
//!   item on the body stream, which the loop handles where it reads: abort
//!   upstream, answer 400. But a dropped connection can also cancel this
//!   future outright, and then no code in this file runs again — so the
//!   abort ALSO has to be reachable without running any, which is why
//!   [`crate::client`]'s upload guard sends `AbortUpload` from its `Drop`.
//!   The explicit arm is the ordinary path; the guard is the one that
//!   cannot be skipped.
//! - **The supervisor stops acking.** Credit stops flowing and the
//!   `CLIENT_UPLOAD_STALL_TIMEOUT` deadline ends the transfer rather than
//!   letting it hang forever.
//! - **The supervisor aborts.** Its reason is what the caller is told, via
//!   `upload_ended_error`, because a supervisor's own words are always
//!   more actionable than a generic failure.
//!
//! `UploadStep` is what lets one loop wait on all of those at once
//! without any of them being able to starve the others.

use crate::sessions::route_session;
use crate::{AppState, SupervisorError, http_error};
use axum::extract::{Path as AxPath, Query, State};
use axum::response::IntoResponse;
use farhelm_proto::ErrorKind;
use serde::Deserialize;
use std::sync::Arc;
use tracing::{info, warn};

/// Query parameters for `POST /api/sessions/{id}/attachments` — the
/// pinned attachment REST contract's `?filename=` proposal (PLAN_M4.md
/// item 5). `#[serde(default)]` is what makes an ABSENT query string and a
/// PRESENT-but-empty `?filename=` decode identically to `""`: both mean
/// "no name proposed", and `ControlMsg::BeginUpload`'s own docs say an
/// empty proposal is never a refusal, only ever sanitized away in favor of
/// a generated fallback name.
#[derive(Deserialize)]
pub(crate) struct UploadQuery {
    #[serde(default)]
    filename: String,
}

/// How long the browser→helm hop of an attachment upload may go without
/// the request body producing BYTES before the relay gives up on it.
///
/// This is the helm's own leg of PLAN_M4.md item 4's per-hop progress
/// timeout (the pinned REST contract's "per-hop progress timeout per
/// proto docs"), and it is genuinely a different bound from the
/// supervisor-facing credit stall `UploadGuard::send_upload_chunk`
/// applies: this one catches a browser that stops SENDING (a stalled
/// paste, a dead network path), which would otherwise park this handler —
/// and the supervisor's upload channel behind it — forever, with nothing
/// but the browser's own disconnect to ever notice.
///
/// Progress means BYTES, not events. A body stream is free to yield
/// empty chunks, and an endless supply of them is exactly the shape a
/// no-progress transfer takes; rearming on any yielded item would let a
/// client hold an upload open indefinitely while relaying nothing, so the
/// deadline is absolute and only a non-empty relayed chunk moves it.
///
/// Sixty seconds matches `WRITER_STALL_TIMEOUT` and
/// `UPLOAD_ACK_STALL_TIMEOUT` (`client.rs`), so every hop of one transfer
/// is declared stalled on the same generous, non-arbitrary timescale.
const CLIENT_UPLOAD_STALL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// Turn an upload-ending reason (a stall, an `UploadAborted`, or a dead
/// connection to the supervisor) into the mapped error response the
/// pinned REST contract promises: "500-class with the reason text",
/// because a receiver-side abort is a server-side fault the caller could
/// not have avoided by sending a different request — never a 4xx, which
/// would wrongly suggest the browser's request was itself the problem.
fn upload_ended_error(reason: String) -> axum::response::Response {
    http_error(anyhow::Error::new(SupervisorError {
        kind: ErrorKind::Internal,
        message: reason,
    }))
}

/// `POST /api/sessions/{id}/attachments?filename=<proposed name>` — the
/// helm's streaming relay for one attachment upload (PLAN_M4.md item 5;
/// see the pinned attachment REST contract for the wire-level shape this
/// implements). No direct client-to-supervisor path exists, so every byte
/// crosses this handler: `BeginUpload` (declaring the request's
/// `Content-Length` as the size), then the body forwarded chunk by chunk
/// via `SupervisorClient::send_upload_chunk` (rechunked at
/// `UPLOAD_CHUNK_BYTES`, paced by the credit window), then `CommitUpload`
/// once the body ends — mirroring `serve_term`'s "every exit path tells
/// the supervisor" discipline, but for a send-direction attachment
/// instead of a terminal.
///
/// STREAMS rather than buffers: `request.into_body().into_data_stream()`
/// is read one chunk at a time and each chunk is forwarded (and its
/// memory released) before the next is read, so a multi-gigabyte upload
/// costs this handler time, never a proportional amount of memory — the
/// same "no size cap in v1" promise PLAN_M4.md item 4 makes for the
/// supervisor's own half of this path.
///
/// "Every exit path" includes the one that runs no code here at all: a
/// browser resetting the connection cancels this future outright, so the
/// obligation to abort belongs to the `UploadGuard` that `begin_upload`
/// returns rather than to the branches below (see its docs). The branches
/// exist to report accurately, not to be the only thing standing between a
/// dead client and a supervisor holding a temp file forever.
///
/// The endings, and what each is answerable for:
/// - Body read error (a genuine client disconnect surfaces this way once
///   `Content-Length` framing is in play — the transport cannot honestly
///   deliver a short body any other way) or a stall past
///   [`CLIENT_UPLOAD_STALL_TIMEOUT`]: `AbortUpload` reaches the
///   supervisor and the failure is reported.
/// - A body LONGER than the declared `Content-Length`: refused here, as a
///   400, before the excess is forwarded. The supervisor cannot classify
///   this one — past `UploadStarted` it has no pending `req_id` to hang a
///   correlated `InvalidRequest` on, so an overrun would reach it as an
///   uncorrelated abort — and this hop is the only one holding both the
///   declaration and the byte count, so the mismatch is the helm's to
///   name. A SHORT body is the opposite case: the supervisor detects it
///   at commit and its own message passes through verbatim.
/// - `UploadAborted` mid-stream (the supervisor's own stall timeout, a
///   storage error, the session deleted mid-transfer): raced against the
///   body read, so it ends the request when it arrives rather than
///   whenever the browser next sends something, and the reason is mapped
///   through [`upload_ended_error`].
/// - The supervisor's own `Error` replies to `BeginUpload`/`CommitUpload`
///   (unknown session, size mismatch, storage failure) reach the browser
///   through the same `http_error` every other endpoint uses — the
///   supervisor's message verbatim, sentinel-testable.
/// - Success: `CommitUpload` answers with the published path, returned as
///   `{"path": "..."}` — `UploadCommitted::path` passed through verbatim.
pub(crate) async fn upload_attachment(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
    Query(q): Query<UploadQuery>,
    request: axum::extract::Request,
) -> axum::response::Response {
    use farhelm_proto::UPLOAD_CHUNK_BYTES;
    use futures_util::{FutureExt, StreamExt};

    // Declared size = Content-Length (the pinned contract's own words):
    // the browser sets this from the Blob it is uploading, so an absent
    // or unparsable value means this was never a conformant upload
    // request at all, and there is nothing to declare to `BeginUpload`.
    let Some(size) = request
        .headers()
        .get(axum::http::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
    else {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            "attachment upload requires a valid Content-Length header\n".to_string(),
        )
            .into_response();
    };

    // Routed BEFORE the body is touched, like every other session
    // operation: an upload to a session on a non-connected host must be
    // refused with that host's state named, not half-relayed and then
    // abandoned.
    let (_claim, client) = match route_session(&state, &id).await {
        Ok(routed) => routed,
        Err(e) => return http_error(e),
    };
    let mut upload = match client.begin_upload(&id, &q.filename, size).await {
        Ok(upload) => upload,
        Err(e) => return http_error(e),
    };
    let channel = upload.channel();

    let mut body = request.into_body().into_data_stream();
    let mut sent: u64 = 0;
    // Bytes accepted from the body but not yet framed. Rechunking works
    // in both directions — a body chunk larger than
    // `UPLOAD_CHUNK_BYTES` is split, and several smaller ones are
    // coalesced — because the protocol's frame size is the SENDER's
    // decision, not an echo of however hyper happened to slice the
    // request. Without the coalescing half, a browser trickling 8 KiB
    // pieces would put tens of thousands of undersized frames on a
    // connection whose framing discipline exists to bound exactly that.
    let mut pending: Vec<u8> = Vec::new();
    // Absolute, not per-item: see `CLIENT_UPLOAD_STALL_TIMEOUT` for why
    // only non-empty body bytes may move it.
    let mut deadline = tokio::time::Instant::now() + CLIENT_UPLOAD_STALL_TIMEOUT;
    loop {
        // Coalescing must never become buffering: bytes are held back
        // only while more are already waiting. The moment the body has
        // nothing ready, whatever is pending goes out before this parks —
        // otherwise a slow producer's bytes would sit here unsent, and
        // the SUPERVISOR's own progress timeout would fire on a transfer
        // that was making perfectly good progress.
        //
        // `now_or_never` polls with a throwaway waker, so a readiness
        // notification arriving during that poll would be dropped — which
        // is harmless only because the `None` arm polls the same stream
        // again, with a real waker, a few lines later.
        let step = match body.next().now_or_never() {
            Some(next) => UploadStep::Body(next),
            None => {
                if !pending.is_empty() {
                    let flushed = std::mem::take(&mut pending);
                    if let Err(reason) = upload.send_upload_chunk(&flushed).await {
                        return upload_ended_error(reason);
                    }
                }
                // The select yields a value rather than acting inside its
                // arms: every arm borrows `upload` or `body`, and the
                // paths below need `&mut upload` back to abort.
                tokio::select! {
                    biased;
                    reason = upload.ended() => UploadStep::Ended(reason),
                    _ = tokio::time::sleep_until(deadline) => UploadStep::Stalled,
                    next = body.next() => UploadStep::Body(next),
                }
            }
        };
        let chunk = match step {
            // The transfer ended upstream — the supervisor gave up, or
            // the connection to it died. Nothing left to abort in either
            // case, and the reason is what the browser must see.
            UploadStep::Ended(reason) => return upload_ended_error(reason),
            // No body progress within the stall window — this hop's own
            // leg of PLAN_M4.md item 4's per-hop timeout.
            UploadStep::Stalled => {
                warn!(
                    session_id = %id, channel,
                    "attachment upload stalled: no body progress from the client"
                );
                let stalled = farhelm_proto::UPLOAD_ABORT_REASON_STALLED.to_string();
                upload.abort(stalled.clone()).await;
                return upload_ended_error(stalled);
            }
            UploadStep::Body(Some(Ok(bytes))) => bytes,
            // The body stream itself failed — a genuine disconnect, or a
            // framing violation. Release the supervisor's half rather
            // than leaving it waiting for bytes that will never arrive.
            UploadStep::Body(Some(Err(e))) => {
                warn!(
                    session_id = %id, channel, error = %e,
                    "attachment upload body ended before Content-Length was reached"
                );
                upload.abort(format!("client body failed: {e}")).await;
                return (
                    axum::http::StatusCode::BAD_REQUEST,
                    format!("attachment upload failed: {e}\n"),
                )
                    .into_response();
            }
            // The body ended normally: every declared byte arrived (or
            // the browser's framing said so), so it is time to publish.
            UploadStep::Body(None) => break,
        };
        // An empty chunk is a legal thing for a body stream to yield and
        // carries no progress, so it neither reaches the supervisor nor
        // extends the deadline.
        if chunk.is_empty() {
            continue;
        }
        if sent.saturating_add(chunk.len() as u64) > size {
            warn!(
                session_id = %id, channel, declared = size,
                "attachment upload body exceeded its declared Content-Length"
            );
            upload
                .abort("client body exceeded its declared size".to_string())
                .await;
            return (
                axum::http::StatusCode::BAD_REQUEST,
                format!(
                    "attachment upload body is longer than the declared Content-Length of {size} \
                     bytes\n"
                ),
            )
                .into_response();
        }
        sent += chunk.len() as u64;
        deadline = tokio::time::Instant::now() + CLIENT_UPLOAD_STALL_TIMEOUT;
        pending.extend_from_slice(&chunk);
        // Only whole frames go out here; the tail waits for either more
        // bytes or the flush above, so no partial frame is ever emitted
        // while the body still has data queued behind it.
        let whole = pending.len() - pending.len() % UPLOAD_CHUNK_BYTES;
        if whole > 0 {
            if let Err(reason) = upload.send_upload_chunk(&pending[..whole]).await {
                return upload_ended_error(reason);
            }
            pending.drain(..whole);
        }
    }

    // The body's tail, whatever is left of a partial frame. An empty
    // `pending` is the ordinary case for a body that ended on a chunk
    // boundary — and for a zero-byte upload, which sends no data at all
    // and goes straight to the commit.
    if !pending.is_empty()
        && let Err(reason) = upload.send_upload_chunk(&pending).await
    {
        return upload_ended_error(reason);
    }

    match upload.commit().await {
        Ok(path) => {
            info!(session_id = %id, channel, bytes = sent, "attachment upload published");
            axum::Json(serde_json::json!({ "path": path })).into_response()
        }
        Err(e) => http_error(e),
    }
}

/// What one turn of [`upload_attachment`]'s relay loop resolved to.
///
/// Exists so the loop's `select!` can hand its outcome out before anything
/// acts on it: the arms hold borrows of the upload guard and the body
/// stream, while every teardown path needs the guard back to abort with.
enum UploadStep {
    /// The upload ended upstream, with this verbatim reason.
    Ended(String),
    /// The stall deadline expired with no relayed bytes.
    Stalled,
    /// The body stream produced an item (or ended, as `None`).
    Body(Option<Result<axum::body::Bytes, axum::Error>>),
}

#[cfg(test)]
mod tests {
    use crate::{BUILD_STAMP_HEADER, rest_harness};
    use std::time::Duration;

    /// `POST /api/sessions/{id}/attachments` happy path (PLAN_M4.md item
    /// 5, the pinned attachment REST contract): a body larger than one
    /// `UPLOAD_CHUNK_BYTES` streams as `BeginUpload` -> N data frames ->
    /// `CommitUpload`, and the published path comes back verbatim under
    /// `{"path": ...}`.
    ///
    /// The body is built as ONE `Body::from` buffer deliberately — a
    /// single HTTP body chunk with no relation to `UPLOAD_CHUNK_BYTES` —
    /// and the peer asserts the exact frame BOUNDARIES, not merely that
    /// the bytes reassemble. That makes this the pinned test for the
    /// contract's "rechunked at UPLOAD_CHUNK_BYTES regardless of body
    /// chunking" in the one-giant-chunk direction; the opposite direction
    /// (many small body chunks coalescing into full frames) is
    /// `upload_attachment_rechunks_irregular_streaming_body_chunks`.
    #[tokio::test]
    async fn upload_attachment_happy_path_streams_chunks_and_returns_the_path() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, UPLOAD_CHUNK_BYTES};
        use tower::ServiceExt;

        let total = UPLOAD_CHUNK_BYTES * 2 + 500;
        let content: Vec<u8> = (0..total).map(|i| (i % 256) as u8).collect();

        let (client_side, peer_side) = tokio::io::duplex(4 * 1024 * 1024);
        let peer = tokio::spawn({
            let content = content.clone();
            async move {
                let (r, w) = tokio::io::split(peer_side);
                let mut reader = FrameReader::new(r);
                let mut writer = FrameWriter::new(w);
                handshake(&mut reader, &mut writer, "supervisor")
                    .await
                    .unwrap();
                let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
                let ControlMsg::BeginUpload {
                    req_id,
                    session_id,
                    channel,
                    size,
                    ..
                } = request
                else {
                    panic!("expected BeginUpload, got {request:?}");
                };
                assert_eq!(session_id, "sess-1");
                assert_eq!(
                    size, total as u64,
                    "declared size must be the request's Content-Length"
                );
                writer
                    .write_control(&ControlMsg::UploadStarted { req_id, channel })
                    .await
                    .unwrap();

                // Read every data frame until the whole body is accounted
                // for, checking the split points and byte-for-byte
                // fidelity: a rechunking bug that reordered or duplicated
                // bytes must fail this too, not only one that split at
                // the wrong size.
                let mut reassembled = Vec::new();
                let mut lens = Vec::new();
                while reassembled.len() < content.len() {
                    let frame = reader.read_frame().await.unwrap().unwrap();
                    assert_eq!(frame.channel, channel);
                    lens.push(frame.body.len());
                    reassembled.extend_from_slice(&frame.body);
                }
                assert_eq!(
                    lens,
                    vec![UPLOAD_CHUNK_BYTES, UPLOAD_CHUNK_BYTES, 500],
                    "one HTTP body buffer must still land on the wire as UPLOAD_CHUNK_BYTES \
                     frames, with only the final piece short"
                );
                assert_eq!(reassembled, content);

                let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
                let ControlMsg::CommitUpload { req_id, .. } = request else {
                    panic!("expected CommitUpload, got {request:?}");
                };
                writer
                    .write_control(&ControlMsg::UploadCommitted {
                        req_id,
                        path: "/data/sessions/sess-1/attachments/screenshot.png".to_string(),
                    })
                    .await
                    .unwrap();
            }
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments?filename=screenshot.png")
            .header("host", "127.0.0.1:7433")
            .header("content-length", total.to_string())
            .body(axum::body::Body::from(content))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            value["path"],
            "/data/sessions/sess-1/attachments/screenshot.png"
        );

        peer.await.unwrap();
    }

    /// The other rechunking direction, and the one a per-body-chunk relay
    /// silently fails: a body arriving as many IRREGULAR pieces, all
    /// smaller than `UPLOAD_CHUNK_BYTES` and none a divisor of it, must
    /// still reach the wire as full-sized frames with only the final piece
    /// short. Forwarding each body chunk as its own frame would reassemble
    /// to the same bytes — which is why the happy path alone cannot catch
    /// it — while putting hundreds of undersized frames on a connection
    /// whose framing discipline exists to bound exactly that.
    ///
    /// The sizes deliberately do not line up with the chunk boundary, so
    /// frames end mid-piece and a coalescing bug that only worked for
    /// aligned inputs fails here.
    #[tokio::test]
    async fn upload_attachment_rechunks_irregular_streaming_body_chunks() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, UPLOAD_CHUNK_BYTES};
        use tower::ServiceExt;

        // Repeating sizes that share no factor with UPLOAD_CHUNK_BYTES,
        // so every protocol frame boundary lands inside a body piece.
        let piece_sizes: [usize; 4] = [1000, 7777, 33_333, 101];
        let mut pieces = Vec::new();
        let mut content = Vec::new();
        while content.len() < UPLOAD_CHUNK_BYTES * 2 {
            let size = piece_sizes[pieces.len() % piece_sizes.len()];
            let piece: Vec<u8> = (0..size)
                .map(|i| ((content.len() + i) % 251) as u8)
                .collect();
            content.extend_from_slice(&piece);
            pieces.push(piece);
        }
        let total = content.len();
        let expected_lens: Vec<usize> =
            std::iter::repeat_n(UPLOAD_CHUNK_BYTES, total / UPLOAD_CHUNK_BYTES)
                .chain(std::iter::once(total % UPLOAD_CHUNK_BYTES))
                .filter(|len| *len > 0)
                .collect();

        let (client_side, peer_side) = tokio::io::duplex(4 * 1024 * 1024);
        let peer = tokio::spawn({
            let content = content.clone();
            async move {
                let (r, w) = tokio::io::split(peer_side);
                let mut reader = FrameReader::new(r);
                let mut writer = FrameWriter::new(w);
                handshake(&mut reader, &mut writer, "supervisor")
                    .await
                    .unwrap();
                let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
                let ControlMsg::BeginUpload {
                    req_id, channel, ..
                } = request
                else {
                    panic!("expected BeginUpload, got {request:?}");
                };
                writer
                    .write_control(&ControlMsg::UploadStarted { req_id, channel })
                    .await
                    .unwrap();

                let mut lens = Vec::new();
                let mut reassembled = Vec::new();
                while reassembled.len() < content.len() {
                    let frame = reader.read_frame().await.unwrap().unwrap();
                    lens.push(frame.body.len());
                    reassembled.extend_from_slice(&frame.body);
                }
                assert_eq!(
                    lens, expected_lens,
                    "irregular body pieces must be coalesced into UPLOAD_CHUNK_BYTES frames, not \
                     forwarded one frame per body chunk"
                );
                assert_eq!(
                    reassembled, content,
                    "rechunking must preserve every byte in order"
                );

                let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
                let ControlMsg::CommitUpload { req_id, .. } = request else {
                    panic!("expected CommitUpload, got {request:?}");
                };
                writer
                    .write_control(&ControlMsg::UploadCommitted {
                        req_id,
                        path: "/tmp/pub.bin".to_string(),
                    })
                    .await
                    .unwrap();
            }
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let body_stream =
            futures_util::stream::iter(pieces.into_iter().map(Ok::<Vec<u8>, std::io::Error>));
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments")
            .header("host", "127.0.0.1:7433")
            .header("content-length", total.to_string())
            .body(axum::body::Body::from_stream(body_stream))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        peer.await.unwrap();
    }

    /// The desktop build's `fetch` preflights before it may upload
    /// anything, because A1's explicit Authorization header is not
    /// CORS-simple (and the file's content type may not be either). Without
    /// a credential-free `OPTIONS` route that permits both headers, the
    /// browser stops there and the desktop attachment flow never sends a byte.
    ///
    /// Pinned per header rather than "some CORS headers exist": each one
    /// is separately load-bearing, and a preflight missing any of them
    /// fails in a way whose only symptom is an upload that never happens.
    #[tokio::test]
    async fn attachment_preflight_answers_the_desktop_webview_origin() {
        use tower::ServiceExt;

        let harness = rest_harness::idle_helm().await;

        let app = harness.unauthenticated_router();
        let request = axum::http::Request::builder()
            .method("OPTIONS")
            .uri("/api/sessions/sess-1/attachments?filename=shot.png")
            .header("host", "127.0.0.1:7433")
            .header("origin", "dioxus://index.html")
            .header("access-control-request-method", "POST")
            .header(
                "access-control-request-headers",
                "authorization, content-type",
            )
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);
        let headers = response.headers();
        assert_eq!(
            headers["access-control-allow-origin"], "dioxus://index.html",
            "the origin is echoed, not answered with a wildcard"
        );
        assert_eq!(headers["access-control-allow-methods"], "POST, OPTIONS");
        assert_eq!(
            headers["access-control-allow-headers"],
            "authorization, content-type"
        );
        assert_eq!(
            headers["vary"], "Origin",
            "one origin's answer must not be cached for another"
        );
        assert!(headers.contains_key("access-control-max-age"));

        let post = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments?filename=shot.png")
            .header("host", "127.0.0.1:7433")
            .header("origin", "dioxus://index.html")
            .body(axum::body::Body::empty())
            .unwrap();
        let response = harness
            .unauthenticated_router()
            .oneshot(post)
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);
    }

    /// The upload itself: a successful POST from the desktop webview's
    /// origin must come back READABLE, or the page is told nothing while
    /// the file sits published on the host — the worst available failure,
    /// since the user is shown an error for an attachment that exists.
    #[tokio::test]
    async fn a_successful_upload_is_readable_by_the_desktop_webview() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use tower::ServiceExt;

        let content = b"desktop-upload".to_vec();
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn({
            let len = content.len();
            async move {
                let (r, w) = tokio::io::split(peer_side);
                let mut reader = FrameReader::new(r);
                let mut writer = FrameWriter::new(w);
                handshake(&mut reader, &mut writer, "supervisor")
                    .await
                    .unwrap();
                let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
                let ControlMsg::BeginUpload {
                    req_id, channel, ..
                } = request
                else {
                    panic!("expected BeginUpload, got {request:?}");
                };
                writer
                    .write_control(&ControlMsg::UploadStarted { req_id, channel })
                    .await
                    .unwrap();
                let mut received = 0;
                while received < len {
                    received += reader.read_frame().await.unwrap().unwrap().body.len();
                }
                let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
                let ControlMsg::CommitUpload { req_id, .. } = request else {
                    panic!("expected CommitUpload, got {request:?}");
                };
                writer
                    .write_control(&ControlMsg::UploadCommitted {
                        req_id,
                        path: "/state/attachments/sess-1/shot.png".to_string(),
                    })
                    .await
                    .unwrap();
            }
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments?filename=shot.png")
            .header("host", "127.0.0.1:7433")
            .header("origin", "wry://localhost")
            .header("content-length", content.len().to_string())
            .body(axum::body::Body::from(content))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        assert_eq!(
            response.headers()["access-control-allow-origin"],
            "wry://localhost"
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["path"], "/state/attachments/sess-1/shot.png");
        peer.await.unwrap();
    }

    /// The half that a handler-level CORS implementation gets wrong: an
    /// ERROR response has to be readable too.
    ///
    /// SPEC.md requires upload failures to be visible, and a response the
    /// webview refuses to hand back is a failure with no message at all —
    /// the browser reports a generic network error and the supervisor's
    /// own words (the whole point of the pinned contract's verbatim error
    /// body) never reach the user.
    #[tokio::test]
    async fn a_refused_upload_is_readable_by_the_desktop_webview_too() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, ErrorKind};
        use tower::ServiceExt;

        // A SUPERVISOR-side refusal, aimed at a session the helm does know:
        // an id the merged view has never heard of is refused by the helm
        // itself now (PLAN_M6.md item 5), which would exercise the wrong
        // path for a test about what a cross-origin page can READ.
        const SENTINEL: &str = "the session's attachments directory is gone";
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::BeginUpload { req_id, .. } = request else {
                panic!("expected BeginUpload, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::Error {
                    req_id,
                    message: SENTINEL.to_string(),
                    kind: ErrorKind::NotFound,
                })
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments?filename=shot.png")
            .header("host", "127.0.0.1:7433")
            .header("origin", "dioxus://index.html")
            .header("content-length", "4")
            .body(axum::body::Body::from("data"))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
        assert_eq!(
            response.headers()["access-control-allow-origin"],
            "dioxus://index.html",
            "an error the page cannot read is a failure with no message"
        );
        // The build stamp has to be READABLE, not merely present: a
        // cross-origin response hides every header script did not ask to
        // have exposed, so without this the desktop webview's upload — the
        // one request this UI makes with `fetch` — could not perform the
        // skew check every other reply gets (PLAN_M6.md item 6).
        assert_eq!(
            response.headers()["access-control-expose-headers"],
            BUILD_STAMP_HEADER,
            "the upload path must be able to read the stamp it is checking"
        );
        assert!(
            response.headers().contains_key(BUILD_STAMP_HEADER),
            "and the stamp itself must be on the refusal too"
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert!(String::from_utf8_lossy(&body).contains(SENTINEL));
        peer.await.unwrap();
    }

    /// The CORS headers must not widen what the loopback guard allows.
    ///
    /// A page on another origin is refused before it reaches the route at
    /// all, and — the part worth pinning — the refusal carries NO
    /// `Access-Control-Allow-Origin`, so the attacker's page cannot even
    /// read the 403. Handing one back would turn a refusal into a probe
    /// that confirms a helm is listening.
    #[tokio::test]
    async fn a_foreign_origin_is_refused_without_cors_headers() {
        use tower::ServiceExt;

        let harness = rest_harness::idle_helm().await;

        let app = harness.router();
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments?filename=shot.png")
            .header("host", "127.0.0.1:7433")
            .header("origin", "https://attacker.example")
            .header("content-length", "4")
            .body(axum::body::Body::from("data"))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::FORBIDDEN);
        assert!(
            !response
                .headers()
                .contains_key("access-control-allow-origin"),
            "a refused origin must not be handed the means to read the refusal"
        );
    }

    /// A zero-byte attachment is an ordinary upload, not a refusal: the
    /// pinned contract has no minimum size, and an empty file is a
    /// perfectly reasonable thing to paste. `BeginUpload` declares 0, no
    /// data frames follow, and the commit publishes — the shape a relay
    /// that treated "no bytes to send" as "nothing to do" would break by
    /// never committing at all.
    #[tokio::test]
    async fn upload_attachment_zero_byte_body_publishes() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use tower::ServiceExt;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::BeginUpload {
                req_id,
                channel,
                size,
                ..
            } = request
            else {
                panic!("expected BeginUpload, got {request:?}");
            };
            assert_eq!(size, 0, "an empty body must declare a size of zero");
            writer
                .write_control(&ControlMsg::UploadStarted { req_id, channel })
                .await
                .unwrap();

            // The very next frame must be the commit: an empty upload
            // sends no data frames at all.
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::CommitUpload { req_id, .. } = request else {
                panic!("expected CommitUpload with no data frames before it, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::UploadCommitted {
                    req_id,
                    path: "/tmp/empty.txt".to_string(),
                })
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments?filename=empty.txt")
            .header("host", "127.0.0.1:7433")
            .header("content-length", "0")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["path"], "/tmp/empty.txt");
        peer.await.unwrap();
    }

    /// A body LONGER than its declared `Content-Length` must be refused as
    /// an invalid request (400), with the excess never forwarded.
    ///
    /// The pinned contract requires a size mismatch to fail, and this
    /// direction has to fail HERE: past `UploadStarted` the supervisor has
    /// no pending `req_id` left to answer with a correlated
    /// `InvalidRequest`, so an overrun would reach the browser as an
    /// uncorrelated 500-class abort — the wrong class for a request whose
    /// own framing was wrong. The peer proves the excess never reached the
    /// wire: after the in-bounds prefix it must see `AbortUpload`, not
    /// more data and not a commit.
    #[tokio::test]
    async fn upload_attachment_body_longer_than_content_length_is_refused() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, UPLOAD_CHUNK_BYTES};
        use tower::ServiceExt;

        // Exactly one frame's worth, so the in-bounds prefix is
        // observable on the wire before the overrun ends the transfer.
        let declared = UPLOAD_CHUNK_BYTES as u64;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::BeginUpload {
                req_id,
                channel,
                size,
                ..
            } = request
            else {
                panic!("expected BeginUpload, got {request:?}");
            };
            assert_eq!(size, declared);
            writer
                .write_control(&ControlMsg::UploadStarted { req_id, channel })
                .await
                .unwrap();

            // The first piece fits the declaration and is forwarded...
            let frame = reader.read_frame().await.unwrap().unwrap();
            assert_eq!(frame.body.len(), declared as usize);
            // ...and the overrun ends the transfer instead of extending it.
            let next = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            assert!(
                matches!(next, ControlMsg::AbortUpload { channel: c } if c == channel),
                "an overlong body must abort, never commit or forward the excess: {next:?}"
            );
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let body_stream = futures_util::stream::iter(vec![
            Ok::<Vec<u8>, std::io::Error>(vec![1u8; declared as usize]),
            Ok::<Vec<u8>, std::io::Error>(vec![2u8; 40]),
        ]);
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments")
            .header("host", "127.0.0.1:7433")
            .header("content-length", declared.to_string())
            .body(axum::body::Body::from_stream(body_stream))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(
            response.status(),
            axum::http::StatusCode::BAD_REQUEST,
            "a body that outruns its own Content-Length is the caller's error, not the \
             supervisor's"
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert!(
            String::from_utf8_lossy(&body).contains("longer than the declared Content-Length"),
            "the body must name the mismatch: {}",
            String::from_utf8_lossy(&body)
        );

        peer.await.unwrap();
    }

    /// A size mismatch the SUPERVISOR detects — the short-body case, and
    /// whatever else it decides at commit — must pass through this route
    /// untouched: its status class from `ErrorKind`, its message verbatim.
    /// The helm has no business rewording a commit refusal, and a locally
    /// invented "upload failed" would strip exactly the detail SPEC.md
    /// requires the user to see.
    ///
    /// Both mismatch directions are exercised because their sentinels are
    /// the only difference: the relay must be equally transparent to a
    /// commit refusal whichever way the counts disagreed.
    #[tokio::test]
    async fn upload_attachment_commit_mismatch_passes_the_sentinel_through() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, ErrorKind};
        use tower::ServiceExt;

        const SHORT: &str = "SENTINEL-commit-short-2b7e: declared 10 bytes, received 4";
        const LONG: &str = "SENTINEL-commit-long-2b7e: declared 10 bytes, received 12";

        for sentinel in [SHORT, LONG] {
            let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
            let peer = tokio::spawn(async move {
                let (r, w) = tokio::io::split(peer_side);
                let mut reader = FrameReader::new(r);
                let mut writer = FrameWriter::new(w);
                handshake(&mut reader, &mut writer, "supervisor")
                    .await
                    .unwrap();
                let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
                let ControlMsg::BeginUpload {
                    req_id, channel, ..
                } = request
                else {
                    panic!("expected BeginUpload, got {request:?}");
                };
                writer
                    .write_control(&ControlMsg::UploadStarted { req_id, channel })
                    .await
                    .unwrap();

                // The short body still reaches commit — the helm never
                // second-guesses a body that simply ended.
                let frame = reader.read_frame().await.unwrap().unwrap();
                assert_eq!(frame.body.len(), 4);
                let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
                let ControlMsg::CommitUpload { req_id, .. } = request else {
                    panic!("expected CommitUpload, got {request:?}");
                };
                writer
                    .write_control(&ControlMsg::Error {
                        req_id,
                        message: sentinel.to_string(),
                        kind: ErrorKind::InvalidRequest,
                    })
                    .await
                    .unwrap();
            });

            let harness = rest_harness::spliced_helm(client_side).await;
            let app = harness.router();

            // Four bytes against a declared ten: a body that ends early
            // without the transport ever erroring.
            let body_stream =
                futures_util::stream::iter(vec![Ok::<Vec<u8>, std::io::Error>(vec![7u8; 4])]);
            let request = axum::http::Request::builder()
                .method("POST")
                .uri("/api/sessions/sess-1/attachments")
                .header("host", "127.0.0.1:7433")
                .header("content-length", "10")
                .body(axum::body::Body::from_stream(body_stream))
                .unwrap();

            let response = app.oneshot(request).await.unwrap();
            assert_eq!(
                response.status(),
                axum::http::StatusCode::BAD_REQUEST,
                "a commit refusal's ErrorKind must pick the status, not this route"
            );
            let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            assert_eq!(
                String::from_utf8_lossy(&body),
                sentinel,
                "the supervisor's commit refusal must reach the browser verbatim"
            );
            peer.await.unwrap();
        }
    }

    /// The credit window bounds the relay to at most `UPLOAD_WINDOW_BYTES`
    /// unacknowledged bytes on the wire — the pinned REST contract's
    /// "window respected" requirement. The scripted peer withholds every
    /// ack until it has proven the sender stalled at the window, then acks
    /// and proves progress resumes and the transfer still completes.
    #[tokio::test]
    async fn upload_attachment_respects_the_credit_window_then_progresses_after_an_ack() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, UPLOAD_CHUNK_BYTES, UPLOAD_WINDOW_BYTES};
        use tower::ServiceExt;

        // UPLOAD_WINDOW_BYTES is an exact multiple of UPLOAD_CHUNK_BYTES,
        // so "one chunk past the window" needs no remainder arithmetic.
        let total = UPLOAD_WINDOW_BYTES as usize + UPLOAD_CHUNK_BYTES;
        let content = vec![3u8; total];

        let (client_side, peer_side) = tokio::io::duplex(16 * 1024 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            (reader, writer)
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();
        let (mut peer_reader, mut peer_writer) = peer.await.unwrap();

        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments")
            .header("host", "127.0.0.1:7433")
            .header("content-length", total.to_string())
            .body(axum::body::Body::from(content))
            .unwrap();
        let response_task = tokio::spawn(app.oneshot(request));

        let begin = parse_control(&peer_reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::BeginUpload {
            req_id, channel, ..
        } = begin
        else {
            panic!("expected BeginUpload, got {begin:?}");
        };
        peer_writer
            .write_control(&ControlMsg::UploadStarted { req_id, channel })
            .await
            .unwrap();

        let mut received = 0u64;
        while received < UPLOAD_WINDOW_BYTES {
            let frame = tokio::time::timeout(Duration::from_secs(5), peer_reader.read_frame())
                .await
                .expect("timed out waiting for the initial window")
                .unwrap()
                .expect("connection closed mid-window");
            received += frame.body.len() as u64;
        }
        assert_eq!(received, UPLOAD_WINDOW_BYTES);

        assert!(
            tokio::time::timeout(Duration::from_millis(300), peer_reader.read_frame())
                .await
                .is_err(),
            "the relay sent more than UPLOAD_WINDOW_BYTES before any ack"
        );
        assert!(!response_task.is_finished());

        peer_writer
            .write_control(&ControlMsg::UploadAck {
                channel,
                received: UPLOAD_WINDOW_BYTES,
            })
            .await
            .unwrap();

        let frame = tokio::time::timeout(Duration::from_secs(5), peer_reader.read_frame())
            .await
            .expect("the final chunk never arrived after the ack")
            .unwrap()
            .expect("connection closed after the ack");
        assert_eq!(frame.body.len(), UPLOAD_CHUNK_BYTES);

        let commit = parse_control(&peer_reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::CommitUpload { req_id, .. } = commit else {
            panic!("expected CommitUpload, got {commit:?}");
        };
        peer_writer
            .write_control(&ControlMsg::UploadCommitted {
                req_id,
                path: "/tmp/windowed.bin".to_string(),
            })
            .await
            .unwrap();

        let response = tokio::time::timeout(Duration::from_secs(5), response_task)
            .await
            .expect("the response never completed")
            .expect("handler task panicked")
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
    }

    /// A client disconnecting mid-body must reach the supervisor as
    /// `AbortUpload`, never a silent hang or a `CommitUpload` for bytes
    /// that never fully arrived — the pinned REST contract's own words.
    /// The body is a stream that yields a partial chunk and then a hard
    /// `Err`, which is how a genuine disconnect surfaces once
    /// `Content-Length` framing is in play: the transport cannot honestly
    /// deliver a short body any other way once it has promised an exact
    /// byte count.
    ///
    /// The partial deliberately overruns one `UPLOAD_CHUNK_BYTES` so both
    /// halves are observable: the full frame that had already been
    /// rechunked out, and then the abort. The sub-chunk tail behind it is
    /// simply dropped — flushing bytes for a transfer that is being
    /// abandoned would only make the supervisor write more of a file it
    /// is about to delete.
    #[tokio::test]
    async fn upload_attachment_client_disconnect_mid_body_sends_abort_upload() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, UPLOAD_CHUNK_BYTES};
        use tower::ServiceExt;

        let declared_size = (UPLOAD_CHUNK_BYTES * 10) as u64;
        let partial = vec![5u8; UPLOAD_CHUNK_BYTES + 100];

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::BeginUpload {
                req_id,
                channel,
                size,
                ..
            } = request
            else {
                panic!("expected BeginUpload, got {request:?}");
            };
            assert_eq!(size, declared_size);
            writer
                .write_control(&ControlMsg::UploadStarted { req_id, channel })
                .await
                .unwrap();

            // The partial bytes arrive as ordinary data frames...
            let frame = reader.read_frame().await.unwrap().unwrap();
            assert_eq!(frame.body.len(), UPLOAD_CHUNK_BYTES);
            assert!(frame.body.iter().all(|b| *b == 5));

            // ...and then the disconnect must show up as AbortUpload,
            // never a CommitUpload for the (never fully sent) declared
            // size.
            let next = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            assert!(
                matches!(next, ControlMsg::AbortUpload { channel: c } if c == channel),
                "expected AbortUpload after a mid-body disconnect, got {next:?}"
            );
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        // A stream that yields the partial body and then a hard error —
        // the disconnect. `into` converts each item's error into the
        // trait object `Body::from_stream` requires.
        let body_stream = futures_util::stream::iter(vec![
            Ok::<Vec<u8>, std::io::Error>(partial.clone()),
            Err(std::io::Error::other("simulated client disconnect")),
        ]);
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments")
            .header("host", "127.0.0.1:7433")
            .header("content-length", declared_size.to_string())
            .body(axum::body::Body::from_stream(body_stream))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(
            response.status(),
            axum::http::StatusCode::BAD_REQUEST,
            "a body that ends in a transport error must not read as a supervisor-side fault"
        );

        peer.await.unwrap();
    }

    /// An `UploadAborted` arriving mid-stream (the supervisor giving up —
    /// a storage error, its own stall timeout, session deletion) must
    /// reach the browser as the mapped error carrying the reason text —
    /// the pinned REST contract's "500-class with the reason text", never
    /// a bare disconnect or a success.
    #[tokio::test]
    async fn upload_attachment_aborted_mid_stream_maps_to_an_error_with_the_reason() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, UPLOAD_CHUNK_BYTES, UPLOAD_WINDOW_BYTES};
        use tower::ServiceExt;

        const SENTINEL: &str = "SENTINEL-upload-aborted-mid-stream: disk full";
        let total = UPLOAD_WINDOW_BYTES as usize + UPLOAD_CHUNK_BYTES;
        let content = vec![4u8; total];

        let (client_side, peer_side) = tokio::io::duplex(16 * 1024 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::BeginUpload {
                req_id, channel, ..
            } = request
            else {
                panic!("expected BeginUpload, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::UploadStarted { req_id, channel })
                .await
                .unwrap();

            // Drain exactly the initial window, then give up instead of
            // acking — the relay must still be mid-transfer (parked on
            // credit for the chunk past the window) when this arrives.
            let mut received = 0u64;
            while received < UPLOAD_WINDOW_BYTES {
                let frame = reader.read_frame().await.unwrap().unwrap();
                received += frame.body.len() as u64;
            }
            writer
                .write_control(&ControlMsg::UploadAborted {
                    channel,
                    reason: SENTINEL.to_string(),
                })
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments")
            .header("host", "127.0.0.1:7433")
            .header("content-length", total.to_string())
            .body(axum::body::Body::from(content))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(
            response.status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&body),
            SENTINEL,
            "the abort reason must reach the body verbatim"
        );

        peer.await.unwrap();
    }

    /// An `UploadAborted` that arrives while the relay is waiting on the
    /// BROWSER — not on credit — must still end the request promptly and
    /// carry the supervisor's reason verbatim.
    ///
    /// This is where a transfer spends most of its life, and the case the
    /// mid-stream abort test above cannot reach: there, a credit wait
    /// happened to be parked with a receiver subscribed at the exact
    /// instant the abort landed. Here the body is GATED — one chunk, then
    /// nothing, forever — so an implementation that only noticed aborts
    /// while sending would sit until the browser's stall timeout and then
    /// report a stall instead of the storage failure that actually ended
    /// the upload.
    #[tokio::test]
    async fn upload_attachment_abort_while_awaiting_the_body_ends_the_request() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use tower::ServiceExt;

        const SENTINEL: &str = "SENTINEL-abort-awaiting-body: storage went away";

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::BeginUpload {
                req_id, channel, ..
            } = request
            else {
                panic!("expected BeginUpload, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::UploadStarted { req_id, channel })
                .await
                .unwrap();

            let frame = reader.read_frame().await.unwrap().unwrap();
            assert_eq!(frame.body.len(), 4);
            writer
                .write_control(&ControlMsg::UploadAborted {
                    channel,
                    reason: SENTINEL.to_string(),
                })
                .await
                .unwrap();
            (reader, writer)
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        // The sender is held for the whole test, so the body stream stays
        // pending after its one chunk rather than ending.
        let (body_tx, body_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(1);
        body_tx.send(vec![3u8; 4]).await.unwrap();
        let body_stream = futures_util::stream::unfold(body_rx, |mut rx| async move {
            rx.recv()
                .await
                .map(|item| (Ok::<Vec<u8>, std::io::Error>(item), rx))
        });
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments")
            .header("host", "127.0.0.1:7433")
            .header("content-length", "4096")
            .body(axum::body::Body::from_stream(body_stream))
            .unwrap();

        let response = tokio::time::timeout(Duration::from_secs(5), app.oneshot(request))
            .await
            .expect("the abort did not end the request while the body was still pending")
            .unwrap();
        assert_eq!(
            response.status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&body),
            SENTINEL,
            "the abort reason must reach the body verbatim, not be replaced by a stall or a \
             generic failure"
        );

        drop(body_tx);
        let _peer = peer.await.unwrap();
    }

    /// The same retention one step later: an `UploadAborted` racing the
    /// commit must surface as ITS reason, not as whatever the commit
    /// exchange would have collected for a channel the supervisor already
    /// tore down.
    ///
    /// The peer never answers the commit, deliberately. That leaves the
    /// abort as the only thing that can end this request, so an
    /// implementation which dropped the abort — or which waited on the
    /// commit reply without watching for one — hangs instead of quietly
    /// reporting something plausible.
    #[tokio::test]
    async fn upload_attachment_abort_racing_the_commit_surfaces_the_abort_reason() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use tower::ServiceExt;

        const SENTINEL: &str = "SENTINEL-abort-racing-commit: session deleted mid-transfer";

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::BeginUpload {
                req_id, channel, ..
            } = request
            else {
                panic!("expected BeginUpload, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::UploadStarted { req_id, channel })
                .await
                .unwrap();
            let frame = reader.read_frame().await.unwrap().unwrap();
            assert_eq!(frame.body.len(), 4);
            writer
                .write_control(&ControlMsg::UploadAborted {
                    channel,
                    reason: SENTINEL.to_string(),
                })
                .await
                .unwrap();
            (reader, writer)
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        // The body ends normally, so the relay proceeds to commit — into
        // a supervisor that has already given up and will never answer.
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments")
            .header("host", "127.0.0.1:7433")
            .header("content-length", "4")
            .body(axum::body::Body::from(vec![3u8; 4]))
            .unwrap();

        let response = tokio::time::timeout(Duration::from_secs(5), app.oneshot(request))
            .await
            .expect("the request hung waiting for a commit reply that will never come")
            .unwrap();
        assert_eq!(
            response.status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&body), SENTINEL);

        let _peer = peer.await.unwrap();
    }

    /// Empty body chunks are not progress. A stream may legally yield
    /// zero-length items forever, and a relay that rearmed its stall
    /// deadline on every yielded ITEM rather than on relayed BYTES would
    /// keep such an upload — and the supervisor's temp file behind it —
    /// alive indefinitely while transferring nothing.
    ///
    /// Runs on a paused clock, so `CLIENT_UPLOAD_STALL_TIMEOUT` is
    /// observed exactly rather than waited out: the body's own small
    /// sleeps are what let virtual time advance, and the correct
    /// implementation gives up once sixty seconds of them have passed. An
    /// implementation that rearms per item never gives up, exhausts the
    /// (bounded) stream, and fails on the wrong outcome instead of
    /// hanging.
    #[tokio::test(start_paused = true)]
    async fn upload_attachment_endless_empty_body_chunks_stall_out() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use tower::ServiceExt;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::BeginUpload {
                req_id, channel, ..
            } = request
            else {
                panic!("expected BeginUpload, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::UploadStarted { req_id, channel })
                .await
                .unwrap();

            let next = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            assert!(
                matches!(next, ControlMsg::AbortUpload { channel: c } if c == channel),
                "a body that never delivers a byte must be abandoned, not fed to a commit: \
                 {next:?}"
            );
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        // Bounded rather than truly endless: a per-item rearm bug must
        // fail this test on the outcome, not hang the suite. 300 virtual
        // seconds is comfortably past the 60-second deadline.
        let body_stream = futures_util::stream::unfold(0usize, |n| async move {
            if n >= 300 {
                return None;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
            Some((Ok::<Vec<u8>, std::io::Error>(Vec::new()), n + 1))
        });
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments")
            .header("host", "127.0.0.1:7433")
            .header("content-length", "4096")
            .body(axum::body::Body::from_stream(body_stream))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(
            response.status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&body),
            farhelm_proto::UPLOAD_ABORT_REASON_STALLED,
            "a no-progress upload must report the shared stalled reason"
        );

        peer.await.unwrap();
    }

    /// A cancelled handler must still release the supervisor's half of the
    /// transfer.
    ///
    /// This is the pinned contract's "client disconnect before commit =>
    /// AbortUpload" in its harshest form: a reset connection cancels the
    /// axum handler outright, so no error branch of the relay runs at all.
    /// The abort therefore cannot live in a branch — it has to belong to
    /// the upload's owner and fire from its destructor — and this test
    /// drops the handler at the worst moment, parked on a closed credit
    /// window, to prove it does.
    #[tokio::test]
    async fn upload_attachment_cancelled_handler_still_aborts_upstream() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, UPLOAD_CHUNK_BYTES, UPLOAD_WINDOW_BYTES};
        use tower::ServiceExt;

        let total = UPLOAD_WINDOW_BYTES as usize + UPLOAD_CHUNK_BYTES;

        let (client_side, peer_side) = tokio::io::duplex(16 * 1024 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            (reader, writer)
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();
        let (mut peer_reader, mut peer_writer) = peer.await.unwrap();

        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments")
            .header("host", "127.0.0.1:7433")
            .header("content-length", total.to_string())
            .body(axum::body::Body::from(vec![5u8; total]))
            .unwrap();
        let handler = tokio::spawn(app.oneshot(request));

        let begin = parse_control(&peer_reader.read_frame().await.unwrap().unwrap()).unwrap();
        let ControlMsg::BeginUpload {
            req_id, channel, ..
        } = begin
        else {
            panic!("expected BeginUpload, got {begin:?}");
        };
        peer_writer
            .write_control(&ControlMsg::UploadStarted { req_id, channel })
            .await
            .unwrap();

        // Take the whole window and withhold every ack, so the handler is
        // genuinely parked when it is cancelled.
        let mut received = 0u64;
        while received < UPLOAD_WINDOW_BYTES {
            let frame = tokio::time::timeout(Duration::from_secs(5), peer_reader.read_frame())
                .await
                .expect("timed out waiting for the initial window")
                .unwrap()
                .expect("connection closed mid-window");
            received += frame.body.len() as u64;
        }
        assert!(!handler.is_finished());

        handler.abort();

        let frame = tokio::time::timeout(Duration::from_secs(5), peer_reader.read_frame())
            .await
            .expect("a cancelled handler never released the supervisor's upload")
            .unwrap()
            .expect("connection closed before the abort arrived");
        assert!(
            matches!(
                parse_control(&frame).unwrap(),
                ControlMsg::AbortUpload { channel: c } if c == channel
            ),
            "a cancelled handler must abort the transfer it started"
        );
    }

    /// A `BeginUpload` refusal (admission cap, a vanished attachments
    /// directory, any supervisor-side precondition) must reach the browser
    /// through the same `http_error` mapping every other endpoint uses,
    /// with the supervisor's message verbatim — the pinned REST contract's
    /// "sentinel-testable" promise, exercised here specifically for the
    /// attachments route rather than assumed from the other endpoints'
    /// coverage.
    ///
    /// Deliberately aimed at a session the helm DOES know (PLAN_M6.md item
    /// 5): an id the merged view has never heard of is now refused by the
    /// helm's own owner lookup before any host is contacted, so pointing
    /// this at one would test the helm's 404 rather than the passthrough it
    /// exists to pin.
    #[tokio::test]
    async fn upload_attachment_begin_error_reply_passes_through_the_sentinel_message() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, ErrorKind};
        use tower::ServiceExt;

        const SENTINEL: &str = "SENTINEL-begin-upload-7a2c9f: the attachments directory is gone";

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::BeginUpload { req_id, .. } = request else {
                panic!("expected BeginUpload, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::Error {
                    req_id,
                    message: SENTINEL.to_string(),
                    kind: ErrorKind::NotFound,
                })
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments")
            .header("host", "127.0.0.1:7433")
            .header("content-length", "3")
            .body(axum::body::Body::from(vec![1u8, 2, 3]))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&body),
            SENTINEL,
            "body must carry the supervisor's own message verbatim, not a substring of it"
        );

        peer.await.unwrap();
    }

    /// An absent `?filename=` must forward as the empty proposal —
    /// SPEC.md/`ControlMsg::BeginUpload`'s "names are never a refusal"
    /// contract — never a locally-invented default name and never a
    /// refusal. A present-but-empty `?filename=` shares the same decode
    /// (see `UploadQuery`'s own docs), so only the absent case needs its
    /// own test.
    #[tokio::test]
    async fn upload_attachment_absent_filename_forwards_as_the_empty_proposal() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, ErrorKind};
        use tower::ServiceExt;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::BeginUpload {
                req_id, filename, ..
            } = request
            else {
                panic!("expected BeginUpload, got {request:?}");
            };
            assert_eq!(
                filename, "",
                "an absent ?filename= must forward as the empty proposal, never a made-up name"
            );
            writer
                .write_control(&ControlMsg::Error {
                    req_id,
                    message: "stop here; the filename assertion above is this test's point"
                        .to_string(),
                    kind: ErrorKind::Internal,
                })
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments")
            .header("host", "127.0.0.1:7433")
            .header("content-length", "3")
            .body(axum::body::Body::from(vec![1u8, 2, 3]))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(
            response.status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );

        peer.await.unwrap();
    }

    /// A request with no (or an unparsable) `Content-Length` cannot
    /// declare a size to `BeginUpload` at all, and is refused locally
    /// without ever contacting the supervisor — mirroring
    /// `term_ws_with_empty_lease_is_refused_locally_without_contacting_the_supervisor`'s
    /// pattern for the one shape check the helm makes on its own behalf.
    #[tokio::test]
    async fn upload_attachment_missing_content_length_is_refused_locally() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake};
        use tower::ServiceExt;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        // The peer never needs to answer anything — a refusal that
        // touched the supervisor would show up as this connection
        // observing a frame it never expected, so the peer intentionally
        // does nothing but the handshake and is dropped at the end of the
        // test still holding an unread connection.
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            (reader, writer)
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        // Built with no Content-Length at all: the request builder sets
        // no headers of its own, and `Body::from` only gives the body an
        // exact size HINT — nothing synthesizes the header a real client
        // would have sent. This is the raw-client shape the check exists
        // for.
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/attachments")
            .header("host", "127.0.0.1:7433")
            .body(axum::body::Body::from(vec![1u8, 2, 3]))
            .unwrap();
        assert!(
            !request
                .headers()
                .contains_key(axum::http::header::CONTENT_LENGTH),
            "this test is only meaningful while the builder leaves Content-Length absent"
        );

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);

        let _peer = peer.await.unwrap();
    }
}
