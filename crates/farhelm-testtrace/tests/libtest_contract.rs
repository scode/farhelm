//! Process-isolated contracts for the attribute's libtest-visible behavior.
//!
//! The child cases intentionally pass and fail in different ways. Each parent invocation selects
//! exactly one ignored fixture by its full name, drains bounded nonblocking pipes, and owns the
//! child's process group through timeout, output overflow, and direct-child completion.

#![cfg(unix)]

use std::process::{Command, ExitCode, ExitStatus, Termination};
use std::time::Duration;

use farhelm_testtrace::TestOutcome;

#[path = "support/process.rs"]
mod process;

const CHILD_ENV: &str = "FARHELM_TESTTRACE_CONTRACT_CHILD";
const OUTPUT_LIMIT: usize = 2 * 1024 * 1024;
const CHILD_TIMEOUT: Duration = Duration::from_secs(20);

/// Carries one exact fixture's runner verdict and merged bounded diagnostics.
struct ChildResult {
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

impl ChildResult {
    /// Joins already bounded stream prefixes only for human-readable assertion failures.
    fn display(&self) -> String {
        format!("stdout:\n{}\nstderr:\n{}", self.stdout, self.stderr)
    }

    /// A dump followed by a crash is not evidence that libtest reported the original result.
    fn has_libtest_verdict(&self, success: bool) -> bool {
        let expected_code = if success { 0 } else { 101 };
        let verdict = if success {
            "test result: ok. 1 passed; 0 failed"
        } else {
            "test result: FAILED. 0 passed; 1 failed"
        };
        self.status.code() == Some(expected_code) && self.stdout.contains(verdict)
    }
}

/// Re-executes this one test binary without inheriting ambient test configuration.
fn run_child(exact_name: &str) -> ChildResult {
    let mut command = Command::new(std::env::current_exe().expect("resolve current test binary"));
    command
        .args([
            "--exact",
            exact_name,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env_clear()
        .env(CHILD_ENV, exact_name)
        .env("RUST_BACKTRACE", "0");
    let result = process::run_bounded(command, CHILD_TIMEOUT, OUTPUT_LIMIT)
        .unwrap_or_else(|failure| panic!("child fixture {exact_name}: {failure}"));
    let stdout = String::from_utf8_lossy(&result.output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&result.output.stderr).into_owned();
    assert!(
        stdout.contains("running 1 test"),
        "exact child selection did not execute one fixture:\n{stdout}\n{stderr}"
    );
    assert!(!stdout.contains("running 0 tests"));
    ChildResult {
        status: result.status,
        stdout,
        stderr,
    }
}

/// Lists the compiled fixture names under the same bounded child supervision as exact runs.
fn list_children() -> String {
    let mut command = Command::new(std::env::current_exe().expect("resolve current test binary"));
    command
        .args(["--list"])
        .env_clear()
        .env("RUST_BACKTRACE", "0");
    let result = process::run_bounded(command, CHILD_TIMEOUT, OUTPUT_LIMIT)
        .unwrap_or_else(|failure| panic!("list child fixtures: {failure}"));
    assert!(result.status.success(), "fixture listing failed");
    String::from_utf8_lossy(&result.output.stdout).into_owned()
}

/// Extracts only complete JSON records emitted directly by the tracing support crate.
fn json_records(stderr: &str) -> Vec<serde_json::Value> {
    stderr
        .lines()
        .filter(|line| line.trim_start().starts_with('{'))
        .map(|line| serde_json::from_str(line).expect("dump line is complete JSON"))
        .collect()
}

/// Makes each ignored fixture fail if it was entered outside its exact parent invocation.
fn require_child(expected_name: &str) {
    assert_eq!(std::env::var(CHILD_ENV).as_deref(), Ok(expected_name));
}

mod child_cases {
    use std::future::pending;

    use super::*;

    /// Emits from task cancellation so the parent can prove capture outlives runtime teardown.
    struct TeardownTrace(&'static str);

    impl Drop for TeardownTrace {
        fn drop(&mut self) {
            tracing::warn!(shape = self.0, "runtime teardown marker");
        }
    }

    /// Detects accidental disposal of an arbitrary diagnostic panic payload.
    struct HostilePayload;

    impl Drop for HostilePayload {
        fn drop(&mut self) {
            panic!("diagnostic payload destructor must not run");
        }
    }

    /// Supplies a failing formatter while the test is already unwinding.
    struct PanicDuringDiagnostic;

    impl std::fmt::Debug for PanicDuringDiagnostic {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("retained diagnostic prefix")?;
            std::panic::panic_any(HostilePayload);
        }
    }

    /// Logs from body-local cleanup; a formatter failure must stay inside the diagnostic call.
    struct LoggingDestructor;

    impl Drop for LoggingDestructor {
        fn drop(&mut self) {
            tracing::warn!(broken = ?PanicDuringDiagnostic, "unwind diagnostic marker");
        }
    }

    /// Nested diagnostic unwinds must not abort or replace the expected original panic.
    #[ignore = "subprocess fixture asserted by libtest_contract_matrix"]
    #[farhelm_testtrace::test]
    #[should_panic(expected = "original body panic")]
    fn formatter_panic_during_unwind_preserves_original() {
        require_child("child_cases::formatter_panic_during_unwind_preserves_original");
        let _logging = LoggingDestructor;
        panic!("original body panic");
    }

    /// Supplies a hostile writer payload from an inner session while the outer body unwinds.
    struct WriterDuringUnwind(farhelm_testtrace::CaptureSession);

    /// Fails by unwinding with a payload whose destructor cannot safely be called.
    struct UnwindingWriter;

    impl std::io::Write for UnwindingWriter {
        fn write(&mut self, _bytes: &[u8]) -> std::io::Result<usize> {
            std::panic::panic_any(HostilePayload);
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl Drop for WriterDuringUnwind {
        fn drop(&mut self) {
            assert!(
                self.0
                    .write_failure_dump(
                        farhelm_testtrace::ObservedOutcome::Unwind,
                        &mut UnwindingWriter
                    )
                    .is_err()
            );
            assert!(self.0.handle().snapshot().is_err());
            tracing::warn!("writer failure contained during unwind");
        }
    }

    /// Public writer failure containment must preserve the original panic through cleanup.
    #[ignore = "subprocess fixture asserted by libtest_contract_matrix"]
    #[farhelm_testtrace::test]
    #[should_panic(expected = "original writer body panic")]
    fn writer_panic_during_unwind_preserves_original() {
        require_child("child_cases::writer_panic_during_unwind_preserves_original");
        let session = farhelm_testtrace::CaptureSession::new(
            farhelm_testtrace::TestMetadata::new(
                "inner-writer",
                farhelm_testtrace::ExpectedPanic::None,
                None,
            ),
            farhelm_testtrace::CaptureConfig::default(),
        )
        .unwrap();
        let _writer = WriterDuringUnwind(session);
        panic!("original writer body panic");
    }

    /// Emits a plausible failure dump and then crashes before libtest can report a verdict.
    #[test]
    #[ignore = "subprocess fixture asserted by crash_after_dump_is_not_a_libtest_failure"]
    fn abort_after_valid_dump() {
        require_child("child_cases::abort_after_valid_dump");
        let session = farhelm_testtrace::CaptureSession::new(
            farhelm_testtrace::TestMetadata::new(
                "abort-after-dump",
                farhelm_testtrace::ExpectedPanic::None,
                None,
            ),
            farhelm_testtrace::CaptureConfig::default(),
        )
        .unwrap();
        session
            .write_failure_dump(
                farhelm_testtrace::ObservedOutcome::ReturnedFailure,
                &mut std::io::stderr(),
            )
            .unwrap();
        std::process::abort();
    }

    /// Escaped metadata exceeds 4 KiB, but libtest must still execute and match the valid body panic.
    #[ignore = "subprocess fixture asserted by libtest_contract_matrix"]
    #[farhelm_testtrace::test]
    #[should_panic(
        expected = "\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0"
    )]
    fn oversized_expected_panic_preserves_libtest() {
        require_child("child_cases::oversized_expected_panic_preserves_libtest");
        tracing::warn!("oversized metadata body ran");
        panic!("{}", "\0".repeat(700));
    }

    /// Emits only from OS TLS teardown, after Tokio's on_thread_stop callback has returned.
    struct ThreadLocalTrace;

    impl Drop for ThreadLocalTrace {
        fn drop(&mut self) {
            let span = tracing::info_span!("thread-local-teardown-span", teardown = true);
            let _entered = span.enter();
            tracing::warn!(
                context_present = farhelm_testtrace::current_capture().is_some(),
                "thread-local teardown marker"
            );
        }
    }

    thread_local! {
        static THREAD_LOCAL_TRACE: ThreadLocalTrace = const { ThreadLocalTrace };
    }

    /// Initializes a destructor on an owned blocking thread; joining the task does not run it.
    /// A first user span afterward discriminates registry TLS destruction from dispatcher teardown.
    async fn initialize_thread_local_trace() {
        tokio::task::spawn_blocking(|| {
            THREAD_LOCAL_TRACE.with(|_| ());
            let span = tracing::info_span!("first-user-span");
            let _entered = span.enter();
        })
        .await
        .expect("blocking task initialized its TLS");
    }

    /// Current-thread runtimes still own blocking threads whose TLS evidence must survive shutdown.
    #[ignore = "subprocess fixture asserted by libtest_contract_matrix"]
    #[farhelm_testtrace::test(flavor = "current_thread")]
    async fn current_thread_tls_teardown_is_in_dump() -> Result<(), &'static str> {
        require_child("child_cases::current_thread_tls_teardown_is_in_dump");
        initialize_thread_local_trace().await;
        Err("retain TLS teardown")
    }

    /// Multi-thread runtime shutdown must include OS TLS cleanup as well as task cancellation.
    #[ignore = "subprocess fixture asserted by libtest_contract_matrix"]
    #[farhelm_testtrace::test(flavor = "multi_thread", worker_threads = 2)]
    async fn multi_thread_tls_teardown_is_in_dump() -> Result<(), &'static str> {
        require_child("child_cases::multi_thread_tls_teardown_is_in_dump");
        initialize_thread_local_trace().await;
        Err("retain TLS teardown")
    }

    /// Returns success through Termination even though diagnostic observation itself fails.
    struct PanickingOutcome;

    impl TestOutcome for PanickingOutcome {
        fn observed_success(&self) -> bool {
            std::panic::panic_any(HostilePayload);
        }
    }

    impl Termination for PanickingOutcome {
        fn report(self) -> ExitCode {
            eprintln!("CUSTOM_TERMINATION_REPORT");
            ExitCode::SUCCESS
        }
    }

    /// A failed observer cannot consume or replace a synchronous return value.
    #[ignore = "subprocess fixture asserted by libtest_contract_matrix"]
    #[farhelm_testtrace::test]
    fn panicking_sync_observer_preserves_return() -> PanickingOutcome {
        require_child("child_cases::panicking_sync_observer_preserves_return");
        PanickingOutcome
    }

    /// Async observation happens after shutdown and must still leave reporting to libtest.
    #[ignore = "subprocess fixture asserted by libtest_contract_matrix"]
    #[farhelm_testtrace::test]
    async fn panicking_async_observer_preserves_return() -> PanickingOutcome {
        require_child("child_cases::panicking_async_observer_preserves_return");
        PanickingOutcome
    }

    /// A successful return must leave stderr free of failure-dump records.
    #[ignore = "subprocess fixture asserted by libtest_contract_matrix"]
    #[farhelm_testtrace::test]
    fn success_has_no_dump() {
        require_child("child_cases::success_has_no_dump");
        tracing::info!("successful evidence");
    }

    /// A normal `Err` remains a libtest failure and carries returned-failure evidence.
    #[ignore = "subprocess fixture asserted by libtest_contract_matrix"]
    #[farhelm_testtrace::test]
    fn err_return_keeps_libtest_failure() -> Result<(), &'static str> {
        require_child("child_cases::err_return_keeps_libtest_failure");
        tracing::warn!(contract = "err", "returned error evidence");
        Err("intentional returned error")
    }

    /// A top-level unwind is rethrown after its bounded evidence is emitted.
    #[ignore = "subprocess fixture asserted by libtest_contract_matrix"]
    #[farhelm_testtrace::test]
    fn unwind_keeps_panic_payload() {
        require_child("child_cases::unwind_keeps_panic_payload");
        tracing::warn!("unwind evidence");
        panic!("intentional unwind payload");
    }

    /// A matching expected panic still dumps observed unwind metadata and passes libtest.
    #[ignore = "subprocess fixture asserted by libtest_contract_matrix"]
    #[farhelm_testtrace::test]
    #[should_panic(expected = "expected panic needle")]
    fn matching_expected_panic_passes() {
        require_child("child_cases::matching_expected_panic_passes");
        tracing::warn!("expected panic evidence");
        panic!("expected panic needle");
    }

    /// A missing expected panic emits returned-success evidence before libtest rejects it.
    #[ignore = "subprocess fixture asserted by libtest_contract_matrix"]
    #[farhelm_testtrace::test]
    #[should_panic(expected = "panic never arrives")]
    fn missing_expected_panic_fails() {
        require_child("child_cases::missing_expected_panic_fails");
        tracing::warn!("missing panic evidence");
    }

    /// A wrong expected message preserves libtest's mismatch failure and the actual unwind dump.
    #[ignore = "subprocess fixture asserted by libtest_contract_matrix"]
    #[farhelm_testtrace::test]
    #[should_panic(expected = "wanted message")]
    fn wrong_expected_panic_fails() {
        require_child("child_cases::wrong_expected_panic_fails");
        tracing::warn!("wrong expected evidence");
        panic!("different message");
    }

    /// A bare expected-panic declaration is retained independently of the unwind observation.
    #[ignore = "subprocess fixture asserted by libtest_contract_matrix"]
    #[farhelm_testtrace::test]
    #[should_panic]
    fn bare_expected_panic_passes() {
        require_child("child_cases::bare_expected_panic_passes");
        panic!("any panic is accepted");
    }

    /// An enabled cfg_attr must give libtest and the dump the same expected panic text.
    #[ignore = "subprocess fixture asserted by libtest_contract_matrix"]
    #[farhelm_testtrace::test]
    #[cfg_attr(all(), should_panic(expected = "conditional panic"))]
    fn enabled_conditional_panic_passes() {
        require_child("child_cases::enabled_conditional_panic_passes");
        panic!("conditional panic");
    }

    /// A disabled panic declaration must not turn an ordinary unwind into expected evidence.
    #[ignore = "subprocess fixture asserted by libtest_contract_matrix"]
    #[farhelm_testtrace::test]
    #[cfg_attr(any(), should_panic(expected = "conditional panic"))]
    fn disabled_conditional_panic_fails() {
        require_child("child_cases::disabled_conditional_panic_fails");
        panic!("conditional panic");
    }

    /// Async `Result` uses the same borrowed observation contract as a synchronous return.
    #[ignore = "subprocess fixture asserted by libtest_contract_matrix"]
    #[farhelm_testtrace::test(start_paused = false)]
    async fn async_err_return_keeps_libtest_failure() -> Result<(), &'static str> {
        require_child("child_cases::async_err_return_keeps_libtest_failure");
        tracing::warn!("async returned error evidence");
        Err("intentional async error")
    }

    /// Current-thread teardown cancels a proven-ready task before the failure dump is generated.
    #[ignore = "subprocess fixture asserted by libtest_contract_matrix"]
    #[farhelm_testtrace::test(flavor = "current_thread")]
    async fn current_thread_teardown_is_in_failure_dump() -> Result<(), &'static str> {
        require_child("child_cases::current_thread_teardown_is_in_failure_dump");
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let _trace = TeardownTrace("current_thread");
            ready_tx.send(()).expect("parent remains until readiness");
            pending::<()>().await;
        });
        ready_rx
            .await
            .expect("pending task reached its owned destructor");
        Err("trigger current-thread teardown dump")
    }

    /// Multi-thread teardown keeps worker cancellation inside the same capture lifetime.
    #[ignore = "subprocess fixture asserted by libtest_contract_matrix"]
    #[farhelm_testtrace::test(flavor = "multi_thread", worker_threads = 2)]
    async fn multi_thread_teardown_is_in_failure_dump() -> Result<(), &'static str> {
        require_child("child_cases::multi_thread_teardown_is_in_failure_dump");
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let _trace = TeardownTrace("multi_thread");
            ready_tx.send(()).expect("parent remains until readiness");
            pending::<()>().await;
        });
        ready_rx
            .await
            .expect("pending task reached its owned destructor");
        Err("trigger multi-thread teardown dump")
    }

    /// Separates borrowed success observation from libtest's consuming report call.
    struct CustomOutcome;

    impl TestOutcome for CustomOutcome {
        fn observed_success(&self) -> bool {
            true
        }
    }

    impl Termination for CustomOutcome {
        fn report(self) -> ExitCode {
            eprintln!("CUSTOM_TERMINATION_REPORT");
            ExitCode::SUCCESS
        }
    }

    /// A custom outcome's `Termination::report` remains exclusively owned by libtest.
    #[ignore = "subprocess fixture asserted by libtest_contract_matrix"]
    #[farhelm_testtrace::test]
    fn custom_termination_reports_once() -> CustomOutcome {
        require_child("child_cases::custom_termination_reports_once");
        CustomOutcome
    }

    /// Recursive `Result<ExitCode, _>` inspection preserves a supported success return.
    #[ignore = "subprocess fixture asserted by libtest_contract_matrix"]
    #[farhelm_testtrace::test]
    fn nested_supported_return_passes() -> Result<ExitCode, &'static str> {
        require_child("child_cases::nested_supported_return_passes");
        Ok(ExitCode::SUCCESS)
    }

    /// A non-success `ExitCode` remains the exact failure reported by libtest.
    #[ignore = "subprocess fixture asserted by libtest_contract_matrix"]
    #[farhelm_testtrace::test]
    fn exit_code_failure_is_observed() -> ExitCode {
        require_child("child_cases::exit_code_failure_is_observed");
        tracing::warn!("exit code evidence");
        ExitCode::from(7)
    }

    /// Conditional inclusion, ignore, and the module-qualified name survive expansion together.
    #[cfg(unix)]
    #[ignore = "subprocess fixture asserted by libtest_contract_matrix"]
    #[farhelm_testtrace::test]
    fn cfg_ignore_and_qualified_name_are_preserved() {
        require_child("child_cases::cfg_ignore_and_qualified_name_are_preserved");
    }

    /// A false source cfg must remove the whole generated test before its invalid body is checked.
    #[cfg(any())]
    #[farhelm_testtrace::test]
    fn cfg_disabled_fixture_is_absent() {
        let _: () = 1;
    }
}

/// Expected runner and structured-diagnostic contract for one exact child selector.
struct ContractCase {
    /// Name passed unchanged to libtest's exact selector.
    selector: &'static str,
    /// Final process status expected from libtest.
    success: bool,
    /// Wrapper-observed outcome when a dump is required.
    outcome: Option<&'static str>,
    /// Serialized panic kind and optional required message.
    expected_panic: Option<(&'static str, Option<&'static str>)>,
    /// Captured tracing message that must appear in an event record.
    event_message: Option<&'static str>,
    /// Runtime flavor that teardown metadata must preserve.
    runtime_flavor: Option<&'static str>,
}

/// Every child case proves the wrapper's dump decision separately from libtest's final status.
#[test]
fn libtest_contract_matrix() {
    let listing = list_children();
    assert!(listing.contains("child_cases::cfg_ignore_and_qualified_name_are_preserved"));
    assert!(!listing.contains("cfg_disabled_fixture_is_absent"));
    let cases = [
        ContractCase {
            selector: "child_cases::oversized_expected_panic_preserves_libtest",
            success: true,
            outcome: Some("unwind"),
            expected_panic: Some(("expected_omitted", None)),
            event_message: Some("oversized metadata body ran"),
            runtime_flavor: None,
        },
        ContractCase {
            selector: "child_cases::writer_panic_during_unwind_preserves_original",
            success: true,
            outcome: Some("unwind"),
            expected_panic: Some(("expected", Some("original writer body panic"))),
            event_message: Some("writer failure contained during unwind"),
            runtime_flavor: None,
        },
        ContractCase {
            selector: "child_cases::formatter_panic_during_unwind_preserves_original",
            success: true,
            outcome: Some("unwind"),
            expected_panic: Some(("expected", Some("original body panic"))),
            event_message: Some("unwind diagnostic marker"),
            runtime_flavor: None,
        },
        ContractCase {
            selector: "child_cases::panicking_sync_observer_preserves_return",
            success: true,
            outcome: Some("observation_failed"),
            expected_panic: Some(("none", None)),
            event_message: None,
            runtime_flavor: None,
        },
        ContractCase {
            selector: "child_cases::panicking_async_observer_preserves_return",
            success: true,
            outcome: Some("observation_failed"),
            expected_panic: Some(("none", None)),
            event_message: None,
            runtime_flavor: Some("current_thread"),
        },
        ContractCase {
            selector: "child_cases::current_thread_tls_teardown_is_in_dump",
            success: false,
            outcome: Some("returned_failure"),
            expected_panic: Some(("none", None)),
            event_message: Some("thread-local teardown marker"),
            runtime_flavor: Some("current_thread"),
        },
        ContractCase {
            selector: "child_cases::multi_thread_tls_teardown_is_in_dump",
            success: false,
            outcome: Some("returned_failure"),
            expected_panic: Some(("none", None)),
            event_message: Some("thread-local teardown marker"),
            runtime_flavor: Some("multi_thread"),
        },
        ContractCase {
            selector: "child_cases::success_has_no_dump",
            success: true,
            outcome: None,
            expected_panic: None,
            event_message: None,
            runtime_flavor: None,
        },
        ContractCase {
            selector: "child_cases::err_return_keeps_libtest_failure",
            success: false,
            outcome: Some("returned_failure"),
            expected_panic: Some(("none", None)),
            event_message: Some("returned error evidence"),
            runtime_flavor: None,
        },
        ContractCase {
            selector: "child_cases::unwind_keeps_panic_payload",
            success: false,
            outcome: Some("unwind"),
            expected_panic: Some(("none", None)),
            event_message: Some("unwind evidence"),
            runtime_flavor: None,
        },
        ContractCase {
            selector: "child_cases::matching_expected_panic_passes",
            success: true,
            outcome: Some("unwind"),
            expected_panic: Some(("expected", Some("expected panic needle"))),
            event_message: Some("expected panic evidence"),
            runtime_flavor: None,
        },
        ContractCase {
            selector: "child_cases::missing_expected_panic_fails",
            success: false,
            outcome: Some("returned_success"),
            expected_panic: Some(("expected", Some("panic never arrives"))),
            event_message: Some("missing panic evidence"),
            runtime_flavor: None,
        },
        ContractCase {
            selector: "child_cases::wrong_expected_panic_fails",
            success: false,
            outcome: Some("unwind"),
            expected_panic: Some(("expected", Some("wanted message"))),
            event_message: Some("wrong expected evidence"),
            runtime_flavor: None,
        },
        ContractCase {
            selector: "child_cases::bare_expected_panic_passes",
            success: true,
            outcome: Some("unwind"),
            expected_panic: Some(("any", None)),
            event_message: None,
            runtime_flavor: None,
        },
        ContractCase {
            selector: "child_cases::async_err_return_keeps_libtest_failure",
            success: false,
            outcome: Some("returned_failure"),
            expected_panic: Some(("none", None)),
            event_message: Some("async returned error evidence"),
            runtime_flavor: None,
        },
        ContractCase {
            selector: "child_cases::enabled_conditional_panic_passes",
            success: true,
            outcome: Some("unwind"),
            expected_panic: Some(("expected", Some("conditional panic"))),
            event_message: None,
            runtime_flavor: None,
        },
        ContractCase {
            selector: "child_cases::disabled_conditional_panic_fails",
            success: false,
            outcome: Some("unwind"),
            expected_panic: Some(("none", None)),
            event_message: None,
            runtime_flavor: None,
        },
        ContractCase {
            selector: "child_cases::current_thread_teardown_is_in_failure_dump",
            success: false,
            outcome: Some("returned_failure"),
            expected_panic: Some(("none", None)),
            event_message: Some("runtime teardown marker"),
            runtime_flavor: Some("current_thread"),
        },
        ContractCase {
            selector: "child_cases::multi_thread_teardown_is_in_failure_dump",
            success: false,
            outcome: Some("returned_failure"),
            expected_panic: Some(("none", None)),
            event_message: Some("runtime teardown marker"),
            runtime_flavor: Some("multi_thread"),
        },
        ContractCase {
            selector: "child_cases::custom_termination_reports_once",
            success: true,
            outcome: None,
            expected_panic: None,
            event_message: None,
            runtime_flavor: None,
        },
        ContractCase {
            selector: "child_cases::nested_supported_return_passes",
            success: true,
            outcome: None,
            expected_panic: None,
            event_message: None,
            runtime_flavor: None,
        },
        ContractCase {
            selector: "child_cases::exit_code_failure_is_observed",
            success: false,
            outcome: Some("returned_failure"),
            expected_panic: Some(("none", None)),
            event_message: Some("exit code evidence"),
            runtime_flavor: None,
        },
        ContractCase {
            selector: "child_cases::cfg_ignore_and_qualified_name_are_preserved",
            success: true,
            outcome: None,
            expected_panic: None,
            event_message: None,
            runtime_flavor: None,
        },
    ];

    for case in cases {
        let result = run_child(case.selector);
        assert!(
            result.has_libtest_verdict(case.success),
            "unexpected status for {}:\n{}",
            case.selector,
            result.display()
        );
        let records = json_records(&result.stderr);
        let metadata = records
            .iter()
            .find(|record| record["kind"] == "farhelm-testtrace");
        if let Some(outcome) = case.outcome {
            let metadata = metadata.unwrap_or_else(|| {
                panic!(
                    "missing metadata for {}:\n{}",
                    case.selector,
                    result.display()
                )
            });
            assert_eq!(metadata["outcome"].as_str(), Some(outcome));
            if outcome == "observation_failed" {
                assert_eq!(metadata["loss"]["diagnostic_failures"], 1);
            }
            if case.selector == "child_cases::formatter_panic_during_unwind_preserves_original" {
                assert!(metadata["loss"]["truncated_fields"].as_u64().unwrap() > 0);
            }
            let qualified_name = format!("libtest_contract::{}", case.selector);
            let expected_name =
                if case.selector == "child_cases::oversized_expected_panic_preserves_libtest" {
                    assert_eq!(metadata["loss"]["diagnostic_failures"], 1);
                    "<test metadata omitted>"
                } else {
                    qualified_name.as_str()
                };
            assert_eq!(metadata["test"]["name"].as_str(), Some(expected_name));
            assert!(metadata["identity"]["process_id"].is_u64());
            assert!(metadata["identity"]["started_unix_micros"].is_u64());
            let (kind, expected) = case
                .expected_panic
                .expect("dump cases declare panic metadata");
            assert_eq!(
                metadata["test"]["expected_panic"]["kind"].as_str(),
                Some(kind)
            );
            if let Some(expected) = expected {
                assert_eq!(
                    metadata["test"]["expected_panic"]["expected"].as_str(),
                    Some(expected)
                );
            } else {
                assert!(metadata["test"]["expected_panic"].get("expected").is_none());
            }
            if let Some(flavor) = case.runtime_flavor {
                assert_eq!(metadata["test"]["runtime"]["flavor"].as_str(), Some(flavor));
            }
        } else {
            assert!(
                metadata.is_none(),
                "successful fixture unexpectedly dumped metadata"
            );
        }
        if let Some(message) = case.event_message {
            assert!(
                records
                    .iter()
                    .any(|record| record["fields"]["message"].as_str() == Some(message)),
                "missing event evidence for {}:\n{}",
                case.selector,
                result.display()
            );
            if message == "thread-local teardown marker" {
                let event = records
                    .iter()
                    .find(|record| record["fields"]["message"] == message)
                    .unwrap();
                assert_eq!(event["fields"]["context_present"], "true");
                assert_eq!(event["span_fields"]["teardown"], "true");
            }
        }
        if case.selector == "child_cases::custom_termination_reports_once"
            || case.outcome == Some("observation_failed")
        {
            assert_eq!(
                result.stdout.matches("CUSTOM_TERMINATION_REPORT").count()
                    + result.stderr.matches("CUSTOM_TERMINATION_REPORT").count(),
                1
            );
        }
    }
}

/// Nonzero exit alone cannot distinguish an expected failed test from diagnostic teardown abort.
#[test]
fn crash_after_dump_is_not_a_libtest_failure() {
    let result = run_child("child_cases::abort_after_valid_dump");
    assert!(
        json_records(&result.stderr)
            .iter()
            .any(|record| record["kind"] == "farhelm-testtrace")
    );
    assert!(
        !result.has_libtest_verdict(false),
        "a crash was accepted as a libtest failure"
    );
    assert!(
        result.status.code().is_none(),
        "negative control did not terminate by signal"
    );
}

/// Broken children must hit a bounded failure and be reaped before the assertion resumes.
#[test]
fn bounded_child_supervision_kills_and_reaps() {
    process::assert_supervision_contracts();
}
