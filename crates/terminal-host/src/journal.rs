//! The journal: the latest snapshot plus the records after it, bounded by the
//! cap, and the reads that stream them to a client (SPEC.md, "Journal").
//!
//! Pure and synchronous: the event loop feeds it output, resizes and the
//! child's exit, and pulls one message at a time for the client as the socket
//! accepts it. Keeping it free of I/O is what lets the property tests drive it
//! through tens of thousands of random operations and compare it with a model
//! that keeps everything.
//!
//! Output and resizes are kept in two queues ordered by position rather than
//! one mixed queue: a read then finds its next record by comparing the heads
//! of two sorted sequences, and trimming can cut an output block in the
//! middle. The exit record lives outside both, because it is never trimmed
//! and is sent exactly once per read wherever the read starts.
//!
//! Output is stored in blocks of up to [`MAX_OUTPUT_CHUNK`] bytes that later
//! reads append to, not one allocation per pty read: a program writing a byte
//! at a time would otherwise cost far more memory in bookkeeping than in
//! output, and the cap only counts output. A block can therefore span a
//! resize or the exit; reads stop each `Output` at the next one.

use crate::protocol::{
    ChildExit, HostMessage, MAX_OUTPUT_CHUNK, MAX_SNAPSHOT, Position, RECORD_COST, Size,
    SnapshotRejection,
};
use std::collections::VecDeque;

/// One block of retained output: `data[skip..]` holds the bytes from
/// `offset` on. Trimming advances `skip` instead of moving the bytes, so
/// cutting a little off the front of a full block costs nothing; the skipped
/// bytes are freed with the block.
///
/// Every block reserves [`MAX_OUTPUT_CHUNK`] bytes when it is made. Only the
/// first can be partly trimmed and only the last partly filled, so the
/// blocks hold less than two blocks' worth beyond the retained output
/// (SPEC.md, "Resource bounds"), whatever the cap and however the program
/// writes.
#[derive(Debug)]
struct Block {
    offset: u64,
    data: Vec<u8>,
    skip: usize,
}

impl Block {
    fn len(&self) -> usize {
        self.data.len() - self.skip
    }

    fn bytes(&self) -> &[u8] {
        &self.data[self.skip..]
    }

    fn end(&self) -> u64 {
        self.offset + self.len() as u64
    }

    /// Drop `n` bytes (at most `len()`) from the front.
    fn cut_front(&mut self, n: usize) {
        self.skip += n;
        self.offset += n as u64;
    }
}

/// One retained resize.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Resize {
    offset: u64,
    index: u64,
    size: Size,
}

/// The journal of one terminal.
#[derive(Debug)]
pub struct Journal {
    cap: u64,
    /// The stored snapshot's position and blob; the position is (0, 0) and
    /// the blob `None` until one exists.
    snapshot: Position,
    blob: Option<Vec<u8>>,
    /// Retained output, contiguous and in order; the last block ends at
    /// `end`. No retained block is empty.
    output: VecDeque<Block>,
    /// Retained resizes, in order. Their indices are consecutive, and the
    /// first is `start.resizes`, so the resize with index `i` is at
    /// `i - start.resizes`.
    resizes: VecDeque<Resize>,
    /// The cost of what is retained (output bytes plus `RECORD_COST` per
    /// resize). Never above `cap` after an operation returns.
    cost: u64,
    /// Where retained records start: the first retained output offset (or
    /// `end` when none is retained), the first retained resize index, and the
    /// size in force there.
    start: Position,
    start_size: Size,
    /// Whether anything after the snapshot was trimmed: the tail then starts
    /// with a gap from the snapshot's position to `start`.
    trimmed: bool,
    /// The offset of the next output byte, and the number of resizes so far.
    end: u64,
    resize_count: u64,
    size: Size,
    /// The size given at spawn.
    initial_size: Size,
    /// The child's exit, at the position where it was journaled.
    exit: Option<(Position, ChildExit)>,
}

/// A client's progress through the journal.
#[derive(Debug, Clone)]
pub struct Cursor {
    state: CursorState,
    position: Position,
    exit_sent: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CursorState {
    /// Rebuild from scratch: snapshot record (if any), then gap (if any),
    /// then the tail.
    SendSnapshot,
    SendGap,
    /// Stream the records after `position`.
    Tail,
}

impl Cursor {
    /// The position this cursor has sent up to.
    pub fn position(&self) -> Position {
        self.position
    }
}

impl Journal {
    /// A journal for a terminal of `size`, with a tail budget of `cap`, and
    /// the initial snapshot (at (0, 0)) if any.
    pub fn new(size: Size, cap: u64, initial_snapshot: Option<Vec<u8>>) -> Self {
        Self {
            cap,
            snapshot: Position::default(),
            blob: initial_snapshot,
            output: VecDeque::new(),
            resizes: VecDeque::new(),
            cost: 0,
            start: Position::default(),
            start_size: size,
            trimmed: false,
            end: 0,
            resize_count: 0,
            size,
            initial_size: size,
            exit: None,
        }
    }

    /// The offset of the next output byte.
    pub fn end(&self) -> u64 {
        self.end
    }

    /// The number of resizes applied so far.
    pub fn resize_count(&self) -> u64 {
        self.resize_count
    }

    /// The terminal size in force now.
    pub fn size(&self) -> Size {
        self.size
    }

    /// The terminal size given at spawn, which `Info` reports: output that
    /// a read sends after neither a snapshot record nor a gap starts at it.
    pub fn initial_size(&self) -> Size {
        self.initial_size
    }

    /// The stored snapshot's position.
    pub fn snapshot_position(&self) -> Position {
        self.snapshot
    }

    /// Where retained output starts: the end, when none is retained.
    pub fn tail_start(&self) -> u64 {
        self.start.offset
    }

    /// Where retained records start: the first retained output offset and
    /// the first retained resize index. A read or snapshot position must be
    /// at or after it.
    pub fn retained_start(&self) -> Position {
        self.start
    }

    /// Whether the tail starts with a gap.
    pub fn has_gap(&self) -> bool {
        self.trimmed
    }

    /// The cost of what is retained, for tests of the bound.
    pub fn cost(&self) -> u64 {
        self.cost
    }

    /// Append output read from the pty, filling the last block before
    /// starting another.
    pub fn push_output(&mut self, mut data: &[u8]) {
        while !data.is_empty() {
            let room = self
                .output
                .back()
                .map_or(0, |block| MAX_OUTPUT_CHUNK - block.data.len());
            if room == 0 {
                self.output.push_back(Block {
                    offset: self.end,
                    data: Vec::with_capacity(MAX_OUTPUT_CHUNK),
                    skip: 0,
                });
                continue;
            }
            let take = data.len().min(room);
            let block = self.output.back_mut().expect("just ensured");
            block.data.extend_from_slice(&data[..take]);
            self.end += take as u64;
            self.cost += take as u64;
            data = &data[take..];
        }
        self.trim();
    }

    /// Record a resize applied now, at the current offset.
    pub fn push_resize(&mut self, size: Size) {
        self.resizes.push_back(Resize {
            offset: self.end,
            index: self.resize_count,
            size,
        });
        self.resize_count += 1;
        self.size = size;
        self.cost += RECORD_COST;
        self.trim();
    }

    /// Record the child's exit at the current position. Only the first call
    /// counts: a host reaps its one child once.
    pub fn push_exit(&mut self, exit: ChildExit) {
        if self.exit.is_none() {
            self.exit = Some((self.current(), exit));
        }
    }

    /// The child's exit, if it was journaled.
    pub fn exit(&self) -> Option<ChildExit> {
        self.exit.map(|(_, exit)| exit)
    }

    /// The position after everything journaled so far.
    pub fn current(&self) -> Position {
        Position {
            offset: self.end,
            resizes: self.resize_count,
        }
    }

    /// Accept a client's snapshot at `position`, or say why not (SPEC.md,
    /// "Snapshots"). On rejection nothing changes.
    pub fn accept_snapshot(
        &mut self,
        position: Position,
        blob: Vec<u8>,
    ) -> Result<(), SnapshotRejection> {
        if blob.len() > MAX_SNAPSHOT {
            return Err(SnapshotRejection::TooLarge);
        }
        if position.offset < self.snapshot.offset || position.resizes < self.snapshot.resizes {
            return Err(SnapshotRejection::BeforeCurrent);
        }
        if position.offset < self.start.offset || position.offset > self.end {
            return Err(SnapshotRejection::OutsideRetained);
        }
        if !self.consistent(position) {
            return Err(SnapshotRejection::InconsistentResizes);
        }
        // Drop everything before the position.
        while let Some(block) = self.output.front_mut() {
            if block.end() <= position.offset {
                self.cost -= block.len() as u64;
                self.output.pop_front();
            } else {
                let cut = (position.offset.saturating_sub(block.offset)) as usize;
                if cut > 0 {
                    block.cut_front(cut);
                    self.cost -= cut as u64;
                }
                break;
            }
        }
        while let Some(resize) = self.resizes.front() {
            if resize.index >= position.resizes {
                break;
            }
            self.start_size = resize.size;
            self.cost -= RECORD_COST;
            self.resizes.pop_front();
        }
        self.snapshot = position;
        self.blob = Some(blob);
        self.start = position;
        self.trimmed = false;
        Ok(())
    }

    /// Whether `position` names a point consistent with the retained
    /// resizes: within them (at or after the first retained one, at most the
    /// count), with those below it at or before its offset and those at or
    /// above it at or after.
    ///
    /// Retained resizes are in offset order, so only the two around the
    /// position need checking: the last one below it and the first one at or
    /// above it. A periodic snapshot of a journal with a large cap would
    /// otherwise scan millions of them.
    fn consistent(&self, position: Position) -> bool {
        if position.resizes < self.start.resizes || position.resizes > self.resize_count {
            return false;
        }
        let at = (position.resizes - self.start.resizes) as usize;
        let below = at
            .checked_sub(1)
            .and_then(|below| self.resizes.get(below))
            .is_none_or(|resize| resize.offset <= position.offset);
        let above = self
            .resizes
            .get(at)
            .is_none_or(|resize| resize.offset >= position.offset);
        below && above
    }

    /// Start a read from `position`, the point the client has processed
    /// (SPEC.md, "Reading").
    ///
    /// A read at exactly the stored snapshot's position is answered with the
    /// snapshot: that is where a client with nothing starts, (0, 0), when the
    /// host was spawned with an initial snapshot, and for a client that does
    /// have the snapshot, receiving it again is only redundant.
    pub fn read(&self, position: Position) -> Cursor {
        let at_snapshot = self.blob.is_some() && position == self.snapshot;
        let within = !at_snapshot
            && position.offset >= self.start.offset
            && position.offset <= self.end
            && self.consistent(position);
        if within {
            Cursor {
                state: CursorState::Tail,
                position,
                exit_sent: false,
            }
        } else {
            Cursor {
                state: CursorState::SendSnapshot,
                position: self.snapshot,
                exit_sent: false,
            }
        }
    }

    /// The next message for `cursor`, if there is one now. Call again after
    /// more is journaled to continue the stream.
    pub fn next(&self, cursor: &mut Cursor) -> Option<HostMessage> {
        loop {
            match cursor.state {
                CursorState::SendSnapshot => {
                    cursor.state = CursorState::SendGap;
                    cursor.position = self.snapshot;
                    if let Some(blob) = &self.blob {
                        return Some(HostMessage::SnapshotRecord {
                            position: self.snapshot,
                            blob: blob.clone(),
                        });
                    }
                }
                CursorState::SendGap => {
                    cursor.state = CursorState::Tail;
                    let from = cursor.position.offset;
                    // The tail no longer starts where the snapshot this
                    // cursor sent ends when output was trimmed, or when a
                    // newer snapshot was accepted after this cursor sent its
                    // older one: either way what lies between is gone, and
                    // the client must be told rather than skipped past it.
                    let superseded = cursor.position != self.start;
                    cursor.position = self.start;
                    if self.trimmed || superseded {
                        return Some(self.gap_message(from));
                    }
                }
                CursorState::Tail => return self.next_in_tail(cursor),
            }
        }
    }

    fn gap_message(&self, from: u64) -> HostMessage {
        HostMessage::Gap {
            from,
            to: self.start.offset,
            resizes: self.start.resizes,
            size: self.start_size,
        }
    }

    fn next_in_tail(&self, cursor: &mut Cursor) -> Option<HostMessage> {
        // A cursor whose next records were trimmed (or replaced by a
        // snapshot) jumps over them with a gap.
        if cursor.position.offset < self.start.offset
            || cursor.position.resizes < self.start.resizes
        {
            let from = cursor.position.offset;
            cursor.position = self.start;
            return Some(self.gap_message(from));
        }
        if let Some((at, exit)) = self.exit
            && !cursor.exit_sent
            && at.offset <= cursor.position.offset
            && at.resizes <= cursor.position.resizes
        {
            cursor.exit_sent = true;
            return Some(HostMessage::Exited {
                offset: at.offset,
                exit,
            });
        }
        // A resize at the cursor's offset comes before output there. The
        // check above put the cursor at or after the first retained resize.
        let next_resize = self
            .resizes
            .get((cursor.position.resizes - self.start.resizes) as usize);
        if let Some(resize) = next_resize
            && resize.offset <= cursor.position.offset
        {
            cursor.position.resizes += 1;
            return Some(HostMessage::Resized {
                offset: resize.offset,
                index: resize.index,
                size: resize.size,
            });
        }
        if cursor.position.offset >= self.end {
            return None;
        }
        // Output stops at the next resize and at the exit (when this
        // cursor has not sent it yet), since a block can span either.
        let mut limit = next_resize.map_or(self.end, |resize| resize.offset);
        if let Some((at, _)) = self.exit
            && !cursor.exit_sent
            && at.offset > cursor.position.offset
        {
            limit = limit.min(at.offset);
        }
        let index = self
            .output
            .partition_point(|block| block.end() <= cursor.position.offset);
        let block = &self.output[index];
        let from = (cursor.position.offset - block.offset) as usize;
        let to = ((limit - block.offset) as usize).min(block.len());
        let data = block.bytes()[from..to].to_vec();
        let start = cursor.position.offset;
        cursor.position.offset += data.len() as u64;
        Some(HostMessage::Output {
            offset: start,
            data,
        })
    }

    /// Drop the oldest records until the cost fits the cap. Output is cut
    /// from the front of its first block; the order of output and resizes is
    /// kept by always dropping whichever comes first, and by never cutting
    /// output past a retained resize.
    fn trim(&mut self) {
        while self.cost > self.cap {
            let excess = self.cost - self.cap;
            let resize_first = match (self.resizes.front(), self.output.front()) {
                (Some(resize), Some(block)) => resize.offset <= block.offset,
                (Some(_), None) => true,
                (None, Some(_)) => false,
                (None, None) => unreachable!("cost above zero with nothing retained"),
            };
            self.trimmed = true;
            if resize_first {
                let resize = self.resizes.pop_front().expect("checked");
                self.cost -= RECORD_COST;
                self.start_size = resize.size;
                self.start.resizes = resize.index + 1;
            } else {
                // A resize can sit inside the front block; output after it
                // is trimmed only once the resize itself has gone, or the
                // retained resize would come before retained output.
                let before_resize = self
                    .resizes
                    .front()
                    .map_or(u64::MAX, |resize| resize.offset);
                let block = self.output.front_mut().expect("checked");
                let cut = excess
                    .min(block.len() as u64)
                    .min(before_resize - block.offset) as usize;
                block.cut_front(cut);
                self.cost -= cut as u64;
                if block.len() == 0 {
                    self.output.pop_front();
                }
            }
            self.start.offset = self.output.front().map_or(self.end, |block| block.offset);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A program writing a byte at a time costs a few blocks, not one
    /// allocation per byte.
    ///
    /// Why: the cap counts only output bytes, so per-read bookkeeping would
    /// let a journal well within its cap hold many times its cap in memory,
    /// for weeks (SPEC.md, "Resource bounds"). Spec: after a million one-byte
    /// appends to a 512 KiB journal (so trimming runs throughout), and to
    /// one whose cap is two bytes over one block (so a partly trimmed and a
    /// partly filled block sit on either side of a full one), the blocks
    /// reserve less than two blocks' worth beyond the retained output, which
    /// is the cap.
    #[test]
    fn single_byte_writes_are_packed_into_blocks() {
        for cap in [1 << 19, MAX_OUTPUT_CHUNK as u64 + 2] {
            let mut journal = Journal::new(Size { cols: 80, rows: 24 }, cap, None);
            for i in 0..1_000_000u32 {
                journal.push_output(&[i as u8]);
            }
            assert_eq!(journal.cost(), cap);
            let reserved: usize = journal.output.iter().map(|b| b.data.capacity()).sum();
            assert!(
                reserved < cap as usize + 2 * MAX_OUTPUT_CHUNK,
                "{reserved} bytes reserved for {cap} retained"
            );
        }
    }
}
