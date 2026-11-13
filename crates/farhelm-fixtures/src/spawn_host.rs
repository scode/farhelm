//! `farhelm-fixtures spawn-terminal-host`: start one real terminal host in a
//! state directory, for shell harnesses that have to prove what their
//! cleanup ends and what it leaves alone.
//!
//! The host takes its child spec, an encoded record, on stdin, so a shell
//! script cannot start one by itself; and a test process must not fork one
//! (a host daemonizes from a fork). This verb is the supervisor's own spawn
//! (`farhelm_supervisor::hosts::spawn_host`) behind a command line, used by
//! the desktop smoke and the e2e suite's `terminal_host_cleanup` tests. It starts the host the supervisor's way
//! (same program, same socket directory, same readiness), so a host it
//! starts is indistinguishable, to a cleanup, from one a supervisor started.
//!
//! The program in the host waits to be hung up and nothing else, so ending
//! the host is what ends it. The host outlives this process on purpose: the
//! caller owns it from the moment its pid and socket are printed, and must
//! end it (`farhelm internal stop-terminal-hosts --state-dir <dir>`).

use anyhow::{Context as _, Result};
use farhelm_supervisor::hosts::{ChildSpec, HostConnection, HostRole, HostTag, spawn_host};
use std::path::Path;
use terminal_host::protocol::Size;

/// The session id written into every fixture host's tag. None of the
/// cleanups this verb exists for reads it: they find hosts by place, never
/// by tag. It is a fixed, obviously synthetic UUID so that a host found
/// later in a state directory is recognisable as this fixture's.
const FIXTURE_SESSION_ID: &str = "00000000-0000-4000-8000-0000000f1a7e";

/// Start a host in `state_dir`'s socket directory with `farhelm` as the host
/// program, and print one line, `<pid> <socket>`, once it is ready.
///
/// The pid comes first so that a shell can `read -r pid socket` it whatever
/// the socket path holds. It is the host's own pid, learned from the
/// connection's peer credentials the way the supervisor learns it, so a
/// caller can check the process is alive and still serves that socket.
pub fn run(farhelm: &Path, state_dir: &Path) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("starting the async runtime")?;
    runtime.block_on(async {
        let child = ChildSpec {
            // `exec` so that the hangup reaches `sleep` itself, with no shell
            // between it and the terminal that could outlive it.
            argv: vec!["/bin/sh".into(), "-c".into(), "exec sleep 3600".into()],
            cwd: "/".into(),
            env: vec![("PATH".into(), "/usr/bin:/bin".into())],
            size: Size { cols: 80, rows: 24 },
            journal_cap: 64 * 1024,
            initial_snapshot: None,
        };
        let tag = HostTag {
            session_id: FIXTURE_SESSION_ID.into(),
            created_ns: farhelm_supervisor::hosts::tag::creation_stamp(),
            role: HostRole::Agent { generation: 1 },
        };
        let spawned = spawn_host(farhelm, state_dir, child, &tag)
            .await
            .context("starting the terminal host")?;
        // Connecting takes nothing from the host (it has no other client),
        // and dropping the connection leaves it running with none.
        let connection = HostConnection::connect(&spawned.socket)
            .await
            .with_context(|| format!("connecting to {}", spawned.socket.display()))?;
        let pid = connection
            .peer()
            .with_context(|| format!("no peer process for {}", spawned.socket.display()))?
            .pid;
        println!("{pid} {}", spawned.socket.display());
        Ok(())
    })
}
