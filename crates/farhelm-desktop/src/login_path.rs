//! Resolve the desktop's tool search path before its first runtime starts.
//!
//! A login shell can print arbitrary startup output. It passes its PATH
//! to `/bin/sh` as one argument for the private result file. There are no
//! pipes for startup output or descendants to keep open.

use std::ffi::OsString;
use std::io::{self, Read as _};
use std::os::unix::ffi::OsStringExt as _;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub(crate) const TIMEOUT: Duration = Duration::from_secs(5);
const OUTPUT_FILE: &str = "FARHELM_LOGIN_PATH_FILE";
// The private file already exists. Appending also works for login shells
// configured to refuse overwriting an existing file with `>`.
const PRINT_PATH: &str = r#"exec /bin/sh -c 'printf "%s\n" "$1"' farhelm-login-path "$PATH" >> "$FARHELM_LOGIN_PATH_FILE""#;
const MAX_PATH: u64 = 1024 * 1024;

/// Startup can continue with the inherited PATH for each of these failures.
#[derive(Debug)]
pub(crate) enum LookupError {
    Start(io::Error),
    NoPath,
    Timeout,
}

/// Read PATH after interactive login startup without taking
/// control of the terminal that may have launched the app. The OS bytes
/// matter here, including newlines and non-UTF-8 directory names.
pub(crate) fn lookup(shell: &Path, timeout: Duration) -> Result<OsString, LookupError> {
    let output = tempfile::NamedTempFile::new().map_err(LookupError::Start)?;
    let deadline = Instant::now() + timeout;
    let child = spawn_shell(shell, output.path()).map_err(LookupError::Start)?;
    wait_for_shell(child, deadline)?;

    // Read the originally opened file. The final newline is the writer's,
    // distinct from any trailing newline in the value itself.
    let mut path = Vec::new();
    output
        .as_file()
        .take(MAX_PATH + 1)
        .read_to_end(&mut path)
        .map_err(|_| LookupError::NoPath)?;
    if path.len() as u64 > MAX_PATH
        || path.pop() != Some(b'\n')
        || path.is_empty()
        || path.contains(&0)
    {
        return Err(LookupError::NoPath);
    }
    Ok(OsString::from_vec(path))
}

/// Keep startup and its output off the launching terminal. The shell uses
/// its own PATH value even when a startup file left it unexported. `exec`
/// lets the lookup finish without running the login shell's logout hooks.
fn spawn_shell(shell: &Path, output: &Path) -> io::Result<Child> {
    let mut command = Command::new(shell);
    command
        .args(["-l", "-i", "-c", PRINT_PATH])
        .env(OUTPUT_FILE, output)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    crate::detach_from_terminal(&mut command);
    command.spawn()
}

/// Wait without starting a runtime. The leader's exit status confirms the
/// file was written. On timeout, the inherited PATH stays in effect.
fn wait_for_shell(mut child: Child, deadline: Instant) -> Result<(), LookupError> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(_)) => return Err(LookupError::NoPath),
            Ok(None) => {}
            // `try_wait` retries interrupted waits. An ECHILD gives us no
            // unreaped PID with which to address the group safely.
            Err(error) => return Err(LookupError::Start(error)),
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            terminate_startup(child);
            return Err(LookupError::Timeout);
        }
        std::thread::sleep(remaining.min(Duration::from_millis(20)));
    }
}

/// TERM traps in startup files must not extend the lookup deadline.
/// Signal before reaping so the leader still owns its PID. A stalled
/// filesystem can defer exit even after SIGKILL, so reaping must not keep
/// the app waiting past the deadline.
fn terminate_startup(mut child: Child) {
    if let Ok(group) = libc::pid_t::try_from(child.id()) {
        // SAFETY: setsid made this unreaped child's PID its group ID.
        unsafe { libc::kill(-group, libc::SIGKILL) };
    }
    let _ = child.kill();
    if matches!(child.try_wait(), Ok(None))
        && let Err(error) = std::thread::Builder::new()
            .name("farhelm-login-shell-reaper".into())
            .spawn(move || {
                let _ = child.wait();
            })
    {
        tracing::warn!(%error, "could not start the login shell reaper");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt as _;
    use std::path::PathBuf;

    /// An executable fixture supplies only the startup behavior. The lookup
    /// still spawns, detaches, waits for and reads from a real process.
    fn shell(dir: &Path, startup: &str) -> PathBuf {
        let shell = dir.join("shell");
        fs::write(&shell, format!("#!/bin/sh\n{startup}\n")).unwrap();
        fs::set_permissions(&shell, fs::Permissions::from_mode(0o700)).unwrap();
        shell
    }

    /// Startup chatter can be much larger than a pipe buffer. Only the
    /// PATH is carried back even if unexported, with its Unix bytes intact.
    #[test]
    fn noisy_startup_preserves_the_shells_path_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let shell = shell(
            dir.path(),
            r#"
test "$1 $2 $3" = '-l -i -c' || exit 1
if read unused; then exit 1; fi
/bin/dd if=/dev/zero bs=65536 count=8
unset PATH
PATH=$(printf '/tools/\377:/usr/bin')
set -C
eval "$4"
"#,
        );
        assert_eq!(
            lookup(&shell, TIMEOUT).unwrap(),
            OsString::from_vec(b"/tools/\xff:/usr/bin".to_vec())
        );
    }

    /// Each quick failure preserves the inherited PATH. A successful exit
    /// without a value is still a failure, as is an exported empty value.
    #[test]
    fn distinguishes_a_shell_that_cannot_start_from_one_without_a_path() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            lookup(&dir.path().join("missing"), TIMEOUT),
            Err(LookupError::Start(_))
        ));
        for behavior in ["exit 0", "PATH=; export PATH; exec /bin/sh -c \"$4\""] {
            assert!(matches!(
                lookup(&shell(dir.path(), behavior), TIMEOUT),
                Err(LookupError::NoPath)
            ));
        }
    }

    /// The leader exiting cannot stand in for group cleanup. The script
    /// confirms it started a child that would otherwise still be running.
    #[test]
    fn timeout_ends_the_startup_process_group() {
        let dir = tempfile::tempdir().unwrap();
        let shell = shell(
            dir.path(),
            r#"
/bin/sleep 10 &
kill -0 "$!" || exit 1
printf '%s\n' "$!" > "$0.child"
wait
"#,
        );
        let output = tempfile::NamedTempFile::new().unwrap();
        let child = spawn_shell(&shell, output.path()).unwrap();
        let receipt = dir.path().join("shell.child");
        let sleeper = until_probe(|| {
            let pid = fs::read_to_string(&receipt)
                .ok()?
                .trim()
                .parse::<u32>()
                .ok()?;
            process_running(pid).then_some(pid)
        });
        // Clean up even if a loaded machine failed to reach the fixture's
        // readiness boundary. Start the short test deadline after the probe.
        let outcome = wait_for_shell(child, Instant::now() + Duration::from_millis(50));
        let pid = sleeper.expect("the startup script must have a live sleeper before timing out");
        assert!(matches!(outcome, Err(LookupError::Timeout)));
        assert!(
            until_probe(|| (!process_running(pid)).then_some(())).is_some(),
            "startup sleeper survived the group signal"
        );
    }

    /// Watch the fixture's receipt or exit state, with the same bound and
    /// cadence for both. No particular scheduler interleaving is required.
    fn until_probe<T>(mut observe: impl FnMut() -> Option<T>) -> Option<T> {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(value) = observe() {
                return Some(value);
            }
            if Instant::now() >= deadline {
                return None;
            }
            std::thread::sleep(Duration::from_millis(10)); // sleep-ok: pace the bounded readiness and process-exit probes
        }
    }

    /// A zombie has stopped executing even if its parent's reap is pending.
    /// `ps` reports a missing PID with exit 1 and no output.
    fn process_running(pid: u32) -> bool {
        let output = Command::new("/bin/ps")
            .args(["-o", "state=", "-p", &pid.to_string()])
            .output()
            .unwrap();
        if output.status.code() == Some(1) && output.stdout.is_empty() && output.stderr.is_empty() {
            return false;
        }
        let state = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && !state.trim().is_empty(),
            "could not inspect fixture process {pid}: {output:?}"
        );
        !state.trim().starts_with('Z')
    }
}
