//! Terminal hosts from the supervisor's side: where their sockets live, what
//! they are called, what each one is tagged with, and how one is started.
//!
//! A terminal host (`crates/terminal-host/`, contract in its `SPEC.md`) is a
//! small long-lived process that owns one pty and the program in it. It
//! outlives the supervisor that started it, through restarts and updates, so
//! everything here is shaped by one question: can a later supervisor, maybe
//! of a newer release, find a host and tell what it is for?
//!
//! - [`dir`]: the private socket directory, `<state dir>/hosts`, mode 0700.
//!   The directory's permissions are the only access boundary a host has.
//! - [`names`]: short random socket names, so a host socket path is never
//!   longer than the supervisor's own socket path in the same state directory.
//! - [`tag`]: the client tag, the bytes a host returns verbatim in `Info`.
//!   Its encoding is permanent, like a wire format: running hosts keep their
//!   tag across updates.
//! - [`spawn`]: starting a host from the supervisor's own program and waiting
//!   for its readiness.
//! - [`client`]: connecting to a host (what a socket holds: a usable host,
//!   nothing, or a host that cannot be used) and the requests a host
//!   terminal's runtime drives over the connection.
//! - [`process`]: the host as a process, pinned by pid and start identity
//!   from the connection's peer credentials, and the verified SIGTERM for a
//!   host the protocol cannot end.
//! - [`registry`]: discovery, the scan of a state directory's socket
//!   directory that finds its hosts again after a restart, sorts them into
//!   live, unavailable and gone, and looks them up by session, launch
//!   generation and tab.
//! - [`stop`]: ending every host of a state directory with no supervisor
//!   involved, behind `farhelm internal stop-terminal-hosts`, a command
//!   line every later release keeps accepting.
//! - [`terminal`]: a host terminal's runtime, the one task per host that
//!   owns its connection and the terminal's emulator: output fed and
//!   queries answered, the child's liveness, input, resizes and shutdown,
//!   and adapters to the types the supervisor's lifecycle verbs decide with.
//! - `test_program` (unit tests only): this crate's test binary as a host
//!   program, so that its unit tests run their sessions in real hosts.
//!
//! The service's terminal switch points handle a session whose terminal is
//! a [`terminal::HostTerminal`]. Every
//! create starts its agent in a host, and every new tab opens in one. A
//! restarted supervisor finds
//! the hosts of its host rows again with [`registry`] and adopts them
//! ([`terminal::HostTerminal::adopt_found`]), in
//! `service::core::host_handover`, and its sessions' tab hosts in
//! `service::core::host_tabs`.
//! The module is public, rather than crate-private, because the tests that
//! start real hosts live beside the real `farhelm` binary
//! (`crates/farhelm/tests/terminal_host_spawn.rs`, `terminal_host_client.rs`,
//! `terminal_host_registry.rs` and `terminal_host_runtime.rs`): a host forks,
//! so it is never started inside a test process. (This crate's own unit
//! tests start real hosts too, as processes of their own: `test_program`.)

pub mod client;
pub mod dir;
pub mod names;
pub mod process;
pub mod registry;
pub mod spawn;
pub mod stop;
pub mod tag;
pub mod terminal;
#[cfg(test)]
pub(crate) mod test_program;

pub use client::{
    ClientDeadlines, ClientError, ConnectError, Gone, HostConnection, HostEvent, InputCounts,
    InputTicket, ShutdownOutcome, SnapshotReply, Unavailable,
};
pub use process::{HostProcess, TerminateError, Termination, terminate_verified};

pub use dir::{HOSTS_DIR_NAME, HostsDirError, ensure_hosts_dir, existing_hosts_dir, hosts_dir};
pub use names::HostName;
pub use registry::{
    DuplicateAgents, DuplicateTab, GoneHost, HostRegistry, LiveHost, NamedHost, ScanError,
    ScanOptions, SessionTabs, Skipped, SkippedEntry, SocketRemoval, UnavailableHost,
    WhyUnavailable,
};
pub use spawn::{
    ChildSpec, DEFAULT_JOURNAL_CAP, SpawnError, SpawnedHost, StartingHost, free_host_name,
    own_host_program, spawn_host, start_host,
};
pub use stop::{
    HostOutcome, HostReport, StopLimits, StopReport, Stopped, end_host_at, stop_terminal_hosts,
    write_lines_within,
};
pub use tag::{HostRole, HostTag, TagError, UnreadableTag};
pub use terminal::{
    AdoptError, Adoption, CheckpointOutcome, CheckpointPolicy, Clock, EmulatorRef, HostEvidence,
    HostStatus, HostTerminal, HostTerminalOptions, InputFate, InputReceipt, InputRefused,
    InputSequence, Liveness, LivenessChange, LivenessNotifier, MAX_PASTE, NotTaken,
    NoticeSubscription, ProcessRoot, ResizeRefused, ShutdownReport, Unavailability,
};
