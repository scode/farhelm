//! Give Podman first chance to stop containers whose main monitor belongs to
//! the reaper's scope. A container workload may live outside that scope.

use crate::container::run_control_until;
use crate::procs::{ProcessIdentity, read_process_argv};
use crate::scope::{ScopeManager, ScopeMembers};
use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::os::unix::{ffi::OsStrExt, fs::MetadataExt};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinSet;
use tokio::time::{Instant, timeout_at};
use tracing::{debug, warn};

/// Discovery and all commands share this deadline, even across several scopes.
const BUDGET: Duration = Duration::from_secs(15);
/// Allow time for Podman's five-second grace and leave room for a state read.
const STOP_BUDGET: Duration = Duration::from_secs(10);

/// A full ID and the storage options that locate it. The monitor's exit
/// executable and all other options stay outside the generated command.
struct Container {
    id: String,
    name: String,
    storage: Vec<OsString>,
    auto_remove: bool,
}

impl Container {
    /// Recover the container from a main conmon's own cleanup invocation.
    /// An exec monitor belongs to an existing container and grants no
    /// authority to stop it. The cleanup's target must match the monitor's.
    fn from_argv(argv: &[Vec<u8>]) -> Option<Self> {
        let (program, argv) = argv.split_first()?;
        if Path::new(OsStr::from_bytes(program)).file_name()? != "conmon" {
            return None;
        }
        let mut id = None;
        let mut name = None;
        let mut exit = Vec::new();
        let mut argv = argv.iter().map(Vec::as_slice);
        while let Some(arg) = argv.next() {
            let (flag, inline) = option(arg);
            match flag {
                b"-e" | b"--exec" => return None,
                b"-c" | b"--cid" => {
                    if id.replace(inline.or_else(|| argv.next())?).is_some() {
                        return None;
                    }
                }
                b"-n" | b"--name" => name = inline.or_else(|| argv.next()),
                b"--exit-command-arg" => exit.push(inline.or_else(|| argv.next())?),
                _ => {}
            }
        }
        let id = id?;
        if id.len() != 64 || !id.iter().all(u8::is_ascii_hexdigit) {
            return None;
        }
        let (target, invocation) = exit.split_last()?;
        if *target != id {
            return None;
        }
        let cleanup = invocation
            .windows(2)
            .rposition(|args| args == [b"container", b"cleanup" as &[u8]])?;
        let cleanup_options = &invocation[cleanup + 2..];
        if !cleanup_options
            .iter()
            .all(|arg| matches!(*arg, b"--rm" | b"--rmi" | b"--stopped-only"))
        {
            return None;
        }
        let id = std::str::from_utf8(id).ok()?.to_owned();
        // Podman's name grammar gives a short single-line diagnostic.
        let name = name
            .filter(|name| {
                !name.is_empty()
                    && name.len() <= 128
                    && name.iter().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-')
                    })
            })
            .and_then(|name| std::str::from_utf8(name).ok())
            .unwrap_or(&id)
            .to_owned();
        Some(Self {
            id,
            name,
            storage: storage_options(&invocation[..cleanup])?,
            auto_remove: cleanup_options.contains(&(b"--rm" as &[u8])),
        })
    }

    /// Assemble arguments for the supervisor's local Podman. The full ID is
    /// used for lookup even if the container has been renamed.
    async fn command(
        &self,
        podman: &Path,
        command: &[&str],
        deadline: Instant,
    ) -> anyhow::Result<std::process::Output> {
        // A desktop supervisor can inherit Podman's connection settings.
        // A local conmon only supplies evidence for the local engine.
        let mut args = vec![OsString::from("--remote=false")];
        args.extend(self.storage.iter().cloned());
        args.extend(command.iter().copied().map(OsString::from));
        args.push(self.id.clone().into());
        run_control_until(podman, &args, deadline).await
    }
}

fn option(arg: &[u8]) -> (&[u8], Option<&[u8]>) {
    let mut parts = arg.splitn(2, |byte| *byte == b'=');
    (parts.next().unwrap_or(arg), parts.next())
}

/// Retain only local storage selectors. Keep each recognized value attached
/// to its flag so a value beginning with a dash cannot become a Podman option.
fn storage_options(args: &[&[u8]]) -> Option<Vec<OsString>> {
    let mut storage = Vec::new();
    let mut args = args.iter().copied();
    while let Some(arg) = args.next() {
        let (flag, inline) = option(arg);
        let value = match flag {
            b"--root" | b"--runroot" | b"--tmpdir" | b"--cgroup-manager" | b"--storage-driver"
            | b"--db-backend" | b"--volumepath" | b"--imagestore" => {
                inline.or_else(|| args.next())?
            }
            b"--storage-opt" => {
                let value = inline.or_else(|| args.next())?;
                // Driver options can select executables. Keep only the
                // storage settings needed by ordinary overlay and vfs stores.
                if !matches!(
                    option(value).0,
                    b"overlay.mountopt"
                        | b"overlay.ignore_chown_errors"
                        | b"vfs.ignore_chown_errors"
                ) {
                    continue;
                }
                value
            }
            b"--transient-store" => inline.unwrap_or(b"true"),
            // A token that belongs to one of Podman's unrelated options
            // cannot also supply a storage selector.
            b"--log-level"
            | b"--network-config-dir"
            | b"--network-backend"
            | b"--hooks-dir"
            | b"--module"
            | b"--runtime"
            | b"--events-backend" => {
                inline.or_else(|| args.next())?;
                continue;
            }
            _ => continue,
        };
        let mut pair = OsString::from(OsStr::from_bytes(flag));
        pair.push("=");
        pair.push(OsStr::from_bytes(value));
        storage.push(pair);
    }
    Some(storage)
}

/// Revalidate attribution after reading argv. This also excludes a PID
/// recycled after cgroup.procs was read, and any monitor that has moved.
fn still_in_scope(identity: ProcessIdentity, group: &str) -> bool {
    let process = format!("/proc/{}", identity.pid);
    std::fs::metadata(&process)
        .is_ok_and(|metadata| metadata.uid() == rustix::process::geteuid().as_raw())
        && std::fs::read_to_string(format!("{process}/cgroup")).is_ok_and(|memberships| {
            memberships
                .lines()
                .any(|line| line.strip_prefix("0::") == Some(group))
        })
        && ProcessIdentity::read(identity.pid) == Some(identity)
}

/// Read argv and recheck membership off the async worker. A cmdline read
/// can wait for the target's address space, so an expired deadline must
/// let the reaper proceed even if the kernel has not finished a read.
fn discover(members: ScopeMembers, deadline: Instant) -> Vec<Container> {
    let mut containers = Vec::new();
    for pid in members.pids {
        if Instant::now() >= deadline {
            break;
        }
        let Some(identity) = ProcessIdentity::read(pid) else {
            continue;
        };
        let Some(container) = read_process_argv(pid).and_then(|argv| Container::from_argv(&argv))
        else {
            continue;
        };
        if still_in_scope(identity, &members.group) {
            containers.push(container);
        }
    }
    containers
}

/// Confirm the stop and honor auto-removal before signalling the scope.
/// Only a failed stop needs a state read.
async fn stop(
    podman: &Path,
    container: &Container,
    stop_deadline: Instant,
    deadline: Instant,
) -> bool {
    let stopped = container
        .command(podman, &["stop", "--time", "5", "--"], stop_deadline)
        .await
        .is_ok_and(|result| result.status.success());
    if !stopped {
        let inspected = container
            .command(
                podman,
                &[
                    "container",
                    "inspect",
                    "--format",
                    "{{.State.Status}}",
                    "--",
                ],
                deadline,
            )
            .await;
        if let Ok(out) = inspected
            && out.status.success()
        {
            if !matches!(out.stdout.trim_ascii(), b"exited" | b"stopped") {
                return false;
            }
        } else {
            return container
                .command(podman, &["container", "exists", "--"], deadline)
                .await
                .is_ok_and(|result| result.status.code() == Some(1));
        }
    }

    // Podman 4.9 can return success before conmon finishes removing a
    // --rm container. Finish that removal while the scope is still live.
    // Ignore absence if conmon won the race. A non-forced rm refuses a
    // container another client has already started again.
    !container.auto_remove
        || container
            .command(podman, &["rm", "--ignore", "--"], deadline)
            .await
            .is_ok_and(|result| result.status.success())
}

/// Start all discovered stops before signalling the scopes. Each timeout
/// kills its own command group and leaves the existing reaper as backstop.
pub(super) async fn stop_before_signals(
    scopes: &ScopeManager,
    units: &[&String],
    session_id: &str,
    caller_deadline: Option<Instant>,
) -> Vec<String> {
    let now = Instant::now();
    let deadline = caller_deadline.map_or(now + BUDGET, |limit| limit.min(now + BUDGET));
    if units.is_empty() || deadline <= now {
        return Vec::new();
    }
    let Some(podman) = scopes.podman_program().map(Arc::new) else {
        return Vec::new();
    };
    let mut dispatched = HashSet::new();
    let mut stops = JoinSet::new();
    for unit in units {
        let Ok(members) = timeout_at(deadline, scopes.process_members(unit)).await else {
            debug!(session = %session_id, unit, "Podman scope discovery exceeded its deadline");
            break;
        };
        let Some(members) = members else {
            continue;
        };
        if members.truncated {
            warn!(session = %session_id, unit, "Podman cleanup saw a truncated scope membership");
        }
        let Ok(Ok(containers)) = timeout_at(
            deadline,
            tokio::task::spawn_blocking(move || discover(members, deadline)),
        )
        .await
        else {
            warn!(session = %session_id, unit, "Podman monitor discovery did not complete");
            break;
        };
        for container in containers {
            if !dispatched.insert((container.id.clone(), container.storage.clone())) {
                continue;
            }
            let podman = podman.clone();
            stops.spawn(async move {
                let now = Instant::now();
                let remaining = deadline.saturating_duration_since(now);
                // Keep time for the state read even under startup's shorter budget.
                let stop_deadline = now + STOP_BUDGET.min(remaining * 2 / 3);
                let stopped =
                    timeout_at(deadline, stop(&podman, &container, stop_deadline, deadline))
                        .await
                        .unwrap_or(false);
                (container, stopped)
            });
        }
    }
    let mut failures = Vec::new();
    while let Some(result) = stops.join_next().await {
        match result {
            Ok((container, false)) => {
                warn!(
                    session = %session_id, container = %container.name, id = %container.id,
                    "Podman could not confirm the container stopped; continuing process cleanup"
                );
                if container.name == container.id {
                    failures.push(container.id);
                } else {
                    failures.push(format!("{} ({})", container.name, container.id));
                }
            }
            Err(error) => warn!(session = %session_id, %error, "Podman stop task did not complete"),
            Ok((_, true)) => {}
        }
    }
    failures.sort_unstable();
    failures
}

#[cfg(test)]
mod tests;
