//! Apply supervisor CLI environment precedence to shared private-server ownership.
//!
//! The shared guard owns diagnostics and teardown. Only this adapter knows
//! which child-only overrides the supervisor's CLI will use when it starts.

pub(crate) use farhelm_teststate::tmux::guard::TmuxServerGuard;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

/// Capture the inheriting child's tmux choice before spawning it. These
/// commands do not use env_clear or current_dir; explicit removals and repeated
/// overrides follow Command's final effective entries.
pub(crate) fn for_supervisor_child(
    socket: PathBuf,
    command: &std::process::Command,
) -> TmuxServerGuard {
    let (program, path) = supervisor_child_program(command);
    TmuxServerGuard::with_environment(socket, &program, path.as_deref())
}

/// Keep CLI precedence separate from executable resolution so the shared
/// support crate never depends on a production supervisor implementation.
fn supervisor_child_program(command: &std::process::Command) -> (PathBuf, Option<OsString>) {
    let effective = |name: &str| {
        command
            .get_envs()
            .find(|(key, _)| *key == OsStr::new(name))
            .map_or_else(
                || std::env::var_os(name),
                |(_, value)| value.map(OsStr::to_os_string),
            )
    };
    let override_value = effective("FARHELM_TMUX");
    let program = farhelm_supervisor::tmux::resolve_tmux_program(None, override_value.as_deref());
    (program, effective("PATH"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use farhelm_teststate::process::{CommandRunLimits, run_bounded};
    use std::path::Path;
    use std::time::Duration;

    /// Child-only overrides follow CLI precedence without changing the test
    /// process's environment. Generic PATH resolution is tested by the shared guard.
    #[farhelm_testtrace::test]
    fn child_override_and_path_are_captured_from_the_launch_command() {
        let state = farhelm_teststate::tempdir().unwrap();
        let executable = state.path().join("tmux");
        let mut command = std::process::Command::new("unused");
        command
            .env("FARHELM_TMUX", &executable)
            .env("PATH", "/nonexistent");
        assert_eq!(
            supervisor_child_program(&command),
            (executable, Some(OsString::from("/nonexistent")))
        );
        command.env("FARHELM_TMUX", "").env("PATH", state.path());
        assert_eq!(
            supervisor_child_program(&command),
            (
                PathBuf::from("tmux"),
                Some(state.path().as_os_str().to_owned())
            )
        );
    }

    /// Teardown must capture while the session exists, for successful scopes,
    /// returned errors and panic unwinding alike. Afterwards an independent
    /// client must no longer find that private server. The outer capture stays
    /// active throughout, just as it does for ordinary wrapped test fixtures.
    #[farhelm_testtrace::test]
    async fn guard_captures_before_shutdown_on_success_error_and_unwind() {
        use tracing_subscriber::prelude::*;
        let _slot = super::super::SLOTS.acquire().await.unwrap();
        let capture = farhelm_testtrace::current_capture().expect("test capture");
        for disposition in 0..5 {
            let state = farhelm_teststate::tempdir().unwrap();
            let socket = state.path().join("tmux.sock");
            let guard = TmuxServerGuard::new(socket.clone());
            // Use the same default PATH launch policy as the guard. An
            // advisory display-path resolver has different permission and
            // absent-PATH semantics and cannot select the test's executable.
            let executable = Path::new("tmux");
            let limits = CommandRunLimits::new(
                Duration::from_secs(5),
                Duration::from_millis(250),
                4096,
                4096,
                8192,
            )
            .unwrap();
            let mut start = std::process::Command::new(executable);
            start.args(["-S"]).arg(&socket).args([
                "-f",
                "/dev/null",
                "new-session",
                "-d",
                "-s",
                "guard-owned",
                "exec sleep 30",
            ]);
            let started = run_bounded(&mut start, &limits).unwrap();
            assert!(
                started.status.is_some_and(|status| status.success()),
                "{started:?}"
            );
            let text = guard.diagnostic_text();
            assert!(text.contains("guard-owned"), "{text}");
            let boundary = capture
                .snapshot()
                .unwrap()
                .events()
                .last()
                .unwrap()
                .sequence;
            let result = if disposition < 3 {
                std::panic::catch_unwind(|| -> Result<(), &'static str> {
                    let _guard = guard;
                    match disposition {
                        0 => Ok(()),
                        1 => Err("returned failure"),
                        _ => panic!("fixture unwind"),
                    }
                })
            } else {
                // A subscriber failure is adversarial diagnostic code. Test it
                // both on ordinary Drop and during an existing test unwind.
                let fired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let subscriber = tracing_subscriber::registry().with(PanicOnce(fired.clone()));
                let result = tracing::subscriber::with_default(subscriber, || {
                    std::panic::catch_unwind(|| {
                        let _guard = guard;
                        if disposition == 4 {
                            panic!("original fixture failure");
                        }
                        Ok(())
                    })
                });
                assert!(fired.load(std::sync::atomic::Ordering::SeqCst));
                result
            };
            match disposition {
                0 | 3 => assert_eq!(result.unwrap(), Ok(())),
                1 => assert_eq!(result.unwrap(), Err("returned failure")),
                _ => assert!(result.is_err()),
            }
            if disposition < 3 {
                let evidence = capture
                    .matching_events(|event| {
                        event.sequence > boundary
                            && event
                                .fields
                                .get("text")
                                .is_some_and(|text| text.contains("guard-owned"))
                    })
                    .unwrap();
                assert!(!evidence.is_empty(), "Drop lost the live session snapshot");
                let shutdown = capture
                    .matching_events(|event| {
                        event.sequence > boundary
                            && event
                                .fields
                                .get("message")
                                .is_some_and(|text| text == "tmux fixture shutdown")
                    })
                    .unwrap();
                assert_eq!(shutdown.len(), 1);
                assert!(
                    evidence
                        .iter()
                        .all(|event| event.sequence < shutdown[0].sequence)
                );
            }
            let mut probe = std::process::Command::new(executable);
            probe.arg("-S").arg(&socket).arg("list-sessions");
            let stopped = run_bounded(&mut probe, &limits).unwrap();
            assert!(
                stopped.status.is_some_and(|status| !status.success()),
                "{stopped:?}"
            );
        }
        assert_eq!(capture.matching("tmux fixture shutdown").unwrap().len(), 3);
    }

    /// Exercise payload destruction separately from the subscriber's original
    /// panic: discarding a catch_unwind Err must not run this destructor.
    struct PanicOnDrop;

    impl Drop for PanicOnDrop {
        fn drop(&mut self) {
            panic!("diagnostic payload destructor ran");
        }
    }

    /// Fail the first diagnostic event only, leaving shutdown observable to the
    /// independent tmux client even though this subscriber records no events.
    struct PanicOnce(std::sync::Arc<std::sync::atomic::AtomicBool>);

    impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for PanicOnce {
        fn on_event(&self, _: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
            if !self.0.swap(true, std::sync::atomic::Ordering::SeqCst) {
                std::panic::panic_any(PanicOnDrop);
            }
        }
    }
}
