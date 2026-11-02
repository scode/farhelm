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
//!   and adapters to the types the supervisor decides with for tmux panes.
//!
//! Nothing in the supervisor calls this yet; the switch that creates new
//! terminals in hosts comes later, and so does running discovery at startup.
//! The module is public, rather than crate-private, because the tests that
//! start real hosts live beside the real `farhelm` binary
//! (`crates/farhelm/tests/terminal_host_spawn.rs`, `terminal_host_client.rs`,
//! `terminal_host_registry.rs` and `terminal_host_runtime.rs`): a host forks,
//! so it is never started inside a test process.

pub mod client;
pub mod dir;
pub mod names;
pub mod process;
pub mod registry;
pub mod spawn;
pub mod stop;
pub mod tag;
pub mod terminal;

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
    ChildSpec, DEFAULT_JOURNAL_CAP, SpawnError, SpawnedHost, own_host_program, spawn_host,
};
pub use stop::{
    HostOutcome, HostReport, StopLimits, StopReport, Stopped, stop_terminal_hosts,
    write_lines_within,
};
pub use tag::{HostRole, HostTag, TagError, UnreadableTag};
pub use terminal::{
    AdoptError, Adoption, CheckpointOutcome, CheckpointPolicy, Clock, EmulatorRef, HostEvidence,
    HostStatus, HostTerminal, HostTerminalOptions, InputFate, InputReceipt, InputRefused,
    InputSequence, Liveness, LivenessChange, LivenessNotifier, NotTaken, NoticeSubscription,
    ProcessRoot, ResizeRefused, ShutdownReport, Unavailability,
};
