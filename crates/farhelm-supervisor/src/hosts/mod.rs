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
//!
//! Nothing in the product calls this yet; the switch that creates new
//! terminals in hosts comes later. Connecting to hosts and discovering them
//! after a restart are not here either. The module is public, rather than
//! crate-private, because the tests that start real hosts live beside the real
//! `farhelm` binary (`crates/farhelm/tests/terminal_host_spawn.rs`): a host
//! forks, so it is never started inside a test process.

pub mod dir;
pub mod names;
pub mod spawn;
pub mod tag;

pub use dir::{HOSTS_DIR_NAME, HostsDirError, ensure_hosts_dir, hosts_dir};
pub use names::HostName;
pub use spawn::{ChildSpec, SpawnError, SpawnedHost, own_host_program, spawn_host};
pub use tag::{HostRole, HostTag, TagError, UnreadableTag};
