//! Bounded ownership for fixture worker threads.
//!
//! A fixture usually has a protocol transaction running on one raw thread and
//! a test thread that owns the transaction's cleanup. Rust can join a thread,
//! but it cannot safely kill an arbitrary OS thread. [`FixtureThread`] therefore
//! owns an explicit, prompt cancellation callback and reports when bounded
//! cleanup could not observe the worker's completion.

use std::any::Any;
use std::fmt;
use std::io;
use std::mem;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const DROP_ALLOWANCE: Duration = Duration::from_secs(1);

/// Owns one fixture worker and the callback that interrupts its whole transaction.
///
/// Construct this immediately after spawning the worker. The callback must be
/// prompt and nonblocking, including destruction of unused captured state.
/// [`Drop`] gives join observation a one-second allowance reduced by time spent
/// cancelling. Synchronous callbacks, their destructors, subscribers and scheduler
/// delays cannot be interrupted here, so this is not a hard wall-clock bound.
/// This type never attempts unsafe
/// thread termination; an uncooperative worker and its join observer may outlive
/// their owner. One additional thread performs the blocking join, including TLS
/// destruction, and sends its result. Teardown only waits on that result channel.
pub struct FixtureThread {
    completion: Option<Receiver<bool>>,
    cancel: Option<Box<dyn FnOnce() + Send + 'static>>,
    label: &'static str,
}

/// The result of asking a fixture worker to finish normally.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FixtureThreadError {
    /// The worker did not reach the join boundary within the caller's allowance.
    Timeout {
        /// Static label identifying the fixture in the caller's diagnostics.
        label: &'static str,
        /// Allowance supplied to [`FixtureThread::finish`].
        allowance: Duration,
    },
    /// The requested allowance could not be represented by the clock.
    DeadlineOverflow {
        /// Static label identifying the fixture in the caller's diagnostics.
        label: &'static str,
        /// Allowance that overflowed the monotonic deadline calculation.
        allowance: Duration,
    },
    /// The worker reached the join boundary but unwound instead of completing.
    WorkerPanicked {
        /// Static label identifying the fixture in the caller's diagnostics.
        label: &'static str,
    },
    /// The join observer disappeared without proving that the worker was joined.
    ObserverLost {
        /// Static label identifying the fixture in the caller's diagnostics.
        label: &'static str,
    },
}

impl fmt::Display for FixtureThreadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timeout { label, allowance } => write!(
                formatter,
                "fixture thread {label} did not finish within {allowance:?}"
            ),
            Self::DeadlineOverflow { label, allowance } => write!(
                formatter,
                "fixture thread {label} allowance {allowance:?} overflows its deadline"
            ),
            Self::WorkerPanicked { label } => {
                write!(formatter, "fixture thread {label} panicked")
            }
            Self::ObserverLost { label } => {
                write!(formatter, "fixture thread {label} lost its join observer")
            }
        }
    }
}

impl std::error::Error for FixtureThreadError {}

impl FixtureThread {
    /// Takes ownership of an already-spawned worker and its transaction-wide stop callback.
    ///
    /// The callback is retained until normal completion or drop. A result or
    /// done channel does not replace the handle: runtime and thread-local
    /// destruction can continue after a worker sends its final result. A dedicated
    /// observer sends completion only after `join` returns. If the observer cannot
    /// be spawned, cancellation is requested before returning the I/O error;
    /// the worker handle is intentionally leaked because there is no bounded
    /// join fallback. Dropping that handle could abort on a worker panic payload
    /// whose destructor panics, even after the constructor has returned.
    pub fn new(
        label: &'static str,
        handle: JoinHandle<()>,
        cancel: impl FnOnce() + Send + 'static,
    ) -> io::Result<Self> {
        Self::with_observer(label, handle, cancel, |work| {
            thread::Builder::new()
                .name("fixture-join".into())
                .spawn(work)
                .map(|_| ())
        })
    }

    /// Injects only observer startup so failure ownership can be tested without
    /// exhausting real OS threads. The starter must run the submitted closure
    /// once on success, or drop it without running it on failure.
    fn with_observer(
        label: &'static str,
        handle: JoinHandle<()>,
        cancel: impl FnOnce() + Send + 'static,
        start: impl FnOnce(Box<dyn FnOnce() + Send>) -> io::Result<()>,
    ) -> io::Result<Self> {
        let (completed, completion) = mpsc::channel();
        // Arm cancellation before the fallible spawn. Its error path drops this
        // owner after dropping the observer closure. Retaining the worker handle
        // keeps Rust's thread-result packet from disposing an arbitrary payload.
        let owner = Self {
            completion: Some(completion),
            cancel: Some(Box::new(cancel)),
            label,
        };
        let handle = mem::ManuallyDrop::new(handle);
        start(Box::new(move || {
            let handle = mem::ManuallyDrop::into_inner(handle);
            let successful = match handle.join() {
                Ok(()) => true,
                Err(payload) => {
                    discard_panic_payload(payload);
                    false
                }
            };
            // No fixture callbacks, subscriber, or runtime run on this
            // observer. Only the already-joined worker's outcome crosses it.
            let _ = completed.send(successful);
        }))?;
        Ok(owner)
    }

    /// Waits for normal completion without requesting cancellation first.
    ///
    /// A timeout is explicit and causes drop cleanup to request cancellation.
    /// Drop cleanup may add its one-second cancellation allowance before this
    /// consuming method returns after a timeout.
    /// A worker panic is also reported as an error; its panic payload is
    /// discarded at the diagnostic boundary rather than re-panicked here.
    pub fn finish(mut self, allowance: Duration) -> Result<(), FixtureThreadError> {
        let Some(deadline) = Instant::now().checked_add(allowance) else {
            safe_report(self.label, Report::DeadlineOverflow);
            return Err(FixtureThreadError::DeadlineOverflow {
                label: self.label,
                allowance,
            });
        };
        match self.wait_for_join(deadline) {
            Ok(true) => {
                dispose_cancellation(&mut self.cancel, self.label);
                safe_report(self.label, Report::Completed);
                Ok(())
            }
            Ok(false) => {
                safe_report(self.label, Report::WorkerPanicked);
                Err(FixtureThreadError::WorkerPanicked { label: self.label })
            }
            Err(RecvTimeoutError::Timeout) => {
                safe_report(self.label, Report::TimedOut);
                Err(FixtureThreadError::Timeout {
                    label: self.label,
                    allowance,
                })
            }
            Err(RecvTimeoutError::Disconnected) => {
                safe_report(self.label, Report::ObserverLost);
                Err(FixtureThreadError::ObserverLost { label: self.label })
            }
        }
    }
}

impl Drop for FixtureThread {
    fn drop(&mut self) {
        self.cleanup(DROP_ALLOWANCE);
    }
}

impl FixtureThread {
    /// Requests cancellation and observes the worker within a bounded allowance.
    ///
    /// The private allowance parameter keeps focused tests deterministic while
    /// [`Drop`] retains the one-second join-observation allowance.
    fn cleanup(&mut self, allowance: Duration) {
        if self.completion.is_none() {
            dispose_cancellation(&mut self.cancel, self.label);
            return;
        }

        // Time spent cancelling reduces the remaining join-observation wait.
        // Synchronous callback and subscriber execution cannot be interrupted.
        let deadline = Instant::now()
            .checked_add(allowance)
            .unwrap_or_else(Instant::now);
        if let Some(cancel) = self.cancel.take() {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(cancel));
            if let Err(payload) = result {
                discard_panic_payload(payload);
                safe_report(self.label, Report::CancellationPanicked);
            }
        }

        match self.wait_for_join(deadline) {
            Ok(true) => {}
            Ok(false) => safe_report(self.label, Report::WorkerPanicked),
            Err(RecvTimeoutError::Timeout) => safe_report(self.label, Report::Abandoned),
            Err(RecvTimeoutError::Disconnected) => safe_report(self.label, Report::ObserverLost),
        }
    }

    /// Receives proof of an actual join without calling join on the test thread.
    /// A timeout retains the receiver so drop can still observe cancellation.
    fn wait_for_join(&mut self, deadline: Instant) -> Result<bool, RecvTimeoutError> {
        let result = self
            .completion
            .as_ref()
            .expect("completion receiver is present")
            .recv_timeout(deadline.saturating_duration_since(Instant::now()));
        if result.is_ok() {
            self.completion.take();
        }
        result
    }
}

#[derive(Clone, Copy)]
enum Report {
    Completed,
    TimedOut,
    DeadlineOverflow,
    CancellationPanicked,
    WorkerPanicked,
    Abandoned,
    CancellationDisposedPanicked,
    ObserverLost,
}

/// Emits one diagnostic event without allowing a subscriber panic to escape.
fn safe_report(label: &'static str, report: Report) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match report {
        Report::Completed => tracing::info!(
            target: "farhelm::fixture_thread",
            fixture = label,
            outcome = "completed",
            "fixture thread completed"
        ),
        Report::TimedOut => tracing::error!(
            target: "farhelm::fixture_thread",
            fixture = label,
            outcome = "timed_out",
            "fixture thread did not finish within its allowance"
        ),
        Report::DeadlineOverflow => tracing::error!(
            target: "farhelm::fixture_thread",
            fixture = label,
            outcome = "deadline_overflow",
            "fixture thread allowance overflows its deadline"
        ),
        Report::CancellationPanicked => tracing::error!(
            target: "farhelm::fixture_thread",
            fixture = label,
            outcome = "cancellation_panicked",
            "fixture cancellation callback panicked"
        ),
        Report::WorkerPanicked => tracing::error!(
            target: "farhelm::fixture_thread",
            fixture = label,
            outcome = "worker_panicked",
            "fixture worker panicked"
        ),
        Report::Abandoned => tracing::error!(
            target: "farhelm::fixture_thread",
            fixture = label,
            outcome = "abandoned",
            "fixture thread remained live after bounded cleanup"
        ),
        Report::CancellationDisposedPanicked => tracing::error!(
            target: "farhelm::fixture_thread",
            fixture = label,
            outcome = "cancellation_disposal_panicked",
            "fixture cancellation callback disposal panicked"
        ),
        Report::ObserverLost => tracing::error!(
            target: "farhelm::fixture_thread",
            fixture = label,
            outcome = "observer_lost",
            "fixture join observer ended without a completion result"
        ),
    }));
    if let Err(payload) = result {
        discard_panic_payload(payload);
    }
}

/// Drops a cancellation closure behind a panic boundary, distinguishing that
/// destructor failure from a callback that was actually invoked.
fn dispose_cancellation(
    cancellation: &mut Option<Box<dyn FnOnce() + Send + 'static>>,
    label: &'static str,
) {
    let Some(cancellation) = cancellation.take() else {
        return;
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(cancellation)));
    if let Err(payload) = result {
        discard_panic_payload(payload);
        safe_report(label, Report::CancellationDisposedPanicked);
    }
}

/// Disposes only panic payload types whose destructor is known not to panic.
///
/// `panic_any` permits arbitrary payloads, including a type with a panicking
/// destructor. Forgetting unknown payloads leaks that rare allocation, but it
/// keeps diagnostic cleanup from replacing the original unwind.
fn discard_panic_payload(payload: Box<dyn Any + Send>) {
    if payload.is::<String>() {
        if let Ok(payload) = payload.downcast::<String>() {
            drop(payload);
        }
    } else if payload.is::<&'static str>() {
        if let Ok(payload) = payload.downcast::<&'static str>() {
            drop(payload);
        }
    } else {
        mem::forget(payload);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    /// Gives each regression a fallible observer startup boundary before assertions.
    fn owned_thread(
        label: &'static str,
        handle: JoinHandle<()>,
        cancel: impl FnOnce() + Send + 'static,
    ) -> FixtureThread {
        FixtureThread::new(label, handle, cancel).expect("start fixture join observer")
    }

    /// Requires a deliberately uncooperative worker to be released during test unwinding.
    struct ReleaseOnDrop {
        release: Option<mpsc::Sender<()>>,
        completed: mpsc::Receiver<()>,
    }

    impl ReleaseOnDrop {
        /// Releases the worker while retaining the drop-time fallback.
        fn release(&mut self) {
            if let Some(release) = self.release.take() {
                let _ = release.send(());
            }
        }
    }

    impl Drop for ReleaseOnDrop {
        fn drop(&mut self) {
            self.release();
            let _ = self.completed.recv_timeout(Duration::from_secs(1));
        }
    }

    /// Requires a worker's diagnostic event to be retained without accepting incomplete evidence.
    fn saw_outcome(outcome: &str) -> bool {
        farhelm_testtrace::current_capture()
            .expect("test wrapper installs a capture")
            .matching_events(|event| {
                event
                    .fields
                    .get("outcome")
                    .is_some_and(|value| value == outcome)
            })
            .expect("fixture diagnostics must remain complete")
            .len()
            == 1
    }

    /// Normal finish waits for the handle and leaves the caller's cancellation path untouched.
    #[farhelm_testtrace::test]
    fn normal_finish_joins_without_cancellation() {
        let (release_tx, release_rx) = mpsc::channel();
        let (cancel_tx, cancel_rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            release_rx.recv().unwrap();
        });
        let owner = owned_thread("normal", handle, move || {
            cancel_tx.send(()).unwrap();
        });
        release_tx.send(()).unwrap();
        assert_eq!(owner.finish(Duration::from_secs(1)), Ok(()));
        assert!(cancel_rx.try_recv().is_err());
        assert!(saw_outcome("completed"));
    }

    /// Drop requests cancellation and joins a worker that observes the stop channel.
    #[farhelm_testtrace::test]
    fn drop_cancels_and_joins() {
        let (stop_tx, stop_rx) = mpsc::channel();
        let (joined_tx, joined_rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            stop_rx.recv().unwrap();
            joined_tx.send(()).unwrap();
        });
        drop(owned_thread("drop", handle, move || {
            stop_tx.send(()).unwrap();
        }));
        assert!(joined_rx.try_recv().is_ok());
    }

    /// A cancellation callback is also required when the owner is unwound.
    #[farhelm_testtrace::test]
    fn unwind_path_cancels() {
        let (stop_tx, stop_rx) = mpsc::channel();
        let (joined_tx, joined_rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            stop_rx.recv().unwrap();
            joined_tx.send(()).unwrap();
        });
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _owner = owned_thread("unwind", handle, move || {
                stop_tx.send(()).unwrap();
            });
            panic!("test unwind");
        }));
        assert!(result.is_err());
        assert!(joined_rx.try_recv().is_ok());
    }

    /// A normal timeout remains observable while drop still invokes cancellation.
    #[farhelm_testtrace::test]
    fn normal_timeout_cancels() {
        let (stop_tx, stop_rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            stop_rx.recv().unwrap();
        });
        let result = owned_thread("timeout", handle, move || {
            stop_tx.send(()).unwrap();
        })
        .finish(Duration::ZERO);
        assert!(matches!(result, Err(FixtureThreadError::Timeout { .. })));
        assert!(saw_outcome("timed_out"));
    }

    /// An uncooperative worker is reported within the injected allowance, then released and joined.
    #[farhelm_testtrace::test]
    fn uncooperative_timeout_is_bounded_and_observable() {
        let (release_tx, release_rx) = mpsc::channel();
        let (completed_tx, completed_rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            release_rx.recv().unwrap();
            completed_tx.send(()).unwrap();
        });
        let mut owner = owned_thread("abandoned", handle, || {});
        let mut release_guard = ReleaseOnDrop {
            release: Some(release_tx),
            completed: completed_rx,
        };
        owner.cleanup(Duration::from_millis(10));
        assert!(saw_outcome("abandoned"));
        release_guard.release();
        owner.cleanup(Duration::from_secs(1));
        assert!(owner.completion.is_none(), "released worker must be joined");
    }

    /// Returning from the worker body does not finish thread-local destruction.
    /// Cleanup must retain the handle while a TLS destructor is still running,
    /// then join once that destructor has been released.
    #[farhelm_testtrace::test]
    fn cleanup_waits_for_thread_local_destruction() {
        struct ExitBarrier {
            entered: mpsc::Sender<()>,
            release: mpsc::Receiver<()>,
        }

        impl Drop for ExitBarrier {
            fn drop(&mut self) {
                let _ = self.entered.send(());
                let _ = self.release.recv_timeout(Duration::from_secs(5));
            }
        }

        thread_local! {
            static EXIT_BARRIER: std::cell::RefCell<Option<ExitBarrier>> = const {
                std::cell::RefCell::new(None)
            };
        }

        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            EXIT_BARRIER.with(|slot| {
                *slot.borrow_mut() = Some(ExitBarrier {
                    entered: entered_tx,
                    release: release_rx,
                });
            });
        });
        let mut owner = owned_thread("tls", handle, || {});
        // The sender is dropped before the owner on assertion unwind, releasing
        // the destructor so the test's own failure cannot strand its worker.
        let release = release_tx;
        entered_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        owner.cleanup(Duration::ZERO);
        assert!(
            owner.completion.is_some(),
            "TLS destruction is still pending"
        );
        assert!(saw_outcome("abandoned"));
        release.send(()).unwrap();
        owner.cleanup(Duration::from_secs(1));
        assert!(
            owner.completion.is_none(),
            "TLS destructor must finish before join"
        );
    }

    /// A cancellation panic is retained as evidence without preventing a finished worker join.
    #[farhelm_testtrace::test]
    fn cancellation_panic_does_not_block_join() {
        let (stop_tx, stop_rx) = mpsc::channel();
        let (joined_tx, joined_rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            stop_rx.recv().unwrap();
            joined_tx.send(()).unwrap();
        });
        drop(owned_thread("cancel-panic", handle, move || {
            stop_tx.send(()).unwrap();
            panic!("cancel panic");
        }));
        assert!(joined_rx.try_recv().is_ok());
        assert!(saw_outcome("cancellation_panicked"));
    }

    /// Unknown callback panic payloads are forgotten rather than destructed unsafely.
    #[farhelm_testtrace::test]
    fn panicking_callback_payload_does_not_escape_or_skip_join() {
        struct PanickingPayload;
        impl Drop for PanickingPayload {
            fn drop(&mut self) {
                panic!("payload destructor");
            }
        }

        let (stop_tx, stop_rx) = mpsc::channel();
        let (joined_tx, joined_rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            stop_rx.recv().unwrap();
            joined_tx.send(()).unwrap();
        });
        drop(owned_thread("panic-payload", handle, move || {
            stop_tx.send(()).unwrap();
            std::panic::panic_any(PanickingPayload);
        }));
        assert!(joined_rx.try_recv().is_ok());
        assert!(saw_outcome("cancellation_panicked"));
    }

    /// A panicking subscriber cannot prevent drop from joining a released worker.
    #[farhelm_testtrace::test]
    fn reporting_panic_does_not_escape_or_skip_join() {
        struct PanickingSubscriber(std::sync::Arc<std::sync::atomic::AtomicUsize>);

        impl tracing::Subscriber for PanickingSubscriber {
            fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
                true
            }

            fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
                tracing::span::Id::from_u64(1)
            }

            fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}

            fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}

            fn event(&self, _: &tracing::Event<'_>) {
                self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                panic!("subscriber panic");
            }

            fn enter(&self, _: &tracing::span::Id) {}
            fn exit(&self, _: &tracing::span::Id) {}
        }

        let (stop_tx, stop_rx) = mpsc::channel();
        let (joined_tx, joined_rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            stop_rx.recv().unwrap();
            joined_tx.send(()).unwrap();
        });
        let reports = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let dispatch = tracing::Dispatch::new(PanickingSubscriber(reports.clone()));
        tracing::dispatcher::with_default(&dispatch, || {
            let mut owner = owned_thread("report-panic", handle, move || {
                stop_tx.send(()).unwrap();
                panic!("cancellation triggers the failing reporter");
            });
            owner.cleanup(Duration::from_secs(1));
            assert!(
                owner.completion.is_none(),
                "reporter panic must not skip join"
            );
        });
        assert_eq!(reports.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(joined_rx.try_recv().is_ok());
    }

    /// A worker panic is returned as a failed normal finish and retained as evidence.
    #[farhelm_testtrace::test]
    fn worker_panic_is_observable() {
        let handle = thread::spawn(|| panic!("worker panic"));
        let result = owned_thread("worker-panic", handle, || {}).finish(Duration::from_secs(1));
        assert_eq!(
            result,
            Err(FixtureThreadError::WorkerPanicked {
                label: "worker-panic"
            })
        );
        assert!(saw_outcome("worker_panicked"));
    }

    /// Normal completion disposes unused cancellation state behind the same
    /// panic boundary as cleanup, without ever invoking cancellation.
    #[farhelm_testtrace::test]
    fn unused_callback_disposal_cannot_replace_success() {
        struct ToxicPayload;
        impl Drop for ToxicPayload {
            fn drop(&mut self) {
                panic!("panic payload destructor");
            }
        }
        struct CapturedState;
        impl Drop for CapturedState {
            fn drop(&mut self) {
                std::panic::panic_any(ToxicPayload);
            }
        }
        let (called_tx, called_rx) = mpsc::channel();
        let state = CapturedState;
        let owner = owned_thread("unused-callback", thread::spawn(|| {}), move || {
            called_tx.send(()).unwrap();
            drop(state);
        });
        assert_eq!(owner.finish(Duration::from_secs(1)), Ok(()));
        assert!(called_rx.try_recv().is_err());
        assert!(saw_outcome("cancellation_disposal_panicked"));
    }

    /// A failed observer spawn must request cancellation even when dropping the
    /// unjoined worker's panic payload would abort. Isolate the toxic payload in
    /// a child so a regression produces a failed test rather than killing peers.
    #[farhelm_testtrace::test]
    fn observer_start_failure_preserves_cancellation() {
        const CHILD: &str = "FARHELM_FIXTURE_OBSERVER_FAILURE_CHILD";
        if std::env::var_os(CHILD).is_some() {
            struct ToxicPayload;
            impl Drop for ToxicPayload {
                fn drop(&mut self) {
                    panic!("unjoined worker payload destructor");
                }
            }
            let worker = thread::spawn(|| std::panic::panic_any(ToxicPayload));
            let deadline = Instant::now() + Duration::from_secs(1);
            while !worker.is_finished() {
                assert!(Instant::now() < deadline, "worker must produce its panic");
                thread::yield_now();
            }
            let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let callback_calls = calls.clone();
            let result = FixtureThread::with_observer(
                "observer-start-failure",
                worker,
                move || {
                    callback_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                },
                |work| {
                    drop(work);
                    Err(io::Error::other("injected observer start failure"))
                },
            );
            assert!(matches!(result, Err(error) if error.kind() == io::ErrorKind::Other));
            assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
            assert!(saw_outcome("observer_lost"));
            return;
        }

        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "thread::tests::observer_start_failure_preserves_cancellation",
                "--nocapture",
            ])
            .env(CHILD, "1")
            // The parent owns the retained subprocess result; the child is an
            // implementation probe, not an independently selected test run.
            .env_remove("FARHELM_TEST_TRACE_DIR");
        let limits = crate::process::CommandRunLimits::new(
            Duration::from_secs(10),
            Duration::from_secs(1),
            8192,
            8192,
            16384,
        )
        .unwrap();
        let outcome = crate::process::run_bounded(&mut command, &limits).unwrap();
        assert!(
            outcome.direct_child_reaped && !outcome.timed_out,
            "{outcome:?}"
        );
        assert!(
            outcome.status.is_some_and(|status| status.success()),
            "{outcome:?}"
        );
        assert!(outcome.errors.is_empty(), "{outcome:?}");
        assert!(
            String::from_utf8_lossy(&outcome.stdout.prefix).contains("1 passed"),
            "{outcome:?}"
        );
    }
}
