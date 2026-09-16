//! Read-only inventory of the supervisor user's live executable processes.
//!
//! This is the observation seam for operations such as a future safe
//! uninstaller. It reuses [`crate::procs`] rather than maintaining another
//! process-table walk, and it never reads argv or environments, signals a
//! process, or changes filesystem state.

use std::path::PathBuf;

/// One live same-euid process whose identity survived executable inspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableProcess {
    /// The operating-system process identifier.
    pub pid: u32,
    /// The parent process observed by the final identity read.
    pub parent_pid: u32,
    /// The platform's opaque process-start identity token.
    pub start_time: u64,
    /// The native executable path, unchanged from the kernel response.
    pub executable: PathBuf,
}

/// The result of one process inventory walk.
///
/// `uncertainties` records observations that could not be trusted without
/// turning a partial walk into a false claim that the host was clean. A
/// successful inventory may therefore contain both processes and
/// uncertainties.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInventory {
    /// Processes whose PID and start identity remained stable while reading
    /// the executable path.
    pub processes: Vec<ExecutableProcess>,
    /// Soft enumeration and per-process errors, in deterministic order.
    pub uncertainties: Vec<String>,
}

/// The smallest observation surface needed by [`inventory`]. Keeping process
/// table, identity, and executable reads injectable makes PID-reuse and
/// partial-failure cases deterministic without weakening the native path.
trait Reader {
    fn snapshot(&mut self) -> Result<(crate::procs::ProcessTable, Vec<String>), String>;
    fn read_process(
        &mut self,
        pid: u32,
    ) -> Result<Option<(u32, u64, crate::procs::ProcessState)>, String>;
    fn read_executable(&mut self, pid: u32) -> Result<PathBuf, String>;
}

/// Production adapter that delegates every observation to the existing
/// platform-specific process seam.
struct NativeReader;

impl Reader for NativeReader {
    fn snapshot(&mut self) -> Result<(crate::procs::ProcessTable, Vec<String>), String> {
        crate::procs::inspection_snapshot()
    }

    fn read_process(
        &mut self,
        pid: u32,
    ) -> Result<Option<(u32, u64, crate::procs::ProcessState)>, String> {
        crate::procs::inspection_read_process(pid)
    }

    fn read_executable(&mut self, pid: u32) -> Result<PathBuf, String> {
        crate::procs::read_executable(pid)
    }
}

/// Observe native executable paths for live, stable same-euid process identities.
///
/// A process that vanishes or is a zombie during the walk is omitted. A live
/// process whose identity or executable cannot be read is represented by an
/// uncertainty carrying the PID, operation, and concrete error. Aggregate
/// process-table failure remains an `Err`, preserving the existing sweep's
/// fail-closed distinction between an empty table and an unreadable table.
pub fn processes() -> Result<ProcessInventory, String> {
    inventory(&mut NativeReader)
}

/// Run the identity-checked walk against one observation provider.
///
/// The initial and final reads deliberately bracket the executable lookup;
/// only a matching start-time token can make the path attributable to the
/// snapshot row.
fn inventory<R: Reader>(reader: &mut R) -> Result<ProcessInventory, String> {
    let (table, mut uncertainties) = reader.snapshot()?;
    let mut pids: Vec<_> = table.keys().copied().collect();
    pids.sort_unstable();
    let mut processes = Vec::new();

    for pid in pids {
        let Some((_, snapshot_start)) = table.get(&pid).copied() else {
            continue;
        };
        let initial = match reader.read_process(pid) {
            Ok(Some((_, start, crate::procs::ProcessState::Running))) => start,
            Ok(Some((_, _, crate::procs::ProcessState::Zombie)) | None) => continue,
            Err(error) => {
                uncertainties.push(format!("pid {pid} process identity: {error}"));
                continue;
            }
        };
        if initial != snapshot_start {
            uncertainties.push(format!(
                "pid {pid} process identity changed before executable inspection (expected start time {snapshot_start}, observed {initial})"
            ));
            continue;
        }

        // The final identity read is required even when the path lookup
        // failed. A process that exited during that lookup is absent; only a
        // still-live original identity makes the executable error reportable.
        let executable_result = reader.read_executable(pid);
        let final_read = match reader.read_process(pid) {
            Ok(Some((parent_pid, start, crate::procs::ProcessState::Running))) => {
                (parent_pid, start)
            }
            Ok(Some((_, _, crate::procs::ProcessState::Zombie)) | None) => continue,
            Err(error) => {
                uncertainties.push(format!("pid {pid} process identity: {error}"));
                continue;
            }
        };
        if final_read.1 != snapshot_start {
            uncertainties.push(format!(
                "pid {pid} process identity changed during executable inspection (expected start time {snapshot_start}, observed {})",
                final_read.1
            ));
            continue;
        }

        let executable = match executable_result {
            Ok(path) => path,
            Err(error) => {
                uncertainties.push(format!("pid {pid} executable lookup: {error}"));
                continue;
            }
        };

        processes.push(ExecutableProcess {
            pid,
            parent_pid: final_read.0,
            start_time: snapshot_start,
            executable,
        });
    }

    uncertainties.sort();
    Ok(ProcessInventory {
        processes,
        uncertainties,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, VecDeque};

    /// One scripted identity observation: a live/zombie identity, confirmed
    /// absence, or a concrete read failure. Queues model changes between reads.
    type IdentityRead = Result<Option<(u32, u64, crate::procs::ProcessState)>, String>;

    /// Deterministic reader fixture: each PID owns a queue of identity
    /// results, while executable results are consumed once. Tests replace
    /// individual queue entries to model exits, zombies, reuse, and errors.
    #[derive(Default)]
    struct FakeReader {
        table: crate::procs::ProcessTable,
        snapshot_errors: Vec<String>,
        soft_errors: Vec<String>,
        process_reads: HashMap<u32, VecDeque<IdentityRead>>,
        executable_reads: HashMap<u32, Result<PathBuf, String>>,
    }

    impl Reader for FakeReader {
        fn snapshot(&mut self) -> Result<(crate::procs::ProcessTable, Vec<String>), String> {
            if let Some(error) = self.snapshot_errors.pop() {
                return Err(error);
            }
            Ok((self.table.clone(), std::mem::take(&mut self.soft_errors)))
        }

        fn read_process(
            &mut self,
            pid: u32,
        ) -> Result<Option<(u32, u64, crate::procs::ProcessState)>, String> {
            self.process_reads
                .get_mut(&pid)
                .and_then(VecDeque::pop_front)
                .unwrap_or_else(|| Err(format!("missing fake process read for pid {pid}")))
        }

        fn read_executable(&mut self, pid: u32) -> Result<PathBuf, String> {
            self.executable_reads
                .remove(&pid)
                .unwrap_or_else(|| Err(format!("missing fake executable read for pid {pid}")))
        }
    }

    /// Build one successful process identity response for the fake reader.
    fn running(parent: u32, start: u64) -> Option<(u32, u64, crate::procs::ProcessState)> {
        Some((parent, start, crate::procs::ProcessState::Running))
    }

    /// Construct a fake process table with two stable identity reads per PID;
    /// individual tests replace queue entries to model races and failures.
    fn fake(table: &[(u32, u32, u64)], executable: &[(u32, &str)]) -> FakeReader {
        let mut reader = FakeReader::default();
        for &(pid, parent, start) in table {
            reader.table.insert(pid, (parent, start));
            reader.process_reads.insert(
                pid,
                VecDeque::from([Ok(running(parent, start)), Ok(running(parent, start))]),
            );
        }
        reader.executable_reads.extend(
            executable
                .iter()
                .map(|&(pid, path)| (pid, Ok(PathBuf::from(path)))),
        );
        reader
    }

    /// Stable live rows become inventory entries while preserving their
    /// parent and native executable path.
    #[farhelm_testtrace::test]
    fn stable_process_is_reported() {
        let mut reader = fake(
            &[(9, 3, 90), (4, 2, 40)],
            &[(9, "/opt/agent (deleted)"), (4, "/bin/helper")],
        );
        let result = inventory(&mut reader).unwrap();
        assert_eq!(
            result.processes,
            vec![
                ExecutableProcess {
                    pid: 4,
                    parent_pid: 2,
                    start_time: 40,
                    executable: PathBuf::from("/bin/helper"),
                },
                ExecutableProcess {
                    pid: 9,
                    parent_pid: 3,
                    start_time: 90,
                    executable: PathBuf::from("/opt/agent (deleted)"),
                },
            ]
        );
        assert!(result.uncertainties.is_empty());
    }

    /// A zombie and a process that vanishes during either identity check are
    /// absent because neither can execute the inspected executable.
    #[farhelm_testtrace::test]
    fn zombie_and_vanished_processes_are_absent() {
        let mut reader = fake(
            &[(9, 3, 90), (10, 3, 100), (11, 3, 110), (12, 3, 120)],
            &[
                (9, "/bin/a"),
                (10, "/bin/b"),
                (11, "/bin/c"),
                (12, "/bin/d"),
            ],
        );
        reader.process_reads.get_mut(&9).unwrap()[0] =
            Ok(Some((3, 90, crate::procs::ProcessState::Zombie)));
        reader.process_reads.get_mut(&10).unwrap()[0] = Ok(None);
        reader.process_reads.get_mut(&11).unwrap()[1] = Ok(None);
        reader.process_reads.get_mut(&12).unwrap()[1] =
            Ok(Some((3, 120, crate::procs::ProcessState::Zombie)));
        let result = inventory(&mut reader).unwrap();
        assert!(result.processes.is_empty());
        assert!(result.uncertainties.is_empty());
    }

    /// A replacement PID is uncertainty, whether detected before or after
    /// reading its executable, so the path is never attributed incorrectly.
    #[farhelm_testtrace::test]
    fn pid_reuse_is_uncertainty() {
        let mut reader = fake(&[(9, 3, 90)], &[(9, "/bin/replacement")]);
        reader.process_reads.get_mut(&9).unwrap()[0] = Ok(running(3, 91));
        let result = inventory(&mut reader).unwrap();
        assert_eq!(result.processes, Vec::new());
        assert_eq!(
            result.uncertainties,
            vec!["pid 9 process identity changed before executable inspection (expected start time 90, observed 91)".to_owned()]
        );

        let mut reader = fake(&[(9, 3, 90)], &[(9, "/bin/replacement")]);
        reader.process_reads.get_mut(&9).unwrap()[1] = Ok(running(3, 91));
        let result = inventory(&mut reader).unwrap();
        assert_eq!(result.processes, Vec::new());
        assert_eq!(
            result.uncertainties,
            vec!["pid 9 process identity changed during executable inspection (expected start time 90, observed 91)".to_owned()]
        );
    }

    /// Live read failures remain visible with their operation and concrete
    /// error, while a global snapshot failure stays a hard error.
    #[farhelm_testtrace::test]
    fn lookup_failures_are_uncertainties_and_snapshot_failure_is_hard() {
        let mut reader = fake(&[(9, 3, 90)], &[]);
        reader.process_reads.get_mut(&9).unwrap()[0] = Err("permission denied".into());
        assert_eq!(
            inventory(&mut reader).unwrap().uncertainties,
            vec!["pid 9 process identity: permission denied"]
        );

        let mut reader = fake(&[(9, 3, 90)], &[(9, "/bin/agent")]);
        reader
            .executable_reads
            .insert(9, Err("unreadable exe".into()));
        assert_eq!(
            inventory(&mut reader).unwrap().uncertainties,
            vec!["pid 9 executable lookup: unreadable exe"]
        );

        let mut reader = fake(&[(9, 3, 90)], &[(9, "/bin/agent")]);
        reader.process_reads.get_mut(&9).unwrap()[1] = Err("identity race".into());
        assert_eq!(
            inventory(&mut reader).unwrap().uncertainties,
            vec!["pid 9 process identity: identity race"]
        );

        let mut reader = fake(&[(9, 3, 90)], &[(9, "/bin/agent")]);
        reader
            .executable_reads
            .insert(9, Err("vanishing exe".into()));
        reader.process_reads.get_mut(&9).unwrap()[1] = Ok(None);
        let result = inventory(&mut reader).unwrap();
        assert!(result.processes.is_empty());
        assert!(result.uncertainties.is_empty());

        let mut reader = fake(&[(9, 3, 90)], &[(9, "/bin/agent")]);
        reader.executable_reads.insert(9, Err("zombie exe".into()));
        reader.process_reads.get_mut(&9).unwrap()[1] =
            Ok(Some((3, 90, crate::procs::ProcessState::Zombie)));
        let result = inventory(&mut reader).unwrap();
        assert!(result.processes.is_empty());
        assert!(result.uncertainties.is_empty());

        let mut reader = fake(&[(9, 3, 90)], &[]);
        reader.snapshot_errors.push("table unavailable".into());
        assert_eq!(inventory(&mut reader), Err("table unavailable".into()));
    }

    /// Per-process enumeration diagnostics survive as uncertainties and are
    /// sorted alongside later lookup failures for deterministic callers.
    #[farhelm_testtrace::test]
    fn snapshot_soft_errors_are_preserved_and_sorted() {
        let mut reader = fake(&[(9, 3, 90)], &[(9, "/bin/agent")]);
        reader.soft_errors = vec!["zeta".into(), "alpha".into()];
        let result = inventory(&mut reader).unwrap();
        assert_eq!(result.uncertainties[..2], ["alpha", "zeta"]);
    }

    /// The native path API must resolve this test process itself, proving the
    /// production reader exercises the operating system rather than only the
    /// injected fake seam.
    #[farhelm_testtrace::test]
    fn native_inventory_finds_this_process() {
        let inventory = processes().expect("native process inventory must succeed");
        let own_pid = std::process::id();
        let expected_path = std::env::current_exe().expect("current executable must resolve");
        let expected_identity = crate::procs::read_process(own_pid)
            .expect("native identity lookup must resolve this process")
            .expect("this process must still exist");
        let process = inventory
            .processes
            .iter()
            .find(|process| process.pid == own_pid)
            .expect("inventory must include this process");
        assert_eq!(process.parent_pid, expected_identity.0);
        assert_eq!(process.start_time, expected_identity.1);
        assert_eq!(
            std::fs::canonicalize(&process.executable)
                .unwrap_or_else(|_| process.executable.clone()),
            std::fs::canonicalize(expected_path).expect("current executable canonicalization")
        );
    }

    /// Linux makes non-dumpable processes' procfs directories root-owned even
    /// though their effective UID has not changed. A real post-exec child
    /// proves that the native walk reports that process or its unreadable
    /// executable, rather than silently treating the directory owner as its UID.
    #[cfg(target_os = "linux")]
    #[farhelm_testtrace::test]
    fn native_inventory_retains_non_dumpable_child() {
        use std::io::{BufRead, BufReader};
        use std::os::fd::AsRawFd;
        use std::process::{Command, Stdio};

        /// Keep failure diagnostics inside the child's lifetime and always
        /// reap it, including when a readiness or inventory assertion fails.
        struct ChildGuard(std::process::Child);
        impl Drop for ChildGuard {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let mut child = ChildGuard(Command::new("python3")
            .args(["-c", "import ctypes, os, sys; c = ctypes.CDLL(None); assert c.prctl(4, 0, 0, 0, 0) == 0; print(os.geteuid(), c.prctl(3, 0, 0, 0, 0), flush=True); sys.stdin.buffer.read()"])
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit())
            .spawn().expect("Python fixture must start"));
        let stdout = child.0.stdout.take().unwrap();
        let mut pollfd = libc::pollfd {
            fd: stdout.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: poll receives one initialized descriptor whose owner remains
        // alive through the call. The finite timeout bounds fixture readiness.
        let ready = unsafe { libc::poll(&mut pollfd, 1, 10_000) };
        assert_eq!(
            ready,
            1,
            "child readiness failed; child status {:?}",
            child.0.try_wait()
        );
        let mut line = String::new();
        BufReader::new(stdout).read_line(&mut line).unwrap();
        // SAFETY: geteuid has no arguments and cannot fail.
        let uid = unsafe { libc::geteuid() };
        assert_eq!(
            line,
            format!("{uid} 0\n"),
            "child must report unchanged euid and disabled dumpability"
        );
        let pid = child.0.id();
        // Procfs ownership depends on the kernel and namespace substrate;
        // dumpability and actual credentials are the fixture contract. The
        // credential parser separately covers differing real/effective UIDs.
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "fixture must remain live during inspection"
        );
        let result = processes().expect("native process inventory must succeed");
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "fixture must remain live after inspection"
        );
        assert!(
            result.processes.iter().any(|process| process.pid == pid)
                || result
                    .uncertainties
                    .iter()
                    .any(|error| error.starts_with(&format!(
                        "pid {pid} executable lookup: reading /proc/{pid}/exe:"
                    ))),
            "non-dumpable child {pid} must be visible or carry its concrete executable failure: {result:?}"
        );
    }
}
