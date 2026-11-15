//! A second launch of the Linux desktop app brings the running app's window forward instead of refusing.
//!
//! The app lock already makes a second launch on the same state directory refuse ([`crate::refuse_if_app_open`]); on
//! Linux that refusal was all a user got for launching an app that was already open, behind other windows, say. Now the
//! running app listens on a Unix socket in its state directory, and a second launch that reaches it hands over its
//! `XDG_ACTIVATION_TOKEN`, if its launcher gave it one, and exits. The running app then activates its main window with
//! that token: on Wayland a compositor moves focus only for a valid token, not for a window that merely asks
//! (settled on 2026-11-13 by the GPUI migration's coordinator, under the maintainer's rule that the new Linux app
//! follows its platform; the vendored `activation-token` layer is kept for this).
//! The Mac is unchanged: Launch Services brings an installed `Farhelm.app` forward by itself, and a bare development
//! binary keeps the refusal.
//!
//! One connection is one request; its only payload is the token, and there is no acknowledgement, because the second
//! launch has nothing to do beyond knowing that something accepted. A launch that cannot reach the socket (no app open,
//! a socket left by a crashed app, a path too long for a socket address) starts normally and meets the app lock as
//! before, so nothing here can make a launch go wrong that the lock would have refused. Ported from the GPUI prototype's
//! `single_instance.rs`.

use std::io::{self, Read as _, Write as _};
use std::os::unix::fs::PermissionsExt as _;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::time::Duration;

/// The socket's file name in the app's state directory: one per state directory, so two apps on two state directories
/// never meet, and no fixed path outside it. No longer than `supervisor.sock`, which shares the directory, so any state
/// directory whose supervisor socket fits the Unix socket path limit fits this one too (SPEC_impl.md, "Terminal
/// host").
pub(crate) const SOCKET_FILE: &str = "desktop.sock";

/// One second launch: its activation token, when its launcher gave it one.
pub(crate) type Activation = Option<String>;

/// The longest token read from one connection. Real tokens are a few dozen bytes; the cap only keeps a misbehaving
/// client from growing memory.
const MAX_TOKEN: u64 = 4096;

/// Ask a running app to bring its window forward; `true` means one accepted the request, so this launch should exit.
///
/// `token` is this process's `XDG_ACTIVATION_TOKEN`, read by the caller and passed in so tests never touch the
/// environment. A write that fails after the connection was accepted still counts: the running app then activates
/// without a token, as for a launch that had none. Every connect error means "nothing to forward to". This never
/// removes a file, so a starting launch cannot delete a live app's socket.
pub(crate) fn forward_to_running(path: &Path, token: Option<&str>) -> bool {
    let Ok(mut stream) = UnixStream::connect(path) else {
        return false;
    };
    if let Some(token) = token {
        let _ = stream.write_all(token.as_bytes());
    }
    true
}

/// Serve second launches for this process's life, one request per connection, as a channel of activations.
///
/// Call only once this launch owns the state directory (startup succeeded, so its embedded helm holds the app lock):
/// from then on any file at `path` is a leftover of a crashed app and is replaced. The accept loop is a plain thread
/// that ends when the receiver is gone and the next request cannot be sent. Reading a token is bounded in size and in
/// time, so a client that never closes cannot hold up the next launch.
pub(crate) fn listen(path: &Path) -> io::Result<tokio::sync::mpsc::UnboundedReceiver<Activation>> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    let listener = UnixListener::bind(path)?;
    // The state directory is already private (0700); the socket's own mode keeps it so should the directory ever be
    // looser. Another user could at worst raise this window.
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    let (send, receive) = tokio::sync::mpsc::unbounded_channel();
    std::thread::Builder::new()
        .name("farhelm-second-launch".into())
        .spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else {
                    // A lasting error (out of file descriptors, say) would fail every accept at once and spin.
                    std::thread::sleep(Duration::from_millis(100));
                    continue;
                };
                let mut token = String::new();
                let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
                let _ = stream.take(MAX_TOKEN).read_to_string(&mut token);
                let token = Some(token.trim().to_owned()).filter(|token| !token.is_empty());
                if send.send(token).is_err() {
                    break;
                }
            }
        })?;
    Ok(receive)
}

/// Stop accepting second launches as the app quits. A connection succeeds as soon as the kernel queues it, so while
/// the app shuts down a launch would otherwise hand over its token, exit, and leave nothing running; with the file gone
/// it starts normally and meets the app lock until the old helm has let go. Errors are ignored: a missing file is the
/// goal.
pub(crate) fn retire(path: &Path) {
    let _ = std::fs::remove_file(path);
}

/// The window's side of the channel: a "next activation" future per call, as the GUI crate's
/// `desktop::Activations` takes it. One receiver serves every call, taken out for the wait and put back after (calls
/// come one at a time; a wait dropped halfway, only at the app's end, takes it with it and ends the feed).
pub(crate) fn activations(
    receiver: tokio::sync::mpsc::UnboundedReceiver<Activation>,
) -> farhelm_gui::desktop::Activations {
    let kept = std::rc::Rc::new(std::cell::RefCell::new(Some(receiver)));
    farhelm_gui::desktop::Activations {
        next: Box::new(move || {
            let kept = std::rc::Rc::clone(&kept);
            Box::pin(async move {
                let mut receiver = kept.borrow_mut().take()?;
                let activation = receiver.recv().await?;
                *kept.borrow_mut() = Some(receiver);
                Some(activation)
            })
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A second launch must hand over and exit only when a live app accepts, and its activation token must arrive
    /// whole. A socket file left by a crashed app must read as "nothing open", so that launch starts normally and can
    /// replace the file; a launch without a token is still a request; and once the app has retired its socket as it
    /// quits, a launch starts normally even though the accept thread still runs. Specified over a real socket in a
    /// temporary directory: a leftover file refuses, the listener replaces it at mode 0600, tokens and token-less
    /// requests arrive in order, and a retired socket refuses.
    #[farhelm_testtrace::test]
    fn a_second_launch_hands_over_only_to_a_live_app_and_carries_its_token() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(SOCKET_FILE);
        drop(UnixListener::bind(&path).unwrap());
        assert!(
            path.exists(),
            "the premise: a socket file with no listener, as a crashed app leaves"
        );
        assert!(!forward_to_running(&path, Some("unused")));

        let mut requests = listen(&path).unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(forward_to_running(&path, Some("launcher-token-1")));
        assert_eq!(
            requests.blocking_recv(),
            Some(Some("launcher-token-1".into()))
        );
        assert!(forward_to_running(&path, None));
        assert_eq!(requests.blocking_recv(), Some(None));

        retire(&path);
        assert!(!forward_to_running(&path, Some("too late")));
    }

    /// The window waits on the activations one at a time, so the adapter must hand each over once, in order, and end
    /// when the app stops listening. Polled by hand, with no tokio runtime, as GPUI's executor polls it. Specified: a
    /// queued activation resolves the next call with its token; the end of the channel ends the feed.
    #[farhelm_testtrace::test]
    fn the_activations_reach_the_window_in_order_and_end_with_the_channel() {
        use std::task::{Context, Poll, Waker};
        let (send, receive) = tokio::sync::mpsc::unbounded_channel();
        let mut activations = activations(receive);
        let mut context = Context::from_waker(Waker::noop());
        send.send(Some("token".to_string())).unwrap();
        send.send(None).unwrap();
        drop(send);
        for expected in [Some(Some("token".to_string())), Some(None), None] {
            let mut next = (activations.next)();
            assert_eq!(next.as_mut().poll(&mut context), Poll::Ready(expected));
        }
    }
}
