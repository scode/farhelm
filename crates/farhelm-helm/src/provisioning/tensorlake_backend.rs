//! Sandbox preparation and bounded agent-settings reads on the ordinary
//! provisioning ssh connection. Secrets only travel through captured stdout
//! and upload stdin; every read failure discards the capture's diagnostic.

use super::*;

pub(super) const NO_SYSTEMD: &str = "This sandbox's image does not run systemd as its init system. Your image must have systemd installed, enabled, and running as the init system at startup (for example, images built from tensorlake/ubuntu-systemd).";

/// Start logind and linger for the actual login user, then wait for the user
/// manager. The gateway does not run PAM, so merely logging in cannot do this.
const PREPARE: &str = r#"
[ "$(cat /proc/1/comm 2>/dev/null)" = systemd ] || exit 3
sudo -n systemctl unmask systemd-logind && sudo -n systemctl daemon-reload && sudo -n systemctl start systemd-logind && sudo -n loginctl enable-linger "$(id -un)" || exit 4
uid=$(id -u) || exit 4
i=0
while [ "$i" -lt 30 ]; do
    s=$(XDG_RUNTIME_DIR=/run/user/$uid systemctl --user is-system-running 2>/dev/null)
    case "$s" in running|degraded) printf '%s' "$HOME"; exit 0;; esac
    i=$((i + 1)); sleep 1
done
echo 'the sandbox user manager did not start within 30 seconds' >&2
exit 5
"#;

impl SystemBackend {
    /// Retry only ssh transport failures while the sandbox gateway catches up
    /// with the API's Running state. A rejected key needs registration, not time.
    pub(super) async fn prepare_sandbox(
        &self,
        target: &ProvisioningTarget,
    ) -> Result<PathBuf, BackendFailure> {
        require_remote(target, "preparing a Tensorlake sandbox")?;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
        loop {
            let output = self
                .run_shell(target, "true", Duration::from_secs(15))
                .await?;
            match crate::tensorlake::ssh::classify(output.code, &output.stderr) {
                crate::tensorlake::ssh::SshOutcome::Exited(0) => break,
                crate::tensorlake::ssh::SshOutcome::AuthRefused => {
                    return Err(BackendFailure::new(
                        crate::tensorlake::ssh::AUTH_REFUSED_MESSAGE,
                        "",
                    ));
                }
                crate::tensorlake::ssh::SshOutcome::Unreachable
                    if tokio::time::Instant::now() < deadline =>
                {
                    tokio::time::sleep(Duration::from_secs(1)).await
                }
                _ => {
                    return Err(BackendFailure::new(
                        "reaching the Tensorlake sandbox",
                        output.stderr,
                    ));
                }
            }
        }
        let output = self
            .run_shell(target, PREPARE, Duration::from_secs(90))
            .await?;
        match output.code {
            Some(3) => return Err(BackendFailure::new(NO_SYSTEMD, "")),
            Some(4) => {
                return Err(BackendFailure::new(
                    "Farhelm needs passwordless sudo in the sandbox to start its login manager",
                    "",
                ));
            }
            Some(0) => {}
            _ => {
                return Err(BackendFailure::new(
                    "preparing the sandbox's user manager",
                    output.stderr,
                ));
            }
        }
        let home = PathBuf::from(
            String::from_utf8(output.stdout)
                .map_err(|_| BackendFailure::new("the sandbox's home is not UTF-8", ""))?,
        );
        if !home.is_absolute() {
            return Err(BackendFailure::new(
                "the sandbox did not report an absolute home",
                "",
            ));
        }
        Ok(home)
    }

    /// Read in 32 KiB chunks so the normal 64 KiB command-output budget
    /// stays intact. Size is checked first and again after reading: a file
    /// changed during this read is refused, not partially merged.
    pub(super) async fn read_agent_file(
        &self,
        target: &ProvisioningTarget,
        path: &Path,
    ) -> Result<Option<Vec<u8>>, BackendFailure> {
        let path = shell_path(path)?;
        let failure = || {
            BackendFailure::new(
                "could not read the agent settings (the file must be readable and at most 1 MiB)",
                "",
            )
        };
        let size_script =
            format!("if [ ! -e {path} ] && [ ! -L {path} ]; then exit 75; fi; wc -c < {path}");
        let size = self
            .run_shell(target, &size_script, COMMAND_TIMEOUT)
            .await
            .map_err(|_| failure())?;
        if size.code == Some(75) {
            return Ok(None);
        }
        if size.code != Some(0) {
            return Err(failure());
        }
        let size = std::str::from_utf8(&size.stdout)
            .ok()
            .and_then(|s| s.trim().parse::<usize>().ok())
            .filter(|n| *n <= 1024 * 1024)
            .ok_or_else(failure)?;
        let mut bytes = Vec::with_capacity(size);
        for block in 0..size.div_ceil(32768) {
            let script = format!("dd if={path} bs=32768 skip={block} count=1 2>/dev/null");
            let out = self
                .run_shell(target, &script, COMMAND_TIMEOUT)
                .await
                .map_err(|_| failure())?;
            if out.code != Some(0) {
                return Err(failure());
            }
            bytes.extend(out.stdout);
        }
        let after = self
            .run_shell(target, &size_script, COMMAND_TIMEOUT)
            .await
            .map_err(|_| failure())?;
        if bytes.len() != size
            || after.code != Some(0)
            || String::from_utf8_lossy(&after.stdout).trim() != size.to_string()
        {
            return Err(failure());
        }
        Ok(Some(bytes))
    }

    /// A reused Farhelm state directory may carry another host's identity.
    /// Refuse any existing database before the first install; reruns bypass
    /// this check only after this row records its own installation paths.
    pub(super) async fn check_copied_state(
        &self,
        target: &ProvisioningTarget,
        state: &Path,
    ) -> Result<(), BackendFailure> {
        let path = shell_path(&state.join("supervisor.db"))?;
        self.require_shell(target, &format!("[ ! -e {path} ] && [ ! -L {path} ]"), "This image or snapshot already contains Farhelm supervisor state. Use an image without a Farhelm installation; copying a host's supervisor identity would duplicate that host").await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::VecDeque, sync::Mutex};

    /// Replace only child execution, leaving ssh construction, bounded
    /// capture and preparation's status classification in production code.
    /// The scripts run behind the ssh stand-in, so a sandbox command gets the
    /// pipe behavior of whatever connection its argv asks for.
    struct Scripts(Mutex<VecDeque<&'static str>>);
    impl CommandLauncher for Scripts {
        fn spawn(
            &self,
            command: &mut tokio::process::Command,
        ) -> std::io::Result<tokio::process::Child> {
            let script = self
                .0
                .lock()
                .unwrap()
                .pop_front()
                .expect("expected command");
            let mut child = tokio::process::Command::new("sh");
            child.args(["-c", script]);
            crate::provisioning::ssh_stand_in::spawn_remote(command, child)
        }
    }

    /// A rejected key is not retried; a sandbox without systemd receives
    /// the product's full image requirement rather than an SSH diagnosis.
    #[farhelm_testtrace::test]
    async fn preparation_classifies_key_and_systemd_refusals() {
        let root = tempfile::tempdir().unwrap();
        let target = ProvisioningTarget::Ssh {
            destination: "sandbox@sandbox.sandbox.tensorlake.ai".into(),
            access: crate::ssh::SshAccessKind::Tensorlake,
        };
        for (scripts, expected) in [
            (
                vec!["printf 'Permission denied (publickey).' >&2; exit 255"],
                crate::tensorlake::ssh::AUTH_REFUSED_MESSAGE,
            ),
            (vec!["exit 0", "exit 3"], NO_SYSTEMD),
            (vec!["exit 0", "exit 4"], "passwordless sudo"),
        ] {
            let scripts = Arc::new(Scripts(Mutex::new(scripts.into())));
            let mut backend = SystemBackend::new(root.path().to_path_buf());
            backend.launcher = scripts.clone();
            let failure = backend.prepare_sandbox(&target).await.unwrap_err();
            assert!(
                failure.rendered().contains(expected),
                "{}",
                failure.rendered()
            );
            assert!(scripts.0.lock().unwrap().is_empty());
        }
    }

    /// Every remote script inherits the runtime fallback, while the script
    /// remains one quoted shell word. This covers ordinary SSH too, which
    /// can lack PAM's runtime setting just like Tensorlake's gateway.
    #[test]
    fn remote_scripts_have_the_runtime_fallback_and_prepare_parses() {
        let root = tempfile::tempdir().unwrap();
        let backend = SystemBackend::new(root.path().to_path_buf());
        for access in [
            crate::ssh::SshAccessKind::UserConfig,
            crate::ssh::SshAccessKind::Tensorlake,
        ] {
            let command = backend
                .ssh_command("user@host", access, "printf 'a b'".into())
                .unwrap();
            let args: Vec<_> = command.as_std().get_args().collect();
            let words = shell_words::split(args.last().unwrap().to_str().unwrap()).unwrap();
            assert_eq!(words[0..2], ["sh", "-c"]);
            assert_eq!(words[2], format!("{REMOTE_RUNTIME_PREFIX}printf 'a b'"));
        }
        assert!(
            std::process::Command::new("sh")
                .args(["-n", "-c", PREPARE])
                .status()
                .unwrap()
                .success()
        );
    }
}
