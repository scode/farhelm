//! Container coordinates are durable before the first create is sent.
//!
//! The host row owns cleanup even when creation has an unanswered outcome.
//! Its place is a foreign key to an ordinary host, whose editable endpoint
//! stays authoritative for every later attempt.

use super::*;
use crate::access::ContainerUser;

/// The container's immutable place and login identity, with its observed ID.
/// A reserved name remains available for cleanup before an ID is observed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PodmanBinding {
    pub name: String,
    pub recipe: String,
    pub place_host: HostId,
    pub container_id: Option<String>,
    /// Pending registration precedes preparation, so the image's account is
    /// unknown until observation. All login fields become durable together
    /// with pending-send/install facts, before run or canonical ID admission.
    pub login: Option<PodmanLogin>,
    pub pending_create_at: Option<i64>,
}

/// The account actually observed inside the selected image, never guessed
/// from a recipe's literal user selection. Stored nullable columns are all-or-none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PodmanLogin {
    pub user: String,
    pub home: String,
    pub uid: u32,
}

impl PodmanBinding {
    /// Exec needs the whole login identity because Podman does no login setup.
    pub(crate) fn login_user(&self) -> anyhow::Result<ContainerUser> {
        let login = self
            .login
            .as_ref()
            .context("the container's image account has not been observed")?;
        Ok(ContainerUser {
            name: login.user.clone(),
            home: login.home.clone(),
            uid: login.uid,
        })
    }
}

/// Registration facts chosen before any container-side work begins.
/// The provider must retain this name across the create and cleanup attempt.
pub struct NewPodmanHost {
    pub name: String,
    pub recipe: String,
    pub place_host: HostId,
    pub user: String,
    pub home: String,
    pub uid: u32,
}

/// Connection-defining facts frozen by a provisioning probe. Cache refreshes
/// and decorative edits do not invalidate a plan, but a different container or
/// installation does. The write checks these facts again in its own SQL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PodmanInstallGuard {
    pub(crate) host: HostId,
    entry_uuid: String,
    binding: PodmanBinding,
    pub(crate) farhelm: Option<String>,
    pub(crate) state_dir: Option<String>,
}

impl PodmanInstallGuard {
    /// Freeze only admission facts. Aliases, appearance, permission grants and
    /// session-cache updates cannot change which installation was inspected.
    pub(crate) fn of(row: &HostRow) -> anyhow::Result<Self> {
        Ok(Self {
            host: row.id,
            entry_uuid: row.entry_uuid.clone(),
            binding: row
                .podman
                .clone()
                .context("the host has no container binding")?,
            farhelm: row.remote_farhelm.clone(),
            state_dir: row.remote_state_dir.clone(),
        })
    }
}

/// Append container storage without changing the host table's identity.
/// Historical version-only fixtures can already carry some of these columns.
pub(super) fn migrate(tx: &rusqlite::Transaction<'_>) -> anyhow::Result<()> {
    for (name, definition) in [
        ("environment_provider", "environment_provider TEXT"),
        ("place_host", "place_host INTEGER REFERENCES hosts(id)"),
        ("podman_container_id", "podman_container_id TEXT"),
        ("podman_user", "podman_user TEXT"),
        ("podman_home", "podman_home TEXT"),
        ("podman_uid", "podman_uid INTEGER"),
        (
            "podman_pending_create_at",
            "podman_pending_create_at INTEGER",
        ),
    ] {
        let exists: bool = tx.query_row(
            "SELECT count(*) != 0 FROM pragma_table_info('hosts') WHERE name = ?1",
            [name],
            |row| row.get(0),
        )?;
        if !exists {
            tx.execute_batch(&format!("ALTER TABLE hosts ADD COLUMN {definition};"))?;
        }
    }
    tx.execute_batch("CREATE INDEX IF NOT EXISTS hosts_place_host ON hosts(place_host);")?;
    Ok(())
}

/// Validate recorded coordinates as input, including a hand-edited database.
/// Unknown provider spellings are refused by the shared kind decoder first.
#[allow(clippy::too_many_arguments)]
pub(super) fn decode_binding(
    kind: HostKind,
    name: Option<&str>,
    recipe: Option<&str>,
    place_host: Option<HostId>,
    container_id: Option<String>,
    user: Option<String>,
    home: Option<String>,
    uid: Option<u32>,
    pending_create_at: Option<i64>,
    session_owned: bool,
    admitted: bool,
) -> anyhow::Result<Option<PodmanBinding>> {
    match kind {
        HostKind::Local | HostKind::Ssh | HostKind::Tensorlake => {
            anyhow::ensure!(
                place_host.is_none(),
                "an ordinary or Tensorlake host names a container place"
            );
            Ok(None)
        }
        HostKind::Podman => {
            anyhow::ensure!(session_owned, "a Podman environment must be session-owned");
            let binding = PodmanBinding {
                name: name.context("a Podman row has no environment name")?.into(),
                recipe: recipe.context("a Podman row has no recipe")?.into(),
                place_host: place_host.context("a Podman row has no place host")?,
                container_id,
                login: match (user, home, uid) {
                    (Some(user), Some(home), Some(uid)) => Some(PodmanLogin { user, home, uid }),
                    (None, None, None) => None,
                    _ => anyhow::bail!("a Podman row has only part of its login identity"),
                },
                pending_create_at,
            };
            validate_name(&binding.name)?;
            if let Some(login) = &binding.login {
                validate_identity(&binding.name, &login.user, &login.home)?;
            }
            anyhow::ensure!(
                binding.login.is_some()
                    || (binding.container_id.is_none()
                        && binding.pending_create_at.is_none()
                        && !admitted),
                "a created or admitted container has no observed login identity"
            );
            if let Some(id) = &binding.container_id {
                validate_container_id(id)?;
            }
            Ok(Some(binding))
        }
    }
}

/// Restrict the persisted identity to the provider's reserved-name namespace.
/// Shell quoting still applies later, but validation catches corrupt state here.
fn validate_identity(name: &str, user: &str, home: &str) -> anyhow::Result<()> {
    validate_name(name)?;
    anyhow::ensure!(
        !user.is_empty() && user.len() <= 4096 && !user.contains(char::is_control),
        "invalid container login user"
    );
    anyhow::ensure!(
        home.starts_with('/') && !home.contains('\0') && !home.contains('\n'),
        "invalid container login home"
    );
    Ok(())
}

fn validate_name(name: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        name.starts_with("farhelm-sess-")
            && name.len() > "farhelm-sess-".len()
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte)),
        "invalid reserved container name"
    );
    Ok(())
}

/// Podman's full lowercase ID pins a resource independently of a reused name.
fn validate_container_id(id: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        id.len() == 64
            && id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "Podman returned a noncanonical container ID"
    );
    Ok(())
}

/// Refuse endpoint loss in the same transaction as the retarget or removal.
/// Every referring row counts, including an unresolved or stopped environment.
pub(super) fn refuse_referenced_place(conn: &Connection, host: HostId) -> anyhow::Result<()> {
    let mut statement =
        conn.prepare("SELECT tensorlake_sandbox FROM hosts WHERE place_host = ?1 ORDER BY id")?;
    let names = statement
        .query_map([host], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    anyhow::ensure!(
        names.is_empty(),
        "this host runs environments that must be deleted first: {}",
        names.join(", ")
    );
    let mut statement =
        conn.prepare("SELECT name, state_json FROM tensorlake_recipes ORDER BY name")?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut recipes = Vec::new();
    for (name, json) in rows {
        let state: crate::environment::recipes::RecipeState = serde_json::from_str(&json)
            .map_err(|_| anyhow::anyhow!("could not read resource ownership for recipe {name}; this host was left unchanged"))?;
        if state
            .podman
            .get(&host)
            .is_some_and(crate::environment::recipes::PodmanRecipeState::needs_cleanup)
        {
            recipes.push(name);
        }
    }
    anyhow::ensure!(
        recipes.is_empty(),
        "this host holds recipe images or helpers for {}; clean up those recipes on this host before removing or retargeting it",
        recipes.join(", ")
    );
    Ok(())
}

impl HelmStore {
    /// Freeze the installation selected for this exact create before sending
    /// Podman any request. A retry must use these facts even after renewal.
    pub(crate) async fn set_podman_prepared_create(
        &self,
        expected: PodmanInstallGuard,
        sent: i64,
        facts: &crate::environment::recipes::PreparedInstallFacts,
        login: &ContainerUser,
    ) -> anyhow::Result<bool> {
        validate_identity(&expected.binding.name, &login.name, &login.home)?;
        let json = serde_json::to_string(facts)?;
        let login = login.clone();
        let binding = expected.binding;
        let old = binding.login;
        self.conn.call("record prepared container create task panicked", move |conn| {
            Ok(conn.execute("UPDATE hosts SET podman_pending_create_at = ?2, tensorlake_prepared_install_json = ?3,
                    podman_user = ?4, podman_home = ?5, podman_uid = ?6
                WHERE id = ?1 AND entry_uuid = ?7 AND environment_provider = 'podman' AND podman_container_id IS NULL
                  AND remote_farhelm IS NULL AND remote_state_dir IS NULL AND tensorlake_sandbox = ?8
                  AND place_host = ?9 AND tensorlake_recipe = ?10 AND podman_pending_create_at IS ?11
                  AND podman_user IS ?12 AND podman_home IS ?13 AND podman_uid IS ?14",
                rusqlite::params![expected.host, sent, json, login.name, login.home, login.uid, expected.entry_uuid,
                    binding.name, binding.place_host, binding.recipe, binding.pending_create_at,
                    old.as_ref().map(|login| &login.user), old.as_ref().map(|login| &login.home), old.as_ref().map(|login| login.uid)])? == 1)
        }).await
    }

    /// Recovery reads the created container's own selected installation,
    /// never the recipe's newer prepared image. Admission clears this record.
    pub(crate) async fn podman_prepared_install(
        &self,
        host: HostId,
    ) -> anyhow::Result<Option<crate::environment::recipes::PreparedInstallFacts>> {
        self.conn.call("read prepared container create task panicked", move |conn| {
            let json: Option<String> = conn.query_row(
                "SELECT tensorlake_prepared_install_json FROM hosts WHERE id = ?1 AND environment_provider = 'podman'",
                [host], |row| row.get(0)).optional()?.flatten();
            json.map(|json| serde_json::from_str(&json).map_err(|_| anyhow::anyhow!("this container has unreadable prepared installation facts"))).transpose()
        }).await
    }

    /// Admit only the same container and install coordinates the provisioning
    /// phase inspected. A changed row must be probed again before publishing.
    pub(crate) async fn admit_podman_install(
        &self,
        expected: PodmanInstallGuard,
        farhelm: &str,
        state_dir: &str,
    ) -> anyhow::Result<bool> {
        require_storable_remote_paths(Some(farhelm), Some(state_dir))
            .map_err(anyhow::Error::new)?;
        let binding = expected.binding;
        anyhow::ensure!(
            binding.container_id.is_some(),
            "the container create has no confirmed identity"
        );
        let login = binding
            .login
            .context("the container has no observed login identity")?;
        let farhelm = farhelm.to_string();
        let state_dir = state_dir.to_string();
        self.conn.call("admit container install task panicked", move |conn| {
            Ok(conn.execute(
                "UPDATE hosts SET remote_farhelm = ?1, remote_state_dir = ?2, tensorlake_prepared_install_json = NULL
                 WHERE id = ?3 AND entry_uuid = ?4 AND environment_provider = 'podman'
                   AND session_owned = 1 AND tensorlake_sandbox = ?5
                   AND place_host = ?6 AND podman_container_id IS ?7
                   AND podman_user = ?8 AND podman_home = ?9 AND podman_uid = ?10
                   AND remote_farhelm IS ?11 AND remote_state_dir IS ?12
                   AND tensorlake_recipe = ?13 AND podman_pending_create_at IS ?14",
                rusqlite::params![farhelm, state_dir, expected.host, expected.entry_uuid,
                    binding.name, binding.place_host, binding.container_id, login.user,
                    login.home, login.uid, expected.farhelm, expected.state_dir,
                    binding.recipe, binding.pending_create_at],
            )? == 1)
        }).await
    }

    /// Complete-coordinate fixtures preserve the existing registration seam.
    /// Production registers pending ownership before observing an image login.
    #[cfg(test)]
    pub(crate) async fn register_session_container(
        &self,
        new: NewPodmanHost,
    ) -> anyhow::Result<HostId> {
        validate_identity(&new.name, &new.user, &new.home)?;
        self.register_container(
            new.name,
            new.recipe,
            new.place_host,
            Some(PodmanLogin {
                user: new.user,
                home: new.home,
                uid: new.uid,
            }),
        )
        .await
    }

    /// Launch progress owns a host row before preparation observes a custom
    /// image's account. No guessed home or UID becomes an execution route.
    pub(crate) async fn register_pending_session_container(
        &self,
        name: String,
        recipe: String,
        place: HostId,
    ) -> anyhow::Result<HostId> {
        self.register_container(name, recipe, place, None).await
    }

    async fn register_container(
        &self,
        name: String,
        recipe: String,
        place: HostId,
        login: Option<PodmanLogin>,
    ) -> anyhow::Result<HostId> {
        validate_name(&name)?;
        anyhow::ensure!(!recipe.is_empty(), "a container requires a recipe");
        self.conn.call("register container task panicked", move |conn| {
            let tx = conn.transaction()?;
            let (kind, environment_name, provider): (String, Option<String>, Option<String>) = tx.query_row(
                "SELECT kind, tensorlake_sandbox, environment_provider FROM hosts WHERE id = ?1",
                [place], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            ).context("reading the container place")?;
            anyhow::ensure!(HostKind::from_columns(&kind, environment_name.as_deref(), provider.as_deref())?.can_host_environments(),
                "only a local or ordinary SSH host can hold containers");
            let destination = format!("podman:{name}");
            new_entry_conflict(&tx, &destination, &NewHostEntry::new(None)?)?;
            tx.execute(
                "INSERT INTO hosts (kind, destination, entry_uuid, yolo_without_asking,
                 tensorlake_sandbox, tensorlake_recipe, session_owned, environment_provider,
                 place_host, podman_user, podman_home, podman_uid)
                 VALUES ('ssh', ?1, ?2, 1, ?3, ?4, 1, 'podman', ?5, ?6, ?7, ?8)",
                rusqlite::params![destination, uuid::Uuid::new_v4().to_string(), name, recipe,
                    place, login.as_ref().map(|login| &login.user), login.as_ref().map(|login| &login.home), login.as_ref().map(|login| login.uid)],
            )?;
            let host = tx.last_insert_rowid();
            tx.commit()?;
            Ok(host)
        }).await
    }

    /// Persist an unanswered create before the provider sends it. Clearing the
    /// marker requires a definite refusal or a settled provider observation.
    /// Production writes selected facts/login in the same transaction instead.
    #[cfg(test)]
    pub(crate) async fn set_podman_pending_create(
        &self,
        host: HostId,
        sent: Option<i64>,
    ) -> anyhow::Result<bool> {
        self.conn.call("record pending container create task panicked", move |conn| {
            Ok(conn.execute("UPDATE hosts SET podman_pending_create_at = ?2 WHERE id = ?1 AND environment_provider = 'podman'
                AND (?2 IS NULL OR (podman_user IS NOT NULL AND podman_home IS NOT NULL AND podman_uid IS NOT NULL))",
                rusqlite::params![host, sent])? == 1)
        }).await
    }

    /// Record the canonical ID once create or an ownership-checked lookup
    /// observes it. Repeated observations must agree with the original ID.
    pub(crate) async fn set_podman_container_id(
        &self,
        host: HostId,
        id: &str,
    ) -> anyhow::Result<bool> {
        validate_container_id(id)?;
        let id = id.to_owned();
        self.conn
            .call("record container ID task panicked", move |conn| {
                Ok(conn.execute(
                    "UPDATE hosts SET podman_container_id = ?2, podman_pending_create_at = NULL
                 WHERE id = ?1 AND environment_provider = 'podman'
                 AND podman_user IS NOT NULL AND podman_home IS NOT NULL AND podman_uid IS NOT NULL
                 AND (podman_container_id IS NULL OR podman_container_id = ?2)",
                    rusqlite::params![host, id],
                )? == 1)
            })
            .await
    }

    /// Clear only the send whose completed refusal and absence the provider
    /// just confirmed. Selected image/login facts remain available, but no
    /// possible resource is owned by this send anymore. A replaced row or a
    /// newer create cannot be settled by an earlier command's result.
    pub(crate) async fn clear_refused_podman_create(
        &self,
        expected: PodmanInstallGuard,
        sent: i64,
    ) -> anyhow::Result<bool> {
        let binding = expected.binding;
        self.conn
            .call(
                "settle refused container create task panicked",
                move |conn| {
                    Ok(conn.execute(
                        "UPDATE hosts SET podman_pending_create_at = NULL
                WHERE id = ?1 AND entry_uuid = ?2 AND environment_provider = 'podman'
                  AND tensorlake_sandbox = ?3 AND place_host = ?4 AND tensorlake_recipe = ?5
                  AND podman_container_id IS NULL AND podman_pending_create_at = ?6
                  AND remote_farhelm IS NULL AND remote_state_dir IS NULL",
                        rusqlite::params![
                            expected.host,
                            expected.entry_uuid,
                            binding.name,
                            binding.place_host,
                            binding.recipe,
                            sent
                        ],
                    )? == 1)
                },
            )
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pending row owns cleanup before a custom image reveals its account.
    /// Neither pending-send nor canonical ID can skip that observation, and
    /// the observed account/facts write must reject a changed binding.
    #[farhelm_testtrace::test]
    async fn pending_login_and_selected_facts_are_admitted_atomically_before_send() {
        let directory = tempfile::tempdir().unwrap();
        let store = HelmStore::open(&directory.path().join("helm.db"))
            .await
            .unwrap();
        let place = store.list_hosts().await.unwrap()[0].id;
        let host = store
            .register_pending_session_container("farhelm-sess-observed".into(), "dev".into(), place)
            .await
            .unwrap();
        let row = store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == host)
            .unwrap();
        assert!(row.podman.as_ref().unwrap().login.is_none());
        assert!(
            !store
                .set_podman_pending_create(host, Some(42))
                .await
                .unwrap()
        );
        assert!(
            !store
                .set_podman_container_id(host, &"a".repeat(64))
                .await
                .unwrap()
        );
        let facts = crate::environment::recipes::PreparedInstallFacts {
            home: "/home/image-user".into(),
            farhelm_path: "/home/image-user/farhelm".into(),
            state_dir: "/home/image-user/state".into(),
            farhelm_version: "fixture-version".into(),
        };
        let login = ContainerUser {
            name: "image-user".into(),
            home: "/home/image-user".into(),
            uid: 1234,
        };
        let guard = PodmanInstallGuard::of(&row).unwrap();
        assert!(
            store
                .set_podman_prepared_create(guard.clone(), 42, &facts, &login)
                .await
                .unwrap()
        );
        assert!(
            !store
                .set_podman_prepared_create(guard, 43, &facts, &login)
                .await
                .unwrap(),
            "the old no-login binding cannot overwrite a sent create"
        );
        let current = store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == host)
            .unwrap();
        let binding = current.podman.unwrap();
        assert_eq!(binding.login_user().unwrap(), login);
        assert_eq!(binding.pending_create_at, Some(42));
        assert_eq!(
            store.podman_prepared_install(host).await.unwrap(),
            Some(facts)
        );
        assert!(
            store
                .set_podman_container_id(host, &"a".repeat(64))
                .await
                .unwrap()
        );
    }

    /// A fixture chooses all creation coordinates before provider work, so
    /// roundtrip assertions can distinguish durable intent from inferred state.
    fn registration(place_host: HostId, suffix: &str) -> NewPodmanHost {
        NewPodmanHost {
            name: format!("farhelm-sess-test-{suffix}"),
            recipe: "dev".into(),
            place_host,
            user: "developer".into(),
            home: "/home/developer".into(),
            uid: 1000,
        }
    }

    /// Pending creation and canonical identity must survive a helm restart.
    /// Ordinary SSH registration must never be able to counterfeit this kind.
    #[farhelm_testtrace::test]
    async fn container_registration_roundtrips_pending_and_canonical_id() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("helm.db");
        let store = HelmStore::open(&path).await.unwrap();
        let place = store.list_hosts().await.unwrap()[0].id;
        let host = store
            .register_session_container(registration(place, "roundtrip"))
            .await
            .unwrap();
        let initial = store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == host)
            .unwrap();
        assert_eq!(initial.kind, HostKind::Podman);
        assert!(initial.session_owned && initial.yolo_without_asking);
        assert!(initial.tensorlake.is_none());
        assert_eq!(initial.podman.as_ref().unwrap().container_id, None);
        assert!(
            store
                .add_ssh_host("podman:forged", None, None)
                .await
                .is_err()
        );
        assert!(
            store
                .set_podman_pending_create(host, Some(123))
                .await
                .unwrap()
        );
        drop(store);
        let store = HelmStore::open(&path).await.unwrap();
        let pending = store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == host)
            .unwrap();
        assert_eq!(
            pending.podman.as_ref().unwrap().pending_create_at,
            Some(123)
        );
        assert!(store.set_podman_container_id(host, "short").await.is_err());
        let id = "a".repeat(64);
        assert!(store.set_podman_container_id(host, &id).await.unwrap());
        assert!(
            !store
                .set_podman_container_id(host, &"b".repeat(64))
                .await
                .unwrap()
        );
        let recorded = store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == host)
            .unwrap();
        assert_eq!(
            recorded.podman.as_ref().unwrap().container_id.as_deref(),
            Some(id.as_str())
        );
        assert_eq!(recorded.podman.as_ref().unwrap().pending_create_at, None);
        assert!(
            store.remove_ssh_host(host).await.is_err(),
            "forgetting an owned resource needs provider proof"
        );
        assert!(
            store
                .register_session_container(registration(host, "nested"))
                .await
                .is_err()
        );
    }

    /// Refusals cover every referring row before a container exists, and the
    /// database foreign key guards writers that bypass the public store API.
    #[farhelm_testtrace::test]
    async fn place_removal_and_retarget_name_all_referring_containers() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("helm.db");
        let store = HelmStore::open(&path).await.unwrap();
        let place = store
            .add_ssh_host("place.example", None, None)
            .await
            .unwrap();
        store
            .register_session_container(registration(place, "first"))
            .await
            .unwrap();
        store
            .register_session_container(registration(place, "second"))
            .await
            .unwrap();
        for result in [
            store.remove_ssh_host(place).await,
            store.update_ssh_destination(place, "new.example").await,
        ] {
            let reason = result.unwrap_err().to_string();
            assert!(reason.contains("farhelm-sess-test-first"), "{reason}");
            assert!(reason.contains("farhelm-sess-test-second"), "{reason}");
        }
        let conn = Connection::open(path).unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        assert!(
            conn.execute("DELETE FROM hosts WHERE id = ?1", [place])
                .is_err()
        );
    }

    /// The provider column is part of every kind decode, including scalar
    /// appearance and alias readers that could otherwise misclassify corruption.
    #[farhelm_testtrace::test]
    async fn unknown_environment_provider_refuses_registry_reads_and_writes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("helm.db");
        let store = HelmStore::open(&path).await.unwrap();
        let place = store.list_hosts().await.unwrap()[0].id;
        let host = store
            .register_session_container(registration(place, "unknown"))
            .await
            .unwrap();
        let conn = Connection::open(path).unwrap();
        conn.execute(
            "UPDATE hosts SET environment_provider = 'future' WHERE id = ?1",
            [host],
        )
        .unwrap();
        assert!(store.list_hosts().await.is_err());
        assert!(store.update_alias(host, Some("new alias")).await.is_err());
        assert!(store.remove_ssh_host(host).await.is_err());
    }

    /// A probe cannot refresh a row whose provider changed to an unknown
    /// discriminator. Decode inside the write transaction so earlier reads
    /// cannot authorize convergence after the row changes.
    #[farhelm_testtrace::test]
    async fn unknown_provider_refuses_discovered_ssh_convergence_without_mutation() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("helm.db");
        let store = HelmStore::open(&path).await.unwrap();
        let destination = "kept@unknown.example";
        let host = store
            .add_ssh_host(destination, Some("/old/farhelm"), Some("/old/state"))
            .await
            .unwrap();
        let conn = Connection::open(path).unwrap();
        conn.execute(
            "UPDATE hosts SET tensorlake_sandbox = 'kept', environment_provider = 'future' WHERE id = ?1",
            [host],
        ).unwrap();
        let coordinates = || {
            conn.query_row(
                "SELECT remote_farhelm, remote_state_dir, host_identity FROM hosts WHERE id = ?1",
                [host],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .unwrap()
        };
        let before = coordinates();
        assert_eq!(
            before,
            (Some("/old/farhelm".into()), Some("/old/state".into()), None)
        );

        assert!(
            store
                .register_probed_ssh_host(
                    destination,
                    Some("/new/farhelm"),
                    Some("/new/state"),
                    Some("new-identity"),
                    None,
                )
                .await
                .is_err()
        );

        assert_eq!(coordinates(), before);
    }

    /// A schema upgrade must preserve the table itself and every existing
    /// host binding. Rebuilding hosts would cascade unrelated dependent state.
    #[farhelm_testtrace::test]
    async fn schema_83_appends_columns_without_rebuilding_hosts() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("helm.db");
        let store = HelmStore::open(&path).await.unwrap();
        let ssh = store
            .add_ssh_host("existing.example", None, None)
            .await
            .unwrap();
        store
            .register_tensorlake_host(NewTensorlakeHost {
                destination: "old@old.sandbox.tensorlake.ai".into(),
                entry: NewHostEntry::new(Some("old sandbox")).unwrap(),
                sandbox: "old".into(),
                recipe: "dev".into(),
                resources: farhelm_proto::tensorlake::recipes::RecipeSize {
                    cpus: 2,
                    memory_gb: 4,
                    disk_gb: 10,
                },
            })
            .await
            .unwrap();
        let before = store.list_hosts().await.unwrap();
        drop(store);
        let rootpage;
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE migration_sentinel (host_id INTEGER REFERENCES hosts(id) ON DELETE CASCADE, payload TEXT);
                 DROP INDEX hosts_place_host;
                 ALTER TABLE hosts DROP COLUMN environment_provider;
                 ALTER TABLE hosts DROP COLUMN place_host;
                 ALTER TABLE hosts DROP COLUMN podman_container_id;
                 ALTER TABLE hosts DROP COLUMN podman_user;
                 ALTER TABLE hosts DROP COLUMN podman_home;
                 ALTER TABLE hosts DROP COLUMN podman_uid;
                 ALTER TABLE hosts DROP COLUMN podman_pending_create_at;
                 PRAGMA user_version = 82;"
            ).unwrap();
            conn.execute(
                "INSERT INTO migration_sentinel VALUES (?1, 'retained')",
                [ssh],
            )
            .unwrap();
            rootpage = conn
                .query_row(
                    "SELECT rootpage FROM sqlite_master WHERE name = 'hosts'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap();
        }
        let store = HelmStore::open(&path).await.unwrap();
        assert_eq!(store.list_hosts().await.unwrap(), before);
        let conn = Connection::open(path).unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT rootpage FROM sqlite_master WHERE name = 'hosts'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            rootpage
        );
        assert_eq!(
            conn.query_row("SELECT payload FROM migration_sentinel", [], |row| row
                .get::<_, String>(
                0
            ))
            .unwrap(),
            "retained"
        );
    }

    /// A delayed provisioning completion cannot overwrite coordinates admitted
    /// by another attempt. Cosmetic changes remain outside this guard.
    #[farhelm_testtrace::test]
    async fn container_install_admission_is_bound_to_the_inspected_coordinates() {
        let directory = tempfile::tempdir().unwrap();
        let store = HelmStore::open(&directory.path().join("helm.db"))
            .await
            .unwrap();
        let place = store.list_hosts().await.unwrap()[0].id;
        let host = store
            .register_session_container(registration(place, "admission"))
            .await
            .unwrap();
        store
            .set_podman_container_id(host, &"a".repeat(64))
            .await
            .unwrap();
        let row = store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == host)
            .unwrap();
        let guard = PodmanInstallGuard::of(&row).unwrap();
        assert!(
            store
                .admit_podman_install(guard.clone(), "/bin/farhelm", "/home/developer/state")
                .await
                .unwrap()
        );
        assert!(
            !store
                .admit_podman_install(guard, "/other/farhelm", "/other/state")
                .await
                .unwrap()
        );
    }
}
