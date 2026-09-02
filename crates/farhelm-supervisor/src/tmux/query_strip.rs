//! Remove terminal queries that tmux has already answered from live output.
//!
//! A program in a Farhelm pane can ask its terminal a question such as the
//! cursor position or the current background colour. tmux answers that
//! question inside the pane, so the program gets the answer it is waiting
//! for. The same question also appears in tmux control mode's raw `%output`
//! notification. If Farhelm forwards it to the browser, xterm.js answers a
//! second time; that late answer travels back to the pane after the program
//! has exited, where the shell sees it as typed input. This type removes the
//! question from the live stream before the browser can answer it again.
//!
//! The supervisor owns this transform because it is the one place that knows
//! which bytes came from tmux's live pane stream, and the result then behaves
//! consistently for every rendering client. The table is deliberately the
//! exact set answered by the pinned tmux version. It contains literal byte
//! strings, not a grammar: a sequence with parameters or a similar-looking
//! terminal protocol must pass through because tmux may not answer it.
//! Payload unwrapped from a tmux passthrough wrapper bypasses this matcher:
//! tmux forwarded that payload without parsing or answering it, so filtering
//! it would leave the pane program waiting for an answer that never arrives.
//! Zero-padded CSI spellings such as `CSI 06n` are deliberately outside this
//! table even when tmux treats them like their canonical counterparts.
//!
//! Queries can be split across control-mode notifications, so the matcher
//! holds a possible prefix until the next notification. That hold-back is
//! bounded by the longest table entry. The stream gives it an absolute idle
//! deadline measured from the owning pane's latest ordinary payload, so shared
//! control traffic cannot extend the wait; EOF and `%exit` flush it before the
//! stream ends. This module must never become a VT parser: it does not
//! interpret parameters, terminal state, or any sequence outside this fixed
//! table. A table entry split by longer than that idle window is deliberately
//! forwarded and answered twice rather than held indefinitely.

const QUERY_ENTRIES: &[&[u8]] = &[
    b"\x1b[6n",
    b"\x1b[5n",
    b"\x1b[c",
    b"\x1b[0c",
    b"\x1b[>c",
    b"\x1b[>0c",
    b"\x1b]10;?\x07",
    b"\x1b]10;?\x1b\\",
    b"\x1b]11;?\x07",
    b"\x1b]11;?\x1b\\",
];

/// The longest number of bytes held while a query may still be completed.
#[cfg(test)]
pub(crate) const MAX_HOLD_BACK: usize = 7;

/// Strip only the fixed terminal queries tmux answers from a byte stream.
///
/// `feed` returns bytes that are no longer part of a possible table entry.
/// It may therefore return an empty vector even when input was supplied.
/// `flush` makes an incomplete prefix ordinary output and clears it; callers
/// use that when the stream is idle or ends. The matcher is stateful because
/// control mode is allowed to split one pane notification at any byte.
#[derive(Default)]
pub(crate) struct QueryStripper {
    hold_back: Vec<u8>,
}

impl QueryStripper {
    /// Feed live pane bytes and return all bytes that cannot be a query.
    ///
    /// Every input byte takes one branch-and-copy step. Only ESC candidates
    /// consult the table, so ordinary pane floods avoid extra matching work.
    pub(crate) fn feed(&mut self, bytes: &[u8]) -> Vec<u8> {
        let mut output = Vec::with_capacity(bytes.len());
        for &byte in bytes {
            if self.hold_back.is_empty() && byte != 0x1b {
                output.push(byte);
                continue;
            }
            self.hold_back.push(byte);
            self.resolve_hold_back(&mut output);
        }
        output
    }

    /// Return and clear bytes retained as a possible incomplete query.
    pub(crate) fn flush(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.hold_back)
    }

    /// Report whether an idle read should wait for more bytes or flush now.
    pub(crate) fn has_pending(&self) -> bool {
        !self.hold_back.is_empty()
    }

    /// Emit leading bytes until the suffix is either complete or still viable.
    fn resolve_hold_back(&mut self, output: &mut Vec<u8>) {
        loop {
            if is_complete_query(&self.hold_back) {
                self.hold_back.clear();
                return;
            }
            if is_possible_prefix(&self.hold_back) {
                return;
            }
            output.push(self.hold_back.remove(0));
        }
    }
}

fn is_complete_query(candidate: &[u8]) -> bool {
    QUERY_ENTRIES.contains(&candidate)
}

fn is_possible_prefix(candidate: &[u8]) -> bool {
    QUERY_ENTRIES
        .iter()
        .any(|entry| entry.starts_with(candidate) && candidate.len() < entry.len())
}

/// Return the exact query/terminator entries used by the unit and tmux guards.
#[cfg(test)]
pub(crate) fn all_entries() -> &'static [&'static [u8]] {
    QUERY_ENTRIES
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every table entry is removed without changing ordinary bytes around it;
    /// this protects the user-visible stream from both leakage and corruption.
    #[test]
    fn removes_each_query_between_ordinary_text() {
        for entry in all_entries() {
            let mut stripper = QueryStripper::default();
            let mut input = b"before".to_vec();
            input.extend_from_slice(entry);
            input.extend_from_slice(b"after");
            let mut output = stripper.feed(&input);
            output.extend(stripper.flush());
            assert_eq!(output, b"beforeafter", "query leaked: {entry:?}");
        }
    }

    /// Every notification split must have the same result as one notification,
    /// because tmux is free to divide a pane payload at any byte boundary.
    #[test]
    fn every_transcript_split_matches_the_unsplit_result() {
        let mut transcript = b"start \x1b[".to_vec();
        for entry in all_entries() {
            transcript.extend_from_slice(b" ordinary ");
            transcript.extend_from_slice(entry);
        }
        transcript.extend_from_slice(
            b" \x1b[2004$p \x1b[2004h \x1b \x1b]11;r g b : 0 0 0 0 / 0 0 0 0 \x1b\\ end",
        );

        let mut whole = QueryStripper::default();
        let mut expected = whole.feed(&transcript);
        expected.extend(whole.flush());

        for split in 0..=transcript.len() {
            let mut stripper = QueryStripper::default();
            let mut output = stripper.feed(&transcript[..split]);
            output.extend(stripper.feed(&transcript[split..]));
            output.extend(stripper.flush());
            assert_eq!(output, expected, "split point {split} changed output");
        }
    }

    /// Similar control sequences and ordinary escape bytes must pass through;
    /// the fixed table must not quietly turn into a terminal parser.
    #[test]
    fn decoys_pass_through_untouched() {
        let decoys = b"\x1b[\x1b[?2004$p\x1b[?2004h\x1b\x1b]11;r g b : 0 0 0 0 / 0 0 0 0\x1b\\";
        let mut stripper = QueryStripper::default();
        let mut output = stripper.feed(decoys);
        output.extend(stripper.flush());
        assert_eq!(output, decoys);
    }

    /// The pending suffix is bounded even when fed one byte at a time.
    #[test]
    fn hold_back_never_exceeds_the_table_bound() {
        let mut stripper = QueryStripper::default();
        for entry in all_entries() {
            for length in 0..entry.len() {
                stripper.feed(&entry[..length]);
                assert!(
                    stripper.hold_back.len() <= MAX_HOLD_BACK,
                    "held {} bytes for prefix length {length}",
                    stripper.hold_back.len()
                );
                stripper.flush();
            }
        }
    }

    /// A partial prefix is ordinary output once the caller decides no more
    /// bytes should be waited for, and a second flush must be empty.
    #[test]
    fn flush_returns_and_clears_partial_prefix() {
        let mut stripper = QueryStripper::default();
        assert!(stripper.feed(b"\x1b]11;").is_empty());
        assert_eq!(stripper.flush(), b"\x1b]11;");
        assert!(stripper.flush().is_empty());
    }
}
