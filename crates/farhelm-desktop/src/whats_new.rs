//! The installed Mac app's bounded read of the website's finalized upgrade highlights.
//!
//! Publication can lag the app's version. A feed that omits the running release leaves the
//! baseline untouched, so the next launch can try again after the website has caught up.

use std::time::Duration;

use anyhow::{Context as _, ensure};
use farhelm_gui::desktop::updates::notes::{ReleaseHighlights, UpgradeNotes, UpgradeNotesHost};
use semver::Version;
use serde::Deserialize;

const FEED_URL: &str = "https://farhelm.io/whats-new.json";
// The file accumulates release highlights. Bound it during streaming while leaving room for years of notes.
const FEED_LIMIT: usize = 2 * 1024 * 1024;
const FEED_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Feed {
    format_version: u32,
    releases: Vec<ReleaseHighlights>,
}

/// A capability for the stable release notes feed, attached only to a running updater's installed app.
pub(crate) fn host(running: &str) -> Option<UpgradeNotesHost> {
    let version = Version::parse(running).ok()?;
    if !cfg!(target_os = "macos") || !version.pre.is_empty() || !version.build.is_empty() {
        return None;
    }
    Some(UpgradeNotesHost {
        running: version.to_string(),
        check: Box::new(move |previous, show| {
            let version = version.clone();
            Box::pin(async move {
                let decision = due(&version, previous.as_deref(), show)?;
                if let Some(decision) = decision {
                    return Ok(decision);
                }
                let (answer, receive) = tokio::sync::oneshot::channel();
                std::thread::Builder::new()
                    .name("farhelm-upgrade-highlights".into())
                    .spawn(move || {
                        let result =
                            fetch().and_then(|bytes| select(&bytes, &version, previous.as_deref()));
                        let _ = answer.send(result.map(UpgradeNotes::Show));
                    })
                    .context("starting the upgrade highlights lookup")?;
                receive
                    .await
                    .context("the upgrade highlights worker stopped")?
            })
        }),
    })
}

/// Decide the cases that need no HTTP request. Even a skipped upgrade advances the shared baseline.
fn due(
    running: &Version,
    previous: Option<&str>,
    show: bool,
) -> anyhow::Result<Option<UpgradeNotes>> {
    let previous = previous
        .map(Version::parse)
        .transpose()
        .context("parsing the upgrade baseline")?;
    if previous
        .as_ref()
        .is_some_and(|last| !running.cmp_precedence(last).is_gt())
    {
        Ok(Some(UpgradeNotes::Current))
    } else if !show {
        Ok(Some(UpgradeNotes::Skipped))
    } else {
        Ok(None)
    }
}

/// Use the updater's capped reader and worker deadline. This unsigned file stays on its declared HTTPS origin.
fn fetch() -> anyhow::Result<Vec<u8>> {
    crate::updater::fetch_on_worker(FEED_URL, FEED_LIMIT, FEED_TIMEOUT, &|timeout| {
        reqwest::Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .timeout(timeout)
            .build()
            .context("building the upgrade highlights HTTP client")
    })?
    .context("the website does not serve the upgrade highlights feed yet")
}

/// The site publishes versions newest first today. Sort by SemVer so order in the file cannot change the range.
/// On the first upgrade from a helm without a record, take only the running release.
fn select(
    bytes: &[u8],
    running: &Version,
    previous: Option<&str>,
) -> anyhow::Result<Vec<ReleaseHighlights>> {
    let feed: Feed =
        serde_json::from_slice(bytes).context("reading finalized upgrade highlights")?;
    ensure!(
        feed.format_version == 1,
        "unsupported upgrade highlights format"
    );
    let previous = previous.map(Version::parse).transpose()?;
    let mut selected = Vec::new();
    for mut release in feed.releases {
        let version = Version::parse(&release.version)
            .context("invalid release version in upgrade highlights")?;
        if !version.pre.is_empty()
            || !version.build.is_empty()
            || version.cmp_precedence(running).is_gt()
        {
            continue;
        }
        if &version != running
            && !previous
                .as_ref()
                .is_some_and(|last| version.cmp_precedence(last).is_gt())
        {
            continue;
        }
        for entry in release
            .highlights
            .iter_mut()
            .flat_map(|category| &mut category.entries)
        {
            entry.screenshot_url = entry
                .screenshot_url
                .as_deref()
                .and_then(canonical_screenshot_url);
        }
        selected.push((version, release));
    }
    ensure!(
        selected.iter().any(|(version, _)| version == running),
        "the website does not list the running release yet"
    );
    selected.sort_by(|(a, _), (b, _)| b.cmp_precedence(a));
    selected.dedup_by(|(a, _), (b, _)| a.cmp_precedence(b).is_eq());
    Ok(selected.into_iter().map(|(_, release)| release).collect())
}

/// Hand off the same canonical destination that passed the check. The browser opener parses it again.
fn canonical_screenshot_url(raw: &str) -> Option<String> {
    let url = reqwest::Url::parse(raw).ok()?;
    (url.scheme() == "https"
        && url.host_str() == Some("farhelm.io")
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && url.path() == "/docs/release-notes/"
        && url.query().is_none()
        && url.fragment().is_some_and(|fragment| !fragment.is_empty()))
    .then(|| url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    /// Model the generated wire format with minimal entries so tests exercise the production decoder too.
    fn release(version: &str, screenshot: Option<&str>) -> Value {
        json!({
            "version": version,
            "date": "2026-11-20",
            "highlights": [{
                "name": "Added",
                "entries": [{
                    "pieces": [{"style": "bold", "text": "A highlight"}],
                    "screenshotUrl": screenshot,
                }],
            }],
            "additionalChanges": 3,
        })
    }

    /// A gate that treats equal versions as new pings the website on every fresh install. An opted-out upgrade
    /// advances the record without asking the website, and an enabled upgrade is the only case that needs a fetch.
    #[test]
    fn only_an_enabled_upgrade_needs_the_website() {
        let running = Version::new(0, 100, 0);
        for saved in ["0.100.0", "0.100.0+another-build", "0.101.0"] {
            assert!(matches!(
                due(&running, Some(saved), true).unwrap(),
                Some(UpgradeNotes::Current)
            ));
        }
        assert!(matches!(
            due(&running, Some("0.99.0"), false).unwrap(),
            Some(UpgradeNotes::Skipped)
        ));
        assert!(due(&running, None, true).unwrap().is_none());
        assert!(due(&running, Some("0.100.0-rc.1"), true).unwrap().is_none());
    }

    /// Ordering is numeric and the range is open at the saved release. An old helm with no saved release
    /// offers the running release alone, even when there are other finalized entries available.
    #[test]
    fn select_the_interval_up_to_the_running_release() {
        let running = Version::new(0, 100, 0);
        let releases = [
            "0.98.0",
            "0.101.0",
            "0.99.0",
            "0.100.0+unlisted",
            "0.100.0-rc.1",
            "0.100.0",
            "0.97.0",
        ]
        .map(|version| release(version, None));
        let bytes = serde_json::to_vec(&json!({
            "formatVersion": 1,
            "releases": releases,
        }))
        .unwrap();
        let versions = |saved| {
            select(&bytes, &running, saved)
                .unwrap()
                .into_iter()
                .map(|release| release.version)
                .collect::<Vec<_>>()
        };
        assert_eq!(versions(Some("0.98.0")), ["0.100.0", "0.99.0"]);
        assert_eq!(versions(None), ["0.100.0"]);
    }

    /// Another finalized release can appear before the running one is deployed. That response and an unavailable
    /// format both fail the check, leaving the next launch able to retry.
    #[test]
    fn the_feed_must_list_the_running_release_in_a_known_format() {
        let running = Version::new(0, 100, 0);
        for (format, version, reason) in [
            (1, "0.99.0", "running release"),
            (2, "0.100.0", "unsupported upgrade highlights format"),
        ] {
            let bytes = serde_json::to_vec(&json!({
                "formatVersion": format,
                "releases": [release(version, None)],
            }))
            .unwrap();
            assert!(
                select(&bytes, &running, Some("0.98.0"))
                    .unwrap_err()
                    .to_string()
                    .contains(reason)
            );
        }
    }

    /// Keep a legitimate screenshot link and strip lookalikes through the production selection path.
    #[test]
    fn screenshots_open_only_the_official_release_notes_page() {
        let running = Version::new(0, 100, 0);
        let official = "https://farhelm.io/docs/release-notes/#v0.100.0-pr-1";
        for (url, expected) in [
            (official, Some(official)),
            (
                "https://FARHELM.io/docs/../docs/release-notes/#v0.100.0-pr-1",
                Some(official),
            ),
            (
                "https://farhelm.io.example.test/docs/release-notes/#v",
                None,
            ),
            (
                "https://farhelm.io@example.test/docs/release-notes/#v",
                None,
            ),
            ("https://farhelm.io:8443/docs/release-notes/#v", None),
            ("https://farhelm.io/docs/release-notes/?other=1#v", None),
            ("https://farhelm.io/docs/release-notes/", None),
            ("https://farhelm.io/elsewhere/#v", None),
            ("http://farhelm.io/docs/release-notes/#v", None),
        ] {
            let bytes = serde_json::to_vec(&json!({
                "formatVersion": 1,
                "releases": [release("0.100.0", Some(url))],
            }))
            .unwrap();
            let selected = select(&bytes, &running, None).unwrap();
            assert_eq!(
                selected[0].highlights[0].entries[0]
                    .screenshot_url
                    .as_deref(),
                expected,
                "{url}"
            );
        }
    }
}
