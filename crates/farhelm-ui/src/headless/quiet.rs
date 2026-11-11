//! A tracing filter in front of the test's own capture, for the VirtualDom's
//! work only.
//!
//! `farhelm_testtrace::test` records every event at every level, which is
//! what makes a failing test's evidence complete. Dioxus, though, emits a
//! TRACE event or span for every signal subscription and diff step, many of
//! them carrying a `Debug` dump of whole node trees as fields. A headless
//! test renders the full app dozens of times, and formatting those fields
//! dominated its runtime (about 9 of 10 seconds for the first launcher
//! test). The page therefore drives the VirtualDom under [`Quiet`], which
//! drops TRACE-level callsites from other crates before their fields are
//! formatted and forwards everything else, this crate's events included, to
//! the capture unchanged.

use std::any::TypeId;
use tracing::{
    Event, Metadata, Subscriber,
    dispatcher::Dispatch,
    span::{Attributes, Id, Record},
    subscriber::Interest,
};

/// Forwards to `inner`, minus foreign TRACE noise.
pub struct Quiet {
    inner: Dispatch,
}

impl Quiet {
    /// Wrap whatever dispatcher is current: the test capture, inside a
    /// `farhelm_testtrace::test` body.
    pub fn current() -> Dispatch {
        let inner = tracing::dispatcher::get_default(Dispatch::clone);
        Dispatch::new(Quiet { inner })
    }

    fn keeps(metadata: &Metadata<'_>) -> bool {
        *metadata.level() < tracing::Level::TRACE || metadata.target().starts_with("farhelm")
    }
}

impl Subscriber for Quiet {
    fn register_callsite(&self, _metadata: &'static Metadata<'static>) -> Interest {
        // Callsite interest is cached process-wide across dispatchers, and
        // the capture outside this wrapper wants everything. "Sometimes"
        // makes tracing ask `enabled` per use instead.
        Interest::sometimes()
    }

    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        Self::keeps(metadata) && self.inner.enabled(metadata)
    }

    fn new_span(&self, span: &Attributes<'_>) -> Id {
        self.inner.new_span(span)
    }

    fn record(&self, span: &Id, values: &Record<'_>) {
        self.inner.record(span, values)
    }

    fn record_follows_from(&self, span: &Id, follows: &Id) {
        self.inner.record_follows_from(span, follows)
    }

    fn event(&self, event: &Event<'_>) {
        self.inner.event(event)
    }

    fn enter(&self, span: &Id) {
        self.inner.enter(span)
    }

    fn exit(&self, span: &Id) {
        self.inner.exit(span)
    }

    fn clone_span(&self, id: &Id) -> Id {
        self.inner.clone_span(id)
    }

    fn try_close(&self, id: Id) -> bool {
        self.inner.try_close(id)
    }

    unsafe fn downcast_raw(&self, id: TypeId) -> Option<*const ()> {
        // Only this wrapper is reachable this way. The capture's layers look
        // spans up through their own registry, never through the global
        // dispatcher, so not exposing the inner subscriber loses nothing.
        (id == TypeId::of::<Self>()).then_some(self as *const Self as *const ())
    }
}
