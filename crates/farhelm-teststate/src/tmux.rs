//! Bounded shutdown for a test-owned tmux server.
//!
//! Socket ownership authorizes both the normal `kill-server` command and any
//! stronger cleanup. Linux can bind that authority to the listening process
//! with `SO_PEERPIDFD`; only that socket-derived descriptor may be signalled.
//! Platforms and kernels without that facility retain bounded protocol
//! cleanup and report that forced termination was unavailable.

use crate::process::{CommandRunConfigError, CommandRunLimits, CommandRunOutcome, run_bounded};
use std::ffi::{CString, OsString};
use std::fs;
use std::io;
#[cfg(target_os = "linux")]
use std::io::Read;
use std::os::fd::AsRawFd;
#[cfg(target_os = "linux")]
use std::os::fd::{FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

const TOTAL_ALLOWANCE: Duration = Duration::from_secs(5);
const PROTOCOL_ALLOWANCE: Duration = Duration::from_secs(4);
const CLEANUP_RESERVE: Duration = Duration::from_millis(250);
#[cfg(target_os = "linux")]
const NORMAL_EXIT_GRACE: Duration = Duration::from_millis(50);
const OUTPUT_LIMIT: usize = 4 * 1024;
#[cfg(target_os = "linux")]
const FDINFO_LIMIT: usize = 4 * 1024;

/// One bounded shutdown attempt and the evidence needed to diagnose it.
///
/// The four fields deliberately do not collapse into one success bit. A
/// completed client does not prove its server exited, and unavailable fallback
/// authority remains visible even when the protocol command succeeds.
#[derive(Debug)]
pub struct TmuxShutdownOutcome {
    /// Bounded evidence from the normal `kill-server` client, if authorized.
    pub protocol: TmuxProtocolOutcome,
    /// Whether the socket supplied verified fallback authority.
    pub acquisition: TmuxPeerAcquisition,
    /// Whether descriptor-bound force termination was needed and accepted.
    pub fallback: TmuxFallback,
    /// The liveness fact actually observed through a verified descriptor.
    pub death: TmuxDeath,
}

/// Whether the normal tmux client ran under the shared shutdown deadline.
#[derive(Debug)]
pub enum TmuxProtocolOutcome {
    /// The bounded runner started or attempted to start the configured client.
    Attempted(CommandRunOutcome),
    /// Validation or the shared deadline prevented a client attempt.
    NotAttempted(TmuxProtocolUnavailable),
}

/// Why no normal protocol command was started.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TmuxProtocolUnavailable {
    /// Socket or executable validation did not authorize execution.
    NotAuthorized,
    /// Earlier work consumed the one shutdown allowance.
    DeadlineExpired,
    /// The bounded runner could not represent the remaining limits.
    InvalidLimits(CommandRunConfigError),
}

/// Whether a private socket supplied safe fallback authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TmuxPeerAcquisition {
    /// A Linux peer pidfd passed every ownership and identity check.
    Verified,
    /// The path was absent. Absence does not prove a former server exited.
    SocketAbsent,
    /// The socket path was not a private namespace this caller may control.
    Refused(TmuxPathRefusal),
    /// The caller did not supply one resolved executable file.
    ExecutableRefused,
    /// The platform or listener could not provide verified pidfd authority.
    Unavailable(TmuxPeerUnavailable),
}

/// Reasons a socket path cannot authorize either shutdown mechanism.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TmuxPathRefusal {
    /// The path depends on a caller-controlled current directory.
    Relative,
    /// A literal `.` or `..` would blur the direct-parent authority check.
    DotComponent,
    /// A C-string syscall would interpret only a prefix of the path.
    NulByte,
    /// The direct parent is absent or not a directory.
    ParentNotDirectory,
    /// The direct parent redirects namespace authority.
    ParentSymlink,
    /// The effective user does not own the direct parent.
    ParentWrongOwner,
    /// Group or other users can access the direct parent.
    ParentNotPrivate,
    /// The final component redirects to another filesystem object.
    SocketSymlink,
    /// The final component is not a Unix socket.
    SocketNotSocket,
    /// The effective user does not own the socket.
    SocketWrongOwner,
    /// Required direct-parent or final-component metadata was unreadable.
    MetadataUnavailable,
}

/// Bounded reasons Linux peer-descriptor acquisition did not succeed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TmuxPeerUnavailable {
    /// The one shutdown allowance expired during acquisition.
    DeadlineExpired,
    /// This platform has no socket-derived process descriptor facility.
    UnsupportedPlatform,
    /// The pathname cannot fit in one `sockaddr_un` without truncation.
    SocketNameTooLong,
    /// Creating the nonblocking Unix socket failed.
    SocketOpen(TmuxOsError),
    /// The single nonblocking connection could not complete immediately.
    ConnectWouldBlock,
    /// The single connection failed for another reason.
    Connect(TmuxOsError),
    /// The kernel explicitly lacks `SO_PEERPIDFD`.
    PeerDescriptorUnsupported,
    /// `SO_PEERPIDFD` failed without demonstrating unsupported coverage.
    PeerDescriptor(TmuxOsError),
    /// The kernel returned an invalid descriptor value or size.
    PeerDescriptorInvalid,
    /// The returned descriptor could not be made close-on-exec.
    PeerDescriptorFlags(TmuxOsError),
    /// The listener is this test process and may never be signalled.
    PeerIsCurrentProcess,
    /// The caller's identity in the mounted procfs namespace was unreadable.
    CurrentProcessReadlink(TmuxOsError),
    /// The bounded procfs self link was truncated or was not a positive PID.
    CurrentProcessMalformed,
    /// The descriptor's bounded fdinfo record could not be opened.
    PeerFdInfoOpen(TmuxOsError),
    /// Reading the bounded fdinfo prefix failed.
    PeerFdInfoRead(TmuxOsError),
    /// The fdinfo record exceeded the fixed acquisition cap.
    PeerFdInfoOverflow,
    /// The bounded fdinfo record contained no valid peer PID.
    PeerFdInfoMalformed,
    /// The peer executable's device/inode metadata was unavailable.
    PeerExecutableMetadata(TmuxOsError),
    /// The listener is not the executable the caller authorized.
    PeerExecutableMismatch,
}

/// A bounded operating-system error without paths, arguments, or environment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TmuxOsError {
    /// Portable error class returned by `std::io`.
    pub kind: std::io::ErrorKind,
    /// Platform errno when the operating system supplied one.
    pub raw_os_error: Option<i32>,
}

#[cfg(target_os = "linux")]
impl From<&io::Error> for TmuxOsError {
    fn from(error: &io::Error) -> Self {
        Self {
            kind: error.kind(),
            raw_os_error: error.raw_os_error(),
        }
    }
}

/// What happened after a verified peer survived normal protocol cleanup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TmuxFallback {
    /// No verified peer descriptor existed, so no signal was legal.
    Unavailable,
    /// The verified server died during normal protocol cleanup.
    NotNeeded,
    /// The kernel accepted SIGKILL through the verified peer descriptor.
    SignalAccepted,
    /// The kernel rejected the descriptor-bound signal request.
    SignalFailed(TmuxOsError),
    /// The shared deadline expired before a fallback decision was possible.
    DeadlineExpired,
}

/// What the shutdown attempt actually observed about server liveness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TmuxDeath {
    /// The retained peer descriptor reported that its process exited.
    Observed,
    /// No verified descriptor existed from which liveness could be observed.
    Unobserved,
    /// Polling failed before liveness could be determined.
    PollFailed(TmuxPollFailure),
    /// The observation budget elapsed without observing death.
    DeadlineExpired,
}

/// Why pidfd polling produced no liveness observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TmuxPollFailure {
    /// The poll syscall failed.
    System(TmuxOsError),
    /// Poll returned readiness that does not establish pidfd death.
    UnexpectedEvents(i16),
}

/// Shut down the tmux server listening at a caller-owned private socket.
///
/// `tmux_executable` must already be an absolute, resolved executable file.
/// Invalid socket authority refuses the client command as well as fallback;
/// executable identity alone never authorizes control of an arbitrary socket.
/// This function is synchronous so it remains usable from `Drop` while Tokio
/// is paused or unavailable. Kernel-stuck syscalls and scheduler stalls remain
/// outside the practical five-second allowance.
///
/// The caller must exclusively own waits for this process's protocol child:
/// no concurrent catch-all reaper or automatic child reaping may consume it.
/// Server pidfds do not remove that direct-child constraint. The prompt-spawn
/// assumption and full reaping limitations of [`run_bounded`] apply here too.
/// The caller also owns the private directory's contents for this attempt;
/// retaining its descriptor prevents ancestor renames from redirecting cleanup.
pub fn shutdown_tmux_server(socket: &Path, tmux_executable: &Path) -> TmuxShutdownOutcome {
    let deadline = match Instant::now().checked_add(TOTAL_ALLOWANCE) {
        Some(deadline) => deadline,
        None => {
            return expired(TmuxPeerAcquisition::Unavailable(
                TmuxPeerUnavailable::DeadlineExpired,
            ));
        }
    };
    let authority = match validate_socket(socket) {
        Ok(authority) => authority,
        Err(acquisition) => return refused(acquisition),
    };
    if Instant::now() >= deadline {
        return expired(TmuxPeerAcquisition::Unavailable(
            TmuxPeerUnavailable::DeadlineExpired,
        ));
    }
    let executable_identity = match validate_executable(tmux_executable) {
        Some(identity) => identity,
        None => return refused(TmuxPeerAcquisition::ExecutableRefused),
    };
    if Instant::now() >= deadline {
        return expired(TmuxPeerAcquisition::Unavailable(
            TmuxPeerUnavailable::DeadlineExpired,
        ));
    }

    shutdown_authorized(&authority, tmux_executable, executable_identity, deadline)
}

/// Every shutdown mechanism uses the same retained directory authority. The
/// caller's original pathname is no longer consulted after validation.
fn shutdown_authorized(
    authority: &SocketAuthority,
    tmux_executable: &Path,
    executable_identity: (u64, u64),
    deadline: Instant,
) -> TmuxShutdownOutcome {
    #[cfg(target_os = "linux")]
    {
        shutdown_linux(authority, tmux_executable, executable_identity, deadline)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = executable_identity;
        run_protocol_only(
            authority,
            tmux_executable,
            deadline,
            TmuxPeerAcquisition::Unavailable(TmuxPeerUnavailable::UnsupportedPlatform),
        )
    }
}

/// Refusal means no operation was authorized; it says nothing about whether a
/// process formerly associated with the path is alive or dead.
fn refused(acquisition: TmuxPeerAcquisition) -> TmuxShutdownOutcome {
    TmuxShutdownOutcome {
        protocol: TmuxProtocolOutcome::NotAttempted(TmuxProtocolUnavailable::NotAuthorized),
        acquisition,
        fallback: TmuxFallback::Unavailable,
        death: TmuxDeath::Unobserved,
    }
}

/// Expiration records that the shared allowance ran out without converting a
/// lack of liveness evidence into a claim that a process survived.
fn expired(acquisition: TmuxPeerAcquisition) -> TmuxShutdownOutcome {
    TmuxShutdownOutcome {
        protocol: TmuxProtocolOutcome::NotAttempted(TmuxProtocolUnavailable::DeadlineExpired),
        acquisition,
        fallback: TmuxFallback::DeadlineExpired,
        death: TmuxDeath::DeadlineExpired,
    }
}

/// Retain bounded normal cleanup when safe forced termination is unavailable.
/// A protocol result, including success, cannot establish server death without
/// the verified descriptor this path explicitly lacks.
fn run_protocol_only(
    socket: &SocketAuthority,
    executable: &Path,
    deadline: Instant,
    acquisition: TmuxPeerAcquisition,
) -> TmuxShutdownOutcome {
    TmuxShutdownOutcome {
        protocol: run_protocol(socket, executable, deadline),
        acquisition,
        fallback: TmuxFallback::Unavailable,
        death: TmuxDeath::Unobserved,
    }
}

/// A held private directory and one entry within it authorize cleanup even if
/// an ancestor is renamed. Other users cannot substitute the entry inside the
/// validated private directory; its owner must retain that lifecycle ownership.
struct SocketAuthority {
    directory: fs::File,
    name: OsString,
}

impl SocketAuthority {
    /// Linux resolves this path through our held descriptor, without looking
    /// up the caller's original ancestors again. The descriptor stays open
    /// until peer acquisition and the independently anchored client complete.
    #[cfg(target_os = "linux")]
    fn peer_path(&self) -> std::path::PathBuf {
        std::path::PathBuf::from(format!("/proc/self/fd/{}", self.directory.as_raw_fd()))
            .join(&self.name)
    }
}

/// Open and validate the directory itself, then inspect the socket relative
/// to that retained descriptor. Path metadata is used only to diagnose refusal;
/// it never authorizes a later lookup through mutable ancestors.
fn validate_socket(socket: &Path) -> Result<SocketAuthority, TmuxPeerAcquisition> {
    if !socket.is_absolute() {
        return Err(TmuxPeerAcquisition::Refused(TmuxPathRefusal::Relative));
    }
    if socket.as_os_str().as_bytes().contains(&0) {
        return Err(TmuxPeerAcquisition::Refused(TmuxPathRefusal::NulByte));
    }
    if has_literal_dot_component(socket) {
        return Err(TmuxPeerAcquisition::Refused(TmuxPathRefusal::DotComponent));
    }
    let Some(parent) = socket.parent() else {
        return Err(TmuxPeerAcquisition::Refused(
            TmuxPathRefusal::ParentNotDirectory,
        ));
    };
    // SAFETY: geteuid has no pointer or lifetime preconditions and only reads
    // the calling process's credential state.
    let owner = unsafe { libc::geteuid() };
    let parent_path_metadata = fs::symlink_metadata(parent)
        .map_err(|_| TmuxPeerAcquisition::Refused(TmuxPathRefusal::MetadataUnavailable))?;
    if parent_path_metadata.file_type().is_symlink() {
        return Err(TmuxPeerAcquisition::Refused(TmuxPathRefusal::ParentSymlink));
    }
    let directory = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(parent)
        .map_err(|_| TmuxPeerAcquisition::Refused(TmuxPathRefusal::MetadataUnavailable))?;
    let parent_metadata = directory
        .metadata()
        .map_err(|_| TmuxPeerAcquisition::Refused(TmuxPathRefusal::MetadataUnavailable))?;
    if !parent_metadata.is_dir() {
        return Err(TmuxPeerAcquisition::Refused(
            TmuxPathRefusal::ParentNotDirectory,
        ));
    }
    if parent_metadata.uid() != owner {
        return Err(TmuxPeerAcquisition::Refused(
            TmuxPathRefusal::ParentWrongOwner,
        ));
    }
    if parent_metadata.permissions().mode() & 0o077 != 0 {
        return Err(TmuxPeerAcquisition::Refused(
            TmuxPathRefusal::ParentNotPrivate,
        ));
    }
    let name = socket.file_name().ok_or(TmuxPeerAcquisition::Refused(
        TmuxPathRefusal::SocketNotSocket,
    ))?;
    let c_name = CString::new(name.as_bytes())
        .map_err(|_| TmuxPeerAcquisition::Refused(TmuxPathRefusal::NulByte))?;
    let mut metadata = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: directory is held open; c_name is a terminated single entry
    // name, and metadata is writable stat storage. Successful fstatat fully
    // initializes it. AT_SYMLINK_NOFOLLOW keeps the final entry authoritative.
    let result = unsafe {
        libc::fstatat(
            directory.as_raw_fd(),
            c_name.as_ptr(),
            metadata.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if result != 0 {
        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::NotFound {
            return Err(TmuxPeerAcquisition::SocketAbsent);
        }
        return Err(TmuxPeerAcquisition::Refused(
            TmuxPathRefusal::MetadataUnavailable,
        ));
    }
    // SAFETY: the successful fstatat above initialized this complete stat.
    let final_metadata = unsafe { metadata.assume_init() };
    if final_metadata.st_mode & libc::S_IFMT == libc::S_IFLNK {
        return Err(TmuxPeerAcquisition::Refused(TmuxPathRefusal::SocketSymlink));
    }
    if final_metadata.st_mode & libc::S_IFMT != libc::S_IFSOCK {
        return Err(TmuxPeerAcquisition::Refused(
            TmuxPathRefusal::SocketNotSocket,
        ));
    }
    if final_metadata.st_uid != owner {
        return Err(TmuxPeerAcquisition::Refused(
            TmuxPathRefusal::SocketWrongOwner,
        ));
    }
    Ok(SocketAuthority {
        directory,
        name: name.to_owned(),
    })
}

/// Accept only the already-resolved executable file promised by the public
/// API. Ownership is intentionally unrestricted: choosing what to execute is
/// the caller's authority, while this module only matches the socket peer to
/// that choice.
fn validate_executable(path: &Path) -> Option<(u64, u64)> {
    if !path.is_absolute()
        || path.as_os_str().as_bytes().contains(&0)
        || has_literal_dot_component(path)
    {
        return None;
    }
    let metadata = fs::symlink_metadata(path).ok()?;
    (!metadata.file_type().is_symlink()
        && metadata.is_file()
        && metadata.permissions().mode() & 0o111 != 0)
        .then_some((metadata.dev(), metadata.ino()))
}

/// Spend only the protocol share still left in the one total allowance. The
/// reserve shrinks with tiny remainders but never consumes the whole runner
/// budget, so configuration failure remains explicit rather than discarded.
fn run_protocol(
    socket: &SocketAuthority,
    executable: &Path,
    deadline: Instant,
) -> TmuxProtocolOutcome {
    let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
        return TmuxProtocolOutcome::NotAttempted(TmuxProtocolUnavailable::DeadlineExpired);
    };
    let total = remaining.min(PROTOCOL_ALLOWANCE);
    let reserve = CLEANUP_RESERVE.min(total / 2);
    if reserve.is_zero() {
        return TmuxProtocolOutcome::NotAttempted(TmuxProtocolUnavailable::DeadlineExpired);
    }
    let limits =
        match CommandRunLimits::new(total, reserve, OUTPUT_LIMIT, OUTPUT_LIMIT, OUTPUT_LIMIT) {
            Ok(limits) => limits,
            Err(error) => {
                return TmuxProtocolOutcome::NotAttempted(TmuxProtocolUnavailable::InvalidLimits(
                    error,
                ));
            }
        };
    let mut command = Command::new(executable);
    command
        .arg("-S")
        .arg(&socket.name)
        .arg("kill-server")
        .env_clear();
    let directory_fd = socket.directory.as_raw_fd();
    // SAFETY: only async-signal-safe fchdir and errno inspection run in the
    // forked child. The parent retains directory_fd through spawn and wait;
    // CLOEXEC closes it on exec after the child's cwd has acquired its own
    // directory reference. The calling test process never changes directory.
    unsafe {
        command.pre_exec(move || {
            if libc::fchdir(directory_fd) == 0 {
                Ok(())
            } else {
                Err(io::Error::last_os_error())
            }
        });
    }
    match run_bounded(&mut command, &limits) {
        Ok(outcome) => TmuxProtocolOutcome::Attempted(outcome),
        Err(error) => {
            TmuxProtocolOutcome::NotAttempted(TmuxProtocolUnavailable::InvalidLimits(error))
        }
    }
}

#[cfg(target_os = "linux")]
/// The sole force-stop authority, obtained from the listener socket rather
/// than reopened from numeric process metadata.
struct Peer {
    fd: OwnedFd,
}

#[cfg(target_os = "linux")]
/// Run normal protocol cleanup, observe the short healthy-exit grace, and only
/// then signal a peer whose descriptor and executable identity were verified.
fn shutdown_linux(
    socket: &SocketAuthority,
    executable: &Path,
    executable_identity: (u64, u64),
    deadline: Instant,
) -> TmuxShutdownOutcome {
    let peer = match acquire_peer(&socket.peer_path(), executable_identity, deadline) {
        Ok(peer) => peer,
        Err(reason) => {
            return run_protocol_only(
                socket,
                executable,
                deadline,
                TmuxPeerAcquisition::Unavailable(reason),
            );
        }
    };
    let protocol = run_protocol(socket, executable, deadline);
    let grace_deadline = Instant::now()
        .checked_add(NORMAL_EXIT_GRACE)
        .map_or(deadline, |grace| grace.min(deadline));
    match observe_death(&peer, grace_deadline) {
        DeathObservation::Observed => {
            return TmuxShutdownOutcome {
                protocol,
                acquisition: TmuxPeerAcquisition::Verified,
                fallback: TmuxFallback::NotNeeded,
                death: TmuxDeath::Observed,
            };
        }
        DeathObservation::PollFailed(error) => {
            return TmuxShutdownOutcome {
                protocol,
                acquisition: TmuxPeerAcquisition::Verified,
                fallback: TmuxFallback::Unavailable,
                death: TmuxDeath::PollFailed(error),
            };
        }
        DeathObservation::DeadlineExpired => {}
    }
    if Instant::now() >= deadline {
        return TmuxShutdownOutcome {
            protocol,
            acquisition: TmuxPeerAcquisition::Verified,
            fallback: TmuxFallback::DeadlineExpired,
            death: TmuxDeath::DeadlineExpired,
        };
    }
    let fallback = match signal_peer(&peer, libc::SIGKILL) {
        Ok(()) => TmuxFallback::SignalAccepted,
        Err(error) => {
            return TmuxShutdownOutcome {
                protocol,
                acquisition: TmuxPeerAcquisition::Verified,
                fallback: TmuxFallback::SignalFailed(error),
                death: TmuxDeath::Unobserved,
            };
        }
    };
    let death = match observe_death(&peer, deadline) {
        DeathObservation::Observed => TmuxDeath::Observed,
        DeathObservation::PollFailed(error) => TmuxDeath::PollFailed(error),
        DeathObservation::DeadlineExpired => TmuxDeath::DeadlineExpired,
    };
    TmuxShutdownOutcome {
        protocol,
        acquisition: TmuxPeerAcquisition::Verified,
        fallback,
        death,
    }
}

#[cfg(target_os = "linux")]
struct SocketAddress {
    address: libc::sockaddr_un,
    length: libc::socklen_t,
}

/// Build a pathname Unix address without truncation or interior-NUL aliasing.
#[cfg(target_os = "linux")]
fn socket_address(socket: &Path) -> Result<SocketAddress, TmuxPeerUnavailable> {
    let bytes = socket.as_os_str().as_bytes();
    if bytes.contains(&0) {
        return Err(TmuxPeerUnavailable::Connect(TmuxOsError {
            kind: io::ErrorKind::InvalidInput,
            raw_os_error: None,
        }));
    }
    // One byte is retained for the pathname terminator expected by AF_UNIX.
    let mut address: libc::sockaddr_un = unsafe {
        // SAFETY: sockaddr_un is plain C storage. Zero initialization produces
        // a valid buffer whose family and pathname are filled before use.
        std::mem::zeroed()
    };
    if bytes.len() >= address.sun_path.len() {
        return Err(TmuxPeerUnavailable::SocketNameTooLong);
    }
    address.sun_family = libc::AF_UNIX as libc::sa_family_t;
    for (slot, byte) in address.sun_path.iter_mut().zip(bytes.iter().copied()) {
        *slot = byte as libc::c_char;
    }
    let length = (std::mem::size_of_val(&address.sun_family) + bytes.len() + 1)
        .try_into()
        .map_err(|_| TmuxPeerUnavailable::SocketNameTooLong)?;
    Ok(SocketAddress { address, length })
}

/// Acquire the listening process itself from one nonblocking socket connect.
///
/// The connection is closed before `/proc` inspection. Numeric metadata can
/// race with peer exit and PID reuse, but the retained pidfd still names the
/// original process. Such a race cannot redirect a signal to the replacement.
#[cfg(target_os = "linux")]
fn acquire_peer(
    socket: &Path,
    executable: (u64, u64),
    deadline: Instant,
) -> Result<Peer, TmuxPeerUnavailable> {
    check_deadline(deadline)?;
    let address = socket_address(socket)?;
    // SAFETY: socket has no borrowed-pointer arguments. A successful return is
    // one newly owned descriptor, transferred immediately into OwnedFd.
    let raw = unsafe {
        libc::socket(
            libc::AF_UNIX,
            libc::SOCK_STREAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
            0,
        )
    };
    if raw < 0 {
        let error = io::Error::last_os_error();
        return Err(TmuxPeerUnavailable::SocketOpen((&error).into()));
    }
    // SAFETY: `raw` is the fresh successful socket return above and ownership
    // has not been copied or transferred anywhere else.
    let connection = unsafe { OwnedFd::from_raw_fd(raw) };
    check_deadline(deadline)?;
    // SAFETY: address lives for the call, its initialized length is bounded by
    // sockaddr_un, and connection remains owned for the whole operation.
    let connected = unsafe {
        libc::connect(
            connection.as_raw_fd(),
            (&raw const address.address).cast(),
            address.length,
        )
    };
    if connected != 0 {
        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::WouldBlock
            || error.raw_os_error() == Some(libc::EINPROGRESS)
        {
            return Err(TmuxPeerUnavailable::ConnectWouldBlock);
        }
        return Err(TmuxPeerUnavailable::Connect((&error).into()));
    }
    check_deadline(deadline)?;
    let mut peer_fd: libc::c_int = -1;
    let mut length = std::mem::size_of::<libc::c_int>() as libc::socklen_t;
    // SAFETY: peer_fd and length are writable for their declared sizes;
    // connection is a live AF_UNIX socket, and the kernel writes at most the
    // supplied integer length.
    let result = unsafe {
        libc::getsockopt(
            connection.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERPIDFD,
            (&raw mut peer_fd).cast(),
            &raw mut length,
        )
    };
    if result != 0 {
        let error = io::Error::last_os_error();
        return Err(
            if error.raw_os_error() == Some(libc::ENOPROTOOPT)
                || error.raw_os_error() == Some(libc::ENOSYS)
            {
                TmuxPeerUnavailable::PeerDescriptorUnsupported
            } else {
                TmuxPeerUnavailable::PeerDescriptor((&error).into())
            },
        );
    }
    if peer_fd < 0 || length != std::mem::size_of::<libc::c_int>() as libc::socklen_t {
        return Err(TmuxPeerUnavailable::PeerDescriptorInvalid);
    }
    // SAFETY: SO_PEERPIDFD returned a new descriptor in peer_fd on success;
    // the validation above excludes sentinel values before ownership transfer.
    let peer = Peer {
        fd: unsafe { OwnedFd::from_raw_fd(peer_fd) },
    };
    // SAFETY: fcntl reads and updates flags on the still-owned descriptor. No
    // alias takes ownership and the calls do not retain pointers.
    let descriptor_flags = unsafe { libc::fcntl(peer.fd.as_raw_fd(), libc::F_GETFD) };
    if descriptor_flags < 0 {
        let error = io::Error::last_os_error();
        return Err(TmuxPeerUnavailable::PeerDescriptorFlags((&error).into()));
    }
    // SAFETY: the descriptor remains valid and owned; F_SETFD consumes only
    // the integer flags value and does not alter ownership.
    if unsafe {
        libc::fcntl(
            peer.fd.as_raw_fd(),
            libc::F_SETFD,
            descriptor_flags | libc::FD_CLOEXEC,
        )
    } < 0
    {
        let error = io::Error::last_os_error();
        return Err(TmuxPeerUnavailable::PeerDescriptorFlags((&error).into()));
    }
    drop(connection);
    check_deadline(deadline)?;
    let pid = peer_pid(&peer, deadline)?;
    refuse_current_process(pid, deadline)?;
    check_deadline(deadline)?;
    let metadata = fs::metadata(format!("/proc/{pid}/exe"))
        .map_err(|error| TmuxPeerUnavailable::PeerExecutableMetadata((&error).into()))?;
    check_deadline(deadline)?;
    if (metadata.dev(), metadata.ino()) != executable {
        return Err(TmuxPeerUnavailable::PeerExecutableMismatch);
    }
    Ok(peer)
}

#[cfg(target_os = "linux")]
/// Keep acquisition phases on the shutdown function's original deadline.
fn check_deadline(deadline: Instant) -> Result<(), TmuxPeerUnavailable> {
    (Instant::now() < deadline)
        .then_some(())
        .ok_or(TmuxPeerUnavailable::DeadlineExpired)
}

/// Compare identities in the mounted procfs namespace. `process::id()` uses
/// the active PID namespace, which can differ after entering a child namespace
/// without remounting procfs. Both `/proc/self` and pidfd fdinfo instead use
/// that procfs mount's namespace. These numbers never authorize a numeric kill.
#[cfg(target_os = "linux")]
fn refuse_current_process(pid: u32, deadline: Instant) -> Result<(), TmuxPeerUnavailable> {
    check_deadline(deadline)?;
    let mut name = [0u8; 32];
    // SAFETY: the literal is NUL-terminated and name is writable for its full
    // length. readlink retains neither pointer and writes no more than len.
    let length =
        unsafe { libc::readlink(c"/proc/self".as_ptr(), name.as_mut_ptr().cast(), name.len()) };
    if length < 0 {
        return Err(TmuxPeerUnavailable::CurrentProcessReadlink(
            (&io::Error::last_os_error()).into(),
        ));
    }
    check_deadline(deadline)?;
    if length as usize == name.len() {
        return Err(TmuxPeerUnavailable::CurrentProcessMalformed);
    }
    refuse_procfs_self(pid, &name[..length as usize])
}

/// Refuse self before inspecting executable identity, and fail closed when
/// procfs cannot supply comparable identity. Keeping the procfs spelling as
/// an input lets tests exercise namespace mismatch without privileged setup.
#[cfg(target_os = "linux")]
fn refuse_procfs_self(pid: u32, self_name: &[u8]) -> Result<(), TmuxPeerUnavailable> {
    let own_pid: u32 = std::str::from_utf8(self_name)
        .ok()
        .filter(|name| !name.is_empty() && name.bytes().all(|byte| byte.is_ascii_digit()))
        .and_then(|name| name.parse().ok())
        .filter(|pid| *pid != 0)
        .ok_or(TmuxPeerUnavailable::CurrentProcessMalformed)?;
    if pid == own_pid {
        Err(TmuxPeerUnavailable::PeerIsCurrentProcess)
    } else {
        Ok(())
    }
}

/// Read only the small fdinfo prefix needed to bind the pidfd to executable
/// metadata. Overflow and malformed UTF-8/Pid fields are separate from I/O
/// failures so a kernel-format change cannot silently broaden authority.
#[cfg(target_os = "linux")]
fn peer_pid(peer: &Peer, deadline: Instant) -> Result<u32, TmuxPeerUnavailable> {
    check_deadline(deadline)?;
    let file = fs::File::open(format!("/proc/self/fdinfo/{}", peer.fd.as_raw_fd()))
        .map_err(|error| TmuxPeerUnavailable::PeerFdInfoOpen((&error).into()))?;
    check_deadline(deadline)?;
    parse_peer_pid(file, deadline)
}

#[cfg(target_os = "linux")]
/// Parse one bounded fdinfo record without accepting truncated or malformed
/// numeric metadata as signal authority.
fn parse_peer_pid(mut reader: impl Read, deadline: Instant) -> Result<u32, TmuxPeerUnavailable> {
    check_deadline(deadline)?;
    let mut bytes = Vec::with_capacity(FDINFO_LIMIT + 1);
    reader
        .by_ref()
        .take((FDINFO_LIMIT + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| TmuxPeerUnavailable::PeerFdInfoRead((&error).into()))?;
    check_deadline(deadline)?;
    if bytes.len() > FDINFO_LIMIT {
        return Err(TmuxPeerUnavailable::PeerFdInfoOverflow);
    }
    std::str::from_utf8(&bytes)
        .ok()
        .and_then(|info| {
            info.lines()
                .find_map(|line| line.strip_prefix("Pid:\t")?.parse().ok())
        })
        .ok_or(TmuxPeerUnavailable::PeerFdInfoMalformed)
}

#[cfg(target_os = "linux")]
enum DeathObservation {
    Observed,
    PollFailed(TmuxPollFailure),
    DeadlineExpired,
}

/// Poll the pidfd until it reports process exit or the supplied slice expires.
/// Interrupted polls retry only against the original deadline.
#[cfg(target_os = "linux")]
fn observe_death(peer: &Peer, deadline: Instant) -> DeathObservation {
    observe_death_with(peer, deadline, |pollfd, timeout| {
        // SAFETY: pollfd points to one initialized pollfd for the duration of
        // the call. poll neither retains the pointer nor takes fd ownership.
        let result = unsafe { libc::poll(pollfd, 1, timeout) };
        if result < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(result)
        }
    })
}

#[cfg(target_os = "linux")]
fn observe_death_with(
    peer: &Peer,
    deadline: Instant,
    mut poller: impl FnMut(*mut libc::pollfd, libc::c_int) -> io::Result<libc::c_int>,
) -> DeathObservation {
    loop {
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            return DeathObservation::DeadlineExpired;
        };
        let rounded_millis = remaining
            .as_millis()
            .saturating_add(u128::from(remaining.subsec_nanos() % 1_000_000 != 0));
        let timeout = rounded_millis.min(libc::c_int::MAX as u128) as libc::c_int;
        let mut pollfd = libc::pollfd {
            fd: peer.fd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        match poller(&raw mut pollfd, timeout.max(1)) {
            Ok(0) => return DeathObservation::DeadlineExpired,
            Ok(_) if pollfd.revents & libc::POLLIN != 0 => return DeathObservation::Observed,
            Ok(_) => {
                return DeathObservation::PollFailed(TmuxPollFailure::UnexpectedEvents(
                    pollfd.revents,
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => {
                return DeathObservation::PollFailed(TmuxPollFailure::System((&error).into()));
            }
        }
    }
}

/// Request a signal only through the socket-derived descriptor. A successful
/// syscall means the kernel accepted the request; death still requires a
/// separate pidfd poll.
#[cfg(target_os = "linux")]
fn signal_peer(peer: &Peer, signal: libc::c_int) -> Result<(), TmuxOsError> {
    // SAFETY: peer.fd is an owned pidfd obtained from SO_PEERPIDFD. The null
    // siginfo pointer and zero flags are the documented pidfd_send_signal form;
    // the syscall borrows the descriptor and retains no memory.
    let result = unsafe {
        libc::syscall(
            libc::SYS_pidfd_send_signal,
            peer.fd.as_raw_fd(),
            signal,
            std::ptr::null::<libc::siginfo_t>(),
            0,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        let error = io::Error::last_os_error();
        Err((&error).into())
    }
}

/// Reject literal `.` and `..` before `Path::components` can normalize them
/// away and make a redirected namespace look like the direct authority root.
fn has_literal_dot_component(path: &Path) -> bool {
    path.as_os_str()
        .as_bytes()
        .split(|byte| *byte == b'/')
        .any(|part| part == b"." || part == b"..")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::fs::File;
    use std::os::unix::ffi::OsStringExt;
    use std::os::unix::fs::{PermissionsExt, symlink};
    #[cfg(target_os = "linux")]
    use std::os::unix::net::UnixStream;
    use std::os::unix::net::{UnixDatagram, UnixListener};
    use std::path::PathBuf;
    use std::process::{Child, Stdio};

    const FOREIGN_SOCKET: &str = "FARHELM_TEST_FOREIGN_SOCKET";
    const FOREIGN_READY: &str = "FARHELM_TEST_FOREIGN_READY";

    /// Establish socket authority explicitly; tempfile's default permissions
    /// follow the ambient umask and need not make the directory private.
    fn private_directory() -> tempfile::TempDir {
        tempfile::Builder::new()
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir()
            .expect("private socket authority directory")
    }

    /// Relative names never borrow authority from the caller's current
    /// directory, even when that directory happens to be private.
    #[test]
    fn relative_socket_refuses_every_shutdown_path() {
        let outcome = shutdown_tmux_server(Path::new("tmux.sock"), &true_executable());
        assert_eq!(
            outcome.acquisition,
            TmuxPeerAcquisition::Refused(TmuxPathRefusal::Relative)
        );
        assert_eq!(
            not_attempted(&outcome),
            TmuxProtocolUnavailable::NotAuthorized
        );
    }

    /// Literal dot names remain visible to validation instead of being
    /// normalized across a symlinked parent.
    #[test]
    fn literal_dot_components_are_refused() {
        let directory = private_directory();
        let target = directory.path().join("target");
        fs::create_dir(&target).expect("target directory");
        symlink(&target, directory.path().join("link")).expect("parent symlink");
        for component in [".", ".."] {
            let spelled = directory
                .path()
                .join("link")
                .join(component)
                .join("tmux.sock");
            let outcome = shutdown_tmux_server(&spelled, &true_executable());
            assert_eq!(
                outcome.acquisition,
                TmuxPeerAcquisition::Refused(TmuxPathRefusal::DotComponent)
            );
        }
    }

    /// Group or other access invalidates the direct parent's role as a private
    /// test namespace before the final path is touched.
    #[test]
    fn nonprivate_parent_refuses_protocol_authority() {
        let directory = private_directory();
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o755))
            .expect("open parent");
        let outcome = shutdown_tmux_server(&directory.path().join("tmux.sock"), &true_executable());
        assert_eq!(
            outcome.acquisition,
            TmuxPeerAcquisition::Refused(TmuxPathRefusal::ParentNotPrivate)
        );
        assert_eq!(
            not_attempted(&outcome),
            TmuxProtocolUnavailable::NotAuthorized
        );
    }

    /// Neither a parent symlink nor a final socket symlink can redirect the
    /// namespace that authorizes a client or signal.
    #[test]
    fn symlinked_authority_is_refused() {
        let directory = private_directory();
        let target = directory.path().join("target");
        fs::create_dir(&target).expect("target directory");
        fs::set_permissions(&target, fs::Permissions::from_mode(0o700))
            .expect("private target directory");
        let socket = target.join("real.sock");
        let _listener = UnixListener::bind(&socket).expect("real socket");

        let parent_link = directory.path().join("parent-link");
        symlink(&target, &parent_link).expect("parent link");
        let parent_outcome =
            shutdown_tmux_server(&parent_link.join("real.sock"), &true_executable());
        assert_eq!(
            parent_outcome.acquisition,
            TmuxPeerAcquisition::Refused(TmuxPathRefusal::ParentSymlink)
        );

        let socket_link = target.join("socket-link");
        symlink(&socket, &socket_link).expect("socket link");
        let socket_outcome = shutdown_tmux_server(&socket_link, &true_executable());
        assert_eq!(
            socket_outcome.acquisition,
            TmuxPeerAcquisition::Refused(TmuxPathRefusal::SocketSymlink)
        );
    }

    /// Missing and non-socket final components retain distinct evidence; a
    /// missing pathname is not evidence that an earlier server died.
    #[test]
    fn malformed_final_authority_is_distinguished() {
        let directory = private_directory();
        let absent =
            shutdown_tmux_server(&directory.path().join("absent.sock"), &true_executable());
        assert_eq!(absent.acquisition, TmuxPeerAcquisition::SocketAbsent);
        assert_eq!(
            not_attempted(&absent),
            TmuxProtocolUnavailable::NotAuthorized
        );

        let regular = directory.path().join("regular");
        File::create(&regular).expect("regular file");
        let outcome = shutdown_tmux_server(&regular, &true_executable());
        assert_eq!(
            outcome.acquisition,
            TmuxPeerAcquisition::Refused(TmuxPathRefusal::SocketNotSocket)
        );
    }

    /// Interior NUL bytes are refused as path authority rather than reaching a
    /// libc call whose shorter C-string interpretation names another object.
    #[test]
    fn nul_socket_path_is_refused() {
        let path = PathBuf::from(OsString::from_vec(b"/tmp/fh\0/tmux.sock".to_vec()));
        let outcome = shutdown_tmux_server(&path, &true_executable());
        assert_eq!(
            outcome.acquisition,
            TmuxPeerAcquisition::Refused(TmuxPathRefusal::NulByte)
        );
    }

    /// A private listener owned by this process survives because current-self
    /// identity can never become fallback authority. Older kernels report the
    /// unsupported option explicitly while preserving the same survival fact.
    #[test]
    fn self_owned_fake_listener_survives_fallback_refusal() {
        let directory = private_directory();
        let socket = directory.path().join("tmux.sock");
        let listener = UnixListener::bind(&socket).expect("fake socket listener");
        let outcome = shutdown_tmux_server(&socket, &true_executable());

        #[cfg(target_os = "linux")]
        assert_supported_or_explicit(
            outcome.acquisition,
            TmuxPeerUnavailable::PeerIsCurrentProcess,
            "self-listener refusal",
        );
        #[cfg(not(target_os = "linux"))]
        assert_eq!(
            outcome.acquisition,
            TmuxPeerAcquisition::Unavailable(TmuxPeerUnavailable::UnsupportedPlatform)
        );
        assert_eq!(outcome.fallback, TmuxFallback::Unavailable);
        assert!(listener.local_addr().is_ok(), "fake listener must survive");
    }

    /// A listener in a different process cannot borrow the supplied tmux
    /// executable's identity. Unsupported platforms still run protocol-only
    /// cleanup, and the independently owned child survives either path.
    #[test]
    fn foreign_fake_listener_survives_executable_refusal() {
        let mut fixture = ForeignListenerFixture::start();
        let outcome = shutdown_tmux_server(&fixture.socket, &true_executable());
        #[cfg(target_os = "linux")]
        assert_supported_or_explicit(
            outcome.acquisition,
            TmuxPeerUnavailable::PeerExecutableMismatch,
            "foreign-listener refusal",
        );
        #[cfg(not(target_os = "linux"))]
        assert_eq!(
            outcome.acquisition,
            TmuxPeerAcquisition::Unavailable(TmuxPeerUnavailable::UnsupportedPlatform)
        );
        assert_eq!(outcome.fallback, TmuxFallback::Unavailable);
        assert!(fixture.child.is_running(), "foreign listener must survive");
    }

    /// Injected unavailable acquisition still runs bounded protocol cleanup,
    /// while preserving the lack of liveness and signalling evidence.
    #[test]
    fn injected_unavailable_acquisition_keeps_evidence_honest() {
        let directory = private_directory();
        let socket = directory.path().join("tmux.sock");
        let listener = UnixListener::bind(&socket).expect("fake socket listener");
        let outcome = shutdown_with_unavailable_acquisition_for_test(
            &socket,
            &true_executable(),
            TmuxPeerUnavailable::PeerDescriptorUnsupported,
        );
        assert!(matches!(
            outcome.protocol,
            TmuxProtocolOutcome::Attempted(_)
        ));
        assert_eq!(
            outcome.acquisition,
            TmuxPeerAcquisition::Unavailable(TmuxPeerUnavailable::PeerDescriptorUnsupported)
        );
        assert_eq!(outcome.fallback, TmuxFallback::Unavailable);
        assert_eq!(outcome.death, TmuxDeath::Unobserved);
        assert!(listener.local_addr().is_ok(), "fake listener must survive");
    }

    /// A real private server exits through the bounded client path, and its
    /// fixture already owns a separate pidfd before assertions begin.
    #[cfg(target_os = "linux")]
    #[test]
    fn healthy_tmux_server_is_observed_dead() {
        let Some(fixture) = TmuxFixture::start() else {
            return;
        };
        let started = Instant::now();
        let outcome = shutdown_tmux_server(&fixture.socket, &fixture.executable);
        // Retain absolute teardown cost without making scheduler timing a
        // pass/fail premise. The recorder keeps this with the exact substrate.
        println!("healthy tmux shutdown elapsed: {:?}", started.elapsed());
        let TmuxProtocolOutcome::Attempted(protocol) = &outcome.protocol else {
            panic!("healthy server requires the protocol path: {outcome:?}");
        };
        assert!(!protocol.timed_out && protocol.direct_child_reaped);
        assert!(protocol.status.is_some_and(|status| status.success()));
        assert_eq!(outcome.acquisition, TmuxPeerAcquisition::Verified);
        assert_eq!(outcome.fallback, TmuxFallback::NotNeeded);
        assert_eq!(outcome.death, TmuxDeath::Observed);
    }

    /// Replacing the original pathname after validation must redirect neither
    /// peer acquisition nor the protocol client. Both servers are the same
    /// executable, so executable matching alone cannot distinguish this race.
    #[cfg(target_os = "linux")]
    #[test]
    fn ancestor_replacement_cannot_redirect_shutdown() {
        let Some(original) = TmuxFixture::start() else {
            return;
        };
        let Some(unrelated) = TmuxFixture::start() else {
            return;
        };
        let authority = validate_socket(&original.socket)
            .unwrap_or_else(|error| panic!("original socket authority: {error:?}"));
        let relocation = private_directory();
        let moved = relocation.path().join("moved");
        let original_path = original._directory.path().to_path_buf();
        fs::rename(&original_path, &moved).expect("move original authority directory");

        /// Restore the fixture pathname before its TempDir drops, including
        /// assertion unwind. Server cleanup separately retains both pidfds.
        struct RestoreDirectory {
            original: PathBuf,
            moved: PathBuf,
            substituted: bool,
        }
        impl Drop for RestoreDirectory {
            fn drop(&mut self) {
                if self.substituted {
                    let _ = fs::remove_file(&self.original);
                }
                let _ = fs::rename(&self.moved, &self.original);
            }
        }
        let mut restore = RestoreDirectory {
            original: original_path,
            moved,
            substituted: false,
        };
        symlink(unrelated._directory.path(), &restore.original)
            .expect("substitute unrelated directory");
        restore.substituted = true;

        let outcome = shutdown_authorized(
            &authority,
            &original.executable,
            validate_executable(&original.executable).unwrap(),
            Instant::now() + TOTAL_ALLOWANCE,
        );
        assert_eq!(outcome.acquisition, TmuxPeerAcquisition::Verified);
        assert_eq!(outcome.fallback, TmuxFallback::NotNeeded);
        assert_eq!(outcome.death, TmuxDeath::Observed);
        signal_peer(&unrelated.peer, 0).expect("unrelated server must still exist");
        assert!(
            matches!(
                observe_death(&unrelated.peer, Instant::now() + Duration::from_millis(1)),
                DeathObservation::DeadlineExpired
            ),
            "unrelated server must not have exited"
        );
    }

    /// A stopped server cannot answer `kill-server`; the API and fixture hold
    /// separate verified pidfds, so fallback is tested without making cleanup
    /// depend on the behavior under test.
    #[cfg(target_os = "linux")]
    #[test]
    fn stopped_tmux_server_uses_verified_pidfd_fallback() {
        let Some(fixture) = TmuxFixture::start() else {
            return;
        };
        signal_peer(&fixture.peer, libc::SIGSTOP).expect("stop real server through fixture pidfd");
        let outcome = shutdown_tmux_server(&fixture.socket, &fixture.executable);
        let TmuxProtocolOutcome::Attempted(protocol) = &outcome.protocol else {
            panic!("stopped server requires a bounded protocol attempt: {outcome:?}");
        };
        assert!(protocol.timed_out && protocol.direct_child_reaped);
        assert_eq!(outcome.acquisition, TmuxPeerAcquisition::Verified);
        assert_eq!(outcome.fallback, TmuxFallback::SignalAccepted);
        assert_eq!(outcome.death, TmuxDeath::Observed);
    }

    /// A process can be PID 7 in its active namespace and 42007 in procfs.
    /// Self-refusal must use the latter, without mistaking a different peer
    /// whose procfs PID happens to be 7 for the caller. Invalid self identity
    /// cannot silently grant fallback authority.
    #[cfg(target_os = "linux")]
    #[test]
    fn self_refusal_uses_the_procfs_namespace() {
        assert_eq!(
            refuse_procfs_self(42007, b"42007"),
            Err(TmuxPeerUnavailable::PeerIsCurrentProcess)
        );
        assert_eq!(refuse_procfs_self(7, b"42007"), Ok(()));
        for invalid in [b"".as_slice(), b"0", b"+7", b"../7", b"4294967296", b"\xff"] {
            assert_eq!(
                refuse_procfs_self(7, invalid),
                Err(TmuxPeerUnavailable::CurrentProcessMalformed)
            );
        }
    }

    /// A peer fdinfo record larger than the fixed cap is rejected before its
    /// otherwise-valid PID can authorize executable inspection.
    #[cfg(target_os = "linux")]
    #[test]
    fn fdinfo_overflow_is_rejected() {
        let input = vec![b'x'; FDINFO_LIMIT + 1];
        assert_eq!(
            parse_peer_pid(
                io::Cursor::new(input),
                Instant::now() + Duration::from_secs(1)
            ),
            Err(TmuxPeerUnavailable::PeerFdInfoOverflow)
        );
    }

    /// A poll error remains an error and cannot be upgraded to observed
    /// survival, which would incorrectly authorize a force signal.
    #[cfg(target_os = "linux")]
    #[test]
    fn poll_error_is_not_observed_survival() {
        let (stream, _peer_stream) = UnixStream::pair().expect("descriptor pair");
        let peer = Peer { fd: stream.into() };
        let observation =
            observe_death_with(&peer, Instant::now() + Duration::from_secs(1), |_, _| {
                Err(io::Error::from_raw_os_error(libc::EBADF))
            });
        assert!(matches!(
            observation,
            DeathObservation::PollFailed(TmuxPollFailure::System(TmuxOsError {
                raw_os_error: Some(libc::EBADF),
                ..
            }))
        ));
    }

    /// sockaddr construction refuses a name that cannot fit with its trailing
    /// NUL rather than silently targeting a truncated listener.
    #[cfg(target_os = "linux")]
    #[test]
    fn overlong_socket_address_is_rejected() {
        let mut bytes = vec![b'/'];
        bytes.extend(std::iter::repeat_n(b'x', 256));
        let path = PathBuf::from(OsString::from_vec(bytes));
        assert!(matches!(
            socket_address(&path),
            Err(TmuxPeerUnavailable::SocketNameTooLong)
        ));
    }

    /// Ordinary test discovery returns immediately because it supplies no
    /// fixture markers. A foreign-listener subprocess supplies both markers;
    /// its datagram then proves bind completed before shutdown is attempted.
    #[test]
    fn foreign_listener_helper() {
        let (Some(socket), Some(ready)) = (
            std::env::var_os(FOREIGN_SOCKET),
            std::env::var_os(FOREIGN_READY),
        ) else {
            return;
        };
        let _listener = UnixListener::bind(Path::new(&socket)).expect("bind foreign listener");
        let sender = UnixDatagram::unbound().expect("ready datagram");
        sender
            .send_to(b"R", Path::new(&ready))
            .expect("report listener readiness");
        loop {
            std::thread::park();
        }
    }

    fn not_attempted(outcome: &TmuxShutdownOutcome) -> TmuxProtocolUnavailable {
        match &outcome.protocol {
            TmuxProtocolOutcome::NotAttempted(reason) => *reason,
            TmuxProtocolOutcome::Attempted(_) => panic!("protocol unexpectedly attempted"),
        }
    }

    fn true_executable() -> PathBuf {
        for candidate in [Path::new("/usr/bin/true"), Path::new("/bin/true")] {
            if let Ok(resolved) = fs::canonicalize(candidate)
                && validate_executable(&resolved).is_some()
            {
                return resolved;
            }
        }
        panic!("no resolved true executable")
    }

    /// Inject the exact protocol-only boundary after ordinary authority checks.
    /// This does not bypass socket or executable validation, acquire a hidden
    /// descriptor, or infer liveness from the resulting client evidence.
    fn shutdown_with_unavailable_acquisition_for_test(
        socket: &Path,
        executable: &Path,
        unavailable: TmuxPeerUnavailable,
    ) -> TmuxShutdownOutcome {
        let deadline = Instant::now() + TOTAL_ALLOWANCE;
        let authority = match validate_socket(socket) {
            Ok(authority) => authority,
            Err(acquisition) => return refused(acquisition),
        };
        if validate_executable(executable).is_none() {
            return refused(TmuxPeerAcquisition::ExecutableRefused);
        }
        run_protocol_only(
            &authority,
            executable,
            deadline,
            TmuxPeerAcquisition::Unavailable(unavailable),
        )
    }

    #[cfg(target_os = "linux")]
    /// Accept only the one demonstrated missing-kernel facility as a loud
    /// coverage skip; every other verification failure remains a test error.
    fn assert_supported_or_explicit(
        actual: TmuxPeerAcquisition,
        expected: TmuxPeerUnavailable,
        contract: &str,
    ) {
        match actual {
            TmuxPeerAcquisition::Unavailable(TmuxPeerUnavailable::PeerDescriptorUnsupported) => {
                eprintln!("SKIPPED {contract}: this Linux kernel does not provide SO_PEERPIDFD");
            }
            TmuxPeerAcquisition::Unavailable(actual) => assert_eq!(actual, expected),
            other => panic!("unexpected acquisition for {contract}: {other:?}"),
        }
    }

    #[cfg(target_os = "linux")]
    enum FixtureStart {
        Ready(TmuxFixture),
        Unsupported,
    }

    /// Even partial setup must validate its current socket before protocol
    /// cleanup. A missing or refused path is recorded without starting a child.
    #[cfg(target_os = "linux")]
    fn cleanup_fixture_protocol(socket: &Path, executable: &Path) -> TmuxProtocolOutcome {
        match validate_socket(socket) {
            Ok(authority) => run_protocol(&authority, executable, Instant::now() + TOTAL_ALLOWANCE),
            Err(_) => TmuxProtocolOutcome::NotAttempted(TmuxProtocolUnavailable::NotAuthorized),
        }
    }

    /// Owns a real daemon through a pidfd captured before any test assertion.
    /// Drop never invokes tmux: it can clean a deliberately stopped server
    /// without depending on the protocol path being tested.
    #[cfg(target_os = "linux")]
    struct TmuxFixture {
        _directory: tempfile::TempDir,
        socket: PathBuf,
        executable: PathBuf,
        peer: Peer,
    }

    #[cfg(target_os = "linux")]
    impl TmuxFixture {
        fn start() -> Option<Self> {
            match Self::start_inner() {
                FixtureStart::Ready(fixture) => Some(fixture),
                FixtureStart::Unsupported => {
                    eprintln!(
                        "SKIPPED real tmux pidfd contract: this Linux kernel does not provide SO_PEERPIDFD"
                    );
                    None
                }
            }
        }

        fn start_inner() -> FixtureStart {
            let directory = private_directory();
            let socket = directory.path().join("tmux.sock");
            let executable = tmux_from_path();
            let limits = CommandRunLimits::new(
                TOTAL_ALLOWANCE,
                CLEANUP_RESERVE,
                OUTPUT_LIMIT,
                OUTPUT_LIMIT,
                OUTPUT_LIMIT,
            )
            .expect("valid fixture setup limits");
            let mut command = Command::new(&executable);
            command
                .arg("-f")
                .arg("/dev/null")
                .arg("-S")
                .arg(&socket)
                .args(["new-session", "-d"])
                .env_clear();
            let setup = run_bounded(&mut command, &limits).expect("valid fixture runner limits");
            let identity = validate_executable(&executable).expect("resolved tmux executable");
            let acquisition = acquire_peer(&socket, identity, Instant::now() + TOTAL_ALLOWANCE);
            match (command_succeeded(&setup), acquisition) {
                (true, Ok(peer)) => FixtureStart::Ready(Self {
                    _directory: directory,
                    socket,
                    executable,
                    peer,
                }),
                (false, Ok(peer)) => {
                    let _cleanup = Self {
                        _directory: directory,
                        socket,
                        executable,
                        peer,
                    };
                    panic!("bounded tmux setup failed after daemonization: {setup:?}");
                }
                (true, Err(TmuxPeerUnavailable::PeerDescriptorUnsupported)) => {
                    let cleanup = cleanup_fixture_protocol(&socket, &executable);
                    assert!(
                        matches!(&cleanup, TmuxProtocolOutcome::Attempted(outcome) if command_succeeded(outcome)),
                        "unsupported-kernel fixture cleanup failed: {cleanup:?}"
                    );
                    FixtureStart::Unsupported
                }
                (setup_succeeded, Err(error)) => {
                    let cleanup = cleanup_fixture_protocol(&socket, &executable);
                    panic!(
                        "tmux fixture failed (setup succeeded: {setup_succeeded}, acquisition: \
                         {error:?}); setup: {setup:?}; cleanup: {cleanup:?}"
                    );
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    impl Drop for TmuxFixture {
        fn drop(&mut self) {
            let _ = signal_peer(&self.peer, libc::SIGKILL);
            let _ = observe_death(&self.peer, Instant::now() + TOTAL_ALLOWANCE);
        }
    }

    #[cfg(target_os = "linux")]
    fn command_succeeded(outcome: &CommandRunOutcome) -> bool {
        outcome
            .status
            .as_ref()
            .is_some_and(|status| status.success())
            && !outcome.timed_out
            && outcome.errors.is_empty()
            && outcome.direct_child_reaped
            && !outcome.ownership_lost
    }

    /// Resolve the pinned tmux selected by the inherited PATH without changing
    /// it. A candidate must be executable before and after canonicalization;
    /// finding a merely existing or canonicalizable name is not enough.
    #[cfg(target_os = "linux")]
    fn tmux_from_path() -> PathBuf {
        let path = std::env::var_os("PATH").expect("PATH for pinned tmux");
        for directory in std::env::split_paths(&path) {
            let candidate = directory.join("tmux");
            let Ok(metadata) = fs::metadata(&candidate) else {
                continue;
            };
            if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
                continue;
            }
            let Ok(resolved) = fs::canonicalize(&candidate) else {
                continue;
            };
            if validate_executable(&resolved).is_some() {
                return resolved;
            }
        }
        panic!("no executable tmux in PATH")
    }

    /// Owns a foreign listener subprocess and its causal readiness datagram.
    /// Cleanup addresses only the retained direct child and bounds reaping;
    /// the fake listener must never be signalled through its socket.
    struct ForeignListenerFixture {
        _directory: tempfile::TempDir,
        socket: PathBuf,
        child: DirectChildGuard,
    }

    impl ForeignListenerFixture {
        fn start() -> Self {
            let directory = private_directory();
            let socket = directory.path().join("tmux.sock");
            let ready_path = directory.path().join("ready.sock");
            let ready = UnixDatagram::bind(&ready_path).expect("bind readiness socket");
            ready
                .set_read_timeout(Some(TOTAL_ALLOWANCE))
                .expect("bound readiness timeout");
            let mut command =
                Command::new(std::env::current_exe().expect("current test executable"));
            command
                .args([
                    "--exact",
                    "tmux::tests::foreign_listener_helper",
                    "--nocapture",
                ])
                .env_clear()
                .env(FOREIGN_SOCKET, &socket)
                .env(FOREIGN_READY, &ready_path)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            let child = command.spawn().expect("spawn foreign listener helper");
            let mut child = DirectChildGuard { child };
            let mut byte = [0_u8; 1];
            let read = ready
                .recv(&mut byte)
                .unwrap_or_else(|error| panic!("foreign listener readiness failed: {error}"));
            assert_eq!(&byte[..read], b"R", "foreign listener ready byte");
            assert!(
                child.is_running(),
                "foreign listener exited after readiness"
            );
            Self {
                _directory: directory,
                socket,
                child,
            }
        }
    }

    /// Bounded ownership of one fixture subprocess. This direct child is the
    /// only numeric process handle tests may kill; server cleanup uses pidfds.
    struct DirectChildGuard {
        child: Child,
    }

    impl DirectChildGuard {
        fn is_running(&mut self) -> bool {
            self.child
                .try_wait()
                .expect("poll foreign listener")
                .is_none()
        }
    }

    impl Drop for DirectChildGuard {
        fn drop(&mut self) {
            // Readiness came only from the explicit datagram handshake. These
            // bounded try_wait probes observe cleanup of the retained direct
            // child after kill; they are never used to infer readiness.
            match self.child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) => {}
                Err(_) => return,
            }
            let _ = self.child.kill();
            let deadline = Instant::now() + TOTAL_ALLOWANCE;
            while Instant::now() < deadline {
                match self.child.try_wait() {
                    Ok(Some(_)) | Err(_) => return,
                    Ok(None) => std::thread::park_timeout(Duration::from_millis(10)),
                }
            }
        }
    }
}
