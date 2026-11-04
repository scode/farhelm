//! Tests for the Tensorlake account slice: the HTTP client against an owned
//! stub server (request shapes, lenient decoding, error classification), the
//! stored account row and its migration, the helm tag, Farhelm's ssh key, the
//! browser suite's fake, and the `/api/tensorlake` routes through the real
//! router. No test reaches Tensorlake or changes process configuration: the
//! client is pointed at the stub through its constructor, and the routes get
//! a stub through the harness's injection seam.

use super::api::{
    ApiKey, CreateSandbox, Resources, SandboxSource, SandboxStatus, SnapshotStatus, SnapshotType,
    TensorlakeApi, TensorlakeError, list_all_sandboxes,
};
use super::config::{Config, helm_tag};
use super::e2e::E2eTensorlake;
use super::http::{HttpTensorlake, clean_message, client_builder};
use super::ssh_key::SshKeyStore;
use super::*;
use crate::rest_harness::FleetBuilder;
use axum::body::Body;
use axum::http::{HeaderMap, Request};
use std::collections::VecDeque;
use std::os::unix::fs::PermissionsExt;
use std::sync::Mutex;
use tower::ServiceExt;

/// A key distinctive enough that finding it in any reply, message or log
/// line means it leaked.
const KEY: &str = "tl_apiKey_private-key-marker";

// ---------------------------------------------------------------------------
// The stub Tensorlake server
// ---------------------------------------------------------------------------

/// One request as the stub saw it, whole, so assertions can check the method,
/// the encoded path, the query, the bearer header and the JSON body together.
#[derive(Clone, Debug)]
struct Seen {
    method: String,
    /// The raw (still percent-encoded) path, which is what proves a segment
    /// could not escape into another route.
    path: String,
    query: Option<String>,
    headers: HeaderMap,
    body: String,
}

/// A scripted reply: status, content type and body.
#[derive(Clone)]
struct Reply {
    status: u16,
    content_type: &'static str,
    body: String,
}

/// A JSON success reply.
fn json(body: serde_json::Value) -> Reply {
    Reply {
        status: 200,
        content_type: "application/json",
        body: body.to_string(),
    }
}

/// A plain-text reply with any status, the shape Tensorlake's errors take.
fn text(status: u16, body: &str) -> Reply {
    Reply {
        status,
        content_type: "text/plain",
        body: body.into(),
    }
}

/// An owned loopback server that answers queued replies in order and records
/// every request. Dropping it aborts the server, also when a test fails.
struct Stub {
    base: reqwest::Url,
    seen: Arc<Mutex<Vec<Seen>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Stub {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Stub {
    /// Serve `replies` in order on a kernel-chosen port. A request beyond the
    /// script answers 599, which no test expects, so an extra request fails
    /// its test visibly instead of hanging.
    async fn start(replies: Vec<Reply>) -> Stub {
        let seen: Arc<Mutex<Vec<Seen>>> = Arc::default();
        let queue = Arc::new(Mutex::new(VecDeque::from(replies)));
        let record = seen.clone();
        let app = axum::Router::new().fallback(move |request: axum::extract::Request| {
            let record = record.clone();
            let queue = queue.clone();
            async move {
                let method = request.method().to_string();
                let path = request.uri().path().to_string();
                let query = request.uri().query().map(str::to_string);
                let headers = request.headers().clone();
                let body = axum::body::to_bytes(request.into_body(), 1 << 20)
                    .await
                    .unwrap();
                record.lock().unwrap().push(Seen {
                    method,
                    path,
                    query,
                    headers,
                    body: String::from_utf8(body.to_vec()).unwrap(),
                });
                let reply = queue.lock().unwrap().pop_front().unwrap_or(Reply {
                    status: 599,
                    content_type: "text/plain",
                    body: "unscripted request".into(),
                });
                (
                    StatusCode::from_u16(reply.status).unwrap(),
                    [("content-type", reply.content_type)],
                    reply.body,
                )
                    .into_response()
            }
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Stub {
            base: reqwest::Url::parse(&format!("http://{addr}")).unwrap(),
            seen,
            task,
        }
    }

    /// The production client's policy against this stub, with ambient proxy
    /// configuration excluded so the request really goes to loopback.
    fn client(&self) -> HttpTensorlake {
        HttpTensorlake::new(
            client_builder().no_proxy().build().unwrap(),
            self.base.clone(),
        )
    }

    fn seen(&self) -> Vec<Seen> {
        self.seen.lock().unwrap().clone()
    }
}

fn key() -> ApiKey {
    ApiKey::new(KEY.into())
}

/// A sandbox in the API's own shape, with a field Farhelm does not model.
fn sandbox_json(id: &str, name: &str, status: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id, "name": name, "status": status, "namespace": "ns",
        "created_at": 1_762_000_000_000_i64, "image": "tensorlake/ubuntu-systemd",
        "resources": { "cpus": 2.0, "memory_mb": 4096, "disk_mb": 20480 },
        "timeout_secs": 0, "allow_unauthenticated_access": false, "auto_resume": true,
        "a_field_added_later": { "nested": true },
    })
}

// ---------------------------------------------------------------------------
// The HTTP client
// ---------------------------------------------------------------------------

/// Why: every later Tensorlake feature rides on these requests reaching the
/// documented routes with the key in the bearer header, and a sandbox name
/// must never be able to address another route.
///
/// Spec: each call sends the documented method and path, `Authorization:
/// Bearer <key>`, and (for create and snapshot) the documented JSON body; a
/// name containing `/` and `?` travels as one percent-encoded segment.
#[farhelm_testtrace::test]
async fn every_call_sends_the_documented_request() {
    let stub = Stub::start(vec![
        json(serde_json::json!({ "sandboxes": [], "next_cursor": null })),
        json(sandbox_json("sb-1", "farhelm-host-k7q2-dev-m3x9", "running")),
        json(serde_json::json!({ "sandbox_id": "sb-2", "status": "running" })),
        json(serde_json::json!({ "sandbox_id": "sb-3", "state": "pending" })),
        text(202, ""),
        text(200, ""),
        text(200, ""),
        Reply {
            status: 202,
            content_type: "application/json",
            body: serde_json::json!({ "snapshot_id": "snap-1", "status": "in_progress" })
                .to_string(),
        },
        json(serde_json::json!({ "id": "snap-1", "sandbox_id": "sb-1", "status": "completed", "snapshot_type": "filesystem", "size_bytes": 10, "created_at": 1, "kind": "x" })),
        json(serde_json::json!({ "snapshots": [] })),
        text(200, ""),
    ])
    .await;
    let api = stub.client();
    let key = key();
    api.list_sandboxes(&key, Some("c/ur sor")).await.unwrap();
    let got = api.get_sandbox(&key, "a/b?c").await.unwrap();
    assert_eq!(got.status, SandboxStatus::Running);
    let created = api
        .create_sandbox(
            &key,
            &CreateSandbox {
                name: "farhelm-host-k7q2-dev-m3x9".into(),
                source: SandboxSource::Image("tensorlake/ubuntu-systemd".into()),
                resources: Resources {
                    cpus: 2.0,
                    memory_mb: 4096,
                    disk_mb: 20480,
                },
                timeout_secs: 0,
                auto_resume: false,
            },
        )
        .await
        .unwrap();
    assert_eq!(created.sandbox_id, "sb-2");
    let pending = api
        .create_sandbox(
            &key,
            &CreateSandbox {
                name: "from-snapshot".into(),
                source: SandboxSource::Snapshot("snap-0".into()),
                resources: Resources {
                    cpus: 1.0,
                    memory_mb: 1024,
                    disk_mb: 10240,
                },
                timeout_secs: 0,
                auto_resume: false,
            },
        )
        .await
        .unwrap();
    assert_eq!(pending.status, SandboxStatus::Pending);
    api.suspend_sandbox(&key, "sb-1").await.unwrap();
    api.resume_sandbox(&key, "sb-1").await.unwrap();
    api.terminate_sandbox(&key, "sb-1").await.unwrap();
    let snapshot = api
        .create_snapshot(&key, "sb-1", SnapshotType::Filesystem)
        .await
        .unwrap();
    assert_eq!(snapshot.snapshot_id, "snap-1");
    let info = api.get_snapshot(&key, "snap-1").await.unwrap();
    assert_eq!(info.status, SnapshotStatus::Completed);
    assert_eq!(info.sandbox_id.as_deref(), Some("sb-1"));
    api.list_snapshots(&key).await.unwrap();
    api.delete_snapshot(&key, "snap-1").await.unwrap();

    let seen = stub.seen();
    let requests: Vec<(&str, &str)> = seen
        .iter()
        .map(|seen| (seen.method.as_str(), seen.path.as_str()))
        .collect();
    assert_eq!(
        requests,
        [
            ("GET", "/sandboxes"),
            ("GET", "/sandboxes/a%2Fb%3Fc"),
            ("POST", "/sandboxes"),
            ("POST", "/sandboxes"),
            ("POST", "/sandboxes/sb-1/suspend"),
            ("POST", "/sandboxes/sb-1/resume"),
            ("DELETE", "/sandboxes/sb-1"),
            ("POST", "/sandboxes/sb-1/snapshot"),
            ("GET", "/snapshots/snap-1"),
            ("GET", "/snapshots"),
            ("DELETE", "/snapshots/snap-1"),
        ]
    );
    assert_eq!(seen[0].query.as_deref(), Some("cursor=c%2Fur+sor"));
    for request in &seen {
        assert_eq!(request.headers["authorization"], format!("Bearer {KEY}"));
    }
    let body = |index: usize| serde_json::from_str::<serde_json::Value>(&seen[index].body).unwrap();
    assert_eq!(
        body(2),
        serde_json::json!({
            "name": "farhelm-host-k7q2-dev-m3x9", "image": "tensorlake/ubuntu-systemd",
            "resources": { "cpus": 2.0, "memory_mb": 4096, "disk_mb": 20480 }, "timeout_secs": 0,
            "auto_resume": false,
        })
    );
    assert_eq!(body(3)["snapshot_id"], "snap-0");
    assert!(body(3).get("image").is_none());
    assert_eq!(
        body(7),
        serde_json::json!({ "snapshot_type": "filesystem" })
    );
}

/// Why: Tensorlake adds fields and states without notice, and a strict
/// decoder would turn one new field into "Farhelm cannot read the API".
///
/// Spec: unknown fields are ignored, an unknown sandbox or snapshot status
/// decodes as `Unknown`, and optional fields may be missing entirely.
#[farhelm_testtrace::test]
async fn replies_decode_leniently() {
    let stub = Stub::start(vec![
        json(serde_json::json!({
            "sandboxes": [
                sandbox_json("sb-1", "one", "running"),
                sandbox_json("sb-2", "two", "hibernating-in-a-new-way"),
                { "id": "sb-3", "status": "suspended" },
            ],
            "prev_cursor": null, "next_cursor": null,
            "counts": { "total": 3 },
        })),
        json(serde_json::json!({ "snapshots": [
            { "id": "s1", "status": "uploading", "base_image": "x" },
        ] })),
    ])
    .await;
    let api = stub.client();
    let page = api.list_sandboxes(&key(), None).await.unwrap();
    let statuses: Vec<_> = page.sandboxes.iter().map(|s| s.status).collect();
    assert_eq!(
        statuses,
        [
            SandboxStatus::Running,
            SandboxStatus::Unknown,
            SandboxStatus::Suspended
        ]
    );
    assert_eq!(
        page.sandboxes[0].resources.as_ref().unwrap().memory_mb,
        4096
    );
    assert_eq!(page.sandboxes[2].name, None);
    let snapshots = api.list_snapshots(&key()).await.unwrap();
    assert_eq!(snapshots.snapshots[0].status, SnapshotStatus::Unknown);
    assert_eq!(snapshots.snapshots[0].sandbox_id, None);
}

/// Why: callers branch on the kind of failure (a removal treats "not found"
/// as done, Settings tells a rejected key from an outage, the recipe editor
/// shows a plan refusal's reason), so the classification is the client's
/// real contract.
///
/// Spec: 401 and 403 are `Auth`, 404 `NotFound`, 409 `Conflict`, 400 and 422
/// `Refused` with the server's message, other statuses `Status`; a redirect
/// is not followed; a success body that is not the expected JSON is
/// `Decode`. Messages are whitespace-collapsed, bounded, and stripped of
/// characters that must not be shown as themselves.
#[farhelm_testtrace::test]
async fn failures_are_classified_with_the_servers_message() {
    let stub = Stub::start(vec![
        text(401, ""),
        text(403, "  forbidden\n here "),
        text(404, "Sandbox not found"),
        text(409, "name taken"),
        text(400, "cpus must be at most 4 on your plan"),
        text(422, "bad \u{202E}field"),
        text(503, &"x".repeat(10_000)),
        Reply {
            status: 307,
            content_type: "text/plain",
            body: String::new(),
        },
        json(serde_json::json!({ "not": "a sandbox" })),
    ])
    .await;
    let api = stub.client();
    let key = key();
    let mut errors = Vec::new();
    for _ in 0..9 {
        errors.push(api.get_sandbox(&key, "sb").await.unwrap_err());
    }
    assert_eq!(
        errors[..6],
        [
            TensorlakeError::Auth {
                status: 401,
                message: String::new()
            },
            TensorlakeError::Auth {
                status: 403,
                message: "forbidden here".into()
            },
            TensorlakeError::NotFound {
                message: "Sandbox not found".into()
            },
            TensorlakeError::Conflict {
                message: "name taken".into()
            },
            TensorlakeError::Refused {
                status: 400,
                message: "cpus must be at most 4 on your plan".into()
            },
            TensorlakeError::Refused {
                status: 422,
                message: "bad field".into()
            },
        ]
    );
    let TensorlakeError::Status { status, message } = &errors[6] else {
        panic!("a 503 is a plain status failure, got {:?}", errors[6]);
    };
    assert_eq!(*status, 503);
    assert!(message.chars().count() <= 300);
    assert_eq!(
        errors[7],
        TensorlakeError::Status {
            status: 307,
            message: String::new()
        }
    );
    assert_eq!(errors[8], TensorlakeError::Decode);
    // One request per call: no retry, and the redirect was not followed.
    assert_eq!(stub.seen().len(), 9);
    assert_eq!(
        errors[0].to_string(),
        "Tensorlake rejected the API key (HTTP 401)"
    );
    assert_eq!(
        errors[4].to_string(),
        "Tensorlake refused the request (HTTP 400): cpus must be at most 4 on your plan"
    );
}

/// Why: a server that echoes the request's credentials in an error body
/// would otherwise put the key on screen.
///
/// Spec: the key is replaced in any error message; an empty key replaces
/// nothing.
#[farhelm_testtrace::test]
async fn an_echoed_key_is_redacted() {
    let stub = Stub::start(vec![text(401, &format!("bad token {KEY} given"))]).await;
    let error = stub
        .client()
        .list_sandboxes(&key(), None)
        .await
        .unwrap_err();
    assert_eq!(
        error,
        TensorlakeError::Auth {
            status: 401,
            message: "bad token [API key] given".into()
        }
    );
    assert_eq!(clean_message("a b", &ApiKey::new(String::new())), "a b");
}

/// Why: a connection that never gets an HTTP answer must not look like a
/// rejected key, and its text must not carry request details.
///
/// Spec: a refused connection is `Transport` with a fixed sentence.
#[farhelm_testtrace::test]
async fn an_unreachable_server_is_a_transport_failure() {
    // A bound but non-listening socket reserves a port nobody answers on.
    let socket = tokio::net::TcpSocket::new_v4().unwrap();
    socket.bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let base = reqwest::Url::parse(&format!("http://{}", socket.local_addr().unwrap())).unwrap();
    let api = HttpTensorlake::new(client_builder().no_proxy().build().unwrap(), base);
    assert_eq!(
        api.list_sandboxes(&key(), None).await.unwrap_err(),
        TensorlakeError::Transport("Farhelm could not connect to Tensorlake".into())
    );
    drop(socket);
}

/// Why: the Test button and the later orphan list must see every sandbox, not
/// only the first page, and a server that never stops paging must not hold
/// the request forever.
///
/// Spec: `list_all_sandboxes` follows `next_cursor` until it is absent or
/// empty, passing each cursor back, and gives up after the page bound.
#[farhelm_testtrace::test]
async fn listing_follows_cursors_within_a_bound() {
    let stub = Stub::start(vec![
        json(
            serde_json::json!({ "sandboxes": [sandbox_json("a", "a", "running")], "next_cursor": "p2" }),
        ),
        json(
            serde_json::json!({ "sandboxes": [sandbox_json("b", "b", "suspended")], "next_cursor": "" }),
        ),
    ])
    .await;
    let all = list_all_sandboxes(&stub.client(), &key()).await.unwrap();
    assert_eq!(
        all.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
        ["a", "b"]
    );
    let seen = stub.seen();
    assert_eq!(seen[0].query, None);
    assert_eq!(seen[1].query.as_deref(), Some("cursor=p2"));

    let endless = Stub::start(
        (0..api::MAX_SANDBOX_PAGES)
            .map(|_| json(serde_json::json!({ "sandboxes": [], "next_cursor": "again" })))
            .collect(),
    )
    .await;
    assert!(matches!(
        list_all_sandboxes(&endless.client(), &key()).await,
        Err(TensorlakeError::Status { status: 200, .. })
    ));
    assert_eq!(endless.seen().len(), api::MAX_SANDBOX_PAGES);
}

// ---------------------------------------------------------------------------
// The stored account row and the helm tag
// ---------------------------------------------------------------------------

/// A save that sets only the fields named.
fn save(api_key: Option<&str>, clear: bool, ask: Option<bool>) -> SaveSettings {
    SaveSettings {
        api_key: api_key.map(str::to_string),
        clear_api_key: clear,
        create_without_asking: ask,
    }
}

/// Why: the key is write-only, so a save from a form that never saw it must
/// keep it, and a stale form must not undo another control's save.
///
/// Spec: a blank or absent key keeps the saved one; a pasted key is trimmed
/// and replaces it; clear removes it; a new key together with clear is
/// refused; a key with inner spaces or control characters is refused with a
/// message that does not quote it; an absent toggle keeps the saved choice.
#[farhelm_testtrace::test]
fn saves_keep_replace_and_clear_the_key_explicitly() {
    let with_key = Config::saved(
        Config::default(),
        save(Some(&format!(" {KEY}\n")), false, None),
    )
    .ok()
    .unwrap();
    assert_eq!(with_key.api_key.as_deref(), Some(KEY));
    assert!(!with_key.create_without_asking);
    for blank in [None, Some(""), Some("  \n")] {
        let kept = Config::saved(with_key.clone(), save(blank, false, Some(true)))
            .ok()
            .unwrap();
        assert_eq!(kept.api_key.as_deref(), Some(KEY));
        assert!(kept.create_without_asking);
    }
    let toggled = Config::saved(
        Config {
            create_without_asking: true,
            ..with_key.clone()
        },
        save(None, false, None),
    )
    .ok()
    .unwrap();
    assert!(
        toggled.create_without_asking,
        "an absent toggle keeps the choice"
    );
    let cleared = Config::saved(with_key.clone(), save(None, true, None))
        .ok()
        .unwrap();
    assert_eq!(cleared.api_key, None);
    for refused in [
        save(Some("new-key"), true, None),
        save(Some("two words"), false, None),
        save(Some("tab\tinside"), false, None),
        save(Some(&"k".repeat(4097)), false, None),
    ] {
        let reason = Config::saved(with_key.clone(), refused).err().unwrap();
        assert!(!reason.contains("new-key") && !reason.contains("words"));
    }
}

/// Why: the tag is in the name of every sandbox a helm creates, and the
/// orphan list tells this helm's sandboxes from another's by it. A tag that
/// changed between releases would orphan every existing sandbox.
///
/// Spec: the tag is four lowercase letters or digits, a pure function of the
/// namespace, and pinned for a fixed namespace.
#[farhelm_testtrace::test]
fn the_helm_tag_is_short_stable_and_pinned() {
    let namespace = "0123456789abcdef0123456789abcdef";
    let tag = helm_tag(namespace);
    assert_eq!(tag, helm_tag(namespace));
    assert_eq!(tag.len(), 4);
    assert!(
        tag.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    );
    // Golden: changing the derivation renames every helm's sandboxes.
    assert_eq!(tag, GOLDEN_TAG);
    assert_ne!(helm_tag("fedcba9876543210fedcba9876543210"), tag);
}

/// The tag of the fixed namespace in the test above, recorded when the
/// derivation was written and checked against an independent computation
/// (Python's hashlib, the same domain prefix and base-36 digit order).
const GOLDEN_TAG: &str = "fpps";

/// Why: an upgraded helm must gain the account row without inventing a key
/// or authorizing sandbox creation, and the row must survive reopening.
///
/// Spec: rewinding a database to schema 69 without the table and reopening
/// it creates the row with no key and the toggle off; a saved key and choice
/// persist across reopening.
#[farhelm_testtrace::test]
async fn schema_70_adds_an_empty_account_that_persists() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("helm.db");
    drop(HelmStore::open(&path).await.unwrap());
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch("DROP TABLE tensorlake_account; PRAGMA user_version = 69;")
        .unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE name = 'tensorlake_account'",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        0,
        "fixture premise: schema 69 has no account row"
    );
    drop(conn);
    let store = HelmStore::open(&path).await.unwrap();
    let config = store.tensorlake_config().await.unwrap();
    assert_eq!(config.api_key, None);
    assert!(!config.create_without_asking);
    store
        .save_tensorlake_config(save(Some(KEY), false, Some(true)))
        .await
        .ok()
        .unwrap();
    drop(store);
    let reopened = HelmStore::open(&path).await.unwrap();
    let config = reopened.tensorlake_config().await.unwrap();
    assert_eq!(config.api_key.as_deref(), Some(KEY));
    assert!(config.create_without_asking);
}

// ---------------------------------------------------------------------------
// Farhelm's ssh key
// ---------------------------------------------------------------------------

/// Why: ssh refuses a private key others can read, the key must not change
/// once the user registered it, opening Settings must not leave a credential
/// on a helm whose user never asked for one, and the registration command
/// must work from any machine with `tl`, so it carries the key's text as one
/// shell word.
///
/// Spec: reading the settings reports `NotCreated` and writes nothing; the
/// explicit create generates an ed25519 pair (private key mode 0600, the
/// comment naming the helm tag) inside `<state>/tensorlake/ssh-key/` and
/// removes a leftover temporary folder; the command passes the public key
/// line as one argument; a second create, also through a fresh service as
/// after a restart, keeps the same key. Needs `ssh-keygen` on PATH, as the
/// helm itself does.
#[farhelm_testtrace::test]
async fn the_ssh_key_is_created_only_on_request_with_private_permissions() {
    let dir = tempfile::tempdir().unwrap();
    let state = dir.path().join("state with space");
    std::fs::create_dir(&state).unwrap();
    let store = HelmStore::open(&state.join("helm.db")).await.unwrap();
    let service = Tensorlake::production(store.clone(), &state).unwrap();
    assert_eq!(
        service.settings().await.unwrap().ssh_key,
        SshKeyStatus::NotCreated
    );
    assert!(
        !state.join("tensorlake").exists(),
        "reading the settings wrote nothing"
    );
    std::fs::create_dir_all(state.join("tensorlake/.ssh-key.tmp-leftover")).unwrap();
    service.create_ssh_key().await.unwrap();
    let settings = service.settings().await.unwrap();
    let tag = helm_tag(&store.registry_namespace().await.unwrap());
    let SshKeyStatus::Ready {
        public_key,
        public_key_path,
        name,
        register_command,
    } = settings.ssh_key
    else {
        panic!("the key was not generated: {:?}", settings.ssh_key);
    };
    assert_eq!(settings.helm_tag, tag);
    assert_eq!(name, format!("farhelm-{tag}"));
    assert!(public_key.starts_with("ssh-ed25519 "));
    assert!(public_key.ends_with(&format!(" farhelm-{tag}")));
    let key_dir = state.join("tensorlake/ssh-key");
    assert_eq!(
        public_key_path,
        key_dir.join("id_ed25519.pub").to_str().unwrap()
    );
    assert_eq!(
        register_command,
        format!("tl sbx ssh keys add --name farhelm-{tag} '{public_key}'")
    );
    assert_eq!(
        shell_words::split(&register_command).unwrap(),
        [
            "tl",
            "sbx",
            "ssh",
            "keys",
            "add",
            "--name",
            &name,
            &public_key
        ]
    );
    let mode = std::fs::metadata(key_dir.join("id_ed25519"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
    assert!(!state.join("tensorlake/.ssh-key.tmp-leftover").exists());
    let again = Tensorlake::production(store, &state).unwrap();
    again.create_ssh_key().await.unwrap();
    let SshKeyStatus::Ready {
        public_key: second, ..
    } = again.settings().await.unwrap().ssh_key
    else {
        panic!("the existing key was not found");
    };
    assert_eq!(second, public_key);
}

/// Why: generating over a damaged key folder would silently replace a key
/// the user may have registered, breaking every Tensorlake host at once.
///
/// Spec: a key folder without a readable pair is reported as unavailable,
/// with the folder named, and is left as it was.
#[farhelm_testtrace::test]
async fn a_damaged_key_is_reported_not_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let keys = SshKeyStore::new(dir.path().join("tensorlake"));
    std::fs::create_dir_all(dir.path().join("tensorlake/ssh-key")).unwrap();
    std::fs::write(
        dir.path().join("tensorlake/ssh-key/id_ed25519.pub"),
        "ssh-ed25519 AAAA x",
    )
    .unwrap();
    let reason = keys.ensure("farhelm-test").await.unwrap_err();
    assert!(reason.contains("incomplete or unreadable"), "{reason}");
    assert!(!dir.path().join("tensorlake/ssh-key/id_ed25519").exists());
}

/// Why: Settings must still load (key, toggle, tag) on a helm that cannot
/// keep a key, so the user can see why.
///
/// Spec: a service without a key store reports the key as unavailable with
/// a reason and still returns the rest of the settings.
#[farhelm_testtrace::test]
async fn a_helm_without_a_key_store_still_reads_its_settings() {
    let dir = tempfile::tempdir().unwrap();
    let store = HelmStore::open(&dir.path().join("helm.db")).await.unwrap();
    let settings = Tensorlake::detached(store).settings().await.unwrap();
    assert!(!settings.api_key_set);
    assert!(matches!(
        settings.ssh_key,
        SshKeyStatus::Unavailable { ref reason } if !reason.is_empty()
    ));
}

// ---------------------------------------------------------------------------
// The browser suite's fake
// ---------------------------------------------------------------------------

/// A state directory with a marked fake directory inside it.
fn fake_dirs() -> (tempfile::TempDir, std::path::PathBuf) {
    let state = tempfile::tempdir().unwrap();
    let root = state.path().join("tensorlake-fake");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("ENABLED"), "farhelm-e2e-tensorlake-v1\n").unwrap();
    (state, root)
}

/// Why: the fake replaces Tensorlake for anyone who can set an environment
/// variable, so it must refuse anything but a marked directory inside the
/// helm's own private state directory.
///
/// Spec: a relative path, a path outside the state directory (also through a
/// `..` component or a symlink), and a directory without the exact marker
/// are refused.
#[farhelm_testtrace::test]
fn the_fake_accepts_only_a_marked_directory_inside_the_state_directory() {
    let (state, root) = fake_dirs();
    assert!(E2eTensorlake::new(root.clone(), state.path()).is_ok());
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(
        outside.path().join("ENABLED"),
        "farhelm-e2e-tensorlake-v1\n",
    )
    .unwrap();
    let link = state.path().join("link");
    std::os::unix::fs::symlink(outside.path(), &link).unwrap();
    let escaping = root
        .join("..")
        .join("..")
        .join(outside.path().file_name().unwrap());
    for refused in [
        std::path::PathBuf::from("relative"),
        outside.path().to_path_buf(),
        link,
        escaping,
    ] {
        assert!(
            E2eTensorlake::new(refused.clone(), state.path()).is_err(),
            "{refused:?}"
        );
    }
    std::fs::write(root.join("ENABLED"), "something else\n").unwrap();
    assert!(E2eTensorlake::new(root, state.path()).is_err());
}

/// Why: the browser spec proves the helm sends the key it saved, and that a
/// rejected key reaches the page, through this fake.
///
/// Spec: with no config every key is accepted and the account is empty; with
/// `accepted_key` only that key lists the configured sandboxes and any other
/// is a 401; an injected failure answers its status; every call is logged
/// with whether its key matched, and the log never holds a key.
#[farhelm_testtrace::test]
async fn the_fake_answers_from_its_control_files() {
    let (state, root) = fake_dirs();
    let fake = E2eTensorlake::new(root.clone(), state.path()).unwrap();
    assert!(
        fake.list_sandboxes(&key(), None)
            .await
            .unwrap()
            .sandboxes
            .is_empty()
    );
    std::fs::write(
        root.join("config.json"),
        serde_json::json!({
            "accepted_key": KEY,
            "sandboxes": [sandbox_json("sb-1", "one", "running")],
        })
        .to_string(),
    )
    .unwrap();
    let listed = fake.list_sandboxes(&key(), None).await.unwrap();
    assert_eq!(listed.sandboxes[0].id, "sb-1");
    assert!(matches!(
        fake.list_sandboxes(&ApiKey::new("wrong".into()), None)
            .await,
        Err(TensorlakeError::Auth { status: 401, .. })
    ));
    std::fs::write(
        root.join("config.json"),
        serde_json::json!({ "fail": { "list_sandboxes": { "status": 503, "message": "down" } } })
            .to_string(),
    )
    .unwrap();
    assert_eq!(
        fake.list_sandboxes(&key(), None).await.unwrap_err(),
        TensorlakeError::Status {
            status: 503,
            message: "down".into()
        }
    );
    assert!(matches!(
        fake.suspend_sandbox(&key(), "sb-1").await,
        Err(TensorlakeError::Status { status: 501, .. })
    ));
    let events = std::fs::read_to_string(root.join("events.jsonl")).unwrap();
    assert!(!events.contains(KEY));
    let events: Vec<serde_json::Value> = events
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(
        events,
        [
            serde_json::json!({ "op": "list_sandboxes", "key_accepted": true }),
            serde_json::json!({ "op": "list_sandboxes", "key_accepted": true }),
            serde_json::json!({ "op": "list_sandboxes", "key_accepted": false }),
            serde_json::json!({ "op": "list_sandboxes", "key_accepted": true }),
            serde_json::json!({ "op": "suspend_sandbox", "key_accepted": true }),
        ]
    );
}

// ---------------------------------------------------------------------------
// The routes
// ---------------------------------------------------------------------------

/// A request through the real protected router with the harness's Host.
fn request(method: &str, path: &str, body: Option<String>) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header("host", "127.0.0.1:7433")
        .header("content-type", "application/json")
        .body(body.map_or_else(Body::empty, Body::from))
        .unwrap()
}

/// Status and bounded text of one reply.
async fn call(
    harness: &crate::rest_harness::Harness,
    request: Request<Body>,
) -> (StatusCode, String) {
    let response = harness.router().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 16)
        .await
        .unwrap();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

/// Why: the key is write-only over HTTP, the toggle must persist, and a
/// refusal must not quote what was submitted.
///
/// Spec: GET starts with no key, the toggle off, the helm tag and no ssh
/// key yet; POST `/api/tensorlake/ssh-key` creates one, and a second POST
/// keeps it; PUT saves a key (the reply says only that one is saved), keeps
/// it on a blank save, sets the toggle alone, and clears the key; invalid
/// input, a misspelled field and malformed JSON get fixed messages; no reply
/// carries the key.
#[farhelm_testtrace::test]
async fn the_routes_keep_the_key_write_only() {
    let harness = FleetBuilder::new().await.start().await;
    let (status, body) = call(&harness, request("GET", "/api/tensorlake", None)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let initial: Settings = serde_json::from_str(&body).unwrap();
    assert!(!initial.api_key_set && !initial.create_without_asking);
    assert_eq!(initial.helm_tag.len(), 4);
    assert_eq!(initial.ssh_key, SshKeyStatus::NotCreated, "{body}");
    let create = || request("POST", "/api/tensorlake/ssh-key", None);
    let (status, created) = call(&harness, create()).await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let created: Settings = serde_json::from_str(&created).unwrap();
    assert!(matches!(created.ssh_key, SshKeyStatus::Ready { .. }));
    let (_, again) = call(&harness, create()).await;
    assert_eq!(
        serde_json::from_str::<Settings>(&again).unwrap().ssh_key,
        created.ssh_key,
        "a second create keeps the key"
    );

    let put = |body: serde_json::Value| request("PUT", "/api/tensorlake", Some(body.to_string()));
    let mut replies = Vec::new();
    for (body, key_set, ask) in [
        (serde_json::json!({ "api_key": KEY }), true, false),
        (serde_json::json!({ "api_key": "" }), true, false),
        (
            serde_json::json!({ "create_without_asking": true }),
            true,
            true,
        ),
        (serde_json::json!({}), true, true),
        (serde_json::json!({ "clear_api_key": true }), false, true),
    ] {
        let (status, reply) = call(&harness, put(body)).await;
        assert_eq!(status, StatusCode::OK, "{reply}");
        let settings: Settings = serde_json::from_str(&reply).unwrap();
        assert_eq!(
            (settings.api_key_set, settings.create_without_asking),
            (key_set, ask)
        );
        replies.push(reply);
    }
    for (body, expected) in [
        (
            serde_json::json!({ "api_key": format!("{KEY} more"), }),
            "an API key must be at most 4096 visible ASCII characters with no spaces",
        ),
        (
            serde_json::json!({ "api_key": KEY, "clear_api_key": true }),
            "paste a new API key or clear the saved one, not both",
        ),
        (
            serde_json::json!({ "api_key": { "nested": KEY } }),
            "invalid Tensorlake settings",
        ),
        (
            serde_json::json!({ "create_without_asking": KEY }),
            "invalid Tensorlake settings",
        ),
        (
            serde_json::json!({ "apiKey": KEY }),
            "invalid Tensorlake settings",
        ),
    ] {
        let (status, reply) = call(&harness, put(body)).await;
        assert_eq!(
            (status, reply.as_str()),
            (StatusCode::BAD_REQUEST, expected)
        );
        replies.push(reply);
    }
    let (_, after) = call(&harness, request("GET", "/api/tensorlake", None)).await;
    replies.push(after.clone());
    let after: Settings = serde_json::from_str(&after).unwrap();
    assert!(
        !after.api_key_set && after.create_without_asking,
        "refusals changed nothing"
    );
    assert_eq!(after.ssh_key, created.ssh_key, "the key is generated once");
    for reply in replies {
        assert!(!reply.contains(KEY), "a reply carried the key: {reply}");
    }
}

/// Why: Test is how the user learns the pasted key works; it must use the
/// key that was saved, report the count, and explain a rejection without
/// echoing the key.
///
/// Spec: with no key Test is a 400; with a key it lists sandboxes with that
/// key in the bearer header and answers the count; a 401 from Tensorlake is
/// a 502 whose text names the rejection and what to check; a transport
/// failure is a 502 too.
#[farhelm_testtrace::test]
async fn test_lists_sandboxes_with_the_saved_key() {
    let stub = Stub::start(vec![
        json(serde_json::json!({
            "sandboxes": [sandbox_json("a", "a", "running"), sandbox_json("b", "b", "suspended")],
        })),
        text(401, ""),
    ])
    .await;
    let harness = FleetBuilder::new()
        .await
        .tensorlake_api(Arc::new(stub.client()))
        .start()
        .await;
    let test = || request("POST", "/api/tensorlake/test", None);
    assert_eq!(
        call(&harness, test()).await,
        (
            StatusCode::BAD_REQUEST,
            "save an API key before testing it".into()
        )
    );
    assert!(stub.seen().is_empty(), "no key, no request");
    harness
        .store
        .save_tensorlake_config(save(Some(KEY), false, None))
        .await
        .ok()
        .unwrap();
    let (status, body) = call(&harness, test()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        serde_json::from_str::<TestResult>(&body).unwrap(),
        TestResult { sandboxes: 2 }
    );
    assert_eq!(
        stub.seen()[0].headers["authorization"],
        format!("Bearer {KEY}")
    );
    let (status, body) = call(&harness, test()).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(
        body,
        "Tensorlake rejected the API key (HTTP 401). Check that the key belongs to the Tensorlake project you want sandboxes in, or paste a new one."
    );

    let offline = FleetBuilder::new().await.start().await;
    offline
        .store
        .save_tensorlake_config(save(Some(KEY), false, None))
        .await
        .ok()
        .unwrap();
    let (status, body) = call(&offline, test()).await;
    assert_eq!(
        (status, body.as_str()),
        (
            StatusCode::BAD_GATEWAY,
            "this helm cannot make outbound connections"
        )
    );
}
