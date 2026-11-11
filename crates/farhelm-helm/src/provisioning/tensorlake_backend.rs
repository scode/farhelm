//! Sandbox preparation and bounded agent-settings reads on the selected
//! provisioning SSH transport. Secrets only travel through captured stdout
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
    /// Preserve the enabled boot installation while removing everything that
    /// could give clones the preparer's identity. The recipe engine may take
    /// its checkpoint only after this command succeeds; no host attachment or
    /// session is ever created during preparation.
    pub(super) async fn finish_prepared_install(
        &self,
        target: &ProvisioningTarget,
        unit: &str,
        state: &Path,
    ) -> Result<(), BackendFailure> {
        require_remote(target, "finishing the prepared snapshot installation")?;
        let script = snapshot_cleanup_script(
            unit,
            state,
            Path::new("/sys/fs/cgroup"),
            Path::new("/etc/machine-id"),
        )?;
        self.require_shell_within(
            target,
            &script,
            "stopping and clearing the prepared supervisor",
            Duration::from_secs(150),
        )
        .await?;
        Ok(())
    }

    /// Retry only ssh transport failures while the sandbox gateway catches up
    /// with the API's Running state. A rejected key needs registration, not time.
    pub(super) async fn wait_sandbox_ssh(
        &self,
        target: &ProvisioningTarget,
    ) -> Result<(), BackendFailure> {
        require_remote(target, "reaching a Tensorlake sandbox")?;
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
        Ok(())
    }

    /// Ordinary installation needs logind and linger after transport is ready.
    /// Prepared clones use the readiness wait alone, preserving their boot proof.
    pub(super) async fn prepare_sandbox(
        &self,
        target: &ProvisioningTarget,
    ) -> Result<PathBuf, BackendFailure> {
        self.wait_sandbox_ssh(target).await?;
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

/// Build the final checkpoint guard with injectable platform paths for owned
/// filesystem tests. State deletion follows ordinary uninstall's symlink and
/// mount boundary; linked ancestors are allowed, but the state itself cannot
/// be a link. A cgroup's populated flag covers descendants that MainPID alone
/// misses under the installed unit's process-only stop policy.
fn snapshot_cleanup_script(
    unit: &str,
    state: &Path,
    cgroup_root: &Path,
    machine_id: &Path,
) -> Result<String, BackendFailure> {
    Ok(format!(
        r#"
export LC_ALL=C
unit={unit}
data={data}
cgroup_root={cgroup_root}
machine_id={machine_id}
{mounts}
absent() {{
    if detail=$(stat -c '%F' -- "$1" 2>&1); then
        printf '%s\n' "prepared snapshot still contains $1" >&2
        return 1
    else
        status=$?
        case "$detail" in
            {missing}) return 0 ;;
            *) printf '%s\n' "$detail" >&2; return "$status" ;;
        esac
    fi
}}
cgroup_empty() {{
    if detail=$(stat -c '%F' -- "$group" 2>&1); then
        events=$(cat -- "$group/cgroup.events" 2>/dev/null) || return 2
        printf '%s\n' "$events" | grep -qx 'populated 0' && return 0
        printf '%s\n' "$events" | grep -qx 'populated 1' && return 1
        return 2
    else
        case "$detail" in {missing}) return 0 ;; *) return 2 ;; esac
    fi
}}
enabled=$(systemctl --user is-enabled -- "$unit") && [ "$enabled" = enabled ] || {{ echo 'the supervisor unit is not persistently enabled' >&2; exit 1; }}
user=$(id -un) || exit 1
linger=$(loginctl show-user -p Linger --value -- "$user") && [ "$linger" = yes ] || {{ echo 'the sandbox user does not have boot linger enabled' >&2; exit 1; }}
active=$(systemctl --user show -p ActiveState --value -- "$unit") && [ "$active" = active ] || {{ echo 'the installed supervisor did not become active' >&2; exit 1; }}
cg=$(systemctl --user show -p ControlGroup --value -- "$unit") || exit 1
case "$cg" in /*) [ "$cg" != / ] || exit 1 ;; *) exit 1 ;; esac
group=$cgroup_root$cg
if cgroup_empty; then
    echo 'the active supervisor has no populated service cgroup to verify' >&2; exit 1
else
    status=$?
    [ "$status" = 1 ] || {{ echo 'cannot verify the supervisor service cgroup' >&2; exit 1; }}
fi
systemctl --user stop -- "$unit" || exit 1
if cgroup_empty; then :; else
    status=$?
    [ "$status" = 1 ] || {{ echo 'cannot verify the supervisor service cgroup' >&2; exit 1; }}
    systemctl --user kill --kill-who=all --signal=KILL -- "$unit" || exit 1
fi
i=0
while :; do
    active=$(systemctl --user show -p ActiveState --value -- "$unit") || exit 1
    main=$(systemctl --user show -p MainPID --value -- "$unit") || exit 1
    control=$(systemctl --user show -p ControlPID --value -- "$unit") || exit 1
    empty=no
    if cgroup_empty; then empty=yes; else
        status=$?
        [ "$status" = 1 ] || {{ echo 'cannot verify the supervisor service cgroup' >&2; exit 1; }}
    fi
    case "$active:$main:$control:$empty" in inactive:0:0:yes|failed:0:0:yes) break ;; esac
    [ "$i" -lt 30 ] || {{ echo 'the prepared supervisor did not stop within 30 seconds' >&2; exit 1; }}
    i=$((i + 1)); sleep 1
done
if [ -L "$data" ]; then
    echo 'refusing to clear supervisor state through a symbolic link' >&2; exit 1
fi
if stat -c '%F' -- "$data" >/dev/null 2>&1; then
    resolved=$(readlink -f -- "$data") && [ -n "$resolved" ] || exit 1
    inside=$(mount_points "$resolved" '\n') || exit 1
    [ -z "$inside" ] || {{ echo 'refusing to clear supervisor state with mounts on or inside it' >&2; exit 1; }}
    rm -rf -- "$data" || exit 1
else
    absent "$data" || exit 1
fi
sudo -n truncate -s 0 -- "$machine_id" || exit 1
size=$(stat -L -c '%s' -- "$machine_id") && [ "$size" = 0 ] || exit 1
enabled=$(systemctl --user is-enabled -- "$unit") && [ "$enabled" = enabled ] || exit 1
active=$(systemctl --user show -p ActiveState --value -- "$unit") || exit 1
case "$active" in inactive|failed) ;; *) echo 'the prepared supervisor restarted' >&2; exit 1 ;; esac
absent "$data/supervisor.db" && absent "$data" || exit 1
"#,
        unit = crate::ssh::shell_quote(unit),
        data = shell_path(state)?,
        cgroup_root = shell_path(cgroup_root)?,
        machine_id = shell_path(machine_id)?,
        mounts = MOUNT_POINTS_SH,
        missing = STAT_MISSING_PATH_PATTERNS,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::VecDeque, sync::Mutex};

    /// Execute the real cleanup script against owned files, with simulated
    /// systemd/logind/sudo and explicit rm/stat fault injection. Unexpected
    /// lifecycle verbs fail, so disabling the unit cannot look successful.
    #[cfg(target_os = "linux")]
    struct SnapshotFixture {
        root: tempfile::TempDir,
        state: PathBuf,
    }

    #[cfg(target_os = "linux")]
    impl SnapshotFixture {
        /// Seed both identity and unrelated state; clearing selected known
        /// files would leave the future-state sentinel and fail the test.
        fn new() -> Self {
            let root = tempfile::tempdir().unwrap();
            let state = root.path().join("state with spaces");
            std::fs::create_dir_all(state.join("future-state/nested")).unwrap();
            std::fs::write(state.join("supervisor.db"), "copied identity").unwrap();
            std::fs::write(state.join("future-state/nested/sentinel"), "state").unwrap();
            std::fs::create_dir_all(root.path().join("cgroups/owned.service")).unwrap();
            std::fs::write(
                root.path().join("cgroups/owned.service/cgroup.events"),
                "populated 1\n",
            )
            .unwrap();
            std::fs::write(root.path().join("active"), "active").unwrap();
            std::fs::write(root.path().join("machine-id"), "inherited machine identity").unwrap();
            std::fs::write(root.path().join("unit"), "installed supervisor unit").unwrap();
            #[cfg(unix)]
            std::os::unix::fs::symlink(root.path().join("unit"), root.path().join("enabled"))
                .unwrap();
            Self { root, state }
        }

        /// Arguments supply fixture state without changing the test process's
        /// environment. Cgroup files model observed process ownership, while
        /// state removal and all absence checks execute as real shell tools.
        fn finish(&self, scenario: &str) -> std::process::Output {
            let prelude = r#"
fixture=$1
scenario=$2
systemctl() {
    case "$2" in
        is-enabled) [ -L "$fixture/enabled" ] && printf enabled ;;
        show) case "$4" in
            ActiveState) cat "$fixture/active" ;;
            ControlGroup) printf /owned.service ;;
            MainPID) if [ "$(cat "$fixture/active")" = active ]; then printf 123; else printf 0; fi ;;
            ControlPID) printf 0 ;;
            *) return 91 ;;
        esac ;;
        stop)
            printf inactive > "$fixture/active"
            printf stop >> "$fixture/events"
            case "$scenario" in
                residual) printf 'populated 1\n' > "$fixture/cgroups/owned.service/cgroup.events" ;;
                unverifiable) printf 'unrecognized\n' > "$fixture/cgroups/owned.service/cgroup.events" ;;
                *) printf 'populated 0\n' > "$fixture/cgroups/owned.service/cgroup.events" ;;
            esac ;;
        kill)
            printf kill >> "$fixture/events"
            printf 'populated 0\n' > "$fixture/cgroups/owned.service/cgroup.events" ;;
        *) return 91 ;;
    esac
}
loginctl() { if [ "$scenario" = no-linger ]; then printf no; else printf yes; fi; }
sudo() { [ "$1" = -n ] || return 91; shift; "$@"; }
rm() { if [ "$scenario" = retained ]; then return 0; else command rm "$@"; fi; }
stat() {
    if [ "$scenario" = inspection-failed ] && [ ! -e "$fixture/state with spaces" ]; then
        printf 'stat: simulated I/O failure\n' >&2; return 1
    fi
    command stat "$@"
}
"#;
            let script = snapshot_cleanup_script(
                "owned.service",
                &self.state,
                &self.root.path().join("cgroups"),
                &self.root.path().join("machine-id"),
            )
            .unwrap();
            std::process::Command::new("sh")
                .args(["-c", &format!("{prelude}\n{script}"), "snapshot-fixture"])
                .arg(self.root.path())
                .arg(scenario)
                .output()
                .unwrap()
        }
    }

    /// The checkpoint must contain a boot installation and no identity or
    /// future per-install files. A leftover child is killed even after the
    /// supervisor's MainPID disappeared, without removing enablement.
    #[cfg(target_os = "linux")]
    #[test]
    fn snapshot_cleanup_preserves_boot_install_and_removes_whole_state() {
        for scenario in ["ordinary", "residual"] {
            let fixture = SnapshotFixture::new();
            assert!(fixture.state.join("supervisor.db").is_file());
            assert!(fixture.root.path().join("enabled").is_symlink());
            let output = fixture.finish(scenario);
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(std::fs::symlink_metadata(&fixture.state).is_err());
            assert!(fixture.root.path().join("enabled").is_symlink());
            assert_eq!(
                std::fs::read_to_string(fixture.root.path().join("unit")).unwrap(),
                "installed supervisor unit"
            );
            assert_eq!(
                std::fs::read(fixture.root.path().join("machine-id")).unwrap(),
                b""
            );
            assert_eq!(
                std::fs::read_to_string(fixture.root.path().join("events")).unwrap(),
                if scenario == "residual" {
                    "stopkill"
                } else {
                    "stop"
                }
            );
        }
    }

    /// A successful rm exit alone is no checkpoint proof. Retained state,
    /// failed absence inspection, missing boot linger and unknown cgroup
    /// evidence all make the real finalization command fail.
    #[cfg(target_os = "linux")]
    #[test]
    fn snapshot_cleanup_refuses_unverified_state_or_boot() {
        for scenario in ["retained", "inspection-failed", "no-linger", "unverifiable"] {
            let fixture = SnapshotFixture::new();
            let output = fixture.finish(scenario);
            assert!(
                !output.status.success(),
                "{scenario} unexpectedly succeeded"
            );
            if scenario != "inspection-failed" {
                assert!(fixture.state.join("supervisor.db").exists());
            }
        }
    }

    /// A dangling state link is still captured state; refusing it also
    /// prevents treating its missing referent as proof that cleanup finished.
    #[cfg(target_os = "linux")]
    #[test]
    fn snapshot_cleanup_refuses_dangling_state_link() {
        let mut fixture = SnapshotFixture::new();
        fixture.state = fixture.root.path().join("linked-state");
        std::os::unix::fs::symlink(fixture.root.path().join("missing"), &fixture.state).unwrap();
        assert!(
            std::fs::symlink_metadata(&fixture.state)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        let output = fixture.finish("ordinary");
        assert!(!output.status.success());
        assert!(fixture.state.is_symlink());
    }

    /// A linked home is an accepted path alias, while an interior link must
    /// be unlinked without erasing the unrelated tree it names. Both follow
    /// the existing directory-removal contract rather than banning aliases.
    #[cfg(target_os = "linux")]
    #[test]
    fn snapshot_cleanup_accepts_linked_ancestors_and_spares_link_targets() {
        let mut fixture = SnapshotFixture::new();
        let other = fixture.root.path().join("unrelated");
        std::fs::create_dir(&other).unwrap();
        std::fs::write(other.join("sentinel"), "keep").unwrap();
        std::os::unix::fs::symlink(&other, fixture.state.join("linked-content")).unwrap();
        let linked_home = fixture.root.path().join("linked-home");
        std::os::unix::fs::symlink(fixture.root.path(), &linked_home).unwrap();
        fixture.state = linked_home.join("state with spaces");
        assert!(fixture.state.join("linked-content/sentinel").exists());
        let output = fixture.finish("ordinary");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(std::fs::symlink_metadata(&fixture.state).is_err());
        assert_eq!(
            std::fs::read_to_string(other.join("sentinel")).unwrap(),
            "keep"
        );
        assert!(linked_home.is_symlink());
    }

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
        let target = ProvisioningTarget::Remote(crate::tensorlake::ssh::route(
            "sandbox@sandbox.sandbox.tensorlake.ai",
        ));
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

    /// Running is an API state, not an SSH readiness guarantee. The clone's
    /// transport wait retries a lagging gateway without running user-manager
    /// preparation or any other remote setup script.
    #[farhelm_testtrace::test]
    async fn clone_readiness_retries_transport_without_preparing_the_user_manager() {
        let root = tempfile::tempdir().unwrap();
        let scripts = Arc::new(Scripts(Mutex::new(vec!["exit 255", "exit 0"].into())));
        let mut backend = SystemBackend::new(root.path().to_path_buf());
        backend.launcher = scripts.clone();
        backend
            .wait_sandbox_ssh(&ProvisioningTarget::Remote(crate::tensorlake::ssh::route(
                "clone@clone.sandbox.tensorlake.ai",
            )))
            .await
            .unwrap();
        assert!(
            scripts.0.lock().unwrap().is_empty(),
            "both transport attempts ran"
        );
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
