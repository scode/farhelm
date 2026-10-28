//! A scripted stand-in for the helm's HTTP API, served on an ephemeral
//! loopback port so the UI's real reqwest client talks to it unmodified.
//!
//! Each test states the replies its surface needs ([`FakeHelm::json`],
//! [`FakeHelm::route`]) and reads back what the UI sent
//! ([`FakeHelm::requests`]). Anything it did not script answers 404 and is
//! recorded as unhandled, so a test that forgot a route fails with the list
//! of what the page actually asked for instead of hanging.
//!
//! This is canned JSON, not the helm: it proves what the UI renders for a
//! given reply and what it sends back, and nothing about whether the helm
//! would really answer that way. The shared golden fixtures under
//! `farhelm-helm/http-contract/` are the place to borrow realistic bodies
//! from when a route has one. Hand-written bodies drift: two of the first
//! ports failed only because a canned reply was not the shape the helm
//! sends (the YOLO refusal's headers, a created session's stored launch),
//! which is why the fixtures check their bodies against the UI's own types.
//!
//! NOTE: the UI's native client is a plain `reqwest::Client`, which honours
//! the process's proxy environment. A proxy configured without a loopback
//! exemption would carry the fake's traffic too.

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, Response, StatusCode, header},
};
use std::sync::{Arc, Mutex};

/// One request the page made, as the fake server received it.
#[derive(Clone, Debug)]
pub struct Recorded {
    pub method: String,
    pub path: String,
    pub query: Option<String>,
    /// The body parsed as JSON, when it was JSON. `None` says nothing about
    /// whether there was a body; [`Recorded::body_len`] does.
    pub json: Option<serde_json::Value>,
    /// The raw body's length in bytes.
    pub body_len: usize,
    /// Whether a scripted route answered it (false means 404 "unhandled").
    pub handled: bool,
}

/// What a scripted route answers.
#[derive(Clone, Debug)]
pub struct Reply {
    pub status: u16,
    pub body: String,
    pub headers: Vec<(String, String)>,
}

impl Reply {
    pub fn json(status: u16, value: serde_json::Value) -> Self {
        Reply {
            status,
            body: value.to_string(),
            headers: Vec::new(),
        }
    }

    /// A plain-text reply, the way the helm sends its refusals: the UI shows
    /// the body verbatim.
    pub fn text(status: u16, body: &str) -> Self {
        Reply {
            status,
            body: body.to_owned(),
            headers: vec![("content-type".into(), "text/plain; charset=utf-8".into())],
        }
    }

    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }
}

type Handler = Box<dyn FnMut(&Recorded) -> Reply + Send>;

struct Route {
    method: String,
    /// Path segments; `*` matches any one segment.
    pattern: Vec<String>,
    handler: Handler,
    /// When set, the reply waits until the test releases it.
    gate: Option<Gate>,
}

/// Holds a route's replies until released, so a test can look at the page
/// while a request is in flight (the browser specs' `routeGate`).
#[derive(Clone)]
pub struct Gate(Arc<tokio::sync::Semaphore>);

impl Default for Gate {
    fn default() -> Self {
        Gate(Arc::new(tokio::sync::Semaphore::new(0)))
    }
}

impl Gate {
    /// Let every held and future reply through.
    pub fn release(&self) {
        self.0.add_permits(tokio::sync::Semaphore::MAX_PERMITS / 2);
    }
}

#[derive(Default)]
struct State {
    routes: Vec<Route>,
    requests: Vec<Recorded>,
}

/// The running fake. Dropping it stops the server task.
pub struct FakeHelm {
    base: String,
    state: Arc<Mutex<State>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for FakeHelm {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn split(path: &str) -> Vec<String> {
    path.trim_matches('/')
        .split('/')
        .map(str::to_owned)
        .collect()
}

fn matches(pattern: &[String], path: &str) -> bool {
    let segments = split(path);
    segments.len() == pattern.len()
        && pattern
            .iter()
            .zip(&segments)
            .all(|(p, s)| p == "*" || p == s)
}

impl FakeHelm {
    /// Bind `127.0.0.1:0` and serve on the current Tokio runtime.
    pub async fn start() -> Self {
        let state = Arc::new(Mutex::new(State::default()));
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("bind the fake helm");
        let base = format!(
            "http://{}",
            listener.local_addr().expect("fake helm address")
        );
        let shared = state.clone();
        let app = Router::new().fallback(move |request: Request<Body>| {
            let state = shared.clone();
            async move { answer(state, request).await }
        });
        let task = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("fake helm serves until dropped");
        });
        FakeHelm { base, state, task }
    }

    /// The origin to hand the UI as its `ApiBase`.
    pub fn base(&self) -> &str {
        &self.base
    }

    /// Answer `method path` with `handler`. A later route for the same
    /// request wins, so a test can override a default it set up earlier.
    pub fn route(
        &self,
        method: &str,
        path: &str,
        handler: impl FnMut(&Recorded) -> Reply + Send + 'static,
    ) {
        let route = Route {
            method: method.to_owned(),
            pattern: split(path),
            handler: Box::new(handler),
            gate: None,
        };
        self.state.lock().unwrap().routes.push(route);
    }

    /// Like [`FakeHelm::route`], but each reply waits for the returned gate.
    /// The request is recorded on arrival, so a test can see it was sent
    /// while its reply is still held.
    pub fn route_held(
        &self,
        method: &str,
        path: &str,
        handler: impl FnMut(&Recorded) -> Reply + Send + 'static,
    ) -> Gate {
        let gate = Gate::default();
        let route = Route {
            method: method.to_owned(),
            pattern: split(path),
            handler: Box::new(handler),
            gate: Some(gate.clone()),
        };
        self.state.lock().unwrap().routes.push(route);
        gate
    }

    /// Answer `method path` with the same JSON every time.
    pub fn json(&self, method: &str, path: &str, status: u16, value: serde_json::Value) {
        let reply = Reply::json(status, value);
        self.route(method, path, move |_| reply.clone());
    }

    /// Every request so far, in arrival order.
    pub fn requests(&self) -> Vec<Recorded> {
        self.state.lock().unwrap().requests.clone()
    }

    /// A reader for the request log that outlives this borrow, so a
    /// [`Page`](super::Page) can quote it in failure messages.
    pub fn requests_handle(&self) -> impl Fn() -> Vec<Recorded> + 'static {
        let state = self.state.clone();
        move || state.lock().unwrap().requests.clone()
    }
}

async fn answer(state: Arc<Mutex<State>>, request: Request<Body>) -> Response<Body> {
    let method = request.method().to_string();
    let path = request.uri().path().to_owned();
    let query = request.uri().query().map(str::to_owned);
    // Bodies here are small JSON documents; the bound only keeps a runaway
    // client from making the fake buffer without limit.
    let bytes = to_bytes(request.into_body(), 1 << 20)
        .await
        .unwrap_or_default();
    let json = serde_json::from_slice(&bytes).ok();
    let mut recorded = Recorded {
        method,
        path,
        query,
        json,
        body_len: bytes.len(),
        handled: false,
    };
    let found = |state: &State| {
        state.routes.iter().rposition(|route| {
            route.method == recorded.method && matches(&route.pattern, &recorded.path)
        })
    };
    let gate = {
        let mut state = state.lock().unwrap();
        let index = found(&state);
        recorded.handled = index.is_some();
        state.requests.push(recorded.clone());
        index.and_then(|index| state.routes[index].gate.clone())
    };
    if let Some(gate) = gate {
        // A closed semaphore cannot happen (the gate is never closed); the
        // permit is returned at once so one release lets every reply through.
        drop(gate.0.acquire().await);
    }
    let reply = {
        let mut state = state.lock().unwrap();
        found(&state).map(|index| (state.routes[index].handler)(&recorded))
    };
    let reply = reply.unwrap_or_else(|| {
        Reply::json(404, serde_json::json!({ "error": format!("unhandled by the fake helm: {}", recorded.path) }))
    });
    // Every helm reply carries its build stamp, and the page compares it with
    // its own build (`skew`). Stamping this crate's version makes the fake a
    // matching helm; a test about skew overrides it with its own header.
    let mut response = Response::builder()
        .status(StatusCode::from_u16(reply.status).expect("scripted status is valid"));
    if !reply
        .headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("content-type"))
    {
        response = response.header(header::CONTENT_TYPE, "application/json");
    }
    if !reply
        .headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case(crate::skew::BUILD_HEADER))
    {
        response = response.header(crate::skew::BUILD_HEADER, crate::skew::CLIENT_BUILD);
    }
    for (name, value) in &reply.headers {
        response = response.header(name, value);
    }
    response
        .body(Body::from(reply.body))
        .expect("scripted response builds")
}
