//! Removal of containers created by the retired supervisor-outside model.
//!
//! A creator already running an older binary can still hold or wait for its
//! original flock. Delete takes that same permanent inode and revokes its ticket
//! before removal. Current binaries have no creation or exec implementation.

use crate::launch::PreparationLock;
use anyhow::{Context, bail};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// The explanation shared by every refused use of a retired session or choice.
pub(crate) const RETIRED_MESSAGE: &str = "this experimental containerized session is retired and supports Delete only; create a container session from a recipe";

/// Stored launch intent identifies the retired model even when its image or
/// account settings no longer exist. Reading it grants only removal authority.
pub(crate) fn is_retired(launch: &farhelm_proto::SessionLaunch) -> bool {
    launch
        .agent_selection()
        .is_some_and(|selection| selection.container.is_some())
}

/// Minimal removal coordinates derived from a validated session UUID.
/// Podman is resolved only when deletion actually runs. Its absence is a valid
/// cleanup result, while errors from an installed executable remain unconfirmed.
pub(crate) struct ContainerRuntime {
    session_id: String,
    state_dir: PathBuf,
}

/// Reuse the legacy unit name so Delete can collect its old conmon scope too.
pub(crate) fn unit_name(id: &str) -> anyhow::Result<String> {
    let id = uuid::Uuid::parse_str(id).context("invalid container session identity")?;
    Ok(format!("farhelm-{id}-container.scope"))
}

/// Podman control inherits the user's storage configuration without the
/// supervisor's session credentials or any inherited tmux connection coordinates.
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

/// Bound storage waits and diagnostics. An interrupted command or output over
/// the cap cannot prove that the last owned container disappeared.
async fn run_control(podman: &Path, args: &[&str]) -> anyhow::Result<std::process::Output> {
    use crate::bounded_command::{Ended, GroupChild, OutputCaps};
    let mut command = tokio::process::Command::from(control_command(podman));
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

impl ContainerRuntime {
    /// Recognize a legacy row without touching Podman, the image, token file or
    /// checkout. Those inputs can disappear before a user deletes the session.
    pub(crate) fn for_session(
        state_dir: &Path,
        id: &str,
        launch: &farhelm_proto::SessionLaunch,
    ) -> anyhow::Result<Option<Self>> {
        if !is_retired(launch) {
            return Ok(None);
        }
        let id = uuid::Uuid::parse_str(id).context("invalid container session identity")?;
        Ok(Some(Self {
            session_id: id.to_string(),
            state_dir: state_dir.into(),
        }))
    }

    /// Legacy container names were always derived from the whole session UUID.
    fn name(&self) -> String {
        format!("farhelm-{}", self.session_id)
    }

    /// The inode survives Delete permanently: an old helper may already have
    /// opened it while waiting, and there is no upgrade boundary that proves
    /// every such process has exited. Unlinking would create two independent
    /// creation fences for the same session UUID.
    fn lock_path(&self) -> PathBuf {
        self.state_dir
            .join("container-launch")
            .join(format!("{}.lock", self.session_id))
    }

    /// Locate the only positive creation permit an older helper can consume.
    fn ticket_path(&self) -> PathBuf {
        self.state_dir
            .join("container-launch")
            .join(format!("{}.json", self.session_id))
    }

    /// Retain a failed initialization marker until absence is confirmed.
    fn initialization_path(&self) -> PathBuf {
        self.state_dir
            .join("container-launch")
            .join(format!("{}.initializing", self.session_id))
    }

    /// Wait for an already-running old creator without leaving a blocking
    /// waiter behind if this cleanup attempt reaches its deadline.
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
                "container creation is still running; retry Delete after it finishes"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

/// The old creator fence stays held through revocation and absence confirmation.
pub(crate) struct ContainerGuard<'a> {
    runtime: &'a ContainerRuntime,
    _lock: PreparationLock,
}

impl ContainerGuard<'_> {
    /// Call only after Delete's ordinary read-only preflights have passed.
    pub(crate) fn revoke(&self) -> anyhow::Result<()> {
        match std::fs::remove_file(self.runtime.ticket_path()) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error).context("could not revoke retired container creation"),
        }
    }

    /// An absent executable leaves no supported local container to remove.
    /// Installed Podman's exit status alone never authorizes forgetting the row.
    pub(crate) async fn remove(&self) -> anyhow::Result<()> {
        let podman = crate::scope::resolve_program("podman");
        self.remove_with_podman(podman.as_deref()).await
    }

    /// Inject resolution for focused tests without modifying process environment.
    async fn remove_with_podman(&self, podman: Option<&Path>) -> anyhow::Result<()> {
        if let Some(podman) = podman {
            let name = self.runtime.name();
            let _ = run_control(podman, &["rm", "-f", "--ignore", &name]).await;
            let absent = run_control(podman, &["container", "exists", &name]).await?;
            anyhow::ensure!(
                absent.status.code() == Some(1),
                "retired container removal could not be confirmed: {}",
                String::from_utf8_lossy(&absent.stderr)
            );
        }
        match std::fs::remove_file(self.runtime.initialization_path()) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error).context("could not clear the container initialization fence"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// Each fixture owns its old ticket, marker and fake engine. No process
    /// environment or installed Podman is involved in the cleanup proof.
    fn fixture() -> (tempfile::TempDir, ContainerRuntime) {
        let root = tempfile::tempdir().unwrap();
        let runtime = ContainerRuntime {
            session_id: uuid::Uuid::new_v4().to_string(),
            state_dir: root.path().into(),
        };
        std::fs::create_dir(root.path().join("container-launch")).unwrap();
        std::fs::write(runtime.ticket_path(), b"legacy ticket").unwrap();
        std::fs::write(runtime.initialization_path(), b"pending").unwrap();
        assert_eq!(
            std::fs::read(runtime.ticket_path()).unwrap(),
            b"legacy ticket"
        );
        assert_eq!(
            std::fs::read(runtime.initialization_path()).unwrap(),
            b"pending"
        );
        (root, runtime)
    }

    /// Delete still revokes an old ticket when Podman has been uninstalled.
    #[farhelm_testtrace::test]
    async fn missing_podman_allows_deletion_and_revokes_creation() {
        let (_root, runtime) = fixture();
        assert!(runtime.ticket_path().exists() && runtime.initialization_path().exists());
        let guard = runtime.acquire().await.unwrap();
        guard.revoke().unwrap();
        guard.remove_with_podman(None).await.unwrap();
        assert!(!runtime.ticket_path().exists());
        assert!(!runtime.initialization_path().exists());
        assert!(
            runtime.lock_path().exists(),
            "old waiters retain the same lock inode"
        );
    }

    /// A helper that opened the old inode before Delete must see the revoked
    /// ticket after acquiring it. Keeping the inode also serializes a later
    /// cleanup with that same waiter, rather than a new independent lock.
    #[farhelm_testtrace::test]
    async fn an_old_creation_waiter_keeps_the_revoked_permanent_fence() {
        use std::os::unix::fs::MetadataExt;
        let (_root, runtime) = fixture();
        let guard = runtime.acquire().await.unwrap();
        let inode = std::fs::metadata(runtime.lock_path()).unwrap().ino();
        let old_waiter = PreparationLock::open(&runtime.lock_path()).unwrap();
        assert!(
            !old_waiter.try_acquire().unwrap(),
            "premise: Delete owns the old creator lock"
        );
        guard.revoke().unwrap();
        guard.remove_with_podman(None).await.unwrap();
        drop(guard);

        assert!(old_waiter.try_acquire().unwrap());
        assert!(
            !runtime.ticket_path().exists(),
            "the old creator has no positive permit"
        );
        assert_eq!(std::fs::metadata(runtime.lock_path()).unwrap().ino(), inode);
        let later_cleanup = PreparationLock::open(&runtime.lock_path()).unwrap();
        assert!(
            !later_cleanup.try_acquire().unwrap(),
            "a later Delete uses the old waiter's inode"
        );
        drop(old_waiter);
        assert!(later_cleanup.try_acquire().unwrap());
    }

    /// A failed removal is acceptable only if the engine then confirms absence.
    /// A live object or storage error retains the initialization fence for retry.
    #[farhelm_testtrace::test]
    async fn only_confirmed_absence_releases_the_cleanup_fence() {
        for code in [0, 1, 125] {
            let (root, runtime) = fixture();
            let engine = root.path().join("podman");
            std::fs::write(
                &engine,
                format!("#!/bin/sh\nif [ \"$1\" = rm ]; then exit 125; fi\nexit {code}\n"),
            )
            .unwrap();
            std::fs::set_permissions(&engine, std::fs::Permissions::from_mode(0o700)).unwrap();
            let guard = runtime.acquire().await.unwrap();
            guard.revoke().unwrap();
            let result = guard.remove_with_podman(Some(&engine)).await;
            assert_eq!(result.is_ok(), code == 1);
            assert_eq!(runtime.initialization_path().exists(), code != 1);
            assert!(
                !runtime.ticket_path().exists(),
                "a failed removal cannot restore creation authority"
            );
        }
    }
}
