//! The transfer client against a scripted HTTP server standing in for the helm's transfer routes.
//!
//! A real helm cannot answer a stat, a download or an upload here (each needs a supervisor and a running session), so
//! this server reads one request the way a helm would, hands it to the test whole (method, target, headers and the
//! body as framed by `Content-Length`), and answers with a scripted status and body, or drops the connection. The
//! real routes' paths, credential check and request decoding are pinned against a real helm in
//! `tests/real_helm.rs`. Every test is bounded, so a regression fails instead of hanging.

use super::*;
use farhelm_proto::{DownloadFileInfo, DownloadFileStatus};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpSocket, TcpStream};

const SECRET: &str = "device-secret";

/// One request as the server read it.
struct Received {
    head: String,
    body: Vec<u8>,
}

impl Received {
    /// A header's value, by case-insensitive name.
    fn header(&self, name: &str) -> Option<&str> {
        self.head.lines().skip(1).find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case(name).then(|| value.trim())
        })
    }
}

/// How the server answers the one request it serves.
enum Script {
    Answer(u16, &'static str),
    /// A valid stat answer padded with whitespace past the client's cap, so only the cap can refuse it.
    Oversized,
    /// Read the request whole, then close without a byte of answer: the helm process dying mid-transfer.
    Drop,
}

/// A server for one connection, scripted by [`Script`].
struct Stub {
    addr: SocketAddr,
    received: oneshot::Receiver<Received>,
    task: tokio::task::JoinHandle<()>,
}

impl Stub {
    async fn start(script: Script) -> Stub {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, received) = oneshot::channel();
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let request = read_request(&mut socket).await;
            let _ = tx.send(request);
            match script {
                Script::Answer(status, body) => {
                    socket
                        .write_all(&answer(status, body.as_bytes()))
                        .await
                        .unwrap();
                }
                Script::Oversized => {
                    let mut body =
                        br#"{"host":"h","path":"/a","size":1,"status":"ready"}"#.to_vec();
                    body.resize(ANSWER_MAX_BYTES + 1, b' ');
                    // The client stops reading at the cap and closes, so this write may fail.
                    let _ = socket.write_all(&answer(200, &body)).await;
                }
                Script::Drop => {}
            }
        });
        Stub {
            addr,
            received,
            task,
        }
    }

    /// The request the server read. Joins the server first, so an assertion that failed inside it fails the test with
    /// its own message rather than as a missing request.
    async fn request(self) -> Received {
        self.task.await.unwrap();
        self.received.await.unwrap()
    }
}

/// An HTTP answer with `status` and `body`, closing the connection after it.
fn answer(status: u16, body: &[u8]) -> Vec<u8> {
    let mut answer = format!(
        "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    answer.extend_from_slice(body);
    answer
}

/// Read a request head, up to and including its blank line.
async fn read_head(socket: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    let mut byte = [0u8; 1];
    while !bytes.ends_with(b"\r\n\r\n") {
        assert_eq!(
            socket.read(&mut byte).await.unwrap(),
            1,
            "the request head ended early"
        );
        bytes.push(byte[0]);
    }
    String::from_utf8(bytes).unwrap()
}

async fn read_request(socket: &mut TcpStream) -> Received {
    let mut received = Received {
        head: read_head(socket).await,
        body: Vec::new(),
    };
    assert_eq!(
        received.header("transfer-encoding"),
        None,
        "the helm frames bodies by Content-Length"
    );
    let length: usize = received
        .header("content-length")
        .map_or(0, |value| value.parse().unwrap());
    received.body = vec![0; length];
    socket.read_exact(&mut received.body).await.unwrap();
    received
}

fn endpoint(addr: SocketAddr) -> HelmEndpoint {
    HelmEndpoint::new(addr, SECRET)
}

async fn bounded<T>(work: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(60), work)
        .await
        .expect("the step finished in time")
}

/// The hover check is the helm's stat route: a POST to the session's `files/stat` with the device credential and the
/// path in a JSON body that asks for no save, answered by the host's evidence. Pinned because the old crate docs
/// called it a GET on the channel, and because the answer's flattened shape (the host's display name and the open
/// hint beside the protocol's path, size and status) is what the disclosure reads. The request's bytes are pinned
/// exactly, as this client has always written them, because helms of other releases decode them.
#[tokio::test]
async fn stat_posts_the_path_and_decodes_the_hosts_evidence() {
    bounded(async {
        let stub = Stub::start(Script::Answer(
            200,
            r#"{"host":"build-box","can_open":true,"path":"/home/u/a.md","size":12,"status":"ready"}"#,
        ))
        .await;
        let stat = spawn(endpoint(stub.addr)).stat("s1", "a.md").await.unwrap();
        assert_eq!(
            stat,
            FileStat {
                host: "build-box".into(),
                can_open: Some(true),
                info: DownloadFileInfo {
                    path: "/home/u/a.md".into(),
                    size: Some(12),
                    status: DownloadFileStatus::Ready,
                },
            }
        );
        let request = stub.request().await;
        assert!(request.head.starts_with("POST /api/sessions/s1/files/stat HTTP/1.1\r\n"));
        assert_eq!(request.header("authorization"), Some("Bearer device-secret"));
        assert_eq!(
            String::from_utf8(request.body).unwrap(),
            r#"{"path":"a.md","save_to_downloads":false,"open":false}"#
        );
    })
    .await;
}

/// A server helm has no native opener and leaves `can_open` out, and a host with no readable metadata sends a null
/// size; the GUI must read both as absent rather than fail the hover, and must read each typed refusal by its
/// snake_case spelling. Specified: that answer, written as such a helm writes it, decodes to no open hint, no size and
/// the named refusal.
#[tokio::test]
async fn a_stat_answer_without_an_open_hint_or_size_decodes_as_absent() {
    bounded(async {
        let stub = Stub::start(Script::Answer(
            200,
            r#"{"host":"build-box","path":"/home/u/a.md","size":null,"status":"not_readable"}"#,
        ))
        .await;
        let stat = spawn(endpoint(stub.addr)).stat("s1", "a.md").await.unwrap();
        assert_eq!((stat.can_open, stat.info.size), (None, None));
        assert_eq!(stat.info.status, DownloadFileStatus::NotReadable);
    })
    .await;
}

/// A click always asks the helm for its native save (the desktop app never takes the bytes itself), with `open` only
/// for the modifier-click, and the saved path comes back with what came of opening. An open that failed is still a
/// successful save: SPEC_impl.md says opening can never turn a save into a failed download.
#[tokio::test]
async fn download_asks_for_the_native_save_and_reports_the_open_outcome() {
    bounded(async {
        for (open, answer, expected) in [
            (false, r#"{"path":"/dl/a.md"}"#, OpenOutcome::NotAsked),
            (true, r#"{"path":"/dl/a.md","opened":true}"#, OpenOutcome::Opened),
            (
                true,
                r#"{"path":"/dl/a.md","opened":false,"open_error":"opening downloaded files is turned off in settings"}"#,
                OpenOutcome::NotOpened("opening downloaded files is turned off in settings".into()),
            ),
        ] {
            let stub = Stub::start(Script::Answer(200, answer)).await;
            let saved = spawn(endpoint(stub.addr))
                .download("s1", "a.md", open)
                .await
                .unwrap();
            assert_eq!(
                saved,
                SavedDownload {
                    path: "/dl/a.md".into(),
                    open: expected
                }
            );
            let request = stub.request().await;
            assert!(request.head.starts_with("POST /api/sessions/s1/files/download HTTP/1.1\r\n"));
            assert_eq!(
                String::from_utf8(request.body).unwrap(),
                format!(r#"{{"path":"a.md","save_to_downloads":true,"open":{open}}}"#)
            );
        }
    })
    .await;
}

/// An upload streams the file from disk with the size the helm declares to the supervisor: `Content-Length` matches
/// the file and the body arrives whole and unchanged across several read chunks, with the proposed name in the query.
/// The helm refuses an upload without `Content-Length`, and a chunked body would never carry one. An empty file goes
/// out as an empty body with `Content-Length: 0`, the one case the streamed body is not used for.
#[tokio::test]
async fn upload_streams_the_whole_file_with_its_content_length() {
    bounded(async {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("shot.png");
        let full: Vec<u8> = (0..UPLOAD_READ_BYTES * 3 + 17)
            .map(|i| (i % 251) as u8)
            .collect();
        for bytes in [full, Vec::new()] {
            std::fs::write(&file, &bytes).unwrap();
            let stub = Stub::start(Script::Answer(200, r#"{"path":"/att/s1/shot.png"}"#)).await;

            let path = spawn(endpoint(stub.addr))
                .upload("s1", file.clone(), "my shot.png")
                .await
                .unwrap();
            assert_eq!(path, "/att/s1/shot.png");
            let request = stub.request().await;
            assert!(
                request.head.starts_with(
                    "POST /api/sessions/s1/attachments?filename=my%20shot.png HTTP/1.1\r\n"
                ),
                "{}",
                request.head
            );
            assert_eq!(
                request.header("authorization"),
                Some("Bearer device-secret")
            );
            assert_eq!(
                request.header("content-length"),
                Some(bytes.len().to_string().as_str())
            );
            assert!(request.body == bytes, "the body arrived changed");
        }
    })
    .await;
}

/// The helm's refusal reaches the GUI as its status and its own words, trimmed, because those words are what make
/// the failure line actionable. For an upload, a 4xx means nothing was published, while a 5xx cannot promise that:
/// the helm also answers 5xx when its supervisor connection dies during the commit. A status the helm never sends
/// (a redirect, which the client does not follow) promises nothing either.
#[tokio::test]
async fn a_refusal_carries_the_helms_status_and_words() {
    bounded(async {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, b"hello").unwrap();
        for (status, outcome) in [
            (404, UploadOutcome::NotPublished),
            (500, UploadOutcome::Unknown),
            (302, UploadOutcome::Unknown),
        ] {
            let stub = Stub::start(Script::Answer(
                status,
                "no such session on this host entry\n",
            ))
            .await;
            let error = spawn(endpoint(stub.addr))
                .upload("s1", file.clone(), "a.txt")
                .await
                .unwrap_err();
            assert_eq!(
                error,
                TransferError::Refused {
                    status,
                    message: "no such session on this host entry".into()
                }
            );
            assert_eq!(error.upload_outcome(), outcome);
            stub.request().await;
        }
    })
    .await;
}

/// A connection that dies fails visibly instead of hanging, and an upload's failure says how far it got: one the
/// helm took whole and never answered may have been published, while one that could not connect at all, or whose
/// local file could not be opened, definitely was not.
#[tokio::test]
async fn a_dead_connection_fails_visibly_and_says_whether_an_upload_may_have_landed() {
    bounded(async {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, b"hello").unwrap();

        let stub = Stub::start(Script::Drop).await;
        let error = spawn(endpoint(stub.addr))
            .upload("s1", file.clone(), "a.txt")
            .await
            .unwrap_err();
        assert_eq!(
            stub.request().await.body,
            b"hello",
            "the premise: the helm got the whole body"
        );
        assert!(matches!(error, TransferError::Lost(_)), "{error:?}");
        assert_eq!(error.upload_outcome(), UploadOutcome::Unknown);

        let stub = Stub::start(Script::Drop).await;
        let error = spawn(endpoint(stub.addr))
            .stat("s1", "a.txt")
            .await
            .unwrap_err();
        assert!(matches!(error, TransferError::Lost(_)), "{error:?}");
        stub.request().await;

        // Hold the port without listening until the connection is refused.
        let reserved = TcpSocket::new_v4().unwrap();
        reserved.bind("127.0.0.1:0".parse().unwrap()).unwrap();
        let closed = reserved.local_addr().unwrap();
        let error = spawn(endpoint(closed))
            .upload("s1", file, "a.txt")
            .await
            .unwrap_err();
        assert!(matches!(error, TransferError::NotSent(_)), "{error:?}");
        assert_eq!(error.upload_outcome(), UploadOutcome::NotPublished);

        let error = spawn(endpoint(closed))
            .upload("s1", dir.path().join("missing"), "missing")
            .await
            .unwrap_err();
        assert!(matches!(error, TransferError::File(_)), "{error:?}");
        assert_eq!(error.upload_outcome(), UploadOutcome::NotPublished);
    })
    .await;
}

/// `TransferError::File` promises nothing was published, which holds only if a file that changed size never completes
/// the declared body: the helm publishes once the declared length has arrived. A shrunk file fails before its bytes
/// run out, and a grown one fails in place of its last chunk, so the stream never yields the declared length either
/// way. (An empty file's check is in `FileBody::open`, which a test cannot race; its body is never streamed.)
#[tokio::test]
async fn a_file_that_changes_size_never_completes_its_declared_body() {
    use futures_util::StreamExt;
    bounded(async {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("growing.log");
        for (declared, actual) in [(UPLOAD_READ_BYTES + 10, UPLOAD_READ_BYTES + 5), (10, 11)] {
            std::fs::write(&file, vec![b'x'; declared]).unwrap();
            let (body, size) = FileBody::open(&file, Arc::default()).await.unwrap();
            assert_eq!(
                size, declared as u64,
                "the premise: the size is measured at open"
            );
            std::fs::write(&file, vec![b'x'; actual]).unwrap();

            let mut yielded = 0;
            let mut stream = std::pin::pin!(body.into_stream());
            let mut failed = false;
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(chunk) => yielded += chunk.len(),
                    Err(_) => failed = true,
                }
            }
            assert!(failed, "{declared} -> {actual}: the change went unnoticed");
            assert!(
                yielded < declared,
                "{declared} -> {actual}: the stream handed over the whole declared body"
            );
        }
    })
    .await;
}

/// A file cut short while its upload is on the wire must fail as `TransferError::File`, which promises nothing was
/// published, and not as a lost connection, which would leave the GUI saying the outcome is unknown. This guards the
/// mapping from reqwest's opaque body error back to the reading failure. The file is far larger than the loopback
/// socket buffers, and the server stops reading after the request head, so the client is still reading the file when
/// the test truncates it.
#[tokio::test]
async fn a_file_cut_short_mid_upload_fails_as_unpublished() {
    const SIZE: usize = 32 * 1024 * 1024;
    bounded(async {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("big.bin");
        std::fs::write(&file, vec![b'x'; SIZE]).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (head_tx, head) = oneshot::channel();
        let (go, go_rx) = oneshot::channel::<()>();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            read_head(&mut socket).await;
            head_tx.send(()).unwrap();
            go_rx.await.unwrap();
            // Drain until the client gives up; a reset is as good an ending as a close.
            let mut body = Vec::new();
            let _ = socket.read_to_end(&mut body).await;
            body.len()
        });

        let upload = spawn(endpoint(addr)).upload("s1", file.clone(), "big.bin");
        head.await.unwrap();
        std::fs::OpenOptions::new()
            .write(true)
            .open(&file)
            .unwrap()
            .set_len(0)
            .unwrap();
        go.send(()).unwrap();

        let error = upload.await.unwrap_err();
        assert!(matches!(error, TransferError::File(_)), "{error:?}");
        assert_eq!(error.upload_outcome(), UploadOutcome::NotPublished);
        assert!(
            server.await.unwrap() < SIZE,
            "the premise: the body was cut short"
        );
    })
    .await;
}

/// Dropping a transfer's future (the pointer left the link, the terminal closed) must abandon the request, not leave
/// it running unseen: the connection closes, which is what makes the helm stop the work on its side. This is a stat
/// whose request went out whole; the next test drops an upload mid-body.
#[tokio::test]
async fn dropping_a_transfer_closes_its_connection() {
    bounded(async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let transfers = spawn(endpoint(listener.local_addr().unwrap()));
        let mut stat = transfers.stat("s1", "a.txt");
        let (mut socket, _) = tokio::select! {
            accepted = listener.accept() => accepted.unwrap(),
            _ = &mut stat => panic!("the stat answered without a helm"),
        };
        read_request(&mut socket).await;
        drop(stat);
        let mut rest = Vec::new();
        assert_eq!(
            socket.read_to_end(&mut rest).await.unwrap(),
            0,
            "the client closed the connection"
        );
    })
    .await;
}

/// An upload dropped mid-body must never finish its body, because the helm publishes once the declared length has
/// arrived: an abandoned attachment would land anyway. hyper's connection task owns the body once the request goes
/// out, so this pins that dropping the caller's future stops it there, for a file and for bytes from memory (a whole
/// in-memory body is one hyper finishes writing after the drop, which is why bytes are streamed too). The server reads
/// only the request head, so the client is blocked mid-body when the test drops it.
///
/// NOTE: That premise assumes default loopback socket buffers, far smaller than 32 MiB. On a host tuned to buffer
/// that much, the whole body fits before the drop and this test fails; it cannot pass falsely.
#[tokio::test]
async fn dropping_an_upload_mid_body_never_completes_its_body() {
    const SIZE: usize = 32 * 1024 * 1024;
    bounded(async {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("big.bin");
        std::fs::write(&file, vec![b'x'; SIZE]).unwrap();
        for source in ["file", "bytes"] {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let transfers = spawn(endpoint(listener.local_addr().unwrap()));
            let upload = match source {
                "file" => transfers.upload("s1", file.clone(), "big.bin"),
                _ => transfers.upload_bytes("s1", vec![b'x'; SIZE], "big.bin"),
            };
            let (mut socket, _) = listener.accept().await.unwrap();
            let head = read_head(&mut socket).await;
            assert!(
                head.to_ascii_lowercase()
                    .contains(&format!("content-length: {SIZE}\r\n")),
                "the premise ({source}): the request declared the whole body: {head}"
            );

            drop(upload);
            let mut body = Vec::new();
            // A reset is as good an ending as a close; either way the body stopped short.
            let _ = socket.read_to_end(&mut body).await;
            assert!(
                body.len() < SIZE,
                "the abandoned {source} upload sent its whole body"
            );
        }
    })
    .await;
}

/// A pasted image arrives as bytes the GUI cannot write to a file of its own, so it is uploaded from memory: the same
/// route, query and credential as a file, with `Content-Length` and the body exactly the bytes.
#[tokio::test]
async fn an_upload_from_memory_sends_exactly_its_bytes() {
    bounded(async {
        let bytes: Vec<u8> = (0..UPLOAD_READ_BYTES * 2 + 3)
            .map(|i| (i % 7) as u8)
            .collect();
        let stub = Stub::start(Script::Answer(200, r#"{"path":"/att/s1/pasted-1.png"}"#)).await;
        let path = spawn(endpoint(stub.addr))
            .upload_bytes("s1", bytes.clone(), "pasted-1.png")
            .await
            .unwrap();
        assert_eq!(path, "/att/s1/pasted-1.png");
        let request = stub.request().await;
        assert!(
            request.head.starts_with(
                "POST /api/sessions/s1/attachments?filename=pasted-1.png HTTP/1.1\r\n"
            )
        );
        assert_eq!(
            request.header("authorization"),
            Some("Bearer device-secret")
        );
        assert_eq!(
            request.header("content-length"),
            Some(bytes.len().to_string().as_str())
        );
        assert!(request.body == bytes, "the body arrived changed");
    })
    .await;
}

/// A success answer bigger than any the routes send is a broken peer, and the client stops reading it at its cap
/// instead of holding it all in memory, as the UI channel caps its messages. The answer is valid JSON padded with
/// whitespace, which would decode without the cap, and the error names the cap, so a decode failure cannot pass for
/// it.
#[tokio::test]
async fn an_oversized_answer_is_refused_at_the_cap() {
    bounded(async {
        let stub = Stub::start(Script::Oversized).await;
        let error = spawn(endpoint(stub.addr))
            .stat("s1", "a.txt")
            .await
            .unwrap_err();
        let TransferError::BadReply(reason) = &error else {
            panic!("expected the cap's refusal, got {error:?}");
        };
        assert!(
            reason.contains(&ANSWER_MAX_BYTES.to_string()),
            "the refusal does not name the cap: {reason}"
        );
        stub.request().await;
    })
    .await;
}

/// When the helm refuses a large upload right after its head (no such session, a disconnected host), it answers and
/// closes with the body unread. Depending on timing, hyper may then hand over the 404 or may report the failed send
/// as `Lost`; both have been seen on the same machine. Either way the failure must arrive within the bound and say
/// something true: the 404 that nothing was published (with the helm's words, or none if the body was lost after the
/// status), and `Lost` that the outcome is unknown, as `TransferError::upload_outcome` documents.
#[tokio::test]
async fn an_early_refusal_of_a_large_upload_fails_truthfully_either_way() {
    const SIZE: usize = 32 * 1024 * 1024;
    bounded(async {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("big.bin");
        std::fs::write(&file, vec![b'x'; SIZE]).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            read_head(&mut socket).await;
            socket
                .write_all(&answer(404, b"no such session on this host entry\n"))
                .await
                .unwrap();
        });
        let error = spawn(endpoint(addr))
            .upload("s1", file, "big.bin")
            .await
            .unwrap_err();
        server.await.unwrap();
        match &error {
            TransferError::Lost(_) => assert_eq!(error.upload_outcome(), UploadOutcome::Unknown),
            TransferError::Refused {
                status: 404,
                message,
            } => {
                assert!(
                    message.is_empty() || message == "no such session on this host entry",
                    "{message:?}"
                );
                assert_eq!(error.upload_outcome(), UploadOutcome::NotPublished);
            }
            other => panic!("neither the refusal nor a lost answer: {other:?}"),
        }
    })
    .await;
}
