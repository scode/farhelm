//! Supervisor change hints, end to end: a real supervisor process, a real
//! helm process whose backstop poll is pushed an hour away, and a direct
//! full-authority client making every change.
//!
//! # Why this shape
//!
//! The helm learns about a host's sessions by polling it, and since
//! protocol 33 also by the supervisor's content-free "sessions changed" hint
//! (`farhelm-supervisor`'s `service::hints`). The unit tests on each side pin
//! each half against a scripted peer; this is the one place the two real
//! halves meet. Every change here is made through a separate connection
//! straight to the supervisor, so the helm initiates nothing and hears
//! nothing about it except what the supervisor volunteers, and with the
//! poll an hour out (`--backstop-refresh-secs`), a helm that shows the
//! change within seconds can only have been hinted.
//!
//! NOTE: This does not pin the case that motivated hints, a tab opened and
//! exited between two helm refreshes so that the helm's cached listing
//! never changes. Which of two real processes gets there first cannot be
//! controlled from here; the helm's own
//! `a_hint_refreshes_at_once_and_always_raises_a_feed_event` scripts that
//! boundary exactly.

use crate::harness::*;
use crate::terminal_tabs::wait_for_shell;
use farhelm_proto::TerminalSelector;

/// How long the helm is given to show a hinted change, in seconds. Generous
/// against a loaded machine, and still a small fraction of the hour a poll
/// would take.
const HINTED_WITHIN_SECS: u64 = 15;

/// A full-authority client of the supervisor behind `state`, separate from
/// the helm's own connection.
async fn direct_client(state: &std::path::Path) -> std::sync::Arc<SupervisorClient> {
    let transport = farhelm_supervisor::service::connect(state)
        .await
        .expect("dial the supervisor");
    let (reader, writer) = tokio::io::split(transport);
    SupervisorClient::start(reader, writer)
        .await
        .expect("handshake with the supervisor")
}

/// The tab ids the helm's cached listing shows for `session`, or `None` while
/// it does not list the session at all.
fn listed_tabs(listing: &serde_json::Value, session: &str) -> Option<Vec<String>> {
    let row = listing["sessions"]
        .as_array()
        .expect("sessions is an array")
        .iter()
        .find(|row| row["id"] == session)?;
    Some(
        row["tabs"]
            .as_array()
            .map(|tabs| {
                tabs.iter()
                    .filter_map(|tab| tab["id"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
    )
}

/// Wait until the helm's listing of `session` satisfies `wanted`, failing
/// with `what` after [`HINTED_WITHIN_SECS`]. The budget bounds each request
/// as well as the retries, so a stalled helm fails here rather than hanging
/// the test.
///
/// Each observation pairs the sessions listing with the helm's hosts
/// listing, so a timeout's report of the last one seen also says whether
/// the fixture premise (the local host connected, its refreshes healthy)
/// still held: a missing hint and a broken connection look the same in the
/// sessions listing alone.
async fn until_helm_lists(
    client: &reqwest::Client,
    base: &str,
    session: &str,
    what: &str,
    wanted: impl Fn(&Option<Vec<String>>) -> bool,
) {
    let sessions_url = format!("{base}/api/sessions");
    let hosts_url = format!("{base}/api/hosts");
    wait_for_listing_with(
        || async {
            Ok((
                get_json(client, &sessions_url).await,
                get_json(client, &hosts_url).await,
            ))
        },
        HINTED_WITHIN_SECS,
        &format!("the helm showing {what} with its poll an hour away"),
        |(listing, _hosts)| wanted(&listed_tabs(listing, session)),
    )
    .await;
}

/// Spec: with the helm's backstop poll an hour away, a session created, a tab
/// opened, and that tab's shell exiting, each done through a connection the
/// helm is not party to, all reach the helm's listing within seconds.
///
/// Why: before change hints, everything that happened on a host by itself
/// (a tab whose shell exited above all) reached the helm only on its next
/// poll, and a quick open-and-exit could leave a tab on screen with nothing
/// ever correcting it. This is the path from the supervisor noticing a
/// change to the helm refreshing, with no poll to hide behind.
#[farhelm_testtrace::test]
async fn supervisor_side_changes_reach_the_helm_without_its_poll() {
    let _slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let supervisor = supervisor_process().await;
    let helm = helm_process_with_args(
        supervisor.state.path(),
        None,
        &["--backstop-refresh-secs", "3600"],
    )
    .await;
    let secret = device_secret(supervisor.state.path(), &helm.base).await;
    let client = client_with_secret(&secret);
    // The first poll runs at connect, before the hour starts; after it, only
    // hints refresh this host. Connected alone is not that premise: the
    // helm reports it before its first refresh has finished, and a create
    // made meanwhile could reach the listing through that refresh instead
    // of a hint.
    let hosts_url = format!("{}/api/hosts", helm.base);
    wait_for_listing_with(
        || async { Ok(get_json(&client, &hosts_url).await) },
        REAL_STACK_SETTLE.as_secs(),
        "fixture premise: the helm connected to its local supervisor and finished its first refresh",
        |hosts| {
            hosts["hosts"]
                .as_array()
                .expect("hosts is an array")
                .iter()
                .any(|row| {
                    row["kind"] == "local"
                        && row["state"]["phase"] == "connected"
                        && row["state"]["refresh"]["status"] == "ok"
                })
        },
    )
    .await;

    let direct = direct_client(supervisor.state.path()).await;
    let work = farhelm_teststate::tempdir().expect("work dir");
    let session = direct
        .create_session(
            &work.path().to_string_lossy(),
            &fixture_cmd("fake-agent --script basic"),
            Some("hinted".to_string()),
            80,
            24,
        )
        .await
        .expect("create through the direct connection");
    until_helm_lists(
        &client,
        &helm.base,
        &session.id,
        "the new session",
        |seen| seen.is_some(),
    )
    .await;

    let tab = direct
        .open_tab(&session.id)
        .await
        .expect("open a tab through the direct connection");
    until_helm_lists(&client, &helm.base, &session.id, "the opened tab", |seen| {
        seen.as_ref().is_some_and(|tabs| tabs.contains(&tab.id))
    })
    .await;

    let (channel, replay, mut rx) = direct
        .attach_terminal_live(
            &session.id,
            80,
            24,
            TerminalSelector::Tab { id: tab.id.clone() },
            "hint-lease",
        )
        .await
        .expect("attach the tab");
    let mut seen = replay;
    wait_for_shell(&direct, channel, &mut rx, &mut seen, "READY").await;
    direct.send_input(channel, b"exit\r".to_vec()).await;
    until_helm_lists(
        &client,
        &helm.base,
        &session.id,
        "the exited tab gone",
        |seen| seen.as_ref().is_some_and(|tabs| !tabs.contains(&tab.id)),
    )
    .await;
}
