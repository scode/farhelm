//! `/api/ui` — the UI channel: one ordered WebSocket per GUI, carrying its
//! commands, the helm's replies, and the state the helm pushes.
//!
//! The wire is `farhelm_proto::ui_channel`'s; this module is the helm's side
//! of it. SPEC_impl.md's "GUI architecture" section states the rules, and
//! SPEC.md's "Losing the connection to the helm" and "One GUI at a time" the
//! product behavior they serve. The GUI may treat the channel as a stream that
//! never loses or reorders anything, because three things the old GUI had to
//! defend against one action at a time cannot happen here:
//!
//! - **Commands are admitted in receive order.** [`run`] starts a command only
//!   after the previous one replied. Without that, a quick launch could
//!   overtake the template save sent just before it, which the old GUI fenced
//!   by hand. Heartbeats bypass the queue, so a slow command never makes a live
//!   channel look dead.
//! - **There are no reads to race.** The GUI never polls. After every command
//!   and every fleet feed bump, the channel re-runs each pushed collection's
//!   existing GET handler in process and sends what differs from what it last
//!   sent THIS channel. So "an older list read brought back a deleted session"
//!   has no reads to happen in.
//! - **Changes come before the reply.** The re-projection after a command runs
//!   before its reply is sent, so a GUI that sees "done" already holds the
//!   result and needs no optimistic update.
//!
//! Commands are not a second API. Each one is the HTTP request it names, run
//! through the helm's own router ([`crate::api_router`]) with the channel's own
//! credential, so the same handlers, validation, authentication and request
//! ownership apply: work behind `own_state_change` finishes even if the
//! channel goes away, and the cancellable routes are dropped with it, exactly
//! as for an HTTP caller that disconnects. A reply relays the routed
//! response's status and body, plus the helm-only headers it set as markers
//! ([`markers`]), found the same way for every route.
//!
//! The channel is also the seat ([`UiSeat`]): a helm serves one GUI, and a new
//! channel displaces the old one.
//!
//! ## The open session rides outside the query
//!
//! The session list is this channel's query, so a host or status filter, or
//! the listing cap, can leave out the session the GUI has open, while its pane,
//! header and Rename still need that session's current state. The query names
//! the open session (`SessionQuery::selected`), and when the list's own read
//! leaves it out, [`Channel::read`] reads that one session too and carries it
//! as an extra entry, named in the metadata
//! (`CollectionMeta::selected_outside_query`), kept out of the order and
//! counted nowhere. It is diffed like any entry, so it arrives, changes and
//! goes (the selection moves, the filter starts to match it, it is deleted)
//! through the ordinary push. When its own read finds nothing and was cut
//! (its host lists only its newest sessions, up to the cap), the metadata
//! says so (`CollectionMeta::selected_cut`): the session may be one the cap
//! left out rather than one that was deleted. The counts stay the query's
//! own, since the query's list may have been read whole.
//!
//! ## What the helm owes the GUI's watchdog
//!
//! The GUI treats any command left unanswered for too long as a broken
//! channel and resets (SPEC.md). That only works if handlers answer promptly,
//! which is each route's business, not this module's: a route that waits on a
//! host bounds that wait and answers "the host timed out" as an ordinary reply
//! (`crate::sessions::HOST_REPLY_BOUND`), and genuinely long work answers
//! "accepted" (202) and reports its outcome as pushed state.

use crate::AppState;
use axum::body::{Body, Bytes, to_bytes};
use axum::extract::{State, WebSocketUpgrade, ws};
use axum::http::{HeaderMap, HeaderValue, Method, Request, StatusCode, header};
use axum::response::IntoResponse;
use farhelm_proto::ui_channel::{
    ANNOUNCEMENTS_KEY, Availability, ClientMessage, Collection, CollectionChanges, CollectionMeta,
    CollectionState, Command, HelmMarker, KeyedEntry, ListCounts, PREFERENCES_KEY, Reply,
    SESSION_QUERY_PATH, ServerMessage, SessionQuery,
};
// `Method` alone is axum's, the router's method; this is the command's.
use farhelm_proto::ui_channel::Method as WireMethod;
use serde_json::Value;
use std::collections::{BTreeMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, watch};
use tower::ServiceExt;

// ---------------------------------------------------------------------------
// Bounds
// ---------------------------------------------------------------------------

/// The largest message the GUI may send. Commands are small (the largest
/// bodies are template and credential saves, bounded far lower by their own
/// routes); file contents never travel here. The bound only keeps a broken
/// peer from making the helm buffer an unbounded message on its way out.
const MAX_CLIENT_MESSAGE: usize = 4 * 1024 * 1024;

/// The largest reply body relayed to the GUI. Above it the command answers
/// 500 rather than holding the whole body: nothing the GUI reads through a
/// command is near this size (bulk data has its own HTTP and terminal paths).
const MAX_REPLY_BODY: usize = 32 * 1024 * 1024;

/// How long one outgoing message may take to reach the GUI before the channel
/// is abandoned. Same reasoning as the event feed's write deadline: a peer
/// that stopped reading would otherwise park the writer for good.
const WRITE_DEADLINE: Duration = Duration::from_secs(10);

/// How long the GUI may send nothing at all before the helm closes its
/// channel.
///
/// The GUI's client heartbeats every 5 seconds whatever else it is doing, so a
/// live GUI never comes near this. It exists so that a GUI that vanished
/// without closing (a remote helm whose network went away; embedded, a closed
/// socket is already reliable) gives up the seat within about a minute, rather
/// than holding it, and making agent approvals wait on a window nobody sees,
/// until TCP notices. The helm sends no keepalive of its own: the GUI's
/// heartbeat is the liveness signal, so silence is all the helm has to watch.
const SILENCE_CLOSE: Duration = Duration::from_secs(60);

/// Outgoing messages queued between the channel and its socket writer.
/// Backpressure past this is the write deadline's job, not memory's.
const OUTGOING_QUEUE: usize = 64;

/// Decoded GUI messages queued between the socket reader and the channel.
/// The reader waits when it is full, which in turn stops reading the socket.
const INCOMING_QUEUE: usize = 64;

/// Commands that may wait behind the one in flight before the channel is
/// closed. Commands run one at a time, so a GUI that keeps sending while one
/// is stuck behind a slow host would otherwise grow this queue without bound.
/// A real GUI never comes close: its watchdog resets the window long before.
/// Closing is the same failure the GUI already handles (a reset), not a new
/// refusal for it to understand.
pub(crate) const MAX_QUEUED_COMMANDS: usize = 64;

// ---------------------------------------------------------------------------
// The seat
// ---------------------------------------------------------------------------

/// Which UI holds this helm: at most one, and the newest wins (SPEC.md, "One
/// GUI at a time").
///
/// Taking the seat displaces the holder: its channel is told and closes, and
/// it does not reconnect on its own. Agent approvals ask [`Self::is_held`]
/// whether a window is open to show a card in, so only a live channel counts:
/// the helm's own subscriptions to the fleet feed never do.
///
/// The seat holds the sending half of the current holder's displacement
/// signal, and the holder's channel holds the receiving half for as long as it
/// runs. So "held" is simply "that receiver still exists": a channel that ends
/// for any reason (the GUI left, a write failed, it was displaced) drops its
/// receiver and gives the seat up with nothing to release by hand, and a
/// displaced channel ending later cannot free the seat its successor holds,
/// because the seat no longer points at it.
#[derive(Default)]
pub struct UiSeat {
    holder: Mutex<Option<oneshot::Sender<()>>>,
}

impl UiSeat {
    /// Take the seat for a new channel, displacing any holder. Keep the
    /// returned receiver for as long as the channel runs: it resolves when a
    /// later channel displaces this one, and dropping it gives the seat up.
    pub fn take(&self) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();
        let previous = self
            .holder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .replace(tx);
        if let Some(displace) = previous {
            let _ = displace.send(());
        }
        rx
    }

    /// Whether a UI channel holds the seat right now: what "a Farhelm window
    /// is open" means for agent approvals. A snapshot, stale once read.
    pub fn is_held(&self) -> bool {
        self.holder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .is_some_and(|holder| !holder.is_closed())
    }

    /// Hold the seat with no channel behind it, for tests whose subject is
    /// what an open window allows (agent approvals) rather than the channel.
    /// The seat is held until the returned guard is dropped.
    #[cfg(test)]
    pub fn hold(&self) -> oneshot::Receiver<()> {
        self.take()
    }
}

// ---------------------------------------------------------------------------
// Collections
// ---------------------------------------------------------------------------

/// One collection's read, split into what the channel diffs.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Projected {
    /// Entries in served order, with their keys.
    pub(crate) entries: Vec<(String, Value)>,
    pub(crate) counts: Option<ListCounts>,
    /// The key of the entry in `entries` that is there only because the
    /// session query selected it (always the last entry). The diff keeps it
    /// out of the order and names it in the metadata.
    pub(crate) selected_outside_query: Option<String>,
    /// The session query's selected session is in neither `entries` nor its
    /// own read, and that read was cut at its host's cap
    /// (`CollectionMeta::selected_cut`). `false` for every other collection.
    pub(crate) selected_cut: bool,
}

/// How the channel reads one pushed collection: which GET to run, and how to
/// split its reply into keyed entries.
///
/// The reads are the collections' existing GET handlers, so every consistency
/// rule those enforce (staleness marking, owner routing, the listing cap)
/// holds here too. Every one of them reads helm-local state (the store, the
/// manager's snapshots, in-memory approvals, the provisioning service's
/// in-memory runs and the recipe build registry, whose locks no holder keeps
/// across host I/O, and the Podman check's cache) and never waits on a host,
/// so the snapshot sent when a channel opens never waits on one either. A
/// collection added here must keep that true, and its read must start
/// nothing: it runs after every bump, so a read that schedules work (as
/// `/api/environment-places` schedules Podman checks) would start it over and
/// over.
#[derive(Clone, Copy)]
pub(crate) struct CollectionSpec {
    pub(crate) collection: Collection,
    pub(crate) path: fn(&SessionQuery) -> String,
    pub(crate) split: fn(Value) -> Result<Projected, String>,
    /// For the collection [`SessionQuery::selected`] names an entry of (the
    /// session list): the path of a read that serves only the entry filed
    /// under a key, through the same route and `split`. `None` for every
    /// other collection. Like `path`, the read must be helm-local.
    pub(crate) selected: Option<fn(&str) -> String>,
}

/// The collections a production channel pushes.
pub(crate) fn production_collections() -> Vec<CollectionSpec> {
    vec![
        CollectionSpec {
            collection: Collection::Sessions,
            path: session_list_path,
            split: split_sessions,
            selected: Some(selected_session_path),
        },
        CollectionSpec {
            collection: Collection::Hosts,
            path: |_| "/api/hosts".to_string(),
            split: |body| split_keyed_array(body, "hosts", "id"),
            selected: None,
        },
        CollectionSpec {
            collection: Collection::Preferences,
            path: |_| "/api/preferences".to_string(),
            split: |body| {
                Ok(Projected {
                    entries: vec![(PREFERENCES_KEY.to_string(), body)],
                    counts: None,
                    selected_outside_query: None,
                    selected_cut: false,
                })
            },
            selected: None,
        },
        CollectionSpec {
            collection: Collection::Approvals,
            path: |_| "/api/approvals".to_string(),
            split: |body| split_keyed_array(body, "approvals", "id"),
            selected: None,
        },
        CollectionSpec {
            collection: Collection::Templates,
            path: |_| "/api/templates".to_string(),
            split: |body| split_keyed_array(body, "templates", "name"),
            selected: None,
        },
        CollectionSpec {
            collection: Collection::Notices,
            path: |_| "/api/notices".to_string(),
            split: |body| split_keyed_array(body, "notices", "id"),
            selected: None,
        },
        CollectionSpec {
            collection: Collection::Announcements,
            path: |_| "/api/announcements".to_string(),
            split: |body| {
                Ok(Projected {
                    entries: vec![(ANNOUNCEMENTS_KEY.to_string(), body)],
                    counts: None,
                    selected_outside_query: None,
                    selected_cut: false,
                })
            },
            selected: None,
        },
        CollectionSpec {
            collection: Collection::HostProgress,
            path: |_| "/api/provisioning".to_string(),
            split: |body| split_keyed_array(body, "runs", "host_id"),
            selected: None,
        },
        // Both recipe reads are pure projections of the store and the build
        // registry: unlike `/api/environment-places`, neither starts a Podman
        // check, so re-reading them on every bump starts nothing.
        CollectionSpec {
            collection: Collection::Recipes,
            path: |_| "/api/tensorlake/recipes".to_string(),
            split: |body| split_keyed_array(body, "recipes", "name"),
            selected: None,
        },
        CollectionSpec {
            collection: Collection::RecipePlaces,
            path: |_| "/api/recipe-places".to_string(),
            split: split_recipe_places,
            selected: None,
        },
    ]
}

/// `GET /api/recipe-places`, a bare array rather than an object, keyed by
/// recipe and place. The key comes from the decoded entry through the proto's
/// own spelling ([`farhelm_proto::RecipePlaceView::key`]) so the GUI can look
/// an entry up by the same pair; the entry itself travels as served.
fn split_recipe_places(body: Value) -> Result<Projected, String> {
    let Value::Array(items) = body else {
        return Err("expected an array of recipe places".to_string());
    };
    let entries = items
        .into_iter()
        .map(|item| {
            let view: farhelm_proto::RecipePlaceView = serde_json::from_value(item.clone())
                .map_err(|error| format!("a recipe place does not decode: {error}"))?;
            Ok((
                farhelm_proto::RecipePlaceView::key(&view.recipe, view.place),
                item,
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Projected {
        entries,
        counts: None,
        selected_outside_query: None,
        selected_cut: false,
    })
}

/// `GET /api/sessions` with this channel's query. Absent fields are left out,
/// so the list route applies its own defaults.
fn session_list_path(query: &SessionQuery) -> String {
    let mut params = url::form_urlencoded::Serializer::new(String::new());
    if let Some(host) = query.host {
        params.append_pair("host", &host.to_string());
    }
    for (name, value) in [
        ("parent", &query.parent),
        ("directory", &query.directory),
        ("status", &query.status),
        ("title", &query.title),
        ("sort", &query.sort),
    ] {
        if let Some(value) = value {
            params.append_pair(name, value);
        }
    }
    let params = params.finish();
    if params.is_empty() {
        "/api/sessions".to_string()
    } else {
        format!("/api/sessions?{params}")
    }
}

/// `GET /api/sessions` narrowed to the one session served under `key`, with
/// no other filter, so no other filter and not the listing cap can leave it
/// out. Its host's own cap still can, which the reply's `truncated` then
/// says (scoped to that host, `store::SessionFilter::host_scope`).
fn selected_session_path(key: &str) -> String {
    let params = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("id", key)
        .finish();
    format!("/api/sessions?{params}")
}

/// The key the query selects, if any. An empty key selects nothing: the list
/// route reads an empty `?id=` as absent, so reading it would serve the whole
/// unfiltered fleet after every push only to find nothing in it.
fn selected_key(query: &SessionQuery) -> Option<&str> {
    query.selected.as_deref().filter(|key| !key.is_empty())
}

/// The session list: rows keyed by their served (fully qualified) id, plus
/// the counts that describe more than the rows do.
fn split_sessions(body: Value) -> Result<Projected, String> {
    let counts = ListCounts {
        total: body["total"]
            .as_u64()
            .ok_or("the session list has no `total`")?,
        matching: body.get("matching").and_then(Value::as_u64),
        truncated: body["truncated"]
            .as_bool()
            .ok_or("the session list has no `truncated`")?,
    };
    let mut projected = split_keyed_array(body, "sessions", "id")?;
    projected.counts = Some(counts);
    Ok(projected)
}

/// Split `body[field]`, an array of objects, into entries keyed by each
/// object's `key` field (a string, or a number rendered as one).
fn split_keyed_array(body: Value, field: &str, key: &str) -> Result<Projected, String> {
    let Value::Object(mut object) = body else {
        return Err(format!("expected an object holding `{field}`"));
    };
    let Some(Value::Array(items)) = object.remove(field) else {
        return Err(format!("expected an array at `{field}`"));
    };
    let entries = items
        .into_iter()
        .map(|item| {
            let id = match item.get(key) {
                Some(Value::String(id)) => id.clone(),
                Some(Value::Number(id)) => id.to_string(),
                _ => return Err(format!("an entry of `{field}` has no `{key}`")),
            };
            Ok((id, item))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Projected {
        entries,
        counts: None,
        selected_outside_query: None,
        selected_cut: false,
    })
}

/// What this channel last sent about one collection.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Sent {
    pub(crate) meta: CollectionMeta,
    pub(crate) entries: BTreeMap<String, Value>,
}

/// Compare a fresh read with what was last sent, and say what to send.
///
/// A failed read changes only the availability: the entries, order and
/// counts the GUI holds stay as they were, because a read that failed is no
/// evidence that anything was deleted. `None` means nothing changed.
pub(crate) fn diff(
    collection: Collection,
    previous: &Sent,
    read: Result<Projected, String>,
) -> (Option<CollectionChanges>, Sent) {
    let (meta, upsert, remove, entries) = match read {
        Err(error) => {
            let meta = CollectionMeta {
                availability: Availability::Unavailable { error },
                ..previous.meta.clone()
            };
            (meta, Vec::new(), Vec::new(), previous.entries.clone())
        }
        Ok(projected) => {
            let outside = projected.selected_outside_query;
            let order = projected
                .entries
                .iter()
                .map(|(key, _)| key)
                .filter(|key| Some(*key) != outside.as_ref())
                .cloned()
                .collect();
            let mut upsert = Vec::new();
            for (key, value) in &projected.entries {
                if previous.entries.get(key) != Some(value) {
                    upsert.push(KeyedEntry {
                        key: key.clone(),
                        value: value.clone(),
                    });
                }
            }
            let entries: BTreeMap<String, Value> = projected.entries.into_iter().collect();
            let remove = previous
                .entries
                .keys()
                .filter(|key| !entries.contains_key(*key))
                .cloned()
                .collect();
            let meta = CollectionMeta {
                availability: Availability::Available,
                order,
                counts: projected.counts,
                selected_outside_query: outside,
                selected_cut: projected.selected_cut,
            };
            (meta, upsert, remove, entries)
        }
    };
    let sent = Sent { meta, entries };
    if upsert.is_empty() && remove.is_empty() && sent.meta == previous.meta {
        return (None, sent);
    }
    let changes = CollectionChanges {
        collection,
        meta: sent.meta.clone(),
        upsert,
        remove,
    };
    (Some(changes), sent)
}

// ---------------------------------------------------------------------------
// The channel
// ---------------------------------------------------------------------------

/// One channel's state: how it reaches the router, what it pushes, and what
/// it last pushed.
pub(crate) struct Channel {
    /// The helm's API router, built for this channel. Commands and
    /// projections go through it exactly as HTTP requests would.
    router: axum::Router,
    /// `Bearer <secret>`, the credential this channel's upgrade was admitted
    /// with, so every routed request passes the same authentication.
    authorization: HeaderValue,
    collections: Vec<CollectionSpec>,
    /// The session list's query: its filters come from the GUI's
    /// [`SESSION_QUERY_PATH`] command, and so does its sort while that
    /// command names one; otherwise the sort follows the stored list order
    /// ([`Channel::follow_stored_order`]).
    query: SessionQuery,
    /// Whether the sort follows the stored list-order preference: true while
    /// the GUI's current query names no sort of its own, including before
    /// the GUI has sent any query.
    sort_follows_preference: bool,
    sent: BTreeMap<Collection, Sent>,
}

/// The list order a channel uses when the helm has none stored, or has one
/// this build does not serve: most recent activity, which SPEC.md ("Session
/// list") says is what a client shows until someone picks otherwise. Not the
/// list route's own default (creation order), which stays what an HTTP caller
/// that names no order gets.
const DEFAULT_LIST_ORDER: crate::store::ListSort = crate::store::ListSort::Activity;

/// A routed command in flight: the future that runs it through the router
/// and yields its reply. Channel-local commands never become one; [`run`]
/// answers those itself.
type InFlight = Pin<Box<dyn Future<Output = Reply> + Send>>;

/// The router's method for a command's method. The one place the wire's
/// closed set becomes HTTP, so a method added to the wire type does not
/// compile until the helm routes it here.
fn http_method(method: WireMethod) -> Method {
    match method {
        WireMethod::Get => Method::GET,
        WireMethod::Post => Method::POST,
        WireMethod::Put => Method::PUT,
        WireMethod::Delete => Method::DELETE,
        WireMethod::Patch => Method::PATCH,
    }
}

impl Channel {
    pub(crate) fn new(
        router: axum::Router,
        authorization: HeaderValue,
        collections: Vec<CollectionSpec>,
    ) -> Channel {
        Channel {
            router,
            authorization,
            collections,
            query: SessionQuery::default(),
            sort_follows_preference: true,
            sent: BTreeMap::new(),
        }
    }

    /// Build the routed request for `method path` with an optional JSON body.
    ///
    /// Fails only for a path no request can carry; command paths are
    /// canonical by their type (`ApiPath`), and the channel's own paths are
    /// built from encoded parts, so a failure here is answered as a refusal or
    /// an unavailable read rather than trusted never to happen.
    fn request(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Request<Body>, axum::http::Error> {
        let builder = Request::builder()
            .method(method)
            .uri(path)
            .header(header::AUTHORIZATION, self.authorization.clone());
        match body {
            Some(body) => builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string())),
            None => builder.body(Body::empty()),
        }
    }

    /// Read one collection through its GET handler, plus the entry the query
    /// selected when the collection has one and the read left it out (see the
    /// module docs, "The open session rides outside the query").
    ///
    /// A selected entry that is listed anyway is not read again and not
    /// marked, so it is never sent twice. A selected key the helm knows no
    /// session under is simply not carried.
    ///
    /// When the selected read finds nothing and its own reply was cut (its
    /// host's supervisor lists only its newest sessions, so an old one can
    /// exist without being listed), the read says so in `selected_cut`
    /// rather than in the counts: the query's own list may have been read
    /// whole (a host filter naming another host), and its counts say only
    /// what happened to it. Without the flag, that whole list missing the
    /// open session would read to the GUI as proof that it was deleted.
    async fn read(&self, spec: &CollectionSpec, query: &SessionQuery) -> Result<Projected, String> {
        let mut projected = self.read_path(spec, &(spec.path)(query)).await?;
        if let (Some(selected_path), Some(key)) = (spec.selected, selected_key(query))
            && !projected.entries.iter().any(|(listed, _)| listed == key)
        {
            match self.read_selected(spec, &selected_path(key), key).await? {
                (Some(entry), _) => {
                    projected.entries.push(entry);
                    projected.selected_outside_query = Some(key.to_owned());
                }
                (None, cut) => projected.selected_cut = cut,
            }
        }
        Ok(projected)
    }

    /// One GET of `path`, split as `spec` splits its collection.
    async fn read_path(&self, spec: &CollectionSpec, path: &str) -> Result<Projected, String> {
        let request = self
            .request(Method::GET, path, None)
            .map_err(|error| format!("this list's read could not be built: {error}"))?;
        let Routed { status, body, .. } = route(self.router.clone(), request).await;
        if !status.is_success() {
            return Err(format!(
                "reading this list failed ({status}): {}",
                String::from_utf8_lossy(&body)
            ));
        }
        let body: Value = serde_json::from_slice(&body)
            .map_err(|error| format!("this list's reply was not JSON: {error}"))?;
        (spec.split)(body)
    }

    /// The selected entry's own read: the entry, or `None` when the helm
    /// serves none under `key`, and whether that reply was cut
    /// (`truncated`), which is what says a `None` may not mean "deleted".
    /// The one-session read is scoped to the key's host, so only that host's
    /// cap cuts it (`store::SessionFilter::host_scope`).
    ///
    /// A refusal (a 4xx: a key that does not parse as a session reference, or
    /// whose host entry is gone) means the key names nothing, so the entry is
    /// absent and the reply uncut, as for a session deleted since. Any other
    /// failure fails the collection's read like a failed list read, because a
    /// failed read is no evidence that the session is gone.
    async fn read_selected(
        &self,
        spec: &CollectionSpec,
        path: &str,
        key: &str,
    ) -> Result<(Option<(String, Value)>, bool), String> {
        let request = self
            .request(Method::GET, path, None)
            .map_err(|error| format!("the open session's read could not be built: {error}"))?;
        // The markers are left out: this read is never relayed to the GUI as
        // a reply, only merged into the push.
        let Routed { status, body, .. } = route(self.router.clone(), request).await;
        if status.is_client_error() {
            return Ok((None, false));
        }
        if !status.is_success() {
            return Err(format!(
                "reading the open session failed ({status}): {}",
                String::from_utf8_lossy(&body)
            ));
        }
        let body: Value = serde_json::from_slice(&body)
            .map_err(|error| format!("the open session's reply was not JSON: {error}"))?;
        let projected = (spec.split)(body)?;
        let cut = projected.counts.is_some_and(|counts| counts.truncated);
        let entry = projected
            .entries
            .into_iter()
            .find(|(served, _)| served == key);
        Ok((entry, cut))
    }

    /// Sort the session list in the order the helm has stored as the shared
    /// list-order preference, read afresh, unless the GUI's own query has
    /// named a sort. Runs before every read of the collections: the first
    /// snapshot and every re-projection.
    ///
    /// Without this, every channel would open in creation order and the GUI
    /// would have to send a query to correct it: the list would visibly
    /// reorder after every connect, and every reset (a new channel) would put
    /// it back in the wrong order until the query was answered. Reading it
    /// again on every re-projection is what makes a change to the preference
    /// made elsewhere (another client, an agent, the command line) move the
    /// open list, as SPEC.md ("Session list") asks of every shared preference:
    /// a preference write bumps the fleet feed, and the re-projection that
    /// follows reads the new order. The read is one more GET of a helm-local
    /// row per re-projection.
    ///
    /// The stored word is read through the preferences route, like every
    /// other read the channel makes. A word this build does not serve (a newer
    /// build can have stored it), no word, or a failed read all use
    /// [`DEFAULT_LIST_ORDER`]: the list must never fail to load because of a
    /// remembered order. Only the sort is set; filters stay as the GUI set
    /// them. A channel that pushes no session list reads nothing.
    async fn follow_stored_order(&mut self) {
        if !self.sort_follows_preference
            || !self
                .collections
                .iter()
                .any(|spec| spec.collection == Collection::Sessions)
        {
            return;
        }
        let stored = match self.request(Method::GET, "/api/preferences", None) {
            Ok(request) => {
                let Routed { status, body, .. } = route(self.router.clone(), request).await;
                status
                    .is_success()
                    .then(|| serde_json::from_slice::<Value>(&body).ok())
                    .flatten()
                    .and_then(|body| body["list_sort"].as_str().map(str::to_owned))
            }
            Err(_) => None,
        };
        let sort = stored
            .as_deref()
            .and_then(crate::store::parse_sort_key)
            .unwrap_or(DEFAULT_LIST_ORDER);
        self.query.sort = Some(sort.key().to_owned());
    }

    /// Every collection as it stands, recording it as sent.
    pub(crate) async fn snapshot(&mut self) -> ServerMessage {
        self.follow_stored_order().await;
        let mut collections = Vec::new();
        for spec in self.collections.clone() {
            let read = self.read(&spec, &self.query).await;
            let empty = Sent {
                meta: CollectionMeta {
                    availability: Availability::Available,
                    order: Vec::new(),
                    counts: None,
                    selected_outside_query: None,
                    selected_cut: false,
                },
                entries: BTreeMap::new(),
            };
            let (_, sent) = diff(spec.collection, &empty, read);
            let entries = sent
                .meta
                .order
                .iter()
                .chain(&sent.meta.selected_outside_query)
                .filter_map(|key| {
                    sent.entries.get(key).map(|value| KeyedEntry {
                        key: key.clone(),
                        value: value.clone(),
                    })
                })
                .collect();
            collections.push(CollectionState {
                collection: spec.collection,
                meta: sent.meta.clone(),
                entries,
            });
            self.sent.insert(spec.collection, sent);
        }
        ServerMessage::Snapshot { collections }
    }

    /// Re-read every collection and return the changes to push.
    pub(crate) async fn changes(&mut self) -> Vec<ServerMessage> {
        self.follow_stored_order().await;
        let mut messages = Vec::new();
        for spec in self.collections.clone() {
            let read = self.read(&spec, &self.query).await;
            let previous = self.sent.get(&spec.collection).cloned().unwrap_or(Sent {
                meta: CollectionMeta {
                    availability: Availability::Available,
                    order: Vec::new(),
                    counts: None,
                    selected_outside_query: None,
                    selected_cut: false,
                },
                entries: BTreeMap::new(),
            });
            let (changes, sent) = diff(spec.collection, &previous, read);
            self.sent.insert(spec.collection, sent);
            messages.extend(changes.map(ServerMessage::Changes));
        }
        messages
    }

    /// Start a routed command, or refuse it at once. The future owns a clone
    /// of the router, so it can run while the channel serves heartbeats.
    fn start(&self, command: Command) -> InFlight {
        let id = command.id;
        let refused = |message: String| -> InFlight {
            Box::pin(async move {
                Reply {
                    id,
                    status: StatusCode::BAD_REQUEST.as_u16(),
                    markers: Vec::new(),
                    body: Some(Value::String(message)),
                }
            })
        };
        let method = http_method(command.method);
        // The path is canonical by its type: a command whose path is not did
        // not decode, so it never got this far (`farhelm_proto::ui_channel::ApiPath`).
        // A canonical path can still encode a separator inside one segment (an id
        // that holds a `/`); routes would decode it into two, so it is refused here.
        if command.path.encodes_a_separator() {
            return refused("a command path may not encode a slash or backslash".to_string());
        }
        let request = match self.request(method, command.path.as_str(), command.body.as_ref()) {
            Ok(request) => request,
            Err(error) => return refused(format!("the command cannot be sent: {error}")),
        };
        let router = self.router.clone();
        Box::pin(async move { route(router, request).await.reply(id) })
    }

    /// The channel-local [`SESSION_QUERY_PATH`] command. The new query is
    /// validated by serving the list with it, so the route's own refusals (an
    /// unknown status or sort word) answer here unchanged and leave the old
    /// query in force. Its selected key is not validated: one that names no
    /// session is accepted and carries nothing.
    async fn set_session_query(&mut self, command: Command) -> (Vec<ServerMessage>, Reply) {
        let reply = |status: StatusCode, body: Option<Value>| Reply {
            id: command.id,
            status: status.as_u16(),
            markers: Vec::new(),
            body,
        };
        if command.method != WireMethod::Put {
            let body = Value::String(format!("{SESSION_QUERY_PATH} takes PUT"));
            return (
                Vec::new(),
                reply(StatusCode::METHOD_NOT_ALLOWED, Some(body)),
            );
        }
        let query: SessionQuery =
            match serde_json::from_value(command.body.clone().unwrap_or(Value::Null)) {
                Ok(query) => query,
                Err(error) => {
                    let body = Value::String(format!("not a session query: {error}"));
                    return (Vec::new(), reply(StatusCode::BAD_REQUEST, Some(body)));
                }
            };
        let request = match self.request(Method::GET, &session_list_path(&query), None) {
            Ok(request) => request,
            Err(error) => {
                let body = Value::String(format!("not a session query: {error}"));
                return (Vec::new(), reply(StatusCode::BAD_REQUEST, Some(body)));
            }
        };
        let routed = route(self.router.clone(), request).await;
        if !routed.status.is_success() {
            return (Vec::new(), routed.reply(command.id));
        }
        // A query that names a sort takes the order over from the stored
        // preference; a later one without a sort (the GUI clearing a filter,
        // say) hands it back, rather than leaving the list in the list
        // route's default order, which is neither what the user stored nor
        // what the GUI last named.
        self.sort_follows_preference = query.sort.is_none();
        self.query = query;
        let changes = self.changes().await;
        (changes, reply(StatusCode::NO_CONTENT, None))
    }
}

/// What the router answered one request with: everything of the response
/// that a [`Reply`] relays. Its other headers stay behind.
struct Routed {
    status: StatusCode,
    /// The helm-only markers among the response's headers ([`markers`]).
    markers: Vec<HelmMarker>,
    body: Bytes,
}

impl Routed {
    /// The reply to command `id` that relays this response.
    fn reply(self, id: u64) -> Reply {
        Reply {
            id,
            status: self.status.as_u16(),
            markers: self.markers,
            body: reply_body(&self.body),
        }
    }
}

/// Run one request through the router and collect its reply.
///
/// A body too large to relay turns the reply into a 500 of the channel's own,
/// which carries no markers: they described the response that was dropped.
async fn route(router: axum::Router, request: Request<Body>) -> Routed {
    let response = match router.oneshot(request).await {
        Ok(response) => response,
        Err(infallible) => match infallible {},
    };
    let status = response.status();
    let markers = markers(response.headers());
    match to_bytes(response.into_body(), MAX_REPLY_BODY).await {
        Ok(body) => Routed {
            status,
            markers,
            body,
        },
        Err(error) => Routed {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            markers: Vec::new(),
            body: Bytes::from(format!("the reply could not be relayed: {error}")),
        },
    }
}

/// The helm-only markers a routed response's headers state, in
/// [`HelmMarker::ALL`]'s order.
///
/// One lookup for every route: the channel knows the closed set of helm-only
/// headers, not which route sets which, so a route that starts marking its
/// refusals reaches the GUI unchanged (SPEC_impl.md, "The UI channel": no
/// second, typed command API). A header counts only with the one value its
/// marker names; any other value is not that marker. The router's responses
/// are the helm's own, and a supervisor's words only ever reach a body, so
/// nothing a host says can set one of these.
fn markers(headers: &HeaderMap) -> Vec<HelmMarker> {
    HelmMarker::ALL
        .into_iter()
        .filter(|marker| {
            let (name, value) = marker.header();
            headers.get(name).is_some_and(|sent| sent == value)
        })
        .collect()
}

/// A reply body as the GUI receives it: JSON when it parses, text otherwise,
/// nothing when empty.
fn reply_body(body: &[u8]) -> Option<Value> {
    if body.is_empty() {
        return None;
    }
    Some(
        serde_json::from_slice(body)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(body).into_owned())),
    )
}

/// Serve one channel until the GUI goes away, the channel breaks, or another
/// UI takes the seat.
///
/// The loop owns all ordering. Commands queue in arrival order and start one
/// at a time; while one runs, heartbeats are still answered at once, and feed
/// bumps are left for the re-projection that follows the command (which
/// covers them). When a command completes, its changes are pushed and only
/// then its reply. Returning drops any command still in flight: owned work
/// carries on, cancellable work stops, as for an HTTP caller that left.
pub(crate) async fn run(
    mut channel: Channel,
    mut incoming: mpsc::Receiver<ClientMessage>,
    outgoing: mpsc::Sender<ServerMessage>,
    mut displaced: oneshot::Receiver<()>,
    mut feed: watch::Receiver<u64>,
) {
    feed.borrow_and_update();
    if outgoing.send(channel.snapshot().await).await.is_err() {
        return;
    }
    let mut queue: VecDeque<Command> = VecDeque::new();
    let mut in_flight: Option<InFlight> = None;
    loop {
        while in_flight.is_none() {
            let Some(command) = queue.pop_front() else {
                break;
            };
            if command.path.as_str() == SESSION_QUERY_PATH {
                let (changes, reply) = channel.set_session_query(command).await;
                for message in changes.into_iter().chain([ServerMessage::Reply(reply)]) {
                    if outgoing.send(message).await.is_err() {
                        return;
                    }
                }
            } else {
                in_flight = Some(channel.start(command));
            }
        }
        tokio::select! {
            biased;
            _ = &mut displaced => {
                let _ = outgoing.send(ServerMessage::Displaced).await;
                return;
            }
            message = incoming.recv() => match message {
                None => return,
                Some(ClientMessage::Ping { n }) => {
                    if outgoing.send(ServerMessage::Pong { n }).await.is_err() {
                        return;
                    }
                }
                Some(ClientMessage::Command(command)) => {
                    if queue.len() >= MAX_QUEUED_COMMANDS {
                        tracing::debug!(
                            "a UI channel queued more than {MAX_QUEUED_COMMANDS} commands; closing it"
                        );
                        return;
                    }
                    queue.push_back(command);
                }
            },
            reply = async { in_flight.as_mut().expect("guarded").await }, if in_flight.is_some() => {
                in_flight = None;
                feed.borrow_and_update();
                let changes = channel.changes().await;
                for message in changes.into_iter().chain([ServerMessage::Reply(reply)]) {
                    if outgoing.send(message).await.is_err() {
                        return;
                    }
                }
            }
            changed = feed.changed(), if in_flight.is_none() => {
                if changed.is_err() {
                    return;
                }
                feed.borrow_and_update();
                for message in channel.changes().await {
                    if outgoing.send(message).await.is_err() {
                        return;
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The endpoint
// ---------------------------------------------------------------------------

/// `GET /api/ui` — open this GUI's channel, taking the seat.
///
/// Authentication has already run (the protected router's middleware); the
/// credential it admitted is reused for every routed command. The seat is
/// taken before the upgrade, so a displaced channel learns it promptly; the
/// channel holds the seat for exactly as long as it holds `displaced`.
pub(crate) async fn ui_ws(
    State(state): State<Arc<AppState>>,
    upgrade: WebSocketUpgrade,
    headers: HeaderMap,
) -> impl IntoResponse {
    let Some(secret) = crate::auth::websocket_device_secret(&headers) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Ok(authorization) = HeaderValue::from_str(&format!("Bearer {secret}")) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let displaced = state.manager.ui_seat().take();
    let channel = Channel::new(
        crate::api_router(Arc::clone(&state)),
        authorization,
        production_collections(),
    );
    let feed = state.manager.events().subscribe();
    upgrade
        .protocols([crate::auth::WS_PROTOCOL])
        .max_frame_size(MAX_CLIENT_MESSAGE)
        .max_message_size(MAX_CLIENT_MESSAGE)
        .on_upgrade(move |socket| async move {
            let (tx, rx) = futures_util::StreamExt::split(socket);
            serve_socket(tx, rx, channel, displaced, feed).await;
        })
        .into_response()
}

/// Bridge one WebSocket to [`run`]: decode what the GUI sends, encode what
/// the channel sends, close on silence, and end everything together.
///
/// Generic over the socket's two halves so tests can stand in a sink that
/// stops accepting writes; production passes the halves of an axum socket.
///
/// Three tasks, and how each ending ends the others:
///
/// - The channel ([`run`]) ends when the GUI goes away, the channel breaks or
///   the seat is taken. Its writer then drains what it queued, so a displaced
///   channel's last message still reaches the GUI before the socket closes.
/// - The writer owns the outgoing queue. If a write fails or misses its
///   deadline, the writer ends, and with it the whole channel: [`run`] is
///   dropped, which drops its seat receiver and so frees the seat. A channel
///   whose GUI can no longer be written to must not keep the seat (agent
///   approvals would wait on a window that will never show their card).
/// - The reader ends on a close, an error, a message it cannot decode (a
///   command whose path is not canonical is one: see `ApiPath`), or
///   [`SILENCE_CLOSE`] without any frame.
pub(crate) async fn serve_socket<Tx, Rx>(
    mut tx: Tx,
    mut rx: Rx,
    channel: Channel,
    displaced: oneshot::Receiver<()>,
    feed: watch::Receiver<u64>,
) where
    Tx: futures_util::Sink<ws::Message> + Unpin,
    Rx: futures_util::Stream<Item = Result<ws::Message, axum::Error>> + Unpin,
{
    use futures_util::{SinkExt, StreamExt};

    let (incoming_tx, incoming_rx) = mpsc::channel(INCOMING_QUEUE);
    let (outgoing_tx, mut outgoing_rx) = mpsc::channel::<ServerMessage>(OUTGOING_QUEUE);

    let core = run(channel, incoming_rx, outgoing_tx, displaced, feed);
    // `async move`: the writer owns the outgoing queue and the socket's write
    // half, so its ending is final for both.
    let writer = async move {
        while let Some(message) = outgoing_rx.recv().await {
            let frame = match serde_json::to_string(&message) {
                Ok(text) => ws::Message::Text(text.into()),
                Err(error) => {
                    tracing::warn!(%error, "a UI channel message did not serialize; closing the channel");
                    return;
                }
            };
            match tokio::time::timeout(WRITE_DEADLINE, tx.send(frame)).await {
                Ok(Ok(())) => {}
                _ => {
                    tracing::debug!("a UI channel write failed or timed out; closing the channel");
                    return;
                }
            }
        }
        let _ = tokio::time::timeout(WRITE_DEADLINE, tx.send(ws::Message::Close(None))).await;
    };
    let channel_and_writer = async move {
        tokio::pin!(writer);
        tokio::select! {
            // The channel ended on its own: let the writer drain and close.
            () = core => writer.await,
            // The writer ended first, which only a failed write does while
            // the channel still runs: end the channel with it.
            () = &mut writer => {}
        }
    };

    let reader = async move {
        loop {
            let frame = match tokio::time::timeout(SILENCE_CLOSE, rx.next()).await {
                Ok(frame) => frame,
                Err(_) => {
                    tracing::debug!("a UI channel sent nothing for {SILENCE_CLOSE:?}; closing it");
                    return;
                }
            };
            let message = match frame {
                Some(Ok(ws::Message::Text(text))) => {
                    match serde_json::from_str::<ClientMessage>(text.as_str()) {
                        Ok(message) => message,
                        Err(error) => {
                            tracing::debug!(%error, "a UI channel sent a message it has no word for; closing it");
                            return;
                        }
                    }
                }
                Some(Ok(ws::Message::Ping(_) | ws::Message::Pong(_))) => continue,
                Some(Ok(ws::Message::Binary(_) | ws::Message::Close(_))) | None | Some(Err(_)) => {
                    return;
                }
            };
            if incoming_tx.send(message).await.is_err() {
                return;
            }
        }
    };

    tokio::select! {
        () = channel_and_writer => {}
        () = reader => {}
    }
}

#[cfg(test)]
#[path = "ui_channel_tests.rs"]
mod tests;
