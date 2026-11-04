//! How Farhelm reaches Tensorlake sandboxes over ssh: the option set, the
//! destination, and what an ssh failure means.
//!
//! Every ssh command Farhelm sends to a sandbox goes through Tensorlake's ssh
//! gateway and authenticates with Farhelm's own key (`ssh_key.rs`), never the
//! user's: the gateway refuses outright when the first key offered is not
//! registered with the account, so the user's agent and `~/.ssh` keys must
//! stay out of it (`IdentitiesOnly=yes`). Host keys go to a known_hosts file
//! Farhelm owns, under one alias, because every `*.sandbox.tensorlake.ai`
//! name presents the same gateway host key (measured 2026-11-03): a single
//! entry covers every sandbox, and the user's own `~/.ssh/known_hosts` is
//! never touched. The key is trusted on first use (`accept-new`), which
//! `BatchMode` otherwise forbids; compiling the key in would break the day
//! Tensorlake rotates it.
//!
//! The snapshot builder uses these options with a one-shot argv
//! ([`crate::ssh::ssh_oneshot_args`]); Tensorlake hosts reuse the same set
//! for their shared connections, so builders and hosts share one key policy
//! and one known_hosts file.
//!
//! The user's ssh config is still read (`-F` is not passed): a user who needs
//! a proxy to reach Tensorlake keeps it. Its `IdentityFile` lines are still
//! offered, but after Farhelm's key, which is what the gateway decides on.

use std::path::{Path, PathBuf};

/// The alias every sandbox's host key is recorded and checked under.
pub(crate) const GATEWAY_HOST_KEY_ALIAS: &str = "sandbox.tensorlake.ai";

/// The file Farhelm records the gateway's host key in, under the helm's
/// Tensorlake directory.
const KNOWN_HOSTS: &str = "known_hosts";

/// The keepalive interval in seconds, and how many unanswered keepalives end
/// the connection. The gateway dropped connections that carried no traffic
/// for 300 to 400 seconds (measured 2026-11-03); a 30-second keepalive keeps
/// a long, quiet step (an apt install, a snapshot of a large disk) from
/// silently losing its connection, and 3 misses notice a dead one in 90 s.
const SERVER_ALIVE_INTERVAL: u32 = 30;
const SERVER_ALIVE_COUNT_MAX: u32 = 3;

/// Farhelm's ssh credentials for Tensorlake: its private key and its
/// known_hosts file. Both paths are on the helm's machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TensorlakeSshAccess {
    pub(crate) identity_file: PathBuf,
    pub(crate) known_hosts: PathBuf,
}

impl TensorlakeSshAccess {
    /// The access for a private key, with the known_hosts file beside the
    /// key directory in the helm's Tensorlake directory.
    pub(crate) fn new(tensorlake_dir: &Path, identity_file: PathBuf) -> Self {
        Self {
            identity_file,
            known_hosts: tensorlake_dir.join(KNOWN_HOSTS),
        }
    }

    /// The `-o` options every Tensorlake ssh command carries after the
    /// connection overrides, in a fixed order. Paths are config-encoded, so a
    /// state directory with spaces, quotes or `%` still names the right file.
    ///
    /// `GlobalKnownHostsFile=/dev/null` keeps a system-wide entry for the
    /// alias (unlikely, but outside Farhelm's control) from overriding the
    /// one Farhelm recorded. `LogLevel=ERROR` drops the gateway's login
    /// banner and ssh's "Permanently added" note, which would otherwise
    /// open every failed command's stderr; a refused key's "Permission
    /// denied (publickey)" is an error and still shows (checked live). Fails only for a path that is not UTF-8, which
    /// ssh's option text cannot carry.
    pub(crate) fn options(&self) -> Result<Vec<String>, String> {
        let text = |path: &Path| {
            path.to_str().map(str::to_string).ok_or_else(|| {
                format!(
                    "{} is not valid UTF-8, which ssh cannot use as a path; move the helm's state directory",
                    path.to_string_lossy()
                )
            })
        };
        Ok(vec![
            crate::ssh::ssh_config_path_option("IdentityFile", &text(&self.identity_file)?),
            "IdentitiesOnly=yes".to_string(),
            crate::ssh::ssh_config_path_option("UserKnownHostsFile", &text(&self.known_hosts)?),
            "GlobalKnownHostsFile=/dev/null".to_string(),
            format!("HostKeyAlias={GATEWAY_HOST_KEY_ALIAS}"),
            "StrictHostKeyChecking=accept-new".to_string(),
            format!("ServerAliveInterval={SERVER_ALIVE_INTERVAL}"),
            format!("ServerAliveCountMax={SERVER_ALIVE_COUNT_MAX}"),
            "LogLevel=ERROR".to_string(),
        ])
    }
}

/// The ssh destination of a sandbox: `<name>@<name>.sandbox.tensorlake.ai`.
/// The gateway takes the sandbox's name (or ID) both as the user and as the
/// host's first label.
pub(crate) fn destination(sandbox: &str) -> String {
    format!("{sandbox}@{sandbox}.{GATEWAY_HOST_KEY_ALIAS}")
}

/// What a finished ssh command's exit means.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SshOutcome {
    /// The remote command ran and exited with this status.
    Exited(i32),
    /// The gateway refused Farhelm's key: it is not registered with the
    /// user's Tensorlake account.
    AuthRefused,
    /// ssh itself failed (exit 255) before or while running the command:
    /// the sandbox's sshd not up yet, the gateway unreachable, or a dropped
    /// connection.
    Unreachable,
}

/// Classify a finished ssh command from its exit status and stderr.
///
/// ssh reserves 255 for its own failures, so only 255 is a transport
/// failure; a remote command that exits 255 itself is misread as one, which
/// the build scripts never do. A refused key is told apart by OpenSSH's own
/// message, the only signal there is.
pub(crate) fn classify(code: Option<i32>, stderr: &str) -> SshOutcome {
    match code {
        Some(255) if stderr.contains("Permission denied (publickey") => SshOutcome::AuthRefused,
        Some(255) | None => SshOutcome::Unreachable,
        Some(code) => SshOutcome::Exited(code),
    }
}

/// What the user sees when the gateway refuses Farhelm's key.
pub(crate) const AUTH_REFUSED_MESSAGE: &str = "Tensorlake refused Farhelm's SSH key. Register it once with the command in Settings → Tensorlake, then try again.";

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec: the options are Farhelm's key with `IdentitiesOnly`, Farhelm's
    /// known_hosts beside the key directory under the gateway alias with no
    /// global file, trust on first use, a 30-second keepalive, and errors
    /// only in ssh's own output, in that order; paths with spaces, quotes and `%` survive ssh's config
    /// parsing. Composed with the one-shot argv, `--` still precedes the
    /// destination.
    ///
    /// Why: these options are the whole difference between reaching a
    /// sandbox and being refused by the gateway (the user's own keys first),
    /// writing the user's known_hosts, or prompting under `BatchMode`.
    #[farhelm_testtrace::test]
    fn options_name_farhelms_key_and_known_hosts_and_never_share() {
        let dir = Path::new("/state dir/te'n%s\"or/tensorlake");
        let access = TensorlakeSshAccess::new(dir, dir.join("ssh-key/id_ed25519"));
        assert_eq!(access.known_hosts, dir.join("known_hosts"));
        assert_eq!(
            access.options().unwrap(),
            [
                r#"IdentityFile="/state dir/te'n%%s\"or/tensorlake/ssh-key/id_ed25519""#,
                "IdentitiesOnly=yes",
                r#"UserKnownHostsFile="/state dir/te'n%%s\"or/tensorlake/known_hosts""#,
                "GlobalKnownHostsFile=/dev/null",
                "HostKeyAlias=sandbox.tensorlake.ai",
                "StrictHostKeyChecking=accept-new",
                "ServerAliveInterval=30",
                "ServerAliveCountMax=3",
                "LogLevel=ERROR",
            ]
        );
        let argv = crate::ssh::ssh_oneshot_args(
            &destination("farhelm-build-k7q2-rust"),
            &access.options().unwrap(),
        );
        assert_eq!(
            &argv[argv.len() - 2..],
            [
                "--",
                "farhelm-build-k7q2-rust@farhelm-build-k7q2-rust.sandbox.tensorlake.ai"
            ]
        );
        for option in ["ControlMaster=no", "ControlPath=none", "BatchMode=yes"] {
            assert!(
                argv.windows(2).any(|pair| pair == ["-o", option]),
                "{option}"
            );
        }
    }

    /// Spec: exit 255 with OpenSSH's publickey refusal is an auth refusal,
    /// any other 255 (or no status) is unreachable, and every other status
    /// is the remote command's own.
    ///
    /// Why: the build retries an unreachable sandbox for a while (sshd can
    /// lag the sandbox's "running" state) but must fail at once, with the
    /// registration hint, when the key is not registered.
    #[farhelm_testtrace::test]
    fn ssh_failures_are_classified_by_status_and_message() {
        let refused =
            "farhelm-build-x@farhelm-build-x.sandbox.tensorlake.ai: Permission denied (publickey).";
        assert_eq!(classify(Some(255), refused), SshOutcome::AuthRefused);
        assert_eq!(
            classify(Some(255), "Connection closed by 1.2.3.4 port 22"),
            SshOutcome::Unreachable
        );
        assert_eq!(classify(None, ""), SshOutcome::Unreachable);
        assert_eq!(classify(Some(1), refused), SshOutcome::Exited(1));
        assert_eq!(classify(Some(0), ""), SshOutcome::Exited(0));
    }
}
