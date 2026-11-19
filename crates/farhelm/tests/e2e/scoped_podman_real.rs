//! Opt-in integration coverage for process cleanup with real local Podman.
//! All containers use an owned store and an offline root filesystem.

use crate::harness::*;
use crate::podman_environments::engine;
use farhelm_helm::probe_local_podman_with_test_storage;
use farhelm_proto::text::shell_quote;
use farhelm_supervisor::scope::unit_name;
use std::path::Path;
use std::process::{Command, Output, Stdio};

/// Container identities are known before Podman runs, so the drop guard can
/// clean up even if a launch or test command times out before returning an ID.
struct Store {
    /// Kept on an unconfirmed removal so cleanup can still find the engine
    /// records for the three owned names.
    dir: Option<tempfile::TempDir>,
    names: [String; 3],
    cleanup_needed: bool,
}

impl Store {
    /// Build the smallest root filesystem needed by the test. Using
    /// a root directory keeps image pulls and caches out of the fixture.
    fn new(busybox: &Path) -> Self {
        let dir = tempfile::Builder::new()
            .prefix("fh-podman-")
            .tempdir_in("/tmp")
            .unwrap();
        let suffix = dir.path().file_name().unwrap().to_str().unwrap();
        let names = ["auto", "plain", "outside"].map(|label| format!("farhelm-{suffix}-{label}"));
        let bin = dir.path().join("rootfs/bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::copy(busybox, bin.join("busybox")).unwrap();
        Self {
            dir: Some(dir),
            names,
            cleanup_needed: false,
        }
    }

    fn path(&self) -> &Path {
        self.dir
            .as_ref()
            .expect("the store owns a directory")
            .path()
    }

    /// Use one private store for launches, inspection and teardown. The
    /// short test directory also keeps runroot inside Podman's socket limit.
    fn args(&self, command: &[&str]) -> Vec<String> {
        let p = self.path().display();
        [
            "--remote=false".into(),
            format!("--root={p}/store"),
            format!("--runroot={p}/run"),
            format!("--tmpdir={p}/tmp"),
            "--storage-driver=vfs".into(),
            "--cgroup-manager=systemd".into(),
        ]
        .into_iter()
        .chain(command.iter().map(|arg| (*arg).to_owned()))
        .collect()
    }

    /// Resolve all arguments before the login shell sees them. This
    /// command goes through the same launch path as an ordinary agent.
    fn shell_command(&self, command: &[&str]) -> String {
        std::iter::once("podman".to_owned())
            .chain(self.args(command))
            .map(|arg| shell_quote(&arg))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Preserve exit status for the existence probe, where code 1 alone
    /// proves that Podman cannot find the container in the private store.
    async fn output(&self, command: &[&str]) -> Output {
        let args = self.args(command);
        engine(&args.iter().map(String::as_str).collect::<Vec<_>>())
            .await
            .unwrap_or_else(|error| panic!("private Podman {command:?}: {error:#}"))
    }

    async fn read(&self, command: &[&str]) -> String {
        let out = self.output(command).await;
        assert!(
            out.status.success(),
            "private Podman {command:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap().trim().to_owned()
    }

    /// Keep monitors in the caller's scope in either systemd environment.
    /// The workload still gets its own cgroup, and its expiry backs up the
    /// teardown guard if the test is killed.
    fn launch<'a>(name: &'a str, rootfs: &'a str, auto_remove: bool) -> Vec<&'a str> {
        let mut args = vec![
            "run",
            "-d",
            "--network",
            "none",
            "--cgroups=no-conmon",
            "--read-only",
            "--name",
            name,
        ];
        if auto_remove {
            args.push("--rm");
        }
        args.extend(["--rootfs", rootfs, "/bin/busybox", "sleep", "120"]);
        args
    }

    /// Check that the container really is running and return its live
    /// main monitor. Container names are insufficient evidence of scope.
    async fn running_monitor(&self, name: &str) -> u32 {
        let report = self
            .read(&[
                "container",
                "inspect",
                "--format",
                "{{.State.Status}} {{.State.ConmonPid}}",
                name,
            ])
            .await;
        let pid = report
            .strip_prefix("running ")
            .and_then(|pid| pid.parse::<u32>().ok())
            .expect("a live container reports its main monitor");
        assert!(
            !process_is_gone(pid),
            "container {name} monitor {pid} must be live"
        );
        pid
    }

    /// The names are owned before launch, including when a lost reply keeps
    /// the test from learning a container's ID.
    fn removal(&self) -> [&str; 7] {
        [
            "rm",
            "--force",
            "--ignore",
            "--",
            &self.names[0],
            &self.names[1],
            &self.names[2],
        ]
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        if !self.cleanup_needed {
            return;
        }
        let args = self.args(&self.removal());
        // Cleanup runs even on unwind, under a wall-clock bound, and can
        // address only these names in the private store.
        let status = Command::new("timeout")
            .args(["--kill-after=2s", "15s", "podman"])
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if !status.is_ok_and(|status| status.success()) {
            let kept = self.dir.take().expect("owned Podman storage").keep();
            eprintln!(
                "could not remove all owned containers. Kept Podman storage at {}",
                kept.display()
            );
        }
    }
}

/// Read the same cgroup v2 membership evidence the supervisor uses, from
/// an owned main monitor that is still running.
fn cgroup(pid: u32) -> String {
    std::fs::read_to_string(format!("/proc/{pid}/cgroup"))
        .unwrap()
        .lines()
        .find_map(|line| line.strip_prefix("0::"))
        .expect("cgroup v2 membership")
        .to_owned()
}

/// A prompt from the exec subprocess is independent of the external
/// container's status. Also check its monitor really joined this scope,
/// since an exec monitor outside it would be a vacuous exclusion test.
fn exec_monitor(group: &str, container: &str) -> u32 {
    let pids = std::fs::read_to_string(format!("/sys/fs/cgroup{group}/cgroup.procs")).unwrap();
    pids.lines()
        .filter_map(|pid| pid.parse::<u32>().ok())
        .find(|pid| {
            let Ok(bytes) = std::fs::read(format!("/proc/{pid}/cmdline")) else {
                return false;
            };
            let mut args = bytes.split(|byte| *byte == 0);
            if !args
                .next()
                .is_some_and(|name| name.rsplit(|byte| *byte == b'/').next() == Some(b"conmon"))
            {
                return false;
            }
            let args: Vec<_> = args.collect();
            args.iter().any(|arg| matches!(*arg, b"--exec" | b"-e"))
                && args.contains(&container.as_bytes())
                && !process_is_gone(*pid)
        })
        .expect("the external container's exec monitor belongs to the agent scope")
}

/// Real Podman state is the verdict. A surviving external container proves
/// the reaper did not mistake its exec monitor for the monitor of a container
/// the agent started. The agent's non-default store makes a default-store
/// stop unable to satisfy either cleanup assertion.
#[farhelm_testtrace::test]
#[ignore = "requires local rootless Podman, systemd user scopes and static BusyBox"]
async fn real_scoped_stop_respects_storage_auto_remove_and_exec() {
    let busybox = Path::new("/usr/bin/busybox");
    let static_busybox = Command::new("file")
        .arg("-Lb")
        .arg(busybox)
        .env("LC_ALL", "C")
        .output()
        .is_ok_and(|out| {
            let description = String::from_utf8_lossy(&out.stdout);
            out.status.success()
                && (description.contains("statically linked")
                    || description.contains("static-pie linked"))
        });
    if !static_busybox {
        eprintln!(
            "SKIPPED real scoped Podman cleanup: install a static BusyBox at /usr/bin/busybox"
        );
        return;
    }
    // Drop the harness and its terminal hosts before the storage guard so
    // teardown cannot race a launch still running in the agent terminal.
    let mut store = Store::new(busybox);
    let version = match probe_local_podman_with_test_storage(store.path(), &store.args(&[])).await {
        Ok(version) => version,
        Err(reason) => {
            eprintln!("SKIPPED real scoped Podman cleanup: {reason}");
            return;
        }
    };
    eprintln!("Real scoped cleanup uses Podman {version} in the fixture's private store");
    let Some((h, _scopes)) =
        scope_gated_harness("real_scoped_stop_respects_storage_auto_remove_and_exec").await
    else {
        return;
    };
    store.cleanup_needed = true;
    let [auto, plain, outside] = &store.names;
    let rootfs = store.path().join("rootfs");
    let rootfs = rootfs.to_str().unwrap();
    let external_id = store.read(&Store::launch(outside, rootfs, false)).await;
    let external_pid = store.running_monitor(outside).await;

    let script = [
        store.shell_command(&Store::launch(auto, rootfs, true)),
        store.shell_command(&Store::launch(plain, rootfs, false)),
        format!(
            "exec {}",
            store.shell_command(&[
                "exec",
                "-t",
                outside,
                "/bin/busybox",
                "sh",
                "-c",
                "printf 'PODMAN-EXEC-READY\\n'; exec /bin/busybox sleep 120",
            ])
        ),
    ]
    .join("\n");
    let session = h
        .client
        .create_session(
            store.path().to_str().unwrap(),
            &format!("sh -ec {}", shell_quote(&script)),
            None,
            80,
            24,
        )
        .await
        .expect("start the agent with two containers and an attached exec");
    let (_channel, mut seen, mut rx) = h.client.attach_live(&session.id, 80, 24).await.unwrap();
    wait_for(&mut rx, &mut seen, "PODMAN-EXEC-READY", 30).await;
    let auto_pid = store.running_monitor(auto).await;
    let plain_pid = store.running_monitor(plain).await;
    let unit = unit_name(&session.id, 0).unwrap();
    let group = cgroup(auto_pid);
    assert!(
        group.ends_with(&format!("/{unit}")),
        "auto container monitor cgroup: {group}"
    );
    assert_eq!(cgroup(plain_pid), group);
    assert_ne!(cgroup(external_pid), group);
    let exec_pid = exec_monitor(&group, &external_id);

    h.client.stop_session(&session.id).await.unwrap();
    assert_eq!(
        store
            .output(&["container", "exists", "--", auto])
            .await
            .status
            .code(),
        Some(1)
    );
    assert_eq!(
        store
            .read(&[
                "container",
                "inspect",
                "--format",
                "{{.State.Status}}",
                plain
            ])
            .await,
        "exited"
    );
    assert_eq!(store.running_monitor(outside).await, external_pid);
    wait_until_pid_gone(exec_pid, 10).await;
    drop(rx);
    drop(h);
    store.read(&store.removal()).await;
    store.cleanup_needed = false;
}
