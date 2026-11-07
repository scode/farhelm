//! One connection's channel routes and the history that prevents queued bytes
//! from changing meaning when a producer ends.
//!
//! Routing and reservation have different lifetimes. Terminal ids become free
//! when their route goes away; upload tombstones still answer late commits but
//! do not reserve ids. Download and relay ids stay reserved until disconnect,
//! because their old bulk frames can outlive the task. Even a refused non-relay
//! attempt rules out a later relay on that id: frames already sent for the
//! attempt must never become bytes for a newly accepted TCP stream.

use super::downloads::DownloadRoute;
use super::tcp_relays::RelayRoute;
use super::terminals::InputRoute;
use super::uploads::{UploadRoute, prune_finished_uploads};
use std::collections::{HashMap, HashSet};

/// Own every route and reservation in one connection's channel-id space.
///
/// Checks remain separate operations because their placement is observable:
/// relay reservations are refused before download reservations, and both precede
/// attachment lease validation and upload admission. Collapsing those checks
/// into one handler refusal would change which error a malformed request gets.
/// Allocation stays on the reader, so no later frame can race route installation.
#[derive(Default)]
pub(crate) struct ConnectionChannels {
    // Tunnel listeners and undecided sockets must disappear even when the
    // reader unwinds before reaching its explicit asynchronous shutdown tail.
    pub(crate) tunnels: super::tcp_relays::TunnelClaims,
    // A cancelled or panicking reader skips its explicit shutdown tail. Rust
    // drops fields in declaration order, so retain the reverse declaration
    // order of the former connection locals even on that path. Normal shutdown
    // separately drops transfer maps before attachment cleanup.
    non_relay_ids: HashSet<u32>,
    relay_ids: HashSet<u32>,
    relays: HashMap<u32, RelayRoute>,
    download_ids: HashSet<u32>,
    downloads: HashMap<u32, DownloadRoute>,
    uploads: HashMap<u32, UploadRoute>,
    inputs: HashMap<u32, InputRoute>,
}

impl ConnectionChannels {
    /// Remember full-authority attach/upload/download attempts before validation.
    /// Session-authenticated refusals must not add history to this table.
    pub(crate) fn note_non_relay_attempt(&mut self, channel: u32) {
        self.non_relay_ids.insert(channel);
    }

    /// A TCP stream gets an id only if no earlier stream or attempt used it.
    /// Non-relay history includes failed attempts, not merely installed routes.
    pub(crate) fn can_begin_relay(&self, channel: u32) -> bool {
        channel != 0 && !self.non_relay_ids.contains(&channel) && !self.relay_ids.contains(&channel)
    }

    /// Keep the id reserved before starting its producer, including failed starts.
    pub(crate) fn begin_relay(&mut self, channel: u32, start: impl FnOnce() -> RelayRoute) {
        self.reserve_relay(channel);
        self.relays.insert(channel, start());
    }

    /// Refused accepted-socket admissions also reserve their channel: late
    /// bytes from that attempt must never feed a later accepted connection.
    pub(crate) fn reserve_relay(&mut self, channel: u32) {
        self.relay_ids.insert(channel);
    }

    /// Old relay data still belongs to this id after its route is removed.
    pub(crate) fn relay_reserved(&self, channel: u32) -> bool {
        self.relay_ids.contains(&channel)
    }

    /// Look up a live TCP route without exposing the reservation history.
    pub(crate) fn relay(&self, channel: u32) -> Option<&RelayRoute> {
        self.relays.get(&channel)
    }

    /// Cancel only the producer; queued relay frames still reserve the id.
    pub(crate) fn close_relay(&mut self, channel: u32) {
        self.relays.remove(&channel);
    }

    /// Disconnect cancels every socket pump before any unrelated teardown
    /// wait. Return unfinished task ids so the connection can reap those
    /// cancellations and prove sockets closed before touching other cleanup.
    /// Already-finished tasks may have been reaped by the ordinary reader.
    pub(crate) fn close_relays(&mut self) -> HashSet<tokio::task::Id> {
        let pending = self
            .relays
            .values()
            .filter(|route| !route.task.is_finished())
            .map(|route| route.task.id())
            .collect();
        self.relays.clear();
        pending
    }

    /// Reserve before spawning, so even a refused file read keeps its id.
    pub(crate) fn begin_download(&mut self, channel: u32, start: impl FnOnce() -> DownloadRoute) {
        self.download_ids.insert(channel);
        self.downloads.insert(channel, start());
    }

    /// A completed or aborted download can still have bytes in the writer.
    pub(crate) fn download_reserved(&self, channel: u32) -> bool {
        self.download_ids.contains(&channel)
    }

    /// Credit affects only the current producer, never its retained reservation.
    pub(crate) fn download(&self, channel: u32) -> Option<&DownloadRoute> {
        self.downloads.get(&channel)
    }

    /// Cancel the producer without letting its queued bytes acquire a new owner.
    pub(crate) fn abort_download(&mut self, channel: u32) {
        self.downloads.remove(&channel);
    }

    /// Check the terminal/upload/download id space after pre-dispatch refusals.
    /// Finished uploads are diagnostic tombstones and do not count as in use.
    /// Historical reservations are checked here too, so an allocating handler
    /// cannot accidentally accept one if called without the reader's prechecks.
    pub(crate) fn can_begin_stream(&self, channel: u32) -> bool {
        channel != 0
            && !self.inputs.contains_key(&channel)
            && !self.uploads.get(&channel).is_some_and(UploadRoute::is_live)
            && !self.relay_reserved(channel)
            && !self.download_reserved(channel)
    }

    /// Install an attachment whose inline handler already checked this table.
    pub(crate) fn attach(&mut self, channel: u32, route: InputRoute) {
        self.inputs.insert(channel, route);
    }

    /// Input and resize use the attachment's exact session/terminal ownership key.
    pub(crate) fn input(&self, channel: u32) -> Option<&InputRoute> {
        self.inputs.get(&channel)
    }

    /// Removing a terminal route makes its id reusable by terminal/upload traffic.
    pub(crate) fn detach(&mut self, channel: u32) -> Option<InputRoute> {
        self.inputs.remove(&channel)
    }

    /// Count running uploads, excluding tombstones that only retain diagnostics.
    pub(crate) fn live_uploads(&self) -> usize {
        self.uploads
            .values()
            .filter(|route| route.is_live())
            .count()
    }

    /// Replace any tombstone only after the begin handler has passed validation.
    pub(crate) fn begin_upload(&mut self, channel: u32, route: UploadRoute) {
        self.uploads.insert(channel, route);
    }

    /// Keep tombstones visible to commit handling as well as live chunk routing.
    pub(crate) fn upload(&self, channel: u32) -> Option<&UploadRoute> {
        self.uploads.get(&channel)
    }

    /// Commit and abort change the transfer's answer state, not id allocation.
    pub(crate) fn upload_mut(&mut self, channel: u32) -> Option<&mut UploadRoute> {
        self.uploads.get_mut(&channel)
    }

    /// Shutdown signals every retained upload before dropping any route sender.
    pub(crate) fn uploads(&self) -> impl Iterator<Item = &UploadRoute> {
        self.uploads.values()
    }

    /// Reclaim finished producers without releasing relay/download reservations.
    /// Upload pruning retains its bounded diagnostic tombstones.
    pub(crate) fn prune_finished(&mut self) {
        self.downloads.retain(|_, route| !route.task.is_finished());
        self.relays.retain(|_, route| !route.task.is_finished());
        prune_finished_uploads(&mut self.uploads);
    }

    /// Preserve shutdown's upload, download, relay drop order before writer drain.
    /// Release the maps' allocations too; terminal routes stay alive until
    /// attachment cleanup has completed.
    pub(crate) fn drop_transfers(&mut self) {
        drop(std::mem::take(&mut self.uploads));
        drop(std::mem::take(&mut self.downloads));
        drop(std::mem::take(&mut self.relays));
    }

    /// Direct-dispatch tests can verify refusals installed no terminal route.
    #[cfg(test)]
    pub(crate) fn has_inputs(&self) -> bool {
        !self.inputs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::ConnectionChannels;

    /// A refused non-relay begin leaves no route, but any queued bytes from
    /// that attempt must never become a TCP stream's input. Retrying the id
    /// for a terminal or file stream remains allowed.
    #[test]
    fn a_non_relay_attempt_reserves_only_against_later_relays() {
        let mut channels = ConnectionChannels::default();
        assert!(channels.can_begin_relay(7));
        assert!(channels.can_begin_stream(7));

        channels.note_non_relay_attempt(7);

        assert!(channels.input(7).is_none());
        assert!(channels.upload(7).is_none());
        assert!(channels.download(7).is_none());
        assert!(!channels.can_begin_relay(7));
        assert!(channels.can_begin_stream(7));
    }

    /// An unknown accepted-socket id still spends a fresh relay channel. A
    /// refusal must not let later bytes for that attempt become a different
    /// socket's input, even though no route was installed by the failed begin.
    #[test]
    fn a_refused_relay_channel_cannot_be_reused_by_any_stream() {
        let mut channels = ConnectionChannels::default();
        assert!(channels.can_begin_relay(7));
        channels.reserve_relay(7);
        assert!(channels.relay(7).is_none());
        assert!(!channels.can_begin_relay(7));
        assert!(!channels.can_begin_stream(7));
        channels.prune_finished();
        channels.drop_transfers();
        assert!(channels.relay_reserved(7));
    }
}
