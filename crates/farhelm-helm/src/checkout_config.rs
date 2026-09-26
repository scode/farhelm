//! Checkout configuration: WHERE fresh checkouts are created (a working-copy
//! root) and WHAT runs after cloning (an optional post-clone shell command),
//! plus the `farhelm helm checkout-config` CLI that edits it.
//!
//! The shape is a global setting with optional per-host overrides, stored in
//! helm.db (schema version 27 — see [`crate::store::CHECKOUT_CONFIG_SCHEMA`]
//! for the tables). Preview and create consume the same resolved snapshot.
//! The running helm observes CLI writes and invalidates browser readers; this
//! module deliberately touches no host, creates no directories, and never
//! runs the configured hook.
//!
//! ## Inheritance semantics (binding)
//!
//! - A NULL override (or no override row at all) means INHERIT the global
//!   value for that field. Clearing a host override returns that field to
//!   inheritance; clearing a global value unsets it.
//! - An EMPTY post-clone string is an explicit DISABLED. It is stored
//!   verbatim and normalized to "no hook" only AFTER inheritance resolves,
//!   so a host override of `""` disables a hook that the global setting
//!   would otherwise provide — distinguishable from NULL, which would
//!   inherit it.
//! - Every ACTUAL change to a stored value (global or override) increments
//!   the global `revision`, in the same transaction as the change. Writing
//!   the value that is already there bumps nothing, so the revision is a
//!   cheap changed-only signal for future readers.
//! - Overrides are config for the REGISTRY ROW: a retarget or adoption of
//!   that row leaves them in place (they say nothing about the machine that
//!   used to be behind it), while removing the host cascades them away.

use crate::store::{HelmStore, HostId, HostRow};
use anyhow::Context;
use rusqlite::OptionalExtension;
use std::path::{Path, PathBuf};

/// Publish committed external configuration changes through the ordinary
/// invalidation feed. The CLI writes SQLite directly, so an in-process setter
/// callback would miss precisely the edits an open composer needs to observe.
///
/// The serving future owns this loop in its select: shutdown drops it, rather
/// than leaving a detached watcher holding the database open. Reads are serial
/// and the interval starts after each read; a slow database cannot accumulate
/// work or cause catch-up bursts. A failed read preserves the last observation.
pub(crate) async fn watch_revision(
    store: HelmStore,
    events: std::sync::Arc<crate::feed::FleetEvents>,
    mut observed: i64,
) {
    let mut read_failed = false;
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        match publish_revision_change(&store, &events, &mut observed).await {
            Ok(_) => read_failed = false,
            Err(error) => {
                if !read_failed {
                    tracing::warn!(%error, "cannot observe checkout configuration changes");
                }
                read_failed = true;
            }
        }
    }
}

/// Advance only after a successful database read and notify only on a change.
/// The caller establishes its baseline before serving browsers, closing the
/// startup gap in which a preview could otherwise precede the first observation.
async fn publish_revision_change(
    store: &HelmStore,
    events: &crate::feed::FleetEvents,
    observed: &mut i64,
) -> anyhow::Result<bool> {
    let revision = store.checkout_config_snapshot(None).await?.revision;
    if revision == *observed {
        return Ok(false);
    }
    *observed = revision;
    events.bump();
    Ok(true)
}

/// Longest configured checkout root accepted, bytes (Design A's cap).
///
/// Enforced at set time for global and host settings alike, before any
/// database write: a root the filesystem layer later cannot canonicalize
/// or name in an error is a config bug, not a runtime surprise. UTF-8
/// multi-byte characters count per byte, matching how the path travels.
pub const MAX_ROOT_BYTES: usize = 4096;

/// Longest configured post-clone hook accepted, bytes (Design A's cap).
///
/// Enforced at set time; the resolved hook travels in private supervisor
/// persistence, and errors never quote the body back.
pub const MAX_POST_CLONE_BYTES: usize = 16 * 1024;

/// The effective checkout configuration for one scope: the global value
/// with any per-host override applied per field, after inheritance and
/// empty-string normalization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCheckoutConfig {
    /// Where fresh checkouts are created. Stored unexpanded — `~` and
    /// `~/...` are expanded by the target supervisor, never here.
    pub root: Option<String>,
    /// The post-clone shell command, if one is configured. `None` covers
    /// unset, inherited-nothing, AND a host's explicit empty-string
    /// disable; use [`Self::config_revision`] or the snapshot view when
    /// the distinction matters.
    pub post_clone: Option<String>,
    /// The global revision at the moment of this snapshot: bumped on every
    /// actual stored change, stable across no-op writes.
    pub config_revision: i64,
}

/// Everything stored for one scope, BEFORE inheritance: what `show`
/// renders and what [`resolve_snapshot`] turns into a
/// [`ResolvedCheckoutConfig`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckoutConfigSnapshot {
    pub global_root: Option<String>,
    pub global_post_clone: Option<String>,
    pub revision: i64,
    /// The host's override row, when one exists. `None` covers both "no
    /// host requested" and "no override row for this host".
    pub host_override: Option<CheckoutHostOverride>,
}

/// One host's stored overrides. `None` per field means inherit; a
/// post-clone of `Some("")` means explicitly disable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckoutHostOverride {
    pub root: Option<String>,
    pub post_clone: Option<String>,
}

/// Which column a setter writes. A tiny enum rather than `&str` column
/// names so no caller can pass an arbitrary identifier into the SQL.
#[derive(Debug, Clone, Copy)]
enum Field {
    Root,
    PostClone,
}

impl Field {
    fn column(self) -> &'static str {
        match self {
            Field::Root => "root",
            Field::PostClone => "post_clone",
        }
    }
}

impl HelmStore {
    /// Read everything stored for one scope in one database snapshot. No
    /// caching: every call re-reads, so a value is never served stale
    /// against a write another connection just committed.
    pub async fn checkout_config_snapshot(
        &self,
        host: Option<HostId>,
    ) -> anyhow::Result<CheckoutConfigSnapshot> {
        let conn = self.conn();
        tokio::task::spawn_blocking(move || -> anyhow::Result<CheckoutConfigSnapshot> {
            let conn = conn.lock();
            read_snapshot(&conn, host)
        })
        .await
        .context("checkout config read task panicked")?
    }

    /// Resolve the effective checkout configuration for one scope — the
    /// global value with any host override applied per field, an
    /// empty-string post-clone override disabling the hook, and the global
    /// revision. See the module docs for the full inheritance contract.
    pub async fn resolve_checkout_config(
        &self,
        host: Option<HostId>,
    ) -> anyhow::Result<ResolvedCheckoutConfig> {
        Ok(resolve_snapshot(
            &self.checkout_config_snapshot(host).await?,
        ))
    }

    /// Set the checkout root for one scope. The path is validated here
    /// (absolute, `~`, or `~/...`; stored UNEXPANDED — only the target
    /// supervisor expands it) and the global revision increments only if
    /// the stored value actually changes. Never contacts a host, creates a
    /// directory, or runs anything.
    pub async fn set_checkout_root(&self, host: Option<HostId>, root: &str) -> anyhow::Result<()> {
        validate_root_path(root)?;
        if root.len() > MAX_ROOT_BYTES {
            anyhow::bail!(
                "checkout root is {} bytes; the limit is {MAX_ROOT_BYTES} bytes",
                root.len()
            );
        }
        self.write_setting(host, Field::Root, Some(root)).await
    }

    /// Set the post-clone shell command for one scope. Stored verbatim; an
    /// empty string is a legitimate explicit disable on a host whose
    /// global hook it must override. The command is configuration the
    /// operator typed on their own machine, so a cap refusal names the
    /// limit but never quotes the body back.
    pub async fn set_checkout_post_clone(
        &self,
        host: Option<HostId>,
        command: &str,
    ) -> anyhow::Result<()> {
        if command.len() > MAX_POST_CLONE_BYTES {
            anyhow::bail!(
                "post-clone command is {} bytes; the limit is {MAX_POST_CLONE_BYTES} bytes",
                command.len()
            );
        }
        self.write_setting(host, Field::PostClone, Some(command))
            .await
    }

    /// Clear the checkout root for one scope: on a host, remove the
    /// override so the field inherits again; globally, unset it.
    pub async fn clear_checkout_root(&self, host: Option<HostId>) -> anyhow::Result<()> {
        self.write_setting(host, Field::Root, None).await
    }

    /// Clear the post-clone setting for one scope, with the same
    /// clear-to-inherit / global-unset split as [`Self::clear_checkout_root`].
    pub async fn clear_checkout_post_clone(&self, host: Option<HostId>) -> anyhow::Result<()> {
        self.write_setting(host, Field::PostClone, None).await
    }

    /// The one write path for every setter: read the current stored value
    /// and the revision inside one IMMEDIATE transaction, skip entirely
    /// (no revision bump) when the value already equals the target, and
    /// otherwise write the value and bump the revision in that same
    /// transaction. A named host must exist in the registry — a typo'd id
    /// is an error, never a silent write against nothing.
    async fn write_setting(
        &self,
        host: Option<HostId>,
        field: Field,
        value: Option<&str>,
    ) -> anyhow::Result<()> {
        let conn = self.conn();
        let value = value.map(str::to_string);
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let mut conn = conn.lock();
            let tx = conn
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .context("beginning checkout config transaction")?;
            if let Some(host) = host {
                let known: Option<i64> = tx
                    .query_row(
                        "SELECT 1 FROM hosts WHERE id = ?1",
                        rusqlite::params![host],
                        |row| row.get(0),
                    )
                    .optional()
                    .context("checking the host exists")?;
                if known.is_none() {
                    anyhow::bail!("no host with id {host} is registered in this helm");
                }
            }
            let current = read_field(&tx, host, field)?;
            if current == value {
                tx.commit()
                    .context("committing no-op checkout config write")?;
                return Ok(());
            }
            write_field(&tx, host, field, value.as_deref())?;
            tx.execute(
                "UPDATE checkout_config SET revision = revision + 1 WHERE singleton = 1",
                [],
            )
            .context("bumping the checkout config revision")?;
            tx.commit().context("committing checkout config write")?;
            Ok(())
        })
        .await
        .context("checkout config write task panicked")?
    }
}

/// Read one stored field for one scope, inside the caller's transaction.
fn read_field(
    conn: &rusqlite::Connection,
    host: Option<HostId>,
    field: Field,
) -> anyhow::Result<Option<String>> {
    let column = field.column();
    Ok(match host {
        Some(host) => conn
            .query_row(
                &format!("SELECT {column} FROM checkout_config_host WHERE host_id = ?1"),
                rusqlite::params![host],
                |row| row.get(0),
            )
            .optional()
            .with_context(|| format!("reading the host checkout {column} override"))?
            .flatten(),
        None => conn
            .query_row(
                &format!("SELECT {column} FROM checkout_config WHERE singleton = 1"),
                [],
                |row| row.get(0),
            )
            .optional()
            .with_context(|| format!("reading the global checkout {column}"))?
            .flatten(),
    })
}

/// Write one stored field for one scope, inside the caller's transaction:
/// hosts get an upserted override row (deleted outright once both fields
/// are NULL, so "no row" and "all-inherit" stay the same state on disk),
/// and the global singleton gets an UPDATE that relies on the schema's own
/// minted row.
fn write_field(
    conn: &rusqlite::Transaction<'_>,
    host: Option<HostId>,
    field: Field,
    value: Option<&str>,
) -> anyhow::Result<()> {
    let column = field.column();
    match host {
        Some(host) => {
            let updated = conn
                .execute(
                    &format!("UPDATE checkout_config_host SET {column} = ?2 WHERE host_id = ?1"),
                    rusqlite::params![host, value],
                )
                .context("updating the host checkout override")?;
            if updated == 0 {
                conn.execute(
                    &format!(
                        "INSERT INTO checkout_config_host (host_id, {column}) VALUES (?1, ?2)"
                    ),
                    rusqlite::params![host, value],
                )
                .context("inserting the host checkout override")?;
            }
            // Canonical storage: a row with nothing left to override is the
            // same as no row, so remove it rather than accumulate husks.
            conn.execute(
                "DELETE FROM checkout_config_host
                 WHERE host_id = ?1 AND root IS NULL AND post_clone IS NULL",
                rusqlite::params![host],
            )
            .context("pruning an all-NULL checkout override row")?;
        }
        None => {
            conn.execute(
                &format!("UPDATE checkout_config SET {column} = ?1 WHERE singleton = 1"),
                rusqlite::params![value],
            )
            .context("updating the global checkout setting")?;
        }
    }
    Ok(())
}

/// Read the whole stored state for one scope in one snapshot.
///
/// The two SELECTs run inside ONE explicit transaction: without it, each
/// autocommit read sees its own view, so a concurrent CLI (or another
/// `HelmStore` on a second connection) could commit a settings change
/// between them — the reader would then pair a new override with an old
/// revision, or stitch global and override values that never coexisted,
/// and present the mixture as `config_revision`. A deferred transaction is
/// enough: it pins the snapshot SQLite actually reads from, and it upgrades
/// to a write lock only if someone writes mid-read. (`unchecked_transaction`
/// is the immutable-reference form; its default DEFERRED behavior is the
/// pinning one.)
fn read_snapshot(
    conn: &rusqlite::Connection,
    host: Option<HostId>,
) -> anyhow::Result<CheckoutConfigSnapshot> {
    let _tx = conn
        .unchecked_transaction()
        .context("opening the checkout config snapshot transaction")?;
    let (global_root, global_post_clone, revision): (Option<String>, Option<String>, i64) = conn
        .query_row(
            "SELECT root, post_clone, revision FROM checkout_config WHERE singleton = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .context("reading the global checkout config")?
        .unwrap_or((None, None, 0));
    let host_override = match host {
        Some(host) => conn
            .query_row(
                "SELECT root, post_clone FROM checkout_config_host WHERE host_id = ?1",
                rusqlite::params![host],
                |row| {
                    Ok(CheckoutHostOverride {
                        root: row.get(0)?,
                        post_clone: row.get(1)?,
                    })
                },
            )
            .optional()
            .context("reading the host checkout config override")?,
        None => None,
    };
    Ok(CheckoutConfigSnapshot {
        global_root,
        global_post_clone,
        revision,
        host_override,
    })
}

/// Apply the inheritance contract to a snapshot: per-field override wins,
/// NULL inherits, and an empty post-clone string disables the hook only
/// AFTER the override-vs-global choice is made.
fn resolve_snapshot(snapshot: &CheckoutConfigSnapshot) -> ResolvedCheckoutConfig {
    let raw_post_clone = snapshot
        .host_override
        .as_ref()
        .and_then(|o| o.post_clone.clone())
        .or_else(|| snapshot.global_post_clone.clone());
    ResolvedCheckoutConfig {
        root: snapshot
            .host_override
            .as_ref()
            .and_then(|o| o.root.clone())
            .or_else(|| snapshot.global_root.clone()),
        // The empty string is stored verbatim and only HERE becomes "no
        // hook" — the whole reason it is distinguishable from NULL.
        post_clone: match raw_post_clone {
            Some(value) if value.is_empty() => None,
            other => other,
        },
        config_revision: snapshot.revision,
    }
}

/// Accept exactly the root shapes the design allows: absolute paths, `~`,
/// and `~/...`. Stored UNEXPANDED — expansion belongs to the target
/// supervisor, and storing the expanded form of one machine's home would
/// be wrong on every other host. Relative paths and `~user` forms are
/// refused with a message that says what IS accepted.
fn validate_root_path(path: &str) -> anyhow::Result<()> {
    if path.is_empty() {
        anyhow::bail!("checkout root must not be empty; use clear-root to unset it");
    }
    if path == "~" || path.starts_with("~/") {
        return Ok(());
    }
    if path.starts_with('~') {
        anyhow::bail!(
            "checkout root does not support ~user paths; use an absolute path, ~, or ~/..."
        );
    }
    if Path::new(path).is_absolute() {
        return Ok(());
    }
    anyhow::bail!(
        "checkout root must be an absolute path (or ~, ~/...), not the relative path '{path}'"
    )
}

/// What one `farhelm helm checkout-config` invocation should do.
#[derive(Debug, Clone)]
pub enum CheckoutConfigAction {
    Show,
    SetRoot { path: String },
    ClearRoot,
    SetPostClone { command: String },
    ClearPostClone,
}

/// The `farhelm helm checkout-config` implementation: resolve the state
/// dir from the explicit flag or the platform default (the PURE resolution
/// — unlike token control's, this path never creates the directory),
/// open the existing current-schema helm.db without creating or migrating
/// it (storage rule R1.5), and apply the action. Returns the text to print.
pub async fn checkout_config_cli(
    state_dir: Option<PathBuf>,
    host: Option<HostId>,
    action: CheckoutConfigAction,
) -> anyhow::Result<String> {
    let state_dir = match state_dir {
        Some(dir) => dir,
        None => farhelm_supervisor::default_state_dir()?,
    };
    let db_path = state_dir.join("helm.db");
    let read_only = matches!(action, CheckoutConfigAction::Show);
    let store = if read_only {
        HelmStore::open_existing_current_schema_read_only(&db_path).await?
    } else {
        HelmStore::open_existing_current_schema(&db_path).await?
    };
    // An explicit host id must name a registered host: validate against
    // the registry so a typo cannot be mistaken for a global write or
    // silently resolve as "no override".
    if let Some(host) = host {
        let rows: Vec<HostRow> = store.list_hosts().await?;
        if !rows.iter().any(|row| row.id == host) {
            let ids = rows
                .iter()
                .map(|row| row.id.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            anyhow::bail!(
                "no host with id {host} is registered in this helm (registered host ids: {})",
                if ids.is_empty() { "none" } else { &ids }
            );
        }
    }
    let scope = match host {
        Some(host) => format!("host {host}"),
        None => "global".to_string(),
    };
    match action {
        CheckoutConfigAction::Show => Ok(render_show(
            &store.checkout_config_snapshot(host).await?,
            host,
        )),
        CheckoutConfigAction::SetRoot { path } => {
            validate_root_path(&path)?;
            store.set_checkout_root(host, &path).await?;
            after_write(&store, host, &scope, &format!("set checkout root: {path}")).await
        }
        CheckoutConfigAction::ClearRoot => {
            store.clear_checkout_root(host).await?;
            after_write(&store, host, &scope, "cleared checkout root").await
        }
        CheckoutConfigAction::SetPostClone { command } => {
            store.set_checkout_post_clone(host, &command).await?;
            let described = if command.is_empty() {
                "set post-clone override to empty (disables the hook for this host)".to_string()
            } else {
                format!("set post-clone command: {command}")
            };
            after_write(&store, host, &scope, &described).await
        }
        CheckoutConfigAction::ClearPostClone => {
            store.clear_checkout_post_clone(host).await?;
            after_write(&store, host, &scope, "cleared post-clone").await
        }
    }
}

/// The confirmation line for a completed write, with the post-write
/// revision so an operator (and a test) can see the change landed.
async fn after_write(
    store: &HelmStore,
    host: Option<HostId>,
    scope: &str,
    what: &str,
) -> anyhow::Result<String> {
    let revision = store.checkout_config_snapshot(host).await?.revision;
    Ok(format!("{scope}: {what}\nrevision is now {revision}\n"))
}

/// Render `show`: the stored values plus the effective inheritance for the
/// selected scope, and the revision. The post-clone command is printed
/// verbatim — it is the operator's own config, read back from their own
/// database by their own CLI.
fn render_show(snapshot: &CheckoutConfigSnapshot, host: Option<HostId>) -> String {
    let mut out = String::new();
    match host {
        Some(id) => {
            out.push_str(&format!("scope: host {id}\n"));
            out.push_str(&format!(
                "root: {}\n",
                root_line(&snapshot.host_override, snapshot)
            ));
            out.push_str(&format!(
                "post-clone: {}\n",
                post_clone_line(&snapshot.host_override, snapshot)
            ));
        }
        None => {
            out.push_str("scope: global\n");
            out.push_str(&format!("root: {}\n", global_line(&snapshot.global_root)));
            out.push_str(&format!(
                "post-clone: {}\n",
                global_line_disabled_aware(&snapshot.global_post_clone)
            ));
        }
    }
    out.push_str(&format!("revision: {}\n", snapshot.revision));
    out
}

/// The root line for a host scope: override, inherited value, or unset.
fn root_line(
    host_override: &Option<CheckoutHostOverride>,
    snapshot: &CheckoutConfigSnapshot,
) -> String {
    match host_override.as_ref().and_then(|o| o.root.clone()) {
        Some(root) => format!("{root} (host override)"),
        None => match &snapshot.global_root {
            Some(root) => format!("{root} (inherited from global)"),
            None => "unset".to_string(),
        },
    }
}

/// The post-clone line for a host scope, where the empty-string override's
/// "disable" meaning must be visible rather than silently normalized away.
fn post_clone_line(
    host_override: &Option<CheckoutHostOverride>,
    snapshot: &CheckoutConfigSnapshot,
) -> String {
    match host_override.as_ref().map(|o| o.post_clone.clone()) {
        Some(Some(command)) if command.is_empty() => match &snapshot.global_post_clone {
            Some(global) if !global.is_empty() => {
                format!("disabled by host override (global would run: {global})")
            }
            _ => "disabled by host override".to_string(),
        },
        Some(Some(command)) => format!("{command} (host override)"),
        // No override: whatever the global row says is what this host gets.
        _ => match &snapshot.global_post_clone {
            Some(global) if global.is_empty() => "disabled (global setting)".to_string(),
            Some(global) => format!("{global} (inherited from global)"),
            None => "unset".to_string(),
        },
    }
}

/// The global row's own value: an empty stored post-clone reads as an
/// explicit "disabled".
fn global_line(value: &Option<String>) -> String {
    match value {
        Some(value) => value.clone(),
        None => "unset".to_string(),
    }
}

fn global_line_disabled_aware(value: &Option<String>) -> String {
    match value {
        Some(value) if value.is_empty() => "disabled".to_string(),
        other => global_line(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::HelmStore;
    use farhelm_proto::SessionInfo;
    use std::os::unix::fs::PermissionsExt;

    /// A private, freshly initialized helm.db per test — never the user's
    /// real state directory.
    async fn fresh_store() -> (tempfile::TempDir, HelmStore) {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = HelmStore::open(&dir.path().join("helm.db"))
            .await
            .expect("create fixture helm.db");
        (dir, store)
    }

    /// CLI writes use a separate database connection. Only committed revision
    /// changes may invalidate browsers; failed reads must leave the observation
    /// pending so a later successful read can still publish it.
    #[tokio::test]
    async fn checkout_revision_feed_observes_external_changes_and_preserves_failed_reads() {
        let (dir, reader) = fresh_store().await;
        let writer = HelmStore::open(&dir.path().join("helm.db")).await.unwrap();
        let events = crate::feed::FleetEvents::new();
        let mut observed = reader
            .checkout_config_snapshot(None)
            .await
            .unwrap()
            .revision;
        assert_eq!(observed, 0);
        assert!(
            !publish_revision_change(&reader, &events, &mut observed)
                .await
                .unwrap()
        );
        assert_eq!(events.revision(), 0);

        writer.set_checkout_root(None, "/first-root").await.unwrap();
        assert_eq!(
            writer
                .checkout_config_snapshot(None)
                .await
                .unwrap()
                .revision,
            1
        );
        assert!(
            publish_revision_change(&reader, &events, &mut observed)
                .await
                .unwrap()
        );
        assert_eq!((observed, events.revision()), (1, 1));
        writer.set_checkout_root(None, "/first-root").await.unwrap();
        assert!(
            !publish_revision_change(&reader, &events, &mut observed)
                .await
                .unwrap()
        );
        assert_eq!(
            events.revision(),
            1,
            "no-op CLI writes must not wake every browser"
        );

        writer
            .set_checkout_root(None, "/second-root")
            .await
            .unwrap();
        // Make the reader fail after a real unseen commit. Restoring this
        // private fixture's table must not erase the pending notification.
        writer
            .conn()
            .lock()
            .execute_batch("ALTER TABLE checkout_config RENAME TO checkout_config_unavailable")
            .unwrap();
        assert!(
            publish_revision_change(&reader, &events, &mut observed)
                .await
                .is_err()
        );
        assert_eq!((observed, events.revision()), (1, 1));
        writer
            .conn()
            .lock()
            .execute_batch("ALTER TABLE checkout_config_unavailable RENAME TO checkout_config")
            .unwrap();
        assert!(
            publish_revision_change(&reader, &events, &mut observed)
                .await
                .unwrap()
        );
        assert_eq!((observed, events.revision()), (2, 2));
    }

    /// Register an ssh host and return its id — a stable registered HostId,
    /// exactly what the CLI's `--host` is defined against.
    async fn ssh_host(store: &HelmStore, destination: &str) -> HostId {
        store
            .add_ssh_host(destination, None, None)
            .await
            .expect("register ssh host")
    }

    /// Build a complete wire-shaped session so migration evidence covers the
    /// persisted cache JSON rather than only its ordering columns.
    fn migration_session(id: &str, created_at: i64) -> SessionInfo {
        SessionInfo {
            id: id.into(),
            parent: None,
            title: id.into(),
            created_at,
            last_activity_at: created_at + 1,
            last_work_started_at: 0,
            creation_seq: None,
            cwd: format!("/srv/{id}"),
            canonical_cwd: None,
            invocation: "codex --resume retained".into(),
            resume_template: None,
            launch: None,
            status: farhelm_proto::SessionStatus::Waiting,
            annotation: None,
            restart_offer: farhelm_proto::RestartOffer::default(),
            tabs: Vec::new(),
            source_profile: None,
            github_repo: None,
            working_copy: None,
        }
    }

    // ---- Inheritance and revision semantics --------------------------

    /// Spec: global set/clear round-trips for both fields, and clearing an
    /// already-unset field is a quiet no-op rather than an error.
    #[tokio::test]
    async fn checkout_global_set_clear_round_trips() {
        let (_dir, store) = fresh_store().await;
        assert_eq!(
            store.resolve_checkout_config(None).await.unwrap(),
            ResolvedCheckoutConfig {
                root: None,
                post_clone: None,
                config_revision: 0
            },
            "a fresh helm starts fully unset at revision 0"
        );

        store.set_checkout_root(None, "~/work").await.unwrap();
        store
            .set_checkout_post_clone(None, "make sync")
            .await
            .unwrap();
        let resolved = store.resolve_checkout_config(None).await.unwrap();
        assert_eq!(resolved.root.as_deref(), Some("~/work"));
        assert_eq!(resolved.post_clone.as_deref(), Some("make sync"));
        assert_eq!(resolved.config_revision, 2, "one bump per actual change");

        store.clear_checkout_root(None).await.unwrap();
        store.clear_checkout_post_clone(None).await.unwrap();
        let resolved = store.resolve_checkout_config(None).await.unwrap();
        assert_eq!(resolved.root, None);
        assert_eq!(resolved.post_clone, None);
        assert_eq!(resolved.config_revision, 4, "clearing a set value counts");
    }

    /// Spec: precedence is PER FIELD — a host override wins only the field
    /// it names, NULL inherits, and a second host with no override row
    /// inherits everything.
    #[tokio::test]
    async fn checkout_host_overrides_win_per_field_while_null_inherits() {
        let (_dir, store) = fresh_store().await;
        store.set_checkout_root(None, "/global/root").await.unwrap();
        store
            .set_checkout_post_clone(None, "global-hook")
            .await
            .unwrap();

        let host = ssh_host(&store, "override.example").await;
        store
            .set_checkout_root(Some(host), "/host/root")
            .await
            .unwrap();
        let resolved = store.resolve_checkout_config(Some(host)).await.unwrap();
        assert_eq!(resolved.root.as_deref(), Some("/host/root"));
        assert_eq!(
            resolved.post_clone.as_deref(),
            Some("global-hook"),
            "the unoverridden field inherits the global value"
        );
        assert_eq!(resolved.config_revision, 3);

        let plain = ssh_host(&store, "plain.example").await;
        let resolved = store.resolve_checkout_config(Some(plain)).await.unwrap();
        assert_eq!(resolved.root.as_deref(), Some("/global/root"));
        assert_eq!(resolved.post_clone.as_deref(), Some("global-hook"));
    }

    /// Spec: the empty-string post-clone override DISABLES a global hook —
    /// stored verbatim, normalized to None only after inheritance — while
    /// a host without the override still runs the hook, and clearing the
    /// override restores it.
    #[tokio::test]
    async fn checkout_empty_post_clone_override_disables_a_global_hook() {
        let (_dir, store) = fresh_store().await;
        store
            .set_checkout_post_clone(None, "global-hook")
            .await
            .unwrap();
        let host = ssh_host(&store, "disable.example").await;

        store.set_checkout_post_clone(Some(host), "").await.unwrap();
        let resolved = store.resolve_checkout_config(Some(host)).await.unwrap();
        assert_eq!(resolved.post_clone, None, "an empty override disables");
        assert_eq!(
            store
                .resolve_checkout_config(None)
                .await
                .unwrap()
                .post_clone,
            Some("global-hook".to_string()),
            "the global hook itself is untouched"
        );

        store.clear_checkout_post_clone(Some(host)).await.unwrap();
        assert_eq!(
            store
                .resolve_checkout_config(Some(host))
                .await
                .unwrap()
                .post_clone,
            Some("global-hook".to_string()),
            "clearing the override returns to inheritance"
        );
    }

    /// Spec: clearing a host root override returns that field to
    /// inheritance, and the override row is pruned once nothing in it is
    /// left to override.
    #[tokio::test]
    async fn checkout_clearing_host_overrides_returns_to_inheritance() {
        let (_dir, store) = fresh_store().await;
        store.set_checkout_root(None, "/global/root").await.unwrap();
        let host = ssh_host(&store, "clear.example").await;
        store
            .set_checkout_root(Some(host), "/host/root")
            .await
            .unwrap();
        assert_eq!(
            store
                .resolve_checkout_config(Some(host))
                .await
                .unwrap()
                .root,
            Some("/host/root".to_string())
        );

        store.clear_checkout_root(Some(host)).await.unwrap();
        let snapshot = store.checkout_config_snapshot(Some(host)).await.unwrap();
        assert_eq!(
            snapshot.host_override, None,
            "an all-NULL override row must be pruned, not left behind"
        );
        assert_eq!(
            store
                .resolve_checkout_config(Some(host))
                .await
                .unwrap()
                .root,
            Some("/global/root".to_string())
        );
    }

    /// Spec: the revision increments on REAL changes only, and the bump is
    /// durable — visible through a SECOND independently opened connection,
    /// not just the writer's own handle.
    #[tokio::test]
    async fn checkout_revision_bumps_only_on_real_changes_seen_from_a_second_connection() {
        let (dir, store) = fresh_store().await;
        let path = dir.path().join("helm.db");
        let reopen = || HelmStore::open(&path);
        let observer = reopen().await.expect("second connection");

        let revision = || async {
            observer
                .checkout_config_snapshot(None)
                .await
                .unwrap()
                .revision
        };

        assert_eq!(revision().await, 0);
        store.set_checkout_root(None, "/one").await.unwrap();
        assert_eq!(revision().await, 1, "an actual change bumps");
        store.set_checkout_root(None, "/one").await.unwrap();
        assert_eq!(revision().await, 1, "writing the SAME value must NOT bump");

        store.set_checkout_root(None, "/two").await.unwrap();
        assert_eq!(revision().await, 2);
        store.clear_checkout_root(None).await.unwrap();
        assert_eq!(revision().await, 3);
        store.clear_checkout_root(None).await.unwrap();
        assert_eq!(
            revision().await,
            3,
            "clearing an already-cleared value must NOT bump"
        );
        assert_eq!(
            store.resolve_checkout_config(None).await.unwrap().root,
            None,
            "the no-op writes left the value where the real writes put it"
        );
    }

    /// Spec: per-host choices are config for the registry ROW — removing
    /// the host cascades the override away (ON DELETE CASCADE), while the
    /// global settings and other hosts are untouched.
    #[tokio::test]
    async fn checkout_host_removal_cascades_the_override_away() {
        let (_dir, store) = fresh_store().await;
        store.set_checkout_root(None, "/global/root").await.unwrap();
        let host = ssh_host(&store, "cascade.example").await;
        store
            .set_checkout_post_clone(Some(host), "host-hook")
            .await
            .unwrap();

        store.remove_ssh_host(host).await.expect("remove the host");
        let snapshot = store.checkout_config_snapshot(Some(host)).await.unwrap();
        assert_eq!(
            snapshot.host_override, None,
            "the override row must be gone with its host"
        );
        assert_eq!(snapshot.revision, 2, "the cascade is not a settings write");
        assert_eq!(
            store.resolve_checkout_config(None).await.unwrap().root,
            Some("/global/root".to_string()),
            "global settings survive a host removal"
        );
    }

    /// Spec: root validation at set time — absolute, `~`, and `~/...`
    /// accepted and stored unexpanded; relative paths and `~user` forms
    /// refused with a message naming what is accepted. Rejected writes
    /// must not touch the stored value or the revision.
    #[tokio::test]
    async fn checkout_root_validation_rejects_relative_and_tilde_user_paths() {
        let (_dir, store) = fresh_store().await;
        for accepted in ["/abs/path", "~/work", "~"] {
            store
                .set_checkout_root(None, accepted)
                .await
                .unwrap_or_else(|_| panic!("an accepted root shape must store: {accepted}"));
            assert_eq!(
                store.resolve_checkout_config(None).await.unwrap().root,
                Some(accepted.to_string()),
                "accepted shapes are stored UNEXPANDED"
            );
        }
        for rejected in ["relative/path", "./here", "~alice/repos", ""] {
            let error = store
                .set_checkout_root(None, rejected)
                .await
                .expect_err("a rejected root shape must be refused");
            let message = format!("{error:#}");
            assert!(
                message.contains("absolute path") || message.contains("clear-root"),
                "the refusal must say what is accepted: {message}"
            );
        }
        assert_eq!(
            store.resolve_checkout_config(None).await.unwrap().root,
            Some("~".to_string()),
            "the last accepted value survives every rejected write"
        );
        assert_eq!(
            store.checkout_config_snapshot(None).await.unwrap().revision,
            3,
            "only the three accepted writes bumped the revision; every refusal was a no-op"
        );
    }

    /// Spec (Design A caps): a configured root is at most 4096 bytes and a
    /// post-clone hook at most 16 KiB, enforced at set time for global and
    /// host settings alike, before any database write. The exact boundary
    /// is accepted; one byte over is refused naming the limit. The hook
    /// refusal must not echo the body back.
    #[tokio::test]
    async fn checkout_settings_enforce_byte_caps_at_set_time() {
        let (_dir, store) = fresh_store().await;

        // Exact-limit acceptance: "/" + 4095 ASCII bytes = 4096 bytes total.
        let root_ok = format!("/{}", "a".repeat(4095));
        store.set_checkout_root(None, &root_ok).await.unwrap();
        // One byte over.
        let root_over = format!("{root_ok}x");
        assert_eq!(root_over.len(), MAX_ROOT_BYTES + 1);
        let error = store
            .set_checkout_root(None, &root_over)
            .await
            .expect_err("an over-limit root is refused");
        assert!(format!("{error:#}").contains("limit is 4096"));

        // Exact-limit hook accepted; one byte over refused WITHOUT the
        // body in the error.
        let hook_ok = "x".repeat(MAX_POST_CLONE_BYTES);
        store.set_checkout_post_clone(None, &hook_ok).await.unwrap();
        let hook_over = format!("{hook_ok}y");
        let error = store
            .set_checkout_post_clone(None, &hook_over)
            .await
            .expect_err("an over-limit hook is refused");
        let message = format!("{error:#}");
        assert!(message.contains("limit is 16384"), "{message}");
        assert!(
            !message.contains(&hook_over[..64]),
            "the hook body must not be quoted: {message}"
        );

        // Host overrides enforce the same caps.
        let host = ssh_host(&store, "cap.example").await;
        let error = store
            .set_checkout_root(Some(host), &root_over)
            .await
            .expect_err("host overrides enforce the root cap too");
        assert!(format!("{error:#}").contains("limit is 4096"));

        // The host HOOK cap is enforced too, at the same boundary.
        let hook_exact = "y".repeat(MAX_POST_CLONE_BYTES);
        store
            .set_checkout_post_clone(Some(host), &hook_exact)
            .await
            .unwrap();
        let error = store
            .set_checkout_post_clone(Some(host), &hook_over)
            .await
            .expect_err("a host hook over the limit is refused");
        assert!(
            format!("{error:#}").contains("limit is 16384"),
            "the refusal names the limit: {error:#}"
        );

        // BYTE counting, not character counting: "/" + 2047 two-byte chars
        // + 1 ASCII byte is exactly 4096 bytes and is ACCEPTED, and adding
        // one more char (4098 bytes, 2050 chars — under 4096 by character
        // count) is REFUSED — a character-count implementation would pass
        // this root.
        let multibyte_exact = format!("/{}a", "é".repeat(2047));
        assert_eq!(multibyte_exact.len(), MAX_ROOT_BYTES);
        assert_eq!(multibyte_exact.chars().count(), 2049);
        store
            .set_checkout_root(None, &multibyte_exact)
            .await
            .unwrap();
        let multibyte_over = format!("{multibyte_exact}é");
        assert_eq!(multibyte_over.len(), MAX_ROOT_BYTES + 2);
        assert!(multibyte_over.chars().count() < 4096);
        let error = store
            .set_checkout_root(None, &multibyte_over)
            .await
            .expect_err("a root over the byte limit is refused even under 4096 chars");
        assert!(format!("{error:#}").contains("limit is 4096"));

        // Refused writes change nothing: the stored global values and the
        // host override are still the last ACCEPTED values.
        let snapshot = store.checkout_config_snapshot(None).await.unwrap();
        assert_eq!(
            snapshot.global_root.as_deref(),
            Some(multibyte_exact.as_str())
        );
        let host_snapshot = store.checkout_config_snapshot(Some(host)).await.unwrap();
        assert_eq!(
            host_snapshot
                .host_override
                .as_ref()
                .and_then(|o| o.post_clone.clone())
                .as_deref(),
            Some(hook_exact.as_str()),
            "the host hook keeps its accepted value after the refused write"
        );

        // Refusals write nothing: revision still reflects the accepted
        // writes only (global root, global hook, host hook, multibyte
        // root — the host root attempt was refused as over-limit).
        assert_eq!(
            snapshot.revision, 4,
            "cap refusals are no-ops like other rejected writes"
        );
    }

    /// Spec (single-snapshot resolution): the global values, the override,
    /// and the revision come from ONE consistent view — a writer that
    /// commits between the reader's internal SELECTs must not be
    /// observable as a mixture.
    ///
    /// The observable must DISTINGUISH the transactional read from two
    /// bare autocommit SELECTs, so the test genuinely interleaves a racing
    /// commit between them: an authorizer on the reading connection fires
    /// at the PREPARE of the host-override SELECT — after the global
    /// SELECT has fully completed, before the override read runs — and
    /// commits a root change on a second connection at that instant.
    ///
    /// - With the snapshot transaction (`unchecked_transaction`): the
    ///   reader has held a shared read lock since the global SELECT, so
    ///   the racing commit blocks and fails (short busy timeout keeps the
    ///   test fast); the reader returns the OLD override with the OLD
    ///   revision.
    /// - Without the transaction (the original defect): the commit lands
    ///   in the gap and the reader returns the NEW override tagged with
    ///   the OLD revision — the mixture this test fails on.
    #[tokio::test]
    async fn checkout_config_snapshot_reads_one_consistent_view() {
        use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};

        let (dir, store) = fresh_store().await;
        let path = dir.path().join("helm.db");
        store.set_checkout_root(None, "/one").await.unwrap();
        let host = ssh_host(&store, "snapshot.example").await;
        store
            .set_checkout_root(Some(host), "/host-one")
            .await
            .unwrap();
        let revision_before = store.checkout_config_snapshot(None).await.unwrap().revision;

        // The racing writer: a second, independently opened connection.
        let writer = HelmStore::open(&path).await.unwrap();
        let writer_conn = writer.conn();
        // A short busy timeout so a commit blocked by the reader's lock
        // fails fast instead of stalling the test. Its setup is asserted:
        // the oracle below depends on the blocked COMMIT failing FAST.
        writer_conn
            .lock()
            .busy_timeout(std::time::Duration::from_millis(200))
            .expect("set the racing writer's busy timeout");
        // The oracle depends on readers BLOCKING commits, which holds in
        // SQLite's rollback-journal mode, not under WAL: a WAL reader
        // snapshots while the writer commits. Pin that premise.
        let journal_mode: String = writer_conn
            .lock()
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .expect("read journal mode");
        assert_eq!(
            journal_mode.to_ascii_lowercase(),
            "delete",
            "fixture premise: rollback-journal mode, where a read lock blocks commits"
        );

        // Shared evidence, written inside the closure and asserted after
        // the read: whether the interposer ran, and exactly what the
        // racing write did (both UPDATEs, then the COMMIT's outcome).
        let interposed = Arc::new(AtomicBool::new(false));
        let commit_attempted = Arc::new(AtomicBool::new(false));
        let commit_landed = Arc::new(AtomicBool::new(false));
        let commit_error = Arc::new(std::sync::Mutex::new(None::<rusqlite::ErrorCode>));

        // The reading connection, with the interposer registered.
        let reader = HelmStore::open(&path).await.unwrap();
        {
            let conn = reader.conn();
            let conn = conn.lock();
            let interposed = interposed.clone();
            let commit_attempted = commit_attempted.clone();
            let commit_landed = commit_landed.clone();
            let commit_error = commit_error.clone();
            let registered = Arc::new(AtomicBool::new(false));
            conn.authorizer(Some(move |ctx: AuthContext<'_>| {
                // Fire exactly once, at the first prepare-time read of the
                // override table: the global SELECT is complete by then.
                if matches!(
                    ctx.action,
                    AuthAction::Read { table_name: "checkout_config_host", .. }
                ) && !registered.swap(true, Ordering::SeqCst)
                {
                    interposed.store(true, Ordering::SeqCst);
                    // Per-stage outcome: `commit_attempted` is set ONLY
                    // after both UPDATEs succeeded, so a later DatabaseBusy
                    // is attributable to the COMMIT itself.
                    (|| {
                        let mut conn = writer_conn.lock();
                        let tx = match conn.transaction() {
                            Ok(tx) => tx,
                            Err(error) => {
                                *commit_error.lock().unwrap() = error.sqlite_error_code();
                                return;
                            }
                        };
                        if let Err(error) = tx.execute(
                            "UPDATE checkout_config_host SET root = '/host-two' WHERE host_id = ?1",
                            rusqlite::params![host],
                        ) {
                            *commit_error.lock().unwrap() = error.sqlite_error_code();
                            return;
                        }
                        if let Err(error) = tx.execute(
                            "UPDATE checkout_config SET revision = revision + 1 WHERE singleton = 1",
                            [],
                        ) {
                            *commit_error.lock().unwrap() = error.sqlite_error_code();
                            return;
                        }
                        commit_attempted.store(true, Ordering::SeqCst);
                        match tx.commit() {
                            Ok(()) => commit_landed.store(true, Ordering::SeqCst),
                            Err(error) => {
                                *commit_error.lock().unwrap() = error.sqlite_error_code();
                            }
                        }
                    })();
                }
                Authorization::Allow
            }))
            .expect("register authorizer");
        }

        let snapshot = reader.checkout_config_snapshot(Some(host)).await.unwrap();

        // The interposer MUST have run (otherwise the oracle proves
        // nothing), both UPDATEs inside it must have SUCCEEDED (else the
        // commit was never attempted), and the COMMIT itself must have
        // failed specifically with SQLite's busy code — the reader's
        // snapshot transaction blocking it.
        assert!(
            interposed.load(Ordering::SeqCst),
            "the authorizer interposition must have fired at the override SELECT"
        );
        assert!(
            commit_attempted.load(Ordering::SeqCst),
            "both UPDATEs must have succeeded and COMMIT been attempted, or the \
             DatabaseBusy assertion below is attributable to the wrong stage"
        );
        let commit_error = *commit_error.lock().unwrap();
        let commit_error = commit_error
            .expect("the racing write must reach COMMIT and be refused, not fail earlier");
        assert_eq!(
            commit_error,
            rusqlite::ErrorCode::DatabaseBusy,
            "the racing commit must fail with SQLITE_BUSY (readers block commits in \
             rollback-journal mode), not another error ({commit_error:?})"
        );
        // The racing commit was BLOCKED by the reader's snapshot
        // transaction, so the reader returns the pre-write override and
        // the pre-write revision — a state that actually coexisted.
        assert!(
            !commit_landed.load(Ordering::SeqCst),
            "the racing commit must be blocked while the snapshot transaction is open"
        );
        assert_eq!(
            snapshot.host_override.as_ref().and_then(|o| o.root.clone()),
            Some("/host-one".to_string()),
            "the override must come from the same snapshot as the revision"
        );
        assert_eq!(snapshot.revision, revision_before);

        // The racing write is not silently lost: with the reader's
        // transaction finished (lock released) the SAME write now lands,
        // and the next read sees the new override with the incremented
        // revision. (The authorizer remains registered; its one-shot flag
        // keeps it from re-interposing.)
        writer
            .set_checkout_root(Some(host), "/host-two")
            .await
            .expect("the racing write succeeds once the snapshot is closed");
        let after = reader.checkout_config_snapshot(Some(host)).await.unwrap();
        assert_eq!(
            after.host_override.as_ref().and_then(|o| o.root.clone()),
            Some("/host-two".to_string()),
            "the retried write lands once the snapshot transaction is done"
        );
        assert_eq!(after.revision, revision_before + 1);
    }

    // ---- R1.5: the never-create, never-migrate opening mode ----------

    /// Rewind a current-schema database to the exact schema-26 shape.
    /// Later checkout configuration tables and `remembered_workspace_trust`
    /// did not exist in v26; remove them, restore the retired `archived`
    /// cache column, and remove repository-history provenance before stamping
    /// the old version. Tests then exercise the real migrations instead of a
    /// current schema carrying an old label.
    async fn rewind_to_v26(path: &Path) {
        let store = HelmStore::open(path).await.expect("open to rewind");
        let conn = store.conn();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock();
            conn.execute_batch(
                "DROP TABLE checkout_config_host;
                 DROP TABLE checkout_config;
                 ALTER TABLE create_history_sessions DROP COLUMN github_repo;
                 ALTER TABLE session_cache ADD COLUMN archived INTEGER NOT NULL DEFAULT 0;
                 ALTER TABLE preferences DROP COLUMN remembered_workspace_trust;
                 PRAGMA user_version = 26;",
            )
            .unwrap();
            let version: i64 = conn
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .unwrap();
            assert_eq!(version, 26, "fixture premise: the file is at v26");
            let historical = rusqlite::Connection::open_in_memory().unwrap();
            historical
                .execute_batch(include_str!("../tests/fixtures/helm-v26.sql"))
                .unwrap();
            assert_eq!(
                historical_schema_objects(&conn),
                historical_schema_objects(&historical),
                "the populated downgrade fixture must match the frozen historical DDL",
            );
        })
        .await
        .unwrap();
    }

    /// Compare schema constraints rather than SQLite's preserved formatting.
    /// These DDL literals contain no comment delimiters inside quoted values;
    /// stripping comments and whitespace avoids ALTER TABLE's formatting noise
    /// without hiding columns, indexes, defaults or CHECK constraints.
    fn historical_schema_objects(conn: &rusqlite::Connection) -> Vec<(String, String)> {
        let mut stmt = conn.prepare(
            "SELECT name, COALESCE(sql, '') FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' ORDER BY name",
        ).unwrap();
        stmt.query_map([], |row| {
            let sql: String = row.get(1)?;
            let normalized = sql
                .lines()
                .map(|line| line.split("--").next().unwrap_or_default())
                .flat_map(str::split_whitespace)
                .collect::<Vec<_>>()
                .join(" ")
                .replace(" ,", ",")
                .replace(" )", ")");
            Ok((row.get(0)?, normalized))
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
    }

    /// Spec (R1.5): an OLDER-schema database is refused WITHOUT migrating —
    /// the error tells the operator to start or update the helm, the file
    /// is left byte-for-byte at v26 (version stamp and missing tables
    /// intact), and only the helm's own `open` migrates it.
    #[tokio::test]
    async fn checkout_open_existing_refuses_older_schema_without_migrating() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("helm.db");
        {
            let store = HelmStore::open(&path).await.expect("create current schema");
            store
                .set_checkout_root(None, "/pre-existing")
                .await
                .unwrap();
        }
        rewind_to_v26(&path).await;

        let error = HelmStore::open_existing_current_schema(&path)
            .await
            .expect_err("an older schema must be refused");
        let message = format!("{error:#}");
        assert!(
            message.contains("26") && message.contains("start (or update) the helm"),
            "the refusal must name the version and the remedy: {message}"
        );
        {
            // The refusal must not have migrated behind the error: the file
            // is still stamped v26 with no config tables, and the pre-existing
            // global value the config tables would have lost is irrelevant —
            // the rewind already discarded it, which is exactly what a real
            // v26 database looks like.
            let conn = rusqlite::Connection::open(&path).expect("reopen raw");
            let version: i64 = conn
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .unwrap();
            assert_eq!(version, 26);
            let tables: i64 = conn
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE name LIKE 'checkout_config%'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(tables, 0, "no config table may appear before a migration");
        }

        let store = HelmStore::open(&path)
            .await
            .expect("the helm's own open migrates");
        assert_eq!(
            store.checkout_config_snapshot(None).await.unwrap().revision,
            0,
            "a migrated v26 database starts with fresh, unset config"
        );
    }

    /// Spec (R1.5): with no database present, the existing-only open fails
    /// and the file still does not exist afterwards — the CREATE flag is
    /// omitted at the sqlite level, so no code path can mint a helm.db (or
    /// a state directory) behind a config command's back.
    #[tokio::test]
    async fn checkout_open_existing_never_creates_the_database() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("helm.db");
        let error = HelmStore::open_existing_current_schema(&path)
            .await
            .expect_err("an absent database must be refused");
        assert!(
            format!("{error:#}").contains("start (or update) the helm"),
            "the refusal must point at the remedy: {error:#}"
        );
        assert!(!path.exists(), "no database may be created");
        let error = HelmStore::open_existing_current_schema_read_only(&path)
            .await
            .expect_err("the read-only variant refuses too");
        assert!(!path.exists(), "still nothing created");
        let _ = error;
    }

    /// Spec (R1.5): a CURRENT-schema database opens in both modes and the
    /// read-only connection serves the same resolved values — while being
    /// a connection that could not write even if a future caller tried.
    #[tokio::test]
    async fn checkout_open_existing_read_only_serves_resolution() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("helm.db");
        let host = {
            let store = HelmStore::open(&path).await.expect("create");
            store.set_checkout_root(None, "/root").await.unwrap();
            let host = ssh_host(&store, "readonly.example").await;
            store.set_checkout_post_clone(Some(host), "").await.unwrap();
            host
        };
        let store = HelmStore::open_existing_current_schema_read_only(&path)
            .await
            .expect("read-only open at the current schema");
        let resolved = store.resolve_checkout_config(Some(host)).await.unwrap();
        assert_eq!(resolved.root.as_deref(), Some("/root"));
        assert_eq!(resolved.post_clone, None, "the empty override disables");

        // A write attempt through the read-only connection must fail
        // rather than silently succeed.
        let error = store
            .set_checkout_root(None, "/should-fail")
            .await
            .expect_err("a read-only connection cannot write");
        assert!(
            format!("{error:#}")
                .to_lowercase()
                .contains("readonly database"),
            "the failure should be sqlite's own read-only refusal: {error:#}"
        );
    }

    /// Spec: the 26→27 migration creates the config tables while preserving
    /// populated historical cache and create-history content, including its
    /// ordering and bounded eviction state; the upgraded helm still starts
    /// checkout configuration unset at revision 0, like a fresh database.
    #[tokio::test]
    async fn checkout_migration_from_26_recreates_the_config_tables() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("helm.db");
        let store = HelmStore::open(&path).await.expect("create current schema");
        let host = ssh_host(&store, "migration.example").await;
        let identity = "migration-identity";
        {
            let conn = store.conn();
            let conn = conn.lock();
            conn.execute(
                "UPDATE hosts SET host_identity = ?2 WHERE id = ?1",
                rusqlite::params![host, identity],
            )
            .unwrap();
        }
        let cached = migration_session("cached-migration", 1_700_000_042);
        store
            .replace_host_sessions(host, identity, vec![cached.clone()], false)
            .await
            .unwrap();
        for sequence in 1..=100 {
            let mut entry =
                migration_session(&format!("history-{sequence}"), 1_700_000_000 + sequence);
            entry.creation_seq = Some(sequence as u64);
            entry.launch = Some(farhelm_proto::LaunchSelection {
                harness: farhelm_proto::LaunchHarness::Codex,
                model: Some(format!("model-{sequence}")),
                effort: None,
                permissions: None,
                workspace_trust: None,
            });
            store
                .record_create_history(host, identity, &entry)
                .await
                .unwrap();
        }
        let history_entry = SessionInfo {
            title: "history title".into(),
            creation_seq: Some(9_999),
            launch: Some(farhelm_proto::LaunchSelection {
                harness: farhelm_proto::LaunchHarness::Codex,
                model: Some("gpt-6-astra".into()),
                effort: Some(farhelm_proto::LaunchEffort::High),
                permissions: Some(farhelm_proto::LaunchPermission::Approve),
                workspace_trust: None,
            }),
            ..migration_session("history-retained", 1_700_000_100)
        };
        store
            .record_create_history(host, identity, &history_entry)
            .await
            .unwrap();
        {
            // v26 predates these two additive wire fields. Keep the cached
            // JSON at that historical shape while retaining every field v26
            // did persist, then prove the decoded content remains identical.
            let conn = store.conn();
            let conn = conn.lock();
            conn.execute(
                "UPDATE session_cache
                 SET info_json = json_remove(info_json, '$.github_repo', '$.working_copy')
                 WHERE session_id = ?1",
                ["cached-migration"],
            )
            .unwrap();
            let json: String = conn
                .query_row(
                    "SELECT info_json FROM session_cache WHERE session_id = 'cached-migration'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(!json.contains("github_repo"));
            assert!(!json.contains("working_copy"));
        }
        let cached_before = store.cached_sessions(host).await.unwrap();
        let launches_before = store.launch_history(host, identity).await.unwrap();
        let folders_before = store.folder_history(host, identity).await.unwrap();
        assert_eq!(cached_before, vec![cached]);
        assert_eq!(launches_before.len(), 100);
        assert_eq!(folders_before.len(), 100);
        assert_eq!(launches_before[0].cwd, "/srv/history-retained");
        assert!(
            !launches_before
                .iter()
                .any(|entry| entry.cwd == "/srv/history-1"),
            "the oldest entry must establish the bounded eviction premise"
        );

        drop(store);
        rewind_to_v26(&path).await;
        let raw = rusqlite::Connection::open(&path).unwrap();
        let version: i64 = raw
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 26, "fixture premise: exact v26 version");
        let additive_tables: i64 = raw
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name IN ('checkout_config', 'checkout_config_host')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            additive_tables, 0,
            "only the two post-v26 tables are removed"
        );
        drop(raw);

        let store = HelmStore::open(&path).await.expect("migrate to 27");
        assert_eq!(store.cached_sessions(host).await.unwrap(), cached_before);
        assert_eq!(
            store.launch_history(host, identity).await.unwrap(),
            launches_before
        );
        assert_eq!(
            store.folder_history(host, identity).await.unwrap(),
            folders_before
        );
        let snapshot = store.checkout_config_snapshot(None).await.unwrap();
        assert_eq!(snapshot.global_root, None);
        assert_eq!(snapshot.global_post_clone, None);
        assert_eq!(snapshot.revision, 0);
        // The migrated tables are usable, not just present.
        store
            .set_checkout_root(None, "/post-migration")
            .await
            .unwrap();
        assert_eq!(
            store.resolve_checkout_config(None).await.unwrap().root,
            Some("/post-migration".to_string())
        );
    }

    /// A newer schema is refused by both checkout-config actions while the
    /// production command preserves the database bytes and mode independently.
    #[tokio::test]
    async fn checkout_config_refuses_newer_schema_for_show_and_mutation() {
        let dir = tempfile::tempdir().unwrap();
        let state_dir = dir.path().join("state");
        std::fs::create_dir(&state_dir).unwrap();
        let path = state_dir.join("helm.db");
        let store = HelmStore::open(&path).await.unwrap();
        drop(store);
        let conn = rusqlite::Connection::open(&path).unwrap();
        let current_version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        let newer_version = current_version + 1;
        assert!(newer_version > current_version);
        conn.pragma_update(None, "user_version", newer_version)
            .unwrap();
        drop(conn);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
        let before = std::fs::read(&path).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o640);

        let show = checkout_config_cli(Some(state_dir.clone()), None, CheckoutConfigAction::Show)
            .await
            .expect_err("show must refuse a newer schema");
        assert!(format!("{show:#}").contains(&format!("schema version {newer_version}")));
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            mode
        );

        let mutation = checkout_config_cli(
            Some(state_dir),
            None,
            CheckoutConfigAction::SetRoot {
                path: "/refused".into(),
            },
        )
        .await
        .expect_err("mutation must refuse a newer schema");
        assert!(format!("{mutation:#}").contains(&format!("schema version {newer_version}")));
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            mode
        );
    }

    // ---- The CLI-facing command layer (in-process) --------------------

    /// Spec: the CLI layer refuses an absent helm.db with the
    /// start-or-update remedy (creating nothing), and `show` renders the
    /// stored values, the effective inheritance, and the revision for both
    /// scopes.
    #[tokio::test]
    async fn checkout_cli_reports_absent_database_and_renders_show() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state_dir = dir.path().join("state");
        let error = checkout_config_cli(Some(state_dir.clone()), None, CheckoutConfigAction::Show)
            .await
            .expect_err("an absent database must be refused");
        assert!(
            format!("{error:#}").contains("start (or update) the helm"),
            "{error:#}"
        );
        assert!(
            !state_dir.exists(),
            "the state dir must not be created either"
        );

        let state_dir = dir.path().join("populated");
        std::fs::create_dir(&state_dir).unwrap();
        {
            let store = HelmStore::open(&state_dir.join("helm.db"))
                .await
                .expect("fixture db");
            store.set_checkout_root(None, "/global/root").await.unwrap();
            let host = ssh_host(&store, "render.example").await;
            store.set_checkout_post_clone(Some(host), "").await.unwrap();

            let global =
                checkout_config_cli(Some(state_dir.clone()), None, CheckoutConfigAction::Show)
                    .await
                    .unwrap();
            // Both writes above (global root, host disable) bumped the
            // revision, so BOTH views report revision 2 here.
            assert_eq!(
                global,
                "scope: global\nroot: /global/root\npost-clone: unset\nrevision: 2\n"
            );
            let host_view = checkout_config_cli(
                Some(state_dir.clone()),
                Some(host),
                CheckoutConfigAction::Show,
            )
            .await
            .unwrap();
            assert_eq!(
                host_view,
                format!(
                    "scope: host {host}\nroot: /global/root (inherited from global)\n\
                     post-clone: disabled by host override\nrevision: 2\n"
                )
            );
        }
    }
}
