//! The production [`TensorlakeApi`]: Tensorlake's REST API over reqwest.
//!
//! One client per helm, bearer-key authentication, JSON both ways. No
//! redirects and no retries: a redirect is not permission to send the key
//! somewhere else, and a lifecycle call (create, terminate) retried behind
//! the caller's back could double an action the caller is about to reconcile
//! by reading the sandbox's state. Every body read is bounded, error bodies
//! more tightly than replies, since an error body is only ever shown.

use super::api::{
    ApiKey, CreateSandbox, CreatedSandbox, CreatedSnapshot, SandboxInfo, SandboxPage, SnapshotInfo,
    SnapshotList, SnapshotType, TensorlakeApi, TensorlakeError,
};
use async_trait::async_trait;
use reqwest::{Method, Url};
use serde::de::DeserializeOwned;
use std::time::Duration;

/// Where Tensorlake's API lives.
pub(crate) const PRODUCTION_BASE: &str = "https://api.tensorlake.ai";

/// The bound on an ordinary call. Listing, suspending or deleting answers in
/// well under a second (measured 2026-11-03); this only stops a hung
/// connection from holding a request forever.
const CALL_TIMEOUT: Duration = Duration::from_secs(30);

/// The bound on a create. A create waits until the sandbox runs by default,
/// and Tensorlake's docs warn that can take minutes when its fleet has to
/// add capacity first.
const CREATE_TIMEOUT: Duration = Duration::from_secs(600);

/// The most of a success body Farhelm reads. A full page of sandboxes is
/// tens of kilobytes; this is generous headroom, not an expected size.
const MAX_REPLY_BYTES: usize = 8 * 1024 * 1024;

/// The most of an error body kept for display, in bytes, and then in
/// characters after cleaning. Error bodies are short plain text in practice
/// ("Sandbox not found").
const MAX_ERROR_BYTES: usize = 4096;
const MAX_ERROR_CHARS: usize = 300;

/// The HTTP client and the API's base URL, which tests point at a stub.
pub(crate) struct HttpTensorlake {
    client: reqwest::Client,
    base: Url,
}

impl HttpTensorlake {
    /// The production client against [`PRODUCTION_BASE`].
    pub(crate) fn production() -> anyhow::Result<Self> {
        Ok(Self::new(
            client_builder().build()?,
            Url::parse(PRODUCTION_BASE).expect("the production base URL parses"),
        ))
    }

    /// Any client against any base: the seam the stub-server tests use.
    pub(crate) fn new(client: reqwest::Client, base: Url) -> Self {
        Self { client, base }
    }

    /// `base` plus the given path segments, each percent-encoded as one
    /// segment, so a sandbox name containing `/` or `?` cannot address
    /// another route.
    fn url(&self, segments: &[&str]) -> Url {
        let mut url = self.base.clone();
        url.path_segments_mut()
            .expect("an http(s) base URL has path segments")
            .pop_if_empty()
            .extend(segments);
        url
    }

    /// Send one request and classify its outcome. `Ok` carries a success
    /// response whose body the caller still has to read.
    async fn send(
        &self,
        method: Method,
        url: Url,
        key: &ApiKey,
        body: Option<serde_json::Value>,
        timeout: Duration,
    ) -> Result<reqwest::Response, TensorlakeError> {
        let mut request = self
            .client
            .request(method, url)
            .bearer_auth(key.expose())
            .header(reqwest::header::ACCEPT, "application/json")
            .timeout(timeout);
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await.map_err(transport)?;
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }
        let message = error_message(response, key).await;
        Err(TensorlakeError::from_status(status.as_u16(), message))
    }

    /// A call whose success body is JSON of type `T`.
    async fn json<T: DeserializeOwned>(
        &self,
        method: Method,
        url: Url,
        key: &ApiKey,
        body: Option<serde_json::Value>,
        timeout: Duration,
    ) -> Result<T, TensorlakeError> {
        let response = self.send(method, url, key, body, timeout).await?;
        let bytes = read_bounded(response, MAX_REPLY_BYTES)
            .await
            .map_err(|_| TensorlakeError::Decode)?;
        serde_json::from_slice(&bytes).map_err(|_| TensorlakeError::Decode)
    }

    /// A call whose success body Farhelm does not need (suspend, resume,
    /// delete answer an empty body or a status line).
    async fn empty(&self, method: Method, url: Url, key: &ApiKey) -> Result<(), TensorlakeError> {
        self.send(method, url, key, None, CALL_TIMEOUT).await?;
        Ok(())
    }
}

/// The production client's policy. Shared with tests so a stub-server test
/// exercises the same redirect and retry rules production has.
pub(crate) fn client_builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .retry(reqwest::retry::never())
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("farhelm/", env!("CARGO_PKG_VERSION")))
}

/// Classify a failure to get any HTTP answer. The URL is stripped first
/// even though it never carries the key, so no request detail reaches the
/// text; reqwest's source chain is dropped for the same reason.
fn transport(error: reqwest::Error) -> TensorlakeError {
    let error = error.without_url();
    TensorlakeError::Transport(if error.is_timeout() {
        "Tensorlake did not answer in time".into()
    } else if error.is_connect() {
        "Farhelm could not connect to Tensorlake".into()
    } else {
        "the connection to Tensorlake failed".into()
    })
}

/// Read at most `limit` bytes of a body; more than that is an error rather
/// than a truncated read, since a truncated JSON reply cannot be trusted.
async fn read_bounded(mut response: reqwest::Response, limit: usize) -> Result<Vec<u8>, ()> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| ())? {
        if bytes.len() + chunk.len() > limit {
            return Err(());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

/// The server's error text, made safe to show: bounded, invalid UTF-8
/// replaced, characters that must never be shown as themselves dropped,
/// whitespace collapsed, and the key redacted should the server echo it.
/// A body that cannot be read becomes an empty message, never an error of
/// its own: the status already says what happened.
async fn error_message(mut response: reqwest::Response, key: &ApiKey) -> String {
    let mut bytes = Vec::new();
    while bytes.len() < MAX_ERROR_BYTES {
        match response.chunk().await {
            Ok(Some(chunk)) => bytes.extend_from_slice(&chunk),
            _ => break,
        }
    }
    bytes.truncate(MAX_ERROR_BYTES);
    clean_message(&String::from_utf8_lossy(&bytes), key)
}

/// The text half of [`error_message`], separate so tests can pin it.
pub(crate) fn clean_message(raw: &str, key: &ApiKey) -> String {
    let raw = if key.expose().is_empty() {
        raw.to_string()
    } else {
        raw.replace(key.expose(), "[API key]")
    };
    let words: Vec<&str> = raw.split_whitespace().collect();
    words
        .join(" ")
        .chars()
        .filter(|c| !farhelm_proto::text::is_presentation_unsafe(*c))
        .take(MAX_ERROR_CHARS)
        .collect()
}

#[async_trait]
impl TensorlakeApi for HttpTensorlake {
    async fn list_sandboxes(
        &self,
        key: &ApiKey,
        cursor: Option<&str>,
    ) -> Result<SandboxPage, TensorlakeError> {
        let mut url = self.url(&["sandboxes"]);
        if let Some(cursor) = cursor {
            url.query_pairs_mut().append_pair("cursor", cursor);
        }
        self.json(Method::GET, url, key, None, CALL_TIMEOUT).await
    }

    async fn get_sandbox(
        &self,
        key: &ApiKey,
        sandbox: &str,
    ) -> Result<SandboxInfo, TensorlakeError> {
        let url = self.url(&["sandboxes", sandbox]);
        self.json(Method::GET, url, key, None, CALL_TIMEOUT).await
    }

    async fn create_sandbox(
        &self,
        key: &ApiKey,
        request: &CreateSandbox,
    ) -> Result<CreatedSandbox, TensorlakeError> {
        let body = serde_json::to_value(request).expect("a create request serializes");
        let url = self.url(&["sandboxes"]);
        self.json(Method::POST, url, key, Some(body), CREATE_TIMEOUT)
            .await
    }

    async fn suspend_sandbox(&self, key: &ApiKey, sandbox: &str) -> Result<(), TensorlakeError> {
        let url = self.url(&["sandboxes", sandbox, "suspend"]);
        self.empty(Method::POST, url, key).await
    }

    async fn resume_sandbox(&self, key: &ApiKey, sandbox: &str) -> Result<(), TensorlakeError> {
        let url = self.url(&["sandboxes", sandbox, "resume"]);
        self.empty(Method::POST, url, key).await
    }

    async fn terminate_sandbox(&self, key: &ApiKey, sandbox: &str) -> Result<(), TensorlakeError> {
        let url = self.url(&["sandboxes", sandbox]);
        self.empty(Method::DELETE, url, key).await
    }

    async fn create_snapshot(
        &self,
        key: &ApiKey,
        sandbox: &str,
        snapshot_type: SnapshotType,
    ) -> Result<CreatedSnapshot, TensorlakeError> {
        let url = self.url(&["sandboxes", sandbox, "snapshot"]);
        let body = serde_json::json!({ "snapshot_type": snapshot_type });
        self.json(Method::POST, url, key, Some(body), CALL_TIMEOUT)
            .await
    }

    async fn get_snapshot(
        &self,
        key: &ApiKey,
        snapshot: &str,
    ) -> Result<SnapshotInfo, TensorlakeError> {
        let url = self.url(&["snapshots", snapshot]);
        self.json(Method::GET, url, key, None, CALL_TIMEOUT).await
    }

    async fn list_snapshots(&self, key: &ApiKey) -> Result<SnapshotList, TensorlakeError> {
        let url = self.url(&["snapshots"]);
        self.json(Method::GET, url, key, None, CALL_TIMEOUT).await
    }

    async fn delete_snapshot(&self, key: &ApiKey, snapshot: &str) -> Result<(), TensorlakeError> {
        let url = self.url(&["snapshots", snapshot]);
        self.empty(Method::DELETE, url, key).await
    }
}
