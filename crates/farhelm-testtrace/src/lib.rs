//! Bounded, test-owned `tracing` capture.
//!
//! This crate is deliberately a test dependency. Each capture owns its dispatcher and, for
//! asynchronous tests, its Tokio runtime. Concurrent libtest invocations can therefore use the
//! same tracing callsites without sharing events or installing a process-global subscriber.
//!
//! NOTE: capture is memory-only in this unit. Ordinary returns and unwinds can emit a bounded
//! diagnostic tail, but abort, kill, and power loss cannot retain it. Incremental persistence is
//! a separate unit so filesystem ownership does not leak into the attribution contract here.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::fmt::{self, Write as _};
use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use futures_util::FutureExt;
use serde::Serialize;
use tracing::dispatcher::{self, DefaultGuard, Dispatch};
use tracing::subscriber::NoSubscriber;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::registry::{LookupSpan, Registry};

pub use farhelm_testtrace_macros::test;

extern crate self as farhelm_testtrace;

/// Maximum events retained by one test capture.
pub const MAX_EVENTS: usize = 4_096;
/// Maximum collector-owned bytes retained for event field names and values.
pub const MAX_EVENT_BYTES: usize = 1024 * 1024;
/// Maximum collector-owned bytes retained for one field name and rendered value.
pub const MAX_FIELD_BYTES: usize = 4 * 1024;
/// Maximum collector-owned field bytes retained by one event, including inherited fields.
pub const MAX_EVENT_FIELD_BYTES: usize = 16 * 1024;
/// Maximum fields retained for one event or span.
pub const MAX_FIELDS: usize = 64;
/// Maximum collector-owned span-field bytes retained by one test.
pub const MAX_SPAN_BYTES: usize = 1024 * 1024;
/// Maximum live spans with collector-owned bookkeeping.
pub const MAX_SPANS: usize = 4_096;
/// Maximum collector-owned field bytes retained by one span.
pub const MAX_SPAN_FIELD_BYTES: usize = 16 * 1024;
/// Maximum bytes in the identity/outcome record of a failure dump.
pub const MAX_METADATA_RECORD_BYTES: usize = 4 * 1024;
/// Maximum bytes in one JSONL record in a failure dump.
pub const MAX_JSONL_RECORD_BYTES: usize = 16 * 1024;
/// Maximum bytes emitted as one failure dump.
pub const MAX_FAILURE_DUMP_BYTES: usize = 1024 * 1024;

const MIN_DIAGNOSTIC_RECORD_BYTES: usize = 768;
const FALLBACK_DIAGNOSTIC: &[u8] =
    br#"{"kind":"farhelm-testtrace","incomplete":true,"diagnostic_failure":true}"#;

/// Lowerable capture budgets whose defaults are the design's hard ceilings.
///
/// Validation rejects zero, over-ceiling, and cross-inconsistent values. The bounds cover retained
/// field payloads, counts, and encoded output, not total process memory. Registry storage, arbitrary
/// formatter allocations, and snapshots retained by callers have separate lifetimes and costs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureConfig {
    /// Maximum retained event count before oldest-first eviction begins.
    pub max_events: usize,
    /// Maximum aggregate bytes for retained event field names and values.
    pub max_event_bytes: usize,
    /// Maximum bytes for one retained field name and rendered value.
    pub max_field_bytes: usize,
    /// Maximum combined own and inherited field bytes in one event.
    pub max_event_field_bytes: usize,
    /// Maximum combined own and inherited fields in one event, or fields in one span.
    pub max_fields: usize,
    /// Maximum aggregate bytes for fields attached to live spans.
    pub max_span_bytes: usize,
    /// Maximum live spans with collector-owned bookkeeping.
    pub max_spans: usize,
    /// Maximum field bytes attached to one live span.
    pub max_span_field_bytes: usize,
    /// Maximum escaped bytes in the identity/outcome JSON record.
    pub max_metadata_record_bytes: usize,
    /// Maximum escaped bytes in any one JSONL record.
    pub max_jsonl_record_bytes: usize,
    /// Maximum bytes delivered for one complete failure dump.
    pub max_failure_dump_bytes: usize,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            max_events: MAX_EVENTS,
            max_event_bytes: MAX_EVENT_BYTES,
            max_field_bytes: MAX_FIELD_BYTES,
            max_event_field_bytes: MAX_EVENT_FIELD_BYTES,
            max_fields: MAX_FIELDS,
            max_span_bytes: MAX_SPAN_BYTES,
            max_spans: MAX_SPANS,
            max_span_field_bytes: MAX_SPAN_FIELD_BYTES,
            max_metadata_record_bytes: MAX_METADATA_RECORD_BYTES,
            max_jsonl_record_bytes: MAX_JSONL_RECORD_BYTES,
            max_failure_dump_bytes: MAX_FAILURE_DUMP_BYTES,
        }
    }
}

impl CaptureConfig {
    /// Validates both the hard ceilings and the containment relationships between budgets.
    pub fn validate(self) -> Result<Self, CaptureConfigError> {
        let values = [
            ("max_events", self.max_events, MAX_EVENTS),
            ("max_event_bytes", self.max_event_bytes, MAX_EVENT_BYTES),
            ("max_field_bytes", self.max_field_bytes, MAX_FIELD_BYTES),
            (
                "max_event_field_bytes",
                self.max_event_field_bytes,
                MAX_EVENT_FIELD_BYTES,
            ),
            ("max_fields", self.max_fields, MAX_FIELDS),
            ("max_span_bytes", self.max_span_bytes, MAX_SPAN_BYTES),
            ("max_spans", self.max_spans, MAX_SPANS),
            (
                "max_span_field_bytes",
                self.max_span_field_bytes,
                MAX_SPAN_FIELD_BYTES,
            ),
            (
                "max_metadata_record_bytes",
                self.max_metadata_record_bytes,
                MAX_METADATA_RECORD_BYTES,
            ),
            (
                "max_jsonl_record_bytes",
                self.max_jsonl_record_bytes,
                MAX_JSONL_RECORD_BYTES,
            ),
            (
                "max_failure_dump_bytes",
                self.max_failure_dump_bytes,
                MAX_FAILURE_DUMP_BYTES,
            ),
        ];
        for (name, value, ceiling) in values {
            if value == 0 {
                return Err(CaptureConfigError::Zero(name));
            }
            if value > ceiling {
                return Err(CaptureConfigError::AboveCeiling {
                    field: name,
                    value,
                    ceiling,
                });
            }
        }
        for (inner_name, inner, outer_name, outer) in [
            (
                "max_field_bytes",
                self.max_field_bytes,
                "max_event_field_bytes",
                self.max_event_field_bytes,
            ),
            (
                "max_field_bytes",
                self.max_field_bytes,
                "max_span_field_bytes",
                self.max_span_field_bytes,
            ),
            (
                "max_event_field_bytes",
                self.max_event_field_bytes,
                "max_event_bytes",
                self.max_event_bytes,
            ),
            (
                "max_span_field_bytes",
                self.max_span_field_bytes,
                "max_span_bytes",
                self.max_span_bytes,
            ),
            (
                "max_metadata_record_bytes",
                self.max_metadata_record_bytes,
                "max_jsonl_record_bytes",
                self.max_jsonl_record_bytes,
            ),
            (
                "max_jsonl_record_bytes",
                self.max_jsonl_record_bytes,
                "max_failure_dump_bytes",
                self.max_failure_dump_bytes,
            ),
        ] {
            if inner > outer {
                return Err(CaptureConfigError::ExceedsContainingBudget {
                    field: inner_name,
                    value: inner,
                    container: outer_name,
                    container_value: outer,
                });
            }
        }
        if self.max_metadata_record_bytes < MIN_DIAGNOSTIC_RECORD_BYTES
            || self.max_jsonl_record_bytes < MIN_DIAGNOSTIC_RECORD_BYTES
            || self.max_failure_dump_bytes < self.max_metadata_record_bytes + 1
        {
            return Err(CaptureConfigError::DiagnosticBudgetTooSmall);
        }
        Ok(self)
    }
}

/// A rejected capture budget or metadata identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CaptureConfigError {
    /// A boundary that cannot retain anything was requested.
    Zero(&'static str),
    /// A configured value exceeded the design's non-configurable ceiling.
    AboveCeiling {
        field: &'static str,
        value: usize,
        ceiling: usize,
    },
    /// A nested boundary was larger than the aggregate budget containing it.
    ExceedsContainingBudget {
        field: &'static str,
        value: usize,
        container: &'static str,
        container_value: usize,
    },
    /// Encoded-output limits cannot hold the mandatory diagnostic record.
    DiagnosticBudgetTooSmall,
    /// This test's exact escaped identity does not fit the configured metadata record.
    MetadataDoesNotFit { encoded_bytes: usize, budget: usize },
    /// Runtime metadata describes a combination Tokio cannot construct with this contract.
    InvalidRuntime(&'static str),
}

impl fmt::Display for CaptureConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zero(field) => write!(formatter, "{field} must be greater than zero"),
            Self::AboveCeiling {
                field,
                value,
                ceiling,
            } => {
                write!(
                    formatter,
                    "{field} is {value}, above the hard ceiling of {ceiling}"
                )
            }
            Self::ExceedsContainingBudget {
                field,
                value,
                container,
                container_value,
            } => write!(
                formatter,
                "{field} is {value}, greater than {container} ({container_value})"
            ),
            Self::DiagnosticBudgetTooSmall => write!(
                formatter,
                "diagnostic budgets must leave room for a useful identity and outcome record"
            ),
            Self::MetadataDoesNotFit {
                encoded_bytes,
                budget,
            } => write!(
                formatter,
                "encoded test identity needs {encoded_bytes} bytes, above the metadata budget of {budget}"
            ),
            Self::InvalidRuntime(reason) => write!(formatter, "invalid runtime metadata: {reason}"),
        }
    }
}

impl std::error::Error for CaptureConfigError {}

/// Tokio runtime shape preserved from the replaced Tokio test declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct RuntimeConfig {
    /// Scheduler selected by the source attribute.
    pub flavor: RuntimeFlavor,
    /// Explicit worker count for a multi-thread runtime.
    pub worker_threads: Option<usize>,
    /// Whether Tokio's clock begins paused.
    pub start_paused: bool,
}

/// The two Tokio scheduler shapes supported by the test attribute.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeFlavor {
    /// A scheduler driven by the libtest calling thread.
    CurrentThread,
    /// A scheduler with owned worker and blocking threads.
    MultiThread,
}

/// The source-level expected-panic declaration, kept separate from the observed outcome.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "expected", rename_all = "snake_case")]
pub enum ExpectedPanic<'a> {
    /// The test has no `should_panic` declaration.
    None,
    /// Any top-level panic satisfies the source declaration.
    Any,
    /// Libtest requires the panic display to contain this exact substring.
    Expected(Cow<'a, str>),
    /// A required substring was omitted because automatic metadata exceeded its encoded budget.
    ExpectedOmitted,
}

impl ExpectedPanic<'_> {
    fn is_declared(&self) -> bool {
        !matches!(self, Self::None)
    }

    /// Copies a borrowed declaration only after its enclosing metadata has passed validation.
    fn into_owned(self) -> ExpectedPanic<'static> {
        match self {
            Self::None => ExpectedPanic::None,
            Self::Any => ExpectedPanic::Any,
            Self::Expected(value) => ExpectedPanic::Expected(Cow::Owned(value.into_owned())),
            Self::ExpectedOmitted => ExpectedPanic::ExpectedOmitted,
        }
    }
}

/// Metadata supplied before a capture starts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TestMetadata<'a> {
    /// Module-qualified libtest name emitted by the attribute.
    pub name: Cow<'a, str>,
    /// Expected-panic declaration copied from the source item.
    pub expected_panic: ExpectedPanic<'a>,
    /// Owned runtime configuration, absent for synchronous capture.
    pub runtime: Option<RuntimeConfig>,
}

impl<'a> TestMetadata<'a> {
    /// Describes the exact libtest identity and declarations associated with a capture.
    ///
    /// Borrowed strings remain borrowed until CaptureSession validates their encoded size. Owned
    /// inputs are accepted too; their pre-existing allocations remain the caller's responsibility.
    pub fn new(
        name: impl Into<Cow<'a, str>>,
        expected_panic: ExpectedPanic<'a>,
        runtime: Option<RuntimeConfig>,
    ) -> Self {
        Self {
            name: name.into(),
            expected_panic,
            runtime,
        }
    }

    /// Retains validated identity text independently of the test declaration's borrow.
    fn into_owned(self) -> TestMetadata<'static> {
        TestMetadata {
            name: Cow::Owned(self.name.into_owned()),
            expected_panic: self.expected_panic.into_owned(),
            runtime: self.runtime,
        }
    }
}

/// What the wrapper directly observed before returning control to libtest.
///
/// This is not libtest's verdict. In particular, an unwind may satisfy `should_panic`, and a
/// returned success violates it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservedOutcome {
    /// The wrapped return value reports success through `TestOutcome`.
    ReturnedSuccess,
    /// The wrapped return value reports failure through `TestOutcome`.
    ReturnedFailure,
    /// Control left the wrapped body or runtime construction through an unwind.
    Unwind,
    /// The body returned, but its borrowed status observer panicked. Libtest still owns the value.
    ObservationFailed,
}

/// A captured tracing event with separately addressable event and inherited span fields.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct EventSnapshot {
    /// Monotonic sequence assigned when the collector admits the event.
    pub sequence: u64,
    /// Monotonic elapsed time from capture construction.
    pub elapsed_micros: u128,
    /// Bounded tracing target.
    pub target: String,
    /// Tracing level rendered with its conventional uppercase spelling.
    pub level: String,
    /// Fields recorded directly on the event.
    pub fields: BTreeMap<String, String>,
    /// Effective inherited fields: event fields shadow all spans, then inner spans shadow outer ones.
    pub span_fields: BTreeMap<String, String>,
    /// Whether this event or its inherited scope lost field evidence.
    pub truncated: bool,
}

/// Why an otherwise useful capture cannot support ordinary assertion matching.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct LossCounters {
    /// Oldest events removed to admit newer evidence.
    pub evicted_events: u64,
    /// Events that could not be admitted at all.
    pub dropped_events: u64,
    /// Field names or values discarded or shortened at any capture boundary.
    pub truncated_fields: u64,
    /// Spans denied some or all collector-owned storage.
    pub saturated_spans: u64,
    /// Reserved for the following persistence unit; it remains zero for memory-only sessions.
    pub persistence_failures: u64,
    /// Mutex poison, serialization fallback, or output-delivery failures.
    pub diagnostic_failures: u64,
    /// In-memory events omitted from this particular bounded dump.
    pub omitted_dump_events: u64,
}

impl LossCounters {
    /// Reports whether no collector or diagnostic boundary has discarded evidence.
    pub fn is_complete(&self) -> bool {
        self.evicted_events == 0
            && self.dropped_events == 0
            && self.truncated_fields == 0
            && self.saturated_spans == 0
            && self.persistence_failures == 0
            && self.diagnostic_failures == 0
            && self.omitted_dump_events == 0
    }

    fn add_field_loss(&mut self, count: u64) {
        self.truncated_fields = self.truncated_fields.saturating_add(count);
    }
}

/// A diagnostic snapshot, which may contain useful partial evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PartialSnapshot {
    /// Every event still retained in the bounded active window.
    pub events: Vec<EventSnapshot>,
    /// Permanent loss observed before this snapshot.
    pub loss: LossCounters,
    /// Direct indication that ordinary matching must refuse this window.
    pub incomplete: bool,
}

/// A complete active observation window suitable for normal assertion consumers.
///
/// Completeness describes retained evidence up to this snapshot. It does not imply that the test
/// or its runtime has finished.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompleteSnapshot(PartialSnapshot);

impl CompleteSnapshot {
    /// Returns every retained event in monotonic sequence order.
    pub fn events(&self) -> &[EventSnapshot] {
        &self.0.events
    }
}

/// The incomplete evidence returned instead of a misleading normal match result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SnapshotError {
    Incomplete(PartialSnapshot),
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Incomplete(snapshot) => {
                write!(
                    formatter,
                    "trace capture is incomplete: {:?}",
                    snapshot.loss
                )
            }
        }
    }
}

impl std::error::Error for SnapshotError {}

/// A handle to one independently configured trace capture.
#[derive(Clone)]
pub struct CaptureHandle(Arc<Collector>);

impl CaptureHandle {
    /// Takes a bounded diagnostic snapshot even when evidence was lost.
    pub fn partial_snapshot(&self) -> PartialSnapshot {
        self.0.snapshot()
    }

    /// Takes an assertion-safe snapshot or returns the partial evidence that made it unsafe.
    pub fn snapshot(&self) -> Result<CompleteSnapshot, SnapshotError> {
        let snapshot = self.partial_snapshot();
        if snapshot.loss.is_complete() {
            Ok(CompleteSnapshot(snapshot))
        } else {
            Err(SnapshotError::Incomplete(snapshot))
        }
    }

    /// Returns matching message events only when the active observation window is complete.
    pub fn matching(&self, needle: &str) -> Result<Vec<EventSnapshot>, SnapshotError> {
        self.matching_events(|event| {
            event
                .fields
                .get("message")
                .is_some_and(|message| message.contains(needle))
        })
    }

    /// Applies a structured predicate only after proving the active window complete.
    pub fn matching_events(
        &self,
        mut predicate: impl FnMut(&EventSnapshot) -> bool,
    ) -> Result<Vec<EventSnapshot>, SnapshotError> {
        let snapshot = self.snapshot()?;
        Ok(snapshot
            .events()
            .iter()
            .filter(|event| predicate(event))
            .cloned()
            .collect())
    }
}

thread_local! {
    static CURRENT_CONTEXT: RefCell<Vec<ThreadContext>> = const { RefCell::new(Vec::new()) };
    static RUNTIME_THREAD_GUARD: RefCell<Option<RuntimeThreadGuard>> = const { RefCell::new(None) };
}

// tracing caches callsite interest process-wide even though scoped dispatchers are thread-local.
// Keeping a second dispatcher alive forces the multi-dispatch path, so a callsite first observed
// on a foreign thread cannot become permanently disabled for later scoped captures. This value is
// never installed as the process-global default.
static SCOPED_DISPATCH_SENTINEL: LazyLock<Dispatch> =
    LazyLock::new(|| Dispatch::new(NoSubscriber::default()));

/// Returns the capture attributed to this thread's active test, if any.
pub fn current_capture() -> Option<CaptureHandle> {
    current_thread_context().map(|context| context.capture)
}

/// Returns an explicit context that a test may carry across a raw-thread boundary.
pub fn current_thread_context() -> Option<ThreadContext> {
    CURRENT_CONTEXT.with(|contexts| contexts.borrow().last().cloned())
}

/// Carries one test's dispatcher explicitly across a thread boundary.
///
/// Foreign runtimes and raw threads do not inherit this context. Callers must move a clone to the
/// foreign thread and enter it there when that work belongs to the test capture.
#[derive(Clone)]
pub struct ThreadContext {
    capture: CaptureHandle,
    dispatch: Dispatch,
}

impl ThreadContext {
    /// Runs `body` with this capture and dispatcher installed on the calling thread.
    pub fn enter<R>(&self, body: impl FnOnce() -> R) -> R {
        let _context = CurrentContext::push(self.clone());
        dispatcher::with_default(&self.dispatch, body)
    }

    /// Keeps the dispatcher installed until user thread-local destructors have finished.
    ///
    /// Initialize tracing's TLS and our context stack before the guard's TLS, and install the
    /// guard before Tokio runs user tasks. On supported Unix platforms TLS destructors run in
    /// reverse initialization order: user destructors can still log and inspect their context,
    /// then the guard restores the dispatcher while both backing TLS values remain available.
    /// Tokio's on_thread_stop hook runs too early to perform this cleanup.
    fn enter_runtime_thread(&self) {
        let guard = RuntimeThreadGuard {
            _dispatch: dispatcher::set_default(&self.dispatch),
            _context: CurrentContext::push(self.clone()),
        };
        initialize_registry_thread_locals();
        RUNTIME_THREAD_GUARD.with(|slot| {
            let previous = slot.borrow_mut().replace(guard);
            assert!(
                previous.is_none(),
                "a runtime thread must be initialized once"
            );
        });
    }
}

/// Initializes registry allocation and span-entry TLS before any user destructor can be registered.
///
/// Keeping the dispatcher alive is insufficient: sharded-slab and thread_local each register
/// their own thread-exit guards on first span use. A private bare registry initializes both
/// without adding an event, span fields, or admission pressure to the test's capture. Direct
/// Subscriber calls avoid compile-time logging filters suppressing this lifecycle prerequisite.
fn initialize_registry_thread_locals() {
    use tracing::Subscriber;

    static CALLSITE: tracing::callsite::DefaultCallsite =
        tracing::callsite::DefaultCallsite::new(&METADATA);
    static METADATA: tracing::Metadata<'static> = tracing::Metadata::new(
        "registry-thread-initialization",
        "farhelm_testtrace",
        tracing::Level::TRACE,
        None,
        None,
        None,
        tracing::field::FieldSet::new(&[], tracing::callsite::Identifier(&CALLSITE)),
        tracing::metadata::Kind::SPAN,
    );
    let registry = Registry::default();
    let values = METADATA.fields().value_set_all(&[]);
    let span = registry.new_span(&tracing::span::Attributes::new_root(&METADATA, &values));
    registry.enter(&span);
    registry.exit(&span);
    registry.try_close(span);
}

/// Owns runtime attribution through OS thread-local teardown, rather than Tokio's pre-exit hook.
struct RuntimeThreadGuard {
    _dispatch: DefaultGuard,
    _context: CurrentContext,
}

/// Observes whether a returned test value represents success without consuming or reporting it.
///
/// Custom `Termination` types must implement this trait explicitly. The wrapper never calls
/// `Termination::report`; libtest remains its sole caller. A panicking observer produces incomplete
/// diagnostics with `ObservationFailed`, then the original value is returned unchanged.
pub trait TestOutcome {
    /// Reports the return-value status without consuming it or invoking `Termination`.
    fn observed_success(&self) -> bool;
}

impl TestOutcome for () {
    fn observed_success(&self) -> bool {
        true
    }
}

impl<T: TestOutcome, E> TestOutcome for Result<T, E> {
    fn observed_success(&self) -> bool {
        self.as_ref().is_ok_and(TestOutcome::observed_success)
    }
}

impl TestOutcome for std::process::ExitCode {
    fn observed_success(&self) -> bool {
        *self == std::process::ExitCode::SUCCESS
    }
}

/// Owns one test's metadata, collector, and dispatcher for the capture's full lifetime.
pub struct CaptureSession {
    metadata: TestMetadata<'static>,
    identity: CaptureIdentity,
    collector: Arc<Collector>,
    dispatch: Dispatch,
    /// Automatic metadata loss must be reported even if the body later succeeds.
    metadata_incomplete: bool,
}

impl CaptureSession {
    /// Creates an independent capture after validating all memory and encoded-output bounds.
    pub fn new(
        metadata: TestMetadata<'_>,
        config: CaptureConfig,
    ) -> Result<Self, CaptureConfigError> {
        let config = config.validate()?;
        if let Some(runtime) = metadata.runtime {
            validate_runtime(runtime).map_err(CaptureConfigError::InvalidRuntime)?;
        }
        LazyLock::force(&SCOPED_DISPATCH_SENTINEL);
        let identity = CaptureIdentity::now();
        validate_metadata(&metadata, &identity, config)?;
        let metadata = metadata.into_owned();
        let collector = Arc::new(Collector::new(config));
        let dispatch = Dispatch::new(Registry::default().with(CaptureLayer(collector.clone())));
        Ok(Self {
            metadata,
            identity,
            collector,
            dispatch,
            metadata_incomplete: false,
        })
    }

    /// Returns a cloneable handle for snapshots and structured matching.
    pub fn handle(&self) -> CaptureHandle {
        CaptureHandle(self.collector.clone())
    }

    /// Returns the explicit dispatcher context used by raw and runtime-owned threads.
    pub fn thread_context(&self) -> ThreadContext {
        ThreadContext {
            capture: self.handle(),
            dispatch: self.dispatch.clone(),
        }
    }

    /// Generates a bounded JSONL dump without performing output I/O.
    pub fn failure_dump(&self, outcome: ObservedOutcome) -> FailureDump {
        self.collector
            .failure_dump(&self.metadata, &self.identity, outcome)
    }

    /// Writes a generated failure dump and records delivery failure without panicking.
    pub fn write_failure_dump(
        &self,
        outcome: ObservedOutcome,
        destination: &mut impl io::Write,
    ) -> io::Result<()> {
        let dump = self.failure_dump(outcome);
        match catch_unwind(AssertUnwindSafe(|| destination.write_all(dump.as_bytes()))) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => {
                self.collector.mark_diagnostic_failure();
                Err(error)
            }
            Err(payload) => {
                self.collector.mark_diagnostic_failure();
                discard_diagnostic_panic(payload);
                Err(io::Error::other("test tracing diagnostic writer panicked"))
            }
        }
    }

    fn should_dump(&self, outcome: ObservedOutcome) -> bool {
        !matches!(outcome, ObservedOutcome::ReturnedSuccess)
            || self.metadata.expected_panic.is_declared()
            || self.metadata_incomplete
    }

    fn finish(&self, outcome: ObservedOutcome) {
        if self.should_dump(outcome) {
            let mut stderr = io::stderr().lock();
            let _ = self.write_failure_dump(outcome, &mut stderr);
        }
    }
}

/// A fully generated JSONL diagnostic whose total and per-record bounds are already enforced.
#[derive(Clone, Debug)]
pub struct FailureDump {
    bytes: Vec<u8>,
    loss: LossCounters,
}

impl FailureDump {
    /// Returns the complete encoded diagnostic, including its trailing newlines.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns dump-local losses, including events omitted from the bounded output.
    pub fn loss(&self) -> &LossCounters {
        &self.loss
    }
}

/// Runs a synchronous test body without changing its return or libtest reporting behavior.
pub fn run_sync<R: TestOutcome>(
    name: &str,
    expected_panic: ExpectedPanic<'_>,
    body: impl FnOnce() -> R,
) -> R {
    let session = automatic_session(name, expected_panic, None);
    let context = session.thread_context();
    match context.enter(|| catch_unwind(AssertUnwindSafe(body))) {
        Ok(value) => {
            session.finish(returned_outcome(&value, &session));
            value
        }
        Err(payload) => {
            session.finish(ObservedOutcome::Unwind);
            resume_unwind(payload)
        }
    }
}

/// Runs an asynchronous test on an owned Tokio runtime and captures through runtime teardown.
pub fn run_async<R: TestOutcome>(
    name: &str,
    expected_panic: ExpectedPanic<'_>,
    runtime_config: RuntimeConfig,
    body: impl std::future::Future<Output = R>,
) -> R {
    validate_runtime(runtime_config)
        .expect("the test attribute must generate a valid Tokio runtime configuration");
    let session = automatic_session(name, expected_panic, Some(runtime_config));
    let context = session.thread_context();
    let result = context.enter(|| {
        catch_unwind(AssertUnwindSafe(|| {
            let runtime = build_runtime(runtime_config, context.clone());
            let result = runtime.block_on(AssertUnwindSafe(body).catch_unwind());
            // Dropping before the session is finished keeps cancellation and thread-stop events in
            // the same bounded window that the eventual failure dump reads.
            drop(runtime);
            result
        }))
    });
    match result {
        Ok(Ok(value)) => {
            session.finish(returned_outcome(&value, &session));
            value
        }
        Ok(Err(payload)) | Err(payload) => {
            session.finish(ObservedOutcome::Unwind);
            resume_unwind(payload)
        }
    }
}

/// Preserves valid libtest execution when its identity cannot fit automatic diagnostic metadata.
///
/// Explicit CaptureSession construction still rejects oversized metadata. The attribute cannot
/// impose that restriction on valid function names or should_panic literals, so its wrappers
/// substitute a fixed identity and mark permanent diagnostic loss. Libtest's original attribute
/// remains untouched; an omitted expected substring is never presented as an exact pattern.
fn automatic_session(
    name: &str,
    expected_panic: ExpectedPanic<'_>,
    runtime: Option<RuntimeConfig>,
) -> CaptureSession {
    let fallback_panic = match &expected_panic {
        ExpectedPanic::None => ExpectedPanic::None,
        ExpectedPanic::Any => ExpectedPanic::Any,
        ExpectedPanic::Expected(_) | ExpectedPanic::ExpectedOmitted => {
            ExpectedPanic::ExpectedOmitted
        }
    };
    let metadata = TestMetadata::new(name, expected_panic, runtime);
    match CaptureSession::new(metadata, CaptureConfig::default()) {
        Ok(session) => session,
        Err(CaptureConfigError::MetadataDoesNotFit { .. }) => {
            let mut session = CaptureSession::new(
                TestMetadata::new("<test metadata omitted>", fallback_panic, runtime),
                CaptureConfig::default(),
            )
            .expect("fixed fallback metadata fits default capture budgets");
            session.metadata_incomplete = true;
            session.collector.mark_diagnostic_failure();
            session
        }
        Err(error) => panic!("invalid automatic capture configuration: {error}"),
    }
}

/// Observes a borrowed return value without granting diagnostics control over libtest's outcome.
fn returned_outcome(value: &impl TestOutcome, session: &CaptureSession) -> ObservedOutcome {
    match catch_unwind(AssertUnwindSafe(|| value.observed_success())) {
        Ok(true) => ObservedOutcome::ReturnedSuccess,
        Ok(false) => ObservedOutcome::ReturnedFailure,
        Err(payload) => {
            session.collector.mark_diagnostic_failure();
            discard_diagnostic_panic(payload);
            ObservedOutcome::ObservationFailed
        }
    }
}

/// Disposes of ordinary panic messages without executing an unknown payload's destructor.
///
/// Diagnostic failures must not introduce a second unwind, particularly when called from Drop
/// during the test's original unwind. String payloads have known non-panicking destructors;
/// arbitrary panic_any payloads are deliberately leaked because their destructors are user code.
/// Such payload allocations, like arbitrary formatter allocations, are outside capture bounds.
fn discard_diagnostic_panic(payload: Box<dyn std::any::Any + Send>) {
    if payload.is::<String>() || payload.is::<&'static str>() {
        drop(payload);
    } else {
        std::mem::forget(payload);
    }
}

/// Defends the public wrapper against configurations the attribute would reject at compile time.
fn validate_runtime(config: RuntimeConfig) -> Result<(), &'static str> {
    if config.worker_threads == Some(0) {
        return Err("worker_threads must be at least one");
    }
    if config.worker_threads.is_some() && config.flavor != RuntimeFlavor::MultiThread {
        return Err("worker_threads requires the multi_thread flavor");
    }
    if config.start_paused && config.flavor != RuntimeFlavor::CurrentThread {
        return Err("start_paused requires the current_thread flavor");
    }
    Ok(())
}

/// Builds only configurations the macro has already validated.
fn build_runtime(config: RuntimeConfig, context: ThreadContext) -> tokio::runtime::Runtime {
    let mut builder = match config.flavor {
        RuntimeFlavor::CurrentThread => tokio::runtime::Builder::new_current_thread(),
        RuntimeFlavor::MultiThread => tokio::runtime::Builder::new_multi_thread(),
    };
    builder.enable_all();
    if let Some(workers) = config.worker_threads {
        builder.worker_threads(workers);
    }
    builder.start_paused(config.start_paused);
    let started = context.clone();
    builder.on_thread_start(move || started.enter_runtime_thread());
    builder
        .build()
        .expect("farhelm test tracing could not build the requested Tokio runtime")
}

/// Pops only the explicit thread context pushed by this lexical scope.
struct CurrentContext;

impl CurrentContext {
    fn push(context: ThreadContext) -> Self {
        CURRENT_CONTEXT.with(|contexts| contexts.borrow_mut().push(context));
        Self
    }
}

impl Drop for CurrentContext {
    fn drop(&mut self) {
        CURRENT_CONTEXT.with(|contexts| {
            contexts.borrow_mut().pop();
        });
    }
}

#[derive(Clone, Debug, Serialize)]
struct CaptureIdentity {
    process_id: u32,
    started_unix_micros: u128,
}

impl CaptureIdentity {
    /// Captures process and wall-clock identity once so every dump from a session agrees.
    fn now() -> Self {
        Self {
            process_id: std::process::id(),
            started_unix_micros: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_micros(),
        }
    }
}

#[derive(Serialize)]
struct DumpMetadata<'a> {
    kind: &'static str,
    test: &'a TestMetadata<'a>,
    identity: &'a CaptureIdentity,
    outcome: ObservedOutcome,
    loss: &'a LossCounters,
}

/// Validates the exact escaped identity record with worst-case outcome and counter widths.
fn validate_metadata(
    metadata: &TestMetadata<'_>,
    identity: &CaptureIdentity,
    config: CaptureConfig,
) -> Result<(), CaptureConfigError> {
    let maximum_loss = LossCounters {
        evicted_events: u64::MAX,
        dropped_events: u64::MAX,
        truncated_fields: u64::MAX,
        saturated_spans: u64::MAX,
        persistence_failures: u64::MAX,
        diagnostic_failures: u64::MAX,
        omitted_dump_events: u64::MAX,
    };
    let record = DumpMetadata {
        kind: "farhelm-testtrace",
        test: metadata,
        identity,
        outcome: ObservedOutcome::ObservationFailed,
        loss: &maximum_loss,
    };
    match encode_bounded(&record, config.max_metadata_record_bytes) {
        Ok(_) => Ok(()),
        Err(_) => Err(CaptureConfigError::MetadataDoesNotFit {
            encoded_bytes: encoded_size(&record),
            budget: config.max_metadata_record_bytes,
        }),
    }
}

/// Collector state carries redundant byte totals so hot-path bounds stay constant-time.
///
/// The per-span map makes those totals reconstructible after mutex poison. Poison permanently
/// marks the window incomplete because repair cannot prove that an interrupted mutation retained
/// every event.
struct Collector {
    started: Instant,
    config: CaptureConfig,
    state: Mutex<CollectorState>,
}

/// Mutable accounting kept behind one mutex so every admitted record and loss is atomic.
struct CollectorState {
    events: VecDeque<(usize, EventSnapshot)>,
    event_bytes: usize,
    span_allocations: BTreeMap<u64, usize>,
    span_bytes: usize,
    next_sequence: u64,
    loss: LossCounters,
    poison_observed: bool,
}

impl Collector {
    fn new(config: CaptureConfig) -> Self {
        Self {
            started: Instant::now(),
            config,
            state: Mutex::new(CollectorState {
                events: VecDeque::new(),
                event_bytes: 0,
                span_allocations: BTreeMap::new(),
                span_bytes: 0,
                next_sequence: 0,
                loss: LossCounters::default(),
                poison_observed: false,
            }),
        }
    }

    /// Recovers bounded accounting from retained records after poison and marks the loss once.
    fn state(&self) -> MutexGuard<'_, CollectorState> {
        match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => {
                let mut state = poisoned.into_inner();
                state.event_bytes = state.events.iter().map(|(bytes, _)| bytes).sum();
                state.span_bytes = state.span_allocations.values().sum();
                if !state.poison_observed {
                    state.poison_observed = true;
                    state.loss.diagnostic_failures =
                        state.loss.diagnostic_failures.saturating_add(1);
                }
                state
            }
        }
    }

    /// Records an internal or delivery failure without tracing recursively through this layer.
    fn mark_diagnostic_failure(&self) {
        let mut state = self.state();
        state.loss.diagnostic_failures = state.loss.diagnostic_failures.saturating_add(1);
    }

    /// Makes field loss visible as soon as the boundary rejects data, even without a later event.
    fn mark_field_loss(&self, count: u64) {
        if count > 0 {
            self.state().loss.add_field_loss(count);
        }
    }

    /// Clones only retained bounded records and computes the explicit incomplete flag.
    fn snapshot(&self) -> PartialSnapshot {
        let state = self.state();
        let loss = state.loss.clone();
        PartialSnapshot {
            events: state
                .events
                .iter()
                .map(|(_, event)| event.clone())
                .collect(),
            incomplete: !loss.is_complete(),
            loss,
        }
    }

    /// Retains a fully bounded event, evicting the oldest records before admitting it.
    fn push_event(
        &self,
        target: String,
        level: String,
        fields: BTreeMap<String, String>,
        span_fields: BTreeMap<String, String>,
        truncated: bool,
    ) {
        let bytes = map_bytes(&fields).saturating_add(map_bytes(&span_fields));
        let mut state = self.state();
        if bytes > self.config.max_event_field_bytes || bytes > self.config.max_event_bytes {
            state.loss.dropped_events = state.loss.dropped_events.saturating_add(1);
            return;
        }
        while state.events.len() >= self.config.max_events
            || state.event_bytes.saturating_add(bytes) > self.config.max_event_bytes
        {
            let Some((removed, _)) = state.events.pop_front() else {
                state.loss.dropped_events = state.loss.dropped_events.saturating_add(1);
                return;
            };
            state.event_bytes = state.event_bytes.saturating_sub(removed);
            state.loss.evicted_events = state.loss.evicted_events.saturating_add(1);
        }
        let sequence = state.next_sequence;
        state.next_sequence = state.next_sequence.saturating_add(1);
        state.event_bytes = state.event_bytes.saturating_add(bytes);
        state.events.push_back((
            bytes,
            EventSnapshot {
                sequence,
                elapsed_micros: self.started.elapsed().as_micros(),
                target,
                level,
                fields,
                span_fields,
                truncated,
            },
        ));
    }

    /// Encodes newest-fitting evidence while reserving the metadata record's full budget.
    fn failure_dump(
        &self,
        metadata: &TestMetadata<'_>,
        identity: &CaptureIdentity,
        outcome: ObservedOutcome,
    ) -> FailureDump {
        let snapshot = self.snapshot();
        let event_budget = self
            .config
            .max_failure_dump_bytes
            .saturating_sub(self.config.max_metadata_record_bytes + 1);
        let mut retained = VecDeque::new();
        let mut retained_bytes = 0_usize;
        let mut encoded_loss = 0_u64;
        for event in snapshot.events.iter().rev() {
            let (record, lost_fields) = encode_event(event, self.config.max_jsonl_record_bytes);
            let record_bytes = record.len().saturating_add(1);
            if retained_bytes.saturating_add(record_bytes) > event_budget {
                continue;
            }
            retained_bytes += record_bytes;
            encoded_loss = encoded_loss.saturating_add(lost_fields);
            retained.push_front(record);
        }
        let mut loss = snapshot.loss;
        loss.add_field_loss(encoded_loss);
        loss.omitted_dump_events = loss
            .omitted_dump_events
            .saturating_add(snapshot.events.len().saturating_sub(retained.len()) as u64);
        let metadata_record = DumpMetadata {
            kind: "farhelm-testtrace",
            test: metadata,
            identity,
            outcome,
            loss: &loss,
        };
        let metadata_bytes =
            match encode_bounded(&metadata_record, self.config.max_metadata_record_bytes) {
                Ok(bytes) => bytes,
                Err(_) => {
                    self.mark_diagnostic_failure();
                    FALLBACK_DIAGNOSTIC[..FALLBACK_DIAGNOSTIC
                        .len()
                        .min(self.config.max_metadata_record_bytes)]
                        .to_vec()
                }
            };
        let mut bytes = Vec::with_capacity(
            metadata_bytes
                .len()
                .saturating_add(1)
                .saturating_add(retained_bytes),
        );
        bytes.extend_from_slice(&metadata_bytes);
        bytes.push(b'\n');
        for record in retained {
            bytes.extend_from_slice(&record);
            bytes.push(b'\n');
        }
        debug_assert!(bytes.len() <= self.config.max_failure_dump_bytes);
        FailureDump { bytes, loss }
    }
}

/// Per-span extensions retain only collector-owned, already-bounded fields.
struct SpanFields {
    fields: BTreeMap<String, String>,
    bytes: usize,
    incomplete: bool,
    aggregate_saturated: bool,
}

/// The capture layer enforces bounds before data enters collector-owned storage.
struct CaptureLayer(Arc<Collector>);

impl<S> Layer<S> for CaptureLayer
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(
        &self,
        attributes: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        context: Context<'_, S>,
    ) {
        let Some(span) = context.span(id) else {
            return;
        };
        let config = self.0.config;
        let mut collected = collect_fields(
            config.max_fields,
            config.max_field_bytes,
            config.max_span_field_bytes,
            |visitor| attributes.record(visitor),
        );
        let span_key = id.clone().into_u64();
        let mut state = self.0.state();
        if state.span_allocations.len() >= config.max_spans {
            state.loss.saturated_spans = state.loss.saturated_spans.saturating_add(1);
            state
                .loss
                .add_field_loss(collected.lost.saturating_add(collected.fields.len() as u64));
            return;
        }
        let available = config.max_span_bytes.saturating_sub(state.span_bytes);
        let aggregate_loss = trim_fields_to_budget(&mut collected.fields, available);
        collected.bytes = map_bytes(&collected.fields);
        collected.lost = collected.lost.saturating_add(aggregate_loss);
        let aggregate_saturated = aggregate_loss > 0;
        if aggregate_saturated {
            state.loss.saturated_spans = state.loss.saturated_spans.saturating_add(1);
        }
        state.loss.add_field_loss(collected.lost);
        state.span_bytes = state.span_bytes.saturating_add(collected.bytes);
        state.span_allocations.insert(span_key, collected.bytes);
        drop(state);
        span.extensions_mut().insert(SpanFields {
            fields: collected.fields,
            bytes: collected.bytes,
            incomplete: collected.lost > 0,
            aggregate_saturated,
        });
    }

    fn on_record(
        &self,
        id: &tracing::span::Id,
        values: &tracing::span::Record<'_>,
        context: Context<'_, S>,
    ) {
        let Some(span) = context.span(id) else {
            return;
        };
        let config = self.0.config;
        let updates = collect_fields(
            config.max_fields,
            config.max_field_bytes,
            config.max_span_field_bytes,
            |visitor| values.record(visitor),
        );
        let mut extensions = span.extensions_mut();
        let Some(stored) = extensions.get_mut::<SpanFields>() else {
            let missing = updates
                .lost
                .saturating_add(updates.fields.len() as u64)
                .max(1);
            self.0.mark_field_loss(missing);
            return;
        };
        let span_key = id.clone().into_u64();
        let mut state = self.0.state();
        let mut lost = updates.lost;

        for (key, mut value) in updates.fields {
            let old_bytes = stored
                .fields
                .get(&key)
                .map_or(0, |old| entry_bytes(&key, old));
            if old_bytes == 0 && stored.fields.len() >= config.max_fields {
                lost = lost.saturating_add(1);
                continue;
            }
            let span_base = stored.bytes.saturating_sub(old_bytes);
            let aggregate_base = state.span_bytes.saturating_sub(old_bytes);
            let span_available = config.max_span_field_bytes.saturating_sub(span_base);
            let aggregate_available = config.max_span_bytes.saturating_sub(aggregate_base);
            let available = span_available.min(aggregate_available);
            let aggregate_limited = aggregate_available < span_available
                && entry_bytes(&key, &value) > aggregate_available;
            if aggregate_limited && !stored.aggregate_saturated {
                stored.aggregate_saturated = true;
                state.loss.saturated_spans = state.loss.saturated_spans.saturating_add(1);
            }
            if key.len() > available {
                lost = lost.saturating_add(1);
                continue;
            }
            let value_budget = available - key.len();
            if value.len() > value_budget {
                value = truncate_owned(value, value_budget);
                lost = lost.saturating_add(1);
            }
            let new_bytes = entry_bytes(&key, &value);
            stored.fields.insert(key, value);
            stored.bytes = span_base.saturating_add(new_bytes);
            state.span_bytes = aggregate_base.saturating_add(new_bytes);
        }
        stored.incomplete |= lost > 0;
        state.loss.add_field_loss(lost);
        state.span_allocations.insert(span_key, stored.bytes);
    }

    fn on_event(&self, event: &tracing::Event<'_>, context: Context<'_, S>) {
        let config = self.0.config;
        let collected = collect_fields(
            config.max_fields,
            config.max_field_bytes,
            config.max_event_field_bytes,
            |visitor| event.record(visitor),
        );
        let fields = collected.fields;
        let mut span_fields = BTreeMap::new();
        let mut bytes = map_bytes(&fields);
        let mut lost = collected.lost;
        let mut inherited_incomplete = false;

        // Event fields already own their keys. Scope iterates leaf-to-root, so skipping both event
        // and retained span keys gives effective-field precedence before charging either budget.
        if let Some(scope) = context.event_scope(event) {
            for span in scope {
                if let Some(stored) = span.extensions().get::<SpanFields>() {
                    inherited_incomplete |= stored.incomplete;
                    for (key, value) in &stored.fields {
                        if fields.contains_key(key) || span_fields.contains_key(key) {
                            continue;
                        }
                        if fields.len().saturating_add(span_fields.len()) >= config.max_fields {
                            lost = lost.saturating_add(1);
                            continue;
                        }
                        let available = config.max_event_field_bytes.saturating_sub(bytes);
                        if key.len() > available {
                            lost = lost.saturating_add(1);
                            continue;
                        }
                        let value_budget = available - key.len();
                        let inherited = if value.len() > value_budget {
                            lost = lost.saturating_add(1);
                            truncate_str(value, value_budget).to_owned()
                        } else {
                            value.clone()
                        };
                        bytes = bytes.saturating_add(entry_bytes(key, &inherited));
                        span_fields.insert(key.clone(), inherited);
                    }
                } else {
                    inherited_incomplete = true;
                    lost = lost.saturating_add(1);
                }
            }
        }
        if lost > 0 {
            self.0.mark_field_loss(lost);
        }
        let (target, target_lost) = bounded_text(event.metadata().target(), config.max_field_bytes);
        if target_lost {
            self.0.mark_field_loss(1);
        }
        self.0.push_event(
            target,
            event.metadata().level().to_string(),
            fields,
            span_fields,
            lost > 0 || inherited_incomplete || target_lost,
        );
    }

    fn on_close(&self, id: tracing::span::Id, context: Context<'_, S>) {
        if let Some(span) = context.span(&id)
            && let Some(stored) = span.extensions_mut().remove::<SpanFields>()
        {
            let mut state = self.0.state();
            let recorded = state
                .span_allocations
                .remove(&id.into_u64())
                .unwrap_or(stored.bytes);
            state.span_bytes = state.span_bytes.saturating_sub(recorded);
        }
    }
}

/// Bounded field collection tracks names as well as values, so many empty fields still consume
/// both the count and aggregate byte budgets.
struct CollectedFields {
    fields: BTreeMap<String, String>,
    bytes: usize,
    lost: u64,
}

/// Records directly into a field set with both per-entry and aggregate construction bounds.
fn collect_fields(
    max_fields: usize,
    max_field_bytes: usize,
    max_total_bytes: usize,
    record: impl FnOnce(&mut FieldVisitor),
) -> CollectedFields {
    let mut visitor = FieldVisitor::new(max_fields, max_field_bytes, max_total_bytes);
    record(&mut visitor);
    CollectedFields {
        fields: visitor.fields,
        bytes: visitor.bytes,
        lost: visitor.lost,
    }
}

/// Writes arbitrary `Debug` output directly into its final bounded field buffer.
struct FieldVisitor {
    fields: BTreeMap<String, String>,
    bytes: usize,
    lost: u64,
    max_fields: usize,
    max_field_bytes: usize,
    max_total_bytes: usize,
}

impl FieldVisitor {
    /// Starts a visitor whose eventual map cannot exceed any of its three field boundaries.
    fn new(max_fields: usize, max_field_bytes: usize, max_total_bytes: usize) -> Self {
        Self {
            fields: BTreeMap::new(),
            bytes: 0,
            lost: 0,
            max_fields,
            max_field_bytes,
            max_total_bytes,
        }
    }

    /// Renders into the retained budget, containing formatter failure before publishing its prefix.
    fn insert(&mut self, field: &tracing::field::Field, value: impl fmt::Display) {
        let (key, key_lost) = bounded_text(field.name(), self.max_field_bytes);
        self.lost = self.lost.saturating_add(u64::from(key_lost));
        let old_bytes = self
            .fields
            .get(&key)
            .map_or(0, |old| entry_bytes(&key, old));
        if old_bytes == 0 && self.fields.len() >= self.max_fields {
            self.lost = self.lost.saturating_add(1);
            return;
        }
        let base = self.bytes.saturating_sub(old_bytes);
        if key.len() > self.max_total_bytes.saturating_sub(base) {
            self.lost = self.lost.saturating_add(1);
            return;
        }
        let value_budget = self
            .max_field_bytes
            .saturating_sub(key.len())
            .min(self.max_total_bytes.saturating_sub(base + key.len()));
        let mut rendered = CappedWrite::new(value_budget);
        let formatting_failed =
            match catch_unwind(AssertUnwindSafe(|| write!(&mut rendered, "{value}"))) {
                Ok(result) => result.is_err(),
                Err(payload) => {
                    discard_diagnostic_panic(payload);
                    true
                }
            };
        self.lost = self
            .lost
            .saturating_add(u64::from(rendered.was_truncated() || formatting_failed));
        let rendered = rendered.into_string();
        self.bytes = base.saturating_add(entry_bytes(&key, &rendered));
        self.fields.insert(key, rendered);
    }
}

impl tracing::field::Visit for FieldVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
        self.insert(field, format_args!("{value:?}"));
    }
    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.insert(field, value);
    }
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.insert(field, value);
    }
    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.insert(field, value);
    }
    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.insert(field, value);
    }
    fn record_i128(&mut self, field: &tracing::field::Field, value: i128) {
        self.insert(field, value);
    }
    fn record_u128(&mut self, field: &tracing::field::Field, value: u128) {
        self.insert(field, value);
    }
    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.insert(field, value);
    }
}

/// A `fmt::Write` that refuses the first byte beyond its UTF-8-safe cap.
///
/// Cooperative formatters stop on the returned error. A hostile implementation can ignore that
/// result or allocate internally; those allocations are outside the collector-owned bound.
struct CappedWrite {
    value: String,
    cap: usize,
    truncated: bool,
}

impl CappedWrite {
    /// Reserves at most the configured field cap rather than an input-sized buffer.
    fn new(cap: usize) -> Self {
        Self {
            value: String::new(),
            cap,
            truncated: false,
        }
    }
    /// Reports that at least one formatter write crossed the retained boundary.
    fn was_truncated(&self) -> bool {
        self.truncated
    }
    /// Returns the valid UTF-8 prefix admitted before the first rejected write.
    fn into_string(self) -> String {
        self.value
    }
}

impl fmt::Write for CappedWrite {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if value.is_empty() {
            return Ok(());
        }
        if self.truncated {
            return Err(fmt::Error);
        }
        let remaining = self.cap.saturating_sub(self.value.len());
        if remaining == 0 {
            self.truncated = true;
            return Err(fmt::Error);
        }
        let end = utf8_prefix_len(value, remaining);
        self.value.push_str(&value[..end]);
        if end < value.len() {
            self.truncated = true;
            Err(fmt::Error)
        } else {
            Ok(())
        }
    }
}

/// An `io::Write` used by serde that never grows beyond the encoded-record cap.
struct BoundedBytes {
    bytes: Vec<u8>,
    cap: usize,
}

impl BoundedBytes {
    /// Caps allocation at the hard JSON record ceiling even for an invalid larger request.
    fn new(cap: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(cap.min(MAX_JSONL_RECORD_BYTES)),
            cap,
        }
    }
}

impl io::Write for BoundedBytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.bytes.len().saturating_add(bytes.len()) > self.cap {
            return Err(io::Error::other(
                "encoded trace record exceeded its byte cap",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Serializes through a refusing writer so JSON escaping cannot cross the encoded-byte cap.
fn encode_bounded(value: &impl Serialize, cap: usize) -> Result<Vec<u8>, usize> {
    let mut writer = BoundedBytes::new(cap);
    match serde_json::to_writer(&mut writer, value) {
        // A short record must not keep the full cap while thousands of peers await assembly.
        // Boxed storage removes spare capacity before returning a retainable record vector.
        Ok(()) => Ok(writer.bytes.into_boxed_slice().into_vec()),
        Err(_) => Err(cap.saturating_add(1)),
    }
}

/// Counts an encoded record without retaining it, used only to explain rejected metadata.
fn encoded_size(value: &impl Serialize) -> usize {
    struct CountingWriter(usize);
    impl io::Write for CountingWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0 = self.0.saturating_add(bytes.len());
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut writer = CountingWriter(0);
    if serde_json::to_writer(&mut writer, value).is_err() {
        usize::MAX
    } else {
        writer.0
    }
}

/// A dump-only copy can discard fields to account for JSON escaping without changing capture.
fn encode_event(event: &EventSnapshot, cap: usize) -> (Vec<u8>, u64) {
    let mut bounded = event.clone();
    let mut lost = 0_u64;
    loop {
        if let Ok(bytes) = encode_bounded(&bounded, cap) {
            return (bytes, lost);
        }
        bounded.truncated = true;
        if let Some(key) = bounded.span_fields.keys().next_back().cloned() {
            bounded.span_fields.remove(&key);
            lost = lost.saturating_add(1);
            continue;
        }
        if let Some(key) = bounded
            .fields
            .keys()
            .rev()
            .find(|key| key.as_str() != "message")
            .cloned()
        {
            bounded.fields.remove(&key);
            lost = lost.saturating_add(1);
            continue;
        }
        if let Some(message) = bounded.fields.get_mut("message")
            && !message.is_empty()
        {
            *message = truncate_str(message, message.len() / 2).to_owned();
            lost = lost.saturating_add(1);
            continue;
        }
        if !bounded.target.is_empty() {
            bounded.target = truncate_str(&bounded.target, bounded.target.len() / 2).to_owned();
            lost = lost.saturating_add(1);
            continue;
        }
        return (
            FALLBACK_DIAGNOSTIC[..FALLBACK_DIAGNOSTIC.len().min(cap)].to_vec(),
            lost.saturating_add(1),
        );
    }
}

/// Applies a later aggregate boundary to an already bounded per-span field set.
fn trim_fields_to_budget(fields: &mut BTreeMap<String, String>, budget: usize) -> u64 {
    let mut lost = 0_u64;
    while map_bytes(fields) > budget {
        let Some(key) = fields.keys().next_back().cloned() else {
            break;
        };
        fields.remove(&key);
        lost = lost.saturating_add(1);
    }
    lost
}

/// Copies only a UTF-8-safe prefix and reports whether evidence was shortened.
fn bounded_text(value: &str, cap: usize) -> (String, bool) {
    let prefix = truncate_str(value, cap);
    (prefix.to_owned(), prefix.len() != value.len())
}

/// Reuses an already bounded allocation when it fits and otherwise returns a valid prefix.
fn truncate_owned(value: String, cap: usize) -> String {
    if value.len() <= cap {
        value
    } else {
        truncate_str(&value, cap).to_owned()
    }
}

/// Borrows the largest valid UTF-8 prefix within a byte cap.
fn truncate_str(value: &str, cap: usize) -> &str {
    &value[..utf8_prefix_len(value, cap)]
}

/// Finds the largest UTF-8-safe prefix without allocating or examining bytes past the cap.
fn utf8_prefix_len(value: &str, cap: usize) -> usize {
    if value.len() <= cap {
        return value.len();
    }
    let mut end = cap;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    end
}

/// Counts the collector-owned string payload for one map entry.
fn entry_bytes(key: &str, value: &str) -> usize {
    key.len().saturating_add(value.len())
}

/// Reconstructs aggregate payload accounting for validation and poison repair.
fn map_bytes(fields: &BTreeMap<String, String>) -> usize {
    fields
        .iter()
        .map(|(key, value)| entry_bytes(key, value))
        .sum()
}

#[cfg(test)]
mod tests {
    use std::fmt;
    use std::fmt::Write as _;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::Duration;

    use super::*;

    /// Builds the ordinary synchronous identity used by direct-session contract tests.
    fn metadata(name: &str) -> TestMetadata<'_> {
        TestMetadata::new(name, ExpectedPanic::None, None)
    }

    /// Keeps boundary tests cheap while retaining valid diagnostic-output defaults.
    fn small_config() -> CaptureConfig {
        CaptureConfig {
            max_events: 4,
            max_event_bytes: 256,
            max_field_bytes: 32,
            max_event_field_bytes: 96,
            max_fields: 4,
            max_span_bytes: 96,
            max_span_field_bytes: 48,
            ..CaptureConfig::default()
        }
    }

    /// Gives attribution tests one process-wide callsite whose dispatcher must remain dynamic.
    fn shared_runtime_callsite(owner: &'static str) {
        tracing::info!(owner, "shared runtime callsite");
    }

    /// Defaults pin every hard ceiling so a later refactor cannot weaken normal wrapper wiring.
    #[::core::prelude::v1::test]
    fn default_configuration_matches_the_design_ceilings() {
        assert_eq!(
            CaptureConfig::default(),
            CaptureConfig {
                max_events: MAX_EVENTS,
                max_event_bytes: MAX_EVENT_BYTES,
                max_field_bytes: MAX_FIELD_BYTES,
                max_event_field_bytes: MAX_EVENT_FIELD_BYTES,
                max_fields: MAX_FIELDS,
                max_span_bytes: MAX_SPAN_BYTES,
                max_spans: MAX_SPANS,
                max_span_field_bytes: MAX_SPAN_FIELD_BYTES,
                max_metadata_record_bytes: MAX_METADATA_RECORD_BYTES,
                max_jsonl_record_bytes: MAX_JSONL_RECORD_BYTES,
                max_failure_dump_bytes: MAX_FAILURE_DUMP_BYTES,
            }
        );
    }

    /// Invalid and cross-inconsistent lower bounds fail before a capture becomes observable.
    #[::core::prelude::v1::test]
    fn configuration_rejects_invalid_budget_relationships() {
        let zero = CaptureConfig {
            max_events: 0,
            ..CaptureConfig::default()
        };
        assert!(matches!(zero.validate(), Err(CaptureConfigError::Zero(_))));

        let above = CaptureConfig {
            max_fields: MAX_FIELDS + 1,
            ..CaptureConfig::default()
        };
        assert!(matches!(
            above.validate(),
            Err(CaptureConfigError::AboveCeiling { .. })
        ));

        let crossed = CaptureConfig {
            max_event_bytes: 8,
            max_event_field_bytes: 9,
            ..CaptureConfig::default()
        };
        assert!(matches!(
            crossed.validate(),
            Err(CaptureConfigError::ExceedsContainingBudget { .. })
        ));

        let diagnostic = CaptureConfig {
            max_metadata_record_bytes: MIN_DIAGNOSTIC_RECORD_BYTES - 1,
            ..CaptureConfig::default()
        };
        assert_eq!(
            diagnostic.validate(),
            Err(CaptureConfigError::DiagnosticBudgetTooSmall)
        );

        let invalid_runtime = RuntimeConfig {
            flavor: RuntimeFlavor::CurrentThread,
            worker_threads: Some(2),
            start_paused: false,
        };
        assert!(matches!(
            CaptureSession::new(
                TestMetadata::new(
                    "invalid-runtime",
                    ExpectedPanic::None,
                    Some(invalid_runtime)
                ),
                CaptureConfig::default()
            ),
            Err(CaptureConfigError::InvalidRuntime(_))
        ));
    }

    /// Escaping is part of metadata validation, so a large encoded identity fails explicitly.
    #[::core::prelude::v1::test]
    fn metadata_validation_uses_the_encoded_record() {
        let hostile_name = "\0".repeat(MAX_METADATA_RECORD_BYTES);
        let error = CaptureSession::new(metadata(&hostile_name), CaptureConfig::default())
            .err()
            .expect("encoded identity must exceed the metadata record");
        assert!(matches!(
            error,
            CaptureConfigError::MetadataDoesNotFit { .. }
        ));
    }

    /// A scoped synchronous dispatcher exposes a complete active observation window.
    #[crate::test]
    fn synchronous_capture_exposes_a_complete_snapshot() {
        tracing::info!(
            marker = "sync-capture",
            count = 7_u64,
            ready = true,
            "test event"
        );
        let capture = current_capture().expect("test wrapper installs a capture");
        let events = capture
            .matching("test event")
            .expect("no evidence was lost");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].fields.get("message"), Some(&"test event".into()));
        assert_eq!(events[0].fields.get("marker"), Some(&"sync-capture".into()));
        assert_eq!(events[0].fields.get("count"), Some(&"7".into()));
        assert_eq!(events[0].fields.get("ready"), Some(&"true".into()));
        assert_eq!(events[0].target, module_path!());
        assert_eq!(events[0].level, "INFO");
        assert_eq!(events[0].sequence, 0);
        assert_eq!(
            capture
                .matching_events(|event| {
                    event.level == "INFO"
                        && event.fields.get("marker") == Some(&"sync-capture".into())
                })
                .unwrap()
                .len(),
            1
        );
    }

    /// Sequence and elapsed-time metadata are monotonic without relying on wall-clock ordering.
    #[::core::prelude::v1::test]
    fn event_ordering_metadata_is_monotonic() {
        let session = CaptureSession::new(metadata("ordering"), CaptureConfig::default()).unwrap();
        let handle = session.handle();
        session.thread_context().enter(|| {
            tracing::info!("first ordering event");
            tracing::info!("second ordering event");
        });
        let snapshot = handle.snapshot().unwrap();
        assert_eq!(snapshot.events()[0].sequence, 0);
        assert_eq!(snapshot.events()[1].sequence, 1);
        assert!(snapshot.events()[0].elapsed_micros <= snapshot.events()[1].elapsed_micros);
    }

    /// Nested tasks and blocking workers use the runtime owner's dispatcher.
    #[crate::test(flavor = "multi_thread", worker_threads = 2)]
    async fn runtime_workers_keep_the_test_capture() {
        tokio::spawn(async {
            assert!(current_capture().is_some());
            tokio::spawn(async {
                assert!(current_capture().is_some());
                tracing::info!("nested task")
            })
            .await
        })
        .await
        .expect("outer task must complete")
        .expect("nested task must complete");
        tokio::task::spawn_blocking(|| {
            assert!(current_capture().is_some());
            tracing::info!("blocking task")
        })
        .await
        .expect("blocking task must complete");
        let capture = current_capture().expect("calling thread keeps its capture");
        assert_eq!(capture.matching("nested task").unwrap().len(), 1);
        assert_eq!(capture.matching("blocking task").unwrap().len(), 1);
    }

    /// A paused current-thread clock remains usable under the replacement attribute.
    #[crate::test(flavor = "current_thread", start_paused = true)]
    async fn current_thread_runtime_preserves_paused_time() {
        let before = tokio::time::Instant::now();
        tokio::time::sleep(Duration::from_secs(60)).await;
        assert_eq!(
            tokio::time::Instant::now() - before,
            Duration::from_secs(60)
        );
        tokio::spawn(async {
            assert!(current_capture().is_some());
            tracing::info!("paused clock advanced")
        })
        .await
        .expect("current-thread nested task must complete");
        assert_eq!(
            current_capture()
                .unwrap()
                .matching("paused clock advanced")
                .unwrap()
                .len(),
            1
        );
    }

    /// Two simultaneous runtimes at one callsite retain only their own events.
    #[::core::prelude::v1::test]
    fn simultaneous_runtimes_isolate_shared_callsites() {
        let barrier = Arc::new(Barrier::new(2));
        let run = |owner: &'static str, barrier: Arc<Barrier>| {
            thread::spawn(move || {
                let config = RuntimeConfig {
                    flavor: RuntimeFlavor::MultiThread,
                    worker_threads: Some(1),
                    start_paused: false,
                };
                let session = CaptureSession::new(
                    TestMetadata::new(owner, ExpectedPanic::None, Some(config)),
                    CaptureConfig::default(),
                )
                .unwrap();
                let handle = session.handle();
                let context = session.thread_context();
                context.enter(|| {
                    let runtime = build_runtime(config, context.clone());
                    runtime.block_on(async move {
                        barrier.wait();
                        tokio::spawn(async move { shared_runtime_callsite(owner) })
                            .await
                            .unwrap();
                    });
                });
                (owner, handle)
            })
        };
        let left = run("left", barrier.clone());
        let right = run("right", barrier);
        for task in [left, right] {
            let (owner, handle) = task.join().unwrap();
            let events = handle.matching("shared runtime callsite").unwrap();
            assert_eq!(events.len(), 1);
            assert_eq!(events[0].fields.get("owner"), Some(&owner.into()));
        }
    }

    /// The same callsite proves both the foreign-thread negative control and explicit carry path.
    #[crate::test]
    fn foreign_threads_require_explicit_context() {
        thread::spawn(|| {
            assert!(current_capture().is_none());
            shared_runtime_callsite("foreign")
        })
        .join()
        .unwrap();
        thread::spawn(|| {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap();
            runtime.block_on(async {
                assert!(current_capture().is_none());
                shared_runtime_callsite("foreign-runtime")
            });
        })
        .join()
        .unwrap();
        let context = current_thread_context().expect("wrapper installs explicit context");
        thread::spawn(move || context.enter(|| shared_runtime_callsite("carried")))
            .join()
            .unwrap();

        let events = current_capture()
            .unwrap()
            .matching("shared runtime callsite")
            .unwrap();
        assert_eq!(
            events.len(),
            1,
            "negative control detects leaked propagation"
        );
        assert_eq!(events[0].fields.get("owner"), Some(&"carried".into()));
    }

    /// Late records update nested spans and inner values shadow outer inherited values.
    #[::core::prelude::v1::test]
    fn late_span_records_and_inner_shadowing_are_preserved() {
        let session =
            CaptureSession::new(metadata("span-records"), CaptureConfig::default()).unwrap();
        let handle = session.handle();
        session.thread_context().enter(|| {
            let outer =
                tracing::info_span!("outer", shared = "outer", late = tracing::field::Empty);
            let _outer = outer.enter();
            outer.record("late", "recorded");
            let inner = tracing::info_span!("inner", shared = "inner");
            let _inner = inner.enter();
            tracing::info!("inside spans");
        });
        let events = handle.matching("inside spans").unwrap();
        assert_eq!(events[0].span_fields.get("shared"), Some(&"inner".into()));
        assert_eq!(events[0].span_fields.get("late"), Some(&"recorded".into()));
    }

    /// A shadowed span value consumes neither a field slot nor bytes in the effective event.
    #[::core::prelude::v1::test]
    fn event_fields_shadow_spans_at_both_admission_boundaries() {
        let config = CaptureConfig {
            max_fields: 1,
            max_field_bytes: 11,
            max_event_field_bytes: 11,
            ..CaptureConfig::default()
        };
        let session = CaptureSession::new(metadata("event-shadowing"), config).unwrap();
        session.thread_context().enter(|| {
            let span = tracing::info_span!("outer", shared = "outer");
            let _entered = span.enter();
            tracing::info!(target: "t", shared = "event");
        });
        let snapshot = session
            .handle()
            .snapshot()
            .expect("one effective field fits exactly");
        assert_eq!(snapshot.events().len(), 1);
        assert_eq!(snapshot.events()[0].fields["shared"], "event");
        assert!(snapshot.events()[0].span_fields.is_empty());
    }

    /// Closing a span releases aggregate accounting so later spans get the same full allowance.
    #[::core::prelude::v1::test]
    fn closing_spans_releases_aggregate_accounting() {
        let mut config = small_config();
        config.max_field_bytes = 16;
        config.max_span_bytes = 16;
        config.max_span_field_bytes = 16;
        let session = CaptureSession::new(metadata("span-close"), config).unwrap();
        let handle = session.handle();
        session.thread_context().enter(|| {
            {
                let first = tracing::info_span!("first", value = "123456789");
                let _entered = first.enter();
            }
            {
                let second = tracing::info_span!("second", value = "abcdefghi");
                let _entered = second.enter();
                tracing::info!(target: "t", "ok");
            }
        });
        assert_eq!(handle.matching("ok").unwrap().len(), 1);
    }

    /// Late update truncation makes the window incomplete before any later event and after close.
    #[::core::prelude::v1::test]
    fn late_update_loss_is_visible_immediately_and_survives_close() {
        let mut config = small_config();
        config.max_field_bytes = 12;
        config.max_span_field_bytes = 12;
        let session = CaptureSession::new(metadata("late-loss"), config).unwrap();
        let handle = session.handle();
        session.thread_context().enter(|| {
            let span = tracing::info_span!("late", value = tracing::field::Empty);
            span.record("value", "a value too long for the configured span");
            assert!(matches!(
                handle.snapshot(),
                Err(SnapshotError::Incomplete(_))
            ));
            drop(span);
        });
        assert!(matches!(
            handle.snapshot(),
            Err(SnapshotError::Incomplete(_))
        ));
    }

    /// Repeated late records cannot grow a span past its field-count boundary.
    #[::core::prelude::v1::test]
    fn late_update_field_count_is_enforced_without_a_later_event() {
        let mut config = small_config();
        config.max_fields = 2;
        let session = CaptureSession::new(metadata("late-count-loss"), config).unwrap();
        let handle = session.handle();
        session.thread_context().enter(|| {
            let span = tracing::info_span!(
                "late-count",
                alpha = tracing::field::Empty,
                beta = tracing::field::Empty,
                gamma = tracing::field::Empty
            );
            span.record("alpha", "one");
            span.record("beta", "two");
            span.record("gamma", "three");
            assert!(handle.snapshot().is_err());
        });
        assert!(handle.partial_snapshot().loss.truncated_fields > 0);
    }

    /// Field count, field bytes, event bytes, and eviction all expose loss explicitly.
    #[::core::prelude::v1::test]
    fn event_limits_make_ordinary_matching_fail_closed() {
        let mut config = small_config();
        config.max_events = 1;
        config.max_fields = 2;
        config.max_field_bytes = 12;
        config.max_event_field_bytes = 20;
        let session = CaptureSession::new(metadata("event-limits"), config).unwrap();
        let handle = session.handle();
        session.thread_context().enter(|| {
            tracing::info!(one = "123456789", two = "", three = "", "first");
            tracing::info!("second");
        });
        let partial = handle.partial_snapshot();
        assert_eq!(partial.events.len(), 1);
        assert!(partial.loss.evicted_events > 0);
        assert!(partial.loss.truncated_fields > 0);
        assert!(handle.matching("second").is_err());
    }

    /// Aggregate event bytes evict old evidence independently of the retained-event count.
    #[::core::prelude::v1::test]
    fn aggregate_event_bytes_have_their_own_eviction_boundary() {
        let mut config = small_config();
        config.max_event_bytes = 48;
        config.max_event_field_bytes = 24;
        config.max_field_bytes = 24;
        let session = CaptureSession::new(metadata("event-byte-budget"), config).unwrap();
        let handle = session.handle();
        session.thread_context().enter(|| {
            tracing::info!("1234567890");
            tracing::info!("abcdefghij");
            tracing::info!("ABCDEFGHIJ");
        });
        let partial = handle.partial_snapshot();
        assert!(partial.events.len() < 3);
        assert!(partial.loss.evicted_events > 0);
    }

    /// One span can never retain more than the configured field count, even with empty values.
    #[::core::prelude::v1::test]
    fn per_span_field_count_is_enforced_at_creation() {
        let mut config = small_config();
        config.max_fields = 2;
        let session = CaptureSession::new(metadata("span-field-count"), config).unwrap();
        let handle = session.handle();
        session.thread_context().enter(|| {
            let span = tracing::info_span!("counted", alpha = "", beta = "", gamma = "");
            let _entered = span.enter();
            tracing::info!("inside counted span");
        });
        let partial = handle.partial_snapshot();
        assert_eq!(
            partial.events[0].span_fields.len(),
            1,
            "event message shares the field count"
        );
        assert!(partial.loss.truncated_fields > 0);
    }

    /// Empty span values still consume name bytes and the inherited field-count budget.
    #[::core::prelude::v1::test]
    fn empty_span_fields_cannot_bypass_aggregate_or_flattened_limits() {
        let mut config = small_config();
        config.max_fields = 2;
        config.max_field_bytes = 12;
        config.max_span_bytes = 12;
        config.max_span_field_bytes = 12;
        let session = CaptureSession::new(metadata("empty-span-fields"), config).unwrap();
        let handle = session.handle();
        session.thread_context().enter(|| {
            let first = tracing::info_span!("first", alpha = "");
            let _first = first.enter();
            let second = tracing::info_span!("second", beta = "");
            let _second = second.enter();
            let third = tracing::info_span!("third", gamma = "");
            let _third = third.enter();
            tracing::info!("bounded scope");
        });
        let partial = handle.partial_snapshot();
        assert!(partial.loss.saturated_spans > 0 || partial.loss.truncated_fields > 0);
        assert!(partial.events[0].span_fields.len() <= 2);
        assert!(handle.snapshot().is_err());
    }

    /// Fieldless spans consume the live-span count, and closing one makes that slot reusable.
    #[::core::prelude::v1::test]
    fn live_span_count_is_bounded_and_released_on_close() {
        let mut config = small_config();
        config.max_spans = 2;
        let session = CaptureSession::new(metadata("span-count"), config).unwrap();
        let handle = session.handle();
        session.thread_context().enter(|| {
            let first = tracing::info_span!("first");
            let second = tracing::info_span!("second");
            let skipped = tracing::info_span!("skipped", marker = "not retained");
            {
                let _entered = skipped.enter();
                tracing::info!("inside saturated span");
            }
            drop(first);
            let replacement = tracing::info_span!("replacement", marker = "retained");
            let _entered = replacement.enter();
            tracing::info!("after count release");
            drop(skipped);
            drop(second);
        });
        let partial = handle.partial_snapshot();
        assert!(partial.loss.saturated_spans > 0);
        assert!(partial.events[0].truncated);
        assert_eq!(
            partial.events.last().unwrap().span_fields.get("marker"),
            Some(&"retained".into())
        );
    }

    /// Writes forever unless the bounded formatter propagates its first refusal.
    struct ChunkedDebug(Arc<AtomicUsize>);

    impl fmt::Debug for ChunkedDebug {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            loop {
                self.0.fetch_add(1, Ordering::Relaxed);
                formatter.write_str("abcd")?;
            }
        }
    }

    /// Returns its own error after a short prefix without crossing any byte boundary.
    struct EarlyErrorDebug;

    impl fmt::Debug for EarlyErrorDebug {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("short prefix")?;
            Err(fmt::Error)
        }
    }

    /// Panics after producing useful evidence, without crossing a configured bound.
    struct PanickingDebug;

    impl fmt::Debug for PanickingDebug {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("safe prefix")?;
            panic!("diagnostic formatter failed");
        }
    }

    /// Formatter unwinds preserve their prefix but permanently invalidate ordinary matching.
    #[::core::prelude::v1::test]
    fn formatter_panics_cannot_hide_loss_or_escape_logging() {
        let session =
            CaptureSession::new(metadata("formatter-panic"), CaptureConfig::default()).unwrap();
        let handle = session.handle();
        session.thread_context().enter(|| {
            tracing::info!(broken = ?PanickingDebug, "event after formatter panic");
            let span = tracing::info_span!("new-span", initial = ?PanickingDebug, late = tracing::field::Empty);
            span.record("late", tracing::field::debug(PanickingDebug));
            span.in_scope(|| tracing::info!("inherited partial evidence"));
        });
        let partial = handle.partial_snapshot();
        assert_eq!(partial.events.len(), 2);
        assert_eq!(partial.events[0].fields["broken"], "safe prefix");
        assert_eq!(partial.events[1].span_fields["late"], "safe prefix");
        assert!(partial.loss.truncated_fields >= 3);
        assert!(handle.matching_events(|_| true).is_err());
    }

    /// Both wrapper shapes must rethrow the exact typed body payload, not a diagnostic replacement.
    #[::core::prelude::v1::test]
    fn wrappers_preserve_typed_panic_payloads() {
        let sync = catch_unwind(|| {
            run_sync::<()>("typed-sync", ExpectedPanic::None, || {
                std::panic::panic_any(713usize);
            })
        })
        .unwrap_err();
        assert_eq!(*sync.downcast::<usize>().unwrap(), 713);
        let asynchronous = catch_unwind(|| {
            run_async::<()>(
                "typed-async",
                ExpectedPanic::None,
                RuntimeConfig {
                    flavor: RuntimeFlavor::CurrentThread,
                    worker_threads: None,
                    start_paused: false,
                },
                async {
                    std::panic::panic_any(917usize);
                },
            )
        })
        .unwrap_err();
        assert_eq!(*asynchronous.downcast::<usize>().unwrap(), 917);
    }

    /// Field truncation rejects the overflowing write without retaining an invalid UTF-8 prefix.
    #[::core::prelude::v1::test]
    fn capped_field_writer_stops_before_a_partial_codepoint() {
        let mut writer = CappedWrite::new(5);
        assert!(write!(&mut writer, "abc😀").is_err());
        assert!(writer.was_truncated());
        // A formatter can ignore the first error. Its later writes must not fabricate a prefix
        // that skips the rejected codepoint, even though two bytes of physical capacity remain.
        assert!(writer.write_str("d").is_err());
        assert_eq!(writer.into_string(), "abc");
    }

    /// A cooperative chunked formatter stops at the first rejected write instead of rendering all.
    #[::core::prelude::v1::test]
    fn hostile_chunked_debug_is_stopped_by_the_field_writer() {
        let calls = Arc::new(AtomicUsize::new(0));
        let session = CaptureSession::new(metadata("chunked-debug"), small_config()).unwrap();
        let handle = session.handle();
        session.thread_context().enter(|| {
            tracing::info!(hostile = ?ChunkedDebug(calls.clone()), "chunked");
        });
        assert!(calls.load(Ordering::Relaxed) <= 9);
        assert!(handle.snapshot().is_err());
    }

    /// A formatter's own error marks its retained prefix incomplete even without overflowing.
    #[::core::prelude::v1::test]
    fn early_formatter_error_is_visible_as_field_loss() {
        let session = CaptureSession::new(metadata("formatter-error"), small_config()).unwrap();
        let handle = session.handle();
        session.thread_context().enter(|| {
            tracing::info!(target: "t", broken = ?EarlyErrorDebug, "fmt");
        });
        let partial = handle.partial_snapshot();
        assert_eq!(
            partial.events[0].fields.get("broken"),
            Some(&"short prefix".into())
        );
        assert!(partial.loss.truncated_fields > 0);
        assert!(handle.snapshot().is_err());
    }

    /// JSON escaping, record truncation, and whole-dump omission obey both encoded byte caps.
    #[::core::prelude::v1::test]
    fn encoded_records_and_the_whole_dump_stay_bounded() {
        let mut config = small_config();
        config.max_events = 32;
        config.max_event_bytes = 2_048;
        config.max_field_bytes = 256;
        config.max_event_field_bytes = 512;
        config.max_span_bytes = 256;
        config.max_span_field_bytes = 256;
        config.max_jsonl_record_bytes = MIN_DIAGNOSTIC_RECORD_BYTES;
        config.max_metadata_record_bytes = MIN_DIAGNOSTIC_RECORD_BYTES;
        config.max_failure_dump_bytes = 1_100;
        let session = CaptureSession::new(metadata("encoded-bounds"), config).unwrap();
        session.thread_context().enter(|| {
            for sequence in 0..32 {
                tracing::info!(sequence, escaped = %"\0\n\r\t".repeat(64), "encoded event");
            }
        });
        let dump = session.failure_dump(ObservedOutcome::ReturnedFailure);
        assert!(dump.as_bytes().len() <= config.max_failure_dump_bytes);
        let records = dump
            .as_bytes()
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| {
                assert!(line.len() <= config.max_jsonl_record_bytes);
                serde_json::from_slice::<serde_json::Value>(line)
                    .expect("every line is complete JSON")
            })
            .collect::<Vec<_>>();
        assert_eq!(records[0]["kind"], "farhelm-testtrace");
        assert_eq!(records[0]["outcome"], "returned_failure");
        assert_eq!(
            records
                .last()
                .and_then(|record| record["sequence"].as_u64()),
            Some(31),
            "the newest fitting event remains available"
        );
        assert!(dump.loss().omitted_dump_events > 0);
    }

    /// Thousands of short records must retain their actual storage, not one full cap apiece.
    #[::core::prelude::v1::test]
    fn short_encoded_records_do_not_amplify_retained_capacity() {
        let event = EventSnapshot {
            sequence: 0,
            elapsed_micros: 0,
            target: "t".into(),
            level: "INFO".into(),
            fields: BTreeMap::new(),
            span_fields: BTreeMap::new(),
            truncated: false,
        };
        let records = (0..MAX_EVENTS)
            .map(|_| encode_event(&event, MAX_JSONL_RECORD_BYTES).0)
            .collect::<Vec<_>>();
        let used = records.iter().map(Vec::len).sum::<usize>();
        let reserved = records.iter().map(Vec::capacity).sum::<usize>();
        assert!(used < MAX_FAILURE_DUMP_BYTES);
        assert_eq!(
            reserved, used,
            "retained encoded records have no spare capacity"
        );
    }

    /// Rejects diagnostic delivery before accepting any output byte.
    struct FailingWriter;

    impl io::Write for FailingWriter {
        fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("injected diagnostic failure"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// Exercises a writer implementation that unwinds instead of returning an I/O error.
    struct PanickingWriter;

    impl io::Write for PanickingWriter {
        fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
            panic!("writer failure");
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// Panicking sinks must produce a fixed error and permanently invalidate capture matching.
    #[::core::prelude::v1::test]
    fn panicking_diagnostic_writer_is_contained() {
        let session =
            CaptureSession::new(metadata("panicking-writer"), CaptureConfig::default()).unwrap();
        let error = session
            .write_failure_dump(ObservedOutcome::ReturnedFailure, &mut PanickingWriter)
            .unwrap_err();
        assert_eq!(error.to_string(), "test tracing diagnostic writer panicked");
        assert!(session.handle().snapshot().is_err());
        assert_eq!(
            session.handle().partial_snapshot().loss.diagnostic_failures,
            1
        );
    }

    /// Automatic identity limits cannot replace valid body return values in either wrapper shape.
    #[::core::prelude::v1::test]
    fn oversized_automatic_names_preserve_body_returns() {
        let name = "n".repeat(MAX_METADATA_RECORD_BYTES);
        let sync = run_sync(&name, ExpectedPanic::None, || {
            assert!(current_capture().unwrap().snapshot().is_err());
            Ok::<_, &'static str>(std::process::ExitCode::from(7))
        });
        assert_eq!(sync, Ok(std::process::ExitCode::from(7)));
        let asynchronous = run_async(
            &name,
            ExpectedPanic::None,
            RuntimeConfig {
                flavor: RuntimeFlavor::CurrentThread,
                worker_threads: None,
                start_paused: false,
            },
            async {
                assert!(current_capture().unwrap().snapshot().is_err());
                Ok::<_, &'static str>(std::process::ExitCode::from(9))
            },
        );
        assert_eq!(asynchronous, Ok(std::process::ExitCode::from(9)));
    }

    /// Diagnostic construction borrows arbitrarily large caller text until the encoded-size gate.
    #[::core::prelude::v1::test]
    fn automatic_identity_text_is_borrowed_before_validation() {
        let text = "\0".repeat(MAX_METADATA_RECORD_BYTES * 16);
        let metadata = TestMetadata::new(
            text.as_str(),
            ExpectedPanic::Expected(Cow::Borrowed(&text)),
            None,
        );
        assert!(matches!(metadata.name, Cow::Borrowed(value) if value.as_ptr() == text.as_ptr()));
        assert!(matches!(metadata.expected_panic,
            ExpectedPanic::Expected(Cow::Borrowed(value)) if value.as_ptr() == text.as_ptr()));
        assert!(matches!(
            CaptureSession::new(metadata, CaptureConfig::default()),
            Err(CaptureConfigError::MetadataDoesNotFit { .. })
        ));
        let session = automatic_session(&text, ExpectedPanic::Expected(Cow::Borrowed(&text)), None);
        assert_eq!(session.metadata.name, "<test metadata omitted>");
        assert_eq!(
            session.metadata.expected_panic,
            ExpectedPanic::ExpectedOmitted
        );
        assert!(session.metadata_incomplete);
    }

    /// Broken diagnostic output preserves control flow and makes later assertions unsafe.
    #[::core::prelude::v1::test]
    fn failing_diagnostic_io_marks_the_capture_incomplete() {
        let session =
            CaptureSession::new(metadata("failing-writer"), CaptureConfig::default()).unwrap();
        assert!(
            session
                .write_failure_dump(ObservedOutcome::ReturnedFailure, &mut FailingWriter)
                .is_err()
        );
        assert!(matches!(
            session.handle().snapshot(),
            Err(SnapshotError::Incomplete(_))
        ));
    }

    /// Poison recovery reconstructs corrupted totals and permits later bounded useful evidence.
    #[::core::prelude::v1::test]
    fn poisoned_state_never_returns_to_a_complete_window() {
        let config = small_config();
        let session = CaptureSession::new(metadata("poison-repair"), config).unwrap();
        let collector = session.collector.clone();
        let first_span = session.thread_context().enter(|| {
            let span = tracing::info_span!("first", held = "retained");
            let entered = span.enter();
            tracing::info!(target: "t", "one");
            drop(entered);
            span
        });
        {
            let state = collector.state.lock().unwrap();
            assert!(!state.events.is_empty());
            assert!(!state.span_allocations.is_empty());
        }

        let poison = collector.clone();
        let _ = thread::spawn(move || {
            let mut state = poison.state.lock().unwrap();
            state.event_bytes = usize::MAX;
            state.span_bytes = usize::MAX;
            panic!("poison collector for contract test");
        })
        .join();
        let first = collector.snapshot();
        assert_eq!(first.loss.diagnostic_failures, 1);
        {
            let state = collector.state();
            assert_eq!(
                state.event_bytes,
                state.events.iter().map(|(bytes, _)| bytes).sum::<usize>()
            );
            assert_eq!(
                state.span_bytes,
                state.span_allocations.values().sum::<usize>()
            );
        }

        session.thread_context().enter(|| {
            let second_span = tracing::info_span!("second", later = "useful");
            let _entered = second_span.enter();
            tracing::info!(target: "t", "two");
        });
        drop(first_span);
        let second = collector.snapshot();
        assert_eq!(second.loss.diagnostic_failures, 1);
        assert!(!second.loss.is_complete());
        assert_eq!(
            second.events.last().unwrap().fields.get("message"),
            Some(&"two".into())
        );
        assert_eq!(
            second.events.last().unwrap().span_fields.get("later"),
            Some(&"useful".into())
        );
        let state = collector.state();
        assert!(state.event_bytes <= config.max_event_bytes);
        assert!(state.span_bytes <= config.max_span_bytes);
    }
}
