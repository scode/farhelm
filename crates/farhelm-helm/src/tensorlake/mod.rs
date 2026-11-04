//! Tensorlake sandboxes as hosts: the account side.
//!
//! Farhelm creates and manages Tensorlake sandboxes through Tensorlake's REST
//! API, with an API key the user pastes into Settings → Tensorlake, and
//! reaches them over Tensorlake's ssh gateway with an ssh key of its own.
//! This module owns both credentials and the API client:
//!
//! - [`api`]: the [`TensorlakeApi`] trait every caller uses, its types, and
//!   the error classification; [`http`] is the production implementation and
//!   [`e2e`] the browser suite's fake.
//! - [`config`]: the stored account row (the API key and the "create
//!   sandboxes without asking" choice) and the helm tag.
//! - [`ssh_key`]: Farhelm's own ssh key pair, generated when the user asks.
//! - The [`Tensorlake`] service and the `/api/tensorlake` routes below, which
//!   follow the ntfy settings' write-only rule: no reply ever carries the key.
//!
//! This is the first slice of Tensorlake hosts. Recipes, snapshot builds, the
//! hosts themselves, suspend and resume, and tunnels arrive in follow-up PRs
//! and build on this service, which is why the client already carries the
//! calls they need.

// The client carries the calls the follow-up PRs need (create, suspend,
// snapshots, ...), and only the Test button's listing has a caller yet. The
// unit tests exercise every call, so the lint fires only outside them; the
// expectation is meant to fail, and come off, once the last call is used.
// On `e2e` and `http` it fires through helpers only those unused calls reach
// (the fake's `unimplemented`, the client's `empty` and create timeout), so it
// is not wrong there even though both modules' listing is in use.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "calls typed now for the follow-up Tensorlake PRs")
)]
pub(crate) mod api;
mod config;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "calls typed now for the follow-up Tensorlake PRs")
)]
mod e2e;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "calls typed now for the follow-up Tensorlake PRs")
)]
mod http;
mod ssh_key;

#[cfg(test)]
mod tests;

use crate::{AppState, store::HelmStore};
use api::{ApiKey, Offline, TensorlakeApi, TensorlakeError, list_all_sandboxes};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use config::{SaveError, helm_tag};
use farhelm_proto::tensorlake::{SaveSettings, Settings, SshKeyStatus, TestResult};
use ssh_key::SshKeyStore;
use std::{path::Path, sync::Arc};

/// The helm's Tensorlake account: its stored configuration, the API client,
/// and Farhelm's ssh key. Cheap to clone; every clone shares one client and
/// one key-generation lock.
#[derive(Clone)]
pub(crate) struct Tensorlake(Arc<Inner>);

struct Inner {
    store: HelmStore,
    api: Arc<dyn TensorlakeApi>,
    /// `None` for a helm built without a state directory (the test seam
    /// [`AppState::with_provisioning`]), which has nowhere to keep a key.
    ssh_keys: Option<Arc<SshKeyStore>>,
}

impl Tensorlake {
    /// The helm's service as `AppState::new` builds it: the real API, or
    /// the browser suite's fake when its environment variable names a
    /// marked directory inside the state directory, and the ssh key under
    /// `<state>/tensorlake/`.
    ///
    /// Test builds never read the environment and never reach Tensorlake:
    /// their API is offline unless a test injects one ([`Self::with_api`]).
    pub(crate) fn production(store: HelmStore, state_dir: &Path) -> anyhow::Result<Self> {
        let api: Arc<dyn TensorlakeApi> = if cfg!(test) {
            Arc::new(Offline::test())
        } else {
            production_api(state_dir)?
        };
        let tensorlake_dir = std::path::absolute(state_dir)?.join("tensorlake");
        Ok(Self(Arc::new(Inner {
            store,
            api,
            ssh_keys: Some(Arc::new(SshKeyStore::new(tensorlake_dir))),
        })))
    }

    /// A service that can neither reach Tensorlake nor keep a key: what a
    /// helm assembled without a state directory holds until `AppState::new`
    /// replaces it with the production one.
    pub(crate) fn detached(store: HelmStore) -> Self {
        Self(Arc::new(Inner {
            store,
            api: Arc::new(Offline::test()),
            ssh_keys: None,
        }))
    }

    /// The same service with another API behind it, for tests that point
    /// the client at a stub server or a scripted fake.
    #[cfg(test)]
    pub(crate) fn with_api(&self, api: Arc<dyn TensorlakeApi>) -> Self {
        Self(Arc::new(Inner {
            store: self.0.store.clone(),
            api,
            ssh_keys: self.0.ssh_keys.clone(),
        }))
    }

    /// The settings snapshot the routes return. Read-only: it reports
    /// Farhelm's ssh key as it is on disk and never creates one, so opening
    /// Settings (which mounts every section) leaves no credential behind for
    /// someone who never uses Tensorlake. [`Self::create_ssh_key`] is the only
    /// path that generates it.
    pub(crate) async fn settings(&self) -> anyhow::Result<Settings> {
        let config = self.0.store.tensorlake_config().await?;
        let tag = helm_tag(&self.0.store.registry_namespace().await?);
        let ssh_key = match &self.0.ssh_keys {
            None => no_key_store(),
            Some(keys) => match keys.read().await {
                Ok(Some(key)) => ready(key, &tag),
                Ok(None) => SshKeyStatus::NotCreated,
                Err(reason) => SshKeyStatus::Unavailable { reason },
            },
        };
        Ok(Settings {
            api_key_set: config.api_key.is_some(),
            create_without_asking: config.create_without_asking,
            helm_tag: tag,
            ssh_key,
        })
    }

    /// Generate Farhelm's ssh key if this helm has none, on the user's
    /// explicit request from Settings. An existing key is kept as it is:
    /// replacing it would break a registration the user already made.
    async fn create_ssh_key(&self) -> Result<(), String> {
        let Some(keys) = &self.0.ssh_keys else {
            return Err("this helm keeps no SSH key for Tensorlake".into());
        };
        let tag = helm_tag(
            &self
                .0
                .store
                .registry_namespace()
                .await
                .map_err(|_| "could not read this helm's registry".to_string())?,
        );
        keys.ensure(&key_name(&tag)).await.map(|_| ())
    }

    /// Prove the saved key works by listing the account's sandboxes.
    async fn test(&self) -> Result<TestResult, TestFailure> {
        let config = self
            .0
            .store
            .tensorlake_config()
            .await
            .map_err(|_| TestFailure::Storage)?;
        let key: ApiKey = config.api_key().ok_or(TestFailure::NoKey)?;
        let sandboxes = list_all_sandboxes(self.0.api.as_ref(), &key)
            .await
            .map_err(TestFailure::Api)?;
        Ok(TestResult {
            sandboxes: sandboxes.len() as u64,
        })
    }
}

/// The name the key gets in the user's Tensorlake key list, and its comment:
/// the tag says which helm it belongs to when several share one account.
fn key_name(tag: &str) -> String {
    format!("farhelm-{tag}")
}

/// The state a helm without a key store reports.
fn no_key_store() -> SshKeyStatus {
    SshKeyStatus::Unavailable {
        reason: "this helm keeps no SSH key for Tensorlake".into(),
    }
}

/// An existing key as the UI shows it, with its registration command.
///
/// The command carries the public key's text rather than its file's path,
/// which `tl sbx ssh keys add` accepts too: the browser showing Settings is
/// often on another machine than the helm, and a path would only mean
/// something on the helm's machine. The key line is quoted as one shell word.
fn ready(key: ssh_key::SshKey, tag: &str) -> SshKeyStatus {
    let name = key_name(tag);
    SshKeyStatus::Ready {
        register_command: format!(
            "tl sbx ssh keys add --name {name} {}",
            shell_words::quote(&key.public_key)
        ),
        public_key: key.public_key,
        public_key_path: key.public_key_path.to_string_lossy().into_owned(),
        name,
    }
}

/// The real client, or the browser suite's fake when the helm was started
/// with its environment variable. Reading the environment here, once at
/// startup, is the same opt-in the provisioning backend's fake uses.
fn production_api(state_dir: &Path) -> anyhow::Result<Arc<dyn TensorlakeApi>> {
    if let Some(root) = std::env::var_os(e2e::E2E_TENSORLAKE_ENV) {
        let root = std::path::PathBuf::from(root);
        let fake = e2e::E2eTensorlake::new(root.clone(), state_dir)?;
        eprintln!(
            "WARNING: E2E-only Tensorlake fake enabled from {}; Tensorlake API calls are simulated",
            root.display()
        );
        return Ok(Arc::new(fake));
    }
    match http::HttpTensorlake::production() {
        Ok(client) => Ok(Arc::new(client)),
        // A machine whose TLS setup cannot build a client (no CA bundle,
        // say) still serves everything else; Tensorlake calls then fail
        // with a reason that names the likely fix.
        Err(error) => {
            tracing::warn!(%error, "the Tensorlake HTTP client could not be built");
            Ok(Arc::new(Offline::no_client()))
        }
    }
}

/// Why Test did not list sandboxes.
enum TestFailure {
    NoKey,
    Storage,
    Api(TensorlakeError),
}

/// GET never serializes the stored key.
pub(crate) async fn get(State(state): State<Arc<AppState>>) -> Response {
    match state.tensorlake.settings().await {
        Ok(settings) => axum::Json(settings).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "could not read Tensorlake settings",
        )
            .into_response(),
    }
}

/// Reject malformed JSON with a fixed body: axum's usual diagnostic can quote
/// a submitted string, which is unsafe for a request that carries a key.
pub(crate) async fn put(
    State(state): State<Arc<AppState>>,
    body: Result<axum::Json<SaveSettings>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Ok(axum::Json(save)) = body else {
        return (StatusCode::BAD_REQUEST, "invalid Tensorlake settings").into_response();
    };
    match state.store.save_tensorlake_config(save).await {
        Ok(_) => get(State(state)).await,
        Err(SaveError::Invalid(reason)) => (StatusCode::BAD_REQUEST, reason).into_response(),
        Err(SaveError::Storage) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "could not save Tensorlake settings",
        )
            .into_response(),
    }
}

/// Create Farhelm's ssh key on the user's request and answer the new
/// snapshot. A failure is the reason, for display; an existing key is kept.
pub(crate) async fn create_ssh_key(State(state): State<Arc<AppState>>) -> Response {
    if let Err(reason) = state.tensorlake.create_ssh_key().await {
        return (StatusCode::INTERNAL_SERVER_ERROR, reason).into_response();
    }
    get(State(state)).await
}

/// One user-initiated listing with the saved key. Its reply says how many
/// sandboxes the key can see, or why it could not list them; neither ever
/// carries the key.
pub(crate) async fn test(State(state): State<Arc<AppState>>) -> Response {
    match state.tensorlake.test().await {
        Ok(result) => axum::Json(result).into_response(),
        Err(TestFailure::NoKey) => (
            StatusCode::BAD_REQUEST,
            "save an API key before testing it",
        )
            .into_response(),
        Err(TestFailure::Storage) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "could not read Tensorlake settings",
        )
            .into_response(),
        Err(TestFailure::Api(error @ TensorlakeError::Auth { .. })) => (
            StatusCode::BAD_GATEWAY,
            format!(
                "{error}. Check that the key belongs to the Tensorlake project you want sandboxes in, or paste a new one."
            ),
        )
            .into_response(),
        Err(TestFailure::Api(error)) => {
            (StatusCode::BAD_GATEWAY, error.to_string()).into_response()
        }
    }
}
