//! Where to try a snapshot partway through a read.
//!
//! A snapshot can only be taken where the emulator is clean
//! (`Emulator::is_clean`): the parser between sequences with no partial
//! UTF-8 character, and no synchronized update open. Only then do the
//! snapshot plus the output after it rebuild the same state. Agent output is
//! rarely clean at the end of a read, because agents draw in synchronized
//! updates and a read ends wherever the pty's buffer did. The snapshot spike
//! found that reads of real agent output nonetheless had a clean point right
//! before their last update start, or else right before their last ESC.
//!
//! [`snapshot_split_candidate`] names that point from the bytes alone, and is
//! only a hint. The caller feeds the read up to the candidate, asks the
//! emulator itself whether it is clean, snapshots only if it is, and feeds the
//! rest; if it took no snapshot and the read ends clean, it can snapshot
//! there instead. Because the emulator is asked rather than predicted, a
//! wrong candidate costs one missed snapshot opportunity, never a snapshot
//! that cannot be continued.

/// The exact bytes of a synchronized update's start.
const UPDATE_START: &[u8] = b"\x1b[?2026h";

/// The exact bytes vte recognizes as a synchronized update's end.
const UPDATE_END: &[u8] = b"\x1b[?2026l";

/// The offset in `read` worth trying a snapshot at: just before the last
/// update start (`ESC [ ? 2026 h`, exactly) if no update end follows it in
/// the read, else just before the read's last ESC; `None` if the read has no
/// ESC.
///
/// A hint, not a promise: feed `read[..offset]`, check
/// `Emulator::is_clean`, and snapshot only if it says so (see the module
/// docs). Only the exact spellings of the start and end markers are
/// recognized; agents use those, and a miss only costs an opportunity.
pub fn snapshot_split_candidate(read: &[u8]) -> Option<usize> {
    if let Some(start) = rfind(read, UPDATE_START)
        && find(&read[start..], UPDATE_END).is_none()
    {
        return Some(start);
    }
    read.iter().rposition(|&byte| byte == 0x1b)
}

/// The first position of `needle` in `haystack`.
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// The last position of `needle` in `haystack`.
fn rfind(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .rposition(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The candidate is the last update start when it is still open at the
    /// read's end, else the last ESC, else nothing.
    ///
    /// Why: these are the points the snapshot spike found clean in reads of
    /// real agent output; a different rule would trade away those
    /// opportunities. Spec: `snapshot_split_candidate`.
    #[test]
    fn the_candidate_follows_the_rule() {
        let open_at_end = b"\x1b[?2026hone\x1b[?2026l text \x1b[?2026htwo\x1b[1m";
        assert_eq!(snapshot_split_candidate(open_at_end), Some(25));
        let closed = b"\x1b[?2026hone\x1b[?2026l text \x1b[3";
        assert_eq!(snapshot_split_candidate(closed), Some(closed.len() - 3));
        let ends_on_end_marker = b"text \x1b[?2026hone\x1b[?2026l";
        assert_eq!(
            snapshot_split_candidate(ends_on_end_marker),
            Some(ends_on_end_marker.len() - UPDATE_END.len())
        );
        assert_eq!(snapshot_split_candidate(b"plain text"), None);
        assert_eq!(snapshot_split_candidate(b""), None);
    }
}
