//! Recipe admission returns ordinary host-progress headers, then one JSON result.
//! The request owns its long transport budget through the body read. No retry or
//! progress failure can turn an uncertain environment launch into a second launch.

use super::*;

/// A resource may exist after any transport or decoding failure. Always give
/// the user the existing Hosts recovery surface before another creation attempt.
fn uncertain(reason: impl std::fmt::Display) -> SandboxLaunchError {
    SandboxLaunchError::Unknown(format!(
        "{reason}. Check Hosts for the environment before launching again."
    ))
}

/// A environment launch can outlive its transport. Keep uncertainty distinct from
/// a definite refusal so the composer never claims a running task did not launch.
#[derive(Debug)]
pub(crate) enum SandboxLaunchError {
    Refused(String),
    Unknown(String),
}

impl SandboxLaunchError {
    /// Unknown results require checking Hosts before another creation attempt.
    pub(crate) fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown(_))
    }
}

impl std::fmt::Display for SandboxLaunchError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Refused(text) | Self::Unknown(text) => formatter.write_str(text),
        }
    }
}

/// Ordinary launch encoding with placement deliberately omitted. An older
/// ordinary-create route must never receive this body's recipe intent.
pub(crate) fn sandbox_launch_body(
    cwd: &str,
    agent: CreateAgent<'_>,
    title: &str,
    key: &str,
) -> serde_json::Value {
    let mut body = create_body(cwd, agent, title, key, None, None);
    body.as_object_mut()
        .expect("create body is an object")
        .remove("host");
    body
}

/// Submit captured recipe/place intent without an installed-host selector.
/// Admission exposes the existing provisioning run; the final body decides success.
pub(crate) async fn create_environment_session(
    base: &str,
    target: &farhelm_proto::LaunchTarget,
    confirmed: bool,
    name_kind: Option<farhelm_proto::launcher::TemplateNameKind>,
    request: serde_json::Value,
    admitted: impl FnOnce(HostId, String),
) -> Result<Session, SandboxLaunchError> {
    let recipe = target.recipe();
    let place = target.place();
    let url = format!("{base}/api/environment-sessions");
    let mut deadline = LaunchDeadline::default();
    let response = send_inner_marked(
        client().post(&url).json(&serde_json::json!({
            "recipe": recipe, "place": place, "confirmed": confirmed, "name_kind": name_kind, "request": request,
        })),
        std::time::Duration::from_secs(30 * 60),
        true,
        Some(&mut deadline),
    )
    .await
    .map_err(|error| uncertain(send_error_text(error)))?;
    if !response.status().is_success() {
        return Err(SandboxLaunchError::Refused(
            refusal_text("POST", &url, response).await,
        ));
    }
    let host = response
        .headers()
        .get("x-farhelm-environment-host")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<HostId>().ok())
        .ok_or_else(|| uncertain("The environment admission did not identify its host"))?;
    let run = response
        .headers()
        .get("x-farhelm-environment-run")
        .and_then(|value| value.to_str().ok())
        .filter(|value| {
            value.len() == 36
                && value.bytes().enumerate().all(|(index, byte)| {
                    if [8, 13, 18, 23].contains(&index) {
                        byte == b'-'
                    } else {
                        byte.is_ascii_hexdigit()
                    }
                })
        })
        .ok_or_else(|| uncertain("The environment admission did not identify its setup run"))?
        .to_string();
    admitted(host, run);
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Reply {
        Session {
            session: Box<Session>,
        },
        Error {
            error: String,
            #[serde(default)]
            uncertain: bool,
        },
    }
    match response
        .json::<Reply>()
        .await
        .map_err(|error| uncertain(send_error_text(deadline.error(error))))?
    {
        Reply::Session { session } => Ok(*session),
        Reply::Error { error, uncertain } => Err(if uncertain {
            SandboxLaunchError::Unknown(error)
        } else {
            SandboxLaunchError::Refused(error)
        }),
    }
}

/// How long the page waits for an environment session Delete to answer.
///
/// The helm's own bounds, added up: its wait for this host's turn among
/// environment Deletes (60 seconds), its deadline for the environment's
/// Farhelm to answer (240 seconds), and, after a seal, the destruction of the
/// environment, which the whole-environment Delete below already budgets at
/// 180 seconds. A request still unanswered after that has lost its connection;
/// its outcome is unknown and shown as an ordinary failure, never as "unasked".
const ENVIRONMENT_DELETE_BUDGET: std::time::Duration = std::time::Duration::from_secs(9 * 60);

/// Why an environment session Delete did not report success.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EnvironmentDeleteRefusal {
    /// Nothing was decided: the helm could not ask the environment's own
    /// Farhelm, or got no answer (`ENVIRONMENT_UNASKED_HEADER`). The page
    /// answers this by opening the warned whole-environment confirmation; it
    /// never sends that Delete without the user's answer.
    Unasked(String),
    /// The helm no longer knows the session (404). The session is gone, for
    /// instance because an earlier Delete of the same bulk run sealed and
    /// destroyed its environment, so the caller treats it as deleted.
    Gone,
    /// Any other refusal, busy included, or a transport failure whose outcome
    /// is unknown: shown as the ordinary Delete error.
    Refused(String),
}

impl EnvironmentDeleteRefusal {
    /// Sort a non-success reply by the helm's own markers. Only the helm sets
    /// the unasked header, so neither a supervisor's text nor a provider's can
    /// make the page offer a wider Delete.
    fn classify(status: reqwest::StatusCode, headers: &reqwest::header::HeaderMap) -> RefusalKind {
        if status == reqwest::StatusCode::NOT_FOUND {
            RefusalKind::Gone
        } else if status == reqwest::StatusCode::CONFLICT
            && headers
                .get(farhelm_proto::http::ENVIRONMENT_UNASKED_HEADER)
                .is_some_and(|value| value == farhelm_proto::http::ENVIRONMENT_UNASKED)
        {
            RefusalKind::Unasked
        } else {
            RefusalKind::Refused
        }
    }
}

/// [`EnvironmentDeleteRefusal::classify`]'s answer before the body is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RefusalKind {
    Unasked,
    Gone,
    Refused,
}

/// The request line of an environment session Delete. The two scopes are two
/// distinct opt-ins; a plain DELETE of such a session is refused by the helm,
/// so an older page can never reach either without its confirmation.
fn environment_delete_url(
    base: &str,
    id: &str,
    scope: crate::sandbox_sessions::DeleteScope,
) -> String {
    let query = match scope {
        crate::sandbox_sessions::DeleteScope::Session => "delete_environment_session=true",
        crate::sandbox_sessions::DeleteScope::Whole => "delete_sandbox=true",
    };
    format!("{base}/api/sessions/{}?{query}", encode_path_segment(id))
}

/// Delete a session of a disposable environment under the consent `scope`
/// names (`sandbox_sessions::DeleteScope`).
///
/// `Session` asks the environment's own Farhelm: success carries
/// `environment_remaining: Some(n)` when the environment stayed with `n`
/// other sessions, and `None` when it was destroyed with this, its last
/// session (or an earlier attempt had already deleted only this session).
/// `Whole` destroys the resource with everything in it, independently of the
/// selected session's cached liveness or tab count; its success body carries
/// no checkout facts and cannot earn an archive cue.
///
/// Neither carries the ordinary Delete guard: the environment's Farhelm takes
/// the session as the confirmation described it, and the whole Delete's
/// consent covers every process anyway.
pub(crate) async fn delete_in_environment(
    base: &str,
    id: &str,
    scope: crate::sandbox_sessions::DeleteScope,
) -> Result<farhelm_proto::SessionDeleteOutcome, EnvironmentDeleteRefusal> {
    let url = environment_delete_url(base, id, scope);
    let budget = match scope {
        crate::sandbox_sessions::DeleteScope::Session => ENVIRONMENT_DELETE_BUDGET,
        crate::sandbox_sessions::DeleteScope::Whole => std::time::Duration::from_secs(180),
    };
    let response = send_within(client().delete(&url), budget)
        .await
        .map_err(EnvironmentDeleteRefusal::Refused)?;
    if !response.status().is_success() {
        let kind = EnvironmentDeleteRefusal::classify(response.status(), response.headers());
        let text = refusal_text("DELETE", &url, response).await;
        return Err(match kind {
            RefusalKind::Unasked => EnvironmentDeleteRefusal::Unasked(text),
            RefusalKind::Gone => EnvironmentDeleteRefusal::Gone,
            RefusalKind::Refused => EnvironmentDeleteRefusal::Refused(text),
        });
    }
    match scope {
        // The supervisor's count and any notice about the discarded checkout
        // live in the body. An unreadable body still means the Delete
        // happened; the ordinary decoder keeps that and says what was lost.
        crate::sandbox_sessions::DeleteScope::Session => {
            Ok(delete_reply(response.text().await.map_err(http_error_text)))
        }
        // Whole-resource cleanup completed before these success headers.
        crate::sandbox_sessions::DeleteScope::Whole => {
            let _ = response.bytes().await;
            Ok(farhelm_proto::SessionDeleteOutcome::default())
        }
    }
}

/// The helm owns both phases after admission, so closing a page after Stop
/// cannot abandon the requested Suspend. A failed Stop never freezes peers.
pub(crate) async fn stop_and_suspend(
    base: &str,
    id: &str,
    no_tabs: bool,
    ended: bool,
) -> Result<(), String> {
    let url = format!(
        "{base}/api/sessions/{}/stop-and-suspend",
        encode_path_segment(id)
    );
    let response = send_within(
        client().post(&url).json(&serde_json::json!({
            "refuse_if_tabs_open": no_tabs, "refuse_if_agent_running": ended,
        })),
        std::time::Duration::from_secs(180),
    )
    .await?;
    if !response.status().is_success() {
        return Err(refusal_text("POST", &url, response).await);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An installed host or connection token would bind this recipe launch to
    /// another machine. Both must be absent, rather than null or a local ID.
    #[farhelm_testtrace::test]
    fn sandbox_body_omits_installed_placement_without_losing_command_intent() {
        let command = farhelm_proto::CommandLaunch {
            command: "printf ready".into(),
            yolo: false,
            agent: None,
            resume: None,
        };
        let body = sandbox_launch_body("~", CreateAgent::Command(&command), "work", "intent");
        assert!(body.get("host").is_none());
        assert!(body.get("expected_incarnation").is_none());
        assert_eq!(body["cwd"], "~");
        assert_eq!(body["command"], serde_json::to_value(command).unwrap());
        assert_eq!(body["title"], "work");
    }

    /// The two environment Deletes are distinct opt-ins on the ordinary
    /// session route: the one that asks the environment's Farhelm, and the
    /// warned whole-environment one. Sending the wrong query would either
    /// destroy an environment the user only agreed to maybe destroy, or be
    /// refused as a plain Delete.
    #[farhelm_testtrace::test]
    fn environment_delete_url_names_the_answered_scope() {
        use crate::sandbox_sessions::DeleteScope;
        assert_eq!(
            environment_delete_url("http://h", "a/b", DeleteScope::Session),
            "http://h/api/sessions/a%2Fb?delete_environment_session=true"
        );
        assert_eq!(
            environment_delete_url("http://h", "s1", DeleteScope::Whole),
            "http://h/api/sessions/s1?delete_sandbox=true"
        );
    }

    /// Only the helm's own marker on a 409 means "nothing was decided"; a 409
    /// without it (busy, or a definite refusal from the environment) stays an
    /// ordinary error, so no text can make the page offer a wider Delete. A
    /// 404 means the session is already gone.
    #[farhelm_testtrace::test]
    fn refusals_offer_the_warned_delete_only_on_the_helms_marker() {
        use reqwest::StatusCode;
        use reqwest::header::{HeaderMap, HeaderValue};
        let mut unasked = HeaderMap::new();
        unasked.insert(
            farhelm_proto::http::ENVIRONMENT_UNASKED_HEADER,
            HeaderValue::from_static(farhelm_proto::http::ENVIRONMENT_UNASKED),
        );
        let mut other_value = HeaderMap::new();
        other_value.insert(
            farhelm_proto::http::ENVIRONMENT_UNASKED_HEADER,
            HeaderValue::from_static("something-else"),
        );
        let mut busy = HeaderMap::new();
        busy.insert(
            farhelm_proto::http::HOST_BUSY_HEADER,
            HeaderValue::from_static(farhelm_proto::http::HOST_BUSY_OPERATION),
        );
        let classify = EnvironmentDeleteRefusal::classify;
        assert_eq!(
            classify(StatusCode::CONFLICT, &unasked),
            RefusalKind::Unasked
        );
        assert_eq!(classify(StatusCode::CONFLICT, &busy), RefusalKind::Refused);
        assert_eq!(
            classify(StatusCode::CONFLICT, &other_value),
            RefusalKind::Refused
        );
        assert_eq!(
            classify(StatusCode::CONFLICT, &HeaderMap::new()),
            RefusalKind::Refused
        );
        assert_eq!(
            classify(StatusCode::INTERNAL_SERVER_ERROR, &unasked),
            RefusalKind::Refused
        );
        assert_eq!(
            classify(StatusCode::NOT_FOUND, &HeaderMap::new()),
            RefusalKind::Gone
        );
    }

    /// The real request path, through the protected HTTP funnel: the query
    /// each scope sends, the supervisor's count read from the body, the
    /// helm's "unasked" marker carried with its text, and a vanished session.
    #[cfg(not(target_arch = "wasm32"))]
    #[farhelm_testtrace::test]
    async fn environment_deletes_send_their_scope_and_read_the_helms_answer() {
        use crate::sandbox_sessions::DeleteScope;
        use std::future::Future;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut dom = dioxus::prelude::VirtualDom::new(dioxus::core::VNode::empty);
        dom.rebuild_in_place();
        let unasked_header = format!(
            "{}: {}\r\n",
            farhelm_proto::http::ENVIRONMENT_UNASKED_HEADER,
            farhelm_proto::http::ENVIRONMENT_UNASKED
        );
        let cases = [
            (
                DeleteScope::Session,
                "/api/sessions/s1?delete_environment_session=true",
                "200 OK",
                String::new(),
                r#"{"environment_remaining":2}"#,
            ),
            (
                DeleteScope::Session,
                "/api/sessions/s1?delete_environment_session=true",
                "409 Conflict",
                unasked_header,
                "nothing was deleted",
            ),
            (
                DeleteScope::Session,
                "/api/sessions/s1?delete_environment_session=true",
                "404 Not Found",
                String::new(),
                "session not found",
            ),
            (
                DeleteScope::Whole,
                "/api/sessions/s1?delete_sandbox=true",
                "200 OK",
                String::new(),
                "{}",
            ),
        ];
        let mut results = Vec::new();
        for (scope, path, status, extra, body) in cases {
            let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
                .await
                .unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let expected = format!("DELETE {path} HTTP/1.1\r\n");
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\n{extra}{}: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                crate::skew::BUILD_HEADER,
                crate::skew::CLIENT_BUILD,
                body.len()
            );
            let peer = tokio::spawn(async move {
                tokio::time::timeout(std::time::Duration::from_secs(10), async move {
                    let (mut socket, _) = listener.accept().await.unwrap();
                    let mut request = Vec::new();
                    let mut chunk = [0; 1024];
                    while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                        let read = socket.read(&mut chunk).await.unwrap();
                        assert!(read > 0, "the request must finish its headers");
                        request.extend_from_slice(&chunk[..read]);
                        assert!(request.len() <= 8192, "bounded fixture request");
                    }
                    let request = std::str::from_utf8(&request).unwrap();
                    assert!(request.starts_with(&expected), "{request}");
                    socket.write_all(response.as_bytes()).await.unwrap();
                    socket.shutdown().await.unwrap();
                })
                .await
                .expect("the owned Delete exchange must finish");
            });
            let mut request = Box::pin(delete_in_environment(&base, "s1", scope));
            let result = std::future::poll_fn(|cx| {
                dom.in_scope(dioxus::core::ScopeId::APP, || request.as_mut().poll(cx))
            })
            .await;
            peer.await.unwrap();
            results.push(result);
        }
        let [kept, unasked, gone, whole] = <[_; 4]>::try_from(results).unwrap();
        assert_eq!(kept.unwrap().environment_remaining, Some(2));
        match unasked {
            Err(EnvironmentDeleteRefusal::Unasked(text)) => {
                assert!(text.contains("nothing was deleted"), "{text}");
            }
            other => panic!("expected an unasked refusal, got {other:?}"),
        }
        assert_eq!(gone, Err(EnvironmentDeleteRefusal::Gone));
        assert_eq!(whole, Ok(farhelm_proto::SessionDeleteOutcome::default()));
    }

    /// A transport problem can leave billed work behind. Ordinary create's
    /// safe-retry promise must never be borrowed by a recipe launch.
    #[farhelm_testtrace::test]
    fn uncertain_recipe_errors_point_to_hosts_without_promising_safe_retry() {
        let failure = uncertain("Connection lost");
        assert!(failure.is_unknown());
        let message = failure.to_string();
        assert!(message.contains("Check Hosts"));
        assert!(message.contains("before launching again"));
        assert!(!message.contains("will not start a second"));
    }
}
