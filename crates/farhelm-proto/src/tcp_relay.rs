//! Native bookkeeping for one bidirectional TCP channel. TCP read boundaries
//! are arbitrary: a byte window must not become a seventeen-packet limit just
//! because file transfers normally fill their frames. Both peers use this
//! bounded byte inbox and cumulative credit, while retaining their own writer
//! queues, task ownership, and socket lifetimes.

use crate::{UPLOAD_CHUNK_BYTES, UPLOAD_WINDOW_BYTES};
use std::collections::VecDeque;
use std::sync::Mutex;
use tokio::sync::watch;

/// Shared state between a connection's nonblocking demultiplexer and its TCP
/// pumps. No method used by demux waits for a socket or a consumer. Credit is
/// returned only after the destination socket accepts the bytes; dequeuing
/// alone does not reopen the peer's window.
pub struct TcpRelayState {
    inner: Mutex<Inner>,
    changed: watch::Sender<u64>,
}

/// Counts have independent directions: sent/acked describe our producer,
/// accepted/written describe the peer's. The inbox excludes an in-progress
/// socket write, but accepted minus written includes it in the byte bound.
#[derive(Default)]
struct Inner {
    inbox: VecDeque<u8>,
    accepted: u64,
    written: u64,
    sent: u64,
    acked: u64,
    end: Option<Result<(), String>>,
}

impl Default for TcpRelayState {
    fn default() -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
            changed: watch::channel(0).0,
        }
    }
}

impl TcpRelayState {
    /// Append a short or full data frame without waiting. Packet count is
    /// immaterial; oversized frames or a peer exceeding byte credit end only
    /// this relay. Late bytes after an ending are discarded.
    pub fn push(&self, bytes: &[u8]) {
        let mut inner = self.inner.lock().expect("TCP relay mutex poisoned");
        if inner.end.is_some() {
            return;
        }
        let accepted = inner.accepted.checked_add(bytes.len() as u64);
        if bytes.len() > UPLOAD_CHUNK_BYTES
            || accepted.is_none_or(|total| total - inner.written > UPLOAD_WINDOW_BYTES)
        {
            inner.end = Some(Err(
                "TCP relay exceeded its receive window or chunk limit".into()
            ));
        } else {
            inner.accepted = accepted.expect("the bound above checked overflow");
            inner.inbox.extend(bytes);
        }
        drop(inner);
        self.wake();
    }

    /// Take at most one frame's bytes. A successful peer EOF drains the inbox
    /// first; a failure is immediate. Only one socket-writing consumer may
    /// call this method for a relay.
    pub async fn next(&self) -> Result<Option<Vec<u8>>, String> {
        let mut changed = self.changed.subscribe();
        loop {
            changed.borrow_and_update();
            {
                let mut inner = self.inner.lock().expect("TCP relay mutex poisoned");
                if let Some(Err(reason)) = &inner.end {
                    return Err(reason.clone());
                }
                if !inner.inbox.is_empty() {
                    let count = inner.inbox.len().min(UPLOAD_CHUNK_BYTES);
                    return Ok(Some(inner.inbox.drain(..count).collect()));
                }
                if inner.end.is_some() {
                    return Ok(None);
                }
            }
            changed
                .changed()
                .await
                .map_err(|_| "TCP relay closed".to_string())?;
        }
    }

    /// Record bytes actually written to the destination and return cumulative
    /// credit for the peer. The sole consumer supplies exactly the chunk it
    /// just wrote; this does not acknowledge merely buffered bytes.
    pub fn written(&self, count: usize) -> u64 {
        let mut inner = self.inner.lock().expect("TCP relay mutex poisoned");
        inner.written += count as u64;
        assert!(
            inner.written <= inner.accepted,
            "cannot credit unwritten relay bytes"
        );
        inner.written
    }

    /// Reserve outbound bytes before enqueueing their frame, so a prompt peer
    /// acknowledgement cannot outrun the sent counter. The initial window is
    /// implicit in Started; no clock runs while a websocket is idle.
    pub async fn reserve(&self, count: usize) -> Result<(), String> {
        if count > UPLOAD_CHUNK_BYTES {
            return Err("TCP relay chunk exceeds the limit".into());
        }
        let mut changed = self.changed.subscribe();
        loop {
            changed.borrow_and_update();
            {
                let mut inner = self.inner.lock().expect("TCP relay mutex poisoned");
                if let Some(end) = &inner.end {
                    return Err(end
                        .clone()
                        .err()
                        .unwrap_or_else(|| "TCP relay ended".into()));
                }
                if inner.sent - inner.acked + count as u64 <= UPLOAD_WINDOW_BYTES {
                    inner.sent = inner
                        .sent
                        .checked_add(count as u64)
                        .ok_or_else(|| "TCP relay byte count overflow".to_string())?;
                    return Ok(());
                }
            }
            changed
                .changed()
                .await
                .map_err(|_| "TCP relay closed".to_string())?;
        }
    }

    /// Validate cumulative outgoing credit. Duplicates grant nothing, and a
    /// regressing or invented count fails the channel rather than granting
    /// memory to an untrusted peer.
    pub fn ack(&self, received: u64) {
        let mut inner = self.inner.lock().expect("TCP relay mutex poisoned");
        if inner.end.is_some() || received == inner.acked {
            return;
        }
        if received < inner.acked || received > inner.sent {
            inner.end = Some(Err("invalid TCP relay credit".into()));
        } else {
            inner.acked = received;
        }
        drop(inner);
        self.wake();
    }

    /// Publish the first terminal outcome. Keeping success and failure in the
    /// same slot prevents a late EOF from concealing an earlier refusal.
    pub fn finish(&self, result: Result<(), String>) {
        let mut inner = self.inner.lock().expect("TCP relay mutex poisoned");
        if inner.end.is_none() {
            inner.end = Some(result);
        }
        drop(inner);
        self.wake();
    }

    /// Snapshot the first outcome without consuming it. A socket pump which
    /// wakes on successful remote EOF must allow the opposite pump to drain
    /// already-received bytes before closing the local socket.
    pub fn outcome(&self) -> Option<Result<(), String>> {
        self.inner
            .lock()
            .expect("TCP relay mutex poisoned")
            .end
            .clone()
    }

    /// Resolve only on failure, including while a socket write is parked.
    /// Successful EOF is handled by next(), which must drain earlier data.
    pub async fn failed(&self) -> String {
        let mut changed = self.changed.subscribe();
        loop {
            changed.borrow_and_update();
            if let Some(Err(reason)) = self.outcome() {
                return reason;
            }
            changed
                .changed()
                .await
                .expect("the relay owns its change sender");
        }
    }

    /// A version watch avoids lost wakeups between inspecting the inbox and
    /// subscribing. The value itself has no protocol meaning.
    fn wake(&self) {
        self.changed
            .send_modify(|version| *version = version.wrapping_add(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Legal tiny TCP reads must consume byte credit rather than exhaust a
    /// file-transfer queue's seventeen frame slots. EOF preserves the bytes.
    #[farhelm_testtrace::test]
    async fn fragmented_frames_use_byte_capacity_and_drain_before_eof() {
        let state = TcpRelayState::default();
        for _ in 0..1000 {
            state.push(b"x");
        }
        state.finish(Ok(()));
        let bytes = state.next().await.unwrap().unwrap();
        assert_eq!(bytes, vec![b'x'; 1000]);
        assert_eq!(state.written(bytes.len()), 1000);
        assert!(state.next().await.unwrap().is_none());
    }

    /// A receiver cannot acknowledge unsent bytes to manufacture credit, and
    /// successful EOF cannot overwrite the protocol violation.
    #[farhelm_testtrace::test]
    async fn invalid_credit_and_receive_overflow_keep_the_first_failure() {
        let state = TcpRelayState::default();
        state.reserve(3).await.unwrap();
        state.ack(4);
        state.finish(Ok(()));
        assert_eq!(state.next().await.unwrap_err(), "invalid TCP relay credit");
        let state = TcpRelayState::default();
        let chunk = vec![0; UPLOAD_CHUNK_BYTES];
        for _ in 0..UPLOAD_WINDOW_BYTES as usize / UPLOAD_CHUNK_BYTES {
            state.push(&chunk);
        }
        state.push(b"x");
        assert!(state.next().await.unwrap_err().contains("receive window"));
    }

    /// Exactly one window may be outstanding. Repeating the last ack cannot
    /// release another byte, but genuine consumption reopens the window.
    #[farhelm_testtrace::test]
    async fn only_advancing_credit_releases_the_next_byte() {
        use std::future::Future;
        let state = TcpRelayState::default();
        for _ in 0..UPLOAD_WINDOW_BYTES as usize / UPLOAD_CHUNK_BYTES {
            state.reserve(UPLOAD_CHUNK_BYTES).await.unwrap();
        }
        let next = state.reserve(1);
        tokio::pin!(next);
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        assert!(next.as_mut().poll(&mut context).is_pending());
        state.ack(0);
        assert!(next.as_mut().poll(&mut context).is_pending());
        state.ack(1);
        next.await.unwrap();
        state.ack(2);
        state.ack(1);
        assert_eq!(state.failed().await, "invalid TCP relay credit");
    }
}
