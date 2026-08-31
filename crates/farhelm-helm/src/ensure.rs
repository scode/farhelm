//! `--ensure-hosts <file>`: a JSON5 floor under the host registry
//! (PLAN_M6.md item 5).
//!
//! A file of hosts the helm guarantees are registered before it starts
//! serving, for half-automated setups and agent-driven testing — the shape
//! that replaces M1's `--ssh`, now that the registry and the create API are
//! the mechanism. It is a FLOOR and nothing else: it adds hosts that are
//! missing, it never removes or reshapes hosts that are present, and after
//! ingestion it plays no further role. Nothing re-reads it, nothing watches
//! it, and removing a host through the API does not fight it — until the
//! next startup, which is the honest consequence of "guaranteed at
//! startup".
//!
//! ## All-or-nothing
//!
//! Every entry is validated before any entry is written, and the writes
//! themselves are ONE transaction ([`HelmStore::ensure_ssh_hosts`]). A
//! half-ingested guaranteed set is worse than a loud refusal: a helm that
//! came up with three of five hosts registered looks healthy, and the two
//! that are missing look like hosts the user forgot to add. So a malformed
//! file, an entry whose destination `ssh` could not use, or the same
//! destination listed twice all fail startup with the offending entry
//! named, and nothing is written at all.
//!
//! Both halves are needed and neither is redundant. Validating up front is
//! what turns the predictable failures into a clear message naming the
//! entry rather than a constraint violation from the middle of a batch; the
//! transaction is what makes the guarantee hold for the unpredictable ones
//! (a disk error on the fourth insert) that no amount of validation can
//! anticipate.
//!
//! ## Why JSON5
//!
//! The file is hand-edited and checked in beside other config. Comments
//! and trailing commas are the difference between a fleet description
//! someone can annotate ("this box only has the old binary") and one they
//! have to keep notes about elsewhere.

use crate::store::{EnsureHost as StoreEnsureHost, HelmStore};
use anyhow::Context as _;
use serde::Deserialize;
use std::path::Path;

/// The whole file: `{ hosts: [ ... ] }`.
///
/// A wrapper object rather than a bare array so the format has somewhere to
/// grow — the alternative would make any future top-level key a breaking
/// change to every existing file. `deny_unknown_fields` throughout because
/// this is hand-written config: a typo'd key silently ignored is a host
/// that silently does not appear, which is precisely the failure the file
/// exists to prevent.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnsureFile {
    hosts: Vec<EnsureHost>,
}

/// One guaranteed host — the same three fields `POST /api/hosts` takes,
/// under the same names, because both go through the same registration
/// path and a second spelling would be a second thing to keep in sync.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnsureHost {
    ssh: String,
    #[serde(default)]
    remote_farhelm: Option<String>,
    #[serde(default)]
    remote_state_dir: Option<String>,
}

/// Register every host in `path` that is not registered already.
///
/// Runs BEFORE the helm serves anything, so the guarantee is true for the
/// first request rather than eventually. Idempotent by construction: a
/// destination already in the registry is left EXACTLY as it is, including
/// its `remote_farhelm`/`remote_state_dir` and its learned identity.
///
/// That last point is the one deliberate narrowing of the word "upsert",
/// and it is worth being explicit about because it is the case a reader
/// will wonder about: editing a host's remote binary path in this file and
/// restarting does NOT retarget an existing row. helm.db is the durable
/// registry and the API is how it is edited; a startup file that silently
/// overwrote user edits every boot would make the two authorities fight,
/// and the losing one would be the interactive one. Fix a registered
/// host through `/api/hosts`, or remove it and let the file re-add it.
///
/// ALL-OR-NOTHING, at the store: parsing and per-entry validation happen
/// here, and the registration itself is one transaction
/// ([`HelmStore::ensure_ssh_hosts`]). A loop of individual adds could not
/// promise that — each would commit on its own, so a failure halfway would
/// leave a helm running with part of its guaranteed fleet, which looks
/// healthy and is not.
pub(crate) async fn ingest(store: &HelmStore, path: &Path) -> anyhow::Result<()> {
    let text = tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("reading the --ensure-hosts file {}", path.display()))?;
    let parsed: EnsureFile = json5::from_str(&text)
        .with_context(|| format!("parsing the --ensure-hosts file {}", path.display()))?;

    let added = store
        .ensure_ssh_hosts(
            parsed
                .hosts
                .into_iter()
                .map(|host| StoreEnsureHost {
                    destination: host.ssh,
                    remote_farhelm: host.remote_farhelm,
                    remote_state_dir: host.remote_state_dir,
                })
                .collect(),
        )
        .await
        .with_context(|| format!("applying the --ensure-hosts file {}", path.display()))?;
    if !added.is_empty() {
        tracing::info!(
            hosts = ?added,
            file = %path.display(),
            "registered guaranteed hosts from --ensure-hosts"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::HostKind;

    /// A store on a fresh helm.db, with only the reserved local row in it.
    async fn fresh_store() -> (HelmStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("temp state dir");
        let store = HelmStore::open(&dir.path().join("helm.db"))
            .await
            .expect("open helm.db");
        (store, dir)
    }

    async fn write_file(dir: &tempfile::TempDir, text: &str) -> std::path::PathBuf {
        let path = dir.path().join("ensure.json5");
        tokio::fs::write(&path, text).await.expect("write file");
        path
    }

    /// Destinations registered by the ensure file, in registry order.
    async fn destinations(store: &HelmStore) -> Vec<String> {
        store
            .list_hosts()
            .await
            .expect("list hosts")
            .into_iter()
            .filter(|row| row.kind == HostKind::Ssh)
            .map(|row| row.destination.expect("ssh rows always carry one"))
            .collect()
    }

    /// The feature's basic promise, plus the two JSON5 affordances the
    /// format was chosen for: a file with comments and a trailing comma
    /// must register its hosts with their optional fields intact.
    #[tokio::test]
    async fn a_fresh_file_registers_its_hosts_with_their_remote_fields() {
        let (store, dir) = fresh_store().await;
        let path = write_file(
            &dir,
            r#"{
                // the build box
                hosts: [
                    { ssh: "user@build", remote_farhelm: "/opt/farhelm" },
                    { ssh: "user@spare", remote_state_dir: "/srv/state" },
                ],
            }"#,
        )
        .await;

        ingest(&store, &path).await.expect("ingest");

        let rows = store.list_hosts().await.expect("list hosts");
        let build = rows
            .iter()
            .find(|row| row.destination.as_deref() == Some("user@build"))
            .expect("build host registered");
        assert_eq!(build.remote_farhelm.as_deref(), Some("/opt/farhelm"));
        assert_eq!(build.remote_state_dir, None);
        let spare = rows
            .iter()
            .find(|row| row.destination.as_deref() == Some("user@spare"))
            .expect("spare host registered");
        assert_eq!(spare.remote_farhelm, None);
        assert_eq!(spare.remote_state_dir.as_deref(), Some("/srv/state"));
    }

    /// Restarting a helm with the same ensure file must be a no-op, not a
    /// failure and not a duplicate: the file is a floor that is re-applied
    /// on every boot, so idempotence is the ordinary case rather than an
    /// edge one.
    ///
    /// The second leg also pins the deliberate narrowing of "upsert" the
    /// module docs record: an entry whose `remote_farhelm` CHANGED leaves
    /// the registered row alone, because helm.db is the authority for a
    /// host that already exists.
    #[tokio::test]
    async fn re_running_the_same_file_neither_duplicates_nor_overwrites() {
        let (store, dir) = fresh_store().await;
        let path = write_file(
            &dir,
            r#"{ hosts: [{ ssh: "user@build", remote_farhelm: "/opt/farhelm" }] }"#,
        )
        .await;
        ingest(&store, &path).await.expect("first ingest");

        let edited = write_file(
            &dir,
            r#"{ hosts: [{ ssh: "user@build", remote_farhelm: "/usr/bin/farhelm" }] }"#,
        )
        .await;
        ingest(&store, &edited).await.expect("second ingest");

        assert_eq!(destinations(&store).await, vec!["user@build".to_string()]);
        let row = store
            .list_hosts()
            .await
            .expect("list hosts")
            .into_iter()
            .find(|row| row.destination.as_deref() == Some("user@build"))
            .expect("still registered exactly once");
        assert_eq!(
            row.remote_farhelm.as_deref(),
            Some("/opt/farhelm"),
            "a registered host's fields belong to helm.db, not to the startup file"
        );
    }

    /// The file never deletes. A host registered through the API and then
    /// absent from the ensure file must survive, because "guaranteed
    /// present" is not "exclusively present" — treating the file as the
    /// complete fleet would make one forgotten line silently forget a host.
    #[tokio::test]
    async fn a_host_absent_from_the_file_is_never_removed() {
        let (store, dir) = fresh_store().await;
        store
            .add_ssh_host("user@added-by-hand", None, None)
            .await
            .expect("register out of band");
        let path = write_file(&dir, r#"{ hosts: [{ ssh: "user@from-file" }] }"#).await;

        ingest(&store, &path).await.expect("ingest");

        let mut found = destinations(&store).await;
        found.sort();
        assert_eq!(
            found,
            vec![
                "user@added-by-hand".to_string(),
                "user@from-file".to_string()
            ]
        );
    }

    /// A malformed file, an unusable destination, and a repeated
    /// destination must all fail startup — and must write NOTHING, so a
    /// helm never comes up with a partially applied guarantee. The
    /// error names the offending entry, since "the ensure file is bad" is
    /// not something a user can act on.
    ///
    /// Each case asserts on its OWN diagnostic fragment rather than merely
    /// on "an error happened": every one of these fails for a different
    /// reason a user has to act on differently, and a shared assertion
    /// would pass just as happily if they all collapsed into one message.
    #[tokio::test]
    async fn a_bad_file_fails_loudly_and_registers_nothing() {
        for (label, body, fragment) in [
            ("not json5 at all", "{ hosts: [", "parsing"),
            ("unknown key", r#"{ hosts: [{ shh: "user@typo" }] }"#, "shh"),
            (
                "option-shaped destination",
                r#"{ hosts: [{ ssh: "user@ok" }, { ssh: "-oProxyCommand=touch /tmp/pwned" }] }"#,
                "entry 1",
            ),
            (
                "empty destination",
                r#"{ hosts: [{ ssh: "" }] }"#,
                "entry 0",
            ),
            (
                "NUL in the destination",
                "{ hosts: [{ ssh: \"good.example\\u0000evil.example\" }] }",
                "NUL",
            ),
            (
                "repeated destination",
                r#"{ hosts: [{ ssh: "user@twice" }, { ssh: "user@twice" }] }"#,
                "repeats destination",
            ),
        ] {
            let (store, dir) = fresh_store().await;
            let path = write_file(&dir, body).await;
            let error = ingest(&store, &path)
                .await
                .expect_err("a bad ensure file must fail startup");
            let rendered = format!("{error:#}");
            assert!(
                rendered.contains(&path.display().to_string()),
                "the {label} error must name the file: {rendered}"
            );
            assert!(
                rendered.contains(fragment),
                "the {label} error must say what is wrong with it (expected {fragment:?}): \
                 {rendered}"
            );
            assert!(
                destinations(&store).await.is_empty(),
                "the {label} file must have registered nothing at all"
            );
        }
    }

    /// Re-ingesting the same file must not disturb what the registry has
    /// LEARNED since: a host's identity and its cached sessions both belong
    /// to helm.db, not to the startup file.
    ///
    /// The failure this pins against would be quiet and expensive: an
    /// ingestion that re-registered (or rewrote) an existing row would drop
    /// its identity, which cascades its cache away — so every boot would
    /// wipe the stale list the cache exists to serve, and a host that was
    /// down at startup would show nothing at all.
    #[tokio::test]
    async fn re_ingesting_preserves_a_hosts_learned_identity_and_cache() {
        let (store, dir) = fresh_store().await;
        let path = write_file(&dir, r#"{ hosts: [{ ssh: "user@known" }] }"#).await;
        ingest(&store, &path).await.expect("first ingest");

        let row = store
            .list_hosts()
            .await
            .expect("list hosts")
            .into_iter()
            .find(|row| row.destination.as_deref() == Some("user@known"))
            .expect("registered");
        let outcome = store
            .record_first_contact(row.id, &crate::store::DialedAs::of(&row), "identity-known")
            .await
            .expect("first contact");
        assert_eq!(outcome, crate::store::FirstContactOutcome::Recorded);
        store
            .replace_host_sessions(
                row.id,
                "identity-known",
                vec![farhelm_proto::SessionInfo {
                    parent: None,
                    archived: false,
                    id: "remembered".to_string(),
                    title: "remembered".to_string(),
                    created_at: 100,
                    last_activity_at: 100,
                    creation_seq: None,
                    cwd: "/tmp".to_string(),
                    invocation: "agent".to_string(),
                    status: farhelm_proto::SessionStatus::Running,
                    annotation: None,
                    restart_offer: farhelm_proto::RestartOffer::default(),
                    tabs: Vec::new(),
                    source_profile: None,
                }],
                false,
            )
            .await
            .expect("cache a session");

        ingest(&store, &path).await.expect("second ingest");

        let after = store
            .list_hosts()
            .await
            .expect("list hosts")
            .into_iter()
            .find(|entry| entry.destination.as_deref() == Some("user@known"))
            .expect("still registered");
        assert_eq!(after.id, row.id, "the row itself must survive, id and all");
        assert_eq!(
            after.host_identity.as_deref(),
            Some("identity-known"),
            "a learned identity belongs to helm.db and must survive re-ingestion"
        );
        let cached: Vec<String> = store
            .cached_sessions(row.id)
            .await
            .expect("cached sessions")
            .into_iter()
            .map(|info| info.id)
            .collect();
        assert_eq!(
            cached,
            vec!["remembered".to_string()],
            "and so must the stale list that identity vouches for"
        );
    }

    /// An empty `hosts` list is a legal file, not an error: it is what a
    /// generated file looks like before anything has been added to it, and
    /// refusing it would make the generator's job harder for no benefit.
    #[tokio::test]
    async fn an_empty_host_list_is_accepted() {
        let (store, dir) = fresh_store().await;
        let path = write_file(&dir, r#"{ hosts: [] }"#).await;
        ingest(&store, &path).await.expect("ingest");
        assert!(destinations(&store).await.is_empty());
    }

    /// A missing file is a startup failure naming the path. The flag is an
    /// explicit request for a guarantee, so silently proceeding without it
    /// would deliver the opposite of what was asked for.
    #[tokio::test]
    async fn a_missing_file_fails_with_its_path() {
        let (store, dir) = fresh_store().await;
        let missing = dir.path().join("nope.json5");
        let error = ingest(&store, &missing)
            .await
            .expect_err("a missing ensure file must fail startup");
        assert!(
            format!("{error:#}").contains("nope.json5"),
            "the error must name the file: {error:#}"
        );
    }
}
