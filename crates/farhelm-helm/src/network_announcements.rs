//! Import verified, display-ready announcements from the release origin.
//!
//! The network supplies the same document to every installation. This helm
//! applies targeting locally and keeps the first content received for each ID.

use crate::announcements::{Announcement, AnnouncementSource};
use crate::feed::FleetEvents;
use crate::store::HelmStore;
use crate::{RELEASE_KEY_RING, verify_signed_metadata};
use anyhow::{Context, ensure};
use chrono::{DateTime, FixedOffset, Utc};
use reqwest::{Client, StatusCode};
use rusqlite::{Connection, OptionalExtension};
use semver::Version;
use serde::Deserialize;
use std::ffi::OsStr;
use std::time::{Duration, SystemTime};
use url::Url;

const ORIGIN: &str = "https://get.farhelm.io";
const DEFAULT_PATH: &str = "announcements.json";
const PURPOSE: &str = "farhelm announcements";
const BODY_CAP: usize = 256 * 1024;
const SIGNATURE_CAP: usize = 4 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const CHECK_INTERVAL: Duration = Duration::from_secs(12 * 60 * 60);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Feed {
    version: u32,
    announcements: Vec<Entry>,
}

/// Targeting is validated for the entire signed feed before any row is admitted.
#[derive(Deserialize)]
struct Entry {
    #[serde(flatten)]
    announcement: Announcement,
    min_version: Option<String>,
    max_version: Option<String>,
    os: Option<TargetOs>,
    expires_at: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum TargetOs {
    One(Os),
    Many(Vec<Os>),
}

#[derive(Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Os {
    Macos,
    Linux,
}

impl Os {
    /// The known OS words are part of the feed. A new helm platform can still
    /// receive untargeted announcements until the schema gives it a word.
    fn running() -> Option<Self> {
        if cfg!(target_os = "macos") {
            Some(Self::Macos)
        } else if cfg!(target_os = "linux") {
            Some(Self::Linux)
        } else {
            None
        }
    }
}

/// A brick test can change the filename at the pinned origin. Invalid input
/// uses the ordinary production feed, including values that are not UTF-8.
fn filename(override_path: Option<&OsStr>) -> &str {
    let Some(path) = override_path else {
        return DEFAULT_PATH;
    };
    if let Some(name) = path.to_str()
        && name.len() <= 128
        && name
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && !name.contains("..")
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
    {
        return name;
    }
    tracing::warn!("invalid FARHELM_ANNOUNCEMENTS_PATH, using announcements.json");
    DEFAULT_PATH
}

/// The test transport can use the same response policy over its private HTTP listener.
fn client_builder() -> reqwest::ClientBuilder {
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(REQUEST_TIMEOUT)
}

/// Treat only 404 as a quiet absence. reqwest's error_for_status accepts
/// redirects, so require a successful status before consuming response bytes.
async fn get(client: &Client, url: &str, cap: usize) -> anyhow::Result<Option<Vec<u8>>> {
    let mut response = client.get(url).send().await?;
    let status = response.status();
    if status == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    ensure!(
        status.is_success(),
        "announcement feed response was HTTP {status}"
    );
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        ensure!(
            chunk.len() <= cap - bytes.len(),
            "announcement response exceeds {cap} bytes"
        );
        bytes.extend_from_slice(&chunk);
    }
    Ok(Some(bytes))
}

/// Drop just the optional link when its authority is outside the project site.
/// The parser reports embedded credentials even when it normalizes away empty userinfo.
fn safe_link(raw: &str) -> Option<String> {
    let credentials = std::cell::Cell::new(false);
    let url = Url::options()
        .syntax_violation_callback(Some(&|violation| {
            if violation == url::SyntaxViolation::EmbeddedCredentials {
                credentials.set(true);
            }
        }))
        .parse(raw)
        .ok()?;
    (url.scheme() == "https"
        && url.host_str() == Some("farhelm.io")
        && url.port().is_none()
        && !credentials.get())
    .then(|| url.into())
}

/// Authentication is the admission boundary for parsing as well as storage.
fn decode(
    body: &[u8],
    signature: &[u8],
    running: &Version,
    os: Option<Os>,
    now: DateTime<Utc>,
    verify: impl FnOnce(&str, &[u8], &[u8]) -> anyhow::Result<()>,
) -> anyhow::Result<Vec<Announcement>> {
    verify(PURPOSE, body, signature)?;
    let feed: Feed = serde_json::from_slice(body).context("decoding the announcement feed")?;
    ensure!(
        feed.version == 1,
        "unsupported announcement feed version {}",
        feed.version
    );
    let mut active = Vec::new();
    for entry in feed.announcements {
        let min = entry
            .min_version
            .as_deref()
            .map(Version::parse)
            .transpose()
            .context("invalid announcement min_version")?;
        let max = entry
            .max_version
            .as_deref()
            .map(Version::parse)
            .transpose()
            .context("invalid announcement max_version")?;
        let expiry: Option<DateTime<FixedOffset>> = entry
            .expires_at
            .as_deref()
            .map(DateTime::parse_from_rfc3339)
            .transpose()
            .context("invalid announcement expiry")?;
        let mut announcement = entry.announcement;
        ensure!(
            !announcement.id.trim().is_empty()
                && !announcement.title.trim().is_empty()
                && !announcement.body.trim().is_empty(),
            "announcement requires an ID, title and body"
        );
        let matches_os = match entry.os {
            None => true,
            Some(TargetOs::One(target)) => os == Some(target),
            Some(TargetOs::Many(targets)) => os.is_some_and(|os| targets.contains(&os)),
        };
        if !matches_os
            || min.is_some_and(|min| running.cmp_precedence(&min).is_lt())
            || max.is_some_and(|max| running.cmp_precedence(&max).is_gt())
            || expiry.is_some_and(|expiry| expiry.to_utc() <= now)
        {
            continue;
        }
        announcement.link = announcement.link.and_then(|mut link| {
            link.url = safe_link(&link.url)?;
            Some(link)
        });
        active.push(announcement);
    }
    Ok(active)
}

/// Read the preference before making a request and again within the enqueue
/// transaction. A missing singleton or unset value inherits the enabled default.
pub(crate) fn enabled(conn: &Connection) -> anyhow::Result<bool> {
    Ok(conn
        .query_row(
            "SELECT check_announcements FROM preferences WHERE singleton = 1",
            [],
            |row| row.get::<_, Option<bool>>(0),
        )
        .optional()?
        .flatten()
        != Some(false))
}

/// Sample the current preference rather than caching a launch-time choice.
async fn requests_enabled(store: &HelmStore) -> anyhow::Result<bool> {
    store
        .conn()
        .call("announcement preference read task panicked", |conn| {
            enabled(conn)
        })
        .await
}

/// Fetch a complete pair without issuing a signature request for an absent
/// feed or a helm that opted out while the first request was open.
async fn fetch_pair(
    store: &HelmStore,
    client: &Client,
    body_url: &str,
    signature_url: &str,
) -> anyhow::Result<Option<(Vec<u8>, Vec<u8>)>> {
    let Some(body) = get(client, body_url, BODY_CAP).await? else {
        return Ok(None);
    };
    if !requests_enabled(store).await? {
        return Ok(None);
    }
    Ok(get(client, signature_url, SIGNATURE_CAP)
        .await?
        .map(|signature| (body, signature)))
}

/// Each tick samples current preferences, with fetch injection confined to
/// this call boundary. A no-op never invalidates the UI collections.
async fn check(
    store: &HelmStore,
    events: &FleetEvents,
    fetch: impl AsyncFnOnce() -> anyhow::Result<Option<Vec<Announcement>>>,
) -> anyhow::Result<()> {
    if !requests_enabled(store).await? {
        return Ok(());
    }
    if let Some(announcements) = fetch().await?
        && store
            .enqueue_announcements(announcements, AnnouncementSource::Network)
            .await?
            > 0
    {
        events.bump();
    }
    Ok(())
}

/// Runs alongside serving. No request delays the first client connection,
/// and shutting down the helm cancels this loop along with its HTTP request.
pub(crate) async fn watch(store: &HelmStore, events: &FleetEvents, override_path: Option<&OsStr>) {
    let build = farhelm_proto::BUILD_VERSION;
    if !cfg!(test)
        && !farhelm_proto::is_development_build(build)
        && let Err(error) = poll(store, events, filename(override_path), build).await
    {
        tracing::warn!(%error, "could not start announcement checks");
    }
    // The caller selects over this future for the entire serving lifetime.
    std::future::pending().await
}

async fn poll(
    store: &HelmStore,
    events: &FleetEvents,
    name: &str,
    build: &str,
) -> anyhow::Result<()> {
    let running = Version::parse(build).context("invalid helm build version")?;
    let client = client_builder()
        .https_only(true)
        .build()
        .context("creating the announcement HTTP client")?;
    let body_url = format!("{ORIGIN}/{name}");
    let signature_url = format!("{body_url}.minisig");
    let mut interval = tokio::time::interval(CHECK_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        if let Err(error) = check(store, events, async || {
            let Some((body, signature)) =
                fetch_pair(store, &client, &body_url, &signature_url).await?
            else {
                return Ok(None);
            };
            decode(
                &body,
                &signature,
                &running,
                Os::running(),
                SystemTime::now().into(),
                |purpose, body, signature| {
                    verify_signed_metadata(RELEASE_KEY_RING, purpose, body, signature)
                },
            )
            .map(Some)
        })
        .await
        {
            tracing::warn!(%error, "could not check the announcement feed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::PreferencePatch;
    use serde_json::json;

    /// A private fetch fixture injects only authentication. Targeting still runs
    /// against the real schema, clock, release precedence and link validator.
    fn decode_fixture(body: &serde_json::Value) -> anyhow::Result<Vec<Announcement>> {
        let bytes = serde_json::to_vec(body).unwrap();
        let now = DateTime::parse_from_rfc3339("2026-11-23T12:00:00Z")
            .unwrap()
            .to_utc();
        decode(
            &bytes,
            b"test signature",
            &Version::parse("1.2.3-rc.1+test").unwrap(),
            Some(Os::Linux),
            now,
            |purpose, body, signature| {
                assert_eq!(purpose, "farhelm announcements");
                assert_eq!(body, bytes);
                assert_eq!(signature, b"test signature");
                Ok(())
            },
        )
    }

    /// A staging filename cannot change the trust or request boundary.
    #[farhelm_testtrace::test]
    fn only_one_safe_filename_can_replace_the_default() {
        for invalid in [
            "",
            ".hidden",
            "../news.json",
            "x..y",
            "a/b",
            "a\\b",
            "a%2fb",
            "a?query",
            "a#fragment",
            "https://farhelm.io/a",
            "é.json",
        ] {
            assert_eq!(
                filename(Some(OsStr::new(invalid))),
                DEFAULT_PATH,
                "{invalid}"
            );
        }
        assert_eq!(filename(None), DEFAULT_PATH);
        for name in ["announcements-test.json".to_string(), "a".repeat(128)] {
            assert_eq!(filename(Some(OsStr::new(&name))), name);
        }
        assert_eq!(filename(Some(OsStr::new(&"a".repeat(129)))), DEFAULT_PATH);
    }

    /// Targeting is inclusive at both SemVer bounds and exclusive at expiry.
    /// An unsafe link leaves the sender's title, body and detail intact.
    #[farhelm_testtrace::test]
    fn an_active_feed_filters_locally_and_retains_text_from_unsafe_links() {
        let announcements = decode_fixture(&json!({"version": 1, "announcements": [
            {"id": "match", "title": "Title", "body": "Body", "detail": "Details",
             "min_version": "1.2.3-rc.1", "max_version": "1.2.3-rc.1+other",
             "os": ["linux", "macos"], "expires_at": "2026-11-23T07:00:01-05:00",
             "link": {"label": "Read more", "url": "https://farhelm.io:8080/news"}},
            {"id": "any", "title": "Title", "body": "Body",
             "link": {"label": "Read more", "url": "https://farhelm.io/news"}},
            {"id": "mac", "title": "Title", "body": "Body", "os": "macos"},
            {"id": "expired", "title": "Title", "body": "Body", "expires_at": "2026-11-23T12:00:00Z"},
            {"id": "newer", "title": "Title", "body": "Body", "min_version": "1.2.3"},
            {"id": "older", "title": "Title", "body": "Body", "max_version": "1.2.3-rc.0"}
        ]})).unwrap();
        assert_eq!(
            announcements
                .iter()
                .map(|a| a.id.as_str())
                .collect::<Vec<_>>(),
            ["match", "any"]
        );
        assert_eq!(announcements[0].body, "Body");
        assert_eq!(announcements[0].detail.as_deref(), Some("Details"));
        assert!(announcements[0].link.is_none());
        assert_eq!(
            announcements[1].link.as_ref().unwrap().url,
            "https://farhelm.io/news"
        );
        for url in [
            "http://farhelm.io",
            "https://get.farhelm.io",
            "https://farhelm.io.evil.invalid",
            "https://user@farhelm.io",
            "https://@farhelm.io",
            "https:////@farhelm.io",
            "https://\t/@farhelm.io/news",
        ] {
            assert!(safe_link(url).is_none(), "{url}");
        }
    }

    /// Schema and verification failures precede any attempt to enqueue rows.
    /// Exercise the real release verifier for a genuine wrong-purpose envelope
    /// and for bytes changed after signing, using the committed throwaway key.
    #[farhelm_testtrace::test]
    fn unauthenticated_and_unsupported_feeds_are_refused() {
        let key = include_str!("../tests/fixtures/release/test-key.pub")
            .lines()
            .nth(1)
            .unwrap()
            .trim();
        let body = include_bytes!("../tests/fixtures/release/SHA256SUMS");
        let signature = include_bytes!("../tests/fixtures/release/SHA256SUMS.minisig");
        let run = Version::new(1, 0, 0);
        let now = DateTime::<Utc>::from(SystemTime::UNIX_EPOCH);
        verify_signed_metadata(&[key], "farhelm v0.0.3", body, signature)
            .expect("premise: the throwaway signature authenticates the original body");
        for (body, signature, reason) in [
            (body.as_slice(), signature.as_slice(), "different purpose"),
            (
                b"changed".as_slice(),
                signature.as_slice(),
                "does not verify",
            ),
            (
                body.as_slice(),
                b"bad signature".as_slice(),
                "does not verify",
            ),
        ] {
            let error = decode(
                body,
                signature,
                &run,
                None,
                now,
                |purpose, body, signature| verify_signed_metadata(&[key], purpose, body, signature),
            )
            .unwrap_err();
            assert!(error.to_string().contains(reason), "{error:#}");
        }
        assert!(decode_fixture(&json!({"version": 2, "announcements": []})).is_err());
        for targeting in [
            json!({"os": ["linux", "bogus"]}),
            json!({"min_version": "today"}),
            json!({"expires_at": "tomorrow"}),
        ] {
            let mut entry = json!({"id": "one", "title": "Title", "body": "Body"});
            entry
                .as_object_mut()
                .unwrap()
                .extend(targeting.as_object().unwrap().clone());
            assert!(decode_fixture(&json!({"version": 1, "announcements": [entry]})).is_err());
        }
    }

    /// Refuse redirects even though reqwest ordinarily leaves 3xx outside its
    /// error status set. A streamed chunk beyond the cap fails the whole read,
    /// and an opt-out during the body request prevents a second request.
    #[farhelm_testtrace::test]
    async fn transport_distinguishes_absence_redirects_and_oversized_bodies() {
        use axum::{Router, body::Body, response::Redirect, routing::get as route};
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let dir = tempfile::tempdir().unwrap();
        let store = HelmStore::open(&dir.path().join("helm.db")).await.unwrap();
        let settings = store.clone();
        let signatures = Arc::new(AtomicUsize::new(0));
        let signature_requests = Arc::clone(&signatures);
        let router = Router::new()
            .route(
                "/redirect",
                route(|| async { Redirect::temporary("/bounded") }),
            )
            .route("/bounded", route(|| async { "1234" }))
            .route(
                "/large",
                route(|| async {
                    Body::from_stream(futures_util::stream::iter([
                        Ok::<_, std::io::Error>("1234"),
                        Ok("5"),
                    ]))
                }),
            )
            .route(
                "/opt-out",
                route(move || async move {
                    settings
                        .update_preferences(PreferencePatch {
                            check_announcements: Some(Some(false)),
                            ..Default::default()
                        })
                        .await
                        .unwrap();
                    "1234"
                }),
            )
            .route(
                "/signature",
                route(move || async move {
                    signature_requests.fetch_add(1, Ordering::Relaxed);
                    "signature"
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let client = client_builder().no_proxy().build().unwrap();
        assert_eq!(
            get(&client, &format!("{base}/bounded"), 4).await.unwrap(),
            Some(b"1234".to_vec())
        );
        assert_eq!(
            get(&client, &format!("{base}/missing"), 4).await.unwrap(),
            None
        );
        let error = get(&client, &format!("{base}/redirect"), 4)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("307"));
        let error = get(&client, &format!("{base}/large"), 4).await.unwrap_err();
        assert!(error.to_string().contains("exceeds 4 bytes"));
        let signature_url = format!("{base}/signature");
        let pair = fetch_pair(&store, &client, &format!("{base}/bounded"), &signature_url)
            .await
            .unwrap();
        assert_eq!(pair, Some((b"1234".to_vec(), b"signature".to_vec())));
        let pair = fetch_pair(&store, &client, &format!("{base}/opt-out"), &signature_url)
            .await
            .unwrap();
        assert!(pair.is_none());
        assert_eq!(
            store.preferences().await.unwrap().check_announcements,
            Some(false)
        );
        assert_eq!(signatures.load(Ordering::Relaxed), 1);
        server.abort();
        assert!(server.await.unwrap_err().is_cancelled());
    }

    /// Exercise a fresh helm and explicitly re-read the queue and bell across
    /// deliveries and partial dismissals. A concurrent delivery must survive a
    /// dismissal of just the earlier dialog, even when it replaces the reminder.
    #[farhelm_testtrace::test]
    async fn delivery_is_idempotent_and_only_the_last_dismissal_resolves_the_bell() {
        let helm = crate::rest_harness::idle_helm().await;
        let store = &helm.store;
        let events = helm.manager.events();
        assert!(store.announcements().await.unwrap().pending.is_empty());
        assert_eq!(store.preferences().await.unwrap().check_announcements, None);
        let first = |id: &str| Announcement {
            id: id.into(),
            title: "News".into(),
            body: "Message".into(),
            detail: None,
            link: None,
        };
        let before = events.revision();
        check(store, events, async || Ok(Some(vec![first("one")])))
            .await
            .unwrap();
        assert!(
            events.revision() > before,
            "new entries invalidate the UI feed"
        );
        let initial = announcement_notice(store).await;
        assert_eq!(initial.host, None);
        assert!(!initial.read && !initial.resolved);
        let mut changed = first("one");
        changed.body = "A replacement".into();
        check(store, events, async || {
            Ok(Some(vec![changed.clone(), first("two")]))
        })
        .await
        .unwrap();
        assert_eq!(
            store.announcements().await.unwrap().pending[0],
            first("one")
        );
        let next = announcement_notice(store).await;
        assert!(next.id > initial.id && !next.read);
        // An acknowledgement from the panel's previous frame must not hide the new delivery.
        crate::notices::tests::mark(&helm, "read", &[initial.id]).await;
        assert_eq!(announcement_notice(store).await, next);
        store
            .mark_announcements_shown(vec!["one".into()])
            .await
            .unwrap();
        assert!(!announcement_notice(store).await.resolved);
        store
            .mark_announcements_shown(vec!["two".into()])
            .await
            .unwrap();
        let before = events.revision();
        check(store, events, async || Ok(Some(vec![changed])))
            .await
            .unwrap();
        assert_eq!(
            events.revision(),
            before,
            "a duplicate alone leaves the feed alone"
        );
        let current = announcement_notice(store).await;
        assert_eq!(current.id, next.id);
        assert!(current.read && current.resolved);
        assert!(store.announcements().await.unwrap().pending.is_empty());
        check(store, events, async || Ok(Some(vec![first("three")])))
            .await
            .unwrap();
        let current = announcement_notice(store).await;
        assert!(current.id > next.id && !current.read && !current.resolved);
        assert_eq!(store.announcements().await.unwrap().history.len(), 2);
    }

    /// The bell retains one reminder even when several batches await dismissal.
    async fn announcement_notice(store: &HelmStore) -> crate::notices::Notice {
        let notices: Vec<_> = store
            .notices()
            .await
            .unwrap()
            .notices
            .into_iter()
            .filter(|notice| notice.content == crate::notices::NoticeContent::NewAnnouncement)
            .collect();
        let [notice] = notices
            .try_into()
            .expect("exactly one announcement reminder");
        notice
    }

    /// A saved off value prevents even calling the fetcher. If it changes while a
    /// request is outstanding, the completed response cannot add announcements.
    #[farhelm_testtrace::test]
    async fn opt_out_prevents_fetching_and_refuses_an_inflight_result() {
        let dir = tempfile::tempdir().unwrap();
        let store = HelmStore::open(&dir.path().join("helm.db")).await.unwrap();
        let events = FleetEvents::new();
        let set = |value| {
            store.update_preferences(PreferencePatch {
                check_announcements: Some(value),
                ..Default::default()
            })
        };
        set(Some(false)).await.unwrap();
        check(&store, &events, async || {
            panic!("opted-out helm attempted a request")
        })
        .await
        .unwrap();
        set(None).await.unwrap();
        check(&store, &events, async || {
            set(Some(false)).await?;
            Ok(Some(vec![Announcement {
                id: "one".into(),
                title: "News".into(),
                body: "Message".into(),
                detail: None,
                link: None,
            }]))
        })
        .await
        .unwrap();
        assert!(store.announcements().await.unwrap().pending.is_empty());
        assert!(store.notices().await.unwrap().notices.is_empty());
    }
}
