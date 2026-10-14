//! The real streaming boundary, with HTTP allowed only in the injected test
//! client. None of these fixtures needs external DNS, TLS or a listening port
//! picked in advance; each owns its listener and task until cleanup.

use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// An owned response server. Dropping after a failed assertion cancels the task
/// and closes its socket, so a failed fixture cannot outlive its test.
struct Server {
    url: String,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Server {
    /// Bind first, then accept exactly one request. A `release` receiver keeps
    /// the response deliberately unsettled for the client's timeout assertion.
    async fn new(response: Vec<u8>, release: Option<tokio::sync::oneshot::Receiver<()>>) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        assert_ne!(
            address.port(),
            0,
            "fixture premise: listener has its allocated port"
        );
        let task = tokio::spawn(async move {
            let (mut stream, _) = tokio::time::timeout(Duration::from_secs(5), listener.accept())
                .await
                .unwrap()
                .unwrap();
            let mut request = vec![0; 4096];
            let count = tokio::time::timeout(Duration::from_secs(5), stream.read(&mut request))
                .await
                .unwrap()
                .unwrap();
            assert!(
                request[..count].starts_with(b"GET /"),
                "fixture premise: actual HTTP request received"
            );
            if let Some(release) = release {
                let _ = release.await;
            }
            let _ = stream.write_all(&response).await;
            let _ = stream.shutdown().await;
        });
        Self {
            url: format!("http://{address}/"),
            task,
        }
    }

    /// Reap the server before reading assertions about its response handling.
    async fn finish(&mut self) {
        tokio::time::timeout(Duration::from_secs(5), &mut self.task)
            .await
            .unwrap()
            .unwrap();
    }
}

/// Disable proxies in this client so host configuration cannot redirect an
/// owned loopback fixture. This does not alter the process environment.
fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap()
}

/// 404 is absence; every other unsuccessful status is a failed fetch. A cap
/// must hold on both complete bodies and streamed chunks, while declared-length
/// truncation must not become a successful partial publication.
#[farhelm_testtrace::test]
async fn statuses_caps_and_truncated_bodies_use_the_real_fetch_boundary() {
    for (response, cap, expected) in [
        (
            "HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\ntest",
            4,
            "body",
        ),
        (
            "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n",
            4,
            "missing",
        ),
        (
            "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n",
            4,
            "403",
        ),
        (
            "HTTP/1.1 300 Multiple Choices\r\nContent-Length: 4\r\n\r\ntest",
            4,
            "300",
        ),
        (
            "HTTP/1.1 302 Found\r\nContent-Length: 4\r\n\r\ntest",
            4,
            "302",
        ),
        ("HTTP/1.1 500 Error\r\nContent-Length: 0\r\n\r\n", 4, "500"),
        (
            "HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\ntests",
            4,
            "over",
        ),
        (
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\ntes\r\n2\r\nts\r\n0\r\n\r\n",
            4,
            "over",
        ),
        (
            "HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\ntest",
            8,
            "body error",
        ),
        (
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nte",
            8,
            "body error",
        ),
    ] {
        let mut server = Server::new(response.as_bytes().to_vec(), None).await;
        let result = fetch_with_client(&client(), &server.url, cap).await;
        server.finish().await;
        match expected {
            "body" => assert_eq!(result.unwrap(), Some(b"test".to_vec())),
            "missing" => assert_eq!(result.unwrap(), None),
            "body error" => {
                assert!(format!("{:#}", result.unwrap_err()).contains("body"));
            }
            text => assert!(format!("{:#}", result.unwrap_err()).contains(text)),
        }
    }
}

/// A request that has no response must hit the client's total deadline, not
/// hang the worker. Release and reap the peer once that boundary was observed.
#[farhelm_testtrace::test]
async fn a_nonresponding_peer_hits_the_client_deadline() {
    let (release, hold) = tokio::sync::oneshot::channel();
    let mut server = Server::new(Vec::new(), Some(hold)).await;
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_millis(500))
        .build()
        .unwrap();
    let result = fetch_with_client(&client, &server.url, 8).await;
    let _ = release.send(());
    server.finish().await;
    let error = result.unwrap_err();
    assert!(
        error.chain().any(|cause| cause
            .downcast_ref::<reqwest::Error>()
            .is_some_and(|e| e.is_timeout())),
        "{error:#}"
    );
}

/// A reserved then closed loopback port gives a real connection refusal with
/// no external service or fixed-port premise. The error must remain an error.
#[farhelm_testtrace::test]
async fn a_refused_connection_is_not_missing_metadata() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let error = fetch_with_client(&client(), &format!("http://{address}/"), 8)
        .await
        .unwrap_err();
    assert!(
        error.chain().any(|cause| cause
            .downcast_ref::<reqwest::Error>()
            .is_some_and(|e| e.is_connect())),
        "{error:#}"
    );
}

/// Make DNS fail inside the real HTTP client, without depending on external
/// DNS configuration or converting that failure into a mere HTTP status.
#[farhelm_testtrace::test]
async fn failed_dns_is_not_missing_metadata() {
    struct FailedDns;
    impl reqwest::dns::Resolve for FailedDns {
        fn resolve(&self, _name: reqwest::dns::Name) -> reqwest::dns::Resolving {
            Box::pin(async { Err(std::io::Error::other("fixture DNS failure").into()) })
        }
    }
    let client = reqwest::Client::builder()
        .no_proxy()
        .dns_resolver(Arc::new(FailedDns))
        .timeout(Duration::from_secs(1))
        .build()
        .unwrap();
    let error = fetch_with_client(&client, "http://fixture.invalid/", 8)
        .await
        .unwrap_err();
    assert!(
        format!("{error:#}").contains("fixture DNS failure"),
        "{error:#}"
    );
}

/// Use the production builder to prove HTTP is refused before a connection,
/// including a URL that would otherwise return valid metadata. Reqwest applies
/// this same scheme gate after redirects; no permissive production seam exists.
#[farhelm_testtrace::test]
async fn the_production_client_refuses_non_https_urls() {
    let client = production_client(Duration::from_secs(1)).unwrap();
    let error = fetch_with_client(&client, "http://127.0.0.1:1/", 8)
        .await
        .unwrap_err();
    assert!(
        error.chain().any(|cause| cause
            .downcast_ref::<reqwest::Error>()
            .is_some_and(|e| e.is_builder())),
        "{error:#}"
    );
    assert!(!error.chain().any(|cause| {
        cause
            .downcast_ref::<reqwest::Error>()
            .is_some_and(|e| e.is_connect())
    }));
}

/// A blocking system resolver can remain alive after HTTP cancellation. Own
/// and release that work on every assertion path; observe wrapper completion
/// while it is still held, so an async-only timeout test cannot hide shutdown.
#[farhelm_testtrace::test]
fn a_blocking_resolver_cannot_hold_the_worker_after_its_deadline() {
    struct BlockingDns {
        entered: std::sync::mpsc::Sender<()>,
        finished: std::sync::mpsc::Sender<()>,
        release: Arc<(Mutex<bool>, Condvar)>,
    }
    impl reqwest::dns::Resolve for BlockingDns {
        fn resolve(&self, _name: reqwest::dns::Name) -> reqwest::dns::Resolving {
            let entered = self.entered.clone();
            let finished = self.finished.clone();
            let release = self.release.clone();
            Box::pin(async move {
                tokio::task::spawn_blocking(move || {
                    let (lock, wake) = &*release;
                    let held = lock.lock().unwrap();
                    entered.send(()).unwrap();
                    let _held = wake.wait_while(held, |released| !*released).unwrap();
                    finished.send(()).unwrap();
                    Err(std::io::Error::other("released fixture resolver").into())
                })
                .await
                .unwrap()
            })
        }
    }
    /// Sample the result before releasing DNS, with cleanup on assertion
    /// failure as well as success. Joining also reaps the worker.
    struct OwnedWorker {
        release: Arc<(Mutex<bool>, Condvar)>,
        worker: Option<std::thread::JoinHandle<()>>,
    }
    impl Drop for OwnedWorker {
        fn drop(&mut self) {
            let (lock, wake) = &*self.release;
            *lock.lock().unwrap() = true;
            wake.notify_all();
            if let Some(worker) = self.worker.take() {
                worker.join().unwrap();
            }
        }
    }
    let release = Arc::new((Mutex::new(false), Condvar::new()));
    let (entered, observed) = std::sync::mpsc::channel();
    let (finished, resolved) = std::sync::mpsc::channel();
    let (result, returned) = std::sync::mpsc::channel();
    let dns = Arc::new(BlockingDns {
        entered,
        finished,
        release: release.clone(),
    });
    let worker = std::thread::spawn(move || {
        let fetched = fetch_on_worker(
            "http://fixture.invalid/",
            8,
            Duration::from_millis(500),
            &|timeout| {
                Ok(reqwest::Client::builder()
                    .no_proxy()
                    .dns_resolver(dns.clone())
                    .timeout(timeout)
                    .build()?)
            },
        );
        result.send(fetched).unwrap();
    });
    let owned = OwnedWorker {
        release: release.clone(),
        worker: Some(worker),
    };
    let premise = observed.recv_timeout(Duration::from_secs(3));
    let held = !*release.0.lock().unwrap();
    let result = returned.recv_timeout(Duration::from_secs(3));
    drop(owned);
    let cleaned = resolved.recv_timeout(Duration::from_secs(3));
    assert!(
        cleaned.is_ok(),
        "owned blocking resolver must finish after release"
    );
    assert!(
        premise.is_ok() && held,
        "fixture premise: resolver entered and remains held"
    );
    let error = result
        .expect("worker must return before resolver is released")
        .unwrap_err();
    assert!(
        error.chain().any(|cause| cause
            .downcast_ref::<reqwest::Error>()
            .is_some_and(|e| e.is_timeout())),
        "{error:#}"
    );
}
