//! Discovery: which terminal hosts a state directory's socket directory
//! holds, what each one is for, and which ones cannot be used.
//!
//! Hosts outlive the supervisor that started them, and a supervisor keeps no
//! list of them: a host is found again by its socket in `<state dir>/hosts`
//! and identified by the client tag it returns in `Info`
//! ([`super::tag`]), never by its socket name, which means nothing. A scan
//! ([`HostRegistry::scan`]) connects to every socket there, asks each host
//! for its `Info`, decodes its tag, and sorts what it found into four
//! classes. The classes decide what a caller may do:
//!
//! - **Live** ([`LiveHost`]): a host that answered and whose tag this build
//!   reads. Live means attributable, not adoptable: the scan reads only the
//!   tag, and a host whose snapshot record is in a format this build cannot
//!   decode is live here too. The adopter learns that only when it reads the
//!   host's stream, and must then treat the host as unavailable (leave it
//!   running, never reclaim it), exactly as if discovery had said so. It
//!   comes with the connection the scan made, which is the host's owning
//!   connection from then on (a second one would take the host over from
//!   it, `hosts::client`, "Always take over"); a caller adopting the host
//!   takes it out of the registry ([`HostRegistry::take`]) rather than
//!   reconnect.
//! - **Unavailable** ([`UnavailableHost`]): something that may be a running
//!   host, but one this build cannot use or cannot attribute: it does not
//!   answer, speaks another protocol version, stays busy, does not report
//!   its `Info`, or carries a tag of an unknown version, not Farhelm's or
//!   malformed. Such a host is kept as it is. It is never an orphan, never
//!   shut down and never has its socket removed by a policy built on this
//!   scan, because a newer supervisor may own it and come back after a
//!   downgrade; only a deliberate end of a host the caller knows to be its
//!   own goes through [`super::process::terminate_verified`] with the
//!   [`UnavailableHost::peer`] given here.
//! - **Gone** ([`GoneHost`]): nothing listens. Either the socket file went
//!   between listing and connecting (a host removes its own as it ends), or
//!   it refused every connection for the refusal window and no process of
//!   this user serves it (a host killed without the chance to remove it).
//!   This is the one class whose socket may be removed, with
//!   [`GoneHost::remove_socket`], which judges the socket gone again at the
//!   time of removal rather than trusting the scan. When to remove it is the
//!   caller's policy (startup reclaim); this module only provides the
//!   removal.
//! - **Skipped** ([`SkippedEntry`]): an entry that is not a socket, or one
//!   that could not be read. Not a host, and never touched.
//!
//! Lookups ([`HostRegistry::agent`], [`HostRegistry::tabs`],
//! [`HostRegistry::of_session`]) see live hosts only, since only their tags
//! were read. An unavailable host may belong to any session, so an empty
//! lookup means "no host of that session" only when
//! [`HostRegistry::is_complete`] says every socket was attributed.
//!
//! What a scan does to the hosts it finds: it takes over each one's
//! connection (the plan's D-11), which cuts off any other client the host
//! had. It sends nothing else but `Info`, never input, and dropping a
//! registry closes every connection it still holds, which leaves the hosts
//! as they were. Taking over is right only because the scanning process is
//! the one that owns the state directory, so two preconditions bind every
//! caller:
//!
//! - **The caller holds the state directory's lock**, as the supervisor
//!   does. Any other process scanning a running supervisor's state directory
//!   would cut off every host connection that supervisor has.
//! - **Inside a running supervisor, nothing it drives or is starting is
//!   scanned.** [`ScanOptions::skip`] must hold the socket name of every
//!   host a runtime holds a connection to, and of every spawn in flight. A
//!   spawn's socket is in the directory, and connectable, before the spawn
//!   returns, so a name added only once the spawn has finished leaves a
//!   window in which the scan takes the new host over from its starter. A
//!   caller either serializes scans with its spawns or reserves each
//!   spawn's name in its skip source before the host binds it (the spawn
//!   draws its name itself today; a caller that needs the reservation has
//!   `hosts::spawn` take the name from it).
//!
//! Every host is classified within its own bounds: the connect deadline (a
//! socket that only refuses is followed for up to the refusal window past
//! it, `hosts::client`'s connect docs), at most one process-table walk
//! bounded by [`super::process::PROCESS_SEARCH_DEADLINE`] for a socket that
//! names no peer, then one reply deadline for `Info`. Up to
//! [`SCAN_CONCURRENCY`] hosts are classified at once, so a few wedged hosts
//! cost one deadline, not one each.

use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::os::unix::fs::{FileTypeExt as _, MetadataExt as _};
use std::path::{Path, PathBuf};

use terminal_host::protocol::Info;
use tokio::task::JoinSet;

use super::client::{
    ClientDeadlines, ClientError, ConnectError, Gone, HostConnection, Unavailable,
};
use super::dir::{HostsDirError, existing_hosts_dir};
use super::process::{HostProcess, PROCESS_SEARCH_DEADLINE, find_socket_process};
use super::tag::{HostRole, HostTag, UnreadableTag};

/// How many hosts one scan classifies at the same time. Each takes one
/// connection and, for a socket that does not answer, one walk of the
/// process table; this bounds both while keeping a directory with a few
/// wedged hosts from costing a connect deadline per host.
pub const SCAN_CONCURRENCY: usize = 16;

// ---- Options and errors -----------------------------------------------------------

/// How a scan connects, and which sockets it leaves alone.
#[derive(Debug, Clone, Default)]
pub struct ScanOptions {
    /// The client's deadlines for each host. [`Default`] gives the client's
    /// own; tests shorten them.
    pub deadlines: ClientDeadlines,
    /// Socket names in the directory not to contact. In a running
    /// supervisor this must hold both the hosts its runtimes hold the owning
    /// connection to and the names of spawns in flight (module docs, the
    /// preconditions); a name missing from it is taken over. Skipped names
    /// appear in no class of the result.
    pub skip: HashSet<OsString>,
}

/// Why a scan could not run at all. Problems with single hosts or entries
/// are never errors: they are classes of the result.
#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    /// The state directory's absolute path could not be made.
    #[error("resolving the state directory: {0}")]
    StateDir(io::Error),
    /// The socket directory exists in a shape that is refused: a socket in
    /// it may not be this supervisor's host (`hosts::dir`).
    #[error(transparent)]
    HostsDir(#[from] HostsDirError),
    /// The socket directory could not be listed.
    #[error("listing terminal host directory {path}: {source}")]
    List { path: PathBuf, source: io::Error },
}

// ---- What a scan finds -------------------------------------------------------------

/// A host that answered and whose tag this build reads: attributable to a
/// session, which is not yet adoptable. Whether its snapshot record decodes
/// is learned only by the adopter reading its stream, and one that does
/// not is then handled as unavailable (module docs).
#[derive(Debug)]
pub struct LiveHost {
    /// The socket's name in the socket directory.
    pub name: OsString,
    /// The socket's full path, absolute.
    pub socket: PathBuf,
    /// What the host is for.
    pub tag: HostTag,
    /// The host's `Info` as it was during the scan. Its child may have
    /// exited since, and its offsets have moved on; it is for classifying,
    /// not for driving a terminal.
    pub info: Info,
    /// The connection the scan made, which owns the host from now on (see
    /// the module docs).
    pub connection: HostConnection,
}

impl LiveHost {
    /// The host process, as the connection's peer credentials named it.
    pub fn peer(&self) -> Option<HostProcess> {
        self.connection.peer()
    }
}

/// Something at a socket that may be a running host this build cannot use
/// or attribute. Kept as it is: see the module docs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnavailableHost {
    /// The socket's name in the socket directory.
    pub name: OsString,
    /// The socket's full path, absolute.
    pub socket: PathBuf,
    /// The process serving the socket, from the connection's peer
    /// credentials or the process table, when either named one: what
    /// [`super::process::terminate_verified`] needs, should the caller have
    /// reason to end this host anyway.
    pub peer: Option<HostProcess>,
    /// Why it cannot be used.
    pub why: WhyUnavailable,
}

/// Why a host is unavailable.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WhyUnavailable {
    /// Connecting found a host that cannot be used: another protocol
    /// version, a refusal, a malformed answer, no answer in time, a socket
    /// that takes no connection while a process still serves it.
    #[error("{0}")]
    Connect(Unavailable),
    /// The host answered `Busy` until the connect deadline. A version-1 host
    /// never does to a take-over; kept apart so that it never reads as gone.
    #[error("it stayed busy")]
    Busy,
    /// The host said `Welcome` but its `Info` did not arrive.
    #[error("it did not report its state: {0}")]
    Info(ClientError),
    /// The host answered, but its tag is one this build cannot read: most
    /// likely a newer release's. Its `Info` is kept for display.
    #[error("{tag}")]
    Tag { tag: UnreadableTag, info: Info },
}

/// A socket nothing listened on when the scan judged it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoneHost {
    /// The socket's name in the socket directory.
    pub name: OsString,
    /// The socket's full path, absolute.
    pub socket: PathBuf,
    /// How connecting found it gone.
    pub why: Gone,
    /// The socket file as it was listed, before the connect judged it.
    file: FileId,
    /// The scan's deadlines, whose refusal window [`Self::remove_socket`]
    /// applies again.
    deadlines: ClientDeadlines,
}

/// A file's identity on its filesystem: device and inode number. It tells a
/// later file at the same path apart only while the inode number is not
/// reused, so it never authorizes a removal on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileId {
    dev: u64,
    ino: u64,
}

impl FileId {
    fn of(meta: &fs::Metadata) -> Self {
        Self {
            dev: meta.dev(),
            ino: meta.ino(),
        }
    }
}

/// What [`GoneHost::remove_socket`] did. Only [`Self::Removed`] removed
/// anything; every other outcome left the path as it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SocketRemoval {
    /// The socket was still gone, and its file was removed.
    Removed,
    /// Nothing was at the path any more.
    AlreadyGone,
    /// Another file is at the path now (not a socket, or a socket of another
    /// identity).
    Replaced,
    /// Something serves the socket now: a connection was accepted or queued,
    /// or a live process of this user names the socket on its command line
    /// (a host that has bound but not yet listened, or a stopped one whose
    /// backlog is full). The process, when the table named one.
    StillServed { peer: Option<HostProcess> },
    /// Whether the socket is gone could not be settled (a connect failed in
    /// some other way, the process table could not be searched in time) or
    /// the removal itself failed. Kept, as an unavailable host would be.
    Unconfirmed(String),
}

/// What plain connections, without a `Hello`, found at a socket over the
/// refusal window.
enum Probe {
    /// The file is missing.
    Missing,
    /// Every connection was refused for the whole window.
    Refused,
    /// A connection was accepted or queued, or the backlog was full.
    Answers,
    /// Anything else, with what happened.
    Unsettled(String),
}

impl GoneHost {
    /// Remove this host's socket file, if the socket is still gone now.
    ///
    /// The scan's judgment is not trusted for this: by now the file may be
    /// another host's. Not even the file's identity settles it, since a
    /// socket bound after the old file was unlinked can get the same inode
    /// number. So the socket is judged again the way the client judges gone
    /// (`hosts::client`, "Outcome classes at connect"): plain connections,
    /// with no `Hello` (which a host ignores and closes, so a live host's
    /// own client is never disturbed), refused for the whole refusal window,
    /// and then no live process of this user naming the socket on its
    /// command line. The process check is what covers a host between its
    /// `bind` and its `listen`, whose socket refuses connections like a dead
    /// one; its command line already names the socket. A process search that
    /// fails or runs past [`PROCESS_SEARCH_DEADLINE`] is
    /// [`SocketRemoval::Unconfirmed`], never gone. Only then, and only while
    /// the path still holds a socket of the identity the scan listed, is the
    /// file unlinked.
    ///
    /// **Precondition: nothing binds sockets in this state directory while
    /// this runs.** Between the last check and the unlink a host could still
    /// bind the path if the old file had been removed and its name drawn
    /// again in that moment. The caller closes that window by holding the
    /// state directory's lock (so no other supervisor spawns there) and by
    /// never running a removal concurrently with a spawn of its own in the
    /// same state directory.
    ///
    /// Takes at least the refusal window, plus one process-table walk.
    pub async fn remove_socket(&self) -> SocketRemoval {
        match self.listed_file() {
            Ok(true) => {}
            Ok(false) => return SocketRemoval::Replaced,
            Err(outcome) => return outcome,
        }
        match probe(&self.socket, self.deadlines).await {
            Probe::Missing => return SocketRemoval::AlreadyGone,
            Probe::Answers => return SocketRemoval::StillServed { peer: None },
            Probe::Unsettled(why) => return SocketRemoval::Unconfirmed(why),
            Probe::Refused => {}
        }
        match find_socket_process(&self.socket, PROCESS_SEARCH_DEADLINE).await {
            Ok(None) => {}
            Ok(Some(process)) => {
                return SocketRemoval::StillServed {
                    peer: Some(process),
                };
            }
            Err(why) => return SocketRemoval::Unconfirmed(why),
        }
        match self.listed_file() {
            Ok(true) => {}
            Ok(false) => return SocketRemoval::Replaced,
            Err(outcome) => return outcome,
        }
        match fs::remove_file(&self.socket) {
            Ok(()) => SocketRemoval::Removed,
            Err(error) if error.kind() == io::ErrorKind::NotFound => SocketRemoval::AlreadyGone,
            Err(error) => SocketRemoval::Unconfirmed(format!("removing the socket: {error}")),
        }
    }

    /// Whether the path still holds a socket of the identity the scan
    /// listed. A missing path or an unreadable one is the outcome to return.
    fn listed_file(&self) -> Result<bool, SocketRemoval> {
        match fs::symlink_metadata(&self.socket) {
            Ok(meta) => Ok(meta.file_type().is_socket() && FileId::of(&meta) == self.file),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                Err(SocketRemoval::AlreadyGone)
            }
            Err(error) => Err(SocketRemoval::Unconfirmed(format!(
                "reading the socket file: {error}"
            ))),
        }
    }
}

/// Connect to `socket` without saying anything, until either something
/// other than a refusal happens or refusals have lasted the refusal window.
/// A connection that is accepted is closed at once; a host closes a
/// connection that sends no `Hello` and keeps serving its own client.
async fn probe(socket: &Path, deadlines: ClientDeadlines) -> Probe {
    let since = tokio::time::Instant::now();
    loop {
        // tokio's connect never blocks in the kernel; the timeout only
        // bounds an implementation that would.
        match tokio::time::timeout(deadlines.connect, tokio::net::UnixStream::connect(socket)).await
        {
            Err(_) => return Probe::Unsettled("connecting did not finish".into()),
            Ok(Ok(_stream)) => return Probe::Answers,
            Ok(Err(error)) => match error.kind() {
                io::ErrorKind::NotFound => return Probe::Missing,
                // A full Linux backlog: a listener that does not accept.
                io::ErrorKind::WouldBlock => return Probe::Answers,
                io::ErrorKind::ConnectionRefused => {}
                _ => return Probe::Unsettled(format!("connecting: {error}")),
            },
        }
        if since.elapsed() >= deadlines.refusal_window {
            return Probe::Refused;
        }
        tokio::time::sleep(deadlines.retry_interval).await;
    }
}

/// An entry of the socket directory that is not a host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedEntry {
    /// The entry's path; the directory's own path when reading the listing
    /// itself failed partway.
    pub path: PathBuf,
    /// Whether it is simply not a host, or could not be read.
    pub why: Skipped,
}

/// Why an entry was skipped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Skipped {
    /// Not a socket (a file, a directory, a symlink): no host made it.
    NotASocket,
    /// It, or the rest of the listing, could not be read, so a host may be
    /// hiding there: the scan is not complete.
    Unreadable(String),
}

/// What one socket name refers to in a registry ([`HostRegistry::by_name`]).
#[derive(Debug, Clone, Copy)]
pub enum NamedHost<'a> {
    /// A host attributed by its tag; [`HostRegistry::take`] adopts it.
    Live(&'a LiveHost),
    /// A host that may be running and must be left as it is.
    Unavailable(&'a UnavailableHost),
    /// A socket nothing listened on: the only class whose file may be
    /// removed, through [`GoneHost::remove_socket`].
    Gone(&'a GoneHost),
}

/// More than one live host claims the same agent launch: a session's agent
/// for one generation must be one host, so which is the real one is the
/// caller's to decide, not a lookup's to guess.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "{} terminal hosts claim generation {generation} of session {session_id}'s agent",
    names.len()
)]
pub struct DuplicateAgents {
    /// The session whose agent is claimed twice.
    pub session_id: String,
    /// The launch generation the hosts claim.
    pub generation: i64,
    /// The claiming hosts' socket names, sorted: what
    /// [`HostRegistry::by_name`] and [`HostRegistry::take`] act on.
    pub names: Vec<OsString>,
}

/// Several live hosts of one session carry the same tab id. A tab is one
/// terminal, so all but one of them are leftovers (a tab open is never
/// retried under the same id, so this should not happen); which one is the
/// tab is the caller's to decide.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateTab {
    /// The tab id they share.
    pub tab_id: String,
    /// Their socket names, sorted.
    pub names: Vec<OsString>,
}

/// A session's live tab hosts ([`HostRegistry::tabs`]).
#[derive(Debug)]
pub struct SessionTabs<'a> {
    /// Every live tab host of the session, in the order its tabs were
    /// opened. Hosts sharing a tab id are all here, each in its own place.
    pub hosts: Vec<&'a LiveHost>,
    /// The tab ids more than one of `hosts` carries, sorted by id; empty in
    /// the normal case.
    pub duplicates: Vec<DuplicateTab>,
}

// ---- The registry ------------------------------------------------------------------

/// What one scan of a state directory's socket directory found, with
/// lookups by what the hosts are for. A snapshot: nothing in it changes
/// after the scan, except that [`Self::take`] moves live hosts out.
///
/// Every list is sorted by socket name (skipped entries by path), so two
/// scans of the same directory list it the same way.
#[derive(Debug, Default)]
pub struct HostRegistry {
    live: Vec<LiveHost>,
    unavailable: Vec<UnavailableHost>,
    gone: Vec<GoneHost>,
    skipped: Vec<SkippedEntry>,
}

impl HostRegistry {
    /// Scan `<state_dir>/hosts`, classifying every socket in it except those
    /// [`ScanOptions::skip`] names.
    ///
    /// The caller must hold the state directory's lock and, inside a running
    /// supervisor, skip every host it drives or is starting (module docs, the
    /// preconditions): every other host found is taken over.
    ///
    /// A state directory without a socket directory has no hosts, and an
    /// empty registry is returned; nothing is created. A socket directory in
    /// a refused shape fails the scan ([`ScanError::HostsDir`]) without
    /// contacting anything. Removes nothing and shuts nothing down. See the
    /// module docs for what it does to the hosts it reaches and how long it
    /// takes.
    ///
    /// Dropping the future partway closes the connections made so far,
    /// which leaves those hosts as they were.
    pub async fn scan(state_dir: &Path, options: &ScanOptions) -> Result<Self, ScanError> {
        // Absolute, as the spawn makes it: a host's command line names its
        // socket absolute, and the process-table checks compare against it.
        let state_dir = std::path::absolute(state_dir).map_err(ScanError::StateDir)?;
        let Some(dir) = existing_hosts_dir(&state_dir)? else {
            return Ok(Self::default());
        };
        let (candidates, skipped) = list(&dir, &options.skip)?;
        let mut registry = Self {
            skipped,
            ..Self::default()
        };
        let mut pending = candidates.into_iter();
        let mut running = JoinSet::new();
        loop {
            while running.len() < SCAN_CONCURRENCY
                && let Some(candidate) = pending.next()
            {
                running.spawn(classify(candidate, options.deadlines));
            }
            match running.join_next().await {
                None => break,
                Some(Ok(class)) => registry.add(class),
                // The tasks are this scan's own and never aborted; a panic
                // in one is a bug, surfaced where the scan was called.
                Some(Err(error)) => std::panic::resume_unwind(error.into_panic()),
            }
        }
        registry.live.sort_by(|a, b| a.name.cmp(&b.name));
        registry.unavailable.sort_by(|a, b| a.name.cmp(&b.name));
        registry.gone.sort_by(|a, b| a.name.cmp(&b.name));
        registry.skipped.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(registry)
    }

    fn add(&mut self, class: Class) {
        match class {
            Class::Live(host) => self.live.push(*host),
            Class::Unavailable(host) => self.unavailable.push(host),
            Class::Gone(host) => self.gone.push(host),
        }
    }

    /// The live hosts not yet taken.
    pub fn live(&self) -> &[LiveHost] {
        &self.live
    }

    /// The hosts that cannot be used or attributed.
    pub fn unavailable(&self) -> &[UnavailableHost] {
        &self.unavailable
    }

    /// The sockets nothing listens on.
    pub fn gone(&self) -> &[GoneHost] {
        &self.gone
    }

    /// The entries that are not hosts.
    pub fn skipped(&self) -> &[SkippedEntry] {
        &self.skipped
    }

    /// Whether every socket the scan contacted was attributed: no host is
    /// unavailable and no entry was unreadable. Only then does a lookup that
    /// finds nothing mean the session has no host here (sockets in
    /// [`ScanOptions::skip`] are the caller's own and do not count).
    pub fn is_complete(&self) -> bool {
        self.unavailable.is_empty()
            && !self
                .skipped
                .iter()
                .any(|entry| matches!(entry.why, Skipped::Unreadable(_)))
    }

    /// The live host of `session_id`'s agent for launch `generation`.
    ///
    /// `Ok(None)` when no live host claims it, which proves nothing about
    /// unavailable hosts ([`Self::is_complete`]). Several claiming it is an
    /// error the caller resolves.
    pub fn agent(
        &self,
        session_id: &str,
        generation: i64,
    ) -> Result<Option<&LiveHost>, DuplicateAgents> {
        let claims: Vec<&LiveHost> = self
            .of_session(session_id)
            .into_iter()
            .filter(|host| host.tag.role == HostRole::Agent { generation })
            .collect();
        match claims.as_slice() {
            [] => Ok(None),
            [one] => Ok(Some(one)),
            several => Err(DuplicateAgents {
                session_id: session_id.to_owned(),
                generation,
                // Already in name order.
                names: several.iter().map(|host| host.name.clone()).collect(),
            }),
        }
    }

    /// The live tab hosts of `session_id`, in the order the session's tabs
    /// were opened: by the tag's creation stamp, then (for equal stamps) by
    /// tab id and socket name, so the order never depends on the scan. Tab
    /// ids carried by more than one host are flagged in
    /// [`SessionTabs::duplicates`] rather than resolved.
    pub fn tabs(&self, session_id: &str) -> SessionTabs<'_> {
        let mut tabs: Vec<(&str, &LiveHost)> = self
            .of_session(session_id)
            .into_iter()
            .filter_map(|host| match &host.tag.role {
                HostRole::Tab { tab_id, .. } => Some((tab_id.as_str(), host)),
                HostRole::Agent { .. } => None,
            })
            .collect();
        tabs.sort_by(|(a_id, a), (b_id, b)| {
            (a.tag.created_ns, *a_id, &a.name).cmp(&(b.tag.created_ns, *b_id, &b.name))
        });
        let mut by_id: std::collections::BTreeMap<&str, Vec<OsString>> =
            std::collections::BTreeMap::new();
        for (tab_id, host) in &tabs {
            by_id.entry(tab_id).or_default().push(host.name.clone());
        }
        let duplicates = by_id
            .into_iter()
            .filter(|(_, names)| names.len() > 1)
            .map(|(tab_id, mut names)| {
                names.sort();
                DuplicateTab {
                    tab_id: tab_id.to_owned(),
                    names,
                }
            })
            .collect();
        SessionTabs {
            hosts: tabs.into_iter().map(|(_, host)| host).collect(),
            duplicates,
        }
    }

    /// Every live host of `session_id`, agents of every generation and tabs,
    /// in socket name order.
    pub fn of_session(&self, session_id: &str) -> Vec<&LiveHost> {
        self.live
            .iter()
            .filter(|host| host.tag.session_id == session_id)
            .collect()
    }

    /// What the socket called `name` turned out to be. `None` for a name the
    /// scan did not find, skipped, or one already taken.
    pub fn by_name(&self, name: &OsStr) -> Option<NamedHost<'_>> {
        if let Some(host) = self.live.iter().find(|host| host.name == name) {
            return Some(NamedHost::Live(host));
        }
        if let Some(host) = self.unavailable.iter().find(|host| host.name == name) {
            return Some(NamedHost::Unavailable(host));
        }
        self.gone
            .iter()
            .find(|host| host.name == name)
            .map(NamedHost::Gone)
    }

    /// Move the live host called `name` out of the registry, with its
    /// connection, for a caller that adopts or ends it. Later lookups no
    /// longer see it.
    pub fn take(&mut self, name: &OsStr) -> Option<LiveHost> {
        let index = self.live.iter().position(|host| host.name == name)?;
        Some(self.live.remove(index))
    }
}

// ---- Scanning ---------------------------------------------------------------------

/// A socket to classify.
struct Candidate {
    name: OsString,
    socket: PathBuf,
    file: FileId,
}

/// One socket's class. A live host is boxed only because its connection
/// makes it several times larger than the other classes.
enum Class {
    Live(Box<LiveHost>),
    Unavailable(UnavailableHost),
    Gone(GoneHost),
}

/// The sockets of `dir` to classify, and the entries that are not hosts.
///
/// Each entry is looked at without following symlinks. An entry that
/// vanishes between the listing and the look ended meanwhile (a host
/// removes its socket as it ends) and is left out. Reading stops at the
/// first failure of the listing itself, which is recorded as unreadable
/// rather than failing the scan: the hosts already listed are still worth
/// classifying. Synchronous: the directory holds one entry per terminal.
fn list(
    dir: &Path,
    skip: &HashSet<OsString>,
) -> Result<(Vec<Candidate>, Vec<SkippedEntry>), ScanError> {
    let entries = fs::read_dir(dir).map_err(|source| ScanError::List {
        path: dir.to_path_buf(),
        source,
    })?;
    let mut candidates = Vec::new();
    let mut skipped = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                skipped.push(SkippedEntry {
                    path: dir.to_path_buf(),
                    why: Skipped::Unreadable(format!("listing the directory: {error}")),
                });
                break;
            }
        };
        let name = entry.file_name();
        if skip.contains(&name) {
            continue;
        }
        let path = dir.join(&name);
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => skipped.push(SkippedEntry {
                path,
                why: Skipped::Unreadable(error.to_string()),
            }),
            Ok(meta) if !meta.file_type().is_socket() => skipped.push(SkippedEntry {
                path,
                why: Skipped::NotASocket,
            }),
            Ok(meta) => candidates.push(Candidate {
                name,
                socket: path,
                file: FileId::of(&meta),
            }),
        }
    }
    Ok((candidates, skipped))
}

/// Connect to one socket, ask for its `Info` and read its tag.
async fn classify(candidate: Candidate, deadlines: ClientDeadlines) -> Class {
    let Candidate { name, socket, file } = candidate;
    let unavailable = |name, socket, peer, why| {
        Class::Unavailable(UnavailableHost {
            name,
            socket,
            peer,
            why,
        })
    };
    let mut connection = match HostConnection::connect_with(&socket, deadlines).await {
        Ok(connection) => connection,
        Err(ConnectError::Gone(why)) => {
            return Class::Gone(GoneHost {
                name,
                socket,
                why,
                file,
                deadlines,
            });
        }
        Err(ConnectError::Busy { peer }) => {
            return unavailable(name, socket, peer, WhyUnavailable::Busy);
        }
        Err(ConnectError::Unavailable { reason, peer }) => {
            return unavailable(name, socket, peer, WhyUnavailable::Connect(reason));
        }
    };
    let peer = connection.peer();
    let info = match connection.info().await {
        Ok(info) => info,
        Err(error) => return unavailable(name, socket, peer, WhyUnavailable::Info(error)),
    };
    match HostTag::decode(&info.tag) {
        Ok(tag) => Class::Live(Box::new(LiveHost {
            name,
            socket,
            tag,
            info,
            connection,
        })),
        // The connection closes here: an unreadable host is left alone.
        Err(tag) => unavailable(name, socket, peer, WhyUnavailable::Tag { tag, info }),
    }
}

#[cfg(test)]
mod tests;
