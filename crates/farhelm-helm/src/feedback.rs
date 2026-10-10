//! `POST /api/feedback`: the helm sends the UI's feedback submission to the
//! project's feedback endpoint (SPEC.md "Feedback", SPEC_impl.md "Feedback
//! forwarding").
//!
//! This is the helm's one outbound connection that carries user-written
//! content, and it exists only for this route: nothing else calls
//! [`FeedbackForwarder::forward`], no agent request verb reaches it, and the
//! route sits inside the ordinary protected group, so only an authenticated
//! UI can send. The browser and the desktop webview never post to the
//! internet themselves; the helm does it for them, which keeps the one
//! outbound edge in one place.
//!
//! The submission is forwarded exactly as the UI composed and displayed it:
//! [`FeedbackSubmission`] is validated with the caps it shares with the UI
//! and the endpoint, then re-serialized unchanged. The helm adds nothing,
//! trims nothing, stores nothing, and retries nothing. A failure is one
//! plain-text error the dialog shows, and the log records that a send
//! failed and why, never what it said or who sent it.

use crate::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use farhelm_proto::feedback::FeedbackSubmission;
use std::sync::Arc;
use std::time::Duration;

/// The largest request body this route reads: above the largest valid
/// submission even when every character is JSON-escaped (about 52 KiB), and
/// far below axum's 2 MiB default, since the JSON is allocated whole before
/// the caps are checked.
pub(crate) const MAX_BODY_BYTES: usize = 64 * 1024;

/// Where feedback goes: the project's own endpoint, a URL the maintainer
/// controls for as long as the project exists, so the backend behind it can
/// change without an app release.
pub(crate) const FEEDBACK_ENDPOINT: &str = "https://farhelm.io/api/feedback";

/// How long one send may take end to end before the dialog hears it failed.
/// The endpoint bounds its own call to GitHub at 10 seconds, so a slow GitHub
/// shows up as the endpoint's refusal rather than as this timeout.
pub(crate) const FORWARD_TIMEOUT: Duration = Duration::from_secs(15);

/// The outbound half of the route: where to post and with which client.
///
/// Held in [`AppState`] so tests can point a helm at a stand-in server they
/// started on port 0 ([`Self::with_client`]) instead of changing anything
/// global. Production helms always use [`Self::production`]; there is
/// deliberately no flag, configuration key, or environment variable that
/// redirects feedback.
#[derive(Clone)]
pub(crate) struct FeedbackForwarder {
    endpoint: String,
    /// `None` when the client could not be built (a TLS backend failure),
    /// which turns every send into an ordinary failure rather than falling
    /// back to a client without the timeout and redirect policy.
    client: Option<reqwest::Client>,
}

impl FeedbackForwarder {
    /// The production forwarder: [`FEEDBACK_ENDPOINT`] with a client from
    /// [`client_builder`].
    pub(crate) fn production() -> FeedbackForwarder {
        let client = client_builder()
            .build()
            .inspect_err(
                |error| tracing::warn!(%error, "the feedback HTTP client could not be built"),
            )
            .ok();
        FeedbackForwarder {
            endpoint: FEEDBACK_ENDPOINT.to_string(),
            client,
        }
    }

    /// A forwarder that cannot send at all: what every test-built helm
    /// starts with, so a test that did not give it a stand-in can never
    /// reach the production endpoint.
    #[cfg(test)]
    pub(crate) fn offline() -> FeedbackForwarder {
        FeedbackForwarder {
            endpoint: String::new(),
            client: None,
        }
    }

    /// A forwarder for tests: any endpoint and a client built from
    /// [`client_builder`] (typically plus `no_proxy()`, so a loopback
    /// stand-in is reached directly).
    #[cfg(test)]
    pub(crate) fn with_client(endpoint: String, client: reqwest::Client) -> FeedbackForwarder {
        FeedbackForwarder {
            endpoint,
            client: Some(client),
        }
    }

    /// Post one validated submission. `Ok` means the endpoint answered with
    /// success; `Err` is the reason in words the dialog can show after
    /// "Couldn't send feedback:".
    async fn forward(&self, submission: &FeedbackSubmission) -> Result<(), String> {
        let Some(client) = &self.client else {
            return Err("this helm cannot make outbound connections".to_string());
        };
        let response = client
            .post(&self.endpoint)
            .json(submission)
            .send()
            .await
            .map_err(|error| {
                // reqwest's own Display names only the URL; the cause (DNS,
                // TLS, refused, proxy) is in its source chain, which `{:#}`
                // on an anyhow error prints. Neither carries the request
                // body, so this is safe to log and is what an operator needs
                // to diagnose a send that never works.
                let timed_out = error.is_timeout();
                let cause = format!("{:#}", anyhow::Error::new(error));
                tracing::warn!(%cause, "feedback request failed");
                if timed_out {
                    "the feedback service did not answer in time".to_string()
                } else {
                    "the feedback service could not be reached".to_string()
                }
            })?;
        let status = response.status();
        if status.is_success() {
            Ok(())
        } else {
            Err(format!(
                "the feedback service refused it (HTTP {})",
                status.as_u16()
            ))
        }
    }
}

/// The production client's settings, shared with tests so they exercise the
/// same redirect policy rather than a copy of it: bounded by
/// [`FORWARD_TIMEOUT`], following no redirects (a redirect would let whoever
/// answers at the endpoint send the user's text somewhere else, and the
/// endpoint has no reason to issue one), and naming the Farhelm version.
pub(crate) fn client_builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .timeout(FORWARD_TIMEOUT)
        // reqwest's own retry only resends requests a server refused
        // unprocessed, but this route promises to retry nothing at all.
        .retry(reqwest::retry::never())
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("farhelm/", env!("CARGO_PKG_VERSION")))
}

/// `POST /api/feedback` — check the submission against the shared caps and
/// send it on a helm-owned task, so closing the page loses only the reply.
///
/// 204 when the endpoint accepted it; 400 when a field does not fit (the
/// dialog enforces the same caps, so this is a client bug or a hand-made
/// request); 502 when sending failed for any reason. Every refusal body is
/// one plain-text sentence starting "Couldn't send feedback:", which the
/// dialog shows as it is. A JSON body that does not match the type at all is
/// axum's usual 4xx, like every other JSON route here.
pub(crate) async fn send_feedback(
    State(state): State<Arc<AppState>>,
    axum::Json(submission): axum::Json<FeedbackSubmission>,
) -> Response {
    if let Err(reason) = submission.validate() {
        return (
            StatusCode::BAD_REQUEST,
            format!("Couldn't send feedback: {reason}."),
        )
            .into_response();
    }
    // Validation accepts the send; the requesting page no longer owns its lifetime.
    let forwarder = state.feedback.clone();
    match crate::run_owned(async move { forwarder.forward(&submission).await }).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(reason) => {
            // The reason is ours and fixed; the submission's text and
            // contact never reach the log.
            tracing::warn!(%reason, "feedback send failed");
            (
                StatusCode::BAD_GATEWAY,
                format!("Couldn't send feedback: {reason}."),
            )
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    //! The route against a stand-in endpoint on a loopback port the test
    //! picked itself (port 0), through the real router and its auth layer.
    //! Nothing here reaches the network or touches the process environment:
    //! the stand-in's URL is injected through the harness's
    //! [`FeedbackForwarder`], and its client ignores ambient proxy settings.

    use super::*;
    use crate::rest_harness::{FleetBuilder, Harness, HostScript};
    use axum::body::Body;
    use farhelm_proto::feedback::{FEEDBACK_MESSAGE_MAX_CHARS, FeedbackSurface};
    use std::sync::Mutex;
    use tower::ServiceExt;

    /// Text no log line may carry.
    const SECRET_MESSAGE: &str = "private remark about my setup";
    const SECRET_CONTACT: &str = "someone@example.com";

    /// How the stand-in endpoint answers every request.
    #[derive(Clone)]
    enum Answer {
        Status(u16),
        /// Never answers, so only the client's timeout ends the request.
        Hang,
        /// A 307 to another path on the same stand-in, which would record a
        /// second request if the redirect were followed.
        Redirect,
    }

    /// One request the stand-in received: its content type and JSON body.
    type Received = Arc<Mutex<Vec<(String, serde_json::Value)>>>;

    /// A loopback HTTP server standing in for the feedback endpoint, aborted
    /// when dropped so it cannot outlive its test.
    struct StandIn {
        endpoint: String,
        received: Received,
        task: tokio::task::JoinHandle<()>,
    }

    impl Drop for StandIn {
        fn drop(&mut self) {
            self.task.abort();
        }
    }

    async fn stand_in(answer: Answer) -> StandIn {
        let received: Received = Arc::default();
        let record = Arc::clone(&received);
        let app = axum::Router::new().fallback(move |request: axum::extract::Request| {
            let record = Arc::clone(&record);
            let answer = answer.clone();
            async move {
                let content_type = request
                    .headers()
                    .get(axum::http::header::CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or_default()
                    .to_string();
                let bytes = axum::body::to_bytes(request.into_body(), usize::MAX)
                    .await
                    .unwrap_or_default();
                let body = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
                record.lock().unwrap().push((content_type, body));
                match answer {
                    Answer::Status(code) => StatusCode::from_u16(code).unwrap().into_response(),
                    Answer::Hang => std::future::pending::<Response>().await,
                    Answer::Redirect => (
                        StatusCode::TEMPORARY_REDIRECT,
                        [(axum::http::header::LOCATION, "/elsewhere")],
                    )
                        .into_response(),
                }
            }
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        StandIn {
            endpoint: format!("http://{addr}/api/feedback"),
            received,
            task,
        }
    }

    /// A forwarder with the production client settings, minus proxies, and
    /// the given timeout: the production one for every test but the stalled
    /// endpoint's, so a slow loopback on a loaded machine cannot turn into a
    /// spurious timeout.
    fn forwarder(endpoint: String, timeout: Duration) -> FeedbackForwarder {
        let client = client_builder()
            .no_proxy()
            .timeout(timeout)
            .build()
            .unwrap();
        FeedbackForwarder::with_client(endpoint, client)
    }

    async fn helm(endpoint: String) -> Harness {
        helm_with_timeout(endpoint, FORWARD_TIMEOUT).await
    }

    async fn helm_with_timeout(endpoint: String, timeout: Duration) -> Harness {
        FleetBuilder::new()
            .await
            .feedback(forwarder(endpoint, timeout))
            .local(HostScript {
                identity: Some("local-identity".to_string()),
                ..HostScript::default()
            })
            .await
            .start()
            .await
    }

    fn submission() -> FeedbackSubmission {
        FeedbackSubmission {
            message: SECRET_MESSAGE.to_string(),
            contact: Some(SECRET_CONTACT.to_string()),
            version: "0.24.0".to_string(),
            surface: FeedbackSurface::Web,
            os: "Linux".to_string(),
        }
    }

    fn post(body: &impl serde::Serialize) -> axum::http::Request<Body> {
        axum::http::Request::builder()
            .method("POST")
            .uri("/api/feedback")
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(body).unwrap()))
            .unwrap()
    }

    async fn text(response: Response) -> String {
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    /// Production helms send to the project's own endpoint and nowhere else,
    /// with a working client. The URL is the contract with the website
    /// project, so a change to it must be deliberate. Nothing is sent here.
    #[farhelm_testtrace::test]
    fn production_sends_to_the_project_endpoint() {
        let production = FeedbackForwarder::production();
        assert_eq!(production.endpoint, "https://farhelm.io/api/feedback");
        assert_eq!(production.endpoint, FEEDBACK_ENDPOINT);
        assert!(production.client.is_some());
    }

    /// The whole point of the route: the endpoint receives exactly the
    /// submission the dialog displayed, as JSON, and the UI hears success.
    /// "Exactly" is what SPEC.md promises the user ("that display is the
    /// whole submission"), so the received body is compared whole.
    #[farhelm_testtrace::test]
    async fn forwards_the_submission_unchanged_and_answers_204() {
        let endpoint = stand_in(Answer::Status(200)).await;
        let harness = helm(endpoint.endpoint.clone()).await;
        let response = harness.router().oneshot(post(&submission())).await.unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        let received = endpoint.received.lock().unwrap().clone();
        assert_eq!(received.len(), 1);
        assert_eq!(received[0].0, "application/json");
        assert_eq!(received[0].1, serde_json::to_value(submission()).unwrap());
    }

    /// A refusal by the endpoint is a failure the dialog can show, naming
    /// the status, and the helm does not retry.
    #[farhelm_testtrace::test]
    async fn an_endpoint_refusal_is_a_502_naming_its_status() {
        let endpoint = stand_in(Answer::Status(503)).await;
        let harness = helm(endpoint.endpoint.clone()).await;
        let response = harness.router().oneshot(post(&submission())).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(
            text(response).await,
            "Couldn't send feedback: the feedback service refused it (HTTP 503)."
        );
        assert_eq!(endpoint.received.lock().unwrap().len(), 1, "no retry");
    }

    /// An endpoint nobody listens on is an ordinary send failure, not a hang
    /// or a 500.
    ///
    /// The port is held by a socket that is bound but never listening, kept
    /// for the whole test: the kernel refuses connections to it, and no other
    /// test or process can take the port in the meantime (releasing it first
    /// would let something else answer there).
    #[farhelm_testtrace::test]
    async fn an_unreachable_endpoint_is_a_502() {
        let reserved = tokio::net::TcpSocket::new_v4().unwrap();
        reserved.bind("127.0.0.1:0".parse().unwrap()).unwrap();
        let addr = reserved.local_addr().unwrap();
        let harness = helm(format!("http://{addr}/api/feedback")).await;
        let response = harness.router().oneshot(post(&submission())).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(
            text(response).await,
            "Couldn't send feedback: the feedback service could not be reached."
        );
        drop(reserved);
    }

    /// The send is bounded: an endpoint that accepts the request and never
    /// answers ends in the timeout's failure, so the dialog is never left
    /// waiting forever.
    #[farhelm_testtrace::test]
    async fn a_stalled_endpoint_times_out_as_a_502() {
        let endpoint = stand_in(Answer::Hang).await;
        // Short enough to keep the test quick, long enough for the stand-in
        // to have read and recorded the request before the client gives up.
        let harness = helm_with_timeout(endpoint.endpoint.clone(), Duration::from_secs(2)).await;
        let response = harness.router().oneshot(post(&submission())).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(
            text(response).await,
            "Couldn't send feedback: the feedback service did not answer in time."
        );
        assert_eq!(
            endpoint.received.lock().unwrap().len(),
            1,
            "the stand-in never recorded the request within the client timeout: the timeout fired \
             before the request arrived, not while the endpoint stalled"
        );
    }

    /// A redirect is not followed: whoever answers at the endpoint must not
    /// be able to send the user's text on to another place. Uses the
    /// production client settings (`client_builder`), so this pins the real
    /// redirect policy rather than a test copy of it.
    #[farhelm_testtrace::test]
    async fn a_redirect_is_not_followed() {
        let endpoint = stand_in(Answer::Redirect).await;
        let harness = helm(endpoint.endpoint.clone()).await;
        let response = harness.router().oneshot(post(&submission())).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(
            text(response).await,
            "Couldn't send feedback: the feedback service refused it (HTTP 307)."
        );
        assert_eq!(
            endpoint.received.lock().unwrap().len(),
            1,
            "following the redirect would have reached the stand-in a second time"
        );
    }

    /// A submission outside the shared caps is refused with the reason and
    /// never leaves the machine. The dialog enforces the same caps, so this
    /// guards against a client bug or a hand-made request, not a user.
    #[farhelm_testtrace::test]
    async fn an_invalid_submission_is_refused_before_any_outbound_call() {
        let endpoint = stand_in(Answer::Status(200)).await;
        let harness = helm(endpoint.endpoint.clone()).await;
        let blank = FeedbackSubmission {
            message: " \n\t".to_string(),
            ..submission()
        };
        let oversized = FeedbackSubmission {
            message: "x".repeat(FEEDBACK_MESSAGE_MAX_CHARS + 1),
            ..submission()
        };
        for (case, expected) in [
            (blank, "Couldn't send feedback: the message is empty."),
            (
                oversized,
                "Couldn't send feedback: the message is longer than 4000 characters.",
            ),
        ] {
            let response = harness.router().oneshot(post(&case)).await.unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            assert_eq!(text(response).await, expected);
        }
        assert!(endpoint.received.lock().unwrap().is_empty());
    }

    /// A body over the route's limit is refused before it is parsed, and
    /// nothing goes out: the limit is what keeps one request from costing a
    /// 2 MiB allocation, and a later router-level layer could silently undo
    /// it.
    #[farhelm_testtrace::test]
    async fn an_oversized_body_is_refused_before_parsing() {
        let endpoint = stand_in(Answer::Status(200)).await;
        let harness = helm(endpoint.endpoint.clone()).await;
        let oversized = FeedbackSubmission {
            message: "x".repeat(MAX_BODY_BYTES + 1),
            ..submission()
        };
        let response = harness.router().oneshot(post(&oversized)).await.unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert!(endpoint.received.lock().unwrap().is_empty());
    }

    /// Only an authenticated UI can send feedback: the route sits inside the
    /// protected group like every other UI route, so a request without the
    /// device credential is refused and nothing goes out.
    #[farhelm_testtrace::test]
    async fn an_unauthenticated_request_sends_nothing() {
        let endpoint = stand_in(Answer::Status(200)).await;
        let harness = helm(endpoint.endpoint.clone()).await;
        let response = harness
            .unauthenticated_router()
            .oneshot(post(&submission()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(endpoint.received.lock().unwrap().is_empty());
    }

    /// A failed send is logged with its cause, but never with what the user
    /// wrote or how to reach them: the helm's log is not where feedback goes.
    /// The unreachable case shows the cause is the transport's own (the
    /// refused connection), not just the fixed sentence the dialog gets.
    #[farhelm_testtrace::test]
    async fn a_transport_failure_logs_its_cause() {
        let events = crate::test_capture::current();
        let reserved = tokio::net::TcpSocket::new_v4().unwrap();
        reserved.bind("127.0.0.1:0".parse().unwrap()).unwrap();
        let addr = reserved.local_addr().unwrap();
        let harness = helm(format!("http://{addr}/api/feedback")).await;
        let response = harness.router().oneshot(post(&submission())).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        let logged = crate::test_capture::matching(&events, "feedback request failed");
        assert_eq!(logged.len(), 1);
        let cause = logged[0]
            .field("cause")
            .unwrap_or_default()
            .to_ascii_lowercase();
        assert!(
            cause.contains("refused") || cause.contains("connect"),
            "the log names the transport cause, not only the URL: {cause}"
        );
        assert!(!cause.contains(SECRET_MESSAGE) && !cause.contains(SECRET_CONTACT));
        drop(reserved);
    }

    /// A failed send is logged, but never with what the user wrote or how to
    /// reach them: the helm's log is not where feedback goes.
    #[farhelm_testtrace::test]
    async fn a_failed_send_logs_without_the_message_or_contact() {
        let events = crate::test_capture::current();
        let endpoint = stand_in(Answer::Status(500)).await;
        let harness = helm(endpoint.endpoint.clone()).await;
        let response = harness.router().oneshot(post(&submission())).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(
            crate::test_capture::matching(&events, "feedback send failed").len(),
            1
        );
        let leaks = events
            .matching_events(|event| {
                event
                    .fields
                    .values()
                    .chain(event.span_fields.values())
                    .any(|value| value.contains(SECRET_MESSAGE) || value.contains(SECRET_CONTACT))
            })
            .expect("complete trace evidence");
        assert!(leaks.is_empty(), "logged feedback content: {leaks:?}");
    }
}
