//! What feeding output makes the emulator do outside itself, and which of it
//! happens in each feed mode.
//!
//! Interpreting a program's output has two kinds of consequences beyond the
//! screen: answers to the program's queries, which must be written back to the
//! program, and notices for the user (a bell, a clipboard write, a new
//! title). Both are fine the first time the bytes are interpreted and wrong
//! when the same bytes are interpreted again to rebuild an emulator after a
//! supervisor restart: the program got its answers long ago, and the user
//! already heard the bell. [`FeedMode`] says which situation a feed is in, and
//! `route` applies the policy.
//!
//! During a feed, alacritty and this crate's query handler queue effects in
//! the order the output caused them (`Listener`), and the emulator routes the
//! queue to the caller's [`Effects`] once the feed's bytes are parsed. Order
//! matters for answers: a program that asks two questions reads the answers
//! in the order it asked.

use std::mem;
use std::sync::{Arc, Mutex, PoisonError};

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::term::ClipboardType;

/// Where the bytes being fed come from, which decides what their side effects
/// may do.
///
/// The host keeps a journal of the program's output and knows how far it
/// delivered it to a supervisor. A recovering supervisor rebuilds its emulator
/// from a snapshot plus the journal's tail, and passes [`FeedMode::ReplaySeen`]
/// for the part a previous supervisor already received and
/// [`FeedMode::ReplayUnseen`] for the rest.
///
/// The mode applies to the effects of the parsing a call does, not to the
/// bytes it passes. The difference shows in a synchronized update: vte holds
/// its bytes and interprets them all when it ends, so their effects follow
/// the mode of the feed (or flush) that ends it. An update that straddles the
/// end of the seen part is therefore handled as unseen, and a query inside its
/// seen part is answered a second time. That is part of the delivery
/// ambiguity a supervisor crash leaves anyway; tracking a mode per buffered
/// byte would mean reaching into vte's buffer.
///
/// The same rule applies at the end of a recovery: an update opened during
/// [`FeedMode::ReplayUnseen`] and ended by the first live feed would ring its
/// bells and write its clipboard live, long after the fact. So recovery must
/// end with [`crate::Emulator::flush_sync`] in [`FeedMode::ReplayUnseen`]
/// before the first live feed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedMode {
    /// Normal operation: answers go to the program, and bells, clipboard
    /// writes and title changes are reported.
    Live,
    /// Bytes a previous supervisor already received and acted on. Nothing is
    /// answered or reported: the program had its answers and the user their
    /// bells.
    ReplaySeen,
    /// Bytes no supervisor received, such as output during a supervisor's
    /// downtime. Queries are answered, because the program may still be
    /// waiting for them. Bells and clipboard writes are dropped: nobody was
    /// there to see them when they happened, and acting on them now, possibly
    /// minutes later, would surprise the user. Title changes are not reported
    /// either; the caller reads the title once the replay is over.
    ReplayUnseen,
}

/// A side effect meant for the user rather than the program.
///
/// Only [`FeedMode::Live`] reports these. A title change is never reported
/// during a replay; once a replay ends, the caller reads
/// [`crate::Emulator::title`] instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// The program rang the bell (BEL).
    Bell,
    /// The program wrote to a clipboard with OSC 52. Reads (`OSC 52 ; c ; ?`)
    /// are never answered, so this is the only clipboard effect there is.
    Clipboard {
        /// Which clipboard the program named.
        target: ClipboardTarget,
        /// The decoded text. alacritty drops a write whose payload is not
        /// valid base64 of UTF-8 text before it gets here.
        text: String,
    },
    /// The program set or reset its title (OSC 0 or 2, a pop from the title
    /// stack, or a full reset, RIS, that cleared it). The new value is
    /// [`crate::Emulator::title`].
    Title,
}

/// The clipboard an OSC 52 write names: `c` is the clipboard, `p` and `s`
/// the selection (alacritty's mapping; it drops any other name).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardTarget {
    Clipboard,
    Selection,
}

/// Receives what feeding output makes the emulator do, already filtered by
/// the feed's [`FeedMode`].
pub trait Effects {
    /// Bytes to write to the program's input, as if the terminal had typed
    /// them: answers to its queries. Called once per answer, in the order the
    /// program asked.
    fn reply(&mut self, bytes: &[u8]);

    /// Something to tell the user.
    fn notice(&mut self, notice: Notice);
}

/// An [`Effects`] that keeps everything, in order: what a caller that acts
/// on effects after the feed returns needs, and what tests inspect.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Collected {
    /// Every answer, concatenated in the order the program asked.
    pub replies: Vec<u8>,
    /// Every notice, in order.
    pub notices: Vec<Notice>,
}

impl Effects for Collected {
    fn reply(&mut self, bytes: &[u8]) {
        self.replies.extend_from_slice(bytes);
    }

    fn notice(&mut self, notice: Notice) {
        self.notices.push(notice);
    }
}

/// One queued side effect, before the feed mode is applied.
#[derive(Debug)]
pub(crate) enum Effect {
    Reply(String),
    Bell,
    Clipboard(ClipboardTarget, String),
    Title,
}

/// The emulator's `alacritty_terminal` event listener: it queues the effects
/// the terminal reports, and the query handler queues its own answers into
/// the same queue so that the order of answers survives.
///
/// Shared (`Arc<Mutex<_>>`) because alacritty's `Term` owns its listener and
/// reports through `&self`, while the query handler beside it needs to append
/// too; and `Arc`/`Mutex` rather than `Rc`/`RefCell` so that the emulator stays
/// `Send` for the async task that will own it. The lock is never contended:
/// only the thread feeding the emulator touches it.
#[derive(Debug, Clone, Default)]
pub struct Listener {
    queue: Arc<Mutex<Vec<Effect>>>,
}

impl Listener {
    /// Queue an effect.
    pub(crate) fn push(&self, effect: Effect) {
        self.queue
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(effect);
    }

    /// Everything queued since the last call.
    pub(crate) fn take(&self) -> Vec<Effect> {
        mem::take(&mut *self.queue.lock().unwrap_or_else(PoisonError::into_inner))
    }
}

impl EventListener for Listener {
    /// Keep only the events that are side effects. alacritty also reports
    /// redraw hints (`MouseCursorDirty`, `CursorBlinkingChange`) that matter
    /// only to a renderer reading the terminal itself, and requests
    /// (`ColorRequest`, `TextAreaSizeRequest`, `ClipboardLoad`) that this
    /// crate's query handler answers before alacritty would raise them, or
    /// that its configuration turns off.
    fn send_event(&self, event: Event) {
        let effect = match event {
            Event::PtyWrite(text) => Effect::Reply(text),
            Event::Bell => Effect::Bell,
            Event::ClipboardStore(ClipboardType::Clipboard, text) => {
                Effect::Clipboard(ClipboardTarget::Clipboard, text)
            }
            Event::ClipboardStore(ClipboardType::Selection, text) => {
                Effect::Clipboard(ClipboardTarget::Selection, text)
            }
            Event::Title(_) | Event::ResetTitle => Effect::Title,
            Event::MouseCursorDirty
            | Event::CursorBlinkingChange
            | Event::ColorRequest(..)
            | Event::TextAreaSizeRequest(_)
            | Event::ClipboardLoad(..)
            | Event::Wakeup
            | Event::Exit
            | Event::ChildExit(_) => return,
        };
        self.push(effect);
    }
}

/// Deliver queued effects to `sink` as `mode` allows (the table in
/// [`FeedMode`]'s variants).
pub(crate) fn route(effects: Vec<Effect>, mode: FeedMode, sink: &mut impl Effects) {
    for effect in effects {
        match (mode, effect) {
            (FeedMode::ReplaySeen, _) => {}
            (FeedMode::Live | FeedMode::ReplayUnseen, Effect::Reply(text)) => {
                sink.reply(text.as_bytes())
            }
            (FeedMode::Live, Effect::Bell) => sink.notice(Notice::Bell),
            (FeedMode::Live, Effect::Clipboard(target, text)) => {
                sink.notice(Notice::Clipboard { target, text })
            }
            (FeedMode::Live, Effect::Title) => sink.notice(Notice::Title),
            (FeedMode::ReplayUnseen, Effect::Bell | Effect::Clipboard(..) | Effect::Title) => {}
        }
    }
}
