//! A plain Replace that the helm refused because the source's host asks before YOLO launches: the launcher's YOLO
//! question, drawn where that Replace was asked (the source's row, or the open session's header or interrupted card),
//! and the Replace again with consent once the user confirms.
//!
//! SPEC.md, "Creation", the YOLO paragraph: the GUI answers that refusal "with a prominent confirmation, shown with the
//! control or surface that started the launch and scrolled into view", and "a plain Replace's question, which arrives
//! after its reply, takes focus only if the user has not moved on since answering Replace". The question itself (its
//! words, its answers, the permanent answer's host write) is the launcher's, asked through
//! [`entry::ask_yolo_for_replace`]; what is kept here is where it shows and what its answer sends. Here's the TLDR of
//! the rules:
//!
//! - **When.** Only a refusal carrying [`HelmMarker::YoloConfirmation`] asks, never one read from its words, which the
//!   helm writes for the command line (`super::replace::settle_sent`). The store's record of that refusal is forgotten once
//!   the question is asked, since the question says it instead; any other refusal shows on the row as before
//!   ([`super::outcome`]). A refusal whose source the store no longer holds asks nothing and keeps the record.
//! - **Where.** The question belongs to the surface the Replace was asked from ([`Surface`]): the row draws it under
//!   its lines, the header below its top row, the interrupted card inside the card (the header's place while the card
//!   is gone), or the source's row while the source is not the open session ([`shown`]).
//!   One question per session, since a session has one Replace in flight at most; a later plain Replace of the session
//!   replaces it. The question is kept here rather than by the row or the header, because the reply settles from the
//!   operation store, with neither view at hand, and the virtualized row is rebuilt on every frame.
//! - **Focus and scrolling.** The question takes focus (on Cancel) only if the user has not moved on since answering
//!   Replace, and only then does the list scroll the row into view ([`take_reveal`]): the old app's arrival test had
//!   the question leave a user who went on to type into another terminal, and their sidebar's scroll position, alone.
//!   Answering returns focus to what held it when the question arrived, which is where the Replace question left it.
//! - **The answer.** The one-off answer, or the permanent answer once its host write has succeeded, sends the same
//!   Replace again with `confirm_yolo` set: the source and the Delete guard the user's Replace question froze, on the
//!   session's own key, at most once per question. Its success opens the replacement under Replace's own rule, counting
//!   "moved on" from that answer's press rather than from the original Replace (`super::replace`).
//! - **While it is open** ([`open`]), the session's other lifecycle actions (Stop, Restart, Replace, Delete and Rename)
//!   are refused and drawn unavailable, as for any open lifecycle confirmation (SPEC.md: "an open lifecycle
//!   confirmation or a pending request refuses another lifecycle action"; `super::menu::refused`). That also keeps the
//!   permanent answer's host write from meeting a second Replace of the session, and the answer's Replace from meeting
//!   a Stop or Delete on the session's key. Open is the prompt's own `Inline::is_open`, never whether an entry is kept
//!   here: a question dropped unanswered stays open for any clone.
//! - **One UI session, and pruned once closed.** The questions are kept with the UI session they were asked under and
//!   are neither drawn nor open under another; the next question asked after a reset, or the list's next frame, starts
//!   the map over. A reset drops the host write's reply, so a question in flight then would otherwise stay open,
//!   refusing its session's actions, for good. A closed question draws nothing and refuses nothing, and the next
//!   question asked prunes it.
//!
//! NOTE: A question on a row the list's query leaves out waits there unseen until the row is drawn again, as any
//! refusal on that row would.
//!
//! [`HelmMarker::YoloConfirmation`]: crate::operations::HelmMarker::YoloConfirmation

use std::collections::HashMap;

use farhelm_proto::DeleteGuard;
use farhelm_proto::ui_channel::{HostView, SessionRow};
use gpui::{
    AnyElement, AnyWindowHandle, App, AppContext as _, Entity, Global, IntoElement,
    ParentElement as _, Styled as _, Window, div, px,
};

use crate::focus::{self, Mark};
use crate::host_text::HostText;
use crate::hosts::identity;
use crate::launcher::entry::{self, YoloAsk};
use crate::local_state;
use crate::prompt::Inline;
use crate::session::{self, PerSession};

use super::replace;
use crate::theme::{self, metrics};

/// How many frames the list keeps scrolling a row whose question took focus into view: the first frame draws the row
/// (it may have been scrolled away, and focus on a control not drawn in the next frame is lost), and the second
/// measures it with the question under it and scrolls again if the question's foot is out of view.
const REVEAL_FRAMES: u8 = 2;

/// Where a plain Replace was asked, which is where its YOLO question shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Surface {
    /// The source's row, from its menu's Replace.
    Row,
    /// The open session's header, from its Replace button.
    Header,
    /// The open session's interrupted card (`session_pane/card.rs`), from its Replace button. The card stands in the
    /// open session's pane as the header does, so its question shows where the header's would ([`shown`]): in the card
    /// while the card shows, below the top row once it does not (the session recovered meanwhile), and at the row while
    /// the session is not open.
    Card,
}

/// One session's YOLO question: where it shows, the question, and how many more frames the list scrolls its row into
/// view.
struct Asked {
    surface: Surface,
    question: Inline<YoloAsk>,
    reveal: u8,
}

/// The YOLO questions of one UI session, by the source session's key. An entity rather than plain state, so that the
/// row and the header, which read it while they draw, redraw when a question is asked (as the wait clocks in
/// [`super::outcome`] do).
pub(crate) struct Questions {
    /// The questions by the source session's key, kept with the UI session they were asked under: reads under any
    /// other find nothing, and the next change under another starts the map over.
    asked: PerSession<HashMap<String, Asked>>,
}

/// Where the [`Questions`] are kept: a global, because the list and the header read the same ones.
#[derive(Default)]
struct Kept(Option<Entity<Questions>>);

impl Global for Kept {}

/// `key`'s question under the current UI session, if one was asked. Read through [`PerSession::read`], never
/// `current`, because the row and the header read this while they draw, with only `&App`.
fn asked<'a>(key: &str, cx: &'a App) -> Option<&'a Asked> {
    let questions = cx.try_global::<Kept>()?.0.as_ref()?.read(cx);
    questions.asked.read(cx)?.get(key)
}

/// Change the current UI session's questions with `change`, after starting them over if they were kept for another
/// UI session and pruning the closed ones, and redraw whoever draws them.
fn update(cx: &mut App, change: impl FnOnce(&mut HashMap<String, Asked>)) {
    let questions = match cx.try_global::<Kept>().and_then(|kept| kept.0.clone()) {
        Some(questions) => questions,
        None => {
            let questions = cx.new(|_| Questions {
                asked: PerSession::default(),
            });
            cx.set_global(Kept(Some(questions.clone())));
            questions
        }
    };
    questions.update(cx, |questions, cx| {
        let asked = questions.asked.current(cx);
        // A closed question draws nothing and refuses nothing, so it goes at the next change.
        asked.retain(|_, asked| asked.question.is_open(cx));
        change(asked);
        cx.notify();
    });
}

/// `key`'s question for drawing on `surface`, if it shows there: where it was asked, except that a question asked from
/// the header or the interrupted card shows at the source's row while the source is not the open session, since both
/// are drawn only for the open session and the question must never be shown nowhere (SPEC.md: "A failure or partial
/// result appears on the source session's row"). The header and the card are one place, the open session's pane,
/// which draws the question in the card while it shows the card and below the top row otherwise, never both. So a
/// question shows in exactly one place, and only while it is open, so a surface's spacing around it goes with it.
pub(crate) fn shown(key: &str, surface: Surface, cx: &App) -> Option<Inline<YoloAsk>> {
    let asked = asked(key, cx)?;
    let open = local_state::selected(cx).as_deref() == Some(key);
    let here = match (asked.surface, surface) {
        (Surface::Row, Surface::Row) => true,
        (Surface::Row, Surface::Header | Surface::Card) => false,
        (Surface::Header | Surface::Card, Surface::Row) => !open,
        (Surface::Header | Surface::Card, Surface::Header | Surface::Card) => true,
    };
    (here && asked.question.is_open(cx)).then(|| asked.question.clone())
}

/// Whether `key`'s YOLO question is open under the current UI session: no answer has run (Cancel included), and the
/// permanent answer has not succeeded. While it is, the session's other lifecycle actions are refused (see the module
/// docs). Read from the prompt's own `Inline::is_open`, not from whether an entry is kept: entries go only once closed.
pub(crate) fn open(key: &str, cx: &App) -> bool {
    asked(key, cx).is_some_and(|asked| asked.question.is_open(cx))
}

/// The key of a row whose question took focus and still needs scrolling into view, counting this frame against it.
/// The list calls it at the top of each frame and scrolls that row wholly into view; it asks for one more frame while
/// this returns a key. Nothing redraws for the count itself.
pub(super) fn take_reveal(cx: &mut App) -> Option<String> {
    let questions = cx.try_global::<Kept>()?.0.clone()?;
    questions.update(cx, |questions, cx| {
        // Under another UI session (or none) this starts the map over, which drops only questions no read could find,
        // so it finds nothing to reveal, as it should.
        let (key, asked) = questions
            .asked
            .current(cx)
            .iter_mut()
            .find(|(_, asked)| asked.surface == Surface::Row && asked.reveal > 0)?;
        asked.reveal -= 1;
        Some(key.clone())
    })
}

/// The helm refused `source`'s plain Replace, asked on `surface` with `guard` at the user's answer `mark`, because its
/// host asks before YOLO launches: ask the launcher's question in `window` and keep it for `surface` to draw. Its
/// answer sends the Replace again with consent ([`replace::send_with_consent`]). Returns whether it was asked: not when
/// the store holds no row for the source any more (its host and stored launch are what the question names) or the
/// window is gone.
pub(super) fn ask(
    source: &str,
    guard: DeleteGuard,
    surface: Surface,
    mark: Mark,
    window: AnyWindowHandle,
    cx: &mut App,
) -> bool {
    let Some(store) = session::current_store(cx) else {
        return false;
    };
    let collections = store.read(cx).collections();
    let Some(row) = collections.get::<SessionRow>(source) else {
        return false;
    };
    let (host, launch) = (row.host, row.info.launch.clone());
    let host_name = host_name(row, collections.get::<HostView>(&HostView::key(row.host)));
    // The launcher runs this at most once per question (`entry::ask_yolo_for_replace`'s contract), and the operation
    // store refuses a second start on the session's key while one is out, so nothing here counts the calls.
    let key = source.to_owned();
    let send_again = move |answer: Mark, window: &mut Window, cx: &mut App| {
        replace::send_with_consent(&key, guard, surface, answer, window.window_handle(), cx);
    };
    let asked = window.update(cx, |_, window, cx| {
        entry::ask_yolo_for_replace(host, host_name, &launch, &mark, send_again, window, cx)
    });
    let Ok(question) = asked else {
        return false;
    };
    let reveal = if surface == Surface::Row && !focus::has_moved_on(&mark, cx) {
        REVEAL_FRAMES
    } else {
        0
    };
    update(cx, |asked| {
        asked.insert(
            source.to_owned(),
            Asked {
                surface,
                question,
                reveal,
            },
        );
    });
    true
}

/// The host's name as the question says it: the registry's display name ([`identity::display_name`]) while the
/// registry lists the host, else the name the helm sent with the row, else "this host", as the launcher's own question
/// falls back.
fn host_name(row: &SessionRow, host: Option<&HostView>) -> HostText {
    match host {
        Some(host) => identity::display_name(host),
        None if row.host_name.is_empty() => HostText::from_host("this host"),
        None => identity::named(&row.host_name, false),
    }
}

// ---- Drawing ---------------------------------------------------------------------------------------------------------

/// `key`'s question under its row's lines, inset from the row's sides as the row's other lines are and 8 pixels from
/// what is above and below it, as the old `.yolo-confirmation`'s margin was. It sits on the window's ground rather than
/// on the row's fill: on the selected row's accent fill the question's red wash and ordinary ink would lose their
/// contrast. `None` while the row has no open question, so the spacing goes when the question closes.
pub(super) fn row_question(key: &str, cx: &App) -> Option<impl IntoElement> {
    let question = shown(key, Surface::Row, cx)?;
    Some(
        div().py(px(8.)).px(px(14.)).child(
            div()
                .bg(theme::color(theme::palette(cx).bg_0))
                .rounded(metrics::RADIUS_MD)
                .child(question),
        ),
    )
}

/// `key`'s question for the open session's header, below its top row, inset from the sides as the header's refusal
/// line is and 8 pixels from what is above and below it (the old `.yolo-confirmation`'s margin). `None` while the
/// session has no open question asked from the header.
pub(crate) fn header_question(key: &str, cx: &App) -> Option<AnyElement> {
    let question = shown(key, Surface::Header, cx)?;
    Some(
        div()
            .py(px(8.))
            .px(px(10.))
            .child(question)
            .into_any_element(),
    )
}
