//! Discovery against stand-in hosts: listeners in this test process, one
//! per socket of a real socket directory, each answering the way one class
//! of host does (a host whose tag is one of the golden fixtures, a newer
//! release's tag, another protocol version, silence, busy). Real hosts,
//! started from the built `farhelm`, are in
//! `crates/farhelm/tests/terminal_host_registry.rs`.
//!
//! No stand-in forks, so these run as ordinary unit tests. Every stand-in
//! is served by this process, so every peer a scan names is this process.

use super::*;
use crate::hosts::HostName;
use crate::hosts::dir::ensure_hosts_dir;
use std::os::unix::fs::PermissionsExt as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use terminal_host::protocol::{
    ClientMessage, HostMessage, MAX_FRAME, Position, Size, decode_client, encode_host, split_frame,
};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{UnixListener, UnixStream};

/// Short deadlines, so every class arrives in about a second.
fn options() -> ScanOptions {
    ScanOptions {
        deadlines: ClientDeadlines {
            connect: Duration::from_secs(1),
            refusal_window: Duration::from_secs(1),
            retry_interval: Duration::from_millis(50),
            reply: Duration::from_secs(1),
            shutdown: Duration::from_secs(5),
        },
        skip: HashSet::new(),
        only: None,
    }
}

/// A golden version 1 tag's bytes (`tests/fixtures/host-tag/v1/<name>.hex`).
/// Discovery reads tags that running hosts carry for good, so the stand-ins
/// serve exactly those bytes rather than what this build's encoder writes.
fn golden_tag(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/host-tag/v1")
        .join(format!("{name}.hex"));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("reading {}: {error}", path.display()))
        .split_whitespace()
        .map(|pair| u8::from_str_radix(pair, 16).unwrap())
        .collect()
}

/// The session the golden `agent`, `tab-scoped` and `tab-unscoped` tags name.
const GOLDEN_SESSION: &str = "0f8e2b9c-4c1d-4e7a-9a53-7d2f61c0b8e4";

/// An `Info` carrying `tag`, told apart by `host_pid`.
fn info_with(host_pid: u32, tag: Vec<u8>) -> Info {
    let size = Size { cols: 80, rows: 24 };
    Info {
        host_pid,
        child_pid: host_pid + 1,
        child_exit: None,
        size,
        initial_size: size,
        snapshot: Position::default(),
        tail_start: 0,
        end: 0,
        resizes: 0,
        gap: false,
        delivered_end: 0,
        tag,
    }
}

/// How a stand-in answers.
#[derive(Clone)]
enum Answer {
    /// `Welcome` for version 1, then `info` to every `Info`.
    Host(Info),
    /// `Welcome` for version 1, then silence.
    MuteInfo,
    /// `Welcome` for another protocol version.
    Version(u32),
    /// `Busy`, to every connection.
    Busy,
    /// Nothing at all.
    Silent,
}

/// A state directory with a real socket directory, where stand-ins and
/// entries are placed by name.
struct Dir {
    state: tempfile::TempDir,
    hosts: PathBuf,
    /// The stand-ins' tasks, ended when the test ends.
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl Dir {
    fn new() -> Dir {
        let state = tempfile::tempdir().unwrap();
        let hosts = ensure_hosts_dir(state.path()).unwrap();
        Dir {
            state,
            hosts,
            tasks: Vec::new(),
        }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.hosts.join(name)
    }

    /// A stand-in at `name` answering with `answer`; the count is the
    /// connections it accepted.
    fn stand_in(&mut self, name: &str, answer: Answer) -> Arc<AtomicUsize> {
        let listener = UnixListener::bind(self.path(name)).unwrap();
        let accepted = Arc::new(AtomicUsize::new(0));
        let counted = accepted.clone();
        self.tasks.push(tokio::spawn(async move {
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                counted.fetch_add(1, Ordering::SeqCst);
                tokio::spawn(serve(stream, answer.clone()));
            }
        }));
        accepted
    }

    /// A socket file nothing listens on, what a killed host leaves, under a
    /// fresh random name, which this returns. Random because the check that
    /// a gone socket has no process matches any process naming a socket of
    /// the same file name in any directory (`hosts::process::socket_process`):
    /// a fixed name could be matched by a run of the same test in another
    /// checkout at the same moment.
    fn stale(&self) -> String {
        let name = HostName::random().unwrap().to_string();
        drop(std::os::unix::net::UnixListener::bind(self.path(&name)).unwrap());
        assert!(
            self.path(&name).exists(),
            "premise: the stale socket file stays"
        );
        name
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

/// One stand-in connection: read `Hello`, answer as `answer` says, and keep
/// the connection until the client closes it.
async fn serve(mut stream: UnixStream, answer: Answer) {
    let mut buf = Vec::new();
    let Some(ClientMessage::Hello { .. }) = read_frame(&mut stream, &mut buf).await else {
        return;
    };
    let reply = |message: HostMessage| encode_host(&message);
    let info = match answer {
        Answer::Silent => None,
        Answer::Busy => {
            let _ = stream.write_all(&reply(HostMessage::Busy)).await;
            return;
        }
        Answer::Version(version) => {
            let _ = stream
                .write_all(&reply(HostMessage::Welcome { version }))
                .await;
            None
        }
        Answer::MuteInfo => {
            let _ = stream
                .write_all(&reply(HostMessage::Welcome { version: 1 }))
                .await;
            None
        }
        Answer::Host(info) => {
            let _ = stream
                .write_all(&reply(HostMessage::Welcome { version: 1 }))
                .await;
            Some(info)
        }
    };
    while let Some(message) = read_frame(&mut stream, &mut buf).await {
        if let (ClientMessage::Info, Some(info)) = (message, &info) {
            let _ = stream
                .write_all(&reply(HostMessage::InfoReply(info.clone())))
                .await;
        }
    }
}

/// The next client frame, or `None` once the client closed.
async fn read_frame(stream: &mut UnixStream, buf: &mut Vec<u8>) -> Option<ClientMessage> {
    loop {
        if let Some(frame) = split_frame(buf, MAX_FRAME).expect("a well-formed frame") {
            let consumed = frame.consumed;
            let message = decode_client(frame, 1).expect("a well-formed message");
            buf.drain(..consumed);
            return Some(message);
        }
        if stream.read_buf(buf).await.ok()? == 0 {
            return None;
        }
    }
}

fn names<'a>(names: impl IntoIterator<Item = &'a OsString>) -> Vec<String> {
    names
        .into_iter()
        .map(|name| name.to_string_lossy().into_owned())
        .collect()
}

/// The unavailable host called `name`, failing the test when there is none.
fn unavailable<'a>(registry: &'a HostRegistry, name: &str) -> &'a UnavailableHost {
    match registry.by_name(OsStr::new(name)) {
        Some(NamedHost::Unavailable(host)) => host,
        other => panic!("{name}: expected an unavailable host, got {other:?}"),
    }
}

// ---- The mixed directory ------------------------------------------------------------

/// A directory holding every class at once is sorted into live,
/// unavailable, gone and skipped, and lookups find live hosts by what their
/// tags say.
///
/// Why: this is the scan a restarted supervisor relies on to find its
/// terminals again (by tag, never by name), and the classes are what keep
/// it from mistaking a host it cannot read for one that is gone: a
/// downgraded supervisor that treated a newer release's host as gone or as
/// an orphan would lose that terminal for good. Spec: in one socket
/// directory, stand-ins serving the golden `agent`, `tab-scoped`,
/// `tab-unscoped` and `agent-negative-generation` tags are live, with each
/// tag decoded and the stand-in's `Info` kept; a stand-in whose tag has
/// version 2 is unavailable for that version with its `Info` kept, one with
/// a foreign tag for not being Farhelm's, one welcoming with protocol 2 for
/// that version, a silent one for timing out, one that never answers
/// `Info` for that, and a busy one as busy, each naming this process as
/// the peer; a stale socket file is gone (refused), and a regular file and
/// a directory are skipped as not sockets. The agent lookup finds the
/// golden agent by session and generation 7 and nothing for generation 8
/// or another session; the tab lookup gives the two tabs in creation stamp
/// order (the scoped tab's stamp is the smaller), though their names sort
/// the other way; the session has three live hosts; the scan is not
/// complete; and the scan removed nothing (every socket file is still
/// there).
#[farhelm_testtrace::test]
async fn a_mixed_directory_is_sorted_into_its_classes() {
    let mut dir = Dir::new();
    dir.stand_in("agent", Answer::Host(info_with(100, golden_tag("agent"))));
    // Named so that name order is the reverse of creation order.
    dir.stand_in(
        "tab-1",
        Answer::Host(info_with(101, golden_tag("tab-unscoped"))),
    );
    dir.stand_in(
        "tab-2",
        Answer::Host(info_with(102, golden_tag("tab-scoped"))),
    );
    dir.stand_in(
        "negative",
        Answer::Host(info_with(103, golden_tag("agent-negative-generation"))),
    );
    let mut newer = golden_tag("agent");
    newer[4..6].copy_from_slice(&2u16.to_be_bytes());
    dir.stand_in("newer-tag", Answer::Host(info_with(104, newer)));
    dir.stand_in(
        "foreign",
        Answer::Host(info_with(105, b"not ours".to_vec())),
    );
    dir.stand_in("protocol-2", Answer::Version(2));
    dir.stand_in("silent", Answer::Silent);
    dir.stand_in("mute", Answer::MuteInfo);
    dir.stand_in("busy", Answer::Busy);
    let stale = dir.stale();
    std::fs::write(dir.path("file"), b"").unwrap();
    std::fs::create_dir(dir.path("subdir")).unwrap();

    let registry = HostRegistry::scan(dir.state.path(), &options())
        .await
        .unwrap();

    assert_eq!(
        names(registry.live().iter().map(|host| &host.name)),
        ["agent", "negative", "tab-1", "tab-2"]
    );
    let agent = registry
        .agent(GOLDEN_SESSION, 7)
        .unwrap()
        .expect("the golden agent");
    assert_eq!(agent.name, "agent");
    assert_eq!(agent.socket, dir.path("agent"));
    assert_eq!(agent.info.host_pid, 100);
    assert_eq!(agent.peer().map(|peer| peer.pid), Some(std::process::id()));
    assert!(registry.agent(GOLDEN_SESSION, 8).unwrap().is_none());
    assert!(registry.agent("another session", 7).unwrap().is_none());
    assert_eq!(
        registry
            .agent("s", -2)
            .unwrap()
            .map(|host| host.name.clone()),
        Some("negative".into())
    );
    let tabs = registry.tabs(GOLDEN_SESSION);
    assert!(tabs.duplicates.is_empty());
    let tabs: Vec<(&str, &HostRole)> = tabs
        .hosts
        .into_iter()
        .map(|host| (host.name.to_str().unwrap(), &host.tag.role))
        .collect();
    assert_eq!(
        tabs,
        [
            (
                "tab-2",
                &HostRole::Tab {
                    tab_id: "6b1c0d3e-2f4a-4b5c-8d6e-7f8091a2b3c4".into(),
                    scoped: true
                }
            ),
            (
                "tab-1",
                &HostRole::Tab {
                    tab_id: "t".into(),
                    scoped: false
                }
            ),
        ]
    );
    assert_eq!(registry.of_session(GOLDEN_SESSION).len(), 3);

    assert_eq!(
        names(registry.unavailable().iter().map(|host| &host.name)),
        [
            "busy",
            "foreign",
            "mute",
            "newer-tag",
            "protocol-2",
            "silent"
        ]
    );
    for host in registry.unavailable() {
        assert_eq!(
            host.peer.map(|peer| peer.pid),
            Some(std::process::id()),
            "{:?} names its peer",
            host.name
        );
    }
    match &unavailable(&registry, "newer-tag").why {
        WhyUnavailable::Tag { tag, info } => {
            assert_eq!(*tag, UnreadableTag::UnknownVersion(2));
            assert_eq!(info.host_pid, 104);
        }
        other => panic!("newer-tag: {other:?}"),
    }
    assert!(matches!(
        &unavailable(&registry, "foreign").why,
        WhyUnavailable::Tag {
            tag: UnreadableTag::NotFarhelm,
            ..
        }
    ));
    assert_eq!(
        unavailable(&registry, "protocol-2").why,
        WhyUnavailable::Connect(Unavailable::UnsupportedVersion(2))
    );
    assert_eq!(
        unavailable(&registry, "silent").why,
        WhyUnavailable::Connect(Unavailable::Timeout)
    );
    assert_eq!(
        unavailable(&registry, "mute").why,
        WhyUnavailable::Info(ClientError::Timeout)
    );
    assert_eq!(unavailable(&registry, "busy").why, WhyUnavailable::Busy);

    assert_eq!(
        names(registry.gone().iter().map(|host| &host.name)),
        [stale.as_str()]
    );
    assert_eq!(registry.gone()[0].why, Gone::Refused);
    assert_eq!(
        registry.skipped(),
        [
            SkippedEntry {
                path: dir.path("file"),
                why: Skipped::NotASocket
            },
            SkippedEntry {
                path: dir.path("subdir"),
                why: Skipped::NotASocket
            },
        ]
    );
    assert!(!registry.is_complete());

    for name in [
        "agent",
        "tab-1",
        "tab-2",
        "negative",
        "newer-tag",
        "foreign",
        "protocol-2",
        "silent",
        "mute",
        "busy",
        stale.as_str(),
    ] {
        assert!(dir.path(name).exists(), "the scan removed {name}");
    }
}

// ---- The directory itself -----------------------------------------------------------

/// A state directory without a socket directory has no hosts, and the scan
/// creates none.
///
/// Why: discovery runs in every state directory, most of which never ran a
/// host; that must be an empty result, not an error, and must leave no
/// trace. Spec: scanning a fresh state directory gives an empty, complete
/// registry, and `hosts` still does not exist.
#[farhelm_testtrace::test]
async fn a_state_directory_without_hosts_has_none() {
    let state = tempfile::tempdir().unwrap();
    let registry = HostRegistry::scan(state.path(), &options()).await.unwrap();
    assert!(registry.live().is_empty());
    assert!(registry.unavailable().is_empty());
    assert!(registry.gone().is_empty());
    assert!(registry.skipped().is_empty());
    assert!(registry.is_complete());
    assert!(!state.path().join("hosts").exists());
}

/// A socket directory whose permissions are wrong fails the scan without
/// contacting anything in it.
///
/// Why: a socket in a directory others can reach may be a planted one, and
/// a supervisor that adopted it would read a stranger's tag as its own
/// session's terminal. Spec: with the directory at mode 0755 and a stand-in
/// host inside, the scan fails with `HostsDir(WrongMode)` and the stand-in
/// accepted no connection.
#[farhelm_testtrace::test]
async fn a_socket_directory_with_wrong_permissions_fails_the_scan() {
    let mut dir = Dir::new();
    let accepted = dir.stand_in("agent", Answer::Host(info_with(1, golden_tag("agent"))));
    std::fs::set_permissions(&dir.hosts, std::fs::Permissions::from_mode(0o755)).unwrap();
    let result = HostRegistry::scan(dir.state.path(), &options()).await;
    assert!(
        matches!(
            result,
            Err(ScanError::HostsDir(HostsDirError::WrongMode {
                mode: 0o755,
                ..
            }))
        ),
        "{result:?}"
    );
    assert_eq!(accepted.load(Ordering::SeqCst), 0);
}

// ---- Lookups and the caller's own hosts ---------------------------------------------

/// Two live hosts claiming one agent launch make the agent lookup an error
/// naming both; another generation of the same session is still found.
///
/// Why: a session's agent for one generation is one terminal, and a lookup
/// that picked one of two would adopt one and leave the other running
/// unseen; which one is real is for the caller's reconciliation to decide.
/// Spec: with stand-ins `a` and `b` both tagged as generation 5 of a
/// session and `c` as generation 6, the lookup of generation 5 gives
/// `DuplicateAgents` naming `a` and `b` in that order, and the lookup of
/// generation 6 gives `c`.
#[farhelm_testtrace::test]
async fn two_hosts_claiming_one_agent_launch_are_an_error() {
    let mut dir = Dir::new();
    let tag = |generation| {
        HostTag {
            session_id: "s1".into(),
            created_ns: 1,
            role: HostRole::Agent { generation },
        }
        .encode()
        .unwrap()
    };
    dir.stand_in("b", Answer::Host(info_with(2, tag(5))));
    dir.stand_in("a", Answer::Host(info_with(1, tag(5))));
    dir.stand_in("c", Answer::Host(info_with(3, tag(6))));
    let registry = HostRegistry::scan(dir.state.path(), &options())
        .await
        .unwrap();
    assert_eq!(
        registry.agent("s1", 5).unwrap_err(),
        DuplicateAgents {
            session_id: "s1".into(),
            generation: 5,
            names: vec!["a".into(), "b".into()],
        }
    );
    assert_eq!(
        registry
            .agent("s1", 6)
            .unwrap()
            .map(|host| host.info.host_pid),
        Some(3)
    );
}

/// A session's tabs come in creation-stamp order, whatever their tab ids
/// and socket names say, and a tab id carried by two hosts is flagged.
///
/// Why: tabs have no stored record, so the stamp in their tags is the only
/// record of the order the user opened them in; an order taken from tab ids
/// (random UUIDs in practice) or socket names would shuffle the tab strip
/// after every restart. A duplicated tab id means a leftover host that would
/// otherwise show as a second copy of the tab with nothing said. Spec: with
/// tab stand-ins of one session named `n1` (tab `a`, stamp 300), `n2` (tab
/// `c`, stamp 200), `n3` (tab `b`, stamp 100) and `n0` (tab `b` again,
/// stamp 400), and an agent of the session, the tab lookup gives `n3`,
/// `n2`, `n1`, `n0` (stamp order, which is neither tab-id nor name order),
/// and flags tab `b` as carried by `n0` and `n3`.
#[farhelm_testtrace::test]
async fn tabs_come_in_creation_order_and_duplicate_ids_are_flagged() {
    let mut dir = Dir::new();
    let tab = |tab_id: &str, created_ns| {
        HostTag {
            session_id: "s1".into(),
            created_ns,
            role: HostRole::Tab {
                tab_id: tab_id.into(),
                scoped: false,
            },
        }
        .encode()
        .unwrap()
    };
    dir.stand_in("n1", Answer::Host(info_with(1, tab("a", 300))));
    dir.stand_in("n2", Answer::Host(info_with(2, tab("c", 200))));
    dir.stand_in("n3", Answer::Host(info_with(3, tab("b", 100))));
    dir.stand_in("n0", Answer::Host(info_with(4, tab("b", 400))));
    let agent = HostTag {
        session_id: "s1".into(),
        created_ns: 0,
        role: HostRole::Agent { generation: 1 },
    };
    dir.stand_in("agent", Answer::Host(info_with(5, agent.encode().unwrap())));
    let registry = HostRegistry::scan(dir.state.path(), &options())
        .await
        .unwrap();
    let tabs = registry.tabs("s1");
    assert_eq!(
        names(tabs.hosts.iter().map(|host| &host.name)),
        ["n3", "n2", "n1", "n0"]
    );
    assert_eq!(
        tabs.duplicates,
        [DuplicateTab {
            tab_id: "b".into(),
            names: vec!["n0".into(), "n3".into()],
        }]
    );
}

/// Sockets the caller names as its own are never contacted and appear in
/// no class.
///
/// Why: a scan takes over every host it reaches, so scanning a host the
/// supervisor already drives would cut off its own runtime's connection;
/// the skip list is how a running supervisor scans without doing that.
/// Spec: with stand-ins `owned` and `other`, a scan skipping `owned` finds
/// `other` live, finds nothing by the name `owned`, is complete, and
/// `owned` accepted no connection while `other` accepted one.
#[farhelm_testtrace::test]
async fn sockets_the_caller_owns_are_not_contacted() {
    let mut dir = Dir::new();
    let owned = dir.stand_in("owned", Answer::Host(info_with(1, golden_tag("agent"))));
    let other = dir.stand_in(
        "other",
        Answer::Host(info_with(2, golden_tag("tab-scoped"))),
    );
    let mut options = options();
    options.skip.insert("owned".into());
    let registry = HostRegistry::scan(dir.state.path(), &options)
        .await
        .unwrap();
    assert_eq!(
        names(registry.live().iter().map(|host| &host.name)),
        ["other"]
    );
    assert!(registry.by_name(OsStr::new("owned")).is_none());
    assert!(registry.is_complete());
    assert_eq!(owned.load(Ordering::SeqCst), 0);
    assert_eq!(other.load(Ordering::SeqCst), 1);
}

/// A scan restricted to some socket names contacts only those.
///
/// Why: a supervisor adopting particular hosts again (ones that were
/// unavailable) must not take over, or pay the deadlines of, every other
/// socket in the directory each time it retries. Spec: with stand-ins
/// `wanted` and `other`, a scan restricted to `wanted` finds it live and
/// nothing by the name `other`, which accepted no connection.
#[farhelm_testtrace::test]
async fn a_scan_restricted_to_some_names_contacts_only_those() {
    let mut dir = Dir::new();
    let wanted = dir.stand_in("wanted", Answer::Host(info_with(1, golden_tag("agent"))));
    let other = dir.stand_in(
        "other",
        Answer::Host(info_with(2, golden_tag("tab-scoped"))),
    );
    let mut options = options();
    options.only = Some(HashSet::from([OsString::from("wanted")]));
    let registry = HostRegistry::scan(dir.state.path(), &options)
        .await
        .unwrap();
    assert_eq!(
        names(registry.live().iter().map(|host| &host.name)),
        ["wanted"]
    );
    assert!(registry.by_name(OsStr::new("other")).is_none());
    assert_eq!(wanted.load(Ordering::SeqCst), 1);
    assert_eq!(other.load(Ordering::SeqCst), 0);
}

/// Taking a live host hands over the scan's connection, still usable, and
/// removes the host from later lookups.
///
/// Why: the scan's connection is the host's owning connection; a caller
/// that reconnected instead would take the host over from its own
/// registry, and one that could take a host twice would drive it from two
/// places. Spec: after a scan finding the golden agent, taking it by name
/// gives that host, whose connection answers a second `Info` with the
/// stand-in's; afterwards the agent lookup and the name lookup find
/// nothing, and a second take gives `None`.
#[farhelm_testtrace::test]
async fn taking_a_live_host_hands_over_its_connection() {
    let mut dir = Dir::new();
    let accepted = dir.stand_in("agent", Answer::Host(info_with(42, golden_tag("agent"))));
    let mut registry = HostRegistry::scan(dir.state.path(), &options())
        .await
        .unwrap();
    let mut host = registry.take(OsStr::new("agent")).expect("the live host");
    assert_eq!(host.connection.info().await.unwrap().host_pid, 42);
    assert_eq!(
        accepted.load(Ordering::SeqCst),
        1,
        "the connection was reused"
    );
    assert!(registry.agent(GOLDEN_SESSION, 7).unwrap().is_none());
    assert!(registry.by_name(OsStr::new("agent")).is_none());
    assert!(registry.take(OsStr::new("agent")).is_none());
}

// ---- Removing gone sockets ----------------------------------------------------------

/// A gone socket is removed when it is still gone; a missing one or another
/// file at its path is left alone.
///
/// Why: removing the sockets killed hosts leave is the one cleanup
/// discovery offers, and it must never touch what is at the path now unless
/// that is still the dead socket. Spec: of two stale sockets, the first is
/// removed (`Removed`, the file gone) and removing it again is
/// `AlreadyGone`; the second is replaced by a regular file before removal,
/// which then gives `Replaced` and leaves the file in place.
#[farhelm_testtrace::test]
async fn a_gone_socket_is_removed_while_it_is_still_gone() {
    let dir = Dir::new();
    let first_name = dir.stale();
    let second_name = dir.stale();
    let registry = HostRegistry::scan(dir.state.path(), &options())
        .await
        .unwrap();
    assert_eq!(registry.gone().len(), 2, "{:?}", registry.gone());
    let gone = |name: &str| {
        registry
            .gone()
            .iter()
            .find(|host| host.name == name)
            .expect("a gone socket by that name")
    };
    let (first, second) = (gone(&first_name), gone(&second_name));
    assert_eq!(first.remove_socket().await, SocketRemoval::Removed);
    assert!(!dir.path(&first_name).exists());
    assert_eq!(first.remove_socket().await, SocketRemoval::AlreadyGone);

    std::fs::remove_file(dir.path(&second_name)).unwrap();
    std::fs::write(dir.path(&second_name), b"").unwrap();
    assert_eq!(second.remove_socket().await, SocketRemoval::Replaced);
    assert!(dir.path(&second_name).exists(), "the file was removed");
}

/// A socket that listens at a gone host's path is never removed, even when
/// it has the very device and inode number the scan recorded.
///
/// Why: inode numbers are reused (ext4 hands a just-freed number to the
/// next socket bound at once), so a host's socket bound after the old file
/// was unlinked can look identical to the dead one by device and inode;
/// removing it would strand a live terminal. Removal must judge the socket
/// gone again, not trust the file's identity. Spec: after a scan judges a
/// stale socket gone, its file is unlinked and a listener bound at the
/// path; the listener accepts a connection (the premise). The identity the
/// scan recorded is then set to the listener's own file (what reuse
/// produces, made certain on every filesystem, APFS included, which does
/// not reuse numbers), and removal gives `StillServed` with no peer (the
/// listener answered). Where the filesystem did reuse the number by
/// itself, the scan's own record gives the same. The socket file is still
/// there and still accepts connections.
#[farhelm_testtrace::test]
async fn a_listening_socket_with_the_old_inode_is_never_removed() {
    let dir = Dir::new();
    let name = dir.stale();
    let path = dir.path(&name);
    let registry = HostRegistry::scan(dir.state.path(), &options())
        .await
        .unwrap();
    let [gone] = registry.gone() else {
        panic!("expected one gone socket, got {:?}", registry.gone());
    };

    std::fs::remove_file(&path).unwrap();
    let _listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
    assert!(
        std::os::unix::net::UnixStream::connect(&path).is_ok(),
        "premise: the replacement listens"
    );
    let now = FileId::of(&std::fs::symlink_metadata(&path).unwrap());
    let reused = GoneHost {
        file: now,
        ..gone.clone()
    };
    assert_eq!(
        reused.remove_socket().await,
        SocketRemoval::StillServed { peer: None }
    );
    if gone.file == now {
        assert_eq!(
            gone.remove_socket().await,
            SocketRemoval::StillServed { peer: None }
        );
    }
    assert!(path.exists(), "the live socket's file was removed");
    assert!(
        std::os::unix::net::UnixStream::connect(&path).is_ok(),
        "the live socket no longer accepts connections"
    );
}

/// A refusing socket that a live process names on its command line is
/// never removed.
///
/// Why: a host between its `bind` and its `listen`, or a stopped host whose
/// listen backlog filled, refuses connections exactly like a dead socket;
/// what tells it apart is its process, whose command line ends with
/// `--socket <that path>`. Spec: after a scan judges a stale socket gone, a
/// `/bin/sh` is started whose command line ends with `--socket <the
/// socket>`; removal then gives `StillServed` naming that process, and the
/// socket file is still there.
#[farhelm_testtrace::test]
async fn a_socket_a_process_names_is_never_removed() {
    let dir = Dir::new();
    dir.stale();
    let registry = HostRegistry::scan(dir.state.path(), &options())
        .await
        .unwrap();
    let [gone] = registry.gone() else {
        panic!("expected one gone socket, got {:?}", registry.gone());
    };
    // `sh -c <script> <$0> <$1>`: the shell keeps its whole argv, so its
    // command line ends with `--socket <path>`, as a host's does. The `:`
    // keeps the shell from replacing itself with `sleep`.
    let mut server = tokio::process::Command::new("/bin/sh")
        .arg("-c")
        .arg("sleep 30; :")
        .arg("--socket")
        .arg(&gone.socket)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let pid = server.id().expect("the stand-in runs");
    let outcome = gone.remove_socket().await;
    let _ = server.start_kill();
    let _ = tokio::time::timeout(Duration::from_secs(10), server.wait()).await;
    match outcome {
        SocketRemoval::StillServed {
            peer: Some(process),
        } => assert_eq!(process.pid, pid),
        other => panic!("expected the serving process, got {other:?}"),
    }
    assert!(gone.socket.exists(), "the socket file was removed");
}
