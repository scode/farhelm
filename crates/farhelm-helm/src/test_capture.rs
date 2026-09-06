//! Assertion helpers over the test-owned `farhelm-testtrace` capture.
//!
//! The `farhelm_testtrace::test` attribute owns both the Tokio runtime and
//! its scoped tracing dispatcher. That dispatcher follows work submitted to
//! its worker and blocking threads, so these helpers can observe store
//! warnings from `spawn_blocking` and manager actor events without a
//! process-global subscriber. They do not capture child processes or work on
//! a separately constructed runtime.

use std::ops::Deref;

use farhelm_testtrace::{CaptureHandle, EventSnapshot};

/// The assertion-facing event view kept stable while capture ownership moved
/// into `farhelm-testtrace`.
///
/// Event fields take precedence over inherited span fields, matching the
/// collector's structured shadowing semantics and the old test helper's
/// lookup contract. Deref exposes the target, level, and raw field maps for
/// assertions that need their exact representation without copying evidence
/// into another buffer.
#[derive(Debug, Clone)]
pub(crate) struct CapturedEvent(EventSnapshot);

impl Deref for CapturedEvent {
    type Target = EventSnapshot;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl CapturedEvent {
    /// Returns an event-owned field, falling back to inherited span context.
    pub(crate) fn field(&self, name: &str) -> Option<&str> {
        self.fields
            .get(name)
            .or_else(|| self.span_fields.get(name))
            .map(String::as_str)
    }

    /// Whether the event's message contains `needle`.
    pub(crate) fn message_contains(&self, needle: &str) -> bool {
        self.field("message")
            .is_some_and(|message| message.contains(needle))
    }
}

/// Returns the capture belonging to the active wrapped test.
///
/// There is deliberately no fallback subscriber or empty capture. A caller
/// outside `farhelm_testtrace::test` has no attributable evidence, so making
/// that mistake must fail where the assertion is written.
pub(crate) fn current() -> CaptureHandle {
    farhelm_testtrace::current_capture()
        .expect("test log assertions require #[farhelm_testtrace::test]")
}

/// Returns message matches only after the collector proves its evidence is complete.
pub(crate) fn matching(events: &CaptureHandle, needle: &str) -> Vec<CapturedEvent> {
    events
        .matching(needle)
        .unwrap_or_else(|error| panic!("test trace evidence is incomplete: {error}"))
        .into_iter()
        .map(CapturedEvent)
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use farhelm_testtrace::{
        CaptureConfig, CaptureSession, EventSnapshot, ExpectedPanic, TestMetadata,
    };

    /// A test without the wrapper must not turn absent evidence into an empty match.
    #[test]
    #[should_panic(expected = "test log assertions require #[farhelm_testtrace::test]")]
    fn current_requires_a_test_owned_capture() {
        let _ = super::current();
    }

    /// A lossy collector must fail an assertion even when its retained tail has no match.
    #[test]
    #[should_panic(expected = "test trace evidence is incomplete")]
    fn matching_rejects_incomplete_capture() {
        let session = CaptureSession::new(
            TestMetadata::new("incomplete-adapter", ExpectedPanic::None, None),
            CaptureConfig {
                max_events: 1,
                ..CaptureConfig::default()
            },
        )
        .expect("one retained event is a valid explicit test configuration");
        let capture = session.handle();

        session.thread_context().enter(|| {
            tracing::info!(message = "first event evicted by the second");
            tracing::info!(message = "retained event");
        });

        let _ = super::matching(&capture, "missing event");
    }

    /// An event's own value remains visible when its enclosing span has the same field name.
    #[test]
    fn event_fields_shadow_span_fields() {
        let event = super::CapturedEvent(EventSnapshot {
            sequence: 0,
            elapsed_micros: 0,
            target: "test".to_string(),
            level: "INFO".to_string(),
            fields: BTreeMap::from([("host".to_string(), "event-host".to_string())]),
            span_fields: BTreeMap::from([("host".to_string(), "span-host".to_string())]),
            truncated: false,
        });

        assert_eq!(event.field("host"), Some("event-host"));
    }
}
