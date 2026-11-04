//! The compiled release table is one producer of the general announcement queue.
//!
//! Only this module interprets release versions or composes release-notes links.
//! Eligibility compares the database's original build with the running build,
//! rather than a last-seen version: skipped releases remain eligible until their
//! ids have been enqueued, and idempotent storage keeps later wording changes
//! from rewriting content a helm already received.

use crate::announcements::{Announcement, AnnouncementLink};
use crate::store::HelmStore;
use anyhow::Context;
use semver::Version;
use serde::Deserialize;
use std::cmp::Ordering;
use std::collections::HashSet;

/// Maintainer-selected content attached to the exact release it first ships in.
/// Keep the data file empty until a release explicitly adds an announcement.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseAnnouncement {
    version: String,
    id: String,
    title: String,
    body: String,
}

/// Load the shipped table and finish enqueueing before the helm serves clients.
/// Main builds enqueue nothing, even if the table contains future releases.
pub(crate) async fn enqueue_builtin(store: &HelmStore) -> anyhow::Result<()> {
    let table: Vec<ReleaseAnnouncement> =
        serde_json::from_str(include_str!("release_announcements.json"))
            .context("decoding the compiled release announcement table")?;
    enqueue(store, env!("CARGO_PKG_VERSION"), &table).await
}

/// Apply the release producer with explicit build and content inputs.
/// Production supplies the compiled values above; tests supply tables directly,
/// without adding a runtime switch that could deliver fixture announcements.
async fn enqueue(
    store: &HelmStore,
    build: &str,
    table: &[ReleaseAnnouncement],
) -> anyhow::Result<()> {
    let mut entries = validate_table(table)?;
    if build == "0.0.0-unreleased" {
        return Ok(());
    }
    let running = Version::parse(build).context("parsing the helm's release build")?;
    let baseline = store
        .creation_build()
        .await?
        .filter(|build| build != "0.0.0-unreleased")
        .map(|build| Version::parse(&build).context("parsing the helm's creation build"))
        .transpose()?;

    // Table file order is editorial, not delivery order. Full semver precedence
    // puts RC/dev entries between the releases on either side, and stable sort
    // preserves the maintainer's order for entries attached to the same release.
    entries.sort_by(|(_, a), (_, b)| a.cmp_precedence(b));
    let announcements = entries
        .into_iter()
        .filter(|(_, version)| {
            version.cmp_precedence(&running) != Ordering::Greater
                && baseline
                    .as_ref()
                    .is_none_or(|baseline| version.cmp_precedence(baseline) == Ordering::Greater)
        })
        .map(|(entry, version)| Announcement {
            id: entry.id.clone(),
            title: entry.title.clone(),
            body: entry.body.clone(),
            detail: Some(entry.version.clone()),
            // Prereleases have no curated notes yet. Omitting their link avoids
            // promising a stable section that may not have been published.
            link: version.pre.is_empty().then(|| AnnouncementLink {
                label: "Release notes".into(),
                url: format!(
                    "https://farhelm.io/docs/release-notes/#v{}.{}.{}",
                    version.major, version.minor, version.patch
                ),
            }),
        })
        .collect();
    store.enqueue_announcements(announcements).await
}

/// Validate every entry, including releases that this build cannot enqueue.
/// A bad future entry must fail the release edit's focused check rather than
/// wait until users reach that version. Return parsed versions for one shared
/// definition of valid release text and semver precedence.
fn validate_table(
    table: &[ReleaseAnnouncement],
) -> anyhow::Result<Vec<(&ReleaseAnnouncement, Version)>> {
    let mut ids = HashSet::new();
    table
        .iter()
        .map(|entry| {
            anyhow::ensure!(
                !entry.id.trim().is_empty(),
                "announcement id must not be empty"
            );
            anyhow::ensure!(
                ids.insert(&entry.id),
                "duplicate announcement id {:?}",
                entry.id
            );
            anyhow::ensure!(
                !entry.title.trim().is_empty() && !entry.body.trim().is_empty(),
                "announcement {:?} needs a title and body",
                entry.id
            );
            anyhow::ensure!(
                entry.version != "0.0.0-unreleased",
                "announcement {:?} must belong to a tagged release",
                entry.id
            );
            let version = Version::parse(&entry.version).with_context(|| {
                format!("invalid release version for announcement {:?}", entry.id)
            })?;
            Ok((entry, version))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Keep synthetic releases in test memory only; the shipped table never
    /// needs a fixture entry or a runtime override to make producer rules testable.
    fn entry(version: &str, id: &str) -> ReleaseAnnouncement {
        ReleaseAnnouncement {
            version: version.into(),
            id: id.into(),
            title: format!("Announcement {id}"),
            body: format!("What changed in {id}.\n\nWhat to do next."),
        }
    }

    /// Set the durable baseline explicitly, without changing process-global
    /// build inputs. Creation/migration themselves are tested in the store.
    async fn store_with_baseline(baseline: Option<&str>) -> (tempfile::TempDir, HelmStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = HelmStore::open(&dir.path().join("helm.db")).await.unwrap();
        {
            let db = store.connection_for_test();
            let conn = db.lock();
            conn.execute("DELETE FROM helm_creation_build", []).unwrap();
            if let Some(baseline) = baseline {
                conn.execute(
                    "INSERT INTO helm_creation_build (singleton, build) VALUES (1, ?1)",
                    [baseline],
                )
                .unwrap();
            }
        }
        assert_eq!(store.creation_build().await.unwrap().as_deref(), baseline);
        (dir, store)
    }

    /// Release procedure edits run this uniquely named check before merging.
    /// Empty is valid: the mechanism ships without a real announcement.
    #[test]
    fn release_announcement_table_is_valid() {
        let table: Vec<ReleaseAnnouncement> =
            serde_json::from_str(include_str!("release_announcements.json")).unwrap();
        validate_table(&table).unwrap();
    }

    /// Invalid future entries must fail even while the running version is too
    /// old to deliver them. Duplicate ids would otherwise silently lose content.
    #[test]
    fn release_announcement_table_rejects_invalid_content() {
        let valid = entry("0.36.0-rc.3", "0.36.0-rc.3-action");
        assert!(validate_table(&[valid]).is_ok());
        assert!(validate_table(&[entry("0.36.0", "same"), entry("0.37.0", "same")]).is_err());
        for version in ["not-a-version", "0.36", "v0.36.0", "0.0.0-unreleased"] {
            assert!(
                validate_table(&[entry(version, "notice")]).is_err(),
                "{version}"
            );
        }
        for field in ["id", "title", "body"] {
            let mut invalid = entry("0.36.0", "notice");
            match field {
                "id" => invalid.id = "  ".into(),
                "title" => invalid.title = "\n\t".into(),
                "body" => invalid.body = " ".into(),
                _ => unreachable!(),
            }
            assert!(validate_table(&[invalid]).is_err(), "empty {field}");
        }
        assert!(
            serde_json::from_str::<Vec<ReleaseAnnouncement>>(
                r#"[{"version":"0.36.0","id":"notice","title":"Title","body":"Body","typo":true}]"#,
            )
            .is_err()
        );
    }

    /// A skipped stable release includes all intervening tagged prereleases.
    /// File order, lexical prerelease order and build metadata must not replace
    /// semver precedence, and future releases must remain out of the queue.
    #[farhelm_testtrace::test]
    async fn release_producer_uses_full_precedence_and_orders_skipped_releases() {
        let (_dir, store) = store_with_baseline(Some("0.35.0+original")).await;
        let table = vec![
            entry("0.37.0", "future"),
            entry("0.36.0", "stable"),
            entry("0.36.0-rc.10", "rc10"),
            entry("0.35.0+rebuilt", "baseline"),
            entry("0.36.0-rc.3", "rc3"),
            entry("0.36.0-dev.2", "dev"),
            entry("0.34.0", "old"),
            entry("0.36.0", "stable-second"),
        ];
        enqueue(&store, "0.36.0+running", &table).await.unwrap();
        let queue = store.announcements().await.unwrap();
        assert!(queue.history.is_empty());
        assert_eq!(
            queue
                .pending
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["dev", "rc3", "rc10", "stable", "stable-second"]
        );
        for notice in &queue.pending[..3] {
            assert!(
                notice.link.is_none(),
                "prereleases have no curated release notes"
            );
        }
        let stable = &queue.pending[3];
        assert_eq!(stable.detail.as_deref(), Some("0.36.0"));
        assert_eq!(
            stable.link,
            Some(AnnouncementLink {
                label: "Release notes".into(),
                url: "https://farhelm.io/docs/release-notes/#v0.36.0".into(),
            })
        );
        assert_eq!(stable.title, table[1].title);
        assert_eq!(
            stable.body, table[1].body,
            "producer preserves the maintainer's exact wording"
        );
    }

    /// Baselines exclude their own release and everything earlier. Main builds
    /// deliver nothing; a database created on main retains no release baseline.
    /// Legacy databases likewise start older than every table entry.
    #[farhelm_testtrace::test]
    async fn release_producer_respects_fresh_legacy_and_unreleased_baselines() {
        let table = vec![
            entry("0.35.0", "previous"),
            entry("0.36.0-dev.2", "dev"),
            entry("0.36.0-rc.3", "rc"),
            entry("0.36.0", "stable"),
            entry("0.37.0", "next"),
        ];
        let cases: &[(Option<&str>, &str, &[&str])] = &[
            (Some("0.36.0"), "0.36.0", &[]),
            (Some("0.36.0-rc.3"), "0.36.0-rc.3", &[]),
            (Some("0.36.0-rc.3"), "0.36.0", &["stable"]),
            (Some("0.35.0"), "0.36.0-rc.3", &["dev", "rc"]),
            (Some("0.35.0"), "0.36.0-dev.2", &["dev"]),
            (Some("0.35.0"), "0.37.0", &["dev", "rc", "stable", "next"]),
            (Some("0.35.0"), "0.0.0-unreleased", &[]),
            (
                Some("0.0.0-unreleased"),
                "0.36.0",
                &["previous", "dev", "rc", "stable"],
            ),
            (None, "0.36.0", &["previous", "dev", "rc", "stable"]),
            (Some("0.37.0"), "0.36.0", &[]),
        ];
        for (baseline, running, expected) in cases {
            let (_dir, store) = store_with_baseline(*baseline).await;
            enqueue(&store, running, &table).await.unwrap();
            let queue = store.announcements().await.unwrap();
            assert_eq!(
                queue
                    .pending
                    .iter()
                    .map(|item| item.id.as_str())
                    .collect::<Vec<_>>(),
                *expected,
                "baseline {baseline:?}, running {running}"
            );
        }
    }

    /// Restarting on a later build delivers new entries but cannot replay a
    /// shown RC announcement or replace the wording this helm already received.
    #[farhelm_testtrace::test]
    async fn release_producer_keeps_original_content_and_shown_state_on_restart() {
        let (dir, store) = store_with_baseline(Some("0.35.0")).await;
        let table = vec![entry("0.36.0-rc.3", "rc")];
        enqueue(&store, "0.36.0-rc.3", &table).await.unwrap();
        let original = store.announcements().await.unwrap().pending;
        assert_eq!(original.len(), 1, "fixture premise: the RC was received");
        store
            .mark_announcements_shown(vec!["rc".into()])
            .await
            .unwrap();
        drop(store);

        let store = HelmStore::open(&dir.path().join("helm.db")).await.unwrap();
        assert_eq!(
            store.creation_build().await.unwrap().as_deref(),
            Some("0.35.0")
        );
        let mut changed = entry("0.36.0-rc.3", "rc");
        changed.title = "A later wording edit".into();
        changed.body = "A different body".into();
        enqueue(&store, "0.36.0", &[changed, entry("0.36.0", "stable")])
            .await
            .unwrap();
        let queue = store.announcements().await.unwrap();
        assert_eq!(queue.history, original);
        assert_eq!(
            queue
                .pending
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["stable"]
        );
    }
}
