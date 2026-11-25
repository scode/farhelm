//! Delivery contracts over an owned filesystem and the existing provisioning
//! fake. Counters include reads and home recovery, so an unchanged reconnect
//! proves absence of remote work rather than merely absence of a final write.

use super::*;
use crate::provisioning::tests::FakeBackend;
use farhelm_proto::credentials::{CreateCredential, CredentialKind, SaveCredential};
use farhelm_proto::tensorlake::recipes::EndpointSave;
use farhelm_supervisor::recipe_credentials::CODEX_ENDPOINT_TOKEN_ENV;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::Ordering;
use std::task::{Context, Waker};

const FIRST: &str = "fictional-first-endpoint-token";
const SECOND: &str = "fictional-replacement-endpoint-token";
const THIRD: &str = "fictional-third-endpoint-token";
const CODEX_FIRST: &str =
    r#"{"tokens":{"refresh_token":"fictional-codex-seed"},"vendor_field":true}"#;
const CODEX_REFRESHED: &str = r#"{"tokens":{"refresh_token":"fictional-codex-refreshed"}}"#;

/// Both providers resolve current named selections and retain home/stamps in
/// the same guarded record. A Podman row has no legacy Tensorlake binding,
/// which catches a hook that silently rejects containers before delivery.
#[farhelm_testtrace::test]
async fn container_connections_resolve_shared_recipe_selections_and_delivery_records() {
    let f = Fixture::new(false).await;
    let place = f
        .store
        .list_hosts()
        .await
        .unwrap()
        .into_iter()
        .find(|row| row.kind.can_host_environments())
        .unwrap()
        .id;
    let host = f
        .store
        .register_session_container(crate::store::NewPodmanHost {
            name: "farhelm-sess-credential".into(),
            recipe: "recipe".into(),
            place_host: place,
            user: "image-user".into(),
            home: "/home/image-user".into(),
            uid: 1000,
        })
        .await
        .unwrap();
    let row = f
        .store
        .list_hosts()
        .await
        .unwrap()
        .into_iter()
        .find(|row| row.id == host)
        .unwrap();
    assert!(
        row.tensorlake.is_none() && row.podman.is_some(),
        "premise: this is a real decoded container binding"
    );
    assert!(f.delivery.read_bindings(&row).await.unwrap().uses("work"));
    f.delivery
        .remember_home(&row, "/home/image-user".into())
        .await
        .unwrap();
    assert_eq!(
        f.delivery.read_record(&row).await.unwrap().home.as_deref(),
        Some(Path::new("/home/image-user"))
    );
}

/// The one stamped Codex cleanup must keep edits saved while it is running. Two competing saves exhaust the retry
/// without enabling the new endpoint or stamping cleanup as complete.
#[farhelm_testtrace::test]
async fn concurrent_settings_edits_are_merged_once_then_warn_and_retry_on_connect() {
    for conflicts in [1, 2] {
        let f = Fixture::new(true).await;
        let path = f.home.join(".codex/config.toml");
        tokio::fs::create_dir(path.parent().unwrap()).await.unwrap();
        tokio::fs::write(&path, legacy_codex_config("old"))
            .await
            .unwrap();
        f.stamp_legacy_codex().await;
        let editor_path = path.clone();
        let backend = Arc::downgrade(&f.backend);
        *f.backend.on_operation.lock().unwrap() = Some((
            "write-unit",
            Box::new(move || {
                std::fs::write(&editor_path, legacy_codex_config("first")).unwrap();
                if conflicts == 2 {
                    *backend.upgrade().unwrap().on_operation.lock().unwrap() = Some((
                        "write-unit",
                        Box::new(move || {
                            std::fs::write(editor_path, legacy_codex_config("second")).unwrap();
                        }),
                    ));
                }
            }),
        ));
        let _lease = f.delivery.enter(&f.row);
        f.delivery.connect(&f.row, &f.events).await;
        assert_eq!(
            f.operations()
                .iter()
                .filter(|op| *op == "write-unit")
                .count(),
            2
        );
        let record = f.delivery.read_record(&f.row).await.unwrap();
        if conflicts == 2 {
            assert_eq!(
                tokio::fs::read_to_string(&path).await.unwrap(),
                legacy_codex_config("second")
            );
            assert_eq!(
                f.delivery.warning(f.row.id).as_deref(),
                Some(
                    "Codex recipe credential cleanup is pending (could not install the agent settings)"
                )
            );
            assert!(matches!(record.codex, Some(EndpointStamp::Endpoint { .. })));
            assert!(record.codex_environment.is_none());
            f.delivery.connect(&f.row, &f.events).await;
        } else {
            assert_eq!(record.codex, Some(EndpointStamp::Cleared));
        }
        assert_eq!(f.delivery.warning(f.row.id), None);
        let settings: toml_edit::DocumentMut = tokio::fs::read_to_string(&path)
            .await
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(
            settings["editor"].as_str(),
            Some(if conflicts == 2 { "second" } else { "first" })
        );
        assert!(!settings.contains_key("model_provider"));
        assert!(
            settings["model_providers"]["farhelm"]
                .as_table_like()
                .unwrap()
                .get("experimental_bearer_token")
                .is_none()
        );
        assert_eq!(
            f.snapshot().await.env[recipe_credentials::CODEX_ENDPOINT_TOKEN_ENV],
            FIRST
        );
    }
}

/// Model an older settings write that a concurrent user save still carries along with their own value.
fn legacy_codex_config(editor: &str) -> String {
    format!(
        "editor = '{editor}'\nmodel_provider = 'farhelm'\n[model_providers.farhelm]\nexperimental_bearer_token = 'fictional-legacy'\n"
    )
}

/// Own both sides of delivery until every file and durable stamp is inspected.
struct Fixture {
    _root: tempfile::TempDir,
    home: PathBuf,
    store: HelmStore,
    row: HostRow,
    backend: Arc<FakeBackend>,
    delivery: Arc<CredentialDelivery>,
    events: Arc<FleetEvents>,
}

impl Fixture {
    /// A durable stamp grants cleanup authority over the fixture's prior Codex write.
    async fn stamp_legacy_codex(&self) {
        let entry = self.delivery.entry(&self.row);
        let guard = entry.serial.clone().lock_owned().await;
        let mut record = self.delivery.read_record(&self.row).await.unwrap();
        record.codex = Some(EndpointStamp::Endpoint {
            base_url: "https://legacy.example/v1".into(),
            credential: Some(self.stamp("work", 1).await),
        });
        self.delivery
            .write_record(&self.row, &record, guard)
            .await
            .unwrap();
    }

    /// Decode the file delivered for the next spawn, so a token shared by Claude and Codex cannot mask a missing slot.
    async fn snapshot(&self) -> RecipeCredentials {
        let snapshot: RecipeCredentials =
            serde_json::from_slice(&tokio::fs::read(self.claude_path()).await.unwrap()).unwrap();
        // The supervisor rejects a Codex bearer with no matching endpoint flag, including when cleanup withheld it.
        let endpoint = snapshot.codex_endpoint.as_ref();
        let has_bearer = snapshot.env.contains_key(CODEX_ENDPOINT_TOKEN_ENV);
        assert_eq!(endpoint.is_some_and(|entry| entry.has_token), has_bearer);
        snapshot
    }

    /// Registration uses the real recipe/reference store; no supervisor or
    /// vendor service is needed to exercise serialized endpoint convergence.
    async fn new(codex: bool) -> Self {
        let root = tempfile::tempdir().unwrap();
        let store = HelmStore::open(&root.path().join("helm.db")).await.unwrap();
        let mut fixture = Self::with_store(root, store, codex).await;
        fixture.admit().await;
        fixture
    }

    /// Actor cases share the manager's store rather than planting a parallel
    /// database that the connection hook could never actually consult.
    async fn with_store(root: tempfile::TempDir, store: HelmStore, codex: bool) -> Self {
        let home = root.path().join("home");
        tokio::fs::create_dir(&home).await.unwrap();
        store
            .create_credential(CreateCredential {
                name: "work".into(),
                kind: CredentialKind::EndpointToken,
                secret: FIRST.into(),
            })
            .await
            .unwrap();
        let mut recipe = crate::tensorlake::recipes::tests_support::save();
        recipe.claude = Some(endpoint("https://first.example/v1", "work"));
        recipe.codex = codex.then(|| endpoint("https://codex.example/v1", "work"));
        store
            .create_tensorlake_recipe("recipe".into(), recipe)
            .await
            .unwrap()
            .unwrap();
        let id = crate::tensorlake::hosts::test_support::plant(
            &store,
            "fixture host",
            "sandbox",
            "recipe",
            Some("sandbox-id"),
        )
        .await;
        let row = store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == id)
            .unwrap();
        let backend = FakeBackend::absent(home.clone());
        backend.stateful.store(true, Ordering::SeqCst);
        let delivery = CredentialDelivery::new(store.clone(), backend.clone());
        Self {
            _root: root,
            home,
            store,
            row,
            backend,
            delivery,
            events: Arc::new(FleetEvents::new()),
        }
    }

    /// Model real setup's copied-state admission before delivery can use the
    /// supervisor directory. Both file destinations stay under this fixture.
    async fn admit(&mut self) {
        let state = self._root.path().join("supervisor-state");
        assert!(
            self.store
                .admit_tensorlake_install(self.row.id, "/fixture/farhelm", state.to_str().unwrap())
                .await
                .unwrap()
        );
        self.row = self
            .store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == self.row.id)
            .unwrap();
    }

    /// A connection lease is a fixture premise, not a consequence of connect:
    /// the actor owns it before the hook starts reading selected credentials.
    async fn connected(&self) -> ConnectionLease {
        let lease = self.delivery.enter(&self.row);
        self.delivery.connect(&self.row, &self.events).await;
        assert_eq!(self.delivery.warning(self.row.id), None);
        assert!(self.claude().await.contains(FIRST));
        lease
    }

    /// Commit before push, matching the real replacement handler's contract.
    async fn replace(&self, name: &str, value: &str) {
        self.store
            .save_credential(
                name.into(),
                SaveCredential {
                    secret: Some(value.into()),
                },
            )
            .await
            .unwrap();
    }

    /// Inspect only the fixture-owned destination; no operator configuration
    /// is involved in the byte-level delivery assertions.
    async fn claude(&self) -> String {
        tokio::fs::read_to_string(self.claude_path()).await.unwrap()
    }

    /// Use the consumer's filename contract to observe the exact delivery.
    fn claude_path(&self) -> PathBuf {
        farhelm_supervisor::recipe_credentials::host_file(Path::new(
            self.row.remote_state_dir.as_deref().unwrap(),
        ))
    }

    /// Every transport operation, including reads, participates in this oracle.
    fn operations(&self) -> Vec<String> {
        self.backend.operations.lock().unwrap().clone()
    }

    /// Read expected catalog identity independently of the delivery record.
    /// Name/revision alone cannot distinguish a recreated credential.
    async fn stamp(&self, name: &str, revision: u64) -> (String, u64, String) {
        let name = name.to_owned();
        self.store
            .conn()
            .call("fixture catalog identity", move |conn| {
                let incarnation = conn.query_row(
                    "SELECT incarnation FROM credentials WHERE name = ?1",
                    [&name],
                    |row| row.get(0),
                )?;
                Ok((name, revision, incarnation))
            })
            .await
            .unwrap()
    }

    /// Keep the endpoint constant while changing the GitHub selection, so a
    /// later failure cannot make already successful endpoint work run again.
    async fn select_github(&self, name: Option<&str>) {
        let mut recipe = crate::tensorlake::recipes::tests_support::save();
        recipe.claude = Some(endpoint("https://first.example/v1", "work"));
        recipe.github_token_credential = name.map(str::to_owned);
        self.store
            .update_tensorlake_recipe("recipe".into(), recipe)
            .await
            .unwrap()
            .unwrap();
    }

    /// Create one write-only GitHub identity with a marker distinct from the
    /// endpoint, allowing file and durable-record privacy assertions.
    async fn github_credential(&self) {
        self.store
            .create_credential(CreateCredential {
                name: "git-work".into(),
                kind: CredentialKind::GithubToken,
                secret: "fictional-github-first".into(),
            })
            .await
            .unwrap();
    }

    /// The same supervisor-owned destination used by real host spawning.
    fn github_path(&self) -> PathBuf {
        farhelm_supervisor::github_token::host_file(Path::new(
            self.row.remote_state_dir.as_deref().unwrap(),
        ))
    }

    /// Seed only the catalog. The host file must be created by actual delivery,
    /// so the test cannot accidentally prove its own fixture write instead.
    async fn codex_credential(&self) {
        self.store
            .create_credential(CreateCredential {
                name: "codex-work".into(),
                kind: CredentialKind::CodexLogin,
                secret: CODEX_FIRST.into(),
            })
            .await
            .unwrap();
    }

    /// Change only Codex's selected mode, keeping the endpoint and optional
    /// GitHub slot available to exercise earlier/later partial successes.
    async fn select_codex(&self, login: bool, api_endpoint: bool, github: bool) {
        let mut recipe = crate::tensorlake::recipes::tests_support::save();
        recipe.claude = Some(endpoint("https://first.example/v1", "work"));
        recipe.codex = api_endpoint.then(|| endpoint("https://codex.example/v1", "work"));
        recipe.codex_login_credential = login.then(|| "codex-work".into());
        recipe.github_token_credential = github.then(|| "git-work".into());
        self.store
            .update_tensorlake_recipe("recipe".into(), recipe)
            .await
            .unwrap()
            .unwrap();
    }

    /// Use real catalog admission and recipe writes so delivery's frozen
    /// binding can distinguish a pending mode edit from a value replacement.
    async fn select_claude(&self, login: Option<&str>, api_endpoint: bool) {
        let mut recipe = crate::tensorlake::recipes::tests_support::save();
        recipe.claude = api_endpoint.then(|| endpoint("https://first.example/v1", "work"));
        recipe.claude_login_credential = login.map(str::to_owned);
        self.store
            .update_tensorlake_recipe("recipe".into(), recipe)
            .await
            .unwrap()
            .unwrap();
    }

    /// The fake value lives only in the catalog until delivery writes the
    /// ordinary settings file. No fixture preinstalls the expected token.
    async fn claude_credential(&self, value: &str) {
        let saved = self
            .store
            .create_credential(CreateCredential {
                name: "claude-work".into(),
                kind: CredentialKind::ClaudeLogin,
                secret: value.into(),
            })
            .await
            .unwrap();
        assert_eq!(saved.revision, 1);
    }
}

/// A mode edit waits for connection, while a saved value replacement reaches
/// the current connection. Image settings without a legacy stamp survive
/// every mode, and unchanged reconnects skip remote work.
#[farhelm_testtrace::test]
async fn claude_subscription_reconciles_modes_replacements_and_reconnects() {
    const LOGIN: &str = "sk-ant-oat01-fictional-claude-first";
    const REPLACED: &str = "sk-ant-oat01-fictional-claude-second";
    let f = Fixture::new(false).await;
    f.claude_credential(LOGIN).await;
    let settings_path = f.home.join(".claude/settings.json");
    tokio::fs::create_dir(settings_path.parent().unwrap())
        .await
        .unwrap();
    let image_settings = br#"{"permissions":{"allow":["Read"]},"env":{"KEEP":"personal","CLAUDE_CODE_OAUTH_TOKEN":"image-token"}}"#;
    tokio::fs::write(&settings_path, image_settings)
        .await
        .unwrap();
    assert!(
        f.delivery
            .read_record(&f.row)
            .await
            .unwrap()
            .claude
            .is_none()
    );
    let lease = f.connected().await;
    f.select_claude(Some("claude-work"), false).await;
    let before = f.operations();
    assert!(
        f.delivery
            .push("claude-work", past_the_ssh_budget(), &f.events)
            .await
            .is_empty()
    );
    assert_eq!(f.operations(), before, "recipe edits remain deferred");
    assert!(!f.claude().await.contains(LOGIN));
    drop(lease);
    let lease = f.delivery.enter(&f.row);
    f.delivery.connect(&f.row, &f.events).await;
    assert_eq!(f.delivery.warning(f.row.id), None);
    let settings: serde_json::Value = serde_json::from_str(&f.claude().await).unwrap();
    assert_eq!(settings["env"]["CLAUDE_CODE_OAUTH_TOKEN"], LOGIN);
    assert!(settings["env"].get("ANTHROPIC_BASE_URL").is_none());
    assert!(settings["env"].get("ANTHROPIC_AUTH_TOKEN").is_none());
    assert_eq!(
        settings["env"],
        serde_json::json!({"CLAUDE_CODE_OAUTH_TOKEN": LOGIN})
    );
    assert_eq!(
        tokio::fs::read(&settings_path).await.unwrap(),
        image_settings
    );
    assert!(
        !f.home.join(".claude.json").exists(),
        "Farhelm writes no onboarding metadata"
    );
    assert_eq!(
        std::fs::metadata(f.claude_path())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    f.replace("claude-work", REPLACED).await;
    assert!(
        f.delivery
            .push("claude-work", past_the_ssh_budget(), &f.events)
            .await
            .is_empty()
    );
    assert!(f.claude().await.contains(REPLACED) && !f.claude().await.contains(LOGIN));
    let record = f.delivery.read_record(&f.row).await.unwrap();
    assert!(!serde_json::to_string(&record).unwrap().contains(REPLACED));
    drop(lease);
    let restarted = CredentialDelivery::new(f.store.clone(), f.backend.clone());
    let lease = restarted.enter(&f.row);
    let before = f.operations();
    restarted.connect(&f.row, &f.events).await;
    assert_eq!(restarted.warning(f.row.id), None);
    assert_eq!(
        f.operations(),
        before,
        "unchanged reconnect performs no transport work"
    );
    drop(lease);
    for endpoint in [true, false] {
        f.select_claude(None, endpoint).await;
        let lease = restarted.enter(&f.row);
        restarted.connect(&f.row, &f.events).await;
        assert_eq!(restarted.warning(f.row.id), None);
        if endpoint {
            let credentials: serde_json::Value = serde_json::from_str(&f.claude().await).unwrap();
            assert_eq!(
                credentials["env"],
                serde_json::json!({
                    "ANTHROPIC_BASE_URL": "https://first.example/v1",
                    "ANTHROPIC_AUTH_TOKEN": FIRST,
                })
            );
        } else {
            assert!(!f.claude_path().exists());
        }
        assert_eq!(
            tokio::fs::read(&settings_path).await.unwrap(),
            image_settings
        );
        drop(lease);
    }
}

/// A legacy URL can override the launch environment, so a failed cleanup
/// must withhold the whole Claude snapshot while other deliveries proceed.
/// A later GitHub failure must leave the cleanup warning on the host and on a save that needs Claude.
/// Once cleanup succeeds, an upload retry must leave subsequent edits alone.
#[farhelm_testtrace::test]
async fn stamped_claude_cleanup_gates_only_its_launch_environment() {
    const PENDING: &str =
        "Claude recipe credential cleanup is pending (could not install the agent settings)";
    let both = format!("{PENDING}. GitHub CLI (gh) is missing; private clones need gh installed");
    let f = Fixture::new(true).await;
    f.github_credential().await;
    f.select_codex(false, true, true).await;
    let settings = f.home.join(".claude/settings.json");
    tokio::fs::create_dir(settings.parent().unwrap())
        .await
        .unwrap();
    let legacy = br#"{"env":{"KEEP":"old","ANTHROPIC_BASE_URL":"https://old.example"}}"#;
    tokio::fs::write(&settings, legacy).await.unwrap();
    let entry = f.delivery.entry(&f.row);
    let guard = entry.serial.clone().lock_owned().await;
    let stamped = Record {
        home: Some(f.home.clone()),
        claude: Some(EndpointStamp::Absent),
        ..Record::default()
    };
    drop(
        f.delivery
            .write_record(&f.row, &stamped, guard)
            .await
            .unwrap(),
    );
    let _lease = f.delivery.enter(&f.row);
    *f.backend.fail.lock().unwrap() = Some("write-legacy-claude-settings".into());
    f.backend.github_cli.store(false, Ordering::SeqCst);
    f.delivery.connect(&f.row, &f.events).await;
    assert_eq!(f.delivery.warning(f.row.id).as_deref(), Some(both.as_str()));
    let snapshot = f.snapshot().await;
    assert!(!snapshot.env.contains_key("ANTHROPIC_AUTH_TOKEN"));
    assert_eq!(
        snapshot.env[recipe_credentials::CODEX_ENDPOINT_TOKEN_ENV],
        FIRST
    );
    assert_eq!(tokio::fs::read(&settings).await.unwrap(), legacy);
    let record = f.delivery.read_record(&f.row).await.unwrap();
    assert!(record.claude_environment.is_none());
    assert_eq!(record.claude, Some(EndpointStamp::Absent));
    assert!(record.codex.is_some() && record.github.is_none());

    f.replace("work", SECOND).await;
    let shared = f
        .delivery
        .push("work", past_the_ssh_budget(), &f.events)
        .await;
    assert_eq!(shared.len(), 1);
    assert_eq!(shared[0].reason, PENDING);
    assert_eq!(f.delivery.warning(f.row.id).as_deref(), Some(both.as_str()));
    let snapshot = f.snapshot().await;
    assert!(!snapshot.env.contains_key("ANTHROPIC_AUTH_TOKEN"));
    assert_eq!(
        snapshot.env[recipe_credentials::CODEX_ENDPOINT_TOKEN_ENV],
        SECOND
    );
    assert!(!f.home.join(".codex/config.toml").exists());

    f.backend.github_cli.store(true, Ordering::SeqCst);
    f.replace("git-work", "fictional-github-second").await;
    assert!(
        f.delivery
            .push("git-work", past_the_ssh_budget(), &f.events)
            .await
            .is_empty()
    );
    assert_eq!(
        tokio::fs::read_to_string(f.github_path()).await.unwrap(),
        "fictional-github-second"
    );
    assert_eq!(f.delivery.warning(f.row.id).as_deref(), Some(PENDING));

    *f.backend.fail.lock().unwrap() = Some("wrote-agent-credentials".into());
    f.delivery.connect(&f.row, &f.events).await;
    assert_eq!(
        f.delivery.warning(f.row.id).as_deref(),
        Some("could not install the managed launch credentials")
    );
    assert!(!f.snapshot().await.env.contains_key("ANTHROPIC_AUTH_TOKEN"));
    let actual: serde_json::Value =
        serde_json::from_slice(&tokio::fs::read(&settings).await.unwrap()).unwrap();
    assert_eq!(actual, serde_json::json!({"env":{"KEEP":"old"}}));
    assert_eq!(
        f.delivery.read_record(&f.row).await.unwrap().claude,
        Some(EndpointStamp::Cleared)
    );

    let user_edit = br#"{"env":{"CLAUDE_CODE_OAUTH_TOKEN":"personal-edit"}}"#;
    tokio::fs::write(&settings, user_edit).await.unwrap();
    *f.backend.fail.lock().unwrap() = None;
    f.replace("work", THIRD).await;
    assert!(
        f.delivery
            .push("work", past_the_ssh_budget(), &f.events)
            .await
            .is_empty()
    );
    assert_eq!(f.delivery.warning(f.row.id), None);
    let env = f.snapshot().await.env;
    assert_eq!(env["ANTHROPIC_AUTH_TOKEN"], THIRD);
    assert_eq!(env["ANTHROPIC_BASE_URL"], "https://first.example/v1");
    assert_eq!(env[CODEX_ENDPOINT_TOKEN_ENV], THIRD);
    assert_eq!(tokio::fs::read(&settings).await.unwrap(), user_edit);
}

/// A deleted name can be recreated at revision1. A completed None mode must
/// not let its old subscription stamp suppress the new incarnation's value.
#[farhelm_testtrace::test]
async fn recreated_claude_login_is_delivered_after_none() {
    let f = Fixture::new(false).await;
    f.claude_credential("fictional-first-claude").await;
    f.select_claude(Some("claude-work"), false).await;
    let lease = f.delivery.enter(&f.row);
    f.delivery.connect(&f.row, &f.events).await;
    assert_eq!(f.delivery.warning(f.row.id), None);
    let first = f.stamp("claude-work", 1).await;
    drop(lease);
    f.select_claude(None, false).await;
    let lease = f.delivery.enter(&f.row);
    f.delivery.connect(&f.row, &f.events).await;
    assert_eq!(f.delivery.warning(f.row.id), None);
    assert!(!f.claude_path().exists());
    drop(lease);
    f.store
        .delete_credential("claude-work".into())
        .await
        .unwrap();
    f.claude_credential("fictional-recreated-claude").await;
    assert_ne!(f.stamp("claude-work", 1).await, first);
    f.select_claude(Some("claude-work"), false).await;
    let _lease = f.delivery.enter(&f.row);
    f.delivery.connect(&f.row, &f.events).await;
    assert_eq!(f.delivery.warning(f.row.id), None);
    assert!(f.claude().await.contains("fictional-recreated-claude"));
    assert!(!f.claude().await.contains("fictional-first-claude"));
}

/// Reusing a login's name and revision must deliver its new value even when
/// the last successful delivery still identifies the deleted login.
#[farhelm_testtrace::test]
async fn recreated_claude_login_replaces_the_previous_subscription_stamp() {
    let f = Fixture::new(false).await;
    f.claude_credential("fictional-first-claude").await;
    f.select_claude(Some("claude-work"), false).await;
    let lease = f.delivery.enter(&f.row);
    f.delivery.connect(&f.row, &f.events).await;
    let previous = f
        .delivery
        .read_record(&f.row)
        .await
        .unwrap()
        .claude_environment;
    assert!(matches!(
        &previous,
        Some(EndpointStamp::Subscription { name, revision: 1, .. }) if name == "claude-work"
    ));
    drop(lease);

    // Release the catalog reference without delivering the intermediate mode.
    f.select_claude(None, false).await;
    f.store
        .delete_credential("claude-work".into())
        .await
        .unwrap();
    f.claude_credential("fictional-recreated-claude").await;
    f.select_claude(Some("claude-work"), false).await;
    let carried = f
        .delivery
        .read_record(&f.row)
        .await
        .unwrap()
        .claude_environment;
    assert_eq!(carried, previous);

    let _lease = f.delivery.enter(&f.row);
    f.delivery.connect(&f.row, &f.events).await;
    assert!(f.claude().await.contains("fictional-recreated-claude"));
}

/// Codex owns refreshed auth bytes after the initial seed. Durable revision
/// memory must survive a helm restart and switches through Endpoint and None;
/// only an explicitly replaced catalog value authorizes a fresh auth write.
#[farhelm_testtrace::test]
async fn codex_refresh_survives_reconnect_and_mode_changes_until_replacement() {
    let f = Fixture::new(false).await;
    f.codex_credential().await;
    f.select_codex(true, false, false).await;
    let lease = f.connected().await;
    let auth = f.home.join(".codex/auth.json");
    assert_eq!(std::fs::read_to_string(&auth).unwrap(), CODEX_FIRST);
    assert_eq!(
        std::fs::metadata(&auth).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(
        !f.home.join(".codex/config.toml").exists(),
        "absent clean config stays absent"
    );
    std::fs::write(&auth, CODEX_REFRESHED).unwrap();
    drop(lease);
    let delivery = CredentialDelivery::new(f.store.clone(), f.backend.clone());
    let _lease = delivery.enter(&f.row);
    let before = f.operations();
    delivery.connect(&f.row, &f.events).await;
    assert_eq!(
        f.operations(),
        before,
        "persisted seed and cleared config require no SSH"
    );
    for (login, endpoint) in [(false, true), (false, false), (true, false)] {
        f.select_codex(login, endpoint, false).await;
        delivery.connect(&f.row, &f.events).await;
        assert_eq!(delivery.warning(f.row.id), None);
        assert_eq!(std::fs::read_to_string(&auth).unwrap(), CODEX_REFRESHED);
        assert_eq!(
            delivery.read_record(&f.row).await.unwrap().codex_auth,
            Some(f.stamp("codex-work", 1).await)
        );
    }
    assert!(!f.home.join(".codex/config.toml").exists());
    assert!(f.snapshot().await.codex_endpoint.is_none());
    let replacement = r#"{"new_vendor_shape":{"opaque":"fictional-new-login"}}"#;
    f.replace("codex-work", replacement).await;
    assert!(
        delivery
            .push("codex-work", past_the_ssh_budget(), &f.events)
            .await
            .is_empty()
    );
    assert_eq!(std::fs::read_to_string(&auth).unwrap(), replacement);
    assert_eq!(
        delivery.read_record(&f.row).await.unwrap().codex_auth,
        Some(f.stamp("codex-work", 2).await)
    );
    let record = serde_json::to_string(&delivery.read_record(&f.row).await.unwrap()).unwrap();
    assert!(
        !record.contains("fictional"),
        "only names, revisions and creation identities are durable"
    );
}

/// Leaving Subscription preserves refreshed bytes and its successful stamp.
/// Deleting and recreating that name at revision1 is a different login, so
/// returning afterwards must seed the new value rather than skip delivery.
#[farhelm_testtrace::test]
async fn recreated_codex_login_is_delivered_after_none_preserved_the_old_seed() {
    let f = Fixture::new(false).await;
    f.codex_credential().await;
    f.select_codex(true, false, false).await;
    let _lease = f.connected().await;
    let auth = f.home.join(".codex/auth.json");
    assert_eq!(std::fs::read_to_string(&auth).unwrap(), CODEX_FIRST);
    let first = f.stamp("codex-work", 1).await;
    std::fs::write(&auth, CODEX_REFRESHED).unwrap();
    f.select_codex(false, false, false).await;
    f.delivery.connect(&f.row, &f.events).await;
    assert_eq!(f.delivery.warning(f.row.id), None);
    assert_eq!(std::fs::read_to_string(&auth).unwrap(), CODEX_REFRESHED);
    assert_eq!(
        f.delivery.read_record(&f.row).await.unwrap().codex_auth,
        Some(first.clone())
    );
    f.store
        .delete_credential("codex-work".into())
        .await
        .unwrap();
    let value = r#"{"opaque":"fictional-recreated-codex-login"}"#;
    let saved = f
        .store
        .create_credential(CreateCredential {
            name: "codex-work".into(),
            kind: CredentialKind::CodexLogin,
            secret: value.into(),
        })
        .await
        .unwrap();
    assert_eq!(saved.revision, 1);
    let next = f.stamp("codex-work", 1).await;
    assert_ne!(first.2, next.2);
    f.select_codex(true, false, false).await;
    f.delivery.connect(&f.row, &f.events).await;
    assert_eq!(f.delivery.warning(f.row.id), None);
    assert_eq!(std::fs::read_to_string(&auth).unwrap(), value);
    assert_eq!(
        f.delivery.read_record(&f.row).await.unwrap().codex_auth,
        Some(next)
    );
    let before = f.operations();
    f.delivery.connect(&f.row, &f.events).await;
    assert_eq!(f.operations(), before);
}

/// A later helper failure cannot erase an already committed Codex seed stamp.
/// Retry must leave a refreshed file alone even though the overall connect
/// originally warned; otherwise gh installation would silently undo a login.
#[farhelm_testtrace::test]
async fn codex_auth_success_is_durable_before_later_github_failure() {
    let f = Fixture::new(false).await;
    f.codex_credential().await;
    f.github_credential().await;
    f.select_codex(true, false, true).await;
    f.backend.github_cli.store(false, Ordering::SeqCst);
    let _lease = f.delivery.enter(&f.row);
    f.delivery.connect(&f.row, &f.events).await;
    let auth = f.home.join(".codex/auth.json");
    assert_eq!(std::fs::read_to_string(&auth).unwrap(), CODEX_FIRST);
    let record = f.delivery.read_record(&f.row).await.unwrap();
    assert_eq!(record.codex_auth, Some(f.stamp("codex-work", 1).await));
    assert!(record.github.is_none());
    assert!(
        f.delivery
            .warning(f.row.id)
            .unwrap()
            .contains("private clones need gh")
    );
    std::fs::write(&auth, CODEX_REFRESHED).unwrap();
    f.backend.github_cli.store(true, Ordering::SeqCst);
    let before = f.operations().len();
    f.delivery.connect(&f.row, &f.events).await;
    assert_eq!(f.delivery.warning(f.row.id), None);
    assert_eq!(std::fs::read_to_string(&auth).unwrap(), CODEX_REFRESHED);
    assert!(
        !f.operations()[before..]
            .iter()
            .any(|operation| operation == "read-agent-settings")
    );
}

/// Explicit None must clear previously delivered settings once, while preserving manually selected providers,
/// comments and unrelated table keys. A reconnect then needs no settings access.
#[farhelm_testtrace::test]
async fn codex_none_cleans_legacy_endpoint_once_without_touching_auth() {
    let f = Fixture::new(false).await;
    std::fs::create_dir(f.home.join(".codex")).unwrap();
    let config = f.home.join(".codex/config.toml");
    let auth = f.home.join(".codex/auth.json");
    std::fs::write(&config, "# personal comment\nmodel_provider = 'personal'\n[model_providers.farhelm]\nexperimental_bearer_token = 'fictional-old-token'\ncustom = true\n").unwrap();
    // A directory at auth.json makes any attempted read or replacement fail;
    // None has no authority over it, even when cleaning a legacy config.
    std::fs::create_dir(&auth).unwrap();
    f.stamp_legacy_codex().await;
    let _lease = f.connected().await;
    let text = std::fs::read_to_string(&config).unwrap();
    assert!(text.contains("# personal comment") && !text.contains("fictional-old-token"));
    let doc: toml_edit::DocumentMut = text.parse().unwrap();
    assert_eq!(doc["model_provider"].as_str(), Some("personal"));
    assert_eq!(
        doc["model_providers"]["farhelm"]["custom"].as_bool(),
        Some(true)
    );
    assert!(auth.is_dir());
    assert_eq!(
        f.delivery.read_record(&f.row).await.unwrap().codex,
        Some(EndpointStamp::Cleared)
    );
    let before = f.operations();
    f.delivery.connect(&f.row, &f.events).await;
    assert_eq!(f.operations(), before);
}

/// An old None or Subscription delivery could record Absent without creating a settings file. A user who subsequently
/// chose the `farhelm` provider owns its settings and the upgrade must leave them alone.
#[farhelm_testtrace::test]
async fn codex_absent_stamp_preserves_later_user_settings() {
    let f = Fixture::new(false).await;
    let config = f.home.join(".codex/config.toml");
    tokio::fs::create_dir(config.parent().unwrap())
        .await
        .unwrap();
    let personal = legacy_codex_config("personal");
    tokio::fs::write(&config, &personal).await.unwrap();
    let entry = f.delivery.entry(&f.row);
    let guard = entry.serial.clone().lock_owned().await;
    let mut record = f.delivery.read_record(&f.row).await.unwrap();
    record.codex = Some(EndpointStamp::Absent);
    f.delivery
        .write_record(&f.row, &record, guard)
        .await
        .unwrap();

    let _lease = f.connected().await;
    assert_eq!(tokio::fs::read_to_string(config).await.unwrap(), personal);
    assert!(!f.operations().iter().any(|op| op == "read-agent-settings"));
    let record = f.delivery.read_record(&f.row).await.unwrap();
    assert_eq!(record.codex, Some(EndpointStamp::Cleared));
}

/// Pending recipe edits do not update host stamps. Reusing an unreferenced
/// name at revision1 and restoring its endpoint URLs must still deliver the
/// newly created credential to both agents on their next connection.
#[farhelm_testtrace::test]
async fn recreated_endpoint_names_do_not_match_a_previous_delivery() {
    let f = Fixture::new(true).await;
    let _lease = f.connected().await;
    let first = f.stamp("work", 1).await;
    let empty = crate::tensorlake::recipes::tests_support::save();
    f.store
        .update_tensorlake_recipe("recipe".into(), empty)
        .await
        .unwrap()
        .unwrap();
    f.store.delete_credential("work".into()).await.unwrap();
    let saved = f
        .store
        .create_credential(CreateCredential {
            name: "work".into(),
            kind: CredentialKind::EndpointToken,
            secret: SECOND.into(),
        })
        .await
        .unwrap();
    assert_eq!(saved.revision, 1);
    let next = f.stamp("work", 1).await;
    assert_ne!(first.2, next.2);
    let mut recipe = crate::tensorlake::recipes::tests_support::save();
    recipe.claude = Some(endpoint("https://first.example/v1", "work"));
    recipe.codex = Some(endpoint("https://codex.example/v1", "work"));
    f.store
        .update_tensorlake_recipe("recipe".into(), recipe)
        .await
        .unwrap()
        .unwrap();
    assert!(
        f.claude().await.contains(FIRST),
        "pending recipe edits have not reconnected"
    );
    f.delivery.connect(&f.row, &f.events).await;
    assert_eq!(f.delivery.warning(f.row.id), None);
    assert!(f.claude().await.contains(SECOND));
    let snapshot = f.snapshot().await;
    assert_eq!(snapshot.env["ANTHROPIC_AUTH_TOKEN"], SECOND);
    assert_eq!(
        snapshot.env[recipe_credentials::CODEX_ENDPOINT_TOKEN_ENV],
        SECOND
    );
    assert!(!f.home.join(".codex/config.toml").exists());
    let record = f.delivery.read_record(&f.row).await.unwrap();
    for stamp in [record.claude_environment, record.codex_environment] {
        let Some(EndpointStamp::Endpoint { credential, .. }) = stamp else {
            panic!("endpoint stamp missing")
        };
        assert_eq!(credential, Some(next.clone()));
    }
    let before = f.operations();
    f.delivery.connect(&f.row, &f.events).await;
    assert_eq!(f.operations(), before);
}

/// First delivery configures Git's token-free helper; a connected replacement
/// rewrites only the private file. A reconstructed delivery object does no SSH
/// for unchanged state, and None deletes only that file on the next connection.
#[farhelm_testtrace::test]
async fn github_delivery_replaces_on_push_and_removes_on_reconnect() {
    let f = Fixture::new(false).await;
    f.github_credential().await;
    f.select_github(Some("git-work")).await;
    let lease = f.connected().await;
    let path = f.github_path();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "fictional-github-first"
    );
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        f.operations()
            .iter()
            .filter(|op| *op == "configure-github-helper")
            .count(),
        1
    );
    f.replace("git-work", "fictional-github-second").await;
    assert!(
        f.delivery
            .push("git-work", past_the_ssh_budget(), &f.events)
            .await
            .is_empty()
    );
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "fictional-github-second"
    );
    assert_eq!(
        f.operations()
            .iter()
            .filter(|op| *op == "configure-github-helper")
            .count(),
        1
    );
    let record = f.delivery.read_record(&f.row).await.unwrap();
    assert_eq!(
        record.github,
        Some(GithubStamp::Token {
            name: "git-work".into(),
            revision: 2,
            incarnation: f.stamp("git-work", 2).await.2,
        })
    );
    let json = serde_json::to_string(&record).unwrap();
    assert!(!json.contains("fictional-github") && !json.contains(FIRST));
    drop(lease);
    let delivery = CredentialDelivery::new(f.store.clone(), f.backend.clone());
    let lease = delivery.enter(&f.row);
    let before = f.operations();
    delivery.connect(&f.row, &f.events).await;
    assert_eq!(
        f.operations(),
        before,
        "unchanged persisted state makes no remote call"
    );
    f.select_github(None).await;
    f.replace("git-work", "fictional-github-third").await;
    assert!(
        delivery
            .push("git-work", past_the_ssh_budget(), &f.events)
            .await
            .is_empty()
    );
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "fictional-github-third",
        "recipe edits wait for a connection"
    );
    let manual = path.parent().unwrap().join("manual-credential");
    std::fs::write(&manual, "owned-manual-marker").unwrap();
    drop(lease);
    let _lease = delivery.enter(&f.row);
    delivery.connect(&f.row, &f.events).await;
    assert!(!path.exists());
    assert_eq!(
        std::fs::read_to_string(manual).unwrap(),
        "owned-manual-marker"
    );
    assert_eq!(
        delivery.read_record(&f.row).await.unwrap().github,
        Some(GithubStamp::Absent)
    );
    assert_eq!(delivery.warning(f.row.id), None);
}

/// Recreating a deselected name before reconnect keeps the same public
/// revision. Its new creation identity must still replace the host's old
/// token; helper setup remains valid because it belongs to the host, not token.
#[farhelm_testtrace::test]
async fn recreated_github_names_do_not_match_a_previous_delivery() {
    let f = Fixture::new(false).await;
    f.github_credential().await;
    f.select_github(Some("git-work")).await;
    let _lease = f.connected().await;
    let first = f.stamp("git-work", 1).await;
    f.select_github(None).await;
    f.store.delete_credential("git-work".into()).await.unwrap();
    let saved = f
        .store
        .create_credential(CreateCredential {
            name: "git-work".into(),
            kind: CredentialKind::GithubToken,
            secret: "fictional-recreated-github".into(),
        })
        .await
        .unwrap();
    assert_eq!(saved.revision, 1);
    let next = f.stamp("git-work", 1).await;
    assert_ne!(first.2, next.2);
    f.select_github(Some("git-work")).await;
    assert_eq!(
        std::fs::read_to_string(f.github_path()).unwrap(),
        "fictional-github-first"
    );
    f.delivery.connect(&f.row, &f.events).await;
    assert_eq!(f.delivery.warning(f.row.id), None);
    assert_eq!(
        std::fs::read_to_string(f.github_path()).unwrap(),
        "fictional-recreated-github"
    );
    assert_eq!(
        f.delivery.read_record(&f.row).await.unwrap().github,
        Some(GithubStamp::Token {
            name: next.0,
            revision: next.1,
            incarnation: next.2
        })
    );
    assert_eq!(
        f.operations()
            .iter()
            .filter(|operation| *operation == "configure-github-helper")
            .count(),
        1
    );
    let before = f.operations();
    f.delivery.connect(&f.row, &f.events).await;
    assert_eq!(f.operations(), before);
}

/// Reused images can lack gh. The token still reaches new children, the host
/// stays usable with a fixed warning, and only the unsuccessful GitHub slot
/// retries after gh is installed; successful endpoint writes remain durable.
#[farhelm_testtrace::test]
async fn github_without_gh_keeps_the_file_and_retries_only_the_unfinished_slot() {
    let f = Fixture::new(false).await;
    f.github_credential().await;
    f.select_github(Some("git-work")).await;
    f.backend.github_cli.store(false, Ordering::SeqCst);
    let lease = f.delivery.enter(&f.row);
    f.delivery.connect(&f.row, &f.events).await;
    assert!(
        f.delivery
            .warning(f.row.id)
            .unwrap()
            .contains("private clones need gh")
    );
    assert_eq!(
        std::fs::read_to_string(f.github_path()).unwrap(),
        "fictional-github-first"
    );
    assert!(f.claude().await.contains(FIRST));
    let record = f.delivery.read_record(&f.row).await.unwrap();
    assert!(record.claude_environment.is_some() && record.github.is_none());
    let endpoint = std::fs::read(f.claude_path()).unwrap();
    f.backend.github_cli.store(true, Ordering::SeqCst);
    drop(lease);
    let before = f.operations().len();
    let _lease = f.delivery.enter(&f.row);
    f.delivery.connect(&f.row, &f.events).await;
    assert_eq!(f.delivery.warning(f.row.id), None);
    assert_eq!(std::fs::read(f.claude_path()).unwrap(), endpoint);
    let retried = &f.operations()[before..];
    assert!(
        !retried.iter().any(|op| matches!(
            op.as_str(),
            "read-agent-settings" | "wrote-agent-credentials"
        )),
        "no completed endpoint delivery repeats: {retried:?}"
    );
    assert_eq!(
        retried
            .iter()
            .filter(|op| *op == "configure-github-helper")
            .count(),
        1
    );
}

/// Recipe edits may legally release a catalog name before the old connection
/// retires its binding. A missing later GitHub value must not withhold a valid
/// endpoint replacement or report it as undelivered. Hosts still show the GitHub failure.
#[farhelm_testtrace::test]
async fn a_deleted_github_selection_does_not_block_endpoint_replacement() {
    let f = Fixture::new(false).await;
    f.github_credential().await;
    f.select_github(Some("git-work")).await;
    let _lease = f.connected().await;
    f.select_github(None).await;
    f.store.delete_credential("git-work".into()).await.unwrap();
    f.replace("work", SECOND).await;
    let failures = f
        .delivery
        .push("work", past_the_ssh_budget(), &f.events)
        .await;
    assert!(failures.is_empty());
    assert_eq!(
        f.delivery.warning(f.row.id).as_deref(),
        Some("could not resolve the recipe's GitHub credential")
    );
    assert!(f.claude().await.contains(SECOND));
    assert_eq!(
        f.delivery
            .read_record(&f.row)
            .await
            .unwrap()
            .claude_environment,
        Some(EndpointStamp::Endpoint {
            base_url: "https://first.example/v1".into(),
            credential: Some(f.stamp("work", 2).await),
        })
    );
    assert_eq!(
        std::fs::read_to_string(f.github_path()).unwrap(),
        "fictional-github-first"
    );
}

/// Before this behavior, a failed lookup for one agent rewrote the combined launch
/// file without that agent's entry, even during an unrelated GitHub push, which
/// silently took the agent's credentials off the host. Failed endpoint lookups
/// leave the last delivered snapshot intact. A push for either entry reports the
/// failed lookup, including when GitHub lookup also fails, because a rotation that
/// cannot reach the host must not be reported as delivered.
#[farhelm_testtrace::test]
async fn endpoint_lookup_failures_preserve_the_shared_file_and_fail_dependent_pushes() {
    for (lose_claude, lose_codex) in [(true, false), (false, true), (true, true)] {
        let f = Fixture::new(false).await;
        f.store
            .create_credential(CreateCredential {
                name: "codex-api".into(),
                kind: CredentialKind::EndpointToken,
                secret: SECOND.into(),
            })
            .await
            .unwrap();
        f.github_credential().await;
        let mut recipe = crate::tensorlake::recipes::tests_support::save();
        recipe.claude = Some(endpoint("https://first.example/v1", "work"));
        recipe.codex = Some(endpoint("https://codex.example/v1", "codex-api"));
        recipe.github_token_credential = Some("git-work".into());
        f.store
            .update_tensorlake_recipe("recipe".into(), recipe.clone())
            .await
            .unwrap()
            .unwrap();
        let _lease = f.connected().await;
        assert_eq!(
            f.snapshot().await.env[recipe_credentials::CODEX_ENDPOINT_TOKEN_ENV],
            SECOND
        );
        let prior = tokio::fs::read(f.claude_path()).await.unwrap();
        assert_eq!(
            tokio::fs::read_to_string(f.github_path()).await.unwrap(),
            "fictional-github-first"
        );

        // A stamped legacy file would authorize cleanup if delivery made it past the failed lookups.
        let config = f.home.join(".codex/config.toml");
        tokio::fs::create_dir(config.parent().unwrap())
            .await
            .unwrap();
        let legacy = legacy_codex_config("personal");
        tokio::fs::write(&config, &legacy).await.unwrap();
        f.stamp_legacy_codex().await;
        let before = f.delivery.read_record(&f.row).await.unwrap();
        assert!(before.claude_environment.is_some() && before.codex_environment.is_some());

        // An edit can release a name in the catalog while a connected host still uses that selection.
        if lose_claude {
            recipe.claude = None;
        }
        if lose_codex {
            recipe.codex = None;
        }
        f.store
            .update_tensorlake_recipe("recipe".into(), recipe.clone())
            .await
            .unwrap()
            .unwrap();
        if lose_claude {
            f.store.delete_credential("work".into()).await.unwrap();
        }
        if lose_codex {
            f.store.delete_credential("codex-api".into()).await.unwrap();
        }

        let warning = match (lose_claude, lose_codex) {
            (true, false) => "could not resolve the recipe's Claude endpoint credential",
            (false, true) => "could not resolve the recipe's Codex endpoint credential",
            (true, true) => {
                "could not resolve the recipe's Claude endpoint credential. could not resolve the recipe's Codex endpoint credential"
            }
            (false, false) => unreachable!(),
        };
        if let Some(name) = match (lose_claude, lose_codex) {
            (true, false) => Some("codex-api"),
            (false, true) => Some("work"),
            _ => None,
        } {
            f.replace(name, THIRD).await;
            let failures = f
                .delivery
                .push(name, past_the_ssh_budget(), &f.events)
                .await;
            assert_eq!(failures.len(), 1);
            assert_eq!(failures[0].reason, warning);
            assert_eq!(tokio::fs::read(f.claude_path()).await.unwrap(), prior);
        }

        f.replace("git-work", "fictional-github-second").await;
        let failures = f
            .delivery
            .push("git-work", past_the_ssh_budget(), &f.events)
            .await;
        assert!(failures.is_empty());
        assert_eq!(
            tokio::fs::read_to_string(f.github_path()).await.unwrap(),
            "fictional-github-second"
        );
        assert_eq!(f.delivery.warning(f.row.id).as_deref(), Some(warning));
        assert_eq!(tokio::fs::read(f.claude_path()).await.unwrap(), prior);
        assert_eq!(tokio::fs::read_to_string(&config).await.unwrap(), legacy);
        let after = f.delivery.read_record(&f.row).await.unwrap();
        assert_eq!(after.claude_environment, before.claude_environment);
        assert_eq!(after.codex_environment, before.codex_environment);
        assert_eq!(after.codex, before.codex);

        if lose_claude && !lose_codex {
            // The old connection also keeps the GitHub selection until it reconnects.
            recipe.github_token_credential = None;
            f.store
                .update_tensorlake_recipe("recipe".into(), recipe)
                .await
                .unwrap()
                .unwrap();
            f.store.delete_credential("git-work".into()).await.unwrap();
            f.replace("codex-api", "fictional-fourth-endpoint-token")
                .await;
            let failures = f
                .delivery
                .push("codex-api", past_the_ssh_budget(), &f.events)
                .await;
            let both = format!("{warning}. could not resolve the recipe's GitHub credential");
            assert_eq!(failures.len(), 1);
            assert_eq!(failures[0].reason, warning);
            assert_eq!(f.delivery.warning(f.row.id).as_deref(), Some(both.as_str()));
            assert_eq!(tokio::fs::read(f.claude_path()).await.unwrap(), prior);
        }
    }
}

/// A failed Claude subscription lookup must not take Claude's credentials off the host
/// or hold back the separately stored ones: its last launch environment stays in place,
/// a valid Codex login can still rotate, and a missing login still leaves GitHub
/// available. Setup reports the failed Claude lookup to its caller, which shows it on
/// the setup step.
#[farhelm_testtrace::test]
async fn claude_lookup_failure_allows_codex_login_rotation_and_reports_a_setup_notice() {
    const CLAUDE_LOGIN: &str = "sk-ant-oat01-fictional-claude-first";
    const WARNING: &str = "could not resolve the recipe's Claude login";
    let f = Fixture::new(false).await;
    f.claude_credential(CLAUDE_LOGIN).await;
    f.codex_credential().await;
    f.github_credential().await;
    let mut recipe = crate::tensorlake::recipes::tests_support::save();
    recipe.claude_login_credential = Some("claude-work".into());
    recipe.codex_login_credential = Some("codex-work".into());
    recipe.github_token_credential = Some("git-work".into());
    let confirmed = f
        .store
        .update_tensorlake_recipe("recipe".into(), recipe.clone())
        .await
        .unwrap()
        .unwrap();
    let _lease = f.delivery.enter(&f.row);
    f.delivery.connect(&f.row, &f.events).await;
    assert_eq!(f.delivery.warning(f.row.id), None);
    assert_eq!(
        f.snapshot().await.env["CLAUDE_CODE_OAUTH_TOKEN"],
        CLAUDE_LOGIN
    );
    let prior = tokio::fs::read(f.claude_path()).await.unwrap();
    let before = f.delivery.read_record(&f.row).await.unwrap();
    let auth = f.home.join(".codex/auth.json");
    assert_eq!(tokio::fs::read_to_string(&auth).await.unwrap(), CODEX_FIRST);
    assert_eq!(
        tokio::fs::read_to_string(f.github_path()).await.unwrap(),
        "fictional-github-first"
    );

    recipe.claude_login_credential = None;
    f.store
        .update_tensorlake_recipe("recipe".into(), recipe.clone())
        .await
        .unwrap()
        .unwrap();
    f.store
        .delete_credential("claude-work".into())
        .await
        .unwrap();
    f.replace("codex-work", CODEX_REFRESHED).await;
    let failures = f
        .delivery
        .push("codex-work", past_the_ssh_budget(), &f.events)
        .await;
    assert!(failures.is_empty());
    assert_eq!(
        tokio::fs::read_to_string(&auth).await.unwrap(),
        CODEX_REFRESHED
    );
    assert_eq!(f.delivery.warning(f.row.id).as_deref(), Some(WARNING));
    let delivered_login = f.stamp("codex-work", 2).await;

    assert_eq!(
        f.delivery
            .setup(&f.row, &confirmed, &f.home, &f.events)
            .await
            .unwrap_err(),
        WARNING
    );
    recipe.codex_login_credential = None;
    f.store
        .update_tensorlake_recipe("recipe".into(), recipe)
        .await
        .unwrap()
        .unwrap();
    f.store
        .delete_credential("codex-work".into())
        .await
        .unwrap();
    f.replace("git-work", "fictional-github-second").await;
    let failures = f
        .delivery
        .push("git-work", past_the_ssh_budget(), &f.events)
        .await;
    assert!(failures.is_empty());
    assert_eq!(
        tokio::fs::read_to_string(f.github_path()).await.unwrap(),
        "fictional-github-second"
    );
    let both = format!("{WARNING}. could not resolve the recipe's Codex login");
    assert_eq!(f.delivery.warning(f.row.id).as_deref(), Some(both.as_str()));
    assert_eq!(
        tokio::fs::read_to_string(&auth).await.unwrap(),
        CODEX_REFRESHED
    );
    assert_eq!(tokio::fs::read(f.claude_path()).await.unwrap(), prior);
    let after = f.delivery.read_record(&f.row).await.unwrap();
    assert_eq!(after.claude_environment, before.claude_environment);
    assert_eq!(after.codex_environment, before.codex_environment);
    assert_eq!(after.codex_auth, Some(delivered_login));
}

/// A Codex login can reach its file even when a shared launch lookup and the later GitHub lookup fail.
/// The save confirms delivery of the login, and host details report both other failures.
#[farhelm_testtrace::test]
async fn delivered_codex_login_save_leaves_other_failures_in_host_details() {
    let f = Fixture::new(false).await;
    f.codex_credential().await;
    f.github_credential().await;
    f.select_codex(true, false, true).await;
    let _lease = f.connected().await;
    let before = tokio::fs::read(f.claude_path()).await.unwrap();
    let auth = f.home.join(".codex/auth.json");
    assert_eq!(tokio::fs::read_to_string(&auth).await.unwrap(), CODEX_FIRST);
    assert_eq!(
        tokio::fs::read_to_string(f.github_path()).await.unwrap(),
        "fictional-github-first"
    );

    let mut recipe = crate::tensorlake::recipes::tests_support::save();
    recipe.codex_login_credential = Some("codex-work".into());
    f.store
        .update_tensorlake_recipe("recipe".into(), recipe)
        .await
        .unwrap()
        .unwrap();
    f.store.delete_credential("work".into()).await.unwrap();
    f.store.delete_credential("git-work".into()).await.unwrap();
    f.replace("codex-work", CODEX_REFRESHED).await;
    let failures = f
        .delivery
        .push("codex-work", past_the_ssh_budget(), &f.events)
        .await;

    assert!(failures.is_empty());
    assert_eq!(
        f.delivery.warning(f.row.id).as_deref(),
        Some(
            "could not resolve the recipe's Claude endpoint credential. could not resolve the recipe's GitHub credential"
        )
    );
    assert_eq!(
        tokio::fs::read_to_string(&auth).await.unwrap(),
        CODEX_REFRESHED
    );
    assert_eq!(
        f.delivery.read_record(&f.row).await.unwrap().codex_auth,
        Some(f.stamp("codex-work", 2).await)
    );
    assert_eq!(tokio::fs::read(f.claude_path()).await.unwrap(), before);
}

/// A GitHub save must still report a failure when an earlier Codex login error stops the pass before its turn.
#[farhelm_testtrace::test]
async fn a_github_save_reports_the_codex_login_failure_that_blocked_it() {
    let f = Fixture::new(false).await;
    f.codex_credential().await;
    f.github_credential().await;
    f.select_codex(true, false, true).await;
    let _lease = f.connected().await;
    let before = tokio::fs::read(f.github_path()).await.unwrap();
    assert_eq!(f.delivery.warning(f.row.id), None);

    f.select_codex(false, false, true).await;
    f.store
        .delete_credential("codex-work".into())
        .await
        .unwrap();
    f.replace("git-work", "fictional-github-second").await;
    let failures = f
        .delivery
        .push("git-work", past_the_ssh_budget(), &f.events)
        .await;
    assert_eq!(failures.len(), 1);
    assert_eq!(
        failures[0].reason,
        "could not resolve the recipe's Codex login"
    );
    assert_eq!(tokio::fs::read(f.github_path()).await.unwrap(), before);
}

/// Distinct URLs and names let the edit-timing test distinguish a binding
/// snapshot from accidentally reloading the newest recipe during a push.
fn endpoint(url: &str, name: &str) -> EndpointSave {
    EndpointSave {
        base_url: url.into(),
        token_credential: Some(name.into()),
    }
}

/// Poll once to establish that the operation actually reached its held wait.
/// Its eventual await supplies a real waker; this observation schedules no work.
fn assert_pending(future: Pin<&mut impl Future>) {
    assert!(
        future
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
}

/// Restarting the delivery object models a helm restart: unchanged persisted
/// stamps must avoid even a home probe while preserving unrelated host settings.
#[farhelm_testtrace::test]
async fn unchanged_reconnect_uses_durable_stamps_without_remote_work() {
    let f = Fixture::new(false).await;
    let lease = f.connected().await;
    let path = f.home.join(".claude/settings.json");
    let settings = br#"{"unrelated": true}"#;
    tokio::fs::create_dir(path.parent().unwrap()).await.unwrap();
    tokio::fs::write(&path, settings).await.unwrap();
    drop(lease);
    let before = f.operations();
    assert!(
        !before.iter().any(|op| op == "read-home"),
        "a fresh host can receive launch credentials without a home lookup"
    );
    let restarted = CredentialDelivery::new(f.store.clone(), f.backend.clone());
    let _lease = restarted.enter(&f.row);
    restarted.connect(&f.row, &f.events).await;
    assert_eq!(f.operations(), before);
    assert_eq!(restarted.warning(f.row.id), None);
    assert_eq!(tokio::fs::read(path).await.unwrap(), settings);
    let record = restarted.read_record(&f.row).await.unwrap();
    assert_eq!(record.home, None);
    assert_eq!(record.codex, Some(EndpointStamp::Cleared));
    assert_eq!(
        std::fs::metadata(f.claude_path())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}

use std::os::unix::fs::PermissionsExt;

/// Credential replacement uses the active connection's URL/name selection;
/// editing a recipe cannot silently activate its pending changes on that host.
#[farhelm_testtrace::test]
async fn replacement_defers_recipe_edits_until_the_next_connection() {
    let f = Fixture::new(false).await;
    let lease = f.connected().await;
    f.store
        .create_credential(CreateCredential {
            name: "next".into(),
            kind: CredentialKind::EndpointToken,
            secret: "fictional-next-token".into(),
        })
        .await
        .unwrap();
    let mut changed = crate::tensorlake::recipes::tests_support::save();
    changed.claude = Some(endpoint("https://next.example/v1", "next"));
    f.store
        .update_tensorlake_recipe("recipe".into(), changed)
        .await
        .unwrap()
        .unwrap();
    f.replace("work", SECOND).await;
    assert!(
        f.delivery
            .push("work", past_the_ssh_budget(), &f.events)
            .await
            .is_empty()
    );
    let bytes = f.claude().await;
    assert!(bytes.contains(SECOND) && bytes.contains("https://first.example/v1"));
    assert!(!bytes.contains("next.example"));
    let before = f.operations();
    f.replace("next", "fictional-newer-next-token").await;
    assert!(
        f.delivery
            .push("next", past_the_ssh_budget(), &f.events)
            .await
            .is_empty()
    );
    assert_eq!(f.operations(), before);
    drop(lease);
    let _lease = f.delivery.enter(&f.row);
    f.delivery.connect(&f.row, &f.events).await;
    let bytes = f.claude().await;
    assert!(
        bytes.contains("https://next.example/v1") && bytes.contains("fictional-newer-next-token")
    );
    assert!(!bytes.contains(SECOND));
}

/// A reply deadline later than the delivery's own SSH budget, for the tests of
/// the delivery itself: every host then reports its own outcome rather than
/// being cut off at the reply (see `CredentialDelivery::push`).
fn past_the_ssh_budget() -> Instant {
    Instant::now() + COMMAND_TIMEOUT * 2
}

impl Fixture {
    /// A second connected host on the fixture's recipe, with a home of its
    /// own so that its settings file can be told apart from the first host's.
    /// Returns its row, credential file and connection lease; the lease is the premise
    /// that makes a replacement select it.
    async fn second_host(&self) -> (HostRow, PathBuf, ConnectionLease) {
        let id = crate::tensorlake::hosts::test_support::plant(
            &self.store,
            "second host",
            "second-sandbox",
            "recipe",
            Some("second-sandbox-id"),
        )
        .await;
        let state = self._root.path().join("second-supervisor-state");
        assert!(
            self.store
                .admit_tensorlake_install(id, "/fixture/farhelm", state.to_str().unwrap())
                .await
                .unwrap()
        );
        let row = self
            .store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == id)
            .unwrap();
        let home = self._root.path().join("second-home");
        tokio::fs::create_dir(&home).await.unwrap();
        self.delivery
            .remember_home(&row, home.clone())
            .await
            .unwrap();
        let lease = self.delivery.enter(&row);
        self.delivery.connect(&row, &self.events).await;
        assert_eq!(self.delivery.warning(row.id), None);
        let credentials = farhelm_supervisor::recipe_credentials::host_file(&state);
        assert!(
            tokio::fs::read_to_string(&credentials)
                .await
                .unwrap()
                .contains(FIRST)
        );
        (row, credentials, lease)
    }
}

/// The save's reply is due at the helm's host reply bound, because the GUI's
/// channel admits one command at a time: one host that cannot take the value
/// must not hold the reply for the whole SSH budget, nor hide another host's
/// success, nor lose its own delivery. Specified: with one host's delivery
/// held at its host lock (as it is behind another delivery to that host) and
/// the clock paused, a push whose reply is due at `HOST_REPLY_BOUND` answers
/// then (to the timer's millisecond), naming only the held host, as not
/// delivered, with the reason that it did not answer in time; the quick host
/// already has the replacement and the saved revision stands. Released after
/// the reply, the held host's delivery finishes on its own: a late success
/// installs the replacement and leaves no warning, and a late failure (a
/// second replacement, whose held delivery meets a transport failure planted
/// after its reply) becomes the host's warning, the saved revision standing.
/// The late outcome's warning is the only place it shows, so both are checked.
#[farhelm_testtrace::test]
async fn a_stalled_host_is_reported_at_the_reply_bound_while_its_delivery_carries_on() {
    let f = Fixture::new(false).await;
    let _lease = f.connected().await;
    let (_second, second_credentials, _second_lease) = f.second_host().await;
    f.replace("work", SECOND).await;
    let entry = f.delivery.entry(&f.row);
    let held = entry.serial.clone().lock_owned().await;
    // Paused, the clock moves only when every task waits on a timer; the
    // quick host's database and file work runs on the blocking pool, which
    // holds the clock still until it is done.
    tokio::time::pause();
    let started = Instant::now();
    let failures = f
        .delivery
        .push(
            "work",
            started + crate::sessions::HOST_REPLY_BOUND,
            &f.events,
        )
        .await;
    let waited = started.elapsed();
    crate::rest_harness::assert_answered_at_reply_bound(waited, "the push");
    assert_eq!(
        failures,
        vec![DeliveryFailure {
            host: "fixture host".into(),
            reason: still_delivering(),
        }]
    );
    assert!(
        failures[0]
            .reason
            .contains("did not answer within 20 seconds")
    );
    assert!(
        tokio::fs::read_to_string(&second_credentials)
            .await
            .unwrap()
            .contains(SECOND)
    );
    assert!(f.claude().await.contains(FIRST));
    assert_eq!(f.store.credentials().await.unwrap()[0].revision, 2);

    // The mutex is fair, so the held delivery, queued first, finishes its
    // file work before this lock is granted. Its warning is set in the same
    // poll that releases the lock, so it is already in place here too.
    drop(held);
    let held = entry.serial.clone().lock_owned().await;
    assert!(f.claude().await.contains(SECOND));
    assert_eq!(
        f.delivery
            .read_record(&f.row)
            .await
            .unwrap()
            .claude_environment,
        Some(EndpointStamp::Endpoint {
            base_url: "https://first.example/v1".into(),
            credential: Some(f.stamp("work", 2).await),
        })
    );
    assert_eq!(f.delivery.warning(f.row.id), None);

    // A second replacement, held again (the lock that waited for the first
    // one's late success is kept as the hold). The failure is planted only
    // after the reply, when the quick host has already finished, so the held
    // host's late delivery is the only one that meets it.
    f.replace("work", THIRD).await;
    let started = Instant::now();
    let failures = f
        .delivery
        .push(
            "work",
            started + crate::sessions::HOST_REPLY_BOUND,
            &f.events,
        )
        .await;
    assert_eq!(
        failures,
        vec![DeliveryFailure {
            host: "fixture host".into(),
            reason: still_delivering(),
        }]
    );
    assert!(
        tokio::fs::read_to_string(&second_credentials)
            .await
            .unwrap()
            .contains(THIRD)
    );
    *f.backend.fail.lock().unwrap() = Some("wrote-agent-credentials".into());
    drop(held);
    let _settled = entry.serial.clone().lock_owned().await;
    assert_eq!(
        f.delivery.warning(f.row.id).as_deref(),
        Some("could not install the managed launch credentials")
    );
    assert!(f.claude().await.contains(SECOND));
    assert_eq!(f.store.credentials().await.unwrap()[0].revision, 3);
}

/// A saved replacement cannot wait indefinitely behind another delivery. The
/// deadline includes the mutex wait, and the committed value survives failure.
#[farhelm_testtrace::test]
async fn replacement_deadline_includes_waiting_for_the_host_lock() {
    let f = Fixture::new(false).await;
    let _lease = f.connected().await;
    f.replace("work", SECOND).await;
    let before = f.operations();
    let entry = f.delivery.entry(&f.row);
    let _held = entry.serial.clone().lock_owned().await;
    tokio::time::pause();
    let mut push = Box::pin(f.delivery.push("work", past_the_ssh_budget(), &f.events));
    assert_pending(push.as_mut());
    tokio::time::advance(COMMAND_TIMEOUT + Duration::from_millis(1)).await;
    let failures = push.await;
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].host, "fixture host");
    assert_eq!(failures[0].reason, TIMED_OUT);
    assert_eq!(f.operations(), before);
    assert!(f.claude().await.contains(FIRST));
    assert_eq!(f.store.credentials().await.unwrap()[0].revision, 2);
}

/// A hook that has not selected credentials owns its own failure report. An
/// early replacement must not supersede that warning and then silently discard
/// its outcome because no active recipe binding ever became available.
#[farhelm_testtrace::test]
async fn an_early_replacement_does_not_hide_an_unready_connections_timeout() {
    let f = Fixture::new(false).await;
    let _lease = f.delivery.enter(&f.row);
    let entry = f.delivery.entry(&f.row);
    let _held = entry.serial.clone().lock_owned().await;
    let mut connect = Box::pin(f.delivery.connect(&f.row, &f.events));
    assert_pending(connect.as_mut());
    assert!(entry.live.lock().unwrap().binding.is_none());
    f.replace("work", SECOND).await;
    tokio::time::pause();
    let mut push = Box::pin(f.delivery.push("work", past_the_ssh_budget(), &f.events));
    let first_poll = push.as_mut().poll(&mut Context::from_waker(Waker::noop()));
    tokio::time::advance(COMMAND_TIMEOUT + Duration::from_millis(1)).await;
    connect.await;
    let failures = match first_poll {
        std::task::Poll::Ready(failures) => failures,
        std::task::Poll::Pending => push.await,
    };
    assert!(failures.is_empty());
    assert_eq!(f.delivery.warning(f.row.id).as_deref(), Some(TIMED_OUT));
    assert!(f.operations().is_empty());
    assert_eq!(f.store.credentials().await.unwrap()[0].revision, 2);
}

/// Suspension cancels a replacement already inside transport work. Opening the
/// gate afterwards must not resume it and overwrite the host's previous file.
/// Starting another lease on the same turn also cannot hide the retired lifetime.
#[farhelm_testtrace::test]
async fn last_connection_lease_cancels_an_in_progress_push() {
    let f = Fixture::new(false).await;
    let lease = f.connected().await;
    f.replace("work", SECOND).await;
    f.backend.block_first.store(true, Ordering::SeqCst);
    let mut push = Box::pin(f.delivery.push("work", past_the_ssh_budget(), &f.events));
    tokio::select! {
        _ = f.backend.entered.notified() => {},
        result = &mut push => panic!("push finished before its transport gate: {} failures", result.len()),
    }
    let stopped_at = f.operations();
    drop(lease);
    let _new_lease = f.delivery.enter(&f.row);
    assert!(
        tokio::time::timeout(Duration::from_secs(5), push)
            .await
            .expect("the new lease must not keep the retired push in its held transport")
            .is_empty()
    );
    f.backend.release.notify_one();
    assert_eq!(f.operations(), stopped_at);
    assert!(f.claude().await.contains(FIRST));
    assert_eq!(
        f.delivery
            .read_record(&f.row)
            .await
            .unwrap()
            .claude_environment,
        Some(EndpointStamp::Endpoint {
            base_url: "https://first.example/v1".into(),
            credential: Some(f.stamp("work", 1).await),
        })
    );
}

/// An older connection ending must not cancel a still-live newer incarnation;
/// cancellation belongs to the last resource bundle, not whichever drops first.
#[farhelm_testtrace::test]
async fn an_old_lease_cannot_cancel_a_second_live_connection() {
    let f = Fixture::new(false).await;
    let old = f.connected().await;
    let _new = f.delivery.enter(&f.row);
    drop(old);
    f.replace("work", SECOND).await;
    assert!(
        f.delivery
            .push("work", past_the_ssh_budget(), &f.events)
            .await
            .is_empty()
    );
    assert!(f.claude().await.contains(SECOND));
}

/// A malformed stamped Codex document withholds Codex from the shared snapshot while Claude proceeds.
#[farhelm_testtrace::test]
async fn codex_cleanup_failure_preserves_the_claude_slot() {
    let f = Fixture::new(true).await;
    f.github_credential().await;
    f.select_codex(false, true, true).await;
    f.stamp_legacy_codex().await;
    tokio::fs::create_dir(f.home.join(".codex")).await.unwrap();
    let codex = f.home.join(".codex/config.toml");
    tokio::fs::write(&codex, b"[invalid fictional-private-marker")
        .await
        .unwrap();
    let _lease = f.delivery.enter(&f.row);
    f.delivery.connect(&f.row, &f.events).await;
    let snapshot = f.snapshot().await;
    assert_eq!(snapshot.env["ANTHROPIC_AUTH_TOKEN"], FIRST);
    let record = f.delivery.read_record(&f.row).await.unwrap();
    assert!(record.claude_environment.is_some() && record.codex_environment.is_none());
    assert!(matches!(record.codex, Some(EndpointStamp::Endpoint { .. })));
    assert!(snapshot.codex_endpoint.is_none());
    let warning = f.delivery.warning(f.row.id).unwrap();
    assert!(!warning.contains("fictional-private-marker"));
    assert_eq!(
        warning,
        "Codex recipe credential cleanup is pending (~/.codex/config.toml is not valid TOML)"
    );
    f.replace("git-work", "fictional-github-replacement").await;
    assert!(
        f.delivery
            .push("git-work", past_the_ssh_budget(), &f.events)
            .await
            .is_empty(),
        "the GitHub save does not inherit the Codex refusal"
    );
    assert_eq!(
        f.delivery.warning(f.row.id).as_deref(),
        Some(warning.as_str())
    );
    let before = f.operations().len();
    tokio::fs::write(&codex, b"unrelated = true\n")
        .await
        .unwrap();
    f.delivery.connect(&f.row, &f.events).await;
    assert_eq!(
        &f.operations()[before..],
        [
            "read-agent-settings",
            "create-directories",
            "wrote-agent-credentials",
        ]
    );
    assert_eq!(f.delivery.warning(f.row.id), None);
    assert!(
        tokio::fs::read_to_string(codex)
            .await
            .unwrap()
            .contains("unrelated = true")
    );
    let snapshot = f.snapshot().await;
    assert_eq!(snapshot.env["ANTHROPIC_AUTH_TOKEN"], FIRST);
    assert_eq!(
        snapshot.env[recipe_credentials::CODEX_ENDPOINT_TOKEN_ENV],
        FIRST
    );
}

/// A fresh host can carry its own Codex config. Preparation may write the old Claude-only file before hello,
/// but only an accepted connection can add a Codex descriptor, with or without a bearer.
#[farhelm_testtrace::test]
async fn tokenless_codex_waits_for_hello_and_keeps_unstamped_settings() {
    let f = Fixture::new(false).await;
    let config = f.home.join(".codex/config.toml");
    tokio::fs::create_dir(config.parent().unwrap())
        .await
        .unwrap();
    let personal = b"# Personal settings\nmodel = 'personal'\n";
    tokio::fs::write(&config, personal).await.unwrap();
    let mut recipe = crate::tensorlake::recipes::tests_support::save();
    recipe.claude = Some(endpoint("https://first.example/v1", "work"));
    recipe.codex = Some(EndpointSave {
        base_url: "https://tokenless.example/v1".into(),
        token_credential: None,
    });
    f.store
        .update_tensorlake_recipe("recipe".into(), recipe)
        .await
        .unwrap()
        .unwrap();

    let bindings = f.delivery.read_bindings(&f.row).await.unwrap();
    assert!(
        bindings.codex.is_some(),
        "the test selected a tokenless Codex endpoint"
    );
    let entry = f.delivery.entry(&f.row);
    let guard = entry.serial.clone().lock_owned().await;
    f.delivery
        .apply(
            &f.row,
            bindings,
            Some(f.home.clone()),
            DeliveryPhase::Setup,
            guard,
        )
        .await
        .unwrap();
    assert!(f.snapshot().await.codex_endpoint.is_none());
    assert!(
        f.delivery
            .read_record(&f.row)
            .await
            .unwrap()
            .codex_environment
            .is_none()
    );

    let _lease = f.connected().await;
    let snapshot = f.snapshot().await;
    let codex = snapshot.codex_endpoint.unwrap();
    assert_eq!(codex.base_url, "https://tokenless.example/v1");
    assert!(!codex.has_token);
    assert!(
        !snapshot
            .env
            .contains_key(recipe_credentials::CODEX_ENDPOINT_TOKEN_ENV)
    );
    assert_eq!(snapshot.env["ANTHROPIC_AUTH_TOKEN"], FIRST);
    assert_eq!(tokio::fs::read(config).await.unwrap(), personal);
    assert!(!f.operations().iter().any(|op| op == "read-agent-settings"));
}

/// Each backend failure is mapped at the delivery boundary; transport context
/// and stderr must not reach the warning or saved-with-failure acknowledgement.
#[farhelm_testtrace::test]
async fn backend_failures_leave_saved_values_and_safe_warnings() {
    for (operation, reason) in [
        (
            "read-home",
            "Codex recipe credential cleanup is pending (could not read the sandbox's home directory)",
        ),
        (
            "read-agent-settings",
            "Codex recipe credential cleanup is pending (could not read the agent settings)",
        ),
        (
            "create-directories",
            "could not prepare the managed launch credentials directory",
        ),
        (
            "write-unit",
            "Codex recipe credential cleanup is pending (could not install the agent settings)",
        ),
        (
            "wrote-agent-credentials",
            "could not install the managed launch credentials",
        ),
    ] {
        let f = Fixture::new(true).await;
        f.stamp_legacy_codex().await;
        let codex = f.home.join(".codex/config.toml");
        tokio::fs::create_dir(codex.parent().unwrap())
            .await
            .unwrap();
        tokio::fs::write(&codex, legacy_codex_config("old"))
            .await
            .unwrap();
        let _lease = f.delivery.enter(&f.row);
        *f.backend.fail.lock().unwrap() = Some(operation.into());
        f.delivery.connect(&f.row, &f.events).await;
        assert_eq!(f.delivery.warning(f.row.id).as_deref(), Some(reason));
        if operation == "read-home" {
            let snapshot = f.snapshot().await;
            assert_eq!(snapshot.env["ANTHROPIC_AUTH_TOKEN"], FIRST);
            assert!(snapshot.codex_endpoint.is_none());
        }
        f.replace("work", SECOND).await;
        let failures = f
            .delivery
            .push("work", past_the_ssh_budget(), &f.events)
            .await;
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].reason, reason);
        assert!(
            !serde_json::to_string(&failures)
                .unwrap()
                .contains("host said no")
        );
        assert_eq!(f.store.credentials().await.unwrap()[0].revision, 2);
        assert!(!f.home.join(".claude/settings.json").exists());
        *f.backend.fail.lock().unwrap() = None;
        assert!(
            f.delivery
                .push("work", past_the_ssh_budget(), &f.events)
                .await
                .is_empty()
        );
        assert_eq!(f.delivery.warning(f.row.id), None);
        let env = f.snapshot().await.env;
        assert_eq!(env["ANTHROPIC_AUTH_TOKEN"], SECOND);
        assert_eq!(env[CODEX_ENDPOINT_TOKEN_ENV], SECOND);
    }
}

/// Cancelling an await cannot release the host lock while its blocking SQLite
/// write is queued. A later delivery must observe that write before proceeding.
#[farhelm_testtrace::test]
async fn a_cancelled_record_write_keeps_serialization_until_sqlite_finishes() {
    let f = Fixture::new(false).await;
    let store = f.store.clone();
    let (entered, ready) = tokio::sync::oneshot::channel();
    let (release, held) = std::sync::mpsc::channel();
    // Dropping release also wakes recv if an assertion unwinds this fixture;
    // runtime teardown never depends on a forgotten blocking-thread gate.
    let blocker = tokio::spawn(async move {
        store
            .conn()
            .call("hold the fixture database", move |_| {
                let _ = entered.send(());
                let _ = held.recv();
                Ok(())
            })
            .await
            .unwrap();
    });
    ready.await.unwrap();
    let entry = f.delivery.entry(&f.row);
    let guard = entry.serial.clone().lock_owned().await;
    let wanted = Record {
        home: Some(f.home.clone()),
        ..Record::default()
    };
    let mut write = Box::pin(f.delivery.write_record(&f.row, &wanted, guard));
    assert_pending(write.as_mut());
    drop(write);
    assert!(entry.serial.clone().try_lock_owned().is_err());
    let before = f.operations();
    let mut next = Box::pin(f.delivery.connect(&f.row, &f.events));
    assert_pending(next.as_mut());
    assert_eq!(f.operations(), before);
    release.send(()).unwrap();
    blocker.await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), next)
        .await
        .unwrap();
    let record = f.delivery.read_record(&f.row).await.unwrap();
    assert_eq!(record.home, Some(f.home.clone()));
    assert!(record.claude_environment.is_some());
    assert!(
        !f.operations()
            .iter()
            .any(|operation| operation == "read-home")
    );
    assert!(f.claude().await.contains(FIRST));
}

use crate::environment::{
    ConnectedResources, Destroyed, EnvironmentBinding, EnvironmentProvider, EnvironmentStatus,
    StatusUnanswered,
};
use crate::manager::HostState;
use crate::rest_harness::{FleetBuilder, Harness};
use crate::tensorlake::Tensorlake;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use farhelm_proto::credentials::SavedCredential;
use std::sync::atomic::AtomicBool;
use tower::ServiceExt as _;

/// Hold the real hook's resources after acquisition but before the actor can
/// publish them. Late fixture assembly happens before any sandbox row exists.
#[derive(Default)]
struct HookGate {
    service: Mutex<Option<Tensorlake>>,
    hold: AtomicBool,
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
}

#[async_trait::async_trait]
impl EnvironmentProvider for HookGate {
    /// This observation fixture must fail if production sends creation or Settings work here.
    async fn places(&self) -> anyhow::Result<Vec<crate::environment::EnvironmentPlace>> {
        unreachable!("observation fixture")
    }
    async fn rebuild_recipe(
        &self,
        _: &str,
        _: farhelm_proto::Place,
        _: std::sync::Arc<crate::feed::FleetEvents>,
    ) -> anyhow::Result<()> {
        unreachable!("observation fixture")
    }
    async fn clean_recipe(
        &self,
        _: &str,
        _: &crate::tensorlake::build::DeleteGuard,
    ) -> anyhow::Result<()> {
        unreachable!("observation fixture")
    }
    async fn ensure_prepared(
        &self,
        _: &str,
        _: &crate::environment::recipes::Recipe,
        _: farhelm_proto::Place,
        _: std::sync::Arc<crate::feed::FleetEvents>,
        _: &dyn crate::environment::PreparationProgress,
    ) -> anyhow::Result<crate::environment::PreparedArtifact> {
        unreachable!("observation fixture")
    }
    async fn create(
        &self,
        _: &crate::environment::EnvironmentBinding,
        _: Option<crate::environment::PreparedArtifact>,
        _: &dyn crate::environment::PreparationProgress,
    ) -> anyhow::Result<crate::environment::CreatedEnvironment> {
        unreachable!("observation fixture")
    }
    async fn list_owned(
        &self,
        _: farhelm_proto::Place,
    ) -> anyhow::Result<Vec<crate::environment::OwnedEnvironment>> {
        unreachable!("observation fixture")
    }

    async fn status(
        &self,
        _binding: &EnvironmentBinding,
    ) -> Result<EnvironmentStatus, StatusUnanswered> {
        Err(StatusUnanswered(
            "fixture asks the supervisor directly".into(),
        ))
    }

    async fn suspend(
        &self,
        _manager: &crate::manager::ConnectionManager,
        _binding: &EnvironmentBinding,
    ) -> anyhow::Result<()> {
        unreachable!("the hook fixture exercises connection resources only")
    }

    async fn resume(
        &self,
        _manager: &crate::manager::ConnectionManager,
        _binding: &EnvironmentBinding,
    ) -> anyhow::Result<()> {
        unreachable!("the hook fixture exercises connection resources only")
    }

    async fn destroy(&self, _binding: &EnvironmentBinding) -> anyhow::Result<Destroyed> {
        unreachable!("the hook fixture exercises connection resources only")
    }

    async fn connected(
        &self,
        row: &HostRow,
        client: Arc<crate::client::SupervisorClient>,
        events: Arc<FleetEvents>,
    ) -> ConnectedResources {
        if row.tensorlake.is_none() {
            return ConnectedResources::default();
        }
        let service = self.service.lock().unwrap().clone().unwrap();
        let resources = EnvironmentProvider::connected(&service, row, client, events).await;
        if self.hold.swap(false, Ordering::SeqCst) {
            self.entered.notify_one();
            self.release.notified().await;
        }
        resources
    }
}

/// Use the ordinary protected router and real actor with the same injected
/// delivery backend. The planted sandbox is scripted before registry sync.
async fn actor_fixture(hold_hook: bool, hold_backend: bool) -> (Harness, Fixture, Arc<HookGate>) {
    let gate = Arc::new(HookGate::default());
    gate.hold.store(hold_hook, Ordering::SeqCst);
    let mut harness = FleetBuilder::new()
        .await
        .sandboxes(gate.clone())
        .start()
        .await;
    let root = tempfile::tempdir().unwrap();
    let mut f = Fixture::with_store(root, harness.store.clone(), false).await;
    f.backend.block_first.store(hold_backend, Ordering::SeqCst);
    let service = crate::provisioning::tests::service(&harness, f.backend.clone(), f._root.path());
    f.delivery = service.credential_delivery();
    harness.state = Arc::new(crate::AppState::with_provisioning(
        harness.manager.clone(),
        harness.store.clone(),
        service,
    ));
    *gate.service.lock().unwrap() = Some(harness.state.tensorlake.clone());
    f.admit().await;
    harness.fleet.edit(f.row.id, |_| {});
    harness.manager.sync_registry().await.unwrap();
    (harness, f, gate)
}

/// Exercise authentication, response decoding and host projection together;
/// no fixture-only route bypasses the save handler or connection actor.
fn request(method: &str, path: &str, body: String) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header("host", "127.0.0.1:7433")
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap()
}

/// Replacement responses may carry metadata and fixed failure reasons only.
async fn replace_route(harness: &Harness, token: &str) -> SavedCredential {
    let response = harness
        .router()
        .oneshot(request(
            "PUT",
            "/api/credentials/work",
            serde_json::to_string(&SaveCredential {
                secret: Some(token.into()),
            })
            .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    assert!(!text.contains(token) && !text.contains(FIRST));
    serde_json::from_slice(&bytes).unwrap()
}

/// A save includes a connection whose real hook has read revision one but has
/// not published Connected yet. Failed subsequent pushes remain successful
/// saves, with a routable host and a matching sanitized host warning.
#[farhelm_testtrace::test]
async fn route_replacement_covers_the_hook_publication_gap_and_reports_failures() {
    let (harness, f, gate) = actor_fixture(true, false).await;
    tokio::time::timeout(Duration::from_secs(5), gate.entered.notified())
        .await
        .unwrap();
    assert!(f.claude().await.contains(FIRST));
    assert!(!matches!(
        harness.manager.state(f.row.id),
        Some(HostState::Connected { .. })
    ));
    assert!(harness.manager.status(f.row.id).unwrap().client.is_none());
    let ack = replace_route(&harness, SECOND).await;
    assert_eq!(ack.credential.revision, 2);
    assert!(ack.delivery_failures.is_empty());
    assert!(f.claude().await.contains(SECOND));
    assert!(harness.manager.status(f.row.id).unwrap().client.is_none());
    gate.release.notify_one();
    harness
        .await_state(f.row.id, |state| {
            matches!(state, HostState::Connected { .. })
        })
        .await;
    assert!(harness.manager.status(f.row.id).unwrap().client.is_some());
    *f.backend.fail.lock().unwrap() = Some("wrote-agent-credentials".into());
    let ack = replace_route(&harness, "fictional-third-token").await;
    assert_eq!(ack.credential.revision, 3);
    assert_eq!(ack.delivery_failures.len(), 1);
    assert_eq!(
        ack.delivery_failures[0].reason,
        "could not install the managed launch credentials"
    );
    assert_eq!(f.store.credentials().await.unwrap()[0].revision, 3);
    assert!(harness.manager.status(f.row.id).unwrap().client.is_some());
    let response = harness
        .router()
        .oneshot(request("GET", "/api/hosts", String::new()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    let hosts: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let host = hosts["hosts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|host| host["id"].as_i64() == Some(f.row.id))
        .unwrap();
    assert_eq!(
        host["tensorlake"]["credential_warning"],
        "could not install the managed launch credentials"
    );
    assert!(
        !std::str::from_utf8(&bytes)
            .unwrap()
            .contains("host said no")
    );
    *f.backend.fail.lock().unwrap() = None;
    assert!(
        replace_route(&harness, "fictional-fourth-token")
            .await
            .delivery_failures
            .is_empty()
    );
    assert_eq!(f.delivery.warning(f.row.id), None);

    // An accepted replacement owns its push even if the HTTP caller vanishes.
    f.backend.block_first.store(true, Ordering::SeqCst);
    let mut saving = Box::pin(
        harness.router().oneshot(request(
            "PUT",
            "/api/credentials/work",
            serde_json::to_string(&SaveCredential {
                secret: Some("fictional-disconnected-caller-token".into()),
            })
            .unwrap(),
        )),
    );
    tokio::select! {
        _ = f.backend.entered.notified() => {},
        result = &mut saving => panic!("save returned before its held push: {:?}", result.map(|response| response.status())),
    }
    drop(saving);
    f.backend.release.notify_one();
    let entry = f.delivery.entry(&f.row);
    let _settled = tokio::time::timeout(Duration::from_secs(5), entry.serial.clone().lock_owned())
        .await
        .unwrap();
    assert!(
        f.claude()
            .await
            .contains("fictional-disconnected-caller-token")
    );
    assert_eq!(f.store.credentials().await.unwrap()[0].revision, 5);
}

/// The reply bound is on the real save route, not only on the delivery: the
/// route is what the GUI's one-at-a-time channel waits on. Specified: against
/// a connected host whose delivery is held at its host lock, with the clock
/// paused, `PUT /api/credentials/{name}` answers 200 once `HOST_REPLY_BOUND`
/// has passed and before the SSH budget would have, acknowledging the new
/// revision and naming the host as not delivered because it did not answer
/// in time. The held delivery then completes after the reply.
#[farhelm_testtrace::test]
async fn the_save_route_answers_at_the_reply_bound_while_a_host_is_held() {
    let (harness, f, _gate) = actor_fixture(false, false).await;
    harness
        .await_state(f.row.id, |state| {
            matches!(state, HostState::Connected { .. })
        })
        .await;
    let entry = f.delivery.entry(&f.row);
    assert!(f.claude().await.contains(FIRST));
    assert_eq!(entry.active.borrow().count, 1);
    let held = entry.serial.clone().lock_owned().await;
    tokio::time::pause();
    let started = Instant::now();
    let ack = replace_route(&harness, SECOND).await;
    let waited = started.elapsed();
    assert!(
        waited >= crate::sessions::HOST_REPLY_BOUND && waited < COMMAND_TIMEOUT,
        "the save answered after {waited:?}"
    );
    assert_eq!(ack.credential.revision, 2);
    assert_eq!(
        ack.delivery_failures,
        vec![DeliveryFailure {
            host: "fixture host".into(),
            reason: still_delivering(),
        }]
    );
    drop(held);
    let _settled = entry.serial.clone().lock_owned().await;
    assert!(f.claude().await.contains(SECOND));
}

/// Parking an actual connection actor cancels its acquisition before replying.
/// Releasing the old backend gate cannot restart file delivery after the hold.
#[farhelm_testtrace::test]
async fn actor_hold_cancels_the_real_hook_before_acknowledging_parking() {
    let (harness, f, _gate) = actor_fixture(false, true).await;
    tokio::time::timeout(Duration::from_secs(5), f.backend.entered.notified())
        .await
        .unwrap();
    assert_eq!(f.delivery.entry(&f.row).active.borrow().count, 1);
    assert!(harness.manager.status(f.row.id).unwrap().client.is_none());
    let shown = harness.manager.state(f.row.id).unwrap();
    assert!(harness.manager.hold_connection(f.row.id, shown).await);
    assert_eq!(f.delivery.entry(&f.row).active.borrow().count, 0);
    let before = f.operations();
    f.backend.release.notify_one();
    assert_eq!(f.operations(), before);
    assert!(!f.claude_path().exists());
    harness.manager.release_hold(f.row.id, true);
    harness
        .await_state(f.row.id, |state| {
            matches!(state, HostState::Connected { .. })
        })
        .await;
    assert!(f.claude().await.contains(FIRST));
    assert_eq!(f.delivery.entry(&f.row).active.borrow().count, 1);
    let before = f.operations();
    let previous_dials = harness.fleet.dials(f.row.id);
    harness.fleet.kill_connection(f.row.id);
    // Connecting can begin and end between state polls. A new dial followed
    // by Connected proves a replacement without requiring that transient view.
    harness
        .await_state(f.row.id, |state| {
            harness.fleet.dials(f.row.id) > previous_dials
                && matches!(state, HostState::Connected { .. })
        })
        .await;
    assert_eq!(f.operations(), before);
}
