//! One process-global `tracing` capture, shared by every test in this
//! crate that needs to observe what was LOGGED rather than what was
//! returned.
//!
//! ## Why this is shared, and why it has to be
//!
//! `tracing` has a thread-local subscriber and a process-global one, and
//! only the global reaches the places this crate does its interesting work:
//! a `spawn_blocking` closure inside the store, and the spawned connection
//! actors in the manager. A global can be installed exactly ONCE per
//! process, and all of this crate's unit tests share one test binary — so
//! two modules each installing their own capture would mean whichever test
//! ran first silently won and the other's assertions observed an empty
//! buffer. One buffer, filtered per test, is the only arrangement that is
//! not a race.
//!
//! Every caller must therefore filter the buffer down to ITS OWN data
//! (a unique session id, a host id it just created), because tests run
//! concurrently and the buffer is genuinely everyone's.
//!
//! ## What is captured
//!
//! Each event's own fields, plus the fields of every span enclosing it —
//! which is what lets a test assert that the manager's per-host span
//! context actually reaches the lines SPEC.md's reconnection trail is made
//! of, rather than only that the messages were emitted.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;

/// One captured event: its own fields, and the fields inherited from the
/// spans it was emitted inside.
///
/// The message itself lands under the key `"message"`, matching `tracing`'s
/// own convention for the format-string argument.
#[derive(Debug, Clone)]
pub(crate) struct CapturedEvent {
    pub(crate) fields: HashMap<String, String>,
    /// Span fields, innermost span last, flattened into one map — this
    /// crate's spans use distinct field names (`host`, `kind`), so
    /// flattening loses nothing and spares every caller a nested walk.
    pub(crate) span_fields: HashMap<String, String>,
}

impl CapturedEvent {
    /// One field's value, looked up in the event's own fields first and
    /// then in its span context — the order a reader of the rendered log
    /// line would resolve it in.
    pub(crate) fn field(&self, name: &str) -> Option<&str> {
        self.fields
            .get(name)
            .or_else(|| self.span_fields.get(name))
            .map(String::as_str)
    }

    /// Whether this event's message contains `needle` — the coarse filter
    /// every caller starts from.
    pub(crate) fn message_contains(&self, needle: &str) -> bool {
        self.field("message").is_some_and(|m| m.contains(needle))
    }
}

/// A visitor that stringifies every field regardless of its original type.
///
/// Full typed fidelity would be wasted here: assertions in these tests
/// compare against text, and a `Debug`-rendered `i64` compares equal to the
/// number a reader would expect.
#[derive(Default)]
struct Fields(HashMap<String, String>);

impl tracing::field::Visit for Fields {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0
            .insert(field.name().to_string(), format!("{value:?}"));
    }
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.0.insert(field.name().to_string(), value.to_string());
    }
    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.0.insert(field.name().to_string(), value.to_string());
    }
    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.0.insert(field.name().to_string(), value.to_string());
    }
    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.0.insert(field.name().to_string(), value.to_string());
    }
}

struct Capture(Arc<Mutex<Vec<CapturedEvent>>>);

impl<S> tracing_subscriber::Layer<S> for Capture
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    /// A span's fields are recorded once, when it is created, and kept in
    /// the span's own extensions — the registry is what makes them
    /// reachable later, from an event that names none of them itself.
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut fields = Fields::default();
        attrs.record(&mut fields);
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(SpanFields(fields.0));
        }
    }

    fn on_event(&self, event: &tracing::Event<'_>, ctx: tracing_subscriber::layer::Context<'_, S>) {
        let mut fields = Fields::default();
        event.record(&mut fields);
        let mut span_fields = HashMap::new();
        if let Some(scope) = ctx.event_scope(event) {
            // Outermost first, so an inner span naming the same field
            // wins — the same shadowing a reader of the rendered line
            // would apply.
            for span in scope.from_root() {
                if let Some(SpanFields(recorded)) = span.extensions().get::<SpanFields>() {
                    span_fields.extend(recorded.clone());
                }
            }
        }
        self.0.lock().expect("capture mutex").push(CapturedEvent {
            fields: fields.0,
            span_fields,
        });
    }
}

/// One span's recorded fields, parked in that span's extensions.
struct SpanFields(HashMap<String, String>);

/// Install the capture (once per process) and hand back the shared buffer.
///
/// `try_init` rather than `init`: a second call must be a harmless no-op
/// against the SAME buffer rather than a panic, since every test that wants
/// to observe logs calls this.
pub(crate) fn install() -> Arc<Mutex<Vec<CapturedEvent>>> {
    static CAPTURED: OnceLock<Arc<Mutex<Vec<CapturedEvent>>>> = OnceLock::new();
    let events = CAPTURED
        .get_or_init(|| Arc::new(Mutex::new(Vec::new())))
        .clone();
    let _ = tracing_subscriber::util::SubscriberInitExt::try_init(
        tracing_subscriber::registry().with(Capture(events.clone())),
    );
    events
}

/// Every captured event whose message contains `needle`, as a snapshot —
/// so a caller asserts against a stable list rather than holding the
/// buffer's lock while other tests are still writing to it.
pub(crate) fn matching(
    events: &Arc<Mutex<Vec<CapturedEvent>>>,
    needle: &str,
) -> Vec<CapturedEvent> {
    events
        .lock()
        .expect("capture mutex")
        .iter()
        .filter(|event| event.message_contains(needle))
        .cloned()
        .collect()
}
