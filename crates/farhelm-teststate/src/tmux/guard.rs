//! Keep diagnostic collection and private-server shutdown in the same lifetime.
//!
//! Successful tests discard their capture only after fixture destruction. Taking
//! a snapshot on every drop therefore also covers returned errors and caught
//! task panics, whose fixtures disappear before the outer test knows it failed.

use crate::process::{CapturedOutput, CommandRunOutcome};
use crate::tmux::diagnostics::{
    TmuxDiagnosticAttempt, TmuxDiagnosticsOutcome, snapshot_tmux_diagnostics,
};
use crate::tmux::{TmuxProtocolOutcome, shutdown_tmux_server};
use std::ffi::{CString, OsStr, OsString};
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

/// Collect evidence, then stop a private server before its state directory dies.
///
/// Keep this field before the state directory in fixture structs, and after it
/// in destructuring patterns (locals drop in reverse order). A separate owner
/// is intentional: reboot tests drop it mid-test while retaining the directory.
/// The bounded support APIs report uncertain cleanup rather than signalling a
/// numeric PID. Abrupt process death still bypasses Drop.
pub struct TmuxServerGuard {
    socket: PathBuf,
    executable: Result<PathBuf, io::ErrorKind>,
}

impl TmuxServerGuard {
    /// Match default in-process supervisor seams: literal `tmux` through PATH.
    /// Ambient FARHELM_TMUX belongs to CLI startup and must not affect these
    /// fixtures, which construct the supervisor directly.
    pub fn new(socket: PathBuf) -> Self {
        Self::with_program(socket, Path::new("tmux"))
    }

    /// Capture a seam's configured spelling before moving it into a supervisor.
    /// Resolution does not launch a probe or alter the product's own lookup.
    pub fn with_program(socket: PathBuf, program: &Path) -> Self {
        Self::with_environment(socket, program, std::env::var_os("PATH").as_deref())
    }

    /// Freeze relative paths against the fixture's launch directory. A failed
    /// resolution is retained as evidence, so Drop never retries against some
    /// later environment or guesses an executable in the socket directory.
    /// `path` is the child's effective PATH, including explicit removal: None
    /// uses the platform's default search path. The caller must use the current
    /// directory for both this capture and the child's launch.
    pub fn with_environment(socket: PathBuf, program: &Path, path: Option<&OsStr>) -> Self {
        let executable = std::env::current_dir()
            .and_then(|cwd| resolve_executable(program, path, &cwd))
            .map_err(|error| error.kind());
        Self { socket, executable }
    }

    /// Preserve a readable failure message as well as structured trace events.
    /// Both paths consume the same bounded snapshot; this does not run a second
    /// set of independent probes with fresh per-command timeouts.
    pub fn diagnostic_text(&self) -> String {
        match &self.executable {
            Ok(executable) => {
                let snapshot = snapshot_tmux_diagnostics(&self.socket, executable);
                emit_snapshot(&snapshot);
                render_snapshot(&snapshot)
            }
            Err(reason) => format!("tmux diagnostic executable unavailable: {reason:?}"),
        }
    }
}

impl Drop for TmuxServerGuard {
    fn drop(&mut self) {
        let Ok(executable) = &self.executable else {
            ignore_diagnostic_panic(|| {
                tracing::warn!(reason = ?self.executable, "tmux cleanup executable unavailable");
            });
            return;
        };
        // Keep the two phases separate: even an unexpected diagnostic or
        // subscriber panic must not prevent the shutdown attempt on unwind.
        ignore_diagnostic_panic(|| {
            emit_snapshot(&snapshot_tmux_diagnostics(&self.socket, executable));
        });
        ignore_diagnostic_panic(|| {
            let outcome = shutdown_tmux_server(&self.socket, executable);
            tracing::info!(
                acquisition = ?outcome.acquisition,
                fallback = ?outcome.fallback,
                death = ?outcome.death,
                "tmux fixture shutdown"
            );
            match &outcome.protocol {
                TmuxProtocolOutcome::Attempted(command) => emit_command("shutdown", command),
                TmuxProtocolOutcome::NotAttempted(reason) => {
                    tracing::info!(?reason, "tmux shutdown client not attempted");
                }
            }
        });
    }
}

/// Contain both the diagnostic unwind and disposal of its payload. An unknown
/// panic_any payload can run arbitrary Drop code, including another panic, so
/// only known string payloads are destroyed here. The rare unknown allocation
/// is deliberately leaked, as in the test capture's diagnostic panic boundary.
fn ignore_diagnostic_panic(body: impl FnOnce() + std::panic::UnwindSafe) {
    if let Err(payload) = std::panic::catch_unwind(body) {
        if payload.is::<String>() || payload.is::<&'static str>() {
            drop(payload);
        } else {
            std::mem::forget(payload);
        }
    }
}

/// Resolve a cleanup candidate without changing the supervisor's launch policy.
/// Empty and relative PATH entries use the launch cwd. Access checks reject
/// ordinary permission/noexec shadows, but cannot prove future exec success or
/// prevent replacement between lookups. Forced cleanup independently verifies
/// the socket peer's executable; this path alone never authorizes a signal.
fn resolve_executable(program: &Path, path: Option<&OsStr>, cwd: &Path) -> io::Result<PathBuf> {
    let absolute = |path: PathBuf| {
        if path.is_absolute() {
            path
        } else {
            cwd.join(path)
        }
    };
    if program.as_os_str().as_bytes().contains(&b'/') {
        return absolute(program.to_path_buf()).canonicalize();
    }
    let default;
    let path = match path {
        Some(path) => path,
        None => {
            default = default_search_path()?;
            &default
        }
    };
    for directory in std::env::split_paths(path) {
        let candidate = absolute(directory.join(program));
        let Ok(name) = CString::new(candidate.as_os_str().as_bytes()) else {
            continue;
        };
        // SAFETY: name is a live NUL-terminated path, and access retains no
        // pointer. This is a candidate filter, not a security authorization.
        if candidate.is_file() && unsafe { libc::access(name.as_ptr(), libc::X_OK) } == 0 {
            return candidate.canonicalize();
        }
    }
    Err(io::ErrorKind::NotFound.into())
}

/// Ask the platform for its default search path when PATH is absent. A fixed
/// buffer bounds this metadata read; an unavailable or oversized answer leaves
/// resolution explicitly unavailable instead of inventing platform defaults.
fn default_search_path() -> io::Result<OsString> {
    let mut bytes = [0u8; 4096];
    // SAFETY: the pointer covers exactly the writable length supplied; confstr
    // writes a NUL-terminated string when that buffer fits the returned size.
    let size = unsafe { libc::confstr(libc::_CS_PATH, bytes.as_mut_ptr().cast(), bytes.len()) };
    if size == 0 || size > bytes.len() {
        return Err(io::ErrorKind::NotFound.into());
    }
    Ok(OsStr::from_bytes(&bytes[..size - 1]).to_os_string())
}

/// Emit each bounded stream in small fields, preserving late-query evidence.
/// A single Debug field for the snapshot would hit the capture's 4 KiB field
/// ceiling and silently lose most of the metadata and visible pane output.
fn emit_snapshot(snapshot: &TmuxDiagnosticsOutcome) {
    tracing::info!(authorization = ?snapshot.authorization, omitted_valid_panes = ?snapshot.omitted_valid_panes, "tmux fixture snapshot");
    for command in snapshot.metadata.iter().chain(&snapshot.captures) {
        let label = format!("{:?}", command.label);
        match &command.result {
            TmuxDiagnosticAttempt::Attempted(outcome) => emit_command(&label, outcome),
            TmuxDiagnosticAttempt::NotAttempted(reason) => {
                tracing::info!(%label, ?reason, "tmux diagnostic not attempted");
            }
        }
    }
}

/// Keep status, truncation, stream completeness and ownership loss distinct.
/// A nonzero client exit or an observed EOF cannot stand in for the other facts.
fn emit_command(label: &str, command: &CommandRunOutcome) {
    tracing::info!(
        %label, status = ?command.status, timed_out = command.timed_out,
        successful = command.status.is_some_and(|status| status.success()),
        reaped = command.direct_child_reaped, ownership_lost = command.ownership_lost,
        errors_truncated = command.errors_truncated, "tmux diagnostic command"
    );
    for error in &command.errors {
        tracing::info!(%label, stage = ?error.stage, kind = ?error.kind, code = ?error.raw_os_error,
            message = %error.message, "tmux diagnostic command error");
    }
    emit_stream(label, "stdout", &command.stdout);
    emit_stream(label, "stderr", &command.stderr);
}

/// At most 768 raw bytes expand to 2304 bytes under lossy UTF-8 decoding,
/// leaving room inside the capture's field cap. Offsets are raw byte offsets;
/// a split multibyte character may display as replacements at chunk boundaries.
fn emit_stream(label: &str, stream: &str, output: &CapturedOutput) {
    tracing::info!(%label, %stream, retained_bytes = output.prefix.len(),
        observed_bytes = ?output.observed_bytes, omitted_bytes = ?output.omitted_bytes,
        complete = output.complete, "tmux diagnostic stream");
    for (index, bytes) in output.prefix.chunks(768).enumerate() {
        tracing::info!(%label, %stream, byte_offset = index * 768,
            text = %String::from_utf8_lossy(bytes), "tmux diagnostic output");
    }
}

/// Render only already-bounded buffers. The fixed query count and stream caps
/// also bound this assertion text, including UTF-8 replacement expansion.
fn render_snapshot(snapshot: &TmuxDiagnosticsOutcome) -> String {
    use std::fmt::Write;
    let mut text = format!(
        "tmux snapshot: {:?}; omitted panes={:?}\n",
        snapshot.authorization, snapshot.omitted_valid_panes
    );
    for command in snapshot.metadata.iter().chain(&snapshot.captures) {
        let _ = writeln!(text, "{:?}:", command.label);
        match &command.result {
            TmuxDiagnosticAttempt::Attempted(outcome) => {
                let _ = writeln!(
                    text,
                    "status={:?} timeout={} reaped={} ownership_lost={} errors={:?} errors_truncated={}",
                    outcome.status,
                    outcome.timed_out,
                    outcome.direct_child_reaped,
                    outcome.ownership_lost,
                    outcome.errors,
                    outcome.errors_truncated
                );
                for (name, output) in [("stdout", &outcome.stdout), ("stderr", &outcome.stderr)] {
                    let _ = writeln!(
                        text,
                        "{name}: observed={:?} omitted={:?} complete={}\n{}",
                        output.observed_bytes,
                        output.omitted_bytes,
                        output.complete,
                        String::from_utf8_lossy(&output.prefix)
                    );
                }
            }
            TmuxDiagnosticAttempt::NotAttempted(reason) => {
                let _ = writeln!(text, "not attempted: {reason:?}");
            }
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// PATH lookup must preserve launch-directory semantics without reading or
    /// changing ambient variables. An unexecutable earlier file must not hide
    /// the later executable, and an explicit relative path must bypass PATH.
    #[farhelm_testtrace::test]
    fn executable_resolution_preserves_relative_and_empty_path_entries() {
        let state = crate::tempdir().unwrap();
        let cwd = state.path();
        std::fs::create_dir(cwd.join("first")).unwrap();
        std::fs::create_dir(cwd.join("second")).unwrap();
        std::fs::write(cwd.join("first/tmux"), "not executable").unwrap();
        for name in ["tmux", "second/tmux"] {
            std::fs::write(cwd.join(name), "#!/bin/sh\nexit 0\n").unwrap();
            std::fs::set_permissions(cwd.join(name), std::fs::Permissions::from_mode(0o700))
                .unwrap();
        }
        assert_eq!(
            resolve_executable(Path::new("tmux"), Some(OsStr::new("first:second")), cwd).unwrap(),
            cwd.join("second/tmux").canonicalize().unwrap()
        );
        assert_eq!(
            resolve_executable(Path::new("tmux"), Some(OsStr::new("")), cwd).unwrap(),
            cwd.join("tmux").canonicalize().unwrap()
        );
        assert_eq!(
            resolve_executable(Path::new("./tmux"), Some(OsStr::new("second")), cwd).unwrap(),
            cwd.join("tmux").canonicalize().unwrap()
        );
        assert!(resolve_executable(Path::new("absent"), Some(OsStr::new("first")), cwd).is_err());
        std::os::unix::fs::symlink(cwd, cwd.join("alias")).unwrap();
        assert_eq!(
            resolve_executable(Path::new("./tmux"), None, &cwd.join("alias")).unwrap(),
            cwd.join("tmux").canonicalize().unwrap()
        );
    }

    /// Chunking must preserve the end of a full stream even when decoding
    /// expands every byte. The complete capture check detects field truncation
    /// instead of letting a substring assertion accept a partial observation.
    #[farhelm_testtrace::test]
    fn output_chunks_fit_capture_fields_and_retain_the_tail() {
        let capture = farhelm_testtrace::current_capture().unwrap();
        let mut bytes = vec![0xff; 8192];
        bytes.extend_from_slice(b"last-evidence");
        emit_stream(
            "panes",
            "stdout",
            &CapturedOutput {
                observed_bytes: Some(bytes.len() as u64),
                omitted_bytes: Some(0),
                complete: true,
                prefix: bytes,
            },
        );
        let tails = capture
            .matching_events(|event| {
                event
                    .fields
                    .get("text")
                    .is_some_and(|text| text.contains("last-evidence"))
            })
            .unwrap();
        assert_eq!(tails.len(), 1);
    }
}
