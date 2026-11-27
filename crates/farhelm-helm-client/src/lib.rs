//! The GPUI desktop app's connection to its helm.
//!
//! NOTE: This crate knows nothing about GPUI, and must not. It is the one place the GUI's async runtime lives
//! (SPEC_impl.md, "Where the GUI meets the helm"): the GUI crates may not spawn threads or runtimes, so everything
//! that needs sockets, timers or HTTP runs here, and the GUI sees only plain channels and futures that any executor
//! can poll.
//!
//! The pieces, by the order a window meets them:
//!
//! - [`HelmConnection`] owns the runtime and is what the app creates once it knows its helm's address and device
//!   credential.
//! - [`channel`] is the UI channel: one ordered WebSocket that carries every command and pushes every collection
//!   change. Its [`channel::ChannelEvents`] stream is the single ordered feed the GUI drains; a command's completion
//!   resolves only after every event the helm sent before its reply has been handed out.
//! - [`collections`] applies that feed to keyed collections with one generic code path.
//! - [`terminal`] attaches terminal streams (the agent terminal and tabs) and reconnects them, on a reconnect ladder of
//!   their own whose every wait and attempt the terminal's outage card is told about
//!   ([`terminal::ReconnectStatus`]).
//! - `reconnect` holds the two reconnect policies: the UI channel's doubling backoff and the terminal ladder.
//! - [`transfers`] is the plain-HTTP side: a file path's hover check, saving a file into Downloads, reading a small
//!   file's contents for the clipboard, and attachment uploads. The transfers stay off the UI channel so a large one
//!   never delays a command, and the hover check does because a channel command cannot be cancelled on its own (see
//!   its module docs).
//! - [`ApiPath`] (from `farhelm-proto`, where the helm decodes it too) is the only constructor of a command path.
//! - `fake` stands in for the helm in GUI tests, in process and without a runtime: the UI channel, terminal streams
//!   through `fake::terminal::FakeTerminal`, and file transfers through `fake::transfers::FakeTransfers`. It exists
//!   only in this crate's own tests and with the `test-support` feature, which the GUI turns on for its tests alone,
//!   so the app never compiles it.

pub mod channel;
pub mod collections;
// The crate docs above name `fake` in plain code, not as links, which would break in every build without the feature.
#[cfg(any(test, feature = "test-support"))]
pub mod fake;
mod reconnect;
pub mod terminal;
pub mod transfers;
mod ws;

pub use channel::{
    ChannelConfig, ChannelEvent, ChannelEvents, ChannelHandle, ChannelState, CommandError,
    CommandFuture, Lane, Method,
};
pub use collections::{Collections, Pushed, PushedChanges, PushedEntry, PushedState};
pub use farhelm_proto::ui_channel::{ApiPath, PathError};
pub use transfers::{
    FileStat, OpenOutcome, SavedDownload, Transfer, TransferError, Transfers, UploadOutcome,
    UploadSource,
};
pub use ws::{ConnectError, HelmEndpoint};

use std::sync::Arc;

/// The GUI's handle on one helm: a private tokio runtime plus the UI channel and the file transfers running on it.
///
/// Everything this hands out (channel handles, event receivers, command futures, terminal streams, transfers) is
/// runtime-agnostic: the GUI polls it on its own executor and never enters this runtime. Dropping the connection
/// stops the runtime without waiting for its tasks, so a window closing never blocks on a socket.
pub struct HelmConnection {
    /// `Some` for the connection's whole life; `Drop` takes it to shut it down without waiting.
    runtime: Option<tokio::runtime::Runtime>,
    /// The runtime's handle, for starting work on it. Kept beside the runtime so starting work never has to unwrap the
    /// `Option` above, which is empty only inside `Drop`.
    handle: tokio::runtime::Handle,
    endpoint: HelmEndpoint,
    channel: ChannelHandle,
    transfers: Transfers,
}

impl HelmConnection {
    /// Start the runtime, open the UI channel and start serving file transfers. Returns the connection and the
    /// channel's event feed, which the caller must keep draining (see [`ChannelEvents`]).
    pub fn open(
        endpoint: HelmEndpoint,
        config: ChannelConfig,
    ) -> std::io::Result<(HelmConnection, ChannelEvents)> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("farhelm-helm-client")
            .enable_all()
            .build()?;
        let connector = Arc::new(ws::UiChannelConnector::new(
            endpoint.clone(),
            config.watchdog,
        ));
        let (channel, events, transfers) = {
            let _guard = runtime.enter();
            let (channel, events) = channel::spawn_channel(connector, config);
            (channel, events, transfers::spawn(endpoint.clone()))
        };
        Ok((
            HelmConnection {
                handle: runtime.handle().clone(),
                runtime: Some(runtime),
                endpoint,
                channel,
                transfers,
            },
            events,
        ))
    }

    /// The UI channel. Clones share the one channel.
    pub fn channel(&self) -> &ChannelHandle {
        &self.channel
    }

    /// File transfers to this helm. Clones share one HTTP client.
    pub fn transfers(&self) -> &Transfers {
        &self.transfers
    }

    /// Attach one terminal stream. See [`terminal::open_terminal`].
    pub fn terminal(
        &self,
        attach: terminal::TerminalAttach,
        config: ChannelConfig,
    ) -> (terminal::TerminalHandle, terminal::TerminalEvents) {
        let _guard = self.handle.enter();
        terminal::open_terminal(self.endpoint.clone(), attach, config)
    }
}

impl Drop for HelmConnection {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}
