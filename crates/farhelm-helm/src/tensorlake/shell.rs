//! Running a shell script in a Tensorlake sandbox: the narrow seam the
//! snapshot builder uses.
//!
//! [`SandboxShell`] runs one script under `sh -c` as the sandbox's login user
//! and reports how it ended. The real implementation, [`SshSandboxShell`],
//! shells out to `ssh` through Tensorlake's gateway with Farhelm's own key
//! and a one-shot connection (see `ssh.rs` for why it never shares one). The
//! browser suite substitutes a fake on the same gate as its Tensorlake API
//! fake (`e2e.rs`), and the engine's unit tests a scripted one, so nothing
//! above this seam knows whether ssh ran.
//!
//! Route lowering is shared with provisioning and supervisor connections.
//! This caller still owns its named build steps, one-shot policy, process
//! group and bounded capture, which are the recipe engine's contract.

use std::{process::Stdio, sync::Arc, time::Duration};

use async_trait::async_trait;

use super::ssh::{AUTH_REFUSED_MESSAGE, SshOutcome, TensorlakeSshAccess, classify, destination};
use crate::provisioning::{CommandLauncher, SystemLauncher, capture_child, isolate_process_group};

/// The most of a failed script's stderr kept for display: its last lines,
/// at most this many characters. The build scripts already send only a
/// bounded tail of their logs; this bounds what a misbehaving peer can put
/// in front of the user.
const STDERR_DISPLAY_CHARS: usize = 2000;
const STDERR_DISPLAY_LINES: usize = 12;

/// How a script that did not succeed ended. Every variant renders as a
/// sentence fragment for the user.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ShellFailure {
    /// The gateway refused Farhelm's key.
    #[error("{AUTH_REFUSED_MESSAGE}")]
    AuthRefused,
    /// ssh could not reach the sandbox or lost the connection; the text is
    /// ssh's own diagnostic, cleaned and bounded by [`shown`].
    #[error("Farhelm could not reach the sandbox over SSH{}", detail(.0))]
    Unreachable(String),
    /// The script ran and exited nonzero; `stderr` is its cleaned tail,
    /// which is the whole message when there is one (the scripts write
    /// their own diagnostics there).
    #[error("{}", exited(*.code, .stderr))]
    Exited { code: i32, stderr: String },
    /// The script did not finish within its bound and was killed.
    #[error("the command did not finish within {} minutes", .0.as_secs().div_ceil(60))]
    TimedOut(Duration),
    /// Farhelm could not run ssh at all, or the fake refused (the reason
    /// says which).
    #[error("{0}")]
    Unavailable(String),
}

/// A nonzero exit's message: the script's own stderr, or the status when
/// it wrote none.
fn exited(code: i32, stderr: &str) -> String {
    if stderr.is_empty() {
        format!("the command exited with status {code}")
    } else {
        stderr.to_string()
    }
}

/// `": detail"`, or nothing for an empty detail.
fn detail(text: &str) -> String {
    if text.is_empty() {
        String::new()
    } else {
        format!(": {text}")
    }
}

/// A script's stdout, decoded lossily. Bounded by the capture (64 KiB).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ShellOutput {
    pub(crate) stdout: String,
}

impl ShellOutput {
    /// The last non-empty line, trimmed: where the build scripts put the one
    /// fact they report (an agent's `--version` line).
    pub(crate) fn last_line(&self) -> Option<String> {
        self.stdout
            .lines()
            .map(str::trim)
            .rfind(|line| !line.is_empty())
            .map(str::to_string)
    }
}

/// Run scripts in sandboxes.
#[async_trait]
pub(crate) trait SandboxShell: Send + Sync {
    /// Run `script` under `sh -c` as the login user of the sandbox named
    /// `sandbox`, killing it after `timeout`. `label` names the step for
    /// diagnostics and for the e2e fake's failure injection; it never
    /// reaches the remote side.
    ///
    /// `access` is Farhelm's key and known_hosts; a fake ignores it.
    async fn run(
        &self,
        access: &TensorlakeSshAccess,
        sandbox: &str,
        label: &str,
        script: &str,
        timeout: Duration,
    ) -> Result<ShellOutput, ShellFailure>;
}

/// The production shell: `ssh` through Tensorlake's gateway.
pub(crate) struct SshSandboxShell {
    launcher: Arc<dyn CommandLauncher>,
}

impl SshSandboxShell {
    pub(crate) fn new() -> Self {
        Self {
            launcher: Arc::new(SystemLauncher),
        }
    }
}

/// The ssh argv for one script: the one-shot prefix with Tensorlake's
/// options, then the whole `sh -c` invocation as ONE shell-quoted word.
/// ssh joins everything after the destination with spaces and hands it to
/// the remote login shell, so a script left as several words would be
/// re-split there.
pub(crate) fn script_argv(
    access: &TensorlakeSshAccess,
    sandbox: &str,
    script: &str,
) -> Result<Vec<String>, String> {
    crate::access::ssh_command_args(
        &destination(sandbox),
        std::path::Path::new(""),
        crate::access::ConnectionPolicy::OneShot,
        &crate::ssh::SshAccess::Tensorlake(access.clone()),
        &["sh".into(), "-c".into(), script.into()],
    )
    .map_err(|error| error.to_string())
}

#[async_trait]
impl SandboxShell for SshSandboxShell {
    async fn run(
        &self,
        access: &TensorlakeSshAccess,
        sandbox: &str,
        _label: &str,
        script: &str,
        timeout: Duration,
    ) -> Result<ShellOutput, ShellFailure> {
        let argv = script_argv(access, sandbox, script).map_err(ShellFailure::Unavailable)?;
        let mut command = tokio::process::Command::new("ssh");
        command
            .args(argv)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        isolate_process_group(&mut command);
        let child = self.launcher.spawn(&mut command).map_err(|error| {
            ShellFailure::Unavailable(if error.kind() == std::io::ErrorKind::NotFound {
                "ssh is not installed on the helm's machine".to_string()
            } else {
                format!("Farhelm could not run ssh: {error}")
            })
        })?;
        let result = match capture_child(child, timeout, "the sandbox command").await {
            Ok(result) => result,
            Err(failure) if failure.context.ends_with("timed out") => {
                return Err(ShellFailure::TimedOut(timeout));
            }
            Err(failure) => return Err(ShellFailure::Unavailable(failure.rendered())),
        };
        let stderr = shown(&result.stderr);
        match classify(result.code, &result.stderr) {
            SshOutcome::Exited(0) => Ok(ShellOutput {
                stdout: String::from_utf8_lossy(&result.stdout).into_owned(),
            }),
            SshOutcome::Exited(code) => Err(ShellFailure::Exited { code, stderr }),
            SshOutcome::AuthRefused => Err(ShellFailure::AuthRefused),
            SshOutcome::Unreachable => Err(ShellFailure::Unreachable(stderr)),
        }
    }
}

/// Peer text made safe to show: its last non-empty lines (the end of a log
/// is where the error is), each stripped of characters that must not be
/// shown as themselves, joined with newlines and cut to the last
/// [`STDERR_DISPLAY_CHARS`] characters. Newlines are kept because an apt
/// error reads as lines; the UI shows them as such.
pub(crate) fn shown(stderr: &str) -> String {
    let lines: Vec<String> = stderr
        .lines()
        .map(|line| {
            line.chars()
                .filter(|c| !farhelm_proto::text::is_presentation_unsafe(*c))
                .collect::<String>()
                .trim()
                .to_string()
        })
        .filter(|line| !line.is_empty())
        .collect();
    let joined = lines[lines.len().saturating_sub(STDERR_DISPLAY_LINES)..].join("\n");
    let tail_start = joined
        .char_indices()
        .rev()
        .nth(STDERR_DISPLAY_CHARS - 1)
        .map_or(0, |(index, _)| index);
    joined[tail_start..].to_string()
}

/// The shell of a helm that cannot run sandbox commands: a test-built helm
/// that did not inject one.
pub(crate) struct OfflineShell;

#[async_trait]
impl SandboxShell for OfflineShell {
    async fn run(
        &self,
        _: &TensorlakeSshAccess,
        _: &str,
        _: &str,
        _: &str,
        _: Duration,
    ) -> Result<ShellOutput, ShellFailure> {
        Err(ShellFailure::Unavailable(
            "this helm cannot run commands in sandboxes".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec: the argv is the one-shot prefix with Tensorlake's options, `--`,
    /// the sandbox's gateway destination, and then exactly one word that is
    /// `sh -c` followed by the script quoted as a single shell word.
    ///
    /// Why: ssh hands everything after the destination to the remote login
    /// shell as one string. A script passed as several words, or unquoted,
    /// would be re-split remotely, and package names would leak into the
    /// remote shell's own syntax.
    #[farhelm_testtrace::test]
    fn the_script_is_one_quoted_word_after_the_destination() {
        let dir = std::path::Path::new("/state/tensorlake");
        let access = TensorlakeSshAccess::new(dir, dir.join("ssh-key/id_ed25519"));
        let script = "echo 'a b' && exit 3";
        let argv = script_argv(&access, "farhelm-build-k7q2-x", script).unwrap();
        let n = argv.len();
        assert_eq!(argv[n - 3], "--");
        assert_eq!(
            argv[n - 2],
            "farhelm-build-k7q2-x@farhelm-build-k7q2-x.sandbox.tensorlake.ai"
        );
        let remote = shell_words::split(&argv[n - 1]).unwrap();
        assert_eq!(remote, ["sh", "-c", script]);
    }

    /// Spec: only the last non-empty line of stdout is reported; shown
    /// stderr keeps the last non-empty lines of a long text, at most 2000
    /// characters, with presentation-unsafe characters removed.
    ///
    /// Why: the agent scripts print their `--version` line last, after any
    /// installer chatter that reached stdout; and a failing step's useful
    /// diagnostic is at the end of its log, not the start.
    #[farhelm_testtrace::test]
    fn stdout_reports_its_last_line_and_stderr_keeps_its_tail() {
        let output = ShellOutput {
            stdout: "installing\n2.1.0 (Claude Code)\n\n".into(),
        };
        assert_eq!(output.last_line().as_deref(), Some("2.1.0 (Claude Code)"));
        assert_eq!(ShellOutput::default().last_line(), None);
        let long = format!("{}E: Unable to locate package nope", "x".repeat(10_000));
        let shown = shown(&long);
        assert!(
            shown.ends_with("E: Unable to locate package nope"),
            "{shown}"
        );
        assert!(
            shown.chars().count() <= STDERR_DISPLAY_CHARS,
            "{}",
            shown.len()
        );
        assert_eq!(
            super::shown("bell\u{7}\u{202e}\n\n  next \r\n"),
            "bell\nnext"
        );
        let many: String = (0..20).map(|n| format!("line {n}\n")).collect();
        assert_eq!(super::shown(&many).lines().next(), Some("line 8"));
    }
}
