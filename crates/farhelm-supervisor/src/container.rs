//! Rootless Podman launch and teardown, with one cross-process creation fence.
//!
//! The creator lives outside agent containment so conmon and pasta survive agent
//! exit. Consequently teardown must revoke creation under the same flock the
//! creator holds: the supervisor's in-memory lifecycle claim cannot stop a late
//! shim in another process. No credential contents are persisted in this module.

use anyhow::{Context, bail};
use farhelm_proto::ContainerConfig;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::launch::PreparationLock;

/// Everything the terminal shim needs, excluding the optional token's contents.
/// The creator argv uses absolute host executables and a separately scoped helper;
/// the agent itself is always an exec inside the resulting container.
#[derive(Clone, Serialize, Deserialize)]
pub struct ContainerLaunch {
    pub runtime: ContainerRuntime,
    pub creator: Vec<String>,
}

/// Stable derivations plus launch intent. Names are never accepted from a stored
/// row: they are derived from its validated UUID each time they are used.
#[derive(Clone, Serialize, Deserialize)]
pub struct ContainerRuntime {
    pub config: ContainerConfig,
    pub podman: PathBuf,
    pub session_id: String,
    pub generation: i64,
    pub cwd: PathBuf,
    pub state_dir: PathBuf,
}

/// Only creation needs the shared checkout root. Control reconstruction must
/// still work after a checkout, image or token file disappears, while exec keeps
/// its separate canonical working directory. This ticket carries no credential.
#[derive(Serialize, Deserialize)]
struct CreationTicket {
    runtime: ContainerRuntime,
    checkout_root: PathBuf,
}

/// Reuse the session UUID without matching the numeric agent or tab scope globs.
pub(crate) fn unit_name(id: &str) -> anyhow::Result<String> {
    let id = uuid::Uuid::parse_str(id).context("invalid container session identity")?;
    Ok(format!("farhelm-{id}-container.scope"))
}

/// The image and token are paths on this host, never on the helm's machine.
/// Keep checks cheap and certain; Podman reports capability failures in the pane.
pub(crate) fn preflight(config: &ContainerConfig) -> anyhow::Result<PathBuf> {
    config.validate().map_err(anyhow::Error::msg)?;
    anyhow::ensure!(
        cfg!(target_os = "linux"),
        "containerized sessions require Linux"
    );
    let podman = executable()?;
    anyhow::ensure!(
        std::fs::metadata(&config.image_file).is_ok_and(|metadata| metadata.is_file()),
        "container image file must be an existing regular file"
    );
    drop(read_token(config)?);
    Ok(podman)
}

/// Resolve once before shell startup so that a shell cannot substitute Podman.
pub(crate) fn executable() -> anyhow::Result<PathBuf> {
    crate::scope::resolve_program("podman")
        .context("containerized sessions require podman on this host")
}

/// Read at the point of delivery, with errors that disclose no contents. Preflight
/// uses the same check and immediately drops its value; it is never a token cache.
pub(crate) fn read_token(config: &ContainerConfig) -> anyhow::Result<Option<String>> {
    config
        .github_token_file
        .as_ref()
        .map(|path| {
            let text = std::fs::read_to_string(path)
                .context("could not read the container's GitHub token file")?;
            let token = text.trim();
            anyhow::ensure!(!token.is_empty(), "container GitHub token file is empty");
            anyhow::ensure!(
                !token.contains('\0'),
                "container GitHub token file contains NUL"
            );
            Ok(token.to_string())
        })
        .transpose()
}

/// Remove authority before even an inspection command can initialize Podman's
/// shared pause process. Ordinary host XDG/DBus/container configuration still applies.
pub(crate) fn control_command(program: &Path) -> Command {
    let mut command = Command::new(program);
    for (name, _) in std::env::vars_os() {
        if name
            .to_str()
            .is_some_and(|name| name.starts_with("FARHELM_"))
        {
            command.env_remove(name);
        }
    }
    for name in crate::launch::PRIVATE_TMUX_ENV_VARS {
        command.env_remove(name);
    }
    command
}

impl ContainerRuntime {
    /// Reconstruct control authority from the stored launch without rechecking inputs
    /// that may legitimately disappear before Stop/Delete, such as the image archive.
    pub(crate) fn for_session(
        state_dir: &Path,
        id: &str,
        generation: i64,
        cwd: &str,
        launch: &farhelm_proto::SessionLaunch,
    ) -> anyhow::Result<Option<Self>> {
        launch
            .agent_selection()
            .and_then(|selection| selection.container.as_ref())
            .map(|config| {
                Ok(Self {
                    config: config.clone(),
                    podman: executable()?,
                    session_id: id.into(),
                    generation,
                    cwd: cwd.into(),
                    state_dir: state_dir.into(),
                })
            })
            .transpose()
    }
    /// One container belongs to the whole session, independent of launch generation.
    pub(crate) fn name(&self) -> anyhow::Result<String> {
        let id = uuid::Uuid::parse_str(&self.session_id)?;
        Ok(format!("farhelm-{id}"))
    }

    /// A permanent inode prevents an old waiter and a new opener from taking
    /// different locks. Delete removes the permit, never this lock file.
    fn lock_path(&self) -> PathBuf {
        self.state_dir
            .join("container-launch")
            .join(format!("{}.lock", self.session_id))
    }

    /// The only positive authority to create, distinct from the permanent lock.
    /// Removing this file revokes delayed helpers without splitting lock identity.
    fn ticket_path(&self) -> PathBuf {
        self.state_dir
            .join("container-launch")
            .join(format!("{}.json", self.session_id))
    }

    /// Acquire without an abandoned blocking waiter. The caller already owns the
    /// ordinary lifecycle claim; expiry changes no permit and sends no signals.
    pub(crate) async fn acquire(&self) -> anyhow::Result<ContainerGuard<'_>> {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(self.state_dir.join("container-launch"))?;
        let lock = PreparationLock::open(&self.lock_path())?;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
        loop {
            if lock.try_acquire()? {
                return Ok(ContainerGuard {
                    runtime: self,
                    _lock: lock,
                });
            }
            anyhow::ensure!(
                tokio::time::Instant::now() < deadline,
                "container image preparation is still running; retry the operation after it finishes"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Inspect only bounded state text. Exit 1 from `exists` means absent; a
    /// connection/storage failure is not absence and cannot authorize cleanup.
    pub(crate) async fn state(&self) -> anyhow::Result<Option<String>> {
        let name = self.name()?;
        let exists = self.run(&["container", "exists", &name]).await?;
        if exists.status.code() == Some(1) {
            return Ok(None);
        }
        anyhow::ensure!(
            exists.status.success(),
            "could not inspect container existence: {}",
            String::from_utf8_lossy(&exists.stderr)
        );
        let output = self
            .run(&[
                "container",
                "inspect",
                "--format",
                "{{.State.Status}}",
                &name,
            ])
            .await?;
        anyhow::ensure!(
            output.status.success(),
            "could not inspect container state: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let state = std::str::from_utf8(&output.stdout)?.trim();
        anyhow::ensure!(
            !state.is_empty(),
            "podman returned an empty container state"
        );
        Ok(Some(state.to_string()))
    }

    /// Control operations are bounded and their output capped. A timeout is a
    /// failed confirmation; the retained session remains available for retry.
    async fn run(&self, args: &[&str]) -> anyhow::Result<std::process::Output> {
        use crate::bounded_command::{Ended, GroupChild, OutputCaps};
        let command = control_command(&self.podman);
        let mut command = tokio::process::Command::from(command);
        command.args(args);
        let caps = OutputCaps {
            stdout: 65536,
            stderr: Some(65536),
        };
        let mut child = GroupChild::spawn(&mut command, caps)?;
        match child
            .finish(tokio::time::Instant::now() + Duration::from_secs(30), caps)
            .await?
        {
            Ended::Exited(output) => Ok(output),
            Ended::TimedOut => bail!("container control command timed out; state is unconfirmed"),
            Ended::OutputOverCap => {
                bail!("container control output exceeded its bound; state is unconfirmed")
            }
        }
    }

    /// Build only the host-side exec client. Podman forwards exactly these named
    /// variables; no host credential or Farhelm reporter is an in-container env entry.
    pub(crate) fn exec_command(&self, argv: &[String]) -> anyhow::Result<Command> {
        let mut command = Command::new(&self.podman);
        command.args(["exec", "-it", "--workdir"]).arg(&self.cwd);
        command.args(["--env", "TERM", "--env", "COLORTERM"]);
        command.env("COLORTERM", "truecolor");
        if let Some(token) = read_token(&self.config)? {
            command.env("GH_TOKEN", token).args(["--env", "GH_TOKEN"]);
        }
        command.arg(self.name()?).args(argv);
        Ok(command)
    }

    /// Select the keeper independently of the image's ENTRYPOINT/CMD. Only the
    /// fenced creator may execute this command; constructing it acquires no permit.
    fn create_command(&self, checkout_root: &Path) -> anyhow::Result<Command> {
        let mut command = control_command(&self.podman);
        command
            .args([
                "run",
                "-d",
                "--init",
                "--name",
                &self.name()?,
                "--userns=keep-id:uid=1000,gid=1000",
                "--network",
                &format!("pasta:-T,{}", self.config.api_proxy_port),
                "--entrypoint",
                "bash",
                "--mount",
            ])
            .arg(format!(
                "type=bind,\"source={}\",\"destination={}\"",
                checkout_root
                    .to_str()
                    .context("checkout path is not UTF-8")?
                    .replace('"', "\"\""),
                checkout_root
                    .to_str()
                    .context("checkout path is not UTF-8")?
                    .replace('"', "\"\"")
            ))
            .arg("--workdir")
            .arg(&self.cwd)
            .arg(format!("oci-archive:{}", self.config.image_file))
            .args(["-c", "while :; do sleep 3600; done"])
            .stdin(Stdio::null());
        Ok(command)
    }
}

/// The lifetime of a lifecycle operation's cross-process fence. It is deliberately
/// borrowed from the runtime so no async cleanup can outlive its session owner.
pub(crate) struct ContainerGuard<'a> {
    runtime: &'a ContainerRuntime,
    _lock: PreparationLock,
}

impl ContainerGuard<'_> {
    /// Revocation is separate from acquisition: Stop must first commit its durable
    /// intent, and Delete must first pass all read-only preflights and cancel uploads.
    pub(crate) fn revoke(&self) -> anyhow::Result<()> {
        match std::fs::remove_file(self.runtime.ticket_path()) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    /// Podman stop may fail waiting for a killed exec's bookkeeping even after
    /// stopping the container. Its inspected state, not its exit code, is authoritative.
    pub(crate) async fn stop(&self) -> anyhow::Result<()> {
        let name = self.runtime.name()?;
        let _ = self.runtime.run(&["stop", "--time", "5", &name]).await;
        match self.runtime.state().await? {
            None => Ok(()),
            Some(state) if matches!(state.as_str(), "exited" | "stopped") => Ok(()),
            Some(state) => bail!("container stop could not be confirmed: podman reports {state}"),
        }
    }

    /// Idempotent removal converges after partial Delete, but only a successful
    /// absence observation can authorize discarding the session's final handle.
    pub(crate) async fn remove(&self) -> anyhow::Result<()> {
        let name = self.runtime.name()?;
        let _ = self.runtime.run(&["rm", "-f", "--ignore", &name]).await;
        match self.runtime.state().await? {
            None => Ok(()),
            Some(state) => {
                bail!("container removal could not be confirmed: podman reports {state}")
            }
        }
    }
    /// A tab may restart its existing keeper, never create a replacement for
    /// missing state. The tab's owned lifecycle task holds this fence through
    /// startup and window publication, so Stop/Delete cannot race that restart.
    pub(crate) async fn start_existing(
        &self,
        scopes: &crate::scope::ScopeManager,
    ) -> anyhow::Result<()> {
        use crate::bounded_command::{Ended, GroupChild, OutputCaps};
        match self.runtime.state().await? {
            Some(state) if state == "running" => return Ok(()),
            None => bail!("this session's container does not exist; replace or delete the session"),
            Some(_) => {}
        }
        let name = self.runtime.name()?;
        let unit = unit_name(&self.runtime.session_id)?;
        let prefix = scopes.launch_prefix(&unit).await.unwrap_or_default();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
        if !prefix.is_empty() {
            // Stop confirms the container itself, while systemd can collect
            // its old conmon scope a moment later. Wait for that exact unit;
            // killing it to reclaim the name can corrupt Podman's state.
            while scopes.exists(&unit).await? {
                anyhow::ensure!(
                    tokio::time::Instant::now() < deadline,
                    "the stopped container's helper scope is still closing; retry opening the tab"
                );
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
        let mut command = match prefix.first() {
            Some(program) => {
                let mut command = control_command(Path::new(program));
                command.args(&prefix[1..]).arg(&self.runtime.podman);
                command
            }
            None => control_command(&self.runtime.podman),
        };
        command.args(["start", &name]);
        let caps = OutputCaps {
            stdout: 65536,
            stderr: Some(65536),
        };
        let mut command = tokio::process::Command::from(command);
        let mut child = GroupChild::spawn(&mut command, caps)?;
        let ended = child.finish(deadline, caps).await;
        let state = self.runtime.state().await?;
        if state.as_deref() == Some("running") {
            return Ok(());
        }
        let detail = match ended {
            Ok(Ended::Exited(output)) => String::from_utf8_lossy(&output.stderr).into_owned(),
            Ok(Ended::TimedOut) => "start command timed out".into(),
            Ok(Ended::OutputOverCap) => "start command output exceeded its bound".into(),
            Err(error) => format!("start command failed: {error:#}"),
        };
        bail!("container start could not be confirmed (state {state:?}): {detail}")
    }
}

impl ContainerLaunch {
    /// Publish a generation-bound positive ticket before tmux can schedule its
    /// shim. A late helper from a prior create retry cannot spend a newer ticket.
    pub(crate) async fn publish(
        runtime: ContainerRuntime,
        checkout_root: PathBuf,
        helper: &Path,
        scopes: &crate::scope::ScopeManager,
    ) -> anyhow::Result<Self> {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(runtime.state_dir.join("container-launch"))?;
        let guard = runtime.acquire().await?;
        crate::files::write_durable_sync(
            &runtime.ticket_path(),
            &serde_json::to_vec(&CreationTicket {
                runtime: runtime.clone(),
                checkout_root,
            })?,
            &crate::files::RealFs,
        )?;
        drop(guard);
        let mut creator = scopes
            .launch_prefix(&unit_name(&runtime.session_id)?)
            .await
            .unwrap_or_default();
        creator.extend([
            helper.to_string_lossy().into_owned(),
            "internal".into(),
            "container-prepare".into(),
            runtime.ticket_path().to_string_lossy().into_owned(),
            runtime.generation.to_string(),
        ]);
        Ok(Self { runtime, creator })
    }

    /// Keep image-copy progress in the pane. The actual helper, not this shim,
    /// holds the lock and ignores terminal hangup while its Podman child completes.
    pub(crate) fn prepare(&self) -> anyhow::Result<()> {
        // A live container already owns this dedicated scope. Reusing its name
        // would make systemd-run refuse before Podman could report its no-op start.
        let running = control_command(&self.runtime.podman)
            .args(["container", "inspect", "--format", "{{.State.Status}}"])
            .arg(self.runtime.name()?)
            .output()?;
        if running.status.success() && running.stdout == b"running\n" {
            return Ok(());
        }
        let creator = self
            .creator
            .first()
            .context("container creator command is empty")?;
        let mut command = control_command(Path::new(creator));
        let status = command
            .args(&self.creator[1..])
            .stdin(Stdio::null())
            .status()?;
        anyhow::ensure!(
            status.success(),
            "container preparation failed; see podman's error above"
        );
        Ok(())
    }
}

/// Short-lived CLI helper run inside the container's own scope. Ignore normal
/// terminal hangup before spawning anything: the pane can disappear during import,
/// but the helper must retain its fence until its create child has finished.
pub fn prepare(ticket: &Path, generation: i64) -> anyhow::Result<()> {
    // SAFETY: this is a disposable synchronous CLI process, before its child spawn.
    unsafe {
        libc::signal(libc::SIGHUP, libc::SIG_IGN);
    }
    let lock = PreparationLock::open(&ticket.with_extension("lock"))?;
    lock.acquire()?;
    let ticket: CreationTicket = serde_json::from_slice(
        &std::fs::read(ticket).context("container creation was cancelled")?,
    )?;
    let runtime = ticket.runtime;
    anyhow::ensure!(
        runtime.generation == generation,
        "this container creation was superseded"
    );
    let name = runtime.name()?;
    // Only this initial helper may create. Later tab opens may start an existing
    // container but never turn missing state into a fresh session silently.
    let exists = control_command(&runtime.podman)
        .args(["container", "exists", &name])
        .status()?;
    if exists.success() {
        let status = control_command(&runtime.podman)
            .args(["start", &name])
            .status()?;
        anyhow::ensure!(
            status.success(),
            "podman could not start the existing container"
        );
    } else {
        anyhow::ensure!(
            exists.code() == Some(1),
            "could not inspect container existence"
        );
        let status = runtime.create_command(&ticket.checkout_root)?.status()?;
        anyhow::ensure!(status.success(), "podman could not create the container");
    }
    let output = control_command(&runtime.podman)
        .args([
            "container",
            "inspect",
            "--format",
            "{{.State.Status}}",
            &name,
        ])
        .output()?;
    anyhow::ensure!(
        output.status.success() && output.stdout == b"running\n",
        "container preparation did not confirm a running container"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// Every fixture owns its lock, state and fake Podman; child-only inputs never
    /// change the test process environment or depend on an installed container engine.
    fn fixture() -> (tempfile::TempDir, ContainerRuntime) {
        let root = tempfile::tempdir().unwrap();
        let runtime = ContainerRuntime {
            config: ContainerConfig {
                engine: farhelm_proto::ContainerEngine::Podman,
                image_file: "/images/fixture.oci".into(),
                api_proxy_port: 43000,
                github_token_file: Some(root.path().join("token").to_str().unwrap().into()),
            },
            podman: root.path().join("podman"),
            session_id: uuid::Uuid::new_v4().to_string(),
            generation: 0,
            cwd: root.path().join("checkout"),
            state_dir: root.path().into(),
        };
        std::fs::create_dir(root.path().join("container-launch")).unwrap();
        (root, runtime)
    }

    /// A file edit between preflight and exec must deliver the new token, and
    /// neither token value may appear in the resulting command arguments.
    #[farhelm_testtrace::test]
    fn exec_reads_token_afresh_without_putting_it_in_argv() {
        let (_root, runtime) = fixture();
        let path = runtime.config.github_token_file.as_ref().unwrap();
        std::fs::write(path, " old-synthetic-token\n").unwrap();
        assert_eq!(
            read_token(&runtime.config).unwrap().as_deref(),
            Some("old-synthetic-token")
        );
        std::fs::write(path, " new-synthetic-token\n").unwrap();
        let command = runtime.exec_command(&["bash".into()]).unwrap();
        let delivered = command
            .get_envs()
            .find(|(name, _)| *name == "GH_TOKEN")
            .unwrap()
            .1
            .unwrap();
        assert_eq!(delivered, "new-synthetic-token");
        let args: Vec<_> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert!(args.windows(2).any(|args| args == ["--env", "GH_TOKEN"]));
        assert!(!args.iter().any(|arg| arg.contains("synthetic-token")));
        assert!(
            !serde_json::to_string(&runtime)
                .unwrap()
                .contains("synthetic-token")
        );
        std::fs::remove_file(path).unwrap();
        assert!(runtime.exec_command(&["bash".into()]).is_err());
    }

    /// Bash words after an image only replace CMD; an image ENTRYPOINT would
    /// still intercept them. Explicit entrypoint selection keeps the container's
    /// lifetime independent of whatever program the user normally starts there.
    #[test]
    fn keeper_overrides_the_image_entrypoint_without_replacing_its_environment() {
        let (_root, runtime) = fixture();
        let command = runtime.create_command(&runtime.cwd).unwrap();
        let args: Vec<_> = command
            .get_args()
            .map(|arg| arg.to_str().unwrap())
            .collect();
        assert!(args.windows(2).any(|args| args == ["--entrypoint", "bash"]));
        assert!(args.contains(&"--userns=keep-id:uid=1000,gid=1000"));
        assert_eq!(
            &args[args.len() - 3..],
            [
                "oci-archive:/images/fixture.oci",
                "-c",
                "while :; do sleep 3600; done"
            ]
        );
        assert!(
            !args.contains(&"--env"),
            "the keeper uses the image's environment"
        );
    }

    /// Timing out on a creator's lock must not revoke its permit or leave a
    /// blocking worker that can acquire it after the caller already failed.
    #[tokio::test(start_paused = true)]
    async fn lock_timeout_leaves_the_ticket_and_lock_identity_untouched() {
        let (_root, runtime) = fixture();
        std::fs::write(runtime.ticket_path(), b"permit").unwrap();
        let owner = PreparationLock::open(&runtime.lock_path()).unwrap();
        owner.acquire().unwrap();
        let error = runtime
            .acquire()
            .await
            .err()
            .expect("a held lock must time out");
        assert!(error.to_string().contains("retry"));
        assert_eq!(std::fs::read(runtime.ticket_path()).unwrap(), b"permit");
        drop(owner);
        let guard = runtime.acquire().await.unwrap();
        guard.revoke().unwrap();
        assert!(
            runtime.lock_path().exists(),
            "unlinking the inode would split waiting contenders"
        );
    }

    /// The child helper test runs only as a disposable subprocess so SIGHUP policy
    /// changes cannot affect the concurrent parent test runner.
    #[test]
    #[ignore = "subprocess entry point for the creator fence tests"]
    fn creator_child() {
        let ticket = std::env::var_os("CONTAINER_TEST_TICKET").expect("child input");
        if let Err(error) = prepare(Path::new(&ticket), 0) {
            eprintln!("creator refused: {error:#}");
            std::process::exit(1);
        }
    }

    /// Start the actual helper implementation in a child with no inherited lock
    /// descriptor. Readiness is a fake-Podman write performed inside its run call.
    fn child(runtime: &ContainerRuntime) -> CreatorChild {
        use std::os::unix::process::CommandExt;
        let child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "container::tests::creator_child",
                "--nocapture",
            ])
            .env("CONTAINER_TEST_TICKET", runtime.ticket_path())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .unwrap();
        CreatorChild {
            group: Some(child.id()),
            child,
        }
    }

    /// Panic cleanup cannot strand a fake Podman blocked on its FIFO. The helper
    /// and child have an owned process group, separate from the test runner.
    /// Exit observations leave the leader unreaped until after group signalling,
    /// so a successful wait cannot free its number for an unrelated process group.
    struct CreatorChild {
        child: std::process::Child,
        group: Option<u32>,
    }
    impl Drop for CreatorChild {
        fn drop(&mut self) {
            // SAFETY: the group is minted by the owned child below, and cleanup
            // remains separate from the test runner, including on a failed assertion.
            if let Some(group) = self.group.take() {
                unsafe {
                    libc::kill(-(group as i32), libc::SIGKILL);
                }
            }
            let _ = self.child.wait();
        }
    }
    impl CreatorChild {
        fn id(&self) -> u32 {
            self.child.id()
        }

        /// Observe success without reaping the leader that reserves the group id.
        /// An unexpected external reap disarms signalling before reporting failure.
        fn try_status(&mut self) -> std::io::Result<Option<bool>> {
            // SAFETY: waitid fills an initialized buffer for our owned child;
            // WNOWAIT preserves identity until Drop signals and reaps the group.
            let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
            let result = unsafe {
                libc::waitid(
                    libc::P_PID,
                    self.id() as libc::id_t,
                    &mut info,
                    libc::WEXITED | libc::WNOWAIT | libc::WNOHANG,
                )
            };
            if result == 0 {
                return Ok((info.si_signo != 0).then(|| {
                    info.si_code == libc::CLD_EXITED && unsafe { info.si_status() } == 0
                }));
            }
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::EINTR) {
                return Ok(None);
            }
            if error.raw_os_error() == Some(libc::ECHILD) {
                self.group = None;
            }
            Err(error)
        }
    }

    /// Bound helper completion independently of the outer runner's timeout.
    async fn wait_child(child: &mut CreatorChild) -> bool {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = child.try_status().unwrap() {
                return status;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "creator child did not exit"
            );
            // sleep-ok: bounded polling of the owned child's exit-status oracle.
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// Block the fake create on a FIFO, proving the helper actually keeps its
    /// flock through Podman's child lifetime rather than merely around spawn.
    fn install_fake(runtime: &ContainerRuntime) -> PathBuf {
        let root = &runtime.state_dir;
        let fifo = root.join("gate");
        let path = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
        // SAFETY: CString supplies a terminated path; this fixture owns its directory.
        assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
        let quote = |name: &str| shell_words::quote(root.join(name).to_str().unwrap()).into_owned();
        let script = format!(
            "#!/bin/sh\ncase \"$1 $2\" in\n'container exists') test -f {};;\n'container inspect') cat {};;\n'run -d') touch {}; read -r ready < {}; echo running > {};;\nesac\n",
            quote("state"),
            quote("state"),
            quote("entered"),
            quote("gate"),
            quote("state")
        );
        std::fs::write(&runtime.podman, script).unwrap();
        std::fs::set_permissions(&runtime.podman, std::fs::Permissions::from_mode(0o700)).unwrap();
        write_ticket(runtime);
        fifo
    }

    /// Match the creator's credential-free ticket without inventing a second
    /// configuration path in tests. Fake roots need no real checkout registry.
    fn write_ticket(runtime: &ContainerRuntime) {
        std::fs::write(
            runtime.ticket_path(),
            serde_json::to_vec(&CreationTicket {
                runtime: runtime.clone(),
                checkout_root: runtime.cwd.clone(),
            })
            .unwrap(),
        )
        .unwrap();
    }

    /// A normal terminal hangup must not release the helper's fence while Podman
    /// is still creating outside agent containment. Teardown waits, then revokes.
    #[farhelm_testtrace::test]
    async fn creator_holds_the_fence_through_hangup_and_child_completion() {
        use std::io::Write;
        let (_root, runtime) = fixture();
        let fifo = install_fake(&runtime);
        let mut child = child(&runtime);
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        while !runtime.state_dir.join("entered").exists() {
            assert!(
                child.try_status().unwrap().is_none(),
                "creator died before Podman run"
            );
            assert!(
                tokio::time::Instant::now() < deadline,
                "creator did not reach Podman run"
            );
            // sleep-ok: bounded polling of the fake child's post-spawn readiness oracle.
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let contender = PreparationLock::open(&runtime.lock_path()).unwrap();
        assert!(!contender.try_acquire().unwrap());
        // SAFETY: this still-live child is owned and has just proved its identity.
        assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGHUP) }, 0);
        std::fs::OpenOptions::new()
            .write(true)
            .open(fifo)
            .unwrap()
            .write_all(b"continue\n")
            .unwrap();
        assert!(wait_child(&mut child).await);
        let guard = runtime.acquire().await.unwrap();
        assert_eq!(
            std::fs::read_to_string(runtime.state_dir.join("state"))
                .unwrap()
                .trim(),
            "running"
        );
        guard.revoke().unwrap();
    }

    /// A creator queued before Stop/Delete must fail after revocation rather than
    /// recreating a container after teardown observed absence. The permanent lock
    /// inode remains available to that already-started contender.
    #[farhelm_testtrace::test]
    async fn late_creator_cannot_spend_a_revoked_ticket() {
        let (_root, runtime) = fixture();
        install_fake(&runtime);
        let guard = runtime.acquire().await.unwrap();
        let mut child = child(&runtime);
        guard.revoke().unwrap();
        drop(guard);
        assert!(!wait_child(&mut child).await);
        assert!(!runtime.state_dir.join("entered").exists());
        assert!(!runtime.state_dir.join("state").exists());
    }

    /// A delayed helper from a prior attempt cannot use a retry's positive ticket.
    /// Presence alone would authorize old configuration after the new generation won.
    #[farhelm_testtrace::test]
    async fn superseded_creator_cannot_spend_a_newer_generations_ticket() {
        let (_root, mut runtime) = fixture();
        install_fake(&runtime);
        runtime.generation = 1;
        write_ticket(&runtime);
        let mut child = child(&runtime);
        assert!(!wait_child(&mut child).await);
        assert!(!runtime.state_dir.join("entered").exists());
        assert!(!runtime.state_dir.join("state").exists());
    }
}
