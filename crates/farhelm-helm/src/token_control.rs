//! Local coordination for `farhelm helm token rotate`.
//!
//! Token management is a separate CLI invocation, but rotation must run in
//! the serving helm when one exists: that process owns the authenticated
//! WebSockets which have to close immediately after their device rows are
//! deleted. A private Unix socket in the helm state directory is the narrow
//! bridge. It carries one command, never browser traffic, and the directory's
//! 0700 mode gives it the same trust boundary as direct access to helm.db. A
//! lifetime flock serializes serving ownership with offline rotation, while
//! the CLI verifies the peer's uid (SO_PEERCRED on Linux, getpeereid on
//! macOS) and bounds connect, write, and read under one deadline before
//! trusting the reply.

use crate::{auth, auth::AuthState, store::HelmStore};
use anyhow::Context;
use std::fs::{File, OpenOptions};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};

const SOCKET_NAME: &str = "helm-token.sock";
const LOCK_NAME: &str = "helm-token.lock";
const MAX_REPLY_BYTES: u64 = 128;
const CONTROL_DEADLINE: std::time::Duration = std::time::Duration::from_secs(2);

/// Signals that a serving helm has claimed the state directory but its socket
/// may still be in the short startup window before bind completes.
#[derive(Debug, thiserror::Error)]
#[error("another process owns token control")]
struct OwnershipBusy;

/// Exclusive ownership of token-control decisions for one state directory.
struct OwnershipLock {
    file: File,
}

impl Drop for OwnershipLock {
    fn drop(&mut self) {
        // SAFETY: `file` remains open for this call, and unlocking an owned
        // advisory flock neither aliases memory nor transfers the fd.
        unsafe { libc::flock(self.file.as_raw_fd(), libc::LOCK_UN) };
    }
}

/// Keep the token-control listener alive and remove its socket on shutdown.
pub(crate) struct ControlGuard {
    task: tokio::task::JoinHandle<anyhow::Result<()>>,
    path: PathBuf,
    socket_identity: (u64, u64),
    _ownership: OwnershipLock,
}

impl ControlGuard {
    /// Resolve only if the listener task dies; serving HTTP past that point
    /// would make later offline rotation miss this process's live sockets.
    pub(crate) async fn failed(&mut self) -> anyhow::Result<()> {
        (&mut self.task)
            .await
            .context("token-control listener task panicked")??;
        anyhow::bail!("token-control listener stopped unexpectedly")
    }
}

impl Drop for ControlGuard {
    fn drop(&mut self) {
        self.task.abort();
        if let Ok(metadata) = std::fs::symlink_metadata(&self.path)
            && metadata.file_type().is_socket()
            && (metadata.dev(), metadata.ino()) == self.socket_identity
        {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Start the serving helm's local token-control endpoint.
pub(crate) async fn serve(state_dir: &Path, auth: AuthState) -> anyhow::Result<ControlGuard> {
    farhelm_supervisor::ensure_private_dir(state_dir).await?;
    let ownership = acquire_ownership(state_dir.to_path_buf()).await?;
    let path = state_dir.join(SOCKET_NAME);
    remove_stale_socket(&path)?;
    let listener = tokio::net::UnixListener::bind(&path)
        .with_context(|| format!("binding token-control socket {}", path.display()))?;
    let metadata = std::fs::symlink_metadata(&path)
        .with_context(|| format!("inspecting bound token-control socket {}", path.display()))?;
    let socket_identity = (metadata.dev(), metadata.ino());
    let task = tokio::spawn(async move {
        loop {
            let stream = match listener.accept().await {
                Ok((stream, _)) => stream,
                Err(error) if transient_accept_error(&error) => {
                    tracing::warn!(error = %error, "transient token-control accept failed");
                    continue;
                }
                Err(error) => {
                    return Err(
                        anyhow::Error::new(error).context("accepting a token-control connection")
                    );
                }
            };
            let auth = auth.clone();
            tokio::spawn(async move {
                if let Err(error) = serve_client(stream, auth).await {
                    tracing::warn!(error = %error, "token-control request failed");
                }
            });
        }
    });
    Ok(ControlGuard {
        task,
        path,
        socket_identity,
        _ownership: ownership,
    })
}

/// Print-or-mint command implementation shared with the binary grammar.
pub async fn show(state_dir: Option<PathBuf>) -> anyhow::Result<String> {
    let state_dir = state_dir_or_default(state_dir)?;
    farhelm_supervisor::ensure_private_dir(&state_dir).await?;
    let store = HelmStore::open(&state_dir.join("helm.db")).await?;
    auth::show_token(&store).await
}

/// Rotate in the serving helm when possible, falling back to an offline
/// database transaction when no helm owns the state directory.
pub async fn rotate(state_dir: Option<PathBuf>) -> anyhow::Result<String> {
    let state_dir = state_dir_or_default(state_dir)?;
    farhelm_supervisor::ensure_private_dir(&state_dir).await?;
    let socket_path = state_dir.join(SOCKET_NAME);
    let deadline = tokio::time::Instant::now() + CONTROL_DEADLINE;
    match connect_before(&socket_path, deadline).await {
        Ok(stream) => rotate_through_helm(stream, deadline).await,
        Err(error)
            if error.downcast_ref::<std::io::Error>().is_some_and(|error| {
                matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
                )
            }) =>
        {
            let ownership = loop {
                match acquire_ownership(state_dir.clone()).await {
                    Ok(ownership) => break ownership,
                    Err(error) if error.downcast_ref::<OwnershipBusy>().is_some() => {
                        match connect_before(&socket_path, deadline).await {
                            Ok(stream) => return rotate_through_helm(stream, deadline).await,
                            Err(connect_error)
                                if connect_error.downcast_ref::<std::io::Error>().is_some_and(
                                    |error| {
                                        matches!(
                                            error.kind(),
                                            std::io::ErrorKind::NotFound
                                                | std::io::ErrorKind::ConnectionRefused
                                        )
                                    },
                                ) => {}
                            Err(connect_error) => {
                                return Err(connect_error);
                            }
                        }
                        if tokio::time::Instant::now() >= deadline {
                            anyhow::bail!(
                                "token control is owned but {} did not become reachable",
                                socket_path.display()
                            );
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    }
                    Err(error) => return Err(error),
                }
            };
            match connect_before(&socket_path, deadline).await {
                Ok(stream) => {
                    drop(ownership);
                    return rotate_through_helm(stream, deadline).await;
                }
                Err(error)
                    if error.downcast_ref::<std::io::Error>().is_some_and(|error| {
                        matches!(
                            error.kind(),
                            std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
                        )
                    }) => {}
                Err(error) => return Err(error),
            }
            let store = HelmStore::open(&state_dir.join("helm.db")).await?;
            let token = auth::rotate_offline(&store).await?;
            drop(ownership);
            Ok(token)
        }
        Err(error) => Err(error),
    }
}

/// Connect before the operation-wide deadline while preserving I/O kinds for
/// the absent/refused offline-fallback decision.
async fn connect_before(
    path: &Path,
    deadline: tokio::time::Instant,
) -> anyhow::Result<tokio::net::UnixStream> {
    tokio::time::timeout_at(deadline, tokio::net::UnixStream::connect(path))
        .await
        .with_context(|| {
            format!(
                "timed out connecting to token-control socket {}",
                path.display()
            )
        })?
        .with_context(|| format!("connecting to token-control socket {}", path.display()))
}

async fn rotate_through_helm(
    mut stream: tokio::net::UnixStream,
    deadline: tokio::time::Instant,
) -> anyhow::Result<String> {
    verify_peer_uid(&stream)?;
    tokio::time::timeout_at(deadline, stream.write_all(b"rotate\n"))
        .await
        .context("timed out sending token rotation to the helm")?
        .context("sending token rotation to the helm")?;
    let mut reply = Vec::new();
    let mut reader = tokio::io::BufReader::new(stream).take(MAX_REPLY_BYTES + 1);
    let read = reader.read_until(b'\n', &mut reply);
    tokio::time::timeout_at(deadline, read)
        .await
        .context("timed out reading token rotation reply")?
        .context("reading token rotation reply")?;
    if reply.len() > usize::try_from(MAX_REPLY_BYTES).expect("reply limit fits usize") {
        anyhow::bail!("the helm returned an oversized token rotation reply");
    }
    if !reply.ends_with(b"\n") {
        anyhow::bail!("the helm returned an unterminated token rotation reply");
    }
    let reply = std::str::from_utf8(&reply)
        .context("the helm returned a non-UTF-8 token rotation reply")?;
    if let Some(token) = reply.strip_prefix("OK ").map(str::trim) {
        if token.is_empty() {
            anyhow::bail!("the helm returned an empty rotated token");
        }
        return Ok(token.to_string());
    }
    if let Some(detail) = reply.strip_prefix("ERR ").map(str::trim) {
        anyhow::bail!("the helm refused token rotation: {detail}");
    }
    anyhow::bail!("the helm returned an unreadable token rotation reply")
}

/// Refuse a socket not owned by the effective user before sending a command
/// or trusting any reply bytes.
///
/// tokio's `peer_cred` is the portability seam: it reads `SO_PEERCRED` on
/// Linux and `getpeereid` on the BSD family including macOS, both reporting
/// the peer's effective uid as of `connect`. A hand-rolled `SO_PEERCRED`
/// call here once made this file the only thing in the workspace that could
/// not compile for the Mac app.
fn verify_peer_uid(stream: &tokio::net::UnixStream) -> anyhow::Result<()> {
    let credentials = stream
        .peer_cred()
        .context("reading token-control peer credentials")?;
    // SAFETY: geteuid has no preconditions and reads process identity only.
    let expected = unsafe { libc::geteuid() };
    if credentials.uid() != expected {
        anyhow::bail!(
            "refusing token-control peer owned by uid {}; expected uid {expected}",
            credentials.uid()
        );
    }
    Ok(())
}

async fn serve_client(stream: tokio::net::UnixStream, auth: AuthState) -> anyhow::Result<()> {
    let (read, mut write) = stream.into_split();
    let mut command = String::new();
    tokio::io::BufReader::new(read)
        .take(64)
        .read_line(&mut command)
        .await
        .context("reading token-control command")?;
    let reply = if command.trim_end() != "rotate" {
        "ERR unsupported command\n".to_string()
    } else {
        match auth.rotate().await {
            Ok(token) => format!("OK {token}\n"),
            Err(error) => {
                tracing::error!(error = %error, "token rotation failed");
                format!("ERR {error}\n")
            }
        }
    };
    write
        .write_all(reply.as_bytes())
        .await
        .context("writing token-control reply")
}

fn state_dir_or_default(state_dir: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    state_dir.map_or_else(farhelm_supervisor::default_state_dir, Ok)
}

/// Try to claim token-control ownership without parking an async worker.
///
/// Serving helms retain the returned flock for their lifetime. CLI callers
/// treat [`OwnershipBusy`] as a reason to re-probe the socket, never as a
/// lock they can wait to acquire while that helm remains alive.
async fn acquire_ownership(state_dir: PathBuf) -> anyhow::Result<OwnershipLock> {
    tokio::task::spawn_blocking(move || {
        let path = state_dir.join(LOCK_NAME);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(&path)
            .with_context(|| format!("opening token-control lock {}", path.display()))?;
        let operation = libc::LOCK_EX | libc::LOCK_NB;
        // SAFETY: `file` owns a live fd for this call. flock changes kernel
        // advisory-lock state only and does not access Rust memory.
        let result = unsafe { libc::flock(file.as_raw_fd(), operation) };
        if result != 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::WouldBlock {
                return Err(OwnershipBusy.into());
            }
            return Err(anyhow::Error::new(error)
                .context(format!("locking token control for {}", state_dir.display())));
        }
        Ok(OwnershipLock { file })
    })
    .await
    .context("token-control lock task panicked")?
}

/// Accept failures caused by one aborted peer do not invalidate the listener;
/// every other error means the serving helm must stop with token control.
fn transient_accept_error(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::Interrupted
            | std::io::ErrorKind::WouldBlock
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::ConnectionReset
    )
}

/// Remove only an abandoned Unix socket while the caller holds ownership.
///
/// A live listener, regular file, or symlink at the control path is refused
/// rather than displaced or destroyed.
fn remove_stale_socket(path: &Path) -> anyhow::Result<()> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(anyhow::Error::new(error)
                .context(format!("inspecting token-control path {}", path.display())));
        }
    };
    if !metadata.file_type().is_socket() {
        anyhow::bail!(
            "refusing to replace non-socket token-control path {}",
            path.display()
        );
    }
    match std::os::unix::net::UnixStream::connect(path) {
        Ok(_) => anyhow::bail!(
            "a helm already owns token-control socket {}",
            path.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused => {}
        Err(error) => {
            return Err(anyhow::Error::new(error)
                .context(format!("probing token-control socket {}", path.display())));
        }
    }
    std::fs::remove_file(path)
        .with_context(|| format!("removing stale token-control socket {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The CLI-to-helm path must do all three parts of live rotation: commit a
    /// different token, delete device rows, and notify already-admitted
    /// sockets. Exercising the Unix protocol matters because a direct call to
    /// `AuthState::rotate` would not prove a separate CLI invocation reaches
    /// the process that owns those sockets.
    #[tokio::test]
    async fn live_rotation_reaches_the_serving_helm_and_revokes_sockets() {
        let dir = tempfile::tempdir().unwrap();
        let store = HelmStore::open(&dir.path().join("helm.db")).await.unwrap();
        let auth = AuthState::new(store.clone());
        let before = auth.token().await.unwrap();
        auth.mint_device().await.unwrap();
        let socket = auth.socket_session();
        let _guard = serve(dir.path(), auth).await.unwrap();

        let after = rotate(Some(dir.path().to_path_buf())).await.unwrap();
        assert_ne!(after, before);
        assert!(store.device_session_hashes().await.unwrap().is_empty());
        tokio::time::timeout(std::time::Duration::from_secs(1), socket.revoked())
            .await
            .expect("a live socket must be revoked as part of rotation");
    }

    /// Starting a second helm against the same state directory must not
    /// unlink the first helm's control path and silently steal future
    /// rotations from its live WebSockets.
    #[tokio::test]
    async fn a_live_token_control_socket_is_never_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let store = HelmStore::open(&dir.path().join("helm.db")).await.unwrap();
        let auth = AuthState::new(store);
        let _guard = serve(dir.path(), auth.clone()).await.unwrap();

        let error = match serve(dir.path(), auth).await {
            Ok(_) => panic!("the existing listener must keep the control path"),
            Err(error) => error,
        };
        assert!(format!("{error:#}").contains("another process owns token control"));
    }

    /// Both ordinary offline shapes—no path and an abandoned socket—rotate
    /// under the ownership lock and invalidate every retained device.
    #[tokio::test]
    async fn offline_rotation_handles_absent_and_orphaned_sockets() {
        for orphaned in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let store = HelmStore::open(&dir.path().join("helm.db")).await.unwrap();
            let auth = AuthState::new(store.clone());
            let before = auth.token().await.unwrap();
            auth.mint_device().await.unwrap();
            if orphaned {
                let listener =
                    std::os::unix::net::UnixListener::bind(dir.path().join(SOCKET_NAME)).unwrap();
                drop(listener);
            }

            let after = rotate(Some(dir.path().to_path_buf())).await.unwrap();
            assert_ne!(after, before);
            assert_eq!(store.device_session_count().await.unwrap(), 0);
        }
    }

    /// The ownership lock is claimed before the serving socket is bound. A
    /// CLI landing in that startup window re-probes the socket instead of
    /// waiting forever behind the lock the helm retains for its lifetime.
    #[tokio::test]
    async fn rotation_reprobes_while_a_serving_helm_finishes_startup() {
        let dir = tempfile::tempdir().unwrap();
        farhelm_supervisor::ensure_private_dir(dir.path())
            .await
            .unwrap();
        let ownership = acquire_ownership(dir.path().to_path_buf()).await.unwrap();
        let state_dir = dir.path().to_path_buf();
        let rotation = tokio::spawn(async move { rotate(Some(state_dir)).await });

        tokio::task::yield_now().await;
        let listener = tokio::net::UnixListener::bind(dir.path().join(SOCKET_NAME)).unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut command = [0; 7];
            stream.read_exact(&mut command).await.unwrap();
            assert_eq!(&command, b"rotate\n");
            stream
                .write_all(b"OK AAAAAAAAAAAAAAAAAAAAAA\n")
                .await
                .unwrap();
        });

        assert_eq!(rotation.await.unwrap().unwrap(), "AAAAAAAAAAAAAAAAAAAAAA");
        server.await.unwrap();
        drop(ownership);
    }

    /// The real listener refuses unknown vocabulary without treating it as a
    /// rotation or mutating either durable credential table.
    #[tokio::test]
    async fn unknown_control_command_returns_err_without_rotation() {
        let dir = tempfile::tempdir().unwrap();
        let store = HelmStore::open(&dir.path().join("helm.db")).await.unwrap();
        let auth = AuthState::new(store.clone());
        let before = auth.token().await.unwrap();
        auth.mint_device().await.unwrap();
        let before_devices = store.device_session_count().await.unwrap();
        let _guard = serve(dir.path(), auth).await.unwrap();

        let mut stream = tokio::net::UnixStream::connect(dir.path().join(SOCKET_NAME))
            .await
            .unwrap();
        stream.write_all(b"unknown\n").await.unwrap();
        let mut reply = String::new();
        tokio::io::BufReader::new(stream)
            .read_line(&mut reply)
            .await
            .unwrap();
        assert_eq!(reply, "ERR unsupported command\n");
        assert_eq!(auth::show_token(&store).await.unwrap(), before);
        assert_eq!(store.device_session_count().await.unwrap(), before_devices);
    }

    /// A live rotation failure tells the local operator which safe subsystem
    /// failed without copying raw SQLite text onto the control protocol.
    #[tokio::test]
    async fn rotation_failure_reply_is_actionable_and_sanitized() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("helm.db");
        let store = HelmStore::open(&path).await.unwrap();
        let auth = AuthState::new(store.clone());
        let before = auth.token().await.unwrap();
        auth.mint_device().await.unwrap();
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TRIGGER refuse_control_delete BEFORE DELETE ON device_sessions \
             BEGIN SELECT RAISE(ABORT, 'private sqlite detail'); END;",
        )
        .unwrap();
        let _guard = serve(dir.path(), auth).await.unwrap();

        let mut stream = tokio::net::UnixStream::connect(dir.path().join(SOCKET_NAME))
            .await
            .unwrap();
        stream.write_all(b"rotate\n").await.unwrap();
        let mut reply = String::new();
        tokio::io::BufReader::new(stream)
            .read_line(&mut reply)
            .await
            .unwrap();
        assert_eq!(
            reply,
            "ERR authentication storage is unavailable; inspect the helm logs\n"
        );
        assert!(!reply.contains("private sqlite detail"));
        assert_eq!(auth::show_token(&store).await.unwrap(), before);
        assert_eq!(store.device_session_count().await.unwrap(), 1);
    }

    /// A stale Unix socket is recoverable, but regular files and symlinks are
    /// never displaced merely because they occupy the conventional path.
    #[tokio::test]
    async fn startup_rebinds_only_an_abandoned_socket() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join(SOCKET_NAME);
        let abandoned = std::os::unix::net::UnixListener::bind(&socket_path).unwrap();
        drop(abandoned);
        let store = HelmStore::open(&dir.path().join("helm.db")).await.unwrap();
        let auth = AuthState::new(store);
        let guard = serve(dir.path(), auth).await.unwrap();
        assert!(std::os::unix::net::UnixStream::connect(&socket_path).is_ok());
        drop(guard);

        std::fs::write(&socket_path, "do not replace").unwrap();
        let store = HelmStore::open(&dir.path().join("helm.db")).await.unwrap();
        let error = match serve(dir.path(), AuthState::new(store)).await {
            Ok(_) => panic!("a regular file must not be replaced"),
            Err(error) => error,
        };
        assert!(format!("{error:#}").contains("refusing to replace non-socket"));
        std::fs::remove_file(&socket_path).unwrap();

        let target = dir.path().join("target");
        std::fs::write(&target, "target").unwrap();
        std::os::unix::fs::symlink(&target, &socket_path).unwrap();
        let store = HelmStore::open(&dir.path().join("helm.db")).await.unwrap();
        let error = match serve(dir.path(), AuthState::new(store)).await {
            Ok(_) => panic!("a symlink must not be replaced"),
            Err(error) => error,
        };
        assert!(format!("{error:#}").contains("refusing to replace non-socket"));
        assert_eq!(std::fs::read_to_string(target).unwrap(), "target");
    }

    /// Guard teardown compares inode ownership, so an externally installed
    /// replacement at the same pathname survives the old guard's drop.
    #[tokio::test]
    async fn guard_drop_does_not_unlink_a_replacement_socket() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(SOCKET_NAME);
        let store = HelmStore::open(&dir.path().join("helm.db")).await.unwrap();
        let guard = serve(dir.path(), AuthState::new(store)).await.unwrap();
        std::fs::remove_file(&path).unwrap();
        let replacement = std::os::unix::net::UnixListener::bind(&path).unwrap();
        drop(guard);
        assert!(path.exists());
        drop(replacement);
    }

    /// A compromised or mismatched server cannot make the CLI buffer an
    /// unlimited reply or accept EOF as a complete control response.
    #[tokio::test]
    async fn client_rejects_oversized_and_unterminated_replies() {
        for reply in [vec![b'x'; 129], b"OK short-without-newline".to_vec()] {
            let (client, mut server) = tokio::net::UnixStream::pair().unwrap();
            let writer = tokio::spawn(async move {
                let mut command = [0; 7];
                server.read_exact(&mut command).await.unwrap();
                assert_eq!(&command, b"rotate\n");
                server.write_all(&reply).await.unwrap();
            });
            let error = rotate_through_helm(client, tokio::time::Instant::now() + CONTROL_DEADLINE)
                .await
                .unwrap_err();
            let detail = format!("{error:#}");
            assert!(detail.contains("oversized") || detail.contains("unterminated"));
            writer.await.unwrap();
        }
    }

    /// Once connected, a silent serving peer is a failed live rotation—not
    /// evidence that the CLI may bypass it with an offline transaction.
    #[tokio::test]
    async fn established_control_exchange_has_one_whole_operation_deadline() {
        let (client, mut server) = tokio::net::UnixStream::pair().unwrap();
        let peer = tokio::spawn(async move {
            let mut command = [0; 7];
            server.read_exact(&mut command).await.unwrap();
            assert_eq!(&command, b"rotate\n");
            std::future::pending::<()>().await;
        });
        let error = rotate_through_helm(
            client,
            tokio::time::Instant::now() + std::time::Duration::from_millis(50),
        )
        .await
        .unwrap_err();
        assert!(format!("{error:#}").contains("timed out reading"));
        peer.abort();
    }
}
