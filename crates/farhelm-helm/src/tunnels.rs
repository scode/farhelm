//! Desktop-only loopback listeners have session lifetime, not request lifetime.
//!
//! Consent remains authoritative in helm.db. The mutation gate orders publication,
//! revocation and successful deletion; it never covers relay bytes or the OS opener.
//! Accepted sockets resolve the current client again so reconnects recover without
//! redirecting an existing listener onto a different host or known installation.

use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, Mutex, Weak};

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use farhelm_proto::ErrorKind;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::task::{JoinHandle, JoinSet};

use crate::manager::SessionClaim;
use crate::store::{HostId, HostKind, SessionKey};
use crate::{AppState, SupervisorError, http_error};

/// A canonical supervisor target. None asks it to try IPv4 then IPv6 loopback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TunnelTarget {
    pub(crate) address: Option<IpAddr>,
    pub(crate) port: u16,
}

/// Admit only the plan's narrow loopback set after the ordinary raw HTTP(S) checks.
/// No DNS is consulted, and mapped IPv6 and LAN names are deliberately excluded.
pub(crate) fn classify_url(raw: &str) -> anyhow::Result<TunnelTarget> {
    crate::browser::permission_for_url(raw).map_err(invalid)?;
    let url = url::Url::parse(raw)?;
    let address = match url.host() {
        Some(url::Host::Domain(name)) => {
            let name = name.trim_end_matches('.');
            if name != "localhost" && !name.ends_with(".localhost") {
                return Err(invalid("only loopback links can be tunneled"));
            }
            None
        }
        Some(url::Host::Ipv4(ip)) if ip.is_unspecified() => None,
        Some(url::Host::Ipv4(ip)) if ip.is_loopback() => Some(IpAddr::V4(ip)),
        Some(url::Host::Ipv6(ip)) if ip.is_unspecified() => None,
        Some(url::Host::Ipv6(ip)) if ip.is_loopback() => Some(IpAddr::V6(ip)),
        _ => return Err(invalid("only loopback links can be tunneled")),
    };
    let port = url
        .port_or_known_default()
        .filter(|port| *port != 0)
        .ok_or_else(|| invalid("the tunnel needs a nonzero HTTP(S) port"))?;
    Ok(TunnelTarget { address, port })
}

/// Keep all URL components except the endpoint; localhost itself is never opened.
fn rewrite_url(raw: &str, port: u16) -> anyhow::Result<String> {
    let mut url = url::Url::parse(raw)?;
    url.set_host(Some("127.0.0.1"))?;
    url.set_port(Some(port))
        .map_err(|()| invalid("the URL cannot name a tunnel port"))?;
    Ok(url.into())
}

/// Public listener metadata contains no connection authority or remote URL secrets.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct TunnelInfo {
    pub(crate) id: String,
    pub(crate) local_port: u16,
    pub(crate) remote_port: u16,
}

/// The native opener accepted this rewritten URL; destination loading is separate.
#[derive(Serialize)]
pub(crate) struct OpenedTunnel {
    pub(crate) tunnel: TunnelInfo,
    pub(crate) url: String,
}

/// The entry/session pair is retained for reuse, close and accepted-socket routing.
/// A published listener owns its accept task even during a cancelled close request.
struct Entry {
    session: SessionKey,
    owner: SessionClaim,
    target: TunnelTarget,
    info: TunnelInfo,
    cancel: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}

impl Entry {
    /// Close returns only after the accept loop and every accepted relay have ended.
    async fn stop(mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(());
        }
        let _ = (&mut self.task).await;
    }
}

impl Drop for Entry {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// State behind a synchronous lock so destruction can abort tasks without a runtime.
#[derive(Default)]
struct Listeners {
    stopped: bool,
    entries: BTreeMap<String, Entry>,
}

/// The helm owns listeners; tasks retain only Weak application state, avoiding a cycle.
#[derive(Default)]
pub(crate) struct TunnelManager {
    /// External consent and deletion tails hold this through persistence and cleanup.
    pub(crate) mutation: tokio::sync::Mutex<()>,
    listeners: Mutex<Listeners>,
}

impl TunnelManager {
    /// Forget terminated accept loops before exposing or reusing their metadata.
    fn prune(&self) {
        self.listeners
            .lock()
            .expect("tunnel listeners poisoned")
            .entries
            .retain(|_, entry| !entry.task.is_finished());
    }

    /// List local resources without consulting a supervisor or requiring a live host.
    fn list(&self, session: &SessionKey) -> Vec<TunnelInfo> {
        self.prune();
        let mut result: Vec<_> = self
            .listeners
            .lock()
            .expect("tunnel listeners poisoned")
            .entries
            .values()
            .filter(|entry| &entry.session == session)
            .map(|entry| entry.info.clone())
            .collect();
        result.sort_by(|a, b| {
            (a.local_port, a.remote_port, &a.id).cmp(&(b.local_port, b.remote_port, &b.id))
        });
        result
    }

    /// Retain the exact pair's owner for disconnected identity-less sessions with no cache.
    /// Another entry reporting the same suffix cannot lend its retained listener to this list.
    fn retained_host(&self, session: &SessionKey) -> Option<HostId> {
        self.listeners
            .lock()
            .expect("tunnel listeners poisoned")
            .entries
            .values()
            .find(|entry| &entry.session == session)
            .map(|entry| entry.owner.host)
    }

    /// Caller holds mutation through this close and the successful-delete cache tail.
    pub(crate) async fn close_session_locked(&self, session: &SessionKey) {
        let entries = {
            let mut listeners = self.listeners.lock().expect("tunnel listeners poisoned");
            let ids: Vec<_> = listeners
                .entries
                .iter()
                .filter(|(_, e)| &e.session == session)
                .map(|(id, _)| id.clone())
                .collect();
            ids.into_iter()
                .filter_map(|id| listeners.entries.remove(&id))
                .collect::<Vec<_>>()
        };
        for entry in entries {
            entry.stop().await;
        }
    }

    /// Caller holds mutation after durably disabling consent; no creation can slip in.
    pub(crate) async fn close_all_locked(&self) {
        let entries = std::mem::take(
            &mut self
                .listeners
                .lock()
                .expect("tunnel listeners poisoned")
                .entries,
        );
        for entry in entries.into_values() {
            entry.stop().await;
        }
    }

    /// Permanently stop publication, then await all listeners and active relay sockets.
    pub(crate) async fn shutdown(&self) {
        let _mutation = self.mutation.lock().await;
        self.listeners
            .lock()
            .expect("tunnel listeners poisoned")
            .stopped = true;
        self.close_all_locked().await;
    }
}

impl Drop for TunnelManager {
    fn drop(&mut self) {
        // Dropping entries aborts their tasks. Async shutdown is the ordinary path.
        self.listeners
            .get_mut()
            .expect("tunnel listeners poisoned")
            .entries
            .clear();
    }
}

/// Hold the original host and every known installation identity across reconnects.
fn same_owner(expected: &SessionClaim, current: &SessionClaim) -> bool {
    expected.host == current.host
        && expected.entry_uuid == current.entry_uuid
        && expected
            .identity
            .as_ref()
            .is_none_or(|identity| current.identity.as_ref() == Some(identity))
}

/// Bind once to the requested IPv4 port, then ask the kernel for a free alternative.
async fn bind_listener(port: u16) -> std::io::Result<TcpListener> {
    match TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await {
        Ok(listener) => Ok(listener),
        Err(_) => TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await,
    }
}

/// Re-resolve each socket, retaining no application Arc while relay bytes are idle.
async fn accept_loop(
    state: Weak<AppState>,
    listener: TcpListener,
    session: SessionKey,
    owner: SessionClaim,
    target: TunnelTarget,
    mut cancel: oneshot::Receiver<()>,
) {
    let mut relays = JoinSet::new();
    loop {
        tokio::select! {
            biased;
            _ = &mut cancel => break,
            _ = relays.join_next(), if !relays.is_empty() => {},
            accepted = listener.accept() => {
                let Ok((socket, _)) = accepted else { break; };
                let state = state.clone();
                let session = session.clone();
                let owner = owner.clone();
                relays.spawn(async move {
                    let Some(state) = state.upgrade() else { return; };
                    let route = crate::sessions::route_session(&state, &session).await;
                    drop(state);
                    if let Ok((claim, client)) = route && same_owner(&owner, &claim) {
                        let _ = client.relay_tcp(&session.session_id, target.address, target.port, socket).await;
                    }
                });
            }
        }
    }
    drop(listener);
    relays.shutdown().await;
}

/// Validate and publish under current consent, then dispatch outside the mutation gate.
/// Opener failure retains a still-live listener for retry; revocation may close it meanwhile.
pub(crate) async fn open_url(
    state: &Arc<AppState>,
    session: &SessionKey,
    raw: &str,
) -> anyhow::Result<OpenedTunnel> {
    open_from(state, session, raw, None).await
}

/// Bind an approved agent link to its requesting host's exact routed connection.
/// A browser-side route check can precede adoption or reconnect while this call
/// waits for consent serialization; authorization must use our publication route.
/// The asking ID is supervisor-local, so its authenticated origin supplies the entry.
pub(crate) async fn open_agent_url(
    state: &Arc<AppState>,
    origin: crate::agent_requests::AgentOrigin,
    session: &str,
    raw: &str,
) -> anyhow::Result<OpenedTunnel> {
    let key = SessionKey::new(origin.host, session);
    open_from(state, &key, raw, Some(origin)).await
}

/// A terminal gesture authorizes its selected session; an agent grant is host-scoped.
/// Both paths otherwise share the same consent, publication and opener lifecycle.
async fn open_from(
    state: &Arc<AppState>,
    session: &SessionKey,
    raw: &str,
    expected: Option<crate::agent_requests::AgentOrigin>,
) -> anyhow::Result<OpenedTunnel> {
    let opener = state
        .link_opener
        .clone()
        .ok_or_else(|| refusal(ErrorKind::NotFound, "tunnels require the Mac app"))?;
    let target = classify_url(raw)?;
    let info = {
        let _mutation = state.tunnels.mutation.lock().await;
        if state.store.preferences().await?.tunnel_consent != Some(true) {
            return Err(refusal(
                ErrorKind::Conflict,
                "accept the tunnel security warning before opening a tunnel",
            ));
        }
        let (owner, client) = crate::sessions::route_session(state, session).await?;
        if expected.is_some_and(|origin| {
            origin.host != owner.host || origin.connection != client.connection_id()
        }) {
            return Err(refusal(
                ErrorKind::Conflict,
                "the requesting session no longer belongs to this host connection; retry",
            ));
        }
        let host = state
            .store
            .list_hosts()
            .await?
            .into_iter()
            .find(|host| host.id == owner.host)
            .ok_or_else(|| {
                refusal(
                    ErrorKind::NotFound,
                    "the tunnel's host is no longer registered",
                )
            })?;
        if host.kind != HostKind::Ssh {
            return Err(invalid("sessions on this machine do not need a tunnel"));
        }
        state.tunnels.prune();
        let reused = {
            let listeners = state
                .tunnels
                .listeners
                .lock()
                .expect("tunnel listeners poisoned");
            if listeners.stopped {
                return Err(refusal(ErrorKind::Conflict, "the helm is shutting down"));
            }
            if let Some(entry) = listeners
                .entries
                .values()
                .find(|entry| &entry.session == session && entry.target == target)
            {
                if !same_owner(&entry.owner, &owner) {
                    return Err(refusal(
                        ErrorKind::Conflict,
                        "the tunnel's host installation changed; close the tunnel and retry",
                    ));
                }
                Some(entry.info.clone())
            } else {
                None
            }
        };
        match reused {
            Some(info) => info,
            None => {
                let listener = bind_listener(target.port).await?;
                let info = TunnelInfo {
                    id: uuid::Uuid::new_v4().to_string(),
                    local_port: listener.local_addr()?.port(),
                    remote_port: target.port,
                };
                let (cancel, cancelled) = oneshot::channel();
                let task = tokio::spawn(accept_loop(
                    Arc::downgrade(state),
                    listener,
                    session.clone(),
                    owner.clone(),
                    target,
                    cancelled,
                ));
                state
                    .tunnels
                    .listeners
                    .lock()
                    .expect("tunnel listeners poisoned")
                    .entries
                    .insert(
                        info.id.clone(),
                        Entry {
                            session: session.clone(),
                            owner,
                            target,
                            info: info.clone(),
                            cancel: Some(cancel),
                            task,
                        },
                    );
                state.manager.events().bump();
                info
            }
        }
    };
    let url = rewrite_url(raw, info.local_port)?;
    let opening_url = url.clone();
    tokio::task::spawn_blocking(move || opener(&opening_url))
        .await?
        .map_err(|_| anyhow::anyhow!("the default browser could not open the tunneled link"))?;
    Ok(OpenedTunnel { tunnel: info, url })
}

/// A list is a local resource view; availability is independent of consent and connection.
#[derive(Serialize)]
struct TunnelList {
    available: bool,
    host_name: String,
    consent: bool,
    tunnels: Vec<TunnelInfo>,
}

/// List and close stay useful while a host is down; native capability still gates all routes.
pub(crate) async fn list_tunnels(
    State(state): State<Arc<AppState>>,
    Path(session): Path<String>,
) -> Response {
    if state.link_opener.is_none() {
        return StatusCode::NOT_FOUND.into_response();
    }
    let result: anyhow::Result<_> = async {
        let session = crate::sessions::resolve_session_ref(&state, &session).await?;
        let host_id = match crate::sessions::resolve_owner(&state, &session).await {
            Ok((host, _)) => host,
            Err(error) if crate::error_kind(&error) == ErrorKind::NotFound => {
                state.tunnels.retained_host(&session).ok_or(error)?
            }
            Err(error) => return Err(error),
        };
        let host = state
            .store
            .list_hosts()
            .await?
            .into_iter()
            .find(|host| host.id == host_id)
            .ok_or_else(|| {
                refusal(
                    ErrorKind::NotFound,
                    "the tunnel's host is no longer registered",
                )
            })?;
        let available = host.kind == HostKind::Ssh;
        Ok(TunnelList {
            available,
            host_name: crate::aggregate::host_display_name(
                host.kind,
                host.destination.as_deref(),
                host.alias.as_deref(),
            ),
            consent: state.store.preferences().await?.tunnel_consent == Some(true),
            tunnels: if available {
                state.tunnels.list(&session)
            } else {
                Vec::new()
            },
        })
    }
    .await;
    match result {
        Ok(list) => axum::Json(list).into_response(),
        Err(error) => http_error(error),
    }
}

/// The URL is data from the authenticated UI; every security gate is checked again here.
#[derive(Deserialize)]
pub(crate) struct OpenReq {
    url: String,
}

/// Open returns only after native dispatch accepted the rewritten URL.
pub(crate) async fn open_tunnel(
    State(state): State<Arc<AppState>>,
    Path(session): Path<String>,
    axum::Json(request): axum::Json<OpenReq>,
) -> Response {
    if state.link_opener.is_none() {
        return StatusCode::NOT_FOUND.into_response();
    }
    let session = match crate::sessions::resolve_session_ref(&state, &session).await {
        Ok(session) => session,
        Err(error) => return http_error(error),
    };
    match open_url(&state, &session, &request.url).await {
        Ok(opened) => axum::Json(opened).into_response(),
        Err(error) => http_error(error),
    }
}

/// An idempotent local close never borrows another session's listener or needs a live host.
/// The helm owns removal through shutdown and invalidation: dropping a caller
/// while the accept task is ending must not leave other clients' lists stale.
pub(crate) async fn close_tunnel(
    State(state): State<Arc<AppState>>,
    Path((session, id)): Path<(String, String)>,
) -> Response {
    crate::run_owned(async move {
        if state.link_opener.is_none() {
            return StatusCode::NOT_FOUND.into_response();
        }
        let session = match crate::sessions::resolve_session_ref(&state, &session).await {
            Ok(session) => session,
            Err(error) => return http_error(error),
        };
        let _mutation = state.tunnels.mutation.lock().await;
        let entry = {
            let mut listeners = state
                .tunnels
                .listeners
                .lock()
                .expect("tunnel listeners poisoned");
            if listeners
                .entries
                .get(&id)
                .is_some_and(|entry| entry.session == session)
            {
                listeners.entries.remove(&id)
            } else {
                None
            }
        };
        if let Some(entry) = entry {
            entry.stop().await;
            state.manager.events().bump();
        }
        StatusCode::NO_CONTENT.into_response()
    })
    .await
}

/// Keep refusal classification shared with ordinary session REST and agent responses.
fn refusal(kind: ErrorKind, message: &str) -> anyhow::Error {
    anyhow::Error::new(SupervisorError {
        origin: crate::client::ErrorOrigin::Helm,
        kind,
        message: message.to_string(),
    })
}

/// Malformed or ineligible targets are caller-correctable request errors.
fn invalid(message: &str) -> anyhow::Error {
    refusal(ErrorKind::InvalidRequest, message)
}

#[cfg(test)]
mod tests {
    /// Select the fixture's registered SSH entry without inferring ownership from a session suffix.
    fn remote_key(harness: &Harness) -> SessionKey {
        let host = harness
            .manager
            .snapshots()
            .into_iter()
            .find(|host| host.destination.as_deref() == Some("tunnel-fixture"))
            .expect("the fixture has its SSH entry");
        SessionKey::new(host.id, "remote-session")
    }

    /// Capture the known entry's public spelling; a supervisor's bare ID is never parsed here.
    fn reference(harness: &Harness, key: &SessionKey) -> String {
        let host = harness
            .manager
            .snapshots()
            .into_iter()
            .find(|host| host.id == key.host)
            .expect("the fixture entry remains registered");
        format!("{}:{}", host.entry_uuid, key.session_id)
    }

    use super::*;
    use crate::rest_harness::{self, FleetBuilder, Harness, HostScript};
    use axum::body::{Body, to_bytes};
    use axum::http::{Method, Request};
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use farhelm_proto::{ControlMsg, Frame};
    use serde_json::json;
    use tokio::io::{AsyncReadExt, DuplexStream, ReadHalf, WriteHalf};
    use tokio::net::TcpStream;
    use tower::ServiceExt;

    type PeerRead = FrameReader<ReadHalf<DuplexStream>>;
    type PeerWrite = FrameWriter<WriteHalf<DuplexStream>>;

    /// Keep normalized URL classification aligned with the terminal's WHATWG parser.
    /// Wildcards choose fallback; a literal IPv6 target must remain IPv6 at the supervisor.
    #[farhelm_testtrace::test]
    fn classifier_is_narrow_and_canonical() {
        for raw in [
            "http://localhost/",
            "http://localhost./",
            "http://a.localhost./",
            "http://0.0.0.0/",
            "http://[::]/",
        ] {
            assert_eq!(
                classify_url(raw).unwrap(),
                TunnelTarget {
                    address: None,
                    port: 80
                },
                "{raw}"
            );
        }
        for raw in [
            "http://127.0.0.1/",
            "http://127.1/",
            "http://0x7f000001/",
            "http://2130706433/",
        ] {
            assert_eq!(
                classify_url(raw).unwrap().address,
                Some(IpAddr::V4(Ipv4Addr::LOCALHOST)),
                "{raw}"
            );
        }
        assert_eq!(classify_url("https://127.33.4.5/").unwrap().port, 443);
        assert_eq!(
            classify_url("http://[::1]:1234/").unwrap().address,
            Some(IpAddr::V6(std::net::Ipv6Addr::LOCALHOST))
        );
        for raw in [
            "http://example.test/",
            "http://localhost.example/",
            "http://machine/",
            "http://192.168.1.2/",
            "http://0.1.2.3/",
            "http://[::ffff:127.0.0.1]/",
            "http://[fe80::1]/",
            "http://localhost:0/",
            "http://localhost:bad/",
            "http:/localhost/",
            "file:///localhost/",
            "http://localhost/a b",
            "http://localhost/\u{feff}",
        ] {
            assert!(classify_url(raw).is_err(), "{raw:?}");
        }
    }

    /// Port fallback must not discard credentials, paths, queries, fragments or HTTPS.
    #[farhelm_testtrace::test]
    fn rewrite_changes_only_the_endpoint() {
        assert_eq!(
            rewrite_url("https://user:p%40ss@[::1]:5173/a%20b?q=one#part", 43210).unwrap(),
            "https://user:p%40ss@127.0.0.1:43210/a%20b?q=one#part"
        );
    }

    /// A real fleet supplies current owner routing; the opener is observed without a browser.
    async fn remote(opener: crate::LinkOpener, identity: Option<String>) -> (Harness, HostId) {
        let (builder, host) = FleetBuilder::new()
            .await
            .link_opener(opener)
            .ssh(
                "tunnel-fixture",
                HostScript {
                    identity,
                    sessions: vec![rest_harness::session("remote-session", 1)],
                    ..HostScript::default()
                },
            )
            .await;
        let harness = builder.start().await;
        harness.await_refreshed(host).await;
        (harness, host)
    }

    /// Equal supervisor suffixes on separate entries must neither reuse nor close each other's
    /// listeners. Identity-less hosts exercise both pairs before the cache uniqueness retirement.
    #[farhelm_testtrace::test]
    async fn identical_suffixes_keep_tunnel_reuse_lists_and_cleanup_entry_scoped() {
        let script = || HostScript {
            identity: None,
            sessions: vec![rest_harness::session("same-session", 1)],
            ..HostScript::default()
        };
        let (builder, first) = FleetBuilder::new()
            .await
            .link_opener(Arc::new(|_| Ok(())))
            .ssh("first.example", script())
            .await;
        let (builder, second) = builder.ssh("second.example", script()).await;
        let harness = builder.start().await;
        harness.await_refreshed(first).await;
        harness.await_refreshed(second).await;
        let first = SessionKey::new(first, "same-session");
        let second = SessionKey::new(second, "same-session");
        assert_eq!(
            crate::sessions::route_session(&harness.state, &first)
                .await
                .unwrap()
                .0
                .host,
            first.host
        );
        assert_eq!(
            crate::sessions::route_session(&harness.state, &second)
                .await
                .unwrap()
                .0
                .host,
            second.host
        );
        let first_ref = reference(&harness, &first);
        let second_ref = reference(&harness, &second);
        assert_ne!(first_ref, second_ref);
        consent(&harness, json!(true)).await;
        let occupied = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let raw = format!(
            "http://localhost:{}/",
            occupied.local_addr().unwrap().port()
        );
        let one = open_url(&harness.state, &first, &raw).await.unwrap();
        let two = open_url(&harness.state, &second, &raw).await.unwrap();
        assert_ne!(one.tunnel.id, two.tunnel.id);
        assert_ne!(one.tunnel.local_port, two.tunnel.local_port);
        assert_eq!(
            open_url(&harness.state, &first, &raw)
                .await
                .unwrap()
                .tunnel
                .id,
            one.tunnel.id
        );
        for (full, expected) in [(&first_ref, &one.tunnel.id), (&second_ref, &two.tunnel.id)] {
            let response = harness
                .router()
                .oneshot(request(
                    Method::GET,
                    &format!("/api/sessions/{full}/tunnels"),
                    json!(null),
                ))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let body: serde_json::Value =
                serde_json::from_slice(&to_bytes(response.into_body(), 16 * 1024).await.unwrap())
                    .unwrap();
            assert_eq!(body["tunnels"].as_array().unwrap().len(), 1);
            assert_eq!(
                body["tunnels"][0]["id"].as_str().unwrap(),
                expected.as_str()
            );
        }
        let response = close_tunnel(
            State(Arc::clone(&harness.state)),
            Path((second_ref, one.tunnel.id.clone())),
        )
        .await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(harness.state.tunnels.list(&first).len(), 1);
        assert_eq!(harness.state.tunnels.list(&second).len(), 1);
        {
            let _mutation = harness.state.tunnels.mutation.lock().await;
            harness.state.tunnels.close_session_locked(&first).await;
        }
        assert!(harness.state.tunnels.list(&first).is_empty());
        assert_eq!(harness.state.tunnels.list(&second)[0].id, two.tunnel.id);
        let _released = TcpListener::bind((Ipv4Addr::LOCALHOST, one.tunnel.local_port))
            .await
            .unwrap();
        assert!(
            TcpListener::bind((Ipv4Addr::LOCALHOST, two.tunnel.local_port))
                .await
                .is_err()
        );
        harness.state.tunnels.shutdown().await;
    }

    /// Script admission on the real client connection while the fixture owns refresh traffic.
    async fn remote_peer() -> (Harness, HostId, PeerRead, PeerWrite) {
        remote_peer_with_identity(Some("installation".into())).await
    }

    /// Identity-less peers use live owner routing and must recover on reconnect too.
    async fn remote_peer_with_identity(
        identity: Option<String>,
    ) -> (Harness, HostId, PeerRead, PeerWrite) {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (read, write) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(read);
            let mut writer = FrameWriter::new(write);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            (reader, writer)
        });
        let (builder, host) = FleetBuilder::new()
            .await
            .link_opener(Arc::new(|_| Ok(())))
            .ssh(
                "tunnel-fixture",
                HostScript {
                    identity,
                    sessions: vec![rest_harness::session("remote-session", 1)],
                    peer: Some(client_side),
                    ..HostScript::default()
                },
            )
            .await;
        let harness = builder.start().await;
        harness.await_refreshed(host).await;
        let (reader, writer) = peer.await.unwrap();
        (harness, host, reader, writer)
    }

    /// Bound peer observations and ignore credit frames whose ordering is incidental here.
    async fn control(reader: &mut FrameReader<ReadHalf<DuplexStream>>) -> ControlMsg {
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                let frame = reader
                    .read_frame()
                    .await
                    .unwrap()
                    .expect("peer remains alive");
                let message = parse_control(&frame).unwrap();
                if !matches!(message, ControlMsg::TcpRelayAck { .. }) {
                    return message;
                }
            }
        })
        .await
        .expect("expected a tunnel control message")
    }

    /// A byte from the scripted supervisor proves the accepted relay passed admission.
    /// Keeping that socket and peer alive distinguishes cleanup from connection failure.
    async fn live_socket(
        harness: &Harness,
        reader: &mut FrameReader<ReadHalf<DuplexStream>>,
        writer: &mut FrameWriter<WriteHalf<DuplexStream>>,
    ) -> (OpenedTunnel, TcpStream, u32) {
        let occupied = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let port = occupied.local_addr().unwrap().port();
        let opened = open_url(
            &harness.state,
            &remote_key(harness),
            &format!("http://[::1]:{port}/"),
        )
        .await
        .unwrap();
        let mut socket = TcpStream::connect((Ipv4Addr::LOCALHOST, opened.tunnel.local_port))
            .await
            .unwrap();
        let ControlMsg::BeginTcpRelay {
            req_id,
            session_id,
            channel,
            address,
            port: target_port,
        } = control(reader).await
        else {
            panic!("expected admission");
        };
        assert_eq!(session_id, "remote-session");
        assert_eq!(address, Some(IpAddr::V6(std::net::Ipv6Addr::LOCALHOST)));
        assert_eq!(target_port, port);
        writer
            .write_control(&ControlMsg::TcpRelayStarted { req_id, channel })
            .await
            .unwrap();
        writer
            .write_frame(&Frame::data(channel, b"ready".to_vec()))
            .await
            .unwrap();
        let mut ready = [0; 5];
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            socket.read_exact(&mut ready),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(&ready, b"ready");
        (opened, socket, channel)
    }

    /// EOF is observed while the supervisor connection stays connected, not inferred from a map.
    async fn assert_socket_closed(socket: &mut TcpStream) {
        let mut byte = [0];
        assert_eq!(
            tokio::time::timeout(std::time::Duration::from_secs(10), socket.read(&mut byte))
                .await
                .unwrap()
                .unwrap(),
            0
        );
    }

    /// Every request goes through the actual auth and origin middleware, including mutations.
    fn request(method: Method, path: &str, value: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(path)
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(Body::from(value.to_string()))
            .unwrap()
    }

    /// Persist the same acknowledged consent patch that the native UI sends.
    async fn consent(harness: &Harness, value: serde_json::Value) {
        let response = harness
            .router()
            .oneshot(request(
                Method::PUT,
                "/api/preferences",
                json!({"tunnel_consent":value}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            harness.store.preferences().await.unwrap().tunnel_consent,
            value.as_bool()
        );
    }

    /// Decode a bounded response so assertions observe the shipped list contract.
    async fn list(harness: &Harness) -> serde_json::Value {
        let response = harness
            .router()
            .oneshot(request(
                Method::GET,
                &format!(
                    "/api/sessions/{}/tunnels",
                    reference(harness, &remote_key(harness))
                ),
                json!(null),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        serde_json::from_slice(&to_bytes(response.into_body(), 16 * 1024).await.unwrap()).unwrap()
    }

    /// Disabled consent and invalid/local/native-ineligible requests must not publish or open.
    #[farhelm_testtrace::test]
    async fn create_refusals_precede_listener_and_opener() {
        let openings = Arc::new(Mutex::new(Vec::new()));
        let observed = Arc::clone(&openings);
        let opener: crate::LinkOpener = Arc::new(move |url| {
            observed.lock().unwrap().push(url.to_string());
            Ok(())
        });
        let (harness, _) = remote(Arc::clone(&opener), Some("installation".into())).await;
        assert!(!list(&harness).await["consent"].as_bool().unwrap());
        let error = open_url(
            &harness.state,
            &remote_key(&harness),
            "http://localhost:1234/",
        )
        .await
        .err()
        .unwrap();
        assert_eq!(crate::error_kind(&error), ErrorKind::Conflict);
        consent(&harness, json!(true)).await;
        for raw in [
            "http://192.168.1.2/",
            "http://localhost:0/",
            "file:///localhost/",
        ] {
            assert!(
                open_url(&harness.state, &remote_key(&harness), raw)
                    .await
                    .is_err()
            );
        }
        let local = rest_harness::idle_helm_with_link_opener(opener).await;
        let local_key = SessionKey::new(
            local
                .manager
                .snapshots()
                .into_iter()
                .find(|host| host.kind == HostKind::Local)
                .unwrap()
                .id,
            "sess-1",
        );
        consent(&local, json!(true)).await;
        assert_eq!(
            crate::error_kind(
                &open_url(&local.state, &local_key, "http://localhost/")
                    .await
                    .err()
                    .unwrap()
            ),
            ErrorKind::InvalidRequest
        );
        let standalone = rest_harness::idle_helm().await;
        for (method, path, body) in [
            (Method::GET, "/api/sessions/sess-1/tunnels", json!(null)),
            (
                Method::POST,
                "/api/sessions/sess-1/tunnels/open",
                json!({"url":"http://localhost/"}),
            ),
            (
                Method::DELETE,
                "/api/sessions/sess-1/tunnels/missing",
                json!(null),
            ),
        ] {
            assert_eq!(
                standalone
                    .router()
                    .oneshot(request(method, path, body))
                    .await
                    .unwrap()
                    .status(),
                StatusCode::NOT_FOUND
            );
        }
        assert!(harness.state.tunnels.list(&remote_key(&harness)).is_empty());
        assert!(local.state.tunnels.list(&local_key).is_empty());
        assert!(openings.lock().unwrap().is_empty());
    }

    /// Reuse is target based, and a busy preferred port must fall back to a live IPv4 listener.
    /// Closing another session's id must leave that listener alone.
    #[farhelm_testtrace::test]
    async fn fallback_reuse_scoped_close_and_shutdown_release_ports() {
        let (harness, _) = remote(Arc::new(|_| Ok(())), Some("installation".into())).await;
        consent(&harness, json!(true)).await;
        let occupied = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let remote_port = occupied.local_addr().unwrap().port();
        let first = open_url(
            &harness.state,
            &remote_key(&harness),
            &format!("http://localhost:{remote_port}/one"),
        )
        .await
        .unwrap();
        assert_ne!(
            first.tunnel.local_port, remote_port,
            "premise: owned listener reserves the preferred port"
        );
        let second = open_url(
            &harness.state,
            &remote_key(&harness),
            &format!("http://a.localhost:{remote_port}/two?q=1#part"),
        )
        .await
        .unwrap();
        assert_eq!(first.tunnel.id, second.tunnel.id);
        assert!(second.url.ends_with("/two?q=1#part"));
        let local = (Ipv4Addr::LOCALHOST, first.tunnel.local_port);
        assert!(
            TcpListener::bind(local).await.is_err(),
            "published listener must own its port"
        );
        let response = close_tunnel(
            State(Arc::clone(&harness.state)),
            Path((
                reference(
                    &harness,
                    &SessionKey::new(remote_key(&harness).host, "other-session"),
                ),
                first.tunnel.id.clone(),
            )),
        )
        .await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert!(TcpListener::bind(local).await.is_err());
        close_tunnel(
            State(Arc::clone(&harness.state)),
            Path((reference(&harness, &remote_key(&harness)), first.tunnel.id)),
        )
        .await;
        let reclaimed = TcpListener::bind(local).await.unwrap();
        drop(reclaimed);
        assert!(harness.state.tunnels.list(&remote_key(&harness)).is_empty());
        let again = open_url(
            &harness.state,
            &remote_key(&harness),
            &format!("http://localhost:{remote_port}/"),
        )
        .await
        .unwrap();
        harness.state.tunnels.shutdown().await;
        let _reclaimed = TcpListener::bind((Ipv4Addr::LOCALHOST, again.tunnel.local_port))
            .await
            .unwrap();
        assert!(
            open_url(
                &harness.state,
                &remote_key(&harness),
                &format!("http://localhost:{remote_port}/")
            )
            .await
            .is_err()
        );
    }

    /// Closing a tunnel must finish shutdown and announce removal even when its
    /// HTTP caller leaves. A gated task holds a real listener after accepting the
    /// close signal, making the remove/await/publish boundary deterministic rather
    /// than depending on the real accept loop happening to shut down slowly.
    #[farhelm_testtrace::test]
    async fn cancelled_close_still_finishes_shutdown_and_announces_removal() {
        let (harness, _) = remote(Arc::new(|_| Ok(())), Some("installation".into())).await;
        // The fixture's unscripted local host retries independently of SSH.
        // Settle it before attributing fleet revisions to this tunnel alone.
        let local = rest_harness::local_id(&harness.store).await;
        harness
            .await_state(local, |state| {
                matches!(state, crate::manager::HostState::Unreachable { .. })
            })
            .await;
        let session = remote_key(&harness);
        let (owner, _) = crate::sessions::route_session(&harness.state, &session)
            .await
            .unwrap();
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let info = TunnelInfo {
            id: uuid::Uuid::new_v4().to_string(),
            local_port: address.port(),
            remote_port: address.port(),
        };
        let (cancel, cancelled) = oneshot::channel();
        let (stopping, stop_seen) = oneshot::channel();
        let (release, released) = oneshot::channel();
        let task = tokio::spawn(async move {
            let _ = cancelled.await;
            let _ = stopping.send(());
            // Keep the socket alive through the test-controlled shutdown tail.
            // Dropping release on an assertion failure also lets this task end.
            let _ = released.await;
            drop(listener);
        });
        harness
            .state
            .tunnels
            .listeners
            .lock()
            .unwrap()
            .entries
            .insert(
                info.id.clone(),
                Entry {
                    session: session.clone(),
                    owner,
                    target: TunnelTarget {
                        address: None,
                        port: address.port(),
                    },
                    info: info.clone(),
                    cancel: Some(cancel),
                    task,
                },
            );
        assert_eq!(harness.state.tunnels.list(&session)[0].id, info.id);
        assert!(
            TcpListener::bind(address).await.is_err(),
            "fixture owns its live port"
        );
        let mut events = harness.manager.events().subscribe();
        let revision = *events.borrow_and_update();
        let caller = tokio::spawn(harness.router().oneshot(request(
            Method::DELETE,
            &format!(
                "/api/sessions/{}/tunnels/{}",
                reference(&harness, &session),
                info.id
            ),
            json!(null),
        )));
        tokio::time::timeout(std::time::Duration::from_secs(5), stop_seen)
            .await
            .expect("close must reach the listener's shutdown boundary")
            .expect("the published listener must receive cancellation");
        caller.abort();
        assert!(caller.await.unwrap_err().is_cancelled());
        assert!(harness.state.tunnels.list(&session).is_empty());
        assert!(
            TcpListener::bind(address).await.is_err(),
            "shutdown is still gated"
        );
        assert_eq!(
            *events.borrow(),
            revision,
            "removal is announced only after shutdown"
        );
        release
            .send(())
            .expect("the owned shutdown task must still be waiting");
        tokio::time::timeout(std::time::Duration::from_secs(5), events.changed())
            .await
            .expect("close must announce after its HTTP caller disappears")
            .expect("the helm still owns its feed");
        let _reclaimed = TcpListener::bind(address).await.unwrap();
        assert_eq!(*events.borrow(), revision + 1);
    }

    /// A failed OS dispatch leaves the already-authorized listener available for retry.
    #[farhelm_testtrace::test]
    async fn opener_failure_retains_listener_and_revocation_closes_it() {
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let observed = Arc::clone(&calls);
        let (harness, _) = remote(
            Arc::new(move |_| {
                observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Err("fixture refusal".into())
            }),
            Some("installation".into()),
        )
        .await;
        consent(&harness, json!(true)).await;
        let occupied = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let url = format!(
            "http://localhost:{}/",
            occupied.local_addr().unwrap().port()
        );
        assert!(
            open_url(&harness.state, &remote_key(&harness), &url)
                .await
                .is_err()
        );
        let first = harness.state.tunnels.list(&remote_key(&harness));
        assert_eq!(first.len(), 1);
        assert!(
            open_url(&harness.state, &remote_key(&harness), &url)
                .await
                .is_err()
        );
        assert_eq!(
            harness.state.tunnels.list(&remote_key(&harness))[0].id,
            first[0].id
        );
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
        consent(&harness, json!(null)).await;
        assert!(harness.state.tunnels.list(&remote_key(&harness)).is_empty());
        let _reclaimed = TcpListener::bind((Ipv4Addr::LOCALHOST, first[0].local_port))
            .await
            .unwrap();
        assert!(
            open_url(&harness.state, &remote_key(&harness), &url)
                .await
                .is_err()
        );
    }

    /// Revocation cannot wait for the OS opener, which may be blocked outside our control.
    #[farhelm_testtrace::test]
    async fn revocation_closes_while_native_opener_is_blocked() {
        let (entered_tx, entered_rx) = oneshot::channel();
        let entered = Mutex::new(Some(entered_tx));
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let release = Mutex::new(release_rx);
        let (harness, _) = remote(
            Arc::new(move |_| {
                let _ = entered.lock().unwrap().take().unwrap().send(());
                release.lock().unwrap().recv().unwrap();
                Ok(())
            }),
            Some("installation".into()),
        )
        .await;
        consent(&harness, json!(true)).await;
        let occupied = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let raw = format!(
            "http://localhost:{}/",
            occupied.local_addr().unwrap().port()
        );
        let state = Arc::clone(&harness.state);
        let key = remote_key(&harness);
        let opening = tokio::spawn(async move { open_url(&state, &key, &raw).await });
        tokio::time::timeout(std::time::Duration::from_secs(10), entered_rx)
            .await
            .expect("native opener must be reached")
            .unwrap();
        let tunnel = harness
            .state
            .tunnels
            .list(&remote_key(&harness))
            .pop()
            .unwrap();
        // Always release the blocking fixture even if the revocation assertion fails.
        let revoked = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            consent(&harness, json!(false)),
        )
        .await;
        release_tx.send(()).unwrap();
        opening.await.unwrap().unwrap();
        revoked.expect("revocation must not wait for the OS opener");
        assert!(harness.state.tunnels.list(&remote_key(&harness)).is_empty());
        let _reclaimed = TcpListener::bind((Ipv4Addr::LOCALHOST, tunnel.local_port))
            .await
            .unwrap();
    }

    /// Listener lifetime outlasts host connectivity, even without a durable session cache.
    #[farhelm_testtrace::test]
    async fn offline_identityless_list_and_close_remain_local() {
        let (harness, host) = remote(Arc::new(|_| Ok(())), None).await;
        consent(&harness, json!(true)).await;
        let occupied = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let opened = open_url(
            &harness.state,
            &remote_key(&harness),
            &format!(
                "http://localhost:{}/",
                occupied.local_addr().unwrap().port()
            ),
        )
        .await
        .unwrap();
        assert!(
            !harness
                .store
                .has_cached_session(&remote_key(&harness))
                .await
                .unwrap(),
            "premise: identity-less sessions have no durable cache"
        );
        harness.fleet.take_down(host);
        harness
            .await_state(host, |state| {
                matches!(state, crate::manager::HostState::Unreachable { .. })
            })
            .await;
        let listing = list(&harness).await;
        assert_eq!(listing["available"], true);
        assert_eq!(listing["tunnels"][0]["id"], opened.tunnel.id);
        close_tunnel(
            State(Arc::clone(&harness.state)),
            Path((reference(&harness, &remote_key(&harness)), opened.tunnel.id)),
        )
        .await;
        assert!(
            TcpStream::connect((Ipv4Addr::LOCALHOST, opened.tunnel.local_port))
                .await
                .is_err()
        );
    }

    /// Close, both revocation spellings and app shutdown must cancel already-admitted streams.
    /// The peer remains connected after cleanup, so EOF cannot be credited to host teardown.
    #[farhelm_testtrace::test]
    async fn close_revoke_and_shutdown_cancel_active_relays() {
        for boundary in ["close", "false", "null", "shutdown"] {
            let (harness, host, mut reader, mut writer) = remote_peer().await;
            consent(&harness, json!(true)).await;
            let (opened, mut socket, channel) =
                live_socket(&harness, &mut reader, &mut writer).await;
            match boundary {
                "close" => {
                    close_tunnel(
                        State(Arc::clone(&harness.state)),
                        Path((reference(&harness, &remote_key(&harness)), opened.tunnel.id)),
                    )
                    .await;
                }
                "false" => consent(&harness, json!(false)).await,
                "null" => consent(&harness, json!(null)).await,
                "shutdown" => harness.state.tunnels.shutdown().await,
                _ => unreachable!(),
            }
            assert_socket_closed(&mut socket).await;
            assert_eq!(
                control(&mut reader).await,
                ControlMsg::CloseTcpRelay { channel }
            );
            assert!(matches!(
                harness.manager.status(host).unwrap().state,
                crate::manager::HostState::Connected { .. }
            ));
            assert!(
                TcpStream::connect((Ipv4Addr::LOCALHOST, opened.tunnel.local_port))
                    .await
                    .is_err()
            );
            drop(writer);
        }
    }

    /// An explicit failed delete preserves the tunnel; successful teardown closes it and its stream.
    #[farhelm_testtrace::test]
    async fn failed_delete_preserves_and_successful_delete_closes_active_relay() {
        let (harness, _, mut reader, mut writer) = remote_peer().await;
        consent(&harness, json!(true)).await;
        let (opened, mut socket, channel) = live_socket(&harness, &mut reader, &mut writer).await;
        for successful in [false, true] {
            let state = Arc::clone(&harness.state);
            let key = remote_key(&harness);
            let deleting = tokio::spawn(async move {
                crate::sessions::do_delete_session(
                    &state,
                    &key,
                    farhelm_proto::DeleteGuard::Unconditional,
                    None,
                )
                .await
            });
            let ControlMsg::DeleteSession { req_id, .. } = control(&mut reader).await else {
                panic!("expected delete");
            };
            let reply = if successful {
                ControlMsg::SessionDeleted {
                    req_id,
                    notice: None,
                    archived: false,
                }
            } else {
                ControlMsg::Error {
                    req_id,
                    kind: ErrorKind::Conflict,
                    message: "fixture refusal".into(),
                    create_unaccepted: false,
                }
            };
            writer.write_control(&reply).await.unwrap();
            assert_eq!(deleting.await.unwrap().is_ok(), successful);
            if !successful {
                assert_eq!(
                    harness.state.tunnels.list(&remote_key(&harness))[0].id,
                    opened.tunnel.id
                );
                writer
                    .write_frame(&Frame::data(channel, vec![7]))
                    .await
                    .unwrap();
                let mut byte = [0];
                tokio::time::timeout(
                    std::time::Duration::from_secs(10),
                    socket.read_exact(&mut byte),
                )
                .await
                .unwrap()
                .unwrap();
                assert_eq!(
                    byte,
                    [7],
                    "failed deletion must keep the actual stream alive"
                );
            }
        }
        assert_socket_closed(&mut socket).await;
        assert_eq!(
            control(&mut reader).await,
            ControlMsg::CloseTcpRelay { channel }
        );
        assert!(harness.state.tunnels.list(&remote_key(&harness)).is_empty());
        assert!(
            !harness
                .store
                .has_cached_session(&remote_key(&harness))
                .await
                .unwrap()
        );
    }

    /// Replace's successful delete tail must close only the old session's active tunnel.
    /// The new session remains routable and inherits the old session's pin.
    #[farhelm_testtrace::test]
    async fn replacement_closes_old_tunnel_after_the_new_session_exists() {
        let (harness, host, mut reader, mut writer) = remote_peer().await;
        consent(&harness, json!(true)).await;
        harness
            .store
            .set_pinned(&crate::store::SessionKey::new(host, "remote-session"), true)
            .await
            .unwrap();
        let (opened, mut socket, channel) = live_socket(&harness, &mut reader, &mut writer).await;
        let state = Arc::clone(&harness.state);
        let key = remote_key(&harness);
        let replacing = tokio::spawn(async move {
            crate::sessions::do_replace_session(
                &state,
                &key,
                None,
                None,
                farhelm_proto::DeleteGuard::Unconditional,
                false,
            )
            .await
        });
        let ControlMsg::CreateSession { req_id, .. } = control(&mut reader).await else {
            panic!("expected replacement create");
        };
        let created = rest_harness::session("replacement-session", 2);
        harness
            .fleet
            .edit(host, |script| script.sessions.push(created.clone()));
        writer
            .write_control(&ControlMsg::SessionCreated {
                req_id,
                session: created,
            })
            .await
            .unwrap();
        let ControlMsg::DeleteSession {
            req_id, session_id, ..
        } = control(&mut reader).await
        else {
            panic!("expected replacement delete");
        };
        assert_eq!(session_id, "remote-session");
        assert_eq!(
            harness.state.tunnels.list(&remote_key(&harness))[0].id,
            opened.tunnel.id,
            "creation alone is not deletion"
        );
        harness.fleet.edit(host, |script| {
            script
                .sessions
                .retain(|session| session.id != "remote-session")
        });
        writer
            .write_control(&ControlMsg::SessionDeleted {
                req_id,
                notice: None,
                archived: false,
            })
            .await
            .unwrap();
        assert_eq!(
            replacing.await.unwrap().unwrap().session.id,
            "replacement-session"
        );
        assert_socket_closed(&mut socket).await;
        assert_eq!(
            control(&mut reader).await,
            ControlMsg::CloseTcpRelay { channel }
        );
        assert!(harness.state.tunnels.list(&remote_key(&harness)).is_empty());
        assert!(
            harness
                .store
                .has_cached_session(&crate::store::SessionKey::new(host, "replacement-session"))
                .await
                .unwrap()
        );
        assert!(
            harness
                .store
                .pinned_sessions(&[crate::store::SessionKey::new(host, "replacement-session")])
                .await
                .unwrap()
                .contains(&crate::store::SessionKey::new(host, "replacement-session"))
        );
    }

    /// The preferred local port is used when free; manager destruction has no strong-state cycle.
    #[farhelm_testtrace::test]
    async fn free_preferred_port_and_drop_fallback() {
        let (harness, _) = remote(Arc::new(|_| Ok(())), Some("installation".into())).await;
        consent(&harness, json!(true)).await;
        let reservation = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let port = reservation.local_addr().unwrap().port();
        drop(reservation);
        let opened = open_url(
            &harness.state,
            &remote_key(&harness),
            &format!("http://localhost:{port}/"),
        )
        .await
        .unwrap();
        assert_eq!(opened.tunnel.local_port, port);
        let weak = Arc::downgrade(&harness.state);
        drop(harness);
        assert!(
            weak.upgrade().is_none(),
            "accept task must not retain AppState"
        );
        // Rebinding observes actual listener release after async cancellation;
        // the bounded retry yields only, without inventing a readiness sleep.
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                match TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await {
                    Ok(listener) => break listener,
                    Err(_) => tokio::task::yield_now().await,
                }
            }
        })
        .await
        .expect("Drop must release the listener");
    }

    /// A reconnect may change connection incarnation but never a known install or host.
    #[farhelm_testtrace::test]
    fn ownership_checks_preserve_identityless_recovery_without_retargeting() {
        let original = SessionClaim {
            host: 1,
            entry_uuid: "00000000-0000-4000-8000-000000000001".into(),
            incarnation: 1,
            identity: Some("one".into()),
        };
        assert!(same_owner(
            &original,
            &SessionClaim {
                incarnation: 2,
                ..original.clone()
            }
        ));
        assert!(!same_owner(
            &original,
            &SessionClaim {
                identity: Some("two".into()),
                ..original.clone()
            }
        ));
        assert!(!same_owner(
            &original,
            &SessionClaim {
                identity: None,
                ..original.clone()
            }
        ));
        assert!(!same_owner(
            &original,
            &SessionClaim {
                host: 2,
                ..original.clone()
            }
        ));
        let unknown = SessionClaim {
            identity: None,
            ..original
        };
        assert!(same_owner(
            &unknown,
            &SessionClaim {
                incarnation: 2,
                ..unknown.clone()
            }
        ));
    }

    /// A retained listener must resolve the replacement client for each socket,
    /// including an identity-less host whose session ownership lives only in memory.
    #[farhelm_testtrace::test]
    async fn reconnect_uses_the_new_client_for_known_and_identityless_hosts() {
        for identity in [Some("installation".to_string()), None] {
            let (harness, host, mut reader, mut writer) = remote_peer_with_identity(identity).await;
            consent(&harness, json!(true)).await;
            let (opened, mut old_socket, _) = live_socket(&harness, &mut reader, &mut writer).await;
            harness.fleet.take_down(host);
            harness
                .await_state(host, |state| {
                    matches!(state, crate::manager::HostState::Unreachable { .. })
                })
                .await;
            assert_socket_closed(&mut old_socket).await;
            let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
            let peer = tokio::spawn(async move {
                let (read, write) = tokio::io::split(peer_side);
                let mut reader = FrameReader::new(read);
                let mut writer = FrameWriter::new(write);
                handshake(&mut reader, &mut writer, "supervisor")
                    .await
                    .unwrap();
                (reader, writer)
            });
            harness.fleet.edit(host, |script| {
                script.reachable = true;
                script.peer = Some(client_side);
            });
            harness.manager.retry_now(host).await.unwrap();
            harness.await_refreshed(host).await;
            let (mut new_reader, mut new_writer) = peer.await.unwrap();
            let reused = open_url(
                &harness.state,
                &remote_key(&harness),
                &format!("http://[::1]:{}/again", opened.tunnel.remote_port),
            )
            .await
            .unwrap();
            assert_eq!(reused.tunnel.id, opened.tunnel.id);
            let mut socket = TcpStream::connect((Ipv4Addr::LOCALHOST, opened.tunnel.local_port))
                .await
                .unwrap();
            let ControlMsg::BeginTcpRelay {
                req_id, channel, ..
            } = control(&mut new_reader).await
            else {
                panic!("replacement client must receive admission");
            };
            new_writer
                .write_control(&ControlMsg::TcpRelayStarted { req_id, channel })
                .await
                .unwrap();
            new_writer
                .write_frame(&Frame::data(channel, b"new".to_vec()))
                .await
                .unwrap();
            let mut bytes = [0; 3];
            tokio::time::timeout(
                std::time::Duration::from_secs(10),
                socket.read_exact(&mut bytes),
            )
            .await
            .unwrap()
            .unwrap();
            assert_eq!(&bytes, b"new");
            harness.state.tunnels.shutdown().await;
            assert_socket_closed(&mut socket).await;
        }
    }

    /// A failed persistence operation must not report success or revoke live resources.
    /// The old consent and actual byte stream survive until a later acknowledged write.
    #[farhelm_testtrace::test]
    async fn failed_consent_write_preserves_authorization_and_active_stream() {
        let (harness, _, mut reader, mut writer) = remote_peer().await;
        consent(&harness, json!(true)).await;
        let (opened, mut socket, channel) = live_socket(&harness, &mut reader, &mut writer).await;
        harness.store.connection_for_test().call("install preference refusal", |conn| {
            conn.execute_batch("CREATE TRIGGER refuse_consent BEFORE UPDATE OF tunnel_consent ON preferences BEGIN SELECT RAISE(ABORT, 'fixture consent refusal'); END;")?;
            Ok(())
        }).await.unwrap();
        let response = harness
            .router()
            .oneshot(request(
                Method::PUT,
                "/api/preferences",
                json!({"tunnel_consent":false}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            harness.store.preferences().await.unwrap().tunnel_consent,
            Some(true)
        );
        assert_eq!(
            harness.state.tunnels.list(&remote_key(&harness))[0].id,
            opened.tunnel.id
        );
        writer
            .write_frame(&Frame::data(channel, vec![9]))
            .await
            .unwrap();
        let mut byte = [0];
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            socket.read_exact(&mut byte),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(byte, [9]);
        harness
            .store
            .connection_for_test()
            .call("remove preference refusal", |conn| {
                conn.execute_batch("DROP TRIGGER refuse_consent;")?;
                Ok(())
            })
            .await
            .unwrap();
        consent(&harness, json!(false)).await;
        assert_socket_closed(&mut socket).await;
    }

    /// A create parked behind revocation must read consent after it acquires the gate.
    /// Otherwise it can publish a new listener using the authorization just revoked.
    #[farhelm_testtrace::test]
    async fn creation_waiting_on_mutation_gate_cannot_use_stale_consent() {
        use std::future::Future;
        let (harness, _) = remote(
            Arc::new(|_| panic!("revoked create must not open")),
            Some("installation".into()),
        )
        .await;
        consent(&harness, json!(true)).await;
        let gate = harness.state.tunnels.mutation.lock().await;
        let key = remote_key(&harness);
        let opening = open_url(&harness.state, &key, "http://localhost/");
        tokio::pin!(opening);
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        assert!(opening.as_mut().poll(&mut context).is_pending());
        harness
            .store
            .update_preferences(serde_json::from_value(json!({"tunnel_consent":false})).unwrap())
            .await
            .unwrap();
        drop(gate);
        let error = opening.await.err().unwrap();
        assert_eq!(crate::error_kind(&error), ErrorKind::Conflict);
        assert!(harness.state.tunnels.list(&remote_key(&harness)).is_empty());
    }

    /// Dropping the HTTP caller must not cancel a revocation already handed to
    /// its owned task. The admitted stream and host stay alive until that task
    /// crosses the gate, commits false and finishes its resource cleanup.
    #[farhelm_testtrace::test]
    async fn cancelled_consent_handler_still_commits_and_closes_active_relay() {
        use std::future::Future;
        let (harness, host, mut reader, mut writer) = remote_peer().await;
        consent(&harness, json!(true)).await;
        let (opened, mut socket, channel) = live_socket(&harness, &mut reader, &mut writer).await;

        let gate = harness.state.tunnels.mutation.lock().await;
        let mut caller = Box::pin(crate::preferences::put_preferences(
            State(Arc::clone(&harness.state)),
            axum::Json(serde_json::from_value(json!({"tunnel_consent":false})).unwrap()),
        ));
        // The handler has no await before spawning its consent transaction.
        // Its first Pending therefore means ownership has crossed to that task;
        // the gate prevents it from reaching persistence before cancellation.
        {
            let mut context = std::task::Context::from_waker(std::task::Waker::noop());
            assert!(caller.as_mut().poll(&mut context).is_pending());
        }
        drop(caller);
        assert_eq!(
            harness.store.preferences().await.unwrap().tunnel_consent,
            Some(true)
        );
        assert_eq!(
            harness.state.tunnels.list(&remote_key(&harness))[0].id,
            opened.tunnel.id
        );
        writer
            .write_frame(&Frame::data(channel, vec![11]))
            .await
            .unwrap();
        let mut byte = [0];
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            socket.read_exact(&mut byte),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(byte, [11], "the held gate must preserve the actual stream");

        drop(gate);
        // EOF is the completion oracle: cleanup follows the successful write.
        assert_socket_closed(&mut socket).await;
        assert_eq!(
            harness.store.preferences().await.unwrap().tunnel_consent,
            Some(false)
        );
        assert!(harness.state.tunnels.list(&remote_key(&harness)).is_empty());
        assert_eq!(
            control(&mut reader).await,
            ControlMsg::CloseTcpRelay { channel }
        );
        assert!(matches!(
            harness.manager.status(host).unwrap().state,
            crate::manager::HostState::Connected { .. }
        ));
        assert!(
            TcpStream::connect((Ipv4Addr::LOCALHOST, opened.tunnel.local_port))
                .await
                .is_err()
        );
    }

    /// A grant from host A cannot become a tunnel to host B while creation
    /// waits behind consent serialization. Both connections remain alive;
    /// the refusal must follow the actual publication route, not host loss.
    #[farhelm_testtrace::test]
    async fn agent_origin_cannot_follow_session_reassignment_while_waiting_for_admission() {
        use std::future::Future;
        let opened = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let observed = opened.clone();
        let (builder, first) = FleetBuilder::new()
            .await
            .link_opener(Arc::new(move |_| {
                observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            }))
            .ssh(
                "first.example",
                HostScript {
                    identity: None,
                    sessions: vec![rest_harness::session("remote-session", 1)],
                    ..HostScript::default()
                },
            )
            .await;
        let (builder, second) = builder
            .ssh(
                "second.example",
                HostScript {
                    identity: None,
                    ..HostScript::default()
                },
            )
            .await;
        let harness = builder.start().await;
        harness.await_refreshed(first).await;
        harness.await_refreshed(second).await;
        consent(&harness, json!(true)).await;
        let key = SessionKey::new(first, "remote-session");
        let (owner, client) = crate::sessions::route_session(&harness.state, &key)
            .await
            .unwrap();
        assert_eq!(owner.host, first);
        let origin = crate::agent_requests::AgentOrigin {
            host: first,
            connection: client.connection_id(),
        };
        assert!(
            crate::browser::tunnel_origin_matches(&harness.state, origin, &key)
                .await
                .unwrap()
        );

        let gate = harness.state.tunnels.mutation.lock().await;
        let mut opening = Box::pin(open_agent_url(
            &harness.state,
            origin,
            "remote-session",
            "http://localhost:5173/",
        ));
        {
            let mut context = std::task::Context::from_waker(std::task::Waker::noop());
            assert!(opening.as_mut().poll(&mut context).is_pending());
        }
        let mut revisions = harness.manager.events().subscribe();
        harness.fleet.edit(first, |script| script.sessions.clear());
        harness.fleet.edit(second, |script| {
            script
                .sessions
                .push(rest_harness::session("remote-session", 2))
        });
        harness.manager.refresh_now(first);
        harness.manager.refresh_now(second);
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                // Pair routing no longer lets the second host's presence prove the first
                // has withdrawn its own session. Establish both sides of the movement.
                let first_withdrawn = crate::sessions::route_session(&harness.state, &key)
                    .await
                    .is_err_and(|error| crate::error_kind(&error) == ErrorKind::NotFound);
                if first_withdrawn
                    && let Ok((claim, _)) = crate::sessions::route_session(
                        &harness.state,
                        &SessionKey::new(second, "remote-session"),
                    )
                    .await
                    && claim.host == second
                {
                    break;
                }
                revisions.changed().await.unwrap();
            }
        })
        .await
        .expect("the first entry must withdraw the suffix and the second must report it");
        assert!(
            crate::agent_requests::origin_is_live(&harness.state, origin),
            "the old connection must still be alive"
        );
        drop(gate);
        assert_eq!(
            crate::error_kind(
                &opening
                    .await
                    .err()
                    .expect("the old grant cannot authorize host B")
            ),
            ErrorKind::NotFound
        );
        assert!(harness.state.tunnels.list(&key).is_empty());
        assert_eq!(opened.load(std::sync::atomic::Ordering::SeqCst), 0);
    }
}
