//! One read at a time, latest wins: the request state the launcher's debounced reads share.
//!
//! The managed checkout's repository suggestions and preview (`checkout`) and the folder check (`folder_check`) each
//! ask the helm about something the user is still changing (what is typed, the host, its connection). Each follows the
//! same rules, which [`Latest`] keeps once:
//!
//! - **Latest wins.** An answer is kept only while the launcher still asks what it was asked; any change forgets the
//!   earlier answer at once, so nothing about an old key shows a moment longer, and going back to a key asks again.
//! - **One at a time.** A read is not sent while another of its kind is out; the next goes out when that one answers,
//!   for the key current then. This keeps typing from piling requests onto the ordered UI channel, where each would
//!   wait behind the last.
//!
//! What stays with each read is everything that differs: building and sending its request (`operations::send`),
//! decoding the answer, checking which installation it came from, and what an accepted answer changes besides itself.
//! Browse (`browse`) is not one of these: every press sends, and nothing waits for a read that is out.

use crate::ticker;

/// One kind of read, keyed by what it asks about: the last key asked, its answer while it is still the current one,
/// whether a read is out and whether another is due when it answers, and the debounce deadline, if the read has one.
pub(super) struct Latest<K, A> {
    /// The key the pending deadline, the read out, or the last answer is about.
    asked: Option<K>,
    /// The answer for `asked`, once it came.
    answered: Option<A>,
    /// Whether a read is out, which holds the next one until it answers. A reset drops the answer's wait, leaving this
    /// set, but a reset drops the launcher too (drafts do not survive it), so nothing is left waiting on it.
    out: bool,
    /// Whether the read out still belongs to this generation. Returning to the same key starts a new generation.
    out_current: bool,
    /// Whether a read for the current key came due while one was out: its answer then sends the read for the current
    /// key. A key change clears it, since the change's own deadline or send will ask, so one change never sends two.
    due: bool,
    /// The debounce deadline before the next read, for a read that waits for the input to settle; dropping it cancels
    /// it.
    waiting: Option<ticker::Deadline>,
}

/// What finishing a read ([`Latest::finish`]) leaves its caller to do.
pub(super) struct Finished<K> {
    /// Whether the answer was kept: the read was about the current key and brought an answer to keep. Only then does
    /// the caller act on it.
    pub(super) kept: bool,
    /// The current key, when a read for it came due while this one was out: the caller sends it now.
    pub(super) next: Option<K>,
}

impl<K, A> Default for Latest<K, A> {
    fn default() -> Self {
        Latest {
            asked: None,
            answered: None,
            out: false,
            out_current: false,
            due: false,
            waiting: None,
        }
    }
}

impl<K: Clone + PartialEq, A> Latest<K, A> {
    /// Follow `key`, what the read would ask about now: returns the key to read (now, or after a debounce) when it
    /// changed, else `None`. A change forgets the answer for the old key, cancels a pending deadline and drops a read
    /// marked due, so nothing about the old key shows or goes out.
    pub(super) fn follow(&mut self, key: Option<K>) -> Option<K> {
        if key == self.asked {
            return None;
        }
        self.asked = key.clone();
        self.out_current = false;
        self.answered = None;
        self.waiting = None;
        self.due = false;
        key
    }

    /// Hold `deadline` as the debounce before the next read: it sends the read when it fires, and a key change drops
    /// it, which cancels it.
    pub(super) fn wait(&mut self, deadline: ticker::Deadline) {
        self.waiting = Some(deadline);
    }

    /// The answer, while it is about `key`.
    pub(super) fn answer_for(&self, key: &K) -> Option<&A> {
        (self.asked.as_ref() == Some(key))
            .then_some(self.answered.as_ref())
            .flatten()
    }

    /// Whether the current key is awaiting an answer, including the debounce and the time before a slot opens.
    /// Returns `None` for an unrelated key. A read the caller could not send stays pending until the key changes.
    pub(super) fn pending_for(&self, key: &K) -> Option<bool> {
        (self.asked.as_ref() == Some(key)).then_some(self.answered.is_none())
    }

    /// Take the slot for a read about `key`: `true` when the caller sends it now, which marks a read out. A key that is
    /// no longer current is not sent; while a read is out, another is marked due instead, which that read's answer
    /// sends ([`Latest::finish`]).
    pub(super) fn begin(&mut self, key: &K) -> bool {
        if self.asked.as_ref() != Some(key) {
            return false;
        }
        if self.out {
            self.due = true;
            return false;
        }
        self.out = true;
        self.out_current = true;
        true
    }

    /// The read [`Latest::begin`] allowed could not be sent after all (no UI session): free the slot it took.
    pub(super) fn unsent(&mut self) {
        self.out = false;
        self.out_current = false;
    }

    /// The read about `key` answered with `answer`, `None` when it brought nothing to keep (an inconclusive answer):
    /// free the slot, and keep the answer only while `key` is still the current key. Says whether it was kept, and
    /// which key to send now if a read came due meanwhile.
    pub(super) fn finish(&mut self, key: &K, answer: Option<A>) -> Finished<K> {
        self.out = false;
        let kept = std::mem::take(&mut self.out_current)
            && self.asked.as_ref() == Some(key)
            && answer.is_some();
        if kept {
            self.answered = answer;
        }
        let next = if std::mem::take(&mut self.due) {
            self.asked.clone()
        } else {
            None
        };
        Finished { kept, next }
    }
}

#[cfg(test)]
mod tests {
    use super::Latest;

    /// A read state that has followed `key`, as a launcher's draw does before the read's deadline fires.
    fn following(key: &str) -> Latest<String, &'static str> {
        let mut latest = Latest::default();
        assert_eq!(latest.follow(Some(key.to_owned())), Some(key.to_owned()));
        latest
    }

    /// A key the launcher still asks about goes out at once, and following the same key again asks nothing new. Why:
    /// each frame follows the key, and a read per frame would flood the helm.
    #[test]
    fn the_current_key_begins_once() {
        let mut latest = following("a");
        assert_eq!(
            latest.follow(Some("a".to_owned())),
            None,
            "an unchanged key asks nothing"
        );
        assert!(latest.begin(&"a".to_owned()), "the current key goes out");
        assert!(
            !latest.begin(&"b".to_owned()),
            "a key no longer current never does"
        );
    }

    /// While a read is out, a read for the newer key waits and goes out when the first answers, for the key current
    /// then. Why: one read at a time keeps typing from piling requests onto the ordered UI channel, yet the newest key
    /// must still be asked.
    #[test]
    fn a_read_due_while_one_is_out_goes_out_when_it_answers() {
        let mut latest = following("a");
        assert!(latest.begin(&"a".to_owned()));
        latest.follow(Some("b".to_owned()));
        assert!(
            !latest.begin(&"b".to_owned()),
            "the newer key waits while a read is out"
        );
        let finished = latest.finish(&"a".to_owned(), Some("for a"));
        assert!(!finished.kept, "the answer for the old key is not kept");
        assert_eq!(
            finished.next.as_deref(),
            Some("b"),
            "the newer key is sent now"
        );
        assert!(latest.begin(&"b".to_owned()), "and the slot is free for it");
    }

    /// An answer for the key still asked about is kept and shown for that key only, and an inconclusive one keeps
    /// nothing. Why: "latest wins" means only the current key's answer ever shows, and the folder check shows nothing
    /// for a timeout or a busy host.
    #[test]
    fn an_answer_for_the_current_key_is_kept() {
        let mut latest = following("a");
        assert!(latest.begin(&"a".to_owned()));
        let finished = latest.finish(&"a".to_owned(), Some("for a"));
        assert!(finished.kept);
        assert_eq!(finished.next, None, "nothing came due");
        assert_eq!(latest.answer_for(&"a".to_owned()), Some(&"for a"));
        assert_eq!(latest.answer_for(&"b".to_owned()), None);

        let mut inconclusive = following("a");
        assert!(inconclusive.begin(&"a".to_owned()));
        assert!(!inconclusive.finish(&"a".to_owned(), None).kept);
        assert_eq!(inconclusive.answer_for(&"a".to_owned()), None);
    }

    /// A key change drops a read marked due for the earlier key: the change's own deadline asks, so one change never
    /// sends two. Why: otherwise the answer still out and the new key's deadline would both send the newest key.
    #[test]
    fn a_key_change_drops_a_due_read() {
        let mut latest = following("a");
        assert!(latest.begin(&"a".to_owned()), "the premise: a is sent");
        latest.follow(Some("b".to_owned()));
        assert!(
            !latest.begin(&"b".to_owned()),
            "the premise: b is marked due"
        );
        latest.follow(Some("c".to_owned()));
        assert_eq!(
            latest.finish(&"a".to_owned(), None).next,
            None,
            "a's answer sends nothing"
        );
        assert!(latest.begin(&"c".to_owned()), "c's own deadline sends it");
    }

    /// An answer that arrives after the key changed is dropped, and the change forgot the earlier answer at once. Why:
    /// a late answer must never bring back what the user has moved away from (SPEC.md: "Late responses cannot restore
    /// its authority").
    #[test]
    fn an_answer_for_a_stale_key_is_dropped() {
        let mut latest = following("a");
        assert!(latest.begin(&"a".to_owned()));
        assert!(latest.finish(&"a".to_owned(), Some("first")).kept);
        latest.follow(Some("b".to_owned()));
        assert_eq!(
            latest.answer_for(&"a".to_owned()),
            None,
            "a key change forgets the old answer at once"
        );
        assert!(latest.begin(&"b".to_owned()));
        latest.follow(Some("c".to_owned()));
        // Returning to b owes a fresh read. The first b's answer cannot settle the new generation.
        latest.follow(Some("b".to_owned()));
        assert!(!latest.begin(&"b".to_owned()), "the new read is due");
        let finished = latest.finish(&"b".to_owned(), Some("for b"));
        assert!(!finished.kept);
        assert_eq!(finished.next.as_deref(), Some("b"));
        assert_eq!(latest.answer_for(&"b".to_owned()), None);
        assert_eq!(latest.pending_for(&"b".to_owned()), Some(true));
        assert_eq!(latest.pending_for(&"c".to_owned()), None);
    }

    /// A read that could not be sent frees its slot, so the next one is not held behind an answer that will never come.
    /// Why: without a UI session `operations::send` sends nothing and never answers.
    #[test]
    fn a_send_failure_frees_the_slot() {
        let mut latest = following("a");
        assert!(latest.begin(&"a".to_owned()));
        latest.unsent();
        assert!(latest.begin(&"a".to_owned()), "the slot is free again");
    }
}
