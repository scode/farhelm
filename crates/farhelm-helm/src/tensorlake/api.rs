//! The Tensorlake REST API as Farhelm uses it: one trait, the request and
//! reply types, and the error classification every caller branches on.
//!
//! The trait is the seam. Production talks HTTP ([`super::http`]); the helm's
//! unit tests point that same implementation at a stub server; the browser
//! suite swaps in a filesystem-controlled fake ([`super::e2e`]); and a helm
//! built without either answers every call as offline. Callers never learn
//! which one they hold.
//!
//! Replies are decoded leniently, the opposite of Farhelm's own strict
//! internal protocol: Tensorlake is a young platform that adds fields without
//! notice, so unknown fields are ignored, most fields are optional, and an
//! enum value Farhelm has never seen decodes as `Unknown` rather than failing
//! the whole reply.
//!
//! This PR's Settings section uses only [`TensorlakeApi::list_sandboxes`]. The
//! rest of the trait is the set of calls the follow-up PRs (snapshot builds,
//! Tensorlake hosts, suspend and resume) are written against, typed now so
//! the client and its tests settle in one place.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// A Tensorlake API key, passed explicitly to every call.
///
/// Deliberately neither `Debug` nor `Display`: the key must never reach a log
/// line, an error message or a reply to the browser, and a type that cannot
/// be formatted cannot be logged by accident. [`Self::expose`] is the one way
/// out, for the `Authorization` header and the e2e fake's comparison.
#[derive(Clone)]
pub(crate) struct ApiKey(String);

impl ApiKey {
    pub(crate) fn new(key: String) -> Self {
        Self(key)
    }

    /// The raw key, for the request header and nothing else.
    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

/// A sandbox's lifecycle state as Tensorlake reports it.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SandboxStatus {
    Pending,
    Running,
    Snapshotting,
    Suspending,
    Suspended,
    Terminated,
    /// A resume that failed but can be retried with an explicit resume.
    ResumeFailed,
    /// A state this build does not know. Callers must treat it as "not
    /// known to be usable", never as running.
    #[serde(other)]
    Unknown,
}

/// A sandbox's size. Tensorlake reports CPUs as a fraction-capable number.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub(crate) struct Resources {
    pub(crate) cpus: f64,
    pub(crate) memory_mb: u64,
    pub(crate) disk_mb: u64,
}

/// One sandbox, from a listing or a get. Only `id` and `status` are required;
/// everything else Farhelm may want is optional so a reply that omits it
/// still decodes.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(crate) struct SandboxInfo {
    pub(crate) id: String,
    /// Present for named sandboxes, which are the only ones Farhelm creates
    /// (only a named sandbox can be suspended).
    #[serde(default)]
    pub(crate) name: Option<String>,
    pub(crate) status: SandboxStatus,
    /// Milliseconds since the Unix epoch.
    #[serde(default)]
    pub(crate) created_at: Option<i64>,
    #[serde(default)]
    pub(crate) image: Option<String>,
    #[serde(default)]
    pub(crate) resources: Option<Resources>,
    /// The retained suspend checkpoint, while the sandbox has one.
    #[serde(default)]
    pub(crate) snapshot_id: Option<String>,
    /// Tensorlake's diagnosis of a failed resume or termination, for display.
    #[serde(default)]
    pub(crate) error_details: Option<String>,
    /// Why a terminated sandbox ended (`ImageNotFound`, `no_capacity`, ...):
    /// what explains a create that died.
    #[serde(default)]
    pub(crate) termination_reason: Option<String>,
    /// Why a pending sandbox is still waiting (`no_resources_available`, ...).
    #[serde(default)]
    pub(crate) pending_reason: Option<String>,
    /// Whether traffic through Tensorlake's proxy (an ssh connection
    /// included) wakes the sandbox while it is suspended.
    #[serde(default)]
    pub(crate) auto_resume: Option<bool>,
}

/// One page of `GET /sandboxes`. A `next_cursor` means there is more.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(crate) struct SandboxPage {
    pub(crate) sandboxes: Vec<SandboxInfo>,
    #[serde(default)]
    pub(crate) next_cursor: Option<String>,
}

/// What a new sandbox boots from: a named image or a snapshot ID. Tensorlake
/// takes exactly one of the two fields, which this enum makes impossible to
/// get wrong.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub(crate) enum SandboxSource {
    #[serde(rename = "image")]
    Image(String),
    #[serde(rename = "snapshot_id")]
    Snapshot(String),
}

/// `POST /sandboxes`. Farhelm always names its sandboxes: the name is both
/// the ownership record (the API has no labels) and what makes the sandbox
/// suspendable.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub(crate) struct CreateSandbox {
    pub(crate) name: String,
    #[serde(flatten)]
    pub(crate) source: SandboxSource,
    pub(crate) resources: Resources,
    /// Idle timeout in seconds; `0` asks for the plan's maximum (24 hours on
    /// Usage Credits, unlimited on Pro), which is what Farhelm's hosts use.
    pub(crate) timeout_secs: u64,
    /// Whether proxied traffic may wake the sandbox while it is suspended.
    /// Tensorlake defaults to `true`, and a gateway ssh connection counts as
    /// such traffic, so any stray dial (a reconnect that slipped past a status
    /// check, a tunnel restart, the user's own `ssh`) would silently resume a
    /// suspended sandbox and bill. Farhelm promises never to resume on its
    /// own, so its sandboxes are meant to set `false`: a proxied request then
    /// fails promptly with 409 `SANDBOX_AUTO_RESUME_DISABLED` instead. Always
    /// sent, so the choice is explicit at every call site.
    pub(crate) auto_resume: bool,
    /// Bound capacity waiting at Tensorlake as well as the client's wait.
    /// A lost response can otherwise leave an unbounded pending create.
    pub(crate) max_pending_secs: u64,
}

/// The create reply. A waiting create answers `status`; an accepted, still
/// pending one answers `state`; both mean the same field here.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(crate) struct CreatedSandbox {
    pub(crate) sandbox_id: String,
    #[serde(alias = "state")]
    pub(crate) status: SandboxStatus,
}

/// What a snapshot captures. Farhelm's prepared snapshots are filesystem
/// snapshots, restored by a cold boot of a new sandbox.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SnapshotType {
    Filesystem,
    Memory,
    #[serde(other)]
    Unknown,
}

/// A snapshot's progress. Tensorlake documents `local_ready` (usable for a
/// restore) and `completed` (durably stored); `failed` is assumed by name.
/// Anything else, including the in-progress states and any new way to fail,
/// is `Unknown`, so `Unknown` cannot tell "still working" from "failed in a
/// way this build does not know": a caller polling for `LocalReady` or
/// `Completed` must bound its wait.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SnapshotStatus {
    LocalReady,
    Completed,
    Failed,
    #[serde(other)]
    Unknown,
}

/// The reply to `POST /sandboxes/{id}/snapshot`: the new snapshot's ID, given
/// before the snapshot is ready.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(crate) struct CreatedSnapshot {
    pub(crate) snapshot_id: String,
}

/// One snapshot, from `GET /snapshots` or `GET /snapshots/{id}` (both answer
/// the same shape, checked live on 2026-11-03).
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(crate) struct SnapshotInfo {
    pub(crate) id: String,
    /// The sandbox the snapshot was taken from. Removing a Tensorlake host
    /// deletes every snapshot whose source is its sandbox, because suspend
    /// snapshots outlive the sandbox and keep billing.
    #[serde(default)]
    pub(crate) sandbox_id: Option<String>,
    pub(crate) status: SnapshotStatus,
    #[serde(default)]
    pub(crate) snapshot_type: Option<SnapshotType>,
    #[serde(default)]
    pub(crate) size_bytes: Option<u64>,
    #[serde(default)]
    pub(crate) created_at: Option<i64>,
}

/// `GET /snapshots` has no cursor (checked live on 2026-11-03): one reply
/// lists the whole namespace.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(crate) struct SnapshotList {
    pub(crate) snapshots: Vec<SnapshotInfo>,
}

/// A failed call, classified so callers can tell the cases apart without
/// parsing text. Server messages are kept for display, bounded and stripped
/// of characters that must not be shown as themselves, and with the API key
/// redacted should a server ever echo it.
///
/// The `Display` text is a complete sentence fragment meant for a person;
/// callers add their own guidance (where to fix a rejected key, say).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum TensorlakeError {
    /// 401 or 403: the key is wrong, revoked, or lacks permission.
    #[error("Tensorlake rejected the API key (HTTP {status}){}", suffix(.message))]
    Auth { status: u16, message: String },
    /// 404: the sandbox or snapshot does not exist (any more).
    #[error("Tensorlake has no such sandbox or snapshot (HTTP 404){}", suffix(.message))]
    NotFound { message: String },
    /// 409: a name is taken, or a lifecycle request lost a race.
    #[error("Tensorlake refused the request as a conflict (HTTP 409){}", suffix(.message))]
    Conflict { message: String },
    /// 400 or 422: Tensorlake refused the request itself, which includes a
    /// size the plan does not allow. The message says why.
    #[error("Tensorlake refused the request (HTTP {status}){}", suffix(.message))]
    Refused { status: u16, message: String },
    /// Any other non-success status, typically a server-side failure.
    #[error("Tensorlake answered HTTP {status}{}", suffix(.message))]
    Status { status: u16, message: String },
    /// No HTTP answer at all: DNS, connect, TLS, timeout, or a helm that is
    /// not allowed to make outbound connections.
    #[error("{0}")]
    Transport(String),
    /// A success status whose body Farhelm could not read.
    #[error("Tensorlake sent a reply Farhelm could not read")]
    Decode,
}

/// `": message"`, or nothing when the server sent no message.
fn suffix(message: &str) -> String {
    if message.is_empty() {
        String::new()
    } else {
        format!(": {message}")
    }
}

impl TensorlakeError {
    /// Classify a non-success status with the server's (already cleaned)
    /// message. Success statuses never reach this.
    pub(crate) fn from_status(status: u16, message: String) -> Self {
        match status {
            401 | 403 => Self::Auth { status, message },
            404 => Self::NotFound { message },
            409 => Self::Conflict { message },
            400 | 422 => Self::Refused { status, message },
            _ => Self::Status { status, message },
        }
    }
}

/// Every Tensorlake call Farhelm makes. Each takes the key explicitly, since
/// the user can replace it at any time and the stored copy is the authority.
///
/// Sandbox arguments accept an ID or a name: Tensorlake's lifecycle docs
/// (2026-11-03) address sandboxes by name in the get, suspend, resume and
/// delete paths, and Farhelm addresses its own sandboxes by the names it
/// chose before creating them. Arguments are sent as one percent-encoded path
/// segment, so no value can reach another route.
#[async_trait]
pub(crate) trait TensorlakeApi: Send + Sync {
    /// One page of the sandboxes that have not terminated, any creator's.
    /// `cursor` is the previous page's `next_cursor`.
    async fn list_sandboxes(
        &self,
        key: &ApiKey,
        cursor: Option<&str>,
    ) -> Result<SandboxPage, TensorlakeError>;

    async fn get_sandbox(
        &self,
        key: &ApiKey,
        sandbox: &str,
    ) -> Result<SandboxInfo, TensorlakeError>;

    /// Create a named sandbox; Tensorlake waits until it runs. A
    /// `Transport` failure, the client's timeout included, does NOT mean it
    /// was not created: Tensorlake may still be waiting for capacity and
    /// start it later. Callers reconcile by the name they chose (get it, and
    /// terminate it if it is unwanted).
    async fn create_sandbox(
        &self,
        key: &ApiKey,
        request: &CreateSandbox,
    ) -> Result<CreatedSandbox, TensorlakeError>;

    /// Ask a running named sandbox to suspend. Success means accepted or
    /// already suspended; poll [`Self::get_sandbox`] for the outcome.
    async fn suspend_sandbox(&self, key: &ApiKey, sandbox: &str) -> Result<(), TensorlakeError>;

    /// Ask a suspended sandbox to resume. Acceptance is not readiness.
    async fn resume_sandbox(&self, key: &ApiKey, sandbox: &str) -> Result<(), TensorlakeError>;

    /// Terminate a sandbox (`DELETE`). Tensorlake treats an already
    /// terminated sandbox as success; a sandbox that no longer exists at all
    /// is `NotFound`, which a removal treats as done.
    async fn terminate_sandbox(&self, key: &ApiKey, sandbox: &str) -> Result<(), TensorlakeError>;

    async fn create_snapshot(
        &self,
        key: &ApiKey,
        sandbox: &str,
        snapshot_type: SnapshotType,
    ) -> Result<CreatedSnapshot, TensorlakeError>;

    async fn get_snapshot(
        &self,
        key: &ApiKey,
        snapshot: &str,
    ) -> Result<SnapshotInfo, TensorlakeError>;

    /// Every snapshot in the namespace. Each carries its source `sandbox_id`;
    /// the API has no filter for it, so callers filter.
    async fn list_snapshots(&self, key: &ApiKey) -> Result<SnapshotList, TensorlakeError>;

    async fn delete_snapshot(&self, key: &ApiKey, snapshot: &str) -> Result<(), TensorlakeError>;
}

/// The most pages [`list_all_sandboxes`] follows. At Tensorlake's default of
/// 100 per page that is 5000 sandboxes, far past any account Farhelm serves;
/// the bound exists so a server that keeps answering with a cursor cannot
/// keep a request running forever.
pub(crate) const MAX_SANDBOX_PAGES: usize = 50;

/// Every live sandbox the key can see, following the cursor page by page.
pub(crate) async fn list_all_sandboxes(
    api: &dyn TensorlakeApi,
    key: &ApiKey,
) -> Result<Vec<SandboxInfo>, TensorlakeError> {
    let mut all = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..MAX_SANDBOX_PAGES {
        let page = api.list_sandboxes(key, cursor.as_deref()).await?;
        all.extend(page.sandboxes);
        match page.next_cursor {
            Some(next) if !next.is_empty() => cursor = Some(next),
            _ => return Ok(all),
        }
    }
    // Every page was a 200 answer, so this is not a transport failure; it
    // is classified as a status failure carrying that status.
    Err(TensorlakeError::Status {
        status: 200,
        message: format!(
            "Tensorlake listed more than {MAX_SANDBOX_PAGES} pages of sandboxes; Farhelm stopped reading"
        ),
    })
}

/// The API of a helm that cannot reach Tensorlake. Every call fails as a
/// transport failure carrying the reason, which differs by why the helm has
/// no client.
pub(crate) struct Offline(&'static str);

impl Offline {
    /// A test-built helm that did not inject an API.
    pub(crate) fn test() -> Self {
        Self("this helm cannot make outbound connections")
    }

    /// A production helm whose HTTPS client could not be built, which in
    /// practice means the machine has no CA certificates for TLS (see the
    /// reqwest comment in the root `Cargo.toml`).
    pub(crate) fn no_client() -> Self {
        Self(
            "Farhelm could not set up HTTPS on the helm's machine; check that its CA certificates are installed",
        )
    }

    fn fail<T>(&self) -> Result<T, TensorlakeError> {
        Err(TensorlakeError::Transport(self.0.into()))
    }
}

#[async_trait]
impl TensorlakeApi for Offline {
    async fn list_sandboxes(
        &self,
        _: &ApiKey,
        _: Option<&str>,
    ) -> Result<SandboxPage, TensorlakeError> {
        self.fail()
    }
    async fn get_sandbox(&self, _: &ApiKey, _: &str) -> Result<SandboxInfo, TensorlakeError> {
        self.fail()
    }
    async fn create_sandbox(
        &self,
        _: &ApiKey,
        _: &CreateSandbox,
    ) -> Result<CreatedSandbox, TensorlakeError> {
        self.fail()
    }
    async fn suspend_sandbox(&self, _: &ApiKey, _: &str) -> Result<(), TensorlakeError> {
        self.fail()
    }
    async fn resume_sandbox(&self, _: &ApiKey, _: &str) -> Result<(), TensorlakeError> {
        self.fail()
    }
    async fn terminate_sandbox(&self, _: &ApiKey, _: &str) -> Result<(), TensorlakeError> {
        self.fail()
    }
    async fn create_snapshot(
        &self,
        _: &ApiKey,
        _: &str,
        _: SnapshotType,
    ) -> Result<CreatedSnapshot, TensorlakeError> {
        self.fail()
    }
    async fn get_snapshot(&self, _: &ApiKey, _: &str) -> Result<SnapshotInfo, TensorlakeError> {
        self.fail()
    }
    async fn list_snapshots(&self, _: &ApiKey) -> Result<SnapshotList, TensorlakeError> {
        self.fail()
    }
    async fn delete_snapshot(&self, _: &ApiKey, _: &str) -> Result<(), TensorlakeError> {
        self.fail()
    }
}
